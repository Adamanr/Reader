<script lang="ts">
  import { onMount } from "svelte";
  import { goto } from "$app/navigation";
  import { invoke } from "@tauri-apps/api/core";
  import { open } from "@tauri-apps/plugin-dialog";
  import type { BookMeta, Importance, LibraryMetadata, LibrarySnapshot, Shelf } from "$lib/types";
  import { IMPORTANCE_OPTIONS } from "$lib/types";
  import { formatBadgeLabel, getBookFormat } from "$lib/bookFormat";
  import { bookOnShelf, effectiveShelfIds } from "$lib/library/shelves";
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
  import { isTauriRuntime } from "$lib/isTauri";
  import { exportBookToTypst } from "$lib/typst/exportToTypst";

  let snapshot = $state<LibrarySnapshot | null>(null);
  let shelfFilter = $state<string>("all");
  let newShelfName = $state("");
  let banner = $state<string | null>(null);
  let saveTimer: ReturnType<typeof setTimeout> | null = null;
  let reviewDraft = $state("");

  let ctxMenu = $state<{ x: number; y: number; path: string } | null>(null);
  let editPath = $state<string | null>(null);
  const editOpen = $derived(editPath != null);

  let fullTranslateBook = $state<string | null>(null);

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
    let paths = snapshot.bookPaths;
    if (shelfFilter !== "all") {
      paths = paths.filter((p) =>
        bookOnShelf(snapshot!.metadata.books[p], shelfFilter),
      );
    }
    return paths.slice().sort(cmpBookPaths);
  });

  const editMeta = $derived(
    editPath && snapshot ? snapshot.metadata.books[editPath] ?? null : null,
  );

  function titleFromPath(path: string) {
    const i = path.lastIndexOf("/");
    return i >= 0 ? path.slice(i + 1) : path;
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
      snapshot = await invoke<LibrarySnapshot>("get_library_snapshot");
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
    if (snapshot) {
      snapshot = { ...snapshot, metadata: next };
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
    if (!editPath || !editMeta) return;
    let ids = [...effectiveShelfIds(editMeta)];
    if (ids.includes(shelfId)) {
      ids = ids.filter((id) => id !== shelfId);
      if (ids.length === 0) ids = ["default"];
    } else {
      ids.push(shelfId);
    }
    patchBook(editPath, { shelfIds: ids, shelfId: ids[0] ?? "default" });
  }

  function scheduleReviewSave(path: string, review: string) {
    if (saveTimer) clearTimeout(saveTimer);
    saveTimer = setTimeout(() => {
      patchBook(path, { review });
      saveTimer = null;
    }, 450);
  }

  function openBook(p: string) {
    if (!getBookFormat(p)) return;
    goto("/read?path=" + encodeURIComponent(p));
  }

  function onCardContextMenu(e: MouseEvent, p: string) {
    e.preventDefault();
    ctxMenu = { x: e.clientX, y: e.clientY, path: p };
  }

  function closeCtx() {
    ctxMenu = null;
  }

  function startEdit() {
    if (ctxMenu) {
      editPath = ctxMenu.path;
      const m = snapshot?.metadata.books[ctxMenu.path];
      reviewDraft = m?.review ?? "";
      closeCtx();
    }
  }

  function closeEdit() {
    editPath = null;
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
    void refresh();
  });

  $effect(() => {
    if (!ctxMenu) return;
    const onDown = (e: MouseEvent) => {
      const t = e.target as Node;
      const el = document.getElementById("ctx-menu");
      if (el && !el.contains(t)) closeCtx();
    };
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") closeCtx();
    };
    document.addEventListener("pointerdown", onDown, true);
    document.addEventListener("keydown", onKey);
    return () => {
      document.removeEventListener("pointerdown", onDown, true);
      document.removeEventListener("keydown", onKey);
    };
  });
</script>

<div class="shell">
  <header class="top">
    <div class="brand">
      <span class="logo">Reader</span>
      <span class="tag">книжная полка</span>
    </div>
    <div class="top-actions">
      <button type="button" class="btn primary" onclick={() => void pickFolder()}>Папка библиотеки</button>
      <button type="button" class="btn" onclick={() => void refresh()}>Обновить</button>
    </div>
    {#if snapshot?.libraryRoot}
      <p class="path" title={snapshot.libraryRoot}>{snapshot.libraryRoot}</p>
    {/if}
  </header>

  {#if banner}
    <div class="banner">{banner}</div>
  {/if}

  <div class="main">
    <aside class="sidebar">
      <section class="block">
        <h2>Полки</h2>
        <div class="shelf-list">
          <button
            type="button"
            class="shelf-row"
            class:active={shelfFilter === "all"}
            onclick={() => (shelfFilter = "all")}
          >
            Все книги
          </button>
          {#each shelvesSorted as s (s.id)}
            <div class="shelf-row-wrap">
              <button
                type="button"
                class="shelf-row"
                class:active={shelfFilter === s.id}
                onclick={() => (shelfFilter = s.id)}
              >
                {s.name}
              </button>
              {#if s.id !== "default"}
                <button
                  type="button"
                  class="icon-btn"
                  title="Удалить полку"
                  onclick={() => void removeShelf(s.id)}>×</button>
              {/if}
            </div>
          {/each}
        </div>
        <div class="add-shelf">
          <input placeholder="Новая полка…" bind:value={newShelfName} />
          <button type="button" class="btn small" onclick={() => void addShelf()}>+</button>
        </div>
      </section>

      <section class="block block-settings">
        <SettingsThemeCard />
      </section>
    </aside>

    <section class="content">
      {#if !snapshot?.libraryRoot}
        <div class="empty-hero">
          <p class="empty-title">Начните с папки</p>
          <p class="empty-text">
            Укажите каталог с PDF, EPUB и FB2 — книги появятся на полке. Нажмите на карточку, чтобы
            читать. Правая кнопка мыши — правка сведений.
          </p>
        </div>
      {:else if displayedBooks.length === 0}
        <div class="empty-hero">
          <p class="empty-title">Пока пусто</p>
          <p class="empty-text">Добавьте файлы в папку или смените полку слева.</p>
        </div>
      {:else}
        <div class="library-toolbar">
          <label class="sort-label">
            Сортировка
            <select
              class="sort-select"
              value={librarySort}
              onchange={(e) =>
                setLibrarySort((e.currentTarget as HTMLSelectElement).value as LibrarySortMode)}
            >
              <option value="recent">{LIBRARY_SORT_LABELS.recent}</option>
              <option value="title">{LIBRARY_SORT_LABELS.title}</option>
              <option value="title_desc">{LIBRARY_SORT_LABELS.title_desc}</option>
              <option value="importance">{LIBRARY_SORT_LABELS.importance}</option>
            </select>
          </label>
        </div>
        <ul class="book-grid">
          {#each displayedBooks as p (p)}
            {@const meta = snapshot!.metadata.books[p]}
            {@const fmt = getBookFormat(p)}
            {@const prog = fmt && meta ? readingProgress(meta, fmt) : null}
            {@const fileName = titleFromPath(p)}
            {@const shelfTitle = meta?.title?.trim() ?? ""}
            {@const cardTitle = shelfTitle || fileName}
            <li>
              <button
                type="button"
                class="book-card"
                onclick={() => openBook(p)}
                oncontextmenu={(e) => onCardContextMenu(e, p)}
              >
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
                      const cur = snapshot?.metadata.books[p]?.coverThumbDataUrl;
                      if (cur === u) return;
                      patchBook(p, { coverThumbDataUrl: u });
                    }}
                  >
                    <span class="cover-k">{fmt ? formatBadgeLabel(fmt) : "?"}</span>
                  </BookCoverThumb>
                </div>
                <span class="card-title">{cardTitle}</span>
                {#if shelfTitle && shelfTitle !== fileName}
                  <span class="card-file">{fileName}</span>
                {/if}
                <span class="card-meta">{importanceLabel(meta?.importance ?? "normal")}</span>
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
            </li>
          {/each}
        </ul>
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

{#if editOpen && editPath && editMeta}
  <!-- svelte-ignore a11y_click_events_have_key_events -->
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <!-- svelte-ignore a11y_interactive_supports_focus -->
  <div class="modal-back" onclick={closeEdit}>
    <div
      class="modal"
      onclick={(e) => e.stopPropagation()}
      role="dialog"
      aria-modal="true"
      tabindex="-1"
    >
      <header class="modal-h">
        <h3>Редактирование</h3>
        <button type="button" class="modal-x" onclick={closeEdit}>×</button>
      </header>
      <p class="modal-path">{titleFromPath(editPath)}</p>

      <label class="field">
        <span>Название книги</span>
        <input
          type="text"
          placeholder="Как отображать в цитатах"
          value={editMeta.title ?? ""}
          oninput={(e) =>
            patchBook(editPath!, {
              title: (e.currentTarget as HTMLInputElement).value.trim() || null,
            })}
        />
      </label>

      <label class="field">
        <span>Автор</span>
        <input
          type="text"
          placeholder="Для подписи цитат"
          value={editMeta.author ?? ""}
          oninput={(e) =>
            patchBook(editPath!, {
              author: (e.currentTarget as HTMLInputElement).value.trim() || null,
            })}
        />
      </label>

      <label class="field">
        <span>Шаблон стиля Typst при экспорте (путь в библиотеке)</span>
        <input
          type="text"
          placeholder="Пусто — общий стиль из настроек; например .reader-typst-themes/minimal.typ"
          value={editMeta.typstStyleRelativePath ?? ""}
          oninput={(e) => {
            const v = (e.currentTarget as HTMLInputElement).value.trim();
            patchBook(editPath!, { typstStyleRelativePath: v.length ? v : null });
          }}
        />
      </label>

      <label class="field">
        <span>Важность</span>
        <select
          value={editMeta.importance}
          onchange={(e) =>
            patchBook(editPath!, {
              importance: (e.currentTarget as HTMLSelectElement).value as Importance,
            })}
        >
          {#each IMPORTANCE_OPTIONS as o (o.value)}
            <option value={o.value}>{o.label}</option>
          {/each}
        </select>
      </label>

      <fieldset class="field shelf-field">
        <legend>Полки</legend>
        <p class="field-hint">Можно отметить несколько.</p>
        <div class="shelf-checks">
          {#each shelvesSorted as s (s.id)}
            <label class="shelf-check">
              <input
                type="checkbox"
                checked={effectiveShelfIds(editMeta).includes(s.id)}
                onchange={() => toggleBookShelf(s.id)}
              />
              {s.name}
            </label>
          {/each}
        </div>
      </fieldset>

      <label class="field">
        <span>Рецензия и заметки</span>
        <textarea
          rows="8"
          placeholder="Впечатления, цитаты…"
          bind:value={reviewDraft}
          oninput={() => scheduleReviewSave(editPath!, reviewDraft)}
        ></textarea>
      </label>

      <footer class="modal-f">
        <button type="button" class="btn primary" onclick={closeEdit}>Готово</button>
      </footer>
    </div>
  </div>
{/if}

<style>
  .shell {
    display: flex;
    flex-direction: column;
    min-height: 100vh;
    min-height: 100dvh;
  }

  .top {
    display: flex;
    align-items: center;
    gap: 0.75rem 1rem;
    padding: 0.75rem clamp(0.85rem, 3vw, 1.25rem);
    border-bottom: 1px solid var(--border-soft);
    background: var(--panel-veil);
    flex-shrink: 0;
    flex-wrap: wrap;
    backdrop-filter: blur(10px);
  }

  .brand {
    display: flex;
    align-items: baseline;
    gap: 0.55rem;
  }

  .logo {
    font-weight: 600;
    font-size: 1.2rem;
    letter-spacing: 0.02em;
    color: var(--accent-2);
    font-family: system-ui, sans-serif;
  }

  .tag {
    font-size: 0.78rem;
    color: var(--muted);
    font-weight: 500;
  }

  .top-actions {
    display: flex;
    flex-wrap: wrap;
    gap: 0.5rem;
    margin-left: auto;
    align-items: center;
  }

  .path {
    flex-basis: 100%;
    margin: 0;
    font-size: 0.74rem;
    color: var(--muted);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    font-family: ui-monospace, monospace;
    opacity: 0.9;
  }

  .btn {
    padding: 0.48rem 0.95rem;
    border-radius: var(--radius-md);
    border: 1px solid var(--border-soft);
    background: var(--elevated-soft);
    color: var(--text-soft);
    cursor: pointer;
    font-size: 0.88rem;
    transition:
      background 0.15s ease,
      border-color 0.15s ease;
  }

  .btn:hover {
    border-color: color-mix(in srgb, var(--accent) 35%, var(--border-soft));
    background: var(--panel-soft);
  }

  .btn.primary {
    background: color-mix(in srgb, var(--accent) 14%, var(--elevated-soft));
    border-color: color-mix(in srgb, var(--accent) 40%, var(--border-soft));
    color: var(--text-soft);
  }

  .btn.small {
    padding: 0.28rem 0.55rem;
  }

  .banner {
    padding: 0.55rem 1rem;
    background: color-mix(in srgb, #c45 18%, var(--panel-soft));
    color: #fde8e8;
    font-size: 0.88rem;
    flex-shrink: 0;
  }

  .main {
    display: grid;
    grid-template-columns: minmax(230px, 270px) 1fr;
    grid-template-rows: minmax(0, 1fr);
    flex: 1;
    min-height: 0;
    height: 100%;
    align-items: stretch;
  }

  .sidebar {
    border-right: 1px solid var(--border-soft);
    background: var(--panel-soft);
    display: flex;
    flex-direction: column;
    min-height: 0;
  }

  .block {
    padding: 1rem;
    flex: 1;
    overflow-y: auto;
  }

  .block-settings {
    flex: 0 0 auto;
    border-top: 1px solid var(--border-soft);
    padding-top: 1rem;
  }

  .block h2 {
    margin: 0 0 0.65rem;
    font-size: 0.7rem;
    text-transform: uppercase;
    letter-spacing: 0.12em;
    color: var(--muted);
    font-weight: 600;
    font-family: system-ui, sans-serif;
  }

  .shelf-list {
    display: flex;
    flex-direction: column;
    gap: 0.3rem;
  }

  .shelf-row-wrap {
    display: flex;
    align-items: stretch;
    gap: 0.25rem;
  }

  .shelf-row {
    flex: 1;
    text-align: left;
    padding: 0.5rem 0.55rem;
    border-radius: var(--radius-md);
    border: 1px solid transparent;
    background: transparent;
    color: var(--text-soft);
    cursor: pointer;
    font-size: 0.9rem;
    transition: background 0.12s ease;
  }

  .shelf-row:hover {
    background: var(--elevated-soft);
  }

  .shelf-row.active {
    border-color: color-mix(in srgb, var(--accent) 28%, var(--border-soft));
    background: color-mix(in srgb, var(--accent) 9%, transparent);
  }

  .icon-btn {
    width: 1.85rem;
    border: none;
    background: transparent;
    color: var(--muted);
    cursor: pointer;
    font-size: 1.05rem;
    line-height: 1;
    border-radius: var(--radius-sm);
  }

  .icon-btn:hover {
    color: #d08080;
    background: var(--elevated-soft);
  }

  .add-shelf {
    display: flex;
    gap: 0.4rem;
    margin-top: 0.75rem;
  }

  .add-shelf input {
    flex: 1;
    min-width: 0;
    padding: 0.42rem 0.55rem;
    border-radius: var(--radius-md);
    border: 1px solid var(--border-soft);
    background: var(--elevated-soft);
    color: var(--text-soft);
    font-size: 0.85rem;
  }

  .content {
    overflow-y: auto;
    padding: clamp(0.75rem, 2vw, 1.25rem) clamp(0.85rem, 3vw, 1.5rem);
    background: var(--bg-soft);
    min-height: 0;
    min-width: 0;
    width: 100%;
    box-sizing: border-box;
  }

  .library-toolbar {
    display: flex;
    align-items: center;
    justify-content: flex-end;
    margin-bottom: 0.75rem;
    gap: 0.75rem;
  }

  .sort-label {
    display: inline-flex;
    align-items: center;
    gap: 0.45rem;
    font-size: 0.85rem;
    color: var(--muted);
  }

  .sort-select {
    padding: 0.35rem 0.55rem;
    border-radius: var(--radius-md);
    border: 1px solid var(--border-soft);
    background: var(--elevated-soft);
    color: var(--text-soft);
    font-size: 0.85rem;
    max-width: 16rem;
  }

  .empty-hero {
    max-width: 28rem;
    margin: 8vh auto;
    text-align: center;
    padding: 2rem;
  }

  .empty-title {
    font-size: 1.25rem;
    font-weight: 600;
    color: var(--text-soft);
    margin: 0 0 0.5rem;
    font-family: system-ui, sans-serif;
  }

  .empty-text {
    margin: 0;
    color: var(--muted);
    line-height: 1.55;
    font-size: 0.95rem;
  }

  .book-grid {
    list-style: none;
    margin: 0;
    padding: 0;
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(min(100%, 9.75rem), 1fr));
    gap: 1rem 1.15rem;
    align-items: stretch;
  }

  .book-grid > li {
    display: flex;
    min-width: 0;
  }

  .book-card {
    display: flex;
    flex-direction: column;
    align-items: stretch;
    gap: 0.5rem;
    width: 100%;
    padding: 0.55rem 0.55rem 0.65rem;
    border: 1px solid color-mix(in srgb, var(--border-soft) 92%, transparent);
    background: var(--elevated-soft);
    cursor: pointer;
    text-align: left;
    border-radius: var(--radius-lg);
    box-shadow: 0 1px 0 rgba(255, 253, 250, 0.75) inset;
    transition:
      transform 0.15s ease,
      border-color 0.15s ease,
      box-shadow 0.15s ease;
  }

  .book-card:hover {
    transform: translateY(-2px);
    border-color: color-mix(in srgb, var(--accent) 28%, var(--border-soft));
    box-shadow: var(--shadow-soft);
  }

  .book-card:focus-visible {
    outline: 2px solid color-mix(in srgb, var(--accent) 55%, transparent);
    outline-offset: 3px;
  }

  /* Явные классы — не пересекаются с «нейтральной» обложкой и не путаются с PDF */
  .cover {
    aspect-ratio: 3 / 4;
    border-radius: var(--radius-md);
    display: flex;
    align-items: center;
    justify-content: center;
    overflow: hidden;
    box-shadow: 0 2px 10px rgba(61, 56, 51, 0.06);
    border: 1px solid color-mix(in srgb, var(--border-soft) 85%, #b0a69a);
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
    min-width: 11rem;
    padding: 0.35rem;
    border-radius: var(--radius-md);
    border: 1px solid var(--border-soft);
    background: var(--panel-elevated);
    box-shadow: var(--shadow-float);
  }

  .ctx-item {
    width: 100%;
    text-align: left;
    padding: 0.45rem 0.6rem;
    border: none;
    border-radius: var(--radius-sm);
    background: transparent;
    color: var(--text-soft);
    cursor: pointer;
    font-size: 0.88rem;
  }

  .ctx-item:hover {
    background: color-mix(in srgb, var(--accent) 10%, transparent);
  }

  .modal-back {
    position: fixed;
    inset: 0;
    z-index: 90;
    background: color-mix(in srgb, #2a2218 55%, transparent);
    display: flex;
    align-items: center;
    justify-content: center;
    padding: 1rem;
    backdrop-filter: blur(4px);
  }

  .modal {
    width: min(26rem, 100%);
    max-height: min(90vh, 36rem);
    overflow: auto;
    border-radius: var(--radius-lg);
    border: 1px solid var(--border-soft);
    background: var(--panel-elevated);
    box-shadow: var(--shadow-float);
    padding: 0 1.1rem 1rem;
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
    border-radius: var(--radius-sm);
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

  .shelf-field {
    border: 1px solid var(--border-soft);
    border-radius: 10px;
    padding: 0.5rem 0.65rem 0.6rem;
    margin: 0 0 0.85rem;
  }

  .shelf-field legend {
    padding: 0 0.2rem;
    font-size: 0.78rem;
    color: var(--muted);
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

  .field select,
  .field textarea {
    padding: 0.48rem 0.55rem;
    border-radius: var(--radius-md);
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

  @media (max-width: 900px) {
    .main {
      grid-template-columns: 1fr;
      grid-template-rows: auto 1fr;
    }

    .sidebar {
      border-right: none;
      border-bottom: 1px solid var(--border-soft);
      max-height: min(42vh, 320px);
    }
  }

  @media (max-width: 768px) {
    .top {
      padding: 0.65rem clamp(0.75rem, 4vw, 1.25rem);
    }

    .top-actions {
      flex: 1 1 100%;
      justify-content: stretch;
      margin-left: 0;
      margin-top: 0.35rem;
    }

    .top-actions .btn {
      flex: 1;
      justify-content: center;
      text-align: center;
    }

    .book-grid {
      grid-template-columns: repeat(auto-fill, minmax(min(100%, 8.5rem), 1fr));
      gap: 0.85rem;
    }

    .book-card {
      padding: 0.45rem 0.45rem 0.55rem;
    }
  }

  @media (max-width: 420px) {
    .brand .tag {
      display: none;
    }

    .book-grid {
      gap: 0.75rem;
      grid-template-columns: repeat(2, minmax(0, 1fr));
    }

    .card-title {
      font-size: 0.8rem;
      min-height: 2.8em;
      -webkit-line-clamp: 2;
      line-clamp: 2;
    }
  }
</style>
