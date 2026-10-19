<script lang="ts">
  import { goBack, nav, ui } from "../lib/state.svelte";
  import Icon from "./Icon.svelte";

  // Two-finger swipe to the right on a trackpad goes back, like a browser.
  const THRESHOLD = 140; // px of horizontal travel
  const GAP = 180; // ms without wheel events ends a gesture (also swallows momentum)

  let progress = $state(0);
  let armed = $state(false);
  let acc = 0;
  let last = 0;
  let axis: "x" | "y" | null = null;
  let fired = false;
  let endTimer: ReturnType<typeof setTimeout> | undefined;

  /** True if something under the pointer can still scroll horizontally in this direction. */
  function scrollsHorizontally(target: EventTarget | null, dx: number): boolean {
    for (let el = target as HTMLElement | null; el && el !== document.body; el = el.parentElement) {
      if (el.scrollWidth <= el.clientWidth + 1) continue;
      const ox = getComputedStyle(el).overflowX;
      if (ox !== "auto" && ox !== "scroll") continue;
      if (dx < 0 ? el.scrollLeft > 0 : el.scrollLeft + el.clientWidth < el.scrollWidth - 1) return true;
    }
    return false;
  }

  function end() {
    acc = 0;
    axis = null;
    fired = false;
    progress = 0;
    armed = false;
  }

  function onWheel(e: WheelEvent) {
    if (ui.fullscreen || ui.settingsOpen || ui.palette || ui.menu || e.ctrlKey) return;
    if (e.timeStamp - last > GAP) end();
    last = e.timeStamp;
    clearTimeout(endTimer);
    endTimer = setTimeout(end, GAP);

    // The first movement of a gesture decides whether it is a scroll or a swipe.
    if (!axis) axis = Math.abs(e.deltaX) > Math.abs(e.deltaY) * 1.3 ? "x" : "y";
    if (axis === "y" || fired || !nav.canGoBack) return;
    if (scrollsHorizontally(e.target, e.deltaX)) return;

    // With natural scrolling, fingers moving right report a negative deltaX; otherwise positive.
    const natural = (e as WheelEvent & { webkitDirectionInvertedFromDevice?: boolean }).webkitDirectionInvertedFromDevice ?? true;
    const unit = e.deltaMode === 1 ? 16 : 1;
    acc = Math.max(0, acc + (natural ? -e.deltaX : e.deltaX) * unit);
    progress = Math.min(1, acc / THRESHOLD);
    armed = progress >= 1;
    if (armed) {
      fired = true;
      goBack();
      setTimeout(() => {
        progress = 0;
        armed = false;
      }, 220);
    }
  }

  function onMouse(e: MouseEvent) {
    // The "back" side button on mice.
    if (e.button === 3) {
      e.preventDefault();
      goBack();
    }
  }
</script>

<svelte:window onwheel={onWheel} onmouseup={onMouse} />

<div class="swipe" class:armed style:--p={progress} aria-hidden="true">
  <Icon name="chev-left" size={22} />
</div>

<style>
  .swipe {
    --p: 0;
    position: fixed;
    left: 0;
    top: 50%;
    z-index: 55;
    width: 46px;
    height: 46px;
    margin-top: -23px;
    border-radius: 50%;
    display: grid;
    place-items: center;
    color: var(--text);
    background: var(--glass-strong);
    border: 1px solid var(--glass-border);
    backdrop-filter: blur(16px) saturate(160%);
    -webkit-backdrop-filter: blur(16px) saturate(160%);
    box-shadow: 0 8px 24px rgba(0, 0, 0, 0.25), inset 0 1px 0 var(--glass-hi);
    transform: translateX(calc(-56px + var(--p) * 76px)) scale(calc(0.7 + var(--p) * 0.3));
    opacity: var(--p);
    pointer-events: none;
    transition:
      transform 0.12s linear,
      opacity 0.12s linear,
      background-color 0.2s;
  }
  .swipe.armed {
    background: var(--accent);
    color: #fff;
  }
</style>
