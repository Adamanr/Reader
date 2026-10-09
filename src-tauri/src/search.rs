//! Полнотекстовый поиск по книгам и заметкам библиотеки.
//!
//! Локальный индекс tantivy в каталоге приложения (не в библиотеке: его всегда можно
//! пересобрать, синхронизировать незачем). Документ индекса — страница PDF, глава EPUB,
//! раздел FB2 или заметки одной книги. Морфология русского и английского: «книгами»
//! находит «книга». Обновляется инкрементально — по отпечаткам содержимого из
//! `library_index`, поэтому переименование файла не заставляет перечитывать книгу.

use crate::library::scan_library;
use crate::storage::{app_dir, load_config, load_metadata, META_LOCK};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, OnceLock};
use tantivy::collector::TopDocs;
use tantivy::directory::MmapDirectory;
use tantivy::query::QueryParser;
use tantivy::schema::{
    Field, IndexRecordOption, Schema, TextFieldIndexing, TextOptions, Value, STORED, STRING,
};
use tantivy::snippet::SnippetGenerator;
use tantivy::tokenizer::{
    Language, LowerCaser, RemoveLongFilter, SimpleTokenizer, Stemmer, TextAnalyzer,
};
use tantivy::{doc, Index, IndexReader, IndexWriter, ReloadPolicy, TantivyDocument, Term};
use tauri::{AppHandle, Emitter};

/// Меняется вместе со схемой или разбором — индекс пересобирается с нуля.
const SCHEMA_VERSION: u32 = 1;
const TOKENIZER: &str = "ru_en";
/// Больше этого из одной книги не берём (энциклопедии на тысячи страниц).
const MAX_TEXT_PER_BOOK: usize = 30 * 1024 * 1024;
const COMMIT_EVERY: usize = 10;

#[derive(Clone, Copy)]
struct Fields {
    path: Field,
    kind: Field,
    title: Field,
    label: Field,
    body: Field,
}

struct Engine {
    index: Index,
    reader: IndexReader,
    fields: Fields,
}

static ENGINE: OnceLock<Mutex<Option<std::sync::Arc<Engine>>>> = OnceLock::new();
static RUNNING: AtomicBool = AtomicBool::new(false);

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct IndexState {
    version: u32,
    /// путь → отпечаток содержимого, с которым книга проиндексирована.
    books: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct SearchHit {
    pub path: String,
    /// `book` — текст книги, `note` — заметки, цитаты и выделения.
    pub kind: String,
    pub title: String,
    pub label: String,
    /// Фрагмент с `<b>` вокруг совпадений; остальной текст экранирован.
    pub snippet: String,
}

#[derive(Debug, Clone, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct SearchStatus {
    pub indexed_books: usize,
    pub library_books: usize,
    pub running: bool,
}

#[derive(Debug, Clone, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct IndexProgress {
    done: usize,
    total: usize,
    current: String,
    finished: bool,
    error: Option<String>,
}

fn index_dir(app: &AppHandle) -> PathBuf {
    app_dir(app).join("search-index")
}

fn state_path(app: &AppHandle) -> PathBuf {
    app_dir(app).join("search-index.json")
}

fn schema() -> (Schema, Fields) {
    let mut b = Schema::builder();
    let text = TextOptions::default()
        .set_indexing_options(
            TextFieldIndexing::default()
                .set_tokenizer(TOKENIZER)
                .set_index_option(IndexRecordOption::WithFreqsAndPositions),
        )
        .set_stored();
    let fields = Fields {
        path: b.add_text_field("path", STRING | STORED),
        kind: b.add_text_field("kind", STRING | STORED),
        title: b.add_text_field("title", text.clone()),
        label: b.add_text_field("label", STORED),
        body: b.add_text_field("body", text),
    };
    (b.build(), fields)
}

fn analyzer() -> TextAnalyzer {
    TextAnalyzer::builder(SimpleTokenizer::default())
        .filter(RemoveLongFilter::limit(40))
        .filter(LowerCaser)
        .filter(Stemmer::new(Language::Russian))
        .filter(Stemmer::new(Language::English))
        .build()
}

fn open_engine_at(dir: &Path) -> tantivy::Result<Engine> {
    std::fs::create_dir_all(dir)?;
    let (schema, fields) = schema();
    let index = Index::open_or_create(MmapDirectory::open(dir)?, schema)?;
    index.tokenizers().register(TOKENIZER, analyzer());
    let reader = index
        .reader_builder()
        .reload_policy(ReloadPolicy::OnCommitWithDelay)
        .try_into()?;
    Ok(Engine {
        index,
        reader,
        fields,
    })
}

fn engine(app: &AppHandle) -> Result<std::sync::Arc<Engine>, String> {
    let cell = ENGINE.get_or_init(|| Mutex::new(None));
    let mut guard = cell.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(e) = guard.as_ref() {
        return Ok(e.clone());
    }
    let dir = index_dir(app);
    let state: IndexState = read_state(app);
    // Схема поменялась или индекс повреждён — начинаем заново.
    let opened = if state.version == SCHEMA_VERSION {
        open_engine_at(&dir).ok()
    } else {
        None
    };
    let e = match opened {
        Some(e) => e,
        None => {
            let _ = std::fs::remove_dir_all(&dir);
            let _ = std::fs::remove_file(state_path(app));
            open_engine_at(&dir).map_err(|e| format!("Поисковый индекс: {e}"))?
        }
    };
    let e = std::sync::Arc::new(e);
    *guard = Some(e.clone());
    Ok(e)
}

fn read_state(app: &AppHandle) -> IndexState {
    std::fs::read_to_string(state_path(app))
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

fn write_state(app: &AppHandle, s: &IndexState) {
    if let Ok(json) = serde_json::to_string(s) {
        let _ = crate::storage::atomic_write(&state_path(app), json.as_bytes());
    }
}

// ——— Извлечение текста ———

#[derive(Serialize, Deserialize)]
pub(crate) struct Chunk {
    pub label: String,
    pub text: String,
}

/// Флаг вспомогательного режима: `reader --reader-extract-text <книга> <вывод.json>`.
pub const EXTRACT_FLAG: &str = "--reader-extract-text";

/// Вспомогательный процесс: извлекает текст и пишет JSON в файл (не в stdout —
/// сторонние парсеры иногда печатают туда отладку).
pub fn helper_extract(book: &Path, out: &Path) -> i32 {
    match extract_chunks(book).and_then(|c| serde_json::to_vec(&c).map_err(|e| e.to_string())) {
        Ok(json) => match std::fs::write(out, json) {
            Ok(()) => 0,
            Err(_) => 2,
        },
        Err(_) => 1,
    }
}

/// Разбор PDF в отдельном процессе с таймаутом: чужой парсер может зависнуть,
/// упасть или съесть память на неудачном файле — индексация просто пропустит книгу.
fn extract_isolated(path: &Path) -> Result<Vec<Chunk>, String> {
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let out = std::env::temp_dir().join(format!(
        "reader-extract-{}-{}.json",
        std::process::id(),
        crate::storage::now_ms()
    ));
    let size_mb = std::fs::metadata(path)
        .map(|m| m.len() / (1024 * 1024))
        .unwrap_or(0);
    let limit = std::time::Duration::from_secs((60 + 8 * size_mb).min(600));
    let mut child = std::process::Command::new(exe)
        .arg(EXTRACT_FLAG)
        .arg(path)
        .arg(&out)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .map_err(|e| e.to_string())?;
    let started = std::time::Instant::now();
    let status = loop {
        if let Some(st) = child.try_wait().map_err(|e| e.to_string())? {
            break st;
        }
        if started.elapsed() > limit {
            let _ = child.kill();
            let _ = child.wait();
            let _ = std::fs::remove_file(&out);
            return Err(format!("текст не извлечён за {} с", limit.as_secs()));
        }
        std::thread::sleep(std::time::Duration::from_millis(100));
    };
    let result = if status.success() {
        std::fs::read(&out)
            .map_err(|e| e.to_string())
            .and_then(|b| serde_json::from_slice(&b).map_err(|e| e.to_string()))
    } else {
        Err("не удалось разобрать файл".to_string())
    };
    let _ = std::fs::remove_file(&out);
    result
}

fn decode_entities(s: &str) -> String {
    if !s.contains('&') {
        return s.to_string();
    }
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(i) = rest.find('&') {
        out.push_str(&rest[..i]);
        rest = &rest[i..];
        // По байтам: срез строки по 12-му байту мог бы попасть в середину буквы.
        let Some(end) = rest.bytes().take(12).position(|b| b == b';') else {
            out.push('&');
            rest = &rest[1..];
            continue;
        };
        let ent = &rest[1..end];
        let ch = match ent {
            "amp" => Some('&'),
            "lt" => Some('<'),
            "gt" => Some('>'),
            "quot" => Some('"'),
            "apos" => Some('\''),
            "nbsp" => Some(' '),
            "mdash" => Some('—'),
            "ndash" => Some('–'),
            "laquo" => Some('«'),
            "raquo" => Some('»'),
            "hellip" => Some('…'),
            _ => ent
                .strip_prefix("#x")
                .or_else(|| ent.strip_prefix("#X"))
                .and_then(|h| u32::from_str_radix(h, 16).ok())
                .or_else(|| ent.strip_prefix('#').and_then(|d| d.parse().ok()))
                .and_then(char::from_u32),
        };
        match ch {
            Some(c) => {
                out.push(c);
                rest = &rest[end + 1..];
            }
            None => {
                out.push('&');
                rest = &rest[1..];
            }
        }
    }
    out.push_str(rest);
    out
}

/// Текст из (X)HTML главы и её первый заголовок.
fn html_to_text(html: &str) -> (String, Option<String>) {
    let mut out = String::with_capacity(html.len() / 2);
    let mut heading: Option<String> = None;
    let mut in_heading = false;
    let mut heading_buf = String::new();
    let lower = html.to_ascii_lowercase();
    let bytes = html.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'<' {
            let Some(rel_end) = html[i..].find('>') else {
                break;
            };
            let tag = &lower[i + 1..i + rel_end];
            // Содержимое script/style не текст: прыгаем сразу к закрывающему тегу.
            if let Some(skip) = ["script", "style"].into_iter().find(|t| tag.starts_with(t)) {
                if let Some(close) = lower[i + rel_end..].find(&format!("</{skip}")) {
                    i += rel_end + close;
                    continue;
                }
            }
            let name: String = tag
                .trim_start_matches('/')
                .chars()
                .take_while(|c| c.is_ascii_alphanumeric())
                .collect();
            if matches!(name.as_str(), "h1" | "h2" | "h3") && heading.is_none() {
                if tag.starts_with('/') {
                    in_heading = false;
                    let h = heading_buf.split_whitespace().collect::<Vec<_>>().join(" ");
                    if !h.is_empty() {
                        heading = Some(decode_entities(&h));
                    }
                } else {
                    in_heading = true;
                }
            }
            out.push(' ');
            i += rel_end + 1;
        } else {
            let next = html[i..].find('<').map_or(html.len(), |n| i + n);
            let piece = &html[i..next];
            out.push_str(piece);
            if in_heading {
                heading_buf.push_str(piece);
            }
            i = next;
        }
    }
    let text = decode_entities(&out);
    (
        text.split_whitespace().collect::<Vec<_>>().join(" "),
        heading,
    )
}

fn zip_read(zip: &mut zip::ZipArchive<std::fs::File>, name: &str) -> Option<String> {
    let mut f = zip.by_name(name).ok()?;
    let mut s = String::new();
    (&mut f)
        .take(MAX_TEXT_PER_BOOK as u64)
        .read_to_string(&mut s)
        .ok()?;
    Some(s)
}

fn percent_decode(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%' && i + 2 < b.len() {
            if let Ok(v) = u8::from_str_radix(&s[i + 1..i + 3], 16) {
                out.push(v);
                i += 3;
                continue;
            }
        }
        out.push(b[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// `a/b/../c.xhtml` → `a/c.xhtml`.
fn normalize_zip_path(p: &str) -> String {
    let mut parts: Vec<&str> = Vec::new();
    for seg in p.split('/') {
        match seg {
            "" | "." => {}
            ".." => {
                parts.pop();
            }
            s => parts.push(s),
        }
    }
    parts.join("/")
}

fn epub_chunks(path: &Path) -> Result<Vec<Chunk>, String> {
    let file = std::fs::File::open(path).map_err(|e| e.to_string())?;
    let mut zip = zip::ZipArchive::new(file).map_err(|e| e.to_string())?;
    let container = zip_read(&mut zip, "META-INF/container.xml").ok_or("нет container.xml")?;
    let opf_path = roxmltree::Document::parse(&container)
        .ok()
        .and_then(|d| {
            d.descendants()
                .find(|n| n.tag_name().name() == "rootfile")
                .and_then(|n| n.attribute("full-path").map(str::to_string))
        })
        .ok_or("нет пути к OPF")?;
    let opf = zip_read(&mut zip, &opf_path).ok_or("нет OPF")?;
    let opf_dir = opf_path.rsplit_once('/').map_or("", |(d, _)| d);
    let doc = roxmltree::Document::parse(&opf).map_err(|e| e.to_string())?;
    let manifest: HashMap<&str, &str> = doc
        .descendants()
        .filter(|n| n.tag_name().name() == "item")
        .filter_map(|n| Some((n.attribute("id")?, n.attribute("href")?)))
        .collect();
    let spine: Vec<String> = doc
        .descendants()
        .filter(|n| n.tag_name().name() == "itemref")
        .filter_map(|n| manifest.get(n.attribute("idref")?))
        .map(|href| {
            let href = percent_decode(href.split('#').next().unwrap_or(href));
            normalize_zip_path(&if opf_dir.is_empty() {
                href
            } else {
                format!("{opf_dir}/{href}")
            })
        })
        .collect();
    let mut out = Vec::new();
    let mut total = 0;
    for (i, name) in spine.iter().enumerate() {
        let Some(html) = zip_read(&mut zip, name) else {
            continue;
        };
        let (text, heading) = html_to_text(&html);
        if text.trim().is_empty() {
            continue;
        }
        total += text.len();
        out.push(Chunk {
            label: heading
                .map(|h| h.chars().take(80).collect())
                .unwrap_or_else(|| format!("Часть {}", i + 1)),
            text,
        });
        if total > MAX_TEXT_PER_BOOK {
            break;
        }
    }
    Ok(out)
}

/// FB2 бывает в windows-1251 и других кодировках — смотрим объявление XML.
fn decode_xml(bytes: &[u8]) -> String {
    let head = String::from_utf8_lossy(&bytes[..bytes.len().min(200)]).to_lowercase();
    let label = head
        .split("encoding=")
        .nth(1)
        .and_then(|r| r.trim_start_matches(['"', '\'']).split(['"', '\'']).next())
        .unwrap_or("utf-8")
        .to_string();
    let enc = encoding_rs::Encoding::for_label(label.as_bytes()).unwrap_or(encoding_rs::UTF_8);
    enc.decode(bytes).0.into_owned()
}

fn node_text(n: roxmltree::Node) -> String {
    n.descendants()
        .filter(|d| d.is_text())
        .filter_map(|d| d.text())
        .collect::<Vec<_>>()
        .join(" ")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn fb2_chunks(path: &Path) -> Result<Vec<Chunk>, String> {
    let bytes = crate::formats::read_book_file(path).map_err(|e| e.to_string())?;
    let xml = decode_xml(&bytes);
    let opts = roxmltree::ParsingOptions {
        allow_dtd: true,
        ..Default::default()
    };
    let doc = roxmltree::Document::parse_with_options(&xml, opts).map_err(|e| e.to_string())?;
    // Основное тело — первое `<body>` без name="notes"/"comments".
    let body = doc
        .descendants()
        .find(|n| n.tag_name().name() == "body" && n.attribute("name").is_none())
        .ok_or("нет <body>")?;
    let sections: Vec<_> = body
        .children()
        .filter(|c| c.tag_name().name() == "section")
        .collect();
    let mut out = Vec::new();
    if sections.is_empty() {
        out.push(Chunk {
            label: "Текст".into(),
            text: node_text(body),
        });
    }
    for (i, s) in sections.iter().enumerate() {
        let title = s
            .children()
            .find(|c| c.tag_name().name() == "title")
            .map(node_text)
            .filter(|t| !t.is_empty());
        out.push(Chunk {
            label: title
                .map(|t| t.chars().take(80).collect())
                .unwrap_or_else(|| format!("Раздел {}", i + 1)),
            text: node_text(*s),
        });
    }
    Ok(out)
}

fn pdf_chunks(path: &Path) -> Result<Vec<Chunk>, String> {
    let bytes = std::fs::read(path).map_err(|e| e.to_string())?;
    // Разбор PDF чужой библиотекой может паниковать на повреждённых файлах — не роняем индексацию.
    let pages = std::panic::catch_unwind(|| pdf_extract::extract_text_from_mem_by_pages(&bytes))
        .map_err(|_| "не удалось разобрать PDF".to_string())?
        .map_err(|e| e.to_string())?;
    Ok(pages
        .into_iter()
        .enumerate()
        .map(|(i, t)| Chunk {
            label: format!("стр. {}", i + 1),
            text: t.split_whitespace().collect::<Vec<_>>().join(" "),
        })
        .filter(|c| !c.text.is_empty())
        .collect())
}

pub(crate) fn extract_chunks(path: &Path) -> Result<Vec<Chunk>, String> {
    let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
    match crate::formats::book_kind(name) {
        Some("pdf") => pdf_chunks(path),
        Some("epub") => epub_chunks(path),
        Some("fb2" | "fb2.zip") => fb2_chunks(path),
        Some("typ") => Ok(vec![Chunk {
            label: "Typst".into(),
            text: std::fs::read_to_string(path).map_err(|e| e.to_string())?,
        }]),
        _ => Ok(vec![]),
    }
}

// ——— Индексация ———

/// Служебные файлы Typst (темы оформления) — не книги, в поиске им не место.
fn searchable(path: &str) -> bool {
    !path.starts_with(".reader-typst-themes/") && !path.ends_with("/theme.typ")
}

fn library_books(root: &Path) -> Result<Vec<String>, String> {
    Ok(scan_library(root)?
        .into_iter()
        .filter(|p| searchable(p))
        .collect())
}

fn book_title(meta: &crate::model::LibraryMetadata, path: &str) -> String {
    meta.books
        .get(path)
        .and_then(|b| b.title.clone())
        .filter(|t| !t.trim().is_empty())
        .unwrap_or_else(|| path.rsplit('/').next().unwrap_or(path).to_string())
}

/// Заметки, цитаты и выделения книги одним документом.
fn notes_text(book: &crate::model::BookMeta) -> String {
    let mut parts: Vec<String> = Vec::new();
    for q in &book.quotes {
        parts.push(q.text.clone());
    }
    for c in &book.comments {
        parts.push(c.body.clone());
        parts.extend(c.excerpt.clone());
    }
    if let Some(hs) = book.extra.get("highlights").and_then(|v| v.as_array()) {
        for h in hs {
            for k in ["text", "note"] {
                if let Some(s) = h.get(k).and_then(|v| v.as_str()) {
                    parts.push(s.to_string());
                }
            }
        }
    }
    if !book.review.trim().is_empty() {
        parts.push(book.review.clone());
    }
    parts.join("\n")
}

fn emit(app: &AppHandle, p: IndexProgress) {
    let _ = app.emit("search-index-progress", p);
}

fn run_index(app: &AppHandle) -> Result<(), String> {
    let root = load_config(app)?
        .library_root
        .ok_or("Сначала выберите папку библиотеки")?;
    let root = PathBuf::from(root)
        .canonicalize()
        .map_err(|e| e.to_string())?;
    let paths = library_books(&root)?;
    let (meta, ids) = {
        let _g = META_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let meta = load_metadata(app)?;
        let ids: HashMap<String, String> = crate::storage::load_index(app)
            .files
            .into_iter()
            .map(|(p, f)| (p, f.id))
            .collect();
        (meta, ids)
    };
    let e = engine(app)?;
    let f = e.fields;
    let mut writer: IndexWriter = e
        .index
        .writer(64 * 1024 * 1024)
        .map_err(|e| format!("Поисковый индекс занят: {e}"))?;
    let mut state = read_state(app);
    state.version = SCHEMA_VERSION;

    // Отпечаток книги; если индекс файлов ещё не построен — размер и время изменения.
    let fingerprint = |p: &str| -> String {
        ids.get(p).cloned().unwrap_or_else(|| {
            std::fs::metadata(root.join(p))
                .map(|m| format!("{}:{:?}", m.len(), m.modified().ok()))
                .unwrap_or_default()
        })
    };
    let current: HashMap<&String, String> = paths.iter().map(|p| (p, fingerprint(p))).collect();
    let stale: Vec<String> = state
        .books
        .iter()
        .filter(|(p, id)| current.get(p).is_none_or(|cur| cur != *id))
        .map(|(p, _)| p.clone())
        .collect();
    for p in &stale {
        writer.delete_term(Term::from_field_text(f.path, p));
        state.books.remove(p);
    }
    let todo: Vec<&String> = paths
        .iter()
        .filter(|p| !state.books.contains_key(*p))
        .collect();
    let total = todo.len();

    for (i, &p) in todo.iter().enumerate() {
        // Явно &str: трейт `tantivy::schema::Value` тоже даёт `as_str()`.
        let p: &String = p;
        let ps: &str = p;
        emit(
            app,
            IndexProgress {
                done: i,
                total,
                current: book_title(&meta, p),
                finished: false,
                error: None,
            },
        );
        let title = book_title(&meta, p);
        // Неразборчивая книга индексируется пустой — повторно мучить её не будем.
        let full = root.join(ps);
        let chunks = match crate::formats::book_kind(ps) {
            Some("pdf") => extract_isolated(&full),
            _ => extract_chunks(&full),
        }
        .unwrap_or_default();
        for c in chunks {
            writer
                .add_document(doc!(
                    f.path => ps,
                    f.kind => "book",
                    f.title => title.as_str(),
                    f.label => c.label,
                    f.body => c.text,
                ))
                .map_err(|e| e.to_string())?;
        }
        state.books.insert(p.clone(), current[p].clone());
        if (i + 1) % COMMIT_EVERY == 0 {
            writer.commit().map_err(|e| e.to_string())?;
            write_state(app, &state);
        }
    }

    // Заметки меняются часто, а стоят дёшево — пересобираем целиком.
    writer.delete_term(Term::from_field_text(f.kind, "note"));
    for (path, book) in &meta.books {
        let text = notes_text(book);
        if text.trim().is_empty() {
            continue;
        }
        writer
            .add_document(doc!(
                f.path => path.as_str(),
                f.kind => "note",
                f.title => book_title(&meta, path),
                f.label => "Заметки и цитаты",
                f.body => text,
            ))
            .map_err(|e| e.to_string())?;
    }
    writer.commit().map_err(|e| e.to_string())?;
    write_state(app, &state);
    let _ = e.reader.reload();
    Ok(())
}

/// Запускает обновление индекса в фоне; ход — событие `search-index-progress`.
#[tauri::command]
#[specta::specta]
pub async fn search_reindex(app: AppHandle) -> Result<bool, String> {
    if RUNNING.swap(true, Ordering::SeqCst) {
        return Ok(false);
    }
    std::thread::spawn(move || {
        let result = run_index(&app);
        RUNNING.store(false, Ordering::SeqCst);
        emit(
            &app,
            IndexProgress {
                done: 0,
                total: 0,
                current: String::new(),
                finished: true,
                error: result.err(),
            },
        );
    });
    Ok(true)
}

#[tauri::command]
#[specta::specta]
pub async fn search_status(app: AppHandle) -> Result<SearchStatus, String> {
    let state = read_state(&app);
    let library_books = load_config(&app)?
        .library_root
        .map(|r| library_books(Path::new(&r)).map(|p| p.len()).unwrap_or(0))
        .unwrap_or(0);
    Ok(SearchStatus {
        indexed_books: if state.version == SCHEMA_VERSION {
            state.books.len()
        } else {
            0
        },
        library_books,
        running: RUNNING.load(Ordering::SeqCst),
    })
}

fn query_engine(e: &Engine, q: &str, limit: usize) -> Result<Vec<SearchHit>, String> {
    let f = e.fields;
    let searcher = e.reader.searcher();
    let mut parser = QueryParser::for_index(&e.index, vec![f.title, f.body]);
    parser.set_conjunction_by_default();
    parser.set_field_boost(f.title, 2.0);
    // Кавычки, минусы и прочее — можно, но опечатки в синтаксисе не роняют поиск.
    let (query, _errors) = parser.parse_query_lenient(q);
    let top = searcher
        .search(&query, &TopDocs::with_limit(limit).order_by_score())
        .map_err(|e| e.to_string())?;
    let mut snippets =
        SnippetGenerator::create(&searcher, &*query, f.body).map_err(|e| e.to_string())?;
    snippets.set_max_num_chars(240);
    let mut hits = Vec::with_capacity(top.len());
    for (_score, addr) in top {
        let d: TantivyDocument = searcher.doc(addr).map_err(|e| e.to_string())?;
        let s = |field: Field| {
            d.get_first(field)
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string()
        };
        let snippet = snippets.snippet_from_doc(&d);
        hits.push(SearchHit {
            path: s(f.path),
            kind: s(f.kind),
            title: s(f.title),
            label: s(f.label),
            snippet: if snippet.is_empty() {
                // Совпадение только в названии — показываем начало текста.
                let body = s(f.body);
                let start: String = body.chars().take(200).collect();
                tantivy_escape(&start)
            } else {
                snippet.to_html()
            },
        });
    }
    Ok(hits)
}

fn tantivy_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

#[tauri::command]
#[specta::specta]
pub async fn search_query(
    app: AppHandle,
    query: String,
    limit: Option<usize>,
) -> Result<Vec<SearchHit>, String> {
    let q = query.trim().to_string();
    if q.is_empty() {
        return Ok(vec![]);
    }
    let e = engine(&app)?;
    let limit = limit.unwrap_or(60).clamp(1, 300);
    tauri::async_runtime::spawn_blocking(move || query_engine(&e, &q, limit))
        .await
        .map_err(|e| e.to_string())?
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_engine(name: &str) -> (PathBuf, Engine) {
        let dir = std::env::temp_dir().join(format!("reader-search-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let e = open_engine_at(&dir).unwrap();
        (dir, e)
    }

    #[test]
    fn russian_morphology_and_safe_snippets() {
        let (dir, e) = temp_engine("morph");
        let f = e.fields;
        let mut w: IndexWriter = e.index.writer(15_000_000).unwrap();
        w.add_document(doc!(
            f.path => "tolstoy.fb2", f.kind => "book", f.title => "Война и мир",
            f.label => "Том 1",
            f.body => "Князь Андрей смотрел на высокое небо <script>alert(1)</script> над Аустерлицем.",
        ))
        .unwrap();
        w.add_document(doc!(
            f.path => "sicp.pdf", f.kind => "book", f.title => "Structure and Interpretation",
            f.label => "стр. 3", f.body => "Programs must be written for people to read.",
        ))
        .unwrap();
        w.commit().unwrap();
        e.reader.reload().unwrap();

        // Другая форма слова находит исходную.
        let hits = query_engine(&e, "небом", 10).unwrap();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].path, "tolstoy.fb2");
        assert!(
            hits[0].snippet.contains("<b>небо</b>"),
            "{}",
            hits[0].snippet
        );
        // Разметка из книги в сниппете экранирована.
        assert!(!hits[0].snippet.contains("<script>"));

        assert_eq!(
            query_engine(&e, "programming", 10).unwrap()[0].path,
            "sicp.pdf"
        );
        // Поиск по названию и битый синтаксис запроса.
        assert_eq!(
            query_engine(&e, "войны", 10).unwrap()[0].path,
            "tolstoy.fb2"
        );
        assert!(query_engine(&e, "небо AND (", 10).is_ok());
        drop(e);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn extracts_epub_and_fb2_text() {
        let dir = std::env::temp_dir().join(format!("reader-extract-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();

        let epub = dir.join("b.epub");
        {
            use std::io::Write;
            let mut z = zip::ZipWriter::new(std::fs::File::create(&epub).unwrap());
            let o = zip::write::SimpleFileOptions::default();
            z.start_file("mimetype", o).unwrap();
            z.write_all(b"application/epub+zip").unwrap();
            z.start_file("META-INF/container.xml", o).unwrap();
            z.write_all(br#"<container><rootfiles><rootfile full-path="OEBPS/c.opf"/></rootfiles></container>"#).unwrap();
            z.start_file("OEBPS/c.opf", o).unwrap();
            z.write_all(br#"<package><manifest><item id="a" href="Text/ch%201.xhtml"/><item id="b" href="Text/ch2.xhtml"/></manifest><spine><itemref idref="b"/><itemref idref="a"/></spine></package>"#).unwrap();
            z.start_file("OEBPS/Text/ch 1.xhtml", o).unwrap();
            z.write_all("<html><head><style>p{}</style></head><body><h2>Глава первая</h2><p>Жили&nbsp;были &laquo;дед&raquo;</p></body></html>".as_bytes()).unwrap();
            z.start_file("OEBPS/Text/ch2.xhtml", o).unwrap();
            z.write_all(b"<html><body><p>Preface</p></body></html>")
                .unwrap();
            z.finish().unwrap();
        }
        let c = extract_chunks(&epub).unwrap();
        assert_eq!(c.len(), 2);
        assert_eq!(c[0].text, "Preface"); // порядок spine, а не manifest
        assert_eq!(c[1].label, "Глава первая");
        assert!(c[1].text.contains("Жили были «дед»"), "{}", c[1].text);
        assert!(!c[1].text.contains("p{}"));

        let fb2 = dir.join("k.fb2");
        let xml = "<?xml version=\"1.0\" encoding=\"windows-1251\"?><FictionBook><body><section><title><p>Пролог</p></title><p>Тишина</p></section><section><p>Шум</p></section></body><body name=\"notes\"><section><p>сноска</p></section></body></FictionBook>";
        let (bytes, _, _) = encoding_rs::WINDOWS_1251.encode(xml);
        std::fs::write(&fb2, &bytes).unwrap();
        let c = extract_chunks(&fb2).unwrap();
        assert_eq!(c.len(), 2);
        assert_eq!(c[0].label, "Пролог");
        assert!(c[0].text.contains("Тишина"));
        assert_eq!(c[1].label, "Раздел 2");
        assert!(!c.iter().any(|x| x.text.contains("сноска")));
        let _ = std::fs::remove_dir_all(&dir);
    }
}

#[cfg(test)]
mod real_library {
    /// Только чтение: `READER_LIB=~/Книги cargo test -- --ignored extract_real_library --nocapture`.
    #[test]
    #[ignore]
    fn extract_real_library() {
        let Ok(root) = std::env::var("READER_LIB") else {
            return;
        };
        let root = std::path::PathBuf::from(root);
        for p in crate::library::scan_library(&root).unwrap() {
            let t = std::time::Instant::now();
            let r = super::extract_chunks(&root.join(&p));
            let (n, chars, first) = match &r {
                Ok(c) => (
                    c.len(),
                    c.iter().map(|x| x.text.chars().count()).sum::<usize>(),
                    c.first()
                        .map(|x| x.text.chars().take(60).collect::<String>())
                        .unwrap_or_default(),
                ),
                Err(e) => (0, 0, format!("ОШИБКА: {e}")),
            };
            println!(
                "{:>6} мс | {n:>4} фрагм. | {chars:>8} симв. | {p} | {first}",
                t.elapsed().as_millis()
            );
        }
    }
}

#[cfg(test)]
mod one_pdf {
    /// `READER_PDF=файл cargo test --release -- --ignored extract_one_pdf --nocapture`
    #[test]
    #[ignore]
    fn extract_one_pdf() {
        let Ok(p) = std::env::var("READER_PDF") else {
            return;
        };
        let t = std::time::Instant::now();
        let r = super::extract_chunks(std::path::Path::new(&p));
        println!("{} мс, {:?}", t.elapsed().as_millis(), r.map(|c| c.len()));
    }
}
