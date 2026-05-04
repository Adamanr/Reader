import { getDocument, GlobalWorkerOptions } from "pdfjs-dist";
import pdfWorker from "pdfjs-dist/build/pdf.worker.min.mjs?url";
import { invoke } from "@tauri-apps/api/core";
import { llmChatCompletion } from "./llmClient";
import type { PdfBookTranslationFile } from "./pdfFullBookTypes";
import { parsePdfBookTranslationFile } from "./pdfFullBookTypes";
import { extractTextItemStrings } from "./pdfTextItems";
import {
  assertBufferLooksLikePdf,
  readLibraryBookBytes,
} from "$lib/library/readLibraryBookBytes";
import { buildPdfChunkTranslationPromptPair } from "./pdfChunkPrompts";
import { extractTranslationTagged } from "./translationExtract";

GlobalWorkerOptions.workerSrc = pdfWorker;

const ITEM_CHUNK = 22;

function nowIso() {
  return new Date().toISOString();
}

/**
 * Из ответа модели: контент между &lt;TRANSLATION&gt;…&lt;/TRANSLATION&gt; (как TranslateBooksWithLLMs),
 * внутри — JSON-массив строк. Fallback: прежний разбор «голого» JSON при отсутствии тегов.
 */
export function parseTranslatedStringArray(reply: string): string[] {
  const inner = extractTranslationTagged(reply);
  const candidate = inner ?? reply.trim();
  const fence = candidate.match(/```(?:json)?\s*([\s\S]*?)```/);
  const body = (fence?.[1] ?? candidate).trim();
  let parsed: unknown;
  try {
    parsed = JSON.parse(body);
  } catch {
    throw new Error(
      inner == null
        ? "Ответ модели: нет блока <TRANSLATION>…</TRANSLATION> и не удалось разобрать JSON"
        : "Внутри <TRANSLATION> ожидался JSON-массив строк",
    );
  }
  if (!Array.isArray(parsed)) {
    throw new Error("Ответ модели не JSON-массив строк");
  }
  return parsed.map((x) => (typeof x === "string" ? x : String(x)));
}

async function translateChunk(
  chunk: string[],
  opts: {
    baseUrl: string;
    model: string;
    sourceLang: string;
    targetLang: string;
    temperature: number;
    maxTokens: number | null;
  },
): Promise<string[]> {
  const { system, user } = buildPdfChunkTranslationPromptPair({
    chunk,
    sourceLang: opts.sourceLang,
    targetLang: opts.targetLang,
  });

  const reply = await llmChatCompletion({
    baseUrl: opts.baseUrl,
    model: opts.model,
    messages: [
      { role: "system", content: system },
      { role: "user", content: user },
    ],
    temperature: opts.temperature,
    maxTokens: opts.maxTokens ?? undefined,
  });

  const out = parseTranslatedStringArray(reply);
  if (out.length !== chunk.length) {
    throw new Error(`Ожидалось ${chunk.length} строк, модель вернула ${out.length}`);
  }
  return out;
}

async function translateChunkWithFallback(
  chunk: string[],
  opts: Parameters<typeof translateChunk>[1],
  signal: AbortSignal | undefined,
): Promise<string[]> {
  signal?.throwIfAborted();
  if (chunk.length === 0) return [];
  try {
    return await translateChunk(chunk, opts);
  } catch (e) {
    signal?.throwIfAborted();
    if (chunk.length === 1) throw e;
    const mid = Math.ceil(chunk.length / 2);
    const a = chunk.slice(0, mid);
    const b = chunk.slice(mid);
    const [ta, tb] = await Promise.all([
      translateChunkWithFallback(a, opts, signal),
      translateChunkWithFallback(b, opts, signal),
    ]);
    return [...ta, ...tb];
  }
}

async function translatePageItems(
  items: string[],
  opts: Parameters<typeof translateChunk>[1],
  signal: AbortSignal | undefined,
): Promise<string[]> {
  const result: string[] = [];
  for (let i = 0; i < items.length; i += ITEM_CHUNK) {
    signal?.throwIfAborted();
    const chunk = items.slice(i, i + ITEM_CHUNK);
    const tr = await translateChunkWithFallback(chunk, opts, signal);
    result.push(...tr);
  }
  return result;
}

async function persistDoc(bookRelativePath: string, doc: PdfBookTranslationFile) {
  doc.updatedAt = nowIso();
  await invoke("pdf_translation_save", {
    bookRelativePath,
    json: JSON.stringify(doc),
  });
}

export type PdfBookTranslationProgress = {
  phase: "extract" | "translate";
  pageIndex: number;
  totalPages: number;
  message?: string;
  /** Короткие фрагменты с только что готовой страницы (превью в UI) */
  previewSamples?: string[];
};

export async function runPdfBookTranslation(opts: {
  bookRelativePath: string;
  baseUrl: string;
  model: string;
  sourceLang: string;
  targetLang: string;
  temperature: number;
  maxTokens: number | null;
  resumeFromPartial?: PdfBookTranslationFile | null;
  onProgress?: (p: PdfBookTranslationProgress) => void;
  signal?: AbortSignal;
}): Promise<PdfBookTranslationFile> {
  const { signal } = opts;

  const bytes = await readLibraryBookBytes(opts.bookRelativePath);
  assertBufferLooksLikePdf(bytes, "Полный перевод PDF");

  signal?.throwIfAborted();

  const pdf = await getDocument({ data: bytes }).promise;
  const totalPages = pdf.numPages;

  const base: PdfBookTranslationFile = {
    version: 1,
    status: "running",
    baseUrl: opts.baseUrl,
    model: opts.model,
    sourceLang: opts.sourceLang,
    targetLang: opts.targetLang,
    temperature: opts.temperature,
    maxTokens: opts.maxTokens,
    totalPages,
    pages: opts.resumeFromPartial?.pages ? { ...opts.resumeFromPartial.pages } : {},
    updatedAt: nowIso(),
  };

  const llmOpts = {
    baseUrl: opts.baseUrl,
    model: opts.model,
    sourceLang: opts.sourceLang,
    targetLang: opts.targetLang,
    temperature: opts.temperature,
    maxTokens: opts.maxTokens,
  };

  for (let p = 1; p <= totalPages; p++) {
    signal?.throwIfAborted();
    const key = String(p);
    const existing = base.pages[key];
    const page = await pdf.getPage(p);
    const tc = await page.getTextContent();
    const items = extractTextItemStrings(tc);

    if (existing && existing.length === items.length) {
      opts.onProgress?.({
        phase: "translate",
        pageIndex: p,
        totalPages,
        message: `Стр. ${p}: уже есть`,
      });
      continue;
    }

    opts.onProgress?.({
      phase: "translate",
      pageIndex: p,
      totalPages,
      message: `Стр. ${p}: извлечение…`,
    });

    if (items.length === 0) {
      base.pages[key] = [];
      base.lastProcessedPage = p;
      await persistDoc(opts.bookRelativePath, base);
      opts.onProgress?.({
        phase: "translate",
        pageIndex: p,
        totalPages,
        message: `Стр. ${p}: без текста`,
        previewSamples: [],
      });
      continue;
    }

    opts.onProgress?.({
      phase: "translate",
      pageIndex: p,
      totalPages,
      message: `Стр. ${p}: перевод…`,
    });

    const translated = await translatePageItems(items, llmOpts, signal);
    base.pages[key] = translated;
    base.lastProcessedPage = p;
    await persistDoc(opts.bookRelativePath, base);

    const samples = translated
      .map((t) => t.trim())
      .filter(Boolean)
      .slice(0, 5)
      .map((t) => (t.length > 240 ? `${t.slice(0, 240)}…` : t));
    opts.onProgress?.({
      phase: "translate",
      pageIndex: p,
      totalPages,
      message: `Стр. ${p}: сохранено`,
      previewSamples: samples,
    });
  }

  base.status = "complete";
  await persistDoc(opts.bookRelativePath, base);
  return base;
}

export async function loadPdfBookTranslationFile(
  bookRelativePath: string,
): Promise<PdfBookTranslationFile | null> {
  const raw = await invoke<string | null>("pdf_translation_load", { bookRelativePath });
  if (raw == null || raw === "") return null;
  return parsePdfBookTranslationFile(raw);
}
