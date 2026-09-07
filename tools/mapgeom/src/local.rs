//! `LocalScene` — the map's collision triangles, TAGGED with the placement they
//! came from, behind a 3-D grid: `raycast`, `layers_below_above`, and a
//! `physics_special` flag per hit. Written for the reachability model R (the
//! MODEL arm casts ~100 rays per sample); INTERFACES.md §4.
//!
//! What it answers that the plumb grid (`surf`) cannot: walls, tunnels, loops,
//! wallrides, stacked roads, and WHICH block a surface belongs to — a booster
//! pad, a reactor, a checkpoint arch, a decoration stand.
//!
//! Sources: the map's grid blocks, free blocks, BAKED blocks (terrain and clips
//! the editor generates — `surf` never had them), items, and the decoration's own
//! map. Placement transforms are `assemble`'s / `place`'s. Materials are the
//! pak's physics ids (`scene::physics_name`); `NotCollidable` and `OffZone`
//! triangles are kept but flagged, so a caller can ignore or use them.
//!
//! Build cost: the assembly (~10–60 s per map, dominated by the pak reads) plus
//! the grid (~1 s per million triangles). `save`/`load` keep a built scene as a
//! flat little-endian file (`LSC1`) so a training loop pays the assembly once.

use crate::assemble::Assembler;
use crate::geom::{apply, Xform};
use crate::place;
use crate::scene::{physics_id, Scene};
use tmmaps::map::{MapFile, FREE_BLOCK_FLAG};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum PlacementKind {
    Block = 0,
    FreeBlock = 1,
    Baked = 2,
    Item = 3,
    Deco = 4,
}

/// Gameplay blocks that change the CAR, read off the placement's model name.
/// The name is the only thing the pak gives us for this; the list is the
/// TM2020 block/item vocabulary (`GateGameplay*`, `*Special*`, `Turbo*`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum Special {
    None = 0,
    Boost = 1,
    Boost2 = 2,
    Turbo = 3,
    Turbo2 = 4,
    Reactor = 5,
    ReactorDown = 6,
    SlowMotion = 7,
    Reset = 8,
    NoEngine = 9,
    Cruise = 10,
    NoBrake = 11,
    NoSteer = 12,
    Fragile = 13,
    Checkpoint = 14,
    Finish = 15,
    Start = 16,
    Multilap = 17,
    Bumper = 18,
    /// Car-change gates (`GateExpandableGameplaySnow`, `GateGameplaySnow24m`, `SnowGateGameplay`).
    TransformStadium = 19,
    TransformSnow = 20,
    TransformRally = 21,
    TransformDesert = 22,
}

impl Special {
    pub fn from_u8(v: u8) -> Special {
        use Special::*;
        match v {
            1 => Boost, 2 => Boost2, 3 => Turbo, 4 => Turbo2, 5 => Reactor, 6 => ReactorDown, 7 => SlowMotion,
            8 => Reset, 9 => NoEngine, 10 => Cruise, 11 => NoBrake, 12 => NoSteer, 13 => Fragile,
            14 => Checkpoint, 15 => Finish, 16 => Start, 17 => Multilap, 18 => Bumper,
            19 => TransformStadium, 20 => TransformSnow, 21 => TransformRally, 22 => TransformDesert, _ => None,
        }
    }
    /// Model name → special. Order matters: "Boost2" before "Boost", "ReactorDown" before "Reactor".
    pub fn of_name(name: &str) -> Special {
        let n = name;
        let has = |s: &str| n.contains(s);
        // car-change gates: "Gameplay" + the car. `GateExpandableGameplayVFC/FCT/InPillarFCB` are
        // structural pieces of the same gate and carry no car → None.
        if has("Gameplay") {
            if has("Snow") { return Special::TransformSnow; }
            if has("Rally") { return Special::TransformRally; }
            if has("Desert") { return Special::TransformDesert; }
            if has("Stadium") { return Special::TransformStadium; }
        }
        if has("Boost2") { return Special::Boost2; }
        if has("Boost") { return Special::Boost; }
        if has("Turbo2") { return Special::Turbo2; }
        if has("Turbo") { return Special::Turbo; }
        if has("ReactorDown") { return Special::ReactorDown; }
        if has("Reactor") { return Special::Reactor; }
        if has("SlowMotion") || has("Slowmo") { return Special::SlowMotion; }
        if has("Reset") { return Special::Reset; }
        if has("NoEngine") { return Special::NoEngine; }
        if has("Cruise") { return Special::Cruise; }
        if has("NoBrake") { return Special::NoBrake; }
        if has("NoSteer") { return Special::NoSteer; }
        if has("Fragile") { return Special::Fragile; }
        if has("Bumper") { return Special::Bumper; }
        if has("Multilap") { return Special::Multilap; }
        if has("Checkpoint") { return Special::Checkpoint; }
        if has("Finish") || has("Goal") { return Special::Finish; }
        // `PlatformTechLoopStart` is a LOOP piece, not a start
        if (n.ends_with("Start") || has("StartBlock")) && !has("Loop") { return Special::Start; }
        Special::None
    }
}

/// The block/item FAMILY: the model name with its variant suffix trimmed to the
/// family stem the MODEL arm buckets on — `RoadTechCurve2` → `RoadTech`,
/// `PlatformDirtLoopStart` → `PlatformDirt`, `GateCheckpointLeft32m` →
/// `GateCheckpoint`, `DecoWallSlope2UTop` → `DecoWall`. Rule: the leading
/// CamelCase words up to and including the first surface word
/// (Tech|Dirt|Ice|Bump|Grass|Water|Platform…) or, for gates/deco, the first two
/// words. Unknown shapes keep the first two CamelCase words.
pub fn family_of(name: &str) -> String {
    let base = name.trim_end_matches(".Item.Gbx").trim_end_matches(".Block.Gbx");
    let base = base.rsplit(['/', '\\']).next().unwrap_or(base);
    // digits are variants (`LandHill2`, `SupportTruss64m`, `Dirt0Dirt4`), never family
    let strip = |w: String| -> String {
        // `Truss64m`: a metre suffix after digits goes with the digits
        let w = if w.ends_with('m') && w.len() >= 2 && w.as_bytes()[w.len() - 2].is_ascii_digit() { w[..w.len() - 1].to_string() } else { w };
        w.chars().filter(|c| !c.is_ascii_digit()).collect()
    };
    let mut words: Vec<String> = camel_words(base).into_iter().map(strip).filter(|w| !w.is_empty()).collect();
    words.dedup();
    if words.is_empty() {
        return base.chars().filter(|c| !c.is_ascii_digit()).collect();
    }
    const SURF: [&str; 12] = ["Tech", "Dirt", "Ice", "Bump", "Grass", "Water", "Sand", "Wood", "Snow", "Plastic", "Rock", "Magnet"];
    let mut out = String::new();
    for (i, w) in words.iter().enumerate() {
        out.push_str(w);
        if SURF.contains(&w.as_str()) || i == 1 {
            break;
        }
    }
    out
}

fn camel_words(s: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let mut cur = String::new();
    for c in s.chars() {
        if c.is_ascii_uppercase() && !cur.is_empty() && !cur.chars().last().unwrap().is_ascii_uppercase() {
            out.push(std::mem::take(&mut cur));
        }
        if c.is_ascii_alphanumeric() {
            cur.push(c);
        } else if !cur.is_empty() {
            out.push(std::mem::take(&mut cur));
        }
    }
    if !cur.is_empty() {
        out.push(cur);
    }
    out
}

#[derive(Clone, Debug)]
pub struct Placement {
    pub name: String,
    pub family: String,
    pub kind: PlacementKind,
    pub special: Special,
    /// The map's own index of the record (block/baked/item index; u32::MAX for decoration).
    pub index: u32,
}

#[derive(Clone, Copy, Debug)]
pub struct Tri {
    pub v: [[f32; 3]; 3],
    /// Physics material id (`scene::physics_name`); 255 = unknown.
    pub mat: u8,
    /// Index into `LocalScene::placements`.
    pub tag: u32,
}

#[derive(Clone, Debug)]
pub struct Hit {
    pub dist: f32,
    pub point: [f32; 3],
    /// Unit normal, oriented AGAINST the ray (facing the origin).
    pub normal: [f32; 3],
    pub material: u8,
    pub material_name: &'static str,
    /// The triangle's placement.
    pub placement: u32,
    pub family: String,
    pub kind: PlacementKind,
    pub special: Special,
    /// `NotCollidable` / `OffZone`: a surface the car does not rest on.
    pub collidable: bool,
}

pub struct LocalScene {
    pub tris: Vec<Tri>,
    pub placements: Vec<Placement>,
    pub cell: f32,
    pub origin: [f32; 3],
    pub dims: [usize; 3],
    /// Per cell: triangle indices (CSR).
    pub cell_start: Vec<u32>,
    pub cell_tris: Vec<u32>,
}

pub struct BuildOpts {
    pub with_deco: bool,
    pub with_baked: bool,
    pub cell: f32,
}

impl Default for BuildOpts {
    fn default() -> Self {
        BuildOpts { with_deco: true, with_baked: true, cell: 4.0 }
    }
}

fn mat_id(name: &str) -> u8 {
    if let Some(id) = physics_id(name) {
        return id;
    }
    // "Unknown(37)"
    name.trim_start_matches("Unknown(").trim_end_matches(')').parse().unwrap_or(255)
}

fn push_scene(tris: &mut Vec<Tri>, s: &Scene, xf: &Xform, tag: u32) {
    for (mat_name, g) in &s.groups {
        let mat = mat_id(mat_name);
        for t in &g.tris {
            let a = apply(xf, g.verts[t[0] as usize]);
            let b = apply(xf, g.verts[t[1] as usize]);
            let c = apply(xf, g.verts[t[2] as usize]);
            tris.push(Tri { v: [a, b, c], mat, tag });
        }
    }
}

impl LocalScene {
    /// Assemble and index a map. `yoff` is the map height (`yoff::measure`, or
    /// tmroute's cellmode); `store` holds the pak(s).
    pub fn build(store: &mut crate::store::DataStore, m: &MapFile, yoff: f32, o: &BuildOpts) -> LocalScene {
        let mut tris: Vec<Tri> = Vec::new();
        let mut placements: Vec<Placement> = Vec::new();
        let mut asm = Assembler::new(store);
        let _ = asm.with_embedded(m);
        Self::assemble_into(&mut asm, m, yoff, o.with_baked, false, &mut tris, &mut placements);
        if o.with_deco {
            // the decoration is its own map, placed at the same yoff
            if let Some(name) = crate::assemble::decoration_id_pub(m) {
                let deco = ["Base", ""]
                    .iter()
                    .map(|p| format!("Stadium\\GameCtnDecoration\\{}{}.Decoration.Gbx", p, name))
                    .find(|p| asm.store.resolve(p).is_some());
                if let Some(deco) = deco {
                    if let Ok(model) = asm.store.load_model(&deco) {
                        if let Some(deco_map) = model.refs_ending(".Map.Gbx").into_iter().next() {
                            if let Ok(bytes) = asm.store.read(&deco_map) {
                                let inner = MapFile::from_gbx(tmmaps::gbx::Gbx::parse(&bytes));
                                Self::assemble_into(&mut asm, &inner, yoff, o.with_baked, true, &mut tris, &mut placements);
                            }
                        }
                    }
                }
            }
        }
        Self::index(tris, placements, o.cell)
    }

    fn assemble_into(asm: &mut Assembler, m: &MapFile, yoff: f32, with_baked: bool, deco: bool, tris: &mut Vec<Tri>, placements: &mut Vec<Placement>) {
        let kind_of = |free: bool, baked: bool| {
            if deco { PlacementKind::Deco } else if baked { PlacementKind::Baked } else if free { PlacementKind::FreeBlock } else { PlacementKind::Block }
        };
        let lists: Vec<(bool, &Vec<tmmaps::map::BlockRec>)> = if with_baked { vec![(false, &m.blocks), (true, &m.baked)] } else { vec![(false, &m.blocks)] };
        for (baked, list) in lists {
            for b in list.iter() {
                let free = b.flags & FREE_BLOCK_FLAG != 0;
                let size = match asm.block_model(&b.name) {
                    Some(lm) => lm.size,
                    None => continue,
                };
                let xf: Xform = if free {
                    match (b.free_pos, b.free_rot) {
                        (Some(p), Some(r)) => place::free(p, r),
                        (Some(p), None) => place::free(p, [0.0; 3]),
                        _ => continue,
                    }
                } else {
                    place::grid_block(b.coords(), b.dir, size, yoff)
                };
                let tag = placements.len() as u32;
                placements.push(Placement { name: b.name.clone(), family: family_of(&b.name), kind: kind_of(free, baked), special: Special::of_name(&b.name), index: if deco { u32::MAX } else { b.index as u32 } });
                if let Some(lm) = asm.block_model(&b.name) {
                    push_scene(tris, &lm.scene, &xf, tag);
                }
            }
        }
        for it in &m.items {
            if asm.item_model(&it.model).is_none() {
                continue;
            }
            let xf = place::anchored(it.pos, [it.yaw, it.pitch, it.roll], it.pivot, it.scale);
            let tag = placements.len() as u32;
            placements.push(Placement { name: it.model.clone(), family: family_of(&it.model), kind: if deco { PlacementKind::Deco } else { PlacementKind::Item }, special: Special::of_name(&it.model), index: if deco { u32::MAX } else { it.index as u32 } });
            if let Some(lm) = asm.item_model(&it.model) {
                push_scene(tris, &lm.scene, &xf, tag);
            }
        }
    }

    /// Grid the triangles (every cell a triangle's bounding box touches).
    pub fn index(tris: Vec<Tri>, placements: Vec<Placement>, cell: f32) -> LocalScene {
        let mut lo = [f32::INFINITY; 3];
        let mut hi = [f32::NEG_INFINITY; 3];
        for t in &tris {
            for v in &t.v {
                for a in 0..3 {
                    lo[a] = lo[a].min(v[a]);
                    hi[a] = hi[a].max(v[a]);
                }
            }
        }
        if tris.is_empty() {
            lo = [0.0; 3];
            hi = [1.0; 3];
        }
        let origin = [lo[0] - 1.0, lo[1] - 1.0, lo[2] - 1.0];
        let dims = [0, 1, 2].map(|a| (((hi[a] + 1.0 - origin[a]) / cell).ceil() as usize).max(1));
        let n_cells = dims[0] * dims[1] * dims[2];
        // two passes: count, then fill
        let mut count = vec![0u32; n_cells + 1];
        let cell_range = |t: &Tri| -> ([usize; 3], [usize; 3]) {
            let mut a = [usize::MAX; 3];
            let mut b = [0usize; 3];
            for v in &t.v {
                for k in 0..3 {
                    let c = (((v[k] - origin[k]) / cell).floor() as isize).clamp(0, dims[k] as isize - 1) as usize;
                    a[k] = a[k].min(c);
                    b[k] = b[k].max(c);
                }
            }
            (a, b)
        };
        let idx = |x: usize, y: usize, z: usize| (z * dims[1] + y) * dims[0] + x;
        for t in &tris {
            let (a, b) = cell_range(t);
            for z in a[2]..=b[2] {
                for y in a[1]..=b[1] {
                    for x in a[0]..=b[0] {
                        count[idx(x, y, z) + 1] += 1;
                    }
                }
            }
        }
        for i in 0..n_cells {
            count[i + 1] += count[i];
        }
        let mut fill = count.clone();
        let mut cell_tris = vec![0u32; count[n_cells] as usize];
        for (ti, t) in tris.iter().enumerate() {
            let (a, b) = cell_range(t);
            for z in a[2]..=b[2] {
                for y in a[1]..=b[1] {
                    for x in a[0]..=b[0] {
                        let c = idx(x, y, z);
                        cell_tris[fill[c] as usize] = ti as u32;
                        fill[c] += 1;
                    }
                }
            }
        }
        LocalScene { tris, placements, cell, origin, dims, cell_start: count, cell_tris }
    }

    pub fn tri_count(&self) -> usize {
        self.tris.len()
    }

    fn cell_of(&self, p: [f32; 3]) -> Option<[isize; 3]> {
        let mut c = [0isize; 3];
        for k in 0..3 {
            c[k] = ((p[k] - self.origin[k]) / self.cell).floor() as isize;
            if c[k] < 0 || c[k] >= self.dims[k] as isize {
                return None;
            }
        }
        Some(c)
    }

    fn hit_of(&self, ti: u32, dist: f32, origin: [f32; 3], dir: [f32; 3]) -> Hit {
        let t = &self.tris[ti as usize];
        let e1 = sub(t.v[1], t.v[0]);
        let e2 = sub(t.v[2], t.v[0]);
        let mut n = norm(cross(e1, e2));
        if dot(n, dir) > 0.0 {
            n = [-n[0], -n[1], -n[2]];
        }
        let p = &self.placements[t.tag as usize];
        let mname = crate::scene::physics_name(t.mat);
        Hit {
            dist,
            point: [origin[0] + dir[0] * dist, origin[1] + dir[1] * dist, origin[2] + dir[2] * dist],
            normal: n,
            material: t.mat,
            material_name: mname,
            placement: t.tag,
            family: p.family.clone(),
            kind: p.kind,
            special: p.special,
            collidable: crate::scene::is_collidable(mname),
        }
    }

    /// Nearest triangle along `dir` from `origin` within `max_m`. `dir` need not
    /// be unit. Both faces of every triangle count. `skip_noncollidable` drops
    /// NotCollidable/OffZone surfaces (the car passes through them).
    pub fn raycast(&self, origin: [f32; 3], dir: [f32; 3], max_m: f32, skip_noncollidable: bool) -> Option<Hit> {
        let d = norm(dir);
        self.raycast_raw(origin, dir, max_m, skip_noncollidable).map(|(dist, ti)| self.hit_of(ti, dist, origin, d))
    }

    /// The nearest surface straight below and straight above `p`, within `reach`.
    pub fn layers_below_above(&self, p: [f32; 3], reach: f32, skip_noncollidable: bool) -> (Option<Hit>, Option<Hit>) {
        (self.raycast(p, [0.0, -1.0, 0.0], reach, skip_noncollidable), self.raycast(p, [0.0, 1.0, 0.0], reach, skip_noncollidable))
    }

    /// Slab clip of the ray against the grid box → (t_enter, t_exit).
    fn clip(&self, o: [f32; 3], d: [f32; 3]) -> Option<(f32, f32)> {
        let mut t0 = f32::NEG_INFINITY;
        let mut t1 = f32::INFINITY;
        for k in 0..3 {
            let lo = self.origin[k];
            let hi = self.origin[k] + self.dims[k] as f32 * self.cell;
            if d[k].abs() < 1e-9 {
                if o[k] < lo || o[k] > hi {
                    return None;
                }
                continue;
            }
            let a = (lo - o[k]) / d[k];
            let b = (hi - o[k]) / d[k];
            t0 = t0.max(a.min(b));
            t1 = t1.min(a.max(b));
        }
        (t0 <= t1).then_some((t0, t1))
    }

    // ------------------------------------------------------------------ cache
    /// `LSC1` little-endian: header, placements (name table), triangles, grid.
    pub fn save(&self, path: &std::path::Path) -> std::io::Result<()> {
        use std::io::Write;
        let mut w = std::io::BufWriter::new(std::fs::File::create(path)?);
        w.write_all(b"LSC1")?;
        w.write_all(&(self.placements.len() as u32).to_le_bytes())?;
        w.write_all(&(self.tris.len() as u32).to_le_bytes())?;
        w.write_all(&self.cell.to_le_bytes())?;
        for v in self.origin {
            w.write_all(&v.to_le_bytes())?;
        }
        for d in self.dims {
            w.write_all(&(d as u32).to_le_bytes())?;
        }
        for p in &self.placements {
            let nb = p.name.as_bytes();
            w.write_all(&(nb.len() as u16).to_le_bytes())?;
            w.write_all(nb)?;
            w.write_all(&[p.kind as u8, p.special as u8])?;
            w.write_all(&p.index.to_le_bytes())?;
        }
        for t in &self.tris {
            for v in &t.v {
                for c in v {
                    w.write_all(&c.to_le_bytes())?;
                }
            }
            w.write_all(&[t.mat])?;
            w.write_all(&t.tag.to_le_bytes())?;
        }
        w.write_all(&(self.cell_start.len() as u32).to_le_bytes())?;
        for c in &self.cell_start {
            w.write_all(&c.to_le_bytes())?;
        }
        w.write_all(&(self.cell_tris.len() as u32).to_le_bytes())?;
        for c in &self.cell_tris {
            w.write_all(&c.to_le_bytes())?;
        }
        w.flush()
    }

    pub fn load(path: &std::path::Path) -> Result<LocalScene, String> {
        let b = std::fs::read(path).map_err(|e| format!("{}: {e}", path.display()))?;
        let mut o = 0usize;
        let take = |o: &mut usize, n: usize| -> Result<&[u8], String> {
            if *o + n > b.len() {
                return Err("truncated LSC1".into());
            }
            let s = &b[*o..*o + n];
            *o += n;
            Ok(s)
        };
        if take(&mut o, 4)? != b"LSC1" {
            return Err("not an LSC1 file".into());
        }
        let u32_at = |o: &mut usize| -> Result<u32, String> { Ok(u32::from_le_bytes(take(o, 4)?.try_into().unwrap())) };
        let f32_at = |o: &mut usize| -> Result<f32, String> { Ok(f32::from_le_bytes(take(o, 4)?.try_into().unwrap())) };
        let np = u32_at(&mut o)? as usize;
        let nt = u32_at(&mut o)? as usize;
        let cell = f32_at(&mut o)?;
        let origin = [f32_at(&mut o)?, f32_at(&mut o)?, f32_at(&mut o)?];
        let dims = [u32_at(&mut o)? as usize, u32_at(&mut o)? as usize, u32_at(&mut o)? as usize];
        let mut placements = Vec::with_capacity(np);
        for _ in 0..np {
            let nl = u16::from_le_bytes(take(&mut o, 2)?.try_into().unwrap()) as usize;
            let name = String::from_utf8_lossy(take(&mut o, nl)?).to_string();
            let kb = take(&mut o, 2)?;
            let kind = match kb[0] { 0 => PlacementKind::Block, 1 => PlacementKind::FreeBlock, 2 => PlacementKind::Baked, 3 => PlacementKind::Item, _ => PlacementKind::Deco };
            let special = Special::from_u8(kb[1]);
            let index = u32_at(&mut o)?;
            placements.push(Placement { family: family_of(&name), name, kind, special, index });
        }
        let mut tris = Vec::with_capacity(nt);
        for _ in 0..nt {
            let mut v = [[0f32; 3]; 3];
            for vv in v.iter_mut() {
                for c in vv.iter_mut() {
                    *c = f32_at(&mut o)?;
                }
            }
            let mat = take(&mut o, 1)?[0];
            let tag = u32_at(&mut o)?;
            tris.push(Tri { v, mat, tag });
        }
        let ncs = u32_at(&mut o)? as usize;
        let mut cell_start = Vec::with_capacity(ncs);
        for _ in 0..ncs {
            cell_start.push(u32_at(&mut o)?);
        }
        let nct = u32_at(&mut o)? as usize;
        let mut cell_tris = Vec::with_capacity(nct);
        for _ in 0..nct {
            cell_tris.push(u32_at(&mut o)?);
        }
        Ok(LocalScene { tris, placements, cell, origin, dims, cell_start, cell_tris })
    }
}

// ---------------------------------------------------------------- vector maths
fn sub(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}
fn cross(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]]
}
fn dot(a: [f32; 3], b: [f32; 3]) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}
fn norm(v: [f32; 3]) -> [f32; 3] {
    let l = dot(v, v).sqrt();
    if l < 1e-12 {
        [f32::NAN; 3]
    } else {
        [v[0] / l, v[1] / l, v[2] / l]
    }
}

/// Möller–Trumbore, both faces. Returns the distance along `d` (unit) or None.
pub fn ray_tri(o: [f32; 3], d: [f32; 3], v: &[[f32; 3]; 3]) -> Option<f32> {
    let e1 = sub(v[1], v[0]);
    let e2 = sub(v[2], v[0]);
    let p = cross(d, e2);
    let det = dot(e1, p);
    if det.abs() < 1e-9 {
        return None;
    }
    let inv = 1.0 / det;
    let s = sub(o, v[0]);
    let u = dot(s, p) * inv;
    if u < -1e-6 || u > 1.0 + 1e-6 {
        return None;
    }
    let q = cross(s, e1);
    let w = dot(d, q) * inv;
    if w < -1e-6 || u + w > 1.0 + 1e-6 {
        return None;
    }
    let t = dot(e2, q) * inv;
    (t > 1e-6).then_some(t)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn square(y: f32, tag: u32, mat: u8) -> Vec<Tri> {
        vec![
            Tri { v: [[0.0, y, 0.0], [10.0, y, 0.0], [10.0, y, 10.0]], mat, tag },
            Tri { v: [[0.0, y, 0.0], [10.0, y, 10.0], [0.0, y, 10.0]], mat, tag },
        ]
    }

    #[test]
    fn layers_and_rays() {
        let mut tris = square(0.0, 0, 16); // asphalt floor
        tris.extend(square(8.0, 1, 4)); // metal ceiling
        let pl = vec![
            Placement { name: "RoadTechStraight".into(), family: "RoadTech".into(), kind: PlacementKind::Block, special: Special::None, index: 0 },
            Placement { name: "RoadTechSpecialTurbo".into(), family: "RoadTech".into(), kind: PlacementKind::Block, special: Special::of_name("RoadTechSpecialTurbo"), index: 1 },
        ];
        let s = LocalScene::index(tris, pl, 4.0);
        let (below, above) = s.layers_below_above([5.0, 3.0, 5.0], 50.0, true);
        let b = below.unwrap();
        assert!((b.dist - 3.0).abs() < 1e-4 && b.material_name == "Asphalt" && b.family == "RoadTech");
        let a = above.unwrap();
        assert!((a.dist - 5.0).abs() < 1e-4 && a.material_name == "Metal" && a.special == Special::Turbo);
        assert!(a.normal[1] < 0.0, "normal faces the ray origin");
        // a ray that misses
        assert!(s.raycast([5.0, 3.0, 5.0], [1.0, 0.0, 0.0], 100.0, true).is_none());
        // a slanted ray hits the floor
        let h = s.raycast([5.0, 3.0, 5.0], [1.0, -1.0, 0.0], 100.0, true).unwrap();
        assert!((h.point[1]).abs() < 1e-3 && (h.dist - 3.0 * 2f32.sqrt()).abs() < 1e-3);
        // save / load round trip
        let p = std::env::temp_dir().join("lsc1-test.bin");
        s.save(&p).unwrap();
        let s2 = LocalScene::load(&p).unwrap();
        assert_eq!(s2.tris.len(), 4);
        assert_eq!(s2.placements[1].special, Special::Turbo);
        let h2 = s2.raycast([5.0, 3.0, 5.0], [0.0, -1.0, 0.0], 50.0, true).unwrap();
        assert!((h2.dist - 3.0).abs() < 1e-4);
    }

    #[test]
    fn families_and_specials() {
        assert_eq!(family_of("RoadTechCurve2"), "RoadTech");
        assert_eq!(family_of("PlatformDirtLoopStart"), "PlatformDirt");
        assert_eq!(family_of("GateCheckpointLeft32m"), "GateCheckpoint");
        assert_eq!(family_of("DecoWallSlope2UTop"), "DecoWall");
        assert_eq!(family_of("RoadBumpCheckpointSlopeUp"), "RoadBump");
        assert_eq!(family_of("LandHill2"), "LandHill");
        assert_eq!(family_of("Dirt0Dirt4"), "Dirt");
        assert_eq!(family_of("SupportTruss64m"), "SupportTruss");
        assert_eq!(family_of("TrackBarrier4m"), "TrackBarrier");
        assert_eq!(Special::of_name("GateGameplayBoost2"), Special::Boost2);
        assert_eq!(Special::of_name("RoadTechSpecialTurbo"), Special::Turbo);
        assert_eq!(Special::of_name("GateGameplayReactorDown"), Special::ReactorDown);
        assert_eq!(Special::of_name("RoadTechCheckpoint"), Special::Checkpoint);
        assert_eq!(Special::of_name("RoadDirtFinish"), Special::Finish);
        assert_eq!(Special::of_name("RoadTechStart"), Special::Start);
        assert_eq!(Special::of_name("RoadTechStraight"), Special::None);
        assert_eq!(Special::of_name("PlatformTechLoopStart"), Special::None);
        assert_eq!(Special::of_name("GateExpandableGameplaySnow"), Special::TransformSnow);
        assert_eq!(Special::of_name("GateGameplayStadium24m"), Special::TransformStadium);
        assert_eq!(Special::of_name("RallyGateGameplay"), Special::TransformRally);
        assert_eq!(Special::of_name("GateExpandableGameplayVFC"), Special::None);
        assert_eq!(Special::of_name("GateExpandableSpecialReset"), Special::Reset);
        assert_eq!(Special::of_name("PlatformSpecialFCLeft"), Special::None);
    }
}

/// The STABLE family id table (INTERFACES §4, MODEL arm's ask): the same u8 for a
/// family on every map, so an embedding index means one thing. Built from the
/// census over the 25 Summer 2026 maps (`tmplan families`); anything not listed
/// is `FAMILY_OTHER`. Append only — never renumber.
pub const FAMILIES: &[&str] = &[
    "RoadTech", "RoadDirt", "RoadBump", "RoadIce", "RoadWater", "RoadTechSpecial", "OpenTech", "OpenDirt", "OpenIce", "OpenGrass",
    "PlatformTech", "PlatformDirt", "PlatformGrass", "PlatformIce", "PlatformPlastic", "PlatformWater", "PlatformWood",
    "GateCheckpoint", "GateFinish", "GateSpecial", "GateExpandable", "GateGameplay", "GateMultilap",
    "DecoWall", "DecoHill", "DecoCliff", "DecoPlatform", "DecoTerrain", "DecoWater", "DecoTree", "DecoLight", "DecoPillar",
    "Land", "LandHill", "LandCliff", "LandWater", "Beach", "Sea", "Cliff", "Hill", "Water", "Grass", "Dirt", "Sand", "Rock",
    "TrackWall", "TrackWallDiag", "StadiumStructure", "Stadium", "Technics", "TechnicsScreen", "Pillar", "Tunnel", "Loop",
    "Wallride", "Booster", "Turbo", "Reactor", "Sculpture", "Screen", "Advertisement", "Fence", "Rail", "Ramp",
    // appended from the 25-map census (`tmplan families`, 2026-09-07), placements ≥ 10, in census order
    "Lake", "ShowLights", "StructurePillar", "ShowSpeakers", "TrackBarrier", "LakeShore", "Flag", "StructureSupport",
    "SummerPalm", "RoadSign", "SupportTruss", "Summer", "ShowRace", "StructureBase", "Show", "InflatableMat", "SeaCliff",
    "StandStraight", "TechnicsScreenx", "Lamp", "Sparkler", "StageTechnics", "LightCube", "LampSmall", "ShowScreen",
    "LightCylinder", "LightTube", "ShowTorch", "ObstaclePillar", "Snow", "StandCorner", "TunnelSupport", "ShowRig",
    "DecoLake", "SeaLand", "PlatformBase", "CanopyEnd", "DecoTerraforming", "StructureDeadend", "SupportConnector",
    "SupportTube", "GateSupport", "Screenx", "CanopyCenter", "ObstacleRotor", "StandDiag", "RampMedv", "RampLowv",
    "Podium", "InflatableBorder", "TMESaudi", "ShowFogger", "ShowLight", "StructureStraight", "TMEJapan", "RallyBarrier",
    "TMENorway", "ObstaclePusher", "TMEPoland", "RallyCastle", "StageStraight", "TMEArgentina", "Stade", "StadeScreen",
    "TMEExtras",
];
pub const FAMILY_OTHER: u8 = 255;

/// Stable id of a family name (`FAMILY_OTHER` when not in the table).
pub fn family_id(family: &str) -> u8 {
    FAMILIES.iter().position(|f| *f == family).map_or(FAMILY_OTHER, |i| i as u8)
}

impl Hit {
    /// Stable family id (`family_id`).
    pub fn family_id(&self) -> u8 {
        family_id(&self.family)
    }
}

impl LocalScene {
    /// Batched rays from one origin: one `Option<Hit>` per direction, no per-ray
    /// allocation beyond the output vector. Directions need not be unit.
    pub fn raycast_many(&self, origin: [f32; 3], dirs: &[[f32; 3]], max_m: f32, skip_noncollidable: bool) -> Vec<Option<Hit>> {
        dirs.iter().map(|d| self.raycast(origin, *d, max_m, skip_noncollidable)).collect()
    }

    /// Fill-into-slice form for a hot loop: `out[i] = (dist, material, family_id,
    /// special as u8)`; a miss is `(max_m, 255, 255, 0)`. No allocation at all.
    pub fn raycast_fill(&self, origin: [f32; 3], dirs: &[[f32; 3]], max_m: f32, skip_noncollidable: bool, out: &mut [(f32, u8, u8, u8)]) {
        for (i, d) in dirs.iter().enumerate() {
            out[i] = match self.raycast_raw(origin, *d, max_m, skip_noncollidable) {
                Some((dist, ti)) => {
                    let t = &self.tris[ti as usize];
                    let p = &self.placements[t.tag as usize];
                    (dist, t.mat, family_id(&p.family), p.special as u8)
                }
                None => (max_m, 255, 255, 0),
            };
        }
    }

    /// The core of `raycast` without building a `Hit`: (distance, triangle index).
    pub fn raycast_raw(&self, origin: [f32; 3], dir: [f32; 3], max_m: f32, skip_noncollidable: bool) -> Option<(f32, u32)> {
        let d = norm(dir);
        if d[0].is_nan() {
            return None;
        }
        let (mut t_enter, t_exit) = self.clip(origin, d)?;
        if t_exit < 0.0 || t_enter > max_m {
            return None;
        }
        t_enter = t_enter.max(0.0);
        let t_end = t_exit.min(max_m);
        let start = [origin[0] + d[0] * (t_enter + 1e-4), origin[1] + d[1] * (t_enter + 1e-4), origin[2] + d[2] * (t_enter + 1e-4)];
        let mut c = self.cell_of(start)?;
        let step: [isize; 3] = [0, 1, 2].map(|k| if d[k] > 0.0 { 1 } else if d[k] < 0.0 { -1 } else { 0 });
        let mut t_max = [f32::INFINITY; 3];
        let mut t_delta = [f32::INFINITY; 3];
        for k in 0..3 {
            if d[k] != 0.0 {
                let next = self.origin[k] + (c[k] as f32 + if step[k] > 0 { 1.0 } else { 0.0 }) * self.cell;
                t_max[k] = t_enter + (next - start[k]) / d[k] + 1e-4;
                t_delta[k] = self.cell / d[k].abs();
            }
        }
        let mut best: Option<(f32, u32)> = None;
        loop {
            let t_cell_exit = t_max[0].min(t_max[1]).min(t_max[2]);
            let ci = (c[2] as usize * self.dims[1] + c[1] as usize) * self.dims[0] + c[0] as usize;
            for k in self.cell_start[ci]..self.cell_start[ci + 1] {
                let ti = self.cell_tris[k as usize];
                let t = &self.tris[ti as usize];
                if skip_noncollidable && !crate::scene::is_collidable(crate::scene::physics_name(t.mat)) {
                    continue;
                }
                if let Some(dist) = ray_tri(origin, d, &t.v) {
                    if dist >= 0.0 && dist <= t_end && best.map_or(true, |(bd, _)| dist < bd) {
                        best = Some((dist, ti));
                    }
                }
            }
            if let Some((bd, _)) = best {
                if bd <= t_cell_exit {
                    break;
                }
            }
            if t_cell_exit > t_end {
                break;
            }
            let k = if t_max[0] < t_max[1] { if t_max[0] < t_max[2] { 0 } else { 2 } } else if t_max[1] < t_max[2] { 1 } else { 2 };
            c[k] += step[k];
            if c[k] < 0 || c[k] >= self.dims[k] as isize {
                break;
            }
            t_max[k] += t_delta[k];
        }
        best
    }
}

// ---------------------------------------------------------------- items index
/// A placement's bounding sphere — for `nearest_items`: thin things (poles,
/// barriers, signs, gate pillars) that a ray fan undersamples.
#[derive(Clone, Copy, Debug)]
pub struct Bound {
    pub centre: [f32; 3],
    pub radius: f32,
    pub tri_count: u32,
}

#[derive(Clone, Debug)]
pub struct NearItem {
    pub placement: u32,
    /// Vector from the query point to the placement's bounding centre (world frame).
    pub rel: [f32; 3],
    pub dist: f32,
    pub radius: f32,
    pub family: String,
    pub family_id: u8,
    pub kind: PlacementKind,
    pub special: Special,
}

impl LocalScene {
    /// Bounding sphere of every placement (centre of its triangles' AABB, radius
    /// to the farthest vertex). `None` for a placement without triangles.
    pub fn bounds(&self) -> Vec<Option<Bound>> {
        let n = self.placements.len();
        let mut lo = vec![[f32::INFINITY; 3]; n];
        let mut hi = vec![[f32::NEG_INFINITY; 3]; n];
        let mut cnt = vec![0u32; n];
        for t in &self.tris {
            let k = t.tag as usize;
            cnt[k] += 1;
            for v in &t.v {
                for a in 0..3 {
                    lo[k][a] = lo[k][a].min(v[a]);
                    hi[k][a] = hi[k][a].max(v[a]);
                }
            }
        }
        (0..n)
            .map(|k| {
                if cnt[k] == 0 {
                    return None;
                }
                let c = [(lo[k][0] + hi[k][0]) / 2.0, (lo[k][1] + hi[k][1]) / 2.0, (lo[k][2] + hi[k][2]) / 2.0];
                let r = ((hi[k][0] - c[0]).powi(2) + (hi[k][1] - c[1]).powi(2) + (hi[k][2] - c[2]).powi(2)).sqrt();
                Some(Bound { centre: c, radius: r, tri_count: cnt[k] })
            })
            .collect()
    }
}

/// The k-nearest-items index over a scene's ITEM placements (and free blocks —
/// both are "things placed on the track"; grid blocks, baked terrain and the
/// decoration are excluded, the rays see those). Built once per scene.
pub struct ItemIndex {
    pub bounds: Vec<(u32, Bound)>,
    cell: f32,
    origin: [f32; 2],
    nx: usize,
    nz: usize,
    cell_start: Vec<u32>,
    cell_items: Vec<u32>,
}

impl ItemIndex {
    /// `max_radius`: placements with a larger bounding radius are not "items" (a
    /// 200 m stand is not a pole); 40 m keeps gates, barriers, signs, tubes.
    pub fn build(scene: &LocalScene, max_radius: f32) -> ItemIndex {
        let all = scene.bounds();
        let mut bounds: Vec<(u32, Bound)> = Vec::new();
        for (k, b) in all.iter().enumerate() {
            let p = &scene.placements[k];
            if !matches!(p.kind, PlacementKind::Item | PlacementKind::FreeBlock) {
                continue;
            }
            if let Some(b) = b {
                if b.radius <= max_radius {
                    bounds.push((k as u32, *b));
                }
            }
        }
        let cell = 16.0f32;
        let (mut lo, mut hi) = ([f32::INFINITY; 2], [f32::NEG_INFINITY; 2]);
        for (_, b) in &bounds {
            lo[0] = lo[0].min(b.centre[0]);
            lo[1] = lo[1].min(b.centre[2]);
            hi[0] = hi[0].max(b.centre[0]);
            hi[1] = hi[1].max(b.centre[2]);
        }
        if bounds.is_empty() {
            lo = [0.0, 0.0];
            hi = [1.0, 1.0];
        }
        let origin = [lo[0] - 1.0, lo[1] - 1.0];
        let nx = (((hi[0] + 1.0 - origin[0]) / cell).ceil() as usize).max(1);
        let nz = (((hi[1] + 1.0 - origin[1]) / cell).ceil() as usize).max(1);
        let mut count = vec![0u32; nx * nz + 1];
        let cell_of = |b: &Bound| -> usize {
            let x = (((b.centre[0] - origin[0]) / cell) as usize).min(nx - 1);
            let z = (((b.centre[2] - origin[1]) / cell) as usize).min(nz - 1);
            z * nx + x
        };
        for (_, b) in &bounds {
            count[cell_of(b) + 1] += 1;
        }
        for i in 0..nx * nz {
            count[i + 1] += count[i];
        }
        let mut fill = count.clone();
        let mut cell_items = vec![0u32; bounds.len()];
        for (i, (_, b)) in bounds.iter().enumerate() {
            let c = cell_of(b);
            cell_items[fill[c] as usize] = i as u32;
            fill[c] += 1;
        }
        ItemIndex { bounds, cell, origin, nx, nz, cell_start: count, cell_items }
    }

    /// The k nearest items (by distance from `p` to the bounding sphere's SURFACE,
    /// i.e. centre distance minus radius, floored at 0) within `r` metres.
    pub fn nearest(&self, scene: &LocalScene, p: [f32; 3], r: f32, k: usize) -> Vec<NearItem> {
        let mut out: Vec<NearItem> = Vec::new();
        let cx = ((p[0] - self.origin[0]) / self.cell).floor() as isize;
        let cz = ((p[2] - self.origin[1]) / self.cell).floor() as isize;
        let ring = (r / self.cell).ceil() as isize + 1;
        for dz in -ring..=ring {
            for dx in -ring..=ring {
                let (x, z) = (cx + dx, cz + dz);
                if x < 0 || z < 0 || x >= self.nx as isize || z >= self.nz as isize {
                    continue;
                }
                let c = z as usize * self.nx + x as usize;
                for i in self.cell_start[c]..self.cell_start[c + 1] {
                    let (pl, b) = self.bounds[self.cell_items[i as usize] as usize];
                    let rel = [b.centre[0] - p[0], b.centre[1] - p[1], b.centre[2] - p[2]];
                    let dc = (rel[0] * rel[0] + rel[1] * rel[1] + rel[2] * rel[2]).sqrt();
                    let dist = (dc - b.radius).max(0.0);
                    if dist > r {
                        continue;
                    }
                    let pm = &scene.placements[pl as usize];
                    out.push(NearItem { placement: pl, rel, dist, radius: b.radius, family: pm.family.clone(), family_id: family_id(&pm.family), kind: pm.kind, special: pm.special });
                }
            }
        }
        out.sort_by(|a, b| a.dist.partial_cmp(&b.dist).unwrap());
        out.truncate(k);
        out
    }
}

impl LocalScene {
    /// Convenience: build an `ItemIndex` (40 m bounding cap) and query it once. For
    /// many queries build the index once with `ItemIndex::build`.
    pub fn nearest_items(&self, point: [f32; 3], r: f32, k: usize) -> Vec<NearItem> {
        ItemIndex::build(self, 40.0).nearest(self, point, r, k)
    }
}

#[cfg(test)]
mod item_tests {
    use super::*;
    #[test]
    fn nearest_items_finds_the_pole() {
        // a floor (grid block) and two thin poles (items) at x = 20 and x = 60
        let mut tris = vec![
            Tri { v: [[0.0, 0.0, 0.0], [100.0, 0.0, 0.0], [100.0, 0.0, 100.0]], mat: 16, tag: 0 },
        ];
        for (tag, x) in [(1u32, 20.0f32), (2, 60.0)] {
            tris.push(Tri { v: [[x, 0.0, 50.0], [x + 0.5, 0.0, 50.0], [x, 10.0, 50.0]], mat: 4, tag });
        }
        let pl = vec![
            Placement { name: "PlatformTechBase".into(), family: "PlatformTech".into(), kind: PlacementKind::Block, special: Special::None, index: 0 },
            Placement { name: "Pole".into(), family: "Pole".into(), kind: PlacementKind::Item, special: Special::None, index: 0 },
            Placement { name: "TrackBarrier4m".into(), family: "TrackBarrier".into(), kind: PlacementKind::Item, special: Special::None, index: 1 },
        ];
        let s = LocalScene::index(tris, pl, 4.0);
        let idx = ItemIndex::build(&s, 40.0);
        let near = idx.nearest(&s, [10.0, 1.0, 50.0], 100.0, 2);
        assert_eq!(near.len(), 2);
        assert_eq!(near[0].placement, 1);
        assert!(near[0].dist < near[1].dist);
        assert_eq!(near[1].family, "TrackBarrier");
        assert_eq!(idx.nearest(&s, [10.0, 1.0, 50.0], 5.0, 2).len(), 0);
    }
}

impl Special {
    /// The car a transformation gate hands the driver, if this is one.
    pub fn car(&self) -> Option<&'static str> {
        match self {
            Special::TransformStadium => Some("Stadium"),
            Special::TransformSnow => Some("Snow"),
            Special::TransformRally => Some("Rally"),
            Special::TransformDesert => Some("Desert"),
            _ => None,
        }
    }
    /// A gameplay block that changes the CAR'S STATE (not a waypoint, not a transformation).
    pub fn is_physics(&self) -> bool {
        matches!(self, Special::Boost | Special::Boost2 | Special::Turbo | Special::Turbo2 | Special::Reactor | Special::ReactorDown | Special::SlowMotion | Special::Reset | Special::NoEngine | Special::Cruise | Special::NoBrake | Special::NoSteer | Special::Fragile | Special::Bumper)
    }
    pub fn is_waypoint(&self) -> bool {
        matches!(self, Special::Checkpoint | Special::Finish | Special::Start | Special::Multilap)
    }
    pub fn name(&self) -> String {
        format!("{:?}", self)
    }
}
