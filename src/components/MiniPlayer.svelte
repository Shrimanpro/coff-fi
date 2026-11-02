<script lang="ts">
  import { revealItemInDir } from "@tauri-apps/plugin-opener";
  import { quality } from "../lib/format";
  import { api, blurSrc, isRemote } from "../lib/ipc";
  import { remote } from "../lib/remote.svelte";
  import { anchors, enqueueTracks, openMenu, player, setVolume, toast, ui, volume } from "../lib/state.svelte";
  import Cover from "./Cover.svelte";
  import RemoteBadge from "./RemoteBadge.svelte";
  import Icon from "./Icon.svelte";
  import Scrubber from "./Scrubber.svelte";
  import Transport from "./Transport.svelte";

  const t = $derived(player.track);
  const show = $derived(!!t && player.status.track_id != null);
  let volOpen = $state(false);
  let art: HTMLDivElement;
  $effect(() => {
    anchors.miniArt = art;
  });

  async function fav() {
    if (t) player.fav = await api.toggleFavorite(t.id);
  }

  function more(e: MouseEvent) {
    if (!t) return;
    const tr = t;
    openMenu(e, [
      { label: "Go to album", action: () => ((ui.fullscreen = false), (ui.collection = { kind: "album", id: `${tr.album_artist}\u001f${tr.album}` })) },
      { label: "Go to artist", action: () => (ui.collection = { kind: "artist", id: tr.artist }) },
      { label: "Open folder", action: () => ((ui.collection = null), (ui.base = "directory"), (ui.dirPath = tr.dir)) },
      { label: "Play again next", action: () => enqueueTracks([tr], true) },
      ...(isRemote(tr) || remote.target
        ? []
        : [
            { label: "View / edit info", action: () => (ui.info = tr) },
            { label: "Show file", action: () => revealItemInDir(tr.path).catch((x) => toast(String(x))) },
          ]),
    ]);
  }

  const setVol = setVolume;
  const vol = $derived(volume());

  function wheel(e: WheelEvent) {
    e.preventDefault();
    setVol(Math.max(0, Math.min(1, vol - Math.sign(e.deltaY) * 0.05)));
  }
</script>

<div
  class="mini glass"
  class:show
  role="button"
  tabindex="0"
  aria-label="Open full player"
  onclick={(e) => {
    // Controls inside the bar handle their own clicks.
    if (!(e.target as Element).closest("button, input, [role=slider]")) ui.fullscreen = true;
  }}
  onkeydown={(e) => e.key === "Enter" && (ui.fullscreen = true)}
  onmouseleave={() => (volOpen = false)}
>
  {#if t?.cover}
    <img class="tint" src={blurSrc(t.cover)} alt="" />
  {/if}
  <div class="row">
    <div class="art" bind:this={art}><Cover covers={t?.cover ?? null} size={96} radius={9} /></div>
    <div class="meta">
      <div class="title ellipsis">{t?.title ?? ""}</div>
      <div class="artist ellipsis">{t?.artist ?? ""}{t?.album ? ` — ${t.album}` : ""}</div>
      {#if remote.target || remote.controllers.length}<div class="where"><RemoteBadge /></div>{/if}
    </div>
    <button class="icon-btn" class:on={player.fav} title={player.fav ? "Unfavorite" : "Favorite"} onclick={fav}>
      <Icon name={player.fav ? "star-fill" : "star"} />
    </button>
    <button class="icon-btn" title="More" onclick={more}><Icon name="more" /></button>

    <div class="center"><Transport /></div>

    <div class="right">
      <button class="icon-btn" class:on={!!remote.target} title="Play on…" onclick={() => (remote.picker = !remote.picker)}><Icon name="devices" /></button>
      <button class="icon-btn" title="Lyrics" onclick={() => (ui.fullscreen = true)}><Icon name="lyrics" /></button>
      <button class="icon-btn" class:on={ui.leftPinned} title="Queue" onclick={() => (ui.leftPinned = !ui.leftPinned)}><Icon name="queue" /></button>
      <div class="vol" class:open={volOpen} onwheel={wheel}>
        <input
          type="range"
          min="0"
          max="1"
          step="0.01"
          tabindex={volOpen ? 0 : -1}
          value={vol}
          oninput={(e) => setVol(+e.currentTarget.value)}
          style:--v="{vol * 100}%"
          aria-label="Volume"
        />
        <button class="icon-btn" title="Volume" onclick={() => (volOpen = !volOpen)}>
          <Icon name={vol === 0 ? "mute" : "volume"} />
        </button>
      </div>
    </div>
  </div>
  <div class="scrub-row">
    <Scrubber middle={`${remote.target ? remote.target.name : isRemote(t) ? "Navidrome" : "Local"} · ${quality(t)}`} />
  </div>
</div>

<style>
  .mini {
    position: fixed;
    left: 50%;
    bottom: 18px;
    z-index: 40;
    width: min(860px, calc(100vw - 140px));
    height: 68px;
    border-radius: 24px;
    padding: 0 12px;
    overflow: hidden;
    transform: translate(-50%, 140%);
    opacity: 0;
    pointer-events: none;
    transition:
      transform 0.55s var(--spring),
      opacity 0.35s var(--ease),
      height 0.4s var(--spring),
      bottom 0.4s var(--spring);
    cursor: pointer;
  }
  .mini.show {
    transform: translate(-50%, 0);
    opacity: 1;
    pointer-events: auto;
  }
  .mini.show:hover,
  .mini.show:focus-within {
    height: 104px;
    bottom: 22px;
  }
  .tint {
    position: absolute;
    inset: -20%;
    width: 140%;
    height: 140%;
    object-fit: cover;
    opacity: 0.32;
    filter: saturate(1.4);
    pointer-events: none;
    z-index: -1;
  }
  .row {
    display: flex;
    align-items: center;
    gap: 6px;
    height: 68px;
  }
  .art {
    width: 48px;
    flex: none;
    border-radius: 9px;
    box-shadow: 0 4px 12px rgba(0, 0, 0, 0.3);
    margin-right: 6px;
  }
  .meta {
    min-width: 0;
    flex: 1 1 0;
  }
  .title {
    font-weight: 650;
    font-size: 14.5px;
  }
  .artist {
    font-size: 12.5px;
    color: var(--text-2);
  }
  .where {
    display: flex;
    margin-top: 2px;
  }
  .center {
    flex: none;
    margin: 0 10px;
  }
  .right {
    flex: 1 1 0;
    display: flex;
    justify-content: flex-end;
    gap: 2px;
  }
  .icon-btn.on {
    color: var(--accent);
  }
  .vol {
    display: flex;
    align-items: center;
  }
  .vol input {
    width: 0;
    opacity: 0;
    margin: 0;
    appearance: none;
    height: 5px;
    border-radius: 5px;
    background: linear-gradient(90deg, var(--accent) var(--v), color-mix(in srgb, var(--text) 18%, transparent) var(--v));
    transition: width 0.35s var(--spring), opacity 0.25s var(--ease), margin 0.35s var(--spring);
  }
  .vol.open input {
    width: 96px;
    opacity: 1;
    margin: 0 6px 0 4px;
  }
  .vol input::-webkit-slider-thumb {
    appearance: none;
    width: 13px;
    height: 13px;
    border-radius: 50%;
    background: var(--text);
    box-shadow: 0 1px 4px rgba(0, 0, 0, 0.4);
  }
  .scrub-row {
    padding: 0 8px;
    opacity: 0;
    transform: translateY(6px);
    transition: opacity 0.25s var(--ease), transform 0.35s var(--ease);
  }
  .mini:hover .scrub-row,
  .mini:focus-within .scrub-row {
    opacity: 1;
    transform: none;
  }
</style>
