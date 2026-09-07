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
use mapgeom::pack::{MapPack, Route};

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

pub struct Track {
    pub pack: MapPack,
    pub route: Route,
    /// Gate centres on the road surface, in tour order, finish last.
    pub gate_pos: Vec<V3>,
    /// Arc length of each gate, tour order, finish last.
    pub gate_s: Vec<f32>,
}

impl Track {
    pub fn new(pack: MapPack, route: Route) -> Track {
        let gate_s = route.gate_s.clone();
        let gate_pos = gate_s.iter().map(|&s| route.at(s)).collect();
        Track { pack, route, gate_pos, gate_s }
    }

    pub fn length(&self) -> f32 {
        self.route.length
    }

    pub fn n_gates(&self) -> usize {
        self.gate_s.len()
    }

    /// The author time in seconds, out of the map file's own header.
    pub fn author_s(&self) -> Option<f64> {
        self.pack.author_ms.map(|m| m as f64 / 1000.0)
    }

    /// The route tangent at `s`, by central difference on the polyline.
    pub fn tangent(&self, s: f32) -> V3 {
        let h = 1.0f32;
        let a = self.route.at((s - h).max(0.0));
        let b = self.route.at((s + h).min(self.route.length));
        let t = unit(sub(b, a));
        if norm(t) < 0.5 {
            [1.0, 0.0, 0.0]
        } else {
            t
        }
    }

    /// The corridor half-width at `s`, read off the nearest vertex.
    pub fn half_width(&self, s: f32) -> f32 {
        let mut best = (f32::INFINITY, mapgeom::pack::CORRIDOR_FLOOR);
        for v in &self.route.verts {
            let d = (v.s - s).abs();
            if d < best.0 {
                best = (d, v.half_width.max(mapgeom::pack::CORRIDOR_FLOOR));
            }
        }
        best.1
    }

    /// The gate's plane normal: the route's own tangent where it crosses.
    pub fn gate_normal(&self, i: usize) -> V3 {
        self.tangent(self.gate_s[i])
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
            None => self.route.progress(p).0,
            Some(s0) => self.project_window(p, s0),
        };
        let c = self.route.at(s);
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
        let v = &self.route.verts;
        if v.len() < 2 {
            return s0;
        }
        let mut best = (f32::INFINITY, s0);
        for w in v.windows(2) {
            let (a, b) = (&w[0], &w[1]);
            if (a.s - s0).abs() > PROGRESS_WINDOW && (b.s - s0).abs() > PROGRESS_WINDOW {
                continue;
            }
            let ab = sub(b.pos, a.pos);
            let len2 = dot(ab, ab);
            let t = if len2 < 1e-9 { 0.0 } else { (dot(sub(p, a.pos), ab) / len2).clamp(0.0, 1.0) };
            let q = add(a.pos, scale(ab, t));
            let d = norm(sub(p, q));
            if d < best.0 {
                best = (d, a.s + (b.s - a.s) * t);
            }
        }
        if best.0.is_finite() {
            best.1
        } else {
            s0
        }
    }

    /// Points on the route ahead of `s`, in the car's frame.
    pub fn lookahead(&self, s: f32, car: V3, q: Quat, offsets: &[f32]) -> Vec<V3> {
        offsets
            .iter()
            .map(|&o| {
                let t = (s + o).min(self.route.length);
                q.world_to_car(sub(self.route.at(t), car))
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
