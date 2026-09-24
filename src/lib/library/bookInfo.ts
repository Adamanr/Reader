import type { BookMeta, ReadingStatus } from "$lib/types";

export type ShelfStatus = ReadingStatus | "new";

export const STATUS_LABELS: Record<ShelfStatus, string> = {
  reading: "Читаю",
  want: "Хочу прочитать",
  new: "Не начата",
  done: "Прочитано",
  dropped: "Отложено",
};

export function titleFromPath(path: string) {
  const i = path.lastIndexOf("/");
  return i >= 0 ? path.slice(i + 1) : path;
}

export function bookTitle(path: string, meta: BookMeta | undefined | null): string {
  return meta?.title?.trim() || titleFromPath(path);
}

/** Прогресс 0…1 для любого формата (у старых записей — из страниц PDF). */
export function bookProgress(meta: BookMeta | undefined | null): number | null {
  if (!meta) return null;
  if (meta.progress != null && Number.isFinite(meta.progress)) return Math.min(1, Math.max(0, meta.progress));
  const t = meta.lastReadPdfTotal;
  const p = meta.lastReadPdfPage;
  if (t && p) return Math.min(1, p / t);
  return null;
}

export function effectiveStatus(meta: BookMeta | undefined | null): ShelfStatus {
  if (meta?.status) return meta.status;
  const p = bookProgress(meta);
  if (p != null && p >= 0.985) return "done";
  if (meta?.lastOpenedAt) return "reading";
  return "new";
}

function lastTouchedMs(meta: BookMeta | undefined | null): number | null {
  const opened = meta?.lastOpenedAt ? Date.parse(meta.lastOpenedAt) : NaN;
  if (Number.isFinite(opened)) return opened;
  return meta?.addedAtMs ?? null;
}

/**
 * «Пыль на полке»: книги, которых давно не касались, понемногу выцветают.
 * Дочитанные не пылятся — они своё отработали.
 */
export function dustLevel(meta: BookMeta | undefined | null, now = Date.now()): number {
  if (!meta || effectiveStatus(meta) === "done") return 0;
  const last = lastTouchedMs(meta);
  if (last == null) return 0;
  const days = (now - last) / 86_400_000;
  return Math.max(0, Math.min(1, (days - 30) / 150));
}

export function dustLabel(meta: BookMeta | undefined | null, now = Date.now()): string {
  const last = lastTouchedMs(meta);
  if (last == null) return "";
  const days = Math.floor((now - last) / 86_400_000);
  if (days < 30) return "";
  const months = Math.floor(days / 30);
  const opened = !!meta?.lastOpenedAt;
  const span =
    months >= 12
      ? `${Math.floor(months / 12)} г.`
      : `${months} ${months === 1 ? "месяц" : months < 5 ? "месяца" : "месяцев"}`;
  return opened ? `Не открывалась ${span}` : `Ждёт на полке ${span}`;
}

/** Стабильное псевдослучайное число из строки (толщина корешка и т. п.). */
export function hashNum(s: string): number {
  let h = 2166136261;
  for (let i = 0; i < s.length; i++) {
    h ^= s.charCodeAt(i);
    h = Math.imul(h, 16777619);
  }
  return (h >>> 0) / 4294967295;
}

export function pluralBooks(n: number): string {
  const d = n % 10;
  const dd = n % 100;
  if (d === 1 && dd !== 11) return "книга";
  if (d >= 2 && d <= 4 && (dd < 12 || dd > 14)) return "книги";
  return "книг";
}
