<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { goto } from "$app/navigation";
  import { isTauriRuntime } from "$lib/isTauri";
  import {
    loadPdfBookTranslationFile,
    runPdfBookTranslation,
  } from "$lib/translate/pdfFullBookJob";
  import type { PdfBookTranslationFile } from "$lib/translate/pdfFullBookTypes";
  import { llmListModels } from "$lib/translate/llmClient";
  import {
    exportTranslatedPdfToChosenFile,
    exportTranslatedPdfToLibrary,
  } from "$lib/pdf/exportTranslatedPdf";

  interface Props {
    bookPath: string;
    bookTitle: string;
    onClose: () => void;
    onLibraryChanged?: () => void;
  }
  let { bookPath, bookTitle, onClose, onLibraryChanged }: Props = $props();

  const LM_DEFAULT = "http://127.0.0.1:1234/v1";
  const OLLAMA_DEFAULT = "http://127.0.0.1:11434/v1";

  let baseUrl = $state(LM_DEFAULT);
  let model = $state("");
  let models = $state<string[]>([]);
  let modelsErr = $state<string | null>(null);
  let sourceLang = $state("English");
  let targetLang = $state("Russian");
  let temperature = $state(0.35);
  let maxTokens = $state(8192);

  let phase = $state<"form" | "running" | "done" | "stopped">("form");
  let progressMsg = $state("");
  let progressPage = $state(0);
  let progressTotal = $state(0);
  let lastErr = $state<string | null>(null);
  let existingPartial = $state<PdfBookTranslationFile | null>(null);

  let abortCtrl: AbortController | null = null;

  let exportBusy = $state(false);
  let exportErr = $state<string | null>(null);
  let autoSaveErr = $state<string | null>(null);

  /** Превью последней сохранённой страницы во время перевода */
  let livePreviewPage = $state(0);
  let livePreviewSamples = $state<string[]>([]);
  /** После прерывания: сообщение об автосохранении черновика PDF */
  let stoppedDraftMsg = $state<string | null>(null);
  let stoppedDraftErr = $state<string | null>(null);

  async function saveExportedPdfToLibraryClick() {
    exportErr = null;
    exportBusy = true;
    try {
      await exportTranslatedPdfToLibrary(bookPath);
      onLibraryChanged?.();
    } catch (e) {
      exportErr = e instanceof Error ? e.message : String(e);
    } finally {
      exportBusy = false;
    }
  }

  async function saveExportedPdfAsFile() {
    exportErr = null;
    exportBusy = true;
    try {
      await exportTranslatedPdfToChosenFile(bookPath);
    } catch (e) {
      exportErr = e instanceof Error ? e.message : String(e);
    } finally {
      exportBusy = false;
    }
  }

  $effect(() => {
    void bookPath;
    void refreshExisting();
  });

  async function refreshExisting() {
    if (!bookPath || !isTauriRuntime()) return;
    existingPartial = await loadPdfBookTranslationFile(bookPath);
  }

  async function refreshModels() {
    modelsErr = null;
    try {
      models = await llmListModels(baseUrl.trim());
      if (models.length && !model) model = models[0] ?? "";
    } catch (e) {
      modelsErr = e instanceof Error ? e.message : String(e);
      models = [];
    }
  }

  function presetLm() {
    baseUrl = LM_DEFAULT;
    void refreshModels();
  }

  function presetOllama() {
    baseUrl = OLLAMA_DEFAULT;
    void refreshModels();
  }

  async function startJob(resume: boolean) {
    lastErr = null;
    if (!isTauriRuntime()) {
      lastErr = "Запуск только в приложении Tauri.";
      return;
    }
    if (!baseUrl.trim() || !model.trim()) {
      lastErr = "Укажите URL API и модель.";
      return;
    }

    const resumeDoc =
      resume && existingPartial?.pages && Object.keys(existingPartial.pages).length > 0
        ? existingPartial
        : null;

    if (!resume && existingPartial?.pages && Object.keys(existingPartial.pages).length > 0) {
      const ok = confirm(
        "У этой книги уже есть сохранённый перевод. Заменить целиком (текущий будет удалён)?",
      );
      if (!ok) return;
      await invoke("pdf_translation_delete", { bookRelativePath: bookPath });
      existingPartial = null;
    }

    phase = "running";
    progressPage = 0;
    progressTotal = 0;
    progressMsg = "Старт…";
    livePreviewPage = 0;
    livePreviewSamples = [];
    stoppedDraftMsg = null;
    stoppedDraftErr = null;
    abortCtrl = new AbortController();

    try {
      await runPdfBookTranslation({
        bookRelativePath: bookPath,
        baseUrl: baseUrl.trim(),
        model: model.trim(),
        sourceLang: sourceLang.trim() || "source",
        targetLang: targetLang.trim() || "target",
        temperature,
        maxTokens: Number.isFinite(maxTokens) ? maxTokens : null,
        resumeFromPartial: resume ? resumeDoc : null,
        signal: abortCtrl.signal,
        onProgress: (p) => {
          progressPage = p.pageIndex;
          progressTotal = p.totalPages;
          progressMsg = p.message ?? "";
          if (p.previewSamples !== undefined) {
            livePreviewPage = p.pageIndex;
            livePreviewSamples = p.previewSamples ?? [];
          }
        },
      });
      autoSaveErr = null;
      try {
        await exportTranslatedPdfToLibrary(bookPath);
        onLibraryChanged?.();
      } catch (e) {
        autoSaveErr = e instanceof Error ? e.message : String(e);
      }
      phase = "done";
    } catch (e) {
      const ename =
        e instanceof Error
          ? e.name
          : typeof e === "object" && e && "name" in e
            ? String((e as { name: unknown }).name)
            : "";
      const aborted =
        ename === "AbortError" || abortCtrl?.signal.aborted || String(e).includes("abort");
      if (aborted) {
        progressMsg = "Прерывание… сохранение черновика PDF и кэша…";
        await markInterrupted();
        phase = "stopped";
      } else {
        lastErr = e instanceof Error ? e.message : String(e);
        phase = "form";
      }
    } finally {
      abortCtrl = null;
    }
  }

  async function markInterrupted() {
    stoppedDraftMsg = null;
    stoppedDraftErr = null;
    try {
      const cur = await loadPdfBookTranslationFile(bookPath);
      if (!cur) return;
      cur.status = "interrupted";
      await invoke("pdf_translation_save", {
        bookRelativePath: bookPath,
        json: JSON.stringify(cur),
      });
      existingPartial = cur;

      const hasTranslated = Object.keys(cur.pages ?? {}).some((k) => {
        const arr = cur.pages![k];
        return Array.isArray(arr) && arr.some((s) => String(s).trim().length > 0);
      });
      if (hasTranslated) {
        try {
          await exportTranslatedPdfToLibrary(bookPath);
          onLibraryChanged?.();
          stoppedDraftMsg =
            "Черновик translate_*.pdf сохранён рядом с книгой (валидный PDF по уже готовым страницам). Можно открыть файл или читалку с превью слоя.";
        } catch (e) {
          stoppedDraftErr = e instanceof Error ? e.message : String(e);
        }
      } else {
        stoppedDraftMsg =
          "Переведённых блоков пока нет — файл PDF не создан. После появления хотя бы одной страницы прервите снова или дождитесь страницы.";
      }
    } catch (e) {
      stoppedDraftErr = e instanceof Error ? e.message : String(e);
    }
  }

  function cancelJob() {
    abortCtrl?.abort();
  }

  async function deleteTranslation() {
    if (!confirm("Удалить сохранённый перевод этой книги?")) return;
    await invoke("pdf_translation_delete", { bookRelativePath: bookPath });
    existingPartial = null;
    phase = "form";
    lastErr = null;
  }

  function pct(): number {
    if (progressTotal <= 0) return 0;
    return Math.min(100, Math.round((progressPage / progressTotal) * 100));
  }

  function openReaderPartial() {
    goto("/read?path=" + encodeURIComponent(bookPath));
    onClose();
  }
</script>

<!-- svelte-ignore a11y_click_events_have_key_events -->
<!-- svelte-ignore a11y_no_static_element_interactions -->
<div class="modal-back" onclick={onClose}>
  <div
    class="modal"
    onclick={(e) => e.stopPropagation()}
    role="dialog"
    aria-modal="true"
    tabindex="-1"
  >
    <header class="modal-h">
      <h3>Полный перевод PDF</h3>
      <button type="button" class="modal-x" onclick={onClose}>×</button>
    </header>

    <p class="modal-path">{bookTitle}</p>

    {#if phase === "form"}
      {#if existingPartial?.pages && Object.keys(existingPartial.pages).length > 0}
        <p class="banner partial">
          {#if existingPartial.status === "complete"}
            Для этой книги уже есть полный перевод (LM/Ollama).
          {:else}
            Есть сохранённый перевод ({existingPartial.status}). Можно продолжить или удалить.
          {/if}
          <button type="button" class="linkish" onclick={() => deleteTranslation()}
            >Удалить сохранённый</button>
        </p>
      {/if}

      <div class="preset-row">
        <button type="button" class="btn ghost" onclick={presetLm}>LM Studio</button>
        <button type="button" class="btn ghost" onclick={presetOllama}>Ollama</button>
        <button type="button" class="btn" onclick={() => void refreshModels()}
          >Обновить список моделей</button>
      </div>

      <label class="field">
        <span>URL API (оканчивается на /v1)</span>
        <input type="text" bind:value={baseUrl} placeholder={LM_DEFAULT} />
      </label>

      <label class="field">
        <span>Модель</span>
        {#if models.length > 0}
          <select bind:value={model}>
            {#each models as m (m)}
              <option value={m}>{m}</option>
            {/each}
          </select>
        {:else}
          <input type="text" bind:value={model} placeholder="Например llama3.2 / mistral" />
        {/if}
        {#if modelsErr}
          <span class="field-err">{modelsErr}</span>
        {/if}
      </label>

      <div class="grid2">
        <label class="field">
          <span>Исходный язык (описание для модели)</span>
          <input type="text" bind:value={sourceLang} />
        </label>
        <label class="field">
          <span>Целевой язык</span>
          <input type="text" bind:value={targetLang} />
        </label>
      </div>

      <div class="grid2">
        <label class="field">
          <span>Temperature</span>
          <input type="number" step="0.05" min="0" max="2" bind:value={temperature} />
        </label>
        <label class="field">
          <span>Max tokens (ответ)</span>
          <input type="number" step="256" min="256" bind:value={maxTokens} />
        </label>
      </div>

      {#if lastErr}
        <p class="err">{lastErr}</p>
      {/if}

      <p class="hint">
        После завершения перевода приложение само сохранит в папке книги файл
        <strong>translate_*.pdf</strong>. При необходимости можно сохранить копию в другом месте.
        Кэш перевода хранится для превью в читалке.
      </p>

      <footer class="modal-f">
        <button type="button" class="btn" onclick={onClose}>Отмена</button>
        {#if existingPartial?.pages && Object.keys(existingPartial.pages).length > 0 && existingPartial.status !== "complete"}
          <button type="button" class="btn primary" onclick={() => void startJob(true)}
            >Продолжить</button>
        {/if}
        <button type="button" class="btn primary" onclick={() => void startJob(false)}
          >Начать перевод</button>
      </footer>
    {:else if phase === "running"}
      <div class="prog">
        <div class="prog-bar" style:width="{pct()}%"></div>
      </div>
      <p class="prog-cap">{pct()}% — стр. {progressPage} / {progressTotal}</p>
      {#if progressMsg}
        <p class="muted">{progressMsg}</p>
      {/if}
      {#if livePreviewSamples.length > 0}
        <div class="live-preview">
          <p class="live-prev-cap">
            Фрагменты со стр. {livePreviewPage} (как пойдёт в PDF и читалку):
          </p>
          <ul class="live-prev-list">
            {#each livePreviewSamples as s, i (livePreviewPage + "-" + i)}
              <li>{s}</li>
            {/each}
          </ul>
        </div>
      {/if}
      <footer class="modal-f">
        <button type="button" class="btn danger" onclick={cancelJob}>Прервать</button>
      </footer>
    {:else if phase === "done"}
      <p class="ok">
        Готово. В папке с книгой должен появиться <strong>translate_*.pdf</strong> (оригинал с
        подставленным текстом). В списке у исходника — пометка «Переведена».
      </p>
      {#if autoSaveErr}
        <p class="err">Автосохранение PDF: {autoSaveErr}</p>
      {/if}
      {#if exportErr}
        <p class="err">{exportErr}</p>
      {/if}
      <footer class="modal-f">
        <button
          type="button"
          class="btn primary"
          onclick={() => void saveExportedPdfToLibraryClick()}
          disabled={exportBusy}>{exportBusy ? "Сборка PDF…" : "Повторить сохранение в библиотеку"}</button>
        <button
          type="button"
          class="btn"
          onclick={() => void saveExportedPdfAsFile()}
          disabled={exportBusy}>Другой файл…</button>
        <button type="button" class="btn" onclick={() => goto("/read?path=" + encodeURIComponent(bookPath))}
          >Открыть в читалке</button>
        <button type="button" class="btn" onclick={onClose}>Закрыть</button>
      </footer>
    {:else if phase === "stopped"}
      <p class="warn-block">
        Перевод прерван. Черновик PDF сохраняется автоматически, если уже есть переведённые страницы.
        В читалке на вкладке «Перевод» включите «Превью слоя», чтобы видеть текст поверх скана.
      </p>
      {#if stoppedDraftMsg}
        <p class="draft-ok">{stoppedDraftMsg}</p>
      {/if}
      {#if stoppedDraftErr}
        <p class="err">Черновик PDF: {stoppedDraftErr}</p>
      {/if}
      {#if exportErr}
        <p class="err">{exportErr}</p>
      {/if}
      <footer class="modal-f">
        <button
          type="button"
          class="btn primary"
          onclick={() => void saveExportedPdfToLibraryClick()}
          disabled={exportBusy}>{exportBusy ? "Сборка PDF…" : "Сохранить PDF в библиотеку (частично)"}</button>
        <button
          type="button"
          class="btn"
          onclick={() => void saveExportedPdfAsFile()}
          disabled={exportBusy}>Другой файл…</button>
        <button type="button" class="btn" onclick={openReaderPartial}
          >Читать в приложении</button>
        <button type="button" class="btn danger" onclick={() => void deleteTranslation()}
          >Удалить и заново</button>
        <button type="button" class="btn" onclick={onClose}>Закрыть</button>
      </footer>
    {/if}
  </div>
</div>

<style>
  .modal-back {
    position: fixed;
    inset: 0;
    z-index: 12000;
    background: rgba(15, 18, 28, 0.55);
    display: flex;
    align-items: center;
    justify-content: center;
    padding: 1rem;
  }
  .modal {
    width: min(520px, 100%);
    max-height: min(90vh, 900px);
    overflow: auto;
    background: var(--elevated, #1e2230);
    color: var(--text, #e8eaef);
    border-radius: 14px;
    border: 1px solid var(--border-soft, #334);
    box-shadow: 0 18px 48px rgba(0, 0, 0, 0.35);
    padding: 0 1.1rem 1rem;
  }
  .modal-h {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 0.75rem;
    margin: 0 -1.1rem;
    padding: 0.75rem 1.1rem;
    border-bottom: 1px solid var(--border-soft, #334);
  }
  .modal-h h3 {
    margin: 0;
    font-size: 1.05rem;
    font-weight: 600;
  }
  .modal-x {
    border: none;
    background: transparent;
    color: var(--muted, #9aa3b2);
    font-size: 1.35rem;
    line-height: 1;
    cursor: pointer;
    padding: 0.2rem 0.45rem;
    border-radius: 8px;
  }
  .modal-x:hover {
    background: color-mix(in srgb, var(--text, #fff) 8%, transparent);
    color: var(--text, #fff);
  }
  .modal-path {
    font-size: 0.88rem;
    color: var(--muted, #9aa3b2);
    margin: 0.65rem 0 1rem;
    word-break: break-all;
  }
  .field {
    display: flex;
    flex-direction: column;
    gap: 0.35rem;
    margin-bottom: 0.75rem;
    font-size: 0.88rem;
  }
  .field span {
    color: var(--muted, #9aa3b2);
  }
  .field input,
  .field select {
    padding: 0.45rem 0.55rem;
    border-radius: 8px;
    border: 1px solid var(--border-soft, #334);
    background: var(--panel-soft, #262a38);
    color: inherit;
  }
  .grid2 {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 0.65rem;
  }
  @media (max-width: 520px) {
    .grid2 {
      grid-template-columns: 1fr;
    }
  }
  .preset-row {
    display: flex;
    flex-wrap: wrap;
    gap: 0.45rem;
    margin-bottom: 0.65rem;
  }
  .btn {
    padding: 0.45rem 0.75rem;
    border-radius: 10px;
    border: 1px solid var(--border-soft, #334);
    background: var(--panel-soft, #262a38);
    color: inherit;
    cursor: pointer;
    font-size: 0.88rem;
  }
  .btn.primary {
    border-color: color-mix(in srgb, var(--accent, #6c9cff) 55%, transparent);
    background: color-mix(in srgb, var(--accent, #6c9cff) 22%, var(--panel-soft, #262a38));
  }
  .btn.ghost {
    background: transparent;
  }
  .btn.danger {
    border-color: color-mix(in srgb, #f66 45%, transparent);
    background: color-mix(in srgb, #f66 14%, var(--panel-soft, #262a38));
  }
  .modal-f {
    display: flex;
    flex-wrap: wrap;
    gap: 0.5rem;
    justify-content: flex-end;
    margin-top: 1rem;
    padding-top: 0.75rem;
    border-top: 1px solid var(--border-soft, #334);
  }
  .hint {
    font-size: 0.82rem;
    color: var(--muted, #9aa3b2);
    line-height: 1.45;
    margin: 0.5rem 0 0;
  }
  .err {
    color: #f88;
    font-size: 0.88rem;
    margin: 0.35rem 0;
  }
  .field-err {
    color: #f88;
    font-size: 0.78rem;
  }
  .banner.partial {
    font-size: 0.88rem;
    padding: 0.55rem 0.65rem;
    border-radius: 10px;
    background: color-mix(in srgb, var(--accent, #6c9cff) 12%, transparent);
    margin: 0 0 0.75rem;
    display: flex;
    flex-wrap: wrap;
    gap: 0.5rem;
    align-items: center;
  }
  .linkish {
    border: none;
    background: none;
    color: var(--accent, #8af);
    cursor: pointer;
    text-decoration: underline;
    padding: 0;
    font: inherit;
  }
  .prog {
    height: 10px;
    border-radius: 999px;
    background: var(--panel-soft, #262a38);
    overflow: hidden;
    margin: 0.75rem 0 0.35rem;
  }
  .prog-bar {
    height: 100%;
    background: color-mix(in srgb, var(--accent, #6c9cff) 75%, #fff);
    transition: width 0.2s ease;
  }
  .prog-cap {
    margin: 0;
    font-variant-numeric: tabular-nums;
    font-size: 0.92rem;
  }
  .muted {
    color: var(--muted, #9aa3b2);
    font-size: 0.85rem;
    margin: 0.35rem 0 0;
  }
  .ok {
    color: #9dffb8;
    margin: 0.75rem 0;
    font-size: 0.92rem;
  }
  .warn-block {
    background: color-mix(in srgb, #fa0 12%, transparent);
    padding: 0.65rem 0.75rem;
    border-radius: 10px;
    font-size: 0.9rem;
    margin: 0.5rem 0 0;
    line-height: 1.45;
  }
  .live-preview {
    margin-top: 0.65rem;
    padding: 0.55rem 0.65rem;
    border-radius: 10px;
    background: color-mix(in srgb, var(--panel-soft, #262a38) 100%, #000 8%);
    border: 1px solid var(--border-soft, #334);
    max-height: min(220px, 38vh);
    overflow: auto;
  }
  .live-prev-cap {
    margin: 0 0 0.4rem;
    font-size: 0.78rem;
    color: var(--muted, #9aa3b2);
  }
  .live-prev-list {
    margin: 0;
    padding-left: 1.1rem;
    font-size: 0.82rem;
    line-height: 1.45;
    color: var(--text, #e8eaef);
  }
  .live-prev-list li {
    margin-bottom: 0.35rem;
  }
  .draft-ok {
    color: #9dffb8;
    font-size: 0.88rem;
    margin: 0.5rem 0 0;
    line-height: 1.45;
  }
</style>
