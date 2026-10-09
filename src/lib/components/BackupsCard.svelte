<script lang="ts">
  import { onMount } from "svelte";
  import { commands, type BackupInfo } from "$lib/bindings";
  import { isTauriRuntime } from "$lib/isTauri";
  import { flushStats } from "$lib/reading/stats.svelte";
  import { flushVocab } from "$lib/vocab/vocab.svelte";
  import { toast, toastError } from "$lib/ui/toast.svelte";

  let backups = $state<BackupInfo[]>([]);
  let loaded = $state(false);
  let busy = $state(false);
  /** Копия, для которой показан вопрос «точно восстановить?». */
  let confirmId = $state<string | null>(null);

  onMount(() => {
    if (isTauriRuntime()) void load();
  });

  async function load() {
    try {
      backups = await commands.metadataBackupsList();
    } catch (e) {
      toastError(e, "Резервные копии");
    } finally {
      loaded = true;
    }
  }

  function formatDate(ms: number): string {
    return new Date(ms).toLocaleString("ru-RU", {
      day: "numeric",
      month: "long",
      year: "numeric",
      hour: "2-digit",
      minute: "2-digit",
    });
  }

  function summary(b: BackupInfo): string {
    if (b.books == null) return "не удалось прочитать";
    return `книг: ${b.books}, цитат: ${b.quotes ?? 0}`;
  }

  async function createNow() {
    busy = true;
    try {
      const created = await commands.metadataBackupCreate();
      toast(created ? "Резервная копия создана" : "Пока нечего сохранять", created ? "success" : "info");
      await load();
    } catch (e) {
      toastError(e, "Резервная копия");
    } finally {
      busy = false;
    }
  }

  async function restore(id: string) {
    busy = true;
    try {
      // Отложенные записи статистики и словаря должны лечь на диск до восстановления,
      // иначе они перезапишут восстановленные файлы.
      await Promise.all([flushStats(), flushVocab()]);
      await commands.metadataBackupRestore(id);
      location.reload();
    } catch (e) {
      busy = false;
      confirmId = null;
      toastError(e, "Восстановление");
    }
  }
</script>

<section class="card" id="backups" aria-labelledby="backups-h">
  <header>
    <h2 id="backups-h">Резервные копии</h2>
    <p>
      Заметки, цитаты, прогресс, словарь и статистика копируются автоматически — не чаще раза в
      час, последние 24 копии и по одной за день в течение месяца. Копии хранятся на этом
      компьютере, отдельно от папки библиотеки.
    </p>
  </header>

  {#if !isTauriRuntime()}
    <p class="muted">Доступно в приложении.</p>
  {:else}
    <div class="actions">
      <button type="button" class="btn" disabled={busy} onclick={() => void createNow()}>
        Создать копию сейчас
      </button>
    </div>

    {#if loaded && backups.length === 0}
      <p class="muted">Копий пока нет — первая появится при следующем сохранении данных.</p>
    {:else if backups.length > 0}
      <ul class="list">
        {#each backups as b (b.id)}
          <li>
            <div class="meta">
              <span class="label">{formatDate(b.createdAtMs)}</span>
              <span class="sub">{summary(b)}</span>
            </div>
            {#if confirmId === b.id}
              <div class="confirm" role="group" aria-label="Подтверждение восстановления">
                <span class="sub">Текущие данные тоже сохранятся копией.</span>
                <button type="button" class="btn primary" disabled={busy} onclick={() => void restore(b.id)}>
                  {busy ? "…" : "Восстановить"}
                </button>
                <button type="button" class="btn" disabled={busy} onclick={() => (confirmId = null)}>
                  Отмена
                </button>
              </div>
            {:else}
              <button type="button" class="btn" disabled={busy} onclick={() => (confirmId = b.id)}>
                Восстановить…
              </button>
            {/if}
          </li>
        {/each}
      </ul>
    {/if}
  {/if}
</section>

<style>
  .card {
    display: flex;
    flex-direction: column;
    gap: 0.75rem;
    padding: 1.2rem 1.3rem;
    border-radius: var(--radius-lg);
    background: var(--panel-elevated);
    border: 1px solid color-mix(in srgb, var(--border-soft) 75%, transparent);
    box-shadow: var(--shadow-soft);
    color: var(--text-soft);
  }

  header h2 {
    margin: 0;
    font-size: 1.05rem;
    font-weight: 650;
  }

  header p {
    margin: 0.3rem 0 0;
    font-size: 0.86rem;
    line-height: 1.45;
    color: var(--muted);
  }

  .muted {
    font-size: 0.8rem;
    line-height: 1.45;
    color: var(--muted);
    margin: 0;
  }

  .actions {
    display: flex;
    gap: 0.5rem;
    flex-wrap: wrap;
  }

  .list {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 0.4rem;
    max-height: 22rem;
    overflow-y: auto;
  }

  .list li {
    display: flex;
    align-items: center;
    justify-content: space-between;
    flex-wrap: wrap;
    gap: 0.5rem;
    padding: 0.5rem 0.65rem;
    border-radius: var(--radius-sm);
    background: var(--elevated-soft);
  }

  .meta {
    display: flex;
    flex-direction: column;
    gap: 0.1rem;
    min-width: 0;
  }

  .label {
    font-size: 0.88rem;
    font-weight: 550;
  }

  .sub {
    font-size: 0.76rem;
    color: var(--muted);
  }

  .confirm {
    display: flex;
    align-items: center;
    flex-wrap: wrap;
    gap: 0.4rem;
  }

  .btn {
    border: 1px solid var(--border-soft);
    background: var(--elevated-soft);
    color: var(--text-soft);
    border-radius: 999px;
    padding: 0.4rem 0.85rem;
    font-size: 0.82rem;
    cursor: pointer;
  }

  .btn.primary {
    background: color-mix(in srgb, var(--accent) 28%, var(--elevated-soft));
    border-color: color-mix(in srgb, var(--accent-2) 35%, var(--border-soft));
    font-weight: 600;
  }

  .btn:disabled {
    opacity: 0.5;
    cursor: default;
  }
</style>
