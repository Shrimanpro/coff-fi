<script lang="ts">
  import Cover from "../components/Cover.svelte";
  import Icon from "../components/Icon.svelte";
  import SongRow from "../components/SongRow.svelte";
  import TileC from "../components/Tile.svelte";
  import VirtualList from "../components/VirtualList.svelte";
  import { api } from "../lib/ipc";
  import { conf, playTracks, shufflePlay, toast, ui } from "../lib/state.svelte";
  import type { FolderView, Tile } from "../lib/types";

  let { scroller }: { scroller: HTMLElement | null } = $props();

  let data = $state<FolderView | null>(null);
  let roots = $state<Tile[]>([]);

  $effect(() => {
    ui.libVersion;
    const p = ui.dirPath;
    if (p) {
      api.folder(p).then((f) => (data = f)).catch((e) => toast(String(e)));
    } else {
      data = null;
      api.roots().then((r) => {
        roots = r;
        if (r.length === 1 && !ui.dirPath) ui.dirPath = r[0].id;
      });
    }
  });

  /** Breadcrumbs from the library root that contains the current folder. */
  const crumbs = $derived.by(() => {
    const p = data?.path;
    if (!p || !conf.s) return [];
    const sep = p.includes("\\") && !p.includes("/") ? "\\" : "/";
    const roots = conf.s.navidrome.active ? ["Navidrome"] : conf.s.roots;
    const root = roots.map((r) => r.replace(/[\\/]+$/, "")).find((r) => p === r || p.startsWith(r + sep));
    if (!root) return [{ name: data!.name, path: p }];
    const out = [{ name: root.split(sep).pop() || root, path: root }];
    let cur = root;
    for (const part of p.slice(root.length).split(sep).filter(Boolean)) {
      cur = cur + sep + part;
      out.push({ name: part, path: cur });
    }
    return out;
  });

  const summary = $derived.by(() => {
    if (!data) return "";
    const n = data.tracks.length + data.folders.reduce((s, f) => s + f.count, 0);
    const parts = [];
    if (data.folders.length) parts.push(`${data.folders.length} ${data.folders.length === 1 ? "folder" : "folders"}`);
    parts.push(`${n} ${n === 1 ? "song" : "songs"}`);
    return parts.join(" · ");
  });

  async function all() {
    return data ? (await api.collection("folder", data.path)).tracks : [];
  }
</script>

<div class="dir">
  {#if !ui.dirPath}
    <h1>Library folders</h1>
    {#if roots.length === 0}
      <p class="hint">No music folder yet. Add one in Settings.</p>
    {/if}
    <div class="grid">
      {#each roots as t (t.id)}
        <TileC tile={t} size={640} />
      {/each}
    </div>
  {:else if data}
    <nav class="crumbs">
      {#each crumbs as c, i (c.path)}
        {#if i > 0}<Icon name="chev-right" size={14} />{/if}
        <button class:last={i === crumbs.length - 1} onclick={() => (ui.dirPath = c.path)}>{c.name}</button>
      {/each}
    </nav>
    <header>
      <div class="art"><Cover covers={data.covers} size={640} radius={14} icon="folder" /></div>
      <div class="info">
        <span class="eyebrow">Folder</span>
        <h1>{data.name}</h1>
        <p class="sub">{summary}</p>
        <div class="actions">
          <button class="pill-btn" onclick={async () => playTracks(await all(), 0, data!.name)}><Icon name="play" size={16} /> Play</button>
          <button class="pill-btn ghost" onclick={async () => shufflePlay(await all(), data!.name)}><Icon name="shuffle" size={16} /> Shuffle</button>
        </div>
      </div>
    </header>

    {#if data.folders.length}
      <div class="grid">
        {#each data.folders as t (t.id)}
          <TileC tile={t} size={640} />
        {/each}
      </div>
    {/if}

    {#if data.tracks.length}
      <div class="sec-head">
        <h2>Songs</h2>
        <span class="count">{data.tracks.length}</span>
      </div>
      <VirtualList items={data.tracks} rowHeight={56} {scroller}>
        {#snippet row(t, i)}
          <SongRow track={t} index={i} onplay={() => data && playTracks(data.tracks, i, data.name)} />
        {/snippet}
      </VirtualList>
    {/if}

    {#if !data.folders.length && !data.tracks.length}
      <p class="hint">Nothing playable in this folder.</p>
    {/if}
  {/if}
</div>

<style>
  h1 {
    font-size: 44px;
    font-weight: 600;
    line-height: 1.1;
  }
  header h1 {
    font-size: 52px;
    line-height: 1.02;
    margin: 6px 0 8px;
    overflow-wrap: anywhere;
  }
  .crumbs {
    display: flex;
    align-items: center;
    flex-wrap: wrap;
    gap: 4px;
    color: var(--text-3);
    margin-bottom: 8px;
  }
  .crumbs button {
    color: var(--text-3);
    padding: 2px 4px;
    border-radius: 6px;
  }
  .crumbs button:hover {
    color: var(--text);
    background: var(--hover);
  }
  .crumbs button.last {
    color: var(--text-2);
  }
  header {
    display: flex;
    gap: 32px;
    align-items: flex-end;
    margin-bottom: 36px;
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
  .sub {
    color: var(--text-2);
    margin: 0 0 18px;
  }
  .actions {
    display: flex;
    gap: 10px;
  }
  .grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(230px, 1fr));
    gap: 32px 28px;
    margin-bottom: 44px;
  }
  .sec-head {
    display: flex;
    align-items: baseline;
    gap: 14px;
    margin-bottom: 12px;
    padding-bottom: 8px;
    border-bottom: 1px solid var(--rule);
  }
  h2 {
    font-size: 28px;
  }
  .count,
  .hint {
    color: var(--text-3);
  }
</style>
