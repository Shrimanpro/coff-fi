<script lang="ts">
  // A song row for touch: tap plays, long-press or "…" opens the action sheet,
  // swipe right → Play Next, swipe left → Add to Queue.
  import Cover from "../components/Cover.svelte";
  import Icon from "../components/Icon.svelte";
  import { time } from "../lib/format";
  import { enqueueTracks, player } from "../lib/state.svelte";
  import type { Track } from "../lib/types";
  import { haptic } from "../lib/haptics";
  import { longpress, trackMenu, trackSheet } from "./nav.svelte";

  let {
    track,
    onplay,
    number = null,
    art = true,
    focus = false,
  }: { track: Track; onplay: () => void; number?: number | null; art?: boolean; focus?: boolean } = $props();

  const current = $derived(player.status.track_id === track.id);
  const sub = $derived([track.artist, track.album].filter(Boolean).join(" · "));

  const TRIGGER = 72;
  let dx = $state(0);
  let swiping = $state(false);
  let settling = $state(false);
  let sx = 0;
  let sy = 0;
  let decided: "x" | "y" | null = null;
  let pid: number | null = null;

  function down(e: PointerEvent) {
    if (e.pointerType === "mouse" && e.button !== 0) return;
    sx = e.clientX;
    sy = e.clientY;
    decided = null;
    pid = e.pointerId;
  }
  function move(e: PointerEvent) {
    if (pid !== e.pointerId) return;
    const x = e.clientX - sx;
    const y = e.clientY - sy;
    if (!decided && (Math.abs(x) > 10 || Math.abs(y) > 10)) {
      decided = Math.abs(x) > Math.abs(y) * 1.2 ? "x" : "y";
      if (decided === "x") {
        swiping = true;
        (e.currentTarget as HTMLElement).setPointerCapture(e.pointerId);
      }
    }
    if (decided === "x") {
      // Rubber-band past the trigger point.
      const lim = (v: number) => (Math.abs(v) < TRIGGER ? v : Math.sign(v) * (TRIGGER + (Math.abs(v) - TRIGGER) * 0.35));
      const next = lim(x);
      // Tick once when the swipe crosses into (or out of) "will trigger".
      if ((Math.abs(next) >= TRIGGER) !== (Math.abs(dx) >= TRIGGER)) haptic(Math.abs(next) >= TRIGGER ? "medium" : "light");
      dx = next;
    }
  }
  function up(e: PointerEvent) {
    if (pid !== e.pointerId) return;
    pid = null;
    if (decided === "x") {
      if (dx >= TRIGGER) enqueueTracks([track], true);
      else if (dx <= -TRIGGER) enqueueTracks([track], false);
      if (Math.abs(dx) >= TRIGGER) haptic("success");
      settling = true;
      dx = 0;
      setTimeout(() => {
        settling = false;
        swiping = false;
      }, 260);
    }
    decided = null;
  }
  function tap() {
    if (swiping) return;
    haptic("light");
    onplay();
  }
</script>

<div class="wrap" class:focus>
  <div class="under left" class:armed={dx >= TRIGGER} style:opacity={dx > 0 ? 1 : 0}>
    <Icon name="next-up" size={20} /><span>Play Next</span>
  </div>
  <div class="under right" class:armed={dx <= -TRIGGER} style:opacity={dx < 0 ? 1 : 0}>
    <span>Add to Queue</span><Icon name="to-queue" size={20} />
  </div>
  <div
    class="row"
    class:current
    class:settling
    style:transform="translateX({dx}px)"
    role="button"
    tabindex="0"
    onclick={tap}
    onkeydown={(e) => e.key === "Enter" && onplay()}
    onpointerdown={down}
    onpointermove={move}
    onpointerup={up}
    onpointercancel={up}
    use:longpress={(el) => !swiping && trackMenu(track, el, onplay)}
  >
    {#if art}
      <div class="art"><Cover covers={track.cover} size={96} radius={7} /></div>
    {:else}
      <span class="num">{number ?? ""}</span>
    {/if}
    <div class="text">
      <div class="title ellipsis">
        {#if current}<span class="eq" class:paused={!player.status.playing}><i></i><i></i><i></i></span>{/if}{track.title}
      </div>
      <div class="sub ellipsis">{sub}</div>
    </div>
    {#if !art}<span class="dur">{time(track.duration)}</span>{/if}
    <button class="more" aria-label="More" onclick={(e) => (e.stopPropagation(), trackSheet(track))}><Icon name="more" size={18} /></button>
  </div>
</div>

<style>
  .wrap {
    position: relative;
    height: 62px;
    overflow: hidden;
  }
  .wrap.focus .row {
    background: color-mix(in srgb, var(--accent) 16%, transparent);
  }
  .under {
    position: absolute;
    inset: 0;
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 0 22px;
    font-weight: 600;
    font-size: 13.5px;
    color: var(--text-2);
    transition: background-color 0.18s, color 0.18s;
  }
  .under.left {
    background: color-mix(in srgb, var(--accent) 22%, var(--bg));
  }
  .under.right {
    justify-content: flex-end;
    background: color-mix(in srgb, var(--accent-2) 18%, var(--bg));
  }
  .under.armed {
    background: var(--accent);
    color: var(--accent-ink);
  }
  .row {
    position: relative;
    height: 100%;
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 0 6px 0 var(--gutter, 18px);
    background: var(--bg);
    touch-action: pan-y;
    -webkit-tap-highlight-color: transparent;
  }
  .row:active:not(.settling) {
    background: color-mix(in srgb, var(--text) 6%, var(--bg));
  }
  .row.settling {
    transition: transform 0.26s var(--spring);
  }
  .art {
    width: 46px;
    flex: none;
    border-radius: 7px;
    overflow: hidden;
    box-shadow: 0 2px 8px rgba(0, 0, 0, 0.18);
  }
  .num {
    width: 24px;
    flex: none;
    text-align: center;
    color: var(--text-3);
    font-variant-numeric: tabular-nums;
    font-size: 14px;
  }
  .text {
    flex: 1;
    min-width: 0;
    border-bottom: 0.5px solid var(--rule);
    align-self: stretch;
    display: flex;
    flex-direction: column;
    justify-content: center;
    padding-right: 4px;
  }
  .title {
    font-size: 15.5px;
    font-weight: 500;
  }
  .current .title {
    color: var(--accent-2);
  }
  :global([data-theme="light"]) .current .title {
    color: var(--accent);
  }
  .sub {
    font-size: 13px;
    color: var(--text-2);
    margin-top: 1px;
  }
  .dur {
    font-size: 13px;
    color: var(--text-3);
    font-variant-numeric: tabular-nums;
  }
  .more {
    width: 40px;
    height: 44px;
    flex: none;
    display: grid;
    place-items: center;
    color: var(--text-2);
  }
  .eq {
    display: inline-flex;
    align-items: flex-end;
    gap: 2px;
    height: 11px;
    margin-right: 7px;
  }
  .eq i {
    width: 2.5px;
    height: 100%;
    border-radius: 2px;
    background: currentColor;
    animation: bar 0.9s ease-in-out infinite alternate;
    transform-origin: bottom;
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
  @keyframes bar {
    from {
      transform: scaleY(0.25);
    }
    to {
      transform: scaleY(1);
    }
  }
</style>
