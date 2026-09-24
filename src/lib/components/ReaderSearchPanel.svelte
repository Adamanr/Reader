<script lang="ts">
  import type { EpubReaderApi, SearchHit } from "$lib/types";

  interface Props {
    api: EpubReaderApi | null;
    /** Запрос, пришедший извне (например «Найти» у выделения) */
    initialQuery?: string;
    onPicked?: () => void;
  }
  let { api, initialQuery = "", onPicked }: Props = $props();

  let query = $state("");
  let hits = $state<SearchHit[]>([]);
  let busy = $state(false);
  let done = $state(false);
  let activeId = $state<string | null>(null);
  let inputEl = $state<HTMLInputElement | null>(null);
  let ctrl: AbortController | null = null;
  let timer: ReturnType<typeof setTimeout> | null = null;
  let lastInitial = "";

  $effect(() => {
    if (initialQuery && initialQuery !== lastInitial) {
      lastInitial = initialQuery;
      query = initialQuery;
      schedule(0);
    }
  });

  $effect(() => {
    inputEl?.focus();
  });

  function schedule(delay = 350) {
    if (timer) clearTimeout(timer);
    timer = setTimeout(run, delay);
  }

  async function run() {
    ctrl?.abort();
    const q = query.trim();
    if (q.length < 2 || !api?.search) {
      hits = [];
      done = false;
      return;
    }
    const my = new AbortController();
    ctrl = my;
    busy = true;
    done = false;
    try {
      const res = await api.search(q, my.signal);
      if (my.signal.aborted) return;
      hits = res;
      done = true;
    } catch {
      if (!my.signal.aborted) hits = [];
    } finally {
      if (ctrl === my) busy = false;
    }
  }

  async function pick(h: SearchHit) {
    activeId = h.id;
    await api?.goToHit?.(h);
    onPicked?.();
  }

  function onKey(e: KeyboardEvent) {
    if (e.key === "Enter") {
      e.preventDefault();
      if (hits.length) {
        const i = hits.findIndex((h) => h.id === activeId);
        const next = hits[(i + (e.shiftKey ? -1 : 1) + hits.length) % hits.length];
        if (next) void pick(next);
      } else schedule(0);
    }
  }
</script>

<div class="sp">
  <label class="sp-field">
    <svg viewBox="0 0 24 24" aria-hidden="true"><circle cx="11" cy="11" r="6.5" /><path d="m16 16 4 4" /></svg>
    <input
      bind:this={inputEl}
      type="search"
      placeholder="Слово или фраза…"
      bind:value={query}
      oninput={() => schedule()}
      onkeydown={onKey}
    />
  </label>
  <p class="sp-status">
    {#if busy}
      Ищу по книге…
    {:else if done}
      {hits.length === 0 ? "Ничего не нашлось" : `Найдено: ${hits.length}${hits.length >= 300 ? "+" : ""} · Enter — следующее`}
    {:else}
      Регистр и ё/е не важны
    {/if}
  </p>
  <ul class="sp-list">
    {#each hits as h (h.id)}
      <li>
        <button type="button" class="sp-hit" class:active={activeId === h.id} onclick={() => void pick(h)}>
          <span class="sp-label">{h.label}</span>
          <span class="sp-ex">{h.before}<mark>{h.match}</mark>{h.after}</span>
        </button>
      </li>
    {/each}
  </ul>
</div>

<style>
  .sp {
    display: flex;
    flex-direction: column;
    gap: 0.5rem;
    min-height: 0;
  }

  .sp-field {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    padding: 0.45rem 0.7rem;
    border-radius: 999px;
    border: 1px solid var(--border-soft);
    background: var(--elevated-soft);
  }

  .sp-field:focus-within {
    border-color: var(--accent-2);
    box-shadow: var(--focus-ring);
  }

  .sp-field svg {
    width: 1rem;
    height: 1rem;
    fill: none;
    stroke: var(--muted);
    stroke-width: 1.8;
    stroke-linecap: round;
  }

  .sp-field input {
    flex: 1;
    min-width: 0;
    border: none;
    outline: none;
    background: transparent;
    color: var(--text-soft);
    font: inherit;
  }

  .sp-status {
    margin: 0;
    font-size: 0.72rem;
    color: var(--muted);
  }

  .sp-list {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 0.3rem;
  }

  .sp-hit {
    width: 100%;
    display: flex;
    flex-direction: column;
    gap: 0.2rem;
    text-align: left;
    border: 1px solid transparent;
    background: transparent;
    color: var(--text-soft);
    border-radius: var(--radius-sm);
    padding: 0.45rem 0.55rem;
    cursor: pointer;
    font: inherit;
  }

  .sp-hit:hover,
  .sp-hit.active {
    background: var(--elevated-soft);
    border-color: var(--border-soft);
  }

  .sp-label {
    font-size: 0.68rem;
    color: var(--muted);
  }

  .sp-ex {
    font-size: 0.84rem;
    line-height: 1.45;
    font-family: "Literata Variable", Georgia, serif;
  }

  .sp-ex mark {
    background: color-mix(in srgb, var(--accent) 38%, transparent);
    color: inherit;
    border-radius: 2px;
    padding: 0 1px;
  }
</style>
