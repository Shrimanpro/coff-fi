//! Helpers to turn messy tags into good search queries and to reject wrong matches.

/// Drops bracketed annotations and edition suffixes: `Song (From "Film") [Remastered] - 2011 Remaster` → `Song`.
pub fn clean_title(t: &str) -> String {
    let mut out = String::with_capacity(t.len());
    let mut depth = 0i32;
    for c in t.chars() {
        match c {
            '(' | '[' | '{' => depth += 1,
            ')' | ']' | '}' => depth = (depth - 1).max(0),
            _ if depth == 0 => out.push(c),
            _ => {}
        }
    }
    let lower = out.to_lowercase();
    // Byte offsets from the lowercased copy are only valid if lowercasing kept lengths.
    let cut = if lower.len() != out.len() { None } else { [" - ", " – ", " — "]
        .iter()
        .filter_map(|sep| lower.find(sep))
        .filter(|&i| {
            let tail = &lower[i..];
            ["remaster", "version", "live", "mono", "stereo", "edit", "mix", "from "].iter().any(|k| tail.contains(k))
        })
        .min() };
    let base = match cut {
        Some(i) => &out[..i],
        None => &out[..],
    };
    let cleaned = base.split_whitespace().collect::<Vec<_>>().join(" ");
    if cleaned.is_empty() { t.trim().to_string() } else { cleaned }
}

/// First credited artist: `A, B & C feat. D` → `A`.
pub fn primary_artist(a: &str) -> String {
    let lower = a.to_lowercase();
    let mut end = a.len();
    if lower.len() != a.len() {
        return a.trim().to_string();
    }
    for sep in [",", "&", ";", "/", " feat", " ft.", " ft ", " x ", " and ", " with "] {
        if let Some(i) = lower.find(sep) {
            end = end.min(i);
        }
    }
    let p = a[..end].trim();
    if p.is_empty() { a.trim().to_string() } else { p.to_string() }
}

fn norm(s: &str) -> String {
    s.chars().filter(|c| c.is_alphanumeric()).flat_map(char::to_lowercase).collect()
}

/// True when a provider's title plausibly names the same song.
pub fn same_title(ours: &str, theirs: &str) -> bool {
    let (a, b) = (norm(&clean_title(ours)), norm(&clean_title(theirs)));
    !a.is_empty() && !b.is_empty() && (a == b || a.starts_with(&b) || b.starts_with(&a))
}

/// Durations agree within `tol` seconds; unknown durations never count as a match.
pub fn same_length(ours: f64, theirs: f64, tol: f64) -> bool {
    ours > 0.0 && theirs > 0.0 && (ours - theirs).abs() <= tol
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cleans_titles() {
        assert_eq!(clean_title("Nee Pottu Vechcha (From \"Ponmaana Selvan\")"), "Nee Pottu Vechcha");
        assert_eq!(clean_title("Song [Remastered 2011]"), "Song");
        assert_eq!(clean_title("Song - 2011 Remaster"), "Song");
        assert_eq!(clean_title("Hyphen - In - Title"), "Hyphen - In - Title");
        assert_eq!(clean_title("(Intro)"), "(Intro)");
    }

    #[test]
    fn primary_artists() {
        assert_eq!(primary_artist("Mano, Malaysia Vasudevan, K.S. Chithra & Ilaiyaraaja"), "Mano");
        assert_eq!(primary_artist("Drake feat. Rihanna"), "Drake");
        assert_eq!(primary_artist("A.R. Rahman"), "A.R. Rahman");
    }

    #[test]
    fn matching() {
        assert!(same_title("Nee Pottu Vechcha (From \"Ponmaana Selvan\")", "Nee Pottu Vechcha"));
        assert!(!same_title("Blinding Lights", "NOKIA"));
        assert!(same_length(266.0, 264.5, 3.0));
        assert!(!same_length(266.0, 0.0, 3.0));
    }
}
