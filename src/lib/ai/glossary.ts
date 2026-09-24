import type { TextChunk } from "$lib/types";
import { ask, assistantReady } from "$lib/ai/assistant.svelte";
import { pathKey, storeRead, storeWrite } from "$lib/storage/store";

/**
 * «Кто это?» — объяснение персонажа или понятия только по прочитанному.
 * Ответы копятся в словаре книги вместе с позицией, на которой были даны.
 */

export interface Mention {
  label: string;
  sentence: string;
}

export interface GlossaryEntry {
  term: string;
  answer: string;
  /** Прогресс чтения, на котором дан ответ (0…1) */
  progress: number;
  mentions: number;
  at: string;
  /** Ответ составлен моделью (иначе — просто упоминания) */
  ai: boolean;
}

export type Glossary = Record<string, GlossaryEntry>;

export function termKey(term: string): string {
  return term.toLocaleLowerCase("ru").replace(/ё/g, "е").replace(/[^\p{L}\p{N}\s-]/gu, "").trim();
}

/** Основа слова: падежи и склонения имён меняют хвост («Анна», «Анны», «Анной»). */
function stemOf(word: string): string {
  const w = termKey(word);
  if (w.length <= 4) return w;
  if (w.length <= 6) return w.slice(0, -1);
  return w.slice(0, -2);
}

export function findMentions(chunks: TextChunk[], term: string, limit = 32): Mention[] {
  const words = termKey(term).split(/\s+/).filter(Boolean);
  if (!words.length) return [];
  const stems = words.map(stemOf);
  const all: Mention[] = [];
  for (const c of chunks) {
    const sentences = c.text.match(/[^.!?…\n]+[.!?…]*/g) ?? [];
    for (const s of sentences) {
      const low = s.toLocaleLowerCase("ru").replace(/ё/g, "е");
      const hit = stems.every((st) => new RegExp(`(^|[^\\p{L}])${st.replace(/[.*+?^${}()|[\]\\]/g, "\\$&")}`, "u").test(low));
      if (hit) all.push({ label: c.label, sentence: s.trim() });
    }
  }
  if (all.length <= limit) return all;
  // Первые появления, равномерная выборка из середины и самые свежие.
  const head = all.slice(0, 6);
  const tail = all.slice(-12);
  const middle = all.slice(6, -12);
  const step = middle.length / (limit - head.length - tail.length);
  const mid: Mention[] = [];
  for (let i = 0; i < limit - head.length - tail.length; i++) mid.push(middle[Math.floor(i * step)]!);
  return [...head, ...mid, ...tail];
}

export async function loadGlossary(path: string): Promise<Glossary> {
  return storeRead<Glossary>(`glossary-${await pathKey(path)}`, {});
}

export async function saveGlossary(path: string, g: Glossary): Promise<void> {
  await storeWrite(`glossary-${await pathKey(path)}`, g);
}

export async function explainTerm(opts: {
  term: string;
  bookTitle: string;
  mentions: Mention[];
}): Promise<{ answer: string; ai: boolean }> {
  const { term, bookTitle, mentions } = opts;
  if (!mentions.length) {
    return { answer: `В прочитанной части «${term}» ещё не встречалось.`, ai: false };
  }
  if (!assistantReady()) {
    const first = mentions[0]!;
    const last = mentions[mentions.length - 1]!;
    const answer =
      `Впервые: «${first.sentence}» (${first.label}).` +
      (mentions.length > 1 ? `\n\nПоследнее упоминание: «${last.sentence}» (${last.label}).` : "");
    return { answer, ai: false };
  }
  const list = mentions.map((m) => `[${m.label}] ${m.sentence}`).join("\n");
  const answer = await ask(
    "Ты помогаешь читателю вспомнить персонажей и понятия книги. Используй ТОЛЬКО приведённые цитаты — " +
      "это всё, что читатель уже прочитал. Не упоминай ничего, чего в цитатах нет, никаких спойлеров. Пиши по-русски.",
    `Книга: «${bookTitle}». Кто или что такое «${term}»?\n\nВсе упоминания в прочитанном:\n${list}\n\n` +
      "Ответь в 2–4 предложениях: кто это, как связан с другими героями, что с ним происходило. " +
      "Если данных мало — так и скажи.",
    400,
  );
  return { answer, ai: true };
}
