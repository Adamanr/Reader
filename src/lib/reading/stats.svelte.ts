import { debouncedStoreWriter, storeRead } from "$lib/storage/store";

/**
 * «Ритм чтения»: активное время по дням и книгам, личная скорость.
 * Время считается только когда окно видно и читатель что-то делал
 * в последние полторы минуты — открытая и забытая книга не накручивает статистику.
 */

export interface DayStats {
  /** Активное время, мс */
  ms: number;
  chars: number;
  pages: number;
  books: string[];
}

export interface BookStats {
  ms: number;
  sessions: number;
  firstAt: string;
  lastAt: string;
}

export interface ReadingStats {
  days: Record<string, DayStats>;
  books: Record<string, BookStats>;
  speed: {
    /** Символов в минуту (EPUB/FB2) */
    charsPerMin: number;
    /** Секунд на страницу PDF */
    secPerPage: number;
    samples: number;
  };
}

const DEFAULT_STATS: ReadingStats = {
  days: {},
  books: {},
  speed: { charsPerMin: 1100, secPerPage: 90, samples: 0 },
};

export const stats = $state<{ s: ReadingStats; loaded: boolean }>({ s: DEFAULT_STATS, loaded: false });

const writer = debouncedStoreWriter("reading-stats", 4000);

export async function loadStats(): Promise<ReadingStats> {
  if (stats.loaded) return stats.s;
  const s = await storeRead<ReadingStats>("reading-stats", DEFAULT_STATS);
  stats.s = { ...DEFAULT_STATS, ...s, speed: { ...DEFAULT_STATS.speed, ...(s.speed ?? {}) } };
  stats.loaded = true;
  return stats.s;
}

export function dayKey(d = new Date()): string {
  const y = d.getFullYear();
  const m = String(d.getMonth() + 1).padStart(2, "0");
  const day = String(d.getDate()).padStart(2, "0");
  return `${y}-${m}-${day}`;
}

function persist() {
  writer.write(stats.s);
}

export function flushStats(): Promise<void> {
  return writer.flush();
}

/** Книга сменила путь (переименована/перенесена) — переносим её статистику. */
export function relocateStatsBook(from: string, to: string) {
  if (!stats.loaded) return; // файл уже поправил бэкенд, загрузится свежим
  const s = stats.s;
  const books = { ...s.books };
  if (books[from]) {
    books[to] ??= books[from];
    delete books[from];
  }
  const days: Record<string, DayStats> = {};
  for (const [k, d] of Object.entries(s.days)) {
    days[k] = d.books.includes(from)
      ? { ...d, books: [...d.books.filter((b) => b !== from && b !== to), to] }
      : d;
  }
  stats.s = { ...s, books, days };
  persist();
}

const TICK_MS = 10_000;
const IDLE_MS = 90_000;

/** Одна сессия чтения книги. */
export function startSession(bookPath: string) {
  let lastActivity = Date.now();
  let lastPage: number | null = null;
  let lastCharsLeft: number | null = null;
  let sessionMs = 0;
  let sessionChars = 0;
  let sessionPages = 0;
  let counted = false;

  void loadStats();

  const markActive = () => {
    lastActivity = Date.now();
  };

  const tick = () => {
    if (!stats.loaded) return;
    if (document.visibilityState !== "visible") return;
    if (Date.now() - lastActivity > IDLE_MS) return;
    const key = dayKey();
    const s = stats.s;
    const day = s.days[key] ?? { ms: 0, chars: 0, pages: 0, books: [] };
    day.ms += TICK_MS;
    if (!day.books.includes(bookPath)) day.books = [...day.books, bookPath];
    const now = new Date().toISOString();
    const book = s.books[bookPath] ?? { ms: 0, sessions: 0, firstAt: now, lastAt: now };
    book.ms += TICK_MS;
    book.lastAt = now;
    if (!counted) {
      book.sessions += 1;
      counted = true;
    }
    stats.s = { ...s, days: { ...s.days, [key]: day }, books: { ...s.books, [bookPath]: book } };
    sessionMs += TICK_MS;
    updateSpeed();
    persist();
  };

  /** Личная скорость: сглаженное среднее после пары минут чтения. */
  const updateSpeed = () => {
    if (sessionMs < 120_000) return;
    const s = stats.s;
    const minutes = sessionMs / 60_000;
    const next = { ...s.speed };
    if (sessionChars > 1500) {
      const cpm = sessionChars / minutes;
      if (cpm > 150 && cpm < 6000) next.charsPerMin = Math.round(next.charsPerMin * 0.8 + cpm * 0.2);
    }
    if (sessionPages >= 2) {
      const spp = (sessionMs / 1000) / sessionPages;
      if (spp > 5 && spp < 900) next.secPerPage = Math.round(next.secPerPage * 0.8 + spp * 0.2);
    }
    next.samples += 1;
    stats.s = { ...s, speed: next };
    sessionMs = 0;
    sessionChars = 0;
    sessionPages = 0;
  };

  const timer = setInterval(tick, TICK_MS);
  const events = ["keydown", "pointerdown", "wheel", "touchstart"] as const;
  for (const e of events) window.addEventListener(e, markActive, { passive: true });

  return {
    /** Сообщить о новой позиции — это и есть признак чтения. */
    position(p: { charsLeftBook: number | null; page: number | null }) {
      markActive();
      if (!stats.loaded) return;
      const key = dayKey();
      const s = stats.s;
      const day = s.days[key] ?? { ms: 0, chars: 0, pages: 0, books: [] };
      let changed = false;
      if (p.charsLeftBook != null) {
        if (lastCharsLeft != null) {
          const delta = lastCharsLeft - p.charsLeftBook;
          // Большие скачки — это переход по оглавлению, а не чтение.
          if (delta > 0 && delta < 12_000) {
            day.chars += delta;
            sessionChars += delta;
            changed = true;
          }
        }
        lastCharsLeft = p.charsLeftBook;
      }
      if (p.page != null) {
        if (lastPage != null) {
          const delta = p.page - lastPage;
          if (delta > 0 && delta <= 2) {
            day.pages += delta;
            sessionPages += delta;
            changed = true;
          }
        }
        lastPage = p.page;
      }
      if (changed) {
        stats.s = { ...s, days: { ...s.days, [key]: day } };
        persist();
      }
    },
    activity: markActive,
    stop() {
      clearInterval(timer);
      for (const e of events) window.removeEventListener(e, markActive);
      flushStats();
    },
  };
}

/** Оценка оставшегося времени, минуты. */
export function minutesLeft(p: {
  charsLeft: number | null;
  pagesLeft: number | null | undefined;
}): number | null {
  const sp = stats.s.speed;
  if (p.charsLeft != null) return p.charsLeft / Math.max(200, sp.charsPerMin);
  if (p.pagesLeft != null) return (p.pagesLeft * sp.secPerPage) / 60;
  return null;
}

export function formatMinutes(min: number | null): string {
  if (min == null || !Number.isFinite(min)) return "";
  if (min < 1) return "меньше минуты";
  if (min < 60) return `${Math.round(min)} мин`;
  const h = Math.floor(min / 60);
  const m = Math.round(min % 60);
  return m ? `${h} ч ${m} мин` : `${h} ч`;
}

/** Серия дней подряд с чтением (включая сегодня или вчера). */
export function streak(days: Record<string, DayStats>): number {
  let n = 0;
  const d = new Date();
  if (!days[dayKey(d)] || days[dayKey(d)]!.ms < 60_000) d.setDate(d.getDate() - 1);
  for (;;) {
    const k = dayKey(d);
    if (!days[k] || days[k]!.ms < 60_000) break;
    n++;
    d.setDate(d.getDate() - 1);
  }
  return n;
}
