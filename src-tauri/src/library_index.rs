//! Опознание книг по содержимому.
//!
//! Метаданные (заметки, цитаты, прогресс) по-прежнему хранятся по пути файла, но рядом ведётся
//! индекс `library-index.json`: путь → отпечаток содержимого. Благодаря ему:
//! - переименование или перенос книги внутри библиотеки переносит её данные на новый путь;
//! - записи пропавших книг не удаляются, а уходят в архив `lost` и возвращаются, когда файл
//!   с тем же содержимым (или по тому же пути) снова появляется — например, после
//!   досинхронизации, подключения диска или смены папки библиотеки.

use crate::library::new_book_meta;
use crate::model::{BookMeta, LibraryMetadata};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{HashMap, HashSet};
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;

pub const INDEX_NAME: &str = "library-index.json";
/// Отпечаток строится по размеру, началу и концу файла — без чтения сотен мегабайт PDF.
const SAMPLE: u64 = 64 * 1024;
/// Через сколько архивная запись пропавшей книги забывается.
const LOST_TTL_MS: u64 = 365 * 24 * 60 * 60 * 1000;
const LOST_MAX: usize = 5000;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FileEntry {
    pub id: String,
    pub size: u64,
    pub mtime_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LostBook {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    pub path: String,
    pub lost_at_ms: u64,
    pub meta: BookMeta,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LibraryIndex {
    #[serde(default)]
    pub files: HashMap<String, FileEntry>,
    #[serde(default)]
    pub lost: Vec<LostBook>,
}

/// Книга сменила путь: связанные с путём данные нужно перенести.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Relocation {
    pub from: String,
    pub to: String,
}

#[derive(Debug, Default)]
pub struct Reconciled {
    pub relocations: Vec<Relocation>,
    pub metadata_changed: bool,
    pub index_changed: bool,
}

pub fn content_id(path: &Path, size: u64) -> std::io::Result<String> {
    let mut file = std::fs::File::open(path)?;
    let mut h = Sha256::new();
    h.update(size.to_le_bytes());
    let mut buf = Vec::with_capacity((2 * SAMPLE) as usize);
    (&mut file).take(SAMPLE).read_to_end(&mut buf)?;
    if size > SAMPLE {
        file.seek(SeekFrom::Start(size.saturating_sub(SAMPLE).max(SAMPLE)))?;
        file.take(SAMPLE).read_to_end(&mut buf)?;
    }
    h.update(&buf);
    Ok(h.finalize()[..16]
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect())
}

fn mtime_ms(meta: &std::fs::Metadata) -> u64 {
    meta.modified()
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map_or(0, |d| d.as_millis() as u64)
}

/// Отпечатки текущих файлов; пересчитываются только для новых или изменившихся.
fn fingerprint(
    root: &Path,
    paths: &[String],
    old: &HashMap<String, FileEntry>,
) -> HashMap<String, FileEntry> {
    let mut out = HashMap::with_capacity(paths.len());
    for p in paths {
        let full = root.join(p);
        let Ok(meta) = std::fs::metadata(&full) else {
            continue;
        };
        let (size, mtime_ms) = (meta.len(), mtime_ms(&meta));
        let entry = match old.get(p) {
            Some(e) if e.size == size && e.mtime_ms == mtime_ms => e.clone(),
            _ => match content_id(&full, size) {
                Ok(id) => FileEntry { id, size, mtime_ms },
                Err(_) => continue,
            },
        };
        out.insert(p.clone(), entry);
    }
    out
}

/// Сверяет метаданные с файлами библиотеки. Ничего не удаляет безвозвратно:
/// пропавшие записи попадают в `index.lost`.
pub fn reconcile(
    root: &Path,
    paths: &[String],
    meta: &mut LibraryMetadata,
    index: &mut LibraryIndex,
    now_ms: u64,
) -> Reconciled {
    let mut out = Reconciled::default();
    let files = fingerprint(root, paths, &index.files);
    let present: HashSet<&String> = paths.iter().collect();

    // Сначала убираем пропавшие — новые пути ниже могут оказаться ими же после переименования.
    let missing: Vec<String> = meta
        .books
        .keys()
        .filter(|k| !present.contains(k))
        .cloned()
        .collect();
    for path in missing {
        if let Some(book) = meta.books.remove(&path) {
            index.lost.push(LostBook {
                id: index.files.get(&path).map(|e| e.id.clone()),
                path,
                lost_at_ms: now_ms,
                meta: book,
            });
            out.metadata_changed = true;
            out.index_changed = true;
        }
    }

    for path in paths {
        if meta.books.contains_key(path) {
            continue;
        }
        let id = files.get(path).map(|e| &e.id);
        // Свежая архивная запись с тем же содержимым, иначе — с тем же путём.
        let found = id
            .and_then(|id| newest_lost(&index.lost, |l| l.id.as_ref() == Some(id)))
            .or_else(|| newest_lost(&index.lost, |l| &l.path == path));
        let book = match found {
            Some(i) => {
                let lost = index.lost.swap_remove(i);
                if &lost.path != path {
                    out.relocations.push(Relocation {
                        from: lost.path,
                        to: path.clone(),
                    });
                }
                out.index_changed = true;
                BookMeta {
                    path: path.clone(),
                    ..lost.meta
                }
            }
            None => new_book_meta(path, now_ms),
        };
        meta.books.insert(path.clone(), book);
        out.metadata_changed = true;
    }

    let before = index.lost.len();
    index
        .lost
        .retain(|l| now_ms.saturating_sub(l.lost_at_ms) < LOST_TTL_MS);
    if index.lost.len() > LOST_MAX {
        index.lost.sort_by_key(|l| std::cmp::Reverse(l.lost_at_ms));
        index.lost.truncate(LOST_MAX);
    }
    out.index_changed |= index.lost.len() != before || index.files != files;
    index.files = files;
    out
}

fn newest_lost(lost: &[LostBook], pred: impl Fn(&LostBook) -> bool) -> Option<usize> {
    lost.iter()
        .enumerate()
        .filter(|(_, l)| pred(l))
        .max_by_key(|(_, l)| l.lost_at_ms)
        .map(|(i, _)| i)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::default_metadata;
    use std::path::PathBuf;

    struct TempLib(PathBuf);

    impl TempLib {
        fn new(name: &str) -> Self {
            let dir = std::env::temp_dir()
                .join(format!("reader-index-test-{name}-{}", std::process::id()));
            let _ = std::fs::remove_dir_all(&dir);
            std::fs::create_dir_all(&dir).unwrap();
            Self(dir)
        }

        fn write(&self, rel: &str, body: &[u8]) {
            let p = self.0.join(rel);
            std::fs::create_dir_all(p.parent().unwrap()).unwrap();
            std::fs::write(p, body).unwrap();
        }

        fn rename(&self, from: &str, to: &str) {
            let to = self.0.join(to);
            std::fs::create_dir_all(to.parent().unwrap()).unwrap();
            std::fs::rename(self.0.join(from), to).unwrap();
        }

        fn scan(
            &self,
            meta: &mut LibraryMetadata,
            index: &mut LibraryIndex,
            now: u64,
        ) -> Reconciled {
            let paths = crate::library::scan_library(&self.0).unwrap();
            reconcile(&self.0, &paths, meta, index, now)
        }
    }

    impl Drop for TempLib {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn with_note(meta: &mut LibraryMetadata, path: &str, review: &str) {
        meta.books.get_mut(path).unwrap().review = review.into();
    }

    #[test]
    fn rename_keeps_notes() {
        let lib = TempLib::new("rename");
        lib.write("a.pdf", b"%PDF-1.7 one");
        let (mut meta, mut index) = (default_metadata(), LibraryIndex::default());
        lib.scan(&mut meta, &mut index, 1);
        with_note(&mut meta, "a.pdf", "заметка");

        lib.rename("a.pdf", "shelf/b.pdf");
        let r = lib.scan(&mut meta, &mut index, 2);

        assert_eq!(
            r.relocations,
            vec![Relocation {
                from: "a.pdf".into(),
                to: "shelf/b.pdf".into()
            }]
        );
        let book = &meta.books["shelf/b.pdf"];
        assert_eq!(book.review, "заметка");
        assert_eq!(book.path, "shelf/b.pdf");
        assert!(!meta.books.contains_key("a.pdf"));
        assert!(index.lost.is_empty());
    }

    #[test]
    fn vanished_books_come_back() {
        let lib = TempLib::new("vanish");
        lib.write("a.epub", b"PK\x03\x04 book a");
        lib.write("b.fb2", b"<FictionBook> b");
        let (mut meta, mut index) = (default_metadata(), LibraryIndex::default());
        lib.scan(&mut meta, &mut index, 1);
        with_note(&mut meta, "a.epub", "A");
        with_note(&mut meta, "b.fb2", "B");

        // Диск отключён / синхронизация не закончена: файлов нет, данные уходят в архив.
        std::fs::remove_file(lib.0.join("a.epub")).unwrap();
        std::fs::remove_file(lib.0.join("b.fb2")).unwrap();
        lib.scan(&mut meta, &mut index, 2);
        assert!(meta.books.is_empty());
        assert_eq!(index.lost.len(), 2);

        lib.write("a.epub", b"PK\x03\x04 book a");
        lib.write("b.fb2", b"<FictionBook> b, edited");
        let r = lib.scan(&mut meta, &mut index, 3);
        assert!(r.relocations.is_empty());
        assert_eq!(meta.books["a.epub"].review, "A");
        // Содержимое поменялось, но путь тот же — это та же книга.
        assert_eq!(meta.books["b.fb2"].review, "B");
        assert!(index.lost.is_empty());
    }

    #[test]
    fn unrelated_new_book_starts_clean_and_lost_expires() {
        let lib = TempLib::new("fresh");
        lib.write("a.pdf", b"%PDF- a");
        let (mut meta, mut index) = (default_metadata(), LibraryIndex::default());
        lib.scan(&mut meta, &mut index, 1);
        with_note(&mut meta, "a.pdf", "A");

        std::fs::remove_file(lib.0.join("a.pdf")).unwrap();
        lib.write("c.pdf", b"%PDF- completely different");
        lib.scan(&mut meta, &mut index, 2);
        assert_eq!(meta.books["c.pdf"].review, "");
        assert_eq!(index.lost.len(), 1);

        lib.scan(&mut meta, &mut index, 2 + LOST_TTL_MS);
        assert!(index.lost.is_empty());
    }

    #[test]
    fn large_file_id_uses_head_and_tail() {
        let lib = TempLib::new("large");
        let mut body = vec![7u8; 300 * 1024];
        lib.write("x.pdf", &body);
        let p = lib.0.join("x.pdf");
        let a = content_id(&p, body.len() as u64).unwrap();
        *body.last_mut().unwrap() = 8;
        lib.write("x.pdf", &body);
        let b = content_id(&p, body.len() as u64).unwrap();
        assert_ne!(a, b);
    }
}
