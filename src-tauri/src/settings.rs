use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
pub enum Repeat {
    #[default]
    Off,
    All,
    One,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct Settings {
    pub roots: Vec<String>,
    pub excluded: Vec<String>,
    pub volume: f32,
    pub shuffle: bool,
    pub repeat: Repeat,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            roots: Vec::new(),
            excluded: Vec::new(),
            volume: 0.8,
            shuffle: false,
            repeat: Repeat::Off,
        }
    }
}
