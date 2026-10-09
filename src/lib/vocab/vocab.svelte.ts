import { debouncedStoreWriter, storeRead } from "$lib/storage/store";

/**
 * «Мои слова»: слова из книг с переводом и живым контекстом.
 * Повторение по интервалам (упрощённый SM-2).
 */

export interface VocabCard {
  id: string;
  word: string;
  translation: string;
  /** Пояснение по контексту от помощника */
  explanation?: string;
  context: string;
  bookPath: string;
  bookTitle: string;
  addedAt: string;
  due: string;
  interval: number;
  ease: number;
  reps: number;
  lapses: number;
}

export const vocab = $state<{ cards: VocabCard[]; loaded: boolean }>({ cards: [], loaded: false });

const writer = debouncedStoreWriter("vocab", 600);

export async function loadVocab(): Promise<VocabCard[]> {
  if (vocab.loaded) return vocab.cards;
  vocab.cards = await storeRead<VocabCard[]>("vocab", []);
  vocab.loaded = true;
  return vocab.cards;
}

function persist() {
  writer.write(vocab.cards);
}

export function flushVocab(): Promise<void> {
  return writer.flush();
}

/** Книга сменила путь (переименована/перенесена) — карточки слов ссылаются на новый. */
export function relocateVocabBook(from: string, to: string) {
  if (!vocab.loaded) return; // файл уже поправил бэкенд, загрузится свежим
  if (!vocab.cards.some((c) => c.bookPath === from)) return;
  vocab.cards = vocab.cards.map((c) => (c.bookPath === from ? { ...c, bookPath: to } : c));
  persist();
}

export function hasWord(word: string): boolean {
  const w = word.trim().toLocaleLowerCase();
  return vocab.cards.some((c) => c.word.toLocaleLowerCase() === w);
}

export async function addWord(card: Omit<VocabCard, "id" | "addedAt" | "due" | "interval" | "ease" | "reps" | "lapses">) {
  await loadVocab();
  if (hasWord(card.word)) return;
  const now = new Date().toISOString();
  vocab.cards = [
    {
      ...card,
      id: crypto.randomUUID(),
      addedAt: now,
      due: now,
      interval: 0,
      ease: 2.5,
      reps: 0,
      lapses: 0,
    },
    ...vocab.cards,
  ];
  persist();
}

export function removeWord(id: string) {
  vocab.cards = vocab.cards.filter((c) => c.id !== id);
  persist();
}

export type Grade = 0 | 1 | 2 | 3; // забыл, трудно, помню, легко

/** Следующий показ карточки. */
export function review(id: string, grade: Grade) {
  vocab.cards = vocab.cards.map((c) => {
    if (c.id !== id) return c;
    let { interval, ease, reps, lapses } = c;
    if (grade === 0) {
      reps = 0;
      lapses += 1;
      interval = 0;
      ease = Math.max(1.3, ease - 0.2);
    } else {
      reps += 1;
      if (reps === 1) interval = grade === 3 ? 3 : 1;
      else if (reps === 2) interval = grade === 1 ? 3 : 6;
      else interval = Math.round(interval * (grade === 1 ? 1.2 : grade === 3 ? ease * 1.3 : ease));
      ease = Math.max(1.3, ease + (grade === 1 ? -0.15 : grade === 3 ? 0.15 : 0));
    }
    const due = new Date();
    if (interval === 0) due.setMinutes(due.getMinutes() + 10);
    else due.setDate(due.getDate() + interval);
    return { ...c, interval, ease, reps, lapses, due: due.toISOString() };
  });
  persist();
}

export function dueCards(now = Date.now()): VocabCard[] {
  return vocab.cards.filter((c) => Date.parse(c.due) <= now);
}

/** Предложение из контекста, в котором встретилось слово. */
export function sentenceAround(context: string, word: string): string {
  const text = context.replace(/\s+/g, " ").trim();
  const i = text.toLocaleLowerCase().indexOf(word.trim().toLocaleLowerCase());
  if (i < 0) return text.slice(0, 240);
  let start = i;
  while (start > 0 && !/[.!?…]/.test(text[start - 1]!)) start--;
  let end = i + word.length;
  while (end < text.length && !/[.!?…]/.test(text[end]!)) end++;
  const s = text.slice(start, Math.min(text.length, end + 1)).trim();
  return s.length > 320 ? text.slice(Math.max(0, i - 140), i + word.length + 140).trim() : s;
}
