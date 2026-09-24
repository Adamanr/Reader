<script lang="ts">
  import { onMount } from "svelte";
  import type { BookMeta } from "$lib/types";
  import Book3D from "$lib/components/home/Book3D.svelte";
  import { bookProgress, bookTitle } from "$lib/library/bookInfo";
  import { coverTone, type CoverTone } from "$lib/reading/palette";
  import { dayKey, formatMinutes, loadStats, stats, streak } from "$lib/reading/stats.svelte";
  import { loadCover, peekCover } from "$lib/covers/coverCache";
  import { getBookFormat } from "$lib/bookFormat";

  interface Props {
    books: { path: string; meta: BookMeta }[];
    total: number;
    onOpen: (path: string) => void;
    onTone?: (t: CoverTone | null) => void;
  }
  let { books, total, onOpen, onTone }: Props = $props();

  let coverUrl = $state<string | null>(null);
  let now = $state(new Date());

  onMount(() => {
    void loadStats();
    const t = setInterval(() => (now = new Date()), 60_000);
    return () => clearInterval(t);
  });

  const main = $derived(books[0]);
  const rest = $derived(books.slice(1, 5));

  $effect(() => {
    const m = main;
    coverUrl = m ? (peekCover(m.path) ?? null) : null;
    if (!m) {
      onTone?.(null);
      return;
    }
    let alive = true;
    void loadCover(m.path, getBookFormat(m.path)).then(async (u) => {
      if (!alive) return;
      coverUrl = u;
      onTone?.(await coverTone(u));
    });
    return () => {
      alive = false;
    };
  });

  const greeting = $derived.by(() => {
    const h = now.getHours();
    if (h < 5) return "Доброй ночи";
    if (h < 12) return "Доброе утро";
    if (h < 18) return "Добрый день";
    return "Добрый вечер";
  });

  const todayMin = $derived((stats.s.days[dayKey(now)]?.ms ?? 0) / 60000);
  const run = $derived(streak(stats.s.days));
  const progress = $derived(main ? bookProgress(main.meta) : null);

  const subline = $derived.by(() => {
    const parts: string[] = [];
    if (todayMin >= 1) parts.push(`сегодня вы читали ${formatMinutes(todayMin)}`);
    if (run > 1) parts.push(`${run} дн. подряд с книгой`);
    if (!parts.length) {
      if (main) return "Самое время вернуться к книге.";
      return total ? `На полках ${total} книг ждут своего часа.` : "";
    }
    const s = parts.join(" · ");
    return s.charAt(0).toUpperCase() + s.slice(1) + ".";
  });

  function relTime(iso: string | null | undefined): string {
    if (!iso) return "";
    const t = Date.parse(iso);
    if (!Number.isFinite(t)) return "";
    const min = (now.getTime() - t) / 60000;
    if (min < 60) return "только что";
    if (min < 60 * 24) return `${Math.floor(min / 60)} ч назад`;
    const d = Math.floor(min / 1440);
    if (d === 1) return "вчера";
    if (d < 7) return `${d} дн. назад`;
    return new Date(t).toLocaleDateString("ru", { day: "numeric", month: "long" });
  }
</script>

<header class="greet">
  <h1>{greeting}<span class="dot">.</span></h1>
  {#if subline}<p>{subline}</p>{/if}
</header>

{#if main}
  <section class="hero" aria-label="Продолжить чтение">
    {#if coverUrl}
      <div class="backdrop" style:background-image="url({coverUrl})" aria-hidden="true"></div>
    {/if}
    <div class="veil" aria-hidden="true"></div>
    <button type="button" class="hero-book book-hover-target-parent" onclick={() => onOpen(main.path)} aria-label="Открыть «{bookTitle(main.path, main.meta)}»">
      <div class="book-hover-target">
        <Book3D path={main.path} meta={main.meta} eager />
      </div>
    </button>
    <div class="hero-body">
      <p class="kicker">Продолжить чтение · {relTime(main.meta.lastOpenedAt)}</p>
      <h2>{bookTitle(main.path, main.meta)}</h2>
      {#if main.meta.author?.trim()}<p class="author">{main.meta.author}</p>{/if}
      {#if main.meta.lastReadLocationLabel?.trim()}
        <p class="chapter">
          <svg viewBox="0 0 24 24" aria-hidden="true"><path d="M7 4h10v16l-5-3.5L7 20z" /></svg>
          {main.meta.lastReadLocationLabel}
        </p>
      {/if}
      <div class="progress">
        <div class="track"><div class="fill" style:width="{Math.round((progress ?? 0) * 100)}%"></div></div>
        <span>{progress != null ? `${Math.round(progress * 100)}%` : "начало"}</span>
      </div>
      <div class="actions">
        <button type="button" class="read" onclick={() => onOpen(main.path)}>
          Читать
          <svg viewBox="0 0 24 24" aria-hidden="true"><path d="M5 12h14M13 6l6 6-6 6" /></svg>
        </button>
        {#if main.meta.minutesLeft}
          <span class="left">≈ {formatMinutes(main.meta.minutesLeft)} до конца</span>
        {/if}
      </div>
    </div>
  </section>

  {#if rest.length}
    <section class="also" aria-label="Также читаете">
      <h3>Также в процессе</h3>
      <ul>
        {#each rest as b (b.path)}
          {@const p = bookProgress(b.meta)}
          <li>
            <button type="button" class="mini" onclick={() => onOpen(b.path)}>
              <span class="mini-cover book-hover-target"><Book3D path={b.path} meta={b.meta} eager compact /></span>
              <span class="mini-text">
                <span class="mini-title">{bookTitle(b.path, b.meta)}</span>
                {#if b.meta.author}<span class="mini-author">{b.meta.author}</span>{/if}
                <span class="mini-track"><span style:width="{Math.round((p ?? 0) * 100)}%"></span></span>
                <span class="mini-meta">{p != null ? `${Math.round(p * 100)}%` : ""} · {relTime(b.meta.lastOpenedAt)}</span>
              </span>
            </button>
          </li>
        {/each}
      </ul>
    </section>
  {/if}
{/if}

<style>
  .greet {
    margin: 0.4rem 0 1.6rem;
  }

  .greet h1 {
    margin: 0;
    font-family: "Literata Variable", Georgia, serif;
    font-weight: 500;
    font-size: clamp(2rem, 4.6vw, 3.1rem);
    letter-spacing: -0.02em;
    line-height: 1.05;
    color: var(--text-soft);
  }

  .greet .dot {
    color: var(--accent);
  }

  .greet p {
    margin: 0.55rem 0 0;
    font-size: 1rem;
    color: var(--muted);
  }

  .hero {
    position: relative;
    isolation: isolate;
    display: grid;
    grid-template-columns: minmax(8.5rem, 11.5rem) 1fr;
    gap: clamp(1.2rem, 3vw, 2.4rem);
    align-items: center;
    padding: clamp(1.2rem, 3vw, 2rem) clamp(1.2rem, 3vw, 2.2rem);
    border-radius: 1.75rem;
    overflow: hidden;
    background: var(--panel-elevated);
    border: 1px solid color-mix(in srgb, var(--border-soft) 70%, transparent);
    box-shadow: var(--shadow-soft);
  }

  .backdrop {
    position: absolute;
    inset: -20%;
    z-index: -2;
    background-size: cover;
    background-position: center;
    filter: blur(48px) saturate(1.3);
    opacity: 0.55;
    transform: scale(1.1);
  }

  .veil {
    position: absolute;
    inset: 0;
    z-index: -1;
    background: linear-gradient(
      100deg,
      color-mix(in srgb, var(--panel-elevated) 35%, transparent) 0%,
      color-mix(in srgb, var(--panel-elevated) 78%, transparent) 42%,
      color-mix(in srgb, var(--panel-elevated) 88%, transparent) 100%
    );
  }

  .hero-book {
    border: none;
    background: none;
    padding: 0;
    cursor: pointer;
    width: 100%;
  }

  .hero-body {
    display: flex;
    flex-direction: column;
    gap: 0.35rem;
    min-width: 0;
  }

  .kicker {
    margin: 0;
    font-size: 0.72rem;
    text-transform: uppercase;
    letter-spacing: 0.16em;
    color: var(--muted);
  }

  h2 {
    margin: 0.3rem 0 0;
    font-family: "Literata Variable", Georgia, serif;
    font-size: clamp(1.45rem, 3vw, 2.15rem);
    font-weight: 600;
    line-height: 1.15;
    letter-spacing: -0.01em;
    color: var(--text-soft);
    display: -webkit-box;
    -webkit-line-clamp: 3;
    line-clamp: 3;
    -webkit-box-orient: vertical;
    overflow: hidden;
    overflow-wrap: anywhere;
  }

  .author {
    margin: 0;
    font-size: 1rem;
    color: var(--muted);
  }

  .chapter {
    display: flex;
    align-items: center;
    gap: 0.4rem;
    margin: 0.55rem 0 0;
    font-size: 0.88rem;
    color: var(--accent-2);
    min-width: 0;
  }

  .chapter svg {
    width: 1rem;
    height: 1rem;
    flex-shrink: 0;
    fill: currentColor;
    opacity: 0.8;
  }

  .progress {
    display: flex;
    align-items: center;
    gap: 0.8rem;
    margin-top: 1.1rem;
    max-width: 26rem;
  }

  .progress span {
    font-size: 0.82rem;
    color: var(--muted);
    font-variant-numeric: tabular-nums;
  }

  .track {
    flex: 1;
    height: 6px;
    border-radius: 999px;
    background: color-mix(in srgb, var(--text-soft) 10%, transparent);
    overflow: hidden;
  }

  .fill {
    height: 100%;
    border-radius: inherit;
    background: linear-gradient(90deg, var(--accent), var(--accent-2));
  }

  .actions {
    display: flex;
    align-items: center;
    gap: 1rem;
    margin-top: 1.1rem;
    flex-wrap: wrap;
  }

  .read {
    display: inline-flex;
    align-items: center;
    gap: 0.55rem;
    padding: 0.7rem 1.3rem;
    border: none;
    border-radius: 999px;
    background: var(--text-soft);
    color: var(--bg-soft);
    font: inherit;
    font-size: 0.95rem;
    font-weight: 600;
    cursor: pointer;
    box-shadow: 0 10px 24px -10px var(--text-soft);
    transition: transform 0.2s ease;
  }

  .read:hover {
    transform: translateX(3px);
  }

  .read svg {
    width: 1.05rem;
    height: 1.05rem;
    fill: none;
    stroke: currentColor;
    stroke-width: 2;
    stroke-linecap: round;
    stroke-linejoin: round;
  }

  .left {
    font-size: 0.86rem;
    color: var(--muted);
  }

  .also {
    margin-top: 1.6rem;
  }

  .also h3 {
    margin: 0 0 0.8rem;
    font-size: 0.74rem;
    text-transform: uppercase;
    letter-spacing: 0.14em;
    color: var(--muted);
    font-weight: 650;
  }

  .also ul {
    list-style: none;
    margin: 0;
    padding: 0;
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(15rem, 1fr));
    gap: 0.8rem;
  }

  .mini {
    width: 100%;
    display: flex;
    gap: 0.9rem;
    align-items: center;
    padding: 0.7rem 0.8rem;
    border-radius: 1.1rem;
    border: 1px solid color-mix(in srgb, var(--border-soft) 65%, transparent);
    background: color-mix(in srgb, var(--panel-elevated) 75%, transparent);
    color: var(--text-soft);
    text-align: left;
    cursor: pointer;
    transition:
      background 0.2s ease,
      transform 0.2s ease;
  }

  .mini:hover {
    background: var(--panel-elevated);
    transform: translateY(-2px);
  }

  .mini-cover {
    width: 3.1rem;
    flex-shrink: 0;
  }

  .mini-text {
    display: flex;
    flex-direction: column;
    gap: 0.2rem;
    min-width: 0;
    flex: 1;
  }

  .mini-title {
    font-family: "Literata Variable", Georgia, serif;
    font-weight: 600;
    font-size: 0.92rem;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .mini-author {
    font-size: 0.78rem;
    color: var(--muted);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .mini-track {
    height: 3px;
    border-radius: 999px;
    background: color-mix(in srgb, var(--text-soft) 10%, transparent);
    margin-top: 0.35rem;
    overflow: hidden;
  }

  .mini-track span {
    display: block;
    height: 100%;
    background: var(--accent-2);
  }

  .mini-meta {
    font-size: 0.72rem;
    color: var(--muted);
  }

  @media (max-width: 620px) {
    .hero {
      grid-template-columns: 6.5rem 1fr;
      align-items: start;
      border-radius: 1.3rem;
    }
  }
</style>
