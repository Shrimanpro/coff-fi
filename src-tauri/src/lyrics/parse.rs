use super::{Line, Synced};

/// Parses `mm:ss.xx` / `mm:ss:xx` / `mm:ss` into seconds.
fn parse_ts(s: &str) -> Option<f64> {
    let (m, rest) = s.split_once(':')?;
    let rest = rest.replacen(':', ".", 1);
    Some(m.trim().parse::<f64>().ok()? * 60.0 + rest.trim().parse::<f64>().ok()?)
}

/// Splits leading `[..]` tags off a line.
fn leading_tags(mut line: &str) -> (Vec<&str>, &str) {
    let mut tags = Vec::new();
    while let Some(rest) = line.strip_prefix('[') {
        let Some(end) = rest.find(']') else { break };
        tags.push(&rest[..end]);
        line = &rest[end + 1..];
    }
    (tags, line)
}

/// Finalises timing: line ends from the next line, word ends that were open, background vocals.
fn finish(mut lines: Vec<Line>) -> Vec<Line> {
    lines.sort_by(|a, b| a.start.total_cmp(&b.start));
    let n = lines.len();
    for i in 0..n {
        let next = if i + 1 < n { lines[i + 1].start } else { lines[i].start + 6.0 };
        if lines[i].end.is_nan() || lines[i].end <= lines[i].start {
            lines[i].end = next.max(lines[i].start);
        }
    }
    // Drop blank lines except real instrumental breaks (rendered as a ••• pause).
    let mut out: Vec<Line> = Vec::with_capacity(n);
    for (i, l) in lines.iter().enumerate() {
        let next = lines.get(i + 1).map_or(l.start + 6.0, |x| x.start);
        if !l.text.is_empty() || (next - l.start >= 4.0 && i > 0 && i + 1 < n) {
            out.push(l.clone());
        }
    }
    out
}

/// Parses LRC and enhanced (word-timed) LRC. None if there are no timestamps.
pub fn lrc(src: &str) -> Option<(Synced, Vec<Line>)> {
    let mut offset = 0.0;
    let mut lines = Vec::new();
    for raw in src.lines() {
        let (tags, text) = leading_tags(raw.trim());
        let mut stamps = Vec::new();
        for t in tags {
            if let Some(v) = t.strip_prefix("offset:") {
                offset = v.trim().parse::<f64>().unwrap_or(0.0) / 1000.0;
            } else if let Some(ts) = parse_ts(t) {
                stamps.push(ts);
            }
        }
        for &start in &stamps {
            lines.push(Line { start, end: f64::NAN, text: text.trim().to_string() });
        }
    }
    if lines.is_empty() {
        return None;
    }
    // A positive LRC offset means the lyrics should appear earlier.
    for l in &mut lines {
        l.start = (l.start - offset).max(0.0);
    }
    let synced = Synced::Line;
    Some((synced, finish(lines)))
}

pub fn plain(src: &str) -> Vec<Line> {
    src.lines()
        .map(|l| Line { start: 0.0, end: 0.0, text: l.trim().to_string() })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn basic_lrc() {
        let (s, l) = lrc("[ti:Song]\n[00:01.00]Hello\n[00:03.50]World\n").unwrap();
        assert_eq!(s, Synced::Line);
        assert_eq!(l.len(), 2);
        assert_eq!((l[0].start, l[0].end, l[0].text.as_str()), (1.0, 3.5, "Hello"));
        assert!(l[1].end > l[1].start);
    }

    #[test]
    fn repeated_timestamps_and_offset() {
        let (_, l) = lrc("[offset:500]\n[00:10.00][00:20.00]Chorus\n[00:15.00]Verse").unwrap();
        let starts: Vec<f64> = l.iter().map(|x| x.start).collect();
        assert_eq!(starts, vec![9.5, 14.5, 19.5]);
        assert_eq!(l[2].text, "Chorus");
    }

    #[test]
    fn instrumental_gaps() {
        let (_, l) = lrc("[00:01.00]A\n[00:02.00]\n[00:20.00]B\n[00:21.00]\n[00:21.50]C").unwrap();
        let texts: Vec<&str> = l.iter().map(|x| x.text.as_str()).collect();
        assert_eq!(texts, vec!["A", "", "B", "C"]);
    }

    #[test]
    fn plain_text_is_not_lrc() {
        assert!(lrc("just words\nmore words").is_none());
    }
}
