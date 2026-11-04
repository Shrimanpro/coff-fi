<script lang="ts">
  // Album / playlist / artist / folder tile: tap opens, long-press shows Play / Play next / Add to queue.
  import Cover from "../components/Cover.svelte";
  import Icon from "../components/Icon.svelte";
  import { playTile } from "../lib/state.svelte";
  import type { Tile } from "../lib/types";
  import { longpress, openTile, tileMenu } from "./nav.svelte";

  let { tile, width = null, big = false }: { tile: Tile; width?: number | null; big?: boolean } = $props();
  const round = $derived(tile.kind === "artist");
</script>

<div class="tile" class:big style:width={width ? `${width}px` : null}>
  <div
    class="art"
    class:round
    role="button"
    tabindex="0"
    onclick={() => openTile(tile)}
    onkeydown={(e) => e.key === "Enter" && openTile(tile)}
    data-lp-scale
    use:longpress={(el) => tileMenu(tile, el)}
  >
    <Cover covers={tile.covers} size={big ? 640 : 256} radius={round ? 0 : 10} {round} icon={tile.kind === "artist" ? "artist" : tile.kind === "folder" ? "folder" : "music"} />
    {#if big}
      <button class="play" aria-label="Play {tile.name}" onclick={(e) => (e.stopPropagation(), playTile(tile))}><Icon name="play" size={16} /></button>
    {/if}
  </div>
  <div class="name ellipsis" class:center={round}>{tile.name}</div>
  {#if tile.subtitle && !round}<div class="sub ellipsis">{tile.subtitle}</div>{/if}
</div>

<style>
  .tile {
    min-width: 0;
    flex: none;
  }
  .art {
    position: relative;
    border-radius: 10px;
    overflow: hidden;
    box-shadow: var(--tile-shadow);
    -webkit-tap-highlight-color: transparent;
    transition: transform 0.25s var(--spring);
  }
  .art.round {
    border-radius: 50%;
  }
  .play {
    position: absolute;
    right: 8px;
    bottom: 8px;
    width: 34px;
    height: 34px;
    border-radius: 50%;
    display: grid;
    place-items: center;
    padding-left: 2px;
    color: #fff;
    background: rgba(20, 14, 10, 0.45);
    backdrop-filter: blur(14px) saturate(160%);
    -webkit-backdrop-filter: blur(14px) saturate(160%);
    border: 0.5px solid rgba(255, 255, 255, 0.18);
  }
  .name {
    margin-top: 7px;
    font-size: 14px;
    font-weight: 500;
  }
  .big .name {
    font-family: var(--font-serif);
    font-size: 19px;
    font-weight: 600;
    margin-top: 8px;
  }
  .center {
    text-align: center;
  }
  .sub {
    font-size: 12.5px;
    color: var(--text-2);
    margin-top: 1px;
  }
</style>
