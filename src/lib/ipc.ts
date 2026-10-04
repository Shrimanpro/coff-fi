import { invoke } from "@tauri-apps/api/core";
import type { Settings } from "./types";
export const api = {
  getSettings: () => invoke<Settings>("get_settings"),
  setSettings: (settings: Settings) => invoke<void>("set_settings", { settings }),
  rescan: () => invoke<void>("rescan"),
};
