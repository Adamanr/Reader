<script lang="ts">
  import type { QuoteAccent, QuoteBgFit, QuoteDraftOptions, QuoteLayout } from "$lib/types";
  import {
    ACCENT_PREVIEW_BG,
    ACCENT_QUOTE_COLOR,
    previewMargin,
    previewMaxWidth,
  } from "$lib/quoteCardStyles";
  import { copyQuotePngToClipboard } from "$lib/quoteToPng";
  import { resizeImageFileToJpegDataUrl } from "$lib/resizeImageDataUrl";

  interface Props {
    open: boolean;
    quoteText: string;
    bookTitle: string;
    bookAuthor: string;
    pageLabel: string;
    chapterLabel: string;
    onClose: () => void;
    onSave: (opts: QuoteDraftOptions) => void;
  }
  let {
    open,
    quoteText,
    bookTitle,
    bookAuthor,
    pageLabel,
    chapterLabel,
    onClose,
    onSave,
  }: Props = $props();

  let accent = $state<QuoteAccent>("sand");
  let layout = $state<QuoteLayout>("wide");
  let includePage = $state(true);
  let includeChapter = $state(false);
  let editTitle = $state("");
  let editAuthor = $state("");
  let copyBusy = $state(false);
  let copyToast = $state<{ ok: boolean; text: string } | null>(null);

  let bgImageDataUrl = $state<string | null>(null);
  let bgImageOpacityPct = $state(100);
  let overlayColor = $state("#2c261c");
  let overlayOpacityPct = $state(28);
  let bgScalePct = $state(100);
  let bgFit = $state<QuoteBgFit>("cover");
  let fileInput = $state<HTMLInputElement | null>(null);
  let imageBusy = $state(false);

  $effect(() => {
    if (open) {
      editTitle = bookTitle;
      editAuthor = bookAuthor;
      copyToast = null;
      bgImageDataUrl = null;
      bgImageOpacityPct = 100;
      overlayColor = "#2c261c";
      overlayOpacityPct = 28;
      bgScalePct = 100;
      bgFit = "cover";
    }
  });

  const accents: { value: QuoteAccent; label: string }[] = [
    { value: "sand", label: "Песок" },
    { value: "sage", label: "Шалфей" },
    { value: "dustyRose", label: "Пыльная роза" },
    { value: "ink", label: "Чернила" },
  ];

  const layouts: { value: QuoteLayout; label: string; hint: string }[] = [
    { value: "wide", label: "Широкая", hint: "На всю колонку" },
    { value: "narrow", label: "Узкая", hint: "Компактная колонка" },
    { value: "center", label: "Центр", hint: "По центру" },
  ];

  const fits: { value: QuoteBgFit; label: string }[] = [
    { value: "cover", label: "Заполнить (обрезать)" },
    { value: "contain", label: "Вписать целиком" },
  ];

  function draftOptions(): QuoteDraftOptions {
    return {
      accent,
      layout,
      includePage,
      includeChapter,
      bookTitle: editTitle.trim(),
      bookAuthor: editAuthor.trim(),
      bgImageDataUrl,
      bgImageOpacity: bgImageOpacityPct / 100,
      overlayColor,
      overlayOpacity: overlayOpacityPct / 100,
      bgScale: bgScalePct,
      bgFit,
    };
  }

  function save() {
    onSave(draftOptions());
    onClose();
  }

  async function onPickFile(e: Event) {
    const inp = e.currentTarget as HTMLInputElement;
    const f = inp.files?.[0];
    inp.value = "";
    if (!f || !f.type.startsWith("image/")) return;
    imageBusy = true;
    try {
      bgImageDataUrl = await resizeImageFileToJpegDataUrl(f, 1400, 0.82);
    } catch {
      copyToast = { ok: false, text: "Не удалось загрузить изображение" };
    } finally {
      imageBusy = false;
    }
  }

  function clearBg() {
    bgImageDataUrl = null;
  }

  async function copyPng() {
    copyToast = null;
    copyBusy = true;
    try {
      const o = draftOptions();
      await copyQuotePngToClipboard({
        quoteText,
        bookTitle: editTitle.trim() || "Без названия",
        bookAuthor: editAuthor.trim(),
        pageLabel,
        chapterLabel,
        accent,
        layout,
        includePage,
        includeChapter,
        bgImageDataUrl: o.bgImageDataUrl,
        bgImageOpacity: o.bgImageOpacity,
        overlayColor: o.overlayColor,
        overlayOpacity: o.overlayOpacity,
        bgScale: o.bgScale,
        bgFit: o.bgFit,
      });
      copyToast = { ok: true, text: "PNG в буфере обмена" };
    } catch (e) {
      copyToast = {
        ok: false,
        text: e instanceof Error ? e.message : "Не удалось скопировать изображение",
      };
    } finally {
      copyBusy = false;
    }
  }
</script>

{#if open}
  <!-- svelte-ignore a11y_click_events_have_key_events -->
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <div class="backdrop" onclick={onClose}>
    <div
      class="dlg"
      onclick={(e) => e.stopPropagation()}
      role="dialog"
      aria-modal="true"
      aria-labelledby="quote-dlg-title"
      tabindex="-1"
    >
      <header class="hd">
        <div class="hd-lead">
          <span class="hd-mark" aria-hidden="true">❝</span>
          <div class="hd-text">
            <h3 id="quote-dlg-title">Цитата</h3>
            <p class="hd-sub">Оформление, подпись и экспорт в PNG</p>
          </div>
        </div>
        <button type="button" class="x" onclick={onClose} aria-label="Закрыть">×</button>
      </header>

      <div class="studio">
        <section class="preview-pane" aria-label="Предпросмотр">
          <span class="pane-kicker">Предпросмотр</span>
          <div class="preview-frame">
            <div class="preview-wrap" class:centered={layout === "center"}>
              <div
                class="quote-preview"
                style:max-width={previewMaxWidth(layout)}
                style:margin={previewMargin(layout)}
              >
                <div class="qp-stack">
                  {#if !bgImageDataUrl}
                    <div class="qp-fill qp-gradient" style:background={ACCENT_PREVIEW_BG[accent]}></div>
                  {:else}
                    <div class="qp-fill qp-clip">
                      <div
                        class="qp-img"
                        style:background-image="url({bgImageDataUrl})"
                        style:opacity={bgImageOpacityPct / 100}
                        style:background-size={bgFit === "cover" ? "cover" : "contain"}
                        style:transform="scale({bgScalePct / 100})"
                      ></div>
                    </div>
                  {/if}
                  {#if overlayOpacityPct > 0}
                    <div
                      class="qp-overlay"
                      style:background-color={overlayColor}
                      style:opacity={overlayOpacityPct / 100}
                    ></div>
                  {/if}
                  <div class="qp-front">
                    <p class="quote-text" style:color={ACCENT_QUOTE_COLOR[accent]}>{quoteText}</p>
                    <footer class="quote-meta">
                      <span class="qt">{editTitle.trim() || "Без названия"}</span>
                      {#if editAuthor.trim()}
                        <span class="qa">{editAuthor.trim()}</span>
                      {/if}
                      {#if includePage && pageLabel}
                        <span class="qm">{pageLabel}</span>
                      {/if}
                      {#if includeChapter && chapterLabel}
                        <span class="qm">{chapterLabel}</span>
                      {/if}
                    </footer>
                  </div>
                </div>
              </div>
            </div>
          </div>

          <div class="preview-actions">
            <button
              type="button"
              class="btn-copy"
              onclick={() => void copyPng()}
              disabled={copyBusy}
            >
              <span class="btn-copy-icon" aria-hidden="true">⎘</span>
              {copyBusy ? "Копируем…" : "Скопировать PNG"}
            </button>
            {#if copyToast}
              <span class="copy-toast" class:ok={copyToast.ok} class:err={!copyToast.ok}>
                {copyToast.text}
              </span>
            {/if}
          </div>
        </section>

        <aside class="controls-pane" aria-label="Настройки карточки">
          <div class="settings-stack">
            <section class="settings-block">
              <h4 class="sec-kicker">Подпись</h4>
              <label class="inp">
                <span class="inp-label">Название на карточке</span>
                <input type="text" bind:value={editTitle} placeholder="Книга" autocomplete="off" />
              </label>
              <label class="inp">
                <span class="inp-label">Автор</span>
                <input type="text" bind:value={editAuthor} placeholder="По желанию" autocomplete="off" />
              </label>
              <p class="inp-hint">
                Только для этой цитаты. Название в каталоге — на главной странице.
              </p>
            </section>

            <section class="settings-block">
              <h4 class="sec-kicker">Стиль</h4>
              <p class="micro-label">Цветовой акцент</p>
              <div class="accent-grid" role="group" aria-label="Акцент">
                {#each accents as a (a.value)}
                  <button
                    type="button"
                    class="accent-chip"
                    class:active={accent === a.value}
                    onclick={() => (accent = a.value)}
                    aria-pressed={accent === a.value}
                  >
                    <span class="accent-swatch" style:background={ACCENT_PREVIEW_BG[a.value]}></span>
                    {a.label}
                  </button>
                {/each}
              </div>

              <p class="micro-label spaced">Ширина карточки</p>
              <div class="layout-seg" role="group" aria-label="Вёрстка">
                {#each layouts as L (L.value)}
                  <button
                    type="button"
                    class="layout-btn"
                    class:active={layout === L.value}
                    onclick={() => (layout = L.value)}
                    aria-pressed={layout === L.value}
                    title={L.hint}
                  >
                    <span class="layout-name">{L.label}</span>
                    <span class="layout-hint">{L.hint}</span>
                  </button>
                {/each}
              </div>

              <div class="toggle-grid">
                <label class="toggle">
                  <input type="checkbox" bind:checked={includePage} />
                  <span class="toggle-text">Показывать страницу</span>
                </label>
                <label class="toggle">
                  <input type="checkbox" bind:checked={includeChapter} />
                  <span class="toggle-text">Показывать главу</span>
                </label>
              </div>
            </section>

            <section class="settings-block settings-block-last">
              <h4 class="sec-kicker">Фон</h4>
              <input
                bind:this={fileInput}
                type="file"
                accept="image/*"
                class="file-inp"
                onchange={onPickFile}
              />
              <div class="bg-row">
                <button
                  type="button"
                  class="btn-file"
                  onclick={() => fileInput?.click()}
                  disabled={imageBusy}
                >
                  {imageBusy ? "Загрузка…" : bgImageDataUrl ? "Заменить изображение" : "Добавить фото"}
                </button>
                {#if bgImageDataUrl}
                  <button type="button" class="btn-text" onclick={clearBg}>Убрать фото</button>
                {/if}
              </div>

              <label class="inp">
                <span class="inp-label range-label">
                  Яркость фото <strong>{bgImageOpacityPct}%</strong>
                </span>
                <input type="range" min="8" max="100" bind:value={bgImageOpacityPct} />
              </label>

              <label class="inp">
                <span class="inp-label">Как вписать</span>
                <select class="sel" bind:value={bgFit}>
                  {#each fits as f (f.value)}
                    <option value={f.value}>{f.label}</option>
                  {/each}
                </select>
              </label>

              <label class="inp">
                <span class="inp-label range-label">
                  Масштаб фона <strong>{bgScalePct}%</strong>
                </span>
                <input type="range" min="40" max="200" bind:value={bgScalePct} />
              </label>

              <div class="color-row">
                <label class="inp inp-inline">
                  <span class="inp-label">Тон поверх фото</span>
                  <input type="color" class="color-inp" bind:value={overlayColor} title="Цвет тинта" />
                </label>
                <label class="inp inp-grow">
                  <span class="inp-label range-label">
                    Сила тона <strong>{overlayOpacityPct}%</strong>
                  </span>
                  <input type="range" min="0" max="95" bind:value={overlayOpacityPct} />
                </label>
              </div>
            </section>
          </div>
        </aside>
      </div>

      <footer class="ft">
        <button type="button" class="btn ghost-ft" onclick={onClose}>Отмена</button>
        <button type="button" class="btn primary" onclick={save}>Сохранить в заметки</button>
      </footer>
    </div>
  </div>
{/if}

<style>
  .backdrop {
    position: fixed;
    inset: 0;
    z-index: 500;
    background: color-mix(in srgb, #141008 62%, transparent);
    display: flex;
    align-items: center;
    justify-content: center;
    padding: clamp(0.75rem, 3vw, 1.5rem);
    backdrop-filter: blur(14px);
  }

  .dlg {
    width: min(1020px, 100%);
    max-height: min(94dvh, 880px);
    display: flex;
    flex-direction: column;
    overflow: hidden;
    border-radius: 20px;
    border: 1px solid color-mix(in srgb, var(--border-soft) 80%, #c9bfb2);
    background: linear-gradient(
      165deg,
      color-mix(in srgb, var(--panel-elevated) 100%, #fff 4%) 0%,
      var(--elevated-soft) 100%
    );
    box-shadow:
      0 28px 80px rgba(18, 14, 10, 0.35),
      0 0 0 1px rgba(255, 255, 255, 0.5) inset;
    color-scheme: light dark;
  }

  .hd {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 1rem;
    padding: 1.15rem 1.35rem 1rem;
    flex-shrink: 0;
    border-bottom: 1px solid color-mix(in srgb, var(--border-soft) 65%, transparent);
    background: linear-gradient(
      180deg,
      color-mix(in srgb, var(--panel-elevated) 92%, transparent),
      transparent
    );
  }

  .hd-lead {
    display: flex;
    align-items: center;
    gap: 0.85rem;
    min-width: 0;
  }

  .hd-mark {
    font-size: 2rem;
    line-height: 1;
    opacity: 0.35;
    font-family: Georgia, serif;
    user-select: none;
  }

  .hd-text h3 {
    margin: 0;
    font-size: 1.28rem;
    font-weight: 650;
    letter-spacing: -0.03em;
    font-family: system-ui, sans-serif;
    color: var(--text-soft);
  }

  .hd-sub {
    margin: 0.15rem 0 0;
    font-size: 0.84rem;
    color: var(--muted);
    line-height: 1.35;
  }

  .x {
    border: none;
    background: color-mix(in srgb, var(--elevated-soft) 95%, transparent);
    width: 2.5rem;
    height: 2.5rem;
    border-radius: 12px;
    font-size: 1.45rem;
    line-height: 1;
    cursor: pointer;
    color: var(--muted);
    flex-shrink: 0;
    border: 1px solid var(--border-soft);
    transition:
      background 0.15s ease,
      color 0.15s ease;
  }

  .x:hover {
    color: var(--text-soft);
    background: var(--panel-soft);
  }

  .studio {
    display: grid;
    grid-template-columns: minmax(0, 1fr) minmax(280px, 380px);
    grid-template-rows: minmax(0, 1fr);
    gap: 0;
    min-height: 0;
    flex: 1 1 auto;
    overflow: hidden;
  }

  @media (max-width: 880px) {
    .studio {
      grid-template-columns: 1fr;
      grid-template-rows: auto;
      overflow: auto;
      align-content: start;
    }
  }

  .preview-pane {
    display: flex;
    flex-direction: column;
    gap: 0.85rem;
    padding: 1.1rem 1.25rem 1.25rem;
    min-width: 0;
    min-height: 0;
    border-right: 1px solid color-mix(in srgb, var(--border-soft) 55%, transparent);
    background: linear-gradient(
      180deg,
      color-mix(in srgb, var(--accent) 5%, transparent),
      transparent 42%
    );
  }

  @media (max-width: 880px) {
    .preview-pane {
      border-right: none;
      border-bottom: 1px solid color-mix(in srgb, var(--border-soft) 55%, transparent);
    }
  }

  .pane-kicker {
    font-size: 0.68rem;
    text-transform: uppercase;
    letter-spacing: 0.14em;
    font-weight: 650;
    color: var(--muted);
  }

  .preview-frame {
    flex: 1 1 auto;
    min-height: clamp(13.5rem, 32vh, 24rem);
    border-radius: 16px;
    padding: 1rem;
    background: color-mix(in srgb, #1a1510 6%, var(--elevated-soft));
    border: 1px solid color-mix(in srgb, var(--border-soft) 70%, transparent);
    box-shadow:
      inset 0 1px 0 rgba(255, 255, 255, 0.35),
      inset 0 -2px 12px rgba(20, 16, 12, 0.06);
    overflow: auto;
  }

  .preview-wrap {
    width: 100%;
  }

  .preview-wrap.centered {
    display: flex;
    justify-content: center;
  }

  .quote-preview {
    position: relative;
    padding: 0;
    border-radius: 14px;
    border: 1px solid color-mix(in srgb, #3d3833 14%, transparent);
    box-shadow:
      0 12px 36px rgba(18, 14, 10, 0.12),
      0 0 0 1px rgba(255, 255, 255, 0.25) inset;
    overflow: hidden;
  }

  .qp-stack {
    position: relative;
    min-height: 6.5rem;
  }

  .qp-fill {
    position: absolute;
    inset: 0;
    pointer-events: none;
  }

  .qp-gradient {
    border-radius: inherit;
  }

  .qp-clip {
    overflow: hidden;
    border-radius: inherit;
  }

  .qp-img {
    position: absolute;
    inset: 0;
    background-position: center center;
    background-repeat: no-repeat;
    transform-origin: center center;
    will-change: transform;
  }

  .qp-overlay {
    position: absolute;
    inset: 0;
    pointer-events: none;
    border-radius: inherit;
  }

  .qp-front {
    position: relative;
    z-index: 2;
    padding: 1.35rem 1.45rem;
  }

  .quote-text {
    margin: 0 0 1.05rem;
    font-size: 1.08rem;
    line-height: 1.62;
    font-family: Georgia, "Times New Roman", serif;
    white-space: pre-wrap;
    text-shadow: 0 0 14px color-mix(in srgb, var(--elevated-soft) 60%, transparent);
  }

  .quote-meta {
    display: flex;
    flex-wrap: wrap;
    gap: 0.35rem 0.85rem;
    font-size: 0.8rem;
    opacity: 0.94;
    font-family: system-ui, sans-serif;
    text-shadow: 0 1px 12px color-mix(in srgb, var(--elevated-soft) 75%, transparent);
  }

  .qt {
    font-weight: 650;
  }

  .qa::before {
    content: "· ";
    font-weight: 400;
    opacity: 0.7;
  }

  .qm {
    opacity: 0.82;
    font-size: 0.76rem;
  }

  .preview-actions {
    display: flex;
    flex-direction: column;
    align-items: stretch;
    gap: 0.45rem;
  }

  .btn-copy {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    gap: 0.5rem;
    padding: 0.65rem 1rem;
    border-radius: 12px;
    border: 1px dashed color-mix(in srgb, var(--accent) 42%, var(--border-soft));
    background: linear-gradient(
      180deg,
      color-mix(in srgb, var(--accent) 14%, var(--elevated-soft)),
      color-mix(in srgb, var(--accent) 6%, var(--elevated-soft))
    );
    color: var(--text-soft);
    font-size: 0.9rem;
    font-weight: 600;
    cursor: pointer;
    font-family: system-ui, sans-serif;
    transition:
      filter 0.15s ease,
      border-color 0.15s ease;
  }

  .btn-copy:hover:not(:disabled) {
    filter: brightness(1.03);
    border-color: color-mix(in srgb, var(--accent) 55%, var(--border-soft));
  }

  .btn-copy:disabled {
    opacity: 0.55;
    cursor: not-allowed;
  }

  .btn-copy-icon {
    font-size: 1.1rem;
    opacity: 0.85;
  }

  .copy-toast {
    font-size: 0.78rem;
    line-height: 1.4;
    padding: 0.35rem 0.5rem;
    text-align: center;
    border-radius: 8px;
  }

  .copy-toast.ok {
    color: color-mix(in srgb, var(--accent-2) 85%, var(--muted));
    background: color-mix(in srgb, var(--accent) 8%, transparent);
  }

  .copy-toast.err {
    color: var(--danger);
    background: color-mix(in srgb, var(--danger) 12%, transparent);
  }

  .controls-pane {
    display: block;
    min-height: 0;
    min-width: 0;
    padding: 0.85rem 1.15rem 1.15rem;
    overflow-y: auto;
    overflow-x: hidden;
    scrollbar-gutter: stable;
    background: color-mix(in srgb, var(--panel-soft) 88%, transparent);
    -webkit-overflow-scrolling: touch;
  }

  .controls-pane::-webkit-scrollbar {
    width: 8px;
  }

  .controls-pane::-webkit-scrollbar-thumb {
    background: color-mix(in srgb, var(--border-soft) 80%, transparent);
    border-radius: 99px;
  }

  .settings-stack {
    display: flex;
    flex-direction: column;
    gap: 0;
  }

  .settings-block {
    padding: 0.85rem 0 1.05rem;
    border-bottom: 1px solid color-mix(in srgb, var(--border-soft) 65%, transparent);
    display: flex;
    flex-direction: column;
    gap: 0.65rem;
  }

  .settings-block-last {
    border-bottom: none;
    padding-bottom: 0.35rem;
  }

  .sec-kicker {
    margin: 0 0 0.1rem;
    font-size: 0.68rem;
    text-transform: uppercase;
    letter-spacing: 0.14em;
    font-weight: 700;
    color: var(--muted);
    font-family: system-ui, sans-serif;
  }

  .inp {
    display: flex;
    flex-direction: column;
    gap: 0.32rem;
  }

  .inp-label {
    font-size: 0.74rem;
    font-weight: 550;
    color: var(--muted);
  }

  .range-label strong {
    font-variant-numeric: tabular-nums;
    font-weight: 650;
    color: color-mix(in srgb, var(--accent-2) 70%, var(--text-soft));
  }

  .inp input[type="text"],
  .sel {
    padding: 0.55rem 0.65rem;
    border-radius: 10px;
    border: 1px solid var(--border-soft);
    background: var(--elevated-soft);
    color: var(--text-soft);
    font-size: 0.9rem;
    transition:
      border-color 0.15s ease,
      box-shadow 0.15s ease;
  }

  .inp input[type="text"]:focus,
  .sel:focus {
    outline: none;
    border-color: color-mix(in srgb, var(--accent) 45%, var(--border-soft));
    box-shadow: 0 0 0 3px color-mix(in srgb, var(--accent) 16%, transparent);
  }

  .inp input[type="range"] {
    width: 100%;
    padding: 0.3rem 0;
    accent-color: var(--accent);
    cursor: pointer;
  }

  .sel {
    cursor: pointer;
    appearance: none;
    background-image: url("data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' width='12' height='8' viewBox='0 0 12 8'%3E%3Cpath fill='%238a817a' d='M1 1.5L6 6l5-4.5'/%3E%3C/svg%3E");
    background-repeat: no-repeat;
    background-position: right 0.65rem center;
    padding-right: 2rem;
  }

  .sel option {
    background-color: var(--panel-elevated);
    color: var(--text-soft);
  }

  .inp-hint {
    margin: -0.15rem 0 0;
    font-size: 0.72rem;
    line-height: 1.45;
    color: var(--muted);
  }

  .micro-label {
    margin: 0;
    font-size: 0.68rem;
    text-transform: uppercase;
    letter-spacing: 0.1em;
    font-weight: 650;
    color: var(--muted);
  }

  .micro-label.spaced {
    margin-top: 0.35rem;
  }

  .accent-grid {
    display: grid;
    grid-template-columns: repeat(2, minmax(0, 1fr));
    gap: 0.45rem;
  }

  .accent-chip {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    padding: 0.45rem 0.55rem;
    border-radius: 11px;
    border: 1px solid color-mix(in srgb, var(--border-soft) 85%, transparent);
    background: var(--elevated-soft);
    cursor: pointer;
    font-size: 0.8rem;
    font-weight: 550;
    color: var(--text-soft);
    text-align: left;
    font-family: system-ui, sans-serif;
    transition:
      border-color 0.15s ease,
      box-shadow 0.15s ease,
      transform 0.1s ease;
  }

  .accent-chip:hover {
    border-color: color-mix(in srgb, var(--accent) 35%, var(--border-soft));
  }

  .accent-chip.active {
    border-color: color-mix(in srgb, var(--accent) 55%, var(--border-soft));
    box-shadow: 0 0 0 2px color-mix(in srgb, var(--accent) 22%, transparent);
    transform: translateY(-1px);
  }

  .accent-swatch {
    width: 1.75rem;
    height: 1.75rem;
    border-radius: 8px;
    flex-shrink: 0;
    border: 1px solid color-mix(in srgb, #000 12%, transparent);
    box-shadow: inset 0 1px 0 rgba(255, 255, 255, 0.35);
  }

  .layout-seg {
    display: flex;
    flex-direction: column;
    gap: 0.4rem;
  }

  .layout-btn {
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: 0.12rem;
    padding: 0.55rem 0.65rem;
    border-radius: 11px;
    border: 1px solid color-mix(in srgb, var(--border-soft) 85%, transparent);
    background: color-mix(in srgb, var(--elevated-soft) 96%, transparent);
    cursor: pointer;
    text-align: left;
    transition:
      border-color 0.15s ease,
      background 0.15s ease,
      box-shadow 0.15s ease;
  }

  .layout-btn:hover {
    border-color: color-mix(in srgb, var(--accent) 28%, var(--border-soft));
  }

  .layout-btn.active {
    border-color: color-mix(in srgb, var(--accent) 50%, var(--border-soft));
    background: color-mix(in srgb, var(--accent) 12%, var(--elevated-soft));
    box-shadow: 0 0 0 2px color-mix(in srgb, var(--accent) 15%, transparent);
  }

  .layout-name {
    font-size: 0.86rem;
    font-weight: 650;
    color: var(--text-soft);
    font-family: system-ui, sans-serif;
  }

  .layout-hint {
    font-size: 0.72rem;
    color: var(--muted);
    line-height: 1.25;
  }

  .toggle-grid {
    display: flex;
    flex-direction: column;
    gap: 0.5rem;
    margin-top: 0.25rem;
    padding-top: 0.65rem;
    border-top: 1px dashed color-mix(in srgb, var(--border-soft) 75%, transparent);
  }

  .toggle {
    display: flex;
    align-items: center;
    gap: 0.6rem;
    cursor: pointer;
    font-size: 0.84rem;
    color: var(--text-soft);
  }

  .toggle input {
    width: 1.05rem;
    height: 1.05rem;
    flex-shrink: 0;
    accent-color: var(--accent);
    cursor: pointer;
  }

  .toggle-text {
    font-weight: 500;
    line-height: 1.4;
  }

  .file-inp {
    position: absolute;
    width: 0;
    height: 0;
    opacity: 0;
    pointer-events: none;
  }

  .bg-row {
    display: flex;
    flex-wrap: wrap;
    gap: 0.5rem;
    align-items: center;
  }

  .btn-file {
    padding: 0.48rem 0.85rem;
    border-radius: 10px;
    border: 1px solid color-mix(in srgb, var(--accent) 38%, var(--border-soft));
    background: linear-gradient(
      180deg,
      color-mix(in srgb, var(--accent) 16%, var(--elevated-soft)),
      color-mix(in srgb, var(--accent) 7%, var(--elevated-soft))
    );
    color: var(--text-soft);
    font-size: 0.82rem;
    font-weight: 600;
    cursor: pointer;
    font-family: system-ui, sans-serif;
  }

  .btn-file:disabled {
    opacity: 0.55;
    cursor: not-allowed;
  }

  .btn-text {
    border: none;
    background: none;
    color: var(--accent-2);
    font-size: 0.8rem;
    font-weight: 550;
    cursor: pointer;
    text-decoration: underline;
    text-underline-offset: 3px;
    font-family: system-ui, sans-serif;
  }

  .color-row {
    display: flex;
    flex-wrap: wrap;
    gap: 0.75rem;
    align-items: flex-end;
  }

  .inp-inline {
    flex: 0 0 auto;
    min-width: 7rem;
  }

  .inp-grow {
    flex: 1 1 160px;
    min-width: 0;
  }

  .color-inp {
    width: 100%;
    height: 2.35rem;
    padding: 3px;
    border-radius: 10px;
    border: 1px solid var(--border-soft);
    background: var(--elevated-soft);
    cursor: pointer;
  }

  .ft {
    display: flex;
    justify-content: flex-end;
    align-items: center;
    gap: 0.65rem;
    padding: 1rem 1.35rem 1.15rem;
    flex-shrink: 0;
    border-top: 1px solid color-mix(in srgb, var(--border-soft) 65%, transparent);
    background: linear-gradient(0deg, var(--elevated-soft) 70%, transparent);
  }

  .btn {
    padding: 0.58rem 1.15rem;
    border-radius: 12px;
    border: 1px solid var(--border-soft);
    background: var(--elevated-soft);
    cursor: pointer;
    font-size: 0.92rem;
    color: var(--text-soft);
    font-family: system-ui, sans-serif;
    font-weight: 550;
    transition:
      filter 0.12s ease,
      box-shadow 0.12s ease;
  }

  .ghost-ft {
    background: transparent;
    border-color: transparent;
    color: var(--muted);
  }

  .ghost-ft:hover {
    color: var(--text-soft);
    background: color-mix(in srgb, var(--panel-soft) 90%, transparent);
  }

  .btn.primary {
    background: linear-gradient(
      180deg,
      color-mix(in srgb, var(--accent) 26%, var(--elevated-soft)),
      color-mix(in srgb, var(--accent) 14%, var(--elevated-soft))
    );
    border-color: color-mix(in srgb, var(--accent) 45%, var(--border-soft));
    font-weight: 650;
    box-shadow: 0 2px 12px color-mix(in srgb, var(--accent) 18%, transparent);
  }

  .btn.primary:hover {
    filter: brightness(1.03);
  }
</style>
