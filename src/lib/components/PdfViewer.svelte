<script lang="ts">
  import { tick } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import {
    AnnotationType,
    getDocument,
    GlobalWorkerOptions,
    TextLayer,
  } from "pdfjs-dist";
  import type { PDFDocumentProxy } from "pdfjs-dist";
  import pdfWorker from "pdfjs-dist/build/pdf.worker.min.mjs?url";
  import type { PdfOutlineItem, PdfReadyInfo, QuoteDraftOptions } from "$lib/types";
  import { writeTextToClipboard } from "$lib/clipboardWrite";
  import "pdfjs-dist/legacy/web/pdf_viewer.css";
  import ReaderSelectionToolbar from "$lib/components/ReaderSelectionToolbar.svelte";
  import CommentDialog from "$lib/components/CommentDialog.svelte";
  import QuoteDialog from "$lib/components/QuoteDialog.svelte";
  import {
    extractReadablePageText,
    splitForTranslation,
  } from "$lib/pdf/readablePageText";
  import { translateStringList } from "$lib/translate/translateApi";

  GlobalWorkerOptions.workerSrc = pdfWorker;

  interface Props {
    relativePath: string;
    variant?: "full" | "reader";
    pageNum?: number;
    scale?: number;
    onPdfReady?: (info: PdfReadyInfo) => void;
    displayTitle?: string;
    displayAuthor?: string;
    chapterLabel?: string;
    onAddComment?: (c: {
      body: string;
      excerpt: string;
      page?: number;
      chapterLabel?: string;
    }) => void;
    onAddQuote?: (q: { text: string; options: QuoteDraftOptions }) => void;
    /** Показать колонку с текстом перевода страницы */
    pdfTranslationPanel?: boolean;
    translateRunKey?: number;
    translateSource?: string;
    translateTarget?: string;
    onTranslateActivity?: (p: { busy: boolean; error: string | null }) => void;
    /** Полный перевод книги: сегменты по страницам (ключ — номер страницы строкой) */
    pdfInlineSpans?: Record<string, string[]> | null;
    /** Показывать перевод поверх скана (иначе только для выделения, прозрачный текст) */
    pdfInlineShow?: boolean;
  }
  let {
    relativePath,
    variant = "full",
    pageNum = $bindable(1),
    scale = $bindable(1),
    onPdfReady,
    displayTitle = "",
    displayAuthor = "",
    chapterLabel = "",
    onAddComment,
    onAddQuote,
    pdfTranslationPanel = false,
    translateRunKey = 0,
    translateSource = "auto",
    translateTarget = "ru",
    onTranslateActivity,
    pdfInlineSpans = null,
    pdfInlineShow = true,
  }: Props = $props();

  let canvas = $state<HTMLCanvasElement | null>(null);
  /** Область прокрутки чтения (страница PDF или текст перевода) — для колеса страниц */
  let scrollHost = $state<HTMLDivElement | null>(null);
  let pageStackEl = $state<HTMLDivElement | null>(null);
  let textLayerEl = $state<HTMLDivElement | null>(null);
  let linkLayerEl = $state<HTMLDivElement | null>(null);
  /** Выделение в режиме перевода (текст вместо страницы) */
  let transBodyEl = $state<HTMLDivElement | null>(null);
  let numPages = $state(0);
  let loading = $state(true);
  let err = $state<string | null>(null);
  let pdfDoc = $state<PDFDocumentProxy | null>(null);
  let textLayerInst: InstanceType<typeof TextLayer> | null = null;
  let pageStackStyle = $state("");

  let toolbarVisible = $state(false);
  let toolbarX = $state(0);
  let toolbarY = $state(0);
  let selectedText = $state("");

  let commentOpen = $state(false);
  let quoteOpen = $state(false);

  const interactionsEnabled = $derived(!!(onAddComment && onAddQuote));

  async function resolveDestToPageNumber(
    pdf: PDFDocumentProxy,
    dest: string | unknown[] | null | undefined,
  ): Promise<number | null> {
    if (dest == null) return null;
    if (typeof dest === "string") {
      const resolved = await pdf.getDestination(dest);
      return resolveDestToPageNumber(pdf, resolved);
    }
    if (Array.isArray(dest) && dest.length > 0) {
      const target = dest[0];
      if (target && typeof target === "object") {
        try {
          const idx = await pdf.getPageIndex(target as Parameters<PDFDocumentProxy["getPageIndex"]>[0]);
          return idx + 1;
        } catch {
          return null;
        }
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
      const pad = "\u2003".repeat(depth);
      const title = pad + (node.title?.trim() || "Без названия");
      const page = await resolveDestToPageNumber(pdf, node.dest);
      out.push({ title, page });
      if (node.items?.length) {
        out.push(...(await flattenOutline(pdf, node.items, depth + 1)));
      }
    }
    return out;
  }

  async function loadPdf() {
    loading = true;
    err = null;
    pdfDoc = null;
    numPages = 0;
    pageNum = 1;
    textLayerInst?.cancel();
    textLayerInst = null;
    try {
      const b64 = await invoke<string>("read_book_base64", { relativePath });
      const bin = atob(b64);
      const bytes = new Uint8Array(bin.length);
      for (let i = 0; i < bin.length; i++) bytes[i] = bin.charCodeAt(i);
      const loadingTask = getDocument({ data: bytes });
      const doc = await loadingTask.promise;
      pdfDoc = doc;
      numPages = doc.numPages;

      const rawOutline = await doc.getOutline();
      const outline = rawOutline?.length ? await flattenOutline(doc, rawOutline) : [];
      onPdfReady?.({ outline, numPages });
    } catch (e) {
      err = String(e);
      onPdfReady?.({ outline: [], numPages: 0 });
    } finally {
      loading = false;
    }
  }

  async function renderLinks(
    page: import("pdfjs-dist").PDFPageProxy,
    viewport: import("pdfjs-dist").PageViewport,
    pdf: PDFDocumentProxy,
    layer: HTMLDivElement,
  ) {
    layer.innerHTML = "";
    layer.style.width = `${viewport.width}px`;
    layer.style.height = `${viewport.height}px`;
    const annotations = await page.getAnnotations({ intent: "display" });
    for (const ann of annotations) {
      if (ann.annotationType !== AnnotationType.LINK) continue;
      const rect = ann.rect;
      if (!rect || rect.length < 4) continue;
      const [vx1, vy1, vx2, vy2] = viewport.convertToViewportRectangle(rect);
      const left = Math.min(vx1, vx2);
      const top = Math.min(vy1, vy2);
      const w = Math.abs(vx2 - vx1);
      const h = Math.abs(vy2 - vy1);
      const box = document.createElement("div");
      box.className = "pdf-link-box";
      box.style.left = `${left}px`;
      box.style.top = `${top}px`;
      box.style.width = `${w}px`;
      box.style.height = `${h}px`;
      const url = ann.url || ann.unsafeUrl;
      if (typeof url === "string" && url.length > 0) {
        const a = document.createElement("a");
        a.href = url;
        a.target = "_blank";
        a.rel = "noopener noreferrer";
        a.className = "pdf-link-hit";
        a.textContent = "";
        box.appendChild(a);
        layer.appendChild(box);
      } else if (ann.dest != null) {
        const destVal = ann.dest;
        const btn = document.createElement("button");
        btn.type = "button";
        btn.className = "pdf-link-hit";
        btn.addEventListener("click", async () => {
          const targetPage = await resolveDestToPageNumber(pdf, destVal);
          if (targetPage != null) pageNum = targetPage;
        });
        box.appendChild(btn);
        layer.appendChild(box);
      }
    }
  }

  let renderToken = 0;

  $effect(() => {
    if (!pdfDoc || !canvas || !pageStackEl || !textLayerEl || !linkLayerEl) return;
    if (loading || err) return;

    const doc = pdfDoc;
    const pn = pageNum;
    const sc = scale;
    const inlineSpans = pdfInlineSpans;
    const inlineShow = pdfInlineShow;
    const myToken = ++renderToken;

    void (async () => {
      textLayerInst?.cancel();
      textLayerInst = null;
      textLayerEl!.innerHTML = "";
      linkLayerEl!.innerHTML = "";

      const page = await doc.getPage(pn);
      if (myToken !== renderToken) return;

      const viewport = page.getViewport({ scale });
      pageStackStyle = `width:${viewport.width}px;height:${viewport.height}px`;

      await tick();

      if (myToken !== renderToken) return;

      canvas.width = viewport.width;
      canvas.height = viewport.height;
      const ctx = canvas.getContext("2d");
      if (!ctx) return;

      await page
        .render({
          canvas,
          canvasContext: ctx,
          viewport,
          background: "#ffffff",
          /** Должен совпадать с intent в getOptionalContentConfig (иначе pdf.js бросает и страница белая). */
          intent: "any",
          optionalContentConfigPromise: doc.getOptionalContentConfig({ intent: "any" }),
        })
        .promise;
      if (myToken !== renderToken) return;

      textLayerInst = new TextLayer({
        textContentSource: page.streamTextContent(),
        container: textLayerEl,
        viewport,
      });
      await textLayerInst.render();
      if (myToken !== renderToken) return;

      const inline = inlineSpans?.[String(pn)];
      if (
        inline &&
        textLayerInst &&
        inline.length === textLayerInst.textContentItemsStr.length
      ) {
        const divs = textLayerInst.textDivs;
        for (let i = 0; i < inline.length; i++) {
          const el = divs[i];
          if (!el) continue;
          el.textContent = inline[i] ?? "";
          if (inlineShow) el.classList.add("pdf-inline-tr");
          else el.classList.remove("pdf-inline-tr");
        }
        textLayerInst.update({ viewport });
      } else if (textLayerInst) {
        for (const el of textLayerInst.textDivs) {
          el.classList.remove("pdf-inline-tr");
        }
      }

      if (myToken !== renderToken) return;

      await renderLinks(page, viewport, doc, linkLayerEl);
    })();

    return () => {
      renderToken += 1;
      textLayerInst?.cancel();
      textLayerInst = null;
    };
  });

  $effect(() => {
    relativePath;
    void loadPdf();
  });

  function prev() {
    if (pageNum > 1) pageNum -= 1;
  }
  function next() {
    if (pageNum < numPages) pageNum += 1;
  }

  function keyNavAllowed(target: EventTarget | null) {
    const el = target as HTMLElement | null;
    if (!el) return true;
    return !el.closest("input, textarea, select, [contenteditable=true]");
  }

  function onWindowKeydown(e: KeyboardEvent) {
    if (loading || err || !pdfDoc || numPages === 0) return;
    if (!keyNavAllowed(e.target)) return;
    if (e.key === "ArrowLeft") {
      e.preventDefault();
      prev();
    } else if (e.key === "ArrowRight") {
      e.preventDefault();
      next();
    }
  }

  let wheelAccum = 0;
  let wheelReset = 0;

  $effect(() => {
    const el = scrollHost;
    if (!el || loading || err || !pdfDoc) return;

    const onWheel = (e: WheelEvent) => {
      if (e.ctrlKey || e.metaKey) return;
      e.preventDefault();

      wheelAccum += e.deltaY;
      window.clearTimeout(wheelReset);
      wheelReset = window.setTimeout(() => {
        wheelAccum = 0;
      }, 160);

      const threshold = 50;
      if (wheelAccum >= threshold) {
        next();
        wheelAccum = 0;
      } else if (wheelAccum <= -threshold) {
        prev();
        wheelAccum = 0;
      }
    };

    el.addEventListener("wheel", onWheel, { passive: false });
    return () => {
      el.removeEventListener("wheel", onWheel);
      window.clearTimeout(wheelReset);
    };
  });

  function refreshToolbar() {
    if (!interactionsEnabled || loading || err) {
      toolbarVisible = false;
      return;
    }
    const sel = document.getSelection();
    if (!sel || sel.isCollapsed) {
      toolbarVisible = false;
      return;
    }
    const inLayer = textLayerEl?.contains(sel.anchorNode) ?? false;
    const inTrans = transBodyEl?.contains(sel.anchorNode) ?? false;
    if (!inLayer && !inTrans) {
      toolbarVisible = false;
      return;
    }
    const t = sel.toString().trim();
    if (!t) {
      toolbarVisible = false;
      return;
    }
    selectedText = t;
    const range = sel.getRangeAt(0);
    const rect = range.getBoundingClientRect();
    toolbarX = rect.left + rect.width / 2;
    toolbarY = rect.top;
    toolbarVisible = true;
  }

  $effect(() => {
    if (!interactionsEnabled) return;
    const onSel = () => requestAnimationFrame(refreshToolbar);
    const onMouseUp = () => requestAnimationFrame(refreshToolbar);
    document.addEventListener("selectionchange", onSel);
    document.addEventListener("mouseup", onMouseUp);
    return () => {
      document.removeEventListener("selectionchange", onSel);
      document.removeEventListener("mouseup", onMouseUp);
    };
  });

  async function doCopy() {
    const t = selectedText.trim();
    if (!t) return;
    try {
      await writeTextToClipboard(t);
    } catch {
      /* ignore */
    }
    toolbarVisible = false;
    document.getSelection()?.removeAllRanges();
  }

  function openComment() {
    if (!selectedText.trim()) return;
    commentOpen = true;
    toolbarVisible = false;
  }

  function openQuote() {
    if (!selectedText.trim()) return;
    quoteOpen = true;
    toolbarVisible = false;
  }

  function saveComment(body: string) {
    onAddComment?.({
      body,
      excerpt: selectedText.trim().slice(0, 2000),
      page: pageNum,
    });
    document.getSelection()?.removeAllRanges();
  }

  function saveQuote(opts: QuoteDraftOptions) {
    onAddQuote?.({ text: selectedText.trim(), options: opts });
    document.getSelection()?.removeAllRanges();
  }

  const zoomPct = $derived(Math.round(scale * 100));
  const pageLabelStr = $derived(`стр. ${pageNum}`);

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
      const structured = await extractReadablePageText(page, scale);
      if (!structured.trim()) {
        pdfSideText = "На этой странице нет извлекаемого текста.";
        onTranslateActivity?.({ busy: false, error: null });
        return;
      }
      const chunks = splitForTranslation(structured);
      const translated = await translateStringList(
        chunks,
        translateSource,
        translateTarget,
      );
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
    void refreshPdfTranslation();
  });
</script>

<svelte:window onkeydown={onWindowKeydown} />

<div class="pdf-root" class:reader={variant === "reader"}>
  {#if loading}
    <p class="hint">Загрузка PDF…</p>
  {:else if err}
    <p class="error">{err}</p>
  {:else}
    <div class="pdf-main">
      {#if pdfTranslationPanel}
        <div class="canvas-wrap pdf-trans-wrap" bind:this={scrollHost}>
          {#if pdfSideErr}
            <p class="pdf-trans-err">{pdfSideErr}</p>
          {:else}
            <article class="pdf-trans-article" aria-label="Перевод страницы {pageNum}">
              <p class="pdf-trans-meta">
                Страница {pageNum} из {numPages}
              </p>
              <div class="pdf-trans-body" bind:this={transBodyEl}>{pdfSideText}</div>
            </article>
          {/if}
        </div>
      {:else}
        <div class="canvas-wrap" bind:this={scrollHost}>
          <div class="page-stack" bind:this={pageStackEl} style={pageStackStyle}>
            <canvas bind:this={canvas} class="page"></canvas>
            <div class="textLayer" bind:this={textLayerEl}></div>
            <div class="link-layer" bind:this={linkLayerEl}></div>
          </div>
        </div>
      {/if}
      <footer class="toolbar">
        {#if variant === "full"}
          <button type="button" onclick={prev} disabled={pageNum <= 1}>←</button>
          <span class="pages">{pageNum} / {numPages}</span>
          <button type="button" onclick={next} disabled={pageNum >= numPages}>→</button>
        {:else}
          <span class="pages soft">{pageNum} / {numPages}</span>
        {/if}
        {#if !pdfTranslationPanel}
          <label class="zoom">
            <span class="zoom-label">{zoomPct}%</span>
            <input type="range" min="0.5" max="2.5" step="0.05" bind:value={scale} />
          </label>
        {/if}
      </footer>
    </div>
  {/if}
</div>

{#if interactionsEnabled}
  <ReaderSelectionToolbar
    visible={toolbarVisible}
    x={toolbarX}
    y={toolbarY}
    onCopy={doCopy}
    onComment={openComment}
    onQuote={openQuote}
  />

  <CommentDialog
    open={commentOpen}
    excerpt={selectedText}
    pageHint={pageLabelStr}
    onClose={() => (commentOpen = false)}
    onSave={saveComment}
  />

  <QuoteDialog
    open={quoteOpen}
    quoteText={selectedText}
    bookTitle={displayTitle}
    bookAuthor={displayAuthor}
    pageLabel={pageLabelStr}
    chapterLabel={chapterLabel}
    onClose={() => (quoteOpen = false)}
    onSave={saveQuote}
  />
{/if}

<style>
  .pdf-root {
    display: flex;
    flex-direction: column;
    height: 100%;
    min-height: 0;
    background: var(--reader-bg);
  }
  .pdf-root.reader {
    background: var(--elevated-soft);
  }

  .pdf-main {
    flex: 1;
    display: flex;
    flex-direction: column;
    min-width: 0;
    min-height: 0;
  }

  .pdf-root.reader .canvas-wrap {
    padding: 0.6rem 0.85rem 0.85rem;
  }

  .pdf-trans-wrap {
    justify-content: flex-start;
    align-items: stretch;
  }

  .pdf-trans-article {
    width: 100%;
    max-width: 42rem;
    margin: 0 auto;
    padding: 0.35rem 0.5rem 1.25rem;
    box-sizing: border-box;
  }

  .pdf-trans-meta {
    margin: 0 0 1rem;
    font-size: 0.72rem;
    font-weight: 650;
    text-transform: uppercase;
    letter-spacing: 0.12em;
    color: var(--muted);
    font-family: system-ui, sans-serif;
  }

  .pdf-trans-body {
    font-size: 1.02rem;
    line-height: 1.68;
    color: var(--text-soft);
    white-space: pre-wrap;
    font-family: Literata, Georgia, "Times New Roman", serif;
    text-align: left;
    user-select: text;
    cursor: text;
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
  }
  .canvas-wrap {
    flex: 1;
    overflow: auto;
    display: flex;
    justify-content: center;
    padding: 1rem;
    min-height: 0;
  }
  .page-stack {
    position: relative;
    box-shadow: var(--shadow-book);
    background: #fff;
    border-radius: 2px;
  }
  .page-stack :global(.textLayer) {
    position: absolute;
    left: 0;
    top: 0;
    right: 0;
    bottom: 0;
    overflow: hidden;
    line-height: 1;
    opacity: 1;
    z-index: 2;
  }
  .page-stack :global(.textLayer span) {
    position: absolute;
    transform-origin: 0 0;
    white-space: pre;
    cursor: text;
    color: transparent;
  }
  /** Полный перевод: читаемый текст поверх белой подложки (растр страницы без изменений) */
  .page-stack :global(.textLayer span.pdf-inline-tr) {
    color: #141414;
    background: rgba(255, 255, 255, 0.92);
    box-decoration-break: clone;
    -webkit-box-decoration-break: clone;
    overflow: hidden;
    text-overflow: clip;
  }
  .page-stack :global(.textLayer br) {
    color: transparent;
  }
  canvas.page {
    display: block;
    position: relative;
    z-index: 1;
  }
  .link-layer {
    position: absolute;
    left: 0;
    top: 0;
    z-index: 3;
    pointer-events: none;
  }
  .link-layer :global(.pdf-link-box) {
    position: absolute;
    pointer-events: none;
  }
  .link-layer :global(.pdf-link-hit) {
    display: block;
    width: 100%;
    height: 100%;
    pointer-events: auto;
    border: none;
    padding: 0;
    background: transparent;
    cursor: pointer;
  }
  .link-layer :global(a.pdf-link-hit) {
    cursor: pointer;
  }
  .toolbar {
    display: flex;
    align-items: center;
    gap: 0.75rem;
    padding: 0.55rem 1rem;
    border-top: 1px solid var(--border-soft);
    background: var(--panel-soft);
    flex-shrink: 0;
  }
  .toolbar button {
    min-width: 2.5rem;
    padding: 0.35rem 0.6rem;
    border-radius: 10px;
    border: 1px solid var(--border-soft);
    background: var(--elevated-soft);
    color: var(--text);
    cursor: pointer;
  }
  .toolbar button:disabled {
    opacity: 0.35;
    cursor: default;
  }
  .pages {
    font-variant-numeric: tabular-nums;
    color: var(--muted);
    font-size: 0.88rem;
  }
  .pages.soft {
    font-weight: 500;
    color: var(--text-soft);
  }
  .zoom {
    margin-left: auto;
    display: flex;
    align-items: center;
    gap: 0.5rem;
    font-size: 0.82rem;
    color: var(--muted);
  }
  .zoom-label {
    min-width: 2.75rem;
    text-align: right;
    font-variant-numeric: tabular-nums;
  }
  .zoom input {
    width: 120px;
    accent-color: var(--accent);
  }
</style>
