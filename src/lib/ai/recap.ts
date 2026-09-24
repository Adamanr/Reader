import type { TextChunk } from "$lib/types";
import { ask } from "$lib/ai/assistant.svelte";
import { pathKey, storeRead, storeWrite } from "$lib/storage/store";

/**
 * «Ранее в книге…» — пересказ строго до текущего места, без спойлеров.
 * Каждая прочитанная глава сжимается один раз и кэшируется; итог собирается
 * из этих конспектов и свежего куска текущей главы.
 */

const SYSTEM =
  "Ты — внимательный помощник читателя. Опирайся ТОЛЬКО на присланный текст. " +
  "Никогда не упоминай и не угадывай события, которых нет в тексте, не раскрывай дальнейший сюжет. " +
  "Пиши по-русски, живо и ясно, без вступлений вроде «Конечно».";

const PIECE = 9000;

interface RecapCache {
  summaries: Record<string, { len: number; text: string }>;
}

function pieces(text: string, size = PIECE): string[] {
  const out: string[] = [];
  let i = 0;
  while (i < text.length) {
    let end = Math.min(text.length, i + size);
    if (end < text.length) {
      const dot = text.lastIndexOf(". ", end);
      if (dot > i + size * 0.6) end = dot + 1;
    }
    out.push(text.slice(i, end));
    i = end;
  }
  return out;
}

async function summarizeText(label: string, text: string, words: number, signal?: AbortSignal): Promise<string> {
  const parts = pieces(text);
  const partial: string[] = [];
  for (const p of parts) {
    if (signal?.aborted) throw new DOMException("cancelled", "AbortError");
    partial.push(
      await ask(
        SYSTEM,
        `Фрагмент книги (${label}). Перескажи его в ${parts.length > 1 ? Math.round(words / 1.5) : words} словах максимум: ` +
          `кто действует, что произошло, важные детали и перемены. Только факты из фрагмента.\n\n«${p}»`,
        600,
      ),
    );
  }
  if (partial.length === 1) return partial[0]!;
  if (signal?.aborted) throw new DOMException("cancelled", "AbortError");
  return ask(
    SYSTEM,
    `Сведи эти конспекты частей одной главы («${label}») в один связный пересказ до ${words} слов:\n\n${partial.join("\n\n")}`,
    600,
  );
}

export async function recapBook(opts: {
  path: string;
  title: string;
  chunks: TextChunk[];
  onStage?: (s: string) => void;
  signal?: AbortSignal;
}): Promise<string> {
  const { path, title, chunks, onStage, signal } = opts;
  const key = `recap-${await pathKey(path)}`;
  const cache = await storeRead<RecapCache>(key, { summaries: {} });
  const full = chunks.filter((c) => !c.partial);
  const current = chunks.find((c) => c.partial);

  const summaries: { label: string; text: string }[] = [];
  let done = 0;
  for (const c of full) {
    if (signal?.aborted) throw new DOMException("cancelled", "AbortError");
    const cached = cache.summaries[c.id];
    if (cached && Math.abs(cached.len - c.text.length) < 40) {
      summaries.push({ label: c.label, text: cached.text });
    } else {
      onStage?.(`Вспоминаю «${c.label}» (${done + 1} из ${full.length})…`);
      const text = await summarizeText(c.label, c.text, 110, signal);
      cache.summaries[c.id] = { len: c.text.length, text };
      summaries.push({ label: c.label, text });
      void storeWrite(key, cache).catch(() => {});
    }
    done++;
  }

  let currentSummary = "";
  if (current?.text) {
    onStage?.("Перечитываю, где вы остановились…");
    currentSummary = await summarizeText(current.label, current.text.slice(-PIECE * 2), 130, signal);
  }

  // Слишком длинную историю сжимаем: старые главы — одним абзацем.
  let history = summaries.map((s) => `— ${s.label}: ${s.text}`).join("\n");
  if (history.length > 14000) {
    onStage?.("Собираю начало книги воедино…");
    const early = summaries.slice(0, -8);
    const late = summaries.slice(-8);
    const earlyText = await ask(
      SYSTEM,
      `Сожми эти конспекты начала книги в один абзац до 180 слов:\n\n${early.map((s) => `${s.label}: ${s.text}`).join("\n")}`,
      500,
    );
    history = `— Начало книги: ${earlyText}\n` + late.map((s) => `— ${s.label}: ${s.text}`).join("\n");
  }

  if (signal?.aborted) throw new DOMException("cancelled", "AbortError");
  onStage?.("Пишу пересказ…");
  return ask(
    SYSTEM,
    `Книга: «${title}». Читатель вернулся к ней после перерыва. Напомни, что было до этого места.\n\n` +
      `Конспекты прочитанных глав:\n${history || "(это первая глава)"}\n\n` +
      `Текущая глава до места остановки:\n${currentSummary || "(нет)"}\n\n` +
      "Напиши 2–4 коротких абзаца: главные герои и их положение, ключевые события, " +
      "а последним предложением — на чём именно читатель остановился. Никаких догадок о будущем.",
    900,
  );
}
