<script lang="ts">
  // iOS-style bottom action sheet; replaces the desktop right-click menus.
  import { fade, fly } from "svelte/transition";
  import Icon from "../components/Icon.svelte";
  import { haptic } from "../lib/haptics";
  import { m } from "./nav.svelte";

  const close = () => (m.sheet = null);
</script>

{#if m.sheet}
  {@const s = m.sheet}
  <div class="scrim" transition:fade={{ duration: 180 }} role="presentation" onclick={close}></div>
  <div class="sheet" transition:fly={{ y: 320, duration: 340, opacity: 1 }} role="menu">
    <div class="group glass">
      {#if s.title}
        <div class="head">
          <div class="t ellipsis">{s.title}</div>
          {#if s.subtitle}<div class="st ellipsis">{s.subtitle}</div>{/if}
        </div>
      {/if}
      {#each s.items as it (it.label)}
        <button class="item" class:danger={it.danger} role="menuitem" onclick={() => (haptic("selection"), close(), it.action())}>
          <span>{it.label}</span>
          {#if it.icon}<Icon name={it.icon} size={19} />{/if}
        </button>
      {/each}
    </div>
    <button class="cancel glass" onclick={close}>Cancel</button>
  </div>
{/if}

<style>
  .scrim {
    position: fixed;
    inset: 0;
    z-index: 200;
    background: rgba(0, 0, 0, 0.35);
  }
  .sheet {
    position: fixed;
    left: 10px;
    right: 10px;
    bottom: calc(env(safe-area-inset-bottom) + 10px);
    z-index: 201;
    display: flex;
    flex-direction: column;
    gap: 8px;
  }
  .group,
  .cancel {
    border-radius: 16px;
    background: var(--glass-strong);
    overflow: hidden;
  }
  .head {
    padding: 14px 18px 10px;
    text-align: center;
    border-bottom: 0.5px solid var(--rule);
  }
  .t {
    font-weight: 600;
    font-size: 14px;
  }
  .st {
    font-size: 12.5px;
    color: var(--text-2);
    margin-top: 2px;
  }
  .item {
    width: 100%;
    display: flex;
    align-items: center;
    justify-content: space-between;
    min-height: 54px;
    padding: 0 20px;
    font-size: 17px;
    text-align: left;
    color: var(--text);
    -webkit-tap-highlight-color: transparent;
  }
  .item + .item {
    border-top: 0.5px solid var(--rule);
  }
  .item:active,
  .cancel:active {
    background: var(--active);
  }
  .item :global(svg) {
    color: var(--text-2);
  }
  .danger,
  .danger :global(svg) {
    color: #e0605a !important;
  }
  .cancel {
    min-height: 56px;
    font-size: 17px;
    font-weight: 600;
    color: var(--accent-2);
  }
  :global([data-theme="light"]) .cancel {
    color: var(--accent);
  }
</style>
