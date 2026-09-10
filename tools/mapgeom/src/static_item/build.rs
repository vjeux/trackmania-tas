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
            // NoRespawn } (read off Items\Gate\CheckpointLeft32m: 01 00 00 00 | 02 00 00
            // 00 = checkpoint | 1d 00 00 00 = node 29, the external
            // *_Trigger.Shape.Gbx | 00 00 00 00). The item gets that type, the
            // shape, transformed like the geometry, and the no-respawn flag.
            Some(Node::WaypointTrigger(wp)) => {
                let wtype = wp.wtype;
                let shape_idx = wp.shape.index;
                if wp.no_respawn != 0 {
                    m.no_respawn = true;
                }
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
    add_sign_logo_pictures(store, &mut m);
    add_screen_logo_pictures(store, &mut m);
    m.darken_screen_faces();
    trigger_fx_pass(store, &mut m);
    let opts = BuildOpts { ident: ident.to_string(), author: author.to_string(), scale, collection, skin: m.skin.clone() };
    let f = assemble(&m, &opts)?;
    if let Some(n) = super::assemble::REPACK_NOTE.with(|c| c.get()) {
        m.notes.push(format!("lightmap atlas: {n} parts repacked into disjoint cells"));
    }
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
    if let Some(n) = super::assemble::REPACK_NOTE.with(|c| c.get()) {
        m.notes.push(format!("lightmap atlas: {n} parts repacked into disjoint cells"));
    }
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
/// its frames, frame table, tween material and the pack's inline-vertex form.
/// OPT-IN (`TINY_FLAG_TWEEN=1`; the default is the still cloth under `ItemFlagNoAnim`,
/// until the hidden-driver form passes a full-map check — coordinator, 20:06Z). What
/// the day's lineups established: an embedded tween cloth never gets tween state
/// of its own — its draw borrows the per-material frame state a STOCK flag's draw fills, so it is right
/// only while a stock flag is DRAWN in the same view at the SAME detail level
/// (the state indexes the level's frame table); no stock in view → bare pole,
/// stock in the map but out of view → nothing or shards, different level →
/// garbage (crumpled shards, giant sails). Hence the two halves of the form:
/// the cloth keeps the PACK detail ladder (`add_dyna_tween_part`), and
/// `tmmaps tiny` hangs a stock flag of the same kind upside down under every
/// converted placement (`TINY_FLAG_DRIVER`, TINY.md "Animated items" — a
/// named HACK; the proper self-contained form is still wanted). `Flag16m`
/// placements are the stock `Flag8m` anyway (`stock_half_variant`); this is
/// for the `Flag8m` ones.
pub fn tween_parts_enabled() -> bool {
    if let Some(v) = TWEEN_OVERRIDE.with(|o| o.get()) {
        return v;
    }
    std::env::var("TINY_FLAG_TWEEN").as_deref() == Ok("1")
}

thread_local! {
    /// An explicit animation phase (0..1 of the period) for every constrained
    /// moving part baked while set: written into the part's SInstanceParams
    /// (Phase01 and Phase01Max, the pack pushers carry -1 = unset in both).
    /// `mapgeom static-item --phase01`, and tiny-library's per-placement
    /// phase variants (the map's AnimPhaseOffset byte, chunk 0x03043063).
    pub static DYNA_PHASE01: std::cell::Cell<Option<f32>> = const { std::cell::Cell::new(None) };
}

/// `NPlugDynaObjectModel_SInstanceParams` (0x2F0B6000) bytes with Phase01 and
/// Phase01Max (words 5 and 6 after the version) set to `phase`. The struct is
/// {version, PeriodSc, TextureId, IsKinematic, [v>=1: PeriodScMax, Phase01,
/// Phase01Max], [v>=2: CastStaticShadow]}; a version-0 record (no phase
/// words) is returned unchanged.
pub fn with_phase01(params: &[u8], phase: f32) -> Vec<u8> {
    let mut out = params.to_vec();
    if out.len() < 28 || u32::from_le_bytes(out[0..4].try_into().unwrap()) < 1 {
        return out;
    }
    out[20..24].copy_from_slice(&phase.to_le_bytes());
    out[24..28].copy_from_slice(&phase.to_le_bytes());
    out
}

thread_local! {
    /// A per-bake override of `tween_parts_enabled` (tiny-library bakes the
    /// STILL copy of a flag for the placements whose driver has nowhere to
    /// hide): `Some(false)` while that copy is built, `None` otherwise.
    pub static TWEEN_OVERRIDE: std::cell::Cell<Option<bool>> = const { std::cell::Cell::new(None) };
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
    // an explicit phase for this part (DYNA_PHASE01): the placement's
    // AnimPhaseOffset baked into the instance params
    let instance_params = match DYNA_PHASE01.with(|o| o.get()) {
        Some(p) => {
            m.notes.push(format!("  (moving part) instance params Phase01 = Phase01Max = {p:.3}"));
            with_phase01(&ent.params, p)
        }
        None => ent.params.clone(),
    };
    m.dyna.push(DynaPart {
        path: path.to_string(),
        rot,
        pos,
        mesh,
        move_shape,
        hit_shape,
        model: src.model.clone(),
        instance_params_id: ent.params_id,
        instance_params,
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
    // The cloth's detail ladder keeps the PACK distances ([16, 64, 128, 256]
    // on FlagSmall, [16, 64, 128, 512] on Flag) instead of the halved ones:
    // the stock flag that drives the tween switches level at those, and the
    // draw is right only while driver and cloth are at the same level (an13,
    // 2026-09-08: proper cloth at 10–200 m with the pack ladder; the halved
    // ladder is garbage in every band where the two disagree).
    // `TINY_FLAG_LADDER=half` restores the halved ladder for A/B.
    mesh.ladder_scale = match std::env::var("TINY_FLAG_LADDER").as_deref() {
        Ok("half") => None,
        _ => Some(1.0),
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
    mesh.materials_external = matches!(std::env::var("TINY_FLAG_MATREF").as_deref(), Ok("ext") | Ok("bare"));
    mesh.materials_bare = std::env::var("TINY_FLAG_MATREF").as_deref() == Ok("bare");
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

/// The gate sign panels' pictures (signlogo.rs), one per kind the item's
/// materials name through the `SignLogo<Kind>` pseudo link;
/// `sign_logo_material` re-points the panels at them at assembly. Without the
/// picture the panel falls back to the kind's `Modifier\<Kind>\Sign` game
/// material, whose unfed display shows the material's own `_I` picture (white
/// chevrons pointing UP on red for Turbo2) where the live gate shows the lit
/// logo (red chevrons pointing down on black). Until 2026-09-09 only the pack
/// gate ITEMS took this step; every BLOCK-baked pad and gate (the pads' kerb
/// signs, GateSpecialReset, Summer 15's Boost ring) fell back — Argentina 21's
/// PlatformGrassSpecialTurbo2 kerb sign was vjeux's "the super turbo decal is
/// wrong" (frame sp21b pgB).
pub fn add_sign_logo_pictures(store: &mut crate::store::DataStore, m: &mut Merged) {
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
    add_sign_logo_pictures(store, &mut m);
    add_screen_logo_pictures(store, &mut m);
    m.darken_screen_faces();
    trigger_fx_pass(store, &mut m);
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
    if let Some(n) = super::assemble::REPACK_NOTE.with(|c| c.get()) {
        m.notes.push(format!("lightmap atlas: {n} parts repacked into disjoint cells"));
    }
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
/// on the game's own shading models with the pack's images in the slots the
/// model reads (`crate::crystal_model::USER_TEXTURE_SLOTS`, read off the exe):
/// the bark under `TDSN` with its diffuse in slot 0 (Diffuse); the leaf cards
/// under `TDOSN` with the atlas in slot 1 (DiffuseO — the diffuse WHOSE ALPHA
/// IS THE OPACITY, alpha-tested). Measured 2026-09-09 on the tiny-19 lineup
/// `y1` (TreeBigA and BushMediumD, seven oaks at 14 m and one at 6 m): under
/// TDSN the same atlas draws as OPAQUE cards — flat quads with the leaf
/// picture painted on the atlas' tan/grey-green background, the "origami"
/// crowns vjeux saw on every map — because TDSN reads slot 0 and treats the
/// alpha as nothing; under TDOSN and TDOBSN with the atlas in slot 1 every
/// card is cut to its leaves (twigs, single leaves, a sky-lit silhouette);
/// the earlier "TDOSN draws the cards invisible" (2026-09-08, fb2bad56) was the
/// atlas sitting in slot 0 while slot 1 stayed at the game's default image,
/// which is transparent. Two-sidedness comes from a
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
/// Knobs: TINY_TREE_LEAF_MODEL / TINY_TREE_BARK_MODEL (shading model names;
/// defaults TDOSN / TDSN), TINY_TREE_LEAF_SLOTS / TINY_TREE_BARK_SLOTS (the
/// slots the diffuse fills; defaults 1 / 0 — the slot the model reads),
/// TINY_TREE_TEX_MAX (pixels), TINY_TREE_KEEP_COLOR=1 (keep the vertex
/// colour elements), TINY_TREE_LOD_MIN=N (drop the levels finer than N: the
/// size lever), TINY_TREE_NORMAL_MAP=1 (also name the `_N` image in slot 5,
/// Normal; TINY_TREE_NORMAL_SLOT overrides), TINY_TREE_LEAF_CONST=SLOT:RRGGBB
/// (a constant image in one more slot: 4 is Specular, 12 RoughMetal).
/// `TINY_TREE_DEPTH=T1:F1[,T2:F2…]`: the leaf-card depth bands — (normalised
/// radius threshold, colour factor) pairs, ascending; None when unset or `0`.
pub fn depth_bands() -> Option<Vec<(f32, f32)>> {
    let spec = match std::env::var("TINY_TREE_DEPTH") {
        Ok(s) => s,
        // unset: the collection's own bands when the table is in force
        Err(_) if color_table_on() => leaf_look_for(VEGET_COLLECTION.with(|c| c.get())).3.to_string(),
        Err(_) => return None,
    };
    if spec.trim().is_empty() || spec.trim() == "0" {
        return None;
    }
    let mut out: Vec<(f32, f32)> = Vec::new();
    for part in spec.split(',').filter(|s| !s.trim().is_empty()) {
        let (t, f) = part.split_once(':')?;
        out.push((t.trim().parse().ok()?, f.trim().parse().ok()?));
    }
    out.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
    if out.is_empty() { None } else { Some(out) }
}

pub fn add_veget_tree_model(store: &mut crate::store::DataStore, model_path: &str, scale: f32, m: &mut Merged) -> R<VegetBake> {
    use super::vstream::N_COLOR0;
    let t = crate::veget::parse_tree_model(store, model_path)?;
    let stats = t.stats();
    let leaf_model = std::env::var("TINY_TREE_LEAF_MODEL").unwrap_or_else(|_| "TDOSN".into());
    let bark_model = std::env::var("TINY_TREE_BARK_MODEL").unwrap_or_else(|_| "TDSN".into());
    let tex_max: u32 = std::env::var("TINY_TREE_TEX_MAX").ok().and_then(|v| v.parse().ok()).unwrap_or(256);
    // The leaf atlases may keep one more level than the bark (TINY_TREE_LEAF_TEX_MAX;
    // default = TINY_TREE_TEX_MAX): the big oaks' branch atlas is authored at
    // 1024×512 and a 256-px cut of it is a sixteenth of the pixels. Measured
    // 2026-09-09: 512 costs ~1.1 MB on the big maps (24: 47.8 → 48.9 MB) and
    // buys little while the cards are not alpha-cut (see the trees thread note),
    // so it stays a knob.
    let leaf_tex_max: u32 = std::env::var("TINY_TREE_LEAF_TEX_MAX").ok().and_then(|v| v.parse().ok()).unwrap_or(tex_max);
    let normal_map = std::env::var_os("TINY_TREE_NORMAL_MAP").is_some();
    let keep_color = std::env::var_os("TINY_TREE_KEEP_COLOR").is_some();
    // Vertex colour probes of 2026-09-09 (tiny 19, six-variant lineups of
    // TreeBigA and BushMediumD, BEFORE the lightmap-uv1 fix f9039561): a tree
    // visual without a colour element or with a white one drew as a blazing
    // yellow-white ball where the same item with RGB black (any alpha), alpha 0
    // or mid grey drew dark green — the vertex colour scaled the broken
    // lightmap term. With the per-card uv1 charts the black and the stripped
    // form render identically (x2 lineup), so the default stays the stripped
    // form; TINY_TREE_BLACK_COLOR=1 writes an explicit black colour0 on every
    // tree vertex, TINY_TREE_COLOR=AARRGGBB any word, TINY_TREE_KEEP_COLOR=1
    // the model's own (RGB 0xFF, the self-AO byte in alpha).
    let default_color: Option<u32> = if std::env::var_os("TINY_TREE_BLACK_COLOR").is_some() && !keep_color { Some(0xFF00_0000) } else { None };
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
    let mut band_slots_per_mat: Vec<Vec<usize>> = Vec::with_capacity(t.materials.len());
    for mat in &t.materials {
        let mut files: Vec<(i32, String)> = Vec::new();
        // TINY_TREE_LEAF_SLOTS=1 / TINY_TREE_BARK_SLOTS=0: the user-texture
        // slots the diffuse image fills. The default is the slot the material's
        // model reads its colour from (`model_color_slot`: the `O` models read
        // DiffuseO = 1 and cut on its alpha, the others Diffuse = 0), so a model
        // knob alone never leaves the image in a slot the model ignores (the
        // 2026-09-08 "TDOSN draws nothing" was exactly that).
        let model_name = if mat.leaf { leaf_model.clone() } else { bark_model.clone() };
        let slots_env = if mat.leaf { "TINY_TREE_LEAF_SLOTS" } else { "TINY_TREE_BARK_SLOTS" };
        let default_slot = crate::crystal_model::model_color_slot(&model_name).unwrap_or(0);
        let d_slots: Vec<i32> = std::env::var(slots_env).ok().map(|s| s.split(',').filter_map(|x| x.trim().parse().ok()).collect()).filter(|v: &Vec<i32>| !v.is_empty()).unwrap_or_else(|| vec![default_slot]);
        let mut wanted: Vec<(i32, &Option<String>)> = d_slots.iter().map(|s| (*s, &mat.images[0])).collect();
        if normal_map {
            // TINY_TREE_NORMAL_SLOT (default 5 = Normal): the user-texture slot the _N image fills
            let n_slot: i32 = std::env::var("TINY_TREE_NORMAL_SLOT").ok().and_then(|v| v.parse().ok()).unwrap_or(5);
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
                let cap = if mat.leaf { leaf_tex_max } else { tex_max };
                // TINY_TREE_LEAF_MIPS=pack (default) | coverage | plain: the leaf
                // atlas' mip chain. `pack` ships the pack's own levels cut at the
                // cap; `coverage` rebuilds the chain from the capped top level with
                // every level's alpha scaled so the alpha test keeps the same share
                // of texels as at the top (`texture::mip_chain`) — a plain chain
                // halves that share per level, so an alpha-tested crown thins to
                // twigs with distance; `plain` is the rebuilt chain without the
                // scaling (the control). TINY_TREE_ALPHA_REF=N (default 128) is the
                // test's reference, TINY_TREE_ALPHA_GAIN=F (default 1) scales the top
                // level's alpha first. Bark keeps the pack chain.
                // The default chain of a LEAF atlas is the pack's own, re-encoded with
                // the calibrated colour (`pack-adj`, below); `pack` ships the pack
                // bytes untouched.
                let mips_mode = std::env::var("TINY_TREE_LEAF_MIPS").unwrap_or_else(|_| if mat.leaf { "pack-adj".into() } else { "pack".into() });
                // THE LEAF COLOUR (default since 2026-09-10, the trees quality pass):
                // the atlas is brightened and saturated at bake, because an item's
                // leaf cards are lit by the lightmap the game bakes at load — every
                // card an opaque occluder to that bake, so a crown gets the sky's
                // light and little sun — and the vegetation shader the stock trees
                // use adds a subsurface (translucent) term on top of a per-leaf sun
                // term. Measured on the GreenCoast lineups A–D (TreeSmallA,
                // BushMediumD, TreeThinSmallA, TreeBigA, BushBigB against the stock
                // species at the same angular size, both sides of the sun, 4K):
                // untouched, our crown is a flat dark blob (mean luma ~half the
                // stock's, hsat 18 % vs 40 %); gain 1.6 matches the small trees, 1.3
                // the bushes and the big oak, autumn foliage wants 1.6 whatever its
                // atlas; 2.0 overshoots everything. The rule that fits the five:
                // gain = clamp(132 / L, 1.0, 1.65) with L the atlas' mean
                // luma over its opaque texels (HoneyLocust 80 -> 1.65, Cercidophylle 92 -> 1.43,
                // BigOak 96 -> 1.38, pine 87 -> 1.52, the palms 100-113 -> 1.2-1.3, a snowy fir
                // 183 and the pink cherry 210 -> 1.0), and at least 1.6 for a warm mid-luma
                // autumn atlas (red over green by 12 or more, luma under 150: Populus 118); saturation x1.15 throughout.
                // TINY_TREE_LEAF_COLOR_ADJ=GAIN[,SAT] sets both by hand (`1,1` = the
                // atlas as it is).
                let color_adj: Option<(f32, f32, f32)> = match std::env::var("TINY_TREE_LEAF_COLOR_ADJ") {
                    Ok(adj) => {
                        let mut it = adj.split(',').map(|x| x.trim().parse::<f32>());
                        Some((it.next().and_then(|v| v.ok()).unwrap_or(1.0), it.next().and_then(|v| v.ok()).unwrap_or(1.0), it.next().and_then(|v| v.ok()).unwrap_or(0.0)))
                    }
                    Err(_) if mat.leaf => {
                        let (w0, h0, top) = super::texture::decode_capped_rgba(&bytes, cap).map_err(|e| format!("{path}: {e}"))?;
                        let _ = (w0, h0);
                        let (mut n, mut s) = (0u64, [0u64; 3]);
                        for px in top.chunks(4) {
                            if px[3] >= 128 {
                                n += 1;
                                for c in 0..3 {
                                    s[c] += px[c] as u64;
                                }
                            }
                        }
                        if n == 0 {
                            None
                        } else {
                            let (r, g, b) = (s[0] as f32 / n as f32, s[1] as f32 / n as f32, s[2] as f32 / n as f32);
                            let luma = 0.299 * r + 0.587 * g + 0.114 * b;
                            // the per-collection gain (`leaf_color_for`), capped by the atlas'
                            // own luma so a light atlas (the pink cherry 210, a snowy fir 183)
                            // is left alone — a gain of 1.6 bleached the cherry white
                            // TINY_TREE_COLOR_TABLE=1: the per-collection table (pass 3, being
                            // measured); unset = the pass-2 rule that ships in ship16
                            let table = color_table_on();
                            let (cg, csat, chue) = if table { leaf_color_for(VEGET_COLLECTION.with(|c| c.get())) } else { (1.65, 1.15, 0.0) };
                            // TINY_TREE_LIGHT_CAP=C (probe, 2026-09-10 pass 3b): the atlas-luma cap constant,
                            // 128 by default — lower makes the LIGHT atlases (birch 110–113,
                            // sous-bois 109–115) take less gain while the dark ones keep the
                            // collection's (the pale-foliage sheen probe)
                            let light_cap: f32 = std::env::var("TINY_TREE_LIGHT_CAP").ok().and_then(|v| v.parse().ok()).unwrap_or(128.0);
                            let mut gain = if table { cg.min(light_cap / luma.max(1.0)).clamp(1.0, 1.65) } else { (132.0 / luma.max(1.0)).clamp(1.0, 1.65) };
                            // warm (autumn) foliage renders dull under the item shading: at least
                            // 1.6 under the pass-2 rule, 1.25 under the table (Populus: measured
                            // need 1.2–1.3 on both sides)
                            if r >= g + 12.0 && luma < 150.0 {
                                gain = gain.max(if table { 1.25 } else { 1.6 });
                            }
                            // …and keeps its hue: the collection's shift towards yellow is fitted on
                            // GREEN crowns; on an autumn gold it lands on salmon (the eyes on the
                            // 19 frames, 2026-09-10 17:20Z: "salmon/rust-red instead of the muted
                            // gold"); its saturation stays moderate for the same reason
                            let warm = r >= g + 12.0 && luma < 150.0;
                            // …and so does any atlas that is not green to begin with (red over green
                            // by 4 or more: the desert creosote (109, 100, 67) and its near-grey
                            // twig atlas (88, 83, 78); the quince at +4 measured a wash either way
                            // and the yellow-green hazel at r = g better WITH the shift, lineup Z5):
                            // the shift
                            // counters the sky's cyan pull on GREEN crowns, on a khaki atlas it
                            // overshoots to brown (the eyes on the 17 frames: "bushes brown-khaki,
                            // too brown vs the original's dark olive"). TINY_TREE_KHAKI_SHIFT=1
                            // keeps the collection's shift on them (the probe's other arm).
                            let khaki = r >= g + 4.0 && !warm && std::env::var("TINY_TREE_KHAKI_SHIFT").as_deref() != Ok("1");
                            let (csat, chue) = if warm && table { (csat.min(1.2), 0.0) } else if khaki && table { (csat.min(1.3), 0.0) } else { (csat, chue) };
                            // a LOW-CHROMA atlas (HSV saturation of its opaque mean 0.30 or under:
                            // birch 0.25, hazel 0.26, sous-bois 0.21–0.30; the laurel at 0.32 sits
                            // 5 under the stock as it is and 6 over with the boost — left alone) renders
                            // 5–8 points less saturated than the stock under the collection's
                            // saturation and pulls yellower under its full hue shift (lineups Z3/Z6,
                            // 2026-09-10: ×1.3 more saturation put birch/sous-bois/hazel on the
                            // stock within 3 points; the shift showed through as hue 64–70 vs the
                            // stock's 73–84) — so ×1.3 saturation and half the shift. The pale
                            // "sheen" the eyes saw on these is that deficit, not brightness: a lower
                            // gain moved them away from the stock (Z3).
                            let mx = r.max(g).max(b);
                            let atlas_sat = if mx > 0.0 { (mx - r.min(g).min(b)) / mx } else { 0.0 };
                            let low_chroma = table && !warm && !khaki && atlas_sat <= 0.30 && std::env::var("TINY_TREE_LOWCHROMA").as_deref() != Ok("0");
                            let (csat, chue) = if low_chroma { (csat * 1.3, chue * 0.5) } else { (csat, chue) };
                            m.notes.push(format!("leaf atlas {file}: opaque mean ({r:.0}, {g:.0}, {b:.0}) luma {luma:.0} -> colour gain {gain:.2} saturation x{csat} hue {chue:+}{}", if warm { " (warm foliage: hue kept)" } else if khaki { " (khaki foliage: hue kept)" } else if low_chroma { " (low-chroma atlas: sat x1.3, half the shift)" } else { "" }));
                            Some((gain, csat, chue))
                        }
                    }
                    Err(_) => None,
                };
                let adjust = |rgba: &mut [u8], alpha_gain: f32| {
                    if let Some((gain, sat, hue)) = color_adj {
                        if hue.abs() <= 1e-3 {
                            // the pass-2 form (ship16's bytes): saturation stretched about the luma
                            for px in rgba.chunks_mut(4) {
                                let (r, g, b) = (px[0] as f32, px[1] as f32, px[2] as f32);
                                let l = 0.299 * r + 0.587 * g + 0.114 * b;
                                let f = |c: f32| ((l + (c - l) * sat) * gain).round().clamp(0.0, 255.0) as u8;
                                px[0] = f(r);
                                px[1] = f(g);
                                px[2] = f(b);
                            }
                        } else {
                            // hue: a rotation of the chroma plane (YIQ) by `hue` degrees — NEGATIVE =
                            // towards yellow (lower HSV hue): red→yellow→green→cyan runs CLOCKWISE in
                            // the (I, Q) plane, so the HSV sense is the negated IQ angle (the first
                            // pass-3 bakes had it backwards: −15 made the crowns GREENER) — then the
                            // saturation and the gain
                            let (hc, hs) = ((-hue).to_radians().cos(), (-hue).to_radians().sin());
                            for px in rgba.chunks_mut(4) {
                                let (r, g, b) = (px[0] as f32, px[1] as f32, px[2] as f32);
                                let y = 0.299 * r + 0.587 * g + 0.114 * b;
                                let i0 = 0.596 * r - 0.274 * g - 0.322 * b;
                                let q0 = 0.211 * r - 0.523 * g + 0.312 * b;
                                let (i, q) = ((i0 * hc - q0 * hs) * sat, (i0 * hs + q0 * hc) * sat);
                                let rr = (y + 0.956 * i + 0.621 * q) * gain;
                                let gg = (y - 0.272 * i - 0.647 * q) * gain;
                                let bb = (y - 1.106 * i + 1.703 * q) * gain;
                                px[0] = rr.round().clamp(0.0, 255.0) as u8;
                                px[1] = gg.round().clamp(0.0, 255.0) as u8;
                                px[2] = bb.round().clamp(0.0, 255.0) as u8;
                            }
                        }
                    }
                    if (alpha_gain - 1.0).abs() > 1e-4 {
                        for px in rgba.chunks_mut(4) {
                            px[3] = (px[3] as f32 * alpha_gain).round().clamp(0.0, 255.0) as u8;
                        }
                    }
                };
                let bytes = if mat.leaf && mips_mode == "pack-adj" {
                    // the pack's own chain, level by level (its alpha already grows
                    // down the chain), colour-adjusted and alpha-scaled
                    // (TINY_TREE_ALPHA_GAIN on EVERY level), re-encoded as DXT5
                    let gain: f32 = std::env::var("TINY_TREE_ALPHA_GAIN").ok().and_then(|v| v.parse().ok()).unwrap_or(1.0);
                    let mut levels: Vec<super::texture::Level> = Vec::new();
                    let mut side = cap;
                    loop {
                        let (w, h, mut rgba) = super::texture::decode_capped_rgba(&bytes, side).map_err(|e| format!("{path}: {e}"))?;
                        if let Some(l) = levels.last() {
                            if l.w == w && l.h == h {
                                break;
                            }
                        }
                        adjust(&mut rgba, gain);
                        levels.push(super::texture::Level { w, h, rgba });
                        if w <= 1 && h <= 1 || side <= 1 {
                            break;
                        }
                        side /= 2;
                    }
                    m.notes.push(format!("leaf atlas {file}: pack chain re-encoded, {} levels from {}x{}, alpha x{gain}{}", levels.len(), levels[0].w, levels[0].h, color_adj.map(|(g, s, h)| format!(", colour gain {g} saturation {s} hue {h:+}")).unwrap_or_default()));
                    if uncompressed {
                        super::texture::write_dds_rgba_mips(&levels)
                    } else {
                        super::texture::write_dds_dxt5_mips(&levels)
                    }
                } else if mat.leaf && mips_mode != "pack" {
                    let alpha_ref: u8 = std::env::var("TINY_TREE_ALPHA_REF").ok().and_then(|v| v.parse().ok()).unwrap_or(128);
                    let gain: f32 = std::env::var("TINY_TREE_ALPHA_GAIN").ok().and_then(|v| v.parse().ok()).unwrap_or(1.0);
                    let (w, h, mut rgba) = super::texture::decode_capped_rgba(&bytes, cap).map_err(|e| format!("{path}: {e}"))?;
                    if let Some(cap) = alpha_max {
                        for px in rgba.chunks_mut(4) {
                            px[3] = px[3].min(cap);
                        }
                    }
                    adjust(&mut rgba, 1.0);
                    if let Some((g, s, h)) = color_adj {
                        m.notes.push(format!("leaf atlas {file}: colour gain {g} saturation {s} hue {h:+}"));
                    }
                    let levels = super::texture::mip_chain(super::texture::Level { w, h, rgba }, alpha_ref, mips_mode == "coverage", gain);
                    let top_cov = super::texture::alpha_coverage(&levels[0].rgba, alpha_ref);
                    let last = levels.iter().rev().find(|l| l.w >= 16 && l.h >= 16).unwrap_or(&levels[0]);
                    m.notes.push(format!("leaf atlas {file}: {mips_mode} chain of {} levels from {w}x{h}, coverage at alpha {alpha_ref}: top {:.1} %, {}x{} {:.1} %", levels.len(), top_cov * 100.0, last.w, last.h, super::texture::alpha_coverage(&last.rgba, alpha_ref) * 100.0));
                    if uncompressed {
                        super::texture::write_dds_rgba_mips(&levels)
                    } else {
                        super::texture::write_dds_dxt5_mips(&levels)
                    }
                } else if uncompressed || alpha_max.is_some() {
                    let (w, h, mut rgba) = super::texture::decode_capped_rgba(&bytes, cap).map_err(|e| format!("{path}: {e}"))?;
                    if let Some(cap) = alpha_max {
                        for px in rgba.chunks_mut(4) {
                            px[3] = px[3].min(cap);
                        }
                    }
                    super::texture::write_dds_rgba(w, h, &rgba)
                } else {
                    super::texture::dds_cap(&bytes, cap).map_err(|e| format!("{path}: {e}"))?
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
        // TINY_TREE_NATURAL=1: the material's `IsNatural` byte (chunk 0x090FD001
        // v5) — the 2026-09-09 probe of whether it is the vegetation switch
        // that keeps the game's own foliage from glaring under local lights
        if std::env::var("TINY_TREE_NATURAL").map(|v| v == "1").unwrap_or(false) {
            if let Some(t) = inst.tiling.as_mut() {
                t.is_natural = true;
            }
        }
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
                m.materials.push(inst.clone());
                m.materials.len() - 1
            }
        };
        slots.push(slot);
        // TINY_TREE_DEPTH=T1:F1[,T2:F2…] (self-shadow, 2026-09-10): the leaf cards
        // are split into radial BANDS about the crown centre — a card whose
        // normalised radius is under T1 draws the atlas darkened by F1, under T2
        // by F2, …, the rest the atlas itself — so the crown has a dark interior
        // and a lit rim like the stock canopy's self-shadowing, which the game's
        // per-item lightmap does not give our cards (every card the same value).
        // One more material and one more (small) atlas copy per band.
        let mut band_slots: Vec<usize> = Vec::new();
        if mat.leaf {
            if let Some(bands) = depth_bands() {
                for (bi, (_thr, factor)) in bands.iter().enumerate() {
                    let mut binst = inst.clone();
                    let mut bfiles: Vec<(i32, String)> = Vec::new();
                    for (slot_id, file) in binst.main.as_ref().map(|mn| mn.user_textures.iter().map(|t| (t.u01, t.texture.clone())).collect::<Vec<_>>()).unwrap_or_default() {
                        // only the colour slot gets a darkened copy; the others ride as they are
                        if slot_id == default_slot {
                            let dark_file = format!("{}_in{bi}.dds", file.trim_end_matches(".dds"));
                            if !m.pictures.iter().any(|(f, _)| *f == dark_file) {
                                let src = m.pictures.iter().find(|(f, _)| *f == file).map(|(_, b)| b.clone()).ok_or_else(|| format!("depth band: no picture {file}"))?;
                                // the inner bands are dark and half-hidden: their copies ride one
                                // mip level smaller (128 px) — 2.3 MB on Summer 19 otherwise
                                let src = super::texture::dds_cap(&src, 128).map_err(|e| format!("{file}: {e}"))?;
                                let bytes = super::texture::darken_dds(&src, *factor).map_err(|e| format!("{file}: {e}"))?;
                                out.textures.push((dark_file.clone(), bytes.len()));
                                m.pictures.push((dark_file.clone(), bytes));
                            }
                            bfiles.push((slot_id, dark_file));
                        } else {
                            bfiles.push((slot_id, file));
                        }
                    }
                    if let Some(main) = binst.main.as_mut() {
                        let name = match &main.material_name {
                            crate::crystal_model::Id::Str(s) => format!("{s}_in{bi}"),
                            _ => format!("{model_name}_in{bi}"),
                        };
                        main.material_name = crate::crystal_model::Id::Str(name);
                        main.user_textures = bfiles.into_iter().map(|(u01, texture)| crate::crystal_model::UserTexture { u01, texture }).collect();
                    }
                    let bslot = match m.materials.iter().position(|x| same_look(x, &binst)) {
                        Some(i) => i,
                        None => {
                            m.materials.push(binst);
                            m.materials.len() - 1
                        }
                    };
                    band_slots.push(bslot);
                }
            }
        }
        band_slots_per_mat.push(band_slots);
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
            // colour0 — the probe knob that found the glare; the default is the
            // black it measured (see `default_color`)
            {
                let forced: Option<u32> = std::env::var("TINY_TREE_COLOR").ok().and_then(|hex| u32::from_str_radix(hex.trim_start_matches("0x"), 16).ok()).or(default_color);
                // TINY_TREE_VCOL_AO=1 (probe, 2026-09-10): a per-vertex grey ramp by
                // normalised radius about the visual's box centre — inner vertices
                // 0x40, outer 0xFF (alpha 0xFF) — the test of whether the shading
                // model multiplies its diffuse by colour0 at all (the cheapest
                // self-shadow if it does)
                let vcol_ao = std::env::var("TINY_TREE_VCOL_AO").as_deref() == Ok("1");
                if forced.is_some() || vcol_ao {
                    let word = forced.unwrap_or(0xFFFF_FFFF);
                    if let Some(main) = v.main.as_mut() {
                        let (c, half) = ([main.bounding_box[0], main.bounding_box[1], main.bounding_box[2]], [main.bounding_box[3].max(0.01), main.bounding_box[4].max(0.01), main.bounding_box[5].max(0.01)]);
                        if let Some(Node::VertexStream(s)) = main.vertex_streams.first_mut().and_then(|r| r.inline.as_deref_mut()) {
                            let n = s.count.max(0) as usize;
                            let words: Vec<u32> = if vcol_ao {
                                let pos: Vec<[f32; 3]> = match s.decls.iter().zip(s.elems.iter()).find(|(d, _)| d.name() == super::vstream::N_POSITION).map(|(_, e)| e) {
                                    Some(Elem::Float3(p)) => p.clone(),
                                    _ => Vec::new(),
                                };
                                (0..n)
                                    .map(|k| {
                                        let p = pos.get(k).copied().unwrap_or(c);
                                        let d = [(p[0] - c[0]) / half[0], (p[1] - c[1]) / half[1], (p[2] - c[2]) / half[2]];
                                        let r = (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt().clamp(0.0, 1.0);
                                        let g = (0x40 as f32 + (0xFF - 0x40) as f32 * r).round() as u32;
                                        0xFF00_0000 | (g << 16) | (g << 8) | g
                                    })
                                    .collect()
                            } else {
                                vec![word; n]
                            };
                            if let Some(i) = s.decls.iter().position(|d| d.name() == N_COLOR0) {
                                s.elems[i] = Elem::Word(words);
                            } else {
                                use super::vstream::{Decl, T_COLOR};
                                let compress = s.compress_local3d.unwrap_or(false);
                                let mut items: Vec<(Decl, u32, Elem)> = s.decls.iter().zip(s.elems.iter()).map(|(d, e)| (d.clone(), d.stored_type(compress), e.clone())).collect();
                                items.push((Decl::with_stride(N_COLOR0, T_COLOR, 0, 0, 0), T_COLOR, Elem::Word(words)));
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
                    // TINY_TREE_UV1=atlas (default) | copy | const: the LIGHTMAP layout
                    // of the cards (see `tree_lightmap_uv1`)
                    let uv1_mode = std::env::var("TINY_TREE_UV1").unwrap_or_else(|_| "atlas".into());
                    let idx: Vec<u32> = v.index_buffer.as_ref().map(|ib| ib.indices.clone()).unwrap_or_default();
                    match uv1_mode.as_str() {
                        "atlas" => {
                            if let Some(charts) = tree_lightmap_uv1(s, &idx) {
                                m.notes.push(format!("lightmap uv1 atlas: {charts} charts over {} triangles ({} vertices)", idx.len() / 3, s.count));
                                if !out.stripped.contains(&"uv1=atlas") {
                                    out.stripped.push("uv1=atlas");
                                }
                            }
                        }
                        "const" => {
                            if let Some(i) = s.decls.iter().position(|d| d.name() == N_TEXCOORD0 + 1) {
                                s.elems[i] = Elem::Float2(vec![[0.5, 0.5]; s.count.max(0) as usize]);
                            }
                        }
                        _ => {}
                    }
                }
            }
            transform_visual(&mut v, &IDENTITY, scale)?;
            // TINY_TREE_NORMALS=model (default) | up | shell (probe, 2026-09-10): the
            // leaf cards' vertex normals as the model has them, all straight up, or
            // radial from the visual's box centre (the shell the vegetation shader
            // lights). The lightmapper takes its irradiance direction from them.
            let leaf = t.materials[e.material as usize].leaf;
            if leaf {
                // Default `shell` since 2026-09-10: with the model's own normals (a
                // mix, 0.2–0.5 shell-ness on the GreenCoast species) the crown had
                // no sun side at all; radial normals give it a lit side and a
                // shaded side under the same bake (lineups C and D, every species).
                let mode = std::env::var("TINY_TREE_NORMALS").unwrap_or_else(|_| "shell".into());
                if mode != "model" {
                    if let Some(main) = v.main.as_mut() {
                        let c = [main.bounding_box[0], main.bounding_box[1], main.bounding_box[2]];
                        if let Some(Node::VertexStream(s)) = main.vertex_streams.first_mut().and_then(|r| r.inline.as_deref_mut()) {
                            let pos: Vec<[f32; 3]> = match s.decls.iter().zip(s.elems.iter()).find(|(d, _)| d.name() == super::vstream::N_POSITION).map(|(_, e)| e) {
                                Some(Elem::Float3(p)) => p.clone(),
                                _ => Vec::new(),
                            };
                            if let Some(i) = s.decls.iter().position(|d| d.name() == N_NORMAL) {
                                let n = s.count.max(0) as usize;
                                let mut words = Vec::with_capacity(n);
                                for k in 0..n {
                                    let nrm = if mode == "up" {
                                        [0.0, 1.0, 0.0]
                                    } else {
                                        let p = pos.get(k).copied().unwrap_or(c);
                                        // radial from the crown centre, tilted up a little (a crown is lit from above)
                                        let d = [p[0] - c[0], p[1] - c[1] + 0.3 * main.bounding_box[4], p[2] - c[2]];
                                        let l = (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt();
                                        if l > 1e-4 { [d[0] / l, d[1] / l, d[2] / l] } else { [0.0, 1.0, 0.0] }
                                    };
                                    words.push(super::merged::dec3n_pack(nrm));
                                }
                                s.elems[i] = Elem::Word(words);
                                let tag = if mode == "up" { "normals=up" } else { "normals=shell" };
                                if !out.stripped.contains(&tag) {
                                    out.stripped.push(tag);
                                }
                            }
                        }
                    }
                }
            }
            // Leaf cards are seen from both sides. A shading model without a
            // two-sided variant gets its back faces as a second, reversed copy
            // of the index list (the normals stay the front ones — a lit back
            // face, not a dark one); TINY_TREE_LEAF_BACKFACES=0 leaves it.
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
            // TINY_TREE_DENSE=1 (probe, 2026-09-10): level 1's leaf cards ALSO draw at
            // level 0 (lod mask bits 0 and 1) — the LOD0+LOD1 union, a denser near crown
            let dense = std::env::var("TINY_TREE_DENSE").as_deref() == Ok("1") && leaf && pick.is_none() && bit == 1;
            let lod_mask = if pick.is_some() { 0 } else { (1 << bit) | if dense { 1 } else { 0 } };
            if dense && !out.stripped.contains(&"dense") {
                out.stripped.push("dense");
            }
            let bands = if leaf { depth_bands() } else { None };
            match bands {
                Some(bands) if !band_slots_per_mat[e.material as usize].is_empty() => {
                    // per triangle: the normalised radius of its centroid about the
                    // visual's box centre (elliptical, by the box half extents)
                    let (c, half) = v.main.as_ref().map(|mn| ([mn.bounding_box[0], mn.bounding_box[1], mn.bounding_box[2]], [mn.bounding_box[3].max(0.01), mn.bounding_box[4].max(0.01), mn.bounding_box[5].max(0.01)])).unwrap_or(([0.0; 3], [1.0; 3]));
                    let pos: Vec<[f32; 3]> = v.stream().and_then(|s| s.decls.iter().zip(s.elems.iter()).find(|(d, _)| d.name() == super::vstream::N_POSITION).map(|(_, el)| el.clone())).and_then(|el| if let Elem::Float3(p) = el { Some(p) } else { None }).unwrap_or_default();
                    let idx: Vec<u32> = v.index_buffer.as_ref().map(|ib| ib.indices.clone()).unwrap_or_default();
                    let ntri = idx.len() / 3;
                    let radius: Vec<f32> = (0..ntri)
                        .map(|ti| {
                            let mut r = 0.0f32;
                            for k in 0..3 {
                                let p = pos.get(idx[ti * 3 + k] as usize).copied().unwrap_or(c);
                                let d = [(p[0] - c[0]) / half[0], (p[1] - c[1]) / half[1], (p[2] - c[2]) / half[2]];
                                r += (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt() / 3.0;
                            }
                            r
                        })
                        .collect();
                    // band b takes the triangles with thr[b-1] <= r < thr[b]; the last (outer) band the rest
                    let mut lo = 0.0f32;
                    let mut counts: Vec<usize> = Vec::new();
                    for (bi, (thr, _)) in bands.iter().enumerate() {
                        let keep: Vec<bool> = radius.iter().map(|r| *r >= lo && *r < *thr).collect();
                        let kept = keep.iter().filter(|k| **k).count();
                        counts.push(kept);
                        if kept > 0 {
                            let sv = super::merged::sub_visual(&v, &keep)?;
                            m.visuals.push(MergedVisual { visual: sv, material: band_slots_per_mat[e.material as usize][bi], lod_mask, lod_ladder: ladder.clone(), part: 0 });
                            n += 1;
                        }
                        lo = *thr;
                    }
                    let keep: Vec<bool> = radius.iter().map(|r| *r >= lo).collect();
                    let kept = keep.iter().filter(|k| **k).count();
                    counts.push(kept);
                    if kept > 0 {
                        let sv = super::merged::sub_visual(&v, &keep)?;
                        m.visuals.push(MergedVisual { visual: sv, material: slots[e.material as usize], lod_mask, lod_ladder: ladder.clone(), part: 0 });
                        n += 1;
                    }
                    m.notes.push(format!("depth bands level {l}: {ntri} triangles per band (inner..outer) {counts:?}"));
                    if !out.stripped.contains(&"depth") {
                        out.stripped.push("depth");
                    }
                }
                _ => {
                    m.visuals.push(MergedVisual { visual: v, material: slots[e.material as usize], lod_mask, lod_ladder: ladder.clone(), part: 0 });
                    n += 1;
                }
            }
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
    // TINY_TREE_PRELIGHT_U02=F (probe, 2026-09-10): the Solid2's PreLightGen
    // scale word — metres per lightmap uv unit, the size the lightmapper takes
    // the item's atlas to be and so the texel budget it gets (the default
    // 32.14 is the reference items'; a tree's charts cover ~1 uv² for ~500 m²
    // of cards). A bigger word = more texels per card.
    if let Some(u02) = std::env::var("TINY_TREE_PRELIGHT_U02").ok().and_then(|v| v.parse::<f32>().ok()) {
        m.pre_light_gen = Some(super::solid2::PreLightGen { version: 1, u01: 1, u02, u03: true, u04: [0.001, 0.001, 0.99712694, 0.999, f32::MAX, f32::MAX, f32::MIN, f32::MIN], sprite_count: [0, 0], boxes: Vec::new(), uv_groups: Vec::new() });
        m.notes.push(format!("prelight u02 = {u02}"));
    }
    if std::env::var("TINY_TREE_NO_PRELIGHT").as_deref() == Ok("1") {
        m.no_prelight = true;
        m.notes.push("no PreLightGen (probe)".into());
    }
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
thread_local! {
    /// The map collection the tree being baked is for (set by
    /// `static_item_from_veget_report`): the per-collection leaf colour
    /// calibration keys on it.
    pub static VEGET_COLLECTION: std::cell::Cell<u32> = const { std::cell::Cell::new(26) };
}

/// The per-collection LEAF COLOUR calibration (colour gain, saturation
/// factor, hue shift in degrees — negative = towards yellow), measured on
/// 2026-09-10 against the stock species standing beside ours at the same
/// angular size (lineups D–G: GreenCoast TreeSmallA/BushMediumD/TreeThinSmallA/
/// TreeBigA/BushBigB, BlueBay PalmTreeBigB1/BigA1/SugarBigA/BushBigA,
/// RedIsland TreePineBigA2/MediumA1/BushBigA, Stadium PalmTreeMedium/Small/
/// SpringTreeBig; back-lit and sun sides, 4K, `cropstats --fg green`). The
/// stock's back-lit crown is a dark green mass with bright yellow-green rim
/// highlights; ours under the gain-1.3 shell-normal bake came out brighter
/// than the stock on GreenCoast (+10–30 %) and RedIsland (+20 %), about right
/// on BlueBay and Stadium (−10 %), and LESS saturated everywhere but BlueBay
/// (RedIsland pines 15 % vs 30 %, GreenCoast oaks 20 % vs 27 %), greener by
/// 5–10° on the sun side. `TINY_TREE_LEAF_COLOR_ADJ=GAIN[,SAT[,HUE]]`
/// overrides the table.
pub fn leaf_color_for(collection: u32) -> (f32, f32, f32) {
    let (g, s, h, _) = leaf_look_for(collection);
    (g, s, h)
}

/// The per-collection LEAF LOOK: (colour gain, saturation, hue shift, depth
/// bands) — the pass-3 default (2026-09-10 15:30Z), fitted on the sky-backed
/// lineups M/N/O (source-19/06/17 hosts at y 300, `cropstats --fg leaf`, the
/// centre crop, stock at 30 m beside ours at 15 m = the same angular size, sun
/// AND shade side) and the Stadium lineup L. Depth bands = the self-shadow
/// (`depth_bands`): three for the GreenCoast crowns (r < 0.45 ×0.45, r < 0.75
/// ×0.7, the rest as is — the shade side of every species lands within 3 % of
/// the stock where the plain bake was +20–50 %), one mild band for the pines,
/// firs and palms. The gains are the table's own after the bands (they darken
/// the mean too). Fit residuals, luma, sun/shade: GreenCoast −5/+3, +4/−2,
/// +3/+15 (TreeSmallA, TreeBigA, BushBigB); BlueBay −7/0, +13/+2, +29/+14 (the
/// jungle bush high); RedIsland −7/+2, +9/+2, −8/−9; Stadium (y-150 lineup)
/// −3/+9, −14/+4, −4/+10. Hue: our shade side renders 10–25° bluer-green than
/// the stock (the sky lights it), the shift is the compromise between the two
/// sides. `TINY_TREE_LEAF_COLOR_ADJ` / `TINY_TREE_DEPTH` override a field each;
/// `TINY_TREE_COLOR_TABLE=0` restores the pass-2 rule (ship16's bytes).
pub fn leaf_look_for(collection: u32) -> (f32, f32, f32, &'static str) {
    match collection {
        0xf => (1.3, 1.6, -25.0, "0.45:0.45,0.75:0.7"),  // GreenCoast
        0x1c => (1.12, 1.06, 0.0, "0.6:0.5"),            // BlueBay
        0x10 => (1.15, 1.9, -15.0, "0.55:0.55"),         // RedIsland
        0x1d => (1.1, 1.5, -10.0, "0.55:0.55"),          // WhiteShore (firs: the pines' setting, unmeasured)
        _ => (1.2, 1.1, 0.0, "0.6:0.7"),                 // Stadium (the palms want the band, the spring crown less of it)
    }
}

/// Whether the pass-3 per-collection table is in force (default yes;
/// `TINY_TREE_COLOR_TABLE=0` = the pass-2 rule).
pub fn color_table_on() -> bool {
    std::env::var("TINY_TREE_COLOR_TABLE").map(|v| v != "0").unwrap_or(true)
}

pub fn static_item_from_veget_report(store: &mut crate::store::DataStore, path: &str, ident: &str, author: &str, scale: f32, collection: u32) -> R<(Vec<u8>, Merged, VegetBake)> {
    VEGET_COLLECTION.with(|c| c.set(collection));
    let model_path = crate::veget::tree_model_path(store, path)?;
    let mut m = Merged::default();
    m.keep_water = keep_water_for(collection);
    let bake = add_veget_tree_model(store, &model_path, scale, &mut m)?;
    if std::env::var_os("TINY_TREE_KINEMATIC").is_some() {
        veget_as_kinematic(store, &mut m, scale)?;
    }
    let opts = BuildOpts { ident: ident.to_string(), author: author.to_string(), scale, collection, skin: None };
    let file = assemble(&m, &opts)?;
    Ok((super::file::write_file(&file), m, bake))
}

/// TINY_TREE_KINEMATIC=1 (probe, 2026-09-10): the baked tree as a KINEMATIC
/// dyna entity instead of a static object — the visuals and materials move
/// into a `CPlugDynaObjectModel` (the pusher piston's model bytes as the
/// template, the trunk hull as both its shapes), the entity kinematic and
/// bound by the pack's ZERO-range pusher constraint, like our half-size
/// pistons that the game animates and LIGHTS DYNAMICALLY. Why: a static
/// item's leaves are lit by the lightmap the game bakes at load, and that
/// bake treats every card as an opaque occluder — the crown comes out lit by
/// the sky alone, flat and blue-grey, no sun side (lineup trA, TreeSmallA at 5
/// and 15 m against the stock: no lit side at all). A dyna entity is lit per
/// pixel with the sun. The item's static part keeps nothing but the
/// collision, which stays the trunk hull.
pub fn veget_as_kinematic(store: &mut crate::store::DataStore, m: &mut Merged, scale: f32) -> R<()> {
    const PISTON: &str = "Stadium\\Media\\Dyna\\ObstaclePusher\\ObstaclePusher8mPiston.DynaObject.Gbx";
    const CONSTRAINT: &str = "Stadium\\Media\\KinematicConstraints\\ObstaclePusher8m.KinematicConstraint.Gbx";
    let mut scratch = Merged::default();
    let src = load_dyna_source(store, PISTON, &mut scratch, false)?;
    let kmodel = store.load_model(CONSTRAINT)?;
    let constraint = super::dyna::KinematicConstraint::parse_body(&kmodel.body).map_err(|e| format!("{CONSTRAINT}: {e}"))?;
    // the tree's own hull (the trunk, Wood) as the moving and the static shape
    // (the accumulator's surface arrays are already in the item frame, scaled)
    let hull: Option<CPlugSurface> = if m.surf_vertices.is_empty() { None } else { Some(CPlugSurface::mesh(m.surf_vertices.clone(), m.surf_triangles.clone(), m.surf_ids.clone(), [0.0, 0.0, 1.0])) };
    let Some(hull) = hull else { return Err("kinematic tree: the species has no hull to move".into()) };
    let mut mesh = Merged::default();
    mesh.keep_water = m.keep_water;
    mesh.no_split = true;
    mesh.visuals = std::mem::take(&mut m.visuals);
    mesh.materials = std::mem::take(&mut m.materials);
    mesh.lod_max_dist = std::mem::take(&mut m.lod_max_dist);
    mesh.all_lods = m.all_lods;
    // `NPlugDynaObjectModel_SInstanceParams` v2: PeriodSc 1, TextureId 0,
    // IsKinematic 1, PeriodScMax 1, Phase01/Max -1 (unset), CastStaticShadow 1
    let mut params = Vec::with_capacity(32);
    for w in [2u32, 1.0f32.to_bits(), 0, 1, 1.0f32.to_bits(), (-1.0f32).to_bits(), (-1.0f32).to_bits(), std::env::var("TINY_TREE_KINEMATIC_SHADOW").ok().and_then(|v| v.parse().ok()).unwrap_or(1u32)] {
        params.extend_from_slice(&w.to_le_bytes());
    }
    let cparams = super::dyna::ConstraintParams { version: 0, ent1: -1, ent2: 0, pos1: [0.0; 3], pos2: [0.0; 3] };
    m.notes.push(format!("kinematic tree: {} visuals, {} materials as a dyna entity over the trunk hull, zero-range constraint [{}]", mesh.visuals.len(), mesh.materials.len(), constraint.summary()));
    // TINY_TREE_KINEMATIC=file: the dyna object and its mesh as two sidecar FILES
    // next to the item (the flag probes' form) — the mesh file's own folder is then
    // where its custom textures are looked up
    let pack_ref = if std::env::var("TINY_TREE_KINEMATIC").as_deref() == Ok("file") { Some(super::merged::PackRef::File) } else { None };
    m.dyna.push(DynaPart { path: PISTON.to_string(), rot: [0.0, 0.0, 0.0, 1.0], pos: [0.0; 3], mesh, move_shape: Some(hull.clone()), hit_shape: Some(hull), model: src.model.clone(), instance_params_id: super::dyna::P_DYNA_INSTANCE, instance_params: params, constraint: Some((constraint, cparams)), pack_ref });
    let _ = scale;
    Ok(())
}

/// The LIGHTMAP layout (TexCoord1) of a vegetation visual: every connected
/// group of triangles — a leaf card, a bark strip — is one chart, projected
/// on its own plane and packed into its own cell of a square grid over the
/// unit square (a margin of a tenth of the cell on every side), so no two
/// charts share a lightmap texel.
///
/// Why (2026-09-09, the "paper lantern" bushes of tiny 24's cp8 and vjeux's
/// "the bright spots look completely white"): `ensure_texcoord1` gave the
/// cards uv0 as their uv1, i.e. every card of the crown mapped onto the SAME
/// lightmap texels (the leaf atlas's 0..1). The game bakes a lightmap for
/// every static item at load and the shader multiplies the diffuse by it —
/// with all the cards stacked on one region the bake handed the whole crown
/// one arbitrary value: at night a bush stood sunlit-bright next to a black
/// lawn (lineup `bl`, no light within 40 m), under the show rigs the rigs'
/// baked light covered every leaf. The intensity of the lights themselves
/// was never the lever (×0.25 and ×0.01 on a stock Lamp changed the bush
/// nothing). Returns None when the stream has no Float3 positions (the
/// caller keeps the uv0 copy).
pub fn tree_lightmap_uv1(s: &mut super::vstream::CPlugVertexStream, indices: &[u32]) -> Option<usize> {
    let n = s.count.max(0) as usize;
    let pos: Vec<[f32; 3]> = match s.decls.iter().zip(s.elems.iter()).find(|(d, _)| d.name() == super::vstream::N_POSITION).map(|(_, e)| e)? {
        Elem::Float3(p) => p.clone(),
        _ => return None,
    };
    if n == 0 || pos.len() != n || indices.len() < 3 {
        return None;
    }
    // connected components of vertices through the triangles (union-find)
    let mut parent: Vec<usize> = (0..n).collect();
    fn find(p: &mut [usize], mut i: usize) -> usize {
        while p[i] != i {
            p[i] = p[p[i]];
            i = p[i];
        }
        i
    }
    for t in indices.chunks_exact(3) {
        let (a, b, c) = (t[0] as usize, t[1] as usize, t[2] as usize);
        if a >= n || b >= n || c >= n {
            continue;
        }
        let ra = find(&mut parent, a);
        let rb = find(&mut parent, b);
        if ra != rb {
            parent[ra] = rb;
        }
        let rb = find(&mut parent, b);
        let rc = find(&mut parent, c);
        if rb != rc {
            parent[rb] = rc;
        }
    }
    // component id per vertex, in order of first appearance
    let mut comp_of_root: std::collections::BTreeMap<usize, usize> = Default::default();
    let mut comp: Vec<usize> = vec![0; n];
    for i in 0..n {
        let r = find(&mut parent, i);
        let next = comp_of_root.len();
        let c = *comp_of_root.entry(r).or_insert(next);
        comp[i] = c;
    }
    let ncomp = comp_of_root.len();
    // per component: an area-weighted normal (from its triangles), then a
    // planar frame and the projected bounds
    let mut normal: Vec<[f64; 3]> = vec![[0.0; 3]; ncomp];
    for t in indices.chunks_exact(3) {
        let (a, b, c) = (t[0] as usize, t[1] as usize, t[2] as usize);
        if a >= n || b >= n || c >= n {
            continue;
        }
        let e1 = [pos[b][0] - pos[a][0], pos[b][1] - pos[a][1], pos[b][2] - pos[a][2]];
        let e2 = [pos[c][0] - pos[a][0], pos[c][1] - pos[a][1], pos[c][2] - pos[a][2]];
        let cr = [e1[1] * e2[2] - e1[2] * e2[1], e1[2] * e2[0] - e1[0] * e2[2], e1[0] * e2[1] - e1[1] * e2[0]];
        let k = comp[a];
        // orient consistently against the running sum (a two-sided card's
        // front and back triangles would otherwise cancel)
        let dot = normal[k][0] * cr[0] as f64 + normal[k][1] * cr[1] as f64 + normal[k][2] * cr[2] as f64;
        let sgn = if dot < 0.0 { -1.0 } else { 1.0 };
        for d in 0..3 {
            normal[k][d] += sgn * cr[d] as f64;
        }
    }
    let mut frames: Vec<([f64; 3], [f64; 3])> = Vec::with_capacity(ncomp);
    for nrm in &normal {
        let len = (nrm[0] * nrm[0] + nrm[1] * nrm[1] + nrm[2] * nrm[2]).sqrt();
        let nn = if len > 1e-12 { [nrm[0] / len, nrm[1] / len, nrm[2] / len] } else { [0.0, 1.0, 0.0] };
        // ex = the world axis least aligned with the normal, made orthogonal
        let ax = if nn[0].abs() <= nn[1].abs() && nn[0].abs() <= nn[2].abs() { [1.0, 0.0, 0.0] } else if nn[1].abs() <= nn[2].abs() { [0.0, 1.0, 0.0] } else { [0.0, 0.0, 1.0] };
        let d = ax[0] * nn[0] + ax[1] * nn[1] + ax[2] * nn[2];
        let mut ex = [ax[0] - d * nn[0], ax[1] - d * nn[1], ax[2] - d * nn[2]];
        let l = (ex[0] * ex[0] + ex[1] * ex[1] + ex[2] * ex[2]).sqrt().max(1e-12);
        ex = [ex[0] / l, ex[1] / l, ex[2] / l];
        let ey = [nn[1] * ex[2] - nn[2] * ex[1], nn[2] * ex[0] - nn[0] * ex[2], nn[0] * ex[1] - nn[1] * ex[0]];
        frames.push((ex, ey));
    }
    let mut proj: Vec<[f64; 2]> = vec![[0.0; 2]; n];
    let mut lo: Vec<[f64; 2]> = vec![[f64::MAX; 2]; ncomp];
    let mut hi: Vec<[f64; 2]> = vec![[f64::MIN; 2]; ncomp];
    for i in 0..n {
        let (ex, ey) = frames[comp[i]];
        let p = [pos[i][0] as f64, pos[i][1] as f64, pos[i][2] as f64];
        let u = p[0] * ex[0] + p[1] * ex[1] + p[2] * ex[2];
        let v = p[0] * ey[0] + p[1] * ey[1] + p[2] * ey[2];
        proj[i] = [u, v];
        let k = comp[i];
        lo[k] = [lo[k][0].min(u), lo[k][1].min(v)];
        hi[k] = [hi[k][0].max(u), hi[k][1].max(v)];
    }
    // the grid: cell = 1/N, the chart fills the cell minus a margin, uniform
    // scale (the larger side fits)
    let grid = (ncomp as f64).sqrt().ceil().max(1.0);
    let cell = 1.0 / grid;
    let margin = cell * 0.1;
    let inner = cell - 2.0 * margin;
    let mut uv1: Vec<[f32; 2]> = vec![[0.5, 0.5]; n];
    for i in 0..n {
        let k = comp[i];
        let w = (hi[k][0] - lo[k][0]).max(1e-9);
        let h = (hi[k][1] - lo[k][1]).max(1e-9);
        let sc = inner / w.max(h);
        let cx = (k as f64 % grid) * cell + margin;
        let cy = (k as f64 / grid).floor() * cell + margin;
        // centred in the cell along the shorter side
        let ox = (inner - w * sc) * 0.5;
        let oy = (inner - h * sc) * 0.5;
        let u = cx + ox + (proj[i][0] - lo[k][0]) * sc;
        let v = cy + oy + (proj[i][1] - lo[k][1]) * sc;
        uv1[i] = [u.clamp(0.0, 1.0) as f32, v.clamp(0.0, 1.0) as f32];
    }
    let slot = s.decls.iter().position(|d| d.name() == N_TEXCOORD0 + 1)?;
    s.elems[slot] = Elem::Float2(uv1);
    Some(ncomp)
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

#[cfg(test)]
mod tree_uv1_tests {
    use super::super::vstream::{CPlugVertexStream, Decl, Elem, N_POSITION, N_TEXCOORD0, T_FLOAT2, T_FLOAT3};
    use super::super::null_ref;

    /// Three leaf cards (two triangles, four vertices each) in three planes:
    /// every card gets its own grid cell, no two cards share lightmap space,
    /// and every uv1 stays inside the unit square.
    #[test]
    fn cards_get_disjoint_lightmap_cells() {
        let mut pos: Vec<[f32; 3]> = Vec::new();
        let mut idx: Vec<u32> = Vec::new();
        for (k, (ex, ey)) in [([1.0f32, 0.0, 0.0], [0.0f32, 1.0, 0.0]), ([0.0, 1.0, 0.0], [0.0, 0.0, 1.0]), ([1.0, 0.0, 1.0], [0.0, 1.0, 0.0])].iter().enumerate() {
            let o = [k as f32 * 3.0, 0.0, 0.0];
            let b = pos.len() as u32;
            for (a, c) in [(0.0f32, 0.0f32), (1.0, 0.0), (1.0, 2.0), (0.0, 2.0)] {
                pos.push([o[0] + a * ex[0] + c * ey[0], o[1] + a * ex[1] + c * ey[1], o[2] + a * ex[2] + c * ey[2]]);
            }
            idx.extend_from_slice(&[b, b + 1, b + 2, b, b + 2, b + 3]);
        }
        let n = pos.len();
        let uv0: Vec<[f32; 2]> = (0..n).map(|i| [(i % 4) as f32 * 0.5, (i / 4) as f32 * 0.1]).collect();
        let mut s = CPlugVertexStream {
            version: 1,
            count: n as i32,
            flags: 0,
            base: null_ref(),
            decls: vec![Decl::with_stride(N_POSITION, T_FLOAT3, 0, 0, 7), Decl::with_stride(N_TEXCOORD0, T_FLOAT2, 0, 12, 7), Decl::with_stride(N_TEXCOORD0 + 1, T_FLOAT2, 0, 20, 7)],
            compress_local3d: Some(false),
            elems: vec![Elem::Float3(pos), Elem::Float2(uv0.clone()), Elem::Float2(uv0)],
        };
        assert_eq!(super::tree_lightmap_uv1(&mut s, &idx), Some(3));
        let Elem::Float2(uv1) = &s.elems[2] else { panic!("uv1 gone") };
        assert_eq!(uv1.len(), n);
        // grid of 2x2 cells (ceil(sqrt(3)) = 2): each card's four corners
        // inside one cell, cells distinct
        let cell_of = |uv: [f32; 2]| ((uv[0] * 2.0).floor() as i32, (uv[1] * 2.0).floor() as i32);
        let mut cells = Vec::new();
        for k in 0..3 {
            let c = cell_of(uv1[k * 4]);
            for j in 0..4 {
                let uv = uv1[k * 4 + j];
                assert!((0.0..=1.0).contains(&uv[0]) && (0.0..=1.0).contains(&uv[1]), "uv1 {uv:?} outside the unit square");
                assert_eq!(cell_of(uv), c, "card {k} corner {j} left its cell");
            }
            assert!(!cells.contains(&c), "two cards in one cell");
            cells.push(c);
        }
        // the 1 x 2 card keeps its aspect: the long side spans the cell's inner
        // 80 %, the short one 40 %
        let w = (uv1[0][0] - uv1[1][0]).abs().max((uv1[0][1] - uv1[1][1]).abs());
        let h = (uv1[1][0] - uv1[2][0]).abs().max((uv1[1][1] - uv1[2][1]).abs());
        assert!((h - 0.4).abs() < 1e-3 && (w - 0.2).abs() < 1e-3, "card 0 spans {w} x {h}");
    }
}

/// `TINY_SCREENS=logo`: the picture every ad screen face shows — the pack's
/// `RaceAd6x1` default (the TRACKMANIA wordmark on its LED band), as a 32-bit
/// DDS, rows flipped like the gate sign pictures (the display samples the
/// panel uv V-flipped against a plain texture). One picture per item that has
/// an ad face; the same file name in every item, so the game caches it once.
pub fn add_screen_logo_pictures(store: &mut crate::store::DataStore, m: &mut Merged) {
    if screen_mode() != ScreenMode::Logo {
        return;
    }
    if !m.materials.iter().any(|mat| mat.link().map(is_ad_screen_link).unwrap_or(false)) {
        return;
    }
    if m.pictures.iter().any(|(f, _)| f == SCREEN_LOGO_FILE) {
        return;
    }
    let path = "Stadium\\Media\\Texture\\Image\\RaceAd6x1.dds";
    match store.read(path).map_err(|e| format!("{path}: {e}")).and_then(|bytes| super::texture::decode_capped_rgba(&bytes, 1024).map_err(|e| format!("{path}: {e}"))) {
        Ok((w, h, rgba)) => {
            let mut out = Vec::with_capacity(rgba.len());
            for row in (0..h as usize).rev() {
                let r = &rgba[row * w as usize * 4..(row + 1) * w as usize * 4];
                for px in r.chunks(4) {
                    out.extend_from_slice(&[px[0], px[1], px[2], 0xFF]);
                }
            }
            m.notes.push(format!("screen logo picture: {SCREEN_LOGO_FILE} {w}x{h} from {path}"));
            m.pictures.push((SCREEN_LOGO_FILE.to_string(), super::texture::write_dds_rgba(w, h, &out)));
        }
        Err(e) => m.notes.push(format!("screen logo picture: {e}; game material kept")),
    }
}

/// `TINY_TRIGGERFX`: `off` drops every visual under a `Modifier\<Kind>\TriggerFX`
/// material (the gate curtain that draws as a checkerboard in an item);
/// `picture` extracts the pak's `TriggerFX<Kind>_I.dds` (capped at 512) as the
/// icon picture the custom material draws. See `materials::trigger_fx_material`.
pub fn trigger_fx_pass(store: &mut crate::store::DataStore, m: &mut Merged) {
    let mode = trigger_fx_mode();
    if mode == "game" {
        return;
    }
    let fx_slots: Vec<usize> = m.materials.iter().enumerate().filter(|(_, mat)| mat.link().and_then(trigger_fx_kind).is_some()).map(|(i, _)| i).collect();
    if fx_slots.is_empty() {
        return;
    }
    if mode == "picture" {
        for slot in &fx_slots {
            let Some(kind) = m.materials[*slot].link().and_then(trigger_fx_kind) else { continue };
            let file = trigger_fx_file(&kind);
            if m.pictures.iter().any(|(f, _)| *f == file) {
                continue;
            }
            let path = format!("Stadium\\Media\\Texture\\Image\\TriggerFX{kind}_I.dds");
            match store.read(&path).map_err(|e| format!("{path}: {e}")).and_then(|b| super::texture::dds_cap(&b, 512).map_err(|e| format!("{path}: {e}"))) {
                Ok(dds) => {
                    m.notes.push(format!("trigger FX picture {file} ({} bytes) from {path}", dds.len()));
                    m.pictures.push((file, dds));
                }
                Err(e) => m.notes.push(format!("trigger FX picture: {e}; game material kept")),
            }
        }
        return;
    }
    let before = m.visuals.len();
    m.visuals.retain(|v| !fx_slots.contains(&v.material));
    let n = before - m.visuals.len();
    if n > 0 {
        m.notes.push(format!("{n} trigger FX curtain visual(s) dropped (TINY_TRIGGERFX=off: the FuncShader-driven icon draws as a checkerboard in an item)"));
    }
}
