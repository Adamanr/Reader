<script lang="ts">
  import { tick } from "svelte";

  export interface DreamSelectOption {
    value: string;
    label: string;
    hint?: string;
    disabled?: boolean;
  }

  interface Props {
    value: string;
    options: DreamSelectOption[];
    onChange: (value: string) => void;
    ariaLabel: string;
    placeholder?: string;
    disabled?: boolean;
    compact?: boolean;
  }

  let {
    value,
    options,
    onChange,
    ariaLabel,
    placeholder = "Выберите…",
    disabled = false,
    compact = false,
  }: Props = $props();

  let root = $state<HTMLDivElement | null>(null);
  let trigger = $state<HTMLButtonElement | null>(null);
  let open = $state(false);
  let activeIndex = $state(0);
  let menuStyle = $state("");

  const selected = $derived(options.find((option) => option.value === value) ?? null);

  function firstEnabledIndex(): number {
    const selectedIndex = options.findIndex((option) => option.value === value && !option.disabled);
    if (selectedIndex >= 0) return selectedIndex;
    const first = options.findIndex((option) => !option.disabled);
    return Math.max(0, first);
  }

  function positionMenu() {
    const el = trigger;
    if (!el) return;
    const rect = el.getBoundingClientRect();
    const estimatedHeight = Math.min(286, options.length * 44 + 12);
    const below = window.innerHeight - rect.bottom;
    const openUp = below < Math.min(190, estimatedHeight) && rect.top > below;
    const top = openUp
      ? Math.max(8, rect.top - estimatedHeight - 6)
      : Math.min(window.innerHeight - estimatedHeight - 8, rect.bottom + 6);
    const width = Math.max(rect.width, Math.min(320, window.innerWidth - 16));
    const left = Math.min(Math.max(8, rect.left), window.innerWidth - width - 8);
    menuStyle = `left:${left}px;top:${Math.max(8, top)}px;width:${width}px;max-height:${Math.max(96, Math.min(286, openUp ? rect.top - 14 : below - 14))}px`;
  }

  async function show() {
    if (disabled || !options.length) return;
    activeIndex = firstEnabledIndex();
    open = true;
    await tick();
    positionMenu();
  }

  function hide({ focus = false } = {}) {
    open = false;
    if (focus) requestAnimationFrame(() => trigger?.focus());
  }

  function choose(option: DreamSelectOption) {
    if (option.disabled) return;
    onChange(option.value);
    hide({ focus: true });
  }

  function move(step: number) {
    if (!options.length) return;
    let next = activeIndex;
    for (let count = 0; count < options.length; count++) {
      next = (next + step + options.length) % options.length;
      if (!options[next]?.disabled) {
        activeIndex = next;
        requestAnimationFrame(() => {
          root?.querySelector<HTMLElement>(`[data-option-index="${next}"]`)?.scrollIntoView({ block: "nearest" });
        });
        return;
      }
    }
  }

  function onTriggerKeydown(event: KeyboardEvent) {
    if (event.key === "ArrowDown" || event.key === "ArrowUp" || event.key === "Enter" || event.key === " ") {
      event.preventDefault();
      void show();
    }
  }

  $effect(() => {
    if (!open) return;
    const onPointerDown = (event: PointerEvent) => {
      if (!root?.contains(event.target as Node)) hide();
    };
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key === "Escape") {
        event.preventDefault();
        hide({ focus: true });
      } else if (event.key === "ArrowDown") {
        event.preventDefault();
        move(1);
      } else if (event.key === "ArrowUp") {
        event.preventDefault();
        move(-1);
      } else if (event.key === "Enter" || event.key === " ") {
        event.preventDefault();
        const option = options[activeIndex];
        if (option) choose(option);
      } else if (event.key === "Tab") {
        hide();
      }
    };
    const onViewportChange = () => positionMenu();
    document.addEventListener("pointerdown", onPointerDown, true);
    document.addEventListener("keydown", onKeyDown);
    window.addEventListener("resize", onViewportChange);
    window.addEventListener("scroll", onViewportChange, true);
    return () => {
      document.removeEventListener("pointerdown", onPointerDown, true);
      document.removeEventListener("keydown", onKeyDown);
      window.removeEventListener("resize", onViewportChange);
      window.removeEventListener("scroll", onViewportChange, true);
    };
  });
</script>

<div class="dream-select" class:compact bind:this={root}>
  <button
    bind:this={trigger}
    type="button"
    class="select-trigger"
    class:placeholder={!selected}
    aria-label={ariaLabel}
    aria-haspopup="listbox"
    aria-expanded={open}
    {disabled}
    onclick={() => (open ? hide() : void show())}
    onkeydown={onTriggerKeydown}
  >
    <span class="selected-copy">
      <span class="selected-label">{selected?.label ?? placeholder}</span>
      {#if selected?.hint}
        <span class="selected-hint">{selected.hint}</span>
      {/if}
    </span>
    <span class="chevron" class:open aria-hidden="true"></span>
  </button>

  {#if open}
    <div class="select-menu" style={menuStyle} role="listbox" aria-label={ariaLabel}>
      {#each options as option, index (option.value)}
        <button
          type="button"
          class="select-option"
          class:active={index === activeIndex}
          class:selected={option.value === value}
          role="option"
          aria-selected={option.value === value}
          disabled={option.disabled}
          data-option-index={index}
          onpointerenter={() => (activeIndex = index)}
          onclick={() => choose(option)}
        >
          <span class="option-mark" aria-hidden="true">{option.value === value ? "✓" : ""}</span>
          <span class="option-copy">
            <span class="option-label">{option.label}</span>
            {#if option.hint}<span class="option-hint">{option.hint}</span>{/if}
          </span>
        </button>
      {/each}
    </div>
  {/if}
</div>

<style>
  .dream-select {
    position: relative;
    width: 100%;
    min-width: 0;
  }

  .select-trigger {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 0.75rem;
    width: 100%;
    min-width: 0;
    min-height: 2.65rem;
    padding: 0.55rem 0.78rem;
    border: 1px solid color-mix(in srgb, var(--accent) 18%, var(--border-soft));
    border-radius: 0.9rem;
    background: color-mix(in srgb, var(--elevated-soft) 82%, transparent);
    color: var(--text-soft);
    box-shadow: inset 0 1px 0 color-mix(in srgb, #fff 48%, transparent);
    cursor: pointer;
    text-align: left;
    font-family: inherit;
    transition: border-color 0.15s ease, background 0.15s ease, box-shadow 0.15s ease;
  }

  .compact .select-trigger {
    min-height: 2.25rem;
    padding: 0.42rem 0.65rem;
    border-radius: 0.75rem;
  }

  .select-trigger:hover:not(:disabled),
  .select-trigger[aria-expanded="true"] {
    border-color: color-mix(in srgb, var(--accent-2) 38%, var(--border-soft));
    background: var(--elevated-soft);
    box-shadow: 0 0 0 3px color-mix(in srgb, var(--accent) 11%, transparent);
  }

  .select-trigger:disabled {
    opacity: 0.48;
    cursor: default;
  }

  .selected-copy,
  .option-copy {
    display: flex;
    flex-direction: column;
    min-width: 0;
  }

  .selected-label,
  .option-label {
    overflow: hidden;
    color: inherit;
    font-size: 0.82rem;
    font-weight: 600;
    line-height: 1.25;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .compact .selected-label {
    font-size: 0.77rem;
  }

  .selected-hint,
  .option-hint {
    margin-top: 0.12rem;
    overflow: hidden;
    color: var(--muted);
    font-size: 0.64rem;
    line-height: 1.3;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .placeholder .selected-label {
    color: var(--muted);
    font-weight: 500;
  }

  .chevron {
    width: 0.5rem;
    height: 0.5rem;
    flex: 0 0 auto;
    margin: -0.2rem 0.15rem 0 0;
    border-right: 2px solid var(--muted);
    border-bottom: 2px solid var(--muted);
    rotate: 45deg;
    transition: rotate 0.16s ease, margin 0.16s ease;
  }

  .chevron.open {
    margin-top: 0.2rem;
    rotate: 225deg;
  }

  .select-menu {
    position: fixed;
    z-index: 1400;
    overflow-y: auto;
    padding: 0.38rem;
    border: 1px solid color-mix(in srgb, var(--accent) 24%, var(--border-soft));
    border-radius: 1rem;
    background: color-mix(in srgb, var(--panel-elevated) 96%, transparent);
    box-shadow: var(--shadow-float);
    backdrop-filter: blur(22px) saturate(1.18);
  }

  .select-option {
    display: grid;
    grid-template-columns: 1.25rem minmax(0, 1fr);
    align-items: center;
    gap: 0.45rem;
    width: 100%;
    min-height: 2.5rem;
    padding: 0.48rem 0.58rem;
    border: 1px solid transparent;
    border-radius: 0.72rem;
    background: transparent;
    color: var(--text-soft);
    cursor: pointer;
    text-align: left;
    font-family: inherit;
  }

  .select-option.active:not(:disabled) {
    border-color: color-mix(in srgb, var(--accent) 14%, transparent);
    background: color-mix(in srgb, var(--accent) 10%, transparent);
  }

  .select-option.selected {
    color: var(--accent-2);
  }

  .select-option:disabled {
    opacity: 0.42;
    cursor: default;
  }

  .option-mark {
    display: grid;
    place-items: center;
    width: 1.12rem;
    height: 1.12rem;
    border-radius: 50%;
    background: color-mix(in srgb, var(--accent) 12%, transparent);
    color: var(--accent-2);
    font-size: 0.66rem;
    font-weight: 800;
  }

  @media (hover: none) and (pointer: coarse) {
    .select-trigger,
    .compact .select-trigger,
    .select-option {
      min-height: 2.75rem;
    }
  }
</style>
