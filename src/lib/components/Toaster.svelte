<script lang="ts">
  import { dismissToast, toasts } from "$lib/ui/toast.svelte";
</script>

<div class="toaster" aria-live="polite">
  {#each toasts.items as t (t.id)}
    <div class="toast toast-{t.kind}" role={t.kind === "error" ? "alert" : "status"}>
      <span class="toast-dot" aria-hidden="true"></span>
      <p>{t.text}</p>
      {#if t.action}
        {@const action = t.action}
        <button
          type="button"
          class="toast-action"
          onclick={() => {
            action.run();
            dismissToast(t.id);
          }}>{action.label}</button>
      {/if}
      <button type="button" class="toast-x" aria-label="Закрыть" onclick={() => dismissToast(t.id)}>×</button>
    </div>
  {/each}
</div>

<style>
  .toaster {
    position: fixed;
    z-index: 2000;
    left: 50%;
    bottom: max(1rem, env(safe-area-inset-bottom));
    transform: translateX(-50%);
    display: flex;
    flex-direction: column;
    gap: 0.5rem;
    width: min(92vw, 30rem);
    pointer-events: none;
  }

  .toast {
    pointer-events: auto;
    display: flex;
    align-items: center;
    gap: 0.6rem;
    padding: 0.65rem 0.7rem 0.65rem 0.9rem;
    border-radius: var(--radius-md);
    border: 1px solid var(--toolbar-border);
    background: var(--toolbar-surface);
    color: var(--text-soft);
    box-shadow: var(--shadow-float);
    font-size: 0.88rem;
    line-height: 1.4;
    animation: toast-in 0.22s ease-out;
  }

  .toast p {
    margin: 0;
    flex: 1;
    min-width: 0;
    overflow-wrap: anywhere;
  }

  .toast-dot {
    width: 0.5rem;
    height: 0.5rem;
    border-radius: 50%;
    flex-shrink: 0;
    background: var(--accent);
  }

  .toast-success .toast-dot {
    background: #6aa77c;
  }

  .toast-error {
    border-color: color-mix(in srgb, var(--danger) 45%, var(--toolbar-border));
  }

  .toast-error .toast-dot {
    background: var(--danger);
  }

  .toast-action {
    border: 1px solid var(--accent-2);
    background: transparent;
    color: var(--accent-2);
    border-radius: 999px;
    padding: 0.25rem 0.7rem;
    font-size: 0.8rem;
    font-weight: 650;
    cursor: pointer;
  }

  .toast-x {
    border: none;
    background: transparent;
    color: var(--muted);
    font-size: 1.1rem;
    line-height: 1;
    cursor: pointer;
    padding: 0.15rem 0.35rem;
  }

  @keyframes toast-in {
    from {
      opacity: 0;
      transform: translateY(8px);
    }
  }
</style>
