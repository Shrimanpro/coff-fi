<script lang="ts">
  let { name, size = 18 }: { name: string; size?: number } = $props();

  // Filled glyphs (playback) vs. stroked line icons.
  const filled: Record<string, string> = {
    play: "M8 5.5v13a1 1 0 0 0 1.5.86l10.6-6.5a1 1 0 0 0 0-1.72L9.5 4.64A1 1 0 0 0 8 5.5z",
    pause: "M7 4.5h3.2a.8.8 0 0 1 .8.8v13.4a.8.8 0 0 1-.8.8H7a.8.8 0 0 1-.8-.8V5.3a.8.8 0 0 1 .8-.8zm6.8 0H17a.8.8 0 0 1 .8.8v13.4a.8.8 0 0 1-.8.8h-3.2a.8.8 0 0 1-.8-.8V5.3a.8.8 0 0 1 .8-.8z",
    next: "M3.5 6.6v10.8a.9.9 0 0 0 1.4.75L12 13.4v4a.9.9 0 0 0 1.4.75l8.1-5.4a.9.9 0 0 0 0-1.5l-8.1-5.4A.9.9 0 0 0 12 6.6v4L4.9 5.85a.9.9 0 0 0-1.4.75z",
    prev: "M20.5 6.6v10.8a.9.9 0 0 1-1.4.75L12 13.4v4a.9.9 0 0 1-1.4.75l-8.1-5.4a.9.9 0 0 1 0-1.5l8.1-5.4a.9.9 0 0 1 1.4.75v4l7.1-4.75a.9.9 0 0 1 1.4.75z",
    "star-fill": "M12 3.2l2.6 5.5 6 .8-4.4 4.1 1.1 6L12 16.7l-5.3 2.9 1.1-6-4.4-4.1 6-.8z",
  };
  const stroked: Record<string, string> = {
    star: "M12 3.2l2.6 5.5 6 .8-4.4 4.1 1.1 6L12 16.7l-5.3 2.9 1.1-6-4.4-4.1 6-.8z",
    shuffle: "M3 7h3.5c2 0 3.2 1 4.3 2.6l2.4 4.8c1.1 1.6 2.3 2.6 4.3 2.6H21M18 14l3 3-3 3M3 17h3.5c1.4 0 2.4-.5 3.2-1.3M13.3 8.3c.8-.8 1.8-1.3 3.2-1.3H21M18 4l3 3-3 3",
    repeat: "M4 11V9a3 3 0 0 1 3-3h13l-3-3M20 13v2a3 3 0 0 1-3 3H4l3 3",
    lyrics: "M7 9.5h10M7 13h6M5.5 4h13A2.5 2.5 0 0 1 21 6.5v8a2.5 2.5 0 0 1-2.5 2.5H11l-4.5 3.5V17h-1A2.5 2.5 0 0 1 3 14.5v-8A2.5 2.5 0 0 1 5.5 4z",
    queue: "M9 6h12M9 12h12M9 18h12M4 6h.01M4 12h.01M4 18h.01",
    volume: "M4 9.5v5h3.5L12 18.5v-13L7.5 9.5zM15.5 9a4 4 0 0 1 0 6M18 6.5a7.5 7.5 0 0 1 0 11",
    mute: "M4 9.5v5h3.5L12 18.5v-13L7.5 9.5zM16 9.5l5 5M21 9.5l-5 5",
    more: "M5.5 12h.01M12 12h.01M18.5 12h.01",
    gear: "M12 15.2a3.2 3.2 0 1 0 0-6.4 3.2 3.2 0 0 0 0 6.4zM19.4 13.5l1.6 1.2-1.8 3.1-1.9-.7a7 7 0 0 1-1.9 1.1l-.3 2h-3.6l-.3-2a7 7 0 0 1-1.9-1.1l-1.9.7-1.8-3.1 1.6-1.2a7 7 0 0 1 0-2.9L3 9.3l1.8-3.1 1.9.7a7 7 0 0 1 1.9-1.1l.3-2h3.6l.3 2a7 7 0 0 1 1.9 1.1l1.9-.7L21 9.3l-1.6 1.2a7 7 0 0 1 0 3z",
    sun: "M12 16.5a4.5 4.5 0 1 0 0-9 4.5 4.5 0 0 0 0 9zM12 2.5v2M12 19.5v2M4.6 4.6 6 6M18 18l1.4 1.4M2.5 12h2M19.5 12h2M4.6 19.4 6 18M18 6l1.4-1.4",
    moon: "M20 14.5A8 8 0 0 1 9.5 4a8 8 0 1 0 10.5 10.5z",
    "chev-down": "M6 9l6 6 6-6",
    "chev-right": "M9 6l6 6-6 6",
    "chev-left": "M15 6l-6 6 6 6",
    folder: "M3.5 7.5A2 2 0 0 1 5.5 5.5h4l2 2h7a2 2 0 0 1 2 2v7.5a2 2 0 0 1-2 2h-13a2 2 0 0 1-2-2z",
    music: "M9 18V6l11-2v12M9 18a3 3 0 1 1-6 0 3 3 0 0 1 6 0zM20 16a3 3 0 1 1-6 0 3 3 0 0 1 6 0z",
    close: "M6 6l12 12M18 6L6 18",
    plus: "M12 5v14M5 12h14",
    save: "M5 4h11l3 3v13H5zM8 4v5h7V4M8 20v-6h8v6",
    history: "M3.5 12a8.5 8.5 0 1 0 2.5-6M3.5 4v4h4M12 7.5V12l3 2",
    refresh: "M20 12a8 8 0 1 1-2.3-5.6M20 4v4.5h-4.5",
    trash: "M4.5 7h15M10 11v6M14 11v6M6 7l1 13h10l1-13M9 7V4h6v3",
    pencil: "M4 20h4L19.5 8.5a2.1 2.1 0 0 0-4-4L4 16v4zM14 6l4 4",
    grip: "M9 6h.01M15 6h.01M9 12h.01M15 12h.01M9 18h.01M15 18h.01",
    search: "M11 18a7 7 0 1 0 0-14 7 7 0 0 0 0 14zM20 20l-4-4",
    home: "M4 10.5 12 4l8 6.5V19a1.5 1.5 0 0 1-1.5 1.5H15v-6H9v6H5.5A1.5 1.5 0 0 1 4 19z",
    library: "M5 4v16M9.5 4v16M14 5l4.5 14.5",
    playlist: "M4 6h11M4 11h11M4 16h6M16 18.5V10l4.5-1.2M16 18.5a2 2 0 1 1-4 0 2 2 0 0 1 4 0z",
    airplay: "M6.5 17H5a2 2 0 0 1-2-2V6a2 2 0 0 1 2-2h14a2 2 0 0 1 2 2v9a2 2 0 0 1-2 2h-1.5M12 15l5 6H7z",
    sleep: "M19.5 14.5A7.5 7.5 0 0 1 9.5 4.5a7.5 7.5 0 1 0 10 10zM15 3.5h4l-4 4h4",
    artist: "M12 13a4 4 0 1 0 0-8 4 4 0 0 0 0 8zM4.5 20.5a7.5 7.5 0 0 1 15 0",
    album: "M12 21a9 9 0 1 0 0-18 9 9 0 0 0 0 18zM12 14.5a2.5 2.5 0 1 0 0-5 2.5 2.5 0 0 0 0 5z",
    check: "M5 12.5l4.5 4.5L19 7.5",
    info: "M12 21a9 9 0 1 0 0-18 9 9 0 0 0 0 18zM12 11v5.5M12 7.5h.01",
    "next-up": "M5 6h14M5 12h14M5 18h8M17 15v6M14 18h6",
    "to-queue": "M5 6h14M5 12h14M5 18h8M17 15l3 3-3 3",
    devices: "M4 15.5V6a1 1 0 0 1 1-1h11a1 1 0 0 1 1 1v1M2 18.5h10M15 9h5a1 1 0 0 1 1 1v9a1 1 0 0 1-1 1h-5a1 1 0 0 1-1-1v-9a1 1 0 0 1 1-1zM17.5 17.5h.01",
    laptop: "M5 16V7a1 1 0 0 1 1-1h12a1 1 0 0 1 1 1v9M3 19h18",
    phone: "M8 3h8a1 1 0 0 1 1 1v16a1 1 0 0 1-1 1H8a1 1 0 0 1-1-1V4a1 1 0 0 1 1-1zM11 18h2",
  };
</script>

<svg width={size} height={size} viewBox="0 0 24 24" aria-hidden="true">
  {#if filled[name]}
    <path d={filled[name]} fill="currentColor" />
  {:else if name === "repeat-one"}
    <path d={stroked.repeat} fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round" />
    <text x="12" y="14.6" text-anchor="middle" font-size="7.5" font-weight="700" fill="currentColor" font-family="Inter Variable, sans-serif">1</text>
  {:else}
    <path d={stroked[name] ?? ""} fill="none" stroke="currentColor" stroke-width={["more", "queue", "grip"].includes(name) ? 2.6 : 1.8} stroke-linecap="round" stroke-linejoin="round" />
  {/if}
</svg>
