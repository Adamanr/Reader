<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { tick } from "svelte";
  import { extractTypstOutline, type TypstOutlineItem } from "$lib/typst/outlineTypst";
  import { scrollTextareaToLine } from "$lib/typst/scrollTextareaToLine";

  interface Props {
    relativePath: string;
    displayTitle?: string;
    onOutline?: (items: TypstOutlineItem[]) => void;
    /** Одноразовый переход к строке (из сайдбара «Главы»). После обработки вызовите onGoToLineHandled. */
    goToLineRequest?: number | null;
    onGoToLineHandled?: () => void;
  }

  let {
    relativePath,
    displayTitle = "",
    onOutline,
    goToLineRequest = null,
    onGoToLineHandled,
  }: Props = $props();

  /** Первые N физических страниц — быстрый предпросмотр (typst всё равно парсит весь файл, но рендерит меньше). */
  const PREVIEW_PAGE_RANGE = "1-20";
  const LARGE_FILE_CHARS = 280_000;

  type Tab = "read" | "edit" | "style";
  type PreviewMode = "preview" | "full";

  let tab = $state<Tab>("read");
  let bookSource = $state("");
  let themeSource = $state("");
  let loading = $state(true);
  let err = $state<string | null>(null);
  let saveHint = $state<string | null>(null);
  let compileErr = $state<string | null>(null);
  let previewSvg = $state<string | null>(null);
  let typstVersion = $state<string | null>(null);
  let editorRef = $state<HTMLTextAreaElement | null>(null);
  let previewBusy = $state(false);
  let previewMode = $state<PreviewMode>("preview");
  let saveCompileTimer: ReturnType<typeof setTimeout> | null = null;
  let compileGen = 0;

  const themeRel = $derived.by(() => {
    const main = relativePath.replace(/\\/g, "/");
    const slash = main.lastIndexOf("/");
    return slash >= 0 ? `${main.slice(0, slash)}/theme.typ` : "theme.typ";
  });

  const isLargeFile = $derived(bookSource.length >= LARGE_FILE_CHARS);

  $effect(() => {
    const src = bookSource;
    const t = setTimeout(() => onOutline?.(extractTypstOutline(src)), 0);
    return () => clearTimeout(t);
  });

  $effect(() => {
    const line = goToLineRequest;
    if (line == null || line < 1) return;
    tab = "edit";
    void tick().then(() => {
      if (editorRef) scrollTextareaToLine(editorRef, line);
      onGoToLineHandled?.();
    });
  });

  async function compilePreview(mode: PreviewMode) {
    const gen = ++compileGen;
    compileErr = null;
    previewBusy = true;
    try {
      const v = await invoke<string | null>("typst_cli_version");
      if (gen !== compileGen) return;
      typstVersion = v ?? null;
      if (!v) {
        compileErr =
          "Командная строка Typst не найдена. Установите Typst и добавьте в PATH — тогда появится предпросмотр SVG.";
        return;
      }
      const pages = mode === "preview" ? PREVIEW_PAGE_RANGE : null;
      const svg = await invoke<string>("compile_typst_to_svg", {
        mainRelative: relativePath,
        pages,
      });
      if (gen !== compileGen) return;
      previewSvg = svg;
      previewMode = mode;
    } catch (e) {
      if (gen !== compileGen) return;
      compileErr = String(e);
      previewSvg = null;
    } finally {
      if (gen === compileGen) previewBusy = false;
    }
  }

  async function refreshSources() {
    compileGen++; // отменить предыдущую сборку предпросмотра
    loading = true;
    err = null;
    previewSvg = null;
    compileErr = null;
    try {
      bookSource = await invoke<string>("read_library_utf8", {
        relativePath,
      });
      try {
        themeSource = await invoke<string>("read_library_utf8", {
          relativePath: themeRel,
        });
      } catch {
        themeSource = "";
      }
    } catch (e) {
      err = String(e);
    } finally {
      loading = false;
    }
    void compilePreview("preview");
  }

  $effect(() => {
    relativePath;
    void refreshSources();
  });

  function schedulePreviewAfterSave() {
    if (saveCompileTimer) clearTimeout(saveCompileTimer);
    saveCompileTimer = setTimeout(() => {
      saveCompileTimer = null;
      void compilePreview("preview");
    }, 500);
  }

  async function saveBook() {
    saveHint = null;
    err = null;
    try {
      await invoke("write_library_utf8", {
        relativePath,
        content: bookSource,
      });
      saveHint = "Книга сохранена.";
      schedulePreviewAfterSave();
    } catch (e) {
      err = String(e);
    }
  }

  async function saveTheme() {
    saveHint = null;
    err = null;
    try {
      await invoke("write_library_utf8", {
        relativePath: themeRel,
        content: themeSource,
      });
      saveHint = "Стиль сохранён.";
      schedulePreviewAfterSave();
    } catch (e) {
      err = String(e);
    }
  }

  function wrapSelection(before: string, after: string) {
    const el = editorRef;
    if (!el) return;
    const start = el.selectionStart;
    const end = el.selectionEnd;
    const v = el.value;
    const mid = v.slice(start, end);
    el.value = v.slice(0, start) + before + mid + after + v.slice(end);
    void tick().then(() => {
      el.focus();
      el.setSelectionRange(
        start + before.length,
        start + before.length + mid.length,
      );
    });
    bookSource = el.value;
  }
</script>

<div class="typst-root">
  <div class="typst-tabs">
    <button
      type="button"
      class="tt"
      class:active={tab === "read"}
      onclick={() => (tab = "read")}>Читать</button>
    <button
      type="button"
      class="tt"
      class:active={tab === "edit"}
      onclick={() => (tab = "edit")}>Редактировать книгу</button>
    <button
      type="button"
      class="tt"
      class:active={tab === "style"}
      onclick={() => (tab = "style")}>Редактировать стиль</button>
    {#if typstVersion}
      <span class="typst-ver" title="CLI Typst">{typstVersion}</span>
    {/if}
  </div>

  {#if isLargeFile}
    <p class="typst-large">
      Большой файл ({Math.round(bookSource.length / 1024)} КиБ) — предпросмотр по умолчанию только первых страниц; при
      правке редактор может подтормаживать. Сохраняйте реже или отредактируйте <code>book.typ</code> во внешней IDE.
    </p>
  {/if}

  {#if err}
    <p class="typst-err">{err}</p>
  {/if}
  {#if saveHint}
    <p class="typst-ok">{saveHint}</p>
  {/if}

  {#if tab === "read" && !loading}
    <div class="typst-read-bar">
      {#if previewBusy}
        <span class="typst-busy" aria-live="polite">Сборка предпросмотра…</span>
      {/if}
      {#if previewMode === "preview" && previewSvg && !previewBusy}
        <span class="typst-read-meta">Показаны страницы {PREVIEW_PAGE_RANGE} (быстро).</span>
        <button
          type="button"
          class="typst-read-all"
          disabled={previewBusy}
          onclick={() => void compilePreview("full")}
        >
          Показать всю книгу
        </button>
      {:else if previewMode === "full" && previewSvg && !previewBusy}
        <span class="typst-read-meta">Полный предпросмотр (может быть тяжёлым).</span>
        <button
          type="button"
          class="typst-read-all"
          disabled={previewBusy}
          onclick={() => void compilePreview("preview")}
        >
          Только начало (быстрее)
        </button>
      {/if}
    </div>
  {/if}

  {#if loading && tab === "read"}
    <p class="typst-muted">Загрузка текста…</p>
  {:else if tab === "read"}
    <div class="typst-read">
      {#if compileErr}
        <p class="typst-warn">{compileErr}</p>
      {/if}
      {#if previewSvg}
        <div class="svg-host" aria-label="Предпросмотр Typst">{@html previewSvg}</div>
      {:else if !previewBusy}
        <pre class="typst-fallback">{bookSource.slice(0, 120_000)}{bookSource.length > 120_000 ? "\n\n… (обрезано для отображения)" : ""}</pre>
      {/if}
    </div>
  {:else if tab === "edit"}
    <div class="typst-toolbar">
      <span class="tb-label">Разметка:</span>
      <button type="button" class="tb" onclick={() => wrapSelection("*", "*")} title="Жирный">
        Жирный
      </button>
      <button type="button" class="tb" onclick={() => wrapSelection("_", "_")} title="Курсив">
        Курсив
      </button>
      <button type="button" class="tb" onclick={() => wrapSelection("`", "`")} title="Код">
        Код
      </button>
      <button
        type="button"
        class="tb"
        onclick={() => wrapSelection("\n= ", "\n")}
        title="Заголовок 1">Заголовок</button>
      <button type="button" class="tb" onclick={() => wrapSelection("\n== ", "\n")}>
        Подзаголовок
      </button>
      <button type="button" class="save" onclick={() => void saveBook()}>Сохранить книгу</button>
    </div>
    <label class="sr-only" for="typst-book-edit">{displayTitle || "Книга Typst"}</label>
    <textarea
      id="typst-book-edit"
      class="typst-editor"
      bind:this={editorRef}
      bind:value={bookSource}
      spellcheck="false"
      rows="28"
      placeholder="# Разметка Typst…"
      autocomplete="off"
    ></textarea>
    <p class="typst-hint">
      Полная правка исходника Typst. Главы в сайдбаре берутся из строк <code>=</code>, <code>==</code> и т.д.
    </p>
  {:else}
    <div class="typst-toolbar">
      <button type="button" class="save" onclick={() => void saveTheme()}>Сохранить стиль</button>
    </div>
    <label class="sr-only" for="typst-theme-edit">Тема {themeRel}</label>
    <textarea
      id="typst-theme-edit"
      class="typst-editor theme"
      bind:value={themeSource}
      spellcheck="false"
      rows="28"
      placeholder="// Тема оформления (apply-book-theme)"
      autocomplete="off"
    ></textarea>
    <p class="typst-hint">
      Файл <code>theme.typ</code> рядом с книгой. Общие шаблоны — в <code>.reader-typst-themes/</code>.
    </p>
  {/if}
</div>

<style>
  .typst-root {
    display: flex;
    flex-direction: column;
    gap: 0.65rem;
    min-height: 0;
    flex: 1;
    padding: 0.5rem clamp(0.5rem, 2vw, 1rem);
    box-sizing: border-box;
    contain: content;
  }

  .typst-tabs {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 0.35rem 0.5rem;
  }

  .tt {
    border: 1px solid var(--border-soft, #333);
    background: var(--elevated-soft, #1e1e1e);
    color: var(--text-soft, #e8e4dc);
    padding: 0.4rem 0.75rem;
    border-radius: var(--radius-sm, 8px);
    font-size: 0.84rem;
    font-weight: 600;
    cursor: pointer;
    font-family: system-ui, sans-serif;
  }

  .tt.active {
    border-color: color-mix(in srgb, var(--accent, #c9a227) 55%, var(--border-soft, #333));
    background: color-mix(in srgb, var(--accent, #c9a227) 12%, var(--elevated-soft, #1e1e1e));
  }

  .typst-ver {
    font-size: 0.75rem;
    color: var(--muted, #9a948a);
    margin-left: 0.35rem;
  }

  .typst-large {
    margin: 0;
    padding: 0.45rem 0.55rem;
    font-size: 0.78rem;
    line-height: 1.4;
    color: #e8d49a;
    background: color-mix(in srgb, var(--accent, #c9a227) 8%, transparent);
    border-radius: var(--radius-sm, 8px);
    border: 1px solid color-mix(in srgb, var(--accent, #c9a227) 22%, var(--border-soft, #444));
  }

  .typst-large code {
    font-size: 0.85em;
  }

  .typst-read-bar {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 0.45rem 0.75rem;
    font-size: 0.78rem;
    color: var(--muted, #9a948a);
  }

  .typst-busy {
    color: var(--accent-2, #d4c4a8);
    font-weight: 600;
  }

  .typst-read-meta {
    flex: 1;
    min-width: 8rem;
  }

  .typst-read-all {
    padding: 0.28rem 0.65rem;
    border-radius: 6px;
    border: 1px solid var(--border-soft, #555);
    background: var(--elevated-soft, #252220);
    color: var(--text-soft, #e8e4dc);
    font-size: 0.78rem;
    font-weight: 600;
    cursor: pointer;
    font-family: system-ui, sans-serif;
  }

  .typst-read-all:disabled {
    opacity: 0.5;
    cursor: default;
  }

  .typst-err {
    color: #f0a8a8;
    margin: 0;
    font-size: 0.88rem;
  }

  .typst-ok {
    color: #a8e4c0;
    margin: 0;
    font-size: 0.88rem;
  }

  .typst-warn {
    color: #e8d49a;
    margin: 0 0 0.5rem;
    font-size: 0.86rem;
    max-width: 52rem;
  }

  .typst-muted {
    color: var(--muted, #9a948a);
    margin: 0;
  }

  .typst-read {
    flex: 1;
    min-height: 0;
    overflow: auto;
    background: #faf8f5;
    border-radius: var(--radius-sm, 8px);
    border: 1px solid var(--border-soft, #333);
    content-visibility: auto;
  }

  .svg-host {
    padding: 1rem;
    max-width: 100%;
    overflow: auto;
  }

  .svg-host :global(.reader-typst-svg-pages) {
    display: flex;
    flex-direction: column;
    gap: 1.25rem;
  }

  .svg-host :global(.reader-typst-svg-page svg) {
    max-width: 100%;
    height: auto;
    display: block;
    margin: 0 auto;
  }

  .typst-fallback {
    margin: 0;
    padding: 1rem;
    font-size: 0.82rem;
    line-height: 1.45;
    white-space: pre-wrap;
    word-break: break-word;
    color: #1a1814;
    background: #faf8f5;
    min-height: 12rem;
  }

  .typst-toolbar {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 0.35rem;
    font-size: 0.82rem;
  }

  .tb-label {
    color: var(--muted, #9a948a);
    margin-right: 0.25rem;
  }

  .tb {
    border: 1px solid var(--border-soft, #444);
    background: var(--elevated-soft, #252220);
    color: var(--text-soft, #e8e4dc);
    padding: 0.3rem 0.55rem;
    border-radius: 6px;
    cursor: pointer;
    font-size: 0.8rem;
    font-family: system-ui, sans-serif;
  }

  .tb:hover {
    border-color: color-mix(in srgb, var(--accent, #c9a227) 40%, var(--border-soft, #444));
  }

  .save {
    margin-left: auto;
    border: 1px solid color-mix(in srgb, var(--accent, #c9a227) 45%, var(--border-soft, #444));
    background: color-mix(in srgb, var(--accent, #c9a227) 14%, var(--elevated-soft, #252220));
    color: var(--text-soft, #e8e4dc);
    padding: 0.35rem 0.85rem;
    border-radius: 8px;
    font-weight: 600;
    cursor: pointer;
    font-family: system-ui, sans-serif;
    font-size: 0.82rem;
  }

  .typst-editor {
    width: 100%;
    min-height: 50vh;
    box-sizing: border-box;
    font-family: "JetBrains Mono", "Consolas", ui-monospace, monospace;
    font-size: 0.84rem;
    line-height: 1.45;
    padding: 0.75rem 0.9rem;
    border-radius: var(--radius-sm, 8px);
    border: 1px solid var(--border-soft, #444);
    background: #141210;
    color: #f0ebe4;
    resize: vertical;
    min-height: 320px;
  }

  .typst-editor.theme {
    background: #121820;
  }

  .typst-hint {
    margin: 0;
    font-size: 0.8rem;
    color: var(--muted, #9a948a);
    max-width: 52rem;
    line-height: 1.45;
  }

  .typst-hint code {
    font-size: 0.78em;
    padding: 0.1em 0.35em;
    border-radius: 4px;
    background: color-mix(in srgb, var(--accent, #c9a227) 8%, transparent);
  }

  .sr-only {
    position: absolute;
    width: 1px;
    height: 1px;
    padding: 0;
    margin: -1px;
    overflow: hidden;
    clip: rect(0, 0, 0, 0);
    border: 0;
  }
</style>
