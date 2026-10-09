<script lang="ts">
  import { onMount } from "svelte";
  import { isTauriRuntime } from "$lib/isTauri";
  import { OPDS_PRESETS, opdsFetch } from "$lib/sources/opds";
  import {
    addBookSource,
    loadBookSources,
    newSourceId,
    removeBookSource,
  } from "$lib/sources/store";
  import { telegramResolveChannel } from "$lib/sources/telegram";
  import type { BookSource, SourceKind } from "$lib/sources/types";
  import { toast, toastError } from "$lib/ui/toast.svelte";

  let sources = $state<BookSource[]>([]);
  let kind = $state<SourceKind>("telegram");
  let input = $state("");
  let busy = $state(false);
  let loaded = $state(false);

  onMount(() => {
    void loadBookSources()
      .then((s) => {
        sources = s.sources;
        loaded = true;
      })
      .catch((e) => toastError(e, "Источники"));
  });

  function hostOf(url: string): string {
    try {
      return new URL(url).host;
    } catch {
      return url;
    }
  }

  async function addTelegram(raw: string) {
    const info = await telegramResolveChannel(raw);
    const title = info.title?.trim() || info.username;
    sources = (
      await addBookSource({
        id: newSourceId(),
        kind: "telegram",
        label: title,
        username: info.username,
        enabled: true,
        title,
        addedAt: new Date().toISOString(),
      })
    ).sources;
    toast(`Добавлен канал «${title}»`, "success");
  }

  /** Каталог проверяем сразу: открывается ли лента и как она называется. */
  async function addOpds(raw: string) {
    const url = /^https?:\/\//i.test(raw) ? raw : `https://${raw}`;
    const feed = await opdsFetch(url);
    const title = feed.title?.trim() || hostOf(url);
    sources = (
      await addBookSource({
        id: newSourceId(),
        kind: "opds",
        label: title,
        username: "",
        url,
        enabled: true,
        title,
        addedAt: new Date().toISOString(),
      })
    ).sources;
    toast(`Добавлен каталог «${title}»`, "success");
  }

  async function addSource(value = input) {
    const raw = value.trim();
    if (!isTauriRuntime() || !raw) return;
    busy = true;
    try {
      if (kind === "telegram") await addTelegram(raw);
      else await addOpds(raw);
      input = "";
    } catch (e) {
      toastError(e, "Источник");
    } finally {
      busy = false;
    }
  }

  async function remove(id: string) {
    busy = true;
    try {
      const state = await removeBookSource(id);
      sources = state.sources;
      toast("Источник удалён", "info");
    } catch (e) {
      toastError(e, "Источник");
    } finally {
      busy = false;
    }
  }
</script>

<section class="card" id="book-sources" aria-labelledby="book-sources-h">
  <header>
    <h2 id="book-sources-h">Источники книг</h2>
    <p>
      Telegram-каналы и OPDS-каталоги (Calibre, Project Gutenberg, домашние библиотеки) с книгами
      PDF, EPUB и FB2. После добавления откройте каталог и выберите, что импортировать.
    </p>
  </header>

  {#if !isTauriRuntime()}
    <p class="muted">Доступно в приложении.</p>
  {:else}
    <div class="seg" role="radiogroup" aria-label="Тип источника">
      <button type="button" role="radio" aria-checked={kind === "telegram"} class:on={kind === "telegram"} onclick={() => (kind = "telegram")}>
        Telegram-канал
      </button>
      <button type="button" role="radio" aria-checked={kind === "opds"} class:on={kind === "opds"} onclick={() => (kind = "opds")}>
        OPDS-каталог
      </button>
    </div>

    <div class="field-row">
      <label class="field grow">
        <span>{kind === "telegram" ? "Ссылка или @username" : "Адрес каталога"}</span>
        <input
          type="text"
          bind:value={input}
          placeholder={kind === "telegram" ? "https://t.me/channel или @channel" : "https://example.org/opds"}
          onkeydown={(e) => {
            if (e.key === "Enter") {
              e.preventDefault();
              void addSource();
            }
          }}
        />
      </label>
      <button type="button" class="btn" disabled={busy || !input.trim()} onclick={() => void addSource()}>
        {busy ? "…" : "Добавить"}
      </button>
    </div>

    {#if kind === "opds"}
      <div class="presets">
        {#each OPDS_PRESETS as p (p.url)}
          <button type="button" class="preset" disabled={busy} title={p.hint} onclick={() => void addSource(p.url)}>
            + {p.label}
          </button>
        {/each}
      </div>
    {/if}

    {#if loaded && sources.length === 0}
      <p class="muted">Пока нет источников.</p>
    {:else if sources.length > 0}
      <ul class="list">
        {#each sources as s (s.id)}
          <li>
            <div class="meta">
              <span class="label">{s.label}</span>
              <span class="sub">{s.kind === "telegram" ? `Telegram · @${s.username}` : `OPDS · ${hostOf(s.url ?? "")}`}</span>
            </div>
            <button
              type="button"
              class="btn danger"
              disabled={busy}
              aria-label="Удалить {s.label}"
              onclick={() => void remove(s.id)}
            >
              Удалить
            </button>
          </li>
        {/each}
      </ul>
    {/if}

    <a class="link" href="/sources">Открыть каталог</a>
  {/if}
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

  .muted {
    font-size: 0.8rem;
    line-height: 1.45;
    color: var(--muted);
    margin: 0;
  }

  .field {
    display: flex;
    flex-direction: column;
    gap: 0.3rem;
    font-size: 0.8rem;
    color: var(--muted);
  }

  .field input {
    border: 1px solid var(--border-soft);
    background: var(--elevated-soft);
    color: var(--text-soft);
    border-radius: var(--radius-sm);
    padding: 0.5rem 0.65rem;
    font: inherit;
    font-size: 0.9rem;
  }

  .seg {
    display: inline-flex;
    align-self: flex-start;
    padding: 0.2rem;
    gap: 0.2rem;
    border-radius: 999px;
    background: color-mix(in srgb, var(--panel-soft) 80%, transparent);
  }

  .seg button {
    border: none;
    background: none;
    color: var(--muted);
    border-radius: 999px;
    padding: 0.3rem 0.8rem;
    font: inherit;
    font-size: 0.8rem;
    cursor: pointer;
  }

  .seg button.on {
    background: var(--elevated-soft);
    color: var(--text-soft);
    font-weight: 600;
  }

  .presets {
    display: flex;
    flex-wrap: wrap;
    gap: 0.4rem;
  }

  .preset {
    border: 1px dashed var(--border-soft);
    background: none;
    color: var(--accent-2);
    border-radius: 999px;
    padding: 0.3rem 0.75rem;
    font: inherit;
    font-size: 0.78rem;
    cursor: pointer;
  }

  .preset:disabled {
    opacity: 0.5;
  }

  .field-row {
    display: flex;
    gap: 0.5rem;
    align-items: flex-end;
  }

  .grow {
    flex: 1;
    min-width: 0;
  }

  .btn {
    border: 1px solid var(--border-soft);
    background: var(--elevated-soft);
    color: var(--text-soft);
    border-radius: 999px;
    padding: 0.4rem 0.85rem;
    font-size: 0.82rem;
    cursor: pointer;
    align-self: flex-start;
    white-space: nowrap;
  }

  .btn:disabled {
    opacity: 0.5;
    cursor: default;
  }

  .btn.danger {
    color: color-mix(in srgb, var(--text-soft) 70%, #b44);
  }

  .list {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 0.45rem;
  }

  .list li {
    display: flex;
    align-items: center;
    gap: 0.6rem;
    justify-content: space-between;
    padding: 0.45rem 0.55rem;
    border-radius: var(--radius-sm);
    background: color-mix(in srgb, var(--panel-soft) 80%, transparent);
  }

  .meta {
    display: flex;
    flex-direction: column;
    gap: 0.1rem;
    min-width: 0;
  }

  .label {
    font-size: 0.9rem;
    font-weight: 600;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .sub {
    font-size: 0.78rem;
    color: var(--muted);
  }

  .link {
    align-self: flex-start;
    color: var(--accent-2);
    font-size: 0.86rem;
    text-decoration: none;
  }

  .link:hover {
    text-decoration: underline;
  }
</style>
