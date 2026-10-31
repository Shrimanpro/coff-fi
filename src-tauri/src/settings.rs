use serde::{Deserialize, Serialize};

pub const EQ_FREQS: [f32; 10] = [31.0, 62.0, 125.0, 250.0, 500.0, 1000.0, 2000.0, 4000.0, 8000.0, 16000.0];

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct Eq {
    pub enabled: bool,
    pub preamp: f32,
    pub bands: [f32; 10],
    pub preset: String,
}

impl Default for Eq {
    fn default() -> Self {
        Self { enabled: false, preamp: 0.0, bands: [0.0; 10], preset: "Flat".into() }
    }
}

/// A user-saved equalizer curve. Kept outside `Eq` so renaming or saving one
/// never rebuilds the live filters.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Default)]
#[serde(default)]
pub struct EqPreset {
    pub name: String,
    pub preamp: f32,
    pub bands: [f32; 10],
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
pub enum ReplayGain {
    #[default]
    Off,
    Track,
    Album,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
pub enum Repeat {
    #[default]
    Off,
    All,
    One,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Default)]
#[serde(default)]
pub struct Discord {
    pub enabled: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Default)]
#[serde(default)]
pub struct LastFm {
    pub enabled: bool,
    pub api_key: String,
    pub api_secret: String,
    pub username: String,
    pub session_key: String,
}

/// A Navidrome (Subsonic API) server. `active` = the app is browsing the server instead of local files.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Default)]
#[serde(default)]
pub struct Navidrome {
    pub active: bool,
    pub url: String,
    pub username: String,
    pub password: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct Settings {
    pub roots: Vec<String>,
    pub excluded: Vec<String>,
    pub watch: bool,
    pub eq: Eq,
    pub eq_presets: Vec<EqPreset>,
    pub volume: f32,
    pub output_device: Option<String>,
    pub gapless: bool,
    pub crossfade: f32,
    pub replaygain: ReplayGain,
    pub rg_preamp: f32,
    pub speed: f32,
    pub shuffle: bool,
    pub repeat: Repeat,
    pub media_controls: bool,
    pub discord: Discord,
    pub lastfm: LastFm,
    pub navidrome: Navidrome,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            roots: Vec::new(),
            excluded: Vec::new(),
            watch: true,
            eq: Eq::default(),
            eq_presets: Vec::new(),
            volume: 0.8,
            output_device: None,
            gapless: true,
            crossfade: 0.0,
            replaygain: ReplayGain::Off,
            rg_preamp: 0.0,
            speed: 1.0,
            shuffle: false,
            repeat: Repeat::Off,
            media_controls: true,
            discord: Discord::default(),
            lastfm: LastFm::default(),
            navidrome: Navidrome::default(),
        }
    }
}
