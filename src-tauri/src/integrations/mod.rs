//! OS media controls (MPRIS / SMTC / Now Playing), driven from a background thread.

use std::collections::HashMap;
use std::path::PathBuf;
use std::time::Duration;
use std::time::{SystemTime, UNIX_EPOCH};

use crossbeam_channel::{unbounded, Sender};
use discord_rich_presence::activity::{self, ActivityType, StatusDisplayType};
use discord_rich_presence::{DiscordIpc, DiscordIpcClient};
use serde_json::Value;
use souvlaki::{MediaControlEvent, MediaControls, MediaMetadata, MediaPlayback, MediaPosition, PlatformConfig, SeekDirection};

use crate::settings::Settings;

/// Coff-fi's own Discord application, so users only need to flip the toggle.
const DISCORD_APP_ID: &str = "1567060675333722172";

#[derive(Clone, Debug)]
pub struct NowPlaying {
    pub title: String,
    pub artist: String,
    pub album: String,
    pub duration: f64,
    pub cover: Option<PathBuf>,
}

pub enum Msg {
    Track(Option<NowPlaying>),
    Playback { playing: bool, position: f64 },
    Config(Box<Settings>),
    /// Result of a background cover-art lookup for Discord (keyed by `art_key`).
    Art(String, Option<String>),
}

/// Discord can only show artwork from a public URL, so local covers are matched
/// against the iTunes catalogue (keyless) and cached per album for the session.
fn art_key(t: &NowPlaying) -> String {
    let what = if t.album.is_empty() { &t.title } else { &t.album };
    format!("{}\u{1f}{}", t.artist, what).to_lowercase()
}

fn lookup_art(t: &NowPlaying) -> Option<String> {
    let (term, entity) = if t.album.is_empty() { (&t.title, "song") } else { (&t.album, "album") };
    let url = format!(
        "https://itunes.apple.com/search?term={}&entity={entity}&limit=1",
        urlencoding::encode(&format!("{} {term}", t.artist))
    );
    let v: Value = crate::lyrics::http().get(url).send().ok()?.json().ok()?;
    let art = v.pointer("/results/0/artworkUrl100")?.as_str()?;
    Some(art.replace("100x100bb", "512x512bb"))
}

fn now_ms() -> i64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis() as i64).unwrap_or(0)
}

/// Remote-control requests coming from the OS.
#[derive(Clone, Copy, Debug)]
pub enum Control {
    Play,
    Pause,
    Toggle,
    Next,
    Prev,
    Stop,
    SeekTo(f64),
    SeekBy(f64),
}

#[derive(Clone)]
pub struct Integrations(Sender<Msg>);

impl Integrations {
    pub fn send(&self, m: Msg) {
        let _ = self.0.send(m);
    }
}

pub fn spawn(settings: Settings, hwnd: Option<isize>, on_control: impl Fn(Control) + Send + Sync + 'static) -> Integrations {
    let (tx, rx) = unbounded::<Msg>();
    let art_tx = tx.clone();
    std::thread::Builder::new()
        .name("integrations".into())
        .spawn(move || {
            let on_control = std::sync::Arc::new(on_control);
            let mut cfg = settings;
            let mut controls: Option<MediaControls> = None;
            let mut discord: Option<DiscordIpcClient> = None;
            let mut track: Option<NowPlaying> = None;
            let mut playing = false;
            let mut position = 0.0;
            let mut art: HashMap<String, Option<String>> = HashMap::new();

            let make_controls = |cfg: &Settings| -> Option<MediaControls> {
                if !cfg.media_controls {
                    return None;
                }
                let platform = PlatformConfig {
                    dbus_name: "coffi",
                    display_name: "Coff-fi",
                    hwnd: hwnd.map(|h| h as *mut std::ffi::c_void),
                };
                let mut c = MediaControls::new(platform).ok()?;
                let cb = on_control.clone();
                c.attach(move |e| {
                    let cmd = match e {
                        MediaControlEvent::Play => Control::Play,
                        MediaControlEvent::Pause => Control::Pause,
                        MediaControlEvent::Toggle => Control::Toggle,
                        MediaControlEvent::Next => Control::Next,
                        MediaControlEvent::Previous => Control::Prev,
                        MediaControlEvent::Stop => Control::Stop,
                        MediaControlEvent::SetPosition(MediaPosition(d)) => Control::SeekTo(d.as_secs_f64()),
                        MediaControlEvent::Seek(dir) => Control::SeekBy(if dir == SeekDirection::Forward { 10.0 } else { -10.0 }),
                        MediaControlEvent::SeekBy(dir, d) => {
                            Control::SeekBy(if dir == SeekDirection::Forward { d.as_secs_f64() } else { -d.as_secs_f64() })
                        }
                        _ => return,
                    };
                    cb(cmd);
                })
                .ok()?;
                Some(c)
            };
            controls = make_controls(&cfg).or(controls);

            let connect_discord = |cfg: &Settings| -> Option<DiscordIpcClient> {
                if !cfg.discord.enabled {
                    return None;
                }
                let mut c = DiscordIpcClient::new(DISCORD_APP_ID);
                c.connect().ok()?;
                Some(c)
            };
            discord = connect_discord(&cfg).or(discord);


            while let Ok(msg) = rx.recv() {
                match msg {
                    Msg::Track(np) => {
                        track = np;
                        if let Some(c) = &mut controls {
                            match &track {
                                Some(t) => {
                                    let cover = t.cover.as_ref().map(|p| format!("file://{}", p.display()));
                                    let _ = c.set_metadata(MediaMetadata {
                                        title: Some(&t.title),
                                        artist: Some(&t.artist),
                                        album: Some(&t.album),
                                        cover_url: cover.as_deref(),
                                        duration: Some(Duration::from_secs_f64(t.duration.max(0.0))),
                                    });
                                }
                                None => {
                                    let _ = c.set_playback(MediaPlayback::Stopped);
                                }
                            }
                        }
                    }
                    Msg::Playback { playing: p, position: pos } => {
                        let changed = p != playing || (pos - position).abs() > 2.0;
                        playing = p;
                        position = pos;
                        if !changed {
                            continue;
                        }
                        if let Some(c) = &mut controls {
                            let progress = Some(MediaPosition(Duration::from_secs_f64(pos.max(0.0))));
                            let _ = c.set_playback(if track.is_none() {
                                MediaPlayback::Stopped
                            } else if p {
                                MediaPlayback::Playing { progress }
                            } else {
                                MediaPlayback::Paused { progress }
                            });
                        }
                    }
                    Msg::Config(s) => {
                        if s.media_controls != cfg.media_controls {
                            controls = if s.media_controls { make_controls(&s) } else { None };
                        }
                        if s.discord != cfg.discord {
                            if let Some(mut d) = discord.take() {
                                let _ = d.close();
                            }
                            discord = connect_discord(&s);
                        }
                        cfg = *s;
                    }
                    Msg::Art(key, url) => {
                        let current = track.as_ref().is_some_and(|t| art_key(t) == key);
                        art.insert(key, url);
                        if !current {
                            continue;
                        }
                    }
                }
                if let Some(d) = &mut discord {
                    let ok = match (&track, playing) {
                        (Some(t), true) => {
                            let start = now_ms() - (position * 1000.0) as i64;
                            let mut times = activity::Timestamps::new().start(start);
                            if t.duration > 0.0 {
                                times = times.end(start + (t.duration * 1000.0) as i64);
                            }
                            let artist = if t.artist.is_empty() { "Unknown artist" } else { &t.artist };
                            let mut assets = activity::Assets::new()
                                .large_text(if t.album.is_empty() { "Coff-fi" } else { &t.album })
                                .small_text("Coff-fi");
                            let key = art_key(t);
                            match art.get(&key) {
                                Some(Some(url)) => assets = assets.large_image(url.clone()),
                                Some(None) => {}
                                None => {
                                    // Mark as pending so each album is only looked up once.
                                    art.insert(key.clone(), None);
                                    let (t, tx) = (t.clone(), art_tx.clone());
                                    std::thread::spawn(move || {
                                        let _ = tx.send(Msg::Art(key, lookup_art(&t)));
                                    });
                                }
                            }
                            d.set_activity(
                                activity::Activity::new()
                                    .activity_type(ActivityType::Listening)
                                    .name("Coff-fi")
                                    .status_display_type(StatusDisplayType::Name)
                                    .details(&t.title)
                                    .state(artist)
                                    .timestamps(times)
                                    .assets(assets),
                            )
                            .is_ok()
                        }
                        _ => d.clear_activity().is_ok(),
                    };
                    if !ok {
                        discord = connect_discord(&cfg);
                    }
                }
            }
        })
        .expect("spawn integrations thread");
    Integrations(tx)
}
