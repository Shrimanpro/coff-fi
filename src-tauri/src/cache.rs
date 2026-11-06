//! The queue cache: keeps the playing song and the next upcoming songs fully downloaded, so
//! playback survives a flaky connection and skipping ahead starts instantly.
//!
//! The cache is dynamic: only songs that are currently "upcoming" (see [`SLOTS`]) are kept.
//! Whenever the queue or the playing song changes, files that dropped out of that window are
//! deleted and missing ones are downloaded, at most [`PARALLEL`] at a time. It is only enabled
//! where [`init`] is called (iOS).

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use parking_lot::Mutex;

/// The playing song plus the next 8.
pub const SLOTS: usize = 9;
const PARALLEL: usize = 2;

static DIR: OnceLock<PathBuf> = OnceLock::new();
static STATE: Mutex<State> = Mutex::new(State { wanted: Vec::new(), inflight: Vec::new() });

struct State {
    /// Navidrome song ids, in play order.
    wanted: Vec<String>,
    inflight: Vec<String>,
}

fn safe(id: &str) -> String {
    id.replace(|c: char| !c.is_ascii_alphanumeric() && c != '-' && c != '_', "_")
}

/// Enables the cache, storing songs in `<cache>/queue`. Files from the last run are kept and
/// pruned on the first [`update`].
#[cfg_attr(not(target_os = "ios"), allow(dead_code))]
pub fn init(cache: &Path) {
    let dir = cache.join("queue");
    let _ = std::fs::create_dir_all(&dir);
    // Half-finished downloads are useless after a restart.
    if let Ok(rd) = std::fs::read_dir(&dir) {
        for e in rd.flatten() {
            if e.path().extension().is_some_and(|x| x == "dl") {
                let _ = std::fs::remove_file(e.path());
            }
        }
    }
    let _ = DIR.set(dir);
}

pub fn enabled() -> bool {
    DIR.get().is_some()
}

fn file_for(id: &str) -> Option<PathBuf> {
    DIR.get().map(|d| d.join(safe(id)))
}

/// The cached file for a Navidrome song, if it is completely downloaded.
pub fn cached(id: &str) -> Option<PathBuf> {
    file_for(id).filter(|p| p.is_file())
}

fn is_wanted(id: &str) -> bool {
    STATE.lock().wanted.iter().any(|w| w == id)
}

/// A finished streaming download: moves it into the cache when the song is wanted, so the
/// playing song isn't downloaded twice. Returns true when the file was taken.
pub fn adopt(id: &str, file: &Path) -> bool {
    if !is_wanted(id) {
        return false;
    }
    let Some(dest) = file_for(id) else { return false };
    if dest.is_file() {
        return false;
    }
    std::fs::rename(file, &dest).is_ok()
}

/// Sets the songs that should be cached (play order, already limited to [`SLOTS`]).
pub fn update(ids: Vec<String>) {
    let Some(dir) = DIR.get() else { return };
    let keep: HashSet<String> = ids.iter().map(|i| safe(i)).collect();
    {
        let mut st = STATE.lock();
        if st.wanted == ids {
            return;
        }
        st.wanted = ids;
    }
    if let Ok(rd) = std::fs::read_dir(dir) {
        for e in rd.flatten() {
            let name = e.file_name().to_string_lossy().into_owned();
            // In-flight downloads (`.dl`) clean themselves up when they notice they're unwanted.
            if !name.ends_with(".dl") && !keep.contains(&name) {
                let _ = std::fs::remove_file(e.path());
            }
        }
    }
    pump();
}

/// Re-checks for missing songs (e.g. after a stream of a wanted song ended without being
/// adopted).
pub fn kick() {
    if enabled() {
        pump();
    }
}

/// Starts downloads for missing wanted songs, nearest first. Songs the player is already
/// streaming are skipped: the stream's file is adopted when it finishes.
fn pump() {
    let mut st = STATE.lock();
    while st.inflight.len() < PARALLEL {
        let next = st
            .wanted
            .iter()
            .find(|id| !st.inflight.contains(id) && cached(id).is_none() && !crate::navidrome::is_streaming(id))
            .cloned();
        let Some(id) = next else { break };
        st.inflight.push(id.clone());
        let spawned = std::thread::Builder::new().name("cache".into()).spawn({
            let id = id.clone();
            move || {
                if let Err(e) = fetch(&id) {
                    eprintln!("cache {id}: {e:#}");
                }
                STATE.lock().inflight.retain(|i| *i != id);
                pump();
            }
        });
        if spawned.is_err() {
            st.inflight.retain(|i| *i != id);
            break;
        }
    }
}

fn fetch(id: &str) -> anyhow::Result<()> {
    let dest = file_for(id).ok_or_else(|| anyhow::anyhow!("cache disabled"))?;
    let tmp = dest.with_extension("dl");
    let ok = crate::navidrome::download(id, &tmp, || is_wanted(id));
    match ok {
        Ok(true) if is_wanted(id) && !dest.is_file() => {
            std::fs::rename(&tmp, &dest)?;
        }
        _ => {
            let _ = std::fs::remove_file(&tmp);
        }
    }
    ok.map(|_| ())
}

/// The next `SLOTS` song ids starting at the playing one, following the queue's play order
/// (wrapping around when repeat is on). Only Navidrome songs are cacheable.
pub fn window(items: &[(i64, Option<String>)], pos: usize, wrap: bool) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let n = items.len();
    if n == 0 {
        return out;
    }
    let steps = if wrap { n } else { n.saturating_sub(pos) };
    for k in 0..steps.min(SLOTS) {
        if let Some(id) = &items[(pos + k) % n].1 {
            if !out.contains(id) {
                out.push(id.clone());
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn items(n: i64) -> Vec<(i64, Option<String>)> {
        (0..n).map(|i| (i, Some(format!("s{i}")))).collect()
    }

    #[test]
    fn window_is_current_plus_next_eight() {
        let w = window(&items(20), 3, false);
        assert_eq!(w.len(), SLOTS);
        assert_eq!(w.first().map(String::as_str), Some("s3"));
        assert_eq!(w.last().map(String::as_str), Some("s11"));
    }

    #[test]
    fn window_stops_at_end_or_wraps_with_repeat() {
        assert_eq!(window(&items(5), 3, false), vec!["s3", "s4"]);
        assert_eq!(window(&items(5), 3, true), vec!["s3", "s4", "s0", "s1", "s2"]);
    }

    #[test]
    fn window_skips_local_tracks() {
        let mut it = items(4);
        it[1].1 = None;
        assert_eq!(window(&it, 0, false), vec!["s0", "s2", "s3"]);
    }
}
