<script>import "../app.css";
import { browser } from "$app/environment";
import { initThemeFromStorage } from "$lib/theme";

let { children } = $props();

if (browser) {
  initThemeFromStorage();
}</script>

<div class="app-root">
  {@render children()}
</div>


<style>
  :global(html, body) {
    margin: 0;
    min-height: 100%;
    background: var(--bg-soft);
    color: var(--text-soft);
    -webkit-font-smoothing: antialiased;
  }

  :global(*) {
    box-sizing: border-box;
  }

  :global(button),
  :global(input),
  :global(select),
  :global(textarea) {
    font-family:
      system-ui,
      -apple-system,
      "Segoe UI",
      sans-serif;
  }

  .app-root {
    min-height: 100vh;
    min-height: 100dvh;
    position: relative;
  }
</style>
