/** Окно встроено в Tauri (не браузер и не предпросмотр без webview). */
export function isTauriRuntime(): boolean {
  if (typeof window === "undefined") return false;
  return "__TAURI_INTERNALS__" in window || "__TAURI__" in window;
}
