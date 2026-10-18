//! `mapgeom item-scale IN.Item.Gbx --out F --ident NAME.Item.Gbx [--author A]
//! [--scale S | --sx X --sy Y --sz Z] [--name N] [--description D]
//! [--catalog N] [--collection N]`: an already-baked item at another size.
//!
//! Why (Altrd_Amill101, 2026-10-18): he wants a checkpoint gate "half the
//! height of the tiny CP and 1 m or 2 m wide". The size that matters is the
//! WAYPOINT TRIGGER, and the game does not scale a trigger with the
//! placement's scale word (measured on the box: a tiny CP placed at scale 0.25
//! still collected a car 2 m off centre) — so a smaller checkpoint has to be a
//! smaller ITEM.
//!
//! Re-baking from the packs is the usual road (`item-set-items --scale`), but
//! it needs a readable client pak; this takes the item file we already
//! shipped and scales every length in it: visual vertex positions and their
//! bounding boxes, collision and TRIGGER surfaces (mesh vertices, boxes,
//! spheres, hulls), prefab entity positions, the spawn point, and the
//! placement parameters (grid steps, pivots, magnets).
//!
//! Non-uniform factors are allowed (`--sx 0.25 --sy 0.5 --sz 0.5` is the 1 m
//! wide, half height gate); vertex normals stored as plain floats are carried
//! through the inverse transform and renormalised, packed ones are left alone.
use crate::static_item::{self, item::ItemChunk, Node};

pub fn cmd(rest: &[String]) {
    if let Err(e) = run(rest) {
        eprintln!("item-scale: {e}");
        std::process::exit(1);
    }
}

/// How many values each pass touched — the proof that the walk reached the
/// geometry rather than silently doing nothing.
#[derive(Default, Debug)]
pub struct Counts {
    pub positions: usize,
    pub normals: usize,
    pub bboxes: usize,
    pub surf_vertices: usize,
    pub surf_prims: usize,
    pub entities: usize,
    pub spawns: usize,
    pub placements: usize,
    pub unknown: Vec<String>,
}

fn run(rest: &[String]) -> Result<(), String> {
    let flag = |n: &str| rest.iter().position(|a| a == n).and_then(|i| rest.get(i + 1)).cloned();
    let input = rest.get(1).filter(|a| !a.starts_with("--")).cloned().ok_or("item-scale IN.Item.Gbx --out F --ident NAME.Item.Gbx")?;
    let out = flag("--out").ok_or("--out F is required")?;
    let ident = flag("--ident").ok_or("--ident NAME.Item.Gbx is required")?;
    let s: f32 = flag("--scale").unwrap_or_else(|| "0.5".into()).parse().map_err(|e| format!("--scale: {e}"))?;
    let sx: f32 = flag("--sx").map(|v| v.parse()).transpose().map_err(|e| format!("--sx: {e}"))?.unwrap_or(s);
    let sy: f32 = flag("--sy").map(|v| v.parse()).transpose().map_err(|e| format!("--sy: {e}"))?.unwrap_or(s);
    let sz: f32 = flag("--sz").map(|v| v.parse()).transpose().map_err(|e| format!("--sz: {e}"))?.unwrap_or(s);
    if sx <= 0.0 || sy <= 0.0 || sz <= 0.0 {
        return Err("scale factors must be positive".into());
    }
    let bytes = std::fs::read(&input).map_err(|e| format!("{input}: {e}"))?;
    let mut f = static_item::parse_file(&bytes)?;

    let k = [sx, sy, sz];
    let mut c = Counts::default();
    for ch in f.item.chunks.iter_mut() {
        match ch {
            ItemChunk::Model(mc) => {
                if let Some(n) = mc.entity_model.inline.as_deref_mut() {
                    scale_node(n, k, &mut c);
                }
            }
            ItemChunk::DefaultPlacement { placement, .. } => {
                let Some(Node::Placement(p)) = placement.inline.as_deref_mut() else { continue };
                let old = static_item::placement::PlacementParam::from_node(p)?;
                let sclass_index = old.sclass.as_ref().map(|(i, _)| *i).unwrap_or(-1);
                // the placement's lengths are horizontal grid steps, a vertical
                // fly step and pivot/magnet points: one factor each would need
                // three scalers, and a gate is placed on the horizontal grid —
                // sx drives it, sy the vertical step
                let mut np = crate::item_set::scaled_placement(&old, sx, sclass_index);
                np.grid_v_step = if old.grid_v_step > 0.0 { old.grid_v_step * sy } else { old.grid_v_step };
                np.fly_v_step = if old.fly_v_step > 0.0 { old.fly_v_step * sy } else { old.fly_v_step };
                np.pivot_positions = old.pivot_positions.iter().map(|v| [v[0] * sx, v[1] * sy, v[2] * sz]).collect();
                np.magnet_locs = old.magnet_locs.iter().map(|(p, r)| ([p[0] * sx, p[1] * sy, p[2] * sz], *r)).collect();
                *p = np.to_node()?;
                c.placements += 1;
            }
            _ => {}
        }
    }

    // ident / name / description, so the copy is its own item in the browser
    let stem = ident.rsplit('\\').next().unwrap_or(&ident).trim_end_matches(".Item.Gbx").to_string();
    let name = flag("--name").unwrap_or_else(|| stem.clone());
    let author = flag("--author");
    for ch in f.item.chunks.iter_mut() {
        match ch {
            ItemChunk::Ident { path, author: a, .. } => {
                *path = static_item::Id::Str(ident.clone());
                if let Some(au) = &author {
                    *a = static_item::Id::Str(au.clone());
                }
            }
            ItemChunk::Name(n) => *n = name.clone(),
            ItemChunk::Description(d) => {
                if let Some(desc) = flag("--description") {
                    *d = desc;
                }
            }
            _ => {}
        }
    }
    // the header's collector chunk carries the browser's copy of all that
    let collection: u32 = flag("--collection").map(|v| v.parse()).transpose().map_err(|e| format!("--collection: {e}"))?.unwrap_or(26);
    let catalog: i16 = flag("--catalog").map(|v| v.parse()).transpose().map_err(|e| format!("--catalog: {e}"))?.unwrap_or(1);
    let header_author = author.clone().unwrap_or_else(|| {
        f.item
            .chunks
            .iter()
            .find_map(|ch| match ch {
                ItemChunk::Ident { author: static_item::Id::Str(a), .. } => Some(a.clone()),
                _ => None,
            })
            .unwrap_or_else(|| "Nadeo".into())
    });
    if let Some(i) = f.header_chunks.iter().position(|h| h.id == 0x2E001003) {
        f.header_chunks[i] = crate::item_set::desc_chunk_at(&ident, collection, &header_author, &name, catalog);
    }
    f.body_comp = b'C';
    let written = static_item::write_file(&f);
    std::fs::write(&out, &written).map_err(|e| format!("{out}: {e}"))?;
    println!(
        "{input} x({sx}, {sy}, {sz}) -> {out} ({} bytes): {} vertex positions, {} normals, {} bounding boxes, {} collision/trigger vertices, {} primitives, {} entities, {} spawn(s), {} placement(s)",
        written.len(),
        c.positions,
        c.normals,
        c.bboxes,
        c.surf_vertices,
        c.surf_prims,
        c.entities,
        c.spawns,
        c.placements
    );
    for u in &c.unknown {
        println!("  not scaled: {u}");
    }
    Ok(())
}

fn scale_ref(r: &mut static_item::Ref, k: [f32; 3], c: &mut Counts) {
    if let Some(n) = r.inline.as_deref_mut() {
        scale_node(n, k, c);
    }
}

/// Every length inside one node, recursively.
pub fn scale_node(n: &mut Node, k: [f32; 3], c: &mut Counts) {
    match n {
        Node::EntityModel(em) => {
            scale_ref(&mut em.static_object, k, c);
            scale_ref(&mut em.trigger_shape, k, c);
            // the entity model's own Iso4 translation
            for j in 0..3 {
                em.iso[9 + j] *= k[j];
            }
        }
        Node::StaticObject(so) => {
            scale_ref(&mut so.mesh, k, c);
            scale_ref(&mut so.shape, k, c);
        }
        Node::Solid2(s2) => {
            for b in s2.boxes.iter_mut() {
                scale_box6(b, k);
                c.bboxes += 1;
            }
            for v in s2.visuals.iter_mut() {
                scale_ref(v, k, c);
            }
            for l in s2.lights.iter_mut() {
                // the light socket's Iso4 translation
                for j in 0..3 {
                    l.u05[9 + j] *= k[j];
                }
                scale_ref(&mut l.node, k, c);
            }
        }
        Node::Visual(v) => {
            if let Some(m) = v.main.as_mut() {
                scale_box6(&mut m.bounding_box, k);
                c.bboxes += 1;
                for s in m.vertex_streams.iter_mut() {
                    scale_ref(s, k, c);
                }
            }
            for (_, _, b) in v.splits.iter_mut() {
                scale_box6(b, k);
                c.bboxes += 1;
            }
        }
        Node::VertexStream(vs) => {
            let compress = vs.compress_local3d.unwrap_or(false);
            // decls and elems are in the same order
            let names: Vec<(u32, u32)> = vs.decls.iter().map(|d| (d.name(), d.stored_type(compress))).collect();
            for (i, e) in vs.elems.iter_mut().enumerate() {
                let Some((name, _)) = names.get(i).copied() else { continue };
                match (name, e) {
                    (static_item::vstream::N_POSITION, crate::static_item::vstream::Elem::Float3(p)) => {
                        for v in p.iter_mut() {
                            v[0] *= k[0];
                            v[1] *= k[1];
                            v[2] *= k[2];
                        }
                        c.positions += p.len();
                    }
                    // a non-uniform scale turns a normal by the INVERSE
                    // transform (and it must be unit length again)
                    (static_item::vstream::N_NORMAL, crate::static_item::vstream::Elem::Float3(p)) => {
                        if k[0] != k[1] || k[1] != k[2] {
                            for v in p.iter_mut() {
                                let n = [v[0] / k[0], v[1] / k[1], v[2] / k[2]];
                                let l = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt();
                                if l > 1e-6 {
                                    *v = [n[0] / l, n[1] / l, n[2] / l];
                                }
                            }
                            c.normals += p.len();
                        }
                    }
                    (static_item::vstream::N_NORMAL, crate::static_item::vstream::Elem::Word(_)) if k[0] != k[1] || k[1] != k[2] => {
                        c.unknown.push("packed vertex normals (DEC3N) under a non-uniform scale".into());
                    }
                    _ => {}
                }
            }
        }
        Node::Surface(s) => {
            scale_surf(&mut s.surf, k, c);
        }
        Node::Prefab(p) => {
            for e in p.ents.iter_mut() {
                e.pos[0] *= k[0];
                e.pos[1] *= k[1];
                e.pos[2] *= k[2];
                c.entities += 1;
                scale_ref(&mut e.model, k, c);
            }
        }
        Node::WaypointTrigger(w) => scale_ref(&mut w.shape, k, c),
        Node::GateSpecial(g) => scale_ref(&mut g.shape, k, c),
        Node::VariantList(vl) => {
            for v in vl.variants.iter_mut() {
                scale_ref(&mut v.model, k, c);
            }
        }
        // NPlugTrigger_SSpawn (0x0917A000): chunk id, version, then the Iso4
        // whose last three floats are the translation (assemble.rs writes it)
        Node::Opaque(o) if o.class_id == 0x0917A000 => {
            let n = o.raw.len();
            // the Iso4 ends 24 bytes + FACADE before the tail the writer adds;
            // find it by its own layout instead of a fixed offset
            if n >= 4 * (1 + 12 + 6 + 1) {
                let base = n - 4 * (6 + 1) - 4 * 3; // translation = the three floats before the 24-byte tail
                for (j, s) in k.iter().enumerate() {
                    let o0 = base + j * 4;
                    let v = f32::from_le_bytes([o.raw[o0], o.raw[o0 + 1], o.raw[o0 + 2], o.raw[o0 + 3]]) * s;
                    o.raw[o0..o0 + 4].copy_from_slice(&v.to_le_bytes());
                }
                c.spawns += 1;
            } else {
                c.unknown.push(format!("spawn node of {n} bytes (layout not recognised)"));
            }
        }
        Node::Dyna(_) | Node::Kinematic(_) => c.unknown.push("a moving (dyna) part".into()),
        _ => {}
    }
}

fn scale_box6(b: &mut [f32; 6], k: [f32; 3]) {
    for j in 0..3 {
        b[j] *= k[j];
        b[j + 3] *= k[j];
    }
}

fn scale_surf(s: &mut static_item::surface::Surf, k: [f32; 3], c: &mut Counts) {
    use static_item::surface::Surf;
    match s {
        Surf::Mesh { vertices, .. } => {
            for v in vertices.iter_mut() {
                v[0] *= k[0];
                v[1] *= k[1];
                v[2] *= k[2];
            }
            c.surf_vertices += vertices.len();
        }
        Surf::Sphere { size, .. } => {
            *size *= k[0].min(k[1]).min(k[2]);
            c.surf_prims += 1;
        }
        Surf::Ellipsoid { size, .. } => {
            size[0] *= k[0];
            size[1] *= k[1];
            size[2] *= k[2];
            c.surf_prims += 1;
        }
        Surf::Box { transform, .. } => {
            scale_box6(transform, k);
            c.surf_prims += 1;
        }
        Surf::ConvexPolyhedron { center, half, vertices, .. } => {
            for j in 0..3 {
                center[j] *= k[j];
                half[j] *= k[j];
            }
            for v in vertices.iter_mut() {
                v[0] *= k[0];
                v[1] *= k[1];
                v[2] *= k[2];
            }
            c.surf_vertices += vertices.len();
            c.surf_prims += 1;
        }
        _ => c.unknown.push("a collision primitive of an unhandled kind".into()),
    }
}
