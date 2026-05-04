import { invoke } from "@tauri-apps/api/core";
import { isTauriRuntime } from "$lib/isTauri";
import { cacheGet, cacheSet } from "./cache";
import { getTranslateSettings } from "./settings";

const MAX_CHUNK = 4200;
const PARALLEL = 6;

async function translateOneBlob(
  text: string,
  source: string,
  target: string,
): Promise<string> {
  const t = text.trim();
  if (!t) return text;
  if (t.length <= MAX_CHUNK) {
    const [out] = await translateStringListRaw([t], source, target);
    return out ?? text;
  }
  const chunks = t.split(/\n{2,}/).filter(Boolean);
  if (chunks.length <= 1) {
    const a = await translateStringListRaw([t.slice(0, MAX_CHUNK)], source, target);
    const b = await translateStringListRaw([t.slice(MAX_CHUNK)], source, target);
    return (a[0] ?? "") + (b[0] ?? "");
  }
  const tr = await translateStringListRaw(chunks, source, target);
  return tr.join("\n\n");
}

async function translateStringListRaw(
  texts: string[],
  source: string,
  target: string,
): Promise<string[]> {
  if (!isTauriRuntime()) {
    throw new Error(
      "Перевод выполняется в десктоп-приложении (Tauri). Запустите через «npm run tauri dev» или собранный Reader.",
    );
  }
  const s = getTranslateSettings();
  return await invoke<string[]>("translate_texts", {
    texts,
    source,
    target,
    apiBase: s.apiBase,
    apiKey: s.apiKey.trim() || null,
  });
}

export async function translateStringList(
  texts: string[],
  source: string,
  target: string,
): Promise<string[]> {
  const src = source === "auto" ? "auto" : source;
  const out: string[] = new Array(texts.length);
  const pendingIdx: number[] = [];

  for (let i = 0; i < texts.length; i++) {
    const raw = texts[i] ?? "";
    if (!raw.trim()) {
      out[i] = raw;
      continue;
    }
    const hit = cacheGet(raw, target, src);
    if (hit != null) out[i] = hit;
    else pendingIdx.push(i);
  }

  if (pendingIdx.length === 0) return out;

  for (let b = 0; b < pendingIdx.length; b += PARALLEL) {
    const chunk = pendingIdx.slice(b, b + PARALLEL);
    const results = await Promise.all(
      chunk.map(async (i) => {
        const tr = await translateOneBlob(texts[i] ?? "", src, target);
        return { i, tr };
      }),
    );
    for (const { i, tr } of results) {
      out[i] = tr;
      const orig = texts[i];
      if (orig?.trim()) cacheSet(orig, target, src, tr);
    }
  }

  return out;
}
