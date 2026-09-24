<script lang="ts">
  import type { HighlightColor } from "$lib/types";
  import { HIGHLIGHT_COLORS } from "$lib/reading/highlights";

  interface Props {
    visible: boolean;
    rect: { left: number; top: number; width: number; height: number } | null;
    /** Короткое выделение (слово/фраза) — показываем «Перевод» и «Кто это?» */
    short?: boolean;
    onHighlight: (color: HighlightColor) => void;
    onNote: () => void;
    onQuote: () => void;
    onCopy: () => void;
    onTranslate?: () => void;
    onWho?: () => void;
    onSearch?: () => void;
  }
  let { visible, rect, short = false, onHighlight, onNote, onQuote, onCopy, onTranslate, onWho, onSearch }: Props =
    $props();

  let el = $state<HTMLDivElement | null>(null);
  let width = $state(360);

  const pos = $derived.by(() => {
    if (!rect) return { x: 0, y: 0, below: false };
    const vw = typeof window !== "undefined" ? window.innerWidth : 1200;
    const half = width / 2 + 8;
    const x = Math.min(vw - half, Math.max(half, rect.left + rect.width / 2));
    const below = rect.top < 90;
    return { x, y: below ? rect.top + rect.height + 10 : rect.top - 10, below };
  });

  $effect(() => {
    if (el && visible) width = el.offsetWidth;
  });
</script>

{#if visible && rect}
  <div
    bind:this={el}
    class="tb"
    class:below={pos.below}
    style="left: {pos.x}px; top: {pos.y}px;"
    role="toolbar"
    aria-label="Действия с выделением"
  >
    <div class="tb-colors">
      {#each HIGHLIGHT_COLORS as c (c.id)}
        <button
          type="button"
          class="dot"
          style:--c={c.fill}
          title="Выделить: {c.label.toLowerCase()}"
          aria-label="Выделить цветом: {c.label}"
          onmousedown={(e) => e.preventDefault()}
          onclick={() => onHighlight(c.id)}
        ></button>
      {/each}
    </div>
    <span class="sep" aria-hidden="true"></span>
    <button type="button" class="tbtn" onmousedown={(e) => e.preventDefault()} onclick={onNote}>Заметка</button>
    <button type="button" class="tbtn" onmousedown={(e) => e.preventDefault()} onclick={onQuote}>Цитата</button>
    {#if short && onTranslate}
      <button type="button" class="tbtn" onmousedown={(e) => e.preventDefault()} onclick={onTranslate}>Перевод</button>
    {/if}
    {#if short && onWho}
      <button type="button" class="tbtn" onmousedown={(e) => e.preventDefault()} onclick={onWho}>Кто это?</button>
    {/if}
    {#if short && onSearch}
      <button type="button" class="tbtn icon" title="Найти в книге" aria-label="Найти в книге" onclick={onSearch}>
        <svg viewBox="0 0 24 24" aria-hidden="true"><circle cx="11" cy="11" r="6.5" /><path d="m16 16 4 4" /></svg>
      </button>
    {/if}
    <button type="button" class="tbtn icon" title="Копировать" aria-label="Копировать" onclick={onCopy}>
      <svg viewBox="0 0 24 24" aria-hidden="true"><rect x="8" y="8" width="12" height="12" rx="2" /><path d="M16 8V5a1 1 0 0 0-1-1H5a1 1 0 0 0-1 1v10a1 1 0 0 0 1 1h3" /></svg>
    </button>
  </div>
{/if}

<style>
  .tb {
    position: fixed;
    z-index: 500;
    display: flex;
    align-items: center;
    gap: 0.2rem;
    padding: 0.28rem 0.35rem;
    border-radius: 999px;
    background-color: var(--toolbar-surface);
    border: 1px solid var(--toolbar-border);
    color: var(--text-soft);
    box-shadow: var(--shadow-float);
    transform: translate(-50%, -100%);
    animation: tb-in 0.14s ease-out;
    white-space: nowrap;
  }

  .tb.below {
    transform: translate(-50%, 0);
  }

  .tb-colors {
    display: flex;
    gap: 0.22rem;
    padding: 0 0.2rem;
  }

  .dot {
    width: 1.25rem;
    height: 1.25rem;
    border-radius: 50%;
    border: 2px solid color-mix(in srgb, var(--c) 70%, #000 12%);
    background: var(--c);
    cursor: pointer;
    padding: 0;
    transition: transform 0.12s ease;
  }

  .dot:hover {
    transform: scale(1.18);
  }

  .sep {
    width: 1px;
    height: 1.2rem;
    background: var(--toolbar-border);
    margin: 0 0.15rem;
  }

  .tbtn {
    padding: 0.38rem 0.62rem;
    border-radius: 999px;
    border: 1px solid transparent;
    background: transparent;
    color: var(--text-soft);
    font-size: 0.8rem;
    font-weight: 550;
    cursor: pointer;
  }

  .tbtn.icon {
    padding: 0.32rem 0.42rem;
    display: grid;
    place-items: center;
  }

  .tbtn svg {
    width: 1rem;
    height: 1rem;
    fill: none;
    stroke: currentColor;
    stroke-width: 1.8;
    stroke-linecap: round;
    stroke-linejoin: round;
  }

  .tbtn:hover {
    background: var(--panel-soft);
    border-color: var(--border-soft);
  }

  @keyframes tb-in {
    from {
      opacity: 0;
    }
  }

  @media (max-width: 600px) {
    .tb {
      left: max(0.5rem, env(safe-area-inset-left)) !important;
      right: max(0.5rem, env(safe-area-inset-right));
      top: auto !important;
      bottom: max(0.55rem, env(safe-area-inset-bottom));
      transform: none !important;
      border-radius: 1rem;
      flex-wrap: wrap;
      justify-content: center;
      white-space: normal;
    }

    .dot {
      width: 1.7rem;
      height: 1.7rem;
    }

    .tbtn {
      min-height: 2.5rem;
    }
  }
</style>
