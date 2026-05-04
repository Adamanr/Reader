<script lang="ts">
  import { onMount } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import { isTauriRuntime } from "$lib/isTauri";
  import { ensureBuiltinTypstThemes } from "$lib/typst/exportToTypst";

  let busy = $state(false);
  let hint = $state<string | null>(null);
  let themes = $state<string[]>([]);
  let defaultStylePath = $state("");
  let typstCli = $state<string | null>(null);

  async function refresh() {
    if (!isTauriRuntime()) return;
    hint = null;
    busy = true;
    try {
      await ensureBuiltinTypstThemes();
      themes = await invoke<string[]>("list_typst_theme_files");
      const snap = await invoke<{
        defaultTypstStyleRelativePath?: string | null;
      }>("get_library_snapshot");
      defaultStylePath = snap.defaultTypstStyleRelativePath?.trim() ?? "";
      typstCli = await invoke<string | null>("typst_cli_version");
    } catch (e) {
      hint = String(e);
    } finally {
      busy = false;
    }
  }

  onMount(() => {
    void refresh();
  });

  async function saveDefault() {
    hint = null;
    busy = true;
    try {
      const v = defaultStylePath.trim();
      await invoke("set_default_typst_style", {
        relativePath: v.length ? v : null,
      });
      hint = "Сохранено.";
      await refresh();
    } catch (e) {
      hint = String(e);
    } finally {
      busy = false;
    }
  }
</script>

<section class="card" aria-labelledby="typst-settings-heading">
  <h2 id="typst-settings-heading">Книги в Typst</h2>
  <p class="hint">
    Стили — файлы <code>.typ</code> в папке библиотеки <code>.reader-typst-themes/</code>. При экспорте из PDF, EPUB или FB2
    копируется выбранный шаблон в <code>theme.typ</code> рядом с книгой; его можно править отдельно для каждого проекта.
  </p>
  {#if !isTauriRuntime()}
    <p class="hint">Доступно в приложении Tauri.</p>
  {:else}
    {#if hint}
      <p class="banner-line">{hint}</p>
    {/if}
    <p class="cli-line">
      {#if typstCli}
        <span class="ok">{typstCli}</span>
      {:else}
        <span class="warn">CLI Typst не найден в PATH — предпросмотр «Читать» будет текстом; установите Typst для SVG.</span>
      {/if}
    </p>
    <label class="field">
      <span>Общий стиль по умолчанию (относительный путь)</span>
      <input
        type="text"
        placeholder=".reader-typst-themes/minimal.typ"
        bind:value={defaultStylePath}
        disabled={busy}
      />
    </label>
    {#if themes.length > 0}
      <label class="field">
        <span>Быстрый выбор из библиотеки стилей</span>
        <select
          class="sel"
          onchange={(e) => {
            const v = (e.currentTarget as HTMLSelectElement).value;
            if (v) defaultStylePath = `.reader-typst-themes/${v}`;
          }}
        >
          <option value="">— файл —</option>
          {#each themes as t (t)}
            <option value={t}>{t}</option>
          {/each}
        </select>
      </label>
    {/if}
    <button type="button" class="btn-save" onclick={() => void saveDefault()} disabled={busy}>
      {busy ? "…" : "Сохранить стиль по умолчанию"}
    </button>
  {/if}
</section>

<style>
  .card {
    padding: 1rem;
    border-radius: var(--radius-md, 12px);
    border: 1px solid var(--border-soft);
    background: var(--elevated-soft);
    box-shadow: var(--shadow-soft);
    max-width: 28rem;
    width: 100%;
    box-sizing: border-box;
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

  .hint code {
    font-size: 0.78em;
    padding: 0.08em 0.28em;
    border-radius: 4px;
    background: color-mix(in srgb, var(--accent) 8%, transparent);
  }

  .banner-line {
    margin: 0 0 0.65rem;
    font-size: 0.82rem;
    color: var(--text-soft);
  }

  .cli-line {
    margin: 0 0 0.75rem;
    font-size: 0.78rem;
    line-height: 1.4;
  }

  .ok {
    color: #a8d4b4;
  }

  .warn {
    color: #e8c98f;
  }

  .field {
    display: flex;
    flex-direction: column;
    gap: 0.35rem;
    margin-bottom: 0.65rem;
    font-size: 0.82rem;
    color: var(--muted);
    font-family: system-ui, sans-serif;
  }

  .field input,
  .sel {
    padding: 0.45rem 0.55rem;
    border-radius: var(--radius-sm, 8px);
    border: 1px solid var(--border-soft);
    background: var(--panel-soft);
    color: var(--text-soft);
    font-size: 0.88rem;
  }

  .btn-save {
    margin-top: 0.25rem;
    padding: 0.45rem 0.9rem;
    border-radius: var(--radius-sm, 8px);
    border: 1px solid color-mix(in srgb, var(--accent) 45%, var(--border-soft));
    background: color-mix(in srgb, var(--accent) 12%, var(--elevated-soft));
    color: var(--text-soft);
    font-weight: 600;
    cursor: pointer;
    font-family: system-ui, sans-serif;
    font-size: 0.84rem;
  }

  .btn-save:disabled {
    opacity: 0.55;
    cursor: default;
  }
</style>
