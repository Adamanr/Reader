<script lang="ts">
  import { onDestroy, onMount, untrack } from "svelte";
  import { goto } from "$app/navigation";
  import { fetchLibrarySnapshot } from "$lib/library/librarySnapshotCache";
  import { onOpdsProgress, opdsFetch, opdsImport, opdsSearchUrl } from "$lib/sources/opds";
  import { touchBookSourceSeen, touchBookSourceSynced } from "$lib/sources/store";
  import type { BookSource, ImportProgress, OpdsEntry, OpdsFeed } from "$lib/sources/types";
  import { toast, toastError } from "$lib/ui/toast.svelte";
  import { formatSize, pluralBooks } from "./format";

  interface Props {
    source: BookSource;
    libraryRoot: string | null;
  }

  let { source, libraryRoot }: Props = $props();

  interface Crumb {
    url: string;
    title: string;
  }

  const rootUrl = untrack(() => source.url ?? "");
  let stack = $state<Crumb[]>([{ url: rootUrl, title: untrack(() => source.label) }]);
  let feed = $state<OpdsFeed | null>(null);
  let entries = $state<OpdsEntry[]>([]);
  /** Шаблон поиска запоминаем с корня: во вложенных разделах его часто нет. */
  let searchTemplate = $state<string | null>(null);
  let query = $state("");
  let busy = $state(false);
  let error = $state<string | null>(null);
  /** Выбранные книги и формат для каждой. */
  let picked = $state<Record<string, string>>({});
  let importBusy = $state(false);
  let progress = $state<ImportProgress | null>(null);
  let seq = 0;
  let unlisten: (() => void) | null = null;

  const books = $derived(entries.filter((e) => e.acquisitions.length > 0));
  const sections = $derived(entries.filter((e) => e.acquisitions.length === 0 && e.navigation));
  const pickedCount = $derived(Object.keys(picked).length);
  const canImport = $derived(!!libraryRoot && pickedCount > 0 && !importBusy);

  onMount(() => {
    void onOpdsProgress((p) => {
      if (importBusy) progress = p;
    }).then((fn) => (unlisten = fn));
    void open(rootUrl, null);
    void touchBookSourceSeen(untrack(() => source.id));
  });

  onDestroy(() => unlisten?.());

  function keyOf(e: OpdsEntry): string {
    return e.id || `${e.title}|${e.authors.join(",")}`;
  }

  /** `crumb === null` — заменить текущий уровень (обновление, корень), иначе — шаг вглубь. */
  async function open(url: string, crumb: Crumb | null, append = false) {
    const my = ++seq;
    busy = true;
    error = null;
    try {
      const f = await opdsFetch(url);
      if (my !== seq) return;
      if (append) {
        const seen = new Set(entries.map(keyOf));
        entries = [...entries, ...f.entries.filter((e) => !seen.has(keyOf(e)))];
      } else {
        entries = f.entries;
        picked = {};
        if (crumb) stack = [...stack, crumb];
      }
      feed = f;
      if (f.searchTemplate && !searchTemplate) searchTemplate = f.searchTemplate;
      if (!f.entries.length && !append) error = "В этом разделе пусто";
    } catch (e) {
      if (my !== seq) return;
      error = e instanceof Error ? e.message : String(e);
    } finally {
      if (my === seq) busy = false;
    }
  }

  function goTo(index: number) {
    const target = stack[index];
    stack = stack.slice(0, index + 1);
    void open(target.url, null);
  }

  function search() {
    const q = query.trim();
    if (!q || !searchTemplate) return;
    stack = stack.slice(0, 1);
    void open(opdsSearchUrl(searchTemplate, q), { url: opdsSearchUrl(searchTemplate, q), title: `Поиск: ${q}` });
  }

  function toggle(e: OpdsEntry, on: boolean) {
    const k = keyOf(e);
    const next = { ...picked };
    if (on) next[k] = picked[k] ?? e.acquisitions[0].url;
    else delete next[k];
    picked = next;
  }

  function chooseFormat(e: OpdsEntry, url: string) {
    picked = { ...picked, [keyOf(e)]: url };
  }

  async function importPicked() {
    if (!canImport) return;
    const urls = Object.values(picked);
    importBusy = true;
    progress = { done: 0, total: urls.length, name: "" };
    try {
      const result = await opdsImport(urls);
      await touchBookSourceSynced(source.id);
      await fetchLibrarySnapshot().catch(() => null);
      picked = {};
      const parts: string[] = [];
      if (result.added.length) parts.push(`добавлено: ${pluralBooks(result.added.length)}`);
      if (result.existing.length) parts.push(`уже были: ${result.existing.length}`);
      if (parts.length) {
        const text = parts.join(", ");
        toast(text[0].toUpperCase() + text.slice(1), "success", {
          action: { label: "В библиотеку", run: () => void goto("/") },
        });
      }
      for (const f of result.failed) toast(`«${f.title}»: ${f.error}`, "error");
    } catch (e) {
      toastError(e, "Импорт");
    } finally {
      importBusy = false;
      progress = null;
    }
  }
</script>

<nav class="crumbs" aria-label="Разделы каталога">
  {#each stack as c, i (i)}
    {#if i > 0}<span class="sep" aria-hidden="true">›</span>{/if}
    <button type="button" class="crumb" disabled={busy || i === stack.length - 1} onclick={() => goTo(i)}>
      {c.title || "Каталог"}
    </button>
  {/each}
</nav>

{#if searchTemplate}
  <div class="toolbar">
    <label class="field grow">
      <span>Поиск по каталогу</span>
      <input
        type="search"
        bind:value={query}
        placeholder="название или автор"
        onkeydown={(e) => {
          if (e.key === "Enter") search();
        }}
      />
    </label>
    <button type="button" class="btn" disabled={busy || !query.trim()} onclick={search}>Найти</button>
  </div>
{/if}

{#if error}
  <p class="warn" role="alert">{error}</p>
{/if}

{#if busy && !entries.length}
  <p class="muted">Загрузка каталога…</p>
{/if}

{#if sections.length}
  <ul class="sections">
    {#each sections as s (keyOf(s))}
      <li>
        <button
          type="button"
          class="section"
          disabled={busy}
          onclick={() => void open(s.navigation!, { url: s.navigation!, title: s.title })}
        >
          <span class="section-title">{s.title}</span>
          {#if s.summary}<span class="section-sub">{s.summary}</span>{/if}
          <span class="section-arrow" aria-hidden="true">›</span>
        </button>
      </li>
    {/each}
  </ul>
{/if}

{#if books.length}
  <ul class="books">
    {#each books as b (keyOf(b))}
      {@const k = keyOf(b)}
      <li class:on={!!picked[k]}>
        <input
          type="checkbox"
          class="pick"
          aria-label="Выбрать {b.title}"
          checked={!!picked[k]}
          disabled={importBusy}
          onchange={(e) => toggle(b, e.currentTarget.checked)}
        />
        {#if b.cover}
          <img class="cover" src={b.cover} alt="" loading="lazy" referrerpolicy="no-referrer" />
        {:else}
          <div class="cover placeholder" aria-hidden="true"></div>
        {/if}
        <div class="info">
          <span class="book-title">{b.title}</span>
          {#if b.authors.length}<span class="authors">{b.authors.join(", ")}</span>{/if}
          {#if b.summary}<span class="summary">{b.summary}</span>{/if}
        </div>
        <div class="formats">
          {#if b.acquisitions.length > 1}
            <select
              aria-label="Формат"
              value={picked[k] ?? b.acquisitions[0].url}
              onchange={(e) => chooseFormat(b, e.currentTarget.value)}
            >
              {#each b.acquisitions as a (a.url)}
                <option value={a.url}>{a.ext.toUpperCase()}{a.size ? ` · ${formatSize(a.size)}` : ""}</option>
              {/each}
            </select>
          {:else}
            <span class="ext">
              {b.acquisitions[0].ext}{b.acquisitions[0].size ? ` · ${formatSize(b.acquisitions[0].size)}` : ""}
            </span>
          {/if}
        </div>
      </li>
    {/each}
  </ul>
{:else if !busy && !error && entries.length && !sections.length}
  <p class="muted">Здесь нет книг в форматах PDF, EPUB или FB2.</p>
{/if}

{#if feed?.next}
  <button type="button" class="chip" disabled={busy} onclick={() => void open(feed!.next!, null, true)}>
    {busy ? "Загрузка…" : "Загрузить ещё"}
  </button>
{/if}

{#if books.length}
  <div class="import-row">
    <button type="button" class="btn primary" disabled={!canImport} onclick={() => void importPicked()}>
      {importBusy ? "Импорт…" : pickedCount ? `Добавить в библиотеку (${pickedCount})` : "Добавить в библиотеку"}
    </button>
    {#if progress}
      <span class="muted" aria-live="polite">
        {progress.done} из {progress.total}{progress.name ? ` — ${progress.name}` : ""}
      </span>
    {/if}
  </div>
{/if}

<style>
  .crumbs {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 0.25rem;
    font-size: 0.84rem;
  }

  .crumb {
    border: none;
    background: none;
    padding: 0.15rem 0.3rem;
    border-radius: 0.4rem;
    color: var(--accent-2);
    font: inherit;
    cursor: pointer;
  }

  .crumb:disabled {
    color: var(--text-soft);
    font-weight: 600;
    cursor: default;
  }

  .sep {
    color: var(--muted);
  }

  .sections,
  .books {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 0.4rem;
  }

  .section {
    width: 100%;
    display: grid;
    grid-template-columns: 1fr auto;
    gap: 0.1rem 0.6rem;
    text-align: left;
    padding: 0.6rem 0.75rem;
    border-radius: var(--radius-sm);
    border: 1px solid var(--border-soft);
    background: var(--elevated-soft);
    color: var(--text-soft);
    font: inherit;
    cursor: pointer;
  }

  .section:hover:not(:disabled) {
    border-color: color-mix(in srgb, var(--accent-2) 40%, var(--border-soft));
  }

  .section-title {
    font-weight: 600;
    font-size: 0.9rem;
  }

  .section-sub {
    grid-column: 1;
    font-size: 0.78rem;
    color: var(--muted);
    overflow: hidden;
    display: -webkit-box;
    -webkit-line-clamp: 1;
    line-clamp: 1;
    -webkit-box-orient: vertical;
  }

  .section-arrow {
    grid-column: 2;
    grid-row: 1 / span 2;
    align-self: center;
    color: var(--muted);
    font-size: 1.2rem;
  }

  .books li {
    display: grid;
    grid-template-columns: auto auto 1fr auto;
    gap: 0.75rem;
    align-items: start;
    padding: 0.6rem 0.75rem;
    border-radius: var(--radius-sm);
    border: 1px solid var(--border-soft);
    background: var(--elevated-soft);
  }

  .books li.on {
    border-color: color-mix(in srgb, var(--accent-2) 45%, var(--border-soft));
  }

  .pick {
    margin-top: 0.2rem;
    width: 1.05rem;
    height: 1.05rem;
    accent-color: var(--accent-2);
  }

  .cover {
    width: 3rem;
    height: 4.3rem;
    object-fit: cover;
    border-radius: 0.3rem;
    background: color-mix(in srgb, var(--accent) 12%, var(--panel-soft));
  }

  .info {
    display: flex;
    flex-direction: column;
    gap: 0.15rem;
    min-width: 0;
  }

  .book-title {
    font-weight: 600;
    font-size: 0.9rem;
  }

  .authors {
    font-size: 0.8rem;
    color: var(--muted);
  }

  .summary {
    font-size: 0.78rem;
    color: var(--muted);
    overflow: hidden;
    display: -webkit-box;
    -webkit-line-clamp: 2;
    line-clamp: 2;
    -webkit-box-orient: vertical;
  }

  .formats select {
    border: 1px solid var(--border-soft);
    background: var(--panel-elevated);
    color: var(--text-soft);
    border-radius: var(--radius-sm);
    padding: 0.3rem 0.4rem;
    font: inherit;
    font-size: 0.78rem;
  }

  @media (max-width: 560px) {
    .books li {
      grid-template-columns: auto auto 1fr;
    }

    .formats {
      grid-column: 3;
    }
  }
</style>
