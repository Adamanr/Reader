//! Форматы файлов книг: распознавание по имени, проверка содержимого, чтение.
//!
//! `.fb2.zip` — самый частый вид FB2 в каналах и библиотеках. Распаковывается здесь, при чтении,
//! поэтому читалка, обложки, экспорт и перевод видят обычный FB2 и ничего о zip не знают.

use std::io::Read;
use std::path::Path;

/// Сколько можно распаковать из `.fb2.zip` — защита от zip-бомб.
const MAX_UNPACKED: u64 = 256 * 1024 * 1024;

/// Тип книги по имени файла: `pdf`, `epub`, `fb2`, `fb2.zip`, `typ`.
pub fn book_kind(name: &str) -> Option<&'static str> {
    let lower = name.to_lowercase();
    [
        (".fb2.zip", "fb2.zip"),
        (".pdf", "pdf"),
        (".epub", "epub"),
        (".fb2", "fb2"),
        (".typ", "typ"),
    ]
    .into_iter()
    .find(|(suffix, _)| lower.len() > suffix.len() && lower.ends_with(suffix))
    .map(|(_, kind)| kind)
}

/// Книга, которую можно импортировать (Typst-проекты создаются внутри приложения).
pub fn is_importable(name: &str) -> bool {
    matches!(book_kind(name), Some("pdf" | "epub" | "fb2" | "fb2.zip"))
}

/// Имя без расширения книги и само расширение (`"Книга.fb2.zip"` → `("Книга", "fb2.zip")`).
pub fn split_book_name(name: &str) -> Option<(&str, &'static str)> {
    let kind = book_kind(name)?;
    Some((&name[..name.len() - kind.len() - 1], kind))
}

fn read_fb2_from_zip(path: &Path) -> std::io::Result<Vec<u8>> {
    let file = std::fs::File::open(path)?;
    let mut zip = zip::ZipArchive::new(file).map_err(std::io::Error::other)?;
    let index = (0..zip.len())
        .find(|&i| {
            zip.by_index(i).is_ok_and(|f| {
                f.is_file() && f.name().is_ok_and(|n| n.to_lowercase().ends_with(".fb2"))
            })
        })
        .ok_or_else(|| std::io::Error::other("в архиве нет файла .fb2"))?;
    let entry = zip.by_index(index).map_err(std::io::Error::other)?;
    let mut out = Vec::new();
    entry.take(MAX_UNPACKED + 1).read_to_end(&mut out)?;
    if out.len() as u64 > MAX_UNPACKED {
        return Err(std::io::Error::other("FB2 в архиве слишком большой"));
    }
    Ok(out)
}

/// Имя файла из внешнего источника: без каталогов, управляющих и запрещённых символов.
pub fn sanitize_filename(name: &str) -> String {
    let base = name.rsplit(['/', '\\']).next().unwrap_or(name);
    let cleaned: String = base
        .chars()
        .map(|c| {
            if c.is_control() || matches!(c, ':' | '*' | '?' | '"' | '<' | '>' | '|') {
                '_'
            } else {
                c
            }
        })
        .collect();
    let cleaned = cleaned.trim().trim_start_matches('.').trim();
    if cleaned.is_empty() {
        "book".into()
    } else {
        cleaned.to_string()
    }
}

/// Свободное имя в папке: `Книга.epub`, `Книга (2).epub`… Расширение книги сохраняется целиком.
pub fn unique_dest(root: &Path, name: &str) -> std::path::PathBuf {
    let name = sanitize_filename(name);
    let (stem, ext) = split_book_name(&name).unwrap_or((name.as_str(), "bin"));
    let mut dest = root.join(format!("{stem}.{ext}"));
    let mut n = 2;
    while dest.exists() {
        dest = root.join(format!("{stem} ({n}).{ext}"));
        n += 1;
    }
    dest
}

/// Тот же файл уже есть в библиотеке (имя после очистки и размер совпадают).
pub fn existing_copy(root: &Path, name: &str, size: u64) -> Option<std::path::PathBuf> {
    let p = root.join(sanitize_filename(name));
    let meta = std::fs::metadata(&p).ok()?;
    (meta.is_file() && size > 0 && meta.len() == size).then_some(p)
}

/// Содержимое книги для читалки: для `.fb2.zip` — уже распакованный FB2.
pub fn read_book_file(path: &Path) -> std::io::Result<Vec<u8>> {
    let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
    if book_kind(name) == Some("fb2.zip") {
        read_fb2_from_zip(path)
    } else {
        std::fs::read(path)
    }
}

/// Проверка сигнатуры: в канал или каталог могут выложить что угодно с расширением `.pdf`.
pub fn content_matches(path: &Path, kind: &str) -> bool {
    let mut head = Vec::with_capacity(4096);
    let Ok(file) = std::fs::File::open(path) else {
        return false;
    };
    if file.take(4096).read_to_end(&mut head).is_err() {
        return false;
    }
    let contains = |needle: &[u8]| head.windows(needle.len()).any(|w| w == needle);
    match kind {
        "pdf" => contains(b"%PDF-"),
        "epub" => head.starts_with(b"PK\x03\x04"),
        "fb2" => contains(b"FictionBook"),
        "fb2.zip" => {
            head.starts_with(b"PK\x03\x04")
                && read_fb2_from_zip(path)
                    .is_ok_and(|b| b.windows(11).take(8192).any(|w| w == b"FictionBook"))
        }
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn recognizes_book_names() {
        assert_eq!(book_kind("Война и мир.FB2.ZIP"), Some("fb2.zip"));
        assert_eq!(book_kind("a.fb2"), Some("fb2"));
        assert_eq!(book_kind("a.zip"), None);
        assert_eq!(book_kind(".pdf"), None);
        assert_eq!(split_book_name("Книга.fb2.zip"), Some(("Книга", "fb2.zip")));
        assert_eq!(split_book_name("x.y.epub"), Some(("x.y", "epub")));
        assert!(is_importable("a.fb2.zip") && !is_importable("a.typ"));
    }

    #[test]
    fn sanitizes_and_dedupes_names() {
        assert_eq!(sanitize_filename("../../etc/passwd.pdf"), "passwd.pdf");
        assert_eq!(sanitize_filename("..\\..\\x.epub"), "x.epub");
        assert_eq!(sanitize_filename(".hidden.fb2"), "hidden.fb2");
        assert_eq!(sanitize_filename("a:b?.pdf"), "a_b_.pdf");
        assert_eq!(sanitize_filename("  "), "book");
        let dir = std::env::temp_dir().join(format!("reader-dest-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("Книга.fb2.zip"), b"x").unwrap();
        assert_eq!(
            unique_dest(&dir, "Книга.fb2.zip"),
            dir.join("Книга (2).fb2.zip")
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn unpacks_fb2_zip() {
        let dir = std::env::temp_dir().join(format!("reader-fb2zip-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("book.fb2.zip");
        let body = "<?xml version=\"1.0\"?><FictionBook><body><p>Привет</p></body></FictionBook>";
        {
            let mut zip = zip::ZipWriter::new(std::fs::File::create(&path).unwrap());
            let opts = zip::write::SimpleFileOptions::default()
                .compression_method(zip::CompressionMethod::Deflated);
            zip.start_file("readme.txt", opts).unwrap();
            zip.write_all(b"not a book").unwrap();
            zip.start_file("Book.FB2", opts).unwrap();
            zip.write_all(body.as_bytes()).unwrap();
            zip.finish().unwrap();
        }
        assert_eq!(read_book_file(&path).unwrap(), body.as_bytes());
        assert!(content_matches(&path, "fb2.zip"));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
