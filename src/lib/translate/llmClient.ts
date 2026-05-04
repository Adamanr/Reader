import { invoke } from "@tauri-apps/api/core";

export interface LlmMessage {
  role: string;
  content: string;
}

export async function llmChatCompletion(opts: {
  baseUrl: string;
  model: string;
  messages: LlmMessage[];
  temperature: number;
  maxTokens?: number | null;
}): Promise<string> {
  return await invoke<string>("llm_chat_completion", {
    baseUrl: opts.baseUrl.trim(),
    model: opts.model.trim(),
    messages: opts.messages,
    temperature: opts.temperature,
    maxTokens: opts.maxTokens ?? null,
  });
}

export async function llmListModels(baseUrl: string): Promise<string[]> {
  return await invoke<string[]>("llm_list_models", {
    baseUrl: baseUrl.trim(),
  });
}
