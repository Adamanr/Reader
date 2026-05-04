<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import type { EpubReaderApi, QuoteDraftOptions } from "$lib/types";
  import { writeTextToClipboard } from "$lib/clipboardWrite";
  import ReaderSelectionToolbar from "$lib/components/ReaderSelectionToolbar.svelte";
  import CommentDialog from "$lib/components/CommentDialog.svelte";
  import QuoteDialog from "$lib/components/QuoteDialog.svelte";
  import { extractTextNodesHtml } from "$lib/translate/htmlText";
  import { translateStringList } from "$lib/translate/translateApi";

  interface Props {
    relativePath: string;
    variant?: "full" | "reader";
    /** Восстановить позицию (href из spine, как в epub.js relocated). */
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

  let relocatedLabel = $state("");
  let currentHref = $state("");
  let toolbarVisible = $state(false);
  let toolbarX = $state(0);
  let toolbarY = $state(0);
  let selectedText = $state("");
  let commentOpen = $state(false);
  let quoteOpen = $state(false);

  const interactionsEnabled = $derived(!!(onAddComment && onAddQuote));

  function flattenToc(items: any[], depth = 0): { label: string; href: string }[] {
    const r: { label: string; href: string }[] = [];
    for (const it of items || []) {
      const pad = "\u2003".repeat(depth);
      r.push({ label: pad + (it.label || "Без названия"), href: it.href });
      if (it.subitems?.length) r.push(...flattenToc(it.subitems, depth + 1));
    }
    return r;
  }

  function buildSpineList(b: any): { label: string; href: string }[] {
    const list: { label: string; href: string }[] = [];
    const items = b?.spine?.spineItems as { href: string }[] | undefined;
    if (!items?.length) return list;
    for (const section of items) {
      const href = section.href;
      if (!href) continue;
      const short = href.includes("/") ? href.split("/").pop() || href : href;
      list.push({ label: short, href });
    }
    return list;
  }

  function attachSelectionHooks(contents: any) {
    if (!interactionsEnabled) return;
    const doc = contents.document;
    const win = contents.window;
    const refresh = () => {
      requestAnimationFrame(() => {
        const sel = win.getSelection();
        if (!sel || sel.isCollapsed || sel.rangeCount === 0) {
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
        const iframeEl = win.frameElement as HTMLIFrameElement | null;
        const fr = iframeEl?.getBoundingClientRect();
        if (!fr) {
          toolbarVisible = false;
          return;
        }
        toolbarX = fr.left + rect.left + rect.width / 2;
        toolbarY = fr.top + rect.top;
        toolbarVisible = true;
      });
    };
    doc.addEventListener("mouseup", refresh);
    doc.addEventListener("keyup", refresh);
  }

  async function applyEpubTranslation(contents: any) {
    const href = String(contents?.href || currentHref || "");
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

  function translationContentHook(contents: any) {
    attachSelectionHooks(contents);
    void applyEpubTranslation(contents);
  }

  async function mountBook(
    path: string,
    el: HTMLDivElement,
    sid: number,
    openAt: string,
  ) {
    loading = true;
    err = null;
    ready = false;
    relocatedLabel = "";
    currentHref = "";
    epubOrigHtml.clear();
    epubTransHtml.clear();
    onReaderApi?.(null);
    rendition?.destroy();
    book?.destroy();
    rendition = null;
    book = null;
    try {
      const b64 = await invoke<string>("read_book_base64", { relativePath: path });
      if (sid !== session) return;
      const bin = atob(b64);
      const bytes = new Uint8Array(bin.length);
      for (let i = 0; i < bin.length; i++) bytes[i] = bin.charCodeAt(i);
      const ePub = (await import("epubjs")).default;
      book = ePub(bytes.buffer);
      await book.ready;
      if (sid !== session) return;
      rendition = book.renderTo(el, {
        width: "100%",
        height: "100%",
        flow: "paginated",
        spread: "none",
      });
      await rendition.display();
      if (sid !== session) return;

      if (openAt.trim()) {
        try {
          await rendition.display(openAt.trim());
        } catch {
          /* остаёмся на первой странице */
        }
        if (sid !== session) return;
      }

      rendition.hooks.content.register(translationContentHook);

      rendition.on?.("relocated", (loc: any) => {
        const href = loc?.start?.href || loc?.end?.href || "";
        if (href) currentHref = href;
        let label = "";
        if (href) {
          try {
            label = decodeURIComponent(href.split("/").pop() || href);
          } catch {
            label = href.split("/").pop() || href;
          }
        }
        relocatedLabel = label;
        if (href) {
          onReadingProgress?.({ location: href, label });
        }
      });

      let toc: { label: string; href: string }[] = [];
      try {
        await book.loaded.navigation;
        toc = flattenToc(book.navigation?.toc || []);
      } catch {
        toc = [];
      }
      const spine = buildSpineList(book);

      const api: EpubReaderApi = {
        toc,
        spine,
        goTo: async (href: string) => {
          await rendition?.display(href);
        },
        prev: async () => {
          await rendition?.prev?.();
        },
        next: async () => {
          await rendition?.next?.();
        },
      };
      onReaderApi?.(api);
      ready = true;
    } catch (e) {
      if (sid === session) err = String(e);
      onReaderApi?.(null);
    } finally {
      if (sid === session) loading = false;
    }
  }

  $effect(() => {
    const path = relativePath;
    const el = host;
    const openAt = initialLocation ?? "";
    if (!el) return;
    const sid = ++session;
    void mountBook(path, el, sid, openAt);
    return () => {
      session += 1;
      onReaderApi?.(null);
      rendition?.destroy();
      book?.destroy();
      rendition = null;
      book = null;
    };
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
    const r = rendition;
    const href = currentHref;
    if (!r || !href || !ready) return;
    void r.display(href);
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

  async function doCopy() {
    const t = selectedText.trim();
    if (!t) return;
    try {
      await writeTextToClipboard(t);
    } catch {
      /* ignore */
    }
    toolbarVisible = false;
    if (host?.querySelector("iframe")?.contentWindow) {
      host.querySelector("iframe")?.contentWindow?.getSelection()?.removeAllRanges();
    }
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
    try {
      host?.querySelector("iframe")?.contentWindow?.getSelection()?.removeAllRanges();
    } catch {
      /* ignore */
    }
  }

  function saveQuote(opts: QuoteDraftOptions) {
    onAddQuote?.({ text: selectedText.trim(), options: opts });
    try {
      host?.querySelector("iframe")?.contentWindow?.getSelection()?.removeAllRanges();
    } catch {
      /* ignore */
    }
  }

  const pageHintStr = $derived(
    relocatedLabel ? `Фрагмент: ${relocatedLabel}` : "Электронная книга (EPUB)",
  );
</script>

<div class="epub-root" class:reader={variant === "reader"}>
  <div class="stage-wrap">
    <div class="stage" bind:this={host}></div>
    {#if loading}
      <div class="overlay"><span class="hint">Загрузка EPUB…</span></div>
    {/if}
    {#if err}
      <div class="overlay"><span class="error">{err}</span></div>
    {/if}
  </div>
  {#if variant === "full" && ready && !err}
    <footer class="toolbar">
      <button type="button" onclick={() => void prev()}>← Назад</button>
      <button type="button" onclick={() => void next()}>Вперёд →</button>
    </footer>
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
  .epub-root {
    display: flex;
    flex-direction: column;
    height: 100%;
    min-height: 0;
    background: var(--reader-bg);
    position: relative;
  }
  .epub-root.reader {
    background: var(--elevated-soft);
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
    background: color-mix(in srgb, var(--reader-bg) 88%, transparent);
    z-index: 2;
    border-radius: var(--radius-md);
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
    min-height: 0;
    overflow: hidden;
    padding: 0.5rem 1rem 0.75rem;
  }
  .stage :global(iframe) {
    border: none !important;
    border-radius: var(--radius-sm) !important;
    box-shadow: var(--shadow-book) !important;
    background: #faf8f3 !important;
  }
  .toolbar {
    display: flex;
    gap: 0.75rem;
    padding: 0.55rem 1rem;
    border-top: 1px solid var(--border-soft);
    background: var(--panel-soft);
    flex-shrink: 0;
  }
  .toolbar button {
    padding: 0.4rem 0.85rem;
    border-radius: 10px;
    border: 1px solid var(--border-soft);
    background: var(--elevated-soft);
    color: var(--text);
    cursor: pointer;
  }
</style>
