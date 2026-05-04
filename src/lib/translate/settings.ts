import { browser } from "$app/environment";

export const STORAGE_TRANSLATE = "reader-translate-settings";

export const DEFAULT_TRANSLATE_API = "https://libretranslate.com";

export interface TranslateSettings {
  apiBase: string;
  apiKey: string;
}

export function getTranslateSettings(): TranslateSettings {
  if (!browser) return { apiBase: DEFAULT_TRANSLATE_API, apiKey: "" };
  try {
    const raw = localStorage.getItem(STORAGE_TRANSLATE);
    if (raw) {
      const j = JSON.parse(raw) as Record<string, unknown>;
      return {
        apiBase:
          typeof j.apiBase === "string" && j.apiBase.trim()
            ? j.apiBase.trim()
            : DEFAULT_TRANSLATE_API,
        apiKey: typeof j.apiKey === "string" ? j.apiKey : "",
      };
    }
  } catch {
    /* ignore */
  }
  return { apiBase: DEFAULT_TRANSLATE_API, apiKey: "" };
}

export function saveTranslateSettings(s: TranslateSettings): void {
  if (!browser) return;
  localStorage.setItem(STORAGE_TRANSLATE, JSON.stringify(s));
}

/** Языки интерфейса выбора (ISO-коды LibreTranslate). */
export const LANG_SOURCE: { code: string; label: string }[] = [
  { code: "auto", label: "Автоопределение" },
  { code: "en", label: "English" },
  { code: "de", label: "Deutsch" },
  { code: "fr", label: "Français" },
  { code: "es", label: "Español" },
  { code: "it", label: "Italiano" },
  { code: "pt", label: "Português" },
  { code: "pl", label: "Polski" },
  { code: "uk", label: "Українська" },
  { code: "ru", label: "Русский" },
];

export const LANG_TARGET: { code: string; label: string }[] = [
  { code: "ru", label: "Русский" },
  { code: "uk", label: "Українська" },
  { code: "en", label: "English" },
  { code: "de", label: "Deutsch" },
  { code: "fr", label: "Français" },
  { code: "es", label: "Español" },
  { code: "pl", label: "Polski" },
  { code: "sq", label: "Albanian" },
];
