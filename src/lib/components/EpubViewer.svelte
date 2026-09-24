<script lang="ts">
  import { untrack } from "svelte";
  import type {
    EpubReaderApi,
    Highlight,
    ReaderSelection,
    ReadingPosition,
    ReadingUnit,
    SearchHit,
    TextChunk,
  } from "$lib/types";
  import { extractTextNodesHtml } from "$lib/translate/htmlText";
  import { translateStringList } from "$lib/translate/translateApi";
  import { readLibraryBookBytes } from "$lib/library/readLibraryBookBytes";
  import { fontFaceCss } from "$lib/reading/fonts";
  import { epubContentCss } from "$lib/reading/contentCss";
  import type { PagePalette } from "$lib/reading/palette";
  import { reading } from "$lib/reading/settings.svelte";
  import { highlightFill } from "$lib/reading/highlights";
  import { storeRead, storeWrite, pathKey } from "$lib/storage/store";

  interface Props {
    relativePath: string;
    /** CFI или href главы */
    initialLocation?: string;
    palette: PagePalette;
    highlights?: Highlight[];
    onReadingProgress?: (p: { location: string; label: string; href: string; progress: number | null }) => void;
    onPosition?: (p: ReadingPosition) => void;
    onReaderApi?: (api: EpubReaderApi | null) => void;
    onSelection?: (s: ReaderSelection | null) => void;
    onHighlightClick?: (id: string, rect: DOMRect) => void;
    onCenterTap?: () => void;
    translateRunKey?: number;
    translateSource?: string;
    translateTarget?: string;
    translateLayout?: "orig" | "trans" | "split";
    onTranslateActivity?: (p: { busy: boolean; error: string | null }) => void;
  }
  let {
    relativePath,
    initialLocation = "",
    palette,
    highlights = [],
    onReadingProgress,
    onPosition,
    onReaderApi,
    onSelection,
    onHighlightClick,
    onCenterTap,
    translateRunKey = 0,
    translateSource = "auto",
    translateTarget = "ru",
    translateLayout = "orig",
    onTranslateActivity,
  }: Props = $props();

  /** Размер «локации» epub.js в символах — единица прогресса и оценки времени. */
  const LOC_CHARS = 1024;

  /** Оригинал и кэш перевода по spine-href */
  const epubOrigHtml = new Map<string, string>();
  const epubTransHtml = new Map<string, string>();

  let host = $state<HTMLDivElement | null>(null);
  let loading = $state(true);
  let err = $state<string | null>(null);
  let ready = $state(false);

  let rendition: any = null;
  let book: any = null;
  let session = 0;

  let currentHref = $state("");
  let lastMarkClick = 0;
  let currentCfi = "";
  let lastLoc: any = null;
  let locationsReady = $state(false);
  /** Индекс spine для каждой локации (для «до конца главы») */
  let locSpine: number[] = [];
  let tocSpine: { label: string; spine: number }[] = [];
  let toc: { label: string; href: string; level: number }[] = [];


  const flow = $derived(reading.s.epubFlow);

  function flattenToc(items: any[], depth = 0): { label: string; href: string; level: number }[] {
    const r: { label: string; href: string; level: number }[] = [];
    for (const it of items || []) {
      r.push({ label: (it.label || "Без названия").trim(), href: it.href, level: depth });
      if (it.subitems?.length) r.push(...flattenToc(it.subitems, depth + 1));
    }
    return r;
  }

  function spineIndexOfHref(href: string): number {
    if (!book) return -1;
    try {
      const item = book.spine.get(href.split("#")[0]) ?? book.spine.get(href);
      return typeof item?.index === "number" ? item.index : -1;
    } catch {
      return -1;
    }
  }

  function chapterForSpine(spine: number): { label: string; nextSpine: number } {
    let label = "";
    let bestSpine = -1;
    let nextSpine = book?.spine?.spineItems?.length ?? spine + 1;
    for (const t of tocSpine) {
      if (t.spine <= spine && t.spine > bestSpine) {
        bestSpine = t.spine;
        label = t.label;
      }
      if (t.spine > spine && t.spine < nextSpine) nextSpine = t.spine;
    }
    return { label, nextSpine };
  }

  function spineOfCfi(cfi: string): number {
    const m = /^epubcfi\(\/6\/(\d+)/.exec(cfi);
    return m ? Number(m[1]) / 2 - 1 : -1;
  }

  function emitPosition() {
    const loc = lastLoc;
    if (!loc || !book) return;
    const cfi = loc.start?.cfi ?? "";
    const spine = loc.start?.index ?? spineIndexOfHref(loc.start?.href ?? "");
    const { label, nextSpine } = chapterForSpine(spine);
    const total = book.spine?.spineItems?.length || 1;
    let progress: number | null = null;
    let charsLeftChapter: number | null = null;
    let charsLeftBook: number | null = null;
    if (locationsReady && book.locations.length() > 0) {
      const endCfi = loc.end?.cfi || cfi;
      progress = loc.atEnd ? 1 : book.locations.percentageFromCfi(endCfi);
      const cur = book.locations.locationFromCfi(cfi);
      const totalLocs = book.locations.length();
      let chapterEnd = locSpine.findIndex((s) => s >= nextSpine);
      if (chapterEnd < 0) chapterEnd = totalLocs;
      if (typeof cur === "number" && cur >= 0) {
        charsLeftChapter = Math.max(0, chapterEnd - cur - 1) * LOC_CHARS;
        charsLeftBook = Math.max(0, totalLocs - cur - 1) * LOC_CHARS;
      }
    } else {
      const disp = loc.start?.displayed;
      const within = disp?.total ? (disp.page - 1) / disp.total : 0;
      progress = Math.min(1, (spine + within) / total);
    }
    const disp = loc.start?.displayed;
    onPosition?.({
      progress,
      chapterLabel: label,
      charsLeftChapter,
      charsLeftBook,
      pageLabel: disp?.total ? `стр. ${disp.page} из ${disp.total} в главе` : undefined,
    });
    if (cfi) {
      onReadingProgress?.({
        location: cfi,
        label: label || "",
        href: loc.start?.href ?? "",
        progress,
      });
    }
  }

  async function prepareLocations(path: string, sid: number) {
    const key = `epubloc-${await pathKey(path)}`;
    try {
      const saved = await storeRead<string | null>(key, null);
      if (sid !== session || !book) return;
      if (saved) {
        book.locations.load(saved);
      } else {
        await book.locations.generate(LOC_CHARS);
        if (sid !== session || !book) return;
        void storeWrite(key, book.locations.save()).catch(() => {});
      }
      locSpine = (book.locations._locations as string[]).map(spineOfCfi);
      locationsReady = true;
      emitPosition();
    } catch {
      /* прогресс останется приблизительным */
    }
  }

  function styleText(): string {
    return fontFaceCss() + epubContentCss(reading.s, palette);
  }

  function injectStyle(doc: Document) {
    let el = doc.getElementById("rd-style") as HTMLStyleElement | null;
    if (!el) {
      el = doc.createElement("style");
      el.id = "rd-style";
      doc.head?.appendChild(el);
    }
    el.textContent = styleText();
  }

  function frameRect(win: Window): DOMRect | null {
    const iframeEl = win.frameElement as HTMLIFrameElement | null;
    return iframeEl?.getBoundingClientRect() ?? null;
  }

  function attachInteractions(contents: any) {
    const doc = contents.document as Document;
    const win = contents.window as Window;

    const refreshSelection = () => {
      requestAnimationFrame(() => {
        const sel = win.getSelection();
        if (!sel || sel.isCollapsed || sel.rangeCount === 0) {
          onSelection?.(null);
          return;
        }
        const text = sel.toString().trim();
        if (!text) {
          onSelection?.(null);
          return;
        }
        const range = sel.getRangeAt(0);
        const rect = range.getBoundingClientRect();
        const fr = frameRect(win);
        if (!fr) return;
        let cfi = "";
        try {
          cfi = contents.cfiFromRange(range);
        } catch {
          cfi = "";
        }
        onSelection?.({
          text,
          rect: { left: fr.left + rect.left, top: fr.top + rect.top, width: rect.width, height: rect.height },
          anchor: { cfi, chapterLabel: chapterForSpine(spineIndexOfHref(currentHref)).label || undefined },
          clear: () => win.getSelection()?.removeAllRanges(),
        });
      });
    };
    doc.addEventListener("mouseup", refreshSelection);
    doc.addEventListener("keyup", refreshSelection);
    doc.addEventListener("touchend", () => setTimeout(refreshSelection, 60));

    // Тап по краям — листание, по центру — показать/скрыть интерфейс.
    doc.addEventListener("click", (e: MouseEvent) => {
      const sel = win.getSelection();
      if (sel && !sel.isCollapsed) return;
      if ((e.target as Element | null)?.closest?.("a")) return;
      const fr = frameRect(win);
      const hr = host?.getBoundingClientRect();
      if (!fr || !hr) return;
      const x = (fr.left + e.clientX - hr.left) / hr.width;
      // Клик по выделению приходит сюда же (marks-pane проксирует события) —
      // ждём мгновение, чтобы не листать и не прятать интерфейс вместо попапа.
      setTimeout(() => {
        if (Date.now() - lastMarkClick < 400) return;
        if (reading.s.tapZones && x < 0.28) void prev();
        else if (reading.s.tapZones && x > 0.72) void next();
        else onCenterTap?.();
      }, 40);
    });

    let touchX = 0;
    let touchY = 0;
    doc.addEventListener(
      "touchstart",
      (e: TouchEvent) => {
        touchX = e.touches[0]?.clientX ?? 0;
        touchY = e.touches[0]?.clientY ?? 0;
      },
      { passive: true },
    );
    doc.addEventListener(
      "touchend",
      (e: TouchEvent) => {
        if (flow !== "paginated") return;
        const t = e.changedTouches[0];
        if (!t) return;
        const dx = t.clientX - touchX;
        const dy = t.clientY - touchY;
        if (Math.abs(dx) > 50 && Math.abs(dy) < 60) {
          if (dx < 0) void next();
          else void prev();
        }
      },
      { passive: true },
    );

    // Клавиши внутри iframe передаём окну: там живут общие сочетания.
    doc.addEventListener("keydown", (e: KeyboardEvent) => {
      const clone = new KeyboardEvent("keydown", {
        key: e.key,
        code: e.code,
        ctrlKey: e.ctrlKey,
        metaKey: e.metaKey,
        shiftKey: e.shiftKey,
        altKey: e.altKey,
        cancelable: true,
      });
      window.dispatchEvent(clone);
      if (clone.defaultPrevented) e.preventDefault();
    });

    // Режим фокуса: абзац под курсором остаётся ярким.
    doc.addEventListener("mousemove", (e: MouseEvent) => {
      if (reading.s.focus !== "paragraph") return;
      const block = (e.target as Element | null)?.closest?.("p, li, h1, h2, h3, h4, blockquote");
      if (!block) return;
      for (const el of Array.from(doc.querySelectorAll(".rd-focus-current"))) {
        if (el !== block) el.classList.remove("rd-focus-current");
      }
      block.classList.add("rd-focus-current");
    });
    applyFocusClass(doc);
  }

  function applyFocusClass(doc: Document) {
    doc.documentElement.classList.toggle("rd-focus-dim", reading.s.focus === "paragraph");
  }

  async function applyEpubTranslation(contents: any) {
    const href = String(contents?.section?.href || currentHref || "");
    if (!href) return;
    const doc = contents.document as Document;
    if (!doc?.body) return;

    if (translateLayout === "orig") {
      const o = epubOrigHtml.get(href);
      if (o) doc.body.innerHTML = o;
      return;
    }

    if (!epubOrigHtml.has(href)) {
      epubOrigHtml.set(href, doc.body.innerHTML);
    }

    const cached = epubTransHtml.get(href);
    if (cached) {
      doc.body.innerHTML = cached;
      return;
    }

    const srcHtml = epubOrigHtml.get(href) ?? doc.body.innerHTML;
    const ex = extractTextNodesHtml(srcHtml);
    if (ex.texts.length === 0) return;

    onTranslateActivity?.({ busy: true, error: null });
    try {
      const tr = await translateStringList(ex.texts, translateSource, translateTarget);
      const applied = ex.apply(tr);
      epubTransHtml.set(href, applied);
      doc.body.innerHTML = applied;
      onTranslateActivity?.({ busy: false, error: null });
    } catch (e) {
      onTranslateActivity?.({
        busy: false,
        error: e instanceof Error ? e.message : String(e),
      });
    }
  }

  function contentHook(contents: any) {
    injectStyle(contents.document);
    attachInteractions(contents);
    void applyEpubTranslation(contents);
  }

  function blocksOf(doc: Document): HTMLElement[] {
    const all = Array.from(doc.body.querySelectorAll<HTMLElement>("p, li, h1, h2, h3, h4, h5, blockquote, pre"));
    // Только «листья»: без вложенных блоков из этого же списка.
    return all.filter((el) => !el.querySelector("p, li, h1, h2, h3, h4, h5, blockquote, pre") && el.textContent?.trim());
  }

  function currentContents(): any | null {
    const list = rendition?.getContents?.() ?? [];
    if (!list.length) return null;
    const idx = lastLoc?.start?.index;
    return list.find((c: any) => c.sectionIndex === idx) ?? list[0];
  }

  function isVisible(el: HTMLElement): boolean {
    const win = el.ownerDocument.defaultView;
    const fr = win ? frameRect(win) : null;
    const hr = host?.getBoundingClientRect();
    if (!fr || !hr) return true;
    const r = el.getBoundingClientRect();
    const left = fr.left + r.left;
    const top = fr.top + r.top;
    return left >= hr.left - 4 && left < hr.right - 4 && top < hr.bottom && top + r.height > hr.top;
  }

  async function unitsFromHere(): Promise<ReadingUnit[]> {
    const contents = currentContents();
    if (!contents) return [];
    const doc = contents.document as Document;
    const blocks = blocksOf(doc);
    let start = blocks.findIndex((b) => isVisible(b));
    if (start < 0) start = 0;
    return blocks.slice(start).map((el) => ({
      text: (el.textContent ?? "").replace(/\s+/g, " ").trim(),
      mark: () => el.classList.add("rd-tts-current"),
      unmark: () => el.classList.remove("rd-tts-current"),
      reveal: async () => {
        if (isVisible(el)) return;
        try {
          await rendition?.display(contents.cfiFromNode(el));
        } catch {
          /* ignore */
        }
      },
    }));
  }

  async function advanceChapter(): Promise<boolean> {
    const idx = lastLoc?.start?.index;
    const items = book?.spine?.spineItems ?? [];
    if (typeof idx !== "number" || idx + 1 >= items.length) return false;
    await rendition?.display(items[idx + 1].href);
    return true;
  }

  async function sectionText(item: any): Promise<string> {
    try {
      await item.load(book.load.bind(book));
      const text = (item.document?.body?.textContent ?? "").replace(/\s+/g, " ").trim();
      item.unload();
      return text;
    } catch {
      return "";
    }
  }

  async function chunksBefore(): Promise<TextChunk[]> {
    const idx = lastLoc?.start?.index;
    if (typeof idx !== "number" || !book) return [];
    const out: TextChunk[] = [];
    const items = book.spine.spineItems as any[];
    for (let i = 0; i < idx; i++) {
      const text = await sectionText(items[i]);
      if (text.length < 40) continue;
      out.push({ id: `spine-${i}`, label: chapterForSpine(i).label || `Часть ${i + 1}`, text, partial: false });
    }
    const contents = currentContents();
    if (contents && lastLoc?.end?.cfi) {
      try {
        const doc = contents.document as Document;
        const end = contents.range(lastLoc.end.cfi) as Range;
        const r = doc.createRange();
        r.setStart(doc.body, 0);
        r.setEnd(end.endContainer, end.endOffset);
        const text = r.toString().replace(/\s+/g, " ").trim();
        if (text) out.push({ id: `spine-${idx}-partial`, label: chapterForSpine(idx).label || "Текущая глава", text, partial: true });
      } catch {
        /* ignore */
      }
    }
    return out;
  }

  async function search(query: string, signal?: AbortSignal): Promise<SearchHit[]> {
    const hits: SearchHit[] = [];
    const q = query.trim();
    if (!q || !book) return hits;
    const lower = q.toLocaleLowerCase();
    for (const item of book.spine.spineItems as any[]) {
      if (signal?.aborted) break;
      try {
        await item.load(book.load.bind(book));
        const found = (item.find(q) ?? []) as { cfi: string; excerpt: string }[];
        item.unload();
        const label = chapterForSpine(item.index).label || `Часть ${item.index + 1}`;
        for (const f of found) {
          const ex = f.excerpt ?? "";
          const i = ex.toLocaleLowerCase().indexOf(lower);
          hits.push({
            id: f.cfi,
            label,
            before: i >= 0 ? "…" + ex.slice(0, i) : ex,
            match: i >= 0 ? ex.slice(i, i + q.length) : "",
            after: i >= 0 ? ex.slice(i + q.length) + "…" : "",
            loc: f.cfi,
          });
        }
      } catch {
        /* пропускаем проблемную главу */
      }
      if (hits.length >= 300) break;
      await new Promise((r) => setTimeout(r, 0));
    }
    return hits;
  }

  async function goToHit(hit: SearchHit) {
    if (!rendition) return;
    await rendition.display(hit.loc);
    try {
      rendition.annotations.highlight(hit.loc, {}, undefined, "rd-search", {
        fill: palette.accent,
        "fill-opacity": "0.4",
      });
      setTimeout(() => {
        try {
          rendition?.annotations.remove(hit.loc, "highlight");
        } catch {
          /* ignore */
        }
      }, 2600);
    } catch {
      /* ignore */
    }
  }

  async function seek(fraction: number) {
    if (!rendition || !book) return;
    const f = Math.min(1, Math.max(0, fraction));
    if (locationsReady) {
      const cfi = book.locations.cfiFromPercentage(f);
      if (cfi) {
        await rendition.display(cfi);
        return;
      }
    }
    const items = book.spine.spineItems as any[];
    const item = items[Math.min(items.length - 1, Math.floor(f * items.length))];
    if (item) await rendition.display(item.href);
  }

  async function mountBook(path: string, el: HTMLDivElement, sid: number, openAt: string, mode: string) {
    loading = true;
    err = null;
    ready = false;
    locationsReady = false;
    currentHref = "";
    lastLoc = null;
    epubOrigHtml.clear();
    epubTransHtml.clear();
    onReaderApi?.(null);
    rendition?.destroy();
    book?.destroy();
    rendition = null;
    book = null;
    try {
      const bytes = await readLibraryBookBytes(path);
      if (sid !== session) return;
      const ePub = (await import("epubjs")).default;
      book = ePub(bytes.buffer);
      await book.ready;
      if (sid !== session) return;
      rendition = book.renderTo(el, {
        width: "100%",
        height: "100%",
        spread: "none",
        ...(mode === "scrolled" ? { flow: "scrolled", manager: "continuous" } : { flow: "paginated" }),
      });
      rendition.hooks.content.register(contentHook);
      if (import.meta.env.DEV) (window as any).__rendition = rendition;

      try {
        await book.loaded.navigation;
        toc = flattenToc(book.navigation?.toc || []);
      } catch {
        toc = [];
      }
      tocSpine = toc
        .map((t) => ({ label: t.label, spine: spineIndexOfHref(t.href) }))
        .filter((t) => t.spine >= 0);

      rendition.on("rendered", (_section: any, view: any) => paintView(view));

      rendition.on("relocated", (loc: any) => {
        lastLoc = loc;
        const href = loc?.start?.href || "";
        if (href) currentHref = href;
        currentCfi = loc?.start?.cfi ?? currentCfi;
        emitPosition();
      });

      let opened = false;
      if (openAt.trim()) {
        try {
          await rendition.display(openAt.trim());
          opened = true;
        } catch {
          opened = false;
        }
      }
      if (!opened) await rendition.display();
      if (sid !== session) return;

      const spine = (book.spine.spineItems as any[]).map((s) => ({
        label: chapterForSpine(s.index).label || s.href.split("/").pop() || s.href,
        href: s.href,
      }));

      const api: EpubReaderApi = {
        toc,
        spine,
        goTo: async (href: string) => {
          await rendition?.display(href);
        },
        prev,
        next,
        seek,
        search,
        goToHit,
        unitsFromHere,
        advanceChapter,
        chunksBefore,
      };
      onReaderApi?.(api);
      ready = true;
      redrawHighlights(true);
      void prepareLocations(path, sid);
    } catch (e) {
      if (sid === session) err = String(e);
      onReaderApi?.(null);
    } finally {
      if (sid === session) loading = false;
    }
  }

  let mountedPath = "";

  $effect(() => {
    const path = relativePath;
    const el = host;
    const mode = flow;
    if (!el) return;
    // Новая книга открывается с сохранённой позиции; при смене режима
    // листания продолжаем с того же места.
    if (path !== mountedPath) {
      currentCfi = "";
      mountedPath = path;
    }
    const openAt = currentCfi || untrack(() => initialLocation) || "";
    const sid = ++session;
    void mountBook(path, el, sid, openAt, mode);
    return () => {
      session += 1;
      onReaderApi?.(null);
      rendition?.destroy();
      book?.destroy();
      rendition = null;
      book = null;
    };
  });

  /** Типографика и цвета: обновляем стиль во всех открытых главах. */
  let restyleTimer: ReturnType<typeof setTimeout> | null = null;
  $effect(() => {
    const css = styleText();
    const focus = reading.s.focus;
    if (!ready || !rendition) return;
    for (const c of rendition.getContents?.() ?? []) {
      const el = c.document?.getElementById("rd-style");
      if (el) el.textContent = css;
      c.document?.documentElement.classList.toggle("rd-focus-dim", focus === "paragraph");
    }
    if (restyleTimer) clearTimeout(restyleTimer);
    restyleTimer = setTimeout(() => {
      const cfi = currentCfi;
      try {
        rendition?.resize();
        if (cfi) void rendition?.display(cfi);
      } catch {
        /* ignore */
      }
      redrawHighlights(true);
    }, 180);
  });

  /** Размер области меняется (панели, ширина строки) — перераскладываем колонки. */
  $effect(() => {
    const el = host;
    if (!el) return;
    let t: ReturnType<typeof setTimeout> | null = null;
    const ro = new ResizeObserver(() => {
      if (t) clearTimeout(t);
      t = setTimeout(() => {
        if (!ready) return;
        try {
          rendition?.resize();
          if (currentCfi) void rendition?.display(currentCfi);
        } catch {
          /* ignore */
        }
      }, 140);
    });
    ro.observe(el);
    return () => {
      ro.disconnect();
      if (t) clearTimeout(t);
    };
  });

  /**
   * Выделения рисуем прямо в представлениях глав (а не через rendition.annotations):
   * так они надёжно переживают перераскладку и смену темы.
   */
  function paintView(view: any) {
    if (!view?.highlight) return;
    const drawnHere: Map<string, string> = (view.__rdDrawn ??= new Map());
    const styleKey = palette.dark ? "d" : "l";
    const wanted = new Map<string, Highlight>();
    for (const h of highlights) {
      if (h.cfi && spineOfCfi(h.cfi) === view.index) wanted.set(h.id, h);
    }
    for (const [id, key] of drawnHere) {
      const h = wanted.get(id);
      if (!h || key !== `${h.cfi}|${h.color}|${styleKey}`) {
        try {
          view.unhighlight(key.split("|")[0]);
        } catch {
          /* ignore */
        }
        drawnHere.delete(id);
      }
    }
    for (const [id, h] of wanted) {
      if (drawnHere.has(id)) continue;
      try {
        view.highlight(
          h.cfi,
          { id },
          (e: MouseEvent) => {
            lastMarkClick = Date.now();
            const target = e?.target as Element | null;
            const r = target?.getBoundingClientRect?.() ?? new DOMRect(e?.clientX ?? 0, e?.clientY ?? 0, 1, 1);
            onHighlightClick?.(id, r);
          },
          "rd-hl",
          {
            fill: highlightFill(h.color),
            "fill-opacity": palette.dark ? "0.38" : "0.42",
            "mix-blend-mode": palette.dark ? "screen" : "multiply",
          },
        );
        drawnHere.set(id, `${h.cfi}|${h.color}|${styleKey}`);
      } catch {
        /* CFI мог устареть после правки книги */
      }
    }
  }

  function currentViews(): any[] {
    const views = rendition?.views?.();
    return views?.all?.() ?? views?._views ?? [];
  }

  function redrawHighlights(force = false) {
    if (!rendition || !ready) return;
    for (const v of currentViews()) {
      if (force) {
        for (const key of (v.__rdDrawn as Map<string, string> | undefined)?.values() ?? []) {
          try {
            v.unhighlight(key.split("|")[0]);
          } catch {
            /* ignore */
          }
        }
        v.__rdDrawn?.clear();
      }
      paintView(v);
    }
  }

  $effect(() => {
    highlights;
    palette.dark;
    if (ready) untrack(() => redrawHighlights());
  });

  /** Повторный перевод текущей главы после нажатия «Перевести» */
  $effect(() => {
    translateRunKey;
    if (currentHref) epubTransHtml.delete(currentHref);
  });

  /** Смена режима оригинал/перевод — перезагрузить текущий экран */
  $effect(() => {
    translateLayout;
    translateRunKey;
    untrack(() => {
      const r = rendition;
      const href = currentHref;
      if (!r || !href || !ready) return;
      void r.display(currentCfi || href);
    });
  });

  async function prev() {
    try {
      await rendition?.prev?.();
    } catch {
      /* ignore */
    }
  }
  async function next() {
    try {
      await rendition?.next?.();
    } catch {
      /* ignore */
    }
  }

  const stageStyle = $derived(
    `max-width:calc(${reading.s.measure * 0.52}em + ${reading.s.margin * 2}px);` +
      `font-size:${reading.s.fontSize}px;padding:0 ${reading.s.margin}px;`,
  );
</script>

<div class="epub-root" style:--page-bg={palette.bg}>
  <div class="stage-wrap">
    <div class="stage" class:scrolled={flow === "scrolled"} style={stageStyle} bind:this={host}></div>
    {#if loading}
      <div class="overlay"><span class="hint">Загрузка EPUB…</span></div>
    {/if}
    {#if err}
      <div class="overlay"><span class="error">{err}</span></div>
    {/if}
  </div>
</div>

<style>
  .epub-root {
    display: flex;
    flex-direction: column;
    height: 100%;
    min-height: 0;
    background: var(--page-bg);
    position: relative;
    transition: background 0.35s ease;
  }
  .stage-wrap {
    flex: 1;
    min-height: 0;
    position: relative;
    display: flex;
    flex-direction: column;
  }
  .overlay {
    position: absolute;
    inset: 0;
    display: flex;
    align-items: center;
    justify-content: center;
    background: color-mix(in srgb, var(--page-bg) 88%, transparent);
    z-index: 2;
  }
  .hint {
    color: var(--muted);
  }
  .error {
    color: var(--danger);
    text-align: center;
    padding: 1rem;
  }
  .stage {
    flex: 1;
    width: 100%;
    min-height: 0;
    overflow: hidden;
    margin: 0 auto;
    padding-top: 1.4rem !important;
    padding-bottom: 1.4rem !important;
    box-sizing: border-box;
  }
  .stage.scrolled {
    padding-top: 0 !important;
    padding-bottom: 0 !important;
  }
  .stage :global(iframe) {
    border: none !important;
    background: transparent !important;
  }
  .stage :global(.rd-hl) {
    cursor: pointer;
  }

  @media (max-width: 600px) {
    .stage {
      padding-left: max(14px, env(safe-area-inset-left)) !important;
      padding-right: max(14px, env(safe-area-inset-right)) !important;
      padding-top: 0.8rem !important;
    }
  }
</style>
