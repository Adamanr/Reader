<script lang="ts">
  import { onDestroy, onMount } from "svelte";
  import type { EpubReaderApi, ReadingUnit } from "$lib/types";
  import type { PagePalette } from "$lib/reading/palette";
  import { reading, updateReading } from "$lib/reading/settings.svelte";
  import { fontStack } from "$lib/reading/fonts";

  /**
   * Быстрое чтение (RSVP): слова появляются по одному в одной точке,
   * глаз не бегает по строке. Опорная буква подсвечена.
   */
  interface Props {
    api: EpubReaderApi;
    palette: PagePalette;
    onClose: () => void;
  }
  let { api, palette, onClose }: Props = $props();

  type Word = { w: string; unit: number; end: boolean };
  let units: ReadingUnit[] = [];
  let words = $state<Word[]>([]);
  let idx = $state(0);
  let playing = $state(false);
  let loading = $state(true);
  let finished = $state(false);
  let timer: ReturnType<typeof setTimeout> | null = null;

  function explode(list: ReadingUnit[]): Word[] {
    const out: Word[] = [];
    list.forEach((u, ui) => {
      const ws = u.text.split(/\s+/).filter(Boolean);
      ws.forEach((w, k) => out.push({ w, unit: ui, end: k === ws.length - 1 }));
    });
    return out;
  }

  async function loadFromHere() {
    loading = true;
    units = (await api.unitsFromHere?.()) ?? [];
    words = explode(units);
    idx = 0;
    loading = false;
  }

  onMount(() => {
    void loadFromHere().then(() => {
      if (words.length) play();
    });
  });

  onDestroy(() => {
    if (timer) clearTimeout(timer);
  });

  function delayFor(w: Word): number {
    const base = 60000 / reading.s.rsvpWpm;
    let k = 1;
    if (/[.!?…]["»”)]*$/.test(w.w)) k = 2.3;
    else if (/[,;:—–]$/.test(w.w)) k = 1.5;
    if (w.w.length > 9) k *= 1.3;
    if (w.end) k += 1.2;
    return base * k;
  }

  function step() {
    if (!playing) return;
    if (idx >= words.length - 1) {
      void nextChapter();
      return;
    }
    idx++;
    timer = setTimeout(step, delayFor(words[idx]!));
  }

  async function nextChapter() {
    playing = false;
    const moved = await api.advanceChapter?.();
    if (!moved) {
      finished = true;
      return;
    }
    await new Promise((r) => setTimeout(r, 300));
    await loadFromHere();
    if (words.length) play();
    else finished = true;
  }

  function play() {
    if (!words.length) return;
    finished = false;
    playing = true;
    if (timer) clearTimeout(timer);
    timer = setTimeout(step, delayFor(words[idx]!));
  }

  function pause() {
    playing = false;
    if (timer) clearTimeout(timer);
  }

  function jump(delta: number) {
    idx = Math.max(0, Math.min(words.length - 1, idx + delta));
    if (playing) play();
  }

  function setWpm(v: number) {
    updateReading({ rsvpWpm: Math.max(100, Math.min(1000, Math.round(v / 10) * 10)) });
  }

  async function close() {
    pause();
    // Возвращаемся в книгу к абзацу, на котором остановились.
    const w = words[idx];
    if (w) await units[w.unit]?.reveal?.();
    onClose();
  }

  function onKey(e: KeyboardEvent) {
    e.stopPropagation();
    if (e.key === " ") {
      e.preventDefault();
      if (playing) pause();
      else play();
    } else if (e.key === "ArrowLeft") {
      e.preventDefault();
      jump(-10);
    } else if (e.key === "ArrowRight") {
      e.preventDefault();
      jump(10);
    } else if (e.key === "ArrowUp") {
      e.preventDefault();
      setWpm(reading.s.rsvpWpm + 25);
    } else if (e.key === "ArrowDown") {
      e.preventDefault();
      setWpm(reading.s.rsvpWpm - 25);
    } else if (e.key === "Escape") {
      e.preventDefault();
      void close();
    }
  }

  function orp(len: number): number {
    if (len <= 1) return 0;
    if (len <= 5) return 1;
    if (len <= 9) return 2;
    if (len <= 13) return 3;
    return 4;
  }

  const cur = $derived(words[idx]?.w ?? "");
  const pivot = $derived.by(() => {
    // Опорная буква считается по буквам, без кавычек и скобок в начале.
    const lead = cur.match(/^[^\p{L}\p{N}]*/u)?.[0].length ?? 0;
    const core = cur.slice(lead).replace(/[^\p{L}\p{N}]+$/u, "");
    const p = lead + Math.min(orp(core.length), Math.max(0, core.length - 1));
    return { left: cur.slice(0, p), mid: cur.charAt(p), right: cur.slice(p + 1) };
  });
  const contextUnit = $derived(words[idx] ? units[words[idx]!.unit]?.text ?? "" : "");
  const progress = $derived(words.length ? (idx + 1) / words.length : 0);
  const minutesLeft = $derived(Math.max(0, words.length - idx) / reading.s.rsvpWpm);
</script>

<svelte:window onkeydown={onKey} />

<div
  class="rsvp"
  style:--bg={palette.bg}
  style:--fg={palette.text}
  style:--muted={palette.muted}
  style:--acc={palette.accent}
  style:--font={fontStack(reading.s.font === "publisher" ? "literata" : reading.s.font)}
  role="dialog"
  aria-modal="true"
  aria-label="Быстрое чтение"
>
  <header class="top">
    <span class="title">Быстрое чтение</span>
    <span class="grow"></span>
    <label class="wpm">
      <input type="range" min="100" max="1000" step="10" value={reading.s.rsvpWpm} oninput={(e) => setWpm(+e.currentTarget.value)} />
      <b>{reading.s.rsvpWpm}</b> слов/мин
    </label>
    <button type="button" class="x" aria-label="Закрыть" onclick={() => void close()}>×</button>
  </header>

  <!-- svelte-ignore a11y_click_events_have_key_events -->
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <div class="stage" onclick={() => (playing ? pause() : play())}>
    {#if loading}
      <p class="hint">Готовлю текст…</p>
    {:else if finished}
      <p class="hint">Книга дочитана до конца. ✦</p>
    {:else if !words.length}
      <p class="hint">Здесь нет текста для быстрого чтения.</p>
    {:else}
      <div class="guide top-g" aria-hidden="true"></div>
      <div class="word" aria-live="off">
        <span class="l">{pivot.left}</span><span class="m">{pivot.mid}</span><span class="r">{pivot.right}</span>
      </div>
      <div class="guide bottom-g" aria-hidden="true"></div>
      {#if !playing}
        <p class="context">{contextUnit}</p>
        <p class="hint small">Пробел — продолжить · ← → — 10 слов · ↑ ↓ — скорость · Esc — вернуться в книгу</p>
      {/if}
    {/if}
  </div>

  <footer class="bottom">
    <div class="bar"><div style:width="{progress * 100}%"></div></div>
    <span>≈ {Math.ceil(minutesLeft)} мин до конца главы в этом темпе</span>
  </footer>
</div>

<style>
  .rsvp {
    position: fixed;
    inset: 0;
    z-index: 900;
    display: flex;
    flex-direction: column;
    background: var(--bg);
    color: var(--fg);
    animation: in 0.25s ease-out;
  }

  .top {
    display: flex;
    align-items: center;
    gap: 1rem;
    padding: 0.8rem 1.2rem;
    color: var(--muted);
    font-size: 0.84rem;
  }

  .title {
    font-weight: 650;
  }

  .grow {
    flex: 1;
  }

  .wpm {
    display: flex;
    align-items: center;
    gap: 0.5rem;
  }

  .wpm b {
    color: var(--fg);
    font-variant-numeric: tabular-nums;
  }

  .wpm input {
    accent-color: var(--acc);
  }

  .x {
    border: none;
    background: transparent;
    color: var(--muted);
    font-size: 1.6rem;
    cursor: pointer;
    line-height: 1;
  }

  .stage {
    flex: 1;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 0.4rem;
    cursor: pointer;
    user-select: none;
    padding: 0 1rem;
  }

  .word {
    display: grid;
    grid-template-columns: 1fr auto 1fr;
    width: min(90vw, 48rem);
    font-family: var(--font);
    font-size: clamp(2rem, 6vw, 3.6rem);
    line-height: 1.2;
    white-space: pre;
  }

  .word .l {
    text-align: right;
  }

  .word .m {
    color: var(--acc);
    font-weight: 700;
  }

  .word .r {
    text-align: left;
  }

  .guide {
    width: 2px;
    height: 1.1rem;
    background: color-mix(in srgb, var(--acc) 60%, transparent);
  }

  .context {
    margin: 2rem 0 0;
    max-width: 40rem;
    font-family: var(--font);
    font-size: 1rem;
    line-height: 1.6;
    color: var(--muted);
    text-align: center;
  }

  .hint {
    color: var(--muted);
  }

  .hint.small {
    font-size: 0.78rem;
    margin-top: 1rem;
  }

  .bottom {
    display: flex;
    flex-direction: column;
    gap: 0.4rem;
    padding: 0 1.2rem max(1rem, env(safe-area-inset-bottom));
    font-size: 0.74rem;
    color: var(--muted);
  }

  .bar {
    height: 3px;
    border-radius: 3px;
    background: color-mix(in srgb, var(--muted) 25%, transparent);
    overflow: hidden;
  }

  .bar div {
    height: 100%;
    background: var(--acc);
    transition: width 0.2s linear;
  }

  @keyframes in {
    from {
      opacity: 0;
    }
  }

  @media (max-width: 600px) {
    .wpm input {
      width: 6rem;
    }
  }
</style>
