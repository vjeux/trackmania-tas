//! From the accumulator to the item file: the merged visuals and materials
//! as one `CPlugSolid2Model` (`build_solid2`), the collision as one
//! `CPlugSurface` (`build_surface`), and the whole `CGameItemModel` tree in
//! the reference `.Item.Gbx` layout (`assemble`, `header_chunks`).

use super::lod::{cap_lod_ladder, lod0_only, remap_lod_mask, MAX_LOD_LEVELS};
use super::materials::{custom_texture_material, light_skin_material, sign_logo_material, skinned_material};
use super::merged::{coalesce, harmonize_layouts, Merged};
use super::build::fx_entities;
use super::solid2::{CPlugSolid2Model, Material, PreLightGen, ShadedGeom};
use super::surface::CPlugSurface;
use super::{Node, NodeRef, Ref, R};
use crate::geom::IDENTITY;

/// What the built item is called and how big it is.
#[derive(Clone, Debug)]
pub struct BuildOpts {
    /// `Ident` path, e.g. `ZZZ_TinyBlocks\Tiny\Road\Tiny_Road_01.Item.Gbx`.
    pub ident: String,
    pub author: String,
    pub scale: f32,
    /// Collection id (26 = Stadium).
    pub collection: u32,
    /// The source model's CPlugGameSkin HEADER chunk (0x090F4000), copied
    /// verbatim: the declaration (`Any\Advertisement6x1\`, `*Image` slot)
    /// that makes the game paint the current in-game advertisement — the
    /// campaign artwork — onto the model's `Image` texture, and lets a
    /// placement's own skin file (a light colour) apply. Without it a screen
    /// draws the material's default yellow `RaceAd6x1.                // TINY_TREE_TEX_FORMAT=dds|rgba|leaf-rgba (default dds): the pack's
                // block-compressed image with its mips, cut to the cap — a DXT5
                // leaf atlas alpha-tests fine under TDSN once the visual has its
                // TexCoord1 (2026-09-08 probe G: DXT5 and uncompressed crowns
                // identical). `leaf-rgba` ships the leaf images UNCOMPRESSED 32-bit
                // at the capped level (three times the bytes), `rgba` everything.` panel.
    pub skin: Option<Vec<u8>>,
}

pub fn inline(index: i32, node: Node) -> Ref {
    NodeRef { index, inline: Some(Box::new(node)) }
}

/// The reference items' PreLightGen (every one of the 26 carries these).
fn default_prelight() -> PreLightGen {
    PreLightGen {
        version: 1,
        u01: 1,
        u02: 32.14457,
        u03: true,
        u04: [0.001, 0.001, 0.99712694, 0.999, f32::MAX, f32::MAX, f32::MIN, f32::MIN],
        sprite_count: [0, 0],
        boxes: Vec::new(),
        uv_groups: Vec::new(),
    }
}

/// `CGameItemPlacementParam` as the reference items carry it: chunks
/// 2E020000 (grid snap 1 m, fly step 1 m, pivot snap -1), 001 (no pivots), 004,
/// 005 (an inline `NPlugItemPlacement_SClass` node at `sclass_index`).
pub fn placement_param(sclass_index: i32) -> super::item::CGameItemPlacementParam {
    let mut p0 = Vec::new();
    p0.extend_from_slice(&0u32.to_le_bytes()); // version
    p0.extend_from_slice(&1u16.to_le_bytes()); // flags
    // cube center, cube size 0, grid snap h 1 m / v 0, offsets 0, fly v step
    // 1 m, fly v offset 0, pivot snap distance -1
    for f in [0.0f32, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0, 0.0, -1.0] {
        p0.extend_from_slice(&f.to_le_bytes());
    }
    let mut p5 = Vec::new();
    p5.extend_from_slice(&sclass_index.to_le_bytes());
    p5.extend_from_slice(&0x09187000u32.to_le_bytes());
    for w in [10u32, 0xFFFF_FFFF, 0, 0, 1, 0, 0, 0, 0x3F80_0000, 0, 0] {
        p5.extend_from_slice(&w.to_le_bytes());
    }
    super::item::CGameItemPlacementParam {
        chunks: vec![
            super::RawChunk { id: 0x2E020000, payload: p0 },
            super::RawChunk { id: 0x2E020001, payload: vec![0; 8] },
            super::RawChunk { id: 0x2E020004, payload: vec![0; 8] },
            super::RawChunk { id: 0x2E020005, payload: p5 },
        ],
    }
}

thread_local! {
    /// External files the item names (node index, pack path) — its reference
    /// table: the effect systems' particle textures (`fx_entities`), which
    /// the game resolves by pack path. (A light's `.Light.Gbx` or bitmaps by
    /// path were probed on 2026-09-07 and resolve to NOTHING — every such light
    /// was dark — so nothing else goes here.)
    pub static EXTERNALS: std::cell::RefCell<Vec<(u32, String)>> = const { std::cell::RefCell::new(Vec::new()) };
    /// Sidecar files an item form writes next to the item (`TINY_FLAG_REF=file`:
    /// the dyna object and its mesh as files): (bare file name, bytes). The
    /// `static-item` command writes them into the --out directory.
    pub static SIDECARS: std::cell::RefCell<Vec<(String, Vec<u8>)>> = const { std::cell::RefCell::new(Vec::new()) };
}

/// The merged visuals + materials as one `CPlugSolid2Model`, node indices
/// taken from `next` (visual, its stream, then the materials).
pub fn build_solid2(m: &Merged, opts: &BuildOpts, next: &mut i32) -> R<CPlugSolid2Model> {
    if m.visuals.is_empty() {
        return Err("no visuals: nothing to build".into());
    }
    let mut s2 = CPlugSolid2Model::new_v34();
    if let Some(t) = m.vis_cst_type {
        s2.vis_cst_type = t;
    }
    if let Some(u) = m.solid2_u07 {
        s2.u07 = u;
    }
    let mut pre = m.visuals.clone();
    // The item's detail ladder (see `Merged::lod_max_dist`): the union of the
    // parts' distances, sorted; every visual's mask moved onto it by range
    // (`remap_lod_mask`). With a `--lod-pick` nothing but one level was
    // merged and the ladder is empty. A tween mesh (`all_lods`) keeps the
    // pack's ladder whole — five levels are what its draw path expects.
    let mut ladder: Vec<f32> = if lod0_only() && !m.all_lods { Vec::new() } else { m.lod_max_dist.clone() };
    if !m.all_lods {
        cap_lod_ladder(&mut ladder, MAX_LOD_LEVELS - 1);
    }
    for mv in pre.iter_mut() {
        mv.lod_mask = remap_lod_mask(mv.lod_mask, &mv.lod_ladder, &ladder);
        mv.lod_ladder = ladder.clone();
    }
    // a level the cap folded away leaves its geoms with no bit: never drawn,
    // so not written
    let dropped = pre.iter().filter(|mv| mv.lod_mask == 0).count();
    if dropped > 0 {
        pre.retain(|mv| mv.lod_mask != 0);
        if pre.is_empty() {
            return Err("every visual fell off the capped detail ladder".into());
        }
    }
    harmonize_layouts(&mut pre);
    let visuals = coalesce(&pre);
    // Only the materials some visual draws with, in first-use order (the
    // reference items list exactly one material per visual).
    let mut used: Vec<usize> = Vec::new();
    for mv in &visuals {
        if !used.contains(&mv.material) {
            used.push(mv.material);
        }
    }
    // Geom order. Nadeo's prefabs are LEVEL-major: every level-0 geom, then
    // every level-1 geom, ... (GateArchCenterCheckpoint24m: lod1 x6, lod2
    // x6, lod4 x5, lod8 x4, the materials in no particular order inside a
    // level), so same-material geoms next to each other are always of one
    // level — whatever draw merging the client does on a run of one material
    // (the FORMAT-RULES declaration rule) never fuses two levels. Inside a
    // level the geoms are material-sorted: every item that ever loaded here
    // is non-decreasing in material index (the first split terrain items
    // with (0,1,2,0,1,2,3,4) crashed the client at 0x140456507 reading a
    // garbage material index, 2026-09-06), and a one-level item keeps
    // exactly that order. Stable, so same-material same-level visuals keep
    // their relative order.
    let mut visuals = visuals;
    let level_of = |mask: u32| -> u32 { if mask == 0 { 0 } else { mask.trailing_zeros() } };
    visuals.sort_by_key(|mv| (level_of(mv.lod_mask), used.iter().position(|u| *u == mv.material).unwrap_or(usize::MAX), mv.lod_mask));
    for mv in &visuals {
        let mut v = mv.visual.clone();
        let main = v.main.as_mut().unwrap();
        // the stream sits right after its visual (an inline-form visual has
        // none and takes one index)
        let mut has_stream = false;
        for r in main.vertex_streams.iter_mut() {
            if r.inline.is_some() && !v.inline_form {
                r.index = *next + 1;
                has_stream = true;
            }
        }
        let material_index = used.iter().position(|u| *u == mv.material).unwrap() as i32;
        s2.shaded_geoms.push(ShadedGeom { visual_index: s2.visuals.len() as i32, material_index, u01: -1, lod_mask: mv.lod_mask as i32, u02: 0 });
        s2.visuals.push(inline(*next, Node::Visual(v)));
        *next += if has_stream { 2 } else { 1 };
    }
    s2.lod_max_dist = ladder;
    if let Some(u) = m.solid2_u13 {
        s2.u13 = u;
    }
    for inst in used.iter().map(|u| &m.materials[*u]) {
        let inst = skinned_material(inst, opts.collection);
        let inst = custom_texture_material(&inst, &opts.ident);
        let inst = sign_logo_material(&inst, m);
        let inst = light_skin_material(&inst, m);
        if m.materials_external {
            // the pack mesh's form: the material is a FILE the reference table
            // names (`Stadium\Media\Material\ItemFlag.Material.Gbx`), not a
            // user-inst node; the Solid2 writer emits the `materials` refs
            // when there is no custom material
            let link = inst.link().ok_or("external material form: the material has no link")?.to_string();
            let i = next_index(next);
            EXTERNALS.with(|e| e.borrow_mut().push((i as u32, format!("{link}.Material.Gbx"))));
            s2.materials.push(super::NodeRef { index: i, inline: None });
            continue;
        }
        s2.custom_materials.push(Material { name: String::new(), node: Some(inline(*next, Node::Material(inst))) });
        *next += 1;
    }
    // The source model's lights, in the pack's own form: each socket points at
    // an INLINE CPlugLight whose GxLight rides inline in turn (two node
    // indices). It renders in the editor AND in play once the source map's
    // stale lightmap is out of the way (`tmmaps tiny`; with it kept every
    // converted-block item was BLACK in play, lights or not).
    //
    // The forms tried and refused (2026-09-07): the item editor's
    // CPlugLightUserModel per light + `light_insts` (alone or beside the
    // socket) CRASHED the client at map load (Trackmania.exe+0x4c9062, the
    // light-inst loop reading [r14+0x78] = NULL — no reference item with
    // editor lights exists to read the layout off); a socket naming the pack's
    // `.Light.Gbx` as an external, or a `.Light.Gbx` written next to the item
    // (with or without `Items\`, use-file 0/1, ancestor levels 0–3), resolved
    // NOTHING — every such light was dark 60 m from any stock lamp. So the
    // projector cookie and the flare sprite (both texture fids) cannot be had;
    // the inline socket form is what works.
    for ml in m.lights_out.iter() {
        let mut socket = ml.socket.clone();
        let mut light = ml.light.clone();
        match light.gx_mut() {
            Some(gx) if gx.inline.is_some() => gx.index = *next + 1,
            Some(gx) => *gx = super::null_ref(),
            None => {}
        }
        socket.u02 = true;
        socket.u04.clear();
        socket.node = inline(*next, Node::Light(light));
        *next += 2;
        s2.lights.push(socket);
    }
    s2.pre_light_gen = if m.no_prelight { None } else { Some(m.pre_light_gen.clone().unwrap_or_else(default_prelight)) };
    s2.file_write_time = m.file_write_time;
    Ok(s2)
}

/// The merged collision as the canonical mesh surface.
pub fn build_surface(m: &Merged) -> CPlugSurface {
    if m.surf_triangles.is_empty() {
        // An item with NO collision at all (the OpenTech `_FC_Ground` decals:
        // DecalPlatform quads, 8 of Summer 09's models) is DROPPED by the
        // editor on re-save — all 122 placements of exactly those 8 models were
        // gone after SaveMap, everything else kept (2026-09-07). Play mode
        // draws them. So it gets one 1 mm triangle 4 m under the item's origin —
        // a shape the editor accepts and nothing can hit.
        let v = vec![[0.0, -4.0, 0.0], [0.001, -4.0, 0.0], [0.0, -4.0, 0.001]];
        // byte and table both NotCollidable (28): the one place the two
        // disagreed in a whole library (surfhist, 2026-09-07)
        let t = vec![super::surface::Triangle { indices: [0, 1, 2], material_id: 28, gameplay: 0, surface_index: 0 }];
        return CPlugSurface::mesh(v, t, vec![28], [0.0, 0.0, 1.0]);
    }
    CPlugSurface::mesh(m.surf_vertices.clone(), m.surf_triangles.clone(), m.surf_ids.clone(), [0.0, 0.0, 1.0])
}

/// Build the whole item tree from the merged geometry.
pub fn assemble(m: &Merged, opts: &BuildOpts) -> R<super::StaticItemFile> {
    use super::item::*;
    use super::Id;
    if m.visuals.is_empty() && m.dyna.is_empty() {
        return Err("no visuals: nothing to build".into());
    }
    // node 1 = the entity model; a static item fixes 2 (static object) and 3
    // (its solid) like the reference items — the item editor's
    // CGameCommonItemEntityModel form; a moving item, a gameplay gate or an
    // effect carrier takes the pack's own CPlugPrefab form (both read a detail
    // ladder the same way, probed 2026-09-07) and hands indices out in write
    // order from 2
    let prefab_form = !m.dyna.is_empty() || m.special.is_some() || !m.fx.is_empty();
    let mut next = if !prefab_form { 4i32 } else { 2i32 };
    // The static geometry: one static object (mesh + collision) — the whole
    // item when nothing moves, else one entity of the prefab.
    let static_object = if m.visuals.is_empty() {
        None
    } else {
        let mesh_index = if !prefab_form { 3 } else { next_index(&mut next) };
        let s2 = build_solid2(m, opts, &mut next)?;
        let surface_index = next_index(&mut next);
        Some(CPlugStaticObjectModel { version: 3, mesh: inline(mesh_index, Node::Solid2(s2)), is_mesh_collidable: false, shape: inline(surface_index, Node::Surface(build_surface(m))) })
    };
    let trigger = match m.trigger.as_ref() {
        Some(t) => {
            let i = next_index(&mut next);
            let mut t = t.clone();
            // inline material nodes of the trigger surface take their indices here
            for sm in t.materials.iter_mut() {
                if let super::surface::SurfMaterial::Node(r) = sm {
                    if r.inline.is_some() {
                        r.index = next_index(&mut next);
                    }
                }
            }
            inline(i, Node::Surface(t))
        }
        None => super::null_ref(),
    };
    let common = |entity: Ref| CGameCommonItemEntityModel {
        version: 6,
        v0_models: None,
        v3_strings: None,
        static_object: entity,
        trigger_shape: trigger.clone(),
        iso: { let mut i = IDENTITY; i[9] = m.spawn[0]; i[10] = m.spawn[1]; i[11] = m.spawn[2]; i },
        particle_emitter: super::null_ref(),
        actions: Vec::new(),
        u_node: super::null_ref(),
        strings: Default::default(),
        iso2: IDENTITY,
        expr_validator: 0,
        u_byte: 1,
    };
    let entity_model: Ref = if !prefab_form {
        let so = static_object.ok_or("no visuals: nothing to build")?;
        inline(1, Node::EntityModel(common(inline(2, Node::StaticObject(so)))))
    } else {
        // A prefab, laid out like the pack's obstacle prefabs: the moving
        // parts first (each a CPlugDynaObjectModel with its own mesh and two
        // hulls, its instance params carried), then the static part, then one
        // kinematic constraint per moving part binding the world (-1) to it.
        let mut ents: Vec<super::prefab::Entity> = Vec::new();
        let static_entity = |next: &mut i32, so: CPlugStaticObjectModel| {
            let i = next_index(next);
            super::prefab::Entity { model: inline(i, Node::StaticObject(so)), rot: [0.0, 0.0, 0.0, 1.0], pos: [0.0; 3], params_id: -1, params: Vec::new(), u01: Vec::new() }
        };
        for part in &m.dyna {
            // TINY_FLAG_REF=dyna: the entity model is the pack's own dyna FILE
            // (reference table), nothing of ours but the pose and the params
            if part.pack_ref == Some(super::merged::PackRef::Dyna) {
                let i = next_index(&mut next);
                EXTERNALS.with(|e| e.borrow_mut().push((i as u32, part.path.clone())));
                ents.push(super::prefab::Entity { model: super::NodeRef { index: i, inline: None }, rot: part.rot, pos: part.pos, params_id: part.instance_params_id, params: part.instance_params.clone(), u01: Vec::new() });
                continue;
            }
            // TINY_FLAG_REF=file: our dyna object and our mesh as two sidecar
            // FILES next to the item, named bare in the reference tables (the
            // in-archive form); the item's entity names the dyna file
            if part.pack_ref == Some(super::merged::PackRef::File) {
                let stem = opts.ident.strip_suffix(".Item.Gbx").unwrap_or(&opts.ident).to_string();
                let mesh_name = format!("{stem}.Mesh.Gbx");
                let dyna_name = format!("{stem}.DynaObject.Gbx");
                // the mesh file: root node 0 = the Solid2, its visuals/streams/
                // materials inline from 1 (its own numbering, its own strings)
                let mut mnext = 1;
                let s2 = build_solid2(&part.mesh, opts, &mut mnext).map_err(|e| format!("{}: {e}", part.path))?;
                let mesh_body = {
                    let mut out = Vec::new();
                    let mut lb = super::LookbackState::default();
                    let mut w = super::Wr { w: &mut out, lb: &mut lb };
                    super::write_node(&mut w, &Node::Solid2(s2));
                    out
                };
                SIDECARS.with(|s| s.borrow_mut().push((mesh_name.clone(), super::file::write_node_file(super::C_SOLID2_MODEL, &mesh_body, mnext as u32, &[]))));
                // the dyna file: root node 0 = the model, node 1 = the mesh file,
                // the hulls inline after it
                let mut model = part.model.clone();
                let mut dnext = 2;
                model.mesh = super::NodeRef { index: 1, inline: None };
                model.dyna_shape = match &part.move_shape {
                    Some(s) => inline(next_index(&mut dnext), Node::Surface(s.clone())),
                    None => super::null_ref(),
                };
                model.static_shape = match &part.hit_shape {
                    Some(s) => inline(next_index(&mut dnext), Node::Surface(s.clone())),
                    None => super::null_ref(),
                };
                let dyna_body = {
                    let mut out = Vec::new();
                    let mut lb = super::LookbackState::default();
                    let mut w = super::Wr { w: &mut out, lb: &mut lb };
                    super::write_node(&mut w, &Node::Dyna(model));
                    out
                };
                SIDECARS.with(|s| s.borrow_mut().push((dyna_name.clone(), super::file::write_node_file(super::dyna::C_DYNA_OBJECT_MODEL, &dyna_body, dnext as u32, &[(1, mesh_name.clone())]))));
                let i = next_index(&mut next);
                EXTERNALS.with(|e| e.borrow_mut().push((i as u32, dyna_name)));
                ents.push(super::prefab::Entity { model: super::NodeRef { index: i, inline: None }, rot: part.rot, pos: part.pos, params_id: part.instance_params_id, params: part.instance_params.clone(), u01: Vec::new() });
                continue;
            }
            let mesh_index = next_index(&mut next);
            let mut model = part.model.clone();
            model.mesh = match &part.pack_ref {
                // TINY_FLAG_REF=mesh: our CPlugDynaObjectModel over the pack's
                // own mesh FILE (full size)
                Some(super::merged::PackRef::Mesh(mp)) => {
                    EXTERNALS.with(|e| e.borrow_mut().push((mesh_index as u32, mp.clone())));
                    super::NodeRef { index: mesh_index, inline: None }
                }
                _ => {
                    let s2 = build_solid2(&part.mesh, opts, &mut next).map_err(|e| format!("{}: {e}", part.path))?;
                    inline(mesh_index, Node::Solid2(s2))
                }
            };
            model.dyna_shape = match &part.move_shape {
                Some(s) => {
                    let i = next_index(&mut next);
                    inline(i, Node::Surface(s.clone()))
                }
                None => super::null_ref(),
            };
            model.static_shape = match &part.hit_shape {
                Some(s) => {
                    let i = next_index(&mut next);
                    inline(i, Node::Surface(s.clone()))
                }
                None => super::null_ref(),
            };
            let i = next_index(&mut next);
            ents.push(super::prefab::Entity { model: inline(i, Node::Dyna(model)), rot: part.rot, pos: part.pos, params_id: part.instance_params_id, params: part.instance_params.clone(), u01: Vec::new() });
        }
        if let Some(so) = static_object {
            ents.push(static_entity(&mut next, so));
        }
        // the gameplay gate's effect volume, the pack's entity 1: an
        // NPlugTrigger_SGateSpecial at the identity with its shape inline
        if let Some(sp) = &m.special {
            let si = next_index(&mut next);
            let gi = next_index(&mut next);
            let g = super::GateSpecialTrigger { version: 2, shape: inline(si, Node::Surface(sp.clone())), u01: 0 };
            ents.push(super::prefab::Entity { model: inline(gi, Node::GateSpecial(g)), rot: [0.0, 0.0, 0.0, 1.0], pos: [0.0; 3], params_id: -1, params: Vec::new(), u01: Vec::new() });
        }
        // the effect systems (smoke, sparks), after the static part like the
        // pack's Show prefabs (Fogger16M: entity 0 the box, entity 1 the FxSys)
        ents.extend(fx_entities(m, opts.scale, &mut next));
        for (k, part) in m.dyna.iter().enumerate() {
            // Ent2 ranks the dyna objects of the prefab (the k-th
            // CPlugDynaObjectModel entity), whatever sits between them
            let Some((constraint, cparams)) = part.constraint.as_ref() else { continue };
            let mut cp = cparams.clone();
            cp.ent1 = -1;
            cp.ent2 = k as i32;
            let i = next_index(&mut next);
            ents.push(super::prefab::Entity { model: inline(i, Node::Kinematic(constraint.clone())), rot: [0.0, 0.0, 0.0, 1.0], pos: [0.0; 3], params_id: super::dyna::P_CONSTRAINT, params: cp.bytes(), u01: Vec::new() });
        }
        let prefab = super::prefab::CPlugPrefab { version: 11, file_write_time: 0, url: String::new(), u01: 0, u02: 0, ents };
        // straight under CGameItemModel, as the pack's own obstacle items do:
        // wrapped in a CGameCommonItemEntityModel the game drops the item
        // silently (MovD, 2026-09-07)
        inline(1, Node::Prefab(prefab))
    };
    let placement_index = next_index(&mut next);
    let sclass_index = next_index(&mut next);
    let ident = || ItemChunk::Ident { path: Id::Str(opts.ident.clone()), collection: Id::Raw(opts.collection), author: Id::Str(opts.author.clone()) };
    let model = ModelChunk {
        version: 15,
        old_models: None,
        default_weapon_name: Id::Null,
        phy_model_custom: super::null_ref(),
        vis_model_custom: super::null_ref(),
        actions: Vec::new(),
        default_cam: 0,
        entity_model_edition: super::null_ref(),
        entity_model,
        vfx: super::null_ref(),
        material_modifier: super::null_ref(),
    };
    let chunks = vec![
        ItemChunk::Collector1009 { page_name: "Items".into(), icon: None, u01: Id::Null },
        ident(),
        ItemChunk::Name("New Item".into()),
        ItemChunk::Description("No Description".into()),
        ItemChunk::Skin { version: 4, default_skin: super::null_ref(), skin_directory: String::new(), extra: Some(super::null_ref()) },
        ItemChunk::Catalog { version: 1, is_internal: false, is_advanced: false, position: 1, prod_state: Some(3) },
        ItemChunk::Ints1012([0, 1, 0, 0]),
        ItemChunk::NadeoSkinFids(vec![super::null_ref(); 7]),
        ItemChunk::Cameras(10, Vec::new()),
        ItemChunk::RaceInterface(super::null_ref()),
        ItemChunk::Ground { ground_point: [0.0; 3], floats: [0.0, 0.0, -1.0, 0.15] },
        ItemChunk::ItemType(1),
        ItemChunk::Model(model),
        ItemChunk::Node201A(super::null_ref()),
        ItemChunk::DefaultPlacement { version: 5, placement: inline(placement_index, Node::Placement(placement_param(sclass_index))) },
        ItemChunk::Archetype { version: 7, archetype_ref: String::new(), archetype_fid: Some(super::null_ref()), skin_dir: Some(String::new()), u01: Some(-1) },
        ItemChunk::Waypoint { version: 12, waypoint_type: m.waypoint_type.unwrap_or(3), disable_lightmap: false, u_node: Some(super::null_ref()), u_byte: Some(0), u_ints: Some((-1, -1)) },
        ItemChunk::Icon { version: 3, icon_fid: String::new(), u_byte: Some(1) },
        ItemChunk::Skippable(super::RawChunk { id: 0x2E002025, payload: vec![0; 8] }),
        ItemChunk::Skippable(super::RawChunk { id: 0x2E002026, payload: vec![0; 8] }),
        ItemChunk::Skippable(super::RawChunk { id: 0x2E002027, payload: vec![0; 8] }),
    ];
    Ok(super::StaticItemFile {
        version: 6,
        format: b'B',
        ref_comp: b'U',
        body_comp: b'U',
        unknown: Some(b'R'),
        class_id: super::C_ITEM_MODEL,
        header_chunks: header_chunks(opts),
        num_nodes: next as u32,
        // the reference table: the pack files the effect systems name, as the
        // packs spell them
        ref_table: {
            let ext = EXTERNALS.with(|e| std::mem::take(&mut *e.borrow_mut()));
            super::file::ref_table(&ext)
        },
        item: CGameItemModel { chunks },
    })
}

pub fn next_index(next: &mut i32) -> i32 {
    let i = *next;
    *next += 1;
    i
}

/// Header chunks 2E001003 (desc, v8), 2E001006 (lightmap time 0),
/// 2E002000 (item type Ornament), 2E002001 (file version 0).
/// Header chunks 2E001003 (desc, v8), [090F4000 the source's game skin],
/// 2E001006 (lightmap time 0), 2E002000 (item type Ornament), 2E002001 (file
/// version 0). Nadeo's items carry the skin chunk right after the icon.
pub fn header_chunks(opts: &BuildOpts) -> Vec<super::file::HeaderChunk> {
    use super::file::HeaderChunk;
    let mut d = Vec::new();
    let mut lb = super::LookbackState::default();
    {
        let mut w = super::Wr { w: &mut d, lb: &mut lb };
        w.id(&super::Id::Str(opts.ident.clone()));
        w.id(&super::Id::Raw(opts.collection));
        w.id(&super::Id::Str(opts.author.clone()));
        w.u32(8);
        w.string("Items");
        w.id(&super::Id::Null);
        // the collector flags word: 8 on every item-editor item, 0x10 on
        // every pack item (Flag16m, ObstaclePusher8mLevel1: flags 0x10,
        // catalog position 101/233, prod state 3). TINY_ITEM_DESC_FLAGS=N
        // (decimal or 0xHEX) is the 2026-09-08 probe of whether that word
        // gates anything at runtime (the dyna animation driver).
        let desc_flags: i32 = std::env::var("TINY_ITEM_DESC_FLAGS").ok().and_then(|v| v.strip_prefix("0x").map(|h| i32::from_str_radix(h, 16).ok()).unwrap_or_else(|| v.parse().ok())).unwrap_or(8);
        w.i32(desc_flags);
        w.i16(1);
        w.string("New Item");
        w.u8(3);
    }
    let mut out = vec![HeaderChunk { id: 0x2E001003, heavy: false, payload: d }];
    if let Some(skin) = opts.skin.as_ref() {
        out.push(HeaderChunk { id: tmmaps::header::GAME_SKIN_CHUNK, heavy: false, payload: skin.clone() });
    }
    out.extend([
        HeaderChunk { id: 0x2E001006, heavy: false, payload: vec![0; 8] },
        HeaderChunk { id: 0x2E002000, heavy: false, payload: 1u32.to_le_bytes().to_vec() },
        HeaderChunk { id: 0x2E002001, heavy: false, payload: vec![0; 4] },
    ]);
    out
}

