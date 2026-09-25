//! `lmtool e2e-check ROOT` — the transcribed passes CHAINED: every stage runs on OUR previous stage's output (no
//! captured render target enters after the start), only the scene's frozen inputs do — the meshes, instance stream,
//! textures, cbuffer constants and pipeline state the capture's env/ exports hold (they are the game's CPU-side
//! products: the chart layout, the light camera, the material constants — the CPU rows of the table). Each stage
//! is compared with the captured intermediate at the same point, and the FIRST stage whose chained value diverges
//! from the captured one beyond one quantum of its target format is named with its numbers.
//!
//! Stages (the game's order, frames 127447 → 127448 of pwc-day):
//!
//! | # | stage | ours from | compared with |
//! |---|---|---|---|
//! | 1 | attribute pre-pass, nine runs (prepass.rs) | meshes / instances / textures | setup_ps1109 (16963 after runs 6, 7, 8) |
//! | 2 | PS 17043 → the MDiffuse 16969 (ilightin.rs) | stage 1 | setup_ps17043 |
//! | 3 | sun shadow map (shadowmap.rs, engineer B) | casters / instance tables | sun_shadow |
//! | 4 | direct sun (sunpass.rs, the baker) | stage 3 | sun_direct |
//! | 5 | PS 1038 mask, PS 17043 + PS 1109 multiply, PS 1335 × 8 (ilightin.rs) | stages 2 + 4 | setup_ps1038, setup_ps1109 (17095), setup_ps1335 |
//!
//! The sweep stages (the peels, LmILightDir_Set, AddAmbient, the probes, the H-basis accumulate, sweep 1, the
//! finalisation) follow in the bake path (`lmtool bake --raster --game-peel …`, engineers 2 / D / E) — this command
//! stops where the standalone kernels end and prints what the next stage would consume.

use crate::gpucmp::{compare, compare_where, Fmt, Report};
use crate::gpufmt::{quantise_f16, Rounding};
use crate::passdiff::{load_entry, read_manifest, Buf};
use crate::passdump::{Entry, Manifest};
use crate::prepass::{self, H, W};
use std::path::{Path, PathBuf};

fn arg(a: &[String], k: &str) -> Option<String> {
    a.iter().position(|x| x == k).and_then(|i| a.get(i + 1)).cloned()
}

fn flag(a: &[String], k: &str) -> bool {
    a.iter().any(|x| x == k)
}

/// One stage's verdict.
pub struct Stage {
    pub name: String,
    pub report: Report,
    /// the fraction of values beyond one quantum
    pub beyond_frac: f64,
}

impl Stage {
    fn diverged(&self, tol: f64) -> bool {
        self.beyond_frac > tol
    }
}

fn entry<'a>(m: &'a Manifest, pass: &str, frame: u32, sub: &str) -> Option<&'a Entry> {
    let mut v: Vec<&Entry> = m.passes.iter().filter(|e| e.pass == pass && e.frame == Some(frame) && e.file.contains(sub)).collect();
    v.sort_by_key(|e| e.eid_last.unwrap_or(0));
    v.last().copied()
}

fn m4(v: &serde_json::Value) -> [[f32; 4]; 4] {
    let mut o = [[0f32; 4]; 4];
    for i in 0..4 {
        for j in 0..4 {
            o[i][j] = v[i][j].as_f64().unwrap_or(0.0) as f32;
        }
    }
    o
}

fn v3(v: &serde_json::Value) -> [f32; 3] {
    [v[0].as_f64().unwrap_or(0.0) as f32, v[1].as_f64().unwrap_or(0.0) as f32, v[2].as_f64().unwrap_or(0.0) as f32]
}

fn v2(v: &serde_json::Value) -> [f32; 2] {
    [v[0].as_f64().unwrap_or(0.0) as f32, v[1].as_f64().unwrap_or(0.0) as f32]
}

/// Stage 3: the sun shadow map from the casters of the compute frame (the setup of `lmtool shadow-check`, with
/// engineer B's pinned model: fused arithmetic, UNORM16 nearest, the 2^-20 fixed point of z + bias).
pub fn shadow_map(root: &Path, frame: u32, m: &Manifest, draws: &serde_json::Value) -> Result<(crate::shadowmap::ShadowTarget, Report), String> {
    use crate::shadowmap::*;
    let env = root.join(format!("env/frame{frame}"));
    let shadow_ent = m.passes.iter().find(|e| e.pass == "sun_shadow" && e.frame == Some(frame)).ok_or("no sun_shadow entry")?;
    let (eid_first, eid_last) = (shadow_ent.eid.unwrap_or(0), shadow_ent.eid_last.unwrap_or(u64::MAX));
    // the manifest's entry carries eid_first in the raw json only; take every caster draw between the clear and the entry's last eid
    let mesh_json: serde_json::Value = serde_json::from_str(&crate::passdiff::repair_truncated_json(&std::fs::read_to_string(env.join("mesh.json")).map_err(|e| e.to_string())?)).map_err(|e| e.to_string())?;
    let samplers: serde_json::Value = std::fs::read_to_string(env.join("samplers.json")).ok().and_then(|t| serde_json::from_str(&crate::passdiff::repair_truncated_json(&t)).ok()).unwrap_or(serde_json::Value::Null);
    let textures: serde_json::Value = std::fs::read_to_string(env.join("textures.json")).ok().and_then(|t| serde_json::from_str(&crate::passdiff::repair_truncated_json(&t)).ok()).unwrap_or(serde_json::Value::Null);
    let all = draws.as_array().ok_or("draws")?;
    // the pass = the depth-only draws (PS 937 / 1147) up to the entry's last eid, after the last clear of the target
    let last_clear = all.iter().filter(|e| e["eid"].as_u64().unwrap_or(0) < eid_last && e["name"].as_str().map_or(false, |s| s.contains("ClearDepthStencil"))).map(|e| e["eid"].as_u64().unwrap()).filter(|&eid| eid >= eid_first.min(eid_last)).max().unwrap_or(0);
    let pass_draws: Vec<&serde_json::Value> = all.iter().filter(|e| { let eid = e["eid"].as_u64().unwrap_or(0); eid > last_clear && eid <= eid_last && e["flags"].as_str().map_or(false, |s| s.contains("Drawcall")) && matches!(e["Pixel"]["shader"].as_str(), Some("937") | Some("1147")) }).collect();
    let mut cam: Option<LightCamera> = None;
    let mut state: Option<RasterState> = None;
    let mut casters: Vec<CasterDraw> = Vec::new();
    for e in &pass_draws {
        let eid = e["eid"].as_u64().unwrap();
        let vs = e["Vertex"]["shader"].as_str().unwrap_or("?");
        let ps = e["Pixel"]["shader"].as_str().unwrap_or("?");
        let c = LightCamera { world_pr_camera: m4(&e["Vertex"]["cbuffers"]["SceneV"]["GbxV_WorldPrCamera"]) };
        cam.get_or_insert(c);
        let r = &e["raster"];
        let vp = e["viewport"].as_array().map(|v| { let mut o = [0f32; 6]; for (i, x) in v.iter().take(6).enumerate() { o[i] = x.as_f64().unwrap_or(0.0) as f32; } o }).unwrap_or([0.0, 0.0, 4096.0, 4096.0, 0.0, 1.0]);
        let st = RasterState { viewport: vp, depth_bias: r["depthBias"].as_i64().unwrap_or(0) as i32, slope_scaled_depth_bias: r["slopeScaledDepthBias"].as_f64().unwrap_or(0.0) as f32, depth_bias_clamp: r["depthBiasClamp"].as_f64().unwrap_or(0.0) as f32, cull_back: r["cull"].as_str() == Some("CullMode.Back"), front_ccw: r["frontCCW"].as_bool().unwrap_or(true), depth_clip: r["depthClip"].as_bool().unwrap_or(true), plane: PlaneEval::F64Snapped, coef_bits: 36, vertex_z_bits: 0 };
        state.get_or_insert(st);
        let drawv = &e["Vertex"]["cbuffers"]["DrawV"]["g_CBufferV_Draw"];
        let instance_start = drawv["InstanceStart"].as_u64().unwrap_or(0) as u32;
        let visual_to_world = drawv.get("VisualToWorld").and_then(|v| v.as_array()).map(|rows| { let mut o = [[0f32; 3]; 4]; for i in 0..4 { for j in 0..3 { o[i][j] = rows[i][j].as_f64().unwrap_or(0.0) as f32; } } o });
        let rec = mesh_json.as_array().unwrap().iter().find(|r| r["eid"].as_u64() == Some(eid)).ok_or(format!("mesh.json has no eid {eid}"))?;
        let vbs = rec["vertex_buffers"].as_array().unwrap();
        let vb = std::fs::read(env.join("mesh").join(vbs[0]["file"].as_str().unwrap())).map_err(|err| format!("vb of eid {eid}: {err}"))?;
        let stride = vbs[0]["stride"].as_u64().unwrap() as usize;
        let il = rec["input_layout"].as_array().unwrap();
        let find = |sem: &str, idx: u64| il.iter().find(|x| x["semantic"].as_str() == Some(sem) && x["index"].as_u64() == Some(idx)).map(|x| x["offset"].as_u64().unwrap() as usize);
        let pos_off = find("POSITION", 0).ok_or("POSITION0")?;
        let uv_off = if vs == "14613" { find("TEXCOORD", 0) } else { None };
        let ib = std::fs::read(env.join("mesh").join(rec["vsout"]["index_file"].as_str().ok_or("index file")?)).map_err(|e| e.to_string())?;
        let mesh = CasterMesh::parse(&vb, stride, pos_off, uv_off, &ib);
        let dyna = std::fs::read(env.join("bufs").join(format!("e{eid:06}_Vertex_srv0_2185.bin"))).unwrap_or_default();
        let sm = std::fs::read(env.join("bufs").join(format!("e{eid:06}_Vertex_srv1_17163.bin"))).unwrap_or_default();
        let tables = InstanceTables::parse(&dyna, &sm);
        let mut alpha = None;
        if ps == "1147" {
            let thr = e["Pixel"]["cbuffers"]["ShaderP"]["GbxShadowAlphaThreshold"].as_f64().ok_or("GbxShadowAlphaThreshold")? as f32;
            let tex_id = e["Pixel"]["srvs"][0]["tex"]["id"].as_str().ok_or("PS srv0")?;
            let tex_file = textures.as_array().and_then(|arr| arr.iter().find(|t| t["id"].as_u64().map(|i| i.to_string()) == Some(tex_id.to_string()) || t["id"].as_str() == Some(tex_id))).and_then(|t| t["file"].as_str()).map(|s| s.to_string()).unwrap_or(format!("e{eid:06}_{tex_id}.dds"));
            let tex_bytes = crate::passdiff::read_entry_bytes(root, &format!("env/frame{frame}/textures/{tex_file}"))?;
            let texture = AlphaTexture::from_dds(&tex_bytes)?;
            let aniso = samplers.as_array().and_then(|arr| arr.iter().find(|s| s["eid"].as_u64() == Some(eid))).and_then(|s| s["stages"]["Pixel"][0]["maxAnisotropy"].as_f64()).unwrap_or(16.0) as f32;
            alpha = Some(AlphaTest { threshold: thr, texture, max_anisotropy: aniso });
        }
        let inst = e["inst"].as_u64().unwrap_or(0).max(1) as u32;
        casters.push(CasterDraw { eid, mesh, instance_start, instance_count: inst, visual_to_world, tables, alpha, vsout: None });
    }
    let cam = cam.ok_or("no caster draw found for the shadow map")?;
    let st = state.unwrap();
    let game = load_entry(root, shadow_ent)?;
    let o = RunOpts { arith: Arith::Fma, unorm: UnormRounding::Nearest, alpha_test: true, depth_fixed_bits: 0, fixed_before_bias: false, step_fixed_k: 20, bias_round: 3, scale_ulps: 0.0 };
    let mut tgt = ShadowTarget::new(game.w, game.h);
    for (k, d) in casters.iter().enumerate() {
        draw_caster(d, &cam, &st, &mut tgt, (k + 1) as u8, &o);
    }
    let ours = tgt.to_buf();
    // one D16 step = 1/65535: compare in that quantum
    let mut r = Report::default();
    for y in 0..game.h {
        for x in 0..game.w {
            let (o, t) = (ours.get(x, y, 0), game.get(x, y, 0));
            r.values += 1;
            r.texels += 1;
            if o == t {
                r.exact += 1;
                continue;
            }
            r.texels_off += 1;
            let d = (o - t).abs();
            if d <= 1.0001 / 65535.0 {
                r.ulp1 += 1;
            } else {
                r.beyond += 1;
            }
            if d > r.max_abs {
                r.max_abs = d;
                r.worst = (x, y, 0, t, o);
            }
        }
    }
    Ok((tgt, r))
}

/// Stage 4: the direct-sun pass on a shadow map (the setup of `lmtool sun-check`).
pub fn direct_sun(root: &Path, frame: u32, draws: &serde_json::Value, shadow: &Buf) -> Result<crate::sunpass::Target, String> {
    let env = root.join(format!("env/frame{frame}"));
    let mesh_json: serde_json::Value = serde_json::from_str(&crate::passdiff::repair_truncated_json(&std::fs::read_to_string(env.join("mesh.json")).map_err(|e| e.to_string())?)).map_err(|e| e.to_string())?;
    let sun: Vec<&serde_json::Value> = draws.as_array().unwrap().iter().filter(|e| e.pointer("/Pixel/shader").and_then(|v| v.as_str()) == Some("15187")).collect();
    if sun.len() < 4 {
        return Err("fewer than 4 direct-sun draws (PS 15187)".into());
    }
    let first_block: Vec<u64> = sun.iter().take(4).map(|e| e["eid"].as_u64().unwrap()).collect();
    let mut meshes = Vec::new();
    let mut inst_first = Vec::new();
    let mut instance_bytes: Option<Vec<u8>> = None;
    for eid in &first_block {
        let rec = mesh_json.as_array().unwrap().iter().find(|r| r["eid"].as_u64() == Some(*eid)).ok_or(format!("mesh.json has no eid {eid}"))?;
        let vbs = rec["vertex_buffers"].as_array().unwrap();
        let vb0 = std::fs::read(env.join("mesh").join(vbs[0]["file"].as_str().unwrap())).map_err(|e| e.to_string())?;
        let ib = std::fs::read(env.join("mesh").join(rec["vsout"]["index_file"].as_str().ok_or("index file")?)).map_err(|e| e.to_string())?;
        let indices: Vec<u16> = ib.chunks_exact(2).map(|c| u16::from_le_bytes([c[0], c[1]])).collect();
        meshes.push(crate::sunpass::LmMesh { verts: crate::sunpass::parse_lm_vertices(&vb0), indices });
        inst_first.push((vbs[1]["offset"].as_u64().unwrap_or(0) / 48) as usize);
        if instance_bytes.is_none() {
            instance_bytes = Some(std::fs::read(env.join("mesh").join(vbs[1]["file"].as_str().unwrap())).map_err(|e| e.to_string())?);
        }
    }
    let instances = crate::sunpass::parse_instances(instance_bytes.as_ref().unwrap());
    let table_bytes = std::fs::read(env.join("bufs").join(format!("e{:06}_Vertex_srv0_16959.bin", first_block[0]))).unwrap_or_default();
    let table: Vec<[f32; 4]> = table_bytes.chunks_exact(16).map(|c| [f32::from_le_bytes(c[0..4].try_into().unwrap()), f32::from_le_bytes(c[4..8].try_into().unwrap()), f32::from_le_bytes(c[8..12].try_into().unwrap()), f32::from_le_bytes(c[12..16].try_into().unwrap())]).collect();
    let mut sd = Vec::new();
    for (k, e) in sun.iter().enumerate() {
        let ps = &e["Pixel"]["cbuffers"]["ShaderP"]["g_CBufferP"];
        let vs = &e["Vertex"]["cbuffers"]["ShaderV"]["g_CBuffer"];
        let inst = e["inst"].as_u64().unwrap_or(0).max(1) as usize;
        sd.push(crate::sunpass::SunDraw { eid: e["eid"].as_u64().unwrap(), mesh: k % 4, instance_first: inst_first[k % 4], instance_count: inst, scale_ss: v2(&vs["LM01_Scale_RasterSS"]), trans_ss: v2(&vs["LM01_Trans_RasterSS"]), world_pw01_shadow: m4(&ps["WorldPw01Shadow"]), dir_in_world: v3(&ps["DirInWorld"]), light_rgb: v3(&ps["LightRgb"]), out_scale: ps["OutScale"].as_f64().unwrap_or(0.0) as f32 });
    }
    let sm = crate::sunpass::ShadowMap { depth: shadow };
    Ok(crate::sunpass::run_sun_pass(&meshes, &instances, &table, &sd, &sm, W, H, crate::sunpass::BlendModel::TruncSrcRoundSum))
}

fn sun_target_to_buf(t: &crate::sunpass::Target) -> Buf {
    let mut b = Buf::new(t.w, t.h, 4);
    for i in 0..t.px.len() {
        for c in 0..4 {
            b.data[i * 4 + c] = t.px[i][c];
        }
    }
    b
}

pub fn run(a: Vec<String>) {
    let root = PathBuf::from(&a[1]);
    let pre_frame: u32 = arg(&a, "--pre-frame").map(|v| v.parse().expect("--pre-frame")).unwrap_or(127447);
    let frame: u32 = arg(&a, "--frame").map(|v| v.parse().expect("--frame")).unwrap_or(127448);
    let tol: f64 = arg(&a, "--tol").map(|v| v.parse().expect("--tol")).unwrap_or(0.0);
    let txt = std::fs::read_to_string(root.join("MANIFEST.json")).expect("MANIFEST.json");
    let m = read_manifest(&txt).expect("manifest");
    let mut stages: Vec<Stage> = Vec::new();
    let mut push = |name: &str, r: Report| {
        let frac = if r.values > 0 { r.beyond as f64 / r.values as f64 } else { 0.0 };
        println!("[{}] {name}: {}", stages.len() + 1, r.line());
        stages.push(Stage { name: name.to_string(), report: r, beyond_frac: frac });
    };
    let t0 = std::time::Instant::now();

    // ---- stage 1: the pre-pass, nine runs → 16963
    let acc = if flag(&a, "--skip-prepass") {
        let e = entry(&m, "setup_ps1109", pre_frame, "").expect("setup_ps1109");
        println!("[1] pre-pass SKIPPED: the captured 16963 ({}) enters", e.file);
        load_entry(&root, e).unwrap()
    } else {
        let acc = prepass_nine_runs(&root, pre_frame, frame);
        let e = entry(&m, "setup_ps1109", pre_frame, "").expect("setup_ps1109 (the frame's last run)");
        let cap = load_entry(&root, e).unwrap();
        let mut r = compare_where(&acc, &cap, 4, Fmt::F16, &|_, _| true);
        // the alpha channel alone: the raster
        let mut oa = Buf::new(W, H, 1);
        let mut ca = Buf::new(W, H, 1);
        for y in 0..H { for x in 0..W { oa.set(x, y, 0, acc.get(x, y, 3)); ca.set(x, y, 0, cap.get(x, y, 3)); } }
        let ra = compare_where(&oa, &ca, 1, Fmt::F16, &|_, _| true);
        r.texels = ra.exact; // borrow the field for the summary line below
        println!("    alpha (the nine-run coverage): {} of {} texels bit-identical", ra.exact, ra.values);
        r.texels = W as usize * H as usize;
        push("attribute pre-pass, nine runs → 16963 (RGBA16F; rgb = the materials' sRGB-decoded samples × coverage)", r);
        acc
    };

    // ---- stage 2: PS 17043 → 16969
    let res = crate::ilightin::resolve_ps17043(&acc, false);
    let mut mdiff8 = Buf::new(W, H, 4);
    for y in 0..H {
        for x in 0..W {
            for c in 0..4 {
                let v = res.get(x, y, c);
                let v = if c < 3 { crate::gpufmt::linear_to_srgb(v) } else { v };
                mdiff8.set(x, y, c, crate::ilightin::unorm8_rt(v, crate::gpuenc::UnormRounding::NearestEven));
            }
        }
    }
    {
        let e = entry(&m, "setup_ps17043", frame, "_16969").expect("setup_ps17043");
        let cap = load_entry(&root, e).unwrap();
        push("PS 17043 → the MDiffuse 16969 (sRGB UNORM8)", compare(&mdiff8, &cap, 4, Fmt::Unorm8));
    }

    // ---- stage 3: the sun shadow map
    let draws_bytes = crate::passdiff::read_entry_bytes(&root, &format!("logs/draws-frame{frame}.json")).expect("draws log");
    let draws: serde_json::Value = serde_json::from_slice(&draws_bytes).expect("draws json");
    let (shadow, rs) = shadow_map(&root, frame, &m, &draws).unwrap_or_else(|e| panic!("shadow map: {e}"));
    push("sun shadow map (D16 4096², engineer B's shadowmap.rs: fused VS, 2^-20 fixed z + bias, UNORM16 nearest)", rs);
    let shadow_buf = shadow.to_buf();

    // ---- stage 4: the direct sun on OUR shadow map
    let sun = direct_sun(&root, frame, &draws, &shadow_buf).unwrap_or_else(|e| panic!("direct sun: {e}"));
    let sun_buf = sun_target_to_buf(&sun);
    {
        let e = entry(&m, "sun_direct", frame, "").expect("sun_direct");
        let cap = load_entry(&root, e).unwrap();
        push("direct sun (RGBA16F 2048², the baker's sunpass.rs on OUR shadow map)", compare(&sun_buf, &cap, 4, Fmt::F16));
    }

    // ---- stage 5: the ILightInput chain on OUR sun_direct and OUR MDiffuse
    let mask = crate::ilightin::quantise_unorm8(&crate::ilightin::mask_ps1038(&sun_buf, [1.0, 1.0, 0.0, 0.0], [[0.0; 4], [0.0; 4], [0.0; 4], [1.0; 4]], W, H), crate::gpuenc::UnormRounding::NearestEven);
    {
        let e = entry(&m, "setup_ps1038", frame, "_17104").expect("setup_ps1038");
        push("PS 1038 coverage mask 17104 (R8)", compare(&mask, &load_entry(&root, e).unwrap(), 1, Fmt::Unorm8));
    }
    let stage = crate::ilightin::quantise_r11(&crate::ilightin::resolve_ps17043(&sun_buf, false), Rounding::Truncate);
    let mut mdiff_lin = mdiff8.clone();
    for y in 0..H { for x in 0..W { for c in 0..3 { mdiff_lin.set(x, y, c, crate::gpufmt::srgb_to_linear(mdiff8.get(x, y, c))); } } }
    let ilin = crate::ilightin::quantise_r11(&crate::finalprep::multiply_ps1109(&stage, &mdiff_lin, [1.0; 4]), Rounding::Truncate);
    {
        let e = entry(&m, "setup_ps1109", frame, "_17095").expect("setup_ps1109 of the compute frame");
        push("PS 17043 + PS 1109 (DstCol × the sRGB-decoded MDiffuse) → ILightInput 17095 (R11G11B10)", compare(&ilin, &load_entry(&root, e).unwrap(), 3, Fmt::R11G11B10));
    }
    let (mut c, mut w) = (ilin.clone(), mask.clone());
    let snaps: Vec<&Entry> = { let mut v: Vec<&Entry> = m.passes.iter().filter(|e| e.pass == "setup_ps1335" && e.frame == Some(frame)).collect(); v.sort_by_key(|e| e.eid_last.unwrap_or(0)); v };
    let mut eids: Vec<u64> = snaps.iter().filter_map(|e| e.eid_last).collect();
    eids.sort();
    eids.dedup();
    for (k, eid) in eids.iter().enumerate() {
        let (oc, ow) = crate::ilightin::dilate_ps1335(&c, &w);
        c = crate::ilightin::quantise_r11(&oc, Rounding::Truncate);
        w = crate::ilightin::quantise_unorm8(&ow, crate::gpuenc::UnormRounding::NearestEven);
        if k + 1 == eids.len() {
            let tc = load_entry(&root, snaps.iter().find(|e| e.eid_last == Some(*eid) && e.file.contains("_rt0_")).unwrap()).unwrap();
            let tw = load_entry(&root, snaps.iter().find(|e| e.eid_last == Some(*eid) && e.file.contains("_rt1_")).unwrap()).unwrap();
            push(&format!("PS 1335 × {} → the dilated ILightInput (R11G11B10)", eids.len()), compare(&c, &tc, 3, Fmt::R11G11B10));
            push(&format!("PS 1335 × {} → its coverage (R8)", eids.len()), compare(&w, &tw, 1, Fmt::Unorm8));
        }
    }
    if let Some(out) = arg(&a, "--dump-dir") {
        let dir = PathBuf::from(out);
        std::fs::create_dir_all(&dir).expect("dump dir");
        let f16 = |b: &Buf, name: &str| { let mut bytes = Vec::new(); for i in 0..(b.w * b.h) as usize { for k in 0..b.channels as usize { bytes.extend_from_slice(&crate::gpufmt::encode_f16(b.data[i * b.channels as usize + k], Rounding::NearestEven).to_le_bytes()); } } std::fs::write(dir.join(name), bytes).expect("write"); };
        f16(&acc, "e2e-16963-prepass-sum.rgba16f");
        f16(&sun_buf, "e2e-sun_direct.rgba16f");
        let mut r11 = Vec::new();
        for i in 0..(W * H) as usize { r11.extend_from_slice(&crate::gpufmt::pack_r11g11b10([c.data[i * 3], c.data[i * 3 + 1], c.data[i * 3 + 2]], Rounding::Truncate).to_le_bytes()); }
        std::fs::write(dir.join("e2e-ilightinput-17095.r11g11b10"), r11).expect("write");
        std::fs::write(dir.join("e2e-shadow-d16.dds"), shadow.to_dds()).expect("write");
        println!("wrote the chained intermediates under {}", dir.display());
    }

    // ---- the verdict
    println!("\nE2E ({:.0} s): {} stages", t0.elapsed().as_secs_f32(), stages.len());
    let first = stages.iter().position(|s| s.diverged(tol));
    for (i, s) in stages.iter().enumerate() {
        let mark = if Some(i) == first { " ← FIRST DIVERGENCE" } else if s.report.beyond == 0 { " (closed to the quantum)" } else { "" };
        println!("  {}. {}: {} exact / {} within 1 quantum / {} beyond of {} values ({:.4} % beyond){mark}", i + 1, s.name, s.report.exact, s.report.ulp1, s.report.beyond, s.report.values, 100.0 * s.beyond_frac);
    }
    match first {
        Some(i) => println!("first divergent stage: {} — worst value at ({}, {}) ch {}: captured {:.6} ours {:.6}", stages[i].name, stages[i].report.worst.0, stages[i].report.worst.1, stages[i].report.worst.2, stages[i].report.worst.3, stages[i].report.worst.4),
        None => println!("no stage beyond one quantum"),
    }
    let _ = quantise_f16;
}

/// Stage 1 as `prepass_check --all-runs` does it, returning our 16963 after the nine runs.
pub fn prepass_nine_runs(root: &Path, pre_frame: u32, env_frame: u32) -> Buf {
    let a: Vec<String> = vec!["prepass-check".into(), root.display().to_string(), "--frame".into(), pre_frame.to_string(), "--env-frame".into(), env_frame.to_string()];
    let _ = a;
    crate::prepass_check::nine_run_sum(root, pre_frame, env_frame)
}
