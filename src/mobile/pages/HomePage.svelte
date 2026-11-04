<script lang="ts">
  // Mirrors the desktop Home: greeting, smart playlists, Navidrome playlists, albums, artists and all songs.
  import Icon from "../../components/Icon.svelte";
  import VirtualList from "../../components/VirtualList.svelte";
  import { api } from "../../lib/ipc";
  import { remote as connect } from "../../lib/remote.svelte";
  import { conf, playTracks, setSource, setTheme, toast, ui } from "../../lib/state.svelte";
  import type { Home } from "../../lib/types";
  import MSongRow from "../MSongRow.svelte";
  import MTile from "../MTile.svelte";
  import { push, selectTab } from "../nav.svelte";
  import PageHead from "../PageHead.svelte";

  let { scroller }: { scroller: HTMLElement | null } = $props();
  let data = $state<Home | null>(null);

  $effect(() => {
    ui.libVersion;
    ui.statsVersion;
    api.home().then((h) => (data = h)).catch((e) => toast(String(e)));
  });

  const greeting = (() => {
    const h = new Date().getHours();
    return h < 5 ? "Late night listening" : h < 12 ? "Good morning" : h < 18 ? "Good afternoon" : "Good evening";
  })();
  const date = new Date().toLocaleDateString(undefined, { weekday: "long", month: "long", day: "numeric" });
  const remote = $derived(!!conf.s?.navidrome.active);
  const SHELF = 18;
</script>

<PageHead title={greeting} eyebrow={date} italic>
  {#snippet actions()}
    {#if connect.target}
      <button class="source on" onclick={() => (connect.picker = true)}><span class="dot"></span>On {connect.target.name}</button>
    {:else}
      <button class="source" class:on={remote} disabled={ui.switching} onclick={() => setSource(!remote)}>
        <span class="dot"></span>{ui.switching ? "Connecting…" : remote ? "Navidrome" : "Local"}
      </button>
    {/if}
    <button class="hbtn" class:on={!!connect.target} aria-label="Play on…" onclick={() => (connect.picker = true)}><Icon name="devices" size={19} /></button>
    <button class="hbtn" aria-label="Toggle theme" onclick={() => setTheme(ui.theme === "dark" ? "light" : "dark")}><Icon name={ui.theme === "dark" ? "sun" : "moon"} size={18} /></button>
    <button class="hbtn" aria-label="Settings" onclick={() => (ui.settingsOpen = true)}><Icon name="gear" size={19} /></button>
  {/snippet}
</PageHead>

{#if ui.scanning}
  <div class="scanning"><span class="spin"></span>{remote ? "Syncing with Navidrome…" : "Scanning your music…"}</div>
{/if}

{#if !data}
  <div class="empty">Brewing your library…</div>
{:else if data.songs.length === 0}
  <div class="empty">
    <h2>Your shelves are empty</h2>
    <p>
      {ui.scanning
        ? "Scanning your music…"
        : "Connect your Navidrome server, or add music in the Files app under On My iPhone › Coff-fi › Music."}
    </p>
    <button class="pill-btn" onclick={() => ((ui.settingsTab = remote ? "library" : "navidrome"), (ui.settingsOpen = true))}>Open settings</button>
  </div>
{:else}
  {@const songs = data.songs}
  <section class="smart">
    {#each data.playlists as t (t.id)}
      <MTile tile={t} big />
    {/each}
  </section>

  {#if data.server_playlists.length}
    <section>
      <div class="sec-head">
        <h2>Playlists</h2>
        <span class="count">from Navidrome</span>
        <button class="all" onclick={() => selectTab("playlists")}>See all</button>
      </div>
      <div class="shelf">
        {#each data.server_playlists.slice(0, SHELF) as t (t.id)}<MTile tile={t} width={150} />{/each}
      </div>
    </section>
  {/if}

  {#if data.albums.length}
    <section>
      <div class="sec-head">
        <h2>Albums</h2>
        <span class="count">{data.albums.length}</span>
        <button class="all" onclick={() => push({ type: "list", which: "albums" })}>See all</button>
      </div>
      <div class="shelf">
        {#each data.albums.slice(0, SHELF) as t (t.id)}<MTile tile={t} width={150} />{/each}
      </div>
    </section>
  {/if}

  {#if data.artists.length}
    <section>
      <div class="sec-head">
        <h2>Artists</h2>
        <span class="count">{data.artists.length}</span>
        <button class="all" onclick={() => push({ type: "list", which: "artists" })}>See all</button>
      </div>
      <div class="shelf">
        {#each data.artists.slice(0, SHELF) as t (t.id)}<MTile tile={t} width={112} />{/each}
      </div>
    </section>
  {/if}

  <section>
    <div class="sec-head">
      <h2>Songs</h2>
      <span class="count">{data.songs.length}</span>
    </div>
    <VirtualList items={songs} rowHeight={62} {scroller}>
      {#snippet row(t, i)}
        <MSongRow track={t} onplay={() => playTracks(songs, i, "All songs")} />
      {/snippet}
    </VirtualList>
  </section>
{/if}

<style>
  .source {
    display: flex;
    align-items: center;
    gap: 6px;
    height: 32px;
    padding: 0 12px;
    border-radius: 16px;
    font-size: 13px;
    font-weight: 600;
    color: var(--text-2);
    background: var(--hover);
    margin-right: 2px;
  }
  .source .dot {
    width: 7px;
    height: 7px;
    border-radius: 50%;
    background: var(--text-3);
  }
  .hbtn.on {
    color: var(--accent);
  }
  .source.on {
    color: var(--text);
  }
  .source.on .dot {
    background: #5fbf7a;
    box-shadow: 0 0 0 3px color-mix(in srgb, #5fbf7a 25%, transparent);
  }
  .scanning {
    display: flex;
    align-items: center;
    gap: 9px;
    margin: -4px 18px 14px;
    font-size: 13px;
    color: var(--text-2);
  }
  .spin {
    width: 13px;
    height: 13px;
    border-radius: 50%;
    border: 2px solid var(--rule);
    border-top-color: var(--accent);
    animation: spin 0.8s linear infinite;
  }
  @keyframes spin {
    to {
      transform: rotate(360deg);
    }
  }
  section {
    margin-bottom: 26px;
  }
  .smart {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 16px 14px;
    padding: 0 18px;
  }
  .sec-head {
    display: flex;
    align-items: baseline;
    gap: 8px;
    padding: 0 18px 10px;
  }
  .sec-head h2 {
    font-size: 25px;
  }
  .count {
    font-size: 13px;
    color: var(--text-3);
  }
  .all {
    margin-left: auto;
    font-size: 15px;
    color: var(--accent-2);
  }
  :global([data-theme="light"]) .all {
    color: var(--accent);
  }
  .shelf {
    display: flex;
    gap: 14px;
    overflow-x: auto;
    padding: 0 18px 6px;
    scroll-snap-type: x mandatory;
    scroll-padding: 0 18px;
    scrollbar-width: none;
  }
  .shelf::-webkit-scrollbar {
    display: none;
  }
  .shelf > :global(*) {
    scroll-snap-align: start;
  }
  .empty {
    padding: 60px 28px;
    text-align: center;
    color: var(--text-2);
  }
  .empty h2 {
    color: var(--text);
    font-size: 26px;
    margin-bottom: 8px;
  }
  .empty .pill-btn {
    margin-top: 14px;
  }
</style>
