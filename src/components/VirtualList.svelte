<script lang="ts" generics="T">
  import type { Snippet } from "svelte";

  let {
    items,
    rowHeight,
    scroller,
    row,
    overscan = 8,
  }: { items: T[]; rowHeight: number; scroller: HTMLElement | null | undefined; row: Snippet<[T, number]>; overscan?: number } = $props();

  let el: HTMLDivElement;
  let start = $state(0);
  let end = $state(40);

  function update() {
    if (!scroller || !el) return;
    const offset = el.getBoundingClientRect().top - scroller.getBoundingClientRect().top + scroller.scrollTop;
    const top = scroller.scrollTop - offset;
    start = Math.max(0, Math.floor(top / rowHeight) - overscan);
    end = Math.min(items.length, Math.ceil((top + scroller.clientHeight) / rowHeight) + overscan);
  }

  $effect(() => {
    items.length;
    const s = scroller;
    if (!s) return;
    update();
    s.addEventListener("scroll", update, { passive: true });
    const ro = new ResizeObserver(update);
    ro.observe(s);
    return () => {
      s.removeEventListener("scroll", update);
      ro.disconnect();
    };
  });

  const visible = $derived(items.slice(start, Math.max(start, end)));
</script>

<div bind:this={el} class="vlist" style:height="{items.length * rowHeight}px">
  <div class="win" style:transform="translateY({start * rowHeight}px)">
    {#each visible as item, i (start + i)}
      {@render row(item, start + i)}
    {/each}
  </div>
</div>

<style>
  .vlist {
    position: relative;
    contain: strict;
    width: 100%;
  }
  .win {
    position: absolute;
    inset: 0 0 auto 0;
    will-change: transform;
  }
</style>
