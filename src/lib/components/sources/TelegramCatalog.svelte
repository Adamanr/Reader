<script lang="ts">
  import { onDestroy, onMount, untrack } from "svelte";
  import { goto } from "$app/navigation";
  import { fetchLibrarySnapshot } from "$lib/library/librarySnapshotCache";
  import { touchBookSourceSeen, touchBookSourceSynced } from "$lib/sources/store";
  import { onImportProgress, sourceImportFiles, sourceListFiles } from "$lib/sources/telegram";
  import type { BookSource, ImportProgress, RemoteBookFile } from "$lib/sources/types";
  import { toast, toastError } from "$lib/ui/toast.svelte";
  import { formatDate, formatSize, pluralBooks } from "./format";

  interface Props {
    source: BookSource;
    libraryRoot: string | null;
    /** Каталог открыт — счётчик новых книг у источника можно обнулить. */
    onSeen?: () => void;
  }

  let { source, libraryRoot, onSeen }: Props = $props();

  let query = $state("");
  let files = $state<RemoteBookFile[]>([]);
  let nextOffsetId = $state<number | null>(null);
  let selected = $state<Record<string, boolean>>({});
  let listBusy = $state(false);
  let importBusy = $state(false);
  let progress = $state<ImportProgress | null>(null);
  /** Время прошлого визита: файлы новее помечаются «новое». Фиксируем при открытии. */
  const seenBefore = untrack(() => (source.lastSeenAt ? Date.parse(source.lastSeenAt) : null));
  let markedSeen = false;
  /** Защита от гонки: ответ старого запроса не должен перетирать список нового поиска. */
  let listSeq = 0;
  let unlistenProgress: (() => void) | null = null;

  const selectable = $derived(files.filter((f) => !f.inLibrary));
  const selectedFiles = $derived(files.filter((f) => selected[f.id]));
  const allSelected = $derived(selectable.length > 0 && selectable.every((f) => selected[f.id]));
  const canImport = $derived(!!libraryRoot && selectedFiles.length > 0 && !importBusy);

  onMount(() => {
    void onImportProgress((p) => {
      if (importBusy) progress = p;
    }).then((fn) => (unlistenProgress = fn));
  });

  onDestroy(() => unlistenProgress?.());

  function isNew(f: RemoteBookFile): boolean {
    if (seenBefore == null || !f.date) return false;
    const t = Date.parse(f.date);
    return Number.isFinite(t) && t > seenBefore;
  }

  function selectAll(on: boolean) {
    const next: Record<string, boolean> = {};
    if (on) for (const f of selectable) next[f.id] = true;
    selected = next;
  }

  async function loadFiles(more = false) {
    const seq = ++listSeq;
    listBusy = true;
    try {
      const page = await sourceListFiles({
        username: source.username,
        query,
        offsetId: more ? nextOffsetId : null,
      });
      if (seq !== listSeq) return;
      if (more) {
        const seen = new Set(files.map((f) => f.id));
        files = [...files, ...page.files.filter((f) => !seen.has(f.id))];
      } else {
        files = page.files;
        selected = {};
      }
      nextOffsetId = page.nextOffsetId ?? null;
      if (!files.length) toast("Файлов книг не найдено", "info");
      if (!markedSeen && !query.trim()) {
        markedSeen = true;
        void touchBookSourceSeen(source.id).then(() => onSeen?.());
      }
    } catch (e) {
      if (seq === listSeq) toastError(e, "Список файлов");
    } finally {
      if (seq === listSeq) listBusy = false;
    }
  }

  async function importSelected() {
    if (!canImport) return;
    importBusy = true;
    progress = { done: 0, total: selectedFiles.length, name: "" };
    try {
      const result = await sourceImportFiles({
        username: source.username,
        messageIds: selectedFiles.map((f) => f.messageId),
      });
      await touchBookSourceSynced(source.id);
      await fetchLibrarySnapshot().catch(() => null);

      const failedIds = new Set(result.failed.map((f) => f.messageId));
      files = files.map((f) =>
        selected[f.id] && !failedIds.has(f.messageId) ? { ...f, inLibrary: true } : f,
      );
      const next: Record<string, boolean> = {};
      for (const f of files) if (failedIds.has(f.messageId)) next[f.id] = true;
      selected = next;

      const parts: string[] = [];
      if (result.added.length) parts.push(`добавлено: ${pluralBooks(result.added.length)}`);
      if (result.existing.length) parts.push(`уже были: ${result.existing.length}`);
      if (parts.length) {
        const text = parts.join(", ");
        toast(text[0].toUpperCase() + text.slice(1), "success", {
          action: { label: "В библиотеку", run: () => void goto("/") },
        });
      }
      for (const f of result.failed) toast(`«${f.name}»: ${f.error}`, "error");
    } catch (e) {
      toastError(e, "Импорт");
    } finally {
      importBusy = false;
      progress = null;
    }
  }
</script>

<div class="toolbar">
  <label class="field grow">
    <span>Поиск в канале</span>
    <input
      type="search"
      bind:value={query}
      placeholder="название или автор"
      onkeydown={(e) => {
        if (e.key === "Enter") void loadFiles();
      }}
    />
  </label>
  <button type="button" class="btn" disabled={listBusy} onclick={() => void loadFiles()}>
    {listBusy && !files.length ? "…" : files.length ? "Обновить" : "Показать файлы"}
  </button>
</div>

{#if files.length > 0}
  <div class="sel-row">
    <button
      type="button"
      class="chip"
      onclick={() => selectAll(true)}
      disabled={allSelected || !selectable.length}
    >
      Выбрать все
    </button>
    <button type="button" class="chip" onclick={() => selectAll(false)} disabled={selectedFiles.length === 0}>
      Снять выбор
    </button>
    <span class="muted">{selectedFiles.length} из {selectable.length}</span>
  </div>

  <div class="table-wrap">
    <table>
      <thead>
        <tr>
          <th class="check"></th>
          <th>Имя</th>
          <th>Размер</th>
          <th>Тип</th>
          <th>Дата</th>
          <th>Подпись</th>
        </tr>
      </thead>
      <tbody>
        {#each files as f (f.id)}
          <tr class:done={f.inLibrary}>
            <td class="check">
              <input
                type="checkbox"
                aria-label="Выбрать {f.name}"
                checked={!!selected[f.id]}
                disabled={f.inLibrary || importBusy}
                onchange={(e) => {
                  selected = { ...selected, [f.id]: e.currentTarget.checked };
                }}
              />
            </td>
            <td class="name">
              {f.name}
              {#if f.inLibrary}<span class="badge">в библиотеке</span>
              {:else if isNew(f)}<span class="badge new">новое</span>{/if}
            </td>
            <td>{formatSize(f.size)}</td>
            <td class="ext">{f.ext}</td>
            <td>{formatDate(f.date)}</td>
            <td class="cap">{f.caption?.trim() || "—"}</td>
          </tr>
        {/each}
      </tbody>
    </table>
  </div>

  {#if nextOffsetId}
    <button type="button" class="chip" disabled={listBusy} onclick={() => void loadFiles(true)}>
      {listBusy ? "Загрузка…" : "Загрузить ещё"}
    </button>
  {/if}

  <div class="import-row">
    <button type="button" class="btn primary" disabled={!canImport} onclick={() => void importSelected()}>
      {importBusy ? "Импорт…" : "Добавить в библиотеку"}
    </button>
    {#if progress}
      <span class="muted" aria-live="polite">
        {progress.done} из {progress.total}{progress.name ? ` — ${progress.name}` : ""}
      </span>
    {/if}
  </div>
{:else if !listBusy}
  <p class="muted">Нажмите «Показать файлы», чтобы загрузить книги канала.</p>
{/if}
