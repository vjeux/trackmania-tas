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

use super::solid2::{CPlugSolid2Model, Material, PreLightGen, ShadedGeom};
use super::surface::{CPlugSurface, Surf, Triangle};
use super::visual::CPlugVisualIndexedTriangles;
use super::vstream::{Elem, N_COLOR0, N_NORMAL, N_POSITION, N_TANGENT_U, N_TANGENT_V, N_TEXCOORD0, T_DEC3N, T_FLOAT3};
use super::{Node, NodeRef, Ref, R};
use crate::crystal_model::CPlugMaterialUserInst;
use crate::geom::{apply, compose, Xform, IDENTITY};

/// What the built item is called and how big it is.
#[derive(Clone, Debug)]
pub struct BuildOpts {
    /// `Ident` path, e.g. `ZZZ_TinyBlocks\Tiny\Road\Tiny_Road_01.Item.Gbx`.
    pub ident: String,
    pub author: String,
    pub scale: f32,
    /// Collection id (26 = Stadium).
    pub collection: u32,
    /// Remap every material link onto the mesh-editor family (BlueBay
    /// embedded items: only `Editors\...` links are known to render there).
    pub editors: bool,
    /// The source model's CPlugGameSkin HEADER chunk (0x090F4000), copied
    /// verbatim: the declaration (`Any\Advertisement6x1\`, `*Image` slot)
    /// that makes the game paint the current in-game advertisement — the
    /// campaign artwork — onto the model's `Image` texture, and lets a
    /// placement's own skin file (a light colour) apply. Without it a screen
    /// draws the material's default yellow `RaceAd6x1.dds` panel.
    pub skin: Option<Vec<u8>>,
}

/// One source visual + the material slot it draws with.
#[derive(Clone, Debug)]
pub struct MergedVisual {
    pub visual: CPlugVisualIndexedTriangles,
    pub material: usize,
    /// The detail levels this visual draws at, as the SOURCE part's
    /// `ShadedGeom::lod_mask` (bit k = level k of that part's ladder, bit 0 =
    /// nearest; 0 = every level), and that part's ladder — its switch
    /// distances, already scaled (level k spans `ladder[k-1]..ladder[k]`,
    /// the level past the last distance is unbounded). The merged item's
    /// ladder is the union of its parts' distances, and `remap_lod_mask`
    /// moves the mask onto it by distance range at assembly.
    pub lod_mask: u32,
    pub lod_ladder: Vec<f32>,
}

impl MergedVisual {
    /// A visual drawn at every distance (a part without detail levels).
    pub fn every_level(visual: CPlugVisualIndexedTriangles, material: usize) -> MergedVisual {
        MergedVisual { visual, material, lod_mask: 0, lod_ladder: Vec::new() }
    }
}

/// `TINY_LOD0_ONLY=1`: the bake of before 2026-09-07 — keep only the nearest
/// detail level of every source model and write no switch distances (every
/// tiny item at full detail at every distance).
///
/// How the game reads the ladder (measured 2026-09-07 on Summer 15's grass
/// with `tmmaps lineup` probes of a half-size GateCheckpointCenter24m, ladder
/// [32, 64, 128]): a geom draws when its mask has the current level's bit —
/// an item whose geoms all lack bit 0 is invisible near, one with level-0
/// geoms only is culled far — and the level advances with the CAMERA
/// distance times the game's LOD bias: the level-0-only item was still drawn
/// at 100 m and gone at 150 m, i.e. the 32 m step fired between 100 and
/// 150 m (~x4; a stock ShowLights rig whose own ladder culls at 256 m was
/// still drawn at 400 m, so the bias is the game's, not ours — resolution
/// and quality settings of the render box). Both the item editor's
/// CGameCommonItemEntityModel form and the pack's CPlugPrefab form behave the
/// same, in the editor and in play. Nothing else was needed: the Solid2
/// header words (flags, u05, u07, vis_cst_type, 0x090BB002) match the pack's.
pub fn lod0_only() -> bool {
    std::env::var_os("TINY_LOD0_ONLY").is_some() || lod_pick().is_some()
}

/// `TINY_LOD_PICK=N`: like TINY_LOD0_ONLY but the single level kept is level
/// N (its geoms; a part without a level N keeps its nearest) — the size
/// lever for a map Nadeo refuses to store (Summer 21 at 36 MB: HTTP 413;
/// the visual meshes of the nearest level are a third of the bytes).
pub fn lod_pick() -> Option<u32> {
    std::env::var("TINY_LOD_PICK").ok().and_then(|v| v.parse().ok())
}

/// A source part's detail ladder length: one more level than it has switch
/// distances, and at least one past its highest mask bit (a mask bit with no
/// distance is the unbounded last level).
pub fn lod_levels_of(lod_max_dist: &[f32], geoms: &[ShadedGeom]) -> u32 {
    let by_dist = lod_max_dist.len() as u32 + 1;
    let by_mask = geoms.iter().map(|g| if g.lod_mask <= 0 { 1 } else { 32 - (g.lod_mask as u32).leading_zeros() }).max().unwrap_or(1);
    by_dist.max(by_mask)
}

/// Two switch distances that are the same step (the parts of one item scale
/// alike, so equal source distances stay equal; the slack absorbs float noise).
fn same_dist(a: f32, b: f32) -> bool {
    if a.is_infinite() || b.is_infinite() {
        return a == b;
    }
    (a - b).abs() <= 1e-3 * a.abs().max(b.abs()).max(1.0)
}

/// `d` merged into a sorted, deduplicated ladder.
pub fn merge_lod_ladder(ladder: &mut Vec<f32>, d: &[f32]) {
    for x in d {
        if !x.is_finite() || *x <= 0.0 || ladder.iter().any(|y| same_dist(*x, *y)) {
            continue;
        }
        let at = ladder.iter().position(|y| *y > *x).unwrap_or(ladder.len());
        ladder.insert(at, *x);
    }
}

/// A part's mask moved onto the merged ladder by DISTANCE RANGE: the part's
/// level k spans `part[k-1]..part[k]` (0 below the first distance, unbounded
/// past the last), and every merged level whose span STARTS inside that
/// range takes the bit. The merged ladder normally holds every distance of
/// every part, so the spans tile exactly: a part keeps drawing precisely
/// where its own ladder drew it, whatever the other parts' steps (Flag16m: a
/// pole with [16, 64, 512] and a cloth with [16, 64, 128, 512] merge into
/// [16, 64, 128, 512] with the pole's third level on bits 2 and 3 — the
/// per-level max of before made a zero-width level [16, 64, 512, 512]). When
/// the ladder was capped (`cap_lod_ladder`) a merged level may straddle a
/// part's step; the level active at its start is drawn through it. A part
/// without a ladder, or a mask of 0, draws at every level; a part whose
/// ladder is longer than its masks (Nadeo's cull idiom: Sparkler8m lists
/// [16, 128, 256] but draws nothing past bit 2) stays culled past its last
/// distance, since no level of it starts there.
pub fn remap_lod_mask(mask: u32, part: &[f32], merged: &[f32]) -> u32 {
    let levels = merged.len() + 1;
    let all = if levels >= 32 { u32::MAX } else { (1u32 << levels) - 1 };
    if mask == 0 || part.is_empty() {
        return all;
    }
    let mut out = 0u32;
    for k in 0..32usize {
        if mask & (1 << k) == 0 {
            continue;
        }
        let lo = if k == 0 { 0.0 } else { part.get(k - 1).copied().unwrap_or(f32::INFINITY) };
        let hi = part.get(k).copied().unwrap_or(f32::INFINITY);
        if lo >= hi {
            continue;
        }
        for j in 0..levels {
            let jlo = if j == 0 { 0.0 } else { merged[j - 1] };
            let starts_inside = (jlo > lo || same_dist(jlo, lo)) && jlo < hi && !same_dist(jlo, hi);
            if starts_inside {
                out |= 1 << j;
            }
        }
    }
    out
}

/// The most detail levels a static object's Solid2 may carry. Every pack
/// static model surveyed (93 laddered Solid2s, 2026-09-07) has at most 3
/// switch distances = 4 levels; the one 5-level model is a dyna object's
/// mesh (Flag.Mesh.Gbx, [16, 64, 128, 512]). A static item merged to 5
/// levels (Flag16m: pole + cloth, [8, 32, 64, 256]) crashed the client at
/// map load with an assertion (ud2 at Trackmania.exe+0x1e9947, rax = 5,
/// r9 = 4) — twice, once per ladder variant — so the merged ladder is capped
/// at 4 levels.
pub const MAX_LOD_LEVELS: usize = 4;

/// Collapse a ladder to at most `max_dists` switch distances: while it is
/// longer, the two closest adjacent steps (smallest ratio) become one, the
/// larger distance dropped — the finer level then draws on through the
/// removed step (see `remap_lod_mask`), which costs a little detail budget
/// rather than any geometry.
pub fn cap_lod_ladder(ladder: &mut Vec<f32>, max_dists: usize) {
    while ladder.len() > max_dists && ladder.len() >= 2 {
        let mut best = 1usize;
        let mut best_ratio = f32::INFINITY;
        for i in 1..ladder.len() {
            let r = ladder[i] / ladder[i - 1].max(1e-6);
            if r < best_ratio {
                best_ratio = r;
                best = i;
            }
        }
        ladder.remove(best);
    }
}

/// A light of the source model, ready for the item's Solid2 `lights` list.
#[derive(Clone, Debug)]
pub struct MergedLight {
    /// The socket as the pack Solid2 carried it, its iso (`u05`) moved into
    /// the item's scaled frame; `node` is filled at assembly.
    pub socket: super::solid2::Light,
    /// The CPlugLight (GxLight inline, pack references dropped, scaled).
    pub light: super::light::CPlugLight,
    /// The `.Light.Gbx` it came from, for the report.
    pub source: String,
    /// The pack files its dropped references named: (path, slot) with slot
    /// 0 = flare bitmap, 1 = projector bitmap, 2 = colour table, 3 = anim image.
    pub bitmaps: Vec<(String, u8)>,
}

/// One `.FxSys.Gbx` entity of the source prefab, ready to inline.
#[derive(Clone, Debug)]
pub struct FxPart {
    /// The pack path of the `.FxSys.Gbx`.
    pub path: String,
    /// The effect script; its emitters' `model` refs are EXTERNAL indices of
    /// the source file, resolved through `models`.
    pub fx: super::particle::CPlugFxSystem,
    /// (source node index, pack path, the parsed `.ParticleModel.Gbx` with its
    /// own externals — the texture the sub-model names).
    pub models: Vec<(u32, String, super::particle::ParticleNode, Vec<(u32, String)>)>,
    /// The textures those models name, for `TINY_FX_TEXTURE=archive`: (pack
    /// path of the `.Texture.gbx`, its CPlugBitmap parsed, the image file's
    /// bare name, the image bytes) — the bitmap rides inline, the image as
    /// `Items/<name>` in the library archive.
    pub textures: Vec<(String, super::particle::ParticleNode, String, Vec<u8>)>,
    /// The entity's pose in the item's unscaled frame.
    pub at: Xform,
}

/// The accumulator.
#[derive(Clone, Debug, Default)]
pub struct Merged {
    pub visuals: Vec<MergedVisual>,
    /// Deduplicated by (link, physics).
    pub materials: Vec<CPlugMaterialUserInst>,
    pub surf_vertices: Vec<[f32; 3]>,
    pub surf_triangles: Vec<Triangle>,
    pub surf_ids: Vec<u16>,
    pub pre_light_gen: Option<PreLightGen>,
    pub file_write_time: u64,
    /// A trigger surface carried over from a source item (scaled), and its
    /// waypoint type (chunk 2E00201F; 3 = none).
    pub trigger: Option<CPlugSurface>,
    /// Explicit waypoint type (0 start, 1 finish, 2 checkpoint, 4 start+finish);
    /// None writes 3 (not a waypoint). A start has a type and NO trigger.
    pub waypoint_type: Option<i32>,
    /// A gameplay gate's effect volume in the PREFAB form: the item is laid
    /// out like the pack's own (`CGameItemModel -> CPlugPrefab { static
    /// object, NPlugTrigger_SGateSpecial }`), this is the trigger surface of
    /// that second entity (item space, ids = physics | gameplay << 8).
    pub special: Option<CPlugSurface>,
    /// Entity-model iso translation: the spawn point for waypoint items
    /// (Granady: block spawn_loc x scale, e.g. RoadTechStart [16,2,16] ->
    /// [8,1,8]). Zero = identity.
    pub spawn: [f32; 3],
    /// Things skipped, for the report.
    pub notes: Vec<String>,
    /// The prefab's procedural vegetation entities (`.VegetTreeModel.Gbx`
    /// externals: no mesh to bake), as (model path, iso in the item's
    /// UNSCALED frame) — `tiny-library` re-emits them as stock tree items next
    /// to the block's item (a DecoLake shore carries hundreds of trees).
    pub veget: Vec<(String, Xform)>,
    /// The prefab's effect systems (`.FxSys.Gbx` entities: the Show items'
    /// smoke and sparks), each parsed with the particle models it drives,
    /// posed in the item's UNSCALED frame. `assemble` inlines them as
    /// entities of the prefab form (`add_fx_system`).
    pub fx: Vec<FxPart>,
    /// Light sources the source model carries (Solid2 `lights` +
    /// `light_insts`): how many were found (report).
    pub lights: usize,
    /// The lights, ready to embed: each pack socket with its iso composed
    /// into the item's frame (scaled) and the `.Light.Gbx` it named parsed,
    /// externals dropped, radii scaled.
    pub lights_out: Vec<MergedLight>,
    /// Sockets whose light file is still to be read from the store: (path,
    /// socket with the composed iso, scale). `resolve_pending_lights` drains it.
    pub pending_lights: Vec<(String, super::solid2::Light, f32)>,
    /// A gameplay gate's kind (`Turbo2`, `Boost`, …) from its own
    /// `<Kind>.TerrainModifier.Gbx`; the sign panels' logo (signlogo.rs).
    pub gate_kind: Option<String>,
    /// A DecoPlatform block: its `Deco` material is drawn as the coloured
    /// platform plastic (`PlatformTech`), the way the game shows it.
    pub deco_as_platform: bool,
    /// Picture files the item's custom-texture materials name, to ride in
    /// the library archive next to the item: (file name, DDS bytes).
    pub pictures: Vec<(String, Vec<u8>)>,
    /// The placement's light colour skin (light_skin.rs): multiplies every
    /// embedded light's colour, and re-dresses the `_I`-textured materials
    /// (`illum_links`) as swatch-lit custom materials.
    pub light_skin: Option<crate::light_skin::LightSkin>,
    /// Material links whose pack material carries a self-illumination
    /// (`*_I.Texture.gbx`) texture — the glass of a light item.
    pub illum_links: Vec<String>,
    /// Remap every material link onto the mesh-editor family (BlueBay).
    /// Do not split shared-id visuals by layer for this model (the Mangrove
    /// split crashes the client — open bug, minimal repro in var-m1).
    pub no_split: bool,
    /// The block's material modifier (blockinfo `material_modifier` ->
    /// `X.TerrainModifier.Gbx` -> folder `…\Modifier\X\`): every material
    /// link `Stadium\Media\Material\S` whose `S.Material.Gbx` exists in the
    /// folder is taken from there (PlatformDirt: the dirt-brown
    /// PlatformTech/TrackWall/Deco…; the gate specials likewise). Full links.
    pub modifier: Vec<String>,
    /// The modifier's re-dress of the prefab's COLLISION materials, from its
    /// GameSkin slot table (`tiny_library::modifier_collision_redress`): rows
    /// of (default material path, lower-cased; replacement link; (physics,
    /// gameplay)). A hull triangle whose surface material is a listed default
    /// takes the replacement's ids — the deck of a Boost/Reset/NoEngine
    /// platform special is authored as `CollisionTurbo*` (gameplay 1) and
    /// drove as a Turbo until this (Summer 24 cp10, 2026-09-08).
    pub collision_redress: Vec<(String, String, (u8, u8))>,
    /// An ITEM modifier names its materials with a suffix: the obstacle items
    /// (Summer 15's pushers and rotors) reference
    /// `Stadium\Media\Modifier\ItemObstacleLevel1.Gbx`, whose materials live
    /// in `…\Modifier\ItemObstacle\<stem>Level1.Material.Gbx` — the base
    /// `ItemObstaclePusher` is the grey "off" look, `…Level1` the orange
    /// active one. A material stem S is remapped to the folder entry named
    /// `S<suffix>`; block modifiers have no suffix.
    pub modifier_suffix: String,
    /// Keep `…\Material\Water` visuals (Stadium: the pool blocks draw their
    /// own water; the terrain collections regenerate theirs from the zone).
    /// `TINY_WATER=keep|drop` overrides.
    pub keep_water: bool,
    pub editors: bool,
    /// The source model's CPlugGameSkin header chunk (0x090F4000), verbatim
    /// — see `BuildOpts::skin`. Set from the pack item file or the block
    /// info file; the built item carries it in its own header.
    pub skin: Option<Vec<u8>>,
    /// The crystal bake built `surf_vertices`/`surf_triangles`/`surf_ids`
    /// itself (per-slot entries, trigger synthesis); skip the shared
    /// dedup-and-weld tail in `add_crystal`.
    pub surface_built: bool,
    /// The moving parts (a pusher's piston, a rotor's disc): kept apart from
    /// the static merge and emitted as `CPlugDynaObjectModel` entities of a
    /// prefab entity model, each with its kinematic constraint. Empty for a
    /// static item.
    pub dyna: Vec<DynaPart>,
    /// The item's detail ladder: switch distance of every level but the last
    /// (level k draws while the camera is within `lod_max_dist[k]`; the last
    /// level is unbounded), already SCALED by the item scale (a half-size
    /// object switches at half the distance — the same size on screen) and
    /// merged across the parts baked into the item as the UNION of their
    /// distances (each part's masks are moved onto it by distance range at
    /// assembly, `remap_lod_mask`; the union is capped at `MAX_LOD_LEVELS`
    /// there). Empty = one level, no switching.
    pub lod_max_dist: Vec<f32>,
    /// Keep every detail level of the source whatever `TINY_LOD0_ONLY` says,
    /// and never cap or fold its ladder — the tween cloth: ten or more
    /// instances of a LOD0-only copy drew as garbage (giant black sails, or
    /// nothing) while one or two drew right (2026-09-07); the pack's five
    /// levels are the layout the many-instance draw path expects.
    pub all_lods: bool,
    /// Solid2 fields carried from the source mesh for a tween part: the pack's
    /// Flag.Mesh.Gbx says `vis_cst_type` 2 (its vertices are not constant —
    /// the frames), `u07` -1; a static item says 1 and 1.
    pub vis_cst_type: Option<i32>,
    pub solid2_u07: Option<i32>,
    /// Write NO PreLightGen (the pack's dyna meshes — Flag.Mesh.Gbx — carry
    /// none; a static item always gets one).
    pub no_prelight: bool,
}

/// One `CPlugDynaObjectModel` entity of the source prefab, scaled: its mesh
/// merged on its own (in the object's local frame), its two hulls, the
/// constraint that moves it (translation range scaled), and the entity
/// params both carried from the pack.
#[derive(Clone, Debug)]
pub struct DynaPart {
    pub path: String,
    /// The entity's pose in the item frame: rotation as the game's quaternion
    /// (x, y, z, w), position already scaled.
    pub rot: [f32; 4],
    pub pos: [f32; 3],
    pub mesh: Merged,
    /// `MoveShape` — moves with the object.
    pub move_shape: Option<CPlugSurface>,
    /// `HitShape` — stays.
    pub hit_shape: Option<CPlugSurface>,
    pub model: super::dyna::CPlugDynaObjectModel,
    pub instance_params_id: i32,
    pub instance_params: Vec<u8>,
    /// The constraint that moves it, with its entity params — `None` for a
    /// part the pack drives without one (the flag cloth: its motion is the
    /// mesh's own vertex tween, played by the material).
    pub constraint: Option<(super::dyna::KinematicConstraint, super::dyna::ConstraintParams)>,
}

pub fn dec3n_unpack(v: u32) -> [f32; 3] {
    let c = |s: u32| -> f32 {
        let x = ((v >> s) & 0x3FF) as i32;
        let x = if x >= 512 { x - 1024 } else { x };
        x as f32 / 511.0
    };
    [c(0), c(10), c(20)]
}

pub fn dec3n_pack(n: [f32; 3]) -> u32 {
    let mut out = 0u32;
    for (k, x) in n.iter().enumerate() {
        // Truncation toward zero (C-style `(int)(x*511)`), NOT round:
        // fitted against Tiny_Road_17 (modefit: trunc beats round 612-298
        // on TSpecials, Sign 214/214 exact; axis-aligned components are
        // unaffected since their fractions are 0).
        let q = (x.clamp(-1.0, 1.0) * 511.0) as i32;
        out |= ((q & 0x3FF) as u32) << (10 * k);
    }
    out
}

fn rotate(m: &Xform, v: [f32; 3]) -> [f32; 3] {
    [
        m[0] * v[0] + m[3] * v[1] + m[6] * v[2],
        m[1] * v[0] + m[4] * v[1] + m[7] * v[2],
        m[2] * v[0] + m[5] * v[1] + m[8] * v[2],
    ]
}

fn is_identity_rotation(m: &Xform) -> bool {
    m[..9].iter().zip(IDENTITY[..9].iter()).all(|(a, b)| (a - b).abs() < 1e-6)
}

fn bbox(points: &[[f32; 3]]) -> [f32; 6] {
    let mut lo = [f32::MAX; 3];
    let mut hi = [f32::MIN; 3];
    for p in points {
        for k in 0..3 {
            lo[k] = lo[k].min(p[k]);
            hi[k] = hi[k].max(p[k]);
        }
    }
    if points.is_empty() {
        return [0.0; 6];
    }
    // A FLAT visual (the OpenTech road/zone `_FC_Ground` decals: 8 vertices in
    // one plane, half-height 1e-7) is dropped by the EDITOR on re-save (122
    // placements of Summer 09 gone after an editor SaveMap, 2026-09-07) while
    // play mode draws it — a degenerate box reads as an empty item. 2 cm of
    // half-extent on every axis keeps the box a box.
    let half = |k: usize| ((hi[k] - lo[k]) / 2.0).max(0.02);
    [(lo[0] + hi[0]) / 2.0, (lo[1] + hi[1]) / 2.0, (lo[2] + hi[2]) / 2.0, half(0), half(1), half(2)]
}

/// Transform a visual's inline vertex stream in place: positions by `iso`
/// then `scale`, packed normals/tangents by the rotation; bounding box
/// recomputed.
pub fn transform_visual(v: &mut CPlugVisualIndexedTriangles, iso: &Xform, scale: f32) -> R<()> {
    let m = v.main.as_mut().ok_or("visual without chunk 0x0900600F")?;
    let stream = match m.vertex_streams.first_mut().and_then(|r| r.inline.as_deref_mut()) {
        Some(Node::VertexStream(s)) => s,
        _ => return Err("visual without an inline vertex stream".into()),
    };
    let compress = stream.compress_local3d.unwrap_or(false);
    let rot = !is_identity_rotation(iso);
    let mut positions = Vec::new();
    for (d, e) in stream.decls.iter().zip(stream.elems.iter_mut()) {
        match (d.name(), d.stored_type(compress), e) {
            (N_POSITION, T_FLOAT3, Elem::Float3(p)) => {
                for q in p.iter_mut() {
                    let t = apply(iso, *q);
                    *q = [t[0] * scale, t[1] * scale, t[2] * scale];
                }
                positions = p.clone();
            }
            (N_POSITION, _, _) => return Err("packed (Dec3N) positions cannot be transformed".into()),
            (N_NORMAL | N_TANGENT_U | N_TANGENT_V, T_DEC3N, Elem::Word(w)) if rot => {
                for x in w.iter_mut() {
                    *x = dec3n_pack(rotate(iso, dec3n_unpack(*x)));
                }
            }
            (N_NORMAL | N_TANGENT_U | N_TANGENT_V, T_FLOAT3, Elem::Float3(p)) if rot => {
                for q in p.iter_mut() {
                    *q = rotate(iso, *q);
                }
            }
            _ => {}
        }
    }
    m.bounding_box = bbox(&positions);
    Ok(())
}

/// Remap every position of the merged item (visual streams and collision
/// vertices) through `f`; bounding boxes recomputed. Visuals with packed
/// positions are left alone (they never reach a merged item: transform_visual
/// refused them first).
pub fn remap_positions(m: &mut Merged, f: &dyn Fn([f32; 3]) -> [f32; 3]) {
    for mv in &mut m.visuals {
        let Some(main) = mv.visual.main.as_mut() else { continue };
        let Some(Node::VertexStream(stream)) = main.vertex_streams.first_mut().and_then(|r| r.inline.as_deref_mut()) else { continue };
        let compress = stream.compress_local3d.unwrap_or(false);
        let mut positions = Vec::new();
        for (d, e) in stream.decls.iter().zip(stream.elems.iter_mut()) {
            if let (N_POSITION, T_FLOAT3, Elem::Float3(p)) = (d.name(), d.stored_type(compress), e) {
                for q in p.iter_mut() {
                    *q = f(*q);
                }
                positions = p.clone();
            }
        }
        if !positions.is_empty() {
            main.bounding_box = bbox(&positions);
        }
    }
    for v in &mut m.surf_vertices {
        *v = f(*v);
    }
}

/// The sea floor keeps its depth. A shore tile at the water row (BlueBay's
/// Beach) slopes from the sand through the shallows down to a sea-floor apron
/// a cell wide; halved with the rest of the tile that apron sits at half its
/// depth, and the water over it turns dark: a rectangle of shaded sea around
/// every island, one tiny cell wide, where the original shows open sea
/// (Summer 06 start island, top-down). Below `water` (the surface, in the
/// item's scaled frame) the first `keep` metres keep the tile's scale — the
/// visible shallows — and every metre beyond regains its source depth
/// (divided by `scale`), so the apron sinks back to where the water hides it.
pub fn restore_depth(m: &mut Merged, water: f32, keep: f32, scale: f32) {
    remap_positions(m, &|p| {
        let depth = water - p[1];
        if depth <= keep {
            p
        } else {
            [p[0], water - keep - (depth - keep) / scale, p[2]]
        }
    });
}

impl Merged {
    /// Slot of a game material, adding it when new.
    pub fn material_slot(&mut self, link: &str, physics: u8) -> usize {
        // A gameplay gate's sign panels (signlogo.rs): every panel of the item,
        // whatever its game material, shares ONE slot per kind — the picture
        // material — so the row and the beam square are one draw, not two
        // equal materials (which the format rules refuse).
        let pseudo;
        let (link, physics) = match super::signlogo::enabled().then(|| super::signlogo::sign_kind(link, self.gate_kind.as_deref())).flatten() {
            Some(kind) => {
                pseudo = super::signlogo::pseudo_link(&kind);
                (pseudo.as_str(), 32u8)
            }
            None => (link, physics),
        };
        // a DecoPlatform block's `Deco` is the coloured platform plastic
        let (link, physics) = if self.deco_as_platform && link == "Stadium\\Media\\Material\\Deco" { ("Stadium\\Media\\Material\\PlatformTech", 16u8) } else { (link, physics) };
        let modified;
        let link = match link.strip_prefix("Stadium\\Media\\Material\\") {
            Some(stem) if !self.modifier.is_empty() => {
                let by_stem = |want: &str| self.modifier.iter().find(|m| m.rsplit('\\').next().map(|t| t.strip_suffix(self.modifier_suffix.as_str())) == Some(Some(want))).cloned();
                // The gameplay-gate prefabs (Special8m/16m/24m/32m) are
                // authored in their Turbo dress; a gate item's
                // `<Kind>.TerrainModifier.Gbx` folder names the same pieces
                // without the kind (`Modifier\Boost\{Sign,SignOff,SpecialFX,
                // TriggerFX,Decal…}`) — the editor's own bake resolves
                // `SpecialSignTurbo` to `Modifier\Turbo\Sign` (MATERIAL_LINK_RESOLVE).
                match by_stem(stem).or_else(|| gate_special_stem(stem).and_then(|s| by_stem(s))) {
                    Some(m) => {
                        modified = m;
                        modified.as_str()
                    }
                    None => link,
                }
            }
            _ => link,
        };
        // the modifier's own physics when the table knows it (PlatformDirt\PlatformTech = Dirt 6)
        let physics = if link.contains("\\Modifier\\") { physics_for_link(link).unwrap_or(physics) } else { physics };
        if let Some(i) = self.materials.iter().position(|m| m.link() == Some(link) && m.physics() == physics) {
            return i;
        }
        self.materials.push(CPlugMaterialUserInst::game_material(link, physics));
        self.materials.len() - 1
    }

    /// Slot of a link, remapped onto the editor family when asked.
    pub fn link_slot(&mut self, link: &str, physics: u8, editors: bool) -> usize {
        let link = if editors { crate::tiny_assets::editors_link_for_stadium_material(link.rsplit('\\').next().unwrap_or(link)).to_string() } else { link.to_string() };
        self.material_slot(&link, physics)
    }

    /// The plain (`_Ids`, no decal) variant of a BlueBay terrain layer material:
    /// `…\TransitionToLand` / `…\TransitionRocksToLand` -> `…\Land`, likewise
    /// Sand, SeaFloor, CliffPxz, RocksTop. Anything else is returned as is.
    pub fn plain_variant_slot(&mut self, slot: usize) -> usize {
        let Some(link) = self.materials.get(slot).and_then(|m| m.link()).map(|s| s.to_string()) else { return slot };
        let phys = self.materials[slot].physics();
        let (dir, stem) = match link.rfind('\\') {
            Some(i) => (&link[..=i], &link[i + 1..]),
            None => ("", link.as_str()),
        };
        if !stem.starts_with("Transition") {
            return slot;
        }
        let Some(i) = stem.rfind("To") else { return slot };
        let layer = &stem[i + 2..];
        if !matches!(layer, "Land" | "Sand" | "SeaFloor" | "CliffPxz" | "RocksTop" | "HillPxz") {
            return slot;
        }
        let plain = format!("{dir}{layer}");
        self.material_slot(&plain, phys)
    }

    /// Slot of a material instance copied from a source, deduplicated by its
    /// whole LOOK — link, physics and every constant — not by link alone.
    ///
    /// A mesh-modeler item (the club's TME nation items, Summer 21–25) dresses
    /// its parts in one game material many times over with a different
    /// `TargetColor` constant per part: the Guanako is seven
    /// `Material_BlockCustom\CustomPlastic` slots (fur, belly, hooves…), each
    /// with its own `color`. Keyed by link + physics they collapsed into the
    /// first slot's colour (2026-09-08: the whole animal in one plastic). Two
    /// slots merge only when everything but their author-side names agrees.
    ///
    /// `folder` is the source Solid2's `materials_folder`: a modeler material
    /// (`is_using_game_material` false, a bare `TechnicsTrims`) names its file
    /// relative to it. The copy is rewritten as the equivalent game material
    /// with the full link (`Stadium\Media\Material\TechnicsTrims`) — the form
    /// every other baked item uses and `item-check` can resolve.
    pub fn material_inst_slot(&mut self, inst: &CPlugMaterialUserInst, folder: &str) -> usize {
        let mut owned = inst.clone();
        if let Some(main) = owned.main.as_mut() {
            if !main.is_using_game_material && main.version >= 11 {
                if let Some(bare) = main.link.as_str().filter(|l| !l.is_empty() && !l.contains('\\')).map(|s| s.to_string()) {
                    let folder = if folder.is_empty() { "Stadium\\Media\\Material\\" } else { folder };
                    let full = format!("{}{bare}", folder.strip_suffix('\\').map(|f| format!("{f}\\")).unwrap_or_else(|| folder.to_string()));
                    main.link = crate::crystal_model::Id::Str(full);
                    main.is_using_game_material = true;
                }
            }
        }
        if let Some(i) = self.materials.iter().position(|m| same_look(m, &owned)) {
            return i;
        }
        self.materials.push(owned);
        self.materials.len() - 1
    }

    /// Slot for a crystal material instance with the editor-resolved link
    /// ([`resolve_crystal_link`]; deduplicated by resolved link + physics).
    pub fn resolved_inst_slot(&mut self, inst: &CPlugMaterialUserInst) -> usize {
        let link = resolve_crystal_link(inst.link().unwrap_or("")).to_string();
        if let Some(i) = self.materials.iter().position(|m| m.link() == Some(link.as_str()) && m.physics() == inst.physics()) {
            return i;
        }
        let mut owned = inst.clone();
        if let Some(main) = owned.main.as_mut() {
            main.link = crate::crystal_model::Id::Str(link);
        }
        self.materials.push(owned);
        self.materials.len() - 1
    }

    pub fn surf_id_slot(&mut self, physics: u16) -> i16 {
        if let Some(i) = self.surf_ids.iter().position(|x| *x == physics) {
            return i as i16;
        }
        self.surf_ids.push(physics);
        (self.surf_ids.len() - 1) as i16
    }

    /// Append collision triangles (already in source space).
    pub fn add_surface_mesh(&mut self, vertices: &[[f32; 3]], triangles: &[Triangle], iso: &Xform, scale: f32) {
        let base = self.surf_vertices.len() as u32;
        for v in vertices {
            let t = apply(iso, *v);
            self.surf_vertices.push([t[0] * scale, t[1] * scale, t[2] * scale]);
        }
        for t in triangles {
            // The u16 list entry is physics | gameplay << 8 (Z_Mini_Pltf_Flat_Turbo1_Grass:
            // Green (76) with Turbo (1) is listed as 332).
            let si = self.surf_id_slot(t.material_id as u16 | ((t.u03 as u16) << 8));
            self.surf_triangles.push(Triangle { indices: [t.indices[0] + base, t.indices[1] + base, t.indices[2] + base], material_id: t.material_id, u03: t.u03, surface_index: si });
        }
    }

    /// The block modifier's re-dress of a hull (`collision_redress`): every
    /// triangle whose surface material — `sf.materials[t.surface_index]`, an
    /// external `.Material.Gbx` resolved through `resolve` — is one of the
    /// modifier GameSkin's Collision* defaults takes the replacement's
    /// (physics, gameplay); the prefab's `Effects\Media\Material\CollisionTurbo`
    /// deck (Concrete, Turbo 1) under a Reset modifier becomes
    /// `Modifier\Reset\Collision` (Concrete, Reset 8), `CollisionTurboGreen`
    /// under Boost becomes `Modifier\Boost\CollisionGrass` (Green, ReactorBoost
    /// 12) — what the game does to the block. None when no triangle matched.
    fn redress_collision(&mut self, sf: &CPlugSurface, triangles: &[Triangle], resolve: &mut MaterialResolver) -> Option<Vec<Triangle>> {
        if self.collision_redress.is_empty() {
            return None;
        }
        let mut by_index: Vec<Option<(String, String, (u8, u8))>> = Vec::new();
        for sm in &sf.materials {
            let hit = match sm {
                super::surface::SurfMaterial::Node(nr) if nr.inline.is_none() && nr.index >= 0 => resolve(nr.index).and_then(|(path, _, _)| {
                    let low = path.to_ascii_lowercase();
                    self.collision_redress.iter().find(|(d, _, _)| *d == low).map(|(_, link, ids)| (path.clone(), link.clone(), *ids))
                }),
                _ => None,
            };
            by_index.push(hit);
        }
        if by_index.iter().all(|h| h.is_none()) {
            return None;
        }
        let mut out = triangles.to_vec();
        let mut counts: std::collections::BTreeMap<usize, usize> = std::collections::BTreeMap::new();
        for t in out.iter_mut() {
            let si = t.surface_index.max(0) as usize;
            if let Some(Some((_, _, ids))) = by_index.get(si) {
                t.material_id = ids.0;
                t.u03 = ids.1;
                *counts.entry(si).or_default() += 1;
            }
        }
        if counts.is_empty() {
            return None;
        }
        for (si, n) in counts {
            if let Some(Some((path, link, ids))) = by_index.get(si) {
                let file = path.rsplit('\\').next().unwrap_or(path);
                self.notes.push(format!("hull: {n} triangles of {file} re-dressed by the modifier as {link} (physics {}, gameplay {})", ids.0, ids.1));
            }
        }
        Some(out)
    }
}

/// Resolves an external node (by node index) to (path, link, physics).
pub type MaterialResolver<'a> = dyn FnMut(i32) -> Option<(String, String, u8)> + 'a;

impl Merged {
    /// Slot for a pre-UserInst material: the first non-shared
    /// `.Material.Gbx` it stands for gives the link (shared Techno3 id bases
    /// match first in file order); its SurfaceId, the library table, then the
    /// object's most common collision physics give the physics.
    pub fn old_material_slot(&mut self, om: &super::oldmat::OldMaterial, resolve: &mut MaterialResolver, common: Option<u8>) -> Option<usize> {
        let mut found = None;
        let mut shared = None;
        for ri in &om.refs {
            if let Some((path, _, _)) = resolve(*ri) {
                if path.to_ascii_lowercase().ends_with(".material.gbx") {
                    if path.to_ascii_lowercase().contains("techno3") {
                        if shared.is_none() {
                            shared = Some(material_link(&path));
                        }
                    } else {
                        found = Some(material_link(&path));
                        break;
                    }
                }
            }
        }
        match found.or(shared) {
            Some(link) => {
                let mut phys = if om.physics != 0 { om.physics } else { 0 };
                if phys == 0 {
                    phys = physics_for_link(&link).or(common).unwrap_or(0);
                }
                Some(self.link_slot(&link, phys, self.editors))
            }
            None => {
                self.notes.push("old material with no .Material.Gbx ref".into());
                None
            }
        }
    }

    /// Add every visual and the collision shape of one static object, placed
    /// by `iso` and scaled.
    pub fn add_static_object(&mut self, so: &super::item::CPlugStaticObjectModel, iso: &Xform, scale: f32, resolve: &mut MaterialResolver) -> R<()> {
        let iso_entity_only = *iso;
        let s2 = so.solid2().ok_or("static object without an inline CPlugSolid2Model")?;
        // The part's detail ladder. Every pack model measured (2026-09-07:
        // RoadTech Straight_Air [64, 128] + masks 1/2/4, the gate prefabs
        // [64, 128, 256] + masks 1/2/4/8, Beach Base1A [64] + masks 1/2)
        // lists one distance per level but the last: level k draws while the
        // camera is within lod_max_dist[k], the last level beyond. Scaled by
        // the item scale and merged into the item's ladder (the union of the
        // parts' distances; each part's masks are moved onto it by range at
        // assembly, `remap_lod_mask`) — see `Merged::lod_max_dist`.
        // `TINY_LOD0_ONLY=1` keeps the nearest level only, no ladder.
        let lod0_only = lod0_only() && !self.all_lods;
        let part_levels = if lod0_only { 1 } else { lod_levels_of(&s2.lod_max_dist, &s2.shaded_geoms) };
        let mut part_ladder: Vec<f32> = Vec::new();
        if part_levels > 1 {
            let before = self.lod_max_dist.clone();
            // a ladder shorter than its masks (a mask bit past the last
            // distance): the extra levels take the last distance doubled
            let mut dists: Vec<f32> = s2.lod_max_dist.iter().map(|d| d * scale).collect();
            while (dists.len() as u32) + 1 < part_levels {
                let last = dists.last().copied().unwrap_or(32.0 * scale);
                dists.push(last * 2.0);
            }
            merge_lod_ladder(&mut part_ladder, &dists);
            merge_lod_ladder(&mut self.lod_max_dist, &dists);
            if self.lod_max_dist != before {
                self.notes.push(format!("lod ladder: {} levels, distances {:?} (x{scale}); item ladder now {:?}", part_levels, s2.lod_max_dist, self.lod_max_dist));
            }
        }
        // Which geoms are coarser levels only (no bit 0). Kept with their
        // masks unless `TINY_LOD0_ONLY`.
        let coarser = |g: &ShadedGeom| g.lod_mask > 0 && g.lod_mask & 1 == 0;
        // Light sources: a Solid2 `lights` socket = a name, an Iso4 in model
        // space and (in the packs) an EXTERNAL CPlugLight ref — Lamp.Mesh.Gbx
        // carries `Stadium\Media\Light\ItemLampSpot.Light.Gbx`. The socket's
        // iso is composed into the item's frame like a visual's vertices
        // (rotation kept, translation scaled); the light file is read when
        // the caller has the store (`resolve_pending_lights`), inline lights
        // are taken as they are.
        self.lights += s2.lights.len() + s2.light_insts.len();
        for l in &s2.lights {
            let mut socket = l.clone();
            // The light's frame in the item: its POSITION follows the geometry
            // (entity iso applied to the socket's translation), but its
            // ROTATION composes with the entity chain's rotation INVERTED —
            // measured 2026-09-07 on Summer 09's grass with `tmmaps lineup`:
            // the Lamp (entity = identity) lights the ground south of its post
            // with the socket rotation read column-major, beam along local -Z
            // (a Lamp light forced to identity beams south, to Rx(-90) beams
            // straight down, to Rx(+90) beams up); the LightsFront rig's eight
            // spots (entities rotated 90 and 115 degrees about X) only lit the
            // grass like the stock rig with the entity rotation transposed —
            // entity*socket sent them north/up, nothing on the ground.
            let mut iso = compose(iso, &l.u05);
            let et = [iso_entity_only[0], iso_entity_only[3], iso_entity_only[6], iso_entity_only[1], iso_entity_only[4], iso_entity_only[7], iso_entity_only[2], iso_entity_only[5], iso_entity_only[8], 0.0, 0.0, 0.0];
            let r = compose(&et, &[l.u05[0], l.u05[1], l.u05[2], l.u05[3], l.u05[4], l.u05[5], l.u05[6], l.u05[7], l.u05[8], 0.0, 0.0, 0.0]);
            iso[..9].copy_from_slice(&r[..9]);
            socket.u05 = iso;
            for k in 9..12 {
                socket.u05[k] *= scale;
            }
            socket.node = super::null_ref();
            match l.node.inline.as_deref() {
                Some(super::Node::Light(light)) if l.u02 => {
                    let mut light = light.clone();
                    light.drop_external_refs();
                    if let Some(super::Node::GxLight(g)) = light.gx_mut().and_then(|r| r.inline.as_deref_mut()) {
                        g.scale(scale);
                    }
                    self.lights_out.push(MergedLight { socket, light, source: "(inline)".into(), bitmaps: Vec::new() });
                }
                _ if l.u02 && l.node.index >= 0 => match resolve(l.node.index) {
                    Some((path, _, _)) => self.pending_lights.push((path, socket, scale)),
                    None => self.notes.push(format!("light socket {:?}: external node {} unnamed; light dropped", l.u01, l.node.index)),
                },
                _ => self.notes.push(format!("light socket {:?} names no light node ({:?}); dropped", l.u01, l.u04)),
            }
        }
        if !s2.light_user_models.is_empty() {
            self.notes.push(format!("{} light user models / {} light insts not carried over", s2.light_user_models.len(), s2.light_insts.len()));
        }
        if self.pre_light_gen.is_none() {
            self.pre_light_gen = s2.pre_light_gen.clone();
        }
        if self.file_write_time == 0 {
            self.file_write_time = s2.file_write_time;
        }
        // Material slot per source material index. Surface votes only ever
        // replace pre-UserInst (old) materials: shared Techno3 id bases
        // carry no look, so the coincident surface material wins over direct
        // resolution. UserInst links are never overridden (validated 1:1).
        let (votes, smap) = surface_votes(so, s2, resolve);
        let mut slots: Vec<Option<usize>> = Vec::new();
        if !s2.custom_materials.is_empty() {
            for (mi, m) in s2.custom_materials.iter().enumerate() {
                let is_old = matches!(m.node.as_ref().and_then(|r| r.inline.as_deref()), Some(super::Node::OldMaterial(_))) && m.inst().is_none() && m.name.is_empty();
                if is_old {
                    if let Some((link, phys)) = votes.get(mi).and_then(|v| v.clone()) {
                        slots.push(Some(self.link_slot(&link, phys, self.editors)));
                        continue;
                    }
                }
                slots.push(match m.inst() {
                    Some(inst) => {
                        if self.editors {
                            let link = inst.link().unwrap_or("").to_string();
                            let stem = link.rsplit('\\').next().unwrap_or(&link);
                            Some(self.material_slot(crate::tiny_assets::editors_link_for_stadium_material(stem), inst.physics()))
                        } else {
                            Some(self.material_inst_slot(inst, &s2.materials_folder))
                        }
                    }
                    None if !m.name.is_empty() => Some(self.link_slot(&m.name, 0, self.editors)),
                    None => match m.node.as_ref().and_then(|r| r.inline.as_deref()) {
                        Some(super::Node::OldMaterial(om)) => {
                            let om = om.clone();
                            let common = most_common_physics(so);
                            self.old_material_slot(&om, resolve, common)
                        }
                        _ => None,
                    },
                });
            }
        } else {
            for (mi, r) in s2.materials.iter().enumerate() {
                let is_old = matches!(r.inline.as_deref(), Some(super::Node::OldMaterial(_)));
                if is_old {
                    if let Some((link, phys)) = votes.get(mi).and_then(|v| v.clone()) {
                        slots.push(Some(self.link_slot(&link, phys, self.editors)));
                        continue;
                    }
                }
                // Inline pre-UserInst materials (BlueBay terrain) resolve
                // through their own `.Material.Gbx` refs, like above.
                let inline_old = match r.inline.as_deref() {
                    Some(super::Node::OldMaterial(om)) => {
                        let om = om.clone();
                        let common = most_common_physics(so);
                        self.old_material_slot(&om, resolve, common)
                    }
                    _ => None,
                };
                if inline_old.is_some() {
                    slots.push(inline_old);
                    continue;
                }
                slots.push(match resolve(r.index).map(|(_, l, p)| (l, p)) {
                    Some((link, phys)) => Some(self.link_slot(&link, phys, self.editors)),
                    None => {
                        self.notes.push(format!("material node {} unresolved", r.index));
                        None
                    }
                });
            }
        }
        // Per source material index: the slot came from a surface vote, i.e. a
        // shared Techno3 id material whose look the collision surface decides
        // per triangle (see the split below).
        let voted: Vec<bool> = (0..slots.len())
            .map(|mi| {
                let is_old = if !s2.custom_materials.is_empty() {
                    s2.custom_materials.get(mi).map(|m| matches!(m.node.as_ref().and_then(|r| r.inline.as_deref()), Some(super::Node::OldMaterial(_))) && m.inst().is_none() && m.name.is_empty()).unwrap_or(false)
                } else {
                    s2.materials.get(mi).map(|r| matches!(r.inline.as_deref(), Some(super::Node::OldMaterial(_)))).unwrap_or(false)
                };
                is_old && votes.get(mi).map(|v| v.is_some()).unwrap_or(false)
            })
            .collect();
        let mut visual_slots: Vec<(usize, usize)> = Vec::new();
        if std::env::var_os("TINY_DUMP_DECLS").is_some() {
            self.notes.push(format!(
                "solid2 v{} material_ids {:?} folder {:?} u03 {:?} u04 {:?} lod_max_dist {:?} vis_cst_type {} damage_zone {} flags {:#x} u05 {} u07 {} u11 {} u13 {} u15 {} u16 {} u18 {} u10 {:?} u12 {:?} u14 {} u17 {:?} u19 {:?} joints {:?} chunks {:x?} raw {:?} prelight {:?} geoms {:?} material refs {:?}",
                s2.version,
                s2.material_ids,
                s2.materials_folder,
                s2.u03,
                s2.u04,
                s2.lod_max_dist,
                s2.vis_cst_type,
                s2.damage_zone,
                s2.flags,
                s2.u05,
                s2.u07,
                s2.u11,
                s2.u13,
                s2.u15,
                s2.u16,
                s2.u18,
                s2.u10,
                s2.u12,
                s2.u14.index,
                s2.u17,
                s2.u19,
                s2.joints,
                s2.chunks,
                s2.raw.iter().map(|c| format!("{:08x}:{}", c.id, c.payload.iter().map(|b| format!("{b:02x}")).collect::<String>())).collect::<Vec<_>>(),
                s2.pre_light_gen.as_ref().map(|p| (p.version, p.u01, p.u02, p.u03, p.u04, p.sprite_count, p.boxes.len(), p.uv_groups.len())),
                s2.shaded_geoms.iter().map(|g| (g.visual_index, g.material_index, g.u01, g.lod_mask, g.u02)).collect::<Vec<_>>(), s2.materials.iter().map(|r| r.index).collect::<Vec<_>>()
            ));
        }
        // Prefab-wide layer table for the voted (shared Techno3 id) materials:
        // vertex layer id -> material slot, voted from the collision material
        // under the triangles whose three vertices carry that id (see the
        // split below). One table per static object: an id means the same
        // layer in every visual of the prefab (Beach: 0 Land, 1 SeaFloor,
        // 2 Sand; LandHill: 1 HillPxz; LandCliff: 1 CliffPxz).
        let mut layer_votes: std::collections::BTreeMap<u32, std::collections::BTreeMap<usize, usize>> = Default::default();
        if !smap.is_empty() && !self.no_split && std::env::var_os("TINY_NO_SPLIT").is_none() {
            for g in &s2.shaded_geoms {
                // the table is voted from the nearest level only: the collision
                // mesh coincides with LOD0's triangles, not with a coarser sheet's
                if coarser(g) {
                    continue;
                }
                let mi = g.material_index.max(0) as usize;
                if !voted.get(mi).copied().unwrap_or(false) {
                    continue;
                }
                let Some(Node::Visual(vis)) = s2.visuals.get(g.visual_index as usize).and_then(|r| r.inline.as_deref()) else { continue };
                let Some(eff) = effective_layer_ids(vis) else { continue };
                let (pos, idx) = visual_triangles(vis);
                for t in idx.chunks(3) {
                    if t.len() < 3 {
                        continue;
                    }
                    let (a, b, c) = (eff[t[0] as usize], eff[t[1] as usize], eff[t[2] as usize]);
                    if a != b || b != c {
                        continue;
                    }
                    let mut k = [mm(&pos[t[0] as usize]), mm(&pos[t[1] as usize]), mm(&pos[t[2] as usize])];
                    k.sort();
                    if let Some((link, phys)) = smap.get(&k) {
                        let slot = self.link_slot(link, *phys, self.editors);
                        *layer_votes.entry(a).or_default().entry(slot).or_default() += 1;
                    }
                }
            }
        }
        let layer_table: std::collections::BTreeMap<u32, usize> = layer_votes.into_iter().filter_map(|(id, votes)| votes.into_iter().max_by_key(|(_, n)| *n).map(|(m, _)| (id, m))).collect();
        if !layer_table.is_empty() {
            self.notes.push(format!("layer table {}", layer_table.iter().map(|(id, m)| format!("{id:x}->{}", self.materials[*m].link().unwrap_or("?").rsplit('\\').next().unwrap_or("?"))).collect::<Vec<_>>().join(" ")));
        }
        for g in &s2.shaded_geoms {
            // A geom's lod mask says which detail levels draw it (bit 0 =
            // nearest). Every level is kept with its mask and the ladder's
            // switch distances are written (scaled), so a coarser sheet only
            // draws once the camera is far enough — before 2026-09-07 the
            // item carried no distances and every level drew at once,
            // coplanar (Beach: a coarse 22-vertex LOD1 sheet voted SeaFloor
            // over the LOD0 grass = the "wide bright beaches with hard
            // seams"), which is why the nearest level alone was kept;
            // `TINY_LOD0_ONLY=1` restores that bake.
            if lod0_only {
                // the one level kept: N of TINY_LOD_PICK when the part has it, else the nearest
                // TINY_LOD_PICK_MIN_VERTS=N: a part whose nearest level has fewer than N
                // vertices keeps that level (small parts stay sharp; only the heavy
                // ones — a 50 000-vertex gate arch — go one level coarser)
                let level0_verts: i32 = s2.shaded_geoms.iter().filter(|h| h.lod_mask == 0 || h.lod_mask & 1 != 0).filter_map(|h| s2.visuals.get(h.visual_index as usize).and_then(|r| r.inline.as_deref())).filter_map(|n| if let Node::Visual(v) = n { v.main.as_ref().map(|m| m.count) } else { None }).sum();
                let min_verts: i32 = std::env::var("TINY_LOD_PICK_MIN_VERTS").ok().and_then(|v| v.parse().ok()).unwrap_or(0);
                let pick = lod_pick().filter(|_| level0_verts >= min_verts).filter(|p| s2.shaded_geoms.iter().any(|h| h.lod_mask & (1 << p) != 0)).unwrap_or(0);
                let keep = g.lod_mask == 0 || g.lod_mask & (1 << pick) != 0;
                if !keep {
                    self.notes.push(format!("visual {} (lod mask {}) skipped: not level {pick} (TINY_LOD0_ONLY/TINY_LOD_PICK)", g.visual_index, g.lod_mask));
                    continue;
                }
            }
            let lod = (g.lod_mask.max(0) as u32, part_ladder.clone());
            let vis = match s2.visuals.get(g.visual_index as usize).and_then(|r| r.inline.as_deref()) {
                Some(Node::Visual(v)) => v,
                _ => {
                    self.notes.push(format!("shaded geom visual {} is not an inline visual", g.visual_index));
                    continue;
                }
            };
            let mi = g.material_index.max(0) as usize;
            let mut mat = slots.get(mi).copied().flatten().unwrap_or_else(|| self.material_slot("Stadium\\Media\\Material\\PlatformTech", 0));
            let mut is_voted = voted.get(mi).copied().unwrap_or(false);
            // A coarser level's sheet under a shared Techno3 id material that
            // no collision triangle coincides with (Beach Base1A's LOD1 slope,
            // 25 vertices: its triangles are coarser than the collision's) has
            // no vote of its own and would resolve to the id-pass link; it
            // paints its layers per vertex like LOD0 does, so the prefab's
            // layer table (voted from LOD0) dresses it, starting from the
            // table entry of its most common layer id.
            let is_id_pass = |m: &Merged, slot: usize| m.materials.get(slot).and_then(|m| m.link()).map(|l| l.contains("_Ids")).unwrap_or(false);
            if !is_voted && coarser(g) && is_id_pass(self, mat) && !layer_table.is_empty() && !self.no_split && std::env::var_os("TINY_NO_SPLIT").is_none() {
                if let Some(eff) = effective_layer_ids(vis) {
                    let mut counts: std::collections::BTreeMap<u32, usize> = Default::default();
                    for id in &eff {
                        if layer_table.contains_key(id) {
                            *counts.entry(*id).or_default() += 1;
                        }
                    }
                    if let Some((id, _)) = counts.into_iter().max_by_key(|(_, n)| *n) {
                        mat = layer_table[&id];
                        is_voted = true;
                        self.notes.push(format!("visual {} (lod mask {}): unvoted shared-id material dressed from the layer table (id {id:x} -> {})", g.visual_index, g.lod_mask, self.materials[mat].link().unwrap_or("?").rsplit('\\').next().unwrap_or("?")));
                    }
                }
            }
            let mat = mat;
            let is_voted = is_voted;
            // Techno3 "_Ids" materials are the terrain id/mask pass (Land Base
            // carries a 4-vertex `Tech3 Block PyPxz_Ids` quad over its Land
            // quad): no look of their own, and as an item material they draw
            // flat grey and z-fight the real surface into stripes (2026-09-06).
            if self.materials.get(mat).and_then(|m| m.link()).map(|l| l.contains("_Ids")).unwrap_or(false) {
                self.notes.push(format!("id-pass visual {} dropped", self.materials[mat].link().unwrap_or("")));
                continue;
            }
            // A terrain WATER surface (`RedIsland\Media\Material\Water`: the water
            // quad of WaterHill / DecoLake prefabs) is not drawable as an item
            // material — the game's water is a render pass of the Water zone,
            // and as a static visual it came out as a black quad (Summer 02).
            // The regenerated full-size lake is at that very height, so dropping
            // the quad leaves the real water showing through. Stadium has no
            // water zone: its pools are the `WaterBase` blocks' own quads
            // (`Stadium\Media\Material\Water`), so there the quad is kept
            // (`keep_water`; Summer 15's pools drew as the bare cyan floor).
            if !self.keep_water && self.materials.get(mat).and_then(|m| m.link()).map(|l| l.ends_with("\\Material\\Water")).unwrap_or(false) {
                self.notes.push("water surface visual dropped (the zone water draws it)".to_string());
                continue;
            }
            // TINY_DROP_MATS=sub1,sub2: drop visuals whose material link contains
            // a substring (bisecting which material makes the game drop an item)
            if let Ok(drop) = std::env::var("TINY_DROP_MATS") {
                let link = self.materials.get(mat).and_then(|m| m.link()).unwrap_or("").to_string();
                if drop.split(',').any(|s| !s.is_empty() && link.contains(s)) {
                    self.notes.push(format!("visual {link} dropped (TINY_DROP_MATS)"));
                    continue;
                }
            }
            // (A gameplay gate's sign panels — the row along the arch on
            // `SpecialSign<Kind>` / the kind folder's `Sign`, the beam square on
            // `SpecialSignOff` / `SignOff` — are kept as geometry: their LED
            // shader shows a display the live gate feeds, so what they SHOW is
            // fixed at the material (signlogo.rs / `sign_logo_material`), not by
            // dropping one of them. Checked in the data first: the two panels
            // are ordinary geoms of the same Solid2 with identical records
            // (TINY_DUMP_DECLS on Special24m.Prefab.Gbx: u01 -1, same lod
            // mask, u02 0; no u10/u12/u19 tables, flags 0) — no on/off flag.)
            let mut v = vis.clone();
            // A visual read in the pack's inline-vertex form is written back
            // that way only while it keeps its frame table (the flag cloth with
            // its 86 tween frames); a frame-less copy takes the stream form
            // every other item visual has.
            if v.sub_visuals.is_empty() {
                v.inline_form = false;
            }
            // TINY_DUMP_DECLS=1: one note per visual with its vertex
            // declarations and the distinct values of every one-word element
            // (colour / int32 ids), for reading a shader's per-vertex inputs.
            if std::env::var_os("TINY_DUMP_DECLS").is_some() {
                if let Some(Node::VertexStream(s)) = v.main.as_ref().and_then(|m| m.vertex_streams.first()).and_then(|r| r.inline.as_deref()) {
                    let compress = s.compress_local3d.unwrap_or(false);
                    let mut parts = Vec::new();
                    for (d, e) in s.decls.iter().zip(s.elems.iter()) {
                        let mut p = format!("name{} type{} space{}", d.name(), d.stored_type(compress), d.space());
                        if d.name() == N_POSITION {
                            if let Elem::Float3(pts) = e {
                                // y histogram in 0.5 m bins (prefab space, before scaling)
                                let mut bins: std::collections::BTreeMap<i32, usize> = std::collections::BTreeMap::new();
                                for q in pts {
                                    *bins.entry((q[1] * 2.0).floor() as i32).or_default() += 1;
                                }
                                p.push_str(&format!(" y{{{}}}", bins.iter().map(|(b, n)| format!("{}:{}", *b as f32 / 2.0, n)).collect::<Vec<_>>().join(" ")));
                            }
                        }
                        if let Elem::Word(w) = e {
                            let mut u: Vec<u32> = w.clone();
                            u.sort_unstable();
                            u.dedup();
                            p.push_str(&format!(" values[{}]", u.iter().take(12).map(|x| format!("{x:08x}")).collect::<Vec<_>>().join(",")));
                            // per distinct word: the y histogram of the vertices carrying it
                            if let Some(Elem::Float3(pts)) = s.decls.iter().zip(s.elems.iter()).find(|(d, _)| d.name() == N_POSITION).map(|(_, e)| e) {
                                for val in u.iter().take(6) {
                                    let mut bins: std::collections::BTreeMap<i32, usize> = std::collections::BTreeMap::new();
                                    for (q, x) in pts.iter().zip(w.iter()) {
                                        if x == val {
                                            *bins.entry((q[1] * 2.0).floor() as i32).or_default() += 1;
                                        }
                                    }
                                    p.push_str(&format!(" {val:x}@y{{{}}}", bins.iter().map(|(b, n)| format!("{}:{}", *b as f32 / 2.0, n)).collect::<Vec<_>>().join(" ")));
                                }
                            }
                        }
                        if let Elem::Float4(f) = e {
                            p.push_str(&format!(" f4[{:?}..]", f.first()));
                        }
                        if let Elem::Float2(f) = e {
                            p.push_str(&format!(" f2[{:?}..]", f.first()));
                        }
                        parts.push(p);
                    }
                    self.notes.push(format!("visual {} mat#{} ({} verts) material {}: {} || chunks {:x?} uvg {} u02 {} u03 {} u04 {:x?} sflags {:x} cflags {:x} sver {} tcs {} subvis {} splits {} tangents {:?} idx {}", g.visual_index, g.material_index, s.elems.first().map(|e| e.len()).unwrap_or(0), self.materials[mat].link().unwrap_or("?"), parts.join(" | "), v.chunks, v.main.as_ref().map(|m| m.uv_groups.len()).unwrap_or(0), v.main.as_ref().map(|m| m.u02).unwrap_or(0), v.main.as_ref().map(|m| m.u03).unwrap_or(0), v.main.as_ref().map(|m| m.u04.chunks(4).map(|c| u32::from_le_bytes([c[0], c[1], c[2], c[3]])).collect::<Vec<u32>>()).unwrap_or_default(), s.flags, v.main.as_ref().map(|m| m.chunk_flags).unwrap_or(0), s.version, v.main.as_ref().map(|m| m.tex_coord_sets.len()).unwrap_or(0), v.sub_visuals.len(), v.splits.len(), v.tangents.as_ref().map(|(a, b)| (a.len(), b.len())), v.index_buffer.as_ref().map(|b| b.indices.len()).unwrap_or(0)));
                }
            }
            // A voted (shared Techno3 id) material has no look of its own: the
            // prefab paints its layers per VERTEX (Beach: id 0 = Land on the
            // plateau, 2 = Sand on the slope, 1 = SeaFloor under water; hills
            // and cliffs likewise switch to HillPxz/CliffPxz on the steep part)
            // and the collision surface is partitioned the same way. A single
            // linkable pak material is uniform, so one material per visual
            // painted a whole tile with one layer (the 2026-09-06 beaches: sand
            // and sea floor where the original shows grass). Split the visual
            // by the collision material under each triangle instead — one
            // sub-visual per coincident surface material, unmatched triangles
            // staying with the voted majority. `TINY_NO_SPLIT=1` disables.
            if is_voted && !smap.is_empty() && !self.no_split && std::env::var_os("TINY_NO_SPLIT").is_none() {
                let (pos, idx) = visual_triangles(&v);
                let ntri = idx.len() / 3;
                // Per-triangle collision material (None where no collision
                // triangle coincides).
                let surf_mat: Vec<Option<usize>> = idx
                    .chunks(3)
                    .take(ntri)
                    .map(|t| {
                        let mut k = [mm(&pos[t[0] as usize]), mm(&pos[t[1] as usize]), mm(&pos[t[2] as usize])];
                        k.sort();
                        smap.get(&k).map(|(link, phys)| self.link_slot(link, *phys, self.editors))
                    })
                    .collect();
                // The prefab's own painting: vertex element 4 (Int32) is the
                // layer id — one byte, or two bytes `hi:lo` on a blend visual
                // whose vertex colour's G byte picks lo (G < 128) or hi. The
                // id -> material table is voted from the collision material
                // under the single-id triangles, so the split lands where the
                // original changes layer (the blend's midpoint), not where the
                // physics zone happens to change.
                let eff_ids = effective_layer_ids(&v);
                let mut tri_mat: Vec<usize> = Vec::with_capacity(ntri);
                match eff_ids {
                    Some(eff) if !layer_table.is_empty() && std::env::var_os("TINY_SPLIT_SURFACE").is_none() => {
                        let table = &layer_table;
                        for (ti, t) in idx.chunks(3).take(ntri).enumerate() {
                            let (a, b, c) = (eff[t[0] as usize], eff[t[1] as usize], eff[t[2] as usize]);
                            let m = if a == b && b == c {
                                table.get(&a).copied().or(surf_mat[ti]).unwrap_or(mat)
                            } else {
                                // a triangle straddling two layers: the collision
                                // material under it, else the majority id
                                let maj = if a == b || a == c { a } else if b == c { b } else { a.max(b).max(c) };
                                surf_mat[ti].or_else(|| table.get(&maj).copied()).unwrap_or(mat)
                            };
                            tri_mat.push(m);
                        }
                    }
                    _ => {
                        for sm in &surf_mat {
                            tri_mat.push(sm.unwrap_or(mat));
                        }
                    }
                }
                let mut groups: Vec<usize> = tri_mat.clone();
                groups.sort_unstable();
                groups.dedup();
                if groups.len() > 1 {
                    for gm in groups {
                        let keep: Vec<bool> = tri_mat.iter().map(|m| *m == gm).collect();
                        let mut sv = sub_visual(&v, &keep)?;
                        transform_visual(&mut sv, iso, scale)?;
                        self.notes.push(format!("visual {} split: {} triangles -> {}", g.visual_index, keep.iter().filter(|k| **k).count(), self.materials[gm].link().unwrap_or("?")));
                        // TINY_SPLIT_KEEPMAT=1: split the geometry but keep the voted material (bisecting a crash)
                        let gm = if std::env::var_os("TINY_SPLIT_KEEPMAT").is_some() { mat } else { gm };
                        // TINY_SPLIT_MATS=a,b: only pieces whose new material link contains one of these take it (bisecting)
                        let gm = match std::env::var("TINY_SPLIT_MATS") {
                            Ok(list) => {
                                let link = self.materials[gm].link().unwrap_or("").to_string();
                                if list.split(',').any(|s| !s.is_empty() && link.ends_with(s)) { gm } else { mat }
                            }
                            Err(_) => gm,
                        };
                        // TINY_SPLIT_MINVERTS / TINY_SPLIT_MAXVERTS: pieces outside the range keep the voted material (bisecting)
                        let nv = sv.main.as_ref().map(|m| m.count).unwrap_or(0);
                        let lo: i32 = std::env::var("TINY_SPLIT_MINVERTS").ok().and_then(|s| s.parse().ok()).unwrap_or(0);
                        let hi: i32 = std::env::var("TINY_SPLIT_MAXVERTS").ok().and_then(|s| s.parse().ok()).unwrap_or(i32::MAX);
                        let gm = if nv < lo || nv > hi { mat } else { gm };
                        let skip: Vec<i32> = std::env::var("TINY_SPLIT_SKIPVERTS").ok().map(|s| s.split(',').filter_map(|x| x.parse().ok()).collect()).unwrap_or_default();
                        let gm = if skip.contains(&nv) { mat } else { gm };
                        // A piece without vertex colours takes the PLAIN variant of its layer
                        // (Land, not TransitionToLand): the Transition* materials are the
                        // `_Ids_Tex` shader whose decal is driven by the vertex colour, and a
                        // colourless visual under it paints the decal everywhere (grass
                        // plateaus came out sand, 2026-09-06). The prefab does the same: its
                        // colourless visuals use the plain `_Ids` shader.
                        let has_color = sv.stream().map(|s| s.decls.iter().any(|d| d.name() == N_COLOR0)).unwrap_or(false);
                        let gm = if !has_color { self.plain_variant_slot(gm) } else { gm };
                        visual_slots.push((self.visuals.len(), gm));
                        self.visuals.push(MergedVisual { visual: sv, material: gm, lod_mask: lod.0, lod_ladder: lod.1.clone() });
                    }
                    continue;
                }
                if let Some(&only) = tri_mat.first() {
                    if only != mat {
                        // every matched triangle disagrees with the majority
                        // vote (cannot happen — the vote is over these same
                        // triangles — but keep the per-triangle answer)
                        transform_visual(&mut v, iso, scale)?;
                        visual_slots.push((self.visuals.len(), only));
                        self.visuals.push(MergedVisual { visual: v, material: only, lod_mask: lod.0, lod_ladder: lod.1.clone() });
                        continue;
                    }
                }
            }
            transform_visual(&mut v, iso, scale)?;
            visual_slots.push((self.visuals.len(), mat));
            self.visuals.push(MergedVisual { visual: v, material: mat, lod_mask: lod.0, lod_ladder: lod.1.clone() });
        }
        if let Some(sf) = so.surface() {
            match &sf.surf {
                Surf::Mesh { vertices, triangles, .. } => {
                    // A triangle's u8 is its physics id (matches the u16 list
                    // it indexes on every Nadeo prefab measured).
                    match self.redress_collision(sf, triangles, resolve) {
                        Some(redressed) => self.add_surface_mesh(vertices, &redressed, iso, scale),
                        None => self.add_surface_mesh(vertices, triangles, iso, scale),
                    }
                }
                // A primitive collision (sphere / ellipsoid / axis box, or a
                // compound of them) is meshed: the item has ONE collision mesh,
                // and a skipped primitive was a drive-through prop (2026-09-07).
                // Its triangles take the shape's surface index as physics id,
                // resolved through the surface's material table like a mesh's.
                other => match other.triangulate() {
                    Some((verts, tris)) if !tris.is_empty() => {
                        let phys_of = |t: &Triangle| -> u8 { sf.material_ids.get(t.surface_index.max(0) as usize).map(|id| (id & 0xFF) as u8).unwrap_or(t.material_id) };
                        let tris: Vec<Triangle> = tris.iter().map(|t| Triangle { material_id: phys_of(t), ..*t }).collect();
                        self.notes.push(format!("collision surf type {} meshed: {} triangles", other.type_id(), tris.len()));
                        self.add_surface_mesh(&verts, &tris, iso, scale);
                    }
                    _ => self.notes.push(format!("collision surf type {} is not a mesh and could not be meshed; skipped", other.type_id())),
                },
            }
        } else if so.is_mesh_collidable {
            // Collide against the visuals themselves — the nearest level's
            // (a coarser level is the same surface, coarser).
            for (vi, mat) in visual_slots {
                if self.visuals[vi].lod_mask > 0 && self.visuals[vi].lod_mask & 1 == 0 {
                    continue;
                }
                let v = &self.visuals[vi].visual;
                let phys = self.materials[mat].physics();
                let (pos, idx) = visual_triangles(v);
                let tris: Vec<Triangle> = idx.chunks(3).filter(|c| c.len() == 3).map(|c| Triangle { indices: [c[0], c[1], c[2]], material_id: phys, u03: 0, surface_index: 0 }).collect();
                self.add_surface_mesh(&pos, &tris, &IDENTITY, 1.0);
            }
        }
        Ok(())
    }
}

/// The prefab's own terrain painting, per vertex: element 4 (Int32) is the
/// layer id — one byte, or two bytes `hi:lo` on a blend visual whose vertex
/// colour's G byte picks lo (G < 128) or hi (measured on Beach\Base1A: 0x200
/// vertices at the plateau edge read Land at the top ring and Sand below it,
/// 0x201 at the water line read SeaFloor below and Sand above). `None` when
/// the stream has no such element.
pub fn effective_layer_ids(v: &CPlugVisualIndexedTriangles) -> Option<Vec<u32>> {
    let s = v.stream()?;
    let compress = s.compress_local3d.unwrap_or(false);
    let ids = s.decls.iter().zip(s.elems.iter()).find(|(d, _)| d.name() == 4 && d.stored_type(compress) == super::vstream::T_INT32).and_then(|(_, e)| match e {
        Elem::Word(w) => Some(w),
        _ => None,
    })?;
    let colors = s.decls.iter().zip(s.elems.iter()).find(|(d, _)| d.name() == super::vstream::N_COLOR0).and_then(|(_, e)| match e {
        Elem::Word(w) => Some(w),
        _ => None,
    });
    Some(
        ids.iter()
            .enumerate()
            .map(|(i, id)| {
                if *id < 0x100 {
                    *id
                } else {
                    let g = colors.and_then(|c| c.get(i)).map(|c| (c >> 8) & 0xFF).unwrap_or(0);
                    if g < 128 { id & 0xFF } else { (id >> 8) & 0xFF }
                }
            })
            .collect(),
    )
}

/// Every visual of one material gets the same vertex declaration.
///
/// The client merges same-material visuals into one draw using the FIRST
/// visual's declaration and fetches each element from every other visual by
/// name; a visual missing an element the first one has hands it a null
/// pointer (crash at 0x140456c35, 2026-09-06: two TransitionToLand pieces,
/// 71 vertices with tangents + 29 without). Nadeo's prefabs never mix
/// layouts under one material; the per-layer split does. So per material,
/// take the union of the declarations and synthesise what a visual lacks:
/// colour = opaque white, a second texcoord set = copy of the first, layer
/// ids = 0, tangents = an orthonormal frame around the normal.
/// Declarations are rebuilt in ascending name order with cumulative offsets
/// (the order every pack visual uses).
pub fn harmonize_layouts(visuals: &mut [MergedVisual]) {
    use super::vstream::{Decl, T_FLOAT2};
    use std::collections::BTreeMap;
    // material -> union of (name -> donor decl, stored type)
    let mut unions: BTreeMap<usize, BTreeMap<u32, (Decl, u32)>> = BTreeMap::new();
    for mv in visuals.iter() {
        let Some(s) = mv.visual.stream() else { continue };
        let compress = s.compress_local3d.unwrap_or(false);
        let u = unions.entry(mv.material).or_default();
        for d in &s.decls {
            u.entry(d.name()).or_insert((d.clone(), d.stored_type(compress)));
        }
    }
    for mv in visuals.iter_mut() {
        let Some(union) = unions.get(&mv.material) else { continue };
        let Some(m) = mv.visual.main.as_mut() else { continue };
        let Some(Node::VertexStream(s)) = m.vertex_streams.first_mut().and_then(|r| r.inline.as_deref_mut()) else { continue };
        let compress = s.compress_local3d.unwrap_or(false);
        let have: Vec<u32> = s.decls.iter().map(|d| d.name()).collect();
        if union.keys().all(|n| have.contains(n)) {
            continue;
        }
        let n = s.count.max(0) as usize;
        let by_name: BTreeMap<u32, (Decl, Elem)> = s.decls.iter().zip(s.elems.iter()).map(|(d, e)| (d.name(), (d.clone(), e.clone()))).collect();
        let uv0 = by_name.get(&N_TEXCOORD0).map(|(_, e)| e.clone());
        let normals: Option<Vec<[f32; 3]>> = by_name.get(&N_NORMAL).map(|(_, e)| match e {
            Elem::Word(w) => w.iter().map(|x| dec3n_unpack(*x)).collect(),
            Elem::Float3(p) => p.clone(),
            _ => vec![[0.0, 1.0, 0.0]; n],
        });
        let mut decls: Vec<Decl> = Vec::new();
        let mut elems: Vec<Elem> = Vec::new();
        let mut offset = 0u32;
        // stride first (bits 20..27 of every decl)
        let stride: u32 = union.values().map(|(_, st)| super::vstream::type_size(*st).unwrap_or(4) as u32).sum();
        for (name, (donor, stored)) in union {
            let elem = match by_name.get(name) {
                Some((_, e)) => e.clone(),
                None => match (*name, *stored) {
                    // the value the prefab gives its uniform Transition* visuals: decal off
                    (N_COLOR0, _) => Elem::Word(vec![0xFFFF_00FF; n]),
                    (11, T_FLOAT2) => match &uv0 {
                        Some(Elem::Float2(v)) => Elem::Float2(v.clone()),
                        _ => Elem::Float2(vec![[0.0, 0.0]; n]),
                    },
                    (N_TANGENT_U | N_TANGENT_V, T_DEC3N) => {
                        let frame = |nrm: [f32; 3]| -> ([f32; 3], [f32; 3]) {
                            let up = if nrm[1].abs() < 0.9 { [0.0, 1.0, 0.0] } else { [1.0, 0.0, 0.0] };
                            let mut u = [nrm[1] * up[2] - nrm[2] * up[1], nrm[2] * up[0] - nrm[0] * up[2], nrm[0] * up[1] - nrm[1] * up[0]];
                            let l = (u[0] * u[0] + u[1] * u[1] + u[2] * u[2]).sqrt().max(1e-6);
                            u = [u[0] / l, u[1] / l, u[2] / l];
                            let v = [nrm[1] * u[2] - nrm[2] * u[1], nrm[2] * u[0] - nrm[0] * u[2], nrm[0] * u[1] - nrm[1] * u[0]];
                            (u, v)
                        };
                        let def = vec![[0.0, 1.0, 0.0]; n];
                        let nrms = normals.as_ref().unwrap_or(&def);
                        Elem::Word(nrms.iter().map(|nr| { let (u, v) = frame(*nr); dec3n_pack(if *name == N_TANGENT_U { u } else { v }) }).collect())
                    }
                    (_, T_FLOAT2) => Elem::Float2(vec![[0.0, 0.0]; n]),
                    (_, T_FLOAT3) => Elem::Float3(vec![[0.0, 0.0, 0.0]; n]),
                    (_, st) if super::vstream::type_size(st) == Some(4) => Elem::Word(vec![0; n]),
                    (_, st) => Elem::Raw { size: super::vstream::type_size(st).unwrap_or(4), bytes: vec![0; n * super::vstream::type_size(st).unwrap_or(4)] },
                },
            };
            let size = super::vstream::type_size(*stored).unwrap_or(4) as u32;
            decls.push(Decl::with_stride(donor.name(), donor.ty(), donor.space(), offset, stride / 4));
            offset += size;
            elems.push(elem);
        }
        let _ = compress;
        s.decls = decls;
        s.elems = elems;
    }
}

/// The visual restricted to the triangles flagged in `keep` (one flag per
/// triangle): the index buffer is filtered, the vertices it no longer uses are
/// dropped from every stream element and both tangent arrays, and the counts
/// follow. Vertex order is preserved. The bounding box is left to
/// `transform_visual`, which recomputes it.
pub fn sub_visual(v: &CPlugVisualIndexedTriangles, keep: &[bool]) -> R<CPlugVisualIndexedTriangles> {
    let mut out = v.clone();
    let idx = v.index_buffer.as_ref().map(|b| b.indices.clone()).unwrap_or_default();
    let nverts = v.main.as_ref().map(|m| m.count.max(0) as usize).unwrap_or(0);
    let mut used = vec![false; nverts];
    let mut new_idx: Vec<u32> = Vec::new();
    for (ti, t) in idx.chunks(3).enumerate() {
        if t.len() == 3 && keep.get(ti).copied().unwrap_or(false) {
            for &i in t {
                *used.get_mut(i as usize).ok_or("sub_visual: index past the vertex count")? = true;
            }
            new_idx.extend_from_slice(t);
        }
    }
    let mut remap = vec![u32::MAX; nverts];
    let mut n = 0u32;
    for (i, u) in used.iter().enumerate() {
        if *u {
            remap[i] = n;
            n += 1;
        }
    }
    for i in new_idx.iter_mut() {
        *i = remap[*i as usize];
    }
    let m = out.main.as_mut().ok_or("sub_visual: visual without chunk 0x0900600F")?;
    let per = (((!(m.flags() >> 17)) & 8) | 4) as usize;
    m.count = n as i32;
    let stream = match m.vertex_streams.first_mut().and_then(|r| r.inline.as_deref_mut()) {
        Some(Node::VertexStream(s)) => s,
        _ => return Err("sub_visual: visual without an inline vertex stream".into()),
    };
    stream.count = n as i32;
    for e in stream.elems.iter_mut() {
        *e = match e {
            Elem::Float2(a) => Elem::Float2(a.iter().zip(&used).filter(|(_, u)| **u).map(|(x, _)| *x).collect()),
            Elem::Float3(a) => Elem::Float3(a.iter().zip(&used).filter(|(_, u)| **u).map(|(x, _)| *x).collect()),
            Elem::Float4(a) => Elem::Float4(a.iter().zip(&used).filter(|(_, u)| **u).map(|(x, _)| *x).collect()),
            Elem::Word(a) => Elem::Word(a.iter().zip(&used).filter(|(_, u)| **u).map(|(x, _)| *x).collect()),
            Elem::Raw { size, bytes } => {
                let size = *size;
                Elem::Raw { size, bytes: bytes.chunks(size.max(1)).zip(&used).filter(|(_, u)| **u).flat_map(|(c, _)| c.iter().copied()).collect() }
            }
        };
    }
    if let Some((a, b)) = out.tangents.as_mut() {
        for t in [a, b] {
            if !t.is_empty() {
                *t = t.chunks(per).zip(&used).filter(|(_, u)| **u).flat_map(|(c, _)| c.iter().copied()).collect();
            }
        }
    }
    if let Some(ib) = out.index_buffer.as_mut() {
        ib.indices = new_idx;
    }
    Ok(out)
}

/// A visual's (already transformed) positions and triangle indices.
pub fn visual_triangles(v: &CPlugVisualIndexedTriangles) -> (Vec<[f32; 3]>, Vec<u32>) {
    let pos = match v.stream().and_then(|s| s.elems.first()) {
        Some(Elem::Float3(p)) => p.clone(),
        _ => Vec::new(),
    };
    let idx = v.index_buffer.as_ref().map(|b| b.indices.clone()).unwrap_or_default();
    (pos, idx)
}

fn mm(p: &[f32; 3]) -> (i32, i32, i32) {
    ((p[0] * 1000.0).round() as i32, (p[1] * 1000.0).round() as i32, (p[2] * 1000.0).round() as i32)
}

/// Per source-material-index vote: visuals whose link came out shared-id
/// (Techno3) get the surface material most of their triangles coincide
/// with. Triangle lookup is by sorted mm vertex keys in prefab space.
/// Collision triangle (sorted mm vertex keys) -> (material link, physics).
pub type SurfMap = std::collections::HashMap<[(i32, i32, i32); 3], (String, u8)>;

fn surface_votes(
    so: &super::item::CPlugStaticObjectModel,
    s2: &super::solid2::CPlugSolid2Model,
    resolve: &mut MaterialResolver,
) -> (Vec<Option<(String, u8)>>, SurfMap) {
    use std::collections::BTreeMap;
    let sf = match so.surface() {
        Some(sf) => sf,
        None => return (Vec::new(), SurfMap::new()),
    };
    let (verts, tris) = match &sf.surf {
        super::surface::Surf::Mesh { vertices, triangles, .. } => (vertices, triangles),
        _ => return (Vec::new(), SurfMap::new()),
    };
    // surface tri -> (material path, physics)
    let mut smap: SurfMap = SurfMap::new();
    for t in tris {
        let mut k = [mm(&verts[t.indices[0] as usize]), mm(&verts[t.indices[1] as usize]), mm(&verts[t.indices[2] as usize])];
        k.sort();
        let si = t.surface_index.max(0) as usize;
        let (link, phys) = match sf.materials.get(si) {
            Some(super::surface::SurfMaterial::Node(r)) => match resolve(r.index) {
                Some((path, _, _)) => {
                    let phys = sf.material_ids.get(si).map(|id| (id & 0xFF) as u8).unwrap_or(t.material_id);
                    (material_link(&path), phys)
                }
                None => continue,
            },
            _ => continue,
        };
        smap.insert(k, (link, phys));
    }
    // votes per source material index
    let mut votes: Vec<BTreeMap<(String, u8), usize>> = vec![BTreeMap::new(); s2.shaded_geoms.iter().map(|g| g.material_index.max(0) as usize + 1).max().unwrap_or(0)];
    for g in &s2.shaded_geoms {
        let mi = g.material_index.max(0) as usize;
        let vis = match s2.visuals.get(g.visual_index as usize).and_then(|r| r.inline.as_deref()) {
            Some(super::Node::Visual(v)) => v,
            _ => continue,
        };
        let (pos, idx) = visual_triangles(vis);
        for t in idx.chunks(3) {
            if t.len() < 3 {
                continue;
            }
            let mut k = [mm(&pos[t[0] as usize]), mm(&pos[t[1] as usize]), mm(&pos[t[2] as usize])];
            k.sort();
            if let Some(vote) = smap.get(&k) {
                if mi < votes.len() {
                    *votes[mi].entry(vote.clone()).or_default() += 1;
                }
            }
        }
    }
    (votes.into_iter().map(|v| v.into_iter().max_by_key(|(_, n)| *n).map(|(lp, _)| lp)).collect(), smap)
}

fn inline(index: i32, node: Node) -> Ref {
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

/// The merged visuals + materials as one `CPlugSolid2Model`, node indices
/// taken from `next` (visual, its stream, then the materials).
/// External files the solid names (node index, pack path) — the item's reference
/// table (`TINY_LIGHT_FORM=extern|socket-tex`, a probe of whether an embedded
/// item can reach the packs by path). Empty in the production forms.
thread_local! {
    pub static EXTERNALS: std::cell::RefCell<Vec<(u32, String)>> = const { std::cell::RefCell::new(Vec::new()) };
    /// The `.Light.Gbx` files the `file` light form writes next to the item:
    /// (file name, bytes), drained by the caller into the library archive.
    pub static LIGHT_FILES: std::cell::RefCell<Vec<(String, Vec<u8>)>> = const { std::cell::RefCell::new(Vec::new()) };
}

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
    // (`remap_lod_mask`). With `TINY_LOD0_ONLY` nothing but level 0 was
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
    if std::env::var_os("TINY_NO_HARMONIZE").is_none() {
        harmonize_layouts(&mut pre);
    }
    let visuals = if std::env::var_os("TINY_NO_COALESCE").is_some() { pre } else { coalesce(&pre) };
    // TINY_LOD_TEST=vanish: the ladder is written but only the nearest level's
    // geoms are kept, masked to level 0 alone — an item that must DISAPPEAR
    // past its first switch distance if the game honours an embedded item's
    // detail ladder (the 2026-09-07 probe: LOD1..3 of a gate look like LOD0
    // at 100 m, as they should, so a switch cannot be seen on the real geometry).
    let visuals: Vec<MergedVisual> = match std::env::var("TINY_LOD_TEST").as_deref() {
        Ok("vanish") => visuals.into_iter().filter(|mv| mv.lod_mask & 1 != 0).map(|mut mv| { mv.lod_mask = 1; mv }).collect(),
        // TINY_LOD_TEST=far: the coarser levels only (no level-0 geom): invisible
        // near if the game reads the masks, visible everywhere if it draws
        // every geom regardless
        Ok("far") => visuals.into_iter().filter(|mv| mv.lod_mask & 1 == 0).collect(),
        _ => visuals,
    };
    // TINY_ONLY_MATS=a,b: keep only the visuals whose material link ends with one of these (minimising a crasher)
    let visuals: Vec<MergedVisual> = match std::env::var("TINY_ONLY_MATS") {
        Ok(list) => visuals.into_iter().filter(|mv| { let l = m.materials[mv.material].link().unwrap_or(""); list.split(',').any(|s| !s.is_empty() && l.ends_with(s)) }).collect(),
        Err(_) => visuals,
    };
    if visuals.is_empty() {
        return Err("TINY_ONLY_MATS left no visual".into());
    }
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
    // exactly that order. `TINY_LOD_ORDER=material` sorts material-major
    // instead (level minor). Stable, so same-material same-level visuals
    // keep their relative order.
    let mut visuals = visuals;
    let level_of = |mask: u32| -> u32 { if mask == 0 { 0 } else { mask.trailing_zeros() } };
    let material_major = std::env::var("TINY_LOD_ORDER").map(|v| v == "material").unwrap_or(false);
    if material_major {
        visuals.sort_by_key(|mv| (used.iter().position(|u| *u == mv.material).unwrap_or(usize::MAX), level_of(mv.lod_mask), mv.lod_mask));
    } else {
        visuals.sort_by_key(|mv| (level_of(mv.lod_mask), used.iter().position(|u| *u == mv.material).unwrap_or(usize::MAX), mv.lod_mask));
    }
    let per_visual = std::env::var_os("TINY_MAT_PER_VISUAL").is_some();
    for mv in &visuals {
        let mut v = mv.visual.clone();
        let main = v.main.as_mut().unwrap();
        // TINY_STRIP_U04=1: drop the version-6 trailing blob (u02/u03/u04) the pack visuals carry;
        // the reference items have none. TINY_SFLAGS=N: force the vertex stream flags word.
        if std::env::var_os("TINY_STRIP_U04").is_some() {
            main.u02 = 0;
            main.u03 = 0;
            main.u04.clear();
        }
        // TINY_STRIP_TANGENT_CHUNK=1: drop the empty CPlugVisual3D tangent-array chunk 0x0902C004
        if std::env::var_os("TINY_STRIP_TANGENT_CHUNK").is_some() {
            v.tangents = None;
            v.chunks.retain(|c| *c != 0x0902C004);
        }
        if let Ok(f) = std::env::var("TINY_SFLAGS") {
            if let Some(Node::VertexStream(s)) = main.vertex_streams.first_mut().and_then(|r| r.inline.as_deref_mut()) {
                s.flags = f.parse().unwrap_or(s.flags);
            }
        }
        // the stream sits right after its visual (an inline-form visual has
        // none and takes one index)
        let mut has_stream = false;
        for r in main.vertex_streams.iter_mut() {
            if r.inline.is_some() && !v.inline_form {
                r.index = *next + 1;
                has_stream = true;
            }
        }
        // TINY_MAT_PER_VISUAL=1: one custom material entry per visual (duplicating the
        // inst), the way the reference items are built.
        let material_index = if per_visual { s2.visuals.len() as i32 } else { used.iter().position(|u| *u == mv.material).unwrap() as i32 };
        s2.shaded_geoms.push(ShadedGeom { visual_index: s2.visuals.len() as i32, material_index, u01: -1, lod_mask: mv.lod_mask as i32, u02: 0 });
        s2.visuals.push(inline(*next, Node::Visual(v)));
        *next += if has_stream { 2 } else { 1 };
    }
    s2.lod_max_dist = ladder;
    let mat_list: Vec<usize> = if per_visual { visuals.iter().map(|mv| mv.material).collect() } else { used.clone() };
    for inst in mat_list.iter().map(|u| &m.materials[*u]) {
        let inst = skinned_material(inst, opts.collection);
        let inst = custom_texture_material(&inst, &opts.ident);
        let inst = sign_logo_material(&inst, m);
        let inst = light_skin_material(&inst, m);
        s2.custom_materials.push(Material { name: String::new(), node: Some(inline(*next, Node::Material(inst))) });
        *next += 1;
    }
    // The source model's lights. Two forms (TINY_LIGHT_FORM):
    //  * `socket`: each socket points at an INLINE CPlugLight whose GxLight
    //    rides inline in turn (two node indices) — the pack's own form; the
    //    EDITOR renders these, PLAY mode did not (Summer 09's start deck dark,
    //    2026-09-07);
    //  * `user`: the item editor's form — a CPlugLightUserModel per light in
    //    `light_user_models`, the socket carrying no node (u02 false, name
    //    string), `light_insts` tying model k to socket k;
    //  * `both`: the socket carries the CPlugLight AND a user model instance
    //    points at it.
    // ⚠ `user` and `both` CRASHED the client at map load (Trackmania.exe+0x4c9062:
    // the light-inst loop reads [r14+0x78] = NULL; 2026-09-07, two dumps) — the
    // CPlugLightUserModel layout or the inst semantics are still guesses (no
    // reference item with editor lights was found: none of 95 TME items or the
    // ItemExchange "light" items carries one). They stay as experiment knobs.
    // The default `socket` form renders in the editor AND in play once the
    // source map's stale lightmap is stripped (`tmmaps tiny` does since 1b7adc5;
    // with it kept every converted-block item was BLACK in play, lights or not).
    // TINY_LIGHTS=drop leaves them out (the unlit bake of before 2026-09-07).
    if std::env::var("TINY_LIGHTS").map(|v| v != "drop").unwrap_or(true) {
        let form = std::env::var("TINY_LIGHT_FORM").unwrap_or_else(|_| "socket".into());
        for (k, ml) in m.lights_out.iter().enumerate() {
            let mut socket = ml.socket.clone();
            // ⚠ `extern`, `file` and `socket-tex` are PROBES THAT FAILED (2026-09-07):
            // an embedded item's reference table resolves NOTHING — neither a
            // pack path (`Stadium\Media\Light\ItemLampSpot.Light.Gbx`, ancestor
            // levels 0-3) nor a file placed next to the item in the archive
            // (use-file 0/1, with or without `Items\`); every such light was
            // dark on Summer 09's grass 60 m from any stock lamp. (Two earlier
            // "successes" were the stock Lamp's 50 m pool spilling onto a probe
            // placed 22 m from it.) So the projector cookie and the light sprite
            // (both texture fids) cannot be had; the inline `socket` form is what
            // works, and the material system's by-name texture lookup is the only
            // file an embedded item reaches.
            if form == "extern" {
                // probe: the socket names the PACK's `.Light.Gbx` as an EXTERNAL
                // node (reference table) — unscaled, untinted
                socket.u02 = true;
                socket.u04.clear();
                socket.node = super::NodeRef { index: *next, inline: None };
                EXTERNALS.with(|e| e.borrow_mut().push((*next as u32, ml.source.clone())));
                *next += 1;
            } else if form == "file" {
                // PRODUCTION: our scaled, tinted copy of the light as its own
                // `.Light.Gbx` next to the item, its projector/flare textures
                // referenced in the packs; the socket names that file
                let stem = opts.ident.trim_end_matches(".Item.Gbx");
                let file = format!("{stem}_L{k}.Light.Gbx");
                // TINY_LIGHT_EXT_PREFIX=Items\: how the socket spells the file's folder (probe)
                let spelled = format!("{}{file}", std::env::var("TINY_LIGHT_EXT_PREFIX").unwrap_or_default());
                let mut light = ml.light.clone();
                let mut ext: Vec<(u32, String)> = Vec::new();
                for (path, slot) in ml.bitmaps.iter().cloned() {
                    let idx = 2 + ext.len() as u32;
                    light.set_bitmap(slot, super::NodeRef { index: idx as i32, inline: None });
                    ext.push((idx, path));
                }
                LIGHT_FILES.with(|f| f.borrow_mut().push((file.clone(), super::light::light_file(&light, &ext))));
                socket.u02 = true;
                socket.u04.clear();
                socket.node = super::NodeRef { index: *next, inline: None };
                EXTERNALS.with(|e| e.borrow_mut().push((*next as u32, spelled)));
                *next += 1;
            } else if form == "socket" || form == "both" || form == "socket-tex" {
                let mut light = ml.light.clone();
                match light.gx_mut() {
                    Some(gx) if gx.inline.is_some() => gx.index = *next + 1,
                    Some(gx) => *gx = super::null_ref(),
                    None => {}
                }
                socket.u02 = true;
                socket.u04.clear();
                let mut used = 2;
                if form == "socket-tex" {
                    // the projector / flare bitmaps as EXTERNAL nodes again
                    for (path, slot) in ml.bitmaps.iter().cloned() {
                        let idx = *next + used;
                        light.set_bitmap(slot, super::NodeRef { index: idx, inline: None });
                        EXTERNALS.with(|e| e.borrow_mut().push((idx as u32, path)));
                        used += 1;
                    }
                }
                socket.node = inline(*next, Node::Light(light));
                *next += used;
            } else {
                socket.u02 = false;
                socket.u04 = format!("Light{k}");
                socket.node = super::null_ref();
            }
            if form == "user" || form == "both" {
                if let Some(g) = ml.light.gx_light() {
                    let um = super::light::CPlugLightUserModel::from_gx(g);
                    s2.light_user_models.push(inline(*next, Node::LightUserModel(um)));
                    *next += 1;
                    s2.light_insts.push((s2.light_user_models.len() as i32 - 1, s2.lights.len() as i32));
                }
            }
            s2.lights.push(socket);
        }
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
        // draws them. TINY_EMPTY_SURFACE=empty keeps the empty mesh; `tri`
        // (default) gives it one 1 mm triangle 4 m under the item's origin —
        // a shape the editor accepts and nothing can hit.
        let form = std::env::var("TINY_EMPTY_SURFACE").unwrap_or_else(|_| "tri".into());
        if form == "tri" {
            let v = vec![[0.0, -4.0, 0.0], [0.001, -4.0, 0.0], [0.0, -4.0, 0.001]];
            // byte and table both NotCollidable (28): the one place the two
            // disagreed in a whole library (surfhist, 2026-09-07)
            let t = vec![super::surface::Triangle { indices: [0, 1, 2], material_id: 28, u03: 0, surface_index: 0 }];
            return CPlugSurface::mesh(v, t, vec![28], [0.0, 0.0, 1.0]);
        }
    }
    CPlugSurface::mesh(m.surf_vertices.clone(), m.surf_triangles.clone(), m.surf_ids.clone(), [0.0, 0.0, 1.0])
}

/// `TINY_STATIC_FORM=prefab`: a static item laid out like the pack's own
/// items — `CGameItemModel -> CPlugPrefab { CPlugStaticObjectModel }` — instead
/// of the item editor's `CGameCommonItemEntityModel` wrapper (the 2026-09-07
/// probe of which form the game reads a detail ladder from; no waypoint
/// trigger in this form yet).
fn static_form_prefab() -> bool {
    std::env::var("TINY_STATIC_FORM").map(|v| v == "prefab").unwrap_or(false)
}

/// Build the whole item tree from the merged geometry.
pub fn assemble(m: &Merged, opts: &BuildOpts) -> R<super::StaticItemFile> {
    use super::item::*;
    use super::Id;
    if m.visuals.is_empty() && m.dyna.is_empty() {
        return Err("no visuals: nothing to build".into());
    }
    // node 1 = the entity model; a static item fixes 2 (static object) and 3
    // (its solid) like the reference items, a moving item hands indices out
    // in write order from 2
    let prefab_form = !m.dyna.is_empty() || static_form_prefab() || m.special.is_some() || !m.fx.is_empty();
    let mut next = if !prefab_form { 4i32 } else { 2i32 };
    let no_wp = std::env::var_os("TINY_NO_WAYPOINT").is_some();
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
    let trigger = match m.trigger.as_ref().filter(|_| !no_wp) {
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
            let mesh_index = next_index(&mut next);
            let s2 = build_solid2(&part.mesh, opts, &mut next).map_err(|e| format!("{}: {e}", part.path))?;
            let mut model = part.model.clone();
            model.mesh = inline(mesh_index, Node::Solid2(s2));
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
        ItemChunk::Waypoint { version: 12, waypoint_type: if no_wp { 3 } else { m.waypoint_type.unwrap_or(3) }, disable_lightmap: false, u_node: Some(super::null_ref()), u_byte: Some(0), u_ints: Some((-1, -1)) },
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
        // the reference table: the pack files the `extern` light probes name,
        // relative to the item's own folder after TINY_LIGHT_EXT_UP steps up
        // (default 0: the paths as the packs spell them)
        ref_table: {
            let ext = EXTERNALS.with(|e| std::mem::take(&mut *e.borrow_mut()));
            let up: u32 = std::env::var("TINY_LIGHT_EXT_UP").ok().and_then(|v| v.parse().ok()).unwrap_or(0);
            super::file::ref_table(up, &ext)
        },
        item: CGameItemModel { chunks },
    })
}

fn next_index(next: &mut i32) -> i32 {
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
        w.i32(8);
        w.i16(1);
        w.string("New Item");
        w.u8(3);
    }
    let mut out = vec![HeaderChunk { id: 0x2E001003, heavy: false, payload: d }];
    // TINY_ITEM_SKIN=0 leaves the declaration out (the A/B of 2026-09-07)
    if let Some(skin) = opts.skin.as_ref().filter(|_| std::env::var("TINY_ITEM_SKIN").as_deref() != Ok("0")) {
        out.push(HeaderChunk { id: tmmaps::header::GAME_SKIN_CHUNK, heavy: false, payload: skin.clone() });
    }
    out.extend([
        HeaderChunk { id: 0x2E001006, heavy: false, payload: vec![0; 8] },
        HeaderChunk { id: 0x2E002000, heavy: false, payload: 1u32.to_le_bytes().to_vec() },
        HeaderChunk { id: 0x2E002001, heavy: false, payload: vec![0; 4] },
    ]);
    out
}

/// Physics id of an external `.Material.Gbx` (its `CPlugMaterial` surface
/// id), through the store; `None` when it cannot be read.
pub fn material_physics(store: &mut crate::store::DataStore, path: &str) -> Option<u8> {
    let m = store.load_model(path).ok()?;
    let g = m.graph().ok()?;
    match g.root.as_ref()? {
        crate::node::Node::Material(_, phys) => Some(*phys),
        _ => None,
    }
}

/// A `.Material.Gbx`'s (physics, gameplay) surface ids, read off the chunks
/// that carry them: `0x09079017` = { version 1, [physics u8, gameplay u8,
/// u8, flags u8], f32, u32, string } (`Modifier\Boost\Collision`: 00 12 00
/// 80 = Concrete, ReactorBoost_Oriented; `RoadTech`: 10 00 0f 80 = Asphalt,
/// none) — and, for the older files that lack it, `0x0907900E` = { physics
/// u16, u16 } (TechnicsTrims: Metal). The bodies are short and chunk-framed
/// without sizes for these chunks, so the ids are located by their chunk
/// header rather than by a full walk.
pub fn material_surface_ids(store: &mut crate::store::DataStore, path: &str) -> Option<(u8, u8)> {
    let model = store.load_model(path).ok()?;
    let b = &model.body;
    let find = |pat: &[u8]| b.windows(pat.len()).position(|w| w == pat);
    if let Some(i) = find(&[0x17, 0x90, 0x07, 0x09, 0x01, 0x00, 0x00, 0x00]) {
        if let Some(w) = b.get(i + 8..i + 12) {
            return Some((w[0], w[1]));
        }
    }
    if let Some(i) = find(&[0x0e, 0x90, 0x07, 0x09]) {
        if let Some(w) = b.get(i + 4..i + 8) {
            return Some((w[0], 0));
        }
    }
    None
}

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
                tris.push(super::surface::Triangle { indices: [f[0] as u32 + base, f[1] as u32 + base, f[2] as u32 + base], material_id: *phys, u03: *gp, surface_index: 0 });
            }
        }
        if !tris.is_empty() {
            return Some((verts, tris, ids, sf.main_dir.unwrap_or([0.0, 0.0, 1.0])));
        }
    }
    None
}

/// The special gate's effect: the item's modifier folder names a
/// `Collision` material (`Stadium\Media\Modifier\Boost\Collision`), whose
/// surface ids are what the trigger slab carries under that dress. `None`
/// when the modifier has no such file (Turbo: the prefab's own dress) or
/// there is no modifier.
pub fn special_collision_ids(store: &mut crate::store::DataStore, m: &Merged) -> Option<(String, (u8, u8))> {
    let want = format!("collision{}", m.modifier_suffix.to_ascii_lowercase());
    let link = m.modifier.iter().find(|l| l.rsplit('\\').next().map(|s| s.to_ascii_lowercase()) == Some(want.clone()))?.clone();
    let ids = material_surface_ids(store, &format!("{link}.Material.Gbx"))?;
    Some((link, ids))
}

/// `Stadium\Media\Material\RoadTech.Material.Gbx` -> `Stadium\Media\Material\RoadTech`.
pub fn material_link(path: &str) -> String {
    let lower = path.to_ascii_lowercase();
    match lower.rfind(".material.gbx") {
        Some(i) => path[..i].to_string(),
        None => path.to_string(),
    }
}

/// Two material instances draw the same: every field of the main chunk but
/// the author-side `material_name` (`TM_Argentina_CustomPlastic43` vs
/// `…43S1` — the same plastic, the same colour), and the tiling chunk, agree.
/// The dedup key of [`Merged::material_inst_slot`]; `item-check` refuses two
/// slots that are the same by this measure.
pub fn same_look(a: &CPlugMaterialUserInst, b: &CPlugMaterialUserInst) -> bool {
    let strip = |m: &CPlugMaterialUserInst| {
        m.main.clone().map(|mut main| {
            main.material_name = crate::crystal_model::Id::Null;
            main
        })
    };
    strip(a) == strip(b) && a.tiling == b.tiling
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
    let tris: Vec<super::surface::Triangle> = triangles.iter().map(|t| super::surface::Triangle { indices: t.indices, material_id: ids.0, u03: ids.1, surface_index: 0 }).collect();
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
    let dyna_static = std::env::var("TINY_DYNA").map(|v| v == "static").unwrap_or(false);
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
                                        super::surface::Surf::Mesh { triangles, .. } => triangles.first().map(|t| (t.material_id, t.u03)).unwrap_or((0, 0)),
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
                // collision. `TINY_DYNA=static` bakes every one at rest.
                Some(p) if p.to_ascii_lowercase().ends_with(".dynaobject.gbx") => {
                    let bound = constraints.iter().find(|(target, _, _)| *target == i as i32).cloned();
                    match bound {
                        Some((_, cpath, cparams)) if !dyna_static => {
                            if let Err(err) = add_dyna_part(store, &p, &iso, scale, m, &cpath, cparams, e) {
                                m.notes.push(format!("{path} entity {i}: moving part {p} failed ({err}); baked at rest"));
                                if let Err(e2) = add_dyna_object_file(store, &p, &iso, scale, m) {
                                    m.notes.push(format!("{path} entity {i}: external {p} failed: {e2}"));
                                }
                            }
                        }
                        // a self-animating mesh (the flag cloth's vertex tween):
                        // a dyna entity of its own, no constraint
                        None if !dyna_static && tween_parts_enabled() && dyna_has_tween_material(store, &p) => {
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
                // an effect system (the Show items' smoke / sparks): parsed
                // with its particle models, inlined by `assemble` as an
                // entity of the prefab form. TINY_FX=drop leaves them out.
                Some(p) if p.to_ascii_lowercase().ends_with(".fxsys.gbx") => {
                    if std::env::var("TINY_FX").map(|v| v == "drop").unwrap_or(false) {
                        m.notes.push(format!("{path} entity {i}: external {p} dropped (TINY_FX=drop)"));
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
    if std::env::var("TINY_FX_TEXTURE").map(|v| v == "archive").unwrap_or(false) {
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

fn place_particle_node(node: &mut super::particle::ParticleNode, externals: &[(u32, String)], textures: &[(String, super::particle::ParticleNode, String, Vec<u8>)], next: &mut i32) {
    let texture_mode = std::env::var("TINY_FX_TEXTURE").unwrap_or_else(|_| "extern".into());
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
                    (Some(p), "archive") => match textures.iter().find(|(tp, _, _, _)| *tp == p) {
                        // the `.Texture.gbx` inline; its image named by its bare file
                        // name (folder 0 = the item's own folder in the archive)
                        Some((_, bitmap, name, _)) => {
                            let i = next_index(next);
                            let mut b = bitmap.clone();
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
/// overrides expression K (1..12) of every emitter — K=3 is the candidate
/// ScaleExpr; `TINY_FX_SCALE_EXPR=K` writes the item scale into expression K.
fn fx_entities(m: &Merged, scale: f32, next: &mut i32) -> Vec<super::prefab::Entity> {
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
            // the particle models: one inline copy per emitter that names it
            // the first time, a back reference afterwards
            let mut placed: Vec<(u32, i32)> = Vec::new();
            for e in fx.root.emitters_mut() {
                if e.model.index < 0 || e.model.inline.is_some() {
                    continue;
                }
                let src = e.model.index as u32;
                if let Some((_, i)) = placed.iter().find(|(k, _)| *k == src) {
                    e.model = super::NodeRef { index: *i, inline: None };
                    continue;
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
                if let Some(k) = scale_expr {
                    let v = format!("{scale}");
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
    m.editors = std::env::var_os("TINY_EDITORS").is_some();
    add_prefab(store, prefab, &IDENTITY, scale, &mut m, 0)?;
    let opts = BuildOpts { ident: ident.to_string(), author: author.to_string(), scale, collection, editors: m.editors, skin: m.skin.clone() };
    let f = assemble(&m, &opts)?;
    m.pictures.extend(LIGHT_FILES.with(|l| std::mem::take(&mut *l.borrow_mut())));
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
    m.editors = std::env::var_os("TINY_EDITORS").is_some();
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
            m2.editors = std::env::var_os("TINY_EDITORS").is_some();
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
    let opts = BuildOpts { ident: ident.to_string(), author: author.to_string(), scale, collection, editors: m.editors, skin: m.skin.clone() };
    let f = assemble(&m, &opts)?;
    m.pictures.extend(LIGHT_FILES.with(|l| std::mem::take(&mut *l.borrow_mut())));
    Ok((super::write_file(&f), m))
}

/// A gameplay gate's LED sign panel as a plain picture of the kind's logo on
/// black (signlogo.rs: the live gate feeds the panels' `_DispIn` shader a
/// display the static item cannot). `TINY_SIGN_LOGO=off` keeps the game
/// material (dark row, ⊗ on the beam).
pub fn sign_logo_material(inst: &CPlugMaterialUserInst, m: &Merged) -> CPlugMaterialUserInst {
    let Some(link) = inst.link().map(|s| s.to_string()) else { return inst.clone() };
    let Some(kind) = super::signlogo::kind_of_pseudo(&link).map(|s| s.to_string()) else { return inst.clone() };
    let file = super::signlogo::logo_file(&kind);
    if !m.pictures.iter().any(|(f, _)| *f == file) {
        // no picture was produced for this kind (no pack texture): the pseudo
        // link would resolve to nothing — fall back to the kind's Sign material
        let mut owned = inst.clone();
        if let Some(main) = owned.main.as_mut() {
            main.link = crate::crystal_model::Id::Str(format!("Stadium\\Media\\Modifier\\{kind}\\Sign"));
        }
        return owned;
    }
    let mut owned = inst.clone();
    if let Some(main) = owned.main.as_mut() {
        main.is_using_game_material = false;
        // TINY_SIGN_MODEL=TDSN (default) — the shading model of the panel.
        // Measured on a lineup of the 16m Turbo gate (Summer 20 host, close-up
        // frames, panel corners sampled): TDSN with the picture in slot 0
        // alone gives BLACK cells (0x0a0e12) and a sunlit logo (0x919712);
        // ANY use of slot 5 (the self-illumination, in TDSN or TDSNI) turns the
        // black cells into a sky-coloured grey (0x21303e here, 0x5d under
        // Summer 19's hazy sky against the original's 0x18) — the illum term
        // adds an ambient over the whole panel, not just where the picture
        // is lit. Slots 1–4, 6, 7 change nothing; 8 brightens everything; the
        // picture's alpha (full/mask/zero) changes nothing; BaseTexture and
        // TDSNE draw a checker/lighter panel, TDSNEM a flat colour. So the
        // panel is a plain diffuse: black cells like the original, the logo
        // lit by the sun instead of glowing.
        main.model = crate::crystal_model::Id::Str(std::env::var("TINY_SIGN_MODEL").unwrap_or_else(|_| "TDSN".into()));
        main.material_name = crate::crystal_model::Id::Str(format!("SignLogo{kind}"));
        main.link = crate::crystal_model::Id::Null;
        // `TINY_SIGN_SLOTS=0` (the default) names the texture slots the picture
        // fills (0 = diffuse; 5 = self-illumination, see above); the token `b`
        // puts the file into BaseTexture instead (draws a checker — no).
        let tokens: Vec<String> = std::env::var("TINY_SIGN_SLOTS")
            .ok()
            .map(|s| s.split(',').filter(|c| !c.trim().is_empty()).map(|c| c.trim().to_string()).collect())
            .unwrap_or_else(|| vec!["0".into()]);
        main.user_textures.clear();
        for t in tokens {
            if t == "b" {
                main.base_texture = file.clone();
            } else {
                let u01: i32 = t.parse().unwrap_or_else(|_| panic!("TINY_SIGN_SLOTS: {t:?} is not a slot index"));
                main.user_textures.push(crate::crystal_model::UserTexture { u01, texture: file.clone() });
            }
        }
    }
    owned
}

/// Crystal virtual links resolved to the real game-material paths the
/// editor's bake writes (harvested against Granady's items + pak
/// existence). A crystal `Material\Special<Kind><Mod>` link has no
/// `.Material.Gbx` of its own; the baked item carries the existing
/// `Modifier\<Mod>\<Kind>` file instead (both the 2025.7.4 and the current
/// pak contain `Modifier\Turbo\{Sign,SignOff,Decal}.Material.Gbx`; tri-count
/// correspondence on Road_17 is exact: Sign 654idx/218tris, SignOff 54/18,
/// Decal 384/128). `SpecialFXTurbo` has no `Modifier\Turbo\SpecialFX` file,
/// so it stays virtual -- matching all 26 references.
pub const MATERIAL_LINK_RESOLVE: &[(&str, &str)] = &[
    ("Stadium\\Media\\Material\\SpecialSignTurbo", "Stadium\\Media\\Modifier\\Turbo\\Sign"),
    ("Stadium\\Media\\Material\\SpecialSignOff", "Stadium\\Media\\Modifier\\Turbo\\SignOff"),
    ("Stadium\\Media\\Material\\DecalSpecialTurbo", "Stadium\\Media\\Modifier\\Turbo\\Decal"),
];

/// The editor-resolved link for a crystal material link.
pub fn resolve_crystal_link(link: &str) -> &str {
    MATERIAL_LINK_RESOLVE.iter().find(|(v, _)| *v == link).map(|(_, r)| *r).unwrap_or(link)
}

/// Physics id the game gives a library material (harvested from the 26
/// reference items: every link there carries exactly one id). The
/// `.Material.Gbx` files themselves carry none (classes.rs reads their
/// surface id as 0), so this table is what makes a prefab-built item's
/// `CPlugMaterialUserInst.surface_physic_id` match the game's own items.
pub const MATERIAL_PHYSICS: &[(&str, u8)] = &[
    ("Stadium\\Media\\Material\\ChronoFinish", 32),
    ("Stadium\\Media\\Material\\DecoHill", 2),
    ("Stadium\\Media\\Material\\DecoHill2", 2),
    ("Stadium\\Media\\Material\\LightSpot", 32),
    ("Stadium\\Media\\Material\\LightSpot2", 32),
    ("Stadium\\Media\\Material\\PlatformTech", 16),
    ("Stadium\\Media\\Material\\RaceAd6x1", 32),
    ("Stadium\\Media\\Material\\RaceArchFinish", 4),
    ("Stadium\\Media\\Material\\RaceScreenStart", 32),
    ("Stadium\\Media\\Material\\RaceScreenStartSmall", 32),
    ("Stadium\\Media\\Material\\RoadTech", 16),
    ("Stadium\\Media\\Material\\Speedometer", 4),
    ("Stadium\\Media\\Material\\SpeedometerLight", 4),
    ("Stadium\\Media\\Material\\Technics", 4),
    ("Stadium\\Media\\Material\\TechnicsSpecials", 4),
    ("Stadium\\Media\\Material\\TechnicsTrims", 4),
    ("Stadium\\Media\\Material\\TrackBorders", 9),
    ("Stadium\\Media\\Material\\TrackBordersOff", 9),
    ("Stadium\\Media\\Material\\TrackWall", 14),
    ("Stadium\\Media\\Material\\TrackWallClips", 22),
    ("Stadium\\Media\\Modifier\\PlatformGrass\\OpenTechBorders", 76),
    ("Stadium\\Media\\Modifier\\PlatformGrass\\PlatformTech", 76),
    ("Stadium\\Media\\Modifier\\PlatformIce\\DecoHill", 21),
    ("Stadium\\Media\\Modifier\\PlatformDirt\\PlatformTech", 6),
    ("Stadium\\Media\\Modifier\\PlatformIce\\PlatformTech", 74),
    ("Stadium\\Media\\Modifier\\Turbo\\Sign", 32),
    ("Stadium\\Media\\Modifier\\Turbo\\SignOff", 32),
];

/// Physics for a material link: the table, then the rules the table shows
/// (`Decal*`/`SpecialFX*`/`Turbo\Decal` -> NotCollidable 28, `ChronoFinish-*`
/// -> 32), else `None`.
/// The kind-less name a gameplay-gate material has inside a
/// `Modifier\<Kind>\` folder: the prefab's `SpecialSignTurbo` is the folder's
/// `Sign`, `SpecialSignOff` → `SignOff`, `SpecialFXTurbo` → `SpecialFX`,
/// `TriggerFXTurbo` → `TriggerFX`, `DecalSpecialTurbo` → `Decal` (the base
/// prefab wears the Turbo dress; the pak's `Modifier\Turbo\` holds exactly
/// these files for it). Anything else: `None`.
pub fn gate_special_stem(stem: &str) -> Option<&'static str> {
    Some(match stem {
        "SpecialSignTurbo" => "Sign",
        "SpecialSignOff" => "SignOff",
        "SpecialFXTurbo" => "SpecialFX",
        "TriggerFXTurbo" => "TriggerFX",
        "DecalSpecialTurbo" => "Decal",
        _ => return None,
    })
}

pub fn physics_for_link(link: &str) -> Option<u8> {
    let l = link.to_ascii_lowercase();
    if let Some((_, p)) = MATERIAL_PHYSICS.iter().find(|(k, _)| k.to_ascii_lowercase() == l) {
        return Some(*p);
    }
    let base = l.rsplit('\\').next().unwrap_or(&l);
    if base.starts_with("decal") || base.starts_with("specialfx") || base.starts_with("racetriggerfx") {
        return Some(28);
    }
    if base.starts_with("chronofinish") {
        return Some(32);
    }
    // every gameplay kind's `Modifier\<Kind>\Sign|SignOff` is the Turbo one's (32)
    if l.contains("\\modifier\\") && (base == "sign" || base == "signoff") {
        return Some(32);
    }
    None
}

/// The physics id most of a static object's collision triangles carry.
pub fn most_common_physics(so: &super::item::CPlugStaticObjectModel) -> Option<u8> {
    let sf = so.surface()?;
    let Surf::Mesh { triangles, .. } = &sf.surf else { return None };
    let mut counts = [0usize; 256];
    for t in triangles {
        counts[t.material_id as usize] += 1;
    }
    let (best, n) = counts.iter().enumerate().max_by_key(|(_, n)| **n)?;
    (*n > 0).then_some(best as u8)
}

/// Merge visuals that share a material slot AND a vertex layout (declaration
/// list, flags, no skin / tex-coord sets) into one, as the reference items
/// have exactly one visual per material. Vertex data is concatenated, indices
/// offset; a group is split when it would pass 65000 vertices (u16 indices).
pub fn coalesce(visuals: &[MergedVisual]) -> Vec<MergedVisual> {
    let mut out: Vec<MergedVisual> = Vec::new();
    for mv in visuals {
        let (Some(main), Some(stream)) = (mv.visual.main.as_ref(), mv.visual.stream()) else {
            out.push(mv.clone());
            continue;
        };
        // a visual with a frame table (a tween cloth's LOD levels) is never merged
        let mergeable = main.tex_coord_sets.is_empty() && main.skin.is_none() && stream.base.index == -1 && main.vertex_streams.len() == 1 && mv.visual.sub_visuals.is_empty();
        if !mergeable {
            out.push(mv.clone());
            continue;
        }
        let key = |v: &CPlugVisualIndexedTriangles| -> Option<(Vec<(u32, u32)>, u32, Option<bool>, Vec<u32>)> {
            let m = v.main.as_ref()?;
            let s = v.stream()?;
            Some((s.decls.iter().map(|d| (d.flags1, d.flags2)).collect(), m.chunk_flags, s.compress_local3d, v.chunks.clone()))
        };
        let k = key(&mv.visual);
        let target = out.iter_mut().find(|o| o.material == mv.material && o.lod_mask == mv.lod_mask && o.lod_ladder == mv.lod_ladder && key(&o.visual) == k && k.is_some() && o.visual.main.as_ref().map(|m| m.count as usize).unwrap_or(0) + main.count as usize <= 65000);
        match target {
            None => out.push(mv.clone()),
            Some(o) => append_visual(&mut o.visual, &mv.visual),
        }
    }
    out
}

fn append_visual(dst: &mut CPlugVisualIndexedTriangles, src: &CPlugVisualIndexedTriangles) {
    let src_main = src.main.as_ref().unwrap();
    let src_stream = src.stream().unwrap().clone();
    let src_idx = src.index_buffer.as_ref().map(|b| b.indices.clone()).unwrap_or_default();
    let dm = dst.main.as_mut().unwrap();
    let base = dm.count as u32;
    dm.count += src_main.count;
    let ds = match dm.vertex_streams.first_mut().and_then(|r| r.inline.as_deref_mut()) {
        Some(Node::VertexStream(s)) => s,
        _ => unreachable!("coalesce checked the stream"),
    };
    ds.count += src_stream.count;
    for (d, s) in ds.elems.iter_mut().zip(src_stream.elems.iter()) {
        match (d, s) {
            (Elem::Float2(a), Elem::Float2(b)) => a.extend_from_slice(b),
            (Elem::Float3(a), Elem::Float3(b)) => a.extend_from_slice(b),
            (Elem::Float4(a), Elem::Float4(b)) => a.extend_from_slice(b),
            (Elem::Word(a), Elem::Word(b)) => a.extend_from_slice(b),
            (Elem::Raw { bytes: a, .. }, Elem::Raw { bytes: b, .. }) => a.extend_from_slice(b),
            _ => unreachable!("coalesce matched the layouts"),
        }
    }
    if let Some(Elem::Float3(p)) = ds.elems.first() {
        dm.bounding_box = bbox(p);
    }
    match &mut dst.index_buffer {
        Some(ib) => ib.indices.extend(src_idx.iter().map(|i| i + base)),
        None => dst.index_buffer = Some(super::visual::IndexBuffer::delta(src_idx.iter().map(|i| i + base).collect())),
    }
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

/// A `.Light.Gbx` of the packs (`CPlugLight`, class 0x0901D000), its GxLight
/// inline.
pub fn load_light(store: &mut crate::store::DataStore, path: &str) -> R<super::light::CPlugLight> {
    let model = store.load_model(path)?;
    if model.class_id != super::light::C_PLUG_LIGHT {
        return Err(format!("{path}: class 0x{:08X} is not CPlugLight", model.class_id));
    }
    let mut lb = super::LookbackState::default();
    lb.defined_nodes.extend(model.external_indices().iter().copied());
    let mut r = super::Rd::new(&model.body, 0, lb);
    super::light::CPlugLight::parse(&mut r).map_err(|e| format!("{path}: {e}"))
}

impl Merged {
    /// Read every pending socket's `.Light.Gbx` from the store, drop the
    /// pack references it carries (flare/projector bitmaps, colour table),
    /// scale its radii, and queue it for the item's `lights` list.
    pub fn resolve_pending_lights(&mut self, store: &mut crate::store::DataStore) {
        for (path, socket, scale) in std::mem::take(&mut self.pending_lights) {
            match load_light(store, &path) {
                Ok(mut light) => {
                    // the pack files the light names (for the `extern` probes)
                    let bitmaps: Vec<(String, u8)> = store.load_model(&path).map(|mm| light.external_slots(&mm.externals)).unwrap_or_default();
                    // A light driven by an animation image (the checkpoint
                    // gates' blue speedometer LEDs: `SpeedometerCP.Light.Gbx`
                    // with `Anims\Speedometer.tga`; the TurboRoulette colour
                    // cycle) or by a CFuncLight is a GAMEPLAY light: idle it is
                    // dark, and without its driver it would burn at full
                    // strength — Summer 17's gates bloomed white until these
                    // were left out (2026-09-07).
                    if light.is_animated() {
                        self.notes.push(format!("{path}: animated light (image anim / func light) not embedded — idle it is off"));
                        continue;
                    }
                    // The gameplay gates' lights — `SpecialSpot` / `SpecialFXLight`
                    // of the Turbo/Boost/Reset/NoEngine… Special prefabs (base
                    // `Media\Light\Special*.Light.Gbx`, per-kind copies under
                    // `Media\Modifier\<Kind>\`) and the GateGameplay spots: they
                    // fire when a car passes and are dark at rest — steady ON
                    // they streaked Summer 19's decks white (maps loop, 09:39).
                    let lower = path.to_ascii_lowercase();
                    let file = lower.rsplit('\\').next().unwrap_or(&lower);
                    if lower.contains("\\modifier\\") || file.starts_with("special") {
                        self.notes.push(format!("{path}: gameplay-gate light not embedded — at rest it is off"));
                        continue;
                    }
                    light.drop_external_refs();
                    match light.gx_mut().and_then(|r| r.inline.as_deref_mut()) {
                        Some(super::Node::GxLight(g)) => {
                            g.scale(scale);
                            // the placement's light colour skin: the swatch
                            // multiplies the light (the stock item's projector
                            // and glow textures are replaced by it)
                            if let Some(skin) = &self.light_skin {
                                if skin.is_off() {
                                    self.notes.push(format!("{path}: light skin {} is OFF; light dropped", skin.name));
                                    continue;
                                }
                                g.tint(skin.linear);
                            }
                        }
                        _ => {
                            self.notes.push(format!("{path}: no inline GxLight; light dropped"));
                            continue;
                        }
                    }
                    let (color, intensity, range) = light.gx_light().map(|g| g.summary()).unwrap_or_default();
                    self.notes.push(format!(
                        "light {:?} from {path}: colour [{:.2}, {:.2}, {:.2}] intensity {intensity} range {range} at [{:.2}, {:.2}, {:.2}] rot [{:.2} {:.2} {:.2} | {:.2} {:.2} {:.2} | {:.2} {:.2} {:.2}] ints {:?} u15 {} {:?}",
                        socket.u01, color[0], color[1], color[2], socket.u05[9], socket.u05[10], socket.u05[11],
                        socket.u05[0], socket.u05[1], socket.u05[2], socket.u05[3], socket.u05[4], socket.u05[5], socket.u05[6], socket.u05[7], socket.u05[8],
                        socket.ints, socket.u15, socket.u16
                    ));
                    self.lights_out.push(MergedLight { socket, light, source: path, bitmaps });
                }
                Err(e) => self.notes.push(format!("{path}: light not embedded: {e}")),
            }
        }
    }
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

/// A material whose shader tweens between vertex frames
/// (`Tech3_Warp_TDiffSpec_VertexTween`, the flag cloth's `ItemFlag`).
fn is_tween_material(store: &mut crate::store::DataStore, p: &str) -> bool {
    store.load_model(p).map(|mm| mm.externals.iter().any(|(_, e)| e.to_ascii_lowercase().contains("tween"))).unwrap_or(false)
}

/// A vertex-tweened cloth (the flag) is kept as a dyna entity of its own with
/// its frames, frame table, tween material and the pack's inline-vertex form
/// — the waving, hue-masked flag of 2026-09-07. `TINY_FLAG_TWEEN=0` bakes it
/// as frame 0 under TrackBorders instead (the still white flag of before).
pub fn tween_parts_enabled() -> bool {
    // OFF by default until the many-instance draw is right: ten or more
    // copies of the tween item in one map drew as giant black sails or
    // nothing (Summer 20, 2026-09-07), two copies drew right. TINY_FLAG_TWEEN=1
    // turns it on.
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
        if band != [0.90, 0.97] {
            m.notes.push(format!("TINY_FLAG_BAND={},{}: cloth uv0 mapped into that TrackBorders v band", band[0], band[1]));
        }
        for g in &s2.shaded_geoms {
            if !tween_mats.get(g.material_index.max(0) as usize).copied().unwrap_or(false) {
                continue;
            }
            if let Some(Node::Visual(v)) = s2.visuals.get_mut(g.visual_index as usize).and_then(|r| r.inline.as_deref_mut()) {
                if let Some(Node::VertexStream(s)) = v.main.as_mut().and_then(|mn| mn.vertex_streams.first_mut()).and_then(|r| r.inline.as_deref_mut()) {
                    for (d, e) in s.decls.iter().zip(s.elems.iter_mut()) {
                        if d.name() == N_TEXCOORD0 {
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
            tween_notes.push(format!("{p}: vertex-tween shader; drawn as TrackBorders (uv0 in the hue-masked stripe band)"));
            return Some((p, "Stadium\\Media\\Material\\TrackBorders".to_string(), 9));
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
    mesh.editors = m.editors;
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
            tween_notes.push(format!("{p}: vertex-tween shader; drawn as TrackBorders (uv0 in the hue-masked stripe band)"));
            return Some((p, "Stadium\\Media\\Material\\TrackBorders".to_string(), 9));
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
    mesh.editors = m.editors;
    mesh.keep_water = m.keep_water;
    mesh.modifier = m.modifier.clone();
    mesh.collision_redress = m.collision_redress.clone();
    mesh.modifier_suffix = m.modifier_suffix.clone();
    mesh.no_split = true;
    mesh.all_lods = true;
    mesh.vis_cst_type = Some(src.s2.vis_cst_type);
    // its ladder ([16, 64, 128, 512], five levels) is registered, scaled, by
    // add_static_object like every part's
    mesh.solid2_u07 = Some(src.s2.u07);
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
    m.notes.push(format!("{}: TWEEN part, {} visuals [{}], no constraint, params 0x{:X} ({} bytes)", path.rsplit('\\').next().unwrap_or(path), mesh.visuals.len(), frames.join("; "), ent.params_id, ent.params.len()));
    m.notes.extend(mesh.notes.drain(..).map(|n| format!("  (tween part) {n}")));
    m.dyna.push(DynaPart { path: path.to_string(), rot, pos, mesh, move_shape, hit_shape, model: src.model.clone(), instance_params_id: ent.params_id, instance_params: ent.params.clone(), constraint: None });
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
    m.editors = std::env::var_os("TINY_EDITORS").is_some();
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
    if super::signlogo::enabled() {
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
    let opts = BuildOpts { ident: ident.to_string(), author: author.to_string(), scale, collection, editors: m.editors, skin: m.skin.clone() };
    let f = assemble(&m, &opts)?;
    m.pictures.extend(LIGHT_FILES.with(|l| std::mem::take(&mut *l.borrow_mut())));
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

/// The "StadiumOnTerrain" game skin: in a BlueBay map every Stadium-family
/// block draws some of its materials through
/// `BlueBay\Media\Modifier\StadiumOnTerrain\<slot>.Material.Gbx` instead of
/// `Stadium\Media\Material\<name>` (the slot table is
/// `Stadium\GameSkin\StadiumOnTerrain.GameSkin.gbx`). Items know nothing of
/// skins, so the link is remapped here: without it the wall faces under the
/// stands drew Stadium's wooden `TrackWallClips` where the original shows
/// BlueBay's concrete `TrackWallClipsInWorld` (2026-09-06).
/// `TINY_NO_SKIN=1` disables the remap.
/// Whether a collection's `Water` visuals stay in the bake: Stadium's pools
/// are drawn by the `WaterBase` blocks themselves (no water zone to fall back
/// on); BlueBay / RedIsland / WhiteShore / GreenCoast regenerate their sea or
/// lake from the genealogy at that very height. `TINY_WATER=keep|drop`
/// overrides for experiments.
pub fn keep_water_for(collection: u32) -> bool {
    match std::env::var("TINY_WATER").as_deref() {
        Ok("keep") => true,
        Ok("drop") => false,
        _ => collection == 0x1a,
    }
}

/// The environment folder of a map collection id (the pack's root folder).
pub fn env_name(collection: u32) -> &'static str {
    match collection {
        0x1c => "BlueBay",
        0x1a => "Stadium",
        0x10 => "RedIsland",
        0x1d => "WhiteShore",
        0xf => "GreenCoast",
        _ => "BlueBay",
    }
}

/// `TINY_MAT_CUSTOM="Stem=mode[:path],…"`: rewrite one material link into
/// the engine's CUSTOM-texture form (the ManiaPlanet item-editor material:
/// `IsUsingGameMaterial` off, a shading `Model`, textures named by path) —
/// the probe of 2026-09-07 for whether a texture referenced by PATH becomes
/// a fid the game's skin remap (the in-game advertisement) can reach.
/// Modes: `base` (Model TDSN, BaseTexture = path), `basefile` (same with the
/// file name), `user` (UserTextures slot 0 = path, Model TDSN), `linkuser`
/// (game link kept, UserTextures slot 0 = path). Experiments only.
pub fn custom_texture_material(inst: &CPlugMaterialUserInst, ident: &str) -> CPlugMaterialUserInst {
    // TINY_PICTURES=DIR: the production form. A material whose link stem has a
    // `<stem>.dds` in DIR draws that picture — a custom-texture material with
    // the texture named by file name, which the game resolves in the item's
    // own archive folder (tiny-library puts every DIR/*.dds into the library
    // zip as Items/<stem>.dds). Several pictures per stem (`<stem>.dds`,
    // `<stem>.2.dds`, …) are spread over the models by ident hash — every
    // placement of one model shows the same one. TINY_PICTURES_MODEL=TDSN|TDSNI
    // (default TDSNI: slot 0 diffuse + slot 5 self-illumination, the lit-screen
    // look).
    if let Some(dir) = std::env::var_os("TINY_PICTURES") {
        if let Some(link) = inst.link().map(|s| s.to_string()) {
            let stem = link.rsplit('\\').next().unwrap_or(&link).to_string();
            let mut choices: Vec<String> = std::fs::read_dir(&dir)
                .map(|rd| {
                    rd.filter_map(|e| e.ok())
                        .map(|e| e.file_name().to_string_lossy().into_owned())
                        .filter(|n| n.to_ascii_lowercase().ends_with(".dds") && (n == &format!("{stem}.dds") || n.starts_with(&format!("{stem}."))))
                        .collect()
                })
                .unwrap_or_default();
            choices.sort();
            if !choices.is_empty() {
                let h: usize = ident.bytes().fold(5381usize, |h, b| h.wrapping_mul(33).wrapping_add(b as usize));
                let file = choices[h % choices.len()].clone();
                let model = std::env::var("TINY_PICTURES_MODEL").unwrap_or_else(|_| "TDSNI".into());
                let mut owned = inst.clone();
                if let Some(main) = owned.main.as_mut() {
                    main.is_using_game_material = false;
                    main.model = crate::crystal_model::Id::Str(model.clone());
                    main.material_name = crate::crystal_model::Id::Str(stem.clone());
                    main.link = crate::crystal_model::Id::Null;
                    main.user_textures = vec![crate::crystal_model::UserTexture { u01: 0, texture: file.clone() }];
                    if model == "TDSNI" {
                        main.user_textures.push(crate::crystal_model::UserTexture { u01: 5, texture: file.clone() });
                    }
                }
                return owned;
            }
        }
    }
    let Ok(list) = std::env::var("TINY_MAT_CUSTOM") else { return inst.clone() };
    let Some(link) = inst.link().map(|s| s.to_string()) else { return inst.clone() };
    let stem = link.rsplit('\\').next().unwrap_or(&link).to_string();
    for entry in list.split(',') {
        let Some((from, spec)) = entry.split_once('=') else { continue };
        if from != stem {
            continue;
        }
        let (mode, path) = spec.split_once(':').unwrap_or((spec, ""));
        let mut owned = inst.clone();
        let Some(main) = owned.main.as_mut() else { return owned };
        match mode {
            "base" | "basefile" => {
                main.is_using_game_material = false;
                main.model = crate::crystal_model::Id::Str("TDSN".into());
                main.material_name = crate::crystal_model::Id::Str(stem.clone());
                main.base_texture = path.to_string();
                main.link = crate::crystal_model::Id::Null;
            }
            "user" => {
                main.is_using_game_material = false;
                main.model = crate::crystal_model::Id::Str("TDSN".into());
                main.material_name = crate::crystal_model::Id::Str(stem.clone());
                main.link = crate::crystal_model::Id::Null;
                main.user_textures = vec![crate::crystal_model::UserTexture { u01: 0, texture: path.to_string() }];
            }
            "linkuser" => {
                main.user_textures = vec![crate::crystal_model::UserTexture { u01: 0, texture: path.to_string() }];
            }
            // tex:MODEL:slot=path;slot=path — any shading model, any slots
            "tex" => {
                let (model, slots) = path.split_once(':').unwrap_or((path, ""));
                main.is_using_game_material = false;
                main.model = crate::crystal_model::Id::Str(model.to_string());
                main.material_name = crate::crystal_model::Id::Str(stem.clone());
                main.link = crate::crystal_model::Id::Null;
                main.user_textures = slots
                    .split(';')
                    .filter(|s| !s.is_empty())
                    .filter_map(|s| s.split_once('='))
                    .map(|(slot, p)| crate::crystal_model::UserTexture { u01: slot.parse().unwrap_or(0), texture: p.to_string() })
                    .collect();
            }
            _ => {}
        }
        // TINY_MAT_UVANIM="u01|u02|u03|u04hex|u05": one UvAnim entry on the
        // rewritten material (the chunk's v3+ list: Id, Id, f32, u64, Id) —
        // the 2026-09-07 probe of whether a custom material can scroll its
        // texture. Ids: `-` = null, else the string.
        if let Ok(spec) = std::env::var("TINY_MAT_UVANIM") {
            let f: Vec<&str> = spec.split('|').collect();
            if f.len() == 5 {
                let id = |s: &str| if s == "-" { crate::crystal_model::Id::Null } else { crate::crystal_model::Id::Str(s.to_string()) };
                main.uv_anims = vec![crate::crystal_model::UvAnim {
                    u01: id(f[0]),
                    u02: id(f[1]),
                    u03: f[2].parse().unwrap_or(1.0),
                    u04: u64::from_str_radix(f[3].trim_start_matches("0x"), 16).unwrap_or(0),
                    u05: id(f[4]),
                }];
            }
        }
        return owned;
    }
    inst.clone()
}

pub fn skinned_material(inst: &CPlugMaterialUserInst, collection: u32) -> CPlugMaterialUserInst {
    // TINY_MAT_SUBST="Stem=Other,…": rewrite a material link stem (any family) before the skin remap — experiments.
    let inst = &match std::env::var("TINY_MAT_SUBST") {
        Ok(list) => {
            let mut owned = inst.clone();
            if let Some(link) = inst.link().map(|s| s.to_string()) {
                let (dir, stem) = match link.rfind('\\') { Some(i) => (&link[..=i], &link[i + 1..]), None => ("", link.as_str()) };
                for pair in list.split(',') {
                    if let Some((from, to)) = pair.split_once('=') {
                        if from == stem {
                            if let Some(main) = owned.main.as_mut() {
                                main.link = crate::crystal_model::Id::Str(format!("{dir}{to}"));
                            }
                        }
                    }
                }
            }
            owned
        }
        Err(_) => inst.clone(),
    };
    const SKIN: &[(&str, &str)] = &[
        ("TrackWallClips", "TrackWallClipsInWorld"),
        ("TrackWall", "TrackWallInWorld"),
        ("TrackBorders", "TrackBordersInWorld"),
        ("TrackBordersOff", "TrackBordersOffInWorld"),
        ("Structure", "StructureInWorld"),
        ("Deco", "Deco"),
        ("DecoHill", "DecoHill"),
        ("DecoHill2", "DecoHill2"),
        ("DecalPaintSponsor4x1D", "DecalPaintSponsor4x1D"),
        ("DecalPaint2Sponsor4x1D", "DecalPaint2Sponsor4x1D"),
        ("DecalPaint2Sponsor4x1NoColorizeD", "DecalPaint2Sponsor4x1NoColorizeD"),
        ("DecalPaintSponsor4x1NoColorizeD", "DecalPaintSponsor4x1NoColorizeD"),
    ];
    // every terrain environment carries `<Env>\Media\Modifier\StadiumOnTerrain\`
    // with the same slots (BlueBay and RedIsland checked); Stadium itself has none
    if collection == 0x1a || std::env::var_os("TINY_NO_SKIN").is_some() {
        return inst.clone();
    }
    let env = env_name(collection);
    let Some(link) = inst.link() else { return inst.clone() };
    // The skin is applied AFTER the block's material modifier, by material
    // stem: Summer 16's OpenDirtZone blocks (modifier PlatformDirt, whose
    // folder has its own `Deco` = DecoHillDirt) draw BlueBay grass in the
    // game, not dirt — `Modifier\PlatformDirt\Deco` still lands on the skin's
    // `Deco` slot. So a `Stadium\Media\Modifier\<X>\<stem>` link is skinned
    // like the plain material of the same stem.
    let stem = match link.strip_prefix("Stadium\\Media\\Material\\") {
        Some(stem) => stem,
        None => match link.strip_prefix("Stadium\\Media\\Modifier\\").and_then(|rest| rest.split_once('\\')) {
            Some((_folder, stem)) if !stem.contains('\\') => stem,
            _ => return inst.clone(),
        },
    };
    let Some((_, slot)) = SKIN.iter().find(|(name, _)| *name == stem) else { return inst.clone() };
    let mut owned = inst.clone();
    if let Some(main) = owned.main.as_mut() {
        main.link = crate::crystal_model::Id::Str(format!("{env}\\Media\\Modifier\\StadiumOnTerrain\\{slot}"));
    }
    owned
}

/// Remove one declared element from a vertex stream; the remaining
/// declarations keep their order and get fresh cumulative offsets and stride.
pub fn drop_element(s: &mut super::vstream::CPlugVertexStream, name: u32) {
    use super::vstream::Decl;
    if !s.decls.iter().any(|d| d.name() == name) {
        return;
    }
    let compress = s.compress_local3d.unwrap_or(false);
    let kept: Vec<(Decl, Elem)> = s.decls.iter().zip(s.elems.iter()).filter(|(d, _)| d.name() != name).map(|(d, e)| (d.clone(), e.clone())).collect();
    let stride: u32 = kept.iter().map(|(d, _)| super::vstream::type_size(d.stored_type(compress)).unwrap_or(4) as u32).sum();
    let mut offset = 0u32;
    let mut decls = Vec::new();
    let mut elems = Vec::new();
    for (d, e) in kept {
        let size = super::vstream::type_size(d.stored_type(compress)).unwrap_or(4) as u32;
        decls.push(Decl::with_stride(d.name(), d.ty(), d.space(), offset, stride / 4));
        offset += size;
        elems.push(e);
    }
    s.decls = decls;
    s.elems = elems;
}

#[cfg(test)]
mod lod_tests {
    use super::*;

    fn geom(mask: i32) -> ShadedGeom {
        ShadedGeom { visual_index: 0, material_index: 0, u01: -1, lod_mask: mask, u02: 0 }
    }

    #[test]
    fn ladder_length_is_distances_plus_one_or_highest_bit() {
        // RoadTech Straight_Air: [64, 128] with masks 1/2/4
        assert_eq!(lod_levels_of(&[64.0, 128.0], &[geom(1), geom(2), geom(4)]), 3);
        // Sparkler8m: [16, 128, 256] with masks 1/2/4 only — the 4th level is
        // empty (culled past 256 m)
        assert_eq!(lod_levels_of(&[16.0, 128.0, 256.0], &[geom(1), geom(2), geom(4)]), 4);
        // a mask bit past the distances counts as an unbounded last level
        assert_eq!(lod_levels_of(&[64.0], &[geom(1), geom(2), geom(4)]), 3);
        // no ladder at all
        assert_eq!(lod_levels_of(&[], &[geom(1), geom(1)]), 1);
        assert_eq!(lod_levels_of(&[], &[geom(0)]), 1);
    }

    #[test]
    fn masks_move_onto_the_merged_ladder_by_range() {
        // a 3-level part [32, 64] on a 4-level item [32, 64, 128]: its last
        // level (past 64) spans bits 2 and 3
        assert_eq!(remap_lod_mask(1, &[32.0, 64.0], &[32.0, 64.0, 128.0]), 1);
        assert_eq!(remap_lod_mask(2, &[32.0, 64.0], &[32.0, 64.0, 128.0]), 2);
        assert_eq!(remap_lod_mask(4, &[32.0, 64.0], &[32.0, 64.0, 128.0]), 4 | 8);
        // a 2-level part [32] on the same item
        assert_eq!(remap_lod_mask(2, &[32.0], &[32.0, 64.0, 128.0]), 2 | 4 | 8);
        // Flag16m: pole [8, 32, 256] + cloth [8, 32, 64, 256] -> [8, 32, 64, 256];
        // the pole's third level (32..256) spans bits 2 and 3, its fourth
        // (past 256) bit 4
        let mut merged = Vec::new();
        merge_lod_ladder(&mut merged, &[8.0, 32.0, 256.0]);
        merge_lod_ladder(&mut merged, &[8.0, 32.0, 64.0, 256.0]);
        assert_eq!(merged, vec![8.0, 32.0, 64.0, 256.0]);
        assert_eq!(remap_lod_mask(4, &[8.0, 32.0, 256.0], &merged), 4 | 8);
        assert_eq!(remap_lod_mask(8, &[8.0, 32.0, 256.0], &merged), 16);
        assert_eq!(remap_lod_mask(8, &[8.0, 32.0, 64.0, 256.0], &merged), 8);
        // a culled part (Sparkler8m: [8, 64, 128], geoms up to bit 2) stays
        // culled past 128 on a longer ladder
        assert_eq!(remap_lod_mask(4, &[8.0, 64.0, 128.0], &[8.0, 64.0, 128.0, 256.0]), 4);
        // a one-level part, or mask 0: every level
        assert_eq!(remap_lod_mask(1, &[], &[32.0, 64.0]), 7);
        assert_eq!(remap_lod_mask(0, &[32.0], &[32.0, 64.0]), 7);
        // a multi-bit mask
        assert_eq!(remap_lod_mask(7, &[32.0, 64.0], &[32.0, 64.0, 128.0]), 15);
        // same ladder: unchanged
        assert_eq!(remap_lod_mask(4, &[32.0, 64.0], &[32.0, 64.0]), 4);
        // merging ignores duplicates and keeps the order
        let mut l = vec![32.0, 128.0];
        merge_lod_ladder(&mut l, &[64.0, 128.0, 32.0]);
        assert_eq!(l, vec![32.0, 64.0, 128.0]);
    }

    #[test]
    fn ladders_are_capped_at_four_levels() {
        // Flag16m's union [8, 32, 64, 256]: the closest step pair is
        // (32, 64), the larger goes
        let mut l = vec![8.0, 32.0, 64.0, 256.0];
        cap_lod_ladder(&mut l, MAX_LOD_LEVELS - 1);
        assert_eq!(l, vec![8.0, 32.0, 256.0]);
        // the cloth [8, 32, 64, 256] on the capped ladder: its level 2
        // (32..64) is active at the start of the merged level 32..256, so it
        // draws through it; level 3 (64..256) is never drawn
        assert_eq!(remap_lod_mask(4, &[8.0, 32.0, 64.0, 256.0], &l), 4);
        assert_eq!(remap_lod_mask(8, &[8.0, 32.0, 64.0, 256.0], &l), 0);
        assert_eq!(remap_lod_mask(16, &[8.0, 32.0, 64.0, 256.0], &l), 8);
        // the pole [8, 32, 256] is exact on it
        assert_eq!(remap_lod_mask(4, &[8.0, 32.0, 256.0], &l), 4);
        assert_eq!(remap_lod_mask(8, &[8.0, 32.0, 256.0], &l), 8);
        // a short ladder is left alone
        let mut s = vec![32.0, 64.0];
        cap_lod_ladder(&mut s, 3);
        assert_eq!(s, vec![32.0, 64.0]);
    }
}

/// The glass of a light item under a light colour skin (light_skin.rs): a
/// material whose pack file carries a self-illumination texture becomes a
/// self-lit custom-texture material with the swatch as diffuse AND
/// illumination (`Items/LightColor_<Name>.dds`), the way the skin replaces the
/// stock item's `_I` textures. `Off` glows black. `TINY_LIGHT_SKIN_GLASS=off`
/// keeps the game material (white glow).
pub fn light_skin_material(inst: &CPlugMaterialUserInst, m: &Merged) -> CPlugMaterialUserInst {
    let Some(skin) = m.light_skin.as_ref() else { return inst.clone() };
    if std::env::var("TINY_LIGHT_SKIN_GLASS").map(|v| v == "off").unwrap_or(false) {
        return inst.clone();
    }
    let Some(link) = inst.link().map(|s| s.to_string()) else { return inst.clone() };
    if !m.illum_links.iter().any(|l| *l == link) {
        return inst.clone();
    }
    let stem = link.rsplit('\\').next().unwrap_or(&link).to_string();
    let file = skin.file();
    // TINY_LIGHT_SKIN_GLASS=<Model>[+cst=Name:value;…]: the shading model of the
    // glass (default TDSNI) and optional material constants (a Cst row: name,
    // "", the f32 bits). Probed 2026-09-07 on a Red LightTubeBig4m against the
    // stock skinned tube (whose pack material has SelfIllumScale 1.5 + a
    // refract layer + a _G glow map): TDSNI and TDSNEM glow red but dimmer,
    // TDSNE/TDSNI_Night dimmer still, TIAdd invisible, a SelfIllumScale cst
    // turned the glass BLACK (the row is read, the encoding is not this).
    let spec = std::env::var("TINY_LIGHT_SKIN_GLASS").unwrap_or_else(|_| "TDSNI".into());
    let (model, extra) = spec.split_once('+').unwrap_or((spec.as_str(), ""));
    let mut owned = inst.clone();
    if let Some(main) = owned.main.as_mut() {
        main.is_using_game_material = false;
        main.model = crate::crystal_model::Id::Str(model.to_string());
        main.material_name = crate::crystal_model::Id::Str(format!("{stem}{}", skin.name));
        main.link = crate::crystal_model::Id::Null;
        main.user_textures = vec![crate::crystal_model::UserTexture { u01: 0, texture: file.clone() }, crate::crystal_model::UserTexture { u01: 5, texture: file }];
        if let Some((_, list)) = extra.split_once("cst=") {
            for kv in list.split(';') {
                if let Some((name, v)) = kv.split_once(':') {
                    let f: f32 = v.parse().unwrap_or(1.0);
                    main.csts.push(crate::crystal_model::Cst { u01: crate::crystal_model::Id::Str(name.to_string()), u02: crate::crystal_model::Id::Null, u03: f.to_bits() as i32 });
                }
            }
        }
    }
    owned
}

#[cfg(test)]
mod material_slot_tests {
    use super::*;
    use crate::crystal_model::{Cst, Id};

    /// A modeler item's `CustomPlastic` slot with one `TargetColor`.
    fn plastic(name: &str, rgb: [f32; 3]) -> CPlugMaterialUserInst {
        let mut m = CPlugMaterialUserInst::game_material("Stadium\\Media\\Material_BlockCustom\\CustomPlastic", 77);
        let main = m.main.as_mut().unwrap();
        main.material_name = Id::Str(name.to_string());
        main.csts = vec![Cst { u01: Id::Str("TargetColor".into()), u02: Id::Str("Real".into()), u03: 3 }];
        main.color = rgb.iter().map(|c| c.to_bits() as i32).collect();
        m
    }

    #[test]
    fn colour_slots_stay_distinct_and_equal_looks_merge() {
        // the Guanako: same link + physics, seven colours → seven slots
        let mut m = Merged::default();
        let fur = m.material_inst_slot(&plastic("TM_Argentina_CustomPlastic43", [0.462, 0.130, 0.053]), "Stadium\\Media\\Material\\");
        let belly = m.material_inst_slot(&plastic("TM_Argentina_CustomPlastic44", [0.723, 0.730, 0.687]), "Stadium\\Media\\Material\\");
        assert_ne!(fur, belly);
        // the same colour under another author-side name is the same slot
        let fur_again = m.material_inst_slot(&plastic("TM_Argentina_CustomPlastic43S1", [0.462, 0.130, 0.053]), "Stadium\\Media\\Material\\");
        assert_eq!(fur, fur_again);
        assert_eq!(m.materials.len(), 2);
        // the constants travel
        let main = m.materials[fur].main.as_ref().unwrap();
        assert_eq!(main.csts.len(), 1);
        assert_eq!(f32::from_bits(main.color[0] as u32), 0.462);
    }

    #[test]
    fn a_bare_modeler_link_becomes_the_full_game_material() {
        let mut inst = CPlugMaterialUserInst::game_material("TechnicsTrims", 4);
        let main = inst.main.as_mut().unwrap();
        main.is_using_game_material = false;
        main.material_name = Id::Str("TM_TechnicsTrims_asset".into());
        let mut m = Merged::default();
        let slot = m.material_inst_slot(&inst, "Stadium\\Media\\Material\\");
        let out = m.materials[slot].main.as_ref().unwrap();
        assert!(out.is_using_game_material);
        assert_eq!(out.link.as_str(), Some("Stadium\\Media\\Material\\TechnicsTrims"));
        // an empty folder means the collection's material folder
        let mut m2 = Merged::default();
        let slot2 = m2.material_inst_slot(&inst, "");
        assert_eq!(m2.materials[slot2].link(), Some("Stadium\\Media\\Material\\TechnicsTrims"));
        // and a full game link is left alone (`Stadium256\…\WarpTechnic`)
        let full = CPlugMaterialUserInst::game_material("Stadium256\\Media\\Material_BlockCustom\\WarpTechnic", 4);
        let slot3 = m2.material_inst_slot(&full, "Stadium\\Media\\Material\\");
        assert_eq!(m2.materials[slot3].link(), Some("Stadium256\\Media\\Material_BlockCustom\\WarpTechnic"));
        // the same look again is the same slot
        assert_eq!(m2.material_inst_slot(&full, ""), slot3);
    }
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
