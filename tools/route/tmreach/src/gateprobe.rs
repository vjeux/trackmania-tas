//! `tmreach gateprobe` — put a car to a STOP at chosen distances in front of
//! a gate and ask the plain oracle whether it was credited. The decisive
//! instrument for a trigger's front face: the fan-out only samples it by
//! chance, and the human notices fix the crediting TICK, not the geometry.
//!
//! From the human's own run (its prefix keeps the line), brake (steer held at
//! the human's value, gas off, brake on) from tick t; t is bisected until the
//! resting position's `s` along the gate normal lands at the target. Each
//! tape is then adjudicated by the plain oracle. Needs cps_before ≥ 1 so the
//! oracle's count is visible (it is blind to a lone checkpoint).

use crate::gates::MapGates;
use crate::rig::{pos, speed, Worker};
use crate::starts::{run_on_worker, StartsOpts};
use crate::tele::Telemetry;
use forkoracle::forksrv::rec_of;
use std::path::{Path, PathBuf};

pub struct Probe {
    pub target_s: f64,
    pub brake_tick: usize,
    pub rest_s: f64,
    pub rest_lat: f64,
    pub rest_up: f64,
    pub rest_speed: f64,
    pub tape: PathBuf,
    pub cps_before: u32,
    pub oracle_cps: Option<u32>,
    pub oracle_desc: String,
}

/// Brake from tick `t` to the end; return the resting row (last row) and the
/// full recs written.
fn brake_run(w: &mut Worker, node: branch::Handle, f: usize, t: usize) -> Result<(Vec<forkoracle::layout::Row>, Vec<forkoracle::forksrv::Rec>), String> {
    let n = w.n_ticks();
    let mut recs = Vec::with_capacity(n - f);
    for k in f..n {
        if k < t {
            recs.push(rec_of(w.tape.steer[k], w.tape.accel[k], w.tape.brake[k]));
        } else {
            recs.push(rec_of(w.tape.steer[k], 0, 1));
        }
    }
    let rolled = w.rollout(node, &recs, f, (n - f) as u64 + 400)?;
    Ok((rolled.rows, recs))
}

pub fn probe_gate(w: &mut Worker, tel: &Telemetry, gates: &MapGates, det: &crate::gates::Detector, gate_wp: u32, targets: &[f64], out: &Path) -> Result<Vec<Probe>, String> {
    let o = StartsOpts { every_ms: 500, out: None, trace_out: None, verbose: false };
    let rep = run_on_worker(w, tel, gates, &o)?;
    if !(rep.start_ctrl_pass && rep.identity.passes()) {
        return Err("startup controls FAILED".into());
    }
    let flat = rep.flat;
    let g = gates.gate(gate_wp).ok_or("no such gate")?;
    let gi = gates.gates.iter().position(|x| x.waypoint == gate_wp).unwrap();
    // the human's crossing row of this gate (detector), and its tick
    let first = det.first_crossings(gates, &flat, &vec![false; gates.gates.len()]);
    let cross_row = first[gi];
    if cross_row < 0 {
        return Err(format!("the human never crossed wp{gate_wp} by the detector"));
    }
    let cross_tick = crate::rig::tick_of_ms(&w.tape, flat[cross_row as usize].time_ms);
    let cps_before = first.iter().enumerate().filter(|(i, t)| **t >= 0 && *i != gi && **t < cross_row).count() as u32;
    // a node ~4 s before the crossing
    let f0 = cross_tick.saturating_sub(400).max(w.root_probe + 2);
    let (_, node) = w.rollout_keep(branch::ROOT, &[], 0, (f0 - w.root_probe) as u64)?;
    let f = w.floor(node)?;
    let name = w.ghost.file_stem().map(|s| s.to_string_lossy().trim_end_matches(".Ghost").to_string()).unwrap_or_default();
    let mut probes = Vec::new();
    for &target in targets {
        // bisect the brake tick between f and the crossing tick + 50
        let (mut lo, mut hi) = (f + 1, cross_tick + 50);
        let mut best: Option<(usize, Vec<forkoracle::layout::Row>, Vec<forkoracle::forksrv::Rec>, f64)> = None;
        for _ in 0..12 {
            let t = (lo + hi) / 2;
            let (rows, recs) = brake_run(w, node, f, t)?;
            let last = rest_row(&rows).ok_or("no rows")?;
            let (s, _, _) = g.local(pos(last));
            if std::env::var("TMREACH_PROBE_DEBUG").is_ok() {
                eprintln!("  probe target {target:+.1}: t {t} (lo {lo} hi {hi}) -> rest s {s:+.2} at {} v {:.1}, {} rows", crate::secs(w.race_of(last)), speed(last), rows.len());
            }
            if best.as_ref().map(|b| (s - target).abs() < (b.3 - target).abs()).unwrap_or(true) {
                best = Some((t, rows.clone(), recs, s));
            }
            if (s - target).abs() < 0.15 || hi - lo <= 1 {
                break;
            }
            if s < target {
                lo = t + 1;
            } else {
                hi = t.saturating_sub(1).max(lo);
            }
        }
        let (t, rows, recs, s) = best.ok_or("bisection produced nothing")?;
        let last = rest_row(&rows).unwrap();
        let (_, lat, up) = g.local(pos(last));
        let (mut st, mut gs, mut br) = (w.tape.steer.clone(), w.tape.accel.clone(), w.tape.brake.clone());
        for (i, r) in recs.iter().enumerate() {
            let k = f + i;
            if k < st.len() {
                st[k] = (r.steer * 127.0).round() as i8 as u8;
                gs[k] = (r.gas > 0.5) as u8;
                br[k] = (r.brake > 0.5) as u8;
            }
        }
        let tape = out.join(format!("probe-{name}-wp{gate_wp}-s{:+.1}.Ghost.Gbx", target));
        std::fs::create_dir_all(out).map_err(|e| e.to_string())?;
        w.tape.write_candidate(&st, &gs, &br, &tape)?;
        crate::starts::write_trace(&tape.with_extension("rows.tsv"), &rows)?;
        probes.push(Probe { target_s: target, brake_tick: t, rest_s: s, rest_lat: lat, rest_up: up, rest_speed: speed(last), tape, cps_before, oracle_cps: None, oracle_desc: String::new() });
    }
    w.release(node);
    Ok(probes)
}

pub fn adjudicate(server: &Path, map: &Path, probes: &mut [Probe]) -> Result<(), String> {
    let files: Vec<&Path> = probes.iter().map(|p| p.tape.as_path()).collect();
    let res = ghost::oracle::validate_many(server, &files, ghost::oracle::MapsMode::One(map), "tmreach-gateprobe")?;
    for p in probes.iter_mut() {
        let fname = p.tape.file_name().unwrap().to_string_lossy().into_owned();
        if let Some(r) = res.iter().find(|r| r.file == fname || r.file.ends_with(&fname)) {
            p.oracle_cps = r.cps;
            p.oracle_desc = r.desc.trim().replace('\n', " ");
            if p.oracle_cps.is_none() && r.time_ms.is_none() && r.desc.contains("wrong simu") {
                p.oracle_cps = Some(0);
            }
        }
    }
    Ok(())
}

/// The STOP: the first row at which the car is below 1 m/s after having been
/// above 10 m/s (a braked car on a slope rolls away again afterwards -- one
/// probe came to rest at s +1.1 and then rolled 30 m back down over 5 s, so
/// the last row is not the rest).
pub fn rest_row(rows: &[forkoracle::layout::Row]) -> Option<&forkoracle::layout::Row> {
    let mut fast = false;
    for r in rows {
        let v = speed(r);
        if v > 10.0 {
            fast = true;
        } else if fast && v < 1.0 {
            return Some(r);
        }
    }
    rows.last()
}
