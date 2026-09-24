<script lang="ts">
  import { hashNum } from "$lib/library/bookInfo";

  /**
   * Обложка для книги без картинки: спокойная типографская композиция
   * в цвете, стабильном для каждой книги.
   */
  interface Props {
    seed: string;
    title: string;
    author?: string;
    format?: string;
    compact?: boolean;
  }
  let { seed, title, author = "", format = "", compact = false }: Props = $props();

  const PALETTES = [
    ["#2f3a56", "#e8d9b5"],
    ["#5a2e3a", "#f1d6c8"],
    ["#28463d", "#dfe8cf"],
    ["#3b3355", "#e9dcf5"],
    ["#6b4a2b", "#f5e6c8"],
    ["#1f4a5c", "#d6ecf0"],
    ["#4d3b2f", "#efe2d0"],
    ["#553a5c", "#f3d9e6"],
  ];

  const pal = $derived(PALETTES[Math.floor(hashNum(seed) * PALETTES.length)]!);
  const motif = $derived(Math.floor(hashNum(seed + "m") * 4));
  const cleanTitle = $derived(title.replace(/\.(pdf|epub|fb2|typ)$/i, "").replace(/[_]+/g, " "));
</script>

<div class="art" class:compact style:--bg={pal[0]} style:--ink={pal[1]}>
  <svg class="motif" viewBox="0 0 100 140" preserveAspectRatio="none" aria-hidden="true">
    {#if motif === 0}
      <circle cx="78" cy="30" r="22" fill="var(--ink)" opacity="0.12" />
      <circle cx="78" cy="30" r="12" fill="var(--ink)" opacity="0.14" />
    {:else if motif === 1}
      <path d="M0 112 Q25 96 50 112 T100 112 V140 H0Z" fill="var(--ink)" opacity="0.12" />
      <path d="M0 122 Q25 108 50 122 T100 122 V140 H0Z" fill="var(--ink)" opacity="0.12" />
    {:else if motif === 2}
      <rect x="10" y="10" width="80" height="120" fill="none" stroke="var(--ink)" stroke-opacity="0.35" stroke-width="0.8" />
      <rect x="14" y="14" width="72" height="112" fill="none" stroke="var(--ink)" stroke-opacity="0.18" stroke-width="0.5" />
    {:else}
      <path d="M50 18 L54 30 L66 30 L56 38 L60 50 L50 42 L40 50 L44 38 L34 30 L46 30Z" fill="var(--ink)" opacity="0.16" />
    {/if}
  </svg>
  <div class="text">
    <span class="title">{cleanTitle}</span>
    {#if author && !compact}<span class="author">{author}</span>{/if}
  </div>
  {#if format}<span class="fmt">{format}</span>{/if}
</div>

<style>
  .art {
    position: relative;
    width: 100%;
    height: 100%;
    display: flex;
    flex-direction: column;
    justify-content: flex-end;
    padding: 12% 11% 13%;
    box-sizing: border-box;
    background:
      radial-gradient(120% 80% at 20% 0%, rgba(255, 255, 255, 0.12), transparent 60%),
      var(--bg);
    color: var(--ink);
    overflow: hidden;
    container-type: inline-size;
  }

  .motif {
    position: absolute;
    inset: 0;
    width: 100%;
    height: 100%;
  }

  .text {
    position: relative;
    display: flex;
    flex-direction: column;
    gap: 0.5em;
  }

  .title {
    font-family: "Literata Variable", Georgia, serif;
    font-weight: 600;
    font-size: clamp(0.62rem, 11cqi, 1.5rem);
    line-height: 1.15;
    display: -webkit-box;
    -webkit-line-clamp: 5;
    line-clamp: 5;
    -webkit-box-orient: vertical;
    overflow: hidden;
    overflow-wrap: anywhere;
    text-wrap: balance;
  }

  .author {
    font-size: clamp(0.5rem, 6.5cqi, 0.85rem);
    letter-spacing: 0.04em;
    opacity: 0.75;
  }

  .fmt {
    position: absolute;
    top: 8%;
    left: 11%;
    font-size: clamp(0.45rem, 5cqi, 0.62rem);
    letter-spacing: 0.2em;
    text-transform: uppercase;
    opacity: 0.55;
  }

  .compact .title {
    -webkit-line-clamp: 3;
    line-clamp: 3;
  }
</style>
