<script lang="ts">
  import type { BookFormat } from "$lib/bookFormat";

  interface Props {
    open?: boolean;
    format: BookFormat;
    ttsActive: boolean;
    /** Название включённого фонового звука, если он играет. */
    ambientLabel: string | null;
    isFullscreen: boolean;
    typstExportBusy: boolean;
    onTranslate: () => void;
    onRecap: () => void;
    onToggleTts: () => void;
    onRsvp: () => void;
    onAmbient: () => void;
    onFullscreen: () => void;
    onExportNotes: () => void;
    onExportTypst: () => void;
  }

  let {
    open = $bindable(false),
    format,
    ttsActive,
    ambientLabel,
    isFullscreen,
    typstExportBusy,
    onTranslate,
    onRecap,
    onToggleTts,
    onRsvp,
    onAmbient,
    onFullscreen,
    onExportNotes,
    onExportTypst,
  }: Props = $props();

  /** Пункты, открывающие панель или диалог, сначала закрывают меню. */
  function closeThen(action: () => void) {
    open = false;
    action();
  }
</script>

<div class="more-wrap">
  <button
    type="button"
    class="icon-btn"
    class:on={open}
    title="Ещё"
    aria-label="Ещё"
    aria-expanded={open}
    onclick={() => (open = !open)}
  >
    <svg viewBox="0 0 24 24" aria-hidden="true"><circle cx="5" cy="12" r="1.3" /><circle cx="12" cy="12" r="1.3" /><circle cx="19" cy="12" r="1.3" /></svg>
  </button>
  {#if open}
    <!-- svelte-ignore a11y_click_events_have_key_events -->
    <!-- svelte-ignore a11y_no_static_element_interactions -->
    <div class="menu-back" onclick={() => (open = false)}></div>
    <div class="menu" role="menu">
      {#if format !== "typst"}
        <button type="button" role="menuitem" onclick={() => closeThen(onTranslate)}>Перевод книги…</button>
        <button type="button" role="menuitem" onclick={() => closeThen(onRecap)}>
          Ранее в книге… <kbd>без спойлеров</kbd>
        </button>
        <button type="button" role="menuitem" onclick={onToggleTts}>
          {ttsActive ? "Остановить чтение вслух" : "Читать вслух"} <kbd>S</kbd>
        </button>
        <button type="button" role="menuitem" onclick={onRsvp}>Быстрое чтение (RSVP) <kbd>R</kbd></button>
      {/if}
      <button type="button" role="menuitem" onclick={() => closeThen(onAmbient)}>
        Фоновый звук… {#if ambientLabel}<kbd>♪ {ambientLabel}</kbd>{/if}
      </button>
      <div class="menu-sep"></div>
      <button type="button" role="menuitem" onclick={onFullscreen}>
        {isFullscreen ? "Выйти из полного экрана" : "Во весь экран"} <kbd>F11</kbd>
      </button>
      {#if format !== "typst"}
        <button type="button" role="menuitem" onclick={() => closeThen(onExportNotes)}>Конспект в Markdown</button>
        <button type="button" role="menuitem" disabled={typstExportBusy} onclick={onExportTypst}>
          {typstExportBusy ? "Экспорт в Typst…" : "Экспорт в Typst"}
        </button>
      {/if}
      <div class="menu-sep"></div>
      <a role="menuitem" href="/settings">Настройки</a>
    </div>
  {/if}
</div>

<style>
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

  @keyframes pop {
    from {
      opacity: 0;
      transform: translateY(-4px);
    }
  }
</style>
