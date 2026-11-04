<script lang="ts">
  // Album / artist / playlist page (touch version of views/CollectionView).
  import Cover from "../../components/Cover.svelte";
  import Icon from "../../components/Icon.svelte";
  import VirtualList from "../../components/VirtualList.svelte";
  import { api } from "../../lib/ipc";
  import { enqueueTracks, playTracks, shufflePlay, toast, ui } from "../../lib/state.svelte";
  import type { Collection, TileKind } from "../../lib/types";
  import MSongRow from "../MSongRow.svelte";
  import PageHead from "../PageHead.svelte";

  let { kind, id, focus, scroller }: { kind: TileKind; id: string; focus?: number; scroller: HTMLElement | null } = $props();
  let data = $state<Collection | null>(null);

  $effect(() => {
    ui.libVersion;
    ui.statsVersion;
    api.collection(kind, id).then((c) => (data = c)).catch((e) => toast(String(e)));
  });

  // "Go to album" from a song: bring it into view and highlight it briefly.
  let listEl = $state<HTMLDivElement>();
  let flashId = $state<number | null>(null);
  let focused = false;
  $effect(() => {
    if (focused || focus == null || !data || !listEl || !scroller) return;
    focused = true;
    const i = data.tracks.findIndex((t) => t.id === focus);
    if (i < 0) return;
    const top = listEl.getBoundingClientRect().top - scroller.getBoundingClientRect().top + scroller.scrollTop + i * 62;
    scroller.scrollTo({ top: Math.max(0, top - scroller.clientHeight / 2 + 31), behavior: "smooth" });
    flashId = focus;
    setTimeout(() => (flashId = null), 2400);
  });

  const total = $derived(data ? data.tracks.reduce((s, t) => s + t.duration, 0) : 0);
  const label = $derived({ playlist: "Playlist", album: "Album", artist: "Artist", folder: "Folder" }[kind] ?? "");
  const isAlbum = $derived(kind === "album");
</script>

<PageHead title="" />

{#if data}
  {@const d = data}
  <header>
    <div class="art" class:round={kind === "artist"}><Cover covers={d.covers} size={640} radius={kind === "artist" ? 0 : 12} round={kind === "artist"} /></div>
    <span class="eyebrow">{label}</span>
    <h1>{d.title}</h1>
    <p class="sub">{d.subtitle} · {Math.round(total / 60)} min</p>
    <div class="actions">
      <button class="pill-btn" disabled={!d.tracks.length} onclick={() => playTracks(d.tracks, 0, d.title)}><Icon name="play" size={15} /> Play</button>
      <button class="pill-btn ghost" disabled={!d.tracks.length} onclick={() => shufflePlay(d.tracks, d.title)}><Icon name="shuffle" size={15} /> Shuffle</button>
      <button class="sq ghost" aria-label="Add to queue" disabled={!d.tracks.length} onclick={() => enqueueTracks(d.tracks, false)}><Icon name="to-queue" size={19} /></button>
    </div>
  </header>
  <div bind:this={listEl}>
    <VirtualList items={d.tracks} rowHeight={62} {scroller}>
      {#snippet row(t, i)}
        <MSongRow track={t} art={!isAlbum} number={t.track_no ?? i + 1} focus={flashId === t.id} onplay={() => playTracks(d.tracks, i, d.title)} />
      {/snippet}
    </VirtualList>
  </div>
{/if}

<style>
  header {
    display: flex;
    flex-direction: column;
    align-items: center;
    text-align: center;
    padding: 0 22px 18px;
  }
  .art {
    width: min(68vw, 300px);
    border-radius: 12px;
    overflow: hidden;
    box-shadow: 0 18px 44px rgba(0, 0, 0, 0.35);
    margin-bottom: 18px;
  }
  .art.round {
    border-radius: 50%;
  }
  h1 {
    font-size: 30px;
    line-height: 1.1;
    margin-top: 4px;
  }
  .sub {
    margin: 6px 0 16px;
    color: var(--text-2);
    font-size: 14px;
  }
  .actions {
    display: flex;
    gap: 10px;
    width: 100%;
  }
  .actions .pill-btn {
    flex: 1;
    justify-content: center;
    height: 46px;
    border-radius: 12px;
  }
  .sq {
    width: 46px;
    height: 46px;
    flex: none;
    border-radius: 12px;
    display: grid;
    place-items: center;
    background: var(--hover);
  }
</style>
