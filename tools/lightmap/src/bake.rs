//! The baker: sky + sun (+ one bounce) irradiance per lightmap texel of every
//! item, from the map's own geometry (`geometry.rs`) through a BVH
//! (`bvh.rs`). Output is HDR per chart; `synth` packs and encodes it.

use crate::bvh::{Bvh, WTri};
use crate::geometry::{add, cross, dot, mul, norm, sub, xf_normal, xf_point, Scene, V3};

#[derive(Clone, Debug)]
pub struct BakeParams {
    /// Irradiance of an unoccluded, up-facing surface from the sky alone.
    pub sky: [f32; 3],
    /// A constant ambient floor added everywhere (the fitted fill of Nadeo's bakes).
    pub ambient: [f32; 3],
    /// An "upness" term × (0.5 + 0.5·n.y): the fitted zenith-weighted part of the sky.
    pub up: [f32; 3],
    /// Sun irradiance at normal incidence.
    pub sun: [f32; 3],
    /// Unit vector towards the sun.
    pub sun_dir: V3,
    /// Angular radius of the sun disc (radians) for soft shadows.
    pub sun_radius: f32,
    pub sky_samples: usize,
    pub sun_samples: usize,
    /// Layout texels (2048 space) per metre; Nadeo ≈ 1.1 on the tiny maps.
    pub texels_per_m: f32,
    /// Pixel cap per chart side (1024-space pixels).
    pub max_px: u32,
    pub min_px: u32,
    /// The sea / ground plane: rays going below it are blocked.
    pub ground_y: f32,
    /// The rasterised dome peel (`crate::peel`) instead of the ray-cast sphere sweep: the sub-samples
    /// per axis of the atlas raster, the peel target size (pixels per side) and the depth bias (metres)
    /// that keeps a texel's own surface from occluding it.
    pub raster_peel: bool,
    pub ss: u32,
    pub peel_res: u32,
    pub peel_bias: f32,
    /// Per-material albedo (`crate::albedo`) is used for a hit surface unless `flat_albedo` — then
    /// `albedo` applies to every surface (an explicit `--albedo`).
    pub flat_albedo: bool,
    /// The cut-out masks the world triangles' `alpha` index (alpha-tested materials: the baked vegetation
    /// cards) — the peel and the sun shadow map skip their transparent texels like the GPU's alpha test.
    pub alpha_masks: std::sync::Arc<Vec<crate::geometry::AlphaMask>>,
    /// The decoration's stand-in lightmap: LAmbient × this (its ILightInput's C0 in the peel).
    pub decor_ambient: f32,
    /// The mood's LAmbient (kept whatever the receivers' ambient term is).
    pub l_ambient: [f32; 3],
    /// The open-sky irradiance of an up-facing surface (computed by the peel when zero).
    pub decor_sky_up: [f32; 3],
    /// The water surfaces' sky reflectance in the peel (0 = none).
    pub water_reflect: f32,
    /// The sun's specular glitter on water: radiance += K · cos^P(r·sun) · LDirSun.
    pub water_sun: f32,
    pub water_sun_pow: f32,
    /// Vegetation cards take the sun on one side only (default: both sides).
    pub card_one_sided: bool,
    /// One-bounce factor (0 = off) and the average albedo it uses.
    pub bounce: f32,
    pub albedo: f32,
    /// Flip the chart's V axis (uv v = 0 at the bottom row instead of the top).
    pub flip_v: bool,
    /// Map the model's uv1 BOUNDS (PreLightGen u04) to the chart instead of [0,1]².
    pub uv_bounds: bool,
    /// Sky model: 0 = uniform hemisphere, 1 = horizon-darkened (cos-weighted zenith brightening).
    pub sky_model: u32,
    pub threads: usize,
    /// Which third regressor the component fit uses: 0 bounce estimate, 1 upness (0.5+0.5·n.y), 2 skyVis².
    pub fit_regressor: u32,
    /// Pixels of chart border the uv square is inset by on each side (0 = the uv square fills the chart).
    pub inset_px: f32,
    /// Compute the bounce estimate even when `bounce` is 0 (for the component fit).
    pub want_bounce: bool,
    /// Debug: paint texels by world position (a 4 m checkerboard) instead of lighting.
    pub pattern: bool,
    /// With `pattern`: a neutral (white) checker — the hue comes from the caller
    /// (the `--base-candidates` test paints one hue per candidate object base).
    pub pattern_flat: bool,
    /// Point-light scale for frame 1 (K = 1 units per unit light intensity); 0 = frame 1 not baked.
    pub light_k: f32,
    /// The mood's LAmbient: E += LAmbient·(0.8 + 0.2·n.y), unoccluded (the game's ambient pass).
    pub ambient_la: [f32; 3],
    /// 1 = the direct sun is baked (the pre-2026-09-23 model); 0 = the sun only feeds the bounce
    /// (the game: the sun is real-time, the lightmap is diffuse AMBIENT — RE child, disassembly).
    pub direct_sun: f32,
    /// Multiply the ambient term by the dome visibility (the editor's enclosed texels go to ~0).
    pub ambient_ao: bool,
    /// The dome model (the editor's, measured 2026-09-23): the sky is a cosine-weighted cone of
    /// this half-angle around the ZENITH (a 16 m roof 15.8 m up shadows a pad like a 30–32° cone,
    /// a north wall reads 0.15 of a floor like a 40° one; 35° is the compromise), unoccluded
    /// directions in it add `sky`; every direction of a sphere set that
    /// hits a surface adds that surface's bounced radiance. 90 = the plain hemisphere model.
    pub dome_deg: f32,
    /// Effective albedo × BounceFactor of the sea/ground plane (undersides read 0.37 of an open floor).
    pub ground_bounce: f32,
    /// The game's own dome directions (the sphere-table points inside the cone); stratified random when empty.
    pub dome_dirs: std::sync::Arc<Vec<[f32; 3]>>,
    /// Bounce over the full sphere of directions (the game's ComputeBounces_SpherePoints) instead of
    /// the ±dome cone; weight |n·D|/π per direction (radiance → irradiance).
    pub bounce_sphere: bool,
    /// The previous iteration's lightmap (multi-bounce): a hit surface's radiance = albedo ×
    /// (its stored value + its direct sun) instead of the one-bounce estimate.
    pub field: Option<std::sync::Arc<RadianceField>>,
    /// The game's peel model (RE child 2): the 256-point full-sphere set, `Scale = 4/N`, per
    /// direction the FIRST surface — the sky (upward, unoccluded), a lit surface's bounce
    /// (albedo·(E_prev/bounce_decode + direct sun)), or the ground/sea below.
    pub peel: bool,
    /// The sphere directions of the peel model (empty → stratified random over the sphere).
    pub sphere_dirs: std::sync::Arc<Vec<[f32; 3]>>,
    /// The bounce read-back divisor (RE child 2 (d): the lightmap so far is decoded /BounceFactor).
    pub bounce_decode: f32,
    /// The rendered sky as the dome radiance (Tech3/Sky_p: the mood's SkyColor gradient + glow lobes).
    pub sky_grad: Option<std::sync::Arc<crate::skygrad::SkyGradient>>,
    /// A stand-in for the decoration terrain around the map until its geometry is in the BVH: every
    /// unoccluded direction below this elevation (degrees) sees a surface of `horizon_radiance` instead
    /// of the sky (0 = off).
    pub horizon_el: f32,
    pub horizon_radiance: [f32; 3],
    /// The mood's HDR sky as a light source: every unoccluded cosine-sampled ray adds
    /// `sky_cube_scale × L(ω)`; with a cube the constant `sky` term is not used.
    pub sky_cube: Option<std::sync::Arc<crate::skycube::CubeMap>>,
    pub sky_cube_scale: f32,
    /// The differential harness (`--dump-passes`): every intermediate written at the game's points.
    pub dump: Option<std::sync::Arc<std::sync::Mutex<crate::passdump::PassDump>>>,
    /// Which sweep this call bakes (0 = the sky sweep) — the dump's `sweep` field.
    pub sweep: u32,
    /// The game's peel semantics in the gather (`--game-peel`): depth layers peeled far-to-near with the
    /// D3D rasteriser bias, the texel's POINT lookup with the one-texel inset, the layer's pixel colour
    /// (not the exact hit point's), a synthetic dome layer 0; off = the port's A-buffer ray walk.
    pub game_peel: bool,
    /// Gather at every ss² sub-sample of a texel (the game's supersampled raster) instead of once at the
    /// texel's centroid; the resolve box-averages the covered sub-samples either way.
    pub per_subsample: bool,
    /// Per-direction PEELS to rasterise (a captured MANIFEST's frustums), indexed like `sphere_dirs`: the
    /// game runs two per direction — the whole-scene frustum, then one fitted to the items — and the
    /// accumulate takes the later peel's layer wherever it has one (last write wins).
    pub frustums: Option<std::sync::Arc<Vec<Vec<crate::passdump::Frustum>>>>,
    /// The sun shadow map's frustum, when captured.
    pub shadow_frustum: Option<crate::passdump::Frustum>,
    /// The storage formats the game's targets quantise to: the peel colour, TMapILightDir, the
    /// accumulation target; and the float→small-float rounding rule.
    pub quant_peel: crate::gpufmt::Quant,
    pub quant_ilightdir: crate::gpufmt::Quant,
    pub quant_accum: crate::gpufmt::Quant,
    pub rounding: crate::gpufmt::Rounding,
    /// The D3D rasteriser depth bias of the peel layers (DepthBias, SlopeScaledDepthBias) on a D32 target.
    pub depth_bias: (i32, f32),
    /// The one-texel inset of the depth lookup (`u = 0.5 + (u − 0.5)·(w − 2)/w`).
    pub peel_inset: bool,
    /// Layer 0 = the sky dome (the mood's dome mesh peels as the farthest layer).
    pub dome_layer: bool,
    /// The object id of item 0 in the game's mapping (`base`): the dump's chart references.
    pub obj_base: u32,
    /// D3D DepthClipEnable on the peel layers: true drops fragments beyond the frustum's far plane, false
    /// (pancaking) clamps them onto it.
    pub depth_clip: bool,
    /// The peel depth target's bits: 16 (D16_UNORM, the capture) or 32 (D32_FLOAT).
    pub depth_bits: u32,
    /// The accumulate: the game's H-basis constant-term projection (4π/N)·P(n·D) (true) or the RNM-style
    /// 4/N·max(0, n·D); `hbasis_kappa` scales the H-basis C0 into the port's E units (1/√(2π) keeps a
    /// uniform sky at E = L; 1.0 = the raw C0 the game's target holds).
    pub accum_hbasis: bool,
    pub hbasis_kappa: f32,
    /// The sun on the peeled surfaces in the FIRST sweep (the RE reading); the capture shows black peel
    /// colours in sweep 0, so it is off by default.
    pub sweep0_sun: bool,
    /// Directions (indices in the sweep's issue order) after which the accumulation target is dumped.
    pub lightsum_after: std::collections::BTreeSet<u32>,
    /// The transcribed sky dome per peel pixel (the ellipsoid at the world origin, the mesh's uv, the
    /// game's VS/PS) instead of one sky colour per direction.
    pub dome_exact: bool,
    /// The game's per-direction LM raster jitter: direction k (issue order) rasterises the lightmap quad
    /// shifted by `jitter_cycle[k mod 9]` layout texels (LM01_Trans_RasterSS, read off the capture: the
    /// sun pass walks the nine, the sweep one per direction); `jitter_sign` = −1 samples the geometry at
    /// centre − shift (the geometry moved by +shift), +1 the mirror reading.
    /// The harness: stop a sweep after this many directions (0 = all).
    pub max_dirs: usize,
    /// Print the stage timers at the end of every sweep.
    pub profile: bool,
    pub raster_jitter: bool,
    pub jitter_cycle: [[f32; 2]; 9],
    pub jitter_sign: f32,
    /// The game's peel layer-count rule (0x140234df0: stop under 0.1 % of the viewport written, read
    /// `lag` layers late; 20 item layers at most) — `peelcap::PeelStop`.
    pub peel_stop: crate::peelcap::PeelStop,
    /// A fixed item-layer count for every peel (`--peel-layers N`; overrides the rule).
    pub peel_layers_fixed: Option<usize>,
    /// The captured item-layer counts per direction (indexed like `sphere_dirs`) and peel, taken instead
    /// of the rule when `layers_from_capture` (the harness: the layer count is timing-dependent in the
    /// game, so the comparison takes the count the capture shows).
    pub peel_layer_counts: Option<std::sync::Arc<Vec<Vec<Option<usize>>>>>,
    pub layers_from_capture: bool,
    /// The game's sky dome MESH (capture e001051), rasterised per peel with VS 16773's constants; None =
    /// the analytic ellipsoid model (`SkyGradient::dome_radiance`).
    pub dome_mesh: Option<std::sync::Arc<crate::domemesh::DomeMesh>>,
    /// THE TRANSCRIBED ACCUMULATE (lmaccum.rs) in the harness: the capture's LM meshes / instance stream (the game's own
    /// LM raster geometry: vertex normals, tangents, PSIZE modes, two-sided cards) drive LmILightDir_Set over OUR peel
    /// layers and the H-basis MRTs; `fitted_world_box` = the fitted blocks' WorldBoxMinXZ / MaxXZ (the clip distances
    /// of VS 17115); `hbasis_game` = the capture (root, entries) to compare each direction's ilightdir / MRTs with.
    pub lm_scene: Option<std::sync::Arc<crate::lmaccum::LmScene>>,
    pub fitted_world_box: Option<[[f32; 2]; 2]>,
    pub hbasis_game: Option<(std::path::PathBuf, std::sync::Arc<Vec<crate::lmaccum::CapEntry>>)>,
}

impl Default for BakeParams {
    fn default() -> Self {
        BakeParams {
            sky: [0.407, 0.458, 0.546],
            ambient: [0.0; 3],
            up: [0.0; 3],
            sun: [1.9, 1.387, 0.399],
            sun_dir: norm([0.5, 0.6, 0.6]),
            sun_radius: 0.01,
            sky_samples: 64,
            sun_samples: 8,
            texels_per_m: 1.1,
            max_px: 512,
            min_px: 2,
            ground_y: -1.0e9,
            bounce: 0.0,
            raster_peel: false,
            ss: 3,
            peel_res: 2048,
            peel_bias: 0.02,
            flat_albedo: false,
            alpha_masks: std::sync::Arc::new(Vec::new()),
            decor_ambient: 1.0,
            l_ambient: [0.0; 3],
            decor_sky_up: [0.0; 3],
            water_reflect: 0.5,
            water_sun: 0.0,
            water_sun_pow: 8.0,
            card_one_sided: std::env::var_os("LMTOOL_CARD_ONE_SIDED").is_some(),
            albedo: 0.5,
            flip_v: false,
            uv_bounds: false,
            sky_model: 0,
            threads: 0,
            want_bounce: false,
            inset_px: 0.0,
            fit_regressor: 0,
            pattern: false,
            pattern_flat: false,
            light_k: 0.27,
            ambient_la: [0.0; 3],
            direct_sun: 1.0,
            ambient_ao: false,
            dome_deg: 90.0,
            ground_bounce: 0.37,
            dome_dirs: std::sync::Arc::new(Vec::new()),
            bounce_sphere: false,
            field: None,
            peel: false,
            sphere_dirs: std::sync::Arc::new(Vec::new()),
            bounce_decode: 1.0,
            sky_grad: None,
            horizon_el: 0.0,
            horizon_radiance: [0.0; 3],
            sky_cube: None,
            sky_cube_scale: 1.0,
            dump: None,
            sweep: 0,
            game_peel: false,
            per_subsample: false,
            frustums: None,
            shadow_frustum: None,
            quant_peel: crate::gpufmt::Quant::None,
            quant_ilightdir: crate::gpufmt::Quant::None,
            quant_accum: crate::gpufmt::Quant::None,
            rounding: crate::gpufmt::Rounding::NearestEven,
            depth_bias: (1, 1.0),
            peel_inset: true,
            dome_layer: true,
            obj_base: 4096,
            depth_clip: false,
            depth_bits: 16,
            accum_hbasis: false,
            // the game stores 2·C0 (the finalisation's PS 1109 ScaleSrc 2; the baker's encode reading) and the
            // port's decode carries the 1/√(2π) of the frame's MaxHDR record: E_port = 2·C0/√(2π)
            hbasis_kappa: 0.797_884_56,
            sweep0_sun: false,
            lightsum_after: Default::default(),
            dome_exact: true,
            max_dirs: 0,
            profile: false,
            raster_jitter: false,
            jitter_cycle: [[-4.0, 2.0], [-1.0, 3.0], [2.0, 4.0], [-3.0, -1.0], [0.0, 0.0], [3.0, 1.0], [-2.0, -4.0], [1.0, -3.0], [4.0, -2.0]],
            jitter_sign: -1.0,
            peel_stop: crate::peelcap::PeelStop::default(),
            peel_layers_fixed: None,
            peel_layer_counts: None,
            layers_from_capture: true,
            dome_mesh: None,
            lm_scene: None,
            fitted_world_box: None,
            hbasis_game: None,
        }
    }
}

/// One item's baked chart: HDR irradiance per pixel, and which pixels hold
/// geometry (the rest are dilated copies).
#[derive(Clone, Debug)]
pub struct ChartBake {
    pub item: usize,
    pub w: u32,
    pub h: u32,
    pub rgb: Vec<[f32; 3]>,
    /// Frame 1: the point lights' irradiance per texel (empty when not baked).
    pub rgb1: Vec<[f32; 3]>,
    pub covered: Vec<bool>,
    /// Sun-visibility mean over the chart (diagnostics / fitting).
    pub sun_vis: f32,
    pub sky_vis: f32,
    /// The plain gathered irradiance when `rgb` holds something else (the prelit card texels): what the
    /// next sweep's bounce reads back. Empty = `rgb` is the irradiance.
    pub rgb_irr: Vec<[f32; 3]>,
}

impl ChartBake {
    pub fn max_channel(&self) -> f32 {
        self.rgb.iter().flat_map(|c| c.iter().copied()).fold(0.0, f32::max)
    }
    pub fn mean(&self) -> [f32; 3] {
        let mut s = [0f32; 3];
        let mut n = 0f32;
        for (c, &cov) in self.rgb.iter().zip(self.covered.iter()) {
            if cov {
                for k in 0..3 {
                    s[k] += c[k];
                }
                n += 1.0;
            }
        }
        if n > 0.0 {
            for k in 0..3 {
                s[k] /= n;
            }
        }
        s
    }
}

/// Is this model a flat ground tile (a zero-thickness quad-like mesh: ≤ 16 triangles, under 5 cm tall —
/// the "16×16×0" terrain tiles; a track plate with a real thickness is a caster)? The
/// editor's test bakes show such items do not occlude the dome (a 16×16 terrain tile 16 m above a pad
/// leaves it at 0.86–1.02 of open on RedIsland/WhiteShore/Stadium) — Nadeo's ground tiles are receivers
/// only (CastShadowGrp flags); they stay charted but leave the BVH.
pub fn is_flat_tile(m: &crate::geometry::ModelGeom) -> bool {
    if m.tris.is_empty() || m.tris.len() > 16 {
        return false;
    }
    let (mut lo, mut hi) = (f32::MAX, f32::MIN);
    for t in &m.tris {
        for p in &t.p {
            lo = lo.min(p[1]);
            hi = hi.max(p[1]);
        }
    }
    hi - lo < 0.05
}

/// World-space triangles of every instance, for the BVH (flat ground tiles excluded only with
/// `LMTOOL_TILES_CAST=0`).
pub fn world_tris(scene: &Scene) -> Vec<WTri> {
    world_tris_masks(scene).0
}

/// The world triangles and the cut-out mask list their `alpha` indexes (the scene's masks in name order;
/// a triangle whose material has no decodable mask is opaque).
pub fn world_tris_masks(scene: &Scene) -> (Vec<WTri>, Vec<crate::geometry::AlphaMask>) {
    // OFF by default: BlueBay's track plates (AC16902154, a zero-thickness quad too) DO occlude in the
    // editor, the RI/WS/Stadium terrain tiles do not — the difference is the item's CastShadow flag,
    // not its shape; without the flag every mesh casts. LMTOOL_TILES_CAST=0 drops the flat quads.
    let tiles_cast = std::env::var("LMTOOL_TILES_CAST").map(|v| v != "0").unwrap_or(true);
    // LMTOOL_ALPHA_TEST=0: the cut-out materials as opaque geometry
    let alpha_test = std::env::var("LMTOOL_ALPHA_TEST").map(|v| v != "0").unwrap_or(true);
    let mask_names: Vec<&String> = scene.alpha_masks.keys().collect();
    let masks: Vec<crate::geometry::AlphaMask> = scene.alpha_masks.values().cloned().collect();
    let mut out = Vec::with_capacity(scene.tri_count());
    let mut skipped = 0usize;
    let mut cut_tris = 0usize;
    for (ii, inst) in scene.instances.iter().enumerate() {
        let m = &scene.models[inst.model];
        if !tiles_cast && is_flat_tile(m) {
            skipped += 1;
            continue;
        }
        // per model material → mask index
        let mask_of: Vec<u16> = m.alpha_tex.iter().map(|f| mask_names.iter().position(|n| *n == f).map(|i| i as u16).unwrap_or(u16::MAX)).collect();
        for (ti, t) in m.tris.iter().enumerate() {
            let p0 = xf_point(&inst.xf, t.p[0]);
            let p1 = xf_point(&inst.xf, t.p[1]);
            let p2 = xf_point(&inst.xf, t.p[2]);
            let alpha = if alpha_test && t.alpha != u16::MAX { mask_of.get(t.alpha as usize).copied().unwrap_or(u16::MAX) } else { u16::MAX };
            if alpha != u16::MAX { cut_tris += 1; }
            out.push(WTri { p0, e1: sub(p1, p0), e2: sub(p2, p0), inst: ii as u32, tri: ti as u32, alpha, uv0: t.uv0 });
        }
    }
    if skipped > 0 {
        eprintln!("bvh: {skipped} flat ground-tile items left out (receivers only)");
    }
    if cut_tris > 0 {
        eprintln!("bvh: {cut_tris} alpha-tested triangles ({} cut-out masks: {})", masks.len(), mask_names.iter().enumerate().map(|(i, n)| format!("{i} = {n} {}×{}", masks[i].w, masks[i].h)).collect::<Vec<_>>().join(", "));
        // LMTOOL_DUMP_CARDS=FILE: every alpha-tested world triangle as text (mask index, the three vertices'
        // world positions and TexCoord0) — a differential check against a capture's post-VS card vertices
        if let Ok(path) = std::env::var("LMTOOL_DUMP_CARDS") {
            let mut txt = String::new();
            for t in out.iter().filter(|t| t.alpha != u16::MAX) {
                let p1 = [t.p0[0] + t.e1[0], t.p0[1] + t.e1[1], t.p0[2] + t.e1[2]];
                let p2 = [t.p0[0] + t.e2[0], t.p0[1] + t.e2[1], t.p0[2] + t.e2[2]];
                txt.push_str(&format!("{} {} {} {} {} {} {} {} {} {} {} {} {} {} {} {} {} {}\n", t.alpha, t.inst, t.tri, t.p0[0], t.p0[1], t.p0[2], t.uv0[0][0], t.uv0[0][1], p1[0], p1[1], p1[2], t.uv0[1][0], t.uv0[1][1], p2[0], p2[1], p2[2], t.uv0[2][0], t.uv0[2][1]));
            }
            std::fs::write(&path, txt).expect("LMTOOL_DUMP_CARDS");
            // and the masks as PGM (255 = opaque) next to it
            for (i, m) in masks.iter().enumerate() {
                let mut pgm = format!("P5\n{} {}\n255\n", m.w, m.h).into_bytes();
                for y in 0..m.h { for x in 0..m.w { let bit = m.bits[(y * m.w + x) >> 3] & (1 << ((y * m.w + x) & 7)) != 0; pgm.push(if bit { 255 } else { 0 }); } }
                std::fs::write(format!("{path}.mask{i}.pgm"), pgm).expect("mask pgm");
            }
        }
    }
    for (di, d) in scene.decor.iter().enumerate() {
        out.push(WTri { p0: d.p[0], e1: sub(d.p[1], d.p[0]), e2: sub(d.p[2], d.p[0]), inst: crate::geometry::DECOR_INST, tri: di as u32, alpha: u16::MAX, uv0: [[0.0; 2]; 3] });
    }
    (out, masks)
}

struct Rng(u64);
impl Rng {
    fn next(&mut self) -> f32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        ((self.0 >> 40) as f32) / (1u64 << 24) as f32
    }
}

/// Orthonormal frame around `n`.
fn frame(n: V3) -> (V3, V3) {
    let a = if n[0].abs() > 0.9 { [0.0, 1.0, 0.0] } else { [1.0, 0.0, 0.0] };
    let t = norm(cross(a, n));
    let b = cross(n, t);
    (t, b)
}

/// A texel to shade: world position, shading normal, item.
#[derive(Clone, Copy)]
struct Sample {
    p: V3,
    n: V3,
    px: u32,
    py: u32,
    /// The triangle's material is alpha-tested (a vegetation card).
    cut: bool,
    /// The triangle's material index (`ModelGeom::mat_links`; u16::MAX = none).
    mat: u16,
}

/// Chart size in pixels (w, h). With a PreLightGen the game's own rule is
/// followed: layout texels = u02 × 0.5625 × uv extent (Nadeo: u02 32 → 18,
/// u02 104 × 0.326 → 20); `texels_per_m` scales that (1.0 = Nadeo density).
pub fn chart_size(m: &crate::geometry::ModelGeom, scale: f32, prm: &BakeParams) -> (u32, u32) {
    let (ew, eh) = match (prm.uv_bounds, m.plg_bounds) {
        (true, Some(b)) => ((b[2] - b[0]).clamp(0.05, 1.0), (b[3] - b[1]).clamp(0.05, 1.0)),
        _ => (1.0, 1.0),
    };
    let base = if m.plg_u02 > 0.0 { m.plg_u02 * 0.5625 } else { m.metres_per_uv * 1.1 };
    let tw = base * scale * prm.texels_per_m * ew;
    let th = base * scale * prm.texels_per_m * eh;
    let px = |t: f32| ((t / 2.0).ceil() as u32).clamp(prm.min_px, prm.max_px);
    (px(tw), px(th))
}

/// Rasterise one instance's uv1 triangles into a w×h chart; returns the
/// samples (one per covered pixel centre) plus the coverage mask.
fn rasterise_mode(scene: &Scene, ii: usize, w: u32, h: u32, flip_v: bool, use_bounds: bool) -> (Vec<Sample>, Vec<bool>) {
    rasterise_inset(scene, ii, w, h, flip_v, use_bounds, 0.0)
}

fn rasterise_inset(scene: &Scene, ii: usize, w: u32, h: u32, flip_v: bool, use_bounds: bool, inset: f32) -> (Vec<Sample>, Vec<bool>) {
    let inst = &scene.instances[ii];
    let m = &scene.models[inst.model];
    let (u0, v0, su, sv) = match (use_bounds, m.plg_bounds) {
        (true, Some(b)) => (b[0], b[1], 1.0 / (b[2] - b[0]), 1.0 / (b[3] - b[1])),
        _ => (0.0, 0.0, 1.0, 1.0),
    };
    let mut covered = vec![false; (w * h) as usize];
    let mut samples = Vec::new();
    let (fw, fh) = (w as f32, h as f32);
    let (iw, ih) = (fw - 2.0 * inset, fh - 2.0 * inset);
    for t in &m.tris {
        let uvp: Vec<[f32; 2]> = t
            .uv
            .iter()
            .map(|uv| {
                let u = (uv[0] - u0) * su;
                let v = (uv[1] - v0) * sv;
                let v = if flip_v { 1.0 - v } else { v };
                [inset + u * iw, inset + v * ih]
            })
            .collect();
        let minx = uvp.iter().map(|p| p[0]).fold(f32::MAX, f32::min).floor().max(0.0) as i32;
        let maxx = uvp.iter().map(|p| p[0]).fold(f32::MIN, f32::max).ceil().min(fw) as i32;
        let miny = uvp.iter().map(|p| p[1]).fold(f32::MAX, f32::min).floor().max(0.0) as i32;
        let maxy = uvp.iter().map(|p| p[1]).fold(f32::MIN, f32::max).ceil().min(fh) as i32;
        if minx >= maxx || miny >= maxy {
            continue;
        }
        let (a, b, c) = (uvp[0], uvp[1], uvp[2]);
        let det = (b[0] - a[0]) * (c[1] - a[1]) - (c[0] - a[0]) * (b[1] - a[1]);
        if det.abs() < 1e-12 {
            continue;
        }
        let wp = [xf_point(&inst.xf, t.p[0]), xf_point(&inst.xf, t.p[1]), xf_point(&inst.xf, t.p[2])];
        let wn = [xf_normal(&inst.xf, t.n[0]), xf_normal(&inst.xf, t.n[1]), xf_normal(&inst.xf, t.n[2])];
        // a triangle smaller than a pixel still owns the pixel its centroid sits in
        let cx = ((a[0] + b[0] + c[0]) / 3.0).floor().clamp(0.0, fw - 1.0) as i32;
        let cy = ((a[1] + b[1] + c[1]) / 3.0).floor().clamp(0.0, fh - 1.0) as i32;
        let mut any = false;
        for py in miny..maxy {
            for px in minx..maxx {
                let x = px as f32 + 0.5;
                let y = py as f32 + 0.5;
                let l1 = ((b[0] - x) * (c[1] - y) - (c[0] - x) * (b[1] - y)) / det;
                let l2 = ((c[0] - x) * (a[1] - y) - (a[0] - x) * (c[1] - y)) / det;
                let l3 = 1.0 - l1 - l2;
                let eps = -0.002;
                if l1 < eps || l2 < eps || l3 < eps {
                    continue;
                }
                let p = add(add(mul(wp[0], l1), mul(wp[1], l2)), mul(wp[2], l3));
                let n = norm(add(add(mul(wn[0], l1), mul(wn[1], l2)), mul(wn[2], l3)));
                let idx = (py as u32 * w + px as u32) as usize;
                if !covered[idx] {
                    covered[idx] = true;
                    samples.push(Sample { p, n, px: px as u32, py: py as u32, cut: t.alpha != u16::MAX, mat: t.mat });
                }
                any = true;
            }
        }
        if !any {
            let idx = (cy as u32 * w + cx as u32) as usize;
            if !covered[idx] {
                covered[idx] = true;
                let p = mul(add(add(wp[0], wp[1]), wp[2]), 1.0 / 3.0);
                let n = norm(add(add(wn[0], wn[1]), wn[2]));
                samples.push(Sample { p, n, px: cx as u32, py: cy as u32, cut: t.alpha != u16::MAX, mat: t.mat });
            }
        }
    }
    (samples, covered)
}

/// Shade one sample. Returns (irradiance, sky visibility, sun visibility).
/// The previous bounce iteration's lightmap: one chart per instance, looked up from a hit
/// through the triangle's uv1 (the same uv → pixel map the rasteriser uses).
impl std::fmt::Debug for RadianceField {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "RadianceField({} charts)", self.charts.len())
    }
}

pub struct RadianceField {
    pub charts: Vec<Option<(u32, u32, Vec<[f32; 3]>)>>,
    pub flip_v: bool,
    pub uv_bounds: bool,
}

impl RadianceField {
    /// The stored irradiance at the point where a ray hit triangle `tri` of instance `inst`
    /// with barycentrics (b1, b2) on (e1, e2).
    pub fn lookup(&self, scene: &Scene, inst: u32, tri: u32, b1: f32, b2: f32) -> Option<[f32; 3]> {
        if inst == crate::geometry::DECOR_INST {
            return None;
        }
        let (w, h, px) = self.charts.get(inst as usize)?.as_ref()?;
        let instance = &scene.instances[inst as usize];
        let m = &scene.models[instance.model];
        let t = m.tris.get(tri as usize)?;
        let uv = [t.uv[0][0] + b1 * (t.uv[1][0] - t.uv[0][0]) + b2 * (t.uv[2][0] - t.uv[0][0]), t.uv[0][1] + b1 * (t.uv[1][1] - t.uv[0][1]) + b2 * (t.uv[2][1] - t.uv[0][1])];
        let (u0, v0, su, sv) = match (self.uv_bounds, m.plg_bounds) {
            (true, Some(b)) => (b[0], b[1], 1.0 / (b[2] - b[0]), 1.0 / (b[3] - b[1])),
            _ => (0.0, 0.0, 1.0, 1.0),
        };
        let u = (uv[0] - u0) * su;
        let v = (uv[1] - v0) * sv;
        let v = if self.flip_v { 1.0 - v } else { v };
        let x = ((u * *w as f32) as i64).clamp(0, *w as i64 - 1) as u32;
        let y = ((v * *h as f32) as i64).clamp(0, *h as i64 - 1) as u32;
        px.get((y * w + x) as usize).copied()
    }
}

/// Per-texel lighting components (the bounce estimate is per unit albedo·bounce).
pub struct Shaded {
    pub e: [f32; 3],
    pub sky_vis: f32,
    pub sun_vis: f32,
    pub bounce: [f32; 3],
    /// Per-channel sky term: the cube-weighted mean (E/π) with a cube, else sky_vis in every channel.
    pub sky_rgb: [f32; 3],
}

/// Shade one world point with a normal (debug probes).
pub fn shade_point_inst(scene: &Scene, bvh: &Bvh, prm: &BakeParams, p: V3, n: V3, ii: u32) -> Shaded {
    let s = Sample { px: 0, py: 0, p, n, cut: false, mat: u16::MAX };
    let mut rng = Rng(0x9E37_79B9_7F4A_7C15 ^ ((p[0] * 1000.0) as u64).wrapping_mul(0x2545_F491_4F6C_DD1D));
    shade_full(scene, bvh, prm, &s, ii, &mut rng)
}

pub fn solve4_pub(m: [[f64; 4]; 4], r: [f64; 4]) -> Option<[f64; 4]> {
    solve4(m, r)
}

pub fn shade_point(scene: &Scene, bvh: &Bvh, prm: &BakeParams, p: V3, n: V3) -> Shaded {
    let s = Sample { px: 0, py: 0, p, n, cut: false, mat: u16::MAX };
    let mut rng = Rng(0x9E37_79B9_7F4A_7C15);
    shade_full(scene, bvh, prm, &s, u32::MAX, &mut rng)
}

fn shade(scene: &Scene, bvh: &Bvh, prm: &BakeParams, s: &Sample, ii: u32, rng: &mut Rng) -> ([f32; 3], f32, f32) {
    let r = shade_full(scene, bvh, prm, s, ii, rng);
    (r.e, r.sky_vis, r.sun_vis)
}

fn shade_full(scene: &Scene, bvh: &Bvh, prm: &BakeParams, s: &Sample, ii: u32, rng: &mut Rng) -> Shaded {
    if prm.pattern {
        // 4 m checkerboard in x/z, hue by height band: continuous across items iff the uv mapping is right
        let c = ((s.p[0] / 4.0).floor() as i64 + (s.p[2] / 4.0).floor() as i64).rem_euclid(2);
        let band = ((s.p[1] / 4.0).floor() as i64).rem_euclid(3);
        let base = if c == 0 { 1.0 } else { 0.25 };
        let col = if prm.pattern_flat { [1.0, 1.0, 1.0] } else { match band { 0 => [1.0, 0.3, 0.3], 1 => [0.3, 1.0, 0.3], _ => [0.3, 0.3, 1.0] } };
        return Shaded { e: [col[0] * base, col[1] * base, col[2] * base], sky_vis: 1.0, sun_vis: 1.0, bounce: [0.0; 3], sky_rgb: [1.0; 3] };
    }
    if prm.peel {
        return shade_peel(scene, bvh, prm, s, ii, rng);
    }
    if prm.dome_deg < 89.0 {
        return shade_dome(scene, bvh, prm, s, ii, rng);
    }
    let o = add(s.p, mul(s.n, 0.03));
    let (t, b) = frame(s.n);
    let mut sky_vis = 0f32;
    let mut sky_e = [0f32; 3];
    let mut bounce = [0f32; 3];
    let nsky = prm.sky_samples.max(1);
    let rot = rng.next();
    // stratified cosine-weighted hemisphere
    let side = (nsky as f32).sqrt().ceil() as usize;
    let mut count = 0;
    for i in 0..side {
        for j in 0..side {
            if count >= nsky {
                break;
            }
            count += 1;
            let u = (i as f32 + rng.next()) / side as f32;
            let v = ((j as f32 + rng.next()) / side as f32 + rot).fract();
            let r = u.sqrt();
            let phi = 2.0 * std::f32::consts::PI * v;
            let (x, y) = (r * phi.cos(), r * phi.sin());
            let z = (1.0 - u).max(0.0).sqrt();
            let d = norm(add(add(mul(t, x), mul(b, y)), mul(s.n, z)));
            // the ground plane
            let mut tmax = 1.0e4f32;
            let mut ground_hit = false;
            if d[1] < 0.0 && o[1] > prm.ground_y {
                let tg = (prm.ground_y - o[1]) / d[1];
                if tg < tmax {
                    tmax = tg;
                    ground_hit = true;
                }
            }
            let blocked = bvh.occluded(o, d, tmax, ii, 0.05);
            if !blocked && !ground_hit {
                // sky radiance weight: uniform (cosine weighting is in the sampling)
                let wgt = if prm.sky_model == 1 { 0.5 + 0.5 * d[1].max(0.0) } else { 1.0 };
                sky_vis += wgt;
                if let Some(cube) = &prm.sky_cube {
                    let l = cube.sample(d);
                    for k in 0..3 {
                        sky_e[k] += l[k];
                    }
                }
            } else if prm.bounce > 0.0 || prm.want_bounce {
                // one bounce: the hit surface's own direct sun + half sky, times albedo
                let hit = if ground_hit && !blocked { None } else { bvh.closest(o, d, tmax) };
                let (hp, hn) = match hit {
                    Some(h) => {
                        let tri = &bvh.tris[h.tri as usize];
                        let hn = norm(cross(tri.e1, tri.e2));
                        let hn = if dot(hn, d) > 0.0 { mul(hn, -1.0) } else { hn };
                        (add(o, mul(d, h.t)), hn)
                    }
                    None => (add(o, mul(d, tmax)), [0.0, 1.0, 0.0]),
                };
                let ho = add(hp, mul(hn, 0.03));
                let ndl = dot(hn, prm.sun_dir).max(0.0);
                let sun_v = if ndl > 0.0 && !bvh.occluded(ho, prm.sun_dir, 1.0e4, u32::MAX, 0.0) { 1.0 } else { 0.0 };
                // the occluder's own radiance (the lightmapper's peel passes: its ambient, its
                // direct sun — the sun exists only here — and half the sky), before the albedo
                let sky_half = 0.5 * (0.5 + 0.5 * hn[1]);
                for k in 0..3 {
                    bounce[k] += prm.ambient_la[k] * (0.8 + 0.2 * hn[1]) + prm.sun[k] * ndl * sun_v + prm.sky[k] * sky_half;
                }
            }
        }
    }
    let sky_vis = sky_vis / count as f32;
    // sun
    let ndl = dot(s.n, prm.sun_dir);
    let mut sun_vis = 0f32;
    if ndl > 0.0 {
        let nsun = prm.sun_samples.max(1);
        let (st, sb) = frame(prm.sun_dir);
        for k in 0..nsun {
            let d = if k == 0 && nsun == 1 {
                prm.sun_dir
            } else {
                let r = prm.sun_radius * rng.next().sqrt();
                let phi = 2.0 * std::f32::consts::PI * rng.next();
                norm(add(prm.sun_dir, add(mul(st, r * phi.cos()), mul(sb, r * phi.sin()))))
            };
            if !bvh.occluded(o, d, 1.0e4, ii, 0.05) {
                sun_vis += 1.0;
            }
        }
        sun_vis /= nsun as f32;
    }
    let mut e = [0f32; 3];
    let bounce_n = [bounce[0] / count as f32, bounce[1] / count as f32, bounce[2] / count as f32];
    for k in 0..3 {
        // with a sky cube the mean cube radiance over the cosine-sampled hemisphere IS E/π of the sky
        let sky_term = match &prm.sky_cube {
            Some(_) => prm.sky_cube_scale * sky_e[k] / count as f32,
            None => prm.sky[k] * sky_vis,
        };
        // the ambient pass is occluded by the dome visibility when `ambient_ao` (the peel passes' alpha)
        let ao = if prm.ambient_ao { sky_vis } else { 1.0 };
        e[k] = prm.ambient_la[k] * (0.8 + 0.2 * s.n[1]) * ao + prm.ambient[k] + prm.up[k] * (0.5 + 0.5 * s.n[1]) + sky_term + prm.sun[k] * ndl.max(0.0) * sun_vis * prm.direct_sun + prm.bounce * prm.albedo * bounce_n[k];
    }
    let sky_rgb = match &prm.sky_cube {
        Some(_) => [sky_e[0] / count as f32, sky_e[1] / count as f32, sky_e[2] / count as f32],
        None => [sky_vis, sky_vis, sky_vis],
    };
    Shaded { e, sky_vis, sun_vis, bounce: bounce_n, sky_rgb }
}


/// The dome model: directions uniformly over the sphere (stratified), weight max(0, n·D).
/// An unoccluded direction inside the zenith cone adds the sky (normalised so an open
/// horizontal surface receives exactly `sky`); an occluded one adds the hit surface's
/// bounced radiance (its direct sun + its own sky share, times BounceFactor·albedo); a
/// direction reaching the sea/ground plane adds the plane's bounce.
fn shade_dome(scene: &Scene, bvh: &Bvh, prm: &BakeParams, s: &Sample, ii: u32, rng: &mut Rng) -> Shaded {
    let o = add(s.p, mul(s.n, 0.03));
    let n = prm.sky_samples.max(16);
    let side = (n as f32).sqrt().ceil() as usize;
    let cone_cos = prm.dome_deg.to_radians().cos();
    let rot = rng.next();
    // All transport runs along the dome directions D (N uniform directions inside the zenith cone —
    // the game's cone-light sample set, FUN_140237330 — swept as depth peels): a texel facing up
    // along D receives what lies above it (the sky when nothing does, else the occluder's bounced
    // radiance); a texel facing down along D receives what lies below it (a surface's bounced
    // radiance, the sea/ground plane's, nothing over the void). Weight |n·D|, normalised by the
    // cone's mean cosθ so an open floor gets exactly `sky` and an underside over the sea gets
    // `ground_bounce × the sea's value`. A wall (|n·D| small for every D) gets little of either —
    // the editor's wall/floor ratio of 0.15 = cone factor + 0.37 × cone factor.
    let mut sky_acc = 0f32;
    let mut open_cone = 0f32;
    let mut cone_total = 0f32;
    let mut bounce = [0f32; 3];
    let mut count = 0usize;
    let bounce_on = prm.bounce > 0.0;
    let cone_bounce = bounce_on && !prm.bounce_sphere;
    let exact = !prm.dome_dirs.is_empty();
    let total = if exact { prm.dome_dirs.len() } else { n };
    for idx in 0..total {
        {
            let (i, j) = (idx / side, idx % side);
            if count >= total {
                break;
            }
            count += 1;
            let up = if exact {
                prm.dome_dirs[idx]
            } else {
                let u = (i as f32 + rng.next()) / side as f32;
                let v = ((j as f32 + rng.next()) / side as f32 + rot).fract();
                let cos_t = 1.0 - (1.0 - cone_cos) * u; // uniform in solid angle within the cone
                let sin_t = (1.0 - cos_t * cos_t).max(0.0).sqrt();
                let phi = 2.0 * std::f32::consts::PI * v;
                [sin_t * phi.cos(), cos_t, sin_t * phi.sin()]
            };
            let cos_t = up[1];
            cone_total += cos_t;
            let ndl = dot(s.n, up);
            if ndl.abs() < 1e-4 {
                continue;
            }
            if ndl > 0.0 {
                // facing up along D: the sky, or the occluder above
                match bvh.closest(o, up, 1.0e4) {
                    None => {
                        sky_acc += ndl;
                        open_cone += cos_t;
                    }
                    Some(h) if cone_bounce => {
                        let tri = &bvh.tris[h.tri as usize];
                        let hn = norm(cross(tri.e1, tri.e2));
                        let hn = if dot(hn, up) > 0.0 { mul(hn, -1.0) } else { hn };
                        let e_hit = hit_irradiance(scene, bvh, prm, &h, o, up, hn, cone_cos);
                        for k in 0..3 {
                            bounce[k] += prm.bounce * prm.albedo * e_hit[k] * ndl;
                        }
                    }
                    Some(_) => {}
                }
            } else if cone_bounce {
                // facing down along D: what lies below
                let down = mul(up, -1.0);
                let w = -ndl;
                let mut tmax = 1.0e4f32;
                let mut ground_hit = false;
                if o[1] > prm.ground_y {
                    let tg = (prm.ground_y - o[1]) / down[1];
                    if tg < tmax {
                        tmax = tg;
                        ground_hit = true;
                    }
                }
                match bvh.closest(o, down, tmax) {
                    None if ground_hit => {
                        let hp = add(o, mul(down, tmax));
                        let sun_v = if prm.sun_dir[1] > 0.0 && !bvh.occluded(add(hp, [0.0, 0.03, 0.0]), prm.sun_dir, 1.0e4, u32::MAX, 0.0) { 1.0 } else { 0.0 };
                        for k in 0..3 {
                            let e_ground = prm.sky[k] + prm.sun[k] * prm.sun_dir[1].max(0.0) * sun_v;
                            bounce[k] += prm.ground_bounce * e_ground * w;
                        }
                    }
                    None => {}
                    Some(h) => {
                        let tri = &bvh.tris[h.tri as usize];
                        let hn = norm(cross(tri.e1, tri.e2));
                        let hn = if dot(hn, down) > 0.0 { mul(hn, -1.0) } else { hn };
                        let e_hit = hit_irradiance(scene, bvh, prm, &h, o, down, hn, cone_cos);
                        for k in 0..3 {
                            bounce[k] += prm.bounce * prm.albedo * e_hit[k] * w;
                        }
                    }
                }
            }
        }
    }
    let norm_c = cone_total.max(1e-6);
    let sky_frac = sky_acc / norm_c;
    let sky_vis = open_cone / norm_c;
    let mut bounce_n = [bounce[0] / norm_c, bounce[1] / norm_c, bounce[2] / norm_c];
    if bounce_on && prm.bounce_sphere {
        // the sphere pass: uniform directions over the sphere, each hit surface's bounced radiance
        // (L = albedo·E_hit/π) weighted by (n·D)⁺·(4π/N) — an enclosing Lambertian surface of
        // irradiance E gives back albedo·E
        let nb = prm.sky_samples.max(16);
        let bside = (nb as f32).sqrt().ceil() as usize;
        let rot2 = rng.next();
        let mut acc = [0f32; 3];
        let mut cnt = 0usize;
        for i in 0..bside {
            for j in 0..bside {
                if cnt >= nb {
                    break;
                }
                cnt += 1;
                let u = (i as f32 + rng.next()) / bside as f32;
                let v = ((j as f32 + rng.next()) / bside as f32 + rot2).fract();
                let cos_t = 1.0 - 2.0 * u;
                let sin_t = (1.0 - cos_t * cos_t).max(0.0).sqrt();
                let phi = 2.0 * std::f32::consts::PI * v;
                let d = [sin_t * phi.cos(), cos_t, sin_t * phi.sin()];
                let ndl = dot(s.n, d);
                if ndl <= 0.0 {
                    continue;
                }
                let mut tmax = 1.0e4f32;
                let mut ground_hit = false;
                if d[1] < 0.0 && o[1] > prm.ground_y {
                    let tg = (prm.ground_y - o[1]) / d[1];
                    if tg < tmax {
                        tmax = tg;
                        ground_hit = true;
                    }
                }
                match bvh.closest(o, d, tmax) {
                    None if !ground_hit => {}
                    None => {
                        let hp = add(o, mul(d, tmax));
                        let sun_v = if prm.sun_dir[1] > 0.0 && !bvh.occluded(add(hp, [0.0, 0.03, 0.0]), prm.sun_dir, 1.0e4, u32::MAX, 0.0) { 1.0 } else { 0.0 };
                        for k in 0..3 {
                            let e_ground = prm.sky[k] + prm.sun[k] * prm.sun_dir[1].max(0.0) * sun_v;
                            acc[k] += prm.ground_bounce * e_ground / std::f32::consts::PI * ndl;
                        }
                    }
                    Some(h) => {
                        let tri = &bvh.tris[h.tri as usize];
                        let hn = norm(cross(tri.e1, tri.e2));
                        let hn = if dot(hn, d) > 0.0 { mul(hn, -1.0) } else { hn };
                        let e_hit = hit_irradiance(scene, bvh, prm, &h, o, d, hn, cone_cos);
                        for k in 0..3 {
                            acc[k] += prm.bounce * prm.albedo * e_hit[k] / std::f32::consts::PI * ndl;
                        }
                    }
                }
            }
        }
        let w = 4.0 * std::f32::consts::PI / cnt.max(1) as f32;
        bounce_n = [acc[0] * w, acc[1] * w, acc[2] * w];
    }
    // the direct sun: a jittered disc of angular radius sun_radius, shadow rays against the scene
    // and the ground plane
    let ndl = dot(s.n, prm.sun_dir);
    let mut sun_vis = 0f32;
    if prm.direct_sun > 0.0 && ndl > 0.0 && prm.sun_dir[1] > 0.0 {
        let nsun = prm.sun_samples.max(1);
        let (st, sb) = frame(prm.sun_dir);
        for k in 0..nsun {
            let d = if k == 0 && nsun == 1 {
                prm.sun_dir
            } else {
                let r = prm.sun_radius * rng.next().sqrt();
                let phi = 2.0 * std::f32::consts::PI * rng.next();
                norm(add(prm.sun_dir, add(mul(st, r * phi.cos()), mul(sb, r * phi.sin()))))
            };
            if !bvh.occluded(o, d, 1.0e4, ii, 0.05) {
                sun_vis += 1.0;
            }
        }
        sun_vis /= nsun as f32;
    }
    let mut e = [0f32; 3];
    for k in 0..3 {
        e[k] = prm.sky[k] * sky_frac + bounce_n[k] + prm.ambient[k] + prm.up[k] * (0.5 + 0.5 * s.n[1]) + prm.direct_sun * prm.sun[k] * ndl.max(0.0) * sun_vis;
    }
    Shaded { e, sky_vis, sun_vis, bounce: bounce_n, sky_rgb: [sky_frac; 3] }
}


/// A hit surface's irradiance for the bounce: the previous iteration's stored value when a
/// field is given (the game's "lightmap so far"), else its unoccluded sky share; plus its direct
/// sun. `hp` = the hit point offset off the surface, `hn` = the surface normal facing the ray.
/// The albedo of the surface a ray hit: the material's (`Tri.mat` → `mat_albedo`, measured from the
/// diffuse texture or the keyword table), `prm.albedo` for an unknown material, or flat `prm.albedo`.
pub fn hit_albedo(scene: &Scene, bvh: &Bvh, prm: &BakeParams, h: &crate::bvh::Hit) -> [f32; 3] {
    if prm.flat_albedo {
        return [prm.albedo; 3];
    }
    let wt = &bvh.tris[h.tri as usize];
    if wt.inst == crate::geometry::DECOR_INST {
        return scene.decor.get(wt.tri as usize).map(|d| d.albedo).unwrap_or([prm.albedo; 3]);
    }
    let inst = &scene.instances[wt.inst as usize];
    let m = &scene.models[inst.model];
    // a known material (measured or keyword) is taken as is; an unknown one gets the per-collection
    // default `prm.albedo`
    let Some(t) = m.tris.get(wt.tri as usize) else { return [prm.albedo; 3] };
    if (t.mat as usize) < m.mat_albedo.len() && m.mat_albedo[t.mat as usize][0].is_finite() {
        return m.mat_albedo[t.mat as usize];
    }
    // a vegetation card (a cut-out material without a game-material link): the texture's own mean colour —
    // the game's MDiffuse for the card is its leaf texture, and a dense canopy relights itself in that colour
    if t.alpha != u16::MAX {
        if let Some(file) = m.alpha_tex.get(t.alpha as usize) {
            if let Some(&c) = scene.card_albedo.get(file) { return c; }
        }
    }
    // a custom-texture material (no game link): its diffuse texture's mean colour
    if t.diff != u16::MAX {
        if let Some(file) = m.diff_tex.get(t.diff as usize) {
            if let Some(&c) = scene.tex_albedo.get(file) { return c; }
        }
    }
    [prm.albedo; 3]
}

fn hit_irradiance(scene: &Scene, bvh: &Bvh, prm: &BakeParams, h: &crate::bvh::Hit, o: V3, d: V3, hn: V3, cone_cos: f32) -> [f32; 3] {
    let hp = add(add(o, mul(d, h.t)), mul(hn, 0.03));
    let ndl_h = dot(hn, prm.sun_dir).max(0.0);
    let sun_v = if ndl_h > 0.0 && prm.sun_dir[1] > 0.0 && !bvh.occluded(hp, prm.sun_dir, 1.0e4, u32::MAX, 0.0) { 1.0 } else { 0.0 };
    let stored: Option<[f32; 3]> = prm.field.as_ref().and_then(|f| {
        let tri = &bvh.tris[h.tri as usize];
        let v = sub(add(o, mul(d, h.t)), tri.p0);
        let (d00, d01, d11, d20, d21) = (dot(tri.e1, tri.e1), dot(tri.e1, tri.e2), dot(tri.e2, tri.e2), dot(v, tri.e1), dot(v, tri.e2));
        let den = d00 * d11 - d01 * d01;
        if den.abs() < 1e-12 {
            return None;
        }
        let b1 = ((d11 * d20 - d01 * d21) / den).clamp(0.0, 1.0);
        let b2 = ((d00 * d21 - d01 * d20) / den).clamp(0.0, 1.0);
        f.lookup(scene, tri.inst, tri.tri, b1, b2)
    });
    let mut e = [0f32; 3];
    for k in 0..3 {
        // the first sweep peels the surfaces with their lightmap term forced to 0 (RE child 2): the
        // bounce input is the direct sun only; later sweeps read the lightmap so far. The older dome
        // model (no peel) keeps its one-bounce sky-share estimate.
        // the stored value is read back divided by bounce_decode (BounceFactor); the direct sun is not
        let base = match stored {
            Some(s) => s[k] / if prm.peel { prm.bounce_decode } else { 1.0 },
            None if prm.peel => 0.0,
            None => prm.sky[k] * cone_factor(hn, cone_cos),
        };
        e[k] = base + prm.sun[k] * ndl_h * sun_v;
    }
    e
}


/// The sky radiance in direction `d` (unit): the cube when given, else the constant colour.
pub fn sky_radiance(prm: &BakeParams, d: V3) -> [f32; 3] {
    if let Some(g) = &prm.sky_grad {
        return g.radiance(d);
    }
    match &prm.sky_cube {
        Some(cube) => {
            let l = cube.sample(d);
            [l[0] * prm.sky_cube_scale, l[1] * prm.sky_cube_scale, l[2] * prm.sky_cube_scale]
        }
        None => prm.sky,
    }
}

/// The game's peel model: E(texel) = Σ_D (4/N)·max(0, n·D)·L(D) over the full-sphere direction
/// set, L(D) = the first surface along D — the sky (its radiance, as the dome renders it), a hit
/// surface's bounce `albedo·(E_hit/bounce_decode + sun·max(0,n_h·L)·shadow)` (E_hit = the
/// previous iteration's stored value, else its own sky share), or the ground/sea plane's bounce.
/// With a uniform sky S over the sphere and no geometry, an up-facing texel gets exactly S.
fn shade_peel(scene: &Scene, bvh: &Bvh, prm: &BakeParams, s: &Sample, ii: u32, rng: &mut Rng) -> Shaded {
    let o = add(s.p, mul(s.n, 0.03));
    let exact = !prm.sphere_dirs.is_empty();
    let n = if exact { prm.sphere_dirs.len() } else { prm.sky_samples.max(16) };
    let side = (n as f32).sqrt().ceil() as usize;
    let rot = rng.next();
    let mut e = [0f32; 3];
    let mut sky_acc = [0f32; 3];
    let mut bounce_acc = [0f32; 3];
    let mut sky_vis = 0f32;
    let mut sun_vis = 0f32;
    let cone_cos = 0.0f32; // hit surfaces' own sky share: the hemisphere (cone_factor(hn, 0) = ½·(1+n.y))
    for idx in 0..n {
        let d = if exact {
            prm.sphere_dirs[idx]
        } else {
            let (i, j) = (idx / side, idx % side);
            let u = (i as f32 + rng.next()) / side as f32;
            let v = ((j as f32 + rng.next()) / side as f32 + rot).fract();
            let cos_t = 1.0 - 2.0 * u;
            let sin_t = (1.0 - cos_t * cos_t).max(0.0).sqrt();
            let phi = 2.0 * std::f32::consts::PI * v;
            [sin_t * phi.cos(), cos_t, sin_t * phi.sin()]
        };
        let ndl = dot(s.n, d);
        if ndl <= 0.0 {
            continue;
        }
        let w = 4.0 / n as f32 * ndl;
        // the ground/sea plane
        let mut tmax = 1.0e4f32;
        let mut ground_hit = false;
        if d[1] < 0.0 && o[1] > prm.ground_y {
            let tg = (prm.ground_y - o[1]) / d[1];
            if tg < tmax {
                tmax = tg;
                ground_hit = true;
            }
        }
        match bvh.closest(o, d, tmax) {
            None if !ground_hit => {
                let el = d[1].asin().to_degrees();
                if prm.horizon_el > 0.0 && el < prm.horizon_el {
                    for k in 0..3 {
                        bounce_acc[k] += w * prm.horizon_radiance[k];
                    }
                } else {
                    let l = sky_radiance(prm, d);
                    for k in 0..3 {
                        sky_acc[k] += w * l[k];
                    }
                    sky_vis += w;
                }
            }
            None => {
                // the sea/ground: its own sky share (a horizontal plane, half the sphere) + direct sun
                let hp = add(o, mul(d, tmax));
                let sun_v = if prm.sun_dir[1] > 0.0 && !bvh.occluded(add(hp, [0.0, 0.03, 0.0]), prm.sun_dir, 1.0e4, u32::MAX, 0.0) { 1.0 } else { 0.0 };
                let up_sky = sky_radiance(prm, [0.0, 1.0, 0.0]);
                for k in 0..3 {
                    let e_ground = up_sky[k] / prm.bounce_decode + prm.sun[k] * prm.sun_dir[1].max(0.0) * sun_v;
                    bounce_acc[k] += w * prm.ground_bounce * e_ground;
                }
            }
            Some(h) => {
                let tri = &bvh.tris[h.tri as usize];
                let hn = norm(cross(tri.e1, tri.e2));
                let hn = if dot(hn, d) > 0.0 { mul(hn, -1.0) } else { hn };
                let e_hit = hit_irradiance(scene, bvh, prm, &h, o, d, hn, cone_cos);
                // hit_irradiance returns (stored/bounce_decode + direct sun) — see there; the surface's
                // own material albedo scales what it gives back
                let alb = hit_albedo(scene, bvh, prm, &h);
                for k in 0..3 {
                    bounce_acc[k] += w * prm.bounce * alb[k] * e_hit[k];
                }
            }
        }
    }
    // the direct sun on the receiver only when asked (the game has none)
    let ndl = dot(s.n, prm.sun_dir);
    if prm.direct_sun > 0.0 && ndl > 0.0 && prm.sun_dir[1] > 0.0 && !bvh.occluded(o, prm.sun_dir, 1.0e4, ii, 0.05) {
        sun_vis = 1.0;
    }
    for k in 0..3 {
        e[k] = sky_acc[k] + bounce_acc[k] + prm.direct_sun * prm.sun[k] * ndl.max(0.0) * sun_vis;
    }
    Shaded { e, sky_vis: sky_vis / 2.0, sun_vis, bounce: bounce_acc, sky_rgb: sky_acc }
}

/// The share of the zenith cone's cosine-weighted light a surface of normal `hn` collects when
/// nothing occludes it: ∫_cone max(0, hn·D) dω / ∫_cone cosθ dω (1 for a floor, ~0.16 for a
/// wall at 40°, 0 for a ceiling). Numerical, cached per call site would be nicer; cheap enough.
fn cone_factor(hn: V3, cone_cos: f32) -> f32 {
    if hn[1] > 0.999 {
        return 1.0;
    }
    if hn[1] < -0.999 {
        return 0.0;
    }
    let (mut num, mut den) = (0f32, 0f32);
    let steps = 12;
    for i in 0..steps {
        let cos_t = cone_cos + (1.0 - cone_cos) * (i as f32 + 0.5) / steps as f32;
        let sin_t = (1.0 - cos_t * cos_t).max(0.0).sqrt();
        for j in 0..24 {
            let phi = 2.0 * std::f32::consts::PI * (j as f32 + 0.5) / 24.0;
            let d = [sin_t * phi.cos(), cos_t, sin_t * phi.sin()];
            num += dot(hn, d).max(0.0);
            den += cos_t;
        }
    }
    num / den.max(1e-6)
}

/// Bake every instance. Returns one chart per instance (index = instance).
/// `lights`: the scene's point lights (frame 1), used when `prm.light_k > 0`.
pub fn bake(scene: &Scene, bvh: &Bvh, prm: &BakeParams, lights: &[(usize, crate::geometry::LightDef)]) -> Vec<ChartBake> {
    let n = scene.instances.len();
    let threads = if prm.threads == 0 { std::thread::available_parallelism().map(|x| x.get()).unwrap_or(8).min(160) } else { prm.threads };
    let next = std::sync::atomic::AtomicUsize::new(0);
    let results: Vec<std::sync::Mutex<Option<ChartBake>>> = (0..n).map(|_| std::sync::Mutex::new(None)).collect();
    std::thread::scope(|sc| {
        for _ in 0..threads {
            sc.spawn(|| loop {
                let ii = next.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                if ii >= n {
                    break;
                }
                let inst = &scene.instances[ii];
                let m = &scene.models[inst.model];
                let scale = {
                    let c0 = [inst.xf[0], inst.xf[1], inst.xf[2]];
                    dot(c0, c0).sqrt()
                };
                let (w, h) = chart_size(m, scale, prm);
                let (samples, covered) = rasterise_inset(scene, ii, w, h, prm.flip_v, prm.uv_bounds, prm.inset_px);
                let mut rgb = vec![[0f32; 3]; (w * h) as usize];
                let want_lights = prm.light_k > 0.0 && !lights.is_empty();
                let mut rgb1 = if want_lights { vec![[0f32; 3]; (w * h) as usize] } else { Vec::new() };
                let mut rng = Rng(0x9E37_79B9_7F4A_7C15 ^ ((ii as u64 + 1) * 0x2545_F491_4F6C_DD1D));
                let (mut sv, mut kv) = (0f32, 0f32);
                for s in &samples {
                    let (e, sky_vis, sun_vis) = shade(scene, bvh, prm, s, ii as u32, &mut rng);
                    rgb[(s.py * w + s.px) as usize] = e;
                    if want_lights {
                        let o = add(s.p, mul(s.n, 0.03));
                        rgb1[(s.py * w + s.px) as usize] = crate::probes::light_sum(bvh, lights, o, Some(s.n), prm.light_k, ii as u32);
                    }
                    sv += sun_vis;
                    kv += sky_vis;
                }
                let ns = samples.len().max(1) as f32;
                dilate(&mut rgb, &covered, w, h, prm.sky);
                if want_lights {
                    dilate(&mut rgb1, &covered, w, h, [0.0; 3]);
                }
                *results[ii].lock().unwrap() = Some(ChartBake { item: inst.item, w, h, rgb, rgb1, covered, sun_vis: sv / ns, sky_vis: kv / ns, rgb_irr: Vec::new() });
            });
        }
    });
    results.into_iter().map(|m| m.into_inner().unwrap().unwrap()).collect()
}

/// Flood-fill the uncovered texels of a chart from their covered neighbours
/// (repeated until full); anything still uncovered takes the covered mean
/// (or `fallback` on an empty chart).
fn dilate(rgb: &mut [[f32; 3]], covered: &[bool], w: u32, h: u32, fallback: [f32; 3]) {
    let mut cov = covered.to_vec();
    for _ in 0..256 {
        if cov.iter().all(|&c| c) {
            break;
        }
        let src = rgb.to_vec();
        let scov = cov.clone();
        for y in 0..h {
            for x in 0..w {
                let i = (y * w + x) as usize;
                if scov[i] {
                    continue;
                }
                let mut acc = [0f32; 3];
                let mut cnt = 0;
                for (dx, dy) in [(-1i32, 0i32), (1, 0), (0, -1), (0, 1), (-1, -1), (1, 1), (-1, 1), (1, -1)] {
                    let (nx, ny) = (x as i32 + dx, y as i32 + dy);
                    if nx < 0 || ny < 0 || nx >= w as i32 || ny >= h as i32 {
                        continue;
                    }
                    let j = (ny as u32 * w + nx as u32) as usize;
                    if scov[j] {
                        for k in 0..3 {
                            acc[k] += src[j][k];
                        }
                        cnt += 1;
                    }
                }
                if cnt > 0 {
                    for k in 0..3 {
                        rgb[i][k] = acc[k] / cnt as f32;
                    }
                    cov[i] = true;
                }
            }
        }
    }
    let mut s = [0f32; 3];
    let mut c = 0f32;
    for (i, v) in rgb.iter().enumerate() {
        if cov[i] {
            for k in 0..3 {
                s[k] += v[k];
            }
            c += 1.0;
        }
    }
    let mean = if c > 0.0 { [s[0] / c, s[1] / c, s[2] / c] } else { fallback };
    for (i, v) in rgb.iter_mut().enumerate() {
        if !cov[i] {
            *v = mean;
        }
    }
}

/// Bake a SUBSET scene whose instances are a selection of a full scene's;
/// `full_ids[i]` is the full-scene instance id of `sub.instances[i]` (the BVH
/// was built over the full scene, so self-hit filtering needs those ids).
pub fn bake_subset(sub: &Scene, full_ids: &[u32], bvh: &Bvh, prm: &BakeParams) -> Vec<ChartBake> {
    let n = sub.instances.len();
    let threads = if prm.threads == 0 { std::thread::available_parallelism().map(|x| x.get()).unwrap_or(8).min(160) } else { prm.threads };
    let next = std::sync::atomic::AtomicUsize::new(0);
    let results: Vec<std::sync::Mutex<Option<ChartBake>>> = (0..n).map(|_| std::sync::Mutex::new(None)).collect();
    std::thread::scope(|sc| {
        for _ in 0..threads {
            sc.spawn(|| loop {
                let ii = next.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                if ii >= n {
                    break;
                }
                let inst = &sub.instances[ii];
                let m = &sub.models[inst.model];
                let scale = {
                    let c0 = [inst.xf[0], inst.xf[1], inst.xf[2]];
                    dot(c0, c0).sqrt()
                };
                let (pw0, ph0) = chart_size(m, scale, prm);
                let px = pw0.max(ph0).min(16);
                let (samples, covered) = rasterise_mode(sub, ii, px, px, prm.flip_v, prm.uv_bounds);
                let mut rgb = vec![[0f32; 3]; (px * px) as usize];
                let mut rng = Rng(0x9E37_79B9_7F4A_7C15 ^ ((ii as u64 + 1) * 0x2545_F491_4F6C_DD1D));
                let (mut sv, mut kv) = (0f32, 0f32);
                for s in &samples {
                    let (e, sky_vis, sun_vis) = shade(sub, bvh, prm, s, full_ids[ii], &mut rng);
                    rgb[(s.py * px + s.px) as usize] = e;
                    sv += sun_vis;
                    kv += sky_vis;
                }
                let ns = samples.len().max(1) as f32;
                *results[ii].lock().unwrap() = Some(ChartBake { item: inst.item, w: px, h: px, rgb, rgb1: Vec::new(), covered, sun_vis: sv / ns, sky_vis: kv / ns, rgb_irr: Vec::new() });
            });
        }
    });
    results.into_iter().map(|m| m.into_inner().unwrap().unwrap()).collect()
}

/// Public rasteriser (positions/normals per covered pixel) for analysis tools.
pub struct PubSample {
    pub p: V3,
    pub n: V3,
    pub px: u32,
    pub py: u32,
    /// The triangle's material is alpha-tested (a vegetation card).
    pub cut: bool,
    pub mat: u16,
}
pub fn rasterise_pub(scene: &Scene, ii: usize, w: u32, h: u32, flip_v: bool, use_bounds: bool) -> (Vec<PubSample>, Vec<bool>) {
    let (s, c) = rasterise_mode(scene, ii, w, h, flip_v, use_bounds);
    (s.into_iter().map(|x| PubSample { p: x.p, n: x.n, px: x.px, py: x.py, cut: x.cut, mat: x.mat }).collect(), c)
}

/// `bake_subset` at a fixed chart size.
pub fn bake_subset_px(sub: &Scene, full_ids: &[u32], bvh: &Bvh, prm: &BakeParams, w: u32, h: u32) -> Vec<ChartBake> {
    let mut out = Vec::new();
    for (ii, inst) in sub.instances.iter().enumerate() {
        let (samples, covered) = rasterise_mode(sub, ii, w, h, prm.flip_v, prm.uv_bounds);
        let mut rgb = vec![[0f32; 3]; (w * h) as usize];
        let mut rng = Rng(0x9E37_79B9_7F4A_7C15 ^ ((ii as u64 + 1) * 0x2545_F491_4F6C_DD1D));
        let (mut sv, mut kv) = (0f32, 0f32);
        for s in &samples {
            let (e, sky_vis, sun_vis) = shade(sub, bvh, prm, s, full_ids[ii], &mut rng);
            rgb[(s.py * w + s.px) as usize] = e;
            sv += sun_vis;
            kv += sky_vis;
        }
        let ns = samples.len().max(1) as f32;
        out.push(ChartBake { item: inst.item, w, h, rgb, rgb1: Vec::new(), covered, sun_vis: sv / ns, sky_vis: kv / ns, rgb_irr: Vec::new() });
    }
    out
}

/// Per-texel Pearson correlation between our bake and a reference atlas for a
/// set of instances (`sel`, full-scene ids) whose charts are `(px, py, pw, ph)`
/// pixel rects in `atlas`. Returns the mean per-chart correlation and count.
pub fn texel_correlation(scene: &Scene, bvh: &Bvh, sel: &[(usize, u32, u32, u32, u32)], atlas: &crate::img::Rgb, prm: &BakeParams) -> (f64, usize) {
    let threads = std::thread::available_parallelism().map(|x| x.get()).unwrap_or(8).min(160);
    let next = std::sync::atomic::AtomicUsize::new(0);
    let acc = std::sync::Mutex::new((0f64, 0usize));
    std::thread::scope(|sc| {
        for _ in 0..threads {
            sc.spawn(|| loop {
                let k = next.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                if k >= sel.len() {
                    break;
                }
                let (ii, px, py, pw, ph) = sel[k];
                let (samples, _) = rasterise_inset(scene, ii, pw, ph, prm.flip_v, prm.uv_bounds, prm.inset_px);
                let mut rng = Rng(0x9E37_79B9_7F4A_7C15 ^ ((ii as u64 + 1) * 0x2545_F491_4F6C_DD1D));
                let (mut xs, mut ys) = (Vec::new(), Vec::new());
                for s in &samples {
                    let (e, _, _) = shade(scene, bvh, prm, s, ii as u32, &mut rng);
                    let lum_m = 0.2126 * e[0] + 0.7152 * e[1] + 0.0722 * e[2];
                    let n = atlas.get((px + s.px).min(atlas.w - 1), (py + s.py).min(atlas.h - 1));
                    let lum_n = 0.2126 * n[0] as f32 + 0.7152 * n[1] as f32 + 0.0722 * n[2] as f32;
                    xs.push(lum_m as f64);
                    ys.push(lum_n as f64);
                }
                if xs.len() < 12 {
                    continue;
                }
                let n = xs.len() as f64;
                let (mx, my) = (xs.iter().sum::<f64>() / n, ys.iter().sum::<f64>() / n);
                let (mut sxy, mut sxx, mut syy) = (0.0, 0.0, 0.0);
                for (p, q) in xs.iter().zip(&ys) {
                    sxy += (p - mx) * (q - my);
                    sxx += (p - mx) * (p - mx);
                    syy += (q - my) * (q - my);
                }
                if sxx > 1e-9 && syy > 1e-9 {
                    let mut g = acc.lock().unwrap();
                    g.0 += sxy / (sxx * syy).sqrt();
                    g.1 += 1;
                }
            });
        }
    });
    let (s, n) = acc.into_inner().unwrap();
    (s / n.max(1) as f64, n)
}

/// Shadow agreement: on charts whose reference luminance is bimodal (lit and
/// shadowed texels), the fraction of texels whose predicted sun visibility
/// (one ray towards `prm.sun_dir`) agrees with the reference's lit/shadowed
/// label. Returns (agreement, texels used). Only charts with a real
/// dark/bright split count; charts without shadows say nothing about the sun.
pub fn shadow_agreement(scene: &Scene, bvh: &Bvh, sel: &[(usize, u32, u32, u32, u32)], atlas: &crate::img::Rgb, prm: &BakeParams) -> (f64, usize) {
    let threads = std::thread::available_parallelism().map(|x| x.get()).unwrap_or(8).min(160);
    let next = std::sync::atomic::AtomicUsize::new(0);
    let acc = std::sync::Mutex::new((0usize, 0usize));
    std::thread::scope(|sc| {
        for _ in 0..threads {
            sc.spawn(|| loop {
                let k = next.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                if k >= sel.len() {
                    break;
                }
                let (ii, px, py, pw, ph) = sel[k];
                let (samples, _) = rasterise_inset(scene, ii, pw, ph, prm.flip_v, prm.uv_bounds, prm.inset_px);
                if samples.len() < 24 {
                    continue;
                }
                let mut lums: Vec<f32> = samples
                    .iter()
                    .map(|s| {
                        let n = atlas.get((px + s.px).min(atlas.w - 1), (py + s.py).min(atlas.h - 1));
                        0.2126 * n[0] as f32 + 0.7152 * n[1] as f32 + 0.0722 * n[2] as f32
                    })
                    .collect();
                let mut sorted = lums.clone();
                sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
                let (lo, hi) = (sorted[sorted.len() / 5], sorted[sorted.len() * 4 / 5]);
                if hi - lo < 40.0 {
                    continue; // no shadow split on this chart
                }
                let thr = 0.5 * (lo + hi);
                let (mut agree, mut n) = (0usize, 0usize);
                for (s, l) in samples.iter().zip(lums.drain(..)) {
                    let o = add(s.p, mul(s.n, 0.03));
                    let facing = dot(s.n, prm.sun_dir) > 0.05;
                    let lit = facing && !bvh.occluded(o, prm.sun_dir, 1.0e4, ii as u32, 0.05);
                    if lit == (l >= thr) {
                        agree += 1;
                    }
                    n += 1;
                }
                let mut g = acc.lock().unwrap();
                g.0 += agree;
                g.1 += n;
            });
        }
    });
    let (a, n) = acc.into_inner().unwrap();
    (a as f64 / n.max(1) as f64, n)
}

/// Pooled (all texels of all selected charts) comparison: our HDR luminance
/// vs the reference atlas luminance × its chart scale. Returns (pearson,
/// slope reference/ours, count). `sel` = (inst, px, py, pw, ph, fb0).
pub fn pooled_fit(scene: &Scene, bvh: &Bvh, sel: &[(usize, u32, u32, u32, u32, u8)], atlas: &crate::img::Rgb, prm: &BakeParams) -> (f64, f64, usize) {
    let threads = std::thread::available_parallelism().map(|x| x.get()).unwrap_or(8).min(160);
    let next = std::sync::atomic::AtomicUsize::new(0);
    let acc = std::sync::Mutex::new(Vec::<(f64, f64)>::new());
    std::thread::scope(|sc| {
        for _ in 0..threads {
            sc.spawn(|| loop {
                let k = next.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                if k >= sel.len() {
                    break;
                }
                let (ii, px, py, pw, ph, fb0) = sel[k];
                let (samples, _) = rasterise_inset(scene, ii, pw, ph, prm.flip_v, prm.uv_bounds, prm.inset_px);
                let mut rng = Rng(0x9E37_79B9_7F4A_7C15 ^ ((ii as u64 + 1) * 0x2545_F491_4F6C_DD1D));
                let mut local = Vec::with_capacity(samples.len());
                for s in &samples {
                    let (e, _, _) = shade(scene, bvh, prm, s, ii as u32, &mut rng);
                    let lum_m = 0.2126 * e[0] + 0.7152 * e[1] + 0.0722 * e[2];
                    let n = atlas.get((px + s.px).min(atlas.w - 1), (py + s.py).min(atlas.h - 1));
                    let lum_n = (0.2126 * n[0] as f32 + 0.7152 * n[1] as f32 + 0.0722 * n[2] as f32) / 255.0 * fb0 as f32 / 255.0;
                    local.push((lum_m as f64, lum_n as f64));
                }
                acc.lock().unwrap().extend(local);
            });
        }
    });
    let v = acc.into_inner().unwrap();
    let n = v.len() as f64;
    if n < 3.0 {
        return (0.0, 0.0, 0);
    }
    let (mx, my) = (v.iter().map(|p| p.0).sum::<f64>() / n, v.iter().map(|p| p.1).sum::<f64>() / n);
    let (mut sxy, mut sxx, mut syy) = (0.0, 0.0, 0.0);
    for (p, q) in &v {
        sxy += (p - mx) * (q - my);
        sxx += (p - mx) * (p - mx);
        syy += (q - my) * (q - my);
    }
    (sxy / (sxx * syy).sqrt().max(1e-12), sxy / sxx.max(1e-12), v.len())
}

/// Least-squares fit of the reference luminance against our per-texel
/// components: ref ≈ a·skyVis + b·(N·L·sunVis) + c. Returns (a, b, c, r²).
pub fn component_fit(scene: &Scene, bvh: &Bvh, sel: &[(usize, u32, u32, u32, u32, u8)], atlas: &crate::img::Rgb, prm: &BakeParams) -> (f64, f64, f64, f64) {
    let (c, r2) = component_fit_rgb(scene, bvh, sel, atlas, prm);
    let lum = |k: usize| 0.2126 * c[0][k] + 0.7152 * c[1][k] + 0.0722 * c[2][k];
    (lum(0), lum(1), lum(3), r2)
}

/// Per-channel least squares: ref_c ≈ a_c·skyVis + b_c·(N·L·sunVis) + k_c. Returns ([r,g,b] × [a,b,k], mean r²).
pub fn component_fit_rgb(scene: &Scene, bvh: &Bvh, sel: &[(usize, u32, u32, u32, u32, u8)], atlas: &crate::img::Rgb, prm: &BakeParams) -> ([[f64; 4]; 3], f64) {
    let mut prm = prm.clone();
    prm.want_bounce = true;
    let prm = &prm;
    let threads = std::thread::available_parallelism().map(|x| x.get()).unwrap_or(8).min(160);
    let next = std::sync::atomic::AtomicUsize::new(0);
    let acc = std::sync::Mutex::new(Vec::<[f64; 9]>::new());
    std::thread::scope(|sc| {
        for _ in 0..threads {
            sc.spawn(|| loop {
                let k = next.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                if k >= sel.len() {
                    break;
                }
                let (ii, px, py, pw, ph, fb0) = sel[k];
                let (samples, _) = rasterise_inset(scene, ii, pw, ph, prm.flip_v, prm.uv_bounds, prm.inset_px);
                let mut rng = Rng(0x9E37_79B9_7F4A_7C15 ^ ((ii as u64 + 1) * 0x2545_F491_4F6C_DD1D));
                let mut local = Vec::with_capacity(samples.len());
                for s in &samples {
                    let sh = shade_full(scene, bvh, prm, s, ii as u32, &mut rng);
                    let ndl = dot(s.n, prm.sun_dir).max(0.0);
                    let n0 = atlas.get((px + s.px).min(atlas.w - 1), (py + s.py).min(atlas.h - 1));
                    // the atlas is sqrt-encoded: E = (p/255)² · fb0/255 (K = 1 units)
                    let n = [crate::synth::decode_value(n0[0], fb0) as f64, crate::synth::decode_value(n0[1], fb0) as f64, crate::synth::decode_value(n0[2], fb0) as f64];
                    let sc = 1.0;
                    let bl = if prm.fit_regressor == 1 { (0.5 + 0.5 * s.n[1]) as f64 } else if prm.fit_regressor == 2 { (sh.sky_vis * sh.sky_vis) as f64 } else { (0.2126 * sh.bounce[0] + 0.7152 * sh.bounce[1] + 0.0722 * sh.bounce[2]) as f64 };
                    local.push([sh.sky_vis as f64, (ndl * sh.sun_vis) as f64, bl, n[0] as f64 * sc, n[1] as f64 * sc, n[2] as f64 * sc, sh.sky_rgb[0] as f64, sh.sky_rgb[1] as f64, sh.sky_rgb[2] as f64]);
                }
                acc.lock().unwrap().extend(local);
            });
        }
    });
    let v = acc.into_inner().unwrap();
    let mut out = [[0f64; 4]; 3];
    let mut r2sum = 0.0;
    let cube = prm.sky_cube.is_some();
    for ch in 0..3 {
        let mut m = [[0f64; 4]; 4];
        let mut r = [0f64; 4];
        for p in &v {
            let x = [if cube { p[6 + ch] } else { p[0] }, p[1], p[2], 1.0];
            for i in 0..4 {
                for j in 0..4 {
                    m[i][j] += x[i] * x[j];
                }
                r[i] += x[i] * p[3 + ch];
            }
        }
        let Some(coef) = solve4(m, r) else { continue };
        let mean = v.iter().map(|p| p[3 + ch]).sum::<f64>() / v.len() as f64;
        let (mut ss_res, mut ss_tot) = (0.0, 0.0);
        for p in &v {
            let x0 = if cube { p[6 + ch] } else { p[0] };
            let pred = coef[0] * x0 + coef[1] * p[1] + coef[2] * p[2] + coef[3];
            ss_res += (p[3 + ch] - pred) * (p[3 + ch] - pred);
            ss_tot += (p[3 + ch] - mean) * (p[3 + ch] - mean);
        }
        out[ch] = coef;
        r2sum += 1.0 - ss_res / ss_tot.max(1e-12);
    }
    (out, r2sum / 3.0)
}

/// The RE-model fit: target = frame_max × decoded texel − LAmbient·(0.8 + 0.2·n.y); regressors
/// [skyVis (or the cube term), sun·vis (0 with direct_sun = 0), bounce estimate, 1].
pub fn component_fit_rgb2(scene: &Scene, bvh: &Bvh, sel: &[(usize, u32, u32, u32, u32, u8)], atlas: &crate::img::Rgb, prm: &BakeParams, frame_max: f32) -> ([[f64; 4]; 3], f64) {
    let mut prm = prm.clone();
    prm.want_bounce = true;
    let prm = &prm;
    let threads = std::thread::available_parallelism().map(|x| x.get()).unwrap_or(8).min(160);
    let next = std::sync::atomic::AtomicUsize::new(0);
    let acc = std::sync::Mutex::new(Vec::<[f64; 9]>::new());
    std::thread::scope(|sc| {
        for _ in 0..threads {
            sc.spawn(|| loop {
                let k = next.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                if k >= sel.len() {
                    break;
                }
                let (ii, px, py, pw, ph, fb0) = sel[k];
                let (samples, _) = rasterise_inset(scene, ii, pw, ph, prm.flip_v, prm.uv_bounds, prm.inset_px);
                let mut rng = Rng(0x9E37_79B9_7F4A_7C15 ^ ((ii as u64 + 1) * 0x2545_F491_4F6C_DD1D));
                let mut local = Vec::with_capacity(samples.len());
                for s in &samples {
                    let sh = shade_full(scene, bvh, prm, s, ii as u32, &mut rng);
                    let ndl = dot(s.n, prm.sun_dir).max(0.0);
                    let n0 = atlas.get((px + s.px).min(atlas.w - 1), (py + s.py).min(atlas.h - 1));
                    let amb = (0.8 + 0.2 * s.n[1]) as f64;
                    let t = |c: usize| (crate::synth::decode_value(n0[c], fb0) * frame_max) as f64;
                    // columns: skyVis, ambient shape (0.8+0.2n.y), unused, target rgb, bounce rgb
                    local.push([sh.sky_vis as f64, amb, (ndl * sh.sun_vis) as f64 * prm.direct_sun as f64, t(0), t(1), t(2), sh.bounce[0] as f64, sh.bounce[1] as f64, sh.bounce[2] as f64]);
                }
                acc.lock().unwrap().extend(local);
            });
        }
    });
    let v = acc.into_inner().unwrap();
    let mut out = [[0f64; 4]; 3];
    let mut r2sum = 0.0;
    for ch in 0..3 {
        let mut m = [[0f64; 4]; 4];
        let mut r = [0f64; 4];
        // regressors: [skyVis, ambient shape, bounce, 1]
        for p in &v {
            let x = [p[0], p[1], p[6 + ch], 1.0];
            for i in 0..4 {
                for j in 0..4 {
                    m[i][j] += x[i] * x[j];
                }
                r[i] += x[i] * p[3 + ch];
            }
        }
        let Some(coef) = solve4(m, r) else { continue };
        let mean = v.iter().map(|p| p[3 + ch]).sum::<f64>() / v.len() as f64;
        let (mut ss_res, mut ss_tot) = (0.0, 0.0);
        for p in &v {
            let pred = coef[0] * p[0] + coef[1] * p[1] + coef[2] * p[6 + ch] + coef[3];
            ss_res += (p[3 + ch] - pred) * (p[3 + ch] - pred);
            ss_tot += (p[3 + ch] - mean) * (p[3 + ch] - mean);
        }
        out[ch] = coef;
        r2sum += 1.0 - ss_res / ss_tot.max(1e-12);
    }
    (out, r2sum / 3.0)
}

fn solve4(mut m: [[f64; 4]; 4], mut r: [f64; 4]) -> Option<[f64; 4]> {
    for c in 0..4 {
        let mut p = c;
        for i in c + 1..4 {
            if m[i][c].abs() > m[p][c].abs() {
                p = i;
            }
        }
        if m[p][c].abs() < 1e-12 {
            return None;
        }
        m.swap(c, p);
        r.swap(c, p);
        for i in 0..4 {
            if i == c {
                continue;
            }
            let f = m[i][c] / m[c][c];
            for j in 0..4 {
                m[i][j] -= f * m[c][j];
            }
            r[i] -= f * r[c];
        }
    }
    Some([r[0] / m[0][0], r[1] / m[1][1], r[2] / m[2][2], r[3] / m[3][3]])
}
