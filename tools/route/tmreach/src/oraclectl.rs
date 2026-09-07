//! `tmreach oraclectl` — G2 control (ii): varied rollouts written as full
//! tapes and re-simulated by the PLAIN oracle (no shim, no fork). The oracle's
//! checkpoint count for each tape must equal the detector's count over the
//! same run (human prefix + macro + brake tail). Bar: N/N.

use crate::fanout::{credited_before, label_of_tick};
use crate::gates::{Detector, GateKind, MapGates};
use crate::macros::{build, Built, Macro};
use crate::rig::{dist, pos, Worker};
use crate::starts::{run_on_worker, StartsOpts};
use crate::tele::Telemetry;
use forkoracle::forksrv::rec_of;
use std::path::PathBuf;
use std::sync::Arc;

pub struct CtlCfg {
    pub lib: Arc<Vec<Macro>>,
    pub det: Arc<Detector>,
    pub gates: Arc<MapGates>,
    pub out: PathBuf,
    /// Start every this many ms of race time.
    pub every_ms: i64,
    pub horizon: usize,
    /// Macro ids to run per start (a subset of the library).
    pub macro_ids: Vec<u16>,
    /// Write every case's per-tick rows beside its tape (diagnosis).
    pub save_rows: bool,
}

#[derive(Clone, Debug)]
pub struct Case {
    pub ghost: String,
    pub start_tick: usize,
    pub macro_id: u16,
    pub tape: PathBuf,
    /// The detector's count over the whole run (prefix + rollout to the end).
    pub det_cps: u32,
    pub det_finished: bool,
    /// RACE time (label + shift) of the detector's finish-crossing row, if any.
    pub det_finish_ms: Option<i64>,
    /// The finish falls AFTER the tape's own last record: the engine ran on heap contents
    /// and the plain oracle's answer is batch-dependent (perf arm, 2026-09-07) -- its own class.
    pub finish_after_tape: bool,
    /// Closest approach (m) to any gate the detector did NOT credit in the rollout.
    pub near_miss_m: f64,
    pub near_miss_gate: Option<u32>,
    pub rows: usize,
    pub exited: bool,
    /// Rows the child traced past the assumed adjudication window (declared + grace).
    pub rows_past_cut: usize,
    pub oracle_cps: Option<u32>,
    pub oracle_ms: Option<i64>,
    pub oracle_desc: String,
    /// (waypoint, s, lat, up) at the detector's crossing row, for gates credited DURING the rollout.
    pub crossings: Vec<(u32, f64, f64, f64)>,
}

pub fn cases_for_ghost(w: &mut Worker, tel: &Telemetry, cfg: &CtlCfg, gi: usize) -> Result<Vec<Case>, String> {
    let o = StartsOpts { every_ms: cfg.every_ms, out: None, trace_out: None, verbose: false };
    let rep = run_on_worker(w, tel, &cfg.gates, &o)?;
    if !(rep.start_ctrl_pass && rep.identity.passes()) {
        return Err("startup controls FAILED".into());
    }
    let flat = rep.flat;
    let n = w.n_ticks();
    let off = w.tape.start_offset_ms as i64;
    let gates = &cfg.gates;
    let det = &cfg.det;
    let ng = gates.gates.len();
    let name = w.ghost.file_stem().map(|s| s.to_string_lossy().trim_end_matches(".Ghost").to_string()).unwrap_or_default();
    let mut cases = Vec::new();
    let mut next_ms = cfg.every_ms;
    let mut cursor = w.root_probe;
    let mut node: Option<branch::Handle> = None;
    while next_ms + 10 * cfg.horizon as i64 + 500 <= label_of_tick(w, n) {
        let k = ((next_ms - off) / 10).max(0) as usize;
        next_ms += cfg.every_ms;
        if k + 1 <= cursor {
            continue;
        }
        let want = k + 1 - cursor;
        let from = match node {
            None => 0,
            Some(h) => w.floor(h)?,
        };
        let (_, new_node) = w.rollout_keep(node.unwrap_or(branch::ROOT), &[], from, want as u64)?;
        if let Some(h) = node {
            w.release(h);
        }
        node = Some(new_node);
        let f = w.floor(new_node)?;
        cursor = f - 1;
        if f + cfg.horizon + 10 > n {
            break;
        }
        let credited = credited_before(det, gates, &flat, w, f);
        let prefix: Vec<forkoracle::layout::Row> = flat.iter().filter(|r| r.time_ms < label_of_tick(w, f)).cloned().collect();
        let start_label = label_of_tick(w, f - 1);
        let airborne = flat.iter().find(|r| r.time_ms == start_label).map(|r| r.vy.abs() > 2.0).unwrap_or(false);
        let base: Vec<(u8, u8, u8)> = (f..f + cfg.horizon).map(|t| (w.tape.steer[t], w.tape.accel[t], w.tape.brake[t])).collect();
        for mid in &cfg.macro_ids {
            let m = &cfg.lib[*mid as usize];
            let macro_recs = match build(m, &base, airborne) {
                Built::Recs(r) => r,
                Built::NoOp => continue,
            };
            // the whole remaining tape: macro, then brake to the end
            let mut recs = macro_recs.clone();
            for _ in f + cfg.horizon..n {
                recs.push(rec_of(0, 0, 1));
            }
            let mut rolled = w.rollout(new_node, &recs, f, (n - f) as u64 + 400)?;
            if rolled.exited {
                crate::rig::extrapolate_exit(&mut rolled.rows);
            }
            // the oracle adjudicates nothing later than the grace after the DECLARED time
            let cut = w.label_of_race(w.tape.declared_ms.unwrap_or(u32::MAX / 2) as i64 + ADJUDICATION_GRACE_MS);
            let rows_past_cut = rolled.rows.iter().filter(|r| r.time_ms > cut).count();
            rolled.rows.retain(|r| r.time_ms <= cut);
            // detector over prefix + rollout
            let mut all = prefix.clone();
            all.extend(rolled.rows.iter().cloned());
            // ENGINE-credited (Row::cps steps), geometry attributes -- what the dataset records
            let first = det.credits(gates, &all, &vec![false; ng], 5).gate_row;
            let det_cps = first.iter().filter(|t| **t >= 0).count() as u32;
            let mut det_finished = false;
            let mut det_finish_ms = None;
            let mut crossings = Vec::new();
            for (gi2, t) in first.iter().enumerate() {
                if *t >= 0 && gates.gates[gi2].kind == GateKind::Finish {
                    det_finished = true;
                    det_finish_ms = Some(w.race_of(&all[*t as usize]));
                }
                if *t >= 0 && (*t as usize) >= prefix.len() {
                    let r = &all[*t as usize];
                    let (s0, l0, u0) = gates.gates[gi2].local(pos(r));
                    crossings.push((gates.gates[gi2].waypoint, s0, l0, u0));
                }
            }
            // near miss: closest approach to an uncredited gate during the rollout
            let mut near = f64::INFINITY;
            let mut near_gate = None;
            for (gi2, g) in gates.gates.iter().enumerate() {
                if first[gi2] >= 0 {
                    continue;
                }
                for r in &rolled.rows {
                    let d = dist(pos(r), g.centre);
                    if d < near {
                        near = d;
                        near_gate = Some(g.waypoint);
                    }
                }
            }
            // write the tape: reference prefix, then exactly what was written
            let (mut s, mut g, mut b) = (w.tape.steer.clone(), w.tape.accel.clone(), w.tape.brake.clone());
            for (i, r) in recs.iter().enumerate() {
                let t = f + i;
                if t < n {
                    s[t] = (r.steer * 127.0).round() as i8 as u8;
                    g[t] = (r.gas > 0.5) as u8;
                    b[t] = (r.brake > 0.5) as u8;
                }
            }
            let tape_path = cfg.out.join("tapes").join(format!("g{gi}-{name}-t{f}-m{}.Ghost.Gbx", m.id));
            std::fs::create_dir_all(tape_path.parent().unwrap()).map_err(|e| e.to_string())?;
            if cfg.save_rows {
                crate::starts::write_trace(&tape_path.with_extension("rows.tsv"), &rolled.rows)?;
            }
            w.tape.write_candidate(&s, &g, &b, &tape_path)?;
            let _ = credited;
            cases.push(Case {
                ghost: name.clone(),
                start_tick: f,
                macro_id: m.id,
                tape: tape_path,
                det_cps,
                det_finished,
                det_finish_ms,
                finish_after_tape: det_finish_ms.map(|ms| ms > w.race_of_tick_end()).unwrap_or(false),
                near_miss_m: near,
                near_miss_gate: near_gate,
                rows: rolled.rows.len(),
                exited: rolled.exited,
                rows_past_cut,
                oracle_cps: None,
                oracle_ms: None,
                oracle_desc: String::new(),
                crossings,
            });
        }
    }
    if let Some(h) = node {
        w.release(h);
    }
    Ok(cases)
}

/// Run the plain oracle on every case's tape (batched) and fill in the answers.
pub fn adjudicate(server: &std::path::Path, map: &std::path::Path, cases: &mut [Case]) -> Result<(), String> {
    for chunk in cases.chunks_mut(40) {
        let files: Vec<&std::path::Path> = chunk.iter().map(|c| c.tape.as_path()).collect();
        let res = ghost::oracle::validate_many(server, &files, ghost::oracle::MapsMode::One(map), "tmreach-oraclectl")?;
        for c in chunk.iter_mut() {
            let fname = c.tape.file_name().unwrap().to_string_lossy().into_owned();
            if let Some(r) = res.iter().find(|r| r.file == fname || r.file.ends_with(&fname)) {
                c.oracle_cps = r.cps;
                c.oracle_ms = r.time_ms;
                c.oracle_desc = r.desc.trim().to_string();
                // "wrong simu" with no "reached some checkpoints" is ZERO checkpoints
                // (a DNF with k > 0 says "wrong simu, but reached some checkpoints (k out of N)").
                if c.oracle_cps.is_none() && r.time_ms.is_none() && r.desc.contains("wrong simu") {
                    c.oracle_cps = Some(0);
                }
            }
        }
    }
    Ok(())
}

pub fn case_tsv_header() -> &'static str {
    "ghost\tstart_tick\tmacro_id\tdet_cps\toracle_cps\tagree\tdet_finished\tdet_finish_ms\toracle_ms\tnear_miss_m\tnear_miss_gate\trows\texited\tcrossings(wp:s,lat,up)\toracle_desc\ttape\n"
}

pub fn case_tsv_row(c: &Case) -> String {
    format!(
        "{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{:.1}\t{}\t{}\t{}\t{}\t{}\t{}\n",
        c.ghost,
        c.start_tick,
        c.macro_id,
        c.det_cps,
        c.oracle_cps.map(|x| x.to_string()).unwrap_or("-".into()),
        if c.oracle_cps == Some(c.det_cps) { "yes" } else { "NO" },
        c.det_finished,
        c.det_finish_ms.map(crate::secs).unwrap_or("-".into()),
        c.oracle_ms.map(crate::secs).unwrap_or("-".into()),
        c.near_miss_m,
        c.near_miss_gate.map(|g| format!("wp{g}")).unwrap_or("-".into()),
        c.rows,
        c.exited,
        c.crossings.iter().map(|(wp, s, l, u)| format!("wp{wp}:{s:+.2},{l:+.2},{u:+.2}")).collect::<Vec<_>>().join(" "),
        c.oracle_desc.replace('\n', " "),
        c.tape.display()
    )
}

/// The plain oracle adjudicates nothing later than about this long after the
/// file's DECLARED time: finishes at declared + 2.203 s were credited, a
/// crossing at declared + 2.728 s was not (Summer 2026 - 01, 2026-09-07). The
/// tape itself ends at its last input EVENT and says nothing about this.
pub const ADJUDICATION_GRACE_MS: i64 = 2500;
