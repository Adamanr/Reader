<script lang="ts">
  import type { BookMeta } from "$lib/types";
  import BookCoverThumb from "$lib/components/BookCoverThumb.svelte";
  import { formatBadgeLabel, getBookFormat } from "$lib/bookFormat";
  import { bookProgress, bookTitle } from "$lib/library/bookInfo";
  import { coverTone, type CoverTone } from "$lib/reading/palette";
  import { formatMinutes } from "$lib/reading/stats.svelte";

  interface Props {
    books: { path: string; meta: BookMeta }[];
    onOpen: (path: string) => void;
  }
  let { books, onOpen }: Props = $props();

  const main = $derived(books[0]);
  const rest = $derived(books.slice(1, 4));
  let tone = $state<CoverTone | null>(null);

  const progress = $derived(main ? bookProgress(main.meta) : null);
  const glow = $derived(tone ? `hsl(${tone.h} ${Math.max(30, Math.min(tone.s, 60))}% 60% / 0.35)` : "transparent");

  function relTime(iso: string | null | undefined): string {
    if (!iso) return "";
    const t = Date.parse(iso);
    if (!Number.isFinite(t)) return "";
    const min = (Date.now() - t) / 60000;
    if (min < 60) return "только что";
    if (min < 60 * 24) return `${Math.floor(min / 60)} ч назад`;
    const d = Math.floor(min / 1440);
    if (d === 1) return "вчера";
    if (d < 7) return `${d} дн. назад`;
    return new Date(t).toLocaleDateString("ru", { day: "numeric", month: "long" });
  }
</script>

{#if main}
  {@const fmt = getBookFormat(main.path)}
  <section class="cr" style:--glow={glow} aria-label="Продолжить чтение">
    <button type="button" class="cr-main" onclick={() => onOpen(main.path)}>
      <div class="cr-cover">
        <BookCoverThumb
          bookPath={main.path}
          format={fmt}
          eager
          onCover={(u) => {
            void coverTone(u).then((t) => (tone = t));
          }}
        >
          <span class="cr-fmt">{fmt ? formatBadgeLabel(fmt) : "?"}</span>
        </BookCoverThumb>
      </div>
      <div class="cr-body">
        <p class="cr-kicker">Продолжить чтение · {relTime(main.meta.lastOpenedAt)}</p>
        <h2 class="cr-title">{bookTitle(main.path, main.meta)}</h2>
        {#if main.meta.author?.trim()}
          <p class="cr-author">{main.meta.author}</p>
        {/if}
        {#if main.meta.lastReadLocationLabel?.trim()}
          <p class="cr-chapter">{main.meta.lastReadLocationLabel}</p>
        {/if}
        <div class="cr-progress">
          <div class="cr-track"><div class="cr-fill" style:width="{Math.round((progress ?? 0) * 100)}%"></div></div>
          <span class="cr-pct">{progress != null ? `${Math.round(progress * 100)}%` : "только начата"}</span>
        </div>
        <div class="cr-foot">
          <span class="cr-cta">Читать дальше →</span>
          {#if main.meta.minutesLeft}
            <span class="cr-left">≈ {formatMinutes(main.meta.minutesLeft)} до конца</span>
          {/if}
        </div>
      </div>
    </button>
    {#if rest.length}
      <ul class="cr-rest">
        {#each rest as b (b.path)}
          {@const f = getBookFormat(b.path)}
          {@const p = bookProgress(b.meta)}
          <li>
            <button type="button" class="cr-mini" onclick={() => onOpen(b.path)} title={bookTitle(b.path, b.meta)}>
              <span class="cr-mini-cover">
                <BookCoverThumb bookPath={b.path} format={f} eager>
                  <span class="cr-fmt">{f ? formatBadgeLabel(f) : "?"}</span>
                </BookCoverThumb>
              </span>
              <span class="cr-mini-text">
                <span class="cr-mini-title">{bookTitle(b.path, b.meta)}</span>
                <span class="cr-mini-meta">{p != null ? `${Math.round(p * 100)}%` : ""} · {relTime(b.meta.lastOpenedAt)}</span>
              </span>
            </button>
          </li>
        {/each}
      </ul>
    {/if}
  </section>
{/if}

<style>
  .cr {
    display: grid;
    grid-template-columns: minmax(0, 1.7fr) minmax(0, 1fr);
    gap: 1rem;
    margin-bottom: 1.6rem;
  }

  .cr-main {
    position: relative;
    display: flex;
    gap: 1.3rem;
    padding: 1.1rem;
    border-radius: var(--radius-xl);
    border: 1px solid color-mix(in srgb, var(--border-soft) 80%, transparent);
    background:
      radial-gradient(ellipse 70% 120% at 0% 0%, var(--glow), transparent 70%),
      var(--panel-elevated);
    box-shadow: var(--shadow-soft);
    text-align: left;
    color: var(--text-soft);
    cursor: pointer;
    overflow: hidden;
    transition:
      transform 0.25s ease,
      box-shadow 0.25s ease;
  }

  .cr-main:hover {
    transform: translateY(-3px);
    box-shadow: var(--shadow-book);
  }

  .cr-cover {
    width: 7.4rem;
    flex-shrink: 0;
    aspect-ratio: 3 / 4.3;
    border-radius: 0.6rem;
    overflow: hidden;
    background: linear-gradient(155deg, #dcd6ee, #b8aed4);
    box-shadow:
      0 14px 30px rgba(30, 20, 60, 0.22),
      0 2px 6px rgba(30, 20, 60, 0.18);
    transform: rotate(-2deg);
    transition: transform 0.3s ease;
  }

  .cr-main:hover .cr-cover {
    transform: rotate(0deg) scale(1.03);
  }

  .cr-fmt {
    font-size: 0.7rem;
    font-weight: 750;
    letter-spacing: 0.16em;
    color: #3d3555;
  }

  .cr-body {
    display: flex;
    flex-direction: column;
    gap: 0.3rem;
    min-width: 0;
    flex: 1;
    padding: 0.2rem 0;
  }

  .cr-kicker {
    margin: 0;
    font-size: 0.68rem;
    text-transform: uppercase;
    letter-spacing: 0.14em;
    color: var(--muted);
  }

  .cr-title {
    margin: 0.2rem 0 0;
    font-family: "Literata Variable", Georgia, serif;
    font-size: clamp(1.2rem, 2.4vw, 1.65rem);
    font-weight: 600;
    line-height: 1.2;
    display: -webkit-box;
    -webkit-line-clamp: 2;
    line-clamp: 2;
    -webkit-box-orient: vertical;
    overflow: hidden;
  }

  .cr-author {
    margin: 0;
    font-size: 0.88rem;
    color: var(--muted);
  }

  .cr-chapter {
    margin: 0.35rem 0 0;
    font-size: 0.8rem;
    color: var(--accent-2);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .cr-progress {
    display: flex;
    align-items: center;
    gap: 0.6rem;
    margin-top: auto;
    padding-top: 0.6rem;
  }

  .cr-track {
    flex: 1;
    height: 5px;
    border-radius: 999px;
    background: color-mix(in srgb, var(--border-soft) 85%, transparent);
    overflow: hidden;
  }

  .cr-fill {
    height: 100%;
    border-radius: inherit;
    background: linear-gradient(90deg, var(--accent), var(--accent-2));
  }

  .cr-pct {
    font-size: 0.78rem;
    font-variant-numeric: tabular-nums;
    color: var(--muted);
  }

  .cr-foot {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 0.6rem;
    margin-top: 0.45rem;
  }

  .cr-cta {
    font-size: 0.86rem;
    font-weight: 650;
    color: var(--accent-2);
  }

  .cr-left {
    font-size: 0.76rem;
    color: var(--muted);
  }

  .cr-rest {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 0.55rem;
  }

  .cr-mini {
    width: 100%;
    display: flex;
    align-items: center;
    gap: 0.75rem;
    padding: 0.55rem;
    border-radius: var(--radius-md);
    border: 1px solid color-mix(in srgb, var(--border-soft) 70%, transparent);
    background: color-mix(in srgb, var(--panel-elevated) 80%, transparent);
    color: var(--text-soft);
    text-align: left;
    cursor: pointer;
    transition: background 0.15s ease;
  }

  .cr-mini:hover {
    background: var(--panel-elevated);
  }

  .cr-mini-cover {
    width: 2.6rem;
    flex-shrink: 0;
    aspect-ratio: 3 / 4.3;
    border-radius: 0.35rem;
    overflow: hidden;
    background: linear-gradient(155deg, #dcd6ee, #b8aed4);
  }

  .cr-mini-cover .cr-fmt {
    font-size: 0.45rem;
  }

  .cr-mini-text {
    display: flex;
    flex-direction: column;
    gap: 0.15rem;
    min-width: 0;
  }

  .cr-mini-title {
    font-size: 0.84rem;
    font-weight: 550;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .cr-mini-meta {
    font-size: 0.72rem;
    color: var(--muted);
  }

  @media (max-width: 860px) {
    .cr {
      grid-template-columns: 1fr;
    }
  }

  @media (max-width: 520px) {
    .cr-main {
      gap: 0.9rem;
      padding: 0.85rem;
    }
    .cr-cover {
      width: 5.4rem;
    }
  }
</style>
