//! Локальные резервные копии пользовательских данных: метаданные (заметки, цитаты, прогресс),
//! индекс книг и JSON-хранилище (словарь, статистика, источники…).
//!
//! Копии лежат в каталоге приложения, а не в библиотеке: синхронизация не разнесёт по устройствам
//! испорченные данные вместе с их «резервной копией». Каждая копия — папка `backups/<ms>/`,
//! которая появляется атомарно (пишется во временную и переименовывается).

use crate::library_index::INDEX_NAME;
use crate::storage::{app_dir, atomic_write, data_dir, now_ms, METADATA_NAME, META_LOCK};
use serde::Serialize;
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use tauri::AppHandle;

/// Не чаще раза в час при обычном сохранении метаданных.
const MIN_INTERVAL_MS: u64 = 60 * 60 * 1000;
/// Сколько последних копий хранить всегда.
const KEEP_RECENT: usize = 24;
/// Кроме них — по одной копии на день за последний месяц.
const KEEP_DAYS: u64 = 30;
const DAY_MS: u64 = 24 * 60 * 60 * 1000;

#[derive(Debug, Clone, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct BackupInfo {
    pub id: String,
    pub created_at_ms: u64,
    /// Книг в метаданных копии (None — не удалось прочитать).
    pub books: Option<usize>,
    pub quotes: Option<usize>,
}

fn backups_dir(app: &AppHandle) -> PathBuf {
    app_dir(app).join("backups")
}

/// Копии от новых к старым.
fn list(dir: &Path) -> Vec<(u64, PathBuf)> {
    let mut out: Vec<(u64, PathBuf)> = std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|e| {
            let ms = e.file_name().to_str()?.parse::<u64>().ok()?;
            e.path().is_dir().then(|| (ms, e.path()))
        })
        .collect();
    out.sort_by_key(|(ms, _)| std::cmp::Reverse(*ms));
    out
}

fn copy_file(from: &Path, to: &Path) -> Result<(), String> {
    if from.is_file() {
        let bytes = std::fs::read(from).map_err(|e| e.to_string())?;
        atomic_write(to, &bytes)?;
    }
    Ok(())
}

fn json_files(dir: &Path) -> impl Iterator<Item = PathBuf> {
    std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.is_file() && p.extension().is_some_and(|x| x == "json"))
}

/// Копирует текущие данные в новую папку резервной копии. `None` — копировать нечего.
pub fn create(app: &AppHandle) -> Result<Option<String>, String> {
    let src = data_dir(app);
    if !src.join(METADATA_NAME).is_file() {
        return Ok(None);
    }
    let root = backups_dir(app);
    let mut ms = now_ms();
    while root.join(ms.to_string()).exists() {
        ms += 1;
    }
    let id = ms.to_string();
    let tmp = root.join(format!(".{id}.tmp"));
    let _ = std::fs::remove_dir_all(&tmp);
    std::fs::create_dir_all(tmp.join("store")).map_err(|e| e.to_string())?;

    let copied = (|| {
        for name in [METADATA_NAME, INDEX_NAME] {
            copy_file(&src.join(name), &tmp.join(name))?;
        }
        for f in json_files(&src.join("store")) {
            copy_file(
                &f,
                &tmp.join("store").join(f.file_name().unwrap_or_default()),
            )?;
        }
        std::fs::rename(&tmp, root.join(&id)).map_err(|e| e.to_string())
    })();
    if let Err(e) = copied {
        let _ = std::fs::remove_dir_all(&tmp);
        return Err(format!("Резервная копия: {e}"));
    }
    prune(&root, now_ms());
    Ok(Some(id))
}

/// Вызывается перед каждой записью метаданных; сам решает, пора ли делать копию.
pub fn maybe_create(app: &AppHandle) {
    let newest = list(&backups_dir(app)).first().map(|(ms, _)| *ms);
    if newest.is_none_or(|ms| now_ms().saturating_sub(ms) >= MIN_INTERVAL_MS) {
        let _ = create(app);
    }
}

fn prune(root: &Path, now: u64) {
    let mut days = HashSet::new();
    for (i, (ms, path)) in list(root).into_iter().enumerate() {
        let fresh_day = now.saturating_sub(ms) < KEEP_DAYS * DAY_MS && days.insert(ms / DAY_MS);
        if i >= KEEP_RECENT && !fresh_day {
            let _ = std::fs::remove_dir_all(path);
        }
    }
}

fn info(ms: u64, dir: &Path) -> BackupInfo {
    let meta: Option<serde_json::Value> = std::fs::read_to_string(dir.join(METADATA_NAME))
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok());
    let books = meta.as_ref().and_then(|m| m["books"].as_object());
    BackupInfo {
        id: ms.to_string(),
        created_at_ms: ms,
        books: books.map(|b| b.len()),
        quotes: books.map(|b| {
            b.values()
                .filter_map(|book| book["quotes"].as_array())
                .map(Vec::len)
                .sum()
        }),
    }
}

#[tauri::command]
#[specta::specta]
pub async fn metadata_backups_list(app: AppHandle) -> Result<Vec<BackupInfo>, String> {
    Ok(list(&backups_dir(&app))
        .into_iter()
        .map(|(ms, dir)| info(ms, &dir))
        .collect())
}

#[tauri::command]
#[specta::specta]
pub async fn metadata_backup_create(app: AppHandle) -> Result<Option<BackupInfo>, String> {
    let _guard = META_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    Ok(create(&app)?.map(|id| info(id.parse().unwrap_or(0), &backups_dir(&app).join(&id))))
}

/// Возвращает данные из копии. Перед этим сохраняет текущее состояние отдельной копией,
/// так что восстановление можно отменить. После вызова интерфейс нужно перезагрузить.
#[tauri::command]
#[specta::specta]
pub async fn metadata_backup_restore(app: AppHandle, id: String) -> Result<(), String> {
    if id.is_empty() || !id.bytes().all(|b| b.is_ascii_digit()) {
        return Err("Некорректная резервная копия".into());
    }
    let src = backups_dir(&app).join(&id);
    if !src.join(METADATA_NAME).is_file() {
        return Err("Резервная копия не найдена".into());
    }
    let _guard = META_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    create(&app)?;
    let dst = data_dir(&app);
    for name in [METADATA_NAME, INDEX_NAME] {
        copy_file(&src.join(name), &dst.join(name))?;
    }
    for f in json_files(&src.join("store")) {
        copy_file(
            &f,
            &dst.join("store").join(f.file_name().unwrap_or_default()),
        )?;
    }
    crate::storage::forget_revisions();
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prune_keeps_recent_and_one_per_day() {
        let root = std::env::temp_dir().join(format!("reader-backups-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let now = 100 * DAY_MS;
        // 40 копий раз в 6 часов (10 дней) + одна очень старая.
        let mut all: Vec<u64> = (0..40).map(|i| now - i * DAY_MS / 4).collect();
        all.push(now - 60 * DAY_MS);
        for ms in &all {
            std::fs::create_dir_all(root.join(ms.to_string())).unwrap();
        }
        prune(&root, now);
        let left: Vec<u64> = list(&root).into_iter().map(|(ms, _)| ms).collect();
        let _ = std::fs::remove_dir_all(&root);

        // Последние 24 на месте.
        assert_eq!(&left[..KEEP_RECENT], &all[..KEEP_RECENT]);
        // Из более старых осталось не больше одной на день, а копия двухмесячной давности удалена.
        let older = &left[KEEP_RECENT..];
        let days: HashSet<u64> = older.iter().map(|ms| ms / DAY_MS).collect();
        assert_eq!(days.len(), older.len());
        assert!(!left.contains(&(now - 60 * DAY_MS)));
    }
}
