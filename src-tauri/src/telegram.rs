//! Telegram MTProto (grammers): вход в аккаунт, каталог книг публичных каналов, импорт в библиотеку.
//!
//! Безопасность. Ключ авторизации даёт полный доступ к аккаунту, поэтому:
//! - он хранится в системной связке ключей (см. `telegram_vault`), а не в файле рядом с настройками;
//!   без связки ключей — в файле 0600, и интерфейс об этом предупреждает;
//! - папка `telegram/` (API ID/Hash, телефон) лежит в каталоге приложения, не в библиотеке
//!   (не уезжает с синхронизацией), права 0700, файлы — 0600;
//! - в webview отдаются только имя, телефон и API ID — ни ключ, ни API Hash наружу не выходят;
//! - при выходе сеанс завершается на сервере (`auth.logOut`), а не только удаляется локально;
//! - в Telegram → Настройки → Устройства сеанс виден как «Reader» и его можно завершить оттуда.

use crate::formats::{content_matches, existing_copy, unique_dest};
use crate::storage::{app_dir, load_config};
use crate::telegram_vault::VaultSession;
use grammers_client::client::{LoginToken, PasswordToken};
use grammers_client::media::Media;
use grammers_client::message::Message;
use grammers_client::peer::Peer;
use grammers_client::{tl, Client, SignInError};
use grammers_mtsender::{ConnectionParams, InvocationError, SenderPool};
use grammers_session::types::PeerRef;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tauri::{AppHandle, Emitter, State};
use tokio::sync::Mutex;

const CREDS_NAME: &str = "creds.json";
const NOT_SIGNED_IN: &str = "Сначала войдите в Telegram в настройках";
/// Сколько документов просматривать за один запрос каталога (книги среди них — не все).
const MAX_SCAN: usize = 1000;
/// `channels.getMessages` принимает не больше 100 id за раз.
const FETCH_CHUNK: usize = 100;
/// Защита от «книги» на несколько гигабайт.
const MAX_BOOK_SIZE: u64 = 1024 * 1024 * 1024;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct TelegramCreds {
    api_id: i32,
    api_hash: String,
    #[serde(default)]
    phone: String,
}

#[derive(Debug, Clone, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct TelegramStatus {
    pub connected: bool,
    pub phone: Option<String>,
    pub display_name: Option<String>,
    /// Сохранённый API ID — чтобы форма входа не требовала вводить его заново.
    pub api_id: Option<i32>,
    /// API Hash сохранён в бэкенде (сам hash в webview не отдаётся).
    pub has_api_hash: bool,
    /// Ключ сессии в связке ключей ОС (`false` — в файле; `None` — неизвестно).
    pub secure_storage: Option<bool>,
}

#[derive(Debug, Clone, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct TelegramChannelInfo {
    pub username: String,
    pub title: String,
}

#[derive(Debug, Clone, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct RemoteBookFile {
    pub id: String,
    pub name: String,
    pub size: u64,
    pub ext: String,
    pub message_id: i32,
    pub date: Option<String>,
    pub caption: Option<String>,
    /// Файл с таким же именем и размером уже лежит в библиотеке.
    pub in_library: bool,
}

#[derive(Debug, Clone, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct RemoteFilesPage {
    pub files: Vec<RemoteBookFile>,
    /// Передать в следующий запрос, чтобы получить более старые файлы; `None` — дальше пусто.
    pub next_offset_id: Option<i32>,
}

#[derive(Debug, Clone, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ImportFailure {
    pub message_id: i32,
    pub name: String,
    pub error: String,
}

#[derive(Debug, Clone, Serialize, Default, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ImportResult {
    /// Новые книги (пути относительно библиотеки).
    pub added: Vec<String>,
    /// Уже были в библиотеке — не скачивались.
    pub existing: Vec<String>,
    pub failed: Vec<ImportFailure>,
}

#[derive(Debug, Clone, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ImportProgress {
    done: usize,
    total: usize,
    name: String,
}

#[derive(Debug, Clone, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct TelegramSignInResult {
    pub ok: bool,
    pub need_password: Option<bool>,
    pub password_hint: Option<String>,
    pub status: Option<TelegramStatus>,
}

struct LiveClient {
    client: Client,
    secure: bool,
    /// Сетевой цикл пула соединений; завершается после `disconnect()`.
    runner: tokio::task::JoinHandle<()>,
}

/// Мьютекс защищает только состояние; сетевые запросы каталога и загрузки идут на клоне `Client`
/// без блокировки, чтобы долгий импорт не подвешивал остальные команды.
#[derive(Default)]
pub struct TelegramState {
    inner: Mutex<TelegramInner>,
}

#[derive(Default)]
struct TelegramInner {
    live: Option<LiveClient>,
    login_token: Option<LoginToken>,
    password_token: Option<PasswordToken>,
    creds: Option<TelegramCreds>,
    /// username (lowercase) → PeerRef. `contacts.resolveUsername` жёстко ограничен по частоте.
    peers: HashMap<String, PeerRef>,
}

#[cfg(unix)]
fn restrict(path: &Path, mode: u32) {
    use std::os::unix::fs::PermissionsExt;
    if path.exists() {
        let _ = std::fs::set_permissions(path, std::fs::Permissions::from_mode(mode));
    }
}

#[cfg(not(unix))]
fn restrict(_path: &Path, _mode: u32) {}

fn telegram_dir(app: &AppHandle) -> PathBuf {
    let d = app_dir(app).join("telegram");
    let _ = std::fs::create_dir_all(&d);
    restrict(&d, 0o700);
    d
}

fn restrict_private_files(dir: &Path) {
    restrict(&dir.join(CREDS_NAME), 0o600);
}

/// Связка ключей ОС отвечает через D-Bus — блокирующие вызовы уводим с async-потока.
async fn open_vault(app: &AppHandle) -> Result<VaultSession, String> {
    let dir = telegram_dir(app);
    let d = dir.clone();
    let vault = tokio::task::spawn_blocking(move || VaultSession::open(&d))
        .await
        .map_err(|e| e.to_string())??;
    vault.migrate_legacy(&dir).await;
    Ok(vault)
}

/// (есть ли сохранённая сессия, хранится ли она в связке ключей)
async fn saved_session(app: &AppHandle) -> (bool, Option<bool>) {
    match open_vault(app).await {
        Ok(v) => (v.has_auth_key(), Some(v.is_secure())),
        Err(_) => (false, None),
    }
}

async fn clear_session(app: &AppHandle) {
    let dir = telegram_dir(app);
    let _ = tokio::task::spawn_blocking(move || VaultSession::clear(&dir)).await;
}

fn load_creds(app: &AppHandle) -> Option<TelegramCreds> {
    let s = std::fs::read_to_string(telegram_dir(app).join(CREDS_NAME)).ok()?;
    serde_json::from_str(&s).ok()
}

fn save_creds(app: &AppHandle, creds: &TelegramCreds) -> Result<(), String> {
    let dir = telegram_dir(app);
    let s = serde_json::to_string_pretty(creds).map_err(|e| e.to_string())?;
    std::fs::write(dir.join(CREDS_NAME), s).map_err(|e| e.to_string())?;
    restrict_private_files(&dir);
    Ok(())
}

fn human_wait(secs: u32) -> String {
    match secs {
        0..=59 => format!("{secs} с"),
        60..=3599 => format!("{} мин", secs.div_ceil(60)),
        _ => format!("{} ч", secs.div_ceil(3600)),
    }
}

fn map_err(e: InvocationError) -> String {
    let InvocationError::Rpc(rpc) = &e else {
        return format!("Telegram: {e}");
    };
    match rpc.name.as_str() {
        "FLOOD_WAIT" | "FLOOD_PREMIUM_WAIT" => format!(
            "Telegram временно ограничил запросы. Повторите через {}.",
            human_wait(rpc.value.unwrap_or(0))
        ),
        "API_ID_INVALID" | "API_ID_PUBLISHED_FLOOD" => {
            "Неверные API ID / API Hash — проверьте их на my.telegram.org".into()
        }
        "PHONE_NUMBER_INVALID" => "Неверный номер телефона".into(),
        "PHONE_NUMBER_BANNED" => "Этот номер заблокирован в Telegram".into(),
        "PHONE_CODE_EXPIRED" => "Код устарел — запросите новый".into(),
        "PHONE_PASSWORD_FLOOD" => "Слишком много попыток ввода пароля. Попробуйте позже.".into(),
        "USERNAME_INVALID" | "USERNAME_NOT_OCCUPIED" => "Канал не найден".into(),
        "CHANNEL_PRIVATE" | "CHANNEL_INVALID" => {
            "Канал приватный или недоступен для вашего аккаунта".into()
        }
        "AUTH_KEY_UNREGISTERED" | "SESSION_REVOKED" | "USER_DEACTIVATED" => {
            "Сеанс Telegram завершён — войдите снова в настройках".into()
        }
        _ => format!("Telegram: {e}"),
    }
}

fn map_sign_in_err(e: SignInError) -> String {
    match e {
        SignInError::Other(e) => map_err(e),
        e => format!("Telegram: {e}"),
    }
}

/// `@name`, `name`, `t.me/name`, `https://t.me/s/name/123?x` → `name`.
fn parse_channel_input(raw: &str) -> Result<String, String> {
    let t = raw.trim();
    if t.is_empty() {
        return Err("Пустая ссылка на канал".into());
    }
    let rest = t
        .strip_prefix("https://")
        .or_else(|| t.strip_prefix("http://"))
        .unwrap_or(t);
    let candidate = if let Some(name) = rest.strip_prefix('@') {
        name
    } else if let Some((host, path)) = rest.split_once('/') {
        let host = host.to_ascii_lowercase();
        let host = host.strip_prefix("www.").unwrap_or(&host);
        if !matches!(host, "t.me" | "telegram.me" | "telegram.dog") {
            return Err("Ожидается ссылка t.me/… или @username".into());
        }
        let path = path.split(['?', '#']).next().unwrap_or("");
        let mut parts = path.split('/').filter(|s| !s.is_empty());
        let first = parts.next().ok_or("В ссылке нет имени канала")?;
        if first.starts_with('+') || matches!(first, "c" | "joinchat" | "addlist") {
            return Err(
                "Приватные ссылки-приглашения не поддерживаются — нужен публичный @username".into(),
            );
        }
        if first == "s" {
            parts.next().ok_or("В ссылке нет имени канала")?
        } else {
            first
        }
    } else {
        rest
    };
    let name = candidate.trim_start_matches('@');
    if is_username(name) {
        Ok(name.to_string())
    } else {
        Err("Некорректное имя канала".into())
    }
}

fn is_username(u: &str) -> bool {
    (4..=32).contains(&u.len())
        && u.starts_with(|c: char| c.is_ascii_alphabetic())
        && u.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
}

fn normalize_phone(raw: &str) -> Result<String, String> {
    let digits: String = raw.chars().filter(|c| c.is_ascii_digit()).collect();
    let only_allowed = raw
        .chars()
        .all(|c| c.is_ascii_digit() || matches!(c, '+' | ' ' | '-' | '(' | ')'));
    if !only_allowed || !(7..=15).contains(&digits.len()) {
        return Err("Укажите телефон в международном формате (+7…)".into());
    }
    Ok(format!("+{digits}"))
}

fn is_api_hash(s: &str) -> bool {
    s.len() == 32 && s.chars().all(|c| c.is_ascii_hexdigit())
}

/// Книжный файл (pdf, epub, fb2, fb2.zip) — по имени из Telegram.
fn book_ext(name: &str) -> Option<&'static str> {
    crate::formats::book_kind(name).filter(|_| crate::formats::is_importable(name))
}

fn library_root(app: &AppHandle) -> Result<PathBuf, String> {
    let root = load_config(app)?
        .library_root
        .ok_or("Сначала выберите папку библиотеки")?;
    PathBuf::from(root)
        .canonicalize()
        .map_err(|e| e.to_string())
}

fn rel_path(root: &Path, p: &Path) -> String {
    p.strip_prefix(root)
        .unwrap_or(p)
        .to_string_lossy()
        .replace('\\', "/")
}

async fn connect_client(app: &AppHandle, api_id: i32) -> Result<LiveClient, String> {
    let vault = open_vault(app)
        .await
        .map_err(|e| format!("Сессия Telegram: {e}"))?;
    let secure = vault.is_secure();
    let session = Arc::new(vault);
    // Так сеанс подписан в Telegram → Устройства.
    let params = ConnectionParams {
        device_model: "Reader".into(),
        system_version: std::env::consts::OS.into(),
        app_version: env!("CARGO_PKG_VERSION").into(),
        system_lang_code: "ru".into(),
        lang_code: "ru".into(),
        ..Default::default()
    };
    let SenderPool { runner, handle, .. } = SenderPool::with_configuration(session, api_id, params);
    let client = Client::new(handle);
    let runner = tokio::spawn(runner.run());
    Ok(LiveClient {
        client,
        secure,
        runner,
    })
}

async fn disconnect(live: LiveClient) {
    live.client.disconnect();
    let _ = live.runner.await;
}

async fn ensure_live(app: &AppHandle, inner: &mut TelegramInner) -> Result<Client, String> {
    if let Some(live) = &inner.live {
        return Ok(live.client.clone());
    }
    let creds = inner
        .creds
        .clone()
        .or_else(|| load_creds(app))
        .ok_or("Сначала войдите в Telegram (укажите API ID и телефон)")?;
    let live = connect_client(app, creds.api_id).await?;
    let client = live.client.clone();
    inner.creds = Some(creds);
    inner.live = Some(live);
    Ok(client)
}

/// Клиент с действующей авторизацией; блокировка состояния снимается до сетевых запросов.
async fn authorized_client(app: &AppHandle, state: &TelegramState) -> Result<Client, String> {
    let client = {
        let mut inner = state.inner.lock().await;
        if inner.live.is_none() && !saved_session(app).await.0 {
            return Err(NOT_SIGNED_IN.into());
        }
        ensure_live(app, &mut inner).await?
    };
    if !client.is_authorized().await.map_err(map_err)? {
        return Err(NOT_SIGNED_IN.into());
    }
    Ok(client)
}

async fn status_from_client(
    client: &Client,
    creds: Option<&TelegramCreds>,
    secure: bool,
) -> TelegramStatus {
    let mut st = offline_status(creds, Some(secure));
    if !matches!(client.is_authorized().await, Ok(true)) {
        return st;
    }
    st.connected = true;
    st.display_name = client.get_me().await.ok().and_then(|u| {
        let name = format!(
            "{} {}",
            u.first_name().unwrap_or(""),
            u.last_name().unwrap_or("")
        );
        let name = name.trim();
        if name.is_empty() {
            u.username().map(str::to_string)
        } else {
            Some(name.to_string())
        }
    });
    st
}

fn offline_status(creds: Option<&TelegramCreds>, secure_storage: Option<bool>) -> TelegramStatus {
    TelegramStatus {
        connected: false,
        phone: creds.map(|c| c.phone.clone()).filter(|p| !p.is_empty()),
        display_name: None,
        api_id: creds.map(|c| c.api_id).filter(|&id| id > 0),
        has_api_hash: creds.is_some_and(|c| is_api_hash(&c.api_hash)),
        secure_storage,
    }
}

fn extract_book(msg: &Message) -> Option<RemoteBookFile> {
    let Media::Document(doc) = msg.media()? else {
        return None;
    };
    let name = doc.name()?.trim().to_string();
    let ext = book_ext(&name)?.to_string();
    let message_id = msg.id();
    let caption = msg.text().trim();
    Some(RemoteBookFile {
        id: message_id.to_string(),
        size: doc.size().unwrap_or(0) as u64,
        ext,
        message_id,
        date: Some(msg.date().to_string()),
        caption: (!caption.is_empty()).then(|| caption.chars().take(280).collect()),
        in_library: false,
        name,
    })
}

async fn resolve_peer(client: &Client, username: &str) -> Result<(Peer, PeerRef), String> {
    let peer = client
        .resolve_username(username)
        .await
        .map_err(map_err)?
        .ok_or_else(|| format!("Канал @{username} не найден"))?;
    let peer_ref = peer
        .to_ref()
        .await
        .map_err(|e| format!("Канал @{username}: {e}"))?
        .ok_or_else(|| format!("Нет доступа к каналу @{username}"))?;
    Ok((peer, peer_ref))
}

async fn channel_ref(
    client: &Client,
    state: &TelegramState,
    username: &str,
) -> Result<PeerRef, String> {
    let key = username.to_ascii_lowercase();
    if let Some(r) = state.inner.lock().await.peers.get(&key).copied() {
        return Ok(r);
    }
    let (_, r) = resolve_peer(client, username).await?;
    state.inner.lock().await.peers.insert(key, r);
    Ok(r)
}

#[tauri::command]
#[specta::specta]
pub async fn telegram_status(
    app: AppHandle,
    state: State<'_, TelegramState>,
) -> Result<TelegramStatus, String> {
    let mut inner = state.inner.lock().await;
    if inner.creds.is_none() {
        inner.creds = load_creds(&app);
    }
    if inner.live.is_none() {
        let (has, secure) = saved_session(&app).await;
        if !has {
            return Ok(offline_status(inner.creds.as_ref(), secure));
        }
    }
    match ensure_live(&app, &mut inner).await {
        Ok(client) => {
            let creds = inner.creds.clone();
            let secure = inner.live.as_ref().is_some_and(|l| l.secure);
            drop(inner);
            Ok(status_from_client(&client, creds.as_ref(), secure).await)
        }
        Err(_) => Ok(offline_status(inner.creds.as_ref(), None)),
    }
}

/// Пустой `api_hash` — взять сохранённый (форма не получает его обратно из бэкенда).
#[tauri::command]
#[specta::specta]
pub async fn telegram_send_code(
    app: AppHandle,
    state: State<'_, TelegramState>,
    api_id: i32,
    api_hash: String,
    phone: String,
) -> Result<(), String> {
    if api_id <= 0 {
        return Err("Укажите API ID с my.telegram.org".into());
    }
    let phone = normalize_phone(&phone)?;
    let api_hash = match api_hash.trim() {
        "" => load_creds(&app)
            .filter(|c| c.api_id == api_id)
            .map(|c| c.api_hash)
            .unwrap_or_default(),
        h => h.to_ascii_lowercase(),
    };
    if !is_api_hash(&api_hash) {
        return Err("API Hash — 32 шестнадцатеричных символа с my.telegram.org".into());
    }

    let mut inner = state.inner.lock().await;
    // Новое подключение: API ID мог поменяться.
    if let Some(live) = inner.live.take() {
        disconnect(live).await;
    }
    inner.login_token = None;
    inner.password_token = None;
    let live = connect_client(&app, api_id).await?;
    let client = live.client.clone();
    inner.live = Some(live);

    if client.is_authorized().await.map_err(map_err)? {
        return Err("Вход уже выполнен. Чтобы сменить аккаунт, сначала выйдите.".into());
    }
    let token = client
        .request_login_code(&phone, &api_hash)
        .await
        .map_err(map_err)?;

    let creds = TelegramCreds {
        api_id,
        api_hash,
        phone,
    };
    save_creds(&app, &creds)?;
    inner.creds = Some(creds);
    inner.login_token = Some(token);
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub async fn telegram_sign_in(
    app: AppHandle,
    state: State<'_, TelegramState>,
    code: String,
    password: Option<String>,
) -> Result<TelegramSignInResult, String> {
    let mut inner = state.inner.lock().await;
    let client = ensure_live(&app, &mut inner).await?;

    // Пароль не обрезаем: пробелы по краям — допустимая часть пароля.
    let _user = if let Some(pwd) = password.filter(|p| !p.is_empty()) {
        let token = inner
            .password_token
            .take()
            .ok_or("Сначала введите код из Telegram, затем пароль 2FA")?;
        match client.check_password(token, pwd.as_bytes()).await {
            Ok(user) => user,
            Err(SignInError::InvalidPassword(token)) => {
                inner.password_token = Some(token);
                return Err("Неверный пароль двухфакторной аутентификации".into());
            }
            Err(e) => return Err(map_sign_in_err(e)),
        }
    } else {
        let code: String = code.chars().filter(|c| c.is_ascii_digit()).collect();
        if code.is_empty() {
            return Err("Введите код из Telegram".into());
        }
        let token = inner
            .login_token
            .take()
            .ok_or("Сначала запросите код (кнопка «Отправить код»)")?;
        match client.sign_in(&token, &code).await {
            Ok(user) => user,
            Err(SignInError::PasswordRequired(pwd_token)) => {
                let hint = pwd_token.hint().map(str::to_string);
                inner.password_token = Some(pwd_token);
                return Ok(TelegramSignInResult {
                    ok: false,
                    need_password: Some(true),
                    password_hint: hint,
                    status: None,
                });
            }
            Err(SignInError::InvalidCode) => {
                inner.login_token = Some(token);
                return Err("Неверный код".into());
            }
            Err(SignInError::SignUpRequired) => {
                return Err("Номер не зарегистрирован в Telegram. Создайте аккаунт в официальном приложении.".into());
            }
            Err(e) => return Err(map_sign_in_err(e)),
        }
    };

    inner.login_token = None;
    inner.password_token = None;
    let creds = inner.creds.clone();
    let secure = inner.live.as_ref().is_some_and(|l| l.secure);
    drop(inner);
    Ok(TelegramSignInResult {
        ok: true,
        need_password: None,
        password_hint: None,
        status: Some(status_from_client(&client, creds.as_ref(), secure).await),
    })
}

/// Возвращает `true`, если сеанс завершён и на сервере Telegram. `false` — локальные файлы
/// удалены, но сервер не подтвердил выход (нет сети): сеанс «Reader» стоит завершить вручную
/// в Telegram → Настройки → Устройства.
#[tauri::command]
#[specta::specta]
pub async fn telegram_logout(
    app: AppHandle,
    state: State<'_, TelegramState>,
) -> Result<bool, String> {
    let mut inner = state.inner.lock().await;
    inner.login_token = None;
    inner.password_token = None;
    inner.peers.clear();

    let mut revoked = inner.live.is_none() && !saved_session(&app).await.0;
    if !revoked {
        if let Ok(client) = ensure_live(&app, &mut inner).await {
            revoked = match client.is_authorized().await {
                Ok(true) => client.sign_out().await.is_ok(),
                Ok(false) => true,
                Err(_) => false,
            };
        }
    }
    if let Some(live) = inner.live.take() {
        disconnect(live).await;
    }
    clear_session(&app).await;
    Ok(revoked)
}

#[tauri::command]
#[specta::specta]
pub async fn telegram_resolve_channel(
    app: AppHandle,
    state: State<'_, TelegramState>,
    input: String,
) -> Result<TelegramChannelInfo, String> {
    let username = parse_channel_input(&input)?;
    let client = authorized_client(&app, &state).await?;
    let (peer, peer_ref) = resolve_peer(&client, &username).await?;
    if matches!(peer, Peer::User(_)) {
        return Err(format!(
            "@{username} — это пользователь или бот, а не канал"
        ));
    }
    state
        .inner
        .lock()
        .await
        .peers
        .insert(username.to_ascii_lowercase(), peer_ref);
    Ok(TelegramChannelInfo {
        title: peer
            .name()
            .map(str::to_string)
            .unwrap_or_else(|| format!("@{username}")),
        username: peer.username().map(str::to_string).unwrap_or(username),
    })
}

/// Страница книг канала (от новых к старым). Ищем через `messages.search` с фильтром документов —
/// так не приходится листать всю ленту с картинками и текстом.
#[tauri::command]
#[specta::specta]
pub async fn source_list_files(
    app: AppHandle,
    state: State<'_, TelegramState>,
    username: String,
    query: Option<String>,
    offset_id: Option<i32>,
    limit: Option<u32>,
) -> Result<RemoteFilesPage, String> {
    let username = parse_channel_input(&username)?;
    let limit = limit.unwrap_or(50).clamp(1, 200) as usize;
    let client = authorized_client(&app, &state).await?;
    let peer = channel_ref(&client, &state, &username).await?;
    let root = library_root(&app).ok();

    let mut iter = client
        .search_messages(peer)
        .filter(tl::enums::MessagesFilter::InputMessagesFilterDocument);
    if let Some(q) = query.as_deref().map(str::trim).filter(|q| !q.is_empty()) {
        iter = iter.query(q);
    }
    if let Some(off) = offset_id.filter(|&o| o > 0) {
        iter = iter.offset_id(off);
    }

    let mut files = Vec::new();
    let mut last_id = None;
    let mut scanned = 0;
    let mut more = false;
    while let Some(msg) = iter.next().await.map_err(map_err)? {
        last_id = Some(msg.id());
        scanned += 1;
        if let Some(mut f) = extract_book(&msg) {
            f.in_library = root
                .as_deref()
                .is_some_and(|r| existing_copy(r, &f.name, f.size).is_some());
            files.push(f);
        }
        if files.len() >= limit || scanned >= MAX_SCAN {
            more = true;
            break;
        }
    }
    Ok(RemoteFilesPage {
        files,
        next_offset_id: if more { last_id } else { None },
    })
}

/// Сколько книг появилось в канале после `since_ms` (до 100 — дальше не считаем).
/// Для значка «новое» у источника: один поисковый запрос на канал.
#[tauri::command]
#[specta::specta]
pub async fn source_count_new(
    app: AppHandle,
    state: State<'_, TelegramState>,
    username: String,
    since_ms: u64,
) -> Result<u32, String> {
    let username = parse_channel_input(&username)?;
    let client = authorized_client(&app, &state).await?;
    let peer = channel_ref(&client, &state, &username).await?;
    let mut iter = client
        .search_messages(peer)
        .filter(tl::enums::MessagesFilter::InputMessagesFilterDocument);
    let since_s = (since_ms / 1000) as i64;
    let mut count = 0u32;
    let mut scanned = 0;
    while let Some(msg) = iter.next().await.map_err(map_err)? {
        if msg.date().as_second() <= since_s || scanned >= MAX_SCAN || count >= 100 {
            break;
        }
        scanned += 1;
        if extract_book(&msg).is_some() {
            count += 1;
        }
    }
    Ok(count)
}

/// Скачивает книги в корень библиотеки. Имя и тип берутся только из Telegram, не из webview.
/// Ошибка одного файла не прерывает остальные. Прогресс — событие `telegram-import-progress`.
#[tauri::command]
#[specta::specta]
pub async fn source_import_files(
    app: AppHandle,
    state: State<'_, TelegramState>,
    username: String,
    message_ids: Vec<i32>,
) -> Result<ImportResult, String> {
    let mut ids = message_ids;
    ids.retain(|&id| id > 0);
    ids.sort_unstable();
    ids.dedup();
    let mut result = ImportResult::default();
    if ids.is_empty() {
        return Ok(result);
    }
    let username = parse_channel_input(&username)?;
    let root = library_root(&app)?;
    let client = authorized_client(&app, &state).await?;
    let peer = channel_ref(&client, &state, &username).await?;

    let total = ids.len();
    let mut done = 0;
    for chunk in ids.chunks(FETCH_CHUNK) {
        let fetched = client
            .get_messages_by_id(peer, chunk)
            .await
            .map_err(map_err)?;
        for (&message_id, msg) in chunk.iter().zip(fetched) {
            let name = msg
                .as_ref()
                .and_then(extract_book)
                .map(|f| f.name)
                .unwrap_or_else(|| format!("#{message_id}"));
            let _ = app.emit(
                "telegram-import-progress",
                ImportProgress {
                    done,
                    total,
                    name: name.clone(),
                },
            );
            done += 1;
            match import_one(&client, &root, msg).await {
                Ok(Imported::Added(p)) => result.added.push(rel_path(&root, &p)),
                Ok(Imported::Existing(p)) => result.existing.push(rel_path(&root, &p)),
                Err(error) => result.failed.push(ImportFailure {
                    message_id,
                    name,
                    error,
                }),
            }
        }
    }
    let _ = app.emit(
        "telegram-import-progress",
        ImportProgress {
            done,
            total,
            name: String::new(),
        },
    );
    Ok(result)
}

enum Imported {
    Added(PathBuf),
    Existing(PathBuf),
}

async fn import_one(
    client: &Client,
    root: &Path,
    msg: Option<Message>,
) -> Result<Imported, String> {
    let msg = msg.ok_or("Сообщение удалено или недоступно")?;
    let file = extract_book(&msg).ok_or("В сообщении нет файла PDF, EPUB или FB2")?;
    if file.size > MAX_BOOK_SIZE {
        return Err("Файл больше 1 ГБ".into());
    }
    if let Some(p) = existing_copy(root, &file.name, file.size) {
        return Ok(Imported::Existing(p));
    }
    let media = msg.media().ok_or("В сообщении нет файла")?;

    // Скрытый временный файл рядом с библиотекой: при обрыве в каталоге не появится битая книга.
    let tmp = root.join(format!(".tg-{}.part", msg.id()));
    let downloaded = client.download_media(&media, &tmp).await.map_err(map_err);
    let checked = downloaded.and_then(|()| {
        if content_matches(&tmp, &file.ext) {
            Ok(())
        } else {
            Err(format!(
                "Содержимое не похоже на {}",
                file.ext.to_uppercase()
            ))
        }
    });
    if let Err(e) = checked {
        let _ = tokio::fs::remove_file(&tmp).await;
        return Err(e);
    }
    // Имя выбираем после загрузки — параллельный импорт мог занять исходное.
    let dest = unique_dest(root, &file.name);
    if let Err(e) = tokio::fs::rename(&tmp, &dest).await {
        let _ = tokio::fs::remove_file(&tmp).await;
        return Err(format!("Сохранение файла: {e}"));
    }
    Ok(Imported::Added(dest))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_channel_links() {
        for (input, want) in [
            ("@books_ru", "books_ru"),
            ("books_ru", "books_ru"),
            ("t.me/books_ru", "books_ru"),
            ("https://t.me/Books_Ru/", "Books_Ru"),
            ("https://t.me/s/books_ru/123?single", "books_ru"),
            ("http://www.telegram.me/books_ru#x", "books_ru"),
            ("https://telegram.dog/books_ru/42", "books_ru"),
        ] {
            assert_eq!(parse_channel_input(input).as_deref(), Ok(want), "{input}");
        }
        for input in [
            "",
            "https://t.me/+AbCdEf",
            "https://t.me/joinchat/AbCdEf",
            "https://t.me/c/123/45",
            "https://evil.com/books_ru",
            "https://t.me/",
            "@1abc",
            "@ab",
        ] {
            assert!(parse_channel_input(input).is_err(), "{input}");
        }
    }

    #[test]
    fn detects_book_extensions() {
        assert_eq!(book_ext("Книга.PDF"), Some("pdf"));
        assert_eq!(book_ext("x.epub"), Some("epub"));
        assert_eq!(book_ext("x.fb2.zip"), Some("fb2.zip"));
        assert_eq!(book_ext("x.typ"), None);
        assert_eq!(book_ext("pdf"), None);
    }

    #[test]
    fn validates_phone_and_hash() {
        assert_eq!(
            normalize_phone("+7 (999) 123-45-67").as_deref(),
            Ok("+79991234567")
        );
        assert!(normalize_phone("+7abc").is_err());
        assert!(is_api_hash("0123456789abcdef0123456789ABCDEF"));
        assert!(!is_api_hash("0123456789abcdef"));
    }
}
