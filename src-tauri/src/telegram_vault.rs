//! Хранилище сессии Telegram.
//!
//! Ключи авторизации (`DcOption::auth_key`) — единственное, что даёт доступ к аккаунту, — лежат
//! в системной связке ключей: Secret Service (GNOME Keyring / KWallet), Keychain или
//! Credential Manager. Скопировать папку настроек недостаточно, чтобы угнать сеанс.
//! Кэш пиров и состояние обновлений держим только в памяти: они восстанавливаются сами.
//!
//! Если связки ключей нет (минимальное окружение без Secret Service), ключи пишутся в файл
//! с правами 0600 — интерфейс сообщает об этом пользователю.

use grammers_session::storages::{MemorySession, SqliteSession};
use grammers_session::types::{DcOption, PeerId, PeerInfo, UpdateState, UpdatesState};
use grammers_session::{BoxFuture, Session, SessionData};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

const KEYRING_SERVICE: &str = "com.adaman.reader.telegram";
const KEYRING_USER: &str = "session";
/// Запасной вариант без связки ключей.
pub const FALLBACK_FILE: &str = "session.json";
/// Сессия первой версии интеграции (SQLite с ключами в открытом виде) — переносится и удаляется.
pub const LEGACY_SQLITE: &str = "session.db";

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct Persisted {
    home_dc: Option<i32>,
    dc_options: Vec<DcOption>,
}

impl Persisted {
    fn is_empty(&self) -> bool {
        self.home_dc.is_none() && self.dc_options.iter().all(|o| o.auth_key.is_none())
    }
}

#[derive(Debug, Clone, PartialEq)]
enum Backend {
    Keyring,
    File(PathBuf),
}

#[derive(Debug)]
pub struct VaultError(String);

impl std::fmt::Display for VaultError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "хранилище сессии: {}", self.0)
    }
}

impl std::error::Error for VaultError {}

fn err(e: impl std::fmt::Display) -> VaultError {
    VaultError(e.to_string())
}

fn entry() -> keyring::Result<keyring::Entry> {
    keyring::Entry::new(KEYRING_SERVICE, KEYRING_USER)
}

/// Связка ключей работает: запись либо есть, либо её просто ещё нет.
fn keyring_read() -> Option<Option<String>> {
    match entry().and_then(|e| e.get_password()) {
        Ok(s) => Some(Some(s)),
        Err(keyring::Error::NoEntry) => Some(None),
        Err(_) => None,
    }
}

#[cfg(unix)]
fn write_private(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    use std::io::Write;
    use std::os::unix::fs::OpenOptionsExt;
    let tmp = path.with_extension("tmp");
    let mut f = std::fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .mode(0o600)
        .open(&tmp)?;
    f.write_all(bytes)?;
    f.sync_all()?;
    std::fs::rename(tmp, path)
}

#[cfg(not(unix))]
fn write_private(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    std::fs::write(path, bytes)
}

pub struct VaultSession {
    mem: MemorySession,
    persisted: Mutex<Persisted>,
    backend: Backend,
}

impl VaultSession {
    /// Открывает сохранённую сессию. Блокирующий вызов (D-Bus) — звать из `spawn_blocking`.
    pub fn open(dir: &Path) -> Result<Self, String> {
        let fallback = dir.join(FALLBACK_FILE);
        let (backend, raw) = match keyring_read() {
            Some(raw) => (Backend::Keyring, raw),
            None => (
                Backend::File(fallback.clone()),
                std::fs::read_to_string(&fallback).ok(),
            ),
        };
        let mut persisted: Persisted = raw
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default();
        // Связка ключей появилась после того, как сессия жила в файле, — переносим.
        if backend == Backend::Keyring && persisted.is_empty() {
            if let Some(p) = std::fs::read_to_string(&fallback)
                .ok()
                .and_then(|s| serde_json::from_str::<Persisted>(&s).ok())
            {
                persisted = p;
            }
        }
        let vault = Self::from_persisted(persisted, backend);
        if vault.backend == Backend::Keyring && fallback.exists() {
            vault.save().map_err(|e| e.to_string())?;
            let _ = std::fs::remove_file(&fallback);
        }
        Ok(vault)
    }

    fn from_persisted(persisted: Persisted, backend: Backend) -> Self {
        let mut data = SessionData::default();
        if let Some(dc) = persisted.home_dc {
            data.home_dc = dc;
        }
        for o in &persisted.dc_options {
            data.dc_options.insert(o.id, o.clone());
        }
        Self {
            mem: MemorySession::from(data),
            persisted: Mutex::new(persisted),
            backend,
        }
    }

    /// Ключи в связке ключей ОС (а не в файле).
    pub fn is_secure(&self) -> bool {
        self.backend == Backend::Keyring
    }

    /// Есть ли что-то сохранённое (без подключения к Telegram).
    pub fn has_auth_key(&self) -> bool {
        !self.persisted.lock().map(|p| p.is_empty()).unwrap_or(true)
    }

    /// Переносит ключи из SQLite-сессии первой версии и удаляет её файлы.
    pub async fn migrate_legacy(&self, dir: &Path) {
        let legacy = dir.join(LEGACY_SQLITE);
        if !legacy.exists() {
            return;
        }
        if !self.has_auth_key() {
            if let Ok(old) = SqliteSession::open(&legacy).await {
                if let Ok(dc) = old.home_dc_id() {
                    let _ = self.set_home_dc_id(dc).await;
                }
                for id in 1..=5 {
                    if let Ok(Some(o)) = old.dc_option(id) {
                        let _ = self.set_dc_option(&o).await;
                    }
                }
            }
        }
        for name in [
            LEGACY_SQLITE.to_string(),
            format!("{LEGACY_SQLITE}-wal"),
            format!("{LEGACY_SQLITE}-shm"),
        ] {
            let _ = std::fs::remove_file(dir.join(name));
        }
    }

    fn save(&self) -> Result<(), VaultError> {
        let json = {
            let p = self.persisted.lock().map_err(err)?;
            serde_json::to_string(&*p).map_err(err)?
        };
        match &self.backend {
            Backend::Keyring => entry().and_then(|e| e.set_password(&json)).map_err(err),
            Backend::File(path) => write_private(path, json.as_bytes()).map_err(err),
        }
    }

    /// Стирает сохранённую сессию везде, где она могла лежать.
    pub fn clear(dir: &Path) {
        let _ = entry().and_then(|e| e.delete_credential());
        Self::clear_files(dir);
    }

    fn clear_files(dir: &Path) {
        for name in [
            FALLBACK_FILE.to_string(),
            LEGACY_SQLITE.to_string(),
            format!("{LEGACY_SQLITE}-wal"),
            format!("{LEGACY_SQLITE}-shm"),
        ] {
            let _ = std::fs::remove_file(dir.join(name));
        }
    }
}

impl Session for VaultSession {
    type Error = VaultError;

    fn home_dc_id(&self) -> Result<i32, VaultError> {
        self.mem.home_dc_id().map_err(err)
    }

    fn set_home_dc_id(&self, dc_id: i32) -> BoxFuture<'_, Result<(), VaultError>> {
        Box::pin(async move {
            self.mem.set_home_dc_id(dc_id).await.map_err(err)?;
            self.persisted.lock().map_err(err)?.home_dc = Some(dc_id);
            self.save()
        })
    }

    fn dc_option(&self, dc_id: i32) -> Result<Option<DcOption>, VaultError> {
        self.mem.dc_option(dc_id).map_err(err)
    }

    fn set_dc_option(&self, dc_option: &DcOption) -> BoxFuture<'_, Result<(), VaultError>> {
        let dc_option = dc_option.clone();
        Box::pin(async move {
            self.mem.set_dc_option(&dc_option).await.map_err(err)?;
            {
                let mut p = self.persisted.lock().map_err(err)?;
                p.dc_options.retain(|o| o.id != dc_option.id);
                p.dc_options.push(dc_option);
            }
            self.save()
        })
    }

    fn peer(&self, peer: PeerId) -> BoxFuture<'_, Result<Option<PeerInfo>, VaultError>> {
        Box::pin(async move { self.mem.peer(peer).await.map_err(err) })
    }

    fn cache_peer(&self, peer: PeerInfo) -> BoxFuture<'_, Result<(), VaultError>> {
        Box::pin(async move { self.mem.cache_peer(peer).await.map_err(err) })
    }

    fn updates_state(&self) -> BoxFuture<'_, Result<UpdatesState, VaultError>> {
        Box::pin(async move { self.mem.updates_state().await.map_err(err) })
    }

    fn set_update_state(&self, update: UpdateState) -> BoxFuture<'_, Result<(), VaultError>> {
        Box::pin(async move { self.mem.set_update_state(update).await.map_err(err) })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn file_backend_roundtrip_keeps_auth_keys_private() {
        let dir = std::env::temp_dir().join(format!("reader-vault-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join(FALLBACK_FILE);

        let vault = VaultSession::from_persisted(Persisted::default(), Backend::File(path.clone()));
        assert!(!vault.has_auth_key());
        let mut dc = vault.dc_option(2).unwrap().unwrap();
        dc.auth_key = Some([7; 256]);
        vault.set_dc_option(&dc).await.unwrap();
        vault.set_home_dc_id(2).await.unwrap();

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = std::fs::metadata(&path).unwrap().permissions().mode();
            assert_eq!(mode & 0o777, 0o600);
        }
        let raw = std::fs::read_to_string(&path).unwrap();
        let reopened = VaultSession::from_persisted(
            serde_json::from_str(&raw).unwrap(),
            Backend::File(path.clone()),
        );
        assert!(reopened.has_auth_key());
        assert_eq!(reopened.home_dc_id().unwrap(), 2);
        assert_eq!(
            reopened.dc_option(2).unwrap().unwrap().auth_key,
            Some([7; 256])
        );
        // Остальные DC по-прежнему известны (статические адреса).
        assert!(reopened.dc_option(4).unwrap().is_some());

        // Только файлы: настоящую запись в связке ключей тест трогать не должен.
        VaultSession::clear_files(&dir);
        assert!(!path.exists());
        let _ = std::fs::remove_dir_all(&dir);
    }
}

#[cfg(test)]
mod keyring_smoke {
    /// Требует Secret Service; запуск вручную: `cargo test -- --ignored keyring_roundtrip`.
    /// Пишет и сразу удаляет запись под отдельным именем — сессию приложения не трогает.
    #[test]
    #[ignore]
    fn keyring_roundtrip() {
        let e = keyring::Entry::new("com.adaman.reader.selftest", "probe").unwrap();
        e.set_password("ok").unwrap();
        assert_eq!(e.get_password().unwrap(), "ok");
        e.delete_credential().unwrap();
        assert!(matches!(e.get_password(), Err(keyring::Error::NoEntry)));
    }
}
