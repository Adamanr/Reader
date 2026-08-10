import { invoke } from "@tauri-apps/api/core";
import type { LibraryMetadata, LibrarySnapshot } from "$lib/types";

let cachedSnapshot: LibrarySnapshot | null = null;
let inFlight: Promise<LibrarySnapshot> | null = null;

/**
 * Общий для библиотеки и читалки кэш. Он живёт между переходами SvelteKit,
 * поэтому возврат к сетке книг не начинается с пустого экрана.
 */
export function getCachedLibrarySnapshot(): LibrarySnapshot | null {
  return cachedSnapshot;
}

export function setCachedLibrarySnapshot(snapshot: LibrarySnapshot): LibrarySnapshot {
  cachedSnapshot = snapshot;
  return snapshot;
}

export function setCachedLibraryMetadata(metadata: LibraryMetadata): void {
  if (cachedSnapshot) cachedSnapshot = { ...cachedSnapshot, metadata };
}

/** Дедуплицирует одновременные запросы со страницы библиотеки и читалки. */
export async function fetchLibrarySnapshot(): Promise<LibrarySnapshot> {
  if (inFlight) return inFlight;

  inFlight = invoke<LibrarySnapshot>("get_library_snapshot")
    .then(setCachedLibrarySnapshot)
    .finally(() => {
      inFlight = null;
    });

  return inFlight;
}
