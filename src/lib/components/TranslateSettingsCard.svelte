<script lang="ts">
  import { browser } from "$app/environment";
  import {
    DEFAULT_TRANSLATE_API,
    getTranslateSettings,
    saveTranslateSettings,
    type TranslateSettings,
  } from "$lib/translate/settings";

  let apiBase = $state(DEFAULT_TRANSLATE_API);
  let apiKey = $state("");

  if (browser) {
    const s = getTranslateSettings();
    apiBase = s.apiBase;
    apiKey = s.apiKey;
  }

  function persist() {
    saveTranslateSettings({ apiBase: apiBase.trim() || DEFAULT_TRANSLATE_API, apiKey });
  }
</script>

<section class="card" id="translate-api" aria-labelledby="translate-api-h">
  <h2 id="translate-api-h">Перевод (LibreTranslate)</h2>
  <p class="hint">
    Укажите URL совместимого с
    <a href="https://github.com/LibreTranslate/LibreTranslate" target="_blank" rel="noreferrer"
      >LibreTranslate</a
    >
    (по умолчанию публичный сервер; для частых переводов лучше свой или платный ключ).
  </p>
  <label class="field">
    <span>Базовый URL</span>
    <input type="url" bind:value={apiBase} placeholder={DEFAULT_TRANSLATE_API} onblur={persist} />
  </label>
  <label class="field">
    <span>API-ключ (если требуется)</span>
    <input type="password" bind:value={apiKey} autocomplete="off" onblur={persist} placeholder="" />
  </label>
  <p class="note">
    Эндпоинт: <code>POST …/translate</code> с телом JSON <code>q</code>, <code>source</code>, <code
      >target</code
    >.
  </p>
</section>

<style>
  .card {
    padding: 1rem;
    border-radius: var(--radius-md, 12px);
    border: 1px solid var(--border-soft);
    background: var(--elevated-soft);
    box-shadow: var(--shadow-soft);
    margin-top: 1rem;
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

  .hint a {
    color: var(--accent-2);
  }

  .field {
    display: flex;
    flex-direction: column;
    gap: 0.28rem;
    margin-bottom: 0.65rem;
    font-size: 0.78rem;
    color: var(--muted);
  }

  .field span {
    font-weight: 500;
    color: var(--text-soft);
  }

  .field input {
    padding: 0.48rem 0.55rem;
    border-radius: var(--radius-sm);
    border: 1px solid var(--border-soft);
    background-color: var(--elevated-soft);
    color: var(--text-soft);
    font-size: 0.88rem;
  }

  .note {
    margin: 0;
    font-size: 0.74rem;
    color: var(--muted);
    line-height: 1.4;
  }

  .note code {
    font-size: 0.72em;
    padding: 0.1em 0.25em;
    border-radius: 4px;
    background: color-mix(in srgb, var(--panel-soft) 90%, transparent);
  }
</style>
