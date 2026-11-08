export interface Track {
  id: number;
  path: string;
  dir: string;
  title: string;
  artist: string;
  album: string;
  album_artist: string;
  track_no: number | null;
  year: number | null;
  duration: number;
  codec: string;
  sample_rate: number | null;
  bit_depth: number | null;
  bitrate: number | null;
  cover: string | null;
}

/** Editable tags of a local file (song info panel). */
export interface Tags {
  title: string;
  artist: string;
  album: string;
  album_artist: string;
  genre: string;
  track_no: number | null;
  track_total: number | null;
  disc_no: number | null;
  disc_total: number | null;
  year: number | null;
}

export interface FileInfo {
  tags: Tags;
  size: number;
  read_only: boolean;
}

export type TileKind = "playlist" | "album" | "artist" | "folder";

export interface Tile {
  kind: TileKind;
  id: string;
  name: string;
  subtitle: string;
  count: number;
  covers: string[];
}

export interface Home {
  playlists: Tile[];
  /** Playlists made on the Navidrome server (empty for local files). */
  server_playlists: Tile[];
  albums: Tile[];
  artists: Tile[];
  songs: Track[];
}

export interface FolderView {
  path: string;
  name: string;
  covers: string[];
  folders: Tile[];
  tracks: Track[];
}

export interface Collection {
  kind: TileKind;
  id: string;
  title: string;
  subtitle: string;
  covers: string[];
  tracks: Track[];
}

export interface Status {
  track_id: number | null;
  playing: boolean;
  position: number;
  duration: number;
}

export type Repeat = "off" | "all" | "one";

export interface QueueView {
  name: string;
  pos: number;
  shuffle: boolean;
  repeat: Repeat;
  tracks: Track[];
}

export interface SavedQueue {
  id: number;
  name: string;
  kind: "saved" | "history";
  count: number;
  updated_at: number;
  covers: string[];
}

export interface Word {
  start: number;
  end: number;
  text: string;
}

export interface LyricLine {
  start: number;
  end: number;
  text: string;
  words: Word[];
  background: boolean;
}

export interface Lyrics {
  source: string;
  synced: "none" | "line" | "word";
  lines: LyricLine[];
}

export interface EqSettings {
  enabled: boolean;
  preamp: number;
  bands: number[];
  preset: string;
}

export interface EqPreset {
  name: string;
  preamp: number;
  bands: number[];
}

export interface Settings {
  roots: string[];
  excluded: string[];
  watch: boolean;
  eq: EqSettings;
  eq_presets: EqPreset[];
  volume: number;
  output_device: string | null;
  gapless: boolean;
  crossfade: number;
  replaygain: "off" | "track" | "album";
  rg_preamp: number;
  speed: number;
  shuffle: boolean;
  repeat: Repeat;
  media_controls: boolean;
  discord: { enabled: boolean };
  lastfm: { enabled: boolean; api_key: string; api_secret: string; username: string; session_key: string };
  navidrome: Navidrome;
  /** iOS: turning the phone sideways in Now Playing opens the full-screen Nightstand view. */
  nightstand: boolean;
  /** iOS: keep the screen awake while the Nightstand view is showing. */
  nightstand_awake: boolean;
  /** Being controlled from other devices on the network. */
  remote: { allow: boolean; name: string; id: string };
}

export interface SearchResults {
  songs: Track[];
  albums: Tile[];
  artists: Tile[];
  playlists: Tile[];
}

export interface SleepState {
  mode: "off" | "minutes" | "end_of_track";
  /** Unix ms when a "minutes" timer pauses playback. */
  until: number | null;
}

export interface Navidrome {
  /** True while the app browses the server instead of local files. */
  active: boolean;
  url: string;
  username: string;
  password: string;
}

/** Another Coff-fi on the local network. */
export interface Device {
  id: string;
  name: string;
  platform: string;
  host: string;
  port: number;
}

/** Volume and play mode of a device (the controlled one, while controlling). */
export interface PlayMode {
  volume: number;
  shuffle: boolean;
  repeat: Repeat;
}
