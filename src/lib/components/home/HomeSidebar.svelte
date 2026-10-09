<script lang="ts">
  import { page } from "$app/state";
  import type { Shelf } from "$lib/types";
  import { THEMES, applyTheme, getStoredTheme, type ThemeId } from "$lib/theme";

  interface Props {
    shelves: Shelf[];
    active: string;
    counts: (id: string) => number;
    open: boolean;
    onSelect: (id: string) => void;
    onAddShelf: (name: string) => void;
    onRemoveShelf: (id: string) => void;
    onClose: () => void;
  }
  let { shelves, active, counts, open, onSelect, onAddShelf, onRemoveShelf, onClose }: Props = $props();

  let adding = $state(false);
  let name = $state("");
  let theme = $state<ThemeId>(getStoredTheme());

  const SWATCH: Record<ThemeId, [string, string]> = {
    light: ["#f5f3fb", "#a694d6"],
    dark: ["#1d1929", "#aa91e1"],
    sepia: ["#fbf1ee", "#dca2a8"],
    forest: ["#f0f5f1", "#9bc4a8"],
    ocean: ["#eef6fb", "#96c2dc"],
  };

  const NAV = [
    { href: "/", label: "Библиотека", icon: "M4 19V6a2 2 0 0 1 2-2h3v15M9 4h4v15M13 6l3.5-1 3 13.5-3.5 1" },
    { href: "/sources", label: "Источники", icon: "M12 3v12m0 0l4-4m-4 4l-4-4M4 17v2a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2v-2" },
    { href: "/notes", label: "Заметки", icon: "M6 4h9l3 3v13H6zM9 11h6M9 15h4" },
    { href: "/words", label: "Мои слова", icon: "M4 19l4.5-12h1L14 19M6 14h6M15 11h5M17.5 8.5v5" },
    { href: "/stats", label: "Ритм чтения", icon: "M4 18h3V11H4zM10.5 18h3V6h-3zM17 18h3v-9h-3z" },
  ];

  function submit(e: SubmitEvent) {
    e.preventDefault();
    const n = name.trim();
    if (n) onAddShelf(n);
    name = "";
    adding = false;
  }

  function pickTheme(id: ThemeId) {
    theme = id;
    applyTheme(id);
  }
</script>

<!-- svelte-ignore a11y_click_events_have_key_events -->
<!-- svelte-ignore a11y_no_static_element_interactions -->
<div class="scrim" class:open onclick={onClose}></div>

<aside class="side" class:open aria-label="Навигация">
  <div class="brand">
    <span class="mark" aria-hidden="true">
      <svg viewBox="0 0 32 32"><path d="M6 7.5c3.5-1.6 7-1.6 10 .8 3-2.4 6.5-2.4 10-.8V25c-3.5-1.6-7-1.6-10 .8-3-2.4-6.5-2.4-10-.8z" /><path d="M16 8.3v17.5" /></svg>
    </span>
    <span class="name">Reader</span>
    <button type="button" class="close" aria-label="Закрыть меню" onclick={onClose}>×</button>
  </div>

  <nav class="nav">
    {#each NAV as n (n.href)}
      <a href={n.href} class:on={page.url.pathname === n.href}>
        <svg viewBox="0 0 24 24" aria-hidden="true"><path d={n.icon} /></svg>
        {n.label}
      </a>
    {/each}
  </nav>

  <div class="section">
    <div class="section-head">
      <span>Полки</span>
      <button type="button" class="plus" title="Новая полка" aria-label="Новая полка" onclick={() => (adding = !adding)}>+</button>
    </div>
    {#if adding}
      <form class="add" onsubmit={submit}>
        <!-- svelte-ignore a11y_autofocus -->
        <input bind:value={name} placeholder="Название полки" autofocus onblur={() => !name.trim() && (adding = false)} />
      </form>
    {/if}
    <ul class="shelves">
      <li>
        <button type="button" class="shelf" class:on={active === "all"} onclick={() => onSelect("all")}>
          <span class="dot all" aria-hidden="true"></span>
          <span class="label">Все книги</span>
          <span class="count">{counts("all")}</span>
        </button>
      </li>
      {#each shelves as s (s.id)}
        <li class="shelf-row">
          <button type="button" class="shelf" class:on={active === s.id} onclick={() => onSelect(s.id)}>
            <span class="dot" aria-hidden="true"></span>
            <span class="label">{s.name}</span>
            <span class="count">{counts(s.id)}</span>
          </button>
          {#if s.id !== "default"}
            <button type="button" class="remove" title="Удалить полку" aria-label="Удалить полку {s.name}" onclick={() => onRemoveShelf(s.id)}>×</button>
          {/if}
        </li>
      {/each}
      {#if counts("hidden") > 0 || active === "hidden"}
        <li>
          <button type="button" class="shelf muted" class:on={active === "hidden"} onclick={() => onSelect("hidden")}>
            <span class="dot hidden-dot" aria-hidden="true"></span>
            <span class="label">Скрытые</span>
            <span class="count">{counts("hidden")}</span>
          </button>
        </li>
      {/if}
    </ul>
  </div>

  <div class="foot">
    <div class="themes" role="group" aria-label="Тема оформления">
      {#each THEMES as t (t.id)}
        <button
          type="button"
          class="swatch"
          class:on={theme === t.id}
          title={t.label}
          aria-label={t.label}
          style:--a={SWATCH[t.id][0]}
          style:--b={SWATCH[t.id][1]}
          onclick={() => pickTheme(t.id)}
        ></button>
      {/each}
    </div>
    <a href="/settings" class="settings" class:on={page.url.pathname === "/settings"}>
      <svg viewBox="0 0 24 24" aria-hidden="true"><circle cx="12" cy="12" r="3" /><path d="M19.4 15a1.7 1.7 0 0 0 .34 1.88l.06.06-2.83 2.83-.06-.06a1.7 1.7 0 0 0-1.88-.34 1.7 1.7 0 0 0-1.03 1.56V21h-4v-.09A1.7 1.7 0 0 0 9 19.37a1.7 1.7 0 0 0-1.88.34l-.06.06-2.83-2.83.06-.06A1.7 1.7 0 0 0 4.63 15 1.7 1.7 0 0 0 3.09 14H3v-4h.09A1.7 1.7 0 0 0 4.63 9a1.7 1.7 0 0 0-.34-1.88l-.06-.06 2.83-2.83.06.06A1.7 1.7 0 0 0 9 4.63 1.7 1.7 0 0 0 10 3.09V3h4v.09A1.7 1.7 0 0 0 15 4.63a1.7 1.7 0 0 0 1.88-.34l.06-.06 2.83 2.83-.06.06A1.7 1.7 0 0 0 19.37 9 1.7 1.7 0 0 0 20.91 10H21v4h-.09A1.7 1.7 0 0 0 19.4 15Z" /></svg>
      Настройки
    </a>
  </div>
</aside>

<style>
  .side {
    height: 100%;
    width: 15.5rem;
    flex-shrink: 0;
    display: flex;
    flex-direction: column;
    gap: 1.4rem;
    padding: 1.3rem 0.9rem 1rem;
    box-sizing: border-box;
    border-right: 1px solid color-mix(in srgb, var(--border-soft) 60%, transparent);
    background: color-mix(in srgb, var(--panel-soft) 45%, var(--bg-soft));
    overflow-y: auto;
    z-index: 30;
  }

  .brand {
    display: flex;
    align-items: center;
    gap: 0.6rem;
    padding: 0 0.45rem;
  }

  .mark {
    display: grid;
    place-items: center;
    width: 2.1rem;
    height: 2.1rem;
    border-radius: 0.7rem;
    background: linear-gradient(145deg, var(--accent), var(--accent-2));
    box-shadow: 0 6px 16px -6px var(--accent-2);
  }

  .mark svg {
    width: 1.35rem;
    height: 1.35rem;
    fill: none;
    stroke: #fff;
    stroke-width: 1.8;
    stroke-linejoin: round;
  }

  .name {
    flex: 1;
    font-family: "Literata Variable", Georgia, serif;
    font-size: 1.3rem;
    font-weight: 600;
    letter-spacing: -0.01em;
    color: var(--text-soft);
  }

  .close {
    display: none;
    border: none;
    background: transparent;
    color: var(--muted);
    font-size: 1.5rem;
    cursor: pointer;
  }

  .nav {
    display: flex;
    flex-direction: column;
    gap: 0.15rem;
  }

  .nav a,
  .settings {
    display: flex;
    align-items: center;
    gap: 0.7rem;
    padding: 0.55rem 0.7rem;
    border-radius: 0.75rem;
    color: var(--text-soft);
    text-decoration: none;
    font-size: 0.92rem;
    transition: background 0.15s ease;
  }

  .nav a:hover,
  .settings:hover {
    background: color-mix(in srgb, var(--accent) 12%, transparent);
  }

  .nav a.on,
  .settings.on {
    background: var(--elevated-soft);
    box-shadow: 0 1px 3px rgba(20, 14, 30, 0.06);
    font-weight: 600;
  }

  .nav svg,
  .settings svg {
    width: 1.15rem;
    height: 1.15rem;
    fill: none;
    stroke: currentColor;
    stroke-width: 1.7;
    stroke-linecap: round;
    stroke-linejoin: round;
    opacity: 0.8;
  }

  .section {
    display: flex;
    flex-direction: column;
    gap: 0.35rem;
    min-height: 0;
  }

  .section-head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 0 0.7rem;
    font-size: 0.7rem;
    text-transform: uppercase;
    letter-spacing: 0.14em;
    color: var(--muted);
    font-weight: 650;
  }

  .plus {
    width: 1.5rem;
    height: 1.5rem;
    border-radius: 0.5rem;
    border: none;
    background: transparent;
    color: var(--muted);
    font-size: 1.1rem;
    cursor: pointer;
  }

  .plus:hover {
    background: color-mix(in srgb, var(--accent) 14%, transparent);
    color: var(--text-soft);
  }

  .add input {
    width: 100%;
    box-sizing: border-box;
    border: 1px solid var(--border-soft);
    background: var(--elevated-soft);
    color: var(--text-soft);
    border-radius: 0.65rem;
    padding: 0.45rem 0.7rem;
    font: inherit;
    font-size: 0.86rem;
  }

  .shelves {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 0.1rem;
  }

  .shelf-row {
    position: relative;
  }

  .shelf {
    width: 100%;
    display: flex;
    align-items: center;
    gap: 0.65rem;
    padding: 0.45rem 0.7rem;
    border: none;
    border-radius: 0.7rem;
    background: transparent;
    color: var(--text-soft);
    font: inherit;
    font-size: 0.88rem;
    text-align: left;
    cursor: pointer;
  }

  .shelf:hover {
    background: color-mix(in srgb, var(--accent) 10%, transparent);
  }

  .shelf.on {
    background: color-mix(in srgb, var(--accent) 20%, transparent);
    font-weight: 600;
  }

  .shelf.muted {
    color: var(--muted);
  }

  .dot {
    width: 0.5rem;
    height: 0.5rem;
    border-radius: 2px;
    background: var(--accent);
    flex-shrink: 0;
  }

  .dot.all {
    background: var(--accent-2);
  }

  .dot.hidden-dot {
    background: transparent;
    border: 1.5px dashed var(--muted);
  }

  .label {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .count {
    font-size: 0.74rem;
    color: var(--muted);
    font-variant-numeric: tabular-nums;
  }

  .remove {
    position: absolute;
    right: 0.3rem;
    top: 50%;
    transform: translateY(-50%);
    width: 1.4rem;
    height: 1.4rem;
    border: none;
    border-radius: 0.45rem;
    background: var(--elevated-soft);
    color: var(--muted);
    cursor: pointer;
    opacity: 0;
    transition: opacity 0.15s ease;
  }

  .shelf-row:hover .remove,
  .remove:focus-visible {
    opacity: 1;
  }

  .shelf-row:hover .count {
    opacity: 0;
  }

  .foot {
    margin-top: auto;
    display: flex;
    flex-direction: column;
    gap: 0.6rem;
  }

  .themes {
    display: flex;
    gap: 0.4rem;
    padding: 0 0.7rem;
  }

  .swatch {
    width: 1.35rem;
    height: 1.35rem;
    border-radius: 50%;
    border: 2px solid transparent;
    background: linear-gradient(135deg, var(--a) 50%, var(--b) 50%);
    box-shadow: 0 0 0 1px var(--border-soft);
    cursor: pointer;
    padding: 0;
    transition: transform 0.15s ease;
  }

  .swatch:hover {
    transform: scale(1.12);
  }

  .swatch.on {
    border-color: var(--elevated-soft);
    box-shadow: 0 0 0 2px var(--accent-2);
  }

  .scrim {
    display: none;
  }

  @media (max-width: 900px) {
    .side {
      position: fixed;
      left: 0;
      top: 0;
      bottom: 0;
      height: auto;
      width: min(18rem, 86vw);
      transform: translateX(-104%);
      transition: transform 0.28s cubic-bezier(0.2, 0.8, 0.2, 1);
      background: var(--panel-elevated);
      box-shadow: var(--shadow-float);
      padding-top: max(1.3rem, env(safe-area-inset-top));
      z-index: 60;
    }

    .side.open {
      transform: none;
    }

    .close {
      display: block;
    }

    .scrim {
      display: block;
      position: fixed;
      inset: 0;
      z-index: 55;
      background: rgba(20, 14, 30, 0.3);
      opacity: 0;
      pointer-events: none;
      transition: opacity 0.28s ease;
    }

    .scrim.open {
      opacity: 1;
      pointer-events: auto;
    }
  }
</style>
