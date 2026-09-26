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
    RasterState { viewport: [1.0, 1.0, 4094.0, 4094.0, 0.0, 1.0], depth_bias: -1, slope_scaled_depth_bias: std::env::var("LMTOOL_SUNMAP_SLOPE_BIAS").ok().and_then(|v| v.parse().ok()).unwrap_or(-1.0), depth_bias_clamp: 0.0, cull_back: true, front_ccw: true, depth_clip: true, plane: PlaneEval::F64Snapped, coef_bits: 36, vertex_z_bits: 0 }
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
        for (i, t) in scene.decor.iter().enumerate() {
            if t.water || !t.sun_caster { skipped_water += 1; continue; }
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
                let rows = rows_of_xform(&inst.xf);
                let mut opaque = CasterMesh { pos: Vec::new(), uv0: Vec::new(), indices: Vec::new() };
                let mut cut: std::collections::BTreeMap<u16, CasterMesh> = std::collections::BTreeMap::new();
                for t in &model.tris {
                    let m = if t.alpha == u16::MAX { &mut opaque } else { cut.entry(t.alpha).or_insert_with(|| CasterMesh { pos: Vec::new(), uv0: Vec::new(), indices: Vec::new() }) };
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
                    let tex = alpha_cache.get(&name).cloned().flatten();
                    let Some(texture) = tex else { notes.push(format!("shadow: cut-out texture {name} not loaded — the caster is skipped")); continue };
                    let texture = shadowmap::AlphaTexture { w: texture.w, h: texture.h, mips: texture.mips.clone() };
                    let d = CasterDraw { eid: *n_draws as u64, mesh, instance_start: 0xffff_ffff, instance_count: 1, visual_to_world: Some(rows), tables: InstanceTables { dyna_u32: Vec::new(), static_meshs: Vec::new() }, alpha: Some(shadowmap::AlphaTest { threshold: SHADOW_ALPHA_THRESHOLD, texture, max_anisotropy: 16.0 }), vsout: None };
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
    let mut draws = Vec::new();
    for j in 0..9usize {
        let cb = LmRasterCb::for_offset(j, W, H);
        for (k, _) in lm.meshes.iter().enumerate() {
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
    let item_meshes: Vec<(usize, usize)> = lm.meshes.iter().enumerate().filter(|(k, _)| lm.inst_count[*k] < 1000).map(|(k, mesh)| {
        let best = lookups.iter().enumerate().max_by_key(|(_, (_, map))| mesh.verts.iter().filter(|v| map.contains_key(&key(v.pos))).count()).map(|(li, _)| li).unwrap_or(0);
        (k, best)
    }).collect();
    for (mk, li) in &item_meshes {
        let (model_idx, map) = &lookups[*li];
        let mesh = &lm.meshes[*mk];
        let hits = mesh.verts.iter().filter(|v| map.contains_key(&key(v.pos))).count();
        notes.push(format!("item mesh {mk} ↔ model {} ({}): {} of {} LM verts matched by position ({} port triangles)", model_idx, scene.model_names[*model_idx], hits, mesh.verts.len(), scene.models[*model_idx].tris.len()));
        notes.push(format!("  model {} materials: links {:?}, diffuse textures {:?}, cut-out textures {:?}; LM uv range {:?}", model_idx, scene.models[*model_idx].mat_links, scene.models[*model_idx].diff_tex, scene.models[*model_idx].alpha_tex, mesh.verts.iter().fold(([f32::MAX; 2], [f32::MIN; 2]), |(lo, hi), v| ([lo[0].min(v.uv[0]), lo[1].min(v.uv[1])], [hi[0].max(v.uv[0]), hi[1].max(v.uv[1])]))));
    }
    // THE NINE RUNS IN PARALLEL: each run renders its own target (independent), the accumulation into 16963 follows in run
    // order (the f16 add chain is sequential, so the result is bit-identical to the serial loop)
    let run_k = |k: usize| -> (Target, Vec<String>, [usize; 4]) {
        let mut notes: Vec<String> = Vec::new();
        let mut class_count = [0usize; 4];
        let mut tgt = Target::new();
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
            for inst in lm.instances.iter().skip(lm.inst_first[*mk]).take(lm.inst_count[*mk]) {
                let rlm = prepass::raster_lm_for(inst.st, k);
                for (uv, uv0, class, diff) in &tris {
                    let (class, uv0) = (*class, *uv0);
                    let p = [prepass::viewport(prepass::lm_ndc(uv[0], &rlm), W, H), prepass::viewport(prepass::lm_ndc(uv[1], &rlm), W, H), prepass::viewport(prepass::lm_ndc(uv[2], &rlm), W, H)];
                    let (dudx, dudy) = prepass::attr_gradient(p, [uv0[0][0], uv0[1][0], uv0[2][0]]);
                    let (dvdx, dvdy) = prepass::attr_gradient(p, [uv0[0][1], uv0[1][1], uv0[2][1]]);
                    let tex = match class {
                        MatClass::Textured => {
                            let tn = diff_name(model, *diff);
                            tex_cache.get(&tn).and_then(|t| t.as_ref())
                        }
                        _ => None,
                    };
                    // the constant per LINK when the pack gave one (diff carries the material index for a linked material), else the class global
                    let link_const = if *diff & 0xC000 == 0x4000 { model.mat_links.get((*diff & 0x3fff) as usize).and_then(|l| frozen.link_rgb.get(&l.to_ascii_lowercase())).copied() } else { None };
                    let konst = match class {
                        MatClass::Pad => Some(link_const.unwrap_or(frozen.pad_rgb)),
                        MatClass::Wall => Some(link_const.unwrap_or(frozen.wall_rgb)),
                        _ => None,
                    };
                    let cls = match class { MatClass::Textured => 6u8, MatClass::CutOut => 7, MatClass::Pad => 3, MatClass::Wall => 4 };
                    if k == 0 { class_count[match class { MatClass::Textured => 0, MatClass::CutOut => 1, MatClass::Pad => 2, MatClass::Wall => 3 }] += 1; }
                    prepass::raster_tri(p, W, H, |x, y, b| {
                        let src = match class {
                            MatClass::CutOut => return, // AlphaToCoverage with alpha 1/9 on the 1-sample target: coverage 0
                            MatClass::Textured => match tex {
                                Some(tx) => {
                                    // the game uploads the zip's DDS bottom-up (D's rule): the GPU texture's row y is the file's row h − 1 − y, so
                                    // the file image is sampled at (u, 1 − v)
                                    let uvs = [b[0] * uv0[0][0] + b[1] * uv0[1][0] + b[2] * uv0[2][0], 1.0 - (b[0] * uv0[0][1] + b[1] * uv0[1][1] + b[2] * uv0[2][1])];
                                    // a card under the study switch: the 128/255 alpha test (GbxShadowAlphaThreshold) discards the cut-out
                                    let at = if *diff & 0x8000 != 0 || (*diff & 0xC000 == 0x4000 && link_alpha_tested.contains(&diff_name(model, *diff))) { Some(SHADOW_ALPHA_THRESHOLD) } else { None };
                                    match prepass::ps_basecolor(tx, &sampler, uvs, [dudx, -dvdx], [dudy, -dvdy], at, lm_scale) { Some(s) => s, None => return }
                                }
                                None => [0.0, 0.0, 0.0, lm_scale],
                            },
                            _ => { let c = konst.unwrap(); [c[0] * lm_scale, c[1] * lm_scale, c[2] * lm_scale, lm_scale] }
                        };
                        tgt.blend(x, y, src, cls);
                    });
                }
            }
        }
        // the zone tiles: the terrain constant over the tile mesh — or, for a textured tile material (Stadium's Grass), the 17023
        // class sampling the BaseColor texture at the quad's single uv set (= its lightmap uv; the file image at (u, 1 − v))
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
                    match &frozen.tile_tex {
                        Some(tx) => {
                            let uv0 = [0, 1, 2].map(|i| mesh.verts[tri[i] as usize].uv);
                            let (dudx, dudy) = prepass::attr_gradient(p, [uv0[0][0], uv0[1][0], uv0[2][0]]);
                            let (dvdx, dvdy) = prepass::attr_gradient(p, [uv0[0][1], uv0[1][1], uv0[2][1]]);
                            // LMTOOL_TILE_ALBEDO_SCALE=K (study): the tiles' sampled albedo scaled — the ground-bounce lever test on stpad's posts
                            let ascale: f32 = std::env::var("LMTOOL_TILE_ALBEDO_SCALE").ok().and_then(|v| v.parse().ok()).unwrap_or(1.0);
                            prepass::raster_tri(p, W, H, |x, y, b| {
                                let uvs = [b[0] * uv0[0][0] + b[1] * uv0[1][0] + b[2] * uv0[2][0], 1.0 - (b[0] * uv0[0][1] + b[1] * uv0[1][1] + b[2] * uv0[2][1])];
                                if let Some(mut s) = prepass::ps_basecolor(tx, &sampler, uvs, [dudx, -dvdx], [dudy, -dvdy], None, lm_scale) { s[0] *= ascale; s[1] *= ascale; s[2] *= ascale; tgt.blend(x, y, s, 6); }
                            });
                        }
                        None => prepass::raster_tri(p, W, H, |x, y, _| tgt.blend(x, y, src, 1)),
                    }
                }
            }
        }
        // the water tint (PS 17018 over the tile mesh with the frozen id map / tables)
        crate::prepass_check::tint_from_map(frozen, lm, k, &mut tgt);
        if k == 8 {
            let mut hist: std::collections::BTreeMap<u32, usize> = std::collections::BTreeMap::new();
            for i in 0..(W * H) as usize { let a = tgt.buf.data[i * 4 + 3]; if a > 0.0 { *hist.entry((a * 9.0).round() as u32).or_default() += 1; } }
            notes.push(format!("pre-pass run 8: alpha histogram (fragments per texel → texels) {:?}; items-only check: tiles {} instances", hist, lm.meshes.iter().enumerate().filter(|(k, _)| lm.inst_count[*k] >= 1000).map(|(k, _)| lm.inst_count[k]).sum::<usize>()));
        }
        (tgt, notes, class_count)
    };
    let runs: Vec<(Target, Vec<String>, [usize; 4])> = crate::pool::pool().map(9, |k| run_k(k));
    let mut class_count = [0usize; 4];
    for (k, (tgt, nts, cc)) in runs.into_iter().enumerate() {
        notes.extend(nts);
        if k == 0 { class_count = cc; }
        // PS 1109: run k accumulated into 16963
        for y in 0..H {
            for x in 0..W {
                for c in 0..4 {
                    let s = crate::gpufmt::quantise_f16(tgt.buf.get(x, y, c), crate::gpufmt::Rounding::Truncate);
                    acc.set(x, y, c, crate::gpufmt::quantise_f16(acc.get(x, y, c) + s, crate::gpufmt::Rounding::NearestEven));
                }
            }
        }
    }
    notes.push(format!("pre-pass: {} item instances → per run {} textured / {} cut-out (ATC: nothing) / {} pad / {} wall triangles; tiles at the frozen terrain constant {:?}, wall {:?}, pad {:?}", scene.instances.len(), class_count[0], class_count[1], class_count[2], class_count[3], frozen.tile_rgb, frozen.wall_rgb, frozen.pad_rgb));
    acc
}

/// PS 17043 → 16969 (sRGB UNORM8, alpha linear), as e2e stage 2.
pub fn mdiffuse8_of(attr: &Buf) -> Buf {
    let res = crate::ilightin::resolve_ps17043(attr, false);
    let mut out = Buf::new(W, H, 4);
    for y in 0..H {
        for x in 0..W {
            for c in 0..4 {
                let v = res.get(x, y, c);
                let v = if c < 3 { crate::gpufmt::linear_to_srgb(v) } else { v };
                out.set(x, y, c, crate::ilightin::unorm8_rt(v, crate::gpuenc::UnormRounding::NearestEven));
            }
        }
    }
    out
}

/// The ILightInput chain (e2e stages 5–8): PS 1038 mask, PS 17043 resolve, PS 1109 × the sRGB-decoded MDiffuse, PS 1335 × 8.
pub fn ilightinput_chain(sun: &Buf, mdiffuse8: &Buf) -> (Buf, Buf) {
    use crate::gpufmt::Rounding;
    let mask = crate::ilightin::quantise_unorm8(&crate::ilightin::mask_ps1038(sun, [1.0, 1.0, 0.0, 0.0], [[0.0; 4], [0.0; 4], [0.0; 4], [1.0; 4]], W, H), crate::gpuenc::UnormRounding::NearestEven);
    let stage = crate::ilightin::quantise_r11(&crate::ilightin::resolve_ps17043(sun, false), Rounding::Truncate);
    let mut mdiff_lin = mdiffuse8.clone();
    for y in 0..H {
        for x in 0..W {
            for c in 0..3 {
                mdiff_lin.set(x, y, c, crate::gpufmt::srgb_to_linear(mdiffuse8.get(x, y, c)));
            }
        }
    }
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
    let mut notes = Vec::new();
    let t0 = std::time::Instant::now();
    let cam = sun_camera(sbox, dir_in_world);
    let pw01 = cam.world_pw01_shadow(4096, 4096);
    notes.push(format!("sun camera: eye {:?} h {:?} near {} far {} (the scene box {:?}–{:?})", cam.eye, cam.h, cam.near(), cam.far(), sbox.min, sbox.max));
    let shadow = shadow_from_map(scene, lm, &cam, item_bytes, &mut notes).to_buf();
    if !quiet { eprintln!("setup-from-map: shadow map ({:.1}s)", t0.elapsed().as_secs_f32()); }
    let sun = sun_from_map(lm, &pw01, dir_in_world, light_rgb, &shadow);
    if !quiet { eprintln!("setup-from-map: direct sun ({:.1}s)", t0.elapsed().as_secs_f32()); }
    let attr = attr_from_map(scene, lm, frozen, item_bytes, &mut notes);
    if !quiet { eprintln!("setup-from-map: the nine pre-pass runs ({:.1}s)", t0.elapsed().as_secs_f32()); }
    let mdiffuse8 = mdiffuse8_of(&attr);
    let (ilightinput, coverage) = ilightinput_chain(&sun, &mdiffuse8);
    if !quiet { eprintln!("setup-from-map: ILightInput chain ({:.1}s)", t0.elapsed().as_secs_f32()); }
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
pub fn basecolor_texture(store: &mut mapgeom::store::DataStore, link: &str) -> Result<Option<(String, Texture, bool)>, String> {
    let mat = if link.to_ascii_uppercase().ends_with(".MATERIAL.GBX") { link.to_string() } else { format!("{link}.Material.Gbx") };
    let chain = mapgeom::envblock::material_chain(store, &mat);
    let pick = |name: &str| chain.bitmaps.iter().find(|(n, p)| n.eq_ignore_ascii_case(name) && !p.is_empty()).map(|(_, p)| p.clone());
    // BaseColor (CubeOut / TDSN), else Diffuse (PDiff), else BaseColorOp (DispIn: alpha varies → alpha-tested, RE 12's Q1 inference)
    let (slot, alpha_tested) = match pick("BaseColor").or_else(|| pick("Diffuse")) { Some(p) => (Some(p), false), None => (pick("BaseColorOp"), true) };
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
            Ok(Some((path, tx, _))) => { got.push(format!("tiles {tile_link} → BaseColor {path} ({}×{}, {} mips; the textured class over the tile quads)", tx.w, tx.h, tx.mips)); f.tile_tex = Some(tx); }
            Ok(None) => notes.push(format!("paktables: tiles {tile_link}: {e}; no BaseColor slot either (the frozen tile constant {:?} stays)", f.tile_rgb)),
            Err(e2) => notes.push(format!("paktables: tiles {tile_link}: {e}; BaseColor: {e2} (the frozen tile constant {:?} stays)", f.tile_rgb)),
        },
    }
    // the items' constant materials: every game-material link of the scene's models that the pack resolves
    let mut links: Vec<String> = Vec::new();
    for m in &scene.models {
        for l in &m.mat_links {
            if !links.contains(l) {
                links.push(l.clone());
            }
        }
    }
    for l in &links {
        match crate::paktables::material_constant(store, l) {
            Ok(mc) => {
                f.link_rgb.insert(l.to_ascii_lowercase(), mc.rgb);
                match mc.family {
                    crate::paktables::Family::PyPxzIds => { got.push(format!("{l} → {:?} (terrain ids {:?}; frozen Land {:?})", mc.rgb, mc.ids, f.wall_rgb)); f.wall_rgb = mc.rgb; }
                    crate::paktables::Family::PyPxzProjected => { got.push(format!("{l} → {:?} (projected, {}; frozen TrackWall {:?})", mc.rgb, mc.image, f.pad_rgb)); f.pad_rgb = mc.rgb; }
                }
            }
            Err(e) => {
                // THE 17023 CLASS (RE 12): an opaque textured non-PyPxz material (CubeOut / TDSN / TDSNI — np-tk3's Modifier\StadiumOnTerrain
                // TrackWallClipsInWorld / StructureInWorld, tiny-16's Technics, ScreenBack …) takes its BaseColor slot texture from the pack
                // at the mesh TEXCOORD0 (VS 17021 o1 = v1, PS 17023: TMapBaseColor × 1/9, alpha forced 1) — never a constant, never black
                match basecolor_texture(store, l) {
                    Ok(Some((path, tx, at))) => { got.push(format!("{l} → {} {path} ({}×{}, {} mips; the {} class)", if at { "BaseColorOp" } else { "BaseColor" }, tx.w, tx.h, tx.mips, if at { "17022 alpha-tested (A2C off: an inference)" } else { "17023 textured" })); f.link_tex.insert(l.to_ascii_lowercase(), (tx, at)); }
                    Ok(None) => notes.push(format!("pak: {l}: {e}; no BaseColor slot in its chain — the constant path keeps it")),
                    Err(e2) => notes.push(format!("pak: {l}: BaseColor: {e2}")),
                }
            }
        }
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
    // THE ID MAP AND THE PLANE TABLE (RE 11): the records' water quads — the SetWaterId draw — else the sea-zone map
    let size_m = [f.ids.w as f32, f.ids.h as f32];
    let wm = crate::waterid::water_id_map_of_records(store, records, size_m)?;
    if wm.quads > 0 {
        f.ids = wm.ids;
        f.top_by_plane = wm.plane_tops.iter().map(|t| [*t, 0.0, 0.0, 1.0]).collect();
        got.push(format!("water-id map from {} record water quads: {} of {} texels under water, g_WaterTop_ByPlanes {:?} (WORLD heights; the collection's local WaterTop {} is not a plane)", wm.quads, wm.texels, f.ids.w as usize * f.ids.h as usize, wm.plane_tops, w.top));
        for n in &wm.notes {
            notes.push(format!("water quads: {n}"));
        }
    } else {
        f.top_by_plane = vec![[w.top, 0.0, 0.0, 1.0]];
        let ch = f.ids.channels as usize;
        for (i, v) in f.ids.data.iter_mut().enumerate() { *v = if i % ch == 0 { 1.0 } else { 0.0 }; }
        got.push(format!("no record carries a water quad: the whole map is under the collection's water plane (id 1, plane 0, top {}) — a sea zone", w.top));
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
