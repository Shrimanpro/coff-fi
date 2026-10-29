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

