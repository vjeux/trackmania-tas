//! A `Plan` → a `router-plan` `TrackGeom` route (INTERFACES §1, `Predicted` legs).

use crate::estimator::EdgeKind;
use crate::planner::Plan;
use crate::surface::{Nodes, SurfaceModel};
use tmroute::gates::{GatesFile, WpKind};
use tmroute::types::*;

fn dist(a: [f32; 3], b: [f32; 3]) -> f32 {
    ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2) + (a[2] - b[2]).powi(2)).sqrt()
}

fn norm(v: [f32; 3]) -> [f32; 3] {
    let n = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
    if n < 1e-6 { [0.0, 0.0, 1.0] } else { [v[0] / n, v[1] / n, v[2] / n] }
}

pub fn export(
    gates: &GatesFile,
    nodes: &Nodes,
    surf: &SurfaceModel,
    fields: &[Option<(Vec<f32>, Vec<u32>)>],
    plan: &Plan,
    rank: u32,
    estimator: &str,
    produced_by: &str,
) -> TrackGeom {
    let mut pts: Vec<[f32; 3]> = vec![nodes.pos[0]];
    let mut hws: Vec<f32> = vec![surf.half_width_at(nodes.pos[0], 12.0).max(4.0)];
    let mut legs = Vec::new();
    let mut tg_gates = Vec::new();
    let mut gate_order = Vec::new();
    let mut s_acc = 0.0f32;
    for (li, w) in plan.visit.windows(2).enumerate() {
        let (from, to) = (w[0], w[1]);
        let e = plan.edges[li];
        let raw: Vec<[f32; 3]> = match e.kind {
            EdgeKind::Surface | EdgeKind::Learned => surf.path_points(nodes, fields, from, to).unwrap_or_else(|| vec![nodes.pos[from], nodes.pos[to]]),
            _ => vec![nodes.pos[from], nodes.pos[to]],
        };
        let leg_pts = tmroute::human::resample(&raw, 2.0);
        let s_start = s_acc;
        for p in leg_pts.iter().skip(1) {
            s_acc += dist(*pts.last().unwrap(), *p);
            pts.push(*p);
            hws.push(if e.kind == EdgeKind::Surface { surf.half_width_at(*p, 12.0).max(4.0) } else { 6.0 });
        }
        let n = pts.len();
        let heading = if n >= 2 { norm([pts[n - 1][0] - pts[n - 2][0], 0.0, pts[n - 1][2] - pts[n - 2][2]]) } else { [0.0, 0.0, 1.0] };
        let grp = nodes.groups[to];
        let (centre, _axis, half) = gates.group_geometry(grp).unwrap();
        let rep = gates.group_rep(grp).unwrap();
        // the LAST gate is where the race ends, whatever block it is (a lap line on a lap race)
        let kind = if li + 1 == plan.visit.len() - 1 {
            GateKind::Finish
        } else {
            match rep.kind {
                WpKind::Finish => GateKind::Finish,
                WpKind::Multilap => GateKind::Multilap,
                _ => GateKind::Checkpoint,
            }
        };
        tg_gates.push(Gate { kind, centre, normal: heading, half_width: half, s: s_acc, map_waypoint: rep.waypoint });
        let (_, dy) = crate::estimator::chord(nodes.pos[from], nodes.pos[to]);
        let connection = match e.kind {
            EdgeKind::Surface => ConnectionClass::Road,
            EdgeKind::Flight => if dy < -8.0 { ConnectionClass::Drop } else { ConnectionClass::Jump },
            // a learned leg over a surface path is a road leg R agreed with; without a path it is R's guess
            EdgeKind::Learned => if surf.path_points(nodes, fields, from, to).is_some() { ConnectionClass::Road } else { ConnectionClass::Unknown },
            EdgeKind::None => ConnectionClass::Unknown,
        };
        let v = e.arrival.speed();
        legs.push(Leg {
            gate_idx: li as u32,
            map_waypoint: rep.waypoint,
            s_start,
            s_end: s_acc,
            connection,
            arrival_speed: [(v - 20.0).max(5.0), v + 20.0],
            arrival_heading: heading,
            arrival_heading_tol: 0.5,
            arrival_height: [centre[1] - rep.half_height - 1.0, centre[1] - rep.half_height + 3.0],
            p_reach: e.p_reach,
            expected_ms: e.expected_ms,
            evidence: LegEvidence::Predicted,
        });
        gate_order.push(rep.waypoint);
    }
    let mut s = Vec::with_capacity(pts.len());
    let mut acc = 0.0;
    for k in 0..pts.len() {
        if k > 0 {
            acc += dist(pts[k - 1], pts[k]);
        }
        s.push(acc);
    }
    for (li, l) in legs.iter().enumerate() {
        tg_gates[li].s = l.s_end;
    }
    let spawn_dir = if pts.len() > 3 { norm([pts[3][0] - pts[0][0], 0.0, pts[3][2] - pts[0][2]]) } else { [0.0, 0.0, 1.0] };
    TrackGeom {
        geom_version: GEOM_VERSION,
        map_uid: gates.map_uid.clone(),
        pts,
        half_width: hws,
        s,
        gates: tg_gates,
        spawn: gates.spawn.pos,
        spawn_yaw: spawn_dir[0].atan2(spawn_dir[2]),
        source: "router-plan".into(),
        legs: Some(legs),
        route: Some(RouteMeta {
            route_version: ROUTE_VERSION,
            source: "router-plan".into(),
            rank,
            predicted_ms: plan.total_ms,
            status: RouteStatus::Hypothesis,
            gate_order,
            produced_by: format!("{produced_by}; estimator {estimator}; P(reach) {:.3}; score {:.0}", plan.p_reach, plan.score),
        }),
    }
}
