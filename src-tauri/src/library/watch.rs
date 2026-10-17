use std::path::Path;
use std::time::Duration;

use notify::{RecommendedWatcher, RecursiveMode};
use notify_debouncer_mini::{new_debouncer, DebounceEventResult, Debouncer};

use super::scan::is_audio;

pub type Watcher = Debouncer<RecommendedWatcher>;

/// Watches the library roots and calls `on_change` (debounced) when audio or image files change.
pub fn watch(roots: &[String], on_change: impl Fn() + Send + 'static) -> notify::Result<Watcher> {
    let mut d = new_debouncer(Duration::from_secs(2), move |res: DebounceEventResult| {
        let Ok(events) = res else { return };
        let relevant = events.iter().any(|e| {
            let p = &e.path;
            is_audio(p) || p.extension().is_none() || p.extension().is_some_and(|x| matches!(x.to_str(), Some("jpg" | "jpeg" | "png" | "webp")))
        });
        if relevant {
            on_change();
        }
    })?;
    for r in roots {
        if Path::new(r).is_dir() {
            d.watcher().watch(Path::new(r), RecursiveMode::Recursive)?;
        }
    }
    Ok(d)
}
