import { browser } from "$app/environment";

const PREFIX = "reader-tr:";
const MAX_KEYS = 600;

function key(text: string, target: string, source: string): string {
  return `${PREFIX}${source}|${target}|${hashLight(text)}`;
}

/** Лёгкий хэш для ключа кэша (не крипто). */
function hashLight(s: string): string {
  let h = 5381;
  for (let i = 0; i < s.length; i++) {
    h = (h * 33) ^ s.charCodeAt(i);
  }
  return (h >>> 0).toString(36) + ":" + s.length;
}

export function cacheGet(text: string, target: string, source: string): string | null {
  if (!browser || !text.trim()) return null;
  try {
    return localStorage.getItem(key(text, target, source));
  } catch {
    return null;
  }
}

export function cacheSet(text: string, target: string, source: string, translated: string): void {
  if (!browser || !text.trim()) return;
  try {
    const k = key(text, target, source);
    localStorage.setItem(k, translated);
    pruneIfNeeded();
  } catch {
    /* quota */
  }
}

function pruneIfNeeded(): void {
  if (!browser) return;
  try {
    const keys: string[] = [];
    for (let i = 0; i < localStorage.length; i++) {
      const k = localStorage.key(i);
      if (k?.startsWith(PREFIX)) keys.push(k);
    }
    if (keys.length <= MAX_KEYS) return;
    keys.sort();
    const drop = keys.length - MAX_KEYS;
    for (let i = 0; i < drop; i++) localStorage.removeItem(keys[i]!);
  } catch {
    /* ignore */
  }
}
