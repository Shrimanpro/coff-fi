<script lang="ts">
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import { player, setTheme, ui } from "../lib/state.svelte";
  import Icon from "./Icon.svelte";
  import Logo from "./Logo.svelte";
  import Search from "./Search.svelte";

  const mac = navigator.userAgent.includes("Mac");
  const view = $derived(ui.base);

  // The whole bar acts as a title bar (macOS has none with the overlay style):
  // press anywhere but a control to move the window, double-click to zoom.
  function drag(e: MouseEvent) {
    if (e.button !== 0 || (e.target as Element).closest("button, input, label, a, nav, .search")) return;
    e.preventDefault();
    const win = getCurrentWindow();
    if (e.detail === 2) win.toggleMaximize();
    else win.startDragging();
  }

  // The bar now catches the pointer, so pass scrolling through to the page beneath it.
  function wheel(e: WheelEvent) {
    document.querySelector("main")?.scrollBy({ top: e.deltaY, left: e.deltaX });
  }

  function go(v: "home" | "directory") {
    ui.collection = null;
    ui.base = v;
  }
</script>

<!-- svelte-ignore a11y_no_static_element_interactions (mouse-only window dragging) -->
<header class="top" class:mac onmousedown={drag} onwheel={wheel}>
  <div class="brand glass">
    <div class="logo-word">
      <Logo size={26} live={player.status.playing} />
      <span class="word">Coff-fi</span>
    </div>
    <span class="sep"></span>
    <button class="icon-btn theme" title={ui.theme === "dark" ? "Light mode" : "Dark mode"} onclick={() => setTheme(ui.theme === "dark" ? "light" : "dark")}>
      {#key ui.theme}
        <span class="swap"><Icon name={ui.theme === "dark" ? "sun" : "moon"} /></span>
      {/key}
    </button>
  </div>

  <nav class="seg glass">
    <span class="thumb" class:right={view === "directory"}></span>
    <button class:on={view === "home"} onclick={() => go("home")}>Home</button>
    <button class:on={view === "directory"} onclick={() => go("directory")}>Directory</button>
  </nav>

  <div class="right">
    <Search />
  </div>
</header>

<style>
  .top {
    position: fixed;
    inset: 0 0 auto 0;
    height: 72px;
    z-index: 45;
    display: flex;
    align-items: center;
    padding: 0 18px;
    user-select: none;
    -webkit-user-select: none;
  }
  .top.mac {
    padding-left: 84px;
  }
  .brand {
    display: flex;
    align-items: center;
    gap: 4px;
    height: 48px;
    padding: 0 8px 0 14px;
    border-radius: 999px;
  }
  .logo-word {
    display: flex;
    align-items: center;
    gap: 8px;
    margin-right: 6px;
  }
  .word {
    font-family: var(--font-serif);
    font-size: 23px;
    font-weight: 600;
    font-style: italic;
    letter-spacing: 0.01em;
  }
  .sep {
    width: 1px;
    height: 22px;
    background: var(--rule);
    margin: 0 4px;
  }
  .swap {
    display: grid;
    animation: swap 0.45s var(--spring);
  }
  @keyframes swap {
    from {
      transform: rotate(-90deg) scale(0.4);
      opacity: 0;
    }
  }
  .seg {
    position: absolute;
    left: 50%;
    transform: translateX(-50%);
    display: grid;
    grid-template-columns: 1fr 1fr;
    height: 42px;
    padding: 4px;
    border-radius: 999px;
  }
  .seg button {
    position: relative;
    z-index: 1;
    padding: 0 22px;
    border-radius: 999px;
    color: var(--text-2);
    font-weight: 600;
    font-size: 13.5px;
    transition: color 0.25s;
  }
  .seg button.on {
    color: var(--text);
  }
  .thumb {
    position: absolute;
    top: 4px;
    bottom: 4px;
    left: 4px;
    width: calc(50% - 4px);
    border-radius: 999px;
    background: var(--active);
    box-shadow: inset 0 1px 0 var(--glass-hi);
    transition: transform 0.45s var(--spring);
  }
  .thumb.right {
    transform: translateX(100%);
  }
  .right {
    margin-left: auto;
    display: flex;
    align-items: center;
    gap: 10px;
  }
  .source {
    display: flex;
    align-items: center;
    gap: 10px;
    height: 42px;
    padding: 0 8px 0 16px;
    border-radius: 999px;
    cursor: pointer;
    font-size: 13.5px;
    font-weight: 600;
    color: var(--text-2);
  }
  .dev-btn {
    padding: 0 13px;
  }
  .on-device {
    color: var(--accent);
    gap: 8px;
  }
  .source.busy {
    cursor: progress;
  }
  .busy .src-label {
    animation: pulse 1.2s ease-in-out infinite;
  }
  @keyframes pulse {
    50% {
      opacity: 0.45;
    }
  }
  .switch {
    position: relative;
    width: 44px;
    height: 26px;
  }
  .switch input {
    position: absolute;
    inset: 0;
    opacity: 0;
    margin: 0;
    cursor: inherit;
  }
  .switch span {
    position: absolute;
    inset: 0;
    border-radius: 999px;
    background: var(--hover);
    box-shadow: inset 0 0 0 1px var(--rule);
    transition: background-color 0.3s var(--ease);
    pointer-events: none;
  }
  .switch span::after {
    content: "";
    position: absolute;
    top: 3px;
    left: 3px;
    width: 20px;
    height: 20px;
    border-radius: 50%;
    background: var(--text);
    box-shadow: 0 1px 4px rgba(0, 0, 0, 0.3);
    transition: transform 0.4s var(--spring);
  }
  .switch input:checked + span {
    background: var(--accent);
  }
  .switch input:checked + span::after {
    transform: translateX(18px);
    background: #fff;
  }
</style>
