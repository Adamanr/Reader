import { storeRead, storeWrite } from "$lib/storage/store";
import type { BookSource, BookSourcesState } from "./types";

const STORE_NAME = "book-sources";

const EMPTY: BookSourcesState = { sources: [] };

export async function loadBookSources(): Promise<BookSourcesState> {
  const raw = await storeRead<BookSourcesState>(STORE_NAME, EMPTY);
  if (!raw || !Array.isArray(raw.sources)) return { ...EMPTY };
  return {
    sources: raw.sources.filter(
      (s): s is BookSource =>
        !!s &&
        typeof s.id === "string" &&
        typeof s.label === "string" &&
        ((s.kind === "telegram" && typeof s.username === "string" && !!s.username) ||
          (s.kind === "opds" && typeof s.url === "string" && !!s.url)),
    ),
  };
}

export async function saveBookSources(state: BookSourcesState): Promise<void> {
  await storeWrite(STORE_NAME, state);
}

export async function addBookSource(source: BookSource): Promise<BookSourcesState> {
  const state = await loadBookSources();
  const same = (s: BookSource) =>
    s.kind === source.kind &&
    (source.kind === "telegram"
      ? s.username.toLowerCase() === source.username.toLowerCase()
      : s.url === source.url);
  if (state.sources.some(same)) {
    throw new Error(source.kind === "telegram" ? "Этот канал уже добавлен" : "Этот каталог уже добавлен");
  }
  state.sources = [...state.sources, source];
  await saveBookSources(state);
  return state;
}

export async function removeBookSource(id: string): Promise<BookSourcesState> {
  const state = await loadBookSources();
  state.sources = state.sources.filter((s) => s.id !== id);
  await saveBookSources(state);
  return state;
}

export async function touchBookSourceSynced(id: string): Promise<void> {
  const state = await loadBookSources();
  const i = state.sources.findIndex((s) => s.id === id);
  if (i < 0) return;
  state.sources[i] = { ...state.sources[i], lastSyncedAt: new Date().toISOString() };
  await saveBookSources(state);
}

/** Отмечает, что каталог источника открыт сейчас: новые книги дальше считаются от этого момента. */
export async function touchBookSourceSeen(id: string): Promise<void> {
  const state = await loadBookSources();
  const i = state.sources.findIndex((s) => s.id === id);
  if (i < 0) return;
  state.sources[i] = { ...state.sources[i], lastSeenAt: new Date().toISOString() };
  await saveBookSources(state);
}

export function newSourceId(): string {
  if (typeof crypto !== "undefined" && "randomUUID" in crypto) {
    return crypto.randomUUID();
  }
  return `src_${Date.now().toString(36)}_${Math.random().toString(36).slice(2, 10)}`;
}
