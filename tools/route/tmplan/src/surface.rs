//! The surface model a planner can ask distances of — the cartographer's grid
//! and graph (`mapgeom::surf`), built over the FULL checkpoint set of a
//! `gates.json` (linked groups included), plus the full-scene plumb index for
//! characterising what lies between two gates.

use mapgeom::probe::Index;
use mapgeom::surf::{Graph, Grid};
use tmroute::gates::{GatesFile, WpKind};

pub struct SurfaceModel {
    pub yoff: f32,
    pub grid: Grid,
    pub graph: Graph,
    /// Plumb index over the whole assembled scene (track + decoration).
    pub full: Index,
    pub road_materials: Vec<String>,
    pub notes: Vec<String>,
}

/// The planner's nodes: index 0 = spawn, then every checkpoint group in
/// `gates.checkpoint_group_ids()` order, then every finish group.
pub struct Nodes {
    pub groups: Vec<u32>, // u32::MAX for the spawn
    pub kinds: Vec<WpKind>,
    pub pos: Vec<[f32; 3]>,
    pub graph_node: Vec<Option<usize>>,
    pub n_cp: usize,
    pub n_fin: usize,
    /// The car's facing at the spawn (unit XZ): a first leg that departs against it is a U-turn from standstill.
    /// 9 of 14 τ < 0.4 honest maps (F22) were the human tour driven BACKWARDS — a symmetric cost matrix cannot
    /// tell the two directions apart; the spawn heading can.
    pub spawn_dir: [f32; 2],
}

impl Nodes {
    pub fn from_gates(g: &GatesFile) -> Nodes {
        let mut groups = vec![u32::MAX];
        let mut kinds = vec![WpKind::Start];
        let mut pos = vec![g.spawn.pos];
        let cps = g.checkpoint_group_ids();
        let fins = g.finish_group_ids();
        for grp in cps.iter().chain(fins.iter()) {
            let (c, _n, _hw) = g.group_geometry(*grp).unwrap();
            let rep = g.group_rep(*grp).unwrap();
            groups.push(*grp);
            kinds.push(rep.kind);
            // anchor at the road surface: centre.y is surface + half_height
            pos.push([c[0], c[1] - rep.half_height, c[2]]);
        }
        let n = groups.len();
        Nodes { groups, kinds, pos, graph_node: vec![None; n], n_cp: cps.len(), n_fin: fins.len(), spawn_dir: [g.spawn.yaw.sin(), g.spawn.yaw.cos()] }
    }
    pub fn finish_range(&self) -> std::ops::Range<usize> {
        1 + self.n_cp..1 + self.n_cp + self.n_fin
    }
}

/// Item / free gates carry an absolute road-level position: look DOWN a little.
/// Grid gates' anchor is `cell base + 2` (the road): look up into the cell too.
fn window(from_item: bool) -> (f32, f32) {
    // +2.5 above: the tiny converter's gate items sit 1 m under their own road surface
    if from_item { (-6.0, 2.5) } else { (-2.5, 7.0) }
}

impl SurfaceModel {
    /// `with_deco`: probe index includes the decoration (the cartographer's rule:
    /// route grid from the track only, station control against everything).
    /// `deco_grid`: build the ROUTE grid from track + decoration instead of the
    /// track alone (the cartographer's fallback rule, INTERFACE.md §7b: only when
    /// the track-only grid produced no route — Summer 2026 - 05 and 10 needed it).
    pub fn build(map: &std::path::Path, gates: &GatesFile, verbose: bool, deco_grid: bool) -> Result<(SurfaceModel, Nodes), String> {
        let paths = crate::pak_paths()?;
        let mut store = mapgeom::store::DataStore::open(&paths, mapgeom::store::STADIUM_KEY)?;
        let m = tmmaps::map::MapFile::load(map);
        let mut notes = Vec::new();

        let mut asm = mapgeom::assemble::Assembler::new(&mut store);
        let _ = asm.with_embedded(&m);
        let (grid_scene, _) = asm.map_split(&m);
        let yrep = mapgeom::yoff::measure(&m, &grid_scene);
        let yoff = match yrep.value() {
            Some(y) => y,
            None => {
                notes.push(format!("mapgeom yoff UNMEASURED ({}); using gates.json yoff {}", yrep.line(), gates.yoff));
                gates.yoff
            }
        };
        if (yoff - gates.yoff).abs() > 0.5 {
            notes.push(format!("yoff disagreement: mapgeom {} vs gates.json cellmode {}", yoff, gates.yoff));
        }
        drop(grid_scene);

        let mut asm = mapgeom::assemble::Assembler::new(&mut store);
        let _ = asm.with_embedded(&m);
        let scene = asm.map(&m, yoff, true);
        let mut full = mapgeom::scene::Scene::default();
        full.append(&scene, &mapgeom::geom::IDENTITY);
        if let Some((_p, d)) = asm.decoration(&m, yoff) {
            full.append(&d, &mapgeom::geom::IDENTITY);
        }
        if verbose {
            eprintln!("  scene {} triangles (track), {} with decoration, yoff {}", scene.tri_count(), full.tri_count(), yoff);
        }

        let mut nodes = Nodes::from_gates(gates);
        // the grid covers the TRACK (spawn + gates ± 40 % of their span, ≥ 400 m), not every placement on the map
        let clip = {
            let mut lo = [f32::INFINITY; 3];
            let mut hi = [f32::NEG_INFINITY; 3];
            for p in &nodes.pos { for a in 0..3 { lo[a] = lo[a].min(p[a]); hi[a] = hi[a].max(p[a]); } }
            let m = [0usize, 2].iter().map(|&a| (hi[a] - lo[a]) * 0.4).fold(400.0f32, f32::max);
            ([lo[0] - m, lo[1], lo[2] - m], [hi[0] + m, hi[1], hi[2] + m])
        };
        let span = (clip.1[0] - clip.0[0]).max(clip.1[2] - clip.0[2]);
        if span > 2600.0 {
            return Err(format!("track spans {span:.0} m — too large for the 2 m surface grid (NOSEDIVE class); no surface model"));
        }
        let mut grid = Grid::build_within(if deco_grid { &full } else { &scene }, Some(clip));
        if deco_grid {
            notes.push("route grid built from track + DECORATION (fallback; the track-only grid had no route)".into());
        }
        let mut anchors: Vec<([f32; 3], f32, f32)> = Vec::new();
        for i in 0..nodes.pos.len() {
            let from_item = if i == 0 {
                gates.gates.iter().find(|g| g.kind == WpKind::Start).map_or(false, |g| g.from_item)
            } else {
                gates.group_rep(nodes.groups[i]).map_or(false, |g| g.from_item)
            };
            let (lo, hi) = window(from_item);
            anchors.push((nodes.pos[i], lo, hi));
        }
        let (road_materials, dropped) = grid.learn_road(&anchors);
        for d in dropped {
            notes.push(format!("material {d} covers too much of the map to be track; not adopted as road"));
        }
        if road_materials.is_empty() {
            return Err("no road material under the spawn or any gate".into());
        }
        let mut graph = Graph::build_step(&grid, mapgeom::surf::STEP);
        if verbose {
            eprintln!("  grid {}x{} cells, {} surfaces, {} road ({}); graph {} nodes {} edges", grid.nx, grid.nz, grid.surface_count(), grid.road_count(), road_materials.join(", "), graph.len(), graph.edges.len());
        }
        let mut found = Vec::new();
        for (i, (p, lo, hi)) in anchors.iter().enumerate() {
            nodes.graph_node[i] = graph.nearest_window(&grid, *p, 12, *lo, *hi);
            match nodes.graph_node[i] {
                Some(n) => found.push(n),
                None => notes.push(format!("node {i} (group {}) at ({:.1}, {:.1}, {:.1}) has no drivable surface within 24 m of its window — reader gap", nodes.groups[i], p[0], p[1], p[2])),
            }
        }
        for n in graph.bridge_components(&grid, &found, 240.0) {
            notes.push(n);
        }
        let full_idx = Index::build(&full, 4.0);
        Ok((SurfaceModel { yoff, grid, graph, full: full_idx, road_materials, notes }, nodes))
    }

    /// Distance matrices over the nodes: `(cost, length, fields)`. `cost` is the
    /// surface-graph Dijkstra cost (off-road cells count 20×, leaps 4×; INFINITY =
    /// no path); `length` is the traced path's true metres — the number a speed
    /// model may divide by. The cartographer ordered by cost and reported length;
    /// the two differ by up to 4× on a leg that crosses grass.
    pub fn distance_matrix(&self, nodes: &Nodes) -> (Vec<Vec<f32>>, Vec<Vec<f32>>, Vec<Option<(Vec<f32>, Vec<u32>)>>) {
        let (d, len, _drop, fields) = self.distance_matrix_full(nodes);
        (d, len, fields)
    }

    /// `(cost, length, drop, fields)` — `drop[i][j]` = metres of step-downs > 1.6 m along the traced path.
    pub fn distance_matrix_full(&self, nodes: &Nodes) -> (Vec<Vec<f32>>, Vec<Vec<f32>>, Vec<Vec<f32>>, Vec<Option<(Vec<f32>, Vec<u32>)>>) {
        let n = nodes.pos.len();
        let mut fields: Vec<Option<(Vec<f32>, Vec<u32>)>> = Vec::with_capacity(n);
        for i in 0..n {
            fields.push(nodes.graph_node[i].map(|g| self.graph.dijkstra(g)));
        }
        let mut d = vec![vec![f32::INFINITY; n]; n];
        let mut len = vec![vec![f32::INFINITY; n]; n];
        let mut drop = vec![vec![f32::NAN; n]; n];
        for i in 0..n {
            let Some((dist, prev)) = &fields[i] else { continue };
            for j in 0..n {
                if let Some(gj) = nodes.graph_node[j] {
                    d[i][j] = dist[gj];
                    if dist[gj].is_finite() {
                        let path = self.graph.path(prev, gj);
                        let mut l = 0.0f32;
                        let mut dr = 0.0f32;
                        for w in path.windows(2) {
                            let a = self.graph.world(&self.grid, w[0]);
                            let b = self.graph.world(&self.grid, w[1]);
                            l += ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2) + (a[2] - b[2]).powi(2)).sqrt();
                            if b[1] - a[1] < -1.6 {
                                dr += a[1] - b[1];
                            }
                        }
                        len[i][j] = l;
                        drop[i][j] = dr;
                    }
                }
            }
        }
        (d, len, drop, fields)
    }

    /// The surface-graph path from node i to node j as world points.
    pub fn path_points(&self, nodes: &Nodes, fields: &[Option<(Vec<f32>, Vec<u32>)>], i: usize, j: usize) -> Option<Vec<[f32; 3]>> {
        let (_, prev) = fields[i].as_ref()?;
        let gj = nodes.graph_node[j]?;
        if !fields[i].as_ref().unwrap().0[gj].is_finite() {
            return None;
        }
        Some(self.graph.path(prev, gj).into_iter().map(|n| self.graph.world(&self.grid, n)).collect())
    }

    /// Corridor half-width at a world point (nearest graph node's road extent), capped.
    pub fn half_width_at(&self, p: [f32; 3], cap: f32) -> f32 {
        match self.graph.nearest_window(&self.grid, p, 2, -3.0, 3.0) {
            Some(n) => mapgeom::pack::half_width(&self.grid, &self.graph, n, cap),
            None => 6.0,
        }
    }

    /// What lies along the straight chord from `a` to `b`: one row per `step`
    /// metres — the chord height, the highest surface at or below it within
    /// `reach`, that surface's material — and the gaps.
    pub fn chord_profile(&self, a: [f32; 3], b: [f32; 3], step: f32, reach: f32) -> ChordProfile {
        let dx = b[0] - a[0];
        let dz = b[2] - a[2];
        let horiz = (dx * dx + dz * dz).sqrt();
        let n = (horiz / step).ceil().max(1.0) as usize;
        let mut rows = Vec::with_capacity(n + 1);
        for k in 0..=n {
            let t = k as f32 / n as f32;
            let x = a[0] + dx * t;
            let z = a[2] + dz * t;
            let y = a[1] + (b[1] - a[1]) * t;
            // highest surface at or below chord + 3 m within reach
            let col = self.full.column(x, z);
            let hit = col.iter().find(|(sy, _)| *sy <= y + 3.0 && *sy >= y - reach).cloned();
            rows.push(ChordRow { t, pos: [x, y, z], surface: hit });
        }
        let mut gaps = Vec::new();
        let mut cur: Option<(f32, f32)> = None;
        for r in &rows {
            match (&r.surface, &mut cur) {
                (None, None) => cur = Some((r.t * horiz, r.t * horiz)),
                (None, Some(c)) => c.1 = r.t * horiz,
                (Some(_), Some(c)) => {
                    gaps.push(*c);
                    cur = None;
                }
                _ => {}
            }
        }
        if let Some(c) = cur {
            gaps.push(c);
        }
        ChordProfile { horiz, dy: b[1] - a[1], rows, gaps }
    }
}

#[derive(Clone, Debug)]
pub struct ChordRow {
    pub t: f32,
    pub pos: [f32; 3],
    /// (surface y, material) — None = nothing under the chord within reach.
    pub surface: Option<(f32, String)>,
}

#[derive(Clone, Debug)]
pub struct ChordProfile {
    pub horiz: f32,
    pub dy: f32,
    pub rows: Vec<ChordRow>,
    /// (start_m, end_m) along the chord with nothing beneath.
    pub gaps: Vec<(f32, f32)>,
}

impl ChordProfile {
    pub fn longest_gap(&self) -> f32 {
        self.gaps.iter().map(|(s, e)| e - s).fold(0.0, f32::max)
    }
    /// Materials in order of first appearance along the chord, with the share of samples each covers.
    pub fn materials(&self) -> Vec<(String, f32)> {
        let mut out: Vec<(String, usize)> = Vec::new();
        for r in &self.rows {
            let key = r.surface.as_ref().map(|s| s.1.clone()).unwrap_or_else(|| "NOTHING".into());
            if let Some(e) = out.iter_mut().find(|e| e.0 == key) {
                e.1 += 1;
            } else {
                out.push((key, 1));
            }
        }
        let n = self.rows.len().max(1) as f32;
        out.into_iter().map(|(k, c)| (k, c as f32 / n)).collect()
    }
}

impl SurfaceModel {
    /// Unit XZ directions of the surface path i→j at its two ends: `out[i][j]`
    /// = the first ~10 m leaving i, `inn[i][j]` = the last ~10 m arriving at j.
    /// NaN where there is no path. Lets the estimator charge a TURN at a gate:
    /// the angle between how the car arrives (prev→from) and how it must leave
    /// (from→to).
    pub fn directions(&self, nodes: &Nodes, fields: &[Option<(Vec<f32>, Vec<u32>)>]) -> (Vec<Vec<[f32; 2]>>, Vec<Vec<[f32; 2]>>) {
        let n = nodes.pos.len();
        let nan = [f32::NAN, f32::NAN];
        let mut out = vec![vec![nan; n]; n];
        let mut inn = vec![vec![nan; n]; n];
        let dir = |a: [f32; 3], b: [f32; 3]| -> [f32; 2] {
            let dx = b[0] - a[0];
            let dz = b[2] - a[2];
            let l = (dx * dx + dz * dz).sqrt();
            if l < 1e-3 { nan } else { [dx / l, dz / l] }
        };
        for i in 0..n {
            for j in 0..n {
                if i == j { continue; }
                let Some(p) = self.path_points(nodes, fields, i, j) else { continue };
                if p.len() < 2 { continue; }
                // ~10 m from each end
                let mut k = 0;
                let mut acc = 0.0;
                while k + 1 < p.len() && acc < 10.0 {
                    acc += ((p[k + 1][0] - p[k][0]).powi(2) + (p[k + 1][2] - p[k][2]).powi(2)).sqrt();
                    k += 1;
                }
                out[i][j] = dir(p[0], p[k]);
                let mut k2 = p.len() - 1;
                acc = 0.0;
                while k2 > 0 && acc < 10.0 {
                    acc += ((p[k2][0] - p[k2 - 1][0]).powi(2) + (p[k2][2] - p[k2 - 1][2]).powi(2)).sqrt();
                    k2 -= 1;
                }
                inn[i][j] = dir(p[k2], p[p.len() - 1]);
            }
        }
        (out, inn)
    }
}

impl SurfaceModel {
    /// Metres of DROP along the surface path i→j taken through drop edges — steps
    /// down of more than the climbable 1.6 m (`mapgeom::surf`'s up limit). A route
    /// with 0 here is driven; one with 30 m of it falls off things.
    pub fn path_drop(&self, nodes: &Nodes, fields: &[Option<(Vec<f32>, Vec<u32>)>], i: usize, j: usize) -> f32 {
        let Some(p) = self.path_points(nodes, fields, i, j) else { return f32::NAN };
        let mut d = 0.0f32;
        for w in p.windows(2) {
            let dy = w[1][1] - w[0][1];
            if dy < -1.6 {
                d += -dy;
            }
        }
        d
    }
}

impl SurfaceModel {
    /// For every (i, j): the indices into `gates.specials` whose centre lies
    /// within `half_width + 6` m (XZ) and 8 m (Y) of the surface path i→j, in
    /// path order. The planner reads a TRANSFORMATION gate here as "the rest of
    /// this leg is driven by that car" and a booster as a speed change.
    pub fn leg_specials(&self, nodes: &Nodes, fields: &[Option<(Vec<f32>, Vec<u32>)>], gates: &GatesFile) -> Vec<Vec<Vec<usize>>> {
        let n = nodes.pos.len();
        let mut out = vec![vec![Vec::new(); n]; n];
        if gates.specials.is_empty() {
            return out;
        }
        for i in 0..n {
            for j in 0..n {
                if i == j { continue; }
                let Some(p) = self.path_points(nodes, fields, i, j) else { continue };
                let mut found: Vec<(usize, usize)> = Vec::new(); // (path index, special index)
                for (si, s) in gates.specials.iter().enumerate() {
                    let r = s.half_width + 6.0;
                    let mut best: Option<usize> = None;
                    for (k, q) in p.iter().enumerate() {
                        let dx = q[0] - s.centre[0];
                        let dz = q[2] - s.centre[2];
                        if dx * dx + dz * dz <= r * r && (q[1] - s.centre[1]).abs() <= 8.0 {
                            best = Some(k);
                            break;
                        }
                    }
                    if let Some(k) = best {
                        found.push((k, si));
                    }
                }
                found.sort();
                out[i][j] = found.into_iter().map(|(_, si)| si).collect();
            }
        }
        out
    }
}
