//! The route DATA types — coord/INTERFACES.md §1.
//!
//! The types themselves live in the player's `tmstate` crate (`tools/rl/tmstate`,
//! on this branch since the agentcloud/player merge of 2026-09-07): `TrackGeom`,
//! `Gate`, `GateKind`, `Leg`, `ConnectionClass`, `LegEvidence`, `RouteMeta`,
//! `RouteStatus`, and the `nanf` JSON-null rule for unknown floats. This module
//! re-exports them and adds the route-side helpers (`TrackGeomExt`) and the two
//! version constants the route files carry.
//!
//! Units: metres, m/s, radians, milliseconds as i32 in files (printed as
//! seconds with a decimal). World frame = the map's (x east, y UP, z).

pub use tmstate::nanf;
pub use tmstate::{ConnectionClass, Gate, GateKind, Leg, LegEvidence, RouteMeta, RouteStatus, TrackGeom};

pub const GEOM_VERSION: u32 = 1;
pub const ROUTE_VERSION: u32 = 1;

/// Route-side helpers on the player's `TrackGeom`.
pub trait TrackGeomExt {
    /// Structural invariants of a route (INTERFACES §1). Returns every violation.
    fn validate(&self) -> Vec<String>;
    /// The order line: map_waypoint per leg, finish last. Empty for a bare geometry.
    fn gate_order(&self) -> Vec<u32>;
}

impl TrackGeomExt for TrackGeom {
    fn validate(&self) -> Vec<String> {
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

    fn gate_order(&self) -> Vec<u32> {
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
