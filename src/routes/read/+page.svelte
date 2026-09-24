<script lang="ts">
  import { goto } from "$app/navigation";
  import { page } from "$app/state";
  import { invoke } from "@tauri-apps/api/core";
  import type {
    BookComment,
    BookMeta,
    EpubReaderApi,
    LibraryMetadata,
    LibrarySnapshot,
    PdfOutlineItem,
    PdfReadyInfo,
    QuoteAccent,
    QuoteDraftOptions,
    ReaderOutlineItem,
    SavedQuote,
  } from "$lib/types";
  import PdfViewer from "$lib/components/PdfViewer.svelte";
  import EpubViewer from "$lib/components/EpubViewer.svelte";
  import Fb2Viewer from "$lib/components/Fb2Viewer.svelte";
  import TypstViewer from "$lib/components/TypstViewer.svelte";
  import TranslationBar from "$lib/components/TranslationBar.svelte";
  import ReaderOutlineTree from "$lib/components/ReaderOutlineTree.svelte";
  import { ACCENT_CHIP_BG } from "$lib/quoteCardStyles";
  import { getBookFormat } from "$lib/bookFormat";
  import { loadPdfBookTranslationFile } from "$lib/translate/pdfFullBookJob";
  import { exportTranslatedPdfToLibrary } from "$lib/pdf/exportTranslatedPdf";
  import { exportBookToTypst } from "$lib/typst/exportToTypst";
  import {
    fetchLibrarySnapshot,
    getCachedLibrarySnapshot,
    setCachedLibraryMetadata,
  } from "$lib/library/librarySnapshotCache";
  import type { TypstOutlineItem } from "$lib/typst/outlineTypst";
  import { toastError } from "$lib/ui/toast.svelte";

  const QUOTE_ACCENTS: QuoteAccent[] = ["sand", "sage", "dustyRose", "ink"];

  function quoteAccent(raw: string): QuoteAccent {
    return QUOTE_ACCENTS.includes(raw as QuoteAccent) ? (raw as QuoteAccent) : "sand";
  }

  function quoteDetailLine(q: SavedQuote): string {
    const parts: string[] = [];
    if (q.includePage) parts.push("страница");
    if (q.includeChapter) parts.push("глава");
    parts.push(String(q.layout));
    return parts.join(" · ");
  }

  const bookPath = $derived.by(() => {
    const raw = page.url.searchParams.get("path");
    if (!raw) return "";
    try {
      return decodeURIComponent(raw);
    } catch {
      return "";
    }
  });

  let snapshot = $state<LibrarySnapshot | null>(getCachedLibrarySnapshot());
  let pdfOutline = $state<PdfOutlineItem[]>([]);
  let pdfNumPages = $state(0);
  let navApi = $state<EpubReaderApi | null>(null);
  let pdfPage = $state(1);
  let pdfScale = $state(1);
  let tab = $state<"chapters" | "pages" | "notes" | "translate">("chapters");
  let reviewDraft = $state("");
  let saveTimer: ReturnType<typeof setTimeout> | null = null;
  let notesPath = $state<string | null>(null);
  let pdfProgressKey = $state("");

  let transSource = $state("auto");
  let transTarget = $state("ru");
  let transLayout = $state<"orig" | "trans" | "split">("orig");
  let transBusy = $state(false);
  let transErr = $state<string | null>(null);
  let transRunKey = $state(0);
  let hasFb2Translation = $state(false);
  /** Полный перевод PDF (LM/Ollama), слой поверх скана */
  let pdfInlineSpans = $state<Record<string, string[]> | null>(null);
  /** Превью перевода в слое (грубее, чем экспорт PDF). Вкл., чтобы сразу видеть полный перевод. */
  let pdfInlineShow = $state(true);
  let pdfExportBusy = $state(false);
  let typstExportBusy = $state(false);
  let typstExportBanner = $state<string | null>(null);
  let typstOutline = $state<TypstOutlineItem[]>([]);
  let typstJumpLine = $state<number | null>(null);
  let readerPanelOpen = $state(false);

  function handleTranslateActivity(e: { busy: boolean; error: string | null }) {
    transBusy = e.busy;
    transErr = e.error;
    if (!e.busy && !e.error && bookPath && getBookFormat(bookPath) === "fb2") {
      hasFb2Translation = true;
    }
  }

  const meta = $derived(
    bookPath && snapshot?.metadata.books[bookPath]
      ? snapshot.metadata.books[bookPath]
      : null,
  );

  const displayTitle = $derived(
    bookPath
      ? (meta?.title?.trim() || titleFromPath(bookPath))
      : "",
  );
  const displayAuthor = $derived(meta?.author?.trim() ?? "");

  const pdfTreeItems = $derived<ReaderOutlineItem[]>(
    pdfOutline.map((item, index) => ({
      id: `pdf-${index}`,
      label: item.title,
      level: item.level,
      meta: item.page == null ? undefined : `стр. ${item.page}`,
      disabled: item.page == null,
    })),
  );

  const activePdfOutlineIndex = $derived.by(() => {
    let found = -1;
    let foundPage = -1;
    for (let index = 0; index < pdfOutline.length; index++) {
      const page = pdfOutline[index]?.page;
      if (page != null && page <= pdfPage && page > foundPage) {
        found = index;
        foundPage = page;
      }
    }
    return found;
  });

  const activePdfTreeId = $derived(activePdfOutlineIndex >= 0 ? `pdf-${activePdfOutlineIndex}` : null);
  const activePdfChapterLabel = $derived(
    activePdfOutlineIndex >= 0 ? (pdfOutline[activePdfOutlineIndex]?.title ?? "") : "",
  );

  const typstTreeItems = $derived<ReaderOutlineItem[]>(
    typstOutline.map((item, index) => ({
      id: `typst-${index}`,
      label: item.title,
      level: item.level,
      meta: `строка ${item.line}`,
    })),
  );

  const navTreeItems = $derived<ReaderOutlineItem[]>(
    (navApi?.toc ?? []).map((item, index) => ({
      id: `nav-${index}`,
      label: item.label,
      level: item.level ?? 0,
    })),
  );

  const activeNavTreeId = $derived.by(() => {
    const location = meta?.lastReadLocation?.trim();
    if (!location || !navApi) return null;
    const locationBase = location.split("#")[0];
    const index = navApi.toc.findIndex(
      (item) => item.href === location || item.href.split("#")[0] === locationBase,
    );
    return index >= 0 ? `nav-${index}` : null;
  });

  function finishChapterNavigation() {
    if (window.matchMedia("(max-width: 900px)").matches) readerPanelOpen = false;
  }

  function selectPdfChapter(id: string) {
    const item = pdfOutline[Number(id.replace("pdf-", ""))];
    if (item?.page != null) pdfPage = item.page;
    finishChapterNavigation();
  }

  function selectTypstChapter(id: string) {
    const item = typstOutline[Number(id.replace("typst-", ""))];
    if (item) typstJumpLine = item.line;
    finishChapterNavigation();
  }

  function selectNavChapter(id: string) {
    const item = navApi?.toc[Number(id.replace("nav-", ""))];
    if (item) void navApi?.goTo(item.href);
    finishChapterNavigation();
  }

  const storageFootnote = $derived.by(() => {
    const root = snapshot?.libraryRoot?.trim();
    if (root)
      return `Корень библиотеки на диске: ${root}. Файл метаданных лежит рядом с каталогом приложения (часто ~/.config/com.adaman.reader/library-metadata.json).`;
    return "Файл library-metadata.json в каталоге настроек приложения (на Linux обычно ~/.config/com.adaman.reader/).";
  });

  function titleFromPath(path: string) {
    const i = path.lastIndexOf("/");
    return i >= 0 ? path.slice(i + 1) : path;
  }

  async function exportCurrentBookToTypst() {
    if (!bookPath) return;
    const fmt = getBookFormat(bookPath);
    if (!fmt || fmt === "typst") return;
    typstExportBusy = true;
    typstExportBanner = null;
    try {
      if (!snapshot) throw new Error("Сначала откройте библиотеку.");
      const { mainRelativePath } = await exportBookToTypst(
        bookPath,
        fmt,
        snapshot,
        meta?.typstStyleRelativePath,
      );
      await loadSnapshot();
      typstExportBanner = `Создан проект Typst: ${mainRelativePath}`;
      await goto("/read?path=" + encodeURIComponent(mainRelativePath));
    } catch (e) {
      typstExportBanner = e instanceof Error ? e.message : String(e);
    } finally {
      typstExportBusy = false;
    }
  }

  async function exportPdfTranslation() {
    if (!bookPath || getBookFormat(bookPath) !== "pdf") return;
    pdfExportBusy = true;
    try {
      await exportTranslatedPdfToLibrary(bookPath);
      await loadSnapshot();
    } catch (e) {
      toastError(e, "Экспорт перевода");
    } finally {
      pdfExportBusy = false;
    }
  }

  async function loadSnapshot() {
    try {
      snapshot = await fetchLibrarySnapshot();
    } catch {
      snapshot = null;
    }
  }

  async function persist(next: LibraryMetadata) {
    await invoke("save_library_metadata", { metadata: next });
    setCachedLibraryMetadata(next);
    if (snapshot) snapshot = { ...snapshot, metadata: next };
  }

  function patchBook(path: string, patch: Partial<BookMeta>) {
    if (!snapshot) return;
    const cur = snapshot.metadata.books[path];
    if (!cur) return;
    const books = { ...snapshot.metadata.books, [path]: { ...cur, ...patch } };
    void persist({ ...snapshot.metadata, books });
  }

  /** Сбрасываем при уходе со страницы чтения, чтобы при следующем открытии той же книги снова записать время. */
  let lastOpenedPatchKey = $state("");

  $effect(() => {
    if (!bookPath) lastOpenedPatchKey = "";
  });

  $effect(() => {
    const p = bookPath;
    const snap = snapshot;
    if (!p || !snap?.metadata.books[p]) return;
    if (p === lastOpenedPatchKey) return;
    lastOpenedPatchKey = p;
    patchBook(p, { lastOpenedAt: new Date().toISOString() });
  });

  function scheduleReviewSave(path: string, text: string) {
    if (saveTimer) clearTimeout(saveTimer);
    saveTimer = setTimeout(() => {
      patchBook(path, { review: text });
      saveTimer = null;
    }, 450);
  }

  function onPdfReady(info: PdfReadyInfo) {
    pdfOutline = info.outline;
    pdfNumPages = info.numPages;
  }

  function onReaderNavApi(api: EpubReaderApi | null) {
    navApi = api;
  }

  let locSaveTimer: ReturnType<typeof setTimeout> | null = null;
  function onReadingLocationSave(p: { location: string; label: string }) {
    if (!bookPath) return;
    if (locSaveTimer) clearTimeout(locSaveTimer);
    locSaveTimer = setTimeout(() => {
      patchBook(bookPath, {
        lastReadLocation: p.location,
        lastReadLocationLabel: p.label,
      });
      locSaveTimer = null;
    }, 500);
  }

  /** Восстановить страницу PDF после известного числа страниц */
  $effect(() => {
    bookPath;
    pdfProgressKey = "";
  });

  $effect(() => {
    const path = bookPath;
    const snap = snapshot;
    if (!path || !snap || getBookFormat(path) !== "pdf") return;
    if (pdfNumPages <= 0) return;
    const key = `${path}:${pdfNumPages}`;
    if (pdfProgressKey === key) return;
    const pg = snap.metadata.books[path]?.lastReadPdfPage;
    if (pg != null && pg >= 1) {
      pdfPage = Math.min(Math.max(1, Math.round(pg)), pdfNumPages);
    }
    pdfProgressKey = key;
  });

  let pdfProgTimer: ReturnType<typeof setTimeout> | null = null;
  $effect(() => {
    const path = bookPath;
    const fmt = path ? getBookFormat(path) : null;
    const total = pdfNumPages;
    const page = pdfPage;
    if (!path || !snapshot || fmt !== "pdf" || total <= 0) return;
    if (pdfProgTimer) clearTimeout(pdfProgTimer);
    pdfProgTimer = setTimeout(() => {
      patchBook(path, { lastReadPdfPage: page, lastReadPdfTotal: total });
      pdfProgTimer = null;
    }, 450);
    return () => {
      if (pdfProgTimer) clearTimeout(pdfProgTimer);
    };
  });

  function addComment(payload: {
    body: string;
    excerpt: string;
    page?: number;
    chapterLabel?: string;
  }) {
    if (!snapshot || !bookPath) return;
    const cur = snapshot.metadata.books[bookPath];
    if (!cur) return;
    const com: BookComment = {
      id: crypto.randomUUID(),
      body: payload.body,
      excerpt: payload.excerpt,
      createdAt: new Date().toISOString(),
    };
    if (payload.page != null) com.page = payload.page;
    if (payload.chapterLabel) com.chapterLabel = payload.chapterLabel;
    const comments = [...(cur.comments ?? []), com];
    patchBook(bookPath, { comments });
  }

  function addQuote(payload: { text: string; options: QuoteDraftOptions }) {
    if (!snapshot || !bookPath) return;
    const cur = snapshot.metadata.books[bookPath];
    if (!cur) return;
    const o = payload.options;
    const quote: SavedQuote = {
      id: crypto.randomUUID(),
      text: payload.text,
      createdAt: new Date().toISOString(),
      accent: o.accent,
      layout: o.layout,
      includePage: o.includePage,
      includeChapter: o.includeChapter,
    };
    const bt = o.bookTitle.trim();
    const ba = o.bookAuthor.trim();
    if (bt) quote.bookTitle = bt;
    if (ba) quote.bookAuthor = ba;
    if (o.bgImageDataUrl) quote.bgImageDataUrl = o.bgImageDataUrl;
    quote.bgImageOpacity = o.bgImageOpacity;
    quote.overlayColor = o.overlayColor;
    quote.overlayOpacity = o.overlayOpacity;
    quote.bgScale = o.bgScale;
    quote.bgFit = o.bgFit;
    const quotes = [...(cur.quotes ?? []), quote];
    patchBook(bookPath, { quotes });
  }


  $effect(() => {
    const path = bookPath;
    if (!snapshot || (path && !snapshot.metadata.books[path])) void loadSnapshot();
  });

  $effect(() => {
    const p = bookPath;
    const fmt = p ? getBookFormat(p) : null;
    if (!p || fmt !== "typst") {
      typstOutline = [];
      typstJumpLine = null;
    }
  });

  $effect(() => {
    const p = bookPath;
    if (p && snapshot && notesPath !== p) {
      notesPath = p;
      reviewDraft = snapshot.metadata.books[p]?.review ?? "";
    }
    if (!p) notesPath = null;
  });

  $effect(() => {
    const p = bookPath;
    if (!getBookFormat(p)) {
      goto("/");
    }
  });

  $effect(() => {
    bookPath;
    transRunKey = 0;
    transLayout = "orig";
    transErr = null;
    hasFb2Translation = false;
  });

  $effect(() => {
    const p = bookPath;
    void (async () => {
      if (!p || getBookFormat(p) !== "pdf") {
        pdfInlineSpans = null;
        return;
      }
      try {
        const doc = await loadPdfBookTranslationFile(p);
        pdfInlineSpans =
          doc?.pages && Object.keys(doc.pages).length > 0 ? doc.pages : null;
      } catch {
        pdfInlineSpans = null;
      }
    })();
  });
</script>

{#if bookPath && getBookFormat(bookPath)}
  {@const fmt = getBookFormat(bookPath)!}
  <div class="read-shell">
    <header class="read-top">
      <div class="read-top-left">
        <button type="button" class="back" onclick={() => goto("/")}>
          <span class="back-arr" aria-hidden="true">←</span>
          <span>Библиотека</span>
        </button>
        <a href="/settings" class="settings-link">Настройки</a>
      </div>
      <div class="title-block">
        <h1 class="read-title">{displayTitle}</h1>
        {#if displayAuthor}
          <p class="read-author">{displayAuthor}</p>
        {/if}
        {#if typstExportBanner}
          <p class="typst-export-banner" role="status">{typstExportBanner}</p>
        {/if}
      </div>
      <button
        type="button"
        class="reader-panel-toggle"
        aria-expanded={readerPanelOpen}
        onclick={() => (readerPanelOpen = !readerPanelOpen)}
      >
        <svg viewBox="0 0 24 24" aria-hidden="true"><path d="M4 5.5h16M4 12h16M4 18.5h10" /></svg>
        <span>Навигация</span>
      </button>
      {#if fmt === "pdf" || fmt === "epub" || fmt === "fb2"}
        <button
          type="button"
          class="typst-export-top"
          onclick={() => void exportCurrentBookToTypst()}
          disabled={typstExportBusy}
        >
          {typstExportBusy ? "Экспорт в Typst…" : "В Typst"}
        </button>
      {/if}
      {#if (fmt === "epub" || fmt === "fb2") && navApi}
        {@const api = navApi}
        <div class="epub-nav">
          <button type="button" class="mini" onclick={() => void api.prev()} title="Назад">‹</button>
          <button type="button" class="mini" onclick={() => void api.next()} title="Вперёд">›</button>
        </div>
      {/if}
    </header>

    <div class="read-body">
      <aside class="tabs-panel" class:tabs-panel-open={readerPanelOpen}>
        <div class="panel-head-mobile">
          <span>Навигация по книге</span>
          <button type="button" aria-label="Закрыть навигацию" onclick={() => (readerPanelOpen = false)}>×</button>
        </div>
        <div class="tab-row">
          <button
            type="button"
            class="tab"
            class:active={tab === "chapters"}
            onclick={() => (tab = "chapters")}>Главы</button>
          <button
            type="button"
            class="tab"
            class:active={tab === "pages"}
            onclick={() => (tab = "pages")}>Страницы</button>
          <button
            type="button"
            class="tab"
            class:active={tab === "translate"}
            onclick={() => (tab = "translate")}>Перевод</button>
          <button
            type="button"
            class="tab"
            class:active={tab === "notes"}
            onclick={() => (tab = "notes")}>Заметки</button>
        </div>

        <div class="tab-body">
          {#if tab === "chapters"}
            <div class="scroll-list">
              {#if fmt === "pdf"}
                {#if pdfOutline.length === 0}
                  <p class="empty-hint">Оглавление не найдено или недоступно.</p>
                {:else}
                  <ReaderOutlineTree
                    items={pdfTreeItems}
                    activeId={activePdfTreeId}
                    onSelect={selectPdfChapter}
                  />
                {/if}
              {:else if fmt === "typst"}
                {#if typstOutline.length === 0}
                  <p class="empty-hint">
                    Заголовки из <code class="code-in-hint">=</code>, <code class="code-in-hint">==</code>… появятся здесь
                    после загрузки <strong>book.typ</strong>.
                  </p>
                {:else}
                  <ReaderOutlineTree items={typstTreeItems} onSelect={selectTypstChapter} />
                {/if}
              {:else if navApi}
                {#if navApi.toc.length === 0}
                  <p class="empty-hint">Оглавление пустое.</p>
                {:else}
                  <ReaderOutlineTree
                    items={navTreeItems}
                    activeId={activeNavTreeId}
                    onSelect={selectNavChapter}
                  />
                {/if}
              {:else}
                <p class="empty-hint">Загрузка…</p>
              {/if}
            </div>
          {:else if tab === "pages"}
            <div class="scroll-list">
              {#if fmt === "pdf"}
                {#if pdfNumPages <= 0}
                  <p class="empty-hint">Загрузка…</p>
                {:else}
                  <div class="page-grid">
                    {#each Array.from({ length: pdfNumPages }, (_, i) => i + 1) as n (n)}
                      <button
                        type="button"
                        class="page-cell"
                        class:cur={pdfPage === n}
                        onclick={() => (pdfPage = n)}>{n}</button>
                    {/each}
                  </div>
                {/if}
              {:else if fmt === "typst"}
                <p class="empty-hint typst-pages-hint">
                  Быстрый предпросмотр — первые ~20 стр.; полная книга — кнопка «Показать всю книгу» на вкладке «Читать».
                  Поля и разрывы — в <strong>theme.typ</strong> и <code>#pagebreak</code>.
                </p>
              {:else if navApi}
                {#if navApi.spine.length === 0}
                  <p class="empty-hint">Нет сегментов.</p>
                {:else}
                  {#each navApi.spine as seg (seg.href)}
                    <button type="button" class="list-btn" onclick={() => void navApi?.goTo(seg.href)}>
                      {seg.label}
                    </button>
                  {/each}
                {/if}
              {:else}
                <p class="empty-hint">Загрузка…</p>
              {/if}
            </div>
          {:else if tab === "translate"}
            <div class="translate-tab">
              {#if fmt === "typst"}
                <p class="empty-hint">
                  Встроенный перевод для Typst не подключён — редактируйте текст в исходнике или экспортируйте снова из
                  PDF/EPUB/FB2 с нужным языком.
                </p>
              {:else}
                <TranslationBar
                  compact
                  format={fmt}
                  sourceLang={transSource}
                  targetLang={transTarget}
                  layout={transLayout}
                  busy={transBusy}
                  error={transErr}
                  hasTranslation={hasFb2Translation}
                  onSourceChange={(v) => (transSource = v)}
                  onTargetChange={(v) => (transTarget = v)}
                  onLayoutChange={(v) => (transLayout = v)}
                  onRunTranslate={() => {
                    transRunKey += 1;
                  }}
                />
                {#if fmt === "pdf" && pdfInlineSpans && Object.keys(pdfInlineSpans).length > 0}
                  <div class="pdf-inline-bar translate-inline">
                    <button
                      type="button"
                      class="pdf-export-btn"
                      onclick={() => void exportPdfTranslation()}
                      disabled={pdfExportBusy}>{pdfExportBusy ? "Сборка PDF…" : "Сохранить translate_*.pdf в папку книги"}</button>
                    <label class="pdf-inline-lab">
                      <input type="checkbox" bind:checked={pdfInlineShow} />
                      Превью слоя (грубо)
                    </label>
                  </div>
                {/if}
              {/if}
            </div>
          {:else}
            <div class="notes-tab">
              {#if meta}
                <details class="storage-details">
                  <summary>Где хранятся заметки и цитаты</summary>
                  <div class="storage-body">
                    <p>{storageFootnote}</p>
                    <p class="storage-second">
                      Комментарии к выделению и цитаты из «Сделать цитату» попадают туда же.
                    </p>
                  </div>
                </details>

                <p class="notes-hint">
                  Полку, важность и рецензию — на главной: правая кнопка по книге → «Редактирование».
                </p>

                {#if (meta.comments?.length ?? 0) > 0}
                  <section class="mini-block">
                    <h4>Комментарии к отрывкам</h4>
                    <ul class="cmt-list">
                      {#each meta.comments ?? [] as c (c.id)}
                        <li class="cmt-item">
                          {#if c.page != null}
                            <span class="cmt-meta">стр. {c.page}</span>
                          {:else if c.chapterLabel}
                            <span class="cmt-meta">{c.chapterLabel}</span>
                          {/if}
                          {#if c.excerpt}
                            <p class="cmt-ex">{c.excerpt}</p>
                          {/if}
                          <p class="cmt-body">{c.body}</p>
                        </li>
                      {/each}
                    </ul>
                  </section>
                {/if}

                {#if (meta.quotes?.length ?? 0) > 0}
                  <section class="mini-block">
                    <h4>Сохранённые цитаты</h4>
                    <p class="quotes-lead">Список ниже — это те же данные, что в файле метаданных; их можно копировать как PNG из диалога при создании цитаты.</p>
                    <ul class="quote-card-list">
                      {#each meta.quotes ?? [] as q (q.id)}
                        {@const ac = quoteAccent(String(q.accent))}
                        {@const hasBg = !!(q.bgImageDataUrl && q.bgImageDataUrl.length > 12)}
                        <li class="quote-card" class:with-bg={hasBg} style:--stripe={ACCENT_CHIP_BG[ac]}>
                          {#if hasBg}
                            <div class="quote-card-media" aria-hidden="true">
                              <div
                                class="quote-card-media-img"
                                style:background-image={`url(${q.bgImageDataUrl})`}
                                style:opacity={q.bgImageOpacity ?? 1}
                                style:background-size={(q.bgFit === "contain" ? "contain" : "cover")}
                                style:transform={`scale(${(q.bgScale ?? 100) / 100})`}
                              ></div>
                              {#if (q.overlayOpacity ?? 0) > 0.02}
                                <div
                                  class="quote-card-media-tint"
                                  style:background-color={q.overlayColor ?? "#1a1510"}
                                  style:opacity={q.overlayOpacity ?? 0}
                                ></div>
                              {/if}
                            </div>
                          {/if}
                          <div class="quote-card-body">
                            <p class="quote-card-text">
                              «{q.text.length > 320 ? `${q.text.slice(0, 320)}…` : q.text}»
                            </p>
                            <p class="quote-card-by">
                              {q.bookTitle?.trim() || displayTitle}
                              {#if q.bookAuthor?.trim() || displayAuthor}
                                <span class="quote-card-auth">
                                  — {q.bookAuthor?.trim() || displayAuthor}
                                </span>
                              {/if}
                            </p>
                            <p class="quote-card-meta">{quoteDetailLine(q)}</p>
                          </div>
                        </li>
                      {/each}
                    </ul>
                  </section>
                {/if}

                <label class="notes-label">
                  <span>Заметки к книге</span>
                  <textarea
                    rows="14"
                    placeholder="Мысли, цитаты, заметки…"
                    bind:value={reviewDraft}
                    oninput={() => scheduleReviewSave(bookPath, reviewDraft)}
                  ></textarea>
                </label>
              {:else}
                <p class="empty-hint">Метаданные загружаются…</p>
              {/if}
            </div>
          {/if}
        </div>
      </aside>

      <button
        type="button"
        class="reader-backdrop"
        class:reader-backdrop-open={readerPanelOpen}
        aria-label="Закрыть навигацию"
        onclick={() => (readerPanelOpen = false)}
      ></button>

      <section class="read-stage">
        <div class="stage-frame">
          {#if fmt === "pdf"}
            <PdfViewer
              relativePath={bookPath}
              variant="reader"
              bind:pageNum={pdfPage}
              bind:scale={pdfScale}
              onPdfReady={onPdfReady}
              displayTitle={displayTitle}
              displayAuthor={displayAuthor}
              chapterLabel={activePdfChapterLabel}
              onAddComment={addComment}
              onAddQuote={addQuote}
              pdfTranslationPanel={transLayout === "trans"}
              translateRunKey={transRunKey}
              translateSource={transSource}
              translateTarget={transTarget}
              onTranslateActivity={handleTranslateActivity}
              pdfInlineSpans={pdfInlineSpans}
              pdfInlineShow={pdfInlineShow}
            />
          {:else if fmt === "epub"}
            <EpubViewer
              relativePath={bookPath}
              variant="reader"
              initialLocation={meta?.lastReadLocation ?? ""}
              onReadingProgress={onReadingLocationSave}
              onReaderApi={onReaderNavApi}
              displayTitle={displayTitle}
              displayAuthor={displayAuthor}
              onAddComment={addComment}
              onAddQuote={addQuote}
              translateRunKey={transRunKey}
              translateSource={transSource}
              translateTarget={transTarget}
              translateLayout={transLayout === "split" ? "trans" : transLayout}
              onTranslateActivity={handleTranslateActivity}
            />
          {:else if fmt === "typst"}
            <TypstViewer
              relativePath={bookPath}
              displayTitle={displayTitle}
              onOutline={(o) => (typstOutline = o)}
              goToLineRequest={typstJumpLine}
              onGoToLineHandled={() => (typstJumpLine = null)}
            />
          {:else}
            <Fb2Viewer
              relativePath={bookPath}
              variant="reader"
              initialLocation={meta?.lastReadLocation ?? ""}
              onReadingProgress={onReadingLocationSave}
              onReaderApi={onReaderNavApi}
              displayTitle={displayTitle}
              displayAuthor={displayAuthor}
              onAddComment={addComment}
              onAddQuote={addQuote}
              translateRunKey={transRunKey}
              translateSource={transSource}
              translateTarget={transTarget}
              translateLayout={transLayout}
              onTranslateActivity={handleTranslateActivity}
            />
          {/if}
        </div>
      </section>
    </div>
  </div>
{/if}

<style>
  .read-shell {
    display: flex;
    flex-direction: column;
    height: 100vh;
    height: 100dvh;
    min-height: 0;
    position: relative;
    isolation: isolate;
    background:
      radial-gradient(circle at 95% 0%, color-mix(in srgb, var(--accent) 10%, transparent), transparent 28rem),
      var(--reader-bg);
  }

  .translate-tab {
    display: flex;
    flex-direction: column;
    gap: 0.45rem;
    min-height: 0;
    padding-bottom: 0.35rem;
  }

  .pdf-inline-bar.translate-inline {
    border-bottom: none;
    margin: 0;
  }

  .pdf-inline-bar {
    flex-shrink: 0;
    padding: 0.35rem clamp(0.65rem, 2vw, 1rem);
    font-size: 0.84rem;
    color: var(--muted);
    border-bottom: 1px solid var(--border-soft);
    background: color-mix(in srgb, var(--accent) 4%, var(--elevated-soft));
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 0.65rem 1rem;
  }

  .pdf-export-btn {
    padding: 0.35rem 0.75rem;
    border-radius: var(--radius-sm);
    border: 1px solid color-mix(in srgb, var(--accent) 45%, var(--border-soft));
    background: color-mix(in srgb, var(--accent) 14%, var(--elevated-soft));
    color: var(--text-soft);
    font-size: 0.82rem;
    font-weight: 600;
    cursor: pointer;
    font-family: system-ui, sans-serif;
  }

  .pdf-export-btn:disabled {
    opacity: 0.55;
    cursor: default;
  }

  .pdf-inline-lab {
    display: inline-flex;
    align-items: center;
    gap: 0.45rem;
    cursor: pointer;
    user-select: none;
  }

  .read-top {
    display: flex;
    align-items: center;
    gap: 0.65rem 1rem;
    min-height: 4.2rem;
    padding: 0.6rem clamp(0.75rem, 3vw, 1.3rem);
    border-bottom: 1px solid color-mix(in srgb, var(--border-soft) 82%, transparent);
    background: color-mix(in srgb, var(--panel-veil) 96%, transparent);
    backdrop-filter: blur(16px) saturate(1.15);
    box-shadow: 0 1px 0 color-mix(in srgb, var(--elevated-soft) 65%, transparent);
    flex-shrink: 0;
    flex-wrap: wrap;
    row-gap: 0.45rem;
  }

  .read-top-left {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 0.35rem 0.75rem;
    flex-shrink: 0;
  }

  .settings-link {
    font-size: 0.83rem;
    color: var(--muted);
    text-decoration: none;
    font-weight: 550;
    padding: 0.45rem 0.65rem;
    border-radius: var(--radius-sm);
    font-family: system-ui, sans-serif;
  }

  .settings-link:hover {
    color: var(--accent-2);
    background: color-mix(in srgb, var(--accent) 8%, transparent);
  }

  .back {
    display: inline-flex;
    align-items: center;
    gap: 0.35rem;
    border: none;
    background: transparent;
    color: var(--accent-2);
    min-height: 2.35rem;
    padding: 0.45rem 0.65rem;
    margin-left: -0.35rem;
    border-radius: var(--radius-sm);
    cursor: pointer;
    font-size: 0.83rem;
    font-weight: 550;
    flex-shrink: 0;
    font-family: system-ui, sans-serif;
  }

  .back-arr {
    opacity: 0.85;
    font-size: 1rem;
  }

  .back:hover {
    background: color-mix(in srgb, var(--accent) 10%, transparent);
    color: var(--text-soft);
  }

  .title-block {
    flex: 1 1 12rem;
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: 0.12rem;
    padding-left: clamp(0rem, 1vw, 0.65rem);
  }

  .read-title {
    margin: 0;
    font-size: clamp(1rem, 3.5vw, 1.18rem);
    font-weight: 750;
    letter-spacing: -0.02em;
    color: var(--text-soft);
    line-height: 1.25;
    overflow: hidden;
    display: -webkit-box;
    -webkit-box-orient: vertical;
    -webkit-line-clamp: 2;
    line-clamp: 2;
  }

  .read-author {
    margin: 0;
    font-size: clamp(0.74rem, 2.5vw, 0.8rem);
    color: var(--muted);
    overflow: hidden;
    display: -webkit-box;
    -webkit-box-orient: vertical;
    -webkit-line-clamp: 1;
    line-clamp: 1;
    font-family: system-ui, sans-serif;
  }

  .typst-export-banner {
    margin: 0.25rem 0 0;
    font-size: 0.78rem;
    color: var(--muted);
    line-height: 1.35;
    max-width: 40rem;
  }

  .typst-export-top {
    flex-shrink: 0;
    align-self: center;
    min-height: 2.35rem;
    padding: 0.45rem 0.9rem;
    border-radius: 999px;
    border: 1px solid color-mix(in srgb, var(--accent) 40%, var(--border-soft));
    background: color-mix(in srgb, var(--accent) 10%, var(--elevated-soft));
    color: var(--text-soft);
    font-size: 0.82rem;
    font-weight: 600;
    cursor: pointer;
    font-family: system-ui, sans-serif;
  }

  .typst-export-top:disabled {
    opacity: 0.55;
    cursor: default;
  }

  .code-in-hint {
    font-size: 0.9em;
    font-family: ui-monospace, monospace;
  }

  .epub-nav {
    display: flex;
    gap: 0.35rem;
    flex-shrink: 0;
  }

  .reader-panel-toggle {
    display: none;
    align-items: center;
    justify-content: center;
    gap: 0.4rem;
    min-height: 2.35rem;
    padding: 0.45rem 0.75rem;
    border: 1px solid var(--border-soft);
    border-radius: 999px;
    background: var(--elevated-soft);
    color: var(--text-soft);
    cursor: pointer;
    font-size: 0.8rem;
    font-weight: 650;
  }

  .reader-panel-toggle svg {
    width: 1rem;
    height: 1rem;
    fill: none;
    stroke: currentColor;
    stroke-width: 1.8;
    stroke-linecap: round;
  }

  .panel-head-mobile,
  .reader-backdrop {
    display: none;
  }

  .mini {
    width: 2.25rem;
    height: 2.25rem;
    border-radius: var(--radius-md);
    border: 1px solid var(--border-soft);
    background: var(--panel-soft);
    color: var(--text);
    cursor: pointer;
    font-size: 1.15rem;
    line-height: 1;
  }

  .mini:hover {
    border-color: color-mix(in srgb, var(--accent) 40%, var(--border-soft));
    background: var(--elevated-soft);
  }

  .read-body {
    flex: 1;
    display: grid;
    grid-template-columns: minmax(min(100%, 240px), 272px) 1fr;
    min-height: 0;
  }

  .tabs-panel {
    display: flex;
    flex-direction: column;
    border-right: 1px solid var(--border-soft);
    background: color-mix(in srgb, var(--elevated-soft) 66%, var(--panel-soft));
    min-height: 0;
    box-shadow: 7px 0 28px color-mix(in srgb, var(--text-soft) 4%, transparent);
    z-index: 2;
  }

  .tab-row {
    display: flex;
    flex-shrink: 0;
    gap: 0;
    padding: 0.45rem 0.5rem 0;
    border-bottom: 1px solid var(--border-soft);
    background: var(--elevated-soft);
  }

  .tab {
    flex: 1;
    padding: 0.6rem 0.32rem 0.68rem;
    font-size: 0.76rem;
    border: none;
    border-bottom: 2px solid transparent;
    margin-bottom: -1px;
    background: transparent;
    color: var(--muted);
    cursor: pointer;
    font-weight: 550;
    font-family: system-ui, sans-serif;
    letter-spacing: 0.02em;
    transition: color 0.12s ease, border-color 0.12s ease;
  }

  .tab:hover {
    color: var(--text-soft);
  }

  .tab.active {
    color: var(--accent-2);
    border-bottom-color: color-mix(in srgb, var(--accent) 65%, var(--accent-2));
    font-weight: 650;
    background: linear-gradient(180deg, transparent, color-mix(in srgb, var(--accent) 7%, transparent));
  }

  .tab-body {
    flex: 1;
    min-height: 0;
    background: color-mix(in srgb, var(--panel-soft) 35%, var(--elevated-soft));
    display: flex;
    flex-direction: column;
  }

  .scroll-list {
    flex: 1;
    overflow-y: auto;
    padding: 0.65rem 0.65rem 0.85rem;
    min-height: 0;
  }

  .empty-hint {
    margin: 0.65rem 0.35rem;
    font-size: 0.85rem;
    color: var(--muted);
    line-height: 1.5;
  }

  .list-btn {
    display: flex;
    align-items: flex-start;
    justify-content: space-between;
    gap: 0.55rem;
    width: 100%;
    text-align: left;
    padding: 0.62rem 0.65rem;
    margin-bottom: 0.35rem;
    border-radius: var(--radius-md);
    border: 1px solid transparent;
    background: transparent;
    color: var(--text-soft);
    cursor: pointer;
    font-size: 0.81rem;
    line-height: 1.38;
    transition: background 0.1s ease;
  }

  .list-btn:hover:not(:disabled) {
    background: color-mix(in srgb, var(--accent) 7%, var(--elevated-soft));
    border-color: color-mix(in srgb, var(--border-soft) 80%, transparent);
  }

  .list-btn:disabled {
    opacity: 0.45;
    cursor: default;
  }

  .page-grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(2.35rem, 1fr));
    gap: 0.4rem;
  }

  .page-cell {
    padding: 0.38rem 0.15rem;
    border-radius: var(--radius-sm);
    border: 1px solid var(--border-soft);
    background: var(--elevated-soft);
    color: var(--text-soft);
    font-size: 0.76rem;
    cursor: pointer;
    font-variant-numeric: tabular-nums;
    transition:
      border-color 0.1s ease,
      background 0.1s ease;
  }

  .page-cell:hover {
    border-color: color-mix(in srgb, var(--accent) 38%, var(--border-soft));
  }

  .page-cell.cur {
    background: color-mix(in srgb, var(--accent) 16%, var(--elevated-soft));
    border-color: color-mix(in srgb, var(--accent) 48%, var(--border-soft));
    font-weight: 650;
    color: var(--accent-2);
  }

  .notes-tab {
    padding: 0.75rem 0.85rem 1rem;
    display: flex;
    flex-direction: column;
    gap: 0.75rem;
    overflow-y: auto;
    flex: 1;
    min-height: 0;
  }

  .storage-details {
    border-radius: var(--radius-md);
    border: 1px solid var(--border-soft);
    background: var(--elevated-soft);
    font-size: 0.8rem;
    line-height: 1.45;
    color: var(--text-soft);
  }

  .storage-details summary {
    padding: 0.55rem 0.65rem;
    cursor: pointer;
    font-weight: 600;
    font-size: 0.78rem;
    font-family: system-ui, sans-serif;
    color: var(--muted);
    list-style: none;
    display: flex;
    align-items: center;
    gap: 0.35rem;
  }

  .storage-details summary::-webkit-details-marker {
    display: none;
  }

  .storage-details summary::before {
    content: "";
    width: 0.35rem;
    height: 0.35rem;
    border-radius: 50%;
    background: color-mix(in srgb, var(--accent) 55%, var(--accent-2));
    flex-shrink: 0;
  }

  .storage-details[open] summary {
    color: var(--accent-2);
    border-bottom: 1px solid var(--border-soft);
  }

  .storage-body {
    padding: 0.55rem 0.7rem 0.65rem;
  }

  .storage-body p {
    margin: 0;
  }

  .storage-second {
    margin-top: 0.45rem !important;
    color: var(--muted) !important;
    font-size: 0.78rem !important;
  }

  .notes-hint {
    margin: 0;
    font-size: 0.8rem;
    line-height: 1.45;
    color: var(--muted);
  }

  .quotes-lead {
    margin: 0 0 0.55rem;
    font-size: 0.76rem;
    line-height: 1.45;
    color: var(--muted);
  }

  .quote-card-list {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 0.55rem;
    max-height: min(42vh, 320px);
    overflow-y: auto;
    padding-right: 2px;
  }

  .quote-card {
    position: relative;
    padding: 0.55rem 0.65rem 0.55rem 0.85rem;
    border-radius: var(--radius-md);
    border: 1px solid color-mix(in srgb, var(--border-soft) 92%, transparent);
    background: var(--elevated-soft);
    overflow: hidden;
  }

  .quote-card.with-bg .quote-card-text,
  .quote-card.with-bg .quote-card-by {
    text-shadow: 0 1px 10px color-mix(in srgb, var(--elevated-soft) 85%, transparent);
  }

  .quote-card-media {
    position: absolute;
    inset: 0;
    pointer-events: none;
    overflow: hidden;
    border-radius: inherit;
  }

  .quote-card-media-img {
    position: absolute;
    inset: 0;
    background-position: center center;
    background-repeat: no-repeat;
    transform-origin: center center;
  }

  .quote-card-media-tint {
    position: absolute;
    inset: 0;
  }

  .quote-card-body {
    position: relative;
    z-index: 1;
  }

  .quote-card::before {
    content: "";
    position: absolute;
    left: 0;
    top: 0;
    bottom: 0;
    width: 4px;
    background: var(--stripe);
    border-radius: var(--radius-sm) 0 0 var(--radius-sm);
  }

  .quote-card-text {
    margin: 0 0 0.4rem;
    font-size: 0.84rem;
    line-height: 1.45;
    color: var(--text-soft);
    font-family: Georgia, "Times New Roman", serif;
  }

  .quote-card-by {
    margin: 0;
    font-size: 0.76rem;
    font-weight: 600;
    color: var(--text-soft);
    font-family: system-ui, sans-serif;
  }

  .quote-card-auth {
    font-weight: 500;
    color: var(--muted);
  }

  .quote-card-meta {
    margin: 0.35rem 0 0;
    font-size: 0.7rem;
    color: var(--muted);
    letter-spacing: 0.02em;
  }

  .mini-block {
    margin-bottom: 0.85rem;
  }

  .mini-block h4 {
    margin: 0 0 0.4rem;
    font-size: 0.72rem;
    text-transform: uppercase;
    letter-spacing: 0.1em;
    color: var(--muted);
    font-family: system-ui, sans-serif;
  }

  .cmt-list {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 0.5rem;
    max-height: min(36vh, 260px);
    overflow-y: auto;
    padding-right: 2px;
  }

  .cmt-item {
    padding: 0.5rem 0.6rem;
    border-radius: var(--radius-md);
    border: 1px solid color-mix(in srgb, var(--border-soft) 90%, transparent);
    background: color-mix(in srgb, var(--elevated-soft) 88%, var(--panel-soft));
    font-size: 0.82rem;
    line-height: 1.35;
    box-shadow: 0 4px 14px rgba(61, 56, 51, 0.05);
  }

  .cmt-meta {
    display: block;
    font-size: 0.72rem;
    color: var(--accent-2);
    margin-bottom: 0.25rem;
    font-weight: 600;
  }

  .cmt-ex {
    margin: 0 0 0.35rem;
    font-size: 0.78rem;
    color: var(--muted);
    font-style: italic;
  }

  .cmt-body {
    margin: 0;
    white-space: pre-wrap;
  }

  .notes-label {
    display: flex;
    flex-direction: column;
    gap: 0.35rem;
    font-size: 0.75rem;
    color: var(--muted);
    flex: 1;
    min-height: 0;
  }

  .notes-label textarea {
    flex: 1;
    min-height: 12rem;
    padding: 0.55rem 0.6rem;
    border-radius: var(--radius-md);
    border: 1px solid var(--border-soft);
    background: var(--panel-soft);
    color: var(--text);
    font-size: 0.9rem;
    line-height: 1.45;
    resize: vertical;
  }

  .read-stage {
    min-width: 0;
    min-height: 0;
    display: flex;
    flex-direction: column;
    padding: clamp(0.65rem, 1.8vw, 1.15rem);
    background: transparent;
  }

  .stage-frame {
    flex: 1;
    min-height: 0;
    display: flex;
    flex-direction: column;
    border-radius: var(--radius-xl);
    border: 1px solid color-mix(in srgb, var(--border-soft) 88%, #c9bfb4);
    background: var(--elevated-soft);
    box-shadow: var(--shadow-book);
    overflow: hidden;
  }

  /* Dreamcore reader chrome: the book remains the visual focus. */
  .read-shell {
    background:
      radial-gradient(circle at 92% 7%, color-mix(in srgb, #fff2bf 45%, transparent) 0 3.4rem, transparent 3.55rem),
      radial-gradient(ellipse 30rem 18rem at 2% 0%, color-mix(in srgb, var(--accent) 21%, transparent), transparent 68%),
      radial-gradient(ellipse 28rem 18rem at 100% 100%, color-mix(in srgb, #b8d7ef 16%, transparent), transparent 70%),
      var(--reader-bg);
  }

  .read-top {
    min-height: 4.6rem;
    border-color: color-mix(in srgb, var(--accent) 15%, var(--border-soft));
    background: color-mix(in srgb, var(--panel-veil) 80%, transparent);
    backdrop-filter: blur(24px) saturate(1.18);
    box-shadow: none;
  }

  .back,
  .settings-link,
  .typst-export-top,
  .reader-panel-toggle,
  .mini {
    border-radius: 999px;
  }

  .back {
    margin-left: 0;
    border: 1px solid color-mix(in srgb, var(--accent) 16%, var(--border-soft));
    background: color-mix(in srgb, var(--elevated-soft) 54%, transparent);
  }

  .read-title {
    font-family: Georgia, "Times New Roman", serif;
    font-size: clamp(1.08rem, 3.5vw, 1.35rem);
    font-weight: 500;
    letter-spacing: -0.035em;
  }

  .tabs-panel {
    border-color: color-mix(in srgb, var(--accent) 16%, var(--border-soft));
    background: color-mix(in srgb, var(--panel-elevated) 68%, transparent);
    box-shadow: 10px 0 45px color-mix(in srgb, var(--accent-2) 6%, transparent);
    backdrop-filter: blur(20px);
  }

  .tab-row {
    gap: 0.25rem;
    padding: 0.55rem;
    border-color: color-mix(in srgb, var(--accent) 14%, var(--border-soft));
    background: transparent;
  }

  .tab {
    margin: 0;
    padding: 0.58rem 0.25rem;
    border: 0;
    border-radius: 0.75rem;
  }

  .tab.active {
    border: 0;
    background: color-mix(in srgb, var(--accent) 15%, var(--elevated-soft));
    box-shadow: inset 0 0 0 1px color-mix(in srgb, var(--accent) 15%, transparent);
  }

  .tab-body {
    background: transparent;
  }

  .list-btn,
  .cmt-item,
  .quote-card {
    border-color: color-mix(in srgb, var(--accent) 15%, var(--border-soft));
    background: color-mix(in srgb, var(--elevated-soft) 58%, transparent);
  }

  .notes-label textarea {
    border-color: color-mix(in srgb, var(--accent) 18%, var(--border-soft));
    border-radius: 1rem;
    background: color-mix(in srgb, var(--elevated-soft) 72%, transparent);
  }

  .stage-frame {
    border-color: color-mix(in srgb, var(--accent) 18%, var(--border-soft));
    border-radius: 1.75rem;
    box-shadow: 0 24px 72px color-mix(in srgb, var(--accent-2) 10%, transparent);
  }

  @media (max-width: 900px) {
    .read-body {
      grid-template-columns: 1fr;
      grid-template-rows: minmax(0, 1fr);
    }

    .tabs-panel {
      position: fixed;
      inset: 0 auto 0 0;
      z-index: 70;
      width: min(90vw, 22rem);
      border-right: 1px solid var(--border-soft);
      border-bottom: none;
      box-shadow: var(--shadow-float);
      transform: translateX(-104%);
      visibility: hidden;
      transition: transform 0.24s cubic-bezier(0.33, 1, 0.68, 1), visibility 0.24s;
      padding-top: env(safe-area-inset-top);
      padding-bottom: env(safe-area-inset-bottom);
      overscroll-behavior: contain;
    }

    .tabs-panel-open {
      transform: translateX(0);
      visibility: visible;
    }

    .reader-panel-toggle {
      display: inline-flex;
    }

    .panel-head-mobile {
      display: flex;
      align-items: center;
      justify-content: space-between;
      min-height: 3.8rem;
      padding: 0.7rem 0.85rem 0.55rem 1rem;
      color: var(--text-soft);
      font-size: 0.9rem;
      font-weight: 700;
    }

    .panel-head-mobile button {
      display: grid;
      place-items: center;
      width: 2.75rem;
      height: 2.75rem;
      padding: 0;
      border: 0;
      border-radius: 999px;
      background: var(--panel-soft);
      color: var(--muted);
      cursor: pointer;
      font-size: 1.3rem;
    }

    .reader-backdrop {
      position: fixed;
      inset: 0;
      z-index: 65;
      border: 0;
      background: color-mix(in srgb, #111 42%, transparent);
      backdrop-filter: blur(3px);
      opacity: 0;
      visibility: hidden;
      transition: opacity 0.2s ease, visibility 0.2s;
    }

    .reader-backdrop-open {
      display: block;
      opacity: 1;
      visibility: visible;
    }

    .read-stage {
      padding: clamp(0.45rem, 2vw, 0.65rem) clamp(0.45rem, 2.5vw, 0.75rem)
        clamp(0.55rem, 2.5vw, 0.75rem);
    }

    .epub-nav {
      margin-left: auto;
    }
  }

  @media (max-width: 480px) {
    .back span:last-child {
      display: none;
    }

    .back {
      width: 2.75rem;
      min-height: 2.75rem;
      justify-content: center;
      padding: 0;
    }

    .settings-link {
      display: none;
    }

    .read-top {
      gap: 0.45rem;
      padding:
        max(0.5rem, env(safe-area-inset-top))
        max(0.55rem, env(safe-area-inset-right))
        0.5rem
        max(0.55rem, env(safe-area-inset-left));
    }

    .title-block {
      order: 3;
      flex-basis: 100%;
      padding: 0.15rem 0 0.1rem;
    }

    .reader-panel-toggle {
      margin-left: auto;
      width: 2.75rem;
      min-height: 2.75rem;
      padding: 0;
      border-radius: 999px;
    }

    .reader-panel-toggle span {
      display: none;
    }

    .typst-export-top {
      padding-inline: 0.7rem;
      min-height: 2.75rem;
    }

    .mini {
      width: 2.75rem;
      height: 2.75rem;
    }

    .read-stage {
      padding: 0;
    }

    .stage-frame {
      border-right: 0;
      border-bottom: 0;
      border-left: 0;
      border-radius: 0;
      box-shadow: none;
    }

    .tab {
      padding: 0.55rem 0.25rem;
      font-size: 0.72rem;
    }
  }

  @media (max-width: 600px) {
    .read-body {
      overscroll-behavior: none;
    }

    .tab,
    .list-btn,
    .page-cell {
      min-height: 2.75rem;
    }
  }

  @media (max-height: 520px) and (orientation: landscape) {
    .read-top {
      min-height: 0;
      flex-wrap: nowrap;
      padding-top: max(0.35rem, env(safe-area-inset-top));
      padding-bottom: 0.35rem;
    }

    .title-block {
      order: 0;
      flex-basis: 8rem;
      padding: 0;
    }

    .read-title {
      -webkit-line-clamp: 1;
      line-clamp: 1;
    }

    .read-author {
      display: none;
    }
  }
</style>
