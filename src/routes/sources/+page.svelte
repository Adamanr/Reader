<script lang="ts">
  import { onMount } from "svelte";
  import { goto } from "$app/navigation";
  import OpdsCatalog from "$lib/components/sources/OpdsCatalog.svelte";
  import TelegramCatalog from "$lib/components/sources/TelegramCatalog.svelte";
  import { isTauriRuntime } from "$lib/isTauri";
  import { fetchLibrarySnapshot } from "$lib/library/librarySnapshotCache";
  import { loadBookSources } from "$lib/sources/store";
  import { sourceCountNew } from "$lib/sources/telegram";
  import type { BookSource } from "$lib/sources/types";
  import { toastError } from "$lib/ui/toast.svelte";

  let sources = $state<BookSource[]>([]);
  let selectedId = $state("");
  let libraryRoot = $state<string | null>(null);
  let loaded = $state(false);
  /** Новые книги в Telegram-каналах с прошлого просмотра каталога. */
  let newCounts = $state<Record<string, number>>({});

  const selectedSource = $derived(sources.find((s) => s.id === selectedId) ?? null);

  onMount(() => {
    if (!isTauriRuntime()) {
      loaded = true;
      return;
    }
    void Promise.all([loadBookSources(), fetchLibrarySnapshot().catch(() => null)])
      .then(([state, snap]) => {
        sources = state.sources;
        libraryRoot = snap?.libraryRoot ?? null;
        if (sources.length && !selectedId) selectedId = sources[0].id;
        loaded = true;
        void countNew(state.sources);
      })
      .catch((e) => {
        loaded = true;
        toastError(e, "Источники");
      });
  });

  /** По одному поисковому запросу на канал; ошибки (нет входа, сеть) просто не показываем. */
  async function countNew(list: BookSource[]) {
    for (const s of list) {
      if (s.kind !== "telegram" || !s.lastSeenAt) continue;
      try {
        const n = await sourceCountNew(s.username, s.lastSeenAt);
        if (n > 0) newCounts = { ...newCounts, [s.id]: n };
      } catch {
        return;
      }
    }
  }

  function optionLabel(s: BookSource): string {
    const kind = s.kind === "telegram" ? `@${s.username}` : "OPDS";
    const n = newCounts[s.id];
    const fresh = n ? ` — новых: ${n >= 100 ? "99+" : n}` : "";
    return `${s.label} (${kind})${fresh}`;
  }

  function markSeen(id: string) {
    const { [id]: _, ...rest } = newCounts;
    newCounts = rest;
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
        <h1 class="title">Источники</h1>
        <p>Книги из подключённых Telegram-каналов и OPDS-каталогов.</p>
      </div>
    </div>
  </header>

  <main class="main">
    {#if !isTauriRuntime()}
      <section class="card">
        <p class="muted">Доступно в приложении.</p>
      </section>
    {:else if !loaded}
      <section class="card">
        <p class="muted">Загрузка…</p>
      </section>
    {:else if sources.length === 0}
      <section class="card">
        <p class="muted">
          Нет источников.
          <a href="/settings#book-sources">Добавьте канал или каталог в настройках</a>.
        </p>
      </section>
    {:else}
      <section class="card">
        <label class="field">
          <span>Источник</span>
          <select bind:value={selectedId}>
            {#each sources as s (s.id)}
              <option value={s.id}>{optionLabel(s)}</option>
            {/each}
          </select>
        </label>

        {#if !libraryRoot}
          <p class="warn">Сначала выберите папку библиотеки — без неё импорт недоступен.</p>
        {/if}

        {#if selectedSource}
          {#key selectedSource.id}
            {#if selectedSource.kind === "telegram"}
              <TelegramCatalog
                source={selectedSource}
                {libraryRoot}
                onSeen={() => markSeen(selectedSource.id)}
              />
            {:else}
              <OpdsCatalog source={selectedSource} {libraryRoot} />
            {/if}
          {/key}
        {/if}
      </section>
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
      radial-gradient(circle at 86% 10%, color-mix(in srgb, #fff2bf 58%, transparent) 0 4.2rem, transparent 4.35rem),
      radial-gradient(ellipse 34rem 20rem at 10% 0%, color-mix(in srgb, var(--accent) 24%, transparent), transparent 68%),
      radial-gradient(ellipse 32rem 20rem at 100% 54%, color-mix(in srgb, #bcd8ee 20%, transparent), transparent 70%);
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
    margin: 0;
    border: 1px solid color-mix(in srgb, var(--accent) 18%, var(--border-soft));
    border-radius: 999px;
    background: color-mix(in srgb, var(--elevated-soft) 54%, transparent);
    color: var(--accent-2);
    font-size: 0.92rem;
    font-weight: 650;
    cursor: pointer;
    font-family: system-ui, sans-serif;
  }

  .back:hover {
    background: var(--elevated-soft);
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
    color: var(--text-soft);
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
    padding: clamp(1.25rem, 4vw, 2.25rem);
    max-width: 64rem;
    width: 100%;
    margin-inline: auto;
    box-sizing: border-box;
  }

  .card {
    display: flex;
    flex-direction: column;
    gap: 0.85rem;
    padding: 1.2rem 1.3rem;
    border-radius: 1.45rem;
    background: color-mix(in srgb, var(--panel-elevated) 92%, transparent);
    border: 1px solid color-mix(in srgb, var(--accent) 17%, var(--border-soft));
    box-shadow: 0 18px 60px color-mix(in srgb, var(--accent-2) 8%, transparent);
    color: var(--text-soft);
  }

  .main :global(.muted) {
    margin: 0;
    font-size: 0.86rem;
    line-height: 1.45;
    color: var(--muted);
  }

  .main :global(.muted a) {
    color: var(--accent-2);
  }

  .main :global(.warn) {
    margin: 0;
    font-size: 0.86rem;
    line-height: 1.45;
    color: color-mix(in srgb, var(--text-soft) 40%, #b86);
  }

  .main :global(.toolbar) {
    display: flex;
    gap: 0.6rem;
    align-items: flex-end;
    flex-wrap: wrap;
  }

  .main :global(.field) {
    display: flex;
    flex-direction: column;
    gap: 0.3rem;
    font-size: 0.8rem;
    color: var(--muted);
  }

  .main :global(.field select),
  .main :global(.field input) {
    border: 1px solid var(--border-soft);
    background: var(--elevated-soft);
    color: var(--text-soft);
    border-radius: var(--radius-sm);
    padding: 0.5rem 0.65rem;
    font: inherit;
    font-size: 0.9rem;
    min-width: 12rem;
  }

  .main :global(.grow) {
    flex: 1;
    min-width: 12rem;
  }

  .main :global(.btn),
  .main :global(.chip) {
    border: 1px solid var(--border-soft);
    background: var(--elevated-soft);
    color: var(--text-soft);
    border-radius: 999px;
    padding: 0.4rem 0.85rem;
    font-size: 0.82rem;
    cursor: pointer;
    align-self: flex-start;
  }

  .main :global(.btn:disabled),
  .main :global(.chip:disabled) {
    opacity: 0.5;
    cursor: default;
  }

  .main :global(.btn.primary) {
    background: color-mix(in srgb, var(--accent) 28%, var(--elevated-soft));
    border-color: color-mix(in srgb, var(--accent-2) 35%, var(--border-soft));
    font-weight: 600;
  }

  .main :global(.import-row) {
    display: flex;
    flex-wrap: wrap;
    gap: 0.6rem;
    align-items: center;
  }

  .main :global(.sel-row) {
    display: flex;
    flex-wrap: wrap;
    gap: 0.45rem;
    align-items: center;
  }

  .main :global(.table-wrap) {
    overflow-x: auto;
    border-radius: var(--radius-sm);
    border: 1px solid var(--border-soft);
  }

  .main :global(table) {
    width: 100%;
    border-collapse: collapse;
    font-size: 0.84rem;
  }

  .main :global(th),
  .main :global(td) {
    padding: 0.5rem 0.55rem;
    text-align: left;
    border-bottom: 1px solid color-mix(in srgb, var(--border-soft) 70%, transparent);
    vertical-align: top;
  }

  .main :global(th) {
    font-size: 0.72rem;
    text-transform: uppercase;
    letter-spacing: 0.06em;
    color: var(--muted);
    font-weight: 650;
    background: color-mix(in srgb, var(--panel-soft) 70%, transparent);
  }

  .main :global(tr:last-child td) {
    border-bottom: none;
  }

  .main :global(.check) {
    width: 2rem;
  }

  .main :global(.check input) {
    width: 1.05rem;
    height: 1.05rem;
    accent-color: var(--accent-2);
  }

  .main :global(.name) {
    font-weight: 550;
    max-width: 14rem;
    word-break: break-word;
  }

  .main :global(tr.done td) {
    color: var(--muted);
  }

  .main :global(.badge.new) {
    background: color-mix(in srgb, #4caf50 22%, transparent);
    color: color-mix(in srgb, #2e7d32 80%, var(--text-soft));
  }

  .main :global(.badge) {
    display: inline-block;
    margin-left: 0.4rem;
    padding: 0.05rem 0.45rem;
    border-radius: 999px;
    font-size: 0.7rem;
    font-weight: 600;
    background: color-mix(in srgb, var(--accent) 18%, transparent);
    color: var(--accent-2);
  }

  .main :global(.ext) {
    text-transform: uppercase;
    font-size: 0.78rem;
    color: var(--muted);
  }

  .main :global(.cap) {
    max-width: 12rem;
    color: var(--muted);
    overflow: hidden;
    display: -webkit-box;
    -webkit-line-clamp: 2;
    line-clamp: 2;
    -webkit-box-orient: vertical;
  }

  @media (max-width: 700px) {
    .top {
      padding:
        max(1rem, env(safe-area-inset-top))
        max(1rem, env(safe-area-inset-right))
        1.25rem
        max(1rem, env(safe-area-inset-left));
    }

    .main {
      padding:
        1rem
        max(1rem, env(safe-area-inset-right))
        max(1.5rem, env(safe-area-inset-bottom))
        max(1rem, env(safe-area-inset-left));
    }

    .card {
      padding: 1rem;
      border-radius: 1.25rem;
    }
  }
</style>
