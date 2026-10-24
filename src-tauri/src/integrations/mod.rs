//! OS media controls (MPRIS / SMTC / Now Playing), driven from a background thread.

use std::path::PathBuf;
use std::time::Duration;

use crossbeam_channel::{unbounded, Sender};
use souvlaki::{MediaControlEvent, MediaControls, MediaMetadata, MediaPlayback, MediaPosition, PlatformConfig, SeekDirection};

use crate::settings::Settings;

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
    std::thread::Builder::new()
        .name("integrations".into())
        .spawn(move || {
            let on_control = std::sync::Arc::new(on_control);
            let mut cfg = settings;
            let mut controls: Option<MediaControls> = None;
            let mut track: Option<NowPlaying> = None;
            let mut playing = false;
            let mut position = 0.0;

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
                        cfg = *s;
                    }
                }
            }
        })
        .expect("spawn integrations thread");
    Integrations(tx)
}
