//! Reading and writing the user-editable tags of local files (the song info panel).

use std::path::Path;

use anyhow::{Context, Result};
use lofty::config::WriteOptions;
use lofty::prelude::*;
use lofty::tag::Tag;
use serde::{Deserialize, Serialize};

#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Tags {
    pub title: String,
    pub artist: String,
    pub album: String,
    pub album_artist: String,
    pub genre: String,
    pub track_no: Option<u32>,
    pub track_total: Option<u32>,
    pub disc_no: Option<u32>,
    pub disc_total: Option<u32>,
    pub year: Option<u32>,
}

#[derive(Debug, Serialize)]
pub struct FileInfo {
    pub tags: Tags,
    pub size: u64,
    pub read_only: bool,
}

pub fn read(path: &Path) -> Result<FileInfo> {
    let meta = std::fs::metadata(path).context("The file can't be found")?;
    let file = lofty::read_from_path(path).context("Couldn't read the file's tags")?;
    let mut tags = Tags::default();
    // Same tag the scanner reads, so the editor shows what the library shows.
    if let Some(t) = file.primary_tag().or_else(|| file.first_tag()) {
        let text = |v: Option<std::borrow::Cow<str>>| v.map(|x| x.trim().to_string()).unwrap_or_default();
        tags.title = text(t.title());
        tags.artist = text(t.artist());
        tags.album = text(t.album());
        tags.album_artist = t.get_string(&ItemKey::AlbumArtist).map(|a| a.trim().to_string()).unwrap_or_default();
        tags.genre = text(t.genre());
        tags.track_no = t.track();
        tags.track_total = t.track_total();
        tags.disc_no = t.disk();
        tags.disc_total = t.disk_total();
        tags.year = t.year();
    }
    Ok(FileInfo { tags, size: meta.len(), read_only: meta.permissions().readonly() })
}

fn text(tag: &mut Tag, key: ItemKey, v: &str) {
    match v.trim() {
        "" => tag.remove_key(&key),
        v => {
            tag.insert_text(key, v.to_string());
        }
    }
}

pub fn write(path: &Path, tags: &Tags) -> Result<()> {
    let mut file = lofty::read_from_path(path).context("Couldn't read the file's tags")?;
    // Files without the format's native tag get one, seeded from whatever tag they had.
    if file.primary_tag().is_none() {
        let mut fresh = Tag::new(file.primary_tag_type());
        if let Some(old) = file.first_tag() {
            for item in old.items() {
                fresh.insert(item.clone());
            }
            for pic in old.pictures() {
                fresh.push_picture(pic.clone());
            }
        }
        file.insert_tag(fresh);
    }
    let tag = file.primary_tag_mut().context("This file type doesn't support tags")?;
    text(tag, ItemKey::TrackTitle, &tags.title);
    text(tag, ItemKey::TrackArtist, &tags.artist);
    text(tag, ItemKey::AlbumTitle, &tags.album);
    text(tag, ItemKey::AlbumArtist, &tags.album_artist);
    text(tag, ItemKey::Genre, &tags.genre);
    match tags.track_no {
        Some(n) => tag.set_track(n),
        None => tag.remove_track(),
    }
    match tags.track_total {
        Some(n) => tag.set_track_total(n),
        None => tag.remove_track_total(),
    }
    match tags.disc_no {
        Some(n) => tag.set_disk(n),
        None => tag.remove_disk(),
    }
    match tags.disc_total {
        Some(n) => tag.set_disk_total(n),
        None => tag.remove_disk_total(),
    }
    match tags.year {
        Some(n) => tag.set_year(n),
        None => tag.remove_year(),
    }
    tag.save_to_path(path, WriteOptions::default()).context("Couldn't save the tags")?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A tiny silent 16-bit mono WAV.
    fn wav(path: &Path) {
        let data = vec![0u8; 800];
        let mut b = Vec::new();
        b.extend_from_slice(b"RIFF");
        b.extend_from_slice(&(36 + data.len() as u32).to_le_bytes());
        b.extend_from_slice(b"WAVEfmt ");
        b.extend_from_slice(&16u32.to_le_bytes());
        b.extend_from_slice(&[1, 0, 1, 0]);
        b.extend_from_slice(&8000u32.to_le_bytes());
        b.extend_from_slice(&16000u32.to_le_bytes());
        b.extend_from_slice(&[2, 0, 16, 0]);
        b.extend_from_slice(b"data");
        b.extend_from_slice(&(data.len() as u32).to_le_bytes());
        b.extend_from_slice(&data);
        std::fs::write(path, b).unwrap();
    }

    #[test]
    fn round_trips_and_clears() {
        let dir = std::env::temp_dir().join(format!("coffi-tags-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let p = dir.join("song.wav");
        wav(&p);

        let t = Tags {
            title: "Night Owl".into(),
            artist: "Somebody".into(),
            album: "Late".into(),
            album_artist: "Various".into(),
            genre: "Lo-fi".into(),
            track_no: Some(3),
            track_total: Some(12),
            disc_no: Some(1),
            disc_total: Some(2),
            year: Some(2021),
        };
        write(&p, &t).unwrap();
        let r = read(&p).unwrap().tags;
        assert_eq!((r.title.as_str(), r.artist.as_str(), r.album.as_str()), ("Night Owl", "Somebody", "Late"));
        assert_eq!((r.album_artist.as_str(), r.genre.as_str()), ("Various", "Lo-fi"));
        assert_eq!((r.track_no, r.track_total, r.disc_no, r.disc_total, r.year), (Some(3), Some(12), Some(1), Some(2), Some(2021)));

        write(&p, &Tags { title: "Renamed".into(), ..Default::default() }).unwrap();
        let r = read(&p).unwrap().tags;
        assert_eq!(r.title, "Renamed");
        assert!(r.artist.is_empty() && r.genre.is_empty() && r.track_no.is_none() && r.year.is_none());
        std::fs::remove_dir_all(&dir).ok();
    }
}
