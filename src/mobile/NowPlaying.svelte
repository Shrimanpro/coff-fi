<script lang="ts">
  // Full-screen Now Playing sheet: Apple Music layout (reference A) with reference B's extras
  // (codec/source line, sleep timer). Three modes share the bottom controls:
  // artwork, synced lyrics, and the queue.
  import { onMount, tick } from "svelte";
  import Cover from "../components/Cover.svelte";
  import Icon from "../components/Icon.svelte";
  import Lyrics from "../components/Lyrics.svelte";
  import Scrubber from "../components/Scrubber.svelte";
  import { quality } from "../lib/format";
  import { api, blurSrc, isRemote } from "../lib/ipc";
  import RemoteBadge from "../components/RemoteBadge.svelte";
  import { remote } from "../lib/remote.svelte";
  import { anchors, conf, cycleRepeat, player, queue, setVolume, toast, toggleShuffle, volume } from "../lib/state.svelte";
  import { goArtist, m, np, openSheet, stage, toggleFav, trackSheet } from "./nav.svelte";
  import Nightstand from "./Nightstand.svelte";
  import QueuePane from "./QueuePane.svelte";

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

  // Close when the queue runs out.
  $effect(() => {
    if (!t) m.nowPlaying = false;
  });

  // ---------- Nightstand (Now Playing turned sideways) ----------
  const landscapeQuery = matchMedia("(orientation: landscape)");
  let landscape = $state(landscapeQuery.matches);
  $effect(() => {
    const on = () => (landscape = landscapeQuery.matches);
    landscapeQuery.addEventListener("change", on);
    return () => landscapeQuery.removeEventListener("change", on);
  });
  const nightstandOn = $derived(conf.s?.nightstand ?? true);
  const nightstand = $derived(landscape && nightstandOn);
  const native = (what: string) => api.nativeRequest(what).catch(() => {});
  // Rotation is only unlocked while this sheet is open (and the feature is on).
  $effect(() => {
    native(nightstandOn ? "orient:any" : "orient:portrait");
    return () => native("orient:portrait");
  });
  // Keep the screen awake while Nightstand is showing, if allowed in Settings.
  $effect(() => {
    const awake = nightstand && (conf.s?.nightstand_awake ?? true);
    native(awake ? "awake:on" : "awake:off");
    return () => native("awake:off");
  });

  // ---------- motion ----------
  // Everything below animates transform/opacity only (compositor, full refresh rate) with
  // iOS-like curves: the sheet curve for presenting, a light spring for everything that settles.
  const SHEET = "cubic-bezier(0.32, 0.72, 0, 1)";
  const SPRING = "cubic-bezier(0.3, 1.18, 0.5, 1)";
  let sheet = $state<HTMLDivElement>();
  let artEl = $state<HTMLDivElement | null>(null);
  let hartEl = $state<HTMLButtonElement | null>(null);
  let miniRect: DOMRect | null = null;
  let closing = false;

  /** The app behind the sheet recedes like an iOS card stack (k = 1 when fully covered). */
  function behind(k: number, ms = 0) {
    const el = stage.el;
    if (!el) return;
    el.style.transition = ms ? `transform ${ms}ms ${SHEET}, border-radius ${ms}ms ${SHEET}` : "none";
    el.style.transform = k > 0.001 ? `translateY(${k * 14}px) scale(${1 - 0.075 * k})` : "";
    el.style.borderRadius = k > 0.001 ? `${Math.round(k * 34)}px` : "";
  }

  /** Transform that maps `el` (laid out at `to`) onto `from`, top-left anchored. */
  const mapTo = (from: DOMRect, to: DOMRect, extraY = 0) =>
    `translate(${from.left - to.left}px, ${from.top - to.top + extraY}px) scale(${from.width / to.width})`;

  onMount(() => {
    np.close = () => close();
    const H = window.innerHeight;
    const mini = anchors.miniArt?.getBoundingClientRect();
    miniRect = mini && mini.width > 0 ? mini : null;
    sheet?.animate([{ transform: `translateY(${H}px)` }, { transform: "translateY(0)" }], { duration: 560, easing: SHEET });
    behind(1, 560);
    // The round mini-player artwork grows into the big square cover as the sheet rises.
    const el = artEl ?? hartEl;
    if (el && miniRect) {
      const to = el.getBoundingClientRect();
      el.animate(
        [
          { transform: mapTo(miniRect, to, -H), transformOrigin: "0 0", borderRadius: `${to.width / 2}px` },
          { transform: "none", transformOrigin: "0 0", borderRadius: artEl ? "12px" : "8px" },
        ],
        { duration: 560, easing: SHEET },
      );
    }
    return () => {
      np.close = () => void (m.nowPlaying = false);
      if (!m.nowPlaying) behind(0);
    };
  });

  /** Animated dismissal; `from` is the current drag offset, `vel` the release speed (px/ms). */
  function close(from = 0, vel = 0) {
    if (closing || !sheet) return;
    closing = true;
    const H = window.innerHeight;
    const rest = H - from;
    const dur = Math.round(Math.max(260, Math.min(460, rest / Math.max(vel, rest / 460))));
    sheet.getAnimations().forEach((a) => a.cancel());
    sheet.style.transform = "";
    const anim = sheet.animate([{ transform: `translateY(${from}px)` }, { transform: `translateY(${H}px)` }], {
      duration: dur,
      easing: vel > 0.4 ? "cubic-bezier(0.2, 0.6, 0.35, 1)" : SHEET,
      fill: "forwards",
    });
    behind(0, dur);
    // ...and the cover shrinks back into the mini player.
    const el = artEl ?? hartEl;
    if (el && miniRect) {
      const r = el.getBoundingClientRect();
      el.animate(
        [
          { transform: "none", transformOrigin: "0 0", borderRadius: artEl ? "12px" : "8px" },
          { transform: mapTo(miniRect, r, -rest), transformOrigin: "0 0", borderRadius: `${r.width / 2}px` },
        ],
        { duration: dur, easing: vel > 0.4 ? "cubic-bezier(0.2, 0.6, 0.35, 1)" : SHEET, fill: "forwards" },
      );
    }
    anim.finished.then(() => (m.nowPlaying = false)).catch(() => (m.nowPlaying = false));
  }

  /** Switch between artwork / lyrics / queue: the cover glides between big and thumbnail. */
  async function setMode(x: "lyrics" | "queue" | "art") {
    const next = x !== "art" && m.npMode === x ? "art" : x;
    if (next === m.npMode) return;
    const fromEl = artEl ?? hartEl;
    const from = fromEl?.getBoundingClientRect();
    m.npMode = next;
    await tick();
    const toEl = next === "art" ? artEl : hartEl;
    if (!from || !toEl) return;
    const to = toEl.getBoundingClientRect();
    toEl.animate([{ transform: mapTo(from, to), transformOrigin: "0 0" }, { transform: "none", transformOrigin: "0 0" }], {
      duration: 540,
      easing: SPRING,
    });
  }

  function press(_feedback: string, fn: () => unknown) {
    return () => fn();
  }

  const source = $derived(remote.target ? `On ${remote.target.name}` : player.cached ? "Cache" : isRemote(t) ? "Navidrome" : "Local");
  const middle = $derived(t ? `${source} · ${quality(t)}` : "");

  // ---- drag down to dismiss (follows the finger, springs back or flings closed) ----
  let dragging = $state(false);
  let startY = 0;
  let lastY = 0;
  let lastT = 0;
  let vel = 0;
  let y = 0;
  function dragStart(e: PointerEvent) {
    if (closing || (e.pointerType === "mouse" && e.button !== 0)) return;
    dragging = true;
    startY = lastY = e.clientY;
    lastT = performance.now();
    vel = 0;
    y = 0;
    sheet?.getAnimations().forEach((a) => a.finish());
    (e.currentTarget as HTMLElement).setPointerCapture(e.pointerId);
  }
  function dragMove(e: PointerEvent) {
    if (!dragging || !sheet) return;
    const now = performance.now();
    // Smoothed velocity so a quick flick is recognised even with uneven pointer events.
    vel = vel * 0.6 + ((e.clientY - lastY) / Math.max(1, now - lastT)) * 0.4;
    lastY = e.clientY;
    lastT = now;
    const d = e.clientY - startY;
    // Rubber-band when pulled up past the top.
    y = d >= 0 ? d : -Math.sqrt(-d) * 3;
    sheet.style.transform = `translateY(${y}px)`;
    behind(1 - Math.max(0, y) / window.innerHeight);
  }
  function dragEnd() {
    if (!dragging || !sheet) return;
    dragging = false;
    if (y > 120 || (vel > 0.5 && y > 20)) {
      close(y, Math.max(0, vel));
      return;
    }
    // Spring back.
    const fromY = y;
    sheet.style.transform = "";
    sheet.animate([{ transform: `translateY(${fromY}px)` }, { transform: "translateY(0)" }], { duration: 520, easing: SPRING });
    behind(1, 520);
    y = 0;
  }

  // ---- volume (Coff-fi's own app volume) ----
  let volBar = $state<HTMLDivElement>();
  let volDrag = false;
  function setVol(e: PointerEvent) {
    if (!volBar) return;
    const r = volBar.getBoundingClientRect();
    const v = Math.max(0, Math.min(1, (e.clientX - r.left) / r.width));
    const nv = Math.round(v * 100) / 100;
    setVolume(nv);
  }

  // ---- sleep timer ----
  let now = $state(Date.now());
  $effect(() => {
    if (player.sleep.mode !== "minutes") return;
    const id = setInterval(() => (now = Date.now()), 15000);
    return () => clearInterval(id);
  });
  const sleepLabel = $derived(
    player.sleep.mode === "minutes" && player.sleep.until
      ? `${Math.max(1, Math.round((player.sleep.until - now) / 60000))} min left`
      : player.sleep.mode === "end_of_track"
        ? "Stops after this song"
        : "",
  );
  async function sleep(mode: "off" | "minutes" | "end_of_track", minutes?: number) {
    try {
      player.sleep = await api.sleepTimer(mode, minutes);
      now = Date.now();
      toast(mode === "off" ? "Sleep timer off" : mode === "end_of_track" ? "Playback will stop after this song" : `Playback will pause in ${minutes} minutes`);
    } catch (e) {
      toast(String(e));
    }
  }
  function sleepSheet() {
    const items = [15, 30, 45, 60].map((n) => ({ label: n === 60 ? "1 hour" : `${n} minutes`, action: () => sleep("minutes", n) }));
    items.push({ label: "End of current song", action: () => sleep("end_of_track") });
    if (player.sleep.mode !== "off") items.unshift({ label: "Turn off timer", action: () => sleep("off") });
    openSheet(items, "Sleep Timer", sleepLabel || "Pause playback after…");
  }

  const mode = $derived(m.npMode);
  const repeatIcon = $derived(queue.repeat === "one" ? "repeat-one" : "repeat");

</script>

{#if t && nightstand}
  <Nightstand onclose={() => close()} />
{/if}

{#if t}
  <div class="np" class:dragging bind:this={sheet} role="dialog" aria-label="Now Playing">
    <div class="bg">
      {#if t.cover}<img src={blurSrc(t.cover)} alt="" />{/if}
      <div class="blob b1" style:background="radial-gradient(closest-side, {c(0, '#7a2e2e')}, transparent)"></div>
      <div class="blob b2" style:background="radial-gradient(closest-side, {c(1, '#c8913a')}, transparent)"></div>
      <div class="blob b3" style:background="radial-gradient(closest-side, {c(2, '#3a2a1e')}, transparent)"></div>
      <div class="shade"></div>
    </div>

    <div class="content">
      <div class="grab" role="presentation" onpointerdown={dragStart} onpointermove={dragMove} onpointerup={dragEnd} onpointercancel={dragEnd}>
        <button class="handle" aria-label="Close Now Playing" onclick={() => close()}></button>
      </div>
      {#if remote.target || remote.controllers.length}<div class="where"><RemoteBadge tone="light" /></div>{/if}

      {#if mode === "art"}
        <div class="art-area" role="presentation" onpointerdown={dragStart} onpointermove={dragMove} onpointerup={dragEnd} onpointercancel={dragEnd}>
          <div class="art" bind:this={artEl}>
            <div class="scaler" class:paused={!player.status.playing}><Cover covers={t.cover} size={640} radius={12} /></div>
          </div>
        </div>
        <div class="meta">
          <div class="text">
            <div class="title ellipsis">{t.title}</div>
            <button class="artist ellipsis" onclick={() => goArtist(t)}>{t.artist}</button>
          </div>
          <button class="round" class:on={player.fav} aria-label="Favorite" onclick={() => toggleFav(t)}><span class="pop" class:popped={player.fav}><Icon name={player.fav ? "star-fill" : "star"} size={20} /></span></button>
          <button class="round" aria-label="More" onclick={press("light", () => trackSheet(t))}><Icon name="more" size={20} /></button>
        </div>
      {:else}
        <div class="head" role="presentation" onpointerdown={dragStart} onpointermove={dragMove} onpointerup={dragEnd} onpointercancel={dragEnd}>
          <button class="hart" bind:this={hartEl} aria-label="Show artwork" onclick={() => setMode("art")}><Cover covers={t.cover} size={256} radius={8} /></button>
          <div class="text">
            <div class="title ellipsis">{t.title}</div>
            <button class="artist ellipsis" onclick={() => goArtist(t)}>{t.artist}</button>
          </div>
          <button class="round" class:on={player.fav} aria-label="Favorite" onclick={() => toggleFav(t)}><span class="pop" class:popped={player.fav}><Icon name={player.fav ? "star-fill" : "star"} size={19} /></span></button>
          <button class="round" aria-label="More" onclick={press("light", () => trackSheet(t))}><Icon name="more" size={19} /></button>
        </div>
        <div class="pane">
          {#if mode === "lyrics"}
            {#key t.id}<Lyrics trackId={t.id} />{/key}
          {:else}
            <QueuePane />
          {/if}
        </div>
      {/if}

      <div class="controls">
        <Scrubber tone="light" {middle} />
        <div class="transport">
          <button class="tbtn side" class:on={queue.shuffle} aria-label="Shuffle" onclick={press("selection", toggleShuffle)}><Icon name="shuffle" size={21} /></button>
          <button class="tbtn" aria-label="Previous" onclick={press("light", () => api.cmd("prev"))}><Icon name="prev" size={34} /></button>
          <button class="tbtn main" aria-label={player.status.playing ? "Pause" : "Play"} onclick={press("medium", () => api.cmd("toggle"))}>
            <span class="pp" class:on={!player.status.playing}><Icon name="play" size={46} /></span>
            <span class="pp" class:on={player.status.playing}><Icon name="pause" size={46} /></span>
          </button>
          <button class="tbtn" aria-label="Next" onclick={press("light", () => api.cmd("next"))}><Icon name="next" size={34} /></button>
          <button class="tbtn side" class:on={queue.repeat !== "off"} aria-label="Repeat" onclick={press("selection", cycleRepeat)}><Icon name={repeatIcon} size={21} /></button>
        </div>
        <div class="volume">
          <Icon name="mute" size={15} />
          <div
            class="vbar"
            bind:this={volBar}
            role="slider"
            tabindex="-1"
            aria-label="Volume"
            aria-valuemin={0}
            aria-valuemax={1}
            aria-valuenow={volume()}
            onpointerdown={(e) => {
              volDrag = true;
              volBar?.setPointerCapture(e.pointerId);
              setVol(e);
            }}
            onpointermove={(e) => volDrag && setVol(e)}
            onpointerup={() => (volDrag = false)}
          >
            <div class="vtrack"><div class="vfill" style:transform="scaleX({volume()})"></div></div>
          </div>
          <Icon name="volume" size={17} />
        </div>
        <div class="tools">
          <button class="tool" class:on={mode === "lyrics"} aria-label="Lyrics" onclick={() => setMode("lyrics")}><Icon name="lyrics" size={22} /></button>
          <button class="tool" class:on={!!remote.target} aria-label="Play on another device" onclick={press("light", () => (remote.picker = true))}><Icon name="devices" size={22} /></button>
          <button class="tool" class:on={player.sleep.mode !== "off"} aria-label="Sleep timer" onclick={press("light", sleepSheet)}>
            <Icon name="sleep" size={22} />
            {#if sleepLabel}<span class="badge">{player.sleep.mode === "minutes" ? sleepLabel.replace(" left", "") : "1 song"}</span>{/if}
          </button>
          <button class="tool" class:on={mode === "queue"} aria-label="Queue" onclick={() => setMode("queue")}><Icon name="queue" size={22} /></button>
        </div>
      </div>
    </div>
  </div>
{/if}

<style>
  .np {
    position: fixed;
    inset: 0;
    z-index: 100;
    color: #fff;
    overflow: hidden;
    background: #18120e;
    border-radius: 0;
    will-change: transform;
    transition: border-radius 0.3s ease;
  }
  .np.dragging {
    border-radius: 34px 34px 0 0;
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
    transform: scale(1.4);
    filter: blur(50px) saturate(1.5) brightness(0.8);
  }
  .blob {
    position: absolute;
    width: 120vmax;
    height: 120vmax;
    border-radius: 50%;
    opacity: 0.6;
    mix-blend-mode: soft-light;
    animation: drift 40s ease-in-out infinite alternate;
  }
  .b1 {
    left: -50vmax;
    top: -50vmax;
  }
  .b2 {
    right: -60vmax;
    top: 0;
    animation-duration: 48s;
    animation-delay: -12s;
  }
  .b3 {
    left: -20vmax;
    bottom: -70vmax;
    animation-duration: 54s;
    animation-delay: -25s;
  }
  @keyframes drift {
    0% {
      transform: translate(0, 0) scale(1) rotate(0deg);
    }
    100% {
      transform: translate(10vmax, 14vmax) scale(1.12) rotate(35deg);
    }
  }
  .shade {
    background: linear-gradient(180deg, rgba(0, 0, 0, 0.12), rgba(0, 0, 0, 0.32));
  }

  .content {
    position: relative;
    height: 100%;
    display: flex;
    flex-direction: column;
    padding: env(safe-area-inset-top) 24px max(14px, env(safe-area-inset-bottom));
  }
  .grab {
    flex: none;
    height: 30px;
    display: grid;
    place-items: center;
    touch-action: none;
  }
  .handle {
    padding: 0;
    width: 38px;
    height: 5px;
    border-radius: 3px;
    background: rgba(255, 255, 255, 0.4);
  }

  /* ---- artwork mode ---- */
  .art-area {
    flex: 1;
    min-height: 0;
    display: flex;
    align-items: center;
    justify-content: center;
    padding: 8px 0 14px;
    touch-action: none;
  }
  .art-area {
    container-type: size;
  }
  .art {
    /* Largest square that fits the free space, on every screen size. */
    width: min(100cqw, 100cqh);
    aspect-ratio: 1;
    border-radius: 12px;
    overflow: hidden;
    box-shadow: 0 26px 60px rgba(0, 0, 0, 0.45);
    will-change: transform;
  }
  /* Paused: the cover settles back a little, like Apple Music (inner layer, so it never fights
     the cover's own morph animations on the outer one). */
  .scaler {
    height: 100%;
    border-radius: 12px;
    overflow: hidden;
    transition: transform 0.7s cubic-bezier(0.3, 1.25, 0.5, 1);
  }
  .scaler.paused {
    transform: scale(0.84);
  }
  .art:has(.paused) {
    box-shadow: 0 12px 30px rgba(0, 0, 0, 0.3);
  }
  .art {
    transition: box-shadow 0.6s ease;
  }
  .meta,
  .pane,
  .head .text,
  .head .round {
    animation: rise 0.5s cubic-bezier(0.3, 1.1, 0.5, 1) both;
  }
  .pane {
    animation-delay: 0.04s;
  }
  @keyframes rise {
    from {
      opacity: 0;
      transform: translateY(14px);
    }
  }
  .meta,
  .head {
    flex: none;
    display: flex;
    align-items: center;
    gap: 10px;
  }
  .meta {
    margin-bottom: 14px;
  }
  .text {
    flex: 1;
    min-width: 0;
  }
  .title {
    font-size: 21px;
    font-weight: 700;
    letter-spacing: -0.01em;
  }
  .artist {
    display: block;
    max-width: 100%;
    text-align: left;
    font-size: 18px;
    color: rgba(255, 255, 255, 0.68);
  }
  .round {
    width: 38px;
    height: 38px;
    flex: none;
    border-radius: 50%;
    display: grid;
    place-items: center;
    color: #fff;
    background: rgba(255, 255, 255, 0.14);
    -webkit-tap-highlight-color: transparent;
  }
  .round.on {
    color: var(--accent-2);
  }

  /* ---- lyrics / queue modes ---- */
  .head {
    padding: 4px 0 12px;
    touch-action: none;
  }
  .hart {
    width: 64px;
    flex: none;
    border-radius: 8px;
    overflow: hidden;
    box-shadow: 0 8px 20px rgba(0, 0, 0, 0.3);
  }
  .head .title {
    font-size: 17px;
  }
  .head .artist {
    font-size: 15px;
  }
  .pane {
    flex: 1;
    min-height: 0;
    margin: 0 -24px;
    padding: 0 24px;
    overflow: hidden;
  }
  .pane :global(.line) {
    font-size: 27px;
  }

  /* ---- controls ---- */
  .controls {
    flex: none;
    padding-top: 6px;
  }
  .transport {
    display: flex;
    align-items: center;
    justify-content: space-between;
    margin: 6px -6px 4px;
  }
  .tbtn {
    width: 64px;
    height: 64px;
    display: grid;
    place-items: center;
    border-radius: 50%;
    color: #fff;
    -webkit-tap-highlight-color: transparent;
    transition:
      transform 0.45s cubic-bezier(0.3, 1.4, 0.5, 1),
      background-color 0.2s;
  }
  .tbtn:active {
    transform: scale(0.84);
    background: rgba(255, 255, 255, 0.1);
    transition-duration: 0.08s;
  }
  .tbtn.main {
    position: relative;
  }
  .pp {
    position: absolute;
    inset: 0;
    display: grid;
    place-items: center;
    opacity: 0;
    transform: scale(0.4) rotate(-25deg);
    transition:
      opacity 0.18s ease,
      transform 0.45s cubic-bezier(0.3, 1.4, 0.5, 1);
  }
  .pp.on {
    opacity: 1;
    transform: none;
  }
  .round {
    transition: transform 0.4s cubic-bezier(0.3, 1.4, 0.5, 1), color 0.2s;
  }
  .round:active,
  .tool:active {
    transform: scale(0.86);
    transition-duration: 0.08s;
  }
  .pop {
    display: grid;
    transition: transform 0.45s cubic-bezier(0.3, 1.8, 0.5, 1);
  }
  .pop.popped {
    animation: pop 0.45s cubic-bezier(0.3, 1.8, 0.5, 1);
  }
  @keyframes pop {
    40% {
      transform: scale(1.35);
    }
  }
  .tbtn.main {
    width: 76px;
    height: 76px;
  }
  .tbtn.side {
    width: 44px;
    height: 44px;
    color: rgba(255, 255, 255, 0.7);
  }
  .tbtn.side.on {
    color: #fff;
    background: rgba(255, 255, 255, 0.2);
  }
  .volume {
    display: flex;
    align-items: center;
    gap: 12px;
    color: rgba(255, 255, 255, 0.7);
    margin: 4px 0 12px;
  }
  .vbar {
    flex: 1;
    height: 28px;
    display: flex;
    align-items: center;
    touch-action: none;
  }
  .vtrack {
    position: relative;
    width: 100%;
    height: 6px;
    border-radius: 6px;
    overflow: hidden;
    background: rgba(255, 255, 255, 0.22);
  }
  .vfill {
    position: absolute;
    inset: 0;
    background: rgba(255, 255, 255, 0.92);
    transform-origin: left;
  }
  .where {
    display: flex;
    justify-content: center;
    padding: 0 24px 4px;
  }
  .tools {
    display: flex;
    justify-content: space-around;
    align-items: center;
  }
  .tool {
    position: relative;
    width: 50px;
    height: 50px;
    display: grid;
    place-items: center;
    border-radius: 50%;
    color: rgba(255, 255, 255, 0.72);
    -webkit-tap-highlight-color: transparent;
    transition:
      background-color 0.25s ease,
      color 0.25s ease,
      transform 0.45s cubic-bezier(0.3, 1.4, 0.5, 1);
  }
  .tool.on {
    color: #1b130d;
    background: rgba(255, 255, 255, 0.82);
  }
  .badge {
    position: absolute;
    top: -4px;
    left: 50%;
    translate: -50% 0;
    white-space: nowrap;
    font-size: 9.5px;
    font-weight: 700;
    padding: 1px 6px;
    border-radius: 8px;
    color: #1b130d;
    background: var(--accent-2);
  }
</style>
