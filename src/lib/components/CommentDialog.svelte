<script lang="ts">
  interface Props {
    open: boolean;
    excerpt: string;
    pageHint: string;
    initialBody?: string;
    onClose: () => void;
    onSave: (body: string) => void;
  }
  let {
    open,
    excerpt,
    pageHint,
    initialBody = "",
    onClose,
    onSave,
  }: Props = $props();

  let body = $state("");

  $effect(() => {
    if (open) body = initialBody || "";
  });

  function save() {
    const t = body.trim();
    if (!t) return;
    onSave(t);
    onClose();
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
      tabindex="-1"
    >
      <header class="hd">
        <h3>Комментарий</h3>
        <button type="button" class="x" onclick={onClose}>×</button>
      </header>
      {#if pageHint}
        <p class="hint">{pageHint}</p>
      {/if}
      {#if excerpt}
        <blockquote class="ex">{excerpt}</blockquote>
      {/if}
      <label class="lab">
        <span>Текст комментария</span>
        <textarea rows="5" bind:value={body} placeholder="Заметка к отрывку или странице…"></textarea>
      </label>
      <footer class="ft">
        <button type="button" class="btn" onclick={onClose}>Отмена</button>
        <button type="button" class="btn primary" onclick={save}>Сохранить</button>
      </footer>
    </div>
  </div>
{/if}

<style>
  .backdrop {
    position: fixed;
    inset: 0;
    z-index: 500;
    background: color-mix(in srgb, #1c1814 55%, transparent);
    display: flex;
    align-items: center;
    justify-content: center;
    padding: 1rem;
    backdrop-filter: blur(10px);
  }

  .dlg {
    width: min(28rem, 100%);
    border-radius: var(--radius-lg, 18px);
    border: 1px solid color-mix(in srgb, var(--border-soft) 85%, #c4b8a8);
    background: linear-gradient(
      165deg,
      var(--panel-elevated, #fffcf8) 0%,
      var(--elevated-soft, #fffdfa) 100%
    );
    padding: 0 1rem 1rem;
    box-shadow:
      var(--shadow-float, 0 18px 48px rgba(61, 56, 51, 0.13)),
      0 0 0 1px rgba(255, 255, 255, 0.45) inset;
  }

  .hd {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 0.75rem 0 0.5rem;
  }

  .hd h3 {
    margin: 0;
    font-size: 1rem;
    font-family: system-ui, sans-serif;
    color: var(--text-soft, #3d3833);
  }

  .x {
    border: none;
    background: transparent;
    font-size: 1.35rem;
    line-height: 1;
    cursor: pointer;
    color: var(--muted, #8a817a);
  }

  .hint {
    margin: 0 0 0.5rem;
    font-size: 0.82rem;
    color: var(--muted, #8a817a);
  }

  .ex {
    margin: 0 0 0.75rem;
    padding: 0.55rem 0.65rem;
    border-left: 3px solid color-mix(in srgb, var(--accent, #c9a574) 55%, transparent);
    font-size: 0.88rem;
    line-height: 1.45;
    color: var(--text-soft, #3d3833);
    background: color-mix(in srgb, var(--accent, #c9a574) 8%, transparent);
    border-radius: 4px;
  }

  .lab {
    display: flex;
    flex-direction: column;
    gap: 0.35rem;
    font-size: 0.78rem;
    color: var(--muted, #8a817a);
  }

  .lab textarea {
    padding: 0.5rem 0.55rem;
    border-radius: var(--radius-md, 12px);
    border: 1px solid var(--border-soft, #e8dfd6);
    font-size: 0.9rem;
    resize: vertical;
    min-height: 6rem;
  }

  .ft {
    display: flex;
    justify-content: flex-end;
    gap: 0.5rem;
    margin-top: 0.75rem;
  }

  .btn {
    padding: 0.45rem 0.85rem;
    border-radius: var(--radius-md, 12px);
    border: 1px solid var(--border-soft, #e8dfd6);
    background: var(--elevated-soft, #fffdfa);
    cursor: pointer;
    font-size: 0.88rem;
  }

  .btn.primary {
    background: color-mix(in srgb, var(--accent, #c9a574) 16%, var(--elevated-soft, #fffdfa));
    border-color: color-mix(in srgb, var(--accent, #c9a574) 45%, var(--border-soft, #e8dfd6));
    font-weight: 600;
  }
</style>
