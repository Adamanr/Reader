<script lang="ts">
  import { onMount } from "svelte";
  import type { BookMeta } from "$lib/types";
  import { getBookFormat } from "$lib/bookFormat";
  import { loadCover } from "$lib/covers/coverCache";
  import { coverTone, type CoverTone } from "$lib/reading/palette";
  import { bookProgress, bookTitle, dustLabel, dustLevel, effectiveStatus, hashNum } from "$lib/library/bookInfo";

  interface Props {
    paths: string[];
    books: Record<string, BookMeta>;
    onOpen: (path: string) => void;
    onMenu: (e: MouseEvent, path: string) => void;
  }
  let { paths, books, onOpen, onMenu }: Props = $props();

  let tones = $state<Record<string, CoverTone | null>>({});
  let root = $state<HTMLUListElement | null>(null);

  /** Цвет корешка — из обложки; грузим, когда корешок появляется на экране. */
  onMount(() => {
    const io = new IntersectionObserver(
      (entries) => {
        for (const e of entries) {
          if (!e.isIntersecting) continue;
          const path = (e.target as HTMLElement).dataset.path!;
          io.unobserve(e.target);
          if (path in tones) continue;
          void loadCover(path, getBookFormat(path)).then(async (url) => {
            tones = { ...tones, [path]: await coverTone(url) };
          });
        }
      },
      { rootMargin: "200px" },
    );
    const observeAll = () => {
      for (const el of Array.from(root?.querySelectorAll<HTMLElement>("[data-path]") ?? [])) io.observe(el);
    };
    observeAll();
    const mo = new MutationObserver(observeAll);
    if (root) mo.observe(root, { childList: true });
    return () => {
      io.disconnect();
      mo.disconnect();
    };
  });

  function spineStyle(path: string, meta: BookMeta | undefined): string {
    const r = hashNum(path);
    const tone = tones[path];
    const h = tone ? tone.h : Math.round(r * 360);
    const s = tone ? Math.max(18, Math.min(tone.s, 55)) : 22 + r * 18;
    const l = tone ? Math.max(24, Math.min(tone.l, 48)) : 34 + r * 12;
    const pages = meta?.lastReadPdfTotal ?? null;
    const width = pages ? Math.max(22, Math.min(60, 16 + pages / 11)) : 24 + Math.round(hashNum(path + "w") * 20);
    const height = 150 + Math.round(hashNum(path + "h") * 52);
    return [
      `--spine-h:${h}`,
      `--spine-s:${s}%`,
      `--spine-l:${l}%`,
      `--spine-w:${width}px`,
      `--spine-height:${height}px`,
      `--dust:${dustLevel(meta).toFixed(2)}`,
    ].join(";");
  }
</script>

<ul class="shelf" bind:this={root}>
  {#each paths as p (p)}
    {@const meta = books[p]}
    {@const status = effectiveStatus(meta)}
    {@const prog = bookProgress(meta)}
    {@const dust = dustLabel(meta)}
    <li class="slot" data-path={p}>
      <button
        type="button"
        class="spine"
        class:reading={status === "reading"}
        class:done={status === "done"}
        class:dusty={dustLevel(meta) > 0.02}
        style={spineStyle(p, meta)}
        title={`${bookTitle(p, meta)}${meta?.author ? " — " + meta.author : ""}${dust ? "\n" + dust : ""}`}
        onclick={() => onOpen(p)}
        oncontextmenu={(e) => {
          e.preventDefault();
          onMenu(e, p);
        }}
      >
        <span class="spine-band top" aria-hidden="true"></span>
        <span class="spine-title">{bookTitle(p, meta)}</span>
        {#if meta?.author?.trim()}
          <span class="spine-author">{meta.author.split(/[,;]/)[0]?.trim().split(" ").pop()}</span>
        {/if}
        <span class="spine-band bottom" aria-hidden="true"></span>
        {#if status === "reading" && prog != null}
          <span class="ribbon" style:--p={prog} aria-hidden="true"></span>
        {/if}
      </button>
    </li>
  {/each}
</ul>

<style>
  .shelf {
    --row: 212px;
    --board: 14px;
    --gap: 26px;
    list-style: none;
    margin: 0;
    padding: 0 0.6rem 2rem;
    display: flex;
    flex-wrap: wrap;
    align-content: flex-start;
    column-gap: 3px;
    row-gap: calc(var(--board) + var(--gap));
    background-image: linear-gradient(
      to bottom,
      transparent var(--row),
      color-mix(in srgb, var(--text-soft) 18%, var(--panel-soft)) var(--row),
      color-mix(in srgb, var(--text-soft) 30%, var(--panel-soft)) calc(var(--row) + var(--board)),
      transparent calc(var(--row) + var(--board))
    );
    background-size: 100% calc(var(--row) + var(--board) + var(--gap));
    background-repeat: repeat-y;
  }

  .slot {
    height: var(--row);
    display: flex;
    align-items: flex-end;
  }

  .spine {
    position: relative;
    width: var(--spine-w);
    height: var(--spine-height);
    padding: 0.7rem 0 0.6rem;
    border: none;
    border-radius: 3px 3px 1px 1px;
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 0.4rem;
    cursor: pointer;
    color: hsl(var(--spine-h) 30% 94%);
    background:
      linear-gradient(
        90deg,
        rgba(0, 0, 0, 0.22),
        rgba(255, 255, 255, 0.1) 18%,
        rgba(255, 255, 255, 0) 40%,
        rgba(0, 0, 0, 0.12) 85%,
        rgba(0, 0, 0, 0.3)
      ),
      hsl(var(--spine-h) var(--spine-s) var(--spine-l));
    /* Тени без размытия — размытые на каждом корешке тормозят прокрутку. */
    box-shadow:
      inset 0 -2px 0 rgba(0, 0, 0, 0.18),
      1px 0 0 rgba(0, 0, 0, 0.14);
    transform-origin: bottom center;
    transition: transform 0.25s cubic-bezier(0.33, 1, 0.68, 1);
    overflow: hidden;
  }

  .spine.dusty {
    filter: grayscale(calc(var(--dust) * 0.75)) sepia(calc(var(--dust) * 0.35)) brightness(calc(1 - var(--dust) * 0.1));
  }

  .spine.dusty::after {
    content: "";
    position: absolute;
    inset: 0;
    pointer-events: none;
    opacity: calc(var(--dust) * 0.8);
    background-image: url("data:image/svg+xml;utf8,<svg xmlns='http://www.w3.org/2000/svg' width='60' height='60'><filter id='n'><feTurbulence type='fractalNoise' baseFrequency='1.4' numOctaves='2'/><feColorMatrix values='0 0 0 0 0.85  0 0 0 0 0.82  0 0 0 0 0.76  0 0 0 0.55 0'/></filter><rect width='60' height='60' filter='url(%23n)'/></svg>");
    transition: opacity 0.6s ease;
  }

  .spine:hover,
  .spine:focus-visible {
    transform: translateY(-12px);
    filter: none;
  }

  .spine:hover::after {
    opacity: 0;
  }

  .spine-band {
    width: 100%;
    height: 3px;
    background: hsl(var(--spine-h) 40% 80% / 0.55);
    flex-shrink: 0;
  }

  .spine-band.bottom {
    margin-top: auto;
  }

  .spine-title {
    writing-mode: vertical-rl;
    transform: rotate(180deg);
    font-family: "Literata Variable", Georgia, serif;
    font-size: 0.74rem;
    font-weight: 600;
    line-height: 1.05;
    max-height: calc(var(--spine-height) - 4.6rem);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    letter-spacing: 0.02em;
  }

  .spine-author {
    writing-mode: vertical-rl;
    transform: rotate(180deg);
    font-size: 0.6rem;
    opacity: 0.75;
    max-height: 3.5rem;
    overflow: hidden;
    white-space: nowrap;
  }

  .ribbon {
    position: absolute;
    top: -2px;
    right: 22%;
    width: 5px;
    height: calc(14px + var(--p) * 40px);
    background: var(--accent);
    clip-path: polygon(0 0, 100% 0, 100% 100%, 50% 82%, 0 100%);
    box-shadow: 0 1px 2px rgba(0, 0, 0, 0.3);
  }

  .spine.done .spine-band {
    background: hsl(45 80% 70% / 0.8);
  }

  @media (max-width: 600px) {
    .shelf {
      --row: 180px;
    }
    .spine {
      height: calc(var(--spine-height) * 0.84);
    }
  }
</style>
