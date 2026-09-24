import { invoke } from "@tauri-apps/api/core";
import { isTauriRuntime } from "$lib/isTauri";

/**
 * Небольшое JSON-хранилище рядом с метаданными библиотеки (`store/<name>.json`).
 * Если включена синхронизация, файлы лежат в `<библиотека>/.reader/store` и
 * переезжают между устройствами вместе с книгами. Вне Tauri — localStorage.
 */

const LS_PREFIX = "reader.store.";

export async function storeRead<T>(name: string, fallback: T): Promise<T> {
  try {
    const raw = isTauriRuntime()
      ? await invoke<string | null>("store_read", { name })
      : localStorage.getItem(LS_PREFIX + name);
    if (!raw) return fallback;
    return JSON.parse(raw) as T;
  } catch {
    return fallback;
  }
}

export async function storeWrite(name: string, value: unknown): Promise<void> {
  const json = JSON.stringify(value);
  if (isTauriRuntime()) {
    await invoke("store_write", { name, json });
  } else {
    localStorage.setItem(LS_PREFIX + name, json);
  }
}

/** Короткий стабильный ключ для имени записи из пути книги. */
export async function pathKey(path: string): Promise<string> {
  const bytes = new TextEncoder().encode(path);
  const digest = await crypto.subtle.digest("SHA-256", bytes);
  return Array.from(new Uint8Array(digest).slice(0, 12))
    .map((b) => b.toString(16).padStart(2, "0"))
    .join("");
}

/** Откладывает запись: быстрые серии изменений сливаются в одну. */
export function debouncedStoreWriter(name: string, delay = 800) {
  let timer: ReturnType<typeof setTimeout> | null = null;
  let pending: unknown = undefined;
  const flush = () => {
    if (timer) clearTimeout(timer);
    timer = null;
    if (pending === undefined) return;
    const value = pending;
    pending = undefined;
    void storeWrite(name, value).catch(() => {});
  };
  return {
    write(value: unknown) {
      pending = value;
      if (timer) clearTimeout(timer);
      timer = setTimeout(flush, delay);
    },
    flush,
  };
}
