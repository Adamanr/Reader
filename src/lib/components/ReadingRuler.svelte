<script lang="ts">
  /**
   * Линейка фокуса: светлая полоса следует за курсором, остальное приглушено.
   * Работает поверх любого формата, включая сканы PDF.
   */
  interface Props {
    host: HTMLElement | null;
    /** Высота полосы, px */
    band: number;
    tint: string;
  }
  let { host, band, tint }: Props = $props();

  let y = $state<number | null>(null);
  let h = $state(0);

  $effect(() => {
    const el = host;
    if (!el) return;
    const move = (e: PointerEvent) => {
      const r = el.getBoundingClientRect();
      if (e.clientY < r.top || e.clientY > r.bottom) return;
      y = e.clientY - r.top;
      h = r.height;
    };
    const ro = new ResizeObserver(() => (h = el.clientHeight));
    ro.observe(el);
    window.addEventListener("pointermove", move, { passive: true });
    h = el.clientHeight;
    if (y == null) y = h * 0.4;
    return () => {
      window.removeEventListener("pointermove", move);
      ro.disconnect();
    };
  });

  const top = $derived(Math.max(0, (y ?? 0) - band / 2));
</script>

{#if y != null}
  <div class="ruler" aria-hidden="true" style:--tint={tint}>
    <div class="shade" style:height="{top}px"></div>
    <div class="band" style:top="{top}px" style:height="{band}px"></div>
    <div class="shade bottom" style:top="{top + band}px" style:height="{Math.max(0, h - top - band)}px"></div>
  </div>
{/if}

<style>
  .ruler {
    position: absolute;
    inset: 0;
    pointer-events: none;
    z-index: 20;
  }

  .shade {
    position: absolute;
    left: 0;
    right: 0;
    top: 0;
    background: color-mix(in srgb, var(--tint) 62%, transparent);
    transition:
      height 0.08s linear,
      top 0.08s linear;
  }

  .band {
    position: absolute;
    left: 0;
    right: 0;
    border-top: 1px solid color-mix(in srgb, var(--accent) 40%, transparent);
    border-bottom: 1px solid color-mix(in srgb, var(--accent) 40%, transparent);
    transition: top 0.08s linear;
  }
</style>
