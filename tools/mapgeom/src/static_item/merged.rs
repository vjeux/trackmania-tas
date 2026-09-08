//! The accumulator of a static item bake — `Merged` — and everything that
//! merges INTO it: material slots (game materials deduplicated by link +
//! physics, modeler instances by look, the block modifier's re-dress),
//! collision triangles (with the modifier's `Redress` table), one static
//! object's visuals with their detail levels and per-layer split, the vertex
//! declaration harmonisation the client's draw merging demands, the
//! coalescing of same-material visuals, and the lights.

use super::solid2::{PreLightGen, ShadedGeom};
use super::surface::{CPlugSurface, Surf, Triangle};
use super::visual::CPlugVisualIndexedTriangles;
use super::vstream::{Elem, N_COLOR0, N_NORMAL, N_POSITION, N_TANGENT_U, N_TANGENT_V, N_TEXCOORD0, T_DEC3N, T_FLOAT3};
use super::{Node, R};
use super::lod::{lod0_only, lod_levels_of, lod_pick, merge_lod_ladder};
use super::materials::{gate_special_stem, material_link, most_common_physics, physics_for_link, resolve_crystal_link, same_look};
use crate::crystal_model::CPlugMaterialUserInst;
use crate::geom::{apply, compose, Xform, IDENTITY};

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
    /// The modifier's re-dress of the prefab's COLLISION materials: the rows
    /// of the block's modifier, GameSkin slot table first, folder shadows
    /// after (`tiny_library::modifier_redress`). A hull triangle whose
    /// surface material matches a row takes the row's (physics, gameplay) —
    /// the deck of a Boost/Reset/NoEngine platform special is authored as
    /// `CollisionTurbo*` (gameplay 1) and drove as a Turbo until the slot-table
    /// rows (Summer 24 cp10, 2026-09-08); every modified platform baked with
    /// the prefab's Asphalt (16) until the shadow rows (vjeux, tiny 18,
    /// 2026-09-08 — "the texture is ice but the driving is also not ice").
    pub collision_redress: Vec<Redress>,
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
    pub keep_water: bool,
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
    /// Keep every detail level of the source whatever `--lod-pick` says,
    /// and never cap or fold its ladder — the tween cloth: ten or more
    /// instances of a LOD0-only copy drew as garbage (giant black sails, or
    /// nothing) while one or two drew right (2026-09-07); the pack's five
    /// levels are the layout the many-instance draw path expects.
    pub all_lods: bool,
    /// The scale the part ladder is registered with instead of the item scale
    /// (`TINY_FLAG_LADDER=pack` on the tween cloth: the PACK distances, so the
    /// cloth switches detail level together with the stock flag that drives it
    /// — the 2026-09-08 finding that the tween draw follows the driver's LOD).
    pub ladder_scale: Option<f32>,
    /// Keep ONE detail level of every part merged into this `Merged`, level
    /// N, whatever `--lod-pick` says (`TINY_FLAG_LODS=N` on
    /// the tween cloth): every instance of the model then renders the same
    /// visual at every distance — the 2026-09-08 finding is that the tween
    /// draw of identical placements goes wrong as soon as they sit at
    /// different detail levels (crumpled shards, giant sails: the full map 10
    /// with its 39 flags spread over 400 m, while 39 in one row were fine).
    pub one_level: Option<u32>,
    /// Solid2 fields carried from the source mesh for a tween part: the pack's
    /// Flag.Mesh.Gbx says `vis_cst_type` 2 (its vertices are not constant —
    /// the frames), `u07` -1; a static item says 1 and 1.
    pub vis_cst_type: Option<i32>,
    pub solid2_u07: Option<i32>,
    /// Solid2 word `u13` carried from the source (`TINY_FLAG_U13=pack` on the
    /// tween cloth): every pack DYNA mesh carries a value with bit 31 set
    /// (Flag.Mesh.Gbx 0x80024108, the pusher piston and rotor 0x8001C779),
    /// every static mesh 0 — the runtime keeps it at Solid2+0x2d8.
    pub solid2_u13: Option<i32>,
    /// Write the materials as the pack meshes do (`TINY_FLAG_MATREF=ext` on
    /// the tween cloth): EXTERNAL references to the pack's own
    /// `<link>.Material.Gbx` files in the Solid2's `materials` array, no
    /// `CPlugMaterialUserInst` at all. The 2026-09-08 exe reading of
    /// CHmsMgrVisDyna::ModelCreate found the material array (`Solid2+0xc8`,
    /// the file refs) is what the animated-model checks look at.
    pub materials_external: bool,
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
    /// Diagnostic forms (`TINY_FLAG_REF=dyna|mesh` on the tween cloth,
    /// 2026-09-08): write the entity model as an EXTERNAL reference to the
    /// pack's own `.DynaObject.Gbx` (`Dyna`: nothing of ours but the prefab
    /// entity and its params) or keep our CPlugDynaObjectModel but point its
    /// mesh at the pack's `.Mesh.Gbx` file (`Mesh`, full size). Neither is a
    /// half-size flag; both say whether an embedded item's dyna entity is
    /// animated at all when its bytes are the pack's.
    pub pack_ref: Option<PackRef>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum PackRef {
    Dyna,
    Mesh(String),
    /// `TINY_FLAG_REF=file`: OUR dyna object and OUR mesh written as two
    /// sidecar FILES next to the item (`<stem>.DynaObject.Gbx`,
    /// `<stem>.Mesh.Gbx`, bare names in the reference tables — the in-archive
    /// form the FX textures proved), so the runtime sees file-backed nodes
    /// with FIDs like the pack's instead of inline ones.
    File,
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

pub fn bbox(points: &[[f32; 3]]) -> [f32; 6] {
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
/// (Summer 06 start island, top-down). Every metre below `water` (the surface,
/// in the item's scaled frame) regains its source depth (divided by `scale`),
/// so the apron sinks back to where the water hides it.
/// Every underwater vertex takes its source depth: the Beach apron is only
/// 0.8..3 m deep in the source and the sea over it reads as open sea from
/// 3 m down, so no band of the shallows keeps the tile's scale.
pub fn restore_depth(m: &mut Merged, water: f32, scale: f32) {
    remap_positions(m, &|p| {
        let depth = water - p[1];
        if depth <= 0.0 {
            p
        } else {
            [p[0], water - depth / scale, p[2]]
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
        let (link, physics) = match super::signlogo::sign_kind(link, self.gate_kind.as_deref()) {
            Some(kind) => {
                pseudo = super::signlogo::pseudo_link(&kind);
                (pseudo.as_str(), 32u8)
            }
            None => (link, physics),
        };
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

    /// Slot of a game-material link.
    pub fn link_slot(&mut self, link: &str, physics: u8) -> usize {
        self.material_slot(link, physics)
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
            let si = self.surf_id_slot(t.material_id as u16 | ((t.gameplay as u16) << 8));
            self.surf_triangles.push(Triangle { indices: [t.indices[0] + base, t.indices[1] + base, t.indices[2] + base], material_id: t.material_id, gameplay: t.gameplay, surface_index: si });
        }
    }

    /// The block modifier's re-dress of a hull (`collision_redress`): every
    /// triangle whose surface material — `sf.materials[t.surface_index]`, an
    /// external `.Material.Gbx` resolved through `resolve` — matches a row
    /// takes the row's (physics, gameplay); the prefab's
    /// `Effects\Media\Material\CollisionTurbo` deck (Concrete, Turbo 1) under
    /// a Reset modifier becomes `Modifier\Reset\Collision` (Concrete, Reset 8),
    /// `CollisionTurboGreen` under Boost becomes `Modifier\Boost\CollisionGrass`
    /// (Green, ReactorBoost 12) — what the game does to the block. The first
    /// matching row wins (slot-table rows come before folder shadows). None
    /// when no triangle matched.
    fn redress_collision(&mut self, sf: &CPlugSurface, triangles: &[Triangle], resolve: &mut MaterialResolver) -> Option<Vec<Triangle>> {
        if self.collision_redress.is_empty() {
            return None;
        }
        let mut by_index: Vec<Option<(String, String, (u8, u8))>> = Vec::new();
        for sm in &sf.materials {
            let hit = match sm {
                super::surface::SurfMaterial::Node(nr) if nr.inline.is_none() && nr.index >= 0 => resolve(nr.index).and_then(|(path, _, _)| self.collision_redress.iter().find(|r| r.matches_path(&path)).map(|r| (path.clone(), r.link.clone(), r.ids))),
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
                t.gameplay = ids.1;
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

/// One row of a modifier's hull re-dress: the prefab material it replaces,
/// the replacement link and that material's (physics, gameplay).
#[derive(Clone, Debug, PartialEq)]
pub struct Redress {
    pub matches: RedressKey,
    pub link: String,
    pub ids: (u8, u8),
}

/// How a re-dress row names the material it replaces.
#[derive(Clone, Debug, PartialEq)]
pub enum RedressKey {
    /// The modifier GameSkin's slot table: the full default material path,
    /// lower-cased (`effects\media\material\collisionturbo.material.gbx`).
    Path(String),
    /// A folder shadow — the mechanism the platform surface modifiers use
    /// (PlatformDirt / Grass / Ice / Snow / Plastic…): no GameSkin, the folder
    /// simply carries a file of the same name as the base material
    /// (`Modifier\PlatformIce\PlatformTech.Material.Gbx` stands in for
    /// `Material\PlatformTech.Material.Gbx`), so the row names the material
    /// FILE, lower-cased (`platformtech.material.gbx`).
    File(String),
}

impl Redress {
    /// Whether the row replaces the material at `path` (a prefab's external
    /// `.Material.Gbx`).
    pub fn matches_path(&self, path: &str) -> bool {
        let low = path.to_ascii_lowercase();
        match &self.matches {
            RedressKey::Path(p) => *p == low,
            RedressKey::File(f) => low.rsplit('\\').next().unwrap_or(&low) == f,
        }
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
                Some(self.link_slot(&link, phys))
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
        // A `--lod-pick` keeps one level only, no ladder.
        let lod0_only = (lod0_only() && !self.all_lods) || self.one_level.is_some();
        let part_levels = if lod0_only { 1 } else { lod_levels_of(&s2.lod_max_dist, &s2.shaded_geoms) };
        let mut part_ladder: Vec<f32> = Vec::new();
        if part_levels > 1 {
            let before = self.lod_max_dist.clone();
            // a ladder shorter than its masks (a mask bit past the last
            // distance): the extra levels take the last distance doubled
            //
            // The switch distances scale with the geometry (see `LodPick`: the
            // box A/B of 2026-09-08 found ×0.5 and ×1.0 indistinguishable on 12
            // of 13 views — the hollow platforms were never the ladder). A part
            // with `ladder_scale` set (the tween cloth under
            // TINY_FLAG_LADDER=pack) keeps its own factor instead.
            let dist_scale = self.ladder_scale.unwrap_or(scale);
            let mut dists: Vec<f32> = s2.lod_max_dist.iter().map(|d| d * dist_scale).collect();
            while (dists.len() as u32) + 1 < part_levels {
                let last = dists.last().copied().unwrap_or(32.0 * dist_scale);
                dists.push(last * 2.0);
            }
            merge_lod_ladder(&mut part_ladder, &dists);
            merge_lod_ladder(&mut self.lod_max_dist, &dists);
            if self.lod_max_dist != before {
                self.notes.push(format!("lod ladder: {} levels, distances {:?} (x{dist_scale}); item ladder now {:?}", part_levels, s2.lod_max_dist, self.lod_max_dist));
            }
        }
        // Which geoms are coarser levels only (no bit 0). Kept with their
        // masks unless one level is picked.
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
                        slots.push(Some(self.link_slot(&link, phys)));
                        continue;
                    }
                }
                slots.push(match m.inst() {
                    Some(inst) => Some(self.material_inst_slot(inst, &s2.materials_folder)),
                    None if !m.name.is_empty() => Some(self.link_slot(&m.name, 0)),
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
                        slots.push(Some(self.link_slot(&link, phys)));
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
                    Some((link, phys)) => Some(self.link_slot(&link, phys)),
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
        if crate::debug::on("decls") {
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
        if !smap.is_empty() && !self.no_split {
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
                        let slot = self.link_slot(link, *phys);
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
            // `--lod-pick 0` restores that bake.
            if lod0_only {
                // the one level kept: the pick's level when the part has it and
                // its nearest level is heavy enough (`LodPick::min_verts`), else
                // the nearest
                let level0_verts: i32 = s2.shaded_geoms.iter().filter(|h| h.lod_mask == 0 || h.lod_mask & 1 != 0).filter_map(|h| s2.visuals.get(h.visual_index as usize).and_then(|r| r.inline.as_deref())).filter_map(|n| if let Node::Visual(v) = n { v.main.as_ref().map(|m| m.count) } else { None }).sum();
                let pick = match self.one_level {
                    Some(n) => n,
                    None => lod_pick().filter(|p| level0_verts >= p.min_verts).map(|p| p.level).filter(|p| s2.shaded_geoms.iter().any(|h| h.lod_mask & (1 << p) != 0)).unwrap_or(0),
                };
                let keep = g.lod_mask == 0 || g.lod_mask & (1 << pick) != 0;
                if !keep {
                    self.notes.push(format!("visual {} (lod mask {}) skipped: not level {pick} (--lod-pick)", g.visual_index, g.lod_mask));
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
            if !is_voted && coarser(g) && is_id_pass(self, mat) && !layer_table.is_empty() && !self.no_split {
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
            // (A gameplay gate's sign panels — the row along the arch on
            // `SpecialSign<Kind>` / the kind folder's `Sign`, the beam square on
            // `SpecialSignOff` / `SignOff` — are kept as geometry: their LED
            // shader shows a display the live gate feeds, so what they SHOW is
            // fixed at the material (signlogo.rs / `sign_logo_material`), not by
            // dropping one of them. Checked in the data first: the two panels
            // are ordinary geoms of the same Solid2 with identical records
            // (`--debug decls` on Special24m.Prefab.Gbx: u01 -1, same lod
            // mask, u02 0; no u10/u12/u19 tables, flags 0) — no on/off flag.)
            let mut v = vis.clone();
            // A visual read in the pack's inline-vertex form is written back
            // that way only while it keeps its frame table (the flag cloth with
            // its 86 tween frames); a frame-less copy takes the stream form
            // every other item visual has.
            if v.sub_visuals.is_empty() {
                v.inline_form = false;
            }
            // `--debug decls`: one note per visual with its vertex
            // declarations and the distinct values of every one-word element
            // (colour / int32 ids), for reading a shader's per-vertex inputs.
            if crate::debug::on("decls") {
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
            // staying with the voted majority.
            if is_voted && !smap.is_empty() && !self.no_split {
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
                        smap.get(&k).map(|(link, phys)| self.link_slot(link, *phys))
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
                    Some(eff) if !layer_table.is_empty() => {
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
                let tris: Vec<Triangle> = idx.chunks(3).filter(|c| c.len() == 3).map(|c| Triangle { indices: [c[0], c[1], c[2]], material_id: phys, gameplay: 0, surface_index: 0 }).collect();
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

