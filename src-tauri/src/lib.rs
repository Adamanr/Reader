use base64::{engine::general_purpose::STANDARD, Engine as _};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use tauri::AppHandle;
use walkdir::WalkDir;

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Shelf {
    pub id: String,
    pub name: String,
    pub order: i32,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct BookComment {
    pub id: String,
    pub body: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub page: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub chapter_label: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub excerpt: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct SavedQuote {
    pub id: String,
    pub text: String,
    pub created_at: String,
    pub accent: String,
    pub layout: String,
    #[serde(default)]
    pub include_page: bool,
    #[serde(default)]
    pub include_chapter: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub book_title: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub book_author: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bg_image_data_url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bg_image_opacity: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub overlay_color: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub overlay_opacity: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bg_scale: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bg_fit: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct BookMeta {
    pub path: String,
    pub shelf_id: String,
    #[serde(default)]
    pub shelf_ids: Vec<String>,
    pub importance: String,
    pub review: String,
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub author: Option<String>,
    #[serde(default)]
    pub comments: Vec<BookComment>,
    #[serde(default)]
    pub quotes: Vec<SavedQuote>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_read_pdf_page: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_read_pdf_total: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_read_location: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_read_location_label: Option<String>,
    #[serde(default)]
    pub translation_exported: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cover_thumb_data_url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_opened_at: Option<String>,
    /// Путь к файлу стиля `.typ` относительно корня библиотеки; None — общий стиль из настроек.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub typst_style_relative_path: Option<String>,
}


#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct LibraryMetadata {
    pub shelves: Vec<Shelf>,
    pub books: HashMap<String, BookMeta>,
}

#[derive(Debug, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct AppConfig {
    pub library_root: Option<String>,
    /// Путь к `.typ` теме по умолчанию (относительно папки библиотеки), например `.reader-typst-themes/minimal.typ`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default_typst_style_relative_path: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LibrarySnapshot {
    pub library_root: Option<String>,
    pub book_paths: Vec<String>,
    pub metadata: LibraryMetadata,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default_typst_style_relative_path: Option<String>,
}

fn app_dir(_app: &AppHandle) -> PathBuf {
    let dir = dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("com.adaman.reader");
    let _ = std::fs::create_dir_all(&dir);
    dir
}

fn pdf_translations_dir(app: &AppHandle) -> PathBuf {
    let d = app_dir(app).join("pdf-translations");
    let _ = std::fs::create_dir_all(&d);
    d
}

fn pdf_translation_file(app: &AppHandle, book_relative_path: &str) -> PathBuf {
    let mut h = Sha256::new();
    h.update(book_relative_path.as_bytes());
    let hash = format!("{:x}", h.finalize());
    pdf_translations_dir(app).join(format!("{}.json", hash))
}

fn config_path(app: &AppHandle) -> PathBuf {
    app_dir(app).join("config.json")
}

fn metadata_path(app: &AppHandle) -> PathBuf {
    app_dir(app).join("library-metadata.json")
}

fn load_config(app: &AppHandle) -> Result<AppConfig, String> {
    let p = config_path(app);
    if !p.exists() {
        return Ok(AppConfig::default());
    }
    let s = std::fs::read_to_string(&p).map_err(|e| e.to_string())?;
    serde_json::from_str(&s).map_err(|e| e.to_string())
}

fn save_config(app: &AppHandle, c: &AppConfig) -> Result<(), String> {
    let p = config_path(app);
    let s = serde_json::to_string_pretty(c).map_err(|e| e.to_string())?;
    std::fs::write(&p, s).map_err(|e| e.to_string())
}

fn default_metadata() -> LibraryMetadata {
    LibraryMetadata {
        shelves: vec![Shelf {
            id: "default".into(),
            name: "Общая полка".into(),
            order: 0,
        }],
        books: HashMap::new(),
    }
}

fn load_metadata(app: &AppHandle) -> Result<LibraryMetadata, String> {
    let p = metadata_path(app);
    if !p.exists() {
        return Ok(default_metadata());
    }
    let s = std::fs::read_to_string(&p).map_err(|e| e.to_string())?;
    let mut m: LibraryMetadata = serde_json::from_str(&s).map_err(|e| e.to_string())?;
    if m.shelves.is_empty() {
        m.shelves = default_metadata().shelves;
    }
    Ok(m)
}

fn save_metadata(app: &AppHandle, m: &LibraryMetadata) -> Result<(), String> {
    let p = metadata_path(app);
    let s = serde_json::to_string_pretty(m).map_err(|e| e.to_string())?;
    std::fs::write(&p, s).map_err(|e| e.to_string())
}

/// Безопасный путь под корнем библиотеки. Цель может ещё не существовать (экспорт Typst и т.д.),
/// поэтому нельзя вызывать `canonicalize()` для `joined` — иначе OS error 2.
fn safe_join(root: &Path, rel: &str) -> Result<PathBuf, String> {
    let root = root.canonicalize().map_err(|e| e.to_string())?;
    let rel = rel.trim().trim_start_matches(['/', '\\']);
    let mut out = root.clone();
    for seg in rel
        .split(|c| c == '/' || c == '\\')
        .filter(|s| !s.is_empty())
    {
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

/// Запись через временный файл и rename — не остаётся обрубка размером 0 при обрыве записи.
fn atomic_write(path: &Path, bytes: &[u8]) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let parent = path.parent().ok_or_else(|| "Некорректный путь".to_string())?;
    let name = path
        .file_name()
        .and_then(|s| s.to_str())
        .ok_or_else(|| "Некорректное имя файла".to_string())?;
    let tmp = parent.join(format!(".{name}.part.{}" , std::process::id()));
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

fn scan_library(root: &Path) -> Result<Vec<String>, String> {
    if !root.exists() {
        return Err("Папка библиотеки не найдена".into());
    }
    let root = root.canonicalize().map_err(|e| e.to_string())?;
    let mut out = Vec::new();
    for entry in WalkDir::new(&root).into_iter().filter_map(|e| e.ok()) {
        let p = entry.path();
        if !p.is_file() {
            continue;
        }
        let Some(ext) = p.extension().and_then(|s| s.to_str()) else {
            continue;
        };
        match ext.to_lowercase().as_str() {
            "pdf" | "epub" | "fb2" | "typ" => {
                let rel = p.strip_prefix(&root).map_err(|e| e.to_string())?;
                out.push(rel.to_string_lossy().replace('\\', "/"));
            }
            _ => {}
        }
    }
    out.sort();
    Ok(out)
}

fn merge_scan_into_metadata(paths: &[String], meta: &mut LibraryMetadata) {
    for p in paths {
        meta.books.entry(p.clone()).or_insert_with(|| BookMeta {
            path: p.clone(),
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
        });
    }
    let set: HashSet<_> = paths.iter().cloned().collect();
    meta.books.retain(|k, _| set.contains(k));
}

#[tauri::command]
fn get_library_snapshot(app: AppHandle) -> Result<LibrarySnapshot, String> {
    let config = load_config(&app)?;
    let root_str = config.library_root.clone();
    let scan_result = root_str
        .as_ref()
        .map(|r| scan_library(Path::new(r)));
    let paths = match scan_result {
        None => vec![],
        Some(Ok(p)) => p,
        Some(Err(_e)) => {
            let metadata = load_metadata(&app)?;
            let default_typst_style_relative_path = load_config(&app)
                .ok()
                .and_then(|c| c.default_typst_style_relative_path);
            return Ok(LibrarySnapshot {
                library_root: root_str,
                book_paths: vec![],
                metadata,
                default_typst_style_relative_path,
            });
        }
    };
    let mut metadata = load_metadata(&app)?;
    merge_scan_into_metadata(&paths, &mut metadata);
    save_metadata(&app, &metadata)?;
    let default_typst_style_relative_path = load_config(&app)
        .ok()
        .and_then(|c| c.default_typst_style_relative_path);
    Ok(LibrarySnapshot {
        library_root: root_str,
        book_paths: paths,
        metadata,
        default_typst_style_relative_path,
    })
}

#[tauri::command]
fn set_library_root(app: AppHandle, path: String) -> Result<(), String> {
    let p = PathBuf::from(&path);
    if !p.is_dir() {
        return Err("Укажите существующую папку".into());
    }
    let canon = p.canonicalize().map_err(|e| e.to_string())?;
    let mut c = load_config(&app)?;
    c.library_root = Some(canon.to_string_lossy().to_string());
    save_config(&app, &c)
}

#[tauri::command]
fn save_library_metadata(app: AppHandle, metadata: LibraryMetadata) -> Result<(), String> {
    save_metadata(&app, &metadata)
}

#[derive(Debug, Deserialize)]
struct LibreTranslateOk {
    #[serde(rename = "translatedText")]
    translated_text: String,
}

#[tauri::command]
async fn translate_texts(
    texts: Vec<String>,
    source: String,
    target: String,
    api_base: String,
    api_key: Option<String>,
) -> Result<Vec<String>, String> {
    let base = api_base.trim().trim_end_matches('/').to_string();
    if base.is_empty() {
        return Err("Укажите URL сервера перевода (LibreTranslate) в настройках.".into());
    }
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(120))
        .build()
        .map_err(|e| e.to_string())?;

    let url = format!("{}/translate", base);
    let mut out: Vec<String> = Vec::with_capacity(texts.len());

    for t in texts {
        let mut body = serde_json::json!({
            "q": t,
            "source": source,
            "target": target,
            "format": "text",
        });
        if let Some(ref k) = api_key {
            let ks = k.trim();
            if !ks.is_empty() {
                body["api_key"] = serde_json::json!(ks);
            }
        }

        let resp = client
            .post(&url)
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await
            .map_err(|e| format!("Запрос перевода: {}", e))?;

        let status = resp.status();
        let raw = resp.text().await.map_err(|e| e.to_string())?;
        if !status.is_success() {
            let short: String = raw.chars().take(280).collect();
            return Err(format!("Перевод: HTTP {} — {}", status, short));
        }

        let parsed: LibreTranslateOk =
            serde_json::from_str(&raw).map_err(|e| format!("Разбор ответа ({}): {}", e, raw.chars().take(160).collect::<String>()))?;
        out.push(parsed.translated_text);
    }

    Ok(out)
}

#[tauri::command]
fn read_book_base64(app: AppHandle, relative_path: String) -> Result<String, String> {
    let config = load_config(&app)?;
    let root = config
        .library_root
        .ok_or("Сначала выберите папку библиотеки")?;
    let root = PathBuf::from(root);
    let full = safe_join(&root, &relative_path)?;
    let bytes = std::fs::read(&full).map_err(|e| e.to_string())?;
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
fn save_translated_pdf_next_to_source(
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
    let mut meta = load_metadata(&app)?;
    if let Some(b) = meta.books.get_mut(&source_relative_path) {
        b.translation_exported = true;
    }
    save_metadata(&app, &meta)?;
    Ok(new_rel)
}

/// Запись произвольного файла (путь из диалога сохранения).
#[tauri::command]
fn write_file_base64(path: String, contents_base64: String) -> Result<(), String> {
    let bytes = STANDARD
        .decode(contents_base64.trim())
        .map_err(|e| e.to_string())?;
    let p = PathBuf::from(path);
    atomic_write(&p, &bytes)
}

#[tauri::command]
fn pdf_translation_load(app: AppHandle, book_relative_path: String) -> Result<Option<String>, String> {
    let p = pdf_translation_file(&app, &book_relative_path);
    if !p.exists() {
        return Ok(None);
    }
    let s = std::fs::read_to_string(&p).map_err(|e| e.to_string())?;
    Ok(Some(s))
}

#[tauri::command]
fn pdf_translation_save(app: AppHandle, book_relative_path: String, json: String) -> Result<(), String> {
    let p = pdf_translation_file(&app, &book_relative_path);
    if let Some(parent) = p.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    std::fs::write(&p, json).map_err(|e| e.to_string())
}

#[tauri::command]
fn pdf_translation_delete(app: AppHandle, book_relative_path: String) -> Result<(), String> {
    let p = pdf_translation_file(&app, &book_relative_path);
    if p.exists() {
        std::fs::remove_file(&p).map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[tauri::command]
fn read_library_utf8(app: AppHandle, relative_path: String) -> Result<String, String> {
    let config = load_config(&app)?;
    let root = config
        .library_root
        .ok_or("Сначала выберите папку библиотеки")?;
    let root = PathBuf::from(root);
    let full = safe_join(&root, &relative_path)?;
    std::fs::read_to_string(&full).map_err(|e| e.to_string())
}

#[tauri::command]
fn write_library_utf8(app: AppHandle, relative_path: String, content: String) -> Result<(), String> {
    let config = load_config(&app)?;
    let root = config
        .library_root
        .ok_or("Сначала выберите папку библиотеки")?;
    let root = PathBuf::from(root);
    let full = safe_join(&root, &relative_path)?;
    atomic_write(&full, content.as_bytes())
}

#[tauri::command]
fn write_library_base64(app: AppHandle, relative_path: String, contents_base64: String) -> Result<(), String> {
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

#[tauri::command]
fn set_default_typst_style(app: AppHandle, relative_path: Option<String>) -> Result<(), String> {
    let mut c = load_config(&app)?;
    c.default_typst_style_relative_path = relative_path;
    save_config(&app, &c)
}

#[tauri::command]
fn list_typst_theme_files(app: AppHandle) -> Result<Vec<String>, String> {
    let config = load_config(&app)?;
    let root = config
        .library_root
        .ok_or("Сначала выберите папку библиотеки")?;
    let root = PathBuf::from(root);
    let themes_dir = root.join(".reader-typst-themes");
    if !themes_dir.is_dir() {
        return Ok(vec![]);
    }
    let mut names: Vec<String> = std::fs::read_dir(&themes_dir)
        .map_err(|e| e.to_string())?
        .filter_map(|e| e.ok())
        .filter(|e| e.path().is_file())
        .filter_map(|e| {
            e.path()
                .extension()
                .and_then(|x| x.to_str())
                .filter(|x| x.eq_ignore_ascii_case("typ"))
                .and_then(|_| e.file_name().into_string().ok())
        })
        .collect();
    names.sort();
    Ok(names)
}

/// Собирает пути `reader-typst-{hash}-N.svg` в temp, отсортированные по N.
fn typst_svg_page_files(temp_dir: &Path, hash_hex: &str) -> Result<Vec<std::path::PathBuf>, String> {
    let prefix = format!("reader-typst-{}-", hash_hex);
    let mut pages: Vec<(u32, std::path::PathBuf)> = Vec::new();
    let read = std::fs::read_dir(temp_dir).map_err(|e| e.to_string())?;
    for entry in read.filter_map(|e| e.ok()) {
        let name = entry.file_name().to_string_lossy().to_string();
        if !name.starts_with(&prefix) || !name.ends_with(".svg") {
            continue;
        }
        let rest = name
            .strip_prefix(&prefix)
            .and_then(|s| s.strip_suffix(".svg"));
        if let Some(num_str) = rest {
            if let Ok(n) = num_str.parse::<u32>() {
                pages.push((n, entry.path()));
            }
        }
    }
    pages.sort_by_key(|(n, _)| *n);
    Ok(pages.into_iter().map(|(_, p)| p).collect())
}

/// Удаляет старые SVG превью для того же ключа (перед перекомпиляцией).
fn remove_old_typst_preview_svgs(temp_dir: &Path, hash_hex: &str) {
    let prefix = format!("reader-typst-{}-", hash_hex);
    if let Ok(read) = std::fs::read_dir(temp_dir) {
        for entry in read.filter_map(|e| e.ok()) {
            let name = entry.file_name().to_string_lossy().to_string();
            if name.starts_with(&prefix) && name.ends_with(".svg") {
                let _ = std::fs::remove_file(entry.path());
            }
        }
    }
}

/// Компиляция `main_relative` в SVG через CLI `typst`.
/// Несколько страниц: в Typst 0.14+ нужен шаблон пути с `{p}`, иначе ошибка экспорта.
/// `pages`: например `"1-18"` для быстрого предпросмотра (`typst compile --pages …`).
#[tauri::command]
fn compile_typst_to_svg(
    app: AppHandle,
    main_relative: String,
    pages: Option<String>,
) -> Result<String, String> {
    let config = load_config(&app)?;
    let root = config
        .library_root
        .ok_or("Сначала выберите папку библиотеки")?;
    let lib_root = PathBuf::from(root);
    let main_full = safe_join(&lib_root, &main_relative)?;
    let work_dir = main_full
        .parent()
        .ok_or("Некорректный путь к файлу")?
        .to_path_buf();
    let file_name = main_full
        .file_name()
        .ok_or("Некорректное имя файла")?
        .to_str()
        .ok_or("Некорректное имя файла")?;

    let hash_hex = format!("{:x}", Sha256::digest(main_relative.as_bytes()));
    let temp_dir = std::env::temp_dir();
    remove_old_typst_preview_svgs(&temp_dir, &hash_hex);

    let pattern_path = temp_dir.join(format!("reader-typst-{}-{{p}}.svg", hash_hex));
    let pattern_for_typst = pattern_path.to_string_lossy().replace('\\', "/");

    let mut cmd = std::process::Command::new("typst");
    cmd.current_dir(&work_dir)
        .arg("compile")
        .arg("--format")
        .arg("svg");
    if let Some(ref p) = pages {
        let pt = p.trim();
        if !pt.is_empty() {
            cmd.arg("--pages").arg(pt);
        }
    }
    cmd.arg(file_name).arg(&pattern_for_typst);

    let output = cmd.output()
        .map_err(|e| {
            format!(
                "Не удалось запустить `typst` (установите Typst и добавьте в PATH): {}",
                e
            )
        })?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
        let detail = if !stderr.is_empty() {
            stderr
        } else if !stdout.is_empty() {
            stdout
        } else {
            format!("код {:?}", output.status.code())
        };
        return Err(format!("Typst: {}", detail));
    }

    let pages = typst_svg_page_files(&temp_dir, &hash_hex)?;
    if pages.is_empty() {
        return Err(
            "Typst не создал SVG (пустой вывод). Проверьте путь к файлу и версию typst.".into(),
        );
    }

    let mut combined = String::from(r#"<div class="reader-typst-svg-pages">"#);
    for path in pages {
        let svg = std::fs::read_to_string(&path).map_err(|e| e.to_string())?;
        combined.push_str(r#"<div class="reader-typst-svg-page">"#);
        combined.push_str(&svg);
        combined.push_str("</div>");
    }
    combined.push_str("</div>");
    Ok(combined)
}

#[tauri::command]
fn typst_cli_version() -> Result<Option<String>, String> {
    let out = match std::process::Command::new("typst").arg("--version").output() {
        Ok(o) => o,
        Err(_) => return Ok(None),
    };
    if !out.status.success() {
        return Ok(None);
    }
    let s = String::from_utf8_lossy(&out.stdout).trim().to_string();
    Ok(if s.is_empty() { None } else { Some(s) })
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LlmMessage {
    pub role: String,
    pub content: String,
}

/// Как TranslateBooksWithLLMs OpenAI: для локального OpenAI-совместимого API отключаем thinking.
fn llm_local_disable_thinking(base: &str) -> bool {
    let b = base.to_lowercase();
    let local = b.contains("127.0.0.1") || b.contains("localhost");
    let official = b.contains("api.openai.com");
    local && !official
}

#[derive(Debug, Deserialize)]
struct OpenAiChatResponse {
    choices: Vec<OpenAiChatChoice>,
}

#[derive(Debug, Deserialize)]
struct OpenAiChatChoice {
    message: OpenAiChatMsgBody,
}

#[derive(Debug, Deserialize)]
struct OpenAiChatMsgBody {
    content: Option<String>,
}

#[derive(Debug, Deserialize)]
struct OpenAiModelList {
    data: Vec<OpenAiModelEntry>,
}

#[derive(Debug, Deserialize)]
struct OpenAiModelEntry {
    id: String,
}

/// OpenAI-совместимый чат (LM Studio, Ollama `/v1`).
#[tauri::command]
async fn llm_chat_completion(
    base_url: String,
    model: String,
    messages: Vec<LlmMessage>,
    temperature: f64,
    max_tokens: Option<u32>,
) -> Result<String, String> {
    let base = base_url.trim().trim_end_matches('/');
    if base.is_empty() {
        return Err("Укажите URL API (например http://127.0.0.1:1234/v1)".into());
    }
    let url = format!("{}/chat/completions", base);
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(900))
        .build()
        .map_err(|e| e.to_string())?;

    let mut payload = serde_json::json!({
        "model": model,
        "messages": serde_json::to_value(&messages).map_err(|e| e.to_string())?,
        "temperature": temperature,
        "stream": false,
    });
    if let Some(mt) = max_tokens {
        if let Some(obj) = payload.as_object_mut() {
            obj.insert("max_tokens".into(), serde_json::json!(mt));
        }
    }
    if llm_local_disable_thinking(&base) {
        if let Some(obj) = payload.as_object_mut() {
            obj.insert("thinking".into(), serde_json::json!(false));
            obj.insert("enable_thinking".into(), serde_json::json!(false));
            obj.insert(
                "chat_template_kwargs".into(),
                serde_json::json!({ "enable_thinking": false }),
            );
        }
    }

    let resp = client
        .post(&url)
        .header("Content-Type", "application/json")
        .json(&payload)
        .send()
        .await
        .map_err(|e| format!("LLM: {}", e))?;

    let status = resp.status();
    let raw = resp.text().await.map_err(|e| e.to_string())?;
    if !status.is_success() {
        let short: String = raw.chars().take(400).collect();
        return Err(format!("LLM: HTTP {} — {}", status, short));
    }

    let parsed: OpenAiChatResponse = serde_json::from_str(&raw).map_err(|e| {
        format!(
            "LLM JSON ({}): {}",
            e,
            raw.chars().take(200).collect::<String>()
        )
    })?;

    let content = parsed
        .choices
        .first()
        .and_then(|c| c.message.content.clone())
        .unwrap_or_default();

    Ok(content)
}

/// GET `{base_url}/models`.
#[tauri::command]
async fn llm_list_models(base_url: String) -> Result<Vec<String>, String> {
    let base = base_url.trim().trim_end_matches('/');
    if base.is_empty() {
        return Err("Укажите URL API".into());
    }
    let url = format!("{}/models", base);
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(60))
        .build()
        .map_err(|e| e.to_string())?;

    let resp = client
        .get(&url)
        .send()
        .await
        .map_err(|e| format!("Модели: {}", e))?;

    let status = resp.status();
    let raw = resp.text().await.map_err(|e| e.to_string())?;
    if !status.is_success() {
        let short: String = raw.chars().take(400).collect();
        return Err(format!("Модели: HTTP {} — {}", status, short));
    }

    let parsed: OpenAiModelList = serde_json::from_str(&raw).map_err(|e| {
        format!(
            "Список моделей ({}): {}",
            e,
            raw.chars().take(200).collect::<String>()
        )
    })?;

    let mut ids: Vec<String> = parsed.data.into_iter().map(|m| m.id).collect();
    ids.sort();
    ids.dedup();
    Ok(ids)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![
            get_library_snapshot,
            set_library_root,
            save_library_metadata,
            read_book_base64,
            save_translated_pdf_next_to_source,
            write_file_base64,
            translate_texts,
            pdf_translation_load,
            pdf_translation_save,
            pdf_translation_delete,
            read_library_utf8,
            write_library_utf8,
            write_library_base64,
            set_default_typst_style,
            list_typst_theme_files,
            compile_typst_to_svg,
            typst_cli_version,
            llm_chat_completion,
            llm_list_models,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
