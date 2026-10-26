use std::collections::BTreeMap;

use anyhow::{bail, Result};
use serde_json::Value;

use crate::settings::LastFm;

const API: &str = "https://ws.audioscrobbler.com/2.0/";

/// Last.fm signature: md5 of sorted name+value pairs followed by the secret.
fn sign(params: &BTreeMap<&str, String>, secret: &str) -> String {
    let mut s: String = params.iter().filter(|(k, _)| **k != "format").map(|(k, v)| format!("{k}{v}")).collect();
    s.push_str(secret);
    format!("{:x}", md5::compute(s))
}

fn post(cfg_key: &str, secret: &str, mut params: BTreeMap<&str, String>) -> Result<Value> {
    params.insert("api_key", cfg_key.to_string());
    params.insert("format", "json".into());
    let sig = sign(&params, secret);
    params.insert("api_sig", sig);
    let v: Value = crate::lyrics::http().post(API).form(&params).send()?.json()?;
    if let Some(msg) = v.get("message").and_then(Value::as_str) {
        if v.get("error").is_some() {
            bail!("Last.fm: {msg}");
        }
    }
    Ok(v)
}

pub fn login(api_key: &str, secret: &str, user: &str, password: &str) -> Result<String> {
    let mut p = BTreeMap::new();
    p.insert("method", "auth.getMobileSession".into());
    p.insert("username", user.to_string());
    p.insert("password", password.to_string());
    let v = post(api_key, secret, p)?;
    match v.pointer("/session/key").and_then(Value::as_str) {
        Some(k) => Ok(k.to_string()),
        None => bail!("Last.fm login failed"),
    }
}

pub struct Scrobble<'a> {
    pub artist: &'a str,
    pub title: &'a str,
    pub album: &'a str,
    pub duration: f64,
}

fn track_params<'a>(method: &str, s: &Scrobble, sk: &str) -> BTreeMap<&'a str, String> {
    let mut p = BTreeMap::new();
    p.insert("method", method.to_string());
    p.insert("artist", s.artist.to_string());
    p.insert("track", s.title.to_string());
    if !s.album.is_empty() {
        p.insert("album", s.album.to_string());
    }
    if s.duration > 0.0 {
        p.insert("duration", (s.duration as i64).to_string());
    }
    p.insert("sk", sk.to_string());
    p
}

pub fn now_playing(cfg: &LastFm, s: &Scrobble) -> Result<()> {
    post(&cfg.api_key, &cfg.api_secret, track_params("track.updateNowPlaying", s, &cfg.session_key)).map(|_| ())
}

pub fn scrobble(cfg: &LastFm, s: &Scrobble, started: i64) -> Result<()> {
    let mut p = track_params("track.scrobble", s, &cfg.session_key);
    p.insert("timestamp", started.to_string());
    post(&cfg.api_key, &cfg.api_secret, p).map(|_| ())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn signature_excludes_format_and_sorts() {
        let mut p = BTreeMap::new();
        p.insert("method", "m".to_string());
        p.insert("api_key", "k".to_string());
        p.insert("format", "json".to_string());
        assert_eq!(sign(&p, "s"), format!("{:x}", md5::compute("api_keykmethodms")));
    }
}
