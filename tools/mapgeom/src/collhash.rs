//! `mapgeom collhash MAP.Map.Gbx` — the physics fingerprint of a tiny map.
//!
//! # Why this exists
//!
//! The player project drives these maps in a dedicated server and banks laps
//! against them. A lap is only valid for the exact surface it was driven on:
//! the fp1 rebuild (2026-09-07) moved the car 1 cm at 2.15 s, 1 m at 2.32 s and
//! 48 m by 6 s against the parked build, and every lap ever driven on the old
//! Summer 01 DNFs on it. So a published map has to be FROZEN: once laps exist,
//! nothing in the file that the physics reads may change.
//!
//! "Nothing that the physics reads" is a smaller set than "nothing". Colours,
//! materials' looks, LOD levels, lights, MediaTracker clips, skins and
//! vegetation cosmetics may all change freely — the car cannot feel them. What
//! it feels is: which collision triangles exist, where, and with which physics
//! id; where every placement puts them; and where the car starts and what
//! counts as a waypoint. This command hashes exactly that, and nothing else, so
//! a cosmetic pass can be PROVEN collision-neutral (same hash) instead of
//! promised to be.
//!
//! # What goes into the hash
//!
//! * every placement: model name, position, yaw/pitch/roll, pivot, scale,
//!   waypoint tag — quantised to 1e-4 m / 1e-6 rad, so a float printed and
//!   re-parsed hashes the same;
//! * every embedded item the map places, by name: each part's collision surface
//!   (vertex positions, triangle indices, per-triangle physics id, the surface
//!   material id table), the waypoint type, the spawn iso, and the trigger
//!   surface;
//! * the map's authored and baked BLOCKS (name, cell, dir, flags, free
//!   position/rotation): a parked block still carries collision and a waypoint,
//!   which is the defect that put the car 155–306 m from the spawn on 19 of the
//!   20 published maps.
//!
//! Explicitly NOT in the hash: visuals, materials, textures, lights, LOD
//! ladders, MediaTracker, thumbnails, the map name, the uid, the lightmap.
//!
//! The output is one line per section plus the total, so a changed hash says
//! WHICH of the three moved.

use std::collections::BTreeMap;

/// FNV-1a, 64-bit: no dependency, stable across machines and runs (a
/// `DefaultHasher` is explicitly not stable across releases, which is the one
/// property this needs).
#[derive(Clone)]
pub struct Fnv(u64);

impl Default for Fnv {
    fn default() -> Self {
        Fnv(0xcbf2_9ce4_8422_2325)
    }
}

impl Fnv {
    pub fn write(&mut self, bytes: &[u8]) {
        for b in bytes {
            self.0 ^= *b as u64;
            self.0 = self.0.wrapping_mul(0x1000_0000_01b3);
        }
    }
    pub fn str(&mut self, s: &str) {
        self.write(s.as_bytes());
        self.write(&[0]);
    }
    /// A float quantised to `q` before hashing: 1e-4 m is far below anything
    /// the physics can feel, and it makes the hash immune to a value that made
    /// a round trip through a decimal print.
    pub fn f32q(&mut self, v: f32, q: f32) {
        let n = (v / q).round() as i64;
        // -0 and 0 must hash the same
        self.write(&(if n == 0 { 0 } else { n }).to_le_bytes());
    }
    pub fn u32(&mut self, v: u32) {
        self.write(&v.to_le_bytes());
    }
    pub fn hex(&self) -> String {
        format!("{:016x}", self.0)
    }
}

const POS_Q: f32 = 1.0e-4;
const ROT_Q: f32 = 1.0e-6;

/// The collision surface of every part of one item file, hashed.
fn item_collision(bytes: &[u8], h: &mut Fnv) -> Result<(), String> {
    let f = crate::static_item::parse_file(bytes)?;
    let mut surfaces: Vec<(&crate::static_item::surface::CPlugSurface, String)> = Vec::new();
    if let Some(so) = f.item.static_object() {
        if let Some(s) = so.surface() {
            surfaces.push((s, "static".into()));
        }
    } else if let Some(p) = f.item.prefab() {
        for (i, e) in p.ents.iter().enumerate() {
            match e.model.inline.as_deref() {
                Some(crate::static_item::Node::StaticObject(so)) => {
                    if let Some(s) = so.surface() {
                        surfaces.push((s, format!("e{i}")));
                    }
                }
                Some(crate::static_item::Node::Dyna(d)) => {
                    // A moving part's hulls are collision too — and its
                    // placement inside the prefab moves them.
                    for (r, what) in [(&d.static_shape, "hit"), (&d.dyna_shape, "move")] {
                        if let Some(crate::static_item::Node::Surface(s)) = r.inline.as_deref() {
                            surfaces.push((s, format!("e{i}{what}")));
                        }
                    }
                    h.str(&format!("dyna{i}"));
                    h.f32q(e.pos[0], POS_Q);
                    h.f32q(e.pos[1], POS_Q);
                    h.f32q(e.pos[2], POS_Q);
                    for k in 0..4 {
                        h.f32q(e.rot[k], ROT_Q);
                    }
                }
                _ => {}
            }
        }
    }
    for (s, what) in surfaces {
        h.str(&what);
        h.write(&(s.material_ids.len() as u32).to_le_bytes());
        for id in &s.material_ids {
            h.write(&id.to_le_bytes());
        }
        match &s.surf {
            crate::static_item::surface::Surf::Mesh { vertices, triangles, .. } => {
                h.str("mesh");
                h.u32(vertices.len() as u32);
                for v in vertices {
                    h.f32q(v[0], POS_Q);
                    h.f32q(v[1], POS_Q);
                    h.f32q(v[2], POS_Q);
                }
                h.u32(triangles.len() as u32);
                for t in triangles {
                    h.u32(t.indices[0]);
                    h.u32(t.indices[1]);
                    h.u32(t.indices[2]);
                    h.write(&[t.material_id]);
                    h.write(&t.surface_index.to_le_bytes());
                }
            }
            other => {
                // a primitive hull: its type and every length in it
                h.str("prim");
                h.u32(other.type_id() as u32);
                let (nv, nt) = other.counts();
                h.u32(nv as u32);
                h.u32(nt as u32);
                if let Some((verts, tris)) = other.triangulate() {
                    for v in &verts {
                        h.f32q(v[0], POS_Q);
                        h.f32q(v[1], POS_Q);
                        h.f32q(v[2], POS_Q);
                    }
                    for t in &tris {
                        h.u32(t.indices[0]);
                        h.u32(t.indices[1]);
                        h.u32(t.indices[2]);
                        h.write(&[t.material_id]);
                    }
                }
            }
        }
    }
    // The waypoint side. This is physics too: the type decides whether the
    // engine treats the item as the start (Summer 15/20/25 baked their start
    // gate as type 3 with spawn (0,0,0) and the playground opened with NO CAR,
    // 2026-09-07), the entity iso is where the car appears, and the trigger
    // shape is what a lap has to cross.
    for c in &f.item.chunks {
        if let crate::static_item::item::ItemChunk::Waypoint { waypoint_type, .. } = c {
            h.str("wp");
            h.write(&waypoint_type.to_le_bytes());
        }
    }
    if let Some(e) = f.item.model().and_then(|m| m.entity_model()) {
        h.str("spawn");
        for v in e.iso {
            h.f32q(v, POS_Q);
        }
        for v in e.iso2 {
            h.f32q(v, POS_Q);
        }
        if let Some(crate::static_item::Node::Surface(tr)) = e.trigger_shape.inline.as_deref() {
            h.str("trig");
            if let crate::static_item::surface::Surf::Mesh { vertices, triangles, .. } = &tr.surf {
                h.u32(vertices.len() as u32);
                for v in vertices {
                    h.f32q(v[0], POS_Q);
                    h.f32q(v[1], POS_Q);
                    h.f32q(v[2], POS_Q);
                }
                h.u32(triangles.len() as u32);
                for t in triangles {
                    h.u32(t.indices[0]);
                    h.u32(t.indices[1]);
                    h.u32(t.indices[2]);
                }
            }
        }
    }
    Ok(())
}

/// The fingerprint, as `run` prints it: total, placements, blocks, items hashes;
/// placement / block / model counts; the Spawn placement index; per-item hashes.
pub struct Summary {
    pub total: String,
    pub placements: String,
    pub blocks: String,
    pub items: String,
    pub n_items: usize,
    pub n_blocks: usize,
    pub n_models: usize,
    pub spawn_index: Option<usize>,
    pub per_item: BTreeMap<String, String>,
}

pub fn run(rest: &[String]) -> Result<(), String> {
    let paths: Vec<&String> = rest.iter().skip(1).filter(|a| !a.starts_with("--")).collect();
    if paths.is_empty() {
        return Err("collhash MAP.Map.Gbx… [--parts]".into());
    }
    let parts_wanted = rest.iter().any(|a| a == "--parts");
    for path in paths {
        let m = tmmaps::map::MapFile::load(std::path::Path::new(path));
        let s = summary(&m);
        println!(
            "{path}\tcollision {}\tplacements {} ({} items)\tblocks {} ({})\titems {} ({} models)",
            s.total, s.placements, s.n_items, s.blocks, s.n_blocks, s.items, s.n_models
        );
        match s.spawn_index {
            Some(i) => {
                let sp = &m.items[i];
                println!("  Spawn placement: index {i} {} at [{:.1}, {:.1}, {:.1}]", sp.model, sp.pos[0], sp.pos[1], sp.pos[2]);
            }
            None => println!("  Spawn placement: NONE — this map has no start"),
        }
        if parts_wanted {
            for (name, hex) in &s.per_item {
                println!("  {hex}  {name}");
            }
        }
    }
    Ok(())
}

/// The fingerprint of a loaded map (what `run` prints), for callers that want
/// the numbers (`tinyctl ship`'s MANIFEST).
pub fn summary(m: &tmmaps::map::MapFile) -> Summary {
    {

        // --- 1. placements. The loop is SEQUENTIAL over the map's record
        // order, and FNV is order-sensitive, so a reordering of identical
        // placements changes this hash. That is worth keeping: a validation
        // record names the start waypoint by INDEX (chunk 0x0309202D, the u32
        // after the settings-flags word — resolved 2026-09-08), so which
        // placement sits at which index is part of what a validated map means.
        let mut ph = Fnv::default();
        let mut spawn_index: Option<usize> = None;
        for (i, it) in m.items.iter().enumerate() {
            ph.str(&it.model);
            ph.f32q(it.pos[0], POS_Q);
            ph.f32q(it.pos[1], POS_Q);
            ph.f32q(it.pos[2], POS_Q);
            ph.f32q(it.yaw, ROT_Q);
            ph.f32q(it.pitch, ROT_Q);
            ph.f32q(it.roll, ROT_Q);
            ph.f32q(it.pivot[0], POS_Q);
            ph.f32q(it.pivot[1], POS_Q);
            ph.f32q(it.pivot[2], POS_Q);
            ph.f32q(it.scale, 1.0e-6);
            let tag = it.waypoint_tag.as_deref().unwrap_or("");
            ph.str(tag);
            if tag == "Spawn" {
                spawn_index = Some(i);
            }
        }

        // --- 2. blocks (a parked block is still collision AND a waypoint)
        let mut bh = Fnv::default();
        let mut nblocks = 0usize;
        for (kind, b) in m.blocks.iter().map(|b| ("u", b)).chain(m.baked.iter().map(|b| ("b", b))) {
            nblocks += 1;
            bh.str(kind);
            bh.str(&b.name);
            bh.write(&b.file_cell);
            bh.write(&[b.dir]);
            bh.u32(b.flags);
            if let Some(p) = b.free_pos {
                bh.f32q(p[0], POS_Q);
                bh.f32q(p[1], POS_Q);
                bh.f32q(p[2], POS_Q);
            }
            if let Some(r) = b.free_rot {
                bh.f32q(r[0], ROT_Q);
                bh.f32q(r[1], ROT_Q);
                bh.f32q(r[2], ROT_Q);
            }
            bh.str(b.waypoint_tag.as_deref().unwrap_or(""));
        }

        // --- 3. the embedded items' collision
        let files = crate::embedded::files(&m).unwrap_or_default();
        let mut per_item: BTreeMap<String, String> = BTreeMap::new();
        let mut ih = Fnv::default();
        for (name, bytes) in &files {
            let base = name.rsplit(['/', '\\']).next().unwrap_or(name).to_string();
            if !base.to_ascii_lowercase().ends_with(".item.gbx") {
                continue;
            }
            let mut one = Fnv::default();
            match item_collision(bytes, &mut one) {
                Ok(()) => {}
                Err(e) => {
                    one.str(&format!("UNPARSED:{e}"));
                }
            }
            per_item.insert(base.clone(), one.hex());
        }
        for (name, hex) in &per_item {
            ih.str(name);
            ih.str(hex);
        }

        let mut total = Fnv::default();
        total.str(&ph.hex());
        total.str(&bh.hex());
        total.str(&ih.hex());

        Summary { total: total.hex(), placements: ph.hex(), blocks: bh.hex(), items: ih.hex(), n_items: m.items.len(), n_blocks: nblocks, n_models: per_item.len(), spawn_index, per_item }
    }
}
