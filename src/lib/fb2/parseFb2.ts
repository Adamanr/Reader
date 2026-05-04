/** Парсинг FictionBook 2 (fb2) → секции с безопасным HTML. */

const XLINK_NS = "http://www.w3.org/1999/xlink";

export interface Fb2Section {
  anchor: string;
  title: string;
  html: string;
}

export interface ParsedFb2 {
  bookTitle: string;
  author: string;
  /** HTML обложки из &lt;title-info&gt;&lt;coverpage&gt; (картинки из &lt;binary&gt;). */
  coverHtml: string;
  sections: Fb2Section[];
}

function escapeHtml(s: string): string {
  return s
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/"/g, "&quot;");
}

function textTrim(el: Element | null | undefined): string {
  if (!el) return "";
  return (el.textContent || "").replace(/\s+/g, " ").trim();
}

function collectBinaries(doc: Document): Map<string, string> {
  const map = new Map<string, string>();
  const nodes = doc.getElementsByTagName("binary");
  for (let i = 0; i < nodes.length; i++) {
    const b = nodes[i];
    const id = b.getAttribute("id");
    if (!id) continue;
    const ct = b.getAttribute("content-type") || "image/jpeg";
    const raw = (b.textContent || "").replace(/\s+/g, "");
    if (!raw) continue;
    map.set(id, `data:${ct};base64,${raw}`);
  }
  return map;
}

function extractTitleFromTitleEl(titleEl: Element): string {
  const ps = titleEl.getElementsByTagName("p");
  if (ps.length) {
    const lines: string[] = [];
    for (let i = 0; i < ps.length; i++) lines.push(textTrim(ps[i]));
    return lines.filter(Boolean).join(" ");
  }
  return textTrim(titleEl);
}

function imageHref(el: Element): string {
  return (
    el.getAttributeNS(XLINK_NS, "href") ||
    el.getAttribute("l:href") ||
    el.getAttribute("href") ||
    ""
  ).replace(/^#/, "");
}

function elementToHtml(el: Element, binaries: Map<string, string>): string {
  const tag = el.localName || el.tagName.toLowerCase();

  const inner = (): string => {
    let s = "";
    for (const n of el.childNodes) {
      if (n.nodeType === Node.TEXT_NODE) {
        s += escapeHtml(n.textContent || "");
      } else if (n.nodeType === Node.ELEMENT_NODE) {
        s += elementToHtml(n as Element, binaries);
      }
    }
    return s;
  };

  switch (tag) {
    case "p":
      return `<p class="fb2-p">${inner()}</p>`;
    case "empty-line":
      return `<div class="fb2-empty-line" aria-hidden="true"></div>`;
    case "emphasis":
      return `<em>${inner()}</em>`;
    case "strong":
      return `<strong>${inner()}</strong>`;
    case "strikethrough":
      return `<s>${inner()}</s>`;
    case "sub":
      return `<sub>${inner()}</sub>`;
    case "sup":
      return `<sup>${inner()}</sup>`;
    case "code":
      return `<code>${inner()}</code>`;
    case "subtitle":
      return `<h3 class="fb2-subtitle">${inner()}</h3>`;
    case "cite":
      return `<cite class="fb2-cite">${inner()}</cite>`;
    case "text-author":
      return `<p class="fb2-text-author">${inner()}</p>`;
    case "date":
      return `<p class="fb2-date">${inner()}</p>`;
    case "epigraph":
      return `<blockquote class="fb2-epigraph">${inner()}</blockquote>`;
    case "stanza":
      return `<div class="fb2-stanza">${inner()}</div>`;
    case "poem":
      return `<div class="fb2-poem">${inner()}</div>`;
    case "v":
      return `<div class="fb2-v">${inner()}</div>`;
    case "image": {
      const id = imageHref(el);
      const src = id ? binaries.get(id) : "";
      const alt = escapeHtml(el.getAttribute("alt") || "");
      if (!src) return "";
      return `<figure class="fb2-fig"><img class="fb2-img" src="${escapeHtml(src)}" alt="${alt}" /></figure>`;
    }
    case "table": {
      return `<table class="fb2-table">${walkTableChildren(el, binaries)}</table>`;
    }
    case "tr":
      return `<tr>${walkTableChildren(el, binaries)}</tr>`;
    case "th":
      return `<th>${inner()}</th>`;
    case "td":
      return `<td>${inner()}</td>`;
    case "a": {
      const t = el.getAttribute("type");
      const xlink = el.getAttributeNS(XLINK_NS, "href") || el.getAttribute("l:href") || "";
      if (t === "note" || xlink.startsWith("#")) {
        return `<span class="fb2-noteref">${inner()}</span>`;
      }
      return `<span class="fb2-link">${inner()}</span>`;
    }
    default:
      return inner();
  }
}

function walkTableChildren(el: Element, binaries: Map<string, string>): string {
  let s = "";
  for (const ch of el.children) {
    s += elementToHtml(ch, binaries);
  }
  return s;
}

function parseSectionTree(section: Element, binaries: Map<string, string>, idCounter: { i: number }): Fb2Section[] {
  const titleEl = [...section.children].find((c) => c.localName === "title");
  const titleText = titleEl ? extractTitleFromTitleEl(titleEl) : "";

  const directHtml: string[] = [];
  const nested: Element[] = [];
  for (const ch of section.children) {
    if (ch.localName === "title") continue;
    if (ch.localName === "section") nested.push(ch);
    else directHtml.push(elementToHtml(ch, binaries));
  }

  const combined = directHtml.join("");
  if (nested.length && !combined.trim() && !titleText) {
    const out: Fb2Section[] = [];
    for (const n of nested) out.push(...parseSectionTree(n, binaries, idCounter));
    return out;
  }

  const out: Fb2Section[] = [];
  if (combined.trim() || titleText) {
    out.push({
      anchor: `fb2-${idCounter.i++}`,
      title: titleText || "Раздел",
      html: combined,
    });
  }
  for (const n of nested) {
    out.push(...parseSectionTree(n, binaries, idCounter));
  }
  return out;
}

function parseBody(body: Element, binaries: Map<string, string>): Fb2Section[] {
  const idCounter = { i: 0 };
  const out: Fb2Section[] = [];
  for (const ch of body.children) {
    if (ch.localName === "section") {
      out.push(...parseSectionTree(ch, binaries, idCounter));
    } else {
      const html = elementToHtml(ch, binaries);
      if (html.trim()) {
        out.push({
          anchor: `fb2-${idCounter.i++}`,
          title: "",
          html,
        });
      }
    }
  }
  if (!out.length) {
    out.push({
      anchor: "fb2-0",
      title: "Текст",
      html: `<p class="fb2-p">${escapeHtml("Не удалось разобрать секции книги.")}</p>`,
    });
  }
  return out;
}

function coverpageHtml(titleInfo: Element | null, binaries: Map<string, string>): string {
  if (!titleInfo) return "";
  const cp = titleInfo.getElementsByTagName("coverpage")[0];
  if (!cp) return "";
  let html = "";
  for (const ch of cp.children) {
    html += elementToHtml(ch as Element, binaries);
  }
  return html.trim();
}

function metaFromDescription(root: Element): { bookTitle: string; author: string } {
  let bookTitle = "";
  let author = "";
  const desc = [...root.children].find((c) => c.localName === "description");
  if (!desc) return { bookTitle, author };

  const titleInfos = desc.getElementsByTagName("title-info");
  const ti = titleInfos[0];
  if (ti) {
    const bt = ti.getElementsByTagName("book-title")[0];
    bookTitle = textTrim(bt);

    const authors = ti.getElementsByTagName("author");
    const names: string[] = [];
    for (let i = 0; i < authors.length; i++) {
      const a = authors[i];
      const fn = textTrim(a.getElementsByTagName("first-name")[0]);
      const mn = textTrim(a.getElementsByTagName("middle-name")[0]);
      const ln = textTrim(a.getElementsByTagName("last-name")[0]);
      const nick = textTrim(a.getElementsByTagName("nickname")[0]);
      const line = [fn, mn, ln].filter(Boolean).join(" ") || nick;
      if (line) names.push(line);
    }
    author = names.join(", ");
  }

  return { bookTitle, author };
}

export function parseFb2(xmlString: string): ParsedFb2 {
  const doc = new DOMParser().parseFromString(xmlString, "application/xml");
  const pe = doc.querySelector("parsererror");
  if (pe) {
    throw new Error("Файл FB2 не является корректным XML.");
  }

  const root =
    doc.documentElement?.localName === "FictionBook"
      ? doc.documentElement
      : doc.getElementsByTagName("FictionBook")[0];
  if (!root) {
    throw new Error("Не найден корневой элемент FictionBook.");
  }

  const binaries = collectBinaries(doc);
  const { bookTitle, author } = metaFromDescription(root);

  const desc = [...root.children].find((c) => c.localName === "description");
  const ti = desc?.getElementsByTagName("title-info")[0] ?? null;
  const coverHtml = coverpageHtml(ti, binaries);

  const bodies = [...root.children].filter((c) => c.localName === "body");
  const main =
    bodies.find((b) => b.getAttribute("name") !== "notes") || bodies[0] || null;
  if (!main) {
    throw new Error("В файле нет элемента body.");
  }

  const sections = parseBody(main, binaries);
  return { bookTitle, author, coverHtml, sections };
}
