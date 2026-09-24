/**
 * Привязка фрагментов к тексту контейнера по смещению в `textContent`.
 * Используется для выделений и поиска в FB2 и в текстовом слое PDF.
 */

function textNodes(root: Node): Text[] {
  const out: Text[] = [];
  const doc = root.ownerDocument ?? (root as Document);
  const walker = doc.createTreeWalker(root, NodeFilter.SHOW_TEXT);
  let n = walker.nextNode();
  while (n) {
    out.push(n as Text);
    n = walker.nextNode();
  }
  return out;
}

/** Смещение точки (узел + offset) от начала текста `root`. */
export function offsetOfPoint(root: Node, node: Node, offset: number): number | null {
  const doc = root.ownerDocument ?? (root as Document);
  const r = doc.createRange();
  try {
    r.setStart(root, 0);
    r.setEnd(node, offset);
  } catch {
    return null;
  }
  return r.toString().length;
}

export function rangeFromOffsets(root: Node, start: number, end: number): Range | null {
  const doc = root.ownerDocument ?? (root as Document);
  const nodes = textNodes(root);
  let pos = 0;
  let startNode: Text | null = null;
  let startOff = 0;
  let endNode: Text | null = null;
  let endOff = 0;
  for (const t of nodes) {
    const len = t.data.length;
    if (!startNode && start <= pos + len) {
      startNode = t;
      startOff = Math.max(0, start - pos);
    }
    if (startNode && end <= pos + len) {
      endNode = t;
      endOff = Math.max(0, end - pos);
      break;
    }
    pos += len;
  }
  if (!startNode || !endNode) return null;
  const r = doc.createRange();
  r.setStart(startNode, Math.min(startOff, startNode.data.length));
  r.setEnd(endNode, Math.min(endOff, endNode.data.length));
  return r;
}

/** Ищет `needle` в тексте `root`, выбирая вхождение ближе всего к подсказке. */
export function locateText(root: Node, needle: string, hint = 0): { start: number; end: number } | null {
  const hay = root.textContent ?? "";
  const n = needle.trim();
  if (!n) return null;
  let best = -1;
  let from = 0;
  for (;;) {
    const i = hay.indexOf(n, from);
    if (i < 0) break;
    if (best < 0 || Math.abs(i - hint) < Math.abs(best - hint)) best = i;
    from = i + 1;
  }
  if (best >= 0) return { start: best, end: best + n.length };
  // Выделение могло захватить переносы строк иначе, чем textContent.
  const compact = n.replace(/\s+/g, " ");
  const words = compact.split(" ");
  if (words.length < 2) return null;
  const head = words.slice(0, 3).join(" ");
  const i = hay.replace(/\s+/g, " ").indexOf(head);
  if (i < 0) return null;
  return { start: i, end: Math.min(hay.length, i + compact.length) };
}

/** Оборачивает все текстовые куски диапазона в элементы (выделение не ломает разметку). */
export function wrapRange(range: Range, make: () => HTMLElement): HTMLElement[] {
  const root = range.commonAncestorContainer;
  const doc = range.startContainer.ownerDocument!;
  const nodes =
    root.nodeType === Node.TEXT_NODE ? [root as Text] : textNodes(root).filter((t) => range.intersectsNode(t));
  const out: HTMLElement[] = [];
  for (const t of nodes) {
    let s = 0;
    let e = t.data.length;
    if (t === range.startContainer) s = range.startOffset;
    if (t === range.endContainer) e = range.endOffset;
    if (e <= s) continue;
    if (!t.data.slice(s, e).trim()) continue;
    let target = t;
    if (s > 0) target = target.splitText(s);
    if (e - s < target.data.length) target.splitText(e - s);
    const el = make();
    target.parentNode?.insertBefore(el, target);
    el.appendChild(target);
    out.push(el);
  }
  void doc;
  return out;
}

export function unwrapAll(root: ParentNode, selector: string) {
  for (const el of Array.from(root.querySelectorAll(selector))) {
    const parent = el.parentNode;
    if (!parent) continue;
    while (el.firstChild) parent.insertBefore(el.firstChild, el);
    parent.removeChild(el);
    parent.normalize();
  }
}

/** Фрагмент вокруг совпадения для списка результатов поиска. */
export function excerptAround(text: string, start: number, end: number, radius = 48) {
  const a = Math.max(0, start - radius);
  const b = Math.min(text.length, end + radius);
  return {
    before: (a > 0 ? "…" : "") + text.slice(a, start).replace(/\s+/g, " "),
    match: text.slice(start, end),
    after: text.slice(end, b).replace(/\s+/g, " ") + (b < text.length ? "…" : ""),
  };
}

/** Все вхождения запроса без учёта регистра (и ё/е). */
export function findAll(text: string, query: string, limit = 200): number[] {
  const norm = (s: string) => s.toLocaleLowerCase("ru").replace(/ё/g, "е");
  const hay = norm(text);
  const q = norm(query.trim());
  if (!q) return [];
  const out: number[] = [];
  let from = 0;
  while (out.length < limit) {
    const i = hay.indexOf(q, from);
    if (i < 0) break;
    out.push(i);
    from = i + q.length;
  }
  return out;
}
