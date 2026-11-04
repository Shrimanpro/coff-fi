<script lang="ts">
  // Large-title page header (serif, Coff-fi style) with an iOS back button when pushed.
  import type { Snippet } from "svelte";
  import Icon from "../components/Icon.svelte";
  import { m, pop } from "./nav.svelte";

  let { title, eyebrow = "", italic = false, actions }: { title: string; eyebrow?: string; italic?: boolean; actions?: Snippet } = $props();
  const canBack = $derived(m.stacks[m.tab].length > 0);
</script>

<div class="bar">
  {#if canBack}
    <button class="back" onclick={pop}><Icon name="chev-left" size={22} /><span>Back</span></button>
  {:else}
    <span></span>
  {/if}
  <div class="actions">{@render actions?.()}</div>
</div>
{#if title}
  <header>
    {#if eyebrow}<p class="eyebrow">{eyebrow}</p>{/if}
    <h1 class:italic>{title}</h1>
  </header>
{/if}

<style>
  .bar {
    display: flex;
    align-items: center;
    justify-content: space-between;
    min-height: 44px;
    padding: 0 8px 0 4px;
  }
  .back {
    display: flex;
    align-items: center;
    gap: 0;
    height: 44px;
    padding-right: 12px;
    font-size: 17px;
    color: var(--accent-2);
    -webkit-tap-highlight-color: transparent;
  }
  :global([data-theme="light"]) .back {
    color: var(--accent);
  }
  .actions {
    display: flex;
    align-items: center;
    gap: 4px;
  }
  .actions :global(.hbtn) {
    width: 38px;
    height: 38px;
    border-radius: 50%;
    display: grid;
    place-items: center;
    color: var(--text-2);
    background: var(--hover);
    -webkit-tap-highlight-color: transparent;
  }
  .actions :global(.hbtn:active) {
    background: var(--active);
  }
  header {
    padding: 2px 18px 14px;
  }
  h1 {
    font-size: 36px;
    line-height: 1.05;
  }
  h1.italic {
    font-style: italic;
  }
  .eyebrow {
    margin: 0 0 4px;
  }
</style>
