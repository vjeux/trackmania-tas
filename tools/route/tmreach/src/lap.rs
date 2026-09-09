//! `tmreach lap`: a route-guided SAVESTATE SEARCH (Go-Explore) that finds a
//! FINISHING input tape on a map with no human ghost (the tiny maps).
//!
//! Cells: (s along GEOM's road centreline in 4 m bins, speed bucket 5 m/s,
//! checkpoints credited). The archive keeps the best chain per cell (more
//! credits, then further along, then fewer ticks). Expansion: a cell is picked
//! with weight rising steeply with its progress rank and falling with its
//! visits; the search returns to it by REPLAYING its whole input chain from
//! the root (deterministic — the replay end is checked against the archived
//! end) and fans out every macro for `h` ticks: the open-loop library
//! (hold-steer with/without brake, ramps, doublets, slaloms) plus closed-loop
//! FOLLOW macros that steer toward the centreline point 12–45 m ahead in
//! 10-tick chunks (pure pursuit), with gas, with hint braking, or coasting.
//!
//! Credits are the engine counter's (`Row::cps`); the finish is credited when
//! the counter reaches the number of ordered groups (checkpoint groups + the
//! finish). The output is the concatenated input chain from race 0, as the
//! player's gtape text lines and as a `tick steer gas brake` TSV.
use crate::macros::{build, library_v0, Built, Macro};
use crate::rig::{pos, speed, Worker};
use branch;
use forkoracle::forksrv::Rec;
use forkoracle::layout::Row;

/// GEOM's road centreline (tm-route/tiny/<build>/centreline/*.road-centreline.json).
pub struct Track {
    pub pts: Vec<[f64; 3]>,
    pub s: Vec<f64>,
    pub half_width: Vec<f64>,
    pub speed_hint: Vec<f64>,
    /// number of ordered groups (checkpoint groups + finish): the finish is credited when cps reaches it
    pub n_groups: usize,
    /// per polyline segment i (pts[i] -> pts[i+1]): part of a GAP leg (free exploration)
    pub gap_seg: Vec<bool>,
    /// arc length of the end of each ordered leg (the leg's gate), in credit order
    pub gate_s: Vec<f64>,
    /// per ordered leg: a GAP leg (jump/drop/bowl: free exploration)
    pub leg_gap: Vec<bool>,
    /// the ordered group ids (deck/gates.json groups) — credits are matched to gates and to this order
    pub order_groups: Vec<u32>,
    /// the human's speed at each point (author line only: 100 ms samples), empty otherwise
    pub human_speed: Vec<f64>,
}

impl Track {
    pub fn load(path: &std::path::Path) -> Result<Track, String> {
        let txt = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
        let j = crate::json::parse(&txt)?;
        let pts: Vec<[f64; 3]> = j.get("pts").and_then(|v| v.arr()).ok_or("pts")?.iter().filter_map(|p| p.vec3()).collect();
        let s: Vec<f64> = j.get("s").and_then(|v| v.arr()).ok_or("s")?.iter().filter_map(|x| x.f64()).collect();
        let half_width: Vec<f64> = j.get("half_width").and_then(|v| v.arr()).map(|a| a.iter().filter_map(|x| x.f64()).collect()).unwrap_or_else(|| vec![5.5; pts.len()]);
        let speed_hint: Vec<f64> = j.get("speed_hint").and_then(|v| v.arr()).map(|a| a.iter().filter_map(|x| x.f64()).collect()).unwrap_or_else(|| vec![80.0; pts.len()]);
        let order_groups: Vec<u32> = j.get("order_groups").and_then(|v| v.arr()).ok_or("order_groups")?.iter().filter_map(|x| x.f64()).map(|x| x as u32).collect();
        let n_groups = order_groups.len();
        if pts.len() != s.len() || pts.len() < 2 {
            return Err(format!("centreline: {} pts, {} s values", pts.len(), s.len()));
        }
        let mut gap_seg = vec![false; pts.len() - 1];
        let mut gate_s: Vec<f64> = Vec::new();
        let mut leg_gap: Vec<bool> = Vec::new();
        if let Some(segs) = j.get("segments").and_then(|v| v.arr()) {
            // a gap leg from pts[i1 of the previous] to pts[i0 of this]: the segments list gives i0/i1 per
            // on-road leg; the polyline segments between consecutive legs are the gaps
            let mut prev_end: Option<usize> = None;
            for sg in segs {
                let gap = sg.get("gap").map(|g| matches!(g, crate::json::Json::Bool(true))).unwrap_or(false);
                let i0 = sg.get("i0").and_then(|v| v.f64()).map(|x| x as usize);
                let i1 = sg.get("i1").and_then(|v| v.f64()).map(|x| x as usize);
                if let (Some(a), Some(b)) = (i0, i1) {
                    // a gap leg has no on-road points (i0 == i1): its gate is `to_pos`, the nearest polyline point
                    let gs = match sg.get("to_pos").and_then(|v| v.vec3()) {
                        Some(tp) if gap || a == b => {
                            let mut best = (f64::INFINITY, 0.0);
                            for (i, p) in pts.iter().enumerate() {
                                let d = ((p[0] - tp[0]).powi(2) + (p[1] - tp[1]).powi(2) + (p[2] - tp[2]).powi(2)).sqrt();
                                if d < best.0 {
                                    best = (d, s[i]);
                                }
                            }
                            best.1
                        }
                        _ => s[b.min(s.len() - 1)],
                    };
                    gate_s.push(gs);
                    leg_gap.push(gap);
                    if gap {
                        for k in a..b.min(gap_seg.len()) {
                            gap_seg[k] = true;
                        }
                    }
                    if let Some(pe) = prev_end {
                        for k in pe..a.min(gap_seg.len()) {
                            gap_seg[k] = true;
                        }
                    }
                    prev_end = Some(b);
                }
            }
        }
        if gate_s.len() != n_groups {
            eprintln!("centreline: {} segments but {} ordered groups; progress capped by the segments", gate_s.len(), n_groups);
        }
        Ok(Track { pts, s, half_width, speed_hint, n_groups, gap_seg, gate_s, leg_gap, order_groups, human_speed: Vec::new() })
    }

    /// Replace the FIRST leg (spawn -> first gate) by an explicit waypoint polyline (x y z per line):
    /// GEOM's rayed human line where the centreline's gap leg is a straight line through the air.
    /// The leg becomes a road leg with a wide tolerance; every later arc length shifts by the new length.
    pub fn replace_first_leg(&mut self, wps: &[[f64; 3]]) {
        self.replace_leg(0, wps)
    }

    /// Replace ordered leg `k` (from gate k-1, or the spawn, to gate k) by an explicit waypoint polyline.
    pub fn replace_leg(&mut self, k: usize, wps: &[[f64; 3]]) {
        if wps.len() < 2 || k >= self.gate_s.len() {
            return;
        }
        let idx_of = |t: &Track, s_target: f64| -> usize {
            let mut best = (f64::INFINITY, 0usize);
            for (i, x) in t.s.iter().enumerate() {
                if (*x - s_target).abs() < best.0 {
                    best = ((*x - s_target).abs(), i);
                }
            }
            best.1
        };
        let g_idx = idx_of(self, self.gate_s[k]);
        let p_idx = if k == 0 { 0 } else { idx_of(self, self.gate_s[k - 1]) };
        let mut pts: Vec<[f64; 3]> = self.pts[..p_idx].to_vec();
        let removed_len = self.s[g_idx] - self.s[p_idx];
        pts.extend_from_slice(wps);
        pts.extend_from_slice(&self.pts[g_idx..]);
        let mut s = vec![0.0; pts.len()];
        for i in 1..pts.len() {
            let (a, b) = (pts[i - 1], pts[i]);
            s[i] = s[i - 1] + ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2) + (a[2] - b[2]).powi(2)).sqrt();
        }
        let new_gate_k = s[p_idx + wps.len() - 1];
        let shift = new_gate_k - self.gate_s[k];
        let _ = removed_len;
        let hw0 = 8.0;
        let mut half_width: Vec<f64> = self.half_width[..p_idx.min(self.half_width.len())].to_vec();
        half_width.extend(std::iter::repeat(hw0).take(wps.len()));
        half_width.extend_from_slice(&self.half_width[g_idx.min(self.half_width.len())..]);
        let mut speed_hint: Vec<f64> = self.speed_hint[..p_idx.min(self.speed_hint.len())].to_vec();
        speed_hint.extend(std::iter::repeat(80.0).take(wps.len()));
        speed_hint.extend_from_slice(&self.speed_hint[g_idx.min(self.speed_hint.len())..]);
        let mut gap_seg: Vec<bool> = self.gap_seg[..p_idx.min(self.gap_seg.len())].to_vec();
        gap_seg.extend(std::iter::repeat(false).take(wps.len()));
        gap_seg.extend_from_slice(&self.gap_seg[g_idx.min(self.gap_seg.len())..]);
        gap_seg.truncate(pts.len() - 1);
        half_width.truncate(pts.len());
        speed_hint.truncate(pts.len());
        for (i, g) in self.gate_s.iter_mut().enumerate() {
            if i >= k {
                *g += shift;
            }
        }
        self.gate_s[k] = new_gate_k;
        if let Some(l) = self.leg_gap.get_mut(k) {
            *l = false;
        }
        self.pts = pts;
        self.s = s;
        // a replaced leg has no human speed profile: drop it (the clinic then skips the speed band there)
        if !self.human_speed.is_empty() {
            self.human_speed = vec![0.0; self.pts.len()];
        }
        self.half_width = half_width;
        self.speed_hint = speed_hint;
        self.gap_seg = gap_seg;
    }

    /// A track from GEOM's AUTHOR-LINE (the original author's validation ghost in the tiny frame, 100 ms
    /// samples) and the map's gates: s along the author's line, gates ordered by where the line passes
    /// them (the author's order), every leg a road leg (the author drove it).
    pub fn from_author_line(path: &std::path::Path, gates: &crate::gates::MapGates) -> Result<Track, String> {
        Self::from_author_line_ordered(path, gates, None)
    }

    /// Same, with an explicit gate ORDER (GEOM's human order from the centreline file): each gate is placed
    /// at the line's first pass within 12 m AFTER the previous gate's arc length (a line that runs under
    /// or beside a gate earlier must not pull it forward -- 21's deck gate 1 sits 16 m above leg 1).
    pub fn from_author_line_ordered(path: &std::path::Path, gates: &crate::gates::MapGates, order: Option<&[u32]>) -> Result<Track, String> {
        let txt = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
        let j = crate::json::parse(&txt)?;
        let pts: Vec<[f64; 3]> = j.get("pts").and_then(|v| v.arr()).ok_or("pts")?.iter().filter_map(|p| p.vec3()).collect();
        if pts.len() < 2 {
            return Err("author line: fewer than 2 points".into());
        }
        let mut s = vec![0.0; pts.len()];
        for i in 1..pts.len() {
            s[i] = s[i - 1] + ((pts[i][0] - pts[i - 1][0]).powi(2) + (pts[i][1] - pts[i - 1][1]).powi(2) + (pts[i][2] - pts[i - 1][2]).powi(2)).sqrt();
        }
        let n = pts.len();
        // 100 ms samples: the human's speed is the sample spacing x 10 (smoothed over 5 samples)
        let mut human_speed = vec![0.0; n];
        for i in 0..n {
            let a = i.saturating_sub(2);
            let b = (i + 2).min(n - 1);
            if b > a {
                human_speed[i] = (s[b] - s[a]) / (0.1 * (b - a) as f64);
            }
        }
        let mut t = Track { pts, s, half_width: vec![6.0; n], speed_hint: vec![80.0; n], n_groups: 0, gap_seg: vec![false; n - 1], gate_s: Vec::new(), leg_gap: Vec::new(), order_groups: Vec::new(), human_speed };
        if let Some(ord) = order {
            let mut gate_s = Vec::new();
            let mut s_from = 0.0f64;
            for grp in ord {
                // the group's gates; the first pass of the line within 12 m after s_from, else the nearest after s_from
                let mut best: Option<(f64, f64)> = None; // (s, d)
                for g in gates.gates.iter().filter(|g| g.group == *grp && g.kind != crate::gates::GateKind::Start) {
                    for i in 0..n {
                        if t.s[i] < s_from {
                            continue;
                        }
                        let p = t.pts[i];
                        let d = ((p[0] - g.centre[0]).powi(2) + (p[1] - g.centre[1]).powi(2) + (p[2] - g.centre[2]).powi(2)).sqrt();
                        if d < 20.0 {
                            if best.map(|b| t.s[i] < b.0).unwrap_or(true) {
                                best = Some((t.s[i], d));
                            }
                            break;
                        }
                        if best.is_none() || (best.unwrap().1 >= 20.0 && d < best.unwrap().1) {
                            best = Some((t.s[i], d));
                        }
                    }
                }
                let (gs, d) = best.unwrap_or((s_from, f64::INFINITY));
                if d > 20.0 {
                    eprintln!("author line: group {grp} is {d:.1} m from the line after s {s_from:.0}; placed at s {gs:.0}");
                }
                gate_s.push(gs);
                s_from = gs;
            }
            t.gate_s = gate_s;
            t.order_groups = ord.to_vec();
            t.n_groups = ord.len();
            t.leg_gap = vec![false; t.n_groups];
            return Ok(t);
        }
        // each gate group: the arc length where the line passes nearest its (first) gate
        let mut groups: Vec<(f64, u32)> = Vec::new();
        for g in &gates.gates {
            if g.kind == crate::gates::GateKind::Start {
                continue;
            }
            let (gs, _, _, d) = t.project(g.centre, n / 2, n);
            if d > 40.0 {
                eprintln!("author line: gate wp{} (group {}) is {d:.1} m from the line; ordered by nearest s anyway", g.waypoint, g.group);
            }
            match groups.iter_mut().find(|(_, grp)| *grp == g.group) {
                Some(e) => e.0 = e.0.min(gs),
                None => groups.push((gs, g.group)),
            }
        }
        groups.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());
        t.gate_s = groups.iter().map(|x| x.0).collect();
        t.order_groups = groups.iter().map(|x| x.1).collect();
        t.n_groups = groups.len();
        t.leg_gap = vec![false; t.n_groups];
        Ok(t)
    }

    /// the human's speed at arc length s (0 when unknown)
    pub fn human_speed_at(&self, s: f64) -> f64 {
        if self.human_speed.is_empty() {
            return 0.0;
        }
        let i = match self.s.binary_search_by(|x| x.partial_cmp(&s).unwrap()) {
            Ok(i) => i,
            Err(i) => i.min(self.s.len() - 1),
        };
        self.human_speed[i]
    }

    /// the lowest line height within `w` m of arc length around s
    pub fn min_y_near(&self, s: f64, w: f64) -> f64 {
        let i = match self.s.binary_search_by(|x| x.partial_cmp(&s).unwrap()) {
            Ok(i) => i,
            Err(i) => i.min(self.s.len() - 1),
        };
        let mut m = self.pts[i][1];
        let mut j = i;
        while j > 0 && self.s[i] - self.s[j - 1] <= w {
            j -= 1;
            m = m.min(self.pts[j][1]);
        }
        let mut j = i;
        while j + 1 < self.s.len() && self.s[j + 1] - self.s[i] <= w {
            j += 1;
            m = m.min(self.pts[j][1]);
        }
        m
    }

    pub fn len_m(&self) -> f64 {
        *self.s.last().unwrap()
    }

    /// Project `p` onto the polyline near segment index `hint` (a window of segments, so a
    /// road that comes back beside itself does not steal the projection); returns
    /// (s, lateral distance, segment index, 3-D distance to the polyline).
    pub fn project(&self, p: [f64; 3], hint: usize, window: usize) -> (f64, f64, usize, f64) {
        let n = self.pts.len() - 1;
        let lo = hint.saturating_sub(window);
        let hi = (hint + window).min(n - 1);
        // a spur the route drives in and back out (08: the linked gate at the end of a 14 m dead end) has the
        // same points on both legs: among near-equal distances (within 1 m) the LARGER arc length wins,
        // so progress is monotone along the route
        let mut cands: Vec<(f64, f64, usize, f64)> = Vec::new();
        let mut best = (f64::INFINITY, 0.0, 0usize, 0.0f64);
        for i in lo..=hi {
            let (a, b) = (self.pts[i], self.pts[i + 1]);
            let ab = [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
            let ap = [p[0] - a[0], p[1] - a[1], p[2] - a[2]];
            let l2 = ab[0] * ab[0] + ab[1] * ab[1] + ab[2] * ab[2];
            let t = if l2 > 1e-9 { ((ap[0] * ab[0] + ap[1] * ab[1] + ap[2] * ab[2]) / l2).clamp(0.0, 1.0) } else { 0.0 };
            let q = [a[0] + t * ab[0], a[1] + t * ab[1], a[2] + t * ab[2]];
            let d = ((p[0] - q[0]).powi(2) + (p[1] - q[1]).powi(2) + (p[2] - q[2]).powi(2)).sqrt();
            // horizontal lateral distance (the road frame is mostly horizontal)
            // signed: positive on the right of the direction of travel (xz cross product)
            let cross = ab[0] * (p[2] - q[2]) - ab[2] * (p[0] - q[0]);
            let dh = ((p[0] - q[0]).powi(2) + (p[2] - q[2]).powi(2)).sqrt() * if cross < 0.0 { -1.0 } else { 1.0 };
            let cand = (d, self.s[i] + t * (self.s[i + 1] - self.s[i]), i, dh);
            if d < best.0 {
                best = cand;
            }
            cands.push(cand);
        }
        let dmin = best.0;
        for c in cands {
            if c.0 <= dmin + 1.0 && c.1 > best.1 {
                best = c;
            }
        }
        (best.1, best.3, best.2, best.0)
    }

    /// The point at arc length `s` (clamped).
    pub fn at(&self, s: f64) -> [f64; 3] {
        let s = s.clamp(0.0, self.len_m());
        let i = match self.s.binary_search_by(|x| x.partial_cmp(&s).unwrap()) {
            Ok(i) => i.min(self.pts.len() - 2),
            Err(i) => i.saturating_sub(1).min(self.pts.len() - 2),
        };
        let (a, b) = (self.pts[i], self.pts[i + 1]);
        let ds = self.s[i + 1] - self.s[i];
        let t = if ds > 1e-9 { ((s - self.s[i]) / ds).clamp(0.0, 1.0) } else { 0.0 };
        [a[0] + t * (b[0] - a[0]), a[1] + t * (b[1] - a[1]), a[2] + t * (b[2] - a[2])]
    }

    pub fn hint_at(&self, seg: usize) -> f64 {
        self.speed_hint.get(seg).copied().unwrap_or(80.0)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Key {
    pub cs: i32,
    pub cv: i32,
    /// 3 m height bucket: an airborne state over a road is not the state on it (21's gate ring sits above the road)
    pub cy: i32,
    /// the credited GROUPS (bit = position in the order; bit 31 = a credit no gate explains)
    pub mask: u32,
}

#[derive(Clone)]
pub struct Entry {
    pub key: Key,
    /// every input record from the root probe tick (the chain is the tape)
    pub chain: Vec<Rec>,
    pub cps: u8,
    /// credited groups by order position (bit 31: unexplained)
    pub mask: u32,
    pub s: f64,
    pub seg: usize,
    pub progress: f64,
    pub visits: u32,
    pub end: Row,
    pub macro_desc: Vec<String>,
}

pub struct LapCfg {
    pub track: Track,
    /// the map's gates (GEOM gates.json) for credit attribution; None = count only
    pub gates: Option<crate::gates::MapGates>,
    pub h: usize,
    pub budget: usize,
    pub seed: u64,
    pub steer_sign: f64,
    pub out: std::path::PathBuf,
    pub max_chain_ticks: usize,
    pub verbose: bool,
    /// Seed the search from the base tape's OWN inputs replayed for this many ticks (a savestate
    /// inside a known run, e.g. 50 m before a jump), instead of from the root only.
    pub prefix_ticks: usize,
    /// Seed from an explicit chain (a previous run's best.tsv) instead of the base tape's inputs.
    pub seed_chain: Option<Vec<Rec>>,
    /// keep the seed only through its K-th credit (+0.3 s); 0 = whole
    pub seed_to_gate: usize,
    /// lateral tolerance beyond the half width on road legs (m): 6 on roads, 25+ on open terrain
    pub lat_tol: f64,
    /// how far below the line a car may be while laterally on it (m): 25 = dips allowed (08), 4 = the
    /// line is an elevated ledge/rim and the floor under it is a dead end (15's pool)
    pub below_tol: f64,
    /// CLINIC mode: the objective is the NEXT gate only — stop when it is credited with a good arrival
    /// (laterally on the line, speed within 30 % of the human's there) and hand the chain on
    pub clinic: bool,
    /// Policy proposals (MODEL arm): the per-map tmrl policy rolled forward in closed loop as extra macros.
    pub policy: Option<crate::policy_src::PolicySrc>,
}

struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.0 = x;
        x.wrapping_mul(0x2545F4914F6CDD1D)
    }
    fn unit(&mut self) -> f64 {
        (self.next() >> 11) as f64 / (1u64 << 53) as f64
    }
}

/// Closed-loop follow macros (ids 1000+): (description, gas, brake mode, lookahead scale)
/// brake mode: 0 none, 1 brake when faster than the hint + margin, 2 coast (no gas)
const FOLLOW: &[(&str, bool, u8, f64)] = &[
    ("follow centreline, gas", true, 0, 0.9),
    ("follow centreline, gas, hint braking", true, 1, 0.9),
    ("follow centreline, coast", false, 2, 0.9),
    ("follow centreline (short lookahead), gas", true, 0, 0.5),
    ("follow centreline (long lookahead), gas", true, 0, 1.5),
    // bmode 3: pedals like the human (author line): gas below the human's speed 15 m ahead, coast above, brake well above
    ("follow the human (line + pedals)", true, 3, 0.9),
    ("follow the human (line + pedals), long lookahead", true, 3, 1.5),
];

pub fn yaw_of(r: &Row) -> f64 {
    let v = speed(r);
    let (fx, fz) = if v > 2.0 { (r.vx, r.vz) } else { let f = crate::gatecal::rotate(r, [0.0, 0.0, 1.0]); (f[0], f[2]) };
    fx.atan2(fz)
}

pub fn wrap(a: f64) -> f64 {
    let mut a = a;
    while a > std::f64::consts::PI {
        a -= 2.0 * std::f64::consts::PI;
    }
    while a < -std::f64::consts::PI {
        a += 2.0 * std::f64::consts::PI;
    }
    a
}

/// Pure-pursuit steer toward the centreline point `look` metres ahead of the car's projection.
fn follow_steer(cfg: &LapCfg, r: &Row, seg_hint: usize, look_scale: f64) -> (f32, usize, f64) {
    let (s, _lat, seg, _d) = cfg.track.project(pos(r), seg_hint, 60);
    let v = speed(r);
    let look = (look_scale * v).clamp(12.0, 45.0);
    let target = cfg.track.at(s + look);
    let yaw = yaw_of(r);
    let want = (target[0] - r.x).atan2(target[2] - r.z);
    let delta = wrap(want - yaw);
    // full lock at 25 degrees of heading error
    // quantized to the tape's i8 steer grid (-127..127), so the exported tape replays the search exactly
    let steer = ((cfg.steer_sign * delta / 25f64.to_radians()).clamp(-1.0, 1.0) * 127.0).round() as f32 / 127.0;
    (steer, seg, s)
}

pub struct LapOut {
    pub finished: Option<Entry>,
    /// clinic mode: the leg (gate index) that was completed
    pub leg_done: Option<usize>,
    pub rollouts: usize,
    pub steps: usize,
    pub cells: usize,
    pub best: Option<Entry>,
    pub log: Vec<String>,
    /// rollout end-state census: [off-world, off-route (lateral/d3), fell below the line, stopped-no-credit, alive-but-crawling (<3 m/s), alive]
    pub deaths: [usize; 6],
}

pub fn run(w: &mut Worker, cfg: &LapCfg) -> Result<LapOut, String> {
    let n = w.n_ticks();
    let root = w.root_probe;
    let macros: Vec<Macro> = library_v0();
    let h = cfg.h;
    let mut rng = Rng(cfg.seed ^ 0x9E3779B97F4A7C15);
    let mut archive: std::collections::HashMap<Key, Entry> = Default::default();
    let mut out = LapOut { finished: None, leg_done: None, rollouts: 0, steps: 0, cells: 0, best: None, log: Vec::new(), deaths: [0; 6] };
    let mut near_misses: usize = 0;
    // clinic: the leg index the seed sits on (credits in order at the seed)
    let seed_k = std::cell::Cell::new(0usize);
    let track = &cfg.track;
    let n_groups = track.n_groups as u8;
    let root_row = w.root_row.clone();
    let (s0, _, seg0, d0) = track.project(pos(&root_row), 0, 30);
    out.log.push(format!("root at ({:.1}, {:.1}, {:.1}): centreline s {:.1} m (segment {seg0}, {:.1} m off), track {:.0} m, {} groups at s {:?}, h {h}, steer sign {:+}", root_row.x, root_row.y, root_row.z, s0, d0, track.len_m(), n_groups, track.gate_s.iter().map(|x| format!("{x:.0}")).collect::<Vec<_>>(), cfg.steer_sign));
    let cps_of = |r: &Row| -> u8 { if r.cps == u32::MAX { 0 } else { r.cps as u8 } };
    let debug = std::env::var("TMREACH_LAP_DEBUG").is_ok();
    let debug_fan = std::env::var("TMREACH_LAP_DEBUG_FAN").is_ok();
    let root_cps = cps_of(&root_row);
    let track_min_y = track.pts.iter().map(|p| p[1]).fold(f64::INFINITY, f64::min);

    // one rollout of `recs` from `node` (state after tick `from`-1); returns rows and whether the run ended
    // fan-out from a node: every macro once
    let mut fan = |w: &mut Worker, node: branch::Handle, from: usize, base: Option<&Entry>, archive: &mut std::collections::HashMap<Key, Entry>, out: &mut LapOut, rng: &mut Rng, h: usize| -> Result<usize, String> {
        if from + h > n {
            return Ok(0);
        }
        let chain0: Vec<Rec> = base.map(|e| e.chain.clone()).unwrap_or_default();
        let seg_hint = base.map(|e| e.seg).unwrap_or(seg0);
        let base_recs: Vec<(u8, u8, u8)> = (from..from + h).map(|t| (w.tape.steer[t.min(n - 1)], w.tape.accel[t.min(n - 1)], w.tape.brake[t.min(n - 1)])).collect();
        let mut count = 0;
        // (recs, rows, exited, description)
        let mut results: Vec<(Vec<Rec>, Vec<Row>, bool, String)> = Vec::new();
        for m in &macros {
            let recs = match build(m, &base_recs, false) {
                Built::Recs(r) => r,
                Built::NoOp => continue,
            };
            match w.rollout(node, &recs, from, h as u64) {
                Ok(r) => {
                    count += 1;
                    results.push((recs, r.rows, r.exited, m.description.clone()));
                }
                Err(e) => out.log.push(format!("  rollout failed (macro {}): {e}", m.id)),
            }
        }
        // closed-loop follow macros: 10-tick chunks, steer recomputed from the last row
        for (desc, gas, bmode, look) in FOLLOW {
            let mut cur = node;
            let mut recs: Vec<Rec> = Vec::with_capacity(h);
            let mut rows: Vec<Row> = Vec::new();
            let mut last = base.map(|e| e.end.clone()).unwrap_or_else(|| root_row.clone());
            let mut seg = seg_hint;
            let mut exited = false;
            let mut done = 0usize;
            while done < h {
                let k = 10.min(h - done);
                let (st, sg, s_now) = follow_steer(cfg, &last, seg, *look);
                seg = sg;
                let v = speed(&last);
                let hint = track.hint_at(seg);
                let (g, b) = match bmode {
                    1 => if v > hint + 3.0 && s_now > 10.0 { (false, true) } else { (*gas, false) },
                    3 => {
                        let vh = track.human_speed_at(s_now + 15.0);
                        if vh <= 3.0 || v <= vh * 1.05 { (true, false) } else if v > vh * 1.3 { (false, true) } else { (false, false) }
                    }
                    2 => (false, false),
                    _ => (*gas, false),
                };
                let chunk: Vec<Rec> = (0..k).map(|_| Rec { steer: st, gas: g as u8 as f32, brake: b as u8 as f32 }).collect();
                match w.rollout_keep(cur, &chunk, from + done, k as u64) {
                    Ok((rr, nh)) => {
                        if cur != node {
                            w.release(cur);
                        }
                        cur = nh;
                        recs.extend(chunk);
                        if let Some(l) = rr.last() {
                            last = l.clone();
                        }
                        rows.extend(rr);
                        done += k;
                        if rows.last().map(|r| r.y < -50.0).unwrap_or(false) {
                            break;
                        }
                    }
                    Err(e) => {
                        // the run ended (or the child died): keep what we have
                        if !e.contains("ended") {
                            out.log.push(format!("  follow chunk failed: {e}"));
                        }
                        exited = true;
                        break;
                    }
                }
            }
            if cur != node {
                w.release(cur);
            }
            if !rows.is_empty() {
                count += 1;
                results.push((recs, rows, exited, desc.to_string()));
            }
        }
        // POLICY proposals (MODEL arm): the per-map tmrl policy in closed loop, k-tick chunks, N independent
        // samples at temperature T (T = 0 → one deterministic proposal)
        if let Some(ps) = &cfg.policy {
            let n_prop = if ps.temp > 0.0 { ps.n } else { 1 };
            for pi in 0..n_prop {
                let mut cur = node;
                let mut recs: Vec<Rec> = Vec::with_capacity(h);
                let mut rows: Vec<Row> = Vec::new();
                let mut last = base.map(|e| e.end.clone()).unwrap_or_else(|| root_row.clone());
                // the actions the base chain ended with (the observation carries the last 5)
                let mut prev: Vec<tmstate::Action> = chain0.iter().rev().take(tmobs::N_PREV).rev().map(|r| tmstate::Action { steer: (r.steer * 127.0).round().clamp(-127.0, 127.0) as i8, gas: r.gas > 0.5, brake: r.brake > 0.5 }).collect();
                let mut exited = false;
                let mut done = 0usize;
                let mut unit = || rng.unit() as f32;
                while done < h {
                    let k = ps.k().min(h - done);
                    let (chunk_full, acts) = ps.propose(&last, w.race_of(&last), cps_of(&last), &prev, &mut unit);
                    let chunk: Vec<Rec> = chunk_full.into_iter().take(k).collect();
                    crate::policy_src::push_prev(&mut prev, &acts[..k.min(acts.len())]);
                    match w.rollout_keep(cur, &chunk, from + done, k as u64) {
                        Ok((rr, nh)) => {
                            if cur != node {
                                w.release(cur);
                            }
                            cur = nh;
                            recs.extend(chunk);
                            if let Some(l) = rr.last() {
                                last = l.clone();
                            }
                            rows.extend(rr);
                            done += k;
                            if rows.last().map(|r| r.y < -50.0).unwrap_or(false) {
                                break;
                            }
                        }
                        Err(e) => {
                            if !e.contains("ended") {
                                out.log.push(format!("  policy chunk failed: {e}"));
                            }
                            exited = true;
                            break;
                        }
                    }
                }
                if cur != node {
                    w.release(cur);
                }
                if debug {
                    let e = rows.last();
                    eprintln!("    policy proposal #{pi}: {} rows, {} ticks of inputs, end {:?} speed {:.1}, exited {exited}", rows.len(), recs.len(), e.map(|r| (r.x as i32, r.y as i32, r.z as i32)), e.map(speed).unwrap_or(0.0));
                }
                if !rows.is_empty() {
                    count += 1;
                    results.push((recs, rows, exited, format!("POLICY {} T{:.2} #{pi}", ps.label, ps.temp)));
                }
            }
        }
        for (recs, rows, exited, desc) in results {
            let end = match rows.last() {
                Some(r) => r.clone(),
                None => continue,
            };
            let _ = exited;
            let cps = cps_of(&end);
            // WHICH gates: every counter step in these rows is attributed to the nearest gate centre
            // (within 30 m) -> its group -> its position in the order; an unexplained step sets bit 31
            let mut mask = base.map(|e| e.mask).unwrap_or(0);
            let mut prev_cps = base.map(|e| cps_of(&e.end)).unwrap_or(root_cps);
            for r in &rows {
                let c = cps_of(r);
                if c > prev_cps {
                    let mut bit = 31u32;
                    if let Some(g) = &cfg.gates {
                        let mut best = (30.0f64, None);
                        for gg in &g.gates {
                            if gg.kind == crate::gates::GateKind::Start {
                                continue;
                            }
                            let d = crate::rig::dist(pos(r), gg.centre);
                            if d < best.0 {
                                best = (d, Some(gg.group));
                            }
                        }
                        if let Some(grp) = best.1 {
                            if let Some(pos_in_order) = track.order_groups.iter().position(|x| *x == grp) {
                                bit = pos_in_order as u32;
                            }
                        }
                        if debug {
                            let nearest = g.gates.iter().map(|gg| (crate::rig::dist(pos(r), gg.centre), gg.waypoint, gg.group)).fold((f64::INFINITY, 0, 0), |a, b| if b.0 < a.0 { b } else { a });
                            eprintln!("    credit cps {c} at ({:.1}, {:.1}, {:.1}): nearest gate wp{} group {} at {:.1} m -> bit {bit}", r.x, r.y, r.z, nearest.1, nearest.2, nearest.0);
                        }
                    } else {
                        bit = (c as u32 - 1).min(30);
                    }
                    mask |= 1 << bit;
                    prev_cps = c;
                }
            }
            // k = how many leading groups of the order are credited (the leg the car is on)
            let k_pref = (0..track.n_groups).take_while(|i| mask & (1 << i) != 0).count();
            // a credit was taken, so re-localize globally (a gate reached out of order sits far from the hint)
            let seg_hint = if cps > base.map(|e| cps_of(&e.end)).unwrap_or(root_cps) { track.pts.len() / 2 } else { seg_hint };
            let win = if cps > base.map(|e| cps_of(&e.end)).unwrap_or(root_cps) { track.pts.len() } else { 80 };
            let (s, lat, seg, d3) = track.project(pos(&end), seg_hint, win);
            if debug && base.is_none() && std::env::var("TMREACH_LAP_TRACE").map(|m| desc.contains(&m)).unwrap_or(false) {
                for (i, r) in rows.iter().enumerate().step_by(20) {
                    let (ss, ll, sg, dd) = track.project(pos(r), seg_hint, 80);
                    eprintln!("      t {i:4}: ({:.1}, {:.1}, {:.1}) v {:.1} vy {:+.1} s {:.1} lat {:.1} seg {sg} d3 {:.1} cps {}", r.x, r.y, r.z, speed(r), r.vy, ss, ll, dd, cps_of(r));
                }
            }
            if debug && base.is_none() {
                eprintln!("    seed macro {desc:40}: end ({:.1}, {:.1}, {:.1}) v {:.1} s {:.1} lat {:.1} d3 {:.1} cps {cps} rows {}", end.x, end.y, end.z, speed(&end), s, lat, d3, rows.len());
            }
            // off the world / far off the road on a road leg: no cell
            if end.y < -20.0 {
                out.deaths[0] += 1;
                continue;
            }
            // the LEG toward the next uncredited gate decides (a jump's flight projects onto whatever road is near)
            let on_gap = track.leg_gap.get(k_pref).copied().unwrap_or(false) || track.gap_seg.get(seg).copied().unwrap_or(false);
            let hw = track.half_width.get(seg).copied().unwrap_or(5.5);
            let lat_abs = lat.abs();
            if !on_gap && (lat_abs > hw + cfg.lat_tol || d3 > 25.0 + cfg.lat_tol) {
                out.deaths[1] += 1;
                if debug_fan {
                    eprintln!("    OFFROUTE {desc:34}: end ({:.1}, {:.1}, {:.1}) v {:.1} s {s:.1} lat {lat:.1} d3 {d3:.1} hw {hw:.1}", end.x, end.y, end.z, speed(&end));
                }
                continue;
            }
            // fell off: far below the nearest centreline point on a road leg; on a GAP leg (jump, drop,
            // bowl) anything above the track's lowest point - 5 m and within 120 m of the polyline lives
            let road_y = track.at(s)[1];
            // on a steep climb the car is legitimately below the line point at its own s (21's 32-degree ramp:
            // 4-6 m); "below" is measured against the line's LOWEST point within 15 m of arc length
            let road_y_min = track.min_y_near(s, 15.0);
            // (below the polyline while laterally ON the road = a dip the centreline's y does not follow: 08 at s 585)
            if (!on_gap && end.y < road_y - 5.0 && (lat_abs > hw + 1.0 || end.y < road_y_min - cfg.below_tol)) || (on_gap && (end.y < track_min_y - 5.0 || d3 > 120.0)) {
                out.deaths[2] += 1;
                if debug_fan {
                    eprintln!("    FELL {desc:38}: end ({:.1}, {:.1}, {:.1}) v {:.1} s {s:.1} lat {lat:.1} d3 {d3:.1} road_y {road_y:.1} min15 {road_y_min:.1} hw {hw:.1}", end.x, end.y, end.z, speed(&end));
                }
                continue;
            }
            // dead: stopped and not at the start
            if speed(&end) < 1.0 && s > 5.0 && cps == root_cps {
                out.deaths[3] += 1;
                continue;
            }
            if speed(&end) < 3.0 {
                out.deaths[4] += 1;
            } else {
                out.deaths[5] += 1;
            }
            let mut chain = chain0.clone();
            chain.extend(recs);
            let mut descs = base.map(|e| e.macro_desc.clone()).unwrap_or_default();
            descs.push(desc);
            // progress: credits first; arc length only up to the next uncredited gate (+10 m): a road
            // reached below/beside the route without its gate earns nothing beyond that gate
            // past the next gate's arc length WITHOUT its credit = off the route: progress falls back to 20 m before that gate
            // the next gate is the first uncredited one IN ORDER; credits out of order earn nothing here
            let k_next = k_pref;
            let s_gate = track.gate_s.get(k_next).copied().unwrap_or(f64::INFINITY);
            let s_prev = if k_next == 0 { 0.0 } else { track.gate_s.get(k_next - 1).copied().unwrap_or(0.0) };
            let progress = if on_gap {
                // a GAP leg (coordinator 13:54Z: 21's line flies past the gate's z and comes back 8-10 s later):
                // no off-route rule; the leg's start plus a WEAK pull toward the next gate (<= 5 m worth);
                // novelty comes from the 4 m squares and the visit discount
                let g = track.at(s_gate.min(track.len_m()));
                // 3-D: the ground under an elevated gate is not near it (21: cells at y 32 under the deck at 48)
                let dg = ((end.x - g[0]).powi(2) + (2.0 * (end.y - g[1])).powi(2) + (end.z - g[2]).powi(2)).sqrt();
                k_pref as f64 * 10_000.0 + s_prev + 0.5 * (1.0 - dg / 300.0).clamp(0.0, 1.0)
            } else {
                // past the next gate without its credit: worth only the leg start (20 m before the gate
                // ranked level with a legit approach; 20's lower deck under gate 3 sat there for 2 h)
                let s_eff = if s > s_gate + 40.0 { s_prev + 10.0 } else { s };
                // SPEED matters on jump/ramp/wall legs (21: 31 m/s at the foot or the car drops in the gap):
                // a cell at the human's speed ranks 30 m ahead of a stopped one at the same arc length
                let vh = track.human_speed_at(s);
                let speed_bonus = if vh > 3.0 { 30.0 * (speed(&end) / vh).clamp(0.0, 1.2) } else { 0.0 };
                k_pref as f64 * 10_000.0 + s_eff - 0.02 * lat_abs.min(20.0) + speed_bonus
            };
            // on a gap leg the arc length says little: the cell is the 4 m x 4 m ground square there
            // on a road leg the cell also carries a 2 m LATERAL bucket (14's ramp: the line's x on the ramp decides the flight)
            let cs = if on_gap { ((end.x / 4.0).floor() as i32) * 100_000 + (end.z / 4.0).floor() as i32 } else { ((s / 4.0).floor() as i32) * 64 + ((lat / 2.0).floor() as i32 + 32).clamp(0, 63) };
            let e = Entry { key: Key { cs, cv: (speed(&end) / 5.0).floor() as i32, cy: (end.y / 3.0).floor() as i32, mask }, chain, cps, mask, s, seg, progress, visits: 0, end: end.clone(), macro_desc: descs };
            // CLINIC: the next gate credited with a good arrival ends this leg
            if cfg.clinic && out.finished.is_none() && k_pref > seed_k.get() {
                let vh = track.human_speed_at(s);
                let v = speed(&end);
                let speed_ok = vh <= 0.0 || ((v - vh).abs() <= 0.3 * vh.max(5.0));
                let lat_ok = lat_abs <= hw + 0.5;
                if speed_ok && lat_ok {
                    out.log.push(format!("LEG DONE: gate {} (order position {}) credited with a good arrival at race {}: s {s:.1} lat {lat:.1} v {v:.1} (human {vh:.1}) after {} ticks ({} macros)", k_pref, k_pref - 1, crate::secs(w.race_of(&end)), e.chain.len(), e.macro_desc.len()));
                    out.leg_done = Some(k_pref);
                    out.finished = Some(e.clone());
                } else {
                    near_misses += 1;
                    if near_misses <= 20 {
                        out.log.push(format!("  clinic: gate {} credited but arrival poor: lat {lat:.1} (hw {hw:.1}) v {v:.1} vs human {vh:.1}", k_pref));
                    }
                }
            }
            // the finish: the engine counter reached every group (the finish only credits with all checkpoints)
            if cps >= n_groups && out.finished.is_none() {
                out.log.push(format!("FINISH credited: cps {cps} at race {} after {} ticks of inputs ({} macros)", crate::secs(w.race_of(&end)), e.chain.len(), e.macro_desc.len()));
                out.finished = Some(e.clone());
            }
            if out.best.as_ref().map(|b| progress > b.progress).unwrap_or(true) {
                out.best = Some(e.clone());
            }
            match archive.entry(e.key.clone()) {
                std::collections::hash_map::Entry::Vacant(v) => {
                    v.insert(e);
                }
                std::collections::hash_map::Entry::Occupied(mut oc) => {
                    let cur = oc.get_mut();
                    // fewer ticks (faster to here) or further along replaces
                    if e.chain.len() < cur.chain.len() || (e.chain.len() == cur.chain.len() && e.progress > cur.progress) {
                        let visits = cur.visits;
                        *cur = e;
                        cur.visits = visits;
                    }
                }
            }
        }
        let _ = rng;
        Ok(count)
    };

    // SEED: the root, or the base tape replayed for --prefix-ticks (its state becomes the first cell)
    if cfg.prefix_ticks > 0 || cfg.seed_chain.is_some() {
        let mut recs = match &cfg.seed_chain {
            Some(c) => c.clone(),
            None => w.reference_recs(root, cfg.prefix_ticks),
        };
        // --seed-to-gate K: keep the seed only up to 0.3 s after its K-th credit (the parent's rule: seed the stuck
        // leg from a FASTER upstream chain, gate N-2, and let the speed-matching search redo the approach)
        if cfg.seed_to_gate > 0 {
            let (rows0, nh0) = w.rollout_keep(branch::ROOT, &recs, root, recs.len() as u64)?;
            w.release(nh0);
            let root_c = if root_row.cps == u32::MAX { 0 } else { root_row.cps };
            let mut cut_at: Option<usize> = None;
            for (i, r) in rows0.iter().enumerate() {
                let c = if r.cps == u32::MAX { 0 } else { r.cps };
                if c >= root_c + cfg.seed_to_gate as u32 {
                    cut_at = Some((i + 30).min(recs.len()));
                    break;
                }
            }
            match cut_at {
                Some(n) => {
                    out.log.push(format!("seed cut to {n} ticks: 0.3 s after its credit #{}", cfg.seed_to_gate));
                    recs.truncate(n);
                }
                None => out.log.push(format!("seed never reaches credit #{}; kept whole", cfg.seed_to_gate)),
            }
        }
        // a seed whose end state the search would drop (stopped, fallen under the line, off the road) is a
        // dead end: cut the chain back 3 s at a time (up to 12 times) until it ends in a live state
        let mut tries = 0;
        let (rows, nh, end, s, seg) = loop {
            let (rows, nh) = w.rollout_keep(branch::ROOT, &recs, root, recs.len() as u64)?;
            let end = rows.last().cloned().unwrap_or_else(|| root_row.clone());
            let (s, lat, seg, d3) = track.project(pos(&end), track.pts.len() / 2, track.pts.len());
            let road_y = track.at(s)[1];
            let hw = track.half_width.get(seg).copied().unwrap_or(5.5);
            let road_y_min = track.min_y_near(s, 15.0);
            let bad = speed(&end) < 3.0 || (end.vy < -3.0 && end.y < road_y_min - 3.0) || lat.abs() > hw + cfg.lat_tol || d3 > 25.0 + cfg.lat_tol || (end.y < road_y - 5.0 && (lat.abs() > hw + 1.0 || end.y < road_y_min - cfg.below_tol));
            if !bad || tries >= 40 || recs.len() <= 300 {
                break (rows, nh, end, s, seg);
            }
            w.release(nh);
            tries += 1;
            // 3 s per cut for the first 12, then 15 s (a car that stopped at 24 s of a 106 s chain)
            let step = if tries <= 12 { 300 } else { 1500 };
            let cut = recs.len().saturating_sub(step).max(300);
            recs.truncate(cut);
            out.log.push(format!("seed ends in a dead state (v {:.1}, vy {:+.1}, {:.1} m below the line, lat {:.1}); chain cut to {} ticks", speed(&end), end.vy, road_y - end.y, lat, cut));
            if cfg.verbose {
                eprintln!("{}", out.log.last().unwrap());
            }
        };
        let recs_len = recs.len();
        let cps = cps_of(&end);
        // credits along the prefix, attributed like a rollout's
        let mut mask = 0u32;
        let mut prev = root_cps;
        for r in &rows {
            let c = cps_of(r);
            if c > prev {
                let mut bit = 31u32;
                if let Some(g) = &cfg.gates {
                    let best = g.gates.iter().filter(|gg| gg.kind != crate::gates::GateKind::Start).map(|gg| (crate::rig::dist(pos(r), gg.centre), gg.group)).fold((30.0f64, None), |a, b| if b.0 < a.0 { (b.0, Some(b.1)) } else { a });
                    if let Some(grp) = best.1 {
                        if let Some(p) = track.order_groups.iter().position(|x| *x == grp) {
                            bit = p as u32;
                        }
                    }
                } else {
                    bit = (c as u32 - 1).min(30);
                }
                mask |= 1 << bit;
                prev = c;
            }
        }
        let k_pref = (0..track.n_groups).take_while(|i| mask & (1 << i) != 0).count();
        seed_k.set(k_pref);
        // the same cap as a rollout: past the next uncredited gate without its credit = off the route
        let s_gate_seed = track.gate_s.get(k_pref).copied().unwrap_or(f64::INFINITY);
        let s_prev_seed = if k_pref == 0 { 0.0 } else { track.gate_s.get(k_pref - 1).copied().unwrap_or(0.0) };
        let s_eff_seed = if s > s_gate_seed + 40.0 { s_prev_seed + 10.0 } else { s };
        let seed = Entry { key: Key { cs: (s / 4.0).floor() as i32, cv: (speed(&end) / 5.0).floor() as i32, cy: (end.y / 3.0).floor() as i32, mask }, chain: recs, cps, mask, s, seg, progress: k_pref as f64 * 10_000.0 + s_eff_seed, visits: 0, end: end.clone(), macro_desc: vec![format!("seed chain {} ticks", recs_len)] };
        out.log.push(format!("seed from a {} tick chain: ({:.1}, {:.1}, {:.1}) v {:.1} cps {cps} mask {mask:#x} s {s:.1}", recs_len, end.x, end.y, end.z, speed(&end)));
        if cfg.verbose {
            eprintln!("{}", out.log.last().unwrap());
        }
        let from = w.floor(nh)?;
        out.best = Some(seed.clone());
        out.rollouts += fan(w, nh, from, Some(&seed), &mut archive, &mut out, &mut rng, h)?;
        w.release(nh);
        archive.entry(seed.key.clone()).or_insert(seed);
    } else {
        out.rollouts += fan(w, branch::ROOT, root, None, &mut archive, &mut out, &mut rng, h)?;
    }
    out.log.push(format!("seed: {} rollouts -> {} cells, best s {:.1} cps {}", out.rollouts, archive.len(), out.best.as_ref().map(|b| b.s).unwrap_or(0.0), out.best.as_ref().map(|b| b.cps).unwrap_or(0)));
    let mut last_report = std::time::Instant::now();
    let mut stagnant: usize = 0;
    let mut consecutive_failures: usize = 0;
    while out.rollouts < cfg.budget && out.finished.is_none() {
        if archive.is_empty() {
            break;
        }
        // expandable: the chain plus a macro must fit the tape (and any explicit cap)
        let cap = cfg.max_chain_ticks.min(n.saturating_sub(root + 10));
        let mut keys: Vec<(Key, f64)> = archive.values().filter(|e| e.chain.len() + h <= cap).map(|e| (e.key.clone(), e.progress)).collect();
        if keys.is_empty() {
            break;
        }
        keys.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap());
        let m = keys.len();
        // steep preference for the frontier (top ranks), discounted by visits; 10 % uniform
        // stagnation (no best improvement for 150 steps) widens the uniform share to 50 %
        let uni = if stagnant > 150 { 1.0 } else { 0.1 };
        let weights: Vec<f64> = keys.iter().enumerate().map(|(i, (k, _))| {
            let rank = (i + 1) as f64 / m as f64;
            let v = archive[k].visits as f64;
            (0.05 + rank).powi(4) / (1.0 + v).sqrt() + uni / m as f64
        }).collect();
        let total: f64 = weights.iter().sum();
        let mut pick = rng.unit() * total;
        let mut chosen = keys.len() - 1;
        for (i, wgt) in weights.iter().enumerate() {
            if pick < *wgt {
                chosen = i;
                break;
            }
            pick -= wgt;
        }
        let key = keys[chosen].0.clone();
        let entry = archive.get(&key).unwrap().clone();
        archive.get_mut(&key).unwrap().visits += 1;
        // return: replay the chain from the root
        let (rows, nc) = match w.rollout_keep(branch::ROOT, &entry.chain, root, entry.chain.len() as u64) {
            Ok(x) => {
                consecutive_failures = 0;
                x
            }
            Err(e) => {
                consecutive_failures += 1;
                if out.log.len() < 200 {
                    out.log.push(format!("  return failed (chain {} ticks): {e}", entry.chain.len()));
                }
                // a dead fork server (its socket gone: EPIPE) fails every return; spinning on it burns
                // a core for ever (2026-09-08 18:28Z, four boxes) -- abort so a supervisor can relaunch
                if consecutive_failures >= 20 {
                    return Err(format!("the fork server is gone: {consecutive_failures} consecutive return failures, last: {e}"));
                }
                continue;
            }
        };
        if let Some(a) = rows.last() {
            let d = crate::rig::dist(pos(a), pos(&entry.end));
            if d > 0.05 {
                out.log.push(format!("  RETURN MISMATCH: replay end {:.3} m off the archived end (chain {} ticks)", d, entry.chain.len()));
            }
        }
        let from = w.floor(nc)?;
        // a cell already fanned at h gets 1.5 h next (U-turns, ramp approaches need longer macros)
        let hh = if entry.visits >= 1 { h * 3 / 2 } else { h };
        let best_before = out.best.as_ref().map(|b| b.progress).unwrap_or(0.0);
        out.rollouts += fan(w, nc, from, Some(&entry), &mut archive, &mut out, &mut rng, hh)?;
        w.release(nc);
        out.steps += 1;
        // CLINIC viability check: a "good arrival" must also CONTINUE — replay the chain and follow the
        // line for 3 s; a car that stalls (21: 20.6 m/s on a 32-degree ramp, 2 m below the deck) is
        // not a leg done. Refused arrivals are penalised so the search moves on.
        if cfg.clinic && out.leg_done.is_some() {
            if let Some(f) = out.finished.clone() {
                if (f.cps as usize) < track.n_groups {
                    let (rows0, nf) = w.rollout_keep(branch::ROOT, &f.chain, root, f.chain.len() as u64)?;
                    let start = rows0.last().cloned().unwrap_or_else(|| f.end.clone());
                    let from_f = w.floor(nf)?;
                    let mut cur = nf;
                    let mut last = start.clone();
                    let mut seg_h = f.seg;
                    let mut ok = true;
                    let mut done_t = 0usize;
                    while done_t < 300 {
                        let (st, sg, s_now) = follow_steer(cfg, &last, seg_h, 1.0);
                        seg_h = sg;
                        // pedals like the human: gas below the human's speed a little ahead, coast above it, brake well above
                        let vh = track.human_speed_at(s_now + 15.0);
                        let v_now = speed(&last);
                        let (gas, brake) = if vh <= 3.0 || v_now <= vh * 1.05 { (1.0, 0.0) } else if v_now > vh * 1.3 { (0.0, 1.0) } else { (0.0, 0.0) };
                        let recs: Vec<Rec> = (0..10).map(|_| Rec { steer: st, gas, brake }).collect();
                        match w.forest.advance_or_end(cur, &recs, from_f + done_t, 10)? {
                            branch::Advanced::Node(rs, c) => {
                                if cur != nf {
                                    w.release(cur);
                                }
                                cur = c;
                                if let Some(x) = rs.last() {
                                    last = x.clone();
                                }
                            }
                            branch::Advanced::RunEnded(_) => break,
                        }
                        done_t += 10;
                    }
                    if cur != nf {
                        w.release(cur);
                    }
                    w.release(nf);
                    let (s_after, lat_after, _, _) = track.project(pos(&last), seg_h, 200);
                    let gained = s_after - f.s;
                    let below_after = track.min_y_near(s_after, 15.0) - last.y;
                    // alive, still on the line, and not fallen under it (21: a car in the deck gap lands on the road 25 m below and "moves on")
                    if speed(&last) < 8.0 || gained < 25.0 || below_after > cfg.below_tol || lat_after.abs() > 12.0 {
                        ok = false;
                    }
                    if !ok {
                        out.log.push(format!("  clinic: arrival at s {:.1} refused — 3 s later v {:.1}, s +{:.1} m, {:.1} m under the line, lat {:.1}; searching on", f.s, speed(&last), gained, below_after, lat_after));
                        if cfg.verbose {
                            eprintln!("{}", out.log.last().unwrap());
                        }
                        out.finished = None;
                        out.leg_done = None;
                        if let Some(e) = archive.get_mut(&f.key) {
                            e.visits += 8;
                        }
                    } else {
                        out.log.push(format!("  clinic: arrival viable — 3 s later v {:.1}, s +{:.1} m", speed(&last), gained));
                    }
                }
            }
        }
        if out.best.as_ref().map(|b| b.progress).unwrap_or(0.0) > best_before + 0.5 { stagnant = 0 } else { stagnant += 1 }
        if cfg.verbose && last_report.elapsed().as_secs() >= 30 {
            last_report = std::time::Instant::now();
            let _ = dump_archive(&archive, &cfg.out);
            if let Some(b) = &out.best {
                let _ = std::fs::write(cfg.out.join("best.tsv"), tsv_text(&b.chain));
            }
            let b = out.best.as_ref().unwrap();
            eprintln!("  [{} rollouts, {} steps, {} cells; ends: offworld {} offroute {} fell {} stopped {} crawl {} alive {}] best: mask {:#x} cps {} s {:.1} m ({:.0} %) speed {:.1} m/s after {:.2} s, chain {:?}", out.rollouts, out.steps, archive.len(), out.deaths[0], out.deaths[1], out.deaths[2], out.deaths[3], out.deaths[4], out.deaths[5], b.mask, b.cps, b.s, 100.0 * b.s / track.len_m(), speed(&b.end), b.chain.len() as f64 / 100.0, b.macro_desc.iter().rev().take(3).collect::<Vec<_>>());
        }
    }
    out.cells = archive.len();
    dump_archive(&archive, &cfg.out)?;
    // the best chain's rows (x y z speed cps per tick), replayed from the root
    if let Some(b) = &out.best {
        if let Ok((rows, hh)) = w.rollout_keep(branch::ROOT, &b.chain, root, b.chain.len() as u64) {
            w.release(hh);
            let mut s = String::from("tick\tx\ty\tz\tspeed\tcps\n");
            for (i, r) in rows.iter().enumerate() {
                s.push_str(&format!("{i}\t{:.2}\t{:.2}\t{:.2}\t{:.1}\t{}\n", r.x, r.y, r.z, speed(r), if r.cps == u32::MAX { -1 } else { r.cps as i64 }));
            }
            let _ = std::fs::write(cfg.out.join("best-rows.tsv"), s);
        }
    }
    Ok(out)
}

fn dump_archive(archive: &std::collections::HashMap<Key, Entry>, out: &std::path::Path) -> Result<(), String> {
    let mut s = String::from("cs\tcv\tcps\tvisits\tprogress\ts\tticks\tend_x\tend_y\tend_z\tend_speed\tlast_macro\tmask\n");
    let mut entries: Vec<&Entry> = archive.values().collect();
    entries.sort_by(|a, b| b.progress.partial_cmp(&a.progress).unwrap());
    for e in entries.iter().take(2000) {
        s.push_str(&format!("{}\t{}\t{}\t{}\t{:.1}\t{:.1}\t{}\t{:.2}\t{:.2}\t{:.2}\t{:.1}\t{}\t{:#x}\n", e.key.cs, e.key.cv, e.cps, e.visits, e.progress, e.s, e.chain.len(), e.end.x, e.end.y, e.end.z, speed(&e.end), e.macro_desc.last().cloned().unwrap_or_default(), e.mask));
    }
    std::fs::create_dir_all(out).map_err(|e| e.to_string())?;
    std::fs::write(out.join("archive.tsv"), s).map_err(|e| e.to_string())
}

/// The chain as the player's gtape text (one line per 10 ms tick from race 0; steer ±65536).
pub fn gtape_text(chain: &[Rec]) -> String {
    let mut s = String::new();
    for (t, r) in chain.iter().enumerate() {
        s.push_str(&format!("t={t} mode=2 w=prev respawn=0 mouse=none vsame=0 steer={} accel={} brake={} flags=0x000000\n", (r.steer as f64 * 65536.0).round() as i64, (r.gas > 0.5) as u8, (r.brake > 0.5) as u8));
    }
    s
}

/// The chain as tmauto's `tick steer gas brake` TSV (steer -127..127).
pub fn tsv_text(chain: &[Rec]) -> String {
    let mut s = String::from("tick\tsteer\tgas\tbrake\n");
    for (t, r) in chain.iter().enumerate() {
        s.push_str(&format!("{t}\t{}\t{}\t{}\n", (r.steer * 127.0).round() as i32, (r.gas > 0.5) as u8, (r.brake > 0.5) as u8));
    }
    s
}
