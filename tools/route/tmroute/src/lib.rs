//! `tmroute` — the route DATA of the tm-route project (coord/INTERFACES.md §1, §3).
//!
//! * `types`        — `TrackGeom` + `Leg` + `RouteMeta` (private copy of the player's
//!                    `TrackGeom` until `tools/rl/tmstate` exists)
//! * `gates`        — `gates.json` per map: every gate, grouped into the checkpoints
//!                    the game counts, with the header's declared count as control
//! * `human`        — ghosts → crossings → orders → consensus → `router-human` route
//! * `cartographer` — the cartographer's pack/route JSON → a `cartographer` route
//! * `metrics`      — order agreement (exact, Kendall tau)
//! * `io`           — JSON files and the `routes.tsv` index

pub mod cartographer;
pub mod gates;
pub mod human;
pub mod io;
pub mod md5;
pub mod metrics;
pub mod types;

pub use types::*;

/// `tool git-hash date` provenance string for `produced_by`.
pub fn provenance(tool: &str) -> String {
    let hash = std::process::Command::new("git")
        .args(["rev-parse", "--short=10", "HEAD"])
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .unwrap_or_else(|| "nogit".into());
    let dirty = std::process::Command::new("git")
        .args(["status", "--porcelain", "--untracked-files=no", "."])
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .output()
        .ok()
        .map(|o| !o.stdout.is_empty())
        .unwrap_or(false);
    let hash = if dirty { format!("{hash}+dirty") } else { hash };
    let host = std::fs::read_to_string("/etc/hostname").map(|s| s.trim().to_string()).unwrap_or_default();
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
    format!("{tool} {hash} unix{now} {host}")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_route() -> TrackGeom {
        let pts = vec![[0.0, 10.0, 0.0], [0.0, 10.0, 2.0], [0.0, 10.0, 4.0], [0.0, 10.0, 6.0]];
        TrackGeom {
            geom_version: GEOM_VERSION,
            map_uid: "uid".into(),
            pts,
            half_width: vec![4.0; 4],
            s: vec![0.0, 2.0, 4.0, 6.0],
            gates: vec![
                Gate { kind: GateKind::Checkpoint, centre: [0.0, 12.0, 2.0], normal: [0.0, 0.0, 1.0], half_width: 8.0, s: 2.0 },
                Gate { kind: GateKind::Finish, centre: [0.0, 12.0, 6.0], normal: [0.0, 0.0, 1.0], half_width: 8.0, s: 6.0 },
            ],
            spawn: [0.0, 10.0, 0.0],
            spawn_yaw: 0.0,
            source: "router-human".into(),
            legs: Some(vec![
                Leg { gate_idx: 0, map_waypoint: 3, s_start: 0.0, s_end: 2.0, connection: ConnectionClass::Road, arrival_speed: [40.0, 50.0], arrival_heading: [0.0, 0.0, 1.0], arrival_heading_tol: 0.1, arrival_height: [10.0, 10.5], p_reach: f32::NAN, expected_ms: -1, evidence: LegEvidence::Human { runs: 5, best_ms: 1234 } },
                Leg { gate_idx: 1, map_waypoint: 1, s_start: 2.0, s_end: 6.0, connection: ConnectionClass::Jump, arrival_speed: [40.0, 50.0], arrival_heading: [0.0, 0.0, 1.0], arrival_heading_tol: 0.1, arrival_height: [10.0, 10.5], p_reach: 0.9, expected_ms: 2000, evidence: LegEvidence::Predicted },
            ]),
            route: Some(RouteMeta { route_version: ROUTE_VERSION, source: "router-human".into(), rank: 0, predicted_ms: 3234, status: RouteStatus::Hypothesis, gate_order: vec![3, 1], produced_by: "test".into() }),
        }
    }

    #[test]
    fn round_trip_json() {
        let g = sample_route();
        assert!(g.validate().is_empty(), "{:?}", g.validate());
        let s = serde_json::to_string(&g).unwrap();
        let back: TrackGeom = serde_json::from_str(&s).unwrap();
        assert_eq!(back.gate_order(), vec![3, 1]);
        assert_eq!(back.legs.as_ref().unwrap()[1].evidence, LegEvidence::Predicted);
        assert!(back.legs.as_ref().unwrap()[0].p_reach.is_nan());
        let s2 = serde_json::to_string(&back).unwrap();
        assert_eq!(s, s2);
    }

    #[test]
    fn bare_geometry_has_no_route() {
        let mut g = sample_route();
        g.legs = None;
        g.route = None;
        let s = serde_json::to_string(&g).unwrap();
        assert!(!s.contains("legs"));
        let back: TrackGeom = serde_json::from_str(&s).unwrap();
        assert!(back.legs.is_none() && back.route.is_none());
        assert!(back.validate().is_empty());
    }

    #[test]
    fn validate_catches_order_mismatch() {
        let mut g = sample_route();
        g.route.as_mut().unwrap().gate_order = vec![1, 3];
        assert_eq!(g.validate().len(), 1);
    }
}
