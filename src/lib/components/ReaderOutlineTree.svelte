<script lang="ts">
  import type { ReaderOutlineItem } from "$lib/types";

  interface Props {
    items: ReaderOutlineItem[];
    activeId?: string | null;
    onSelect: (id: string) => void;
  }

  let { items, activeId = null, onSelect }: Props = $props();
  let collapsedIds = $state<string[]>([]);
  let lastSignature = "";

  $effect(() => {
    const signature = items.map((item) => `${item.id}:${item.level}:${item.label}`).join("|");
    if (lastSignature && signature !== lastSignature) collapsedIds = [];
    lastSignature = signature;
  });

  const rows = $derived.by(() => {
    if (!items.length) return [];
    const minLevel = Math.min(...items.map((item) => item.level));
    const collapsed = new Set(collapsedIds);
    const ancestors: { id: string; depth: number; collapsed: boolean }[] = [];

    return items.map((item, index) => {
      const depth = Math.max(0, item.level - minLevel);
      while (ancestors.length && ancestors[ancestors.length - 1]!.depth >= depth) {
        ancestors.pop();
      }
      const hidden = ancestors.some((ancestor) => ancestor.collapsed);
      const next = items[index + 1];
      const nextDepth = next ? Math.max(0, next.level - minLevel) : -1;
      const hasChildren = nextDepth > depth;
      if (hasChildren) ancestors.push({ id: item.id, depth, collapsed: collapsed.has(item.id) });
      return { item, depth: Math.min(depth, 6), hidden, hasChildren };
    });
  });

  function toggle(id: string) {
    collapsedIds = collapsedIds.includes(id)
      ? collapsedIds.filter((value) => value !== id)
      : [...collapsedIds, id];
  }
</script>

<div class="outline-tree" role="tree" aria-label="Дерево глав">
  {#each rows as row (row.item.id)}
    {#if !row.hidden}
      <div
        class="tree-row"
        class:tree-row-root={row.depth === 0}
        class:tree-row-nested={row.depth > 0}
        class:tree-row-active={activeId === row.item.id}
        style={`--tree-depth: ${row.depth}`}
        role="treeitem"
        aria-level={row.depth + 1}
        aria-selected={activeId === row.item.id}
        aria-expanded={row.hasChildren ? !collapsedIds.includes(row.item.id) : undefined}
      >
        {#if row.hasChildren}
          <button
            type="button"
            class="tree-branch"
            aria-label={collapsedIds.includes(row.item.id) ? "Раскрыть подразделы" : "Свернуть подразделы"}
            onclick={() => toggle(row.item.id)}
          >
            <span class:collapsed={collapsedIds.includes(row.item.id)} aria-hidden="true">⌄</span>
          </button>
        {:else}
          <span class="tree-leaf" aria-hidden="true"></span>
        {/if}

        <button
          type="button"
          class="tree-label"
          disabled={row.item.disabled}
          onclick={() => onSelect(row.item.id)}
        >
          <span class="tree-title">{row.item.label}</span>
          {#if row.item.meta}
            <span class="tree-meta">{row.item.meta}</span>
          {/if}
        </button>
      </div>
    {/if}
  {/each}
</div>

<style>
  .outline-tree {
    display: flex;
    flex-direction: column;
    gap: 0.18rem;
    padding: 0.35rem 0.45rem 0.8rem;
  }

  .tree-row {
    --tree-depth: 0;
    position: relative;
    display: grid;
    grid-template-columns: 1.3rem minmax(0, 1fr);
    align-items: stretch;
    min-width: 0;
    margin-left: calc(var(--tree-depth) * 0.78rem);
  }

  .tree-row-nested::before {
    content: "";
    position: absolute;
    left: -0.42rem;
    top: -0.22rem;
    width: 0.48rem;
    height: 1.45rem;
    border-left: 1px solid color-mix(in srgb, var(--accent) 26%, var(--border-soft));
    border-bottom: 1px solid color-mix(in srgb, var(--accent) 26%, var(--border-soft));
    border-radius: 0 0 0 0.42rem;
    pointer-events: none;
  }

  .tree-branch,
  .tree-leaf {
    align-self: center;
    justify-self: center;
    width: 1.15rem;
    height: 1.15rem;
  }

  .tree-branch {
    display: grid;
    place-items: center;
    padding: 0;
    border: 0;
    border-radius: 50%;
    background: color-mix(in srgb, var(--accent) 12%, transparent);
    color: var(--accent-2);
    cursor: pointer;
    font: 700 0.8rem/1 system-ui, sans-serif;
  }

  .tree-branch span {
    translate: 0 -0.08rem;
    transition: rotate 0.15s ease;
  }

  .tree-branch span.collapsed {
    rotate: -90deg;
  }

  .tree-leaf {
    position: relative;
  }

  .tree-leaf::after {
    content: "";
    position: absolute;
    left: 50%;
    top: 50%;
    width: 0.3rem;
    height: 0.3rem;
    translate: -50% -50%;
    border-radius: 50%;
    background: color-mix(in srgb, var(--accent-2) 34%, var(--border-soft));
  }

  .tree-label {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 0.55rem;
    min-width: 0;
    min-height: 2.25rem;
    padding: 0.45rem 0.55rem;
    border: 1px solid transparent;
    border-radius: 0.72rem;
    background: transparent;
    color: var(--text-soft);
    text-align: left;
    cursor: pointer;
    font-family: inherit;
    font-size: 0.79rem;
    line-height: 1.3;
    transition: background 0.12s ease, border-color 0.12s ease, color 0.12s ease;
  }

  .tree-label:hover:not(:disabled) {
    border-color: color-mix(in srgb, var(--accent) 16%, transparent);
    background: color-mix(in srgb, var(--accent) 9%, transparent);
  }

  .tree-label:disabled {
    opacity: 0.48;
    cursor: default;
  }

  .tree-row-root .tree-label {
    min-height: 2.55rem;
    font-family: Georgia, "Times New Roman", serif;
    font-size: 0.92rem;
    font-weight: 600;
  }

  .tree-row-active .tree-label {
    border-color: color-mix(in srgb, var(--accent-2) 24%, var(--border-soft));
    background: color-mix(in srgb, var(--accent) 15%, var(--elevated-soft));
    color: var(--accent-2);
  }

  .tree-title {
    min-width: 0;
    overflow-wrap: anywhere;
  }

  .tree-meta {
    flex: 0 0 auto;
    color: var(--muted);
    font-family: "Reader Sans", system-ui, sans-serif;
    font-size: 0.62rem;
    font-weight: 500;
    white-space: nowrap;
  }
</style>
