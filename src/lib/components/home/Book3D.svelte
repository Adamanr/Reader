<script lang="ts">
  import type { BookMeta } from "$lib/types";
  import { formatBadgeLabel, getBookFormat } from "$lib/bookFormat";
  import BookCoverThumb from "$lib/components/BookCoverThumb.svelte";
  import CoverArt from "$lib/components/home/CoverArt.svelte";
  import { bookTitle, dustLevel } from "$lib/library/bookInfo";
  import type { DiscoveredMeta } from "$lib/covers/coverCache";

  /** Обложка в виде настоящей книги: корешок, обрез страниц, тень. */
  interface Props {
    path: string;
    meta: BookMeta | undefined;
    eager?: boolean;
    compact?: boolean;
    tilt?: boolean;
    needMeta?: boolean;
    onMeta?: (m: DiscoveredMeta) => void;
    onCover?: (url: string | null) => void;
  }
  let { path, meta, eager = false, compact = false, tilt = true, needMeta = false, onMeta, onCover }: Props = $props();

  const fmt = $derived(getBookFormat(path));
  const dust = $derived(dustLevel(meta));
</script>

<div class="book" class:tilt style:--dust={dust.toFixed(2)}>
  <div class="cover">
    <BookCoverThumb bookPath={path} format={fmt} {eager} {needMeta} {onMeta} {onCover}>
      <CoverArt
        seed={path}
        title={bookTitle(path, meta)}
        author={meta?.author ?? ""}
        format={fmt ? formatBadgeLabel(fmt) : ""}
        {compact}
      />
    </BookCoverThumb>
    <span class="spine" aria-hidden="true"></span>
    <span class="gloss" aria-hidden="true"></span>
    <span class="dust" aria-hidden="true"></span>
  </div>
  <span class="pages" aria-hidden="true"></span>
</div>

<style>
  .book {
    position: relative;
    width: 100%;
    aspect-ratio: 2 / 3;
    perspective: 900px;
  }

  .book::after {
    content: "";
    position: absolute;
    left: 8%;
    right: 2%;
    bottom: -7%;
    height: 12%;
    border-radius: 50%;
    background: radial-gradient(closest-side, rgba(20, 14, 30, 0.28), transparent);
    filter: blur(4px);
    z-index: 0;
    transition:
      transform 0.35s ease,
      opacity 0.35s ease;
  }

  .cover {
    position: absolute;
    inset: 0;
    z-index: 2;
    border-radius: 3px 7px 7px 3px;
    overflow: hidden;
    background: #d9d2c6;
    box-shadow:
      0 1px 2px rgba(20, 14, 30, 0.2),
      0 10px 24px -8px rgba(20, 14, 30, 0.35);
    transform-origin: left center;
    transition:
      transform 0.45s cubic-bezier(0.2, 0.8, 0.2, 1),
      box-shadow 0.45s ease,
      filter 0.7s ease;
    filter: grayscale(calc(var(--dust) * 0.8)) sepia(calc(var(--dust) * 0.3)) brightness(calc(1 - var(--dust) * 0.08));
  }

  .cover :global(.thumb-img) {
    border-radius: 0;
  }

  .cover :global(.thumb-fallback) {
    display: block;
  }

  .spine {
    position: absolute;
    inset: 0 auto 0 0;
    width: 9%;
    background: linear-gradient(
      90deg,
      rgba(0, 0, 0, 0.28),
      rgba(255, 255, 255, 0.18) 45%,
      rgba(0, 0, 0, 0.12) 70%,
      transparent
    );
    pointer-events: none;
  }

  .gloss {
    position: absolute;
    inset: 0;
    background: linear-gradient(115deg, rgba(255, 255, 255, 0.18), transparent 38%, transparent 70%, rgba(0, 0, 0, 0.08));
    pointer-events: none;
  }

  .dust {
    position: absolute;
    inset: 0;
    pointer-events: none;
    opacity: calc(var(--dust) * 0.85);
    background-image: url("data:image/svg+xml;utf8,<svg xmlns='http://www.w3.org/2000/svg' width='80' height='80'><filter id='n'><feTurbulence type='fractalNoise' baseFrequency='1.2' numOctaves='2'/><feColorMatrix values='0 0 0 0 0.86  0 0 0 0 0.83  0 0 0 0 0.78  0 0 0 0.6 0'/></filter><rect width='80' height='80' filter='url(%23n)'/></svg>");
    transition: opacity 0.7s ease;
  }

  /* Обрез страниц справа — виден, когда книга приоткрывается */
  .pages {
    position: absolute;
    z-index: 1;
    top: 2%;
    bottom: 2%;
    right: 0;
    width: 12%;
    border-radius: 0 4px 4px 0;
    background: repeating-linear-gradient(90deg, #fbf8f1 0 1px, #e7e0d2 1px 2px);
    box-shadow: inset 0 0 6px rgba(0, 0, 0, 0.12);
    transition: transform 0.45s cubic-bezier(0.2, 0.8, 0.2, 1);
  }

  :global(:hover > .book-hover-target) .book.tilt .cover,
  .book.tilt:hover .cover {
    transform: rotateY(-18deg) translateX(-2%);
    filter: none;
    box-shadow:
      0 1px 2px rgba(20, 14, 30, 0.2),
      18px 18px 30px -12px rgba(20, 14, 30, 0.45);
  }

  :global(:hover > .book-hover-target) .book.tilt .pages,
  .book.tilt:hover .pages {
    transform: translateX(6%);
  }

  :global(:hover > .book-hover-target) .book.tilt .dust,
  .book.tilt:hover .dust {
    opacity: 0;
  }

  :global(:hover > .book-hover-target) .book.tilt::after,
  .book.tilt:hover::after {
    transform: translateX(6%) scaleX(1.05);
    opacity: 0.8;
  }

  @media (prefers-reduced-motion: reduce) {
    .cover,
    .pages {
      transition: none;
    }
  }
</style>
