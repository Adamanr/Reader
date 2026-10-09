import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { commands, type IndexProgress, type SearchHit, type SearchStatus } from "$lib/bindings";

export type { IndexProgress, SearchHit, SearchStatus };

export function searchQuery(query: string, limit = 80): Promise<SearchHit[]> {
  return commands.searchQuery(query, limit);
}

export function searchStatus(): Promise<SearchStatus> {
  return commands.searchStatus();
}

/** Инкрементальное обновление индекса в фоне. `false` — уже идёт. */
export function searchReindex(): Promise<boolean> {
  return commands.searchReindex();
}

export function onIndexProgress(cb: (p: IndexProgress) => void): Promise<UnlistenFn> {
  return listen<IndexProgress>("search-index-progress", (e) => cb(e.payload));
}
