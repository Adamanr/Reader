export type LibrarySortMode = "recent" | "added" | "progress" | "title" | "title_desc" | "importance";

const STORAGE_KEY = "reader.librarySort";
export const DEFAULT_LIBRARY_SORT: LibrarySortMode = "recent";

export function readLibrarySort(): LibrarySortMode {
  if (typeof localStorage === "undefined") return DEFAULT_LIBRARY_SORT;
  const v = localStorage.getItem(STORAGE_KEY);
  if (v && v in LIBRARY_SORT_LABELS) return v as LibrarySortMode;
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
  added: "Недавно добавленные",
  progress: "По прогрессу",
  title: "Название А→Я",
  title_desc: "Название Я→А",
  importance: "Важность",
};

export type LibraryViewMode = "grid" | "spines";
const VIEW_KEY = "reader.libraryView";

export function readLibraryView(): LibraryViewMode {
  try {
    return localStorage.getItem(VIEW_KEY) === "spines" ? "spines" : "grid";
  } catch {
    return "grid";
  }
}

export function writeLibraryView(mode: LibraryViewMode): void {
  try {
    localStorage.setItem(VIEW_KEY, mode);
  } catch {
    /* ignore */
  }
}
