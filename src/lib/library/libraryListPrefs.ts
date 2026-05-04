export type LibrarySortMode = "recent" | "title" | "title_desc" | "importance";

const STORAGE_KEY = "reader.librarySort";
export const DEFAULT_LIBRARY_SORT: LibrarySortMode = "recent";

export function readLibrarySort(): LibrarySortMode {
  if (typeof localStorage === "undefined") return DEFAULT_LIBRARY_SORT;
  const v = localStorage.getItem(STORAGE_KEY);
  if (v === "recent" || v === "title" || v === "title_desc" || v === "importance") return v;
  return DEFAULT_LIBRARY_SORT;
}

export function writeLibrarySort(mode: LibrarySortMode): void {
  try {
    localStorage.setItem(STORAGE_KEY, mode);
  } catch {
    /* ignore */
  }
}

export const LIBRARY_SORT_LABELS: Record<LibrarySortMode, string> = {
  recent: "Недавно открытые",
  title: "Название А→Я",
  title_desc: "Название Я→А",
  importance: "Важность",
};
