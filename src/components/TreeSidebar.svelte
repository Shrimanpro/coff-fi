<script lang="ts">
  import { SvelteMap, SvelteSet } from "svelte/reactivity";
  import { time } from "../lib/format";
  import { api } from "../lib/ipc";
  import { clicks, playTile, playTracks, ui } from "../lib/state.svelte";
  import type { FolderView, Tile, Track } from "../lib/types";
  import Icon from "./Icon.svelte";
  import VirtualList from "./VirtualList.svelte";

  type Row =
    | { kind: "folder"; tile: Tile; depth: number; expanded: boolean }
    | { kind: "track"; track: Track; depth: number; list: Track[]; idx: number; folder: string };

  let roots = $state<Tile[]>([]);
  const expanded = new SvelteSet<string>();
  const cache = new SvelteMap<string, FolderView>();
  let body = $state<HTMLDivElement | null>(null);

  async function load(path: string) {
    try {
      cache.set(path, await api.folder(path));
    } catch {}
  }

  $effect(() => {
    ui.libVersion;
    api.roots().then((r) => {
      roots = r;
      // After switching sources, forget folders that belong to the other library.
      const under = (p: string) => r.some((x) => p === x.id || p.startsWith(x.id));
      for (const p of [...expanded]) if (!under(p)) expanded.delete(p);
      for (const p of [...cache.keys()]) if (!under(p)) cache.delete(p);
      if (expanded.size === 0 && r[0]) expanded.add(r[0].id);
      for (const p of expanded) load(p);
    });
  });

  function toggle(path: string) {
    if (expanded.has(path)) expanded.delete(path);
    else {
      expanded.add(path);
      if (!cache.has(path)) load(path);
    }
  }

  function open(t: Tile) {
    toggle(t.id);
    ui.collection = null;
    ui.base = "directory";
    ui.dirPath = t.id;
  }

  const rows = $derived.by(() => {
    const out: Row[] = [];
    const walk = (tile: Tile, depth: number) => {
      const exp = expanded.has(tile.id);
      out.push({ kind: "folder", tile, depth, expanded: exp });
      if (!exp) return;
      const f = cache.get(tile.id);
      if (!f) return;
      for (const sub of f.folders) walk(sub, depth + 1);
      f.tracks.forEach((t, i) => out.push({ kind: "track", track: t, depth: depth + 1, list: f.tracks, idx: i, folder: f.name }));
    };
    roots.forEach((r) => walk(r, 0));
    return out;
  });
</script>

<div class="head">
  <div>
    <span class="eyebrow">Library</span>
    <h3>Folders</h3>
  </div>
  <button class="icon-btn" class:spin={ui.scanning} title="Rescan library" onclick={() => api.rescan()}><Icon name="refresh" size={16} /></button>
</div>

<div class="body" bind:this={body}>
  {#if roots.length === 0}
    <p class="hint">Add a music folder in Settings.</p>
  {/if}
  <VirtualList items={rows} rowHeight={32} scroller={body}>
    {#snippet row(r: Row)}
      {#if r.kind === "folder"}
        <div
          class="row folder"
          class:sel={ui.base === "directory" && ui.dirPath === r.tile.id}
          style:padding-left="{10 + r.depth * 16}px"
          role="treeitem"
          aria-selected={ui.dirPath === r.tile.id}
          aria-expanded={r.expanded}
          tabindex="-1"
          use:clicks={{ single: () => open(r.tile), double: () => playTile(r.tile) }}
        >
          <span class="chev" class:down={r.expanded}><Icon name="chev-right" size={13} /></span>
          <Icon name="folder" size={15} />
          <span class="name ellipsis">{r.tile.name}</span>
          <span class="meta">{r.tile.count}</span>
        </div>
      {:else}
        <div
          class="row track"
          style:padding-left="{27 + r.depth * 16}px"
          role="treeitem"
          aria-selected="false"
          tabindex="-1"
          ondblclick={() => playTracks(r.list, r.idx, r.folder)}
        >
          <Icon name="music" size={13} />
          <span class="name ellipsis">{r.track.title}</span>
          <span class="meta">{time(r.track.duration)}</span>
        </div>
      {/if}
    {/snippet}
  </VirtualList>
</div>

<style>
  .head {
    display: flex;
    align-items: flex-end;
    justify-content: space-between;
    padding: 18px 16px 12px 20px;
    border-bottom: 1px solid var(--rule);
  }
  h3 {
    font-size: 24px;
  }
  .body {
    flex: 1;
    overflow-y: auto;
    padding: 8px;
  }
  .row {
    height: 32px;
    display: flex;
    align-items: center;
    gap: 7px;
    padding-right: 10px;
    border-radius: 8px;
    color: var(--text-2);
    font-size: 13px;
  }
  .row:hover {
    background: var(--hover);
    color: var(--text);
  }
  .row.sel {
    background: var(--active);
    color: var(--text);
  }
  .folder {
    color: var(--text);
  }
  .chev {
    display: inline-grid;
    transition: transform 0.25s var(--ease);
    color: var(--text-3);
  }
  .chev.down {
    transform: rotate(90deg);
  }
  .name {
    flex: 1;
    min-width: 0;
  }
  .meta {
    color: var(--text-3);
    font-size: 11.5px;
    font-variant-numeric: tabular-nums;
  }
  .hint {
    color: var(--text-3);
    padding: 12px;
  }
  .spin :global(svg) {
    animation: spin 1s linear infinite;
  }
  @keyframes spin {
    to {
      transform: rotate(360deg);
    }
  }
</style>
