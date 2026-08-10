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

- PDF, EPUB, FB2 and Typst reading with format-specific navigation and outlines.
- Custom shelves, multiple shelves per book, sorting, importance, hidden books and cover thumbnails.
- Saved reading progress, comments, reviews and customizable quote cards with PNG export.
- Five built-in visual themes.
- Page and chapter translation through a configurable LibreTranslate-compatible server.
- Resumable full-PDF translation through local OpenAI-compatible APIs such as LM Studio or Ollama.
- Export to Typst and export translated PDF files.
- Local file access through the Tauri backend with path validation and atomic writes.

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
