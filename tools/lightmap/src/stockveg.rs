//! STOCK VEGETATION (the NPlugVeget bushes and trees: `<Collection>\Items\Vegetation\*.Item.Gbx` → `.VegetTreeModel.Gbx`)
//! IN THE LIGHTMAPPER — the game's dedicated CHARTLESS tree path, transcribed from the GpuCache programs
//! `Tech3/Trees/{Tree_VertexAddLight_v,p; PeelDepthDiffuse_Tree_v,p; Tree_SelfAO_Shadow_p}` (client-re/shaders-all2; RE 16
//! 2026-09-28 15:45Z / 16:00Z / 17:05Z: the blobs ARE the build's programs — re16_dxbccmp; the tree renderer registers
//! PeelDepthDiffuse_Tree as its pass variant 0xb = the LM peel's "ShaderPeelDepthDiffuse" kind, FUN_14098c390). Port engineer
//! E5, 2026-09-28.
//!
//! WHAT THE PORT PLACED BEFORE: nothing — the stock lookup tried `<Coll>\Items\<name>.Item.Gbx` only; the bushes live in
//! `<Coll>\Items\Vegetation\` and Forest / Grove are `SVariantList`s (tiny03: BushSmallA ×216 + BushSmallB ×87 + Grove 13 +
//! Forest 2 = 318 placements; g23: 1 481 bushes). The layout side (records.rs, the kind-0 legacy tree records, 380/380 bit-exact
//! on tiny03) already resolves them through `tiny_library::find_item_file` + `veget::item_species`; the SCENE side did not.
//!
//! THE THREE PROGRAMS, instruction by instruction (the DXBC text of each blob is the authority):
//!
//! * `Tree_VertexAddLight_v/p` — ONE run per vertex before the sweeps: the VS reads the instance's `g_Buf_AllTreeInstance_TQuats`
//!   pair (entry 2i = the quaternion (x, y, z, w), entry 2i+1 = (T.x, T.y, T.z, SCALE)), `p_w = R(q)·(s·p) + T`, `n_w = R(q)·n`
//!   (the input normal as decoded, NOT normalised — `dp3` with the rotation rows only, no scale); the PS (SCBufferP_TreeVertex_AddLight
//!   {WorldToShadow float4x4, LightDirInWorld, LightRgb}; t0 TMapShadow through SMapShadow, a COMPARISON sampler; u0 = the typed
//!   float4 UAV `ILightInput`): `uvz = (p_w, 1)·WorldToShadow[0..2]; s = sample_c_lz(TMapShadow, uv, ref z); ndl = max(n_w·−LightDirInWorld,
//!   0); ILightInput[LightInputOffset[inst] + SG_Offset + vertex_id] = s·ndl·LightRgb` — exactly PS 15187's arithmetic (the receivers'
//!   direct-sun pass, `sunpass::ps_15187`) with OutScale 1, at the vertex instead of the texel. The port evaluates it with the setup
//!   chain's own D16 sun shadow map and `WorldPw01Shadow` (the same SMapShadow: ClampEdge, comparison LINEAR = 2×2 PCF, GreaterEqual —
//!   logs/samplers-frame127448.json) and the LM's LightRgb.
//!   NOT YET TRANSCRIBED: `Tree_Instance_AddLight_c` (the per-INSTANCE local-light term added to the vertex light: cInst × cLight,
//!   IsAtt_HN2 / InvCosRange / CosOuter … RE 13's attenuation family) — the lamp frame's tree term. The Day cells carry no lamp.
//!
//! * `PeelDepthDiffuse_Tree_v/p` — the tree's draw in every peel (world and fitted): the VS = the same instance transform, `o0 =
//!   WorldPrCamera·p_w`, `o1 = TexCoord0`, `o2 = ILightInput[…]` (the vertex light, interpolated by the rasteriser), `o3 =
//!   g_WorldPw01Shadow0·p_w` (the depth-to-peel lookup); the PS (s0 SGbxWrap_Aniso, s1 SGbxClamp_Point_Cmp; t0 TMapDepthToPeel, t1
//!   TMapBaseColor):
//!   ```text
//!     sample_c_lz r0.x, v3.xy, t0, s1, v3.z ; add r0.x, r0.x, −0.5 ; lt ; discard_nz      — the depth-peel compare (the LM peel's own)
//!     sample r0, v1.xy, t1, s0                                                             — BaseColor at TexCoord0, aniso ×16, WRAP, all mips
//!     add r0.w, r0.w, −0.5 ; mul r1.xyz, r0.xyz, v2.xyz ; lt r0.x, r0.w, 0 ; discard_nz     — ALPHA CUT 0.5 (a literal — not the context's 128/255)
//!     max r1.w, r1.y, 1e-5 ; and o0.xyz, r1.xwz, isfrontface ; mov o0.w, 1                 — rgb = BaseColor × vertex light, G ≥ 1e-5, BACK FACES BLACK
//!   ```
//!   So a stock tree is a chartless EMITTER / OCCLUDER in the peel: front faces BaseColor × its own per-vertex sun, back faces black,
//!   depth-writing; it never reads the ILightInput atlas and is never a receiver (hasPLG 0 → no record; RE 7). The port colours the
//!   fragment in `peel::fragment_radiance` (`peel_colour`) and tests its alpha at the A-buffer sites (`VegetRule::peel_passes`) with
//!   the tree PS's own sampler — the item cards' 128/255 + ClampEdge + AlphaTex machinery is not this program's.
//!
//! * `Tree_SelfAO_Shadow_p` (with Tree_Shadow_v) — the tree as a SUN-MAP caster: blob 0/1 (the opaque materials — the bark) = `ret`
//!   (no test); blobs 2/3 (the leaf materials, DTwk_SkipMap_OpacityIsDiffuseAlpha) = `sample r0.x, v1.xy, t0.w, s0 (SGbxWrap_Bilinear) ;
//!   add r0.x, r0.x, −0.3 ; lt ; discard_nz` — the caster's ALPHA CUT IS 0.3 through a FIXED BILINEAR sampler (MIN_MAG_LINEAR_MIP_POINT,
//!   WRAP: the engine names Bilinear and Trilinear apart — 85 SGbxWrap_Bilinear vs 84 SGbxWrap_Trilinear declarations in the cache),
//!   unlike the items' 128/255 anisotropic ×16 (RE 16 read 3). `VegetRule::sun_passes`; setupmap::shadow_from_map routes the tree
//!   casters through it (leaf materials tested, bark opaque).
//!
//! THE TEXTURE: the material's D image (`WhiteShore\Media\Texture\Image\VegetBushDryLeaf_D.dds`, BC3 2048² 12 mips; the bark
//! `VegetTreeFirBark_D.dds` BC1) read from the pak, its STORED mip chain (RE 16 19:15Z: the loader honours a DDS's chain), rows in
//! FILE order sampled at (u, 1 − v) with the v derivative negated — the loader uploads a DDS bottom-up (D's rule, the item textures'
//! 100 % flipped agreement; setupmap's pre-pass textured path does the same). The rgb is decoded sRGB → linear before filtering
//! (the `_D` colour textures' view; the pre-pass's BaseColor path is verified so against the capture's MDiffuse) —
//! LMTOOL_STOCK_VEGET_SRGB=0 samples the bytes raw (study).
//!
//! THE INSTANCE: the placement's pose through RE 4's chain (`veget_instance::item_pose` → `variation` with the item spawner's
//! rotation draws and the SCALE draw 1 − (k/7)·ScaleVar01 — the game ignores the placement scale of a VegetTreeModel; the record
//! box path uses the same varied quaternion, bit-exact on tiny03's 380 records). The species of a variant-list item (Forest /
//! Grove) = the placement's variant byte into `veget::item_species`.
//!
//! THE LOD GROUP the tree pass draws is OPEN (RE 16: the runtime picks by distance; the LM's ortho cameras have none) — group 0
//! (the nearest, the game's "LOD 0" the pwc-day peel drew for ITEMS) by default; LMTOOL_STOCK_VEGET_LOD=k is the study knob.
//!
//! KNOBS: LMTOOL_STOCK_VEGET=0 places none (the pre-E5 scene); LMTOOL_STOCK_VEGET_LOD=k; LMTOOL_STOCK_VEGET_SRGB=0;
//! LMTOOL_STOCK_VEGET_TRACE=1 prints every species resolved (visuals, vertices, textures) and every unresolved stock item.

use std::sync::Arc;

use crate::geometry::{ModelGeom, Tri, V3};
use crate::texsample::{self, Address, Filter, Sampler, Texture};

/// PeelDepthDiffuse_Tree_p: `add r0.w, r0.w, l(-0.5)` — the peel's alpha cut on every tree material.
pub const PEEL_ALPHA_CUT: f32 = 0.5;
/// Tree_SelfAO_Shadow_p blob 2: `add r0.x, r0.x, l(-0.3)` — the sun-map caster's alpha cut on the leaf materials.
pub const SUN_ALPHA_CUT: f32 = 0.3;

pub fn enabled() -> bool {
    static V: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *V.get_or_init(|| std::env::var("LMTOOL_STOCK_VEGET").map(|v| v != "0").unwrap_or(true))
}

pub fn trace() -> bool {
    static V: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *V.get_or_init(|| std::env::var_os("LMTOOL_STOCK_VEGET_TRACE").is_some())
}

/// The LOD group the tree pass draws (default 0; LMTOOL_STOCK_VEGET_LOD=k).
pub fn lod_group() -> usize {
    static V: std::sync::OnceLock<usize> = std::sync::OnceLock::new();
    *V.get_or_init(|| std::env::var("LMTOOL_STOCK_VEGET_LOD").ok().and_then(|v| v.parse().ok()).unwrap_or(0))
}

fn srgb_view() -> bool {
    static V: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *V.get_or_init(|| std::env::var("LMTOOL_STOCK_VEGET_SRGB").map(|v| v != "0").unwrap_or(true))
}

/// One tree material's D image as the two tree programs sample it.
#[derive(Debug)]
pub struct VegetTex {
    /// The pak's logical path (`WhiteShore\Media\Texture\Image\VegetBushDryLeaf_D.dds`).
    pub path: String,
    /// Rows in FILE order (sampled at (u, 1 − v)); rgb sRGB-decoded unless the study knob says raw.
    pub tex: Texture,
}

impl VegetTex {
    pub fn load(path: &str, bytes: &[u8]) -> Result<VegetTex, String> {
        let mut tex = texsample::parse_dds(bytes, texsample::Bc1Decode::Expand8Round).map_err(|e| format!("{path}: {e}"))?;
        if srgb_view() {
            tex.decode_srgb();
        }
        if trace() {
            let lv = &tex.levels[0][0];
            let (mut n05, mut n03, mut sa, mut sc) = (0usize, 0usize, 0f64, [0f64; 3]);
            let n = (lv.w * lv.h) as usize;
            for y in 0..lv.h { for x in 0..lv.w { let p = lv.get(x, y); if p[3] >= 0.5 { n05 += 1; } if p[3] >= 0.3 { n03 += 1; } sa += p[3] as f64; for c in 0..3 { sc[c] += p[c] as f64; } } }
            eprintln!("stock vegetation: texture {path}: {:?} {}×{} {} levels; level 0 mean alpha {:.3}, a ≥ 0.5 {:.1} %, a ≥ 0.3 {:.1} %, mean rgb ({:.3}, {:.3}, {:.3}){}", tex.fmt, tex.w, tex.h, tex.levels[0].len(), sa / n as f64, 100.0 * n05 as f64 / n as f64, 100.0 * n03 as f64 / n as f64, sc[0] / n as f64, sc[1] / n as f64, sc[2] / n as f64, if srgb_view() { " (sRGB-decoded)" } else { " (raw)" });
            // the STORED chain's coverage per level: does the authoring tool keep the alpha coverage (a boosted chain) or average it away?
            let per_level: Vec<String> = tex.levels[0].iter().enumerate().map(|(l, lv)| { let n = (lv.w * lv.h) as usize; let (mut c5, mut c3, mut s) = (0usize, 0usize, 0f64); for y in 0..lv.h { for x in 0..lv.w { let a = lv.get(x, y)[3]; if a >= 0.5 { c5 += 1; } if a >= 0.3 { c3 += 1; } s += a as f64; } } format!("L{l} {}² mean {:.3} ≥0.5 {:.1}% ≥0.3 {:.1}%", lv.w, s / n as f64, 100.0 * c5 as f64 / n as f64, 100.0 * c3 as f64 / n as f64) }).collect();
            eprintln!("stock vegetation: texture {path}: stored chain: {}", per_level.join(" | "));
        }
        Ok(VegetTex { path: path.to_string(), tex })
    }
    pub fn w(&self) -> usize {
        self.tex.w as usize
    }
    pub fn h(&self) -> usize {
        self.tex.h as usize
    }
    /// SGbxWrap_Aniso: anisotropic ×16, Wrap, mip linear, no bias.
    fn peel_sampler() -> Sampler {
        Sampler { min_mag: Filter::Linear, mip: Some(Filter::Linear), max_aniso: 16, address_u: Address::Wrap, address_v: Address::Wrap, lod_bias: 0.0, min_lod: 0.0, max_lod: f32::MAX, weight_bits: Some(8) }
    }
    /// SGbxWrap_Bilinear: MIN_MAG_LINEAR_MIP_POINT, Wrap.
    fn sun_sampler() -> Sampler {
        Sampler { min_mag: Filter::Linear, mip: Some(Filter::Point), max_aniso: 1, address_u: Address::Wrap, address_v: Address::Wrap, lod_bias: 0.0, min_lod: 0.0, max_lod: f32::MAX, weight_bits: Some(8) }
    }
    /// `sample(TMapBaseColor, uv)` of PeelDepthDiffuse_Tree_p: (rgb linear, alpha) at TexCoord0 `uv` with the per-pixel
    /// derivatives `ddx`, `ddy` (texture units per peel pixel).
    #[inline]
    pub fn sample_peel(&self, uv: [f32; 2], ddx: [f32; 2], ddy: [f32; 2]) -> [f32; 4] {
        texsample::sample(&self.tex, 0, &Self::peel_sampler(), [uv[0], 1.0 - uv[1]], [ddx[0], -ddx[1]], [ddy[0], -ddy[1]])
    }
    /// `sample(TMapBaseColor_Gbx_Opacity, uv).w` of Tree_SelfAO_Shadow_p.
    #[inline]
    pub fn sample_sun_alpha(&self, uv: [f32; 2], ddx: [f32; 2], ddy: [f32; 2]) -> f32 {
        texsample::sample(&self.tex, 0, &Self::sun_sampler(), [uv[0], 1.0 - uv[1]], [ddx[0], -ddx[1]], [ddy[0], -ddy[1]])[3]
    }
}

/// The alpha rule of one tree material in the two passes (carried by the scene's `AlphaMask.veget`).
#[derive(Debug)]
pub struct VegetRule {
    pub tex: Arc<VegetTex>,
    /// A `*_Leaf` material (VegetMaterial::leaf): the sun-map caster is alpha-tested at 0.3; a bark material casts opaque.
    pub leaf: bool,
}

/// LMTOOL_STOCK_VEGET_TRACE=1: the peel's tree fragments [tested, passed the 0.5 cut] and their isotropic LOD histogram.
pub static PEEL_STATS: [std::sync::atomic::AtomicU64; 2] = [std::sync::atomic::AtomicU64::new(0), std::sync::atomic::AtomicU64::new(0)];
pub static PEEL_LOD: [std::sync::atomic::AtomicU64; 16] = [const { std::sync::atomic::AtomicU64::new(0) }; 16];

/// The census line (printed by the bake when the trace is on and any tree fragment was tested).
pub fn peel_stats_line() -> Option<String> {
    let tested = PEEL_STATS[0].load(std::sync::atomic::Ordering::Relaxed);
    if tested == 0 { return None; }
    let passed = PEEL_STATS[1].load(std::sync::atomic::Ordering::Relaxed);
    let hist: Vec<String> = PEEL_LOD.iter().enumerate().filter_map(|(l, c)| { let n = c.load(std::sync::atomic::Ordering::Relaxed); (n > 0).then(|| format!("lod {l}: {n}")) }).collect();
    Some(format!("stock vegetation: peel alpha test (BaseColor.a ≥ 0.5, aniso-16 wrap): {tested} tree fragments tested, {passed} passed ({:.2} %); isotropic LOD of the footprint: {}", 100.0 * passed as f64 / tested as f64, hist.join(", ")))
}

impl VegetRule {
    /// The triangle's TexCoord0 footprint per peel pixel (the same derivative form the item cards use).
    pub fn footprint(&self, px: [[f32; 2]; 3], uv0: [[f32; 2]; 3]) -> crate::alphatex::Footprint {
        crate::alphatex::Footprint::of_triangle(px, uv0, self.tex.w(), self.tex.h())
    }
    #[inline]
    fn derivs(fp: &crate::alphatex::Footprint) -> ([f32; 2], [f32; 2]) {
        ([fp.dx[0] / fp.w, fp.dx[1] / fp.h], [fp.dy[0] / fp.w, fp.dy[1] / fp.h])
    }
    /// The peel's alpha test: BaseColor.a − 0.5 ≥ 0 keeps the fragment.
    #[inline]
    pub fn peel_passes(&self, u: f32, v: f32, fp: &crate::alphatex::Footprint) -> bool {
        let (ddx, ddy) = Self::derivs(fp);
        let pass = self.tex.sample_peel([u, v], ddx, ddy)[3] - PEEL_ALPHA_CUT >= 0.0;
        if trace() {
            PEEL_STATS[0].fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            if pass { PEEL_STATS[1].fetch_add(1, std::sync::atomic::Ordering::Relaxed); }
            // the LOD the sample took (isotropic form of the footprint, for the census): log2 of the longer derivative in texels
            let l = (fp.dx[0] * fp.dx[0] + fp.dx[1] * fp.dx[1]).max(fp.dy[0] * fp.dy[0] + fp.dy[1] * fp.dy[1]).sqrt().max(1e-30).log2().clamp(0.0, 15.0) as usize;
            PEEL_LOD[l].fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        }
        pass
    }
    /// The peel's colour sample (rgb, a) at the fragment.
    #[inline]
    pub fn peel_sample(&self, u: f32, v: f32, fp: &crate::alphatex::Footprint) -> [f32; 4] {
        let (ddx, ddy) = Self::derivs(fp);
        self.tex.sample_peel([u, v], ddx, ddy)
    }
    /// The sun-map caster's test: a leaf material keeps the fragment iff BaseColor.a − 0.3 ≥ 0; bark always.
    #[inline]
    pub fn sun_passes(&self, uv: [f32; 2], ddx: [f32; 2], ddy: [f32; 2]) -> bool {
        if !self.leaf {
            return true;
        }
        self.tex.sample_sun_alpha(uv, ddx, ddy) - SUN_ALPHA_CUT >= 0.0
    }
}

/// The per-vertex data of a species' drawn LOD group (model space), for the vertex light.
#[derive(Debug)]
pub struct VegetModel {
    pub species: String,
    pub lod: usize,
    /// (position, normal as the stream stores it — Dec3N fields / 511 unnormalised, or the float3).
    pub verts: Vec<(V3, V3)>,
    /// Per `ModelGeom.tris` entry, its three vertex indices into `verts`.
    pub tri_verts: Vec<[u32; 3]>,
    /// Per material slot of the ModelGeom's `alpha_tex`: the rule.
    pub rules: Vec<Arc<VegetRule>>,
    /// The model-space y above which the top-most 10 % of the vertices lie (a diagnostic split of the shadow test).
    pub top90: f32,
}

/// The scene name of a tree material's mask (never collides with an embedded item texture's file name).
pub fn mask_name(image_path: &str, leaf: bool) -> String {
    format!("veget:{}{}", if leaf { "leaf:" } else { "bark:" }, image_path.rsplit(['/', '\\']).next().unwrap_or(image_path))
}

/// The species list of a placed stock item, or None when the item is not a vegetation item (or is not in the packs): only the
/// `.VegetTreeModel.Gbx` members of the item's species list (a variant-list item of PREFABS — Flag8m, the screens — is not one).
pub fn species_of(store: &mut mapgeom::store::DataStore, model_name: &str) -> Option<Vec<String>> {
    let file = mapgeom::tiny_library::find_item_file(store, model_name)?;
    let list: Vec<String> = mapgeom::veget::item_species(store, &file).ok()?.into_iter().filter(|p| p.to_ascii_lowercase().ends_with(".vegettreemodel.gbx")).collect();
    if list.is_empty() { None } else { Some(list) }
}

/// The species a placement draws: its variant byte into the list, the first when out of range (records.rs's rule).
pub fn species_for(list: &[String], variant: u8) -> Option<&String> {
    list.get(variant as usize).or_else(|| list.first())
}

/// The ModelGeom of one species' drawn LOD group: chartless triangles (no LM uv, no material link) whose `alpha` indexes the
/// model's `alpha_tex` = the tree materials' mask names; `veget` carries the vertex data and the rules. `textures` caches the
/// loaded D images by pak path across species.
pub fn build_model(store: &mut mapgeom::store::DataStore, species: &str, textures: &mut std::collections::BTreeMap<String, Arc<VegetTex>>) -> Result<ModelGeom, String> {
    use mapgeom::static_item::vstream::{Elem, N_NORMAL, N_POSITION, N_TEXCOORD0, T_DEC3N, T_FLOAT3};
    let m = mapgeom::veget::parse_tree_model(store, species)?;
    let lod = lod_group().min(m.lods.len().saturating_sub(1));
    let mut g = ModelGeom::default();
    let mut verts: Vec<(V3, V3)> = Vec::new();
    let mut tri_verts: Vec<[u32; 3]> = Vec::new();
    let mut rules: Vec<Arc<VegetRule>> = Vec::new();
    // per material slot: the mask index (the D image loaded once)
    let mut slot_of: std::collections::HashMap<u16, u16> = Default::default();
    let mut n_vis = 0usize;
    for e in &m.lods[lod] {
        let Some(mat) = m.materials.get(e.material as usize) else { continue };
        let Some(image) = mat.images[0].as_deref() else { return Err(format!("{species}: material {} has no D image", mat.name)) };
        let slot: u16 = match slot_of.get(&e.material) {
            Some(&s) => s,
            None => {
                let tex = match textures.get(image) {
                    Some(t) => t.clone(),
                    None => {
                        let bytes = store.read(image).map_err(|err| format!("{species}: {image}: {err}"))?;
                        let t = Arc::new(VegetTex::load(image, &bytes)?);
                        textures.insert(image.to_string(), t.clone());
                        t
                    }
                };
                rules.push(Arc::new(VegetRule { tex, leaf: mat.leaf }));
                g.alpha_tex.push(mask_name(image, mat.leaf));
                let s = (rules.len() - 1) as u16;
                slot_of.insert(e.material, s);
                s
            }
        };
        let Some(st) = e.visual.stream() else { continue };
        let Some(ib) = e.visual.index_buffer.as_ref() else { continue };
        let compress = st.compress_local3d.unwrap_or(false);
        let mut pos: Option<&Vec<[f32; 3]>> = None;
        let mut nrm: Option<Vec<[f32; 3]>> = None;
        let mut uv0: Option<&Vec<[f32; 2]>> = None;
        for (d, el) in st.decls.iter().zip(st.elems.iter()) {
            match (d.name(), d.stored_type(compress), el) {
                (N_POSITION, T_FLOAT3, Elem::Float3(p)) => pos = Some(p),
                (N_NORMAL, T_DEC3N, Elem::Word(w)) => nrm = Some(w.iter().map(|x| mapgeom::static_item::build::dec3n_unpack(*x)).collect()),
                (N_NORMAL, T_FLOAT3, Elem::Float3(p)) => nrm = Some(p.clone()),
                (N_TEXCOORD0, _, Elem::Float2(u)) => uv0 = Some(u),
                _ => {}
            }
        }
        let (Some(pos), Some(nrm), Some(uv0)) = (pos, nrm, uv0) else {
            return Err(format!("{species}: level {lod} visual {} lacks a float3 position / normal / float2 TexCoord0 stream", n_vis));
        };
        if nrm.len() != pos.len() || uv0.len() != pos.len() {
            return Err(format!("{species}: level {lod} visual {}: {} positions, {} normals, {} uv0", n_vis, pos.len(), nrm.len(), uv0.len()));
        }
        let base = verts.len() as u32;
        for i in 0..pos.len() {
            verts.push((pos[i], nrm[i]));
        }
        for t in ib.indices.chunks_exact(3) {
            let (a, b, c) = (t[0] as usize, t[1] as usize, t[2] as usize);
            if a >= pos.len() || b >= pos.len() || c >= pos.len() {
                continue;
            }
            g.tris.push(Tri { p: [pos[a], pos[b], pos[c]], n: [nrm[a], nrm[b], nrm[c]], uv: [[0.0; 2]; 3], uv0: [uv0[a], uv0[b], uv0[c]], mat: u16::MAX, alpha: slot, diff: u16::MAX });
            tri_verts.push([base + a as u32, base + b as u32, base + c as u32]);
        }
        n_vis += 1;
    }
    if g.tris.is_empty() {
        return Err(format!("{species}: LOD group {lod} draws no triangle"));
    }
    if trace() {
        eprintln!("stock vegetation: {species}: LOD group {lod} of {}: {n_vis} visuals, {} vertices, {} triangles, materials {}", m.lods.len(), verts.len(), g.tris.len(), rules.iter().map(|r| format!("{}{}", r.tex.path.rsplit('\\').next().unwrap_or(""), if r.leaf { " (leaf)" } else { " (bark)" })).collect::<Vec<_>>().join(", "));
    }
    g.veget = Some(Arc::new(VegetModel { species: species.to_string(), lod, top90: { let mut ys: Vec<f32> = verts.iter().map(|v| v.0[1]).collect(); ys.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal)); ys.get(ys.len() * 9 / 10).copied().unwrap_or(f32::MAX) }, verts, tri_verts, rules }));
    Ok(g)
}

/// The drawn instance's transform: the placement through `veget_instance::item_pose` + `variation` (the item spawner's rotation
/// draws + the scale draw), as one Xform (columns = R·s, translation = the varied pose's position).
pub fn instance_xform(it: &tmmaps::map::ItemRec, params: mapgeom::veget_instance::TreeParams) -> (mapgeom::geom::Xform, [f32; 4], [f32; 3], f32) {
    let (q1, t, seed) = mapgeom::veget_instance::item_pose(it.yaw, it.pitch, it.roll, it.pos, it.pivot);
    let inst = mapgeom::veget_instance::variation(q1, t, seed, params, true);
    // `Instance.quat` is (w, x, y, z); quat_to_mat gives the row-major 3×3 with p' = M·p
    let m9 = mapgeom::veget_instance::quat_to_mat(inst.quat);
    let s = inst.scale;
    let mut xf = mapgeom::geom::IDENTITY;
    for c in 0..3 {
        for r in 0..3 {
            xf[3 * c + r] = m9[3 * r + c] * s;
        }
    }
    xf[9] = inst.pos[0];
    xf[10] = inst.pos[1];
    xf[11] = inst.pos[2];
    // the TQuats quaternion in (x, y, z, w) order, as the buffer stores it
    (xf, [inst.quat[1], inst.quat[2], inst.quat[3], inst.quat[0]], inst.pos, s)
}

/// `R(q)·n` for a (x, y, z, w) quaternion — Tree_VertexAddLight_v's rotation rows (r2 / r7 / r5 of the DXBC), no normalisation.
#[inline]
pub fn rotate(q: [f32; 4], n: V3) -> V3 {
    let (x, y, z, w) = (q[0], q[1], q[2], q[3]);
    let r00 = 1.0 - 2.0 * y * y - 2.0 * z * z;
    let r01 = 2.0 * x * y - 2.0 * w * z;
    let r02 = 2.0 * x * z + 2.0 * w * y;
    let r10 = 2.0 * x * y + 2.0 * w * z;
    let r11 = 1.0 - 2.0 * x * x - 2.0 * z * z;
    let r12 = 2.0 * y * z - 2.0 * w * x;
    let r20 = 2.0 * x * z - 2.0 * w * y;
    let r21 = 2.0 * y * z + 2.0 * w * x;
    let r22 = 1.0 - 2.0 * x * x - 2.0 * y * y;
    [r00 * n[0] + r01 * n[1] + r02 * n[2], r10 * n[0] + r11 * n[1] + r12 * n[2], r20 * n[0] + r21 * n[1] + r22 * n[2]]
}

/// The per-instance pose the tree programs read (g_Buf_AllTreeInstance_TQuats): quaternion (x, y, z, w), translation, scale.
#[derive(Clone, Copy, Debug)]
pub struct VegetPose {
    pub q: [f32; 4],
    pub t: [f32; 3],
    pub scale: f32,
}

/// The per-vertex sun light of every stock vegetation instance (Tree_VertexAddLight, run once per bake): indexed by scene
/// instance; None for an instance that is not a tree.
#[derive(Debug)]
pub struct VegetLights {
    pub per_inst: Vec<Option<Arc<Vec<[f32; 3]>>>>,
}

impl VegetLights {
    pub fn count(&self) -> usize {
        self.per_inst.iter().flatten().count()
    }
}

/// Tree_VertexAddLight for every tree vertex of the scene: `p_w = R·(s·p) + T`, `n_w = R·n`; `uvz = (p_w, 1)·WorldToShadow`;
/// `s = sample_c_lz(shadow, uv, z)` (2×2 PCF GreaterEqual, ClampEdge — the receivers' SMapShadow); `L = s · max(n_w·−dir, 0) ·
/// LightRgb`. `pw01` = the setup chain's WorldPw01Shadow (the sun camera's), `shadow` its D16 map as f32.
pub fn vertex_lights(scene: &crate::geometry::Scene, pw01: &[[f32; 4]; 4], dir_in_world: [f32; 3], light_rgb: [f32; 3], shadow: &crate::passdiff::Buf) -> VegetLights {
    let sm = crate::sunpass::ShadowMap { depth: shadow };
    let mut per_inst: Vec<Option<Arc<Vec<[f32; 3]>>>> = vec![None; scene.instances.len()];
    let (mut n_inst, mut n_vert, mut n_lit, mut n_s, mut n_ndl) = (0usize, 0usize, 0usize, 0usize, 0usize);
    let (mut n_top, mut n_top_s) = (0usize, 0usize);
    let mut sum = [0f64; 3];
    let mut shown = 0usize;
    for (ii, inst) in scene.instances.iter().enumerate() {
        let m = &scene.models[inst.model];
        let Some(vm) = m.veget.as_ref() else { continue };
        let Some(pose) = scene.veget_poses.get(ii).copied().flatten() else { continue };
        let mut out = Vec::with_capacity(vm.verts.len());
        for (vi, &(p, n)) in vm.verts.iter().enumerate() {
            // r3 = s·p ; p_w = R·r3 + T (the VS's dp4 rows); n_w = R·n
            let sp = [p[0] * pose.scale, p[1] * pose.scale, p[2] * pose.scale];
            let r = rotate(pose.q, sp);
            let pw = [r[0] + pose.t[0], r[1] + pose.t[1], r[2] + pose.t[2]];
            let nw = rotate(pose.q, n);
            // PS: dp4 with the float4x4's registers 0..2 (column_major: register k = column k, the translation in row 3)
            let u = pw[0] * pw01[0][0] + pw[1] * pw01[1][0] + pw[2] * pw01[2][0] + pw01[3][0];
            let v = pw[0] * pw01[0][1] + pw[1] * pw01[1][1] + pw[2] * pw01[2][1] + pw01[3][1];
            let z = pw[0] * pw01[0][2] + pw[1] * pw01[1][2] + pw[2] * pw01[2][2] + pw01[3][2];
            let s = sm.sample_cmp_linear_ge(u, v, z);
            let ndl = (nw[0] * -dir_in_world[0] + nw[1] * -dir_in_world[1] + nw[2] * -dir_in_world[2]).max(0.0);
            let k = ndl * s;
            let l = [k * light_rgb[0], k * light_rgb[1], k * light_rgb[2]];
            if k > 0.0 { n_lit += 1; }
            if s > 0.0 { n_s += 1; }
            if ndl > 0.0 { n_ndl += 1; }
            // the shadow test by height within the instance (the top of the bush should see the sun): the top-most 10 % of the
            // model's vertices counted apart
            if trace() { if p[1] >= vm.top90 { n_top += 1; if s > 0.0 { n_top_s += 1; } } }
            if trace() && shown < 6 && vi % 97 == 0 {
                eprintln!("stock vegetation: inst {ii} {} vertex {vi}: model p ({:.3}, {:.3}, {:.3}) n ({:.3}, {:.3}, {:.3}) → world ({:.2}, {:.2}, {:.2}) n_w ({:.3}, {:.3}, {:.3}); shadow uv ({:.4}, {:.4}) z {:.5} → s {s} ndl {ndl:.3}", vm.species.rsplit('\\').next().unwrap_or(""), p[0], p[1], p[2], n[0], n[1], n[2], pw[0], pw[1], pw[2], nw[0], nw[1], nw[2], u, v, z);
                shown += 1;
            }
            for c in 0..3 { sum[c] += l[c] as f64; }
            out.push(l);
        }
        n_vert += out.len();
        n_inst += 1;
        per_inst[ii] = Some(Arc::new(out));
    }
    if n_inst > 0 {
        eprintln!("stock vegetation: Tree_VertexAddLight over {n_inst} instances, {n_vert} vertices: {n_lit} sun-lit ({:.1} %; shadow test passed {:.1} %, n·L > 0 {:.1} %{}), mean light ({:.4}, {:.4}, {:.4})", 100.0 * n_lit as f64 / n_vert.max(1) as f64, 100.0 * n_s as f64 / n_vert.max(1) as f64, 100.0 * n_ndl as f64 / n_vert.max(1) as f64, if n_top > 0 { format!("; the top-most 10 % of each model's vertices: {n_top}, shadow test passed {:.1} %", 100.0 * n_top_s as f64 / n_top as f64) } else { String::new() }, sum[0] / n_vert.max(1) as f64, sum[1] / n_vert.max(1) as f64, sum[2] / n_vert.max(1) as f64);
    }
    VegetLights { per_inst }
}

/// PeelDepthDiffuse_Tree_p's colour for one accepted fragment: `BaseColor(uv0).rgb × the interpolated vertex light`, G ≥ 1e-5
/// (the front test is the caller's — `and isfrontface`). `bary` = the fragment's barycentrics on the model triangle's (v0, v1, v2);
/// `px` = the triangle's projected pixel positions for the footprint.
#[inline]
pub fn peel_colour(vm: &VegetModel, lights: &[[f32; 3]], t: &Tri, mtri: usize, bary: [f32; 3], px: [[f32; 2]; 3]) -> [f32; 3] {
    let rule = &vm.rules[t.alpha as usize];
    let fp = rule.footprint(px, t.uv0);
    let u = t.uv0[0][0] * bary[0] + t.uv0[1][0] * bary[1] + t.uv0[2][0] * bary[2];
    let v = t.uv0[0][1] * bary[0] + t.uv0[1][1] * bary[1] + t.uv0[2][1] * bary[2];
    let c = rule.peel_sample(u, v, &fp);
    let vi = vm.tri_verts[mtri];
    let l = |k: usize| lights.get(vi[k] as usize).copied().unwrap_or([0.0; 3]);
    let (l0, l1, l2) = (l(0), l(1), l(2));
    let light = [l0[0] * bary[0] + l1[0] * bary[1] + l2[0] * bary[2], l0[1] * bary[0] + l1[1] * bary[1] + l2[1] * bary[2], l0[2] * bary[0] + l1[2] * bary[1] + l2[2] * bary[2]];
    // mul r1.xyz, r0.xyz, v2.xyz ; max r1.w, r1.y, 1e-5 ; o0 = (r1.x, r1.w, r1.z)
    [c[0] * light[0], (c[1] * light[1]).max(0.00001), c[2] * light[2]]
}
