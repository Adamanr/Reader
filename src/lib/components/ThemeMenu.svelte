<script lang="ts">
  import { browser } from "$app/environment";
  import { THEMES, type ThemeId, applyTheme, getStoredTheme } from "$lib/theme";
  import DreamSelect from "$lib/components/DreamSelect.svelte";

  interface Props {
    /** В карточке настроек — подпись всегда видна, селектор на всю ширину */
    variant?: "inline" | "card";
  }
  let { variant = "inline" }: Props = $props();

  let value = $state<ThemeId>(browser ? getStoredTheme() : "light");

  function onChange(v: string) {
    if (THEMES.some((t) => t.id === v)) {
      const id = v as ThemeId;
      applyTheme(id);
      value = id;
    }
  }
</script>

<div
  class="theme-wrap"
  class:card={variant === "card"}
  title="Тема оформления"
>
  <span class="theme-label" class:always={variant === "card"}>Тема</span>
  <span class="theme-select-shell">
    <DreamSelect
      {value}
      options={THEMES.map((theme) => ({ value: theme.id, label: theme.label }))}
      ariaLabel="Тема оформления"
      compact={variant === "inline"}
      onChange={onChange}
    />
  </span>
</div>

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

  .theme-select-shell {
    position: relative;
    display: inline-block;
    max-width: 11rem;
    width: 100%;
  }

  .card .theme-select-shell {
    max-width: none;
  }

</style>
