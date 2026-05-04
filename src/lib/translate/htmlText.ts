/**
 * Извлекает текстовые узлы из HTML-фрагмента и позволяет подставить переводы в том же порядке.
 */
export function extractTextNodesHtml(html: string): {
  texts: string[];
  apply: (translations: string[]) => string;
} {
  const doc = new DOMParser().parseFromString(`<div id="tr-root">${html}</div>`, "text/html");
  const root = doc.getElementById("tr-root");
  if (!root) return { texts: [], apply: () => html };

  const textNodes: Text[] = [];
  const walker = doc.createTreeWalker(root, NodeFilter.SHOW_TEXT);
  let node: Node | null;
  while ((node = walker.nextNode())) {
    const t = node.textContent ?? "";
    if (t.trim().length > 0) textNodes.push(node as Text);
  }

  const texts = textNodes.map((n) => n.textContent ?? "");

  return {
    texts,
    apply(translations: string[]) {
      const n = Math.min(textNodes.length, translations.length);
      for (let i = 0; i < n; i++) textNodes[i].textContent = translations[i];
      return root.innerHTML;
    },
  };
}
