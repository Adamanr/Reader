import { Util } from "pdfjs-dist";
import type { PDFPageProxy } from "pdfjs-dist";

type Item = {
  str: string;
  x: number;
  y: number;
  hasEOL: boolean;
  h: number;
};

/**
 * Собирает текст страницы в порядке чтения: строки и абзацы по координатам из PDF,
 * а не «слипшийся» поток как при простом join().
 */
export async function extractReadablePageText(
  page: PDFPageProxy,
  scale = 1,
): Promise<string> {
  const viewport = page.getViewport({ scale });
  const content = await page.getTextContent();
  const items: Item[] = [];

  let sumH = 0;
  let nH = 0;
  for (const raw of content.items) {
    if (!("str" in raw) || typeof raw.str !== "string" || !raw.str.trim()) continue;
    const tr = Util.transform(viewport.transform, raw.transform);
    const x = tr[4];
    const y = tr[5];
    const hasEOL = "hasEOL" in raw && !!raw.hasEOL;
    const hh = typeof raw.height === "number" && raw.height > 0 ? raw.height : 0;
    if (hh > 0) {
      sumH += hh;
      nH += 1;
    }
    items.push({
      str: raw.str,
      x,
      y,
      hasEOL,
      h: hh || 11,
    });
  }

  if (items.length === 0) return "";

  const avgH = nH > 0 ? sumH / nH : 11;
  /** Допуск «одна строка»: доля высоты шрифта */
  const lineTol = Math.max(2.8, avgH * 0.55);
  /** Разрыв между абзацами — заметно больше межстрочного интервала */
  const paraGap = Math.max(lineTol * 2.4, avgH * 1.65);

  items.sort((a, b) => (Math.abs(a.y - b.y) < lineTol ? a.x - b.x : a.y - b.y));

  const lines: Item[][] = [];
  let cur: Item[] = [];
  for (const it of items) {
    if (cur.length === 0) {
      cur.push(it);
      continue;
    }
    const ref = cur[0]!;
    if (Math.abs(it.y - ref.y) < lineTol) {
      cur.push(it);
    } else {
      lines.push(cur);
      cur = [it];
    }
  }
  if (cur.length) lines.push(cur);

  for (const line of lines) {
    line.sort((a, b) => a.x - b.x);
  }

  const lineMidY = (ln: Item[]) => ln.reduce((s, p) => s + p.y, 0) / ln.length;

  let out = "";
  for (let i = 0; i < lines.length; i++) {
    const ln = lines[i]!;
    if (i > 0) {
      const prev = lines[i - 1]!;
      const dy = lineMidY(ln) - lineMidY(prev);
      out += dy > paraGap ? "\n\n" : "\n";
    }
    const parts = ln.map((p) => p.str);
    out += parts.join(" ");
    const last = ln[ln.length - 1]!;
    if (last.hasEOL && !out.endsWith("\n")) out += "\n";
  }

  return (
    out
      .replace(/[ \t]+\n/g, "\n")
      .replace(/\n{3,}/g, "\n\n")
      .trim()
  );
}

/**
 * Делит на абзацы для отдельных запросов перевода; иначе один блок с сохранением переносов строк внутри.
 */
export function splitForTranslation(text: string): string[] {
  const normalized = text.replace(/\r\n/g, "\n").trim();
  if (!normalized) return [];
  const paras = normalized.split(/\n\n+/).map((p) => p.trim()).filter(Boolean);
  if (paras.length >= 2) return paras;
  return [normalized];
}
