<script lang="ts">
  import { onMount } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import { open } from "@tauri-apps/plugin-dialog";
  import { isTauriRuntime } from "$lib/isTauri";
  import { fetchLibrarySnapshot } from "$lib/library/librarySnapshotCache";
  import { assistant, ask, LM_STUDIO_URL, OLLAMA_URL, saveAssistant } from "$lib/ai/assistant.svelte";
  import { llmListModels } from "$lib/translate/llmClient";
  import { localEngines, saveTts, systemTtsAvailable, systemVoices, tts, type TtsEngine } from "$lib/reading/tts.svelte";
  import { toast, toastError } from "$lib/ui/toast.svelte";

  let syncOn = $state(false);
  let syncBusy = $state(false);
  let libraryRoot = $state<string | null>(null);

  let models = $state<string[]>([]);
  let modelsBusy = $state(false);
  let testBusy = $state(false);

  let voices = $state<SpeechSynthesisVoice[]>([]);
  let engines = $state<string[]>([]);

  onMount(() => {
    void fetchLibrarySnapshot()
      .then((s) => {
        syncOn = !!s.syncInLibrary;
        libraryRoot = s.libraryRoot;
      })
      .catch(() => {});
    void localEngines().then((e) => (engines = e));
    if (systemTtsAvailable()) {
      const upd = () => (voices = systemVoices());
      upd();
      speechSynthesis.addEventListener("voiceschanged", upd);
      return () => speechSynthesis.removeEventListener("voiceschanged", upd);
    }
  });

  async function toggleSync(on: boolean) {
    syncBusy = true;
    try {
      await invoke("set_sync_in_library", { enabled: on });
      syncOn = on;
      await fetchLibrarySnapshot();
      toast(
        on
          ? "Данные теперь хранятся в папке библиотеки (.reader)"
          : "Данные снова хранятся только на этом устройстве",
        "success",
      );
    } catch (e) {
      toastError(e, "Синхронизация");
    } finally {
      syncBusy = false;
    }
  }

  async function loadModels() {
    modelsBusy = true;
    try {
      models = await llmListModels(assistant.s.baseUrl);
      if (!models.length) toast("Сервер не вернул ни одной модели", "info");
    } catch (e) {
      toastError(e, "Модели");
    } finally {
      modelsBusy = false;
    }
  }

  async function testAssistant() {
    testBusy = true;
    try {
      const r = await ask("Отвечай одним коротким предложением по-русски.", "Скажи, что ты готов помогать с чтением книг.", 60);
      toast(r || "Модель ответила пустой строкой", r ? "success" : "info");
    } catch (e) {
      toastError(e, "Помощник");
    } finally {
      testBusy = false;
    }
  }

  async function pickPiperModel() {
    const f = await open({ multiple: false, filters: [{ name: "Piper", extensions: ["onnx"] }] });
    if (typeof f === "string") saveTts({ localVoice: f });
  }

  function testVoice() {
    const text = "Здравствуйте! Так будет звучать чтение вслух.";
    if (tts.s.engine === "system") {
      const u = new SpeechSynthesisUtterance(text);
      const v = voices.find((x) => x.voiceURI === tts.s.voiceURI);
      if (v) u.voice = v;
      u.lang = v?.lang ?? "ru-RU";
      u.rate = tts.s.rate;
      speechSynthesis.cancel();
      speechSynthesis.speak(u);
    } else {
      void invoke<string>("tts_synthesize", {
        engine: tts.s.engine,
        text,
        voice: tts.s.localVoice || null,
        rate: tts.s.rate,
      })
        .then((b64) => new Audio(`data:audio/wav;base64,${b64}`).play())
        .catch((e) => toastError(e, "Озвучка"));
    }
  }

  const ruVoices = $derived(
    [...voices].sort((a, b) => Number(b.lang.startsWith("ru")) - Number(a.lang.startsWith("ru"))),
  );
</script>

<section class="card">
  <header>
    <h2>Синхронизация</h2>
    <p>Без облака: заметки, прогресс и обложки можно хранить прямо в папке с книгами.</p>
  </header>
  {#if !isTauriRuntime()}
    <p class="muted">Доступно в приложении.</p>
  {:else}
    <label class="row">
      <input type="checkbox" checked={syncOn} disabled={syncBusy || !libraryRoot} onchange={(e) => void toggleSync(e.currentTarget.checked)} />
      <span>
        <b>Хранить данные в папке библиотеки</b>
        <small>
          Папка <code>.reader</code> внутри {libraryRoot ?? "библиотеки"}. Синхронизируйте её Syncthing, Nextcloud или
          «Яндекс Диском» — и продолжайте чтение на телефоне с того же места.
        </small>
      </span>
    </label>
  {/if}
</section>

<section class="card">
  <header>
    <h2>Помощник чтения</h2>
    <p>
      Локальная модель (LM Studio, Ollama) для пересказа «Ранее в книге…», ответа «Кто это?» и объяснения слов.
      Текст книги не покидает ваш компьютер.
    </p>
  </header>
  <div class="presets">
    <button type="button" class="chip" onclick={() => saveAssistant({ baseUrl: LM_STUDIO_URL })}>LM Studio</button>
    <button type="button" class="chip" onclick={() => saveAssistant({ baseUrl: OLLAMA_URL })}>Ollama</button>
  </div>
  <label class="field">
    <span>Адрес API (OpenAI-совместимый)</span>
    <input type="text" placeholder={LM_STUDIO_URL} value={assistant.s.baseUrl} oninput={(e) => saveAssistant({ baseUrl: e.currentTarget.value })} />
  </label>
  <div class="field-row">
    <label class="field grow">
      <span>Модель для текста</span>
      <input type="text" list="assistant-models" placeholder="например, qwen2.5-7b-instruct" value={assistant.s.model} oninput={(e) => saveAssistant({ model: e.currentTarget.value })} />
    </label>
    <button type="button" class="btn" disabled={modelsBusy || !assistant.s.baseUrl} onclick={() => void loadModels()}>
      {modelsBusy ? "…" : "Список"}
    </button>
  </div>
  <label class="field">
    <span>Модель эмбеддингов (необязательно, для «созвездия по смыслу»)</span>
    <input type="text" list="assistant-models" placeholder="например, nomic-embed-text" value={assistant.s.embedModel} oninput={(e) => saveAssistant({ embedModel: e.currentTarget.value })} />
  </label>
  <datalist id="assistant-models">
    {#each models as m (m)}<option value={m}></option>{/each}
  </datalist>
  <button type="button" class="btn" disabled={testBusy || !assistant.s.baseUrl || !assistant.s.model} onclick={() => void testAssistant()}>
    {testBusy ? "Спрашиваю модель…" : "Проверить связь"}
  </button>
</section>

<section class="card">
  <header>
    <h2>Чтение вслух</h2>
    <p>Системный голос, или локальные Piper / espeak-ng — работают без интернета.</p>
  </header>
  <div class="seg" role="group" aria-label="Движок">
    <button type="button" class:on={tts.s.engine === "system"} disabled={!systemTtsAvailable()} onclick={() => saveTts({ engine: "system" })}>Системный</button>
    <button
      type="button"
      class:on={tts.s.engine === "piper"}
      title={engines.includes("piper") ? "" : "piper не найден в PATH"}
      onclick={() => saveTts({ engine: "piper" as TtsEngine })}>Piper</button
    >
    <button
      type="button"
      class:on={tts.s.engine === "espeak-ng"}
      title={engines.includes("espeak-ng") ? "" : "espeak-ng не найден в PATH"}
      onclick={() => saveTts({ engine: "espeak-ng" as TtsEngine })}>espeak-ng</button
    >
  </div>
  {#if tts.s.engine === "system"}
    {#if !systemTtsAvailable() || voices.length === 0}
      <p class="muted">Системные голоса не найдены. На Linux поставьте speech-dispatcher или выберите Piper.</p>
    {:else}
      <label class="field">
        <span>Голос</span>
        <select value={tts.s.voiceURI} onchange={(e) => saveTts({ voiceURI: e.currentTarget.value })}>
          <option value="">По умолчанию (русский)</option>
          {#each ruVoices as v (v.voiceURI)}
            <option value={v.voiceURI}>{v.name} · {v.lang}</option>
          {/each}
        </select>
      </label>
    {/if}
  {:else if tts.s.engine === "piper"}
    <p class="muted">
      {engines.includes("piper") ? "Piper найден." : "Установите piper и добавьте его в PATH."}
      Русские голоса: <code>ru_RU-irina-medium</code>, <code>ru_RU-denis-medium</code> (файлы .onnx + .onnx.json).
    </p>
    <div class="field-row">
      <label class="field grow">
        <span>Модель голоса (.onnx)</span>
        <input type="text" value={tts.s.localVoice} oninput={(e) => saveTts({ localVoice: e.currentTarget.value })} />
      </label>
      <button type="button" class="btn" onclick={() => void pickPiperModel()}>Выбрать…</button>
    </div>
  {:else}
    <label class="field">
      <span>Голос espeak-ng</span>
      <input type="text" placeholder="ru" value={tts.s.localVoice} oninput={(e) => saveTts({ localVoice: e.currentTarget.value })} />
    </label>
  {/if}
  <label class="field">
    <span>Скорость: {tts.s.rate.toFixed(2)}×</span>
    <input type="range" min="0.6" max="2" step="0.05" value={tts.s.rate} oninput={(e) => saveTts({ rate: +e.currentTarget.value })} />
  </label>
  <button type="button" class="btn" onclick={testVoice}>Послушать</button>
</section>

<style>
  .card {
    display: flex;
    flex-direction: column;
    gap: 0.75rem;
    padding: 1.2rem 1.3rem;
    border-radius: var(--radius-lg);
    background: var(--panel-elevated);
    border: 1px solid color-mix(in srgb, var(--border-soft) 75%, transparent);
    box-shadow: var(--shadow-soft);
    color: var(--text-soft);
  }

  header h2 {
    margin: 0;
    font-size: 1.05rem;
    font-weight: 650;
  }

  header p {
    margin: 0.3rem 0 0;
    font-size: 0.86rem;
    line-height: 1.45;
    color: var(--muted);
  }

  .row {
    display: flex;
    gap: 0.7rem;
    align-items: flex-start;
    cursor: pointer;
  }

  .row input {
    margin-top: 0.2rem;
    width: 1.1rem;
    height: 1.1rem;
    accent-color: var(--accent-2);
  }

  .row span {
    display: flex;
    flex-direction: column;
    gap: 0.2rem;
  }

  .row small,
  .muted {
    font-size: 0.8rem;
    line-height: 1.45;
    color: var(--muted);
    margin: 0;
  }

  code {
    font-size: 0.78rem;
    padding: 0.05rem 0.3rem;
    border-radius: 5px;
    background: var(--panel-soft);
  }

  .presets {
    display: flex;
    gap: 0.4rem;
  }

  .chip,
  .btn {
    border: 1px solid var(--border-soft);
    background: var(--elevated-soft);
    color: var(--text-soft);
    border-radius: 999px;
    padding: 0.4rem 0.85rem;
    font-size: 0.82rem;
    cursor: pointer;
    align-self: flex-start;
  }

  .btn:disabled {
    opacity: 0.5;
    cursor: default;
  }

  .field {
    display: flex;
    flex-direction: column;
    gap: 0.3rem;
    font-size: 0.8rem;
    color: var(--muted);
  }

  .field input[type="text"],
  .field select {
    border: 1px solid var(--border-soft);
    background: var(--elevated-soft);
    color: var(--text-soft);
    border-radius: var(--radius-sm);
    padding: 0.5rem 0.65rem;
    font: inherit;
    font-size: 0.9rem;
  }

  .field input[type="range"] {
    accent-color: var(--accent-2);
  }

  .field-row {
    display: flex;
    gap: 0.5rem;
    align-items: flex-end;
  }

  .grow {
    flex: 1;
  }

  .seg {
    display: flex;
    padding: 3px;
    border-radius: 999px;
    border: 1px solid var(--border-soft);
    background: var(--panel-soft);
    align-self: flex-start;
  }

  .seg button {
    border: none;
    background: transparent;
    color: var(--muted);
    padding: 0.35rem 0.8rem;
    border-radius: 999px;
    cursor: pointer;
    font-size: 0.82rem;
  }

  .seg button.on {
    background: var(--elevated-soft);
    color: var(--text-soft);
    font-weight: 600;
  }
</style>
