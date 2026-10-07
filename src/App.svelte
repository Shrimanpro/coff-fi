<script lang="ts">
  import { onMount } from "svelte";
  import EdgePanel from "./components/EdgePanel.svelte";
  import MiniPlayer from "./components/MiniPlayer.svelte";
  import TopBar from "./components/TopBar.svelte";
  import TreeSidebar from "./components/TreeSidebar.svelte";
  import { api } from "./lib/ipc";
  import { init, player, toast, ui } from "./lib/state.svelte";
  import CollectionView from "./views/CollectionView.svelte";
  import Directory from "./views/Directory.svelte";
  import Home from "./views/Home.svelte";

  let scroller = $state<HTMLElement | null>(null);
  let ready = $state(false);

  onMount(() => {
    init()
      .catch((e) => toast(String(e)))
      .finally(() => (ready = true));
  });

  // ---- edge-reveal sidebars ----
  const EDGE = 16;
  const sides = $state({ left: false, right: false });
  const over = { left: false, right: false };
  const timers: Record<"left" | "right", ReturnType<typeof setTimeout> | undefined> = { left: undefined, right: undefined };

  function armClose(side: "left" | "right", delay = 420) {
    clearTimeout(timers[side]);
    timers[side] = setTimeout(() => {
      if (!over[side]) sides[side] = false;
    }, delay);
  }

  function reveal(side: "left" | "right") {
    if (sides[side]) return;
    sides[side] = true;
    // If the cursor never actually enters the panel, tuck it away again.
    armClose(side, 900);
  }

  function onMove(e: MouseEvent) {
    if (e.clientX <= EDGE && e.clientY > 60) reveal("left");
    else if (e.clientX >= innerWidth - EDGE && e.clientY > 60) reveal("right");
  }

  const enter = (side: "left" | "right") => () => {
    over[side] = true;
    clearTimeout(timers[side]);
  };
  const leave = (side: "left" | "right") => () => {
    over[side] = false;
    armClose(side);
  };

  // ---- keyboard ----
  function onKey(e: KeyboardEvent) {
    const tag = (e.target as HTMLElement)?.tagName;
    if (tag === "INPUT" || tag === "TEXTAREA" || tag === "SELECT") return;
    const mod = e.ctrlKey || e.metaKey;
    if (e.code === "Space") {
      e.preventDefault();
      if (player.status.track_id != null || player.track) api.cmd("toggle");
    } else if (mod && e.key === "ArrowRight") api.cmd("next");
    else if (mod && e.key === "ArrowLeft") api.cmd("prev");
  }

  // Start each page at the top.
  $effect(() => {
    ui.base;
    ui.dirPath;
    ui.collection;
    if (scroller) scroller.scrollTop = 0;
  });
</script>

<svelte:window onmousemove={onMove} onkeydown={onKey} />

<TopBar />

<main bind:this={scroller}>
  <div class="page">
    {#if !ready}
      <!-- first paint -->
    {:else if ui.collection}
      {#key `${ui.collection.kind}:${ui.collection.id}`}
        <CollectionView kind={ui.collection.kind} id={ui.collection.id} {scroller} />
      {/key}
    {:else if ui.base === "home"}
      <Home {scroller} />
    {:else}
      <Directory {scroller} />
    {/if}
  </div>
</main>

<EdgePanel side="right" width={330} open={sides.right} onenter={enter("right")} onleave={leave("right")}>
  <TreeSidebar />
</EdgePanel>

<MiniPlayer />


<style>
  main {
    position: fixed;
    inset: 0;
    overflow-y: auto;
    overflow-x: hidden;
    scrollbar-gutter: stable;
  }
  .page {
    max-width: 1500px;
    margin: 0 auto;
    padding: 96px 56px 150px;
  }
</style>
