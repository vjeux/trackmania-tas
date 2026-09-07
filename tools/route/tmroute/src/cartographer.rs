//! `tmroute from-cartographer` — the cartographer's `<uid>.pack.json` +
//! `<uid>.route.json` (B-cartographer/packs) → a `router: "cartographer"`
//! route with `Predicted` legs.
//!
//! The cartographer's route is a 2 m grid path over its surface graph, not a
//! racing line (its INTERFACE.md §3); it is good for arc length and order. Its
//! checkpoint list is `tag == "Checkpoint"` only — `LinkedCheckpoint` groups are
//! missing on 8 maps (finding F1), so an imported route may have FEWER legs than
//! `gates.json` has checkpoint groups. That is recorded in `route.produced_by`
//! and printed; the order over the checkpoints it knew is still a control.

use crate::gates::GatesFile;
use crate::types::*;
use serde_json::Value;
use std::path::Path;

fn v3(v: &Value) -> Option<[f32; 3]> {
    let a = v.as_array()?;
    Some([a[0].as_f64()? as f32, a[1].as_f64()? as f32, a[2].as_f64()? as f32])
}

/// Match a cartographer checkpoint (its gate records) to a `gates.json` group:
/// same model name and XZ within 1.5 m for any of its gates.
fn match_group(cp: &Value, gates: &GatesFile) -> Option<u32> {
    for g in cp.get("gates")?.as_array()? {
        let name = g.get("name")?.as_str()?;
        let pos = v3(g.get("pos")?)?;
        for r in &gates.gates {
            if r.model == name {
                let dx = r.centre[0] - pos[0];
                let dz = r.centre[2] - pos[2];
                if (dx * dx + dz * dz).sqrt() <= 1.5 {
                    return Some(r.group);
                }
            }
        }
    }
    None
}

pub struct Imported {
    pub geom: TrackGeom,
    pub map_name: String,
    /// Checkpoint groups in gates.json the cartographer did not know.
    pub missing_groups: Vec<u32>,
}

pub fn import(pack: &Path, route: &Path, gates: &GatesFile, produced_by: &str) -> Result<Imported, String> {
    let pk: Value = serde_json::from_str(&std::fs::read_to_string(pack).map_err(|e| e.to_string())?)
        .map_err(|e| format!("{}: {e}", pack.display()))?;
    let rt: Value = serde_json::from_str(&std::fs::read_to_string(route).map_err(|e| e.to_string())?)
        .map_err(|e| format!("{}: {e}", route.display()))?;
    let uid = pk["uid"].as_str().ok_or("pack: uid")?.to_string();
    if uid != gates.map_uid {
        return Err(format!("pack uid {uid} != gates.json uid {}", gates.map_uid));
    }
    let name = pk["name"].as_str().unwrap_or("").to_string();

    // corridor
    let verts = rt["verts"].as_array().ok_or("route: verts")?;
    let mut pts = Vec::with_capacity(verts.len());
    let mut hw = Vec::with_capacity(verts.len());
    let mut s = Vec::with_capacity(verts.len());
    for v in verts {
        pts.push(v3(&v["p"]).ok_or("vert p")?);
        hw.push(v["w"].as_f64().ok_or("vert w")? as f32);
        s.push(v["s"].as_f64().ok_or("vert s")? as f32);
    }

    // order over the cartographer's checkpoints, then the finish
    let order: Vec<usize> = rt["order"]
        .as_array()
        .ok_or("route: order")?
        .iter()
        .map(|x| x.as_u64().unwrap() as usize)
        .collect();
    let gate_s: Vec<f32> = rt["gate_s"].as_array().ok_or("route: gate_s")?.iter().map(|x| x.as_f64().unwrap() as f32).collect();
    let dirs: Vec<[f32; 3]> = pk["gate_dir_tour_order"].as_array().ok_or("pack: gate_dir_tour_order")?.iter().map(|x| v3(x).unwrap()).collect();
    let cps = pk["checkpoints"].as_array().ok_or("pack: checkpoints")?;
    let fins = pk["finish"].as_array().ok_or("pack: finish")?;
    if gate_s.len() != order.len() + 1 || dirs.len() != gate_s.len() {
        return Err(format!("route/pack disagree: order {} gate_s {} dirs {}", order.len(), gate_s.len(), dirs.len()));
    }

    // Which finish group did the tour end at? The one nearest the last vertex.
    let last = *pts.last().ok_or("empty route")?;
    let fin_group = fins
        .iter()
        .filter_map(|f| {
            let p = v3(&f["pos"])?;
            let d = (p[0] - last[0]).powi(2) + (p[2] - last[2]).powi(2);
            Some((d, match_group(f, gates)?))
        })
        .min_by(|a, b| a.0.partial_cmp(&b.0).unwrap())
        .map(|x| x.1)
        .ok_or("no finish gate in gates.json matches the pack's finish")?;

    let mut groups: Vec<u32> = Vec::new();
    for &ci in &order {
        let g = match_group(&cps[ci], gates).ok_or_else(|| format!("cartographer checkpoint {ci} matches no gates.json group"))?;
        groups.push(g);
    }
    groups.push(fin_group);

    let mut tg_gates = Vec::new();
    let mut legs = Vec::new();
    let mut gate_order = Vec::new();
    let mut prev_s = 0.0f32;
    for (i, g) in groups.iter().enumerate() {
        let (centre, _axis, half) = gates.group_geometry(*g).ok_or("group geometry")?;
        let rep = gates.group_rep(*g).unwrap();
        let kind = if i + 1 == groups.len() { GateKind::Finish } else if rep.kind == crate::gates::WpKind::Multilap { GateKind::Multilap } else { GateKind::Checkpoint };
        let n = dirs[i];
        tg_gates.push(Gate { kind, centre, normal: n, half_width: half, s: gate_s[i] });
        legs.push(Leg {
            gate_idx: i as u32,
            map_waypoint: rep.waypoint,
            s_start: prev_s,
            s_end: gate_s[i],
            connection: ConnectionClass::Unknown,
            arrival_speed: [f32::NAN, f32::NAN],
            arrival_heading: n,
            arrival_heading_tol: f32::NAN,
            arrival_height: [f32::NAN, f32::NAN],
            p_reach: f32::NAN,
            expected_ms: -1,
            evidence: LegEvidence::Predicted,
        });
        gate_order.push(rep.waypoint);
        prev_s = gate_s[i];
    }

    let known: Vec<u32> = gates.checkpoint_group_ids();
    let missing: Vec<u32> = known.iter().copied().filter(|g| !groups.contains(g)).collect();
    let note = if missing.is_empty() { String::new() } else { format!(" MISSING {} checkpoint group(s) {:?} (LinkedCheckpoint, finding F1)", missing.len(), missing) };

    let spawn = v3(&pk["spawn"]).ok_or("pack: spawn")?;
    let start_dir = v3(&pk["start_dir"]).unwrap_or([0.0, 0.0, 1.0]);
    let geom = TrackGeom {
        geom_version: GEOM_VERSION,
        map_uid: uid,
        pts,
        half_width: hw,
        s,
        gates: tg_gates,
        spawn,
        spawn_yaw: start_dir[0].atan2(start_dir[2]),
        source: "router-cartographer".into(),
        legs: Some(legs),
        route: Some(RouteMeta {
            route_version: ROUTE_VERSION,
            source: "cartographer".into(),
            rank: 0,
            predicted_ms: -1,
            status: RouteStatus::Hypothesis,
            gate_order,
            produced_by: format!("{produced_by}; from {} + {}{note}", pack.file_name().unwrap().to_string_lossy(), route.file_name().unwrap().to_string_lossy()),
        }),
    };
    Ok(Imported { geom, map_name: name, missing_groups: missing })
}
