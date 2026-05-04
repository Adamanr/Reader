import { escapeTypstMarkup } from "./escapeTypst";

export interface HtmlToTypstAsset {
  relativePath: string;
  base64: string;
}

/**
 * FB2/EHTML → Typst: текст, акценты, абзацы, картинки (data URL → файлы).
 */
export function htmlFragmentToTypst(
  html: string,
  assetPrefix: string,
  startAssetIndex: { i: number },
): { body: string; assets: HtmlToTypstAsset[] } {
  const doc = new DOMParser().parseFromString(
    `<!DOCTYPE html><meta charset="utf-8"><body>${html}</body>`,
    "text/html",
  );
  const root = doc.body;
  const assets: HtmlToTypstAsset[] = [];

  function pullDataUrl(src: string): HtmlToTypstAsset | null {
    const m = src.match(/^data:([^;]+);base64,(.+)$/i);
    if (!m) return null;
    const mime = m[1]!.toLowerCase();
    const ext =
      mime.includes("png") ? "png" :
      mime.includes("webp") ? "webp" :
      mime.includes("gif") ? "gif" :
      mime.includes("jpeg") || mime.includes("jpg") ? "jpg" :
      "png";
    const rel = `${assetPrefix}img-${startAssetIndex.i++}.${ext}`;
    assets.push({ relativePath: rel, base64: m[2]!.replace(/\s+/g, "") });
    return assets[assets.length - 1]!;
  }

  function walk(node: Node): string {
    if (node.nodeType === Node.TEXT_NODE) {
      const t = node.textContent ?? "";
      if (!t.trim()) return t.includes("\n") ? "\n" : "";
      return escapeTypstMarkup(t);
    }
    if (node.nodeType !== Node.ELEMENT_NODE) return "";
    const el = node as Element;
    const tag = el.localName?.toLowerCase() ?? "";

    const inner = (): string => {
      let s = "";
      for (const ch of el.childNodes) s += walk(ch);
      return s;
    };

    switch (tag) {
      case "br":
        return "\\\n";
      case "p":
        return `\n\n${inner().trim()}\n\n`;
      case "div":
        return `\n\n${inner().trim()}\n\n`;
      case "strong":
      case "b":
        return `*${inner()}*`;
      case "em":
      case "i":
        return `_${inner()}_`;
      case "s":
      case "strike":
        return `#strike[${inner()}]`;
      case "sub":
        return `#sub[${inner()}]`;
      case "sup":
        return `#super[${inner()}]`;
      case "code":
        return `\`${escapeTypstMarkup(inner())}\``;
      case "h1":
        return `\n= ${inner().trim()}\n\n`;
      case "h2":
        return `\n== ${inner().trim()}\n\n`;
      case "h3":
        return `\n=== ${inner().trim()}\n\n`;
      case "blockquote":
        return `\n#block(inset: (left: 1.2em))[\n${inner().trim()}\n]\n\n`;
      case "figure":
        return `\n${inner().trim()}\n\n`;
      case "img": {
        const src = el.getAttribute("src") ?? "";
        if (!src.startsWith("data:")) return "";
        const asset = pullDataUrl(src);
        if (!asset) return "";
        return `#figure(image("${asset.relativePath}"))\n`;
      }
      case "table":
      case "tr":
      case "td":
      case "th":
        return inner();
      default:
        return inner();
    }
  }

  let body = "";
  for (const ch of root.childNodes) body += walk(ch);
  body = body.replace(/\n{4,}/g, "\n\n\n").trim();
  return { body, assets };
}
