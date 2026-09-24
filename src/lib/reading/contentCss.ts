import { fontStack } from "$lib/reading/fonts";
import type { PagePalette } from "$lib/reading/palette";
import type { ReadingSettings } from "$lib/reading/settings.svelte";

/** CSS-переменные типографики для текста, который рендерим сами (FB2, перевод PDF). */
export function typographyVars(s: ReadingSettings, pal: PagePalette): Record<string, string> {
  return {
    "--rd-font": s.font === "publisher" ? fontStack("literata") : fontStack(s.font),
    "--rd-size": `${s.fontSize}px`,
    "--rd-line": String(s.lineHeight),
    "--rd-measure": `${s.measure}ch`,
    "--rd-margin": `${s.margin}px`,
    "--rd-align": s.align,
    "--rd-hyphens": s.hyphens ? "auto" : "manual",
    "--rd-para": `${s.paraSpacing}em`,
    "--rd-indent": s.indent ? "1.4em" : "0",
    "--rd-letter": `${s.letterSpacing}em`,
    "--rd-word": `${s.wordSpacing}em`,
    "--rd-bg": pal.bg,
    "--rd-text": pal.text,
    "--rd-muted": pal.muted,
    "--rd-accent": pal.accent,
  };
}

export function varsToStyle(vars: Record<string, string>): string {
  return Object.entries(vars)
    .map(([k, v]) => `${k}:${v}`)
    .join(";");
}

/**
 * Стили, которые вставляются внутрь iframe EPUB. Цвета — всегда (иначе тёмная
 * тема даёт чёрный текст на тёмном фоне); шрифт и ритм — если не выбран
 * «как в книге».
 */
export function epubContentCss(s: ReadingSettings, pal: PagePalette): string {
  const imp = "!important";
  const own = s.font !== "publisher";
  const typo = own
    ? `
    html, body { font-size: ${s.fontSize}px ${imp}; }
    body, body p, body div, body span, body li, body blockquote, body td {
      font-family: ${fontStack(s.font)} ${imp};
      line-height: ${s.lineHeight} ${imp};
      letter-spacing: ${s.letterSpacing}em ${imp};
      word-spacing: ${s.wordSpacing}em ${imp};
    }
    body p {
      text-align: ${s.align} ${imp};
      margin-top: 0 ${imp};
      margin-bottom: ${s.paraSpacing}em ${imp};
      text-indent: ${s.indent ? "1.4em" : "0"} ${imp};
    }
    body h1, body h2, body h3, body h4 { text-indent: 0 ${imp}; line-height: 1.25 ${imp}; font-family: ${fontStack(s.font)} ${imp}; }
    `
    : `html, body { font-size: ${s.fontSize}px ${imp}; }`;
  return `
    html { color-scheme: ${pal.dark ? "dark" : "light"}; }
    html { background: ${pal.bg} ${imp}; }
    body { background: transparent ${imp}; color: ${pal.text} ${imp}; }
    body * { color: inherit ${imp}; background-color: transparent ${imp}; border-color: ${pal.muted} ${imp}; }
    body a, body a * { color: ${pal.accent} ${imp}; }
    body { hyphens: ${s.hyphens ? "auto" : "manual"} ${imp}; -webkit-hyphens: ${s.hyphens ? "auto" : "manual"} ${imp}; }
    body img, body svg { max-width: 100% ${imp}; height: auto; ${pal.dark ? "filter: brightness(0.88);" : ""} }
    ::selection { background: color-mix(in srgb, ${pal.accent} 30%, transparent); }
    ${typo}
    .rd-focus-dim body p:not(.rd-focus-current), .rd-focus-dim body li:not(.rd-focus-current) { opacity: 0.32; transition: opacity .25s ease; }
    .rd-focus-current { opacity: 1 ${imp}; transition: opacity .25s ease; }
    .rd-search-hit { background: color-mix(in srgb, ${pal.accent} 35%, transparent) ${imp}; border-radius: 2px; }
    .rd-tts-current { background: color-mix(in srgb, ${pal.accent} 18%, transparent) ${imp}; border-radius: 3px; }
  `;
}
