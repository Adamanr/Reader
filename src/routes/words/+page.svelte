<script lang="ts">
  import { onMount } from "svelte";
  import { goto } from "$app/navigation";
  import { dueCards, loadVocab, removeWord, review, vocab, type Grade, type VocabCard } from "$lib/vocab/vocab.svelte";

  let tab = $state<"review" | "list">("review");
  let revealed = $state(false);
  let query = $state("");
  let sessionDone = $state(0);
  let now = $state(Date.now());

  onMount(() => {
    void loadVocab();
    const t = setInterval(() => (now = Date.now()), 30_000);
    return () => clearInterval(t);
  });

  const due = $derived.by(() => {
    now;
    return dueCards(now).sort((a, b) => a.due.localeCompare(b.due));
  });
  const card = $derived<VocabCard | undefined>(due[0]);

  function grade(g: Grade) {
    if (!card) return;
    review(card.id, g);
    revealed = false;
    sessionDone++;
    now = Date.now();
  }

  /** Контекст с пропуском на месте слова — вспоминать по живой фразе. */
  function blanked(c: VocabCard): string {
    const re = new RegExp(c.word.replace(/[.*+?^${}()|[\]\\]/g, "\\$&"), "giu");
    return c.context.replace(re, "_____");
  }

  function highlighted(c: VocabCard): string {
    const esc = (s: string) => s.replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;");
    const re = new RegExp(`(${c.word.replace(/[.*+?^${}()|[\]\\]/g, "\\$&")})`, "giu");
    return esc(c.context).replace(re, "<mark>$1</mark>");
  }

  function onKey(e: KeyboardEvent) {
    if (tab !== "review" || !card) return;
    if ((e.target as HTMLElement)?.closest("input, textarea")) return;
    if (e.key === " " || e.key === "Enter") {
      e.preventDefault();
      revealed = true;
    } else if (revealed && ["1", "2", "3", "4"].includes(e.key)) {
      grade((Number(e.key) - 1) as Grade);
    }
  }

  const list = $derived.by(() => {
    const q = query.trim().toLocaleLowerCase();
    return vocab.cards.filter(
      (c) => !q || c.word.toLocaleLowerCase().includes(q) || c.translation.toLocaleLowerCase().includes(q) || c.bookTitle.toLocaleLowerCase().includes(q),
    );
  });

  function nextIn(c: VocabCard): string {
    const ms = Date.parse(c.due) - now;
    if (ms <= 0) return "сейчас";
    const d = Math.round(ms / 86_400_000);
    if (d < 1) return "сегодня";
    return `через ${d} дн.`;
  }
</script>

<svelte:window onkeydown={onKey} />

<div class="wp">
  <header class="top">
    <button type="button" class="back" onclick={() => goto("/")}>← Библиотека</button>
    <div class="heading">
      <p class="kicker">слова из ваших книг</p>
      <h1>Мои слова</h1>
    </div>
    <div class="tabs" role="tablist">
      <button type="button" role="tab" aria-selected={tab === "review"} class:on={tab === "review"} onclick={() => (tab = "review")}>
        Повторить {#if due.length}<span class="badge">{due.length}</span>{/if}
      </button>
      <button type="button" role="tab" aria-selected={tab === "list"} class:on={tab === "list"} onclick={() => (tab = "list")}>
        Все ({vocab.cards.length})
      </button>
    </div>
  </header>

  <main class="main">
    {#if vocab.cards.length === 0}
      <div class="empty">
        <span aria-hidden="true">Aa</span>
        <h2>Пока пусто</h2>
        <p>
          Выделите слово в книге и нажмите «Перевод» → «В мои слова». Слово сохранится вместе с фразой, в которой вы его
          встретили, — вспоминать по живому контексту гораздо легче.
        </p>
      </div>
    {:else if tab === "review"}
      {#if card}
        <section class="card-wrap">
          <div class="flash" class:revealed>
            <p class="ctx">{revealed ? "" : blanked(card)}</p>
            {#if revealed}
              <p class="word">{card.word}</p>
              <p class="tr">{card.translation}</p>
              {#if card.explanation}<p class="ex">{card.explanation}</p>{/if}
              <!-- eslint-disable-next-line svelte/no-at-html-tags -->
              <p class="ctx full">{@html highlighted(card)}</p>
              <p class="src">{card.bookTitle}</p>
            {:else}
              <p class="prompt">Какое слово пропущено? Вспомните его значение.</p>
            {/if}
          </div>
          {#if !revealed}
            <button type="button" class="btn primary big" onclick={() => (revealed = true)}>Показать <kbd>Пробел</kbd></button>
          {:else}
            <div class="grades">
              <button type="button" class="g g0" onclick={() => grade(0)}>Не помню<kbd>1</kbd></button>
              <button type="button" class="g g1" onclick={() => grade(1)}>Трудно<kbd>2</kbd></button>
              <button type="button" class="g g2" onclick={() => grade(2)}>Помню<kbd>3</kbd></button>
              <button type="button" class="g g3" onclick={() => grade(3)}>Легко<kbd>4</kbd></button>
            </div>
          {/if}
          <p class="left">Осталось сегодня: {due.length}{sessionDone ? ` · повторено: ${sessionDone}` : ""}</p>
        </section>
      {:else}
        <div class="empty">
          <span aria-hidden="true">✦</span>
          <h2>На сегодня всё</h2>
          <p>Все слова повторены. Следующие карточки появятся, когда подойдёт их время.</p>
        </div>
      {/if}
    {:else}
      <label class="search">
        <input type="search" placeholder="Поиск по словам" bind:value={query} />
      </label>
      <ul class="list">
        {#each list as c (c.id)}
          <li>
            <div class="li-main">
              <span class="li-word">{c.word}</span>
              <span class="li-tr">{c.translation}</span>
              <span class="li-ctx">{c.context}</span>
              <span class="li-meta">{c.bookTitle} · повтор {nextIn(c)}</span>
            </div>
            <button type="button" class="del" aria-label="Удалить" title="Удалить" onclick={() => removeWord(c.id)}>×</button>
          </li>
        {/each}
      </ul>
    {/if}
  </main>
</div>

<style>
  .wp {
    min-height: 100dvh;
    background:
      radial-gradient(ellipse 40rem 18rem at 80% -4rem, color-mix(in srgb, var(--accent) 20%, transparent), transparent 70%),
      var(--bg-soft);
    color: var(--text-soft);
  }

  .top {
    max-width: 52rem;
    margin: 0 auto;
    display: flex;
    align-items: flex-end;
    gap: 1.4rem;
    flex-wrap: wrap;
    padding: clamp(1rem, 3vw, 1.8rem) clamp(1rem, 5vw, 2rem) 1rem;
  }

  .heading {
    flex: 1;
  }

  .back,
  .btn {
    border: 1px solid var(--border-soft);
    background: var(--panel-elevated);
    color: var(--text-soft);
    border-radius: 999px;
    padding: 0.45rem 0.9rem;
    cursor: pointer;
  }

  .kicker {
    margin: 0;
    font-size: 0.7rem;
    text-transform: uppercase;
    letter-spacing: 0.16em;
    color: var(--muted);
  }

  h1 {
    margin: 0.2rem 0 0;
    font-family: "Literata Variable", Georgia, serif;
    font-weight: 500;
    font-size: clamp(1.8rem, 4vw, 2.6rem);
  }

  .tabs {
    display: flex;
    padding: 3px;
    border-radius: 999px;
    border: 1px solid var(--border-soft);
    background: var(--panel-soft);
  }

  .tabs button {
    display: flex;
    align-items: center;
    gap: 0.35rem;
    border: none;
    background: transparent;
    color: var(--muted);
    padding: 0.4rem 0.9rem;
    border-radius: 999px;
    cursor: pointer;
  }

  .tabs button.on {
    background: var(--elevated-soft);
    color: var(--text-soft);
    font-weight: 600;
  }

  .badge {
    min-width: 1.2rem;
    padding: 0 0.3rem;
    border-radius: 999px;
    background: var(--accent-2);
    color: var(--elevated-soft);
    font-size: 0.7rem;
  }

  .main {
    max-width: 52rem;
    margin: 0 auto;
    padding: 0 clamp(1rem, 5vw, 2rem) 3rem;
  }

  .card-wrap {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 1.2rem;
    margin-top: 1.5rem;
  }

  .flash {
    width: 100%;
    min-height: 16rem;
    display: flex;
    flex-direction: column;
    justify-content: center;
    gap: 0.7rem;
    padding: 2rem 2.2rem;
    border-radius: var(--radius-xl);
    background: var(--panel-elevated);
    border: 1px solid color-mix(in srgb, var(--border-soft) 75%, transparent);
    box-shadow: var(--shadow-book);
    text-align: center;
    animation: flip 0.35s ease;
  }

  .flash.revealed {
    animation: flip2 0.35s ease;
  }

  @keyframes flip {
    from {
      transform: rotateX(8deg);
      opacity: 0.4;
    }
  }

  @keyframes flip2 {
    from {
      transform: rotateX(-8deg);
      opacity: 0.4;
    }
  }

  .ctx {
    margin: 0;
    font-family: "Literata Variable", Georgia, serif;
    font-size: 1.2rem;
    line-height: 1.6;
  }

  .ctx.full {
    font-size: 0.95rem;
    color: var(--muted);
  }

  .ctx :global(mark) {
    background: color-mix(in srgb, var(--accent) 35%, transparent);
    color: var(--text-soft);
    border-radius: 3px;
    padding: 0 2px;
  }

  .prompt {
    margin: 0;
    font-size: 0.84rem;
    color: var(--muted);
  }

  .word {
    margin: 0;
    font-family: "Literata Variable", Georgia, serif;
    font-size: 2rem;
    font-weight: 600;
  }

  .tr {
    margin: 0;
    font-size: 1.15rem;
    color: var(--accent-2);
    font-weight: 600;
  }

  .ex {
    margin: 0;
    font-size: 0.92rem;
    line-height: 1.5;
  }

  .src {
    margin: 0;
    font-size: 0.74rem;
    color: var(--muted);
  }

  kbd {
    font-family: inherit;
    font-size: 0.7rem;
    opacity: 0.6;
    margin-left: 0.4rem;
  }

  .btn.primary {
    background: var(--accent-2);
    border-color: var(--accent-2);
    color: var(--elevated-soft);
  }

  .btn.big {
    padding: 0.7rem 1.6rem;
    font-size: 1rem;
  }

  .grades {
    display: grid;
    grid-template-columns: repeat(4, 1fr);
    gap: 0.5rem;
    width: 100%;
  }

  .g {
    border: 1px solid var(--border-soft);
    border-radius: var(--radius-md);
    padding: 0.7rem 0.4rem;
    background: var(--panel-elevated);
    color: var(--text-soft);
    cursor: pointer;
    font-size: 0.9rem;
  }

  .g0 {
    border-color: color-mix(in srgb, var(--danger) 50%, var(--border-soft));
  }
  .g1 {
    border-color: color-mix(in srgb, #e0a24a 50%, var(--border-soft));
  }
  .g2 {
    border-color: color-mix(in srgb, #6aa77c 50%, var(--border-soft));
  }
  .g3 {
    border-color: color-mix(in srgb, var(--accent-2) 60%, var(--border-soft));
  }

  .left {
    margin: 0;
    font-size: 0.8rem;
    color: var(--muted);
  }

  .empty {
    text-align: center;
    padding: 4rem 1rem;
    color: var(--muted);
    max-width: 34rem;
    margin: 0 auto;
  }

  .empty span {
    font-family: "Literata Variable", Georgia, serif;
    font-size: 2.4rem;
    color: var(--accent-2);
  }

  .empty h2 {
    color: var(--text-soft);
    font-family: "Literata Variable", Georgia, serif;
    font-weight: 500;
  }

  .search input {
    width: 100%;
    border: 1px solid var(--border-soft);
    background: var(--panel-elevated);
    color: var(--text-soft);
    border-radius: 999px;
    padding: 0.55rem 1rem;
    font: inherit;
    margin-bottom: 1rem;
  }

  .list {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 0.5rem;
  }

  .list li {
    display: flex;
    gap: 0.5rem;
    padding: 0.75rem 0.9rem;
    border-radius: var(--radius-md);
    background: var(--panel-elevated);
    border: 1px solid color-mix(in srgb, var(--border-soft) 70%, transparent);
  }

  .li-main {
    flex: 1;
    min-width: 0;
    display: grid;
    grid-template-columns: auto 1fr;
    column-gap: 0.8rem;
    row-gap: 0.25rem;
    align-items: baseline;
  }

  .li-word {
    font-family: "Literata Variable", Georgia, serif;
    font-weight: 600;
    font-size: 1.05rem;
  }

  .li-tr {
    color: var(--accent-2);
  }

  .li-ctx {
    grid-column: 1 / -1;
    font-size: 0.82rem;
    color: var(--muted);
    font-style: italic;
  }

  .li-meta {
    grid-column: 1 / -1;
    font-size: 0.7rem;
    color: var(--muted);
  }

  .del {
    border: none;
    background: transparent;
    color: var(--muted);
    font-size: 1.2rem;
    cursor: pointer;
    align-self: flex-start;
  }

  @media (max-width: 560px) {
    .grades {
      grid-template-columns: repeat(2, 1fr);
    }
  }
</style>
