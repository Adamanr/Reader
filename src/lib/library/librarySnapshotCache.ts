import { invoke } from "@tauri-apps/api/core";
import { relocateStatsBook } from "$lib/reading/stats.svelte";
import { toast } from "$lib/ui/toast.svelte";
import type { LibraryMetadata, LibrarySnapshot } from "$lib/types";
import { relocateVocabBook } from "$lib/vocab/vocab.svelte";

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

/** Всегда новое сканирование (без объединения с уже идущим запросом). */
export async function fetchFreshLibrarySnapshot(): Promise<LibrarySnapshot> {
  const snap = await invoke<LibrarySnapshot>("get_library_snapshot");
  // Бэкенд уже перенёс данные переименованных книг на диске; здесь — копии в памяти.
  for (const { from, to } of snap.relocated ?? []) {
    relocateStatsBook(from, to);
    relocateVocabBook(from, to);
  }
  if (snap.mergedConflicts) {
    toast("Изменения с другого устройства объединены с этими", "info");
  }
  return setCachedLibrarySnapshot(snap);
}

/** Дедуплицирует одновременные запросы со страницы библиотеки и читалки. */
export async function fetchLibrarySnapshot(): Promise<LibrarySnapshot> {
  if (inFlight) return inFlight;

  inFlight = fetchFreshLibrarySnapshot()
    .finally(() => {
      inFlight = null;
    });

  return inFlight;
}
