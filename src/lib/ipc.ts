import { convertFileSrc, invoke } from "@tauri-apps/api/core";
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

function coverRoot(): string {
  return "";
}
