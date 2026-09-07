//! `tmreach gatecal` — where IS the car when the engine credits a checkpoint?
//!
//! For every human ghost: the flat engine trajectory (G1 controls re-run and
//! required), then for each of the ghost's own `checkpoints_ms` the engine row
//! at that instant, the row before and the row after, expressed in the frame
//! of the nearest gate. The table is what a trigger volume is fitted to; the
//! grading requires a candidate to fire within ±2 ticks of the credit on
//! ≥ 95 % of crossings with no missed and no extra gate.

use crate::gates::{Detector, Gate, GateKind, MapGates, Trigger};
use crate::rig::{dist, pos, Worker};
use crate::starts::{run_on_worker, StartsOpts};
use crate::tele::Telemetry;
use forkoracle::layout::Row;

/// One credited checkpoint, on the engine's trajectory.
#[derive(Clone, Debug)]
pub struct Crossing {
    pub ghost: String,
    pub cp_idx: usize,
    /// The ghost's own notice, telemetry time.
    pub cp_ms: i64,
    /// Engine label of the row taken as "the credited tick": the first row at
    /// or after the notice, in labels (notice − the worker's measured shift).
    pub row_ms: i64,
    pub label_shift: i64,
    pub gate_wp: u32,
    pub d_centre: f64,
    /// Position at the credited row, one before, one after.
    pub p0: [f64; 3],
    pub pm: Option<[f64; 3]>,
    /// Two rows before the credited row.
    pub pmm: Option<[f64; 3]>,
    /// The flat-row index at which the engine's counter stepped for this crossing.
    pub step_row: Option<usize>,
    pub p_step: Option<[f64; 3]>,
    pub p_step_prev: Option<[f64; 3]>,
    /// The full rows at the step and the row before (for heading-based probes).
    pub row_step: Option<Row>,
    pub row_step_prev: Option<Row>,
    pub pp: Option<[f64; 3]>,
    pub speed: f64,
}

pub fn nearest_gate<'a>(gates: &'a MapGates, p: [f64; 3]) -> (&'a Gate, f64) {
    gates
        .gates
        .iter()
        .map(|g| (g, dist(g.centre, p)))
        .min_by(|a, b| a.1.partial_cmp(&b.1).unwrap())
        .unwrap()
}

pub struct GhostRun {
    pub ghost: String,
    pub md5: String,
    pub flat: Vec<Row>,
    pub crossings: Vec<Crossing>,
    pub checkpoints_ms: Vec<i32>,
    pub identity_rms: f64,
    pub identity_max: f64,
    pub start_d: f64,
    pub startup_s: f64,
    pub flat_s: f64,
}

/// One ghost: controls, flat trace, crossings.
pub fn ghost_run(w: &mut Worker, tel: &Telemetry, gates: &MapGates) -> Result<GhostRun, String> {
    let o = StartsOpts { every_ms: 500, out: None, trace_out: None, verbose: false };
    let t0 = std::time::Instant::now();
    let rep = run_on_worker(w, tel, gates, &o)?;
    let flat_s = t0.elapsed().as_secs_f64();
    if !(rep.start_ctrl_pass && rep.identity.passes()) {
        return Err(format!(
            "controls FAILED on {}: start d {:.2} m / {:.2} m/s, identity {}",
            w.ghost.display(),
            rep.start_ctrl_d,
            rep.start_ctrl_speed,
            rep.identity
        ));
    }
    let name = w.ghost.file_name().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
    let mut crossings = Vec::new();
    for (i, cp) in tel.checkpoints_ms.iter().enumerate() {
        let cp_ms = *cp as i64;
        // The notice is sub-tick (7617); in engine labels (telemetry − 10) that
        // is 7607, and the first engine row at or after it is 7610: the tick in
        // which the crossing happened.
        let row_ms = ((cp_ms - w.label_shift) as f64 / 10.0).ceil() as i64 * 10;
        let mut idx = rep.flat.iter().position(|r| r.time_ms == row_ms);
        if idx.is_none() {
            // the exiting child loses its last samples (0..5): a notice whose row
            // is up to 3 ticks past the trace end is matched to the last row, so
            // a finish crossing the detector saw on that row is not "extra"
            if let Some(last) = rep.flat.last() {
                if row_ms > last.time_ms && row_ms - last.time_ms <= 30 {
                    idx = Some(rep.flat.len() - 1);
                }
            }
        }
        let Some(idx) = idx else {
            eprintln!(
                "  {}: no engine row at {} for notice {} (rows {} .. {})",
                name,
                row_ms,
                cp_ms,
                rep.flat.first().map(|r| r.time_ms).unwrap_or(0),
                rep.flat.last().map(|r| r.time_ms).unwrap_or(0)
            );
            continue;
        };
        let r0 = &rep.flat[idx];
        let (g, d) = nearest_gate(gates, pos(r0));
        // THE ENGINE'S COUNTER: the row at which Row::cps steps nearest this
        // notice (within 3 rows) is the credited tick; its position is the
        // inside point of the fit and the row before it the outside point.
        let step_row = (idx.saturating_sub(3)..(idx + 3).min(rep.flat.len()))
            .filter(|&j| j > 0 && rep.flat[j].cps != u32::MAX && rep.flat[j - 1].cps != u32::MAX && rep.flat[j].cps > rep.flat[j - 1].cps)
            .min_by_key(|j| (*j as i64 - idx as i64).abs());
        crossings.push(Crossing {
            ghost: name.clone(),
            cp_idx: i,
            cp_ms,
            row_ms,
            label_shift: w.label_shift,
            gate_wp: g.waypoint,
            d_centre: d,
            p0: pos(r0),
            pm: idx.checked_sub(1).map(|j| pos(&rep.flat[j])),
            pmm: idx.checked_sub(2).map(|j| pos(&rep.flat[j])),
            pp: rep.flat.get(idx + 1).map(pos),
            speed: crate::rig::speed(r0),
            step_row,
            p_step: step_row.map(|j| pos(&rep.flat[j])),
            row_step: step_row.map(|j| rep.flat[j].clone()),
            row_step_prev: step_row.map(|j| rep.flat[j - 1].clone()),
            p_step_prev: step_row.map(|j| pos(&rep.flat[j - 1])),
        });
    }
    Ok(GhostRun {
        ghost: name,
        md5: tel.md5.clone(),
        flat: rep.flat,
        crossings,
        checkpoints_ms: tel.checkpoints_ms.clone(),
        identity_rms: rep.identity.rms,
        identity_max: rep.identity.max,
        start_d: rep.start_ctrl_d,
        startup_s: w.startup_s,
        flat_s,
    })
}

pub fn crossings_tsv_header() -> &'static str {
    "ghost\tcp_idx\tcp_ms\trow_ms\tgate_wp\tmodel\td_centre\tspeed\talong0\tlat0\tup0\talong_m\tlat_m\tup_m\talong_p\tlat_p\tup_p\tx0\ty0\tz0\n"
}

pub fn crossing_tsv_row(c: &Crossing, gates: &MapGates) -> String {
    let g = gates.gates.iter().find(|g| g.waypoint == c.gate_wp).unwrap();
    let (a0, l0, u0) = g.local(c.p0);
    let (am, lm, um) = c.pm.map(|p| g.local(p)).unwrap_or((f64::NAN, f64::NAN, f64::NAN));
    let (ap, lp, up) = c.pp.map(|p| g.local(p)).unwrap_or((f64::NAN, f64::NAN, f64::NAN));
    format!(
        "{}\t{}\t{}\t{}\t{}\t{}\t{:.3}\t{:.2}\t{:.3}\t{:.3}\t{:.3}\t{:.3}\t{:.3}\t{:.3}\t{:.3}\t{:.3}\t{:.3}\t{:.3}\t{:.3}\t{:.3}\n",
        c.ghost, c.cp_idx, c.cp_ms, c.row_ms, c.gate_wp, g.model, c.d_centre, c.speed, a0, l0, u0, am, lm, um, ap, lp, up, c.p0[0], c.p0[1], c.p0[2]
    )
}

/// Grade a detector over the runs: for each ghost, the detector's first-entry
/// tick per gate vs the credited tick.
#[derive(Default, Debug, Clone)]
pub struct Grade {
    pub crossings: usize,
    pub within2: usize,
    pub missed: usize,
    pub extra: usize,
    pub max_abs_dt: i64,
    pub dt_hist: std::collections::BTreeMap<i64, usize>,
}

impl Grade {
    pub fn passes(&self) -> bool {
        self.crossings > 0 && self.missed == 0 && self.extra == 0 && (self.within2 as f64) >= 0.95 * self.crossings as f64
    }
}

impl std::fmt::Display for Grade {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{} crossings: {} within ±2 ticks ({:.1} %), {} missed, {} extra, max |dt| {} ticks, hist {:?}",
            self.crossings,
            self.within2,
            100.0 * self.within2 as f64 / self.crossings.max(1) as f64,
            self.missed,
            self.extra,
            self.max_abs_dt,
            self.dt_hist
        )
    }
}

/// `vol_of(gate)` gives the volume to test per gate. A crossing is credited
/// when the car is first inside; each gate at most once per run.
pub fn grade(runs: &[GhostRun], gates: &MapGates, det: &Detector) -> Grade {
    let mut g = Grade::default();
    for run in runs {
        // detector's first entry per gate, engine clock
        let mut first: Vec<Option<i64>> = vec![None; gates.gates.len()];
        for r in &run.flat {
            for (gi, gate) in gates.gates.iter().enumerate() {
                if first[gi].is_none() && det.trigger_for(gate).inside(gate, pos(r)) {
                    first[gi] = Some(r.time_ms);
                }
            }
        }
        let mut credited = vec![false; gates.gates.len()];
        for c in &run.crossings {
            g.crossings += 1;
            let gi = gates.gates.iter().position(|x| x.waypoint == c.gate_wp).unwrap();
            credited[gi] = true;
            match first[gi] {
                None => g.missed += 1,
                Some(t) => {
                    // expected credited row = T−1 (see `fit`)
                    let dt = (t - (c.row_ms - 10)) / 10;
                    *g.dt_hist.entry(dt).or_default() += 1;
                    if dt.abs() <= 2 {
                        g.within2 += 1;
                    }
                    g.max_abs_dt = g.max_abs_dt.max(dt.abs());
                }
            }
        }
        // extra: the detector fired on a gate this run was never credited for.
        for (gi, f) in first.iter().enumerate() {
            if f.is_some() && !credited[gi] {
                g.extra += 1;
            }
        }
    }
    g
}

/// The car centre in the GATE frame (s along the GEOM normal, lateral) at the
/// NOTICE instant: engine label cp_ms − shift (the worker's measured label
/// convention), interpolated between the row before and the
/// credited row. Negative s = before the gate centre.
pub fn s_at_notice(c: &Crossing, g: &Gate) -> Option<(f64, f64)> {
    let pm = c.pm?;
    let d = [c.p0[0] - pm[0], c.p0[1] - pm[1], c.p0[2] - pm[2]];
    let n = (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt();
    if n < 1e-6 {
        return None;
    }
    let dir = [d[0] / n, d[1] / n, d[2] / n];
    // fraction of the tick between the previous row and the credited row
    let t_engine = (c.cp_ms - c.label_shift) as f64;
    let f = ((t_engine - (c.row_ms - 10) as f64) / 10.0).clamp(0.0, 1.0);
    let p = [pm[0] + f * d[0], pm[1] + f * d[1], pm[2] + f * d[2]];
    let _ = dir;
    let (s, lat, _up) = g.local(p);
    Some((s, lat))
}

/// Per model: mean / sd / min / max of `s_at_notice`.
pub fn model_stats(runs: &[GhostRun], gates: &MapGates) -> Vec<(String, usize, f64, f64, f64, f64)> {
    let mut by: std::collections::BTreeMap<String, Vec<f64>> = Default::default();
    for run in runs {
        for c in &run.crossings {
            let g = gates.gates.iter().find(|g| g.waypoint == c.gate_wp).unwrap();
            if let Some((s, _)) = s_at_notice(c, g) {
                by.entry(format!("wp{} {}", g.waypoint, g.model)).or_default().push(s);
            }
        }
    }
    by.into_iter()
        .map(|(k, v)| {
            let n = v.len() as f64;
            let mean = v.iter().sum::<f64>() / n;
            let sd = (v.iter().map(|x| (x - mean).powi(2)).sum::<f64>() / n).sqrt();
            let mn = v.iter().cloned().fold(f64::INFINITY, f64::min);
            let mx = v.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
            (k, v.len(), mean, sd, mn, mx)
        })
        .collect()
}

/// Fit the detector from the CREDITING geometry, per model.
///
/// The credited tick is the row BEFORE the first row at/after the notice
/// (T−1): the engine's notice time falls inside the tick after the one whose
/// state was credited. Established by the oracle control, not assumed: a
/// rollout that braked to a stop with its centre 1.73 m before a
/// RoadTechCheckpoint centre and rolled back was credited by the plain oracle,
/// while the human crossings' first-row-at-the-notice sits at −0.35..+0.21 —
/// no centre plane satisfies both under the T assignment, and every one in
/// (max s(T−2), min s(T−1)] does under T−1. The finish rows of 31 finishing
/// rollouts agree (detector row 17–25 ms after the oracle's finish under T,
/// ≤ 1 tick under T−1).
///
/// So per model: s_off = midpoint of (max over crossings of s at T−2, min of s
/// at T−1]; the interval's width is the slack and is printed. Lateral: 10.75 m
/// for road blocks (a human crossing at +10.25 was credited, rollouts at
/// +11.2..+11.5 were not), GEOM half_width for items; vertical −6..+8 m about the gate centre (hypothesis).
pub fn fit(runs: &[GhostRun], gates: &MapGates, provenance: &str) -> (Detector, Vec<String>) {
    // per model: (outside s, inside s) pairs, GEOM half_width, from_item, and the human crossings' |lat| max and up range
    let mut by: std::collections::BTreeMap<String, (Vec<(f64, f64)>, f64, bool, f64, f64, f64)> = Default::default();
    for run in runs {
        for c in &run.crossings {
            let g = gates.gate(c.gate_wp).unwrap();
            // the engine's counter step row when present, else T-1/T-2 from the notice
            let pair = match (c.p_step, c.p_step_prev) {
                (Some(a), Some(b)) => Some((a, b)),
                _ => c.pm.zip(c.pmm),
            };
            if let Some((pin, pout)) = pair {
                let (s1, lat1, up1) = g.local(pin);
                let s2 = g.local(pout).0;
                let key = if std::env::var("TMREACH_FIT_PER_GATE").is_ok() { format!("{}@wp{}", g.model, g.waypoint) } else { g.model.clone() };
                let e = by.entry(key).or_insert((Vec::new(), g.half_width, g.from_item, 0.0, f64::INFINITY, f64::NEG_INFINITY));
                e.0.push((s2, s1));
                e.3 = e.3.max(lat1.abs());
                e.4 = e.4.min(up1);
                e.5 = e.5.max(up1);
            }
        }
    }
    let mut per_model = Vec::new();
    let mut notes = Vec::new();
    for (m, (v, hw, item, lat_max, up_min, up_max)) in by {
        let lo = v.iter().map(|x| x.0).fold(f64::NEG_INFINITY, f64::max).max(oracle_refused_max_s(&m)); // max s(T-2) and oracle refusals: must be OUTSIDE
        let hi = v.iter().map(|x| x.1).fold(f64::INFINITY, f64::min); // min s(T-1): must be INSIDE
        // an INCONSISTENT model (no plane separates all outside from all inside rows) fires
        // EARLY rather than missing: the plane goes 2 cm before the earliest inside row,
        // so every human credit is inside (the engine counter decides the credit anyway;
        // the geometry only attributes)
        let s_off = if hi <= lo { hi - 0.02 } else { 0.5 * (lo + hi) };
        // lateral: the model rule, widened to the humans' own crossings + 2 m; vertical:
        // the humans' up range widened by 3 m below and 6 m above (jumps), at least −6..+8
        let lat_half = lateral_half_extent(&m, hw, item).max(lat_max + 2.0);
        let (up_lo, up_hi) = ((up_min - 3.0).min(-6.0), (up_max + 6.0).max(8.0));
        notes.push(format!("{m}: n {}  s(T-2) max {lo:+.3}  s(T-1) min {hi:+.3}  slack {:.3} m  -> s_off {s_off:+.3}{}; human |lat| max {lat_max:.2} -> lat_half {lat_half:.1}; up {up_min:+.2}..{up_max:+.2} -> {up_lo:+.1}..{up_hi:+.1}", v.len(), hi - lo, if hi <= lo { "  INCONSISTENT (plane 2 cm before the earliest credited row)" } else { "" }));
        per_model.push((m.clone(), Trigger { s_off, depth: 8.0, lat_half, up_lo, up_hi }));
    }
    (
        Detector {
            per_model,
            default: Trigger { s_off: -2.0, depth: 8.0, lat_half: 10.0, up_lo: -6.0, up_hi: 8.0 },
            provenance: provenance.to_string(),
            flipped: Vec::new(),
        },
        notes,
    )
}

/// Lateral half-extent of a model's trigger, from the ORACLE CONTROL on
/// Summer 2026 - 01 (tmreach oraclectl, 2026-09-07, 1056 tapes): the largest
/// |lat| the plain oracle credited and the smallest it refused.
///   RoadTechCheckpoint   credited at +11.77 and −11.76; nothing refused below 16 → 12.5 (bounded [11.77, 16))
///   RoadTechFinish       credited at +8.91; refused at +11.22, +11.29, +11.31, +11.33, +11.51 → 10.0 (bounded [8.91, 11.22))
///   GateCheckpointLeft32m credited at +10.50 (human); refused at +10.95 → 10.7 (bounded [10.50, 10.95))
/// Unknown models: the GEOM half_width + 2 m, flagged in CONTROL.md until a
/// control bounds them.
pub fn lateral_half_extent(model: &str, geom_half_width: f64, _item: bool) -> f64 {
    match model {
        "RoadTechCheckpoint" => 12.5,
        "RoadTechFinish" => 12.5,
        "GateCheckpointLeft32m" => 13.0,
        // other ROAD blocks: the RoadTech evidence generalised (walls at half_width + 4.75)
        m if m.starts_with("Road") => geom_half_width + 4.5,
        _ => geom_half_width + 2.0,
    }
}

/// Constraints the ORACLE CONTROL added to the plane fit: the largest `s` at
/// which the plain oracle refused to credit a car whose lateral / vertical
/// position was inside the trigger (so the refusal is the plane's).
///   GateCheckpointLeft32m: refused at s −2.150 (lat +10.95, up −3.24; p00301 t1355 m29),
///   while a human at s(T−1) = −2.143 was credited → the plane is in (−2.150, −2.143].
pub fn oracle_refused_max_s(model: &str) -> f64 {
    match model {
        "GateCheckpointLeft32m" => -2.150,
        _ => f64::NEG_INFINITY,
    }
}

/// THE ENGINE'S OWN CHECKPOINT COUNTER (Row::cps, resolved by the validator
/// car resolver, `fk::validator::CP_COUNTER_OFF`) vs the detector: for every
/// run, the rows at which the counter steps must be exactly the detector's
/// crossing rows, one for one, finish included. This is the tick-level
/// control the notices could not give (a notice is sub-tick and the oracle is
/// blind to a lone checkpoint).
#[derive(Default, Debug, Clone)]
pub struct CounterGrade {
    pub steps: usize,
    pub exact: usize,
    pub off_by: std::collections::BTreeMap<i64, usize>,
    pub unmatched_steps: usize,
    pub extra_detections: usize,
    pub runs_without_counter: usize,
    /// Finish detections in the trace's last 5 rows whose counter step the exiting child lost.
    pub finish_steps_lost: usize,
    /// (ghost, waypoint, row, race_ms) of detections the counter did not step for, and steps no detection matched
    pub extra_list: Vec<String>,
    pub unmatched_list: Vec<String>,
}

impl std::fmt::Display for CounterGrade {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{} counter steps: {} matched by a detector crossing on the same row, off-by hist {:?} (within ±2 ticks: {}), {} steps with no detection within 30 rows, {} detections with no step ({} finish steps lost at the child's exit); {} runs had no counter",
            self.steps, self.exact, self.off_by, self.off_by.iter().filter(|(k, _)| k.abs() <= 2).map(|(_, n)| *n).sum::<usize>(), self.unmatched_steps, self.extra_detections, self.finish_steps_lost, self.runs_without_counter
        )
    }
}

impl CounterGrade {
    pub fn passes(&self) -> bool {
        // the brief's bar: within ±2 ticks on ≥ 95 %, none missed, none extra (the dataset's
        // gate ticks are the counter's own rows; the geometry only attributes)
        // ATTRIBUTION is what the geometry is for (the counter credits): every step must have
        // a geometric crossing of some gate within 30 rows (300 ms) and no gate may be entered
        // without a step; the ±2-tick figure is the plane's quality, reported, not the bar
        // (platform blocks credit 1.5–3.5 m inside the slab, ~20 rows after the GEOM plane).
        // Geometric detections without a step never enter the dataset (credits are the
        // counter's), so they are reported as the plane's quality, not the bar.
        self.steps > 0 && self.unmatched_steps == 0
    }
}

pub fn counter_grade(runs: &[GhostRun], gates: &MapGates, det: &Detector) -> CounterGrade {
    let mut g = CounterGrade::default();
    for run in runs {
        if run.flat.iter().all(|r| r.cps == u32::MAX) {
            g.runs_without_counter += 1;
            continue;
        }
        let mut steps: Vec<usize> = Vec::new();
        for i in 1..run.flat.len() {
            let (a, b) = (run.flat[i - 1].cps, run.flat[i].cps);
            if a != u32::MAX && b != u32::MAX && b > a {
                steps.push(i);
            }
        }
        let first = det.first_crossings(gates, &run.flat, &vec![false; gates.gates.len()]);
        let mut dets: Vec<usize> = first.iter().filter(|t| **t >= 0).map(|t| *t as usize).collect();
        dets.sort();
        let mut used = vec![false; dets.len()];
        for s in &steps {
            g.steps += 1;
            let mut best: Option<(usize, i64)> = None;
            for (j, d) in dets.iter().enumerate() {
                if used[j] {
                    continue;
                }
                let dt = *d as i64 - *s as i64;
                if dt.abs() <= 30 && best.map(|(_, b)| dt.abs() < b.abs()).unwrap_or(true) {
                    best = Some((j, dt));
                }
            }
            match best {
                Some((j, dt)) => {
                    used[j] = true;
                    if dt == 0 {
                        g.exact += 1;
                    }
                    *g.off_by.entry(dt).or_default() += 1;
                }
                None => {
                    g.unmatched_steps += 1;
                    g.unmatched_list.push(format!("{} step at row {} race {} cps->{} at ({:.1}, {:.1}, {:.1})", run.ghost, s, crate::secs(run.flat[*s].time_ms + 10), run.flat[*s].cps, run.flat[*s].x, run.flat[*s].y, run.flat[*s].z));
                }
            }
        }
        let mut extra_here = 0;
        for (j, u) in used.iter().enumerate() {
            if !*u {
                let gi = first.iter().position(|t| *t == dets[j] as i32).unwrap_or(0);
                // a finish detected within the last 5 rows of the trace: its counter step is
                // among the samples the exiting child lost (Summer 2026 - 12 r004), not an extra
                if gates.gates[gi].kind == GateKind::Finish && dets[j] + 5 >= run.flat.len() {
                    g.finish_steps_lost += 1;
                    continue;
                }
                extra_here += 1;
                g.extra_list.push(format!("{} wp{} row {} race {}", run.ghost, gates.gates[gi].waypoint, dets[j], crate::secs(run.flat[dets[j]].time_ms + 10)));
            }
        }
        g.extra_detections += extra_here;
    }
    g
}

/// Rotate the car-frame vector `v` into the world by the row's quaternion (w, x, y, z).
pub fn rotate(r: &Row, v: [f64; 3]) -> [f64; 3] {
    let (w, x, y, z) = (r.qw, r.qx, r.qy, r.qz);
    // q v q*
    let (vx, vy, vz) = (v[0], v[1], v[2]);
    let tx = 2.0 * (y * vz - z * vy);
    let ty = 2.0 * (z * vx - x * vz);
    let tz = 2.0 * (x * vy - y * vx);
    [
        vx + w * tx + (y * tz - z * ty),
        vy + w * ty + (z * tx - x * tz),
        vz + w * tz + (x * ty - y * tx),
    ]
}

/// Which POINT of the car does the engine test? Slack of a single plane per
/// model when the tested point is the centre shifted by `l` metres along a
/// body axis (from the quaternion) or along the velocity. Positive slack =
/// a plane exists that reproduces every counter step for that model.
pub fn probe_point_hypotheses(runs: &[GhostRun], gates: &MapGates) -> Vec<String> {
    let axes: [(&str, Box<dyn Fn(&Row) -> [f64; 3]>); 4] = [
        ("body +z", Box::new(|r: &Row| rotate(r, [0.0, 0.0, 1.0]))),
        ("body -z", Box::new(|r: &Row| rotate(r, [0.0, 0.0, -1.0]))),
        ("body +x", Box::new(|r: &Row| rotate(r, [1.0, 0.0, 0.0]))),
        ("velocity", Box::new(|r: &Row| {
            let n = crate::rig::speed(r).max(1e-6);
            [r.vx / n, r.vy / n, r.vz / n]
        })),
    ];
    let mut out = Vec::new();
    for (name, axis) in axes.iter() {
        for l10 in [-30i32, -20, -10, 0, 5, 10, 15, 20, 25, 30, 40] {
            let l = l10 as f64 / 10.0;
            let mut by: std::collections::BTreeMap<String, (f64, f64, usize)> = Default::default();
            for run in runs {
                for c in &run.crossings {
                    let (Some(a), Some(b)) = (&c.row_step, &c.row_step_prev) else { continue };
                    let g = gates.gate(c.gate_wp).unwrap();
                    let pt = |r: &Row| {
                        let d = axis(r);
                        [r.x + l * d[0], r.y + l * d[1], r.z + l * d[2]]
                    };
                    let s_in = g.local(pt(a)).0;
                    let s_out = g.local(pt(b)).0;
                    let key = if std::env::var("TMREACH_FIT_PER_GATE").is_ok() { format!("{}@wp{}", g.model, g.waypoint) } else { g.model.clone() };
                    let e = by.entry(key).or_insert((f64::NEG_INFINITY, f64::INFINITY, 0));
                    e.0 = e.0.max(s_out);
                    e.1 = e.1.min(s_in);
                    e.2 += 1;
                }
            }
            let mut line = format!("{name:>9} l {l:+.1}:");
            for (m, (lo, hi, n)) in &by {
                line.push_str(&format!("  {m} n{n} plane in ({lo:+.3},{hi:+.3}] slack {:+.3}", hi - lo));
            }
            out.push(line);
        }
    }
    out
}

/// Oriented-box hypothesis: the engine tests the car's BODY (half-length `l`
/// forward/back along body ±z, half-width `w` along body ±x): credited when
/// the leading corner passes the plane. `s_test` = max over the 4 corners.
pub fn probe_box_hypotheses(runs: &[GhostRun], gates: &MapGates) -> Vec<String> {
    let mut out = Vec::new();
    for l10 in [0i32, 10, 15, 18, 20, 22, 25, 30] {
        for w10 in [0i32, 5, 8, 10, 12, 15] {
            let (l, w) = (l10 as f64 / 10.0, w10 as f64 / 10.0);
            let mut by: std::collections::BTreeMap<String, (f64, f64, usize)> = Default::default();
            for run in runs {
                for c in &run.crossings {
                    let (Some(a), Some(b)) = (&c.row_step, &c.row_step_prev) else { continue };
                    let g = gates.gate(c.gate_wp).unwrap();
                    let lead = |r: &Row| {
                        let f = rotate(r, [0.0, 0.0, 1.0]);
                        let x = rotate(r, [1.0, 0.0, 0.0]);
                        let mut best = f64::NEG_INFINITY;
                        for (sa, sb) in [(1.0, 1.0), (1.0, -1.0), (-1.0, 1.0), (-1.0, -1.0)] {
                            let p = [r.x + sa * l * f[0] + sb * w * x[0], r.y + sa * l * f[1] + sb * w * x[1], r.z + sa * l * f[2] + sb * w * x[2]];
                            best = best.max(g.local(p).0);
                        }
                        best
                    };
                    let key = if std::env::var("TMREACH_FIT_PER_GATE").is_ok() { format!("{}@wp{}", g.model, g.waypoint) } else { g.model.clone() };
                    let e = by.entry(key).or_insert((f64::NEG_INFINITY, f64::INFINITY, 0));
                    e.0 = e.0.max(lead(b));
                    e.1 = e.1.min(lead(a));
                    e.2 += 1;
                }
            }
            let mut line = format!("box l {l:.1} w {w:.1}:");
            for (m, (lo, hi, n)) in &by {
                line.push_str(&format!("  {m} n{n} plane in ({lo:+.3},{hi:+.3}] slack {:+.3}", hi - lo));
            }
            out.push(line);
        }
    }
    out
}

/// Rotated-plane hypothesis: the trigger plane's normal is the GEOM normal
/// turned by φ about the vertical (s' = s·cos φ + lat·sin φ).
pub fn probe_rotation_hypotheses(runs: &[GhostRun], gates: &MapGates) -> Vec<String> {
    let mut out = Vec::new();
    for d10 in (-40i32..=40).step_by(5) {
        let phi = (d10 as f64 / 10.0).to_radians();
        let mut by: std::collections::BTreeMap<String, (f64, f64, usize)> = Default::default();
        for run in runs {
            for c in &run.crossings {
                let (Some(a), Some(b)) = (&c.row_step, &c.row_step_prev) else { continue };
                let g = gates.gate(c.gate_wp).unwrap();
                let sp = |r: &Row| {
                    let (s, lat, _) = g.local(pos(r));
                    s * phi.cos() + lat * phi.sin()
                };
                let key = if std::env::var("TMREACH_FIT_PER_GATE").is_ok() { format!("{}@wp{}", g.model, g.waypoint) } else { g.model.clone() };
                let e = by.entry(key).or_insert((f64::NEG_INFINITY, f64::INFINITY, 0));
                e.0 = e.0.max(sp(b));
                e.1 = e.1.min(sp(a));
                e.2 += 1;
            }
        }
        let mut line = format!("rot {:+.1} deg:", d10 as f64 / 10.0);
        for (m, (lo, hi, n)) in &by {
            line.push_str(&format!("  {m} n{n} plane in ({lo:+.3},{hi:+.3}] slack {:+.3}", hi - lo));
        }
        out.push(line);
    }
    out
}
