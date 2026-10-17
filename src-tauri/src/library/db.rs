use std::path::Path;

use anyhow::Result;
use rusqlite::{params, Connection, OptionalExtension, Row};
use serde::Serialize;

const SCHEMA: &str = "
PRAGMA journal_mode = WAL;
PRAGMA synchronous = NORMAL;
PRAGMA foreign_keys = OFF;
CREATE TABLE IF NOT EXISTS tracks(
  id INTEGER PRIMARY KEY,
  path TEXT UNIQUE NOT NULL,
  dir TEXT NOT NULL,
  mtime INTEGER NOT NULL,
  title TEXT NOT NULL,
  artist TEXT NOT NULL,
  album TEXT NOT NULL,
  album_artist TEXT NOT NULL,
  track_no INTEGER,
  disc_no INTEGER,
  year INTEGER,
  duration REAL NOT NULL,
  codec TEXT NOT NULL,
  sample_rate INTEGER,
  bit_depth INTEGER,
  bitrate INTEGER,
  rg_track REAL,
  rg_album REAL,
  cover TEXT,
  added_at INTEGER NOT NULL
);
CREATE INDEX IF NOT EXISTS tracks_dir ON tracks(dir);
CREATE INDEX IF NOT EXISTS tracks_album ON tracks(album, album_artist);
CREATE INDEX IF NOT EXISTS tracks_artist ON tracks(artist);
CREATE TABLE IF NOT EXISTS covers(hash TEXT PRIMARY KEY, path TEXT NOT NULL, embedded INTEGER NOT NULL);
CREATE TABLE IF NOT EXISTS folder_covers(dir TEXT PRIMARY KEY, hash TEXT NOT NULL);
CREATE TABLE IF NOT EXISTS queues(id INTEGER PRIMARY KEY, name TEXT NOT NULL, kind TEXT NOT NULL, pos INTEGER NOT NULL, updated_at INTEGER NOT NULL);
CREATE TABLE IF NOT EXISTS queue_items(queue_id INTEGER NOT NULL, idx INTEGER NOT NULL, track_id INTEGER NOT NULL);
CREATE INDEX IF NOT EXISTS queue_items_q ON queue_items(queue_id);
CREATE TABLE IF NOT EXISTS settings(k TEXT PRIMARY KEY, v TEXT NOT NULL);
";

pub const TRACK_COLS: &str = "t.id, t.path, t.dir, t.title, t.artist, t.album, t.album_artist, t.track_no, t.year, t.duration, t.codec, t.sample_rate, t.bit_depth, t.bitrate, t.cover";

#[derive(Clone, Debug, Serialize)]
pub struct Track {
    pub id: i64,
    pub path: String,
    pub dir: String,
    pub title: String,
    pub artist: String,
    pub album: String,
    pub album_artist: String,
    pub track_no: Option<u32>,
    pub year: Option<u32>,
    pub duration: f64,
    pub codec: String,
    pub sample_rate: Option<u32>,
    pub bit_depth: Option<u32>,
    pub bitrate: Option<u32>,
    pub cover: Option<String>,
}

impl Track {
    pub fn from_row(r: &Row) -> rusqlite::Result<Self> {
        Ok(Self {
            id: r.get(0)?,
            path: r.get(1)?,
            dir: r.get(2)?,
            title: r.get(3)?,
            artist: r.get(4)?,
            album: r.get(5)?,
            album_artist: r.get(6)?,
            track_no: r.get(7)?,
            year: r.get(8)?,
            duration: r.get(9)?,
            codec: r.get(10)?,
            sample_rate: r.get(11)?,
            bit_depth: r.get(12)?,
            bitrate: r.get(13)?,
            cover: r.get(14)?,
        })
    }
}

pub fn now() -> i64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0)
}

pub fn open(path: &Path) -> Result<Connection> {
    let conn = Connection::open(path)?;
    conn.busy_timeout(std::time::Duration::from_secs(5))?;
    conn.execute_batch(SCHEMA)?;
    Ok(conn)
}

pub fn tracks_where(conn: &Connection, clause: &str, p: impl rusqlite::Params) -> Result<Vec<Track>> {
    let sql = format!("SELECT {TRACK_COLS} FROM tracks t {clause}");
    let mut st = conn.prepare_cached(&sql)?;
    let rows = st.query_map(p, Track::from_row)?;
    Ok(rows.collect::<rusqlite::Result<_>>()?)
}

pub fn track(conn: &Connection, id: i64) -> Result<Option<Track>> {
    Ok(tracks_where(conn, "WHERE t.id = ?1", [id])?.into_iter().next())
}

/// Fetch tracks preserving the order of `ids` (duplicates allowed, missing ids dropped).
pub fn tracks_by_ids(conn: &Connection, ids: &[i64]) -> Result<Vec<Track>> {
    if ids.is_empty() {
        return Ok(Vec::new());
    }
    let list = ids.iter().map(i64::to_string).collect::<Vec<_>>().join(",");
    let found = tracks_where(conn, &format!("WHERE t.id IN ({list})"), [])?;
    let map: std::collections::HashMap<i64, Track> = found.into_iter().map(|t| (t.id, t)).collect();
    Ok(ids.iter().filter_map(|id| map.get(id).cloned()).collect())
}

pub fn replaygain(conn: &Connection, id: i64) -> Result<(Option<f32>, Option<f32>)> {
    Ok(conn
        .query_row("SELECT rg_track, rg_album FROM tracks WHERE id = ?1", [id], |r| Ok((r.get(0)?, r.get(1)?)))
        .optional()?
        .unwrap_or((None, None)))
}

pub fn get_setting(conn: &Connection, k: &str) -> Result<Option<String>> {
    Ok(conn.query_row("SELECT v FROM settings WHERE k = ?1", [k], |r| r.get(0)).optional()?)
}

pub fn set_setting(conn: &Connection, k: &str, v: &str) -> Result<()> {
    conn.execute("INSERT INTO settings(k, v) VALUES(?1, ?2) ON CONFLICT(k) DO UPDATE SET v = excluded.v", params![k, v])?;
    Ok(())
}
