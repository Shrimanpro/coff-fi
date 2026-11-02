// Controlling another Coff-fi on the network (and being controlled by one).
// While `remote.target` is set, library / queue / playback calls in `api` go to that device
// (see `call` in ipc.ts) and the UI follows its events instead of our own player's.
import { invoke } from "@tauri-apps/api/core";
import type { Device, PlayMode } from "./types";

export const remote = $state({
  /** The device we're controlling, or null when playing here. */
  target: null as Device | null,
  /** Other Coff-fi devices found on the network. */
  devices: [] as Device[],
  /** Names of the devices controlling this one. */
  controllers: [] as string[],
  /** Our own name on the network. */
  name: "",
  /** The controlled device's volume and play mode. */
  mode: { volume: 0.8, shuffle: false, repeat: "off" } as PlayMode,
  /** The device picker is open. */
  picker: false,
  connecting: null as string | null,
  /** Why the last connection attempt failed (shown in the picker). */
  error: null as string | null,
});

export const remoteApi = {
  state: () => invoke<{ target: Device | null; controllers: string[]; name: string }>("remote_state"),
  devices: () => invoke<Device[]>("remote_devices"),
  connect: (id: string | null, address: string | null) => invoke<Device>("remote_connect", { id, address }),
  disconnect: () => invoke<void>("remote_disconnect"),
  call: <T>(cmd: string, args?: Record<string, unknown>) => invoke<T>("remote_call", { cmd, args: args ?? {} }),
};

/** "Mac", "iPhone"… for the platform a device reports. */
export function deviceKind(platform: string): string {
  return platform === "ios" ? "iPhone" : platform === "macos" ? "Mac" : platform === "windows" ? "PC" : "Computer";
}
