<script lang="ts">
  import { onMount } from "svelte";
  import { dayKey, formatMinutes, loadStats, stats, streak } from "$lib/reading/stats.svelte";

  onMount(() => {
    void loadStats();
  });

  const days = $derived.by(() => {
    const out: { key: string; min: number; label: string }[] = [];
    const d = new Date();
    d.setDate(d.getDate() - 13);
    for (let i = 0; i < 14; i++) {
      const key = dayKey(d);
      out.push({
        key,
        min: (stats.s.days[key]?.ms ?? 0) / 60000,
        label: d.toLocaleDateString("ru", { weekday: "short", day: "numeric" }),
      });
      d.setDate(d.getDate() + 1);
    }
    return out;
  });
  const max = $derived(Math.max(20, ...days.map((d) => d.min)));
  const today = $derived(days[days.length - 1]?.min ?? 0);
  const run = $derived(streak(stats.s.days));
</script>

<a class="rc" href="/stats" title="Ритм чтения — подробнее">
  <div class="rc-head">
    <span class="rc-title">Ритм чтения</span>
    {#if run > 0}
      <span class="rc-streak" title="Дней подряд">✦ {run}</span>
    {/if}
  </div>
  <div class="rc-bars" aria-hidden="true">
    {#each days as d (d.key)}
      <span class="rc-bar" style:--h={Math.max(0.06, d.min / max)} class:zero={d.min < 1} title="{d.label}: {Math.round(d.min)} мин"></span>
    {/each}
  </div>
  <p class="rc-foot">
    {today >= 1 ? `Сегодня ${formatMinutes(today)}` : "Сегодня ещё не читали"}
  </p>
</a>

<style>
  .rc {
    display: flex;
    flex-direction: column;
    gap: 0.55rem;
    padding: 0.85rem 0.9rem;
    border-radius: var(--radius-lg);
    border: 1px solid color-mix(in srgb, var(--border-soft) 75%, transparent);
    background: color-mix(in srgb, var(--panel-elevated) 85%, transparent);
    color: var(--text-soft);
    text-decoration: none;
    transition: transform 0.2s ease;
  }

  .rc:hover {
    transform: translateY(-2px);
  }

  .rc-head {
    display: flex;
    align-items: center;
    justify-content: space-between;
  }

  .rc-title {
    font-size: 0.72rem;
    text-transform: uppercase;
    letter-spacing: 0.13em;
    color: var(--muted);
    font-weight: 650;
  }

  .rc-streak {
    font-size: 0.78rem;
    color: var(--accent-2);
    font-weight: 650;
  }

  .rc-bars {
    display: grid;
    grid-template-columns: repeat(14, 1fr);
    align-items: end;
    gap: 3px;
    height: 2.6rem;
  }

  .rc-bar {
    height: calc(var(--h) * 100%);
    border-radius: 3px;
    background: linear-gradient(180deg, var(--accent), var(--accent-2));
  }

  .rc-bar.zero {
    background: color-mix(in srgb, var(--border-soft) 90%, transparent);
  }

  .rc-foot {
    margin: 0;
    font-size: 0.78rem;
    color: var(--muted);
  }
</style>
