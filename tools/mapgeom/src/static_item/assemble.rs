//! From the accumulator to the item file: the merged visuals and materials
//! as one `CPlugSolid2Model` (`build_solid2`), the collision as one
//! `CPlugSurface` (`build_surface`), and the whole `CGameItemModel` tree in
//! the reference `.Item.Gbx` layout (`assemble`, `header_chunks`).

use super::lod::{cap_lod_ladder, lod0_only, remap_lod_mask, MAX_LOD_LEVELS};
use super::materials::{custom_texture_material, light_skin_material, screen_face_material, sign_logo_material, skinned_material, trigger_fx_material};
use super::merged::{coalesce, harmonize_layouts_with, Merged};
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
                // leaf atlas cuts fine under TDOSN (slot 1) once the visual has its
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
    /// `Merged::share_materials`: custom-texture material nodes already written
    /// in this item, by their identity (name, model, textures) → node index.
    pub static SHARED_MATS: std::cell::RefCell<std::collections::HashMap<String, i32>> = std::cell::RefCell::new(std::collections::HashMap::new());
    /// How many parts the last `build_solid2` repacked lightmap atlases for
    /// (`repack_lightmap_parts`); None = nothing to repack. For the report.
    pub static REPACK_NOTE: std::cell::Cell<Option<usize>> = const { std::cell::Cell::new(None) };
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
    // The platform-plastic family (`…\PlatformTech`, plain or under a
    // modifier) is the one whose loader crashed on a visual without a
    // tangent frame (item-check SH-03); every visual under it gets one,
    // synthesised round the normal when the pack gave none (Nadeo's own
    // PlatformDirt / PlatformPlastic Turbo slopes).
    let want_tangents: Vec<usize> = m.materials.iter().enumerate().filter(|(_, mat)| mat.link().map(|l| l.ends_with("\\PlatformTech")).unwrap_or(false)).map(|(i, _)| i).collect();
    // Every merged PART's lightmap atlas into its own cell (see
    // `repack_lightmap_parts`); TINY_LIGHTMAP_REPACK=0 keeps the overlap.
    if std::env::var("TINY_LIGHTMAP_REPACK").map(|v| v != "0").unwrap_or(true) {
        // a solid visual without a lightmap uv in an item that has one elsewhere gets a planar chart of
        // its own (its own part → its own cell below); the game does not chart a visual without
        // TexCoord1 at all — a bush-bearing hill's slopes took whatever its cards' cell held (2026-09-23)
        if !super::build::card_uv1_legacy() {
            synthesize_solid_uv1(&mut pre, m);
        }
        if let Some(n) = repack_lightmap_parts(&mut pre) {
            REPACK_NOTE.with(|c| c.set(Some(n)));
        }
        if std::env::var_os("MAPGEOM_UV1_DEBUG").is_some() {
            use super::vstream::{Elem, N_TEXCOORD0};
            for (vi, mv) in pre.iter().enumerate() {
                let Some(s) = mv.visual.stream() else { continue };
                let Some(i) = s.decls.iter().position(|d| d.name() == N_TEXCOORD0 + 1) else { eprintln!("uv1 debug: visual {vi} part {:#x} mat {} lod {:#x}: no uv1", mv.part, mv.material, mv.lod_mask); continue };
                if let Elem::Float2(uv) = &s.elems[i] {
                    let (mut lo, mut hi) = ([f32::MAX; 2], [f32::MIN; 2]);
                    for p in uv { for k in 0..2 { lo[k] = lo[k].min(p[k]); hi[k] = hi[k].max(p[k]); } }
                    eprintln!("uv1 debug: visual {vi} part {:#x} mat {} lod {:#x}: uv1 ({:.3},{:.3})..({:.3},{:.3}) over {} verts", mv.part, mv.material, mv.lod_mask, lo[0], lo[1], hi[0], hi[1], uv.len());
                }
            }
        }
    }
    // TINY_LIGHTMAP_FILL=1 (probe, 2026-09-12, the shadow-seam "one tone per
    // tile"): the item's lightmap charts stretched to fill the unit square. A
    // pack block mesh lays its charts over a small corner of the atlas (the
    // platform pieces: 0.12 × 0.045 of it, PreLightGen u02 = 46 m per uv unit)
    // — fine for the game's own BLOCK budget, but as an ITEM that corner is all
    // the texels the piece gets and a 16 m deck bakes to a handful of them. The
    // scale word follows (metres per uv unit ÷ the stretch) so the density the
    // lightmapper derives from it stays honest; u04 = the new bounds.
    let mut fill_scale: Option<(f64, [f64; 2], [f32; 4])> = None;
    if std::env::var("TINY_LIGHTMAP_FILL").map(|v| v == "1").unwrap_or(false) {
        fill_scale = fill_lightmap_atlas(&mut pre);
    }
    // The PreLightGen bounds (u04) must cover EVERY visual's uv1 — the game maps uv1 through the chart's
    // ST built from them, so anything outside lands in the neighbouring items' atlas rects (2026-09-23).
    let uv1_union: Option<[f32; 4]> = if super::build::card_uv1_legacy() { None } else { uv1_bounds(&pre) };
    harmonize_layouts_with(&mut pre, &want_tangents);
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
        let inst = screen_face_material(&inst, m);
        let inst = trigger_fx_material(&inst, m);
        let inst = light_skin_material(&inst, m);
        if m.materials_external {
            // the pack mesh's form: the material is a FILE the reference table
            // names (`Stadium\Media\Material\ItemFlag.Material.Gbx`), not a
            // user-inst node; the Solid2 writer emits the `materials` refs
            // when there is no custom material
            let link = inst.link().ok_or("external material form: the material has no link")?.to_string();
            let i = next_index(next);
            // `materials_bare`: the bare file name (a copy of the material file
            // carried in the map archive next to the item) instead of the pack path
            let file = if m.materials_bare { format!("{}.Material.Gbx", link.rsplit('\\').next().unwrap_or(&link)) } else { format!("{link}.Material.Gbx") };
            EXTERNALS.with(|e| e.borrow_mut().push((i as u32, file)));
            s2.materials.push(super::NodeRef { index: i, inline: None });
            continue;
        }
        if m.share_materials {
            if let Some(main) = inst.main.as_ref().filter(|mm| !mm.is_using_game_material) {
                let key = format!("{:?}|{:?}|{:?}", main.material_name, main.model, main.user_textures.iter().map(|t| (t.u01, t.texture.clone())).collect::<Vec<_>>());
                let known = SHARED_MATS.with(|s| s.borrow().get(&key).copied());
                if let Some(idx) = known {
                    s2.custom_materials.push(Material { name: String::new(), node: Some(super::NodeRef { index: idx, inline: None }) });
                    continue;
                }
                SHARED_MATS.with(|s| s.borrow_mut().insert(key, *next));
            }
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
    // TINY_PRELIGHT_U02=F (probe, 2026-09-12): the scale word of every item that has no
    // computed PreLightGen (the block-converted ones) — the lightmapper's texel budget for
    // the item's atlas. The shadow-seam work: platform tiles come out one flat tone EACH
    // (tile-to-tile steps) under the default 32.14.
    s2.pre_light_gen = if m.no_prelight {
        None
    } else {
        let mut pl = m.pre_light_gen.clone().unwrap_or_else(default_prelight);
        if m.pre_light_gen.is_none() {
            if let Some(u02) = std::env::var("TINY_PRELIGHT_U02").ok().and_then(|v| v.parse::<f32>().ok()) {
                pl.u02 = u02;
            }
        }
        if let Some((s, _lo, bounds)) = fill_scale {
            pl.u02 = (pl.u02 as f64 / s) as f32;
            pl.u04[0] = bounds[0];
            pl.u04[1] = bounds[1];
            pl.u04[2] = bounds[2];
            pl.u04[3] = bounds[3];
        } else if let Some(b) = uv1_union {
            // the union of every visual's uv1 (block mesh AND vegetation cards, after the part repack),
            // never narrower than what the source declared
            pl.u04[0] = pl.u04[0].min(b[0]);
            pl.u04[1] = pl.u04[1].min(b[1]);
            pl.u04[2] = pl.u04[2].max(b[2]);
            pl.u04[3] = pl.u04[3].max(b[3]);
        }
        // TINY_LIGHTMAP_U02=measured (probe, 2026-09-12): the scale word from the
        // geometry itself, the same metres-per-uv rule for every item.
        if std::env::var("TINY_LIGHTMAP_U02").map(|v| v == "measured").unwrap_or(false) {
            if let Some(u) = measured_u02(&pre) {
                pl.u02 = u;
            }
        }
        Some(pl)
    };
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
    REPACK_NOTE.with(|c| c.set(None));
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
    // A no-respawn CHECKPOINT (the `GateCheckpoint` ring block's info says
    // NoRespawn; a pack prefab's NPlugTrigger_SWaypoint may) exists only in the
    // prefab form: the flag is a field of that entity, the entity-model form
    // has none — and a ring written as an entity model respawned the car at
    // its own origin, beside the ring (Argentina 21, 2026-09-09). A finish
    // (GateFinish, GateExpandableFinish: NoRespawn too) keeps the entity-model
    // form — nobody respawns at a finish, and the form is the proven one.
    let no_respawn_wp = m.no_respawn && m.trigger.is_some() && m.waypoint_type == Some(2);
    let prefab_form = !m.dyna.is_empty() || m.special.is_some() || !m.fx.is_empty() || no_respawn_wp;
    SHARED_MATS.with(|s| s.borrow_mut().clear());
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
        // shared materials: the static entity comes first, defining the nodes
        // the moving parts back-reference
        let mut static_object = static_object;
        if m.share_materials {
            if let Some(so) = static_object.take() {
                ents.push(static_entity(&mut next, so));
            }
        }
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
        // the no-respawn waypoint's trigger, the pack's own layout
        // (Items\Gate\CheckpointRight32m.Prefab entity 5): an
        // NPlugTrigger_SWaypoint at the identity with the trigger shape inline
        // and NoRespawn set — followed, as in the pack ring (entities 6 and 7),
        // by an NPlugTrigger_SSpawn at the block's spawn pose and the 8-byte
        // 0x0917B000 node. Without the spawn the CLIENT respawned at the ITEM's
        // pivot — beside the ring, at ring height (vjeux, 2026-09-09: "drops you
        // to the side") — while the server used its own restore; the spawn
        // gives both engines the same point: the block info's spawn location
        // scaled (the road under the ring, facing the route, in the item's
        // frame — the block's yaw is the placement's), the floor centre when
        // the block info has none. The SSpawn body is the pack's byte for byte
        // (chunk 0x0917A000 v3: Iso4, then 24 bytes 0,0,0,0,-1.0,0; FACADE).
        if no_respawn_wp {
            let wi = next_index(&mut next);
            let wp = super::WaypointTrigger { version: 1, wtype: m.waypoint_type.unwrap_or(2), shape: trigger.clone(), no_respawn: 1 };
            ents.push(super::prefab::Entity { model: inline(wi, Node::WaypointTrigger(wp)), rot: [0.0, 0.0, 0.0, 1.0], pos: [0.0; 3], params_id: -1, params: Vec::new(), u01: Vec::new() });
            // TINY_RING_SPAWN=1 opts IN. The SSpawn entity CRASHES THE CLIENT at map
            // load (2026-09-10, startcheck bisect on Summer 15: every build with
            // it — ship16c-c4ce31c5, ship16-60e03cbc — "game process gone" at
            // 12–116 s; the same recipe with the entity left out PASSES; 01, with
            // no ring block, passes with everything else). The pack ring prefab
            // has the same two entities (0x0917A000 at pos 0, 0x0917B000 at pos
            // 0); byte comparison (CheckpointCenter8mV2.Prefab vs our ring item):
            // the SSpawn node, the entity records and the layout are identical,
            // ONE difference — the 0x0917B000 companion's 8-byte body is
            // (0, 11) in the pack and was (0, 0) in ours (the 11 is now copied;
            // untested in the client). Our SSpawn carries the spawn in the ENTITY
            // pos (the pack's is at 0 with an identity Iso4 — its spawn IS the
            // origin), the other candidate if the body word was not it. Off by default: the ship15 form (trigger only, the
            // respawn beside the ring) until the encoding is understood.
            // TINY_RING_SPAWN=1: spawn in the ENTITY pos (our first form); =iso: the pack's
            // form, entity at 0 and the Iso4 carrying the spawn; =1nobody / =isonobody: the
            // same without the 0x0917B000 companion (the bisect of 2026-09-11)
            let ring_mode = std::env::var("TINY_RING_SPAWN").unwrap_or_default();
            let ring_spawn = matches!(ring_mode.as_str(), "1" | "iso" | "1nobody" | "isonobody");
            if ring_spawn {
            let iso_form = ring_mode.starts_with("iso");
            let si = next_index(&mut next);
            let (node, epos) = if iso_form { (spawn_trigger_node_at(m.spawn), [0.0f32; 3]) } else { (spawn_trigger_node(), m.spawn) };
            ents.push(super::prefab::Entity { model: inline(si, Node::Opaque(node)), rot: [0.0, 0.0, 0.0, 1.0], pos: epos, params_id: -1, params: Vec::new(), u01: Vec::new() });
            if !ring_mode.ends_with("nobody") {
            let ti = next_index(&mut next);
            ents.push(super::prefab::Entity { model: inline(ti, Node::Opaque(super::OpaqueNode { class_id: 0x0917B000, raw: { let mut r = vec![0u8; 8]; r[4] = 0x0b; r } })), rot: [0.0, 0.0, 0.0, 1.0], pos: [0.0; 3], params_id: -1, params: Vec::new(), u01: Vec::new() });
            }
            }
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
        // TINY_FLAG_VARIANTLIST=1 (2026-09-08 probe): the pack FLAG items wrap
        // their prefab in an NPlugItem_SVariantList (one variant, tags
        // MatModifier Grass/Dirt/Ice + Type Flag) — the obstacles do not; does
        // the wrapper decide how the item's visual dynas are instantiated?
        if std::env::var("TINY_FLAG_VARIANTLIST").as_deref() == Ok("1") {
            let pi = next_index(&mut next);
            let tags = vec![("MatModifier".to_string(), "Grass".to_string()), ("MatModifier".to_string(), "Dirt".to_string()), ("MatModifier".to_string(), "Ice".to_string()), ("Type".to_string(), "Flag".to_string())];
            inline(1, Node::VariantList(super::VariantList { version: 1, variants: vec![super::Variant { tags, model: inline(pi, Node::Prefab(prefab)), hidden: 0 }] }))
        } else {
            inline(1, Node::Prefab(prefab))
        }
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


/// A TexCoord1 for every SOLID visual that has none, when some visual of the item does have one: a
/// planar projection of the visual's positions onto the plane of its two largest bbox extents,
/// normalised to [0.001, 0.999]², under a fresh part id (`SOLID_PART_BASE` + k) so the repack
/// gives it a cell of its own. A cut-out (card) material's visual is left to the card path. The
/// game charts only visuals with TexCoord1; without one, a hill's slopes sampled the corner texel
/// of its chart — the leaf-card cell (the hill test map, 2026-09-23). Returns how many were given one.
pub fn synthesize_solid_uv1(visuals: &mut [super::merged::MergedVisual], m: &super::merged::Merged) -> usize {
    use super::vstream::{Decl, Elem, N_POSITION, N_TEXCOORD0, SPACE_LOCAL3D, T_FLOAT2};
    let has_uv1 = |v: &super::merged::MergedVisual| v.visual.stream().map(|s| s.decls.iter().any(|d| d.name() == N_TEXCOORD0 + 1)).unwrap_or(false);
    if !visuals.iter().any(|v| has_uv1(v)) {
        return 0;
    }
    let is_card = |v: &super::merged::MergedVisual| m.materials.get(v.material).map(|mat| mat.link().map(|l| l.is_empty()).unwrap_or(true) && mat.main.as_ref().map(|mm| mm.user_textures.iter().any(|t| t.u01 == 1)).unwrap_or(false)).unwrap_or(false);
    let mut next_part = visuals.iter().map(|v| v.part).max().unwrap_or(0).max(super::merged::SOLID_PART_BASE) + 1;
    let mut n_done = 0usize;
    for mv in visuals.iter_mut() {
        if has_uv1(mv) || is_card(mv) {
            continue;
        }
        let Some(main) = mv.visual.main.as_mut() else { continue };
        let Some(super::Node::VertexStream(s)) = main.vertex_streams.first_mut().and_then(|r| r.inline.as_deref_mut()) else { continue };
        let Some(pi) = s.decls.iter().position(|d| d.name() == N_POSITION) else { continue };
        let Elem::Float3(pos) = &s.elems[pi] else { continue };
        if pos.is_empty() { continue; }
        let (mut lo, mut hi) = ([f32::MAX; 3], [f32::MIN; 3]);
        for p in pos { for k in 0..3 { lo[k] = lo[k].min(p[k]); hi[k] = hi[k].max(p[k]); } }
        let ext = [hi[0] - lo[0], hi[1] - lo[1], hi[2] - lo[2]];
        // drop the smallest axis
        let drop = if ext[0] <= ext[1] && ext[0] <= ext[2] { 0 } else if ext[1] <= ext[2] { 1 } else { 2 };
        let (ax, ay) = match drop { 0 => (2, 1), 1 => (0, 2), _ => (0, 1) };
        let (ex, ey) = (ext[ax].max(1e-4), ext[ay].max(1e-4));
        let uv: Vec<[f32; 2]> = pos.iter().map(|p| [0.001 + 0.998 * (p[ax] - lo[ax]) / ex, 0.001 + 0.998 * (p[ay] - lo[ay]) / ey]).collect();
        // append the element: decls carry the stride in bits 20..27 (in 4-byte units) — recompute
        let compress = s.compress_local3d.unwrap_or(false);
        let old_stride: u32 = s.decls.iter().map(|d| super::vstream::type_size(d.stored_type(compress)).unwrap_or(4) as u32).sum();
        let stride = old_stride + 8;
        let mut decls: Vec<Decl> = Vec::new();
        let mut offset = 0u32;
        for d in &s.decls {
            decls.push(Decl::with_stride(d.name(), d.ty(), d.space(), offset, stride / 4));
            offset += super::vstream::type_size(d.stored_type(compress)).unwrap_or(4) as u32;
        }
        decls.push(Decl::with_stride(N_TEXCOORD0 + 1, T_FLOAT2, SPACE_LOCAL3D, offset, stride / 4));
        s.decls = decls;
        s.elems.push(Elem::Float2(uv));
        mv.part = next_part;
        next_part += 1;
        n_done += 1;
    }
    n_done
}

/// Every merged PART's lightmap atlas into its own cell of a grid over the
/// unit square, so no two parts' charts share lightmap texels.
///
/// A pack `CPlugSolid2Model` lays its own lightmap atlas (TexCoord1) over
/// the whole [0,1]²; merging several into one item (a block's mobils, a
/// rig's entities, a gate's arch + sign + lights) stacked their charts on
/// top of each other, and the game's per-item lightmap bake — redone at
/// every load — then wrote every part's light into the same texels: a
/// cross-talk the trees showed at its extreme (every leaf on one region,
/// 2026-09-09). Parts are found by `MergedVisual::part`; a visual without
/// uv1 is left alone. Part 0 (a block's own mesh) is a part like the others
/// when the item has more than one: leaving it at its authored layout put a
/// bush-bearing hill's slopes under its cards' cells (the game's bake wrote
/// the cards' light over the slopes, 2026-09-23). With ONE part nothing
/// moves (Nadeo's layout is kept as authored). Returns the number of parts
/// repacked, or None when there was nothing to do.
pub fn repack_lightmap_parts(visuals: &mut [super::merged::MergedVisual]) -> Option<usize> {
    use super::vstream::{Elem, N_TEXCOORD0};
    // only visuals that carry uv1 count as parts
    let mut parts: Vec<u32> = visuals.iter().filter(|v| v.visual.stream().map(|s| s.decls.iter().any(|d| d.name() == N_TEXCOORD0 + 1)).unwrap_or(false)).map(|v| v.part).collect();
    parts.sort_unstable();
    parts.dedup();
    if parts.len() < 2 {
        return None;
    }
    let n = parts.len();
    let grid = (n as f64).sqrt().ceil().max(1.0);
    let cell = 1.0 / grid;
    // the pack margin (0.001) scaled with the cell, plus a gutter between cells
    let margin = cell * 0.01;
    let inner = cell - 2.0 * margin;
    // `TINY_LIGHTMAP_REPACK=fit`: a part's uv1 BOUNDS fill its cell (uniform scale, centred) instead of
    // its [0,1]² square — a pack block's charts sit in a small corner of the square (a platform piece uses
    // 0.12 × 0.045 of it), so the default leaves each part a dot of texels and its lightmap one value per
    // part (the editor's bake shows the same). Off by default: it changes every merged item's lightmap
    // UVs, i.e. the shipped item lists' bytes. Pair it with TINY_LIGHTMAP_U02=measured.
    let fit = std::env::var("TINY_LIGHTMAP_REPACK").map(|v| v == "fit").unwrap_or(false);
    let mut part_bounds: std::collections::HashMap<u32, [f32; 4]> = std::collections::HashMap::new();
    if fit {
        for mv in visuals.iter() {
            let Some(s) = mv.visual.stream() else { continue };
            let Some(i) = s.decls.iter().position(|d| d.name() == N_TEXCOORD0 + 1) else { continue };
            if let Elem::Float2(uv) = &s.elems[i] {
                let b = part_bounds.entry(mv.part).or_insert([f32::MAX, f32::MAX, f32::MIN, f32::MIN]);
                for p in uv {
                    b[0] = b[0].min(p[0]);
                    b[1] = b[1].min(p[1]);
                    b[2] = b[2].max(p[0]);
                    b[3] = b[3].max(p[1]);
                }
            }
        }
    }
    for mv in visuals.iter_mut() {
        let Some(k) = parts.iter().position(|p| *p == mv.part) else { continue };
        let k = k as f64;
        let (cx, cy) = ((k % grid) * cell + margin, (k / grid).floor() * cell + margin);
        let fitb = if fit { part_bounds.get(&mv.part).copied().filter(|b| b[2] > b[0] && b[3] > b[1]) } else { None };
        let Some(s) = mv.visual.stream_mut() else { continue };
        let Some(i) = s.decls.iter().position(|d| d.name() == N_TEXCOORD0 + 1) else { continue };
        if let Elem::Float2(uv) = &mut s.elems[i] {
            match fitb {
                Some(b) => {
                    let (w, h) = ((b[2] - b[0]) as f64, (b[3] - b[1]) as f64);
                    let sc = inner / w.max(h);
                    let (ox, oy) = (cx + (inner - w * sc) * 0.5, cy + (inner - h * sc) * 0.5);
                    for p in uv.iter_mut() {
                        p[0] = (ox + (p[0] - b[0]) as f64 * sc) as f32;
                        p[1] = (oy + (p[1] - b[1]) as f64 * sc) as f32;
                    }
                }
                None => {
                    for p in uv.iter_mut() {
                        p[0] = (cx + p[0].clamp(0.0, 1.0) as f64 * inner) as f32;
                        p[1] = (cy + p[1].clamp(0.0, 1.0) as f64 * inner) as f32;
                    }
                }
            }
        }
    }
    Some(n)
}

#[cfg(test)]
mod repack_tests {
    use super::super::merged::MergedVisual;
    use super::super::visual::CPlugVisualIndexedTriangles;
    use super::super::vstream::{CPlugVertexStream, Decl, Elem, N_POSITION, N_TEXCOORD0, T_FLOAT2, T_FLOAT3};
    use super::super::null_ref;

    fn visual(part: u32, uv1: Vec<[f32; 2]>) -> MergedVisual {
        let n = uv1.len();
        let s = CPlugVertexStream {
            version: 1,
            count: n as i32,
            flags: 0,
            base: null_ref(),
            decls: vec![Decl::with_stride(N_POSITION, T_FLOAT3, 0, 0, 7), Decl::with_stride(N_TEXCOORD0, T_FLOAT2, 0, 12, 7), Decl::with_stride(N_TEXCOORD0 + 1, T_FLOAT2, 0, 20, 7)],
            compress_local3d: Some(false),
            elems: vec![Elem::Float3(vec![[0.0; 3]; n]), Elem::Float2(vec![[0.0; 2]; n]), Elem::Float2(uv1)],
        };
        let main = super::super::visual::VisualMain {
            version: 6,
            chunk_flags: 0,
            tex_coord_sets: Vec::new(),
            count: n as i32,
            vertex_streams: vec![super::super::Ref { index: 0, inline: Some(Box::new(super::super::Node::VertexStream(s))) }],
            skin: None,
            bounding_box: [0.0; 6],
            bitmap_elems: Vec::new(),
            uv_groups: Vec::new(),
            u02: 0,
            u03: 0,
            u04: Vec::new(),
        };
        let v = CPlugVisualIndexedTriangles {
            chunks: Vec::new(),
            id: super::super::Id::Null,
            u_node: null_ref(),
            sub_visuals: Vec::new(),
            u_float: 0.0,
            splits: Vec::new(),
            main: Some(main),
            morph: None,
            v3d_node: null_ref(),
            tangents: None,
            index_buffer: None,
            inline_form: false,
            inline_uv_sets: 1,
            inline_uv_flags: 256,
            inline_tangents: false,
        };
        MergedVisual { visual: v, material: 0, lod_mask: 0, lod_ladder: Vec::new(), part }
    }

    fn uv1_of(mv: &MergedVisual) -> Vec<[f32; 2]> {
        match &mv.visual.stream().unwrap().elems[2] {
            Elem::Float2(v) => v.clone(),
            _ => panic!("uv1 gone"),
        }
    }

    /// One part: Nadeo's layout is kept. Three parts: a 2x2 grid, every
    /// part inside its own cell, the full-square chart of one part never
    /// touching another's cell.
    #[test]
    fn parts_land_in_disjoint_cells() {
        let full = vec![[0.001, 0.001], [0.999, 0.001], [0.999, 0.999], [0.001, 0.999]];
        let mut one = vec![visual(1, full.clone()), visual(1, full.clone())];
        assert_eq!(super::repack_lightmap_parts(&mut one), None);
        assert_eq!(uv1_of(&one[0]), full);
        let mut three = vec![visual(1, full.clone()), visual(2, full.clone()), visual(1, full.clone()), visual(3, full.clone())];
        assert_eq!(super::repack_lightmap_parts(&mut three), Some(3));
        let cell = |uv: [f32; 2]| ((uv[0] * 2.0).floor() as i32, (uv[1] * 2.0).floor() as i32);
        let cells: Vec<(i32, i32)> = three.iter().map(|mv| {
            let uv = uv1_of(mv);
            let c = cell(uv[0]);
            for p in &uv {
                assert_eq!(cell(*p), c, "a chart crossed a cell border: {p:?}");
                assert!((0.0..=1.0).contains(&p[0]) && (0.0..=1.0).contains(&p[1]));
            }
            c
        }).collect();
        assert_eq!(cells[0], cells[2], "the two visuals of part 1 share a cell");
        assert_ne!(cells[0], cells[1]);
        assert_ne!(cells[1], cells[3]);
        assert_ne!(cells[0], cells[3]);
    }
}

/// The pack's `NPlugTrigger_SSpawn` body (Items\Gate\CheckpointRight32m.Prefab
/// node 27, read off the bytes 2026-09-09): chunk 0x0917A000 version 3, an
/// identity Iso4, then 24 bytes `0, 0, 0, 0, f32 -1.0, 0`, FACADE. The
/// spawn's POSE is the entity's, so the body stays identity and the caller
/// positions the entity.
pub fn spawn_trigger_node() -> super::OpaqueNode {
    spawn_trigger_node_at([0.0; 3])
}

/// The SSpawn node with its Iso4 carrying a translation (the pack's form: the
/// entity at 0, the spawn IN the node — `TINY_RING_SPAWN=iso`).
pub fn spawn_trigger_node_at(t: [f32; 3]) -> super::OpaqueNode {
    let mut raw: Vec<u8> = Vec::with_capacity(84);
    raw.extend_from_slice(&0x0917A000u32.to_le_bytes());
    raw.extend_from_slice(&3u32.to_le_bytes());
    for v in [1.0f32, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0, t[0], t[1], t[2]] {
        raw.extend_from_slice(&v.to_le_bytes());
    }
    for v in [0u32, 0, 0, 0] {
        raw.extend_from_slice(&v.to_le_bytes());
    }
    raw.extend_from_slice(&(-1.0f32).to_le_bytes());
    raw.extend_from_slice(&0u32.to_le_bytes());
    raw.extend_from_slice(&super::FACADE.to_le_bytes());
    super::OpaqueNode { class_id: 0x0917A000, raw }
}

/// The union of every visual's lightmap chart (TexCoord1) stretched to fill the
/// unit square (margin 0.001, aspect kept, anchored at the margin). Returns the
/// stretch factor, the old lower bound and the new bounds `[x0, y0, x1, y1]`;
/// `None` when no visual carries a TexCoord1 or the charts are degenerate.
pub fn fill_lightmap_atlas(visuals: &mut [super::merged::MergedVisual]) -> Option<(f64, [f64; 2], [f32; 4])> {
    use super::vstream::{Elem, N_TEXCOORD0};
    let (mut lo, mut hi) = ([f64::MAX; 2], [f64::MIN; 2]);
    for mv in visuals.iter() {
        let Some(s) = mv.visual.stream() else { continue };
        let Some(i) = s.decls.iter().position(|d| d.name() == N_TEXCOORD0 + 1) else { continue };
        if let Elem::Float2(uv) = &s.elems[i] {
            for p in uv {
                for k in 0..2 {
                    lo[k] = lo[k].min(p[k] as f64);
                    hi[k] = hi[k].max(p[k] as f64);
                }
            }
        }
    }
    let (w, h) = (hi[0] - lo[0], hi[1] - lo[1]);
    if !(w.is_finite() && h.is_finite()) || w.max(h) < 1e-6 {
        return None;
    }
    let margin = 0.001;
    let s = (1.0 - 2.0 * margin) / w.max(h);
    if s <= 1.05 {
        return None; // already (nearly) filling the atlas
    }
    for mv in visuals.iter_mut() {
        let Some(st) = mv.visual.stream_mut() else { continue };
        let Some(i) = st.decls.iter().position(|d| d.name() == N_TEXCOORD0 + 1) else { continue };
        if let Elem::Float2(uv) = &mut st.elems[i] {
            for p in uv.iter_mut() {
                p[0] = (margin + (p[0] as f64 - lo[0]) * s) as f32;
                p[1] = (margin + (p[1] as f64 - lo[1]) * s) as f32;
            }
        }
    }
    Some((s, lo, [margin as f32, margin as f32, (margin + w * s) as f32, (margin + h * s) as f32]))
}

/// The union of every visual's TexCoord1 range: [min u, min v, max u, max v]; None without a uv1 stream.
pub fn uv1_bounds(visuals: &[super::merged::MergedVisual]) -> Option<[f32; 4]> {
    use super::vstream::{Elem, N_TEXCOORD0};
    let mut b = [f32::MAX, f32::MAX, f32::MIN, f32::MIN];
    let mut any = false;
    for mv in visuals {
        let Some(s) = mv.visual.stream() else { continue };
        let Some(i) = s.decls.iter().position(|d| d.name() == N_TEXCOORD0 + 1) else { continue };
        if let Elem::Float2(uv) = &s.elems[i] {
            for p in uv {
                b[0] = b[0].min(p[0]);
                b[1] = b[1].min(p[1]);
                b[2] = b[2].max(p[0]);
                b[3] = b[3].max(p[1]);
                any = true;
            }
        }
    }
    if any { Some(b) } else { None }
}

/// The lightmap texel scale word measured off the geometry: sqrt(Σ world
/// triangle area / Σ TexCoord1 triangle area) over every visual that carries a
/// TexCoord1 set and an index buffer — the crystal path's formula
/// (`bake.rs`), so every item declares the SAME metres-per-uv the lightmapper
/// needs for a uniform texel density across neighbouring items (the deck
/// tiles' visible borders, 2026-09-12). `None` when nothing measurable.
pub fn measured_u02(visuals: &[super::merged::MergedVisual]) -> Option<f32> {
    use super::vstream::{Elem, N_POSITION, N_TEXCOORD0};
    let (mut aworld, mut auv1) = (0.0f64, 0.0f64);
    for mv in visuals {
        let Some(ib) = mv.visual.index_buffer.as_ref() else { continue };
        let Some(s) = mv.visual.stream() else { continue };
        let get = |name: u32| s.decls.iter().zip(s.elems.iter()).find(|(d, _)| d.name() == name).map(|(_, e)| e);
        let (Some(Elem::Float3(pos)), Some(Elem::Float2(uv1))) = (get(N_POSITION), get(N_TEXCOORD0 + 1)) else { continue };
        for t in ib.indices.chunks_exact(3) {
            let (a, b, c) = (t[0] as usize, t[1] as usize, t[2] as usize);
            if a >= pos.len() || b >= pos.len() || c >= pos.len() || a >= uv1.len() || b >= uv1.len() || c >= uv1.len() {
                continue;
            }
            let au = ((uv1[b][0] - uv1[a][0]) as f64 * (uv1[c][1] - uv1[a][1]) as f64 - (uv1[c][0] - uv1[a][0]) as f64 * (uv1[b][1] - uv1[a][1]) as f64).abs() / 2.0;
            let e1 = [pos[b][0] - pos[a][0], pos[b][1] - pos[a][1], pos[b][2] - pos[a][2]];
            let e2 = [pos[c][0] - pos[a][0], pos[c][1] - pos[a][1], pos[c][2] - pos[a][2]];
            let cr = [e1[1] * e2[2] - e1[2] * e2[1], e1[2] * e2[0] - e1[0] * e2[2], e1[0] * e2[1] - e1[1] * e2[0]];
            let aw = ((cr[0] * cr[0] + cr[1] * cr[1] + cr[2] * cr[2]) as f64).sqrt() / 2.0;
            auv1 += au;
            aworld += aw;
        }
    }
    if auv1 > 1e-9 && aworld > 1e-9 { Some((aworld / auv1).sqrt() as f32) } else { None }
}
