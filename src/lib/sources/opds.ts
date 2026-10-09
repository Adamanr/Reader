import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { commands, type OpdsProgress } from "$lib/bindings";
import type { OpdsFeed, OpdsImportResult } from "./types";

/** Каталоги, которые можно добавить одной кнопкой (свободные книги). */
export const OPDS_PRESETS: { label: string; url: string; hint: string }[] = [
  {
    label: "Project Gutenberg",
    url: "https://www.gutenberg.org/ebooks.opds/",
    hint: "70 000+ книг в общественном достоянии, в основном на английском",
  },
  {
    label: "Calibre на этом компьютере",
    url: "http://localhost:8080/opds",
    hint: "Ваша библиотека Calibre: «Подключить/поделиться» → «Запустить сервер»",
  },
];

/** Лента каталога. Ссылки на файлы бэкенд запоминает — скачать можно только их. */
export async function opdsFetch(url: string): Promise<OpdsFeed> {
  return commands.opdsFetch(url);
}

export function opdsSearchUrl(template: string, query: string): string {
  return template
    .replace("{searchTerms}", encodeURIComponent(query.trim()))
    .replace(/\{[^}]+\?\}/g, "");
}

export async function opdsImport(urls: string[]): Promise<OpdsImportResult> {
  return commands.opdsImport(urls.map((url) => ({ url })));
}

export function onOpdsProgress(cb: (p: OpdsProgress) => void): Promise<UnlistenFn> {
  return listen<OpdsProgress>("opds-import-progress", (e) => cb(e.payload));
}
