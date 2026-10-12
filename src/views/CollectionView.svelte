<script lang="ts">
  import Cover from "../components/Cover.svelte";
  import Icon from "../components/Icon.svelte";
  import SongRow from "../components/SongRow.svelte";
  import VirtualList from "../components/VirtualList.svelte";
  import { api } from "../lib/ipc";
  import { enqueueTracks, playTracks, shufflePlay, toast, ui } from "../lib/state.svelte";
  import type { Collection } from "../lib/types";

  let { kind, id, scroller }: { kind: string; id: string; scroller: HTMLElement | null } = $props();

  let data = $state<Collection | null>(null);
  $effect(() => {
    ui.libVersion;
    api.collection(kind, id).then((c) => (data = c)).catch((e) => toast(String(e)));
  });

  // "Go to track": scroll the requested song into the middle of the view and highlight it.
  let listEl = $state<HTMLDivElement>();
  let flashId = $state<number | null>(null);
  $effect(() => {
    const want = ui.focusTrack;
    if (want == null || !data || !listEl || !scroller) return;
    const i = data.tracks.findIndex((t) => t.id === want);
    ui.focusTrack = null;
    if (i < 0) return;
    const top = listEl.getBoundingClientRect().top - scroller.getBoundingClientRect().top + scroller.scrollTop + i * 56;
    scroller.scrollTo({ top: Math.max(0, top - scroller.clientHeight / 2 + 28), behavior: "smooth" });
    flashId = want;
    setTimeout(() => (flashId = null), 2400);
  });

  const total = $derived(data ? data.tracks.reduce((s, t) => s + t.duration, 0) : 0);
  const isAlbum = $derived(kind === "album");
  const label = $derived({ playlist: "Playlist", album: "Album", artist: "Artist", folder: "Folder" }[kind] ?? "");
</script>

<div class="col">
  <button class="back" onclick={() => (ui.collection = null)}><Icon name="chev-left" size={16} /> Back</button>
  {#if data}
    <header>
      <div class="art"><Cover covers={data.covers} size={640} radius={14} round={kind === "artist"} /></div>
      <div class="info">
        <span class="eyebrow">{label}</span>
        <h1>{data.title}</h1>
        <p class="sub">{data.subtitle} · {Math.round(total / 60)} min</p>
        <div class="actions">
          <button class="pill-btn" disabled={!data.tracks.length} onclick={() => data && playTracks(data.tracks, 0, data.title)}><Icon name="play" size={16} /> Play</button>
          <button class="pill-btn ghost" onclick={() => data && shufflePlay(data.tracks, data.title)}><Icon name="shuffle" size={16} /> Shuffle</button>
          <button class="icon-btn" title="Add to queue" onclick={() => data && enqueueTracks(data.tracks, false)}><Icon name="plus" /></button>
        </div>
      </div>
    </header>
    {#if data.tracks.length}
      <div bind:this={listEl}></div>
      <VirtualList items={data.tracks} rowHeight={56} {scroller}>
        {#snippet row(t, i)}
          <SongRow track={t} index={i} showArt={!isAlbum} showAlbum={!isAlbum} flash={t.id === flashId} onplay={() => data && playTracks(data.tracks, i, data.title)} />
        {/snippet}
      </VirtualList>
    {:else}
      <p class="hint">{kind === "playlist" && !id.startsWith("nd:") ? "Nothing here yet — keep listening and this fills itself." : "No songs."}</p>
    {/if}
  {/if}
</div>

<style>
  .back {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    color: var(--text-2);
    margin-bottom: 18px;
    padding: 4px 10px 4px 6px;
    border-radius: 999px;
  }
  .back:hover {
    background: var(--hover);
    color: var(--text);
  }
  header {
    display: flex;
    gap: 32px;
    align-items: flex-end;
    margin-bottom: 30px;
  }
  .art {
    width: 230px;
    flex: none;
    border-radius: 14px;
    box-shadow: var(--tile-shadow);
  }
  .info {
    min-width: 0;
  }
  h1 {
    font-size: 52px;
    line-height: 1.02;
    margin: 6px 0 8px;
    overflow-wrap: anywhere;
  }
  .sub {
    color: var(--text-2);
    margin: 0 0 18px;
  }
  .actions {
    display: flex;
    gap: 10px;
    align-items: center;
  }
  .hint {
    color: var(--text-3);
  }
</style>
