//! `tmreach fitbox` — fit each gate's trigger box from oracle-adjudicated
//! rollouts (the cases of `oraclectl --save-rows`): a credited rollout must
//! have at least one row inside the box, a refused one none. Grid search over
//! the front face `s_lo`, the back face `s_hi` and the lateral window
//! `[lat_lo, lat_hi]`; the vertical band is fixed at −6..+8 (untested).
//!
//! Attribution: a case is usable for gate G when G is the only gate the
//! rollout could have newly credited (every other uncredited gate stays
//! > 40 m away) and the oracle's count is not blind (prefix credits + the
//! new one ≥ 2, or a finish).

use crate::gates::{GateKind, MapGates};
use std::path::Path;

pub struct Sample {
    pub credited: bool,
    /// (s, lat, up) of every rollout row within 60 m of the gate centre.
    pub pts: Vec<(f64, f64, f64)>,
    pub name: String,
}

/// Human crossing labels per ghost (from gatecal's crossings.tsv): ghost stem → [(wp, row_ms)].
pub fn load_crossings(p: &Path) -> Result<std::collections::HashMap<String, Vec<(u32, i64)>>, String> {
    let txt = std::fs::read_to_string(p).map_err(|e| format!("{}: {}", p.display(), e))?;
    let mut m: std::collections::HashMap<String, Vec<(u32, i64)>> = Default::default();
    for l in txt.lines().skip(1) {
        let f: Vec<&str> = l.split('\t').collect();
        if f.len() < 5 {
            continue;
        }
        let stem = f[0].trim_end_matches(".Ghost.Gbx").to_string();
        m.entry(stem).or_default().push((f[4].parse().unwrap_or(0), f[3].parse().unwrap_or(0)));
    }
    Ok(m)
}

pub fn load_cases(dir: &Path, gates: &MapGates, gi: usize, crossings: &std::collections::HashMap<String, Vec<(u32, i64)>>, offsets: &std::collections::HashMap<String, (i64, usize)>) -> Result<Vec<Sample>, String> {
    let txt = std::fs::read_to_string(dir.join("cases.tsv")).map_err(|e| e.to_string())?;
    let g = &gates.gates[gi];
    let mut out = Vec::new();
    for line in txt.lines().skip(1) {
        let f: Vec<&str> = line.split('\t').collect();
        if f.len() < 16 {
            continue;
        }
        let ghost = f[0];
        let start_tick: usize = f[1].parse().unwrap_or(0);
        let oracle: Option<u32> = f[4].parse().ok();
        let finished = f[8] != "-";
        let tape = Path::new(f[15]);
        let Ok(rows) = std::fs::read_to_string(tape.with_extension("rows.tsv")) else { continue };
        // the prefix set: gates the human had crossed before the rollout's first record
        let start_label = start_tick as i64 * 10 + offsets.get(ghost).map(|(o, _)| *o).unwrap_or(-1550);
        let prefix: Vec<u32> = crossings.get(ghost).map(|v| v.iter().filter(|(_, ms)| *ms < start_label).map(|(w, _)| *w).collect()).unwrap_or_default();
        if prefix.contains(&g.waypoint) {
            continue;
        }
        // rows more than ~2.5 s after the DECLARED time are not adjudicated (a
        // car that rolled through the finish 2.73 s after its declared time was not
        // credited, finishes up to +2.20 s were; the tape itself ends at the last
        // input EVENT, 1.6 s before the finish on some ghosts). Labels = race − 20.
        let end_label = offsets.get(ghost).map(|(_, decl)| *decl as i64 - 20 + 2500).unwrap_or(i64::MAX);
        let mut pts = Vec::new();
        let mut min_d: Vec<f64> = vec![f64::INFINITY; gates.gates.len()];
        for l in rows.lines().skip(1) {
            let c: Vec<f64> = l.split('\t').take(4).filter_map(|x| x.parse().ok()).collect();
            if c.len() < 4 {
                continue;
            }
            if c[0] as i64 > end_label {
                break;
            }
            let p = [c[1], c[2], c[3]];
            let (s, lat, up) = g.local(p);
            if crate::rig::dist(p, g.centre) < 60.0 {
                pts.push((s, lat, up));
            }
            for (oi, og) in gates.gates.iter().enumerate() {
                min_d[oi] = min_d[oi].min(crate::rig::dist(p, og.centre));
            }
        }
        if pts.is_empty() {
            continue;
        }
        let n_cps = gates.gates.iter().filter(|x| x.kind != GateKind::Finish).count() as u32;
        let credited = if g.kind == GateKind::Finish {
            // the finish is only armed once every checkpoint is credited
            if !finished && oracle.map(|o| o < n_cps).unwrap_or(true) {
                continue;
            }
            finished
        } else {
            let Some(ocps) = oracle else { continue };
            if ocps == 0 && prefix.is_empty() && !finished {
                continue; // blind: 0 or 1
            }
            // candidate gates this rollout could have newly credited
            let cands: Vec<usize> = gates.gates.iter().enumerate().filter(|(oi, og)| !prefix.contains(&og.waypoint) && min_d[*oi] < 40.0).map(|(oi, _)| oi).collect();
            if !cands.contains(&gi) {
                continue;
            }
            let k_new = ocps as i64 - prefix.len() as i64;
            if k_new <= 0 {
                false
            } else if k_new as usize == cands.len() {
                true
            } else {
                continue; // ambiguous attribution
            }
        };
        if std::env::var("TMREACH_FITBOX_DEBUG").is_ok() {
            let mx = pts.iter().map(|p| p.0).fold(f64::NEG_INFINITY, f64::max);
            let ml = pts.iter().filter(|p| p.0 > -17.0).map(|p| p.1).fold(f64::NAN, f64::max);
            eprintln!("  sample wp{} {} credited={} n_pts={} max_s={mx:.2} max_lat_past-17={ml:.2}", g.waypoint, tape.file_name().unwrap().to_string_lossy(), credited, pts.len());
        }
        out.push(Sample { credited, pts, name: tape.file_name().unwrap().to_string_lossy().into_owned() });
    }
    Ok(out)
}
#[derive(Clone, Copy, Debug)]
pub struct Box3 {
    pub s_lo: f64,
    pub s_hi: f64,
    pub lat_lo: f64,
    pub lat_hi: f64,
}

pub fn inside(b: &Box3, p: (f64, f64, f64)) -> bool {
    p.0 >= b.s_lo && p.0 <= b.s_hi && p.1 >= b.lat_lo && p.1 <= b.lat_hi && p.2 >= -6.0 && p.2 <= 8.0
}

/// Inconsistencies of a box: credited samples with no inside point + refused
/// samples with one.
pub fn score(b: &Box3, samples: &[Sample]) -> (usize, usize) {
    let mut miss = 0;
    let mut extra = 0;
    for s in samples {
        let any = s.pts.iter().any(|p| inside(b, *p));
        if s.credited && !any {
            miss += 1;
        }
        if !s.credited && any {
            extra += 1;
        }
    }
    (miss, extra)
}

pub fn grid_fit(samples: &[Sample]) -> Vec<(Box3, usize, usize)> {
    let mut res = Vec::new();
    let mut s_lo = -20.0;
    while s_lo <= 0.0 {
        for s_hi in [2.0, 4.0, 8.0, 12.0, 16.0] {
            let mut lat_hi = 6.0;
            while lat_hi <= 18.0 {
                let mut lat_lo = -18.0;
                while lat_lo <= -6.0 {
                    let b = Box3 { s_lo, s_hi, lat_lo, lat_hi };
                    let (m, e) = score(&b, samples);
                    res.push((b, m, e));
                    lat_lo += 1.0;
                }
                lat_hi += 1.0;
            }
        }
        s_lo += 0.5;
    }
    res.sort_by_key(|(_, m, e)| m + e);
    res
}
