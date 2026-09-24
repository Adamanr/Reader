<script lang="ts">
  import { loadGlossary, saveGlossary, type Glossary } from "$lib/ai/glossary";

  interface Props {
    bookPath: string;
    /** Меняется, когда словарь пополнился — перечитываем */
    version: number;
    progress: number | null;
  }
  let { bookPath, version, progress }: Props = $props();

  let glossary = $state<Glossary>({});
  let open = $state<string | null>(null);

  $effect(() => {
    version;
    const p = bookPath;
    void loadGlossary(p).then((g) => {
      if (p === bookPath) glossary = g;
    });
  });

  const entries = $derived(Object.entries(glossary).sort((a, b) => a[1].term.localeCompare(b[1].term, "ru")));

  async function remove(key: string) {
    const next = { ...glossary };
    delete next[key];
    glossary = next;
    await saveGlossary(bookPath, next);
  }
</script>

<div class="gp">
  {#if entries.length === 0}
    <div class="empty">
      <span aria-hidden="true">?</span>
      <p>
        Выделите имя героя или незнакомое понятие и нажмите «Кто это?». Ответ — только по уже прочитанному, без
        спойлеров. Так по ходу чтения соберётся словарь книги.
      </p>
    </div>
  {:else}
    <ul>
      {#each entries as [key, e] (key)}
        <li class:open={open === key}>
          <button type="button" class="head" onclick={() => (open = open === key ? null : key)}>
            <span class="term">{e.term}</span>
            <span class="meta">{Math.round(e.progress * 100)}%{e.ai ? "" : " · упоминания"}</span>
          </button>
          {#if open === key}
            <p class="ans">{e.answer}</p>
            {#if progress != null && progress - e.progress > 0.05}
              <p class="hint">Ответ дан на {Math.round(e.progress * 100)}% — выделите имя снова, чтобы обновить.</p>
            {/if}
            <button type="button" class="del" onclick={() => void remove(key)}>Убрать из словаря</button>
          {/if}
        </li>
      {/each}
    </ul>
  {/if}
</div>

<style>
  .empty {
    display: flex;
    gap: 0.6rem;
    padding: 0.8rem;
    border-radius: var(--radius-md);
    border: 1px dashed var(--border-soft);
    color: var(--muted);
    font-size: 0.84rem;
    line-height: 1.45;
  }

  .empty span {
    font-family: "Literata Variable", Georgia, serif;
    font-size: 1.4rem;
    color: var(--accent-2);
  }

  .empty p {
    margin: 0;
  }

  ul {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 0.35rem;
  }

  li {
    border-radius: var(--radius-sm);
    border: 1px solid var(--border-soft);
    background: var(--elevated-soft);
    overflow: hidden;
  }

  .head {
    width: 100%;
    display: flex;
    align-items: baseline;
    justify-content: space-between;
    gap: 0.5rem;
    padding: 0.55rem 0.65rem;
    border: none;
    background: transparent;
    color: var(--text-soft);
    cursor: pointer;
    text-align: left;
  }

  .term {
    font-family: "Literata Variable", Georgia, serif;
    font-weight: 600;
  }

  .meta {
    font-size: 0.7rem;
    color: var(--muted);
  }

  .ans {
    margin: 0;
    padding: 0 0.65rem 0.5rem;
    font-size: 0.86rem;
    line-height: 1.5;
    white-space: pre-wrap;
  }

  .hint {
    margin: 0;
    padding: 0 0.65rem 0.4rem;
    font-size: 0.72rem;
    color: var(--muted);
  }

  .del {
    margin: 0 0.65rem 0.6rem;
    border: none;
    background: transparent;
    color: var(--danger);
    font-size: 0.74rem;
    cursor: pointer;
    padding: 0;
  }
</style>
