import type { Track } from "./types";

export function time(s: number): string {
  if (!isFinite(s) || s < 0) s = 0;
  const m = Math.floor(s / 60);
  const sec = Math.floor(s % 60);
  return `${m}:${sec.toString().padStart(2, "0")}`;
}

const LOSSLESS = new Set(["FLAC", "ALAC", "WAV", "AIFF"]);

/** "FLAC 24/96 · 2304 kbps", "MP3 · 320 kbps" … */
export function quality(t: Track | null): string {
  if (!t) return "";
  const kbps = t.bitrate ? ` · ${t.bitrate} kbps` : "";
  if (LOSSLESS.has(t.codec)) {
    const bits = t.bit_depth ?? 16;
    const khz = t.sample_rate ? +(t.sample_rate / 1000).toFixed(1) : 44.1;
    return `${t.codec} ${bits}/${khz}${kbps}`;
  }
  return `${t.codec}${kbps}`;
}

export function isLossless(t: Track | null): boolean {
  return !!t && LOSSLESS.has(t.codec);
}
