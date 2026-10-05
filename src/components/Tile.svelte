<script lang="ts">
  import { openTile } from "../lib/state.svelte";
  import type { Tile } from "../lib/types";

  let { tile, size = 256 }: { tile: Tile; size?: number } = $props();
  const round = $derived(tile.kind === "artist");
</script>

<div
  class="tile"
  class:round
  role="button"
  tabindex="0"
  onclick={() => openTile(tile)}
  onkeydown={(e) => e.key === "Enter" && openTile(tile)}
>
  <div class="art">
    <div style:aspect-ratio="1" style:border-radius={round ? "50%" : "14px"} style:background="var(--placeholder)"></div>
  </div>
  <div class="name ellipsis" title={tile.name}>{tile.name}</div>
  <div class="sub ellipsis">{tile.subtitle}</div>
</div>

<style>
  .tile {
    min-width: 0;
    outline: none;
  }
  .art {
    position: relative;
    border-radius: 14px;
    box-shadow: var(--tile-shadow);
    transition: transform 0.35s var(--spring), box-shadow 0.35s var(--ease);
  }
  .round .art {
    border-radius: 50%;
  }
  .tile:hover .art,
  .tile:focus-visible .art {
    transform: translateY(-4px) scale(1.015);
  }
  .play {
    position: absolute;
    right: 10px;
    bottom: 10px;
    width: 44px;
    height: 44px;
    border-radius: 50%;
    display: grid;
    place-items: center;
    background: var(--accent);
    color: var(--accent-ink);
    box-shadow: 0 6px 18px rgba(0, 0, 0, 0.35);
    opacity: 0;
    transform: translateY(8px) scale(0.9);
    transition: opacity 0.25s var(--ease), transform 0.3s var(--spring);
  }
  .round .play {
    right: 14%;
    bottom: 14%;
  }
  .tile:hover .play {
    opacity: 1;
    transform: none;
  }
  .play :global(svg) {
    margin-left: 2px;
  }
  .name {
    margin-top: 10px;
    font-weight: 600;
    font-size: 14.5px;
  }
  .round .name,
  .round .sub {
    text-align: center;
  }
  .sub {
    color: var(--text-3);
    font-size: 12.5px;
    margin-top: 2px;
  }
</style>
