//! The GBX node graph, and the classes on the path from a block or an item to
//! its geometry.
//!
//! A GBX body is a graph, not a stream of records: a *node reference* is an
//! index, and the first reference to an index carries the node's whole body
//! inline. So the only way to reach node 40 is to have parsed nodes 1..39
//! byte-exactly. There is no seeking and no skipping — which is why a reader
//! for one class is really a reader for every class that class can reach.
//!
//! That has one consequence worth stating plainly, because it decides what
//! this file must contain: **an unknown non-skippable chunk is fatal, not
//! skippable.** A chunk whose length is not written down cannot be stepped
//! over, and guessing its length desynchronises the walk into somebody else's
//! floats. When this reader meets one it says which class and which chunk, and
//! that sentence is a task, not a verdict about the data.
//!
//! Chunk layouts follow `gbx-py`'s `src/gbx_structs.py`
//! (github.com/schadocalex/gbx-py), the community's transcription of the
//! format; where this file departs from it the comment says why.

use crate::reader::{Reader, R};
use std::collections::HashMap;

pub const C_SURFACE: u32 = 0x0900C000;
pub const C_VISUAL_INDEXED_TRIANGLES: u32 = 0x0901E000;
pub const C_VERTEX_STREAM: u32 = 0x09056000;
pub const C_INDEX_BUFFER: u32 = 0x09057000;
pub const C_SOLID2MODEL: u32 = 0x090BB000;
pub const C_PREFAB: u32 = 0x09145000;
pub const C_STATIC_OBJECT: u32 = 0x09159000;
pub const C_MATERIAL_USER_INST: u32 = 0x090FD000;
pub const C_MATERIAL: u32 = 0x09079000;
pub const C_ITEM_MODEL: u32 = 0x2E002000;
pub const C_VARIANT_LIST: u32 = 0x2F0BC000;
pub const C_BLOCK_ITEM: u32 = 0x2E025000;
pub const C_CRYSTAL: u32 = 0x09003000;
pub const C_COMMON_ITEM_ENTITY_MODEL: u32 = 0x2E027000;
pub const C_DYNA_OBJECT: u32 = 0x09144000;
/// `CPlugSolid`: a tree of `CPlugTree` nodes (the decoration Scene3d's island,
/// a block variant's trigger solid); read as a redirection to its tree.
pub const C_SOLID: u32 = 0x09005000;
pub const C_TREE: u32 = 0x0904F000;

/// Classes whose node body is a single struct with no chunk framing.
fn no_body_chunks(class_id: u32) -> bool {
    matches!(
        class_id,
        0x0912F000
            | 0x09144000
            | 0x09178000
            | C_STATIC_OBJECT

            | 0x0917B000
            | 0x09179000
            | 0x09187000
            | C_PREFAB
            | 0x2F074000
            | 0x2F0BC000
            | 0x2F086000
            | 0x2F0CA000
            | 0x0902F000
    )
}

// ---------------------------------------------------------------- node kinds

#[derive(Clone, Debug)]
pub struct PrefabEnt {
    pub model: i32,
    /// Rotation as (x, y, z, w).
    pub rot: [f32; 4],
    pub pos: [f32; 3],
}

#[derive(Clone, Debug, Default)]
pub struct Prefab {
    pub ents: Vec<PrefabEnt>,
}

/// `CPlugDynaObjectModel`: a block that MOVES -- a rotor, a turnstile, a
/// tube, a flag. It carries up to three shapes, and which of them the car can
/// be standing on is a question the geometry alone cannot answer; see
/// `geom::Collector`.
#[derive(Clone, Debug)]
pub struct DynaObject {
    pub mesh: i32,
    /// the hull that moves (a rotor calls it `MoveShape`)
    pub dyna_shape: i32,
    /// the hull that does not (a rotor calls it `HitShape`)
    pub static_shape: i32,
}

#[derive(Clone, Debug)]
pub struct StaticObject {
    pub mesh: i32,
    pub mesh_collidable: bool,
    pub shape: i32,
}

/// One `CPlugSurface` leaf: a triangle soup with a physics material per face.
#[derive(Clone, Debug, Default)]
pub struct SurfMesh {
    pub verts: Vec<[f32; 3]>,
    /// (a, b, c, physics id, gameplay id)
    pub tris: Vec<([i32; 3], u8, u8)>,
}

/// A `CPlugSurface`'s shape tree, flattened to meshes in the surface's own
/// frame. A `Compound` places its children by an Iso4, which is applied here
/// rather than carried, because nothing downstream wants the tree.
#[derive(Clone, Debug, Default)]
pub struct Surface {
    pub meshes: Vec<SurfMesh>,
    /// Material nodes the surface names (one per material slot); for BlueBay
    /// terrain this is where the real look material (`Land.Material.Gbx`)
    /// is referenced.
    pub materials: Vec<i32>,
    /// Shape types met that are not triangle meshes — spheres, boxes,
    /// cylinders. Reported rather than dropped: a block whose collision is a
    /// primitive is a real answer, not a failure.
    pub primitives: Vec<i32>,
    /// The GameplayMainDir of the outermost shape (surf v2+): the axis an
    /// oriented gameplay gate pushes along.
    pub main_dir: Option<[f32; 3]>,
}

#[derive(Clone, Debug, Default)]
pub struct VertexStream {
    pub positions: Vec<[f32; 3]>,
    pub normals: Vec<[f32; 3]>,
    pub uv0: Vec<[f32; 2]>,
    /// The normals as STORED when the stream packs them (Dec3N words, one per
    /// vertex; empty for float3 normals) — what `envblock` re-encodes for the
    /// GPU without a lossy float round trip.
    pub normals_dec3n: Vec<u32>,
    /// Every declaration of the stream, as (name, stored type) — `envblock`
    /// reproduces the game's upload layout from it.
    pub decls: Vec<(u32, u32)>,
}

#[derive(Clone, Debug, Default)]
pub struct Visual {
    pub vertex_streams: Vec<i32>,
    /// Positions written inline in `0x0902C004` (older visuals with no stream).
    pub inline_positions: Vec<[f32; 3]>,
    pub inline_normals: Vec<[f32; 3]>,
    pub uv0: Vec<[f32; 2]>,
    pub indices: Vec<u32>,
    pub index_is_absolute: bool,
    pub count: u32,
}

#[derive(Clone, Debug)]
pub struct ShadedGeom {
    pub visual: i32,
    pub material: i32,
    pub lod: i32,
    /// the two unexplained words (the second v32+), printed by `dump` — the
    /// 2026-09-07 hunt for what marks a display geom
    pub u01: i32,
    pub u02: i32,
}

/// One `CPlugCrystal` layer: an editable mesh of n-gon faces, each with a
/// material index into the crystal's own material-name list.
#[derive(Clone, Debug, Default)]
pub struct CrystalMesh {
    pub verts: Vec<[f32; 3]>,
    pub faces: Vec<(Vec<i32>, usize)>,
}

#[derive(Clone, Debug, Default)]
pub struct Crystal {
    /// Per slot: the literal material name, and the node index of the
    /// CPlugMaterialUserInst when the name is empty.
    pub materials: Vec<(String, i32)>,
    pub meshes: Vec<CrystalMesh>,
}

#[derive(Clone, Debug, Default)]
pub struct Solid2 {
    pub geoms: Vec<ShadedGeom>,
    /// Node index per visual slot, as referenced by `ShadedGeom::visual`.
    pub visuals: Vec<i32>,
    pub material_names: Vec<String>,
    /// Material NODES, in the same index space as `material_names`: a
    /// CPlugMaterialUserInst per slot when the names are empty.
    pub material_nodes: Vec<i32>,
    /// The light sockets: (name, CPlugLight node or -1, Iso4 in model space).
    pub lights: Vec<(String, i32, [f32; 12])>,
    /// Item-editor lights: the `light_user_models` nodes and the
    /// `light_insts` (model index, socket index) pairs.
    pub light_user_models: Vec<i32>,
    pub light_insts: Vec<(u32, u32)>,
    /// The detail-level switch distances (`LodDistances`, v1+): level k of a
    /// geom's `lod` bitmask draws while the camera is within `lod_max_dist[k]`.
    pub lod_max_dist: Vec<f32>,
    /// `VisCstType` (v2+).
    pub vis_cst_type: u32,
}

/// A `CPlugLight` (0x0901D000) — the wrapper a Solid2's `lights` socket names
/// — or the `GxLight*` node inside it (0x0400B000 spot, 0x04002000 ball,
/// 0x0400A000 frustum, 0x04007000 directional, 0x04005000 ambient), as the
/// `.Light.Gbx` files of the packs carry them. Layouts: GBX.NET
/// `CPlugLight.chunkl` / `GxLight*.chunkl`, read off
/// `Stadium\Media\Light\ItemLampSpot.Light.Gbx` (2026-09-07).
#[derive(Clone, Debug, Default)]
pub struct LightInfo {
    /// The wrapper's GxLight node (chunk 0x0901D000/004), -1 on a GxLight.
    pub gx_node: i32,
    /// Chunk 0x0901D003: the animation image node, its period range.
    pub image_anim: i32,
    pub anim_period: [f32; 2],
    /// CPlugLight flags (chunk 0x0901D002's word, or the fourth int of 0x0901D004): 1 NightOnly, 2 ReflectByGround, 4
    /// DuplicateGxLight, 8 SceneLightOnlyWhenTreeVisible, 16 SceneLightAlwaysActive. A file with neither chunk keeps the
    /// game's constructor default 1 (NightOnly) — this field then reads 0; `static_item::light::CPlugLight::flags()` applies
    /// the default.
    pub flags: u32,
    /// The five ints of chunk 0x0901D004 (a noderef among them?).
    pub tail: [i32; 5],
    // --- GxLight (0x0400100A)
    pub color: [f32; 3],
    /// 1 DoLighting, 2 LightMapOnly, 4 ShadowGen, 8 Specular, 16 LensFlare,
    /// 32 Sprite, 64.. EnableGroup0-3.
    pub gx_flags: u32,
    pub intensity: f32,
    pub diffuse_intensity: f32,
    pub shadow_intensity: f32,
    pub flare_intensity: f32,
    pub shadow_rgb: [f32; 3],
    // --- GxLightPoint (0x04003004)
    pub flare_size: f32,
    pub flare_bias_z: f32,
    // --- GxLightBall (0x04002008)
    pub ball_flags: u32,
    pub radius: f32,
    pub radius_specular: f32,
    pub radius_shadow: f32,
    pub radius_flare: f32,
    pub emitting_radius: f32,
    pub emitting_cylinder_len_z: f32,
    pub att_htnlr: [f32; 2],
    pub ambient_rgb: [f32; 3],
    pub att_hyper2: [f32; 2],
    /// 0x04002009 / 0x0400200A, one float each.
    pub ball_u09: f32,
    pub ball_u0a: f32,
    // --- GxLightSpot (0x0400B003)
    pub spot_flags: u32,
    pub angle_inner: f32,
    pub angle_outer: f32,
    pub angle_flare: f32,
    pub angle_inner_shadow: f32,
    pub angle_outer_shadow: f32,
    pub falloff_exponent: f32,
    pub spot_bytes: [u8; 2],
}


/// One layer of a `CPlugImageArray` (0x0914C000, chunk 0x0914C000 v7; reader 0x1404d4b00): the
/// terrain "layer" of a texture array — the world-position → texture-coordinate mapping the
/// terrain shader (`Tech3/Block_PyPxz_ids_p`) reads per slice from `g_WorldPosToTcPyPxz` /
/// `g_WorldPosToTcPyX2` / `g_WorldPosToTcPyH2` (the buffer is built from these fields by
/// 0x1409f2e50 + 0x1404d49d0, see `terrain::world_pos_to_tc`). Stride 0x50 in memory.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ImageArrayLayer {
    /// +0: the layer name (`Land`, `SeaFloor`, …) — what a material's chunk 0x0903A015 names
    /// select (`SubIndexPyPxz`), and the stem of the slice image `<folder><name><suffix>.dds`.
    pub name: String,
    /// +0x10/+0x14: the Py (top) projection's texture size in metres (x, y).
    pub py_scale: [f32; 2],
    /// +0x18/+0x1c: the Py projection's offsets (the `.w` of the two uv rows, scaled by 1/size).
    pub py_offset: [f32; 2],
    /// +0x20/+0x24: the Pxz (side) projection's texture size in metres (x, y); version < 3 files
    /// copy the Py values.
    pub pxz_scale: [f32; 2],
    /// +0x28/+0x2c: the Pxz projection's offsets — only `.y` reaches the buffer (`[2].z = offset.y / scale.y`).
    pub pxz_offset: [f32; 2],
    /// +0x30: the Py projection's rotation in degrees.
    pub rotation_deg: f32,
    /// +0x34/+0x38: the side blend's start / end angles in degrees (`[3].y = cos(start)`, `[3].x = cos(end)`);
    /// version 0 files default to 40 / 50.
    pub blend_pxz_deg: [f32; 2],
    /// +0x3c/+0x40: the top blend's start / end angles in degrees (`[3].w = cos(start)`, `[3].z = cos(end)`).
    pub blend_py_deg: [f32; 2],
    /// +0x44/+0x48: two ids (version < 2 files: both = the layer index); the SECOND one is what
    /// the buffer carries in `[2].w` (as raw bits).
    pub ids: [u32; 2],
}

/// `CPlugImageArray` (0x0914C000): the layer table of a terrain texture array.
#[derive(Clone, Debug, Default)]
pub struct ImageArrayRaw {
    pub version: u32,
    /// +0x28: the folder the slice images live in (`BlueBay\Media\Texture\Image\`).
    pub folder: String,
    pub layers: Vec<ImageArrayLayer>,
    /// v ≥ 4: a node reference (−1 in every shipped file).
    pub node_ref: i32,
    /// v ≥ 5: +0x40 (1.0 in every shipped file).
    pub f_v5: f32,
    /// v ≥ 6: +0x30, a second folder (`…\Texture\Decal\`).
    pub decal_folder: String,
    /// v ≥ 7: +0x48.
    pub u_v7: u32,
}

/// `CPlugBitmap` (0x09011000) — the fields of a texture / texture-array file this reader keeps.
#[derive(Clone, Debug, Default)]
pub struct BitmapRaw {
    /// 0x09011025 (reader 0x1403f7550 case 0x9011025): {Vec2 scale (u, v) → bitmap+0x30, Vec2 trans
    /// (u, v) → +0x38, f32 `DefaultTexCoordRotate` in DEGREES → +0x40 (CPlugBitmap member 0x0901101D,
    /// range 0..360), u32 colour → +0x1c} — the `GbxSamplerTcScaleTrans_<Map>` /
    /// `GbxWorldPosToTexCoord_<Map>` transform of a projected texture (TrackWallPxzInWorld_D: 1/32,
    /// 1/32, 0, 0, 0°, 0xff000000; BlueBay WarpSand_D: 0.0005, 0.0005, 0, 0, 15°, 0xff000000 — the
    /// 15° the capture's WarpSand draws carry; `envblock::world_pos_to_texcoord`).
    pub tc_scale_trans: Option<[u32; 6]>,
    /// 0x09011030: the image node (a `CPlugFileGen` for a generated texture array, else the
    /// external `.dds` reference).
    pub image: i32,
    /// 0x09011034: {v4, ref ImageArray, string suffix, refs[] slices, ref, 8 bytes} — for a texture
    /// ARRAY the ImageArray node and the slice images IN GPU SLICE ORDER (each uploaded vertically
    /// flipped, see `terrain`).
    pub array_image_array: i32,
    pub array_suffix: String,
    pub array_slices: Vec<i32>,
}

/// `CPlugFileGen` (0x0902F000; archive 0x1404179a0 read / 0x140417c20 write, no chunk framing):
/// a GENERATED image — the texture array of a terrain material (kind 0x1d: u32s {w, h, slices,
/// 1, 3, mips, 1, 1, 0}) or a 1-D lookup table such as `WaterTransmittance.ImageGen.Gbx`
/// (kind 0x33: u32s {2048, 1}, one float4, six f32 parameters).
#[derive(Clone, Debug, Default)]
pub struct FileGenRaw {
    pub version: u32,
    pub kind: u32,
    pub u32s: Vec<u32>,
    pub float4s: Vec<[f32; 4]>,
    pub f32s: Vec<f32>,
    pub name: String,
}

/// `CPlugMaterialCustom` (0x0903A000) — the fields of a material's custom block this reader keeps.
#[derive(Clone, Debug, Default)]
pub struct MaterialCustomRaw {
    /// 0x0903A013 (and 0x0903A006): the texture slots as (slot name, node reference) — `BaseColor`,
    /// `PyX2`, `PyH2`, `PyBaseColor`, `PxzBaseColor`, …
    pub bitmaps: Vec<(String, i32)>,
    /// 0x0903A015: the u32 that precedes the names (0 = names follow; ≠ 0 = none).
    pub layer_mode: i32,
    /// 0x0903A015: the terrain layer names in FILE order = (Pxz, Py, X2, H2) — the material's
    /// +0xe8 / +0xd8 / +0xf8 / +0x108 (reader 0x1404415c0); an empty name = no layer (−1).
    pub layer_names: [String; 4],
    /// 0x0903A00A: the GpuFx parameters (the two lists concatenated), as (name, floats) — a
    /// material's constant overrides (`PxzScaleTrans` = (0.0005, 0.0005, 0.5) on WarpSand).
    pub params: Vec<(String, Vec<f32>)>,
    /// 0x0903A00A: the GpuFx parameter names of the two lists (custom+0x70 and custom+0x80 at runtime,
    /// 0x30-byte entries with the name Id at +0x18). The material's runtime flags come from them (RE 7,
    /// 2026-09-25, CPlugMaterial finaliser 0x14040ee40 l.~40–70): +0xf0 bit 0 "takes the placement colour" ⟺
    /// list 2 has `BaseColorTargetId` or list 1 has `BaseColorTarget` / `BaseColorTarget_sRGB`; bit 1 "colour 2"
    /// ⟺ list 2 has `DeactivableDisplayId`. (The Solid2Model's +0x1f0 bits 1/2 = any material with those bits; the
    /// item clone key then carries the placement colour.)
    pub gpufx_names: [Vec<String>; 2],
}

#[cfg(test)]
mod colour_flag_tests {
    use super::MaterialCustomRaw;
    #[test]
    fn stadium_on_terrain_materials() {
        // WhiteShore\Media\Modifier\StadiumOnTerrain\* as the pack has them (2026-09-25)
        let mk = |l1: &[&str], l2: &[&str]| MaterialCustomRaw { gpufx_names: [l1.iter().map(|s| s.to_string()).collect(), l2.iter().map(|s| s.to_string()).collect()], ..Default::default() };
        assert_eq!(mk(&["BaseColorTarget", "BaseColorTarget_SI"], &["TcScale_BRNH", "Meters_Depth"]).colour_flags(), 1); // TrackBordersInWorld
        assert_eq!(mk(&["BaseColorTarget"], &[]).colour_flags(), 1); // TrackWallClipsInWorld, StructureInWorld
        assert_eq!(mk(&["BaseColorTarget_sRGB"], &[]).colour_flags(), 1); // DecalPaintSponsor4x1D
        assert_eq!(mk(&[], &[]).colour_flags(), 0); // Deco, DecoHill, TrackWallInWorld
        assert_eq!(mk(&["TargetColor"], &[]).colour_flags(), 0); // CustomMetalPainted: a different parameter
        assert_eq!(mk(&[], &["BaseColorTargetId", "DeactivableDisplayId"]).colour_flags(), 3);
    }
}

impl MaterialCustomRaw {
    /// CPlugMaterial+0xf0 bits 0 and 1 as the finaliser derives them (see `gpufx_names`).
    pub fn colour_flags(&self) -> u32 {
        let has = |list: usize, n: &str| self.gpufx_names[list].iter().any(|x| x == n);
        let mut f = 0u32;
        if has(1, "BaseColorTargetId") || has(0, "BaseColorTarget") || has(0, "BaseColorTarget_sRGB") {
            f |= 1;
        }
        if has(1, "DeactivableDisplayId") {
            f |= 2;
        }
        f
    }
}

#[derive(Clone, Debug)]
pub enum Node {
    Prefab(Prefab),
    StaticObject(StaticObject),
    Dyna(DynaObject),
    Surface(Surface),
    Solid2(Solid2),
    Visual(Visual),
    VertexStream(VertexStream),
    /// A class this reader walks but keeps nothing from.
    /// A `CGameItemModel` or `CGameCommonItemEntityModel`: a redirection to
    /// the node that actually holds the shape.
    Crystal(Crystal),
    /// A material: its name, and the physics id the car feels through it.
    Material(String, u8),
    ItemModel(i32),
    /// `CGameCtnBlockInfo*`: a block model (or a clip), see `blockinfo.rs`.
    BlockInfo(Box<crate::blockinfo::BlockInfoRaw>),
    /// `CGameCtnBlockInfoVariant{Ground,Air}`.
    Variant(Box<crate::blockinfo::VariantRaw>),
    /// `CGameCtnBlockUnitInfo`: one cell of a variant and its clips.
    BlockUnit(Box<crate::blockinfo::BlockUnitRaw>),
    /// `CGameCtnBlockInfoMobil`: one drawn prefab/solid of a variant.
    Mobil(Box<crate::blockinfo::MobilRaw>),
    AutoTerrain(crate::blockinfo::AutoTerrainRaw),
    Genealogy(crate::blockinfo::GenealogyRaw),
    /// `CPlugRoadChunk` / `CPlugPlacementPatch`.
    RoadChunk(Box<crate::blockinfo::RoadChunkRaw>),
    /// `CPlugLight` or a `GxLight*` (the class id says which).
    Light(u32, Box<LightInfo>),
    /// `CPlugTree`: one node of a `CPlugSolid`'s tree — its children, its
    /// visual and the material (shader) it is drawn with, its local transform.
    Tree(Box<Tree>),
    /// `CSceneLayout` chunk 0x0A00301C: the decoration's light rig and solids.
    Layout(Box<Layout>),
    /// `CPlugImageArray`: a terrain texture array's layer table.
    ImageArray(Box<ImageArrayRaw>),
    /// `CPlugBitmap`: the texture-array / projection fields of a texture file.
    Bitmap(Box<BitmapRaw>),
    /// `CPlugFileGen`: a generated image's parameters.
    FileGen(Box<FileGenRaw>),
    /// `CPlugMaterialCustom`: the texture slots and the terrain layer names.
    MaterialCustom(Box<MaterialCustomRaw>),
    Other(u32),
}

/// A `CPlugTree` (`0x0904F000`) node: what the decoration Scene3d solids
/// and block trigger solids are built from (`scene3d.rs`).
#[derive(Clone, Debug, Default)]
pub struct Tree {
    pub name: String,
    pub children: Vec<i32>,
    /// `0x0904F016`: Visual, Shader (the material), Surface, Generator.
    pub visual: i32,
    pub shader: i32,
    pub surface: i32,
    /// `0x0904F01A`: flags, and the Iso4 (3×3 rotation, translation) when
    /// bit 2 is set.
    pub flags: u32,
    pub transform: Option<[f32; 12]>,
}

/// `CSceneLayout` (0x0A003000) chunk `0x0A00301C` — the decoration Scene3d:
/// its light rig and the solids placed in the world. Layout from the
/// reader `CSceneLayout::ArchiveChunk` 0x1407efc20 (case 0x1c at
/// 0x1407f0554, versions ≥ 3; v5 on the current packs):
/// `u32 version; lights[]: { Id name; Vec3 pos; Quat xyzw; u32 v; ref[3]
/// CPlugBitmap; ref GxLight; u32 }; mobils[]: { Id name; Vec3 pos; Quat
/// xyzw; u16; u64 flags; ref CPlugSolid; v≥4 ref 0x090BB000; v≥5 ref
/// CPlugPrefab }; ref 0x0A040000` (DISASSEMBLY 2026-09-23; leaf readers
/// 0x141462f40 + 0x140194a20 = pos then quat, 0x1407ef120 = the light
/// record, 0x140155240 / 0x1404b8570 / 0x1407f0d90 = the typed refs).
#[derive(Clone, Debug, Default)]
pub struct Layout {
    pub version: u32,
    pub lights: Vec<LayoutLight>,
    pub mobils: Vec<LayoutMobil>,
    /// The `0x0A040000` reference after the mobils.
    pub extra: i32,
    /// v ≥ 2: the weather node (`DayTime.MotionManagerWeathers.Gbx`).
    pub weather: i32,
    /// v ≥ 2: three CPlugBitmap refs (the third is the environment cube).
    pub env_bitmaps: [i32; 3],
    /// v ≥ 2: an 11-float block (0x141406680) — meaning not pinned.
    pub params: [f32; 11],
    pub u03: [u32; 4],
    /// v ≥ 2: a `0x0A03A000` reference.
    pub u04: i32,
}

#[derive(Clone, Debug, Default)]
pub struct LayoutLight {
    pub name: String,
    pub pos: [f32; 3],
    /// x, y, z, w
    pub rot: [f32; 4],
    pub version: u32,
    pub bitmaps: [i32; 3],
    /// The GxLight node (inline `GxLightAmbient` / `GxLightDirectional`).
    pub light: i32,
    pub u01: u32,
}

#[derive(Clone, Debug, Default)]
pub struct LayoutMobil {
    pub name: String,
    pub pos: [f32; 3],
    pub rot: [f32; 4],
    /// A u16 (`Read2` 0x14012c330): 0x401 on the sky dome, 1 on the solids.
    pub u01: u16,
    pub flags: u64,
    /// The `CPlugSolid` (inline for BlueBay, an external `.Solid.Gbx` for
    /// the other collections).
    pub solid: i32,
    pub u02: i32,
    pub prefab: i32,
}

impl Node {
    pub fn class_id(&self) -> u32 {
        match self {
            Node::Prefab(_) => C_PREFAB,
            Node::StaticObject(_) => C_STATIC_OBJECT,
            Node::Dyna(_) => C_DYNA_OBJECT,
            Node::Surface(_) => C_SURFACE,
            Node::Solid2(_) => C_SOLID2MODEL,
            Node::Visual(_) => C_VISUAL_INDEXED_TRIANGLES,
            Node::VertexStream(_) => C_VERTEX_STREAM,
            Node::Crystal(_) => C_CRYSTAL,
            Node::Material(..) => C_MATERIAL_USER_INST,
            Node::ItemModel(_) => C_ITEM_MODEL,
            Node::BlockInfo(_) => crate::blockinfo::C_BLOCK_INFO,
            Node::Variant(_) => crate::blockinfo::C_VARIANT,
            Node::BlockUnit(_) => crate::blockinfo::C_BLOCK_UNIT,
            Node::Mobil(_) => crate::blockinfo::C_MOBIL,
            Node::AutoTerrain(_) => crate::blockinfo::C_AUTO_TERRAIN,
            Node::Genealogy(_) => crate::blockinfo::C_ZONE_GENEALOGY,
            Node::RoadChunk(_) => crate::blockinfo::C_ROAD_CHUNK,
            Node::Light(c, _) => *c,
            Node::Tree(_) => 0x0904F000,
            Node::Layout(_) => 0x0A003000,
            Node::ImageArray(_) => 0x0914C000,
            Node::Bitmap(_) => 0x09011000,
            Node::FileGen(_) => 0x0902F000,
            Node::MaterialCustom(_) => 0x0903A000,
            Node::Other(c) => *c,
        }
    }
}

#[derive(Clone, Debug)]
pub enum Slot {
    Unset,
    /// Reserved while its body is being read; a reference back to it here
    /// would be a cycle, which this format does not have.
    Reading,
    /// An entry of the reference table: another file, by name.
    External(String),
    Node(Node),
}

// ------------------------------------------------------------------ the walk

pub struct Graph<'a> {
    pub r: Reader<'a>,
    pub slots: Vec<Slot>,
    pub root: Option<Node>,
    /// Counts of chunks walked, by (class, chunk). Diagnostics only.
    pub seen: HashMap<(u32, u32), u32>,
    /// Places where a layout this reader does not know forced a scan to the
    /// node terminator. Never silent: whatever was in the node is missing.
    pub recovered: Vec<String>,
    /// Every node reference word read, as (body offset, index): what a
    /// renumbering of the node table has to rewrite. Filled by `noderef`.
    pub noderef_sites: Vec<(usize, i32)>,
    /// Unknown skippable chunks stepped over, as (chunk id, size). What a
    /// "complete" reader still does not read.
    pub skipped: Vec<(u32, u32)>,
    /// The collector name (chunk 0x2E00100C) of the root node, when it has one.
    pub collector_name: String,
    /// One block-info accumulator per node body being read, innermost last.
    pub bi_stack: Vec<crate::blockinfo::BiAcc>,
    /// Where every chunked node body began, as (body offset, class id), in
    /// read order: the points where the game "dummy-writes" the node's parent
    /// class id into a pak file's cipher (`parents.rs`, `pakfile.rs`).
    /// A node that begins INSIDE a skippable chunk is listed in
    /// `node_starts_skipped` instead: the game reads such a chunk whole into
    /// a memory buffer before parsing it, and a dummy write into a memory
    /// buffer is a no-op (`CMwNod::Archive` 0x1402d0720, memory buffer Write
    /// 0x140123c80), so those nodes never touch the cipher.
    pub node_starts: Vec<(usize, u32)>,
    pub node_starts_skipped: Vec<(usize, u32)>,
    /// How many skippable chunks the walk is currently inside.
    skip_depth: u32,
}

const FACADE: u32 = 0xFACADE01;
const SKIP: &[u8; 4] = b"PIKS";

impl<'a> Graph<'a> {
    pub fn new(body: &'a [u8], num_nodes: u32, externals: &[(u32, String)]) -> Graph<'a> {
        let mut slots = vec![Slot::Unset; (num_nodes as usize).max(1) + 1];
        for (i, name) in externals {
            let i = *i as usize;
            if i < slots.len() {
                slots[i] = Slot::External(name.clone());
            }
        }
        Graph { r: Reader::new(body), slots, root: None, seen: HashMap::new(), recovered: Vec::new(), noderef_sites: Vec::new(), skipped: Vec::new(), collector_name: String::new(), bi_stack: Vec::new(), node_starts: Vec::new(), node_starts_skipped: Vec::new(), skip_depth: 0 }
    }

    /// Parse a whole file body, rooted at `class_id`.
    pub fn parse(body: &'a [u8], class_id: u32, num_nodes: u32, externals: &[(u32, String)]) -> R<Graph<'a>> {
        let mut g = Graph::new(body, num_nodes, externals);
        let root = g.node_body(class_id)?;
        g.root = Some(root);
        Ok(g)
    }

    pub fn node(&self, idx: i32) -> Option<&Node> {
        match self.slots.get(idx.max(0) as usize) {
            Some(Slot::Node(n)) => Some(n),
            _ => None,
        }
    }
    pub fn external(&self, idx: i32) -> Option<&str> {
        match self.slots.get(idx.max(0) as usize) {
            Some(Slot::External(s)) => Some(s.as_str()),
            _ => None,
        }
    }

    /// Read a node reference. Returns the node index, or -1 for null.
    /// An inline node whose `[i32 index][u32 class id][body]` sits at body
    /// offset `off` — the way into a file whose outer class has no reader yet
    /// (`scene3d.rs` walks the CPlugSolid subtrees of a CSceneLayout this way).
    pub fn node_at_offset(&mut self, off: usize) -> R<i32> {
        self.r.o = off;
        self.r.mid_body = off > 0;
        self.noderef()
    }

    pub fn noderef(&mut self) -> R<i32> {
        let at = self.r.o;
        let idx = self.r.i32()?;
        self.noderef_sites.push((at, idx));
        if idx <= 0 {
            return Ok(-1);
        }
        let u = idx as usize;
        if u >= self.slots.len() {
            // The node table is sized from the header. An index past it means
            // the walk is off the rails, not that the file has more nodes.
            return Err(format!("node ref {} past the {} declared nodes", idx, self.slots.len() - 1));
        }
        if matches!(self.slots[u], Slot::Unset) {
            let class_id = self.r.u32()?;
            if class_id == 0xFFFF_FFFF {
                self.slots[u] = Slot::Node(Node::Other(class_id));
                return Ok(idx);
            }
            self.slots[u] = Slot::Reading;
            let n = self
                .node_body(class_id)
                .map_err(|e| format!("node {} (class 0x{:08X}): {}", idx, class_id, e))?;
            self.slots[u] = Slot::Node(n);
        }
        Ok(idx)
    }

    /// A node's body: either one struct (`no_body_chunks`) or a chunk loop.
    pub fn node_body(&mut self, class_id: u32) -> R<Node> {
        crate::reader::trace(|| format!("node class 0x{:08X} at 0x{:x}", class_id, self.r.o));
        if no_body_chunks(class_id) {
            return self.plain_body(class_id);
        }
        if self.skip_depth == 0 {
            self.node_starts.push((self.r.o, class_id));
        } else {
            self.node_starts_skipped.push((self.r.o, class_id));
        }
        let mut acc = Acc::new(class_id);
        self.bi_stack.push(crate::blockinfo::BiAcc::default());
        let walked = self.node_chunks(class_id, &mut acc);
        let bi = self.bi_stack.pop().unwrap_or_default();
        walked?;
        Ok(acc.finish(class_id, bi))
    }

    /// The chunk loop of `node_body`, split out so the accumulator stack is
    /// popped on every exit path.
    fn node_chunks(&mut self, class_id: u32, acc: &mut Acc) -> R<()> {
        loop {
            if self.r.eof() {
                // A body that ends without FACADE is legal for the outermost
                // node of some files; inside a node ref it is not, but we
                // cannot tell from here, so accept and let the caller's own
                // structure fail if it was wrong.
                break;
            }
            let cid = self.r.u32()?;
            crate::reader::trace(|| format!("  chunk 0x{:08X} of class 0x{:08X} at 0x{:x}", cid, class_id, self.r.o));
            if cid == FACADE {
                break;
            }
            let skippable = self.r.b.get(self.r.o..self.r.o + 4) == Some(SKIP);
            if skippable {
                self.r.u32()?;
                let size = self.r.u32()? as usize;
                let known = self.chunk_is_known(class_id, cid);
                if known {
                    let end = self.r.o + size;
                    if end > self.r.b.len() {
                        return Err(format!(
                            "skippable chunk 0x{:08X} of {} bytes past end of body",
                            cid, size
                        ));
                    }
                    self.skip_depth += 1;
                    let walked = self.chunk(class_id, cid, acc);
                    self.skip_depth -= 1;
                    walked?;
                    // Trailing bytes inside a skippable chunk are normal (the
                    // game writes more than any one reader consumes); jump to
                    // the declared end rather than trusting our own cursor.
                    self.r.o = end;
                } else {
                    self.skipped.push((cid, size as u32));
                    self.r.take(size)?;
                }
            } else {
                self.chunk(class_id, cid, acc).map_err(|e| {
                    format!("class 0x{:08X} chunk 0x{:08X} at 0x{:x}: {}", class_id, cid, self.r.o, e)
                })?;
            }
            *self.seen.entry((class_id, cid)).or_insert(0) += 1;
        }
        Ok(())
    }

}

/// Accumulates a node's chunks into one value.
pub struct Acc {
    pub class_id: u32,
    pub prefab: Prefab,
    pub statobj: Option<StaticObject>,
    pub surface: Surface,
    pub solid2: Solid2,
    pub visual: Visual,
    pub vstream: VertexStream,
    pub visual_flags: crate::classes::VisualFlags,
    /// The node an item model hands its geometry to (`0x2E002019` or
    /// `0x2E027000`). `-1` when the class carries none.
    pub entity_model: i32,
    pub crystal_materials: Vec<(String, i32)>,
    /// Nodes a CPlugMaterial references (its custom material, shader, ...):
    /// the external `.Material.Gbx` among them is the game material it stands
    /// for. Surfaced as `Node::Material("@refs:a,b,c", phys)`.
    pub material_refs: Vec<i32>,
    pub crystals: Vec<CrystalMesh>,
    pub material_name: String,
    pub physics_id: u8,
    pub light: Option<Box<LightInfo>>,
    pub tree: Option<Box<Tree>>,
    pub layout: Option<Box<Layout>>,
    pub image_array: Option<Box<ImageArrayRaw>>,
    pub bitmap: Option<Box<BitmapRaw>>,
    pub mat_custom: Option<Box<MaterialCustomRaw>>,
    pub touched: bool,
}

impl Acc {
    fn new(class_id: u32) -> Acc {
        Acc {
            class_id,
            prefab: Prefab::default(),
            statobj: None,
            surface: Surface::default(),
            solid2: Solid2::default(),
            visual: Visual::default(),
            vstream: VertexStream::default(),
            visual_flags: crate::classes::VisualFlags::default(),
            entity_model: -1,
            crystal_materials: Vec::new(),
            material_refs: Vec::new(),
            crystals: Vec::new(),
            material_name: String::new(),
            physics_id: 0,
            light: None,
            tree: None,
            layout: None,
            image_array: None,
            bitmap: None,
            mat_custom: None,
            touched: false,
        }
    }
    /// The tree accumulator, created on the first CPlugTree chunk.
    pub fn tree_mut(&mut self) -> &mut Tree {
        self.touched = true;
        self.tree.get_or_insert_with(|| Box::new(Tree { visual: -1, shader: -1, surface: -1, ..Tree::default() }))
    }
    /// The bitmap accumulator, created on the first CPlugBitmap chunk this reader keeps.
    pub fn bitmap_mut(&mut self) -> &mut BitmapRaw {
        self.touched = true;
        self.bitmap.get_or_insert_with(|| Box::new(BitmapRaw { image: -1, array_image_array: -1, ..BitmapRaw::default() }))
    }
    /// The custom-material accumulator.
    pub fn mat_custom_mut(&mut self) -> &mut MaterialCustomRaw {
        self.touched = true;
        self.mat_custom.get_or_insert_with(|| Box::new(MaterialCustomRaw::default()))
    }
    /// The light accumulator, created on the first light chunk.
    pub fn light_mut(&mut self) -> &mut LightInfo {
        self.touched = true;
        self.light.get_or_insert_with(|| Box::new(LightInfo { gx_node: -1, image_anim: -1, ..LightInfo::default() }))
    }
    fn finish(self, class_id: u32, bi: crate::blockinfo::BiAcc) -> Node {
        if let Some(n) = bi.finish(class_id) {
            return n;
        }
        if !self.touched {
            return Node::Other(class_id);
        }
        if let Some(l) = self.light {
            return Node::Light(class_id, l);
        }
        if let Some(t) = self.tree {
            return Node::Tree(t);
        }
        if let Some(l) = self.layout {
            return Node::Layout(l);
        }
        if let Some(a) = self.image_array {
            return Node::ImageArray(a);
        }
        if let Some(b) = self.bitmap {
            return Node::Bitmap(b);
        }
        if let Some(m) = self.mat_custom {
            return Node::MaterialCustom(m);
        }
        match class_id {
            C_SURFACE => Node::Surface(self.surface),
            C_SOLID2MODEL => Node::Solid2(self.solid2),
            C_MATERIAL_USER_INST => Node::Material(self.material_name, self.physics_id),
            C_MATERIAL => Node::Material(
                format!("@refs:{}", self.material_refs.iter().map(|r| r.to_string()).collect::<Vec<_>>().join(",")),
                self.physics_id,
            ),
            C_CRYSTAL => Node::Crystal(Crystal {
                materials: self.crystal_materials,
                meshes: self.crystals,
            }),
            C_VERTEX_STREAM => Node::VertexStream(self.vstream),
            C_ITEM_MODEL | C_COMMON_ITEM_ENTITY_MODEL | C_BLOCK_ITEM | C_SOLID => {
                Node::ItemModel(self.entity_model)
            }
            c if is_visual(c) => Node::Visual(self.visual),
            c => Node::Other(c),
        }
    }
}

pub fn is_visual(class_id: u32) -> bool {
    matches!(
        class_id,
        C_VISUAL_INDEXED_TRIANGLES | 0x0906A000 | 0x0902C000 | 0x09006000 | 0x0900F000
    )
}

/// A short name for a node's kind, for messages.
pub fn node_kind_name(n: &Node) -> &'static str {
    match n {
        Node::Prefab(_) => "CPlugPrefab",
        Node::StaticObject(_) => "CPlugStaticObjectModel",
        Node::Dyna(_) => "CPlugDynaObjectModel",
        Node::Surface(_) => "CPlugSurface",
        Node::Solid2(_) => "CPlugSolid2Model",
        Node::Visual(_) => "CPlugVisual",
        Node::VertexStream(_) => "CPlugVertexStream",
        Node::Crystal(_) => "CPlugCrystal",
        Node::Material(..) => "material",
        Node::ItemModel(_) => "item model",
        Node::BlockInfo(_) => "CGameCtnBlockInfo",
        Node::Variant(_) => "CGameCtnBlockInfoVariant",
        Node::BlockUnit(_) => "CGameCtnBlockUnitInfo",
        Node::Mobil(_) => "CGameCtnBlockInfoMobil",
        Node::AutoTerrain(_) => "CGameCtnAutoTerrain",
        Node::Genealogy(_) => "CGameCtnZoneGenealogy",
        Node::RoadChunk(_) => "CPlugRoadChunk",
        Node::Light(c, _) => if *c == 0x0901D000 { "CPlugLight" } else { "GxLight" },
        Node::Tree(_) => "CPlugTree",
        Node::Layout(_) => "CSceneLayout",
        Node::ImageArray(_) => "CPlugImageArray",
        Node::Bitmap(_) => "CPlugBitmap",
        Node::FileGen(_) => "CPlugFileGen",
        Node::MaterialCustom(_) => "CPlugMaterialCustom",
        Node::Other(_) => "other",
    }
}
