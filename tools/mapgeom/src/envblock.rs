//! The lightmapper's ENVIRONMENT BLOCK from the pack: the decoration solids the game draws as layer 0
//! of every peel (and, a subset, into the sun shadow map), read off `<Coll>\GameCtnDecoration\Scene3d\
//! Base64x64.Scene3d.Gbx` (the `CSceneLayout`, `scene3d-cscenelayout.md`) and the solids it names,
//! with the SELECTION RULE the engine applies (RE 9, 2026-09-25; `lightmapper-client.md` §env).
//!
//! What the capture shows (pwc-day frame 127448, direction 0, `passcap/pwc-day/env/frame127448`): the
//! peel's layer 0 = the `ShadowCaster64` solid (draw eid 1009, 272 vertices / 262 triangles, the
//! "sea box": an inverted skirt around the island, black on its GPU-back faces), the FOUR
//! `Warp-C00x_1` leaves of the `Square64Water` solid (eids 1028/1033/1071/1076: 184 vertices / 280
//! triangles each = the "terrain patches" — a MESH of the pack, not a heightmap; the two R16 1024×1
//! textures of those draws are the `ACosSmooth` LUTs of the Warp shader, `paktables::acos_smooth_lut`),
//! the SKY DOME (eid 1051, `Sky\Media\Solid\SkyDomeMirror.Solid.Gbx` of Maniaplanet.pak: 2143
//! vertices / 3968 triangles, an ellipsoid of radii 22265.23 (x, z) × 9751.16 (y)) and the cloud
//! sprites (a different system, `clouds.rs`). NOT drawn: the four `Warp-C00x_0` Water leaves (83
//! vertices / 84 triangles each). The sun shadow map (eids 347–410) holds the ShadowCaster64 alone of
//! these — no WarpSand, no Water, no dome.
//!
//! THE RULE, transcribed [DISASSEMBLY] — the legacy `CPlugSolid` → `CPlugTree` render of the scene
//! mobils (`CVisionViewport::Shadow_RenderDelayed` 0x140a4fab0 → per CHmsItem 0x140970070 → per leaf
//! 0x14096fee0 → 0x1409514c0 → shader pick 0x140950f80):
//!
//! * `CPlugTree` flags (`0x0904F01A`, in memory tree+0xa8 = file word | 0x2000): the ROOT is rendered
//!   when `flags & 0x8` (IsVisible) or, in a SHADOW-MODE render (`ctx+0xb0 & 4` — the lightmapper's
//!   sun shadow map AND its peels both are), when `flags & 0x4000` (IsShadowCaster); a LEAF is
//!   submitted in shadow mode only when its own `flags & 0x4000`. Square64Water root 0x1e88a (both),
//!   `Warp-C00x_1` WarpSand 0x1e80a (both), `Warp-C00x_0` Water 0x1a80a (visible, NOT a caster → out
//!   of every lightmapper render), ShadowCaster64 0x1e802 (caster, invisible → in every lightmapper
//!   render, never in the game view), the dome's "Desert" tree 0x1e88a.
//! * The material's compiled shader (`CPlugShaderApply`, file chunk `0x09002020` v3 = {u32 A →
//!   shader+0x140, u32 B → +0x144, u32 → +0x150, f32, ref CPlugMaterialFx, u16 PASS BITS → +0x154}):
//!   0x1409514c0 skips the leaf unless `(passBits & ctx.mask) == ctx.required` with the default
//!   `mask 0x8130, required 0` (0x140408a70) — every environment shader passes (ShadowCaster 0x0401,
//!   Tech3 Warp PyPxzDiff 0x0441, Tech3_Water_MultiH 0x0041); 0x140950f80 refuses, in shadow mode, a
//!   shader with `B & 0x40000` (never casts: Water's 0x004c0020 has it, WarpSand's 0x0018fff0 and the
//!   ShadowCaster's 0x0008ff00 do not).
//! * The PASS KIND decides the sun shadow map vs the peel: the shadow-map render asks the shader for
//!   its shadow-kind variant (0x1403de0d0(shader, kind 6 | 0xb)); a shader without one is NOT drawn
//!   (0x140950f80: no variant, `ctx+0xbc` = no fallback). The variant exists for a pure-shader material
//!   (no `CPlugMaterialCustom`: InvisibleShadowCaster → the default caster program, the capture's VS
//!   1142 / PS 937) or, for a material with a custom part, when its program declares itself a caster
//!   (pass-0 program object +0x248 bit 0 — the source of that bit is NOT located) or has an alpha
//!   parameter (0x1403de6f0: a pass-0 parameter of kind 0x77 → the alpha-cut ShadowCasterCond variant,
//!   PS 1147: `discard TMapAlpha01.a − GbxShadowAlphaThreshold < 0`, threshold 128/255 = the
//!   vegetation card's leaves in the capture). Tech3 Warp PyPxzDiff has neither → WarpSand is absent
//!   from the sun shadow map. The peel renders each caster with its OWN colour program under the
//!   `RenderPath_DblSideBlackBack` permutation (ShadowCaster.PHlsl blob 1 = PS 17316 `discard_nz
//!   is_front_face; o0 = 0` — hence "black back faces"; Warp_PyPxz_p = PS 16752, `and o0.xyz, …,
//!   isfrontface`).
//!
//! So, per collection, the environment block = every leaf of every mobil solid with the caster flag
//! whose shader is not a never-caster; the sun shadow map = the subset whose material has no custom
//! part or an alpha texture. The dome mobil (`u01 & 0x400`) is that block's sky: its material is the
//! sky shader (`Tech3 Sky`), drawn like the rest (cull Back, `GbxSkyV0.VisualToWorld` = the mobil's
//! pose: identity on BlueBay/GreenCoast, (1024, 0, 1024) on RedIsland/WhiteShore).
//!
//! GPU vertex layout (the capture's input layouts): POSITION f32×3 @0, NORMAL snorm16×4 @12, stride
//! 20 (the terrain patches and the box); the dome adds TEXCOORD0 f32×2 @20 (stride 28). The pack
//! stores the normals as Dec3N words; `snorm16_of_dec3n` is the engine's re-encoding (verified
//! bit for bit against the capture by `envblock_matches_capture`).

use crate::node::{Node, Slot};
use crate::store::DataStore;

/// One `CPlugTree` leaf of a decoration mobil: what the game uploads for it and what decides its
/// presence in each lightmapper render.
#[derive(Clone, Debug)]
pub struct EnvLeaf {
    /// Index of the mobil in the layout, its name (`Warp` on the island solid, empty otherwise).
    pub mobil: usize,
    pub mobil_name: String,
    /// The mobil's placement (position, quaternion x y z w) — identity except the dome of
    /// RedIsland/WhiteShore.
    pub pos: [f32; 3],
    pub rot: [f32; 4],
    /// The layout's u16 per mobil (0x401 on the dome, 1 on the solids).
    pub mobil_kind: u16,
    /// Where the solid comes from: `inline` or the external `.Solid.Gbx` path.
    pub solid: String,
    /// Tree names from the solid's root down to this leaf.
    pub path: Vec<String>,
    /// The leaf's and the root's `0x0904F01A` flags (file words).
    pub flags: u32,
    pub root_flags: u32,
    /// The leaf's material file (`<Coll>\Media\Material\WarpSand.Material.Gbx`) and its stem.
    pub material: String,
    pub material_stem: String,
    /// The material's shader chain: the parent `.Material.gbx` and the `.Shader.Gbx` it names.
    pub parent_material: String,
    pub shader: String,
    pub shader_flags: Option<ShaderFlags>,
    /// Whether the material carries a `CPlugMaterialCustom` (bitmaps / parameters of its own).
    pub has_custom: bool,
    /// The custom part's texture slots (name, texture path) and float parameters.
    pub bitmaps: Vec<(String, String)>,
    pub params: Vec<(String, Vec<f32>)>,
    /// Per texture slot, the texture's `0x09011025` projection transform (slot name → transform) —
    /// what `GbxWorldPosToTexCoord_Map<Slot>` / `GbxSamplerTcScaleTrans_<Slot>` are built from.
    pub texcoord: Vec<(String, TexCoordTransform)>,
    /// The vertex stream as stored: positions (metres, the solid's own space), Dec3N normals, uv0.
    pub positions: Vec<[f32; 3]>,
    pub normals_dec3n: Vec<u32>,
    pub normals: Vec<[f32; 3]>,
    pub uv0: Vec<[f32; 2]>,
    /// Absolute triangle-list indices.
    pub indices: Vec<u32>,
}

/// A projected texture's world → texcoord transform: `CPlugBitmap` chunk `0x09011025` = {Vec2 scale,
/// Vec2 trans, f32 rotate (degrees, member `DefaultTexCoordRotate`), u32 colour}.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TexCoordTransform {
    pub scale: [f32; 2],
    pub trans: [f32; 2],
    pub rotate_deg: f32,
    pub colour: u32,
}

impl TexCoordTransform {
    /// The shader constant `GbxWorldPosToTexCoord_Map<Slot>` (a float4x2 as HLSL rows x, y, z, w →
    /// (u, v)): u = su·(cos a·x + sin a·z) + tu, v = sv·(sin a·x − cos a·z) + tv with a = deg·(π/180)
    /// in f32 (π/180 = 0x3c8efa35, the CRT cosf/sinf, the products in that order) — the capture's
    /// WarpSand rows (0x39fd3630, 0x3907b21b) / (0x3907b21b, 0xb9fd3630) bit for bit. The code site
    /// that fills the constant (case 0xca of the constant provider 0x1409f87a0 → 0x1409f8570 from an
    /// Iso4 the texture binding holds at +0x48) was read; the Iso4's own builder was not located, so
    /// the arithmetic is transcribed from the reproduced bits, not from its instructions.
    pub fn world_pos_to_texcoord(&self) -> [[f32; 2]; 4] {
        let a = self.rotate_deg * f32::from_bits(0x3c8e_fa35);
        let (s, c) = (a.sin(), a.cos());
        [[c * self.scale[0], s * self.scale[1]], [0.0, 0.0], [s * self.scale[0], -c * self.scale[1]], [self.trans[0], self.trans[1]]]
    }
}

/// The `CPlugShaderApply` chunk `0x09002020` words (reader 0x1403da430 case 0x9002020).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ShaderFlags {
    /// → shader+0x140
    pub a: u32,
    /// → shader+0x144 (bit 0x40000 = never a shadow caster, 0x140950f80)
    pub b: u32,
    /// → shader+0x150 (v ≥ 3)
    pub c: u32,
    pub f: f32,
    /// → shader+0x154, the render-pass bits tested against the context mask (0x1409514c0)
    pub pass_bits: u16,
}

impl ShaderFlags {
    /// `(passBits & mask) == required` with the default context of the lightmapper renders
    /// (0x140408a70: mask 0x8130, required 0).
    pub fn passes_default_mask(&self) -> bool {
        (self.pass_bits & 0x8130) == 0
    }
    /// shader+0x144 bit 0x40000: refused by every shadow-mode render (0x140950f80).
    pub fn never_casts(&self) -> bool {
        self.b & 0x40000 != 0
    }
}

/// The tree flag bits the renders test (`CPlugTree` +0xa8).
pub const TREE_IS_VISIBLE: u32 = 0x8;
pub const TREE_IS_SHADOW_CASTER: u32 = 0x4000;

/// Where a leaf is drawn by the lightmapper.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EnvRole {
    /// The sky dome mobil (`u01 & 0x400`): the peel's background radiance.
    SkyDome,
    /// In every peel's layer 0 AND the sun shadow map.
    CasterAndShadow,
    /// In every peel's layer 0 only (no shadow-kind shader variant).
    PeelOnly,
    /// Not drawn by the lightmapper (visible in the game view only).
    Excluded,
}

impl EnvLeaf {
    /// The role the transcribed rule gives this leaf.
    pub fn role(&self) -> EnvRole {
        if self.mobil_kind & 0x400 != 0 {
            return EnvRole::SkyDome;
        }
        let root_ok = self.root_flags & TREE_IS_VISIBLE != 0 || self.root_flags & TREE_IS_SHADOW_CASTER != 0;
        let leaf_ok = self.flags & TREE_IS_SHADOW_CASTER != 0;
        let shader_ok = match self.shader_flags {
            Some(f) => f.passes_default_mask() && !f.never_casts(),
            None => true,
        };
        if !(root_ok && leaf_ok && shader_ok) {
            return EnvRole::Excluded;
        }
        // The shadow-kind variant: a pure-shader material takes the default caster program; a
        // material with a custom part needs a declared caster program or an alpha texture
        // (`Alpha01`-class slot → the ShadowCasterCond alpha-cut variant).
        let alpha = self.bitmaps.iter().any(|(n, _)| n.to_ascii_lowercase().contains("alpha"));
        if !self.has_custom || alpha {
            EnvRole::CasterAndShadow
        } else {
            EnvRole::PeelOnly
        }
    }

    pub fn triangles(&self) -> usize {
        self.indices.len() / 3
    }

    /// World-space positions: the mobil's pose applied (identity everywhere but the dome of
    /// RedIsland/WhiteShore).
    pub fn world_positions(&self) -> Vec<[f32; 3]> {
        let m = crate::geom::from_quat(self.rot, self.pos);
        self.positions.iter().map(|p| crate::geom::apply(&m, *p)).collect()
    }

    /// The bytes the game uploads as this leaf's vertex buffer: POSITION f32×3, NORMAL snorm16×4
    /// (Dec3N re-encoded), and TEXCOORD0 f32×2 when the stream has one (the dome) — stride 20 or 28.
    pub fn gpu_vertex_buffer(&self) -> Vec<u8> {
        let has_uv = !self.uv0.is_empty();
        let mut out = Vec::with_capacity(self.positions.len() * if has_uv { 28 } else { 20 });
        for (i, p) in self.positions.iter().enumerate() {
            for c in p {
                out.extend_from_slice(&c.to_le_bytes());
            }
            let n = if let Some(w) = self.normals_dec3n.get(i) {
                snorm16_of_dec3n(*w)
            } else {
                let n = self.normals.get(i).copied().unwrap_or([0.0, 0.0, 0.0]);
                [snorm16(n[0]), snorm16(n[1]), snorm16(n[2]), 0]
            };
            for c in n {
                out.extend_from_slice(&c.to_le_bytes());
            }
            if has_uv {
                let uv = self.uv0.get(i).copied().unwrap_or([0.0, 0.0]);
                out.extend_from_slice(&uv[0].to_le_bytes());
                out.extend_from_slice(&uv[1].to_le_bytes());
            }
        }
        out
    }

    /// The index buffer as uploaded (u16 triangle list).
    pub fn gpu_index_buffer(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(self.indices.len() * 2);
        for i in &self.indices {
            out.extend_from_slice(&(*i as u16).to_le_bytes());
        }
        out
    }
}

/// One 10-bit signed Dec3N component (two's complement, 0x200 = −512) → the GPU's snorm16: the
/// stored integer s is decoded to the float s/511 (`float_to_tenb`'s inverse, `tenb`) and re-encoded
/// as `trunc(f · 32767)` — a float round trip with truncation toward zero, not a bit shift: −16 →
/// −1025 (not −1024), 510 → 32702, 14 → 897, −511 → −32767, matching every vertex of the capture's
/// terrain patches, box and dome (`envblock_matches_capture`).
pub fn snorm16_of_dec3n(w: u32) -> [i16; 4] {
    let comp = |sh: u32| -> i16 {
        let v = ((w >> sh) & 0x3ff) as i32;
        let s = if v >= 0x200 { v - 0x400 } else { v };
        let f = (s as f32 / 511.0).clamp(-1.0, 1.0);
        (f * 32767.0) as i16
    };
    // the two-bit w field: 0..3 → its snorm16 (0 on the pack's normals)
    let w2 = ((w >> 30) & 3) as i16;
    [comp(0), comp(10), comp(20), w2]
}

fn snorm16(f: f32) -> i16 {
    (f.clamp(-1.0, 1.0) * 32767.0).round() as i16
}

/// The environment block of one collection.
#[derive(Clone, Debug, Default)]
pub struct EnvBlock {
    pub collection: String,
    pub layout_path: String,
    pub leaves: Vec<EnvLeaf>,
}

impl EnvBlock {
    pub fn by_role(&self, role: EnvRole) -> impl Iterator<Item = &EnvLeaf> {
        self.leaves.iter().filter(move |l| l.role() == role)
    }
    /// The leaves of every peel's layer 0 (casters, with or without a shadow variant).
    pub fn peel_leaves(&self) -> impl Iterator<Item = &EnvLeaf> {
        self.leaves.iter().filter(|l| matches!(l.role(), EnvRole::CasterAndShadow | EnvRole::PeelOnly))
    }
    pub fn sun_shadow_leaves(&self) -> impl Iterator<Item = &EnvLeaf> {
        self.by_role(EnvRole::CasterAndShadow)
    }
    pub fn sky_dome(&self) -> Option<&EnvLeaf> {
        self.by_role(EnvRole::SkyDome).next()
    }
}

/// The layout paths a collection's decoration may use, in the order tried: the 64×64 island layout
/// (BlueBay, GreenCoast, RedIsland, WhiteShore) and Stadium's (the `Stadium256` decoration
/// collection's 16×12 layout: one `SkyDome` mobil at (0, 3000, 0) drawing
/// `Sky\Media\Solid\SkyDomeDouble.Solid.Gbx`, no ground solids).
pub fn layout_candidates(collection: &str) -> Vec<String> {
    let mut v = vec![format!("{collection}\\GameCtnDecoration\\Scene3d\\Base64x64.Scene3d.Gbx")];
    if collection.eq_ignore_ascii_case("Stadium") || collection.eq_ignore_ascii_case("Stadium256") {
        v.push("Stadium256\\GameCtnDecoration\\Scene3d\\Base16x12.Scene3d.Gbx".to_string());
    }
    v
}

/// The first layout of `layout_candidates` the store holds (else the first candidate).
pub fn layout_path(store: &DataStore, collection: &str) -> String {
    let c = layout_candidates(collection);
    c.iter().find(|p| store.resolve(p).is_some()).cloned().unwrap_or_else(|| c[0].clone())
}

/// Read a collection's environment block. The store needs the collection pack and, for the sky
/// dome, Maniaplanet.pak (`Sky\Media\Solid\SkyDomeMirror.Solid.Gbx`, `Techno3\Media\Shader\…`);
/// a missing external is reported in the leaf list as an error string, never silently dropped.
pub fn load(store: &mut DataStore, collection: &str) -> Result<EnvBlock, String> {
    let path = layout_path(store, collection);
    load_layout(store, collection, &path)
}

/// `load` from an explicit layout file.
pub fn load_layout(store: &mut DataStore, collection: &str, layout: &str) -> Result<EnvBlock, String> {
    let path = layout.to_string();
    let model = store.load_model(&path)?;
    let graph = model.graph().map_err(|e| format!("{path}: {e}"))?;
    let layout = match &graph.root {
        Some(Node::Layout(l)) => (**l).clone(),
        Some(n) => return Err(format!("{path}: root is {}, not a CSceneLayout", crate::node::node_kind_name(n))),
        None => return Err(format!("{path}: no root node")),
    };
    let slots = graph.slots.clone();
    drop(graph);
    let mut block = EnvBlock { collection: collection.to_string(), layout_path: path.clone(), leaves: Vec::new() };
    for (mi, m) in layout.mobils.iter().enumerate() {
        if m.solid < 0 {
            continue;
        }
        match slots.get(m.solid as usize) {
            Some(Slot::Node(_)) => {
                collect_solid(store, &slots, m.solid, "inline", mi, m, &mut block.leaves)?;
            }
            Some(Slot::External(p)) => {
                let p = p.clone();
                let sm = store.load_model(&p).map_err(|e| format!("mobil {mi} solid {p}: {e}"))?;
                let g = sm.graph().map_err(|e| format!("{p}: {e}"))?;
                let root = g.root.clone();
                let sslots = g.slots.clone();
                drop(g);
                // an external .Solid.Gbx: its root is the CPlugSolid (ItemModel → tree)
                let root_idx = match root {
                    Some(Node::ItemModel(t)) => t,
                    Some(Node::Tree(_)) => -1,
                    _ => return Err(format!("{p}: unexpected root")),
                };
                if root_idx >= 0 {
                    let mut leaves = Vec::new();
                    walk_tree(store, &sslots, root_idx, root_idx, &mut Vec::new(), &p, mi, m, &mut leaves)?;
                    block.leaves.extend(leaves);
                }
            }
            _ => return Err(format!("mobil {mi}: solid slot {} unset", m.solid)),
        }
    }
    Ok(block)
}

fn collect_solid(store: &mut DataStore, slots: &[Slot], solid: i32, origin: &str, mi: usize, m: &crate::node::LayoutMobil, out: &mut Vec<EnvLeaf>) -> Result<(), String> {
    let tree = match slots.get(solid as usize) {
        Some(Slot::Node(Node::ItemModel(t))) => *t,
        Some(Slot::Node(Node::Tree(_))) => solid,
        _ => return Err(format!("mobil {mi}: solid node {solid} is not a CPlugSolid")),
    };
    walk_tree(store, slots, tree, tree, &mut Vec::new(), origin, mi, m, out)
}

#[allow(clippy::too_many_arguments)]
fn walk_tree(store: &mut DataStore, slots: &[Slot], root: i32, tree: i32, path: &mut Vec<String>, origin: &str, mi: usize, m: &crate::node::LayoutMobil, out: &mut Vec<EnvLeaf>) -> Result<(), String> {
    let t = match slots.get(tree.max(0) as usize) {
        Some(Slot::Node(Node::Tree(t))) if tree >= 0 => t.clone(),
        _ => return Ok(()),
    };
    let root_flags = match slots.get(root.max(0) as usize) {
        Some(Slot::Node(Node::Tree(r))) => r.flags,
        _ => t.flags,
    };
    path.push(t.name.clone());
    if t.visual >= 0 {
        let (positions, normals_dec3n, normals, uv0, indices) = visual_data(slots, t.visual);
        let material = match slots.get(t.shader.max(0) as usize) {
            Some(Slot::External(p)) if t.shader >= 0 => p.clone(),
            Some(Slot::Node(Node::Material(n, _))) if t.shader >= 0 => n.clone(),
            _ => String::new(),
        };
        let stem = material.rsplit(['\\', '/']).next().unwrap_or(&material).trim_end_matches(".Material.Gbx").trim_end_matches(".Material.gbx").to_string();
        let chain = material_chain(store, &material);
        out.push(EnvLeaf {
            mobil: mi,
            mobil_name: m.name.clone(),
            pos: m.pos,
            rot: m.rot,
            mobil_kind: m.u01,
            solid: origin.to_string(),
            path: path.clone(),
            flags: t.flags,
            root_flags,
            material,
            material_stem: stem,
            parent_material: chain.parent_material,
            shader: chain.shader,
            shader_flags: chain.flags,
            has_custom: chain.has_custom,
            bitmaps: chain.bitmaps,
            params: chain.params,
            texcoord: chain.texcoord,
            positions,
            normals_dec3n,
            normals,
            uv0,
            indices,
        });
    }
    for c in &t.children {
        walk_tree(store, slots, root, *c, path, origin, mi, m, out)?;
    }
    path.pop();
    Ok(())
}

type VisualData = (Vec<[f32; 3]>, Vec<u32>, Vec<[f32; 3]>, Vec<[f32; 2]>, Vec<u32>);

fn visual_data(slots: &[Slot], vi: i32) -> VisualData {
    let v = match slots.get(vi.max(0) as usize) {
        Some(Slot::Node(Node::Visual(v))) => v,
        _ => return Default::default(),
    };
    let mut positions = v.inline_positions.clone();
    let mut normals = v.inline_normals.clone();
    let mut normals_dec3n = Vec::new();
    let mut uv0 = v.uv0.clone();
    for si in &v.vertex_streams {
        if let Some(Slot::Node(Node::VertexStream(vs))) = slots.get((*si).max(0) as usize) {
            positions.extend_from_slice(&vs.positions);
            normals.extend_from_slice(&vs.normals);
            normals_dec3n.extend_from_slice(&vs.normals_dec3n);
            uv0.extend_from_slice(&vs.uv0);
        }
    }
    let n = positions.len();
    // the index list: absolute, or a delta chain from 0 (geom.rs's rule)
    let absolute = v.index_is_absolute && (n == 0 || v.indices.iter().all(|i| (*i as usize) < n));
    let indices: Vec<u32> = if absolute || n == 0 {
        v.indices.clone()
    } else {
        let mut cur = 0i64;
        v.indices.iter().map(|d| { cur = (cur + *d as i16 as i64).rem_euclid(n as i64); cur as u32 }).collect()
    };
    (positions, normals_dec3n, normals, uv0, indices)
}

/// A material's shader chain and custom part.
#[derive(Clone, Debug, Default)]
pub struct MaterialChain {
    pub parent_material: String,
    pub shader: String,
    pub flags: Option<ShaderFlags>,
    pub has_custom: bool,
    pub bitmaps: Vec<(String, String)>,
    pub params: Vec<(String, Vec<f32>)>,
    pub texcoord: Vec<(String, TexCoordTransform)>,
}

/// `X.Material.Gbx` → (its CPlugMaterialCustom: bitmaps + params) and, through the parent
/// `.Material.gbx` references, the `.Shader.Gbx` file with its `0x09002020` words.
pub fn material_chain(store: &mut DataStore, material: &str) -> MaterialChain {
    let mut out = MaterialChain::default();
    if material.is_empty() {
        return out;
    }
    let mut cur = material.to_string();
    for _depth in 0..6 {
        let Ok(m) = store.load_model(&cur) else { break };
        let Ok(g) = m.graph() else { break };
        let mut next_material = None;
        let mut shader = None;
        for s in &g.slots {
            match s {
                Slot::Node(Node::MaterialCustom(c)) => {
                    out.has_custom = true;
                    for (name, idx) in &c.bitmaps {
                        let p = match g.slots.get((*idx).max(0) as usize) {
                            Some(Slot::External(p)) if *idx >= 0 => p.clone(),
                            _ => String::new(),
                        };
                        out.bitmaps.push((name.clone(), p));
                    }
                    for (name, vals) in &c.params {
                        out.params.push((name.clone(), vals.clone()));
                    }
                }
                Slot::External(p) => {
                    let up = p.to_ascii_uppercase();
                    if up.ends_with(".MATERIAL.GBX") && next_material.is_none() {
                        next_material = Some(p.clone());
                    } else if up.ends_with(".SHADER.GBX") && shader.is_none() {
                        shader = Some(p.clone());
                    }
                }
                _ => {}
            }
        }
        drop(g);
        if let Some(s) = shader {
            out.shader = s.clone();
            // the bitmaps' 0x09011025 transforms (the texture files of the custom part)
            let slots: Vec<(String, String)> = out.bitmaps.clone();
            for (name, path) in slots {
                if path.is_empty() { continue; }
                if let Ok(bm) = store.load_model(&path) {
                    if let Ok(bg) = bm.graph() {
                        if let Some(Node::Bitmap(b)) = &bg.root {
                            if let Some(v) = b.tc_scale_trans {
                                out.texcoord.push((name.clone(), TexCoordTransform { scale: [f32::from_bits(v[0]), f32::from_bits(v[1])], trans: [f32::from_bits(v[2]), f32::from_bits(v[3])], rotate_deg: f32::from_bits(v[4]), colour: v[5] }));
                            }
                        }
                    }
                }
            }
            if let Ok(sm) = store.load_model(&s) {
                out.flags = shader_flags(&sm.body);
            }
            break;
        }
        match next_material {
            Some(p) => {
                out.parent_material = p.clone();
                cur = p;
            }
            None => break,
        }
    }
    out
}

/// The `0x09002020` chunk of a `.Shader.Gbx` body (reader 0x1403da430: `u32 version (3); 8 bytes →
/// +0x140/+0x144; v ≥ 3: 4 bytes → +0x150; f32; ref 0x0907A000; u16 → +0x154`). The shader file's
/// chunk list has inline pass nodes this crate has no typed reader for, so the chunk is located by
/// its id word — a 32-bit pattern the pass bodies (names, floats, small ints) do not contain.
pub fn shader_flags(body: &[u8]) -> Option<ShaderFlags> {
    let id = 0x0900_2020u32.to_le_bytes();
    let mut i = 0;
    while i + 4 <= body.len() {
        if body[i..i + 4] == id {
            let u = |o: usize| -> Option<u32> { body.get(i + o..i + o + 4).map(|b| u32::from_le_bytes(b.try_into().unwrap())) };
            let version = u(4)?;
            if version == 0 || version > 3 {
                i += 1;
                continue;
            }
            let a = u(8)?;
            let b = u(12)?;
            let (c, mut o) = if version >= 3 { (u(16)?, 20) } else { (0, 16) };
            let f = f32::from_bits(u(o)?);
            o += 4;
            let r = u(o)? as i32;
            o += 4;
            if r != -1 {
                // an inline CPlugMaterialFx would follow; none of the environment shaders has one
                return None;
            }
            let pass_bits = u16::from_le_bytes(body.get(i + o..i + o + 2)?.try_into().unwrap());
            return Some(ShaderFlags { a, b, c, f, pass_bits });
        }
        i += 1;
    }
    None
}

/// The OBJ text of the block's leaves (world space, one group per leaf, `usemtl` = material stem).
pub fn obj_text(block: &EnvBlock, roles: &[EnvRole]) -> String {
    let mut s = String::new();
    s.push_str(&format!("# {} environment block from {}\n", block.collection, block.layout_path));
    let mut base = 1usize;
    for l in &block.leaves {
        if !roles.contains(&l.role()) {
            continue;
        }
        s.push_str(&format!("o mobil{}_{}_{}\nusemtl {}\n", l.mobil, l.path.last().cloned().unwrap_or_default().replace(' ', "_"), format!("{:?}", l.role()), l.material_stem));
        for p in l.world_positions() {
            s.push_str(&format!("v {} {} {}\n", p[0], p[1], p[2]));
        }
        for n in &l.normals {
            s.push_str(&format!("vn {} {} {}\n", n[0], n[1], n[2]));
        }
        let has_uv = !l.uv0.is_empty();
        for uv in &l.uv0 {
            s.push_str(&format!("vt {} {}\n", uv[0], uv[1]));
        }
        for t in l.indices.chunks_exact(3) {
            let f = |k: u32| {
                let i = base + k as usize;
                if has_uv { format!("{i}/{i}/{i}") } else { format!("{i}//{i}") }
            };
            s.push_str(&format!("f {} {} {}\n", f(t[0]), f(t[1]), f(t[2])));
        }
        base += l.positions.len();
    }
    s
}

/// A one-line summary per leaf.
pub fn describe(l: &EnvLeaf) -> String {
    let fl = l.shader_flags.map(|f| format!("A 0x{:08x} B 0x{:08x} C 0x{:x} f {} pass 0x{:04x}", f.a, f.b, f.c, f.f, f.pass_bits)).unwrap_or_else(|| "shader flags: not read".into());
    format!(
        "mobil {} {:?} kind 0x{:x} pos {:?} | {} | {} flags 0x{:x} (root 0x{:x}) | {} verts {} tris | uv {} | material {} → {} → {} | custom {} bitmaps {:?} params {:?} texcoord {:?} | {} | {:?}",
        l.mobil, l.mobil_name, l.mobil_kind, l.pos, l.solid, l.path.join(" > "), l.flags, l.root_flags, l.positions.len(), l.triangles(), !l.uv0.is_empty(), l.material_stem, l.parent_material.rsplit('\\').next().unwrap_or(""), l.shader.rsplit('\\').next().unwrap_or(""), l.has_custom, l.bitmaps.iter().map(|(n, p)| format!("{n}={}", p.rsplit('\\').next().unwrap_or(p))).collect::<Vec<_>>(), l.params, l.texcoord.iter().map(|(n, t)| format!("{n}: scale {:?} trans {:?} rot {}°", t.scale, t.trans, t.rotate_deg)).collect::<Vec<_>>(), fl, l.role()
    )
}

// ------------------------------------------------------------------ the capture check

/// The result of `compare_capture`.
#[derive(Debug, Default)]
pub struct CaptureCheck {
    pub lines: Vec<String>,
    pub exact: usize,
    pub differ: usize,
    pub unmatched: usize,
}

fn gunzip(b: &[u8]) -> Result<Vec<u8>, String> {
    if b.len() < 18 || b[0] != 0x1f || b[1] != 0x8b || b[2] != 8 {
        return Err("not a gzip member".into());
    }
    let flg = b[3];
    let mut o = 10usize;
    if flg & 4 != 0 {
        let xlen = u16::from_le_bytes([b[o], b[o + 1]]) as usize;
        o += 2 + xlen;
    }
    for bit in [8u8, 16] {
        if flg & bit != 0 {
            while o < b.len() && b[o] != 0 {
                o += 1;
            }
            o += 1;
        }
    }
    if flg & 2 != 0 {
        o += 2;
    }
    if o + 8 > b.len() {
        return Err("truncated gzip".into());
    }
    miniz_oxide::inflate::decompress_to_vec(&b[o..b.len() - 8]).map_err(|e| format!("gzip inflate: {e:?}"))
}

/// A file, or its `.gz` sibling.
pub fn read_maybe_gz(p: &std::path::Path) -> Result<Vec<u8>, String> {
    if p.exists() {
        let b = std::fs::read(p).map_err(|e| format!("{}: {e}", p.display()))?;
        return if b.len() > 2 && b[0] == 0x1f && b[1] == 0x8b { gunzip(&b) } else { Ok(b) };
    }
    let gz = std::path::PathBuf::from(format!("{}.gz", p.display()));
    if gz.exists() {
        let b = std::fs::read(&gz).map_err(|e| format!("{}: {e}", gz.display()))?;
        return gunzip(&b);
    }
    Err(format!("{}: not found (nor .gz)", p.display()))
}

/// One captured environment draw: its vertex buffer (bytes, stride) and index list (u16, when banked).
#[derive(Clone, Debug)]
pub struct CapturedDraw {
    pub eid: u32,
    pub res: String,
    pub stride: usize,
    pub vb: Vec<u8>,
    pub indices: Vec<u16>,
    pub note: String,
}

/// Drop the whitespace outside string literals of a pretty-printed JSON dump so byte patterns hold.
fn compact_json(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len());
    let (mut in_str, mut esc) = (false, false);
    for ch in raw.chars() {
        if in_str {
            out.push(ch);
            if esc {
                esc = false;
            } else if ch == '\\' {
                esc = true;
            } else if ch == '"' {
                in_str = false;
            }
        } else if ch == '"' {
            in_str = true;
            out.push(ch);
        } else if !ch.is_whitespace() {
            out.push(ch);
        }
    }
    out
}

/// The captured environment draws of frame `frame` under a passcap root (`passcap/pwc-day`): the
/// records of `env/frame<N>/mesh.json` and `logs/mesh-frame<N>.json` whose input layout is the
/// environment one (POSITION f32×3 @0, NORMAL snorm16×4 @12, stride 20 or 28 — the items' 40/52-byte
/// layouts are skipped), their vertex bytes from `env/frame<N>/mesh/<file>` or
/// `mesh/frame<N>/e00XXXX_vb0_<res>.bin[.gz]`, their index list from `…/e00XXXX_vsout_indices.bin[.gz]`.
pub fn captured_draws(passcap: &std::path::Path, frame: u32) -> Result<Vec<CapturedDraw>, String> {
    let env = passcap.join("env").join(format!("frame{frame}"));
    let sources = [env.join("mesh.json"), passcap.join("logs").join(format!("mesh-frame{frame}.json"))];
    let mut out: Vec<CapturedDraw> = Vec::new();
    let mut any = false;
    for src in &sources {
        let Ok(raw) = std::fs::read_to_string(src) else { continue };
        any = true;
        let json = compact_json(&raw);
        for rec in json.split("{\"eid\":").skip(1) {
            let eid: u32 = rec.split(',').next().and_then(|s| s.trim().parse().ok()).unwrap_or(0);
            if !(rec.contains("\"semantic\":\"POSITION\",\"index\":0,\"format\":\"R32G32B32_FLOAT\"") && rec.contains("\"semantic\":\"NORMAL\",\"index\":0,\"format\":\"R16G16B16A16_SNORM\",\"slot\":0,\"offset\":12")) {
                continue;
            }
            let Some(vbs) = rec.split("\"vertex_buffers\":[").nth(1) else { continue };
            let first = vbs.split(']').next().unwrap_or("");
            let field = |k: &str| -> Option<String> {
                let s = first.split(&format!("\"{k}\":")).nth(1)?;
                let s = s.trim_start_matches('"');
                Some(s.split(['"', ',', '}']).next()?.trim().to_string())
            };
            let (Some(res), Some(stride)) = (field("res"), field("stride")) else { continue };
            let stride: usize = stride.parse().unwrap_or(0);
            if stride != 20 && stride != 28 {
                continue;
            }
            if out.iter().any(|d| d.eid == eid && d.res == res) {
                continue;
            }
            let file = field("file").unwrap_or_else(|| format!("vb_{res}.bin"));
            let vb = read_maybe_gz(&env.join("mesh").join(&file))
                .or_else(|_| read_maybe_gz(&passcap.join("mesh").join(format!("frame{frame}")).join(&file)))
                .or_else(|_| read_maybe_gz(&passcap.join("mesh").join(format!("frame{frame}")).join(format!("e{eid:06}_vb0_{res}.bin"))));
            let idx_name = format!("e{eid:06}_vsout_indices.bin");
            let ib = read_maybe_gz(&env.join("mesh").join(&idx_name)).or_else(|_| read_maybe_gz(&passcap.join("mesh").join(format!("frame{frame}")).join(&idx_name)));
            let (vb, mut note) = match vb {
                Ok(b) => (b, String::new()),
                Err(e) => (Vec::new(), format!("vertex bytes missing: {e}")),
            };
            let indices = match ib {
                Ok(b) => b.chunks_exact(2).map(|c| u16::from_le_bytes([c[0], c[1]])).collect(),
                Err(e) => {
                    note.push_str(&format!(" index list missing: {e}"));
                    Vec::new()
                }
            };
            out.push(CapturedDraw { eid, res, stride, vb, indices, note });
        }
    }
    if !any {
        return Err(format!("{}: neither env/frame{frame}/mesh.json nor logs/mesh-frame{frame}.json", passcap.display()));
    }
    out.sort_by_key(|d| d.eid);
    Ok(out)
}

/// Every captured environment draw of the frame against the block's leaves: the vertex buffer bit
/// for bit (positions, snorm16 normals, uvs) and the index list.
pub fn compare_capture(block: &EnvBlock, passcap: &std::path::Path, frame: u32) -> Result<CaptureCheck, String> {
    let caps = captured_draws(passcap, frame)?;
    let mut r = CaptureCheck::default();
    let leaf_bufs: Vec<Vec<u8>> = block.leaves.iter().map(|l| l.gpu_vertex_buffer()).collect();
    for d in &caps {
        let label = format!("eid {} vb {}", d.eid, d.res);
        if d.vb.is_empty() {
            r.lines.push(format!("{label}: {}", d.note));
            r.unmatched += 1;
            continue;
        }
        let stride = d.stride.max(1);
        if let Some(i) = leaf_bufs.iter().position(|b| *b == d.vb) {
            let l = &block.leaves[i];
            let idx_ok = if d.indices.is_empty() {
                " (no index list banked)".to_string()
            } else if d.indices.len() == l.indices.len() && d.indices.iter().zip(&l.indices).all(|(a, b)| *a as u32 == *b) {
                format!(", {} indices EXACT", d.indices.len())
            } else {
                let first = d.indices.iter().zip(&l.indices).position(|(a, b)| *a as u32 != *b);
                r.differ += 1;
                format!(", INDICES DIFFER: {} captured vs {} ours, first at {:?}", d.indices.len(), l.indices.len(), first)
            };
            r.lines.push(format!("{label}: {} bytes (stride {stride}) == leaf {i} mobil {} {} — VERTICES EXACT ({}){}{}", d.vb.len(), l.mobil, l.path.join(" > "), l.positions.len(), idx_ok, d.note));
            r.exact += 1;
            continue;
        }
        let mut best: Option<(usize, usize)> = None;
        for (i, b) in leaf_bufs.iter().enumerate() {
            if b.len() != d.vb.len() {
                continue;
            }
            let eq = b.iter().zip(&d.vb).filter(|(x, y)| x == y).count();
            if best.map_or(true, |(_, e)| eq > e) {
                best = Some((i, eq));
            }
        }
        match best {
            Some((i, eq)) => {
                let l = &block.leaves[i];
                let b = &leaf_bufs[i];
                let first = b.iter().zip(&d.vb).position(|(x, y)| x != y).unwrap_or(0);
                let n = d.vb.len() / stride;
                let bad_verts = (0..n).filter(|v| b[v * stride..(v + 1) * stride] != d.vb[v * stride..(v + 1) * stride]).count();
                r.lines.push(format!(
                    "{label}: {} bytes == leaf {i} {} in length, {} bytes differ ({} of {} vertices), first at byte {} (vertex {} field byte {}): ours {:02x?} theirs {:02x?}",
                    d.vb.len(), l.path.join(" > "), d.vb.len() - eq, bad_verts, n, first, first / stride, first % stride,
                    &b[first..(first + 8).min(b.len())], &d.vb[first..(first + 8).min(d.vb.len())]
                ));
                r.differ += 1;
            }
            None => {
                r.lines.push(format!("{label}: {} bytes (stride {stride}, {} vertices) matches no leaf's length — not a decoration mobil (the zone tiles' quad, eid 365 on pwc-day, shares the layout)", d.vb.len(), d.vb.len() / stride));
                r.unmatched += 1;
            }
        }
    }
    Ok(r)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn paks_dir() -> String {
        std::env::var("TM_PAKS").unwrap_or_else(|_| format!("{}/persistent/private-30d/tm-paks", std::env::var("HOME").unwrap_or_default()))
    }

    fn passcap_dir() -> String {
        std::env::var("TM_PASSCAP").unwrap_or_else(|_| format!("{}/persistent/private-30d/tm-player/tiny/lightmap-re/passcap/pwc-day", std::env::var("HOME").unwrap_or_default()))
    }

    /// The store of one island collection + Maniaplanet.pak, or None when the packs are not on this box.
    fn island_store(coll: &str) -> Option<DataStore> {
        let dir = paks_dir();
        let cp = format!("{dir}/{coll}.pak");
        let mp = format!("{dir}/Maniaplanet.pak");
        if !std::path::Path::new(&cp).exists() || !std::path::Path::new(&mp).exists() {
            eprintln!("skipped: {cp} / {mp} not on this box");
            return None;
        }
        let mut s = DataStore::empty();
        s.add_pak(&cp, "660C4C156B80337E296A1034B0AA05B8").unwrap();
        s.add_pak(&mp, "9A93723447347A8CE336CCFC49E65449").unwrap();
        Some(s)
    }

    #[test]
    fn dec3n_to_snorm16_is_a_truncated_float_round_trip() {
        // the capture's normals against the pack's words (terrain patch vertex 0 etc.)
        let enc = |x: i32, y: i32, z: i32| -> u32 { ((x & 0x3ff) as u32) | (((y & 0x3ff) as u32) << 10) | (((z & 0x3ff) as u32) << 20) };
        assert_eq!(snorm16_of_dec3n(enc(-16, 510, 14)), [-1025, 32702, 897, 0]);
        assert_eq!(snorm16_of_dec3n(enc(0, -511, 0)), [0, -32767, 0, 0]);
        assert_eq!(snorm16_of_dec3n(enc(511, 0, -512)), [32767, 0, -32767, 0]);
        assert_eq!(snorm16_of_dec3n(enc(1, -1, 0)), [64, -64, 0, 0]);
    }

    #[test]
    fn shader_flags_chunk_is_read_as_the_engine_stores_it() {
        // Effects\Media\Shader\ShadowCaster.Shader.Gbx body bytes at 0x15c (chunk 0x09002020 v3)
        let mut body = vec![0u8; 16];
        body.extend_from_slice(&[0x20, 0x20, 0x00, 0x09, 0x03, 0, 0, 0, 0x00, 0x60, 0x00, 0x0c, 0x00, 0xff, 0x08, 0x00, 0, 0, 0, 0, 0, 0, 0x80, 0x3f, 0xff, 0xff, 0xff, 0xff, 0x01, 0x04, 0x03, 0x40, 0x00, 0x09]);
        let f = shader_flags(&body).unwrap();
        assert_eq!(f, ShaderFlags { a: 0x0c006000, b: 0x0008ff00, c: 0, f: 1.0, pass_bits: 0x0401 });
        assert!(f.passes_default_mask() && !f.never_casts());
        // Tech3_Water_MultiH: B carries the never-casts bit
        let mut body = vec![0u8; 4];
        body.extend_from_slice(&[0x20, 0x20, 0x00, 0x09, 0x03, 0, 0, 0, 0x00, 0x68, 0x32, 0x00, 0x20, 0x00, 0x4c, 0x00, 0x00, 0x02, 0, 0, 0, 0, 0, 0, 0xff, 0xff, 0xff, 0xff, 0x41, 0x00, 0x03, 0x40, 0x00, 0x09]);
        let w = shader_flags(&body).unwrap();
        assert_eq!((w.a, w.b, w.c, w.pass_bits), (0x00326800, 0x004c0020, 0x200, 0x0041));
        assert!(w.never_casts());
    }

    #[test]
    fn warpsand_py_texcoord_is_the_captured_15_degree_matrix() {
        // WarpSand_D.Texture.gbx chunk 0x09011025: scale 0.0005, trans 0, DefaultTexCoordRotate 15°
        let t = TexCoordTransform { scale: [0.0005, 0.0005], trans: [0.0, 0.0], rotate_deg: 15.0, colour: 0xff00_0000 };
        let m = t.world_pos_to_texcoord();
        // pwc-day frame 127448 draw 1028, ShaderV.GbxWorldPosToTexCoord_MapPyDiffuse
        assert_eq!([m[0][0].to_bits(), m[0][1].to_bits()], [0x39fd3630, 0x3907b21b]);
        assert_eq!([m[2][0].to_bits(), m[2][1].to_bits()], [0x3907b21b, 0xb9fd3630]);
        assert_eq!(m[1], [0.0, 0.0]);
        assert_eq!(m[3], [0.0, 0.0]);
        // TrackWallPxzInWorld_D (RE 8): 1/32, no rotation → the plain scale with v down
        let w = TexCoordTransform { scale: [1.0 / 32.0, 1.0 / 32.0], trans: [0.0, 0.0], rotate_deg: 0.0, colour: 0xff00_0000 }.world_pos_to_texcoord();
        assert_eq!(w, [[1.0 / 32.0, 0.0], [0.0, 0.0], [0.0, -1.0 / 32.0], [0.0, 0.0]]);
    }

    #[test]
    fn bluebay_roles_and_materials_from_the_pack() {
        let Some(mut store) = island_store("BlueBay") else { return };
        let b = load(&mut store, "BlueBay").unwrap();
        assert_eq!(b.leaves.len(), 10);
        let tris = |role: EnvRole| b.by_role(role).map(|l| l.triangles()).sum::<usize>();
        assert_eq!(tris(EnvRole::SkyDome), 3968);
        assert_eq!(tris(EnvRole::PeelOnly), 1120, "4 × 280 WarpSand quadrants");
        assert_eq!(tris(EnvRole::CasterAndShadow), 262, "ShadowCaster64");
        assert_eq!(tris(EnvRole::Excluded), 336, "4 × 84 Water quadrants");
        assert_eq!(b.peel_leaves().map(|l| l.triangles()).sum::<usize>(), 1382);
        let dome = b.sky_dome().unwrap();
        assert_eq!((dome.positions.len(), dome.indices.len(), dome.uv0.len()), (2143, 11904, 2143));
        assert_eq!(dome.pos, [0.0, 0.0, 0.0]);
        assert!(dome.solid.ends_with("SkyDomeMirror.Solid.Gbx"));
        let sand = b.by_role(EnvRole::PeelOnly).next().unwrap();
        assert_eq!(sand.flags, 0x1e80a);
        assert_eq!(sand.root_flags, 0x1e88a);
        assert_eq!(sand.shader_flags.unwrap(), ShaderFlags { a: 0x0c007800, b: 0x0018fff0, c: 0, f: 1.0, pass_bits: 0x0441 });
        assert_eq!(sand.params, vec![("PxzScaleTrans".to_string(), vec![0.0005, 0.0005, 0.5])]);
        assert_eq!(sand.bitmaps.iter().map(|(n, _)| n.as_str()).collect::<Vec<_>>(), ["PyDiffuse", "PxzDiffuse", "PxzNormal"]);
        let py = sand.texcoord.iter().find(|(n, _)| n == "PyDiffuse").map(|(_, t)| *t).expect("PyDiffuse texcoord transform");
        assert_eq!(py, TexCoordTransform { scale: [0.0005, 0.0005], trans: [0.0, 0.0], rotate_deg: 15.0, colour: 0xff00_0000 });
        let water = b.by_role(EnvRole::Excluded).next().unwrap();
        assert_eq!(water.flags, 0x1a80a, "no IsShadowCaster bit");
        assert!(water.shader_flags.unwrap().never_casts());
        let box_ = b.sun_shadow_leaves().next().unwrap();
        assert_eq!((box_.flags, box_.positions.len(), box_.indices.len()), (0x1e802, 272, 786));
        assert!(!box_.has_custom);
    }

    #[test]
    fn the_other_islands_follow_the_same_rule() {
        for (coll, dome_pos) in [("RedIsland", [1024.0, 0.0, 1024.0]), ("WhiteShore", [1024.0, 0.0, 1024.0]), ("GreenCoast", [0.0, 0.0, 0.0])] {
            let Some(mut store) = island_store(coll) else { return };
            let b = load(&mut store, coll).unwrap();
            let tris = |role: EnvRole| b.by_role(role).map(|l| l.triangles()).sum::<usize>();
            assert_eq!(tris(EnvRole::SkyDome), 3968, "{coll}");
            assert_eq!(b.sky_dome().unwrap().pos, dome_pos, "{coll}");
            assert_eq!(tris(EnvRole::CasterAndShadow), 262, "{coll}");
            assert_eq!(tris(EnvRole::Excluded), 336, "{coll}");
            assert_eq!(b.by_role(EnvRole::PeelOnly).count(), 4, "{coll}: four WarpGround quadrants");
            assert!(b.by_role(EnvRole::PeelOnly).all(|l| l.material_stem == "WarpGround"), "{coll}");
        }
    }

    /// Stadium has no capture: the proof is structural — the Stadium256 layout's one mobil is the SkyDome at
    /// (0, 3000, 0) drawing `SkyDomeDouble.Solid.Gbx`, whose "Snow" tree is a 2143-vertex / 3968-triangle
    /// closed ellipsoid of the SkyDomeMirror radii with u in [−1, 1] and v in [0, 1] (its "Stars" tree is a CPlugVisualSprite
    /// layer: 8952 sprites, not a mesh).
    #[test]
    fn stadium_dome_is_the_double_dome_at_3000() {
        let dir = paks_dir();
        let sp = format!("{dir}/Stadium.pak");
        let mp = format!("{dir}/Maniaplanet.pak");
        if !std::path::Path::new(&sp).exists() || !std::path::Path::new(&mp).exists() {
            eprintln!("skipped: {sp} / {mp} not on this box");
            return;
        }
        let mut store = DataStore::empty();
        store.add_pak(&sp, "B773D73047A4104857722366D78D28A6").unwrap();
        store.add_pak(&mp, "9A93723447347A8CE336CCFC49E65449").unwrap();
        let b = load(&mut store, "Stadium").unwrap();
        assert!(b.layout_path.ends_with("Base16x12.Scene3d.Gbx"));
        assert_eq!(b.leaves.len(), 2);
        assert!(b.leaves.iter().all(|l| l.role() == EnvRole::SkyDome && l.pos == [0.0, 3000.0, 0.0] && l.solid.ends_with("SkyDomeDouble.Solid.Gbx")));
        let dome = b.leaves.iter().find(|l| l.path.last().map(|s| s.as_str()) == Some("Snow")).unwrap();
        assert_eq!((dome.positions.len(), dome.indices.len(), dome.uv0.len(), dome.normals_dec3n.len()), (2143, 11904, 2143, 2143));
        assert!(dome.indices.iter().all(|i| (*i as usize) < 2143));
        // u = the azimuth in [−1, 1], v = the elevation in [0, 1] (the manifest's table; ±1e-5 of float slop)
        assert!(dome.uv0.iter().all(|uv| (-1.00001..=1.00001).contains(&uv[0]) && (-0.00001..=1.00001).contains(&uv[1])), "u in [−1, 1], v in [0, 1]");
        // the ellipsoid: every vertex on the SkyDomeMirror radii (x/z 22265.23, y 9751.16) within 1e-3 relative
        for p in &dome.positions {
            let r = (p[0] / 22265.227).powi(2) + (p[1] / 9751.161).powi(2) + (p[2] / 22265.227).powi(2);
            assert!((r - 1.0).abs() < 2e-3, "vertex {p:?} off the ellipsoid: r² {r}");
        }
        // closed: every edge of the triangle list is shared by exactly two triangles (the poles' degenerate
        // edges excluded: a ring of 65 duplicates the seam vertex, so edges are compared by position)
        let key = |i: u32| { let p = dome.positions[i as usize]; (p[0].to_bits(), p[1].to_bits(), p[2].to_bits()) };
        let mut edges: std::collections::HashMap<((u32, u32, u32), (u32, u32, u32)), u32> = std::collections::HashMap::new();
        for t in dome.indices.chunks_exact(3) {
            for (a, b2) in [(t[0], t[1]), (t[1], t[2]), (t[2], t[0])] {
                let (ka, kb) = (key(a), key(b2));
                if ka == kb { continue; }
                let e = if ka < kb { (ka, kb) } else { (kb, ka) };
                *edges.entry(e).or_insert(0) += 1;
            }
        }
        let open = edges.values().filter(|c| **c != 2).count();
        assert_eq!(open, 0, "{open} of {} edges are not shared by exactly two triangles", edges.len());
        // the world placement
        let w = dome.world_positions();
        let (mut lo, mut hi) = (f32::MAX, f32::MIN);
        for p in &w { lo = lo.min(p[1]); hi = hi.max(p[1]); }
        assert!((lo - (3000.0 - 9751.161)).abs() < 0.01 && (hi - (3000.0 + 9751.161)).abs() < 0.01, "y {lo}..{hi}");
        // NOT the SkyDomeMirror vertex order: a different dome file
        let mut island = island_store("BlueBay").unwrap();
        let mirror = load(&mut island, "BlueBay").unwrap();
        assert_ne!(mirror.sky_dome().unwrap().gpu_vertex_buffer(), dome.gpu_vertex_buffer());
    }

    #[test]
    fn envblock_matches_capture() {
        let Some(mut store) = island_store("BlueBay") else { return };
        let passcap = std::path::PathBuf::from(passcap_dir());
        if !passcap.join("logs").join("mesh-frame127448.json").exists() {
            eprintln!("skipped: {} has no frame 127448 mesh dump", passcap.display());
            return;
        }
        let b = load(&mut store, "BlueBay").unwrap();
        let r = compare_capture(&b, &passcap, 127448).unwrap();
        for l in &r.lines {
            eprintln!("{l}");
        }
        // the seven environment draws of the frame: 377 (sun shadow map) + 1009 (peel) = the box,
        // 1028 / 1033 / 1071 / 1076 = the terrain patches, 1051 = the dome — vertices AND indices
        assert_eq!(r.differ, 0);
        assert!(r.exact >= 7, "{} exact", r.exact);
        for eid in [377u32, 1009, 1028, 1033, 1051, 1071, 1076] {
            assert!(r.lines.iter().any(|l| l.starts_with(&format!("eid {eid} ")) && l.contains("VERTICES EXACT") && l.contains("indices EXACT")), "eid {eid}");
        }
    }
}
