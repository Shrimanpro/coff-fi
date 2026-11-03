import { convertFileSrc, invoke as tauriInvoke } from "@tauri-apps/api/core";
import { remote, remoteApi } from "./remote.svelte";
import type { Collection, FileInfo, FolderView, Home, Lyrics, Navidrome, PlayMode, QueueView, Repeat, SavedQueue, SearchResults, Settings, SleepState, Status, Tags, Tile, Track } from "./types";

/** Commands that run on the controlled device while controlling one (see remote/dispatch.rs). */
const FORWARDED = new Set([
  "get_home",
  "get_roots",
  "get_folder",
  "get_collection",
  "search",
  "get_palette",
  "get_lyrics",
  "toggle_favorite",
  "is_favorite",
  "play_tracks",
  "enqueue",
  "player_cmd",
  "get_status",
  "set_volume",
  "set_play_mode",
  "get_sleep",
  "sleep_timer",
  "get_queue",
  "queue_edit",
  "list_queues",
  "load_queue",
  "save_queue",
  "delete_queue",
]);

function invoke<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  if (remote.target && FORWARDED.has(cmd)) return remoteApi.call<T>(cmd, args);
  return tauriInvoke<T>(cmd, args);
}

export const api = {
  getSettings: () => invoke<Settings>("get_settings"),
  setSettings: (settings: Settings) => invoke<void>("set_settings", { settings }),
  setVolume: (volume: number) => invoke<void>("set_volume", { volume }),
  listDevices: () => invoke<string[]>("list_devices"),
  lastfmLogin: (username: string, password: string) => invoke<void>("lastfm_login", { username, password }),
  rescan: () => invoke<void>("rescan"),
  home: () => invoke<Home>("get_home"),
  roots: () => invoke<Tile[]>("get_roots"),
  folder: (path: string) => invoke<FolderView>("get_folder", { path }),
  collection: (kind: string, id: string) => invoke<Collection>("get_collection", { kind, id }),
  palette: (hash: string) => invoke<string[]>("get_palette", { hash }),
  toggleFavorite: (id: number) => invoke<boolean>("toggle_favorite", { id }),
  isFavorite: (id: number) => invoke<boolean>("is_favorite", { id }),
  lyrics: (id: number, refresh = false) => invoke<Lyrics | null>("get_lyrics", { id, refresh }),
  playTracks: (ids: number[], start: number, name: string) => invoke<void>("play_tracks", { ids, start, name }),
  enqueue: (ids: number[], next: boolean) => invoke<void>("enqueue", { ids, next }),
  cmd: (cmd: string, value?: number) => invoke<void>("player_cmd", { cmd, value }),
  status: () => invoke<Status>("get_status"),
  queue: () => invoke<QueueView>("get_queue"),
  queueEdit: (op: "jump" | "remove" | "move", a: number, b?: number) => invoke<void>("queue_edit", { op, a, b }),
  listQueues: () => invoke<SavedQueue[]>("list_queues"),
  loadQueue: (id: number) => invoke<void>("load_queue", { id }),
  saveQueue: (name: string) => invoke<void>("save_queue", { name }),
  deleteQueue: (id: number) => invoke<void>("delete_queue", { id }),
  navidromeTest: (server: Navidrome) => invoke<string>("navidrome_test", { server }),
  setSource: (remote: boolean) => invoke<string>("set_source", { remote }),
  search: (query: string) => invoke<SearchResults>("search", { query }),
  getTags: (id: number) => invoke<FileInfo>("get_tags", { id }),
  setTags: (id: number, tags: Tags) => invoke<Track>("set_tags", { id, tags }),
  platform: () => invoke<"ios" | "macos" | "windows" | "linux">("get_platform"),
  setPlayMode: (mode: { shuffle?: boolean; repeat?: Repeat }) => invoke<void>("set_play_mode", { shuffle: mode.shuffle ?? null, repeat: mode.repeat ?? null }),
  remoteInfo: () => remoteApi.call<PlayMode & { name: string; platform: string }>("remote_info"),
  /** iOS only: "airplay" (route picker), "pick_folder" / "pick_exclude" (Files app). */
  nativeRequest: (what: string) => invoke<void>("native_request", { what }),
  getSleep: () => invoke<SleepState>("get_sleep"),
  sleepTimer: (mode: SleepState["mode"], minutes?: number) => invoke<SleepState>("sleep_timer", { mode, minutes }),
};

/** Tracks streamed from Navidrome carry a virtual `navidrome://` path. */
export const isRemote = (t: { path: string } | null | undefined) => !!t?.path.startsWith("navidrome://");

/** Square cover image for a set of cover hashes (single cover or 2×2 collage). */
export function coverSrc(covers: (string | null | undefined)[] | string | null | undefined, size = 256): string | null {
  const list = (Array.isArray(covers) ? covers : [covers]).filter((c): c is string => !!c);
  if (list.length === 0) return null;
  if (list.length === 1) return convertFileSrc(`${coverRoot()}cover/${list[0]}/${size}`, "coffi");
  return convertFileSrc(`${coverRoot()}collage/${list.slice(0, 4).join(",")}/${size}`, "coffi");
}

export function blurSrc(hash: string): string {
  return convertFileSrc(`${coverRoot()}blur/${hash}`, "coffi");
}

/** Covers of the controlled device are fetched through it (`coffi://localhost/r/...`). */
function coverRoot(): string {
  return remote.target ? "r/" : "";
}
