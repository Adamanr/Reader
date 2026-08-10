<script lang="ts">
  import type { Snippet } from "svelte";
  import { onMount } from "svelte";
  import { isTauriRuntime } from "$lib/isTauri";
  import { getBookFormat } from "$lib/bookFormat";
  import { enqueueCoverGeneration } from "$lib/library/coverLoadQueue";

  interface Props {
    bookPath: string;
    /** Уже сохранённое превью из метаданных */
    cachedUrl?: string | null;
    format: ReturnType<typeof getBookFormat>;
    /** Сохранить миниатюру в метаданные (родитель вызывает patchBook) */
    onCached?: (dataUrl: string) => void;
    children?: Snippet;
  }
  let { bookPath, cachedUrl = null, format, onCached, children }: Props = $props();

  let root: HTMLDivElement | undefined = $state(undefined);
  let visible = $state(false);
  let src = $state<string | null>(null);
  let tried = $state(false);

  $effect(() => {
    if (cachedUrl && visible) {
      src = cachedUrl;
    }
  });

  onMount(() => {
    if (!cachedUrl && (!format || format === "epub" || format === "typst" || !isTauriRuntime())) {
      tried = true;
      return;
    }
    const el = root;
    if (!el) return;
    let idleHandle: number | null = null;
    let frameHandle: number | null = null;
    const reveal = () => {
      visible = true;
    };
    const io = new IntersectionObserver(
      (entries) => {
        if (entries.some((e) => e.isIntersecting)) {
          if ("requestIdleCallback" in window) {
            idleHandle = window.requestIdleCallback(reveal, { timeout: 220 });
          } else {
            frameHandle = requestAnimationFrame(reveal);
          }
          io.disconnect();
        }
      },
      { root: null, rootMargin: "100px 0px", threshold: 0.01 },
    );
    io.observe(el);
    return () => {
      io.disconnect();
      if (idleHandle != null && "cancelIdleCallback" in window) window.cancelIdleCallback(idleHandle);
      if (frameHandle != null) cancelAnimationFrame(frameHandle);
    };
  });

  $effect(() => {
    if (cachedUrl) return;
    if (!visible || tried) return;
    if (!format || format === "epub" || format === "typst") {
      tried = true;
      return;
    }
    if (!isTauriRuntime()) {
      tried = true;
      return;
    }
    tried = true;
    void enqueueCoverGeneration(async () => {
      try {
        const { buildBookCoverDataUrl } = await import("$lib/covers/bookCover");
        const u = await buildBookCoverDataUrl(bookPath, format);
        if (u && u.length < 120_000) {
          src = u;
          onCached?.(u);
        }
      } catch {
        /* обложка необязательна */
      }
    });
  });
</script>

<div class="thumb-root" bind:this={root}>
  {#if src}
    <img class="thumb-img" src={src} alt="" loading="lazy" decoding="async" />
  {:else}
    <span class="thumb-fallback">{@render children?.()}</span>
  {/if}
</div>

<style>
  /* Локальные стили миниатюры: блок намеренно остаётся обычным CSS для стабильного HMR. */
  .thumb-root {
    width: 100%;
    height: 100%;
    min-height: 0;
    border-radius: inherit;
  }
  .thumb-img {
    width: 100%;
    height: 100%;
    object-fit: cover;
    display: block;
    border-radius: inherit;
  }
  .thumb-fallback {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 100%;
    height: 100%;
  }
</style>
