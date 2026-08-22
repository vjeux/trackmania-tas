//! Driver side of the watchdog: the condition language, the reference line,
//! and the wire format that arms them inside the fork server.
//!
//! # The condition language
//!
//! One predicate per `--pred` flag, in three colon-separated parts:
//!
//! ```text
//!     NAME:KIND:key=value,key=value,...
//! ```
//!
//! `NAME` is free text and is what the search logs when the predicate fires.
//! `KIND` is one of:
//!
//! | kind | fires when | keys (defaults) |
//! |---|---|---|
//! | `speeddrop` | speed falls below `frac` of its peak over the last `win` ticks -- what a crash looks like | `frac=0.5`, `win=50`, `minpeak=8` (m/s, below which it is not armed), `need=1`, `after=0`, `until=` |
//! | `floor` | speed below `speed` for `need` consecutive ticks -- stopped, stuck, or facing a wall | `speed=3`, `need=30` |
//! | `box` | the car leaves an axis-aligned region -- off the track, off the map | `xmin= xmax= ymin= ymax= zmin= zmax=`, `need=1` |
//! | `offref` | the car is further than `dist` metres from the reference line | `dist=12`, `need=5` |
//! | `noprog` | net displacement over the last `win` ticks is under `dist` metres | `dist=5`, `win=100` |
//!
//! Common keys: `need` (consecutive ticks required), `after` / `until` (tape
//! tick range in which the predicate is live -- `after` is what keeps a
//! predicate off the standing start, where the car is legitimately stationary).
//!
//! Several may be armed at once; they are evaluated in the order given and the
//! first to trip wins, which is why each one is named.
//!
//! # The reference line and `progress`
//!
//! `offref` and the progress measure both use a reference trajectory: one
//! position per tape tick, from `fk btraj` (which measures ANY tape, including
//! a search incumbent with no recorded telemetry). Deviation is measured to the
//! nearest point of the line within a window of the last match, not to the
//! reference's position at the same millisecond: a candidate that is simply
//! 100 ms ahead of the incumbent has not left the line.
//!
//! `progress` is the arclength of that nearest point, maximised over the run
//! and only counted while inside `corridor` metres of the line. It is the
//! measure the search scores aborted candidates by, so it has to mean the same
//! thing for an aborted and a completed run -- which it does, because the child
//! computes it identically in both cases.

use crate::pred_core::{Pred, Summary, K_BOX, K_FLOOR, K_NOPROG, K_OFFREF, K_SPEEDDROP, PRED_BYTES};

/// One armed, named condition.
#[derive(Clone)]
pub struct NamedPred {
    pub name: String,
    pub pred: Pred,
}

/// A reference line resampled onto tape ticks, with its cumulative arclength.
#[derive(Clone, Default)]
pub struct RefLineData {
    pub n: usize,
    /// 3 * n
    pub xyz: Vec<f32>,
    /// n
    pub s: Vec<f32>,
}

impl RefLineData {
    pub fn from_points(pts: &[[f64; 3]]) -> RefLineData {
        let n = pts.len();
        let mut xyz = Vec::with_capacity(3 * n);
        let mut s = Vec::with_capacity(n);
        let mut acc = 0.0f64;
        for (i, p) in pts.iter().enumerate() {
            if i > 0 {
                let q = pts[i - 1];
                acc += ((p[0] - q[0]).powi(2) + (p[1] - q[1]).powi(2) + (p[2] - q[2]).powi(2))
                    .sqrt();
            }
            xyz.push(p[0] as f32);
            xyz.push(p[1] as f32);
            xyz.push(p[2] as f32);
            s.push(acc as f32);
        }
        RefLineData { n, xyz, s }
    }

    /// Arclength at a tape tick, for turning a checkpoint time into a progress
    /// threshold.
    pub fn s_at_tick(&self, tick: usize) -> f32 {
        if self.n == 0 {
            0.0
        } else {
            self.s[tick.min(self.n - 1)]
        }
    }

    /// Nearest point on the line, searched over the whole line: the offline
    /// twin of what the child does with a moving window.
    pub fn nearest(&self, p: [f64; 3]) -> (usize, f64) {
        let mut bi = 0;
        let mut bd = f64::MAX;
        for i in 0..self.n {
            let d = ((self.xyz[3 * i] as f64 - p[0]).powi(2)
                + (self.xyz[3 * i + 1] as f64 - p[1]).powi(2)
                + (self.xyz[3 * i + 2] as f64 - p[2]).powi(2))
            .sqrt();
            if d < bd {
                bd = d;
                bi = i;
            }
        }
        (bi, bd)
    }
}

/// Everything needed to arm a fork server.
#[derive(Clone, Default)]
pub struct Watch {
    pub preds: Vec<NamedPred>,
    pub refline: RefLineData,
    pub corridor: f32,
    pub ahead: i32,
    pub back: i32,
    /// Arclength of the reference's finish; predicates are disarmed past it.
    pub finish_s: f32,
    /// 1 = the cheap clock-gated sampling path in the child.
    pub fast: u32,
    /// World-x of the sub-tick timing plane; 0 disables it.
    pub plane_x: f32,
}

fn getf(kv: &[(String, String)], k: &str, d: f32) -> f32 {
    kv.iter()
        .find(|(a, _)| a == k)
        .and_then(|(_, v)| v.parse::<f32>().ok())
        .unwrap_or(d)
}
fn geti(kv: &[(String, String)], k: &str, d: i32) -> i32 {
    kv.iter()
        .find(|(a, _)| a == k)
        .and_then(|(_, v)| v.parse::<i32>().ok())
        .unwrap_or(d)
}

/// Parse one `NAME:KIND:k=v,...` spec. Unknown keys are an error rather than a
/// shrug: a typo in a watchdog's threshold is exactly the kind of mistake that
/// silently kills good candidates.
pub fn parse_spec(spec: &str) -> Result<NamedPred, String> {
    let parts: Vec<&str> = spec.splitn(3, ':').collect();
    if parts.len() < 2 {
        return Err(format!("bad predicate {:?}: want NAME:KIND[:k=v,...]", spec));
    }
    let name = parts[0].to_string();
    let kind_s = parts[1];
    let kv: Vec<(String, String)> = if parts.len() == 3 && !parts[2].is_empty() {
        let mut v = Vec::new();
        for item in parts[2].split(',') {
            let (a, b) = item
                .split_once('=')
                .ok_or_else(|| format!("bad key=value {:?} in {:?}", item, spec))?;
            v.push((a.trim().to_string(), b.trim().to_string()));
        }
        v
    } else {
        Vec::new()
    };
    let allowed: &[&str] = match kind_s {
        "speeddrop" => &["frac", "win", "minpeak", "need", "after", "until"],
        "floor" => &["speed", "need", "after", "until"],
        "box" => &["xmin", "xmax", "ymin", "ymax", "zmin", "zmax", "need", "after", "until"],
        "offref" => &["dist", "need", "after", "until"],
        "noprog" => &["dist", "win", "need", "after", "until"],
        k => return Err(format!("unknown predicate kind {:?}", k)),
    };
    for (k, _) in &kv {
        if !allowed.contains(&k.as_str()) {
            return Err(format!(
                "predicate {} ({}): unknown key {:?}; allowed: {:?}",
                name, kind_s, k, allowed
            ));
        }
    }
    let mut p = Pred::ZERO;
    p.after = geti(&kv, "after", 0);
    p.until = geti(&kv, "until", i32::MAX);
    match kind_s {
        "speeddrop" => {
            p.kind = K_SPEEDDROP;
            p.win = geti(&kv, "win", 50).max(1) as u32;
            p.need = geti(&kv, "need", 1).max(1) as u32;
            p.p[0] = getf(&kv, "frac", 0.5);
            p.p[1] = getf(&kv, "minpeak", 8.0);
        }
        "floor" => {
            p.kind = K_FLOOR;
            p.need = geti(&kv, "need", 30).max(1) as u32;
            p.p[0] = getf(&kv, "speed", 3.0);
        }
        "box" => {
            p.kind = K_BOX;
            p.need = geti(&kv, "need", 1).max(1) as u32;
            p.p[0] = getf(&kv, "xmin", f32::NEG_INFINITY);
            p.p[1] = getf(&kv, "xmax", f32::INFINITY);
            p.p[2] = getf(&kv, "ymin", f32::NEG_INFINITY);
            p.p[3] = getf(&kv, "ymax", f32::INFINITY);
            p.p[4] = getf(&kv, "zmin", f32::NEG_INFINITY);
            p.p[5] = getf(&kv, "zmax", f32::INFINITY);
        }
        "offref" => {
            p.kind = K_OFFREF;
            p.need = geti(&kv, "need", 5).max(1) as u32;
            p.p[0] = getf(&kv, "dist", 12.0);
        }
        "noprog" => {
            p.kind = K_NOPROG;
            p.win = geti(&kv, "win", 100).max(1) as u32;
            p.need = geti(&kv, "need", 1).max(1) as u32;
            p.p[0] = getf(&kv, "dist", 5.0);
        }
        _ => unreachable!(),
    }
    if p.win as usize >= crate::pred_core::RINGW {
        return Err(format!(
            "predicate {}: win={} exceeds the child's ring buffer ({} ticks)",
            name,
            p.win,
            crate::pred_core::RINGW
        ));
    }
    Ok(NamedPred { name, pred: p })
}

impl Watch {
    pub fn new() -> Watch {
        Watch {
            preds: Vec::new(),
            refline: RefLineData::default(),
            corridor: 40.0,
            ahead: 24,
            back: 8,
            finish_s: 0.0,
            fast: 1,
            plane_x: 0.0,
        }
    }

    pub fn describe(&self) -> String {
        let mut s = String::new();
        for (i, np) in self.preds.iter().enumerate() {
            let p = &np.pred;
            s.push_str(&format!(
                "  [{}] {:<10} {:<10} need={} after={} p={:?}\n",
                i,
                np.name,
                crate::pred_core::kind_name(p.kind),
                p.need,
                p.after,
                &p.p[..2]
            ));
        }
        s
    }

    pub fn name_of(&self, i: i32) -> &str {
        if i < 0 || i as usize >= self.preds.len() {
            "-"
        } else {
            &self.preds[i as usize].name
        }
    }

    /// The `A` frame: predicates, record layout, watched segments, reference.
    pub fn arm_payload(
        &self,
        clock0: i64,
        off_clock: u32,
        off_pos: u32,
        off_vel: u32,
        rec_len: u32,
        segs: &[(u64, u32)],
    ) -> Vec<u8> {
        let mut v = Vec::with_capacity(64 + 16 * self.refline.n);
        v.push(b'A');
        v.extend_from_slice(&(self.preds.len() as u32).to_le_bytes());
        let mut buf = [0u8; PRED_BYTES];
        for np in &self.preds {
            np.pred.encode(&mut buf);
            v.extend_from_slice(&buf);
        }
        v.extend_from_slice(&clock0.to_le_bytes());
        v.extend_from_slice(&off_clock.to_le_bytes());
        v.extend_from_slice(&off_pos.to_le_bytes());
        v.extend_from_slice(&off_vel.to_le_bytes());
        v.extend_from_slice(&rec_len.to_le_bytes());
        v.extend_from_slice(&(segs.len() as u32).to_le_bytes());
        for (a, l) in segs {
            v.extend_from_slice(&a.to_le_bytes());
            v.extend_from_slice(&l.to_le_bytes());
        }
        v.extend_from_slice(&self.corridor.to_le_bytes());
        v.extend_from_slice(&self.ahead.to_le_bytes());
        v.extend_from_slice(&self.back.to_le_bytes());
        v.extend_from_slice(&self.finish_s.to_le_bytes());
        v.extend_from_slice(&self.fast.to_le_bytes());
        v.extend_from_slice(&(self.refline.n as u32).to_le_bytes());
        for f in &self.refline.xyz {
            v.extend_from_slice(&f.to_le_bytes());
        }
        for f in &self.refline.s {
            v.extend_from_slice(&f.to_le_bytes());
        }
        // trailing, so an older shim simply ignores it
        v.extend_from_slice(&self.plane_x.to_le_bytes());
        v
    }
}

/// What one watched candidate did.
#[derive(Clone, Debug)]
pub struct Outcome {
    /// Finish time in ms, if it finished.
    pub time: Option<i64>,
    /// Checkpoints reached, when the validator reported them (i.e. when the
    /// candidate was NOT aborted).
    pub cps: Option<u32>,
    pub sum: Option<Summary>,
}

impl Outcome {
    pub fn tripped(&self) -> Option<(i32, i32, f32)> {
        match &self.sum {
            Some(s) if s.trip_pred >= 0 => Some((s.trip_pred, s.trip_tick, s.trip_value)),
            _ => None,
        }
    }
    pub fn progress(&self) -> f32 {
        self.sum.map(|s| s.progress).unwrap_or(0.0)
    }
    pub fn travelled(&self) -> f32 {
        self.sum.map(|s| s.travelled).unwrap_or(0.0)
    }
    pub fn last_tick(&self) -> i32 {
        self.sum.map(|s| s.last_tick).unwrap_or(-1)
    }
    /// Continuous arrival time at the armed timing plane, in tape ticks
    /// (fractional). `None` when no plane was armed or the run never crossed
    /// it. Multiply by 10 and add the tape's `start_offset_ms` for race ms.
    pub fn cross(&self) -> Option<f64> {
        match &self.sum {
            Some(s) if s.cross_tick >= 0 => Some(s.cross_tick as f64 + s.cross_frac as f64),
            _ => None,
        }
    }
}

/// Turn the two frames of a `W` reply into an outcome.
pub fn outcome(json: &str, blob: &[u8]) -> Outcome {
    let (time, cps) = crate::forksrv::parse_result(json);
    Outcome {
        time,
        cps,
        sum: Summary::decode(blob),
    }
}

impl RefLineData {
    /// Read the first four columns (`time_ms,x,y,z`) of a trajectory CSV --
    /// the format `fk btraj` and `tmtraj decode --csv` both write -- and index
    /// it by tape tick. `fk btraj` emits one row per 10 ms tick, so this is a
    /// re-index rather than a resample; ticks before the first row (the
    /// standing start) clamp to it, and interior holes interpolate.
    pub fn from_csv(
        path: &str,
        start_offset_ms: i32,
        nticks: usize,
    ) -> Result<RefLineData, String> {
        let text = std::fs::read_to_string(path).map_err(|e| format!("{}: {}", path, e))?;
        let mut pts: Vec<Option<[f64; 3]>> = vec![None; nticks];
        let mut nrow = 0;
        for line in text.lines().skip(1) {
            let f: Vec<&str> = line.trim().split(',').collect();
            if f.len() < 4 {
                continue;
            }
            let (ms, x, y, z) = match (
                f[0].parse::<i64>(),
                f[1].parse::<f64>(),
                f[2].parse::<f64>(),
                f[3].parse::<f64>(),
            ) {
                (Ok(a), Ok(b), Ok(c), Ok(d)) => (a, b, c, d),
                _ => continue,
            };
            let t = (ms - start_offset_ms as i64) / 10;
            if t >= 0 && (t as usize) < nticks {
                pts[t as usize] = Some([x, y, z]);
                nrow += 1;
            }
        }
        if nrow < 10 {
            return Err(format!("{}: only {} usable rows", path, nrow));
        }
        let first = pts.iter().position(|p| p.is_some()).unwrap();
        let last = pts.iter().rposition(|p| p.is_some()).unwrap();
        for i in 0..first {
            pts[i] = pts[first];
        }
        for i in last + 1..nticks {
            pts[i] = pts[last];
        }
        let mut i = first;
        while i <= last {
            if pts[i].is_some() {
                i += 1;
                continue;
            }
            let a = i - 1;
            let mut b = i;
            while pts[b].is_none() {
                b += 1;
            }
            let (pa, pb) = (pts[a].unwrap(), pts[b].unwrap());
            for k in a + 1..b {
                let u = (k - a) as f64 / (b - a) as f64;
                pts[k] = Some([
                    pa[0] + u * (pb[0] - pa[0]),
                    pa[1] + u * (pb[1] - pa[1]),
                    pa[2] + u * (pb[2] - pa[2]),
                ]);
            }
            i = b + 1;
        }
        let flat: Vec<[f64; 3]> = pts.into_iter().map(|p| p.unwrap()).collect();
        Ok(RefLineData::from_points(&flat))
    }
}
