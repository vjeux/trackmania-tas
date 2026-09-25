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
    RasterState { viewport: [1.0, 1.0, 4094.0, 4094.0, 0.0, 1.0], depth_bias: -1, slope_scaled_depth_bias: -1.0, depth_bias_clamp: 0.0, cull_back: true, front_ccw: true, depth_clip: true, plane: PlaneEval::F64Snapped, coef_bits: 36, vertex_z_bits: 0 }
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
    let lcam = LightCamera { world_pr_camera: cam.world_pr_camera() };
    let st = shadow_state();
    let o = shadowmap::RunOpts { arith: shadowmap::Arith::Fma, unorm: shadowmap::UnormRounding::Nearest, alpha_test: true, depth_fixed_bits: 0, fixed_before_bias: false, step_fixed_k: 20, bias_round: 3, scale_ulps: 0.0 };
    let mut tgt = ShadowTarget::new(4096, 4096);
    let empty = InstanceTables { dyna_u32: Vec::new(), static_meshs: Vec::new() };
    let mut n_draws = 0usize;
    let mut alpha_cache: std::collections::HashMap<String, Option<std::sync::Arc<shadowmap::AlphaTexture>>> = std::collections::HashMap::new();
    // items: one opaque caster per instance + one alpha-tested caster per cut-out texture
    for inst in &scene.instances {
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
            let d = CasterDraw { eid: n_draws as u64, mesh: CasterMesh { uv0: Vec::new(), ..opaque }, instance_start: 0xffff_ffff, instance_count: 1, visual_to_world: Some(rows), tables: InstanceTables { dyna_u32: Vec::new(), static_meshs: Vec::new() }, alpha: None, vsout: None };
            shadowmap::draw_caster(&d, &lcam, &st, &mut tgt, 1, &o);
            n_draws += 1;
        }
        for (a, mesh) in cut {
            let name = model.alpha_tex.get(a as usize).cloned().unwrap_or_default();
            let tex = alpha_cache.entry(name.clone()).or_insert_with(|| item_bytes(&name).and_then(|b| shadowmap::AlphaTexture::from_dds(&b).ok()).map(std::sync::Arc::new)).clone();
            let Some(texture) = tex else { notes.push(format!("shadow: cut-out texture {name} not loaded — the caster is skipped")); continue };
            let texture = shadowmap::AlphaTexture { w: texture.w, h: texture.h, mips: texture.mips.clone() };
            let d = CasterDraw { eid: n_draws as u64, mesh, instance_start: 0xffff_ffff, instance_count: 1, visual_to_world: Some(rows), tables: InstanceTables { dyna_u32: Vec::new(), static_meshs: Vec::new() }, alpha: Some(shadowmap::AlphaTest { threshold: SHADOW_ALPHA_THRESHOLD, texture, max_anisotropy: 16.0 }), vsout: None };
            shadowmap::draw_caster(&d, &lcam, &st, &mut tgt, 2, &o);
            n_draws += 1;
        }
    }
    // the zone tiles: the LM tile mesh (the game's own vertex stream) at each tile instance's translation
    for (k, mesh) in lm.meshes.iter().enumerate() {
        if lm.inst_count[k] < 1000 {
            continue;
        }
        let cm = CasterMesh { pos: mesh.verts.iter().map(|v| v.pos).collect(), uv0: Vec::new(), indices: mesh.indices.clone() };
        for inst in lm.instances.iter().skip(lm.inst_first[k]).take(lm.inst_count[k]) {
            let rows = [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0], inst.t];
            let d = CasterDraw { eid: n_draws as u64, mesh: CasterMesh { pos: cm.pos.clone(), uv0: Vec::new(), indices: cm.indices.clone() }, instance_start: 0xffff_ffff, instance_count: 1, visual_to_world: Some(rows), tables: InstanceTables { dyna_u32: Vec::new(), static_meshs: Vec::new() }, alpha: None, vsout: None };
            shadowmap::draw_caster(&d, &lcam, &st, &mut tgt, 3, &o);
            n_draws += 1;
        }
    }
    // the decoration (sea box, terrain): opaque casters
    if !scene.decor.is_empty() {
        let mut m = CasterMesh { pos: Vec::new(), uv0: Vec::new(), indices: Vec::new() };
        for (i, t) in scene.decor.iter().enumerate() {
            if m.pos.len() + 3 > 65535 {
                let d = CasterDraw { eid: n_draws as u64, mesh: std::mem::replace(&mut m, CasterMesh { pos: Vec::new(), uv0: Vec::new(), indices: Vec::new() }), instance_start: 0xffff_ffff, instance_count: 1, visual_to_world: Some([[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0], [0.0; 3]]), tables: InstanceTables { dyna_u32: Vec::new(), static_meshs: Vec::new() }, alpha: None, vsout: None };
                shadowmap::draw_caster(&d, &lcam, &st, &mut tgt, 4, &o);
                n_draws += 1;
            }
            let _ = i;
            let base = m.pos.len() as u16;
            m.pos.extend_from_slice(&t.p);
            m.indices.extend_from_slice(&[base, base + 1, base + 2]);
        }
        if !m.indices.is_empty() {
            let d = CasterDraw { eid: n_draws as u64, mesh: m, instance_start: 0xffff_ffff, instance_count: 1, visual_to_world: Some([[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0], [0.0; 3]]), tables: empty, alpha: None, vsout: None };
            shadowmap::draw_caster(&d, &lcam, &st, &mut tgt, 4, &o);
            n_draws += 1;
        }
    }
    notes.push(format!("shadow: {n_draws} caster draws ({} item instances, {} decoration triangles)", scene.instances.len(), scene.decor.len()));
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
    let t = sunpass::run_sun_pass(&lm.meshes, &lm.instances, &lm.table, &draws, &sm, W, H, sunpass::BlendModel::TruncSrcRoundSum);
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

pub fn classify(model: &crate::geometry::ModelGeom, name: &str, t: &crate::geometry::Tri) -> MatClass {
    if t.alpha != u16::MAX {
        return MatClass::CutOut;
    }
    if t.diff != u16::MAX {
        return MatClass::Textured;
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
    let mut tex_cache: std::collections::HashMap<String, Option<Texture>> = std::collections::HashMap::new();
    let mut acc = Buf::new(W, H, 4);
    let lm_scale = 1.0f32 / 9.0;
    let mut class_count = [0usize; 4];
    // the items: E's LM meshes (the game's own LM stream: exact TexCoord1) per model in first-appearance order; the material
    // of an LM triangle is looked up in the port's model triangles by vertex position (uv0 / the texture / the cut-out)
    let mut by_model: Vec<usize> = Vec::new();
    for inst in &scene.instances {
        if !by_model.contains(&inst.model) {
            by_model.push(inst.model);
        }
    }
    let key = |p: [f32; 3]| -> [i32; 3] { [(p[0] * 1024.0).round() as i32, (p[1] * 1024.0).round() as i32, (p[2] * 1024.0).round() as i32] };
    struct ItemMat { class: MatClass, uv0: [f32; 2], diff: u16 }
    // each LM mesh is paired with the port model whose triangle vertices it shares (E's mesh order is by model, but the
    // pairing is made on the geometry itself)
    let lookups: Vec<(usize, std::collections::HashMap<[i32; 3], ItemMat>)> = by_model.iter().map(|&model_idx| {
        let model = &scene.models[model_idx];
        let name = &scene.model_names[model_idx];
        let mut map: std::collections::HashMap<[i32; 3], ItemMat> = std::collections::HashMap::new();
        for t in &model.tris {
            let class = classify(model, name, t);
            for v in 0..3 {
                map.entry(key(t.p[v])).or_insert(ItemMat { class, uv0: t.uv0[v], diff: t.diff });
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
    for k in 0..9usize {
        let mut tgt = Target::new();
        for (mk, li) in item_meshes.iter() {
            let mesh = &lm.meshes[*mk];
            let (model_idx, lookup) = &lookups[*li];
            let model_idx = *model_idx;
            let model = &scene.models[model_idx];
            let name = &scene.model_names[model_idx];
            let default_class = if model.mat_links.iter().any(|l| { let l = l.to_ascii_lowercase(); l.contains("trackwall") || l.contains("\\modifier\\") }) { MatClass::Pad } else { MatClass::Wall };
            for inst in lm.instances.iter().skip(lm.inst_first[*mk]).take(lm.inst_count[*mk]) {
                let rlm = prepass::raster_lm_for(inst.st, k);
                for tri in mesh.indices.chunks_exact(3) {
                    let vs = [&mesh.verts[tri[0] as usize], &mesh.verts[tri[1] as usize], &mesh.verts[tri[2] as usize]];
                    let mats: [Option<&ItemMat>; 3] = [lookup.get(&key(vs[0].pos)), lookup.get(&key(vs[1].pos)), lookup.get(&key(vs[2].pos))];
                    let class = mats.iter().flatten().next().map(|m| m.class).unwrap_or(default_class);
                    let p = [prepass::viewport(prepass::lm_ndc(vs[0].uv, &rlm), W, H), prepass::viewport(prepass::lm_ndc(vs[1].uv, &rlm), W, H), prepass::viewport(prepass::lm_ndc(vs[2].uv, &rlm), W, H)];
                    let uv0 = [mats[0].map(|m| m.uv0).unwrap_or([0.0; 2]), mats[1].map(|m| m.uv0).unwrap_or([0.0; 2]), mats[2].map(|m| m.uv0).unwrap_or([0.0; 2])];
                    let (dudx, dudy) = prepass::attr_gradient(p, [uv0[0][0], uv0[1][0], uv0[2][0]]);
                    let (dvdx, dvdy) = prepass::attr_gradient(p, [uv0[0][1], uv0[1][1], uv0[2][1]]);
                    let tex = match class {
                        MatClass::Textured => {
                            let diff = mats.iter().flatten().next().map(|m| m.diff).unwrap_or(u16::MAX);
                            let tn = model.diff_tex.get(diff as usize).cloned().unwrap_or_default();
                            tex_cache.entry(tn.clone()).or_insert_with(|| item_bytes(&tn).and_then(|b| texsample::parse_dds(&b, Bc1Decode::Expand8Round).ok()).map(|mut tx| { tx.decode_srgb(); tx })).as_ref()
                        }
                        _ => None,
                    };
                    let konst = match class {
                        MatClass::Pad => Some(frozen.pad_rgb),
                        MatClass::Wall => Some(frozen.wall_rgb),
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
                                    let uv = [b[0] * uv0[0][0] + b[1] * uv0[1][0] + b[2] * uv0[2][0], 1.0 - (b[0] * uv0[0][1] + b[1] * uv0[1][1] + b[2] * uv0[2][1])];
                                    match prepass::ps_basecolor(tx, &sampler, uv, [dudx, -dvdx], [dudy, -dvdy], None, lm_scale) { Some(s) => s, None => return }
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
        // the zone tiles: the terrain constant over the tile mesh
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
                    prepass::raster_tri(p, W, H, |x, y, _| tgt.blend(x, y, src, 1));
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
