<script lang="ts">
  // Folder browser (touch version of views/Directory).
  import Cover from "../../components/Cover.svelte";
  import Icon from "../../components/Icon.svelte";
  import VirtualList from "../../components/VirtualList.svelte";
  import { api } from "../../lib/ipc";
  import { playTracks, shufflePlay, toast, ui } from "../../lib/state.svelte";
  import type { FolderView, Track } from "../../lib/types";
  import MSongRow from "../MSongRow.svelte";
  import { longpress, openTile, tileMenu } from "../nav.svelte";
  import PageHead from "../PageHead.svelte";

  let { path, scroller }: { path: string; scroller: HTMLElement | null } = $props();
  let data = $state<FolderView | null>(null);

  $effect(() => {
    ui.libVersion;
    api.folder(path).then((f) => (data = f)).catch((e) => toast(String(e)));
  });

  async function all(): Promise<Track[]> {
    return (await api.collection("folder", path)).tracks;
  }
  const summary = $derived.by(() => {
    if (!data) return "";
    const n = data.tracks.length + data.folders.reduce((s, f) => s + f.count, 0);
    const parts = [];
    if (data.folders.length) parts.push(`${data.folders.length} folder${data.folders.length === 1 ? "" : "s"}`);
    parts.push(`${n} song${n === 1 ? "" : "s"}`);
    return parts.join(" · ");
  });
</script>

<PageHead title="" />

{#if data}
  {@const d = data}
  <header>
    <div class="art"><Cover covers={d.covers} size={640} radius={12} icon="folder" /></div>
    <span class="eyebrow">Folder</span>
    <h1>{d.name}</h1>
    <p class="sub">{summary}</p>
    <div class="actions">
      <button class="pill-btn" onclick={async () => playTracks(await all(), 0, d.name)}><Icon name="play" size={15} /> Play</button>
      <button class="pill-btn ghost" onclick={async () => shufflePlay(await all(), d.name)}><Icon name="shuffle" size={15} /> Shuffle</button>
    </div>
  </header>

  {#each d.folders as f (f.id)}
    <div class="frow" role="button" tabindex="0" onclick={() => openTile(f)} onkeydown={(e) => e.key === "Enter" && openTile(f)} data-lp-scale use:longpress={(el) => tileMenu(f, el)}>
      <div class="fart"><Cover covers={f.covers} size={96} radius={8} icon="folder" /></div>
      <div class="fname ellipsis">{f.name}</div>
      <span class="fcount">{f.count}</span>
      <Icon name="chev-right" size={16} />
    </div>
  {/each}

  {#if d.tracks.length}
    <VirtualList items={d.tracks} rowHeight={62} {scroller}>
      {#snippet row(t, i)}
        <MSongRow track={t} onplay={() => playTracks(d.tracks, i, d.name)} />
      {/snippet}
    </VirtualList>
  {/if}
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
    width: min(56vw, 240px);
    border-radius: 12px;
    overflow: hidden;
    box-shadow: 0 18px 44px rgba(0, 0, 0, 0.3);
    margin-bottom: 16px;
  }
  h1 {
    font-size: 28px;
    line-height: 1.1;
    margin-top: 4px;
    word-break: break-word;
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
  .frow {
    height: 64px;
    display: flex;
    align-items: center;
    gap: 14px;
    padding: 0 18px;
    color: var(--text-3);
    -webkit-tap-highlight-color: transparent;
  }
  .frow:active {
    background: var(--hover);
  }
  .fart {
    width: 46px;
    flex: none;
    border-radius: 8px;
    overflow: hidden;
  }
  .fname {
    flex: 1;
    font-size: 16px;
    color: var(--text);
  }
  .fcount {
    font-size: 13px;
  }
</style>
