//! Reading and writing the user-editable tags of local files (the song info panel).

use std::path::Path;

use anyhow::{Context, Result};
use lofty::prelude::*;
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

