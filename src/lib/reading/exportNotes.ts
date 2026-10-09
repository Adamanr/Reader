import { commands } from "$lib/bindings";
import type { BookMeta, Highlight } from "$lib/types";
import { isTauriRuntime } from "$lib/isTauri";
import { writeTextToClipboard } from "$lib/clipboardWrite";

function titleFromPath(path: string) {
  const i = path.lastIndexOf("/");
  return i >= 0 ? path.slice(i + 1) : path;
}

function where(h: Pick<Highlight, "page" | "chapterLabel">): string {
  const parts: string[] = [];
  if (h.chapterLabel) parts.push(h.chapterLabel);
  if (h.page != null) parts.push(`стр. ${h.page}`);
  return parts.join(", ");
}

function quoteBlock(text: string): string {
  return text
    .trim()
    .split(/\n+/)
    .map((l) => `> ${l}`)
    .join("\n");
}

/** Markdown-конспект книги: совместим с Obsidian/Logseq. */
export function bookNotesMarkdown(path: string, meta: BookMeta): string {
  const title = meta.title?.trim() || titleFromPath(path);
  const lines: string[] = [];
  lines.push("---");
  lines.push(`title: "${title.replace(/"/g, "'")}"`);
  if (meta.author?.trim()) lines.push(`author: "${meta.author.trim().replace(/"/g, "'")}"`);
  lines.push(`source: "${path}"`);
  if (meta.progress != null) lines.push(`progress: ${Math.round(meta.progress * 100)}%`);
  lines.push(`exported: ${new Date().toISOString().slice(0, 10)}`);
  lines.push("tags: [книги]");
  lines.push("---", "");
  lines.push(`# ${title}`);
  if (meta.author?.trim()) lines.push(`*${meta.author.trim()}*`);
  lines.push("");

  if (meta.review?.trim()) {
    lines.push("## Заметки", "", meta.review.trim(), "");
  }

  const hls = [...(meta.highlights ?? [])];
  if (hls.length) {
    lines.push("## Выделения", "");
    let lastChapter = "";
    for (const h of hls) {
      const ch = h.chapterLabel ?? "";
      if (ch && ch !== lastChapter) {
        lines.push(`### ${ch}`, "");
        lastChapter = ch;
      }
      lines.push(quoteBlock(h.text));
      const w = h.page != null ? ` — стр. ${h.page}` : "";
      if (w) lines.push(`>${w}`);
      if (h.note?.trim()) lines.push("", `**Мысль:** ${h.note.trim()}`);
      lines.push("");
    }
  }

  const comments = meta.comments ?? [];
  if (comments.length) {
    lines.push("## Комментарии", "");
    for (const c of comments) {
      if (c.excerpt) lines.push(quoteBlock(c.excerpt));
      const w = where(c);
      if (w) lines.push(`> — ${w}`);
      lines.push("", c.body.trim(), "");
    }
  }

  const quotes = meta.quotes ?? [];
  if (quotes.length) {
    lines.push("## Цитаты", "");
    for (const q of quotes) lines.push(quoteBlock(q.text), "");
  }

  return lines.join("\n").replace(/\n{3,}/g, "\n\n");
}

function utf8ToBase64(s: string): string {
  const bytes = new TextEncoder().encode(s);
  let bin = "";
  for (let i = 0; i < bytes.length; i += 0x8000) {
    bin += String.fromCharCode(...bytes.subarray(i, i + 0x8000));
  }
  return btoa(bin);
}

/** Сохранить Markdown через системный диалог; вне Tauri — в буфер обмена. */
export async function saveMarkdown(defaultName: string, md: string): Promise<"saved" | "copied" | "cancelled"> {
  if (!isTauriRuntime()) {
    await writeTextToClipboard(md);
    return "copied";
  }
  // Путь выбирается в нативном диалоге на стороне Rust — интерфейс его не передаёт.
  const saved = await commands.saveFileDialog(
    defaultName.replace(/[\\/:*?"<>|]+/g, " ").trim() + ".md",
    "Markdown",
    ["md"],
    utf8ToBase64(md),
  );
  return saved ? "saved" : "cancelled";
}
