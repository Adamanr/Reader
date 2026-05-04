<div align="center">

# Reader

**A beautiful desktop e-book reader and personal library manager**

*Built with Tauri · SvelteKit · TypeScript · Rust*

![Tauri](https://img.shields.io/badge/Tauri-2.x-24C8D8?style=flat-square&logo=tauri&logoColor=white)
![Svelte](https://img.shields.io/badge/Svelte-5.x-FF3E00?style=flat-square&logo=svelte&logoColor=white)
![TypeScript](https://img.shields.io/badge/TypeScript-5.6-3178C6?style=flat-square&logo=typescript&logoColor=white)
![Rust](https://img.shields.io/badge/Rust-2021-CE422B?style=flat-square&logo=rust&logoColor=white)
![License](https://img.shields.io/badge/license-MIT-green?style=flat-square)

</div>

---

## Overview

Reader is a cross-platform desktop application for managing and reading your book collection. It supports multiple e-book formats, provides an integrated translation engine, and lets you save quotes, write reviews, and organize books into custom shelves — all in a clean, themeable interface.

---

## Features

### Library Management
- **Multi-format support** — PDF, EPUB, FB2, and Typst documents
- **Custom shelves** — organize books into any number of named collections
- **Rich metadata** — titles, authors, importance ratings, reviews, and personal notes
- **Smart sorting** — by recent activity, title (A–Z / Z–A), or importance
- **Cover thumbnails** — auto-extracted and cached for fast browsing
- **Reading progress** — automatically restored on reopen

### Reading Experience

| Format | Features |
|--------|----------|
| **PDF** | Page navigation, TOC outline, text selection, scale control, translation overlay |
| **EPUB** | Chapter navigation, spine-based progress, text selection, translation |
| **FB2** | Native XML parsing, progress tracking, translation integration |
| **Typst** | Source editor, live preview, outline extraction, theme customization |

### Translation
- Powered by **LibreTranslate** (self-hosted or custom endpoint)
- Supports **9 source** and **8 target** languages
- Per-page translation with **hash-based caching**
- View modes: source, target, or split layout
- **Full-book PDF export** with translated text overlaid on original pages

### Quotes & Annotations
- Save quotes with **custom accent colors** (sand, sage, dusty rose, ink)
- Layout options: wide, narrow, or centered
- Background images with adjustable opacity
- **Export quote cards** to PNG
- Page-bound comments and book-level reviews

### Themes
Five built-in themes with full CSS variable support:

`Light` · `Dark` · `Sepia` · `Forest` · `Ocean`

### Export
- Convert PDF books to **Typst format** automatically
- Export translated content as a new **annotated PDF**
- Typst theme selection for exported documents

---

## Tech Stack

**Frontend**
- [Svelte 5](https://svelte.dev) with runes (`$state`, `$derived`, `$effect`)
- [SvelteKit 2](https://kit.svelte.dev) — static adapter, SPA mode
- [Tailwind CSS 3](https://tailwindcss.com)
- [pdf.js](https://mozilla.github.io/pdf.js/) — PDF rendering
- [epub.js](https://github.com/futurepress/epub.js/) — EPUB rendering
- [pdf-lib](https://pdf-lib.js.org/) — PDF generation

**Backend**
- [Tauri 2](https://tauri.app) — native desktop shell
- Rust 2021 — file system, metadata, path safety, atomic writes
- `reqwest`, `serde`, `walkdir`, `sha2`, `dirs`

---

## Getting Started

### Prerequisites

- [Node.js](https://nodejs.org) 18+
- [Rust](https://rustup.rs) stable toolchain
- [Tauri CLI prerequisites](https://tauri.app/start/prerequisites/) for your OS
- [Typst CLI](https://typst.app) *(optional — required for Typst preview)*

### Installation

```bash
git clone https://github.com/Adamanr/Reader.git
cd Reader
npm install
```

### Development

```bash
npm run tauri dev
```

Starts the Vite dev server on port `1420` and launches the Tauri app with hot reload.

### Build

```bash
npm run tauri build
```

Produces a native installer for your platform in `src-tauri/target/release/bundle/`.

### Type Checking

```bash
npm run check          # one-shot check
npm run check:watch    # continuous watch mode
```

---

## Project Structure

```
Reader/
├── src/                    # SvelteKit frontend
│   ├── routes/
│   │   ├── +page.svelte        # Library home
│   │   ├── read/+page.svelte   # Reading view
│   │   └── settings/           # Settings page
│   └── lib/
│       ├── components/         # UI components
│       ├── fb2/                # FB2 parser
│       ├── pdf/                # PDF utilities & translation export
│       ├── translate/          # Translation API client
│       ├── typst/              # Typst export & themes
│       └── library/            # Shelf & metadata management
├── src-tauri/              # Tauri + Rust backend
│   ├── src/lib.rs              # Core commands
│   └── tauri.conf.json         # App configuration
└── static/                 # Fonts and static assets
```

---

## Data & Storage

All data is stored locally — no cloud sync, no accounts.

| Data | Location |
|------|----------|
| App config | `~/.config/com.adaman.reader/config.json` |
| Library metadata | `<library-root>/library-metadata.json` |
| Translation cache | `<library-root>/pdf-translations/<hash>.json` |

Metadata is written with **atomic rename** (write to temp → rename) to prevent corruption on crash.

---

## Translation Setup

Reader uses [LibreTranslate](https://libretranslate.com) for text translation.

1. Run a local LibreTranslate instance **or** use a public endpoint
2. Open **Settings → Translation**
3. Enter your server URL and API key
4. Start translating from any reader view

---

## License

MIT © [Adamanr](https://github.com/Adamanr)
