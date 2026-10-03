use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct Settings {
    pub roots: Vec<String>,
    pub excluded: Vec<String>,
    pub volume: f32,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            roots: Vec::new(),
            excluded: Vec::new(),
            volume: 0.8,
        }
    }
}
