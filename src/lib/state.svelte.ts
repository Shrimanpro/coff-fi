import { listen } from "@tauri-apps/api/event";
import { api } from "./ipc";
import type { QueueView, Settings, Status, Tile, TileKind, Track } from "./types";

export interface MenuItem {
  label: string;
  action: () => void;
}

export const ui = $state({
  base: "home" as "home" | "directory",
  dirPath: null as string | null,
  collection: null as { kind: TileKind; id: string } | null,
  /** The centred search palette (Cmd/Ctrl+K). */
  palette: false,
  /** A song to scroll to and highlight once its album opens ("go to track"). */
  focusTrack: null as number | null,
  leftPinned: false,
  theme: (document.documentElement.dataset.theme as "dark" | "light") || (matchMedia("(prefers-color-scheme: light)").matches ? "light" : "dark"),
  scanning: false,
  libVersion: 0,
  toast: null as string | null,
  menu: null as { x: number; y: number; items: MenuItem[] } | null,
});

export const player = $state({
  status: { track_id: null, playing: false, position: 0, duration: 0 } as Status,
  track: null as Track | null,
  at: performance.now(),
});

export const queue = $state<QueueView>({ name: "", pos: 0, shuffle: false, repeat: "off", tracks: [] });

export const conf = $state({ s: null as Settings | null });

/** Smoothly interpolated playback position (status arrives on changes and every few seconds). */
export function position(): number {
  const st = player.status;
  if (!st.playing) return st.position;
  return Math.min(st.duration, st.position + (performance.now() - player.at) / 1000);
}

let toastTimer: ReturnType<typeof setTimeout> | undefined;
export function toast(msg: string) {
  ui.toast = msg;
  clearTimeout(toastTimer);
  toastTimer = setTimeout(() => (ui.toast = null), 4200);
}

export function setTheme(t: "dark" | "light") {
  ui.theme = t;
  document.documentElement.dataset.theme = t;
  try {
    localStorage.setItem("theme", t);
  } catch {}
}

export function openMenu(e: MouseEvent, items: MenuItem[]) {
  e.preventDefault();
  e.stopPropagation();
  ui.menu = { x: e.clientX, y: e.clientY, items };
}

async function refreshQueue() {
  Object.assign(queue, await api.queue());
}

/** Loads the player, queue and sleep timer of whichever device the UI is showing. */
async function syncPlayer() {
  const [st] = await Promise.all([api.status(), refreshQueue()]);
  player.status = st;
  player.at = performance.now();
  player.track = st.track_id != null ? (queue.tracks.find((t) => t.id === st.track_id) ?? null) : null;
}

function on<T>(name: string, fn: (payload: T) => void) {
  return listen<T>(name, (e) => fn(e.payload));
}
export async function init() {
  setTheme(ui.theme);
  conf.s = await api.getSettings();
  await syncPlayer();

  await Promise.all([
    on<Status>("player://state", (p) => {
      player.status = p;
      player.at = performance.now();
    }),
    on<Track | null>("player://track", (p) => {
      player.track = p;
    }),
    on("queue://changed", () => {
      refreshQueue();
    }),
    on("library://changed", () => {
      ui.libVersion++;
      refreshQueue();
    }),
    listen<boolean>("library://scanning", (e) => (ui.scanning = e.payload)),
    listen<string>("app://error", (e) => toast(e.payload)),
  ]);
}

// ---------- actions ----------

export function playTracks(tracks: Track[], start: number, name: string) {
  if (!tracks.length) return;
  api.playTracks(
    tracks.map((t) => t.id),
    start,
    name,
  ).catch((e) => toast(String(e)));
}

export async function tileTracks(t: Tile): Promise<Track[]> {
  return (await api.collection(t.kind, t.id)).tracks;
}

export async function playTile(t: Tile) {
  playTracks(await tileTracks(t), 0, t.name);
}

export async function enqueueTile(t: Tile, next: boolean) {
  const ids = (await tileTracks(t)).map((x) => x.id);
  if (ids.length) await api.enqueue(ids, next);
  toast(next ? `“${t.name}” will play next` : `Added “${t.name}” to the queue`);
}

export function enqueueTracks(tracks: Track[], next: boolean) {
  api.enqueue(
    tracks.map((t) => t.id),
    next,
  );
  const what = tracks.length === 1 ? `“${tracks[0].title}”` : `${tracks.length} songs`;
  toast(next ? `${what} will play next` : `Added ${what} to the queue`);
}

export function openTile(t: Tile) {
  if (t.kind === "folder") {
    ui.base = "directory";
    ui.dirPath = t.id;
    ui.collection = null;
  } else {
    ui.collection = { kind: t.kind, id: t.id };
  }
}

/** Opens the song's album (or its folder) and highlights the song there. */
export function goToTrack(t: Track) {
  ui.focusTrack = t.id;
  ui.collection = t.album ? { kind: "album", id: `${t.album_artist}\u001f${t.album}` } : { kind: "folder", id: t.dir };
}

export function tileMenu(e: MouseEvent, t: Tile) {
  openMenu(e, [
    { label: "Play", action: () => playTile(t) },
    { label: "Play next", action: () => enqueueTile(t, true) },
    { label: "Add to queue", action: () => enqueueTile(t, false) },
  ]);
}

export function trackMenu(e: MouseEvent, t: Track) {
  openMenu(e, [
    { label: "Play next", action: () => enqueueTracks([t], true) },
    { label: "Add to queue", action: () => enqueueTracks([t], false) },
    { label: "Go to album", action: () => (ui.collection = { kind: "album", id: `${t.album_artist}\u001f${t.album}` }) },
    { label: "Go to artist", action: () => (ui.collection = { kind: "artist", id: t.artist }) },
    {
      label: "Open folder",
      action: () => {
        ui.collection = null;
        ui.base = "directory";
        ui.dirPath = t.dir;
      },
    },
  ]);
}

/**
 * Distinguishes single from double clicks: single fires after a short delay
 * unless a second click arrives (then only `double` fires).
 */
export function clicks(node: HTMLElement, h: { single: () => void; double: () => void }) {
  let timer: ReturnType<typeof setTimeout> | undefined;
  const onClick = () => {
    if (timer) {
      clearTimeout(timer);
      timer = undefined;
      h.double();
    } else {
      timer = setTimeout(() => {
        timer = undefined;
        h.single();
      }, 220);
    }
  };
  node.addEventListener("click", onClick);
  return {
    update(n: typeof h) {
      h = n;
    },
    destroy() {
      clearTimeout(timer);
      node.removeEventListener("click", onClick);
    },
  };
}

export async function shufflePlay(tracks: Track[], name: string) {
  if (!tracks.length) return;
  if (!queue.shuffle) {
    queue.shuffle = true;
    await api.setPlayMode({ shuffle: true });
  }
  playTracks(tracks, Math.floor(Math.random() * tracks.length), name);
}

// Shuffle / repeat / volume act on the device that's playing (this one or a controlled one).

export function toggleShuffle() {
  queue.shuffle = !queue.shuffle;
  if (conf.s) conf.s.shuffle = queue.shuffle;
  api.setPlayMode({ shuffle: queue.shuffle }).catch((e) => toast(String(e)));
}

export function cycleRepeat() {
  const order = ["off", "all", "one"] as const;
  queue.repeat = order[(order.indexOf(queue.repeat) + 1) % 3];
  if (conf.s) conf.s.repeat = queue.repeat;
  api.setPlayMode({ repeat: queue.repeat }).catch((e) => toast(String(e)));
}

export function volume(): number {
  return conf.s?.volume ?? 0.8;
}

export function setVolume(v: number) {
  if (conf.s) conf.s.volume = v;
  api.setVolume(v).catch(() => {});
}
