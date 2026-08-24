//! The record format: one line, tab-separated, `<iso8601>\t<kind>\t<k=v>...`.
//!
//! Every durable file this harness writes is a stream of these. The format is
//! deliberately boring: append-only, line-oriented, greppable by a human, and
//! parseable with no dependency. Logs are *sharded by writer* (one file per
//! box per process start), so two boxes appending at the same time never
//! produce a git conflict — the merge is a directory union.

use std::fmt::Write as _;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Rec {
    pub ts: i64,
    pub kind: String,
    pub fields: Vec<(String, String)>,
}

fn esc(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '\t' => out.push_str("\\t"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            c => out.push(c),
        }
    }
    out
}

fn unesc(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut it = s.chars();
    while let Some(c) = it.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }
        match it.next() {
            Some('\\') => out.push('\\'),
            Some('t') => out.push('\t'),
            Some('n') => out.push('\n'),
            Some('r') => out.push('\r'),
            Some(other) => {
                out.push('\\');
                out.push(other);
            }
            None => out.push('\\'),
        }
    }
    out
}

impl Rec {
    pub fn new(kind: &str) -> Rec {
        Rec { ts: crate::time::now(), kind: kind.to_string(), fields: Vec::new() }
    }

    pub fn at(ts: i64, kind: &str) -> Rec {
        Rec { ts, kind: kind.to_string(), fields: Vec::new() }
    }

    pub fn f(mut self, k: &str, v: impl ToString) -> Rec {
        self.fields.push((k.to_string(), v.to_string()));
        self
    }

    pub fn set(&mut self, k: &str, v: impl ToString) {
        let v = v.to_string();
        for slot in self.fields.iter_mut() {
            if slot.0 == k {
                slot.1 = v;
                return;
            }
        }
        self.fields.push((k.to_string(), v));
    }

    pub fn get(&self, k: &str) -> Option<&str> {
        self.fields.iter().find(|(kk, _)| kk == k).map(|(_, v)| v.as_str())
    }

    pub fn get_i64(&self, k: &str) -> Option<i64> {
        self.get(k).and_then(|v| v.parse().ok())
    }

    pub fn get_u64(&self, k: &str) -> Option<u64> {
        self.get(k).and_then(|v| v.parse().ok())
    }

    pub fn get_f64(&self, k: &str) -> Option<f64> {
        self.get(k).and_then(|v| v.parse().ok())
    }

    pub fn render(&self) -> String {
        let mut s = String::new();
        let _ = write!(s, "{}\t{}", crate::time::iso(self.ts), esc(&self.kind));
        for (k, v) in &self.fields {
            let _ = write!(s, "\t{}={}", esc(k), esc(v));
        }
        s
    }

    /// Parse one line. Returns `None` for a blank line or a comment; returns
    /// an `Err` for a line that is present but malformed — a corrupt line is
    /// never silently skipped, because "the log looked empty" is exactly the
    /// failure shape this project keeps paying for.
    pub fn parse(line: &str) -> Result<Option<Rec>, String> {
        let line = line.trim_end_matches(['\n', '\r']);
        if line.trim().is_empty() || line.starts_with('#') {
            return Ok(None);
        }
        let mut parts = line.split('\t');
        let ts_s = parts.next().ok_or("no timestamp field")?;
        let ts = crate::time::parse_iso(ts_s)
            .ok_or_else(|| format!("bad timestamp {ts_s:?}"))?;
        let kind = unesc(parts.next().ok_or("no kind field")?);
        let mut fields = Vec::new();
        for p in parts {
            match p.split_once('=') {
                Some((k, v)) => fields.push((unesc(k), unesc(v))),
                None => return Err(format!("field {p:?} has no '='")),
            }
        }
        Ok(Some(Rec { ts, kind, fields }))
    }

    pub fn parse_all(text: &str) -> Result<Vec<Rec>, String> {
        let mut out = Vec::new();
        for (i, line) in text.lines().enumerate() {
            match Rec::parse(line) {
                Ok(Some(r)) => out.push(r),
                Ok(None) => {}
                Err(e) => return Err(format!("line {}: {e}", i + 1)),
            }
        }
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_a_plain_record() {
        let r = Rec::at(1_787_596_000, "sample").f("evals", 42).f("node", "od1");
        let back = Rec::parse(&r.render()).unwrap().unwrap();
        assert_eq!(r, back);
    }

    #[test]
    fn round_trips_values_containing_tabs_and_newlines() {
        // The `why` field of a ledger entry is prose written by a human or an
        // agent; it will contain newlines sooner or later, and a naive writer
        // would split one record into two and corrupt every later reader.
        let nasty = "line one\nline\ttwo\\three\r\n";
        let r = Rec::at(1, "ledger").f("why", nasty);
        let rendered = r.render();
        assert_eq!(rendered.lines().count(), 1, "a record must stay one line");
        let back = Rec::parse(&rendered).unwrap().unwrap();
        assert_eq!(back.get("why").unwrap(), nasty);
    }

    #[test]
    fn a_corrupt_line_is_an_error_not_a_silent_skip() {
        assert!(Rec::parse("not-a-timestamp\tkind").is_err());
        assert!(Rec::parse("2026-08-24T18:00:00Z\tkind\tnoequals").is_err());
        // ... while blanks and comments are legitimately nothing.
        assert_eq!(Rec::parse("").unwrap(), None);
        assert_eq!(Rec::parse("# a note").unwrap(), None);
    }

    #[test]
    fn parse_all_reports_the_offending_line_number() {
        let text = "2026-08-24T18:00:00Z\ta\n2026-08-24T18:00:01Z\tb\noops\n";
        let e = Rec::parse_all(text).unwrap_err();
        assert!(e.starts_with("line 3:"), "{e}");
    }
}
