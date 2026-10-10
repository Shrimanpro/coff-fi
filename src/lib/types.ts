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

export interface Settings {
  roots: string[];
  excluded: string[];
  volume: number;
  shuffle: boolean;
  repeat: Repeat;
}
