<script lang="ts">
  // Nightstand mode: Now Playing turned sideways. Mirrors the desktop full-screen player
  // (components/FullPlayer.svelte): blurred cover + drifting palette blobs, artwork with
  // scrubber, title and transport on the left, synced lyrics on the right. Controls fade
  // out after a few seconds; tap anywhere to bring them back.
  import { onMount } from "svelte";
  import Cover from "../components/Cover.svelte";
  import Icon from "../components/Icon.svelte";
  import Lyrics from "../components/Lyrics.svelte";
  import Scrubber from "../components/Scrubber.svelte";
  import Transport from "../components/Transport.svelte";
  import { haptic } from "../lib/haptics";
  import { api, blurSrc } from "../lib/ipc";
  import { player } from "../lib/state.svelte";

  let { onclose }: { onclose: () => void } = $props();

  const t = $derived(player.track);
  let palette = $state<string[]>([]);
  $effect(() => {
    const c = t?.cover;
    if (!c) {
      palette = [];
      return;
    }
    api.palette(c).then((p) => (palette = p)).catch(() => (palette = []));
  });
  const c = (i: number, fb: string) => palette[i] ?? fb;

  let idle = $state(false);
  let idleTimer: ReturnType<typeof setTimeout> | undefined;
  function poke() {
    idle = false;
    clearTimeout(idleTimer);
    idleTimer = setTimeout(() => (idle = true), 3200);
  }

  let root: HTMLDivElement;
  onMount(() => {
    haptic("soft");
    root.animate([{ opacity: 0, transform: "scale(1.04)" }, { opacity: 1, transform: "none" }], { duration: 480, easing: "cubic-bezier(0.22, 1, 0.36, 1)" });
    poke();
    return () => clearTimeout(idleTimer);
  });
</script>

<div class="full" class:idle bind:this={root} onpointerdown={poke} role="dialog" aria-label="Nightstand" tabindex="-1">
  <div class="bg" class:still={!player.status.playing}>
    {#if t?.cover}<img src={blurSrc(t.cover)} alt="" />{/if}
    <div class="blob b1" style:background="radial-gradient(closest-side, {c(0, '#7a2e2e')}, transparent)"></div>
    <div class="blob b2" style:background="radial-gradient(closest-side, {c(1, '#c8913a')}, transparent)"></div>
    <div class="blob b3" style:background="radial-gradient(closest-side, {c(2, '#3a2a1e')}, transparent)"></div>
    <div class="shade"></div>
  </div>

  <button class="close" aria-label="Close Now Playing" onclick={() => (haptic("light"), onclose())}><Icon name="chev-down" size={22} /></button>

  {#if t}
    <div class="layout">
      <div class="left">
        <div class="art" class:paused={!player.status.playing}><Cover covers={t.cover} size={640} radius={10} /></div>
        <div class="scrub"><Scrubber inline tone="light" /></div>
        <div class="title ellipsis">{t.title}</div>
        <div class="artist ellipsis">{t.artist}</div>
        <!-- Haptic tick for any transport button (Transport is shared with desktop). -->
        <div class="controls" role="presentation" onclickcapture={() => haptic("light")}><Transport big /></div>
      </div>
      <div class="right">
        {#key t.id}
          <Lyrics trackId={t.id} />
        {/key}
      </div>
    </div>
  {/if}
</div>

<style>
  .full {
    position: fixed;
    inset: 0;
    z-index: 130;
    color: #fff;
    overflow: hidden;
    background: #111;
    outline: none;
  }
  .bg,
  .bg img,
  .shade {
    position: absolute;
    inset: 0;
  }
  .bg img {
    width: 100%;
    height: 100%;
    object-fit: cover;
    transform: scale(1.3);
    filter: blur(50px) saturate(1.5) brightness(0.85);
  }
  .blob {
    position: absolute;
    width: 75vmax;
    height: 75vmax;
    border-radius: 50%;
    opacity: 0.65;
    mix-blend-mode: soft-light;
    animation: drift 38s ease-in-out infinite alternate;
    will-change: transform;
  }
  .b1 {
    left: -20vmax;
    top: -25vmax;
  }
  .b2 {
    right: -25vmax;
    top: -10vmax;
    animation-duration: 46s;
    animation-delay: -12s;
  }
  .b3 {
    left: 10vmax;
    bottom: -35vmax;
    animation-duration: 52s;
    animation-delay: -25s;
  }
  .still .blob {
    animation-play-state: paused;
  }
  @keyframes drift {
    0% {
      transform: translate(0, 0) scale(1) rotate(0deg);
    }
    50% {
      transform: translate(12vmax, 8vmax) scale(1.15) rotate(40deg);
    }
    100% {
      transform: translate(-8vmax, 14vmax) scale(0.95) rotate(-30deg);
    }
  }
  .shade {
    background: linear-gradient(90deg, rgba(0, 0, 0, 0.18), rgba(0, 0, 0, 0.3));
  }

  .close {
    position: absolute;
    top: max(14px, env(safe-area-inset-top));
    left: max(16px, env(safe-area-inset-left));
    z-index: 2;
    width: 40px;
    height: 40px;
    border-radius: 50%;
    display: grid;
    place-items: center;
    color: rgba(255, 255, 255, 0.85);
    background: rgba(255, 255, 255, 0.12);
    backdrop-filter: blur(12px);
    -webkit-backdrop-filter: blur(12px);
    transition:
      opacity 0.4s var(--ease),
      transform 0.4s cubic-bezier(0.3, 1.4, 0.5, 1);
  }
  .close:active {
    transform: scale(0.86);
  }

  .layout {
    position: relative;
    height: 100%;
    display: grid;
    grid-template-columns: minmax(240px, 40%) minmax(0, 1fr);
    grid-template-rows: minmax(0, 1fr);
    gap: 4vw;
    /* Keep clear of the Dynamic Island / notch, which sits on a side in landscape. */
    padding: max(10px, env(safe-area-inset-top)) max(24px, env(safe-area-inset-right)) max(10px, env(safe-area-inset-bottom))
      max(24px, env(safe-area-inset-left));
  }
  .left {
    align-self: center;
    justify-self: center;
    width: min(30vw, 50vh, 380px);
    display: flex;
    flex-direction: column;
    align-items: center;
  }
  .art {
    width: 100%;
    border-radius: 10px;
    overflow: hidden;
    box-shadow: 0 24px 60px rgba(0, 0, 0, 0.45);
    transition: transform 0.7s cubic-bezier(0.3, 1.25, 0.5, 1);
  }
  .art.paused {
    transform: scale(0.9);
  }
  .scrub {
    width: 100%;
    margin: 14px 0 8px;
  }
  .title {
    max-width: 100%;
    font-size: 19px;
    font-weight: 700;
  }
  .artist {
    max-width: 100%;
    font-size: 14.5px;
    color: rgba(255, 255, 255, 0.7);
    margin-top: 2px;
  }
  .controls {
    margin-top: 8px;
    transition:
      opacity 0.4s var(--ease),
      transform 0.4s var(--ease);
  }
  .controls :global(.icon-btn),
  .controls :global(.icon-btn:not(.main):not(.on)) {
    color: rgba(255, 255, 255, 0.9);
  }
  .controls :global(.on) {
    color: #fff !important;
    background: rgba(255, 255, 255, 0.16);
  }
  .idle .controls,
  .idle .close {
    opacity: 0;
    pointer-events: none;
  }
  .idle .controls {
    transform: translateY(6px);
  }
  .right {
    height: 100%;
    min-width: 0;
    min-height: 0;
    overflow: hidden;
  }
  .right :global(.lyrics.synced) {
    padding-top: 0;
  }
</style>
