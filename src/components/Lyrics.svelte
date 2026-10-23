<script lang="ts">
  import { api } from "../lib/ipc";
  import { player, position } from "../lib/state.svelte";
  import type { Lyrics } from "../lib/types";

  let { trackId }: { trackId: number } = $props();

  let lyr = $state<Lyrics | null>(null);
  let loading = $state(true);
  /** Set when LRCLIB couldn't be reached (as opposed to having no lyrics). */
  let failed = $state(false);
  let retryTimer: ReturnType<typeof setTimeout> | undefined;
  let active = $state(-1);
  let shift = $state(0);
  let manual = $state(false);
  let viewport: HTMLDivElement;
  let lineEls: HTMLElement[] = $state([]);
  let manualTimer: ReturnType<typeof setTimeout> | undefined;

  async function load(id: number, refresh = false, attempt = 0) {
    clearTimeout(retryTimer);
    loading = true;
    failed = false;
    lyr = null;
    active = -1;
    try {
      const l = await api.lyrics(id, refresh);
      if (id === trackId) lyr = l;
    } catch {
      if (id !== trackId) return;
      failed = true;
      // LRCLIB is often briefly overloaded; try again on our own a couple of times.
      if (attempt < 2) retryTimer = setTimeout(() => load(id, false, attempt + 1), 4000 * (attempt + 1));
    }
    if (id === trackId) loading = false;
  }

  $effect(() => {
    load(trackId);
    return () => clearTimeout(retryTimer);
  });

  const synced = $derived(!!lyr && lyr.synced !== "none" && lyr.lines.length > 0);

  /** Held notes: words sung noticeably long swell and glow; simple scripts also ripple letter by letter. */
  const LONG_MIN = 0.9;
  const graphemes = new Intl.Segmenter(undefined, { granularity: "grapheme" });
  // Splitting complex scripts (Tamil, Devanagari, Arabic…) into boxes would break their shaping.
  const SIMPLE = /^[\p{Script=Latin}\p{Script=Greek}\p{Script=Cyrillic}\p{M}\p{P}\p{N}'’]+$/u;
  type Held = { letters: string[] | null } | null;
  const held = $derived<Held[][]>(
    lyr?.lines.map((l) =>
      l.words.map((w) => {
        const core = w.text.trim();
        const letters = Array.from(graphemes.segment(core), (g) => g.segment);
        if (!letters.length || letters.length > 14 || w.end - w.start < Math.max(LONG_MIN, letters.length * 0.13)) return null;
        return { letters: SIMPLE.test(core) ? letters : null };
      }),
    ) ?? [],
  );
  const split = (text: string) => {
    const core = text.trimEnd();
    return { core, tail: text.slice(core.length) };
  };
  const clamp01 = (x: number) => (x < 0 ? 0 : x > 1 ? 1 : x);
  const easeOut = (x: number) => 1 - (1 - x) * (1 - x);

  /** Index of the last line that has started (binary search). */
  function find(t: number): number {
    const lines = lyr!.lines;
    let lo = 0,
      hi = lines.length - 1,
      ans = -1;
    while (lo <= hi) {
      const mid = (lo + hi) >> 1;
      if (lines[mid].start <= t) {
        ans = mid;
        lo = mid + 1;
      } else hi = mid - 1;
    }
    return ans;
  }

  // Drive highlighting with rAF; only the active line's words are touched each frame.
  $effect(() => {
    if (!synced) return;
    const playing = player.status.playing;
    player.status.position;
    let raf = 0;
    let words: { el: HTMLElement; chars: HTMLElement[] }[] = [];
    let wordsFor = -1;
    const tick = () => {
      const t = position() + 0.12; // tiny lead feels more in sync with the voice
      const i = find(t);
      if (i !== active) active = i;
      if (lyr && i >= 0 && lyr.lines[i].words.length) {
        if (wordsFor !== i) {
          const els = lineEls[i]?.querySelectorAll<HTMLElement>(".w") ?? [];
          words = Array.from(els, (el) => ({ el, chars: Array.from(el.querySelectorAll<HTMLElement>(".c")) }));
          wordsFor = i;
        }
        const ws = lyr.lines[i].words;
        words.forEach(({ el, chars }, k) => {
          const w = ws[k];
          const p = w ? clamp01((t - w.start) / Math.max(0.05, w.end - w.start)) : 1;
          el.style.setProperty("--p", p.toFixed(3));
          // Words rise as they are sung and stay up until the line ends.
          el.style.setProperty("--lift", easeOut(clamp01(p * 1.6)).toFixed(3));
          if (chars.length) {
            // Swell peaks in the middle of the held note; letters ripple in sequence.
            el.style.setProperty("--g", (p > 0 && p < 1 ? Math.sin(Math.PI * p) : 0).toFixed(3));
            const n = chars.length;
            chars.forEach((c, j) => {
              const cp = clamp01(p * n - j);
              c.style.setProperty("--p", cp.toFixed(3));
              c.style.setProperty("--wave", (p < 1 ? Math.sin(Math.PI * cp) : 0).toFixed(3));
            });
          }
        });
      }
      if (playing) raf = requestAnimationFrame(tick);
    };
    tick();
    return () => cancelAnimationFrame(raf);
  });

  let vh = $state(0);
  // Keep the active line at ~30% of the viewport height.
  $effect(() => {
    vh;
    const el = lineEls[active] ?? lineEls[0];
    if (!synced || !el || !viewport || manual) return;
    shift = viewport.clientHeight * 0.3 - (el.offsetTop + el.offsetHeight / 2);
  });

  function wheel(e: WheelEvent) {
    if (!synced) return;
    e.preventDefault();
    manual = true;
    shift -= e.deltaY;
    clearTimeout(manualTimer);
    manualTimer = setTimeout(() => (manual = false), 2500);
  }

  function seek(i: number) {
    if (!synced || !lyr) return;
    manual = false;
    api.cmd("seek", lyr.lines[i].start);
  }

  const style = (i: number) => {
    const d = i - active;
    const dist = Math.abs(d);
    // Lines below the active one follow with a slight stagger, like a gentle spring.
    const delay = d > 0 ? Math.min(d, 8) * 35 : 0;
    const blur = manual ? 0 : Math.min(dist, 4) * 0.7;
    return `transform: translateY(${shift}px); transition-delay: ${delay}ms; filter: ${blur ? `blur(${blur}px)` : "none"}`;
  };
</script>

<svelte:window onresize={() => (vh = window.innerHeight)} />

<div class="wrap">
<div class="lyrics" role="presentation" class:synced class:manual bind:this={viewport} onwheel={wheel}>
  {#if loading}
    <div class="msg"><span class="dots"><i></i><i></i><i></i></span></div>
  {:else if !lyr}
    <div class="msg">
      <p>{failed ? "Couldn't reach the lyrics service." : "No lyrics found for this song."}</p>
      <button class="retry" onclick={() => load(trackId, true)}>Try again</button>
    </div>
  {:else if lyr.lines.length === 0}
    <div class="msg"><p>Instrumental</p></div>
  {:else}
    <div class="lines">
      {#each lyr.lines as line, i (i)}
        <div
          class="line"
          class:bg={line.background}
          class:active={synced && i === active}
          class:past={synced && i < active}
          class:gap={!line.text}
          class:words={line.words.length > 0}
          style={synced ? style(i) : ""}
          bind:this={lineEls[i]}
          role="button"
          tabindex="-1"
          onclick={() => seek(i)}
          onkeydown={(e) => e.key === "Enter" && seek(i)}
        >
          {#if !line.text}
            <span class="dots" class:run={i === active}><i></i><i></i><i></i></span>
          {:else if line.words.length && synced}
            {#each line.words as w, k (k)}{@const { core, tail } = split(w.text)}{@const h = held[i]?.[k]}{#if h?.letters}<span class="w held split">{#each h.letters as ch, j (j)}<span class="c">{ch}</span>{/each}</span>{:else if h}<span class="w held">{core}</span>{:else}<span class="w">{core}</span>{/if}{tail}{/each}
          {:else}
            {line.text}
          {/if}
        </div>
      {/each}
      <div class="spacer"></div>
    </div>
  {/if}
</div>
  {#if lyr && lyr.lines.length}
    <div class="foot">
      Lyrics from {lyr.source}{lyr.synced === "none" ? " · not time-synced" : ""}
    </div>
  {/if}
</div>

<style>
  .wrap {
    position: relative;
    height: 100%;
  }
  .lyrics {
    position: relative;
    height: 100%;
    overflow: hidden;
    color: #fff;
    -webkit-mask-image: linear-gradient(transparent 0, #000 12%, #000 80%, transparent 100%);
    mask-image: linear-gradient(transparent 0, #000 12%, #000 80%, transparent 100%);
  }
  .lyrics.synced {
    touch-action: none;
  }
  .lyrics:not(.synced) {
    overflow-y: auto;
    padding-top: 10vh;
  }
  .lines {
    padding-right: 8%;
  }
  .line {
    font-size: clamp(24px, 2.6vw, 38px);
    font-weight: 700;
    line-height: 1.22;
    letter-spacing: -0.01em;
    padding: 10px 14px;
    margin-left: -14px;
    border-radius: 14px;
    color: rgba(255, 255, 255, 0.36);
    transition:
      transform 0.75s cubic-bezier(0.2, 0.9, 0.25, 1),
      color 0.45s var(--ease),
      filter 0.45s var(--ease),
      opacity 0.45s var(--ease);
    will-change: transform;
    cursor: pointer;
  }
  .synced .line:hover {
    background: rgba(255, 255, 255, 0.06);
    filter: none !important;
  }
  .line.bg {
    font-size: clamp(16px, 1.6vw, 22px);
    font-weight: 600;
    color: rgba(255, 255, 255, 0.28);
    padding-top: 0;
  }
  .line.active:not(.words) {
    color: #fff;
  }
  .line.active.bg:not(.words) {
    color: rgba(255, 255, 255, 0.85);
  }
  /* Word-synced: each word fills left-to-right with --p (0..1) and rises with --lift. */
  .w,
  .c {
    display: inline-block;
    --p: 0;
  }
  .w {
    --lift: 0;
    --g: 0;
    transform: translateY(calc(var(--lift) * -0.06em));
    transition: transform 0.35s var(--ease);
  }
  .line.active .w:not(.split),
  .line.active .c {
    /* Soft leading edge: the gradient runs from -20% to 120% so 0 and 1 are fully dim / lit. */
    background: linear-gradient(90deg, #fff calc(var(--p) * 140% - 20%), rgba(255, 255, 255, 0.36) calc(var(--p) * 140%));
    -webkit-background-clip: text;
    background-clip: text;
    color: transparent;
  }
  /* Held notes: the word swells and glows while the note is sustained, letters ripple upward. */
  .line.active .w.held {
    transform: translateY(calc(var(--lift) * -0.06em)) scale(calc(1 + var(--g) * 0.07));
    filter: drop-shadow(0 0 calc(var(--g) * 14px) rgba(255, 255, 255, calc(var(--g) * 0.55)));
    transition: none;
  }
  .line.active .c {
    --wave: 0;
    transform: translateY(calc(var(--wave) * -0.08em));
  }
  /* Sung lines leave; scrolling by hand brings them back. */
  .line.past {
    opacity: 0;
    pointer-events: none;
  }
  .manual .line.past {
    opacity: 0.5;
    pointer-events: auto;
  }
  .lyrics:not(.synced) .line {
    color: rgba(255, 255, 255, 0.85);
    cursor: default;
    font-size: clamp(20px, 2vw, 28px);
  }
  .spacer {
    height: 60vh;
  }
  .dots {
    display: inline-flex;
    gap: 8px;
    padding: 8px 0;
  }
  .dots i {
    width: 10px;
    height: 10px;
    border-radius: 50%;
    background: rgba(255, 255, 255, 0.5);
  }
  .dots.run i,
  .msg .dots i {
    animation: breathe 1.4s ease-in-out infinite;
  }
  .dots i:nth-child(2) {
    animation-delay: 0.2s;
  }
  .dots i:nth-child(3) {
    animation-delay: 0.4s;
  }
  @keyframes breathe {
    0%,
    100% {
      transform: scale(0.75);
      opacity: 0.4;
    }
    50% {
      transform: scale(1.1);
      opacity: 1;
    }
  }
  .msg {
    height: 100%;
    display: grid;
    place-content: center;
    justify-items: center;
    gap: 12px;
    font-size: 22px;
    font-weight: 600;
    color: rgba(255, 255, 255, 0.6);
  }
  .msg p {
    margin: 0;
  }
  .retry {
    font-size: 13px;
    padding: 6px 14px;
    border-radius: 999px;
    background: rgba(255, 255, 255, 0.14);
    color: #fff;
  }
  .foot {
    position: absolute;
    left: 0;
    bottom: 2.5%;
    font-size: 12px;
    letter-spacing: 0.04em;
    color: rgba(255, 255, 255, 0.55);
    pointer-events: none;
  }
</style>
