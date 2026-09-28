//! THE swd6 REPLAY (port engineer G2, 2026-09-28): the stpad sweep-0 capture (passcap/stpad-sweep0/f1617 — ONE direction of the
//! Stadium Day q3 bake as three peels: the world SET at eids 32–149, fitted peel A (SET runs 1970…6062), fitted peel B (7883…11975),
//! then the H-basis pass 12009–12126) exported by baker-5 as per-run texture dumps (`tex/`, `tail/`), the H-basis MRTs (`hbasis/`)
//! and the SET draws' RAW vertex / instance buffers with their input layouts (`mesh/mesh.json`, `vb_<res>.bin`, `e<eid>_vsout_indices.bin`
//! = the original index buffer of the draw — 0..n_verts, verified on the palm's 13 266 indices).
//!
//! `lmtool swd6-set` runs the transcribed LmILightDir_Set block (lmaccum: VS 17111/17115 + PS 17112 = swd6's VS 12427/12431 + PS 12428)
//! over ONE captured peel layer for a range of eids, starting from the captured ilightdir target before the block, and compares the
//! result with the captured target after it — the same test that proved pwc-day bit-exact, now on a PACKED vegetation scene (the
//! stpad palms AV06220002 ×4 = eid 2030: 4 422 triangles into 20×20-texel charts). Per texel of the palm charts the verdict names
//! the pass: a SET mismatch is a raster / depth-compare question; a match moves the packed-card term to the H-basis accumulate.

use crate::lmaccum::{DirTarget, LayerTargets, LmScene, SetCb, SetDraw, LmRasterCb};
use crate::passdiff::Buf;
use serde_json::Value;
use std::path::Path;

fn jf(v: &Value) -> f32 { v.as_f64().unwrap_or(0.0) as f32 }
fn jv2(v: &Value) -> [f32; 2] { [jf(&v[0]), jf(&v[1])] }
fn jv3(v: &Value) -> [f32; 3] { [jf(&v[0]), jf(&v[1]), jf(&v[2])] }
fn jm4(v: &Value) -> [[f32; 4]; 4] { let mut o = [[0f32; 4]; 4]; for i in 0..4 { for j in 0..4 { o[i][j] = jf(&v[i][j]); } } o }

/// The LM scene of a swd6 frame's SET draws in `eids` (draws.json + mesh/mesh.json + the raw buffers): one LM mesh per draw
/// (a draw's vertex buffer + its index list), the shared instance stream, and the SetDraw list in eid order.
pub fn scene_and_draws(frame_dir: &Path, eids: &[u64], set_ps: &str, fitted_vs: &str) -> Result<(LmScene, Vec<SetDraw>, Vec<u64>), String> {
    let draws: Vec<Value> = serde_json::from_str(&std::fs::read_to_string(frame_dir.join("draws.json")).map_err(|e| format!("draws.json: {e}"))?).map_err(|e| format!("draws.json: {e}"))?;
    let mesh_dir = frame_dir.join("mesh");
    let meshes_json: Vec<Value> = serde_json::from_str(&std::fs::read_to_string(mesh_dir.join("mesh.json")).map_err(|e| format!("mesh.json: {e}"))?).map_err(|e| format!("mesh.json: {e}"))?;
    let mut sc = LmScene { meshes: Vec::new(), inst_first: Vec::new(), inst_count: Vec::new(), instances: Vec::new(), table: Vec::new(), eids: Vec::new(), frag_lists: Default::default(), fitted_world_box: None, rec_of: Vec::new(), st_src: Vec::new(), port_inst: Vec::new(), caster_tris: Vec::new() };
    let mut out_draws = Vec::new();
    let mut instance_res: Option<(String, Vec<u8>)> = None;
    let mut vb_cache: std::collections::HashMap<String, std::sync::Arc<Vec<u8>>> = Default::default();
    let mut chart_idx_table_refs = 0usize;
    for &eid in eids {
        let Some(d) = draws.iter().find(|d| d["eid"].as_u64() == Some(eid)) else { return Err(format!("eid {eid}: not in draws.json")) };
        let ps = d.pointer("/Pixel/shader").and_then(|v| v.as_str()).unwrap_or("");
        if ps != set_ps { return Err(format!("eid {eid}: PS {ps}, expected the SET PS {set_ps}")); }
        let vs = d.pointer("/Vertex/shader").and_then(|v| v.as_str()).unwrap_or("").to_string();
        let idx = d["idx"].as_u64().unwrap_or(0) as usize;
        let inst = d["inst"].as_u64().unwrap_or(1).max(1) as usize;
        let Some(m) = meshes_json.iter().find(|m| m["eid"].as_u64() == Some(eid)) else { return Err(format!("eid {eid}: not in mesh.json")) };
        let vbs = m["vertex_buffers"].as_array().ok_or_else(|| format!("eid {eid}: vertex_buffers"))?;
        let vb0 = vbs.iter().find(|b| b["slot"].as_u64() == Some(0)).ok_or_else(|| format!("eid {eid}: slot-0 vertex buffer"))?;
        let vb1 = vbs.iter().find(|b| b["slot"].as_u64() == Some(1)).ok_or_else(|| format!("eid {eid}: slot-1 instance buffer"))?;
        if vb0["stride"].as_u64() != Some(40) || vb1["stride"].as_u64() != Some(48) { return Err(format!("eid {eid}: strides {:?}/{:?}, expected 40/48", vb0["stride"], vb1["stride"])); }
        let f0 = vb0["file"].as_str().ok_or_else(|| format!("eid {eid}: vb0 file"))?.to_string();
        let bytes0 = match vb_cache.get(&f0) { Some(b) => b.clone(), None => { let b = std::sync::Arc::new(std::fs::read(mesh_dir.join(&f0)).map_err(|e| format!("{f0}: {e}"))?); vb_cache.insert(f0.clone(), b.clone()); b } };
        let off0 = vb0["offset"].as_u64().unwrap_or(0) as usize;
        let mut verts = crate::sunpass::parse_lm_vertices(&bytes0[off0..]);
        // the index list = the draw's original index buffer (RenderDoc's post-VS index list keeps the input indices)
        let idx_file = m.pointer("/vsout/index_file").and_then(|v| v.as_str()).map(|s| s.to_string()).unwrap_or_else(|| format!("e{eid:06}_vsout_indices.bin"));
        let ib = std::fs::read(mesh_dir.join(&idx_file)).map_err(|e| format!("{idx_file}: {e}"))?;
        let indices: Vec<u16> = ib.chunks_exact(2).map(|c| u16::from_le_bytes([c[0], c[1]])).collect();
        if indices.len() != idx { return Err(format!("eid {eid}: {} indices in {idx_file}, the draw has {idx}", indices.len())); }
        let n_used = indices.iter().map(|&i| i as usize + 1).max().unwrap_or(0);
        if n_used > verts.len() { return Err(format!("eid {eid}: index {} beyond the {} vertices of {f0}", n_used - 1, verts.len())); }
        verts.truncate(n_used);
        chart_idx_table_refs += verts.iter().filter(|v| v.chart_idx < 0xffff).count();
        // the instance stream (one resource for the whole frame), the draw's range by byte offset
        let f1 = vb1["file"].as_str().ok_or_else(|| format!("eid {eid}: vb1 file"))?.to_string();
        if instance_res.as_ref().map(|(n, _)| n != &f1).unwrap_or(false) { return Err(format!("eid {eid}: a second instance stream {f1}")); }
        if instance_res.is_none() { instance_res = Some((f1.clone(), std::fs::read(mesh_dir.join(&f1)).map_err(|e| format!("{f1}: {e}"))?)); }
        let off1 = vb1["offset"].as_u64().unwrap_or(0) as usize;
        if off1 % 48 != 0 { return Err(format!("eid {eid}: instance offset {off1} is not a multiple of 48")); }
        let k = sc.meshes.len();
        sc.meshes.push(crate::sunpass::LmMesh { verts, indices });
        sc.inst_first.push(off1 / 48);
        sc.inst_count.push(inst);
        sc.eids.push(eid);
        sc.caster_tris.push(Vec::new());
        let vcb = &d["Vertex"]["cbuffers"]["ShaderV"]["g_CBufferV"];
        let pcb = &d["Pixel"]["cbuffers"]["ShaderP"]["g_CBufferP"];
        let world_box = if vs == fitted_vs { Some([jv2(&vcb["WorldBoxMinXZ"]), jv2(&vcb["WorldBoxMaxXZ"])]) } else { None };
        out_draws.push(SetDraw {
            eid,
            mesh: k,
            instance_first: off1 / 48,
            instance_count: inst,
            raster: LmRasterCb { scale_ss: jv2(&vcb["LM01_Scale_RasterSS"]), trans_ss: jv2(&vcb["LM01_Trans_RasterSS"]) },
            cb: SetCb { world_pw01_shadow: jm4(&pcb["WorldPw01Shadow"]), peel_dir: jv3(&pcb["PeelDirInW"]) },
            world_box,
        });
    }
    let (_, ib) = instance_res.ok_or("no draws")?;
    sc.instances = crate::sunpass::parse_instances(&ib);
    for k in 0..sc.meshes.len() {
        if sc.inst_first[k] + sc.inst_count[k] > sc.instances.len() { return Err(format!("eid {}: instances {}..{} beyond the stream's {}", sc.eids[k], sc.inst_first[k], sc.inst_first[k] + sc.inst_count[k], sc.instances.len())); }
    }
    if chart_idx_table_refs > 0 { return Err(format!("{chart_idx_table_refs} vertices index the chart-ST table (g_TcLM_ST_LM01, Vertex SRV t0), which the export does not carry")); }
    Ok((sc, out_draws, eids.to_vec()))
}

/// A DirTarget seeded from a captured R11G11B10 target.
pub fn target_from_buf(b: &Buf) -> DirTarget {
    let mut t = DirTarget::cleared(b.w, b.h);
    for y in 0..b.h { for x in 0..b.w { t.px[(y * b.w + x) as usize] = crate::gpufmt::pack_r11g11b10([b.get(x, y, 0), b.get(x, y, 1), b.get(x, y, 2)], crate::gpufmt::Rounding::Truncate); } }
    t
}

/// The atlas pixel rects (2048² target) of a draw's instances from their STs: [x0, y0, x1, y1) — the mesh's uv extent through
/// `clip = ST.xy·uv + ST.zw` (01 space, the raster's Scale (2, −2) flips y).
pub fn instance_rects(sc: &LmScene, k: usize, w: u32, h: u32) -> Vec<[u32; 4]> {
    let m = &sc.meshes[k];
    let (mut u0, mut u1, mut v0, mut v1) = (f32::MAX, f32::MIN, f32::MAX, f32::MIN);
    for v in &m.verts { u0 = u0.min(v.uv[0]); u1 = u1.max(v.uv[0]); v0 = v0.min(v.uv[1]); v1 = v1.max(v.uv[1]); }
    (sc.inst_first[k]..sc.inst_first[k] + sc.inst_count[k]).map(|ii| {
        let st = sc.instances[ii].st;
        let ax = |u: f32| (st[0] * u + st[2]) * w as f32;
        let ay = |v: f32| (st[1] * v + st[3]) * h as f32;
        let (xa, xb, ya, yb) = (ax(u0), ax(u1), ay(v0), ay(v1));
        [xa.min(xb).floor().max(0.0) as u32, ya.min(yb).floor().max(0.0) as u32, xa.max(xb).ceil().min(w as f32) as u32, ya.max(yb).ceil().min(h as f32) as u32]
    }).collect()
}

/// Per-rect comparison of the replayed target with the captured one: (touched, exact, within one R11G11B10 quantum, worse, ours-only, game-only).
pub fn compare_rect(ours: &DirTarget, game: &Buf, r: [u32; 4]) -> (usize, usize, usize, usize, usize, usize, Vec<(u32, u32, [f32; 3], [f32; 3])>) {
    let (mut n, mut exact, mut q, mut worse, mut oo, mut go) = (0usize, 0usize, 0usize, 0usize, 0usize, 0usize);
    let mut worst: Vec<(u32, u32, [f32; 3], [f32; 3])> = Vec::new();
    for y in r[1]..r[3] { for x in r[0]..r[2] {
        let o = ours.rgb(x, y);
        let g = [game.get(x, y, 0), game.get(x, y, 1), game.get(x, y, 2)];
        let (oz, gz) = (o.iter().all(|c| *c == 0.0), g.iter().all(|c| *c == 0.0));
        if oz && gz { continue; }
        n += 1;
        if oz { go += 1; continue; }
        if gz { oo += 1; continue; }
        let gq = crate::gpufmt::quantise_r11g11b10(g, crate::gpufmt::Rounding::Truncate);
        if o == gq { exact += 1; continue; }
        let within = (0..3).all(|c| { let step = (gq[c].abs() * (1.0 / 32.0)).max(1e-6); (o[c] - gq[c]).abs() <= step * 1.001 });
        if within { q += 1; } else { worse += 1; if worst.len() < 8 { worst.push((x, y, o, gq)); } }
    } }
    (n, exact, q, worse, oo, go, worst)
}

pub fn cli(a: &[String]) -> Result<(), String> {
    // lmtool swd6-set FRAME_DIR --eids A..B --before F --layer-color F --layer-depth F --after F [--set-ps 12428] [--fitted-vs 12431]
    //   [--report-eid E]... [--probe x,y] [--cmp unorm|float]
    let f = |k: &str| a.iter().position(|x| x == k).and_then(|i| a.get(i + 1)).cloned();
    let frame_dir = std::path::PathBuf::from(a.get(1).ok_or("FRAME_DIR")?);
    let eids: Vec<u64> = {
        let s = f("--eids").ok_or("--eids A..B or a,b,c")?;
        if let Some((lo, hi)) = s.split_once("..") { (lo.trim().parse::<u64>().map_err(|e| e.to_string())?..=hi.trim().parse::<u64>().map_err(|e| e.to_string())?).collect() } else { s.split(',').map(|t| t.trim().parse::<u64>().map_err(|e| e.to_string())).collect::<Result<_, _>>()? }
    };
    let set_ps = f("--set-ps").unwrap_or_else(|| "12428".into());
    let fitted_vs = f("--fitted-vs").unwrap_or_else(|| "12431".into());
    // the eids in the range that ARE SET draws (the range may span clears / other draws)
    let draws_json: Vec<Value> = serde_json::from_str(&std::fs::read_to_string(frame_dir.join("draws.json")).map_err(|e| format!("draws.json: {e}"))?).map_err(|e| format!("draws.json: {e}"))?;
    let set_eids: Vec<u64> = eids.iter().copied().filter(|e| draws_json.iter().any(|d| d["eid"].as_u64() == Some(*e) && d.pointer("/Pixel/shader").and_then(|v| v.as_str()) == Some(set_ps.as_str()))).collect();
    let t0 = std::time::Instant::now();
    // --mesh-eids A..B: the LM scene from another run's exported draws (the meshes are exported once, for the first fitted run); the
    // replayed run's draws take their cbuffers from their own eids, matched by position
    let (sc, draws) = if let Some(me) = f("--mesh-eids") {
        let (lo, hi) = me.split_once("..").ok_or("--mesh-eids A..B")?;
        let all: Vec<u64> = (lo.trim().parse::<u64>().map_err(|e| e.to_string())?..=hi.trim().parse::<u64>().map_err(|e| e.to_string())?).collect();
        let mesh_eids: Vec<u64> = all.iter().copied().filter(|e| draws_json.iter().any(|d| d["eid"].as_u64() == Some(*e) && d.pointer("/Pixel/shader").and_then(|v| v.as_str()) == Some(set_ps.as_str()))).collect();
        let (sc, tdraws, _) = scene_and_draws(&frame_dir, &mesh_eids, &set_ps, &fitted_vs)?;
        if tdraws.len() != set_eids.len() { return Err(format!("{} SET draws in --eids vs {} in --mesh-eids", set_eids.len(), tdraws.len())); }
        let mut draws = Vec::new();
        for (k, eid) in set_eids.iter().enumerate() {
            let d = draws_json.iter().find(|d| d["eid"].as_u64() == Some(*eid)).unwrap();
            let t = &tdraws[k];
            if d["idx"].as_u64().unwrap_or(0) as usize != sc.meshes[t.mesh].indices.len() || d["inst"].as_u64().unwrap_or(1) as usize != t.instance_count { return Err(format!("eid {eid}: idx/inst differ from the mesh run's draw {}", t.eid)); }
            let vs = d.pointer("/Vertex/shader").and_then(|v| v.as_str()).unwrap_or("");
            let vcb = &d["Vertex"]["cbuffers"]["ShaderV"]["g_CBufferV"];
            let pcb = &d["Pixel"]["cbuffers"]["ShaderP"]["g_CBufferP"];
            draws.push(SetDraw { eid: *eid, mesh: t.mesh, instance_first: t.instance_first, instance_count: t.instance_count, raster: LmRasterCb { scale_ss: jv2(&vcb["LM01_Scale_RasterSS"]), trans_ss: jv2(&vcb["LM01_Trans_RasterSS"]) }, cb: SetCb { world_pw01_shadow: jm4(&pcb["WorldPw01Shadow"]), peel_dir: jv3(&pcb["PeelDirInW"]) }, world_box: if vs == fitted_vs { Some([jv2(&vcb["WorldBoxMinXZ"]), jv2(&vcb["WorldBoxMaxXZ"])]) } else { None } });
        }
        (sc, draws)
    } else { let (sc, draws, _) = scene_and_draws(&frame_dir, &set_eids, &set_ps, &fitted_vs)?; (sc, draws) };
    let n_tris: usize = sc.meshes.iter().map(|m| m.indices.len() / 3).sum();
    let n_inst: usize = sc.inst_count.iter().sum();
    println!("swd6-set: {} SET draws (eids {}..{}), {} LM meshes, {n_tris} triangles, {n_inst} instances of the frame's {} ({:.1}s)", draws.len(), set_eids.first().unwrap_or(&0), set_eids.last().unwrap_or(&0), sc.meshes.len(), sc.instances.len(), t0.elapsed().as_secs_f32());
    let d0 = &draws[0];
    println!("  D {:?}; raster Scale {:?} Trans {:?}; world box {:?}; WorldPw01Shadow row 3 {:?}", d0.cb.peel_dir, d0.raster.scale_ss, d0.raster.trans_ss, d0.world_box, d0.cb.world_pw01_shadow[3]);
    let load = |k: &str| -> Result<Buf, String> { let file = f(k).ok_or_else(|| format!("{k} FILE"))?; let fmt = if file.contains("_ds_") { "R16_UNORM" } else { "" }; crate::passdiff::load_file(&frame_dir, &file, fmt, 0, 0, 0).map_err(|e| format!("{file}: {e}")) };
    let before = load("--before")?;
    let color = load("--layer-color")?;
    let depth = load("--layer-depth")?;
    let after = load("--after")?;
    println!("  before {}×{}, layer colour {}×{} depth {}×{} ({} ch), after {}×{}", before.w, before.h, color.w, color.h, depth.w, depth.h, depth.channels, after.w, after.h);
    let cmp = match f("--cmp").as_deref() { Some("float") => crate::lmaccum::DepthCompare::Float, _ => crate::lmaccum::DepthCompare::Unorm16Round };
    let probe = f("--probe").and_then(|s| { let v: Vec<u32> = s.split(',').filter_map(|t| t.trim().parse().ok()).collect(); if v.len() == 2 { Some((v[0], v[1])) } else { None } });
    let mut tgt = target_from_buf(&before);
    let layer = LayerTargets { color: &color, depth: &depth };
    let t1 = std::time::Instant::now();
    crate::lmaccum::run_set_block_probe(&sc.meshes, &sc.instances, &sc.table, &draws, &layer, cmp, &mut tgt, probe);
    println!("  replayed in {:.1}s", t1.elapsed().as_secs_f32());
    let c = crate::lmaccum::compare_dir(&tgt, &after);
    println!("  WHOLE TARGET vs the captured after-target: {}", crate::lmaccum::fmt_dircmp(&c));
    // the block's own footprint: texels the block CHANGED on either side (before ≠ after in the capture, or before ≠ ours)
    let (mut chg_game, mut chg_ours, mut chg_both, mut chg_exact) = (0usize, 0usize, 0usize, 0usize);
    for y in 0..after.h { for x in 0..after.w {
        let b = [before.get(x, y, 0), before.get(x, y, 1), before.get(x, y, 2)];
        let g = [after.get(x, y, 0), after.get(x, y, 1), after.get(x, y, 2)];
        let o = tgt.rgb(x, y);
        let bq = crate::gpufmt::quantise_r11g11b10(b, crate::gpufmt::Rounding::Truncate);
        let gq = crate::gpufmt::quantise_r11g11b10(g, crate::gpufmt::Rounding::Truncate);
        let cg = gq != bq; let co = o != bq;
        if cg { chg_game += 1; } if co { chg_ours += 1; }
        if cg && co { chg_both += 1; if o == gq { chg_exact += 1; } }
    } }
    println!("  texels the block changed: game {chg_game}, ours {chg_ours}, both {chg_both} (of which bit-identical {chg_exact})");
    // per reported draw: its instances' chart rects
    let report: Vec<u64> = a.iter().enumerate().filter(|(_, x)| *x == "--report-eid").filter_map(|(i, _)| a.get(i + 1).and_then(|s| s.parse().ok())).collect();
    for eid in report {
        let Some(k) = sc.eids.iter().position(|e| *e == eid) else { println!("  --report-eid {eid}: not a SET draw of this block"); continue };
        let rects = instance_rects(&sc, k, after.w, after.h);
        println!("  eid {eid}: mesh {k} ({} verts, {} tris), {} instances", sc.meshes[k].verts.len(), sc.meshes[k].indices.len() / 3, rects.len());
        for (i, r) in rects.iter().enumerate() {
            let inst = &sc.instances[sc.inst_first[k] + i];
            let (n, exact, q, worse, oo, go, worst) = compare_rect(&tgt, &after, *r);
            println!("    instance {} at ({:.1}, {:.1}, {:.1}) rect x {}..{} y {}..{} ({}×{}): {n} non-zero texels: exact {exact}, ≤1 quantum {q}, worse {worse}, ours-only {oo}, game-only {go}", sc.inst_first[k] + i, inst.t[0], inst.t[1], inst.t[2], r[0], r[2], r[1], r[3], r[2] - r[0], r[3] - r[1]);
            for (x, y, o, g) in worst.iter().take(4) { println!("      worst ({x}, {y}): ours ({:.4}, {:.4}, {:.4}) game ({:.4}, {:.4}, {:.4})", o[0], o[1], o[2], g[0], g[1], g[2]); }
        }
    }
    Ok(())
}

/// One exported run of the capture (baker-5's tex.json): its eid range, shaders and files.
#[derive(Clone, Debug)]
pub struct Run { pub run: u32, pub first: u64, pub last: u64, pub vs: String, pub ps: String, pub files: Vec<String>, pub dir: String }

pub fn load_runs(frame_dir: &Path, dirs: &[&str]) -> Result<Vec<Run>, String> {
    let mut out = Vec::new();
    for d in dirs {
        let p = frame_dir.join(d).join("tex.json");
        if !p.exists() { continue; }
        let v: Value = serde_json::from_str(&std::fs::read_to_string(&p).map_err(|e| format!("{}: {e}", p.display()))?).map_err(|e| format!("{}: {e}", p.display()))?;
        for s in v["saved"].as_array().ok_or("tex.json: saved")? {
            out.push(Run {
                run: s["run"].as_u64().unwrap_or(0) as u32,
                first: s["first"].as_u64().unwrap_or(0),
                last: s["last"].as_u64().unwrap_or(0),
                vs: s["shaders"]["Vertex"].as_str().unwrap_or("").to_string(),
                ps: s["shaders"]["Pixel"].as_str().unwrap_or("").to_string(),
                files: s["files"].as_array().map(|a| a.iter().filter_map(|f| f["file"].as_str().map(|s| s.to_string())).collect()).unwrap_or_default(),
                dir: d.to_string(),
            });
        }
    }
    out.sort_by_key(|r| r.first);
    Ok(out)
}

fn file_with(r: &Run, tag: &str) -> Option<String> { r.files.iter().find(|f| f.contains(tag)).map(|f| format!("{}/{f}", r.dir)) }

pub fn chain_cli(a: &[String]) -> Result<(), String> {
    // lmtool swd6-chain FRAME_DIR [--set-ps 12428] [--fitted-vs 12431] [--target 12414] [--color 12417] [--report-eid E]... [--from-game]
    //   every exported SET run in eid order, replayed over the captured layer before it: CHAINED (our target carried run to run from
    //   the world SET's captured target) and, with --from-game, each run started from the captured target before it (isolating the
    //   run's own error). The palm (or any --report-eid) per run.
    let f = |k: &str| a.iter().position(|x| x == k).and_then(|i| a.get(i + 1)).cloned();
    let has = |k: &str| a.iter().any(|x| x == k);
    let frame_dir = std::path::PathBuf::from(a.get(1).ok_or("FRAME_DIR")?);
    let set_ps = f("--set-ps").unwrap_or_else(|| "12428".into());
    let fitted_vs = f("--fitted-vs").unwrap_or_else(|| "12431".into());
    let tgt_id = f("--target").unwrap_or_else(|| "12414".into());
    let col_id = f("--color").unwrap_or_else(|| "12417".into());
    let from_game = has("--from-game");
    let cmp = match f("--cmp").as_deref() { Some("float") => crate::lmaccum::DepthCompare::Float, _ => crate::lmaccum::DepthCompare::Unorm16Round };
    let report: Vec<u64> = a.iter().enumerate().filter(|(_, x)| *x == "--report-eid").filter_map(|(i, _)| a.get(i + 1).and_then(|s| s.parse().ok())).collect();
    let runs = load_runs(&frame_dir, &["tex", "tail"])?;
    let draws_json: Vec<Value> = serde_json::from_str(&std::fs::read_to_string(frame_dir.join("draws.json")).map_err(|e| format!("draws.json: {e}"))?).map_err(|e| format!("draws.json: {e}"))?;
    let set_runs: Vec<&Run> = runs.iter().filter(|r| r.ps == set_ps && file_with(r, &format!("_rt0_{tgt_id}")).is_some()).collect();
    println!("swd6-chain: {} exported runs, {} SET runs with a {tgt_id} target", runs.len(), set_runs.len());
    // the report draws' meshes and the per-eid mesh templates: a SET run's draws are the same 40 objects every run (same idx/inst per
    // position), so the LM scene is built once from the FIRST fitted run's eids and re-used with each run's own cbuffers
    let first_fitted = set_runs.iter().find(|r| draws_json.iter().any(|d| d["eid"].as_u64().map(|e| e >= r.first && e <= r.last).unwrap_or(false) && d.pointer("/Vertex/shader").and_then(|v| v.as_str()) == Some(fitted_vs.as_str()))).ok_or("no fitted SET run")?;
    let template_eids: Vec<u64> = draws_json.iter().filter(|d| d["eid"].as_u64().map(|e| e >= first_fitted.first && e <= first_fitted.last).unwrap_or(false) && d.pointer("/Pixel/shader").and_then(|v| v.as_str()) == Some(set_ps.as_str())).filter_map(|d| d["eid"].as_u64()).collect();
    let t0 = std::time::Instant::now();
    let (sc, template_draws, _) = scene_and_draws(&frame_dir, &template_eids, &set_ps, &fitted_vs)?;
    println!("  LM scene from run {} (eids {}..{}): {} meshes, {} instances ({:.1}s)", first_fitted.run, first_fitted.first, first_fitted.last, sc.meshes.len(), sc.instances.len(), t0.elapsed().as_secs_f32());
    let mut chained: Option<DirTarget> = None;
    let mut prev_target_file: Option<String> = None;
    for (ri, r) in set_runs.iter().enumerate() {
        let after_file = file_with(r, &format!("_rt0_{tgt_id}")).unwrap();
        let present = |fl: &str| frame_dir.join(fl).exists() || frame_dir.join(format!("{fl}.gz")).exists();
        if !present(&after_file) { println!("  run {:2} (eids {:5}..{:5}): {} NOT IN THE STORE — skipped, the chain re-seeds from the next captured target", r.run, r.first, r.last, after_file); chained = None; prev_target_file = None; continue; }
        // the SET draws of this run, in eid order, with the template's meshes matched by position (the k-th SET draw of every run is
        // the same object) — verified by idx/inst
        let run_draws: Vec<&Value> = draws_json.iter().filter(|d| d["eid"].as_u64().map(|e| e >= r.first && e <= r.last).unwrap_or(false) && d.pointer("/Pixel/shader").and_then(|v| v.as_str()) == Some(set_ps.as_str())).collect();
        if run_draws.len() != template_draws.len() { println!("  run {} (eids {}..{}): {} SET draws, the template has {} — skipped", r.run, r.first, r.last, run_draws.len(), template_draws.len()); continue; }
        let mut draws: Vec<SetDraw> = Vec::with_capacity(run_draws.len());
        let mut mismatch = 0usize;
        for (k, d) in run_draws.iter().enumerate() {
            let t = &template_draws[k];
            let idx = d["idx"].as_u64().unwrap_or(0) as usize;
            let inst = d["inst"].as_u64().unwrap_or(1) as usize;
            if idx != sc.meshes[t.mesh].indices.len() || inst != t.instance_count { mismatch += 1; }
            let vs = d.pointer("/Vertex/shader").and_then(|v| v.as_str()).unwrap_or("");
            let vcb = &d["Vertex"]["cbuffers"]["ShaderV"]["g_CBufferV"];
            let pcb = &d["Pixel"]["cbuffers"]["ShaderP"]["g_CBufferP"];
            draws.push(SetDraw {
                eid: d["eid"].as_u64().unwrap_or(0),
                mesh: t.mesh,
                instance_first: t.instance_first,
                instance_count: t.instance_count,
                raster: LmRasterCb { scale_ss: jv2(&vcb["LM01_Scale_RasterSS"]), trans_ss: jv2(&vcb["LM01_Trans_RasterSS"]) },
                cb: SetCb { world_pw01_shadow: jm4(&pcb["WorldPw01Shadow"]), peel_dir: jv3(&pcb["PeelDirInW"]) },
                world_box: if vs == fitted_vs { Some([jv2(&vcb["WorldBoxMinXZ"]), jv2(&vcb["WorldBoxMaxXZ"])]) } else { None },
            });
        }
        if mismatch > 0 { println!("  run {}: {mismatch} draws differ from the template in idx/inst — skipped", r.run); continue; }
        // the layer: the depth SRV the run's first draw samples names the depth file; the colour = the last colour run before it
        let depth_id = run_draws[0].pointer("/Pixel/srvs/1/tex/id").and_then(|v| v.as_str()).unwrap_or("").to_string();
        let layer_run = runs.iter().filter(|l| l.last < r.first && file_with(l, &format!("_rt0_{col_id}")).is_some()).last();
        let Some(lr) = layer_run else { println!("  run {:2} (eids {:5}..{:5}): no colour layer run before it — the captured target seeds the chain", r.run, r.first, r.last); chained = Some(target_from_buf(&crate::passdiff::load_file(&frame_dir, &after_file, "", 0, 0, 0)?)); prev_target_file = Some(after_file); continue };
        let color_file = file_with(lr, &format!("_rt0_{col_id}")).unwrap();
        let depth_file = match file_with(lr, &format!("_ds_{depth_id}")) {
            Some(fl) => fl,
            None => match runs.iter().filter(|l| l.last < r.first && file_with(l, &format!("_ds_{depth_id}")).is_some()).last().and_then(|l| file_with(l, &format!("_ds_{depth_id}"))) { Some(fl) => fl, None => { println!("  run {}: no depth {depth_id} dump before eid {} — skipped", r.run, r.first); continue } },
        };
        if !present(&color_file) || !present(&depth_file) { println!("  run {:2} (eids {:5}..{:5}): layer {} / {} NOT IN THE STORE — skipped; the captured target re-seeds the chain", r.run, r.first, r.last, color_file, depth_file); chained = Some(target_from_buf(&crate::passdiff::load_file(&frame_dir, &after_file, "", 0, 0, 0)?)); prev_target_file = Some(after_file); continue; }
        let color = crate::passdiff::load_file(&frame_dir, &color_file, "", 0, 0, 0).map_err(|e| format!("{color_file}: {e}"))?;
        let depth = crate::passdiff::load_file(&frame_dir, &depth_file, "R16_UNORM", 0, 0, 0).map_err(|e| format!("{depth_file}: {e}"))?;
        let after = crate::passdiff::load_file(&frame_dir, &after_file, "", 0, 0, 0).map_err(|e| format!("{after_file}: {e}"))?;
        // the starting target: the captured previous SET target (--from-game, or the first run), else our chained one
        let before_file = prev_target_file.clone();
        let mut tgt = match (&chained, from_game, &before_file) {
            (Some(c), false, _) => c.clone(),
            (_, _, Some(bf)) => target_from_buf(&crate::passdiff::load_file(&frame_dir, bf, "", 0, 0, 0).map_err(|e| format!("{bf}: {e}"))?),
            (_, _, None) => DirTarget::cleared(after.w, after.h),
        };
        let layer = LayerTargets { color: &color, depth: &depth };
        let t1 = std::time::Instant::now();
        crate::lmaccum::run_set_block_probe(&sc.meshes, &sc.instances, &sc.table, &draws, &layer, cmp, &mut tgt, None);
        let c = crate::lmaccum::compare_dir(&tgt, &after);
        let peel = if draws[0].world_box.is_none() { "world".to_string() } else { format!("box {:?}", draws[0].world_box.unwrap()) };
        println!("  run {:2} (eids {:5}..{:5}, {}; layer {} + {}; from {}) {:.1}s: {}", r.run, r.first, r.last, peel, color_file.rsplit('/').next().unwrap_or(""), depth_file.rsplit('/').next().unwrap_or(""), if ri == 0 || from_game { before_file.as_deref().map(|s| s.rsplit('/').next().unwrap_or("")).unwrap_or("cleared").to_string() } else { "OURS (chained)".into() }, t1.elapsed().as_secs_f32(), crate::lmaccum::fmt_dircmp(&c));
        for eid in &report {
            // the report eid names the object by its position in the template run
            let Some(pos) = template_eids.iter().position(|e| e == eid) else { continue };
            let k = template_draws[pos].mesh;
            let rects = instance_rects(&sc, k, after.w, after.h);
            let (mut n, mut ex, mut q, mut w, mut oo, mut go) = (0, 0, 0, 0, 0, 0);
            let mut worst_all = Vec::new();
            for rc in &rects { let (a1, b1, c1, d1, e1, f1, worst) = compare_rect(&tgt, &after, *rc); n += a1; ex += b1; q += c1; w += d1; oo += e1; go += f1; worst_all.extend(worst); }
            println!("      object of eid {eid} ({} tris × {} instances): {n} non-zero texels — exact {ex}, ≤1 quantum {q}, worse {w}, ours-only {oo}, game-only {go}", sc.meshes[k].indices.len() / 3, rects.len());
            for (x, y, o, g) in worst_all.iter().take(3) { println!("        ({x}, {y}): ours ({:.4}, {:.4}, {:.4}) game ({:.4}, {:.4}, {:.4})", o[0], o[1], o[2], g[0], g[1], g[2]); }
        }
        chained = Some(tgt);
        prev_target_file = Some(after_file);
    }
    Ok(())
}

/// THE H-BASIS PASS of the frame (VS 12434 / PS 12438 = pwc-day's 17118 / 17122, identical disassembly): the 40 accumulate draws over the
/// direction's FINAL captured ilightdir (the last B SET's target), the four MRTs cleared, compared f16 for f16 with the captured MRTs
/// at the frame's end (hbasis/ run039). `--ilightdir FILE` picks another target (ours from the chain, when written).
pub fn hbasis_cli(a: &[String]) -> Result<(), String> {
    // lmtool swd6-hbasis FRAME_DIR --mesh-eids 1970..2087 --hb-eids 12009..12126 --ilightdir tail/run038_e011975_rt0_12414.dds
    //   --mrts hbasis/run039_e012126_rt0_9337.dds,hbasis/…rt1_12396.dds,…rt2_12399.dds,…rt3_12402.dds [--report-eid 12069] [--blend …]
    let f = |k: &str| a.iter().position(|x| x == k).and_then(|i| a.get(i + 1)).cloned();
    let frame_dir = std::path::PathBuf::from(a.get(1).ok_or("FRAME_DIR")?);
    let set_ps = f("--set-ps").unwrap_or_else(|| "12428".into());
    let fitted_vs = f("--fitted-vs").unwrap_or_else(|| "12431".into());
    let hb_vs = f("--hb-vs").unwrap_or_else(|| "12434".into());
    let range = |k: &str| -> Result<Vec<u64>, String> { let s = f(k).ok_or_else(|| format!("{k} A..B"))?; let (lo, hi) = s.split_once("..").ok_or_else(|| format!("{k} A..B"))?; Ok((lo.trim().parse::<u64>().map_err(|e| e.to_string())?..=hi.trim().parse::<u64>().map_err(|e| e.to_string())?).collect()) };
    let draws_json: Vec<Value> = serde_json::from_str(&std::fs::read_to_string(frame_dir.join("draws.json")).map_err(|e| format!("draws.json: {e}"))?).map_err(|e| format!("draws.json: {e}"))?;
    let is = |d: &Value, key: &str, id: &str| d.pointer(key).and_then(|v| v.as_str()) == Some(id);
    let mesh_eids: Vec<u64> = range("--mesh-eids")?.into_iter().filter(|e| draws_json.iter().any(|d| d["eid"].as_u64() == Some(*e) && is(d, "/Pixel/shader", &set_ps))).collect();
    let hb_eids: Vec<u64> = range("--hb-eids")?.into_iter().filter(|e| draws_json.iter().any(|d| d["eid"].as_u64() == Some(*e) && is(d, "/Vertex/shader", &hb_vs))).collect();
    let t0 = std::time::Instant::now();
    let (sc, tdraws, _) = scene_and_draws(&frame_dir, &mesh_eids, &set_ps, &fitted_vs)?;
    if hb_eids.len() != tdraws.len() { return Err(format!("{} H-basis draws vs {} SET draws in the mesh run", hb_eids.len(), tdraws.len())); }
    let mut draws: Vec<crate::lmaccum::HbDraw> = Vec::new();
    for (k, eid) in hb_eids.iter().enumerate() {
        let d = draws_json.iter().find(|d| d["eid"].as_u64() == Some(*eid)).unwrap();
        let t = &tdraws[k];
        if d["idx"].as_u64().unwrap_or(0) as usize != sc.meshes[t.mesh].indices.len() || d["inst"].as_u64().unwrap_or(1) as usize != t.instance_count { return Err(format!("eid {eid}: idx/inst differ from the mesh run's draw {}", t.eid)); }
        let vcb = &d["Vertex"]["cbuffers"]["ShaderV"]["g_CBufferV"];
        let pcb = &d["Pixel"]["cbuffers"]["ShaderP"]["g_CBufferP"];
        draws.push(crate::lmaccum::HbDraw { eid: *eid, mesh: t.mesh, instance_first: t.instance_first, instance_count: t.instance_count, raster: LmRasterCb { scale_ss: jv2(&vcb["LM01_Scale_RasterSS"]), trans_ss: jv2(&vcb["LM01_Trans_RasterSS"]) }, cb: crate::lmaccum::HbCb { peel_dir: jv3(&pcb["PeelDirInW"]), inv_dir_count: jf(&pcb["InvDirCount"]) } });
    }
    println!("swd6-hbasis: {} H-basis draws (eids {}..{}) over {} meshes ({:.1}s); D {:?}, InvDirCount {}", draws.len(), hb_eids[0], hb_eids[hb_eids.len() - 1], sc.meshes.len(), t0.elapsed().as_secs_f32(), draws[0].cb.peel_dir, draws[0].cb.inv_dir_count);
    let il_file = f("--ilightdir").ok_or("--ilightdir FILE")?;
    let il = target_from_buf(&crate::passdiff::load_file(&frame_dir, &il_file, "", 0, 0, 0).map_err(|e| format!("{il_file}: {e}"))?);
    let mrt_files: Vec<String> = f("--mrts").ok_or("--mrts F0,F1,F2,F3")?.split(',').map(|s| s.trim().to_string()).collect();
    if mrt_files.len() != 4 { return Err("--mrts needs four files".into()); }
    let game: Vec<Buf> = mrt_files.iter().map(|fl| crate::passdiff::load_file(&frame_dir, fl, "", 0, 0, 0).map_err(|e| format!("{fl}: {e}"))).collect::<Result<_, _>>()?;
    let blend = match f("--blend").as_deref() { Some("round-src") => crate::sunpass::BlendModel::RoundSrcAndSum, Some("trunc") => crate::sunpass::BlendModel::TruncSum, Some("trunc-src") => crate::sunpass::BlendModel::TruncSrcAndSum, Some("round") => crate::sunpass::BlendModel::RoundSum, _ => crate::sunpass::BlendModel::TruncSrcRoundSum };
    let mut tgt = crate::lmaccum::HbTargets::cleared(il.w, il.h);
    let mut owner: Vec<u8> = vec![0; (il.w * il.h) as usize];
    let t1 = std::time::Instant::now();
    crate::lmaccum::run_hbasis_probe(&sc.meshes, &sc.instances, &sc.table, &draws, &il, &mut tgt, blend, Some(&mut owner), None);
    println!("  replayed in {:.1}s", t1.elapsed().as_secs_f32());
    for m in 0..4 {
        for ch in 0..4 {
            let (n, exact, ulp1, worse, maxd, worst) = crate::lmaccum::compare_mrt(&tgt.mrt[m], &game[m], ch);
            let pct = |v: usize| if n > 0 { 100.0 * v as f64 / n as f64 } else { 0.0 };
            println!("  C{m}.{}: {n:>8} values  exact {exact:>8} ({:6.2} %)  1 ulp {ulp1:>7} ({:5.2} %)  worse {worse:>6} ({:5.3} %)  max |Δ| {maxd:.6} at ({},{}) game {:.6} ours {:.6}", ["r", "g", "b", "a"][ch], pct(exact), pct(ulp1), pct(worse), worst.0, worst.1, worst.2, worst.3);
        }
    }
    let (mut ours_only, mut game_only) = (0usize, 0usize);
    for y in 0..il.h { for x in 0..il.w { let o = tgt.mrt[0][(y * il.w + x) as usize][3]; let g = game[0].get(x, y, 3); if o > 0.0 && g == 0.0 { ours_only += 1; } if g > 0.0 && o == 0.0 { game_only += 1; } } }
    println!("  coverage (C0 alpha > 0): ours-only {ours_only}, game-only {game_only}");
    // the reported objects: per instance rect, the four MRTs' rgb+alpha exact / 1-ulp / worse counts, and the alpha (fragment count) histogram vs the game's
    let report: Vec<u64> = a.iter().enumerate().filter(|(_, x)| *x == "--report-eid").filter_map(|(i, _)| a.get(i + 1).and_then(|s| s.parse().ok())).collect();
    for eid in report {
        let Some(pos) = hb_eids.iter().position(|e| *e == eid) else { println!("  --report-eid {eid}: not an H-basis draw"); continue };
        let k = draws[pos].mesh;
        let rects = instance_rects(&sc, k, il.w, il.h);
        println!("  eid {eid}: mesh {k} ({} tris) × {} instances", sc.meshes[k].indices.len() / 3, rects.len());
        let n_dirs = (1.0 / draws[0].cb.inv_dir_count).round();
        let (mut n, mut ex, mut u1, mut w) = (0usize, 0usize, 0usize, 0usize);
        let mut alpha_ex = 0usize; let mut alpha_n = 0usize;
        let mut hist = std::collections::BTreeMap::<(u32, u32), usize>::new();
        let mut worst: Vec<(u32, u32, usize, f32, f32)> = Vec::new();
        for r in &rects { for y in r[1]..r[3] { for x in r[0]..r[2] {
            let i = (y * il.w + x) as usize;
            let (oa, ga) = (tgt.mrt[0][i][3], game[0].get(x, y, 3));
            if oa == 0.0 && ga == 0.0 { continue; }
            alpha_n += 1; if oa == ga { alpha_ex += 1; }
            *hist.entry(((oa * n_dirs).round() as u32, (ga * n_dirs).round() as u32)).or_default() += 1;
            for m in 0..4 { for ch in 0..3 {
                let (o, g) = (tgt.mrt[m][i][ch], game[m].get(x, y, ch as u32));
                n += 1;
                if o == g { ex += 1; } else { let ulp = crate::gpufmt::f16_ulp(g.abs().max(o.abs())); if (o - g).abs() <= ulp * 1.001 { u1 += 1; } else { w += 1; if worst.len() < 6 { worst.push((x, y, m, g, o)); } } }
            } }
        } } }
        println!("    rgb values over the rects: {n} — exact {ex} ({:.2} %), 1 ulp {u1}, worse {w}; alpha (fragment count) exact on {alpha_ex} of {alpha_n} texels", 100.0 * ex as f64 / n.max(1) as f64);
        let mut hv: Vec<_> = hist.into_iter().collect(); hv.sort_by_key(|(_, c)| std::cmp::Reverse(*c));
        println!("    (ours fragments, game fragments) per texel, top: {:?}", hv.iter().take(12).map(|((o, g), c)| format!("({o},{g})×{c}")).collect::<Vec<_>>());
        for (x, y, m, g, o) in worst { println!("      worst ({x}, {y}) C{m}: game {g:.6} ours {o:.6} (alpha ours {:.5} game {:.5})", tgt.mrt[0][(y * il.w + x) as usize][3], game[0].get(x, y, 3)); }
    }
    Ok(())
}
