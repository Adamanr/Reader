<script lang="ts">
  import { pauseReading, resumeReading, saveTts, skipParagraph, stopReading, tts } from "$lib/reading/tts.svelte";
</script>

{#if tts.active || tts.error}
  <div class="tb" role="region" aria-label="Чтение вслух">
    {#if tts.error}
      <span class="err">{tts.error}</span>
      <button type="button" class="b" onclick={() => (tts.error = null)} aria-label="Закрыть">×</button>
    {:else}
      <span class="wave" class:still={tts.paused || tts.loading} aria-hidden="true"><i></i><i></i><i></i><i></i></span>
      <button type="button" class="b" title="Предыдущий абзац" aria-label="Предыдущий абзац" onclick={() => skipParagraph(-1)}>
        <svg viewBox="0 0 24 24"><path d="M7 6v12M18 6l-8 6 8 6z" /></svg>
      </button>
      <button
        type="button"
        class="b main"
        title={tts.paused ? "Продолжить" : "Пауза"}
        aria-label={tts.paused ? "Продолжить" : "Пауза"}
        onclick={() => (tts.paused ? resumeReading() : pauseReading())}
      >
        {#if tts.paused}
          <svg viewBox="0 0 24 24"><path d="M8 5l11 7-11 7z" /></svg>
        {:else}
          <svg viewBox="0 0 24 24"><path d="M8 5v14M16 5v14" /></svg>
        {/if}
      </button>
      <button type="button" class="b" title="Следующий абзац" aria-label="Следующий абзац" onclick={() => skipParagraph(1)}>
        <svg viewBox="0 0 24 24"><path d="M17 6v12M6 6l8 6-8 6z" /></svg>
      </button>
      <button
        type="button"
        class="rate"
        title="Скорость"
        onclick={() => saveTts({ rate: tts.s.rate >= 1.75 ? 0.8 : Math.round((tts.s.rate + 0.2) * 100) / 100 })}
      >{tts.s.rate.toFixed(1)}×</button>
      <button type="button" class="b" title="Остановить" aria-label="Остановить" onclick={stopReading}>
        <svg viewBox="0 0 24 24"><rect x="6" y="6" width="12" height="12" rx="2" /></svg>
      </button>
    {/if}
  </div>
{/if}

<style>
  .tb {
    position: fixed;
    z-index: 400;
    left: 50%;
    bottom: calc(3.2rem + env(safe-area-inset-bottom));
    transform: translateX(-50%);
    display: flex;
    align-items: center;
    gap: 0.25rem;
    padding: 0.3rem 0.45rem;
    border-radius: 999px;
    background: var(--toolbar-surface);
    border: 1px solid var(--toolbar-border);
    box-shadow: var(--shadow-float);
    color: var(--text-soft);
    animation: up 0.2s ease-out;
  }

  .b {
    display: grid;
    place-items: center;
    width: 2.2rem;
    height: 2.2rem;
    border-radius: 999px;
    border: none;
    background: transparent;
    color: inherit;
    cursor: pointer;
  }

  .b:hover {
    background: var(--panel-soft);
  }

  .b.main {
    background: var(--accent-2);
    color: var(--elevated-soft);
  }

  .b svg {
    width: 1rem;
    height: 1rem;
    fill: currentColor;
    stroke: currentColor;
    stroke-width: 1.6;
    stroke-linejoin: round;
  }

  .rate {
    border: 1px solid var(--border-soft);
    background: transparent;
    color: var(--muted);
    border-radius: 999px;
    padding: 0.2rem 0.5rem;
    font-size: 0.74rem;
    font-variant-numeric: tabular-nums;
    cursor: pointer;
  }

  .wave {
    display: flex;
    align-items: center;
    gap: 2px;
    height: 16px;
    padding: 0 0.4rem;
  }

  .wave i {
    width: 3px;
    height: 100%;
    border-radius: 2px;
    background: var(--accent);
    animation: wave 1s ease-in-out infinite;
  }

  .wave i:nth-child(2) {
    animation-delay: 0.15s;
  }
  .wave i:nth-child(3) {
    animation-delay: 0.3s;
  }
  .wave i:nth-child(4) {
    animation-delay: 0.45s;
  }

  .wave.still i {
    animation: none;
    height: 30%;
  }

  @keyframes wave {
    0%,
    100% {
      transform: scaleY(0.3);
    }
    50% {
      transform: scaleY(1);
    }
  }

  .err {
    font-size: 0.8rem;
    color: var(--danger);
    padding: 0 0.6rem;
    max-width: 70vw;
  }

  @keyframes up {
    from {
      opacity: 0;
      transform: translate(-50%, 8px);
    }
  }
</style>
