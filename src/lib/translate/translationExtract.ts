import {
  TRANSLATE_TAG_IN,
  TRANSLATE_TAG_OUT,
} from "./translationTags";

/** Убирает блоки рассуждений Ollama и др. (как в TranslateBooksWithLLMs extraction.py). */
export function stripThinkBlocks(s: string): string {
  let t = s.replace(/<think>[\s\S]*?<\/redacted_thinking>/gi, "");
  t = t.replace(/^[\s\S]*?<\/redacted_thinking>\s*/i, "");
  return t;
}

/** Снимает внешнюю обёртку ``` … ``` целиком с ответом. */
export function stripOuterMarkdownFence(s: string): string {
  const x = s.trim();
  const m = x.match(/^```\w*\s*\n([\s\S]*?)\n```\s*$/);
  return m?.[1] ?? s;
}

/**
 * Извлекает текст между тегами перевода (первое совпадение или строгие границы ответа).
 */
export function extractTranslationTagged(
  response: string,
  tagIn = TRANSLATE_TAG_IN,
  tagOut = TRANSLATE_TAG_OUT,
): string | null {
  if (!response?.trim()) return null;

  let t = response.trim();
  t = stripThinkBlocks(t);
  t = stripOuterMarkdownFence(t).trim();

  if (t.startsWith(tagIn) && t.endsWith(tagOut)) {
    return t.slice(tagIn.length, t.length - tagOut.length).trim();
  }

  const esc = (x: string) => x.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
  const re = new RegExp(`${esc(tagIn)}([\\s\\S]*?)${esc(tagOut)}`);
  const m = t.match(re);
  return m?.[1]?.trim() ?? null;
}
