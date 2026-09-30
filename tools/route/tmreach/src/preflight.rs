//! LAUNCH CONTROLS for a lap-search lane (the parent's audit, coordinator 2026-09-10 11:24Z): four checks that run before
//! any lane starts, each refusing the launch with a one-line reason.
//!
//! (a) INPUT LIFE — the engine still acts on inputs at the lane's expected leg time (a template inherits its donor's input
//!     life: after the donor's finish split every input is ignored — ENV/GEN 06:20Z). Full-left vs full-right pulse at
//!     `leg_time_s` from the template's own inputs (or a known chain); end states must differ by > 0.5 m.
//! (b) PACKET MODES — the template tape's input packets are all mode 2 (a trimmed tape padded with the donor's last packet
//!     mode caps the input life; GEN's setup-map refuses modes ≠ [2]).
//! (c) GATE PLACEMENT — every gate in the order lies within `gate_tol_m` (20 m) of the line and the gates' `s` are monotone
//!     along the order (a gate the line never passes, or passes out of order, makes the scorer credit the wrong leg).
//! (d) HUMAN IDENTITY — the human tape replayed through the scorer projects monotone onto the line and credits the gates in
//!     the order (the scorer's identity on the run the line came from). Optional: needs a human tape for THIS map
//!     (`human_tape`; a borrowed-donor template's own inputs do not drive this map and are skipped with a note).
//!
//! `run` returns the report lines; `Err` = REFUSE with the first failing reason. GEN owns lap.rs and wires the call.

use crate::rig::{pos, speed, Worker};
use forkoracle::forksrv::Rec;

pub struct PreflightCfg {
    pub map: std::path::PathBuf,
    pub tape: std::path::PathBuf,
    pub work: std::path::PathBuf,
    pub server: std::path::PathBuf,
    pub shim: std::path::PathBuf,
    pub track: crate::lap::Track,
    pub gates: Option<crate::gates::MapGates>,
    /// the lane's expected leg time (race seconds) for the input-life pulse
    pub leg_time_s: f64,
    /// a known-good chain (best.tsv rows) to drive the car to the pulse instead of the template's own inputs
    pub pulse_chain: Option<Vec<Rec>>,
    /// a human tape for this map (Ghost.Gbx with a tape) for check (d); None = skipped with a note
    pub human_tape: Option<std::path::PathBuf>,
    pub gate_tol_m: f64,
    pub verbose: bool,
}

pub struct Preflight {
    pub lines: Vec<String>,
}

fn refuse(check: &str, why: String) -> String {
    format!("PREFLIGHT REFUSED [{check}]: {why}")
}

/// (b) packet modes of the template tape's first archive.
pub fn packet_modes(tape: &std::path::Path) -> Result<Vec<u32>, String> {
    let t = gbx::tape::Tape::from_file(&tape.to_string_lossy()).map_err(|e| format!("{}: {e}", tape.display()))?;
    let a0 = t.archives.first().ok_or_else(|| format!("{}: no input archive", tape.display()))?;
    let mut modes: Vec<u32> = a0.packets.iter().map(|p| p.mode).collect();
    modes.sort();
    modes.dedup();
    Ok(modes)
}

/// (c) gate placement against the line: (worst distance, first non-monotone pair) per the track's order.
pub fn gate_placement(track: &crate::lap::Track, gates: &crate::gates::MapGates, tol_m: f64) -> Result<Vec<String>, String> {
    let mut lines = Vec::new();
    let mut prev_s = f64::NEG_INFINITY;
    let mut hint = 0usize;
    for (k, grp) in track.order_groups.iter().enumerate() {
        // the group's gates: the nearest one to the line stands for the group (linked gates share a group)
        let mut best: Option<(f64, f64, u32)> = None;
        for g in gates.gates.iter().filter(|g| g.group == *grp && g.kind != crate::gates::GateKind::Start) {
            let (s, _lat, seg, d3) = track.project(g.centre, hint, track.pts.len());
            if best.map_or(true, |b| d3 < b.0) {
                best = Some((d3, s, g.waypoint));
                hint = seg;
            }
        }
        let (d3, s, wp) = best.ok_or_else(|| refuse("gates", format!("order position {k} names group {grp}, which has no gate in gates.json")))?;
        if d3 > tol_m {
            return Err(refuse("gates", format!("gate wp{wp} (group {grp}, order position {k}) is {d3:.1} m from the line (limit {tol_m:.0} m)")));
        }
        if s < prev_s - 1.0 {
            return Err(refuse("gates", format!("gate wp{wp} (group {grp}, order position {k}) projects to s {s:.0} m, before the previous gate at s {prev_s:.0} m — the order and the line disagree")));
        }
        lines.push(format!("  gate {k:2} group {grp:2} wp{wp:<3} s {s:7.1} m  {d3:5.1} m off the line"));
        prev_s = s;
    }
    Ok(lines)
}

fn chain_from_tsv(txt: &str) -> Vec<Rec> {
    txt.lines().skip(1).filter_map(|l| { let v: Vec<&str> = l.split('\t').collect(); if v.len() >= 4 { Some(Rec { steer: v[1].parse::<f32>().unwrap_or(0.0) / 127.0, gas: v[2].parse::<f32>().unwrap_or(0.0), brake: v[3].parse::<f32>().unwrap_or(0.0) }) } else { None } }).collect()
}

pub fn chain_file(p: &std::path::Path) -> Result<Vec<Rec>, String> {
    let txt = std::fs::read_to_string(p).map_err(|e| format!("{}: {e}", p.display()))?;
    let c = chain_from_tsv(&txt);
    if c.is_empty() {
        return Err(format!("{}: no rows (tick\tsteer\tgas\tbrake)", p.display()));
    }
    Ok(c)
}

/// (a) input-life pulse: end-state distance between a full-left and a full-right 1.5 s pulse at `at_s`.
/// Returns (live, pulse distance, car speed at the pulse). A car that is not driving at the pulse (speed < 3 m/s: the prefix
/// crashed or ended) cannot tell life from death — the caller refuses with "supply a --pulse-chain reaching the leg time".
pub fn input_life(w: &mut Worker, at_s: f64, prefix: Option<&[Rec]>) -> Result<(bool, f64, f64), String> {
    let root = w.root_probe;
    let n0 = (at_s * 100.0).round() as usize;
    let mut left: Vec<Rec> = match prefix {
        Some(c) => c.to_vec(),
        None => w.reference_recs(root, n0.min(w.n_ticks().saturating_sub(root))),
    };
    left.truncate(n0);
    while left.len() < n0 {
        left.push(Rec { steer: 0.0, gas: 1.0, brake: 0.0 });
    }
    let mut right = left.clone();
    for _ in 0..150 {
        left.push(Rec { steer: -1.0, gas: 1.0, brake: 0.0 });
        right.push(Rec { steer: 1.0, gas: 1.0, brake: 0.0 });
    }
    let (rl, hl) = w.rollout_keep(branch::ROOT, &left, root, left.len() as u64)?;
    w.release(hl);
    let (rr, hr) = w.rollout_keep(branch::ROOT, &right, root, right.len() as u64)?;
    w.release(hr);
    let (el, er) = (rl.last().ok_or("input-life: no rows")?, rr.last().ok_or("input-life: no rows")?);
    let d = ((el.x - er.x).powi(2) + (el.y - er.y).powi(2) + (el.z - er.z).powi(2)).sqrt();
    let v_at = rl.get(n0.saturating_sub(1)).map(speed).unwrap_or(0.0);
    Ok((d > 0.5, d, v_at))
}

/// (d) the human tape through the scorer: monotone projection and credits in the order.
pub fn human_identity(w: &mut Worker, track: &crate::lap::Track, gates: Option<&crate::gates::MapGates>, recs: &[Rec]) -> Result<Vec<String>, String> {
    let root = w.root_probe;
    let n = recs.len().min(w.n_ticks().saturating_sub(root));
    let (rows, nh) = w.rollout_keep(branch::ROOT, &recs[..n], root, n as u64)?;
    w.release(nh);
    let mut lines = Vec::new();
    // walking projector, s must not fall back by more than 8 m between samples 0.1 s apart
    let mut hint = track.project(pos(&rows[0]), 0, track.pts.len()).2;
    let mut s_prev = f64::NEG_INFINITY;
    let mut worst_back = 0.0f64;
    let mut worst_at = 0usize;
    let mut credited: Vec<u32> = Vec::new();
    let mut prev_cps = rows[0].cps;
    for (i, r) in rows.iter().enumerate() {
        if i % 10 == 0 {
            let (s, _lat, seg, _d3) = track.project(pos(r), hint, 30);
            hint = seg;
            if s < s_prev {
                let back = s_prev - s;
                if back > worst_back {
                    worst_back = back;
                    worst_at = i;
                }
            }
            s_prev = s.max(s_prev);
        }
        if r.cps != u32::MAX && prev_cps != u32::MAX && r.cps > prev_cps {
            // attribute the credit to the nearest gate centre → its group
            let grp = gates.and_then(|g| g.gates.iter().filter(|gg| gg.kind != crate::gates::GateKind::Start).map(|gg| (crate::rig::dist(pos(r), gg.centre), gg.group)).fold(None, |a: Option<(f64, u32)>, b| if a.map_or(true, |x| b.0 < x.0) { Some(b) } else { a })).filter(|(d, _)| *d < 30.0).map(|(_, g)| g).unwrap_or(u32::MAX);
            credited.push(grp);
            lines.push(format!("  credit #{} at race {} ({:.1}, {:.1}, {:.1}) v {:.1} -> group {}", credited.len(), crate::secs(w.race_of(r)), r.x, r.y, r.z, speed(r), if grp == u32::MAX { "?".into() } else { grp.to_string() }));
        }
        if r.cps != u32::MAX {
            prev_cps = r.cps;
        }
    }
    if worst_back > 8.0 {
        return Err(refuse("human", format!("the human tape projects NON-monotone on the line: s falls back {worst_back:.1} m around tick {worst_at} (race {}) — the line does not follow the human's run", crate::secs(w.race_of(&rows[worst_at])))));
    }
    // credits must be the order's groups, in order (a prefix is fine when the tape is cut short)
    for (k, grp) in credited.iter().enumerate() {
        match track.order_groups.get(k) {
            Some(want) if want == grp => {}
            Some(want) => return Err(refuse("human", format!("the human tape's credit #{} goes to group {} but the order wants group {want} at position {k}", k + 1, if *grp == u32::MAX { "?".into() } else { grp.to_string() }))),
            None => return Err(refuse("human", format!("the human tape takes {} credits, the order has {} groups", credited.len(), track.order_groups.len()))),
        }
    }
    if credited.is_empty() {
        return Err(refuse("human", "the human tape credits NO gate through the scorer (wrong map, wrong template, or the tape does not drive)".into()));
    }
    lines.push(format!("  human tape: {} rows, {} credits in the order's sequence, worst s fall-back {worst_back:.1} m", rows.len(), credited.len()));
    Ok(lines)
}

/// The four controls. Ok(report) = LAUNCH; Err(one line) = REFUSE.
pub fn run(cfg: &PreflightCfg) -> Result<Preflight, String> {
    let mut lines = Vec::new();
    // (b) first: no engine needed
    let modes = packet_modes(&cfg.tape)?;
    if modes != vec![2] {
        return Err(refuse("modes", format!("template {} has packet modes {modes:?}; a lane needs [2] only (a trimmed tape padded with the donor's last mode caps the input life)", cfg.tape.display())));
    }
    lines.push(format!("  modes: {:?} OK", modes));
    // (c)
    match &cfg.gates {
        Some(g) => lines.extend(gate_placement(&cfg.track, g, cfg.gate_tol_m)?),
        None => lines.push("  gates: no gates.json given — placement not checked".into()),
    }
    // (a) and (d) need the worker
    let mut w = Worker::start(&cfg.server, &cfg.map, &cfg.shim, &cfg.work, &cfg.tape, cfg.verbose)?;
    let (live, d, v_at) = input_life(&mut w, cfg.leg_time_s, cfg.pulse_chain.as_deref())?;
    if !live && v_at < 3.0 {
        return Err(refuse("input-life", format!("the car is not driving at race {:.1} s (speed {v_at:.1} m/s: the prefix inputs crashed or ended before the leg time) — the pulse cannot tell life from death; supply --pulse-chain with a chain that reaches the leg time", cfg.leg_time_s)));
    }
    if !live {
        return Err(refuse("input-life", format!("the engine ignores inputs at race {:.1} s on {} (left/right pulse end states {d:.2} m apart) — the donor's finish split caps this template; build a longer-life template", cfg.leg_time_s, cfg.tape.display())));
    }
    lines.push(format!("  input-life at race {:.1} s: LIVE ({d:.1} m between the pulses; car at {v_at:.1} m/s)", cfg.leg_time_s));
    match &cfg.human_tape {
        Some(p) => {
            let mut hw = Worker::start(&cfg.server, &cfg.map, &cfg.shim, &cfg.work.join("human"), p, cfg.verbose)?;
            let root = hw.root_probe;
            let n = hw.n_ticks().saturating_sub(root);
            let recs = hw.reference_recs(root, n);
            lines.extend(human_identity(&mut hw, &cfg.track, cfg.gates.as_ref(), &recs)?);
        }
        None => lines.push("  human identity: no --human-tape for this map — skipped (the template's own inputs are the donor's)".into()),
    }
    Ok(Preflight { lines })
}
