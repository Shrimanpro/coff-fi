<script lang="ts">
  import { onMount } from "svelte";
  import { api, blurSrc } from "../lib/ipc";
  import { anchors, player, ui } from "../lib/state.svelte";
  import Cover from "./Cover.svelte";
  import Icon from "./Icon.svelte";
  import Lyrics from "./Lyrics.svelte";
  import Scrubber from "./Scrubber.svelte";
  import Transport from "./Transport.svelte";

  const t = $derived(player.track);
  let root: HTMLDivElement;
  let art = $state<HTMLDivElement>();
  let closing = false;
  let idle = $state(false);
  let idleTimer: ReturnType<typeof setTimeout> | undefined;
  let palette = $state<string[]>([]);

  const EASE = "cubic-bezier(0.22, 1, 0.36, 1)";

  $effect(() => {
    const c = t?.cover;
    if (!c) {
      palette = [];
      return;
    }
    api.palette(c).then((p) => (palette = p)).catch(() => (palette = []));
  });

  /** FLIP: transform between the mini-player artwork and the big artwork. */
  function flip(reverse: boolean): Animation | null {
    const from = anchors.miniArt?.getBoundingClientRect();
    if (!from || !art || from.width === 0) return null;
    const to = art.getBoundingClientRect();
    const dx = from.left + from.width / 2 - (to.left + to.width / 2);
    const dy = from.top + from.height / 2 - (to.top + to.height / 2);
    const s = from.width / to.width;
    const small = { transform: `translate(${dx}px, ${dy}px) scale(${s})` };
    const big = { transform: "none" };
    return art.animate(reverse ? [big, small] : [small, big], { duration: 560, easing: EASE, fill: "both" });
  }

  onMount(() => {
    root.animate([{ opacity: 0 }, { opacity: 1 }], { duration: 380, easing: EASE });
    flip(false);
    poke();
  });

  export async function close() {
    if (closing) return;
    closing = true;
    const a = flip(true);
    const fade = root.animate([{ opacity: 1 }, { opacity: 0 }], { duration: 420, easing: EASE, fill: "forwards" });
    await (a ?? fade).finished.catch(() => {});
    ui.fullscreen = false;
  }

  function poke() {
    idle = false;
    clearTimeout(idleTimer);
    idleTimer = setTimeout(() => (idle = true), 2800);
  }

  function onKey(e: KeyboardEvent) {
    if (e.key === "Escape" && !ui.menu && !ui.settingsOpen) {
      e.preventDefault();
      close();
    }
  }

  // Close if the queue runs out while open.
  $effect(() => {
    if (!t) ui.fullscreen = false;
  });

  const c = (i: number, fb: string) => palette[i] ?? fb;
</script>

<svelte:window onkeydown={onKey} />

<div class="full" class:idle bind:this={root} onmousemove={poke} role="dialog" aria-label="Now playing" tabindex="-1">
  <div class="bg" class:still={!player.status.playing}>
    {#if t?.cover}
      <img src={blurSrc(t.cover)} alt="" />
    {/if}
    <div class="blob b1" style:background="radial-gradient(closest-side, {c(0, '#7a2e2e')}, transparent)"></div>
    <div class="blob b2" style:background="radial-gradient(closest-side, {c(1, '#c8913a')}, transparent)"></div>
    <div class="blob b3" style:background="radial-gradient(closest-side, {c(2, '#3a2a1e')}, transparent)"></div>
    <div class="shade"></div>
  </div>

  <button class="close" title="Close (Esc)" onclick={close}><Icon name="chev-down" size={22} /></button>

  {#if t}
    <div class="layout">
      <div class="left">
        <div class="art" bind:this={art}><Cover covers={t.cover} size={640} radius={10} /></div>
        <div class="scrub"><Scrubber inline tone="light" /></div>
        <div class="title ellipsis">{t.title}</div>
        <div class="artist ellipsis">{t.artist}</div>
        <div class="controls"><Transport big /></div>
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
    z-index: 60;
    color: #fff;
    overflow: hidden;
    background: #111;
    outline: none;
  }
  .full.idle {
    cursor: none;
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
    top: 22px;
    left: 24px;
    z-index: 2;
    width: 40px;
    height: 40px;
    border-radius: 50%;
    display: grid;
    place-items: center;
    color: rgba(255, 255, 255, 0.85);
    background: rgba(255, 255, 255, 0.1);
    backdrop-filter: blur(12px);
    transition: opacity 0.4s var(--ease), background-color 0.2s;
  }
  .close:hover {
    background: rgba(255, 255, 255, 0.2);
  }
  .layout {
    position: relative;
    height: 100%;
    display: grid;
    grid-template-columns: minmax(300px, 42%) minmax(0, 1fr);
    /* Pin the row to the viewport, or a long lyric sheet grows it and pushes everything off-screen. */
    grid-template-rows: minmax(0, 1fr);
    gap: 5vw;
    padding: 0 6vw 0 8vw;
  }
  .left {
    align-self: center;
    justify-self: center;
    width: min(32vw, 62vh, 460px);
    display: flex;
    flex-direction: column;
    align-items: center;
  }
  .art {
    width: 100%;
    border-radius: 10px;
    box-shadow: 0 30px 70px rgba(0, 0, 0, 0.45);
    transform-origin: center;
  }
  .scrub {
    width: 92%;
    margin: 22px 0 14px;
  }
  .title {
    max-width: 100%;
    font-size: clamp(20px, 1.8vw, 26px);
    font-weight: 700;
  }
  .artist {
    max-width: 100%;
    font-size: clamp(14px, 1.2vw, 17px);
    color: rgba(255, 255, 255, 0.7);
    margin-top: 4px;
  }
  .controls {
    margin-top: 18px;
    transition: opacity 0.4s var(--ease), transform 0.4s var(--ease);
  }
  .controls :global(.icon-btn),
  .controls :global(.icon-btn:not(.main):not(.on)) {
    color: rgba(255, 255, 255, 0.9);
  }
  .controls :global(.icon-btn:hover) {
    background: rgba(255, 255, 255, 0.12);
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
</style>
