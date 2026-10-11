<script lang="ts">
  import { ui } from "../lib/state.svelte";

  let el = $state<HTMLDivElement | null>(null);
  let pos = $state({ x: 0, y: 0 });

  // Keep the menu inside the window.
  $effect(() => {
    const m = ui.menu;
    if (!m || !el) return;
    const r = el.getBoundingClientRect();
    pos = { x: Math.min(m.x, innerWidth - r.width - 8), y: Math.min(m.y, innerHeight - r.height - 8) };
  });

  const close = () => (ui.menu = null);
</script>

<svelte:window
  onkeydown={(e) => e.key === "Escape" && ui.menu && (e.stopPropagation(), close())}
  onblur={close}
/>

{#if ui.menu}
  <div class="scrim" role="presentation" onpointerdown={close} oncontextmenu={(e) => (e.preventDefault(), close())}></div>
  <div class="menu glass" role="menu" bind:this={el} style:left="{pos.x || ui.menu.x}px" style:top="{pos.y || ui.menu.y}px">
    {#each ui.menu.items as it (it.label)}
      <button
        role="menuitem"
        onclick={() => {
          close();
          it.action();
        }}>{it.label}</button
      >
    {/each}
  </div>
{/if}

<style>
  .scrim {
    position: fixed;
    inset: 0;
    z-index: 90;
  }
  .menu {
    position: fixed;
    z-index: 91;
    min-width: 190px;
    padding: 6px;
    border-radius: 14px;
    background: var(--glass-strong);
    display: flex;
    flex-direction: column;
    animation: pop 0.18s var(--ease);
    transform-origin: top left;
  }
  @keyframes pop {
    from {
      opacity: 0;
      transform: scale(0.94);
    }
  }
  button {
    text-align: left;
    padding: 8px 12px;
    border-radius: 9px;
    font-size: 13.5px;
  }
  button:hover {
    background: var(--accent);
    color: var(--accent-ink);
  }
</style>
