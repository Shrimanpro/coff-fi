pub mod covers;
pub mod db;
pub mod scan;
pub mod tags;
pub mod watch;

use std::collections::{BTreeMap, HashMap};
use std::path::MAIN_SEPARATOR;

use anyhow::Result;
use rusqlite::Connection;
use serde::Serialize;

use db::{tracks_where, Track};

#[derive(Serialize, Clone, Debug)]
pub struct Tile {
    pub kind: &'static str,
    pub id: String,
    pub name: String,
    pub subtitle: String,
    pub count: usize,
    pub covers: Vec<String>,
}

#[derive(Serialize)]
pub struct Home {
    pub albums: Vec<Tile>,
    pub artists: Vec<Tile>,
    pub songs: Vec<Track>,
}

#[derive(Serialize)]
pub struct FolderView {
    pub path: String,
    pub name: String,
    /// The folder's own cover.jpg, else up to 4 distinct covers from its songs (collage).
    pub covers: Vec<String>,
    pub folders: Vec<Tile>,
    pub tracks: Vec<Track>,
}

#[derive(Serialize)]
pub struct Collection {
    pub kind: String,
    pub id: String,
    pub title: String,
    pub subtitle: String,
    pub covers: Vec<String>,
    pub tracks: Vec<Track>,
}

pub fn first_covers<'a>(it: impl IntoIterator<Item = &'a Option<String>>, n: usize) -> Vec<String> {
    let mut out: Vec<String> = Vec::with_capacity(n);
    for c in it.into_iter().flatten() {
        if !out.contains(c) {
            out.push(c.clone());
            if out.len() == n {
                break;
            }
        }
    }
    out
}

pub fn folder_name(path: &str) -> String {
    std::path::Path::new(path).file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| path.to_string())
}

fn norm_dir(path: &str) -> String {
    let t = path.trim_end_matches(['/', '\\']);
    if t.is_empty() { path.to_string() } else { t.to_string() }
}

fn group_tiles(conn: &Connection, kind: &'static str, sql: &str) -> Result<Vec<Tile>> {
    group_tiles_p(conn, kind, sql, [])
}

fn group_tiles_p(conn: &Connection, kind: &'static str, sql: &str, p: impl rusqlite::Params) -> Result<Vec<Tile>> {
    let mut st = conn.prepare_cached(sql)?;
    let rows = st.query_map(p, |r| {
        let name: String = r.get(0)?;
        let sub: String = r.get(1)?;
        let id = if kind == "album" { format!("{sub}\u{1f}{name}") } else { name.clone() };
        Ok(Tile { kind, id, name, subtitle: sub, count: r.get::<_, i64>(2)? as usize, covers: r.get::<_, Option<String>>(3)?.into_iter().collect() })
    })?;
    Ok(rows.collect::<rusqlite::Result<_>>()?)
}

pub fn home(conn: &Connection) -> Result<Home> {
    let albums = group_tiles(
        conn,
        "album",
        "SELECT album, album_artist, COUNT(*), MAX(cover) FROM tracks WHERE album != '' GROUP BY album, album_artist ORDER BY album COLLATE NOCASE",
    )?;
    let artists = group_tiles(
        conn,
        "artist",
        "SELECT artist, '', COUNT(*), MAX(cover) FROM tracks GROUP BY artist ORDER BY artist COLLATE NOCASE",
    )?;
    let songs = tracks_where(conn, "ORDER BY t.title COLLATE NOCASE", [])?;
    Ok(Home { albums, artists, songs })
}

#[derive(Serialize)]
pub struct SearchResults {
    pub songs: Vec<Track>,
    pub albums: Vec<Tile>,
    pub artists: Vec<Tile>,
    pub playlists: Vec<Tile>,
}

fn like_escape(s: &str) -> String {
    s.replace('\\', "\\\\").replace('%', "\\%").replace('_', "\\_")
}

/// Finds songs, albums, artists and playlists. Every word of the query must match; results
/// whose name starts with the query rank first.
pub fn search(conn: &Connection, query: &str) -> Result<SearchResults> {
    let words: Vec<String> = query.split_whitespace().map(|w| format!("%{}%", like_escape(w))).collect();
    if words.is_empty() {
        return Ok(SearchResults { songs: Vec::new(), albums: Vec::new(), artists: Vec::new(), playlists: Vec::new() });
    }
    let prefix = format!("{}%", like_escape(query.trim()));
    let all = |cols: &str| (0..words.len()).map(|i| format!("({cols}) LIKE ?{} ESCAPE '\\'", i + 2)).collect::<Vec<_>>().join(" AND ");
    let mut p: Vec<&dyn rusqlite::ToSql> = vec![&prefix];
    p.extend(words.iter().map(|w| w as &dyn rusqlite::ToSql));

    let songs = tracks_where(
        conn,
        &format!(
            "WHERE {} ORDER BY (t.title LIKE ?1 ESCAPE '\\') DESC, t.title COLLATE NOCASE LIMIT 40",
            all("t.title || ' ' || t.artist || ' ' || t.album")
        ),
        p.as_slice(),
    )?;
    let albums = group_tiles_p(
        conn,
        "album",
        &format!(
            "SELECT album, album_artist, COUNT(*), MAX(cover) FROM tracks WHERE album != '' AND {} GROUP BY album, album_artist
             ORDER BY (album LIKE ?1 ESCAPE '\\') DESC, album COLLATE NOCASE LIMIT 16",
            all("album || ' ' || album_artist")
        ),
        p.as_slice(),
    )?;
    let artists = group_tiles_p(
        conn,
        "artist",
        &format!(
            "SELECT artist, '', COUNT(*), MAX(cover) FROM tracks WHERE {} GROUP BY artist
             ORDER BY (artist LIKE ?1 ESCAPE '\\') DESC, artist COLLATE NOCASE LIMIT 12",
            all("artist")
        ),
        p.as_slice(),
    )?;

    let mut playlists = Vec::new();
    Ok(SearchResults { songs, albums, artists, playlists })
}

/// Immediate sub-folders (that contain music somewhere below) and the loose tracks of `path`.
pub fn folder_view(conn: &Connection, path: &str) -> Result<FolderView> {
    let path = norm_dir(path);
    let prefix = format!("{path}{MAIN_SEPARATOR}");
    let mut children: BTreeMap<String, (usize, Vec<Option<String>>)> = BTreeMap::new();
    {
        let mut st = conn.prepare_cached("SELECT dir, cover FROM tracks WHERE substr(dir, 1, ?1) = ?2 ORDER BY path")?;
        let rows = st.query_map(rusqlite::params![prefix.chars().count() as i64, prefix], |r| {
            Ok((r.get::<_, String>(0)?, r.get::<_, Option<String>>(1)?))
        })?;
        for row in rows {
            let (dir, cover) = row?;
            let rest = &dir[prefix.len()..];
            let child = rest.split(MAIN_SEPARATOR).next().unwrap_or(rest).to_string();
            let e = children.entry(child).or_default();
            e.0 += 1;
            if e.1.len() < 64 {
                e.1.push(cover);
            }
        }
    }
    let folder_covers: HashMap<String, String> = {
        let mut st = conn.prepare_cached("SELECT dir, hash FROM folder_covers WHERE substr(dir, 1, ?1) = ?2")?;
        let rows = st.query_map(rusqlite::params![prefix.chars().count() as i64, prefix], |r| Ok((r.get(0)?, r.get(1)?)))?;
        rows.collect::<rusqlite::Result<_>>()?
    };
    let mut folders: Vec<Tile> = children
        .into_iter()
        .map(|(name, (count, covers))| {
            let full = format!("{prefix}{name}");
            let covers = match folder_covers.get(&full) {
                Some(h) => vec![h.clone()],
                None => first_covers(covers.iter(), 4),
            };
            Tile { kind: "folder", id: full, name, subtitle: format!("{count} songs"), count, covers }
        })
        .collect();
    folders.sort_by_key(|t| t.name.to_lowercase());
    let tracks = tracks_where(conn, "WHERE t.dir = ?1 ORDER BY t.disc_no, t.track_no, t.path", [&path])?;
    let covers = match db_folder_cover(conn, &path)? {
        Some(h) => vec![h],
        None => {
            let own = tracks.iter().map(|t| &t.cover);
            let sub = folders.iter().flat_map(|f| f.covers.first()).map(|c| Some(c.clone())).collect::<Vec<_>>();
            first_covers(own.chain(sub.iter()), 4)
        }
    };
    Ok(FolderView { name: folder_name(&path), path, covers, folders, tracks })
}

pub fn collection(conn: &Connection, kind: &str, id: &str) -> Result<Collection> {
    let (title, subtitle, tracks) = match kind {
        "album" => {
            let (artist, album) = id.split_once('\u{1f}').unwrap_or(("", id));
            let t = tracks_where(conn, "WHERE t.album = ?1 AND t.album_artist = ?2 ORDER BY t.disc_no, t.track_no, t.path", [album, artist])?;
            (album.to_string(), artist.to_string(), t)
        }
        "artist" => (id.to_string(), "Artist".into(), tracks_where(conn, "WHERE t.artist = ?1 ORDER BY t.album, t.disc_no, t.track_no", [id])?),
        _ => {
            let dir = norm_dir(id);
            let prefix = format!("{dir}{MAIN_SEPARATOR}");
            let t = tracks_where(
                conn,
                "WHERE t.dir = ?1 OR substr(t.dir, 1, ?2) = ?3 ORDER BY t.path",
                rusqlite::params![dir, prefix.chars().count() as i64, prefix],
            )?;
            (folder_name(&dir), "Folder".into(), t)
        }
    };
    let covers = if kind == "folder" {
        db_folder_cover(conn, id)?.map(|h| vec![h]).unwrap_or_else(|| first_covers(tracks.iter().map(|t| &t.cover), 4))
    } else if kind == "album" {
        first_covers(tracks.iter().map(|t| &t.cover), 1)
    } else {
        first_covers(tracks.iter().map(|t| &t.cover), 4)
    };
    Ok(Collection { kind: kind.into(), id: id.into(), title, subtitle: format!("{subtitle} · {} songs", tracks.len()), covers, tracks })
}

fn db_folder_cover(conn: &Connection, dir: &str) -> Result<Option<String>> {
    use rusqlite::OptionalExtension;
    Ok(conn.query_row("SELECT hash FROM folder_covers WHERE dir = ?1", [norm_dir(dir)], |r| r.get(0)).optional()?)
}

pub fn root_tiles(conn: &Connection, roots: &[String]) -> Result<Vec<Tile>> {
    roots
        .iter()
        .map(|r| {
            let c = collection(conn, "folder", r)?;
            Ok(Tile { kind: "folder", id: norm_dir(r), name: c.title, subtitle: c.subtitle, count: c.tracks.len(), covers: c.covers })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn covers_are_distinct_and_capped() {
        let v = vec![Some("a".to_string()), None, Some("a".into()), Some("b".into()), Some("c".into()), Some("d".into()), Some("e".into())];
        assert_eq!(first_covers(v.iter(), 4), vec!["a", "b", "c", "d"]);
    }
}
