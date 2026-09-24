<script lang="ts">
  import type { ReadingPosition } from "$lib/types";
  import { formatMinutes, minutesLeft } from "$lib/reading/stats.svelte";

  interface Props {
    position: ReadingPosition | null;
    quiet?: boolean;
    onSeek?: (fraction: number) => void;
  }
  let { position, quiet = false, onSeek }: Props = $props();

  let hover = $state<number | null>(null);
  let bar = $state<HTMLDivElement | null>(null);

  const pct = $derived(position?.progress != null ? Math.round(position.progress * 1000) / 10 : null);
  const chapterLeft = $derived(
    position
      ? minutesLeft({ charsLeft: position.charsLeftChapter, pagesLeft: position.pagesLeftChapter })
      : null,
  );
  const bookLeft = $derived(
    position ? minutesLeft({ charsLeft: position.charsLeftBook, pagesLeft: position.pagesLeftBook }) : null,
  );

  function fractionAt(e: PointerEvent): number {
    const r = bar!.getBoundingClientRect();
    return Math.min(1, Math.max(0, (e.clientX - r.left) / r.width));
  }

  let dragging = false;
  function down(e: PointerEvent) {
    if (!onSeek || !bar) return;
    dragging = true;
    bar.setPointerCapture(e.pointerId);
    hover = fractionAt(e);
  }
  function move(e: PointerEvent) {
    if (!bar) return;
    hover = fractionAt(e);
  }
  function up(e: PointerEvent) {
    if (!dragging || !bar) return;
    dragging = false;
    onSeek?.(fractionAt(e));
  }
  function onKey(e: KeyboardEvent) {
    const p = position?.progress ?? 0;
    if (e.key === "ArrowRight") onSeek?.(Math.min(1, p + 0.01));
    if (e.key === "ArrowLeft") onSeek?.(Math.max(0, p - 0.01));
  }
</script>

<footer class="sb" class:quiet>
  <div
    class="sb-track"
    bind:this={bar}
    role="slider"
    tabindex="0"
    aria-label="Позиция в книге"
    aria-valuemin={0}
    aria-valuemax={100}
    aria-valuenow={pct ?? 0}
    onpointerdown={down}
    onpointermove={move}
    onpointerup={up}
    onpointerleave={() => {
      if (!dragging) hover = null;
    }}
    onkeydown={onKey}
  >
    <div class="sb-fill" style:width="{pct ?? 0}%"></div>
    {#if hover != null}
      <div class="sb-ghost" style:left="{hover * 100}%">
        <span>{Math.round(hover * 100)}%</span>
      </div>
    {/if}
  </div>
  <div class="sb-row">
    <span class="sb-chapter" title={position?.chapterLabel ?? ""}>{position?.chapterLabel || " "}</span>
    <span class="sb-time">
      {#if chapterLeft != null && position?.chapterLabel}
        {formatMinutes(chapterLeft)} до конца главы
      {:else if bookLeft != null}
        ~{formatMinutes(bookLeft)} до конца книги
      {/if}
    </span>
    <span class="sb-pct">
      {#if position?.pageLabel}<span class="sb-page">{position.pageLabel}</span>{/if}
      {pct != null ? `${Math.floor(pct)}%` : ""}
    </span>
  </div>
</footer>

<style>
  .sb {
    flex-shrink: 0;
    padding: 0 1rem max(0.35rem, env(safe-area-inset-bottom));
    color: var(--chrome-muted, var(--muted));
    font-size: 0.74rem;
    transition: opacity 0.3s ease;
    background: transparent;
  }

  .sb.quiet {
    opacity: 0.45;
  }

  .sb.quiet:hover {
    opacity: 1;
  }

  .sb-track {
    position: relative;
    height: 14px;
    cursor: pointer;
    touch-action: none;
  }

  .sb-track::before {
    content: "";
    position: absolute;
    left: 0;
    right: 0;
    top: 6px;
    height: 2px;
    border-radius: 2px;
    background: color-mix(in srgb, var(--chrome-muted, var(--muted)) 25%, transparent);
    transition:
      height 0.15s ease,
      top 0.15s ease;
  }

  .sb-fill {
    position: absolute;
    left: 0;
    top: 6px;
    height: 2px;
    border-radius: 2px;
    background: var(--accent-2);
    transition:
      width 0.25s ease,
      height 0.15s ease,
      top 0.15s ease;
  }

  .sb-track:hover::before,
  .sb-track:hover .sb-fill {
    top: 5px;
    height: 4px;
  }

  .sb-ghost {
    position: absolute;
    top: -1.5rem;
    transform: translateX(-50%);
    pointer-events: none;
  }

  .sb-ghost span {
    padding: 0.1rem 0.4rem;
    border-radius: 6px;
    background: var(--toolbar-surface);
    border: 1px solid var(--toolbar-border);
    color: var(--text-soft);
    font-variant-numeric: tabular-nums;
  }

  .sb-row {
    display: grid;
    grid-template-columns: 1fr auto 1fr;
    gap: 0.75rem;
    align-items: center;
    min-height: 1.3rem;
  }

  .sb-chapter {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .sb-time {
    text-align: center;
    white-space: nowrap;
  }

  .sb-pct {
    text-align: right;
    font-variant-numeric: tabular-nums;
    white-space: nowrap;
  }

  .sb-page {
    margin-right: 0.6rem;
    opacity: 0.8;
  }

  @media (max-width: 600px) {
    .sb-row {
      grid-template-columns: 1fr auto;
    }
    .sb-time {
      display: none;
    }
    .sb-page {
      display: none;
    }
  }
</style>
