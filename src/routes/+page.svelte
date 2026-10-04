<script lang="ts">
  import { onDestroy, onMount } from "svelte";
  import { goto } from "$app/navigation";
  import { invoke } from "@tauri-apps/api/core";
  import { open } from "@tauri-apps/plugin-dialog";
  import type { BookMeta, LibraryMetadata, LibrarySnapshot, ReadingStatus, Shelf } from "$lib/types";
  import { READING_STATUS_OPTIONS } from "$lib/types";
  import { getBookFormat } from "$lib/bookFormat";
  import { bookOnShelf, effectiveShelfIds } from "$lib/library/shelves";
  import {
    fetchLibrarySnapshot,
    getCachedLibrarySnapshot,
    setCachedLibraryMetadata,
  } from "$lib/library/librarySnapshotCache";
  import {
    type LibrarySortMode,
    DEFAULT_LIBRARY_SORT,
    LIBRARY_SORT_LABELS,
    readLibrarySort,
    writeLibrarySort,
    readLibraryView,
    writeLibraryView,
    type LibraryViewMode,
  } from "$lib/library/libraryListPrefs";
  import {
    STATUS_LABELS,
    bookProgress,
    bookTitle,
    dustLabel,
    effectiveStatus,
    pluralBooks,
    titleFromPath,
    type ShelfStatus,
  } from "$lib/library/bookInfo";
  import HomeSidebar from "$lib/components/home/HomeSidebar.svelte";
  import HomeHero from "$lib/components/home/HomeHero.svelte";
  import Book3D from "$lib/components/home/Book3D.svelte";
  import BookEditDialog from "$lib/components/home/BookEditDialog.svelte";
  import SpineShelf from "$lib/components/SpineShelf.svelte";
  import BookFullTranslateModal from "$lib/components/BookFullTranslateModal.svelte";
  import DreamSelect from "$lib/components/DreamSelect.svelte";
  import { isTauriRuntime } from "$lib/isTauri";
  import { exportBookToTypst } from "$lib/typst/exportToTypst";
  import { toast, toastError } from "$lib/ui/toast.svelte";
  import type { CoverTone } from "$lib/reading/palette";

  let snapshot = $state<LibrarySnapshot | null>(getCachedLibrarySnapshot());
  let shelfFilter = $state<string>("all");
  let banner = $state<string | null>(null);
  let searchQuery = $state("");
  let statusFilter = $state<ShelfStatus | "all">("all");
  let viewMode = $state<LibraryViewMode>("grid");
  let librarySort = $state<LibrarySortMode>(DEFAULT_LIBRARY_SORT);
  let dragActive = $state(false);
  let menuOpen = $state(false);
  let searchEl = $state<HTMLInputElement | null>(null);
  let tone = $state<CoverTone | null>(null);
  let scrolled = $state(false);

  let ctxMenu = $state<{ x: number; y: number; path: string } | null>(null);
  let bookAction = $state<{ kind: "hide" | "delete"; path: string } | null>(null);
  let bookActionBusy = $state(false);
  let bookActionError = $state<string | null>(null);
  let editPath = $state<string | null>(null);
  let fullTranslateBook = $state<string | null>(null);

  const pendingMetaPatches = new Map<string, Partial<BookMeta>>();
  let metaPatchTimer: ReturnType<typeof setTimeout> | null = null;

  const IMPORTANCE_ORDER: Record<string, number> = { essential: 0, high: 1, normal: 2, low: 3 };
  const STATUS_FILTERS: (ShelfStatus | "all")[] = ["all", "reading", "new", "want", "done", "dropped"];

  onMount(() => {
    librarySort = readLibrarySort();
    viewMode = readLibraryView();
  });

  function setViewMode(mode: LibraryViewMode) {
    viewMode = mode;
    writeLibraryView(mode);
  }

  function setLibrarySort(mode: LibrarySortMode) {
    librarySort = mode;
    writeLibrarySort(mode);
  }

  function openedAtTs(meta: BookMeta | undefined): number {
    const t = meta?.lastOpenedAt ? Date.parse(meta.lastOpenedAt) : 0;
    return Number.isFinite(t) ? t : 0;
  }

  function cmpBookPaths(a: string, b: string): number {
    const ma = snapshot!.metadata.books[a];
    const mb = snapshot!.metadata.books[b];
    const ta = bookTitle(a, ma).toLowerCase();
    const tb = bookTitle(b, mb).toLowerCase();
    switch (librarySort) {
      case "recent": {
        const d = openedAtTs(mb) - openedAtTs(ma);
        if (d) return d;
        break;
      }
      case "title":
        return ta.localeCompare(tb, "ru");
      case "title_desc":
        return tb.localeCompare(ta, "ru");
      case "added": {
        const d = (mb?.addedAtMs ?? 0) - (ma?.addedAtMs ?? 0);
        if (d) return d;
        break;
      }
      case "progress": {
        const d = (bookProgress(mb) ?? -1) - (bookProgress(ma) ?? -1);
        if (d) return d;
        break;
      }
      case "importance": {
        const d =
          (IMPORTANCE_ORDER[String(ma?.importance ?? "normal")] ?? 2) -
          (IMPORTANCE_ORDER[String(mb?.importance ?? "normal")] ?? 2);
        if (d) return d;
        break;
      }
    }
    return ta.localeCompare(tb, "ru");
  }

  const shelvesSorted = $derived(snapshot ? [...snapshot.metadata.shelves].sort((a, b) => a.order - b.order) : []);

  const shelfPaths = $derived.by(() => {
    if (!snapshot) return [];
    if (shelfFilter === "hidden") return snapshot.hiddenBookPaths;
    if (shelfFilter === "all") return snapshot.bookPaths;
    return snapshot.bookPaths.filter((p) => bookOnShelf(snapshot!.metadata.books[p], shelfFilter));
  });

  const displayedBooks = $derived.by(() => {
    if (!snapshot) return [];
    let paths = shelfPaths;
    if (statusFilter !== "all") paths = paths.filter((p) => effectiveStatus(snapshot!.metadata.books[p]) === statusFilter);
    const query = searchQuery.trim().toLocaleLowerCase("ru");
    if (query) {
      paths = paths.filter((p) => {
        const meta = snapshot!.metadata.books[p];
        return [titleFromPath(p), meta?.title, meta?.author]
          .filter(Boolean)
          .join(" ")
          .toLocaleLowerCase("ru")
          .includes(query);
      });
    }
    return paths.slice().sort(cmpBookPaths);
  });

  const continueBooks = $derived.by(() => {
    if (!snapshot) return [];
    return snapshot.bookPaths
      .map((path) => ({ path, meta: snapshot!.metadata.books[path]! }))
      .filter((b) => b.meta?.lastOpenedAt && effectiveStatus(b.meta) === "reading")
      .sort((a, b) => Date.parse(b.meta.lastOpenedAt!) - Date.parse(a.meta.lastOpenedAt!))
      .slice(0, 5);
  });

  const showHero = $derived(shelfFilter === "all" && statusFilter === "all" && !searchQuery.trim());

  const statusCounts = $derived.by(() => {
    const c: Record<string, number> = {};
    if (!snapshot || shelfFilter === "hidden") return c;
    for (const p of shelfPaths) {
      const st = effectiveStatus(snapshot.metadata.books[p]);
      c[st] = (c[st] ?? 0) + 1;
    }
    return c;
  });

  const heading = $derived(
    searchQuery.trim()
      ? "Поиск"
      : shelfFilter === "all"
        ? "Вся библиотека"
        : shelfFilter === "hidden"
          ? "Скрытые книги"
          : (shelvesSorted.find((s) => s.id === shelfFilter)?.name ?? "Полка"),
  );

  function countForShelf(shelfId: string): number {
    const snap = snapshot;
    if (!snap) return 0;
    if (shelfId === "all") return snap.bookPaths.length;
    if (shelfId === "hidden") return snap.hiddenBookPaths.length;
    return snap.bookPaths.filter((p) => bookOnShelf(snap.metadata.books[p], shelfId)).length;
  }

  function bookIsHidden(path: string): boolean {
    return snapshot?.hiddenBookPaths.includes(path) ?? false;
  }

  // ——— Данные ———
  async function refresh() {
    try {
      banner = null;
      snapshot = await fetchLibrarySnapshot();
    } catch (e) {
      banner = String(e);
    }
  }

  async function pickFolder() {
    const dir = await open({ directory: true, multiple: false });
    if (typeof dir !== "string") return;
    try {
      await invoke("set_library_root", { path: dir });
      await refresh();
    } catch (e) {
      banner = String(e);
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
    void persist({ ...snapshot.metadata, books }).catch((e) => toastError(e, "Сохранение"));
  }

  /** Название и автор из файла книги — только если пользователь их не задавал. */
  function applyDiscoveredMeta(path: string, m: { title: string | null; author: string | null }) {
    const cur = snapshot?.metadata.books[path];
    if (!cur || cur.autoMetaDone) return;
    const patch: Partial<BookMeta> = { autoMetaDone: true };
    if (!cur.title?.trim() && m.title) patch.title = m.title;
    if (!cur.author?.trim() && m.author) patch.author = m.author;
    pendingMetaPatches.set(path, patch);
    if (metaPatchTimer) clearTimeout(metaPatchTimer);
    metaPatchTimer = setTimeout(flushMetaPatches, 700);
  }

  function flushMetaPatches() {
    metaPatchTimer = null;
    if (!snapshot || pendingMetaPatches.size === 0) return;
    const books = { ...snapshot.metadata.books };
    for (const [path, patch] of pendingMetaPatches) {
      const current = books[path];
      if (current) books[path] = { ...current, ...patch };
    }
    pendingMetaPatches.clear();
    void persist({ ...snapshot.metadata, books }).catch(() => {});
  }

  function setBookStatus(path: string, status: ReadingStatus) {
    const patch: Partial<BookMeta> = { status };
    if (status === "done") patch.finishedAt = new Date().toISOString();
    patchBook(path, patch);
  }

  // ——— Полки ———
  async function addShelf(name: string) {
    if (!snapshot) return;
    const order = snapshot.metadata.shelves.reduce((m, s) => Math.max(m, s.order), -1) + 1;
    const shelves: Shelf[] = [...snapshot.metadata.shelves, { id: crypto.randomUUID(), name, order }];
    await persist({ ...snapshot.metadata, shelves });
  }

  async function removeShelf(shelfId: string) {
    if (!snapshot || shelfId === "default") return;
    const shelves = snapshot.metadata.shelves.filter((s) => s.id !== shelfId);
    const books = { ...snapshot.metadata.books };
    for (const k of Object.keys(books)) {
      const b = books[k]!;
      let ids = effectiveShelfIds(b).filter((id) => id !== shelfId);
      if (ids.length === 0) ids = ["default"];
      books[k] = { ...b, shelfIds: ids, shelfId: ids[0] ?? "default" };
    }
    if (shelfFilter === shelfId) shelfFilter = "all";
    await persist({ ...snapshot.metadata, shelves, books });
  }

  function selectShelf(id: string) {
    shelfFilter = id;
    statusFilter = "all";
    menuOpen = false;
  }

  // ——— Импорт: кнопка и перетаскивание файлов в окно ———
  async function importPaths(paths: string[]) {
    if (!paths.length) return;
    if (!snapshot?.libraryRoot) {
      toast("Сначала выберите папку библиотеки", "error");
      return;
    }
    try {
      const added = await invoke<string[]>("import_books", { paths });
      await refresh();
      if (added.length) toast(`Добавлено: ${added.length} ${pluralBooks(added.length)}`, "success");
      else toast("Подходящих файлов нет — поддерживаются PDF, EPUB и FB2", "info");
    } catch (e) {
      toastError(e, "Импорт");
    }
  }

  async function pickBooks() {
    const picked = await open({ multiple: true, filters: [{ name: "Книги", extensions: ["pdf", "epub", "fb2"] }] });
    if (!picked) return;
    await importPaths(Array.isArray(picked) ? picked : [picked]);
  }

  onMount(() => {
    if (!isTauriRuntime()) return;
    let unlisten: (() => void) | null = null;
    let alive = true;
    void import("@tauri-apps/api/webview").then(async ({ getCurrentWebview }) => {
      const un = await getCurrentWebview().onDragDropEvent((event) => {
        const t = event.payload.type;
        if (t === "enter" || t === "over") dragActive = true;
        else if (t === "leave") dragActive = false;
        else if (t === "drop") {
          dragActive = false;
          void importPaths(event.payload.paths);
        }
      });
      if (alive) unlisten = un;
      else un();
    });
    return () => {
      alive = false;
      unlisten?.();
    };
  });

  // ——— Действия с книгой ———
  function openBook(p: string) {
    if (!getBookFormat(p)) return;
    void goto("/read?path=" + encodeURIComponent(p));
  }

  function onCardClick(e: MouseEvent, p: string) {
    if (bookIsHidden(p)) openMenu(e, p);
    else openBook(p);
  }

  function openMenu(e: MouseEvent, p: string) {
    e.preventDefault();
    const w = 240;
    const h = 420;
    ctxMenu = {
      x: Math.max(12, Math.min(e.clientX, window.innerWidth - w - 12)),
      y: Math.max(12, Math.min(e.clientY, window.innerHeight - h - 12)),
      path: p,
    };
  }

  function closeCtx() {
    ctxMenu = null;
  }

  function startBookAction(kind: "hide" | "delete") {
    if (!ctxMenu) return;
    bookAction = { kind, path: ctxMenu.path };
    bookActionError = null;
    closeCtx();
  }

  function closeBookAction() {
    if (bookActionBusy) return;
    bookAction = null;
    bookActionError = null;
  }

  async function setBookHidden(path: string, hidden: boolean) {
    if (!snapshot) return;
    const current = snapshot.metadata.books[path];
    if (!current) return;
    const books = { ...snapshot.metadata.books, [path]: { ...current, hidden } };
    await persist({ ...snapshot.metadata, books });
    await refresh();
  }

  async function restoreBookFromMenu() {
    if (!ctxMenu) return;
    const path = ctxMenu.path;
    closeCtx();
    try {
      await setBookHidden(path, false);
      toast("Книга снова на полке", "success");
    } catch (e) {
      toastError(e, "Не удалось вернуть книгу");
    }
  }

  async function confirmBookAction() {
    if (!bookAction) return;
    const { kind, path } = bookAction;
    bookActionBusy = true;
    bookActionError = null;
    try {
      if (kind === "hide") await setBookHidden(path, true);
      else {
        await invoke("delete_library_book", { relativePath: path });
        await refresh();
      }
      bookAction = null;
    } catch {
      bookActionError =
        kind === "delete"
          ? "Не удалось удалить книгу. Проверьте доступ к файлу и попробуйте снова."
          : "Не удалось скрыть книгу. Попробуйте снова.";
    } finally {
      bookActionBusy = false;
    }
  }

  async function exportBookToTypstFromMenu() {
    if (!ctxMenu || !snapshot) return;
    const path = ctxMenu.path;
    const fmt = getBookFormat(path);
    if (!fmt || fmt === "typst") return;
    const style = snapshot.metadata.books[path]?.typstStyleRelativePath;
    closeCtx();
    try {
      const { mainRelativePath } = await exportBookToTypst(path, fmt, snapshot, style);
      await refresh();
      await goto("/read?path=" + encodeURIComponent(mainRelativePath));
    } catch (e) {
      toastError(e, "Экспорт в Typst");
    }
  }

  function progressLabel(meta: BookMeta | undefined): string {
    const st = effectiveStatus(meta);
    if (st === "done") return "Прочитано";
    const p = bookProgress(meta);
    if (st === "reading" && p != null) return `${Math.round(p * 100)}%`;
    return STATUS_LABELS[st];
  }

  // ——— Клавиатура ———
  function onKey(e: KeyboardEvent) {
    const typing = (e.target as HTMLElement)?.closest?.("input, textarea, select");
    if (e.key === "Escape") {
      if (ctxMenu) closeCtx();
      else if (menuOpen) menuOpen = false;
      else if (typing && searchQuery) searchQuery = "";
      return;
    }
    if (typing || e.ctrlKey || e.metaKey || e.altKey) {
      if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === "k") {
        e.preventDefault();
        searchEl?.focus();
      }
      return;
    }
    if (e.key === "/") {
      e.preventDefault();
      searchEl?.focus();
    }
  }

  $effect(() => {
    if (!snapshot) void refresh();
  });

  $effect(() => {
    if (!ctxMenu) return;
    const onDown = (e: PointerEvent) => {
      const el = document.getElementById("ctx-menu");
      if (el && !el.contains(e.target as Node)) closeCtx();
    };
    document.addEventListener("pointerdown", onDown, true);
    return () => document.removeEventListener("pointerdown", onDown, true);
  });

  onDestroy(() => {
    if (metaPatchTimer) clearTimeout(metaPatchTimer);
    flushMetaPatches();
  });

  const glow = $derived(
    tone ? `hsl(${tone.h} ${Math.max(30, Math.min(tone.s, 60))}% 62% / 0.22)` : "color-mix(in srgb, var(--accent) 16%, transparent)",
  );
  const editMeta = $derived(editPath && snapshot ? (snapshot.metadata.books[editPath] ?? null) : null);
</script>

<svelte:window onkeydown={onKey} />

<div class="app" style:--glow={glow}>
  <HomeSidebar
    shelves={shelvesSorted}
    active={shelfFilter}
    counts={countForShelf}
    open={menuOpen}
    onSelect={selectShelf}
    onAddShelf={(n) => void addShelf(n)}
    onRemoveShelf={(id) => void removeShelf(id)}
    onClose={() => (menuOpen = false)}
  />

  <main class="main">
    <header class="topbar" class:scrolled>
      <button type="button" class="icon menu-btn" aria-label="Меню" onclick={() => (menuOpen = true)}>
        <svg viewBox="0 0 24 24"><path d="M4 7h16M4 12h16M4 17h10" /></svg>
      </button>
      {#if snapshot?.libraryRoot}
        <label class="search">
          <svg viewBox="0 0 24 24" aria-hidden="true"><circle cx="11" cy="11" r="6.5" /><path d="m16 16 4 4" /></svg>
          <span class="sr-only">Поиск по библиотеке</span>
          <input bind:this={searchEl} type="search" placeholder="Найти книгу или автора" bind:value={searchQuery} />
          {#if !searchQuery}<kbd>/</kbd>{/if}
        </label>
      {/if}
      <span class="grow"></span>
      {#if snapshot?.libraryRoot}
        <button type="button" class="icon refresh-btn" title="Обновить библиотеку" aria-label="Обновить" onclick={() => void refresh()}>
          <svg viewBox="0 0 24 24"><path d="M20 11a8 8 0 1 0-2.34 5.66M20 5v6h-6" /></svg>
        </button>
        <button type="button" class="icon" title={`Папка библиотеки: ${snapshot.libraryRoot}`} aria-label="Сменить папку" onclick={() => void pickFolder()}>
          <svg viewBox="0 0 24 24"><path d="M3 7.5h7l2 2h9v9.5H3z" /><path d="M3 7.5V5h7l2 2" /></svg>
        </button>
        <button type="button" class="add" onclick={() => void pickBooks()} title="Или просто перетащите файлы в окно">
          <svg viewBox="0 0 24 24" aria-hidden="true"><path d="M12 5v14M5 12h14" /></svg>
          <span>Добавить книги</span>
        </button>
      {/if}
    </header>

    <!-- Прокручивается только эта область: шапка и боковая панель стоят на месте
         без position: sticky, который WebKitGTK перерисовывает на каждом кадре. -->
    <div class="content" onscroll={(e) => (scrolled = e.currentTarget.scrollTop > 8)}>
    <div class="ambient" aria-hidden="true"></div>

    {#if banner}
      <div class="banner" role="alert">{banner}</div>
    {/if}

    {#if !snapshot?.libraryRoot}
      <section class="welcome">
        <div class="stack" aria-hidden="true">
          <span class="b b1"></span><span class="b b2"></span><span class="b b3"></span><span class="b b4"></span>
          <span class="shelf-line"></span>
        </div>
        <h1>Ваша тихая библиотека</h1>
        <p>
          Выберите папку, где лежат книги — PDF, EPUB или FB2. Reader соберёт их на полки, запомнит, где вы остановились,
          и сохранит ваши заметки. Всё остаётся на вашем устройстве.
        </p>
        <button type="button" class="cta" onclick={() => void pickFolder()}>
          <svg viewBox="0 0 24 24" aria-hidden="true"><path d="M3 7.5h7l2 2h9v9.5H3z" /><path d="M3 7.5V5h7l2 2" /></svg>
          Выбрать папку с книгами
        </button>
      </section>
    {:else}
      {#if showHero}
        <HomeHero books={continueBooks} total={snapshot.bookPaths.length} onOpen={openBook} onTone={(t) => (tone = t)} />
      {/if}

      <section class="library">
        <div class="lib-head">
          <div class="lib-title">
            <h2>{heading}</h2>
            <span class="count">{displayedBooks.length} {pluralBooks(displayedBooks.length)}</span>
          </div>
          <div class="lib-tools">
            <div class="seg" role="group" aria-label="Вид">
              <button type="button" class:on={viewMode === "grid"} title="Обложки" aria-label="Обложки" onclick={() => setViewMode("grid")}>
                <svg viewBox="0 0 24 24"><rect x="4" y="4" width="6.5" height="9" rx="1.2" /><rect x="13.5" y="4" width="6.5" height="9" rx="1.2" /><path d="M4 17h6.5M13.5 17H20" /></svg>
              </button>
              <button type="button" class:on={viewMode === "spines"} title="Корешки на полке" aria-label="Корешки" onclick={() => setViewMode("spines")}>
                <svg viewBox="0 0 24 24"><path d="M5 4v15M9 6v13M13 3v16M17.5 6.5l3 12M3 20.5h18" /></svg>
              </button>
            </div>
            <div class="sort">
              <DreamSelect
                value={librarySort}
                ariaLabel="Порядок книг"
                compact
                options={Object.entries(LIBRARY_SORT_LABELS).map(([value, label]) => ({ value, label }))}
                onChange={(v) => setLibrarySort(v as LibrarySortMode)}
              />
            </div>
          </div>
        </div>

        {#if shelfFilter !== "hidden" && Object.keys(statusCounts).length > 1}
          <div class="chips" role="group" aria-label="Статус чтения">
            {#each STATUS_FILTERS as st (st)}
              {#if st === "all" || statusCounts[st]}
                <button type="button" class="chip" class:on={statusFilter === st} onclick={() => (statusFilter = st)}>
                  {st === "all" ? "Все" : STATUS_LABELS[st]}
                  {#if st !== "all"}<span>{statusCounts[st]}</span>{/if}
                </button>
              {/if}
            {/each}
          </div>
        {/if}

        {#if displayedBooks.length === 0}
          <div class="empty">
            {#if searchQuery.trim() || statusFilter !== "all"}
              <p class="empty-title">Ничего не нашлось</p>
              <p>Попробуйте другой запрос или снимите фильтр.</p>
              <button
                type="button"
                class="ghost"
                onclick={() => {
                  searchQuery = "";
                  statusFilter = "all";
                }}>Сбросить</button
              >
            {:else if shelfFilter === "hidden"}
              <p class="empty-title">Скрытых книг нет</p>
              <p>Сюда попадают книги, которые вы убрали с полок, не удаляя файлы.</p>
            {:else}
              <p class="empty-title">Полка пока пустая</p>
              <p>Добавьте книги кнопкой сверху или перетащите файлы прямо в окно.</p>
            {/if}
          </div>
        {:else if viewMode === "spines"}
          <SpineShelf
            paths={displayedBooks}
            books={snapshot.metadata.books}
            onOpen={(p) => (bookIsHidden(p) ? undefined : openBook(p))}
            onMenu={(e, p) => openMenu(e, p)}
          />
        {:else}
          <ul class="grid">
            {#each displayedBooks as p (p)}
              {@const meta = snapshot.metadata.books[p]}
              {@const st = effectiveStatus(meta)}
              {@const prog = bookProgress(meta)}
              <li class="cell">
                <button
                  type="button"
                  class="card"
                  class:hidden-book={bookIsHidden(p)}
                  title={dustLabel(meta) || bookTitle(p, meta)}
                  onclick={(e) => onCardClick(e, p)}
                  oncontextmenu={(e) => openMenu(e, p)}
                >
                  <div class="book-hover-target">
                    <Book3D
                      path={p}
                      {meta}
                      needMeta={!!meta && !meta.autoMetaDone}
                      onMeta={(m) => applyDiscoveredMeta(p, m)}
                    />
                  </div>
                  {#if st === "done"}
                    <span class="ribbon" title="Прочитано" aria-hidden="true">✓</span>
                  {/if}
                  <span class="card-text">
                    <span class="t">{bookTitle(p, meta)}</span>
                    {#if meta?.author?.trim()}<span class="a">{meta.author}</span>{/if}
                    {#if st === "reading" && prog != null}
                      <span class="bar"><span style:width="{Math.round(prog * 100)}%"></span></span>
                    {/if}
                    <span class="s s-{st}">
                      {progressLabel(meta)}
                      {#if meta?.importance === "essential" || meta?.importance === "high"}<span class="star" title="Важная">★</span>{/if}
                    </span>
                  </span>
                </button>
                <button type="button" class="more" aria-label="Действия с книгой" title="Действия" onclick={(e) => openMenu(e, p)}>
                  <svg viewBox="0 0 24 24"><circle cx="5" cy="12" r="1.4" /><circle cx="12" cy="12" r="1.4" /><circle cx="19" cy="12" r="1.4" /></svg>
                </button>
              </li>
            {/each}
          </ul>
        {/if}
      </section>
    {/if}
    </div>
  </main>
</div>

{#if dragActive}
  <div class="drop" aria-hidden="true">
    <div class="drop-card">
      <svg viewBox="0 0 24 24"><path d="M12 4v11M7 10l5 5 5-5M5 20h14" /></svg>
      <p>Отпустите, чтобы добавить книги</p>
      <small>PDF, EPUB и FB2 скопируются в папку библиотеки</small>
    </div>
  </div>
{/if}

{#if ctxMenu}
  {@const cm = ctxMenu}
  {@const cmMeta = snapshot?.metadata.books[cm.path]}
  {@const cmFmt = getBookFormat(cm.path)}
  <div id="ctx-menu" class="ctx" style="left:{cm.x}px;top:{cm.y}px" role="menu">
    <p class="ctx-title">{bookTitle(cm.path, cmMeta)}</p>
    {#if bookIsHidden(cm.path)}
      <button type="button" role="menuitem" onclick={() => void restoreBookFromMenu()}>Вернуть на полку</button>
      <div class="sep"></div>
      <button type="button" role="menuitem" class="danger" onclick={() => startBookAction("delete")}>Удалить файл…</button>
    {:else}
      <button type="button" role="menuitem" onclick={() => { openBook(cm.path); closeCtx(); }}>Читать</button>
      <button type="button" role="menuitem" onclick={() => { editPath = cm.path; closeCtx(); }}>Изменить сведения…</button>
      <div class="sep"></div>
      <p class="ctx-label">Статус</p>
      <div class="ctx-status">
        {#each READING_STATUS_OPTIONS as o (o.value)}
          <button
            type="button"
            class:on={cmMeta?.status === o.value}
            onclick={() => {
              setBookStatus(cm.path, o.value);
              closeCtx();
            }}>{o.label}</button
          >
        {/each}
      </div>
      <div class="sep"></div>
      {#if isTauriRuntime() && cmFmt === "pdf"}
        <button type="button" role="menuitem" onclick={() => { fullTranslateBook = cm.path; closeCtx(); }}>Перевести книгу…</button>
      {/if}
      {#if isTauriRuntime() && cmFmt && cmFmt !== "typst"}
        <button type="button" role="menuitem" onclick={() => void exportBookToTypstFromMenu()}>Экспорт в Typst…</button>
      {/if}
      <button type="button" role="menuitem" onclick={() => startBookAction("hide")}>Убрать с полок</button>
      <button type="button" role="menuitem" class="danger" onclick={() => startBookAction("delete")}>Удалить файл…</button>
    {/if}
  </div>
{/if}

{#if editPath && editMeta}
  <BookEditDialog
    path={editPath}
    meta={editMeta}
    shelves={shelvesSorted}
    onSave={(patch) => {
      patchBook(editPath!, patch);
      editPath = null;
    }}
    onClose={() => (editPath = null)}
  />
{/if}

{#if fullTranslateBook}
  <BookFullTranslateModal
    bookPath={fullTranslateBook}
    bookTitle={titleFromPath(fullTranslateBook)}
    onClose={() => (fullTranslateBook = null)}
    onLibraryChanged={() => void refresh()}
  />
{/if}

{#if bookAction}
  {@const ba = bookAction}
  {@const baMeta = snapshot?.metadata.books[ba.path]}
  <!-- svelte-ignore a11y_click_events_have_key_events -->
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <div class="confirm-back" onclick={closeBookAction}>
    <div class="confirm" role="alertdialog" aria-modal="true" aria-labelledby="confirm-title" tabindex="-1" onclick={(e) => e.stopPropagation()}>
      <div class="confirm-book"><Book3D path={ba.path} meta={baMeta} eager compact tilt={false} /></div>
      <div class="confirm-body">
        <h2 id="confirm-title">{ba.kind === "delete" ? "Удалить книгу с диска?" : "Убрать книгу с полок?"}</h2>
        <p class="confirm-name">{bookTitle(ba.path, baMeta)}</p>
        <p class="confirm-copy">
          {ba.kind === "delete"
            ? "Файл будет удалён безвозвратно вместе с прогрессом и заметками."
            : "Файл останется на диске. Вернуть книгу можно из раздела «Скрытые»."}
        </p>
        {#if bookActionError}<p class="confirm-error" role="alert">{bookActionError}</p>{/if}
        <div class="confirm-actions">
          <button type="button" class="ghost" onclick={closeBookAction} disabled={bookActionBusy}>Отмена</button>
          <button type="button" class="solid" class:danger={ba.kind === "delete"} onclick={() => void confirmBookAction()} disabled={bookActionBusy}>
            {bookActionBusy ? "Подождите…" : ba.kind === "delete" ? "Удалить" : "Убрать"}
          </button>
        </div>
      </div>
    </div>
  </div>
{/if}

<style>
  .app {
    display: flex;
    height: 100dvh;
    overflow: hidden;
    background: var(--bg-soft);
    color: var(--text-soft);
  }

  .main {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
  }

  .content {
    position: relative;
    isolation: isolate;
    flex: 1;
    min-height: 0;
    overflow-y: auto;
    overscroll-behavior: contain;
    padding: 0 clamp(1rem, 4vw, 3.2rem) 4rem;
  }

  .ambient {
    position: absolute;
    inset: 0 0 auto;
    height: 34rem;
    z-index: -1;
    pointer-events: none;
    background:
      radial-gradient(ellipse 60% 70% at 30% 0%, var(--glow), transparent 70%),
      radial-gradient(ellipse 40% 50% at 90% 10%, color-mix(in srgb, var(--accent) 10%, transparent), transparent 70%);
    transition: background 1s ease;
  }

  /* ——— Верхняя панель ——— */
  .topbar {
    position: relative;
    z-index: 20;
    flex-shrink: 0;
    display: flex;
    align-items: center;
    gap: 0.5rem;
    padding: max(1rem, env(safe-area-inset-top)) clamp(1rem, 4vw, 3.2rem) 0.9rem;
    border-bottom: 1px solid transparent;
    transition:
      background 0.25s ease,
      border-color 0.25s ease;
  }

  .topbar.scrolled {
    background: color-mix(in srgb, var(--bg-soft) 96%, transparent);
    border-bottom-color: color-mix(in srgb, var(--border-soft) 60%, transparent);
  }

  .grow {
    flex: 1;
  }

  .search {
    display: flex;
    align-items: center;
    gap: 0.55rem;
    width: min(28rem, 100%);
    padding: 0.62rem 0.9rem;
    border-radius: 999px;
    border: 1px solid color-mix(in srgb, var(--border-soft) 75%, transparent);
    background: var(--panel-elevated);
    box-shadow: 0 1px 2px rgba(20, 14, 30, 0.04);
    transition:
      border-color 0.2s ease,
      box-shadow 0.2s ease;
  }

  .search:focus-within {
    border-color: var(--accent-2);
    box-shadow: var(--focus-ring);
  }

  .search svg {
    width: 1.05rem;
    height: 1.05rem;
    flex-shrink: 0;
    fill: none;
    stroke: var(--muted);
    stroke-width: 1.8;
    stroke-linecap: round;
  }

  .search input {
    flex: 1;
    min-width: 0;
    border: none;
    outline: none;
    background: transparent;
    color: var(--text-soft);
    font: inherit;
    font-size: 0.95rem;
  }

  .search kbd {
    font-family: inherit;
    font-size: 0.72rem;
    color: var(--muted);
    border: 1px solid var(--border-soft);
    border-radius: 0.35rem;
    padding: 0 0.35rem;
  }

  .icon {
    display: grid;
    place-items: center;
    width: 2.5rem;
    height: 2.5rem;
    border-radius: 999px;
    border: none;
    background: transparent;
    color: var(--text-soft);
    cursor: pointer;
    transition: background 0.15s ease;
  }

  .icon:hover {
    background: color-mix(in srgb, var(--accent) 14%, transparent);
  }

  .icon svg,
  .add svg,
  .cta svg,
  .seg svg,
  .more svg {
    width: 1.15rem;
    height: 1.15rem;
    fill: none;
    stroke: currentColor;
    stroke-width: 1.8;
    stroke-linecap: round;
    stroke-linejoin: round;
  }

  .menu-btn {
    display: none;
  }

  .add {
    display: inline-flex;
    align-items: center;
    gap: 0.45rem;
    padding: 0.62rem 1.1rem;
    border: none;
    border-radius: 999px;
    background: var(--text-soft);
    color: var(--bg-soft);
    font: inherit;
    font-size: 0.9rem;
    font-weight: 600;
    white-space: nowrap;
    cursor: pointer;
    box-shadow: 0 8px 20px -10px var(--text-soft);
    transition: transform 0.2s ease;
  }

  .add:hover {
    transform: translateY(-1px);
  }

  .banner {
    margin-bottom: 1rem;
    padding: 0.8rem 1rem;
    border-radius: 1rem;
    background: color-mix(in srgb, var(--danger) 12%, var(--panel-elevated));
    color: var(--danger);
    font-size: 0.9rem;
  }

  /* ——— Приветствие без библиотеки ——— */
  .welcome {
    max-width: 36rem;
    margin: 8vh auto 0;
    text-align: center;
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 1rem;
  }

  .welcome h1 {
    margin: 0.6rem 0 0;
    font-family: "Literata Variable", Georgia, serif;
    font-weight: 500;
    font-size: clamp(2rem, 5vw, 2.8rem);
    letter-spacing: -0.02em;
  }

  .welcome p {
    margin: 0;
    color: var(--muted);
    line-height: 1.6;
  }

  .cta {
    display: inline-flex;
    align-items: center;
    gap: 0.6rem;
    margin-top: 0.8rem;
    padding: 0.9rem 1.5rem;
    border: none;
    border-radius: 999px;
    background: var(--text-soft);
    color: var(--bg-soft);
    font: inherit;
    font-weight: 600;
    cursor: pointer;
    box-shadow: 0 14px 30px -14px var(--text-soft);
  }

  .stack {
    position: relative;
    width: 12rem;
    height: 9rem;
  }

  .stack .b {
    position: absolute;
    bottom: 10px;
    width: 2.1rem;
    border-radius: 3px 3px 1px 1px;
    box-shadow: inset -4px 0 0 rgba(0, 0, 0, 0.12);
    animation: bob 5s ease-in-out infinite;
  }

  .b1 {
    left: 2.2rem;
    height: 6.6rem;
    background: #3b3355;
  }
  .b2 {
    left: 4.45rem;
    height: 7.6rem;
    background: #6b4a2b;
    animation-delay: 0.4s !important;
  }
  .b3 {
    left: 6.7rem;
    height: 6rem;
    background: #28463d;
    animation-delay: 0.8s !important;
  }
  .b4 {
    left: 8.6rem;
    height: 6.9rem;
    background: var(--accent);
    transform-origin: bottom left;
    transform: rotate(14deg);
    animation: none !important;
  }

  .shelf-line {
    position: absolute;
    left: 0;
    right: 0;
    bottom: 0;
    height: 10px;
    border-radius: 3px;
    background: color-mix(in srgb, var(--text-soft) 22%, var(--panel-soft));
  }

  @keyframes bob {
    50% {
      transform: translateY(-4px);
    }
  }

  /* ——— Библиотека ——— */
  .library {
    margin-top: 3rem;
  }

  .lib-head {
    display: flex;
    align-items: flex-end;
    justify-content: space-between;
    gap: 1rem;
    flex-wrap: wrap;
    padding-bottom: 1rem;
    margin-bottom: 1rem;
    border-bottom: 1px solid color-mix(in srgb, var(--border-soft) 70%, transparent);
  }

  .lib-title {
    display: flex;
    align-items: baseline;
    gap: 0.8rem;
  }

  .lib-title h2 {
    margin: 0;
    font-family: "Literata Variable", Georgia, serif;
    font-weight: 500;
    font-size: clamp(1.5rem, 3vw, 2rem);
    letter-spacing: -0.01em;
  }

  .count {
    color: var(--muted);
    font-size: 0.9rem;
  }

  .lib-tools {
    display: flex;
    align-items: center;
    gap: 0.6rem;
  }

  .seg {
    display: flex;
    padding: 3px;
    border-radius: 999px;
    background: color-mix(in srgb, var(--panel-soft) 80%, transparent);
    border: 1px solid color-mix(in srgb, var(--border-soft) 70%, transparent);
  }

  .seg button {
    display: grid;
    place-items: center;
    width: 2.2rem;
    height: 2rem;
    border: none;
    border-radius: 999px;
    background: transparent;
    color: var(--muted);
    cursor: pointer;
  }

  .seg button.on {
    background: var(--elevated-soft);
    color: var(--text-soft);
    box-shadow: 0 1px 3px rgba(20, 14, 30, 0.1);
  }

  .sort {
    min-width: 12rem;
  }

  .chips {
    display: flex;
    flex-wrap: wrap;
    gap: 0.4rem;
    margin-bottom: 1.8rem;
  }

  .chip {
    display: inline-flex;
    align-items: center;
    gap: 0.4rem;
    padding: 0.38rem 0.85rem;
    border-radius: 999px;
    border: 1px solid color-mix(in srgb, var(--border-soft) 80%, transparent);
    background: transparent;
    color: var(--text-soft);
    font: inherit;
    font-size: 0.85rem;
    cursor: pointer;
    transition: background 0.15s ease;
  }

  .chip:hover {
    background: color-mix(in srgb, var(--accent) 10%, transparent);
  }

  .chip span {
    font-size: 0.74rem;
    color: var(--muted);
    font-variant-numeric: tabular-nums;
  }

  .chip.on {
    background: var(--text-soft);
    border-color: var(--text-soft);
    color: var(--bg-soft);
  }

  .chip.on span {
    color: inherit;
    opacity: 0.7;
  }

  .grid {
    list-style: none;
    margin: 0;
    padding: 0;
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(min(100%, 9.5rem), 1fr));
    gap: 2.4rem clamp(1.2rem, 2.4vw, 2rem);
  }

  .cell {
    position: relative;
    min-width: 0;
  }

  .card {
    position: relative;
    width: 100%;
    display: flex;
    flex-direction: column;
    gap: 0.9rem;
    padding: 0;
    border: none;
    background: none;
    color: inherit;
    text-align: left;
    cursor: pointer;
    font: inherit;
  }

  .card:focus-visible {
    outline: none;
  }

  .card:focus-visible .book-hover-target {
    outline: 2px solid var(--accent-2);
    outline-offset: 6px;
    border-radius: 6px;
  }

  .card.hidden-book {
    opacity: 0.55;
  }

  .ribbon {
    position: absolute;
    top: -4px;
    right: 12%;
    z-index: 4;
    width: 1.35rem;
    padding: 0.35rem 0 0.55rem;
    text-align: center;
    font-size: 0.7rem;
    color: #fff;
    background: #5f9a6e;
    clip-path: polygon(0 0, 100% 0, 100% 100%, 50% 82%, 0 100%);
    box-shadow: 0 2px 4px rgba(0, 0, 0, 0.2);
  }

  .card-text {
    display: flex;
    flex-direction: column;
    gap: 0.22rem;
    min-width: 0;
  }

  .t {
    font-family: "Literata Variable", Georgia, serif;
    font-weight: 600;
    font-size: 0.95rem;
    line-height: 1.3;
    display: -webkit-box;
    -webkit-line-clamp: 2;
    line-clamp: 2;
    -webkit-box-orient: vertical;
    overflow: hidden;
    overflow-wrap: anywhere;
  }

  .a {
    font-size: 0.82rem;
    color: var(--muted);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .bar {
    height: 3px;
    border-radius: 999px;
    margin-top: 0.35rem;
    background: color-mix(in srgb, var(--text-soft) 10%, transparent);
    overflow: hidden;
  }

  .bar span {
    display: block;
    height: 100%;
    border-radius: inherit;
    background: linear-gradient(90deg, var(--accent), var(--accent-2));
  }

  .s {
    display: inline-flex;
    align-items: center;
    gap: 0.3rem;
    font-size: 0.74rem;
    color: var(--muted);
    margin-top: 0.1rem;
  }

  .s-reading {
    color: var(--accent-2);
    font-weight: 600;
  }

  .s-done {
    color: #5f9a6e;
  }

  .star {
    color: #d49a2a;
  }

  .more {
    position: absolute;
    top: 0.45rem;
    right: 0.45rem;
    z-index: 5;
    display: grid;
    place-items: center;
    width: 2rem;
    height: 2rem;
    border: none;
    border-radius: 999px;
    background: var(--panel-elevated);
    color: var(--text-soft);
    box-shadow: 0 4px 12px rgba(20, 14, 30, 0.18);
    cursor: pointer;
    opacity: 0;
    transform: scale(0.9);
    transition:
      opacity 0.15s ease,
      transform 0.15s ease;
  }

  .cell:hover .more,
  .more:focus-visible {
    opacity: 1;
    transform: none;
  }

  .empty {
    padding: 4rem 1rem;
    text-align: center;
    color: var(--muted);
  }

  .empty-title {
    margin: 0 0 0.4rem;
    font-family: "Literata Variable", Georgia, serif;
    font-size: 1.35rem;
    color: var(--text-soft);
  }

  .empty p {
    margin: 0 0 0.3rem;
  }

  .ghost,
  .solid {
    margin-top: 0.8rem;
    padding: 0.55rem 1.1rem;
    border-radius: 999px;
    border: 1px solid var(--border-soft);
    background: transparent;
    color: var(--text-soft);
    font: inherit;
    cursor: pointer;
  }

  .solid {
    background: var(--text-soft);
    border-color: var(--text-soft);
    color: var(--bg-soft);
    font-weight: 600;
  }

  .solid.danger {
    background: var(--danger);
    border-color: var(--danger);
    color: #fff;
  }

  /* ——— Перетаскивание ——— */
  .drop {
    position: fixed;
    inset: 0;
    z-index: 1500;
    display: grid;
    place-items: center;
    background: color-mix(in srgb, var(--bg-soft) 72%, transparent);
    backdrop-filter: blur(8px);
    animation: fade 0.15s ease-out;
  }

  .drop-card {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 0.4rem;
    padding: 2.6rem 3.4rem;
    border-radius: 2rem;
    border: 2px dashed var(--accent-2);
    background: var(--panel-elevated);
    box-shadow: var(--shadow-float);
  }

  .drop-card svg {
    width: 2.6rem;
    height: 2.6rem;
    fill: none;
    stroke: var(--accent-2);
    stroke-width: 1.8;
    stroke-linecap: round;
    stroke-linejoin: round;
    animation: bob 1.6s ease-in-out infinite;
  }

  .drop-card p {
    margin: 0.4rem 0 0;
    font-family: "Literata Variable", Georgia, serif;
    font-size: 1.25rem;
  }

  .drop-card small {
    color: var(--muted);
  }

  /* ——— Контекстное меню ——— */
  .ctx {
    position: fixed;
    z-index: 800;
    width: 15rem;
    padding: 0.4rem;
    border-radius: 1rem;
    background: var(--panel-elevated);
    border: 1px solid color-mix(in srgb, var(--border-soft) 80%, transparent);
    box-shadow: var(--shadow-float);
    display: flex;
    flex-direction: column;
    animation: pop 0.14s ease-out;
  }

  .ctx-title {
    margin: 0;
    padding: 0.45rem 0.7rem 0.5rem;
    font-family: "Literata Variable", Georgia, serif;
    font-weight: 600;
    font-size: 0.9rem;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .ctx-label {
    margin: 0;
    padding: 0.2rem 0.7rem;
    font-size: 0.68rem;
    text-transform: uppercase;
    letter-spacing: 0.12em;
    color: var(--muted);
  }

  .ctx > button {
    border: none;
    background: transparent;
    color: var(--text-soft);
    text-align: left;
    padding: 0.5rem 0.7rem;
    border-radius: 0.6rem;
    font: inherit;
    font-size: 0.88rem;
    cursor: pointer;
  }

  .ctx > button:hover {
    background: color-mix(in srgb, var(--accent) 14%, transparent);
  }

  .ctx > button.danger {
    color: var(--danger);
  }

  .ctx-status {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 0.25rem;
    padding: 0.2rem 0.4rem 0.3rem;
  }

  .ctx-status button {
    border: 1px solid var(--border-soft);
    background: transparent;
    color: var(--text-soft);
    border-radius: 0.55rem;
    padding: 0.35rem 0.3rem;
    font: inherit;
    font-size: 0.74rem;
    cursor: pointer;
  }

  .ctx-status button.on {
    background: var(--text-soft);
    border-color: var(--text-soft);
    color: var(--bg-soft);
  }

  .sep {
    height: 1px;
    margin: 0.3rem 0.5rem;
    background: color-mix(in srgb, var(--border-soft) 80%, transparent);
  }

  /* ——— Подтверждение ——— */
  .confirm-back {
    position: fixed;
    inset: 0;
    z-index: 900;
    display: grid;
    place-items: center;
    padding: 1rem;
    background: rgba(20, 14, 30, 0.38);
    backdrop-filter: blur(6px);
    animation: fade 0.18s ease-out;
  }

  .confirm {
    width: min(30rem, 100%);
    display: flex;
    gap: 1.4rem;
    padding: 1.6rem;
    border-radius: 1.5rem;
    background: var(--panel-elevated);
    box-shadow: var(--shadow-float);
    animation: pop 0.18s ease-out;
  }

  .confirm-book {
    width: 5.2rem;
    flex-shrink: 0;
  }

  .confirm-body h2 {
    margin: 0;
    font-family: "Literata Variable", Georgia, serif;
    font-weight: 500;
    font-size: 1.3rem;
  }

  .confirm-name {
    margin: 0.4rem 0 0;
    font-weight: 600;
  }

  .confirm-copy {
    margin: 0.5rem 0 0;
    color: var(--muted);
    font-size: 0.9rem;
    line-height: 1.5;
  }

  .confirm-error {
    margin: 0.6rem 0 0;
    color: var(--danger);
    font-size: 0.86rem;
  }

  .confirm-actions {
    display: flex;
    justify-content: flex-end;
    gap: 0.5rem;
    margin-top: 0.6rem;
  }

  .sr-only {
    position: absolute;
    width: 1px;
    height: 1px;
    overflow: hidden;
    clip: rect(0 0 0 0);
    white-space: nowrap;
  }

  @keyframes fade {
    from {
      opacity: 0;
    }
  }

  @keyframes pop {
    from {
      opacity: 0;
      transform: translateY(-4px) scale(0.98);
    }
  }

  @media (max-width: 900px) {
    .menu-btn {
      display: grid;
    }
  }

  @media (max-width: 600px) {
    .content {
      padding: 0 1rem 3rem;
    }

    .topbar {
      padding-inline: 1rem;
    }

    .add span {
      display: none;
    }

    .add {
      padding: 0.62rem;
    }

    .search kbd {
      display: none;
    }

    .search {
      flex: 1;
      width: auto;
      min-width: 0;
    }

    .topbar .grow,
    .refresh-btn {
      display: none;
    }

    .grid {
      grid-template-columns: repeat(2, minmax(0, 1fr));
      gap: 2rem 1.2rem;
    }

    .more {
      opacity: 1;
      transform: none;
      width: 1.8rem;
      height: 1.8rem;
    }

    .sort {
      min-width: 0;
    }

    .library {
      margin-top: 2.2rem;
    }
  }
</style>
