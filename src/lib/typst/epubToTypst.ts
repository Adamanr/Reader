import JSZip from "jszip";
import type { HtmlToTypstAsset } from "./htmlToTypst";
import { escapeTypstMarkup } from "./escapeTypst";

function dirname(p: string): string {
  const i = p.replace(/\\/g, "/").lastIndexOf("/");
  return i >= 0 ? p.slice(0, i) : "";
}

function joinUrl(base: string, href: string): string {
  const h = href.trim();
  if (!h || h.startsWith("http://") || h.startsWith("https://") || h.startsWith("data:")) return "";
  const b = base.replace(/\\/g, "/").replace(/\/+$/, "");
  const parts = h.split("/").filter(Boolean);
  let stack = b ? b.split("/").filter(Boolean) : [];
  for (const seg of parts) {
    if (seg === "..") stack.pop();
    else if (seg !== ".") stack.push(seg);
  }
  return stack.join("/");
}

/** Простой разбор EPUB → текстовые блоки и картинки из ZIP. */
export async function epubBytesToTypstParts(
  bytes: Uint8Array,
  assetPrefix: string,
  idx: { i: number },
): Promise<{ body: string; assets: HtmlToTypstAsset[] }> {
  const zip = await JSZip.loadAsync(bytes);
  const containerFile = zip.file("META-INF/container.xml");
  if (!containerFile) throw new Error("В EPUB нет META-INF/container.xml.");
  const containerXml = await containerFile.async("string");
  const cdoc = new DOMParser().parseFromString(containerXml, "application/xml");
  const rootfile = cdoc.getElementsByTagName("rootfile")[0];
  const fullPath = rootfile?.getAttribute("full-path")?.trim();
  if (!fullPath) throw new Error("Не найден full-path к OPF.");

  const opfXml = await zip.file(fullPath)?.async("string");
  if (!opfXml) throw new Error("Не удалось прочитать OPF.");
  const opfDir = dirname(fullPath);

  const opfDoc = new DOMParser().parseFromString(opfXml, "application/xml");
  const manifestItems = new Map<string, string>();
  const itemEls = opfDoc.querySelectorAll(
    "package > manifest > item, manifest item, item",
  );
  for (const item of [...itemEls]) {
    const id = item.getAttribute("id");
    const href = item.getAttribute("href");
    const mt = (item.getAttribute("media-type") || "").toLowerCase();
    if (!id || !href) continue;
    if (
      mt.includes("html") ||
      mt.includes("xml") ||
      /\.(xhtml|html|htm)$/i.test(href)
    ) {
      manifestItems.set(id, href);
    }
  }

  const spineIds: string[] = [];
  for (const ref of [...opfDoc.querySelectorAll("spine itemref, itemref")]) {
    const idref = ref.getAttribute("idref");
    if (idref) spineIds.push(idref);
  }

  const assets: HtmlToTypstAsset[] = [];
  const bodyParts: string[] = [];

  async function imageBytesToAsset(buf: Uint8Array): Promise<string> {
    const head = buf.slice(0, 8);
    let ext = "png";
    if (head[0] === 0xff && head[1] === 0xd8) ext = "jpg";
    else if (head[0] === 0x89 && head[1] === 0x50) ext = "png";
    else if (head[0] === 0x47 && head[1] === 0x49) ext = "gif";
    else if (head[0] === 0x52 && head[1] === 0x49) ext = "webp";

    let bin = "";
    for (let i = 0; i < buf.length; i++) bin += String.fromCharCode(buf[i]!);
    const b64 = btoa(bin);
    const rel = `${assetPrefix}img-${idx.i++}.${ext}`;
    assets.push({ relativePath: rel, base64: b64 });
    return rel;
  }

  for (const sid of spineIds) {
    const href = manifestItems.get(sid);
    if (!href) continue;
    const zipPath = joinUrl(opfDir, href);
    const f = zip.file(zipPath);
    if (!f) continue;
    const xhtml = await f.async("string");
    const xdoc = new DOMParser().parseFromString(xhtml, "application/xml");
    const bodyEl =
      xdoc.getElementsByTagName("body")[0] ||
      xdoc.getElementsByTagNameNS("http://www.w3.org/1999/xhtml", "body")[0];
    if (!bodyEl) continue;

    const baseForHref = dirname(zipPath);

    async function walk(el: Element): Promise<string> {
      const tag = el.localName?.toLowerCase() ?? "";
      const inner = async (): Promise<string> => {
        let s = "";
        for (const ch of el.childNodes) {
          if (ch.nodeType === Node.TEXT_NODE) {
            s += escapeTypstMarkup(ch.textContent ?? "");
          } else if (ch.nodeType === Node.ELEMENT_NODE) {
            s += await walk(ch as Element);
          }
        }
        return s;
      };

      if (tag === "img") {
        const src = el.getAttribute("src") || "";
        const resolved = joinUrl(baseForHref, src);
        if (!resolved) return "";
        const imgFile = zip.file(resolved);
        if (!imgFile) return "";
        const buf = await imgFile.async("uint8array");
        const rel = await imageBytesToAsset(buf);
        return `#figure(image("${rel}"))\n`;
      }
      if (tag === "br") return "\\\n";
      if (tag === "p") return `\n\n${(await inner()).trim()}\n\n`;
      if (tag === "h1") return `\n= ${(await inner()).trim()}\n\n`;
      if (tag === "h2") return `\n== ${(await inner()).trim()}\n\n`;
      if (tag === "h3") return `\n=== ${(await inner()).trim()}\n\n`;
      if (tag === "strong" || tag === "b") return `*${await inner()}*`;
      if (tag === "em" || tag === "i") return `_${await inner()}_`;
      return inner();
    }

    let chunk = "";
    for (const ch of bodyEl.childNodes) {
      if (ch.nodeType === Node.ELEMENT_NODE) chunk += await walk(ch as Element);
      else if (ch.nodeType === Node.TEXT_NODE) {
        const t = (ch.textContent ?? "").trim();
        if (t) chunk += escapeTypstMarkup(t);
      }
    }
    if (chunk.trim()) bodyParts.push(chunk.trim());
  }

  const body = bodyParts.join("\n\n#pagebreak()\n\n").replace(/\n{4,}/g, "\n\n\n");
  return { body, assets };
}
