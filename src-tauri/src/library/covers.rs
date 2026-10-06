use std::path::{Path, PathBuf};

use anyhow::{anyhow, Context, Result};
use image::imageops::FilterType;
use image::{DynamicImage, GenericImageView, RgbImage};
use lofty::picture::PictureType;
use lofty::prelude::*;
use rusqlite::{Connection, OptionalExtension};
use xxhash_rust::xxh3::xxh3_64;

/// Thumbnails are snapped to these sizes so the disk cache stays small and hits often.
const SIZES: [u32; 3] = [96, 256, 640];

pub fn snap(size: u32) -> u32 {
    SIZES.iter().copied().find(|s| *s >= size).unwrap_or(SIZES[SIZES.len() - 1])
}

fn source_bytes(conn: &Connection, hash: &str) -> Result<Vec<u8>> {
    let (path, embedded): (String, bool) = conn
        .query_row("SELECT path, embedded FROM covers WHERE hash = ?1", [hash], |r| Ok((r.get(0)?, r.get(1)?)))
        .optional()?
        .ok_or_else(|| anyhow!("unknown cover {hash}"))?;
    if !embedded {
        return Ok(std::fs::read(&path)?);
    }
    let file = lofty::read_from_path(&path)?;
    let tag = file.primary_tag().or_else(|| file.first_tag()).context("no tag")?;
    let pics = tag.pictures();
    let pic = pics.iter().find(|p| p.pic_type() == PictureType::CoverFront).or_else(|| pics.first()).context("no picture")?;
    Ok(pic.data().to_vec())
}

fn save_jpeg(img: &DynamicImage, path: &Path) -> Result<()> {
    let tmp = path.with_extension("tmp");
    let mut f = std::io::BufWriter::new(std::fs::File::create(&tmp)?);
    image::codecs::jpeg::JpegEncoder::new_with_quality(&mut f, 86).encode_image(&img.to_rgb8())?;
    drop(f);
    std::fs::rename(tmp, path)?;
    Ok(())
}

fn load_square(conn: &Connection, hash: &str, size: u32) -> Result<DynamicImage> {
    let img = image::load_from_memory(&source_bytes(conn, hash)?)?;
    Ok(img.resize_to_fill(size, size, FilterType::Triangle))
}

/// Cached square thumbnail of a single cover.
pub fn thumb(conn: &Connection, cache: &Path, hash: &str, size: u32) -> Result<PathBuf> {
    let size = snap(size);
    let out = cache.join(format!("{hash}_{size}.jpg"));
    if !out.exists() {
        save_jpeg(&load_square(conn, hash, size)?, &out)?;
    }
    Ok(out)
}

/// A 2×2 collage of up to four covers; fewer covers are cycled to fill the grid.
pub fn collage(conn: &Connection, cache: &Path, hashes: &[String], size: u32) -> Result<PathBuf> {
    match hashes.len() {
        0 => return Err(anyhow!("empty collage")),
        1 => return thumb(conn, cache, &hashes[0], size),
        _ => {}
    }
    let size = snap(size);
    let key = xxh3_64(hashes.join(",").as_bytes());
    let out = cache.join(format!("c{key:016x}_{size}.jpg"));
    if out.exists() {
        return Ok(out);
    }
    let half = size / 2;
    let mut canvas = RgbImage::new(size, size);
    // Two covers form a checkerboard; otherwise cycle through them.
    let order = if hashes.len() == 2 { [0, 1, 1, 0] } else { [0, 1, 2, 3].map(|i| i % hashes.len()) };
    for (i, &o) in order.iter().enumerate() {
        let h = &hashes[o];
        let tile = image::open(thumb(conn, cache, h, half)?)?.resize_to_fill(half, half, FilterType::Triangle).to_rgb8();
        image::imageops::replace(&mut canvas, &tile, ((i % 2) as u32 * half) as i64, ((i / 2) as u32 * half) as i64);
    }
    save_jpeg(&DynamicImage::ImageRgb8(canvas), &out)?;
    Ok(out)
}

/// A tiny pre-blurred version of a cover used as the fullscreen backdrop (scaled up by CSS).
pub fn blurred(conn: &Connection, cache: &Path, hash: &str) -> Result<PathBuf> {
    let out = cache.join(format!("b{hash}.jpg"));
    if !out.exists() {
        let small = image::open(thumb(conn, cache, hash, 96)?)?.resize_exact(64, 64, FilterType::Triangle);
        save_jpeg(&small.blur(5.0), &out)?;
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snapping() {
        assert_eq!(snap(10), 96);
        assert_eq!(snap(200), 256);
        assert_eq!(snap(5000), 640);
    }

    #[test]
    fn collage_cycles_covers() {
        let dir = std::env::temp_dir().join(format!("coffi-cov-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let conn = super::super::db::open(&dir.join("t.db")).unwrap();
        for (name, col) in [("a", [200u8, 0, 0]), ("b", [0, 0, 200])] {
            let p = dir.join(format!("{name}.png"));
            RgbImage::from_pixel(50, 50, image::Rgb(col)).save(&p).unwrap();
            conn.execute("INSERT INTO covers VALUES(?1, ?2, 0)", rusqlite::params![name, p.to_string_lossy()]).unwrap();
        }
        let out = collage(&conn, &dir, &["a".into(), "b".into()], 256).unwrap();
        let img = image::open(out).unwrap().to_rgb8();
        assert!(img.get_pixel(10, 10)[0] > 150, "top-left is cover a");
        assert!(img.get_pixel(200, 10)[2] > 150, "top-right is cover b");
        assert!(img.get_pixel(200, 200)[0] > 150, "bottom-right cycles back to a");
        std::fs::remove_dir_all(&dir).ok();
    }
}
