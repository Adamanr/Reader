<script lang="ts">
  import { untrack } from "svelte";
  import type { BookMeta, Importance, ReadingStatus, Shelf } from "$lib/types";
  import { IMPORTANCE_OPTIONS, READING_STATUS_OPTIONS } from "$lib/types";
  import { effectiveShelfIds } from "$lib/library/shelves";
  import { titleFromPath } from "$lib/library/bookInfo";
  import Book3D from "$lib/components/home/Book3D.svelte";

  interface Props {
    path: string;
    meta: BookMeta;
    shelves: Shelf[];
    onSave: (patch: Partial<BookMeta>) => void;
    onClose: () => void;
  }
  let { path, meta, shelves, onSave, onClose }: Props = $props();

  // Черновик формы: берём значения один раз при открытии.
  const initial = untrack(() => meta);
  let title = $state(initial.title ?? "");
  let author = $state(initial.author ?? "");
  let status = $state<ReadingStatus | "">(initial.status ?? "");
  let importance = $state<Importance>(
    (IMPORTANCE_OPTIONS.some((o) => o.value === initial.importance) ? initial.importance : "normal") as Importance,
  );
  let shelfIds = $state<string[]>([...effectiveShelfIds(initial)]);
  let review = $state(initial.review ?? "");
  let typstStyle = $state(initial.typstStyleRelativePath ?? "");

  const preview = $derived({ ...meta, title: title || null, author: author || null });

  function toggleShelf(id: string) {
    shelfIds = shelfIds.includes(id) ? shelfIds.filter((x) => x !== id) : [...shelfIds, id];
    if (!shelfIds.length) shelfIds = ["default"];
  }

  function save(e: SubmitEvent) {
    e.preventDefault();
    const ids = shelfIds.length ? shelfIds : ["default"];
    onSave({
      title: title.trim() || null,
      author: author.trim() || null,
      status: status || null,
      importance,
      shelfIds: ids,
      shelfId: ids[0] ?? "default",
      review,
      typstStyleRelativePath: typstStyle.trim() || null,
    });
  }

  function onKey(e: KeyboardEvent) {
    if (e.key === "Escape") onClose();
  }
</script>

<svelte:window onkeydown={onKey} />

<!-- svelte-ignore a11y_click_events_have_key_events -->
<!-- svelte-ignore a11y_no_static_element_interactions -->
<div class="back" onclick={onClose}>
  <div class="dlg-wrap" role="dialog" aria-modal="true" aria-labelledby="edit-title" tabindex="-1" onclick={(e) => e.stopPropagation()}>
  <form class="dlg" onsubmit={save}>
    <aside class="aside">
      <div class="cover book-hover-target"><Book3D {path} meta={preview} eager /></div>
      <p class="file" title={path}>{titleFromPath(path)}</p>
    </aside>

    <div class="main">
      <header>
        <h2 id="edit-title">О книге</h2>
        <button type="button" class="x" aria-label="Закрыть" onclick={onClose}>×</button>
      </header>

      <div class="grid2">
        <label class="field">
          <span>Название</span>
          <input bind:value={title} placeholder={titleFromPath(path)} />
        </label>
        <label class="field">
          <span>Автор</span>
          <input bind:value={author} placeholder="Имя автора" />
        </label>
      </div>

      <fieldset>
        <legend>Статус</legend>
        <div class="chips">
          {#each READING_STATUS_OPTIONS as o (o.value)}
            <button type="button" class="chip" class:on={status === o.value} onclick={() => (status = status === o.value ? "" : o.value)}>
              {o.label}
            </button>
          {/each}
        </div>
      </fieldset>

      <fieldset>
        <legend>Полки</legend>
        <div class="chips">
          {#each shelves as s (s.id)}
            <button type="button" class="chip" class:on={shelfIds.includes(s.id)} onclick={() => toggleShelf(s.id)}>
              {#if shelfIds.includes(s.id)}<span aria-hidden="true">✓</span>{/if}
              {s.name}
            </button>
          {/each}
        </div>
      </fieldset>

      <fieldset>
        <legend>Важность</legend>
        <div class="chips">
          {#each IMPORTANCE_OPTIONS as o (o.value)}
            <button type="button" class="chip" class:on={importance === o.value} onclick={() => (importance = o.value)}>{o.label}</button>
          {/each}
        </div>
      </fieldset>

      <label class="field">
        <span>Мысли о книге</span>
        <textarea rows="4" bind:value={review} placeholder="Что хочется сохранить после этой книги…"></textarea>
      </label>

      <details>
        <summary>Экспорт в Typst</summary>
        <label class="field">
          <span>Стиль (.typ внутри библиотеки)</span>
          <input bind:value={typstStyle} placeholder="Пусто — общий стиль из настроек" />
        </label>
      </details>

      <footer>
        <button type="button" class="btn" onclick={onClose}>Отмена</button>
        <button type="submit" class="btn primary">Сохранить</button>
      </footer>
    </div>
  </form>
  </div>
</div>

<style>
  .back {
    position: fixed;
    inset: 0;
    z-index: 900;
    display: grid;
    place-items: center;
    padding: 1rem;
    background: rgba(20, 14, 30, 0.38);
    backdrop-filter: blur(6px);
    animation: fade 0.18s ease-out;
  }

  .dlg-wrap {
    width: min(50rem, 100%);
  }

  .dlg {
    width: 100%;
    max-height: calc(100dvh - 2rem);
    display: grid;
    grid-template-columns: 13rem 1fr;
    border-radius: 1.6rem;
    overflow: hidden;
    background: var(--panel-elevated);
    box-shadow: var(--shadow-float);
    color: var(--text-soft);
    animation: rise 0.22s cubic-bezier(0.2, 0.8, 0.2, 1);
  }

  .aside {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 1rem;
    padding: 2rem 1.6rem;
    background: color-mix(in srgb, var(--panel-soft) 70%, var(--panel-elevated));
  }

  .cover {
    width: 100%;
  }

  .file {
    margin: 0;
    font-size: 0.72rem;
    color: var(--muted);
    text-align: center;
    overflow-wrap: anywhere;
  }

  .main {
    display: flex;
    flex-direction: column;
    gap: 1rem;
    padding: 1.5rem 1.7rem 1.3rem;
    overflow-y: auto;
  }

  header {
    display: flex;
    align-items: center;
  }

  h2 {
    flex: 1;
    margin: 0;
    font-family: "Literata Variable", Georgia, serif;
    font-size: 1.5rem;
    font-weight: 500;
  }

  .x {
    border: none;
    background: transparent;
    color: var(--muted);
    font-size: 1.5rem;
    cursor: pointer;
  }

  .grid2 {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 0.8rem;
  }

  .field {
    display: flex;
    flex-direction: column;
    gap: 0.35rem;
    font-size: 0.78rem;
    color: var(--muted);
  }

  .field input,
  .field textarea {
    border: 1px solid var(--border-soft);
    background: var(--elevated-soft);
    color: var(--text-soft);
    border-radius: 0.8rem;
    padding: 0.6rem 0.75rem;
    font: inherit;
    font-size: 0.95rem;
    resize: vertical;
  }

  .field input:focus,
  .field textarea:focus {
    outline: none;
    border-color: var(--accent-2);
    box-shadow: var(--focus-ring);
  }

  fieldset {
    border: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 0.45rem;
  }

  legend {
    padding: 0;
    margin-bottom: 0.4rem;
    font-size: 0.78rem;
    color: var(--muted);
  }

  .chips {
    display: flex;
    flex-wrap: wrap;
    gap: 0.4rem;
  }

  .chip {
    display: inline-flex;
    align-items: center;
    gap: 0.3rem;
    border: 1px solid var(--border-soft);
    background: transparent;
    color: var(--text-soft);
    border-radius: 999px;
    padding: 0.35rem 0.8rem;
    font-size: 0.84rem;
    cursor: pointer;
  }

  .chip.on {
    background: var(--text-soft);
    border-color: var(--text-soft);
    color: var(--bg-soft);
  }

  details summary {
    cursor: pointer;
    font-size: 0.8rem;
    color: var(--muted);
    margin-bottom: 0.5rem;
  }

  footer {
    display: flex;
    justify-content: flex-end;
    gap: 0.5rem;
    margin-top: 0.4rem;
  }

  .btn {
    border: 1px solid var(--border-soft);
    background: transparent;
    color: var(--text-soft);
    border-radius: 999px;
    padding: 0.6rem 1.2rem;
    font: inherit;
    cursor: pointer;
  }

  .btn.primary {
    background: var(--text-soft);
    border-color: var(--text-soft);
    color: var(--bg-soft);
    font-weight: 600;
  }

  @keyframes fade {
    from {
      opacity: 0;
    }
  }

  @keyframes rise {
    from {
      opacity: 0;
      transform: translateY(12px) scale(0.98);
    }
  }

  @media (max-width: 680px) {
    .dlg {
      grid-template-columns: 1fr;
    }

    .aside {
      flex-direction: row;
      padding: 1rem 1.2rem;
    }

    .cover {
      width: 4.5rem;
      flex-shrink: 0;
    }

    .file {
      text-align: left;
    }

    .grid2 {
      grid-template-columns: 1fr;
    }
  }
</style>
