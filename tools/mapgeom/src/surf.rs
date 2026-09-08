//! The drivable surface of a map as a graph, and the route through it.
//!
//! ## What this is for, and what it is deliberately not
//!
//! The route answers three questions and no others:
//!
//! 1. **which way is forward** — arc length `s` along a polyline;
//! 2. **am I still on the track** — a corridor test;
//! 3. **where are the checkpoints** — and **in what legal order**.
//!
//! It is never driven along. A route good to a few metres is ample for all
//! three, so nothing here trades coverage or correctness for smoothness.
//!
//! ## The construction
//!
//! ```text
//!   collision triangles (pak)  ->  a 2 m XZ grid of SURFACES, layered in y
//!                              ->  a graph: adjacent cells whose surfaces are
//!                                  within a drivable step of each other
//!                              ->  Dijkstra between the map's own gates
//!                              ->  the shortest tour that visits EVERY gate
//! ```
//!
//! **Forcing every gate into the tour is what makes the order legal.** The
//! earlier attempt searched a block graph for a good path and then read the
//! checkpoint order off it, which skips a hairpin and returns the checkpoints
//! in the wrong order — the path was free not to go there. Here the gates are
//! constraints, not observations: the tour visits all of them by construction
//! and the only freedom left is which order costs least.
//!
//! ## Road, and why it is not a hardcoded list of materials
//!
//! A shortest path over every collidable surface cuts across grass. A list of
//! "road materials" written down here would be a guess that silently omits
//! whatever Nadeo ships next season.
//!
//! So the road set is read **off the map being routed**: the physics material
//! under the spawn, and under every gate, is track *by definition* — those are
//! the places the game itself puts the car. Everything else is passable at a
//! penalty rather than forbidden, because some maps really do cross grass, and
//! a hard wall there would make a legal route impossible instead of expensive.


use crate::scene::Scene;
use std::collections::BinaryHeap;

/// Grid pitch. A stadium road is 25.6 m wide, so 2 m is thirteen cells across
/// it and two across the narrowest platform ribbon.
pub const CELL: f32 = 2.0;

/// The largest height change between neighbouring cells that a car can drive.
/// Over a 2 m step, 1.6 m is a 39° ramp — steeper than any road and shallower
/// than any wall.
pub const STEP: f32 = 1.6;

/// The largest DROP a car may take between neighbouring cells.
///
/// Asymmetric on purpose, and it is not a tuning knob: **a car can fall off a
/// ledge and cannot climb one.** With a symmetric step the graph could not
/// leave Summer 2026 - 01's start, which sits one whole 8 m cell row above the
/// rest of the track, so the shortest path to the nearest checkpoint came back
/// as 1303 m for a 390 m chord and the whole route was twice as long as a
/// 23-second map can be. Raising the symmetric step to 6 m changed nothing,
/// which is what said the limit was a DIRECTION and not a size.
pub const DROP: f32 = 12.0;

/// How far a leap may span horizontally.
pub const LEAP_XZ: f32 = 64.0;
/// How far a leap may land ABOVE its take-off (a small lip, not a climb).
pub const LEAP_RISE: f32 = 2.0;
/// How far a leap may fall.
pub const LEAP_DROP: f32 = 40.0;
/// Cost multiplier on a leap, so the route uses one only where no road goes.
pub const LEAP_COST: f32 = 4.0;

/// Cost multiplier for a cell that is not the map's own road material.
/// Passable, expensive: a hard barrier would make some legal routes impossible.
pub const OFFROAD: f32 = 20.0;

#[derive(Clone, Copy, Debug)]
pub struct Surf {
    pub y: f32,
    pub mat: u16,
    pub road: bool,
}

pub struct Grid {
    pub ox: f32,
    pub oz: f32,
    pub nx: usize,
    pub nz: usize,
    /// Surfaces per cell, highest first. Flattened: cell `iz*nx + ix`.
    pub cells: Vec<Vec<Surf>>,
    pub mats: Vec<String>,
}

impl Grid {
    pub fn idx(&self, ix: usize, iz: usize) -> usize {
        iz * self.nx + ix
    }
    pub fn cell_of(&self, x: f32, z: f32) -> Option<(usize, usize)> {
        let fx = ((x - self.ox) / CELL).floor();
        let fz = ((z - self.oz) / CELL).floor();
        if fx < 0.0 || fz < 0.0 {
            return None;
        }
        let (ix, iz) = (fx as usize, fz as usize);
        if ix >= self.nx || iz >= self.nz {
            return None;
        }
        Some((ix, iz))
    }
    pub fn centre(&self, ix: usize, iz: usize) -> (f32, f32) {
        (
            self.ox + (ix as f32 + 0.5) * CELL,
            self.oz + (iz as f32 + 0.5) * CELL,
        )
    }

    /// Rasterise a scene's triangles into the grid.
    ///
    /// Every triangle is written into the cells whose CENTRE its vertical
    /// projection contains, at the height of its own plane there. That is the
    /// same question `probe::Index::column` answers, asked once per cell
    /// instead of once per query, which is what makes a whole-map graph
    /// affordable.
    ///
    /// `NotCollidable` and `OffZone` are left out, exactly as the probe index
    /// leaves them out: a car cannot rest on one, and a route that plans over
    /// one is planning through the map.
    pub fn build(scene: &Scene) -> Grid {
        Self::build_within(scene, None)
    }

    /// `clip`: an XZ box the grid is limited to — the track's (spawn + gates ± margin). A stray placement 40 km
    /// away (NOSEDIVE, 2026-09-08: 22 339 × 20 318 cells, 42 M surfaces, 30 GB) must not size the grid.
    pub fn build_within(scene: &Scene, clip: Option<([f32; 3], [f32; 3])>) -> Grid {
        let (mut lo, mut hi) = scene.bounds().unwrap_or(([0.0; 3], [1.0; 3]));
        if let Some((cl, ch)) = clip {
            for a in [0usize, 2] {
                lo[a] = lo[a].max(cl[a]);
                hi[a] = hi[a].min(ch[a]);
                if hi[a] <= lo[a] { hi[a] = lo[a] + 1.0; }
            }
        }
        let ox = (lo[0] / CELL).floor() * CELL;
        let oz = (lo[2] / CELL).floor() * CELL;
        let nx = (((hi[0] - ox) / CELL).ceil() as usize + 1).max(1);
        let nz = (((hi[2] - oz) / CELL).ceil() as usize + 1).max(1);
        let mut g = Grid {
            ox,
            oz,
            nx,
            nz,
            cells: vec![Vec::new(); nx * nz],
            mats: Vec::new(),
        };
        let mut giant = 0usize;
        let mut total = 0usize;
        for (name, grp) in &scene.groups {
            if !crate::scene::is_collidable(name) {
                continue;
            }
            let mut verts = grp.verts.clone();
            if name.starts_with("Water") {
                for v in &mut verts {
                    v[1] -= crate::probe::WATER_DRAFT;
                }
            }
            let mi = g.mats.len() as u16;
            g.mats.push(name.clone());
            for t in &grp.tris {
                let a = verts[t[0] as usize];
                let b = verts[t[1] as usize];
                let c = verts[t[2] as usize];
                // a near-vertical face (|n.y| < 0.35, steeper than 70°) is a wall, not a surface the car stands on: the
                // side of a deck rasterized as heights made a fake ramp down to the road below (Argentina 2026)
                {
                    let e1 = [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
                    let e2 = [c[0] - a[0], c[1] - a[1], c[2] - a[2]];
                    let ny = e1[2] * e2[0] - e1[0] * e2[2];
                    let nl = ((e1[1] * e2[2] - e1[2] * e2[1]).powi(2) + ny * ny + (e1[0] * e2[1] - e1[1] * e2[0]).powi(2)).sqrt();
                    let wall_ny: f32 = std::env::var("TMPLAN_WALL_NY").ok().and_then(|s| s.parse().ok()).unwrap_or(0.35);
                    if nl > 1e-9 && ny.abs() / nl < wall_ny {
                        continue;
                    }
                }
                let minx = a[0].min(b[0]).min(c[0]);
                let maxx = a[0].max(b[0]).max(c[0]);
                let minz = a[2].min(b[2]).min(c[2]);
                let maxz = a[2].max(b[2]).max(c[2]);
                let i0 = (((minx - ox) / CELL).floor().max(0.0)) as usize;
                let i1 = ((((maxx - ox) / CELL).ceil()) as usize).min(nx - 1);
                let j0 = (((minz - oz) / CELL).floor().max(0.0)) as usize;
                let j1 = ((((maxz - oz) / CELL).ceil()) as usize).min(nz - 1);
                if i0 > i1 || j0 > j1 {
                    continue;
                }
                // a giant slab (NOSEDIVE, 2026-09-08: one map rasterized to > 30 GB of cell entries) is sampled at a
                // stride so it costs ≤ 200 000 entries; the probe still finds it, the graph loses nothing it had
                let cover = (i1 - i0 + 1) * (j1 - j0 + 1);
                let stride = ((cover as f32 / 200_000.0).sqrt().ceil() as usize).max(1);
                if stride > 1 { giant += 1; }
                total += cover / (stride * stride);
                if total > 400_000_000 {
                    eprintln!("  surface grid: > 400 M cell entries — the map is too large for the 2 m grid; stopping the rasterization here");
                    break;
                }
                for j in (j0..=j1).step_by(stride) {
                    for i in (i0..=i1).step_by(stride) {
                        let (cx, cz) = (ox + (i as f32 + 0.5) * CELL, oz + (j as f32 + 0.5) * CELL);
                        if let Some(y) = height_at(a, b, c, cx, cz) {
                            g.cells[j * nx + i].push(Surf { y, mat: mi, road: false });
                        }
                    }
                }
            }
        }
        if giant > 0 {
            eprintln!("  surface grid: {giant} giant triangles sampled at a stride ({total} cell entries)");
        }
        // Highest first, and collapse surfaces closer than a wheel's width:
        // a road slab and its kerb cap are one thing to drive on.
        for c in &mut g.cells {
            c.sort_by(|p, q| q.y.partial_cmp(&p.y).unwrap_or(std::cmp::Ordering::Equal));
            c.dedup_by(|p, q| (p.y - q.y).abs() < 0.25);
        }
        g
    }

    /// Mark which materials are the map's own road, from the places the game
    /// itself puts the car: the spawn and every gate.
    ///
    /// Each anchor comes with the vertical window to look in — see
    /// `Graph::nearest_window` for why a grid gate looks up and an item gate
    /// looks down.
    ///
    /// Returns the material names that were adopted, so a report can print
    /// them — a road set is a claim about the map and belongs in the output,
    /// not buried in a bool.
    pub fn learn_road(&mut self, anchors: &[([f32; 3], f32, f32)]) -> (Vec<String>, Vec<String>) {
        let mut votes: std::collections::BTreeMap<u16, usize> = Default::default();
        for (p, lo, hi) in anchors {
            let Some((ix, iz)) = self.cell_of(p[0], p[2]) else { continue };
            let c = &self.cells[iz * self.nx + ix];
            let mut best: Option<(f32, Surf)> = None;
            for s in c {
                let dy = s.y - p[1];
                if dy < *lo || dy > *hi {
                    continue;
                }
                let score = (dy - lo).abs();
                if best.map_or(true, |(b, _)| score < b) {
                    best = Some((score, *s));
                }
            }
            if let Some((_, s)) = best {
                *votes.entry(s.mat).or_insert(0) += 1;
            }
        }
        // Per-material cell counts, so a candidate can be checked against how
        // much of the map it covers.
        let total: usize = self.cells.iter().map(|c| c.len()).sum();
        let mut area: std::collections::BTreeMap<u16, usize> = Default::default();
        for c in &self.cells {
            for s in c {
                *area.entry(s.mat).or_insert(0) += 1;
            }
        }
        // **A material covering a quarter of the map is terrain, not track.**
        //
        // Learning the road from the surface under each gate is right, and it
        // is not robust on its own: one gate whose column happens to put a
        // rock face or a metal walkway at the top of its search window adopts
        // that whole material. On Summer 2026 - 16 that adopted `Rock` and
        // `ResonantMetal` and made 1 021 527 of 2 469 905 surfaces "road" —
        // 41 % of the map — after which the graph is uniformly cheap, the leap
        // rim is a million nodes wide, and the run did not finish in forty
        // minutes. A track never covers a quarter of its map; terrain always
        // does. The test is scale-free, so it needs no constant fitted to a
        // map size.
        const MAX_SHARE: f32 = 0.25;
        let mut want: Vec<u16> = Vec::new();
        let mut dropped: Vec<String> = Vec::new();
        // …unless MOST anchors voted for it: a map whose scene has no decoration (the tiny campaign copies,
        // 2026-09-07 — every road is one converter material) makes the track itself the majority of the
        // surfaces. The Summer 2026 - 16 false positives were single-gate votes.
        let n_votes: usize = votes.values().sum();
        // …and the game's own ROAD surfaces are track wherever they appear (a gate anchor votes only for the material
        // under IT — Argentina 2026's ice sections between the gates were "off-road" and the road-following
        // centreline broke into six gaps). Terrain (Grass, Sand, Rock, Water, Metal…) stays out of this list.
        // Physics ids, never model names: 16 Asphalt, 6 Dirt, 76 Green (the grass ON a platform deck — PlatformGrass),
        // 74 RoadIce… A platform map (Summer 2026 - 10: 200 Platform* placements, every checkpoint an Asphalt-physics
        // deck) has its decks as MOST of a scene without decoration, so these are exempt from the terrain share test.
        const ROAD_NAMES: [&str; 12] = ["Asphalt", "WetAsphalt", "Dirt", "WetDirtRoad", "DirtRoad", "RoadIce", "Ice", "Snow", "Wood", "Tech", "RoadSynthetic", "Green"];
        // Concrete (0) and Grass (2) are DECK physics too (platform decks, the converter's default id) — candidates,
        // but they stay under the terrain share test (grass fields are Grass as well)
        // Opt-in (env TMPLAN_DECK_PHYSICS="Concrete,Grass,Sand"): the road-following centreline sets it for the tiny
        // platform maps; the planner's road set for the exhibit does not change under it.
        let deck_env = std::env::var("TMPLAN_DECK_PHYSICS").unwrap_or_default();
        let deck_names: Vec<String> = deck_env.split(',').filter(|s| !s.is_empty()).map(|s| s.trim().to_string()).collect();
        let mut votes = votes;
        let mut road_by_physics: Vec<u16> = Vec::new();
        let mut deck_by_physics: Vec<u16> = Vec::new();
        for (mi, name) in self.mats.iter().enumerate() {
            let a = area.get(&(mi as u16)).copied().unwrap_or(0);
            if ROAD_NAMES.contains(&name.as_str()) && a > 0 {
                votes.entry(mi as u16).or_insert(1);
                road_by_physics.push(mi as u16);
            } else if deck_names.iter().any(|d| d == name) && a > 0 {
                votes.entry(mi as u16).or_insert(1);
                deck_by_physics.push(mi as u16);
            }
        }
        for (m, n) in &votes {
            let share = *area.get(m).unwrap_or(&0) as f32 / total.max(1) as f32;
            let majority = (*n >= 3 && *n * 2 >= n_votes) || road_by_physics.contains(m);
            // a deck physics (Concrete/Grass) is track up to 60 % of a scene — a tiny map has no stadium around it
            let limit = if deck_by_physics.contains(m) { 0.60 } else { MAX_SHARE };
            if share > limit && !majority {
                dropped.push(format!("{} ({:.0} % of the map)", self.mats[*m as usize], 100.0 * share));
            } else {
                want.push(*m);
            }
        }
        for c in &mut self.cells {
            for s in c {
                s.road = want.contains(&s.mat);
            }
        }
        (want.iter().map(|m| self.mats[*m as usize].clone()).collect(), dropped)
    }

    pub fn surface_count(&self) -> usize {
        self.cells.iter().map(|c| c.len()).sum()
    }
    pub fn road_count(&self) -> usize {
        self.cells.iter().map(|c| c.iter().filter(|s| s.road).count()).sum()
    }
}

// ---------------------------------------------------------------------------
// the graph
// ---------------------------------------------------------------------------

/// A node is one surface in one cell: `(cell, layer)`, flattened.
pub struct Graph {
    pub start: Vec<u32>,
    pub node_cell: Vec<u32>,
    pub node_layer: Vec<u8>,
    pub node_y: Vec<f32>,
    pub node_road: Vec<bool>,
    pub node_mat: Vec<u16>,
    /// Adjacency, flattened: `edges[edge_start[n] .. edge_start[n+1]]`.
    /// The top bit of each entry marks a **diagonal** step, which is `√2`
    /// cells long rather than one.
    pub edge_start: Vec<u32>,
    pub edges: Vec<u32>,
    /// **Leap edges: the gaps a car flies over.**
    ///
    /// A surface graph has no notion of flight, and TM maps are full of it.
    /// Nine of the twenty-five Summer 2026 maps came back "waypoint 0 cannot
    /// reach waypoint 1 over the drivable graph" with a symmetric-step graph
    /// and asymmetric drops both — because the track genuinely stops, and the
    /// car genuinely leaves the ground.
    ///
    /// So road cells on the RIM of a road — those with a neighbour direction
    /// that has no surface at all — may connect to another rim cell within
    /// `LEAP_XZ`, landing at or below their own height, provided nothing
    /// stands between them. The cost carries a penalty, so a leap is what the
    /// route uses when there is no road alternative rather than a shortcut it
    /// prefers.
    ///
    /// This is honest about being a model of flight and not flight: it says
    /// nothing about whether a car can *make* the jump at any speed. What it
    /// buys is a connected graph, which is what an arc length and a gate order
    /// need.
    pub leap_start: Vec<u32>,
    pub leaps: Vec<(u32, f32)>,
}

const DIAG: u32 = 0x8000_0000;
const SQRT2: f32 = std::f32::consts::SQRT_2;

impl Graph {
    /// Build the 8-connected surface graph.
    ///
    /// **Eight, not four.** A 4-connected grid can only produce staircases, so
    /// a straight run at 45° comes back 41 % longer than it is and the arc
    /// length — the one number the whole route exists to provide — is wrong by
    /// that much everywhere the track is not axis-aligned. The first version
    /// of this made exactly that mistake and reported a 23-second map as
    /// 3.3 km of route.
    ///
    /// A diagonal step is only allowed when **both** orthogonal cells between
    /// the two also carry a surface at a drivable height. Otherwise the path
    /// squeezes through the corner where two roads touch at a point, which is
    /// a place no car can go.
    pub fn build(g: &Grid) -> Graph {
        Graph::build_step(g, STEP)
    }

    pub fn build_step(g: &Grid, step: f32) -> Graph {
        let _ = STEP;
        let mut start = Vec::with_capacity(g.cells.len() + 1);
        let mut node_cell = Vec::new();
        let mut node_layer = Vec::new();
        let mut node_y = Vec::new();
        let mut node_road = Vec::new();
        let mut node_mat = Vec::new();
        for (ci, c) in g.cells.iter().enumerate() {
            start.push(node_cell.len() as u32);
            for (li, s) in c.iter().enumerate() {
                node_cell.push(ci as u32);
                node_layer.push(li.min(255) as u8);
                node_y.push(s.y);
                node_road.push(s.road);
                node_mat.push(s.mat);
            }
        }
        start.push(node_cell.len() as u32);
        let n = node_cell.len();
        let mut edge_start = Vec::with_capacity(n + 1);
        let mut edges = Vec::new();
        // does cell (jx,jz) hold a surface within STEP of `y`?
        let passable = |start: &Vec<u32>, node_y: &Vec<f32>, jx: i32, jz: i32, y: f32| -> bool {
            if jx < 0 || jz < 0 || jx >= g.nx as i32 || jz >= g.nz as i32 {
                return false;
            }
            let cj = jz as usize * g.nx + jx as usize;
            (start[cj]..start[cj + 1]).any(|k| {
                let dy = node_y[k as usize] - y;
                dy <= step && dy >= -DROP
            })
        };
        for i in 0..n {
            edge_start.push(edges.len() as u32);
            let ci = node_cell[i] as usize;
            let (ix, iz) = ((ci % g.nx) as i32, (ci / g.nx) as i32);
            let y = node_y[i];
            for (dx, dz) in [
                (1i32, 0i32),
                (-1, 0),
                (0, 1),
                (0, -1),
                (1, 1),
                (1, -1),
                (-1, 1),
                (-1, -1),
            ] {
                let diag = dx != 0 && dz != 0;
                if diag && !(passable(&start, &node_y, ix + dx, iz, y)
                    && passable(&start, &node_y, ix, iz + dz, y))
                {
                    continue;
                }
                let (jx, jz) = (ix + dx, iz + dz);
                if jx < 0 || jz < 0 || jx >= g.nx as i32 || jz >= g.nz as i32 {
                    continue;
                }
                let cj = jz as usize * g.nx + jx as usize;
                for k in start[cj]..start[cj + 1] {
                    let dy = node_y[k as usize] - y;
                    if dy <= step && dy >= -DROP {
                        edges.push(if diag { k | DIAG } else { k });
                    }
                }
            }
        }
        edge_start.push(edges.len() as u32);
        let mut gr = Graph {
            start,
            node_cell,
            node_layer,
            node_y,
            node_road,
            node_mat,
            edge_start,
            edges,
            leap_start: Vec::new(),
            leaps: Vec::new(),
        };
        gr.add_leaps(g);
        gr
    }

    /// Build the leap edges. See the field's own note for why they exist.
    fn add_leaps(&mut self, g: &Grid) {
        let n = self.len();
        // A rim node: a road node with at least one of its eight neighbours
        // carrying no surface it could step to. That is the lip of a gap.
        let mut rim: Vec<u32> = Vec::new();
        for i in 0..n {
            if !self.node_road[i] {
                continue;
            }
            if self.is_rim(g, i) {
                rim.push(i as u32);
            }
        }
        // Bucket the rim by a coarse grid so the pairwise search is local.
        let bs = LEAP_XZ;
        let mut buckets: std::collections::HashMap<(i32, i32), Vec<u32>> = Default::default();
        let key = |p: [f32; 3]| ((p[0] / bs).floor() as i32, (p[2] / bs).floor() as i32);
        let mut pos: Vec<[f32; 3]> = Vec::with_capacity(rim.len());
        for r in &rim {
            let p = self.world(g, *r as usize);
            buckets.entry(key(p)).or_default().push(*r);
            pos.push(p);
        }
        let mut out: Vec<Vec<(u32, f32)>> = vec![Vec::new(); n];
        for (ri, u) in rim.iter().enumerate() {
            let pu = pos[ri];
            let (kx, kz) = key(pu);
            for dz in -1..=1 {
                for dx in -1..=1 {
                    let Some(list) = buckets.get(&(kx + dx, kz + dz)) else { continue };
                    for v in list {
                        if v == u {
                            continue;
                        }
                        let pv = self.world(g, *v as usize);
                        let dxz = ((pv[0] - pu[0]).powi(2) + (pv[2] - pu[2]).powi(2)).sqrt();
                        if dxz < 2.0 * CELL || dxz > LEAP_XZ {
                            continue;
                        }
                        let dy = pv[1] - pu[1];
                        if dy > LEAP_RISE || dy < -LEAP_DROP {
                            continue;
                        }
                        if !self.clear(g, pu, pv) {
                            continue;
                        }
                        out[*u as usize].push((*v, dxz * LEAP_COST));
                    }
                }
            }
        }
        self.leap_start.clear();
        self.leaps.clear();
        for l in out {
            self.leap_start.push(self.leaps.len() as u32);
            self.leaps.extend(l);
        }
        self.leap_start.push(self.leaps.len() as u32);
    }

    /// Is this node on the RIM of a surface — does at least one of its eight
    /// directions have nothing it could step to?
    ///
    /// **Counting edges instead of directions is wrong**, and it was the first
    /// version of this: a cell can hold several stacked surfaces, so a node in
    /// the middle of a road with two layers under each neighbour has sixteen
    /// edges, while a genuine rim node with five open directions and two
    /// layers each has ten. Thresholding the edge count at eight then calls
    /// the middle of the road a rim and the actual lip not one, and every leap
    /// is computed from the wrong set. Summer 2026 - 02 came back
    /// "no rim pair within 240 m" because of it.
    fn is_rim(&self, g: &Grid, i: usize) -> bool {
        let ci = self.node_cell[i] as usize;
        let (ix, iz) = ((ci % g.nx) as i32, (ci / g.nx) as i32);
        let y = self.node_y[i];
        for (dx, dz) in [(1i32, 0i32), (-1, 0), (0, 1), (0, -1), (1, 1), (1, -1), (-1, 1), (-1, -1)]
        {
            let (jx, jz) = (ix + dx, iz + dz);
            if jx < 0 || jz < 0 || jx >= g.nx as i32 || jz >= g.nz as i32 {
                return true;
            }
            let cj = jz as usize * g.nx + jx as usize;
            let any = (self.start[cj]..self.start[cj + 1]).any(|k| {
                let dy = self.node_y[k as usize] - y;
                dy <= STEP && dy >= -DROP
            });
            if !any {
                return true;
            }
        }
        false
    }

    /// Is the straight line from `a` to `b` free of anything standing above
    /// both ends? A leap has to go over a gap, not through a wall.
    ///
    /// "Above" means above the flight line by more than a car's height and
    /// less than a barrier's — a surface far overhead is a canopy, not an
    /// obstruction, and rejecting on one makes every leap under a stadium roof
    /// impossible.
    fn clear(&self, g: &Grid, a: [f32; 3], b: [f32; 3]) -> bool {
        let d = ((b[0] - a[0]).powi(2) + (b[2] - a[2]).powi(2)).sqrt();
        let steps = (d / CELL).ceil() as i32;
        for k in 1..steps {
            let t = k as f32 / steps as f32;
            let (x, z) = (a[0] + (b[0] - a[0]) * t, a[2] + (b[2] - a[2]) * t);
            let Some((ix, iz)) = g.cell_of(x, z) else { return false };
            let floor = a[1] + (b[1] - a[1]) * t;
            for s in &g.cells[iz * g.nx + ix] {
                if s.y > floor + 1.5 && s.y < floor + 10.0 {
                    return false;
                }
            }
        }
        true
    }

    /// Which connected component each node belongs to, over the ordinary
    /// edges and the leaps together, **treated as undirected**.
    ///
    /// Undirected on purpose. The edges are asymmetric — a car can drop and
    /// cannot climb — so a forward DFS from one node computes its
    /// *reachable set*, which is not an equivalence relation and is not what
    /// "is this map in one piece" asks. The first version did that, decided a
    /// split map was a single component, and skipped bridging entirely.
    pub fn components(&self) -> Vec<u32> {
        let n = self.len();
        let mut p: Vec<u32> = (0..n as u32).collect();
        fn find(p: &mut Vec<u32>, a: u32) -> u32 {
            let mut a = a;
            while p[a as usize] != a {
                p[a as usize] = p[p[a as usize] as usize];
                a = p[a as usize];
            }
            a
        }
        let mut union = |p: &mut Vec<u32>, a: u32, b: u32| {
            let (ra, rb) = (find(p, a), find(p, b));
            if ra != rb {
                p[ra as usize] = rb;
            }
        };
        for u in 0..n {
            for e in self.edge_start[u]..self.edge_start[u + 1] {
                union(&mut p, u as u32, self.edges[e as usize] & !DIAG);
            }
            if !self.leap_start.is_empty() {
                for e in self.leap_start[u]..self.leap_start[u + 1] {
                    union(&mut p, u as u32, self.leaps[e as usize].0);
                }
            }
        }
        (0..n as u32).map(|i| find(&mut p, i)).collect()
    }

    /// **Bridge only what must be bridged.**
    ///
    /// A leap edge is local by design, so a map whose track crosses a gap
    /// wider than `LEAP_XZ` comes apart into islands: Summer 2026 - 02 splits
    /// into a component holding the start and CP1 (80.1 % of the graph) and
    /// one holding CP2 and the finish (0.2 %, 6 134 nodes — an elevated
    /// stretch that touches nothing).
    ///
    /// Widening `LEAP_XZ` globally to fix that would be the wrong trade: it
    /// was measured, and raising the leap's allowed RISE from 2 m to 12 m
    /// connected **no** additional map while making Summer 2026 - 10's route
    /// 333 m worse. A parameter that buys nothing and costs something is not a
    /// parameter to widen.
    ///
    /// So this runs only when the waypoints are actually split, joins exactly
    /// the components the waypoints live in, and **returns a line describing
    /// every bridge it added** — a bridge is a claim about the map and belongs
    /// in the transcript, not hidden in the graph.
    pub fn bridge_components(
        &mut self,
        g: &Grid,
        waypoints: &[usize],
        max_span: f32,
    ) -> Vec<String> {
        let mut notes = Vec::new();
        for _ in 0..waypoints.len() {
            let comp = self.components();
            let mut want: Vec<u32> = waypoints.iter().map(|w| comp[*w]).collect();
            want.sort_unstable();
            want.dedup();
            if want.len() < 2 {
                break;
            }
            // Rim nodes of each wanted component, bucketed for a local search.
            let home = comp[waypoints[0]];
            let mut best: Option<(f32, usize, usize)> = None;
            let mut a_side: Vec<usize> = Vec::new();
            let mut b_side: Vec<usize> = Vec::new();
            for i in 0..self.len() {
                if !self.node_road[i] || !self.is_rim(g, i) {
                    continue;
                }
                if comp[i] == home {
                    a_side.push(i);
                } else if want.contains(&comp[i]) {
                    b_side.push(i);
                }
            }
            for u in &a_side {
                let pu = self.world(g, *u);
                for v in &b_side {
                    let pv = self.world(g, *v);
                    let dxz = ((pv[0] - pu[0]).powi(2) + (pv[2] - pu[2]).powi(2)).sqrt();
                    if dxz > max_span {
                        continue;
                    }
                    let dy = pv[1] - pu[1];
                    let d3 = (dxz * dxz + dy * dy).sqrt();
                    if best.map_or(true, |(b, _, _)| d3 < b) && self.clear(g, pu, pv) {
                        best = Some((d3, *u, *v));
                    }
                }
            }
            let Some((d3, u, v)) = best else {
                notes.push(format!(
                    "UNBRIDGED: {} waypoint components remain and no rim pair within {} m has a \
                     clear line between them",
                    want.len(),
                    max_span
                ));
                break;
            };
            let (pu, pv) = (self.world(g, u), self.world(g, v));
            notes.push(format!(
                "bridged a {:.1} m gap ({:+.1} m in y) from ({:.0},{:.0},{:.0}) to \
                 ({:.0},{:.0},{:.0}) -- the map's track crosses it and the surface graph cannot",
                d3, pv[1] - pu[1], pu[0], pu[1], pu[2], pv[0], pv[1], pv[2]
            ));
            // Both ways: the tour may need to cross it in either direction,
            // and a one-way bridge would make the distance matrix lie.
            self.push_leap(u, v, d3 * LEAP_COST);
            self.push_leap(v, u, d3 * LEAP_COST);
        }
        notes
    }

    fn push_leap(&mut self, u: usize, v: usize, w: f32) {
        // The leap list is CSR, so an insert shifts every later entry. There
        // are a handful of bridges per map, so a rebuild is cheaper to get
        // right than an incremental splice.
        let mut lists: Vec<Vec<(u32, f32)>> = vec![Vec::new(); self.len()];
        for i in 0..self.len() {
            for e in self.leap_start[i]..self.leap_start[i + 1] {
                lists[i].push(self.leaps[e as usize]);
            }
        }
        lists[u].push((v as u32, w));
        self.leap_start.clear();
        self.leaps.clear();
        for l in lists {
            self.leap_start.push(self.leaps.len() as u32);
            self.leaps.extend(l);
        }
        self.leap_start.push(self.leaps.len() as u32);
    }

    pub fn len(&self) -> usize {
        self.node_cell.len()
    }
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    fn cost(&self, to: usize, diag: bool) -> f32 {
        let base = if diag { CELL * SQRT2 } else { CELL };
        if self.node_road[to] {
            base
        } else {
            base * OFFROAD
        }
    }

    /// Dijkstra over ROAD nodes only (no off-road cell, no leap): the path a road-following centreline may take;
    /// unreachable is `INFINITY` — a gap, not a detour.
    pub fn dijkstra_road(&self, from: usize) -> (Vec<f32>, Vec<u32>) {
        let n = self.len();
        let mut dist = vec![f32::INFINITY; n];
        let mut prev = vec![u32::MAX; n];
        let mut heap: BinaryHeap<Step> = BinaryHeap::new();
        dist[from] = 0.0;
        heap.push(Step { d: 0.0, n: from as u32 });
        while let Some(Step { d, n: u }) = heap.pop() {
            let u = u as usize;
            if d > dist[u] + 1e-6 {
                continue;
            }
            for e in self.edge_start[u]..self.edge_start[u + 1] {
                let raw = self.edges[e as usize];
                let v = (raw & !DIAG) as usize;
                // an off-road cell costs 5×: a kerb strip or a start deck of another material is crossed, a field is
                // not (the caller rejects paths with a long off-road run)
                let step = if raw & DIAG != 0 { CELL * std::f32::consts::SQRT_2 } else { CELL };
                // vertical continuity: a road follows its own slope (≤ 45°); a step down of more than the cell size is
                // a fall off the deck onto whatever lies below (Argentina 2026's start deck, 16 m above a road)
                if (self.node_y[v] - self.node_y[u]).abs() > 2.5 * step {
                    continue;
                }
                let nd = d + if self.node_road[v] { step } else { 5.0 * step };
                if nd < dist[v] {
                    dist[v] = nd;
                    prev[v] = u as u32;
                    heap.push(Step { d: nd, n: v as u32 });
                }
            }
            // a short leap (≤ 8 m in XZ) between road cells bridges the seam between two converted decks; longer
            // leaps are jumps, not road
            if !self.leap_start.is_empty() {
                for e in self.leap_start[u]..self.leap_start[u + 1] {
                    let (v, w) = self.leaps[e as usize];
                    let v = v as usize;
                    let dxz = w / LEAP_COST;
                    if dxz > 8.0 || !self.node_road[v] || (self.node_y[v] - self.node_y[u]).abs() > 2.0 {
                        continue;
                    }
                    let nd = d + dxz;
                    if nd < dist[v] {
                        dist[v] = nd;
                        prev[v] = u as u32;
                        heap.push(Step { d: nd, n: v as u32 });
                    }
                }
            }
        }
        (dist, prev)
    }

    /// Dijkstra from one node. Returns `(dist, prev)`; unreachable is `INFINITY`.
    pub fn dijkstra(&self, from: usize) -> (Vec<f32>, Vec<u32>) {
        let n = self.len();
        let mut dist = vec![f32::INFINITY; n];
        let mut prev = vec![u32::MAX; n];
        let mut heap: BinaryHeap<Step> = BinaryHeap::new();
        dist[from] = 0.0;
        heap.push(Step { d: 0.0, n: from as u32 });
        while let Some(Step { d, n: u }) = heap.pop() {
            let u = u as usize;
            if d > dist[u] + 1e-6 {
                continue;
            }
            for e in self.edge_start[u]..self.edge_start[u + 1] {
                let raw = self.edges[e as usize];
                let v = (raw & !DIAG) as usize;
                let nd = d + self.cost(v, raw & DIAG != 0);
                if nd < dist[v] {
                    dist[v] = nd;
                    prev[v] = u as u32;
                    heap.push(Step { d: nd, n: v as u32 });
                }
            }
            if !self.leap_start.is_empty() {
                for e in self.leap_start[u]..self.leap_start[u + 1] {
                    let (v, w) = self.leaps[e as usize];
                    let v = v as usize;
                    let nd = d + w;
                    if nd < dist[v] {
                        dist[v] = nd;
                        prev[v] = u as u32;
                        heap.push(Step { d: nd, n: v as u32 });
                    }
                }
            }
        }
        (dist, prev)
    }

    pub fn path(&self, prev: &[u32], to: usize) -> Vec<usize> {
        let mut out = vec![to];
        let mut c = to;
        while prev[c] != u32::MAX {
            c = prev[c] as usize;
            out.push(c);
        }
        out.reverse();
        out
    }

    /// The node nearest a world point, restricted to surfaces whose height
    /// lies in `p.y + lo ..= p.y + hi`, and scored by how close to `p.y + lo`
    /// they are.
    ///
    /// The window is not cosmetic. **A grid-placed gate's anchor is its cell's
    /// BASE, and the road inside that cell is 2 m above it** — measured: on
    /// Summer 2026 - 01 the cell row `cy = 6` at `yoff = -40` has its base at
    /// `y = 8` and its `Asphalt` at `y = 10.000`, and the spawn's row `cy = 7`
    /// has its base at 16 and its asphalt at 18.000. So a grid gate must look
    /// UP into its own cell and take the lowest surface it finds, while an
    /// item gate — whose position is absolute and already on the road — must
    /// look DOWN. One window for both finds the wrong surface for one of them,
    /// and on a map with a deck under the road it finds the deck.
    ///
    /// (That 2 m is also exactly the residual between the two map-height
    /// estimators in `yoff`: `cellmode` reads item anchors against their cell
    /// base and therefore reads `yoff + 2`, while `rest` reads them against
    /// the block surface and reads `yoff`. The offset has a mechanism, not a
    /// tolerance.)
    pub fn nearest_window(
        &self,
        g: &Grid,
        p: [f32; 3],
        max_ring: i32,
        lo: f32,
        hi: f32,
    ) -> Option<usize> {
        let (ix, iz) = g.cell_of(p[0], p[2])?;
        for r in 0..=max_ring {
            let mut best: Option<(f32, usize)> = None;
            for dz in -r..=r {
                for dx in -r..=r {
                    if r > 0 && dx.abs() != r && dz.abs() != r {
                        continue;
                    }
                    let (jx, jz) = (ix as i32 + dx, iz as i32 + dz);
                    if jx < 0 || jz < 0 || jx >= g.nx as i32 || jz >= g.nz as i32 {
                        continue;
                    }
                    let cj = jz as usize * g.nx + jx as usize;
                    for k in self.start[cj]..self.start[cj + 1] {
                        let k = k as usize;
                        let dy = self.node_y[k] - p[1];
                        if dy < lo || dy > hi {
                            continue;
                        }
                        // the surface CLOSEST to the anchor height wins (an item anchor sits on its deck); scoring from the
                        // window's floor picked a road 6 m under Argentina 2026's start deck (player, 2026-09-08)
                        let score = dy.abs() + if self.node_road[k] { 0.0 } else { 6.0 };
                        if best.map_or(true, |(b, _)| score < b) {
                            best = Some((score, k));
                        }
                    }
                }
            }
            if best.is_some() {
                return best.map(|(_, k)| k);
            }
        }
        None
    }

    pub fn world(&self, g: &Grid, n: usize) -> [f32; 3] {
        let ci = self.node_cell[n] as usize;
        let (x, z) = g.centre(ci % g.nx, ci / g.nx);
        [x, self.node_y[n], z]
    }
}

#[derive(PartialEq)]
struct Step {
    d: f32,
    n: u32,
}
impl Eq for Step {}
impl Ord for Step {
    fn cmp(&self, o: &Self) -> std::cmp::Ordering {
        // min-heap
        o.d.partial_cmp(&self.d).unwrap_or(std::cmp::Ordering::Equal).then(o.n.cmp(&self.n))
    }
}
impl PartialOrd for Step {
    fn partial_cmp(&self, o: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(o))
    }
}

fn height_at(a: [f32; 3], b: [f32; 3], c: [f32; 3], x: f32, z: f32) -> Option<f32> {
    let d = (b[2] - c[2]) * (a[0] - c[0]) + (c[0] - b[0]) * (a[2] - c[2]);
    if d.abs() < 1e-9 {
        return None;
    }
    let w0 = ((b[2] - c[2]) * (x - c[0]) + (c[0] - b[0]) * (z - c[2])) / d;
    let w1 = ((c[2] - a[2]) * (x - c[0]) + (a[0] - c[0]) * (z - c[2])) / d;
    let w2 = 1.0 - w0 - w1;
    const E: f32 = -1e-4;
    if w0 < E || w1 < E || w2 < E {
        return None;
    }
    Some(w0 * a[1] + w1 * b[1] + w2 * c[1])
}

// ---------------------------------------------------------------------------
// ordering the gates
// ---------------------------------------------------------------------------

/// The cheapest order in which every checkpoint can be collected between the
/// start and the finish.
///
/// Exact by Held–Karp while the count allows it — 16 checkpoints is 2^16 · 16²
/// states, which costs milliseconds — and nearest-neighbour plus 2-opt beyond
/// that, which is reported as such rather than silently substituted.
///
/// `d[i][j]` is the graph distance from waypoint `i` to waypoint `j`, with
/// index 0 the start and index `n-1` the finish.
pub struct Order {
    pub visit: Vec<usize>,
    pub cost: f32,
    pub exact: bool,
}

pub const EXACT_MAX: usize = 16;

/// An unreachable leg, as a finite cost the DP can carry.
///
/// Held–Karp on a matrix with `INFINITY` in it propagates NaN through
/// comparisons and quietly returns whatever the first candidate was, which
/// looks exactly like an answer. A large finite number keeps the arithmetic
/// honest and lets the caller see which legs the tour was forced through — the
/// check for that is at the call site, and it is not optional.
pub const UNREACHABLE: f32 = 1.0e12;

fn sub(v: f32) -> f32 {
    if v.is_finite() {
        v
    } else {
        UNREACHABLE
    }
}

pub fn order_gates(d: &[Vec<f32>]) -> Order {
    let n = d.len();
    let d: Vec<Vec<f32>> = d.iter().map(|r| r.iter().map(|v| sub(*v)).collect()).collect();
    let d = &d[..];
    if n < 2 {
        return Order { visit: (0..n).collect(), cost: 0.0, exact: true };
    }
    let m = n - 2; // the checkpoints between start (0) and finish (n-1)
    if m == 0 {
        return Order { visit: vec![0, n - 1], cost: d[0][n - 1], exact: true };
    }
    if m <= EXACT_MAX {
        return held_karp(d);
    }
    greedy_2opt(d)
}

fn held_karp(d: &[Vec<f32>]) -> Order {
    let n = d.len();
    let m = n - 2;
    let full = 1usize << m;
    // dp[mask][j] = best cost from start, having visited `mask`, now at cp j
    let mut dp = vec![f32::INFINITY; full * m];
    let mut par = vec![u16::MAX; full * m];
    for j in 0..m {
        dp[(1 << j) * m + j] = d[0][j + 1];
    }
    for mask in 1..full {
        for j in 0..m {
            if mask & (1 << j) == 0 {
                continue;
            }
            let base = dp[mask * m + j];
            if !base.is_finite() {
                continue;
            }
            for k in 0..m {
                if mask & (1 << k) != 0 {
                    continue;
                }
                let nm = mask | (1 << k);
                let nd = base + d[j + 1][k + 1];
                if nd < dp[nm * m + k] {
                    dp[nm * m + k] = nd;
                    par[nm * m + k] = j as u16;
                }
            }
        }
    }
    let last = full - 1;
    let mut best = f32::INFINITY;
    let mut bj = 0usize;
    for j in 0..m {
        let c = dp[last * m + j] + d[j + 1][n - 1];
        if c < best {
            best = c;
            bj = j;
        }
    }
    let mut seq = Vec::new();
    let (mut mask, mut j) = (last, bj);
    loop {
        seq.push(j + 1);
        let p = par[mask * m + j];
        if p == u16::MAX {
            break;
        }
        mask &= !(1 << j);
        j = p as usize;
    }
    seq.reverse();
    let mut visit = vec![0usize];
    visit.extend(seq);
    visit.push(n - 1);
    Order { visit, cost: best, exact: true }
}

fn greedy_2opt(d: &[Vec<f32>]) -> Order {
    let n = d.len();
    let mut left: Vec<usize> = (1..n - 1).collect();
    let mut visit = vec![0usize];
    let mut cur = 0usize;
    while !left.is_empty() {
        let (bi, _) = left
            .iter()
            .enumerate()
            .min_by(|a, b| d[cur][*a.1].partial_cmp(&d[cur][*b.1]).unwrap())
            .unwrap();
        cur = left.remove(bi);
        visit.push(cur);
    }
    visit.push(n - 1);
    let cost = |v: &Vec<usize>| -> f32 { v.windows(2).map(|w| d[w[0]][w[1]]).sum() };
    let mut best = cost(&visit);
    let mut improved = true;
    while improved {
        improved = false;
        for i in 1..visit.len() - 2 {
            for j in i + 1..visit.len() - 1 {
                let mut cand = visit.clone();
                cand[i..=j].reverse();
                let c = cost(&cand);
                if c < best - 1e-3 {
                    best = c;
                    visit = cand;
                    improved = true;
                }
            }
        }
    }
    Order { visit, cost: best, exact: false }
}
