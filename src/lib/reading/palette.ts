import type { PageThemeId } from "$lib/reading/settings.svelte";

export interface PagePalette {
  bg: string;
  text: string;
  muted: string;
  accent: string;
  dark: boolean;
}

export interface CoverTone {
  h: number;
  s: number;
  l: number;
}

export const PAGE_THEME_OPTIONS: { id: PageThemeId; label: string; swatch: string; ink: string }[] = [
  { id: "app", label: "Как тема", swatch: "var(--elevated-soft)", ink: "var(--text-soft)" },
  { id: "paper", label: "Бумага", swatch: "#fbfaf6", ink: "#23211d" },
  { id: "sepia", label: "Сепия", swatch: "#f3e9d2", ink: "#4a3a28" },
  { id: "graphite", label: "Графит", swatch: "#2a2c30", ink: "#dcd9d2" },
  { id: "oled", label: "Ночь", swatch: "#000000", ink: "#bdbdbd" },
  { id: "cover", label: "Обложка", swatch: "conic-gradient(#e7c9a9, #b9d2c2, #c6c3e8, #e7c9a9)", ink: "#2c2840" },
];

function cssVar(name: string, fallback: string): string {
  if (typeof document === "undefined") return fallback;
  const v = getComputedStyle(document.documentElement).getPropertyValue(name).trim();
  return v || fallback;
}

export function appIsDark(): boolean {
  if (typeof document === "undefined") return false;
  return document.documentElement.dataset.theme === "dark";
}

/** Цвета страницы в явном виде (нужны для iframe EPUB, где нет CSS-переменных приложения). */
export function pagePalette(theme: PageThemeId, tone: CoverTone | null): PagePalette {
  switch (theme) {
    case "paper":
      return { bg: "#fbfaf6", text: "#23211d", muted: "#6f6a60", accent: "#8a5a2b", dark: false };
    case "sepia":
      return { bg: "#f3e9d2", text: "#4a3a28", muted: "#80705a", accent: "#9a5b2e", dark: false };
    case "graphite":
      return { bg: "#2a2c30", text: "#dcd9d2", muted: "#9c9a94", accent: "#d9b77e", dark: true };
    case "oled":
      return { bg: "#000000", text: "#bdbdbd", muted: "#7c7c7c", accent: "#c7a86a", dark: true };
    case "cover": {
      if (tone) {
        const dark = appIsDark();
        const s = Math.min(tone.s, 34);
        return dark
          ? {
              bg: `hsl(${tone.h} ${Math.min(s, 22)}% 11%)`,
              text: `hsl(${tone.h} 14% 86%)`,
              muted: `hsl(${tone.h} 10% 62%)`,
              accent: `hsl(${tone.h} 55% 74%)`,
              dark: true,
            }
          : {
              bg: `hsl(${tone.h} ${s}% 95%)`,
              text: `hsl(${tone.h} 26% 15%)`,
              muted: `hsl(${tone.h} 14% 42%)`,
              accent: `hsl(${tone.h} 48% 36%)`,
              dark: false,
            };
      }
      return pagePalette("app", null);
    }
    default:
      return {
        bg: cssVar("--elevated-soft", "#fcfbff"),
        text: cssVar("--text-soft", "#2c2840"),
        muted: cssVar("--muted", "#706b82"),
        accent: cssVar("--accent-2", "#66548f"),
        dark: appIsDark(),
      };
  }
}

/** Акцент интерфейса по обложке («живая обложка»). */
export function coverAccentVars(tone: CoverTone, dark: boolean): Record<string, string> {
  const s = Math.max(28, Math.min(tone.s, 62));
  return dark
    ? {
        "--accent": `hsl(${tone.h} ${s}% 70%)`,
        "--accent-2": `hsl(${tone.h} ${s}% 84%)`,
        "--cover-glow": `hsl(${tone.h} ${s}% 45% / 0.22)`,
      }
    : {
        "--accent": `hsl(${tone.h} ${s}% 68%)`,
        "--accent-2": `hsl(${tone.h} ${Math.min(s, 45)}% 36%)`,
        "--cover-glow": `hsl(${tone.h} ${s}% 70% / 0.28)`,
      };
}

function rgbToHsl(r: number, g: number, b: number): CoverTone {
  r /= 255;
  g /= 255;
  b /= 255;
  const max = Math.max(r, g, b);
  const min = Math.min(r, g, b);
  const l = (max + min) / 2;
  if (max === min) return { h: 30, s: 0, l: l * 100 };
  const d = max - min;
  const s = l > 0.5 ? d / (2 - max - min) : d / (max + min);
  let h = 0;
  if (max === r) h = (g - b) / d + (g < b ? 6 : 0);
  else if (max === g) h = (b - r) / d + 2;
  else h = (r - g) / d + 4;
  return { h: Math.round(h * 60), s: Math.round(s * 100), l: Math.round(l * 100) };
}

const toneCache = new Map<string, CoverTone | null>();

/**
 * Характерный цвет обложки: группируем пиксели по оттенку с весом насыщенности,
 * игнорируя почти белое и почти чёрное.
 */
export async function coverTone(url: string | null): Promise<CoverTone | null> {
  if (!url) return null;
  const cached = toneCache.get(url);
  if (cached !== undefined) return cached;
  const img = new Image();
  img.src = url;
  try {
    await img.decode();
  } catch {
    toneCache.set(url, null);
    return null;
  }
  const w = 32;
  const h = 48;
  const canvas = document.createElement("canvas");
  canvas.width = w;
  canvas.height = h;
  const ctx = canvas.getContext("2d", { willReadFrequently: true });
  if (!ctx) return null;
  ctx.drawImage(img, 0, 0, w, h);
  const data = ctx.getImageData(0, 0, w, h).data;
  const buckets = Array.from({ length: 12 }, () => ({ weight: 0, r: 0, g: 0, b: 0 }));
  let greyR = 0;
  let greyG = 0;
  let greyB = 0;
  let greyN = 0;
  for (let i = 0; i < data.length; i += 4) {
    const r = data[i]!;
    const g = data[i + 1]!;
    const b = data[i + 2]!;
    const hsl = rgbToHsl(r, g, b);
    greyR += r;
    greyG += g;
    greyB += b;
    greyN++;
    if (hsl.l < 12 || hsl.l > 92 || hsl.s < 18) continue;
    const bucket = buckets[Math.floor(hsl.h / 30) % 12]!;
    const wgt = (hsl.s / 100) * (1 - Math.abs(hsl.l - 50) / 60);
    bucket.weight += wgt;
    bucket.r += r * wgt;
    bucket.g += g * wgt;
    bucket.b += b * wgt;
  }
  const best = buckets.reduce((a, b) => (b.weight > a.weight ? b : a));
  let tone: CoverTone;
  if (best.weight > 1.5) {
    tone = rgbToHsl(best.r / best.weight, best.g / best.weight, best.b / best.weight);
  } else {
    const avg = rgbToHsl(greyR / greyN, greyG / greyN, greyB / greyN);
    tone = { h: avg.s > 4 ? avg.h : 35, s: Math.min(avg.s, 14), l: avg.l };
  }
  toneCache.set(url, tone);
  return tone;
}
