<script lang="ts">
  import { onDestroy, onMount } from "svelte";
  import type { EpubReaderApi } from "$lib/types";
  import { assistantReady } from "$lib/ai/assistant.svelte";
  import { recapBook } from "$lib/ai/recap";

  interface Props {
    api: EpubReaderApi;
    bookPath: string;
    bookTitle: string;
    onClose: () => void;
  }
  let { api, bookPath, bookTitle, onClose }: Props = $props();

  let stage = $state("Собираю прочитанное…");
  let text = $state("");
  let lastLines = $state("");
  let error = $state<string | null>(null);
  let busy = $state(true);
  const ctrl = new AbortController();

  async function run() {
    busy = true;
    error = null;
    text = "";
    try {
      const chunks = (await api.chunksBefore?.()) ?? [];
      const current = chunks.find((c) => c.partial) ?? chunks[chunks.length - 1];
      // Даже без модели полезно увидеть последние прочитанные строки.
      lastLines = current ? "…" + current.text.slice(-900).replace(/^\S*\s/, "") : "";
      if (!assistantReady()) return;
      text = await recapBook({
        path: bookPath,
        title: bookTitle,
        chunks,
        signal: ctrl.signal,
        onStage: (s) => (stage = s),
      });
    } catch (e) {
      if ((e as Error)?.name !== "AbortError") error = e instanceof Error ? e.message : String(e);
    } finally {
      busy = false;
    }
  }

  onMount(() => {
    void run();
  });

  onDestroy(() => ctrl.abort());
</script>

<!-- svelte-ignore a11y_click_events_have_key_events -->
<!-- svelte-ignore a11y_no_static_element_interactions -->
<div class="rd-back" onclick={onClose}>
  <div class="rd" role="dialog" aria-modal="true" aria-label="Ранее в книге" tabindex="-1" onclick={(e) => e.stopPropagation()}>
    <header>
      <p class="kicker">без спойлеров</p>
      <h2>Ранее в книге…</h2>
      <button type="button" class="x" aria-label="Закрыть" onclick={onClose}>×</button>
    </header>

    {#if assistantReady()}
      {#if busy}
        <div class="stage"><span class="orb" aria-hidden="true"></span>{stage}</div>
        <p class="muted">Первый раз это займёт время: помощник прочитает главы и запомнит конспекты. Дальше — быстрее.</p>
      {:else if error}
        <p class="err">{error}</p>
        <button type="button" class="btn" onclick={() => void run()}>Попробовать снова</button>
      {:else}
        <div class="text">{text}</div>
      {/if}
    {:else}
      <p class="muted">
        Чтобы получить пересказ, подключите локальную модель в <a href="/settings">Настройках → Помощник чтения</a>.
        Пока — последние строки, на которых вы остановились:
      </p>
    {/if}

    {#if lastLines && (!assistantReady() || !busy)}
      <details open={!assistantReady()}>
        <summary>Последние прочитанные строки</summary>
        <p class="last">{lastLines}</p>
      </details>
    {/if}

    <footer>
      <button type="button" class="btn primary" onclick={onClose}>Продолжить чтение</button>
    </footer>
  </div>
</div>

<style>
  .rd-back {
    position: fixed;
    inset: 0;
    z-index: 800;
    display: grid;
    place-items: center;
    padding: 1rem;
    background: rgba(20, 16, 30, 0.35);
    backdrop-filter: blur(4px);
    animation: in 0.2s ease-out;
  }

  .rd {
    width: min(38rem, 100%);
    max-height: 86vh;
    overflow: auto;
    display: flex;
    flex-direction: column;
    gap: 0.9rem;
    padding: 1.4rem 1.5rem;
    border-radius: var(--radius-xl);
    background: var(--panel-elevated);
    border: 1px solid var(--toolbar-border);
    box-shadow: var(--shadow-float);
    color: var(--text-soft);
  }

  header {
    position: relative;
  }

  .kicker {
    margin: 0;
    font-size: 0.68rem;
    text-transform: uppercase;
    letter-spacing: 0.16em;
    color: var(--muted);
  }

  h2 {
    margin: 0.2rem 0 0;
    font-family: "Literata Variable", Georgia, serif;
    font-weight: 500;
    font-size: 1.6rem;
  }

  .x {
    position: absolute;
    top: -0.3rem;
    right: -0.3rem;
    border: none;
    background: transparent;
    color: var(--muted);
    font-size: 1.5rem;
    cursor: pointer;
  }

  .stage {
    display: flex;
    align-items: center;
    gap: 0.7rem;
    font-size: 0.95rem;
  }

  .orb {
    width: 0.9rem;
    height: 0.9rem;
    border-radius: 50%;
    background: radial-gradient(circle at 35% 35%, var(--accent), var(--accent-2));
    animation: pulse 1.4s ease-in-out infinite;
  }

  @keyframes pulse {
    50% {
      transform: scale(1.35);
      opacity: 0.6;
    }
  }

  .text {
    font-family: "Literata Variable", Georgia, serif;
    font-size: 1.02rem;
    line-height: 1.7;
    white-space: pre-wrap;
  }

  .muted {
    margin: 0;
    font-size: 0.86rem;
    line-height: 1.5;
    color: var(--muted);
  }

  .muted a {
    color: var(--accent-2);
  }

  .err {
    margin: 0;
    color: var(--danger);
    font-size: 0.88rem;
  }

  details summary {
    cursor: pointer;
    font-size: 0.8rem;
    color: var(--muted);
  }

  .last {
    margin: 0.5rem 0 0;
    padding-left: 0.7rem;
    border-left: 3px solid var(--accent);
    font-family: "Literata Variable", Georgia, serif;
    font-size: 0.92rem;
    line-height: 1.6;
    color: var(--muted);
  }

  footer {
    display: flex;
    justify-content: flex-end;
  }

  .btn {
    border: 1px solid var(--border-soft);
    background: var(--elevated-soft);
    color: var(--text-soft);
    border-radius: 999px;
    padding: 0.5rem 1rem;
    cursor: pointer;
  }

  .btn.primary {
    background: var(--accent-2);
    border-color: var(--accent-2);
    color: var(--elevated-soft);
  }

  @keyframes in {
    from {
      opacity: 0;
    }
  }
</style>
