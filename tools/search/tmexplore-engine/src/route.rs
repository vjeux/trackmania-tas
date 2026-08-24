//! Agent B's `MapPack` and `Route`, as they arrive on disk.
//!
//! B publishes two JSON files per map: `<uid>.pack.json` (author time, spawn,
//! checkpoint gates) and `<uid>.route.json` (the polyline, arc lengths,
//! corridor half-widths, and the arc length of each gate).
//!
//! Read-only for us, and nothing in here reaches outside those two files and
//! the `.Map.Gbx` itself.

use tmexplore::branch::{GateLadder, Progress, Route};
use tmtraj::json::{parse, J};

pub struct MapPack {
    pub uid: String,
    pub name: String,
    /// The bar, and it is a number in the map file — not a human's time.
    pub author_ms: i64,
    pub spawn: [f32; 3],
    pub spawn_yaw: f32,
    pub n_checkpoints: u32,
    /// Checkpoint gate positions, in the MAP FILE's order (not tour order).
    pub cp_pos: Vec<[f32; 3]>,
    /// Finish gate positions.
    pub finish_pos: Vec<[f32; 3]>,
}

pub struct BRoute {
    pts: Vec<[f32; 3]>,
    cum: Vec<f32>,
    half: Vec<f32>,
    spacing: f32,
    n_cp: u32,
    /// Arc length of each gate in ROUTE order, finish last.
    pub gate_s: Vec<f32>,
    /// Which pack checkpoint each tour position refers to.
    pub order: Vec<usize>,
    grid: std::collections::HashMap<(i32, i32), Vec<u32>>,
    cell: f32,
    /// How many vertices arrived with no measured corridor width.
    pub width_missing: usize,
    default_half: f32,
}

fn f(j: &J, k: &str) -> Option<f64> {
    j.get(k).map(|v| v.num())
}

impl MapPack {
    pub fn load(p: &std::path::Path) -> Result<MapPack, String> {
        let t = std::fs::read_to_string(p).map_err(|e| format!("{}: {}", p.display(), e))?;
        let j = parse(t.trim())?;
        let sp = j.get("spawn").ok_or("pack has no spawn")?.arr();
        Ok(MapPack {
            uid: j.get("uid").ok_or("pack has no uid")?.str().to_string(),
            name: j.get("name").map(|v| v.str().to_string()).unwrap_or_default(),
            author_ms: f(&j, "author_ms").ok_or("pack has no author_ms")? as i64,
            spawn: [sp[0].num() as f32, sp[1].num() as f32, sp[2].num() as f32],
            spawn_yaw: f(&j, "spawn_yaw").unwrap_or(0.0) as f32,
            n_checkpoints: j.get("checkpoints").map(|v| v.arr().len() as u32).unwrap_or(0),
            cp_pos: poslist(j.get("checkpoints")),
            finish_pos: poslist(j.get("finish")),
        })
    }
}

impl BRoute {
    /// `default_half` is used where B reports a corridor width of zero.
    ///
    /// A zero half-width would put every car off the route, so it cannot be
    /// taken at face value — but it is also not nothing: it means B has no
    /// surface measurement there. The substitution is counted and reported
    /// rather than applied quietly, because a route whose corridor is a
    /// default over half its length is a different instrument from one that
    /// measured it.
    pub fn load(p: &std::path::Path, default_half: f32) -> Result<BRoute, String> {
        let t = std::fs::read_to_string(p).map_err(|e| format!("{}: {}", p.display(), e))?;
        let j = parse(t.trim())?;
        let verts = j.get("verts").ok_or("route has no verts")?.arr();
        if verts.len() < 2 {
            return Err("a route needs at least two vertices".into());
        }
        let mut pts = Vec::with_capacity(verts.len());
        let mut cum = Vec::with_capacity(verts.len());
        let mut half = Vec::with_capacity(verts.len());
        let mut width_missing = 0;
        for v in verts {
            let pv = v.get("p").ok_or("a vert has no p")?.arr();
            pts.push([pv[0].num() as f32, pv[1].num() as f32, pv[2].num() as f32]);
            cum.push(v.get("s").ok_or("a vert has no s")?.num() as f32);
            let w = v.get("w").map(|x| x.num() as f32).unwrap_or(0.0);
            if w <= 0.0 {
                width_missing += 1;
                half.push(default_half);
            } else {
                half.push(w);
            }
        }
        let gate_s: Vec<f32> = j
            .get("gate_s")
            .map(|g| g.arr().iter().map(|v| v.num() as f32).collect())
            .unwrap_or_default();
        let spacing = j
            .get("stations")
            .and_then(|s| {
                let a = s.arr();
                if a.len() >= 2 {
                    Some((a[1].num() - a[0].num()) as f32)
                } else {
                    None
                }
            })
            .unwrap_or(20.0);
        let cell = 24.0;
        let mut grid: std::collections::HashMap<(i32, i32), Vec<u32>> = Default::default();
        for (i, q) in pts.iter().enumerate() {
            grid.entry(((q[0] / cell).floor() as i32, (q[2] / cell).floor() as i32))
                .or_default()
                .push(i as u32);
        }
        let order: Vec<usize> = j
            .get("order")
            .map(|o| o.arr().iter().map(|v| v.int() as usize).collect())
            .unwrap_or_default();
        let n_cp = gate_s.len().saturating_sub(1) as u32;
        Ok(BRoute { pts, cum, half, spacing, n_cp, gate_s, order, grid, cell, width_missing, default_half })
    }

    pub fn default_half(&self) -> f32 {
        self.default_half
    }
    pub fn points(&self) -> &[[f32; 3]] {
        &self.pts
    }
    pub fn n_verts(&self) -> usize {
        self.pts.len()
    }
}

impl BRoute {
    /// Nearest vertex within a window around `hint`, or globally when the hint
    /// is [`ANCHOR`].
    fn nearest(&self, pos: [f32; 3], hint: u32) -> (usize, f32) {
        let n = self.pts.len();
        let (lo, hi) = if hint == tmexplore::branch::ANCHOR {
            (0, n)
        } else {
            let h = (hint as usize).min(n - 1);
            let s0 = self.cum[h];
            let a = s0 - tmexplore::branch::CURSOR_BACK_M;
            let b = s0 + tmexplore::branch::CURSOR_AHEAD_M;
            let lo = self.cum.partition_point(|&c| c < a);
            let hi = self.cum.partition_point(|&c| c <= b).max(lo + 1).min(n);
            (lo, hi)
        };
        let mut best = (f32::INFINITY, lo.min(n - 1));
        for i in lo..hi {
            let q = self.pts[i];
            let d = (pos[0] - q[0]).powi(2) + (pos[1] - q[1]).powi(2) + (pos[2] - q[2]).powi(2);
            if d < best.0 {
                best = (d, i);
            }
        }
        (best.1, best.0.sqrt())
    }

    fn progress_at(&self, pos: [f32; 3], i: usize, d3: f32) -> Progress {
        let j = (i + 1).min(self.pts.len() - 1);
        let (tx, tz) = (self.pts[j][0] - self.pts[i][0], self.pts[j][2] - self.pts[i][2]);
        let (dx, dz) = (pos[0] - self.pts[i][0], pos[2] - self.pts[i][2]);
        let lateral = if tx * dz - tz * dx < 0.0 { d3 } else { -d3 };
        Progress { s: self.cum[i], lateral, on_route: d3 <= self.half[i] }
    }
}

impl Route for BRoute {
    /// THE WINDOWED MATCH. See `tmexplore::branch::ANCHOR` for why this exists:
    /// on this map the spawn is nearest to a vertex 1483 m along the route, so
    /// a global argmin scores a parked car at station 74 of 97.
    fn progress_from(&self, pos: [f32; 3], hint: u32) -> (Progress, u32) {
        let (i, d3) = self.nearest(pos, hint);
        (self.progress_at(pos, i, d3), i as u32)
    }

    fn progress(&self, pos: [f32; 3]) -> Progress {
        let cx = (pos[0] / self.cell).floor() as i32;
        let cz = (pos[2] / self.cell).floor() as i32;
        let mut best = (f32::INFINITY, usize::MAX);
        for dx in -1..=1 {
            for dz in -1..=1 {
                if let Some(l) = self.grid.get(&(cx + dx, cz + dz)) {
                    for &i in l {
                        let i = i as usize;
                        let q = self.pts[i];
                        let d = (pos[0] - q[0]).powi(2) + (pos[1] - q[1]).powi(2) + (pos[2] - q[2]).powi(2);
                        if d < best.0 {
                            best = (d, i);
                        }
                    }
                }
            }
        }
        if best.1 == usize::MAX {
            for (i, q) in self.pts.iter().enumerate() {
                let d = (pos[0] - q[0]).powi(2) + (pos[1] - q[1]).powi(2) + (pos[2] - q[2]).powi(2);
                if d < best.0 {
                    best = (d, i);
                }
            }
        }
        let i = best.1;
        let d3 = best.0.sqrt();
        let j = (i + 1).min(self.pts.len() - 1);
        let (tx, tz) = (self.pts[j][0] - self.pts[i][0], self.pts[j][2] - self.pts[i][2]);
        let (dx, dz) = (pos[0] - self.pts[i][0], pos[2] - self.pts[i][2]);
        let lateral = if tx * dz - tz * dx < 0.0 { d3 } else { -d3 };
        Progress { s: self.cum[i], lateral, on_route: d3 <= self.half[i] }
    }
    fn length(&self) -> f32 {
        *self.cum.last().unwrap()
    }
    fn spacing(&self) -> f32 {
        self.spacing
    }
    fn n_checkpoints(&self) -> u32 {
        self.n_cp
    }
}

fn poslist(j: Option<&J>) -> Vec<[f32; 3]> {
    match j {
        None => Vec::new(),
        Some(a) => a
            .arr()
            .iter()
            .filter_map(|e| e.get("pos"))
            .map(|p| {
                let v = p.arr();
                [v[0].num() as f32, v[1].num() as f32, v[2].num() as f32]
            })
            .collect(),
    }
}

impl BRoute {
    /// The required gates in TOUR order — the order the car must collect them
    /// — as `(arc length, world position)`, the finish last.
    ///
    /// `order` in the route file indexes the pack's checkpoint list; `gate_s`
    /// is already in tour order and is one longer, the extra entry being the
    /// finish. If those two disagree in length the route and the pack are not
    /// describing the same map and this returns an error rather than pairing
    /// them up positionally, which would silently attach the wrong position to
    /// an arc length.
    pub fn gate_ladder(&self, pack: &MapPack, radius: f32) -> Result<GateLadder, String> {
        let mut gates = Vec::new();
        for (i, &oi) in self.order.iter().enumerate() {
            let p = pack
                .cp_pos
                .get(oi)
                .ok_or_else(|| format!("route order names checkpoint {} and the pack has {}", oi, pack.cp_pos.len()))?;
            let s = *self
                .gate_s
                .get(i)
                .ok_or_else(|| format!("gate_s has {} entries, order has {}", self.gate_s.len(), self.order.len()))?;
            gates.push((s, *p));
        }
        if let (Some(f), Some(&s)) = (pack.finish_pos.first(), self.gate_s.last()) {
            gates.push((s, *f));
        }
        if gates.len() != self.gate_s.len() {
            return Err(format!(
                "built {} gates from a route with {} gate arc lengths -- the route and the pack \
                 disagree about this map",
                gates.len(),
                self.gate_s.len()
            ));
        }
        Ok(GateLadder { gates, radius })
    }
}
