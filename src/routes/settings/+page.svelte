<script lang="ts">
  import { goto } from "$app/navigation";
  import SettingsThemeCard from "$lib/components/SettingsThemeCard.svelte";
  import TranslateSettingsCard from "$lib/components/TranslateSettingsCard.svelte";
  import TypstSettingsCard from "$lib/components/TypstSettingsCard.svelte";
</script>

<div class="settings-shell">
  <div class="settings-ambient" aria-hidden="true"></div>
  <header class="settings-top">
    <div class="settings-top-inner">
      <button type="button" class="back" onclick={() => goto("/")}>
        <span class="back-arr" aria-hidden="true">←</span>
        <span>Библиотека</span>
      </button>
      <div class="settings-heading">
        <span class="settings-kicker">Reader</span>
        <h1 class="settings-title">Настройки</h1>
        <p>Оформление, перевод и экспорт — в одном месте.</p>
      </div>
    </div>
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
    position: relative;
    isolation: isolate;
    overflow-x: hidden;
  }

  .settings-ambient {
    position: fixed;
    inset: 0;
    z-index: -1;
    pointer-events: none;
    background:
      radial-gradient(circle at 86% 10%, color-mix(in srgb, #fff2bf 58%, transparent) 0 4.2rem, transparent 4.35rem),
      radial-gradient(ellipse 34rem 20rem at 10% 0%, color-mix(in srgb, var(--accent) 24%, transparent), transparent 68%),
      radial-gradient(ellipse 32rem 20rem at 100% 54%, color-mix(in srgb, #bcd8ee 20%, transparent), transparent 70%);
  }

  .settings-top {
    padding: clamp(1rem, 3vw, 1.5rem) clamp(1rem, 5vw, 2rem) clamp(1.4rem, 4vw, 2rem);
    border-bottom: 1px solid color-mix(in srgb, var(--accent) 15%, var(--border-soft));
    background: color-mix(in srgb, var(--panel-veil) 72%, transparent);
    backdrop-filter: blur(24px) saturate(1.15);
  }

  .settings-top-inner {
    width: min(100%, 58rem);
    margin: 0 auto;
  }

  .back {
    display: inline-flex;
    align-items: center;
    gap: 0.35rem;
    min-height: 2.35rem;
    padding: 0.42rem 0.65rem;
    margin: 0;
    border: 1px solid color-mix(in srgb, var(--accent) 18%, var(--border-soft));
    border-radius: 999px;
    background: color-mix(in srgb, var(--elevated-soft) 54%, transparent);
    color: var(--accent-2);
    font-size: 0.92rem;
    font-weight: 650;
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
    margin: 0.15rem 0 0;
    font-size: clamp(2.15rem, 6vw, 3.15rem);
    font-weight: 500;
    font-family: Georgia, "Times New Roman", serif;
    color: var(--text-soft);
    letter-spacing: -0.045em;
    line-height: 1.08;
  }

  .settings-heading {
    margin-top: clamp(1rem, 3vw, 1.55rem);
  }

  .settings-kicker {
    color: var(--accent-2);
    font-size: 0.64rem;
    font-weight: 750;
    letter-spacing: 0.18em;
    text-transform: uppercase;
  }

  .settings-heading p {
    margin: 0.55rem 0 0;
    color: var(--muted);
    font-size: 0.95rem;
    line-height: 1.5;
  }

  .settings-main {
    flex: 1;
    display: grid;
    grid-template-columns: repeat(2, minmax(0, 1fr));
    align-content: start;
    gap: 1.1rem;
    padding: clamp(1.25rem, 4vw, 2.25rem);
    max-width: 64rem;
    width: 100%;
    margin-inline: auto;
    box-sizing: border-box;
  }

  :global(.settings-main > .card) {
    width: 100%;
    max-width: none;
    margin-top: 0;
    min-width: 0;
    padding: 1.2rem;
    border-color: color-mix(in srgb, var(--accent) 17%, var(--border-soft));
    border-radius: 1.45rem;
    background: color-mix(in srgb, var(--panel-elevated) 74%, transparent);
    box-shadow: 0 18px 60px color-mix(in srgb, var(--accent-2) 8%, transparent);
    backdrop-filter: blur(20px) saturate(1.08);
  }

  :global(.settings-main > .card:last-child) {
    grid-column: 1 / -1;
  }

  @media (max-width: 700px) {
    .settings-top {
      padding:
        max(1rem, env(safe-area-inset-top))
        max(1rem, env(safe-area-inset-right))
        1.25rem
        max(1rem, env(safe-area-inset-left));
    }

    .back {
      min-height: 2.75rem;
      padding-inline: 0.8rem;
    }

    .settings-main {
      grid-template-columns: 1fr;
      gap: 0.9rem;
      padding:
        1rem
        max(1rem, env(safe-area-inset-right))
        max(1.5rem, env(safe-area-inset-bottom))
        max(1rem, env(safe-area-inset-left));
    }

    :global(.settings-main > .card:last-child) {
      grid-column: auto;
    }

    :global(.settings-main > .card) {
      padding: 1rem;
      border-radius: 1.25rem;
    }

    :global(.settings-main button),
    :global(.settings-main input),
    :global(.settings-main .select-trigger) {
      min-height: 2.75rem;
    }
  }
</style>
