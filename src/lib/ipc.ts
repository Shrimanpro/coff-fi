import { invoke } from "@tauri-apps/api/core";
import type { Collection, FolderView, Home, Settings, Tile } from "./types";
export const api = {
  getSettings: () => invoke<Settings>("get_settings"),
  setSettings: (settings: Settings) => invoke<void>("set_settings", { settings }),
  rescan: () => invoke<void>("rescan"),
  home: () => invoke<Home>("get_home"),
  roots: () => invoke<Tile[]>("get_roots"),
  folder: (path: string) => invoke<FolderView>("get_folder", { path }),
  collection: (kind: string, id: string) => invoke<Collection>("get_collection", { kind, id }),
};
