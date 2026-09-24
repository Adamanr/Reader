import { invoke } from "@tauri-apps/api/core";
import { llmChatCompletion } from "$lib/translate/llmClient";
import { isTauriRuntime } from "$lib/isTauri";

/**
 * Локальный помощник чтения (LM Studio, Ollama или любой OpenAI-совместимый API).
 * Всё работает на вашем компьютере: текст книги никуда не уходит.
 */
export interface AssistantSettings {
  baseUrl: string;
  model: string;
  /** Модель эмбеддингов для «созвездия»; пусто — простой словарный метод */
  embedModel: string;
}

const KEY = "reader.assistant.v1";

export const LM_STUDIO_URL = "http://127.0.0.1:1234/v1";
export const OLLAMA_URL = "http://127.0.0.1:11434/v1";

function load(): AssistantSettings {
  const d = { baseUrl: "", model: "", embedModel: "" };
  try {
    const raw = typeof localStorage !== "undefined" ? localStorage.getItem(KEY) : null;
    if (raw) return { ...d, ...(JSON.parse(raw) as Partial<AssistantSettings>) };
  } catch {
    /* ignore */
  }
  return d;
}

export const assistant = $state<{ s: AssistantSettings }>({ s: load() });

export function saveAssistant(patch: Partial<AssistantSettings>) {
  assistant.s = { ...assistant.s, ...patch };
  try {
    localStorage.setItem(KEY, JSON.stringify(assistant.s));
  } catch {
    /* ignore */
  }
}

export function assistantReady(): boolean {
  return isTauriRuntime() && !!assistant.s.baseUrl.trim() && !!assistant.s.model.trim();
}

export class AssistantNotConfigured extends Error {
  constructor() {
    super("Подключите локальную модель в Настройках → «Помощник чтения».");
  }
}

export async function ask(system: string, user: string, maxTokens = 900, temperature = 0.3): Promise<string> {
  if (!assistantReady()) throw new AssistantNotConfigured();
  const raw = await llmChatCompletion({
    baseUrl: assistant.s.baseUrl,
    model: assistant.s.model,
    messages: [
      { role: "system", content: system },
      { role: "user", content: user },
    ],
    temperature,
    maxTokens,
  });
  // Модели с «размышлениями» иногда возвращают их в ответе — убираем.
  return raw.replace(/<think>[\s\S]*?<\/think>/g, "").trim();
}

export async function embed(inputs: string[]): Promise<number[][]> {
  if (!isTauriRuntime() || !assistant.s.baseUrl.trim() || !assistant.s.embedModel.trim()) {
    throw new AssistantNotConfigured();
  }
  const out: number[][] = [];
  for (let i = 0; i < inputs.length; i += 64) {
    const part = await invoke<number[][]>("llm_embeddings", {
      baseUrl: assistant.s.baseUrl.trim(),
      model: assistant.s.embedModel.trim(),
      inputs: inputs.slice(i, i + 64),
    });
    out.push(...part);
  }
  return out;
}

/** Грубая оценка токенов для русского/английского текста. */
export function approxTokens(text: string): number {
  return Math.ceil(text.length / 3.2);
}
