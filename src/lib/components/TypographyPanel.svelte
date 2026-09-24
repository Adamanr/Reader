<script lang="ts">
  import type { BookFormat } from "$lib/bookFormat";
  import { FONT_OPTIONS, ensureAppFonts, fontStack } from "$lib/reading/fonts";
  import { PAGE_THEME_OPTIONS } from "$lib/reading/palette";
  import { reading, resetTypography, TYPO_PRESETS, updateReading } from "$lib/reading/settings.svelte";
  import { THEMES, applyTheme, getStoredTheme, type ThemeId } from "$lib/theme";

  interface Props {
    format: BookFormat;
    onClose: () => void;
  }
  let { format, onClose }: Props = $props();

  ensureAppFonts();

  let appTheme = $state<ThemeId>(getStoredTheme());
  const s = $derived(reading.s);
  const textual = $derived(format === "epub" || format === "fb2");

  function setAppTheme(id: ThemeId) {
    appTheme = id;
    applyTheme(id);
  }
</script>

<!-- svelte-ignore a11y_click_events_have_key_events -->
<!-- svelte-ignore a11y_no_static_element_interactions -->
<div class="tp-back" onclick={onClose}></div>
<aside class="tp" aria-label="Оформление текста">
  <header class="tp-head">
    <span class="tp-aa" aria-hidden="true">Aa</span>
    <h2>Оформление</h2>
    <button type="button" class="tp-x" aria-label="Закрыть" onclick={onClose}>×</button>
  </header>

  <div class="tp-body">
    {#if textual}
      <section>
        <h3>Быстрые наборы</h3>
        <div class="chips">
          {#each TYPO_PRESETS as p (p.id)}
            <button type="button" class="chip" onclick={() => updateReading(p.patch)}>{p.label}</button>
          {/each}
        </div>
      </section>
    {/if}

    <section>
      <h3>Страница</h3>
      <div class="swatches">
        {#each PAGE_THEME_OPTIONS as t (t.id)}
          <button
            type="button"
            class="swatch"
            class:active={s.pageTheme === t.id}
            onclick={() => updateReading({ pageTheme: t.id })}
            title={t.label}
          >
            <span class="swatch-face" style:background={t.swatch} style:color={t.ink}>Аа</span>
            <span class="swatch-label">{t.label}</span>
          </button>
        {/each}
      </div>
      <div class="row-label">Интерфейс</div>
      <div class="chips">
        {#each THEMES as t (t.id)}
          <button type="button" class="chip" class:on={appTheme === t.id} onclick={() => setAppTheme(t.id)}>{t.label}</button>
        {/each}
      </div>
    </section>

    {#if textual || format === "pdf"}
      <section>
        <h3>Шрифт</h3>
        <div class="fonts">
          {#each FONT_OPTIONS as f (f.id)}
            {#if textual || f.id !== "publisher"}
              <button
                type="button"
                class="font"
                class:active={s.font === f.id}
                onclick={() => updateReading({ font: f.id })}
              >
                <span class="font-name" style:font-family={f.id === "publisher" ? "inherit" : fontStack(f.id)}>{f.label}</span>
                <span class="font-sample" style:font-family={f.id === "publisher" ? "inherit" : fontStack(f.id)}>{f.sample}</span>
              </button>
            {/if}
          {/each}
        </div>
      </section>

      <section>
        <h3>Текст</h3>
        <label class="slider">
          <span>Размер <b>{s.fontSize}</b></span>
          <div class="slider-row">
            <button type="button" onclick={() => updateReading({ fontSize: Math.max(12, s.fontSize - 1) })}>A−</button>
            <input type="range" min="12" max="34" step="1" value={s.fontSize} oninput={(e) => updateReading({ fontSize: +e.currentTarget.value })} />
            <button type="button" onclick={() => updateReading({ fontSize: Math.min(34, s.fontSize + 1) })}>A+</button>
          </div>
        </label>
        <label class="slider">
          <span>Межстрочный <b>{s.lineHeight.toFixed(2)}</b></span>
          <input type="range" min="1.15" max="2.3" step="0.05" value={s.lineHeight} oninput={(e) => updateReading({ lineHeight: +e.currentTarget.value })} />
        </label>
        <label class="slider">
          <span>Длина строки <b>{s.measure} зн.</b></span>
          <input type="range" min="36" max="100" step="2" value={s.measure} oninput={(e) => updateReading({ measure: +e.currentTarget.value })} />
        </label>
        <label class="slider">
          <span>Поля <b>{s.margin}px</b></span>
          <input type="range" min="0" max="96" step="4" value={s.margin} oninput={(e) => updateReading({ margin: +e.currentTarget.value })} />
        </label>
        {#if textual}
          <label class="slider">
            <span>Между абзацами <b>{s.paraSpacing.toFixed(2)}em</b></span>
            <input type="range" min="0" max="1.6" step="0.05" value={s.paraSpacing} oninput={(e) => updateReading({ paraSpacing: +e.currentTarget.value })} />
          </label>
          <label class="slider">
            <span>Разрядка <b>{Math.round(s.letterSpacing * 100)}%</b></span>
            <input
              type="range"
              min="0"
              max="0.12"
              step="0.01"
              value={s.letterSpacing}
              oninput={(e) => updateReading({ letterSpacing: +e.currentTarget.value, wordSpacing: +e.currentTarget.value * 3 })}
            />
          </label>
          <div class="toggles">
            <div class="seg" role="group" aria-label="Выравнивание">
              <button type="button" class:on={s.align === "left"} onclick={() => updateReading({ align: "left" })}>По левому краю</button>
              <button type="button" class:on={s.align === "justify"} onclick={() => updateReading({ align: "justify" })}>По ширине</button>
            </div>
            <label class="check"><input type="checkbox" checked={s.hyphens} onchange={(e) => updateReading({ hyphens: e.currentTarget.checked })} /> Переносы</label>
            <label class="check"><input type="checkbox" checked={s.indent} onchange={(e) => updateReading({ indent: e.currentTarget.checked })} /> Красная строка</label>
          </div>
        {/if}
      </section>
    {/if}

    {#if format === "epub"}
      <section>
        <h3>Листание</h3>
        <div class="seg" role="group" aria-label="Режим EPUB">
          <button type="button" class:on={s.epubFlow === "paginated"} onclick={() => updateReading({ epubFlow: "paginated" })}>Страницы</button>
          <button type="button" class:on={s.epubFlow === "scrolled"} onclick={() => updateReading({ epubFlow: "scrolled" })}>Лента</button>
        </div>
      </section>
    {/if}

    {#if format === "pdf"}
      <section>
        <h3>PDF</h3>
        <div class="seg" role="group" aria-label="Раскладка">
          <button type="button" class:on={s.pdfMode === "continuous"} onclick={() => updateReading({ pdfMode: "continuous" })}>Лента</button>
          <button type="button" class:on={s.pdfMode === "single"} onclick={() => updateReading({ pdfMode: "single" })}>Страница</button>
          <button type="button" class:on={s.pdfMode === "spread"} onclick={() => updateReading({ pdfMode: "spread" })}>Разворот</button>
        </div>
        <div class="seg" role="group" aria-label="Масштаб">
          <button type="button" class:on={s.pdfFit === "width"} onclick={() => updateReading({ pdfFit: "width" })}>По ширине</button>
          <button type="button" class:on={s.pdfFit === "page"} onclick={() => updateReading({ pdfFit: "page" })}>Вся страница</button>
          <button type="button" class:on={s.pdfFit === "custom"} onclick={() => updateReading({ pdfFit: "custom" })}>Свой</button>
        </div>
        <div class="toggles">
          <label class="check"><input type="checkbox" checked={s.pdfInvert} onchange={(e) => updateReading({ pdfInvert: e.currentTarget.checked })} /> Тёмные страницы (инверсия)</label>
          <label class="check"><input type="checkbox" checked={s.pdfCrop} onchange={(e) => updateReading({ pdfCrop: e.currentTarget.checked })} /> Обрезать белые поля</label>
        </div>
      </section>
    {/if}

    <section>
      <h3>Поведение</h3>
      <div class="row-label">Фокус</div>
      <div class="seg" role="group" aria-label="Режим фокуса">
        <button type="button" class:on={s.focus === "off"} onclick={() => updateReading({ focus: "off" })}>Выкл</button>
        {#if textual}
          <button type="button" class:on={s.focus === "paragraph"} onclick={() => updateReading({ focus: "paragraph" })}>Абзац</button>
        {/if}
        <button type="button" class:on={s.focus === "line"} onclick={() => updateReading({ focus: "line" })}>Линейка</button>
      </div>
      <div class="toggles">
        <label class="check"><input type="checkbox" checked={s.tapZones} onchange={(e) => updateReading({ tapZones: e.currentTarget.checked })} /> Листать тапом по краям</label>
        <label class="check"><input type="checkbox" checked={s.livingCover} onchange={(e) => updateReading({ livingCover: e.currentTarget.checked })} /> Живая обложка — цвета книги</label>
      </div>
    </section>

    {#if textual}
      <button type="button" class="reset" onclick={resetTypography}>Сбросить оформление текста</button>
    {/if}
  </div>
</aside>

<style>
  .tp-back {
    position: fixed;
    inset: 0;
    z-index: 300;
  }

  .tp {
    position: fixed;
    z-index: 310;
    top: 3.6rem;
    right: 0.75rem;
    bottom: 0.75rem;
    width: min(23rem, calc(100vw - 1.5rem));
    display: flex;
    flex-direction: column;
    border-radius: var(--radius-lg);
    background: var(--panel-elevated);
    border: 1px solid var(--toolbar-border);
    box-shadow: var(--shadow-float);
    color: var(--text-soft);
    animation: tp-in 0.2s ease-out;
    overflow: hidden;
  }

  .tp-head {
    display: flex;
    align-items: center;
    gap: 0.6rem;
    padding: 0.85rem 1rem 0.6rem;
    border-bottom: 1px solid var(--border-soft);
  }

  .tp-aa {
    font-family: "Literata Variable", Georgia, serif;
    font-size: 1.35rem;
    color: var(--accent-2);
  }

  .tp-head h2 {
    margin: 0;
    flex: 1;
    font-size: 1rem;
    font-weight: 650;
  }

  .tp-x {
    border: none;
    background: transparent;
    color: var(--muted);
    font-size: 1.4rem;
    cursor: pointer;
    line-height: 1;
  }

  .tp-body {
    overflow: auto;
    padding: 0.4rem 1rem 1.2rem;
  }

  section {
    padding: 0.7rem 0 0.9rem;
    border-bottom: 1px solid color-mix(in srgb, var(--border-soft) 60%, transparent);
    display: flex;
    flex-direction: column;
    gap: 0.6rem;
  }

  h3 {
    margin: 0;
    font-size: 0.7rem;
    text-transform: uppercase;
    letter-spacing: 0.12em;
    color: var(--muted);
    font-weight: 700;
  }

  .row-label {
    font-size: 0.78rem;
    color: var(--muted);
    margin-top: 0.2rem;
  }

  .chips {
    display: flex;
    flex-wrap: wrap;
    gap: 0.35rem;
  }

  .chip {
    border: 1px solid var(--border-soft);
    background: var(--elevated-soft);
    color: var(--text-soft);
    border-radius: 999px;
    padding: 0.32rem 0.75rem;
    font-size: 0.8rem;
    cursor: pointer;
  }

  .chip:hover,
  .chip.on {
    border-color: var(--accent-2);
    color: var(--accent-2);
  }

  .swatches {
    display: grid;
    grid-template-columns: repeat(6, 1fr);
    gap: 0.4rem;
  }

  .swatch {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 0.25rem;
    border: none;
    background: transparent;
    padding: 0;
    cursor: pointer;
    color: var(--muted);
  }

  .swatch-face {
    width: 100%;
    aspect-ratio: 1;
    border-radius: 12px;
    display: grid;
    place-items: center;
    font-family: "Literata Variable", Georgia, serif;
    font-size: 0.95rem;
    border: 2px solid var(--border-soft);
  }

  .swatch.active .swatch-face {
    border-color: var(--accent-2);
    box-shadow: 0 0 0 3px color-mix(in srgb, var(--accent) 30%, transparent);
  }

  .swatch-label {
    font-size: 0.64rem;
  }

  .fonts {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 0.4rem;
  }

  .font {
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: 0.15rem;
    text-align: left;
    border: 1px solid var(--border-soft);
    background: var(--elevated-soft);
    color: var(--text-soft);
    border-radius: var(--radius-sm);
    padding: 0.5rem 0.6rem;
    cursor: pointer;
  }

  .font.active {
    border-color: var(--accent-2);
    box-shadow: 0 0 0 2px color-mix(in srgb, var(--accent) 25%, transparent);
  }

  .font-name {
    font-size: 1rem;
  }

  .font-sample {
    font-size: 0.72rem;
    color: var(--muted);
    line-height: 1.3;
  }

  .slider {
    display: flex;
    flex-direction: column;
    gap: 0.25rem;
    font-size: 0.82rem;
  }

  .slider b {
    font-weight: 600;
    color: var(--accent-2);
    font-variant-numeric: tabular-nums;
  }

  .slider input {
    width: 100%;
    accent-color: var(--accent-2);
  }

  .slider-row {
    display: flex;
    align-items: center;
    gap: 0.5rem;
  }

  .slider-row button {
    border: 1px solid var(--border-soft);
    background: var(--elevated-soft);
    color: var(--text-soft);
    border-radius: 8px;
    padding: 0.2rem 0.45rem;
    font-size: 0.78rem;
    cursor: pointer;
  }

  .toggles {
    display: flex;
    flex-direction: column;
    gap: 0.45rem;
  }

  .check {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    font-size: 0.84rem;
    cursor: pointer;
  }

  .check input {
    accent-color: var(--accent-2);
    width: 1rem;
    height: 1rem;
  }

  .seg {
    display: flex;
    padding: 3px;
    gap: 2px;
    border-radius: 999px;
    background: var(--panel-soft);
    border: 1px solid var(--border-soft);
  }

  .seg button {
    flex: 1;
    border: none;
    background: transparent;
    color: var(--muted);
    border-radius: 999px;
    padding: 0.35rem 0.4rem;
    font-size: 0.78rem;
    cursor: pointer;
  }

  .seg button.on {
    background: var(--elevated-soft);
    color: var(--text-soft);
    box-shadow: 0 1px 4px rgba(0, 0, 0, 0.08);
    font-weight: 600;
  }

  .reset {
    margin-top: 0.9rem;
    width: 100%;
    border: 1px dashed var(--border-soft);
    background: transparent;
    color: var(--muted);
    border-radius: var(--radius-sm);
    padding: 0.55rem;
    cursor: pointer;
  }

  @keyframes tp-in {
    from {
      opacity: 0;
      transform: translateY(-6px);
    }
  }

  @media (max-width: 600px) {
    .tp {
      top: auto;
      left: 0;
      right: 0;
      bottom: 0;
      width: 100%;
      max-height: 78dvh;
      border-radius: var(--radius-lg) var(--radius-lg) 0 0;
      padding-bottom: env(safe-area-inset-bottom);
    }
  }
</style>
