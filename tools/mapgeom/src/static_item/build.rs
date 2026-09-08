//! Building static items (S3): every `CPlugStaticObjectModel` of a prefab
//! (recursively through its external prefabs), or a Nadeo item's own static
//! object / baked crystal, merged into ONE `CPlugSolid2Model` + ONE
//! `CPlugSurface` and written in the reference `.Item.Gbx` layout.
//!
//! * visuals are appended as they are (their own vertex layout, index
//!   buffer and flags), positions transformed by the entity iso and scaled,
//!   packed normals/tangents rotated, bounding boxes recomputed;
//! * materials are `CPlugMaterialUserInst` game materials deduplicated by
//!   (link, physics id);
//! * the collision mesh is the union of the entity surfaces (triangle
//!   physics id kept, the u16 id list deduplicated), or the visual triangles
//!   of a mesh-collidable object.
//!
//! The pieces: `merged.rs` (the accumulator and what merges into it),
//! `lod.rs` (the detail ladder), `materials.rs` (physics tables and the
//! material rewrites), `assemble.rs` (the accumulator as an item file) — all
//! re-exported here — and, in this file, the walks that FEED the accumulator:
//! prefabs (`add_prefab`), pack static objects, the moving and tween parts,
//! the effect systems, the vegetation bake, and the `static_item_from_*`
//! entry points.

use super::surface::{CPlugSurface, Surf, Triangle};
use super::vstream::{Elem, N_COLOR0, N_NORMAL, N_TANGENT_U, N_TANGENT_V, N_TEXCOORD0};
use super::{Node, Ref, R};
use crate::crystal_model::CPlugMaterialUserInst;
use crate::geom::{apply, compose, Xform, IDENTITY};

pub use super::assemble::*;
pub use super::lod::*;
pub use super::materials::*;
pub use super::merged::*;

/// A gameplay gate BLOCK's trigger disc: the block info (GateSpecialBoost…
/// .EDClassic.Gbx) carries it inline — a CPlugSolid → CPlugTree →
/// CPlugSurface of 68 triangles whose one material is
/// `Effects\Media\Material\CollisionTurbo` (the prefab itself,
/// `Special_AirV2.Prefab.Gbx`, is the arch alone: one static-object entity,
/// no NPlugTrigger_SGateSpecial). Found as the file's surface whose material
/// path names `\Collision`; returned in block space with the disc's own
/// (physics, gameplay) bytes.
pub fn blockinfo_special_trigger(store: &mut crate::store::DataStore, blockinfo_path: &str) -> Option<(Vec<[f32; 3]>, Vec<super::surface::Triangle>, (u8, u8), [f32; 3])> {
    let model = store.load_model(blockinfo_path).ok()?;
    let g = model.graph().ok()?;
    let path_of = |i: i32| -> Option<String> {
        match g.slots.get(i.max(0) as usize) {
            Some(crate::node::Slot::External(p)) => Some(p.clone()),
            _ => None,
        }
    };
    for slot in &g.slots {
        let crate::node::Slot::Node(crate::node::Node::Surface(sf)) = slot else { continue };
        let is_collision = sf.materials.iter().any(|m| path_of(*m).map(|p| p.to_ascii_lowercase().contains("\\collision")).unwrap_or(false));
        if !is_collision || sf.meshes.is_empty() {
            continue;
        }
        let mut verts: Vec<[f32; 3]> = Vec::new();
        let mut tris: Vec<super::surface::Triangle> = Vec::new();
        let mut ids = (0u8, 0u8);
        for mesh in &sf.meshes {
            let base = verts.len() as u32;
            verts.extend(mesh.verts.iter().copied());
            for (f, phys, gp) in &mesh.tris {
                ids = (*phys, *gp);
                tris.push(super::surface::Triangle { indices: [f[0] as u32 + base, f[1] as u32 + base, f[2] as u32 + base], material_id: *phys, gameplay: *gp, surface_index: 0 });
            }
        }
        if !tris.is_empty() {
            return Some((verts, tris, ids, sf.main_dir.unwrap_or([0.0, 0.0, 1.0])));
        }
    }
    None
}


/// Walk a prefab (and its external prefabs, recursively) adding every static
/// object placed by `at`.
/// A pack `*_Trigger.Shape.Gbx` (CPlugSurface) as a waypoint trigger in the
/// item's scaled frame — the canonical mesh form the game accepts from an item
/// (the pack surface kept verbatim, materials and ids included, made the game
/// drop the whole item: GateCheckpointLeft32m, 2026-09-06). `at` is the frame
/// the shape is authored in (identity for a block's own trigger).
pub fn trigger_from_shape_file(store: &mut crate::store::DataStore, shape_path: &str, at: &Xform, scale: f32) -> R<super::surface::CPlugSurface> {
    let sm = store.load_model(shape_path)?;
    let mut lb = super::LookbackState::default();
    lb.defined_nodes.extend(sm.external_indices().iter().copied());
    let mut r = super::Rd::new(&sm.body, 0, lb);
    let sf = super::surface::CPlugSurface::parse(&mut r).map_err(|e| format!("{shape_path}: {e}"))?;
    trigger_mesh(&sf, at, scale, (0, 0)).ok_or_else(|| format!("{shape_path}: trigger shape has no triangles (surf type {})", sf.surf.type_id()))
}

/// A trigger shape's surface as the ONE mesh the item form takes: every
/// child of a compound placed by its Iso4 (the block gates' `Gate\
/// Checkpoint_Trigger.Shape.Gbx` is a Compound of one 36-vertex disc —
/// until 2026-09-08 "not a mesh surface" sent the GateFinish block to the
/// unit-box fallback, a 32 m cube where the game fires on a 0.9 m disc:
/// vjeux, tiny 20, "the finish trigger seems to be too big"), the primitives
/// meshed, the vertices through `at` and `scale`, every triangle carrying
/// `ids` = (physics, gameplay) — (0, 0) for a waypoint, the effect for a
/// gameplay gate — in its bytes and in the one-entry id table.
pub fn trigger_mesh(sf: &super::surface::CPlugSurface, at: &Xform, scale: f32, ids: (u8, u8)) -> Option<super::surface::CPlugSurface> {
    let (vertices, triangles) = sf.surf.triangulate()?;
    if triangles.is_empty() {
        return None;
    }
    let verts: Vec<[f32; 3]> = vertices.iter().map(|v| { let t = apply(at, *v); [t[0] * scale, t[1] * scale, t[2] * scale] }).collect();
    let tris: Vec<super::surface::Triangle> = triangles.iter().map(|t| super::surface::Triangle { indices: t.indices, material_id: ids.0, gameplay: ids.1, surface_index: 0 }).collect();
    let dir = sf.gameplay_main_dir.unwrap_or([0.0, 0.0, 1.0]);
    let d = [at[0] * dir[0] + at[1] * dir[1] + at[2] * dir[2], at[3] * dir[0] + at[4] * dir[1] + at[5] * dir[2], at[6] * dir[0] + at[7] * dir[1] + at[8] * dir[2]];
    Some(super::surface::CPlugSurface::mesh(verts, tris, vec![ids.0 as u16 | ((ids.1 as u16) << 8)], d))
}

pub fn add_prefab(store: &mut crate::store::DataStore, path: &str, at: &Xform, scale: f32, m: &mut Merged, depth: usize) -> R<()> {
    if depth > 8 {
        return Err(format!("{path}: prefab nesting deeper than 8"));
    }
    let model = store.load_model(path)?;
    let prefab = super::prefab::CPlugPrefab::from_model(&model)?;
    let externals = model.externals.clone();
    let ext_name = |i: i32| externals.iter().find(|(k, _)| *k as i32 == i).map(|(_, p)| p.clone());
    // The kinematic constraints of this prefab: which entity each one moves
    // (its params name the dyna object by its rank among the prefab's dyna
    // entities — Ent2 = 0 is the FIRST CPlugDynaObjectModel entity, not
    // entity 0: ObstacleRotor24mWing90X2 lists the constraint first; -1 is
    // the world), and the constraint file — through the item's Level modifier
    // when it has one (the game skin swaps `KinematicConstraints\ObstacleX`
    // for `Modifier\ItemObstacle\AnimX<Level>`, whose ranges are the real
    // ones; the prefab's own file has a zero range).
    let dyna_entities: Vec<i32> = prefab
        .ents
        .iter()
        .enumerate()
        .filter(|(_, e)| e.model.inline.is_none() && e.model.index >= 0 && ext_name(e.model.index).map(|p| p.to_ascii_lowercase().ends_with(".dynaobject.gbx")).unwrap_or(false))
        .map(|(i, _)| i as i32)
        .collect();
    let mut constraints: Vec<(i32, String, super::dyna::ConstraintParams)> = Vec::new();
    for e in &prefab.ents {
        if e.params_id != super::dyna::P_CONSTRAINT || e.model.inline.is_some() || e.model.index < 0 {
            continue;
        }
        let Some(cp) = ext_name(e.model.index).filter(|p| p.to_ascii_lowercase().ends_with(".kinematicconstraint.gbx")) else { continue };
        let Some(params) = super::dyna::ConstraintParams::parse(&e.params) else {
            m.notes.push(format!("{path}: constraint {cp} with {}-byte params skipped", e.params.len()));
            continue;
        };
        let rank = if params.ent2 >= 0 { params.ent2 } else { params.ent1 };
        let Some(target) = dyna_entities.get(rank.max(0) as usize).copied().filter(|_| rank >= 0) else {
            m.notes.push(format!("{path}: constraint {cp} binds dyna object {rank}, which the prefab does not have ({} dyna entities)", dyna_entities.len()));
            continue;
        };
        constraints.push((target, modified_constraint_path(store, m, &cp), params));
    }
    for (i, e) in prefab.ents.iter().enumerate() {
        let iso = compose(at, &super::prefab::CPlugPrefab::entity_iso(e));
        match e.model.inline.as_deref() {
            Some(Node::StaticObject(so)) => {
                // Physics: the game's library table first, the .Material.Gbx's
                // own surface id (usually 0) next, else the object's most
                // common collision physics.
                let common = most_common_physics(so);
                let mut resolve = |idx: i32| -> Option<(String, String, u8)> {
                    let p = ext_name(idx)?;
                    let link = material_link(&p);
                    let phys = physics_for_link(&link).or_else(|| material_physics(store, &p).filter(|x| *x != 0)).or(common).unwrap_or(0);
                    Some((p, link, phys))
                };
                m.add_static_object(so, &iso, scale, &mut resolve).map_err(|err| format!("{path} entity {i}: {err}"))?;
                m.resolve_pending_lights(store);
            }
            // NPlugTrigger_SWaypoint: { version, waypoint type, trigger shape ref,
            // u32 } (read off Items\Gate\CheckpointLeft32m: 01 00 00 00 | 02 00 00
            // 00 = checkpoint | 1d 00 00 00 = node 29, the external
            // *_Trigger.Shape.Gbx | 00 00 00 00). The item gets that type and the
            // shape, transformed like the geometry.
            Some(Node::Opaque(o)) if o.class_id == 0x09178000 && o.raw.len() >= 16 => {
                let wtype = i32::from_le_bytes(o.raw[4..8].try_into().unwrap());
                let shape_idx = i32::from_le_bytes(o.raw[8..12].try_into().unwrap());
                match ext_name(shape_idx) {
                    Some(sp) if sp.to_ascii_lowercase().ends_with(".shape.gbx") => match store.load_model(&sp) {
                        Ok(sm) => {
                            let mut lb = super::LookbackState::default();
                            lb.defined_nodes.extend(sm.external_indices().iter().copied());
                            let mut r = super::Rd::new(&sm.body, 0, lb);
                            match super::surface::CPlugSurface::parse(&mut r) {
                                Ok(sf) => {
                                    // re-emitted in the canonical form the box
                                    // triggers use: the pack shape kept verbatim
                                    // (its materials/ids) made the game drop the
                                    // whole item (GateCheckpointLeft32m, 2026-09-06)
                                    let Some(t) = trigger_mesh(&sf, &iso, scale, (0, 0)) else {
                                        m.notes.push(format!("{path} entity {i}: trigger shape {sp} has no triangles (surf type {}); skipped", sf.surf.type_id()));
                                        continue;
                                    };
                                    m.trigger = Some(t);
                                    m.waypoint_type = Some(wtype);
                                    m.notes.push(format!("{path} entity {i}: waypoint trigger type {wtype} from {sp}"));
                                }
                                Err(e) => m.notes.push(format!("{path} entity {i}: trigger shape {sp} failed: {e}")),
                            }
                        }
                        Err(e) => m.notes.push(format!("{path} entity {i}: trigger shape {sp} failed: {e}")),
                    },
                    _ => m.notes.push(format!("{path} entity {i}: waypoint trigger type {wtype} with shape node {shape_idx} (not an external shape) skipped")),
                }
            }
            // NPlugTrigger_SGateSpecial (0x09179000): { version, trigger shape
            // ref, u32 } — the GAMEPLAY gate's effect volume (Special24m.Prefab
            // entity 1: `Special_Trigger24m.Shape.Gbx`, a 24×7.18×0.9 m slab
            // whose 12 triangles carry (physics 0, gameplay 1 Turbo) — the
            // prefab is authored in its Turbo dress; the item's
            // `<Kind>.TerrainModifier` folder re-dresses it with
            // `Modifier\<Kind>\Collision.Material.Gbx`, chunk 0x09079017 =
            // [physics, gameplay, …]: Boost 18 ReactorBoost_Oriented, NoEngine
            // 4 FreeWheeling, Reset 8; Turbo has no Collision file, the shape's
            // own 1 stands). Until this arm existed the entity was "skipped"
            // and every tiny special gate was a plain metal arch (vjeux drove
            // tiny 20, 2026-09-08: "all the special blocks are not working").
            // The item form of the same thing is what the mesh editor writes
            // for Nadeo.zip's crystal GateSpecialNoEngine: a TRIGGER layer in
            // material `Modifier\NoEngine\Collision` (physics 0, gameplay 4) —
            // i.e. the entity model's trigger shape with the gameplay id on its
            // triangles and in its id table, waypoint type None.
            Some(Node::GateSpecial(g)) => {
                let shape_idx = g.shape.index;
                let word = g.u01;
                match ext_name(shape_idx) {
                    Some(sp) if sp.to_ascii_lowercase().ends_with(".shape.gbx") => match store.load_model(&sp) {
                        Ok(sm) => {
                            let mut lb = super::LookbackState::default();
                            lb.defined_nodes.extend(sm.external_indices().iter().copied());
                            let mut r = super::Rd::new(&sm.body, 0, lb);
                            match super::surface::CPlugSurface::parse(&mut r) {
                                Ok(sf) => {
                                    // the modifier's Collision material decides; the shape's own bytes otherwise
                                    let own = match &sf.surf {
                                        super::surface::Surf::Mesh { triangles, .. } => triangles.first().map(|t| (t.material_id, t.gameplay)).unwrap_or((0, 0)),
                                        _ => (0, 0),
                                    };
                                    let (ids, from) = match special_collision_ids(store, m) {
                                        Some((link, ids)) => (ids, link),
                                        None => (own, format!("{sp} (no modifier Collision material)")),
                                    };
                                    if ids.1 == 0 {
                                        m.notes.push(format!("{path} entity {i}: special trigger from {from} has gameplay 0 — the gate would do nothing; skipped"));
                                        continue;
                                    }
                                    let Some(t) = trigger_mesh(&sf, &iso, scale, ids) else {
                                        m.notes.push(format!("{path} entity {i}: special trigger shape {sp} has no triangles (surf type {}); skipped", sf.surf.type_id()));
                                        continue;
                                    };
                                    if m.special.is_some() {
                                        m.notes.push(format!("{path} entity {i}: a second special trigger ({sp}) replaces the first"));
                                    }
                                    let (n, d) = (t.surf.counts().1, t.gameplay_main_dir);
                                    // ENGINE-VERIFIED 2026-09-08 (tiny 20, full-throttle drive
                                    // through the Boost gate, wheel log): the effect fires ONLY
                                    // from a prefab entity of this class — the same slab as the
                                    // entity model's trigger shape (with or without a material
                                    // node, gameplay 1/12/18), or in the collision hull as
                                    // NotCollidable + gameplay (a wall), did nothing. So the
                                    // item takes the pack's own PREFAB layout (`assemble`).
                                    m.special = Some(t);
                                    m.notes.push(format!("{path} entity {i}: special trigger physics {} gameplay {} from {from} ({n} triangles, word 0x{word:08x}, main dir {:?}) — prefab form", ids.0, ids.1, d));
                                }
                                Err(e) => m.notes.push(format!("{path} entity {i}: special trigger shape {sp} failed: {e}")),
                            }
                        }
                        Err(e) => m.notes.push(format!("{path} entity {i}: special trigger shape {sp} failed: {e}")),
                    },
                    _ => m.notes.push(format!("{path} entity {i}: special trigger with shape node {shape_idx} (not an external shape) skipped")),
                }
            }
            // NPlugTrigger_SSpawn (0x0917A000): the START gate's spawn point —
            // entity 0 of `Items\Gate\StartLeft8m.Prefab` sits at local
            // (0, 0, -10.6), and the car of the original Summer 15 stands at
            // x 1003.4 = 1014 - 10.6 (measured in play, 2026-09-07). A
            // checkpoint prefab carries one too (its respawn point). The
            // entity's position, through the chain and the scale, becomes the
            // item's spawn iso; the FIRST one wins (a nested prefab's would be
            // a helper). Until this arm existed the class was "skipped" and
            // every item-start map (Summer 15/20/25: GateStart items) had no
            // spawn at all — the playground opened with NO CAR.
            Some(Node::Opaque(o)) if o.class_id == 0x0917A000 => {
                let at_world = apply(&iso, [0.0, 0.0, 0.0]);
                let sp = [at_world[0] * scale, at_world[1] * scale, at_world[2] * scale];
                if m.spawn == [0.0, 0.0, 0.0] {
                    m.spawn = sp;
                    m.notes.push(format!("{path} entity {i}: spawn point {:?} (NPlugTrigger_SSpawn)", sp));
                } else {
                    m.notes.push(format!("{path} entity {i}: a second spawn point {:?} ignored (kept {:?})", sp, m.spawn));
                }
            }
            Some(other) => m.notes.push(format!("{path} entity {i}: model class 0x{:08X} skipped", other.class_id())),
            None if e.model.index < 0 => {}
            None => match ext_name(e.model.index) {
                Some(p) if p.to_ascii_lowercase().ends_with(".prefab.gbx") => add_prefab(store, &p, &iso, scale, m, depth + 1)?,
                // an external static object (gate speedometer lights, road
                // signs): its own file, mesh and shape external again
                Some(p) if p.to_ascii_lowercase().ends_with(".staticobject.gbx") => {
                    if let Err(e) = add_static_object_file(store, &p, &iso, scale, m) {
                        m.notes.push(format!("{path} entity {i}: external {p} failed: {e}"));
                    }
                }
                // a dynamic object: a MOVING part when a constraint of this
                // prefab binds it (pusher pistons, rotor discs — kept as a
                // dyna entity with its constraint, see `DynaPart`); otherwise
                // (the flag cloth of Flag16m/Flag8m) its mesh at rest, no
                // collision.
                Some(p) if p.to_ascii_lowercase().ends_with(".dynaobject.gbx") => {
                    let bound = constraints.iter().find(|(target, _, _)| *target == i as i32).cloned();
                    match bound {
                        Some((_, cpath, cparams)) => {
                            if let Err(err) = add_dyna_part(store, &p, &iso, scale, m, &cpath, cparams, e) {
                                m.notes.push(format!("{path} entity {i}: moving part {p} failed ({err}); baked at rest"));
                                if let Err(e2) = add_dyna_object_file(store, &p, &iso, scale, m) {
                                    m.notes.push(format!("{path} entity {i}: external {p} failed: {e2}"));
                                }
                            }
                        }
                        // a self-animating mesh (the flag cloth's vertex tween):
                        // a dyna entity of its own, no constraint
                        None if tween_parts_enabled() && dyna_has_tween_material(store, &p) => {
                            if let Err(err) = add_dyna_tween_part(store, &p, &iso, scale, m, e) {
                                m.notes.push(format!("{path} entity {i}: tween part {p} failed ({err}); baked at rest"));
                                if let Err(e2) = add_dyna_object_file(store, &p, &iso, scale, m) {
                                    m.notes.push(format!("{path} entity {i}: external {p} failed: {e2}"));
                                }
                            }
                        }
                        _ => {
                            if let Err(e2) = add_dyna_object_file(store, &p, &iso, scale, m) {
                                m.notes.push(format!("{path} entity {i}: external {p} failed: {e2}"));
                            }
                        }
                    }
                }
                // the constraint entities were taken above, with the part they move
                Some(p) if p.to_ascii_lowercase().ends_with(".kinematicconstraint.gbx") => {}
                Some(p) if p.to_ascii_lowercase().ends_with(".vegettreemodel.gbx") => {
                    m.veget.push((p.clone(), iso));
                    m.notes.push(format!("{path} entity {i}: external {p} skipped (vegetation, re-emitted as an item)"));
                }
                // An effect system (the Show items' smoke / sparks): parsed with
                // its particle models and inlined by `assemble` as an entity of
                // the prefab form — but LEFT OUT by default, because THE GAME
                // DROPS AN EMBEDDED ITEM WHOSE FX ENTITY HAS A LIVE EMITTER.
                //
                // Proved in the editor on 2026-09-08 (`shootctl mapstate --get
                // /mapitems` over a lineup on Summer 15, one item per map):
                //   kept    - the same item without the FX entity (control)
                //   kept    - an FxSystem entity whose root has NO emitters
                //   dropped - an emitter with an inline particle model (whole,
                //             and cut to four chunks); with the model as the
                //             pack's own `.ParticleModel.Gbx` by path; and with
                //             the FxSystem's ContextClassId set to the value the
                //             two context-carrying pack FX systems use
                //             (0x2F0DD000)
                // A dropped item is worse than no FX: the fogger / sparkler /
                // torch geometry disappears from the map (map 11 carries 183
                // torches) and the game says nothing - no dialog, nothing in
                // UGCErrorsLog. `TINY_FX=1` puts them back for further work.
                Some(p) if p.to_ascii_lowercase().ends_with(".fxsys.gbx") => {
                    if !std::env::var("TINY_FX").map(|v| v == "1" || v == "on").unwrap_or(false) {
                        m.notes.push(format!("{path} entity {i}: external {p} left out (the game drops an item whose FX entity has an emitter; TINY_FX=1 to include)"));
                    } else {
                        match add_fx_system(store, &p, &iso) {
                            Ok(part) => {
                                for (_, _, name, bytes) in &part.textures {
                                    if !m.pictures.iter().any(|(n, _)| n == name) {
                                        m.pictures.push((name.clone(), bytes.clone()));
                                    }
                                }
                                m.notes.push(format!("{path} entity {i}: effect system {p}: {} emitter(s), {} particle model(s)", part.fx.root.emitters().len(), part.models.len()));
                                m.fx.push(part);
                            }
                            Err(e) => m.notes.push(format!("{path} entity {i}: external {p} failed: {e}")),
                        }
                    }
                }
                Some(p) => m.notes.push(format!("{path} entity {i}: external {p} skipped")),
                None => m.notes.push(format!("{path} entity {i}: external node {} unnamed", e.model.index)),
            },
        }
    }
    Ok(())
}

/// An effect system entity of a prefab (`Fogger16M.FxSys.Gbx`): the
/// `CPlugFxSystem` parsed, and every `.ParticleModel.Gbx` its emitters name
/// loaded and parsed too (their externals — the smoke texture — kept as
/// paths). Nothing is scaled here; `assemble` poses and scales it.
/// TINY_FX_KEEP=02D,036,… — keep only these sub-model / particle chunks (by their
/// low 12 bits, hex) when inlining a particle model: the bisect of which raw-copied
/// chunk payload the engine misreads inside an item body (2026-09-08).
fn fx_keep_filter() -> Option<Vec<u32>> {
    let v = std::env::var("TINY_FX_KEEP").ok()?;
    Some(v.split(',').filter_map(|s| u32::from_str_radix(s.trim().trim_start_matches("0x"), 16).ok()).collect())
}

pub fn add_fx_system(store: &mut crate::store::DataStore, path: &str, at: &Xform) -> R<FxPart> {
    let model = store.load_model(path)?;
    if model.class_id != super::particle::C_FX_SYSTEM {
        return Err(format!("{path}: class 0x{:08X} is not CPlugFxSystem", model.class_id));
    }
    let mut lb = super::LookbackState::default();
    lb.defined_nodes.extend(model.external_indices().iter().copied());
    let mut r = super::Rd::new(&model.body, 0, lb);
    let fx = super::particle::CPlugFxSystem::parse(&mut r).map_err(|e| format!("{path}: {e}"))?;
    let mut models = Vec::new();
    for e in fx.root.emitters() {
        if e.model.inline.is_some() || e.model.index < 0 {
            continue;
        }
        let idx = e.model.index as u32;
        if models.iter().any(|(k, _, _, _)| *k == idx) {
            continue;
        }
        let mp = model.externals.iter().find(|(k, _)| *k == idx).map(|(_, p)| p.clone()).ok_or_else(|| format!("{path}: emitter {:?} names model node {idx}, which is neither inline nor external", e.name.as_str().unwrap_or("")))?;
        let pm = store.load_model(&mp)?;
        if pm.class_id != super::particle::C_PARTICLE_EMITTER_MODEL {
            return Err(format!("{mp}: class 0x{:08X} is not CPlugParticleEmitterModel", pm.class_id));
        }
        let mut lb = super::LookbackState::default();
        lb.defined_nodes.extend(pm.external_indices().iter().copied());
        let mut r = super::Rd::new(&pm.body, 0, lb);
        let node = super::particle::ParticleNode::parse(&mut r, pm.class_id).map_err(|e| format!("{mp}: {e}"))?;
        if r.o != pm.body.len() {
            return Err(format!("{mp}: {} trailing bytes after the particle model", pm.body.len() - r.o));
        }
        models.push((idx, mp, node, pm.externals.clone()));
    }
    // the textures, for the in-archive form: each `.Texture.gbx` the models
    // name, parsed, and its image file read out of the pack
    let mut textures: Vec<(String, super::particle::ParticleNode, String, Vec<u8>)> = Vec::new();
    let tex_mode = std::env::var("TINY_FX_TEXTURE").unwrap_or_else(|_| "extern".into());
    if tex_mode == "archive" || tex_mode == "file" || tex_mode.starts_with("path:") {
        for (_, mp, node, ext) in &models {
            for tref in node_texture_refs(node) {
                let Some(tp) = ext.iter().find(|(k, _)| *k as i32 == tref).map(|(_, p)| p.clone()) else { continue };
                if textures.iter().any(|(p, _, _, _)| *p == tp) {
                    continue;
                }
                let tm = store.load_model(&tp).map_err(|e| format!("{mp}: texture {tp}: {e}"))?;
                if tm.class_id != super::particle::C_BITMAP {
                    return Err(format!("{tp}: class 0x{:08X} is not CPlugBitmap", tm.class_id));
                }
                let mut lb = super::LookbackState::default();
                lb.defined_nodes.extend(tm.external_indices().iter().copied());
                let mut r = super::Rd::new(&tm.body, 0, lb);
                let bitmap = super::particle::ParticleNode::parse(&mut r, tm.class_id).map_err(|e| format!("{tp}: {e}"))?;
                let image_idx = bitmap.chunks.iter().find_map(|c| match c {
                    super::particle::PChunk::BitmapImage { image, .. } => Some(image.index),
                    _ => None,
                }).ok_or_else(|| format!("{tp}: no image chunk (0x09011030)"))?;
                let ip = tm.externals.iter().find(|(k, _)| *k as i32 == image_idx).map(|(_, p)| p.clone()).ok_or_else(|| format!("{tp}: image node {image_idx} is not an external file"))?;
                let bytes = store.read(&ip).map_err(|e| format!("{ip}: {e}"))?;
                let name = ip.rsplit('\\').next().unwrap_or(&ip).to_string();
                if tex_mode.starts_with("path:") {
                    // the image alone rides in the archive (bare name), for a `{dds}` spelling
                    textures.push((tp.clone(), bitmap.clone(), name.clone(), bytes.clone()));
                    continue;
                }
                if tex_mode == "file" {
                    // the `.Texture.gbx` as a FILE next to the item — REWRITTEN for a
                    // user file (legacy chunks stripped, `particle::texture_file`) with
                    // its image under `Image/`; the sub-model names it by bare name
                    let tname = tp.rsplit('\\').next().unwrap_or(&tp).to_string();
                    let tbytes = super::particle::texture_file(&bitmap, &name);
                    textures.push((tp.clone(), bitmap.clone(), tname, tbytes));
                    textures.push((format!("{tp}#image"), bitmap, format!("Image/{name}"), bytes));
                    continue;
                }
                textures.push((tp, bitmap, name, bytes));
            }
        }
    }
    Ok(FxPart { path: path.to_string(), fx, models, textures, at: *at })
}

/// Give every inline node of a particle chain its index in the item's node
/// space (depth first, in write order) and settle its external references:
/// the texture the sub-model names is kept as an external reference to the
/// pack path (`TINY_FX_TEXTURE=extern`, default — the game resolves it or
/// not; the probe of 2026-09-08) or dropped (`null`).
/// The external node indices a particle model's sub-models name as their
/// texture (chunk 0x090B2036).
fn node_texture_refs(node: &super::particle::ParticleNode) -> Vec<i32> {
    let mut out = Vec::new();
    for c in &node.chunks {
        match c {
            super::particle::PChunk::Texture { texture, .. } if texture.inline.is_none() && texture.index >= 0 => out.push(texture.index),
            super::particle::PChunk::SubModels { models, .. } => {
                for m in models {
                    if let Some(Node::Particle(inner)) = m.inline.as_deref() {
                        out.extend(node_texture_refs(inner));
                    }
                }
            }
            _ => {}
        }
    }
    out
}

// (see fx_keep_filter)
fn place_particle_node(node: &mut super::particle::ParticleNode, externals: &[(u32, String)], textures: &[(String, super::particle::ParticleNode, String, Vec<u8>)], next: &mut i32) {
    let texture_mode = std::env::var("TINY_FX_TEXTURE").unwrap_or_else(|_| "extern".into());
    if let Some(keep) = fx_keep_filter() {
        if node.class_id == 0x090B2000 {
            node.chunks.retain(|c| {
                let id = super::particle::chunk_id(c);
                keep.contains(&(id & 0xFFF))
            });
        }
    }
    for r in node.refs_mut() {
        if r.index < 0 {
            continue;
        }
        match r.inline.as_deref_mut() {
            Some(Node::Particle(inner)) => {
                r.index = next_index(next);
                place_particle_node(inner, externals, textures, next);
            }
            Some(_) => {
                r.index = next_index(next);
            }
            None => {
                // an external of the source file (the texture, or a bitmap's image)
                let path = externals.iter().find(|(k, _)| *k as i32 == r.index).map(|(_, p)| p.clone());
                match (path, texture_mode.as_str()) {
                    (Some(p), "extern") => {
                        let i = next_index(next);
                        EXTERNALS.with(|e| e.borrow_mut().push((i as u32, p)));
                        r.index = i;
                    }
                    // TINY_FX_TEXTURE=path:SPELLING — the reference-table entry spelled
                    // as given (the probe of what an embedded item's table can reach:
                    // the chunk-036 word is a plain u32 that names a reference-table
                    // entry, 2026-09-08). `{dds}` in the spelling = the image's bare
                    // file name (the DDS rides in the archive as `Items/<name>`).
                    (Some(p), m) if m.starts_with("path:") => {
                        let dds = p.rsplit('\\').next().unwrap_or(&p).replace(".Texture.gbx", ".dds");
                        let spelled = m["path:".len()..].replace("{dds}", &dds);
                        let i = next_index(next);
                        EXTERNALS.with(|e| e.borrow_mut().push((i as u32, spelled)));
                        r.index = i;
                    }
                    (Some(p), "file") => match textures.iter().find(|(tp, _, _, _)| *tp == p) {
                        Some((_, _, tname, _)) => {
                            let i = next_index(next);
                            EXTERNALS.with(|e| e.borrow_mut().push((i as u32, tname.clone())));
                            r.index = i;
                        }
                        None => {
                            let i = next_index(next);
                            EXTERNALS.with(|e| e.borrow_mut().push((i as u32, p)));
                            r.index = i;
                        }
                    },
                    (Some(p), "archive") => match textures.iter().find(|(tp, _, _, _)| *tp == p) {
                        // the `.Texture.gbx` inline; its image named by its bare file
                        // name (folder 0 = the item's own folder in the archive)
                        Some((_, bitmap, name, _)) => {
                            let i = next_index(next);
                            let mut b = bitmap.clone();
                            // only the chunks THIS exe's CPlugBitmap reader knows in a
                            // user file: its switch (exe+0x3f78eb) handles 0x2B-0x2E, 0x30,
                            // 0x32-0x38 and up; the pack files' legacy 0x19/0x20/0x23/0x25/
                            // 0x28/0x2A come through a pak-side descriptor table, and read
                            // from user space they misparse (ReadString on chunk id
                            // 0x09011023 → "class 0x40000000" → crash, 2026-09-08)
                            b.chunks.retain(|c| !super::particle::is_legacy_bitmap_chunk(c));
                            for ir in b.refs_mut() {
                                if ir.index >= 0 && ir.inline.is_none() {
                                    let k = next_index(next);
                                    EXTERNALS.with(|e| e.borrow_mut().push((k as u32, name.clone())));
                                    ir.index = k;
                                }
                            }
                            *r = inline(i, Node::Particle(b));
                        }
                        // not a texture we carried (a bitmap's own image reached
                        // through the recursion is handled above): the pack path
                        None => {
                            let i = next_index(next);
                            EXTERNALS.with(|e| e.borrow_mut().push((i as u32, p)));
                            r.index = i;
                        }
                    },
                    _ => *r = super::null_ref(),
                }
            }
        }
    }
}

/// The effect systems as prefab entities, laid out like the pack's Show
/// prefabs (entity 1 of `Fogger16M.Prefab.Gbx` = the FxSys at its offset):
/// each `CPlugFxSystem` inlined with its particle models inlined under its
/// emitters, the entity pose scaled. Knobs (probes, 2026-09-08):
/// `TINY_FX_FORM=extern` keeps the pack `.FxSys.Gbx` as an EXTERNAL entity
/// model instead (route a of the feasibility test); `TINY_FX_EXPR_K=text`
/// overrides expression K (1..12) of every emitter; the item scale goes into
/// ScaleExpr (string 5) — `TINY_FX_SCALE_EXPR=K` moves it, 0 leaves it out.
pub fn fx_entities(m: &Merged, scale: f32, next: &mut i32) -> Vec<super::prefab::Entity> {
    let form = std::env::var("TINY_FX_FORM").unwrap_or_else(|_| "inline".into());
    let scale_expr: Option<usize> = std::env::var("TINY_FX_SCALE_EXPR").ok().and_then(|v| v.parse().ok());
    let mut ents = Vec::new();
    for part in &m.fx {
        let rot = crate::geom::to_quat(&part.at);
        let pos = [part.at[9] * scale, part.at[10] * scale, part.at[11] * scale];
        let model: Ref = if form == "extern" {
            let i = next_index(next);
            EXTERNALS.with(|e| e.borrow_mut().push((i as u32, part.path.clone())));
            super::NodeRef { index: i, inline: None }
        } else {
            let mut fx = part.fx.clone();
            let fx_index = next_index(next);
            // probes (2026-09-08, the game DROPS an item with an FX entity):
            // TINY_FX_EMPTY=1 writes the FxSystem with an empty root (the class
            // alone), TINY_FX_CONTEXT=hex sets ContextClassId, TINY_FX_NOMODEL=1
            // nulls the emitters' particle-model refs
            if std::env::var("TINY_FX_EMPTY").map(|v| v == "1").unwrap_or(false) {
                let name = match &fx.root {
                    super::particle::FxNode::Parallel { name, .. } => name.clone(),
                    _ => super::Id::Str("Fx".into()),
                };
                fx.root = super::particle::FxNode::Parallel { name, children: Vec::new() };
            }
            if let Some(c) = std::env::var("TINY_FX_CONTEXT").ok().and_then(|v| u32::from_str_radix(v.trim_start_matches("0x"), 16).ok()) {
                fx.context_class_id = c as i32;
            }
            let no_model = std::env::var("TINY_FX_NOMODEL").map(|v| v == "1").unwrap_or(false);
            // the particle models: one inline copy per emitter that names it
            // the first time, a back reference afterwards
            let mut placed: Vec<(u32, i32)> = Vec::new();
            for e in fx.root.emitters_mut() {
                if no_model {
                    e.model = super::null_ref();
                }
                if e.model.index < 0 || e.model.inline.is_some() {
                    continue;
                }
                let src = e.model.index as u32;
                if let Some((_, i)) = placed.iter().find(|(k, _)| *k == src) {
                    e.model = super::NodeRef { index: *i, inline: None };
                    continue;
                }
                // TINY_FX_MODEL=extern: the emitter names the PACK's own
                // `.ParticleModel.Gbx` by path instead of carrying an inline copy
                // (the model — and with it the texture — stays the game's; only the
                // emitter expressions, where our 0.5 ScaleExpr lives, are ours)
                let model_extern = std::env::var("TINY_FX_MODEL").map(|v| v == "extern").unwrap_or(false);
                if model_extern {
                    if let Some((_, mp, _, _)) = part.models.iter().find(|(k, _, _, _)| *k == src) {
                        let i = next_index(next);
                        EXTERNALS.with(|e| e.borrow_mut().push((i as u32, mp.clone())));
                        e.model = super::NodeRef { index: i, inline: None };
                        continue;
                    }
                }
                match part.models.iter().find(|(k, _, _, _)| *k == src) {
                    Some((_, _, node, ext)) => {
                        let i = next_index(next);
                        let mut node = node.clone();
                        place_particle_node(&mut node, ext, &part.textures, next);
                        e.model = inline(i, Node::Particle(node));
                        placed.push((src, i));
                    }
                    None => e.model = super::null_ref(),
                }
                // the emitter's literal offsets follow the item scale: LocalOffsetExpr
                // (string 1: SparklerEnd16m sits `float3(0,0,10)` up the stick, 5 m
                // on the half-size stick) and WorldOffsetExpr (string 2)
                for s in e.exprs.iter_mut().take(2) {
                    if let Some(scaled) = scale_float3_literal(s, scale) {
                        *s = scaled;
                    }
                }
                for k in 1..=12usize {
                    if let Ok(v) = std::env::var(format!("TINY_FX_EXPR_{k}")) {
                        if k <= 10 {
                            e.exprs[k - 1] = v;
                        } else {
                            e.tail[k - 11] = v;
                        }
                    }
                }
                // ScaleExpr is string 5: the archive order read off the exe's
                // CPlugFxSystemNode_ParticleEmitter serialiser (exe+0x62098c: strings
                // into +0x30 LocalOffset, +0x40 WorldOffset, +0x78 LinearVelInW,
                // +0x88 SpawnFreqModifier, +0x98 Scale, +0xb8 LAmbient, +0x58 Up,
                // +0x68 DOV, +0xa8 Opacity, +0xc8 WaterTop, u32 DOVAndUpAreLocalSpace,
                // +0xd8 LinearHue01, +0xe8 HueLightness — consistent with every known
                // slot: the cos() pulse at 4, Up/DOV at 7/8, hue/lightness at 11/12).
                // The item scale goes there (TINY_FX_SCALE_EXPR=K overrides the slot,
                // 0 = leave the emitter unscaled).
                let k = scale_expr.unwrap_or(5);
                if k >= 1 && (scale - 1.0).abs() > 1e-6 {
                    let cur = if k <= 10 { e.exprs[k - 1].clone() } else { e.tail[(k - 11).min(1)].clone() };
                    let v = if cur.trim() == "1" { format!("{scale}") } else { format!("({cur})*{scale}") };
                    if (1..=10).contains(&k) {
                        e.exprs[k - 1] = v;
                    } else if k == 11 || k == 12 {
                        e.tail[k - 11] = v;
                    }
                }
            }
            inline(fx_index, Node::FxSystem(fx))
        };
        ents.push(super::prefab::Entity { model, rot, pos, params_id: -1, params: Vec::new(), u01: Vec::new() });
    }
    ents
}

/// Every `CPlugStaticObjectModel` entity of `prefab` (recursively), merged
/// into one static item.
pub fn static_item_from_prefab(store: &mut crate::store::DataStore, prefab: &str, ident: &str, author: &str, scale: f32, collection: u32) -> R<Vec<u8>> {
    let (bytes, _) = static_item_from_prefab_report(store, prefab, ident, author, scale, collection)?;
    Ok(bytes)
}

/// Same, also returning the merge notes.
pub fn static_item_from_prefab_report(store: &mut crate::store::DataStore, prefab: &str, ident: &str, author: &str, scale: f32, collection: u32) -> R<(Vec<u8>, Merged)> {
    let mut m = Merged::default();
    add_prefab(store, prefab, &IDENTITY, scale, &mut m, 0)?;
    let opts = BuildOpts { ident: ident.to_string(), author: author.to_string(), scale, collection, skin: m.skin.clone() };
    let f = assemble(&m, &opts)?;
    Ok((super::write_file(&f), m))
}

/// A Nadeo (or any) item that is already a static object, or a crystal item
/// (`CGameCommonItemEntityModelEdition` -> `CPlugCrystal`) baked to visuals
/// + collision, rewritten as a static item.
pub fn static_item_from_item(item_bytes: &[u8], ident: &str, author: &str, scale: f32) -> R<Vec<u8>> {
    let (bytes, _) = static_item_from_item_report(item_bytes, ident, author, scale, 26)?;
    Ok(bytes)
}

pub fn static_item_from_item_report(item_bytes: &[u8], ident: &str, author: &str, scale: f32, collection: u32) -> R<(Vec<u8>, Merged)> {
    let mut m = Merged::default();
    // the source item's skin declaration (header chunk 0x090F4000) travels
    let skin = tmmaps::header::game_skin_chunk(item_bytes);
    match super::parse_file(item_bytes) {
        Ok(f) => {
            let so = f.item.static_object().ok_or("item has no CPlugStaticObjectModel (and is not a crystal item)")?;
            let mut resolve = |idx: i32| -> Option<(String, String, u8)> {
                m.notes.push(format!("external material node {idx} in a standalone item"));
                None
            };
            let mut m2 = Merged::default();
            m2.add_static_object(so, &IDENTITY, scale, &mut resolve)?;
            // waypoint type from the item chunk (3 = none), trigger + spawn iso
            // from the entity model, all scaled
            let wt = f.item.chunks.iter().find_map(|c| match c {
                super::item::ItemChunk::Waypoint { waypoint_type, .. } => Some(*waypoint_type),
                _ => None,
            });
            if let Some(t) = wt.filter(|t| *t != 3) {
                m2.waypoint_type = Some(t);
            }
            if let Some(ent) = f.item.model().and_then(|mc| mc.entity_model()) {
                if let Some(Node::Surface(t)) = ent.trigger_shape.inline.as_deref() {
                    let mut t = t.clone();
                    if let super::surface::Surf::Mesh { vertices, .. } = &mut t.surf {
                        for v in vertices.iter_mut() {
                            *v = [v[0] * scale, v[1] * scale, v[2] * scale];
                        }
                    }
                    m2.trigger = Some(t);
                }
                m2.spawn = [ent.iso[9] * scale, ent.iso[10] * scale, ent.iso[11] * scale];
            }
            m2.notes.extend(m.notes.drain(..));
            m = m2;
        }
        Err(e) => {
            // Not a static item: try the crystal path.
            let g = tmmaps::gbx::Gbx::parse(item_bytes);
            let loc = crate::crystal_model::locate(&g.body).map_err(|e2| format!("not a static item ({e}); not a crystal item ({e2})"))?;
            let (crystal, _, _) = crate::crystal_model::CPlugCrystal::parse_with(&g.body, loc.at, loc.lookback.clone())?;
            super::bake::add_crystal(&crystal, scale, &mut m)?;
        }
    }
    m.skin = skin;
    let opts = BuildOpts { ident: ident.to_string(), author: author.to_string(), scale, collection, skin: m.skin.clone() };
    let f = assemble(&m, &opts)?;
    Ok((super::write_file(&f), m))
}


/// A `.StaticObject.Gbx` pack file (a bare `CPlugStaticObjectModel` body):
/// parse it with the typed reader, pull its EXTERNAL mesh (`.Mesh.Gbx`, a
/// `CPlugSolid2Model` file) and shape (`.HitShape.Gbx`/`.Shape.Gbx`, a
/// `CPlugSurface` file) inline, and merge it placed by `at`. Material refs
/// of the three files live in three index spaces; they are offset here
/// (mesh +100000, shape +200000) so one resolver serves all of them.
pub fn add_static_object_file(store: &mut crate::store::DataStore, path: &str, at: &Xform, scale: f32, m: &mut Merged) -> R<()> {
    const MESH_OFF: i32 = 100_000;
    const SHAPE_OFF: i32 = 200_000;
    let model = store.load_model(path)?;
    if model.class_id != super::C_STATIC_OBJECT_MODEL {
        return Err(format!("{path}: class 0x{:08X} is not CPlugStaticObjectModel", model.class_id));
    }
    let mut lb = super::LookbackState::default();
    lb.defined_nodes.extend(model.external_indices().iter().copied());
    let mut r = super::Rd::new(&model.body, 0, lb);
    let mut so = super::item::CPlugStaticObjectModel::parse(&mut r).map_err(|e| format!("{path}: {e}"))?;
    let so_ext = model.externals.clone();
    let mut mesh_ext: Vec<(u32, String)> = Vec::new();
    let mut shape_ext: Vec<(u32, String)> = Vec::new();
    let name_in = |tbl: &[(u32, String)], i: i32| tbl.iter().find(|(k, _)| *k as i32 == i).map(|(_, p)| p.clone());
    if so.mesh.inline.is_none() && so.mesh.index >= 0 {
        let mp = name_in(&so_ext, so.mesh.index).ok_or_else(|| format!("{path}: mesh node {} is neither inline nor external", so.mesh.index))?;
        let mm = store.load_model(&mp)?;
        let mut lb = super::LookbackState::default();
        lb.defined_nodes.extend(mm.external_indices().iter().copied());
        let mut r = super::Rd::new(&mm.body, 0, lb);
        let mut s2 = super::solid2::CPlugSolid2Model::parse(&mut r).map_err(|e| format!("{mp}: {e}"))?;
        for mr in s2.materials.iter_mut() {
            if mr.inline.is_none() && mr.index >= 0 {
                mr.index += MESH_OFF;
            }
        }
        for cm in s2.custom_materials.iter_mut() {
            if let Some(nr) = cm.node.as_mut() {
                if nr.inline.is_none() && nr.index >= 0 {
                    nr.index += MESH_OFF;
                }
            }
        }
        // the light sockets' `.Light.Gbx` refs live in the mesh's table too
        for l in s2.lights.iter_mut() {
            if l.u02 && l.node.inline.is_none() && l.node.index >= 0 {
                l.node.index += MESH_OFF;
            }
        }
        mesh_ext = mm.externals.clone();
        so.mesh.inline = Some(Box::new(Node::Solid2(s2)));
    }
    if !so.is_mesh_collidable && so.shape.inline.is_none() && so.shape.index >= 0 {
        let sp = name_in(&so_ext, so.shape.index).ok_or_else(|| format!("{path}: shape node {} is neither inline nor external", so.shape.index))?;
        let sm = store.load_model(&sp)?;
        let mut lb = super::LookbackState::default();
        lb.defined_nodes.extend(sm.external_indices().iter().copied());
        let mut r = super::Rd::new(&sm.body, 0, lb);
        let mut sf = super::surface::CPlugSurface::parse(&mut r).map_err(|e| format!("{sp}: {e}"))?;
        for sm_ in sf.materials.iter_mut() {
            if let super::surface::SurfMaterial::Node(nr) = sm_ {
                if nr.inline.is_none() && nr.index >= 0 {
                    nr.index += SHAPE_OFF;
                }
            }
        }
        shape_ext = sm.externals.clone();
        so.shape.inline = Some(Box::new(Node::Surface(sf)));
    }
    let common = most_common_physics(&so);
    let mut resolve = |idx: i32| -> Option<(String, String, u8)> {
        let p = if idx >= SHAPE_OFF {
            name_in(&shape_ext, idx - SHAPE_OFF)?
        } else if idx >= MESH_OFF {
            name_in(&mesh_ext, idx - MESH_OFF)?
        } else {
            name_in(&so_ext, idx)?
        };
        let link = material_link(&p);
        let phys = physics_for_link(&link).or_else(|| material_physics(store, &p).filter(|x| *x != 0)).or(common).unwrap_or(0);
        Some((p, link, phys))
    };
    m.add_static_object(&so, at, scale, &mut resolve).map_err(|err| format!("{path}: {err}"))?;
    m.resolve_pending_lights(store);
    Ok(())
}


/// A `.DynaObject.Gbx` pack file loaded for either use: baked at rest
/// (`add_dyna_object_file`) or kept as a moving part (`add_dyna_part`). The
/// mesh is an external `.Mesh.Gbx` (`CPlugSolid2Model`) whose visuals carry
/// their vertices INLINE (converted to streams on parse); a vertex-animated
/// mesh stacks its frames in that one vertex array under a single index list
/// (Flag: 12384 = 43 x 288 vertices, 726 indices) — the first frame, the
/// vertices the indices reach, is what stays.
pub struct DynaSource {
    pub model: super::dyna::CPlugDynaObjectModel,
    /// The dyna file's own reference table (mesh, MoveShape, HitShape).
    pub externals: Vec<(u32, String)>,
    pub mesh_path: String,
    pub s2: super::solid2::CPlugSolid2Model,
    pub mesh_ext: Vec<(u32, String)>,
    /// Per Solid2 material: dressed by a vertex-tween shader (the flag cloth).
    pub tween_mats: Vec<bool>,
}

/// The material a STILL cloth is drawn with in place of its vertex-tween one
/// (a static visual under a tween material reads a frame table it does not
/// have: the 0x140a9c174 crash). `TINY_FLAG_STILL_MAT`:
///   `noanim` (default): the pack's own `ItemFlagNoAnim` — the flag texture,
///     hue mask and roughness under the plain `Tech3_Block_TDSN_CubeOut`
///     shader, what `FlagSmall.Mesh.Gbx` itself uses for its farthest level
///     (a 4-vertex quad). The cloth keeps its own uv0: green with the logo,
///     exactly the stock flag's look at rest (2026-09-08);
///   `trackborders`: the 2026-09-07 form — TrackBorders with uv0 remapped into
///     its hue-masked stripe (a plain cloth in the placement colour; white).
fn still_cloth_material(tween: &str) -> (String, String) {
    match std::env::var("TINY_FLAG_STILL_MAT").as_deref() {
        Ok("trackborders") => (
            "Stadium\\Media\\Material\\TrackBorders".to_string(),
            format!("{tween}: vertex-tween shader; drawn as TrackBorders (uv0 in the hue-masked stripe band)"),
        ),
        _ => (
            "Stadium\\Media\\Material\\ItemFlagNoAnim".to_string(),
            format!("{tween}: vertex-tween shader; drawn still under ItemFlagNoAnim (the pack's own non-animated flag material, own uv0)"),
        ),
    }
}

/// Whether the still cloth keeps its own uv0 (ItemFlagNoAnim) or takes the
/// TrackBorders stripe band.
fn still_cloth_keeps_uv() -> bool {
    std::env::var("TINY_FLAG_STILL_MAT").as_deref() != Ok("trackborders")
}

/// A material whose shader tweens between vertex frames
/// (`Tech3_Warp_TDiffSpec_VertexTween`, the flag cloth's `ItemFlag`).
fn is_tween_material(store: &mut crate::store::DataStore, p: &str) -> bool {
    store.load_model(p).map(|mm| mm.externals.iter().any(|(_, e)| e.to_ascii_lowercase().contains("tween"))).unwrap_or(false)
}

/// A vertex-tweened cloth (the flag) kept as a dyna entity of its own with
/// its frames, frame table, tween material and the pack's inline-vertex form
/// (`TINY_FLAG_TWEEN=1`, an experiment knob). OFF by default, and not because
/// of a count: measured on 2026-09-08 (lineups on a flag-free tiny host), an
/// embedded tween cloth is drawn ONLY while a stock Flag item is loaded in
/// the map and within ~100 m — no stock, or one 300 m away, and the cloths
/// are absent or garbage (crumpled shards, giant sails) — and even with a
/// stock flag beside them, identical placements at DIFFERENT detail levels
/// draw as garbage (39 in one row were fine; the same 39 spread over map 10
/// were not; a single-level cloth, `TINY_FLAG_LODS=1`, fixed that lineup).
/// The engine's frame table for the tween draw evidently comes from the
/// stock visual, not ours. Production therefore uses no embedded tween at
/// all: `Flag16m` placements become the stock `Flag8m` (the game's own
/// half-size flag, `stock_half_variant`) and `Flag8m` placements a still
/// cloth under the pack's `ItemFlagNoAnim` material (`add_dyna_object_file`).
pub fn tween_parts_enabled() -> bool {
    std::env::var("TINY_FLAG_TWEEN").map(|v| v == "1").unwrap_or(false)
}

fn name_in(tbl: &[(u32, String)], i: i32) -> Option<String> {
    tbl.iter().find(|(k, _)| *k as i32 == i).map(|(_, p)| p.clone())
}

/// `keep_frames`: leave a tweened visual whole (every frame, the frame table,
/// the tween material) for a dyna entity; false bakes frame 0 as a static
/// visual under TrackBorders (a static object with the table crashes at draw).
pub fn load_dyna_source(store: &mut crate::store::DataStore, path: &str, m: &mut Merged, keep_frames: bool) -> R<DynaSource> {
    let model = store.load_model(path)?;
    if model.class_id != crate::node::C_DYNA_OBJECT {
        return Err(format!("{path}: class 0x{:08X} is not CPlugDynaObjectModel", model.class_id));
    }
    let mut lb = super::LookbackState::default();
    lb.defined_nodes.extend(model.external_indices().iter().copied());
    let mut r = super::Rd::new(&model.body, 0, lb);
    let dyna = super::dyna::CPlugDynaObjectModel::parse(&mut r).map_err(|e| format!("{path}: {e}"))?;
    if r.o != model.body.len() {
        return Err(format!("{path}: {} trailing bytes after the dyna object", model.body.len() - r.o));
    }
    let mp = match dyna.mesh.inline.as_deref() {
        Some(_) => return Err(format!("{path}: inline dyna mesh is not handled")),
        None => name_in(&model.externals, dyna.mesh.index).ok_or_else(|| format!("{path}: mesh node {} is neither inline nor external", dyna.mesh.index))?,
    };
    let mm_ = store.load_model(&mp)?;
    let mut lb = super::LookbackState::default();
    lb.defined_nodes.extend(mm_.external_indices().iter().copied());
    let mut r = super::Rd::new(&mm_.body, 0, lb);
    let mut s2 = super::solid2::CPlugSolid2Model::parse(&mut r).map_err(|e| format!("{mp}: {e}"))?;
    for vr in s2.visuals.iter_mut() {
        if let Some(Node::Visual(v)) = vr.inline.as_deref_mut() {
            let count = v.main.as_ref().map(|m| m.count).unwrap_or(0).max(0) as usize;
            let reach = v.index_buffer.as_ref().and_then(|b| b.indices.iter().max().copied()).map(|x| x as usize + 1).unwrap_or(count);
            if keep_frames && reach < count {
                m.notes.push(format!("{mp}: visual keeps its {} frames ({count} vertices, inline form, tween material)", v.sub_visuals.len()));
                continue;
            }
            if reach < count {
                m.notes.push(format!("{mp}: visual keeps frame 0 ({reach} of {count} vertices, {} sub-visual frames dropped)", v.sub_visuals.len()));
                v.truncate_vertices(reach);
                // the frame table (0x09006005: vertex base, index start, index
                // count per frame) is what makes the game run the vertex-animation
                // draw path on it — the 2026-09-06 crash in SCBufferDraw@NCharAnimSkelV
                v.sub_visuals.clear();
                if let Some(mn) = v.main.as_mut() {
                    if let Some(Node::VertexStream(s)) = mn.vertex_streams.first().and_then(|r| r.inline.as_deref()) {
                        if let Some(Elem::Float3(p)) = s.elems.first() {
                            let mut lo = [f32::MAX; 3];
                            let mut hi = [f32::MIN; 3];
                            for q in p {
                                for k in 0..3 {
                                    lo[k] = lo[k].min(q[k]);
                                    hi[k] = hi[k].max(q[k]);
                                }
                            }
                            mn.bounding_box = bbox(p);
                        }
                    }
                }
            }
        }
    }
    let mesh_ext = mm_.externals.clone();
    // STATIC bake of a tweened cloth (`keep_frames` false): a material whose
    // shader tweens between vertex frames runs the vertex-animation draw path
    // on every visual it dresses, and a static object's visual has no frame
    // table at run time — a NULL read (0x140a9c174, 2026-09-06 and -07). So the
    // still cloth is drawn as TrackBorders with its uv0 mapped into the road
    // stripe band, the one region of TrackBorders_D whose hue mask is set
    // (alpha 0xff at texture rows v 0.02..0.11; the game reads v from the
    // bottom, so uv v 0.90..0.97): a plain cloth in the placement colour.
    let tween_mats: Vec<bool> = s2.materials.iter().map(|r| r.inline.is_none() && r.index >= 0 && name_in(&mesh_ext, r.index).map(|p| is_tween_material(store, &p)).unwrap_or(false)).collect();
    if !keep_frames && tween_mats.iter().any(|t| *t) {
        let remap_uv = !still_cloth_keeps_uv();
        // TINY_FLAG_BAND=v0,v1: the TrackBorders_D band (v range) the cloth's
        // uv0 is mapped into — the 2026-09-07 hue-mask ladder (which band the
        // placement colour reaches: the mask's alpha is 0xff only at texture
        // rows v 0.02..0.11, and whether the game reads v from the top or the
        // bottom of the DDS was still to be measured)
        let band: [f32; 2] = std::env::var("TINY_FLAG_BAND")
            .ok()
            .and_then(|s| {
                let f: Vec<f32> = s.split(',').filter_map(|x| x.trim().parse().ok()).collect();
                (f.len() == 2).then(|| [f[0], f[1]])
            })
            .unwrap_or([0.90, 0.97]);
        if remap_uv && band != [0.90, 0.97] {
            m.notes.push(format!("TINY_FLAG_BAND={},{}: cloth uv0 mapped into that TrackBorders v band", band[0], band[1]));
        }
        for g in &s2.shaded_geoms {
            if !tween_mats.get(g.material_index.max(0) as usize).copied().unwrap_or(false) {
                continue;
            }
            if let Some(Node::Visual(v)) = s2.visuals.get_mut(g.visual_index as usize).and_then(|r| r.inline.as_deref_mut()) {
                if let Some(Node::VertexStream(s)) = v.main.as_mut().and_then(|mn| mn.vertex_streams.first_mut()).and_then(|r| r.inline.as_deref_mut()) {
                    for (d, e) in s.decls.iter().zip(s.elems.iter_mut()) {
                        if remap_uv && d.name() == N_TEXCOORD0 {
                            if let Elem::Float2(uv) = e {
                                // An affine map into the band (measured 2026-09-07 with three bands
                                // on a lineup: uv v 0.755..0.805 = the unmasked white panel, white at
                                // every colour; 0.90..0.97 = the masked stripe, coloured; 0.03..0.10 =
                                // dark grey), NOT a constant:
                                // a constant uv has zero screen derivatives and the
                                // shader's per-pixel tangent frame divides by them —
                                // the cloth drew pitch black (flagslow2, 2026-09-06).
                                for q in uv.iter_mut() {
                                    *q = [q[0].rem_euclid(1.0), band[0] + (band[1] - band[0]) * q[1].rem_euclid(1.0)];
                                }
                            }
                        }
                    }
                    // The cloth mesh carries a vertex colour (0xFFFFFFFF) that no
                    // TrackBorders visual in the pack has; the Techno3 shader reads
                    // it as a blend/decal weight and drew the cloth black (flagslow2,
                    // 2026-09-06). Drop it: the layout becomes the pack's own.
                    drop_element(s, N_COLOR0);
                }
            }
        }
    }
    Ok(DynaSource { model: dyna, externals: model.externals.clone(), mesh_path: mp, s2, mesh_ext, tween_mats })
}

/// One of the dyna object's hulls (`MoveShape` / `HitShape` / `.Shape.Gbx`)
/// parsed, with the shape file's own externals (its material nodes).
fn load_dyna_shape(store: &mut crate::store::DataStore, src: &DynaSource, sref: &Ref, m: &mut Merged) -> Option<(CPlugSurface, Vec<(u32, String)>, String)> {
    if sref.inline.is_some() || sref.index < 0 {
        return None;
    }
    let sp = name_in(&src.externals, sref.index)?;
    let low = sp.to_ascii_lowercase();
    if !low.ends_with(".hitshape.gbx") && !low.ends_with(".moveshape.gbx") && !low.ends_with(".shape.gbx") {
        return None;
    }
    let sm = store.load_model(&sp).ok()?;
    let mut lb = super::LookbackState::default();
    lb.defined_nodes.extend(sm.external_indices().iter().copied());
    let mut r = super::Rd::new(&sm.body, 0, lb);
    match super::surface::CPlugSurface::parse(&mut r) {
        Ok(sf) => Some((sf, sm.externals.clone(), sp)),
        Err(e) => {
            m.notes.push(format!("{sp}: {e} (no collision from this hull)"));
            None
        }
    }
}

/// A `.DynaObject.Gbx` pack file (`CPlugDynaObjectModel`, class 0x09144000:
/// the cloth of `Items\Flag\Flag16m`, a rotor, a light ray): its mesh at REST,
/// merged as a static object, with one of its hulls as collision.
pub fn add_dyna_object_file(store: &mut crate::store::DataStore, path: &str, at: &Xform, scale: f32, m: &mut Merged) -> R<()> {
    let src = load_dyna_source(store, path, m, false)?;
    let mut tween_notes: Vec<String> = Vec::new();
    // the two hulls: the one that moves with the object, the one that stays —
    // either gives the static copy a collision surface (an item whose surface
    // is EMPTY is dropped by the game: Summer 15's 14 rotors and 2 tubes were
    // placed at the right spot and never drawn until the HitShape came along)
    const SHAPE_OFF: i32 = 200_000;
    let mut shape_ext: Vec<(u32, String)> = Vec::new();
    let mut shape_node = super::null_ref();
    for sref in [&src.model.static_shape, &src.model.dyna_shape] {
        if let Some((mut sf, ext, sp)) = load_dyna_shape(store, &src, sref, m) {
            for sm_ in sf.materials.iter_mut() {
                if let super::surface::SurfMaterial::Node(nr) = sm_ {
                    if nr.inline.is_none() && nr.index >= 0 {
                        nr.index += SHAPE_OFF;
                    }
                }
            }
            shape_ext = ext;
            shape_node = inline(2, Node::Surface(sf));
            m.notes.push(format!("{}: collision from {}", path.rsplit('\\').next().unwrap_or(path), sp.rsplit('\\').next().unwrap_or(&sp)));
            break;
        }
    }
    let mesh_ext = src.mesh_ext.clone();
    let so = super::item::CPlugStaticObjectModel { version: 3, mesh: inline(1, Node::Solid2(src.s2)), is_mesh_collidable: false, shape: shape_node };
    let mut resolve = |idx: i32| -> Option<(String, String, u8)> {
        if idx >= SHAPE_OFF {
            let p = name_in(&shape_ext, idx - SHAPE_OFF)?;
            let link = material_link(&p);
            let phys = physics_for_link(&link).or_else(|| material_physics(store, &p).filter(|x| *x != 0)).unwrap_or(28);
            return Some((p, link, phys));
        }
        let p = name_in(&mesh_ext, idx)?;
        if is_tween_material(store, &p) {
            let (link, note) = still_cloth_material(&p);
            tween_notes.push(note);
            return Some((p, link, 28));
        }
        let link = material_link(&p);
        let phys = physics_for_link(&link).or_else(|| material_physics(store, &p).filter(|x| *x != 0)).unwrap_or(28);
        Some((p, link, phys))
    };
    let r = m.add_static_object(&so, at, scale, &mut resolve).map_err(|err| format!("{path}: {err}"));
    m.resolve_pending_lights(store);
    m.notes.extend(tween_notes);
    r
}

/// A hull re-emitted in the canonical form the item writer uses, scaled: a
/// mesh through `add_surface_mesh` (the pack shape kept verbatim — its
/// material nodes — made the game drop a whole item, 2026-09-06); a
/// primitive or compound (a pusher's `MoveShape` is a convex polyhedron, a
/// rotor's a compound of nine) kept as is with its lengths scaled and its
/// external material nodes dropped — the physics ids stay in `material_ids`.
fn canonical_surface(sf: &CPlugSurface, scale: f32) -> Option<CPlugSurface> {
    match &sf.surf {
        Surf::Mesh { vertices, triangles, .. } => {
            let mut tmp = Merged::default();
            tmp.add_surface_mesh(vertices, triangles, &IDENTITY, scale);
            Some(CPlugSurface::mesh(tmp.surf_vertices, tmp.surf_triangles, tmp.surf_ids, sf.gameplay_main_dir.unwrap_or([0.0, 0.0, 1.0])))
        }
        other => {
            let mut surf = other.clone();
            surf.scale(scale);
            let has_nodes = sf.materials.iter().any(|m| matches!(m, super::surface::SurfMaterial::Node(_)));
            let materials = if has_nodes { Vec::new() } else { sf.materials.clone() };
            let u05 = if has_nodes { None } else { sf.u05 };
            Some(CPlugSurface { version: 4, surf_version: 2, surf, gameplay_main_dir: sf.gameplay_main_dir.or(Some([0.0, 0.0, 1.0])), materials, u05, u01: Vec::new(), material_ids: sf.material_ids.clone(), skel: super::null_ref(), u06: Vec::new() })
        }
    }
}

/// The constraint file an item's Level modifier substitutes for a prefab's
/// own: `…\KinematicConstraints\ObstacleX.KinematicConstraint.Gbx` becomes
/// `<modifier folder>\AnimX<suffix>.KinematicConstraint.Gbx` when that file
/// exists (`ItemObstacleLevel1.GameSkin.gbx` lists exactly these pairs:
/// ObstaclePusher8m -> AnimPusher8mLevel1, ObstacleRotor -> AnimRotorLevel1,
/// ObstacleTube -> AnimTubeLevel1 …). Without a modifier, or without such a
/// file, the prefab's own constraint stands.
pub fn modified_constraint_path(store: &mut crate::store::DataStore, m: &Merged, constraint: &str) -> String {
    let Some(first) = m.modifier.first() else { return constraint.to_string() };
    let Some((folder, _)) = first.rsplit_once('\\') else { return constraint.to_string() };
    let file = constraint.rsplit('\\').next().unwrap_or(constraint);
    let Some(stem) = file.strip_suffix(".KinematicConstraint.Gbx") else { return constraint.to_string() };
    let anim = format!("Anim{}", stem.strip_prefix("Obstacle").unwrap_or(stem));
    let candidate = format!("{folder}\\{anim}{}.KinematicConstraint.Gbx", m.modifier_suffix);
    if store.load_model(&candidate).is_ok() {
        candidate
    } else {
        constraint.to_string()
    }
}

/// A `.DynaObject.Gbx` entity kept MOVING: its mesh merged on its own (the
/// object's local frame, scaled), its two hulls scaled, the constraint that
/// drives it with the translation range scaled, the entity pose and params
/// carried — everything `assemble` needs for a `CPlugDynaObjectModel` entity
/// of the item's prefab.
#[allow(clippy::too_many_arguments)]
pub fn add_dyna_part(store: &mut crate::store::DataStore, path: &str, at: &Xform, scale: f32, m: &mut Merged, constraint_path: &str, cparams: super::dyna::ConstraintParams, ent: &super::prefab::Entity) -> R<()> {
    let src = load_dyna_source(store, path, m, false)?;
    let kmodel = store.load_model(constraint_path)?;
    let mut constraint = super::dyna::KinematicConstraint::parse_body(&kmodel.body).map_err(|e| format!("{constraint_path}: {e}"))?;
    constraint.scale(scale);
    let mut mesh = Merged::default();
    mesh.keep_water = m.keep_water;
    mesh.modifier = m.modifier.clone();
    mesh.collision_redress = m.collision_redress.clone();
    mesh.modifier_suffix = m.modifier_suffix.clone();
    mesh.no_split = true;
    let mesh_ext = src.mesh_ext.clone();
    let mut tween_notes: Vec<String> = Vec::new();
    let so = super::item::CPlugStaticObjectModel { version: 3, mesh: inline(1, Node::Solid2(src.s2.clone())), is_mesh_collidable: false, shape: super::null_ref() };
    let mut resolve = |idx: i32| -> Option<(String, String, u8)> {
        let p = name_in(&mesh_ext, idx)?;
        if is_tween_material(store, &p) {
            let (link, note) = still_cloth_material(&p);
            tween_notes.push(note);
            return Some((p, link, 28));
        }
        let link = material_link(&p);
        let phys = physics_for_link(&link).or_else(|| material_physics(store, &p).filter(|x| *x != 0)).unwrap_or(28);
        Some((p, link, phys))
    };
    mesh.add_static_object(&so, &IDENTITY, scale, &mut resolve).map_err(|err| format!("{path}: {err}"))?;
    // a moving part's own lights (none in the pack so far) ride in its solid
    mesh.resolve_pending_lights(store);
    m.notes.extend(tween_notes);
    if mesh.visuals.is_empty() {
        return Err(format!("{path}: the moving mesh has no visuals"));
    }
    let move_shape = load_dyna_shape(store, &src, &src.model.dyna_shape, m).and_then(|(sf, _, _)| canonical_surface(&sf, scale));
    let hit_shape = load_dyna_shape(store, &src, &src.model.static_shape, m).and_then(|(sf, _, _)| canonical_surface(&sf, scale));
    // A moving part without a hull CRASHES THE CLIENT at map load (a null
    // DynaShape: Trackmania.exe+0xb7088c reading NULL+0x38, Mov2 2026-09-07).
    // No hull → no moving part: the error sends add_prefab down its static
    // bake, which draws the part where it stands.
    if move_shape.is_none() || hit_shape.is_none() {
        return Err(format!(
            "{}: no {} shape for the moving part — baked static instead",
            path.rsplit('\\').next().unwrap_or(path),
            if move_shape.is_none() { "move" } else { "hit" }
        ));
    }
    // the parent chain with the entity iso before calling); composing the
    // entity iso in again doubled the flag cloth's pose (y 11.37 instead of
    // 5.68, a 180-degree turn) — harmless only for a part sitting at the origin
    let iso = *at;
    let rot = crate::geom::to_quat(&iso);
    let pos = [iso[9] * scale, iso[10] * scale, iso[11] * scale];
    let hulls = |s: &Option<CPlugSurface>| match s.as_ref() {
        Some(s) => {
            let (v, t) = s.surf.counts();
            format!("type {} {v}v/{t}f", s.surf.type_id())
        }
        None => "none".to_string(),
    };
    m.notes.push(format!(
        "{}: MOVING part, {} visuals, move shape {}, hit shape {}, constraint {} [{}]",
        path.rsplit('\\').next().unwrap_or(path),
        mesh.visuals.len(),
        hulls(&move_shape),
        hulls(&hit_shape),
        constraint_path.rsplit('\\').next().unwrap_or(constraint_path),
        constraint.summary()
    ));
    m.notes.extend(mesh.notes.drain(..).map(|n| format!("  (moving part) {n}")));
    m.dyna.push(DynaPart {
        path: path.to_string(),
        rot,
        pos,
        mesh,
        move_shape,
        hit_shape,
        model: src.model.clone(),
        instance_params_id: ent.params_id,
        instance_params: ent.params.clone(),
        constraint: Some((constraint, cparams)),
        pack_ref: None,
    });
    Ok(())
}
/// Whether a dyna object's mesh is dressed by a vertex-tween material (the
/// flag cloth) — the one kind of moving part the pack drives without a
/// constraint.
pub fn dyna_has_tween_material(store: &mut crate::store::DataStore, path: &str) -> bool {
    let Ok(model) = store.load_model(path) else { return false };
    let mut lb = super::LookbackState::default();
    lb.defined_nodes.extend(model.external_indices().iter().copied());
    let mut r = super::Rd::new(&model.body, 0, lb);
    let Ok(dyna) = super::dyna::CPlugDynaObjectModel::parse(&mut r) else { return false };
    let Some(mp) = name_in(&model.externals, dyna.mesh.index) else { return false };
    let Ok(mm_) = store.load_model(&mp) else { return false };
    mm_.externals.iter().any(|(_, p)| p.to_ascii_lowercase().ends_with(".material.gbx") && is_tween_material(store, p))
}

/// A `.DynaObject.Gbx` entity whose mesh ANIMATES BY ITSELF — the flag cloth:
/// a vertex-tween material over a vertex array holding every frame, with the
/// frame table (0x09006005) saying where each one starts. Kept as a
/// `CPlugDynaObjectModel` entity of the item's prefab like the pack keeps it
/// (no hulls, no constraint, the pack's instance params), its mesh scaled
/// frame by frame and written in the pack's inline-vertex form. Baked into
/// the STATIC object instead (2026-09-06 and 2026-09-07, stream and inline
/// form alike), the client crashed at draw time reading the frame table
/// through a NULL pointer (0x140a9c174, `mov ecx,[r14+rax*4]` with r14 = 0,
/// rax = 3 x frame index) — the static-object loader rebuilds its visuals and
/// keeps no frame table, while the tween material still asks for one.
pub fn add_dyna_tween_part(store: &mut crate::store::DataStore, path: &str, at: &Xform, scale: f32, m: &mut Merged, ent: &super::prefab::Entity) -> R<()> {
    let src = load_dyna_source(store, path, m, true)?;
    if !src.tween_mats.iter().any(|t| *t) {
        return Err(format!("{path}: no vertex-tween material"));
    }
    let mut mesh = Merged::default();
    mesh.keep_water = m.keep_water;
    mesh.modifier = m.modifier.clone();
    mesh.collision_redress = m.collision_redress.clone();
    mesh.modifier_suffix = m.modifier_suffix.clone();
    mesh.no_split = true;
    mesh.all_lods = true;
    // TINY_FLAG_LADDER=pack: the cloth's detail ladder keeps the PACK distances
    // ([16, 64, 128, 512]) instead of the halved ones — the stock flag that
    // drives the tween switches level at those, and the draw is right only
    // while driver and cloth are at the same level (2026-09-08, L11/L17)
    mesh.ladder_scale = match std::env::var("TINY_FLAG_LADDER").as_deref() {
        Ok("pack") => Some(1.0),
        _ => None,
    };
    // TINY_FLAG_LODS=all|N: every pack level with the ladder (all), or the ONE
    // level N for every distance (the 2026-09-08 probe of the mixed-level draw)
    mesh.one_level = match std::env::var("TINY_FLAG_LODS").ok().as_deref() {
        None | Some("all") => None,
        Some(n) => Some(n.parse::<u32>().unwrap_or(0)),
    };
    mesh.vis_cst_type = Some(src.s2.vis_cst_type);
    // its ladder ([16, 64, 128, 512], five levels) is registered, scaled, by
    // add_static_object like every part's
    mesh.solid2_u07 = Some(src.s2.u07);
    // TINY_FLAG_U13=pack / TINY_FLAG_MATREF=ext: the 2026-09-08 probes of what
    // the pack mesh has that a re-baked copy lacks (see `Merged`)
    mesh.solid2_u13 = match std::env::var("TINY_FLAG_U13").as_deref() {
        Ok("pack") => Some(src.s2.u13),
        _ => None,
    };
    mesh.materials_external = std::env::var("TINY_FLAG_MATREF").as_deref() == Ok("ext");
    mesh.no_prelight = src.s2.pre_light_gen.is_none();
    let mesh_ext = src.mesh_ext.clone();
    let so = super::item::CPlugStaticObjectModel { version: 3, mesh: inline(1, Node::Solid2(src.s2.clone())), is_mesh_collidable: false, shape: super::null_ref() };
    let mut resolve = |idx: i32| -> Option<(String, String, u8)> {
        let p = name_in(&mesh_ext, idx)?;
        let link = material_link(&p);
        let phys = physics_for_link(&link).or_else(|| material_physics(store, &p).filter(|x| *x != 0)).unwrap_or(28);
        Some((p, link, phys))
    };
    mesh.add_static_object(&so, &IDENTITY, scale, &mut resolve).map_err(|err| format!("{path}: {err}"))?;
    mesh.resolve_pending_lights(store);
    if mesh.visuals.is_empty() {
        return Err(format!("{path}: the tween mesh has no visuals"));
    }
    let frames: Vec<String> = mesh.visuals.iter().map(|mv| format!("{} frames x {} vertices", mv.visual.sub_visuals.len(), mv.visual.main.as_ref().map(|mn| mn.count).unwrap_or(0) / mv.visual.sub_visuals.len().max(1) as i32)).collect();
    // the pack's hulls, when it has any (the flag has none: both refs null)
    let move_shape = load_dyna_shape(store, &src, &src.model.dyna_shape, m).and_then(|(sf, _, _)| canonical_surface(&sf, scale));
    let hit_shape = load_dyna_shape(store, &src, &src.model.static_shape, m).and_then(|(sf, _, _)| canonical_surface(&sf, scale));
    // `at` is already the entity's pose in the item frame (add_prefab composes
    // the parent chain with the entity iso before calling); composing the
    // entity iso in again doubled the flag cloth's pose (y 11.37 instead of
    // 5.68, a 180-degree turn) — harmless only for a part sitting at the origin
    let iso = *at;
    let rot = crate::geom::to_quat(&iso);
    let pos = [iso[9] * scale, iso[10] * scale, iso[11] * scale];
    let pack_ref = match std::env::var("TINY_FLAG_REF").as_deref() {
        Ok("dyna") => Some(super::merged::PackRef::Dyna),
        Ok("mesh") => Some(super::merged::PackRef::Mesh(src.mesh_path.clone())),
        Ok("file") => Some(super::merged::PackRef::File),
        _ => None,
    };
    // TINY_FLAG_KINEMATIC=1|constraint (2026-09-08 probe): the cloth entity as
    // a KINEMATIC dyna — SInstanceParams.IsKinematic (word 3) set, which puts
    // it in the entity kind the obstacle pistons use (0x914F000, the kind the
    // game does animate in an embedded item: lineup an4 showed our half-size
    // ObstaclePusher8mLevel1 piston travelling and its screen texture cycling)
    // instead of the visual-only dyna kind (0x914E000) the pack flag gets;
    // `constraint` also binds it with the pack's zero-range pusher constraint
    // (KinematicConstraints\ObstaclePusher8m: trans Z 0..0 m) like a real
    // kinematic part.
    let mut instance_params = ent.params.clone();
    let mut constraint = None;
    let (mut k_move_shape, mut k_hit_shape) = (None, None);
    match std::env::var("TINY_FLAG_KINEMATIC").as_deref() {
        Ok(v) if v == "1" || v == "constraint" => {
            if instance_params.len() >= 16 {
                instance_params[12..16].copy_from_slice(&1u32.to_le_bytes());
            }
            // a kinematic dyna without hulls crashes the loader (NULL DynaShape
            // at Trackmania.exe+0xb7088c — an5, 2026-09-08): the pusher piston's
            // own hulls, at 5 % (a stub the size of the pole's base)
            for (path, slot) in [("Stadium\\Media\\Dyna\\ObstaclePusher\\ObstaclePusher8mPiston.MoveShape.Gbx", &mut k_move_shape), ("Stadium\\Media\\Dyna\\ObstaclePusher\\ObstaclePusher8mPiston.HitShape.Gbx", &mut k_hit_shape)] {
                let sm = store.load_model(path)?;
                let mut lb = super::LookbackState::default();
                lb.defined_nodes.extend(sm.external_indices().iter().copied());
                let mut r = super::Rd::new(&sm.body, 0, lb);
                let sf = super::surface::CPlugSurface::parse(&mut r).map_err(|e| format!("{path}: {e}"))?;
                *slot = canonical_surface(&sf, 0.05);
            }
            if v == "constraint" {
                let cpath = "Stadium\\Media\\KinematicConstraints\\ObstaclePusher8m.KinematicConstraint.Gbx";
                let kmodel = store.load_model(cpath)?;
                let kc = super::dyna::KinematicConstraint::parse_body(&kmodel.body).map_err(|e| format!("{cpath}: {e}"))?;
                constraint = Some((kc, super::dyna::ConstraintParams { version: 0, ent1: -1, ent2: 0, pos1: [0.0; 3], pos2: [0.0; 3] }));
            }
        }
        _ => {}
    }
    m.notes.push(format!("{}: TWEEN part, {} visuals [{}], no constraint, params 0x{:X} ({} bytes)", path.rsplit('\\').next().unwrap_or(path), mesh.visuals.len(), frames.join("; "), ent.params_id, ent.params.len()));
    m.notes.extend(mesh.notes.drain(..).map(|n| format!("  (tween part) {n}")));
    let move_shape = move_shape.or(k_move_shape);
    let hit_shape = hit_shape.or(k_hit_shape);
    m.dyna.push(DynaPart { path: path.to_string(), rot, pos, mesh, move_shape, hit_shape, model: src.model.clone(), instance_params_id: ent.params_id, instance_params, constraint, pack_ref });
    Ok(())
}

/// The variant list of a pack ITEM: its geometry and vegetation externals
/// (`.Prefab.Gbx` / `.StaticObject.Gbx` / `.VegetTreeModel.Gbx`) in reference
/// order — the order the placement's variant byte indexes (Summer 11's
/// `Show` rig: 0 RigStraight2m … 4 RigStraight32m, 23 Light4Spots,
/// 28 Fogger16M; `PalmForest`: 37 species). Empty for an item whose model is
/// inline or of another kind.
pub fn pack_item_variants(store: &mut crate::store::DataStore, item_path: &str) -> R<Vec<String>> {
    let model = store.load_model(item_path)?;
    Ok(model
        .externals
        .iter()
        .filter(|(_, p)| {
            let low = p.to_ascii_lowercase();
            low.ends_with(".prefab.gbx") || low.ends_with(".staticobject.gbx") || low.ends_with(".vegettreemodel.gbx")
        })
        .map(|(_, p)| p.clone())
        .collect())
}

/// The material links of a pack item's modifier, with the name suffix they
/// carry. An item file may reference `<Env>\Media\Modifier\<X>.Gbx` (the
/// wrapper of a `GameSkin`, whose slot table has no reader): the pack keeps
/// the skin's materials in `<Env>\Media\Modifier\<F>\<stem><suffix>.Material.Gbx`
/// where X = F + suffix — `ItemObstacleLevel1` = folder `ItemObstacle` +
/// `Level1` (`ItemObstaclePusherLevel1`, `ItemObstacleLevel1`,
/// `ItemObstacleLightLevel1`, `ScreenPusherLevel1`), `ItemObstacleOff` =
/// `ItemObstacle` + `Off` (`DecalObstacleOff` only: the base materials ARE
/// the off look). The longest folder that prefixes X wins
/// (`ItemObstacleDiscontinuous` does not prefix `ItemObstacleLevel1`).
pub fn item_modifier_links(store: &mut crate::store::DataStore, item_path: &str) -> Option<(Vec<String>, String)> {
    let model = store.load_model(item_path).ok()?;
    let modifier = model.externals.iter().map(|(_, p)| p.as_str()).find(|p| {
        let low = p.to_ascii_lowercase();
        low.contains("\\media\\modifier\\") && low.ends_with(".gbx") && !low.ends_with(".material.gbx") && !low.ends_with(".kinematicconstraint.gbx") && low.matches('\\').count() == 3
    })?;
    let (dir, file) = modifier.rsplit_once('\\')?;
    // `Reset.TerrainModifier .Gbx` — the pack's own GateSpecial24mReset item
    // spells its modifier with a space before the extension
    let file = file.replace(' ', "");
    let x = file.strip_suffix(".Gbx").or_else(|| file.strip_suffix(".gbx"))?;
    // A gameplay gate's `<Kind>.TerrainModifier.Gbx` (GateSpecial24mTurbo2 →
    // `Turbo2.TerrainModifier.Gbx`) is the block-style form: folder = the
    // kind, no name suffix — its files are the kind-less gate pieces that
    // `gate_special_stem` maps the prefab's Turbo dress onto.
    let x = x.strip_suffix(".TerrainModifier").unwrap_or(x);
    let prefix = format!("{dir}\\").to_uppercase();
    // folders under the modifier dir that prefix X, longest first
    let mut folders: Vec<String> = Vec::new();
    for e in store.entries() {
        let p = e.path();
        let up = p.to_uppercase();
        if let Some(rest) = up.strip_prefix(&prefix) {
            if let Some((folder, _)) = rest.split_once('\\') {
                if x.to_uppercase().starts_with(folder) && !folders.iter().any(|f| f == folder) {
                    folders.push(folder.to_string());
                }
            }
        }
    }
    folders.sort_by_key(|f| std::cmp::Reverse(f.len()));
    let folder = folders.first()?.clone();
    let suffix = x[folder.len()..].to_string();
    let folder_prefix = format!("{prefix}{folder}\\");
    let mut links: Vec<String> = Vec::new();
    for e in store.entries() {
        let p = e.path();
        if p.to_uppercase().starts_with(&folder_prefix) {
            if let Some(link) = p.strip_suffix(".Material.Gbx") {
                if link.rsplit('\\').next().map(|t| t.ends_with(suffix.as_str())).unwrap_or(false) {
                    links.push(link.to_string());
                }
            }
        }
    }
    links.sort();
    links.dedup();
    if links.is_empty() {
        return None;
    }
    Some((links, suffix))
}

/// A pack ITEM (`CGameItemModel` wrapper whose entity model -- a static
/// object, a prefab, or a variant list of them -- lives in EXTERNAL files):
/// bake the geometry the placement's `variant` external points at (index
/// into `pack_item_variants`; out of range -> variant 0, noted). Vegetation
/// (`.VegetTreeModel.Gbx`) has no mesh and is reported, not baked: the error
/// names the species file so the caller can substitute the right stock tree.
pub fn static_item_from_pack_item_report(store: &mut crate::store::DataStore, item_path: &str, ident: &str, author: &str, scale: f32, collection: u32, variant: usize) -> R<(Vec<u8>, Merged)> {
    static_item_from_pack_item_report_skin(store, item_path, ident, author, scale, collection, variant, None)
}

/// Same, for a placement with a light colour skin (light_skin.rs): the lights
/// take the swatch colour (an `Off` swatch drops them) and the glass
/// materials glow in it.
#[allow(clippy::too_many_arguments)]
pub fn static_item_from_pack_item_report_skin(store: &mut crate::store::DataStore, item_path: &str, ident: &str, author: &str, scale: f32, collection: u32, variant: usize, light_skin: Option<crate::light_skin::LightSkin>) -> R<(Vec<u8>, Merged)> {
    let variants = pack_item_variants(store, item_path)?;
    let mut m = Merged::default();
    m.light_skin = light_skin;
    m.keep_water = keep_water_for(collection);
    if variants.is_empty() {
        let model = store.load_model(item_path)?;
        return Err(format!("no prefab/static-object external (externals: {})", model.externals.iter().map(|(_, p)| p.rsplit('\\').next().unwrap_or(p).to_string()).collect::<Vec<_>>().join(", ")));
    }
    // The item's own modifier (`…\Media\Modifier\<X>.Gbx`, a game skin
    // wrapper): its materials are `…\Modifier\<folder>\<stem><suffix>` where
    // X = folder + suffix (`ItemObstacleLevel1` = `ItemObstacle` + `Level1`).
    if let Some((links, suffix)) = item_modifier_links(store, item_path) {
        m.notes.push(format!("modifier {} ({} materials)", suffix, links.len()));
        // a gameplay gate's kind folder (`Modifier\Turbo2\{Sign,SignOff,…}`)
        if suffix.is_empty() && links.iter().any(|l| l.ends_with("\\Sign")) {
            if let Some(kind) = links[0].strip_prefix("Stadium\\Media\\Modifier\\").and_then(|r| r.split('\\').next()) {
                m.gate_kind = Some(kind.to_string());
            }
        }
        m.modifier = links;
        m.modifier_suffix = suffix;
    }
    // The item's skin declaration (header chunk 0x090F4000: the in-game
    // advertisement slot of screens and gates, the colour slot of lights) is
    // carried verbatim — the game applies skins only to a model that declares
    // one (2026-09-07: every tiny screen drew the default yellow panel).
    if let Some(chunk) = store.read(item_path).ok().and_then(|b| tmmaps::header::game_skin_chunk(&b)) {
        if let Some(s) = tmmaps::header::GameSkin::decode(&chunk) {
            m.notes.push(format!("skin {} ({} slots)", s.dir, s.fids.len()));
        }
        m.skin = Some(chunk);
    }
    let picked = match variants.get(variant) {
        Some(p) => p.clone(),
        None => {
            m.notes.push(format!("variant {variant} of {} is out of range; baked variant 0", variants.len()));
            variants[0].clone()
        }
    };
    let low = picked.to_ascii_lowercase();
    if low.ends_with(".vegettreemodel.gbx") {
        return Err(format!("procedural vegetation: {picked} (variant {variant} of {}, no mesh)", variants.len()));
    }
    if low.ends_with(".prefab.gbx") {
        add_prefab(store, &picked, &IDENTITY, scale, &mut m, 0)?;
    } else {
        add_static_object_file(store, &picked, &IDENTITY, scale, &mut m)?;
    }
    if variants.len() > 1 {
        m.notes.push(format!("variant {variant} of {}: {}", variants.len(), picked.rsplit('\\').next().unwrap_or(&picked)));
    }
    // The waypoint TYPE lives in the item's own chunk (0 Start, 1 Finish, 2
    // Checkpoint, 3 none, 4 StartFinish). A checkpoint/finish prefab also
    // says so through its NPlugTrigger_SWaypoint node (read above, with the
    // trigger shape); a START prefab has no such node — only the spawn — so
    // without this the baked start gate was type 3 and the map had no start.
    if m.waypoint_type.is_none() {
        if let Some(t) = store.read(item_path).ok().and_then(|b| pack_item_waypoint_type(&b)) {
            if t != 3 {
                m.waypoint_type = Some(t);
                m.notes.push(format!("waypoint type {t} from the item chunk"));
            }
        }
    }
    // A vegetation CLUSTER item (Stadium's `Spring` / `SpringCherryTree`:
    // a prefab of tree entities and nothing else) has no mesh to bake; the
    // caller places its trees as stock items from `m.veget`.
    if m.visuals.is_empty() && !m.veget.is_empty() {
        return Ok((Vec::new(), m));
    }
    // The gate sign panels' pictures (signlogo.rs), one per kind the item's
    // materials name; `sign_logo_material` re-points the panels at them.
    {
        let kinds: Vec<String> = m.materials.iter().filter_map(|mat| mat.link().and_then(super::signlogo::kind_of_pseudo).map(|s| s.to_string())).collect();
        for kind in kinds {
            let file = super::signlogo::logo_file(&kind);
            if m.pictures.iter().any(|(f, _)| *f == file) {
                continue;
            }
            match super::signlogo::logo_dds(store, &kind) {
                Ok(dds) => {
                    m.notes.push(format!("sign logo {kind}: {} ({} bytes)", file, dds.len()));
                    m.pictures.push((file, dds));
                }
                Err(e) => m.notes.push(format!("sign logo {kind}: {e}; game material kept")),
            }
        }
    }
    // A light colour skin: which of the item's materials are the glass (their
    // pack material has a self-illumination `_I` texture) — those get the
    // swatch as a self-lit custom material at assembly; the swatch file rides
    // next to the item.
    if let Some(skin) = m.light_skin.clone() {
        let links: Vec<String> = m.materials.iter().filter_map(|mat| mat.link().map(|s| s.to_string())).collect();
        for link in links {
            let path = format!("{link}.Material.Gbx");
            let illum = store.load_model(&path).map(|mm| mm.externals.iter().any(|(_, e)| e.to_ascii_lowercase().ends_with("_i.texture.gbx"))).unwrap_or(false);
            if illum && !m.illum_links.contains(&link) {
                m.illum_links.push(link);
            }
        }
        m.notes.push(format!("light skin {} (srgb {:?}): {} lit material(s) re-dressed, {} light(s){}", skin.name, skin.srgb, m.illum_links.len(), m.lights_out.len(), if skin.is_off() { " OFF" } else { "" }));
        if !m.pictures.iter().any(|(f, _)| *f == skin.file()) {
            m.pictures.push((skin.file(), skin.dds.to_vec()));
        }
    }
    let opts = BuildOpts { ident: ident.to_string(), author: author.to_string(), scale, collection, skin: m.skin.clone() };
    let f = assemble(&m, &opts)?;
    Ok((super::write_file(&f), m))
}

/// The waypoint type of a PACK item (its chunk 0x2E00201F: version, type).
/// The item parser cannot walk a pack wrapper — its entity model (0x2E002019,
/// before the waypoint chunk) is an external node — so the chunk is found by
/// its id in the 3 KB body and read where the version and the type are
/// plausible (types 0 Start .. 5 Dispenser).
pub fn pack_item_waypoint_type(item_bytes: &[u8]) -> Option<i32> {
    let g = tmmaps::gbx::Gbx::parse(item_bytes);
    let body = &g.body;
    let id = 0x2E00_201Fu32.to_le_bytes();
    let mut i = 0usize;
    while i + 12 <= body.len() {
        if body[i..i + 4] == id {
            let version = u32::from_le_bytes(body[i + 4..i + 8].try_into().unwrap());
            let t = i32::from_le_bytes(body[i + 8..i + 12].try_into().unwrap());
            if version <= 20 && (0..=5).contains(&t) {
                return Some(t);
            }
        }
        i += 1;
    }
    None
}


/// `float3(a)` / `float3(a,b,c)` with numeric literals, every component
/// multiplied by `scale`; `None` for any other expression (left as is).
fn scale_float3_literal(expr: &str, scale: f32) -> Option<String> {
    let inner = expr.trim().strip_prefix("float3(")?.strip_suffix(')')?;
    let parts: Vec<f32> = inner.split(',').map(|p| p.trim().parse::<f32>()).collect::<Result<_, _>>().ok()?;
    if parts.len() != 1 && parts.len() != 3 {
        return None;
    }
    let fmt = |v: f32| {
        let s = format!("{}", v * scale);
        if s.contains('.') || s.contains('e') { s } else { format!("{s}.0") }
    };
    if parts.iter().all(|v| *v == 0.0) {
        return Some(expr.to_string());
    }
    Some(format!("float3({})", parts.iter().map(|v| fmt(*v)).collect::<Vec<_>>().join(",")))
}

#[cfg(test)]
mod fx_tests {
    #[test]
    fn float3_literals_scale() {
        assert_eq!(super::scale_float3_literal("float3(0,0,10)", 0.5).as_deref(), Some("float3(0.0,0.0,5.0)"));
        assert_eq!(super::scale_float3_literal("float3(0)", 0.5).as_deref(), Some("float3(0)"));
        assert_eq!(super::scale_float3_literal("float3(0,0,0)", 0.5).as_deref(), Some("float3(0,0,0)"));
        assert_eq!(super::scale_float3_literal("cos(Time/500)", 0.5), None);
        assert_eq!(super::scale_float3_literal("float3(1.5,-2,3)", 0.5).as_deref(), Some("float3(0.75,-1.0,1.5)"));
    }
}

/// What a vegetation bake did, for the report.
#[derive(Clone, Debug, Default)]
pub struct VegetBake {
    /// The `.VegetTreeModel.Gbx` baked.
    pub model: String,
    /// Visuals per detail level, and the switch distances written (metres,
    /// the model's own — NOT scaled: a half-size tree switches where the
    /// full-size one did).
    pub levels: Vec<usize>,
    pub switch: Vec<f32>,
    /// (archive file name, bytes) of every texture the item names.
    pub textures: Vec<(String, usize)>,
    /// Model-space height and radius of the source (metres, unscaled).
    pub height: f32,
    pub radius: f32,
    /// Trunk hull triangles carried (0: the species has none).
    pub hull_triangles: usize,
    /// Vertex elements stripped from the visuals (the vegetation shader's
    /// wind weights ride as vertex colours the static shaders would tint by).
    pub stripped: Vec<&'static str>,
}

/// A procedural vegetation model (`crate::veget`) baked as STATIC geometry
/// into `m`, scaled: every detail level's visuals under custom-texture
/// materials — the model's inline materials name no `.Material.Gbx`, so
/// each becomes an item-editor material (`is_using_game_material` false)
/// on the game's own shading models with the pack's diffuse image in slot 0:
/// `TDSN` for bark AND for the leaf cards — TDSN alpha-tests a diffuse that
/// carries an alpha channel (measured 2026-09-08: proper fronds, clean
/// edges), while TDOSN / TDOBSN draw such cards invisible and TDOSN2Sided
/// is not a model the item loader knows (red). Two-sidedness comes from a
/// reversed copy of every leaf triangle. Every visual gets a TexCoord1 set
/// (`ensure_texcoord1`): without one a visual is not drawn at all. The images ride next to the item
/// (`Merged::pictures`, `Items/<name>.dds` in the library archive), their
/// top mip levels cut to `TINY_TREE_TEX_MAX` pixels a side (default 256: the
/// 31 leaf and bark atlases of a BlueBay map weighed 5.2 MB at 512, 1.3 at
/// 256, against a ~25 MB upload cap).
/// The trunk hull becomes the collision (Wood). What the bake cannot carry:
/// the wind animation, the game's impostors past the far distance (the last
/// level is drawn at every distance instead) and the placement-colour hue
/// mask (the species' default look is baked).
///
/// Knobs: TINY_TREE_LEAF_MODEL / TINY_TREE_BARK_MODEL (shading model names),
/// TINY_TREE_TEX_MAX (pixels), TINY_TREE_KEEP_COLOR=1 (keep the vertex
/// colour elements), TINY_TREE_LOD_MIN=N (drop the levels finer than N: the
/// size lever), TINY_TREE_NORMAL_MAP=1 (also name the `_N` image in slot 1).
pub fn add_veget_tree_model(store: &mut crate::store::DataStore, model_path: &str, scale: f32, m: &mut Merged) -> R<VegetBake> {
    use super::vstream::N_COLOR0;
    let t = crate::veget::parse_tree_model(store, model_path)?;
    let stats = t.stats();
    let leaf_model = std::env::var("TINY_TREE_LEAF_MODEL").unwrap_or_else(|_| "TDSN".into());
    let bark_model = std::env::var("TINY_TREE_BARK_MODEL").unwrap_or_else(|_| "TDSN".into());
    let tex_max: u32 = std::env::var("TINY_TREE_TEX_MAX").ok().and_then(|v| v.parse().ok()).unwrap_or(256);
    let normal_map = std::env::var_os("TINY_TREE_NORMAL_MAP").is_some();
    let keep_color = std::env::var_os("TINY_TREE_KEEP_COLOR").is_some();
    let mut lod_min: usize = std::env::var("TINY_TREE_LOD_MIN").ok().and_then(|v| v.parse().ok()).unwrap_or(0);
    // The campaign's size lever applies to trees like to blocks: `--lod-pick N`
    // (with its min-verts) keeps level N alone, every distance, for a species
    // whose nearest level has that many vertices. Otherwise every level rides
    // with the model's own ladder (`all_lods`: the ladder is kept whatever
    // the pick says).
    let level0_verts: i32 = t.lods.first().map(|l| l.iter().map(|e| e.visual.main.as_ref().map(|mm| mm.count).unwrap_or(0)).sum()).unwrap_or(0);
    let pick: Option<usize> = match lod_pick() {
        Some(p) if level0_verts >= p.min_verts => Some((p.level as usize).min(t.lods.len() - 1)),
        // a species under the vertex floor keeps level TINY_TREE_LOD_MIN alone
        Some(_) => Some(lod_min.min(t.lods.len() - 1)),
        None => None,
    };
    if let Some(p) = pick {
        lod_min = p;
    }
    let mut out = VegetBake { model: model_path.to_string(), height: stats.top, radius: stats.radius, ..Default::default() };
    // one item material per model material, in model order
    let mut slots: Vec<usize> = Vec::with_capacity(t.materials.len());
    for mat in &t.materials {
        let mut files: Vec<(i32, String)> = Vec::new();
        // TINY_TREE_LEAF_SLOTS=0,4 / TINY_TREE_BARK_SLOTS=0: the user-texture
        // slots the diffuse image fills (the slot enum is the game's: 0 is the
        // diffuse, 5 the self-illumination; which one a model reads its opacity
        // from is what the lineups measure)
        let slots_env = if mat.leaf { "TINY_TREE_LEAF_SLOTS" } else { "TINY_TREE_BARK_SLOTS" };
        let d_slots: Vec<i32> = std::env::var(slots_env).ok().map(|s| s.split(',').filter_map(|x| x.trim().parse().ok()).collect()).filter(|v: &Vec<i32>| !v.is_empty()).unwrap_or_else(|| vec![0]);
        let mut wanted: Vec<(i32, &Option<String>)> = d_slots.iter().map(|s| (*s, &mat.images[0])).collect();
        if normal_map {
            // TINY_TREE_NORMAL_SLOT (default 1): the user-texture slot the _N image fills
            let n_slot: i32 = std::env::var("TINY_TREE_NORMAL_SLOT").ok().and_then(|v| v.parse().ok()).unwrap_or(1);
            wanted.push((n_slot, &mat.images[1]));
        }
        for (slot, image) in wanted {
            let Some(path) = image else { continue };
            // (TINY_TREE_MAT_SUFFIX tags the image file names too: one lineup can
            // then carry the same image in two encodings)
            let base = path.rsplit('\\').next().unwrap_or(path);
            let file = match std::env::var("TINY_TREE_MAT_SUFFIX") {
                Ok(sfx) if !sfx.is_empty() => format!("{}{sfx}.dds", base.trim_end_matches(".dds")),
                _ => base.to_string(),
            };
            if !m.pictures.iter().any(|(f, _)| *f == file) {
                let bytes = store.read(path).map_err(|e| format!("{path}: {e}"))?;
                // TINY_TREE_TEX_FORMAT=rgba|dds|leaf-rgba (default leaf-rgba): the
                // leaf images, whose alpha is the opacity mask, ship UNCOMPRESSED
                // 32-bit at the capped level; the bark stays block-compressed with
                // its mips. `rgba` uncompresses everything, `dds` nothing.
                let fmt = std::env::var("TINY_TREE_TEX_FORMAT").unwrap_or_else(|_| "dds".into());
                let uncompressed = fmt == "rgba" || (fmt == "leaf-rgba" && mat.leaf);
                // TINY_TREE_LEAF_ALPHA_MAX=N (uncompressed leaf path only): opaque alpha
                // clamped to N — the probe for "alpha doubles as the gloss mask"
                let alpha_max: Option<u8> = if mat.leaf { std::env::var("TINY_TREE_LEAF_ALPHA_MAX").ok().and_then(|v| v.parse().ok()) } else { None };
                let bytes = if uncompressed || alpha_max.is_some() {
                    let (w, h, mut rgba) = super::texture::decode_capped_rgba(&bytes, tex_max).map_err(|e| format!("{path}: {e}"))?;
                    if let Some(cap) = alpha_max {
                        for px in rgba.chunks_mut(4) {
                            px[3] = px[3].min(cap);
                        }
                    }
                    super::texture::write_dds_rgba(w, h, &rgba)
                } else {
                    super::texture::dds_cap(&bytes, tex_max).map_err(|e| format!("{path}: {e}"))?
                };
                out.textures.push((file.clone(), bytes.len()));
                m.pictures.push((file.clone(), bytes));
            }
            files.push((slot, file));
        }
        // TINY_TREE_LEAF_CONST=SLOT:RRGGBB[AA] (bark: TINY_TREE_BARK_CONST): a 4x4
        // constant-colour texture in one more slot — the probe for which slot a
        // shading model reads its specular / roughness from (black in the
        // specular slot = matte).
        let const_env = if mat.leaf { "TINY_TREE_LEAF_CONST" } else { "TINY_TREE_BARK_CONST" };
        if let Ok(spec) = std::env::var(const_env) {
            if let Some((slot, hex)) = spec.split_once(':') {
                let slot: i32 = slot.trim().parse().map_err(|_| format!("{const_env}: bad slot in {spec}"))?;
                let v = u32::from_str_radix(hex.trim(), 16).map_err(|_| format!("{const_env}: bad colour in {spec}"))?;
                let (r, g, b, a) = if hex.trim().len() > 6 { ((v >> 24) as u8, (v >> 16) as u8, (v >> 8) as u8, v as u8) } else { ((v >> 16) as u8, (v >> 8) as u8, v as u8, 255u8) };
                let file = format!("Const_{}{}.dds", hex.trim(), std::env::var("TINY_TREE_MAT_SUFFIX").unwrap_or_default());
                if !m.pictures.iter().any(|(f, _)| *f == file) {
                    let px: Vec<u8> = (0..16).flat_map(|_| [r, g, b, a]).collect();
                    let bytes = super::texture::write_dds_rgba(4, 4, &px);
                    out.textures.push((file.clone(), bytes.len()));
                    m.pictures.push((file.clone(), bytes));
                }
                files.push((slot, file));
            }
        }
        if files.is_empty() {
            return Err(format!("material {} names no diffuse image", mat.name));
        }
        let mut inst = CPlugMaterialUserInst::game_material("", 14);
        if let Some(main) = inst.main.as_mut() {
            main.is_using_game_material = false;
            // The name is the material's IDENTITY to the game (custom materials of
            // one name are shared across items): it spells the shading model and
            // the diffuse image, so equal definitions share and different ones
            // never collide — the probe lineup of 13 palms whose "PalmTree_Leaf"
            // differed only by model crashed the client at the visual-merge site
            // 0x140456513 (element 3 of a 186-vertex visual read through NULL,
            // 2026-09-08). RedIsland names its materials plainly "_Leaf" / "_Bark".
            // TINY_TREE_MAT_SUFFIX tags the names further (one lineup, many variants).
            let model_name = if mat.leaf { leaf_model.clone() } else { bark_model.clone() };
            let d_stem = files.first().map(|(_, f)| f.trim_end_matches(".dds").to_string()).unwrap_or_default();
            let suffix = std::env::var("TINY_TREE_MAT_SUFFIX").unwrap_or_default();
            main.material_name = crate::crystal_model::Id::Str(format!("{model_name}_{d_stem}{suffix}"));
            main.model = crate::crystal_model::Id::Str(model_name.clone());
            main.link = crate::crystal_model::Id::Null;
            // TINY_TREE_BASE_TEXTURE=1: the diffuse file name in the BaseTexture string
            // too (the probe for which field a model reads its opacity from)
            if std::env::var("TINY_TREE_BASE_TEXTURE").map(|v| v == "1").unwrap_or(false) {
                main.base_texture = files.first().map(|(_, f)| f.trim_end_matches(".dds").to_string()).unwrap_or_default();
            }
            main.user_textures = files.into_iter().map(|(u01, texture)| crate::crystal_model::UserTexture { u01, texture }).collect();
        }
        // Two model materials of one look (TreeBigB's two bark materials both
        // read VegetOakBark_D) are ONE item material — the name spells the
        // look, so the game would merge them anyway, and item-check refuses
        // the duplicate slot (Summer 24 at ae694aa9, 2026-09-08: the one item
        // of 720 that failed, and the publish gate with it).
        let slot = match m.materials.iter().position(|x| same_look(x, &inst)) {
            Some(i) => i,
            None => {
                m.materials.push(inst);
                m.materials.len() - 1
            }
        };
        slots.push(slot);
    }
    // the visuals, level by level; the ladder is the model's own switch
    // distances, unscaled (see the doc comment)
    let ladder: Vec<f32> = if pick.is_some() { Vec::new() } else { t.switch.iter().skip(lod_min).copied().collect() };
    let mut kept_levels = 0usize;
    for (l, lod) in t.lods.iter().enumerate() {
        if l < lod_min || (pick.is_some() && l != lod_min) {
            continue;
        }
        let bit = l - lod_min;
        let mut n = 0usize;
        for e in lod {
            let mut v = e.visual.clone();
            if !keep_color {
                if let Some(main) = v.main.as_mut() {
                    if let Some(Node::VertexStream(s)) = main.vertex_streams.first_mut().and_then(|r| r.inline.as_deref_mut()) {
                        for name in [N_COLOR0, N_COLOR0 + 1] {
                            if s.decls.iter().any(|d| d.name() == name) {
                                drop_element(s, name);
                                let tag = if name == N_COLOR0 { "color0" } else { "color1" };
                                if !out.stripped.contains(&tag) {
                                    out.stripped.push(tag);
                                }
                            }
                        }
                    }
                }
            }
            // TINY_TREE_COLOR=AARRGGBB (word, as stored): every vertex gets this
            // colour0 — the probe for what the shading models do with the vertex
            // colour (a lighting multiplier? an AO term?)
            if let Ok(hex) = std::env::var("TINY_TREE_COLOR") {
                if let Ok(word) = u32::from_str_radix(hex.trim_start_matches("0x"), 16) {
                    if let Some(main) = v.main.as_mut() {
                        if let Some(Node::VertexStream(s)) = main.vertex_streams.first_mut().and_then(|r| r.inline.as_deref_mut()) {
                            let n = s.count.max(0) as usize;
                            if let Some(i) = s.decls.iter().position(|d| d.name() == N_COLOR0) {
                                s.elems[i] = Elem::Word(vec![word; n]);
                            } else {
                                use super::vstream::{Decl, T_COLOR};
                                let compress = s.compress_local3d.unwrap_or(false);
                                let mut items: Vec<(Decl, u32, Elem)> = s.decls.iter().zip(s.elems.iter()).map(|(d, e)| (d.clone(), d.stored_type(compress), e.clone())).collect();
                                items.push((Decl::with_stride(N_COLOR0, T_COLOR, 0, 0, 0), T_COLOR, Elem::Word(vec![word; n])));
                                items.sort_by_key(|(d, _, _)| d.name());
                                let stride: u32 = items.iter().map(|(_, st, _)| super::vstream::type_size(*st).unwrap_or(4) as u32).sum();
                                let mut offset = 0u32;
                                let (mut decls, mut elems) = (Vec::new(), Vec::new());
                                for (d, st, e) in items {
                                    decls.push(Decl::with_stride(d.name(), d.ty(), d.space(), offset, stride / 4));
                                    offset += super::vstream::type_size(st).unwrap_or(4) as u32;
                                    elems.push(e);
                                }
                                s.decls = decls;
                                s.elems = elems;
                            }
                            if !out.stripped.contains(&"=color") {
                                out.stripped.push("=color");
                            }
                        }
                    }
                }
            }
            // every visual carries a second texcoord set (see `ensure_texcoord1`)
            if let Some(main) = v.main.as_mut() {
                if let Some(Node::VertexStream(s)) = main.vertex_streams.first_mut().and_then(|r| r.inline.as_deref_mut()) {
                    if ensure_texcoord1(s) && !out.stripped.contains(&"+uv1") {
                        out.stripped.push("+uv1");
                    }
                }
            }
            transform_visual(&mut v, &IDENTITY, scale)?;
            // Leaf cards are seen from both sides. A shading model without a
            // two-sided variant gets its back faces as a second, reversed copy
            // of the index list (the normals stay the front ones — a lit back
            // face, not a dark one); TINY_TREE_LEAF_BACKFACES=0 leaves it.
            let leaf = t.materials[e.material as usize].leaf;
            // TINY_TREE_LEAF_BACKFACES=shared (default: reversed winding on the same
            // vertices — a back face lit like its front, the translucent look of
            // real foliage, no extra vertices) | flip (duplicated vertices with
            // reversed normals, `double_sided`; +60% leaf bytes, no visible gain in
            // the cp8 probes) | 0 (none)
            let backfaces = std::env::var("TINY_TREE_LEAF_BACKFACES").unwrap_or_else(|_| "shared".into());
            if leaf && !leaf_model.contains("2Sided") && backfaces == "flip" {
                // duplicated vertices with reversed normals (see `double_sided`)
                double_sided(&mut v)?;
                if !out.stripped.contains(&"+2sided") {
                    out.stripped.push("+2sided");
                }
            } else if leaf && !leaf_model.contains("2Sided") && backfaces == "shared" {
                if let Some(ib) = v.index_buffer.as_mut() {
                    let n = ib.indices.len() / 3 * 3;
                    let mut back = Vec::with_capacity(n);
                    for tri in ib.indices[..n].chunks(3) {
                        back.extend_from_slice(&[tri[0], tri[2], tri[1]]);
                    }
                    ib.indices.extend(back);
                    if !out.stripped.contains(&"+backfaces") {
                        out.stripped.push("+backfaces");
                    }
                }
            }
            m.visuals.push(MergedVisual { visual: v, material: slots[e.material as usize], lod_mask: if pick.is_some() { 0 } else { 1 << bit }, lod_ladder: ladder.clone() });
            n += 1;
        }
        out.levels.push(n);
        kept_levels += 1;
    }
    if kept_levels == 0 {
        return Err(format!("TINY_TREE_LOD_MIN={lod_min} leaves no detail level of {}", t.lods.len()));
    }
    merge_lod_ladder(&mut m.lod_max_dist, &ladder);
    m.all_lods = pick.is_none();
    out.switch = ladder;
    if let Some(p) = pick {
        m.notes.push(format!("vegetation bake: level {p} of {} alone (--lod-pick; nearest level {level0_verts} vertices)", t.lods.len()));
    }
    // the trunk hull as the collision (Wood, 14 — what the model says)
    if !t.hull_triangles.is_empty() {
        let tris: Vec<Triangle> = t.hull_triangles.iter().map(|(idx, mat)| Triangle { indices: *idx, material_id: (*mat).min(255) as u8, gameplay: 0, surface_index: 0 }).collect();
        m.add_surface_mesh(&t.hull_vertices, &tris, &IDENTITY, scale);
        out.hull_triangles = tris.len();
    }
    if m.file_write_time == 0 {
        m.file_write_time = t.file_write_time;
    }
    m.notes.push(format!("vegetation bake: {} levels {:?}, switch {:?} (unscaled), {} textures, hull {} tris{}", out.levels.len(), out.levels, out.switch, out.textures.len(), out.hull_triangles, if out.stripped.is_empty() { String::new() } else { format!(", stripped {}", out.stripped.join("+")) }));
    Ok(out)
}

/// A vegetation species as a half-size static item, on its own: the
/// `.VegetTreeModel.Gbx` (or a vegetation `.Item.Gbx`, followed to its model)
/// baked, assembled and written. The item bytes and the accumulator (its
/// `pictures` are the textures to ship next to the item).
pub fn static_item_from_veget_report(store: &mut crate::store::DataStore, path: &str, ident: &str, author: &str, scale: f32, collection: u32) -> R<(Vec<u8>, Merged, VegetBake)> {
    let model_path = crate::veget::tree_model_path(store, path)?;
    let mut m = Merged::default();
    m.keep_water = keep_water_for(collection);
    let bake = add_veget_tree_model(store, &model_path, scale, &mut m)?;
    let opts = BuildOpts { ident: ident.to_string(), author: author.to_string(), scale, collection, skin: None };
    let file = assemble(&m, &opts)?;
    Ok((super::file::write_file(&file), m, bake))
}

/// Give a vertex stream a second texcoord set (name 11, Float2) when it has
/// none: a copy of the first. The static shaders sample the lightmap through
/// TexCoord1 and a visual without one is not drawn at all — every leaf card
/// of the first tree probes was invisible under every valid material while
/// the same cards drew red under an unknown one (2026-09-08; the pack's
/// vegetation shader has no lightmap and its leaf visuals carry uv0 alone).
/// Declarations are rebuilt in ascending name order like `harmonize_layouts`.
pub fn ensure_texcoord1(s: &mut super::vstream::CPlugVertexStream) -> bool {
    use super::vstream::{Decl, T_FLOAT2};
    if s.decls.iter().any(|d| d.name() == N_TEXCOORD0 + 1) {
        return false;
    }
    let compress = s.compress_local3d.unwrap_or(false);
    let n = s.count.max(0) as usize;
    let uv0 = s.decls.iter().zip(s.elems.iter()).find(|(d, _)| d.name() == N_TEXCOORD0).map(|(_, e)| e.clone());
    let uv1 = match uv0 {
        Some(Elem::Float2(v)) => Elem::Float2(v),
        _ => Elem::Float2(vec![[0.0, 0.0]; n]),
    };
    let donor_uv0 = s.decls.iter().find(|d| d.name() == N_TEXCOORD0).cloned();
    let mut items: Vec<(Decl, u32, Elem)> = s.decls.iter().zip(s.elems.iter()).map(|(d, e)| (d.clone(), d.stored_type(compress), e.clone())).collect();
    let (ty, space) = donor_uv0.map(|d| (d.ty(), d.space())).unwrap_or((T_FLOAT2, 0));
    items.push((Decl::with_stride(N_TEXCOORD0 + 1, ty, space, 0, 0), T_FLOAT2, uv1));
    items.sort_by_key(|(d, _, _)| d.name());
    let stride: u32 = items.iter().map(|(_, st, _)| super::vstream::type_size(*st).unwrap_or(4) as u32).sum();
    let mut offset = 0u32;
    let mut decls = Vec::with_capacity(items.len());
    let mut elems = Vec::with_capacity(items.len());
    for (d, st, e) in items {
        decls.push(Decl::with_stride(d.name(), d.ty(), d.space(), offset, stride / 4));
        offset += super::vstream::type_size(st).unwrap_or(4) as u32;
        elems.push(e);
    }
    s.decls = decls;
    s.elems = elems;
    true
}

/// Make a visual two-sided PROPERLY: every vertex is duplicated with its
/// normal and tangent frame reversed, and every triangle is repeated with
/// reversed winding on the duplicates. A back face then carries a normal that
/// faces its viewer — the shared-normal shortcut (reversed winding on the
/// same vertices) hands the shader a normal pointing AWAY from the camera,
/// and the item shading models answer that with a blown-out yellow-white
/// glare wherever the sun stands behind the card (the "crumpled paper"
/// bushes of tiny 24's cp8, 2026-09-08).
pub fn double_sided(v: &mut super::visual::CPlugVisualIndexedTriangles) -> Result<(), String> {
    use super::vstream::T_DEC3N;
    let Some(main) = v.main.as_mut() else { return Ok(()) };
    let n = main.count.max(0) as usize;
    let Some(Node::VertexStream(s)) = main.vertex_streams.first_mut().and_then(|r| r.inline.as_deref_mut()) else { return Ok(()) };
    let compress = s.compress_local3d.unwrap_or(false);
    for (d, e) in s.decls.iter().zip(s.elems.iter_mut()) {
        let flip = matches!(d.name(), N_NORMAL | N_TANGENT_U | N_TANGENT_V);
        match e {
            Elem::Float2(x) => {
                let c = x.clone();
                x.extend(c);
            }
            Elem::Float3(x) => {
                let mut c = x.clone();
                if flip {
                    for p in c.iter_mut() {
                        *p = [-p[0], -p[1], -p[2]];
                    }
                }
                x.extend(c);
            }
            Elem::Float4(x) => {
                let c = x.clone();
                x.extend(c);
            }
            Elem::Word(x) => {
                let mut c = x.clone();
                if flip && d.stored_type(compress) == T_DEC3N {
                    for w in c.iter_mut() {
                        let p = dec3n_unpack(*w);
                        *w = dec3n_pack([-p[0], -p[1], -p[2]]);
                    }
                }
                x.extend(c);
            }
            Elem::Raw { size, bytes } => {
                let c = bytes[..n * *size].to_vec();
                bytes.extend(c);
            }
        }
    }
    s.count = (n * 2) as i32;
    main.count = (n * 2) as i32;
    if let Some(ib) = v.index_buffer.as_mut() {
        let m = ib.indices.len() / 3 * 3;
        let mut back = Vec::with_capacity(m);
        for tri in ib.indices[..m].chunks(3) {
            back.extend_from_slice(&[tri[0] + n as u32, tri[2] + n as u32, tri[1] + n as u32]);
        }
        ib.indices.extend(back);
    }
    Ok(())
}
