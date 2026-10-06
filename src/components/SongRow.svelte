<script lang="ts">
  import { time } from "../lib/format";
  import type { Track } from "../lib/types";
  import Cover from "./Cover.svelte";

  let {
    track,
    index,
    showArt = true,
    showAlbum = true,
  }: { track: Track; index: number; showArt?: boolean; showAlbum?: boolean } = $props();

</script>

<div
  class="row"
  role="button"
  tabindex="-1"
>
  <div class="lead">
    {#if showArt}
      <div class="art"><Cover covers={track.cover} size={96} radius={6} /></div>
    {:else}
      <span class="num">{track.track_no ?? index + 1}</span>
    {/if}
  </div>
  <div class="main">
    <div class="title ellipsis">{track.title}</div>
    <div class="artist ellipsis">{track.artist}</div>
  </div>
  {#if showAlbum}
    <div class="album ellipsis">{track.album}</div>
  {/if}
  <div class="fmt">{track.codec}</div>
  <div class="dur">{time(track.duration)}</div>
</div>

<style>
  /* "Go to track": a soft pulse so the eye finds the row. */
  .row.flash {
    animation: flash 2.2s var(--ease);
  }
  @keyframes flash {
    0%,
    40% {
      background: color-mix(in srgb, var(--accent) 22%, transparent);
    }
  }
  .row {
    display: grid;
    grid-template-columns: 44px minmax(0, 2fr) minmax(0, 1.4fr) 54px 48px;
    align-items: center;
    gap: 14px;
    height: 56px;
    padding: 0 12px;
    border-radius: 10px;
    outline: none;
    transition: background-color 0.15s;
  }
  .row:not(:has(.album)) {
    grid-template-columns: 44px minmax(0, 1fr) 54px 48px;
  }
  .row:hover {
    background: var(--hover);
  }
  .row.current .title {
    color: var(--accent);
  }
  .lead {
    position: relative;
    width: 40px;
    height: 40px;
    display: grid;
    place-items: center;
  }
  .art {
    width: 40px;
  }
  .num {
    color: var(--text-3);
    font-variant-numeric: tabular-nums;
  }
  .hover-play,
  .eq {
    position: absolute;
    inset: 0;
    display: grid;
    place-items: center;
    border-radius: 6px;
    background: color-mix(in srgb, var(--bg) 55%, transparent);
    color: var(--text);
  }
  .hover-play {
    opacity: 0;
    transition: opacity 0.15s;
  }
  .row:hover .hover-play {
    opacity: 1;
  }
  .eq {
    display: flex;
    gap: 2px;
    align-items: flex-end;
    justify-content: center;
    padding-bottom: 12px;
    color: var(--accent);
  }
  .eq i {
    width: 3px;
    height: 14px;
    background: currentColor;
    border-radius: 2px;
    transform-origin: bottom;
    animation: bounce 0.9s ease-in-out infinite;
  }
  .eq i:nth-child(2) {
    animation-delay: -0.3s;
  }
  .eq i:nth-child(3) {
    animation-delay: -0.6s;
  }
  .eq.paused i {
    animation-play-state: paused;
  }
  @keyframes bounce {
    0%,
    100% {
      transform: scaleY(0.3);
    }
    50% {
      transform: scaleY(1);
    }
  }
  .title {
    font-weight: 550;
  }
  .artist,
  .album {
    color: var(--text-2);
    font-size: 13px;
  }
  .fmt {
    font-size: 10.5px;
    letter-spacing: 0.06em;
    color: var(--text-3);
    border: 1px solid var(--rule);
    border-radius: 5px;
    padding: 1px 5px;
    justify-self: start;
  }
  .dur {
    color: var(--text-3);
    font-variant-numeric: tabular-nums;
    text-align: right;
  }
</style>
