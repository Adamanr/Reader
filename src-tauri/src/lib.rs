use base64::{engine::general_purpose::STANDARD, Engine as _};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use tauri::AppHandle;
use walkdir::WalkDir;

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Shelf {
    pub id: String,
    pub name: String,
    pub order: i32,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct BookComment {
    pub id: String,
    pub body: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub page: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub chapter_label: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub excerpt: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct SavedQuote {
    pub id: String,
    pub text: String,
    pub created_at: String,
    pub accent: String,
    pub layout: String,
    #[serde(default)]
    pub include_page: bool,
    #[serde(default)]
    pub include_chapter: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub book_title: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub book_author: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bg_image_data_url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bg_image_opacity: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub overlay_color: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub overlay_opacity: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bg_scale: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bg_fit: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct BookMeta {
    pub path: String,
    #[serde(default)]
    pub hidden: bool,
    pub shelf_id: String,
    #[serde(default)]
    pub shelf_ids: Vec<String>,
    pub importance: String,
    pub review: String,
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub author: Option<String>,
    #[serde(default)]
    pub comments: Vec<BookComment>,
    #[serde(default)]
    pub quotes: Vec<SavedQuote>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_read_pdf_page: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_read_pdf_total: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_read_location: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_read_location_label: Option<String>,
    #[serde(default)]
    pub translation_exported: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cover_thumb_data_url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_opened_at: Option<String>,
    /// Путь к файлу стиля `.typ` относительно корня библиотеки; None — общий стиль из настроек.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub typst_style_relative_path: Option<String>,
    /// Поля, которые знает только фронтенд (статус, выделения, прогресс…): сохраняем как есть.
    #[serde(flatten)]
    pub extra: serde_json::Map<String, serde_json::Value>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct LibraryMetadata {
    pub shelves: Vec<Shelf>,
    pub books: HashMap<String, BookMeta>,
    #[serde(flatten)]
    pub extra: serde_json::Map<String, serde_json::Value>,
}

#[derive(Debug, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct AppConfig {
    pub library_root: Option<String>,
    /// Путь к `.typ` теме по умолчанию (относительно папки библиотеки), например `.reader-typst-themes/minimal.typ`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default_typst_style_relative_path: Option<String>,
    /// Хранить метаданные, заметки и обложки в `<библиотека>/.reader/` —
    /// тогда их синхронизирует Syncthing/Nextcloud вместе с книгами.
    #[serde(default)]
    pub sync_in_library: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LibrarySnapshot {
    pub library_root: Option<String>,
    pub book_paths: Vec<String>,
    pub hidden_book_paths: Vec<String>,
    pub metadata: LibraryMetadata,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default_typst_style_relative_path: Option<String>,
    pub sync_in_library: bool,
}

/// Служебная папка внутри библиотеки (не сканируется как книги).
const LIBRARY_DATA_DIR: &str = ".reader";

fn app_dir(_app: &AppHandle) -> PathBuf {
    let dir = dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("com.adaman.reader");
    let _ = std::fs::create_dir_all(&dir);
    dir
}

/// Каталог данных: папка приложения или `<библиотека>/.reader` при синхронизации.
fn data_dir(app: &AppHandle) -> PathBuf {
    if let Ok(c) = load_config(app) {
        if c.sync_in_library {
            if let Some(root) = c.library_root.as_deref() {
                let root = Path::new(root);
                if root.is_dir() {
                    let d = root.join(LIBRARY_DATA_DIR);
                    let _ = std::fs::create_dir_all(&d);
                    return d;
                }
            }
        }
    }
    app_dir(app)
}

fn pdf_translations_dir(app: &AppHandle) -> PathBuf {
    let d = data_dir(app).join("pdf-translations");
    let _ = std::fs::create_dir_all(&d);
    d
}

fn pdf_translation_file(app: &AppHandle, book_relative_path: &str) -> PathBuf {
    let mut h = Sha256::new();
    h.update(book_relative_path.as_bytes());
    let hash = format!("{:x}", h.finalize());
    pdf_translations_dir(app).join(format!("{}.json", hash))
}

fn config_path(app: &AppHandle) -> PathBuf {
    app_dir(app).join("config.json")
}

fn metadata_path(app: &AppHandle) -> PathBuf {
    data_dir(app).join("library-metadata.json")
}

fn covers_dir(app: &AppHandle) -> PathBuf {
    let d = data_dir(app).join("covers");
    let _ = std::fs::create_dir_all(&d);
    d
}

fn store_dir(app: &AppHandle) -> PathBuf {
    let d = data_dir(app).join("store");
    let _ = std::fs::create_dir_all(&d);
    d
}

fn path_hash(s: &str) -> String {
    format!("{:x}", Sha256::digest(s.as_bytes()))
}

fn cover_file(app: &AppHandle, book_relative_path: &str) -> PathBuf {
    covers_dir(app).join(format!("{}.txt", path_hash(book_relative_path)))
}

/// Имя записи хранилища: только `[a-z0-9._-]`, без обхода каталогов.
fn store_file(app: &AppHandle, name: &str) -> Result<PathBuf, String> {
    let ok = !name.is_empty()
        && name.len() <= 120
        && !name.starts_with('.')
        && name
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || matches!(c, '-' | '_' | '.'));
    if !ok {
        return Err("Недопустимое имя записи".into());
    }
    Ok(store_dir(app).join(format!("{name}.json")))
}

fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

fn load_config(app: &AppHandle) -> Result<AppConfig, String> {
    let p = config_path(app);
    if !p.exists() {
        return Ok(AppConfig::default());
    }
    let s = std::fs::read_to_string(&p).map_err(|e| e.to_string())?;
    serde_json::from_str(&s).map_err(|e| e.to_string())
}

fn save_config(app: &AppHandle, c: &AppConfig) -> Result<(), String> {
    let p = config_path(app);
    let s = serde_json::to_string_pretty(c).map_err(|e| e.to_string())?;
    atomic_write(&p, s.as_bytes())
}

fn default_metadata() -> LibraryMetadata {
    LibraryMetadata {
        shelves: vec![Shelf {
            id: "default".into(),
            name: "Общая полка".into(),
            order: 0,
        }],
        books: HashMap::new(),
        extra: Default::default(),
    }
}

fn load_metadata(app: &AppHandle) -> Result<LibraryMetadata, String> {
    let p = metadata_path(app);
    if !p.exists() {
        return Ok(default_metadata());
    }
    let s = std::fs::read_to_string(&p).map_err(|e| e.to_string())?;
    let mut m: LibraryMetadata = serde_json::from_str(&s).map_err(|e| e.to_string())?;
    if m.shelves.is_empty() {
        m.shelves = default_metadata().shelves;
    }
    // Раньше миниатюры обложек лежали прямо в JSON и раздували его до мегабайт.
    // Переносим их в отдельные файлы один раз.
    let mut migrated = false;
    for (path, book) in m.books.iter_mut() {
        if let Some(url) = book.cover_thumb_data_url.take() {
            let _ = atomic_write(&cover_file(app, path), url.as_bytes());
            migrated = true;
        }
    }
    if migrated {
        save_metadata(app, &m)?;
    }
    Ok(m)
}

fn save_metadata(app: &AppHandle, m: &LibraryMetadata) -> Result<(), String> {
    let p = metadata_path(app);
    let s = serde_json::to_string_pretty(m).map_err(|e| e.to_string())?;
    atomic_write(&p, s.as_bytes())
}

/// Безопасный путь под корнем библиотеки. Цель может ещё не существовать (экспорт Typst и т.д.),
/// поэтому нельзя вызывать `canonicalize()` для `joined` — иначе OS error 2.
fn safe_join(root: &Path, rel: &str) -> Result<PathBuf, String> {
    let root = root.canonicalize().map_err(|e| e.to_string())?;
    let rel = rel.trim().trim_start_matches(['/', '\\']);
    let mut out = root.clone();
    for seg in rel
        .split(|c| c == '/' || c == '\\')
        .filter(|s| !s.is_empty())
    {
        match seg {
            "." => {}
            ".." => {
                out.pop();
                if !out.starts_with(&root) {
                    return Err("Недопустимый путь".into());
                }
            }
            s => {
                if s.contains('\0') {
                    return Err("Недопустимый путь".into());
                }
                out.push(s);
            }
        }
    }
    if !out.starts_with(&root) {
        return Err("Недопустимый путь".into());
    }
    Ok(out)
}

/// Запись через временный файл и rename — не остаётся обрубка размером 0 при обрыве записи.
fn atomic_write(path: &Path, bytes: &[u8]) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let parent = path
        .parent()
        .ok_or_else(|| "Некорректный путь".to_string())?;
    let name = path
        .file_name()
        .and_then(|s| s.to_str())
        .ok_or_else(|| "Некорректное имя файла".to_string())?;
    let tmp = parent.join(format!(".{name}.part.{}", std::process::id()));
    std::fs::write(&tmp, bytes).map_err(|e| e.to_string())?;
    #[cfg(windows)]
    if path.exists() {
        std::fs::remove_file(path).map_err(|e| e.to_string())?;
    }
    std::fs::rename(&tmp, path).map_err(|e| {
        let _ = std::fs::remove_file(&tmp);
        e.to_string()
    })?;
    Ok(())
}

fn scan_library(root: &Path) -> Result<Vec<String>, String> {
    if !root.exists() {
        return Err("Папка библиотеки не найдена".into());
    }
    let root = root.canonicalize().map_err(|e| e.to_string())?;
    let mut out = Vec::new();
    let walker = WalkDir::new(&root)
        .into_iter()
        .filter_entry(|e| e.depth() == 0 || e.file_name() != LIBRARY_DATA_DIR);
    for entry in walker.filter_map(|e| e.ok()) {
        let p = entry.path();
        if !p.is_file() {
            continue;
        }
        let Some(ext) = p.extension().and_then(|s| s.to_str()) else {
            continue;
        };
        match ext.to_lowercase().as_str() {
            "pdf" | "epub" | "fb2" | "typ" => {
                let rel = p.strip_prefix(&root).map_err(|e| e.to_string())?;
                out.push(rel.to_string_lossy().replace('\\', "/"));
            }
            _ => {}
        }
    }
    out.sort();
    Ok(out)
}

fn merge_scan_into_metadata(paths: &[String], meta: &mut LibraryMetadata) -> bool {
    let mut changed = false;
    for p in paths {
        if !meta.books.contains_key(p) {
            meta.books.insert(
                p.clone(),
                BookMeta {
                    path: p.clone(),
                    hidden: false,
                    shelf_id: "default".into(),
                    shelf_ids: vec![],
                    importance: "normal".into(),
                    review: String::new(),
                    title: None,
                    author: None,
                    comments: vec![],
                    quotes: vec![],
                    last_read_pdf_page: None,
                    last_read_pdf_total: None,
                    last_read_location: None,
                    last_read_location_label: None,
                    translation_exported: false,
                    cover_thumb_data_url: None,
                    last_opened_at: None,
                    typst_style_relative_path: None,
                    extra: {
                        let mut extra = serde_json::Map::new();
                        extra.insert("addedAtMs".into(), serde_json::json!(now_ms()));
                        extra
                    },
                },
            );
            changed = true;
        }
    }
    let set: HashSet<_> = paths.iter().cloned().collect();
    let previous_len = meta.books.len();
    meta.books.retain(|k, _| set.contains(k));
    changed || meta.books.len() != previous_len
}

#[tauri::command]
fn get_library_snapshot(app: AppHandle) -> Result<LibrarySnapshot, String> {
    let config = load_config(&app)?;
    let root_str = config.library_root.clone();
    let scan_result = root_str.as_ref().map(|r| scan_library(Path::new(r)));
    let paths = match scan_result {
        None => vec![],
        Some(Ok(p)) => p,
        Some(Err(_e)) => {
            let metadata = load_metadata(&app)?;
            let default_typst_style_relative_path = load_config(&app)
                .ok()
                .and_then(|c| c.default_typst_style_relative_path);
            return Ok(LibrarySnapshot {
                library_root: root_str,
                book_paths: vec![],
                hidden_book_paths: vec![],
                metadata,
                default_typst_style_relative_path,
                sync_in_library: config.sync_in_library,
            });
        }
    };
    let mut metadata = load_metadata(&app)?;
    if merge_scan_into_metadata(&paths, &mut metadata) {
        save_metadata(&app, &metadata)?;
    }
    let (hidden_book_paths, book_paths): (Vec<_>, Vec<_>) = paths
        .into_iter()
        .partition(|path| metadata.books.get(path).is_some_and(|book| book.hidden));
    let default_typst_style_relative_path = load_config(&app)
        .ok()
        .and_then(|c| c.default_typst_style_relative_path);
    Ok(LibrarySnapshot {
        library_root: root_str,
        book_paths,
        hidden_book_paths,
        metadata,
        default_typst_style_relative_path,
        sync_in_library: config.sync_in_library,
    })
}

#[tauri::command]
fn set_library_root(app: AppHandle, path: String) -> Result<(), String> {
    let p = PathBuf::from(&path);
    if !p.is_dir() {
        return Err("Укажите существующую папку".into());
    }
    let canon = p.canonicalize().map_err(|e| e.to_string())?;
    let mut c = load_config(&app)?;
    c.library_root = Some(canon.to_string_lossy().to_string());
    save_config(&app, &c)
}

#[tauri::command]
fn save_library_metadata(app: AppHandle, metadata: LibraryMetadata) -> Result<(), String> {
    save_metadata(&app, &metadata)
}

#[tauri::command]
fn delete_library_book(app: AppHandle, relative_path: String) -> Result<(), String> {
    let config = load_config(&app)?;
    let root = config
        .library_root
        .as_deref()
        .ok_or_else(|| "Папка библиотеки не выбрана.".to_string())?;
    let root = Path::new(root)
        .canonicalize()
        .map_err(|_| "Папка библиотеки недоступна.".to_string())?;
    let joined = safe_join(&root, &relative_path)?;
    let target = joined
        .canonicalize()
        .map_err(|_| "Файл книги уже отсутствует.".to_string())?;

    if !target.starts_with(&root) || !target.is_file() {
        return Err("Книга находится вне папки библиотеки.".into());
    }
    let supported = target
        .extension()
        .and_then(|value| value.to_str())
        .is_some_and(|ext| {
            matches!(
                ext.to_ascii_lowercase().as_str(),
                "pdf" | "epub" | "fb2" | "typ"
            )
        });
    if !supported {
        return Err("Этот файл нельзя удалить из Reader.".into());
    }

    std::fs::remove_file(&target)
        .map_err(|_| "Не удалось удалить файл. Проверьте права доступа.".to_string())?;

    // Файл уже удалён: очистка связанных данных выполняется best effort и не
    // превращает успешное удаление в ошибку из-за вторичного шага.
    if let Ok(mut metadata) = load_metadata(&app) {
        metadata.books.remove(&relative_path);
        let _ = save_metadata(&app, &metadata);
    }
    let translation = pdf_translation_file(&app, &relative_path);
    if translation.exists() {
        let _ = std::fs::remove_file(translation);
    }
    let _ = std::fs::remove_file(cover_file(&app, &relative_path));
    Ok(())
}

#[tauri::command]
fn cover_get(app: AppHandle, book_relative_path: String) -> Result<Option<String>, String> {
    let p = cover_file(&app, &book_relative_path);
    if !p.exists() {
        return Ok(None);
    }
    std::fs::read_to_string(&p)
        .map(Some)
        .map_err(|e| e.to_string())
}

#[tauri::command]
fn cover_set(app: AppHandle, book_relative_path: String, data_url: String) -> Result<(), String> {
    if !data_url.starts_with("data:image/") || data_url.len() > 400_000 {
        return Err("Некорректная обложка".into());
    }
    atomic_write(&cover_file(&app, &book_relative_path), data_url.as_bytes())
}

#[tauri::command]
fn store_read(app: AppHandle, name: String) -> Result<Option<String>, String> {
    let p = store_file(&app, &name)?;
    if !p.exists() {
        return Ok(None);
    }
    std::fs::read_to_string(&p)
        .map(Some)
        .map_err(|e| e.to_string())
}

#[tauri::command]
fn store_write(app: AppHandle, name: String, json: String) -> Result<(), String> {
    let p = store_file(&app, &name)?;
    atomic_write(&p, json.as_bytes())
}

/// Копирует файлы с диска (перетаскивание в окно) в корень библиотеки.
/// Возвращает относительные пути добавленных книг.
#[tauri::command]
fn import_books(app: AppHandle, paths: Vec<String>) -> Result<Vec<String>, String> {
    let config = load_config(&app)?;
    let root = config
        .library_root
        .ok_or("Сначала выберите папку библиотеки")?;
    let root = PathBuf::from(root)
        .canonicalize()
        .map_err(|e| e.to_string())?;
    let mut added = Vec::new();
    for src in paths {
        let src = PathBuf::from(src);
        if !src.is_file() {
            continue;
        }
        let Some(ext) = src
            .extension()
            .and_then(|e| e.to_str())
            .map(str::to_lowercase)
        else {
            continue;
        };
        if !matches!(ext.as_str(), "pdf" | "epub" | "fb2") {
            continue;
        }
        let stem = src
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("book")
            .to_string();
        if let Ok(canon) = src.canonicalize() {
            if canon.starts_with(&root) {
                // Уже внутри библиотеки — просто покажем её.
                if let Ok(rel) = canon.strip_prefix(&root) {
                    added.push(rel.to_string_lossy().replace('\\', "/"));
                }
                continue;
            }
        }
        let mut dest = root.join(format!("{stem}.{ext}"));
        let mut n = 2;
        while dest.exists() {
            dest = root.join(format!("{stem} ({n}).{ext}"));
            n += 1;
        }
        std::fs::copy(&src, &dest).map_err(|e| format!("Копирование: {e}"))?;
        if let Ok(rel) = dest.strip_prefix(&root) {
            added.push(rel.to_string_lossy().replace('\\', "/"));
        }
    }
    Ok(added)
}

/// Включает/выключает хранение данных в папке библиотеки.
/// При включении копирует текущие данные туда, если там ещё ничего нет;
/// если данные уже есть (пришли с другого устройства) — использует их.
#[tauri::command]
fn set_sync_in_library(app: AppHandle, enabled: bool) -> Result<(), String> {
    let mut c = load_config(&app)?;
    if c.sync_in_library == enabled {
        return Ok(());
    }
    let root = c
        .library_root
        .clone()
        .ok_or("Сначала выберите папку библиотеки")?;
    let lib_dir = Path::new(&root).join(LIBRARY_DATA_DIR);
    let local_dir = app_dir(&app);
    let (from, to) = if enabled {
        (local_dir, lib_dir)
    } else {
        (lib_dir, local_dir)
    };
    std::fs::create_dir_all(&to).map_err(|e| e.to_string())?;
    if !to.join("library-metadata.json").exists() {
        copy_data_tree(&from, &to)?;
    }
    c.sync_in_library = enabled;
    save_config(&app, &c)
}

fn copy_data_tree(from: &Path, to: &Path) -> Result<(), String> {
    for name in ["library-metadata.json"] {
        let src = from.join(name);
        if src.is_file() {
            std::fs::copy(&src, to.join(name)).map_err(|e| e.to_string())?;
        }
    }
    for dir in ["covers", "store", "pdf-translations"] {
        let src = from.join(dir);
        if !src.is_dir() {
            continue;
        }
        let dst = to.join(dir);
        std::fs::create_dir_all(&dst).map_err(|e| e.to_string())?;
        for entry in std::fs::read_dir(&src)
            .map_err(|e| e.to_string())?
            .flatten()
        {
            let p = entry.path();
            if p.is_file() {
                let target = dst.join(entry.file_name());
                if !target.exists() {
                    std::fs::copy(&p, target).map_err(|e| e.to_string())?;
                }
            }
        }
    }
    Ok(())
}

#[derive(Debug, Deserialize)]
struct LibreTranslateOk {
    #[serde(rename = "translatedText")]
    translated_text: String,
}

#[tauri::command]
async fn translate_texts(
    texts: Vec<String>,
    source: String,
    target: String,
    api_base: String,
    api_key: Option<String>,
) -> Result<Vec<String>, String> {
    let base = api_base.trim().trim_end_matches('/').to_string();
    if base.is_empty() {
        return Err("Укажите URL сервера перевода (LibreTranslate) в настройках.".into());
    }
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(120))
        .build()
        .map_err(|e| e.to_string())?;

    let url = format!("{}/translate", base);
    let mut out: Vec<String> = Vec::with_capacity(texts.len());

    for t in texts {
        let mut body = serde_json::json!({
            "q": t,
            "source": source,
            "target": target,
            "format": "text",
        });
        if let Some(ref k) = api_key {
            let ks = k.trim();
            if !ks.is_empty() {
                body["api_key"] = serde_json::json!(ks);
            }
        }

        let resp = client
            .post(&url)
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await
            .map_err(|e| format!("Запрос перевода: {}", e))?;

        let status = resp.status();
        let raw = resp.text().await.map_err(|e| e.to_string())?;
        if !status.is_success() {
            let short: String = raw.chars().take(280).collect();
            return Err(format!("Перевод: HTTP {} — {}", status, short));
        }

        let parsed: LibreTranslateOk = serde_json::from_str(&raw).map_err(|e| {
            format!(
                "Разбор ответа ({}): {}",
                e,
                raw.chars().take(160).collect::<String>()
            )
        })?;
        out.push(parsed.translated_text);
    }

    Ok(out)
}

#[tauri::command]
fn read_book_base64(app: AppHandle, relative_path: String) -> Result<String, String> {
    let config = load_config(&app)?;
    let root = config
        .library_root
        .ok_or("Сначала выберите папку библиотеки")?;
    let root = PathBuf::from(root);
    let full = safe_join(&root, &relative_path)?;
    let bytes = std::fs::read(&full).map_err(|e| e.to_string())?;
    if bytes.is_empty() {
        return Err(
            "Файл книги пуст (0 байт). Проверьте файл в папке библиотеки или переимпортируйте книгу."
                .into(),
        );
    }
    Ok(STANDARD.encode(&bytes))
}

/// Сохраняет PDF перевода в той же папке, что и исходник: `translate_<имя>.pdf`, помечает оригинал.
#[tauri::command]
fn save_translated_pdf_next_to_source(
    app: AppHandle,
    source_relative_path: String,
    pdf_base64: String,
) -> Result<String, String> {
    let bytes = STANDARD
        .decode(pdf_base64.trim())
        .map_err(|e| e.to_string())?;
    let config = load_config(&app)?;
    let root = config
        .library_root
        .ok_or("Сначала выберите папку библиотеки")?;
    let root = PathBuf::from(root);
    let root = root.canonicalize().map_err(|e| e.to_string())?;
    let source_full = safe_join(&root, &source_relative_path)?;
    let parent = source_full
        .parent()
        .ok_or("Некорректный путь к книге")?
        .to_path_buf();
    let stem = source_full
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("book");
    let new_name = format!("translate_{}.pdf", stem);
    let dest_full = parent.join(&new_name);
    atomic_write(&dest_full, &bytes)?;
    let new_rel = dest_full
        .strip_prefix(&root)
        .map_err(|_| String::from("Новый файл вне папки библиотеки"))?
        .to_string_lossy()
        .replace('\\', "/");
    let mut meta = load_metadata(&app)?;
    if let Some(b) = meta.books.get_mut(&source_relative_path) {
        b.translation_exported = true;
    }
    save_metadata(&app, &meta)?;
    Ok(new_rel)
}

/// Запись произвольного файла (путь из диалога сохранения).
#[tauri::command]
fn write_file_base64(path: String, contents_base64: String) -> Result<(), String> {
    let bytes = STANDARD
        .decode(contents_base64.trim())
        .map_err(|e| e.to_string())?;
    let p = PathBuf::from(path);
    atomic_write(&p, &bytes)
}

#[tauri::command]
fn pdf_translation_load(
    app: AppHandle,
    book_relative_path: String,
) -> Result<Option<String>, String> {
    let p = pdf_translation_file(&app, &book_relative_path);
    if !p.exists() {
        return Ok(None);
    }
    let s = std::fs::read_to_string(&p).map_err(|e| e.to_string())?;
    Ok(Some(s))
}

#[tauri::command]
fn pdf_translation_save(
    app: AppHandle,
    book_relative_path: String,
    json: String,
) -> Result<(), String> {
    let p = pdf_translation_file(&app, &book_relative_path);
    if let Some(parent) = p.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    std::fs::write(&p, json).map_err(|e| e.to_string())
}

#[tauri::command]
fn pdf_translation_delete(app: AppHandle, book_relative_path: String) -> Result<(), String> {
    let p = pdf_translation_file(&app, &book_relative_path);
    if p.exists() {
        std::fs::remove_file(&p).map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[tauri::command]
fn read_library_utf8(app: AppHandle, relative_path: String) -> Result<String, String> {
    let config = load_config(&app)?;
    let root = config
        .library_root
        .ok_or("Сначала выберите папку библиотеки")?;
    let root = PathBuf::from(root);
    let full = safe_join(&root, &relative_path)?;
    std::fs::read_to_string(&full).map_err(|e| e.to_string())
}

#[tauri::command]
fn write_library_utf8(
    app: AppHandle,
    relative_path: String,
    content: String,
) -> Result<(), String> {
    let config = load_config(&app)?;
    let root = config
        .library_root
        .ok_or("Сначала выберите папку библиотеки")?;
    let root = PathBuf::from(root);
    let full = safe_join(&root, &relative_path)?;
    atomic_write(&full, content.as_bytes())
}

#[tauri::command]
fn write_library_base64(
    app: AppHandle,
    relative_path: String,
    contents_base64: String,
) -> Result<(), String> {
    let bytes = STANDARD
        .decode(contents_base64.trim())
        .map_err(|e| e.to_string())?;
    let config = load_config(&app)?;
    let root = config
        .library_root
        .ok_or("Сначала выберите папку библиотеки")?;
    let root = PathBuf::from(root);
    let full = safe_join(&root, &relative_path)?;
    atomic_write(&full, &bytes)
}

#[tauri::command]
fn set_default_typst_style(app: AppHandle, relative_path: Option<String>) -> Result<(), String> {
    let mut c = load_config(&app)?;
    c.default_typst_style_relative_path = relative_path;
    save_config(&app, &c)
}

#[tauri::command]
fn list_typst_theme_files(app: AppHandle) -> Result<Vec<String>, String> {
    let config = load_config(&app)?;
    let root = config
        .library_root
        .ok_or("Сначала выберите папку библиотеки")?;
    let root = PathBuf::from(root);
    let themes_dir = root.join(".reader-typst-themes");
    if !themes_dir.is_dir() {
        return Ok(vec![]);
    }
    let mut names: Vec<String> = std::fs::read_dir(&themes_dir)
        .map_err(|e| e.to_string())?
        .filter_map(|e| e.ok())
        .filter(|e| e.path().is_file())
        .filter_map(|e| {
            e.path()
                .extension()
                .and_then(|x| x.to_str())
                .filter(|x| x.eq_ignore_ascii_case("typ"))
                .and_then(|_| e.file_name().into_string().ok())
        })
        .collect();
    names.sort();
    Ok(names)
}

/// Собирает пути `reader-typst-{hash}-N.svg` в temp, отсортированные по N.
fn typst_svg_page_files(
    temp_dir: &Path,
    hash_hex: &str,
) -> Result<Vec<std::path::PathBuf>, String> {
    let prefix = format!("reader-typst-{}-", hash_hex);
    let mut pages: Vec<(u32, std::path::PathBuf)> = Vec::new();
    let read = std::fs::read_dir(temp_dir).map_err(|e| e.to_string())?;
    for entry in read.filter_map(|e| e.ok()) {
        let name = entry.file_name().to_string_lossy().to_string();
        if !name.starts_with(&prefix) || !name.ends_with(".svg") {
            continue;
        }
        let rest = name
            .strip_prefix(&prefix)
            .and_then(|s| s.strip_suffix(".svg"));
        if let Some(num_str) = rest {
            if let Ok(n) = num_str.parse::<u32>() {
                pages.push((n, entry.path()));
            }
        }
    }
    pages.sort_by_key(|(n, _)| *n);
    Ok(pages.into_iter().map(|(_, p)| p).collect())
}

/// Удаляет старые SVG превью для того же ключа (перед перекомпиляцией).
fn remove_old_typst_preview_svgs(temp_dir: &Path, hash_hex: &str) {
    let prefix = format!("reader-typst-{}-", hash_hex);
    if let Ok(read) = std::fs::read_dir(temp_dir) {
        for entry in read.filter_map(|e| e.ok()) {
            let name = entry.file_name().to_string_lossy().to_string();
            if name.starts_with(&prefix) && name.ends_with(".svg") {
                let _ = std::fs::remove_file(entry.path());
            }
        }
    }
}

/// Компиляция `main_relative` в SVG через CLI `typst`.
/// Несколько страниц: в Typst 0.14+ нужен шаблон пути с `{p}`, иначе ошибка экспорта.
/// `pages`: например `"1-18"` для быстрого предпросмотра (`typst compile --pages …`).
#[tauri::command]
fn compile_typst_to_svg(
    app: AppHandle,
    main_relative: String,
    pages: Option<String>,
) -> Result<String, String> {
    let config = load_config(&app)?;
    let root = config
        .library_root
        .ok_or("Сначала выберите папку библиотеки")?;
    let lib_root = PathBuf::from(root);
    let main_full = safe_join(&lib_root, &main_relative)?;
    let work_dir = main_full
        .parent()
        .ok_or("Некорректный путь к файлу")?
        .to_path_buf();
    let file_name = main_full
        .file_name()
        .ok_or("Некорректное имя файла")?
        .to_str()
        .ok_or("Некорректное имя файла")?;

    let hash_hex = format!("{:x}", Sha256::digest(main_relative.as_bytes()));
    let temp_dir = std::env::temp_dir();
    remove_old_typst_preview_svgs(&temp_dir, &hash_hex);

    let pattern_path = temp_dir.join(format!("reader-typst-{}-{{p}}.svg", hash_hex));
    let pattern_for_typst = pattern_path.to_string_lossy().replace('\\', "/");

    let mut cmd = std::process::Command::new("typst");
    cmd.current_dir(&work_dir)
        .arg("compile")
        .arg("--format")
        .arg("svg");
    if let Some(ref p) = pages {
        let pt = p.trim();
        if !pt.is_empty() {
            cmd.arg("--pages").arg(pt);
        }
    }
    cmd.arg(file_name).arg(&pattern_for_typst);

    let output = cmd.output().map_err(|e| {
        format!(
            "Не удалось запустить `typst` (установите Typst и добавьте в PATH): {}",
            e
        )
    })?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
        let detail = if !stderr.is_empty() {
            stderr
        } else if !stdout.is_empty() {
            stdout
        } else {
            format!("код {:?}", output.status.code())
        };
        return Err(format!("Typst: {}", detail));
    }

    let pages = typst_svg_page_files(&temp_dir, &hash_hex)?;
    if pages.is_empty() {
        return Err(
            "Typst не создал SVG (пустой вывод). Проверьте путь к файлу и версию typst.".into(),
        );
    }

    let mut combined = String::from(r#"<div class="reader-typst-svg-pages">"#);
    for path in pages {
        let svg = std::fs::read_to_string(&path).map_err(|e| e.to_string())?;
        combined.push_str(r#"<div class="reader-typst-svg-page">"#);
        combined.push_str(&svg);
        combined.push_str("</div>");
    }
    combined.push_str("</div>");
    Ok(combined)
}

#[tauri::command]
fn typst_cli_version() -> Result<Option<String>, String> {
    let out = match std::process::Command::new("typst")
        .arg("--version")
        .output()
    {
        Ok(o) => o,
        Err(_) => return Ok(None),
    };
    if !out.status.success() {
        return Ok(None);
    }
    let s = String::from_utf8_lossy(&out.stdout).trim().to_string();
    Ok(if s.is_empty() { None } else { Some(s) })
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LlmMessage {
    pub role: String,
    pub content: String,
}

/// Как TranslateBooksWithLLMs OpenAI: для локального OpenAI-совместимого API отключаем thinking.
fn llm_local_disable_thinking(base: &str) -> bool {
    let b = base.to_lowercase();
    let local = b.contains("127.0.0.1") || b.contains("localhost");
    let official = b.contains("api.openai.com");
    local && !official
}

#[derive(Debug, Deserialize)]
struct OpenAiChatResponse {
    choices: Vec<OpenAiChatChoice>,
}

#[derive(Debug, Deserialize)]
struct OpenAiChatChoice {
    message: OpenAiChatMsgBody,
}

#[derive(Debug, Deserialize)]
struct OpenAiChatMsgBody {
    content: Option<String>,
}

#[derive(Debug, Deserialize)]
struct OpenAiModelList {
    data: Vec<OpenAiModelEntry>,
}

#[derive(Debug, Deserialize)]
struct OpenAiModelEntry {
    id: String,
}

/// OpenAI-совместимый чат (LM Studio, Ollama `/v1`).
#[tauri::command]
async fn llm_chat_completion(
    base_url: String,
    model: String,
    messages: Vec<LlmMessage>,
    temperature: f64,
    max_tokens: Option<u32>,
) -> Result<String, String> {
    let base = base_url.trim().trim_end_matches('/');
    if base.is_empty() {
        return Err("Укажите URL API (например http://127.0.0.1:1234/v1)".into());
    }
    let url = format!("{}/chat/completions", base);
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(900))
        .build()
        .map_err(|e| e.to_string())?;

    let mut payload = serde_json::json!({
        "model": model,
        "messages": serde_json::to_value(&messages).map_err(|e| e.to_string())?,
        "temperature": temperature,
        "stream": false,
    });
    if let Some(mt) = max_tokens {
        if let Some(obj) = payload.as_object_mut() {
            obj.insert("max_tokens".into(), serde_json::json!(mt));
        }
    }
    if llm_local_disable_thinking(&base) {
        if let Some(obj) = payload.as_object_mut() {
            obj.insert("thinking".into(), serde_json::json!(false));
            obj.insert("enable_thinking".into(), serde_json::json!(false));
            obj.insert(
                "chat_template_kwargs".into(),
                serde_json::json!({ "enable_thinking": false }),
            );
        }
    }

    let resp = client
        .post(&url)
        .header("Content-Type", "application/json")
        .json(&payload)
        .send()
        .await
        .map_err(|e| format!("LLM: {}", e))?;

    let status = resp.status();
    let raw = resp.text().await.map_err(|e| e.to_string())?;
    if !status.is_success() {
        let short: String = raw.chars().take(400).collect();
        return Err(format!("LLM: HTTP {} — {}", status, short));
    }

    let parsed: OpenAiChatResponse = serde_json::from_str(&raw).map_err(|e| {
        format!(
            "LLM JSON ({}): {}",
            e,
            raw.chars().take(200).collect::<String>()
        )
    })?;

    let content = parsed
        .choices
        .first()
        .and_then(|c| c.message.content.clone())
        .unwrap_or_default();

    Ok(content)
}

/// GET `{base_url}/models`.
#[tauri::command]
async fn llm_list_models(base_url: String) -> Result<Vec<String>, String> {
    let base = base_url.trim().trim_end_matches('/');
    if base.is_empty() {
        return Err("Укажите URL API".into());
    }
    let url = format!("{}/models", base);
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(60))
        .build()
        .map_err(|e| e.to_string())?;

    let resp = client
        .get(&url)
        .send()
        .await
        .map_err(|e| format!("Модели: {}", e))?;

    let status = resp.status();
    let raw = resp.text().await.map_err(|e| e.to_string())?;
    if !status.is_success() {
        let short: String = raw.chars().take(400).collect();
        return Err(format!("Модели: HTTP {} — {}", status, short));
    }

    let parsed: OpenAiModelList = serde_json::from_str(&raw).map_err(|e| {
        format!(
            "Список моделей ({}): {}",
            e,
            raw.chars().take(200).collect::<String>()
        )
    })?;

    let mut ids: Vec<String> = parsed.data.into_iter().map(|m| m.id).collect();
    ids.sort();
    ids.dedup();
    Ok(ids)
}

#[derive(Debug, Deserialize)]
struct OpenAiEmbeddingResponse {
    data: Vec<OpenAiEmbedding>,
}

#[derive(Debug, Deserialize)]
struct OpenAiEmbedding {
    embedding: Vec<f32>,
    #[serde(default)]
    index: usize,
}

/// OpenAI-совместимые эмбеддинги (`/embeddings`) — для «созвездия цитат».
#[tauri::command]
async fn llm_embeddings(
    base_url: String,
    model: String,
    inputs: Vec<String>,
) -> Result<Vec<Vec<f32>>, String> {
    let base = base_url.trim().trim_end_matches('/');
    if base.is_empty() {
        return Err("Укажите URL API".into());
    }
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(300))
        .build()
        .map_err(|e| e.to_string())?;
    let resp = client
        .post(format!("{base}/embeddings"))
        .json(&serde_json::json!({ "model": model, "input": inputs }))
        .send()
        .await
        .map_err(|e| format!("Эмбеддинги: {e}"))?;
    let status = resp.status();
    let raw = resp.text().await.map_err(|e| e.to_string())?;
    if !status.is_success() {
        let short: String = raw.chars().take(400).collect();
        return Err(format!("Эмбеддинги: HTTP {status} — {short}"));
    }
    let mut parsed: OpenAiEmbeddingResponse =
        serde_json::from_str(&raw).map_err(|e| format!("Эмбеддинги JSON: {e}"))?;
    parsed.data.sort_by_key(|d| d.index);
    Ok(parsed.data.into_iter().map(|d| d.embedding).collect())
}

fn command_exists(name: &str) -> bool {
    std::process::Command::new(name)
        .arg("--help")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .is_ok()
}

/// Доступные локальные движки озвучки.
#[tauri::command]
fn tts_engines() -> Vec<String> {
    let mut out = Vec::new();
    if command_exists("piper") {
        out.push("piper".to_string());
    }
    if command_exists("espeak-ng") {
        out.push("espeak-ng".to_string());
    }
    out
}

/// Озвучка фрагмента локальным движком: WAV в base64.
/// `voice` — путь к модели `.onnx` для Piper или имя голоса espeak-ng.
#[tauri::command]
async fn tts_synthesize(
    engine: String,
    text: String,
    voice: Option<String>,
    rate: f32,
) -> Result<String, String> {
    use std::io::Write;
    let text = text.trim().to_string();
    if text.is_empty() {
        return Err("Пустой текст".into());
    }
    tauri::async_runtime::spawn_blocking(move || {
        let tmp = std::env::temp_dir().join(format!(
            "reader-tts-{}-{}.wav",
            std::process::id(),
            now_ms()
        ));
        let rate = rate.clamp(0.5, 3.0);
        let mut cmd = match engine.as_str() {
            "piper" => {
                let model = voice
                    .filter(|v| !v.trim().is_empty())
                    .ok_or("Укажите модель Piper (.onnx) в настройках")?;
                let mut c = std::process::Command::new("piper");
                c.arg("--model")
                    .arg(model.trim())
                    .arg("--output_file")
                    .arg(&tmp)
                    .arg("--length_scale")
                    .arg(format!("{:.2}", 1.0 / rate));
                c
            }
            "espeak-ng" => {
                let mut c = std::process::Command::new("espeak-ng");
                c.arg("-v")
                    .arg(
                        voice
                            .filter(|v| !v.trim().is_empty())
                            .unwrap_or_else(|| "ru".into()),
                    )
                    .arg("-s")
                    .arg(format!("{}", (175.0 * rate) as u32))
                    .arg("--stdin")
                    .arg("-w")
                    .arg(&tmp);
                c
            }
            _ => return Err("Неизвестный движок озвучки".to_string()),
        };
        let mut child = cmd
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .map_err(|e| format!("Не удалось запустить {engine}: {e}"))?;
        if let Some(mut stdin) = child.stdin.take() {
            stdin
                .write_all(text.as_bytes())
                .map_err(|e| e.to_string())?;
        }
        let out = child.wait_with_output().map_err(|e| e.to_string())?;
        if !out.status.success() {
            let err = String::from_utf8_lossy(&out.stderr);
            let _ = std::fs::remove_file(&tmp);
            return Err(format!("{engine}: {}", err.trim()));
        }
        let bytes = std::fs::read(&tmp).map_err(|e| e.to_string())?;
        let _ = std::fs::remove_file(&tmp);
        Ok(STANDARD.encode(bytes))
    })
    .await
    .map_err(|e| e.to_string())?
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![
            get_library_snapshot,
            set_library_root,
            save_library_metadata,
            delete_library_book,
            read_book_base64,
            save_translated_pdf_next_to_source,
            write_file_base64,
            translate_texts,
            pdf_translation_load,
            pdf_translation_save,
            pdf_translation_delete,
            read_library_utf8,
            write_library_utf8,
            write_library_base64,
            set_default_typst_style,
            list_typst_theme_files,
            compile_typst_to_svg,
            typst_cli_version,
            llm_chat_completion,
            llm_list_models,
            cover_get,
            cover_set,
            store_read,
            store_write,
            import_books,
            set_sync_in_library,
            llm_embeddings,
            tts_engines,
            tts_synthesize,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
