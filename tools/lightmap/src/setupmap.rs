//! THE SETUP CHAIN FROM THE MAP (no captured intermediates): the sun camera fit (B's lightcam.rs on the scene box),
//! the shadow map (B's shadowmap.rs on the map's casters), the direct sun (the baker's sunpass.rs on E's from-map LM
//! meshes / STs), the attribute pre-pass (the nine jittered LM rasters of every LM instance with the transcribed
//! material shaders), PS 17043 → MDiffuse, and the ILightInput chain (PS 1038 / 17043 / 1109 / 1335 × 8) → the 17095
//! atlas the peels colour their fragments from (`ilatlas`) and the sweep-1 transition reads.
//!
//! WHAT IS STILL FROZEN FROM THE CAPTURE (`prepass_check::frozen_tables`, named in the log line): the collection's
//! terrain material (the texture array 5354 / 5363 / 5367 and the per-slice buffers 5352 / 5361 / 5365 behind PS 8401 —
//! the tile and wall constants at the pre-pass's zero world matrix), the pad material's four textures (PS 17025), and the
//! water pass's inputs (the water-id map, the plane-top / depth tables, the fog and transmittance LUTs 15075 / 15078 of
//! the mood). They are collection / zone data, not map data; their pak-side derivation is the open row.
//!
//! Everything else here is computed from the map + the item files: the camera, the casters, the LM streams and STs,
//! the item textures (the zip's DDS, bottom-up as the game uploads them), the jitters, the accumulation, the chain.

use crate::lightcam::{fit_camera, Aabb, FitRules, OrthoCamera};
use crate::lmaccum::{LmRasterCb, LmScene};
use crate::passdiff::Buf;
use crate::prepass::{self, Target, H, W};
use crate::prepass_check::FrozenTables;
use crate::shadowmap::{self, CasterDraw, CasterMesh, InstanceTables, LightCamera, PlaneEval, RasterState, ShadowTarget};
use crate::sunpass::{self, SunDraw};
use crate::texsample::{self, Bc1Decode, Texture};

/// THE BAKE'S WARNINGS (E, 2026-09-27): setup findings that make the output NOT a lighting result — a material in none of the
/// --pak files (its triangles go black) — collected here, printed again at the end of the bake, fatal under `--strict`.
pub static WARNINGS: std::sync::Mutex<Vec<String>> = std::sync::Mutex::new(Vec::new());

pub struct FromMap {
    pub cam: OrthoCamera,
    pub pw01: [[f32; 4]; 4],
    pub shadow: Buf,
    pub sun: Buf,
    /// 16963 after the nine runs (RGBA16F values).
    pub attr: Buf,
    /// 16969 (sRGB UNORM8 values as 0..1 floats, alpha linear).
    pub mdiffuse8: Buf,
    pub ilightinput: Buf,
    pub coverage: Buf,
    pub notes: Vec<String>,
}

/// The shadow pass's pipeline state (frame 127448 eids 353…: viewport (1, 1, 4094, 4094), bias −1 / −1.0 / 0, cull
/// back, front CCW, depth clip) — pipeline constants, not scene data.
pub fn shadow_state() -> RasterState {
    // LMTOOL_SUNMAP_SLOPE_BIAS=S (study): the caster pass's SlopeScaledDepthBias (the capture's −1.0) — the grazing-incidence test on
    // stpad's pool walls (a vertical caster under a 13.6° sun has a huge depth slope; the uncapped bias moves its stored depth metres)
    static SLOPE_BIAS: std::sync::LazyLock<f32> = std::sync::LazyLock::new(|| std::env::var("LMTOOL_SUNMAP_SLOPE_BIAS").ok().and_then(|v| v.parse().ok()).unwrap_or(-1.0));
    RasterState { viewport: [1.0, 1.0, 4094.0, 4094.0, 0.0, 1.0], depth_bias: -1, slope_scaled_depth_bias: *SLOPE_BIAS, depth_bias_clamp: 0.0, cull_back: true, front_ccw: true, depth_clip: true, plane: PlaneEval::F64Snapped, coef_bits: 36, vertex_z_bits: 0 }
}

/// `GbxShadowAlphaThreshold` of the alpha-tested caster draws (PS 1147): 128/255.
pub const SHADOW_ALPHA_THRESHOLD: f32 = 0.501960813999176;

/// The VisualToWorld rows (4 × 3, as the DrawV log prints them) of a mapgeom placement.
pub fn rows_of_xform(m: &[f32; 12]) -> [[f32; 3]; 4] {
    [[m[0], m[1], m[2]], [m[3], m[4], m[5]], [m[6], m[7], m[8]], [m[9], m[10], m[11]]]
}

/// The sun camera: the fit on the scene box along the light's travel direction.
pub fn sun_camera(sbox: &Aabb, dir_in_world: [f32; 3]) -> OrthoCamera {
    fit_camera(sbox, dir_in_world, &FitRules::default())
}

/// The shadow map from the map: every item instance's visual mesh (the alpha-tested materials through B's alpha test
/// with the zip's cut-out texture), every zone tile (the LM tile mesh at the instance's translation), the decoration.
pub fn shadow_from_map(scene: &crate::geometry::Scene, lm: &LmScene, cam: &OrthoCamera, item_bytes: &dyn Fn(&str) -> Option<Vec<u8>>, notes: &mut Vec<String>) -> ShadowTarget {
    // THE CASTER JOBS in the sequential draw order: items (one opaque caster per instance + one alpha-tested caster per cut-out
    // texture), the zone tiles (the LM tile mesh at each tile instance), the decoration chunks
    #[derive(Clone)]
    enum Job { Item(usize), Tile(usize, usize), Decor(Vec<usize>) }
    let mut jobs: Vec<Job> = Vec::new();
    let mut weight: Vec<usize> = Vec::new();
    for (ii, inst) in scene.instances.iter().enumerate() { jobs.push(Job::Item(ii)); weight.push(scene.models[inst.model].tris.len().max(1)); }
    for (k, mesh) in lm.meshes.iter().enumerate() {
        if lm.inst_count[k] < 1000 { continue; }
        for ii in lm.inst_first[k]..lm.inst_first[k] + lm.inst_count[k] { jobs.push(Job::Tile(k, ii)); weight.push(mesh.indices.len() / 3); }
    }
    let mut skipped_water = 0usize;
    if !scene.decor.is_empty() {
        let mut chunk: Vec<usize> = Vec::new();
        // LMTOOL_SUN_SKIP_DECOR=env|seabed|all (study, G2 2026-09-28 — the g23 outer tiles' sun leak): leave the env block's casters
        // (the sea box; `env`) or the zone floor quads (`!env`) out of the sun map
        let skip_env = std::env::var("LMTOOL_SUN_SKIP_DECOR").map(|v| v == "env" || v == "all").unwrap_or(false);
        let skip_floor = std::env::var("LMTOOL_SUN_SKIP_DECOR").map(|v| v == "seabed" || v == "all").unwrap_or(false);
        for (i, t) in scene.decor.iter().enumerate() {
            if t.water || !t.sun_caster || (t.env && skip_env) || (!t.env && skip_floor) { skipped_water += 1; continue; }
            if chunk.len() * 3 + 3 > 65535 { jobs.push(Job::Decor(std::mem::take(&mut chunk))); weight.push(21845); }
            chunk.push(i);
        }
        if !chunk.is_empty() { weight.push(chunk.len()); jobs.push(Job::Decor(chunk)); }
    }
    let lcam = LightCamera { world_pr_camera: cam.world_pr_camera() };
    let st = shadow_state();
    let o = shadowmap::RunOpts { arith: shadowmap::Arith::Fma, unorm: shadowmap::UnormRounding::Nearest, alpha_test: true, depth_fixed_bits: 0, fixed_before_bias: false, step_fixed_k: 20, bias_round: 3, scale_ulps: 0.0 };
    // THE WORKERS: contiguous job ranges balanced by triangle count, each into its own target; the merge keeps, per pixel, the
    // greater 16-bit depth and on a tie the EARLIER worker's fragment — exactly the sequential draw's strict `q > depth` test
    // (within a worker the first fragment with that depth already won). LMTOOL_SHADOW_WORKERS=N (default 16, 1 = the serial pass).
    let total: usize = weight.iter().sum();
    let workers: usize = std::env::var("LMTOOL_SHADOW_WORKERS").ok().and_then(|v| v.parse().ok()).unwrap_or(16).clamp(1, jobs.len().max(1));
    let mut ranges: Vec<(usize, usize)> = Vec::new();
    {
        let mut start = 0usize;
        let mut acc = 0usize;
        for (k, wgt) in weight.iter().enumerate() {
            acc += wgt;
            if acc * workers >= total * (ranges.len() + 1) && ranges.len() + 1 < workers { ranges.push((start, k + 1)); start = k + 1; }
        }
        if start < jobs.len() { ranges.push((start, jobs.len())); }
    }
    // the cut-out textures, loaded once up front (the byte source is not Sync)
    let mut alpha_cache: std::collections::HashMap<String, Option<std::sync::Arc<shadowmap::AlphaTexture>>> = std::collections::HashMap::new();
    for inst in &scene.instances {
        let model = &scene.models[inst.model];
        for t in &model.tris {
            if t.alpha == u16::MAX { continue; }
            let name = model.alpha_tex.get(t.alpha as usize).cloned().unwrap_or_default();
            alpha_cache.entry(name.clone()).or_insert_with(|| item_bytes(&name).and_then(|b| shadowmap::AlphaTexture::from_dds(&b).ok()).map(std::sync::Arc::new));
        }
    }
    let alpha_cache = &alpha_cache;
    let draw_job = |job: &Job, tgt: &mut ShadowTarget, n_draws: &mut usize, notes: &mut Vec<String>| {
        let empty = InstanceTables { dyna_u32: Vec::new(), static_meshs: Vec::new() };
        match job {
            Job::Item(ii) => {
                let inst = &scene.instances[*ii];
                let model = &scene.models[inst.model];
                // LMTOOL_STOCK_VEGET_NOCAST=1 (study): the stock trees left out of the sun shadow map
                if model.veget.is_some() && std::env::var_os("LMTOOL_STOCK_VEGET_NOCAST").is_some() { return; }
                let rows = rows_of_xform(&inst.xf);
                let mut opaque = CasterMesh { pos: Vec::new(), uv0: Vec::new(), indices: Vec::new() };
                let mut cut: std::collections::BTreeMap<u16, CasterMesh> = std::collections::BTreeMap::new();
                // a stock tree's material (stockveg): the rule of its mask — a bark material casts OPAQUE (Tree_SelfAO_Shadow_p blob 0 = `ret`),
                // a leaf material through its own 0.3 bilinear test
                let veget_rule = |a: u16| -> Option<std::sync::Arc<crate::stockveg::VegetRule>> { model.alpha_tex.get(a as usize).and_then(|n| scene.alpha_masks.get(n)).and_then(|mk| mk.veget.clone()) };
                for t in &model.tris {
                    let bark = t.alpha != u16::MAX && veget_rule(t.alpha).map(|r| !r.leaf).unwrap_or(false);
                    let m = if t.alpha == u16::MAX || bark { &mut opaque } else { cut.entry(t.alpha).or_insert_with(|| CasterMesh { pos: Vec::new(), uv0: Vec::new(), indices: Vec::new() }) };
                    let base = m.pos.len() as u16;
                    for k in 0..3 {
                        m.pos.push(t.p[k]);
                        m.uv0.push(t.uv0[k]);
                    }
                    m.indices.extend_from_slice(&[base, base + 1, base + 2]);
                }
                if !opaque.indices.is_empty() {
                    let d = CasterDraw { eid: *n_draws as u64, mesh: CasterMesh { uv0: Vec::new(), ..opaque }, instance_start: 0xffff_ffff, instance_count: 1, visual_to_world: Some(rows), tables: InstanceTables { dyna_u32: Vec::new(), static_meshs: Vec::new() }, alpha: None, vsout: None };
                    shadowmap::draw_caster(&d, &lcam, &st, tgt, 1, &o);
                    *n_draws += 1;
                }
                for (a, mesh) in cut {
                    let name = model.alpha_tex.get(a as usize).cloned().unwrap_or_default();
                    if let Some(rule) = veget_rule(a) {
                        let d = CasterDraw { eid: *n_draws as u64, mesh, instance_start: 0xffff_ffff, instance_count: 1, visual_to_world: Some(rows), tables: InstanceTables { dyna_u32: Vec::new(), static_meshs: Vec::new() }, alpha: Some(shadowmap::AlphaTest::veget(rule)), vsout: None };
                        shadowmap::draw_caster(&d, &lcam, &st, tgt, 2, &o);
                        *n_draws += 1;
                        continue;
                    }
                    let tex = alpha_cache.get(&name).cloned().flatten();
                    let Some(texture) = tex else { notes.push(format!("shadow: cut-out texture {name} not loaded — the caster is skipped")); continue };
                    let texture = shadowmap::AlphaTexture { w: texture.w, h: texture.h, mips: texture.mips.clone() };
                    let d = CasterDraw { eid: *n_draws as u64, mesh, instance_start: 0xffff_ffff, instance_count: 1, visual_to_world: Some(rows), tables: InstanceTables { dyna_u32: Vec::new(), static_meshs: Vec::new() }, alpha: Some(shadowmap::AlphaTest::cards(SHADOW_ALPHA_THRESHOLD, texture, 16.0)), vsout: None };
                    shadowmap::draw_caster(&d, &lcam, &st, tgt, 2, &o);
                    *n_draws += 1;
                }
            }
            Job::Tile(k, ii) => {
                let mesh = &lm.meshes[*k];
                let inst = &lm.instances[*ii];
                let rows = [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0], inst.t];
                let d = CasterDraw { eid: *n_draws as u64, mesh: CasterMesh { pos: mesh.verts.iter().map(|v| v.pos).collect(), uv0: Vec::new(), indices: mesh.indices.clone() }, instance_start: 0xffff_ffff, instance_count: 1, visual_to_world: Some(rows), tables: InstanceTables { dyna_u32: Vec::new(), static_meshs: Vec::new() }, alpha: None, vsout: None };
                shadowmap::draw_caster(&d, &lcam, &st, tgt, 3, &o);
                *n_draws += 1;
            }
            Job::Decor(idx) => {
                let mut m = CasterMesh { pos: Vec::new(), uv0: Vec::new(), indices: Vec::new() };
                for &i in idx {
                    let t = &scene.decor[i];
                    let base = m.pos.len() as u16;
                    m.pos.extend_from_slice(&t.p);
                    m.indices.extend_from_slice(&[base, base + 1, base + 2]);
                }
                let d = CasterDraw { eid: *n_draws as u64, mesh: m, instance_start: 0xffff_ffff, instance_count: 1, visual_to_world: Some([[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0], [0.0; 3]]), tables: empty, alpha: None, vsout: None };
                shadowmap::draw_caster(&d, &lcam, &st, tgt, 4, &o);
                *n_draws += 1;
            }
        }
    };
    let results: Vec<(ShadowTarget, usize, Vec<String>)> = crate::pool::pool().map(ranges.len(), |r| {
        let (a, b) = ranges[r];
        let mut tgt = ShadowTarget::new(4096, 4096);
        let mut n = 0usize;
        let mut nts = Vec::new();
        for job in &jobs[a..b] { draw_job(job, &mut tgt, &mut n, &mut nts); }
        (tgt, n, nts)
    });
    let mut n_draws = 0usize;
    let mut it = results.into_iter();
    let (mut tgt, n0, nts0) = it.next().unwrap_or_else(|| (ShadowTarget::new(4096, 4096), 0, Vec::new()));
    n_draws += n0;
    notes.extend(nts0);
    for (t, n, nts) in it {
        n_draws += n;
        notes.extend(nts);
        for i in 0..tgt.depth.len() {
            if t.depth[i] > tgt.depth[i] {
                tgt.depth[i] = t.depth[i];
                tgt.source[i] = t.source[i];
                tgt.raw[i] = t.raw[i];
                tgt.slope[i] = t.slope[i];
                tgt.dxy[i] = t.dxy[i];
            }
        }
    }
    notes.push(format!("shadow: {n_draws} caster draws ({} item instances, {} decoration triangles, {skipped_water} water / peel-only triangles not cast; {} workers)", scene.instances.len(), scene.decor.len(), ranges.len()));
    tgt
}

/// The direct sun: every LM mesh × the nine raster offsets (OutScale 1/9) on our shadow map.
pub fn sun_from_map(lm: &LmScene, pw01: &[[f32; 4]; 4], dir_in_world: [f32; 3], light_rgb: [f32; 3], shadow: &Buf) -> Buf {
    sun_from_map_skip(lm, pw01, dir_in_world, light_rgb, shadow, &[])
}

/// `sun_from_map` with LM meshes left out of the direct-sun draws (`skip` = mesh indices; LMTOOL_STOCK_VEGET_RECV_SUN=0's study:
/// the charted legacy trees' receiver meshes get no D_0 sun — the test of whether the game's RenderLightDirect draws them).
pub fn sun_from_map_skip(lm: &LmScene, pw01: &[[f32; 4]; 4], dir_in_world: [f32; 3], light_rgb: [f32; 3], shadow: &Buf, skip: &[usize]) -> Buf {
    let mut draws = Vec::new();
    for j in 0..9usize {
        let cb = LmRasterCb::for_offset(j, W, H);
        for (k, _) in lm.meshes.iter().enumerate() {
            if skip.contains(&k) { continue; }
            draws.push(SunDraw { eid: (j * lm.meshes.len() + k) as u64, mesh: k, instance_first: lm.inst_first[k], instance_count: lm.inst_count[k], scale_ss: cb.scale_ss, trans_ss: cb.trans_ss, world_pw01_shadow: *pw01, dir_in_world, light_rgb, out_scale: 1.0 / 9.0 });
        }
    }
    let sm = sunpass::ShadowMap { depth: shadow };
    // the parallel pass (bit-identical: the same per-pixel fragment order; LMTOOL_SUN_SERIAL=1 keeps the serial one)
    let t = if std::env::var_os("LMTOOL_SUN_SERIAL").is_some() { sunpass::run_sun_pass(&lm.meshes, &lm.instances, &lm.table, &draws, &sm, W, H, sunpass::BlendModel::TruncSrcRoundSum) } else { sunpass::run_sun_pass_par(&lm.meshes, &lm.instances, &lm.table, &draws, &sm, W, H, sunpass::BlendModel::TruncSrcRoundSum) };
    let mut b = Buf::new(t.w, t.h, 4);
    for i in 0..t.px.len() {
        for c in 0..4 {
            b.data[i * 4 + c] = t.px[i][c];
        }
    }
    b
}

/// How the pre-pass shades an item's material: the transcribed pixel shader it runs.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum MatClass {
    /// PS 17023: the diffuse texture at TexCoord0 (the trunk).
    Textured,
    /// PS 17022 with AlphaToCoverage on the 1-sample target: the leaf cards write nothing (alpha 1/9 < 0.5).
    CutOut,
    /// PS 17025 at the zero world matrix: the pad constant.
    Pad,
    /// PS 8401 at the zero world matrix with the slices (0, 0): the wall constant.
    Wall,
}

/// LMTOOL_PREPASS_CARDS=textured (a STUDY switch, hill4's jungle cards): the alpha-tested card triangles run the textured
/// path (PS 17023 with the cut-out file as the diffuse and the 128/255 alpha test) instead of the AlphaToCoverage path that
/// writes nothing — the question being whether the TDOSN material model's cards carry albedo into MDiffuse (the palm's did
/// not, per the pwc-day capture). The card's diffuse texture index is flagged 0x8000 | its alpha-texture index.
pub fn cards_textured() -> bool {
    static V: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *V.get_or_init(|| std::env::var("LMTOOL_PREPASS_CARDS").map(|v| v == "textured").unwrap_or(false))
}

/// The diffuse texture file of a pre-pass triangle: the model's diffuse list, or (flag 0x8000) its cut-out list.
pub fn diff_name(model: &crate::geometry::ModelGeom, diff: u16) -> String {
    if diff & 0x8000 != 0 { model.alpha_tex.get((diff & 0x7fff) as usize).cloned().unwrap_or_default() }
    else if diff & 0x4000 != 0 { format!("link:{}", model.mat_links.get((diff & 0x3fff) as usize).map(|l| l.to_ascii_lowercase()).unwrap_or_default()) }
    else { model.diff_tex.get(diff as usize).cloned().unwrap_or_default() }
}

/// A triangle's diffuse index for the pre-pass (a card under the study switch takes its cut-out texture, flagged 0x8000; a
/// linked textured material's triangle takes its link, flagged 0x4000 — `diff_name` gives "link:<link>").
pub fn diff_index(t: &crate::geometry::Tri) -> u16 {
    if t.alpha != u16::MAX && cards_textured() { 0x8000 | t.alpha } else if t.diff == u16::MAX && t.mat != u16::MAX && (t.mat as u32) < 0x4000 { 0x4000 | t.mat } else { t.diff }
}

pub fn classify(model: &crate::geometry::ModelGeom, name: &str, t: &crate::geometry::Tri) -> MatClass {
    classify_with(model, name, t, None)
}

/// `classify` knowing the linked textured materials (`FrozenTables::link_tex`): a triangle of such a material is Textured.
pub fn classify_with(model: &crate::geometry::ModelGeom, name: &str, t: &crate::geometry::Tri, link_tex: Option<&std::collections::HashMap<String, (Texture, bool)>>) -> MatClass {
    if t.alpha != u16::MAX {
        return if cards_textured() { MatClass::Textured } else { MatClass::CutOut };
    }
    if t.diff != u16::MAX {
        return MatClass::Textured;
    }
    if let Some(lt) = link_tex {
        if let Some(l) = model.mat_links.get(t.mat as usize) { if lt.contains_key(&l.to_ascii_lowercase()) { return MatClass::Textured; } }
    }
    // the game materials of pwc-day's items: `BlueBay\Media\Material\Land` runs PS 8401 with the slices (0, 0) (the capture's
    // 24-index draw, the RE's "wall" class), `…\Modifier\StadiumOnTerrain\TrackWallInWorld` runs PS 17025 (the 12-index draw,
    // the RE's "pad" class)
    let link = model.mat_links.get(t.mat as usize).map(|s| s.to_ascii_lowercase()).unwrap_or_default();
    let _ = name;
    if link.contains("trackwall") || link.contains("\\modifier\\") {
        MatClass::Pad
    } else {
        MatClass::Wall
    }
}

/// The attribute pre-pass from the map: nine jittered runs of every LM instance (items with their material shader,
/// tiles with the terrain constant, then the water tint), accumulated with PS 1109 into 16963.
pub fn attr_from_map(scene: &crate::geometry::Scene, lm: &LmScene, frozen: &FrozenTables, item_bytes: &dyn Fn(&str) -> Option<Vec<u8>>, notes: &mut Vec<String>) -> Buf {
    let sampler = frozen.sampler;
    // the diffuse textures, loaded once up front (the byte source is not Sync; the nine runs read them in parallel)
    let mut tex_cache: std::collections::HashMap<String, Option<Texture>> = std::collections::HashMap::new();
    let link_tex = Some(&frozen.link_tex);
    for (l, (tx, _)) in &frozen.link_tex { tex_cache.insert(format!("link:{l}"), Some(tx.clone())); }
    let link_alpha_tested: std::collections::HashSet<String> = frozen.link_tex.iter().filter(|(_, (_, at))| *at).map(|(l, _)| format!("link:{l}")).collect();
    for inst in &scene.instances {
        let model = &scene.models[inst.model];
        let name = &scene.model_names[inst.model];
        for t in &model.tris {
            if classify_with(model, name, t, link_tex) != MatClass::Textured { continue; }
            let tn = diff_name(model, diff_index(t));
            tex_cache.entry(tn.clone()).or_insert_with(|| item_bytes(&tn).and_then(|b| texsample::parse_dds(&b, Bc1Decode::Expand8Round).ok()).map(|mut tx| { tx.decode_srgb(); tx }));
        }
    }
    let tex_cache = &tex_cache;
    let mut acc = Buf::new(W, H, 4);
    let lm_scale = 1.0f32 / 9.0;
    // the items: E's LM meshes (the game's own LM stream: exact TexCoord1) per model in first-appearance order; the material
    // of an LM triangle is looked up in the port's model triangles by vertex position (uv0 / the texture / the cut-out)
    let mut by_model: Vec<usize> = Vec::new();
    for inst in &scene.instances {
        if !by_model.contains(&inst.model) {
            by_model.push(inst.model);
        }
    }
    let key = |p: [f32; 3]| -> [i32; 3] { [(p[0] * 1024.0).round() as i32, (p[1] * 1024.0).round() as i32, (p[2] * 1024.0).round() as i32] };
    struct ItemMat { class: MatClass, uv0: [f32; 2], diff: u16, uv1: [f32; 2] }
    // each LM mesh is paired with the port model whose triangle vertices it shares (E's mesh order is by model, but the
    // pairing is made on the geometry itself)
    let lookups: Vec<(usize, std::collections::HashMap<[i32; 3], ItemMat>)> = by_model.iter().map(|&model_idx| {
        let model = &scene.models[model_idx];
        let name = &scene.model_names[model_idx];
        let mut map: std::collections::HashMap<[i32; 3], ItemMat> = std::collections::HashMap::new();
        for t in &model.tris {
            let class = classify_with(model, name, t, link_tex);
            for v in 0..3 {
                map.entry(key(t.p[v])).or_insert(ItemMat { class, uv0: t.uv0[v], diff: diff_index(t), uv1: t.uv[v] });
            }
        }
        (model_idx, map)
    }).collect();
    // (perf 8: the pairing was every LM mesh against every model's map — 455 × 455 × 13 k vertex lookups, 12.6 s of the tiny
    // map's setup; one multimap vertex key → the models holding it, a vote per vertex, the same argmax with the same tie
    // rule — `max_by_key` keeps the LAST of equal maxima — and the meshes in parallel)
    let mut by_key: std::collections::HashMap<[i32; 3], Vec<u32>> = std::collections::HashMap::new();
    for (li, (_, map)) in lookups.iter().enumerate() {
        for k in map.keys() { by_key.entry(*k).or_default().push(li as u32); }
    }
    let by_key = &by_key;
    let candidates: Vec<usize> = lm.meshes.iter().enumerate().filter(|(k, _)| lm.inst_count[*k] < 1000).map(|(k, _)| k).collect();
    let best_of: Vec<usize> = crate::pool::pool().map(candidates.len(), |ci| {
        let mesh = &lm.meshes[candidates[ci]];
        let mut votes = vec![0usize; lookups.len()];
        for v in &mesh.verts {
            if let Some(list) = by_key.get(&key(v.pos)) { for &li in list { votes[li as usize] += 1; } }
        }
        let mut best = 0usize;
        for li in 0..lookups.len() { if votes[li] >= votes[best] { best = li; } }
        best
    });
    let item_meshes: Vec<(usize, usize)> = candidates.iter().zip(best_of).map(|(&k, best)| (k, best)).collect();
    for (mk, li) in &item_meshes {
        let (model_idx, map) = &lookups[*li];
        let mesh = &lm.meshes[*mk];
        let hits = mesh.verts.iter().filter(|v| map.contains_key(&key(v.pos))).count();
        notes.push(format!("item mesh {mk} ↔ model {} ({}): {} of {} LM verts matched by position ({} port triangles)", model_idx, scene.model_names[*model_idx], hits, mesh.verts.len(), scene.models[*model_idx].tris.len()));
        notes.push(format!("  model {} materials: links {:?}, diffuse textures {:?}, cut-out textures {:?}; LM uv range {:?}", model_idx, scene.models[*model_idx].mat_links, scene.models[*model_idx].diff_tex, scene.models[*model_idx].alpha_tex, mesh.verts.iter().fold(([f32::MAX; 2], [f32::MIN; 2]), |(lo, hi), v| ([lo[0].min(v.uv[0]), lo[1].min(v.uv[1])], [hi[0].max(v.uv[0]), hi[1].max(v.uv[1])]))));
    }
    // THE NINE RUNS × THE PIXEL-ROW BANDS IN PARALLEL (perf 8: nine tasks, one per run, kept nine threads busy for 16 s on the
    // tiny map, 36 s on the giant). Per mesh the triangles' materials are resolved once (below: E's per-link constants, the
    // textured sampling, the alpha-test rule — the same values, no longer a String and a hash lookup per instance per triangle
    // per run, nor per fragment); then every (run, band) task walks every instance's triangles in the draw order and
    // rasterises the rows of its band — a texel's fragments arrive in the same order as in the serial run, so the f16 blend
    // chain is bit-identical; the accumulation into 16963 follows in run order per texel.
    // 1. per mesh: the port model it pairs with and its triangles' (LM uv, uv0, class, texture, constant, class id, alpha test)
    // `hue`: the triangle's material has recoloured constants in frozen.hue_rgb (a HueMask material) — the instance's MapElemColor
    // picks one at raster time (colour 0 = the plain constant)
    struct TriDraw<'a> { uv: [[f32; 2]; 3], uv0: [[f32; 2]; 3], class: MatClass, tex: Option<&'a Texture>, konst: Option<[f32; 3]>, cls: u8, at: Option<f32>, hue: Option<[[f32; 3]; 6]>, hue_tex: Option<&'a (Texture, [[f32; 3]; 6])> }
    let mut class_count = [0usize; 4];
    let mut mesh_tris: Vec<Vec<TriDraw>> = Vec::with_capacity(item_meshes.len());
    {
        let k = 0usize;
        for (mk, li) in item_meshes.iter() {
            let mesh = &lm.meshes[*mk];
            let (model_idx, lookup) = &lookups[*li];
            let model_idx = *model_idx;
            let model = &scene.models[model_idx];
            let name = &scene.model_names[model_idx];
            let default_class = if model.mat_links.iter().any(|l| { let l = l.to_ascii_lowercase(); l.contains("trackwall") || l.contains("\\modifier\\") }) { MatClass::Pad } else { MatClass::Wall };
            // the pre-pass draws the VISUAL stream (VS 17021 / 17024 / 8400: its f32 TexCoord1 / TexCoord0), the H-basis passes the
            // snorm16 LM stream. When the port's model carries the same lightmap uv as the LM stream (the tree: TexCoord1), the
            // port's f32 triangles are rasterised (the LM stream's quantised uv moves trunk-edge texels); a model without lightmap
            // uvs in the port's reading (Land, TrackWall: the port falls back to TexCoord0 / a planar map) takes E's LM mesh.
            let port_uv_matches = {
                // every LM vertex must find a port vertex at its position with the same lightmap uv (a seam vertex carries several)
                let mut uvs_at: std::collections::HashMap<[i32; 3], Vec<[f32; 2]>> = std::collections::HashMap::new();
                for t in &model.tris { for v in 0..3 { uvs_at.entry(key(t.p[v])).or_default().push(t.uv[v]); } }
                let (mut n, mut ok) = (0usize, 0usize);
                for v in &mesh.verts {
                    if let Some(list) = uvs_at.get(&key(v.pos)) { n += 1; if list.iter().any(|u| (u[0] - v.uv[0]).abs() < 2e-3 && (u[1] - v.uv[1]).abs() < 2e-3) { ok += 1; } }
                }
                if k == 0 { notes.push(format!("item mesh {mk}: {ok} of {n} LM vertices have a port vertex with the same lightmap uv (±2e-3)")); }
                n > 0 && ok * 10 >= n * 9
            };
            if k == 0 { notes.push(format!("item mesh {mk} ({}): the port's TexCoord1 {} the LM stream's uv → the pre-pass rasterises {}", name, if port_uv_matches { "matches" } else { "does not match" }, if port_uv_matches { format!("the port's {} f32 triangles", model.tris.len()) } else { format!("E's LM mesh ({} triangles)", mesh.indices.len() / 3) })); }
            let _ = k;
            // the triangles to draw: (positions-in-LM-space uv, uv0, class, diff)
            let tris: Vec<([[f32; 2]; 3], [[f32; 2]; 3], MatClass, u16)> = if port_uv_matches {
                model.tris.iter().map(|t| (t.uv, t.uv0, classify_with(model, name, t, link_tex), diff_index(t))).collect()
            } else {
                mesh.indices.chunks_exact(3).map(|tri| {
                    let vs = [&mesh.verts[tri[0] as usize], &mesh.verts[tri[1] as usize], &mesh.verts[tri[2] as usize]];
                    let mats: [Option<&ItemMat>; 3] = [lookup.get(&key(vs[0].pos)), lookup.get(&key(vs[1].pos)), lookup.get(&key(vs[2].pos))];
                    let class = mats.iter().flatten().next().map(|m| m.class).unwrap_or(default_class);
                    let uv0 = [mats[0].map(|m| m.uv0).unwrap_or([0.0; 2]), mats[1].map(|m| m.uv0).unwrap_or([0.0; 2]), mats[2].map(|m| m.uv0).unwrap_or([0.0; 2])];
                    let diff = mats.iter().flatten().next().map(|m| m.diff).unwrap_or(u16::MAX);
                    ([vs[0].uv, vs[1].uv, vs[2].uv], uv0, class, diff)
                }).collect()
            };
            let n_inst = lm.inst_count[*mk];
            let draws: Vec<TriDraw> = tris.iter().map(|(uv, uv0, class, diff)| {
                let tex = match class {
                    MatClass::Textured => {
                        let tn = diff_name(model, *diff);
                        tex_cache.get(&tn).and_then(|t| t.as_ref())
                    }
                    _ => None,
                };
                // the constant per LINK when the pack gave one (diff carries the material index for a linked material), else the class global;
                // a slot with a TargetColor instance override has its own constant (`ModelGeom::link_key`)
                let link_const = if *diff & 0xC000 == 0x4000 { let slot = (*diff & 0x3fff) as usize; if slot < model.mat_links.len() { frozen.link_rgb.get(&model.link_key(slot)).copied() } else { None } } else { None };
                let konst = match class {
                    MatClass::Pad => Some(link_const.unwrap_or(frozen.pad_rgb)),
                    MatClass::Wall => Some(link_const.unwrap_or(frozen.wall_rgb)),
                    _ => None,
                };
                let hue: Option<[[f32; 3]; 6]> = if *diff & 0xC000 == 0x4000 && konst.is_some() && std::env::var_os("LMTOOL_NO_HUE_RECOLOUR").is_none() {
                    model.mat_links.get((*diff & 0x3fff) as usize).and_then(|l| {
                        let lc = l.to_ascii_lowercase();
                        if !frozen.hue_rgb.contains_key(&(lc.clone(), 4)) { return None; }
                        let mut t = [konst.unwrap(); 6];
                        for c in 1u8..=5 { if let Some(v) = frozen.hue_rgb.get(&(lc.clone(), c)) { t[c as usize] = *v; } }
                        Some(t)
                    })
                } else { None };
                let cls = match class { MatClass::Textured => 6u8, MatClass::CutOut => 7, MatClass::Pad => 3, MatClass::Wall => 4 };
                // a card under the study switch: the 128/255 alpha test (GbxShadowAlphaThreshold) discards the cut-out
                let at = if *diff & 0x8000 != 0 || (*diff & 0xC000 == 0x4000 && link_alpha_tested.contains(&diff_name(model, *diff))) { Some(SHADOW_ALPHA_THRESHOLD) } else { None };
                // (the class census of run 0: once per instance and triangle, as the serial run counted)
                class_count[match class { MatClass::Textured => 0, MatClass::CutOut => 1, MatClass::Pad => 2, MatClass::Wall => 3 }] += n_inst;
                // a textured HueMask material: the mask + targets ride with the triangle; the instance's colour picks the target at raster time
                let hue_tex = if matches!(class, MatClass::Textured) && *diff & 0xC000 == 0x4000 && std::env::var_os("LMTOOL_NO_HUE_RECOLOUR").is_none() { model.mat_links.get((*diff & 0x3fff) as usize).and_then(|l| frozen.link_hue.get(&l.to_ascii_lowercase())) } else { None };
                TriDraw { uv: *uv, uv0: *uv0, class: *class, tex, konst, cls, at, hue, hue_tex }
            }).collect();
            if std::env::var_os("LMTOOL_HUE_TRACE").is_some() { let nh = draws.iter().filter(|d| d.hue.is_some()).count(); if nh > 0 || name.contains("AC062201") { eprintln!("hue-trace: mesh {mk} ({name}): {nh} of {} triangles carry a HueMask recolour table; instances {} colours {:?}", draws.len(), lm.inst_count[*mk], (lm.inst_first[*mk]..lm.inst_first[*mk] + lm.inst_count[*mk]).map(|ii| lm.port_inst.get(ii).copied().filter(|p| *p != usize::MAX).and_then(|p| scene.instances.get(p)).map(|si| si.colour).unwrap_or(99)).collect::<Vec<_>>()); } }
            mesh_tris.push(draws);
        }
    }
    let mesh_tris = &mesh_tris;
    // the LM instances' placement colours (through their port instance; entities and tiles 0) for the HueMask recolour
    let inst_colour: Vec<u8> = (0..lm.instances.len()).map(|ii| lm.port_inst.get(ii).copied().filter(|p| *p != usize::MAX).and_then(|p| scene.instances.get(p)).map(|si| si.colour).unwrap_or(0)).collect();
    let inst_colour = &inst_colour;
    // LMTOOL_TILE_ALBEDO_SCALE=K (study): the tiles' sampled albedo scaled — the ground-bounce lever test on stpad's posts
    static ASCALE: std::sync::LazyLock<f32> = std::sync::LazyLock::new(|| std::env::var("LMTOOL_TILE_ALBEDO_SCALE").ok().and_then(|v| v.parse().ok()).unwrap_or(1.0));
    static GRASS_NO2: std::sync::LazyLock<bool> = std::sync::LazyLock::new(|| std::env::var_os("LMTOOL_GRASS_NO2").is_some());
    static GRASS_TEXTURED: std::sync::LazyLock<bool> = std::sync::LazyLock::new(|| std::env::var("LMTOOL_GRASS_X2").as_deref() == Ok("textured"));
    static TILE_TRACE: std::sync::LazyLock<bool> = std::sync::LazyLock::new(|| std::env::var_os("LMTOOL_PREPASS_TILE_TRACE").is_some());
    static TILE_TRACE_N: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let ascale: f32 = *ASCALE;
    // 2. the runs' targets, each written by the bands of its (run, band) tasks — disjoint rows, so through raw pointers
    let threads = crate::pool::pool().threads.max(1);
    let n_bands = (threads * 2).clamp(1, H as usize);
    let rows = (H as usize + n_bands - 1) / n_bands;
    let mut targets: Vec<Target> = (0..9).map(|_| Target::new()).collect();
    let tptrs: Vec<(usize, usize, usize)> = targets.iter_mut().map(|t| (t.buf.data.as_mut_ptr() as usize, t.frags.as_mut_ptr() as usize, t.class.as_mut_ptr() as usize)).collect();
    // Target::blend on the raw target (the same f16 arithmetic)
    let blend_at = |tp: (usize, usize, usize), x: u32, y: u32, src: [f32; 4], class: u8| {
        let i = (y * W + x) as usize;
        // SAFETY: the (run, band) task alone writes the rows of its band of its run's target
        unsafe {
            let data = tp.0 as *mut f32;
            for k in 0..4 {
                let s = crate::gpufmt::quantise_f16(src[k], crate::gpufmt::Rounding::Truncate);
                let d = *data.add(i * 4 + k);
                *data.add(i * 4 + k) = crate::gpufmt::quantise_f16(d + s, crate::gpufmt::Rounding::NearestEven);
            }
            *(tp.1 as *mut u32).add(i) += 1;
            *(tp.2 as *mut u8).add(i) = class;
        }
    };
    {
        let tptrs = &tptrs;
        let blend_at = &blend_at;
        // THE ITEM TRIANGLES BINNED PER BAND, once per run (perf 8.26): every (run, band) task walked all 27 M item triangles
        // of the giant to find the few meeting its rows — 620 core-seconds of walking per bake. The draw list (pair = (mesh,
        // instance) in draw order, triangle) is cut into chunks, each chunk lists its triangles per band (the same
        // conservative row test as before, a row each side; a non-finite coordinate goes to every band), and the bands'
        // lists are the chunks' in chunk order — so a band meets its triangles in the draw order, as the walk did.
        let pairs: Vec<(u32, u32)> = item_meshes.iter().enumerate().flat_map(|(mi, (mk, _))| (lm.inst_first[*mk]..lm.inst_first[*mk] + lm.inst_count[*mk]).map(move |ii| (mi as u32, ii as u32))).collect();
        let pairs = &pairs;
        let n_pairs = pairs.len();
        let per_chunk = (n_pairs / (threads * 4).max(1)).max(1);
        let n_chunks = (n_pairs + per_chunk - 1) / per_chunk;
        for k in 0..9 {
            let chunk_bins: Vec<Vec<Vec<(u32, u32)>>> = crate::pool::pool().map(n_chunks, |ci| {
                let mut bins: Vec<Vec<(u32, u32)>> = vec![Vec::new(); n_bands];
                for pi in ci * per_chunk..((ci + 1) * per_chunk).min(n_pairs) {
                    let (mi, ii) = pairs[pi];
                    let inst = &lm.instances[ii as usize];
                    let rlm = prepass::raster_lm_for(inst.st, k);
                    for (ti, d) in mesh_tris[mi as usize].iter().enumerate() {
                        // a cut-out triangle blends nothing (AlphaToCoverage with alpha 1/9 on the 1-sample target: coverage 0
                        // for every fragment) — its raster is skipped whole (the giant: 17.4 M of a run's 27 M triangles)
                        if d.class == MatClass::CutOut { continue; }
                        let p = [prepass::viewport(prepass::lm_ndc(d.uv[0], &rlm), W, H), prepass::viewport(prepass::lm_ndc(d.uv[1], &rlm), W, H), prepass::viewport(prepass::lm_ndc(d.uv[2], &rlm), W, H)];
                        let (mut lo, mut hi) = (f32::INFINITY, f32::NEG_INFINITY);
                        let mut finite = true;
                        for q in &p { if !q[1].is_finite() { finite = false; } lo = lo.min(q[1]); hi = hi.max(q[1]); }
                        let (b_lo, b_hi) = if finite {
                            let row_lo = ((lo - 1.0).floor() as i64).max(0);
                            let row_hi = ((hi + 1.0) as i64).min(H as i64 - 1);
                            if row_lo > row_hi { continue; }
                            ((row_lo as usize / rows).min(n_bands - 1), (row_hi as usize / rows).min(n_bands - 1))
                        } else { (0, n_bands - 1) };
                        for b in b_lo..=b_hi { bins[b].push((pi as u32, ti as u32)); }
                    }
                }
                bins
            });
            let chunk_bins = &chunk_bins;
            let band_lists: Vec<Vec<(u32, u32)>> = crate::pool::pool().map(n_bands, |b| {
                let n: usize = chunk_bins.iter().map(|c| c[b].len()).sum();
                let mut out = Vec::with_capacity(n);
                for c in chunk_bins { out.extend_from_slice(&c[b]); }
                out
            });
            drop(chunk_bins);
            let band_lists = &band_lists;
        crate::pool::pool().run(n_bands, |band| {
            let (y_lo, y_hi) = ((band * rows).min(H as usize) as i64, ((band + 1) * rows).min(H as usize) as i64);
            if y_lo >= y_hi { return; }
            let tp = tptrs[k];
            // (the zone tiles below keep the walk with the row test: 8 triangles per tile)
            let meets = |p: &[[f32; 2]; 3]| -> bool {
                let (mut lo, mut hi) = (f32::INFINITY, f32::NEG_INFINITY);
                for q in p { if !q[1].is_finite() { return true; } lo = lo.min(q[1]); hi = hi.max(q[1]); }
                ((hi + 1.0) as i64) >= y_lo && ((lo - 1.0).floor() as i64) < y_hi
            };
            for &(pi, ti) in &band_lists[band] {
                let (mi, ii) = pairs[pi as usize];
                let inst = &lm.instances[ii as usize];
                let rlm = prepass::raster_lm_for(inst.st, k);
                let d = &mesh_tris[mi as usize][ti as usize];
                {
                    {
                        let p = [prepass::viewport(prepass::lm_ndc(d.uv[0], &rlm), W, H), prepass::viewport(prepass::lm_ndc(d.uv[1], &rlm), W, H), prepass::viewport(prepass::lm_ndc(d.uv[2], &rlm), W, H)];
                        let uv0 = d.uv0;
                        let (dudx, dudy) = prepass::attr_gradient(p, [uv0[0][0], uv0[1][0], uv0[2][0]]);
                        let (dvdx, dvdy) = prepass::attr_gradient(p, [uv0[0][1], uv0[1][1], uv0[2][1]]);
                        prepass::raster_tri_rows(p, W, H, y_lo, y_hi, |x, y, b| {
                            let src = match d.class {
                                MatClass::CutOut => return, // AlphaToCoverage with alpha 1/9 on the 1-sample target: coverage 0
                                MatClass::Textured => match d.tex {
                                    Some(tx) => {
                                        // the game uploads the zip's DDS bottom-up (D's rule): the GPU texture's row y is the file's row h − 1 − y, so
                                        // the file image is sampled at (u, 1 − v)
                                        let uvs = [b[0] * uv0[0][0] + b[1] * uv0[1][0] + b[2] * uv0[2][0], 1.0 - (b[0] * uv0[0][1] + b[1] * uv0[1][1] + b[2] * uv0[2][1])];
                                        let colour = inst_colour[ii as usize];
                                        match (d.hue_tex, colour) {
                                            (Some((mask, targets)), c) if c != 0 && (c as usize) < 6 => match prepass::ps_basecolor_hue(tx, mask, targets[c as usize], &sampler, uvs, [dudx, -dvdx], [dudy, -dvdy], d.at, lm_scale) { Some(s) => s, None => return },
                                            _ => match prepass::ps_basecolor(tx, &sampler, uvs, [dudx, -dvdx], [dudy, -dvdy], d.at, lm_scale) { Some(s) => s, None => return },
                                        }
                                    }
                                    None => [0.0, 0.0, 0.0, lm_scale],
                                },
                                _ => { let c = match &d.hue { Some(t) => t[inst_colour[ii as usize].min(5) as usize], None => d.konst.unwrap() }; [c[0] * lm_scale, c[1] * lm_scale, c[2] * lm_scale, lm_scale] }
                            };
                            blend_at(tp, x, y, src, d.cls);
                        });
                    }
                }
            }
            // the zone tiles: the terrain constant over the tile mesh — or, for a textured tile material (Stadium's Grass), the
            // 17023 class sampling the BaseColor texture at the quad's single uv set (= its lightmap uv; the file image at (u, 1 − v))
            for (mk, mesh) in lm.meshes.iter().enumerate() {
                if lm.inst_count[mk] < 1000 {
                    continue;
                }
                let c = frozen.tile_rgb;
                let src = [c[0] * lm_scale, c[1] * lm_scale, c[2] * lm_scale, lm_scale];
                for inst in lm.instances.iter().skip(lm.inst_first[mk]).take(lm.inst_count[mk]) {
                    let rlm = prepass::raster_lm_for(inst.st, k);
                    for tri in mesh.indices.chunks_exact(3) {
                        let p = [0, 1, 2].map(|i| prepass::viewport(prepass::lm_ndc(mesh.verts[tri[i] as usize].uv, &rlm), W, H));
                        if !meets(&p) { continue; }
                        match &frozen.tile_tex {
                            Some(tx) => {
                                let uv0 = [0, 1, 2].map(|i| mesh.verts[tri[i] as usize].uv);
                                let (dudx, dudy) = prepass::attr_gradient(p, [uv0[0][0], uv0[1][0], uv0[2][0]]);
                                let (dvdx, dvdy) = prepass::attr_gradient(p, [uv0[0][1], uv0[1][1], uv0[2][1]]);
                                // THE GRASS X2 LAYER (RE 13, 18:28Z, PS 9514 DXBC / VS 9513's uv rows): o0 = 2·(Grass_D(x/32, −z/32)·Grass_X2(uv2), 1/9)
                                // with uv2 = (−x/1024 + 0.25, z/1024 + 0.25) from the fragment's WORLD position — one local X2 value per tile (G's
                                // lookup on stpad: 0.42–0.63 / 0.45–0.64 / 0.25–0.42, mean ≈ the global 0.49 / 0.52 / 0.32), X2 stored bytes / 255.
                                // The ×2 is on o0.xyz only (RE 13, 18:45Z) and the X2 view is raw BC1_UNORM (18:50Z) — the default form; LMTOOL_GRASS_X2
                                // = off | srgb keeps the study switches. Grass_D's mips are linear-correct (e_ddsmean: every mip's sRGB-decoded mean
                                // 0.088 / 0.142 / 0.056 ± 0.5 %), so the footprint LOD (7.55 here) is not a factor either.
                                let x2 = frozen.tile_x2.as_ref();
                                let wp = [0, 1, 2].map(|i| { let v = mesh.verts[tri[i] as usize].pos; [v[0] + inst.t[0], v[2] + inst.t[2]] });
                                // THE GRASS MDIFFUSE IS ONE CONSTANT (E, 2026-09-26 20:30Z, from the banked f4468 draws.json + VS 9513's text):
                                // GbxVisualToWorld is the ZERO matrix in every one of the 9 217 grass draws, so the VS's world position is
                                // (0, 0, 0, 1) and the positional uvs collapse to the translation rows — BaseColor at (0, 0), GrassX2 at
                                // (0.25, 0.25) (GbxWorldPosToTexCoord_MapGrassX2 row 4) — the pre-pass colour of every grass tile texel is
                                // 2 · Grass_D_lin(0, 0) · Grass_X2_raw(0.25, 0.25) (stpad: (0.0798, 0.1356, 0.0339)), sampled with the texture's own
                                // WRAP sampler at zero derivatives (mip 0, the four corner texels' bilinear mix — RE 8's corner rule, now read as
                                // the LM pre-pass's rule for every world-position material, not a pwc-day accident). LMTOOL_GRASS_X2=textured
                                // keeps the per-texel study sampling.
                                if let (Some(t2), false) = (x2, *GRASS_TEXTURED) {
                                    let cd = texsample::sample(tx, 0, &sampler, [0.0, 0.0], [0.0, 0.0], [0.0, 0.0]);
                                    let c2 = texsample::sample(t2, 0, &sampler, [0.25, 0.25], [0.0, 0.0], [0.0, 0.0]);
                                    // LMTOOL_GRASS_NO2=1 (study): the constant WITHOUT the shader's ×2 — G's measured game albedo (0.0409, 0.0755, 0.0229)
                                    // is D(0,0)·X2(0.25,0.25) = (0.0399, 0.0678, 0.0170) to 2 % in R, 10 % in G; the ×2 read from the DXBC gives twice that
                                    let two = if *GRASS_NO2 { 1.0 } else { 2.0 };
                                    let k = [two * cd[0] * c2[0] * ascale * lm_scale, two * cd[1] * c2[1] * ascale * lm_scale, two * cd[2] * c2[2] * ascale * lm_scale, lm_scale];
                                    if TILE_TRACE_N.fetch_add(1, std::sync::atomic::Ordering::Relaxed) == 0 { eprintln!("tile pre-pass: the grass constant {two} · D_lin(0, 0) ({:.4}, {:.4}, {:.4}) · X2_raw(0.25, 0.25) ({:.4}, {:.4}, {:.4}) = ({:.4}, {:.4}, {:.4})", cd[0], cd[1], cd[2], c2[0], c2[1], c2[2], k[0] * 9.0 / ascale, k[1] * 9.0 / ascale, k[2] * 9.0 / ascale); }
                                    prepass::raster_tri_rows(p, W, H, y_lo, y_hi, |x, y, _| blend_at(tp, x, y, k, 6));
                                    continue;
                                }
                                prepass::raster_tri_rows(p, W, H, y_lo, y_hi, |x, y, b| {
                                    let uvs = [b[0] * uv0[0][0] + b[1] * uv0[1][0] + b[2] * uv0[2][0], 1.0 - (b[0] * uv0[0][1] + b[1] * uv0[1][1] + b[2] * uv0[2][1])];
                                    if let Some(mut s) = prepass::ps_basecolor(tx, &sampler, uvs, [dudx, -dvdx], [dudy, -dvdy], None, lm_scale) {
                                        // LMTOOL_PREPASS_TILE_TRACE=1: the first fragments' footprint LOD and sample (the grass mip question, 18:45Z)
                                        if *TILE_TRACE && TILE_TRACE_N.fetch_add(1, std::sync::atomic::Ordering::Relaxed) < 6 {
                                            let (lod, ratio, _) = texsample::lod_and_ratio([dudx * tx.w as f32, -dvdx * tx.h as f32], [dudy * tx.w as f32, -dvdy * tx.h as f32], sampler.max_aniso);
                                            eprintln!("tile trace: raster px ({x},{y}) uv ({:.4},{:.4}) ddx ({:.5},{:.5}) ddy ({:.5},{:.5}) → LOD {lod:.3} (aniso ratio {ratio:.2}, sampler mip {:?} bias {} clamp [{}, {}]) sample×9 ({:.4},{:.4},{:.4})", uvs[0], uvs[1], dudx, -dvdx, dudy, -dvdy, sampler.mip, sampler.lod_bias, sampler.min_lod, sampler.max_lod, s[0] * 9.0, s[1] * 9.0, s[2] * 9.0);
                                        }
                                        if let Some(t2) = x2 {
                                            let wx = b[0] * wp[0][0] + b[1] * wp[1][0] + b[2] * wp[2][0];
                                            let wz = b[0] * wp[0][1] + b[1] * wp[1][1] + b[2] * wp[2][1];
                                            let uv2 = [-wx / 1024.0 + 0.25, 1.0 - (wz / 1024.0 + 0.25)];
                                            let g = 1.0 / 1024.0 / 32.0; // the X2 gradient: 1/1024 per metre, a texel ≈ 1/32 m
                                            let c2 = texsample::sample(t2, 0, &sampler, uv2, [g, 0.0], [0.0, g]);
                                            // the ×2 is on o0.xyz ONLY (RE 13's DXBC re-read, 18:45Z: o0.a = 1/9 undoubled)
                                            s[0] *= 2.0 * c2[0]; s[1] *= 2.0 * c2[1]; s[2] *= 2.0 * c2[2];
                                        }
                                        s[0] *= ascale; s[1] *= ascale; s[2] *= ascale; blend_at(tp, x, y, s, 6);
                                    }
                                });
                            }
                            None => prepass::raster_tri_rows(p, W, H, y_lo, y_hi, |x, y, _| blend_at(tp, x, y, src, 1)),
                        }
                    }
                }
            }
        });
        }
    }
    // 3. the water tint per run (PS 17018 over the tile mesh with the frozen id map / tables), the nine in parallel as before
    {
        let tps: Vec<usize> = targets.iter_mut().map(|t| t as *mut Target as usize).collect();
        let tps = &tps;
        crate::pool::pool().run(9, |k| {
            // SAFETY: one task per target
            let tgt = unsafe { &mut *(tps[k] as *mut Target) };
            crate::prepass_check::tint_from_map(frozen, lm, k, tgt);
        });
    }
    {
        let tgt = &targets[8];
        let mut hist: std::collections::BTreeMap<u32, usize> = std::collections::BTreeMap::new();
        for i in 0..(W * H) as usize { let a = tgt.buf.data[i * 4 + 3]; if a > 0.0 { *hist.entry((a * 9.0).round() as u32).or_default() += 1; } }
        notes.push(format!("pre-pass run 8: alpha histogram (fragments per texel → texels) {:?}; items-only check: tiles {} instances", hist, lm.meshes.iter().enumerate().filter(|(k, _)| lm.inst_count[*k] >= 1000).map(|(k, _)| lm.inst_count[k]).sum::<usize>()));
    }
    // 4. PS 1109: the runs accumulated into 16963 in run order, per texel (parallel over texel chunks: a texel's chain is its own)
    {
        let n = (W * H) as usize;
        let per = (n / (threads * 2).max(1)).max(4096);
        let ap = acc.data.as_mut_ptr() as usize;
        let targets = &targets;
        crate::pool::pool().run((n + per - 1) / per, |ci| {
            let (a, e) = ((ci * per).min(n), ((ci + 1) * per).min(n));
            for i in a..e {
                for c in 0..4 {
                    let mut v = 0.0f32;
                    for tgt in targets.iter() {
                        let s = crate::gpufmt::quantise_f16(tgt.buf.data[i * 4 + c], crate::gpufmt::Rounding::Truncate);
                        v = crate::gpufmt::quantise_f16(v + s, crate::gpufmt::Rounding::NearestEven);
                    }
                    // SAFETY: the chunks partition the texels
                    unsafe { *(ap as *mut f32).add(i * 4 + c) = v; }
                }
            }
        });
    }
    notes.push(format!("pre-pass: {} item instances → per run {} textured / {} cut-out (ATC: nothing) / {} pad / {} wall triangles; tiles at the frozen terrain constant {:?}, wall {:?}, pad {:?}", scene.instances.len(), class_count[0], class_count[1], class_count[2], class_count[3], frozen.tile_rgb, frozen.wall_rgb, frozen.pad_rgb));
    acc
}

/// PS 17043 → 16969 (sRGB UNORM8, alpha linear), as e2e stage 2.
pub fn mdiffuse8_of(attr: &Buf) -> Buf {
    let res = crate::ilightin::resolve_ps17043(attr, false);
    let mut out = Buf::new(W, H, 4);
    let res = &res;
    out.fill_rows_par(|y, row| {
        for x in 0..W {
            for c in 0..4 {
                let v = res.get(x, y, c);
                let v = if c < 3 { crate::gpufmt::linear_to_srgb(v) } else { v };
                row[(x * 4 + c) as usize] = crate::ilightin::unorm8_rt(v, crate::gpuenc::UnormRounding::NearestEven);
            }
        }
    });
    out
}

/// The ILightInput chain (e2e stages 5–8): PS 1038 mask, PS 17043 resolve, PS 1109 × the sRGB-decoded MDiffuse, PS 1335 × 8.
pub fn ilightinput_chain(sun: &Buf, mdiffuse8: &Buf) -> (Buf, Buf) {
    ilightinput_chain_d0(sun, mdiffuse8, None)
}

/// `ilightinput_chain` with the lamps' D_0 (RE 13, 19:25Z): C0img = D_0 + NormWithA(A_0) — `d0` (RGBA, alpha 1 where the lamps cover, the
/// gutter-filled ring included) is added to the resolved sun/lamp accumulation before × MDiffuse; the mask stays the accumulation's.
pub fn ilightinput_chain_d0(sun: &Buf, mdiffuse8: &Buf, d0: Option<&Buf>) -> (Buf, Buf) {
    use crate::gpufmt::Rounding;
    let mask = crate::ilightin::quantise_unorm8(&crate::ilightin::mask_ps1038(sun, [1.0, 1.0, 0.0, 0.0], [[0.0; 4], [0.0; 4], [0.0; 4], [1.0; 4]], W, H), crate::gpuenc::UnormRounding::NearestEven);
    let mut resolved = crate::ilightin::resolve_ps17043(sun, false);
    if let Some(d) = d0 {
        // C0img := D_0 (PS 1034 copy → R11G11B10), then += the normalised accumulation (One/One)
        for y in 0..H.min(d.h) { for x in 0..W.min(d.w) { for c in 0..3 { let v = d.get(x, y, c); if v != 0.0 { resolved.set(x, y, c, resolved.get(x, y, c) + v); } } } }
    }
    let stage = crate::ilightin::quantise_r11(&resolved, Rounding::Truncate);
    let mut mdiff_lin = mdiffuse8.clone();
    let ch = mdiffuse8.channels;
    mdiff_lin.fill_rows_par(|y, row| {
        for x in 0..W {
            for c in 0..3 {
                row[(x * ch + c) as usize] = crate::gpufmt::srgb_to_linear(mdiffuse8.get(x, y, c));
            }
        }
    });
    let mut c = crate::ilightin::quantise_r11(&crate::finalprep::multiply_ps1109(&stage, &mdiff_lin, [1.0; 4]), Rounding::Truncate);
    let mut w = mask;
    for _ in 0..8 {
        let (oc, ow) = crate::ilightin::dilate_ps1335(&c, &w);
        c = crate::ilightin::quantise_r11(&oc, Rounding::Truncate);
        w = crate::ilightin::quantise_unorm8(&ow, crate::gpuenc::UnormRounding::NearestEven);
    }
    (c, w)
}

/// The whole setup chain from the map.
pub fn build(scene: &crate::geometry::Scene, lm: &LmScene, sbox: &Aabb, dir_in_world: [f32; 3], light_rgb: [f32; 3], frozen: &FrozenTables, item_bytes: &dyn Fn(&str) -> Option<Vec<u8>>, quiet: bool) -> FromMap {
    build_with_lamps(scene, lm, sbox, dir_in_world, light_rgb, frozen, item_bytes, quiet, None)
}

/// `build` with the LOCAL LIGHTS' composed light (STUDY, E 2026-09-26 16:25Z): `lamp_light` = the frame-1 compose over the 2048² atlas
/// (localdrive::compose, Σ w²·rgb per texel, alpha 1 where any lamp) ADDED to the sun accumulation before the ILightInput chain — the
/// hypothesis under test: the editor's night frame 0 carries the lamps' first bounce (objects seeing a lamp-lit floor 2–10× brighter
/// than ours, in the floor's colour; REPORT-5-E §4-E.21) and the Sunrise thin-vertical excess is the same term. Not a transcription:
/// RE 13 reads what feeds frame 0; LMTOOL_LAMP_BOUNCE=1 in the bake turns it on.
pub fn build_with_lamps(scene: &crate::geometry::Scene, lm: &LmScene, sbox: &Aabb, dir_in_world: [f32; 3], light_rgb: [f32; 3], frozen: &FrozenTables, item_bytes: &dyn Fn(&str) -> Option<Vec<u8>>, quiet: bool, lamp_light: Option<&Buf>) -> FromMap {
    let mut notes = Vec::new();
    let t0 = std::time::Instant::now();
    let cam = sun_camera(sbox, dir_in_world);
    let pw01 = cam.world_pw01_shadow(4096, 4096);
    notes.push(format!("sun camera: eye {:?} h {:?} near {} far {} (the scene box {:?}–{:?})", cam.eye, cam.h, cam.near(), cam.far(), sbox.min, sbox.max));
    let shadow = shadow_from_map(scene, lm, &cam, item_bytes, &mut notes).to_buf();
    shadowmap::print_shadow_alpha_stats("sun shadow map 4096², the map's casters");
    if !quiet { eprintln!("setup-from-map: shadow map ({:.1}s)", t0.elapsed().as_secs_f32()); }
    // LMTOOL_SUN_DIRECT_SCALE=k (STUDY, RE 13's sweep-0 forms, 19:10Z): the sun / moon direct term in sweep 0's light input scaled — with
    // LMTOOL_LAMP_BOUNCE=1.5 and 0.5 = the "1.5·L + 0.5·S" candidate (the lamps' alpha stacking with the sun's in NormWithA), with 2 and 1 =
    // "2·L + S" (D_0 + (L + S), no alpha stacking); 1 (the default) = our verified sun chain
    let sun_scale: f32 = std::env::var("LMTOOL_SUN_DIRECT_SCALE").ok().and_then(|v| v.parse().ok()).unwrap_or(1.0);
    let light_rgb = if sun_scale != 1.0 { eprintln!("setup-from-map: STUDY sun direct term × {sun_scale} in sweep 0's light input"); [light_rgb[0] * sun_scale, light_rgb[1] * sun_scale, light_rgb[2] * sun_scale] } else { light_rgb };
    // LMTOOL_STOCK_VEGET_RECV_SUN=0 (study, E5): the charted legacy trees' receiver meshes (stockveg: VegetModel::lm_mesh) drawn in the
    // direct-sun pass or not — RedIsland's 38 BushSmallC charts read 1.68/1.55/1.34 the editor WITH the sun on them
    let skip_sun: Vec<usize> = if std::env::var("LMTOOL_STOCK_VEGET_RECV_SUN").map(|v| v == "0").unwrap_or(false) {
        (0..lm.meshes.len()).filter(|&k| lm.inst_count[k] > 0 && lm.port_inst.get(lm.inst_first[k]).map(|&pi| pi != usize::MAX && scene.models[scene.instances[pi].model].veget.is_some()).unwrap_or(false)).collect()
    } else { Vec::new() };
    if !skip_sun.is_empty() { eprintln!("setup-from-map: STUDY — {} legacy-tree receiver mesh(es) left out of the direct-sun pass (LMTOOL_STOCK_VEGET_RECV_SUN=0)", skip_sun.len()); }
    let mut sun = sun_from_map_skip(lm, &pw01, dir_in_world, light_rgb, &shadow, &skip_sun);
    if !quiet { eprintln!("setup-from-map: direct sun ({:.1}s)", t0.elapsed().as_secs_f32()); }
    // LMTOOL_TILE_SUN_CENSUS=1 (diagnostic, G2 2026-09-28 — the giant's buried outer tiles): the direct-sun atlas over the ZONE-TILE
    // instances' texels (the meshes with ≥ 1000 instances), split by the tile's world cell: inside the 64×64 decoration footprint
    // (x, z < 2048) vs outside — mean rgb / w and the lit fraction (w > 0 and rgb > 0). RE 15's test: a game outer tile keeps a
    // dim neutral leak (Σ 0.03) though buried under the WarpGround; if ours is exactly 0 the residue is the shadow pass at the
    // 2-m caster gap (bias / PCF / D16), not the placement.
    if std::env::var_os("LMTOOL_TILE_SUN_CENSUS").is_some() {
        // V4 (2026-09-28, RE 16's test 2): the outside split by DISTANCE BAND to the footprint in cells (1–2, 3–8, 9–32, > 32) =
        // re15_tilefloor's bands, so the direct-sun fraction reads beside the editor's stored floor per band
        const BANDS: [&str; 5] = ["inside the 64×64 footprint", "1–2 cells outside", "3–8 cells outside", "9–32 cells outside", "> 32 cells outside"];
        let mut acc = [(0usize, 0usize, [0f64; 3]); 5];
        for (k, mesh) in lm.meshes.iter().enumerate() {
            if lm.inst_count[k] < 1000 { continue; }
            for ii in lm.inst_first[k]..lm.inst_first[k] + lm.inst_count[k] {
                let inst = &lm.instances[ii];
                let (cx, cz) = ((inst.t[0] / 32.0).floor() as i64, (inst.t[2] / 32.0).floor() as i64);
                let dist = |c: i64| -> i64 { if c < 0 { -c } else if c > 63 { c - 63 } else { 0 } };
                let dcell = dist(cx).max(dist(cz));
                let side = match dcell { 0 => 0, 1..=2 => 1, 3..=8 => 2, 9..=32 => 3, _ => 4 };
                // the instance's chart rect in the atlas: the mesh's uv extent through its ST (01 space, y down = the raster's flip)
                let v0 = &mesh.verts[0];
                let st = crate::lmaccum::chart_st(v0, inst, &lm.table);
                let (mut u0, mut u1, mut v0m, mut v1m) = (f32::MAX, f32::MIN, f32::MAX, f32::MIN);
                for v in &mesh.verts { u0 = u0.min(v.uv[0]); u1 = u1.max(v.uv[0]); v0m = v0m.min(v.uv[1]); v1m = v1m.max(v.uv[1]); }
                let ax = |u: f32| ((st[0] * u + st[2]) * W as f32);
                let ay = |v: f32| ((st[1] * v + st[3]) * H as f32);
                let (xa, xb, ya, yb) = (ax(u0), ax(u1), ay(v0m), ay(v1m));
                let (x0, x1) = (xa.min(xb).floor().max(0.0) as u32, xa.max(xb).ceil().min(W as f32) as u32);
                let (y0, y1) = (ya.min(yb).floor().max(0.0) as u32, ya.max(yb).ceil().min(H as f32) as u32);
                for y in y0..y1 { for x in x0..x1 {
                    let w = sun.get(x, y, 3);
                    if w <= 0.0 { continue; }
                    let e = &mut acc[side];
                    e.0 += 1;
                    let rgb = [sun.get(x, y, 0), sun.get(x, y, 1), sun.get(x, y, 2)];
                    if rgb[0] + rgb[1] + rgb[2] > 0.0 { e.1 += 1; }
                    for c in 0..3 { e.2[c] += (rgb[c] / w) as f64; }
                } }
            }
        }
        for (side, name) in BANDS.iter().enumerate() {
            let (n, lit, s) = acc[side];
            eprintln!("tile-sun-census: {name}: {n} covered texels, {lit} with a non-zero direct sun ({:.2} %), mean sun/w ({:.5}, {:.5}, {:.5})", 100.0 * lit as f64 / n.max(1) as f64, s[0] / n.max(1) as f64, s[1] / n.max(1) as f64, s[2] / n.max(1) as f64);
        }
    }
    // THE LAMPS IN SWEEP 0's LIGHT INPUT — the game's alpha bookkeeping (RE 13, 2026-09-26 19:25Z / 19:30Z, RenderLightDirect →
    // RenderLightIndirectBounces state 0): after RenderLightDirect the accumulation is SS-normalised in place (LmSSNormWithA: A_0 = (L, α := 1)
    // on every texel with coverage > 0.01); D_0 := gutter(A_0) (the frame-1 image); the 36 sun draws add (S_raw, f) with f = the texel's
    // raster coverage (Σ 1/9 over the jitters — 1 inside, < 1 at chart edges); then C0img = D_0 + (L + S_raw)/(1 + f), × MDiffuse, the 8
    // dilates. Interior texels: 1.5·L + 0.5·S; edge texels: (L + f·S)/(1 + f) → the lamp weight rises toward 2, the sun's falls, with the
    // coverage. Verified on stpad (E, 19:25Z, as constants): Sunrise within 1–7 % on every class but the thin posts, night record 0.843 vs
    // 0.855. `lamp_light` = FrameOut::direct (rgb = the raw jitter sums × LightRgb, alpha = the coverage); LMTOOL_LAMP_BOUNCE_FORM=sum keeps
    // the first study's plain L + S.
    let mut d0: Option<Buf> = None;
    if let Some(ll) = lamp_light {
        let plain = std::env::var("LMTOOL_LAMP_BOUNCE_FORM").as_deref() == Ok("sum");
        let (mut n_tex, mut sum) = (0usize, [0.0f32; 3]);
        // A_0 normalised: (L_raw / cov, 1) where the lamps' coverage > 0.01; alpha 1 on EVERY rasterised texel of the atlas (RE 13, 20:15Z:
        // RenderAddAlphaSSAA's coverage pass before the lamps — the sun accumulation's alpha is that raster coverage), so the gutter fill
        // stays in the pad rings and gaps
        // (the SS-normalise divides by the accumulated coverage — LmSSNormWithA; LMTOOL_LL_A0_FLAG=1 = the raw-sum study form, see
        // localdrive::frame1_from_direct)
        let a0_flag = std::env::var_os("LMTOOL_LL_A0_FLAG").is_some();
        let mut a0 = Buf::new(W, H, 4);
        for y in 0..H.min(ll.h) {
            for x in 0..W.min(ll.w) {
                let cov = ll.get(x, y, 3);
                if cov > 0.01 {
                    n_tex += 1;
                    let d = if a0_flag { 1.0 } else { cov };
                    for c in 0..3 { let v = ll.get(x, y, c) / d; sum[c as usize] += v; a0.set(x, y, c, v); }
                    a0.set(x, y, 3, 1.0);
                } else if sun.get(x, y, 3) > 0.01 {
                    a0.set(x, y, 3, 1.0);
                }
            }
        }
        if plain {
            for y in 0..H { for x in 0..W { if a0.get(x, y, 3) > 0.0 { for c in 0..3 { sun.set(x, y, c, sun.get(x, y, c) + a0.get(x, y, c)); } if sun.get(x, y, 3) <= 0.0 { sun.set(x, y, 3, 1.0); } } } }
            notes.push(format!("LAMP BOUNCE STUDY (plain L + S): the lamps' normalised light added to the sun accumulation on {n_tex} texels (Σ rgb {:.1} {:.1} {:.1})", sum[0], sum[1], sum[2]));
        } else {
            // D_0 = the 8 alpha-weighted gutter fills of A_0 (PS 1332), the same image the frame-1 slot stores
            d0 = Some(crate::localdrive::frame1_dilated_n(&a0, W, 8));
            // the sun draws add (S_raw, f) INTO A_0 = (L, 1) on every covered texel (L = 0 where no lamp reached): (L + S_raw, 1 + f) — so the
            // sun's own weight is 1/(1 + f) ≈ 0.5 on every interior texel of a lamp map, lamp-lit or not (RE 13, 20:15Z)
            for y in 0..H { for x in 0..W { if a0.get(x, y, 3) > 0.0 { for c in 0..3 { sun.set(x, y, c, sun.get(x, y, c) + a0.get(x, y, c)); } sun.set(x, y, 3, sun.get(x, y, 3) + 1.0); } } }
            notes.push(format!("LAMPS IN SWEEP 0 (RE 13's alpha bookkeeping): A_0 = (L, 1) on {n_tex} texels (Σ L {:.1} {:.1} {:.1}); C0img = D_0 = gutter(A_0) + NormWithA(A_0 + sun) — interior 1.5·L + 0.5·S", sum[0], sum[1], sum[2]));
        }
    }
    let attr = attr_from_map(scene, lm, frozen, item_bytes, &mut notes);
    if !quiet { eprintln!("setup-from-map: the nine pre-pass runs ({:.1}s)", t0.elapsed().as_secs_f32()); }
    let mdiffuse8 = mdiffuse8_of(&attr);
    let (ilightinput, coverage) = ilightinput_chain_d0(&sun, &mdiffuse8, d0.as_ref());
    if !quiet { eprintln!("setup-from-map: ILightInput chain ({:.1}s)", t0.elapsed().as_secs_f32()); }
    // LMTOOL_SETUP_CENSUS=1 (diagnostic, G2 2026-09-28 — swd6's peel colours: our sweep-0 layer colour is 2–5× the game's on stpad's
    // deck): per PORT MODEL (the LM instances' port_inst → scene instance → model name; tiles = the instances without one), the mean
    // MDiffuse8, sun/w and ILightInput over the instances' ST rects — which material carries which albedo and sweep-0 colour.
    if std::env::var_os("LMTOOL_SETUP_CENSUS").is_some() {
        let mut acc: std::collections::BTreeMap<String, (usize, usize, [f64; 3], [f64; 3], [f64; 3])> = Default::default();
        for (k, mesh) in lm.meshes.iter().enumerate() {
            let (mut u0, mut u1, mut v0, mut v1) = (f32::MAX, f32::MIN, f32::MAX, f32::MIN);
            for v in &mesh.verts { u0 = u0.min(v.uv[0]); u1 = u1.max(v.uv[0]); v0 = v0.min(v.uv[1]); v1 = v1.max(v.uv[1]); }
            for ii in lm.inst_first[k]..lm.inst_first[k] + lm.inst_count[k] {
                let inst = &lm.instances[ii];
                let name = lm.port_inst.get(ii).copied().filter(|p| *p != usize::MAX).and_then(|p| scene.instances.get(p)).map(|si| si.model_name.clone()).unwrap_or_else(|| format!("(no port instance: mesh {k}, {} verts)", mesh.verts.len()));
                let st = crate::lmaccum::chart_st(&mesh.verts[0], inst, &lm.table);
                let ax = |u: f32| (st[0] * u + st[2]) * W as f32;
                let ay = |v: f32| (st[1] * v + st[3]) * H as f32;
                let (xa, xb, ya, yb) = (ax(u0), ax(u1), ay(v0), ay(v1));
                let (x0, x1) = (xa.min(xb).floor().max(0.0) as u32, xa.max(xb).ceil().min(W as f32) as u32);
                let (y0, y1) = (ya.min(yb).floor().max(0.0) as u32, ya.max(yb).ceil().min(H as f32) as u32);
                let e = acc.entry(name).or_insert((0, 0, [0.0; 3], [0.0; 3], [0.0; 3]));
                e.0 += 1;
                for y in y0..y1 { for x in x0..x1 {
                    if coverage.get(x, y, 0) <= 0.0 { continue; }
                    e.1 += 1;
                    let w = sun.get(x, y, 3).max(1e-6);
                    for c in 0..3usize { let cc = c as u32; e.2[c] += mdiffuse8.get(x, y, cc) as f64; e.3[c] += (sun.get(x, y, cc) / w) as f64; e.4[c] += ilightinput.get(x, y, cc) as f64; }
                } }
            }
        }
        let mut v: Vec<_> = acc.into_iter().collect();
        v.sort_by(|a, b| b.1.1.cmp(&a.1.1));
        eprintln!("setup-census: per port model — instances, covered texels, mean MDiffuse8 rgb, mean sun/w rgb, mean ILightInput (sweep 0) rgb");
        for (name, (ni, nt, md, su, il)) in v.iter().take(40) {
            let n = (*nt).max(1) as f64;
            eprintln!("  {:44} {:6} {:9}  md ({:.3}, {:.3}, {:.3})  sun ({:.3}, {:.3}, {:.3})  il ({:.4}, {:.4}, {:.4})", name, ni, nt, md[0] / n, md[1] / n, md[2] / n, su[0] / n, su[1] / n, su[2] / n, il[0] / n, il[1] / n, il[2] / n);
        }
    }
    FromMap { cam, pw01, shadow, sun, attr, mdiffuse8, ilightinput, coverage, notes }
}

// ───────────────────────── the collection tables FROM THE PAK (RE child 8's derivation, 2026-09-25) ─────────────────────────

/// The pre-pass constant of a terrain / pad material at the zero world matrix (RE 8): mip 0 of the Pxz slice at uv (0, 0) with
/// the wrap sampler and zero derivatives = the mean of the dds's four corner texels, BC1-decoded, sRGB → linear (the vertical
/// flip of the GPU upload leaves the corner set unchanged). `SeaFloor_D` → (0.8511, 0.7340, 0.3259), `Land_D` →
/// (0.1158, 0.1753, 0.0428), `TrackWallPxzInWorld_D` → (0.4358, 0.4036, 0.3483) — the captured constants to the printed digit.
pub fn corner_mean(dds: &[u8]) -> Result<[f32; 3], String> {
    let mut tex = texsample::parse_dds(dds, Bc1Decode::Expand8Round)?;
    tex.decode_srgb();
    let lv = tex.levels.first().and_then(|s| s.first()).ok_or("no mip 0")?;
    let (w, h) = (lv.w, lv.h);
    let c = [lv.get(0, 0), lv.get(w - 1, 0), lv.get(0, h - 1), lv.get(w - 1, h - 1)];
    // the bilinear at uv (0, 0): the four corners at weight 1/4, summed as the filter does (two lerps)
    let mut out = [0f32; 3];
    for k in 0..3 {
        let top = c[0][k] * 0.5 + c[1][k] * 0.5;
        let bot = c[2][k] * 0.5 + c[3][k] * 0.5;
        out[k] = top * 0.5 + bot * 0.5;
    }
    Ok(out)
}

/// The fog LUT 15075 (B8G8R8A8_UNORM_SRGB 256 × 1) = column 0 of the mood's `WaterColor.tga` (32 × 256 BGRA, a bottom-up TGA)
/// read top-down: texel i = image row i from the top = file row 255 − i, pixel x = 0 (RE 8: 256 / 256 bytes identical).
pub fn fog_lut_from_tga(tga: &[u8]) -> Result<Texture, String> {
    if tga.len() < 18 {
        return Err("WaterColor.tga: too short".into());
    }
    let id_len = tga[0] as usize;
    let (w, h, bpp, desc) = (u16::from_le_bytes([tga[12], tga[13]]) as usize, u16::from_le_bytes([tga[14], tga[15]]) as usize, tga[16] as usize, tga[17]);
    if tga[2] != 2 || bpp != 32 {
        return Err(format!("WaterColor.tga: type {} bpp {} (expected uncompressed 32-bit truecolor)", tga[2], bpp));
    }
    let data = &tga[18 + id_len..];
    if data.len() < w * h * 4 {
        return Err("WaterColor.tga: truncated".into());
    }
    let top_down = desc & 0x20 != 0;
    let mut px: Vec<[u8; 4]> = Vec::with_capacity(h);
    for i in 0..h {
        let file_row = if top_down { i } else { h - 1 - i };
        let o = (file_row * w) * 4;
        // BGRA in the file → the level's (r, g, b, a)
        px.push([data[o + 2], data[o + 1], data[o], data[o + 3]]);
    }
    let mut lut_t = [0f32; 256];
    for (i, v) in lut_t.iter_mut().enumerate() {
        *v = crate::gpufmt::srgb_to_linear(i as f32 / 255.0);
    }
    let level = texsample::Level { w: h as u32, h: 1, px: texsample::Px::U8(px), lut: std::sync::Arc::new(lut_t) };
    Ok(Texture { fmt: texsample::TexFmt::Bgra8, w: h as u32, h: 1, mips: 1, slices: 1, levels: vec![vec![level]], complete: true })
}

/// Replace the frozen collection constants by the pak's (RE 8's chain): the zone tiles' Pxz texture, the Land item's, the
/// TrackWall's Pxz texture, the mood's WaterColor.tga. `read(path)` = the pak store's reader (logical game paths).
/// The collection's water descriptor (RE 8: `Collections\<Coll>.Collection.Gbx` chunk 0x03033038): the water top, the floor,
/// the fog's maximum depth. BlueBay "Sea" = (7.0, 4.0, 3.5); RedIsland "Deep" (7.7, 2.0, 6.0); GreenCoast (7.2, 0.0, 5.0);
/// WhiteShore (7.0, 2.0, 6.0); Stadium "Shallow" (7.0, 4.0, 50.0) — read from the pak once RE 8's reader lands.
#[derive(Clone, Copy, Debug)]
pub struct WaterDesc {
    pub top: f32,
    pub floor: f32,
    pub fog_max_depth: f32,
}

impl WaterDesc {
    pub fn for_collection(c: &str) -> WaterDesc {
        match c {
            "RedIsland" => WaterDesc { top: 7.7, floor: 2.0, fog_max_depth: 6.0 },
            "GreenCoast" => WaterDesc { top: 7.2, floor: 0.0, fog_max_depth: 5.0 },
            "WhiteShore" => WaterDesc { top: 7.0, floor: 2.0, fog_max_depth: 6.0 },
            "Stadium" => WaterDesc { top: 7.0, floor: 4.0, fog_max_depth: 50.0 },
            _ => WaterDesc { top: 7.0, floor: 4.0, fog_max_depth: 3.5 },
        }
    }
}

pub fn tables_from_pak(f: &mut FrozenTables, read: &mut dyn FnMut(&str) -> Option<Vec<u8>>, collection: &str, mood: &str, tile_pxz: &str, notes: &mut Vec<String>) {
    let mut got = Vec::new();
    // the water tables from the descriptor: g_WaterTop_ByPlanes = the plane heights (one plane: the top), g_WaterDepth_FogMaxDepthInv_ByIds
    // [id − 1] = (top − floor, 1 / fogMaxDepth); the id map = the water type index + 1 over the water quads — one type over the whole
    // map here (the general rule, the quads' raster, is RE 8's reader's)
    let wd = WaterDesc::for_collection(collection);
    f.top_by_plane = vec![[wd.top, 0.0, 0.0, 1.0]];
    f.depth_by_id = vec![[wd.top - wd.floor, 1.0 / wd.fog_max_depth, 0.0, 1.0]];
    // the id map (R8G8_UINT): channel 0 = the water id (type + 1), channel 1 = the plane index (one plane: 0)
    let ch = f.ids.channels as usize;
    for (i, v) in f.ids.data.iter_mut().enumerate() { *v = if i % ch == 0 { 1.0 } else { 0.0 }; }
    got.push(format!("water descriptor {wd:?} → top_by_plane [{}], depth_by_id [({}, {})], id map 1 everywhere", wd.top, wd.top - wd.floor, 1.0 / wd.fog_max_depth));
    if let Some(b) = read(&format!("{collection}\\Media\\Texture\\Image\\{tile_pxz}_D.dds")) {
        match corner_mean(&b) { Ok(c) => { got.push(format!("tiles {tile_pxz}_D → {:?} (frozen {:?})", c, f.tile_rgb)); f.tile_rgb = c; } Err(e) => notes.push(format!("pak: {tile_pxz}_D: {e}")) }
    }
    if let Some(b) = read(&format!("{collection}\\Media\\Texture\\Image\\Land_D.dds")) {
        match corner_mean(&b) { Ok(c) => { got.push(format!("Land_D → {:?} (frozen {:?})", c, f.wall_rgb)); f.wall_rgb = c; } Err(e) => notes.push(format!("pak: Land_D: {e}")) }
    }
    if let Some(b) = read("Stadium\\Media\\Texture\\Image\\TrackWallPxzInWorld_D.dds") {
        match corner_mean(&b) { Ok(c) => { got.push(format!("TrackWallPxzInWorld_D → {:?} (frozen {:?})", c, f.pad_rgb)); f.pad_rgb = c; } Err(e) => notes.push(format!("pak: TrackWallPxzInWorld_D: {e}")) }
    }
    if let Some(b) = read(&format!("{collection}\\Media\\Moods\\{mood}\\WaterColor.tga")) {
        match fog_lut_from_tga(&b) {
            Ok(t) => {
                // against the frozen LUT: the 256 texels
                let mut same = 0;
                if let (Some(a), Some(c)) = (t.levels.first().and_then(|s| s.first()), f.fog.levels.first().and_then(|s| s.first())) {
                    for i in 0..256u32.min(a.w).min(c.w) { if a.get(i, 0) == c.get(i, 0) { same += 1; } }
                }
                got.push(format!("fog LUT = WaterColor.tga column 0 top-down ({same} / 256 texels identical to the captured 15075)"));
                f.fog = t;
            }
            Err(e) => notes.push(format!("pak: WaterColor.tga: {e}")),
        }
    }
    // the water tables (RE 8: the collection descriptor, chunk 0x03033038 of Collections\<Coll>.Collection.Gbx — BlueBay "Sea" WaterTop 7.0,
    // WaterFloor 4.0, FogMaxDepth 3.5): g_WaterTop_ByPlanes = the plane heights, g_WaterDepth_FogMaxDepthInv_ByIds[id − 1] =
    // (WaterTop − WaterFloor, 1 / FogMaxDepth); the id map = the water type index + 1 over the water quads (one type here)
    let mut hist: std::collections::BTreeMap<u32, usize> = std::collections::BTreeMap::new();
    for i in 0..(f.ids.w * f.ids.h) as usize { *hist.entry(f.ids.data[i * f.ids.channels as usize] as u32).or_default() += 1; }
    notes.push(format!("water tables (captured): top_by_plane {:?}, depth_by_id {:?}, id map {}×{} histogram {:?}; the collection descriptor (RE 8): WaterTop 7.0 WaterFloor 4.0 FogMaxDepth 3.5 → depth 3.0, 1/fogMaxDepth {}", f.top_by_plane, f.depth_by_id, f.ids.w, f.ids.h, hist, 1.0f32 / 3.5));
    notes.push(format!("tables FROM THE PAK (RE 8's chain): {}; still frozen: the transmittance LUT 15078 (the kind-51 ImageGen)", got.join("; ")));
}

/// The collection tables through RE child 8's `paktables` (the material chain of the pack, the collection's water descriptor,
/// the fog LUT image and the transmittance generator): nothing of the pre-pass's material inputs stays frozen. `links` = the
/// game-material links of the map's LM instances: the zone tiles' (the terrain material of the zone) and the items' constant
/// materials (Land, TrackWallInWorld).
pub fn tables_from_paktables(f: &mut FrozenTables, store: &mut mapgeom::store::DataStore, collection: &str, tile_link: &str, scene: &crate::geometry::Scene, notes: &mut Vec<String>) -> Result<(), String> {
    tables_from_paktables_with_records(f, store, collection, tile_link, scene, &[], notes)
}

/// `tables_from_paktables` with the map's RECORDS (the layout's `records`, chart k ↔ record k): the water-id map and the
/// plane table come from the records' water quads (RE 11, `waterid`: the SetWaterId pass rebuilt — Stadium's WaterBase
/// blocks' Water geom at world 23 over their 32 × 32 footprints) when any record carries one; a record list without a
/// water quad (pwc-day: the sea is the ZONE TILES, whose records carry no prefab mesh) keeps the whole-map id map at the
/// collection's WaterTop (the captured 17004: every texel id 1, plane 0 — a sea zone).
/// A game material's BaseColor slot texture from the packs (RE 12's 17023 class): the material chain's BaseColor (else Diffuse)
/// bitmap slot is a CPlugBitmap file (`…\Texture\X.Texture.gbx`) whose image node is the external `.dds` reference
/// (`…\Texture\Image\X.dds`; the rename is the fallback when the bitmap graph does not resolve), decoded and linearised.
/// `Ok(None)` = no such slot; `Err` = the slot exists but its file does not load.
/// One named bitmap slot of a material's chain as a texture: `srgb` decodes the stored bytes through the sRGB view (BaseColor-class
/// textures), false keeps them as stored / 255 (the GrassX2 layer — RE 13's view-format check, 18:28Z). None when the slot is absent or empty.
pub fn slot_texture(store: &mut mapgeom::store::DataStore, link: &str, slot_name: &str, srgb: bool) -> Result<Option<(String, Texture)>, String> {
    let mat = if link.to_ascii_uppercase().ends_with(".MATERIAL.GBX") { link.to_string() } else { format!("{link}.Material.Gbx") };
    let chain = mapgeom::envblock::material_chain(store, &mat);
    let Some(slot) = chain.bitmaps.iter().find(|(n, p)| n.eq_ignore_ascii_case(slot_name) && !p.is_empty()).map(|(_, p)| p.clone()) else { return Ok(None) };
    let dds = {
        let mut out: Option<String> = None;
        if let Ok(m) = store.load_model(&slot) { if let Ok(g) = m.graph() { if let Some(mapgeom::node::Node::Bitmap(b)) = &g.root {
            if b.image >= 0 { if let Some(mapgeom::node::Slot::External(dp)) = g.slots.get(b.image as usize) { out = Some(dp.clone()); } }
        } } }
        out.unwrap_or_else(|| {
            if slot.to_ascii_uppercase().ends_with(".TEXTURE.GBX") {
                let stem = &slot[..slot.len() - ".Texture.gbx".len()];
                match stem.rsplit_once('\\') { Some((dir, name)) => format!("{dir}\\Image\\{name}.dds"), None => format!("{stem}.dds") }
            } else { slot.clone() }
        })
    };
    let bytes = store.read(&dds).map_err(|e| format!("{dds}: {e}"))?;
    let mut tx = texsample::parse_dds(&bytes, Bc1Decode::Expand8Round).map_err(|e| format!("{dds}: {e}"))?;
    if srgb { tx.decode_srgb(); }
    Ok(Some((dds, tx)))
}

pub fn basecolor_texture(store: &mut mapgeom::store::DataStore, link: &str) -> Result<Option<(String, Texture, bool)>, String> {
    let mat = if link.to_ascii_uppercase().ends_with(".MATERIAL.GBX") { link.to_string() } else { format!("{link}.Material.Gbx") };
    let chain = mapgeom::envblock::material_chain(store, &mat);
    let pick = |name: &str| chain.bitmaps.iter().find(|(n, p)| n.eq_ignore_ascii_case(name) && !p.is_empty()).map(|(_, p)| p.clone());
    // BaseColor (CubeOut / TDSN), else Diffuse (PDiff), else BaseColorOp — DispIn: OPAQUE (RE 13, 2026-09-26 17:55Z, f4468 PS 9529 DXBC:
    // TMapBaseColorOp(uv), no alpha test, no discard, alpha := 1 — RE 12's "alpha varies → alpha-tested" inference was wrong; the cards'
    // 0x8000 discard at 128/255 (PS 9530, TDOSN) is the flag path in attr_from_map, not this slot). LMTOOL_DISPIN_ALPHA_TEST=1 = the old inference.
    let dispin_at = std::env::var_os("LMTOOL_DISPIN_ALPHA_TEST").is_some();
    let (slot, alpha_tested) = match pick("BaseColor").or_else(|| pick("Diffuse")) { Some(p) => (Some(p), false), None => (pick("BaseColorOp"), dispin_at) };
    let Some(slot) = slot else { return Ok(None) };
    let dds = {
        let mut out: Option<String> = None;
        if let Ok(m) = store.load_model(&slot) { if let Ok(g) = m.graph() { if let Some(mapgeom::node::Node::Bitmap(b)) = &g.root {
            if b.image >= 0 { if let Some(mapgeom::node::Slot::External(dp)) = g.slots.get(b.image as usize) { out = Some(dp.clone()); } }
        } } }
        out.unwrap_or_else(|| {
            if slot.to_ascii_uppercase().ends_with(".TEXTURE.GBX") {
                let stem = &slot[..slot.len() - ".Texture.gbx".len()];
                match stem.rsplit_once('\\') { Some((dir, name)) => format!("{dir}\\Image\\{name}.dds"), None => format!("{stem}.dds") }
            } else { slot.clone() }
        })
    };
    let bytes = store.read(&dds).map_err(|e| format!("{dds}: {e}"))?;
    let mut tx = texsample::parse_dds(&bytes, Bc1Decode::Expand8Round).map_err(|e| format!("{dds}: {e}"))?;
    tx.decode_srgb();
    Ok(Some((dds, tx, alpha_tested)))
}

/// THE TEXTURED HUE-MASK CLASS (PS 9539 — RE 13 17:55Z: m = HueMask(uv); k = max(0, m.g − ½(m.r + m.b)); c = sat((m.g − k)·mean(T) + k·T);
/// o.rgb = lerp(BaseColor.rgb, c, BaseColor.a); T = the material's ColorTable entry of the placement colour, RE 15 07:20Z): the material's
/// HueMask texture (sRGB view on rgb, alpha linear) and its six targets (index = MapElemColor; 0 unused). None = not a HueMask material.
pub fn hue_texture(store: &mut mapgeom::store::DataStore, link: &str) -> Result<Option<(String, Texture, [[f32; 3]; 6])>, String> {
    let mat = if link.to_ascii_uppercase().ends_with(".MATERIAL.GBX") { link.to_string() } else { format!("{link}.Material.Gbx") };
    let chain = mapgeom::envblock::material_chain(store, &mat);
    let pick = |name: &str| chain.bitmaps.iter().find(|(n, p)| n.eq_ignore_ascii_case(name) && !p.is_empty()).map(|(_, p)| p.clone());
    let Some(slot) = pick("BaseColorHueMask").or_else(|| pick("DiffuseHueMask")).or_else(|| pick("HueMask")) else { return Ok(None) };
    let m = store.load_model(&mat)?;
    let Some(table) = m.externals.iter().map(|(_, p)| p.clone()).find(|p| p.to_ascii_lowercase().ends_with(".colortable.gbx.json")) else { return Ok(None) };
    let mut targets = [[0f32; 3]; 6];
    for c in 1u8..=5 { targets[c as usize] = crate::paktables::colour_table_target(store, &table, c, "Classic")?; }
    let dds = {
        let mut out: Option<String> = None;
        if let Ok(tm) = store.load_model(&slot) { if let Ok(g) = tm.graph() { if let Some(mapgeom::node::Node::Bitmap(b)) = &g.root {
            if b.image >= 0 { if let Some(mapgeom::node::Slot::External(dp)) = g.slots.get(b.image as usize) { out = Some(dp.clone()); } }
        } } }
        out.unwrap_or_else(|| {
            if slot.to_ascii_uppercase().ends_with(".TEXTURE.GBX") { let stem = &slot[..slot.len() - ".Texture.gbx".len()]; match stem.rsplit_once('\\') { Some((dir, name)) => format!("{dir}\\Image\\{name}.dds"), None => format!("{stem}.dds") } } else { slot.clone() }
        })
    };
    let bytes = store.read(&dds).map_err(|e| format!("{dds}: {e}"))?;
    let mut tx = texsample::parse_dds(&bytes, Bc1Decode::Expand8Round).map_err(|e| format!("{dds}: {e}"))?;
    tx.decode_srgb();
    Ok(Some((dds, tx, targets)))
}

pub fn tables_from_paktables_with_records(f: &mut FrozenTables, store: &mut mapgeom::store::DataStore, collection: &str, tile_link: &str, scene: &crate::geometry::Scene, records: &[crate::records::Rec], notes: &mut Vec<String>) -> Result<(), String> {
    let mut got = Vec::new();
    // the tiles' constant: a failure here (Stadium has no SeaFloor material; its Grass zone is a PDiff shader) must not take the
    // items' constants and the WATER tables down with it (E, 06:25Z — stpad ran with the whole-map id map because of that early return)
    match crate::paktables::material_constant(store, tile_link) {
        Ok(tile) => {
            got.push(format!("tiles {tile_link} → {:?} ({:?}, ids {:?}, {} at uv {:?}; frozen {:?})", tile.rgb, tile.family, tile.ids, tile.image, tile.uv, f.tile_rgb));
            f.tile_rgb = tile.rgb;
            f.tile_slices = (tile.ids[0].max(0) as u32, tile.ids[1].max(0) as u32);
        }
        Err(e) => match basecolor_texture(store, tile_link) {
            // a textured tile material (Stadium's Grass, PDiff): the 17023 class over the tile quads at their single uv set
            Ok(Some((path, tx, _))) => {
                got.push(format!("tiles {tile_link} → BaseColor {path} ({}×{}, {} mips; the textured class over the tile quads)", tx.w, tx.h, tx.mips));
                f.tile_tex = Some(tx);
                // THE GRASS X2 LAYER (RE 13, 18:28Z: PS 9514 = 2·Grass_D(x/32, −z/32)·Grass_X2(−x/1024 + 0.25, z/1024 + 0.25), X2 NOT sRGB-decoded)
                // THE X2 VIEW IS RAW (RE 13, 18:50Z: the D3D11 view of 5426 is fully typed BC1_UNORM — no sRGB decode, and the binary has no ÷2):
                // the albedo = 2·D_lin(uv1)·X2_raw(uv2) exactly as read — the DEFAULT. LMTOOL_GRASS_X2=off (D alone, the pre-patch behaviour)
                // | srgb (the refuted sRGB-view form) stay as study switches; the measured ~½ against the game is OURS to find elsewhere
                // (G: a tile double-count between the fitted-tile and world peels, or the tile chart resolution).
                let x2_mode = std::env::var("LMTOOL_GRASS_X2").unwrap_or_else(|_| "stored".into());
                let x2_mode = if x2_mode == "textured" { "stored".to_string() } else { x2_mode };
                match if x2_mode == "off" { Ok(None) } else { slot_texture(store, tile_link, "GrassX2", x2_mode == "srgb") } {
                    Ok(Some((p2, t2))) => { got.push(format!("tiles {tile_link} → GrassX2 {p2} ({}×{}, {} mips; {} , positional over 1024 m; ×2 on rgb)", t2.w, t2.h, t2.mips, if x2_mode == "srgb" { "sRGB-decoded" } else { "stored bytes / 255" })); f.tile_x2 = Some(t2); }
                    Ok(None) => {}
                    Err(e2) => notes.push(format!("paktables: tiles {tile_link}: GrassX2: {e2}")),
                }
            }
            Ok(None) => notes.push(format!("paktables: tiles {tile_link}: {e}; no BaseColor slot either (the frozen tile constant {:?} stays)", f.tile_rgb)),
            Err(e2) => notes.push(format!("paktables: tiles {tile_link}: {e}; BaseColor: {e2} (the frozen tile constant {:?} stays)", f.tile_rgb)),
        },
    }
    // the items' constant materials: every game-material link of the scene's models that the pack resolves — per (link, TargetColor
    // instance override) pair (`geometry::link_key`): the PyPxz_Hue class recolours toward the item's own constant
    let mut missing: Vec<String> = Vec::new();
    let mut links: Vec<String> = Vec::new();
    let mut pairs: Vec<(String, Option<[f32; 3]>)> = Vec::new();
    for m in &scene.models {
        for (k, l) in m.mat_links.iter().enumerate() {
            if !links.contains(l) {
                links.push(l.clone());
            }
            let p = m.mat_params.get(k).copied().flatten();
            if p.is_some() && !pairs.iter().any(|(pl, pp)| pl == l && *pp == p) {
                pairs.push((l.clone(), p));
            }
        }
    }
    // LMTOOL_WARP_ITEM_FOG=x,y,z (STUDY, E5 2026-09-28 23:25Z — default off; the coordinator's A/B while RE 16 reads the variant table): an
    // ITEM material of the Warp class (its link contains "warp": g23's hills are 26 % `Material_BlockCustom\WarpTechnic`) gets its
    // pre-pass constant FOGGED like the terrain's — c' = lerp(Fog_LinearRGB, c, f) with f = VS 16748's fog factor at the given world
    // point (the hill's centre), from the scene's Warp shading state. An approximation of "the Warp program draws the item geoms",
    // not its transcription; a positive reading means transcribing the program for item geoms as read.
    let warp_item_fog: Option<(f32, [f32; 3])> = std::env::var("LMTOOL_WARP_ITEM_FOG").ok().and_then(|s| {
        let v: Vec<f32> = s.split(',').filter_map(|x| x.trim().parse().ok()).collect();
        if v.len() != 3 { return None; }
        let sh = scene.warp.as_ref()?;
        let vs = crate::warpterrain::vs_16748(&sh.consts, [v[0], v[1], v[2]], [0.0, 1.0, 0.0]);
        let fac = vs.o4[2].clamp(0.0, 1.0);
        notes.push(format!("STUDY LMTOOL_WARP_ITEM_FOG at ({}, {}, {}): the Warp VS fog factor f = {fac:.4} (eye {:?}), fog rgb {:?} — Warp-class item constants c' = lerp(fog, c, f)", v[0], v[1], v[2], sh.consts.eye, sh.consts.fog_rgb));
        Some((fac, sh.consts.fog_rgb))
    });
    let fog_item = |l: &str, c: [f32; 3]| -> [f32; 3] {
        match warp_item_fog {
            Some((fac, fog)) if l.to_ascii_lowercase().contains("warp") => [fog[0] + fac * (c[0] - fog[0]), fog[1] + fac * (c[1] - fog[1]), fog[2] + fac * (c[2] - fog[2])],
            _ => c,
        }
    };
    for (l, p) in &pairs {
        match crate::paktables::material_constant_with(store, l, *p) {
            Ok(mc) => { let rgb = fog_item(l, mc.rgb); got.push(format!("{l} TargetColor {:?} → {:?} ({})", p.unwrap_or([0.0; 3]), rgb, mc.notes.last().cloned().unwrap_or_default())); f.link_rgb.insert(crate::geometry::link_key(l, *p), rgb); }
            Err(e) => notes.push(format!("paktables: {l} with TargetColor {:?}: {e}", p)),
        }
    }
    for l in &links {
        match crate::paktables::material_constant(store, l) {
            Ok(mc) => {
                let mc = crate::paktables::MaterialConstant { rgb: fog_item(l, mc.rgb), ..mc };
                f.link_rgb.insert(l.to_ascii_lowercase(), mc.rgb);
                // a HueMask material: its recoloured constants for the five placement colours (PS 9544 / 9539 — RE 13's arithmetic,
                // RE 15's colour table; the tap and the sRGB view as paktables::hue_recolour reads them)
                for colour in 1u8..=5 {
                    match crate::paktables::hue_recolour(store, l, colour, mc.rgb) {
                        Ok(h) => { if colour == 4 { got.push(format!("{l} colour 4 → {:?} (HueMask {} tap {:?}, table {}, target {:?})", h.rgb, h.mask_image, h.mask, h.table, h.target)); } f.hue_rgb.insert((l.to_ascii_lowercase(), colour), h.rgb); }
                        Err(_) => break,
                    }
                }
                match mc.family {
                    crate::paktables::Family::PyPxzIds => { got.push(format!("{l} → {:?} (terrain ids {:?}; frozen Land {:?})", mc.rgb, mc.ids, f.wall_rgb)); f.wall_rgb = mc.rgb; }
                    crate::paktables::Family::PyPxzProjected => { got.push(format!("{l} → {:?} (projected, {}; frozen TrackWall {:?})", mc.rgb, mc.image, f.pad_rgb)); f.pad_rgb = mc.rgb; }
                    crate::paktables::Family::PyPxzHue => { got.push(format!("{l} → {:?} (PyPxz_Hue: {}; {})", mc.rgb, mc.image, mc.notes.last().cloned().unwrap_or_default())); }
                }
            }
            Err(e) => {
                // THE 17023 CLASS (RE 12): an opaque textured non-PyPxz material (CubeOut / TDSN / TDSNI — np-tk3's Modifier\StadiumOnTerrain
                // TrackWallClipsInWorld / StructureInWorld, tiny-16's Technics, ScreenBack …) takes its BaseColor slot texture from the pack
                // at the mesh TEXCOORD0 (VS 17021 o1 = v1, PS 17023: TMapBaseColor × 1/9, alpha forced 1) — never a constant, never black
                match basecolor_texture(store, l) {
                    Ok(Some((path, tx, at))) => { got.push(format!("{l} → {} {path} ({}×{}, {} mips; the {} class)", if at { "BaseColorOp" } else { "BaseColor" }, tx.w, tx.h, tx.mips, if at { "17022 alpha-tested (LMTOOL_DISPIN_ALPHA_TEST: the refuted inference)" } else { "17023 / 9529 textured, opaque" })); f.link_tex.insert(l.to_ascii_lowercase(), (tx, at));
                        // a textured HueMask material (RoadTech, Technics …): its mask + the six targets for the per-texel recolour (PS 9539)
                        match hue_texture(store, l) { Ok(Some((mp, mtx, targets))) => { got.push(format!("{l} HueMask {mp} ({}×{}), targets {:?}", mtx.w, mtx.h, targets[4])); f.link_hue.insert(l.to_ascii_lowercase(), (mtx, targets)); } Ok(None) => {} Err(e) => notes.push(format!("{l}: HueMask: {e}")) }
                    }
                    Ok(None) => {
                        // A MATERIAL (OR ITS BASECOLOR TEXTURE) IN NONE OF THE PACKS is not a classification — it is a missing --pak (E, 2026-09-27
                        // 15:30Z, after V2's np-tk3 "regression": a command line without Stadium.pak put the StadiumOnTerrain pillar's 1 016
                        // triangles on the PAD constant, black, and the bake passed as a lighting result). Counted and named in the WARNING
                        // below; `--strict` makes it fatal.
                        if e.contains("not in any pack") { missing.push(l.clone()); }
                        notes.push(format!("pak: {l}: {e}; no BaseColor slot in its chain — the constant path keeps it"));
                    }
                    Err(e2) => { if e2.contains("not in any pack") || e.contains("not in any pack") { missing.push(l.clone()); } notes.push(format!("pak: {l}: BaseColor: {e2}")); }
                }
            }
        }
    }
    if !missing.is_empty() {
        // the triangles and instances behind the missing links: `Tri.mat` indexes `mat_links`
        let mut tris = 0usize;
        let mut insts = 0usize;
        let mut per: Vec<String> = Vec::new();
        for l in &missing {
            let (mut lt, mut li) = (0usize, 0usize);
            for (mi, m) in scene.models.iter().enumerate() {
                let Some(k) = m.mat_links.iter().position(|x| x == l) else { continue };
                let n = m.tris.iter().filter(|t| t.mat as usize == k).count();
                let ni = scene.instances.iter().filter(|i| i.model == mi).count();
                lt += n * ni.max(1);
                li += ni;
            }
            tris += lt;
            insts += li;
            per.push(format!("{l} ({lt} triangles on {li} instances)"));
        }
        notes.push(format!("WARNING: {} material(s) of the scene's models have their material or BaseColor texture in NONE of the --pak files — {tris} triangles on {insts} item instances take the PAD constant (black) instead of their BaseColor: {}. Add the pack that holds them (Modifier\\StadiumOnTerrain / Stadium materials → Stadium.pak, stock items → Maniaplanet.pak; the bake adds Stadium.pak + Maniaplanet.pak beside the collection's pak by default — --no-pak-defaults opts out); --strict makes this fatal.", missing.len(), per.join("; ")));
        WARNINGS.lock().unwrap().push(format!("{} material(s) with their material or BaseColor texture in none of the --pak files ({tris} triangles on {insts} instances black): {}", missing.len(), missing.join(", ")));
    }
    got.push(water_tables_from_records(f, store, collection, records, notes)?);
    notes.push(format!("tables FROM THE PACK (RE 8's paktables): {}; nothing of the material inputs is frozen", got.join("; ")));
    Ok(())
}

/// THE WATER PASS'S INPUTS, independent of the material lookups (RE 11): `g_WaterDepth_FogMaxDepthInv_ByIds` and the two LUTs
/// from the collection's pack (RE 8's `paktables::water_tables`: the fog image and the kind-0x33 transmittance generator), the
/// id map and `g_WaterTop_ByPlanes` from the records' water quads (`waterid`: the SetWaterId draw — Stadium's WaterBase blocks'
/// Water geom at world 23 / 119 / 231 on stpad) when any record carries one; a record list without one (pwc-day's sea = the zone
/// tiles, whose records carry no prefab mesh) keeps the whole-map id map at the collection's WaterTop (the captured 17004: every
/// texel id 1, plane 0). Called by `tables_from_paktables_with_records` and by the bake's interim fallback alike, so a Stadium
/// bake whose tile material is not a PyPxz family still gets the water term.
pub fn water_tables_from_records(f: &mut FrozenTables, store: &mut mapgeom::store::DataStore, collection: &str, records: &[crate::records::Rec], notes: &mut Vec<String>) -> Result<String, String> {
    let mut got = Vec::new();
    let w = crate::paktables::water_tables(store, collection)?;
    f.depth_by_id = vec![[w.depth_inv[0], w.depth_inv[1], 0.0, 1.0]];
    // THE ID GRID AND THE PLANE TABLE: the records' water quads — the SetWaterId draws — over the scene box's tile grid (RE 11, stpad
    // f4468: `waterid::water_grid`; pwc-day's 2048 × 2048 box is the one-tile case at 1 texel/m), else the sea-zone map
    let (quads, qnotes) = crate::waterid::water_quads_of_records(store, records)?;
    for n in &qnotes {
        notes.push(format!("water quads: {n}"));
    }
    if !quads.is_empty() {
        let (c, h) = crate::waterid::records_box(records).ok_or("water grid: no records")?;
        let wt = crate::waterid::water_id_tiles(&quads, c, h);
        f.top_by_plane = wt.plane_tops.iter().map(|t| [*t, 0.0, 0.0, 1.0]).collect();
        got.push(format!("water-id grid from {} record water quads: {} tiles of {}², {} texels under water, g_WaterTop_ByPlanes {:?} (WORLD heights; the collection's local WaterTop {} is not a plane)", wt.quads, wt.tiles.len(), crate::waterid::ID_MAP_SIZE, wt.texels, wt.plane_tops, w.top));
        for n in &wt.notes {
            notes.push(format!("water grid: {n}"));
        }
        f.water_tiles = wt.tiles;
    } else {
        // THE SEA-ZONE PLANE IS A WORLD HEIGHT (E3 2026-09-28): the descriptor's `top` is the surface in the water ZONE PREFAB'S frame
        // (7 on every collection but GreenCoast's 7.2 / RedIsland's 7.7); the game's g_WaterTop_ByPlanes are world heights (stpad's
        // captured 23 / 119 / 231 = block origin + 7). The port wrote the local value as the plane — right on BlueBay alone, whose Sea
        // row 5 has origin 5·8 − 40 = 0. On WhiteShore (Water row 14, origin −8) the plane sat at 7 instead of −1: the seabed at −6 fell
        // under the tint's floor (7 − 5 − 0.1) and was never fogged — the sea floor bounced DRY SAND (0.133, 0.095, 0.064) at every
        // receiver and probe (g23/tiny03's uniform warm tilt R +9 % B −8 %, the probes' R +21 % at 10 m); on RedIsland (Water row 14,
        // origin −8, top 7.7 → −0.3) the Dirt land at 2 was fogged as if 5.7 m deep — V4's blue tilt, the mirror. LMTOOL_WATER_TOP=local
        // keeps the old plane (study); LMTOOL_WATER_TOP=<y> forces a world height.
        let prof = crate::layout::CollectionProfile::of(collection);
        let world_top = match std::env::var("LMTOOL_WATER_TOP").ok().as_deref() {
            Some("local") => w.top,
            Some(v) => v.parse::<f32>().unwrap_or(prof.water_row as f32 * 8.0 + prof.yoff + w.top),
            None => prof.water_row as f32 * 8.0 + prof.yoff + w.top,
        };
        f.top_by_plane = vec![[world_top, 0.0, 0.0, 1.0]];
        let ch = f.ids.channels as usize;
        for (i, v) in f.ids.data.iter_mut().enumerate() { *v = if i % ch == 0 { 1.0 } else { 0.0 }; }
        got.push(format!("no record carries a water quad: the whole map is under the collection's water plane (id 1, plane 0, WORLD top {world_top} = the water zone's row {} origin {} + the descriptor's local top {}) — a sea zone", prof.water_row, prof.water_row as f32 * 8.0 + prof.yoff, w.top));
    }
    // the two LUTs against the frozen ones, texel for texel
    let cmp = |ours: &Texture, theirs: &Texture| -> (usize, usize) {
        let (Some(a), Some(c)) = (ours.levels.first().and_then(|s| s.first()), theirs.levels.first().and_then(|s| s.first())) else { return (0, 0) };
        let n = a.w.min(c.w);
        ((0..n).filter(|&i| a.get(i, 0) == c.get(i, 0)).count(), n as usize)
    };
    let fog = crate::paktables::lut_texture(&w.fog, true);
    let tr = crate::paktables::lut_texture(&w.transmittance, true);
    let (fs, fn_) = cmp(&fog, &f.fog);
    let (ts, tn) = cmp(&tr, &f.transmittance);
    got.push(format!("water {:?}: depth_by_id [({}, {})], fog LUT {} texels ({fs}/{fn_} identical to the frozen one), transmittance LUT (the kind-0x33 generator) {} texels ({ts}/{tn} identical)", w.desc, w.depth_inv[0], w.depth_inv[1], w.fog.len(), w.transmittance.len()));
    f.fog = fog;
    f.transmittance = tr;
    Ok(got.join("; "))
}
