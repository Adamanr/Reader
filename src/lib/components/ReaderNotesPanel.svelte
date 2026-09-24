<script lang="ts">
  import type { BookMeta, Highlight, QuoteAccent, SavedQuote } from "$lib/types";
  import { ACCENT_CHIP_BG } from "$lib/quoteCardStyles";
  import { highlightFill } from "$lib/reading/highlights";

  interface Props {
    meta: BookMeta;
    displayTitle: string;
    displayAuthor: string;
    review: string;
    onReviewInput: (v: string) => void;
    onGoHighlight: (h: Highlight) => void;
    onEditHighlight: (h: Highlight, rect: DOMRect) => void;
    onExport: () => void;
    onCopyMarkdown: () => void;
  }
  let {
    meta,
    displayTitle,
    displayAuthor,
    review,
    onReviewInput,
    onGoHighlight,
    onEditHighlight,
    onExport,
    onCopyMarkdown,
  }: Props = $props();

  const QUOTE_ACCENTS: QuoteAccent[] = ["sand", "sage", "dustyRose", "ink"];
  function quoteAccent(raw: string): QuoteAccent {
    return QUOTE_ACCENTS.includes(raw as QuoteAccent) ? (raw as QuoteAccent) : "sand";
  }

  let filter = $state<"all" | "notes">("all");
  const highlights = $derived(
    (meta.highlights ?? []).filter((h) => filter === "all" || !!h.note?.trim()),
  );
  const total = $derived(
    (meta.highlights?.length ?? 0) + (meta.comments?.length ?? 0) + (meta.quotes?.length ?? 0),
  );

  function where(h: Highlight): string {
    const parts: string[] = [];
    if (h.chapterLabel) parts.push(h.chapterLabel);
    if (h.page != null) parts.push(`стр. ${h.page}`);
    return parts.join(" · ");
  }

  function quoteBy(q: SavedQuote) {
    return q.bookTitle?.trim() || displayTitle;
  }
</script>

<div class="np">
  <div class="np-head">
    <div class="seg" role="group" aria-label="Фильтр">
      <button type="button" class:on={filter === "all"} onclick={() => (filter = "all")}>Всё</button>
      <button type="button" class:on={filter === "notes"} onclick={() => (filter = "notes")}>С мыслями</button>
    </div>
    <span class="grow"></span>
    <button type="button" class="np-btn" title="Скопировать конспект как Markdown" onclick={onCopyMarkdown}>Копировать</button>
    <button type="button" class="np-btn" title="Сохранить конспект в .md (Obsidian и др.)" onclick={onExport}>.md</button>
  </div>

  {#if total === 0}
    <div class="np-empty">
      <span aria-hidden="true">✎</span>
      <p>Выделите текст, чтобы оставить маркер или мысль на полях. Всё соберётся здесь.</p>
    </div>
  {/if}

  {#if highlights.length}
    <ul class="hl-list">
      {#each highlights as h (h.id)}
        <li class="hl" style:--hl={highlightFill(h.color)}>
          <button type="button" class="hl-main" onclick={() => onGoHighlight(h)}>
            <span class="hl-text">{h.text.length > 280 ? h.text.slice(0, 280) + "…" : h.text}</span>
            {#if h.note}
              <span class="hl-note">{h.note}</span>
            {/if}
            {#if where(h)}
              <span class="hl-where">{where(h)}</span>
            {/if}
          </button>
          <button
            type="button"
            class="hl-edit"
            aria-label="Изменить"
            title="Цвет, заметка, удаление"
            onclick={(e) => onEditHighlight(h, (e.currentTarget as HTMLElement).getBoundingClientRect())}>⋯</button>
        </li>
      {/each}
    </ul>
  {/if}

  {#if filter === "all" && (meta.comments?.length ?? 0) > 0}
    <section class="block">
      <h4>Комментарии</h4>
      <ul class="cmt-list">
        {#each meta.comments ?? [] as c (c.id)}
          <li class="cmt">
            {#if c.page != null}
              <span class="cmt-meta">стр. {c.page}</span>
            {:else if c.chapterLabel}
              <span class="cmt-meta">{c.chapterLabel}</span>
            {/if}
            {#if c.excerpt}
              <p class="cmt-ex">{c.excerpt}</p>
            {/if}
            <p class="cmt-body">{c.body}</p>
          </li>
        {/each}
      </ul>
    </section>
  {/if}

  {#if filter === "all" && (meta.quotes?.length ?? 0) > 0}
    <section class="block">
      <h4>Цитаты-карточки</h4>
      <ul class="q-list">
        {#each meta.quotes ?? [] as q (q.id)}
          {@const ac = quoteAccent(String(q.accent))}
          {@const hasBg = !!(q.bgImageDataUrl && q.bgImageDataUrl.length > 12)}
          <li class="q" class:with-bg={hasBg} style:--stripe={ACCENT_CHIP_BG[ac]}>
            {#if hasBg}
              <div class="q-media" aria-hidden="true">
                <div
                  class="q-img"
                  style:background-image={`url(${q.bgImageDataUrl})`}
                  style:opacity={q.bgImageOpacity ?? 1}
                  style:background-size={q.bgFit === "contain" ? "contain" : "cover"}
                ></div>
                {#if (q.overlayOpacity ?? 0) > 0.02}
                  <div class="q-tint" style:background-color={q.overlayColor ?? "#1a1510"} style:opacity={q.overlayOpacity ?? 0}></div>
                {/if}
              </div>
            {/if}
            <div class="q-body">
              <p class="q-text">«{q.text.length > 320 ? `${q.text.slice(0, 320)}…` : q.text}»</p>
              <p class="q-by">
                {quoteBy(q)}{#if q.bookAuthor?.trim() || displayAuthor}<span> — {q.bookAuthor?.trim() || displayAuthor}</span>{/if}
              </p>
            </div>
          </li>
        {/each}
      </ul>
    </section>
  {/if}

  {#if filter === "all"}
    <label class="review">
      <span>Мысли о книге</span>
      <textarea
        rows="8"
        placeholder="Что хочется сохранить после этой книги…"
        value={review}
        oninput={(e) => onReviewInput(e.currentTarget.value)}
      ></textarea>
    </label>
  {/if}
</div>

<style>
  .np {
    display: flex;
    flex-direction: column;
    gap: 0.8rem;
  }

  .np-head {
    display: flex;
    align-items: center;
    gap: 0.35rem;
  }

  .grow {
    flex: 1;
  }

  .seg {
    display: flex;
    padding: 2px;
    border-radius: 999px;
    background: var(--panel-soft);
    border: 1px solid var(--border-soft);
  }

  .seg button {
    border: none;
    background: transparent;
    color: var(--muted);
    border-radius: 999px;
    padding: 0.25rem 0.6rem;
    font-size: 0.74rem;
    cursor: pointer;
  }

  .seg button.on {
    background: var(--elevated-soft);
    color: var(--text-soft);
    font-weight: 600;
  }

  .np-btn {
    border: 1px solid var(--border-soft);
    background: var(--elevated-soft);
    color: var(--text-soft);
    border-radius: 999px;
    padding: 0.25rem 0.6rem;
    font-size: 0.74rem;
    cursor: pointer;
  }

  .np-empty {
    display: flex;
    gap: 0.6rem;
    align-items: flex-start;
    padding: 0.8rem;
    border-radius: var(--radius-md);
    border: 1px dashed var(--border-soft);
    color: var(--muted);
    font-size: 0.84rem;
    line-height: 1.45;
  }

  .np-empty p {
    margin: 0;
  }

  .hl-list,
  .cmt-list,
  .q-list {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 0.45rem;
  }

  .hl {
    position: relative;
    display: flex;
    border-radius: var(--radius-sm);
    background: var(--elevated-soft);
    border: 1px solid var(--border-soft);
    overflow: hidden;
  }

  .hl::before {
    content: "";
    width: 4px;
    flex-shrink: 0;
    background: var(--hl);
  }

  .hl-main {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: 0.3rem;
    text-align: left;
    border: none;
    background: transparent;
    color: var(--text-soft);
    padding: 0.55rem 0.6rem;
    cursor: pointer;
    font: inherit;
  }

  .hl-main:hover {
    background: color-mix(in srgb, var(--hl) 10%, transparent);
  }

  .hl-text {
    font-family: "Literata Variable", Georgia, serif;
    font-size: 0.86rem;
    line-height: 1.45;
  }

  .hl-note {
    font-size: 0.8rem;
    line-height: 1.4;
    padding: 0.35rem 0.5rem;
    border-radius: 8px;
    background: color-mix(in srgb, var(--hl) 16%, var(--panel-soft));
  }

  .hl-where {
    font-size: 0.7rem;
    color: var(--muted);
  }

  .hl-edit {
    border: none;
    background: transparent;
    color: var(--muted);
    padding: 0 0.55rem;
    cursor: pointer;
    font-size: 1rem;
  }

  .block h4 {
    margin: 0.3rem 0 0.5rem;
    font-size: 0.7rem;
    text-transform: uppercase;
    letter-spacing: 0.12em;
    color: var(--muted);
  }

  .cmt {
    padding: 0.55rem 0.65rem;
    border-radius: var(--radius-sm);
    background: var(--elevated-soft);
    border: 1px solid var(--border-soft);
    font-size: 0.84rem;
  }

  .cmt-meta {
    font-size: 0.7rem;
    color: var(--muted);
  }

  .cmt-ex {
    margin: 0.3rem 0;
    padding-left: 0.5rem;
    border-left: 2px solid var(--accent);
    color: var(--muted);
    font-style: italic;
  }

  .cmt-body {
    margin: 0;
  }

  .q {
    position: relative;
    border-radius: var(--radius-sm);
    overflow: hidden;
    border: 1px solid var(--border-soft);
    background: var(--elevated-soft);
    border-left: 4px solid var(--stripe);
  }

  .q-media {
    position: absolute;
    inset: 0;
  }

  .q-img,
  .q-tint {
    position: absolute;
    inset: 0;
    background-position: center;
  }

  .q-body {
    position: relative;
    padding: 0.6rem 0.7rem;
  }

  .q.with-bg .q-body {
    color: #fff;
    text-shadow: 0 1px 6px rgba(0, 0, 0, 0.5);
  }

  .q-text {
    margin: 0;
    font-family: "Literata Variable", Georgia, serif;
    font-size: 0.86rem;
    line-height: 1.45;
  }

  .q-by {
    margin: 0.35rem 0 0;
    font-size: 0.72rem;
    opacity: 0.8;
  }

  .review {
    display: flex;
    flex-direction: column;
    gap: 0.35rem;
    font-size: 0.72rem;
    text-transform: uppercase;
    letter-spacing: 0.12em;
    color: var(--muted);
  }

  .review textarea {
    text-transform: none;
    letter-spacing: normal;
    width: 100%;
    resize: vertical;
    border-radius: var(--radius-sm);
    border: 1px solid var(--border-soft);
    background: var(--elevated-soft);
    color: var(--text-soft);
    padding: 0.6rem;
    font: inherit;
    font-size: 0.9rem;
    line-height: 1.5;
  }
</style>
