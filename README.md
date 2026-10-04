<div align="center">

# Reader

**A local-first e-book reader and personal library for desktop and Android**

[![CI](https://github.com/Adamanr/Reader/actions/workflows/ci.yml/badge.svg)](https://github.com/Adamanr/Reader/actions/workflows/ci.yml)
[![Tauri 2](https://img.shields.io/badge/Tauri-2-24C8D8?logo=tauri&logoColor=white)](https://v2.tauri.app/)
[![Svelte 5](https://img.shields.io/badge/Svelte-5-FF3E00?logo=svelte&logoColor=white)](https://svelte.dev/)
[![License: MIT](https://img.shields.io/badge/License-MIT-green.svg)](LICENSE)

</div>

Reader keeps a book collection on your device and provides a focused Russian-language reading interface. It supports PDF, EPUB, FB2 and Typst, remembers reading positions, and stores library metadata locally. No account or cloud service is required.

> Reader is under active development. Back up important books and metadata before testing pre-release builds.

## Features

**Reading**

- PDF, EPUB, FB2 and Typst with outlines, full-text search and precise position restore.
- Typography panel: bundled Literata, Lora, PT Serif and PT Sans, size, line height, line length, margins, paragraph spacing, hyphenation and presets.
- Page themes independent of the app theme (paper, sepia, graphite, night, cover-tinted) and a "living cover" accent taken from the book cover.
- Immersive and fullscreen modes, tap zones and swipes, keyboard shortcuts, a status line with chapter, time left and a seekable progress bar.
- PDF: sharp HiDPI rendering, continuous scroll, single page and spread layouts, fit width/page, dark inversion and white-margin cropping.
- Focus modes: paragraph dimming and a reading ruler.

**Notes**

- Colored highlights with margin notes in all formats, a notes panel per book and a cross-book notes feed.
- Markdown export (Obsidian-friendly) and customizable quote cards with PNG export.
- Quote constellation: your highlights laid out as stars by similarity.

**Library**

- "Continue reading" block, reading statuses, cover grid or spine shelf, drag-and-drop import and automatic title/author detection.
- "Dust on the shelf": books you haven't touched for months slowly fade.
- Reading rhythm: time per day and per book, streaks, heatmap and your personal reading pace.
- Optional sync without a cloud: keep metadata in `<library>/.reader` and sync the folder with Syncthing or Nextcloud.

**Assistant and audio (all local)**

- "Previously in the book…": a spoiler-free recap up to your current position, via LM Studio or Ollama.
- "Who is this?": explains a character or term using only the part you've already read and builds a per-book glossary.
- Word lookup with translation and in-context meaning, plus a spaced-repetition deck of words with their original sentences.
- Read aloud with system voices, Piper or espeak-ng; RSVP speed reading; generated ambient soundscapes.
- Page and chapter translation through LibreTranslate and resumable full-PDF translation through local OpenAI-compatible APIs.
- Export to Typst and export translated PDF files.

### Keyboard shortcuts in the reader

| Key | Action |
| --- | --- |
| ← → / PgUp PgDn / Space | Previous / next page |
| F, F11 | Immersive mode, fullscreen |
| T, N, / or Ctrl+F | Contents, notes, search |
| A | Typography panel |
| S, R | Read aloud, RSVP speed reading |
| Ctrl + / Ctrl − | Larger / smaller text (zoom for PDF) |
| Esc | Close popups, leave immersive mode |

## Downloads

Installable packages should be attached to [GitHub Releases](https://github.com/Adamanr/Reader/releases). Build artifacts are deliberately not committed to the source repository.

Unsigned or development builds may trigger operating-system warnings. Production distribution also requires the appropriate platform signing credentials.

## Requirements

- Node.js 20 or newer and npm.
- A stable Rust toolchain.
- The [Tauri prerequisites](https://v2.tauri.app/start/prerequisites/) for your operating system.
- Typst CLI for Typst preview and export.
- Android Studio, Android SDK and NDK for Android builds.

Translation is optional. Reader can use a LibreTranslate-compatible endpoint for interactive translation and a local OpenAI-compatible endpoint for full-PDF translation.

The reading assistant is optional too: point it at LM Studio or Ollama in Settings. For read-aloud without system voices, install [Piper](https://github.com/rhasspy/piper) (with a Russian voice such as `ru_RU-irina-medium`) or `espeak-ng` and make sure it is on `PATH`.

## Development

```bash
git clone https://github.com/Adamanr/Reader.git
cd Reader
npm ci
npm run tauri dev
```

Useful checks:

```bash
npm run check
npm run build
cargo fmt --manifest-path src-tauri/Cargo.toml -- --check
cargo check --locked --manifest-path src-tauri/Cargo.toml
```

Install on Linux for the current user (adds Reader to the application menu, no root needed; re-run to update):

```bash
./scripts/install-linux.sh
```

Remove it with `./scripts/install-linux.sh --uninstall`.

Build a desktop package:

```bash
npm run tauri build
```

Run or build the Android application after completing the Tauri mobile prerequisites:

```bash
npm run tauri android dev
npm run tauri android build
```

Desktop packages are written below `src-tauri/target/release/bundle/`. Android packages are generated below `src-tauri/gen/android/app/build/outputs/`. Upload distributable files to a GitHub Release instead of adding them to Git.

## Project layout

```text
src/
├── lib/components/       Reader UI components
├── lib/library/          Library state and cover loading
├── lib/pdf/              PDF rendering and translated PDF export
├── lib/translate/        LibreTranslate and LLM translation clients
├── lib/typst/            Typst conversion, themes and outlines
└── routes/               Library, reader and settings screens

src-tauri/
├── src/                  Rust backend and Tauri commands
├── capabilities/         Application permissions
├── gen/android/          Android project generated and maintained by Tauri
└── tauri.conf.json       Application and bundle configuration
```

## Data and privacy

Reader does not upload the library by itself. Application configuration, metadata and PDF translation cache live in the platform configuration directory under `com.adaman.reader`; book files remain in the library directory selected by the user. On Linux, the configuration directory is normally `~/.config/com.adaman.reader/`.

When translation is enabled, the selected text is sent to the endpoint configured in the application. API settings and the interactive translation cache are stored locally in the application WebView storage. Review the privacy policy of any public translation service before sending sensitive documents.

## License

Released under the [MIT License](LICENSE).
