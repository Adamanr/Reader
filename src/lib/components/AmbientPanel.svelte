<script lang="ts">
  import { AMBIENT_OPTIONS, ambient, playAmbient, setAmbientVolume, stopAmbient, type AmbientId } from "$lib/reading/ambient.svelte";

  interface Props {
    /** Звук, закреплённый за книгой */
    bookSound: string | null;
    onBind: (id: AmbientId | null) => void;
    onClose: () => void;
  }
  let { bookSound, onBind, onClose }: Props = $props();

  function toggle(id: AmbientId) {
    if (ambient.current === id) stopAmbient();
    else void playAmbient(id);
  }
</script>

<!-- svelte-ignore a11y_click_events_have_key_events -->
<!-- svelte-ignore a11y_no_static_element_interactions -->
<div class="ap-back" onclick={onClose}></div>
<div class="ap" role="dialog" aria-label="Фоновый звук">
  <header>
    <h2>Фоновый звук</h2>
    <button type="button" class="x" aria-label="Закрыть" onclick={onClose}>×</button>
  </header>
  <div class="tiles">
    {#each AMBIENT_OPTIONS as o (o.id)}
      <button type="button" class="tile" class:on={ambient.current === o.id} onclick={() => toggle(o.id)}>
        <span class="ico" aria-hidden="true">{o.icon}</span>
        <span>{o.label}</span>
        {#if ambient.current === o.id}<span class="eq" aria-hidden="true"><i></i><i></i><i></i></span>{/if}
      </button>
    {/each}
  </div>
  <label class="vol">
    <span>Громкость</span>
    <input type="range" min="0.05" max="1" step="0.05" value={ambient.volume} oninput={(e) => setAmbientVolume(+e.currentTarget.value)} />
  </label>
  <label class="bind">
    <input
      type="checkbox"
      checked={!!bookSound && bookSound === ambient.current}
      disabled={!ambient.current}
      onchange={(e) => onBind(e.currentTarget.checked ? ambient.current : null)}
    />
    Предлагать этот звук при открытии книги
  </label>
  <p class="hint">Звуки синтезируются на лету — без интернета и файлов.</p>
</div>

<style>
  .ap-back {
    position: fixed;
    inset: 0;
    z-index: 300;
  }

  .ap {
    position: fixed;
    z-index: 310;
    top: 3.6rem;
    right: 0.75rem;
    width: min(20rem, calc(100vw - 1.5rem));
    display: flex;
    flex-direction: column;
    gap: 0.8rem;
    padding: 0.9rem 1rem 1rem;
    border-radius: var(--radius-lg);
    background: var(--panel-elevated);
    border: 1px solid var(--toolbar-border);
    box-shadow: var(--shadow-float);
    color: var(--text-soft);
    animation: in 0.18s ease-out;
  }

  header {
    display: flex;
    align-items: center;
  }

  h2 {
    margin: 0;
    flex: 1;
    font-size: 1rem;
  }

  .x {
    border: none;
    background: transparent;
    color: var(--muted);
    font-size: 1.3rem;
    cursor: pointer;
  }

  .tiles {
    display: grid;
    grid-template-columns: repeat(3, 1fr);
    gap: 0.45rem;
  }

  .tile {
    position: relative;
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 0.3rem;
    padding: 0.7rem 0.3rem 0.6rem;
    border-radius: var(--radius-md);
    border: 1px solid var(--border-soft);
    background: var(--elevated-soft);
    color: var(--text-soft);
    font-size: 0.74rem;
    cursor: pointer;
  }

  .tile.on {
    border-color: var(--accent-2);
    background: color-mix(in srgb, var(--accent) 18%, var(--elevated-soft));
  }

  .ico {
    font-size: 1.35rem;
  }

  .eq {
    position: absolute;
    top: 0.35rem;
    right: 0.4rem;
    display: flex;
    gap: 2px;
    align-items: flex-end;
    height: 10px;
  }

  .eq i {
    width: 2px;
    background: var(--accent-2);
    animation: eq 0.9s ease-in-out infinite;
  }

  .eq i:nth-child(2) {
    animation-delay: 0.2s;
  }

  .eq i:nth-child(3) {
    animation-delay: 0.45s;
  }

  @keyframes eq {
    0%,
    100% {
      height: 3px;
    }
    50% {
      height: 10px;
    }
  }

  .vol {
    display: flex;
    flex-direction: column;
    gap: 0.3rem;
    font-size: 0.8rem;
    color: var(--muted);
  }

  .vol input {
    accent-color: var(--accent-2);
  }

  .bind {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    font-size: 0.8rem;
  }

  .hint {
    margin: 0;
    font-size: 0.72rem;
    color: var(--muted);
  }

  @keyframes in {
    from {
      opacity: 0;
      transform: translateY(-6px);
    }
  }
</style>
