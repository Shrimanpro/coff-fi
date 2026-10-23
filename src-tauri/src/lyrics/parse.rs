use super::{Line, Synced, Word};

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

/// Splits `<ts>word <ts>word <ts>` into words. Returns the plain text and words.
fn parse_words(text: &str, line_start: f64) -> (String, Vec<Word>) {
    if !text.contains('<') {
        return (text.trim().to_string(), Vec::new());
    }
    let mut words: Vec<Word> = Vec::new();
    let mut plain = String::new();
    let mut rest = text;
    let mut cur_start: Option<f64> = None;
    let mut cur_text = String::new();
    let mut end_marker: Option<f64> = None;
    loop {
        let (before, after) = match rest.find('<') {
            Some(i) => (&rest[..i], Some(&rest[i + 1..])),
            None => (rest, None),
        };
        cur_text.push_str(before);
        let Some(after) = after else { break };
        let Some(close) = after.find('>') else {
            cur_text.push('<');
            rest = after;
            continue;
        };
        let Some(ts) = parse_ts(&after[..close]) else {
            cur_text.push('<');
            rest = after;
            continue;
        };
        if !cur_text.trim().is_empty() {
            words.push(Word { start: cur_start.unwrap_or(line_start), end: ts, text: std::mem::take(&mut cur_text) });
        } else if let Some(w) = words.last_mut() {
            w.text.push_str(&std::mem::take(&mut cur_text));
        } else {
            cur_text.clear();
        }
        cur_start = Some(ts);
        end_marker = Some(ts);
        rest = &after[close + 1..];
    }
    if !cur_text.trim().is_empty() {
        words.push(Word { start: cur_start.unwrap_or(line_start), end: f64::NAN, text: cur_text });
        end_marker = None;
    }
    for w in &words {
        plain.push_str(&w.text);
    }
    if let (Some(e), Some(w)) = (end_marker, words.last_mut()) {
        w.end = e;
    }
    // Normalise spacing: a word owns its trailing space, never a leading one.
    for i in 0..words.len() {
        let lead = words[i].text.len() - words[i].text.trim_start().len();
        if lead > 0 && i > 0 {
            let sp = words[i].text[..lead].to_string();
            words[i].text.drain(..lead);
            if !words[i - 1].text.ends_with(char::is_whitespace) {
                words[i - 1].text.push_str(&sp[..1]);
            }
        } else if lead > 0 {
            words[i].text.drain(..lead);
        }
    }
    (plain.split_whitespace().collect::<Vec<_>>().join(" "), words)
}

fn background(text: &str) -> Option<String> {
    let t = text.trim();
    (t.len() > 2 && t.starts_with('(') && t.ends_with(')') && !t[1..t.len() - 1].contains(['(', ')'])).then(|| t[1..t.len() - 1].trim().to_string())
}

/// Finalises timing: line ends from the next line, word ends that were open, background vocals.
fn finish(mut lines: Vec<Line>) -> Vec<Line> {
    lines.sort_by(|a, b| a.start.total_cmp(&b.start));
    let n = lines.len();
    for i in 0..n {
        let next = if i + 1 < n { lines[i + 1].start } else { lines[i].start + 6.0 };
        if lines[i].end.is_nan() || lines[i].end <= lines[i].start {
            let last_word = lines[i].words.last().map(|w| w.end).filter(|e| e.is_finite());
            lines[i].end = last_word.unwrap_or(next).max(lines[i].start);
        }
        let end = lines[i].end.max(next.min(lines[i].end + 1.0));
        if let Some(w) = lines[i].words.last_mut() {
            if !w.end.is_finite() || w.end <= w.start {
                w.end = end.max(w.start + 0.3);
            }
        }
        if let Some(t) = background(&lines[i].text) {
            lines[i].text = t;
            lines[i].background = true;
            if let Some(w) = lines[i].words.first_mut() {
                w.text = w.text.trim_start_matches('(').to_string();
            }
            if let Some(w) = lines[i].words.last_mut() {
                let t = w.text.trim_end();
                w.text = t.strip_suffix(')').unwrap_or(t).to_string();
            }
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
            let (plain, words) = parse_words(text, start);
            lines.push(Line { start, end: f64::NAN, text: plain, words, background: false });
        }
    }
    if lines.is_empty() {
        return None;
    }
    // A positive LRC offset means the lyrics should appear earlier.
    for l in &mut lines {
        l.start = (l.start - offset).max(0.0);
        for w in &mut l.words {
            w.start = (w.start - offset).max(0.0);
            w.end -= offset;
        }
    }
    let synced = if lines.iter().any(|l| !l.words.is_empty()) { Synced::Word } else { Synced::Line };
    Some((synced, finish(lines)))
}

pub fn plain(src: &str) -> Vec<Line> {
    src.lines()
        .map(|l| {
            let (bg, text) = match background(l) {
                Some(t) => (true, t),
                None => (false, l.trim().to_string()),
            };
            Line { start: 0.0, end: 0.0, text, words: Vec::new(), background: bg }
        })
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
    fn enhanced_lrc_words() {
        let (s, l) = lrc("[00:01.00]<00:01.00>I <00:01.50>wake <00:02.00>up<00:02.80>\n[00:04.00]next").unwrap();
        assert_eq!(s, Synced::Word);
        let w = &l[0].words;
        assert_eq!(w.len(), 3);
        assert_eq!(l[0].text, "I wake up");
        assert_eq!((w[0].start, w[0].end), (1.0, 1.5));
        assert_eq!((w[2].start, w[2].end), (2.0, 2.8));
        assert_eq!(w[0].text, "I ");
        assert_eq!(w[2].text, "up");
    }

    #[test]
    fn background_vocals() {
        let (_, l) = lrc("[00:01.00]Lead line\n[00:02.00](Ah-ah)\n[00:03.00]Next").unwrap();
        assert!(l[1].background);
        assert_eq!(l[1].text, "Ah-ah");
        assert!(!l[0].background);
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
        assert_eq!(plain("a\n(b)")[1].text, "b");
    }
}
