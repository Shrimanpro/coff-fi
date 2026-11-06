import { listen } from "@tauri-apps/api/event";
import { api } from "./ipc";
import { remote, remoteApi } from "./remote.svelte";
import type { Device, PlayMode, QueueView, Settings, SleepState, Status, Tile, TileKind, Track } from "./types";

export interface MenuItem {
  label: string;
  action: () => void;
}

export const ui = $state({
  /** "ios" switches to the touch layout (src/mobile). */
  platform: (document.documentElement.dataset.platform ?? "desktop") as string,
  base: "home" as "home" | "directory",
  dirPath: null as string | null,
  collection: null as { kind: TileKind; id: string } | null,
  fullscreen: false,
  settingsOpen: false,
  settingsTab: null as string | null,
  /** Local song whose info/tag editor is open. */
  info: null as Track | null,
  /** The centred search palette (Cmd/Ctrl+K). */
  palette: false,
  /** A song to scroll to and highlight once its album opens ("go to track"). */
  focusTrack: null as number | null,
  /** Switching between local files and Navidrome (connection check in flight). */
  switching: false,
  leftPinned: false,
  theme: (document.documentElement.dataset.theme as "dark" | "light") || (matchMedia("(prefers-color-scheme: light)").matches ? "light" : "dark"),
  scanning: false,
  libVersion: 0,
  statsVersion: 0,
  toast: null as string | null,
  menu: null as { x: number; y: number; items: MenuItem[] } | null,
});

export const player = $state({
  status: { track_id: null, playing: false, position: 0, duration: 0 } as Status,
  track: null as Track | null,
  at: performance.now(),
  fav: false,
  /** The playing song comes from the on-device queue cache (iOS). */
  cached: false,
  sleep: { mode: "off", until: null } as SleepState,
});

export const queue = $state<QueueView>({ name: "", pos: 0, shuffle: false, repeat: "off", tracks: [] });

export const conf = $state({ s: null as Settings | null });

// ---------- back navigation ----------

interface Loc {
  base: "home" | "directory";
  dirPath: string | null;
  collection: { kind: TileKind; id: string } | null;
}

const here = (): Loc => ({ base: ui.base, dirPath: ui.dirPath, collection: ui.collection && { ...ui.collection } });
const sameLoc = (a: Loc, b: Loc) =>
  a.base === b.base && a.dirPath === b.dirPath && a.collection?.kind === b.collection?.kind && a.collection?.id === b.collection?.id;

export const nav = $state({ canGoBack: false });
const backStack: Loc[] = [];
let current = here();
let restoring = false;

// Every page change (however it happened) becomes a history entry.
$effect.root(() => {
  $effect(() => {
    const loc = here();
    if (sameLoc(loc, current)) return;
    // Directory view picks the only library folder by itself; don't make that a step to go back to.
    const autoPick = loc.base === "directory" && current.base === "directory" && current.dirPath === null && !current.collection && !loc.collection;
    if (!restoring && !autoPick) backStack.push(current);
    if (backStack.length > 100) backStack.shift();
    restoring = false;
    current = loc;
    nav.canGoBack = backStack.length > 0;
  });
});

/** Returns to the previous page. False if there is nowhere to go back to. */
export function goBack(): boolean {
  const prev = backStack.pop();
  nav.canGoBack = backStack.length > 0;
  if (!prev) return false;
  restoring = !sameLoc(prev, here());
  ui.base = prev.base;
  ui.dirPath = prev.dirPath;
  ui.collection = prev.collection;
  return true;
}

function clearHistory() {
  backStack.length = 0;
  current = here();
  nav.canGoBack = false;
}

/** Smoothly interpolated playback position (status arrives on changes and every few seconds). */
export function position(): number {
  const st = player.status;
  if (!st.playing) return st.position;
  const speed = conf.s?.speed ?? 1;
  return Math.min(st.duration, st.position + ((performance.now() - player.at) / 1000) * speed);
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

let saveTimer: ReturnType<typeof setTimeout> | undefined;
/** Persists settings (debounced so sliders stay smooth). */
export function saveSettings(delay = 120) {
  clearTimeout(saveTimer);
  saveTimer = setTimeout(() => {
    if (conf.s) api.setSettings($state.snapshot(conf.s) as Settings).catch((e) => toast(String(e)));
  }, delay);
}

async function refreshQueue() {
  Object.assign(queue, await api.queue());
}

async function refreshFav() {
  player.fav = player.track ? await api.isFavorite(player.track.id) : false;
}

async function refreshCached() {
  player.cached = ui.platform === "ios" && player.track ? await api.isCached(player.track.id).catch(() => false) : false;
}

/** Loads the player, queue and sleep timer of whichever device the UI is showing. */
async function syncPlayer() {
  const [st] = await Promise.all([api.status(), refreshQueue()]);
  player.status = st;
  player.at = performance.now();
  player.track = st.track_id != null ? (queue.tracks.find((t) => t.id === st.track_id) ?? null) : null;
  refreshFav();
  refreshCached();
  if (ui.platform === "ios" || remote.target) player.sleep = await api.getSleep().catch(() => player.sleep);
  if (remote.target) {
    const info = await api.remoteInfo().catch(() => null);
    if (info) remote.mode = { volume: info.volume, shuffle: info.shuffle, repeat: info.repeat };
  }
}

/** Run when the UI switches between this device and a controlled one (e.g. to reset navigation). */
export const onTargetChange: (() => void)[] = [];

async function setTarget(device: Device | null, reason?: string | null) {
  const changed = remote.target?.id !== device?.id;
  remote.target = device;
  if (reason) toast(reason);
  if (!changed) return;
  // The pages showed the other library.
  ui.collection = null;
  ui.dirPath = null;
  ui.info = null;
  clearHistory();
  onTargetChange.forEach((f) => f());
  ui.libVersion++;
  ui.statsVersion++;
  await syncPlayer().catch((e) => toast(String(e)));
}

/** Starts controlling another device (null = play here again). */
export async function controlDevice(d: Device | null, address?: string) {
  remote.error = null;
  if (!d && !address) {
    remote.picker = false;
    await remoteApi.disconnect();
    return setTarget(null);
  }
  // The picker stays open (showing "Connecting…") until it worked or failed.
  remote.connecting = d?.id ?? address ?? null;
  try {
    const dev = await remoteApi.connect(d?.id ?? null, address ?? null);
    remote.picker = false;
    await setTarget(dev);
  } catch (e) {
    remote.error = String(e);
  } finally {
    remote.connecting = null;
  }
}

/**
 * Listens to an event of the device the UI is showing: our own (`name`) normally, the
 * controlled device's (`remote:name`) while controlling one.
 */
function on<T>(name: string, fn: (payload: T) => void) {
  return Promise.all([
    listen<T>(name, (e) => {
      if (!remote.target) fn(e.payload);
    }),
    listen<T>(`remote:${name}`, (e) => {
      if (remote.target) fn(e.payload);
    }),
  ]);
}

export async function init() {
  setTheme(ui.theme);
  conf.s = await api.getSettings();
  const rs = await remoteApi.state().catch(() => null);
  if (rs) {
    remote.name = rs.name;
    remote.controllers = rs.controllers;
    remote.target = rs.target;
  }
  remote.devices = await remoteApi.devices().catch(() => []);
  await syncPlayer();

  await Promise.all([
    on<Status>("player://state", (p) => {
      player.status = p;
      player.at = performance.now();
    }),
    on<Track | null>("player://track", (p) => {
      player.track = p;
      refreshFav();
      refreshCached();
    }),
    on("queue://changed", () => {
      refreshQueue();
      // The cache fills in the background; re-check shortly after the queue moves.
      if (!remote.target) setTimeout(refreshCached, 1500);
    }),
    on<SleepState>("sleep://changed", (p) => (player.sleep = p)),
    listen("settings://changed", async () => {
      conf.s = await api.getSettings();
      remote.name = (await remoteApi.state().catch(() => null))?.name ?? remote.name;
    }),
    on<[number, boolean]>("player://favorite", (p) => {
      if (player.track?.id === p[0]) player.fav = p[1];
      ui.statsVersion++;
    }),
    on("library://changed", () => {
      ui.libVersion++;
      refreshQueue();
    }),
    listen<boolean>("library://scanning", (e) => (ui.scanning = e.payload)),
    on("stats://changed", () => {
      ui.statsVersion++;
      refreshFav();
    }),
    listen<PlayMode>("remote:player://mode", (e) => (remote.mode = e.payload)),
    listen<{ device: Device | null; reason: string | null }>("remote://target", (e) => setTarget(e.payload.device, e.payload.reason)),
    listen<Device[]>("remote://devices", (e) => (remote.devices = e.payload)),
    listen<{ names: string[]; joined: string | null; left: string | null }>("remote://controllers", (e) => {
      remote.controllers = e.payload.names;
      if (e.payload.joined) toast(`${e.payload.joined} is now controlling this device`);
      else if (e.payload.left) toast(`${e.payload.left} stopped controlling this device`);
    }),
    listen<string>("app://error", (e) => toast(e.payload)),
    listen<boolean>("source://changed", async () => {
      if (remote.target) return;
      ui.collection = null;
      ui.dirPath = null;
      // Pages from the other library can't be revisited.
      clearHistory();
      conf.s = await api.getSettings();
    }),
  ]);
}

/** Turns Navidrome mode on or off. Turning it on needs a reachable, configured server. */
export async function setSource(remote: boolean) {
  const nd = conf.s?.navidrome;
  if (remote && (!nd?.url.trim() || !nd.username.trim())) {
    toast("Set up your Navidrome server first");
    ui.settingsTab = "navidrome";
    ui.settingsOpen = true;
    return;
  }
  ui.switching = true;
  try {
    const label = await api.setSource(remote);
    toast(remote ? `Connected to ${label}` : "Back to local files");
  } catch (e) {
    toast(`Couldn't connect to Navidrome: ${e}`);
  }
  ui.switching = false;
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

/** Non-reactive DOM anchors shared between the mini and fullscreen players (FLIP morph). */
export const anchors = { miniArt: null as HTMLElement | null };

// Shuffle / repeat / volume act on the device that's playing (this one or a controlled one).

export function toggleShuffle() {
  queue.shuffle = !queue.shuffle;
  if (conf.s && !remote.target) conf.s.shuffle = queue.shuffle;
  api.setPlayMode({ shuffle: queue.shuffle }).catch((e) => toast(String(e)));
}

export function cycleRepeat() {
  const order = ["off", "all", "one"] as const;
  queue.repeat = order[(order.indexOf(queue.repeat) + 1) % 3];
  if (conf.s && !remote.target) conf.s.repeat = queue.repeat;
  api.setPlayMode({ repeat: queue.repeat }).catch((e) => toast(String(e)));
}

export function volume(): number {
  return remote.target ? remote.mode.volume : (conf.s?.volume ?? 0.8);
}

export function setVolume(v: number) {
  if (remote.target) remote.mode.volume = v;
  else if (conf.s) conf.s.volume = v;
  api.setVolume(v).catch(() => {});
}
