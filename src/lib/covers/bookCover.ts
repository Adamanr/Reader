/**
 * Миниатюры обложек и сведения о книге (название, автор) из самого файла.
 */
import { getDocument, GlobalWorkerOptions } from "pdfjs-dist";
import pdfWorker from "pdfjs-dist/build/pdf.worker.min.mjs?url";
import { readLibraryBookBytes } from "$lib/library/readLibraryBookBytes";
import { parseFb2 } from "$lib/fb2/parseFb2";

GlobalWorkerOptions.workerSrc = pdfWorker;

/** Ширина миниатюры: с запасом для HiDPI-экранов и крупного блока «Продолжить». */
const MAX_THUMB_W = 260;
const JPEG_QUALITY = 0.78;

export interface BookFileInfo {
  cover: string | null;
  title: string | null;
  author: string | null;
}

/** Первое data:-изображение из HTML обложки FB2. */
export function dataUrlFromFb2CoverHtml(html: string): string | null {
  const m = html.match(/src="(data:image\/[^"]+)"/i);
  return m?.[1] ?? null;
}

function cleanMetaString(v: unknown): string | null {
  if (typeof v !== "string") return null;
  const s = v.replace(/\s+/g, " ").trim();
  if (s.length < 2 || s.length > 200) return null;
  // Мусор из конвертеров: «Microsoft Word - doc1.docx», «untitled» и т.п.
  if (/\.(docx?|rtf|odt|pdf|tex|indd)$/i.test(s)) return null;
  if (/^(untitled|unknown|без названия|microsoft word)/i.test(s)) return null;
  return s;
}

/** Уменьшает изображение до миниатюры (JPEG). */
async function downscaleImage(src: string): Promise<string | null> {
  const img = new Image();
  img.decoding = "async";
  img.src = src;
  try {
    await img.decode();
  } catch {
    return null;
  }
  if (!img.naturalWidth || !img.naturalHeight) return null;
  const scale = Math.min(1, MAX_THUMB_W / img.naturalWidth);
  const canvas = document.createElement("canvas");
  canvas.width = Math.max(1, Math.round(img.naturalWidth * scale));
  canvas.height = Math.max(1, Math.round(img.naturalHeight * scale));
  const ctx = canvas.getContext("2d");
  if (!ctx) return null;
  ctx.fillStyle = "#fff";
  ctx.fillRect(0, 0, canvas.width, canvas.height);
  ctx.drawImage(img, 0, 0, canvas.width, canvas.height);
  try {
    return canvas.toDataURL("image/jpeg", JPEG_QUALITY);
  } catch {
    return null;
  }
}

async function pdfInfo(bytes: Uint8Array, wantCover: boolean): Promise<BookFileInfo> {
  const pdf = await getDocument({ data: bytes }).promise;
  let title: string | null = null;
  let author: string | null = null;
  try {
    const meta = await pdf.getMetadata();
    const info = (meta?.info ?? {}) as Record<string, unknown>;
    title = cleanMetaString(info.Title);
    author = cleanMetaString(info.Author);
  } catch {
    /* метаданные необязательны */
  }
  let cover: string | null = null;
  if (wantCover) {
    const page = await pdf.getPage(1);
    const oc = pdf.getOptionalContentConfig({ intent: "any" });
    const baseVp = page.getViewport({ scale: 1 });
    const viewport = page.getViewport({ scale: MAX_THUMB_W / baseVp.width });
    const canvas = document.createElement("canvas");
    canvas.width = Math.ceil(viewport.width);
    canvas.height = Math.ceil(viewport.height);
    const ctx = canvas.getContext("2d");
    if (ctx) {
      await page.render({
        canvas,
        canvasContext: ctx,
        viewport,
        background: "#ffffff",
        intent: "any",
        optionalContentConfigPromise: oc,
      }).promise;
      try {
        cover = canvas.toDataURL("image/jpeg", JPEG_QUALITY);
      } catch {
        cover = null;
      }
    }
  }
  void pdf.destroy();
  return { cover, title, author };
}

async function epubInfo(bytes: Uint8Array, wantCover: boolean): Promise<BookFileInfo> {
  const ePub = (await import("epubjs")).default;
  const book = ePub(bytes.buffer as ArrayBuffer);
  try {
    await book.ready;
    const md = (book as any).packaging?.metadata ?? {};
    const title = cleanMetaString(md.title);
    const author = cleanMetaString(md.creator);
    let cover: string | null = null;
    if (wantCover) {
      const url = await book.coverUrl();
      if (url) cover = await downscaleImage(url);
    }
    return { cover, title, author };
  } finally {
    book.destroy();
  }
}

export async function readBookFileInfo(
  bookRelativePath: string,
  format: "pdf" | "epub" | "fb2" | "typst",
  wantCover = true,
): Promise<BookFileInfo> {
  const empty = { cover: null, title: null, author: null };
  if (format === "typst") return empty;

  const bytes = await readLibraryBookBytes(bookRelativePath);
  try {
    if (format === "pdf") return await pdfInfo(bytes, wantCover);
    if (format === "epub") return await epubInfo(bytes, wantCover);
    const xml = new TextDecoder("utf-8").decode(bytes);
    const parsed = parseFb2(xml);
    const raw = wantCover ? dataUrlFromFb2CoverHtml(parsed.coverHtml) : null;
    return {
      cover: raw ? await downscaleImage(raw) : null,
      title: cleanMetaString(parsed.bookTitle),
      author: cleanMetaString(parsed.author),
    };
  } catch {
    return empty;
  }
}

/** Совместимость со старым API: только обложка. */
export async function buildBookCoverDataUrl(
  bookRelativePath: string,
  format: "pdf" | "epub" | "fb2" | "typst",
): Promise<string | null> {
  return (await readBookFileInfo(bookRelativePath, format)).cover;
}
