<script lang="ts">
  import { onDestroy, untrack } from "svelte";
  import { goto } from "$app/navigation";
  import { page } from "$app/state";
  import { invoke } from "@tauri-apps/api/core";
  import type {
    BookComment,
    BookMeta,
    EpubReaderApi,
    Highlight,
    HighlightColor,
    LibraryMetadata,
    LibrarySnapshot,
    PdfOutlineItem,
    PdfReadyInfo,
    QuoteDraftOptions,
    ReaderOutlineItem,
    ReaderSelection,
    ReadingPosition,
    SavedQuote,
  } from "$lib/types";
  import PdfViewer from "$lib/components/PdfViewer.svelte";
  import EpubViewer from "$lib/components/EpubViewer.svelte";
  import Fb2Viewer from "$lib/components/Fb2Viewer.svelte";
  import TypstViewer from "$lib/components/TypstViewer.svelte";
  import TranslationBar from "$lib/components/TranslationBar.svelte";
  import ReaderOutlineTree from "$lib/components/ReaderOutlineTree.svelte";
  import ReaderSelectionToolbar from "$lib/components/ReaderSelectionToolbar.svelte";
  import HighlightPopover from "$lib/components/HighlightPopover.svelte";
  import CommentDialog from "$lib/components/CommentDialog.svelte";
  import QuoteDialog from "$lib/components/QuoteDialog.svelte";
  import TypographyPanel from "$lib/components/TypographyPanel.svelte";
  import ReaderNotesPanel from "$lib/components/ReaderNotesPanel.svelte";
  import ReaderSearchPanel from "$lib/components/ReaderSearchPanel.svelte";
  import ReaderStatusBar from "$lib/components/ReaderStatusBar.svelte";
  import ReadingRuler from "$lib/components/ReadingRuler.svelte";
  import TtsBar from "$lib/components/TtsBar.svelte";
  import RsvpOverlay from "$lib/components/RsvpOverlay.svelte";
  import AmbientPanel from "$lib/components/AmbientPanel.svelte";
  import LookupPopover from "$lib/components/LookupPopover.svelte";
  import RecapDialog from "$lib/components/RecapDialog.svelte";
  import GlossaryPanel from "$lib/components/GlossaryPanel.svelte";
  import { startReading, stopReading, tts } from "$lib/reading/tts.svelte";
  import { AMBIENT_OPTIONS, ambient, playAmbient, stopAmbient, type AmbientId } from "$lib/reading/ambient.svelte";
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
  import { toast, toastError } from "$lib/ui/toast.svelte";
  import { reading, updateReading } from "$lib/reading/settings.svelte";
  import { coverAccentVars, coverTone, pagePalette, type CoverTone } from "$lib/reading/palette";
  import { loadCover } from "$lib/covers/coverCache";
  import { minutesLeft, startSession } from "$lib/reading/stats.svelte";
  import { bookNotesMarkdown, saveMarkdown } from "$lib/reading/exportNotes";
  import { writeTextToClipboard } from "$lib/clipboardWrite";
  import { isTauriRuntime } from "$lib/isTauri";

  const bookPath = $derived.by(() => {
    const raw = page.url.searchParams.get("path");
    if (!raw) return "";
    try {
      return decodeURIComponent(raw);
    } catch {
      return "";
    }
  });
  const fmt = $derived(bookPath ? getBookFormat(bookPath) : null);

  let snapshot = $state<LibrarySnapshot | null>(getCachedLibrarySnapshot());
  let pdfOutline = $state<PdfOutlineItem[]>([]);
  let pdfNumPages = $state(0);
  let navApi = $state<EpubReaderApi | null>(null);
  let pdfPage = $state(1);
  let position = $state<ReadingPosition | null>(null);
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
  let pdfInlineShow = $state(true);
  let pdfExportBusy = $state(false);
  let typstExportBusy = $state(false);
  let typstOutline = $state<TypstOutlineItem[]>([]);
  let typstJumpLine = $state<number | null>(null);

  // ——— Интерфейс ———
  type PanelTab = "toc" | "notes" | "search" | "glossary" | "translate";
  const PANEL_KEY = "reader.panelOpen";
  let panelOpen = $state(readPanelPref());
  let panelTab = $state<PanelTab>("toc");
  let aaOpen = $state(false);
  let moreOpen = $state(false);
  let immersive = $state(false);
  /** В режиме погружения шапка появляется при наведении к верхнему краю */
  let peek = $state(false);
  let searchSeed = $state("");
  let stageEl = $state<HTMLElement | null>(null);
  let isFullscreen = $state(false);
  let rsvpOpen = $state(false);
  let recapOpen = $state(false);
  let glossaryVersion = $state(0);
  let lookup = $state<{
    kind: "word" | "who";
    term: string;
    context: string;
    rect: { left: number; top: number; width: number; height: number };
  } | null>(null);
  let ambientOpen = $state(false);

  // ——— Выделение и заметки ———
  let selection = $state<ReaderSelection | null>(null);
  let commentOpen = $state(false);
  let quoteOpen = $state(false);
  let quoteText = $state("");
  let quoteChapter = $state("");
  let hlPopover = $state<{ id: string; rect: DOMRect; editNote: boolean } | null>(null);

  function narrow(): boolean {
    return typeof window !== "undefined" && window.matchMedia("(max-width: 1100px)").matches;
  }

  function readPanelPref(): boolean {
    try {
      if (narrow()) return false;
      return localStorage.getItem(PANEL_KEY) === "1";
    } catch {
      return false;
    }
  }

  function setPanel(open: boolean, tab?: PanelTab) {
    panelOpen = open;
    if (tab) panelTab = tab;
    try {
      if (!narrow()) localStorage.setItem(PANEL_KEY, open ? "1" : "0");
    } catch {
      /* ignore */
    }
  }

  function togglePanel(tab: PanelTab) {
    if (panelOpen && panelTab === tab) setPanel(false);
    else setPanel(true, tab);
  }

  const meta = $derived(bookPath && snapshot?.metadata.books[bookPath] ? snapshot.metadata.books[bookPath] : null);
  const displayTitle = $derived(bookPath ? meta?.title?.trim() || titleFromPath(bookPath) : "");
  const displayAuthor = $derived(meta?.author?.trim() ?? "");
  /**
   * Список выделений меняем только когда они реально изменились: сохранение
   * позиции пересоздаёт метаданные книги, и без этого просмотрщики
   * перерисовывали бы все выделения на каждом шаге прокрутки.
   */
  const EMPTY_HIGHLIGHTS: Highlight[] = [];
  let highlights = $state.raw<Highlight[]>(EMPTY_HIGHLIGHTS);
  let highlightsSig = "";
  $effect(() => {
    const list = meta?.highlights ?? EMPTY_HIGHLIGHTS;
    const sig = list.map((h) => `${h.id}|${h.color}|${h.note ?? ""}`).join(";");
    if (sig === highlightsSig) return;
    highlightsSig = sig;
    highlights = list;
  });

  // ——— Палитра страницы и «живая обложка» ———
  let themeTick = $state(0);
  let tone = $state<CoverTone | null>(null);

  $effect(() => {
    const obs = new MutationObserver(() => themeTick++);
    obs.observe(document.documentElement, { attributes: true, attributeFilter: ["data-theme"] });
    return () => obs.disconnect();
  });

  $effect(() => {
    const p = bookPath;
    const f = fmt;
    tone = null;
    if (!p || !f) return;
    let alive = true;
    void loadCover(p, f).then(async (url) => {
      const t = await coverTone(url);
      if (alive) tone = t;
    });
    return () => {
      alive = false;
    };
  });

  const palette = $derived.by(() => {
    themeTick;
    return pagePalette(reading.s.pageTheme, tone);
  });

  const shellStyle = $derived.by(() => {
    themeTick;
    if (!reading.s.livingCover || !tone) return "";
    const dark = document.documentElement.dataset.theme === "dark";
    return Object.entries(coverAccentVars(tone, dark))
      .map(([k, v]) => `${k}:${v}`)
      .join(";");
  });

  // ——— Оглавление ———
  const pdfTreeItems = $derived<ReaderOutlineItem[]>(
    pdfOutline.map((item, index) => ({
      id: `pdf-${index}`,
      label: item.title,
      level: item.level,
      meta: item.page == null ? undefined : `${item.page}`,
      disabled: item.page == null,
    })),
  );

  const activePdfTreeId = $derived.by(() => {
    let found = -1;
    let foundPage = -1;
    for (let index = 0; index < pdfOutline.length; index++) {
      const pg = pdfOutline[index]?.page;
      if (pg != null && pg <= pdfPage && pg >= foundPage) {
        found = index;
        foundPage = pg;
      }
    }
    return found >= 0 ? `pdf-${found}` : null;
  });

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
    if (!navApi) return null;
    const label = position?.chapterLabel;
    if (label) {
      const byLabel = navApi.toc.findIndex((t) => t.label === label);
      if (byLabel >= 0) return `nav-${byLabel}`;
    }
    const href = (meta?.lastReadHref ?? meta?.lastReadLocation ?? "").trim();
    if (!href) return null;
    const base = href.split("#")[0];
    const index = navApi.toc.findIndex((item) => item.href === href || item.href.split("#")[0] === base);
    return index >= 0 ? `nav-${index}` : null;
  });

  function finishNavigation() {
    if (narrow()) setPanel(false);
  }

  function selectPdfChapter(id: string) {
    const item = pdfOutline[Number(id.replace("pdf-", ""))];
    if (item?.page != null) pdfPage = item.page;
    finishNavigation();
  }

  function selectTypstChapter(id: string) {
    const item = typstOutline[Number(id.replace("typst-", ""))];
    if (item) typstJumpLine = item.line;
    finishNavigation();
  }

  function selectNavChapter(id: string) {
    const item = navApi?.toc[Number(id.replace("nav-", ""))];
    if (item) void navApi?.goTo(item.href);
    finishNavigation();
  }

  function titleFromPath(path: string) {
    const i = path.lastIndexOf("/");
    return i >= 0 ? path.slice(i + 1) : path;
  }

  // ——— Данные ———
  async function loadSnapshot() {
    try {
      snapshot = await fetchLibrarySnapshot();
    } catch {
      snapshot = null;
    }
  }

  function patchBook(path: string, patch: Partial<BookMeta>) {
    if (!snapshot) return;
    const cur = snapshot.metadata.books[path];
    if (!cur) return;
    const books = { ...snapshot.metadata.books, [path]: { ...cur, ...patch } };
    const next: LibraryMetadata = { ...snapshot.metadata, books };
    setCachedLibraryMetadata(next);
    snapshot = { ...snapshot, metadata: next };
    void invoke("save_library_metadata", { metadata: next }).catch((e) => toastError(e, "Сохранение"));
  }

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
    const cur = snap.metadata.books[p]!;
    const prevOpened = cur.lastOpenedAt ? Date.parse(cur.lastOpenedAt) : NaN;
    const prog = cur.progress ?? (cur.lastReadPdfTotal ? (cur.lastReadPdfPage ?? 0) / cur.lastReadPdfTotal : 0);
    if (Number.isFinite(prevOpened) && Date.now() - prevOpened > 3 * 86_400_000 && prog > 0.03 && prog < 0.98) {
      const days = Math.floor((Date.now() - prevOpened) / 86_400_000);
      setTimeout(() => {
        toast(`Вы не открывали книгу ${days} дн. Напомнить, что было?`, "info", {
          action: { label: "Ранее в книге…", run: () => (recapOpen = true) },
          timeout: 12000,
        });
      }, 1800);
    }
    const patch: Partial<BookMeta> = { lastOpenedAt: new Date().toISOString() };
    if (!cur.status || cur.status === "want") patch.status = "reading";
    untrack(() => patchBook(p, patch));
  });

  function scheduleReviewSave(path: string, text: string) {
    reviewDraft = text;
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

  // ——— Статистика чтения ———
  let session: ReturnType<typeof startSession> | null = null;
  $effect(() => {
    const p = bookPath;
    if (!p) return;
    const s = startSession(p);
    session = s;
    return () => {
      s.stop();
      if (session === s) session = null;
    };
  });

  /** Книга дочитана: отмечаем один раз. */
  function maybeFinish(progress: number | null) {
    if (progress == null || progress < 0.985 || !bookPath) return;
    const cur = snapshot?.metadata.books[bookPath];
    if (!cur || cur.status === "done") return;
    patchBook(bookPath, { status: "done", finishedAt: new Date().toISOString() });
    toast("Книга дочитана — поздравляю! ✦", "success");
  }

  function bookMinutesLeft(): number | null {
    const p = position;
    if (!p) return null;
    const m = minutesLeft({ charsLeft: p.charsLeftBook, pagesLeft: p.pagesLeftBook });
    return m == null ? null : Math.round(m);
  }

  function onPosition(p: ReadingPosition) {
    position = p;
    session?.position({ charsLeftBook: p.charsLeftBook, page: fmt === "pdf" ? pdfPage : null });
  }

  let locSaveTimer: ReturnType<typeof setTimeout> | null = null;
  let lastSavedLocation = "";
  function onReadingLocationSave(p: { location: string; label: string; href: string; progress: number | null }) {
    if (!bookPath) return;
    const path = bookPath;
    // Позиция не изменилась — не пишем метаданные на диск ещё раз.
    if (`${path}|${p.location}` === lastSavedLocation && !locSaveTimer) return;
    if (locSaveTimer) clearTimeout(locSaveTimer);
    locSaveTimer = setTimeout(() => {
      patchBook(path, {
        lastReadLocation: p.location,
        lastReadLocationLabel: p.label,
        lastReadHref: p.href || null,
        progress: p.progress,
        minutesLeft: bookMinutesLeft(),
      });
      maybeFinish(p.progress);
      lastSavedLocation = `${path}|${p.location}`;
      locSaveTimer = null;
    }, 1200);
  }

  $effect(() => {
    bookPath;
    pdfProgressKey = "";
    position = null;
    selection = null;
    hlPopover = null;
  });

  /** Восстановить страницу PDF, когда известно число страниц */
  $effect(() => {
    const path = bookPath;
    const snap = snapshot;
    if (!path || !snap || getBookFormat(path) !== "pdf") return;
    if (pdfNumPages <= 0) return;
    const key = `${path}:${pdfNumPages}`;
    if (pdfProgressKey === key) return;
    const pg = snap.metadata.books[path]?.lastReadPdfPage;
    if (pg != null && pg >= 1) pdfPage = Math.min(Math.max(1, Math.round(pg)), pdfNumPages);
    pdfProgressKey = key;
  });

  let pdfProgTimer: ReturnType<typeof setTimeout> | null = null;
  $effect(() => {
    const path = bookPath;
    const total = pdfNumPages;
    const pg = pdfPage;
    if (!path || fmt !== "pdf" || total <= 0 || pdfProgressKey === "") return;
    untrack(() => {
      if (pdfProgTimer) clearTimeout(pdfProgTimer);
      pdfProgTimer = setTimeout(() => {
        const progress = position?.progress ?? pg / total;
        patchBook(path, {
          lastReadPdfPage: pg,
          lastReadPdfTotal: total,
          progress,
          lastReadLocationLabel: position?.chapterLabel || null,
          minutesLeft: bookMinutesLeft(),
        });
        maybeFinish(pg >= total ? 1 : progress);
        pdfProgTimer = null;
      }, 500);
    });
  });

  // ——— Выделения ———
  function onSelection(s: ReaderSelection | null) {
    selection = s;
  }

  function addHighlight(color: HighlightColor, note?: string): Highlight | null {
    const s = selection;
    if (!s || !bookPath || !meta) return null;
    const anchor = Object.fromEntries(Object.entries(s.anchor).filter(([, v]) => v != null && v !== ""));
    const h: Highlight = {
      id: crypto.randomUUID(),
      text: s.text.slice(0, 4000),
      color,
      createdAt: new Date().toISOString(),
      ...anchor,
    };
    if (note) h.note = note;
    patchBook(bookPath, { highlights: [...(meta.highlights ?? []), h] });
    s.clear();
    selection = null;
    return h;
  }

  function updateHighlight(id: string, patch: Partial<Highlight>) {
    if (!bookPath || !meta) return;
    const list = (meta.highlights ?? []).map((h) => (h.id === id ? { ...h, ...patch } : h));
    patchBook(bookPath, { highlights: list });
  }

  function deleteHighlight(id: string) {
    if (!bookPath || !meta) return;
    const path = bookPath;
    const removed = meta.highlights?.find((h) => h.id === id);
    patchBook(path, { highlights: (meta.highlights ?? []).filter((h) => h.id !== id) });
    hlPopover = null;
    if (removed) {
      toast("Выделение удалено", "info", {
        action: {
          label: "Вернуть",
          run: () => {
            const cur = snapshot?.metadata.books[path];
            if (cur) patchBook(path, { highlights: [...(cur.highlights ?? []), removed] });
          },
        },
      });
    }
  }

  function noteFromSelection() {
    const s = selection;
    if (!s) return;
    const r = s.rect;
    const h = addHighlight("yellow");
    if (h) hlPopover = { id: h.id, rect: new DOMRect(r.left, r.top, r.width, r.height), editNote: true };
  }

  function quoteFromSelection() {
    const s = selection;
    if (!s) return;
    quoteText = s.text;
    quoteChapter = s.anchor.chapterLabel ?? position?.chapterLabel ?? "";
    quoteOpen = true;
    s.clear();
    selection = null;
  }

  async function copySelection() {
    const s = selection;
    if (!s) return;
    try {
      await writeTextToClipboard(s.text);
      toast("Скопировано", "success");
    } catch {
      /* ignore */
    }
    s.clear();
    selection = null;
  }

  function lookupSelection(kind: "word" | "who") {
    const s = selection;
    if (!s) return;
    lookup = { kind, term: s.text.trim().slice(0, 80), context: s.context ?? s.text, rect: s.rect };
    s.clear();
    selection = null;
  }

  function searchSelection() {
    const s = selection;
    if (!s) return;
    searchSeed = s.text.trim().slice(0, 80);
    s.clear();
    selection = null;
    setPanel(true, "search");
  }

  function onHighlightClick(id: string, rect: DOMRect) {
    hlPopover = { id, rect, editNote: false };
  }

  async function goToHighlight(h: Highlight) {
    const api = navApi;
    if (!api?.goToHit) return;
    const len = h.text.length;
    const hit = { id: h.id, label: "", before: "", match: h.text, after: "" };
    if (fmt === "pdf" && h.page != null) {
      await api.goToHit({ ...hit, loc: `p:${h.page}:${h.offset ?? 0}:${len}` });
    } else if (fmt === "fb2" && h.block != null) {
      await api.goToHit({ ...hit, loc: `s:${h.block}:${h.offset ?? 0}:${len}` });
    } else if (h.cfi) {
      await api.goTo(h.cfi);
    }
    finishNavigation();
  }

  /** Переход из ленты заметок: `/read?path=…&hl=<id>` */
  let handledHl = "";
  $effect(() => {
    const id = page.url.searchParams.get("hl");
    const api = navApi;
    const h = id ? highlights.find((x) => x.id === id) : null;
    if (!id || !api || !h || handledHl === id) return;
    handledHl = id;
    setTimeout(() => void goToHighlight(h), 500);
  });

  const popHighlight = $derived(hlPopover ? (highlights.find((h) => h.id === hlPopover!.id) ?? null) : null);

  function addComment(body: string) {
    if (!snapshot || !bookPath) return;
    const cur = snapshot.metadata.books[bookPath];
    if (!cur) return;
    const com: BookComment = {
      id: crypto.randomUUID(),
      body,
      excerpt: quoteText,
      createdAt: new Date().toISOString(),
    };
    patchBook(bookPath, { comments: [...(cur.comments ?? []), com] });
  }

  function addQuote(o: QuoteDraftOptions) {
    if (!snapshot || !bookPath) return;
    const cur = snapshot.metadata.books[bookPath];
    if (!cur) return;
    const quote: SavedQuote = {
      id: crypto.randomUUID(),
      text: o.text,
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
    patchBook(bookPath, { quotes: [...(cur.quotes ?? []), quote] });
    toast("Цитата сохранена", "success");
  }

  async function exportNotes() {
    if (!bookPath || !meta) return;
    try {
      const res = await saveMarkdown(displayTitle, bookNotesMarkdown(bookPath, meta));
      if (res === "saved") toast("Конспект сохранён", "success");
      else if (res === "copied") toast("Конспект скопирован в буфер", "success");
    } catch (e) {
      toastError(e, "Экспорт");
    }
  }

  async function copyNotes() {
    if (!bookPath || !meta) return;
    await writeTextToClipboard(bookNotesMarkdown(bookPath, meta));
    toast("Конспект в Markdown скопирован", "success");
  }

  // ——— Экспорт ———
  async function exportCurrentBookToTypst() {
    moreOpen = false;
    if (!bookPath || !fmt || fmt === "typst") return;
    typstExportBusy = true;
    try {
      if (!snapshot) throw new Error("Сначала откройте библиотеку.");
      const { mainRelativePath } = await exportBookToTypst(bookPath, fmt, snapshot, meta?.typstStyleRelativePath);
      await loadSnapshot();
      toast(`Создан проект Typst: ${mainRelativePath}`, "success");
      await goto("/read?path=" + encodeURIComponent(mainRelativePath));
    } catch (e) {
      toastError(e, "Экспорт в Typst");
    } finally {
      typstExportBusy = false;
    }
  }

  async function exportPdfTranslation() {
    if (!bookPath || fmt !== "pdf") return;
    pdfExportBusy = true;
    try {
      await exportTranslatedPdfToLibrary(bookPath);
      await loadSnapshot();
      toast("Переведённый PDF сохранён рядом с книгой", "success");
    } catch (e) {
      toastError(e, "Экспорт перевода");
    } finally {
      pdfExportBusy = false;
    }
  }

  function handleTranslateActivity(e: { busy: boolean; error: string | null }) {
    transBusy = e.busy;
    transErr = e.error;
    if (!e.busy && !e.error && fmt === "fb2") hasFb2Translation = true;
  }

  // ——— Погружение и полноэкранный режим ———
  function toggleImmersive(force?: boolean) {
    immersive = force ?? !immersive;
    peek = false;
    if (immersive) {
      aaOpen = false;
      moreOpen = false;
      if (narrow()) setPanel(false);
    }
  }

  async function toggleFullscreen() {
    moreOpen = false;
    try {
      if (isTauriRuntime()) {
        const { getCurrentWindow } = await import("@tauri-apps/api/window");
        const w = getCurrentWindow();
        isFullscreen = !(await w.isFullscreen());
        await w.setFullscreen(isFullscreen);
      } else if (!document.fullscreenElement) {
        await document.documentElement.requestFullscreen();
        isFullscreen = true;
      } else {
        await document.exitFullscreen();
        isFullscreen = false;
      }
      if (isFullscreen) toggleImmersive(true);
    } catch (e) {
      toastError(e, "Полный экран");
    }
  }

  function toggleTts() {
    moreOpen = false;
    if (tts.active) {
      stopReading();
      return;
    }
    if (!navApi?.unitsFromHere) {
      toast("Для этого формата чтение вслух недоступно", "info");
      return;
    }
    void startReading(navApi);
  }

  function openRsvp() {
    moreOpen = false;
    if (!navApi?.unitsFromHere) {
      toast("Для этого формата быстрое чтение недоступно", "info");
      return;
    }
    stopReading();
    rsvpOpen = true;
  }

  function bindAmbient(id: AmbientId | null) {
    if (bookPath) patchBook(bookPath, { ambientSound: id });
  }

  // Книга «помнит» свой звук: предлагаем включить (автозапуск звука запрещён без жеста).
  let offeredAmbientFor = "";
  $effect(() => {
    const p = bookPath;
    const id = meta?.ambientSound as AmbientId | null | undefined;
    if (!p || !id || offeredAmbientFor === p || ambient.current === id) return;
    offeredAmbientFor = p;
    const label = AMBIENT_OPTIONS.find((o) => o.id === id)?.label ?? id;
    toast(`У этой книги есть свой звук: «${label}»`, "info", {
      action: { label: "Включить", run: () => void playAmbient(id) },
      timeout: 9000,
    });
  });

  function onCenterTap() {
    if (selection) return;
    toggleImmersive();
  }

  function onPointerMove(e: PointerEvent) {
    if (!immersive) return;
    const want = e.clientY < 64;
    if (want !== peek) peek = want;
  }

  function stepFont(delta: number) {
    if (fmt === "pdf") {
      const z = reading.s.pdfFit === "custom" ? reading.s.pdfZoom : 1.2;
      updateReading({ pdfFit: "custom", pdfZoom: Math.min(5, Math.max(0.3, z * (delta > 0 ? 1.1 : 1 / 1.1))) });
    } else {
      updateReading({ fontSize: Math.min(34, Math.max(12, reading.s.fontSize + delta)) });
    }
  }

  function typing(t: EventTarget | null) {
    const el = t as HTMLElement | null;
    return !!el?.closest?.("input, textarea, select, [contenteditable=true]");
  }

  function onKeydown(e: KeyboardEvent) {
    if (rsvpOpen) return;
    if (typing(e.target)) {
      if (e.key === "Escape") (e.target as HTMLElement).blur();
      return;
    }
    const api = navApi;
    const mod = e.ctrlKey || e.metaKey;
    if (mod && (e.key === "f" || e.key === "а")) {
      e.preventDefault();
      setPanel(true, "search");
      return;
    }
    if (mod && (e.key === "=" || e.key === "+")) {
      e.preventDefault();
      stepFont(1);
      return;
    }
    if (mod && e.key === "-") {
      e.preventDefault();
      stepFont(-1);
      return;
    }
    if (mod || e.altKey) return;
    switch (e.key) {
      case "ArrowRight":
      case "PageDown":
        e.preventDefault();
        void api?.next();
        break;
      case "ArrowLeft":
      case "PageUp":
        e.preventDefault();
        void api?.prev();
        break;
      case " ":
        if (fmt === "fb2" || (fmt === "pdf" && reading.s.pdfMode === "continuous")) return;
        if (fmt === "epub" && reading.s.epubFlow === "scrolled") return;
        e.preventDefault();
        void (e.shiftKey ? api?.prev() : api?.next());
        break;
      case "Escape":
        if (hlPopover) hlPopover = null;
        else if (lookup) lookup = null;
        else if (recapOpen) recapOpen = false;
        else if (ambientOpen) ambientOpen = false;
        else if (aaOpen) aaOpen = false;
        else if (moreOpen) moreOpen = false;
        else if (selection) {
          selection.clear();
          selection = null;
        } else if (immersive) toggleImmersive(false);
        else if (panelOpen && narrow()) setPanel(false);
        break;
      case "f":
      case "а":
        toggleImmersive();
        break;
      case "F11":
        e.preventDefault();
        void toggleFullscreen();
        break;
      case "t":
      case "е":
        togglePanel("toc");
        break;
      case "n":
      case "т":
        togglePanel("notes");
        break;
      case "/":
        e.preventDefault();
        setPanel(true, "search");
        break;
      case "a":
      case "ф":
        aaOpen = !aaOpen;
        break;
      case "s":
      case "ы":
        if (fmt !== "typst") toggleTts();
        break;
      case "r":
      case "к":
        if (fmt !== "typst") openRsvp();
        break;
    }
  }

  // ——— Жизненный цикл ———
  $effect(() => {
    const path = bookPath;
    if (!snapshot || (path && !snapshot.metadata.books[path])) void loadSnapshot();
  });

  $effect(() => {
    if (fmt !== "typst") {
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
    if (!getBookFormat(bookPath)) goto("/");
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
        pdfInlineSpans = doc?.pages && Object.keys(doc.pages).length > 0 ? doc.pages : null;
      } catch {
        pdfInlineSpans = null;
      }
    })();
  });

  // Возврат в окно (например, после синхронизации) — перечитываем библиотеку.
  $effect(() => {
    const onVis = () => {
      if (document.visibilityState === "visible") void loadSnapshot();
    };
    document.addEventListener("visibilitychange", onVis);
    return () => document.removeEventListener("visibilitychange", onVis);
  });

  $effect(() => {
    bookPath;
    return () => stopReading();
  });

  onDestroy(() => {
    stopReading();
    stopAmbient();
    if (saveTimer) clearTimeout(saveTimer);
    if (locSaveTimer) clearTimeout(locSaveTimer);
    if (pdfProgTimer) clearTimeout(pdfProgTimer);
  });

  const rulerBand = $derived(fmt === "pdf" ? 42 : Math.round(reading.s.fontSize * reading.s.lineHeight * 1.5));
  const shortSelection = $derived(!!selection && selection.text.length <= 60 && selection.text.split(/\s+/).length <= 5);
</script>

<svelte:window onkeydown={onKeydown} onpointermove={onPointerMove} />

{#if bookPath && fmt}
  <div
    class="read-shell"
    class:immersive
    class:peek
    class:panel-open={panelOpen}
    style={shellStyle}
    style:--page-bg={palette.bg}
    style:--chrome-text={palette.text}
    style:--chrome-muted={palette.muted}
  >
    <header class="read-top">
      <button type="button" class="icon-btn back" onclick={() => goto("/")} title="Библиотека" aria-label="В библиотеку">
        <svg viewBox="0 0 24 24" aria-hidden="true"><path d="M15 5l-7 7 7 7" /></svg>
      </button>
      <button
        type="button"
        class="icon-btn"
        class:on={panelOpen && panelTab === "toc"}
        title="Оглавление (T)"
        aria-label="Оглавление"
        onclick={() => togglePanel("toc")}
      >
        <svg viewBox="0 0 24 24" aria-hidden="true"><path d="M5 6h14M5 12h14M5 18h9" /></svg>
      </button>

      <div class="title-block">
        <h1 class="read-title" title={displayTitle}>{displayTitle}</h1>
        {#if displayAuthor}
          <p class="read-author">{displayAuthor}</p>
        {/if}
      </div>

      <div class="top-actions">
        {#if fmt !== "typst"}
          <button
            type="button"
            class="icon-btn"
            class:on={panelOpen && panelTab === "search"}
            title="Поиск по книге (Ctrl+F)"
            aria-label="Поиск"
            onclick={() => togglePanel("search")}
          >
            <svg viewBox="0 0 24 24" aria-hidden="true"><circle cx="11" cy="11" r="6.5" /><path d="m16 16 4 4" /></svg>
          </button>
          <button
            type="button"
            class="icon-btn"
            class:on={panelOpen && panelTab === "notes"}
            title="Заметки и выделения (N)"
            aria-label="Заметки"
            onclick={() => togglePanel("notes")}
          >
            <svg viewBox="0 0 24 24" aria-hidden="true"><path d="M6 4h9l3 3v13H6z" /><path d="M9 11h6M9 15h4" /></svg>
            {#if highlights.length}<span class="badge">{highlights.length}</span>{/if}
          </button>
          <button
            type="button"
            class="icon-btn aa"
            class:on={aaOpen}
            title="Оформление (A)"
            aria-label="Оформление"
            onclick={() => (aaOpen = !aaOpen)}>Aa</button
          >
        {/if}
        <button type="button" class="icon-btn immersive-btn" title="Погружение (F)" aria-label="Режим погружения" onclick={() => toggleImmersive()}>
          <svg viewBox="0 0 24 24" aria-hidden="true"><path d="M4 9V4h5M20 9V4h-5M4 15v5h5M20 15v5h-5" /></svg>
        </button>
        <div class="more-wrap">
          <button
            type="button"
            class="icon-btn"
            class:on={moreOpen}
            title="Ещё"
            aria-label="Ещё"
            aria-expanded={moreOpen}
            onclick={() => (moreOpen = !moreOpen)}
          >
            <svg viewBox="0 0 24 24" aria-hidden="true"><circle cx="5" cy="12" r="1.3" /><circle cx="12" cy="12" r="1.3" /><circle cx="19" cy="12" r="1.3" /></svg>
          </button>
          {#if moreOpen}
            <!-- svelte-ignore a11y_click_events_have_key_events -->
            <!-- svelte-ignore a11y_no_static_element_interactions -->
            <div class="menu-back" onclick={() => (moreOpen = false)}></div>
            <div class="menu" role="menu">
              {#if fmt !== "typst"}
                <button
                  type="button"
                  role="menuitem"
                  onclick={() => {
                    moreOpen = false;
                    setPanel(true, "translate");
                  }}>Перевод книги…</button
                >
              {/if}
              {#if fmt !== "typst"}
                <button
                  type="button"
                  role="menuitem"
                  onclick={() => {
                    moreOpen = false;
                    recapOpen = true;
                  }}>Ранее в книге… <kbd>без спойлеров</kbd></button
                >
                <button type="button" role="menuitem" onclick={toggleTts}>
                  {tts.active ? "Остановить чтение вслух" : "Читать вслух"} <kbd>S</kbd>
                </button>
                <button type="button" role="menuitem" onclick={openRsvp}>Быстрое чтение (RSVP) <kbd>R</kbd></button>
              {/if}
              <button
                type="button"
                role="menuitem"
                onclick={() => {
                  moreOpen = false;
                  ambientOpen = true;
                }}
              >
                Фоновый звук… {#if ambient.current}<kbd>♪ {AMBIENT_OPTIONS.find((o) => o.id === ambient.current)?.label}</kbd>{/if}
              </button>
              <div class="menu-sep"></div>
              <button type="button" role="menuitem" onclick={() => void toggleFullscreen()}>
                {isFullscreen ? "Выйти из полного экрана" : "Во весь экран"} <kbd>F11</kbd>
              </button>
              {#if fmt !== "typst"}
                <button
                  type="button"
                  role="menuitem"
                  onclick={() => {
                    moreOpen = false;
                    void exportNotes();
                  }}>Конспект в Markdown</button
                >
                <button type="button" role="menuitem" disabled={typstExportBusy} onclick={() => void exportCurrentBookToTypst()}>
                  {typstExportBusy ? "Экспорт в Typst…" : "Экспорт в Typst"}
                </button>
              {/if}
              <div class="menu-sep"></div>
              <a role="menuitem" href="/settings">Настройки</a>
            </div>
          {/if}
        </div>
      </div>
    </header>

    <div class="read-body">
      <aside class="side" aria-label="Навигация по книге">
        <nav class="side-tabs" aria-label="Разделы панели">
          <button type="button" class:on={panelTab === "toc"} onclick={() => (panelTab = "toc")}>Оглавление</button>
          {#if fmt !== "typst"}
            <button type="button" class:on={panelTab === "notes"} onclick={() => (panelTab = "notes")}>Заметки</button>
            <button type="button" class:on={panelTab === "search"} onclick={() => (panelTab = "search")}>Поиск</button>
            <button type="button" class:on={panelTab === "glossary"} onclick={() => (panelTab = "glossary")}>Герои</button>
            <button type="button" class:on={panelTab === "translate"} onclick={() => (panelTab = "translate")}>Перевод</button>
          {/if}
          <button type="button" class="side-close" aria-label="Закрыть панель" onclick={() => setPanel(false)}>×</button>
        </nav>

        <div class="side-body">
          {#if panelTab === "toc"}
            {#if fmt === "pdf"}
              {#if pdfNumPages > 0}
                <form
                  class="goto"
                  onsubmit={(e) => {
                    e.preventDefault();
                    const v = Number(new FormData(e.currentTarget).get("page"));
                    if (v >= 1 && v <= pdfNumPages) {
                      pdfPage = Math.round(v);
                      finishNavigation();
                    }
                  }}
                >
                  <label>
                    Страница
                    <input name="page" type="number" min="1" max={pdfNumPages} value={pdfPage} />
                  </label>
                  <span>из {pdfNumPages}</span>
                  <button type="submit">Перейти</button>
                </form>
              {/if}
              {#if pdfOutline.length === 0}
                <p class="empty-hint">В этом PDF нет оглавления. Используйте поиск или полосу прогресса внизу.</p>
              {:else}
                <ReaderOutlineTree items={pdfTreeItems} activeId={activePdfTreeId} onSelect={selectPdfChapter} />
              {/if}
            {:else if fmt === "typst"}
              {#if typstOutline.length === 0}
                <p class="empty-hint">
                  Заголовки из <code>=</code>, <code>==</code>… появятся здесь после загрузки <strong>book.typ</strong>.
                </p>
              {:else}
                <ReaderOutlineTree items={typstTreeItems} onSelect={selectTypstChapter} />
              {/if}
            {:else if navApi}
              {#if navApi.toc.length === 0}
                <p class="empty-hint">Оглавление пустое.</p>
              {:else}
                <ReaderOutlineTree items={navTreeItems} activeId={activeNavTreeId} onSelect={selectNavChapter} />
              {/if}
            {:else}
              <p class="empty-hint">Загрузка…</p>
            {/if}
          {:else if panelTab === "notes"}
            {#if meta}
              <ReaderNotesPanel
                {meta}
                {displayTitle}
                {displayAuthor}
                review={reviewDraft}
                onReviewInput={(v) => scheduleReviewSave(bookPath, v)}
                onGoHighlight={(h) => void goToHighlight(h)}
                onEditHighlight={(h, rect) => (hlPopover = { id: h.id, rect, editNote: false })}
                onExport={() => void exportNotes()}
                onCopyMarkdown={() => void copyNotes()}
              />
            {:else}
              <p class="empty-hint">Метаданные загружаются…</p>
            {/if}
          {:else if panelTab === "search"}
            <ReaderSearchPanel api={navApi} initialQuery={searchSeed} onPicked={finishNavigation} />
          {:else if panelTab === "glossary"}
            <GlossaryPanel {bookPath} version={glossaryVersion} progress={position?.progress ?? null} />
          {:else if panelTab === "translate" && fmt !== "typst"}
            <div class="translate-tab">
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
                <div class="pdf-inline-bar">
                  <button type="button" class="pdf-export-btn" onclick={() => void exportPdfTranslation()} disabled={pdfExportBusy}>
                    {pdfExportBusy ? "Сборка PDF…" : "Сохранить translate_*.pdf в папку книги"}
                  </button>
                  <label class="pdf-inline-lab">
                    <input type="checkbox" bind:checked={pdfInlineShow} />
                    Превью слоя перевода
                  </label>
                </div>
              {/if}
            </div>
          {/if}
        </div>
      </aside>

      <button type="button" class="side-backdrop" aria-label="Закрыть панель" onclick={() => setPanel(false)}></button>

      <section class="read-stage" bind:this={stageEl}>
        <div class="stage-frame">
          {#if fmt === "pdf"}
            <PdfViewer
              relativePath={bookPath}
              bind:pageNum={pdfPage}
              {palette}
              {highlights}
              {onPdfReady}
              {onPosition}
              onReaderApi={onReaderNavApi}
              {onSelection}
              {onHighlightClick}
              {onCenterTap}
              pdfTranslationPanel={transLayout === "trans"}
              translateRunKey={transRunKey}
              translateSource={transSource}
              translateTarget={transTarget}
              onTranslateActivity={handleTranslateActivity}
              {pdfInlineSpans}
              {pdfInlineShow}
            />
          {:else if fmt === "epub"}
            <EpubViewer
              relativePath={bookPath}
              initialLocation={meta?.lastReadLocation ?? ""}
              {palette}
              {highlights}
              onReadingProgress={onReadingLocationSave}
              {onPosition}
              onReaderApi={onReaderNavApi}
              {onSelection}
              {onHighlightClick}
              {onCenterTap}
              translateRunKey={transRunKey}
              translateSource={transSource}
              translateTarget={transTarget}
              translateLayout={transLayout === "split" ? "trans" : transLayout}
              onTranslateActivity={handleTranslateActivity}
            />
          {:else if fmt === "typst"}
            <TypstViewer
              relativePath={bookPath}
              {displayTitle}
              onOutline={(o) => (typstOutline = o)}
              goToLineRequest={typstJumpLine}
              onGoToLineHandled={() => (typstJumpLine = null)}
            />
          {:else}
            <Fb2Viewer
              relativePath={bookPath}
              initialLocation={meta?.lastReadLocation ?? ""}
              {palette}
              {highlights}
              onReadingProgress={onReadingLocationSave}
              {onPosition}
              onReaderApi={onReaderNavApi}
              {onSelection}
              {onHighlightClick}
              {onCenterTap}
              translateRunKey={transRunKey}
              translateSource={transSource}
              translateTarget={transTarget}
              translateLayout={transLayout}
              onTranslateActivity={handleTranslateActivity}
            />
          {/if}
          {#if reading.s.focus === "line" && fmt !== "typst"}
            <ReadingRuler host={stageEl} band={rulerBand} tint={palette.bg} />
          {/if}
        </div>
        {#if fmt !== "typst"}
          <ReaderStatusBar {position} quiet={immersive} onSeek={(f) => void navApi?.seek?.(f)} />
        {/if}
      </section>
    </div>
  </div>

  {#if aaOpen}
    <TypographyPanel format={fmt} onClose={() => (aaOpen = false)} />
  {/if}

  <ReaderSelectionToolbar
    visible={!!selection && !hlPopover && !quoteOpen && !lookup}
    rect={selection?.rect ?? null}
    short={shortSelection}
    onHighlight={(c) => void addHighlight(c)}
    onNote={noteFromSelection}
    onQuote={quoteFromSelection}
    onCopy={() => void copySelection()}
    onSearch={searchSelection}
    onTranslate={() => lookupSelection("word")}
    onWho={() => lookupSelection("who")}
  />

  {#if lookup && bookPath}
    <LookupPopover
      kind={lookup.kind}
      term={lookup.term}
      context={lookup.context}
      rect={lookup.rect}
      api={navApi}
      {bookPath}
      bookTitle={displayTitle}
      progress={position?.progress ?? null}
      targetLang={transTarget}
      onClose={() => (lookup = null)}
      onGlossaryChanged={() => glossaryVersion++}
    />
  {/if}

  {#if recapOpen && navApi && bookPath}
    <RecapDialog api={navApi} {bookPath} bookTitle={displayTitle} onClose={() => (recapOpen = false)} />
  {/if}

  {#if hlPopover && popHighlight}
    <HighlightPopover
      highlight={popHighlight}
      rect={hlPopover.rect}
      editNote={hlPopover.editNote}
      onChange={(p) => updateHighlight(popHighlight.id, p)}
      onDelete={() => deleteHighlight(popHighlight.id)}
      onQuote={() => {
        quoteText = popHighlight.text;
        quoteChapter = popHighlight.chapterLabel ?? "";
        hlPopover = null;
        quoteOpen = true;
      }}
      onClose={() => (hlPopover = null)}
    />
  {/if}

  <TtsBar />

  {#if rsvpOpen && navApi}
    <RsvpOverlay api={navApi} {palette} onClose={() => (rsvpOpen = false)} />
  {/if}

  {#if ambientOpen}
    <AmbientPanel bookSound={meta?.ambientSound ?? null} onBind={bindAmbient} onClose={() => (ambientOpen = false)} />
  {/if}

  <CommentDialog
    open={commentOpen}
    excerpt={quoteText}
    pageHint={position?.chapterLabel ?? ""}
    onClose={() => (commentOpen = false)}
    onSave={addComment}
  />

  <QuoteDialog
    open={quoteOpen}
    {quoteText}
    bookTitle={displayTitle}
    bookAuthor={displayAuthor}
    pageLabel={fmt === "pdf" ? `стр. ${pdfPage}` : ""}
    chapterLabel={quoteChapter}
    onClose={() => (quoteOpen = false)}
    onSave={addQuote}
  />
{/if}

<style>
  .read-shell {
    --side-w: 21rem;
    --top-h: 3.4rem;
    display: flex;
    flex-direction: column;
    height: 100vh;
    height: 100dvh;
    min-height: 0;
    position: relative;
    isolation: isolate;
    background: var(--page-bg);
    transition: background 0.4s ease;
  }

  .read-shell::before {
    content: "";
    position: absolute;
    inset: 0;
    z-index: -1;
    pointer-events: none;
    background: radial-gradient(ellipse 60rem 22rem at 50% -8rem, var(--cover-glow, transparent), transparent 70%);
  }

  /* ——— Шапка ——— */
  .read-top {
    height: var(--top-h);
    flex-shrink: 0;
    display: flex;
    align-items: center;
    gap: 0.25rem;
    padding: env(safe-area-inset-top) 0.6rem 0;
    box-sizing: content-box;
    border-bottom: 1px solid color-mix(in srgb, var(--chrome-muted) 18%, transparent);
    background: var(--page-bg);
    z-index: 50;
    transition:
      transform 0.28s ease,
      opacity 0.28s ease;
  }

  .immersive .read-top {
    position: absolute;
    left: 0;
    right: 0;
    top: 0;
    transform: translateY(-110%);
    opacity: 0;
  }

  .immersive.peek .read-top {
    transform: none;
    opacity: 1;
    box-shadow: var(--shadow-soft);
  }

  .icon-btn {
    position: relative;
    display: grid;
    place-items: center;
    min-width: 2.3rem;
    height: 2.3rem;
    padding: 0 0.35rem;
    border-radius: 12px;
    border: 1px solid transparent;
    background: transparent;
    color: var(--chrome-text);
    cursor: pointer;
    transition:
      background 0.15s ease,
      color 0.15s ease;
  }

  .icon-btn:hover {
    background: color-mix(in srgb, var(--accent) 14%, transparent);
  }

  .icon-btn.on {
    background: color-mix(in srgb, var(--accent) 24%, transparent);
    color: var(--chrome-text);
  }

  .icon-btn svg {
    width: 1.2rem;
    height: 1.2rem;
    fill: none;
    stroke: currentColor;
    stroke-width: 1.8;
    stroke-linecap: round;
    stroke-linejoin: round;
  }

  .icon-btn.aa {
    font-family: "Literata Variable", Georgia, serif;
    font-size: 1.05rem;
    font-weight: 500;
  }

  .badge {
    position: absolute;
    top: 0.1rem;
    right: 0.05rem;
    min-width: 1rem;
    height: 1rem;
    padding: 0 0.2rem;
    border-radius: 999px;
    background: var(--accent-2);
    color: var(--page-bg);
    font-size: 0.6rem;
    font-weight: 700;
    line-height: 1rem;
  }

  .title-block {
    flex: 1;
    min-width: 0;
    text-align: center;
    padding: 0 0.5rem;
  }

  .read-title {
    margin: 0;
    font-size: 0.92rem;
    font-weight: 600;
    color: var(--chrome-text);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .read-author {
    margin: 0;
    font-size: 0.74rem;
    color: var(--chrome-muted);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .top-actions {
    display: flex;
    align-items: center;
    gap: 0.15rem;
  }

  .more-wrap {
    position: relative;
  }

  .menu-back {
    position: fixed;
    inset: 0;
    z-index: 60;
  }

  .menu {
    position: absolute;
    right: 0;
    top: calc(100% + 0.4rem);
    z-index: 61;
    min-width: 15rem;
    display: flex;
    flex-direction: column;
    padding: 0.35rem;
    border-radius: var(--radius-md);
    background: var(--panel-elevated);
    border: 1px solid var(--toolbar-border);
    box-shadow: var(--shadow-float);
    animation: pop 0.14s ease-out;
  }

  .menu button,
  .menu a {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 1rem;
    text-align: left;
    border: none;
    background: transparent;
    color: var(--text-soft);
    padding: 0.55rem 0.7rem;
    border-radius: 10px;
    font-size: 0.86rem;
    cursor: pointer;
    text-decoration: none;
  }

  .menu button:hover,
  .menu a:hover {
    background: var(--panel-soft);
  }

  .menu button:disabled {
    opacity: 0.5;
  }

  .menu kbd {
    font-size: 0.68rem;
    color: var(--muted);
    font-family: inherit;
  }

  .menu-sep {
    height: 1px;
    margin: 0.3rem 0.4rem;
    background: var(--border-soft);
  }

  /* ——— Тело ——— */
  .read-body {
    flex: 1;
    min-height: 0;
    display: flex;
    position: relative;
  }

  .side {
    width: 0;
    flex-shrink: 0;
    overflow: hidden;
    display: flex;
    flex-direction: column;
    background: color-mix(in srgb, var(--panel-soft) 70%, var(--page-bg));
    border-right: 1px solid transparent;
    transition:
      width 0.26s ease,
      border-color 0.26s ease;
  }

  .panel-open .side {
    width: var(--side-w);
    border-right-color: color-mix(in srgb, var(--border-soft) 70%, transparent);
  }

  .side-tabs {
    display: flex;
    align-items: center;
    gap: 0.1rem;
    overflow-x: auto;
    scrollbar-width: none;
    padding: 0.55rem 0.6rem 0.4rem;
    min-width: var(--side-w);
    box-sizing: border-box;
  }

  .side-tabs button {
    border: none;
    background: transparent;
    color: var(--muted);
    padding: 0.35rem 0.5rem;
    border-radius: 999px;
    font-size: 0.8rem;
    cursor: pointer;
    white-space: nowrap;
  }

  .side-tabs button.on {
    background: var(--elevated-soft);
    color: var(--text-soft);
    font-weight: 600;
    box-shadow: 0 1px 4px rgba(0, 0, 0, 0.06);
  }

  .side-tabs .side-close {
    margin-left: auto;
    font-size: 1.2rem;
    line-height: 1;
    display: none;
  }

  .side-body {
    flex: 1;
    min-height: 0;
    overflow: auto;
    padding: 0.5rem 0.8rem 1.5rem;
    min-width: var(--side-w);
    box-sizing: border-box;
  }

  .side-backdrop {
    display: none;
  }

  .goto {
    display: flex;
    align-items: center;
    gap: 0.45rem;
    margin-bottom: 0.8rem;
    font-size: 0.8rem;
    color: var(--muted);
  }

  .goto label {
    display: flex;
    align-items: center;
    gap: 0.4rem;
  }

  .goto input {
    width: 4.2rem;
    border: 1px solid var(--border-soft);
    background: var(--elevated-soft);
    color: var(--text-soft);
    border-radius: 8px;
    padding: 0.3rem 0.4rem;
    font: inherit;
  }

  .goto button {
    margin-left: auto;
    border: 1px solid var(--border-soft);
    background: var(--elevated-soft);
    color: var(--text-soft);
    border-radius: 999px;
    padding: 0.25rem 0.65rem;
    font-size: 0.78rem;
    cursor: pointer;
  }

  .empty-hint {
    margin: 0.5rem 0;
    font-size: 0.84rem;
    line-height: 1.5;
    color: var(--muted);
  }

  .translate-tab {
    display: flex;
    flex-direction: column;
    gap: 0.6rem;
  }

  .pdf-inline-bar {
    display: flex;
    flex-direction: column;
    gap: 0.45rem;
  }

  .pdf-export-btn {
    border: 1px solid var(--border-soft);
    background: var(--elevated-soft);
    color: var(--text-soft);
    border-radius: var(--radius-sm);
    padding: 0.5rem 0.65rem;
    font-size: 0.8rem;
    cursor: pointer;
    text-align: left;
  }

  .pdf-inline-lab {
    display: flex;
    align-items: center;
    gap: 0.45rem;
    font-size: 0.8rem;
    color: var(--muted);
  }

  .read-stage {
    flex: 1;
    min-width: 0;
    min-height: 0;
    display: flex;
    flex-direction: column;
    position: relative;
  }

  .stage-frame {
    flex: 1;
    min-height: 0;
    position: relative;
    display: flex;
    flex-direction: column;
  }

  .stage-frame > :global(*:not(.ruler)) {
    flex: 1;
    min-height: 0;
  }

  @keyframes pop {
    from {
      opacity: 0;
      transform: translateY(-4px);
    }
  }

  /* ——— Узкие экраны: панель поверх текста ——— */
  @media (max-width: 1100px) {
    .side {
      position: absolute;
      z-index: 40;
      left: 0;
      top: 0;
      bottom: 0;
      width: min(var(--side-w), 88vw);
      transform: translateX(-102%);
      transition:
        transform 0.26s ease,
        box-shadow 0.26s ease;
      background: var(--panel-elevated);
    }

    .panel-open .side {
      width: min(var(--side-w), 88vw);
      transform: none;
      box-shadow: var(--shadow-float);
    }

    .side-tabs,
    .side-body {
      min-width: 0;
    }

    .side-tabs .side-close {
      display: block;
    }

    .side-backdrop {
      display: block;
      position: absolute;
      inset: 0;
      z-index: 39;
      border: none;
      background: rgba(20, 16, 30, 0.28);
      opacity: 0;
      pointer-events: none;
      transition: opacity 0.26s ease;
    }

    .panel-open .side-backdrop {
      opacity: 1;
      pointer-events: auto;
    }
  }

  @media (max-width: 600px) {
    .read-top {
      padding-left: max(0.35rem, env(safe-area-inset-left));
      padding-right: max(0.35rem, env(safe-area-inset-right));
    }

    .read-author {
      display: none;
    }

    .immersive-btn {
      display: none;
    }
  }
</style>
