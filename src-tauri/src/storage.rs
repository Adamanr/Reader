//! Где лежат данные и как они пишутся: каталоги, конфиг, метаданные, обложки, JSON-хранилище.

use crate::library_index;
use crate::model::*;
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use tauri::AppHandle;

/// Служебная папка внутри библиотеки (не сканируется как книги).
pub(crate) const LIBRARY_DATA_DIR: &str = ".reader";

pub(crate) fn app_dir(_app: &AppHandle) -> PathBuf {
    let dir = dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("com.adaman.reader");
    let _ = std::fs::create_dir_all(&dir);
    dir
}

/// Каталог данных: папка приложения или `<библиотека>/.reader` при синхронизации.
pub(crate) fn data_dir(app: &AppHandle) -> PathBuf {
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

pub(crate) fn pdf_translations_dir(app: &AppHandle) -> PathBuf {
    let d = data_dir(app).join("pdf-translations");
    let _ = std::fs::create_dir_all(&d);
    d
}

pub(crate) fn pdf_translation_file(app: &AppHandle, book_relative_path: &str) -> PathBuf {
    let mut h = Sha256::new();
    h.update(book_relative_path.as_bytes());
    let hash = format!("{:x}", h.finalize());
    pdf_translations_dir(app).join(format!("{}.json", hash))
}

pub(crate) fn config_path(app: &AppHandle) -> PathBuf {
    app_dir(app).join("config.json")
}

pub(crate) const METADATA_NAME: &str = "library-metadata.json";

/// Сериализует чтение-изменение-запись метаданных: сканирование, сохранение из интерфейса,
/// удаление книги и восстановление из копии не должны затирать друг друга.
pub(crate) static META_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

pub(crate) fn metadata_path(app: &AppHandle) -> PathBuf {
    data_dir(app).join(METADATA_NAME)
}

pub(crate) fn index_path(app: &AppHandle) -> PathBuf {
    data_dir(app).join(library_index::INDEX_NAME)
}

pub(crate) fn load_index(app: &AppHandle) -> library_index::LibraryIndex {
    std::fs::read_to_string(index_path(app))
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

pub(crate) fn save_index(
    app: &AppHandle,
    index: &library_index::LibraryIndex,
) -> Result<(), String> {
    let s = serde_json::to_string(index).map_err(|e| e.to_string())?;
    atomic_write(&index_path(app), s.as_bytes())
}

pub(crate) fn covers_dir(app: &AppHandle) -> PathBuf {
    let d = data_dir(app).join("covers");
    let _ = std::fs::create_dir_all(&d);
    d
}

pub(crate) fn store_dir(app: &AppHandle) -> PathBuf {
    let d = data_dir(app).join("store");
    let _ = std::fs::create_dir_all(&d);
    d
}

pub(crate) fn path_hash(s: &str) -> String {
    format!("{:x}", Sha256::digest(s.as_bytes()))
}

pub(crate) fn cover_file(app: &AppHandle, book_relative_path: &str) -> PathBuf {
    covers_dir(app).join(format!("{}.txt", path_hash(book_relative_path)))
}

/// Имя записи хранилища: только `[a-z0-9._-]`, без обхода каталогов.
pub(crate) fn store_file(app: &AppHandle, name: &str) -> Result<PathBuf, String> {
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

pub(crate) fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

pub(crate) fn load_config(app: &AppHandle) -> Result<AppConfig, String> {
    let p = config_path(app);
    if !p.exists() {
        return Ok(AppConfig::default());
    }
    let s = std::fs::read_to_string(&p).map_err(|e| e.to_string())?;
    serde_json::from_str(&s).map_err(|e| e.to_string())
}

pub(crate) fn save_config(app: &AppHandle, c: &AppConfig) -> Result<(), String> {
    let p = config_path(app);
    let s = serde_json::to_string_pretty(c).map_err(|e| e.to_string())?;
    atomic_write(&p, s.as_bytes())
}

pub(crate) fn default_metadata() -> LibraryMetadata {
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

/// Номер версии метаданных на диске. Интерфейс получает его в снимке и присылает обратно:
/// по нему видно, что сохранение сделано поверх устаревшей копии.
const REVISION: &str = "revision";
const REVISIONS_KEEP: usize = 128;
/// Недавние версии (номер → содержимое) — база для трёхстороннего слияния.
static REVISIONS: std::sync::Mutex<std::collections::VecDeque<(u64, LibraryMetadata)>> =
    std::sync::Mutex::new(std::collections::VecDeque::new());

pub(crate) fn revision_of(m: &LibraryMetadata) -> u64 {
    m.extra.get(REVISION).and_then(|v| v.as_u64()).unwrap_or(0)
}

fn remember_revision(m: &LibraryMetadata) {
    let mut q = REVISIONS.lock().unwrap_or_else(|e| e.into_inner());
    let r = revision_of(m);
    if q.iter().any(|(x, _)| *x == r) {
        return;
    }
    q.push_back((r, m.clone()));
    while q.len() > REVISIONS_KEEP {
        q.pop_front();
    }
}

pub(crate) fn remembered_revision(rev: u64) -> Option<LibraryMetadata> {
    let q = REVISIONS.lock().unwrap_or_else(|e| e.into_inner());
    q.iter().find(|(x, _)| *x == rev).map(|(_, m)| m.clone())
}

/// После восстановления из копии номера версий начинаются заново.
pub(crate) fn forget_revisions() {
    REVISIONS.lock().unwrap_or_else(|e| e.into_inner()).clear();
}

/// Файл метаданных как есть, без миграций.
fn read_metadata_file(app: &AppHandle) -> Result<LibraryMetadata, String> {
    let p = metadata_path(app);
    if !p.exists() {
        return Ok(default_metadata());
    }
    let s = std::fs::read_to_string(&p).map_err(|e| e.to_string())?;
    let m: LibraryMetadata = serde_json::from_str(&s).map_err(|e| e.to_string())?;
    remember_revision(&m);
    Ok(m)
}

pub(crate) fn load_metadata(app: &AppHandle) -> Result<LibraryMetadata, String> {
    let mut m = read_metadata_file(app)?;
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

pub(crate) fn save_metadata(app: &AppHandle, m: &LibraryMetadata) -> Result<(), String> {
    save_metadata_rev(app, m).map(|_| ())
}

/// Записывает метаданные и возвращает новый номер версии.
pub(crate) fn save_metadata_rev(app: &AppHandle, m: &LibraryMetadata) -> Result<u64, String> {
    crate::backups::maybe_create(app);
    // Метки изменений нужны, чтобы при синхронизации слить версии с разных устройств.
    let mut m = m.clone();
    let old = read_metadata_file(app).unwrap_or_else(|_| default_metadata());
    crate::sync_merge::stamp_changes(&old, &mut m, now_ms());
    let rev = revision_of(&old) + 1;
    m.extra.insert(REVISION.into(), rev.into());
    write_metadata(app, &m)?;
    remember_revision(&m);
    Ok(rev)
}

fn write_metadata(app: &AppHandle, m: &LibraryMetadata) -> Result<(), String> {
    let p = metadata_path(app);
    let s = serde_json::to_string_pretty(m).map_err(|e| e.to_string())?;
    atomic_write(&p, s.as_bytes())
}

/// Безопасный путь под корнем библиотеки. Цель может ещё не существовать (экспорт Typst и т.д.),
/// поэтому нельзя вызывать `canonicalize()` для `joined` — иначе OS error 2.
pub(crate) fn safe_join(root: &Path, rel: &str) -> Result<PathBuf, String> {
    let root = root.canonicalize().map_err(|e| e.to_string())?;
    let rel = rel.trim().trim_start_matches(['/', '\\']);
    let mut out = root.clone();
    for seg in rel.split(['/', '\\']).filter(|s| !s.is_empty()) {
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

pub(crate) static WRITE_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
pub(crate) static WRITE_SEQ: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

/// Запись через временный файл и rename — не остаётся обрубка размером 0 при обрыве записи.
/// Команды выполняются параллельно, поэтому записи сериализуем и даём временным файлам уникальные имена.
pub(crate) fn atomic_write(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let _guard = WRITE_LOCK.lock().unwrap_or_else(|e| e.into_inner());
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
    let seq = WRITE_SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let tmp = parent.join(format!(".{name}.part.{}.{seq}", std::process::id()));
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

#[tauri::command]
pub async fn cover_get(
    app: AppHandle,
    book_relative_path: String,
) -> Result<Option<String>, String> {
    let p = cover_file(&app, &book_relative_path);
    if !p.exists() {
        return Ok(None);
    }
    std::fs::read_to_string(&p)
        .map(Some)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn cover_set(
    app: AppHandle,
    book_relative_path: String,
    data_url: String,
) -> Result<(), String> {
    if !data_url.starts_with("data:image/") || data_url.len() > 400_000 {
        return Err("Некорректная обложка".into());
    }
    atomic_write(&cover_file(&app, &book_relative_path), data_url.as_bytes())
}

#[tauri::command]
pub async fn store_read(app: AppHandle, name: String) -> Result<Option<String>, String> {
    let p = store_file(&app, &name)?;
    if !p.exists() {
        return Ok(None);
    }
    std::fs::read_to_string(&p)
        .map(Some)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn store_write(app: AppHandle, name: String, json: String) -> Result<(), String> {
    let p = store_file(&app, &name)?;
    atomic_write(&p, json.as_bytes())
}

/// Включает/выключает хранение данных в папке библиотеки.
/// При включении копирует текущие данные туда, если там ещё ничего нет;
/// если данные уже есть (пришли с другого устройства) — использует их.
#[tauri::command]
pub async fn set_sync_in_library(app: AppHandle, enabled: bool) -> Result<(), String> {
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
    if !to.join(METADATA_NAME).exists() {
        copy_data_tree(&from, &to)?;
    }
    c.sync_in_library = enabled;
    save_config(&app, &c)
}

pub(crate) fn copy_data_tree(from: &Path, to: &Path) -> Result<(), String> {
    for name in [METADATA_NAME, library_index::INDEX_NAME] {
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
