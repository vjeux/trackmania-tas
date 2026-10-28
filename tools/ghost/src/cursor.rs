//! `ghost tape cursor` -- the OS cursor samples inside a run's input tape, read
//! as a forensic instrument.
//!
//! Every input packet carries one bit "cursor unchanged"; when the cursor moved
//! since the previous tick the packet carries its new ABSOLUTE position, two
//! 16-bit axes in Windows' normalised virtual-desktop space (0..65535 across the
//! primary monitor; a monitor to the left shows up as a wrap through 65535).
//! The game writes it for every player, mouse-steerer or not -- most runs have
//! one sample at tick 0 (where the cursor sat when the countdown ended) and
//! nothing after.
//!
//! A hand on a mouse produces STREAMS: one sample per tick while it moves, a
//! few hundred units per tick at the fastest, and at rest the odd nudge of a
//! few units. Measured on the top 300 of Summer 2021 - 17 (2026-10-27): 965
//! mid-run samples from 5 players; 139 of them isolated (no sample in the
//! previous 50 ms), the largest isolated move 76 units, none on a screen edge.
//! A program placing the cursor (SetCursorPos and everything wrapping it)
//! produces the opposite: one sample, no run-up, no run-down, a large jump,
//! often to a round place such as the corner (0,0). That is the fingerprint
//! this command measures: `singletons`, `max_isolated_jump`, `edge`, `corner`.
//!
//! A jump is measured from the previous KNOWN position, so the first mid-run
//! sample of a run with no tick-0 sample has no jump (the cursor's resting
//! place is unknown). Wrap-through-65535 moves are reported as what they are
//! (a second monitor), not as 65,000-unit teleports.

use std::path::{Path, PathBuf};

use gbx::tape::Tape;

use crate::cli::{flag, has};

/// One mid-run cursor sample with its context.
#[derive(Debug, Clone)]
pub struct CursorEvent {
    pub tick: usize,
    pub race_ms: i64,
    pub x: u16,
    pub y: u16,
    /// Ticks since the previous cursor sample (tick 0 counts); `None` when
    /// this is the first sample of the tape.
    pub gap_before: Option<usize>,
    /// Ticks until the next cursor sample; `None` when this is the last.
    pub gap_after: Option<usize>,
    /// |dx|+|dy| from the previous known position, with each axis taken the
    /// short way round the 16-bit wrap; `None` when nothing came before.
    pub jump: Option<u32>,
}

impl CursorEvent {
    pub fn on_edge(&self) -> bool {
        self.x == 0 || self.y == 0
    }
    pub fn corner(&self) -> bool {
        self.x == 0 && self.y == 0
    }
    /// No cursor sample within 5 ticks (50 ms) on either side.
    pub fn singleton(&self) -> bool {
        self.gap_before.map_or(true, |g| g > 5) && self.gap_after.map_or(true, |g| g > 5)
    }
}

/// The summary one file gets.
#[derive(Debug, Clone)]
pub struct CursorRow {
    pub file: String,
    pub ticks: usize,
    pub tick0: Option<(u16, u16)>,
    pub events: Vec<CursorEvent>,
}

impl CursorRow {
    pub fn singletons(&self) -> usize {
        self.events.iter().filter(|e| e.singleton()).count()
    }
    pub fn edge(&self) -> usize {
        self.events.iter().filter(|e| e.on_edge()).count()
    }
    pub fn corner(&self) -> usize {
        self.events.iter().filter(|e| e.corner()).count()
    }
    /// The largest move that started from rest (> 5 ticks since the previous
    /// sample), measured from the previous known position.
    pub fn max_isolated_jump(&self) -> Option<u32> {
        self.events
            .iter()
            .filter(|e| e.gap_before.map_or(false, |g| g > 5))
            .filter_map(|e| e.jump)
            .max()
    }
    /// Runs of consecutive-tick samples (a hand moving): how many.
    pub fn streams(&self) -> usize {
        let mut n = 0;
        let mut in_stream = false;
        for e in &self.events {
            let contiguous = e.gap_before == Some(1);
            if contiguous && !in_stream {
                n += 1;
                in_stream = true;
            } else if !contiguous {
                in_stream = false;
            }
        }
        n
    }
}

/// The short way round the 16-bit wrap: a cursor crossing from x=82 to a monitor
/// on the left reads 65508, and that is a 110-unit move, not 65,426.
fn axis_delta(a: u16, b: u16) -> u32 {
    let d = (a as i32 - b as i32).unsigned_abs();
    d.min(65536 - d)
}

pub fn read_file(path: &Path) -> Result<CursorRow, String> {
    let p = path.to_string_lossy().to_string();
    let tape = Tape::from_file(&p)?;
    let a = tape.archives.first().ok_or_else(|| "no input archive".to_string())?;
    let tick0 = a.packets.first().and_then(|p| p.mouse);
    let mut events: Vec<CursorEvent> = Vec::new();
    let mut prev_pos: Option<(u16, u16)> = tick0;
    let mut prev_tick: Option<usize> = if tick0.is_some() { Some(0) } else { None };
    for (i, pk) in a.packets.iter().enumerate().skip(1) {
        if let Some((x, y)) = pk.mouse {
            let jump = prev_pos.map(|(px, py)| axis_delta(x, px) + axis_delta(y, py));
            events.push(CursorEvent {
                tick: i,
                race_ms: a.start_offset_ms as i64 + 10 * i as i64,
                x,
                y,
                gap_before: prev_tick.map(|t| i - t),
                gap_after: None,
                jump,
            });
            prev_pos = Some((x, y));
            prev_tick = Some(i);
        }
    }
    let n = events.len();
    for i in 0..n.saturating_sub(1) {
        let next = events[i + 1].tick;
        events[i].gap_after = Some(next - events[i].tick);
    }
    Ok(CursorRow {
        file: path.file_name().map(|s| s.to_string_lossy().to_string()).unwrap_or(p),
        ticks: a.packets.len(),
        tick0,
        events,
    })
}

fn collect(args: &[String]) -> Vec<PathBuf> {
    let mut out = Vec::new();
    for a in args {
        if a.starts_with("--") {
            continue;
        }
        let p = Path::new(a);
        if p.is_dir() {
            let mut v: Vec<PathBuf> = std::fs::read_dir(p)
                .map(|rd| {
                    rd.filter_map(|e| e.ok())
                        .map(|e| e.path())
                        .filter(|q| {
                            let s = q.to_string_lossy().to_ascii_lowercase();
                            s.ends_with(".gbx")
                        })
                        .collect()
                })
                .unwrap_or_default();
            v.sort();
            out.extend(v);
        } else {
            out.push(p.to_path_buf());
        }
    }
    out
}

fn pos(p: Option<(u16, u16)>) -> String {
    match p {
        Some((x, y)) => format!("{},{}", x, y),
        None => "-".to_string(),
    }
}

pub const HEADER: &str = "file\tticks\ttick0_cursor\tmidrun_samples\tstreams\tsingletons\tmax_isolated_jump\tedge\tcorner";

/// `ghost tape cursor FILE|DIR... [--events] [--tsv OUT]`
pub fn cmd(a: &[String]) {
    let files = collect(a);
    if files.is_empty() {
        crate::cli::die("ghost tape cursor FILE|DIR... [--events] [--tsv OUT]");
    }
    let show_events = has(a, "--events");
    let tsv = flag(a, "--tsv");
    let mut rows: Vec<CursorRow> = Vec::new();
    for f in &files {
        match read_file(f) {
            Ok(r) => rows.push(r),
            Err(e) => eprintln!("unreadable: {}: {}", f.display(), e),
        }
    }
    // Most cursor activity first; a quiet pile reads as a list of zeros.
    rows.sort_by(|p, q| q.events.len().cmp(&p.events.len()).then(p.file.cmp(&q.file)));
    let mut out = String::new();
    out.push_str(HEADER);
    out.push('\n');
    for r in &rows {
        out.push_str(&format!(
            "{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\n",
            r.file,
            r.ticks,
            pos(r.tick0),
            r.events.len(),
            r.streams(),
            r.singletons(),
            r.max_isolated_jump().map(|j| j.to_string()).unwrap_or_else(|| "-".to_string()),
            r.edge(),
            r.corner()
        ));
    }
    match tsv {
        Some(path) => {
            std::fs::write(&path, &out).unwrap_or_else(|e| crate::cli::die(format!("{}: {}", path, e)));
            eprintln!("wrote {} ({} rows)", path, rows.len());
        }
        None => print!("{}", out),
    }
    if show_events {
        for r in rows.iter().filter(|r| !r.events.is_empty()) {
            println!("\n{}  ({} ticks, tick-0 cursor {})", r.file, r.ticks, pos(r.tick0));
            println!("  tick   race_s     x      y   gap_before  gap_after   jump  notes");
            for e in &r.events {
                let mut notes = Vec::new();
                if e.singleton() {
                    notes.push("singleton");
                }
                if e.corner() {
                    notes.push("CORNER (0,0)");
                } else if e.on_edge() {
                    notes.push("edge");
                }
                if let (Some(g), Some(j)) = (e.gap_before, e.jump) {
                    if g > 5 && j > 100 {
                        notes.push("LARGE MOVE FROM REST");
                    }
                }
                println!(
                    "  {:>5}  {:>7.3}  {:>5}  {:>5}   {:>9}  {:>9}  {:>5}  {}",
                    e.tick,
                    e.race_ms as f64 / 1000.0,
                    e.x,
                    e.y,
                    e.gap_before.map(|g| g.to_string()).unwrap_or_else(|| "-".into()),
                    e.gap_after.map(|g| g.to_string()).unwrap_or_else(|| "-".into()),
                    e.jump.map(|j| j.to_string()).unwrap_or_else(|| "-".into()),
                    notes.join(", ")
                );
            }
        }
    }
    // The pile's base rates, the numbers a single file is judged against.
    let with_midrun = rows.iter().filter(|r| !r.events.is_empty()).count();
    let samples: usize = rows.iter().map(|r| r.events.len()).sum();
    let isolated: Vec<u32> = rows
        .iter()
        .flat_map(|r| r.events.iter())
        .filter(|e| e.gap_before.map_or(false, |g| g > 5))
        .filter_map(|e| e.jump)
        .collect();
    let edge: usize = rows.iter().map(|r| r.edge()).sum();
    let corner: usize = rows.iter().map(|r| r.corner()).sum();
    let tick0_edge = rows.iter().filter(|r| r.tick0.map_or(false, |(x, y)| x == 0 || y == 0)).count();
    let tick0_n = rows.iter().filter(|r| r.tick0.is_some()).count();
    eprintln!(
        "\n{} files: {} with mid-run cursor samples ({} samples); isolated moves (from rest) {}, largest {}; \
         mid-run samples on a screen edge {}, at the corner {}; tick-0 cursors {} of which on an edge {}",
        rows.len(),
        with_midrun,
        samples,
        isolated.len(),
        isolated.iter().max().map(|m| m.to_string()).unwrap_or_else(|| "-".into()),
        edge,
        corner,
        tick0_n,
        tick0_edge
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deltas_take_the_short_way_round_the_wrap() {
        assert_eq!(axis_delta(82, 65508), 110);
        assert_eq!(axis_delta(0, 337), 337);
        assert_eq!(axis_delta(5366, 5371), 5);
    }

    fn ev(tick: usize, x: u16, y: u16, before: Option<usize>, after: Option<usize>, jump: Option<u32>) -> CursorEvent {
        CursorEvent { tick, race_ms: 0, x, y, gap_before: before, gap_after: after, jump }
    }

    #[test]
    fn the_fingerprint_counts() {
        let r = CursorRow {
            file: "x".into(),
            ticks: 100,
            tick0: None,
            events: vec![
                ev(10, 337, 0, None, Some(40), None),
                ev(50, 0, 0, Some(40), None, Some(337)),
            ],
        };
        assert_eq!(r.singletons(), 2);
        assert_eq!(r.edge(), 2);
        assert_eq!(r.corner(), 1);
        assert_eq!(r.max_isolated_jump(), Some(337));
        assert_eq!(r.streams(), 0);
        let s = CursorRow {
            file: "y".into(),
            ticks: 100,
            tick0: Some((100, 100)),
            events: vec![
                ev(1, 105, 100, Some(1), Some(1), Some(5)),
                ev(2, 111, 100, Some(1), Some(30), Some(6)),
                ev(32, 113, 100, Some(30), None, Some(2)),
            ],
        };
        assert_eq!(s.streams(), 1);
        assert_eq!(s.singletons(), 1);
        assert_eq!(s.max_isolated_jump(), Some(2));
    }
}
