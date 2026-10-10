use serde::Serialize;

use crate::settings::Repeat;

/// The play queue. `items` is always in play order; when shuffled, `original`
/// keeps the unshuffled order so shuffle can be undone.
#[derive(Clone, Debug, Default, Serialize)]
pub struct Queue {
    pub name: String,
    pub items: Vec<i64>,
    pub pos: usize,
    pub shuffle: bool,
    pub repeat: Repeat,
    #[serde(skip)]
    original: Option<Vec<i64>>,
}

fn rng_seed() -> u64 {
    let t = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos() as u64).unwrap_or(1);
    t | 1
}

/// Fisher–Yates with xorshift; good enough for shuffling a playlist.
fn shuffle_slice(v: &mut [i64], mut s: u64) {
    for i in (1..v.len()).rev() {
        s ^= s << 13;
        s ^= s >> 7;
        s ^= s << 17;
        v.swap(i, (s % (i as u64 + 1)) as usize);
    }
}

impl Queue {
    pub fn current(&self) -> Option<i64> {
        self.items.get(self.pos).copied()
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// Replaces the queue, starting at `start` (index into `items`).
    pub fn set(&mut self, name: String, items: Vec<i64>, start: usize) {
        self.name = name;
        self.items = items;
        self.pos = start.min(self.items.len().saturating_sub(1));
        self.original = None;
        if self.shuffle {
            self.apply_shuffle();
        }
    }

    /// Restores a stored queue verbatim (no reshuffle).
    pub fn restore(&mut self, name: String, items: Vec<i64>, pos: usize) {
        self.name = name;
        self.items = items;
        self.pos = pos.min(self.items.len().saturating_sub(1));
        self.original = None;
    }

    fn apply_shuffle(&mut self) {
        self.original = Some(self.items.clone());
        if self.pos + 1 < self.items.len() {
            // Keep the current track; shuffle everything (including what came before) after it.
            let cur = self.items.remove(self.pos);
            shuffle_slice(&mut self.items, rng_seed());
            self.items.insert(0, cur);
            self.pos = 0;
        }
    }

    pub fn set_shuffle(&mut self, on: bool) {
        if on == self.shuffle {
            return;
        }
        self.shuffle = on;
        if on {
            self.apply_shuffle();
        } else if let Some(orig) = self.original.take() {
            let cur = self.current();
            self.items = orig;
            self.pos = cur.and_then(|c| self.items.iter().position(|&i| i == c)).unwrap_or(0);
        }
    }

    /// Moves to the next track automatically (respects repeat-one).
    pub fn advance(&mut self) -> Option<i64> {
        if self.repeat == Repeat::One {
            return self.current();
        }
        self.skip_next()
    }

    /// User-initiated next: repeat-one does not trap the user on the same song.
    pub fn skip_next(&mut self) -> Option<i64> {
        if self.pos + 1 < self.items.len() {
            self.pos += 1;
        } else if self.repeat != Repeat::Off && !self.items.is_empty() {
            self.pos = 0;
        } else {
            return None;
        }
        self.current()
    }

    pub fn skip_prev(&mut self) -> Option<i64> {
        if self.pos > 0 {
            self.pos -= 1;
        } else if self.repeat != Repeat::Off && !self.items.is_empty() {
            self.pos = self.items.len() - 1;
        }
        self.current()
    }

    pub fn jump(&mut self, idx: usize) -> Option<i64> {
        if idx < self.items.len() {
            self.pos = idx;
        }
        self.current()
    }

    /// Adds tracks at the end, or right after the current track when `next`.
    pub fn add(&mut self, ids: &[i64], next: bool) {
        let at = if next && !self.items.is_empty() { self.pos + 1 } else { self.items.len() };
        self.items.splice(at..at, ids.iter().copied());
        if let Some(orig) = &mut self.original {
            let cur = self.items.get(self.pos).copied();
            let oat = if next { cur.and_then(|c| orig.iter().position(|&i| i == c)).map_or(orig.len(), |p| p + 1) } else { orig.len() };
            orig.splice(oat..oat, ids.iter().copied());
        }
    }

    pub fn remove(&mut self, idx: usize) {
        if idx >= self.items.len() {
            return;
        }
        let id = self.items.remove(idx);
        if let Some(orig) = &mut self.original {
            if let Some(p) = orig.iter().position(|&i| i == id) {
                orig.remove(p);
            }
        }
        if idx < self.pos || (idx == self.pos && self.pos >= self.items.len() && self.pos > 0) {
            self.pos -= 1;
        }
    }

    pub fn move_item(&mut self, from: usize, to: usize) {
        if from >= self.items.len() || to >= self.items.len() || from == to {
            return;
        }
        let id = self.items.remove(from);
        self.items.insert(to, id);
        self.pos = if self.pos == from {
            to
        } else if from < self.pos && to >= self.pos {
            self.pos - 1
        } else if from > self.pos && to <= self.pos {
            self.pos + 1
        } else {
            self.pos
        };
    }
}

