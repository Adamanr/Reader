import { Util } from "pdfjs-dist";
import type { PDFPageProxy } from "pdfjs-dist";

/** Ось PDF в pdf-lib: снизу вверх, начало слева снизу. */
export type PdfAxisBBox = {
  left: number;
  bottom: number;
  width: number;
  height: number;
};

/**
 * Оси выровненные прямоугольники для каждого text item (тот же порядок, что getTextContent / перевод).
 */
export async function getTextItemPdfBoxes(page: PDFPageProxy): Promise<PdfAxisBBox[]> {
  const vp = page.getViewport({ scale: 1 });
  const ph = vp.height;
  const content = await page.getTextContent();
  const out: PdfAxisBBox[] = [];

  for (const item of content.items) {
    if (!item || typeof item !== "object" || !("str" in item)) continue;
    const raw = item as { str?: string; width?: number; height?: number; transform?: number[] };
    if (typeof raw.str !== "string") continue;

    const w = typeof raw.width === "number" ? raw.width : 0;
    const h = typeof raw.height === "number" ? raw.height : 0;
    const tr = raw.transform;
    if (!Array.isArray(tr) || tr.length < 6) continue;

    const T = Util.transform(vp.transform, tr as [number, number, number, number, number, number]);
    const corners = [
      [0, 0],
      [w, 0],
      [w, h],
      [0, h],
    ] as const;
    let minX = Infinity;
    let maxX = -Infinity;
    let minY = Infinity;
    let maxY = -Infinity;
    for (const [x, y] of corners) {
      const xx = T[0]! * x + T[2]! * y + T[4]!;
      const yy = T[1]! * x + T[3]! * y + T[5]!;
      minX = Math.min(minX, xx);
      maxX = Math.max(maxX, xx);
      minY = Math.min(minY, yy);
      maxY = Math.max(maxY, yy);
    }

    const pdfLeft = minX;
    const pdfBottom = ph - maxY;
    const pdfW = Math.max(0, maxX - minX);
    const pdfH = Math.max(0, maxY - minY);

    out.push({
      left: pdfLeft,
      bottom: pdfBottom,
      width: pdfW,
      height: pdfH,
    });
  }

  return out;
}
