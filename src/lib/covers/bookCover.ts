/**
 * Миниатюры обложек для сетки библиотеки (клиент + pdf.js).
 */
import { getDocument, GlobalWorkerOptions } from "pdfjs-dist";
import pdfWorker from "pdfjs-dist/build/pdf.worker.min.mjs?url";
import { readLibraryBookBytes } from "$lib/library/readLibraryBookBytes";
import { parseFb2 } from "$lib/fb2/parseFb2";

GlobalWorkerOptions.workerSrc = pdfWorker;

const MAX_THUMB_W = 168;
const JPEG_QUALITY = 0.72;

/** Первое data:-изображение из HTML обложки FB2. */
export function dataUrlFromFb2CoverHtml(html: string): string | null {
  const m = html.match(/src="(data:image\/[^"]+)"/i);
  return m?.[1] ?? null;
}

async function pdfCoverDataUrl(bytes: Uint8Array): Promise<string | null> {
  const pdf = await getDocument({ data: bytes }).promise;
  const page = await pdf.getPage(1);
  const oc = pdf.getOptionalContentConfig({ intent: "any" });
  const baseVp = page.getViewport({ scale: 1 });
  const scale = MAX_THUMB_W / baseVp.width;
  const viewport = page.getViewport({ scale });
  const canvas = document.createElement("canvas");
  canvas.width = Math.ceil(viewport.width);
  canvas.height = Math.ceil(viewport.height);
  const ctx = canvas.getContext("2d");
  if (!ctx) return null;

  await page
    .render({
      canvas,
      canvasContext: ctx,
      viewport,
      background: "#ffffff",
      intent: "any",
      optionalContentConfigPromise: oc,
    })
    .promise;

  try {
    return canvas.toDataURL("image/jpeg", JPEG_QUALITY);
  } catch {
    return null;
  }
}

export async function buildBookCoverDataUrl(
  bookRelativePath: string,
  format: "pdf" | "epub" | "fb2" | "typst",
): Promise<string | null> {
  if (format === "typst") return null;

  const bytes = await readLibraryBookBytes(bookRelativePath);

  if (format === "pdf") {
    try {
      return await pdfCoverDataUrl(bytes);
    } catch {
      return null;
    }
  }

  if (format === "fb2") {
    try {
      const xml = new TextDecoder("utf-8").decode(bytes);
      const parsed = parseFb2(xml);
      const fromCover = dataUrlFromFb2CoverHtml(parsed.coverHtml);
      if (fromCover) return fromCover;
    } catch {
      return null;
    }
  }

  return null;
}
