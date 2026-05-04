<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import type { EpubReaderApi, QuoteDraftOptions } from "$lib/types";
  import { parseFb2, type Fb2Section } from "$lib/fb2/parseFb2";
  import { writeTextToClipboard } from "$lib/clipboardWrite";
  import ReaderSelectionToolbar from "$lib/components/ReaderSelectionToolbar.svelte";
  import CommentDialog from "$lib/components/CommentDialog.svelte";
  import QuoteDialog from "$lib/components/QuoteDialog.svelte";
  import { extractTextNodesHtml } from "$lib/translate/htmlText";
  import { translateStringList } from "$lib/translate/translateApi";

  interface Props {
    relativePath: string;
    variant?: "full" | "reader";
    /** Формат `#anchor` как в spine FB2. */
    initialLocation?: string;
    onReadingProgress?: (p: { location: string; label: string }) => void;
    onReaderApi?: (api: EpubReaderApi | null) => void;
    displayTitle?: string;
    displayAuthor?: string;
    onAddComment?: (c: {
      body: string;
      excerpt: string;
      page?: number;
      chapterLabel?: string;
    }) => void;
    onAddQuote?: (q: { text: string; options: QuoteDraftOptions }) => void;
    translateRunKey?: number;
    translateSource?: string;
    translateTarget?: string;
    translateLayout?: "orig" | "trans" | "split";
    onTranslateActivity?: (p: { busy: boolean; error: string | null }) => void;
  }
  let {
    relativePath,
    variant = "full",
    initialLocation = "",
    onReadingProgress,
    onReaderApi,
    displayTitle = "",
    displayAuthor = "",
    onAddComment,
    onAddQuote,
    translateRunKey = 0,
    translateSource = "auto",
    translateTarget = "ru",
    translateLayout = "orig",
    onTranslateActivity,
  }: Props = $props();

  let scrollRoot = $state<HTMLDivElement | null>(null);
  let loading = $state(true);
  let err = $state<string | null>(null);
  let sections = $state<Fb2Section[]>([]);
  let coverHtml = $state("");
  /** Переведённые секции (после «Перевести книгу»). */
  let sectionsTr = $state<Fb2Section[] | null>(null);
  let processedTranslateKey = $state(0);
  let relocatedLabel = $state("");
  let activeIdx = $state(0);

  let toolbarVisible = $state(false);
  let toolbarX = $state(0);
  let toolbarY = $state(0);
  let selectedText = $state("");
  let commentOpen = $state(false);
  let quoteOpen = $state(false);

  let session = 0;

  const interactionsEnabled = $derived(!!(onAddComment && onAddQuote));

  function buildApi(list: Fb2Section[]): EpubReaderApi {
    const nav = list.map((s) => ({
      label: s.title || "…",
      href: `#${s.anchor}`,
    }));
    return {
      toc: nav,
      spine: nav,
      goTo: async (href: string) => {
        const id = href.replace(/^#/, "");
        document.getElementById(id)?.scrollIntoView({ behavior: "smooth", block: "start" });
      },
      prev: async () => {
        const idx = Math.max(0, activeIdx - 1);
        const s = list[idx];
        if (s) document.getElementById(s.anchor)?.scrollIntoView({ behavior: "smooth", block: "start" });
      },
      next: async () => {
        const idx = Math.min(list.length - 1, activeIdx + 1);
        const s = list[idx];
        if (s) document.getElementById(s.anchor)?.scrollIntoView({ behavior: "smooth", block: "start" });
      },
    };
  }

  async function loadBook(path: string, sid: number) {
    loading = true;
    err = null;
    sections = [];
    relocatedLabel = "";
    onReaderApi?.(null);
    try {
      const b64 = await invoke<string>("read_book_base64", { relativePath: path });
      if (sid !== session) return;
      const bin = atob(b64);
      const bytes = new Uint8Array(bin.length);
      for (let i = 0; i < bin.length; i++) bytes[i] = bin.charCodeAt(i);
      const xml = new TextDecoder("utf-8").decode(bytes);
      const parsed = parseFb2(xml);
      if (sid !== session) return;
      coverHtml = parsed.coverHtml ?? "";
      sections = parsed.sections;
      sectionsTr = null;
      relocatedLabel = parsed.sections[0]?.title ?? "";
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

  $effect(() => {
    const root = scrollRoot;
    const secs = sections;
    if (!root || secs.length === 0) return;

    const obs = new IntersectionObserver(
      (entries) => {
        const visible = entries
          .filter((e) => e.isIntersecting)
          .sort((a, b) => b.intersectionRatio - a.intersectionRatio);
        const top = visible[0];
        if (!top?.target.id) return;
        const ix = secs.findIndex((s) => s.anchor === top.target.id);
        if (ix >= 0) {
          activeIdx = ix;
          relocatedLabel = secs[ix].title;
          const s = secs[ix];
          onReadingProgress?.({ location: `#${s.anchor}`, label: s.title });
        }
      },
      { root, rootMargin: "-12% 0px -60% 0px", threshold: [0, 0.1, 0.25] },
    );

    const t = window.setTimeout(() => {
      for (const s of secs) {
        const el = document.getElementById(s.anchor);
        if (el) obs.observe(el);
      }
    }, 0);

    return () => {
      window.clearTimeout(t);
      obs.disconnect();
    };
  });

  /** Восстановление якоря после загрузки секций */
  $effect(() => {
    const loc = initialLocation?.trim() ?? "";
    const secs = sections;
    if (!loc || secs.length === 0) return;
    const id = loc.replace(/^#/, "");
    const t = window.setTimeout(() => {
      document.getElementById(id)?.scrollIntoView({ block: "start" });
    }, 80);
    return () => window.clearTimeout(t);
  });

  function refreshToolbar() {
    if (!interactionsEnabled || loading || err) {
      toolbarVisible = false;
      return;
    }
    const sel = document.getSelection();
    if (!sel || sel.isCollapsed || !scrollRoot) {
      toolbarVisible = false;
      return;
    }
    const t = sel.toString().trim();
    if (!t) {
      toolbarVisible = false;
      return;
    }
    if (!scrollRoot.contains(sel.anchorNode)) {
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
    document.addEventListener("selectionchange", refreshToolbar);
    document.addEventListener("mouseup", refreshToolbar);
    return () => {
      document.removeEventListener("selectionchange", refreshToolbar);
      document.removeEventListener("mouseup", refreshToolbar);
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
      chapterLabel: relocatedLabel || undefined,
    });
    document.getSelection()?.removeAllRanges();
  }

  function saveQuote(opts: QuoteDraftOptions) {
    onAddQuote?.({ text: selectedText.trim(), options: opts });
    document.getSelection()?.removeAllRanges();
  }

  const pageHintStr = $derived(
    relocatedLabel ? `Раздел: ${relocatedLabel}` : "Книга FB2",
  );
</script>

<div class="fb2-root" class:reader={variant === "reader"}>
  <div class="fb2-stage-wrap">
    {#if loading}
      <div class="overlay"><span class="hint">Загрузка FB2…</span></div>
    {:else if err}
      <div class="overlay"><span class="error">{err}</span></div>
    {:else}
      <div class="fb2-scroll" bind:this={scrollRoot}>
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
      </div>
    {/if}
  </div>
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
    pageHint={pageHintStr}
    onClose={() => (commentOpen = false)}
    onSave={saveComment}
  />

  <QuoteDialog
    open={quoteOpen}
    quoteText={selectedText}
    bookTitle={displayTitle}
    bookAuthor={displayAuthor}
    pageLabel=""
    chapterLabel={relocatedLabel}
    onClose={() => (quoteOpen = false)}
    onSave={saveQuote}
  />
{/if}

<style>
  .fb2-root {
    display: flex;
    flex-direction: column;
    height: 100%;
    min-height: 0;
    background: var(--reader-bg);
  }

  .fb2-root.reader {
    background: var(--elevated-soft);
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
    padding: 0.75rem 1.1rem 1.25rem;
    scroll-behavior: smooth;
  }

  .fb2-section {
    max-width: 40rem;
    margin: 0 auto 1.75rem;
  }

  .fb2-split-section {
    max-width: min(52rem, 100%);
  }

  .fb2-split-head {
    margin-bottom: 0.65rem;
  }

  .fb2-sec-title-tr {
    margin: 0.35rem 0 0;
    font-size: 0.95rem;
    font-weight: 550;
    color: var(--muted);
    font-family: system-ui, sans-serif;
    line-height: 1.35;
  }

  .fb2-split-grid {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 0 1rem;
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
    color: var(--muted);
    font-family: system-ui, sans-serif;
  }

  .fb2-split-col {
    min-width: 0;
    padding: 0.5rem 0.55rem;
    border-radius: var(--radius-md);
    border: 1px solid color-mix(in srgb, var(--border-soft) 85%, transparent);
    background: color-mix(in srgb, var(--elevated-soft) 55%, transparent);
  }

  .fb2-sec-title {
    margin: 0 0 0.85rem;
    font-size: 1.05rem;
    font-weight: 650;
    color: var(--text-soft);
    font-family: system-ui, sans-serif;
    letter-spacing: -0.02em;
  }

  .fb2-html {
    font-size: 1.02rem;
    line-height: 1.65;
    color: var(--text-soft);
    font-family: Literata, Georgia, serif;
  }

  .fb2-html :global(.fb2-p) {
    margin: 0 0 0.65em;
    text-indent: 1.25em;
  }

  .fb2-html :global(.fb2-p:first-child) {
    text-indent: 0;
  }

  .fb2-html :global(.fb2-empty-line) {
    height: 0.65em;
  }

  .fb2-html :global(.fb2-subtitle) {
    margin: 1.1em 0 0.5em;
    font-size: 1rem;
    font-weight: 600;
    font-family: system-ui, sans-serif;
  }

  .fb2-html :global(.fb2-epigraph) {
    margin: 1rem 0;
    padding: 0.55rem 0 0.55rem 0.85rem;
    border-left: 3px solid color-mix(in srgb, var(--accent) 45%, transparent);
    font-style: italic;
    color: var(--muted);
  }

  .fb2-html :global(.fb2-poem) {
    margin: 1rem 0;
  }

  .fb2-html :global(.fb2-stanza) {
    margin-bottom: 0.75em;
  }

  .fb2-html :global(.fb2-v) {
    margin: 0.15em 0;
    padding-left: 1em;
  }

  .fb2-html :global(.fb2-cite) {
    font-style: italic;
  }

  .fb2-html :global(.fb2-text-author),
  .fb2-html :global(.fb2-date) {
    margin: 0.35em 0;
    font-size: 0.92em;
    color: var(--muted);
    text-indent: 0;
  }

  .fb2-html :global(.fb2-fig) {
    margin: 1rem auto;
    text-align: center;
    max-width: 100%;
  }

  .fb2-cover-block {
    margin: 0 auto 1.35rem;
    padding: 0 0.25rem;
    max-width: min(100%, 36rem);
    text-align: center;
  }
  .fb2-cover-block :global(.fb2-img) {
    max-width: 100%;
    height: auto;
    border-radius: var(--radius-md);
    box-shadow: var(--shadow-soft);
  }
  .fb2-html :global(.fb2-img) {
    max-width: 100%;
    height: auto;
    border-radius: var(--radius-sm);
    box-shadow: var(--shadow-soft);
  }

  .fb2-html :global(.fb2-table) {
    width: 100%;
    border-collapse: collapse;
    margin: 1rem 0;
    font-size: 0.92em;
  }

  .fb2-html :global(.fb2-table td),
  .fb2-html :global(.fb2-table th) {
    border: 1px solid var(--border-soft);
    padding: 0.35rem 0.45rem;
  }

  .overlay {
    position: absolute;
    inset: 0;
    display: flex;
    align-items: center;
    justify-content: center;
    background: color-mix(in srgb, var(--elevated-soft) 85%, transparent);
    z-index: 2;
  }

  .hint {
    color: var(--muted);
    font-size: 0.9rem;
  }

  .error {
    color: var(--danger);
    padding: 0 1rem;
    text-align: center;
    font-size: 0.88rem;
    line-height: 1.45;
  }
</style>
