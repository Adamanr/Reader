/**
 * Встроенные шрифты для чтения. CSS подключается как строка (`?inline`), чтобы
 * тот же набор @font-face можно было вставить и в iframe EPUB.
 */
import literata from "@fontsource-variable/literata/wght.css?inline";
import literataItalic from "@fontsource-variable/literata/wght-italic.css?inline";
import lora from "@fontsource-variable/lora/wght.css?inline";
import loraItalic from "@fontsource-variable/lora/wght-italic.css?inline";
import ptSerif400 from "@fontsource/pt-serif/400.css?inline";
import ptSerif400i from "@fontsource/pt-serif/400-italic.css?inline";
import ptSerif700 from "@fontsource/pt-serif/700.css?inline";
import ptSans400 from "@fontsource/pt-sans/400.css?inline";
import ptSans400i from "@fontsource/pt-sans/400-italic.css?inline";
import ptSans700 from "@fontsource/pt-sans/700.css?inline";

export type FontId = "literata" | "lora" | "ptserif" | "ptsans" | "system" | "publisher";

export const FONT_OPTIONS: { id: FontId; label: string; stack: string; sample: string }[] = [
  {
    id: "literata",
    label: "Literata",
    stack: `"Literata Variable", Literata, Georgia, serif`,
    sample: "Книжная антиква для долгого чтения",
  },
  {
    id: "lora",
    label: "Lora",
    stack: `"Lora Variable", Lora, Georgia, serif`,
    sample: "Мягкая, тёплая, с каллиграфией",
  },
  {
    id: "ptserif",
    label: "PT Serif",
    stack: `"PT Serif", Georgia, serif`,
    sample: "Классика русской типографики",
  },
  {
    id: "ptsans",
    label: "PT Sans",
    stack: `"PT Sans", "Reader Sans", system-ui, sans-serif`,
    sample: "Гротеск: чисто и спокойно",
  },
  {
    id: "system",
    label: "Системный",
    stack: `system-ui, -apple-system, "Segoe UI", sans-serif`,
    sample: "Шрифт вашей системы",
  },
  {
    id: "publisher",
    label: "Как в книге",
    stack: "inherit",
    sample: "Оформление издателя",
  },
];

export function fontStack(id: FontId): string {
  return FONT_OPTIONS.find((f) => f.id === id)?.stack ?? FONT_OPTIONS[0]!.stack;
}

let absoluteCss: string | null = null;

/**
 * Все @font-face с абсолютными адресами файлов: iframe EPUB открывается
 * из blob:, и относительные/корневые ссылки там не работают.
 */
export function fontFaceCss(): string {
  if (absoluteCss != null) return absoluteCss;
  const all = [
    literata,
    literataItalic,
    lora,
    loraItalic,
    ptSerif400,
    ptSerif400i,
    ptSerif700,
    ptSans400,
    ptSans400i,
    ptSans700,
  ].join("\n");
  const origin = typeof location !== "undefined" ? location.origin : "";
  absoluteCss = all.replace(/url\((['"]?)\/(?!\/)/g, `url($1${origin}/`);
  return absoluteCss;
}

/** Подключает шрифты в основной документ (FB2, перевод PDF, панели). */
export function ensureAppFonts(): void {
  if (typeof document === "undefined") return;
  if (document.getElementById("reader-font-faces")) return;
  const style = document.createElement("style");
  style.id = "reader-font-faces";
  style.textContent = fontFaceCss();
  document.head.appendChild(style);
}
