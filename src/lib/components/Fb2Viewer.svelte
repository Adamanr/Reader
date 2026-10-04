<script lang="ts">
  import { tick, untrack } from "svelte";
  import type {
    EpubReaderApi,
    Highlight,
    ReaderSelection,
    ReadingPosition,
    ReadingUnit,
    SearchHit,
    TextChunk,
  } from "$lib/types";
  import { decodeFb2Bytes, parseFb2, type Fb2Section } from "$lib/fb2/parseFb2";
  import { extractTextNodesHtml } from "$lib/translate/htmlText";
  import { translateStringList } from "$lib/translate/translateApi";
  import { readLibraryBookBytes } from "$lib/library/readLibraryBookBytes";
  import { ensureAppFonts } from "$lib/reading/fonts";
  import { snapScroll } from "$lib/ui/snapScroll";
  import { typographyVars, varsToStyle } from "$lib/reading/contentCss";
  import type { PagePalette } from "$lib/reading/palette";
  import { reading } from "$lib/reading/settings.svelte";
  import {
    excerptAround,
    findAll,
    locateText,
    offsetOfPoint,
    rangeFromOffsets,
    unwrapAll,
    wrapRange,
  } from "$lib/reading/textAnchor";

  interface Props {
    relativePath: string;
    /** `b:<номер абзаца>` или устаревший `#anchor` секции. */
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

  const BLOCK_SEL = ".fb2-section :is(p, h2, h3, .fb2-v)";

  let scrollRoot = $state<HTMLDivElement | null>(null);
  let loading = $state(true);
  let err = $state<string | null>(null);
  let sections = $state<Fb2Section[]>([]);
  let coverHtml = $state("");
  /** Переведённые секции (после «Перевести книгу»). */
  let sectionsTr = $state<Fb2Section[] | null>(null);
  let processedTranslateKey = $state(0);
  let session = 0;

  /** Абзацы в порядке чтения и накопленная длина текста до каждого. */
  let blocks: HTMLElement[] = [];
  let cumChars: number[] = [];
  let totalChars = 0;
  let blockSection: number[] = [];
  let sectionEndChars: number[] = [];
  let articles: HTMLElement[] = [];
  let currentBlock = 0;
  let restored = false;

  ensureAppFonts();

  function sectionHref(i: number) {
    return `#${sections[i]?.anchor ?? ""}`;
  }

  function scrollToBlock(i: number, smooth = false) {
    const el = blocks[Math.max(0, Math.min(blocks.length - 1, i))];
    el?.scrollIntoView({ block: "start", behavior: smooth ? "smooth" : "auto" });
  }

  function buildApi(list: Fb2Section[]): EpubReaderApi {
    const nav = list.map((s) => ({
      label: s.title || "…",
      href: `#${s.anchor}`,
      level: s.level,
    }));
    return {
      toc: nav,
      spine: nav,
      goTo: async (href: string) => {
        const id = href.replace(/^#/, "");
        scrollRoot?.querySelector(`#${CSS.escape(id)}`)?.scrollIntoView({ behavior: "smooth", block: "start" });
      },
      prev: async () => pageBy(-1),
      next: async () => pageBy(1),
      seek: async (f: number) => {
        const r = scrollRoot;
        if (!r) return;
        const target = cumChars.findIndex((c) => c >= f * totalChars);
        if (target >= 0) scrollToBlock(target);
        else r.scrollTop = r.scrollHeight;
      },
      search,
      goToHit,
      unitsFromHere,
      advanceChapter,
      chunksBefore,
    };
  }

  function pageBy(dir: number) {
    const r = scrollRoot;
    if (!r) return;
    r.scrollBy({ top: dir * r.clientHeight * 0.9, behavior: "smooth" });
  }

  async function loadBook(path: string, sid: number) {
    loading = true;
    err = null;
    sections = [];
    restored = false;
    onReaderApi?.(null);
    try {
      const bytes = await readLibraryBookBytes(path);
      if (sid !== session) return;
      const parsed = parseFb2(decodeFb2Bytes(bytes));
      if (sid !== session) return;
      coverHtml = parsed.coverHtml ?? "";
      sections = parsed.sections;
      sectionsTr = null;
      onReaderApi?.(buildApi(parsed.sections));
    } catch (e) {
      if (sid === session) err = String(e);
      onReaderApi?.(null);
    } finally {
      if (sid === session) loading = false;
    }
  }

  $effect(() => {
    const path = relativePath;
    processedTranslateKey = 0;
    sectionsTr = null;
    const sid = ++session;
    void loadBook(path, sid);
    return () => {
      session += 1;
      onReaderApi?.(null);
    };
  });

  async function runFullTranslate() {
    const src = sections;
    if (src.length === 0) return;
    const sid = session;
    onTranslateActivity?.({ busy: true, error: null });
    try {
      const out: Fb2Section[] = [];
      for (const sec of src) {
        if (sid !== session) return;
        const ex = extractTextNodesHtml(sec.html);
        const head: string[] = [];
        if (sec.title.trim()) head.push(sec.title);
        const batch = [...head, ...ex.texts];
        const tr = await translateStringList(batch, translateSource, translateTarget);
        let ti = 0;
        const newTitle = sec.title.trim() ? tr[ti++]! : sec.title;
        const rest = tr.slice(ti);
        const newHtml = ex.apply(rest);
        out.push({ ...sec, title: newTitle, html: newHtml });
      }
      if (sid !== session) return;
      sectionsTr = out;
      onTranslateActivity?.({ busy: false, error: null });
    } catch (e) {
      onTranslateActivity?.({
        busy: false,
        error: e instanceof Error ? e.message : String(e),
      });
    }
  }

  $effect(() => {
    const k = translateRunKey;
    if (k === 0 || sections.length === 0) return;
    if (k <= processedTranslateKey) return;
    processedTranslateKey = k;
    void runFullTranslate();
  });

  /** Пересобираем карту абзацев после каждой перерисовки текста. */
  function indexBlocks() {
    const root = scrollRoot;
    if (!root) return;
    articles = Array.from(root.querySelectorAll<HTMLElement>("article.fb2-section"));
    blocks = Array.from(root.querySelectorAll<HTMLElement>(BLOCK_SEL)).filter(
      (el) => !el.parentElement?.closest(":is(p, h2, h3, .fb2-v)"),
    );
    cumChars = [];
    blockSection = [];
    sectionEndChars = new Array(articles.length).fill(0);
    let acc = 0;
    for (const el of blocks) {
      cumChars.push(acc);
      acc += (el.textContent ?? "").length;
      const art = el.closest("article.fb2-section") as HTMLElement | null;
      const si = art ? articles.indexOf(art) : -1;
      blockSection.push(si);
      if (si >= 0) sectionEndChars[si] = acc;
    }
    totalChars = acc || 1;
  }

  function firstVisibleBlock(): number {
    const root = scrollRoot;
    if (!root || blocks.length === 0) return 0;
    const top = root.getBoundingClientRect().top + 6;
    let lo = 0;
    let hi = blocks.length - 1;
    while (lo < hi) {
      const mid = (lo + hi) >> 1;
      if (blocks[mid]!.getBoundingClientRect().bottom <= top) lo = mid + 1;
      else hi = mid;
    }
    return lo;
  }

  function blockAtLine(fraction: number): number {
    const root = scrollRoot;
    if (!root || blocks.length === 0) return 0;
    const rr = root.getBoundingClientRect();
    const y = rr.top + rr.height * fraction;
    let lo = 0;
    let hi = blocks.length - 1;
    while (lo < hi) {
      const mid = (lo + hi) >> 1;
      if (blocks[mid]!.getBoundingClientRect().bottom < y) lo = mid + 1;
      else hi = mid;
    }
    return lo;
  }

  let lastEmitKey = "";

  function emitPosition(force = false) {
    if (blocks.length === 0) return;
    const i = firstVisibleBlock();
    currentBlock = i;
    const root = scrollRoot!;
    const atEnd = root.scrollTop + root.clientHeight >= root.scrollHeight - 4;
    if (reading.s.focus === "paragraph") setFocusBlock(blockAtLine(0.38));
    // Прокрутка внутри того же абзаца ничего не меняет — не будим весь интерфейс.
    const key = `${i}|${atEnd}|${restored}`;
    if (!force && key === lastEmitKey) return;
    lastEmitKey = key;
    const si = blockSection[i] ?? -1;
    const sec = sections[si];
    const read = cumChars[i] ?? 0;
    const progress = atEnd ? 1 : read / totalChars;
    onPosition?.({
      progress,
      chapterLabel: sec?.title ?? "",
      charsLeftChapter: si >= 0 ? Math.max(0, (sectionEndChars[si] ?? read) - read) : null,
      charsLeftBook: Math.max(0, totalChars - read),
    });
    if (restored) {
      onReadingProgress?.({
        location: `b:${i}`,
        label: sec?.title ?? "",
        href: si >= 0 ? sectionHref(si) : "",
        progress,
      });
    }
  }

  function setFocusBlock(i: number) {
    const root = scrollRoot;
    if (!root) return;
    for (const el of Array.from(root.querySelectorAll(".rd-focus-current"))) el.classList.remove("rd-focus-current");
    blocks[i]?.classList.add("rd-focus-current");
  }

  let scrollRaf = 0;
  function onScroll() {
    if (scrollRaf) return;
    scrollRaf = requestAnimationFrame(() => {
      scrollRaf = 0;
      emitPosition();
    });
  }

  $effect(() => {
    const el = scrollRoot;
    if (el) return snapScroll(el);
  });

  /** После отрисовки: индекс абзацев, восстановление позиции, выделения. */
  $effect(() => {
    const root = scrollRoot;
    const secs = sections;
    translateLayout;
    sectionsTr;
    if (!root || secs.length === 0) return;
    void tick().then(() => {
      indexBlocks();
      paintHighlights();
      if (!restored) {
        const loc = untrack(() => initialLocation?.trim() ?? "");
        if (loc.startsWith("b:")) {
          scrollToBlock(Number(loc.slice(2)) || 0);
        } else if (loc.startsWith("#")) {
          root.querySelector(`#${CSS.escape(loc.slice(1))}`)?.scrollIntoView({ block: "start" });
        }
        restored = true;
      }
      emitPosition(true);
    });
  });

  /** Типографика меняет высоту текста — возвращаемся к тому же абзацу. */
  $effect(() => {
    reading.s.fontSize;
    reading.s.lineHeight;
    reading.s.measure;
    reading.s.font;
    reading.s.paraSpacing;
    reading.s.letterSpacing;
    reading.s.wordSpacing;
    const keep = untrack(() => currentBlock);
    if (!untrack(() => restored)) return;
    void tick().then(() => scrollToBlock(keep));
  });

  $effect(() => {
    if (reading.s.focus === "paragraph" && scrollRoot) setFocusBlock(blockAtLine(0.38));
  });

  function paintHighlights() {
    const root = scrollRoot;
    if (!root) return;
    unwrapAll(root, "mark.rd-hl");
    if (translateLayout !== "orig") return;
    for (const h of highlights) {
      if (h.block == null) continue;
      const art = articles[h.block];
      if (!art) continue;
      const loc = locateText(art, h.text, h.offset ?? 0);
      if (!loc) continue;
      const range = rangeFromOffsets(art, loc.start, loc.end);
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
    untrack(() => {
      if (blocks.length) paintHighlights();
    });
  });

  function refreshSelection() {
    const root = scrollRoot;
    if (!root || loading || err) return;
    const sel = document.getSelection();
    if (!sel || sel.isCollapsed || sel.rangeCount === 0 || !root.contains(sel.anchorNode)) {
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
    const startEl =
      range.startContainer.nodeType === Node.ELEMENT_NODE
        ? (range.startContainer as Element)
        : range.startContainer.parentElement;
    const art = startEl?.closest("article.fb2-section") as HTMLElement | null;
    const block = art ? articles.indexOf(art) : -1;
    const offset = art ? offsetOfPoint(art, range.startContainer, range.startOffset) : null;
    onSelection?.({
      text,
      context: startEl?.closest(":is(p, h2, h3, .fb2-v)")?.textContent ?? "",
      rect: { left: rect.left, top: rect.top, width: rect.width, height: rect.height },
      anchor: {
        block: block >= 0 ? block : undefined,
        offset: offset ?? undefined,
        chapterLabel: block >= 0 ? sections[block]?.title : undefined,
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

  function onRootClick(e: MouseEvent) {
    const sel = document.getSelection();
    if (sel && !sel.isCollapsed) return;
    const target = e.target as Element;
    const mark = target.closest("mark.rd-hl") as HTMLElement | null;
    if (mark?.dataset.id) {
      onHighlightClick?.(mark.dataset.id, mark.getBoundingClientRect());
      return;
    }
    if (target.closest("a, button, img")) return;
    const r = scrollRoot!.getBoundingClientRect();
    const x = (e.clientX - r.left) / r.width;
    if (reading.s.tapZones && x < 0.22) pageBy(-1);
    else if (reading.s.tapZones && x > 0.78) pageBy(1);
    else onCenterTap?.();
  }

  function onRootMouseMove(e: MouseEvent) {
    if (reading.s.focus !== "paragraph") return;
    const el = (e.target as Element).closest(":is(p, h2, h3, .fb2-v)") as HTMLElement | null;
    const i = el ? blocks.indexOf(el) : -1;
    if (i >= 0) setFocusBlock(i);
  }

  async function search(query: string): Promise<SearchHit[]> {
    const hits: SearchHit[] = [];
    articles.forEach((art, si) => {
      const text = art.textContent ?? "";
      for (const start of findAll(text, query)) {
        const ex = excerptAround(text, start, start + query.trim().length);
        hits.push({
          id: `s:${si}:${start}`,
          label: sections[si]?.title || `Раздел ${si + 1}`,
          ...ex,
          loc: `s:${si}:${start}:${query.trim().length}`,
        });
      }
    });
    return hits.slice(0, 400);
  }

  async function goToHit(hit: SearchHit) {
    const [, si, start, len] = hit.loc.split(":");
    const art = articles[Number(si)];
    if (!art) return;
    const loc = hit.match ? locateText(art, hit.match, Number(start)) : null;
    const range = loc
      ? rangeFromOffsets(art, loc.start, loc.end)
      : rangeFromOffsets(art, Number(start), Number(start) + Number(len));
    if (!range) return;
    const marks = wrapRange(range, () => {
      const m = document.createElement("mark");
      m.className = "rd-search-hit";
      return m;
    });
    marks[0]?.scrollIntoView({ block: "center", behavior: "smooth" });
    setTimeout(() => unwrapAll(art, "mark.rd-search-hit"), 2600);
  }

  function isOnScreen(el: HTMLElement) {
    const rr = scrollRoot!.getBoundingClientRect();
    const r = el.getBoundingClientRect();
    return r.top >= rr.top - 2 && r.bottom <= rr.bottom + 2;
  }

  async function unitsFromHere(): Promise<ReadingUnit[]> {
    if (!blocks.length) return [];
    const start = firstVisibleBlock();
    const si = blockSection[start];
    const out: ReadingUnit[] = [];
    for (let i = start; i < blocks.length && blockSection[i] === si; i++) {
      const el = blocks[i]!;
      const text = (el.textContent ?? "").replace(/\s+/g, " ").trim();
      if (!text) continue;
      out.push({
        text,
        mark: () => el.classList.add("rd-tts-current"),
        unmark: () => el.classList.remove("rd-tts-current"),
        reveal: async () => {
          if (!isOnScreen(el)) el.scrollIntoView({ block: "center", behavior: "smooth" });
        },
      });
    }
    return out;
  }

  async function advanceChapter(): Promise<boolean> {
    const si = blockSection[firstVisibleBlock()] ?? 0;
    const next = articles[si + 1];
    if (!next) return false;
    next.scrollIntoView({ block: "start" });
    await new Promise((r) => setTimeout(r, 60));
    return true;
  }

  async function chunksBefore(): Promise<TextChunk[]> {
    if (!blocks.length) return [];
    const root = scrollRoot!;
    const rr = root.getBoundingClientRect();
    // Всё, что видно на экране, уже прочитано.
    let last = firstVisibleBlock();
    while (last + 1 < blocks.length && blocks[last + 1]!.getBoundingClientRect().top < rr.bottom) last++;
    const curSection = blockSection[last] ?? 0;
    const out: TextChunk[] = [];
    const sectionText = (si: number) =>
      blocks
        .filter((_, i) => blockSection[i] === si)
        .map((b) => (b.textContent ?? "").replace(/\s+/g, " ").trim())
        .filter(Boolean)
        .join("\n");
    for (let si = 0; si < curSection; si++) {
      const text = sectionText(si);
      if (text.length < 40) continue;
      out.push({ id: `sec-${si}`, label: sections[si]?.title || `Раздел ${si + 1}`, text, partial: false });
    }
    const parts: string[] = [];
    for (let i = 0; i <= last; i++) {
      if (blockSection[i] === curSection) parts.push((blocks[i]!.textContent ?? "").replace(/\s+/g, " ").trim());
    }
    const text = parts.filter(Boolean).join("\n");
    if (text) out.push({ id: `sec-${curSection}-partial`, label: sections[curSection]?.title || "Текущий раздел", text, partial: true });
    return out;
  }

  const rootStyle = $derived(varsToStyle(typographyVars(reading.s, palette)));
</script>

<div class="fb2-root" style={rootStyle}>
  <div class="fb2-stage-wrap">
    {#if loading}
      <div class="overlay"><span class="hint">Загрузка FB2…</span></div>
    {:else if err}
      <div class="overlay"><span class="error">{err}</span></div>
    {:else}
      <!-- svelte-ignore a11y_click_events_have_key_events -->
      <!-- svelte-ignore a11y_no_static_element_interactions -->
      <div
        class="fb2-scroll"
        class:focus-dim={reading.s.focus === "paragraph"}
        bind:this={scrollRoot}
        onscroll={onScroll}
        onclick={onRootClick}
        onmousemove={onRootMouseMove}
        lang="ru"
      >
        {#if coverHtml.trim()}
          <div class="fb2-cover-block fb2-html">{@html coverHtml}</div>
        {/if}
        {#if translateLayout === "split" && sectionsTr && sectionsTr.length === sections.length}
          {#each sections as sec, i (sec.anchor)}
            {@const tr = sectionsTr[i]!}
            <article class="fb2-section fb2-split-section" id={sec.anchor}>
              <div class="fb2-split-head">
                {#if sec.title}
                  <h2 class="fb2-sec-title">{sec.title}</h2>
                {/if}
                {#if tr.title && tr.title !== sec.title}
                  <p class="fb2-sec-title-tr">{tr.title}</p>
                {/if}
              </div>
              <div class="fb2-split-grid">
                <div class="fb2-split-col">
                  <p class="fb2-split-label">Оригинал</p>
                  <div class="fb2-html">{@html sec.html}</div>
                </div>
                <div class="fb2-split-col">
                  <p class="fb2-split-label">Перевод</p>
                  <div class="fb2-html">{@html tr.html}</div>
                </div>
              </div>
            </article>
          {/each}
        {:else if translateLayout === "trans" && sectionsTr}
          {#each sectionsTr as sec (sec.anchor)}
            <article class="fb2-section" id={sec.anchor}>
              {#if sec.title}
                <h2 class="fb2-sec-title">{sec.title}</h2>
              {/if}
              <div class="fb2-html">{@html sec.html}</div>
            </article>
          {/each}
        {:else}
          {#each sections as sec (sec.anchor)}
            <article class="fb2-section" id={sec.anchor}>
              {#if sec.title}
                <h2 class="fb2-sec-title">{sec.title}</h2>
              {/if}
              <div class="fb2-html">{@html sec.html}</div>
            </article>
          {/each}
        {/if}
        <div class="fb2-end" aria-hidden="true">✦</div>
      </div>
    {/if}
  </div>
</div>

<style>
  .fb2-root {
    display: flex;
    flex-direction: column;
    height: 100%;
    min-height: 0;
    background: var(--rd-bg);
    color: var(--rd-text);
    transition:
      background 0.35s ease,
      color 0.35s ease;
  }

  .fb2-stage-wrap {
    flex: 1;
    min-height: 0;
    position: relative;
    display: flex;
    flex-direction: column;
  }

  .fb2-scroll {
    flex: 1;
    overflow: auto;
    min-height: 0;
    padding: 2rem var(--rd-margin) 3rem;
    font-family: var(--rd-font);
    font-size: var(--rd-size);
    line-height: var(--rd-line);
    letter-spacing: var(--rd-letter);
    word-spacing: var(--rd-word);
    hyphens: var(--rd-hyphens);
    -webkit-hyphens: var(--rd-hyphens);
    color: var(--rd-text);
    scrollbar-gutter: stable;
  }

  .fb2-section {
    max-width: var(--rd-measure);
    margin: 0 auto 2.2em;
  }

  .fb2-split-section {
    max-width: min(calc(var(--rd-measure) * 2), 100%);
  }

  .fb2-split-head {
    margin-bottom: 0.65rem;
  }

  .fb2-sec-title-tr {
    margin: 0.35rem 0 0;
    font-size: 0.9em;
    font-weight: 550;
    color: var(--rd-muted);
    line-height: 1.35;
  }

  .fb2-split-grid {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 0 1.2rem;
    align-items: start;
  }

  @media (max-width: 800px) {
    .fb2-split-grid {
      grid-template-columns: 1fr;
      gap: 1rem;
    }
  }

  .fb2-split-label {
    margin: 0 0 0.35rem;
    font-size: 0.62rem;
    font-weight: 700;
    text-transform: uppercase;
    letter-spacing: 0.14em;
    color: var(--rd-muted);
    font-family: system-ui, sans-serif;
  }

  .fb2-split-col {
    min-width: 0;
  }

  .fb2-sec-title {
    margin: 0 0 1.1em;
    font-size: 1.35em;
    font-weight: 600;
    line-height: 1.25;
    text-align: center;
    letter-spacing: -0.01em;
    text-wrap: balance;
  }

  .fb2-html {
    overflow-wrap: break-word;
  }

  .fb2-html :global(.fb2-p) {
    margin: 0 0 var(--rd-para);
    text-indent: var(--rd-indent);
    text-align: var(--rd-align);
  }

  .fb2-html :global(.fb2-empty-line) {
    height: 0.8em;
  }

  .fb2-html :global(.fb2-subtitle) {
    margin: 1.4em 0 0.7em;
    font-size: 1.05em;
    font-weight: 600;
    text-align: center;
  }

  .fb2-html :global(.fb2-epigraph) {
    margin: 1.2em 0 1.6em auto;
    max-width: 80%;
    font-style: italic;
    font-size: 0.94em;
    color: var(--rd-muted);
  }

  .fb2-html :global(.fb2-epigraph .fb2-p) {
    text-indent: 0;
    text-align: left;
  }

  .fb2-html :global(.fb2-poem) {
    margin: 1.2em 0 1.2em 1.5em;
  }

  .fb2-html :global(.fb2-stanza) {
    margin-bottom: 0.9em;
  }

  .fb2-html :global(.fb2-v) {
    margin: 0.1em 0;
  }

  .fb2-html :global(.fb2-cite) {
    display: block;
    margin: 1em 0;
    padding-left: 1em;
    border-left: 2px solid color-mix(in srgb, var(--rd-accent) 40%, transparent);
    font-style: italic;
  }

  .fb2-html :global(.fb2-text-author),
  .fb2-html :global(.fb2-date) {
    margin: 0.35em 0;
    font-size: 0.92em;
    color: var(--rd-muted);
    text-indent: 0;
    text-align: right;
  }

  .fb2-html :global(.fb2-noteref) {
    font-size: 0.72em;
    vertical-align: super;
    color: var(--rd-accent);
  }

  .fb2-html :global(.fb2-fig) {
    margin: 1.2em auto;
    text-align: center;
    max-width: 100%;
  }

  .fb2-cover-block {
    margin: 0 auto 2.4rem;
    max-width: min(100%, 22rem);
    text-align: center;
  }
  .fb2-cover-block :global(.fb2-img) {
    max-width: 100%;
    height: auto;
    border-radius: 6px;
    box-shadow: var(--shadow-book);
  }
  .fb2-html :global(.fb2-img) {
    max-width: 100%;
    height: auto;
    border-radius: 4px;
  }

  .fb2-html :global(.fb2-table) {
    width: 100%;
    border-collapse: collapse;
    margin: 1rem 0;
    font-size: 0.9em;
  }

  .fb2-html :global(.fb2-table td),
  .fb2-html :global(.fb2-table th) {
    border: 1px solid color-mix(in srgb, var(--rd-muted) 40%, transparent);
    padding: 0.35rem 0.45rem;
  }

  .fb2-scroll :global(mark.rd-hl) {
    color: inherit;
    border-radius: 2px;
    cursor: pointer;
    background: color-mix(in srgb, var(--hl) 45%, transparent);
    box-decoration-break: clone;
    -webkit-box-decoration-break: clone;
  }
  .fb2-scroll :global(mark.rd-hl-yellow) {
    --hl: #f5d565;
  }
  .fb2-scroll :global(mark.rd-hl-green) {
    --hl: #94d49a;
  }
  .fb2-scroll :global(mark.rd-hl-blue) {
    --hl: #8ec5ee;
  }
  .fb2-scroll :global(mark.rd-hl-pink) {
    --hl: #f2a3c0;
  }
  .fb2-scroll :global(mark.rd-hl-violet) {
    --hl: #c3a8f0;
  }
  .fb2-scroll :global(mark.rd-hl-note) {
    border-bottom: 2px dotted color-mix(in srgb, var(--rd-text) 55%, transparent);
  }
  .fb2-scroll :global(mark.rd-search-hit) {
    color: inherit;
    background: color-mix(in srgb, var(--rd-accent) 38%, transparent);
    border-radius: 2px;
    animation: hit-pulse 1.2s ease-out 2;
  }
  .fb2-scroll :global(.rd-tts-current) {
    background: color-mix(in srgb, var(--rd-accent) 14%, transparent);
    border-radius: 4px;
    box-shadow: 0 0 0 6px color-mix(in srgb, var(--rd-accent) 14%, transparent);
  }
  .fb2-scroll.focus-dim :global(:is(.fb2-p, .fb2-v, h2, h3)) {
    opacity: 0.3;
    transition: opacity 0.25s ease;
  }
  .fb2-scroll.focus-dim :global(.rd-focus-current) {
    opacity: 1;
  }

  .fb2-end {
    text-align: center;
    color: var(--rd-muted);
    opacity: 0.6;
    padding: 1rem 0 30vh;
  }

  @keyframes hit-pulse {
    50% {
      background: color-mix(in srgb, var(--rd-accent) 65%, transparent);
    }
  }

  .overlay {
    position: absolute;
    inset: 0;
    display: flex;
    align-items: center;
    justify-content: center;
    background: var(--rd-bg);
    z-index: 2;
  }

  .hint {
    color: var(--rd-muted);
    font-size: 0.9rem;
  }

  .error {
    color: var(--danger);
    padding: 0 1rem;
    text-align: center;
    font-size: 0.88rem;
    line-height: 1.45;
  }

  @media (max-width: 600px) {
    .fb2-scroll {
      padding:
        1.2rem
        max(16px, env(safe-area-inset-right))
        max(1.5rem, env(safe-area-inset-bottom))
        max(16px, env(safe-area-inset-left));
      overscroll-behavior: contain;
    }

    .fb2-html :global(.fb2-table) {
      display: block;
      max-width: 100%;
      overflow-x: auto;
    }
  }
</style>
