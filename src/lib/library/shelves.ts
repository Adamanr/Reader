import type { BookMeta } from "$lib/types";

/** Активные полки книги: явный массив или наследие одной `shelfId`. */
export function effectiveShelfIds(meta: BookMeta | undefined | null): string[] {
  if (!meta) return ["default"];
  if (meta.shelfIds?.length) return [...meta.shelfIds];
  return [meta.shelfId || "default"];
}

export function bookOnShelf(meta: BookMeta | undefined, shelfId: string): boolean {
  return effectiveShelfIds(meta).includes(shelfId);
}
