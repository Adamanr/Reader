export type ToastKind = "info" | "success" | "error";

export interface ToastItem {
  id: number;
  kind: ToastKind;
  text: string;
  action?: { label: string; run: () => void };
}

let nextId = 1;

export const toasts = $state<{ items: ToastItem[] }>({ items: [] });

export function dismissToast(id: number) {
  toasts.items = toasts.items.filter((t) => t.id !== id);
}

/** Неблокирующее уведомление вместо alert(). */
export function toast(
  text: string,
  kind: ToastKind = "info",
  opts: { action?: ToastItem["action"]; timeout?: number } = {},
) {
  const id = nextId++;
  toasts.items = [...toasts.items.slice(-3), { id, kind, text, action: opts.action }];
  const timeout = opts.timeout ?? (kind === "error" ? 7000 : 3800);
  if (timeout > 0) setTimeout(() => dismissToast(id), timeout);
  return id;
}

export function toastError(e: unknown, prefix = "") {
  const msg = e instanceof Error ? e.message : String(e);
  toast(prefix ? `${prefix}: ${msg}` : msg, "error");
}
