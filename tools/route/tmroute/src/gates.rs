//! `gates.json` — INTERFACES §3. Every gate of a map in world space, grouped into
//! the checkpoints the game actually counts, with the header's declared count
//! as the control.
//!
//! Sources, all from the `.Map.Gbx` alone:
//! * `tmmaps waypoints` — spawn, checkpoints, finish, tag, the waypoint ORDER
//!   word (the link group of a `LinkedCheckpoint`), cell / free position / yaw.
//! * the header's `nbCheckpoints` (0x03043002 v13 word 13) — checkpoints
//!   INCLUDING the finish.
//! * `yoff`, the map height: `world_y = 8 * cy + yoff`, estimated the
//!   cartographer's `cellmode` way (mode of `item.pos.y - 8 * item.cell.y`,
//!   which reads `yoff + 2` because items sit on the road 2 m above the cell
//!   base — cartographer INTERFACE.md §2/§5).
//!
//! # Grouping — how many checkpoints the map really has
//! 1. `LinkedCheckpoint` gates with the same `order` word are ONE checkpoint
//!    wherever they sit (that is what the tag means: any one of them validates
//!    the group). The cartographer dropped the tag entirely — 8 of the 25
//!    Summer 2026 maps carry it (finding F1, geom/STATUS.md).
//! 2. Same-tag gates within `GROUP_XZ` = 34 m in XZ and `GROUP_Y` = 6 m in Y are
//!    one checkpoint (the cartographer's rule and its two-sided control:
//!    Summer 2026 - 01 stays 3 → 3, Summer 2026 - 15's 16 Goal records → 1).
//!    A stacked pair at the same XZ (a two-piece expandable gate, 8 m apart in
//!    Y) is also one gate.
//! Control per map: checkpoint groups + 1 == declared. Printed, and stored.
//!
//! # Geometry conventions (measured, see geom/FINDINGS.md)
//! * Items: `pos` is the piece's own centre; a gate row `Left32m / Center8m /
//!   Right32m` has its pieces 20 m apart (16 + 4). Left→Right runs along
//!   `(-cos yaw, 0, sin yaw)`; the gate's normal axis is `(sin yaw, 0, cos yaw)`.
//! * Grid blocks: centre = cell centre in XZ, `y = 8*cy + yoff + 2` (road
//!   surface); `dir` 0|2 → normal along Z, 1|3 → along X (same axis rule with
//!   yaw 0 / π/2). Free ROAD/PLATFORM blocks: the position is the block's origin
//!   corner, so the road centre is local (16, 2, 16) through the full (yaw,
//!   pitch, roll) placement and the axis is local +Z — verified on Summer 2026 - 07
//!   (human crossings within 2–8 m of three tilted free pieces; a 2-cell diagonal
//!   piece is ~30 m off: its local centre is not (16, 2, 16), noted). Free GATE
//!   blocks (GateCheckpoint, GateExpandable*) keep their absolute position.
//! * The SIGN of the normal is not in the map file. `normal_source` says who
//!   settled it: "cartographer" (its tour tangent), "human" (the corpus'
//!   crossing direction), or "placement" (unsigned axis, +z/+x side).

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::Path;

pub const GROUP_XZ: f32 = 34.0;
pub const GROUP_Y: f32 = 6.0;
/// Two waypoint records at the same XZ (within this) are one stacked gate
/// whatever their Y gap: Summer 2026 - 16's `GateExpandableFinish` pair sits
/// 8 m apart vertically.
pub const STACK_XZ: f32 = 2.0;
/// Road surface above a grid cell's base (cartographer INTERFACE.md §2).
pub const ROAD_ABOVE_BASE: f32 = 2.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum WpKind {
    Checkpoint,
    Finish,
    Multilap,
    Start,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GateRec {
    /// Index in `tmmaps waypoints` (blocks first, then items).
    pub waypoint: u32,
    pub kind: WpKind,
    /// The file's tag: Spawn | Checkpoint | LinkedCheckpoint | Goal.
    pub tag: String,
    /// Link group (`LinkedCheckpoint`), 0 otherwise.
    pub link_order: u32,
    pub centre: [f32; 3],
    pub normal: [f32; 3],
    pub normal_source: String,
    pub half_width: f32,
    pub half_height: f32,
    pub model: String,
    pub from_item: bool,
    /// Checkpoint group id: all gates that fire the same checkpoint share it.
    /// Finish gates get their own groups after the checkpoints; spawn = u32::MAX.
    pub group: u32,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Spawn {
    pub pos: [f32; 3],
    pub yaw: f32,
    pub waypoint: u32,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GatesFile {
    pub map_uid: String,
    pub map_name: String,
    pub author_ms: i32,
    /// Header word: checkpoints including the finish. -1 if the header is older than v13.
    pub declared_checkpoints: i32,
    /// Header word: laps (1 = a plain race; 2+ = a lap race whose `StartFinish` line is crossed every lap).
    #[serde(default = "one")]
    pub laps: u32,
    /// Checkpoint groups (excluding finish groups).
    pub checkpoint_groups: u32,
    pub finish_groups: u32,
    pub yoff: f32,
    /// `cellmode` residual: the item mode minus yoff (expected +2).
    pub yoff_residual: f32,
    pub spawn: Spawn,
    pub gates: Vec<GateRec>,
    /// Transformation gates and car-state blocks (boosters, reactors, …) with geometry.
    #[serde(default)]
    pub specials: Vec<SpecialRec>,
    pub produced_by: String,
}

fn one() -> u32 {
    1
}

impl GatesFile {
    /// The control against the header: checkpoint groups + 1 for the lap line (`StartFinish`, when the map
    /// has one) + 1 for a plain finish (when the map has one) == declared. Winter 2026 - 10 (2 laps, no
    /// separate finish): 5 + 1 = 6; Fall 2024 - 24 (2 laps, lap line AND two finish lines): 5 + 1 + 1 = 7;
    /// an ordinary map: CPs + 1.
    pub fn control_ok(&self) -> bool {
        let has_lap_line = self.gates.iter().any(|g| g.kind == WpKind::Multilap);
        let has_finish = self.gates.iter().any(|g| g.kind == WpKind::Finish);
        let expected = self.checkpoint_groups as i32 + has_lap_line as i32 + has_finish as i32;
        self.declared_checkpoints >= 0 && expected == self.declared_checkpoints && self.finish_groups >= 1
    }
    /// Splits a FINISHED run declares: every checkpoint and the lap line each lap, the finish last —
    /// `laps × (checkpoint groups + 1)`; a plain race: checkpoints + 1 = the header count.
    pub fn expected_splits(&self) -> i32 {
        self.laps.max(1) as i32 * (self.checkpoint_groups as i32 + 1)
    }
    pub fn group_ids(&self) -> Vec<u32> {
        let mut g: Vec<u32> = self
            .gates
            .iter()
            .filter(|g| g.kind != WpKind::Start)
            .map(|g| g.group)
            .collect();
        g.sort_unstable();
        g.dedup();
        g
    }
    pub fn gates_of_group(&self, group: u32) -> Vec<&GateRec> {
        self.gates.iter().filter(|g| g.group == group).collect()
    }
    /// Representative gate of a group (the first record).
    pub fn group_rep(&self, group: u32) -> Option<&GateRec> {
        self.gates.iter().find(|g| g.group == group)
    }
    pub fn by_waypoint(&self, wp: u32) -> Option<&GateRec> {
        self.gates.iter().find(|g| g.waypoint == wp)
    }
    pub fn finish_group_ids(&self) -> Vec<u32> {
        let mut g: Vec<u32> = self
            .gates
            .iter()
            .filter(|g| matches!(g.kind, WpKind::Finish | WpKind::Multilap))
            .map(|g| g.group)
            .collect();
        g.sort_unstable();
        g.dedup();
        g
    }
    pub fn checkpoint_group_ids(&self) -> Vec<u32> {
        let mut g: Vec<u32> = self
            .gates
            .iter()
            .filter(|g| g.kind == WpKind::Checkpoint)
            .map(|g| g.group)
            .collect();
        g.sort_unstable();
        g.dedup();
        g
    }
    /// Group centroid and the mean half-width extent of the group (a row of
    /// pieces is one wide gate: extent = half the span between the outermost
    /// piece centres plus their half-widths).
    pub fn group_geometry(&self, group: u32) -> Option<([f32; 3], [f32; 3], f32)> {
        let gs = self.gates_of_group(group);
        if gs.is_empty() {
            return None;
        }
        let k = gs.len() as f32;
        let mut c = [0.0f32; 3];
        for g in &gs {
            for a in 0..3 {
                c[a] += g.centre[a] / k;
            }
        }
        let n = gs[0].normal;
        // extent along the row direction (perpendicular to the normal in XZ)
        let row = [-n[2], 0.0, n[0]];
        let mut lo = f32::INFINITY;
        let mut hi = f32::NEG_INFINITY;
        for g in &gs {
            let d = (g.centre[0] - c[0]) * row[0] + (g.centre[2] - c[2]) * row[2];
            lo = lo.min(d - g.half_width);
            hi = hi.max(d + g.half_width);
        }
        let hw = if gs.len() == 1 { gs[0].half_width } else { ((hi - lo) / 2.0).max(gs[0].half_width) };
        Some((c, n, hw))
    }
}

/// Half-width and half-height for a gate model. Measured where noted; the
/// human corpus' crossing offsets are the check (consensus prints them).
pub fn gate_dims(model: &str) -> (f32, f32) {
    let m = model;
    if m.contains("32m") {
        return (16.0, 4.0); // GateCheckpointLeft32m: pieces 20 m from the 8 m centre piece → 16
    }
    if m.contains("24m") {
        return (12.0, 4.0);
    }
    if m.contains("16m") {
        return (8.0, 4.0); // Right16m sits 24 m from Left32m: 16 + 8
    }
    if m.contains("8m") {
        return (4.0, 4.0);
    }
    if m.starts_with("GateExpandable") || m == "GateCheckpoint" || m == "GateFinish" || m == "GateSpecial" {
        return (16.0, 4.0);
    }
    if m.starts_with("Platform") {
        return (16.0, 4.0); // a full 32 m platform cell
    }
    if m.starts_with("Road") {
        return (12.5, 4.0); // GEN arm: humans cross road blocks out to |lat| 11.8 m, the walls sit at 12.75
    }
    (8.0, 4.0)
}

/// `cellmode`: mode of `item.pos.y - 8 * item.cell.y` over every item, snapped
/// to the 8 m cell row below it. Returns (yoff, residual). Falls back to the
/// waypoint items alone when the map has no items, and to -40 with residual NaN
/// when it has none of either.
pub fn yoff_cellmode(m: &tmmaps::map::MapFile) -> (f32, f32) {
    let mut hist: BTreeMap<i32, usize> = BTreeMap::new();
    for it in &m.items {
        let cy = it.coords().1;
        if !(0..=255).contains(&cy) {
            continue;
        }
        let d = (it.pos[1] - 8.0 * cy as f32).round() as i32;
        // a free-placed item carries a dead cell; nothing real is 200 m off a row
        if !(-200..=64).contains(&d) {
            continue;
        }
        *hist.entry(d).or_default() += 1;
    }
    let Some((&mode, _)) = hist.iter().max_by_key(|(k, v)| (**v, -**k)) else {
        return (-40.0, f32::NAN);
    };
    // The mode reads yoff + 2 (items sit on the road above the cell base); snap
    // to the multiple of 8 at or below mode - 2 + 4 (i.e. nearest row).
    let approx = mode as f32 - ROAD_ABOVE_BASE;
    let yoff = (approx / 8.0).round() * 8.0;
    (yoff, mode as f32 - yoff)
}

/// The file TAG decides (the header count agrees with it on 207/207 crawled maps): `StartFinish` is the
/// lap line — in a single-lap race the FINISH (Winter 2026 - 10: a `RoadTechMultilap` block beside a
/// plain `RoadTechStart`); a `GateMultilap*` ITEM tagged `Checkpoint` is a checkpoint (Fall 2024 - 24 has
/// two beside two real finishes, and its header counts them as checkpoints).
fn kind_of(tag: &str, _model: &str) -> WpKind {
    if tag == "StartFinish" {
        return WpKind::Multilap;
    }
    match tag {
        "Spawn" => WpKind::Start,
        "Goal" => WpKind::Finish,
        _ => WpKind::Checkpoint,
    }
}

/// Build the file from a map. `produced_by` is the provenance string.
pub fn build(path: &Path, produced_by: &str) -> Result<GatesFile, String> {
    let m = tmmaps::map::MapFile::load(path);
    let h = tmmaps::header::read(path.to_str().ok_or("path")?)?;
    let (yoff, resid) = yoff_cellmode(&m);
    let wps = m.waypoints();

    struct Raw {
        wp: u32,
        kind: WpKind,
        tag: String,
        order: u32,
        centre: [f32; 3],
        yaw: f32,
        /// Full 3-D gate axis when the placement is pitched/rolled (an item on a wall, a flat gate); else None.
        axis3: Option<[f32; 3]>,
        /// The placement's local UP in world (unit) when pitched/rolled; the arch centre is hh along it.
        up3: Option<[f32; 3]>,
        /// A wall-mounted item (|pitch| ≈ 90°): its plane convention is unsolved (normal_source "unknown-wall").
        wall: bool,
        model: String,
        from_item: bool,
        // a grid-placed block (cell coordinates, no free position)
        grid: bool,
    }
    let mut raws: Vec<Raw> = Vec::new();
    let mut parked = 0;
    for (i, w) in wps.iter().enumerate() {
        // A tiny map parks its original block waypoints at cell (0,0,0) under a
        // non-gate model (tmauto::synth::is_parked_waypoint): the game decides
        // what a waypoint does by its MODEL, so those are dead. Skip them.
        if is_parked(w) {
            parked += 1;
            continue;
        }
        let from_item = w.kind == tmmaps::map::Kind::Item;
        // A FREE block's position is its origin CORNER; the road centre of its
        // 1×1 cell is local (16, 2, 16) through the placement rotation, and the
        // road axis is local +Z through the same rotation (tilted pieces included).
        let mut free_axis: Option<[f32; 3]> = None;
        let mut free_up: Option<[f32; 3]> = None;
        let mut wall_item = false;
        // A road/platform piece's crossing point is its road centre (16, 2, 16); a gate
        // structure's is mid-arch (16, 8, 16) — Poland 2026's finish is a GateFinish rolled
        // −90° about its road axis (lying flat) and the car finishes 17 m BELOW its anchor,
        // exactly where the rotated local frame puts the arch.
        let road_piece = w.name.starts_with("Road") || w.name.starts_with("Platform");
        // A 2×2-cell DIAGONAL piece: the GEN arm measured Summer 2026 - 07's `RoadDirtDiagLeftCheckpoint`
        // crediting at local (46.7, 0.5, 30.7) — the road crosses the block at (48, ~0.5, 32); DiagRight is the
        // mirror (16, 0.5, 32) — a GUESS until a DiagRight gate is measured. Its axis runs along the diagonal.
        let diag = w.name.contains("Diag");
        let local_c = if diag && w.name.contains("DiagRight") { [16.0, 0.5, 32.0] } else if diag { [48.0, 0.5, 32.0] } else if road_piece { [16.0, ROAD_ABOVE_BASE, 16.0] } else { [16.0, 8.0, 16.0] };
        let local_axis: [f32; 3] = if diag && w.name.contains("DiagRight") { [0.7071, 0.0, -0.7071] } else if diag { [0.7071, 0.0, 0.7071] } else { [0.0, 0.0, 1.0] };
        let centre = match (w.pos, w.free_rot) {
            (Some(p), Some(rot)) => {
                let m = turned(p, rot);
                let c = apply(&m, local_c);
                let c2 = apply(&m, [local_c[0] + local_axis[0], local_c[1] + local_axis[1], local_c[2] + local_axis[2]]);
                free_axis = Some([c2[0] - c[0], c2[1] - c[1], c2[2] - c[2]]);
                c
            }
            (Some(p), None) => {
                // an ITEM: its trigger plane turns with the item's full (yaw, pitch, roll) — Fall 2025 - 12's
                // 32 m gates are pitched 66–83° on a wall section (GEN arm, engine-credited rows)
                if let Some(rot) = w.item_rot {
                    // a WALL placement (|pitch| ≈ 90°, Spring 2025 - 24 wp14) does not follow this composition — the
                    // engine-credited rows put its plane on the travel axis, not on the turned local +Z. Keep the
                    // horizontal yaw normal and flag it (normal_source "unknown-wall" set below) until the
                    // convention is solved from more wall cases.
                    let wall = (rot[1].abs() - std::f32::consts::FRAC_PI_2).abs() < 0.05;
                    wall_item = wall;
                    if !wall && (rot[1].abs() > 0.05 || rot[2].abs() > 0.05) {
                        let m = turned(p, rot);
                        let a = apply(&m, [0.0, 0.0, 1.0]);
                        free_axis = Some([a[0] - p[0], a[1] - p[1], a[2] - p[2]]);
                        let u = apply(&m, [0.0, 1.0, 0.0]);
                        free_up = Some([u[0] - p[0], u[1] - p[1], u[2] - p[2]]);
                    }
                }
                p
            }
            (None, _) => [
                32.0 * w.coords.0 as f32 + 16.0,
                8.0 * w.coords.1 as f32 + yoff + ROAD_ABOVE_BASE,
                32.0 * w.coords.2 as f32 + 16.0,
            ],
        };
        raws.push(Raw {
            wp: i as u32,
            kind: kind_of(&w.tag, &w.name),
            tag: w.tag.clone(),
            order: w.order,
            centre,
            // yaw such that (sin yaw, 0, cos yaw) is the road axis: a free block's rotated local axis, else the placement yaw
            yaw: match free_axis { Some(a) if (a[0] * a[0] + a[2] * a[2]).sqrt() > 0.3 => a[0].atan2(a[2]), Some(_) => w.yaw.unwrap_or(0.0), None => w.yaw.unwrap_or(0.0) },
            axis3: free_axis.filter(|a| a[1].abs() > 0.05),
            up3: free_up,
            wall: wall_item,
            model: w.name.clone(),
            from_item,
            grid: w.pos.is_none(),
        });
    }

    // --- grouping: union-find over non-spawn records
    let n = raws.len();
    let mut parent: Vec<usize> = (0..n).collect();
    fn find(p: &mut Vec<usize>, mut a: usize) -> usize {
        while p[a] != a {
            p[a] = p[p[a]];
            a = p[a];
        }
        a
    }
    for i in 0..n {
        for j in i + 1..n {
            let (a, b) = (&raws[i], &raws[j]);
            if a.kind == WpKind::Start || b.kind == WpKind::Start || a.kind != b.kind {
                continue;
            }
            let same_tag = a.tag == b.tag;
            let dx = a.centre[0] - b.centre[0];
            let dz = a.centre[2] - b.centre[2];
            let dxz = (dx * dx + dz * dz).sqrt();
            let dy = (a.centre[1] - b.centre[1]).abs();
            // order 0 is "unset": the tiny converter (2026-09-07) tags every piece LinkedCheckpoint with order 0 — those
            // are grouped by geometry like plain pieces, not all into one
            let linked = a.tag == "LinkedCheckpoint" && b.tag == "LinkedCheckpoint" && a.order == b.order && a.order != 0;
            // Two GRID blocks are two gates however close: Spring 2026 - 17 has
            // RoadBumpCheckpointSlopeUp/SlopeDown in adjacent cells (32 m apart) and
            // the header counts them as two checkpoints. Rows of one gate are built
            // from items or free blocks, and only those merge by distance.
            // A row of grid FINISH blocks is still one finish line (any finish ends the race;
            // Summer 2026 - 18 has six `Goal` blocks side by side); only CHECKPOINT grid
            // blocks are kept apart.
            // Distance merges rows of ITEMS / free blocks only (Fall 2025 - 24: a RoadTechCheckpoint
            // grid block 32 m from a gate item is a second checkpoint; the header says so).
            let both_placed = !a.grid && !b.grid;
            let is_finish = matches!(a.kind, WpKind::Finish | WpKind::Multilap);
            // a ROW is pieces of one kind (item–item or free–free) with parallel axes; Summer 2024 - 23 has a
            // free `GateCheckpoint` 22 m from an item `GateCheckpointLeft32m`, at right angles: two checkpoints
            // tiny maps: the converter names block-derived items AC…, original items AI… — a block and an item
            // 12 m apart were two checkpoints in the full-size map (Norway 2026) and stay two here
            let conv_class = |m: &str| -> String { if m.len() > 2 && m.starts_with('A') && (m.as_bytes()[1] == b'C' || m.as_bytes()[1] == b'I') && m.as_bytes()[2].is_ascii_digit() { m[..2].to_string() } else { "std".to_string() } };
            let same_regime = a.from_item == b.from_item && conv_class(&a.model) == conv_class(&b.model);
            let parallel = ((a.yaw - b.yaw).cos()).abs() > 0.9;
            let near = same_tag && ((both_placed && same_regime && parallel) || is_finish) && dxz <= GROUP_XZ && dy <= GROUP_Y;
            // two touching pieces are one gate whatever their tags (Fall 2024 - 24 has a
            // LinkedCheckpoint piece 2 m from a plain Checkpoint piece of the same row)
            let stacked = dxz <= STACK_XZ && dy <= GROUP_Y * 3.0;
            let mixed = both_placed && (a.tag == "LinkedCheckpoint") != (b.tag == "LinkedCheckpoint") && dxz <= 4.0 && dy <= GROUP_Y;
            if linked || near || stacked || mixed {
                let (ra, rb) = (find(&mut parent, i), find(&mut parent, j));
                if ra != rb {
                    parent[ra] = rb;
                }
            }
        }
    }
    // group ids: checkpoints first in order of first appearance, then finishes
    let mut gid: Vec<u32> = vec![u32::MAX; n];
    let mut next = 0u32;
    let mut root_to_gid: BTreeMap<usize, u32> = BTreeMap::new();
    for pass in [WpKind::Checkpoint, WpKind::Finish, WpKind::Multilap] {
        for i in 0..n {
            if raws[i].kind != pass {
                continue;
            }
            let r = find(&mut parent, i);
            let g = *root_to_gid.entry(r).or_insert_with(|| {
                let g = next;
                next += 1;
                g
            });
            gid[i] = g;
        }
    }
    let cp_groups = root_to_gid.iter().filter(|(r, _)| raws[**r].kind == WpKind::Checkpoint).count() as u32;
    let fin_groups = root_to_gid.iter().filter(|(r, _)| matches!(raws[**r].kind, WpKind::Finish | WpKind::Multilap)).count() as u32;

    let spawn_i = raws.iter().position(|r| r.kind == WpKind::Start).ok_or("map has no Spawn waypoint")?;
    let spawn = Spawn {
        pos: raws[spawn_i].centre,
        yaw: raws[spawn_i].yaw,
        waypoint: raws[spawn_i].wp,
    };

    let gates = raws
        .iter()
        .enumerate()
        .map(|(i, r)| {
            let (hw, hh) = gate_dims(&r.model);
            let (sy, cy) = r.yaw.sin_cos();
            GateRec {
                waypoint: r.wp,
                kind: r.kind,
                tag: r.tag.clone(),
                link_order: r.order,
                centre: match r.up3 { Some(u) => [r.centre[0] + hh * u[0], r.centre[1] + hh * u[1], r.centre[2] + hh * u[2]], None => [r.centre[0], r.centre[1] + hh, r.centre[2]] },
                // a pitched/rolled placement keeps its 3-D axis (unit); the sign is oriented later
                normal: match r.axis3 { Some(a) => { let l = (a[0] * a[0] + a[1] * a[1] + a[2] * a[2]).sqrt(); [a[0] / l, a[1] / l, a[2] / l] } None => [sy, 0.0, cy] },
                normal_source: if r.wall { "placement-wall".into() } else { "placement".into() },
                half_width: hw,
                half_height: hh,
                model: r.model.clone(),
                from_item: r.from_item,
                group: gid[i],
            }
        })
        .collect();

    let author_ms = h.authortime.trim().parse::<i32>().unwrap_or(-1);
    Ok(GatesFile {
        map_uid: h.uid.clone(),
        map_name: strip_fmt(&h.name),
        author_ms,
        declared_checkpoints: h.nb_checkpoints.map(|v| v as i32).unwrap_or(-1),
        laps: h.nb_laps.unwrap_or(1).max(1),
        checkpoint_groups: cp_groups,
        finish_groups: fin_groups,
        yoff,
        yoff_residual: resid,
        spawn,
        gates,
        specials: specials(&m, yoff),
        produced_by: format!("{produced_by}; parked block waypoints skipped: {parked}"),
    })
}

/// Same rule as `tmauto::synth::is_parked_waypoint`.
pub fn is_parked(w: &tmmaps::map::Waypoint) -> bool {
    w.kind == tmmaps::map::Kind::Block
        && w.coords == (0, 0, 0)
        && w.pos.is_none()
        && !w.name.ends_with("Start")
        && !w.name.ends_with("Checkpoint")
        && !w.name.ends_with("Finish")
        && !w.name.ends_with("Multilap")
}

/// Drop `$xxx` / `$o` style TM text formatting.
pub fn strip_fmt(s: &str) -> String {
    let b: Vec<char> = s.chars().collect();
    let mut out = String::new();
    let mut i = 0;
    while i < b.len() {
        if b[i] == '$' && i + 1 < b.len() {
            let c = b[i + 1];
            if c.is_ascii_hexdigit() && i + 3 < b.len() {
                i += 4;
                continue;
            }
            if c == '$' {
                out.push('$');
            }
            i += 2;
            continue;
        }
        out.push(b[i]);
        i += 1;
    }
    out
}

/// Orient every gate's normal along a reference direction (dot > 0), recording
/// the source. `dirs` maps waypoint → direction of travel.
/// Flip the sign of the gate normals whose arrival direction `dirs` opposes; `source` is recorded.
/// A gate already oriented by the ENGINE (GEN arm's credited rows, `orient_engine`) is never touched.
pub fn orient(g: &mut GatesFile, dirs: &BTreeMap<u32, [f32; 3]>, source: &str) -> usize {
    let mut n = 0;
    for gate in &mut g.gates {
        if gate.normal_source == "engine" {
            continue;
        }
        if let Some(d) = dirs.get(&gate.waypoint) {
            let dot = gate.normal[0] * d[0] + gate.normal[2] * d[2];
            if dot < 0.0 {
                gate.normal = [-gate.normal[0], -gate.normal[1], -gate.normal[2]];
            }
            gate.normal_source = source.to_string();
            n += 1;
        }
    }
    n
}

// ---------------------------------------------------------------------------
// placement maths — a copy of mapgeom::geom / mapgeom::place (yaw, then pitch,
// then roll about the already-turned axes), kept here so tmroute does not pull
// the pak reader in. x' = c·x + s·z ; z' = −s·x + c·z (clockwise looking down).
// ---------------------------------------------------------------------------
pub type Xform = [f32; 12];

pub fn apply(m: &Xform, v: [f32; 3]) -> [f32; 3] {
    [
        m[0] * v[0] + m[3] * v[1] + m[6] * v[2] + m[9],
        m[1] * v[0] + m[4] * v[1] + m[7] * v[2] + m[10],
        m[2] * v[0] + m[5] * v[1] + m[8] * v[2] + m[11],
    ]
}

fn compose(outer: &Xform, inner: &Xform) -> Xform {
    let mut out = [0f32; 12];
    for c in 0..3 {
        let col = [inner[c * 3], inner[c * 3 + 1], inner[c * 3 + 2]];
        out[c * 3] = outer[0] * col[0] + outer[3] * col[1] + outer[6] * col[2];
        out[c * 3 + 1] = outer[1] * col[0] + outer[4] * col[1] + outer[7] * col[2];
        out[c * 3 + 2] = outer[2] * col[0] + outer[5] * col[1] + outer[8] * col[2];
    }
    let t = apply(outer, [inner[9], inner[10], inner[11]]);
    out[9] = t[0];
    out[10] = t[1];
    out[11] = t[2];
    out
}

fn yaw_xf(angle: f32, t: [f32; 3]) -> Xform {
    let (s, c) = angle.sin_cos();
    [c, 0.0, -s, 0.0, 1.0, 0.0, s, 0.0, c, t[0], t[1], t[2]]
}

/// Position + (yaw, pitch, roll) as one transform — `mapgeom::place::turned`.
pub fn turned(pos: [f32; 3], rot: [f32; 3]) -> Xform {
    let m = yaw_xf(rot[0], pos);
    if rot[1] == 0.0 && rot[2] == 0.0 {
        return m;
    }
    let (sp, cp) = rot[1].sin_cos();
    let pitch = [1.0, 0.0, 0.0, 0.0, cp, sp, 0.0, -sp, cp, 0.0, 0.0, 0.0];
    let (sr, cr) = rot[2].sin_cos();
    let roll = [cr, sr, 0.0, -sr, cr, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0];
    compose(&compose(&m, &pitch), &roll)
}

// ---------------------------------------------------------------------------
// specials: transformation gates and car-state blocks (coordinator, 2026-09-07)
// ---------------------------------------------------------------------------

/// A gameplay placement the planner must see: a TRANSFORMATION gate (`car` set:
/// the leg after it is driven by that car) or a physics block (booster, turbo,
/// reactor, slow-motion, reset, no-engine, no-brake, no-steer, fragile, cruise,
/// bumper). Not a waypoint; nothing forces the route through it.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SpecialRec {
    /// `mapgeom::local::Special` name: "TransformSnow", "Boost2", "Turbo", "Reset", …
    pub kind: String,
    /// "Stadium" | "Snow" | "Rally" | "Desert" for a transformation gate.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub car: Option<String>,
    pub model: String,
    pub centre: [f32; 3],
    /// Unit axis through the gate / along the pad (sign unknown), XZ.
    pub axis: [f32; 3],
    pub half_width: f32,
    pub half_height: f32,
    pub from_item: bool,
    /// block index / item index in the map (see `tmmaps census`).
    pub index: u32,
}

/// Every gameplay placement of the map with its geometry. Grid blocks at their
/// cell centre (`y = 8·cy + yoff + 2`), free blocks through their placement
/// rotation, items at their position. A `GateExpandable*` structure is several
/// records (the pieces) — they are grouped by 34 m / 6 m into one record here,
/// like gate rows.
pub fn specials(m: &tmmaps::map::MapFile, yoff: f32) -> Vec<SpecialRec> {
    use mapgeom::local::Special;
    struct P {
        kind: Special,
        model: String,
        centre: [f32; 3],
        yaw: f32,
        from_item: bool,
        index: u32,
        hw: f32,
    }
    let mut ps: Vec<P> = Vec::new();
    let mut push = |kind: Special, model: &str, centre: [f32; 3], yaw: f32, from_item: bool, index: u32| {
        if kind == Special::None || kind.is_waypoint() {
            return;
        }
        let (hw, _) = gate_dims(model);
        ps.push(P { kind, model: model.to_string(), centre, yaw, from_item, index, hw });
    };
    for (baked, list) in [(false, &m.blocks), (true, &m.baked)] {
        for b in list.iter() {
            let kind = Special::of_name(&b.name);
            if kind == Special::None || kind.is_waypoint() {
                continue;
            }
            let free = b.flags & tmmaps::map::FREE_BLOCK_FLAG != 0;
            let (centre, yaw) = if free {
                match b.free_pos {
                    Some(p) => {
                        let rot = b.free_rot.unwrap_or([0.0; 3]);
                        let mm = turned(p, rot);
                        let c = apply(&mm, [16.0, ROAD_ABOVE_BASE, 16.0]);
                        let c2 = apply(&mm, [16.0, ROAD_ABOVE_BASE, 17.0]);
                        (c, (c2[0] - c[0]).atan2(c2[2] - c[2]))
                    }
                    None => continue,
                }
            } else {
                let (cx, cy, cz) = b.coords();
                ([32.0 * cx as f32 + 16.0, 8.0 * cy as f32 + yoff + ROAD_ABOVE_BASE, 32.0 * cz as f32 + 16.0], if b.dir & 1 == 0 { 0.0 } else { std::f32::consts::FRAC_PI_2 })
            };
            let _ = baked;
            push(kind, &b.name, centre, yaw, false, b.index as u32);
        }
    }
    for it in &m.items {
        let kind = Special::of_name(&it.model);
        push(kind, &it.model, it.pos, it.yaw, true, it.index as u32);
    }
    // group pieces of one structure: same kind within 34 m XZ / 6 m Y
    let n = ps.len();
    let mut parent: Vec<usize> = (0..n).collect();
    fn find(p: &mut Vec<usize>, mut a: usize) -> usize {
        while p[a] != a {
            p[a] = p[p[a]];
            a = p[a];
        }
        a
    }
    for i in 0..n {
        for j in i + 1..n {
            if ps[i].kind != ps[j].kind {
                continue;
            }
            let dx = ps[i].centre[0] - ps[j].centre[0];
            let dz = ps[i].centre[2] - ps[j].centre[2];
            let dxz = (dx * dx + dz * dz).sqrt();
            let dy = (ps[i].centre[1] - ps[j].centre[1]).abs();
            // a row (34 m / 6 m) or a vertical STACK of expandable pieces (same XZ, 8 m steps)
            if (dxz <= GROUP_XZ && dy <= GROUP_Y) || dxz <= STACK_XZ {
                let (a, b) = (find(&mut parent, i), find(&mut parent, j));
                if a != b {
                    parent[a] = b;
                }
            }
        }
    }
    let mut groups: BTreeMap<usize, Vec<usize>> = BTreeMap::new();
    for i in 0..n {
        let r = find(&mut parent, i);
        groups.entry(r).or_default().push(i);
    }
    groups
        .into_values()
        .map(|members| {
            let k = members.len() as f32;
            let mut c = [0.0f32; 3];
            let mut y_min = f32::INFINITY;
            for &i in &members {
                for a in 0..3 {
                    c[a] += ps[i].centre[a] / k;
                }
                y_min = y_min.min(ps[i].centre[1]);
            }
            // a vertical stack of pieces is driven through at its BOTTOM row
            c[1] = y_min;
            let first = &ps[members[0]];
            let (sy, cy) = first.yaw.sin_cos();
            let axis = [sy, 0.0, cy];
            // extent along the row
            let row = [-axis[2], 0.0, axis[0]];
            let (mut lo, mut hi) = (f32::INFINITY, f32::NEG_INFINITY);
            for &i in &members {
                let d = (ps[i].centre[0] - c[0]) * row[0] + (ps[i].centre[2] - c[2]) * row[2];
                lo = lo.min(d - ps[i].hw);
                hi = hi.max(d + ps[i].hw);
            }
            SpecialRec {
                kind: first.kind.name(),
                car: first.kind.car().map(|s| s.to_string()),
                model: first.model.clone(),
                centre: [c[0], c[1] + 4.0, c[2]],
                axis,
                half_width: if members.len() == 1 { first.hw } else { ((hi - lo) / 2.0).max(first.hw) },
                half_height: 4.0,
                from_item: first.from_item,
                index: first.index,
            }
        })
        .collect()
}

#[cfg(test)]
mod diag_tests {
    use super::*;
    /// GEN arm measurement (gen/g2/GEOM-GATE-FRAMES.md): Summer 2026 - 07 wp5 `RoadDirtDiagLeftCheckpoint`, a FREE
    /// block at anchor (1014, 101.067, 576) rot (−π, 0, 0.5236), credits its checkpoint with the car centre at
    /// world mean (973.85, 124.84, 545.27). Which LOCAL point is that? (rigid transform → R^T (g − t))
    #[test]
    fn diag_left_local_trigger() {
        let m = turned([1014.0, 101.067, 576.0], [-3.1416, 0.0, 0.5236]);
        let g = [973.85f32, 124.84, 545.27];
        let d = [g[0] - m[9], g[1] - m[10], g[2] - m[11]];
        // columns of R are m[0..3], m[3..6], m[6..9]; local = R^T d
        let local = [
            m[0] * d[0] + m[1] * d[1] + m[2] * d[2],
            m[3] * d[0] + m[4] * d[1] + m[5] * d[2],
            m[6] * d[0] + m[7] * d[1] + m[8] * d[2],
        ];
        eprintln!("DIAG local trigger = ({:.2}, {:.2}, {:.2})", local[0], local[1], local[2]);
        // and where (16, 2, 16) lands, for the record
        let c = apply(&m, [16.0, 2.0, 16.0]);
        eprintln!("(16,2,16) → ({:.2}, {:.2}, {:.2})", c[0], c[1], c[2]);
        assert!(local[0].is_finite());
    }
}

/// The GEN arm's engine-credited sign (gen/g2/flipped-normals.tsv: `uid \t map \t [wp, wp, …]`): the listed
/// waypoints are crossed AGAINST the PLACEMENT normal, so this takes a freshly built (placement-signed) file,
/// flips them, and marks every gate `normal_source = "engine"` — the authority over human and cartographer signs.
pub fn orient_engine(g: &mut GatesFile, flipped: &[u32]) -> usize {
    let mut n = 0;
    for gate in &mut g.gates {
        if gate.kind == WpKind::Start {
            continue;
        }
        if flipped.contains(&gate.waypoint) {
            gate.normal = [-gate.normal[0], -gate.normal[1], -gate.normal[2]];
            n += 1;
        }
        gate.normal_source = "engine".to_string();
    }
    n
}

/// Parse `flipped-normals.tsv` → uid → flipped waypoints.
pub fn read_flips(path: &std::path::Path) -> Result<BTreeMap<String, Vec<u32>>, String> {
    let s = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let mut out = BTreeMap::new();
    for line in s.lines() {
        let cols: Vec<&str> = line.split('\t').collect();
        if cols.len() < 3 || cols[0] == "uid" {
            continue;
        }
        let wps: Vec<u32> = cols[2].trim_matches(|c| c == '[' || c == ']').split(',').filter_map(|x| x.trim().parse().ok()).collect();
        out.insert(cols[0].to_string(), wps);
    }
    Ok(out)
}
