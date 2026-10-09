<script lang="ts">
  import { onMount } from "svelte";
  import { isTauriRuntime } from "$lib/isTauri";
  import {
    OFFLINE_TELEGRAM_STATUS,
    telegramLogout,
    telegramSendCode,
    telegramSignIn,
    telegramStatus,
  } from "$lib/sources/telegram";
  import type { TelegramStatus } from "$lib/sources/types";
  import { toast, toastError } from "$lib/ui/toast.svelte";

  /** Раньше API ID/Hash дублировались в localStorage — убираем остатки. */
  const LEGACY_CREDS_KEY = "reader.telegram.api";

  let apiId = $state("");
  let apiHash = $state("");
  let phone = $state("");
  let code = $state("");
  let password = $state("");
  let needPassword = $state(false);
  let passwordHint = $state<string | null>(null);
  let codeSent = $state(false);
  let status = $state<TelegramStatus>(OFFLINE_TELEGRAM_STATUS);
  let busy = $state(false);

  const connectedLabel = $derived(
    status.displayName?.trim() || status.phone?.trim() || "аккаунт",
  );
  /** Hash уже сохранён в приложении для этого API ID — поле можно оставить пустым. */
  const hashSaved = $derived(
    !!status.hasApiHash && !!status.apiId && String(status.apiId) === apiId.trim(),
  );

  onMount(() => {
    try {
      localStorage.removeItem(LEGACY_CREDS_KEY);
    } catch {
      /* хранилище недоступно — нечего чистить */
    }
    if (!isTauriRuntime()) return;
    void telegramStatus()
      .then((s) => {
        status = s;
        if (s.apiId) apiId = String(s.apiId);
        if (s.phone) phone = s.phone;
      })
      .catch(() => {});
  });

  function resetLoginForm() {
    codeSent = false;
    needPassword = false;
    passwordHint = null;
    code = "";
    password = "";
  }

  async function sendCode() {
    if (!isTauriRuntime()) return;
    const id = Number(apiId.trim());
    if (!Number.isInteger(id) || id <= 0) {
      toast("Укажите числовой API ID", "error");
      return;
    }
    if ((!apiHash.trim() && !hashSaved) || !phone.trim()) {
      toast("Заполните API Hash и телефон", "error");
      return;
    }
    busy = true;
    try {
      await telegramSendCode({ apiId: id, apiHash: apiHash.trim(), phone: phone.trim() });
      apiHash = "";
      status = { ...status, apiId: id, hasApiHash: true };
      resetLoginForm();
      codeSent = true;
      toast("Код отправлен в Telegram", "success");
    } catch (e) {
      toastError(e, "Telegram");
    } finally {
      busy = false;
    }
  }

  async function signIn() {
    if (!isTauriRuntime()) return;
    if (needPassword ? !password : !code.trim()) {
      toast(needPassword ? "Введите пароль 2FA" : "Введите код из Telegram", "error");
      return;
    }
    busy = true;
    try {
      // Пароль передаём как есть: пробелы по краям могут быть его частью.
      const result = await telegramSignIn({
        code: code.trim(),
        password: needPassword ? password : null,
      });
      if (result.needPassword) {
        needPassword = true;
        passwordHint = result.passwordHint ?? null;
        toast("Нужен пароль двухфакторной аутентификации", "info");
        return;
      }
      status = result.status ?? (await telegramStatus());
      resetLoginForm();
      toast("Вход в Telegram выполнен", "success");
    } catch (e) {
      password = "";
      toastError(e, "Telegram");
    } finally {
      busy = false;
    }
  }

  async function logout() {
    if (!isTauriRuntime()) return;
    busy = true;
    try {
      const revoked = await telegramLogout();
      status = { ...status, connected: false, displayName: null };
      resetLoginForm();
      if (revoked) {
        toast("Вы вышли из Telegram, сеанс завершён", "info");
      } else {
        toast(
          "Локальный сеанс удалён, но Telegram не подтвердил выход. Завершите сеанс «Reader» в Telegram → Настройки → Устройства.",
          "error",
        );
      }
    } catch (e) {
      toastError(e, "Telegram");
    } finally {
      busy = false;
    }
  }
</script>

<section class="card" id="telegram-auth" aria-labelledby="telegram-auth-h">
  <header>
    <h2 id="telegram-auth-h">Telegram</h2>
    <p>
      Войдите своим аккаунтом, чтобы подключать каналы как источники книг. API ID и Hash —
      на
      <a href="https://my.telegram.org" target="_blank" rel="noreferrer">my.telegram.org</a>
      (раздел API development tools).
    </p>
  </header>

  {#if !isTauriRuntime()}
    <p class="muted">Доступно в приложении.</p>
  {:else if status.connected}
    <div class="status">
      <p class="ok">Подключено: <strong>{connectedLabel}</strong></p>
      {#if status.phone && status.displayName}
        <p class="muted">{status.phone}</p>
      {/if}
      <p class="muted">
        {#if status.secureStorage === false}
          Связка ключей системы недоступна, поэтому ключ сеанса хранится в файле, доступном только
          вашему пользователю. Установите GNOME Keyring или KWallet, чтобы спрятать его надёжнее.
        {:else}
          Ключ сеанса хранится в связке ключей системы, а не в файлах приложения.
        {/if}
        Сеанс виден в Telegram → Настройки → Устройства как «Reader». Приложение лишь читает
        публичные каналы и скачивает выбранные файлы.
      </p>
      <button type="button" class="btn" disabled={busy} onclick={() => void logout()}>
        {busy ? "…" : "Выйти"}
      </button>
    </div>
  {:else}
    <p class="muted note">
      Код входа вводите только здесь и в официальном Telegram — никому его не пересылайте. Ключ
      сеанса будет храниться в связке ключей системы (GNOME Keyring, KWallet).
    </p>
    <label class="field">
      <span>API ID</span>
      <input
        type="text"
        inputmode="numeric"
        autocomplete="off"
        bind:value={apiId}
        placeholder="12345678"
        disabled={codeSent}
      />
    </label>
    <label class="field">
      <span>API Hash</span>
      <input
        type="password"
        autocomplete="off"
        bind:value={apiHash}
        placeholder={hashSaved ? "сохранён — оставьте пустым" : "32 символа"}
        disabled={codeSent}
      />
    </label>
    <label class="field">
      <span>Телефон</span>
      <input
        type="tel"
        autocomplete="tel"
        bind:value={phone}
        placeholder="+7…"
        disabled={codeSent}
      />
    </label>

    <div class="actions">
      {#if codeSent}
        <button type="button" class="btn" disabled={busy} onclick={resetLoginForm}>
          Изменить данные
        </button>
      {/if}
      <button type="button" class="btn" disabled={busy} onclick={() => void sendCode()}>
        {busy && !codeSent ? "…" : codeSent ? "Отправить код ещё раз" : "Отправить код"}
      </button>
    </div>

    {#if codeSent}
      {#if !needPassword}
        <label class="field">
          <span>Код из Telegram</span>
          <input
            type="text"
            inputmode="numeric"
            autocomplete="one-time-code"
            bind:value={code}
            onkeydown={(e) => {
              if (e.key === "Enter") void signIn();
            }}
          />
        </label>
      {:else}
        <label class="field">
          <span>Пароль 2FA{passwordHint ? ` (подсказка: ${passwordHint})` : ""}</span>
          <input
            type="password"
            autocomplete="current-password"
            bind:value={password}
            onkeydown={(e) => {
              if (e.key === "Enter") void signIn();
            }}
          />
        </label>
      {/if}
      <button type="button" class="btn" disabled={busy} onclick={() => void signIn()}>
        {busy ? "…" : "Войти"}
      </button>
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

  header a {
    color: var(--accent-2);
  }

  .muted {
    font-size: 0.8rem;
    line-height: 1.45;
    color: var(--muted);
    margin: 0;
  }

  .status {
    display: flex;
    flex-direction: column;
    gap: 0.4rem;
  }

  .note {
    padding: 0.55rem 0.7rem;
    border-radius: var(--radius-sm);
    background: color-mix(in srgb, var(--accent) 8%, transparent);
  }

  .ok {
    margin: 0;
    font-size: 0.9rem;
  }

  .field {
    display: flex;
    flex-direction: column;
    gap: 0.3rem;
    font-size: 0.8rem;
    color: var(--muted);
  }

  .field input {
    border: 1px solid var(--border-soft);
    background: var(--elevated-soft);
    color: var(--text-soft);
    border-radius: var(--radius-sm);
    padding: 0.5rem 0.65rem;
    font: inherit;
    font-size: 0.9rem;
  }

  .actions {
    display: flex;
    gap: 0.5rem;
    flex-wrap: wrap;
  }

  .btn {
    border: 1px solid var(--border-soft);
    background: var(--elevated-soft);
    color: var(--text-soft);
    border-radius: 999px;
    padding: 0.4rem 0.85rem;
    font-size: 0.82rem;
    cursor: pointer;
    align-self: flex-start;
  }

  .field input:disabled {
    opacity: 0.6;
  }

  .btn:disabled {
    opacity: 0.5;
    cursor: default;
  }
</style>
