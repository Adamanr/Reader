import { browser } from "$app/environment";

export type ThemeId = "light" | "dark" | "sepia" | "forest" | "ocean";

export const THEME_STORAGE = "reader-theme";

export const THEMES: { id: ThemeId; label: string }[] = [
  { id: "light", label: "Сон в облаках" },
  { id: "dark", label: "Полночь" },
  { id: "sepia", label: "Розовый закат" },
  { id: "forest", label: "Тихий сад" },
  { id: "ocean", label: "Утреннее небо" },
];

export function isThemeId(v: string | null): v is ThemeId {
  return v === "light" || v === "dark" || v === "sepia" || v === "forest" || v === "ocean";
}

export function getStoredTheme(): ThemeId {
  if (!browser) return "light";
  try {
    const v = localStorage.getItem(THEME_STORAGE);
    if (isThemeId(v)) return v;
  } catch {
    /* ignore */
  }
  return "light";
}

export function applyTheme(id: ThemeId): void {
  if (!browser) return;
  document.documentElement.dataset.theme = id;
  try {
    localStorage.setItem(THEME_STORAGE, id);
  } catch {
    /* ignore */
  }
}

/** Вызов из корневого layout при монтировании клиента */
export function initThemeFromStorage(): void {
  if (!browser) return;
  applyTheme(getStoredTheme());
}
