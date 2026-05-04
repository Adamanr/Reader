import {
  PDFDocument,
  ParseSpeeds,
  rgb,
  type LoadOptions,
  type PDFFont,
  type PDFPage,
} from "pdf-lib";
import { getDocument, GlobalWorkerOptions } from "pdfjs-dist";
import pdfWorker from "pdfjs-dist/build/pdf.worker.min.mjs?url";
import { getTextItemPdfBoxes, type PdfAxisBBox } from "./textItemPdfBoxes";
import { assertBufferLooksLikePdf } from "$lib/library/readLibraryBookBytes";

GlobalWorkerOptions.workerSrc = pdfWorker;

let fontCache: Uint8Array | null = null;

export async function loadBundledCyrillicFont(): Promise<Uint8Array> {
  if (fontCache) return fontCache;
  const url = `${typeof window !== "undefined" ? window.location.origin : ""}/fonts/NotoSans-Regular.ttf`;
  const res = await fetch(url);
  if (!res.ok) {
    throw new Error(
      "Не найден шрифт /fonts/NotoSans-Regular.ttf (нужен для кириллицы в PDF).",
    );
  }
  fontCache = new Uint8Array(await res.arrayBuffer());
  return fontCache;
}

function breakLongWord(word: string, font: PDFFont, size: number, maxW: number): string[] {
  const out: string[] = [];
  let buf = "";
  for (const ch of word) {
    const trial = buf + ch;
    if (font.widthOfTextAtSize(trial, size) <= maxW) buf = trial;
    else {
      if (buf) out.push(buf);
      buf = ch;
    }
  }
  if (buf) out.push(buf);
  return out.length ? out : [word];
}

function wrapToWidth(
  text: string,
  font: PDFFont,
  size: number,
  maxW: number,
): string[] {
  const t = text.replace(/\s+/g, " ").trim();
  if (!t) return [""];
  const words = t.split(" ");
  const lines: string[] = [];
  let line = "";
  for (const word of words) {
    const trial = line ? `${line} ${word}` : word;
    if (font.widthOfTextAtSize(trial, size) <= maxW) {
      line = trial;
      continue;
    }
    if (line) lines.push(line);
    if (font.widthOfTextAtSize(word, size) <= maxW) {
      line = word;
    } else {
      lines.push(...breakLongWord(word, font, size, maxW));
      line = "";
    }
  }
  if (line) lines.push(line);
  return lines.length ? lines : [""];
}

function drawTextInBox(
  page: PDFPage,
  font: PDFFont,
  text: string,
  box: PdfAxisBBox,
) {
  const pad = 0.75;
  const innerW = Math.max(1, box.width - 2 * pad);
  const innerH = Math.max(1, box.height - 2 * pad);
  const { left, bottom } = box;

  if (!text.trim()) return;

  const lineHeightFactor = 1.14;
  for (
    let size = Math.min(11, innerH / lineHeightFactor);
    size >= 3.2;
    size -= 0.18
  ) {
    const lh = size * lineHeightFactor;
    const lines = wrapToWidth(text, font, size, innerW);
    const totalH = lines.length * lh;
    if (totalH <= innerH && lines.every((ln) => font.widthOfTextAtSize(ln, size) <= innerW)) {
      let baseline = bottom + pad + innerH - size * 0.72;
      for (const ln of lines) {
        page.drawText(ln, {
          x: left + pad,
          y: baseline,
          size,
          font,
          color: rgb(0.08, 0.08, 0.09),
        });
        baseline -= lh;
      }
      return;
    }
  }

  const size = 3.2;
  const lh = size * lineHeightFactor;
  const lines = wrapToWidth(text, font, size, innerW);
  let baseline = bottom + pad + innerH - size * 0.72;
  for (const ln of lines) {
    page.drawText(ln, {
      x: left + pad,
      y: baseline,
      size,
      font,
      color: rgb(0.08, 0.08, 0.09),
      maxWidth: innerW,
    });
    baseline -= lh;
  }
}

/**
 * Собирает новый PDF: исходная страница + белые прямоугольники по текстовым областям + переведённый текст.
 */
export async function buildTranslatedPdf(opts: {
  originalPdfBytes: Uint8Array;
  pages: Record<string, string[]>;
  fontBytes?: Uint8Array;
}): Promise<Uint8Array> {
  const fontBytes = opts.fontBytes ?? (await loadBundledCyrillicFont());

  assertBufferLooksLikePdf(opts.originalPdfBytes, "Сборка PDF");

  const pdfJs = await getDocument({ data: opts.originalPdfBytes }).promise;

  let outDoc: PDFDocument;
  try {
    outDoc = await loadOriginalWithPdfLib(opts.originalPdfBytes);
  } catch (e) {
    const m = e instanceof Error ? e.message : String(e);
    if (m.includes("No PDF header") || m.includes("Failed to parse PDF document")) {
      throw new Error(
        "Исходный PDF не читается движком сборки (pdf-lib), хотя просмотр в читалке может работать. Сохраните книгу в другом приложении как обычный PDF (без защиты/оболочек) и положите в библиотеку снова, либо откройте issue с примером файла.",
      );
    }
    if (/encrypted|password/i.test(m)) {
      throw new Error(
        "PDF с шифрованием: откройте в читалке, при необходимости уберите пароль в просмотрщике и пересохраните копию в библиотеку.",
      );
    }
    throw new Error(
      `Сборка перевода: не удалось открыть исходник для правок (${m.slice(0, 200)}). Попробуйте «печать в PDF» из другой программы и заменить файл.`,
    );
  }

  /** Полный шрифт — чтобы кириллица из перевода точно попала в PDF. */
  const font = await outDoc.embedFont(fontBytes);

  const num = Math.min(pdfJs.numPages, outDoc.getPageCount());

  for (let i = 0; i < num; i++) {
    const pageNum = i + 1;
    const key = String(pageNum);
    const translated = opts.pages[key];
    if (!translated?.length) continue;

    const jsPage = await pdfJs.getPage(pageNum);
    const boxes = await getTextItemPdfBoxes(jsPage);
    if (boxes.length !== translated.length) {
      console.warn(
        `[PDF export] Стр. ${pageNum}: ${boxes.length} блоков геометрии и ${translated.length} строк перевода — пропуск.`,
      );
      continue;
    }

    const page = outDoc.getPage(i);
    const thick = 1.2;

    for (let j = 0; j < boxes.length; j++) {
      const b = boxes[j]!;
      const t = translated[j] ?? "";
      if (b.width < 0.2 || b.height < 0.2) continue;

      page.drawRectangle({
        x: b.left - thick / 2,
        y: b.bottom - thick / 2,
        width: b.width + thick,
        height: b.height + thick,
        color: rgb(1, 1, 1),
        borderColor: rgb(1, 1, 1),
        borderWidth: 0,
      });

      if (t.trim()) {
        drawTextInBox(page, font, t, b);
      }
    }
  }

  return await outDoc.save({ useObjectStreams: false });
}

/** pdf-lib строже pdf.js; цепочка опций для «сложных» PDF (шифрование, кривой xref). */
async function loadOriginalWithPdfLib(bytes: Uint8Array): Promise<PDFDocument> {
  const attempts: LoadOptions[] = [
    {
      parseSpeed: ParseSpeeds.Fast,
      throwOnInvalidObject: false,
      ignoreEncryption: true,
      capNumbers: true,
      updateMetadata: false,
    },
    {
      parseSpeed: ParseSpeeds.Fastest,
      throwOnInvalidObject: false,
      ignoreEncryption: true,
      updateMetadata: false,
    },
    {
      parseSpeed: ParseSpeeds.Medium,
      throwOnInvalidObject: false,
      ignoreEncryption: true,
      capNumbers: true,
      updateMetadata: false,
    },
  ];
  let last: unknown;
  for (const opts of attempts) {
    try {
      return await PDFDocument.load(bytes, opts);
    } catch (e) {
      last = e;
    }
  }
  throw last;
}
