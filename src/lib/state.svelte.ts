import { listen } from "@tauri-apps/api/event";
import { api } from "./ipc";
import type { Settings, Tile, TileKind, Track } from "./types";

export const ui = $state({
  base: "home" as "home" | "directory",
  dirPath: null as string | null,
  collection: null as { kind: TileKind; id: string } | null,
  theme: (document.documentElement.dataset.theme as "dark" | "light") || (matchMedia("(prefers-color-scheme: light)").matches ? "light" : "dark"),
  scanning: false,
  libVersion: 0,
});

export const conf = $state({ s: null as Settings | null });

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

function on<T>(name: string, fn: (payload: T) => void) {
  return listen<T>(name, (e) => fn(e.payload));
}
export async function init() {
  setTheme(ui.theme);
  conf.s = await api.getSettings();

  await Promise.all([
    on("library://changed", () => {
      ui.libVersion++;
    }),
    listen<boolean>("library://scanning", (e) => (ui.scanning = e.payload)),
  ]);
}

// ---------- actions ----------

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

