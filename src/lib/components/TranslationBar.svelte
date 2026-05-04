<script lang="ts">
  import { LANG_SOURCE, LANG_TARGET } from "$lib/translate/settings";

  interface Props {
    /** fb2 — режим двух колонок; epub/pdf — только подпись в UI */
    format: "pdf" | "epub" | "fb2";
    sourceLang: string;
    targetLang: string;
    /** orig | trans | split (split только FB2) */
    layout: "orig" | "trans" | "split";
    busy: boolean;
    error: string | null;
    hasTranslation: boolean;
    onSourceChange: (code: string) => void;
    onTargetChange: (code: string) => void;
    onLayoutChange: (layout: "orig" | "trans" | "split") => void;
    onRunTranslate: () => void;
    /** Узкая колонка (боковая вкладка читалки) */
    compact?: boolean;
  }
  let {
    format,
    sourceLang,
    targetLang,
    layout,
    busy,
    error,
    hasTranslation,
    onSourceChange,
    onTargetChange,
    onLayoutChange,
    onRunTranslate,
    compact = false,
  }: Props = $props();
</script>

<div class="trans-bar" class:compact role="region" aria-label="Перевод книги">
  <div class="trans-row">
    <label class="mini-field">
      <span>С языка</span>
      <select
        class="mini-select"
        value={sourceLang}
        onchange={(e) => onSourceChange((e.currentTarget as HTMLSelectElement).value)}
        disabled={busy}
      >
        {#each LANG_SOURCE as L (L.code)}
          <option value={L.code}>{L.label}</option>
        {/each}
      </select>
    </label>

    <label class="mini-field">
      <span>На язык</span>
      <select
        class="mini-select"
        value={targetLang}
        onchange={(e) => onTargetChange((e.currentTarget as HTMLSelectElement).value)}
        disabled={busy}
      >
        {#each LANG_TARGET as L (L.code)}
          <option value={L.code}>{L.label}</option>
        {/each}
      </select>
    </label>

    <button type="button" class="run-btn" onclick={onRunTranslate} disabled={busy}>
      {busy ? "Перевожу…" : format === "pdf" ? "Перевести страницу" : "Перевести книгу"}
    </button>
  </div>

  <div class="trans-row second">
    <span class="hint-label">Показ:</span>
    <div class="seg" role="tablist">
      <button
        type="button"
        class="seg-btn"
        class:active={layout === "orig"}
        onclick={() => onLayoutChange("orig")}
        disabled={busy}>Оригинал</button>
      <button
        type="button"
        class="seg-btn"
        class:active={layout === "trans"}
        onclick={() => onLayoutChange("trans")}
        disabled={busy}>Перевод</button>
      {#if format === "fb2"}
        <button
          type="button"
          class="seg-btn"
          class:active={layout === "split"}
          onclick={() => onLayoutChange("split")}
          disabled={busy || !hasTranslation}>Рядом</button>
      {/if}
    </div>
    <a class="api-link" href="/settings#translate-api">API перевода</a>
  </div>

  {#if layout === "trans" && !hasTranslation && format === "fb2"}
    <p class="warn">
      Сначала нажмите «Перевести книгу». Сервер — LibreTranslate (настройка URL в параметрах).
    </p>
  {/if}

  {#if error}
    <p class="err">{error}</p>
  {/if}
</div>

<style>
  .trans-bar.compact {
    padding: 0.35rem 0.4rem;
    gap: 0.28rem;
  }

  .trans-bar.compact .trans-row {
    flex-wrap: wrap;
    gap: 0.35rem;
  }

  .trans-bar.compact .mini-select {
    max-width: 100%;
  }

  .trans-bar {
    flex-shrink: 0;
    padding: 0.45rem clamp(0.65rem, 2vw, 1rem);
    border-bottom: 1px solid var(--border-soft);
    background: color-mix(in srgb, var(--accent) 5%, var(--elevated-soft));
    display: flex;
    flex-direction: column;
    gap: 0.35rem;
    font-family: system-ui, sans-serif;
  }

  .trans-row {
    margin-top: 0.45rem;
    margin-bottom: 0.45rem;
    display: flex;
    flex-wrap: wrap;
    align-items: flex-end;
    gap: 0.45rem 0.75rem;
  }

  .trans-row.second {
    align-items: center;
  }

  .mini-field {
    width: 100%;
    display: flex;
    flex-direction: column;
    gap: 0.15rem;
    margin: 0;
    font-size: 0.68rem;
    color: var(--muted);
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.06em;
    margin-bottom: 0.35rem;
  }

  .mini-select {
    padding: 0.32rem 1.75rem 0.32rem 0.45rem;
    border-radius: var(--radius-sm);
    border: 1px solid var(--border-soft);
    background-color: var(--elevated-soft);
    color: var(--text-soft);
    font-size: 0.82rem;
    font-weight: 550;
    min-width: 8.5rem;
    cursor: pointer;
    appearance: none;
    background-image: url("data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' width='12' height='8'%3E%3Cpath fill='%238a817a' d='M1 1.5L6 6l5-4.5'/%3E%3C/svg%3E");
    background-repeat: no-repeat;
    background-position: right 0.45rem center;
  }

  .mini-select option {
    background: var(--panel-elevated);
    color: var(--text-soft);
  }

  .run-btn {
    width: 100%;
    padding: 0.42rem 0.85rem;
    border-radius: var(--radius-md);
    border: 1px solid color-mix(in srgb, var(--accent) 42%, var(--border-soft));
    background: linear-gradient(
      180deg,
      color-mix(in srgb, var(--accent) 18%, var(--elevated-soft)),
      var(--elevated-soft)
    );
    color: var(--text-soft);
    font-size: 0.82rem;
    font-weight: 650;
    cursor: pointer;
    margin-left: auto;
  }

  .run-btn:disabled {
    opacity: 0.55;
    cursor: not-allowed;
  }

  .hint-label {
    font-size: 0.72rem;
    color: var(--muted);
    font-weight: 600;
  }

  .seg {
    display: grid;
    grid-template-columns: repeat(2, 1fr);
    grid-template-rows: 1fr;
    grid-template-areas: "orig trans split";
    gap: 0.2rem;
    width: 100%;
  }

  .seg-btn {
    padding: 0.28rem 0.55rem;
    border: none;
    border-radius: var(--radius-sm);
    background: transparent;
    color: var(--muted);
    font-size: 0.78rem;
    font-weight: 600;
    cursor: pointer;
  }

  .seg-btn.active {
    background: var(--elevated-soft);
    color: var(--text-soft);
    box-shadow: 0 1px 2px rgba(0, 0, 0, 0.06);
  }

  .seg-btn:disabled {
    opacity: 0.45;
    cursor: not-allowed;
  }

  .api-link {
    margin-left: auto;
    font-size: 0.74rem;
    color: var(--accent-2);
    text-decoration: none;
    font-weight: 600;
  }

  .api-link:hover {
    text-decoration: underline;
  }

  .warn {
    margin: 0;
    font-size: 0.76rem;
    line-height: 1.35;
    color: var(--muted);
  }

  .err {
    margin: 0;
    font-size: 0.78rem;
    color: var(--danger);
    line-height: 1.35;
  }
</style>
