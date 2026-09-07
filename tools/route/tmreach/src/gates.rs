//! Gate geometry, and the crossing detector.
//!
//! Gates come from the GEOM arm's `geom/<mapUid>/gates.json` (INTERFACES §3:
//! centre, normal = direction of travel, half_width, half_height, model,
//! group) when it exists, else from `tmmaps waypoints` with the normal from
//! the block's dir (sign unknown — flagged).
//!
//! The detector's SHAPE is calibrated, not assumed (RL-agentG §5.1). What
//! `tmreach gatecal` measured on the 44 Summer 2026 - 01 ghosts: at the
//! instant of the ghost's own checkpoint notice the car centre sits at a
//! fixed signed distance `s` along its travel from the gate centre, with an
//! sd of 0.09–0.36 m per gate (a tenth of a tick), so the credited event is a
//! PLANE crossing at a per-model offset; the lateral / vertical extent of the
//! trigger is what the 200-rollout oracle control bounds.

use crate::json::Json;
use std::path::Path;
use tmmaps::map::{Kind, MapFile};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GateKind {
    Checkpoint,
    Finish,
    Multilap,
    Start,
}

#[derive(Clone, Debug)]
pub struct Gate {
    /// Index in `tmmaps waypoints` order — the `map_waypoint` of INTERFACES §1.
    pub waypoint: u32,
    pub kind: GateKind,
    pub model: String,
    pub from_item: bool,
    pub centre: [f64; 3],
    /// Unit vector, direction of travel through the gate.
    pub normal: [f64; 3],
    /// Whether `normal`'s sign is known (GEOM: "human"/"placement") or guessed.
    pub normal_known: bool,
    pub half_width: f64,
    pub half_height: f64,
    /// Gates that fire the same checkpoint share a group (GEOM). u32::MAX = none.
    pub group: u32,
}

impl Gate {
    /// (s along travel, lateral, up) of a point in the gate frame.
    pub fn local(&self, p: [f64; 3]) -> (f64, f64, f64) {
        let d = [p[0] - self.centre[0], p[1] - self.centre[1], p[2] - self.centre[2]];
        let n = self.normal;
        let s = d[0] * n[0] + d[1] * n[1] + d[2] * n[2];
        // lateral: horizontal, perpendicular to the normal's horizontal part
        let (hx, hz) = (n[0], n[2]);
        let hn = (hx * hx + hz * hz).sqrt().max(1e-9);
        let lat = (-d[0] * hz + d[2] * hx) / hn;
        (s, lat, d[1])
    }
}

pub struct MapGates {
    pub map_uid: String,
    pub map_name: String,
    pub gates: Vec<Gate>,
    pub spawn: Option<Gate>,
    pub source: String,
}

pub fn kind_of(tag: &str) -> GateKind {
    match tag {
        "Spawn" | "Start" => GateKind::Start,
        "Goal" | "Finish" => GateKind::Finish,
        "StartFinish" | "Multilap" => GateKind::Multilap,
        _ => GateKind::Checkpoint,
    }
}

/// The map uid, from the file name when it is one (the bank names maps by uid).
pub fn map_uid(map: &Path) -> String {
    let stem = map.file_name().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
    stem.trim_end_matches(".Map.Gbx").to_string()
}

impl MapGates {
    /// GEOM's gates.json if `geom_root/<uid>/gates.json` exists, else the map.
    pub fn load(map: &Path, geom_root: Option<&Path>) -> Result<MapGates, String> {
        let uid = map_uid(map);
        if let Some(root) = geom_root {
            let p = root.join(&uid).join("gates.json");
            if p.exists() {
                return Self::load_geom(&p);
            }
        }
        Self::load_map(map)
    }

    pub fn load_geom(p: &Path) -> Result<MapGates, String> {
        let txt = std::fs::read_to_string(p).map_err(|e| format!("{}: {}", p.display(), e))?;
        let j = crate::json::parse(&txt).map_err(|e| format!("{}: {}", p.display(), e))?;
        fn need<'a>(v: Option<&'a Json>, p: &Path, what: &str) -> Result<&'a Json, String> {
            v.ok_or_else(|| format!("{}: missing {}", p.display(), what))
        }
        let uid = need(j.get("map_uid"), p, "map_uid")?.str().unwrap_or("").to_string();
        let name = j.get("map_name").and_then(|v| v.str()).unwrap_or("").to_string();
        let mut gates = Vec::new();
        let mut spawn = None;
        for g in need(j.get("gates"), p, "gates")?.arr().ok_or("gates is not an array")? {
            let kind = kind_of(g.get("kind").and_then(|v| v.str()).unwrap_or("Checkpoint"));
            let gate = Gate {
                waypoint: g.get("waypoint").and_then(|v| v.f64()).unwrap_or(0.0) as u32,
                kind,
                model: g.get("model").and_then(|v| v.str()).unwrap_or("").to_string(),
                from_item: g.get("from_item") == Some(&Json::Bool(true)),
                centre: g.get("centre").and_then(|v| v.vec3()).ok_or("gate without centre")?,
                normal: g.get("normal").and_then(|v| v.vec3()).ok_or("gate without normal")?,
                normal_known: g.get("normal_source").and_then(|v| v.str()).map(|s| s != "guess").unwrap_or(true),
                half_width: g.get("half_width").and_then(|v| v.f64()).unwrap_or(8.0),
                half_height: g.get("half_height").and_then(|v| v.f64()).unwrap_or(4.0),
                group: g.get("group").and_then(|v| v.f64()).map(|x| x as u32).unwrap_or(u32::MAX),
            };
            if kind == GateKind::Start {
                spawn = Some(gate);
            } else {
                gates.push(gate);
            }
        }
        Ok(MapGates { map_uid: uid, map_name: name, gates, spawn, source: p.display().to_string() })
    }

    /// From the map alone: cell world points, normals from the block dir (sign
    /// GUESSED), item yaw for items.
    pub fn load_map(map: &Path) -> Result<MapGates, String> {
        let mf = MapFile::try_load(map)?;
        let collection = mf.items.first().map(|i| i.collection_raw).unwrap_or(26);
        let ground_y = tmmaps::map::ground_y(collection) as f64;
        let uid = map_uid(map);
        let mut gates = Vec::new();
        let mut spawn = None;
        for (i, w) in mf.waypoints().iter().enumerate() {
            let kind = kind_of(&w.tag);
            let (cx, cy, cz) = w.coords;
            let centre = match w.pos {
                Some(p) => [p[0] as f64, p[1] as f64 + 4.0, p[2] as f64],
                None => [32.0 * cx as f64 + 16.0, 8.0 * cy as f64 + ground_y + 2.0 + 4.0, 32.0 * cz as f64 + 16.0],
            };
            let yaw = match (w.kind.clone(), w.dir, w.yaw) {
                (Kind::Block, Some(d), _) => d as f64 * std::f64::consts::FRAC_PI_2,
                (_, _, Some(y)) => y as f64,
                _ => 0.0,
            };
            let g = Gate {
                waypoint: i as u32,
                kind,
                model: w.name.clone(),
                from_item: w.kind == Kind::Item,
                centre,
                normal: [yaw.sin(), 0.0, yaw.cos()],
                normal_known: false,
                half_width: if w.kind == Kind::Item { 16.0 } else { 8.0 },
                half_height: 4.0,
                group: u32::MAX,
            };
            if kind == GateKind::Start {
                spawn = Some(g);
            } else {
                gates.push(g);
            }
        }
        Ok(MapGates { map_uid: uid, map_name: String::new(), gates, spawn, source: format!("{} (tmmaps waypoints; normal signs GUESSED)", map.display()) })
    }

    pub fn gate(&self, wp: u32) -> Option<&Gate> {
        self.gates.iter().find(|g| g.waypoint == wp)
    }
}

/// One model's trigger, in the gate frame: the car centre is credited when it
/// crosses the plane `s = s_off` (m along travel from the gate centre) while
/// within `lat_half` of the centre line and within `up_lo..up_hi` of the
/// centre height. `depth` bounds how far past the plane the "inside" test
/// still fires (a fast car can only skip a thin box).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Trigger {
    pub s_off: f64,
    pub depth: f64,
    pub lat_half: f64,
    pub up_lo: f64,
    pub up_hi: f64,
}

impl Trigger {
    pub fn inside(&self, g: &Gate, p: [f64; 3]) -> bool {
        let (s, lat, up) = g.local(p);
        s >= self.s_off && s <= self.s_off + self.depth && lat.abs() <= self.lat_half && up >= self.up_lo && up <= self.up_hi
    }
}

/// The calibrated detector: one trigger per gate model, plus a default.
#[derive(Clone, Debug)]
pub struct Detector {
    pub per_model: Vec<(String, Trigger)>,
    pub default: Trigger,
    pub provenance: String,
}

impl Detector {
    pub fn trigger_for(&self, g: &Gate) -> Trigger {
        self.per_model.iter().find(|(m, _)| *m == g.model).map(|(_, t)| *t).unwrap_or(self.default)
    }

    /// First row index at which the car is inside each gate's trigger, or -1;
    /// `already[gi]` marks gates credited before the rollout began. THE FINISH
    /// IS ARMED ONLY WHEN EVERY CHECKPOINT GROUP HAS BEEN CREDITED (oracle
    /// control: a car through the finish plane with checkpoints missing is not
    /// finished), so its crossing is the first inside row at or after the last
    /// checkpoint's crediting row. A group fires once: its first gate entered.
    pub fn first_crossings(&self, gates: &MapGates, rows: &[forkoracle::layout::Row], already: &[bool]) -> Vec<i32> {
        let mut out = vec![-1i32; gates.gates.len()];
        let mut group_row: std::collections::HashMap<u32, i32> = Default::default();
        // checkpoints (and multilap) first
        for (gi, g) in gates.gates.iter().enumerate() {
            if g.kind == GateKind::Finish {
                continue;
            }
            if already.get(gi).copied().unwrap_or(false) {
                group_row.insert(g.group, -1);
                continue;
            }
            let t = self.trigger_for(g);
            for (i, r) in rows.iter().enumerate() {
                if t.inside(g, [r.x, r.y, r.z]) {
                    out[gi] = i as i32;
                    let e = group_row.entry(g.group).or_insert(i as i32);
                    if *e > i as i32 {
                        *e = i as i32;
                    }
                    break;
                }
            }
        }
        // a group already credited (-1) or credited in the rollout: keep only the
        // FIRST gate of a group credited in the rollout
        for (gi, g) in gates.gates.iter().enumerate() {
            if g.kind != GateKind::Finish && out[gi] >= 0 {
                if let Some(first) = group_row.get(&g.group) {
                    if *first >= 0 && out[gi] != *first {
                        out[gi] = -1;
                    }
                }
            }
        }
        // all checkpoint groups credited?
        let all_groups: std::collections::BTreeSet<u32> = gates.gates.iter().filter(|g| g.kind != GateKind::Finish).map(|g| g.group).collect();
        let armed_from: Option<i32> = if all_groups.iter().all(|grp| group_row.contains_key(grp)) {
            Some(all_groups.iter().map(|grp| group_row[grp]).max().unwrap_or(-1).max(0))
        } else {
            None
        };
        for (gi, g) in gates.gates.iter().enumerate() {
            if g.kind != GateKind::Finish || already.get(gi).copied().unwrap_or(false) {
                continue;
            }
            let Some(from) = armed_from else { continue };
            let t = self.trigger_for(g);
            for (i, r) in rows.iter().enumerate().skip(from as usize) {
                if t.inside(g, [r.x, r.y, r.z]) {
                    out[gi] = i as i32;
                    break;
                }
            }
        }
        out
    }

    /// JSON, for `detector.json` beside the dataset.
    pub fn to_json(&self) -> String {
        let mut s = String::from("{\n  \"detector_version\": 1,\n  \"frame\": \"gate frame: s along GEOM normal from centre, lat horizontal, up = y - centre.y; car CENTRE (CGameVehiclePhy state pos)\",\n");
        s.push_str(&format!("  \"provenance\": {},\n  \"default\": {},\n  \"per_model\": [\n", crate::json::quote(&self.provenance), trig_json(&self.default)));
        for (i, (m, t)) in self.per_model.iter().enumerate() {
            s.push_str(&format!("    {{\"model\": {}, \"trigger\": {}}}{}\n", crate::json::quote(m), trig_json(t), if i + 1 < self.per_model.len() { "," } else { "" }));
        }
        s.push_str("  ]\n}\n");
        s
    }

    pub fn from_json(txt: &str) -> Result<Detector, String> {
        let j = crate::json::parse(txt)?;
        let trig = |v: &Json| -> Result<Trigger, String> {
            Ok(Trigger {
                s_off: v.get("s_off").and_then(|x| x.f64()).ok_or("s_off")?,
                depth: v.get("depth").and_then(|x| x.f64()).ok_or("depth")?,
                lat_half: v.get("lat_half").and_then(|x| x.f64()).ok_or("lat_half")?,
                up_lo: v.get("up_lo").and_then(|x| x.f64()).ok_or("up_lo")?,
                up_hi: v.get("up_hi").and_then(|x| x.f64()).ok_or("up_hi")?,
            })
        };
        let mut per_model = Vec::new();
        for e in j.get("per_model").and_then(|v| v.arr()).ok_or("per_model")? {
            per_model.push((e.get("model").and_then(|v| v.str()).ok_or("model")?.to_string(), trig(e.get("trigger").ok_or("trigger")?)?));
        }
        Ok(Detector {
            per_model,
            default: trig(j.get("default").ok_or("default")?)?,
            provenance: j.get("provenance").and_then(|v| v.str()).unwrap_or("").to_string(),
        })
    }
}

fn trig_json(t: &Trigger) -> String {
    format!(
        "{{\"s_off\": {:.3}, \"depth\": {:.1}, \"lat_half\": {:.1}, \"up_lo\": {:.1}, \"up_hi\": {:.1}}}",
        t.s_off, t.depth, t.lat_half, t.up_lo, t.up_hi
    )
}
