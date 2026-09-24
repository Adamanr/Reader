import { invoke } from "@tauri-apps/api/core";
import { isTauriRuntime } from "$lib/isTauri";
import type { BookFormat } from "$lib/bookFormat";
import { enqueueCoverGeneration } from "$lib/library/coverLoadQueue";

/**
 * Обложки хранятся файлами (`covers/<hash>.txt`), а не внутри JSON библиотеки.
 * Здесь — кэш в памяти, чтобы сетка и читалка не читали их с диска повторно.
 */

const mem = new Map<string, string | null>();
const inflight = new Map<string, Promise<string | null>>();

export interface DiscoveredMeta {
  title: string | null;
  author: string | null;
}

export function peekCover(path: string): string | null | undefined {
  return mem.get(path);
}

async function readStored(path: string): Promise<string | null> {
  if (!isTauriRuntime()) return null;
  try {
    return (await invoke<string | null>("cover_get", { bookRelativePath: path })) ?? null;
  } catch {
    return null;
  }
}

/**
 * Обложка книги. Если её нет — генерирует из файла (в общей очереди).
 * `onMeta` получает название и автора из файла, когда книга читалась целиком.
 */
export function loadCover(
  path: string,
  format: BookFormat | null,
  opts: { onMeta?: (m: DiscoveredMeta) => void; needMeta?: boolean } = {},
): Promise<string | null> {
  const known = mem.get(path);
  if (known !== undefined && !opts.needMeta) return Promise.resolve(known);
  const running = inflight.get(path);
  if (running && !opts.needMeta) return running;

  const task = (async () => {
    const stored = known ?? (await readStored(path));
    if (stored && !opts.needMeta) {
      mem.set(path, stored);
      return stored;
    }
    if (!format || format === "typst" || !isTauriRuntime()) {
      mem.set(path, stored ?? null);
      return stored ?? null;
    }
    return enqueueCoverGeneration(async () => {
      const { readBookFileInfo } = await import("$lib/covers/bookCover");
      const info = await readBookFileInfo(path, format, !stored);
      opts.onMeta?.({ title: info.title, author: info.author });
      const cover = stored ?? info.cover;
      mem.set(path, cover);
      if (!stored && info.cover) {
        void invoke("cover_set", { bookRelativePath: path, dataUrl: info.cover }).catch(() => {});
      }
      return cover;
    });
  })().finally(() => inflight.delete(path));

  inflight.set(path, task);
  return task;
}
