mod lrclib;
mod matching;
pub mod parse;

use std::collections::HashMap;
use std::path::Path;
use std::sync::{Arc, OnceLock};

use parking_lot::Mutex;
use std::time::Duration;

use anyhow::Result;
use lofty::prelude::*;
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};

use crate::library::db::{now, Track};

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Synced {
    None,
    Line,
    Word,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Word {
    pub start: f64,
    pub end: f64,
    pub text: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Line {
    pub start: f64,
    pub end: f64,
    pub text: String,
    pub words: Vec<Word>,
    pub background: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Lyrics {
    pub source: String,
    pub synced: Synced,
    pub lines: Vec<Line>,
}

impl Lyrics {
    fn from_text(source: &str, text: &str) -> Option<Self> {
        if text.trim().is_empty() {
            return None;
        }
        Some(match parse::lrc(text) {
            Some((synced, lines)) => Self { source: source.into(), synced, lines },
            None => Self { source: source.into(), synced: Synced::None, lines: parse::plain(text) },
        })
    }
}

/// Lookup info passed to online providers.
pub struct Query<'a> {
    pub title: &'a str,
    pub artist: &'a str,
    pub album: &'a str,
    pub duration: f64,
}

/// Results without timing (plain text or nothing found) are looked up again after this long,
/// since a synced version may have been added to LRCLIB since. Synced results are kept.
const RECHECK_AFTER: i64 = 24 * 3600;

pub fn http() -> reqwest::blocking::Client {
    reqwest::blocking::Client::builder()
        .user_agent(concat!("Coff-fi/", env!("CARGO_PKG_VERSION"), " (local music player)"))
        .timeout(Duration::from_secs(12))
        .build()
        .expect("http client")
}

fn local(path: &Path) -> Vec<Lyrics> {
    let mut out = Vec::new();
    if let Ok(f) = lofty::read_from_path(path) {
        let text = f.primary_tag().or_else(|| f.first_tag()).and_then(|t| t.get_string(&ItemKey::Lyrics).map(str::to_string));
        if let Some(l) = text.and_then(|t| Lyrics::from_text("Embedded", &t)) {
            out.push(l);
        }
    }
    if let Ok(t) = std::fs::read_to_string(path.with_extension("lrc")) {
        if let Some(l) = Lyrics::from_text("Local .lrc file", &t) {
            out.push(l);
        }
    }
    out
}

fn cached(conn: &Connection, id: i64) -> Option<Option<Lyrics>> {
    let (json, at): (String, i64) = conn.query_row("SELECT json, fetched_at FROM lyrics WHERE track_id = ?1", [id], |r| Ok((r.get(0)?, r.get(1)?))).optional().ok()??;
    let l: Option<Lyrics> = serde_json::from_str(&json).ok()?;
    let timed = l.as_ref().is_some_and(|l| l.synced != Synced::None);
    if !timed && now() - at > RECHECK_AFTER {
        return None;
    }
    Some(l)
}

fn store(conn: &Connection, id: i64, l: &Option<Lyrics>) {
    if let Ok(j) = serde_json::to_string(l) {
        let _ = conn.execute(
            "INSERT INTO lyrics(track_id, json, fetched_at) VALUES(?1, ?2, ?3) ON CONFLICT(track_id) DO UPDATE SET json = excluded.json, fetched_at = excluded.fetched_at",
            params![id, j, now()],
        );
    }
}

/// Resolves lyrics: source file (tags or a sibling .lrc) → LRCLIB. Synced local lyrics win
/// outright; plain local lyrics are only replaced by synced ones from LRCLIB.
pub fn get(conn: &Connection, t: &Track, refresh: bool) -> Result<Option<Lyrics>> {
    // One lookup per song at a time: a background prefetch and the lyrics view share the result.
    static INFLIGHT: OnceLock<Mutex<HashMap<String, Arc<Mutex<()>>>>> = OnceLock::new();
    let inflight = INFLIGHT.get_or_init(Default::default);
    let gate = inflight.lock().entry(t.path.clone()).or_default().clone();
    let res = {
        let _held = gate.lock();
        resolve(conn, t, refresh)
    };
    let mut m = inflight.lock();
    // Only the map and this call still hold the gate: nobody is waiting, so drop it.
    if Arc::strong_count(&gate) <= 2 {
        m.remove(&t.path);
    }
    res
}

fn resolve(conn: &Connection, t: &Track, refresh: bool) -> Result<Option<Lyrics>> {
    let locals = local(Path::new(&t.path));
    if let Some(l) = locals.iter().find(|l| l.synced != Synced::None) {
        return Ok(Some(l.clone()));
    }
    let online = match (refresh, cached(conn, t.id)) {
        (false, Some(c)) => c,
        _ => {
            let q = Query { title: &t.title, artist: &t.artist, album: &t.album, duration: t.duration };
            match lrclib::fetch(&http(), &q) {
                Ok(l) => {
                    store(conn, t.id, &l);
                    l
                }
                // Not cached: a network hiccup shouldn't hide lyrics. With nothing local to show,
                // report it so the view can say so and try again.
                Err(e) if locals.is_empty() => return Err(e.context("Couldn't reach LRCLIB")),
                Err(e) => {
                    eprintln!("lrclib: {e:#}");
                    None
                }
            }
        }
    };
    Ok(match online {
        Some(l) if l.synced != Synced::None => Some(l),
        other => locals.into_iter().next().or(other),
    })
}
