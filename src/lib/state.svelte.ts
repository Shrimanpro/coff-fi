import { listen } from "@tauri-apps/api/event";
import { api } from "./ipc";
import type { Settings, Status, Tile, TileKind, Track } from "./types";

export const ui = $state({
  base: "home" as "home" | "directory",
  dirPath: null as string | null,
  collection: null as { kind: TileKind; id: string } | null,
  theme: (document.documentElement.dataset.theme as "dark" | "light") || (matchMedia("(prefers-color-scheme: light)").matches ? "light" : "dark"),
  scanning: false,
  libVersion: 0,
});

export const player = $state({
  status: { track_id: null, playing: false, position: 0, duration: 0 } as Status,
  track: null as Track | null,
  at: performance.now(),
});

export const conf = $state({ s: null as Settings | null });

/** Smoothly interpolated playback position (status arrives on changes and every few seconds). */
export function position(): number {
  const st = player.status;
  if (!st.playing) return st.position;
  return Math.min(st.duration, st.position + (performance.now() - player.at) / 1000);
}

export function toast(msg: string) {
  console.error(msg);
}

export function setTheme(t: "dark" | "light") {
  ui.theme = t;
  document.documentElement.dataset.theme = t;
  try {
    localStorage.setItem("theme", t);
  } catch {}
}

/** Loads the player, queue and sleep timer of whichever device the UI is showing. */
async function syncPlayer() {
  const st = await api.status();
  player.status = st;
  player.at = performance.now();
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
    on("library://changed", () => {
      ui.libVersion++;
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

export function openTile(t: Tile) {
  if (t.kind === "folder") {
    ui.base = "directory";
    ui.dirPath = t.id;
    ui.collection = null;
  } else {
    ui.collection = { kind: t.kind, id: t.id };
  }
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

export function volume(): number {
  return conf.s?.volume ?? 0.8;
}

export function setVolume(v: number) {
  if (conf.s) conf.s.volume = v;
  api.setVolume(v).catch(() => {});
}
