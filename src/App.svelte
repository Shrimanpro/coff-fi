<script lang="ts">
  import { onMount } from "svelte";
  import BackSwipe from "./components/BackSwipe.svelte";
  import ContextMenu from "./components/ContextMenu.svelte";
  import DevicePicker from "./components/DevicePicker.svelte";
  import EdgePanel from "./components/EdgePanel.svelte";
  import FullPlayer from "./components/FullPlayer.svelte";
  import MiniPlayer from "./components/MiniPlayer.svelte";
  import QueueSidebar from "./components/QueueSidebar.svelte";
  import Settings from "./components/settings/Settings.svelte";
  import TrackInfo from "./components/TrackInfo.svelte";
  import Toast from "./components/Toast.svelte";
  import TopBar from "./components/TopBar.svelte";
  import TreeSidebar from "./components/TreeSidebar.svelte";
  import { api } from "./lib/ipc";
  import { goBack, init, player, position, toast, ui } from "./lib/state.svelte";
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
      if (!over[side] && !ui.menu) sides[side] = false;
      else if (ui.menu) armClose(side);
    }, delay);
  }

  function reveal(side: "left" | "right") {
    if (sides[side]) return;
    sides[side] = true;
    // If the cursor never actually enters the panel, tuck it away again.
    armClose(side, 900);
  }

  function onMove(e: MouseEvent) {
    if (ui.fullscreen || ui.settingsOpen || ui.info) return;
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
    // Back: Cmd+[ on macOS, Alt+Left elsewhere (the usual browser shortcuts).
    if (!ui.fullscreen && !ui.settingsOpen && !ui.info && ((e.metaKey && e.key === "[") || (e.altKey && e.key === "ArrowLeft"))) {
      e.preventDefault();
      goBack();
      return;
    }
    if (e.code === "Space") {
      e.preventDefault();
      if (player.status.track_id != null || player.track) api.cmd("toggle");
    } else if (mod && e.key === "ArrowRight") api.cmd("next");
    else if (mod && e.key === "ArrowLeft") api.cmd("prev");
    else if (ui.fullscreen && e.key === "ArrowRight") api.cmd("seek", position() + 5);
    else if (ui.fullscreen && e.key === "ArrowLeft") api.cmd("seek", Math.max(0, position() - 5));
    else if (mod && e.key === ",") ui.settingsOpen = true;
    else if (e.key === "Escape" && !ui.fullscreen && !ui.settingsOpen && !ui.info && !ui.menu && ui.collection) ui.collection = null;
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

<EdgePanel side="left" width={300} open={sides.left || ui.leftPinned} onenter={enter("left")} onleave={leave("left")}>
  <QueueSidebar />
</EdgePanel>
<EdgePanel side="right" width={330} open={sides.right} onenter={enter("right")} onleave={leave("right")}>
  <TreeSidebar />
</EdgePanel>

<MiniPlayer />

{#if ui.fullscreen && player.track}
  <FullPlayer />
{/if}

{#if ui.settingsOpen}
  <Settings />
{/if}

{#if ui.info}
  <TrackInfo />
{/if}

<BackSwipe />
<ContextMenu />
<DevicePicker />
<Toast />

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
