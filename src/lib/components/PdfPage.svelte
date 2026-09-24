<script lang="ts">
  import { AnnotationType, TextLayer } from "pdfjs-dist";
  import type { PDFDocumentProxy, PDFPageProxy, PageViewport, RenderTask } from "pdfjs-dist";
  import { enqueueRender } from "$lib/pdf/renderQueue";
  import type { Highlight } from "$lib/types";
  import { locateText, rangeFromOffsets, unwrapAll, wrapRange } from "$lib/reading/textAnchor";

  export interface CropBox {
    x: number;
    y: number;
    w: number;
    h: number;
  }

  interface Props {
    doc: PDFDocumentProxy;
    n: number;
    scale: number;
    /** Размер страницы при масштабе 1 */
    base: { w: number; h: number };
    crop: CropBox;
    active: boolean;
    invert: boolean;
    highlights: Highlight[];
    /** Временная подсветка результата поиска */
    flash?: { start: number; len: number; text?: string } | null;
    inlineSpans?: string[] | null;
    inlineShow?: boolean;
    onLink?: (page: number) => void;
    onBase?: (n: number, w: number, h: number) => void;
    onHighlightClick?: (id: string, rect: DOMRect) => void;
  }
  let {
    doc,
    n,
    scale,
    base,
    crop,
    active,
    invert,
    highlights,
    flash = null,
    inlineSpans = null,
    inlineShow = true,
    onLink,
    onBase,
    onHighlightClick,
  }: Props = $props();

  /** Два холста: рисуем в скрытый, затем меняем местами — без мигания при зуме. */
  let canvasA = $state<HTMLCanvasElement | null>(null);
  let canvasB = $state<HTMLCanvasElement | null>(null);
  let front = $state<0 | 1>(0);
  let renderTask: RenderTask | null = null;
  let fontRetryDone = false;
  let textLayerEl = $state<HTMLDivElement | null>(null);
  let linkLayerEl = $state<HTMLDivElement | null>(null);
  let textLayer: InstanceType<typeof TextLayer> | null = null;
  let renderedKey = "";
  let token = 0;
  let textReady = $state(false);

  const fullW = $derived(base.w * scale);
  const fullH = $derived(base.h * scale);
  const boxW = $derived(fullW * crop.w);
  const boxH = $derived(fullH * crop.h);

  async function resolveDest(dest: unknown): Promise<number | null> {
    if (dest == null) return null;
    if (typeof dest === "string") return resolveDest(await doc.getDestination(dest));
    if (Array.isArray(dest) && dest.length > 0 && dest[0] && typeof dest[0] === "object") {
      try {
        return (await doc.getPageIndex(dest[0] as Parameters<PDFDocumentProxy["getPageIndex"]>[0])) + 1;
      } catch {
        return null;
      }
    }
    return null;
  }

  async function renderLinks(page: PDFPageProxy, viewport: PageViewport, layer: HTMLDivElement) {
    layer.innerHTML = "";
    const annotations = await page.getAnnotations({ intent: "display" });
    for (const ann of annotations) {
      if (ann.annotationType !== AnnotationType.LINK) continue;
      const rect = ann.rect;
      if (!rect || rect.length < 4) continue;
      const [x1, y1, x2, y2] = viewport.convertToViewportRectangle(rect);
      const box = document.createElement("div");
      box.className = "pdf-link-box";
      box.style.left = `${Math.min(x1, x2)}px`;
      box.style.top = `${Math.min(y1, y2)}px`;
      box.style.width = `${Math.abs(x2 - x1)}px`;
      box.style.height = `${Math.abs(y2 - y1)}px`;
      const url = ann.url || ann.unsafeUrl;
      if (typeof url === "string" && url.length > 0) {
        const a = document.createElement("a");
        a.href = url;
        a.target = "_blank";
        a.rel = "noopener noreferrer";
        a.className = "pdf-link-hit";
        box.appendChild(a);
      } else if (ann.dest != null) {
        const dest = ann.dest;
        const btn = document.createElement("button");
        btn.type = "button";
        btn.className = "pdf-link-hit";
        btn.setAttribute("aria-label", "Перейти по ссылке");
        btn.addEventListener("click", async (e) => {
          e.stopPropagation();
          const p = await resolveDest(dest);
          if (p != null) onLink?.(p);
        });
        box.appendChild(btn);
      } else continue;
      layer.appendChild(box);
    }
  }

  /** Растровый (CPU) контекст: GPU-холсты WebKitGTK на некоторых драйверах теряют глифы. */
  function ctxOf(c: HTMLCanvasElement): CanvasRenderingContext2D | null {
    return c.getContext("2d", { alpha: false, willReadFrequently: true });
  }

  function clearCanvas(c: HTMLCanvasElement | null) {
    if (!c) return;
    c.width = 0;
    c.height = 0;
  }

  function release() {
    token++;
    renderTask?.cancel();
    renderTask = null;
    textLayer?.cancel();
    textLayer = null;
    textReady = false;
    renderedKey = "";
    clearCanvas(canvasA);
    clearCanvas(canvasB);
    if (textLayerEl) textLayerEl.innerHTML = "";
    if (linkLayerEl) linkLayerEl.innerHTML = "";
  }

  /** Плотность пикселей с ограничением размера холста (лимиты памяти/текстур). */
  function pixelRatio(viewport: PageViewport): number {
    const want = Math.min(window.devicePixelRatio || 1, 3);
    const MAX_PIXELS = 16_000_000;
    const area = viewport.width * viewport.height;
    return Math.max(0.5, Math.min(want, Math.sqrt(MAX_PIXELS / Math.max(1, area))));
  }

  async function render() {
    if (!canvasA || !canvasB || !textLayerEl || !linkLayerEl) return;
    const key = `${scale.toFixed(4)}|${inlineShow}|${inlineSpans?.length ?? 0}`;
    if (key === renderedKey) return;
    const my = ++token;
    const page = await doc.getPage(n);
    if (my !== token) return;
    const vp1 = page.getViewport({ scale: 1 });
    if (Math.abs(vp1.width - base.w) > 0.5 || Math.abs(vp1.height - base.h) > 0.5) {
      onBase?.(n, vp1.width, vp1.height);
    }
    const viewport = page.getViewport({ scale });
    const fontsWereLoading = document.fonts?.status === "loading";

    // Холст должен быть в документе (а не отдельным offscreen): так шрифты
    // PDF гарантированно доступны при рисовании текста.
    const back = front === 0 ? canvasB : canvasA;
    const shown = front === 0 ? canvasA : canvasB;
    const ok = await enqueueRender(doc, async () => {
      if (my !== token) return false;
      const dpr = pixelRatio(viewport);
      back.width = Math.floor(viewport.width * dpr);
      back.height = Math.floor(viewport.height * dpr);
      const ctx = ctxOf(back);
      if (!ctx) return false;
      renderTask = page.render({
        canvas: back,
        canvasContext: ctx,
        viewport,
        transform: dpr !== 1 ? [dpr, 0, 0, dpr, 0, 0] : undefined,
        background: "#ffffff",
        intent: "any",
        optionalContentConfigPromise: doc.getOptionalContentConfig({ intent: "any" }),
      });
      try {
        await renderTask.promise;
        return true;
      } catch {
        return false;
      } finally {
        renderTask = null;
      }
    });
    if (!ok || my !== token) return;
    front = front === 0 ? 1 : 0;
    clearCanvas(shown);

    textLayer?.cancel();
    textLayerEl.innerHTML = "";
    textLayer = new TextLayer({
      textContentSource: page.streamTextContent(),
      container: textLayerEl,
      viewport,
    });
    try {
      await textLayer.render();
    } catch {
      return;
    }
    if (my !== token) return;

    const inline = inlineSpans;
    if (inline && textLayer && inline.length === textLayer.textContentItemsStr.length) {
      const divs = textLayer.textDivs;
      for (let i = 0; i < inline.length; i++) {
        const el = divs[i];
        if (!el) continue;
        el.textContent = inline[i] ?? "";
        el.classList.toggle("pdf-inline-tr", inlineShow);
      }
      textLayer.update({ viewport });
    }

    await renderLinks(page, viewport, linkLayerEl);
    if (my !== token) return;
    renderedKey = key;
    textReady = true;

    // Если шрифты ещё грузились — перерисуем один раз, когда загрузятся.
    if (fontsWereLoading && !fontRetryDone && document.fonts) {
      fontRetryDone = true;
      void document.fonts.ready.then(() => {
        if (my !== token || !active) return;
        renderedKey = "";
        void render();
      });
    }
  }

  $effect(() => {
    scale;
    inlineSpans;
    inlineShow;
    if (!active) {
      release();
      return;
    }
    void render();
  });

  $effect(() => () => release());

  function paintHighlights() {
    const root = textLayerEl;
    if (!root) return;
    unwrapAll(root, "mark.rd-hl");
    for (const h of highlights) {
      const loc = locateText(root, h.text, h.offset ?? 0);
      if (!loc) continue;
      const range = rangeFromOffsets(root, loc.start, loc.end);
      if (!range) continue;
      wrapRange(range, () => {
        const m = document.createElement("mark");
        m.className = `rd-hl rd-hl-${h.color}${h.note ? " rd-hl-note" : ""}`;
        m.dataset.id = h.id;
        return m;
      });
    }
  }

  $effect(() => {
    highlights;
    if (textReady) paintHighlights();
  });

  $effect(() => {
    const f = flash;
    const root = textLayerEl;
    if (!f || !textReady || !root) return;
    // Смещения текстового слоя и getTextContent иногда расходятся на пару
    // символов — ищем сам фрагмент рядом с подсказкой.
    const loc = f.text ? locateText(root, f.text, f.start) : null;
    const range = loc ? rangeFromOffsets(root, loc.start, loc.end) : rangeFromOffsets(root, f.start, f.start + f.len);
    if (!range) return;
    const marks = wrapRange(range, () => {
      const m = document.createElement("mark");
      m.className = "rd-search-hit";
      return m;
    });
    marks[0]?.scrollIntoView({ block: "center", behavior: "smooth" });
    const t = setTimeout(() => unwrapAll(root, "mark.rd-search-hit"), 2600);
    return () => clearTimeout(t);
  });

  function onClick(e: MouseEvent) {
    const mark = (e.target as Element).closest("mark.rd-hl") as HTMLElement | null;
    if (mark?.dataset.id && document.getSelection()?.isCollapsed !== false) {
      e.stopPropagation();
      onHighlightClick?.(mark.dataset.id, mark.getBoundingClientRect());
    }
  }
</script>

<!-- svelte-ignore a11y_click_events_have_key_events -->
<!-- svelte-ignore a11y_no_static_element_interactions -->
<div
  class="pdf-page"
  data-page={n}
  style:width="{boxW}px"
  style:height="{boxH}px"
  onclick={onClick}
>
  <div
    class="pdf-page-inner"
    style:width="{fullW}px"
    style:height="{fullH}px"
    style:transform="translate({-crop.x * fullW}px, {-crop.y * fullH}px)"
    style:--scale-factor={scale}
  >
    <canvas
      bind:this={canvasA}
      class="pdf-canvas"
      class:inverted={invert}
      class:hidden={front !== 0}
      style:width="{fullW}px"
      style:height="{fullH}px"
    ></canvas>
    <canvas
      bind:this={canvasB}
      class="pdf-canvas"
      class:inverted={invert}
      class:hidden={front !== 1}
      style:width="{fullW}px"
      style:height="{fullH}px"
    ></canvas>
    <div class="textLayer" bind:this={textLayerEl}></div>
    <div class="pdf-link-layer" bind:this={linkLayerEl}></div>
  </div>
  {#if !textReady && active}
    <span class="pdf-page-num" aria-hidden="true">{n}</span>
  {/if}
</div>

<style>
  .pdf-page {
    position: relative;
    overflow: hidden;
    flex-shrink: 0;
    background: #fff;
    box-shadow: var(--shadow-book);
    border-radius: 3px;
  }

  .pdf-page-inner {
    position: absolute;
    left: 0;
    top: 0;
  }

  .pdf-canvas {
    display: block;
    position: absolute;
    inset: 0;
  }

  .pdf-canvas.hidden {
    visibility: hidden;
  }

  .pdf-canvas.inverted {
    filter: invert(0.9) hue-rotate(180deg) contrast(0.92);
  }

  .pdf-page-num {
    position: absolute;
    inset: 0;
    display: grid;
    place-items: center;
    color: #b8b3c4;
    font-size: 1.4rem;
    font-variant-numeric: tabular-nums;
  }

  .pdf-page :global(.textLayer) {
    position: absolute;
    inset: 0;
    overflow: hidden;
    line-height: 1;
    opacity: 1;
    z-index: 2;
  }
  .pdf-page :global(.textLayer span),
  .pdf-page :global(.textLayer br) {
    position: absolute;
    transform-origin: 0 0;
    white-space: pre;
    cursor: text;
    color: transparent;
  }
  .pdf-page :global(.textLayer mark) {
    position: static;
    color: transparent;
    border-radius: 2px;
  }
  /* Глобальный ::selection задаёт цвет текста; для прозрачного слоя это
     проявляло бы вторую копию слова поверх страницы. */
  .pdf-page :global(.textLayer :not(.pdf-inline-tr)::selection) {
    color: transparent;
    background: color-mix(in srgb, var(--accent) 34%, transparent);
  }
  .pdf-page :global(.textLayer span.pdf-inline-tr) {
    color: #141414;
    background: rgba(255, 255, 255, 0.92);
    overflow: hidden;
  }
  .pdf-page :global(mark.rd-hl) {
    cursor: pointer;
    background: color-mix(in srgb, var(--hl) 48%, transparent);
    mix-blend-mode: multiply;
  }
  .pdf-page :global(mark.rd-hl-yellow) {
    --hl: #f5d565;
  }
  .pdf-page :global(mark.rd-hl-green) {
    --hl: #94d49a;
  }
  .pdf-page :global(mark.rd-hl-blue) {
    --hl: #8ec5ee;
  }
  .pdf-page :global(mark.rd-hl-pink) {
    --hl: #f2a3c0;
  }
  .pdf-page :global(mark.rd-hl-violet) {
    --hl: #c3a8f0;
  }
  .pdf-page :global(mark.rd-hl-note) {
    border-bottom: 2px dotted rgba(40, 30, 60, 0.6);
  }
  .pdf-page :global(mark.rd-search-hit) {
    background: color-mix(in srgb, var(--accent) 55%, transparent);
    mix-blend-mode: multiply;
  }

  .pdf-link-layer {
    position: absolute;
    inset: 0;
    z-index: 3;
    pointer-events: none;
  }
  .pdf-link-layer :global(.pdf-link-box) {
    position: absolute;
    pointer-events: none;
  }
  .pdf-link-layer :global(.pdf-link-hit) {
    display: block;
    width: 100%;
    height: 100%;
    pointer-events: auto;
    border: none;
    padding: 0;
    background: transparent;
    cursor: pointer;
  }
</style>
