//! `tmreach effects` — locate the ACTIVE EFFECT STATE (reactor boost, turbo,
//! slow-motion, no-engine/no-brake/no-steer, cruise, fragile, reset) in the
//! engine's memory, behaviourally, the way the race clock and the checkpoint
//! counter were located: a known exact ghost drives over a special block
//! (`tmmaps census` names them: *SpecialBoost* = reactor, *SpecialTurbo*,
//! *SpecialNoEngine*, *SpecialReset*, …); the car's memory around the vehicle
//! state and the participant is read at EVERY tick through a window of ticks
//! around the crossing; a 4-byte slot whose series jumps at the crossing and
//! then steps by a constant per tick (−10 ms/tick for an 8000 ms reactor
//! timer, or −0.01 s/tick as f32) is a candidate. Evidence (offset, tick,
//! value series) goes to the player INPUT arm; forkoracle is not forked.
//!
//! Mechanics: the human's node is advanced ONE TICK at a time (`rollout_keep`
//! with one record), and the paused child's memory is read with
//! `forkoracle::procmem::read_at` — public API only.

use crate::rig::{pos, Worker};
use std::path::Path;

pub struct Window {
    pub name: &'static str,
    pub base: u64,
    pub len: usize,
}

pub struct EffectsOut {
    pub ticks: Vec<usize>,
    pub race_ms: Vec<i64>,
    pub pos: Vec<[f64; 3]>,
    /// per window: per tick: bytes
    pub dumps: Vec<Vec<Vec<u8>>>,
    pub windows: Vec<Window>,
}

/// Read the windows at every tick of `t0..t0+n` from the human's own run.
pub fn scan(w: &mut Worker, t0: usize, n: usize, windows: Vec<Window>) -> Result<EffectsOut, String> {
    let root_probe = w.root_probe;
    if t0 <= root_probe {
        return Err(format!("t0 {t0} must be past the root probe {root_probe}"));
    }
    // node at t0
    let recs = w.reference_recs(root_probe, t0 - root_probe);
    let (_, mut node) = w.rollout_keep(branch::ROOT, &recs, root_probe, (t0 - root_probe) as u64)?;
    let mut out = EffectsOut { ticks: Vec::new(), race_ms: Vec::new(), pos: Vec::new(), dumps: windows.iter().map(|_| Vec::new()).collect(), windows };
    let mut cursor = w.floor(node)?;
    for _ in 0..n {
        let pid = w.forest.pid_of(node)?;
        // read the windows of the PAUSED child at this tick
        for (wi, win) in out.windows.iter().enumerate() {
            let bytes = forkoracle::procmem::read_at(pid, win.base, win.len).ok_or_else(|| format!("read {} at {:#x} failed on pid {pid}", win.name, win.base))?;
            out.dumps[wi].push(bytes);
        }
        // one more tick
        let rec = w.reference_recs(cursor, 1);
        let (rows, next) = w.rollout_keep(node, &rec, cursor, 1)?;
        let r = rows.last().cloned();
        w.release(node);
        node = next;
        cursor = w.floor(node)?;
        out.ticks.push(cursor);
        if let Some(r) = r {
            out.race_ms.push(w.race_of(&r));
            out.pos.push(pos(&r));
        } else {
            out.race_ms.push(0);
            out.pos.push([0.0; 3]);
        }
    }
    w.release(node);
    Ok(out)
}

#[derive(Debug, Clone)]
pub struct Candidate {
    pub window: &'static str,
    pub offset: usize,
    pub kind: &'static str,
    pub first_tick_idx: usize,
    pub series: Vec<f64>,
    pub note: String,
}

/// Slots whose series is constant (often 0) before some tick, jumps, then
/// changes by a constant step per tick for at least `min_run` ticks.
pub fn find_timers(o: &EffectsOut, min_run: usize) -> Vec<Candidate> {
    let mut out = Vec::new();
    let nt = o.ticks.len();
    for (wi, win) in o.windows.iter().enumerate() {
        let d = &o.dumps[wi];
        if d.len() != nt {
            continue;
        }
        for off in (0..win.len.saturating_sub(4)).step_by(4) {
            let u: Vec<u32> = d.iter().map(|b| u32::from_le_bytes(b[off..off + 4].try_into().unwrap())).collect();
            let f: Vec<f32> = u.iter().map(|x| f32::from_bits(*x)).collect();
            // u32 timer: a run where u[k+1] = u[k] - step (step in 1..=20) for >= min_run
            for (kind, vals) in [("u32", u.iter().map(|x| *x as f64).collect::<Vec<f64>>()), ("f32", f.iter().map(|x| *x as f64).collect())] {
                if kind == "f32" && vals.iter().any(|v| !v.is_finite() || v.abs() > 1e6) {
                    continue;
                }
                let mut k = 1;
                while k + min_run < nt {
                    let step = vals[k] - vals[k + 1];
                    // down- or up-counting, ms-per-tick (10) or seconds-per-tick (0.01) or frames
                    let plausible = step.abs() >= 0.0005 && step.abs() <= 20.0 && step != 0.0;
                    if plausible && vals[k] != vals[k - 1] {
                        // count the run
                        let mut run = 1;
                        while k + run + 1 < nt && ((vals[k + run] - vals[k + run + 1]) - step).abs() < 1e-6 * (1.0 + step.abs()) + 1e-9 {
                            run += 1;
                        }
                        if run >= min_run {
                            let before = vals[k - 1];
                            out.push(Candidate {
                                window: win.name,
                                offset: off,
                                kind,
                                first_tick_idx: k,
                                series: vals[k.saturating_sub(2)..(k + run + 2).min(nt)].to_vec(),
                                note: format!("before {before}, jumps to {} then −{step}/tick for {run} ticks (down to {})", vals[k], vals[k + run]),
                            });
                            k += run;
                            continue;
                        }
                    }
                    k += 1;
                }
            }
        }
    }
    out
}

/// Slots that CHANGE exactly once in the window (flags: no-engine, cruise,
/// fragile, car kind) — byte-wise.
pub fn find_flags(o: &EffectsOut, at_idx: usize, radius: usize) -> Vec<Candidate> {
    let mut out = Vec::new();
    let nt = o.ticks.len();
    for (wi, win) in o.windows.iter().enumerate() {
        let d = &o.dumps[wi];
        if d.len() != nt {
            continue;
        }
        for off in 0..win.len {
            let s: Vec<u8> = d.iter().map(|b| b[off]).collect();
            let changes: Vec<usize> = (1..nt).filter(|k| s[*k] != s[k - 1]).collect();
            if changes.len() == 1 || changes.len() == 2 {
                let c0 = changes[0];
                if (c0 as i64 - at_idx as i64).abs() <= radius as i64 {
                    out.push(Candidate {
                        window: win.name,
                        offset: off,
                        kind: "u8",
                        first_tick_idx: c0,
                        series: s.iter().map(|x| *x as f64).step_by((nt / 40).max(1)).collect(),
                        note: format!("byte {} -> {} at tick idx {}{}", s[c0 - 1], s[c0], c0, if changes.len() == 2 { format!(", back to {} at idx {}", s[changes[1]], changes[1]) } else { String::new() }),
                    });
                }
            }
        }
    }
    out
}

pub fn write_dumps(o: &EffectsOut, dir: &Path) -> Result<(), String> {
    std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    let mut idx = String::from("tick\trace_ms\tx\ty\tz\n");
    for (i, t) in o.ticks.iter().enumerate() {
        idx.push_str(&format!("{}\t{}\t{:.3}\t{:.3}\t{:.3}\n", t, o.race_ms[i], o.pos[i][0], o.pos[i][1], o.pos[i][2]));
    }
    std::fs::write(dir.join("ticks.tsv"), idx).map_err(|e| e.to_string())?;
    for (wi, win) in o.windows.iter().enumerate() {
        let mut all = Vec::new();
        for b in &o.dumps[wi] {
            all.extend_from_slice(b);
        }
        std::fs::write(dir.join(format!("{}-{:#x}-{}.bin", win.name, win.base, win.len)), all).map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// Slots that take a value in [lo, hi] at some tick in `idx_lo..idx_hi` after
/// being outside it (the 8000 ms reactor countdown appearing at the pad).
pub fn find_appearing(o: &EffectsOut, lo: f64, hi: f64, idx_lo: usize, idx_hi: usize) -> Vec<Candidate> {
    let mut out = Vec::new();
    let nt = o.ticks.len();
    for (wi, win) in o.windows.iter().enumerate() {
        let d = &o.dumps[wi];
        if d.len() != nt {
            continue;
        }
        for off in (0..win.len.saturating_sub(4)).step_by(4) {
            let u: Vec<u32> = d.iter().map(|b| u32::from_le_bytes(b[off..off + 4].try_into().unwrap())).collect();
            for (kind, vals) in [("u32", u.iter().map(|x| *x as f64).collect::<Vec<f64>>()), ("f32", u.iter().map(|x| f32::from_bits(*x) as f64).collect())] {
                for k in idx_lo.max(1)..idx_hi.min(nt) {
                    let inside = vals[k] >= lo && vals[k] <= hi;
                    let before = vals[k - 1] >= lo && vals[k - 1] <= hi;
                    if inside && !before && vals[k - 1].is_finite() {
                        out.push(Candidate { window: win.name, offset: off, kind, first_tick_idx: k, series: vals[k.saturating_sub(1)..(k + 12).min(nt)].to_vec(), note: format!("{} -> {} at idx {k}", vals[k - 1], vals[k]) });
                        break;
                    }
                }
            }
        }
    }
    out
}
