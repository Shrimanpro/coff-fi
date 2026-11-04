<script lang="ts">
  // Library › Artists / Albums / Songs / Folders.
  import Cover from "../../components/Cover.svelte";
  import Icon from "../../components/Icon.svelte";
  import VirtualList from "../../components/VirtualList.svelte";
  import { api } from "../../lib/ipc";
  import { playTracks, shufflePlay, toast, ui } from "../../lib/state.svelte";
  import type { Home, Tile } from "../../lib/types";
  import MSongRow from "../MSongRow.svelte";
  import MTile from "../MTile.svelte";
  import { longpress, openTile, tileMenu } from "../nav.svelte";
  import PageHead from "../PageHead.svelte";

  let { which, scroller }: { which: "albums" | "artists" | "songs" | "folders"; scroller: HTMLElement | null } = $props();
  let home = $state<Home | null>(null);
  let roots = $state<Tile[]>([]);

  $effect(() => {
    ui.libVersion;
    if (which === "folders") api.roots().then((r) => (roots = r)).catch((e) => toast(String(e)));
    else api.home().then((h) => (home = h)).catch((e) => toast(String(e)));
  });

  const TITLES = { albums: "Albums", artists: "Artists", songs: "Songs", folders: "Folders" };
</script>

<PageHead title={TITLES[which]} />

{#if which === "albums" && home}
  <div class="grid">
    {#each home.albums as t (t.id)}<MTile tile={t} />{/each}
  </div>
{:else if which === "artists" && home}
  {@const artists = home.artists}
  <VirtualList items={artists} rowHeight={64} {scroller}>
    {#snippet row(t)}
      <div class="arow" role="button" tabindex="0" onclick={() => openTile(t)} onkeydown={(e) => e.key === "Enter" && openTile(t)} data-lp-scale use:longpress={(el) => tileMenu(t, el)}>
        <div class="aart"><Cover covers={t.covers} size={96} round icon="artist" /></div>
        <div class="aname ellipsis">{t.name}</div>
        <span class="acount">{t.count}</span>
        <Icon name="chev-right" size={16} />
      </div>
    {/snippet}
  </VirtualList>
{:else if which === "songs" && home}
  {@const songs = home.songs}
  <div class="actions">
    <button class="pill-btn" onclick={() => playTracks(songs, 0, "All songs")}><Icon name="play" size={15} /> Play</button>
    <button class="pill-btn ghost" onclick={() => shufflePlay(songs, "All songs")}><Icon name="shuffle" size={15} /> Shuffle</button>
  </div>
  <VirtualList items={songs} rowHeight={62} {scroller}>
    {#snippet row(t, i)}
      <MSongRow track={t} onplay={() => playTracks(songs, i, "All songs")} />
    {/snippet}
  </VirtualList>
{:else if which === "folders"}
  {#each roots as t (t.id)}
    <div class="arow" role="button" tabindex="0" onclick={() => openTile(t)} onkeydown={(e) => e.key === "Enter" && openTile(t)} data-lp-scale use:longpress={(el) => tileMenu(t, el)}>
      <div class="aart sq"><Cover covers={t.covers} size={96} radius={8} icon="folder" /></div>
      <div class="aname ellipsis">{t.name}</div>
      <span class="acount">{t.count}</span>
      <Icon name="chev-right" size={16} />
    </div>
  {:else}
    <p class="none">No music folders yet. Add one in Settings › Library.</p>
  {/each}
{/if}

<style>
  .grid {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 18px 14px;
    padding: 0 18px;
  }
  .actions {
    display: flex;
    gap: 10px;
    padding: 0 18px 12px;
  }
  .actions .pill-btn {
    flex: 1;
    justify-content: center;
    height: 44px;
    border-radius: 12px;
  }
  .arow {
    height: 64px;
    display: flex;
    align-items: center;
    gap: 14px;
    padding: 0 18px;
    color: var(--text-3);
    -webkit-tap-highlight-color: transparent;
  }
  .arow:active {
    background: var(--hover);
  }
  .aart {
    width: 48px;
    flex: none;
  }
  .aart.sq {
    border-radius: 8px;
    overflow: hidden;
  }
  .aname {
    flex: 1;
    font-size: 16px;
    color: var(--text);
  }
  .acount {
    font-size: 13px;
  }
  .none {
    padding: 20px 18px;
    color: var(--text-2);
  }
</style>
