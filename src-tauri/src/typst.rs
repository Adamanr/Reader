//! Typst: темы и компиляция в SVG через CLI.

use crate::storage::*;
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use tauri::AppHandle;

#[tauri::command]
pub async fn set_default_typst_style(
    app: AppHandle,
    relative_path: Option<String>,
) -> Result<(), String> {
    let mut c = load_config(&app)?;
    c.default_typst_style_relative_path = relative_path;
    save_config(&app, &c)
}

#[tauri::command]
pub async fn list_typst_theme_files(app: AppHandle) -> Result<Vec<String>, String> {
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
pub(crate) fn typst_svg_page_files(
    temp_dir: &Path,
    hash_hex: &str,
) -> Result<Vec<std::path::PathBuf>, String> {
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
pub(crate) fn remove_old_typst_preview_svgs(temp_dir: &Path, hash_hex: &str) {
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
pub async fn compile_typst_to_svg(
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

    let output = cmd.output().map_err(|e| {
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
pub async fn typst_cli_version() -> Result<Option<String>, String> {
    let out = match std::process::Command::new("typst")
        .arg("--version")
        .output()
    {
        Ok(o) => o,
        Err(_) => return Ok(None),
    };
    if !out.status.success() {
        return Ok(None);
    }
    let s = String::from_utf8_lossy(&out.stdout).trim().to_string();
    Ok(if s.is_empty() { None } else { Some(s) })
}
