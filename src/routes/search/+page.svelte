<script lang="ts">
  import { onDestroy, onMount } from "svelte";
  import { afterNavigate, goto } from "$app/navigation";
  import { isTauriRuntime } from "$lib/isTauri";
  import {
    onIndexProgress,
    searchQuery,
    searchReindex,
    searchStatus,
    type IndexProgress,
    type SearchHit,
    type SearchStatus,
  } from "$lib/search";
  import { toastError } from "$lib/ui/toast.svelte";

  let query = $state("");
  let hits = $state<SearchHit[]>([]);
  let busy = $state(false);
  let searched = $state("");
  let status = $state<SearchStatus | null>(null);
  let progress = $state<IndexProgress | null>(null);
  let inputEl = $state<HTMLInputElement | null>(null);
  let timer: ReturnType<typeof setTimeout> | null = null;
  let seq = 0;
  let unlisten: (() => void) | null = null;

  interface Group {
    path: string;
    title: string;
    hits: SearchHit[];
  }

  /** Результаты по книгам, в порядке лучшего совпадения в каждой. */
  const groups = $derived.by(() => {
    const map = new Map<string, Group>();
    for (const h of hits) {
      let g = map.get(h.path);
      if (!g) {
        g = { path: h.path, title: h.title, hits: [] };
        map.set(h.path, g);
      }
      g.hits.push(h);
    }
    return [...map.values()];
  });

  const indexing = $derived(!!status?.running || (!!progress && !progress.finished));

  onMount(() => {
    if (!isTauriRuntime()) return;
    void onIndexProgress((p) => {
      progress = p;
      if (p.finished) {
        if (p.error) toastError(p.error, "Индексация");
        void refreshStatus();
        if (searched) void run(searched);
      }
    }).then((fn) => (unlisten = fn));
    // Индекс обновляется инкрементально: если ничего не менялось, это быстро.
    void refreshStatus().then(() => void searchReindex().then(() => refreshStatus()));
  });

  // SvelteKit после навигации переводит фокус на body — ставим курсор в поле уже после неё.
  afterNavigate(() => inputEl?.focus());

  onDestroy(() => {
    unlisten?.();
    if (timer) clearTimeout(timer);
  });

  async function refreshStatus() {
    try {
      status = await searchStatus();
    } catch {
      status = null;
    }
  }

  function schedule() {
    if (timer) clearTimeout(timer);
    timer = setTimeout(() => void run(query), 300);
  }

  async function run(q: string) {
    const text = q.trim();
    const my = ++seq;
    if (!text) {
      hits = [];
      searched = "";
      return;
    }
    busy = true;
    try {
      const res = await searchQuery(text);
      if (my !== seq) return;
      hits = res;
      searched = text;
    } catch (e) {
      if (my === seq) toastError(e, "Поиск");
    } finally {
      if (my === seq) busy = false;
    }
  }

  function open(h: SearchHit) {
    const q = searched || query.trim();
    void goto(`/read?path=${encodeURIComponent(h.path)}&q=${encodeURIComponent(q)}`);
  }

  async function reindex() {
    try {
      progress = { done: 0, total: 0, current: "", finished: false, error: null };
      const started = await searchReindex();
      if (!started) await refreshStatus();
    } catch (e) {
      progress = null;
      toastError(e, "Индексация");
    }
  }
</script>

<div class="shell">
  <div class="ambient" aria-hidden="true"></div>
  <header class="top">
    <div class="top-inner">
      <button type="button" class="back" onclick={() => goto("/")}>
        <span class="back-arr" aria-hidden="true">←</span>
        <span>Библиотека</span>
      </button>
      <div class="heading">
        <span class="kicker">Reader</span>
        <h1 class="title">Поиск</h1>
        <p>По тексту всех книг, заметкам и цитатам. Слова находятся в любой форме: «книгами» — это и «книга».</p>
      </div>
    </div>
  </header>

  <main class="main">
    {#if !isTauriRuntime()}
      <section class="card"><p class="muted">Доступно в приложении.</p></section>
    {:else}
      <section class="card">
        <label class="field">
          <span class="sr-only">Что найти</span>
          <input
            bind:this={inputEl}
            type="search"
            bind:value={query}
            placeholder="Слово, фраза в кавычках, автор…"
            oninput={schedule}
            onkeydown={(e) => {
              if (e.key === "Enter") {
                if (timer) clearTimeout(timer);
                void run(query);
              }
            }}
          />
        </label>

        <div class="status" aria-live="polite">
          {#if indexing && progress && progress.total > 0}
            <div class="bar" role="progressbar" aria-valuemin={0} aria-valuemax={progress.total} aria-valuenow={progress.done}>
              <span style:width="{Math.round((progress.done / progress.total) * 100)}%"></span>
            </div>
            <span class="muted">
              Индексация: {progress.done} из {progress.total}{progress.current ? ` — ${progress.current}` : ""}
            </span>
          {:else if indexing}
            <span class="muted">Проверяем, что изменилось в библиотеке…</span>
          {:else if status}
            <span class="muted">Проиндексировано книг: {status.indexedBooks} из {status.libraryBooks}</span>
            <button type="button" class="link-btn" onclick={() => void reindex()}>Обновить индекс</button>
          {/if}
        </div>
      </section>

      {#if searched && !busy && hits.length === 0}
        <section class="card"><p class="muted">Ничего не нашлось по запросу «{searched}».</p></section>
      {/if}

      {#each groups as g (g.path)}
        <section class="card result">
          <h2 class="book">{g.title}</h2>
          <ul>
            {#each g.hits as h, i (i)}
              <li>
                <button type="button" class="hit" onclick={() => open(h)}>
                  <span class="label" class:note={h.kind === "note"}>{h.label}</span>
                  <!-- Сниппет собирает бэкенд: текст книги экранирован, добавлены только <b>. -->
                  <span class="snippet">{@html h.snippet}</span>
                </button>
              </li>
            {/each}
          </ul>
        </section>
      {/each}
    {/if}
  </main>
</div>

<style>
  .shell {
    min-height: 100vh;
    min-height: 100dvh;
    display: flex;
    flex-direction: column;
    background: var(--bg-soft);
    color: var(--text-soft);
    position: relative;
    isolation: isolate;
    overflow-x: hidden;
  }

  .ambient {
    position: fixed;
    inset: 0;
    z-index: -1;
    pointer-events: none;
    background:
      radial-gradient(ellipse 34rem 20rem at 10% 0%, color-mix(in srgb, var(--accent) 22%, transparent), transparent 68%),
      radial-gradient(ellipse 32rem 20rem at 100% 54%, color-mix(in srgb, #bcd8ee 18%, transparent), transparent 70%);
  }

  .top {
    padding: clamp(1rem, 3vw, 1.5rem) clamp(1rem, 5vw, 2rem) clamp(1.4rem, 4vw, 2rem);
    border-bottom: 1px solid color-mix(in srgb, var(--accent) 15%, var(--border-soft));
    background: color-mix(in srgb, var(--panel-veil) 90%, transparent);
  }

  .top-inner {
    width: min(100%, 58rem);
    margin: 0 auto;
  }

  .back {
    display: inline-flex;
    align-items: center;
    gap: 0.35rem;
    min-height: 2.35rem;
    padding: 0.42rem 0.65rem;
    border: 1px solid color-mix(in srgb, var(--accent) 18%, var(--border-soft));
    border-radius: 999px;
    background: color-mix(in srgb, var(--elevated-soft) 54%, transparent);
    color: var(--accent-2);
    font-size: 0.92rem;
    font-weight: 650;
    cursor: pointer;
    font-family: system-ui, sans-serif;
  }

  .back-arr {
    font-size: 1.1rem;
    line-height: 1;
  }

  .heading {
    margin-top: clamp(1rem, 3vw, 1.55rem);
  }

  .kicker {
    color: var(--accent-2);
    font-size: 0.64rem;
    font-weight: 750;
    letter-spacing: 0.18em;
    text-transform: uppercase;
  }

  .title {
    margin: 0.15rem 0 0;
    font-size: clamp(2.15rem, 6vw, 3.15rem);
    font-weight: 500;
    font-family: Georgia, "Times New Roman", serif;
    letter-spacing: -0.045em;
    line-height: 1.08;
  }

  .heading p {
    margin: 0.55rem 0 0;
    color: var(--muted);
    font-size: 0.95rem;
    line-height: 1.5;
  }

  .main {
    flex: 1;
    display: flex;
    flex-direction: column;
    gap: 0.9rem;
    padding: clamp(1.25rem, 4vw, 2.25rem);
    max-width: 58rem;
    width: 100%;
    margin-inline: auto;
    box-sizing: border-box;
  }

  .card {
    display: flex;
    flex-direction: column;
    gap: 0.7rem;
    padding: 1.1rem 1.25rem;
    border-radius: 1.3rem;
    background: color-mix(in srgb, var(--panel-elevated) 92%, transparent);
    border: 1px solid color-mix(in srgb, var(--accent) 17%, var(--border-soft));
    box-shadow: 0 18px 60px color-mix(in srgb, var(--accent-2) 8%, transparent);
  }

  .field input {
    width: 100%;
    box-sizing: border-box;
    border: 1px solid var(--border-soft);
    background: var(--elevated-soft);
    color: var(--text-soft);
    border-radius: 0.8rem;
    padding: 0.75rem 0.95rem;
    font: inherit;
    font-size: 1.05rem;
  }

  .sr-only {
    position: absolute;
    width: 1px;
    height: 1px;
    overflow: hidden;
    clip: rect(0 0 0 0);
  }

  .status {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 0.4rem 0.8rem;
  }

  .bar {
    flex-basis: 100%;
    height: 0.3rem;
    border-radius: 999px;
    background: color-mix(in srgb, var(--accent) 14%, transparent);
    overflow: hidden;
  }

  .bar span {
    display: block;
    height: 100%;
    background: var(--accent-2);
    transition: width 0.3s ease;
  }

  .muted {
    margin: 0;
    font-size: 0.84rem;
    color: var(--muted);
  }

  .link-btn {
    border: none;
    background: none;
    padding: 0;
    color: var(--accent-2);
    font: inherit;
    font-size: 0.84rem;
    cursor: pointer;
  }

  .book {
    margin: 0;
    font-size: 1rem;
    font-weight: 650;
  }

  .result ul {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 0.35rem;
  }

  .hit {
    width: 100%;
    display: flex;
    flex-direction: column;
    gap: 0.2rem;
    text-align: left;
    padding: 0.55rem 0.7rem;
    border: 1px solid transparent;
    border-radius: 0.7rem;
    background: color-mix(in srgb, var(--panel-soft) 60%, transparent);
    color: var(--text-soft);
    font: inherit;
    cursor: pointer;
  }

  .hit:hover,
  .hit:focus-visible {
    border-color: color-mix(in srgb, var(--accent-2) 40%, var(--border-soft));
  }

  .label {
    font-size: 0.72rem;
    font-weight: 650;
    letter-spacing: 0.04em;
    text-transform: uppercase;
    color: var(--muted);
  }

  .label.note {
    color: var(--accent-2);
  }

  .snippet {
    font-size: 0.9rem;
    line-height: 1.5;
  }

  .snippet :global(b) {
    font-weight: 650;
    background: color-mix(in srgb, var(--accent) 26%, transparent);
    border-radius: 0.2rem;
    padding: 0 0.1rem;
  }

  @media (max-width: 700px) {
    .main {
      padding: 1rem max(1rem, env(safe-area-inset-right)) max(1.5rem, env(safe-area-inset-bottom))
        max(1rem, env(safe-area-inset-left));
    }
  }
</style>
