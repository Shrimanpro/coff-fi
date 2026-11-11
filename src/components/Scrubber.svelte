<script lang="ts">
  import { time } from "../lib/format";
  import { haptic } from "../lib/haptics";
  import { api } from "../lib/ipc";
  import { player, position } from "../lib/state.svelte";

  let {
    showTimes = true,
    middle = "",
    tone = "default",
    inline = false,
  }: { showTimes?: boolean; middle?: string; tone?: "default" | "light"; inline?: boolean } = $props();

  let bar: HTMLDivElement;
  let fill: HTMLDivElement;
  let elapsed = $state(0);
  let drag = $state<number | null>(null);
  const dur = $derived(player.status.duration || player.track?.duration || 0);

  // Progress is driven by rAF straight into the DOM while playing — no reactive churn.
  $effect(() => {
    const playing = player.status.playing;
    player.status.position;
    let raf = 0;
    let lastSec = -1;
    const tick = () => {
      const p = drag ?? position();
      if (fill) fill.style.transform = `scaleX(${dur ? Math.min(1, p / dur) : 0})`;
      const s = Math.floor(p);
      if (s !== lastSec) {
        lastSec = s;
        elapsed = p;
      }
      if (playing && drag === null) raf = requestAnimationFrame(tick);
    };
    tick();
    return () => cancelAnimationFrame(raf);
  });

  function at(e: PointerEvent) {
    const r = bar.getBoundingClientRect();
    return Math.max(0, Math.min(1, (e.clientX - r.left) / r.width)) * dur;
  }

  function down(e: PointerEvent) {
    e.stopPropagation();
    bar.setPointerCapture(e.pointerId);
    drag = at(e);
    haptic("selection");
  }
  function move(e: PointerEvent) {
    if (drag !== null) drag = at(e);
  }
  function up(e: PointerEvent) {
    if (drag === null) return;
    const to = at(e);
    drag = null;
    haptic("light");
    // Optimistically jump so the bar doesn't snap back while the seek happens.
    player.status = { ...player.status, position: to };
    player.at = performance.now();
    api.cmd("seek", to);
  }
</script>

<div class="scrub {tone}" class:inline>
  {#if inline}<span class="t">{time(drag ?? elapsed)}</span>{/if}
  <div
    class="bar"
    class:dragging={drag !== null}
    bind:this={bar}
    role="slider"
    tabindex="-1"
    aria-label="Seek"
    aria-valuemin={0}
    aria-valuemax={dur}
    aria-valuenow={elapsed}
    onpointerdown={down}
    onpointermove={move}
    onpointerup={up}
  >
    <div class="track"><div class="fill" bind:this={fill}></div></div>
  </div>
  {#if inline}
    <span class="t">{time(dur)}</span>
  {:else if showTimes}
    <div class="times">
      <span>{time(drag ?? elapsed)}</span>
      {#if middle}<span class="mid">{middle}</span>{/if}
      <span>-{time(dur - (drag ?? elapsed))}</span>
    </div>
  {/if}
</div>

<style>
  .scrub {
    width: 100%;
  }
  .bar {
    height: 16px;
    display: flex;
    align-items: center;
    cursor: pointer;
    touch-action: none;
  }
  .track {
    position: relative;
    width: 100%;
    height: 5px;
    border-radius: 5px;
    overflow: hidden;
    background: color-mix(in srgb, var(--text) 16%, transparent);
    transition: height 0.2s var(--ease);
  }
  .bar:hover .track,
  .bar.dragging .track {
    height: 8px;
  }
  .fill {
    position: absolute;
    inset: 0;
    background: color-mix(in srgb, var(--text) 82%, transparent);
    transform-origin: left;
    transform: scaleX(0);
    will-change: transform;
  }
  .light .track {
    background: rgba(255, 255, 255, 0.22);
  }
  .light .fill {
    background: rgba(255, 255, 255, 0.92);
  }
  .times {
    display: flex;
    justify-content: space-between;
    align-items: center;
    font-size: 11.5px;
    font-variant-numeric: tabular-nums;
    color: var(--text-2);
    margin-top: 2px;
  }
  .light .times {
    color: rgba(255, 255, 255, 0.7);
  }
  .inline {
    display: flex;
    align-items: center;
    gap: 12px;
  }
  .inline .bar {
    flex: 1;
  }
  .t {
    font-size: 12px;
    font-variant-numeric: tabular-nums;
    color: rgba(255, 255, 255, 0.75);
    min-width: 32px;
  }
  .t:last-child {
    text-align: right;
  }
  .mid {
    font-size: 10.5px;
    letter-spacing: 0.08em;
    text-transform: uppercase;
    opacity: 0.85;
  }
</style>
