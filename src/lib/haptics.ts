import { invoke } from "@tauri-apps/api/core";

export type Haptic = "light" | "medium" | "heavy" | "soft" | "rigid" | "selection" | "success" | "warning" | "error";

const ios = () => document.documentElement.dataset.platform === "ios";

/** Taptic feedback on iOS (no-op elsewhere). Fire-and-forget. */
export function haptic(kind: Haptic = "light") {
  if (!ios()) return;
  invoke("native_request", { what: `haptic:${kind}` }).catch(() => {});
}
