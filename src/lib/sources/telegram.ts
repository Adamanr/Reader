import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { commands } from "$lib/bindings";
import { isTauriRuntime } from "$lib/isTauri";
import type {
  ImportProgress,
  ImportResult,
  RemoteFilesPage,
  TelegramChannelInfo,
  TelegramSignInResult,
  TelegramStatus,
} from "./types";

export const OFFLINE_TELEGRAM_STATUS: TelegramStatus = {
  connected: false,
  phone: null,
  displayName: null,
  apiId: null,
  hasApiHash: false,
  secureStorage: null,
};

export async function telegramStatus(): Promise<TelegramStatus> {
  if (!isTauriRuntime()) return OFFLINE_TELEGRAM_STATUS;
  return commands.telegramStatus();
}

/** Пустой `apiHash` — использовать сохранённый в приложении. */
export async function telegramSendCode(opts: {
  apiId: number;
  apiHash: string;
  phone: string;
}): Promise<void> {
  await commands.telegramSendCode(opts.apiId, opts.apiHash, opts.phone);
}

export async function telegramSignIn(opts: {
  code: string;
  password?: string | null;
}): Promise<TelegramSignInResult> {
  return commands.telegramSignIn(opts.code, opts.password ?? null);
}

/** `true` — сеанс завершён и на сервере Telegram, `false` — только локально. */
export async function telegramLogout(): Promise<boolean> {
  return commands.telegramLogout();
}

export async function telegramResolveChannel(input: string): Promise<TelegramChannelInfo> {
  return commands.telegramResolveChannel(input);
}

export async function sourceListFiles(opts: {
  username: string;
  query?: string;
  offsetId?: number | null;
  limit?: number;
}): Promise<RemoteFilesPage> {
  return commands.sourceListFiles(
    opts.username,
    opts.query?.trim() || null,
    opts.offsetId ?? null,
    opts.limit ?? 50,
  );
}

export async function sourceImportFiles(opts: {
  username: string;
  messageIds: number[];
}): Promise<ImportResult> {
  return commands.sourceImportFiles(opts.username, opts.messageIds);
}

/** Сколько книг появилось в канале после момента `since` (до 100). */
export async function sourceCountNew(username: string, since: string): Promise<number> {
  const sinceMs = Date.parse(since);
  if (!Number.isFinite(sinceMs)) return 0;
  return commands.sourceCountNew(username, sinceMs);
}

export function onImportProgress(cb: (p: ImportProgress) => void): Promise<UnlistenFn> {
  return listen<ImportProgress>("telegram-import-progress", (e) => cb(e.payload));
}
