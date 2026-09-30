//! The track, as the policy and the reward see it.
//!
//! Wraps `mapgeom`'s `Route` with the three things the environment needs and
//! the route does not provide directly:
//!
//! * a **signed lateral offset and a height above the road**, split apart.
//!   `Route::progress` returns a 3-D distance to the polyline, which is the
//!   right number for pruning and the wrong one for an off-route test: a car
//!   twenty metres up a jump reads as twenty metres off line while doing
//!   exactly the right thing (RULES-traps §5).
//! * **gate positions taken from the route**, not from the gate records. A
//!   grid-placed gate's `pos.y` is its CELL BASE and the road inside that cell
//!   is about 2 m above it (B's INTERFACE §2). `route.at(gate_s[i])` is the
//!   road surface at the gate, derived from the same geometry, and needs no
//!   correction.
//! * **saturating progress** — arc length capped at the first uncollected
//!   gate. Without it a car that cuts a corner accrues progress it did not
//!   earn and a reward built on it trains a corner-cutter that never collects
//!   a checkpoint (RULES-traps §4).
//!
//! Zero ghost reads: everything here comes from `mapgeom`, whose inputs are the
//! `.Map.Gbx` and the game's own pak.

use crate::geom::*;
use mapgeom::pack::{MapPack, Route, Vertex};
use tmstate::{Gate, GateKind, TrackGeom};

/// Where the car is, relative to the track.
#[derive(Clone, Copy, Debug)]
pub struct Probe {
    /// Arc length along the route, metres.
    pub s: f32,
    /// Signed horizontal offset from the route centre: positive to the route's
    /// right. This is the number an off-route test should use.
    pub lateral: f32,
    /// Height above the route point at `s`. Positive is above the road.
    pub height: f32,
    /// The corridor half-width at `s`.
    pub half_width: f32,
    /// Route tangent at `s`, unit.
    pub tangent: V3,
}

/// How far along the route the probe may move between samples, metres.
///
/// Not fitted: the car is sampled every tick and cannot cover more than about
/// a metre in one at any speed the game produces, so this is already an order
/// of magnitude of slack. It exists so a long macro or a respawn still finds
/// itself.
pub const PROGRESS_WINDOW: f32 = 60.0;

/// The geometry version `Track::geom` writes. Bump when the resampling or the
/// gate derivation changes.
pub const GEOM_VERSION: u32 = 1;
/// Centreline resampling step, metres (INTERFACES.md: "every ~2 m").
pub const GEOM_STEP: f32 = 2.0;

pub struct Track {
    /// The cartographer's pack and route, when the track came from them. When
    /// it came from a `geom.json` these are SYNTHESIZED from the geometry
    /// (verts = pts, no checkpoint groups) so the diagnostics still run.
    pub pack: MapPack,
    pub route: Route,
    /// **The geometry the observation and the reward are computed against.**
    /// One source of truth, whichever way the track was built.
    pub geom: std::sync::Arc<TrackGeom>,
    /// Gate centres on the road surface, in tour order, finish last.
    pub gate_pos: Vec<V3>,
    /// Arc length of each gate, tour order, finish last.
    pub gate_s: Vec<f32>,
}

impl Track {
    /// From the cartographer's pack and route. The `TrackGeom` is derived here:
    /// the route resampled every `GEOM_STEP` metres, the corridor half-width
    /// read off the nearest route vertex, and the gates at the route's own
    /// road-surface point (`route.at(gate_s)`, see the module docs), with the
    /// route tangent as their normal. `source = "cartographer"`.
    pub fn new(pack: MapPack, route: Route) -> Track {
        let hw_at = |s: f32| -> f32 {
            let mut best = (f32::INFINITY, mapgeom::pack::CORRIDOR_FLOOR);
            for v in &route.verts {
                let d = (v.s - s).abs();
                if d < best.0 {
                    best = (d, v.half_width.max(mapgeom::pack::CORRIDOR_FLOOR));
                }
            }
            best.1
        };
        let tangent_at = |s: f32| -> V3 {
            let a = route.at((s - 1.0).max(0.0));
            let b = route.at((s + 1.0).min(route.length));
            let t = unit(sub(b, a));
            if norm(t) < 0.5 { [1.0, 0.0, 0.0] } else { t }
        };
        let n = ((route.length / GEOM_STEP).ceil() as usize).max(1) + 1;
        let mut pts = Vec::with_capacity(n);
        let mut s = Vec::with_capacity(n);
        let mut half_width = Vec::with_capacity(n);
        for i in 0..n {
            let si = (i as f32 * GEOM_STEP).min(route.length);
            pts.push(route.at(si));
            s.push(si);
            half_width.push(hw_at(si));
        }
        let ng = route.gate_s.len();
        let gates: Vec<Gate> = route
            .gate_s
            .iter()
            .enumerate()
            .map(|(i, &gs)| Gate {
                kind: if i + 1 == ng { GateKind::Finish } else { GateKind::Checkpoint },
                centre: route.at(gs),
                normal: tangent_at(gs),
                half_width: hw_at(gs),
                s: gs,
                map_waypoint: u32::MAX,
            })
            .collect();
        let t0 = tangent_at(0.0);
        let geom = TrackGeom {
            geom_version: GEOM_VERSION,
            map_uid: pack.uid.clone(),
            pts,
            half_width,
            s,
            gates,
            spawn: route.at(0.0),
            spawn_yaw: t0[0].atan2(-t0[2]),
            source: "cartographer".into(),
            legs: None,
            route: None,
            speed_hint: None,
        };
        Self::assemble(pack, route, geom)
    }

    /// From a `TrackGeom` (the DATA arm's `geom.json`, a field median, a WR
    /// line): the pack and route are synthesized from it so every consumer of
    /// `Track` works unchanged. `author_ms` is unknown on this path.
    pub fn from_geom(geom: TrackGeom) -> Track {
        let verts: Vec<Vertex> = geom
            .pts
            .iter()
            .zip(geom.s.iter())
            .enumerate()
            .map(|(i, (p, s))| Vertex {
                pos: *p,
                s: *s,
                half_width: geom.half_width.get(i).copied().unwrap_or(mapgeom::pack::CORRIDOR_FLOOR),
                material: String::new(),
                next_gate: geom.gates.iter().position(|g| g.s > *s).unwrap_or(geom.gates.len()),
            })
            .collect();
        let length = geom.length();
        let stations: Vec<f32> = (0..=((length / 20.0) as usize)).map(|i| i as f32 * 20.0).collect();
        let route = Route {
            verts,
            gate_s: geom.gates.iter().map(|g| g.s).collect(),
            order: (0..geom.gates.len().saturating_sub(1)).collect(),
            order_exact: false,
            length,
            stations,
        };
        let pack = MapPack {
            uid: geom.map_uid.clone(),
            name: geom.map_uid.clone(),
            author_ms: None,
            yoff: 0.0,
            spawn: geom.spawn,
            spawn_yaw: geom.spawn_yaw,
            checkpoints: Vec::new(),
            finish: Vec::new(),
            road_materials: Vec::new(),
            group_control: Vec::new(),
        };
        Self::assemble(pack, route, geom)
    }

    /// Read a `geom.json` (serde form of `tmstate::TrackGeom`).
    pub fn load_geom_json(path: &std::path::Path) -> Result<Track, String> {
        let text = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
        let g: TrackGeom = serde_json::from_str(&text).map_err(|e| format!("{}: {e}", path.display()))?;
        if g.pts.len() != g.s.len() || g.pts.len() != g.half_width.len() {
            return Err(format!(
                "{}: pts/s/half_width lengths differ ({}/{}/{})",
                path.display(),
                g.pts.len(),
                g.s.len(),
                g.half_width.len()
            ));
        }
        if g.s.windows(2).any(|w| w[1] < w[0]) {
            return Err(format!("{}: arc length is not monotone", path.display()));
        }
        Ok(Self::from_geom(g))
    }

    /// Write the geometry as `geom.json`.
    pub fn save_geom_json(&self, path: &std::path::Path) -> Result<(), String> {
        let text = serde_json::to_string_pretty(&*self.geom).map_err(|e| e.to_string())?;
        std::fs::write(path, text).map_err(|e| format!("{}: {e}", path.display()))
    }

    fn assemble(pack: MapPack, route: Route, geom: TrackGeom) -> Track {
        let gate_s = geom.gates.iter().map(|g| g.s).collect();
        let gate_pos = geom.gates.iter().map(|g| g.centre).collect();
        Track { pack, route, geom: std::sync::Arc::new(geom), gate_pos, gate_s }
    }

    pub fn length(&self) -> f32 {
        self.geom.length()
    }

    pub fn n_gates(&self) -> usize {
        self.gate_s.len()
    }

    /// The author time in seconds, out of the map file's own header.
    pub fn author_s(&self) -> Option<f64> {
        self.pack.author_ms.map(|m| m as f64 / 1000.0)
    }

    /// The route tangent at `s` (tmobs: central difference over 1 m).
    pub fn tangent(&self, s: f32) -> V3 {
        tmobs::tangent(&self.geom, s)
    }

    /// The corridor half-width at `s` (tmobs: the nearer geometry point, floored).
    pub fn half_width(&self, s: f32) -> f32 {
        tmobs::half_width(&self.geom, s)
    }

    /// The gate's plane normal, as the geometry carries it.
    pub fn gate_normal(&self, i: usize) -> V3 {
        self.geom.gates[i].normal
    }

    pub fn probe(&self, p: V3) -> Probe {
        self.probe_near(p, None)
    }

    /// The route probe, restricted to a window around where the car was last
    /// seen.
    ///
    /// # Why a window, and why this is not a tuning knob
    ///
    /// `Route::progress` returns the nearest point on the WHOLE polyline. On any
    /// route that uses a piece of road twice — Summer 2026 - 01 drives the same
    /// vertical straight at the start and again at the end — the nearest point
    /// jumps between the two legs, and arc length is then not a progress
    /// measure at all. Measured on the scripted run: the car left the start,
    /// stayed on the first straight, and its arc length immediately read the
    /// LAST leg's value, saturating the progress cap in under a second.
    ///
    /// Linesight has the same problem and the same answer: their progress index
    /// walks forward and backward from where it was, clamped by a corridor and
    /// by the real-checkpoint gate, and `furthest_zone_idx` is tracked
    /// separately. This is that, in arc length.
    ///
    /// The window is not fitted: the car is sampled every tick and cannot
    /// exceed ~1 m per tick at any speed the game produces, so anything above a
    /// couple of metres is already slack. `PROGRESS_WINDOW` is 60 m so that a
    /// macro of 50 ticks, or a respawn, still finds itself.
    pub fn probe_near(&self, p: V3, near: Option<f32>) -> Probe {
        let s = match near {
            None => tmobs::project(&self.geom, p, 0.0, self.length()),
            Some(s0) => self.project_window(p, s0),
        };
        let c = tmobs::at(&self.geom, s);
        let t = self.tangent(s);
        let d = sub(p, c);
        // Split the offset: the component along world up is height, the rest is
        // lateral. `right = tangent x up` gives the sign.
        let up: V3 = [0.0, 1.0, 0.0];
        let right = unit(cross(t, up));
        Probe {
            s,
            lateral: dot(d, right),
            height: d[1],
            half_width: self.half_width(s),
            tangent: t,
        }
    }

    /// Nearest point on the route's SEGMENTS within the window, as arc length.
    ///
    /// Projecting onto segments rather than onto vertices is not a refinement.
    /// The route is Douglas-Peucker simplified, so a straight is two vertices
    /// tens of metres apart; snapping to the nearer of them makes arc length a
    /// staircase, the progress reward a sequence of jumps, and the lateral
    /// offset read tens of metres on a car that is dead centre. Measured before
    /// this was fixed: a car driving straight down the first straight reported
    /// s = 0, 0, 0, 44, 44, 44, 78 and a lateral of 13.3 m, and the off-route
    /// cut fired on it.
    fn project_window(&self, p: V3, s0: f32) -> f32 {
        tmobs::project(&self.geom, p, (s0 - PROGRESS_WINDOW).max(0.0), (s0 + PROGRESS_WINDOW).min(self.length()))
    }

    /// Points on the route ahead of `s`, in the car's frame.
    pub fn lookahead(&self, s: f32, car: V3, q: Quat, offsets: &[f32]) -> Vec<V3> {
        offsets
            .iter()
            .map(|&o| {
                let t = (s + o).min(self.length());
                q.world_to_car(sub(tmobs::at(&self.geom, t), car))
            })
            .collect()
    }
}

/// Which gates the car has collected, and the progress that follows from it.
///
/// # Collection is UNORDERED, and that is measured rather than assumed
///
/// The first version required gates in the tour's order. The plain oracle
/// disagreed with it immediately: on Summer 2026 - 01 the car crosses the gate
/// the tour places at s = 1222 m about four metres after the start line, and
/// the server credits a checkpoint for it while the ordered tracker reported
/// none. An ordered tracker on this map pins the progress cap at the first
/// gate's arc length forever, so the reward cannot pay for anything past 250 m
/// — which would cap training, invisibly, at the first sixth of the map.
///
/// So a gate is collected when the car reaches it, in whatever order, and the
/// oracle is the arbiter of whether that reading is right (`tmenv calibrate`).
///
/// # The saturation rule survives unchanged
///
/// Progress is `min(arc length, the arc length of the first UNCOLLECTED gate
/// along the route)`. A car that leaves the road and rejoins past a checkpoint
/// has a large arc length and has not collected it; capping at the owed gate
/// makes the checkpoint a wall in the reward rather than a preference. Being
/// unordered does not weaken that: collecting a LATER gate early does not raise
/// the cap, because the cap is the first gate still owed.
#[derive(Clone, Debug)]
pub struct GateTracker {
    /// 3-D radius around the gate's road-surface centre.
    pub radius: f32,
    /// How far along the route the car must also be, so a parallel piece of
    /// Unused by the crossing test, kept so a caller that set it still
    /// compiles. The arc-length window was the FIRST thing tried and it was
    /// actively wrong: on a route that reuses a road the car passes a gate at
    /// a completely different arc length than the tour assigns it, so the
    /// window rejected every real crossing and no radius could discriminate.
    pub s_window: f32,
    /// Which way through the plane counts. Swept against the oracle rather
    /// than assumed: the route's tangent is B's tour direction, and on a map
    /// whose tour order is itself in question that sign is a hypothesis.
    pub dir: CrossDir,
    hit: Vec<bool>,
    prev: Option<V3>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CrossDir {
    Forward,
    Reverse,
    Either,
}

impl CrossDir {
    pub fn parse(s: &str) -> Option<CrossDir> {
        match s {
            "fwd" | "forward" => Some(CrossDir::Forward),
            "rev" | "reverse" => Some(CrossDir::Reverse),
            "any" | "either" => Some(CrossDir::Either),
            _ => None,
        }
    }
}

impl GateTracker {
    pub fn new(radius: f32, s_window: f32) -> GateTracker {
        GateTracker { radius, s_window, dir: CrossDir::Either, hit: Vec::new(), prev: None }
    }

    pub fn hit(&self) -> usize {
        self.hit.iter().filter(|x| **x).count()
    }

    pub fn collected(&self) -> &[bool] {
        &self.hit
    }

    pub fn reset(&mut self) {
        for h in self.hit.iter_mut() {
            *h = false;
        }
        self.prev = None;
    }

    /// Offer one tick's position.
    ///
    /// # A gate is a PLANE the car crosses, not a sphere it enters
    ///
    /// Proximity was measured against the oracle and it does not discriminate:
    /// over 40 tapes, every radius from 6 m to 26 m fired on all 40 while the
    /// server credited a checkpoint on 23. Seventeen runs came within six
    /// metres of the gate and got nothing — they approached and stopped, or
    /// veered, without ever passing through it. Widening the radius cannot fix
    /// that and narrowing it starts missing real crossings (radius 4 m: 21
    /// misses).
    ///
    /// So the test is a signed crossing of the gate's plane, whose normal is
    /// the route's own tangent there, within a lateral radius of its centre.
    /// That is what a checkpoint is.
    pub fn observe(&mut self, t: &Track, p: V3, s: f32) -> bool {
        if self.hit.len() != t.n_gates() {
            self.hit = vec![false; t.n_gates()];
        }
        let prev = match self.prev {
            None => {
                self.prev = Some(p);
                return false;
            }
            Some(q) => q,
        };
        self.prev = Some(p);
        let _ = s;
        let mut got = false;
        for i in 0..t.n_gates() {
            if self.hit[i] {
                continue;
            }
            let g = t.gate_pos[i];
            let n = t.gate_normal(i);
            let d0 = dot(sub(prev, g), n);
            let d1 = dot(sub(p, g), n);
            // Crossed the plane going the way the route goes.
            let fwd = d0 <= 0.0 && d1 > 0.0;
            let rev = d0 >= 0.0 && d1 < 0.0;
            let crossed = match self.dir {
                CrossDir::Forward => fwd,
                CrossDir::Reverse => rev,
                CrossDir::Either => fwd || rev,
            };
            if crossed {
                // ...and passed through the gate rather than round it: the
                // component of the offset perpendicular to the normal, at the
                // crossing.
                let f = if (d1 - d0).abs() < 1e-9 { 0.0 } else { -d0 / (d1 - d0) };
                let x = add(prev, scale(sub(p, prev), f));
                let off = sub(x, g);
                let lat = sub(off, scale(n, dot(off, n)));
                if norm(lat) <= self.radius {
                    self.hit[i] = true;
                    got = true;
                }
            }
        }
        got
    }

    /// The arc length progress may not exceed: the first gate still owed.
    pub fn cap(&self, t: &Track) -> f32 {
        for i in 0..t.n_gates() {
            if !self.hit.get(i).copied().unwrap_or(false) {
                return t.gate_s[i];
            }
        }
        t.length()
    }

    pub fn finished(&self, t: &Track) -> bool {
        t.n_gates() > 0 && self.hit.len() == t.n_gates() && self.hit.iter().all(|x| *x)
    }
}

/// The car-switch ("transform") blocks: `Gameplay{Snow,Rally,Desert}` blocks
/// and items re-bind the participant's vehicle to another car model. The
/// validator's car chain used to read slot 0 (Stadium) unconditionally and
/// FREEZE at the switch (INPUT arm, 2026-09-06: the participant holds four
/// vehicles at +0x1118/+0x1128/+0x1138/+0x1148 = Stadium/Snow/Rally/Desert;
/// the live one is the slot whose u32 at phy+0x10 != 0xffffffff). Until the
/// readout follows the live slot, an env on such a map silently reports a
/// frozen car -- so it is refused, by name, before any server starts.
pub fn car_switch_blocks(map: &std::path::Path) -> Vec<String> {
    let Some(mf) = map_load_tolerant(map) else { return Vec::new() };
    let is_switch = |n: &str| {
        let l = n.to_ascii_lowercase();
        l.contains("gameplay") && (l.contains("snow") || l.contains("rally") || l.contains("desert"))
    };
    let mut out: Vec<String> = Vec::new();
    for b in mf.blocks.iter().chain(mf.baked.iter()) {
        if is_switch(&b.name) && !out.contains(&b.name) {
            out.push(b.name.clone());
        }
    }
    for it in &mf.items {
        if is_switch(&it.model) && !out.contains(&it.model) {
            out.push(it.model.clone());
        }
    }
    out
}

/// `tmmaps::map::MapFile::try_load` that also survives tmmaps' own assertions
/// (free-block maps: "chunk 0x0304305F holds N entries but the map has M free
/// blocks", 30+ of the pool's TOTD maps on 2026-09-07). The env does not need
/// the block list to run -- the geometry comes from geom.json and the engine
/// reads the map itself -- so a map tmmaps cannot parse is `None`, not a panic.
pub fn map_load_tolerant(map: &std::path::Path) -> Option<tmmaps::map::MapFile> {
    let prev = std::panic::take_hook();
    std::panic::set_hook(Box::new(|_| {}));
    let r = std::panic::catch_unwind(|| tmmaps::map::MapFile::try_load(map).ok());
    std::panic::set_hook(prev);
    r.ok().flatten()
}
