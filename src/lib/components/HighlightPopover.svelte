<script lang="ts">
  import type { Highlight, HighlightColor } from "$lib/types";
  import { HIGHLIGHT_COLORS } from "$lib/reading/highlights";

  interface Props {
    highlight: Highlight;
    rect: { left: number; top: number; width: number; height: number };
    /** Сразу открыть поле заметки */
    editNote?: boolean;
    onChange: (patch: Partial<Highlight>) => void;
    onDelete: () => void;
    onQuote: () => void;
    onClose: () => void;
  }
  let { highlight, rect, editNote = false, onChange, onDelete, onQuote, onClose }: Props = $props();

  let note = $state("");
  let editing = $state(false);
  let lastId = "";

  $effect(() => {
    if (highlight.id !== lastId) {
      lastId = highlight.id;
      note = highlight.note ?? "";
      editing = editNote || !!highlight.note;
    }
  });

  const pos = $derived.by(() => {
    const vw = typeof window !== "undefined" ? window.innerWidth : 1200;
    const vh = typeof window !== "undefined" ? window.innerHeight : 800;
    const w = Math.min(340, vw - 24);
    const x = Math.min(vw - w - 12, Math.max(12, rect.left + rect.width / 2 - w / 2));
    const below = rect.top + rect.height + 280 < vh;
    return { x, w, y: below ? rect.top + rect.height + 10 : Math.max(12, rect.top - 10), below };
  });

  function saveNote() {
    const t = note.trim();
    if ((highlight.note ?? "") !== t) onChange({ note: t || undefined });
  }

  function onKey(e: KeyboardEvent) {
    if (e.key === "Escape") {
      e.stopPropagation();
      saveNote();
      onClose();
    }
    if (e.key === "Enter" && (e.ctrlKey || e.metaKey)) {
      saveNote();
      onClose();
    }
  }
</script>

<!-- svelte-ignore a11y_click_events_have_key_events -->
<!-- svelte-ignore a11y_no_static_element_interactions -->
<div
  class="hp-back"
  onclick={() => {
    saveNote();
    onClose();
  }}
></div>
<div
  class="hp"
  class:above={!pos.below}
  style="left:{pos.x}px; top:{pos.y}px; width:{pos.w}px"
  role="dialog"
  aria-label="Выделение"
  tabindex="-1"
  onkeydown={onKey}
>
  <div class="hp-row">
    {#each HIGHLIGHT_COLORS as c (c.id)}
      <button
        type="button"
        class="dot"
        class:active={highlight.color === c.id}
        style:--c={c.fill}
        aria-label={c.label}
        title={c.label}
        onclick={() => onChange({ color: c.id as HighlightColor })}
      ></button>
    {/each}
    <span class="grow"></span>
    <button type="button" class="hp-btn" onclick={onQuote}>Цитата</button>
    <button type="button" class="hp-btn danger" onclick={onDelete}>Удалить</button>
  </div>
  <blockquote class="hp-text">{highlight.text.length > 240 ? highlight.text.slice(0, 240) + "…" : highlight.text}</blockquote>
  {#if editing}
    <!-- svelte-ignore a11y_autofocus -->
    <textarea
      class="hp-note"
      rows="3"
      placeholder="Мысль на полях…"
      bind:value={note}
      onblur={saveNote}
      autofocus={editNote}
    ></textarea>
    <p class="hp-hint">Ctrl+Enter — сохранить</p>
  {:else}
    <button type="button" class="hp-add" onclick={() => (editing = true)}>+ Добавить заметку на полях</button>
  {/if}
</div>

<style>
  .hp-back {
    position: fixed;
    inset: 0;
    z-index: 540;
  }

  .hp {
    position: fixed;
    z-index: 550;
    display: flex;
    flex-direction: column;
    gap: 0.55rem;
    padding: 0.75rem;
    border-radius: var(--radius-md);
    background: var(--toolbar-surface);
    border: 1px solid var(--toolbar-border);
    box-shadow: var(--shadow-float);
    color: var(--text-soft);
    animation: hp-in 0.15s ease-out;
  }

  .hp.above {
    transform: translateY(-100%);
  }

  .hp-row {
    display: flex;
    align-items: center;
    gap: 0.3rem;
  }

  .grow {
    flex: 1;
  }

  .dot {
    width: 1.3rem;
    height: 1.3rem;
    border-radius: 50%;
    border: 2px solid transparent;
    background: var(--c);
    cursor: pointer;
    padding: 0;
  }

  .dot.active {
    border-color: var(--text-soft);
  }

  .hp-btn {
    border: 1px solid var(--border-soft);
    background: var(--elevated-soft);
    color: var(--text-soft);
    border-radius: 999px;
    padding: 0.25rem 0.65rem;
    font-size: 0.78rem;
    cursor: pointer;
  }

  .hp-btn.danger {
    color: var(--danger);
  }

  .hp-text {
    margin: 0;
    padding-left: 0.6rem;
    border-left: 3px solid var(--accent);
    font-size: 0.84rem;
    line-height: 1.45;
    color: var(--muted);
    max-height: 7rem;
    overflow: auto;
  }

  .hp-note {
    width: 100%;
    resize: vertical;
    border-radius: var(--radius-sm);
    border: 1px solid var(--border-soft);
    background: var(--elevated-soft);
    color: var(--text-soft);
    padding: 0.5rem 0.6rem;
    font: inherit;
    font-size: 0.88rem;
  }

  .hp-hint {
    margin: -0.3rem 0 0;
    font-size: 0.7rem;
    color: var(--muted);
    text-align: right;
  }

  .hp-add {
    border: 1px dashed var(--border-soft);
    background: transparent;
    color: var(--muted);
    border-radius: var(--radius-sm);
    padding: 0.45rem;
    font-size: 0.82rem;
    cursor: pointer;
  }

  @keyframes hp-in {
    from {
      opacity: 0;
    }
  }
</style>
