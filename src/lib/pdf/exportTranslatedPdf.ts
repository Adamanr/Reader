import { invoke } from "@tauri-apps/api/core";
import { save } from "@tauri-apps/plugin-dialog";
import { buildTranslatedPdf, loadBundledCyrillicFont } from "./buildTranslatedPdf";
import { readLibraryBookBytes } from "$lib/library/readLibraryBookBytes";
import { loadPdfBookTranslationFile } from "$lib/translate/pdfFullBookJob";
import type { LibraryMetadata, LibrarySnapshot } from "$lib/types";
import { effectiveShelfIds } from "$lib/library/shelves";

function titleFromPath(path: string) {
  const i = path.lastIndexOf("/");
  return i >= 0 ? path.slice(i + 1) : path;
}

function bytesToBase64(bytes: Uint8Array): string {
  let binary = "";
  const chunk = 0x8000;
  for (let i = 0; i < bytes.length; i += chunk) {
    const sub = bytes.subarray(i, i + chunk);
    binary += String.fromCharCode.apply(null, sub as unknown as number[]);
  }
  return btoa(binary);
}

/** Собирает байты PDF с переводом (для записи в файл или в библиотеку). */
export async function buildTranslatedPdfBytesForBook(bookRelativePath: string): Promise<Uint8Array> {
  const doc = await loadPdfBookTranslationFile(bookRelativePath);
  if (!doc?.pages || Object.keys(doc.pages).length === 0) {
    throw new Error("Нет сохранённого перевода (сначала полный перевод через LLM).");
  }

  const orig = await readLibraryBookBytes(bookRelativePath);

  const fontBytes = await loadBundledCyrillicFont();
  return await buildTranslatedPdf({
    originalPdfBytes: orig,
    pages: doc.pages,
    fontBytes,
  });
}

/**
 * Сохраняет рядом с исходником `translate_<имя>.pdf`, помечает оригинал, копирует полки на новую книгу.
 */
export async function exportTranslatedPdfToLibrary(bookRelativePath: string): Promise<string> {
  const bytes = await buildTranslatedPdfBytesForBook(bookRelativePath);
  const newPath = await invoke<string>("save_translated_pdf_next_to_source", {
    sourceRelativePath: bookRelativePath,
    pdfBase64: bytesToBase64(bytes),
  });

  const snap = await invoke<LibrarySnapshot>("get_library_snapshot");
  const src = snap.metadata.books[bookRelativePath];
  const nb = snap.metadata.books[newPath];
  if (nb && src) {
    const ids = effectiveShelfIds(src);
    const books = {
      ...snap.metadata.books,
      [newPath]: {
        ...nb,
        shelfIds: ids,
        shelfId: ids[0] ?? "default",
      },
    };
    const metadata: LibraryMetadata = { ...snap.metadata, books };
    await invoke("save_library_metadata", { metadata });
  }

  return newPath;
}

/**
 * Диалог «Сохранить как…» (произвольный путь на диске).
 */
export async function exportTranslatedPdfToChosenFile(bookRelativePath: string): Promise<void> {
  const bytes = await buildTranslatedPdfBytesForBook(bookRelativePath);

  const name = titleFromPath(bookRelativePath).replace(/\.pdf$/i, "");
  const suggested = `translate_${name || "book"}.pdf`;

  const path = await save({
    defaultPath: suggested,
    filters: [{ name: "PDF", extensions: ["pdf"] }],
  });

  if (path == null || path === "") return;

  await invoke("write_file_base64", {
    path,
    contentsBase64: bytesToBase64(bytes),
  });
}
