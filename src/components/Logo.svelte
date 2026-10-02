<script lang="ts">
  let { size = 30, live = false }: { size?: number; live?: boolean } = $props();
  const uid = $props.id();

  // Steam bars from the app icon: x, top (all rise from y = 26.5).
  const BARS: [number, number][] = [
    [23.7, 20.4],
    [27.8, 17.2],
    [32, 11.4],
    [36.1, 15.2],
    [40.2, 18.7],
  ];
  const CUP = "M18 28.3H46V34A14 14 0 0 1 18 34Z";
</script>

<!-- The app icon redrawn without its tile: a cup whose steam is an equaliser, on a record-groove saucer. -->
<svg width={size} height={size} viewBox="8 8 48 48" aria-label="Coff-fi" class="logo" class:live>
  <defs>
    <linearGradient id="{uid}-steam" x1="0" y1="11" x2="0" y2="26.5" gradientUnits="userSpaceOnUse">
      <stop offset="0" class="s-top" />
      <stop offset="1" class="s-bottom" />
    </linearGradient>
    <!-- Cuts the cup's highlight out of the bowl. -->
    <mask id="{uid}-cup" maskUnits="userSpaceOnUse" x="0" y="0" width="64" height="64">
      <rect width="64" height="64" fill="#fff" />
      <path d="M21.2 31.3V34.8A10.9 10.9 0 0 0 25.7 43" fill="none" stroke="#000" stroke-width="1.6" stroke-linecap="round" />
    </mask>
    <!-- Keeps a gap between the saucer grooves and the cup, as on the icon. -->
    <mask id="{uid}-rings" maskUnits="userSpaceOnUse" x="0" y="0" width="64" height="64">
      <rect width="64" height="64" fill="#fff" />
      <path d={CUP} fill="#000" stroke="#000" stroke-width="3.2" stroke-linejoin="round" />
    </mask>
  </defs>

  <g fill="url(#{uid}-steam)">
    {#each BARS as [x, top], i (i)}
      <rect class="steam" style:animation-delay="{-i * 0.37}s" x={x - 1.2} y={top} width="2.4" height={26.5 - top} rx="1.2" />
    {/each}
  </g>
  <g class="ink">
    <g fill="none" stroke="currentColor" stroke-width="1.3" mask="url(#{uid}-rings)">
      <ellipse cx="32" cy="48" rx="20.5" ry="4.6" />
      <ellipse cx="32" cy="48" rx="13.5" ry="2.6" />
    </g>
    <g mask="url(#{uid}-cup)" fill="currentColor">
      <path d={CUP} />
      <path d="M45 31.75h2a4.25 4.25 0 0 1 0 8.5h-3.2" fill="none" stroke="currentColor" stroke-width="2.4" />
    </g>
  </g>
</svg>

<style>
  /* Dark theme mirrors the icon (cream cup, amber steam); light theme inks it in the page's browns. */
  .logo {
    flex: none;
    --logo-ink: #f7e3c2;
    --steam-top: #e2a95c;
    --steam-bottom: #f7e3c2;
  }
  :global(:root[data-theme="light"]) .logo {
    --logo-ink: var(--text);
    --steam-top: var(--accent);
    --steam-bottom: var(--accent-2);
  }
  .ink {
    color: var(--logo-ink);
  }
  .s-top {
    stop-color: var(--steam-top);
  }
  .s-bottom {
    stop-color: var(--steam-bottom);
  }
  .steam {
    transform-box: fill-box;
    transform-origin: bottom;
  }
  /* The steam only dances while music plays, so an idle app stays idle. */
  .live .steam {
    animation: rise 1.4s ease-in-out infinite;
  }
  @keyframes rise {
    0%,
    100% {
      transform: scaleY(1);
    }
    50% {
      transform: scaleY(0.6);
    }
  }
</style>
