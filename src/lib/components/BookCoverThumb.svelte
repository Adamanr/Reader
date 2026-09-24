<script lang="ts">
  import type { Snippet } from "svelte";
  import { onMount } from "svelte";
  import { getBookFormat } from "$lib/bookFormat";
  import { loadCover, peekCover, type DiscoveredMeta } from "$lib/covers/coverCache";

  interface Props {
    bookPath: string;
    format: ReturnType<typeof getBookFormat>;
    /** Прочитать из файла название и автора (однократно для новой книги). */
    needMeta?: boolean;
    onMeta?: (m: DiscoveredMeta) => void;
    /** Сообщает родителю адрес обложки (для палитры и корешков). */
    onCover?: (url: string | null) => void;
    /** Грузить сразу, не дожидаясь появления на экране. */
    eager?: boolean;
    children?: Snippet;
  }
  let { bookPath, format, needMeta = false, onMeta, onCover, eager = false, children }: Props = $props();

  let root: HTMLDivElement | undefined = $state(undefined);
  let visible = $state(false);
  let src = $state<string | null>(null);

  $effect(() => {
    // Мгновенно показываем то, что уже есть в памяти.
    const known = peekCover(bookPath);
    src = known ?? null;
  });

  onMount(() => {
    if (eager) {
      visible = true;
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
      { root: null, rootMargin: "200px 0px", threshold: 0.01 },
    );
    io.observe(el);
    return () => {
      io.disconnect();
      if (idleHandle != null && "cancelIdleCallback" in window) window.cancelIdleCallback(idleHandle);
      if (frameHandle != null) cancelAnimationFrame(frameHandle);
    };
  });

  $effect(() => {
    if (!visible) return;
    const path = bookPath;
    const fmt = format;
    let alive = true;
    void loadCover(path, fmt, { needMeta, onMeta }).then((u) => {
      if (!alive) return;
      src = u;
      onCover?.(u);
    });
    return () => {
      alive = false;
    };
  });
</script>

<div class="thumb-root" bind:this={root}>
  {#if src}
    <img class="thumb-img" src={src} alt="" decoding="async" />
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
