<script lang="ts">
  import { quality } from "../lib/format";
  import { blurSrc } from "../lib/ipc";
  import { player, setVolume, ui, volume } from "../lib/state.svelte";
  import Cover from "./Cover.svelte";
  import Icon from "./Icon.svelte";
  import Scrubber from "./Scrubber.svelte";
  import Transport from "./Transport.svelte";

  const t = $derived(player.track);
  const show = $derived(!!t && player.status.track_id != null);
  let volOpen = $state(false);

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
  onmouseleave={() => (volOpen = false)}
>
  {#if t?.cover}
    <img class="tint" src={blurSrc(t.cover)} alt="" />
  {/if}
  <div class="row">
    <div class="art"><Cover covers={t?.cover ?? null} size={96} radius={9} /></div>
    <div class="meta">
      <div class="title ellipsis">{t?.title ?? ""}</div>
      <div class="artist ellipsis">{t?.artist ?? ""}{t?.album ? ` — ${t.album}` : ""}</div>
    </div>

    <div class="center"><Transport /></div>

    <div class="right">
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
    <Scrubber middle={quality(t)} />
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
