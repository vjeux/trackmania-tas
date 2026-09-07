//! `mapgeom pack` — build a map's `MapPack` and `Route` and write them out.
//!
//! One command, one map, two JSON files and a transcript. Everything it prints
//! is either a measurement with its control beside it or is marked UNMEASURED.

use crate::pack::{gates, group, group_control, half_width, Checkpoint, MapPack, Route, Vertex};
use crate::probe::Index;
use crate::surf::{order_gates, Graph, Grid};
use tmmaps::map::MapFile;

/// Station spacing along the route, in metres.
pub const STATION: f32 = 20.0;

/// How far below a station a surface must be found for the station to count as
/// having ground under it.
pub const STATION_REACH: f32 = 3.0;

/// How far to the side the **negative** control probes.
///
/// A coverage check that only ever probes the route is decoration: a map whose
/// every square metre has some surface passes it. So the same probe is run at
/// each station displaced sideways by this much, where — on a road 25.6 m wide
/// — it should mostly find nothing.
pub const OFFSIDE: f32 = 40.0;

/// Douglas–Peucker over a 3D polyline, returning the indices kept, with a set
/// of indices that must survive whatever the tolerance says.
///
/// The pinned indices are the tour's gate vertices. Simplification is allowed
/// to move the *line*; it is not allowed to lose the places the route exists
/// to pass through.
pub fn douglas_peucker_keep(pts: &[[f32; 3]], tol: f32, pinned: &[usize]) -> Vec<usize> {
    if pts.len() < 3 {
        return (0..pts.len()).collect();
    }
    let mut cuts: Vec<usize> = vec![0, pts.len() - 1];
    cuts.extend(pinned.iter().copied().filter(|i| *i < pts.len()));
    cuts.sort_unstable();
    cuts.dedup();
    let mut keep: Vec<usize> = Vec::new();
    for w in cuts.windows(2) {
        let mut seg = dp(pts, w[0], w[1], tol);
        if !keep.is_empty() {
            seg.remove(0);
        }
        keep.extend(seg);
    }
    keep
}

fn dp(pts: &[[f32; 3]], a: usize, b: usize, tol: f32) -> Vec<usize> {
    if b <= a + 1 {
        return vec![a, b];
    }
    let (pa, pb) = (pts[a], pts[b]);
    let ab = [pb[0] - pa[0], pb[1] - pa[1], pb[2] - pa[2]];
    let l2 = ab[0] * ab[0] + ab[1] * ab[1] + ab[2] * ab[2];
    let mut worst = 0.0f32;
    let mut wi = a;
    for i in a + 1..b {
        let p = pts[i];
        let t = if l2 > 1e-9 {
            (((p[0] - pa[0]) * ab[0] + (p[1] - pa[1]) * ab[1] + (p[2] - pa[2]) * ab[2]) / l2)
                .clamp(0.0, 1.0)
        } else {
            0.0
        };
        let q = [pa[0] + ab[0] * t, pa[1] + ab[1] * t, pa[2] + ab[2] * t];
        let d = ((p[0] - q[0]).powi(2) + (p[1] - q[1]).powi(2) + (p[2] - q[2]).powi(2)).sqrt();
        if d > worst {
            worst = d;
            wi = i;
        }
    }
    if worst <= tol {
        return vec![a, b];
    }
    let mut left = dp(pts, a, wi, tol);
    let right = dp(pts, wi, b, tol);
    left.pop();
    left.extend(right);
    left
}

pub struct Built {
    pub pack: MapPack,
    /// Route length over the author time, m/s. Over ~95 is not physical.
    pub implied_speed: Option<f32>,
    /// Unit tangent of the route at its first vertex: the direction the car
    /// actually leaves the start in.
    pub start_dir: [f32; 3],
    /// Unit tangent at each gate in TOUR order -- the gate's normal, i.e. the
    /// direction the car crosses it in. Last entry is the finish.
    pub gate_dir: Vec<[f32; 3]>,
    /// Per-leg route length beside the straight-line chord it spans.
    pub legs: Vec<(String, f32, f32)>,
    pub route: Route,
    pub coverage: Coverage,
    pub notes: Vec<String>,
    /// The assembled scene, kept so a caller can draw the route through it.
    pub scene: crate::scene::Scene,
}

pub struct Coverage {
    pub stations: usize,
    /// Stations with ANY collidable surface beneath them.
    pub with_surface: usize,
    /// Stations with one of the map's own ROAD materials beneath them. This is
    /// the sharp version: "any surface" is nearly free on a map that has a
    /// stadium floor under everything.
    pub with_road: usize,
    pub offside_with_surface: usize,
    pub offside_with_road: usize,
    pub materials: Vec<(String, usize)>,
}

impl Coverage {
    pub fn frac(&self) -> f32 {
        self.over(self.with_surface)
    }
    pub fn road_frac(&self) -> f32 {
        self.over(self.with_road)
    }
    pub fn offside_frac(&self) -> f32 {
        self.over(self.offside_with_surface)
    }
    pub fn offside_road_frac(&self) -> f32 {
        self.over(self.offside_with_road)
    }
    fn over(&self, n: usize) -> f32 {
        if self.stations == 0 {
            0.0
        } else {
            n as f32 / self.stations as f32
        }
    }
}

pub struct Opts {
    pub with_deco: bool,
    pub verbose: bool,
    pub step: f32,
    /// Solve the tour from THIS world position instead of the map's `Spawn`
    /// waypoint.
    ///
    /// # Why this exists
    ///
    /// On Summer 2026 - 01 the dedicated server puts the validated car at
    /// (1360.00, 10.00, 1108.75), 389 m from the map's own `RoadTechStart`
    /// block at (1584, 16, 784). That is MEASURED — a memory sweep of all 2308
    /// mapped windows finds no other car, the trajectory self-check passes, and
    /// the plain oracle corroborates it (a straight run covering 848 m of path
    /// collects zero checkpoints, which is impossible from the map's start).
    /// Why the two differ is UNKNOWN and is a task.
    ///
    /// A route solved from the wrong origin has the wrong arc length, the wrong
    /// leg order and the wrong first gate, and every one of those is silently
    /// wrong: the geometry is self-consistent and the reward built on it is
    /// nonsense. So the caller who can measure where the car really is gets to
    /// say so. The map's own waypoint remains the default; nothing changes for
    /// a caller who does not pass this.
    pub spawn_override: Option<[f32; 3]>,
}

pub fn build(
    store: &mut crate::store::DataStore,
    path: &str,
    uid: &str,
    name: &str,
    author_ms: Option<i64>,
    o: &Opts,
) -> Result<Built, String> {
    let m = MapFile::load(std::path::Path::new(path));
    let mut notes: Vec<String> = Vec::new();

    // ---- 1. the map height, ghost-free -------------------------------------
    let mut asm = crate::assemble::Assembler::new(store);
    let _ = asm.with_embedded(&m);
    let (grid_scene, _free_scene) = asm.map_split(&m);
    let yrep = crate::yoff::measure(&m, &grid_scene);
    let yoff = match yrep.value() {
        Some(y) => y,
        None => return Err(format!("map height UNMEASURED: {}", yrep.line())),
    };
    if o.verbose {
        println!("{}", yrep.line());
    }
    drop(grid_scene);

    // ---- 2. the whole map at that height ------------------------------------
    //
    // Two scenes, deliberately:
    //
    // * `scene` — the map's own blocks and items. This is what the route grid
    //   is built from. **The decoration is left out of it on purpose.** A
    //   stadium's floor, stands and walkways are `Asphalt`, `Grass` and
    //   `ResonantMetal` in enormous quantity — on Japan 2026 the decoration
    //   makes `Asphalt` 47 % of every surface in the map — so learning the
    //   road from the material under a gate adopts the entire stadium and the
    //   graph becomes uniformly cheap. The decoration is never track.
    // * `full` — the same plus the decoration. This is what the station
    //   control probes against, because the control's job is to ask what the
    //   car could actually be standing on, and that does include a canopy.
    let mut asm = crate::assemble::Assembler::new(store);
    let _ = asm.with_embedded(&m);
    let scene = asm.map(&m, yoff, true);
    let mut full = crate::scene::Scene::default();
    full.append(&scene, &crate::geom::IDENTITY);
    if o.with_deco {
        if let Some((_p, d)) = asm.decoration(&m, yoff) {
            full.append(&d, &crate::geom::IDENTITY);
        }
    }
    if o.verbose {
        println!(
            "  scene {} triangles (track), {} with the decoration",
            scene.tri_count(),
            full.tri_count()
        );
    }

    // ---- 3. the gates -------------------------------------------------------
    let all = gates(&m, yoff);
    let ctrl = group_control(&all);
    let spawn_gates: Vec<_> = all.iter().filter(|g| g.tag == "Spawn").cloned().collect();
    let cp_gates: Vec<_> = all.iter().filter(|g| g.tag == "Checkpoint").cloned().collect();
    let goal_gates: Vec<_> = all.iter().filter(|g| g.tag == "Goal").cloned().collect();
    if spawn_gates.is_empty() {
        return Err("no Spawn waypoint in the map".into());
    }
    if goal_gates.is_empty() {
        return Err("no Goal waypoint in the map".into());
    }
    let checkpoints = group(&cp_gates);
    let finish = group(&goal_gates);
    // The tour's origin. The map's `Spawn` waypoint by default; an override
    // when the caller has MEASURED where the engine actually puts the car (see
    // `Opts::spawn_override`). An overridden origin is treated as item-placed:
    // it is an absolute world position already on the road, not a cell base.
    let spawn_from_item = o.spawn_override.is_some() || spawn_gates[0].from_item;
    let spawn = o.spawn_override.unwrap_or(spawn_gates[0].pos);
    let spawn_yaw = spawn_gates[0].yaw;
    if let Some(s) = o.spawn_override {
        notes.push(format!(
            "tour solved from a MEASURED spawn ({:.2}, {:.2}, {:.2}), {:.1} m from the map's own \
             Spawn waypoint at ({:.2}, {:.2}, {:.2})",
            s[0],
            s[1],
            s[2],
            ((s[0] - spawn_gates[0].pos[0]).powi(2)
                + (s[1] - spawn_gates[0].pos[1]).powi(2)
                + (s[2] - spawn_gates[0].pos[2]).powi(2))
            .sqrt(),
            spawn_gates[0].pos[0],
            spawn_gates[0].pos[1],
            spawn_gates[0].pos[2],
        ));
    }
    if finish.len() > 1 {
        notes.push(format!(
            "{} separate finish groups; the tour ends at the one it can reach most cheaply",
            finish.len()
        ));
    }

    // ---- 4. the drivable grid ----------------------------------------------
    let mut grid = Grid::build(&scene);
    let mut anchors: Vec<([f32; 3], f32, f32)> = vec![{
        let (lo, hi) = crate::pack::window(spawn_from_item);
        (spawn, lo, hi)
    }];
    for c in checkpoints.iter().chain(finish.iter()) {
        let (lo, hi) = c.window();
        anchors.push((c.pos, lo, hi));
    }
    let (road_materials, dropped_materials) = grid.learn_road(&anchors);
    for dm in &dropped_materials {
        let n = format!("material {} covers too much of the map to be track and was NOT adopted as road", dm);
        if o.verbose { println!("  {}", n); }
        notes.push(n);
    }
    if road_materials.is_empty() {
        return Err("no road material could be read under the spawn or any gate".into());
    }
    if o.verbose {
        println!(
            "  grid {}x{} cells, {} surfaces, {} of them road ({})",
            grid.nx,
            grid.nz,
            grid.surface_count(),
            grid.road_count(),
            road_materials.join(", ")
        );
    }
    let g = Graph::build_step(&grid, o.step);
    if o.verbose {
        println!("  graph {} nodes, {} edges", g.len(), g.edges.len());
    }

    // ---- 5. the waypoints as graph nodes ------------------------------------
    let mut wp: Vec<([f32; 3], bool)> = vec![(spawn, spawn_gates[0].from_item)];
    wp.extend(checkpoints.iter().map(|c| (c.pos, c.from_item())));
    let finish_first = wp.len();
    wp.extend(finish.iter().map(|c| (c.pos, c.from_item())));
    let mut nodes: Vec<usize> = Vec::new();
    for (i, (p, item)) in wp.iter().enumerate() {
        let (lo, hi) = crate::pack::window(*item);
        match g.nearest_window(&grid, *p, 12, lo, hi) {
            Some(n) => nodes.push(n),
            None => {
                return Err(format!(
                    "waypoint {} at ({:.1}, {:.1}, {:.1}) has no drivable surface in its search \
                     window within 24 m -- that is a gap in the model, not an absence in the map; \
                     `mapgeom where --at {:.1},{:.1}` says what the map puts there",
                    i, p[0], p[1], p[2], p[0], p[2]
                ))
            }
        }
    }

    // ---- 6. the distance matrix, and the legal order ------------------------
    // Several finish groups collapse to one waypoint: whichever is cheapest to
    // reach from the last checkpoint. Keeping them all in the tour would force
    // the car through every one of them, which is exactly wrong for a finish.
    let mut g = g;
    for note in g.bridge_components(&grid, &nodes, 240.0) {
        if o.verbose {
            println!("  {}", note);
        }
        notes.push(note);
    }
    let n_cp = checkpoints.len();
    let mut fields: Vec<(Vec<f32>, Vec<u32>)> = Vec::new();
    for n in &nodes {
        fields.push(g.dijkstra(*n));
    }
    let dim = 1 + n_cp + 1;
    let mut d = vec![vec![f32::INFINITY; dim]; dim];
    let fin_of = |k: usize| finish_first + k;
    // best finish, from every possible predecessor
    let mut best_fin = 0usize;
    let mut best_fin_cost = f32::INFINITY;
    for k in 0..finish.len() {
        let c: f32 = (0..=n_cp).map(|i| fields[i].0[nodes[fin_of(k)]]).fold(f32::INFINITY, f32::min);
        if c < best_fin_cost {
            best_fin_cost = c;
            best_fin = k;
        }
    }
    let fin_node = fin_of(best_fin);
    for i in 0..dim {
        let src = if i == dim - 1 { fin_node } else { i };
        for j in 0..dim {
            let dst = if j == dim - 1 { fin_node } else { j };
            d[i][j] = fields[src].0[nodes[dst]];
        }
    }
    if o.verbose {
        println!("  waypoint nodes and how much of the graph each can reach:");
        for (i, f) in fields.iter().enumerate() {
            let reach = f.0.iter().filter(|v| v.is_finite()).count();
            let p = wp[i].0;
            let nd = nodes[i];
            println!(
                "    {:>2} ({:7.0},{:6.0},{:7.0}) node y {:7.2} road {} reaches {} of {} nodes ({:.1} %)",
                i, p[0], p[1], p[2], g.node_y[nd], g.node_road[nd], reach, g.len(),
                100.0 * reach as f32 / g.len() as f32
            );
        }
        println!("  distance matrix (m), 0=start, last=finish:");
        for (i, row) in d.iter().enumerate() {
            let cells: Vec<String> = row
                .iter()
                .map(|v| if v.is_finite() { format!("{:8.0}", v) } else { "     inf".into() })
                .collect();
            let p = if i == dim - 1 { wp[fin_node].0 } else { wp[i].0 };
            println!("    {:>2} ({:7.0},{:6.0},{:7.0}) {}", i, p[0], p[1], p[2], cells.join(""));
        }
    }
    // A pair being unreachable is NOT a failure on its own. The edges are
    // asymmetric — a car drops off a ledge and cannot drive back up — so a
    // perfectly ordinary track has ordered pairs with no path, and demanding
    // that every pair be reachable rejects maps whose tour is fine. What must
    // be finite is the tour the solver actually chooses, and that is checked
    // after it chooses.
    let ord = order_gates(&d);
    {
        let mut bad: Vec<String> = Vec::new();
        for w in ord.visit.windows(2) {
            if !d[w[0]][w[1]].is_finite() {
                bad.push(format!("{} -> {}", w[0], w[1]));
            }
        }
        if !bad.is_empty() {
            return Err(format!(
                "no legal order exists: even the cheapest tour has legs with no path over the \
                 drivable graph ({}). The track crosses something the surface graph does not \
                 model -- a jump too wide for a leap edge, or a gate on geometry the reader is \
                 missing",
                bad.join(", ")
            ));
        }
    }

    // ---- 7. the polyline ----------------------------------------------------
    let mut chain: Vec<usize> = Vec::new();
    let mut gate_at: Vec<usize> = Vec::new(); // index into `chain`
    for w in ord.visit.windows(2) {
        let (a, b) = (w[0], w[1]);
        let src = if a == dim - 1 { fin_node } else { a };
        let dst = if b == dim - 1 { fin_node } else { b };
        let p = g.path(&fields[src].1, nodes[dst]);
        if chain.is_empty() {
            chain.extend(p);
        } else {
            chain.extend(p.into_iter().skip(1));
        }
        gate_at.push(chain.len() - 1);
    }

    let mut pts: Vec<[f32; 3]> = chain.iter().map(|n| g.world(&grid, *n)).collect();
    // Simplify. A grid path has a vertex every 2 m and a 45° run comes out as
    // a sawtooth of them; Douglas–Peucker at well under the corridor width
    // removes the sawtooth without moving the line anywhere that matters. The
    // gate vertices are pinned so the tour's own waypoints survive it.
    let simple = douglas_peucker_keep(&pts, 0.6, &gate_at);
    let mut kept: Vec<usize> = simple;
    kept.dedup();
    let gate_vi: Vec<usize> = gate_at
        .iter()
        .map(|gi| kept.iter().position(|k| k == gi).unwrap_or(kept.len() - 1))
        .collect();
    pts = kept.iter().map(|i| g.world(&grid, chain[*i])).collect();

    let mut verts: Vec<Vertex> = Vec::with_capacity(pts.len());
    let mut s = 0.0f32;
    for (i, p) in pts.iter().enumerate() {
        if i > 0 {
            let q = verts[i - 1].pos;
            s += ((p[0] - q[0]).powi(2) + (p[1] - q[1]).powi(2) + (p[2] - q[2]).powi(2)).sqrt();
        }
        let n = chain[kept[i]];
        verts.push(Vertex {
            pos: *p,
            s,
            half_width: half_width(&grid, &g, n, 20.0),
            material: grid.mats[g.node_mat[n] as usize].clone(),
            next_gate: 0,
        });
    }
    let gate_s: Vec<f32> = gate_vi.iter().map(|i| verts[*i].s).collect();
    // Leg lengths beside the straight-line gaps they span. A leg much longer
    // than its chord is either a real hairpin or a detour, and the two are
    // told apart by looking, not by assuming.
    let mut legs: Vec<(String, f32, f32)> = Vec::new();
    {
        let mut prev_s = 0.0f32;
        let mut prev_p = verts[0].pos;
        for (li, w) in ord.visit.windows(2).enumerate() {
            let s_here = gate_s[li];
            let p = verts[gate_vi[li]].pos;
            let chord = ((p[0] - prev_p[0]).powi(2)
                + (p[1] - prev_p[1]).powi(2)
                + (p[2] - prev_p[2]).powi(2))
            .sqrt();
            let label = |k: usize| -> String {
                if k == 0 {
                    "start".into()
                } else if k == dim - 1 {
                    "finish".into()
                } else {
                    format!("cp{}", k)
                }
            };
            legs.push((format!("{} -> {}", label(w[0]), label(w[1])), s_here - prev_s, chord));
            prev_s = s_here;
            prev_p = p;
        }
    }
    for v in verts.iter_mut() {
        v.next_gate = gate_s.iter().position(|x| *x >= v.s).unwrap_or(gate_s.len());
    }
    let length = verts.last().map(|v| v.s).unwrap_or(0.0);
    let mut stations: Vec<f32> = Vec::new();
    let mut t = 0.0f32;
    while t <= length {
        stations.push(t);
        t += STATION;
    }
    if stations.last().map_or(true, |l| (length - l).abs() > 1e-3) {
        stations.push(length);
    }

    // the checkpoint order, back in MapPack indices
    let order: Vec<usize> = ord.visit.iter().skip(1).take(n_cp).map(|i| i - 1).collect();

    // Headings, from the ROUTE rather than from the placement.
    //
    // A block placement gives a quarter-turn, and a quarter-turn does not say
    // which of the two directions along that axis the car goes: `dir = 0` and
    // `dir = 2` are the same line travelled opposite ways. The route knows,
    // because it arrives from somewhere. So the heading a consumer gets is the
    // route tangent, and the placement yaw is reported beside it as what the
    // map file said -- the two axes are named, never merged.
    let tangent_at = |vi: usize| -> [f32; 3] {
        let a = verts[vi.saturating_sub(3)].pos;
        let b = verts[(vi + 3).min(verts.len() - 1)].pos;
        let d = [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
        let l = (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt().max(1e-6);
        [d[0] / l, d[1] / l, d[2] / l]
    };
    let start_dir = tangent_at(0);
    let gate_dir: Vec<[f32; 3]> = gate_vi.iter().map(|i| tangent_at(*i)).collect();
    let route = Route {
        verts,
        gate_s,
        order,
        order_exact: ord.exact,
        length,
        stations,
    };

    // The physical sanity check, which is not a unit test and catches what
    // unit tests do not: a route length divided by the map's own author time.
    // A Stadium car does not average much over 95 m/s over a whole lap, so a
    // higher figure means the route is going somewhere the drive does not.
    // This is how the 2840 m route on a 23-second map was caught.
    let implied = author_ms.map(|ms| length / (ms as f32 / 1000.0));

    // ---- 8. the control ------------------------------------------------------
    //
    // The route was built from a 2 m grid of cell CENTRES over the track
    // blocks' triangles. The control re-asks the question a different way: a
    // plumb probe at each station's exact position, against the full triangle
    // index of the whole scene. It can fail where the route cut a corner
    // between two cell centres, or crossed a gap narrower than a cell.
    let idx = Index::build(&full, 32.0);
    let mut with_surface = 0usize;
    let mut with_road = 0usize;
    let mut offside = 0usize;
    let mut offside_road = 0usize;
    let mut mats: std::collections::BTreeMap<String, usize> = Default::default();
    let is_road = |m: &str| road_materials.iter().any(|r| r == m);
    for st in &route.stations {
        let p = route.at(*st);
        if let Some(h) = idx.below([p[0], p[1] + 0.5, p[2]], STATION_REACH) {
            with_surface += 1;
            if is_road(&h.material) {
                with_road += 1;
            }
            *mats.entry(h.material).or_insert(0) += 1;
        }
        // the negative half: the same probe, displaced sideways
        let (a, b) = (route.at((st - 5.0).max(0.0)), route.at((st + 5.0).min(length)));
        let (tx, tz) = (b[0] - a[0], b[2] - a[2]);
        let l = (tx * tx + tz * tz).sqrt().max(1e-6);
        let q = [p[0] - tz / l * OFFSIDE, p[1] + 0.5, p[2] + tx / l * OFFSIDE];
        if let Some(h) = idx.below(q, STATION_REACH) {
            offside += 1;
            if is_road(&h.material) {
                offside_road += 1;
            }
        }
    }
    let mut materials: Vec<(String, usize)> = mats.into_iter().collect();
    materials.sort_by(|a, b| b.1.cmp(&a.1));

    let coverage = Coverage {
        stations: route.stations.len(),
        with_surface,
        with_road,
        offside_with_surface: offside,
        offside_with_road: offside_road,
        materials,
    };

    Ok(Built {
        implied_speed: implied,
        scene: full,
        start_dir,
        gate_dir,
        legs,
        pack: MapPack {
            uid: uid.to_string(),
            name: name.to_string(),
            author_ms,
            yoff,
            spawn,
            spawn_yaw,
            checkpoints,
            finish,
            road_materials,
            group_control: ctrl,
        },
        route,
        coverage,
        notes,
    })
}

// ---------------------------------------------------------------------------
// JSON, hand-written: this crate has no serde and does not want one for six
// record shapes.
// ---------------------------------------------------------------------------

fn f(v: f32) -> String {
    format!("{:.3}", v)
}
fn v3(p: [f32; 3]) -> String {
    format!("[{}, {}, {}]", f(p[0]), f(p[1]), f(p[2]))
}
fn esc(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
}

fn cp_json(c: &Checkpoint) -> String {
    let gs: Vec<String> = c
        .gates
        .iter()
        .map(|g| {
            format!(
                r#"{{"name":"{}","pos":{},"yaw":{},"cell":[{},{},{}],"from_item":{}}}"#,
                esc(&g.name),
                v3(g.pos),
                f(g.yaw),
                g.cell.0,
                g.cell.1,
                g.cell.2,
                g.from_item
            )
        })
        .collect();
    format!(r#"{{"pos":{},"gates":[{}]}}"#, v3(c.pos), gs.join(","))
}

pub fn pack_json(b: &Built) -> String {
    let p = &b.pack;
    let cps: Vec<String> = p.order_checkpoints().iter().map(|c| cp_json(c)).collect();
    let fin: Vec<String> = p.finish.iter().map(cp_json).collect();
    let ctrl: Vec<String> = p
        .group_control
        .iter()
        .map(|(t, r, gr)| format!(r#"{{"tag":"{}","records":{},"groups":{}}}"#, esc(t), r, gr))
        .collect();
    format!(
        concat!(
            "{{\n",
            "  \"uid\": \"{}\",\n",
            "  \"name\": \"{}\",\n",
            "  \"author_ms\": {},\n",
            "  \"yoff\": {},\n",
            "  \"spawn\": {},\n",
            "  \"spawn_yaw\": {},\n",
            "  \"road_materials\": [{}],\n",
            "  \"checkpoints\": [{}],\n",
            "  \"finish\": [{}],\n",
            "  \"gate_group_control\": [{}],\n",
            "  \"start_dir\": {},\n",
            "  \"gate_dir_tour_order\": [{}],\n",
            "  \"ghost_reads\": 0\n",
            "}}\n"
        ),
        esc(&p.uid),
        esc(&p.name),
        p.author_ms.map(|v| v.to_string()).unwrap_or_else(|| "null".into()),
        f(p.yoff),
        v3(p.spawn),
        f(p.spawn_yaw),
        p.road_materials.iter().map(|m| format!("\"{}\"", esc(m))).collect::<Vec<_>>().join(","),
        cps.join(","),
        fin.join(","),
        ctrl.join(","),
        v3(b.start_dir),
        b.gate_dir.iter().map(|d| v3(*d)).collect::<Vec<_>>().join(",")
    )
}

pub fn route_json(b: &Built) -> String {
    let vs: Vec<String> = b
        .route
        .verts
        .iter()
        .map(|v| {
            format!(
                r#"{{"p":{},"s":{},"w":{},"m":"{}","g":{}}}"#,
                v3(v.pos),
                f(v.s),
                f(v.half_width),
                esc(&v.material),
                v.next_gate
            )
        })
        .collect();
    format!(
        concat!(
            "{{\n",
            "  \"uid\": \"{}\",\n",
            "  \"length_m\": {},\n",
            "  \"order\": {:?},\n",
            "  \"order_exact\": {},\n",
            "  \"gate_s\": [{}],\n",
            "  \"stations\": [{}],\n",
            "  \"station_coverage\": {{\"stations\": {}, \"with_surface\": {}, ",
            "\"offside_with_surface\": {}}},\n",
            "  \"verts\": [\n    {}\n  ]\n",
            "}}\n"
        ),
        esc(&b.pack.uid),
        f(b.route.length),
        b.route.order,
        b.route.order_exact,
        b.route.gate_s.iter().map(|s| f(*s)).collect::<Vec<_>>().join(","),
        b.route.stations.iter().map(|s| f(*s)).collect::<Vec<_>>().join(","),
        b.coverage.stations,
        b.coverage.with_surface,
        b.coverage.offside_with_surface,
        vs.join(",\n    ")
    )
}

impl MapPack {
    /// Checkpoints in the order the route takes them, which is set after the
    /// route is built. Until then they are in file order.
    pub fn order_checkpoints(&self) -> Vec<Checkpoint> {
        self.checkpoints.clone()
    }
}
