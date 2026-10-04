//! WebVTT reading, shaped by what YouTube's auto-captions actually contain.
//!
//! An auto-caption file carries every line twice: once as a "roll-up" cue a
//! few milliseconds long that repeats the previous text, and once with
//! per-word timestamps:
//!
//! ```text
//! 00:00:00.160 --> 00:00:02.389 align:start position:0%
//! how<00:00:00.480><c> to</c><00:00:00.640><c> send</c>
//! ```
//!
//! The timestamp BEFORE a `<c>` span is when that word is spoken; the first
//! word of a line starts with the cue. Lines without inline timestamps are
//! kept only when their cue is long enough to be a real caption (hand-made
//! subtitles have no word timing at all), which drops the roll-up repeats.

#[derive(Debug, Clone, PartialEq)]
pub struct Word {
    pub t: f64,
    pub text: String,
}

/// `hh:mm:ss.mmm` or `mm:ss.mmm` to seconds.
pub fn parse_ts(s: &str) -> Option<f64> {
    let s = s.trim();
    let mut parts: Vec<&str> = s.split(':').collect();
    if parts.is_empty() || parts.len() > 3 {
        return None;
    }
    let sec: f64 = parts.pop()?.parse().ok()?;
    let mut t = sec;
    let mut mult = 60.0;
    while let Some(p) = parts.pop() {
        let v: f64 = p.parse().ok()?;
        t += v * mult;
        mult *= 60.0;
    }
    Some(t)
}

/// Every timed word in the file, in order, duplicates removed.
pub fn words(src: &str) -> Vec<Word> {
    let mut out: Vec<Word> = Vec::new();
    let mut cue_start: Option<f64> = None;
    let mut cue_len = 0.0;
    let mut block: Vec<String> = Vec::new();
    let flush_block = |block: &mut Vec<String>, start: Option<f64>, len: f64, out: &mut Vec<Word>| {
        if let Some(start) = start {
            let any_timed = block.iter().any(|l| l.contains("<c>") || l.contains("><c>"));
            for line in block.iter() {
                let timed = line.contains("<c>") || line.contains("><c>");
                // Inside a cue that carries word timing, the untimed line is
                // the roll-up repeat of the previous cue. In a cue with no
                // timing at all (hand-made subtitles), keep real-length cues.
                if any_timed && !timed {
                    continue;
                }
                if !any_timed && len < 0.5 {
                    continue;
                }
                line_words(line, start, out);
            }
        }
        block.clear();
    };
    for raw in src.lines() {
        let line = raw.trim_end_matches('\r');
        if let Some((a, b)) = line.split_once("-->") {
            flush_block(&mut block, cue_start, cue_len, &mut out);
            let start = parse_ts(a);
            let end = parse_ts(b.split_whitespace().next().unwrap_or(""));
            cue_start = start;
            cue_len = match (start, end) {
                (Some(s), Some(e)) => e - s,
                _ => 0.0,
            };
            continue;
        }
        if cue_start.is_none() || line.trim().is_empty() || line.starts_with("WEBVTT") {
            continue;
        }
        if line.trim().is_empty() {
            continue;
        }
        block.push(line.to_string());
    }
    flush_block(&mut block, cue_start, cue_len, &mut out);
    out
}

fn line_words(line: &str, start: f64, out: &mut Vec<Word>) {
    let mut t = start;
    let mut buf = String::new();
    let mut chars = line.chars().peekable();
    let flush = |buf: &mut String, t: f64, out: &mut Vec<Word>| {
        for w in buf.split_whitespace() {
            let clean: String = w
                .chars()
                .filter(|c| c.is_alphanumeric() || *c == '\'')
                .collect::<String>()
                .to_lowercase();
            if clean.is_empty() {
                continue;
            }
            if out.last().map(|l: &Word| l.text == clean && (l.t - t).abs() < 0.001) == Some(true) {
                continue;
            }
            out.push(Word { t, text: clean });
        }
        buf.clear();
    };
    while let Some(c) = chars.next() {
        if c == '<' {
            let mut tag = String::new();
            for d in chars.by_ref() {
                if d == '>' {
                    break;
                }
                tag.push(d);
            }
            if let Some(ts) = parse_ts(&tag) {
                flush(&mut buf, t, out);
                t = ts;
            }
            // <c>, </c>, <i>... carry no words
            continue;
        }
        buf.push(c);
    }
    flush(&mut buf, t, out);
}

/// Positions (indices into `words`) whose text is one of `needles`.
pub fn find<'a>(words: &'a [Word], needles: &[String]) -> Vec<usize> {
    words
        .iter()
        .enumerate()
        .filter(|(_, w)| needles.iter().any(|n| &w.text == n))
        .map(|(i, _)| i)
        .collect()
}

/// `n` words either side of `i`, joined.
pub fn context(words: &[Word], i: usize, n: usize) -> String {
    let lo = i.saturating_sub(n);
    let hi = (i + n + 1).min(words.len());
    words[lo..hi]
        .iter()
        .enumerate()
        .map(|(k, w)| if lo + k == i { format!("[{}]", w.text) } else { w.text.clone() })
        .collect::<Vec<_>>()
        .join(" ")
}

/// Hit times grouped into windows: consecutive hits closer than `gap` share one.
pub fn windows(times: &[f64], gap: f64) -> Vec<(f64, f64)> {
    let mut ts: Vec<f64> = times.to_vec();
    ts.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let mut out: Vec<(f64, f64)> = Vec::new();
    for t in ts {
        match out.last_mut() {
            Some((_, end)) if t - *end <= gap => *end = t,
            _ => out.push((t, t)),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_word_timing_and_drops_rollups() {
        let src = "WEBVTT\nKind: captions\nLanguage: en\n\n00:00:00.160 --> 00:00:02.389 align:start position:0%\n \nhow<00:00:00.480><c> to</c><00:00:00.640><c> send</c>\n\n00:00:02.389 --> 00:00:02.399 align:start position:0%\nhow to send\n \n\n00:00:02.399 --> 00:00:04.000 align:start position:0%\nhow to send\na<00:00:02.800><c> message</c>\n";
        let w = words(src);
        let texts: Vec<&str> = w.iter().map(|x| x.text.as_str()).collect();
        assert_eq!(texts, vec!["how", "to", "send", "a", "message"]);
        assert!((w[2].t - 0.64).abs() < 1e-9);
        assert!((w[3].t - 2.399).abs() < 1e-9);
        assert!((w[4].t - 2.8).abs() < 1e-9);
    }

    #[test]
    fn timestamps() {
        assert_eq!(parse_ts("00:01:02.500"), Some(62.5));
        assert_eq!(parse_ts("01:02.5"), Some(62.5));
        assert_eq!(parse_ts("7"), Some(7.0));
        assert_eq!(parse_ts("x"), None);
    }

    #[test]
    fn groups_windows() {
        assert_eq!(windows(&[1.0, 2.0, 10.0], 4.0), vec![(1.0, 2.0), (10.0, 10.0)]);
    }
}
