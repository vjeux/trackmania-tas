//! `MapPack` and `Route` — everything downstream needs about a map, and
//! nothing that came from a recording.
//!
//! A `MapPack` is the map's own declared facts, placed in world space: the
//! author time out of the file's header, the spawn, the checkpoint gates in
//! legal order, the finish gates, and what the car will be driving on. A
//! `Route` is a polyline through them with arc length, a corridor and a
//! `progress()` call.
//!
//! Both are **read-only for everyone downstream** and both are derived from
//! the `.Map.Gbx` and the game's own data pack. No ghost is read, at any point,
//! for any purpose — not as a reference line, not to fit the map height, not
//! to check the answer.

use crate::surf::{Graph, Grid};
use tmmaps::map::{Kind, MapFile};

// ---------------------------------------------------------------------------
// gates
// ---------------------------------------------------------------------------

#[derive(Clone, Debug)]
pub struct Gate {
    pub name: String,
    pub tag: String,
    /// Where the gate sits, in world metres.
    pub pos: [f32; 3],
    /// The gate's facing, from the placement's own yaw. This is the axis of
    /// the block or item, **not** the direction a car goes through it — those
    /// differ by a sign nothing in the map file settles. The route's tangent
    /// at the gate is the one that knows, and `Route` carries it.
    pub yaw: f32,
    pub cell: (i32, i32, i32),
    /// A block-placed gate or an item-placed one.
    pub from_item: bool,
}

/// A checkpoint: **one or more gates that all fire for it.**
///
/// Several maps put a whole row of gates across a wide road and every one of
/// them counts the same checkpoint — Summer 2026 - 15 has sixteen goal
/// records and one finish. Treating each record as its own checkpoint gives a
/// tour that visits the same place sixteen times and an order that cannot be
/// right.
#[derive(Clone, Debug)]
pub struct Checkpoint {
    pub gates: Vec<Gate>,
    /// The centroid of the group — what the tour routes through.
    pub pos: [f32; 3],
}

/// How close two gates of the same tag must be to be the same checkpoint.
///
/// A gate structure spans its road: `GateCheckpointLeft32m` is a 32 m block,
/// and a row of them covers a road two or three cells wide. Two genuinely
/// different checkpoints on a campaign map are far further apart than that.
/// The threshold is checked, not assumed — `pack --gates` prints every gate
/// and the group it landed in, and the grouping's own control is in
/// `group_control`.
pub const GROUP_XZ: f32 = 34.0;
pub const GROUP_Y: f32 = 6.0;

/// The vertical window to look in for the surface a waypoint sits on.
///
/// A grid-placed gate's position is its cell BASE and its road is inside the
/// cell above; an item-placed gate's position is absolute and already on the
/// road. See `surf::Graph::nearest_window`.
pub fn window(from_item: bool) -> (f32, f32) {
    if from_item {
        (-6.0, 0.5)
    } else {
        (-0.5, 9.0)
    }
}

impl Checkpoint {
    /// Whether this group's position came from an absolute placement.
    pub fn from_item(&self) -> bool {
        self.gates.iter().all(|g| g.from_item)
    }
    pub fn window(&self) -> (f32, f32) {
        window(self.from_item())
    }
}

/// Every waypoint the map declares, placed in world space.
///
/// A grid-placed gate is located at its **cell centre**, which is right to
/// within half a cell — and half a cell is well inside "a route good to a few
/// metres". An item-placed or free-block gate carries an absolute position and
/// is used exactly.
pub fn gates(m: &MapFile, yoff: f32) -> Vec<Gate> {
    let mut out = Vec::new();
    for w in m.waypoints() {
        let (pos, from_item) = match (w.pos, &w.kind) {
            // An item, or a FREE block: an absolute position in the file.
            (Some(p), _) => (p, w.kind == Kind::Item),
            // A grid block: its cell, centred.
            (None, _) => (
                [
                    32.0 * w.coords.0 as f32 + 16.0,
                    8.0 * w.coords.1 as f32 + yoff,
                    32.0 * w.coords.2 as f32 + 16.0,
                ],
                false,
            ),
        };
        out.push(Gate {
            name: w.name.clone(),
            tag: w.tag.clone(),
            pos,
            yaw: w.yaw.unwrap_or(0.0),
            cell: w.coords,
            from_item,
        });
    }
    out
}

/// Group gates of one tag into checkpoints.
pub fn group(gs: &[Gate]) -> Vec<Checkpoint> {
    let n = gs.len();
    let mut parent: Vec<usize> = (0..n).collect();
    fn find(p: &mut Vec<usize>, a: usize) -> usize {
        let mut a = a;
        while p[a] != a {
            p[a] = p[p[a]];
            a = p[a];
        }
        a
    }
    for i in 0..n {
        for j in i + 1..n {
            let (a, b) = (&gs[i], &gs[j]);
            let dx = a.pos[0] - b.pos[0];
            let dz = a.pos[2] - b.pos[2];
            let dy = (a.pos[1] - b.pos[1]).abs();
            if (dx * dx + dz * dz).sqrt() <= GROUP_XZ && dy <= GROUP_Y {
                let (ra, rb) = (find(&mut parent, i), find(&mut parent, j));
                if ra != rb {
                    parent[ra] = rb;
                }
            }
        }
    }
    let mut buckets: std::collections::BTreeMap<usize, Vec<Gate>> = Default::default();
    for i in 0..n {
        let r = find(&mut parent, i);
        buckets.entry(r).or_default().push(gs[i].clone());
    }
    buckets
        .into_values()
        .map(|gates| {
            let k = gates.len() as f32;
            let mut c = [0.0f32; 3];
            for g in &gates {
                for a in 0..3 {
                    c[a] += g.pos[a] / k;
                }
            }
            Checkpoint { gates, pos: c }
        })
        .collect()
}

/// The two-sided control on the grouping.
///
/// * **positive** — a map whose gate records are already one per checkpoint
///   must come back with the same count. Over-merging would show here.
/// * **negative** — a map with a row of gates across one road must come back
///   with **one** checkpoint, not one per record. A grouping that never merges
///   passes the first half on its own, so the first half alone is decoration.
///
/// Returns `(records, groups)` per tag so a report can show both halves side
/// by side across a campaign, where both cases are present.
pub fn group_control(gs: &[Gate]) -> Vec<(String, usize, usize)> {
    let mut tags: Vec<String> = gs.iter().map(|g| g.tag.clone()).collect();
    tags.sort();
    tags.dedup();
    tags.into_iter()
        .map(|t| {
            let sub: Vec<Gate> = gs.iter().filter(|g| g.tag == t).cloned().collect();
            let n = sub.len();
            (t, n, group(&sub).len())
        })
        .collect()
}

// ---------------------------------------------------------------------------
// the pack
// ---------------------------------------------------------------------------

pub struct MapPack {
    pub uid: String,
    pub name: String,
    pub author_ms: Option<i64>,
    pub yoff: f32,
    pub spawn: [f32; 3],
    pub spawn_yaw: f32,
    pub checkpoints: Vec<Checkpoint>,
    pub finish: Vec<Checkpoint>,
    pub road_materials: Vec<String>,
    pub group_control: Vec<(String, usize, usize)>,
}

// ---------------------------------------------------------------------------
// the route
// ---------------------------------------------------------------------------

pub struct Vertex {
    pub pos: [f32; 3],
    /// Arc length from the start, in metres.
    pub s: f32,
    /// How far from this vertex the road stops, laterally.
    pub half_width: f32,
    pub material: String,
    /// Index into `Route::gate_s` of the next gate that must still be taken.
    pub next_gate: usize,
}

pub struct Route {
    pub verts: Vec<Vertex>,
    /// Arc length at each ordered checkpoint, then the finish.
    pub gate_s: Vec<f32>,
    /// The order the checkpoints are taken in, as indices into
    /// `MapPack::checkpoints`.
    pub order: Vec<usize>,
    pub order_exact: bool,
    pub length: f32,
    /// `s` every ~20 m.
    pub stations: Vec<f32>,
}

/// The maximum lateral distance from the route's centre that still counts as
/// on the track, when the local half-width is unknown.
pub const CORRIDOR_FLOOR: f32 = 6.0;

impl Route {
    pub fn station_pos(&self, s: f32) -> [f32; 3] {
        self.at(s)
    }

    /// The point on the route at arc length `s`.
    pub fn at(&self, s: f32) -> [f32; 3] {
        if self.verts.is_empty() {
            return [0.0; 3];
        }
        if self.verts.len() == 1 {
            return self.verts[0].pos;
        }
        let i = match self.verts.binary_search_by(|v| v.s.partial_cmp(&s).unwrap()) {
            Ok(i) => i.max(1),
            Err(i) => i.max(1).min(self.verts.len() - 1),
        };
        let (a, b) = (&self.verts[i - 1], &self.verts[i]);
        let d = (b.s - a.s).max(1e-6);
        let t = ((s - a.s) / d).clamp(0.0, 1.0);
        [
            a.pos[0] + (b.pos[0] - a.pos[0]) * t,
            a.pos[1] + (b.pos[1] - a.pos[1]) * t,
            a.pos[2] + (b.pos[2] - a.pos[2]) * t,
        ]
    }

    /// **The call agent C makes.** Where along the route a car is, how far off
    /// the centre, and whether it is still on the track.
    ///
    /// `lateral` is the 3D distance to the nearest point of the polyline;
    /// `on_route` compares it with the local corridor half-width, which is
    /// measured from the map's own road edge rather than assumed.
    pub fn progress(&self, p: [f32; 3]) -> (f32, f32, bool) {
        let mut best = (f32::INFINITY, 0.0f32, CORRIDOR_FLOOR);
        for w in self.verts.windows(2) {
            let (a, b) = (&w[0], &w[1]);
            let ab = [b.pos[0] - a.pos[0], b.pos[1] - a.pos[1], b.pos[2] - a.pos[2]];
            let l2 = ab[0] * ab[0] + ab[1] * ab[1] + ab[2] * ab[2];
            let t = if l2 > 1e-9 {
                (((p[0] - a.pos[0]) * ab[0] + (p[1] - a.pos[1]) * ab[1] + (p[2] - a.pos[2]) * ab[2])
                    / l2)
                    .clamp(0.0, 1.0)
            } else {
                0.0
            };
            let q = [
                a.pos[0] + ab[0] * t,
                a.pos[1] + ab[1] * t,
                a.pos[2] + ab[2] * t,
            ];
            let d = ((p[0] - q[0]).powi(2) + (p[1] - q[1]).powi(2) + (p[2] - q[2]).powi(2)).sqrt();
            if d < best.0 {
                best = (d, a.s + (b.s - a.s) * t, a.half_width.max(b.half_width));
            }
        }
        (best.1, best.0, best.0 <= best.2.max(CORRIDOR_FLOOR))
    }

    /// The index of the next gate still owed at arc length `s`.
    pub fn next_gate(&self, s: f32) -> usize {
        self.gate_s.iter().position(|g| *g > s).unwrap_or(self.gate_s.len())
    }
}

/// The local half-width of the road: how far from `(x, z)` you can go before
/// the map's own road material runs out, at roughly this height.
pub fn half_width(g: &Grid, gr: &Graph, node: usize, cap: f32) -> f32 {
    let ci = gr.node_cell[node] as usize;
    let (ix, iz) = (ci % g.nx, ci / g.nx);
    let y = gr.node_y[node];
    let rings = (cap / crate::surf::CELL).ceil() as i32;
    for r in 1..=rings {
        // Any cell on this ring with no road surface near this height ends it.
        for dz in -r..=r {
            for dx in -r..=r {
                if dx.abs() != r && dz.abs() != r {
                    continue;
                }
                let (jx, jz) = (ix as i32 + dx, iz as i32 + dz);
                if jx < 0 || jz < 0 || jx >= g.nx as i32 || jz >= g.nz as i32 {
                    return (r as f32 - 1.0) * crate::surf::CELL;
                }
                let c = &g.cells[jz as usize * g.nx + jx as usize];
                if !c.iter().any(|s| s.road && (s.y - y).abs() <= 4.0) {
                    return (r as f32 - 1.0) * crate::surf::CELL;
                }
            }
        }
    }
    cap
}
