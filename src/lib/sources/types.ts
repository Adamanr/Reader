// Ответы бэкенда описаны в Rust и генерируются в `$lib/bindings` (tauri-specta).
export type {
  ImportFailure,
  ImportProgress,
  ImportResult,
  OpdsAcquisition,
  OpdsEntry,
  OpdsFeed,
  OpdsImportResult,
  RemoteBookFile,
  RemoteFilesPage,
  TelegramChannelInfo,
  TelegramSignInResult,
  TelegramStatus,
} from "$lib/bindings";

export type SourceKind = "telegram" | "opds";

export interface BookSource {
  id: string;
  kind: SourceKind;
  label: string;
  enabled: boolean;
  /** Telegram: username без @ (у OPDS — пустая строка). */
  username: string;
  /** OPDS: адрес корня каталога. */
  url?: string;
  title?: string;
  addedAt: string;
  lastSyncedAt?: string;
  /** Когда каталог источника последний раз открывали — от этого момента считаются «новые» книги. */
  lastSeenAt?: string;
}

export interface BookSourcesState {
  sources: BookSource[];
}

export type RemoteBookExt = "pdf" | "epub" | "fb2" | "fb2.zip";

