//! Библиотека: сканирование папки, снимок для интерфейса, импорт, чтение и запись файлов книг.

use crate::library_index;
use crate::model::*;
use crate::storage::*;
use base64::{engine::general_purpose::STANDARD, Engine as _};
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use tauri::{AppHandle, Emitter};
use walkdir::WalkDir;

pub(crate) fn scan_library(root: &Path) -> Result<Vec<String>, String> {
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
        let name = p.file_name().and_then(|s| s.to_str()).unwrap_or("");
        if crate::formats::book_kind(name).is_some() {
            let rel = p.strip_prefix(&root).map_err(|e| e.to_string())?;
            out.push(rel.to_string_lossy().replace('\\', "/"));
        }
    }
    out.sort();
    Ok(out)
}

pub(crate) fn new_book_meta(path: &str, now_ms: u64) -> BookMeta {
    BookMeta {
        path: path.to_string(),
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
            extra.insert("addedAtMs".into(), serde_json::json!(now_ms));
            extra
        },
    }
}

/// Как `pathKey()` во фронтенде: первые 12 байт SHA-256 пути в hex.
pub(crate) fn front_path_key(path: &str) -> String {
    Sha256::digest(path.as_bytes())[..12]
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

pub(crate) fn move_if_free(from: &Path, to: &Path) {
    if from.is_file() && !to.exists() {
        let _ = std::fs::rename(from, to);
    }
}

pub(crate) fn edit_store_json(
    app: &AppHandle,
    name: &str,
    edit: impl FnOnce(&mut serde_json::Value) -> bool,
) {
    let Ok(p) = store_file(app, name) else { return };
    let Some(mut v) = std::fs::read_to_string(&p)
        .ok()
        .and_then(|s| serde_json::from_str::<serde_json::Value>(&s).ok())
    else {
        return;
    };
    if edit(&mut v) {
        if let Ok(s) = serde_json::to_string(&v) {
            let _ = atomic_write(&p, s.as_bytes());
        }
    }
}

/// Переносит всё, что привязано к пути книги: обложку, перевод PDF, файлы хранилища
/// (позиция EPUB, глоссарий, пересказы), статистику чтения и словарь.
pub(crate) fn relocate_book_data(app: &AppHandle, from: &str, to: &str) {
    move_if_free(&cover_file(app, from), &cover_file(app, to));
    move_if_free(
        &pdf_translation_file(app, from),
        &pdf_translation_file(app, to),
    );
    let (kf, kt) = (front_path_key(from), front_path_key(to));
    for prefix in ["epubloc", "glossary", "recap"] {
        if let (Ok(a), Ok(b)) = (
            store_file(app, &format!("{prefix}-{kf}")),
            store_file(app, &format!("{prefix}-{kt}")),
        ) {
            move_if_free(&a, &b);
        }
    }
    edit_store_json(app, "reading-stats", |v| rename_in_stats(v, from, to));
    edit_store_json(app, "vocab", |v| rename_in_vocab(v, from, to));
}

/// `reading-stats.json`: ключ `books[путь]` и списки книг по дням.
pub(crate) fn rename_in_stats(v: &mut serde_json::Value, from: &str, to: &str) -> bool {
    let mut changed = false;
    if let Some(books) = v["books"].as_object_mut() {
        if let Some(b) = books.remove(from) {
            books.entry(to.to_string()).or_insert(b);
            changed = true;
        }
    }
    if let Some(days) = v["days"].as_object_mut() {
        for day in days.values_mut() {
            let Some(list) = day["books"].as_array_mut() else {
                continue;
            };
            if list.iter().any(|x| x == from) {
                list.retain(|x| x != from && x != to);
                list.push(to.into());
                changed = true;
            }
        }
    }
    changed
}

/// `vocab.json`: у карточек слов поле `bookPath`.
pub(crate) fn rename_in_vocab(v: &mut serde_json::Value, from: &str, to: &str) -> bool {
    let mut changed = false;
    for card in v.as_array_mut().into_iter().flatten() {
        if card["bookPath"] == from {
            card["bookPath"] = to.into();
            changed = true;
        }
    }
    changed
}

/// Сканирует библиотеку и сверяет метаданные с файлами (см. `library_index`).
pub(crate) fn snapshot_blocking(app: &AppHandle) -> Result<LibrarySnapshot, String> {
    let config = load_config(app)?;
    let default_typst_style_relative_path = config.default_typst_style_relative_path.clone();
    let root = config.library_root.clone();
    let mut snapshot = LibrarySnapshot {
        library_root: root.clone(),
        book_paths: vec![],
        hidden_book_paths: vec![],
        metadata: default_metadata(),
        default_typst_style_relative_path,
        sync_in_library: config.sync_in_library,
        relocated: vec![],
        merged_conflicts: 0,
    };
    let _guard = META_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    snapshot.merged_conflicts = crate::sync_merge::resolve_conflicts(app);
    let mut metadata = load_metadata(app)?;
    let scanned = root
        .as_deref()
        .map(|r| (Path::new(r), scan_library(Path::new(r))));
    // Нет папки или она недоступна — показываем метаданные как есть и ничего не трогаем.
    let Some((root, Ok(paths))) = scanned else {
        snapshot.metadata = metadata;
        return Ok(snapshot);
    };
    let root = root.canonicalize().map_err(|e| e.to_string())?;
    let mut index = load_index(app);
    let r = library_index::reconcile(&root, &paths, &mut metadata, &mut index, now_ms());
    for m in &r.relocations {
        relocate_book_data(app, &m.from, &m.to);
    }
    if r.index_changed {
        save_index(app, &index)?;
    }
    if r.metadata_changed {
        save_metadata(app, &metadata)?;
    }
    let (hidden, visible): (Vec<_>, Vec<_>) = paths
        .into_iter()
        .partition(|path| metadata.books.get(path).is_some_and(|book| book.hidden));
    snapshot.book_paths = visible;
    snapshot.hidden_book_paths = hidden;
    snapshot.metadata = metadata;
    snapshot.relocated = r.relocations;
    Ok(snapshot)
}

#[tauri::command]
pub async fn get_library_snapshot(app: AppHandle) -> Result<LibrarySnapshot, String> {
    tauri::async_runtime::spawn_blocking(move || snapshot_blocking(&app))
        .await
        .map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn save_library_metadata(
    app: AppHandle,
    metadata: LibraryMetadata,
) -> Result<u64, String> {
    let _guard = META_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let disk = load_metadata(&app)?;
    let incoming_rev = revision_of(&metadata);
    // Интерфейс сохраняет копию целиком. Если с тех пор на диске что-то поменялось
    // (сканирование, слияние с другим устройством, другая страница), не перезаписываем,
    // а переносим на свежую версию только сделанные в интерфейсе изменения.
    let merged = if incoming_rev == revision_of(&disk) {
        metadata
    } else if let Some(base) = remembered_revision(incoming_rev) {
        crate::sync_merge::merge3_metadata(&base, &disk, &metadata)
    } else {
        // Базы нет (например, после перезапуска): осторожное объединение без удалений.
        crate::sync_merge::merge_metadata(&metadata, &disk)
    };
    save_metadata_rev(&app, &merged)
}

#[tauri::command]
pub async fn delete_library_book(app: AppHandle, relative_path: String) -> Result<(), String> {
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
        .file_name()
        .and_then(|value| value.to_str())
        .is_some_and(|name| crate::formats::book_kind(name).is_some());
    if !supported {
        return Err("Этот файл нельзя удалить из Reader.".into());
    }

    std::fs::remove_file(&target)
        .map_err(|_| "Не удалось удалить файл. Проверьте права доступа.".to_string())?;

    // Файл уже удалён: очистка связанных данных выполняется best effort и не
    // превращает успешное удаление в ошибку из-за вторичного шага.
    let _guard = META_LOCK.lock().unwrap_or_else(|e| e.into_inner());
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

/// Копирует книги в корень библиотеки и возвращает их относительные пути.
/// Пути приходят только из нативного диалога или события перетаскивания ОС —
/// команды, которая принимала бы путь из интерфейса, нет (см. `dialogs`).
pub(crate) fn import_files(app: &AppHandle, paths: &[PathBuf]) -> Result<Vec<String>, String> {
    let config = load_config(app)?;
    let root = config
        .library_root
        .ok_or("Сначала выберите папку библиотеки")?;
    let root = PathBuf::from(root)
        .canonicalize()
        .map_err(|e| e.to_string())?;
    let mut added = Vec::new();
    for src in paths {
        if !src.is_file() {
            continue;
        }
        let name = src.file_name().and_then(|e| e.to_str()).unwrap_or("");
        if !crate::formats::is_importable(name) {
            continue;
        }
        let Some((stem, ext)) = crate::formats::split_book_name(name) else {
            continue;
        };
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
        std::fs::copy(src, &dest).map_err(|e| format!("Копирование: {e}"))?;
        if let Ok(rel) = dest.strip_prefix(&root) {
            added.push(rel.to_string_lossy().replace('\\', "/"));
        }
    }
    Ok(added)
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct BooksImported {
    added: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
}

/// Файлы, брошенные в окно: событие приходит от ОС прямо в Rust, результат уходит
/// в интерфейс событием `books-imported`.
pub(crate) fn import_dropped(app: &AppHandle, paths: Vec<PathBuf>) {
    let app = app.clone();
    std::thread::spawn(move || {
        let payload = match import_files(&app, &paths) {
            Ok(added) => BooksImported { added, error: None },
            Err(e) => BooksImported {
                added: vec![],
                error: Some(e),
            },
        };
        let _ = app.emit("books-imported", payload);
    });
}

/// Файл книги как двоичный ответ (ArrayBuffer в JS) — без base64 и JSON,
/// что заметно быстрее и не нагружает интерфейс на больших PDF.
#[tauri::command]
pub async fn read_book_bytes(
    app: AppHandle,
    relative_path: String,
) -> Result<tauri::ipc::Response, String> {
    let config = load_config(&app)?;
    let root = config
        .library_root
        .ok_or("Сначала выберите папку библиотеки")?;
    let full = safe_join(&PathBuf::from(root), &relative_path)?;
    let bytes = tauri::async_runtime::spawn_blocking(move || crate::formats::read_book_file(&full))
        .await
        .map_err(|e| e.to_string())?
        .map_err(|e| e.to_string())?;
    if bytes.is_empty() {
        return Err(
            "Файл книги пуст (0 байт). Проверьте файл в папке библиотеки или переимпортируйте книгу."
                .into(),
        );
    }
    Ok(tauri::ipc::Response::new(bytes))
}

#[tauri::command]
pub async fn read_book_base64(app: AppHandle, relative_path: String) -> Result<String, String> {
    let config = load_config(&app)?;
    let root = config
        .library_root
        .ok_or("Сначала выберите папку библиотеки")?;
    let root = PathBuf::from(root);
    let full = safe_join(&root, &relative_path)?;
    let bytes = crate::formats::read_book_file(&full).map_err(|e| e.to_string())?;
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
pub async fn save_translated_pdf_next_to_source(
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
    let _guard = META_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut meta = load_metadata(&app)?;
    if let Some(b) = meta.books.get_mut(&source_relative_path) {
        b.translation_exported = true;
    }
    save_metadata(&app, &meta)?;
    Ok(new_rel)
}

#[tauri::command]
pub async fn pdf_translation_load(
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
pub async fn pdf_translation_save(
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
pub async fn pdf_translation_delete(
    app: AppHandle,
    book_relative_path: String,
) -> Result<(), String> {
    let p = pdf_translation_file(&app, &book_relative_path);
    if p.exists() {
        std::fs::remove_file(&p).map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[tauri::command]
pub async fn read_library_utf8(app: AppHandle, relative_path: String) -> Result<String, String> {
    let config = load_config(&app)?;
    let root = config
        .library_root
        .ok_or("Сначала выберите папку библиотеки")?;
    let root = PathBuf::from(root);
    let full = safe_join(&root, &relative_path)?;
    std::fs::read_to_string(&full).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn write_library_utf8(
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
pub async fn write_library_base64(
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

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn relocation_rewrites_stats_and_vocab() {
        let mut stats = json!({
            "books": { "a.pdf": { "ms": 5 }, "other.pdf": { "ms": 1 } },
            "days": { "2026-10-10": { "ms": 5, "books": ["a.pdf", "other.pdf"] } },
            "speed": {}
        });
        assert!(rename_in_stats(&mut stats, "a.pdf", "x/b.pdf"));
        assert_eq!(stats["books"]["x/b.pdf"]["ms"], 5);
        assert!(stats["books"].get("a.pdf").is_none());
        assert_eq!(
            stats["days"]["2026-10-10"]["books"],
            json!(["other.pdf", "x/b.pdf"])
        );
        assert!(!rename_in_stats(&mut stats, "a.pdf", "x/b.pdf"));

        let mut vocab = json!([{ "word": "w", "bookPath": "a.pdf" }, { "bookPath": "c.pdf" }]);
        assert!(rename_in_vocab(&mut vocab, "a.pdf", "x/b.pdf"));
        assert_eq!(vocab[0]["bookPath"], "x/b.pdf");
        assert_eq!(vocab[1]["bookPath"], "c.pdf");
    }

    #[test]
    fn front_path_key_matches_frontend() {
        // pathKey("a.pdf") во фронтенде: первые 12 байт SHA-256 в hex.
        let full = format!("{:x}", Sha256::digest(b"a.pdf"));
        assert_eq!(front_path_key("a.pdf"), full[..24]);
    }
}
