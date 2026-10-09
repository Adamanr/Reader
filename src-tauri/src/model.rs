//! Модель данных библиотеки: полки, книги, заметки, цитаты, настройки.

use crate::library_index;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

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
    #[serde(default)]
    pub hidden: bool,
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
    /// Поля, которые знает только фронтенд (статус, выделения, прогресс…): сохраняем как есть.
    #[serde(flatten)]
    pub extra: serde_json::Map<String, serde_json::Value>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct LibraryMetadata {
    pub shelves: Vec<Shelf>,
    pub books: HashMap<String, BookMeta>,
    #[serde(flatten)]
    pub extra: serde_json::Map<String, serde_json::Value>,
}

#[derive(Debug, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct AppConfig {
    pub library_root: Option<String>,
    /// Путь к `.typ` теме по умолчанию (относительно папки библиотеки), например `.reader-typst-themes/minimal.typ`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default_typst_style_relative_path: Option<String>,
    /// Хранить метаданные, заметки и обложки в `<библиотека>/.reader/` —
    /// тогда их синхронизирует Syncthing/Nextcloud вместе с книгами.
    #[serde(default)]
    pub sync_in_library: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LibrarySnapshot {
    pub library_root: Option<String>,
    pub book_paths: Vec<String>,
    pub hidden_book_paths: Vec<String>,
    pub metadata: LibraryMetadata,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default_typst_style_relative_path: Option<String>,
    pub sync_in_library: bool,
    /// Книги, сменившие путь при этом сканировании: фронтенд переносит свои данные (статистику, словарь).
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub relocated: Vec<library_index::Relocation>,
    /// Сколько конфликтных копий синхронизации объединено при этом сканировании.
    #[serde(skip_serializing_if = "is_zero")]
    pub merged_conflicts: usize,
}

fn is_zero(n: &usize) -> bool {
    *n == 0
}
