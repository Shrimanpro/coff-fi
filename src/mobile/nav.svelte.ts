// Navigation for the iOS layout: four tabs, each with its own push/pop stack (like a
// UINavigationController per tab), plus the bottom action sheet that replaces right-click menus.

import { haptic } from "../lib/haptics";
import { api } from "../lib/ipc";
import { remote } from "../lib/remote.svelte";
import { enqueueTile, enqueueTracks, onTargetChange, playTile, player, shufflePlay, tileTracks, toast, ui } from "../lib/state.svelte";
import type { Tile, TileKind, Track } from "../lib/types";

export type TabId = "home" | "library" | "playlists" | "search";

export type Page =
  | { type: "collection"; kind: TileKind; id: string; focus?: number }
  | { type: "list"; which: "albums" | "artists" | "songs" | "folders" }
  | { type: "folder"; path: string };

export interface SheetItem {
  label: string;
  icon?: string;
  danger?: boolean;
  action: () => void;
}

/** iOS-style context menu: the pressed item lifts out of a blurred backdrop with a menu beside it. */
export interface ContextMenu {
  rect: DOMRect;
  preview: { covers: (string | null)[] | string | null; round?: boolean; title: string; subtitle?: string; row?: boolean };
  items: SheetItem[];
}

export const m = $state({
  tab: "home" as TabId,
  stacks: { home: [], library: [], playlists: [], search: [] } as Record<TabId, Page[]>,
  /** Slide direction of the last navigation, for the page transition. */
  dir: 1,
  nowPlaying: false,
  npMode: "art" as "art" | "lyrics" | "queue",
  sheet: null as { title?: string; subtitle?: string; items: SheetItem[] } | null,
  ctx: null as ContextMenu | null,
  /** Tab bar collapsed to a single tab beside the mini player (scrolled down while playing). */
  collapsed: false,
});

// Pages of one device's library make no sense on another's.
onTargetChange.push(() => {
  m.stacks = { home: [], library: [], playlists: [], search: [] };
  m.dir = 0;
});

export const top = (): Page | null => {
  const s = m.stacks[m.tab];
  return s.length ? s[s.length - 1] : null;
};

export function push(p: Page) {
  m.dir = 1;
  m.stacks[m.tab] = [...m.stacks[m.tab], p];
}

export function pop(): boolean {
  const s = m.stacks[m.tab];
  if (!s.length) return false;
  m.dir = -1;
  m.stacks[m.tab] = s.slice(0, -1);
  return true;
}

/** Tapping the active tab again pops to its root (like iOS). */
export function selectTab(t: TabId) {
  haptic("selection");
  if (m.tab === t) {
    m.dir = -1;
    m.stacks[t] = [];
  } else {
    m.dir = 0;
    m.tab = t;
  }
  m.collapsed = false;
}

export function openTile(t: Tile) {
  if (t.kind === "folder") push({ type: "folder", path: t.id });
  else push({ type: "collection", kind: t.kind, id: t.id });
}

const albumId = (t: Track) => `${t.album_artist}\u001f${t.album}`;

/** The layer holding pages + tab bar; Now Playing scales it back like an iOS card stack. */
export const stage = { el: null as HTMLElement | null };

/** Set by the Now Playing sheet so other code can close it with its animation. */
export const np: { close: () => void } = { close: () => void (m.nowPlaying = false) };

export function goAlbum(t: Track, focus = false) {
  np.close();
  if (t.album) push({ type: "collection", kind: "album", id: albumId(t), focus: focus ? t.id : undefined });
  else goFolder(t);
}

export function goArtist(t: Track) {
  np.close();
  push({ type: "collection", kind: "artist", id: t.artist });
}

export function goFolder(t: Track) {
  np.close();
  push({ type: "folder", path: t.dir });
}

export function openSheet(items: SheetItem[], title?: string, subtitle?: string) {
  m.sheet = { items, title, subtitle };
}

export async function shuffleTile(t: Tile) {
  shufflePlay(await tileTracks(t), t.name);
}

function tileItems(t: Tile): SheetItem[] {
  const items: SheetItem[] = [
    { label: "Play", icon: "play", action: () => playTile(t) },
    { label: "Shuffle", icon: "shuffle", action: () => shuffleTile(t) },
    { label: "Play Next", icon: "next-up", action: () => enqueueTile(t, true) },
    { label: "Add to Queue", icon: "to-queue", action: () => enqueueTile(t, false) },
  ];
  if (t.kind !== "artist") items.push({ label: "Open", icon: "chev-right", action: () => openTile(t) });
  return items;
}

export function tileSheet(t: Tile) {
  openSheet(tileItems(t), t.name, t.subtitle);
}

/** Long-press on an album / playlist / artist / folder tile. */
export function tileMenu(t: Tile, el: HTMLElement) {
  m.ctx = { rect: el.getBoundingClientRect(), preview: { covers: t.covers, round: t.kind === "artist", title: t.name, subtitle: t.subtitle }, items: tileItems(t) };
}

/** Everything the desktop right-click menu on a song offers, plus song info for local files. */
function trackItems(t: Track, extra: SheetItem[] = []): SheetItem[] {
  const items: SheetItem[] = [
    { label: "Play next", icon: "next-up", action: () => enqueueTracks([t], true) },
    { label: "Add to queue", icon: "to-queue", action: () => enqueueTracks([t], false) },
    ...extra,
  ];
  if (t.album) items.push({ label: "Go to album", icon: "album", action: () => goAlbum(t, true) });
  items.push({ label: "Go to artist", icon: "artist", action: () => goArtist(t) });
  items.push({ label: "Open folder", icon: "folder", action: () => goFolder(t) });
  if (!t.path.startsWith("navidrome://") && !remote.target) items.push({ label: "View / edit info", icon: "info", action: () => (ui.info = t) });
  return items;
}

export function trackSheet(t: Track, extra: SheetItem[] = []) {
  openSheet(trackItems(t, extra), t.title, t.artist);
}

/** Long-press on a song row. */
export function trackMenu(t: Track, el: HTMLElement, onplay?: () => void) {
  const extra: SheetItem[] = onplay ? [{ label: "Play", icon: "play", action: onplay }] : [];
  m.ctx = {
    rect: el.getBoundingClientRect(),
    preview: { covers: t.cover, title: t.title, subtitle: [t.artist, t.album].filter(Boolean).join(" · "), row: true },
    items: [...extra, ...trackItems(t)],
  };
}

/** Favorite toggle with immediate feedback. */
export async function toggleFav(t: Track) {
  try {
    const on = await api.toggleFavorite(t.id);
    if (player.track?.id === t.id) player.fav = on;
    toast(on ? `Added “${t.title}” to Favorites` : `Removed “${t.title}” from Favorites`);
  } catch (e) {
    toast(String(e));
  }
}

const PRESS_MS = 300;

/**
 * Long-press: while held the element sinks slightly; after 300 ms it fires with a haptic tick.
 * Moving the finger (scrolling) or lifting early cancels. Set `data-lp-scale` on the node to
 * enable the sink effect (rows that also swipe leave it off).
 */
export function longpress(node: HTMLElement, cb: (el: HTMLElement) => void) {
  let timer: ReturnType<typeof setTimeout> | undefined;
  let sx = 0;
  let sy = 0;
  let fired = false;
  const sink = () => node.dataset.lpScale !== undefined;
  const reset = () => {
    clearTimeout(timer);
    if (sink()) {
      node.style.transition = "transform 0.45s cubic-bezier(0.34, 1.4, 0.64, 1)";
      node.style.transform = "";
    }
  };
  const start = (e: PointerEvent) => {
    if (e.pointerType === "mouse" && e.button !== 0) return;
    fired = false;
    sx = e.clientX;
    sy = e.clientY;
    clearTimeout(timer);
    if (sink()) {
      node.style.transition = `transform ${PRESS_MS}ms cubic-bezier(0.2, 0, 0.2, 1)`;
      node.style.transform = "scale(0.95)";
    }
    timer = setTimeout(() => {
      fired = true;
      haptic("medium");
      cb(node);
      reset();
    }, PRESS_MS);
  };
  const move = (e: PointerEvent) => {
    if (Math.abs(e.clientX - sx) > 8 || Math.abs(e.clientY - sy) > 8) reset();
  };
  // A long-press must not also count as a tap.
  const click = (e: MouseEvent) => {
    if (fired) {
      e.stopPropagation();
      e.preventDefault();
      fired = false;
    }
  };
  node.addEventListener("pointerdown", start);
  node.addEventListener("pointermove", move);
  node.addEventListener("pointerup", reset);
  node.addEventListener("pointercancel", reset);
  node.addEventListener("click", click, true);
  return {
    update(n: (el: HTMLElement) => void) {
      cb = n;
    },
    destroy() {
      clearTimeout(timer);
      node.removeEventListener("pointerdown", start);
      node.removeEventListener("pointermove", move);
      node.removeEventListener("pointerup", reset);
      node.removeEventListener("pointercancel", reset);
      node.removeEventListener("click", click, true);
    },
  };
}
