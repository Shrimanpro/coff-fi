use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use anyhow::Result;
use lofty::file::FileType;
use lofty::picture::PictureType;
use lofty::prelude::*;
use rayon::prelude::*;
use rusqlite::{params, Connection};
use walkdir::WalkDir;
use xxhash_rust::xxh3::xxh3_64;

use super::db::now;

pub const AUDIO_EXTS: &[&str] = &[
    "mp3", "flac", "wav", "wave", "ogg", "oga", "m4a", "mp4", "m4b", "aac", "alac", "aif", "aiff", "aifc", "caf", "mka", "webm",
];
const COVER_NAMES: &[&str] = &["cover", "folder", "front", "album"];
const COVER_EXTS: &[&str] = &["jpg", "jpeg", "png", "webp"];

pub fn is_audio(p: &Path) -> bool {
    p.extension().and_then(|e| e.to_str()).is_some_and(|e| AUDIO_EXTS.contains(&e.to_ascii_lowercase().as_str()))
}

/// Returns the priority of a folder-cover filename (lower is better), `cover.jpg` first as the spec asks.
fn cover_rank(p: &Path) -> Option<usize> {
    let stem = p.file_stem()?.to_str()?.to_ascii_lowercase();
    let ext = p.extension()?.to_str()?.to_ascii_lowercase();
    let n = COVER_NAMES.iter().position(|c| *c == stem)?;
    let e = COVER_EXTS.iter().position(|c| *c == ext)?;
    Some(n * COVER_EXTS.len() + e)
}

fn mtime(p: &Path) -> i64 {
    std::fs::metadata(p)
        .and_then(|m| m.modified())
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

pub fn hex(h: u64) -> String {
    format!("{h:016x}")
}

pub fn file_cover_hash(p: &Path) -> String {
    hex(xxh3_64(format!("{}|{}", p.display(), mtime(p)).as_bytes()))
}

struct Scanned {
    path: String,
    dir: String,
    mtime: i64,
    title: String,
    artist: String,
    album: String,
    album_artist: String,
    track_no: Option<u32>,
    disc_no: Option<u32>,
    year: Option<u32>,
    duration: f64,
    codec: String,
    sample_rate: Option<u32>,
    bit_depth: Option<u32>,
    bitrate: Option<u32>,
    rg_track: Option<f32>,
    rg_album: Option<f32>,
    embedded_cover: Option<String>,
}

fn parse_gain(s: &str) -> Option<f32> {
    s.trim().trim_end_matches("dB").trim_end_matches("db").trim().parse().ok()
}

fn codec_name(ft: &FileType, bit_depth: Option<u8>) -> &'static str {
    match ft {
        FileType::Flac => "FLAC",
        FileType::Mpeg => "MP3",
        FileType::Wav => "WAV",
        FileType::Aiff => "AIFF",
        FileType::Vorbis => "OGG",
        FileType::Opus => "OPUS",
        FileType::Aac => "AAC",
        FileType::Mp4 if bit_depth.is_some() => "ALAC",
        FileType::Mp4 => "AAC",
        _ => "AUDIO",
    }
}

fn read_file(path: &Path, mtime: i64) -> Scanned {
    let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("Unknown").to_string();
    let dir = path.parent().map(|d| d.to_string_lossy().into_owned()).unwrap_or_default();
    let mut s = Scanned {
        path: path.to_string_lossy().into_owned(),
        dir,
        mtime,
        title: stem,
        artist: String::new(),
        album: String::new(),
        album_artist: String::new(),
        track_no: None,
        disc_no: None,
        year: None,
        duration: 0.0,
        codec: path.extension().and_then(|e| e.to_str()).unwrap_or("").to_ascii_uppercase(),
        sample_rate: None,
        bit_depth: None,
        bitrate: None,
        rg_track: None,
        rg_album: None,
        embedded_cover: None,
    };
    let Ok(file) = lofty::read_from_path(path) else { return s };
    let props = file.properties();
    s.duration = props.duration().as_secs_f64();
    s.sample_rate = props.sample_rate();
    s.bit_depth = props.bit_depth().map(u32::from);
    s.bitrate = props.audio_bitrate();
    s.codec = codec_name(&file.file_type(), props.bit_depth()).to_string();
    if let Some(tag) = file.primary_tag().or_else(|| file.first_tag()) {
        if let Some(t) = tag.title().filter(|t| !t.trim().is_empty()) {
            s.title = t.trim().to_string();
        }
        s.artist = tag.artist().map(|a| a.trim().to_string()).unwrap_or_default();
        s.album = tag.album().map(|a| a.trim().to_string()).unwrap_or_default();
        s.album_artist = tag.get_string(&ItemKey::AlbumArtist).map(|a| a.trim().to_string()).unwrap_or_default();
        s.track_no = tag.track();
        s.disc_no = tag.disk();
        s.year = tag.year();
        s.rg_track = tag.get_string(&ItemKey::ReplayGainTrackGain).and_then(parse_gain);
        s.rg_album = tag.get_string(&ItemKey::ReplayGainAlbumGain).and_then(parse_gain);
        let pics = tag.pictures();
        let pic = pics.iter().find(|p| p.pic_type() == PictureType::CoverFront).or_else(|| pics.first());
        s.embedded_cover = pic.map(|p| hex(xxh3_64(p.data())));
    }
    if s.artist.is_empty() {
        s.artist = "Unknown Artist".into();
    }
    if s.album_artist.is_empty() {
        s.album_artist = s.artist.clone();
    }
    s
}

/// Re-reads one file's tags into its existing row after the info panel edits it,
/// recording the new mtime so the next scan doesn't redo the work.
pub fn refresh_file(conn: &Connection, path: &str) -> Result<()> {
    let p = Path::new(path);
    let s = read_file(p, mtime(p));
    conn.execute(
        "UPDATE tracks SET mtime = ?2, title = ?3, artist = ?4, album = ?5, album_artist = ?6, track_no = ?7, disc_no = ?8, year = ?9 WHERE path = ?1",
        params![path, s.mtime, s.title, s.artist, s.album, s.album_artist, s.track_no, s.disc_no, s.year],
    )?;
    Ok(())
}

#[derive(Default, Debug, serde::Serialize)]
pub struct ScanStats {
    pub total: usize,
    pub updated: usize,
    pub removed: usize,
}

fn excluded(p: &Path, excl: &[PathBuf]) -> bool {
    excl.iter().any(|e| p.starts_with(e))
}

/// Incrementally syncs the database with the files under `roots`.
pub fn scan(conn: &mut Connection, roots: &[String], excluded_dirs: &[String]) -> Result<ScanStats> {
    let excl: Vec<PathBuf> = excluded_dirs.iter().map(PathBuf::from).collect();
    let known: HashMap<String, i64> = {
        let mut st = conn.prepare("SELECT path, mtime FROM tracks")?;
        let rows = st.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?)))?;
        rows.collect::<rusqlite::Result<_>>()?
    };

    let mut files: Vec<(PathBuf, i64)> = Vec::new();
    let mut folder_covers: HashMap<PathBuf, (usize, PathBuf)> = HashMap::new();
    for root in roots {
        let walker = WalkDir::new(root).follow_links(true).into_iter().filter_entry(|e| !excluded(e.path(), &excl));
        for entry in walker.filter_map(|e| e.ok()) {
            if !entry.file_type().is_file() {
                continue;
            }
            let p = entry.path();
            if is_audio(p) {
                let m = entry.metadata().ok().and_then(|m| m.modified().ok());
                let m = m.and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok()).map(|d| d.as_secs() as i64).unwrap_or(0);
                files.push((p.to_path_buf(), m));
            } else if let Some(rank) = cover_rank(p) {
                let dir = p.parent().unwrap_or(p).to_path_buf();
                let better = folder_covers.get(&dir).is_none_or(|(r, _)| rank < *r);
                if better {
                    folder_covers.insert(dir, (rank, p.to_path_buf()));
                }
            }
        }
    }

    let seen: HashSet<String> = files.iter().map(|(p, _)| p.to_string_lossy().into_owned()).collect();
    let changed: Vec<Scanned> = files
        .par_iter()
        .filter(|(p, m)| known.get(p.to_string_lossy().as_ref()) != Some(m))
        .map(|(p, m)| read_file(p, *m))
        .collect();

    let stats = ScanStats {
        total: files.len(),
        updated: changed.len(),
        removed: known.keys().filter(|k| !seen.contains(*k)).count(),
    };

    let tx = conn.transaction()?;
    {
        let mut del = tx.prepare("DELETE FROM tracks WHERE path = ?1")?;
        for k in known.keys().filter(|k| !seen.contains(*k)) {
            del.execute([k])?;
        }
        let mut cover = tx.prepare("INSERT OR IGNORE INTO covers(hash, path, embedded) VALUES(?1, ?2, ?3)")?;
        tx.execute("DELETE FROM folder_covers", [])?;
        let mut fc = tx.prepare("INSERT INTO folder_covers(dir, hash) VALUES(?1, ?2)")?;
        let mut folder_hash: HashMap<String, String> = HashMap::new();
        for (dir, (_, path)) in &folder_covers {
            let h = file_cover_hash(path);
            cover.execute(params![h, path.to_string_lossy(), 0])?;
            fc.execute(params![dir.to_string_lossy(), h])?;
            folder_hash.insert(dir.to_string_lossy().into_owned(), h);
        }
        let mut up = tx.prepare(
            "INSERT INTO tracks(path, dir, mtime, title, artist, album, album_artist, track_no, disc_no, year, duration, codec, sample_rate, bit_depth, bitrate, rg_track, rg_album, cover, added_at)
             VALUES(?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19)
             ON CONFLICT(path) DO UPDATE SET mtime = excluded.mtime, title = excluded.title, artist = excluded.artist, album = excluded.album,
               album_artist = excluded.album_artist, track_no = excluded.track_no, disc_no = excluded.disc_no, year = excluded.year,
               duration = excluded.duration, codec = excluded.codec, sample_rate = excluded.sample_rate, bit_depth = excluded.bit_depth,
               bitrate = excluded.bitrate, rg_track = excluded.rg_track, rg_album = excluded.rg_album, cover = excluded.cover",
        )?;
        // On the very first import use file times so "Recently Added" is meaningful straight away.
        let first_import = known.is_empty();
        let ts = now();
        for s in &changed {
            let added = if first_import { s.mtime } else { ts };
            let cover_hash = match &s.embedded_cover {
                Some(h) => {
                    cover.execute(params![h, s.path, 1])?;
                    Some(h.clone())
                }
                None => folder_hash.get(&s.dir).cloned(),
            };
            up.execute(params![
                s.path, s.dir, s.mtime, s.title, s.artist, s.album, s.album_artist, s.track_no, s.disc_no, s.year, s.duration,
                s.codec, s.sample_rate, s.bit_depth, s.bitrate, s.rg_track, s.rg_album, cover_hash, added
            ])?;
        }
        // Unchanged tracks without embedded art may have gained or lost a folder cover.
        tx.execute(
            "UPDATE tracks SET cover = (SELECT hash FROM folder_covers f WHERE f.dir = tracks.dir)
             WHERE cover IS NULL OR cover NOT IN (SELECT hash FROM covers WHERE embedded = 1)",
            [],
        )?;
        tx.execute("DELETE FROM covers WHERE hash NOT IN (SELECT cover FROM tracks WHERE cover IS NOT NULL) AND hash NOT IN (SELECT hash FROM folder_covers)", [])?;
    }
    tx.commit()?;
    Ok(stats)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cover_rank_prefers_cover_jpg() {
        assert_eq!(cover_rank(Path::new("/a/cover.jpg")), Some(0));
        assert!(cover_rank(Path::new("/a/Folder.PNG")).unwrap() > 0);
        assert_eq!(cover_rank(Path::new("/a/song.jpg")), None);
    }

    #[test]
    fn gain_parsing() {
        assert_eq!(parse_gain("-6.50 dB"), Some(-6.5));
        assert_eq!(parse_gain("+1.2"), Some(1.2));
        assert_eq!(parse_gain("x"), None);
    }

    #[test]
    fn incremental_scan() {
        let dir = std::env::temp_dir().join(format!("coffi-scan-{}", std::process::id()));
        std::fs::create_dir_all(dir.join("sub")).unwrap();
        // Not decodable, but the scanner must still index it by filename.
        std::fs::write(dir.join("sub/a.mp3"), b"not really audio").unwrap();
        std::fs::write(dir.join("sub/cover.jpg"), b"jpg").unwrap();
        let mut conn = super::super::db::open(&dir.join("t.db")).unwrap();
        let roots = vec![dir.to_string_lossy().into_owned()];
        let s1 = scan(&mut conn, &roots, &[]).unwrap();
        assert_eq!((s1.total, s1.updated, s1.removed), (1, 1, 0));
        let s2 = scan(&mut conn, &roots, &[]).unwrap();
        assert_eq!((s2.total, s2.updated, s2.removed), (1, 0, 0));
        let cover: Option<String> = conn.query_row("SELECT cover FROM tracks", [], |r| r.get(0)).unwrap();
        assert!(cover.is_some(), "folder cover should be used when no embedded art");
        std::fs::remove_file(dir.join("sub/a.mp3")).unwrap();
        let s3 = scan(&mut conn, &roots, &[]).unwrap();
        assert_eq!((s3.total, s3.removed), (0, 1));
        std::fs::remove_dir_all(&dir).ok();
    }
}
