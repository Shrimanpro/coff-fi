<script lang="ts">
  import type { Snippet } from "svelte";

  let {
    side,
    open,
    width,
    onenter,
    onleave,
    children,
  }: { side: "left" | "right"; open: boolean; width: number; onenter: () => void; onleave: () => void; children: Snippet } = $props();
</script>

<aside
  class="panel glass {side}"
  class:open
  style:width="{width}px"
  onmouseenter={onenter}
  onmouseleave={onleave}
  aria-hidden={!open}
  inert={!open}
>
  {@render children()}
</aside>

<style>
  .panel {
    position: fixed;
    top: 76px;
    bottom: 20px;
    z-index: 50;
    border-radius: 24px;
    background: var(--glass-strong);
    display: flex;
    flex-direction: column;
    overflow: hidden;
    opacity: 0;
    transition:
      transform 0.5s var(--spring),
      opacity 0.3s var(--ease);
  }
  .left {
    left: 14px;
    transform: translateX(calc(-100% - 30px)) scale(0.98);
  }
  .right {
    right: 14px;
    transform: translateX(calc(100% + 30px)) scale(0.98);
  }
  .open {
    transform: none;
    opacity: 1;
  }
</style>
