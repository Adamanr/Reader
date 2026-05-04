import { invoke } from "@tauri-apps/api/core";
import type { LibrarySnapshot } from "$lib/types";
import { parseFb2 } from "$lib/fb2/parseFb2";
import { readLibraryBookBytes } from "$lib/library/readLibraryBookBytes";
import { BUILTIN_MINIMAL_THEME } from "./defaultTheme";
import { htmlFragmentToTypst } from "./htmlToTypst";
import { escapeTypstMarkup } from "./escapeTypst";
import { pdfBytesToTypstBody } from "./pdfToTypst";
import { epubBytesToTypstParts } from "./epubToTypst";
import {
  typstMainFileRelative,
  typstProjectDirFromSource,
  typstThemeFileRelative,
} from "./paths";
import type { BookFormat } from "$lib/bookFormat";

async function readThemeOrNull(relativePath: string): Promise<string | null> {
  try {
    return await invoke<string>("read_library_utf8", { relativePath });
  } catch {
    return null;
  }
}

/** Создаёт `.reader-typst-themes/minimal.typ`, если его ещё нет. */
export async function ensureBuiltinTypstThemes(): Promise<void> {
  const rel = ".reader-typst-themes/minimal.typ";
  const existing = await readThemeOrNull(rel);
  if (existing) return;
  await invoke("write_library_utf8", {
    relativePath: rel,
    content: BUILTIN_MINIMAL_THEME,
  });
}

export async function resolveThemeForExport(
  snapshot: LibrarySnapshot,
  bookStyleRelativePath?: string | null,
): Promise<string> {
  await ensureBuiltinTypstThemes();
  const tryBook = bookStyleRelativePath?.trim();
  if (tryBook) {
    const t = await readThemeOrNull(tryBook);
    if (t) return t;
  }
  const def = snapshot.defaultTypstStyleRelativePath?.trim();
  if (def) {
    const t = await readThemeOrNull(def);
    if (t) return t;
  }
  const minimal = await readThemeOrNull(".reader-typst-themes/minimal.typ");
  return minimal ?? BUILTIN_MINIMAL_THEME;
}

function buildBookTyp(
  title: string,
  author: string,
  body: string,
): string {
  const t = escapeTypstMarkup(title.trim() || "Без названия");
  const a = author.trim();
  const authorBlock = a
    ? `#align(center)[_${escapeTypstMarkup(a)}_]\n#v(0.8em)\n`
    : "";

  return `#import "theme.typ": apply-book-theme
#show: apply-book-theme

#set document(title: [${t}])

= ${t}

${authorBlock}

${body.trim()}

`;
}

/**
 * Конвертирует книгу в проект Typst рядом с файлом (`*.reader.typ/book.typ`).
 */
export async function exportBookToTypst(
  sourceRelativePath: string,
  format: BookFormat,
  snapshot: LibrarySnapshot,
  bookStyleRelativePath?: string | null,
): Promise<{ mainRelativePath: string }> {
  if (format === "typst") {
    throw new Error("Исходник уже в формате Typst.");
  }

  await ensureBuiltinTypstThemes();
  const bytes = await readLibraryBookBytes(sourceRelativePath);
  const projectDir = typstProjectDirFromSource(sourceRelativePath);
  const mainRel = typstMainFileRelative(sourceRelativePath);
  const themeRel = typstThemeFileRelative(sourceRelativePath);
  const themeContent = await resolveThemeForExport(snapshot, bookStyleRelativePath);

  let title = "";
  let author = "";
  let body = "";
  const assetIdx = { i: 0 };

  if (format === "fb2") {
    const xml = new TextDecoder("utf-8").decode(bytes);
    const parsed = parseFb2(xml);
    title = parsed.bookTitle || "";
    author = parsed.author || "";
    const chunks: string[] = [];

    if (parsed.coverHtml.trim()) {
      const cov = htmlFragmentToTypst(parsed.coverHtml, "assets/", assetIdx);
      for (const a of cov.assets) {
        await invoke("write_library_base64", {
          relativePath: `${projectDir}/${a.relativePath}`,
          contentsBase64: a.base64,
        });
      }
      if (cov.body.trim()) chunks.push(cov.body);
    }

    for (const sec of parsed.sections) {
      if (sec.title.trim()) {
        chunks.push(`== ${escapeTypstMarkup(sec.title)}\n\n`);
      }
      const conv = htmlFragmentToTypst(sec.html, "assets/", assetIdx);
      for (const a of conv.assets) {
        await invoke("write_library_base64", {
          relativePath: `${projectDir}/${a.relativePath}`,
          contentsBase64: a.base64,
        });
      }
      chunks.push(conv.body);
    }
    body = chunks.join("\n\n");
  } else if (format === "pdf") {
    title = "";
    author = "";
    body = await pdfBytesToTypstBody(bytes);
  } else if (format === "epub") {
    const conv = await epubBytesToTypstParts(bytes, "assets/", assetIdx);
    for (const a of conv.assets) {
      await invoke("write_library_base64", {
        relativePath: `${projectDir}/${a.relativePath}`,
        contentsBase64: a.base64,
      });
    }
    body = conv.body;
  }

  const bookTyp = buildBookTyp(title, author, body);

  await invoke("write_library_utf8", { relativePath: themeRel, content: themeContent });
  await invoke("write_library_utf8", { relativePath: mainRel, content: bookTyp });

  return { mainRelativePath: mainRel };
}
