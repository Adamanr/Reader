/** Единый разбор формата книги по пути (вложенные папки, разделители Windows). */

export type BookFormat = "pdf" | "epub" | "fb2" | "typst";

export function normalizeBookPath(path: string): string {
  return path.trim().replace(/\\/g, "/");
}

/** Расширение имени файла (последний сегмент пути), без точки, lower-case. */
export function getPathExtension(path: string): string {
  const n = normalizeBookPath(path);
  const slash = n.lastIndexOf("/");
  const base = slash >= 0 ? n.slice(slash + 1) : n;
  const dot = base.lastIndexOf(".");
  if (dot <= 0 || dot >= base.length - 1) return "";
  return base.slice(dot + 1).toLowerCase();
}

export function getBookFormat(path: string): BookFormat | null {
  const ext = getPathExtension(path);
  if (ext === "pdf") return "pdf";
  if (ext === "epub") return "epub";
  if (ext === "fb2") return "fb2";
  if (ext === "typ") return "typst";

  /* Запасной вариант по имени файла (необычные точки в имени, редкие кодировки пути). */
  const n = normalizeBookPath(path);
  const slash = n.lastIndexOf("/");
  const base = (slash >= 0 ? n.slice(slash + 1) : n).toLowerCase();
  if (base.endsWith(".fb2")) return "fb2";
  if (base.endsWith(".epub")) return "epub";
  if (base.endsWith(".pdf")) return "pdf";
  if (base.endsWith(".typ")) return "typst";
  return null;
}

export function formatBadgeLabel(format: BookFormat): string {
  if (format === "typst") return "TYPST";
  return format.toUpperCase();
}
