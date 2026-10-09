<script lang="ts">
  import type { BookFormat } from "$lib/bookFormat";
  import { READING_STATUS_OPTIONS, type ReadingStatus } from "$lib/types";

  interface Props {
    x: number;
    y: number;
    title: string;
    status: ReadingStatus | null;
    format: BookFormat | null;
    /** Книга убрана с полок: доступны только «Вернуть» и «Удалить». */
    hidden: boolean;
    /** Перевод и экспорт работают только в приложении (нужен бэкенд). */
    canTranslate: boolean;
    onRead: () => void;
    onEdit: () => void;
    onStatus: (status: ReadingStatus) => void;
    onTranslate: () => void;
    onExportTypst: () => void;
    onRestore: () => void;
    onHide: () => void;
    onDelete: () => void;
  }

  let {
    x,
    y,
    title,
    status,
    format,
    hidden,
    canTranslate,
    onRead,
    onEdit,
    onStatus,
    onTranslate,
    onExportTypst,
    onRestore,
    onHide,
    onDelete,
  }: Props = $props();
</script>

<div id="ctx-menu" class="ctx" style="left:{x}px;top:{y}px" role="menu">
  <p class="ctx-title">{title}</p>
  {#if hidden}
    <button type="button" role="menuitem" onclick={onRestore}>Вернуть на полку</button>
    <div class="sep"></div>
    <button type="button" role="menuitem" class="danger" onclick={onDelete}>Удалить файл…</button>
  {:else}
    <button type="button" role="menuitem" onclick={onRead}>Читать</button>
    <button type="button" role="menuitem" onclick={onEdit}>Изменить сведения…</button>
    <div class="sep"></div>
    <p class="ctx-label">Статус</p>
    <div class="ctx-status">
      {#each READING_STATUS_OPTIONS as o (o.value)}
        <button type="button" class:on={status === o.value} onclick={() => onStatus(o.value)}>{o.label}</button>
      {/each}
    </div>
    <div class="sep"></div>
    {#if canTranslate && format === "pdf"}
      <button type="button" role="menuitem" onclick={onTranslate}>Перевести книгу…</button>
    {/if}
    {#if canTranslate && format && format !== "typst"}
      <button type="button" role="menuitem" onclick={onExportTypst}>Экспорт в Typst…</button>
    {/if}
    <button type="button" role="menuitem" onclick={onHide}>Убрать с полок</button>
    <button type="button" role="menuitem" class="danger" onclick={onDelete}>Удалить файл…</button>
  {/if}
</div>

<style>
  /* ——— Контекстное меню ——— */
  .ctx {
    position: fixed;
    z-index: 800;
    width: 15rem;
    padding: 0.4rem;
    border-radius: 1rem;
    background: var(--panel-elevated);
    border: 1px solid color-mix(in srgb, var(--border-soft) 80%, transparent);
    box-shadow: var(--shadow-float);
    display: flex;
    flex-direction: column;
    animation: pop 0.14s ease-out;
  }

  .ctx-title {
    margin: 0;
    padding: 0.45rem 0.7rem 0.5rem;
    font-family: "Literata Variable", Georgia, serif;
    font-weight: 600;
    font-size: 0.9rem;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .ctx-label {
    margin: 0;
    padding: 0.2rem 0.7rem;
    font-size: 0.68rem;
    text-transform: uppercase;
    letter-spacing: 0.12em;
    color: var(--muted);
  }

  .ctx > button {
    border: none;
    background: transparent;
    color: var(--text-soft);
    text-align: left;
    padding: 0.5rem 0.7rem;
    border-radius: 0.6rem;
    font: inherit;
    font-size: 0.88rem;
    cursor: pointer;
  }

  .ctx > button:hover {
    background: color-mix(in srgb, var(--accent) 14%, transparent);
  }

  .ctx > button.danger {
    color: var(--danger);
  }

  .ctx-status {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 0.25rem;
    padding: 0.2rem 0.4rem 0.3rem;
  }

  .ctx-status button {
    border: 1px solid var(--border-soft);
    background: transparent;
    color: var(--text-soft);
    border-radius: 0.55rem;
    padding: 0.35rem 0.3rem;
    font: inherit;
    font-size: 0.74rem;
    cursor: pointer;
  }

  .ctx-status button.on {
    background: var(--text-soft);
    border-color: var(--text-soft);
    color: var(--bg-soft);
  }

  .sep {
    height: 1px;
    margin: 0.3rem 0.5rem;
    background: color-mix(in srgb, var(--border-soft) 80%, transparent);
  }

  @keyframes fade {
    from {
      opacity: 0;
    }
  }

  @keyframes pop {
    from {
      opacity: 0;
      transform: translateY(-4px) scale(0.98);
    }
  }
</style>
