use serde::Serialize;

/// The play queue: track ids in play order and the current position.
#[derive(Clone, Debug, Default, Serialize)]
pub struct Queue {
    pub name: String,
    pub items: Vec<i64>,
    pub pos: usize,
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
    }

    /// Moves to the next track automatically.
    pub fn advance(&mut self) -> Option<i64> {
        self.skip_next()
    }

    /// User-initiated next.
    pub fn skip_next(&mut self) -> Option<i64> {
        if self.pos + 1 < self.items.len() {
            self.pos += 1;
        } else {
            return None;
        }
        self.current()
    }

    pub fn skip_prev(&mut self) -> Option<i64> {
        if self.pos > 0 {
            self.pos -= 1;
        }
        self.current()
    }
}

