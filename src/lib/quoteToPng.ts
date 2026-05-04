import type { QuoteAccent, QuoteBgFit, QuoteLayout } from "$lib/types";
import { isTauriRuntime } from "$lib/isTauri";
import {
  ACCENT_CANVAS_GRADIENT,
  ACCENT_QUOTE_COLOR,
  exportContentWidth,
} from "$lib/quoteCardStyles";

export interface QuotePngParams {
  quoteText: string;
  bookTitle: string;
  bookAuthor: string;
  pageLabel: string;
  chapterLabel: string;
  accent: QuoteAccent;
  layout: QuoteLayout;
  includePage: boolean;
  includeChapter: boolean;
  bgImageDataUrl?: string | null;
  bgImageOpacity?: number;
  overlayColor?: string;
  overlayOpacity?: number;
  bgScale?: number;
  bgFit?: QuoteBgFit;
}

function loadImage(src: string): Promise<HTMLImageElement> {
  return new Promise((resolve, reject) => {
    const img = new Image();
    img.onload = () => resolve(img);
    img.onerror = () => reject(new Error("Не удалось загрузить фон"));
    img.crossOrigin = "anonymous";
    img.src = src;
  });
}

function wrapLines(
  ctx: CanvasRenderingContext2D,
  text: string,
  maxWidth: number,
): string[] {
  const out: string[] = [];
  const paras = text.split(/\n/);
  for (let pi = 0; pi < paras.length; pi++) {
    const words = paras[pi].split(/\s+/).filter(Boolean);
    let line = "";
    for (const word of words) {
      const test = line ? `${line} ${word}` : word;
      if (ctx.measureText(test).width > maxWidth && line) {
        out.push(line);
        line = word;
      } else {
        line = test;
      }
    }
    if (line) out.push(line);
    if (pi < paras.length - 1) out.push("");
  }
  return out;
}

function roundRect(
  ctx: CanvasRenderingContext2D,
  x: number,
  y: number,
  w: number,
  h: number,
  r: number,
) {
  const rr = Math.min(r, w / 2, h / 2);
  ctx.beginPath();
  ctx.moveTo(x + rr, y);
  ctx.arcTo(x + w, y, x + w, y + h, rr);
  ctx.arcTo(x + w, y + h, x, y + h, rr);
  ctx.arcTo(x, y + h, x, y, rr);
  ctx.arcTo(x, y, x + w, y, rr);
  ctx.closePath();
}

function drawImageFit(
  ctx: CanvasRenderingContext2D,
  img: HTMLImageElement,
  dw: number,
  dh: number,
  zoom: number,
  fit: QuoteBgFit,
) {
  const iw = img.naturalWidth;
  const ih = img.naturalHeight;
  if (iw <= 0 || ih <= 0) return;
  const base =
    fit === "cover"
      ? Math.max(dw / iw, dh / ih)
      : Math.min(dw / iw, dh / ih);
  const scale = base * zoom;
  const tw = iw * scale;
  const th = ih * scale;
  const ox = (dw - tw) / 2;
  const oy = (dh - th) / 2;
  ctx.drawImage(img, ox, oy, tw, th);
}

export async function renderQuoteToCanvas(params: QuotePngParams): Promise<HTMLCanvasElement> {
  const {
    quoteText,
    bookTitle,
    bookAuthor,
    pageLabel,
    chapterLabel,
    accent,
    layout,
    includePage,
    includeChapter,
    bgImageDataUrl,
    bgImageOpacity = 1,
    overlayColor = "#1a1510",
    overlayOpacity = 0,
    bgScale = 100,
    bgFit = "cover",
  } = params;

  let bgImg: HTMLImageElement | null = null;
  if (bgImageDataUrl) {
    try {
      bgImg = await loadImage(bgImageDataUrl);
    } catch {
      bgImg = null;
    }
  }

  const dpr = Math.min(2, typeof window !== "undefined" ? window.devicePixelRatio || 2 : 2);
  const pad = 40;
  const innerW = exportContentWidth(layout);
  const scale = 1;

  const measure = document.createElement("canvas").getContext("2d")!;
  measure.font = `${22 * scale}px Georgia, "Times New Roman", serif`;
  const raw = quoteText.trim() || " ";
  const bodyLines = wrapLines(measure, raw, innerW);
  const lineHeight = 34 * scale;

  let bodyH = 0;
  for (const line of bodyLines) {
    bodyH += line === "" ? lineHeight * 0.35 : lineHeight;
  }

  const metaParts: string[] = [];
  const title = bookTitle.trim() || "Без названия";
  metaParts.push(title);
  if (bookAuthor.trim()) metaParts.push(bookAuthor.trim());

  const smallMeta: string[] = [];
  if (includePage && pageLabel) smallMeta.push(pageLabel);
  if (includeChapter && chapterLabel) smallMeta.push(chapterLabel);

  measure.font = `600 ${14 * scale}px system-ui, -apple-system, "Segoe UI", sans-serif`;
  const metaLine1 = metaParts.join(" · ");
  const metaLine2 = smallMeta.join(" · ");

  const metaH = 56 * scale + (metaLine2 ? 22 * scale : 0);
  const cardW = innerW + pad * 2;
  const cardH = pad + bodyH + 28 * scale + metaH + pad;

  const canvas = document.createElement("canvas");
  canvas.width = Math.round(cardW * dpr);
  canvas.height = Math.round(cardH * dpr);

  const xctx = canvas.getContext("2d")!;
  xctx.scale(dpr, dpr);
  xctx.textBaseline = "top";

  const radius = 20;
  xctx.save();
  roundRect(xctx, 0, 0, cardW, cardH, radius);
  xctx.clip();

  const [c0, c1] = ACCENT_CANVAS_GRADIENT[accent];
  const grd = xctx.createLinearGradient(0, 0, cardW, cardH);
  grd.addColorStop(0, c0);
  grd.addColorStop(1, c1);
  xctx.fillStyle = grd;
  xctx.fillRect(0, 0, cardW, cardH);

  if (bgImg) {
    xctx.save();
    xctx.globalAlpha = Math.min(1, Math.max(0, bgImageOpacity));
    drawImageFit(xctx, bgImg, cardW, cardH, bgScale / 100, bgFit);
    xctx.restore();
  }

  const ov = Math.min(1, Math.max(0, overlayOpacity));
  if (ov > 0.001) {
    xctx.save();
    xctx.globalAlpha = ov;
    xctx.fillStyle = overlayColor;
    xctx.fillRect(0, 0, cardW, cardH);
    xctx.restore();
  }

  xctx.restore();

  xctx.strokeStyle = "rgba(61, 56, 51, 0.12)";
  xctx.lineWidth = 1;
  roundRect(xctx, 0.5, 0.5, cardW - 1, cardH - 1, radius);
  xctx.stroke();

  const textColor = ACCENT_QUOTE_COLOR[accent];
  xctx.fillStyle = textColor;
  xctx.font = `${22 * scale}px Georgia, "Times New Roman", serif`;

  let y = pad;
  for (const line of bodyLines) {
    if (line === "") {
      y += lineHeight * 0.35;
      continue;
    }
    xctx.fillText(line, pad, y);
    y += lineHeight;
  }

  y += 16 * scale;
  xctx.strokeStyle =
    accent === "ink" ? "rgba(247, 242, 234, 0.25)" : "rgba(61, 56, 51, 0.15)";
  xctx.beginPath();
  xctx.moveTo(pad, y);
  xctx.lineTo(cardW - pad, y);
  xctx.stroke();
  y += 20 * scale;

  xctx.font = `600 ${14 * scale}px system-ui, -apple-system, sans-serif`;
  xctx.fillStyle = textColor;
  xctx.fillText(metaLine1, pad, y);
  y += 22 * scale;

  if (metaLine2) {
    xctx.font = `${12 * scale}px system-ui, sans-serif`;
    xctx.fillStyle =
      accent === "ink" ? "rgba(247, 242, 234, 0.72)" : "rgba(61, 56, 51, 0.62)";
    xctx.fillText(metaLine2, pad, y);
  }

  return canvas;
}

export async function quoteCanvasToPngBlob(canvas: HTMLCanvasElement): Promise<Blob> {
  return new Promise((resolve, reject) => {
    canvas.toBlob((b) => (b ? resolve(b) : reject(new Error("toBlob failed"))), "image/png");
  });
}

/** В Tauri WebView `navigator.clipboard.write` для изображений часто запрещён — пишем через нативный плагин. */
export async function copyQuotePngToClipboard(params: QuotePngParams): Promise<void> {
  const canvas = await renderQuoteToCanvas(params);
  const blob = await quoteCanvasToPngBlob(canvas);
  const bytes = new Uint8Array(await blob.arrayBuffer());

  if (isTauriRuntime()) {
    const { writeImage } = await import("@tauri-apps/plugin-clipboard-manager");
    const { Image } = await import("@tauri-apps/api/image");
    const image = await Image.fromBytes(bytes);
    await writeImage(image);
    return;
  }

  if (!navigator.clipboard?.write) {
    throw new Error("Буфер обмена недоступен в этом окружении");
  }
  await navigator.clipboard.write([
    new ClipboardItem({
      [blob.type]: blob,
    }),
  ]);
}
