<script lang="ts">
  interface Props {
    visible: boolean;
    x: number;
    y: number;
    onCopy: () => void;
    onComment: () => void;
    onQuote: () => void;
  }
  let { visible, x, y, onCopy, onComment, onQuote }: Props = $props();
</script>

{#if visible}
  <div
    class="tb"
    style="left: {x}px; top: {y}px;"
    role="toolbar"
    aria-label="Действия с выделением"
  >
    <button type="button" class="tbtn" onclick={onCopy}>Копировать</button>
    <button type="button" class="tbtn" onclick={onComment}>Комментарий</button>
    <button type="button" class="tbtn accent" onclick={onQuote}>Сделать цитату</button>
  </div>
{/if}

<style>
  .tb {
    position: fixed;
    z-index: 500;
    display: flex;
    gap: 0.3rem;
    border-radius: 999px;
    /* Непрозрачный фон — не зависит от color-mix в WebView */
    background-color: var(--toolbar-surface);
    border: 1px solid var(--toolbar-border);
    color: var(--text-soft);
    box-shadow: var(--shadow-float);
    transform: translate(-50%, -100%);
    margin-top: -10px;
    max-width: min(96vw, 22rem);
  }

  .tbtn {
    padding: 0.4rem 0.72rem;
    border-radius: 999px;
    border: 1px solid transparent;
    background: transparent;
    color: var(--text-soft);
    font-size: 0.8rem;
    font-weight: 550;
    cursor: pointer;
    white-space: nowrap;
    transition:
      background 0.12s ease,
      border-color 0.12s ease;
  }

  .tbtn:hover {
    background: var(--panel-soft);
    border-color: var(--border-soft);
  }

  .tbtn.accent {
    background: var(--panel-soft);
    border-color: var(--accent-2);
    color: var(--accent-2);
    font-weight: 650;
  }

  .tbtn.accent:hover {
    filter: brightness(1.05);
  }

  @media (max-width: 600px) {
    .tb {
      left: max(0.5rem, env(safe-area-inset-left)) !important;
      right: max(0.5rem, env(safe-area-inset-right));
      top: auto !important;
      bottom: max(0.55rem, env(safe-area-inset-bottom));
      width: auto;
      max-width: none;
      margin: 0;
      padding: 0.25rem;
      transform: none;
      border-radius: 1rem;
    }

    .tbtn {
      flex: 1;
      min-width: 0;
      min-height: 2.75rem;
      padding-inline: 0.4rem;
      white-space: normal;
      line-height: 1.15;
    }
  }
</style>
