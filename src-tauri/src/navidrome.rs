//! Navidrome (Subsonic API) support: the server's library is mirrored into its own SQLite
//! database with the same schema as the local one, so every view works unchanged. Tracks get
//! virtual paths (`navidrome://<id>.<ext>`) and are streamed on demand.

use std::collections::HashSet;
use std::fs::File;
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{PathBuf, MAIN_SEPARATOR};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, OnceLock};
use std::time::Duration;

use anyhow::{anyhow, bail, Context, Result};
use parking_lot::{Condvar, Mutex, RwLock};
use reqwest::blocking::Client;
use rusqlite::{params, Connection};
use serde::Deserialize;
use serde_json::Value;
use xxhash_rust::xxh3::xxh3_64;

use crate::library::db::now;
use crate::library::scan::ScanStats;
use crate::settings::Navidrome;

pub const PREFIX: &str = "navidrome://";
/// Top-level folder of the mirrored library in the Directory view.
pub const ROOT: &str = "Navidrome";
const COVER_PREFIX: &str = "navidrome:";
const PAGE: usize = 500;

static SERVER: RwLock<Option<Navidrome>> = parking_lot::const_rwlock(None);
static STREAM_DIR: OnceLock<PathBuf> = OnceLock::new();

/// Sets the server used for streaming, covers and API calls (None = disconnected).
pub fn configure(s: Option<Navidrome>) {
    *SERVER.write() = s;
}

/// Where partially downloaded streams live; stale ones from a previous run are removed.
pub fn init(cache: PathBuf) {
    let dir = cache.join("streams");
    let _ = std::fs::remove_dir_all(&dir);
    let _ = std::fs::create_dir_all(&dir);
    let _ = STREAM_DIR.set(dir);
}

fn server() -> Result<Navidrome> {
    SERVER.read().clone().ok_or_else(|| anyhow!("Navidrome is not connected"))
}

/// Song id from a virtual track path.
pub fn song_id(path: &str) -> Option<&str> {
    let rest = path.strip_prefix(PREFIX)?;
    Some(rest.rsplit_once('.').map_or(rest, |(id, _)| id))
}

/// Cover-art id from a covers-table path.
pub fn cover_id(path: &str) -> Option<&str> {
    path.strip_prefix(COVER_PREFIX).filter(|p| !p.starts_with("//"))
}

fn api_client() -> &'static Client {
    static C: OnceLock<Client> = OnceLock::new();
    C.get_or_init(|| {
        Client::builder()
            .user_agent(concat!("Coff-fi/", env!("CARGO_PKG_VERSION")))
            .connect_timeout(Duration::from_secs(8))
            .timeout(Duration::from_secs(60))
            .build()
            .expect("http client")
    })
}

fn stream_client() -> &'static Client {
    static C: OnceLock<Client> = OnceLock::new();
    // No overall timeout: a long FLAC may take a while to download on a slow link.
    C.get_or_init(|| {
        Client::builder()
            .user_agent(concat!("Coff-fi/", env!("CARGO_PKG_VERSION")))
            .connect_timeout(Duration::from_secs(8))
            .timeout(None)
            .build()
            .expect("http client")
    })
}

fn base(s: &Navidrome) -> String {
    let u = s.url.trim().trim_end_matches('/');
    if u.starts_with("http://") || u.starts_with("https://") { u.to_string() } else { format!("http://{u}") }
}

/// Token auth: t = md5(password + salt).
fn auth(s: &Navidrome) -> Vec<(&'static str, String)> {
    static N: AtomicU64 = AtomicU64::new(0);
    let nanos = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0);
    let salt = format!("{:012x}", xxh3_64(format!("{nanos}-{}", N.fetch_add(1, Ordering::Relaxed)).as_bytes()) & 0xffff_ffff_ffff);
    let token = format!("{:x}", md5::compute(format!("{}{salt}", s.password)));
    vec![("u", s.username.clone()), ("t", token), ("s", salt), ("v", "1.16.1".into()), ("c", "coff-fi".into()), ("f", "json".into())]
}

fn call(s: &Navidrome, method: &str, extra: &[(&str, &str)]) -> Result<Value> {
    let res = api_client()
        .get(format!("{}/rest/{method}", base(s)))
        .query(&auth(s))
        .query(extra)
        .send()
        .with_context(|| format!("Can't reach {}", base(s)))?
        .error_for_status()?;
    let mut v: Value = res.json().context("The server didn't answer like a Subsonic/Navidrome server")?;
    let r = v.get_mut("subsonic-response").map(Value::take).ok_or_else(|| anyhow!("Not a Subsonic/Navidrome server"))?;
    if r.get("status").and_then(Value::as_str) != Some("ok") {
        let msg = r.pointer("/error/message").and_then(Value::as_str).unwrap_or("request failed");
        bail!("Navidrome: {msg}");
    }
    Ok(r)
}

/// Checks the connection and credentials. Returns a short description of the server.
pub fn ping(s: &Navidrome) -> Result<String> {
    if s.url.trim().is_empty() || s.username.trim().is_empty() {
        bail!("Enter the server address and username in Settings → Navidrome");
    }
    let r = call(s, "ping", &[])?;
    let kind = r.get("type").and_then(Value::as_str).unwrap_or("Subsonic");
    let ver = r.get("serverVersion").and_then(Value::as_str).unwrap_or("");
    let mut name = kind.to_string();
    if let Some(c) = name.get_mut(..1) {
        c.make_ascii_uppercase();
    }
    Ok(format!("{name} {ver}").trim().to_string())
}

// ---------- library sync ----------

#[derive(Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
struct ReplayGain {
    track_gain: Option<f64>,
    album_gain: Option<f64>,
}

#[derive(Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
struct Song {
    id: String,
    title: String,
    album: String,
    artist: String,
    display_album_artist: Option<String>,
    track: Option<u32>,
    disc_number: Option<u32>,
    year: Option<u32>,
    cover_art: Option<String>,
    album_id: Option<String>,
    suffix: String,
    duration: Option<f64>,
    bit_rate: Option<u32>,
    sampling_rate: Option<u32>,
    bit_depth: Option<u32>,
    path: Option<String>,
    created: Option<String>,
    starred: Option<String>,
    play_count: Option<i64>,
    played: Option<String>,
    replay_gain: Option<ReplayGain>,
}

fn codec(suffix: &str, bit_depth: Option<u32>) -> String {
    match suffix.to_ascii_lowercase().as_str() {
        "flac" => "FLAC".into(),
        "mp3" => "MP3".into(),
        "wav" => "WAV".into(),
        "aif" | "aiff" => "AIFF".into(),
        "ogg" | "oga" => "OGG".into(),
        "opus" => "OPUS".into(),
        "m4a" | "mp4" if bit_depth.is_some_and(|b| b > 0) => "ALAC".into(),
        "m4a" | "mp4" | "aac" => "AAC".into(),
        other => other.to_ascii_uppercase(),
    }
}

/// Days-from-civil (Howard Hinnant) for parsing ISO-8601 timestamps without a date crate.
fn iso_ts(s: &str) -> Option<i64> {
    let (date, time) = s.split_once('T').unwrap_or((s, "00:00:00"));
    let mut d = date.split('-').map(|x| x.parse::<i64>().ok());
    let (y, m, dd) = (d.next()??, d.next()??, d.next()??);
    let hms: Vec<i64> = time.get(..8).unwrap_or("00:00:00").split(':').filter_map(|x| x.parse().ok()).collect();
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let mp = (m + 9) % 12;
    let doy = (153 * mp + 2) / 5 + dd - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    let days = era * 146097 + doe - 719468;
    Some(days * 86400 + hms.first().copied().unwrap_or(0) * 3600 + hms.get(1).copied().unwrap_or(0) * 60 + hms.get(2).copied().unwrap_or(0))
}

fn clean_part(p: &str) -> String {
    p.replace(['/', '\\'], "_").trim().to_string()
}

/// Virtual folder for a song, mirroring the server's path (`Artist/Album/01 - x.flac`).
fn virtual_dir(s: &Song) -> String {
    let sep = MAIN_SEPARATOR.to_string();
    let mut parts: Vec<String> = match s.path.as_deref() {
        Some(p) if p.contains('/') || p.contains('\\') => {
            let mut v: Vec<String> = p.split(['/', '\\']).filter(|x| !x.is_empty() && *x != "." && *x != "..").map(clean_part).collect();
            v.pop();
            v
        }
        _ => vec![clean_part(&s.artist), clean_part(&s.album)],
    };
    parts.retain(|p| !p.is_empty());
    let mut dir = ROOT.to_string();
    for p in parts {
        dir.push_str(&sep);
        dir.push_str(&p);
    }
    dir
}

fn cover_hash(id: &str) -> String {
    format!("n{:016x}", xxh3_64(id.as_bytes()))
}

fn fetch_songs(s: &Navidrome) -> Result<Vec<Song>> {
    let mut out = Vec::new();
    loop {
        let off = out.len().to_string();
        let count = PAGE.to_string();
        // Navidrome returns every song for an empty query, which is the documented way to sync.
        let r = call(s, "search3", &[("query", ""), ("songCount", &count), ("songOffset", &off), ("artistCount", "0"), ("albumCount", "0")])?;
        let songs: Vec<Song> = r.pointer("/searchResult3/song").map(|v| serde_json::from_value(v.clone())).transpose()?.unwrap_or_default();
        let n = songs.len();
        out.extend(songs);
        if n < PAGE {
            break;
        }
    }
    Ok(out)
}

#[derive(Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
struct Playlist {
    id: String,
    name: String,
    comment: Option<String>,
    cover_art: Option<String>,
}

/// The user's playlists on the server with the song ids they contain, in order.
fn fetch_playlists(s: &Navidrome) -> Result<Vec<(Playlist, Vec<String>)>> {
    let r = call(s, "getPlaylists", &[])?;
    let lists: Vec<Playlist> = r.pointer("/playlists/playlist").map(|v| serde_json::from_value(v.clone())).transpose()?.unwrap_or_default();
    // A few at a time: libraries can have hundreds of playlists, each needing its own request.
    let pool = rayon::ThreadPoolBuilder::new().num_threads(6).build()?;
    pool.install(|| {
        use rayon::prelude::*;
        lists
            .into_par_iter()
            .map(|p| {
                let r = call(s, "getPlaylist", &[("id", &p.id)])?;
                let ids = r
                    .pointer("/playlist/entry")
                    .and_then(Value::as_array)
                    .map(|a| a.iter().filter_map(|e| e.get("id").and_then(Value::as_str).map(str::to_string)).collect())
                    .unwrap_or_default();
                Ok((p, ids))
            })
            .collect()
    })
}

/// Mirrors the server's library into `conn`. Play counts, stars and playlists come from the server.
pub fn sync(conn: &mut Connection) -> Result<ScanStats> {
    let s = server()?;
    let songs = fetch_songs(&s)?;
    let playlists = fetch_playlists(&s)?;
    let known: HashSet<String> = {
        let mut st = conn.prepare("SELECT path FROM tracks")?;
        let rows = st.query_map([], |r| r.get::<_, String>(0))?;
        rows.collect::<rusqlite::Result<_>>()?
    };
    let first_import = known.is_empty();
    let ts = now();
    let mut seen = HashSet::with_capacity(songs.len());
    let tx = conn.transaction()?;
    {
        let mut up = tx.prepare(
            "INSERT INTO tracks(path, dir, mtime, title, artist, album, album_artist, track_no, disc_no, year, duration, codec, sample_rate, bit_depth, bitrate, rg_track, rg_album, cover, added_at)
             VALUES(?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19)
             ON CONFLICT(path) DO UPDATE SET dir = excluded.dir, mtime = excluded.mtime, title = excluded.title, artist = excluded.artist, album = excluded.album,
               album_artist = excluded.album_artist, track_no = excluded.track_no, disc_no = excluded.disc_no, year = excluded.year,
               duration = excluded.duration, codec = excluded.codec, sample_rate = excluded.sample_rate, bit_depth = excluded.bit_depth,
               bitrate = excluded.bitrate, rg_track = excluded.rg_track, rg_album = excluded.rg_album, cover = excluded.cover",
        )?;
        let mut cover = tx.prepare("INSERT OR IGNORE INTO covers(hash, path, embedded) VALUES(?1, ?2, 0)")?;
        tx.execute("DELETE FROM plays", [])?;
        tx.execute("DELETE FROM favorites", [])?;
        let mut play = tx.prepare("INSERT INTO plays(track_id, ts, n) VALUES((SELECT id FROM tracks WHERE path = ?1), ?2, ?3)")?;
        let mut star = tx.prepare("INSERT OR IGNORE INTO favorites(track_id, ts) SELECT id, ?2 FROM tracks WHERE path = ?1")?;
        for song in &songs {
            if song.id.is_empty() {
                continue;
            }
            let ext = if song.suffix.is_empty() { "mp3" } else { &song.suffix };
            let path = format!("{PREFIX}{}.{ext}", song.id);
            let created = song.created.as_deref().and_then(iso_ts).unwrap_or(0);
            // Album art is shared by all songs of an album, so collages don't repeat one cover.
            let art = song.album_id.as_ref().filter(|a| !a.is_empty()).map(|a| format!("al-{a}")).or_else(|| song.cover_art.clone());
            let hash = art.as_ref().map(|a| {
                let h = cover_hash(a);
                let _ = cover.execute(params![h, format!("{COVER_PREFIX}{a}")]);
                h
            });
            let rg = song.replay_gain.as_ref();
            let album_artist = song.display_album_artist.clone().filter(|a| !a.is_empty()).unwrap_or_else(|| song.artist.clone());
            let title = if song.title.is_empty() { song.id.clone() } else { song.title.clone() };
            let added = if first_import || !known.contains(&path) { if created > 0 { created } else { ts } } else { created };
            up.execute(params![
                path,
                virtual_dir(song),
                created,
                title,
                song.artist,
                song.album,
                album_artist,
                song.track,
                song.disc_number,
                song.year,
                song.duration.unwrap_or(0.0),
                codec(ext, song.bit_depth),
                song.sampling_rate,
                song.bit_depth.filter(|b| *b > 0),
                song.bit_rate,
                rg.and_then(|r| r.track_gain),
                rg.and_then(|r| r.album_gain),
                hash,
                added
            ])?;
            if let Some(n) = song.play_count.filter(|n| *n > 0) {
                let last = song.played.as_deref().and_then(iso_ts).unwrap_or(created);
                play.execute(params![path, last, n])?;
            }
            if let Some(st) = &song.starred {
                star.execute(params![path, iso_ts(st).unwrap_or(ts)])?;
            }
            seen.insert(path);
        }
        let mut del = tx.prepare("DELETE FROM tracks WHERE path = ?1")?;
        for k in known.iter().filter(|k| !seen.contains(*k)) {
            del.execute([k])?;
        }
        tx.execute("DELETE FROM covers WHERE hash NOT IN (SELECT cover FROM tracks WHERE cover IS NOT NULL)", [])?;

        let by_song: std::collections::HashMap<String, i64> = {
            let mut st = tx.prepare("SELECT id, path FROM tracks")?;
            let rows = st.query_map([], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?)))?;
            rows.filter_map(Result::ok).filter_map(|(id, p)| song_id(&p).map(|s| (s.to_string(), id))).collect()
        };
        tx.execute("DELETE FROM server_playlists", [])?;
        tx.execute("DELETE FROM server_playlist_items", [])?;
        let mut pl = tx.prepare("INSERT OR REPLACE INTO server_playlists(id, name, comment, idx, cover) VALUES(?1, ?2, ?3, ?4, ?5)")?;
        let mut cover = tx.prepare("INSERT OR IGNORE INTO covers(hash, path, embedded) VALUES(?1, ?2, 0)")?;
        let mut item = tx.prepare("INSERT INTO server_playlist_items(playlist_id, idx, track_id) VALUES(?1, ?2, ?3)")?;
        for (i, (p, ids)) in playlists.iter().enumerate() {
            // The server resolves the playlist's own artwork (uploaded image, sidecar image or the
            // cover named in the .m3u8) and only falls back to a mosaic when there is none.
            let art = p.cover_art.as_ref().filter(|a| !a.is_empty()).map(|a| {
                let h = cover_hash(a);
                let _ = cover.execute(params![h, format!("{COVER_PREFIX}{a}")]);
                h
            });
            pl.execute(params![p.id, p.name, p.comment.clone().unwrap_or_default(), i as i64, art])?;
            for (j, tid) in ids.iter().filter_map(|s| by_song.get(s)).enumerate() {
                item.execute(params![p.id, j as i64, tid])?;
            }
        }
    }
    tx.commit()?;
    let removed = known.iter().filter(|k| !seen.contains(*k)).count();
    Ok(ScanStats { total: seen.len(), updated: seen.len(), removed })
}

// ---------- per-song calls ----------

pub fn cover_bytes(id: &str) -> Result<Vec<u8>> {
    let s = server()?;
    let res = api_client()
        .get(format!("{}/rest/getCoverArt", base(&s)))
        .query(&auth(&s))
        .query(&[("id", id), ("size", "1000")])
        .send()?
        .error_for_status()?;
    let is_image = res.headers().get("content-type").and_then(|v| v.to_str().ok()).is_some_and(|c| c.starts_with("image/"));
    if !is_image {
        bail!("no cover art for {id}");
    }
    Ok(res.bytes()?.to_vec())
}

/// `submission = false` reports "now playing"; true counts a play on the server.
pub fn scrobble(path: &str, submission: bool) {
    let Some(id) = song_id(path).map(str::to_string) else { return };
    std::thread::spawn(move || {
        if let Ok(s) = server() {
            let time = (now() * 1000).to_string();
            let sub = if submission { "true" } else { "false" };
            if let Err(e) = call(&s, "scrobble", &[("id", &id), ("submission", sub), ("time", &time)]) {
                eprintln!("navidrome scrobble: {e:#}");
            }
        }
    });
}

pub fn set_star(path: &str, on: bool) {
    let Some(id) = song_id(path).map(str::to_string) else { return };
    std::thread::spawn(move || {
        if let Ok(s) = server() {
            if let Err(e) = call(&s, if on { "star" } else { "unstar" }, &[("id", &id)]) {
                eprintln!("navidrome star: {e:#}");
            }
        }
    });
}

/// Lyrics stored on the server (embedded or sidecar), as LRC/plain text.
/// Uses the OpenSubsonic `getLyricsBySongId`; older servers simply return nothing.
pub fn lyrics(path: &str) -> Option<String> {
    let id = song_id(path)?;
    let s = server().ok()?;
    let r = call(&s, "getLyricsBySongId", &[("id", id)]).ok()?;
    let list = r.pointer("/lyricsList/structuredLyrics")?.as_array()?;
    let pick = list.iter().find(|l| l.get("synced").and_then(Value::as_bool) == Some(true)).or_else(|| list.first())?;
    let synced = pick.get("synced").and_then(Value::as_bool).unwrap_or(false);
    let offset = pick.get("offset").and_then(Value::as_i64).unwrap_or(0);
    let mut out = String::new();
    if synced && offset != 0 {
        out.push_str(&format!("[offset:{offset}]\n"));
    }
    for line in pick.get("line")?.as_array()? {
        let text = line.get("value").and_then(Value::as_str).unwrap_or("");
        match line.get("start").and_then(Value::as_i64).filter(|_| synced) {
            Some(ms) => out.push_str(&format!("[{:02}:{:02}.{:02}]{text}\n", ms / 60000, (ms / 1000) % 60, (ms % 1000) / 10)),
            None => {
                out.push_str(text);
                out.push('\n');
            }
        }
    }
    (!out.trim().is_empty()).then_some(out)
}

// ---------- streaming ----------

struct Progress {
    have: u64,
    done: bool,
    err: Option<String>,
}

struct Shared {
    id: String,
    file: PathBuf,
    len: Option<u64>,
    progress: Mutex<Progress>,
    cv: Condvar,
}

/// Songs with a live [`Stream`] (one entry per stream). The queue cache doesn't download these
/// a second time; a finished stream hands its file over instead.
static STREAMING: Mutex<Vec<String>> = Mutex::new(Vec::new());

pub fn is_streaming(id: &str) -> bool {
    STREAMING.lock().iter().any(|s| s == id)
}

impl Drop for Shared {
    fn drop(&mut self) {
        {
            let mut live = STREAMING.lock();
            if let Some(i) = live.iter().position(|s| *s == self.id) {
                live.swap_remove(i);
            }
        }
        // A complete download of an upcoming song moves into the queue cache instead.
        let p = self.progress.get_mut();
        let complete = p.done && p.err.is_none() && self.len.is_none_or(|l| l == p.have);
        if !(complete && crate::cache::adopt(&self.id, &self.file)) {
            let _ = std::fs::remove_file(&self.file);
        }
        // If the song is still wanted but wasn't handed over, the cache fetches it now.
        crate::cache::kick();
    }
}

/// Downloads a whole song to `dest` (for the queue cache). `keep_going` is polled between
/// chunks; returns Ok(false) when it asked to stop.
pub fn download(id: &str, dest: &std::path::Path, keep_going: impl Fn() -> bool) -> Result<bool> {
    let s = server()?;
    let mut res = stream_client()
        .get(format!("{}/rest/stream", base(&s)))
        .query(&auth(&s))
        .query(&[("id", id), ("format", "raw")])
        .send()
        .context("Can't reach the Navidrome server")?
        .error_for_status()?;
    if res.headers().get("content-type").and_then(|v| v.to_str().ok()).is_some_and(|c| c.contains("json") || c.contains("xml")) {
        bail!("Navidrome refused to stream this song");
    }
    let len = res.content_length();
    let mut w = File::create(dest)?;
    let mut buf = vec![0u8; 64 * 1024];
    let mut have = 0u64;
    loop {
        if !keep_going() {
            return Ok(false);
        }
        let n = res.read(&mut buf)?;
        if n == 0 {
            break;
        }
        w.write_all(&buf[..n])?;
        have += n as u64;
    }
    w.sync_all()?;
    if len.is_some_and(|l| l != have) {
        bail!("download ended early");
    }
    Ok(true)
}

/// A seekable media source over an HTTP download: a background thread writes the file to a
/// temporary file while the decoder reads from it, waiting only when it gets ahead.
pub struct Stream {
    shared: Arc<Shared>,
    f: File,
    pos: u64,
    len: Option<u64>,
}

const STALL: Duration = Duration::from_secs(30);

impl Stream {
    pub fn open(path: &str) -> Result<Self> {
        let id = song_id(path).ok_or_else(|| anyhow!("not a Navidrome track"))?;
        let s = server()?;
        let mut res = stream_client()
            .get(format!("{}/rest/stream", base(&s)))
            .query(&auth(&s))
            .query(&[("id", id), ("format", "raw")])
            .send()
            .context("Can't reach the Navidrome server")?
            .error_for_status()?;
        if res.headers().get("content-type").and_then(|v| v.to_str().ok()).is_some_and(|c| c.contains("json") || c.contains("xml")) {
            bail!("Navidrome refused to stream this song");
        }
        let len = res.content_length();
        static N: AtomicU64 = AtomicU64::new(0);
        let dir = STREAM_DIR.get().cloned().unwrap_or_else(std::env::temp_dir);
        let file = dir.join(format!("{}-{}.part", id.replace(|c: char| !c.is_ascii_alphanumeric(), "_"), N.fetch_add(1, Ordering::Relaxed)));
        let mut w = File::create(&file)?;
        let f = File::open(&file)?;
        STREAMING.lock().push(id.to_string());
        let shared = Arc::new(Shared { id: id.to_string(), file, len, progress: Mutex::new(Progress { have: 0, done: false, err: None }), cv: Condvar::new() });
        let weak = Arc::downgrade(&shared);
        std::thread::Builder::new().name("stream".into()).spawn(move || {
            let mut buf = vec![0u8; 64 * 1024];
            loop {
                // Stop downloading once the player has dropped the stream.
                let Some(sh) = weak.upgrade() else { return };
                let r = res.read(&mut buf).map_err(|e| e.to_string()).and_then(|n| w.write_all(&buf[..n]).map(|_| n).map_err(|e| e.to_string()));
                let mut p = sh.progress.lock();
                match r {
                    Ok(0) => p.done = true,
                    Ok(n) => p.have += n as u64,
                    Err(e) => {
                        p.err = Some(e);
                        p.done = true;
                    }
                }
                let done = p.done;
                drop(p);
                sh.cv.notify_all();
                if done {
                    return;
                }
            }
        })?;
        Ok(Self { shared, f, pos: 0, len })
    }

    /// Blocks until `want` bytes are downloaded (or the download ends). Returns bytes available.
    fn wait_for(&self, want: u64) -> std::io::Result<u64> {
        let mut p = self.shared.progress.lock();
        while p.have < want && !p.done {
            if self.shared.cv.wait_for(&mut p, STALL).timed_out() {
                return Err(std::io::Error::new(std::io::ErrorKind::TimedOut, "Navidrome stream stalled"));
            }
        }
        if p.have < want {
            if let Some(e) = &p.err {
                return Err(std::io::Error::other(e.clone()));
            }
        }
        Ok(p.have)
    }
}

impl Read for Stream {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        if buf.is_empty() {
            return Ok(0);
        }
        let have = self.wait_for(self.pos + 1)?;
        if have <= self.pos {
            return Ok(0);
        }
        let n = ((have - self.pos) as usize).min(buf.len());
        self.f.seek(SeekFrom::Start(self.pos))?;
        let n = self.f.read(&mut buf[..n])?;
        self.pos += n as u64;
        Ok(n)
    }
}

impl Seek for Stream {
    fn seek(&mut self, to: SeekFrom) -> std::io::Result<u64> {
        let pos = match to {
            SeekFrom::Start(p) => p as i64,
            SeekFrom::Current(d) => self.pos as i64 + d,
            SeekFrom::End(d) => {
                let len = match self.len {
                    Some(l) => l,
                    None => self.wait_for(u64::MAX)?,
                };
                len as i64 + d
            }
        };
        if pos < 0 {
            return Err(std::io::Error::new(std::io::ErrorKind::InvalidInput, "seek before start"));
        }
        self.pos = pos as u64;
        Ok(self.pos)
    }
}

impl symphonia::core::io::MediaSource for Stream {
    fn is_seekable(&self) -> bool {
        self.len.is_some()
    }
    fn byte_len(&self) -> Option<u64> {
        self.len
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_iso_dates() {
        assert_eq!(iso_ts("1970-01-01T00:00:00Z"), Some(0));
        assert_eq!(iso_ts("2024-03-01T12:34:56.789Z"), Some(1709296496));
    }

    #[test]
    fn ids_from_paths() {
        assert_eq!(song_id("navidrome://abc123.flac"), Some("abc123"));
        assert_eq!(song_id("/music/a.flac"), None);
        assert_eq!(cover_id("navidrome:al-xyz"), Some("al-xyz"));
    }

    #[test]
    fn virtual_dirs_follow_server_paths() {
        let s = Song { path: Some("Ilaiyaraaja/Album/01 - x.flac".into()), ..Default::default() };
        let sep = MAIN_SEPARATOR;
        assert_eq!(virtual_dir(&s), format!("Navidrome{sep}Ilaiyaraaja{sep}Album"));
        let s = Song { artist: "A/B".into(), album: "C".into(), ..Default::default() };
        assert_eq!(virtual_dir(&s), format!("Navidrome{sep}A_B{sep}C"));
    }

    /// Talks to the public demo server: `cargo test -- --ignored navidrome_demo --nocapture`.
    #[test]
    #[ignore]
    fn navidrome_demo() {
        let srv = Navidrome { active: true, url: "https://demo.navidrome.org".into(), username: "demo".into(), password: "demo".into() };
        println!("server: {}", ping(&srv).unwrap());
        configure(Some(srv));
        let dir = std::env::temp_dir().join(format!("coffi-nd-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        init(dir.clone());
        let mut conn = crate::library::db::open(&dir.join("nd.db")).unwrap();
        let t0 = std::time::Instant::now();
        let stats = sync(&mut conn).unwrap();
        println!("synced {} songs in {:?}", stats.total, t0.elapsed());
        let (path, dir_, cover): (String, String, Option<String>) =
            conn.query_row("SELECT path, dir, cover FROM tracks ORDER BY id LIMIT 1", [], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?))).unwrap();
        println!("first: {path} in {dir_}");
        let home = crate::library::home(&conn, &[]).unwrap();
        println!("albums {} artists {} playlists {}", home.albums.len(), home.artists.len(), home.server_playlists.len());
        let pl = &home.server_playlists[0];
        assert_eq!(pl.covers.len(), 1, "playlists use the server's own artwork");
        let t = crate::library::covers::thumb(&conn, &dir, &pl.covers[0], 256).unwrap();
        println!("playlist '{}' cover {} bytes", pl.name, std::fs::metadata(&t).unwrap().len());
        let c = crate::library::collection(&conn, "playlist", &pl.id, &[]).unwrap();
        assert_eq!(c.covers, pl.covers);
        let found = crate::library::search(&conn, "love").unwrap();
        println!("search 'love': {} songs {} albums {} artists {} playlists", found.songs.len(), found.albums.len(), found.artists.len(), found.playlists.len());
        let fv = crate::library::folder_view(&conn, ROOT).unwrap();
        println!("root folders {} covers {:?}", fv.folders.len(), fv.covers.len());
        let thumb = crate::library::covers::thumb(&conn, &dir, &cover.unwrap(), 256).unwrap();
        println!("thumb {:?} {} bytes", thumb, std::fs::metadata(&thumb).unwrap().len());
        let t0 = std::time::Instant::now();
        let mut dec = crate::audio::decoder::Decoder::open(std::path::Path::new(&path)).unwrap();
        let mut buf = Vec::new();
        for _ in 0..50 {
            dec.next_chunk(&mut buf).unwrap();
        }
        println!("decoded {} samples @ {} Hz, first audio after {:?}", buf.len(), dec.sample_rate, t0.elapsed());
        std::fs::remove_dir_all(&dir).ok();
    }
}
