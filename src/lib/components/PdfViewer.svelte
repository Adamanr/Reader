<script lang="ts">
  import { untrack } from "svelte";
  import { getDocument, GlobalWorkerOptions } from "pdfjs-dist";
  import type { PDFDocumentProxy } from "pdfjs-dist";
  import pdfWorker from "pdfjs-dist/build/pdf.worker.min.mjs?url";
  import "pdfjs-dist/legacy/web/pdf_viewer.css";
  import type {
    EpubReaderApi,
    Highlight,
    PdfOutlineItem,
    PdfReadyInfo,
    ReaderSelection,
    ReadingPosition,
    ReadingUnit,
    SearchHit,
    TextChunk,
  } from "$lib/types";
  import PdfPage, { type CropBox } from "$lib/components/PdfPage.svelte";
  import { extractReadablePageText, splitForTranslation } from "$lib/pdf/readablePageText";
  import { translateStringList } from "$lib/translate/translateApi";
  import { readLibraryBookBytes } from "$lib/library/readLibraryBookBytes";
  import { enqueueRender } from "$lib/pdf/renderQueue";
  import { reading, updateReading } from "$lib/reading/settings.svelte";
  import type { PagePalette } from "$lib/reading/palette";
  import { excerptAround, findAll, offsetOfPoint } from "$lib/reading/textAnchor";
  import { ensureAppFonts } from "$lib/reading/fonts";
  import { typographyVars, varsToStyle } from "$lib/reading/contentCss";

  GlobalWorkerOptions.workerSrc = pdfWorker;

  interface Props {
    relativePath: string;
    pageNum?: number;
    palette: PagePalette;
    highlights?: Highlight[];
    onPdfReady?: (info: PdfReadyInfo) => void;
    onPosition?: (p: ReadingPosition) => void;
    onReaderApi?: (api: EpubReaderApi | null) => void;
    onSelection?: (s: ReaderSelection | null) => void;
    onHighlightClick?: (id: string, rect: DOMRect) => void;
    onCenterTap?: () => void;
    /** Показать текст перевода страницы вместо скана */
    pdfTranslationPanel?: boolean;
    translateRunKey?: number;
    translateSource?: string;
    translateTarget?: string;
    onTranslateActivity?: (p: { busy: boolean; error: string | null }) => void;
    /** Полный перевод книги: сегменты по страницам (ключ — номер страницы строкой) */
    pdfInlineSpans?: Record<string, string[]> | null;
    pdfInlineShow?: boolean;
  }
  let {
    relativePath,
    pageNum = $bindable(1),
    palette,
    highlights = [],
    onPdfReady,
    onPosition,
    onReaderApi,
    onSelection,
    onHighlightClick,
    onCenterTap,
    pdfTranslationPanel = false,
    translateRunKey = 0,
    translateSource = "auto",
    translateTarget = "ru",
    onTranslateActivity,
    pdfInlineSpans = null,
    pdfInlineShow = true,
  }: Props = $props();

  ensureAppFonts();

  let scroller = $state<HTMLDivElement | null>(null);
  let transBodyEl = $state<HTMLDivElement | null>(null);
  let numPages = $state(0);
  let loading = $state(true);
  let err = $state<string | null>(null);
  let pdfDoc = $state<PDFDocumentProxy | null>(null);
  let outline: PdfOutlineItem[] = [];

  /** Размер первой страницы — по нему размечаем все, пока не узнаем точнее. */
  let defaultBase = $state({ w: 612, h: 792 });
  let baseOverrides = $state<Record<number, { w: number; h: number }>>({});
  let crop = $state<CropBox>({ x: 0, y: 0, w: 1, h: 1 });
  let viewW = $state(800);
  let viewH = $state(600);
  let visiblePages = $state<Set<number>>(new Set());
  let flash = $state<{ page: number; start: number; len: number; text?: string } | null>(null);
  /** Страница, о которой мы сами сообщили наружу при прокрутке */
  let reportedPage = 0;

  const mode = $derived(reading.s.pdfMode);
  const perRow = $derived(mode === "spread" ? 2 : 1);
  const GAP = 14;

  const pageTexts = new Map<number, string>();

  function baseOf(n: number) {
    return baseOverrides[n] ?? defaultBase;
  }

  const scale = $derived.by(() => {
    const s = reading.s;
    if (s.pdfFit === "custom") return s.pdfZoom;
    const b = defaultBase;
    const cw = b.w * crop.w * perRow + GAP * (perRow - 1);
    const ch = b.h * crop.h;
    const pad = viewW < 600 ? 12 : 48;
    const byWidth = Math.max(0.2, (viewW - pad) / cw);
    const capped = Math.min(byWidth, perRow === 1 && viewW > 900 ? 2.2 : 4);
    if (s.pdfFit === "page") return Math.min(capped, Math.max(0.2, (viewH - 28) / ch));
    return capped;
  });

  const rows = $derived.by(() => {
    const out: number[][] = [];
    for (let i = 1; i <= numPages; i += perRow) {
      const row = [i];
      if (perRow === 2 && i + 1 <= numPages) row.push(i + 1);
      out.push(row);
    }
    return out;
  });

  const shownRows = $derived.by(() => {
    if (mode === "continuous") return rows;
    const idx = Math.floor((pageNum - 1) / perRow);
    return rows[idx] ? [rows[idx]!] : [];
  });

  async function resolveDestToPageNumber(pdf: PDFDocumentProxy, dest: unknown): Promise<number | null> {
    if (dest == null) return null;
    if (typeof dest === "string") return resolveDestToPageNumber(pdf, await pdf.getDestination(dest));
    if (Array.isArray(dest) && dest.length > 0 && dest[0] && typeof dest[0] === "object") {
      try {
        return (await pdf.getPageIndex(dest[0] as Parameters<PDFDocumentProxy["getPageIndex"]>[0])) + 1;
      } catch {
        return null;
      }
    }
    return null;
  }

  async function flattenOutline(
    pdf: PDFDocumentProxy,
    nodes: Awaited<ReturnType<PDFDocumentProxy["getOutline"]>>,
    depth = 0,
  ): Promise<PdfOutlineItem[]> {
    const out: PdfOutlineItem[] = [];
    for (const node of nodes) {
      const title = node.title?.trim() || "Без названия";
      const page = await resolveDestToPageNumber(pdf, node.dest);
      out.push({ title, page, level: depth });
      if (node.items?.length) out.push(...(await flattenOutline(pdf, node.items, depth + 1)));
    }
    return out;
  }

  /** Белые поля: ищем границы содержимого на нескольких страницах. */
  async function detectCrop(doc: PDFDocumentProxy): Promise<CropBox> {
    let minX = 1;
    let minY = 1;
    let maxX = 0;
    let maxY = 0;
    const sample = [1, 2, 3, Math.ceil(doc.numPages / 2)].filter((p, i, a) => p <= doc.numPages && a.indexOf(p) === i);
    for (const p of sample) {
      const page = await doc.getPage(p);
      const vp = page.getViewport({ scale: 0.4 });
      const c = document.createElement("canvas");
      c.width = Math.ceil(vp.width);
      c.height = Math.ceil(vp.height);
      const ctx = c.getContext("2d", { willReadFrequently: true });
      if (!ctx) continue;
      await enqueueRender(doc, () => page.render({ canvas: c, canvasContext: ctx, viewport: vp, background: "#ffffff" }).promise);
      const { data, width, height } = ctx.getImageData(0, 0, c.width, c.height);
      for (let y = 0; y < height; y += 2) {
        for (let x = 0; x < width; x += 2) {
          const i = (y * width + x) * 4;
          if (data[i]! < 235 || data[i + 1]! < 235 || data[i + 2]! < 235) {
            minX = Math.min(minX, x / width);
            maxX = Math.max(maxX, x / width);
            minY = Math.min(minY, y / height);
            maxY = Math.max(maxY, y / height);
          }
        }
      }
    }
    if (maxX <= minX || maxY <= minY) return { x: 0, y: 0, w: 1, h: 1 };
    const m = 0.02;
    const x = Math.max(0, minX - m);
    const y = Math.max(0, minY - m);
    return { x, y, w: Math.min(1, maxX + m) - x, h: Math.min(1, maxY + m) - y };
  }

  async function loadPdf() {
    loading = true;
    err = null;
    pdfDoc?.destroy();
    pdfDoc = null;
    numPages = 0;
    baseOverrides = {};
    pageTexts.clear();
    onReaderApi?.(null);
    try {
      const bytes = await readLibraryBookBytes(relativePath);
      const doc = await getDocument({ data: bytes }).promise;
      const first = await doc.getPage(1);
      const vp = first.getViewport({ scale: 1 });
      defaultBase = { w: vp.width, h: vp.height };
      pdfDoc = doc;
      numPages = doc.numPages;
      if (pageNum > numPages || pageNum < 1) pageNum = 1;
      const rawOutline = await doc.getOutline();
      outline = rawOutline?.length ? await flattenOutline(doc, rawOutline) : [];
      onPdfReady?.({ outline, numPages });
      onReaderApi?.(buildApi());
    } catch (e) {
      err = String(e);
      onPdfReady?.({ outline: [], numPages: 0 });
    } finally {
      loading = false;
    }
  }

  $effect(() => {
    relativePath;
    untrack(() => void loadPdf());
  });

  $effect(() => {
    const doc = pdfDoc;
    const on = reading.s.pdfCrop;
    if (!doc || !on) {
      crop = { x: 0, y: 0, w: 1, h: 1 };
      return;
    }
    void detectCrop(doc).then((c) => {
      if (pdfDoc === doc && reading.s.pdfCrop) crop = c;
    });
  });

  $effect(() => {
    const el = scroller;
    if (!el) return;
    const ro = new ResizeObserver(() => {
      viewW = el.clientWidth;
      viewH = el.clientHeight;
    });
    ro.observe(el);
    return () => ro.disconnect();
  });

  /** Рендерим только страницы рядом с экраном — большие PDF не съедают память. */
  $effect(() => {
    const el = scroller;
    rows;
    if (!el || mode !== "continuous") return;
    const io = new IntersectionObserver(
      (entries) => {
        const next = new Set(visiblePages);
        for (const e of entries) {
          const n = Number((e.target as HTMLElement).dataset.row);
          if (e.isIntersecting) next.add(n);
          else next.delete(n);
        }
        visiblePages = next;
      },
      { root: el, rootMargin: "120% 0px" },
    );
    const t = setTimeout(() => {
      for (const r of Array.from(el.querySelectorAll<HTMLElement>("[data-row]"))) io.observe(r);
    }, 0);
    return () => {
      clearTimeout(t);
      io.disconnect();
    };
  });

  function rowTop(rowIdx: number): number {
    const el = scroller?.querySelector<HTMLElement>(`[data-row="${rowIdx}"]`);
    return el ? el.offsetTop : 0;
  }

  function scrollToPage(n: number, smooth = false) {
    const el = scroller;
    if (!el) return;
    const idx = Math.floor((n - 1) / perRow);
    el.scrollTo({ top: Math.max(0, rowTop(idx) - 12), behavior: smooth ? "smooth" : "auto" });
  }

  /** Внешняя смена страницы (оглавление, ссылки) — прокручиваем к ней. */
  $effect(() => {
    const n = pageNum;
    const m = mode;
    if (loading || !numPages) return;
    if (m !== "continuous") {
      scroller?.scrollTo({ top: 0 });
      return;
    }
    if (n === reportedPage) return;
    requestAnimationFrame(() => scrollToPage(n));
    reportedPage = n;
  });

  /** Масштаб поменялся — остаёмся на той же странице. */
  $effect(() => {
    scale;
    perRow;
    const n = untrack(() => pageNum);
    if (untrack(() => mode) !== "continuous") return;
    requestAnimationFrame(() => scrollToPage(n));
  });

  function currentFromScroll() {
    const el = scroller;
    if (!el || mode !== "continuous") return;
    const line = el.scrollTop + el.clientHeight * 0.3;
    // Строки — прямые потомки прокрутки: живая коллекция без выборки на каждом кадре.
    const rowsEls = el.children as HTMLCollectionOf<HTMLElement>;
    let lo = 0;
    let hi = rowsEls.length - 1;
    while (lo < hi) {
      const mid = (lo + hi + 1) >> 1;
      if (rowsEls[mid]!.offsetTop <= line) lo = mid;
      else hi = mid - 1;
    }
    const n = lo * perRow + 1;
    if (n !== pageNum) {
      reportedPage = n;
      pageNum = n;
      return; // позицию отправит эффект смены страницы
    }
    // Внутри страницы обновляем прогресс не чаще, чем на заметный шаг.
    const within = Math.floor(((el.scrollTop - (rowsEls[lo]?.offsetTop ?? 0)) / Math.max(1, rowsEls[lo]?.offsetHeight ?? 1)) * 8);
    if (within !== lastWithin) {
      lastWithin = within;
      emitPosition();
    }
  }

  let lastWithin = -1;

  let scrollRaf = 0;
  function onScroll() {
    if (scrollRaf) return;
    scrollRaf = requestAnimationFrame(() => {
      scrollRaf = 0;
      currentFromScroll();
    });
  }

  function chapterAt(n: number): { label: string; next: number } {
    let label = "";
    let best = -1;
    let next = numPages + 1;
    for (const o of outline) {
      if (o.page == null) continue;
      if (o.page <= n && o.page >= best) {
        best = o.page;
        label = o.title;
      }
      if (o.page > n && o.page < next) next = o.page;
    }
    return { label, next };
  }

  function emitPosition() {
    if (!numPages) return;
    const n = pageNum;
    const { label, next } = chapterAt(n);
    let within = 0;
    const el = scroller;
    if (el && mode === "continuous") {
      const idx = Math.floor((n - 1) / perRow);
      const rowEl = el.querySelector<HTMLElement>(`[data-row="${idx}"]`);
      if (rowEl) within = Math.min(1, Math.max(0, (el.scrollTop - rowEl.offsetTop) / Math.max(1, rowEl.offsetHeight)));
    }
    const atEnd = el ? el.scrollTop + el.clientHeight >= el.scrollHeight - 4 : false;
    onPosition?.({
      progress: atEnd ? 1 : Math.min(1, (n - 1 + within) / numPages),
      chapterLabel: label,
      charsLeftChapter: null,
      charsLeftBook: null,
      pagesLeftChapter: Math.max(0, next - n - within),
      pagesLeftBook: Math.max(0, numPages - n + 1 - within),
      pageLabel: `${n} / ${numPages}`,
    });
  }

  $effect(() => {
    pageNum;
    numPages;
    untrack(emitPosition);
  });

  function go(delta: number) {
    const target = Math.min(numPages, Math.max(1, pageNum + delta * perRow));
    if (mode === "continuous") scrollToPage(target, true);
    pageNum = target;
  }

  let wheelAccum = 0;
  let wheelReset = 0;
  function onWheel(e: WheelEvent) {
    if (e.ctrlKey || e.metaKey) {
      e.preventDefault();
      const next = Math.min(5, Math.max(0.3, scale * (e.deltaY < 0 ? 1.08 : 1 / 1.08)));
      updateReading({ pdfFit: "custom", pdfZoom: Number(next.toFixed(3)) });
      return;
    }
    if (mode === "continuous" || pdfTranslationPanel) return;
    const el = scroller;
    // Внутри увеличенной страницы сначала прокручиваем её саму.
    if (el && el.scrollHeight > el.clientHeight + 4) {
      const atTop = el.scrollTop <= 0;
      const atBottom = el.scrollTop + el.clientHeight >= el.scrollHeight - 2;
      if ((e.deltaY > 0 && !atBottom) || (e.deltaY < 0 && !atTop)) return;
    }
    e.preventDefault();
    wheelAccum += e.deltaY;
    window.clearTimeout(wheelReset);
    wheelReset = window.setTimeout(() => (wheelAccum = 0), 160);
    if (wheelAccum >= 50) {
      go(1);
      wheelAccum = 0;
    } else if (wheelAccum <= -50) {
      go(-1);
      wheelAccum = 0;
    }
  }

  let touchX = 0;
  let touchY = 0;
  function onTouchStart(e: TouchEvent) {
    touchX = e.touches[0]?.clientX ?? 0;
    touchY = e.touches[0]?.clientY ?? 0;
  }
  function onTouchEnd(e: TouchEvent) {
    if (mode === "continuous") return;
    const t = e.changedTouches[0];
    if (!t) return;
    const dx = t.clientX - touchX;
    const dy = t.clientY - touchY;
    if (Math.abs(dx) > 50 && Math.abs(dy) < 60) go(dx < 0 ? 1 : -1);
  }

  function onStageClick(e: MouseEvent) {
    const sel = document.getSelection();
    if (sel && !sel.isCollapsed) return;
    if ((e.target as Element).closest("a, button, mark")) return;
    const el = scroller;
    if (!el) return;
    const r = el.getBoundingClientRect();
    const x = (e.clientX - r.left) / r.width;
    if (reading.s.tapZones && x < 0.2) go(-1);
    else if (reading.s.tapZones && x > 0.8) go(1);
    else onCenterTap?.();
  }

  function refreshSelection() {
    if (loading || err) return;
    const sel = document.getSelection();
    if (!sel || sel.isCollapsed || sel.rangeCount === 0) {
      onSelection?.(null);
      return;
    }
    const inPages = scroller?.contains(sel.anchorNode) ?? false;
    const inTrans = transBodyEl?.contains(sel.anchorNode) ?? false;
    if (!inPages && !inTrans) return;
    const text = sel.toString().trim();
    if (!text) {
      onSelection?.(null);
      return;
    }
    const range = sel.getRangeAt(0);
    const rect = range.getBoundingClientRect();
    const node = range.startContainer.nodeType === Node.ELEMENT_NODE ? (range.startContainer as Element) : range.startContainer.parentElement;
    const pageEl = node?.closest("[data-page]") as HTMLElement | null;
    const layer = pageEl?.querySelector(".textLayer");
    const page = pageEl ? Number(pageEl.dataset.page) : pageNum;
    const offset = layer ? offsetOfPoint(layer, range.startContainer, range.startOffset) : null;
    const layerText = inTrans ? (transBodyEl?.textContent ?? "") : (layer?.textContent ?? "");
    const at = inTrans ? layerText.indexOf(text) : (offset ?? 0);
    onSelection?.({
      text,
      context: layerText.slice(Math.max(0, at - 300), at + text.length + 300),
      rect: { left: rect.left, top: rect.top, width: rect.width, height: rect.height },
      anchor: {
        page,
        offset: inTrans ? undefined : (offset ?? undefined),
        chapterLabel: chapterAt(page).label || undefined,
      },
      clear: () => document.getSelection()?.removeAllRanges(),
    });
  }

  $effect(() => {
    const onUp = () => requestAnimationFrame(refreshSelection);
    document.addEventListener("mouseup", onUp);
    document.addEventListener("keyup", onUp);
    document.addEventListener("touchend", onUp);
    return () => {
      document.removeEventListener("mouseup", onUp);
      document.removeEventListener("keyup", onUp);
      document.removeEventListener("touchend", onUp);
    };
  });

  /** Текст страницы в том же виде, что у текстового слоя (для смещений). */
  async function pageText(n: number): Promise<string> {
    const cached = pageTexts.get(n);
    if (cached != null) return cached;
    if (!pdfDoc) return "";
    const page = await pdfDoc.getPage(n);
    const content = await page.getTextContent();
    const text = content.items.map((it) => ("str" in it ? it.str : "")).join("");
    pageTexts.set(n, text);
    return text;
  }

  async function search(query: string, signal?: AbortSignal): Promise<SearchHit[]> {
    const hits: SearchHit[] = [];
    const q = query.trim();
    if (!q) return hits;
    for (let n = 1; n <= numPages; n++) {
      if (signal?.aborted) break;
      const text = await pageText(n);
      for (const start of findAll(text, q, 50)) {
        hits.push({
          id: `p:${n}:${start}`,
          label: `стр. ${n}${chapterAt(n).label ? " · " + chapterAt(n).label : ""}`,
          ...excerptAround(text, start, start + q.length),
          loc: `p:${n}:${start}:${q.length}`,
        });
      }
      if (hits.length >= 400) break;
      if (n % 20 === 0) await new Promise((r) => setTimeout(r, 0));
    }
    return hits;
  }

  async function goToHit(hit: SearchHit) {
    const [, p, start, len] = hit.loc.split(":");
    const n = Number(p);
    pageNum = n;
    if (mode === "continuous") scrollToPage(n);
    flash = null;
    setTimeout(() => (flash = { page: n, start: Number(start), len: Number(len), text: hit.match || undefined }), 250);
  }

  async function unitsFromHere(): Promise<ReadingUnit[]> {
    if (!pdfDoc) return [];
    const n = pageNum;
    const page = await pdfDoc.getPage(n);
    const text = await extractReadablePageText(page, 1);
    return text
      .split(/\n\s*\n/)
      .map((p) => p.replace(/\s+/g, " ").trim())
      .filter(Boolean)
      .map((t) => ({
        text: t,
        reveal: async () => {
          if (pageNum !== n) {
            pageNum = n;
            if (mode === "continuous") scrollToPage(n);
          }
        },
      }));
  }

  async function advanceChapter(): Promise<boolean> {
    if (pageNum >= numPages) return false;
    go(1);
    await new Promise((r) => setTimeout(r, 120));
    return true;
  }

  async function chunksBefore(): Promise<TextChunk[]> {
    const out: TextChunk[] = [];
    const last = Math.min(numPages, pageNum + perRow - 1);
    let buf: string[] = [];
    let label = "";
    let startPage = 1;
    const flush = (end: number, partial: boolean) => {
      const text = buf.join(" ").replace(/\s+/g, " ").trim();
      if (text.length > 40) {
        out.push({
          id: `pages-${startPage}-${end}${partial ? "-partial" : ""}`,
          label: label || `Стр. ${startPage}–${end}`,
          text,
          partial,
        });
      }
      buf = [];
    };
    for (let n = 1; n <= last; n++) {
      const ch = chapterAt(n).label;
      const newChunk = outline.length ? ch !== label : (n - 1) % 20 === 0;
      if (newChunk && n > 1) {
        flush(n - 1, false);
        startPage = n;
      }
      if (newChunk) label = outline.length ? ch : "";
      buf.push(await pageText(n));
    }
    flush(last, true);
    return out;
  }

  function buildApi(): EpubReaderApi {
    return {
      toc: [],
      spine: [],
      goTo: async () => {},
      prev: async () => go(-1),
      next: async () => go(1),
      seek: async (f: number) => {
        const n = Math.min(numPages, Math.max(1, Math.round(f * numPages) || 1));
        pageNum = n;
        if (mode === "continuous") scrollToPage(n);
      },
      search,
      goToHit,
      unitsFromHere,
      advanceChapter,
      chunksBefore,
    };
  }

  const NO_HIGHLIGHTS: Highlight[] = [];
  /** Выделения по страницам: одни и те же массивы, пока выделения не менялись. */
  const highlightsByPage = $derived.by(() => {
    const m = new Map<number, Highlight[]>();
    for (const h of highlights) {
      if (h.page == null || h.offset == null) continue;
      const list = m.get(h.page) ?? [];
      list.push(h);
      m.set(h.page, list);
    }
    return m;
  });

  function highlightsFor(n: number): Highlight[] {
    return highlightsByPage.get(n) ?? NO_HIGHLIGHTS;
  }

  function isActive(rowIdx: number): boolean {
    if (mode !== "continuous") return true;
    return visiblePages.has(rowIdx);
  }

  function setBase(n: number, w: number, h: number) {
    if (n === 1) defaultBase = { w, h };
    else baseOverrides = { ...baseOverrides, [n]: { w, h } };
  }

  // ——— Перевод страницы (колонка вместо скана) ———
  let pdfSideText = $state("");
  let pdfSideErr = $state<string | null>(null);

  async function refreshPdfTranslation() {
    if (!pdfTranslationPanel || !pdfDoc || pageNum < 1) {
      pdfSideText = "";
      pdfSideErr = null;
      return;
    }
    onTranslateActivity?.({ busy: true, error: null });
    pdfSideErr = null;
    try {
      const page = await pdfDoc.getPage(pageNum);
      const structured = await extractReadablePageText(page, 1);
      if (!structured.trim()) {
        pdfSideText = "На этой странице нет извлекаемого текста.";
        onTranslateActivity?.({ busy: false, error: null });
        return;
      }
      const translated = await translateStringList(splitForTranslation(structured), translateSource, translateTarget);
      pdfSideText = translated.join("\n\n");
      onTranslateActivity?.({ busy: false, error: null });
    } catch (e) {
      pdfSideErr = e instanceof Error ? e.message : String(e);
      pdfSideText = "";
      onTranslateActivity?.({ busy: false, error: pdfSideErr });
    }
  }

  $effect(() => {
    pdfTranslationPanel;
    translateRunKey;
    pageNum;
    pdfDoc;
    untrack(() => void refreshPdfTranslation());
  });

  const zoomPct = $derived(Math.round(scale * 100));
  const invert = $derived(reading.s.pdfInvert);
  const transStyle = $derived(varsToStyle(typographyVars(reading.s, palette)));

  function zoomBy(f: number) {
    updateReading({ pdfFit: "custom", pdfZoom: Number(Math.min(5, Math.max(0.3, scale * f)).toFixed(3)) });
  }
</script>

<div class="pdf-root" style:--page-bg={palette.bg}>
  {#if loading}
    <p class="hint">Загрузка PDF…</p>
  {:else if err}
    <p class="error">{err}</p>
  {:else if pdfTranslationPanel}
    <div class="pdf-trans-wrap" style={transStyle}>
      {#if pdfSideErr}
        <p class="pdf-trans-err">{pdfSideErr}</p>
      {:else}
        <article class="pdf-trans-article" aria-label="Перевод страницы {pageNum}">
          <p class="pdf-trans-meta">Страница {pageNum} из {numPages}</p>
          <div class="pdf-trans-body" bind:this={transBodyEl}>{pdfSideText}</div>
        </article>
      {/if}
    </div>
  {:else}
    <!-- svelte-ignore a11y_click_events_have_key_events -->
    <!-- svelte-ignore a11y_no_static_element_interactions -->
    <div
      class="pdf-scroller"
      class:single={mode !== "continuous"}
      bind:this={scroller}
      onscroll={onScroll}
      onwheel={onWheel}
      onclick={onStageClick}
      ontouchstart={onTouchStart}
      ontouchend={onTouchEnd}
    >
      {#each shownRows as row (row[0])}
        {@const rowIdx = Math.floor((row[0]! - 1) / perRow)}
        <div class="pdf-row" data-row={rowIdx} style:gap="{GAP}px">
          {#each row as n (n)}
            <PdfPage
              doc={pdfDoc!}
              {n}
              {scale}
              base={baseOf(n)}
              {crop}
              active={isActive(rowIdx)}
              {invert}
              highlights={highlightsFor(n)}
              flash={flash?.page === n ? flash : null}
              inlineSpans={pdfInlineSpans?.[String(n)] ?? null}
              inlineShow={pdfInlineShow}
              onLink={(p) => {
                pageNum = p;
                if (mode === "continuous") scrollToPage(p);
              }}
              onBase={setBase}
              {onHighlightClick}
            />
          {/each}
        </div>
      {/each}
    </div>
    <div class="pdf-zoom" role="group" aria-label="Масштаб">
      <button type="button" title="Уменьшить" onclick={() => zoomBy(1 / 1.15)}>−</button>
      <button
        type="button"
        class="pdf-zoom-val"
        title="Переключить: по ширине / по странице"
        onclick={() => updateReading({ pdfFit: reading.s.pdfFit === "width" ? "page" : "width" })}
      >
        {reading.s.pdfFit === "width" ? "по ширине" : reading.s.pdfFit === "page" ? "страница" : `${zoomPct}%`}
      </button>
      <button type="button" title="Увеличить" onclick={() => zoomBy(1.15)}>+</button>
    </div>
  {/if}
</div>

<style>
  .pdf-root {
    position: relative;
    display: flex;
    flex-direction: column;
    height: 100%;
    min-height: 0;
    background: color-mix(in srgb, var(--page-bg) 70%, var(--reader-bg));
  }

  .pdf-scroller {
    flex: 1;
    min-height: 0;
    overflow: auto;
    padding: 1.2rem 0.75rem 40vh;
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 18px;
    overscroll-behavior: contain;
  }

  .pdf-scroller.single {
    justify-content: safe center;
    padding-bottom: 1.2rem;
  }

  .pdf-row {
    display: flex;
    justify-content: center;
    min-width: min-content;
  }

  .pdf-zoom {
    position: absolute;
    right: 1rem;
    bottom: 0.8rem;
    display: flex;
    align-items: center;
    gap: 2px;
    padding: 3px;
    border-radius: 999px;
    background: color-mix(in srgb, var(--toolbar-surface) 92%, transparent);
    border: 1px solid var(--toolbar-border);
    box-shadow: var(--shadow-soft);
    opacity: 0.55;
    transition: opacity 0.2s ease;
    z-index: 5;
  }

  .pdf-zoom:hover,
  .pdf-zoom:focus-within {
    opacity: 1;
  }

  .pdf-zoom button {
    border: none;
    background: transparent;
    color: var(--text-soft);
    min-width: 2rem;
    height: 2rem;
    border-radius: 999px;
    cursor: pointer;
    font-size: 1rem;
  }

  .pdf-zoom button:hover {
    background: var(--panel-soft);
  }

  .pdf-zoom .pdf-zoom-val {
    font-size: 0.76rem;
    font-variant-numeric: tabular-nums;
    padding: 0 0.5rem;
    color: var(--muted);
  }

  .pdf-trans-wrap {
    flex: 1;
    overflow: auto;
    background: var(--rd-bg);
    color: var(--rd-text);
  }

  .pdf-trans-article {
    width: 100%;
    max-width: var(--rd-measure);
    margin: 0 auto;
    padding: 2rem var(--rd-margin) 3rem;
    box-sizing: content-box;
  }

  .pdf-trans-meta {
    margin: 0 0 1.2rem;
    font-size: 0.72rem;
    font-weight: 650;
    text-transform: uppercase;
    letter-spacing: 0.12em;
    color: var(--rd-muted);
    font-family: system-ui, sans-serif;
  }

  .pdf-trans-body {
    font-family: var(--rd-font);
    font-size: var(--rd-size);
    line-height: var(--rd-line);
    white-space: pre-wrap;
    text-align: var(--rd-align);
    hyphens: var(--rd-hyphens);
    user-select: text;
  }

  .pdf-trans-err {
    margin: 1rem auto;
    max-width: 42rem;
    padding: 0 1rem;
    font-size: 0.88rem;
    color: var(--danger);
    line-height: 1.45;
  }

  .hint,
  .error {
    margin: auto;
    color: var(--muted);
  }

  .error {
    color: var(--danger);
    padding: 1rem;
    text-align: center;
  }

  @media (max-width: 600px) {
    .pdf-scroller {
      padding: 0.5rem 0.25rem 30vh;
      gap: 10px;
    }
    .pdf-zoom {
      right: 0.6rem;
      bottom: max(0.6rem, env(safe-area-inset-bottom));
    }
  }
</style>
