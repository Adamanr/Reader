<script lang="ts">
  import { onMount } from "svelte";
  import { goto } from "$app/navigation";
  import type { LibrarySnapshot } from "$lib/types";
  import { fetchLibrarySnapshot, getCachedLibrarySnapshot } from "$lib/library/librarySnapshotCache";
  import { dayKey, formatMinutes, loadStats, stats, streak } from "$lib/reading/stats.svelte";
  import { bookProgress, bookTitle, effectiveStatus } from "$lib/library/bookInfo";

  let snapshot = $state<LibrarySnapshot | null>(getCachedLibrarySnapshot());

  onMount(() => {
    void loadStats();
    if (!snapshot) void fetchLibrarySnapshot().then((s) => (snapshot = s)).catch(() => {});
  });

  const WEEKS = 26;

  /** Сетка «как на GitHub»: столбцы — недели, строки — дни (пн…вс). */
  const grid = $derived.by(() => {
    const today = new Date();
    const dow = (today.getDay() + 6) % 7;
    const start = new Date(today);
    start.setDate(today.getDate() - dow - (WEEKS - 1) * 7);
    const cols: { key: string; min: number; future: boolean; label: string }[][] = [];
    const d = new Date(start);
    for (let w = 0; w < WEEKS; w++) {
      const col = [];
      for (let i = 0; i < 7; i++) {
        const key = dayKey(d);
        col.push({
          key,
          min: (stats.s.days[key]?.ms ?? 0) / 60000,
          future: d > today,
          label: d.toLocaleDateString("ru", { day: "numeric", month: "long", weekday: "short" }),
        });
        d.setDate(d.getDate() + 1);
      }
      cols.push(col);
    }
    return cols;
  });

  const months = $derived.by(() => {
    const out: { idx: number; label: string }[] = [];
    let last = -1;
    grid.forEach((col, i) => {
      const m = Number(col[0]!.key.slice(5, 7));
      if (m !== last) {
        // Первая неполная неделя месяца не должна наезжать на соседнюю подпись.
        if (out.length && i - out[out.length - 1]!.idx < 3) out.pop();
        out.push({ idx: i, label: new Date(2000, m - 1, 1).toLocaleDateString("ru", { month: "short" }) });
        last = m;
      }
    });
    return out;
  });

  function level(min: number): number {
    if (min < 1) return 0;
    if (min < 10) return 1;
    if (min < 25) return 2;
    if (min < 50) return 3;
    return 4;
  }

  const totals = $derived.by(() => {
    const days = stats.s.days;
    const now = new Date();
    let week = 0;
    let month = 0;
    let all = 0;
    let chars = 0;
    let pages = 0;
    for (const [k, v] of Object.entries(days)) {
      all += v.ms;
      chars += v.chars;
      pages += v.pages;
      const d = new Date(k + "T12:00:00");
      const diff = (now.getTime() - d.getTime()) / 86_400_000;
      if (diff < 7) week += v.ms;
      if (diff < 30) month += v.ms;
    }
    return {
      today: (days[dayKey()]?.ms ?? 0) / 60000,
      week: week / 60000,
      month: month / 60000,
      all: all / 60000,
      chars,
      pages,
      activeDays: Object.values(days).filter((d) => d.ms >= 60_000).length,
    };
  });

  const wpm = $derived(Math.round(stats.s.speed.charsPerMin / 6.3));
  const run = $derived(streak(stats.s.days));

  const bookRows = $derived.by(() => {
    const books = snapshot?.metadata.books ?? {};
    return Object.entries(stats.s.books)
      .map(([path, b]) => ({ path, ms: b.ms, sessions: b.sessions, meta: books[path] }))
      .filter((r) => r.meta)
      .sort((a, b) => b.ms - a.ms)
      .slice(0, 12);
  });
  const maxBookMs = $derived(Math.max(1, ...bookRows.map((r) => r.ms)));

  const finishedThisYear = $derived.by(() => {
    const y = String(new Date().getFullYear());
    return Object.entries(snapshot?.metadata.books ?? {})
      .filter(([, m]) => effectiveStatus(m) === "done" && (m.finishedAt ?? "").startsWith(y))
      .map(([path, m]) => ({ path, meta: m }))
      .sort((a, b) => (b.meta.finishedAt ?? "").localeCompare(a.meta.finishedAt ?? ""));
  });

  /** По часам суток: когда вы чаще читаете (по последним сессиям не храним — показываем дни недели). */
  const byWeekday = $derived.by(() => {
    const sums = new Array(7).fill(0);
    for (const [k, v] of Object.entries(stats.s.days)) {
      const d = new Date(k + "T12:00:00");
      sums[(d.getDay() + 6) % 7] += v.ms / 60000;
    }
    return sums;
  });
  const maxWeekday = $derived(Math.max(1, ...byWeekday));
  const WD = ["пн", "вт", "ср", "чт", "пт", "сб", "вс"];
</script>

<div class="st">
  <header class="st-top">
    <button type="button" class="back" onclick={() => goto("/")}>← Библиотека</button>
    <div>
      <p class="kicker">без гонки и очков</p>
      <h1>Ритм чтения</h1>
    </div>
  </header>

  <main class="st-main">
    <section class="tiles">
      <div class="tile">
        <span class="tile-label">Сегодня</span>
        <span class="tile-val">{totals.today >= 1 ? formatMinutes(totals.today) : "—"}</span>
      </div>
      <div class="tile">
        <span class="tile-label">За неделю</span>
        <span class="tile-val">{totals.week >= 1 ? formatMinutes(totals.week) : "—"}</span>
      </div>
      <div class="tile">
        <span class="tile-label">Дней подряд</span>
        <span class="tile-val">{run || "—"}</span>
      </div>
      <div class="tile">
        <span class="tile-label">Всего</span>
        <span class="tile-val">{totals.all >= 1 ? formatMinutes(totals.all) : "—"}</span>
        <span class="tile-sub">{totals.activeDays} дн. с книгой</span>
      </div>
      <div class="tile">
        <span class="tile-label">Ваш темп</span>
        <span class="tile-val">~{wpm} <small>слов/мин</small></span>
        <span class="tile-sub">{stats.s.speed.samples > 0 ? "по вашим сессиям" : "пока средний"}</span>
      </div>
      <div class="tile">
        <span class="tile-label">Дочитано в {new Date().getFullYear()}</span>
        <span class="tile-val">{finishedThisYear.length}</span>
      </div>
    </section>

    <section class="card">
      <h2>Полгода чтения</h2>
      <div class="heat-wrap">
        <div class="heat-days" aria-hidden="true">
          {#each WD as d, i (d)}
            <span>{i % 2 === 0 ? d : ""}</span>
          {/each}
        </div>
        <div class="heat">
          <div class="heat-months" aria-hidden="true">
            {#each months as m (m.idx)}
              <span style:grid-column={m.idx + 1}>{m.label}</span>
            {/each}
          </div>
          <div class="heat-grid">
            {#each grid as col, ci (ci)}
              <div class="heat-col">
                {#each col as cell (cell.key)}
                  <span
                    class="heat-cell l{level(cell.min)}"
                    class:future={cell.future}
                    title="{cell.label}: {cell.min >= 1 ? formatMinutes(cell.min) : 'нет чтения'}"
                  ></span>
                {/each}
              </div>
            {/each}
          </div>
        </div>
      </div>
      <div class="legend" aria-hidden="true">
        меньше <span class="heat-cell l0"></span><span class="heat-cell l1"></span><span class="heat-cell l2"></span><span class="heat-cell l3"></span><span class="heat-cell l4"></span> больше
      </div>
    </section>

    <div class="cols">
      <section class="card">
        <h2>Время с книгами</h2>
        {#if bookRows.length === 0}
          <p class="muted">Откройте книгу — здесь появится, сколько времени вы с ней провели.</p>
        {:else}
          <ul class="books">
            {#each bookRows as r (r.path)}
              <li>
                <button type="button" class="book-row" onclick={() => goto("/read?path=" + encodeURIComponent(r.path))}>
                  <span class="book-name">{bookTitle(r.path, r.meta)}</span>
                  <span class="book-bar"><span style:width="{(r.ms / maxBookMs) * 100}%"></span></span>
                  <span class="book-time">{formatMinutes(r.ms / 60000)}</span>
                  <span class="book-meta">
                    {r.sessions} сесс.{#if bookProgress(r.meta) != null} · {Math.round((bookProgress(r.meta) ?? 0) * 100)}%{/if}
                  </span>
                </button>
              </li>
            {/each}
          </ul>
        {/if}
      </section>

      <section class="card">
        <h2>Дни недели</h2>
        <div class="wd">
          {#each byWeekday as v, i (i)}
            <div class="wd-col">
              <span class="wd-bar" style:--h={v / maxWeekday}></span>
              <span class="wd-label">{WD[i]}</span>
            </div>
          {/each}
        </div>
        {#if finishedThisYear.length}
          <h2 class="mt">Дочитано в этом году</h2>
          <ul class="done">
            {#each finishedThisYear as b (b.path)}
              <li>
                <span>{bookTitle(b.path, b.meta)}</span>
                <small>{new Date(b.meta.finishedAt!).toLocaleDateString("ru", { day: "numeric", month: "short" })}</small>
              </li>
            {/each}
          </ul>
        {/if}
      </section>
    </div>
  </main>
</div>

<style>
  .st {
    min-height: 100dvh;
    background:
      radial-gradient(ellipse 50rem 20rem at 20% -5rem, color-mix(in srgb, var(--accent) 22%, transparent), transparent 70%),
      var(--bg-soft);
    color: var(--text-soft);
  }

  .st-top {
    display: flex;
    align-items: flex-end;
    gap: 1.5rem;
    padding: clamp(1rem, 3vw, 1.8rem) clamp(1rem, 5vw, 3rem) 1rem;
    max-width: 72rem;
    margin: 0 auto;
  }

  .back {
    border: 1px solid var(--border-soft);
    background: var(--panel-elevated);
    color: var(--text-soft);
    border-radius: 999px;
    padding: 0.45rem 0.9rem;
    cursor: pointer;
  }

  .kicker {
    margin: 0;
    font-size: 0.7rem;
    text-transform: uppercase;
    letter-spacing: 0.16em;
    color: var(--muted);
  }

  h1 {
    margin: 0.2rem 0 0;
    font-family: "Literata Variable", Georgia, serif;
    font-weight: 500;
    font-size: clamp(1.8rem, 4vw, 2.6rem);
  }

  .st-main {
    max-width: 72rem;
    margin: 0 auto;
    padding: 0.5rem clamp(1rem, 5vw, 3rem) 3rem;
    display: flex;
    flex-direction: column;
    gap: 1.2rem;
  }

  .tiles {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(10rem, 1fr));
    gap: 0.8rem;
  }

  .tile {
    display: flex;
    flex-direction: column;
    gap: 0.25rem;
    padding: 0.9rem 1rem;
    border-radius: var(--radius-lg);
    background: var(--panel-elevated);
    border: 1px solid color-mix(in srgb, var(--border-soft) 75%, transparent);
    box-shadow: var(--shadow-soft);
  }

  .tile-label {
    font-size: 0.72rem;
    color: var(--muted);
    text-transform: uppercase;
    letter-spacing: 0.1em;
  }

  .tile-val {
    font-family: "Literata Variable", Georgia, serif;
    font-size: 1.5rem;
  }

  .tile-val small {
    font-size: 0.8rem;
    color: var(--muted);
  }

  .tile-sub {
    font-size: 0.72rem;
    color: var(--muted);
  }

  .card {
    padding: 1.1rem 1.2rem;
    border-radius: var(--radius-lg);
    background: var(--panel-elevated);
    border: 1px solid color-mix(in srgb, var(--border-soft) 75%, transparent);
    box-shadow: var(--shadow-soft);
    min-width: 0;
  }

  h2 {
    margin: 0 0 0.9rem;
    font-size: 0.78rem;
    text-transform: uppercase;
    letter-spacing: 0.12em;
    color: var(--muted);
  }

  h2.mt {
    margin-top: 1.4rem;
  }

  .heat-wrap {
    display: flex;
    gap: 0.4rem;
    overflow-x: auto;
    padding-bottom: 0.3rem;
  }

  .heat-days {
    display: grid;
    grid-template-rows: repeat(7, 13px);
    gap: 3px;
    margin-top: 1.2rem;
    font-size: 0.62rem;
    color: var(--muted);
  }

  .heat-months {
    display: grid;
    grid-template-columns: repeat(26, 13px);
    column-gap: 3px;
    font-size: 0.62rem;
    color: var(--muted);
    height: 1.2rem;
  }

  .heat-months span {
    white-space: nowrap;
  }

  .heat-grid {
    display: flex;
    gap: 3px;
  }

  .heat-col {
    display: grid;
    grid-template-rows: repeat(7, 13px);
    gap: 3px;
  }

  .heat-cell {
    display: inline-block;
    width: 13px;
    height: 13px;
    border-radius: 3px;
    background: color-mix(in srgb, var(--border-soft) 70%, transparent);
  }

  .heat-cell.l1 {
    background: color-mix(in srgb, var(--accent) 35%, var(--border-soft));
  }
  .heat-cell.l2 {
    background: color-mix(in srgb, var(--accent) 65%, var(--border-soft));
  }
  .heat-cell.l3 {
    background: var(--accent);
  }
  .heat-cell.l4 {
    background: var(--accent-2);
  }
  .heat-cell.future {
    opacity: 0.25;
  }

  .legend {
    display: flex;
    align-items: center;
    gap: 3px;
    justify-content: flex-end;
    margin-top: 0.6rem;
    font-size: 0.68rem;
    color: var(--muted);
  }

  .cols {
    display: grid;
    grid-template-columns: 1.4fr 1fr;
    gap: 1.2rem;
  }

  .books {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 0.3rem;
  }

  .book-row {
    width: 100%;
    display: grid;
    grid-template-columns: minmax(0, 1fr) 7rem auto;
    grid-template-rows: auto auto;
    column-gap: 0.8rem;
    align-items: center;
    padding: 0.45rem 0.55rem;
    border: none;
    border-radius: var(--radius-sm);
    background: transparent;
    color: var(--text-soft);
    text-align: left;
    cursor: pointer;
  }

  .book-row:hover {
    background: var(--panel-soft);
  }

  .book-name {
    font-size: 0.88rem;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .book-bar {
    height: 6px;
    border-radius: 999px;
    background: color-mix(in srgb, var(--border-soft) 70%, transparent);
    overflow: hidden;
  }

  .book-bar span {
    display: block;
    height: 100%;
    border-radius: inherit;
    background: linear-gradient(90deg, var(--accent), var(--accent-2));
  }

  .book-time {
    font-size: 0.8rem;
    font-variant-numeric: tabular-nums;
    color: var(--muted);
    text-align: right;
  }

  .book-meta {
    grid-column: 1 / -1;
    font-size: 0.7rem;
    color: var(--muted);
  }

  .wd {
    display: grid;
    grid-template-columns: repeat(7, 1fr);
    gap: 0.4rem;
    height: 8rem;
    align-items: end;
  }

  .wd-col {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 0.3rem;
    height: 100%;
    justify-content: flex-end;
  }

  .wd-bar {
    width: 100%;
    max-width: 2rem;
    height: calc(var(--h) * 100% - 1.2rem);
    min-height: 3px;
    border-radius: 6px 6px 2px 2px;
    background: linear-gradient(180deg, var(--accent), var(--accent-2));
  }

  .wd-label {
    font-size: 0.7rem;
    color: var(--muted);
  }

  .done {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 0.35rem;
  }

  .done li {
    display: flex;
    justify-content: space-between;
    gap: 0.6rem;
    font-size: 0.86rem;
  }

  .done small {
    color: var(--muted);
    white-space: nowrap;
  }

  .muted {
    color: var(--muted);
    font-size: 0.88rem;
  }

  @media (max-width: 860px) {
    .cols {
      grid-template-columns: 1fr;
    }
  }
</style>
