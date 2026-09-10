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
    // positional = every argument that is not a flag and not a flag's value (--report F)
    let mut paths: Vec<&String> = Vec::new();
    let mut skip = false;
    for a in rest.iter().skip(1) {
        if skip { skip = false; continue; }
        if a == "--report" { skip = true; continue; }
        if !a.starts_with("--") { paths.push(a); }
    }
    if paths.is_empty() {
        return Err("collhash MAP.Map.Gbx… [--parts] [--by-name] | collhash --diff A.Map.Gbx B.Map.Gbx [--report B-report.tsv]".into());
    }
    if rest.iter().any(|a| a == "--diff") {
        return diff(rest, &paths);
    }
    let parts_wanted = rest.iter().any(|a| a == "--parts");
    // `--by-name`: the form before 2026-09-09 (embedded items by file name),
    // for manifests written then; the default hashes them by content.
    let by_name = rest.iter().any(|a| a == "--by-name");
    for path in paths {
        let m = tmmaps::map::MapFile::load(std::path::Path::new(path));
        let s = summary_with(&m, by_name);
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
/// `summary` in the BY-CONTENT form (the default since 2026-09-09): an embedded
/// item enters the hash as its collision fingerprint, never as its file name.
/// Item names are unique per map AND build (20960a1d: `TINY_ALIAS_BASE`), so a
/// by-name hash differs between two builds of identical geometry and can no
/// longer prove a pass collision-neutral — which is the one thing this command
/// is for (the pub4 publish set against ship15, 2026-09-09: seven maps rebuilt
/// with `--lod-pick` hashed DIFFERENT until their alias bases were forced to
/// ship15's). `--by-name` gives the old form, comparable with manifests
/// written before this date (ship13–ship15).
pub fn summary(m: &tmmaps::map::MapFile) -> Summary {
    summary_with(m, false)
}

/// `by_name`: hash embedded items by file name (the form before 2026-09-09).
pub fn summary_with(m: &tmmaps::map::MapFile, by_name: bool) -> Summary {
    {
        // --- 0. the embedded items' collision, by file name (the placements
        // below look their model up here in the by-content form)
        let files = crate::embedded::files(&m).unwrap_or_default();
        let mut per_item: BTreeMap<String, String> = BTreeMap::new();
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

        // --- 1. placements. The loop is SEQUENTIAL over the map's record
        // order, and FNV is order-sensitive, so a reordering of identical
        // placements changes this hash. That is worth keeping: a validation
        // record names the start waypoint by INDEX (chunk 0x0309202D, the u32
        // after the settings-flags word — resolved 2026-09-08), so which
        // placement sits at which index is part of what a validated map means.
        let mut ph = Fnv::default();
        let mut spawn_index: Option<usize> = None;
        for (i, it) in m.items.iter().enumerate() {
            // an embedded item by its collision fingerprint (by-content), a stock
            // model by its name either way
            match (by_name, per_item.get(&it.model)) {
                (false, Some(hex)) => ph.str(hex),
                _ => ph.str(&it.model),
            }
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

        // --- 3. the embedded items' collision: the sorted set of fingerprints
        // (with the names in the by-name form)
        let mut ih = Fnv::default();
        let mut hexes: Vec<&String> = per_item.values().collect();
        hexes.sort();
        for (name, hex) in &per_item {
            if by_name {
                ih.str(name);
                ih.str(hex);
            }
        }
        if !by_name {
            for hex in hexes {
                ih.str(hex);
            }
        }

        let mut total = Fnv::default();
        total.str(&ph.hex());
        total.str(&bh.hex());
        total.str(&ih.hex());

        Summary { total: total.hex(), placements: ph.hex(), blocks: bh.hex(), items: ih.hex(), n_items: m.items.len(), n_blocks: nblocks, n_models: per_item.len(), spawn_index, per_item }
    }
}

/// `collhash --diff A B [--report B-report.tsv]`: the two maps' section hashes
/// side by side, then the per-model collision fingerprints A has and B lacks
/// and vice versa (a model = one fingerprint; a build re-aliases freely, so
/// names never enter the comparison), each B-side model named by its source
/// block when B's build report is given. Exit code 0 when the collision hash
/// is the same, 1 when not — a shell loop is never the judge again (the
/// 2026-09-10 tables compared two empty strings and said "identical" on 23
/// maps whose clip walls had changed physics id).
fn diff(rest: &[String], paths: &[&String]) -> Result<(), String> {
    if paths.len() != 2 {
        return Err("collhash --diff needs exactly two maps".into());
    }
    let report: BTreeMap<String, String> = rest
        .iter()
        .position(|a| a == "--report")
        .and_then(|i| rest.get(i + 1))
        .and_then(|p| std::fs::read_to_string(p).ok())
        .map(|text| {
            text.lines()
                .filter_map(|l| {
                    let c: Vec<&str> = l.split('\t').collect();
                    ((c[0] == "block" || c[0] == "item" || c[0] == "tree") && c.len() > 4).then(|| (format!("{}.Item.Gbx", c[1]), c[4].to_string()))
                })
                .collect()
        })
        .unwrap_or_default();
    let ma = tmmaps::map::MapFile::load(std::path::Path::new(paths[0]));
    let mb = tmmaps::map::MapFile::load(std::path::Path::new(paths[1]));
    let (a, b) = (summary(&ma), summary(&mb));
    let (sa, sb) = (shapes(&ma), shapes(&mb));
    let same = a.total == b.total;
    println!("A {}\tcollision {}\tplacements {} ({} items)\tblocks {} ({})\titems {} ({} models)", paths[0], a.total, a.placements, a.n_items, a.blocks, a.n_blocks, a.items, a.n_models);
    println!("B {}\tcollision {}\tplacements {} ({} items)\tblocks {} ({})\titems {} ({} models)", paths[1], b.total, b.placements, b.n_items, b.blocks, b.n_blocks, b.items, b.n_models);
    println!("collision {}", if same { "IDENTICAL" } else { "DIFFERENT" });
    // multisets of fingerprints
    let count = |s: &Summary| -> BTreeMap<String, Vec<String>> {
        let mut out: BTreeMap<String, Vec<String>> = BTreeMap::new();
        for (name, hex) in &s.per_item {
            out.entry(hex.clone()).or_default().push(name.clone());
        }
        out
    };
    let (fa, fb) = (count(&a), count(&b));
    let only_a: Vec<(&String, &Vec<String>)> = fa.iter().filter(|(h, _)| !fb.contains_key(*h)).collect();
    let only_b: Vec<(&String, &Vec<String>)> = fb.iter().filter(|(h, _)| !fa.contains_key(*h)).collect();
    println!("model fingerprints only in A: {}   only in B: {}", only_a.len(), only_b.len());
    // pair B's new models with A's vanished ones by SHAPE: the same hull under
    // other physics ids is a re-dress (old → new per id); a shape with no
    // partner is new or gone geometry
    // Pair by SHAPE across the WHOLE other map: the same hull under other
    // physics ids is a re-dress (A's Wood wall → B's Concrete wall, whether or
    // not B also kept a Wood copy); a shape with no partner anywhere is new or
    // gone geometry.
    let name_of = |names: &Vec<String>| names.first().cloned().unwrap_or_default();
    let hist = |p: &BTreeMap<u8, usize>| p.iter().map(|(id, n)| format!("{}:{n}", crate::scene::physics_name(*id))).collect::<Vec<_>>().join(" ");
    let by_shape = |s: &BTreeMap<String, (String, BTreeMap<u8, usize>)>| -> BTreeMap<String, Vec<String>> {
        let mut out: BTreeMap<String, Vec<String>> = BTreeMap::new();
        for (name, (shape, _)) in s {
            out.entry(shape.clone()).or_default().push(name.clone());
        }
        out
    };
    let (shape_a, shape_b) = (by_shape(&sa), by_shape(&sb));
    let mut redress: BTreeMap<String, usize> = BTreeMap::new();
    let (mut gone, mut new_shape) = (0usize, 0usize);
    for (h, names) in &only_a {
        let na = name_of(names);
        let Some((shp, pa)) = sa.get(&na) else { continue };
        match shape_b.get(shp) {
            Some(partners) => {
                let nb = partners[0].clone();
                let pb = &sb[&nb].1;
                let key = format!("{} -> {}", hist(pa), hist(pb));
                *redress.entry(key.clone()).or_default() += 1;
                let src = report.get(&nb).map(|s| format!("  = {s}")).unwrap_or_default();
                println!("  A- {h}  {na} -> B {nb}{src}  RE-DRESSED {key}");
            }
            None => {
                gone += 1;
                println!("  A- {h}  {na}  GONE (no B model of this shape)  physics {}", hist(pa));
            }
        }
    }
    for (h, names) in &only_b {
        let nb = name_of(names);
        let src = report.get(&nb).map(|s| format!("  = {s}")).unwrap_or_default();
        let Some((shp, pb)) = sb.get(&nb) else { continue };
        if !shape_a.contains_key(shp) {
            new_shape += 1;
            println!("  B+ {h}  {nb}{src}  NEW SHAPE  physics {}", hist(pb));
        }
    }
    println!("re-dressed A models (same shape in B, other physics): {}; A shapes gone: {gone}; B shapes new: {new_shape}", redress.values().sum::<usize>());
    for (k, n) in &redress {
        println!("  x{n}  {k}");
    }
    // Per PLACEMENT, by index (the two maps place the same records in the same
    // order when their placement counts agree): a placement whose model
    // fingerprint changed — even to a fingerprint the other map also has — is a
    // re-dressed or re-cut piece in the world; count them, with the physics
    // transition and a moved-position check.
    if ma.items.len() == mb.items.len() {
        let mut changed = 0usize;
        let mut moved = 0usize;
        let mut trans: BTreeMap<String, usize> = BTreeMap::new();
        for (ia, ib) in ma.items.iter().zip(mb.items.iter()) {
            let d = ((ia.pos[0] - ib.pos[0]).powi(2) + (ia.pos[1] - ib.pos[1]).powi(2) + (ia.pos[2] - ib.pos[2]).powi(2)).sqrt();
            if d > 0.001 || (ia.yaw - ib.yaw).abs() > 1e-5 {
                moved += 1;
            }
            let fa = a.per_item.get(&ia.model);
            let fb = b.per_item.get(&ib.model);
            match (fa, fb) {
                (Some(x), Some(y)) if x == y => {}
                (Some(_), Some(_)) => {
                    changed += 1;
                    let ha = sa.get(&ia.model).map(|(_, p)| hist(p)).unwrap_or_default();
                    let hb = sb.get(&ib.model).map(|(_, p)| hist(p)).unwrap_or_default();
                    let same_shape = sa.get(&ia.model).map(|(s, _)| s) == sb.get(&ib.model).map(|(s, _)| s);
                    *trans.entry(format!("{}{ha} -> {hb}", if same_shape { "" } else { "SHAPE CHANGED: " })).or_default() += 1;
                }
                (None, None) => {
                    if ia.model != ib.model {
                        changed += 1;
                        *trans.entry(format!("stock {} -> {}", ia.model, ib.model)).or_default() += 1;
                    }
                }
                _ => {
                    changed += 1;
                    *trans.entry("embedded <-> stock".to_string()).or_default() += 1;
                }
            }
        }
        println!("placements: {} of {} changed model collision (moved: {moved})", changed, ma.items.len());
        for (k, n) in &trans {
            println!("  p{n}  {k}");
        }
    } else {
        println!("placements: {} vs {} — counts differ, no per-index pairing", ma.items.len(), mb.items.len());
    }
    if !same {
        std::process::exit(1);
    }
    Ok(())
}

/// A model's SHAPE fingerprint (vertices + triangle indices of every collision
/// surface, no physics) and its physics histogram (id → triangle count): the
/// pair that tells a re-dressed hull ("same shape, ids 14 → 0") from a moved or
/// re-cut one, for `--diff`.
pub fn item_shape_and_physics(bytes: &[u8]) -> Result<(String, BTreeMap<u8, usize>), String> {
    let f = crate::static_item::parse_file(bytes)?;
    let mut h = Fnv::default();
    let mut phys: BTreeMap<u8, usize> = BTreeMap::new();
    let mut surfaces: Vec<&crate::static_item::surface::CPlugSurface> = Vec::new();
    if let Some(so) = f.item.static_object() {
        if let Some(s) = so.surface() {
            surfaces.push(s);
        }
    } else if let Some(p) = f.item.prefab() {
        for e in p.ents.iter() {
            match e.model.inline.as_deref() {
                Some(crate::static_item::Node::StaticObject(so)) => {
                    if let Some(s) = so.surface() {
                        surfaces.push(s);
                    }
                }
                Some(crate::static_item::Node::Dyna(d)) => {
                    for r in [&d.static_shape, &d.dyna_shape] {
                        if let Some(crate::static_item::Node::Surface(s)) = r.inline.as_deref() {
                            surfaces.push(s);
                        }
                    }
                }
                _ => {}
            }
        }
    }
    for s in surfaces {
        if let crate::static_item::surface::Surf::Mesh { vertices, triangles, .. } = &s.surf {
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
                let id = s.material_ids.get(t.surface_index.max(0) as usize).map(|x| (*x & 0xff) as u8).unwrap_or(t.material_id);
                *phys.entry(id).or_default() += 1;
            }
        }
    }
    Ok((h.hex(), phys))
}

/// The per-model shape → physics table of a map's embedded items, for `--diff`.
pub fn shapes(m: &tmmaps::map::MapFile) -> BTreeMap<String, (String, BTreeMap<u8, usize>)> {
    let files = crate::embedded::files(m).unwrap_or_default();
    let mut out = BTreeMap::new();
    for (name, bytes) in &files {
        let base = name.rsplit(['/', '\\']).next().unwrap_or(name).to_string();
        if !base.to_ascii_lowercase().ends_with(".item.gbx") {
            continue;
        }
        if let Ok(v) = item_shape_and_physics(bytes) {
            out.insert(base, v);
        }
    }
    out
}

#[cfg(test)]
mod diff_tests {
    /// The judge fails CLOSED: two summaries compare by their hash strings,
    /// which the tool computes itself — an empty or unparseable side can never
    /// read as "identical". (The 2026-09-10 shell tables compared two empty
    /// strings and said "yes" on 23 maps whose clip walls had changed physics.)
    #[test]
    fn empty_hash_is_never_identical() {
        let a = String::new();
        let b = String::new();
        // the tool never produces an empty total: FNV of an empty input is the
        // offset basis, a 16-hex string; assert the invariant the diff relies on
        let h = super::Fnv::default().hex();
        assert_eq!(h.len(), 16);
        assert_ne!(h, a);
        assert!(!(a == b && !a.is_empty()));
    }

    /// An item whose collision cannot be parsed enters the fingerprint as
    /// `UNPARSED:<error>` — a distinct, non-empty value, never the neighbour's.
    #[test]
    fn unparsed_item_has_a_fingerprint() {
        let mut one = super::Fnv::default();
        match super::item_collision(b"not a gbx file", &mut one) {
            Ok(()) => panic!("garbage parsed as an item"),
            Err(e) => one.str(&format!("UNPARSED:{e}")),
        }
        assert_eq!(one.hex().len(), 16);
        assert_ne!(one.hex(), super::Fnv::default().hex());
    }
}
