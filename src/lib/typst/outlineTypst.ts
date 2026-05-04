/** Заголовки из разметки Typst (= …, == …) для боковой панели. */

export interface TypstOutlineItem {
  title: string;
  level: number;
  /** 1-based номер строки в book.typ */
  line: number;
}

/** Упрощённый заголовок для показа (без разметки внутри). */
function stripTypstInlineMarkup(s: string): string {
  let t = s.trim();
  t = t.replace(/\[[^\]]*\]/g, "");
  t = t.replace(/\*+/g, "");
  t = t.replace(/_+/g, "");
  t = t.replace(/`+/g, "");
  t = t.replace(/\\#/g, "#");
  return t.replace(/\s+/g, " ").trim() || "Без названия";
}

/**
 * Разбор оглавления: строки с ведущими `=` … `======` в начале (после пробелов),
 * не внутри блоков ``` … ```.
 */
export function extractTypstOutline(source: string): TypstOutlineItem[] {
  const lines = source.split(/\r?\n/);
  const out: TypstOutlineItem[] = [];
  let inFence = false;
  for (let i = 0; i < lines.length; i++) {
    const raw = lines[i]!;
    const trimmedStart = raw.trimStart();
    if (trimmedStart.startsWith("```")) {
      inFence = !inFence;
      continue;
    }
    if (inFence) continue;

    const m = /^(\s*)(=+)\s+(.*)$/.exec(raw);
    if (!m || !m[2] || m[2].length > 6) continue;
    const level = m[2].length;
    const rest = (m[3] ?? "").trim();
    if (!rest) continue;
    out.push({
      title: stripTypstInlineMarkup(rest),
      level,
      line: i + 1,
    });
  }
  return out;
}
