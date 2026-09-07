//! From a TMR0 reach directory to labelled rows: one row per (record, candidate
//! gate). The label rule (BRIEF-MODEL §Data): gate w reached within h ⇔
//! `gate_tick[w] ≥ 0`; candidates are the gates NOT yet credited at the start
//! (from the ghost's own order in geom/<uid>/human-orders.tsv and the start's
//! `cps_before`), within the plausibility radius `radius_m(h)`, the finish
//! only when it is ARMED (every checkpoint group credited — gen/CONTROL.md).
//!
//! Rows are cached per map as `<uid>.rows` (`TMRW` header, feature version,
//! DIM, n rows, then `[features DIM f32][labels NLAB f32]` per row) because the
//! surface probes cost a scene build per map.

use crate::feat::{Featurizer, TargetSpec};
use crate::features2::TargetKind;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use tmreach::tmr::{read_shard, CarState};
use tmroute::gates::{GatesFile, WpKind};

pub const NLAB: usize = 12;
pub const L_Y: usize = 0; // 1 reached, 0 not
pub const L_TICKS: usize = 1; // gate_tick (positives) else -1
pub const L_BAND: usize = 2; // 1 = the record's end state IS the arrival state (band labels valid)
pub const L_ASPEED: usize = 3; // arrival speed m/s
pub const L_ADY: usize = 4; // arrival height − gate centre y
pub const L_AANG: usize = 5; // angle between arrival velocity and the gate normal, rad
pub const L_DIST: usize = 6; // 3-D distance start → gate centre (the baseline's number)
pub const L_REC: usize = 7; // record index in the shard
pub const L_WP: usize = 8; // map waypoint of the candidate
pub const L_HUMAN: usize = 9; // 1 = human leg record (macro HUMAN_MACRO)
pub const L_START: usize = 10; // start_id
pub const L_H: usize = 11; // horizon ticks

/// Plausibility radius for a candidate: what a car could cover in h ticks at
/// 150 m/s plus slack. Logged per map beside the count of positives it would
/// have excluded (must be 0 — a positive outside it is a label bug).
pub fn radius_m(h: u16) -> f32 {
    (60.0 + 1.5 * h as f32).min(1500.0)
}

/// A record's end state is the ARRIVAL state at gate w when the crossing is
/// within this many ticks of the horizon (human legs: exactly 0).
pub const BAND_SLACK_TICKS: i32 = 5;

#[derive(Clone, Debug)]
pub struct StartInfo {
    pub ghost_md5: String,
    pub race_ms: i32,
    pub state: CarState,
    pub cps_before: u8,
    pub source: String,
}

pub fn read_starts(p: &Path) -> Result<HashMap<u32, StartInfo>, String> {
    let s = std::fs::read_to_string(p).map_err(|e| format!("{}: {e}", p.display()))?;
    let mut out = HashMap::new();
    for (i, line) in s.lines().enumerate() {
        if i == 0 || line.is_empty() {
            continue;
        }
        let f: Vec<&str> = line.split('\t').collect();
        if f.len() < 16 {
            return Err(format!("{}:{}: {} fields", p.display(), i + 1, f.len()));
        }
        let g = |k: usize| f[k].parse::<f32>().map_err(|e| format!("{}:{}: field {k}: {e}", p.display(), i + 1));
        let id: u32 = f[0].parse().map_err(|e| format!("{}:{}: {e}", p.display(), i + 1))?;
        let race_ms: i32 = f[3].parse().unwrap_or(0);
        let cps_before: u8 = f[14].parse().unwrap_or(0);
        let vel = [g(7)?, g(8)?, g(9)?];
        let mut st = CarState {
            race_ms,
            pos: [g(4)?, g(5)?, g(6)?],
            vel,
            quat: [g(10)?, g(11)?, g(12)?, g(13)?],
            ang_vel: [f32::NAN; 3],
            speed: (vel[0] * vel[0] + vel[1] * vel[1] + vel[2] * vel[2]).sqrt(),
            gear: u8::MAX,
            rpm: f32::NAN,
            wheel_contact: [u8::MAX; 4],
            wheel_material: [u8::MAX; 4],
            wheel_slip: [f32::NAN; 4],
            turbo: f32::NAN,
            cps: cps_before,
            finished: false,
        };
        st.cps = cps_before;
        out.insert(id, StartInfo { ghost_md5: f[1].to_string(), race_ms, state: st, cps_before, source: f[15].to_string() });
    }
    Ok(out)
}

/// ghost_md5 → gate order (map waypoints, finish last) from human-orders.tsv
/// (+ the .unverified file). Column `order` is the 4th.
pub fn read_orders(geom_dir: &Path) -> HashMap<String, Vec<u32>> {
    let mut out = HashMap::new();
    for name in ["human-orders.tsv", "human-orders.unverified.tsv"] {
        let Ok(s) = std::fs::read_to_string(geom_dir.join(name)) else { continue };
        for (i, line) in s.lines().enumerate() {
            if i == 0 {
                continue;
            }
            let f: Vec<&str> = line.split('\t').collect();
            if f.len() < 4 {
                continue;
            }
            let order: Vec<u32> = f[3].split(',').filter_map(|x| x.trim().parse().ok()).collect();
            if !order.is_empty() {
                out.entry(f[0].to_string()).or_insert(order);
            }
        }
    }
    out
}

/// The modal order from consensus.txt (`order [g1,g2,..] xN` lines are GROUP
/// ids; we take the first human-orders row instead when there is one). Used
/// only as the fallback when a start's ghost is unknown.
pub fn fallback_order(orders: &HashMap<String, Vec<u32>>) -> Option<Vec<u32>> {
    // most common order
    let mut count: HashMap<&Vec<u32>, usize> = HashMap::new();
    for o in orders.values() {
        *count.entry(o).or_insert(0) += 1;
    }
    count.into_iter().max_by_key(|(_, c)| *c).map(|(o, _)| o.clone())
}

#[derive(Default, Debug, Clone)]
pub struct BuildStats {
    pub records: usize,
    pub rows: usize,
    pub positives: usize,
    pub human_rows: usize,
    pub human_pos: usize,
    pub band_rows: usize,
    pub finish_candidates: usize,
    pub positives_beyond_400: usize,
    pub positives_outside_radius: usize,
    pub credited_reached: usize,
    pub unknown_ghost_starts: usize,
    pub skipped_no_start: usize,
    pub waypoint_ge_32: usize,
    pub candidates_per_record: f64,
}

/// One map's rows in memory.
pub struct Rows {
    pub map_uid: String,
    pub fv: u32,
    pub dim: usize,
    pub x: Vec<f32>,
    pub lab: Vec<f32>,
    pub n: usize,
}

impl Rows {
    pub fn feat(&self, i: usize) -> &[f32] {
        &self.x[i * self.dim..(i + 1) * self.dim]
    }
    pub fn lab(&self, i: usize) -> &[f32] {
        &self.lab[i * NLAB..(i + 1) * NLAB]
    }
    pub fn write(&self, p: &Path) -> Result<(), String> {
        let mut b = Vec::with_capacity(32 + self.x.len() * 4 + self.lab.len() * 4);
        let _ = self.dim;
        b.extend_from_slice(b"TMRW");
        b.extend_from_slice(&self.fv.to_le_bytes());
        b.extend_from_slice(&(self.dim as u32).to_le_bytes());
        b.extend_from_slice(&(NLAB as u32).to_le_bytes());
        b.extend_from_slice(&(self.n as u64).to_le_bytes());
        let uid = self.map_uid.as_bytes();
        b.extend_from_slice(&(uid.len() as u32).to_le_bytes());
        b.extend_from_slice(uid);
        for i in 0..self.n {
            for v in self.feat(i).iter().chain(self.lab(i)) {
                b.extend_from_slice(&v.to_le_bytes());
            }
        }
        std::fs::write(p, b).map_err(|e| format!("{}: {e}", p.display()))
    }
    pub fn read(p: &Path) -> Result<Rows, String> {
        let b = std::fs::read(p).map_err(|e| format!("{}: {e}", p.display()))?;
        if b.len() < 28 || &b[0..4] != b"TMRW" {
            return Err(format!("{}: not a TMRW rows file", p.display()));
        }
        let u32at = |o: usize| u32::from_le_bytes(b[o..o + 4].try_into().unwrap());
        let fv = u32at(4);
        let dim = u32at(8) as usize;
        let nlab = u32at(12) as usize;
        let n = u64::from_le_bytes(b[16..24].try_into().unwrap()) as usize;
        let ul = u32at(24) as usize;
        if !(fv == 1 || fv == 2) || dim != crate::feat::dim_of(fv) || nlab != NLAB {
            return Err(format!("{}: feature version {fv}/dim {dim}/nlab {nlab} — rebuild (code has dim {} for v{fv}, nlab {NLAB})", p.display(), if fv == 1 || fv == 2 { crate::feat::dim_of(fv) } else { 0 }));
        }
        let uid = String::from_utf8_lossy(&b[28..28 + ul]).to_string();
        let body = &b[28 + ul..];
        if body.len() != n * (dim + NLAB) * 4 {
            return Err(format!("{}: body {} bytes, header says {} rows", p.display(), body.len(), n));
        }
        let mut x = Vec::with_capacity(n * dim);
        let mut lab = Vec::with_capacity(n * NLAB);
        for i in 0..n {
            let o = i * (dim + NLAB) * 4;
            for k in 0..dim {
                x.push(f32::from_le_bytes(body[o + 4 * k..o + 4 * k + 4].try_into().unwrap()));
            }
            let o2 = o + dim * 4;
            for k in 0..NLAB {
                lab.push(f32::from_le_bytes(body[o2 + 4 * k..o2 + 4 * k + 4].try_into().unwrap()));
            }
        }
        Ok(Rows { map_uid: uid, fv, dim, x, lab, n })
    }
}

fn dist3(a: [f32; 3], b: [f32; 3]) -> f32 {
    ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2) + (a[2] - b[2]).powi(2)).sqrt()
}

/// Build the rows of one map. `probe` = the map's geometry (or `Probe::none()`
/// for the geometry-free ablation build).
pub fn build_map(reach_dir: &Path, gates: &GatesFile, geom_dir: &Path, feat: &Featurizer, max_rows: usize, threads: usize, log: &mut Vec<String>) -> Result<(Rows, BuildStats), String> {
    let shard = read_shard(&reach_dir.join("samples.tmr"))?;
    let starts = read_starts(&reach_dir.join("starts.tsv"))?;
    let orders = read_orders(geom_dir);
    let fallback = fallback_order(&orders);
    let n_cp_groups = gates.checkpoint_group_ids().len() as u8;
    let group_size: HashMap<u32, u32> = {
        let mut m = HashMap::new();
        for g in &gates.gates {
            *m.entry(g.group).or_insert(0) += 1;
        }
        m
    };
    let mut st = BuildStats { records: shard.records.len(), ..Default::default() };
    let mut specs: Vec<Spec> = Vec::new();
    let mut labs: Vec<f32> = Vec::new();
    let mut cand_total = 0usize;
    let candidates: Vec<&tmroute::gates::GateRec> = gates.gates.iter().filter(|g| g.kind != WpKind::Start).collect();
    for (ri, r) in shard.records.iter().enumerate() {
        let Some(s) = starts.get(&r.start_id) else {
            st.skipped_no_start += 1;
            continue;
        };
        let order = match orders.get(&s.ghost_md5) {
            Some(o) => o.clone(),
            None => {
                st.unknown_ghost_starts += 1;
                fallback.clone().unwrap_or_default()
            }
        };
        let credited: Vec<u32> = order.iter().take(s.cps_before as usize).cloned().collect();
        let armed = s.cps_before >= n_cp_groups;
        let radius = radius_m(r.horizon_ticks);
        let is_human = r.macro_id == tmreach::human::HUMAN_MACRO;
        // groups touched in this record (a crossed group's siblings are neither positive nor negative)
        let mut crossed_groups: Vec<u32> = Vec::new();
        for g in &candidates {
            if (g.waypoint as usize) < 32 && r.gate_tick[g.waypoint as usize] >= 0 {
                crossed_groups.push(g.group);
            }
        }
        for g in &candidates {
            let wp = g.waypoint as usize;
            if wp >= 32 {
                st.waypoint_ge_32 += 1;
                continue;
            }
            let reached = r.gate_tick[wp] >= 0;
            if credited.contains(&g.waypoint) {
                if reached {
                    st.credited_reached += 1;
                }
                continue;
            }
            if g.kind == WpKind::Finish && !armed {
                continue;
            }
            if !reached && crossed_groups.contains(&g.group) {
                continue;
            }
            let d = dist3(s.state.pos, g.centre);
            if reached && d > 400.0 {
                st.positives_beyond_400 += 1;
            }
            if d > radius {
                if reached {
                    st.positives_outside_radius += 1;
                } else {
                    continue;
                }
            }
            cand_total += 1;
            let t = TargetSpec { centre: g.centre, normal: g.normal, half_width: g.half_width, group_size: *group_size.get(&g.group).unwrap_or(&1), kind: TargetKind::of_wp(g.kind), collected_share: s.cps_before as f32 / n_cp_groups.max(1) as f32 };
            specs.push(Spec { state: s.state, target: t, h: r.horizon_ticks });
            let mut lab = [0f32; NLAB];
            lab[L_Y] = if reached { 1.0 } else { 0.0 };
            lab[L_TICKS] = if reached { r.gate_tick[wp] as f32 } else { -1.0 };
            let band_ok = reached && (r.horizon_ticks as i32 - r.gate_tick[wp] as i32).abs() <= BAND_SLACK_TICKS;
            lab[L_BAND] = if band_ok { 1.0 } else { 0.0 };
            if band_ok {
                let e = &r.end;
                let sp = if e.speed.is_finite() { e.speed } else { crate::frame::norm3(e.vel) };
                lab[L_ASPEED] = sp;
                lab[L_ADY] = e.pos[1] - g.centre[1];
                let ang = match crate::frame::unit3(e.vel) {
                    Some(u) => (u[0] * g.normal[0] + u[1] * g.normal[1] + u[2] * g.normal[2]).clamp(-1.0, 1.0).acos(),
                    None => 0.0,
                };
                lab[L_AANG] = ang;
                st.band_rows += 1;
            }
            lab[L_DIST] = d;
            lab[L_REC] = ri as f32;
            lab[L_WP] = g.waypoint as f32;
            lab[L_HUMAN] = if is_human { 1.0 } else { 0.0 };
            lab[L_START] = r.start_id as f32;
            lab[L_H] = r.horizon_ticks as f32;
            labs.extend_from_slice(&lab);
            st.rows += 1;
            if reached {
                st.positives += 1;
            }
            if is_human {
                st.human_rows += 1;
                if reached {
                    st.human_pos += 1;
                }
            }
            if g.kind == WpKind::Finish {
                st.finish_candidates += 1;
            }
        }
    }
    st.candidates_per_record = cand_total as f64 / st.records.max(1) as f64;
    let (rows, dropped) = featurise(&gates.map_uid, &specs, &labs, feat, max_rows, 7, threads);
    if dropped > 0 {
        log.push(format!("  {} [gate]: {} of {} rows kept (uniform, seed 7)", gates.map_uid, rows.n, specs.len()));
    }
    log.push(format!(
        "{} ({}): {} records → {} rows ({:.2} candidates/record), {} positives ({:.1} %), human rows {} ({} pos), band rows {}, finish candidates {}; positives beyond 400 m {}, outside radius(h) {}, already-credited-yet-reached {}, unknown-ghost starts {}, records without a start {}, waypoints ≥ 32 {}",
        gates.map_name, gates.map_uid, st.records, st.rows, st.candidates_per_record, st.positives, 100.0 * st.positives as f64 / st.rows.max(1) as f64,
        st.human_rows, st.human_pos, st.band_rows, st.finish_candidates, st.positives_beyond_400, st.positives_outside_radius, st.credited_reached, st.unknown_ghost_starts, st.skipped_no_start, st.waypoint_ge_32
    ));
    let _ = PathBuf::new();
    Ok((rows, st))
}

/// The player's split rule: fnv1a64(map_uid) % 10 == 0 ⇒ held out.
pub fn fnv1a64(s: &str) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for b in s.bytes() {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}
pub fn held_out(map_uid: &str) -> bool {
    fnv1a64(map_uid) % 10 == 0
}

/// Every `<uid>/samples.tmr` under a reach root (`reach/v0`), or the dir itself when it is a shard dir.
pub fn shard_dirs(root: &Path) -> Vec<PathBuf> {
    if root.join("samples.tmr").exists() {
        return vec![root.to_path_buf()];
    }
    let mut v: Vec<PathBuf> = std::fs::read_dir(root)
        .map(|rd| rd.filter_map(|e| e.ok().map(|e| e.path())).filter(|p| p.join("samples.tmr").exists()).collect())
        .unwrap_or_default();
    v.sort();
    v
}

/// The map uid of a shard dir: the directory name, unless it is a scratch
/// name, in which case the shard's starts' geom is looked up by the detector's
/// provenance — simpler: read it from `FANOUT.log`'s first line (`fanout <hash> on <uid>`).
pub fn shard_map_uid(dir: &Path) -> Option<String> {
    let name = dir.file_name()?.to_string_lossy().to_string();
    if name.len() == 27 && !name.contains('-') {
        return Some(name);
    }
    let log = std::fs::read_to_string(dir.join("FANOUT.log")).ok()?;
    for line in log.lines() {
        if let Some(rest) = line.strip_prefix("fanout ") {
            let mut it = rest.split_whitespace();
            let _hash = it.next()?;
            if it.next()? == "on" {
                return Some(it.next()?.to_string());
            }
        }
    }
    None
}

// ───────────────────────── horizon-native rows (LOCAL targets) ─────────────────────────
//
// Design decision (coordinator, 2026-09-07 05:40Z, from vjeux): the fan-out measures the
// car's 2–4 s REACHABLE SET; labelling one bit per rollout (gate credited or not) threw
// most of it away and asked R to extrapolate 5–20 s legs from 2–4 s rollouts. The
// horizon-native rows use EVERY endpoint: for a (start, h) the positives are the
// endpoint cloud itself (target = the endpoint, reached within R_LOCAL by definition)
// and the negatives are local targets no macro came within R_NEG of, sampled beyond
// and beside the cloud, at random in the reachable disc, and along the chords to the
// uncredited gates. Gate rows stay as the ORDER PRIOR.

/// A local target counts as reached when the car is within this of it at h.
pub const R_LOCAL: f32 = 8.0;
/// A sampled target is a negative only if no endpoint of the (start, h) cloud is within this.
pub const R_NEG: f32 = 14.0;
/// Local targets beyond this from the start are not sampled (the head is LOCAL).
pub const LOCAL_MAX_M: f32 = 450.0;

pub const L_WP_LOCAL: f32 = -1.0;
/// Interior passage points of the straight start → endpoint segment used as positives (f · h ticks).
pub const PASS_FRACS: [f32; 3] = [0.35, 0.6, 0.8];

/// Distance from point p to the segment a→b.
pub fn seg_dist(a: [f32; 3], b: [f32; 3], p: [f32; 3]) -> f32 {
    let ab = [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
    let ap = [p[0] - a[0], p[1] - a[1], p[2] - a[2]];
    let l2 = ab[0] * ab[0] + ab[1] * ab[1] + ab[2] * ab[2];
    let t = if l2 < 1e-6 { 0.0 } else { ((ap[0] * ab[0] + ap[1] * ab[1] + ap[2] * ab[2]) / l2).clamp(0.0, 1.0) };
    let q = [a[0] + ab[0] * t, a[1] + ab[1] * t, a[2] + ab[2] * t];
    dist3(q, p)
}

#[derive(Default, Debug, Clone)]
pub struct LocalStats {
    pub groups: usize,
    pub positives: usize,
    pub negatives: usize,
    pub neg_radial: usize,
    pub neg_lateral: usize,
    pub neg_disc: usize,
    pub neg_chord: usize,
    pub rejected_near_endpoint: usize,
    pub skipped_noop: usize,
    pub skipped_human: usize,
    pub cloud_mean_extent_m: f64,
    pub passage_positives: usize,
}

fn xorshift(s: &mut u64) -> u64 {
    *s ^= *s << 13;
    *s ^= *s >> 7;
    *s ^= *s << 17;
    *s
}
fn unif(s: &mut u64) -> f32 {
    (xorshift(s) >> 11) as f32 / (1u64 << 53) as f32
}

/// Build the horizon-native rows of one map.
pub fn build_local_map(reach_dir: &Path, gates: &GatesFile, geom_dir: &Path, feat: &Featurizer, seed: u64, max_rows: usize, threads: usize, log: &mut Vec<String>) -> Result<(Rows, LocalStats), String> {
    let shard = read_shard(&reach_dir.join("samples.tmr"))?;
    let starts = read_starts(&reach_dir.join("starts.tsv"))?;
    let orders = read_orders(geom_dir);
    let fallback = fallback_order(&orders);
    let mut st = LocalStats::default();
    let n_cp_groups = gates.checkpoint_group_ids().len() as f32;
    let mut rows = (Vec::<Spec>::new(), Vec::<f32>::new());
    let mut rng = seed.max(1) ^ 0x5851_f42d_4c95_7f2d;
    // group records by (start_id, h)
    let mut groups: HashMap<(u32, u16), Vec<usize>> = HashMap::new();
    for (ri, r) in shard.records.iter().enumerate() {
        if r.macro_id >= tmreach::human::RESPAWN_MACRO {
            st.skipped_human += 1;
            continue;
        }
        groups.entry((r.start_id, r.horizon_ticks)).or_default().push(ri);
    }
    let mut keys: Vec<(u32, u16)> = groups.keys().cloned().collect();
    keys.sort();
    let mut extent_sum = 0f64;
    let candidates: Vec<&tmroute::gates::GateRec> = gates.gates.iter().filter(|g| g.kind != WpKind::Start).collect();
    // PASSAGE labels (coordinator 07:23Z): a target is reached if the path passes within R_LOCAL at ANY tick
    // ≤ h, the time label = the tick of first passage. Until GEN's intermediate states land the path is
    // approximated by the STRAIGHT segment start → endpoint: positives at fractions PASS_FRACS of it with
    // ticks ≈ f·h (the endpoint itself at f = 1 carries the real arrival state; interior points carry none);
    // a negative must be > R_NEG from EVERY segment of the cloud, not only from the endpoints.
    let mut push_row = |rows: &mut (Vec<Spec>, Vec<f32>), s: &StartInfo, sid: u32, target: [f32; 3], h: u16, ticks: f32, y: f32, end: Option<&CarState>, key: f32| {
        let d = dist3(s.state.pos, target);
        let dir = crate::frame::unit3([target[0] - s.state.pos[0], target[1] - s.state.pos[1], target[2] - s.state.pos[2]]).unwrap_or([0.0, 0.0, 1.0]);
        let t = TargetSpec { centre: target, normal: dir, half_width: R_LOCAL, group_size: 0, kind: TargetKind::LocalPoint, collected_share: s.cps_before as f32 / n_cp_groups.max(1.0) };
        rows.0.push(Spec { state: s.state, target: t, h });
        let mut lab = [0f32; NLAB];
        lab[L_Y] = y;
        lab[L_TICKS] = if y > 0.5 { ticks } else { -1.0 };
        if let Some(e) = end {
            lab[L_BAND] = 1.0;
            lab[L_ASPEED] = if e.speed.is_finite() { e.speed } else { crate::frame::norm3(e.vel) };
            lab[L_ADY] = 0.0;
            lab[L_AANG] = match crate::frame::unit3(e.vel) {
                Some(u) => (u[0] * dir[0] + u[1] * dir[1] + u[2] * dir[2]).clamp(-1.0, 1.0).acos(),
                None => 0.0,
            };
        }
        lab[L_DIST] = d;
        lab[L_REC] = key;
        lab[L_WP] = L_WP_LOCAL;
        lab[L_HUMAN] = 0.0;
        lab[L_START] = sid as f32;
        lab[L_H] = h as f32;
        rows.1.extend_from_slice(&lab);
    };
    for (gi, key) in keys.iter().enumerate() {
        let recs = &groups[key];
        let Some(s) = starts.get(&key.0) else { continue };
        let h = key.1;
        // endpoint cloud (drop no-op duplicates: endpoints within 0.5 m of an earlier one)
        let mut ends: Vec<&CarState> = Vec::new();
        for &ri in recs {
            let e = &shard.records[ri].end;
            if ends.iter().any(|q| dist3(q.pos, e.pos) < 0.5) {
                st.skipped_noop += 1;
                continue;
            }
            ends.push(e);
        }
        if ends.len() < 4 {
            continue;
        }
        st.groups += 1;
        let sp = s.state.pos;
        let max_d = ends.iter().map(|e| dist3(sp, e.pos)).fold(0f32, f32::max).max(5.0);
        // cloud extent: mean pairwise distance to the cloud centroid
        let mut c = [0f32; 3];
        for e in &ends {
            for a in 0..3 {
                c[a] += e.pos[a] / ends.len() as f32;
            }
        }
        extent_sum += ends.iter().map(|e| dist3(c, e.pos) as f64).sum::<f64>() / ends.len() as f64;
        // positives: the endpoints (real arrival state) and the interior passage points
        for e in &ends {
            push_row(&mut rows, s, key.0, e.pos, h, h as f32, 1.0, Some(e), gi as f32);
            st.positives += 1;
            for f in PASS_FRACS {
                let p = [sp[0] + (e.pos[0] - sp[0]) * f, sp[1] + (e.pos[1] - sp[1]) * f, sp[2] + (e.pos[2] - sp[2]) * f];
                if dist3(sp, p) < R_LOCAL {
                    continue; // the start itself is not a target
                }
                push_row(&mut rows, s, key.0, p, h, (h as f32 * f).max(1.0), 1.0, None, gi as f32);
                st.positives += 1;
                st.passage_positives += 1;
            }
        }
        // negatives: candidates → keep those > R_NEG from every endpoint and ≤ LOCAL_MAX_M from the start
        let mut cands: Vec<([f32; 3], u8)> = Vec::new();
        for e in &ends {
            // (a) radially beyond the cloud
            let u = 1.25 + 0.6 * unif(&mut rng);
            cands.push(([sp[0] + (e.pos[0] - sp[0]) * u, e.pos[1], sp[2] + (e.pos[2] - sp[2]) * u], 0));
            // (b) beside the cloud: perpendicular (in XZ) to the start→endpoint direction
            let dx = e.pos[0] - sp[0];
            let dz = e.pos[2] - sp[2];
            let l = (dx * dx + dz * dz).sqrt().max(1e-3);
            let side = if unif(&mut rng) < 0.5 { -1.0 } else { 1.0 };
            let off = (R_NEG + 2.0 + 30.0 * unif(&mut rng)) * side;
            cands.push(([e.pos[0] - dz / l * off, e.pos[1], e.pos[2] + dx / l * off], 1));
        }
        // (c) random in the disc of radius 1.3 × max_d, at the start's height
        for _ in 0..ends.len() / 2 {
            let r = 1.3 * max_d * unif(&mut rng).sqrt();
            let a = 2.0 * std::f32::consts::PI * unif(&mut rng);
            cands.push(([sp[0] + r * a.cos(), sp[1], sp[2] + r * a.sin()], 2));
        }
        // (d) along the chords to the uncredited gates within reach
        let order = orders.get(&s.ghost_md5).cloned().or_else(|| fallback.clone()).unwrap_or_default();
        let credited: Vec<u32> = order.iter().take(s.cps_before as usize).cloned().collect();
        for g in &candidates {
            if credited.contains(&g.waypoint) {
                continue;
            }
            let dg = dist3(sp, g.centre);
            if dg > 1.5 * max_d + 50.0 {
                continue;
            }
            for k in 1..=4 {
                let t = k as f32 / 4.0 * (LOCAL_MAX_M.min(dg) / dg);
                cands.push(([sp[0] + (g.centre[0] - sp[0]) * t, sp[1] + (g.centre[1] - sp[1]) * t, sp[2] + (g.centre[2] - sp[2]) * t], 3));
            }
        }
        for (p, kind) in cands {
            if dist3(sp, p) > LOCAL_MAX_M {
                continue;
            }
            if ends.iter().any(|e| seg_dist(sp, e.pos, p) < R_NEG) {
                st.rejected_near_endpoint += 1;
                continue;
            }
            push_row(&mut rows, s, key.0, p, h, -1.0, 0.0, None, gi as f32);
            st.negatives += 1;
            match kind {
                0 => st.neg_radial += 1,
                1 => st.neg_lateral += 1,
                2 => st.neg_disc += 1,
                _ => st.neg_chord += 1,
            }
        }
    }
    st.cloud_mean_extent_m = extent_sum / st.groups.max(1) as f64;
    let n_specs = rows.0.len();
    let (rows, dropped) = featurise(&gates.map_uid, &rows.0, &rows.1, feat, max_rows, seed.wrapping_add(7), threads);
    if dropped > 0 {
        log.push(format!("  {} [local]: {} of {} rows kept (uniform, seed 7)", gates.map_uid, rows.n, n_specs));
    }
    log.push(format!(
        "{} ({}) LOCAL: {} (start, h) groups → {} rows: {} positives ({} endpoints + {} straight-segment passage points), {} negatives (radial {}, lateral {}, disc {}, chord {}); {} candidates rejected within {} m of an endpoint; {} no-op duplicate endpoints, {} human records skipped; cloud mean extent {:.1} m; R_LOCAL {} m",
        gates.map_name, gates.map_uid, st.groups, n_specs, st.positives, st.positives - st.passage_positives, st.passage_positives, st.negatives, st.neg_radial, st.neg_lateral, st.neg_disc, st.neg_chord, st.rejected_near_endpoint, R_NEG, st.skipped_noop, st.skipped_human, st.cloud_mean_extent_m, R_LOCAL
    ));
    Ok((rows, st))
}

// ───────────────────────── f16 storage + subsampling ─────────────────────────

pub fn f32_to_f16(x: f32) -> u16 {
    let b = x.to_bits();
    let sign = ((b >> 16) & 0x8000) as u16;
    let exp = ((b >> 23) & 0xff) as i32;
    let mant = b & 0x7f_ffff;
    if exp == 0xff {
        return sign | 0x7c00 | if mant != 0 { 0x200 } else { 0 };
    }
    let e = exp - 127 + 15;
    if e >= 0x1f {
        return sign | 0x7c00; // overflow → inf
    }
    if e <= 0 {
        if e < -10 {
            return sign;
        }
        let m = (mant | 0x80_0000) >> (1 - e);
        // round to nearest even
        let half = (m >> 13) as u16;
        let rem = m & 0x1fff;
        return sign | half + if rem > 0x1000 || (rem == 0x1000 && half & 1 == 1) { 1 } else { 0 };
    }
    let half = ((e as u32) << 10 | mant >> 13) as u16;
    let rem = mant & 0x1fff;
    sign | half + if rem > 0x1000 || (rem == 0x1000 && half & 1 == 1) { 1 } else { 0 }
}

pub fn f16_to_f32(h: u16) -> f32 {
    let sign = ((h & 0x8000) as u32) << 16;
    let exp = ((h >> 10) & 0x1f) as u32;
    let mant = (h & 0x3ff) as u32;
    let bits = if exp == 0 {
        if mant == 0 {
            sign
        } else {
            // subnormal
            let mut e = 127 - 15 + 1;
            let mut m = mant;
            while m & 0x400 == 0 {
                m <<= 1;
                e -= 1;
            }
            sign | ((e as u32) << 23) | ((m & 0x3ff) << 13)
        }
    } else if exp == 0x1f {
        sign | 0x7f80_0000 | (mant << 13)
    } else {
        sign | ((exp + 127 - 15) << 23) | (mant << 13)
    };
    f32::from_bits(bits)
}

impl Rows {
    /// Keep at most `max` rows (uniform, seeded) — the v2 layout is 2,692 f32 wide and a
    /// map's local rows would otherwise be 3.9 GB.
    pub fn subsample(&mut self, max: usize, seed: u64) -> usize {
        if self.n <= max {
            return 0;
        }
        let mut idx: Vec<usize> = (0..self.n).collect();
        let mut s = seed.max(1) ^ 0x9e37_79b9_7f4a_7c15;
        for i in (1..idx.len()).rev() {
            s ^= s << 13;
            s ^= s >> 7;
            s ^= s << 17;
            let j = (s % (i as u64 + 1)) as usize;
            idx.swap(i, j);
        }
        idx.truncate(max);
        idx.sort_unstable();
        let mut x = Vec::with_capacity(max * self.dim);
        let mut lab = Vec::with_capacity(max * NLAB);
        for &i in &idx {
            x.extend_from_slice(self.feat(i));
            lab.extend_from_slice(self.lab(i));
        }
        let dropped = self.n - max;
        self.x = x;
        self.lab = lab;
        self.n = max;
        dropped
    }

    /// Half-precision file (`TMRH`): same header, features as f16, labels as f32.
    pub fn write_half(&self, p: &Path) -> Result<(), String> {
        let mut b = Vec::with_capacity(32 + self.x.len() * 2 + self.lab.len() * 4);
        b.extend_from_slice(b"TMRH");
        b.extend_from_slice(&self.fv.to_le_bytes());
        b.extend_from_slice(&(self.dim as u32).to_le_bytes());
        b.extend_from_slice(&(NLAB as u32).to_le_bytes());
        b.extend_from_slice(&(self.n as u64).to_le_bytes());
        let uid = self.map_uid.as_bytes();
        b.extend_from_slice(&(uid.len() as u32).to_le_bytes());
        b.extend_from_slice(uid);
        for i in 0..self.n {
            for v in self.feat(i) {
                b.extend_from_slice(&f32_to_f16(*v).to_le_bytes());
            }
            for v in self.lab(i) {
                b.extend_from_slice(&v.to_le_bytes());
            }
        }
        std::fs::write(p, b).map_err(|e| format!("{}: {e}", p.display()))
    }

    /// Read either format.
    pub fn read_any(p: &Path) -> Result<Rows, String> {
        let b = std::fs::read(p).map_err(|e| format!("{}: {e}", p.display()))?;
        if b.len() >= 4 && &b[0..4] == b"TMRW" {
            return Rows::read(p);
        }
        if b.len() < 28 || &b[0..4] != b"TMRH" {
            return Err(format!("{}: not a TMRW/TMRH rows file", p.display()));
        }
        let u32at = |o: usize| u32::from_le_bytes(b[o..o + 4].try_into().unwrap());
        let fv = u32at(4);
        let dim = u32at(8) as usize;
        let nlab = u32at(12) as usize;
        let n = u64::from_le_bytes(b[16..24].try_into().unwrap()) as usize;
        let ul = u32at(24) as usize;
        if !(fv == 1 || fv == 2) || dim != crate::feat::dim_of(fv) || nlab != NLAB {
            return Err(format!("{}: feature version {fv}/dim {dim}/nlab {nlab} — rebuild", p.display()));
        }
        let uid = String::from_utf8_lossy(&b[28..28 + ul]).to_string();
        let body = &b[28 + ul..];
        let stride = dim * 2 + NLAB * 4;
        if body.len() != n * stride {
            return Err(format!("{}: body {} bytes, header says {} rows", p.display(), body.len(), n));
        }
        let mut x = Vec::with_capacity(n * dim);
        let mut lab = Vec::with_capacity(n * NLAB);
        for i in 0..n {
            let o = i * stride;
            for k in 0..dim {
                x.push(f16_to_f32(u16::from_le_bytes(body[o + 2 * k..o + 2 * k + 2].try_into().unwrap())));
            }
            let o2 = o + dim * 2;
            for k in 0..NLAB {
                lab.push(f32::from_le_bytes(body[o2 + 4 * k..o2 + 4 * k + 4].try_into().unwrap()));
            }
        }
        Ok(Rows { map_uid: uid, fv, dim, x, lab, n })
    }
}

#[cfg(test)]
mod f16_tests {
    use super::*;
    #[test]
    fn f16_round_trip_is_close() {
        for x in [0.0f32, 1.0, -1.0, 0.5, 0.001, 123.4, -0.3333, 1e-5, 3.0e4] {
            let y = f16_to_f32(f32_to_f16(x));
            let tol = (x.abs() * 1e-3).max(1e-6);
            assert!((x - y).abs() <= tol, "{x} -> {y}");
        }
    }
}

// ───────────────────────── deferred, parallel featurisation ─────────────────────────

/// A row before its features: the start state, the target, the horizon. Labels live beside it.
#[derive(Clone, Debug)]
pub struct Spec {
    pub state: CarState,
    pub target: TargetSpec,
    pub h: u16,
}

/// Featurise `specs` (a uniform seeded subsample of `max_rows` when set) on `threads` threads.
/// Labels are `NLAB` per spec, in the same order.
pub fn featurise(map_uid: &str, specs: &[Spec], lab: &[f32], feat: &Featurizer, max_rows: usize, seed: u64, threads: usize) -> (Rows, usize) {
    let n = specs.len();
    let mut idx: Vec<usize> = (0..n).collect();
    let mut dropped = 0;
    if max_rows > 0 && n > max_rows {
        let mut s = seed.max(1) ^ 0x9e37_79b9_7f4a_7c15;
        for i in (1..idx.len()).rev() {
            s ^= s << 13;
            s ^= s >> 7;
            s ^= s << 17;
            let j = (s % (i as u64 + 1)) as usize;
            idx.swap(i, j);
        }
        idx.truncate(max_rows);
        idx.sort_unstable();
        dropped = n - max_rows;
    }
    let dim = feat.dim();
    let threads = threads.max(1).min(idx.len().max(1));
    let chunk = (idx.len() + threads - 1) / threads.max(1);
    let mut parts: Vec<Vec<f32>> = Vec::with_capacity(threads);
    std::thread::scope(|sc| {
        let handles: Vec<_> = idx
            .chunks(chunk.max(1))
            .map(|ids| {
                sc.spawn(move || {
                    let mut out = Vec::with_capacity(ids.len() * dim);
                    let mut buf = vec![0f32; dim];
                    for &i in ids {
                        let s = &specs[i];
                        feat.fill(&s.state, &s.target, s.h, &mut buf);
                        out.extend_from_slice(&buf);
                    }
                    out
                })
            })
            .collect();
        for h in handles {
            parts.push(h.join().expect("featurise thread"));
        }
    });
    let mut x = Vec::with_capacity(idx.len() * dim);
    for p in parts {
        x.extend(p);
    }
    let mut lab_out = Vec::with_capacity(idx.len() * NLAB);
    for &i in &idx {
        lab_out.extend_from_slice(&lab[i * NLAB..(i + 1) * NLAB]);
    }
    (Rows { map_uid: map_uid.to_string(), fv: feat.version(), dim, x, lab: lab_out, n: idx.len() }, dropped)
}

/// The generator build hash of a shard dir (`fanout <hash> on <uid>` in FANOUT.log), or "?".
pub fn shard_build_hash(dir: &Path) -> String {
    if let Ok(log) = std::fs::read_to_string(dir.join("FANOUT.log")) {
        for line in log.lines() {
            if let Some(rest) = line.strip_prefix("fanout ") {
                if let Some(h) = rest.split_whitespace().next() {
                    return h.to_string();
                }
            }
        }
    }
    "?".into()
}

/// The FRAME control on a shard's starts: forward must be local +Z (mean dot with the velocity
/// direction > 0.9 over rows faster than 5 m/s). A new engine build can change the quaternion
/// convention (coordinator, 10:04Z: `quat` becomes the dyna body quaternion); every shard is
/// checked before its rows are built. Returns (mean dot with +Z, rows used).
pub fn frame_control(reach_dir: &Path) -> Result<(f32, usize), String> {
    let starts = read_starts(&reach_dir.join("starts.tsv"))?;
    let rows: Vec<([f32; 3], [f32; 4])> = starts.values().map(|s| (s.state.vel, s.state.quat)).collect();
    let (acc, n) = crate::frame::alignment(&rows, 5.0);
    Ok((acc[2], n))
}
