<script lang="ts">
  import { onDestroy, onMount } from "svelte";
  import { goto } from "$app/navigation";
  import { invoke } from "@tauri-apps/api/core";
  import { open } from "@tauri-apps/plugin-dialog";
  import type { BookMeta, Importance, LibraryMetadata, LibrarySnapshot, Shelf } from "$lib/types";
  import { IMPORTANCE_OPTIONS } from "$lib/types";
  import { formatBadgeLabel, getBookFormat } from "$lib/bookFormat";
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
  } from "$lib/library/libraryListPrefs";
  import SettingsThemeCard from "$lib/components/SettingsThemeCard.svelte";
  import BookFullTranslateModal from "$lib/components/BookFullTranslateModal.svelte";
  import BookCoverThumb from "$lib/components/BookCoverThumb.svelte";
  import DreamSelect from "$lib/components/DreamSelect.svelte";
  import { isTauriRuntime } from "$lib/isTauri";
  import { exportBookToTypst } from "$lib/typst/exportToTypst";

  let snapshot = $state<LibrarySnapshot | null>(getCachedLibrarySnapshot());
  let shelfFilter = $state<string>("all");
  let newShelfName = $state("");
  let banner = $state<string | null>(null);
  let searchQuery = $state("");
  let mobileLibraryOpen = $state(false);

  let ctxMenu = $state<{ x: number; y: number; path: string } | null>(null);
  let bookAction = $state<{ kind: "hide" | "delete"; path: string } | null>(null);
  let bookActionBusy = $state(false);
  let bookActionError = $state<string | null>(null);
  let editPath = $state<string | null>(null);
  const editOpen = $derived(editPath != null);
  let editDraft = $state<{
    title: string;
    author: string;
    typstStyleRelativePath: string;
    importance: Importance;
    shelfIds: string[];
    review: string;
  } | null>(null);

  let fullTranslateBook = $state<string | null>(null);
  const pendingCoverUpdates = new Map<string, string>();
  let coverSaveTimer: ReturnType<typeof setTimeout> | null = null;

  const IMPORTANCE_ORDER: Record<string, number> = {
    essential: 0,
    high: 1,
    normal: 2,
    low: 3,
  };

  let librarySort = $state<LibrarySortMode>(DEFAULT_LIBRARY_SORT);

  onMount(() => {
    librarySort = readLibrarySort();
  });

  function setLibrarySort(mode: LibrarySortMode) {
    librarySort = mode;
    writeLibrarySort(mode);
  }

  function openedAtTs(meta: BookMeta | undefined): number {
    const s = meta?.lastOpenedAt;
    if (!s) return 0;
    const t = Date.parse(s);
    return Number.isFinite(t) ? t : 0;
  }

  function cmpBookPaths(a: string, b: string): number {
    const ma = snapshot!.metadata.books[a];
    const mb = snapshot!.metadata.books[b];
    switch (librarySort) {
      case "recent": {
        const ta = openedAtTs(ma);
        const tb = openedAtTs(mb);
        if (tb !== ta) return tb - ta;
        break;
      }
      case "title": {
        const xa = (ma?.title?.trim() || titleFromPath(a)).toLowerCase();
        const xb = (mb?.title?.trim() || titleFromPath(b)).toLowerCase();
        return xa.localeCompare(xb, "ru");
      }
      case "title_desc": {
        const xa = (ma?.title?.trim() || titleFromPath(a)).toLowerCase();
        const xb = (mb?.title?.trim() || titleFromPath(b)).toLowerCase();
        return xb.localeCompare(xa, "ru");
      }
      case "importance": {
        const ia = IMPORTANCE_ORDER[String(ma?.importance ?? "normal")] ?? 2;
        const ib = IMPORTANCE_ORDER[String(mb?.importance ?? "normal")] ?? 2;
        if (ia !== ib) return ia - ib;
        break;
      }
      default:
        break;
    }
    const xa = (ma?.title?.trim() || titleFromPath(a)).toLowerCase();
    const xb = (mb?.title?.trim() || titleFromPath(b)).toLowerCase();
    return xa.localeCompare(xb, "ru");
  }

  const shelvesSorted = $derived(
    snapshot ? [...snapshot.metadata.shelves].sort((a, b) => a.order - b.order) : [],
  );

  const displayedBooks = $derived.by(() => {
    if (!snapshot) return [];
    let paths = shelfFilter === "hidden" ? snapshot.hiddenBookPaths : snapshot.bookPaths;
    if (shelfFilter !== "all" && shelfFilter !== "hidden") {
      paths = paths.filter((p) =>
        bookOnShelf(snapshot!.metadata.books[p], shelfFilter),
      );
    }
    const query = searchQuery.trim().toLocaleLowerCase("ru");
    if (query) {
      paths = paths.filter((p) => {
        const meta = snapshot!.metadata.books[p];
        const haystack = [titleFromPath(p), meta?.title, meta?.author]
          .filter(Boolean)
          .join(" ")
          .toLocaleLowerCase("ru");
        return haystack.includes(query);
      });
    }
    return paths.slice().sort(cmpBookPaths);
  });

  function selectShelf(id: string) {
    shelfFilter = id;
    mobileLibraryOpen = false;
  }

  const editMeta = $derived(
    editPath && snapshot ? snapshot.metadata.books[editPath] ?? null : null,
  );

  function titleFromPath(path: string) {
    const i = path.lastIndexOf("/");
    return i >= 0 ? path.slice(i + 1) : path;
  }

  function countForShelf(shelfId: string): number {
    const snap = snapshot;
    if (!snap) return 0;
    if (shelfId === "all") return snap.bookPaths.length;
    if (shelfId === "hidden") return snap.hiddenBookPaths.length;
    return snap.bookPaths.filter((p) => bookOnShelf(snap.metadata.books[p], shelfId)).length;
  }

  const stageHeading = $derived(
    shelfFilter === "all"
      ? "Все книги"
      : shelfFilter === "hidden"
        ? "Скрытые книги"
        : (shelvesSorted.find((s) => s.id === shelfFilter)?.name ?? "Полка"),
  );

  function bookIsHidden(path: string): boolean {
    return snapshot?.hiddenBookPaths.includes(path) ?? false;
  }

  function importanceLabel(v: string) {
    return IMPORTANCE_OPTIONS.find((o) => o.value === v)?.label ?? v;
  }

  function readingProgress(
    meta: BookMeta | undefined,
    fmt: NonNullable<ReturnType<typeof getBookFormat>>,
  ): { kind: "pdf"; pct: number; label: string } | { kind: "loc"; label: string } | null {
    if (!meta) return null;
    if (fmt === "pdf") {
      const t = meta.lastReadPdfTotal;
      const p = meta.lastReadPdfPage;
      if (t != null && t > 0 && p != null && p >= 1) {
        return {
          kind: "pdf",
          pct: Math.min(100, Math.round((p / t) * 100)),
          label: `стр. ${p} / ${t}`,
        };
      }
      return null;
    }
    if ((fmt === "epub" || fmt === "fb2") && meta.lastReadLocationLabel?.trim()) {
      return { kind: "loc", label: meta.lastReadLocationLabel.trim() };
    }
    return null;
  }

  async function exportBookToTypstFromMenu() {
    if (!ctxMenu || !snapshot) return;
    const path = ctxMenu.path;
    const fmt = getBookFormat(path);
    if (!fmt || fmt === "typst") return;
    const style = snapshot.metadata.books[path]?.typstStyleRelativePath;
    closeCtx();
    try {
      const { mainRelativePath } = await exportBookToTypst(
        path,
        fmt,
        snapshot,
        style,
      );
      await refresh();
      await goto("/read?path=" + encodeURIComponent(mainRelativePath));
    } catch (e) {
      alert(e instanceof Error ? e.message : String(e));
    }
  }

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
    if (snapshot) {
      snapshot = { ...snapshot, metadata: next };
    }
  }

  function cacheBookCover(path: string, dataUrl: string) {
    if (snapshot?.metadata.books[path]?.coverThumbDataUrl === dataUrl) return;
    pendingCoverUpdates.set(path, dataUrl);
    if (coverSaveTimer) clearTimeout(coverSaveTimer);
    coverSaveTimer = setTimeout(() => {
      coverSaveTimer = null;
      void flushCoverUpdates();
    }, 900);
  }

  async function flushCoverUpdates() {
    if (!snapshot || pendingCoverUpdates.size === 0) return;
    const updates = [...pendingCoverUpdates];
    pendingCoverUpdates.clear();
    const books = { ...snapshot.metadata.books };
    for (const [path, dataUrl] of updates) {
      const current = books[path];
      if (current) books[path] = { ...current, coverThumbDataUrl: dataUrl };
    }
    try {
      await persist({ ...snapshot.metadata, books });
    } catch {
      for (const [path, dataUrl] of updates) pendingCoverUpdates.set(path, dataUrl);
    }
  }

  function patchBook(path: string, patch: Partial<BookMeta>) {
    if (!snapshot) return;
    const cur = snapshot.metadata.books[path];
    if (!cur) return;
    const books = { ...snapshot.metadata.books, [path]: { ...cur, ...patch } };
    void persist({ ...snapshot.metadata, books });
  }

  function toggleBookShelf(shelfId: string) {
    if (!editDraft) return;
    let ids = [...editDraft.shelfIds];
    if (ids.includes(shelfId)) {
      ids = ids.filter((id) => id !== shelfId);
      if (ids.length === 0) ids = ["default"];
    } else {
      ids.push(shelfId);
    }
    editDraft = { ...editDraft, shelfIds: ids };
  }

  function openBook(p: string) {
    if (!getBookFormat(p)) return;
    goto("/read?path=" + encodeURIComponent(p));
  }

  function onCardContextMenu(e: MouseEvent, p: string) {
    e.preventDefault();
    const menuWidth = 224;
    const menuHeight = 300;
    ctxMenu = {
      x: Math.max(12, Math.min(e.clientX, window.innerWidth - menuWidth - 12)),
      y: Math.max(12, Math.min(e.clientY, window.innerHeight - menuHeight - 12)),
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
    } catch {
      banner = "Не удалось вернуть книгу в библиотеку. Попробуйте ещё раз.";
    }
  }

  async function confirmBookAction() {
    if (!bookAction) return;
    const { kind, path } = bookAction;
    bookActionBusy = true;
    bookActionError = null;
    try {
      if (kind === "hide") {
        await setBookHidden(path, true);
      } else {
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

  function startEdit() {
    if (ctxMenu) {
      editPath = ctxMenu.path;
      const m = snapshot?.metadata.books[ctxMenu.path];
      if (m) {
        editDraft = {
          title: m.title ?? "",
          author: m.author ?? "",
          typstStyleRelativePath: m.typstStyleRelativePath ?? "",
          importance: (IMPORTANCE_OPTIONS.some((o) => o.value === m.importance)
            ? m.importance
            : "normal") as Importance,
          shelfIds: [...effectiveShelfIds(m)],
          review: m.review ?? "",
        };
      }
      closeCtx();
    }
  }

  function closeEdit() {
    editPath = null;
    editDraft = null;
  }

  function saveEdit() {
    if (!editPath || !editDraft) return;
    const shelfIds = editDraft.shelfIds.length ? editDraft.shelfIds : ["default"];
    patchBook(editPath, {
      title: editDraft.title.trim() || null,
      author: editDraft.author.trim() || null,
      typstStyleRelativePath: editDraft.typstStyleRelativePath.trim() || null,
      importance: editDraft.importance,
      shelfIds,
      shelfId: shelfIds[0] ?? "default",
      review: editDraft.review,
    });
    closeEdit();
  }

  async function addShelf() {
    const name = newShelfName.trim();
    if (!name || !snapshot) return;
    const id = crypto.randomUUID();
    const order =
      snapshot.metadata.shelves.reduce((m, s) => Math.max(m, s.order), -1) + 1;
    const shelves: Shelf[] = [...snapshot.metadata.shelves, { id, name, order }];
    newShelfName = "";
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
      books[k] = {
        ...b,
        shelfIds: ids,
        shelfId: ids[0] ?? "default",
      };
    }
    if (shelfFilter === shelfId) shelfFilter = "all";
    await persist({ shelves, books });
  }

  $effect(() => {
    if (!snapshot) void refresh();
  });

  onDestroy(() => {
    if (coverSaveTimer) clearTimeout(coverSaveTimer);
    void flushCoverUpdates();
  });

  $effect(() => {
    if (!ctxMenu) return;
    const onDown = (e: MouseEvent) => {
      const t = e.target as Node;
      const el = document.getElementById("ctx-menu");
      if (el && !el.contains(t)) closeCtx();
    };
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") {
        closeCtx();
        mobileLibraryOpen = false;
        closeEdit();
        closeBookAction();
      }
    };
    document.addEventListener("pointerdown", onDown, true);
    document.addEventListener("keydown", onKey);
    return () => {
      document.removeEventListener("pointerdown", onDown, true);
      document.removeEventListener("keydown", onKey);
    };
  });
</script>

<div class="home">
  <div class="home-ambient" aria-hidden="true"></div>
  <div class="home-film" aria-hidden="true"></div>

  <header class="home-top">
    <div class="home-brand">
      <span class="home-mark" aria-hidden="true"></span>
      <div class="home-brand-text">
        <span class="home-logo">Reader</span>
        <span class="home-tagline">пространство для чтения</span>
      </div>
    </div>
    <div class="home-actions">
      <button
        type="button"
        class="home-btn home-btn-ghost home-shelves-toggle"
        aria-expanded={mobileLibraryOpen}
        onclick={() => (mobileLibraryOpen = !mobileLibraryOpen)}
      >
        <svg viewBox="0 0 24 24" aria-hidden="true"><path d="M4 5.5h16M4 12h16M4 18.5h16" /></svg>
        Полки
      </button>
      <button type="button" class="home-btn home-btn-ghost" onclick={() => void refresh()}>
        <svg viewBox="0 0 24 24" aria-hidden="true"><path d="M20 11a8 8 0 1 0-2.34 5.66M20 5v6h-6" /></svg>
        <span class="action-label">Обновить</span>
      </button>
      <a href="/settings" class="home-btn home-btn-ghost home-settings-link">
        <svg viewBox="0 0 24 24" aria-hidden="true"><circle cx="12" cy="12" r="3" /><path d="M19.4 15a1.7 1.7 0 0 0 .34 1.88l.06.06-2.83 2.83-.06-.06a1.7 1.7 0 0 0-1.88-.34 1.7 1.7 0 0 0-1.03 1.56V21h-4v-.09A1.7 1.7 0 0 0 9 19.37a1.7 1.7 0 0 0-1.88.34l-.06.06-2.83-2.83.06-.06A1.7 1.7 0 0 0 4.63 15 1.7 1.7 0 0 0 3.09 14H3v-4h.09A1.7 1.7 0 0 0 4.63 9a1.7 1.7 0 0 0-.34-1.88l-.06-.06 2.83-2.83.06.06A1.7 1.7 0 0 0 9 4.63 1.7 1.7 0 0 0 10 3.09V3h4v.09A1.7 1.7 0 0 0 15 4.63a1.7 1.7 0 0 0 1.88-.34l.06-.06 2.83 2.83-.06.06A1.7 1.7 0 0 0 19.37 9 1.7 1.7 0 0 0 20.91 10H21v4h-.09A1.7 1.7 0 0 0 19.4 15Z" /></svg>
        <span class="action-label">Настройки</span>
      </a>
      <button type="button" class="home-btn home-btn-primary" onclick={() => void pickFolder()}>
        <svg viewBox="0 0 24 24" aria-hidden="true"><path d="M3 7.5h7l2 2h9v9.5H3z" /><path d="M3 7.5V5h7l2 2h9v2.5" /></svg>
        <span class="folder-label">Папка библиотеки</span>
      </button>
    </div>
    {#if snapshot?.libraryRoot}
      <p class="home-path" title={snapshot.libraryRoot}>
        <span class="home-path-label">корень</span>
        <span class="home-path-value">{snapshot.libraryRoot}</span>
      </p>
    {/if}
  </header>

  {#if banner}
    <div class="home-banner">{banner}</div>
  {/if}

  <div class="home-main">
    <aside class="home-rail" class:home-rail-open={mobileLibraryOpen}>
      <div class="home-rail-inner">
        <section class="home-panel">
          <div class="home-panel-head">
            <div>
              <h2 class="home-panel-title">Полки</h2>
              <p class="home-panel-sub">разделите коллекцию по темам</p>
            </div>
            <button
              type="button"
              class="rail-close"
              aria-label="Закрыть полки"
              onclick={() => (mobileLibraryOpen = false)}
            >×</button>
          </div>
          <div class="shelf-list">
            <button
              type="button"
              class="shelf-pill"
              class:shelf-pill-active={shelfFilter === "all"}
              onclick={() => selectShelf("all")}
            >
              <span class="shelf-name">Все книги</span>
              <span class="shelf-count">{countForShelf("all")}</span>
            </button>
            {#each shelvesSorted as s (s.id)}
              <div class="shelf-row-wrap">
                <button
                  type="button"
                  class="shelf-pill"
                  class:shelf-pill-active={shelfFilter === s.id}
                  onclick={() => selectShelf(s.id)}
                >
                  <span class="shelf-name">{s.name}</span>
                  <span class="shelf-count">{countForShelf(s.id)}</span>
                </button>
                {#if s.id !== "default"}
                  <button
                    type="button"
                    class="shelf-remove"
                    title="Удалить полку"
                    onclick={() => void removeShelf(s.id)}>×</button>
                {/if}
              </div>
            {/each}
            <div class="shelf-hidden-separator" aria-hidden="true"></div>
            <button
              type="button"
              class="shelf-pill shelf-pill-hidden"
              class:shelf-pill-active={shelfFilter === "hidden"}
              onclick={() => selectShelf("hidden")}
            >
              <span class="shelf-name shelf-hidden-name">
                <span class="shelf-hidden-mark" aria-hidden="true">◌</span>
                Скрытые
              </span>
              <span class="shelf-count">{countForShelf("hidden")}</span>
            </button>
          </div>
          <div class="add-shelf">
            <input placeholder="Новая полка…" bind:value={newShelfName} />
            <button type="button" class="home-btn home-btn-small" onclick={() => void addShelf()}>+</button>
          </div>
        </section>

        <section class="home-panel home-panel-settings">
          <SettingsThemeCard />
        </section>
      </div>
    </aside>

    <button
      type="button"
      class="rail-backdrop"
      class:rail-backdrop-open={mobileLibraryOpen}
      aria-label="Закрыть полки"
      onclick={() => (mobileLibraryOpen = false)}
    ></button>

    <section class="home-stage">
      {#if !snapshot?.libraryRoot}
        <div class="empty-state">
          <div class="empty-orbit" aria-hidden="true"></div>
          <p class="empty-kicker">шаг первый</p>
          <h1 class="empty-heading">Откройте папку с книгами</h1>
          <p class="empty-body">
            PDF, EPUB и FB2 появятся здесь как живая полка. Клик — чтение, правая кнопка — сведения и
            заметки.
          </p>
          <button type="button" class="home-btn home-btn-primary empty-cta" onclick={() => void pickFolder()}>
            Выбрать каталог
          </button>
        </div>
      {:else if displayedBooks.length === 0 && !searchQuery.trim()}
        <div class="empty-state empty-state-muted">
          <div class="empty-orbit empty-orbit-soft" aria-hidden="true"></div>
          <p class="empty-kicker">{shelfFilter === "hidden" ? "всё на виду" : "ничего не выбрано"}</p>
          <h1 class="empty-heading">
            {shelfFilter === "hidden" ? "Скрытых книг нет" : "Тишина на этой полке"}
          </h1>
          <p class="empty-body">
            {shelfFilter === "hidden"
              ? "Скрытые книги появятся здесь — отсюда их можно вернуть или удалить с диска."
              : "Перетащите файлы в папку библиотеки или переключите полку слева."}
          </p>
        </div>
      {:else}
        <div class="stage-head">
          <div class="stage-intro">
            <p class="stage-kicker">архив снов</p>
            <h1 class="stage-title">{stageHeading}</h1>
            <p class="stage-meta">
              {displayedBooks.length}
              {displayedBooks.length === 1 ? "книга" : displayedBooks.length < 5 ? "книги" : "книг"}
              {#if shelfFilter !== "all"}
                <span class="stage-dot">·</span>
                <span>{shelfFilter === "hidden" ? "не показываются в библиотеке" : "активная полка"}</span>
              {/if}
            </p>
          </div>
          <div class="stage-tools">
            <label class="search-shell">
              <svg viewBox="0 0 24 24" aria-hidden="true"><circle cx="11" cy="11" r="6.5" /><path d="m16 16 4 4" /></svg>
              <span class="sr-only">Поиск книг</span>
              <input type="search" placeholder="Название или автор" bind:value={searchQuery} />
            </label>
            <div class="sort-shell">
              <span class="sr-only">Упорядочить</span>
              <DreamSelect
                value={librarySort}
                ariaLabel="Упорядочить книги"
                compact
                options={Object.entries(LIBRARY_SORT_LABELS).map(([value, label]) => ({ value, label }))}
                onChange={(value) => setLibrarySort(value as LibrarySortMode)}
              />
            </div>
          </div>
        </div>
        {#if displayedBooks.length === 0}
          <div class="search-empty">
            <span class="search-empty-mark" aria-hidden="true">Aa</span>
            <h2>Книги не найдены</h2>
            <p>Попробуйте изменить запрос или посмотреть другую полку.</p>
            <button type="button" class="home-btn" onclick={() => (searchQuery = "")}>Сбросить поиск</button>
          </div>
        {:else}
        <ul class="book-grid">
          {#each displayedBooks as p (p)}
            {@const meta = snapshot!.metadata.books[p]}
            {@const fmt = getBookFormat(p)}
            {@const prog = fmt && meta ? readingProgress(meta, fmt) : null}
            {@const fileName = titleFromPath(p)}
            {@const shelfTitle = meta?.title?.trim() ?? ""}
            {@const cardTitle = shelfTitle || fileName}
            {@const hiddenBook = bookIsHidden(p)}
            <li class="book-cell">
              <button
                type="button"
                class="book-card"
                class:book-card-hidden={hiddenBook}
                onclick={(e) => (hiddenBook ? onCardContextMenu(e, p) : openBook(p))}
                oncontextmenu={(e) => onCardContextMenu(e, p)}
              >
                <span class="book-spine" aria-hidden="true"></span>
                <div
                  class="cover"
                  class:cover-pdf={fmt === "pdf"}
                  class:cover-epub={fmt === "epub"}
                  class:cover-fb2={fmt === "fb2"}
                  class:cover-typst={fmt === "typst"}
                  class:cover-unknown={!fmt}
                >
                  <BookCoverThumb
                    bookPath={p}
                    format={fmt}
                    cachedUrl={meta?.coverThumbDataUrl ?? null}
                    onCached={(u) => {
                      cacheBookCover(p, u);
                    }}
                  >
                    <span class="cover-k">{fmt ? formatBadgeLabel(fmt) : "?"}</span>
                  </BookCoverThumb>
                </div>
                <span class="card-title">{cardTitle}</span>
                {#if meta?.author?.trim()}
                  <span class="card-author">{meta.author.trim()}</span>
                {/if}
                {#if shelfTitle && shelfTitle !== fileName}
                  <span class="card-file">{fileName}</span>
                {/if}
                <span class="card-meta">{importanceLabel(meta?.importance ?? "normal")}</span>
                {#if hiddenBook}
                  <span class="card-badge-hidden">Скрыта</span>
                {/if}
                {#if meta?.translationExported}
                  <span class="card-badge-trans" title="Есть PDF translate_* с переводом">Переведена</span>
                {/if}
                {#if prog}
                  <div class="card-progress">
                    {#if prog.kind === "pdf"}
                      <div class="card-progress-track" aria-hidden="true">
                        <div class="card-progress-fill" style:width="{prog.pct}%"></div>
                      </div>
                    {/if}
                    <span class="card-progress-cap">{prog.label}</span>
                  </div>
                {/if}
              </button>
              <button
                type="button"
                class="card-menu-btn"
                aria-label={`Действия с книгой «${cardTitle}»`}
                title="Действия с книгой"
                onclick={(e) => onCardContextMenu(e, p)}
              >•••</button>
            </li>
          {/each}
        </ul>
        {/if}
      {/if}
    </section>
  </div>
</div>

{#if ctxMenu}
  <!-- svelte-ignore a11y_click_events_have_key_events -->
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <div
    id="ctx-menu"
    class="ctx-menu"
    style="left:{ctxMenu.x}px;top:{ctxMenu.y}px;"
    onclick={(e) => e.stopPropagation()}
  >
    {#if bookIsHidden(ctxMenu.path)}
      <button type="button" class="ctx-item" onclick={() => void restoreBookFromMenu()}>
        Вернуть в библиотеку
      </button>
      <div class="ctx-separator" aria-hidden="true"></div>
      <button type="button" class="ctx-item ctx-item-danger" onclick={() => startBookAction("delete")}>
        Удалить файл…
      </button>
    {:else}
      <button type="button" class="ctx-item" onclick={() => startEdit()}>Редактирование</button>
      {#if isTauriRuntime() && getBookFormat(ctxMenu.path) === "pdf"}
        <button
          type="button"
          class="ctx-item"
          onclick={() => {
            fullTranslateBook = ctxMenu!.path;
            closeCtx();
          }}>Перевести книгу…</button>
      {/if}
      {#if isTauriRuntime() && getBookFormat(ctxMenu.path) && getBookFormat(ctxMenu.path) !== "typst"}
        <button type="button" class="ctx-item" onclick={() => void exportBookToTypstFromMenu()}>
          Экспорт в Typst…
        </button>
      {/if}
      <div class="ctx-separator" aria-hidden="true"></div>
      <button type="button" class="ctx-item" onclick={() => startBookAction("hide")}>Скрыть книгу</button>
      <button type="button" class="ctx-item ctx-item-danger" onclick={() => startBookAction("delete")}>
        Удалить файл…
      </button>
    {/if}
  </div>
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
  {@const actionTitle = snapshot?.metadata.books[bookAction.path]?.title?.trim() || titleFromPath(bookAction.path)}
  <!-- svelte-ignore a11y_click_events_have_key_events -->
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <div class="modal-back" onclick={closeBookAction}>
    <div
      class="action-dialog"
      role="alertdialog"
      aria-modal="true"
      aria-labelledby="book-action-title"
      tabindex="-1"
      onclick={(event) => event.stopPropagation()}
    >
      <div class="action-symbol" class:action-symbol-danger={bookAction.kind === "delete"} aria-hidden="true">
        {bookAction.kind === "delete" ? "×" : "◌"}
      </div>
      <p class="action-kicker">{bookAction.kind === "delete" ? "Файл на диске" : "Видимость книги"}</p>
      <h2 id="book-action-title">
        {bookAction.kind === "delete" ? "Удалить книгу безвозвратно?" : "Скрыть книгу из библиотеки?"}
      </h2>
      <p class="action-copy">
        {bookAction.kind === "delete"
          ? "Reader удалит сам файл книги. Отменить это действие после подтверждения нельзя."
          : "Файл останется на диске, но исчезнет с обычных полок. Вернуть его можно в разделе «Скрытые»."}
      </p>
      <div class="action-book">
        <strong>{actionTitle}</strong>
        <span>{bookAction.path}</span>
      </div>
      {#if bookActionError}
        <p class="action-error" role="alert">{bookActionError}</p>
      {/if}
      <div class="action-buttons">
        <button type="button" class="home-btn home-btn-ghost" onclick={closeBookAction} disabled={bookActionBusy}>
          Отмена
        </button>
        <button
          type="button"
          class="home-btn"
          class:action-danger-button={bookAction.kind === "delete"}
          onclick={() => void confirmBookAction()}
          disabled={bookActionBusy}
        >
          {bookActionBusy
            ? "Подождите…"
            : bookAction.kind === "delete"
              ? "Удалить файл"
              : "Скрыть книгу"}
        </button>
      </div>
    </div>
  </div>
{/if}

{#if editOpen && editPath && editMeta && editDraft}
  <!-- svelte-ignore a11y_click_events_have_key_events -->
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <!-- svelte-ignore a11y_interactive_supports_focus -->
  <div class="modal-back" onclick={closeEdit}>
    <div
      class="modal edit-modal"
      onclick={(e) => e.stopPropagation()}
      role="dialog"
      aria-modal="true"
      aria-labelledby="edit-book-title"
      tabindex="-1"
    >
      <aside class="edit-portrait">
        <div class="edit-halo" aria-hidden="true"></div>
        <div class="edit-cover">
          <BookCoverThumb
            bookPath={editPath}
            format={getBookFormat(editPath)}
            cachedUrl={editMeta.coverThumbDataUrl ?? null}
          >
            <span class="edit-cover-mark">{getBookFormat(editPath)?.toUpperCase() ?? "BOOK"}</span>
          </BookCoverThumb>
        </div>
        <p class="edit-portrait-kicker">личная карточка</p>
        <p class="edit-portrait-name">{editDraft.title.trim() || titleFromPath(editPath)}</p>
        {#if editDraft.author.trim()}
          <p class="edit-portrait-author">{editDraft.author}</p>
        {/if}
        <p class="modal-path" title={editPath}>{titleFromPath(editPath)}</p>
      </aside>

      <form
        class="edit-form"
        onsubmit={(e) => {
          e.preventDefault();
          saveEdit();
        }}
      >
        <header class="modal-h">
          <div>
            <p class="edit-kicker">Сведения о книге</p>
            <h3 id="edit-book-title">Настройте свою полку</h3>
            <p class="edit-subtitle">Название, заметки и полки останутся только в вашей библиотеке.</p>
          </div>
          <button type="button" class="modal-x" aria-label="Закрыть без сохранения" onclick={closeEdit}>×</button>
        </header>

        <div class="field-grid">
          <label class="field">
            <span>Название</span>
            <input type="text" placeholder="Например, Невидимые города" bind:value={editDraft.title} />
          </label>

          <label class="field">
            <span>Автор</span>
            <input type="text" placeholder="Имя автора" bind:value={editDraft.author} />
          </label>
        </div>

        <fieldset class="edit-group">
          <legend>Важность</legend>
          <div class="importance-options">
            {#each IMPORTANCE_OPTIONS as o (o.value)}
              <button
                type="button"
                class="importance-option"
                class:importance-option-active={editDraft.importance === o.value}
                aria-pressed={editDraft.importance === o.value}
                onclick={() => (editDraft = { ...editDraft!, importance: o.value })}
              >
                <span class="importance-dot importance-{o.value}" aria-hidden="true"></span>
                {o.label}
              </button>
            {/each}
          </div>
        </fieldset>

        <fieldset class="edit-group">
          <legend>Полки</legend>
          <p class="field-hint">Книга может жить сразу в нескольких коллекциях.</p>
          <div class="shelf-checks">
            {#each shelvesSorted as s (s.id)}
              <label class="shelf-check" class:shelf-check-active={editDraft.shelfIds.includes(s.id)}>
                <input
                  type="checkbox"
                  checked={editDraft.shelfIds.includes(s.id)}
                  onchange={() => toggleBookShelf(s.id)}
                />
                <span class="shelf-check-mark" aria-hidden="true">✓</span>
                <span>{s.name}</span>
              </label>
            {/each}
          </div>
        </fieldset>

        <label class="field field-notes">
          <span>Личные заметки</span>
          <textarea
            rows="6"
            placeholder="Что хочется сохранить после этой книги…"
            bind:value={editDraft.review}
          ></textarea>
          <small>{editDraft.review.length} символов</small>
        </label>

        <details class="advanced-fields">
          <summary>Параметры экспорта</summary>
          <label class="field">
            <span>Стиль Typst</span>
            <input
              type="text"
              placeholder="Путь к .typ внутри библиотеки"
              bind:value={editDraft.typstStyleRelativePath}
            />
            <small>Оставьте пустым, чтобы использовать общий стиль из настроек.</small>
          </label>
        </details>

        <footer class="modal-f">
          <p>Изменения применятся после сохранения</p>
          <div class="modal-actions">
            <button type="button" class="home-btn home-btn-ghost" onclick={closeEdit}>Отмена</button>
            <button type="submit" class="home-btn home-btn-primary">Сохранить</button>
          </div>
        </footer>
      </form>
    </div>
  </div>
{/if}

<style>
  /* ——— Shell & atmosphere ——— */
  .home {
    position: relative;
    isolation: isolate;
    display: flex;
    flex-direction: column;
    min-height: 100vh;
    min-height: 100dvh;
    overflow: hidden;
    color: var(--text-soft);
  }

  .home-ambient {
    position: fixed;
    inset: -20%;
    z-index: 0;
    pointer-events: none;
    background:
      radial-gradient(ellipse 80% 55% at 12% -8%, color-mix(in srgb, var(--accent) 28%, transparent), transparent 52%),
      radial-gradient(ellipse 70% 50% at 92% 8%, color-mix(in srgb, var(--accent-2) 18%, transparent), transparent 50%),
      radial-gradient(ellipse 55% 40% at 50% 100%, color-mix(in srgb, var(--accent) 12%, transparent), transparent 45%);
    opacity: 0.95;
  }

  .home-film {
    position: fixed;
    inset: 0;
    z-index: 0;
    pointer-events: none;
    opacity: 0.35;
    background-image: radial-gradient(color-mix(in srgb, var(--text-soft) 6%, transparent) 0.8px, transparent 0.8px);
    background-size: 20px 20px;
    mix-blend-mode: multiply;
  }

  :global([data-theme="dark"]) .home-film {
    mix-blend-mode: soft-light;
    opacity: 0.22;
  }

  /* ——— Top bar ——— */
  .home-top {
    position: relative;
    z-index: 2;
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 0.65rem 1.1rem;
    padding: 0.85rem clamp(1rem, 3.5vw, 1.6rem);
    border-bottom: 1px solid color-mix(in srgb, var(--border-soft) 88%, transparent);
    background: color-mix(in srgb, var(--panel-veil) 96%, transparent);
    backdrop-filter: blur(14px) saturate(1.2);
    flex-shrink: 0;
    box-shadow: 0 1px 0 color-mix(in srgb, var(--elevated-soft) 65%, transparent);
  }

  .home-brand {
    display: flex;
    align-items: center;
    gap: 0.75rem;
  }

  .home-mark {
    display: grid;
    place-items: center;
    width: 2.5rem;
    height: 2.5rem;
    border-radius: 0.75rem;
    background: linear-gradient(
      135deg,
      color-mix(in srgb, var(--accent) 55%, var(--elevated-soft)),
      color-mix(in srgb, var(--accent-2) 35%, var(--panel-soft))
    );
    box-shadow:
      0 4px 16px color-mix(in srgb, var(--accent-2) 22%, transparent),
      inset 0 1px 0 color-mix(in srgb, #fff 42%, transparent);
    position: relative;
    overflow: hidden;
  }

  .home-mark::after {
    content: "";
    position: absolute;
    inset: -40%;
    background: repeating-linear-gradient(
      -28deg,
      transparent,
      transparent 6px,
      color-mix(in srgb, #fff 12%, transparent) 6px,
      color-mix(in srgb, #fff 12%, transparent) 7px
    );
    opacity: 0.5;
    transform: rotate(8deg);
  }

  .home-brand-text {
    display: flex;
    flex-direction: column;
    gap: 0.12rem;
    min-width: 0;
  }

  .home-logo {
    font-family: system-ui, -apple-system, "Segoe UI", sans-serif;
    font-weight: 700;
    font-size: 1.28rem;
    letter-spacing: -0.03em;
    background: linear-gradient(
      120deg,
      var(--accent-2) 0%,
      color-mix(in srgb, var(--accent) 85%, var(--text-soft)) 55%,
      var(--accent-2) 100%
    );
    background-size: 160% auto;
    -webkit-background-clip: text;
    background-clip: text;
    color: transparent;
    line-height: 1.1;
  }

  .home-tagline {
    font-size: 0.72rem;
    letter-spacing: 0.07em;
    text-transform: uppercase;
    font-weight: 600;
    color: var(--muted);
  }

  .home-actions {
    display: flex;
    flex-wrap: wrap;
    gap: 0.5rem;
    margin-left: auto;
    align-items: center;
  }

  .home-btn {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    gap: 0.4rem;
    padding: 0.52rem 1rem;
    border-radius: 999px;
    border: 1px solid color-mix(in srgb, var(--border-soft) 92%, transparent);
    background: color-mix(in srgb, var(--elevated-soft) 94%, transparent);
    color: var(--text-soft);
    cursor: pointer;
    font-size: 0.86rem;
    font-weight: 600;
    font-family: system-ui, -apple-system, "Segoe UI", sans-serif;
    transition:
      transform 0.18s cubic-bezier(0.33, 1, 0.68, 1),
      border-color 0.15s ease,
      box-shadow 0.15s ease,
      background 0.15s ease;
  }

  .home-btn:hover {
    transform: translateY(-1px);
    border-color: color-mix(in srgb, var(--accent) 42%, var(--border-soft));
    box-shadow: 0 10px 28px color-mix(in srgb, var(--accent-2) 12%, transparent);
  }

  .home-btn:active {
    transform: translateY(0);
  }

  .home-btn:focus-visible {
    outline: 2px solid color-mix(in srgb, var(--accent) 55%, transparent);
    outline-offset: 3px;
  }

  .home-btn-primary {
    border-color: color-mix(in srgb, var(--accent-2) 38%, var(--border-soft));
    background: linear-gradient(
      135deg,
      color-mix(in srgb, var(--accent) 22%, var(--elevated-soft)),
      color-mix(in srgb, var(--accent-2) 12%, var(--elevated-soft))
    );
    box-shadow:
      inset 0 1px 0 color-mix(in srgb, #fff 44%, transparent),
      0 6px 20px color-mix(in srgb, var(--accent-2) 14%, transparent);
  }

  :global([data-theme="dark"]) .home-btn-primary {
    box-shadow:
      inset 0 1px 0 color-mix(in srgb, #fff 14%, transparent),
      0 6px 24px rgba(0, 0, 0, 0.35);
  }

  .home-btn-ghost {
    background: transparent;
    border-color: color-mix(in srgb, var(--border-soft) 72%, transparent);
  }

  .home-btn-small {
    padding: 0.38rem 0.72rem;
    min-width: 2.35rem;
  }

  .home-btn svg,
  .search-shell svg {
    width: 1.05rem;
    height: 1.05rem;
    flex: 0 0 auto;
    fill: none;
    stroke: currentColor;
    stroke-width: 1.8;
    stroke-linecap: round;
    stroke-linejoin: round;
  }

  .home-settings-link {
    text-decoration: none;
  }

  .home-shelves-toggle,
  .rail-close,
  .rail-backdrop {
    display: none;
  }

  .home-path {
    flex-basis: 100%;
    margin: 0;
    display: flex;
    align-items: baseline;
    gap: 0.5rem;
    min-width: 0;
    font-family: ui-monospace, monospace;
    font-size: 0.72rem;
    color: var(--muted);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .home-path-label {
    flex-shrink: 0;
    font-size: 0.62rem;
    font-weight: 700;
    letter-spacing: 0.14em;
    text-transform: uppercase;
    color: color-mix(in srgb, var(--accent-2) 75%, var(--muted));
  }

  .home-path-value {
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    opacity: 0.95;
  }

  .home-banner {
    position: relative;
    z-index: 2;
    padding: 0.65rem 1.25rem;
    flex-shrink: 0;
    font-size: 0.87rem;
    background: linear-gradient(
      90deg,
      color-mix(in srgb, var(--danger) 32%, var(--panel-soft)),
      color-mix(in srgb, var(--danger) 18%, var(--panel-soft))
    );
    border-bottom: 1px solid color-mix(in srgb, var(--danger) 35%, transparent);
    color: color-mix(in srgb, var(--danger) 58%, var(--text-soft));
    overflow-wrap: anywhere;
  }

  /* ——— Layout ——— */
  .home-main {
    position: relative;
    z-index: 1;
    display: grid;
    grid-template-columns: minmax(272px, 320px) 1fr;
    grid-template-rows: minmax(0, 1fr);
    flex: 1;
    min-height: 0;
  }

  .home-rail {
    padding: 1rem 0 1rem 1rem;
    min-height: 0;
    display: flex;
    flex-direction: column;
    align-items: stretch;
  }

  .home-rail-inner {
    display: flex;
    flex-direction: column;
    gap: 0.85rem;
    min-height: 0;
    flex: 1;
    padding: 0.25rem;
  }

  .home-panel {
    border-radius: 1.15rem;
    border: 1px solid color-mix(in srgb, var(--border-soft) 80%, transparent);
    background: color-mix(in srgb, var(--elevated-soft) 88%, var(--panel-soft));
    box-shadow:
      var(--shadow-soft),
      inset 0 1px 0 color-mix(in srgb, #fff 52%, transparent);
    padding: 1.05rem 1rem 1.1rem;
    flex: 1;
    overflow-y: auto;
    overflow-x: hidden;
    backdrop-filter: blur(8px);
  }

  :global([data-theme="dark"]) .home-panel {
    box-shadow: var(--shadow-soft);
  }

  .home-panel-settings {
    flex: 0 1 auto;
    max-height: 48%;
  }

  .home-panel-head {
    margin-bottom: 1rem;
    display: flex;
    align-items: flex-start;
    justify-content: space-between;
    gap: 1rem;
  }

  .home-panel-title {
    margin: 0;
    font-size: 1.05rem;
    font-weight: 700;
    font-family: system-ui, -apple-system, "Segoe UI", sans-serif;
    letter-spacing: -0.02em;
    color: var(--text-soft);
  }

  .home-panel-sub {
    margin: 0.3rem 0 0;
    font-size: 0.78rem;
    line-height: 1.4;
    color: var(--muted);
  }

  .shelf-list {
    display: flex;
    flex-direction: column;
    gap: 0.42rem;
  }

  .shelf-row-wrap {
    display: flex;
    align-items: stretch;
    gap: 0.38rem;
  }

  .shelf-pill {
    flex: 1;
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 0.65rem;
    text-align: left;
    padding: 0.55rem 0.65rem;
    border-radius: 0.85rem;
    border: 1px solid transparent;
    background: color-mix(in srgb, var(--panel-soft) 55%, transparent);
    color: var(--text-soft);
    cursor: pointer;
    font-size: 0.9rem;
    font-family: inherit;
    transition:
      border-color 0.15s ease,
      background 0.15s ease,
      box-shadow 0.15s ease,
      transform 0.14s ease;
  }

  .shelf-pill:hover {
    background: color-mix(in srgb, var(--accent) 9%, var(--elevated-soft));
    border-color: color-mix(in srgb, var(--accent) 22%, transparent);
    transform: translateX(2px);
  }

  .shelf-pill-active {
    border-color: color-mix(in srgb, var(--accent-2) 38%, var(--border-soft));
    background: linear-gradient(
      120deg,
      color-mix(in srgb, var(--accent) 16%, var(--elevated-soft)),
      color-mix(in srgb, var(--accent-2) 8%, var(--elevated-soft))
    );
    box-shadow: inset 0 1px 0 color-mix(in srgb, #fff 42%, transparent);
  }

  :global([data-theme="dark"]) .shelf-pill-active {
    box-shadow: inset 0 1px 0 color-mix(in srgb, #fff 12%, transparent);
  }

  .shelf-name {
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-weight: 600;
  }

  .shelf-count {
    flex-shrink: 0;
    min-width: 1.85rem;
    padding: 0.12rem 0.45rem;
    border-radius: 999px;
    font-size: 0.7rem;
    font-weight: 700;
    font-variant-numeric: tabular-nums;
    letter-spacing: 0.03em;
    background: color-mix(in srgb, var(--text-soft) 8%, transparent);
    color: var(--muted);
  }

  .shelf-pill-active .shelf-count {
    background: color-mix(in srgb, var(--accent-2) 22%, transparent);
    color: var(--accent-2);
  }

  .shelf-remove {
    width: 2.15rem;
    flex-shrink: 0;
    border: none;
    border-radius: 0.65rem;
    background: transparent;
    color: var(--muted);
    cursor: pointer;
    font-size: 1.1rem;
    line-height: 1;
    transition:
      background 0.12s ease,
      color 0.12s ease,
      transform 0.12s ease;
  }

  .shelf-remove:hover {
    color: var(--danger);
    background: color-mix(in srgb, var(--danger) 10%, transparent);
    transform: scale(1.05);
  }

  .add-shelf {
    display: flex;
    gap: 0.45rem;
    margin-top: 0.95rem;
  }

  .add-shelf input {
    flex: 1;
    min-width: 0;
    padding: 0.48rem 0.62rem;
    border-radius: 0.85rem;
    border: 1px solid color-mix(in srgb, var(--border-soft) 90%, transparent);
    background: var(--elevated-soft);
    color: var(--text-soft);
    font-size: 0.86rem;
  }

  .add-shelf input::placeholder {
    color: color-mix(in srgb, var(--muted) 88%, transparent);
  }

  .add-shelf input:focus-visible {
    outline: 2px solid color-mix(in srgb, var(--accent) 40%, transparent);
    outline-offset: 1px;
  }

  /* ——— Stage (library) ——— */
  .home-stage {
    position: relative;
    overflow-y: auto;
    overflow-x: hidden;
    padding: clamp(1rem, 2.2vw, 1.85rem);
    padding-left: clamp(0.85rem, 2vw, 1.35rem);
    min-height: 0;
    min-width: 0;
  }

  .home-stage::before {
    content: "";
    position: absolute;
    inset: -1px;
    z-index: 0;
    background: linear-gradient(
      155deg,
      color-mix(in srgb, var(--bg-soft) 97%, transparent) 0%,
      color-mix(in srgb, var(--panel-soft) 35%, var(--bg-soft)) 52%,
      var(--bg-soft) 100%
    );
    pointer-events: none;
  }

  .home-stage > * {
    position: relative;
    z-index: 1;
  }

  /* empty */
  .empty-state {
    max-width: 26rem;
    margin: clamp(5rem, 12vh, 9rem) auto;
    text-align: center;
    padding: 2.5rem 1.75rem;
    position: relative;
  }

  .empty-state-muted {
    max-width: 24rem;
  }

  .empty-orbit {
    position: absolute;
    left: 50%;
    top: 42%;
    width: clamp(260px, 65vw, 380px);
    height: clamp(260px, 65vw, 380px);
    translate: -50% -50%;
    border-radius: 50%;
    border: 1px solid color-mix(in srgb, var(--accent-2) 22%, transparent);
    background: radial-gradient(
      circle at 40% 35%,
      color-mix(in srgb, var(--accent) 14%, transparent),
      transparent 62%
    );
    opacity: 0.9;
    z-index: -1;
  }

  .empty-orbit-soft {
    opacity: 0.55;
    border-color: color-mix(in srgb, var(--border-soft) 55%, transparent);
  }

  .empty-kicker {
    margin: 0 0 0.85rem;
    font-size: 0.74rem;
    font-weight: 700;
    letter-spacing: 0.2em;
    text-transform: uppercase;
    color: color-mix(in srgb, var(--accent-2) 70%, var(--muted));
  }

  .empty-heading {
    margin: 0 0 0.85rem;
    font-family: system-ui, -apple-system, "Segoe UI", sans-serif;
    font-size: clamp(1.45rem, 4.2vw, 2rem);
    font-weight: 800;
    letter-spacing: -0.035em;
    line-height: 1.12;
    color: var(--text-soft);
    text-wrap: balance;
  }

  .empty-body {
    margin: 0 0 1.75rem;
    font-size: 1rem;
    line-height: 1.62;
    color: var(--muted);
    text-wrap: pretty;
  }

  .empty-cta {
    padding-inline: 1.65rem;
  }

  /* stage header */
  .stage-head {
    display: flex;
    flex-wrap: wrap;
    align-items: flex-end;
    justify-content: space-between;
    gap: 1rem 1.5rem;
    margin-bottom: 1.5rem;
    padding-bottom: 1rem;
    border-bottom: 1px solid color-mix(in srgb, var(--border-soft) 65%, transparent);
  }

  .stage-tools {
    display: flex;
    align-items: center;
    justify-content: flex-end;
    gap: 0.6rem;
    min-width: min(100%, 28rem);
  }

  .search-shell {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    flex: 1 1 15rem;
    min-width: 10rem;
    height: 2.55rem;
    padding: 0 0.8rem;
    border: 1px solid color-mix(in srgb, var(--border-soft) 86%, transparent);
    border-radius: 999px;
    background: color-mix(in srgb, var(--elevated-soft) 94%, transparent);
    color: var(--muted);
    box-shadow: inset 0 1px 0 color-mix(in srgb, #fff 42%, transparent);
    transition: border-color 0.15s ease, box-shadow 0.15s ease, background 0.15s ease;
  }

  .search-shell:focus-within {
    border-color: color-mix(in srgb, var(--accent) 55%, var(--border-soft));
    box-shadow: var(--focus-ring);
    background: var(--elevated-soft);
  }

  .search-shell input {
    width: 100%;
    min-width: 0;
    border: 0;
    outline: 0;
    background: transparent;
    color: var(--text-soft);
    font-size: 0.85rem;
  }

  .search-shell input::placeholder {
    color: var(--muted);
    opacity: 0.85;
  }

  .search-shell input::-webkit-search-cancel-button {
    cursor: pointer;
  }

  .stage-kicker {
    margin: 0 0 0.35rem;
    font-size: 0.74rem;
    font-weight: 700;
    letter-spacing: 0.18em;
    text-transform: uppercase;
    color: color-mix(in srgb, var(--accent-2) 72%, var(--muted));
  }

  .stage-title {
    margin: 0;
    font-family: system-ui, -apple-system, "Segoe UI", sans-serif;
    font-size: clamp(1.65rem, 4.5vw, 2.15rem);
    font-weight: 800;
    letter-spacing: -0.04em;
    line-height: 1.05;
    background: linear-gradient(
      120deg,
      var(--text-soft) 0%,
      color-mix(in srgb, var(--accent-2) 45%, var(--text-soft)) 100%
    );
    -webkit-background-clip: text;
    background-clip: text;
    color: transparent;
  }

  .stage-meta {
    margin: 0.45rem 0 0;
    font-size: 0.9rem;
    color: var(--muted);
    letter-spacing: 0.01em;
  }

  .stage-dot {
    opacity: 0.5;
    margin: 0 0.35rem;
  }

  .sort-shell {
    display: block;
    flex: 0 1 12.5rem;
  }

  .sr-only {
    position: absolute;
    width: 1px;
    height: 1px;
    padding: 0;
    margin: -1px;
    overflow: hidden;
    clip: rect(0, 0, 0, 0);
    white-space: nowrap;
    border: 0;
  }

  .search-empty {
    width: min(100%, 30rem);
    margin: clamp(2.5rem, 9vh, 6rem) auto;
    padding: 2rem;
    text-align: center;
    border: 1px dashed color-mix(in srgb, var(--border-soft) 92%, transparent);
    border-radius: var(--radius-xl);
    background: color-mix(in srgb, var(--elevated-soft) 52%, transparent);
  }

  .search-empty-mark {
    display: grid;
    place-items: center;
    width: 3.4rem;
    height: 3.4rem;
    margin: 0 auto 1rem;
    border-radius: 1.05rem;
    background: color-mix(in srgb, var(--accent) 14%, var(--elevated-soft));
    color: var(--accent-2);
    font: 700 0.9rem/1 system-ui, sans-serif;
    letter-spacing: -0.04em;
  }

  .search-empty h2 {
    margin: 0;
    color: var(--text-soft);
    font: 750 1.25rem/1.2 system-ui, sans-serif;
    letter-spacing: -0.025em;
  }

  .search-empty p {
    margin: 0.55rem 0 1.25rem;
    color: var(--muted);
    font-size: 0.88rem;
    line-height: 1.5;
  }

  /* ——— Book grid ——— */
  .book-grid {
    list-style: none;
    margin: 0;
    padding: 0 0 2rem;
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(min(100%, 10.25rem), 1fr));
    gap: 1.25rem 1.35rem;
    align-items: stretch;
  }

  .book-cell {
    display: flex;
    min-width: 0;
    perspective: 820px;
    position: relative;
  }

  .book-cell:nth-child(3n + 2) .book-card {
    transform: rotateZ(-0.55deg);
  }

  .book-cell:nth-child(3n) .book-card {
    transform: rotateZ(0.45deg);
  }

  .book-card {
    position: relative;
    display: flex;
    flex-direction: column;
    align-items: stretch;
    gap: 0.52rem;
    width: 100%;
    padding: 0.65rem 0.65rem 0.75rem;
    border: 1px solid color-mix(in srgb, var(--border-soft) 75%, transparent);
    background: linear-gradient(
      165deg,
      color-mix(in srgb, var(--elevated-soft) 100%, transparent),
      color-mix(in srgb, var(--panel-soft) 22%, var(--elevated-soft))
    );
    cursor: pointer;
    text-align: left;
    border-radius: 1.05rem;
    box-shadow:
      0 2px 0 color-mix(in srgb, #fff 52%, transparent) inset,
      0 18px 42px color-mix(in srgb, var(--text-soft) 6%, transparent);
    transition:
      transform 0.22s cubic-bezier(0.33, 1, 0.68, 1),
      border-color 0.18s ease,
      box-shadow 0.22s ease;
  }

  :global([data-theme="dark"]) .book-card {
    box-shadow:
      0 1px 0 color-mix(in srgb, #fff 12%, transparent) inset,
      0 20px 48px rgba(0, 0, 0, 0.35);
  }

  .book-spine {
    position: absolute;
    left: 0.62rem;
    top: 1.15rem;
    bottom: 42%;
    width: 3px;
    border-radius: 2px;
    background: linear-gradient(
      180deg,
      transparent,
      color-mix(in srgb, var(--accent-2) 45%, transparent),
      color-mix(in srgb, var(--accent) 25%, transparent),
      transparent
    );
    opacity: 0;
    transition: opacity 0.18s ease;
    pointer-events: none;
  }

  .book-card:hover .book-spine,
  .book-card:focus-visible .book-spine {
    opacity: 1;
  }

  .book-card:hover {
    transform: translateY(-6px) rotateZ(0deg) scale(1.02);
    border-color: color-mix(in srgb, var(--accent-2) 32%, var(--border-soft));
    box-shadow:
      0 2px 0 color-mix(in srgb, #fff 48%, transparent) inset,
      var(--shadow-book),
      0 0 0 1px color-mix(in srgb, var(--accent) 14%, transparent);
  }

  :global([data-theme="dark"]) .book-card:hover {
    box-shadow:
      0 1px 0 color-mix(in srgb, #fff 10%, transparent) inset,
      var(--shadow-book),
      0 0 0 1px color-mix(in srgb, var(--accent) 22%, transparent);
  }

  .book-card:focus-visible {
    outline: 2px solid color-mix(in srgb, var(--accent) 55%, transparent);
    outline-offset: 4px;
  }

  .card-menu-btn {
    position: absolute;
    top: 0.92rem;
    right: 0.92rem;
    z-index: 3;
    display: grid;
    place-items: center;
    width: 2rem;
    height: 2rem;
    padding: 0 0 0.35rem;
    border: 1px solid color-mix(in srgb, var(--border-soft) 72%, transparent);
    border-radius: 999px;
    background: color-mix(in srgb, var(--panel-elevated) 88%, transparent);
    color: var(--text-soft);
    box-shadow: 0 5px 16px rgba(0, 0, 0, 0.12);
    backdrop-filter: blur(8px);
    cursor: pointer;
    font: 700 0.82rem/1 system-ui, sans-serif;
    letter-spacing: 0.05em;
    opacity: 0;
    transform: translateY(-3px);
    transition: opacity 0.16s ease, transform 0.16s ease, background 0.16s ease;
  }

  .book-cell:hover .card-menu-btn,
  .card-menu-btn:focus-visible {
    opacity: 1;
    transform: translateY(0);
  }

  .card-menu-btn:hover {
    background: var(--panel-elevated);
    color: var(--accent-2);
  }

  /* Явные классы — не пересекаются с «нейтральной» обложкой и не путаются с PDF */
  .cover {
    aspect-ratio: 3 / 4;
    border-radius: 0.72rem;
    display: flex;
    align-items: center;
    justify-content: center;
    overflow: hidden;
    box-shadow:
      0 4px 16px color-mix(in srgb, var(--text-soft) 12%, transparent),
      inset 0 1px 0 color-mix(in srgb, #fff 38%, transparent);
    border: 1px solid color-mix(in srgb, var(--border-soft) 78%, #b0a69a);
    background: linear-gradient(160deg, #e5e0da, #d0cac2);
  }
  .cover :global(.thumb-fallback) {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 100%;
    height: 100%;
  }

  .cover.cover-pdf {
    background: linear-gradient(155deg, #e5d9cc, #c9b8a8);
    border-color: color-mix(in srgb, #a08060 28%, var(--border-soft));
  }

  .cover.cover-epub {
    background: linear-gradient(155deg, #dcd6ee, #b8aed4);
    border-color: color-mix(in srgb, #6b5b9e 22%, var(--border-soft));
  }

  .cover.cover-fb2 {
    background: linear-gradient(155deg, #7ecfb0, #3d9b7a);
    border-color: color-mix(in srgb, #1d6b52 45%, var(--border-soft));
    box-shadow:
      0 2px 12px rgba(30, 110, 85, 0.25),
      inset 0 1px 0 rgba(255, 255, 255, 0.35);
  }

  .cover.cover-typst {
    background: linear-gradient(155deg, #e8c86a, #c49a3c);
    border-color: color-mix(in srgb, #8b6914 35%, var(--border-soft));
  }

  .cover.cover-unknown {
    background: linear-gradient(155deg, #e8e4df, #cdc8c0);
  }

  .cover-k {
    font-size: 0.7rem;
    font-weight: 750;
    letter-spacing: 0.16em;
    color: #3d3833;
  }

  .cover.cover-pdf .cover-k {
    color: #4a3528;
  }

  .cover.cover-epub .cover-k {
    color: #3d3555;
  }

  .cover.cover-fb2 .cover-k {
    color: #f4fffb;
    text-shadow: 0 1px 2px rgba(0, 0, 0, 0.2);
  }

  .cover.cover-typst .cover-k {
    color: #2a2218;
  }

  .cover.cover-unknown .cover-k {
    color: var(--muted);
  }

  .card-title {
    font-size: 0.84rem;
    line-height: 1.35;
    color: var(--text-soft);
    display: -webkit-box;
    -webkit-line-clamp: 3;
    line-clamp: 3;
    -webkit-box-orient: vertical;
    overflow: hidden;
    min-height: 3.2em;
  }

  .card-file {
    font-size: 0.68rem;
    line-height: 1.25;
    color: var(--muted);
    display: -webkit-box;
    -webkit-line-clamp: 1;
    line-clamp: 1;
    -webkit-box-orient: vertical;
    overflow: hidden;
    word-break: break-word;
  }

  .card-author {
    margin-top: -0.2rem;
    font-size: 0.74rem;
    line-height: 1.3;
    color: var(--muted);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .card-meta {
    font-size: 0.72rem;
    color: var(--accent-2);
    opacity: 0.95;
  }

  .card-badge-trans {
    font-size: 0.65rem;
    font-weight: 600;
    padding: 0.12rem 0.4rem;
    border-radius: 6px;
    background: color-mix(in srgb, var(--accent) 22%, transparent);
    color: var(--accent-2);
    width: fit-content;
    margin-top: 0.15rem;
  }

  .card-progress {
    display: flex;
    flex-direction: column;
    gap: 0.28rem;
    margin-top: 0.15rem;
    min-height: 0;
  }

  .card-progress-track {
    height: 4px;
    border-radius: 999px;
    background: color-mix(in srgb, var(--border-soft) 88%, transparent);
    overflow: hidden;
  }

  .card-progress-fill {
    height: 100%;
    border-radius: inherit;
    background: linear-gradient(
      90deg,
      color-mix(in srgb, var(--accent) 55%, var(--accent-2)),
      var(--accent-2)
    );
    min-width: 4px;
    transition: width 0.25s ease;
  }

  .card-progress-cap {
    font-size: 0.66rem;
    line-height: 1.3;
    color: var(--muted);
    font-variant-numeric: tabular-nums;
    display: -webkit-box;
    -webkit-line-clamp: 2;
    line-clamp: 2;
    -webkit-box-orient: vertical;
    overflow: hidden;
  }

  .ctx-menu {
    position: fixed;
    z-index: 80;
    min-width: 11.5rem;
    padding: 0.4rem;
    border-radius: 0.85rem;
    border: 1px solid color-mix(in srgb, var(--border-soft) 88%, transparent);
    background: color-mix(in srgb, var(--panel-elevated) 98%, transparent);
    backdrop-filter: blur(10px);
    box-shadow: var(--shadow-float);
  }

  .ctx-item {
    width: 100%;
    text-align: left;
    padding: 0.48rem 0.65rem;
    border: none;
    border-radius: 0.55rem;
    background: transparent;
    color: var(--text-soft);
    cursor: pointer;
    font-size: 0.89rem;
    font-weight: 500;
    transition: background 0.12s ease;
  }

  .ctx-item:hover {
    background: color-mix(in srgb, var(--accent) 14%, transparent);
  }

  .modal-back {
    position: fixed;
    inset: 0;
    z-index: 90;
    background: color-mix(in srgb, var(--text-soft) 48%, transparent);
    display: flex;
    align-items: center;
    justify-content: center;
    padding: 1rem;
    backdrop-filter: blur(10px);
  }

  .modal {
    width: min(26rem, 100%);
    max-height: min(90vh, 36rem);
    overflow: auto;
    border-radius: 1.1rem;
    border: 1px solid color-mix(in srgb, var(--border-soft) 85%, transparent);
    background: var(--panel-elevated);
    box-shadow: var(--shadow-float);
    padding: 0 1.15rem 1.05rem;
  }

  .modal-h {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 0.85rem 0 0.25rem;
    position: sticky;
    top: 0;
    background: var(--panel-elevated);
    z-index: 1;
  }

  .modal-h h3 {
    margin: 0;
    font-size: 1rem;
    font-weight: 600;
    color: var(--text-soft);
    font-family: system-ui, sans-serif;
  }

  .modal-x {
    border: none;
    background: transparent;
    font-size: 1.35rem;
    line-height: 1;
    color: var(--muted);
    cursor: pointer;
    padding: 0.2rem;
    border-radius: 0.45rem;
  }

  .modal-x:hover {
    color: var(--text-soft);
    background: var(--elevated-soft);
  }

  .modal-path {
    margin: 0 0 1rem;
    font-size: 0.88rem;
    color: var(--muted);
    word-break: break-word;
  }

  .field-hint {
    margin: 0 0 0.4rem;
    font-size: 0.72rem;
    color: var(--muted);
  }

  .shelf-checks {
    display: flex;
    flex-direction: column;
    gap: 0.35rem;
  }

  .shelf-check {
    display: flex;
    align-items: center;
    gap: 0.45rem;
    font-size: 0.86rem;
    cursor: pointer;
    color: var(--text-soft);
  }

  .field {
    display: flex;
    flex-direction: column;
    gap: 0.35rem;
    margin-bottom: 0.85rem;
    font-size: 0.78rem;
    color: var(--muted);
  }

  .field textarea {
    padding: 0.48rem 0.55rem;
    border-radius: 0.65rem;
    border: 1px solid var(--border-soft);
    background: var(--elevated-soft);
    color: var(--text-soft);
    font-size: 0.9rem;
  }

  .field textarea {
    resize: vertical;
    min-height: 7rem;
    line-height: 1.45;
  }

  .modal-f {
    padding-top: 0.5rem;
    display: flex;
    justify-content: flex-end;
  }

  .shelf-hidden-separator {
    height: 1px;
    margin: 0.45rem 0.35rem;
    background: color-mix(in srgb, var(--accent) 16%, var(--border-soft));
  }

  .shelf-hidden-name {
    display: inline-flex;
    align-items: center;
    gap: 0.45rem;
  }

  .shelf-hidden-mark {
    color: var(--muted);
    font-size: 1rem;
  }

  .book-card-hidden {
    opacity: 0.72;
    filter: saturate(0.42);
  }

  .book-card-hidden:hover {
    opacity: 0.94;
    filter: saturate(0.72);
  }

  .card-badge-hidden {
    width: fit-content;
    margin-top: 0.1rem;
    padding: 0.14rem 0.48rem;
    border: 1px dashed color-mix(in srgb, var(--muted) 34%, var(--border-soft));
    border-radius: 999px;
    color: var(--muted);
    font-size: 0.62rem;
    font-weight: 650;
    letter-spacing: 0.06em;
    text-transform: uppercase;
  }

  .ctx-separator {
    height: 1px;
    margin: 0.3rem 0.4rem;
    background: color-mix(in srgb, var(--border-soft) 72%, transparent);
  }

  .ctx-item-danger {
    color: var(--danger);
  }

  .ctx-item-danger:hover {
    background: color-mix(in srgb, var(--danger) 10%, transparent);
  }

  .action-dialog {
    width: min(29rem, 100%);
    padding: clamp(1.35rem, 4vw, 2rem);
    border: 1px solid color-mix(in srgb, var(--accent) 22%, var(--border-soft));
    border-radius: 1.65rem;
    background: color-mix(in srgb, var(--panel-elevated) 96%, transparent);
    box-shadow: var(--shadow-float);
    text-align: center;
  }

  .action-symbol {
    display: grid;
    place-items: center;
    width: 3.5rem;
    height: 3.5rem;
    margin: 0 auto 1rem;
    border-radius: 50%;
    background: color-mix(in srgb, var(--accent) 16%, var(--elevated-soft));
    color: var(--accent-2);
    font: 500 1.5rem/1 Georgia, serif;
    box-shadow: 0 0 0 0.5rem color-mix(in srgb, var(--accent) 5%, transparent);
  }

  .action-symbol-danger {
    background: color-mix(in srgb, var(--danger) 13%, var(--elevated-soft));
    color: var(--danger);
    box-shadow: 0 0 0 0.5rem color-mix(in srgb, var(--danger) 4%, transparent);
  }

  .action-kicker {
    margin: 0 0 0.55rem;
    color: var(--accent-2);
    font-size: 0.63rem;
    font-weight: 750;
    letter-spacing: 0.18em;
    text-transform: uppercase;
  }

  .action-dialog h2 {
    margin: 0;
    color: var(--text-soft);
    font-family: Georgia, "Times New Roman", serif;
    font-size: clamp(1.45rem, 5vw, 1.9rem);
    font-weight: 500;
    letter-spacing: -0.035em;
    line-height: 1.12;
  }

  .action-copy {
    margin: 0.8rem auto 1rem;
    max-width: 24rem;
    color: var(--muted);
    font-size: 0.82rem;
    line-height: 1.55;
  }

  .action-book {
    display: flex;
    flex-direction: column;
    gap: 0.25rem;
    padding: 0.75rem 0.85rem;
    border: 1px solid color-mix(in srgb, var(--accent) 15%, var(--border-soft));
    border-radius: 0.95rem;
    background: color-mix(in srgb, var(--panel-soft) 46%, transparent);
    text-align: left;
  }

  .action-book strong {
    color: var(--text-soft);
    font-size: 0.82rem;
    line-height: 1.35;
    overflow-wrap: anywhere;
  }

  .action-book span {
    overflow: hidden;
    color: var(--muted);
    font-family: ui-monospace, monospace;
    font-size: 0.62rem;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .action-error {
    margin: 0.8rem 0 0;
    padding: 0.65rem 0.75rem;
    border-radius: 0.8rem;
    background: color-mix(in srgb, var(--danger) 10%, transparent);
    color: var(--danger);
    font-size: 0.74rem;
    line-height: 1.4;
  }

  .action-buttons {
    display: flex;
    justify-content: flex-end;
    gap: 0.55rem;
    margin-top: 1.15rem;
  }

  .action-danger-button {
    border-color: color-mix(in srgb, var(--danger) 42%, var(--border-soft));
    background: var(--danger);
    color: #fff;
  }

  .action-danger-button:hover {
    border-color: var(--danger);
    background: color-mix(in srgb, var(--danger) 88%, #38111b);
    color: #fff;
  }

  .action-buttons .home-btn:disabled {
    opacity: 0.5;
    cursor: default;
    transform: none;
  }

  /* ——— Dreamcore interface skin ——— */
  .home {
    background: var(--bg-soft);
  }

  .home-ambient {
    inset: 0;
    opacity: 1;
    background:
      radial-gradient(circle at 84% 12%, color-mix(in srgb, #fff5c9 58%, transparent) 0 4.5rem, transparent 4.65rem),
      radial-gradient(ellipse 34rem 18rem at 7% 5%, color-mix(in srgb, var(--accent) 26%, transparent), transparent 66%),
      radial-gradient(ellipse 32rem 20rem at 100% 42%, color-mix(in srgb, #b8d7ef 24%, transparent), transparent 70%),
      radial-gradient(ellipse 38rem 18rem at 48% 110%, color-mix(in srgb, var(--accent) 14%, transparent), transparent 68%);
  }

  .home-ambient::before,
  .home-ambient::after {
    content: "";
    position: absolute;
    border-radius: 50%;
    border: 1px solid color-mix(in srgb, var(--accent-2) 14%, transparent);
  }

  .home-ambient::before {
    width: min(48vw, 36rem);
    aspect-ratio: 1;
    left: -18rem;
    bottom: -23rem;
  }

  .home-ambient::after {
    width: 11rem;
    height: 11rem;
    right: 7%;
    top: 4rem;
    box-shadow: 0 0 0 2.4rem color-mix(in srgb, var(--accent) 3%, transparent);
  }

  .home-film {
    opacity: 0.18;
    background-image:
      radial-gradient(color-mix(in srgb, var(--accent-2) 11%, transparent) 0.7px, transparent 0.7px);
    background-size: 28px 28px;
    mix-blend-mode: normal;
  }

  .home-top {
    min-height: 4.75rem;
    padding: 0.9rem clamp(1rem, 4vw, 2rem);
    border-color: color-mix(in srgb, var(--accent) 15%, var(--border-soft));
    background: color-mix(in srgb, var(--panel-veil) 96%, var(--bg-soft));
    backdrop-filter: none;
    box-shadow: none;
  }

  .home-mark {
    width: 2.45rem;
    height: 2.45rem;
    border-radius: 50%;
    overflow: visible;
    background: linear-gradient(145deg, #fff9dc, color-mix(in srgb, var(--accent) 56%, #fff));
    box-shadow:
      0 0 0 0.38rem color-mix(in srgb, var(--accent) 9%, transparent),
      0 8px 25px color-mix(in srgb, var(--accent-2) 22%, transparent);
  }

  .home-mark::after {
    inset: 0.54rem 0.38rem auto auto;
    width: 0.8rem;
    height: 0.8rem;
    border-radius: 50%;
    background: color-mix(in srgb, var(--accent-2) 22%, transparent);
    opacity: 0.78;
    transform: none;
  }

  .home-logo {
    font-family: Georgia, "Times New Roman", serif;
    font-size: 1.45rem;
    font-weight: 500;
    letter-spacing: -0.045em;
  }

  .home-tagline {
    font-size: 0.61rem;
    letter-spacing: 0.13em;
    font-weight: 650;
  }

  .home-btn {
    min-height: 2.5rem;
    padding: 0.52rem 0.95rem;
    border-color: color-mix(in srgb, var(--accent) 18%, var(--border-soft));
    background: color-mix(in srgb, var(--elevated-soft) 92%, var(--panel-soft));
    box-shadow: none;
    backdrop-filter: none;
  }

  .home-btn-primary {
    border-color: color-mix(in srgb, var(--accent-2) 28%, var(--border-soft));
    background: var(--text-soft);
    color: var(--panel-elevated);
    box-shadow: 0 10px 28px color-mix(in srgb, var(--accent-2) 19%, transparent);
  }

  .home-btn-primary:hover {
    color: var(--panel-elevated);
    background: color-mix(in srgb, var(--text-soft) 90%, var(--accent-2));
  }

  :global([data-theme="dark"]) .home-btn-primary {
    color: var(--bg-soft);
    background: var(--text-soft);
  }

  .home-path {
    padding-left: 3.25rem;
    opacity: 0.72;
  }

  .home-main {
    grid-template-columns: minmax(250px, 284px) 1fr;
  }

  .home-rail {
    padding: 1.25rem 0 1.25rem 1.25rem;
  }

  .home-panel {
    border-color: color-mix(in srgb, var(--accent) 15%, var(--border-soft));
    border-radius: 1.5rem;
    background: color-mix(in srgb, var(--panel-elevated) 92%, var(--bg-soft));
    box-shadow: 0 16px 55px color-mix(in srgb, var(--accent-2) 7%, transparent);
    backdrop-filter: none;
  }

  .home-panel-title {
    font-family: Georgia, "Times New Roman", serif;
    font-size: 1.2rem;
    font-weight: 500;
  }

  .shelf-pill {
    min-height: 2.65rem;
    border-radius: 1rem;
    background: transparent;
  }

  .shelf-pill:hover {
    transform: none;
    background: color-mix(in srgb, var(--accent) 8%, transparent);
  }

  .shelf-pill-active {
    border-color: color-mix(in srgb, var(--accent) 20%, var(--border-soft));
    background: color-mix(in srgb, var(--accent) 14%, var(--elevated-soft));
    box-shadow: none;
  }

  .add-shelf input,
  .search-shell {
    border-color: color-mix(in srgb, var(--accent) 18%, var(--border-soft));
    background: color-mix(in srgb, var(--elevated-soft) 72%, transparent);
    box-shadow: none;
    backdrop-filter: blur(12px);
  }

  .home-stage {
    padding: clamp(1.2rem, 3vw, 2.35rem);
  }

  .home-stage::before {
    background: transparent;
  }

  .stage-head {
    margin-bottom: 1.9rem;
    padding-bottom: 1.35rem;
    border-color: color-mix(in srgb, var(--accent) 15%, var(--border-soft));
  }

  .stage-kicker,
  .empty-kicker {
    color: var(--accent-2);
    font-size: 0.64rem;
    letter-spacing: 0.22em;
  }

  .stage-title,
  .empty-heading {
    font-family: Georgia, "Times New Roman", serif;
    font-weight: 500;
    letter-spacing: -0.045em;
  }

  .stage-title {
    font-size: clamp(2rem, 5vw, 2.7rem);
    background: none;
    color: var(--text-soft);
  }

  .book-grid {
    grid-template-columns: repeat(auto-fill, minmax(min(100%, 11rem), 1fr));
    gap: clamp(1rem, 2.5vw, 1.7rem);
  }

  .book-cell {
    content-visibility: auto;
    contain: layout paint style;
    contain-intrinsic-size: auto 19rem;
  }

  .book-cell:nth-child(n) .book-card {
    transform: none;
  }

  .book-card {
    gap: 0.58rem;
    padding: 0.65rem 0.65rem 0.85rem;
    border-color: transparent;
    border-radius: 1.35rem;
    background: color-mix(in srgb, var(--panel-elevated) 88%, var(--bg-soft));
    box-shadow: none;
    backdrop-filter: none;
  }

  :global([data-theme="dark"]) .book-card {
    box-shadow: none;
  }

  .book-card:hover {
    transform: translateY(-5px);
    border-color: color-mix(in srgb, var(--accent) 20%, transparent);
    background: color-mix(in srgb, var(--panel-elevated) 88%, transparent);
    box-shadow: var(--shadow-book);
  }

  .cover {
    border-radius: 0.9rem 1.25rem 1.25rem 0.9rem;
  }

  .card-title {
    font-family: Georgia, "Times New Roman", serif;
    font-size: 0.93rem;
    line-height: 1.28;
  }

  .card-meta {
    font-size: 0.63rem;
    letter-spacing: 0.08em;
    text-transform: uppercase;
  }

  .ctx-menu {
    padding: 0.5rem;
    border-radius: 1.1rem;
    border-color: color-mix(in srgb, var(--accent) 20%, var(--border-soft));
    background: color-mix(in srgb, var(--panel-elevated) 92%, transparent);
    backdrop-filter: blur(22px);
  }

  .ctx-item {
    min-height: 2.5rem;
    border-radius: 0.78rem;
  }

  /* ——— Dreamcore book editor ——— */
  .modal-back {
    padding: clamp(0.75rem, 3vw, 2rem);
    background: color-mix(in srgb, #171226 52%, transparent);
    backdrop-filter: blur(18px) saturate(0.9);
  }

  .modal.edit-modal {
    display: grid;
    grid-template-columns: minmax(12.5rem, 0.72fr) minmax(0, 1.8fr);
    width: min(54rem, 100%);
    max-height: min(92vh, 52rem);
    overflow: hidden;
    padding: 0;
    border-radius: 1.75rem;
    border-color: color-mix(in srgb, var(--accent) 24%, var(--border-soft));
    background: color-mix(in srgb, var(--panel-elevated) 96%, transparent);
    box-shadow:
      0 32px 100px rgba(31, 21, 52, 0.34),
      inset 0 1px 0 color-mix(in srgb, #fff 72%, transparent);
  }

  .edit-portrait {
    position: relative;
    overflow: hidden;
    display: flex;
    flex-direction: column;
    align-items: center;
    min-width: 0;
    padding: 2.4rem 1.4rem 1.5rem;
    text-align: center;
    border-right: 1px solid color-mix(in srgb, var(--accent) 18%, var(--border-soft));
    background:
      radial-gradient(circle at 50% 15%, color-mix(in srgb, var(--accent) 30%, transparent), transparent 38%),
      linear-gradient(160deg, color-mix(in srgb, var(--panel-soft) 72%, var(--elevated-soft)), var(--bg-soft));
  }

  .edit-halo {
    position: absolute;
    width: 15rem;
    height: 15rem;
    top: -5.2rem;
    left: 50%;
    translate: -50% 0;
    border: 1px solid color-mix(in srgb, var(--accent) 34%, transparent);
    border-radius: 50%;
    box-shadow:
      0 0 0 2.1rem color-mix(in srgb, var(--accent) 4%, transparent),
      0 0 0 4.2rem color-mix(in srgb, var(--accent-2) 3%, transparent);
  }

  .edit-cover {
    position: relative;
    z-index: 1;
    width: min(9.5rem, 72%);
    aspect-ratio: 3 / 4;
    overflow: hidden;
    margin-bottom: 1.35rem;
    border-radius: 0.9rem 1.35rem 1.35rem 0.9rem;
    border: 1px solid color-mix(in srgb, var(--accent-2) 22%, var(--border-soft));
    background: linear-gradient(145deg, #d9d0f3, #aebde8 54%, #d8e5ef);
    box-shadow: 0 22px 45px color-mix(in srgb, var(--accent-2) 24%, transparent);
  }

  .edit-cover::after {
    content: "";
    position: absolute;
    inset: 0 auto 0 0;
    width: 0.45rem;
    pointer-events: none;
    background: linear-gradient(90deg, rgba(36, 24, 64, 0.28), transparent);
  }

  .edit-cover-mark {
    color: #312847;
    font-size: 0.67rem;
    font-weight: 750;
    letter-spacing: 0.18em;
  }

  .edit-portrait-kicker,
  .edit-kicker {
    margin: 0 0 0.5rem;
    color: var(--accent-2);
    font-size: 0.66rem;
    font-weight: 750;
    letter-spacing: 0.18em;
    text-transform: uppercase;
  }

  .edit-portrait-name {
    margin: 0;
    max-width: 100%;
    color: var(--text-soft);
    font-family: Georgia, "Times New Roman", serif;
    font-size: 1.15rem;
    line-height: 1.18;
    overflow-wrap: anywhere;
  }

  .edit-portrait-author {
    margin: 0.4rem 0 0;
    color: var(--muted);
    font-size: 0.77rem;
    line-height: 1.35;
  }

  .edit-form {
    min-width: 0;
    overflow-y: auto;
    padding: 0 1.6rem 1.4rem;
  }

  .edit-form .modal-h {
    align-items: flex-start;
    gap: 1.2rem;
    padding: 1.55rem 0 1.25rem;
    background: color-mix(in srgb, var(--panel-elevated) 96%, transparent);
    backdrop-filter: blur(14px);
    z-index: 3;
  }

  .edit-form .modal-h h3 {
    margin: 0;
    font-family: Georgia, "Times New Roman", serif;
    font-size: clamp(1.45rem, 3vw, 1.85rem);
    font-weight: 500;
    letter-spacing: -0.035em;
    line-height: 1.08;
  }

  .edit-subtitle {
    max-width: 31rem;
    margin: 0.55rem 0 0;
    color: var(--muted);
    font-size: 0.82rem;
    line-height: 1.45;
  }

  .edit-form .modal-x {
    display: grid;
    place-items: center;
    flex: 0 0 auto;
    width: 2.25rem;
    height: 2.25rem;
    padding: 0;
    border: 1px solid color-mix(in srgb, var(--border-soft) 84%, transparent);
    border-radius: 50%;
    background: color-mix(in srgb, var(--elevated-soft) 88%, transparent);
    font-size: 1.15rem;
    transition: transform 0.15s ease, background 0.15s ease, color 0.15s ease;
  }

  .edit-form .modal-x:hover {
    background: color-mix(in srgb, var(--accent) 14%, var(--elevated-soft));
    transform: rotate(4deg);
  }

  .edit-portrait .modal-path {
    width: 100%;
    margin: auto 0 0;
    padding-top: 1.5rem;
    font-family: ui-monospace, monospace;
    font-size: 0.62rem;
    opacity: 0.72;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .field-grid {
    display: grid;
    grid-template-columns: repeat(2, minmax(0, 1fr));
    gap: 0.85rem;
  }

  .edit-group {
    min-width: 0;
    margin: 1.15rem 0 0;
    padding: 0;
    border: 0;
  }

  .edit-group legend,
  .edit-form .field > span {
    display: block;
    margin: 0 0 0.48rem;
    color: var(--text-soft);
    font-size: 0.73rem;
    font-weight: 700;
    letter-spacing: 0.025em;
  }

  .importance-options {
    display: grid;
    grid-template-columns: repeat(4, minmax(0, 1fr));
    gap: 0.45rem;
  }

  .importance-option {
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 0.4rem;
    min-width: 0;
    min-height: 2.5rem;
    padding: 0.5rem 0.45rem;
    border: 1px solid color-mix(in srgb, var(--border-soft) 86%, transparent);
    border-radius: 0.85rem;
    background: color-mix(in srgb, var(--elevated-soft) 76%, transparent);
    color: var(--muted);
    cursor: pointer;
    font-size: 0.72rem;
    transition: border-color 0.15s ease, background 0.15s ease, transform 0.15s ease;
  }

  .importance-option:hover {
    transform: translateY(-1px);
    border-color: color-mix(in srgb, var(--accent) 42%, var(--border-soft));
  }

  .importance-option-active {
    border-color: color-mix(in srgb, var(--accent-2) 42%, var(--border-soft));
    background: color-mix(in srgb, var(--accent) 14%, var(--elevated-soft));
    color: var(--text-soft);
    box-shadow: inset 0 0 0 1px color-mix(in srgb, var(--accent) 10%, transparent);
  }

  .importance-dot {
    width: 0.45rem;
    height: 0.45rem;
    flex: 0 0 auto;
    border-radius: 50%;
    background: #9f9aa8;
  }

  .importance-low { background: #9ebdb4; }
  .importance-normal { background: #9c91c8; }
  .importance-high { background: #d69a9f; }
  .importance-essential { background: #c76478; }

  .edit-form .field-hint {
    margin: -0.15rem 0 0.6rem;
    font-size: 0.7rem;
    line-height: 1.4;
  }

  .edit-form .shelf-checks {
    flex-direction: row;
    flex-wrap: wrap;
    gap: 0.45rem;
  }

  .edit-form .shelf-check {
    display: inline-flex;
    gap: 0.38rem;
    min-height: 2.25rem;
    padding: 0.4rem 0.7rem;
    border: 1px solid color-mix(in srgb, var(--border-soft) 82%, transparent);
    border-radius: 999px;
    background: color-mix(in srgb, var(--elevated-soft) 72%, transparent);
    font-size: 0.75rem;
    color: var(--muted);
    transition: border-color 0.15s ease, background 0.15s ease, color 0.15s ease;
  }

  .edit-form .shelf-check input {
    position: absolute;
    opacity: 0;
    pointer-events: none;
  }

  .shelf-check-mark {
    display: grid;
    place-items: center;
    width: 1rem;
    height: 1rem;
    border-radius: 50%;
    background: var(--panel-soft);
    color: transparent;
    font-size: 0.62rem;
  }

  .edit-form .shelf-check-active {
    border-color: color-mix(in srgb, var(--accent-2) 38%, var(--border-soft));
    background: color-mix(in srgb, var(--accent) 14%, var(--elevated-soft));
    color: var(--text-soft);
  }

  .shelf-check-active .shelf-check-mark {
    color: #fff;
    background: var(--accent-2);
  }

  .edit-form .field {
    gap: 0;
    min-width: 0;
    margin: 0;
  }

  .edit-form .field input,
  .edit-form .field textarea {
    display: block;
    width: 100%;
    min-width: 0;
    padding: 0.72rem 0.8rem;
    border-radius: 0.9rem;
    border: 1px solid color-mix(in srgb, var(--border-soft) 88%, transparent);
    outline: none;
    background: color-mix(in srgb, var(--elevated-soft) 88%, transparent);
    color: var(--text-soft);
    font-size: 0.85rem;
    line-height: 1.35;
    box-shadow: inset 0 1px 0 color-mix(in srgb, #fff 48%, transparent);
    transition: border-color 0.15s ease, box-shadow 0.15s ease, background 0.15s ease;
  }

  .edit-form .field input::placeholder,
  .edit-form .field textarea::placeholder {
    color: color-mix(in srgb, var(--muted) 76%, transparent);
  }

  .edit-form .field input:focus,
  .edit-form .field textarea:focus {
    border-color: color-mix(in srgb, var(--accent-2) 52%, var(--border-soft));
    background: var(--elevated-soft);
    box-shadow: 0 0 0 3px color-mix(in srgb, var(--accent) 15%, transparent);
  }

  .edit-form .field textarea {
    resize: vertical;
    min-height: 6.6rem;
    line-height: 1.55;
  }

  .edit-form .field small {
    margin-top: 0.35rem;
    color: var(--muted);
    font-size: 0.65rem;
    line-height: 1.35;
  }

  .field-notes {
    margin-top: 1.15rem !important;
  }

  .field-notes small {
    align-self: flex-end;
  }

  .advanced-fields {
    margin-top: 1rem;
    border-top: 1px solid color-mix(in srgb, var(--border-soft) 72%, transparent);
  }

  .advanced-fields summary {
    padding: 0.9rem 0;
    color: var(--muted);
    cursor: pointer;
    font-size: 0.74rem;
    font-weight: 650;
    list-style: none;
  }

  .advanced-fields summary::-webkit-details-marker { display: none; }

  .advanced-fields summary::after {
    content: "+";
    float: right;
    font-size: 1rem;
    font-weight: 400;
  }

  .advanced-fields[open] summary::after { content: "−"; }

  .advanced-fields .field { padding-bottom: 0.35rem; }

  .edit-form .modal-f {
    align-items: center;
    gap: 1rem;
    margin-top: 1rem;
    padding-top: 1rem;
    border-top: 1px solid color-mix(in srgb, var(--border-soft) 72%, transparent);
  }

  .modal-f > p {
    margin: 0 auto 0 0;
    color: var(--muted);
    font-size: 0.66rem;
  }

  .modal-actions {
    display: flex;
    gap: 0.5rem;
  }

  @media (max-width: 900px) {
    .home-main {
      grid-template-columns: 1fr;
      grid-template-rows: minmax(0, 1fr);
    }

    .home-rail {
      position: fixed;
      inset: 0 auto 0 0;
      z-index: 60;
      width: min(88vw, 21rem);
      min-height: 100dvh;
      padding:
        max(0.75rem, env(safe-area-inset-top))
        0.75rem
        max(0.75rem, env(safe-area-inset-bottom))
        max(0.75rem, env(safe-area-inset-left));
      background: color-mix(in srgb, var(--bg-soft) 92%, transparent);
      backdrop-filter: blur(18px) saturate(1.15);
      box-shadow: var(--shadow-float);
      transform: translateX(-104%);
      visibility: hidden;
      transition: transform 0.24s cubic-bezier(0.33, 1, 0.68, 1), visibility 0.24s;
    }

    .home-rail-open {
      transform: translateX(0);
      visibility: visible;
    }

    .home-rail-inner {
      overflow-y: auto;
    }

    .home-panel {
      flex: 0 0 auto;
    }

    .home-panel:first-child {
      flex: 1 0 auto;
    }

    .rail-close {
      display: grid;
      place-items: center;
      width: 2.75rem;
      height: 2.75rem;
      border: 0;
      border-radius: 999px;
      background: var(--panel-soft);
      color: var(--muted);
      cursor: pointer;
      font-size: 1.25rem;
    }

    .rail-backdrop {
      position: fixed;
      inset: 0;
      z-index: 55;
      border: 0;
      background: color-mix(in srgb, #111 42%, transparent);
      backdrop-filter: blur(3px);
      opacity: 0;
      visibility: hidden;
      transition: opacity 0.2s ease, visibility 0.2s;
    }

    .rail-backdrop-open {
      display: block;
      opacity: 1;
      visibility: visible;
    }

    .home-shelves-toggle {
      display: inline-flex;
    }

    .home-panel-settings {
      max-height: none;
    }

    .stage-tools {
      width: 100%;
      justify-content: stretch;
    }

  }

  @media (max-width: 768px) {
    .modal.edit-modal {
      display: block;
      overflow-y: auto;
    }

    .edit-portrait {
      display: grid;
      grid-template-columns: 4.5rem minmax(0, 1fr);
      grid-template-rows: auto auto auto;
      column-gap: 0.9rem;
      min-height: 7.25rem;
      padding: 1rem 3.8rem 1rem 1rem;
      text-align: left;
      border-right: 0;
      border-bottom: 1px solid color-mix(in srgb, var(--accent) 18%, var(--border-soft));
    }

    .edit-halo {
      width: 11rem;
      height: 11rem;
      top: -7.2rem;
      left: auto;
      right: -2rem;
      translate: 0 0;
    }

    .edit-cover {
      grid-row: 1 / 4;
      width: 4.5rem;
      margin: 0;
      align-self: center;
    }

    .edit-portrait-kicker,
    .edit-portrait-name,
    .edit-portrait-author {
      grid-column: 2;
      align-self: end;
    }

    .edit-portrait-name { align-self: center; }
    .edit-portrait-author { align-self: start; }
    .edit-portrait .modal-path { display: none; }

    .edit-form {
      overflow: visible;
      padding-inline: 1rem;
    }

    .home-top {
      padding:
        max(0.75rem, env(safe-area-inset-top))
        max(clamp(0.85rem, 4vw, 1.35rem), env(safe-area-inset-right))
        0.75rem
        max(clamp(0.85rem, 4vw, 1.35rem), env(safe-area-inset-left));
    }

    .home-actions {
      flex: 0 1 auto;
      justify-content: flex-end;
      margin-left: auto;
    }

    .home-actions .home-btn {
      min-width: 2.75rem;
      min-height: 2.75rem;
      padding: 0.55rem 0.72rem;
    }

    .home-actions .action-label {
      display: none;
    }

    .home-path {
      order: 3;
    }

    .stage-head {
      flex-direction: column;
      align-items: stretch;
    }

    .book-grid {
      grid-template-columns: repeat(auto-fill, minmax(min(100%, 8.75rem), 1fr));
      gap: 1rem;
    }

    .book-card {
      padding: 0.52rem 0.52rem 0.62rem;
    }

    .home-stage {
      padding-bottom: max(1.25rem, env(safe-area-inset-bottom));
      overscroll-behavior: contain;
    }

    .card-menu-btn {
      top: 0.78rem;
      right: 0.78rem;
      opacity: 1;
      transform: none;
      width: 2.75rem;
      height: 2.75rem;
    }

    .book-cell:nth-child(n) .book-card {
      transform: none;
    }
  }

  @media (max-width: 420px) {
    .action-dialog {
      align-self: flex-end;
      width: 100%;
      border-radius: 1.55rem 1.55rem 0 0;
    }

    .action-buttons {
      flex-direction: column-reverse;
    }

    .action-buttons .home-btn {
      width: 100%;
    }

    .modal-back {
      padding: 0;
      align-items: flex-end;
    }

    .modal.edit-modal {
      width: 100%;
      max-height: 100dvh;
      min-height: 100dvh;
      border: 0;
      border-radius: 0;
      padding-bottom: env(safe-area-inset-bottom);
    }

    .field-grid { grid-template-columns: 1fr; }

    .importance-options {
      grid-template-columns: repeat(2, minmax(0, 1fr));
    }

    .edit-form .modal-h {
      padding-top: 1.15rem;
    }

    .edit-form .modal-f {
      align-items: stretch;
      flex-direction: column;
    }

    .modal-f > p { display: none; }
    .modal-actions { width: 100%; }
    .modal-actions .home-btn { flex: 1; }

    .home-tagline {
      display: none;
    }

    .home-mark {
      width: 2.2rem;
      height: 2.2rem;
    }

    .home-logo {
      font-size: 1.12rem;
    }

    .home-actions {
      gap: 0.35rem;
    }

    .home-actions .home-btn {
      width: 2.75rem;
      min-width: 2.75rem;
      min-height: 2.75rem;
      padding: 0;
    }

    .home-actions .folder-label,
    .home-shelves-toggle {
      font-size: 0;
      gap: 0;
    }

    .home-actions svg {
      width: 1.08rem;
      height: 1.08rem;
    }

    .stage-tools {
      flex-direction: column;
      align-items: stretch;
    }

    .search-shell,
    .sort-shell {
      flex-basis: auto;
      width: 100%;
    }

    .book-grid {
      gap: 0.85rem;
      grid-template-columns: repeat(2, minmax(0, 1fr));
    }

    .card-title {
      font-size: 0.8rem;
      min-height: 2.8em;
      -webkit-line-clamp: 2;
      line-clamp: 2;
    }
  }

  @media (max-width: 600px) {
    .ctx-menu {
      left: max(0.75rem, env(safe-area-inset-left)) !important;
      right: max(0.75rem, env(safe-area-inset-right));
      top: auto !important;
      bottom: max(0.75rem, env(safe-area-inset-bottom));
      width: auto;
      min-width: 0;
      max-height: min(70dvh, 28rem);
      overflow-y: auto;
      padding: 0.5rem;
      border-radius: 1.25rem;
    }

    .ctx-item {
      min-height: 2.75rem;
      padding: 0.65rem 0.75rem;
    }

    .action-dialog {
      padding-bottom: max(1.35rem, env(safe-area-inset-bottom));
    }

    .modal-actions .home-btn,
    .action-buttons .home-btn {
      min-height: 2.75rem;
    }
  }

  @media (max-height: 520px) and (orientation: landscape) {
    .home-top {
      min-height: 0;
      padding-top: max(0.45rem, env(safe-area-inset-top));
      padding-bottom: 0.45rem;
    }

    .home-path,
    .home-tagline {
      display: none;
    }

    .empty-state {
      margin-block: 1.5rem;
    }
  }
</style>
