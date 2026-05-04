/** Те же элементы, что и у pdf.js TextLayer / textContentItemsStr (порядок stream). */
export function extractTextItemStrings(content: { items: unknown[] }): string[] {
  const out: string[] = [];
  for (const item of content.items) {
    if (
      item &&
      typeof item === "object" &&
      "str" in item &&
      typeof (item as { str?: unknown }).str === "string"
    ) {
      out.push((item as { str: string }).str);
    }
  }
  return out;
}
