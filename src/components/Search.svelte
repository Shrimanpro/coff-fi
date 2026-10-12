<script lang="ts">
  import { tick } from "svelte";
  import { api } from "../lib/ipc";
  import { clicks, goToTrack, openTile, playTile, playTracks, tileMenu, trackMenu, ui } from "../lib/state.svelte";
  import type { SearchResults, Tile, Track } from "../lib/types";
  import Cover from "./Cover.svelte";
  import Icon from "./Icon.svelte";

  const mac = navigator.userAgent.includes("Mac");
  const EASE = "cubic-bezier(0.22, 1, 0.36, 1)";

  type Hit =
    | { type: "Song"; key: string; name: string; sub: string; covers: (string | null)[]; track: Track; index: number }
    | { type: "Album" | "Artist" | "Playlist"; key: string; name: string; sub: string; covers: string[]; tile: Tile };

  let input = $state<HTMLInputElement>();
  let field = $state<HTMLElement>();
  let pinput = $state<HTMLInputElement>();
  let pfield = $state<HTMLElement>();
  let pbox = $state<HTMLElement>();
  let query = $state("");
  let focused = $state(false);
  let res = $state<SearchResults | null>(null);
  let sel = $state(0);
  let seq = 0;
  let timer: ReturnType<typeof setTimeout> | undefined;

  $effect(() => {
    const q = query.trim();
    ui.libVersion;
    clearTimeout(timer);
    if (!q) {
      res = null;
      return;
    }
    const mine = ++seq;
    timer = setTimeout(() => {
      api.search(q).then((r) => {
        if (mine === seq) {
          res = r;
          sel = 0;
        }
      });
    }, 110);
  });

  // Grouped by type: playlists, albums, songs, artists. Within a type the backend ranks matches.
  const hits = $derived.by((): Hit[] => {
    if (!res) return [];
    const tile = (type: "Album" | "Artist" | "Playlist", t: Tile): Hit => ({
      type,
      key: `${type}:${t.id}`,
      name: t.name,
      sub: type === "Album" ? t.subtitle || "Album" : type === "Artist" ? `${t.count} ${t.count === 1 ? "song" : "songs"}` : t.subtitle,
      covers: t.covers,
      tile: t,
    });
    return [
      ...res.playlists.slice(0, 5).map((t) => tile("Playlist", t)),
      ...res.albums.slice(0, 6).map((t) => tile("Album", t)),
      ...res.songs.slice(0, 12).map(
        (t, i): Hit => ({ type: "Song", key: `Song:${t.id}`, name: t.title, sub: [t.artist, t.album].filter(Boolean).join(" · "), covers: [t.cover], track: t, index: i }),
      ),
      ...res.artists.slice(0, 5).map((t) => tile("Artist", t)),
    ];
  });

  const dropdown = $derived(focused && !ui.palette && query.trim().length > 0);

  // ---- centred palette (opened with the keyboard shortcut) ----

  /** Transform that maps the palette's search field onto the top-bar field (for the fly-out). */
  function fromField(): Keyframe | null {
    if (!field || !pfield) return null;
    const a = field.getBoundingClientRect();
    const b = pfield.getBoundingClientRect();
    if (!b.width || !b.height) return null;
    return { transform: `translate(${a.left - b.left}px, ${a.top - b.top}px) scale(${a.width / b.width}, ${a.height / b.height})`, opacity: 0.4 };
  }

  async function openPalette() {
    input?.blur();
    if (ui.palette) {
      pinput?.select();
      return;
    }
    ui.palette = true;
    await tick();
    pinput?.focus();
    pinput?.select();
    const k = fromField();
    if (k) pfield?.animate([k, { transform: "none", opacity: 1 }], { duration: 420, easing: EASE });
    pbox?.animate([{ opacity: 0 }, { opacity: 1 }], { duration: 260, easing: EASE });
  }

  let closing = false;
  async function closePalette() {
    if (!ui.palette || closing) return;
    closing = true;
    const k = fromField();
    const anims = [pbox?.animate([{ opacity: 1 }, { opacity: 0 }], { duration: 200, easing: EASE, fill: "forwards" })];
    if (k) anims.push(pfield?.animate([{ transform: "none", opacity: 1 }, k], { duration: 320, easing: EASE, fill: "forwards" }));
    await Promise.all(anims.map((a) => a?.finished.catch(() => {})));
    ui.palette = false;
    closing = false;
  }

  function done() {
    query = "";
    input?.blur();
    if (ui.palette) closePalette();
  }

  // ---- actions ----

  /** Enter / double click: play it. */
  function play(h: Hit) {
    if (h.type === "Song") playTracks(res?.songs ?? [h.track], h.index, `Search: ${query.trim()}`);
    else playTile(h.tile);
    done();
  }

  /** Shift+Enter / single click: open the playlist, album or artist, or go to the song. */
  function openHit(h: Hit) {
    if (h.type === "Song") goToTrack(h.track);
    else openTile(h.tile);
    done();
  }

  function menu(e: MouseEvent, h: Hit) {
    if (h.type === "Song") trackMenu(e, h.track);
    else tileMenu(e, h.tile);
  }

  function onKey(e: KeyboardEvent) {
    if (e.key === "ArrowDown" || e.key === "ArrowUp") {
      e.preventDefault();
      if (hits.length) sel = (sel + (e.key === "ArrowDown" ? 1 : -1) + hits.length) % hits.length;
      document.querySelector(`[data-hit="${sel}"]`)?.scrollIntoView({ block: "nearest" });
    } else if (e.key === "Enter" && hits[sel]) {
      e.preventDefault();
      if (e.shiftKey) openHit(hits[sel]);
      else play(hits[sel]);
    } else if (e.key === "Escape") {
      e.preventDefault();
      e.stopPropagation();
      if (ui.palette) closePalette();
      else if (query) query = "";
      else input?.blur();
    }
  }

  // Cmd+K on macOS, Ctrl+K elsewhere — works from anywhere in the app.
  function onGlobalKey(e: KeyboardEvent) {
    if ((mac ? e.metaKey : e.ctrlKey) && !e.altKey && !e.shiftKey && e.key.toLowerCase() === "k") {
      e.preventDefault();
      openPalette();
    }
  }
</script>

<svelte:window onkeydown={onGlobalKey} />

{#snippet results()}
  {#if res && hits.length === 0}
    <p class="none">Nothing matches “{query.trim()}”.</p>
  {/if}
  {#each hits as h, i (h.key)}
    <div
      data-hit={i}
      class="hit"
      class:sel={i === sel}
      role="option"
      aria-selected={i === sel}
      tabindex="-1"
      onmouseenter={() => (sel = i)}
      oncontextmenu={(e) => menu(e, h)}
      use:clicks={{ single: () => openHit(h), double: () => play(h) }}
    >
      <div class="thumb" class:round={h.type === "Artist"}>
        <Cover covers={h.covers} size={96} radius={h.type === "Artist" ? 999 : 6} round={h.type === "Artist"} />
      </div>
      <div class="text">
        <div class="name ellipsis">{h.name}</div>
        <div class="sub"><span class="type t-{h.type.toLowerCase()}">{h.type}</span><span class="ellipsis">{h.sub}</span></div>
      </div>
    </div>
  {/each}
{/snippet}

<div class="search" class:focused class:hidden={ui.palette} data-tauri-drag-region="false">
  <label class="field glass" bind:this={field}>
    <Icon name="search" size={16} />
    <input
      bind:this={input}
      bind:value={query}
      type="text"
      placeholder="Search"
      spellcheck="false"
      autocomplete="off"
      aria-label="Search songs, albums, artists and playlists"
      onfocus={() => (focused = true)}
      onblur={() => (focused = false)}
      onkeydown={onKey}
    />
    {#if !query && !focused}<kbd>{mac ? "⌘" : "Ctrl"} K</kbd>{/if}
  </label>

  {#if dropdown}
    <!-- mousedown keeps focus in the input so the panel stays open while clicking -->
    <div class="panel" role="listbox" tabindex="-1" onmousedown={(e) => e.preventDefault()}>
      {@render results()}
    </div>
  {/if}
</div>

{#if ui.palette}
  <div class="scrim" role="presentation" onclick={closePalette}></div>
  <div class="palette" role="dialog" aria-label="Search">
    <label class="pfield" bind:this={pfield}>
      <Icon name="search" size={20} />
      <input
        bind:this={pinput}
        bind:value={query}
        type="text"
        placeholder="Search playlists, albums, songs and artists"
        spellcheck="false"
        autocomplete="off"
        onkeydown={onKey}
      />
    </label>
    <div class="pbox" bind:this={pbox} role="listbox" tabindex="-1" onmousedown={(e) => e.preventDefault()}>
      {#if query.trim()}
        {@render results()}
      {/if}
      <div class="keys">
        <span><kbd>↵</kbd> Play</span>
        <span><kbd>⇧</kbd><kbd>↵</kbd> Open</span>
        <span><kbd>↑</kbd><kbd>↓</kbd> Move</span>
        <span><kbd>Esc</kbd> Close</span>
      </div>
    </div>
  </div>
{/if}

<style>
  .search {
    position: relative;
  }
  .search.hidden .field {
    visibility: hidden;
  }
  .field {
    display: flex;
    align-items: center;
    gap: 8px;
    height: 42px;
    width: 200px;
    padding: 0 12px 0 14px;
    border-radius: 999px;
    color: var(--text-2);
    cursor: text;
    transition: width 0.4s var(--spring);
  }
  .focused .field {
    width: 300px;
    color: var(--text);
  }
  /* Narrow windows: keep clear of the Home | Directory switch in the middle. */
  @media (max-width: 1100px) {
    .field {
      width: 140px;
    }
    .focused .field {
      width: 220px;
    }
    kbd {
      display: none;
    }
  }
  input {
    flex: 1;
    min-width: 0;
    border: 0;
    outline: none;
    background: none;
    color: var(--text);
    font: inherit;
    font-size: 14px;
  }
  input::placeholder {
    color: var(--text-3);
  }
  kbd {
    font-family: inherit;
    font-size: 11px;
    font-weight: 600;
    color: var(--text-3);
    padding: 2px 6px;
    border-radius: 6px;
    border: 1px solid var(--rule);
    white-space: nowrap;
  }
  .panel {
    position: absolute;
    top: calc(100% + 8px);
    right: 0;
    width: min(440px, calc(100vw - 32px));
    max-height: min(70vh, 620px);
    overflow-y: auto;
    padding: 6px;
    border-radius: 18px;
    background: var(--glass-strong);
    backdrop-filter: blur(24px) saturate(160%);
    -webkit-backdrop-filter: blur(24px) saturate(160%);
    border: 1px solid var(--glass-border);
    box-shadow: inset 0 1px 0 var(--glass-hi), var(--shadow);
    animation: drop 0.22s var(--ease);
  }
  @keyframes drop {
    from {
      opacity: 0;
      transform: translateY(-6px);
    }
  }
  .hit {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 7px 10px 7px 7px;
    border-radius: 12px;
    cursor: pointer;
  }
  .hit.sel {
    background: var(--hover);
  }
  .thumb {
    width: 44px;
    flex: none;
    border-radius: 6px;
    overflow: hidden;
  }
  .thumb.round {
    border-radius: 50%;
  }
  .text {
    min-width: 0;
    flex: 1;
  }
  .name {
    font-weight: 600;
    font-size: 14px;
  }
  .sub {
    display: flex;
    align-items: center;
    gap: 8px;
    margin-top: 2px;
    font-size: 12.5px;
    color: var(--text-3);
    min-width: 0;
  }
  .type {
    flex: none;
    font-size: 10.5px;
    font-weight: 700;
    letter-spacing: 0.06em;
    text-transform: uppercase;
    padding: 1px 7px;
    border-radius: 999px;
    color: var(--text-2);
    background: var(--hover);
  }
  .t-song {
    color: var(--accent);
    background: color-mix(in srgb, var(--accent) 16%, transparent);
  }
  .scrim {
    position: fixed;
    inset: 0;
    z-index: 90;
    background: rgba(10, 6, 3, 0.32);
    backdrop-filter: blur(3px);
    -webkit-backdrop-filter: blur(3px);
    animation: fade 0.25s var(--ease);
  }
  @keyframes fade {
    from {
      opacity: 0;
    }
  }
  .palette {
    position: fixed;
    z-index: 91;
    top: 16vh;
    left: 50%;
    width: min(640px, calc(100vw - 32px));
    transform: translateX(-50%);
  }
  .pfield {
    display: flex;
    align-items: center;
    gap: 12px;
    height: 58px;
    padding: 0 20px;
    border-radius: 999px;
    color: var(--text-2);
    background: var(--glass-strong);
    backdrop-filter: blur(24px) saturate(160%);
    -webkit-backdrop-filter: blur(24px) saturate(160%);
    border: 1px solid var(--glass-border);
    box-shadow: inset 0 1px 0 var(--glass-hi), var(--shadow);
    transform-origin: 0 0;
  }
  .pfield input {
    font-size: 18px;
  }
  .pbox {
    margin-top: 10px;
    max-height: min(60vh, 560px);
    overflow-y: auto;
    padding: 6px;
    border-radius: 20px;
    background: var(--glass-strong);
    backdrop-filter: blur(24px) saturate(160%);
    -webkit-backdrop-filter: blur(24px) saturate(160%);
    border: 1px solid var(--glass-border);
    box-shadow: inset 0 1px 0 var(--glass-hi), var(--shadow);
  }
  .palette .hit {
    padding: 8px 12px 8px 8px;
  }
  .palette .thumb {
    width: 48px;
  }
  .palette .name {
    font-size: 15px;
  }
  .keys {
    display: flex;
    gap: 16px;
    flex-wrap: wrap;
    padding: 8px 10px 4px;
    font-size: 12px;
    color: var(--text-3);
  }
  .keys kbd {
    margin-right: 3px;
  }
  .none {
    margin: 0;
    padding: 18px 14px;
    color: var(--text-3);
    text-align: center;
  }
</style>
