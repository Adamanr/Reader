//! OPDS-каталоги (Calibre, Project Gutenberg, Standard Ebooks, домашние библиотеки…):
//! просмотр разделов, поиск через OpenSearch и загрузка книг в библиотеку.
//!
//! Скачивать можно только то, что бэкенд сам получил из каталога: ссылки на файлы
//! запоминаются при разборе ленты, и `opds_import` принимает лишь их. Интерфейс не может
//! попросить загрузить в библиотеку произвольный адрес.

use crate::formats::{content_matches, existing_copy, sanitize_filename, unique_dest};
use crate::storage::load_config;
use reqwest::Url;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::Duration;
use tauri::{AppHandle, Emitter, State};

const MAX_FEED_BYTES: usize = 10 * 1024 * 1024;
const MAX_BOOK_BYTES: u64 = 1024 * 1024 * 1024;
const MAX_OFFERED: usize = 5000;
const USER_AGENT: &str = concat!("Reader/", env!("CARGO_PKG_VERSION"), " (OPDS)");

#[derive(Debug, Clone, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct OpdsAcquisition {
    pub url: String,
    /// `pdf`, `epub`, `fb2` или `fb2.zip`.
    pub ext: String,
    pub size: Option<u64>,
}

#[derive(Debug, Clone, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct OpdsEntry {
    pub id: String,
    pub title: String,
    pub authors: Vec<String>,
    pub summary: Option<String>,
    pub cover: Option<String>,
    /// Ссылка на вложенный раздел каталога.
    pub navigation: Option<String>,
    pub acquisitions: Vec<OpdsAcquisition>,
}

#[derive(Debug, Clone, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct OpdsFeed {
    pub url: String,
    pub title: String,
    pub entries: Vec<OpdsEntry>,
    pub next: Option<String>,
    /// Шаблон поиска с `{searchTerms}`.
    pub search_template: Option<String>,
}

#[derive(Debug, Clone)]
struct Offered {
    ext: String,
    title: String,
    authors: Vec<String>,
}

/// Ссылки на файлы, полученные из каталогов в этой сессии.
#[derive(Default)]
pub struct OpdsState {
    offered: Mutex<HashMap<String, Offered>>,
}

#[derive(Debug, Clone, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct OpdsImportItem {
    pub url: String,
}

#[derive(Debug, Clone, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct OpdsImportFailure {
    pub title: String,
    pub error: String,
}

#[derive(Debug, Clone, Serialize, Default, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct OpdsImportResult {
    pub added: Vec<String>,
    pub existing: Vec<String>,
    pub failed: Vec<OpdsImportFailure>,
}

#[derive(Debug, Clone, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct OpdsProgress {
    done: usize,
    total: usize,
    name: String,
}

fn client(timeout: Duration) -> Result<reqwest::Client, String> {
    reqwest::Client::builder()
        .user_agent(USER_AGENT)
        .connect_timeout(Duration::from_secs(20))
        .read_timeout(timeout)
        .build()
        .map_err(|e| e.to_string())
}

fn parse_http_url(raw: &str) -> Result<Url, String> {
    let url = Url::parse(raw.trim()).map_err(|_| "Некорректный адрес каталога".to_string())?;
    if !matches!(url.scheme(), "http" | "https") {
        return Err("Адрес каталога должен начинаться с http:// или https://".into());
    }
    Ok(url)
}

/// MIME-тип файла книги → расширение. Остальные форматы (mobi, txt…) читалка не открывает.
fn ext_from_mime(mime: &str) -> Option<&'static str> {
    let m = mime
        .split(';')
        .next()
        .unwrap_or("")
        .trim()
        .to_ascii_lowercase();
    match m.as_str() {
        "application/pdf" => Some("pdf"),
        "application/epub+zip" => Some("epub"),
        "application/fb2+xml"
        | "application/x-fictionbook+xml"
        | "text/fb2+xml"
        | "text/xml+fb2" => Some("fb2"),
        "application/fb2+zip"
        | "application/x-zip-compressed-fb2"
        | "application/x-fictionbook+zip" => Some("fb2.zip"),
        _ => None,
    }
}

fn child<'a, 'i>(node: roxmltree::Node<'a, 'i>, name: &str) -> Option<roxmltree::Node<'a, 'i>> {
    node.children().find(|c| c.tag_name().name() == name)
}

fn text_of(node: Option<roxmltree::Node>) -> String {
    node.map(|n| {
        n.descendants()
            .filter(|d| d.is_text())
            .filter_map(|d| d.text())
            .collect::<String>()
    })
    .unwrap_or_default()
    .split_whitespace()
    .collect::<Vec<_>>()
    .join(" ")
}

/// Описание может прийти HTML-строкой внутри XML — оставляем только текст.
fn strip_html(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut in_tag = false;
    for c in s.chars() {
        match c {
            '<' => in_tag = true,
            '>' => {
                in_tag = false;
                out.push(' ');
            }
            _ if !in_tag => out.push(c),
            _ => {}
        }
    }
    let out = out
        .replace("&nbsp;", " ")
        .replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"");
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

struct Link {
    rel: String,
    mime: String,
    href: String,
    length: Option<u64>,
}

fn links(node: roxmltree::Node, base: &Url) -> Vec<Link> {
    node.children()
        .filter(|c| c.tag_name().name() == "link")
        .filter_map(|l| {
            let href = base.join(l.attribute("href")?).ok()?;
            // data: — только для картинок (обложки), остальное — только http(s).
            let ok = match href.scheme() {
                "http" | "https" => true,
                "data" => href.as_str().starts_with("data:image/"),
                _ => false,
            };
            ok.then(|| Link {
                rel: l.attribute("rel").unwrap_or("").to_string(),
                mime: l.attribute("type").unwrap_or("").to_ascii_lowercase(),
                href: href.to_string(),
                length: l.attribute("length").and_then(|v| v.parse().ok()),
            })
        })
        .collect()
}

fn is_atom(mime: &str) -> bool {
    mime.contains("atom+xml")
}

fn parse_entry(e: roxmltree::Node, base: &Url) -> OpdsEntry {
    let ls = links(e, base);
    let mut acquisitions: Vec<OpdsAcquisition> = Vec::new();
    for l in &ls {
        let acq = l.rel == "http://opds-spec.org/acquisition"
            || l.rel == "http://opds-spec.org/acquisition/open-access"
            || (l.rel.is_empty() && ext_from_mime(&l.mime).is_some());
        if l.href.starts_with("data:") {
            continue;
        }
        if let Some(ext) = acq.then(|| ext_from_mime(&l.mime)).flatten() {
            // Несколько вариантов одного формата (с картинками и без) — берём самый полный.
            match acquisitions.iter_mut().find(|a| a.ext == ext) {
                Some(a) if l.length > a.size => {
                    a.url = l.href.clone();
                    a.size = l.length;
                }
                Some(_) => {}
                None => acquisitions.push(OpdsAcquisition {
                    url: l.href.clone(),
                    ext: ext.into(),
                    size: l.length,
                }),
            }
        }
    }
    let cover = [
        "http://opds-spec.org/image/thumbnail",
        "http://opds-spec.org/thumbnail",
        "x-stanza-cover-image-thumbnail",
        "http://opds-spec.org/image",
        "http://opds-spec.org/cover",
        "x-stanza-cover-image",
    ]
    .iter()
    .find_map(|rel| ls.iter().find(|l| l.rel == *rel))
    .map(|l| l.href.clone());
    let navigation = if acquisitions.is_empty() {
        ls.iter()
            .filter(|l| !l.href.starts_with("data:"))
            .filter(|l| {
                is_atom(&l.mime) && !matches!(l.rel.as_str(), "self" | "alternate" | "related")
            })
            .min_by_key(|l| (l.rel != "subsection" && !l.rel.is_empty()) as u8)
            .map(|l| l.href.clone())
    } else {
        None
    };
    let summary = [child(e, "summary"), child(e, "content")]
        .into_iter()
        .map(|n| strip_html(&text_of(n)))
        .find(|s| !s.is_empty())
        .map(|s| s.chars().take(600).collect());
    OpdsEntry {
        id: text_of(child(e, "id")),
        title: text_of(child(e, "title")),
        authors: e
            .children()
            .filter(|c| c.tag_name().name() == "author")
            .map(|a| text_of(child(a, "name")))
            .filter(|n| !n.is_empty())
            .collect(),
        summary,
        cover,
        navigation,
        acquisitions,
    }
}

/// Лента OPDS (Atom). `search` — ссылка поиска как есть: шаблон или описание OpenSearch.
fn parse_feed(xml: &str, url: &Url) -> Result<(OpdsFeed, Option<(String, String)>), String> {
    let doc = roxmltree::Document::parse(xml)
        .map_err(|_| "Это не каталог OPDS (ответ не является XML)".to_string())?;
    let root = doc.root_element();
    if root.tag_name().name() != "feed" {
        return Err("Это не каталог OPDS (нет ленты Atom)".into());
    }
    let ls = links(root, url);
    let next = ls
        .iter()
        .find(|l| l.rel == "next" && !l.href.starts_with("data:"))
        .map(|l| l.href.clone());
    let search = ls
        .iter()
        .find(|l| l.rel == "search")
        .map(|l| (l.href.clone(), l.mime.clone()));
    let entries = root
        .children()
        .filter(|c| c.tag_name().name() == "entry")
        .map(|e| parse_entry(e, url))
        .filter(|e| !e.title.is_empty())
        .collect();
    Ok((
        OpdsFeed {
            url: url.to_string(),
            title: text_of(child(root, "title")),
            entries,
            next,
            search_template: None,
        },
        search,
    ))
}

/// Шаблон из описания OpenSearch: `<Url type="application/atom+xml" template="…"/>`.
fn parse_opensearch(xml: &str, base: &Url) -> Option<String> {
    let doc = roxmltree::Document::parse(xml).ok()?;
    doc.descendants()
        .filter(|n| n.tag_name().name() == "Url")
        .filter(|n| n.attribute("type").is_some_and(is_atom))
        .find_map(|n| n.attribute("template"))
        .map(|t| {
            base.join(t)
                .map(|u| u.to_string())
                .unwrap_or_else(|_| t.to_string())
        })
        // `Url::join` экранирует фигурные скобки — возвращаем плейсхолдер.
        .map(|t| t.replace("%7BsearchTerms%7D", "{searchTerms}"))
}

async fn get_text(client: &reqwest::Client, url: &Url) -> Result<String, String> {
    let resp = client
        .get(url.clone())
        .header(
            "Accept",
            "application/atom+xml, application/xml;q=0.9, */*;q=0.5",
        )
        .send()
        .await
        .map_err(|e| format!("Каталог недоступен: {e}"))?;
    let status = resp.status();
    if status == reqwest::StatusCode::UNAUTHORIZED {
        return Err(
            "Каталог требует вход по паролю — такие каталоги пока не поддерживаются".into(),
        );
    }
    if !status.is_success() {
        return Err(format!("Каталог ответил: HTTP {status}"));
    }
    if resp
        .content_length()
        .is_some_and(|n| n as usize > MAX_FEED_BYTES)
    {
        return Err("Ответ каталога слишком большой".into());
    }
    let bytes = resp.bytes().await.map_err(|e| e.to_string())?;
    if bytes.len() > MAX_FEED_BYTES {
        return Err("Ответ каталога слишком большой".into());
    }
    Ok(String::from_utf8_lossy(&bytes).into_owned())
}

#[tauri::command]
#[specta::specta]
pub async fn opds_fetch(state: State<'_, OpdsState>, url: String) -> Result<OpdsFeed, String> {
    let url = parse_http_url(&url)?;
    let client = client(Duration::from_secs(60))?;
    let xml = get_text(&client, &url).await?;
    let (mut feed, search) = parse_feed(&xml, &url)?;
    feed.search_template = match search {
        Some((href, mime))
            if href.contains("searchTerms")
                || href.contains("%7BsearchTerms%7D")
                || is_atom(&mime) =>
        {
            Some(href.replace("%7BsearchTerms%7D", "{searchTerms}"))
        }
        Some((href, _)) => match Url::parse(&href) {
            Ok(desc) => get_text(&client, &desc)
                .await
                .ok()
                .and_then(|x| parse_opensearch(&x, &desc)),
            Err(_) => None,
        },
        None => None,
    }
    .filter(|t| t.contains("{searchTerms}"));

    let mut offered = state.offered.lock().unwrap_or_else(|e| e.into_inner());
    if offered.len() > MAX_OFFERED {
        offered.clear();
    }
    for e in &feed.entries {
        for a in &e.acquisitions {
            offered.insert(
                a.url.clone(),
                Offered {
                    ext: a.ext.clone(),
                    title: e.title.clone(),
                    authors: e.authors.clone(),
                },
            );
        }
    }
    Ok(feed)
}

/// `attachment; filename*=UTF-8''%D0%9A….epub` или `filename="x.epub"`.
fn filename_from_disposition(v: &str) -> Option<String> {
    let mut plain = None;
    for part in v.split(';').map(str::trim) {
        if let Some(enc) = part.strip_prefix("filename*=") {
            let raw = enc.rsplit("''").next().unwrap_or(enc).trim_matches('"');
            if let Ok(s) = percent_decode(raw) {
                return Some(s);
            }
        } else if let Some(p) = part.strip_prefix("filename=") {
            plain = Some(p.trim_matches('"').to_string());
        }
    }
    plain
}

fn percent_decode(s: &str) -> Result<String, ()> {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%' && i + 2 < b.len() {
            let hex = std::str::from_utf8(&b[i + 1..i + 3]).map_err(|_| ())?;
            out.push(u8::from_str_radix(hex, 16).map_err(|_| ())?);
            i += 3;
        } else {
            out.push(b[i]);
            i += 1;
        }
    }
    String::from_utf8(out).map_err(|_| ())
}

fn book_name(offered: &Offered, disposition: Option<String>) -> String {
    if let Some(name) =
        disposition.filter(|n| crate::formats::book_kind(n) == Some(offered.ext.as_str()))
    {
        return name;
    }
    let author = offered
        .authors
        .first()
        .map(|a| format!("{a} — "))
        .unwrap_or_default();
    let base: String = format!("{author}{}", offered.title)
        .chars()
        .take(150)
        .collect();
    format!("{}.{}", base.trim(), offered.ext)
}

async fn download_one(
    client: &reqwest::Client,
    root: &Path,
    url: &str,
    offered: &Offered,
) -> Result<(PathBuf, bool), String> {
    let resp = client
        .get(url)
        .send()
        .await
        .map_err(|e| format!("Загрузка: {e}"))?;
    if !resp.status().is_success() {
        return Err(format!("Сервер ответил: HTTP {}", resp.status()));
    }
    if resp.content_length().is_some_and(|n| n > MAX_BOOK_BYTES) {
        return Err("Файл больше 1 ГБ".into());
    }
    let disposition = resp
        .headers()
        .get(reqwest::header::CONTENT_DISPOSITION)
        .and_then(|v| v.to_str().ok())
        .and_then(filename_from_disposition);
    let hash: String = Sha256::digest(url.as_bytes())[..8]
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();
    let tmp = root.join(format!(".opds-{hash}.part"));
    let result = write_body(resp, &tmp).await;
    let finish = || -> Result<(PathBuf, bool), String> {
        let size = result.clone()?;
        let mut kind = offered.ext.clone();
        if !content_matches(&tmp, &kind) {
            // Некоторые каталоги отдают FB2 упакованным, не меняя тип ссылки.
            if kind == "fb2" && content_matches(&tmp, "fb2.zip") {
                kind = "fb2.zip".into();
            } else {
                return Err(format!("Содержимое не похоже на {}", kind.to_uppercase()));
            }
        }
        let offered = Offered {
            ext: kind,
            ..offered.clone()
        };
        let name = sanitize_filename(&book_name(&offered, disposition.clone()));
        if let Some(p) = existing_copy(root, &name, size) {
            return Ok((p, false));
        }
        let dest = unique_dest(root, &name);
        std::fs::rename(&tmp, &dest).map_err(|e| format!("Сохранение файла: {e}"))?;
        Ok((dest, true))
    };
    let out = finish();
    let _ = std::fs::remove_file(&tmp);
    out
}

async fn write_body(mut resp: reqwest::Response, tmp: &Path) -> Result<u64, String> {
    use tokio::io::AsyncWriteExt;
    let mut file = tokio::fs::File::create(tmp)
        .await
        .map_err(|e| e.to_string())?;
    let mut size = 0u64;
    while let Some(chunk) = resp.chunk().await.map_err(|e| format!("Загрузка: {e}"))? {
        size += chunk.len() as u64;
        if size > MAX_BOOK_BYTES {
            return Err("Файл больше 1 ГБ".into());
        }
        file.write_all(&chunk).await.map_err(|e| e.to_string())?;
    }
    file.flush().await.map_err(|e| e.to_string())?;
    Ok(size)
}

/// Скачивает выбранные книги в корень библиотеки. Прогресс — событие `opds-import-progress`.
#[tauri::command]
#[specta::specta]
pub async fn opds_import(
    app: AppHandle,
    state: State<'_, OpdsState>,
    items: Vec<OpdsImportItem>,
) -> Result<OpdsImportResult, String> {
    let root = load_config(&app)?
        .library_root
        .ok_or("Сначала выберите папку библиотеки")?;
    let root = PathBuf::from(root)
        .canonicalize()
        .map_err(|e| e.to_string())?;
    let client = client(Duration::from_secs(120))?;
    let mut result = OpdsImportResult::default();
    let total = items.len();
    for (done, item) in items.iter().enumerate() {
        let offered = state
            .offered
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .get(&item.url)
            .cloned();
        let Some(offered) = offered else {
            result.failed.push(OpdsImportFailure {
                title: item.url.clone(),
                error: "Ссылка не из открытого каталога — обновите страницу каталога".into(),
            });
            continue;
        };
        let _ = app.emit(
            "opds-import-progress",
            OpdsProgress {
                done,
                total,
                name: offered.title.clone(),
            },
        );
        let rel = |p: &Path| {
            p.strip_prefix(&root)
                .unwrap_or(p)
                .to_string_lossy()
                .replace('\\', "/")
        };
        match download_one(&client, &root, &item.url, &offered).await {
            Ok((p, true)) => result.added.push(rel(&p)),
            Ok((p, false)) => result.existing.push(rel(&p)),
            Err(error) => result.failed.push(OpdsImportFailure {
                title: offered.title.clone(),
                error,
            }),
        }
    }
    let _ = app.emit(
        "opds-import-progress",
        OpdsProgress {
            done: total,
            total,
            name: String::new(),
        },
    );
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    const FEED: &str = r#"<?xml version="1.0" encoding="utf-8"?>
<feed xmlns="http://www.w3.org/2005/Atom" xmlns:opds="http://opds-spec.org/2010/catalog">
  <title>Каталог</title>
  <link rel="search" type="application/opensearchdescription+xml" href="/opds/search.xml"/>
  <link rel="next" type="application/atom+xml;profile=opds-catalog" href="?page=2"/>
  <entry>
    <title>Новинки</title>
    <id>tag:new</id>
    <link rel="subsection" type="application/atom+xml;profile=opds-catalog;kind=acquisition" href="/opds/new"/>
  </entry>
  <entry>
    <title>Мастер и Маргарита</title>
    <id>tag:book:1</id>
    <author><name>Михаил Булгаков</name></author>
    <content type="text/html">&lt;p&gt;Роман &amp;amp; мистика&lt;/p&gt;</content>
    <link rel="http://opds-spec.org/image/thumbnail" type="image/jpeg" href="/covers/1.jpg"/>
    <link rel="http://opds-spec.org/acquisition/open-access" type="application/fb2+zip" href="/b/1/fb2"/>
    <link rel="http://opds-spec.org/acquisition/open-access" type="application/epub+zip" href="/b/1/epub"/>
    <link rel="http://opds-spec.org/acquisition/open-access" type="application/x-mobipocket-ebook" href="/b/1/mobi"/>
    <link rel="related" type="application/atom+xml" href="/opds/author/7"/>
  </entry>
</feed>"#;

    #[test]
    fn parses_navigation_and_books() {
        let base = Url::parse("https://lib.example/opds/root").unwrap();
        let (feed, search) = parse_feed(FEED, &base).unwrap();
        assert_eq!(feed.title, "Каталог");
        assert_eq!(
            feed.next.as_deref(),
            Some("https://lib.example/opds/root?page=2")
        );
        assert_eq!(search.unwrap().0, "https://lib.example/opds/search.xml");

        let nav = &feed.entries[0];
        assert_eq!(
            nav.navigation.as_deref(),
            Some("https://lib.example/opds/new")
        );
        assert!(nav.acquisitions.is_empty());

        let book = &feed.entries[1];
        assert_eq!(book.authors, ["Михаил Булгаков"]);
        assert_eq!(book.summary.as_deref(), Some("Роман & мистика"));
        assert_eq!(
            book.cover.as_deref(),
            Some("https://lib.example/covers/1.jpg")
        );
        let exts: Vec<&str> = book.acquisitions.iter().map(|a| a.ext.as_str()).collect();
        assert_eq!(exts, ["fb2.zip", "epub"]); // mobi читалка не открывает
        assert!(book.navigation.is_none());
    }

    #[test]
    fn reads_opensearch_template() {
        let xml = r#"<OpenSearchDescription xmlns="http://a9.com/-/spec/opensearch/1.1/">
          <Url type="text/html" template="/search?q={searchTerms}"/>
          <Url type="application/atom+xml;profile=opds-catalog" template="/opds/search?q={searchTerms}&amp;p={startPage?}"/>
        </OpenSearchDescription>"#;
        let base = Url::parse("https://lib.example/opds/search.xml").unwrap();
        let t = parse_opensearch(xml, &base).unwrap();
        assert!(
            t.starts_with("https://lib.example/opds/search?q={searchTerms}"),
            "{t}"
        );
    }

    #[test]
    fn rejects_non_feeds_and_bad_urls() {
        let base = Url::parse("https://x.example/").unwrap();
        assert!(parse_feed("<html><body>hi</body></html>", &base).is_err());
        assert!(parse_feed("not xml", &base).is_err());
        assert!(parse_http_url("file:///etc/passwd").is_err());
        assert!(parse_http_url("javascript:alert(1)").is_err());
    }

    #[test]
    fn names_downloads() {
        assert_eq!(
            filename_from_disposition(
                "attachment; filename*=UTF-8''%D0%9A%D0%BD%D0%B8%D0%B3%D0%B0.epub"
            ),
            Some("Книга.epub".into())
        );
        assert_eq!(
            filename_from_disposition("attachment; filename=\"a.pdf\""),
            Some("a.pdf".into())
        );
        let o = Offered {
            ext: "epub".into(),
            title: "Мастер и Маргарита".into(),
            authors: vec!["Булгаков".into()],
        };
        assert_eq!(
            book_name(&o, Some("x.pdf".into())),
            "Булгаков — Мастер и Маргарита.epub"
        );
        assert_eq!(book_name(&o, Some("mm.epub".into())), "mm.epub");
    }
}

#[cfg(test)]
mod live {
    use super::*;

    /// Живые каталоги; запуск вручную: `cargo test -- --ignored opds_live`.
    #[tokio::test]
    #[ignore]
    async fn opds_live() {
        let c = client(Duration::from_secs(60)).unwrap();
        for root in [
            "https://www.gutenberg.org/ebooks.opds/",
            "https://standardebooks.org/feeds/opds",
        ] {
            let url = Url::parse(root).unwrap();
            let xml = match get_text(&c, &url).await {
                Ok(x) => x,
                Err(e) => {
                    println!("{root}: {e}");
                    continue;
                }
            };
            let (feed, search) = parse_feed(&xml, &url).unwrap();
            println!(
                "{root}: «{}», {} записей, search={search:?}",
                feed.title,
                feed.entries.len()
            );
            for e in feed.entries.iter().take(4) {
                println!(
                    "  - {} | nav={:?} | acq={:?}",
                    e.title,
                    e.navigation,
                    e.acquisitions.iter().map(|a| &a.ext).collect::<Vec<_>>()
                );
            }
            if let Some((href, _)) = search {
                if let Ok(d) = Url::parse(&href) {
                    if let Ok(x) = get_text(&c, &d).await {
                        println!("  search template: {:?}", parse_opensearch(&x, &d));
                    }
                }
            }
        }
        // Книга с файлами и настоящая загрузка во временную «библиотеку».
        let book_url = Url::parse("https://www.gutenberg.org/ebooks/2600.opds").unwrap();
        let (book, _) = parse_feed(&get_text(&c, &book_url).await.unwrap(), &book_url).unwrap();
        // Gutenberg отдаёт «с картинками» и «без» отдельными записями; внутри записи
        // из двух EPUB должен выбраться более полный.
        let entry = book
            .entries
            .iter()
            .find(|e| e.acquisitions.iter().any(|a| a.url.ends_with(".images")))
            .unwrap();
        println!("{} — {:?}", entry.title, entry.acquisitions);
        let epub = entry.acquisitions.iter().find(|a| a.ext == "epub").unwrap();
        assert!(epub.url.ends_with("2600.epub.images"), "{}", epub.url);
        let root = std::env::temp_dir().join(format!("reader-opds-live-{}", std::process::id()));
        std::fs::create_dir_all(&root).unwrap();
        let offered = Offered {
            ext: "epub".into(),
            title: entry.title.clone(),
            authors: entry.authors.clone(),
        };
        let (path, added) = download_one(&c, &root, &epub.url, &offered).await.unwrap();
        println!(
            "скачано: {} ({} байт)",
            path.display(),
            std::fs::metadata(&path).unwrap().len()
        );
        assert!(added && content_matches(&path, "epub"));
        // Повторная загрузка того же файла распознаётся как уже имеющаяся.
        let (_, again) = download_one(&c, &root, &epub.url, &offered).await.unwrap();
        assert!(!again);
        let _ = std::fs::remove_dir_all(&root);
    }
}
