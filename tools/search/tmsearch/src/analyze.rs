//! What one search step actually buys.
//!
//! `tmsearch dump` evaluates candidates from a FIXED incumbent and writes one
//! record per candidate: the operator that made it and the outcome it earned.
//! Nothing is accepted and the incumbent never moves, so the records are an
//! unbiased sample of the neighbourhood. A live search cannot answer this: it
//! logs only what it accepted.
//!
//! Two questions this is for, both of which have paid:
//!
//! * **Which operators and which ticks are productive?** Retuning a stalled
//!   search from its own log -- tally `kind@tick`, restrict the arms to the
//!   productive range -- broke a plateau that four parameter configurations had
//!   not. And a dead zone is not where to search, but its EDGE is: the
//!   breakthrough operator there fired one tick before the dead window began.
//! * **What is the marginal value of the 200th candidate in a batch?** The
//!   best-of-`k` curve below answers it directly, and it is how `--batch` gets
//!   chosen instead of guessed.

use crate::report::{delta, secs};
use std::collections::BTreeMap;

/// One evaluated candidate, as `tmsearch dump` writes it.
#[derive(Clone, Debug, PartialEq)]
pub struct Record {
    pub kind: String,
    pub at: i64,
    pub span: i64,
    pub val: i64,
    /// Finish time in ms, or `None` for a run that did not finish.
    pub ms: Option<i64>,
    pub cps: u32,
}

/// Read the JSONL `tmsearch dump` writes. Deliberately a small hand parser:
/// this project has no third-party runtime, and the format is ours.
pub fn parse_dump(text: &str) -> Vec<Record> {
    text.lines()
        .filter(|l| l.contains("\"kind\""))
        .map(|l| Record {
            kind: jstr(l, "kind"),
            at: jnum(l, "at").unwrap_or(0),
            span: jnum(l, "span").unwrap_or(0),
            val: jnum(l, "val").unwrap_or(0),
            ms: jnum(l, "ms"),
            cps: jnum(l, "cps").unwrap_or(0) as u32,
        })
        .collect()
}

fn jstr(l: &str, k: &str) -> String {
    let pat = format!("\"{}\":\"", k);
    match l.find(&pat) {
        Some(i) => {
            let rest = &l[i + pat.len()..];
            rest[..rest.find('"').unwrap_or(0)].to_string()
        }
        None => String::new(),
    }
}

fn jnum(l: &str, k: &str) -> Option<i64> {
    let pat = format!("\"{}\":", k);
    let i = l.find(&pat)?;
    let rest = &l[i + pat.len()..];
    let end = rest.find(|c: char| !(c.is_ascii_digit() || c == '-')).unwrap_or(rest.len());
    rest[..end].parse().ok()
}

struct Group {
    n: usize,
    finished: usize,
    best: Option<i64>,
    improved: usize,
}

impl Group {
    fn new() -> Group {
        Group { n: 0, finished: 0, best: None, improved: 0 }
    }
    fn add(&mut self, r: &Record, base_ms: i64) {
        self.n += 1;
        if let Some(t) = r.ms {
            self.finished += 1;
            if t < base_ms {
                self.improved += 1;
            }
            if self.best.map(|b| t < b).unwrap_or(true) {
                self.best = Some(t);
            }
        }
    }
    fn row(&self, name: &str, base_ms: i64) -> String {
        format!(
            "  {:<10} {:>7}  finish {:>5.1}%  improved {:>5.2}%  best {}",
            name,
            self.n,
            100.0 * self.finished as f64 / self.n.max(1) as f64,
            100.0 * self.improved as f64 / self.n.max(1) as f64,
            match self.best {
                Some(b) => format!("{} ({})", secs(b), delta(b - base_ms)),
                None => "-".into(),
            }
        )
    }
}

/// The whole report, as text.
pub fn report(rows: &[Record], base_ms: i64, bucket: i64) -> String {
    let mut s = String::new();
    s.push_str(&format!(
        "{} candidates from an incumbent of {}\n\n",
        rows.len(),
        secs(base_ms)
    ));
    if rows.is_empty() {
        return s;
    }

    let mut by_kind: BTreeMap<String, Group> = BTreeMap::new();
    let mut by_tick: BTreeMap<i64, Group> = BTreeMap::new();
    let mut all = Group::new();
    for r in rows {
        all.add(r, base_ms);
        by_kind.entry(r.kind.clone()).or_insert_with(Group::new).add(r, base_ms);
        by_tick.entry(r.at / bucket * bucket).or_insert_with(Group::new).add(r, base_ms);
    }
    s.push_str("by operator\n");
    for (k, g) in &by_kind {
        s.push_str(&g.row(k, base_ms));
        s.push('\n');
    }
    s.push_str(&format!("\nby tick, in buckets of {}\n", bucket));
    for (t, g) in &by_tick {
        if g.n == 0 {
            continue;
        }
        s.push_str(&g.row(&format!("{}..", t), base_ms));
        s.push('\n');
    }

    // The marginal value of a bigger batch: best-of-k over k candidates drawn
    // in the order they were evaluated.
    s.push_str("\nbest-of-k (the marginal value of a larger --batch)\n");
    let mut best = i64::MAX;
    let mut k = 1usize;
    let mut next = 1usize;
    for (i, r) in rows.iter().enumerate() {
        if let Some(t) = r.ms {
            best = best.min(t);
        }
        if i + 1 == next {
            s.push_str(&format!(
                "  k={:<6} {}\n",
                next,
                if best == i64::MAX { "-".to_string() } else { format!("{} ({})", secs(best), delta(best - base_ms)) }
            ));
            next *= 2;
            k += 1;
        }
    }
    let _ = k;
    s.push_str(&format!("\noverall\n{}\n", all.row("all", base_ms)));
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = concat!(
        "{\"kind\":\"cos\",\"at\":100,\"span\":8,\"val\":40,\"ms\":22963,\"cps\":0}\n",
        "{\"kind\":\"cos\",\"at\":140,\"span\":8,\"val\":-9,\"ms\":null,\"cps\":2}\n",
        "{\"kind\":\"edge\",\"at\":900,\"span\":2,\"val\":127,\"ms\":23100,\"cps\":0}\n"
    );

    #[test]
    fn parses_its_own_format() {
        let r = parse_dump(SAMPLE);
        assert_eq!(r.len(), 3);
        assert_eq!(r[0].kind, "cos");
        assert_eq!(r[0].ms, Some(22963));
        assert_eq!(r[1].ms, None);
        assert_eq!(r[1].cps, 2);
        assert_eq!(r[2].at, 900);
    }

    #[test]
    fn the_report_is_in_seconds() {
        let r = parse_dump(SAMPLE);
        let out = report(&r, 23000, 100);
        assert!(out.contains("22.963"), "{}", out);
        assert!(out.contains("-0.037"), "{}", out);
        assert!(!out.contains("22963"), "{}", out);
    }
}
