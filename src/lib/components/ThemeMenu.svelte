<script lang="ts">
  import { browser } from "$app/environment";
  import { THEMES, type ThemeId, applyTheme, getStoredTheme } from "$lib/theme";

  interface Props {
    /** В карточке настроек — подпись всегда видна, селектор на всю ширину */
    variant?: "inline" | "card";
  }
  let { variant = "inline" }: Props = $props();

  let value = $state<ThemeId>(browser ? getStoredTheme() : "light");

  function onChange(e: Event) {
    const v = (e.currentTarget as HTMLSelectElement).value;
    if (THEMES.some((t) => t.id === v)) {
      const id = v as ThemeId;
      applyTheme(id);
      value = id;
    }
  }
</script>

<label
  class="theme-wrap"
  class:card={variant === "card"}
  title="Тема оформления"
>
  <span class="theme-label" class:always={variant === "card"}>Тема</span>
  <select class="theme-select" {value} onchange={onChange}>
    {#each THEMES as t (t.id)}
      <option value={t.id}>{t.label}</option>
    {/each}
  </select>
</label>

<style>
  .theme-wrap {
    display: inline-flex;
    align-items: center;
    gap: 0.4rem;
    margin: 0;
    font-size: 0.78rem;
    color: var(--muted);
    font-family: system-ui, sans-serif;
  }

  .theme-label {
    display: none;
  }

  .theme-label.always {
    display: inline;
    flex-shrink: 0;
  }

  @media (min-width: 480px) {
    .theme-wrap:not(.card) .theme-label:not(.always) {
      display: inline;
      flex-shrink: 0;
    }
  }

  .theme-select {
    padding: 0.38rem 1.75rem 0.38rem 0.5rem;
    border-radius: var(--radius-md, 12px);
    border: 1px solid var(--border-soft);
    background-color: var(--elevated-soft);
    color: var(--text-soft);
    font-size: 0.78rem;
    font-weight: 550;
    cursor: pointer;
    max-width: 11rem;
    appearance: none;
    -webkit-appearance: none;
    background-image: url("data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' width='12' height='8' viewBox='0 0 12 8'%3E%3Cpath fill='%238a817a' d='M1 1.5L6 6l5-4.5'/%3E%3C/svg%3E");
    background-repeat: no-repeat;
    background-position: right 0.5rem center;
  }

  .theme-select:focus {
    outline: 2px solid var(--accent);
    outline-offset: 1px;
  }
</style>
