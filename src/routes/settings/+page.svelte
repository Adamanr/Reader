<script lang="ts">
  import { goto } from "$app/navigation";
  import SettingsThemeCard from "$lib/components/SettingsThemeCard.svelte";
  import TranslateSettingsCard from "$lib/components/TranslateSettingsCard.svelte";
  import TypstSettingsCard from "$lib/components/TypstSettingsCard.svelte";
</script>

<div class="settings-shell">
  <header class="settings-top">
    <button type="button" class="back" onclick={() => goto("/")}>
      <span class="back-arr" aria-hidden="true">←</span>
      <span>Библиотека</span>
    </button>
    <h1 class="settings-title">Настройки</h1>
  </header>

  <main class="settings-main">
    <SettingsThemeCard sectionTitle="Оформление" />
    <TypstSettingsCard />
    <TranslateSettingsCard />
  </main>
</div>

<style>
  .settings-shell {
    min-height: 100vh;
    min-height: 100dvh;
    display: flex;
    flex-direction: column;
    background: var(--bg-soft);
    color: var(--text-soft);
  }

  .settings-top {
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: 0.65rem;
    padding: 0.85rem 1rem;
    border-bottom: 1px solid var(--border-soft);
    background: var(--panel-veil);
    backdrop-filter: blur(10px);
  }

  .back {
    display: inline-flex;
    align-items: center;
    gap: 0.35rem;
    padding: 0.35rem 0.5rem;
    margin: 0;
    border: none;
    border-radius: var(--radius-md, 12px);
    background: transparent;
    color: var(--accent-2);
    font-size: 0.92rem;
    font-weight: 600;
    cursor: pointer;
    font-family: system-ui, sans-serif;
  }

  .back:hover {
    background: var(--elevated-soft);
  }

  .back-arr {
    font-size: 1.1rem;
    line-height: 1;
  }

  .settings-title {
    margin: 0;
    font-size: 1.35rem;
    font-weight: 600;
    font-family: system-ui, sans-serif;
    color: var(--text-soft);
  }

  .settings-main {
    flex: 1;
    padding: clamp(1rem, 4vw, 1.75rem);
    max-width: 28rem;
    width: 100%;
    margin-inline: auto;
    box-sizing: border-box;
  }
</style>
