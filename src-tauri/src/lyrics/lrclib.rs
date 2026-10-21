use std::collections::HashSet;

use anyhow::{anyhow, Result};
use reqwest::blocking::Client;
use reqwest::StatusCode;
use serde::de::DeserializeOwned;
use serde::Deserialize;

use super::matching::{clean_title, primary_artist, same_length, same_title};
use super::{parse, Lyrics, Query, Synced};

const API: &str = "https://lrclib.net/api";
const SOURCE: &str = "LRCLIB";

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Rec {
    #[serde(default)]
    id: i64,
    #[serde(default)]
    track_name: String,
    #[serde(default)]
    duration: f64,
    #[serde(default)]
    instrumental: bool,
    plain_lyrics: Option<String>,
    synced_lyrics: Option<String>,
}

impl Rec {
    fn has_synced(&self) -> bool {
        self.synced_lyrics.as_deref().is_some_and(|s| !s.trim().is_empty())
    }
}

fn synced(r: &Rec) -> Option<Lyrics> {
    let (synced, lines) = r.synced_lyrics.as_deref().and_then(parse::lrc)?;
    Some(Lyrics { source: SOURCE.into(), synced, lines })
}

fn plain(r: &Rec) -> Option<Lyrics> {
    if let Some(p) = r.plain_lyrics.as_deref().filter(|p| !p.trim().is_empty()) {
        return Some(Lyrics { source: SOURCE.into(), synced: Synced::None, lines: parse::plain(p) });
    }
    r.instrumental.then(|| Lyrics { source: SOURCE.into(), synced: Synced::None, lines: Vec::new() })
}

/// GET a JSON endpoint. `Ok(None)` means 404.
fn get<T: DeserializeOwned>(client: &Client, path: &str, params: &[(&str, &str)]) -> Result<Option<T>> {
    match client.get(format!("{API}/{path}")).query(params).send()? {
        res if res.status() == StatusCode::NOT_FOUND => Ok(None),
        res if res.status().is_success() => Ok(Some(res.json()?)),
        res => Err(anyhow!("LRCLIB answered {}", res.status())),
    }
}

/// Candidates gathered from several lookups, de-duplicated. Remembers whether every lookup failed.
struct Found {
    recs: Vec<Rec>,
    seen: HashSet<i64>,
    ok: bool,
    err: Option<anyhow::Error>,
}

impl Found {
    fn add(&mut self, r: Result<Vec<Rec>>, title: &str) {
        match r {
            Ok(list) => {
                self.ok = true;
                for rec in list {
                    if same_title(title, &rec.track_name) && (rec.id == 0 || self.seen.insert(rec.id)) {
                        self.recs.push(rec);
                    }
                }
            }
            Err(e) => {
                eprintln!("lrclib: {e:#}");
                self.err = Some(e);
            }
        }
    }

    /// A synced version of the same length (anything else would drift out of time).
    fn best_synced(&self, dur: f64) -> Option<&Rec> {
        self.recs
            .iter()
            .filter(|r| r.has_synced() && same_length(dur, r.duration, 4.0))
            .min_by(|a, b| (a.duration - dur).abs().total_cmp(&(b.duration - dur).abs()))
    }
}

pub fn fetch(client: &Client, q: &Query) -> Result<Option<Lyrics>> {
    let dur = (q.duration.round() as i64).to_string();
    let title = clean_title(q.title);
    let artist = primary_artist(q.artist);
    let mut f = Found { recs: Vec::new(), seen: HashSet::new(), ok: false, err: None };

    // Cheapest first; stop as soon as a matching synced version turns up.
    // 1. Exact signature, LRCLIB's own database only (fast, no external lookups).
    let exact = [("track_name", q.title), ("artist_name", q.artist), ("album_name", q.album), ("duration", dur.as_str())];
    f.add(get::<Rec>(client, "get-cached", &exact).map(|r| r.into_iter().collect()), q.title);
    // 2. Searches, from most to least specific.
    let title_artist = format!("{title} {artist}");
    let searches: [&[(&str, &str)]; 3] =
        [&[("track_name", &title), ("artist_name", &artist)], &[("q", &title_artist)], &[("q", &title)]];
    for params in searches {
        if f.best_synced(q.duration).is_some() {
            break;
        }
        f.add(get::<Vec<Rec>>(client, "search", params).map(Option::unwrap_or_default), q.title);
    }
    // 3. Last resort: the exact lookup that may query external providers (slow).
    if f.best_synced(q.duration).is_none() {
        f.add(get::<Rec>(client, "get", &exact).map(|r| r.into_iter().collect()), q.title);
    }

    if let Some(l) = f.best_synced(q.duration).and_then(synced) {
        return Ok(Some(l));
    }
    if !f.ok {
        // Every request failed: report it so the result isn't cached as "no lyrics".
        return Err(f.err.unwrap_or_else(|| anyhow!("LRCLIB unreachable")));
    }
    let close = f.recs.iter().filter(|r| same_length(q.duration, r.duration, 4.0)).find_map(plain);
    // Words rarely change between versions, so plain lyrics of another cut are acceptable.
    Ok(close.or_else(|| f.recs.iter().find_map(plain)))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Live check against lrclib.net: `cargo test -- --ignored lrclib_live --nocapture`.
    #[test]
    #[ignore]
    fn lrclib_live() {
        let q = Query { title: "Nee Pottu Vechcha (From \"Ponmaana Selvan\")", artist: "Mano, Malaysia Vasudevan, K.S. Chithra & Ilaiyaraaja", album: "Ponmaana Selvan", duration: 266.0 };
        for _ in 0..3 {
            let l = fetch(&super::super::http(), &q).unwrap().expect("lyrics");
            println!("{:?} {} lines", l.synced, l.lines.len());
            assert_ne!(l.synced, Synced::None);
        }
    }
}
