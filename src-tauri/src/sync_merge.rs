//! Объединение данных, разошедшихся между устройствами при синхронизации папки
//! (Syncthing, Nextcloud, Dropbox…).
//!
//! Когда два устройства меняют файл одновременно, синхронизатор не сливает их, а оставляет
//! рядом «конфликтную копию». Раньше её никто не читал, и изменения с одного из устройств
//! пропадали. Теперь при каждом сканировании копии находятся и объединяются:
//! - элементы со своим `id` (цитаты, заметки, выделения, слова) складываются, удалённые
//!   не возвращаются (их `id` запоминаются в `deletedIds`);
//! - для прочих полей книги (прогресс, статус, полка…) побеждает более свежая версия —
//!   у каждой книги есть `updatedAtMs`, его ставит бэкенд при сохранении;
//! - обработанная копия переносится в локальный архив `backups/conflicts`, а не удаляется.

use crate::library_index::{LibraryIndex, INDEX_NAME};
use crate::model::LibraryMetadata;
use crate::storage::{
    app_dir, atomic_write, data_dir, load_metadata, now_ms, save_metadata, store_dir, METADATA_NAME,
};
use serde_json::{Map, Value};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use tauri::AppHandle;

pub const UPDATED: &str = "updatedAtMs";
pub const TOMBSTONES: &str = "deletedIds";
const MAX_TOMBSTONES: usize = 2000;

fn ms(v: &Value) -> u64 {
    v.get(UPDATED).and_then(Value::as_u64).unwrap_or(0)
}

fn id_of(v: &Value) -> Option<&str> {
    v.get("id").and_then(Value::as_str)
}

/// Массив объектов, у каждого из которых есть строковый `id`.
fn id_array(v: &Value) -> Option<&Vec<Value>> {
    let arr = v.as_array()?;
    (!arr.is_empty() && arr.iter().all(|x| id_of(x).is_some())).then_some(arr)
}

fn tombstones(v: &Value) -> Vec<String> {
    v.get(TOMBSTONES)
        .and_then(Value::as_array)
        .map(|a| {
            a.iter()
                .filter_map(|x| x.as_str().map(str::to_string))
                .collect()
        })
        .unwrap_or_default()
}

fn set_tombstones(obj: &mut Map<String, Value>, mut ids: Vec<String>) {
    let mut seen = HashSet::new();
    ids.retain(|id| seen.insert(id.clone()));
    if ids.len() > MAX_TOMBSTONES {
        ids.drain(..ids.len() - MAX_TOMBSTONES);
    }
    if ids.is_empty() {
        obj.remove(TOMBSTONES);
    } else {
        obj.insert(TOMBSTONES.into(), ids.into());
    }
}

fn without_bookkeeping(v: &Value) -> Value {
    let mut v = v.clone();
    if let Some(o) = v.as_object_mut() {
        o.remove(UPDATED);
        o.remove(TOMBSTONES);
    }
    v
}

/// Перед записью метаданных: изменившимся книгам ставит `updatedAtMs`, а `id` исчезнувших
/// элементов (удалённых цитат, заметок, выделений) добавляет в `deletedIds`.
/// Служебные поля всегда берутся из прежней версии на диске: копия в интерфейсе может их не знать.
pub fn stamp_changes(old: &LibraryMetadata, new: &mut LibraryMetadata, now_ms: u64) {
    for (path, book) in new.books.iter_mut() {
        let Ok(mut nv) = serde_json::to_value(&*book) else {
            continue;
        };
        let ov = old
            .books
            .get(path)
            .and_then(|b| serde_json::to_value(b).ok());
        let (updated, tomb) = match &ov {
            None => (now_ms, tombstones(&nv)),
            Some(ov) if without_bookkeeping(ov) == without_bookkeeping(&nv) => {
                (ms(ov).max(ms(&nv)), {
                    let mut t = tombstones(ov);
                    t.extend(tombstones(&nv));
                    t
                })
            }
            Some(ov) => {
                let mut t = tombstones(ov);
                t.extend(tombstones(&nv));
                if let (Some(oo), Some(no)) = (ov.as_object(), nv.as_object()) {
                    for (key, oval) in oo {
                        let Some(old_items) = id_array(oval) else {
                            continue;
                        };
                        let kept: HashSet<&str> = no
                            .get(key)
                            .and_then(Value::as_array)
                            .map(|a| a.iter().filter_map(id_of).collect())
                            .unwrap_or_default();
                        t.extend(
                            old_items
                                .iter()
                                .filter_map(id_of)
                                .filter(|id| !kept.contains(id))
                                .map(str::to_string),
                        );
                    }
                }
                (now_ms, t)
            }
        };
        if let Some(o) = nv.as_object_mut() {
            o.insert(UPDATED.into(), updated.into());
            set_tombstones(o, tomb);
        }
        if let Ok(b) = serde_json::from_value(nv) {
            *book = b;
        }
    }
}

/// Слияние двух версий одной книги (JSON-объекты).
pub fn merge_book(a: &Value, b: &Value) -> Value {
    let (newer, older) = if ms(b) > ms(a) { (b, a) } else { (a, b) };
    let (Some(n), Some(o)) = (newer.as_object(), older.as_object()) else {
        return newer.clone();
    };
    let mut tomb = tombstones(newer);
    tomb.extend(tombstones(older));
    let dead: HashSet<&str> = tomb.iter().map(String::as_str).collect();

    let mut out = Map::new();
    let keys: Vec<&String> = n
        .keys()
        .chain(o.keys().filter(|k| !n.contains_key(*k)))
        .collect();
    for key in keys {
        if key == UPDATED || key == TOMBSTONES {
            continue;
        }
        let (nv, ov) = (n.get(key), o.get(key));
        // Массивы элементов с id (или пустые/отсутствующие с одной стороны) складываем.
        let mergeable = |v: Option<&Value>| {
            v.is_none_or(|v| id_array(v).is_some() || v.as_array().is_some_and(Vec::is_empty))
        };
        let has_items = nv.and_then(id_array).is_some() || ov.and_then(id_array).is_some();
        let merged = if has_items && mergeable(nv) && mergeable(ov) {
            let mut seen = HashSet::new();
            let items: Vec<Value> = nv
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .chain(ov.and_then(Value::as_array).into_iter().flatten())
                .filter(|x| {
                    id_of(x).is_some_and(|id| !dead.contains(id) && seen.insert(id.to_string()))
                })
                .cloned()
                .collect();
            Value::Array(items)
        } else {
            match (nv, ov) {
                (Some(nv), _) => nv.clone(),
                (None, Some(ov)) => ov.clone(),
                (None, None) => continue,
            }
        };
        out.insert(key.clone(), merged);
    }
    out.insert(UPDATED.into(), ms(newer).into());
    set_tombstones(&mut out, tomb);
    Value::Object(out)
}

pub fn merge_metadata(a: &LibraryMetadata, b: &LibraryMetadata) -> LibraryMetadata {
    let mut out = a.clone();
    for shelf in &b.shelves {
        if !out.shelves.iter().any(|s| s.id == shelf.id) {
            out.shelves.push(shelf.clone());
        }
    }
    for (path, bb) in &b.books {
        let merged = match out.books.get(path) {
            None => bb.clone(),
            Some(ab) => {
                let (Ok(av), Ok(bv)) = (serde_json::to_value(ab), serde_json::to_value(bb)) else {
                    continue;
                };
                match serde_json::from_value(merge_book(&av, &bv)) {
                    Ok(m) => m,
                    Err(_) => continue,
                }
            }
        };
        out.books.insert(path.clone(), merged);
    }
    for (k, v) in &b.extra {
        out.extra.entry(k.clone()).or_insert_with(|| v.clone());
    }
    out
}

/// Слияние JSON-хранилищ по имени записи. `None` — формат неизвестен, оставляем основную версию.
pub fn merge_store(name: &str, a: &Value, b: &Value) -> Option<Value> {
    match name {
        // Карточки слов: у кого больше повторений — та версия свежее.
        "vocab" => {
            let score =
                |c: &Value| c["reps"].as_u64().unwrap_or(0) + c["lapses"].as_u64().unwrap_or(0);
            let mut by_id: HashMap<String, Value> = HashMap::new();
            let mut order = Vec::new();
            for card in a.as_array()?.iter().chain(b.as_array()?.iter()) {
                let Some(id) = id_of(card).map(str::to_string) else {
                    continue;
                };
                match by_id.get(&id) {
                    Some(cur) if score(cur) >= score(card) => {}
                    Some(_) => {
                        by_id.insert(id, card.clone());
                    }
                    None => {
                        order.push(id.clone());
                        by_id.insert(id, card.clone());
                    }
                }
            }
            Some(Value::Array(
                order
                    .into_iter()
                    .filter_map(|id| by_id.remove(&id))
                    .collect(),
            ))
        }
        // Статистика: одни и те же минуты могли попасть в обе копии — берём максимум, а не сумму.
        "reading-stats" => {
            let mut out = a.clone();
            for (day, bd) in b["days"].as_object()? {
                let ad = &mut out["days"][day];
                if ad.is_null() {
                    *ad = bd.clone();
                    continue;
                }
                for f in ["ms", "chars", "pages"] {
                    let m = ad[f].as_u64().unwrap_or(0).max(bd[f].as_u64().unwrap_or(0));
                    ad[f] = m.into();
                }
                let mut books: Vec<Value> = ad["books"].as_array().cloned().unwrap_or_default();
                for x in bd["books"].as_array().into_iter().flatten() {
                    if !books.contains(x) {
                        books.push(x.clone());
                    }
                }
                ad["books"] = books.into();
            }
            for (path, bb) in b["books"].as_object()? {
                let ab = &mut out["books"][path];
                if ab.is_null() {
                    *ab = bb.clone();
                    continue;
                }
                for f in ["ms", "sessions"] {
                    let m = ab[f].as_u64().unwrap_or(0).max(bb[f].as_u64().unwrap_or(0));
                    ab[f] = m.into();
                }
                // ISO-строки сравниваются лексикографически.
                if bb["firstAt"].as_str() < ab["firstAt"].as_str() && bb["firstAt"].is_string() {
                    ab["firstAt"] = bb["firstAt"].clone();
                }
                if bb["lastAt"].as_str() > ab["lastAt"].as_str() {
                    ab["lastAt"] = bb["lastAt"].clone();
                }
            }
            let samples = |v: &Value| v["speed"]["samples"].as_u64().unwrap_or(0);
            if samples(b) > samples(a) {
                out["speed"] = b["speed"].clone();
            }
            Some(out)
        }
        // Источники книг: объединяем по id.
        "book-sources" => {
            let mut out = a.clone();
            let list = out["sources"].as_array_mut()?;
            for s in b["sources"].as_array()? {
                match list.iter_mut().find(|x| id_of(x) == id_of(s)) {
                    Some(x) => {
                        if s["lastSyncedAt"].as_str() > x["lastSyncedAt"].as_str() {
                            x["lastSyncedAt"] = s["lastSyncedAt"].clone();
                        }
                    }
                    None => list.push(s.clone()),
                }
            }
            Some(out)
        }
        _ => None,
    }
}

/// Конфликтные копии файла `<stem>.<ext>` в каталоге:
/// Syncthing `stem.sync-conflict-…`, Nextcloud/ownCloud `stem (conflicted copy …)`,
/// Dropbox `stem (… conflicted copy …)` и подобные.
pub fn conflict_copies(dir: &Path, stem: &str, ext: &str) -> Vec<PathBuf> {
    let main = format!("{stem}.{ext}");
    let suffix = format!(".{ext}");
    let mut out: Vec<PathBuf> = std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|e| {
            let name = e.file_name().to_str()?.to_string();
            // Сразу после имени — `.sync-conflict…` или ` (… conflicted copy …)`,
            // иначе `glossary-ab` поймал бы копии `glossary-abc`.
            let rest = name.strip_prefix(stem)?;
            let is_copy = name != main
                && (rest.starts_with('.') || rest.starts_with(' '))
                && name.ends_with(&suffix)
                && rest.to_lowercase().contains("conflict");
            (is_copy && e.path().is_file()).then(|| e.path())
        })
        .collect();
    out.sort();
    out
}

fn items(v: Option<&Value>) -> Vec<Value> {
    v.and_then(Value::as_array).cloned().unwrap_or_default()
}

/// Трёхстороннее слияние книги: `base` — версия, которую видел интерфейс, `disk` — текущая
/// на диске, `incoming` — присланная интерфейсом. Берутся только изменения `incoming`
/// относительно `base`, всё остальное остаётся как на диске.
pub fn merge3_book(base: &Value, disk: &Value, incoming: &Value) -> Value {
    let empty = Map::new();
    let b = base.as_object().unwrap_or(&empty);
    let (Some(d), Some(i)) = (disk.as_object(), incoming.as_object()) else {
        return incoming.clone();
    };
    let mut out = d.clone();
    let keys: Vec<&String> = i
        .keys()
        .chain(b.keys().filter(|k| !i.contains_key(*k)))
        .collect();
    for key in keys {
        if key == UPDATED || key == TOMBSTONES {
            continue;
        }
        let (bv, dv, iv) = (b.get(key), d.get(key), i.get(key));
        if bv == iv {
            continue; // интерфейс это поле не трогал
        }
        let is_items = [bv, dv, iv]
            .iter()
            .any(|v| v.and_then(|v| id_array(v)).is_some());
        if !is_items {
            match iv {
                Some(v) => out.insert(key.clone(), v.clone()),
                None => out.remove(key),
            };
            continue;
        }
        let (bi, di, ii) = (items(bv), items(dv), items(iv));
        let base_by_id: HashMap<&str, &Value> =
            bi.iter().filter_map(|x| Some((id_of(x)?, x))).collect();
        let inc_by_id: HashMap<&str, &Value> =
            ii.iter().filter_map(|x| Some((id_of(x)?, x))).collect();
        // Удалены в интерфейсе: были в base, нет в incoming.
        let removed: HashSet<&str> = base_by_id
            .keys()
            .filter(|id| !inc_by_id.contains_key(*id))
            .copied()
            .collect();
        let mut seen = HashSet::new();
        let mut merged: Vec<Value> = Vec::new();
        for x in &di {
            let Some(id) = id_of(x) else { continue };
            if removed.contains(id) || !seen.insert(id.to_string()) {
                continue;
            }
            // Элемент правили в интерфейсе — берём его версию, иначе дисковую.
            let edited = inc_by_id
                .get(id)
                .filter(|iv| base_by_id.get(id) != Some(*iv));
            merged.push(edited.map_or_else(|| x.clone(), |v| (*v).clone()));
        }
        for x in &ii {
            if let Some(id) = id_of(x) {
                if !base_by_id.contains_key(id) && seen.insert(id.to_string()) {
                    merged.push(x.clone());
                }
            }
        }
        out.insert(key.clone(), Value::Array(merged));
    }
    Value::Object(out)
}

/// Трёхстороннее слияние метаданных (см. [`merge3_book`]).
pub fn merge3_metadata(
    base: &LibraryMetadata,
    disk: &LibraryMetadata,
    incoming: &LibraryMetadata,
) -> LibraryMetadata {
    let mut out = disk.clone();
    let to_v = |b: &crate::model::BookMeta| serde_json::to_value(b).ok();
    let paths: HashSet<&String> = disk.books.keys().chain(incoming.books.keys()).collect();
    for path in paths {
        let (b, d, i) = (
            base.books.get(path),
            disk.books.get(path),
            incoming.books.get(path),
        );
        match (d, i) {
            (Some(d), Some(i)) => {
                let (bv, dv, iv) = (b.and_then(to_v), to_v(d), to_v(i));
                let (Some(dv), Some(iv)) = (dv, iv) else {
                    continue;
                };
                let bv = bv.unwrap_or(Value::Null);
                if without_bookkeeping(&iv) == without_bookkeeping(&bv) {
                    continue;
                }
                if let Ok(m) = serde_json::from_value(merge3_book(&bv, &dv, &iv)) {
                    out.books.insert(path.clone(), m);
                }
            }
            // Есть на диске, нет в интерфейсе: интерфейс удалил запись, только если видел её
            // и она с тех пор не менялась.
            (Some(d), None) => {
                if let Some(b) = b {
                    if to_v(b).map(|v| without_bookkeeping(&v))
                        == to_v(d).map(|v| without_bookkeeping(&v))
                    {
                        out.books.remove(path);
                    }
                }
            }
            // Нет на диске: новая в интерфейсе — добавляем; была в base — её убрал бэкенд
            // (файл пропал, книга удалена), оставляем как на диске.
            (None, Some(i)) => {
                if b.is_none() {
                    out.books.insert(path.clone(), i.clone());
                }
            }
            (None, None) => {}
        }
    }
    if incoming
        .shelves
        .iter()
        .map(|s| (&s.id, &s.name, s.order))
        .ne(base.shelves.iter().map(|s| (&s.id, &s.name, s.order)))
    {
        out.shelves = incoming.shelves.clone();
    }
    for (k, v) in &incoming.extra {
        if base.extra.get(k) != Some(v) {
            out.extra.insert(k.clone(), v.clone());
        }
    }
    out
}

fn read_json<T: serde::de::DeserializeOwned>(p: &Path) -> Option<T> {
    serde_json::from_str(&std::fs::read_to_string(p).ok()?).ok()
}

/// Обработанную копию не удаляем, а уносим в локальный архив.
fn archive(app: &AppHandle, path: &Path) {
    let dir = app_dir(app).join("backups").join("conflicts");
    let _ = std::fs::create_dir_all(&dir);
    let name = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("conflict.json");
    let dest = dir.join(format!("{}-{name}", now_ms()));
    if std::fs::rename(path, &dest).is_err() && std::fs::copy(path, &dest).is_ok() {
        let _ = std::fs::remove_file(path);
    }
}

/// Находит и объединяет конфликтные копии метаданных, индекса и JSON-хранилища.
/// Вызывать под `META_LOCK`. Возвращает число обработанных копий.
pub fn resolve_conflicts(app: &AppHandle) -> usize {
    let dir = data_dir(app);
    let mut handled = 0;

    let meta_stem = METADATA_NAME.trim_end_matches(".json");
    let copies = conflict_copies(&dir, meta_stem, "json");
    if !copies.is_empty() {
        if let Ok(mut main) = load_metadata(app) {
            for c in &copies {
                if let Some(other) = read_json::<LibraryMetadata>(c) {
                    main = merge_metadata(&main, &other);
                }
            }
            if save_metadata(app, &main).is_ok() {
                for c in &copies {
                    archive(app, c);
                    handled += 1;
                }
            }
        }
    }

    // Индекс почти целиком пересчитывается сканированием; ценен только архив пропавших книг.
    let index_stem = INDEX_NAME.trim_end_matches(".json");
    let copies = conflict_copies(&dir, index_stem, "json");
    if !copies.is_empty() {
        let main_path = dir.join(INDEX_NAME);
        let mut main: LibraryIndex = read_json(&main_path).unwrap_or_default();
        for c in &copies {
            if let Some(other) = read_json::<LibraryIndex>(c) {
                for l in other.lost {
                    if !main
                        .lost
                        .iter()
                        .any(|m| m.path == l.path && m.lost_at_ms == l.lost_at_ms)
                    {
                        main.lost.push(l);
                    }
                }
            }
        }
        if let Ok(s) = serde_json::to_string(&main) {
            if atomic_write(&main_path, s.as_bytes()).is_ok() {
                for c in &copies {
                    archive(app, c);
                    handled += 1;
                }
            }
        }
    }

    let store = store_dir(app);
    let mains: Vec<String> = std::fs::read_dir(&store)
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|e| e.file_name().to_str().map(str::to_string))
        .filter(|n| n.ends_with(".json") && !n.to_lowercase().contains("conflict"))
        .map(|n| n.trim_end_matches(".json").to_string())
        .collect();
    for stem in mains {
        let copies = conflict_copies(&store, &stem, "json");
        if copies.is_empty() {
            continue;
        }
        let main_path = store.join(format!("{stem}.json"));
        let Some(mut main) = read_json::<Value>(&main_path) else {
            continue;
        };
        for c in &copies {
            if let Some(other) = read_json::<Value>(c) {
                if let Some(m) = merge_store(&stem, &main, &other) {
                    main = m;
                }
            }
        }
        if let Ok(s) = serde_json::to_string(&main) {
            if atomic_write(&main_path, s.as_bytes()).is_ok() {
                for c in &copies {
                    archive(app, c);
                    handled += 1;
                }
            }
        }
    }
    handled
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::default_metadata;
    use serde_json::json;

    fn meta_with(books: Value) -> LibraryMetadata {
        let mut m = default_metadata();
        m.books = serde_json::from_value(books).unwrap();
        m
    }

    fn book(extra: Value) -> Value {
        let mut b = json!({
            "path": "a.epub", "shelfId": "default", "importance": "normal", "review": ""
        });
        for (k, v) in extra.as_object().unwrap() {
            b[k] = v.clone();
        }
        b
    }

    #[test]
    fn stamping_marks_changes_and_deletions() {
        let old = meta_with(json!({ "a.epub": book(json!({
            "updatedAtMs": 5,
            "quotes": [{"id":"q1","text":"x","createdAt":"","accent":"","layout":""},
                       {"id":"q2","text":"y","createdAt":"","accent":"","layout":""}]
        })) }));
        // Интерфейс удалил q1 и прислал книгу без служебных полей.
        let mut new = meta_with(json!({ "a.epub": book(json!({
            "quotes": [{"id":"q2","text":"y","createdAt":"","accent":"","layout":""}]
        })) }));
        stamp_changes(&old, &mut new, 100);
        let v = serde_json::to_value(&new.books["a.epub"]).unwrap();
        assert_eq!(v[UPDATED], 100);
        assert_eq!(v[TOMBSTONES], json!(["q1"]));

        // Без изменений метка времени не двигается, надгробия сохраняются.
        let before = new.clone();
        stamp_changes(&before, &mut new, 200);
        let v = serde_json::to_value(&new.books["a.epub"]).unwrap();
        assert_eq!(v[UPDATED], 100);
        assert_eq!(v[TOMBSTONES], json!(["q1"]));
    }

    #[test]
    fn concurrent_edits_on_two_devices_merge() {
        // Устройство A: прочитал дальше и добавил выделение h2, удалил h0.
        let a = book(json!({
            "updatedAtMs": 200, "progress": 0.6,
            "highlights": [{"id":"h1","text":"1"},{"id":"h2","text":"2"}],
            "deletedIds": ["h0"]
        }));
        // Устройство B (раньше): добавил выделение h3, ещё видит h0, сменил оценку.
        let b = book(json!({
            "updatedAtMs": 100, "progress": 0.4, "review": "отлично",
            "highlights": [{"id":"h0","text":"0"},{"id":"h1","text":"1"},{"id":"h3","text":"3"}]
        }));
        let m = merge_book(&a, &b);
        let ids: Vec<&str> = m["highlights"]
            .as_array()
            .unwrap()
            .iter()
            .map(|h| h["id"].as_str().unwrap())
            .collect();
        assert_eq!(ids, ["h1", "h2", "h3"]);
        assert_eq!(m["progress"], 0.6); // свежее устройство
        assert_eq!(m["review"], ""); // поле есть у свежей версии — берём её
        assert_eq!(m[UPDATED], 200);
        assert_eq!(merge_book(&b, &a), m); // порядок аргументов не важен
    }

    #[test]
    fn stale_frontend_save_does_not_clobber_newer_disk() {
        let h = |id: &str| json!({"id": id, "text": id});
        // Интерфейс видел base, бэкенд с тех пор слил с другого устройства h3 и прогресс,
        // а интерфейс добавил h4, удалил h1 и поменял оценку.
        let base = meta_with(json!({ "a.epub": book(json!({
            "progress": 0.1, "highlights": [h("h1"), h("h2")]
        })) }));
        let disk = meta_with(json!({
            "a.epub": book(json!({ "progress": 0.5, "highlights": [h("h1"), h("h2"), h("h3")] })),
            "new.pdf": book(json!({ "path": "new.pdf" }))
        }));
        let incoming = meta_with(json!({ "a.epub": book(json!({
            "progress": 0.1, "review": "хорошо", "highlights": [h("h2"), h("h4")]
        })) }));
        let m = merge3_metadata(&base, &disk, &incoming);
        let a = serde_json::to_value(&m.books["a.epub"]).unwrap();
        let ids: Vec<&str> = a["highlights"]
            .as_array()
            .unwrap()
            .iter()
            .map(|h| h["id"].as_str().unwrap())
            .collect();
        assert_eq!(ids, ["h2", "h3", "h4"]);
        assert_eq!(a["progress"], 0.5); // интерфейс прогресс не менял
        assert_eq!(a["review"], "хорошо");
        // Книгу, найденную сканированием после base, интерфейс не знал — она остаётся.
        assert!(m.books.contains_key("new.pdf"));
    }

    #[test]
    fn whole_metadata_merge_keeps_books_from_both() {
        let a = meta_with(json!({ "a.epub": book(json!({"updatedAtMs": 1})) }));
        let mut b_book = book(json!({"updatedAtMs": 1}));
        b_book["path"] = "b.pdf".into();
        let b = meta_with(json!({ "b.pdf": b_book }));
        let m = merge_metadata(&a, &b);
        assert!(m.books.contains_key("a.epub") && m.books.contains_key("b.pdf"));
    }

    #[test]
    fn store_merges() {
        let a = json!([{"id":"w1","reps":3,"lapses":0},{"id":"w2","reps":0,"lapses":0}]);
        let b = json!([{"id":"w1","reps":1,"lapses":0},{"id":"w3","reps":0,"lapses":0}]);
        let m = merge_store("vocab", &a, &b).unwrap();
        assert_eq!(m.as_array().unwrap().len(), 3);
        assert_eq!(m[0]["reps"], 3);

        let a = json!({"days":{"d":{"ms":10,"chars":0,"pages":1,"books":["x"]}},
                       "books":{"x":{"ms":10,"sessions":1,"firstAt":"2026-01-02","lastAt":"2026-01-02"}},
                       "speed":{"samples":1}});
        let b = json!({"days":{"d":{"ms":30,"chars":0,"pages":0,"books":["y"]},"e":{"ms":5,"books":[]}},
                       "books":{"x":{"ms":5,"sessions":2,"firstAt":"2026-01-01","lastAt":"2026-01-01"}},
                       "speed":{"samples":9}});
        let m = merge_store("reading-stats", &a, &b).unwrap();
        assert_eq!(m["days"]["d"]["ms"], 30);
        assert_eq!(m["days"]["d"]["books"], json!(["x", "y"]));
        assert_eq!(m["days"]["e"]["ms"], 5);
        assert_eq!(m["books"]["x"]["sessions"], 2);
        assert_eq!(m["books"]["x"]["firstAt"], "2026-01-01");
        assert_eq!(m["books"]["x"]["lastAt"], "2026-01-02");
        assert_eq!(m["speed"]["samples"], 9);

        assert!(merge_store("glossary-abc", &a, &b).is_none());
    }

    #[test]
    fn finds_conflict_copies_of_popular_sync_tools() {
        let dir = std::env::temp_dir().join(format!("reader-conflicts-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        for name in [
            "library-metadata.json",
            "library-metadata.sync-conflict-20261010-101010-ABCDEFG.json",
            "library-metadata (conflicted copy 2026-10-10 101010).json",
            "library-metadata (Adam's conflicted copy 2026-10-10).json",
            "library-metadata-old.json",
            "library-metadataX.sync-conflict-1.json",
            "library-index.sync-conflict-20261010-101010-ABCDEFG.json",
        ] {
            std::fs::write(dir.join(name), "{}").unwrap();
        }
        let found = conflict_copies(&dir, "library-metadata", "json");
        let _ = std::fs::remove_dir_all(&dir);
        assert_eq!(found.len(), 3);
    }
}
