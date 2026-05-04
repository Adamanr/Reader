<script lang="ts">
  import ThemeMenu from "$lib/components/ThemeMenu.svelte";

  interface Props {
    /** Заголовок карточки (на странице «Настройки» лучше «Оформление», чтобы не дублировать H1) */
    sectionTitle?: string;
  }
  let { sectionTitle = "Настройки" }: Props = $props();
</script>

<section class="card" aria-labelledby="settings-theme-heading">
  <h2 id="settings-theme-heading">{sectionTitle}</h2>
  <p class="hint">Тема оформления — цвета фона, панелей и подсказок во всём приложении.</p>
  <div class="row">
    <ThemeMenu variant="card" />
  </div>
</section>

<style>
  .card {
    padding: 1rem;
    border-radius: var(--radius-md, 12px);
    border: 1px solid var(--border-soft);
    background: var(--elevated-soft);
    box-shadow: var(--shadow-soft);
  }

  .card h2 {
    margin: 0 0 0.5rem;
    font-size: 0.7rem;
    text-transform: uppercase;
    letter-spacing: 0.12em;
    color: var(--muted);
    font-weight: 600;
    font-family: system-ui, sans-serif;
  }

  .hint {
    margin: 0 0 0.85rem;
    font-size: 0.82rem;
    line-height: 1.45;
    color: var(--muted);
    font-family: system-ui, sans-serif;
  }

  .row {
    display: flex;
    flex-direction: column;
    gap: 0.35rem;
    align-items: stretch;
    max-width: 100%;
  }

  .row :global(.theme-wrap) {
    width: 100%;
    flex-wrap: wrap;
  }

  .row :global(.theme-select) {
    width: 100%;
    max-width: none;
  }
</style>
