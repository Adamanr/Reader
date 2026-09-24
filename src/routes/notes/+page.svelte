<script lang="ts">
  import { onMount } from "svelte";
  import { goto } from "$app/navigation";
  import type { BookMeta, HighlightColor, LibrarySnapshot } from "$lib/types";
  import { fetchLibrarySnapshot, getCachedLibrarySnapshot } from "$lib/library/librarySnapshotCache";
  import { bookTitle, hashNum } from "$lib/library/bookInfo";
  import { HIGHLIGHT_COLORS, highlightFill } from "$lib/reading/highlights";
  import { bookNotesMarkdown, saveMarkdown } from "$lib/reading/exportNotes";
  import { constellationEdges, layoutStars, similarityMatrix, type StarPos } from "$lib/notes/constellation";
  import { assistant, embed } from "$lib/ai/assistant.svelte";
  import { toast, toastError } from "$lib/ui/toast.svelte";

  type Item = {
    key: string;
    kind: "highlight" | "comment" | "quote";
    path: string;
    meta: BookMeta;
    text: string;
    note?: string;
    color?: HighlightColor;
    where: string;
    createdAt: string;
    highlightId?: string;
  };

  let snapshot = $state<LibrarySnapshot | null>(getCachedLibrarySnapshot());
  let tab = $state<"feed" | "sky">("feed");
  let query = $state("");
  let colorFilter = $state<HighlightColor | "all">("all");
  let onlyNotes = $state(false);

  onMount(() => {
    void fetchLibrarySnapshot()
      .then((s) => (snapshot = s))
      .catch(() => {});
  });

  const items = $derived.by(() => {
    const out: Item[] = [];
    const books = snapshot?.metadata.books ?? {};
    for (const [path, meta] of Object.entries(books)) {
      for (const h of meta.highlights ?? []) {
        const where = [h.chapterLabel, h.page != null ? `стр. ${h.page}` : ""].filter(Boolean).join(" · ");
        out.push({
          key: `h-${h.id}`,
          kind: "highlight",
          path,
          meta,
          text: h.text,
          note: h.note,
          color: h.color,
          where,
          createdAt: h.createdAt,
          highlightId: h.id,
        });
      }
      for (const c of meta.comments ?? []) {
        out.push({
          key: `c-${c.id}`,
          kind: "comment",
          path,
          meta,
          text: c.excerpt || c.body,
          note: c.excerpt ? c.body : undefined,
          where: [c.chapterLabel, c.page != null ? `стр. ${c.page}` : ""].filter(Boolean).join(" · "),
          createdAt: c.createdAt,
        });
      }
      for (const q of meta.quotes ?? []) {
        out.push({ key: `q-${q.id}`, kind: "quote", path, meta, text: q.text, where: "цитата-карточка", createdAt: q.createdAt });
      }
    }
    return out.sort((a, b) => b.createdAt.localeCompare(a.createdAt));
  });

  const filtered = $derived.by(() => {
    const q = query.trim().toLocaleLowerCase("ru");
    return items.filter((it) => {
      if (colorFilter !== "all" && it.color !== colorFilter) return false;
      if (onlyNotes && !it.note?.trim()) return false;
      if (!q) return true;
      return [it.text, it.note, bookTitle(it.path, it.meta), it.meta.author]
        .filter(Boolean)
        .join(" ")
        .toLocaleLowerCase("ru")
        .includes(q);
    });
  });

  /** Группы по книгам — в порядке свежести последней заметки. */
  const groups = $derived.by(() => {
    const map = new Map<string, Item[]>();
    for (const it of filtered) {
      const list = map.get(it.path) ?? [];
      list.push(it);
      map.set(it.path, list);
    }
    return [...map.entries()].map(([path, list]) => ({ path, meta: list[0]!.meta, list }));
  });

  function openItem(it: Item) {
    const url = "/read?path=" + encodeURIComponent(it.path) + (it.highlightId ? "&hl=" + encodeURIComponent(it.highlightId) : "");
    void goto(url);
  }

  async function exportAll() {
    const books = snapshot?.metadata.books ?? {};
    const parts = Object.entries(books)
      .filter(([, m]) => (m.highlights?.length ?? 0) + (m.comments?.length ?? 0) + (m.quotes?.length ?? 0) > 0 || m.review?.trim())
      .map(([p, m]) => bookNotesMarkdown(p, m).replace(/^---[\s\S]*?---\n+/, ""));
    if (!parts.length) {
      toast("Пока нечего экспортировать", "info");
      return;
    }
    try {
      const md = `# Заметки из книг\n\n_${new Date().toLocaleDateString("ru")}_\n\n` + parts.join("\n\n---\n\n");
      const res = await saveMarkdown("Заметки из книг", md);
      if (res === "saved") toast("Все заметки сохранены", "success");
      else if (res === "copied") toast("Заметки скопированы в буфер", "success");
    } catch (e) {
      toastError(e, "Экспорт");
    }
  }

  // ——— Созвездие ———
  type Star = StarPos & { it: Item; r: number; hue: number; delay: number };
  let stars = $state<Star[]>([]);
  let edges = $state<[number, number, number][]>([]);
  let skyBusy = $state(false);
  let skyMode = $state<"words" | "meaning">("words");
  let picked = $state<Star | null>(null);
  let hovered = $state<Star | null>(null);
  let builtFor = "";

  const skyItems = $derived(items.filter((it) => it.kind !== "comment" || it.text.length > 20).slice(0, 360));
  const bookHues = $derived.by(() => {
    const m = new Map<string, number>();
    for (const it of skyItems) if (!m.has(it.path)) m.set(it.path, Math.round(hashNum(it.path) * 360));
    return m;
  });

  async function buildSky(mode: "words" | "meaning") {
    const list = skyItems;
    if (list.length < 2) {
      stars = [];
      edges = [];
      return;
    }
    skyBusy = true;
    try {
      const texts = list.map((it) => `${it.text} ${it.note ?? ""}`);
      let embeddings: number[][] | null = null;
      if (mode === "meaning") {
        try {
          embeddings = await embed(texts);
        } catch (e) {
          toastError(e, "Смысловое созвездие");
          skyMode = "words";
        }
      }
      await new Promise((r) => setTimeout(r, 10));
      const sim = similarityMatrix(texts, embeddings);
      const paths = [...new Set(list.map((i) => i.path))];
      const groupsIdx = list.map((i) => paths.indexOf(i.path));
      const pos = layoutStars(sim, groupsIdx);
      stars = list.map((it, i) => ({
        ...pos[i]!,
        it,
        r: 2.8 + Math.min(4, Math.sqrt(it.text.length) / 7) + (it.note ? 1.4 : 0),
        hue: bookHues.get(it.path) ?? 250,
        delay: hashNum(it.key) * 6,
      }));
      edges = constellationEdges(sim, embeddings ? 0.3 : 0.1);
    } finally {
      skyBusy = false;
    }
  }

  $effect(() => {
    if (tab !== "sky") return;
    const sig = skyItems.map((i) => i.key).join("|") + skyMode;
    if (sig === builtFor) return;
    builtFor = sig;
    void buildSky(skyMode);
  });

  const W = 1000;
  const H = 620;
  const focus = $derived(hovered ?? picked);
</script>

<div class="np">
  <header class="np-top">
    <button type="button" class="back" onclick={() => goto("/")}>← Библиотека</button>
    <div class="np-heading">
      <p class="kicker">всё, что вы отметили</p>
      <h1>Заметки</h1>
    </div>
    <div class="tabs" role="tablist">
      <button type="button" role="tab" aria-selected={tab === "feed"} class:on={tab === "feed"} onclick={() => (tab = "feed")}>Лента</button>
      <button type="button" role="tab" aria-selected={tab === "sky"} class:on={tab === "sky"} onclick={() => (tab = "sky")}>Созвездие</button>
    </div>
  </header>

  <main class="np-main">
    {#if tab === "feed"}
      <div class="tools">
        <label class="search">
          <svg viewBox="0 0 24 24" aria-hidden="true"><circle cx="11" cy="11" r="6.5" /><path d="m16 16 4 4" /></svg>
          <input type="search" placeholder="Поиск по заметкам и выделениям" bind:value={query} />
        </label>
        <div class="colors" role="group" aria-label="Цвет">
          <button type="button" class="cdot all" class:on={colorFilter === "all"} onclick={() => (colorFilter = "all")}>все</button>
          {#each HIGHLIGHT_COLORS as c (c.id)}
            <button
              type="button"
              class="cdot"
              class:on={colorFilter === c.id}
              style:--c={c.fill}
              aria-label={c.label}
              title={c.label}
              onclick={() => (colorFilter = colorFilter === c.id ? "all" : c.id)}
            ></button>
          {/each}
        </div>
        <label class="only"><input type="checkbox" bind:checked={onlyNotes} /> только с мыслями</label>
        <button type="button" class="btn" onclick={() => void exportAll()}>Экспорт в Markdown</button>
      </div>

      {#if items.length === 0}
        <div class="empty">
          <span aria-hidden="true">✎</span>
          <h2>Здесь соберутся ваши маргиналии</h2>
          <p>Выделяйте текст в книгах цветом и оставляйте мысли на полях — они появятся в общей ленте, в поиске и в созвездии.</p>
        </div>
      {:else if groups.length === 0}
        <p class="muted">Ничего не найдено.</p>
      {:else}
        {#each groups as g (g.path)}
          <section class="book">
            <header class="book-head">
              <h2>{bookTitle(g.path, g.meta)}</h2>
              {#if g.meta.author}<span>{g.meta.author}</span>{/if}
              <small>{g.list.length}</small>
            </header>
            <ul class="items">
              {#each g.list as it (it.key)}
                <li class="item" style:--hl={it.color ? highlightFill(it.color) : "var(--accent)"}>
                  <button type="button" class="item-btn" onclick={() => openItem(it)}>
                    <span class="item-text">{it.text.length > 420 ? it.text.slice(0, 420) + "…" : it.text}</span>
                    {#if it.note}<span class="item-note">{it.note}</span>{/if}
                    <span class="item-meta">
                      {it.where}{it.where ? " · " : ""}{new Date(it.createdAt).toLocaleDateString("ru", { day: "numeric", month: "short", year: "numeric" })}
                    </span>
                  </button>
                </li>
              {/each}
            </ul>
          </section>
        {/each}
      {/if}
    {:else}
      <div class="sky-tools">
        <p class="muted">
          Каждая звезда — ваше выделение. Похожие мысли из разных книг оказываются рядом и соединяются линиями.
        </p>
        <div class="seg" role="group" aria-label="Способ сравнения">
          <button type="button" class:on={skyMode === "words"} onclick={() => (skyMode = "words")}>По словам</button>
          <button
            type="button"
            class:on={skyMode === "meaning"}
            title={assistant.s.embedModel ? "Через модель эмбеддингов" : "Укажите модель эмбеддингов в настройках помощника"}
            onclick={() => (skyMode = "meaning")}>По смыслу</button
          >
        </div>
      </div>
      <div class="sky">
        {#if skyBusy}
          <div class="sky-hint">Раскладываю звёзды…</div>
        {:else if stars.length < 2}
          <div class="sky-hint">Нужно хотя бы две заметки, чтобы зажечь созвездие.</div>
        {/if}
        <svg viewBox="0 0 {W} {H}" preserveAspectRatio="xMidYMid meet" role="img" aria-label="Созвездие цитат">
          <defs>
            <radialGradient id="glow">
              <stop offset="0%" stop-color="#fff" stop-opacity="0.9" />
              <stop offset="100%" stop-color="#fff" stop-opacity="0" />
            </radialGradient>
          </defs>
          {#each edges as [a, b, s] (a + "-" + b)}
            {@const A = stars[a]}
            {@const B = stars[b]}
            {#if A && B}
              <line
                x1={A.x * W}
                y1={A.y * H}
                x2={B.x * W}
                y2={B.y * H}
                class="edge"
                class:lit={focus && (focus === A || focus === B)}
                stroke-opacity={Math.min(0.55, 0.12 + s * 0.6)}
              />
            {/if}
          {/each}
          {#each stars as st (st.it.key)}
            <g
              class="star"
              class:dim={focus && focus !== st}
              transform="translate({st.x * W} {st.y * H})"
              role="button"
              tabindex="0"
              aria-label={st.it.text.slice(0, 80)}
              onmouseenter={() => (hovered = st)}
              onmouseleave={() => (hovered = null)}
              onfocus={() => (hovered = st)}
              onblur={() => (hovered = null)}
              onclick={() => (picked = st)}
              onkeydown={(e) => e.key === "Enter" && (picked = st)}
            >
              <circle r={st.r * 4.2} fill="url(#glow)" opacity="0.25" style:animation-delay="{st.delay}s" class="halo" />
              <circle r={st.r} fill="hsl({st.hue} 70% 82%)" style:animation-delay="{st.delay}s" class="core" />
            </g>
          {/each}
        </svg>
        {#if focus}
          <div class="sky-card" style:--hue={focus.hue}>
            <p class="sky-text">«{focus.it.text.length > 360 ? focus.it.text.slice(0, 360) + "…" : focus.it.text}»</p>
            {#if focus.it.note}<p class="sky-note">{focus.it.note}</p>{/if}
            <p class="sky-by">{bookTitle(focus.it.path, focus.it.meta)}{focus.it.meta.author ? " — " + focus.it.meta.author : ""}</p>
            {#if picked === focus}
              <button type="button" class="btn" onclick={() => openItem(focus!.it)}>Открыть в книге</button>
            {/if}
          </div>
        {/if}
      </div>
      <ul class="legend">
        {#each [...bookHues.entries()] as [path, hue] (path)}
          <li><span style:background="hsl({hue} 70% 72%)"></span>{bookTitle(path, snapshot?.metadata.books[path])}</li>
        {/each}
      </ul>
    {/if}
  </main>
</div>

<style>
  .np {
    min-height: 100dvh;
    background: var(--bg-soft);
    color: var(--text-soft);
  }

  .np-top {
    max-width: 64rem;
    margin: 0 auto;
    display: flex;
    align-items: flex-end;
    gap: 1.4rem;
    padding: clamp(1rem, 3vw, 1.8rem) clamp(1rem, 5vw, 2rem) 1rem;
    flex-wrap: wrap;
  }

  .np-heading {
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
    font-size: 0.86rem;
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

  .tabs,
  .seg {
    display: flex;
    padding: 3px;
    border-radius: 999px;
    border: 1px solid var(--border-soft);
    background: var(--panel-soft);
  }

  .tabs button,
  .seg button {
    border: none;
    background: transparent;
    color: var(--muted);
    padding: 0.4rem 0.95rem;
    border-radius: 999px;
    cursor: pointer;
  }

  .tabs button.on,
  .seg button.on {
    background: var(--elevated-soft);
    color: var(--text-soft);
    font-weight: 600;
    box-shadow: 0 1px 4px rgba(0, 0, 0, 0.08);
  }

  .np-main {
    max-width: 64rem;
    margin: 0 auto;
    padding: 0 clamp(1rem, 5vw, 2rem) 3rem;
  }

  .tools {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 0.7rem;
    margin-bottom: 1.4rem;
  }

  .search {
    flex: 1 1 16rem;
    display: flex;
    align-items: center;
    gap: 0.5rem;
    padding: 0.5rem 0.9rem;
    border-radius: 999px;
    border: 1px solid var(--border-soft);
    background: var(--panel-elevated);
  }

  .search svg {
    width: 1rem;
    height: 1rem;
    fill: none;
    stroke: var(--muted);
    stroke-width: 1.8;
  }

  .search input {
    flex: 1;
    border: none;
    outline: none;
    background: transparent;
    color: var(--text-soft);
    font: inherit;
  }

  .colors {
    display: flex;
    align-items: center;
    gap: 0.3rem;
  }

  .cdot {
    width: 1.35rem;
    height: 1.35rem;
    border-radius: 50%;
    border: 2px solid transparent;
    background: var(--c);
    cursor: pointer;
    padding: 0;
  }

  .cdot.on {
    border-color: var(--text-soft);
  }

  .cdot.all {
    width: auto;
    padding: 0 0.5rem;
    border-radius: 999px;
    background: var(--panel-soft);
    color: var(--muted);
    font-size: 0.72rem;
    border-color: var(--border-soft);
  }

  .only {
    display: flex;
    align-items: center;
    gap: 0.35rem;
    font-size: 0.84rem;
    color: var(--muted);
  }

  .book {
    margin-bottom: 2rem;
  }

  .book-head {
    display: flex;
    align-items: baseline;
    gap: 0.7rem;
    margin-bottom: 0.7rem;
    padding-bottom: 0.4rem;
    border-bottom: 1px solid var(--border-soft);
  }

  .book-head h2 {
    margin: 0;
    font-family: "Literata Variable", Georgia, serif;
    font-weight: 600;
    font-size: 1.15rem;
  }

  .book-head span {
    color: var(--muted);
    font-size: 0.86rem;
  }

  .book-head small {
    margin-left: auto;
    color: var(--muted);
  }

  .items {
    list-style: none;
    margin: 0;
    padding: 0;
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(18rem, 1fr));
    gap: 0.7rem;
  }

  .item {
    border-radius: var(--radius-md);
    background: var(--panel-elevated);
    border: 1px solid color-mix(in srgb, var(--border-soft) 70%, transparent);
    border-left: 4px solid var(--hl);
    box-shadow: var(--shadow-soft);
    overflow: hidden;
  }

  .item-btn {
    width: 100%;
    height: 100%;
    display: flex;
    flex-direction: column;
    gap: 0.5rem;
    padding: 0.85rem 0.95rem;
    border: none;
    background: transparent;
    color: var(--text-soft);
    text-align: left;
    cursor: pointer;
    font: inherit;
  }

  .item-text {
    font-family: "Literata Variable", Georgia, serif;
    font-size: 0.95rem;
    line-height: 1.55;
  }

  .item-note {
    font-size: 0.86rem;
    line-height: 1.45;
    padding: 0.45rem 0.6rem;
    border-radius: 10px;
    background: color-mix(in srgb, var(--hl) 16%, var(--panel-soft));
  }

  .item-meta {
    margin-top: auto;
    font-size: 0.72rem;
    color: var(--muted);
  }

  .empty {
    text-align: center;
    padding: 4rem 1rem;
    color: var(--muted);
  }

  .empty span {
    font-size: 2.4rem;
  }

  .empty h2 {
    color: var(--text-soft);
    font-family: "Literata Variable", Georgia, serif;
    font-weight: 500;
  }

  .muted {
    color: var(--muted);
    font-size: 0.88rem;
    margin: 0;
  }

  /* ——— Созвездие: всегда ночное небо ——— */
  .sky-tools {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 1rem;
    margin-bottom: 0.9rem;
    flex-wrap: wrap;
  }

  .sky {
    position: relative;
    border-radius: var(--radius-xl);
    overflow: hidden;
    background:
      radial-gradient(ellipse at 30% 20%, rgba(120, 100, 200, 0.25), transparent 60%),
      radial-gradient(ellipse at 80% 90%, rgba(60, 120, 180, 0.2), transparent 55%),
      #0d0b1a;
    box-shadow: var(--shadow-book);
  }

  .sky svg {
    display: block;
    width: 100%;
    height: auto;
  }

  .sky-hint {
    position: absolute;
    inset: 0;
    display: grid;
    place-items: center;
    color: rgba(230, 225, 255, 0.7);
    font-size: 0.95rem;
    pointer-events: none;
  }

  .edge {
    stroke: #cfc6ff;
    stroke-width: 0.8;
    transition: stroke-opacity 0.3s ease;
  }

  .edge.lit {
    stroke-opacity: 0.9 !important;
    stroke-width: 1.3;
  }

  .star {
    cursor: pointer;
    outline: none;
    transition: opacity 0.3s ease;
  }

  .star.dim {
    opacity: 0.45;
  }

  .star .core {
    animation: twinkle 5s ease-in-out infinite;
  }

  .star .halo {
    animation: halo 5s ease-in-out infinite;
  }

  .star:hover .core,
  .star:focus .core {
    fill: #fff;
  }

  @keyframes twinkle {
    0%,
    100% {
      opacity: 1;
    }
    50% {
      opacity: 0.55;
    }
  }

  @keyframes halo {
    0%,
    100% {
      opacity: 0.18;
    }
    50% {
      opacity: 0.4;
    }
  }

  .sky-card {
    position: absolute;
    left: 1rem;
    bottom: 1rem;
    max-width: min(26rem, calc(100% - 2rem));
    padding: 0.9rem 1rem;
    border-radius: var(--radius-md);
    background: rgba(20, 17, 38, 0.86);
    border: 1px solid hsl(var(--hue) 50% 60% / 0.5);
    color: #eeeaff;
    backdrop-filter: blur(10px);
    display: flex;
    flex-direction: column;
    gap: 0.45rem;
  }

  .sky-text {
    margin: 0;
    font-family: "Literata Variable", Georgia, serif;
    font-size: 0.92rem;
    line-height: 1.5;
  }

  .sky-note {
    margin: 0;
    font-size: 0.82rem;
    color: #cfc8f0;
  }

  .sky-by {
    margin: 0;
    font-size: 0.74rem;
    color: hsl(var(--hue) 70% 80%);
  }

  .sky-card .btn {
    align-self: flex-start;
    background: rgba(255, 255, 255, 0.1);
    border-color: rgba(255, 255, 255, 0.25);
    color: #fff;
  }

  .legend {
    list-style: none;
    margin: 0.9rem 0 0;
    padding: 0;
    display: flex;
    flex-wrap: wrap;
    gap: 0.4rem 1rem;
    font-size: 0.78rem;
    color: var(--muted);
  }

  .legend li {
    display: flex;
    align-items: center;
    gap: 0.4rem;
  }

  .legend span {
    width: 0.6rem;
    height: 0.6rem;
    border-radius: 50%;
  }
</style>
