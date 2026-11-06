<script lang="ts">
  // Root of the iOS layout: four tabs with their own navigation stacks, the floating
  // tab bar + mini player, Now Playing, and the shared sheets (settings, song info, actions).
  import { onMount } from "svelte";
  import { cubicOut } from "svelte/easing";
  import DevicePicker from "../components/DevicePicker.svelte";
  import Settings from "../components/settings/Settings.svelte";
  import Toast from "../components/Toast.svelte";
  import TrackInfo from "../components/TrackInfo.svelte";
  import { init, player, toast, ui } from "../lib/state.svelte";
  import ActionSheet from "./ActionSheet.svelte";
  import ContextMenu from "./ContextMenu.svelte";
  import "./mobile.css";
  import { m, pop, stage, top } from "./nav.svelte";
  import NowPlaying from "./NowPlaying.svelte";
  import CollectionPage from "./pages/CollectionPage.svelte";
  import FolderPage from "./pages/FolderPage.svelte";
  import HomePage from "./pages/HomePage.svelte";
  import LibraryPage from "./pages/LibraryPage.svelte";
  import ListPage from "./pages/ListPage.svelte";
  import PlaylistsPage from "./pages/PlaylistsPage.svelte";
  import SearchPage from "./pages/SearchPage.svelte";
  import TabBar from "./TabBar.svelte";

  let scroller = $state<HTMLElement | null>(null);
  let stageEl = $state<HTMLElement>();
  $effect(() => {
    stage.el = stageEl ?? null;
  });
  let ready = $state(false);

  onMount(() => {
    init()
      .catch((e) => toast(String(e)))
      .finally(() => (ready = true));
  });

  const page = $derived(top());
  const key = $derived(`${m.tab}/${m.stacks[m.tab].length}/${JSON.stringify(page)}`);

  // Remember each page's scroll position (tabs keep their place, like iOS).
  const scrolls = new Map<string, number>();
  let lastKey = "";
  $effect.pre(() => {
    key;
    if (scroller && lastKey) scrolls.set(lastKey, scroller.scrollTop);
  });
  $effect(() => {
    const k = key;
    lastKey = k;
    if (scroller) scroller.scrollTop = m.dir === 1 ? 0 : (scrolls.get(k) ?? 0);
    lastY = scroller?.scrollTop ?? 0;
  });

  // Collapse the tab bar while scrolling down with something playing; expand when scrolling up.
  let lastY = 0;
  function onScroll() {
    if (!scroller) return;
    const y = scroller.scrollTop;
    const d = y - lastY;
    if (y < 40) m.collapsed = false;
    else if (d > 6 && player.track) m.collapsed = true;
    else if (d < -6) m.collapsed = false;
    if (Math.abs(d) > 6 || y < 40) lastY = y;
  }

  // Swipe from the left edge to go back.
  let edge: { x: number; y: number } | null = null;
  let edgeDx = $state(0);
  function edgeDown(e: PointerEvent) {
    if (e.clientX < 22 && m.stacks[m.tab].length) {
      edge = { x: e.clientX, y: e.clientY };
      edgeDx = 0;
    }
  }
  function edgeMove(e: PointerEvent) {
    if (!edge) return;
    const dx = e.clientX - edge.x;
    if (Math.abs(e.clientY - edge.y) > 40 && dx < 30) {
      edge = null;
      edgeDx = 0;
      return;
    }
    edgeDx = Math.max(0, dx);
  }
  function edgeUp() {
    if (edge && edgeDx > 90) pop();
    edge = null;
    edgeDx = 0;
  }

  function pageIn(_: Element) {
    const d = m.dir;
    if (d === 0) return { duration: 160, css: (k: number) => `opacity: ${k}` };
    return { duration: 340, easing: cubicOut, css: (k: number) => `transform: translateX(${(1 - k) * 30 * d}%); opacity: ${0.4 + 0.6 * k}` };
  }
</script>

<div class="stage" bind:this={stageEl}>
<main
  bind:this={scroller}
  onscroll={onScroll}
  onpointerdown={edgeDown}
  onpointermove={edgeMove}
  onpointerup={edgeUp}
  onpointercancel={edgeUp}
  class:has-player={!!player.track}
>
  {#if ready}
    {#key key}
      <div class="page" in:pageIn style:transform={edgeDx ? `translateX(${edgeDx * 0.6}px)` : null}>
        {#if page?.type === "collection"}
          <CollectionPage kind={page.kind} id={page.id} focus={page.focus} {scroller} />
        {:else if page?.type === "list"}
          <ListPage which={page.which} {scroller} />
        {:else if page?.type === "folder"}
          <FolderPage path={page.path} {scroller} />
        {:else if m.tab === "home"}
          <HomePage {scroller} />
        {:else if m.tab === "library"}
          <LibraryPage />
        {:else if m.tab === "playlists"}
          <PlaylistsPage />
        {:else}
          <SearchPage />
        {/if}
      </div>
    {/key}
  {/if}
</main>

<!-- Status bar backdrop so content doesn't collide with the clock / Dynamic Island. -->
<div class="statusbar"></div>

<TabBar />
</div>

{#if m.nowPlaying}
  <NowPlaying />
{/if}

{#if ui.settingsOpen}
  <Settings />
{/if}

{#if ui.info}
  <TrackInfo />
{/if}

<ActionSheet />
<ContextMenu />
<DevicePicker />
<Toast />

<style>
  /* Pages + tab bar. Fixed children stay relative to it, so it can be scaled as one card. */
  .stage {
    position: fixed;
    inset: 0;
    overflow: hidden;
    transform-origin: 50% 0;
    background:
      radial-gradient(700px 420px at 20% -10%, color-mix(in srgb, var(--accent) 12%, transparent), transparent 65%),
      var(--bg);
    will-change: transform;
  }
  main {
    position: fixed;
    inset: 0;
    overflow-y: auto;
    overflow-x: hidden;
    -webkit-overflow-scrolling: touch;
    padding-top: env(safe-area-inset-top);
    padding-bottom: calc(env(safe-area-inset-bottom) + 96px);
    overscroll-behavior-y: contain;
  }
  main.has-player {
    padding-bottom: calc(env(safe-area-inset-bottom) + 166px);
  }
  .page {
    min-height: 100%;
    will-change: transform;
  }
  .statusbar {
    position: fixed;
    top: 0;
    left: 0;
    right: 0;
    height: env(safe-area-inset-top);
    z-index: 40;
    background: color-mix(in srgb, var(--bg) 78%, transparent);
    backdrop-filter: blur(18px) saturate(160%);
    -webkit-backdrop-filter: blur(18px) saturate(160%);
    pointer-events: none;
  }
</style>
