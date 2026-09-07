//! The route DATA types — coord/INTERFACES.md §1, verbatim.
//!
//! A route is the player's `TrackGeom` (coord/PLAYER-INTERFACES.md) with two
//! optional fields filled: `legs` and `route`. This file is a PRIVATE COPY of
//! the player's `TrackGeom`/`Gate`/`GateKind` until `tools/rl/tmstate` lands on
//! `agentcloud/player-env`; the day it exists this module becomes
//! `pub use tmstate::{TrackGeom, Gate, GateKind}` plus the route additions.
//!
//! Units: metres, m/s, radians, milliseconds as i32 in files (printed as
//! seconds with a decimal). World frame = the map's (x east, y UP, z).

use serde::{Deserialize, Serialize};

pub const GEOM_VERSION: u32 = 1;
pub const ROUTE_VERSION: u32 = 1;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum GateKind {
    Checkpoint,
    Finish,
    Multilap,
}

/// A gate in race order along the route's centreline.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Gate {
    pub kind: GateKind,
    pub centre: [f32; 3],
    /// Direction of travel through the gate (unit, world frame).
    pub normal: [f32; 3],
    pub half_width: f32,
    /// Arc length along `pts` at which the gate is crossed.
    pub s: f32,
    /// Index of this gate in the .Map.Gbx per `tmmaps waypoints` (u32::MAX unknown) — what makes gate
    /// orders comparable across sources. (tmstate added it 2026-09-07; same serde default.)
    #[serde(default = "u32_max")]
    pub map_waypoint: u32,
}

fn u32_max() -> u32 {
    u32::MAX
}

/// The route the observation is computed against (player) — and, with `legs`
/// and `route` filled, a ROUTE the driver follows leg by leg.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TrackGeom {
    pub geom_version: u32,
    pub map_uid: String,
    /// Centreline, resampled every ~2 m.
    pub pts: Vec<[f32; 3]>,
    /// Corridor half-width per point.
    pub half_width: Vec<f32>,
    /// Cumulative arc length per point (monotone).
    pub s: Vec<f32>,
    /// In race order: checkpoints then finish.
    pub gates: Vec<Gate>,
    pub spawn: [f32; 3],
    pub spawn_yaw: f32,
    /// "cartographer" | "field-median" | "wr-trajectory" | "router-*"
    pub source: String,
    /// None = "not a route, just a centreline".
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub legs: Option<Vec<Leg>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub route: Option<RouteMeta>,
}

/// Leg i ends at gates[gate_idx]; leg 0 starts at spawn, leg i>0 starts at
/// legs[i-1]'s gate. legs.len() == gates.len(); the last leg ends at the finish.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Leg {
    /// Index into TrackGeom.gates (race order).
    pub gate_idx: u32,
    /// The gate's waypoint index in the .Map.Gbx (`tmmaps waypoints`) — lets two
    /// routes from different sources be compared gate for gate.
    pub map_waypoint: u32,
    /// This leg's arc-length span on TrackGeom.pts (s_end == gates[gate_idx].s).
    pub s_start: f32,
    pub s_end: f32,
    pub connection: ConnectionClass,
    /// [lo, hi] m/s at the gate.
    #[serde(with = "nanf::arr2")]
    pub arrival_speed: [f32; 2],
    /// Unit vector, world frame, direction of travel through the gate.
    #[serde(with = "nanf::arr3")]
    pub arrival_heading: [f32; 3],
    /// Cone half-angle, rad.
    #[serde(with = "nanf::scalar")]
    pub arrival_heading_tol: f32,
    /// [lo, hi] world y at the gate (a jump-through vs a drive-through).
    #[serde(with = "nanf::arr2")]
    pub arrival_height: [f32; 2],
    /// R's P(reach this gate from the band at the previous one); NaN if not from R.
    #[serde(with = "nanf::scalar")]
    pub p_reach: f32,
    /// R's expected leg time; -1 if not from R.
    pub expected_ms: i32,
    pub evidence: LegEvidence,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ConnectionClass {
    Road,
    Jump,
    Drop,
    Cut,
    Wallride,
    Launcher,
    Unknown,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum LegEvidence {
    /// n human runs drove this leg this way.
    Human { runs: u32, best_ms: i32 },
    /// Engine fan-out from the previous gate's band.
    Rollout { reached: u32, tried: u32 },
    /// R only — a HYPOTHESIS.
    Predicted,
    /// A certified lap drove it (plain oracle, other box).
    Driven { ms: i32, ghost_md5: String },
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum RouteStatus {
    Hypothesis,
    Certified {
        ms: i32,
        ghost_md5: String,
        oracle_box: String,
    },
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RouteMeta {
    pub route_version: u32,
    /// "router-human" | "router-plan" | "router-rl" | "router-explore" | "cartographer"
    pub source: String,
    /// k-th best from its producer (0 = best).
    pub rank: u32,
    /// Whole-lap prediction; -1 if none.
    pub predicted_ms: i32,
    pub status: RouteStatus,
    /// map_waypoint per leg, finish last — the ORDER as one line.
    pub gate_order: Vec<u32>,
    /// tool + git hash + date, provenance.
    pub produced_by: String,
}

impl TrackGeom {
    /// Structural invariants of a route (INTERFACES §1). Returns every violation.
    pub fn validate(&self) -> Vec<String> {
        let mut e = Vec::new();
        let n = self.pts.len();
        if self.half_width.len() != n {
            e.push(format!("half_width.len {} != pts.len {}", self.half_width.len(), n));
        }
        if self.s.len() != n {
            e.push(format!("s.len {} != pts.len {}", self.s.len(), n));
        }
        if self.s.windows(2).any(|w| w[1] < w[0]) {
            e.push("s is not monotone".into());
        }
        if let Some(legs) = &self.legs {
            if legs.len() != self.gates.len() {
                e.push(format!("legs.len {} != gates.len {}", legs.len(), self.gates.len()));
            }
            let mut prev_end = 0.0f32;
            for (i, l) in legs.iter().enumerate() {
                if l.gate_idx as usize >= self.gates.len() {
                    e.push(format!("leg {i}: gate_idx {} out of range", l.gate_idx));
                    continue;
                }
                if (l.s_end - self.gates[l.gate_idx as usize].s).abs() > 1e-3 {
                    e.push(format!(
                        "leg {i}: s_end {} != gates[{}].s {}",
                        l.s_end, l.gate_idx, self.gates[l.gate_idx as usize].s
                    ));
                }
                if (l.s_start - prev_end).abs() > 1e-3 {
                    e.push(format!("leg {i}: s_start {} != previous s_end {prev_end}", l.s_start));
                }
                prev_end = l.s_end;
            }
            if let Some(last) = legs.last() {
                if let Some(g) = self.gates.get(last.gate_idx as usize) {
                    if g.kind != GateKind::Finish {
                        e.push("last leg does not end at a Finish gate".into());
                    }
                }
            }
            if let Some(r) = &self.route {
                let order: Vec<u32> = legs.iter().map(|l| l.map_waypoint).collect();
                if r.gate_order != order {
                    e.push("route.gate_order != legs' map_waypoint sequence".into());
                }
                if r.route_version != ROUTE_VERSION {
                    e.push(format!("route_version {} != {ROUTE_VERSION}", r.route_version));
                }
            } else {
                e.push("legs present but route meta missing".into());
            }
        }
        e
    }

    /// The order line: map_waypoint per leg, finish last. Empty for a bare geometry.
    pub fn gate_order(&self) -> Vec<u32> {
        match &self.route {
            Some(r) => r.gate_order.clone(),
            None => self
                .legs
                .as_ref()
                .map(|l| l.iter().map(|x| x.map_waypoint).collect())
                .unwrap_or_default(),
        }
    }
}

/// JSON has no NaN: an "unknown" f32 serializes as `null` and reads back as NaN, so route/geom files interchange
/// with the ROUTE FINDER's `tmroute::types::nanf`. Use `#[serde(with = "nanf::scalar")]` / `arr2` / `arr3`.
pub mod nanf {
    use serde::{Deserialize, Deserializer, Serialize, Serializer};

    fn to_opt(x: f32) -> Option<f32> {
        if x.is_nan() {
            None
        } else {
            Some(x)
        }
    }

    pub mod scalar {
        use super::*;
        pub fn serialize<S: Serializer>(x: &f32, s: S) -> Result<S::Ok, S::Error> {
            to_opt(*x).serialize(s)
        }
        pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<f32, D::Error> {
            Ok(Option::<f32>::deserialize(d)?.unwrap_or(f32::NAN))
        }
    }

    macro_rules! arr {
        ($name:ident, $n:expr) => {
            pub mod $name {
                use super::*;
                pub fn serialize<S: Serializer>(x: &[f32; $n], s: S) -> Result<S::Ok, S::Error> {
                    let v: Vec<Option<f32>> = x.iter().map(|&f| to_opt(f)).collect();
                    v.serialize(s)
                }
                pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<[f32; $n], D::Error> {
                    let v = Vec::<Option<f32>>::deserialize(d)?;
                    if v.len() != $n {
                        return Err(serde::de::Error::custom(format!("expected {} floats, got {}", $n, v.len())));
                    }
                    let mut out = [f32::NAN; $n];
                    for (o, x) in out.iter_mut().zip(v) {
                        *o = x.unwrap_or(f32::NAN);
                    }
                    Ok(out)
                }
            }
        };
    }
    arr!(arr2, 2);
    arr!(arr3, 3);
}
