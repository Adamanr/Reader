<script lang="ts">
  import Book3D from "$lib/components/home/Book3D.svelte";
  import { bookTitle } from "$lib/library/bookInfo";
  import type { BookMeta } from "$lib/types";

  interface Props {
    kind: "hide" | "delete";
    path: string;
    meta: BookMeta | undefined;
    busy: boolean;
    error: string | null;
    onCancel: () => void;
    onConfirm: () => void;
  }

  let { kind, path, meta, busy, error, onCancel, onConfirm }: Props = $props();
</script>

<!-- svelte-ignore a11y_click_events_have_key_events -->
<!-- svelte-ignore a11y_no_static_element_interactions -->
<div class="confirm-back" onclick={onCancel}>
  <div
    class="confirm"
    role="alertdialog"
    aria-modal="true"
    aria-labelledby="confirm-title"
    tabindex="-1"
    onclick={(e) => e.stopPropagation()}
  >
    <div class="confirm-book"><Book3D {path} {meta} eager compact tilt={false} /></div>
    <div class="confirm-body">
      <h2 id="confirm-title">{kind === "delete" ? "Удалить книгу с диска?" : "Убрать книгу с полок?"}</h2>
      <p class="confirm-name">{bookTitle(path, meta)}</p>
      <p class="confirm-copy">
        {kind === "delete"
          ? "Файл будет удалён безвозвратно вместе с прогрессом и заметками."
          : "Файл останется на диске. Вернуть книгу можно из раздела «Скрытые»."}
      </p>
      {#if error}<p class="confirm-error" role="alert">{error}</p>{/if}
      <div class="confirm-actions">
        <button type="button" class="ghost" onclick={onCancel} disabled={busy}>Отмена</button>
        <button type="button" class="solid" class:danger={kind === "delete"} onclick={onConfirm} disabled={busy}>
          {busy ? "Подождите…" : kind === "delete" ? "Удалить" : "Убрать"}
        </button>
      </div>
    </div>
  </div>
</div>

<style>
  /* ——— Подтверждение ——— */
  .confirm-back {
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

  .confirm {
    width: min(30rem, 100%);
    display: flex;
    gap: 1.4rem;
    padding: 1.6rem;
    border-radius: 1.5rem;
    background: var(--panel-elevated);
    box-shadow: var(--shadow-float);
    animation: pop 0.18s ease-out;
  }

  .confirm-book {
    width: 5.2rem;
    flex-shrink: 0;
  }

  .confirm-body h2 {
    margin: 0;
    font-family: "Literata Variable", Georgia, serif;
    font-weight: 500;
    font-size: 1.3rem;
  }

  .confirm-name {
    margin: 0.4rem 0 0;
    font-weight: 600;
  }

  .confirm-copy {
    margin: 0.5rem 0 0;
    color: var(--muted);
    font-size: 0.9rem;
    line-height: 1.5;
  }

  .confirm-error {
    margin: 0.6rem 0 0;
    color: var(--danger);
    font-size: 0.86rem;
  }

  .confirm-actions {
    display: flex;
    justify-content: flex-end;
    gap: 0.5rem;
    margin-top: 0.6rem;
  }

  .ghost,
  .solid {
    margin-top: 0.8rem;
    padding: 0.55rem 1.1rem;
    border-radius: 999px;
    border: 1px solid var(--border-soft);
    background: transparent;
    color: var(--text-soft);
    font: inherit;
    cursor: pointer;
  }

  .solid {
    background: var(--text-soft);
    border-color: var(--text-soft);
    color: var(--bg-soft);
    font-weight: 600;
  }

  .solid.danger {
    background: var(--danger);
    border-color: var(--danger);
    color: #fff;
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
