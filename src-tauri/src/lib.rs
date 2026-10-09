mod backups;
mod dialogs;
mod formats;
mod library;
mod library_index;
mod llm;
mod model;
mod opds;
mod search;
mod storage;
mod sync_merge;
mod telegram;
mod telegram_vault;
mod translate;
mod tts;
mod typst;

use tauri::Manager;

/// Вспомогательные режимы того же исполняемого файла (без окна). `Some(код)` — выйти с ним.
pub fn run_helper_if_requested() -> Option<i32> {
    let args: Vec<std::ffi::OsString> = std::env::args_os().skip(1).collect();
    match args.as_slice() {
        [flag, book, out] if flag == search::EXTRACT_FLAG => Some(search::helper_extract(
            std::path::Path::new(book),
            std::path::Path::new(out),
        )),
        _ => None,
    }
}

/// Команды с типизированными обёртками в `src/lib/bindings.ts` (tauri-specta).
/// Остальные пока вызываются через `invoke` с типами, написанными вручную: метаданные
/// библиотеки содержат поля, известные только фронтенду, а чтение книги отдаёт байты.
const TYPED_COMMANDS: &[&str] = &[
    "choose_library_root",
    "pick_and_import_books",
    "save_file_dialog",
    "metadata_backups_list",
    "metadata_backup_create",
    "metadata_backup_restore",
    "search_reindex",
    "search_status",
    "search_query",
    "opds_fetch",
    "opds_import",
    "telegram_status",
    "telegram_send_code",
    "telegram_sign_in",
    "telegram_logout",
    "telegram_resolve_channel",
    "source_list_files",
    "source_import_files",
    "source_count_new",
];

/// Путь к сгенерированным привязкам относительно `src-tauri`.
const BINDINGS: &str = "../src/lib/bindings.ts";

fn specta_builder() -> tauri_specta::Builder<tauri::Wry> {
    tauri_specta::Builder::<tauri::Wry>::new()
        .commands(tauri_specta::collect_commands![
            dialogs::choose_library_root,
            dialogs::pick_and_import_books,
            dialogs::save_file_dialog,
            backups::metadata_backups_list,
            backups::metadata_backup_create,
            backups::metadata_backup_restore,
            search::search_reindex,
            search::search_status,
            search::search_query,
            opds::opds_fetch,
            opds::opds_import,
            telegram::telegram_status,
            telegram::telegram_send_code,
            telegram::telegram_sign_in,
            telegram::telegram_logout,
            telegram::telegram_resolve_channel,
            telegram::source_list_files,
            telegram::source_import_files,
            telegram::source_count_new,
        ])
        // Полезная нагрузка событий прогресса.
        .typ::<telegram::ImportProgress>()
        .typ::<opds::OpdsProgress>()
        .typ::<search::IndexProgress>()
        // Ошибки — строки, как у остальных команд: обёртки бросают их, а не возвращают Result.
        .error_handling(tauri_specta::ErrorHandlingMode::Throw)
        // Размеры файлов и миллисекунды далеко от 2^53.
        .dangerously_cast_bigints_to_number()
        // Типы ходят в одну сторону (из Rust в интерфейс) — отдельные варианты
        // для сериализации и десериализации только запутывают.
        .disable_serde_phases()
}

fn export_bindings(path: &std::path::Path) -> Result<(), String> {
    specta_builder()
        .export(
            specta_typescript::Typescript::default().header(
                "// Сгенерировано tauri-specta из Rust (src-tauri). Не редактируйте вручную.\n",
            ),
            path,
        )
        .map_err(|e| e.to_string())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // В отладке привязки пересобираются при каждом запуске; в CI их свежесть проверяет тест.
    #[cfg(debug_assertions)]
    if let Err(e) =
        export_bindings(&std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(BINDINGS))
    {
        eprintln!("bindings.ts: {e}");
    }
    let typed = specta_builder().invoke_handler();
    let untyped: Box<dyn Fn(tauri::ipc::Invoke<tauri::Wry>) -> bool + Send + Sync> =
        Box::new(tauri::generate_handler![
            library::get_library_snapshot,
            library::save_library_metadata,
            library::delete_library_book,
            library::read_book_base64,
            library::read_book_bytes,
            library::save_translated_pdf_next_to_source,
            translate::translate_texts,
            library::pdf_translation_load,
            library::pdf_translation_save,
            library::pdf_translation_delete,
            library::read_library_utf8,
            library::write_library_utf8,
            library::write_library_base64,
            typst::set_default_typst_style,
            typst::list_typst_theme_files,
            typst::compile_typst_to_svg,
            typst::typst_cli_version,
            llm::llm_chat_completion,
            llm::llm_list_models,
            storage::cover_get,
            storage::cover_set,
            storage::store_read,
            storage::store_write,
            storage::set_sync_in_library,
            llm::llm_embeddings,
            tts::tts_engines,
            tts::tts_synthesize,
        ]);
    tauri::Builder::default()
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .manage(telegram::TelegramState::default())
        .manage(opds::OpdsState::default())
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::DragDrop(tauri::DragDropEvent::Drop { paths, .. }) = event {
                library::import_dropped(window.app_handle(), paths.clone());
            }
        })
        .invoke_handler(move |invoke| {
            if TYPED_COMMANDS.contains(&invoke.message.command()) {
                typed(invoke)
            } else {
                untyped(invoke)
            }
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Привязки в репозитории должны совпадать с тем, что генерирует Rust.
    /// Обновить: `UPDATE_BINDINGS=1 cargo test bindings_are_up_to_date`.
    #[test]
    fn bindings_are_up_to_date() {
        let committed = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(BINDINGS);
        if std::env::var_os("UPDATE_BINDINGS").is_some() {
            export_bindings(&committed).unwrap();
        }
        let fresh = std::env::temp_dir().join(format!("reader-bindings-{}.ts", std::process::id()));
        export_bindings(&fresh).unwrap();
        let generated = std::fs::read_to_string(&fresh).unwrap();
        let _ = std::fs::remove_file(&fresh);
        let current = std::fs::read_to_string(&committed).unwrap_or_default();
        assert!(
            generated == current,
            "src/lib/bindings.ts устарел: UPDATE_BINDINGS=1 cargo test bindings_are_up_to_date"
        );
        // Каждая команда из списка маршрутизации действительно описана в привязках.
        for name in TYPED_COMMANDS {
            let camel: String = name
                .split('_')
                .enumerate()
                .map(|(i, w)| {
                    if i == 0 {
                        w.to_string()
                    } else {
                        w[..1].to_uppercase() + &w[1..]
                    }
                })
                .collect();
            assert!(
                generated.contains(&format!("{camel}: (")),
                "нет {camel} в bindings.ts"
            );
        }
    }
}
