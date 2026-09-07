//! Gate geometry from the map's waypoints, and the crossing detector.
//!
//! The detector's SHAPE is calibrated, not assumed (RL-agentG §5.1: no
//! geometric detector matched the oracle on a car spawned at the wrong
//! waypoint). `tmreach gatecal` measures, on the human runs' own engine
//! trajectories, where the car is at the tick the engine credits each
//! checkpoint; the candidate volumes below are graded against that and the
//! winning one is written to `detector.json` beside the dataset.

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
    pub cell: (i32, i32, i32),
    /// World centre: an item's own position, or the cell's world point.
    pub centre: [f64; 3],
    /// Block dir 0..3 (grid blocks) — the road axis; None for items.
    pub dir: Option<u8>,
    /// Item yaw, radians (items only).
    pub yaw: Option<f64>,
}

pub struct MapGates {
    pub map_uid: String,
    pub gates: Vec<Gate>,
    pub spawn: Option<Gate>,
    /// World y of cell row 0 for this map (tmmaps::map::ground_y).
    pub ground_y: f64,
}

pub fn kind_of(tag: &str) -> GateKind {
    match tag {
        "Spawn" | "Start" => GateKind::Start,
        "Goal" | "Finish" => GateKind::Finish,
        "StartFinish" | "Multilap" => GateKind::Multilap,
        _ => GateKind::Checkpoint,
    }
}

impl MapGates {
    pub fn load(map: &std::path::Path) -> Result<MapGates, String> {
        let mf = MapFile::try_load(map)?;
        // The same rule tmmaps::tiny uses: the collection is the items' collection
        // word (Stadium 26 when there are none).
        let collection = mf.items.first().map(|i| i.collection_raw).unwrap_or(26);
        let ground_y = tmmaps::map::ground_y(collection) as f64;
        let uid = map_uid(map);
        let mut gates = Vec::new();
        let mut spawn = None;
        for (i, w) in mf.waypoints().iter().enumerate() {
            let kind = kind_of(&w.tag);
            let (cx, cy, cz) = w.coords;
            let centre = match w.pos {
                Some(p) => [p[0] as f64, p[1] as f64, p[2] as f64],
                None => [32.0 * cx as f64 + 16.0, 8.0 * cy as f64 + ground_y + 2.0, 32.0 * cz as f64 + 16.0],
            };
            let g = Gate {
                waypoint: i as u32,
                kind,
                model: w.name.clone(),
                from_item: w.kind == Kind::Item,
                cell: w.coords,
                centre,
                dir: w.dir,
                yaw: if w.kind == Kind::Item { w.yaw.map(|y| y as f64) } else { None },
            };
            if kind == GateKind::Start {
                spawn = Some(g);
            } else {
                gates.push(g);
            }
        }
        Ok(MapGates { map_uid: uid, gates, spawn, ground_y })
    }

    /// Waypoint indices that can be credited (everything but the start).
    pub fn n_waypoints(&self) -> usize {
        self.gates.len() + self.spawn.is_some() as usize
    }
}

/// The map uid, from the file name when it is one (the bank names maps by uid).
pub fn map_uid(map: &std::path::Path) -> String {
    let stem = map.file_name().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
    stem.trim_end_matches(".Map.Gbx").to_string()
}

/// A candidate trigger volume, evaluated on a per-tick trajectory. Both the
/// shape and its parameters are what `gatecal` grades.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Volume {
    /// Axis-aligned box around the centre: half sizes (x, y, z) in metres.
    Box { hx: f64, hy: f64, hz: f64 },
    /// Horizontal disc of radius r with vertical half-height hy.
    Cylinder { r: f64, hy: f64 },
}

impl Volume {
    pub fn contains(&self, c: [f64; 3], p: [f64; 3]) -> bool {
        let d = [p[0] - c[0], p[1] - c[1], p[2] - c[2]];
        match *self {
            Volume::Box { hx, hy, hz } => d[0].abs() <= hx && d[1].abs() <= hy && d[2].abs() <= hz,
            Volume::Cylinder { r, hy } => (d[0] * d[0] + d[2] * d[2]).sqrt() <= r && d[1].abs() <= hy,
        }
    }
}

/// The calibrated detector: one volume per gate model (falls back to `default`).
#[derive(Clone, Debug)]
pub struct Detector {
    pub per_model: Vec<(String, Volume)>,
    pub default: Volume,
}

impl Detector {
    pub fn volume_for(&self, g: &Gate) -> Volume {
        self.per_model
            .iter()
            .find(|(m, _)| *m == g.model)
            .map(|(_, v)| *v)
            .unwrap_or(self.default)
    }

    /// First row index at which the car is inside each gate's volume, or -1.
    /// `already` marks gates credited before the rollout began (those are not
    /// re-credited: the engine credits each checkpoint once per lap).
    pub fn first_crossings(&self, gates: &MapGates, rows: &[forkoracle::layout::Row], already: &[bool]) -> Vec<i32> {
        let mut out = vec![-1i32; gates.gates.len()];
        for (gi, g) in gates.gates.iter().enumerate() {
            if already.get(gi).copied().unwrap_or(false) {
                continue;
            }
            let v = self.volume_for(g);
            for (i, r) in rows.iter().enumerate() {
                if v.contains(g.centre, [r.x, r.y, r.z]) {
                    out[gi] = i as i32;
                    break;
                }
            }
        }
        out
    }
}
