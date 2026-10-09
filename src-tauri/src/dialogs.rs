//! Выбор путей на диске — только через нативный диалог на стороне Rust.
//!
//! Интерфейс никогда не передаёт путь сам. Иначе любой скрипт, оказавшийся в webview,
//! мог бы записать файл куда угодно (`~/.bashrc`) или назначить библиотекой домашнюю папку
//! и читать из неё `~/.ssh` через команды чтения книг. Теперь без участия человека,
//! нажавшего «Сохранить»/«Выбрать», ни то ни другое невозможно.

use crate::storage::{atomic_write, load_config, save_config};
use base64::{engine::general_purpose::STANDARD, Engine as _};
use tauri::{AppHandle, Manager};
use tauri_plugin_dialog::{DialogExt, FileDialogBuilder};

fn with_parent(
    app: &AppHandle,
    dialog: FileDialogBuilder<tauri::Wry>,
) -> FileDialogBuilder<tauri::Wry> {
    match app.get_webview_window("main") {
        Some(w) => dialog.set_parent(&w),
        None => dialog,
    }
}

/// Имя файла по умолчанию: без каталогов и запрещённых символов.
fn clean_file_name(name: &str) -> String {
    let base = name.rsplit(['/', '\\']).next().unwrap_or(name);
    let cleaned: String = base
        .chars()
        .map(|c| {
            if c.is_control() || matches!(c, ':' | '*' | '?' | '"' | '<' | '>' | '|') {
                ' '
            } else {
                c
            }
        })
        .collect();
    let cleaned = cleaned.trim().trim_start_matches('.').trim();
    if cleaned.is_empty() {
        "file".into()
    } else {
        cleaned.into()
    }
}

/// Диалог выбора папки библиотеки. `None` — пользователь отменил выбор.
#[tauri::command]
#[specta::specta]
pub async fn choose_library_root(app: AppHandle) -> Result<Option<String>, String> {
    let dialog = with_parent(&app, app.dialog().file().set_title("Папка библиотеки"));
    let picked = tauri::async_runtime::spawn_blocking(move || dialog.blocking_pick_folder())
        .await
        .map_err(|e| e.to_string())?;
    let Some(picked) = picked else {
        return Ok(None);
    };
    let dir = picked.into_path().map_err(|e| e.to_string())?;
    let canon = dir.canonicalize().map_err(|e| e.to_string())?;
    if !canon.is_dir() {
        return Err("Укажите существующую папку".into());
    }
    let root = canon.to_string_lossy().to_string();
    let mut c = load_config(&app)?;
    c.library_root = Some(root.clone());
    save_config(&app, &c)?;
    Ok(Some(root))
}

/// Кнопка «Добавить книги»: выбор файлов и импорт. `None` — диалог отменён.
#[tauri::command]
#[specta::specta]
pub async fn pick_and_import_books(app: AppHandle) -> Result<Option<Vec<String>>, String> {
    let dialog = with_parent(
        &app,
        app.dialog()
            .file()
            .add_filter("Книги", &["pdf", "epub", "fb2", "zip"]),
    );
    let picked = tauri::async_runtime::spawn_blocking(move || dialog.blocking_pick_files())
        .await
        .map_err(|e| e.to_string())?;
    let Some(picked) = picked else {
        return Ok(None);
    };
    let paths: Vec<_> = picked
        .into_iter()
        .filter_map(|p| p.into_path().ok())
        .collect();
    let app2 = app.clone();
    tauri::async_runtime::spawn_blocking(move || crate::library::import_files(&app2, &paths))
        .await
        .map_err(|e| e.to_string())?
        .map(Some)
}

/// «Сохранить как…»: путь выбирает человек в диалоге, интерфейс передаёт только содержимое.
/// Возвращает `false`, если диалог отменён.
#[tauri::command]
#[specta::specta]
pub async fn save_file_dialog(
    app: AppHandle,
    suggested_name: String,
    filter_name: String,
    extensions: Vec<String>,
    contents_base64: String,
) -> Result<bool, String> {
    let bytes = STANDARD
        .decode(contents_base64.trim())
        .map_err(|e| e.to_string())?;
    let extensions: Vec<String> = extensions
        .into_iter()
        .filter(|e| !e.is_empty() && e.len() <= 10 && e.chars().all(|c| c.is_ascii_alphanumeric()))
        .collect();
    let mut dialog = app
        .dialog()
        .file()
        .set_file_name(clean_file_name(&suggested_name));
    if !extensions.is_empty() {
        let refs: Vec<&str> = extensions.iter().map(String::as_str).collect();
        dialog = dialog.add_filter(filter_name, &refs);
    }
    let dialog = with_parent(&app, dialog);
    let picked = tauri::async_runtime::spawn_blocking(move || dialog.blocking_save_file())
        .await
        .map_err(|e| e.to_string())?;
    let Some(picked) = picked else {
        return Ok(false);
    };
    let path = picked.into_path().map_err(|e| e.to_string())?;
    atomic_write(&path, &bytes)?;
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn suggested_names_cannot_escape() {
        assert_eq!(clean_file_name("../../.bashrc"), "bashrc");
        assert_eq!(clean_file_name("C:\\Windows\\x.md"), "x.md");
        assert_eq!(
            clean_file_name("Заметки: «Война и мир».md"),
            "Заметки  «Война и мир».md"
        );
        assert_eq!(clean_file_name(""), "file");
    }
}
