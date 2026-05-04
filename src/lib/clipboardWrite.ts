import { isTauriRuntime } from "$lib/isTauri";

/** Текст в буфер: в Tauri надёжнее через плагин, чем через Web Clipboard API. */
export async function writeTextToClipboard(text: string): Promise<void> {
  if (isTauriRuntime()) {
    const { writeText } = await import("@tauri-apps/plugin-clipboard-manager");
    await writeText(text);
    return;
  }
  await navigator.clipboard.writeText(text);
}
