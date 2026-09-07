//! `tmreach gatecal` — where IS the car when the engine credits a checkpoint?
//!
//! For every human ghost: the flat engine trajectory (G1 controls re-run and
//! required), then for each of the ghost's own `checkpoints_ms` the engine row
//! at that instant, the row before and the row after, expressed in the frame
//! of the nearest gate. The table is what a trigger volume is fitted to; the
//! grading requires a candidate to fire within ±2 ticks of the credit on
//! ≥ 95 % of crossings with no missed and no extra gate.

use crate::gates::{Gate, MapGates, Volume};
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
    /// Engine clock of the row taken as "the credited tick" (cp_ms − 10, the
    /// label convention measured in G1).
    pub row_ms: i64,
    pub gate_wp: u32,
    pub d_centre: f64,
    /// Position at the credited row, one before, one after.
    pub p0: [f64; 3],
    pub pm: Option<[f64; 3]>,
    pub pp: Option<[f64; 3]>,
    pub speed: f64,
}

/// Local frame of a gate: `along` = the road axis (block dir / item yaw),
/// `lat` = across it, both horizontal unit vectors.
pub fn gate_axes(g: &Gate) -> ([f64; 3], [f64; 3]) {
    let yaw = match (g.dir, g.yaw) {
        // grid dir: 0 = +z? Measured below by gatecal, the convention is only
        // a labelling: the grader tries both axis assignments.
        (Some(d), _) => d as f64 * std::f64::consts::FRAC_PI_2,
        (None, Some(y)) => y,
        _ => 0.0,
    };
    let along = [yaw.sin(), 0.0, yaw.cos()];
    let lat = [yaw.cos(), 0.0, -yaw.sin()];
    (along, lat)
}

pub fn local(g: &Gate, p: [f64; 3]) -> (f64, f64, f64) {
    let (along, lat) = gate_axes(g);
    let d = [p[0] - g.centre[0], p[1] - g.centre[1], p[2] - g.centre[2]];
    (
        d[0] * along[0] + d[2] * along[2],
        d[0] * lat[0] + d[2] * lat[2],
        d[1],
    )
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
        let row_ms = ((cp_ms - 10) as f64 / 10.0).ceil() as i64 * 10;
        let idx = rep.flat.iter().position(|r| r.time_ms == row_ms);
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
        crossings.push(Crossing {
            ghost: name.clone(),
            cp_idx: i,
            cp_ms,
            row_ms,
            gate_wp: g.waypoint,
            d_centre: d,
            p0: pos(r0),
            pm: idx.checked_sub(1).map(|j| pos(&rep.flat[j])),
            pp: rep.flat.get(idx + 1).map(pos),
            speed: crate::rig::speed(r0),
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
    let (a0, l0, u0) = local(g, c.p0);
    let (am, lm, um) = c.pm.map(|p| local(g, p)).unwrap_or((f64::NAN, f64::NAN, f64::NAN));
    let (ap, lp, up) = c.pp.map(|p| local(g, p)).unwrap_or((f64::NAN, f64::NAN, f64::NAN));
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
pub fn grade(runs: &[GhostRun], gates: &MapGates, vol_of: &dyn Fn(&Gate) -> Volume) -> Grade {
    let mut g = Grade::default();
    for run in runs {
        // detector's first entry per gate, engine clock
        let mut first: Vec<Option<i64>> = vec![None; gates.gates.len()];
        for r in &run.flat {
            for (gi, gate) in gates.gates.iter().enumerate() {
                if first[gi].is_none() && vol_of(gate).contains(gate.centre, pos(r)) {
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
                    let dt = (t - c.row_ms) / 10;
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
