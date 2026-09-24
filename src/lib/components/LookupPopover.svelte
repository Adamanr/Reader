<script lang="ts">
  import { onMount } from "svelte";
  import type { EpubReaderApi } from "$lib/types";
  import { translateStringList } from "$lib/translate/translateApi";
  import { ask, assistantReady } from "$lib/ai/assistant.svelte";
  import { explainTerm, findMentions, loadGlossary, saveGlossary, termKey, type GlossaryEntry } from "$lib/ai/glossary";
  import { addWord, hasWord, loadVocab, sentenceAround } from "$lib/vocab/vocab.svelte";
  import { toast } from "$lib/ui/toast.svelte";

  interface Props {
    kind: "word" | "who";
    term: string;
    context: string;
    rect: { left: number; top: number; width: number; height: number };
    api: EpubReaderApi | null;
    bookPath: string;
    bookTitle: string;
    progress: number | null;
    targetLang: string;
    onClose: () => void;
    onGlossaryChanged?: () => void;
  }
  let { kind, term, context, rect, api, bookPath, bookTitle, progress, targetLang, onClose, onGlossaryChanged }: Props =
    $props();

  let translation = $state("");
  let explanation = $state("");
  let answer = $state("");
  let stale = $state<GlossaryEntry | null>(null);
  let busy = $state(true);
  let error = $state<string | null>(null);
  let saved = $state(false);
  let mentionsCount = $state(0);

  const cleanTerm = $derived(term.trim().replace(/^[^\p{L}\p{N}]+|[^\p{L}\p{N}]+$/gu, ""));
  const sentence = $derived(sentenceAround(context || term, cleanTerm));
  const cyrillic = $derived(/[а-яё]/i.test(cleanTerm));

  const pos = $derived.by(() => {
    const vw = typeof window !== "undefined" ? window.innerWidth : 1200;
    const vh = typeof window !== "undefined" ? window.innerHeight : 800;
    const w = Math.min(360, vw - 24);
    const x = Math.min(vw - w - 12, Math.max(12, rect.left + rect.width / 2 - w / 2));
    const below = rect.top + rect.height + 320 < vh;
    return { x, w, y: below ? rect.top + rect.height + 10 : Math.max(12, rect.top - 10), below };
  });

  async function lookupWord() {
    await loadVocab();
    saved = hasWord(cleanTerm);
    const jobs: Promise<void>[] = [];
    if (!(cyrillic && targetLang === "ru")) {
      jobs.push(
        translateStringList([cleanTerm], "auto", targetLang)
          .then(([t]) => {
            translation = t ?? "";
          })
          .catch((e) => {
            if (!assistantReady()) error = e instanceof Error ? e.message : String(e);
          }),
      );
    }
    if (assistantReady()) {
      jobs.push(
        ask(
          "Ты — словарь для читателя. Отвечай по-русски, кратко.",
          `Слово или выражение: «${cleanTerm}». Предложение из книги: «${sentence}».\n` +
            "Дай перевод или толкование именно в этом контексте (1–2 предложения). Без вступлений.",
          200,
        )
          .then((t) => {
            explanation = t;
          })
          .catch((e) => {
            error = e instanceof Error ? e.message : String(e);
          }),
      );
    }
    if (!jobs.length) error = "Подключите сервер перевода или локальную модель в настройках.";
    await Promise.all(jobs);
  }

  async function lookupWho(force = false) {
    const key = termKey(cleanTerm);
    const glossary = await loadGlossary(bookPath);
    const cached = glossary[key];
    const now = progress ?? 0;
    if (cached && !force && cached.progress <= now + 0.001) {
      answer = cached.answer;
      mentionsCount = cached.mentions;
      if (now - cached.progress > 0.05) stale = cached;
      return;
    }
    if (!api?.chunksBefore) {
      error = "Для этого формата пока недоступно.";
      return;
    }
    const chunks = await api.chunksBefore();
    const mentions = findMentions(chunks, cleanTerm);
    mentionsCount = mentions.length;
    const res = await explainTerm({ term: cleanTerm, bookTitle, mentions });
    answer = res.answer;
    stale = null;
    if (mentions.length) {
      glossary[key] = {
        term: cleanTerm,
        answer: res.answer,
        progress: now,
        mentions: mentions.length,
        at: new Date().toISOString(),
        ai: res.ai,
      };
      await saveGlossary(bookPath, glossary);
      onGlossaryChanged?.();
    }
  }

  async function run(force = false) {
    busy = true;
    error = null;
    try {
      if (kind === "word") await lookupWord();
      else await lookupWho(force);
    } catch (e) {
      error = e instanceof Error ? e.message : String(e);
    } finally {
      busy = false;
    }
  }

  onMount(() => {
    void run();
  });

  async function save() {
    await addWord({
      word: cleanTerm,
      translation: translation || explanation.split(/[.;]/)[0] || "",
      explanation: explanation || undefined,
      context: sentence,
      bookPath,
      bookTitle,
    });
    saved = true;
    toast(`«${cleanTerm}» — в ваших словах`, "success");
  }
</script>

<!-- svelte-ignore a11y_click_events_have_key_events -->
<!-- svelte-ignore a11y_no_static_element_interactions -->
<div class="lp-back" onclick={onClose}></div>
<div class="lp" class:above={!pos.below} style="left:{pos.x}px; top:{pos.y}px; width:{pos.w}px" role="dialog" aria-label={kind === "word" ? "Перевод" : "Кто это?"}>
  <header>
    <span class="kind">{kind === "word" ? "Слово" : "Кто это?"}</span>
    <h3>{cleanTerm}</h3>
    <button type="button" class="x" aria-label="Закрыть" onclick={onClose}>×</button>
  </header>

  {#if busy}
    <div class="loading"><span></span><span></span><span></span></div>
    {#if kind === "who"}<p class="muted">Перебираю прочитанные страницы…</p>{/if}
  {:else}
    {#if error}<p class="err">{error}</p>{/if}
    {#if kind === "word"}
      {#if translation}<p class="tr">{translation}</p>{/if}
      {#if explanation}<p class="ex">{explanation}</p>{/if}
      {#if sentence}<blockquote>{sentence}</blockquote>{/if}
      <footer>
        <button type="button" class="btn primary" disabled={saved || (!translation && !explanation)} onclick={() => void save()}>
          {saved ? "Уже в словах ✓" : "+ В мои слова"}
        </button>
        <a class="btn" href="/words">Мои слова</a>
      </footer>
    {:else}
      <p class="ans">{answer}</p>
      <p class="muted">
        {mentionsCount ? `Упоминаний в прочитанном: ${mentionsCount}.` : ""}
        {#if !assistantReady()}Подключите помощника в настройках — ответ станет связным рассказом.{/if}
      </p>
      {#if stale}
        <button type="button" class="btn" onclick={() => void run(true)}>Обновить с учётом прочитанного</button>
      {/if}
    {/if}
  {/if}
</div>

<style>
  .lp-back {
    position: fixed;
    inset: 0;
    z-index: 540;
  }

  .lp {
    position: fixed;
    z-index: 550;
    display: flex;
    flex-direction: column;
    gap: 0.6rem;
    padding: 0.85rem 0.95rem;
    border-radius: var(--radius-md);
    background: var(--toolbar-surface);
    border: 1px solid var(--toolbar-border);
    box-shadow: var(--shadow-float);
    color: var(--text-soft);
    animation: in 0.15s ease-out;
    max-height: 70vh;
    overflow: auto;
  }

  .lp.above {
    transform: translateY(-100%);
  }

  header {
    display: flex;
    align-items: baseline;
    gap: 0.5rem;
  }

  .kind {
    font-size: 0.66rem;
    text-transform: uppercase;
    letter-spacing: 0.12em;
    color: var(--muted);
  }

  h3 {
    margin: 0;
    flex: 1;
    font-family: "Literata Variable", Georgia, serif;
    font-size: 1.15rem;
    font-weight: 600;
  }

  .x {
    border: none;
    background: transparent;
    color: var(--muted);
    font-size: 1.2rem;
    cursor: pointer;
  }

  .tr {
    margin: 0;
    font-size: 1.05rem;
    font-weight: 600;
    color: var(--accent-2);
  }

  .ex,
  .ans {
    margin: 0;
    font-size: 0.9rem;
    line-height: 1.55;
    white-space: pre-wrap;
  }

  blockquote {
    margin: 0;
    padding-left: 0.6rem;
    border-left: 3px solid var(--accent);
    font-family: "Literata Variable", Georgia, serif;
    font-size: 0.84rem;
    line-height: 1.45;
    color: var(--muted);
  }

  .muted {
    margin: 0;
    font-size: 0.74rem;
    color: var(--muted);
  }

  .err {
    margin: 0;
    font-size: 0.82rem;
    color: var(--danger);
  }

  footer {
    display: flex;
    gap: 0.4rem;
  }

  .btn {
    border: 1px solid var(--border-soft);
    background: var(--elevated-soft);
    color: var(--text-soft);
    border-radius: 999px;
    padding: 0.35rem 0.8rem;
    font-size: 0.8rem;
    cursor: pointer;
    text-decoration: none;
    align-self: flex-start;
  }

  .btn.primary {
    background: var(--accent-2);
    border-color: var(--accent-2);
    color: var(--elevated-soft);
  }

  .btn:disabled {
    opacity: 0.6;
    cursor: default;
  }

  .loading {
    display: flex;
    gap: 0.3rem;
    padding: 0.3rem 0;
  }

  .loading span {
    width: 0.45rem;
    height: 0.45rem;
    border-radius: 50%;
    background: var(--accent);
    animation: dot 1s ease-in-out infinite;
  }

  .loading span:nth-child(2) {
    animation-delay: 0.15s;
  }
  .loading span:nth-child(3) {
    animation-delay: 0.3s;
  }

  @keyframes dot {
    0%,
    100% {
      opacity: 0.3;
      transform: translateY(0);
    }
    50% {
      opacity: 1;
      transform: translateY(-3px);
    }
  }

  @keyframes in {
    from {
      opacity: 0;
    }
  }
</style>
