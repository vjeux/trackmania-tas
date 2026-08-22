//! Rewrite windows of a `gtape`.
//!
//! The previous arm's editor (`pkzcut`) decoded all 566 327 packets through a
//! `Factory` that has no word for a respawn, re-encoded every vehicle field,
//! and produced a file 1.9 MB larger than the one it read. This one is a text
//! transform over `ghost tape extract`'s dump, which round-trips byte for byte
//! and models the respawn bit as the input it is — so an edited tape differs
//! from its parent in exactly the ticks named and nowhere else, and `--edit
//! ...:respawn:1` is expressible.
//!
//! Windows are in RACE milliseconds, half-open `[from, to)`, and are applied in
//! order. Every window reports how many ticks it actually changed, and the
//! command exits non-zero if any window changed nothing: on a tape with a
//! frozen prefix of edits a whole-file no-op check cannot see a window that
//! did nothing, and that defect cost the previous arm 58 of 326 candidates.

#[derive(Clone)]
pub struct Edit {
    /// A window whose only job is to HOLD a channel at a value it may already
    /// have (the throttle through a switchback). It is not what makes the
    /// candidate the candidate, so a no-op there is not a defect.
    pub optional: bool,
    pub from_ms: i64,
    pub to_ms: i64,
    pub chan: String,
    pub val: i64,
}

pub fn parse_edit(s: &str) -> Result<Edit, String> {
    let p: Vec<&str> = s.split(':').collect();
    if p.len() != 4 {
        return Err(format!("--edit wants FROM_MS:TO_MS:CHAN:VALUE, got {}", s));
    }
    let chan = p[2].to_string();
    if !["steer", "accel", "brake", "respawn"].contains(&chan.as_str()) {
        return Err(format!("unknown channel {}", chan));
    }
    Ok(Edit {
        optional: false,
        from_ms: p[0].parse().map_err(|_| "bad from")?,
        to_ms: p[1].parse().map_err(|_| "bad to")?,
        chan,
        val: p[3].parse().map_err(|_| "bad value")?,
    })
}

fn set_field(line: &str, key: &str, val: i64) -> (String, bool) {
    let pat = format!("{}=", key);
    let Some(i) = line.find(&pat) else { return (line.to_string(), false) };
    let vs = i + pat.len();
    let ve = line[vs..].find(' ').map(|k| vs + k).unwrap_or(line.len());
    if line[vs..ve] == val.to_string() {
        return (line.to_string(), false);
    }
    let mut out = String::with_capacity(line.len() + 4);
    out.push_str(&line[..vs]);
    out.push_str(&val.to_string());
    out.push_str(&line[ve..]);
    (out, true)
}

/// `vsame=1` means the packet inherited the previous tick's vehicle fields.
/// Writing a value into such a line only means something once the line is
/// marked as carrying its own, so every touched line is expanded.
fn expand(line: &str) -> String {
    set_field(line, "vsame", 0).0
}

pub fn run(inp: &str, out: &str, edits: &[Edit]) -> i32 {
    use std::io::{BufRead, Write};
    let f = std::fs::File::open(inp).unwrap_or_else(|e| { eprintln!("{}: {}", inp, e); std::process::exit(1) });
    let mut w = std::io::BufWriter::new(
        std::fs::File::create(out).unwrap_or_else(|e| { eprintln!("{}: {}", out, e); std::process::exit(1) })
    );
    let mut start_offset_ms: i64 = 0;
    let mut changed = vec![0usize; edits.len()];
    let mut touched = vec![0usize; edits.len()];
    for line in std::io::BufReader::new(f).lines() {
        let line = line.unwrap();
        if line.starts_with("@archive") {
            for kv in line.split_whitespace() {
                if let Some(v) = kv.strip_prefix("start_offset_ms=") {
                    start_offset_ms = v.parse().unwrap_or(0);
                }
            }
            writeln!(w, "{}", line).unwrap();
            continue;
        }
        if !line.starts_with("t=") {
            writeln!(w, "{}", line).unwrap();
            continue;
        }
        let tick: i64 = line[2..].split_whitespace().next().unwrap().parse().unwrap();
        let race_ms = tick * 10 + start_offset_ms;
        let mut cur = line;
        for (i, e) in edits.iter().enumerate() {
            if race_ms < e.from_ms || race_ms >= e.to_ms {
                continue;
            }
            touched[i] += 1;
            let key = if e.chan == "respawn" { "respawn" } else { e.chan.as_str() };
            if e.chan != "respawn" {
                cur = expand(&cur);
            }
            let (n, did) = set_field(&cur, key, e.val);
            cur = n;
            if did {
                changed[i] += 1;
            }
        }
        writeln!(w, "{}", cur).unwrap();
    }
    let mut dead = 0;
    for (i, e) in edits.iter().enumerate() {
        println!(
            "edit {}:{}:{}:{}  ticks in window {}  CHANGED {}{}",
            e.from_ms, e.to_ms, e.chan, e.val, touched[i], changed[i],
            if changed[i] == 0 { "   <-- NO-OP" } else { "" }
        );
        if changed[i] == 0 && !e.optional {
            dead += 1;
        }
    }
    if dead > 0 {
        eprintln!("{} of {} windows changed nothing -- this candidate is not the candidate it is named", dead, edits.len());
        return 5;
    }
    0
}
