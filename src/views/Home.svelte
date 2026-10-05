<script lang="ts">
  import { open } from "@tauri-apps/plugin-dialog";
  import SongRow from "../components/SongRow.svelte";
  import TileC from "../components/Tile.svelte";
  import VirtualList from "../components/VirtualList.svelte";
  import { api } from "../lib/ipc";
  import { conf, toast, ui } from "../lib/state.svelte";
  import type { Home, Tile } from "../lib/types";

  let { scroller }: { scroller: HTMLElement | null } = $props();

  let data = $state<Home | null>(null);
  let allAlbums = $state(false);
  let allArtists = $state(false);

  $effect(() => {
    ui.libVersion;
    api.home().then((h) => (data = h)).catch((e) => toast(String(e)));
  });

  async function addFolder() {
    const dir = await open({ directory: true, multiple: false, title: "Choose a music folder" });
    if (typeof dir !== "string" || !conf.s || conf.s.roots.includes(dir)) return;
    conf.s.roots = [...conf.s.roots, dir];
    await api.setSettings($state.snapshot(conf.s)).catch((e) => toast(String(e)));
    api.rescan();
  }

  const greeting = (() => {
    const h = new Date().getHours();
    return h < 5 ? "Late night listening" : h < 12 ? "Good morning" : h < 18 ? "Good afternoon" : "Good evening";
  })();

  const SHELF = 18;
  const shelf = (list: Tile[], all: boolean) => (all ? list : list.slice(0, SHELF));
</script>

<div class="home">
  <header>
    <h1>{greeting}</h1>
    <p class="eyebrow">{new Date().toLocaleDateString(undefined, { weekday: "long", month: "long", day: "numeric" })}</p>
  </header>

  {#if !data}
    <div class="empty">Brewing your library…</div>
  {:else if data.songs.length === 0}
    <div class="empty">
      <h2>Your shelves are empty</h2>
      <p>{ui.scanning ? "Scanning your music folder…" : "Pick a music folder to start listening."}</p>
      <button class="pill-btn" onclick={addFolder}>Choose a folder</button>
    </div>
  {:else}
    {#if data.albums.length}
      <section>
        <div class="sec-head">
          <h2>Albums</h2>
          {#if data.albums.length > SHELF}
            <button class="more" onclick={() => (allAlbums = !allAlbums)}>{allAlbums ? "Show less" : `Show all ${data.albums.length}`}</button>
          {/if}
        </div>
        <div class="grid" class:shelf={!allAlbums}>
          {#each shelf(data.albums, allAlbums) as t (t.id)}
            <TileC tile={t} />
          {/each}
        </div>
      </section>
    {/if}

    {#if data.artists.length}
      <section>
        <div class="sec-head">
          <h2>Artists</h2>
          {#if data.artists.length > SHELF}
            <button class="more" onclick={() => (allArtists = !allArtists)}>{allArtists ? "Show less" : `Show all ${data.artists.length}`}</button>
          {/if}
        </div>
        <div class="grid small" class:shelf={!allArtists}>
          {#each shelf(data.artists, allArtists) as t (t.id)}
            <TileC tile={t} />
          {/each}
        </div>
      </section>
    {/if}

    <section>
      <div class="sec-head">
        <h2>Songs</h2>
        <span class="count">{data.songs.length}</span>
      </div>
      <VirtualList items={data.songs} rowHeight={56} {scroller}>
        {#snippet row(t, i)}
          <SongRow track={t} index={i} />
        {/snippet}
      </VirtualList>
    </section>
  {/if}
</div>

<style>
  header {
    margin-bottom: 26px;
  }
  h1 {
    font-size: 46px;
    font-style: italic;
    font-weight: 500;
    line-height: 1.05;
  }
  header .eyebrow {
    margin: 8px 0 0;
  }
  section {
    margin-bottom: 40px;
  }
  .sec-head {
    display: flex;
    align-items: baseline;
    gap: 14px;
    margin-bottom: 16px;
    padding-bottom: 8px;
    border-bottom: 1px solid var(--rule);
  }
  h2 {
    font-size: 28px;
  }
  .count {
    color: var(--text-3);
  }
  .more {
    margin-left: auto;
    color: var(--text-2);
    font-size: 13px;
  }
  .more:hover {
    color: var(--accent);
  }
  .grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(168px, 1fr));
    gap: 28px 22px;
  }
  .grid.big {
    grid-template-columns: repeat(auto-fill, minmax(230px, 1fr));
    gap: 32px 28px;
  }
  .grid.small {
    grid-template-columns: repeat(auto-fill, minmax(140px, 1fr));
  }
  .grid.shelf {
    grid-auto-flow: column;
    grid-template-columns: none;
    grid-auto-columns: 168px;
    overflow-x: auto;
    padding: 6px 4px 14px;
    margin: -6px -4px 0;
    scroll-snap-type: x proximity;
  }
  .grid.small.shelf {
    grid-auto-columns: 140px;
  }
  .grid.shelf > :global(*) {
    scroll-snap-align: start;
  }
  .empty {
    display: grid;
    justify-items: center;
    gap: 10px;
    padding: 80px 0;
    color: var(--text-2);
    text-align: center;
  }
  .empty h2 {
    font-size: 30px;
    color: var(--text);
  }
  .empty p {
    margin: 0 0 8px;
  }
</style>
