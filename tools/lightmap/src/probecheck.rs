//! `lmtool probe-check ROOT` — ROW 10's verification: the probe passes transcribed in `probepass`
//! run on the CAPTURED inputs (the world peel's colour/depth layers, the safety-offset volume, the
//! draws' cbuffers) and compared f16 for f16 with the captured 3D targets after every draw.

use crate::passdiff::{load_entry, read_entry_bytes, read_manifest, Buf};
use crate::passdump::{Entry, Manifest};
use crate::probepass::*;

pub struct CheckOpts {
    pub frame: u32,
    pub opts: ProbeOpts,
    /// Print every layer's per-probe changes.
    pub verbose: bool,
}

fn v3(v: &serde_json::Value) -> Option<[f32; 3]> {
    Some([v.get(0)?.as_f64()? as f32, v.get(1)?.as_f64()? as f32, v.get(2)?.as_f64()? as f32])
}

/// The `ProbeToShadow` rows (4 × 3) of a draw's `g_CBufferP`.
pub fn probe_to_shadow_rows(cb: &serde_json::Value) -> Option<[[f32; 3]; 4]> {
    let m = cb.get("ProbeToShadow")?;
    let mut rows = [[0f32; 3]; 4];
    for i in 0..4 {
        rows[i] = v3(m.get(i)?)?;
    }
    Some(rows)
}

/// A probe draw from its action record in the draws log.
pub fn probe_draw_from_action(a: &serde_json::Value, scissor: Option<[u32; 4]>) -> Option<ProbeDraw> {
    let eid = a.get("eid")?.as_u64()?;
    let cb = a.pointer("/Pixel/cbuffers/ShaderP/g_CBufferP")?;
    let rows = probe_to_shadow_rows(cb)?;
    let out_scale = cb.get("OutScale")?.as_f64()? as f32;
    let slice_start = a.pointer("/Geometry/cbuffers/ShaderG/g_CBufferG/iSliceStart").and_then(|v| v.as_u64()).unwrap_or(0) as u32;
    let idx = a.get("idx").and_then(|v| v.as_u64()).unwrap_or(0) as u32;
    Some(ProbeDraw { eid, regs: ProbeDraw::regs_from_rows(&rows), out_scale, slice_start, slice_count: idx / 3, scissor })
}

/// The scissor rect of a draw from the capture's `probe_draw_state` (or the sidecar json): the first
/// enabled slot's (x, y, w, h); None when scissoring is off.
pub fn scissor_for(state: Option<&serde_json::Value>, eid: u64) -> Option<Option<[u32; 4]>> {
    let arr = state?.as_array()?;
    let rec = arr.iter().find(|r| r.get("eid").and_then(|v| v.as_u64()) == Some(eid))?;
    if rec.get("scissorEnable").and_then(|v| v.as_bool()) == Some(false) {
        return Some(None);
    }
    let s = rec.get("scissors")?.as_array()?.first()?;
    if s.get("enabled").and_then(|v| v.as_bool()) == Some(false) {
        return Some(None);
    }
    let g = |k: &str| s.get(k).and_then(|v| v.as_u64()).map(|v| v as u32);
    Some(Some([g("x")?, g("y")?, g("w")?, g("h")?]))
}

fn load_volume(root: &std::path::Path, e: &Entry) -> Result<Volume3, String> {
    let bytes = read_entry_bytes(root, &e.file)?;
    load_dds_volume(&bytes, VolFmt::from_name(&e.format), e.depth.unwrap_or(32)).map_err(|err| format!("{}: {err}", e.file))
}

fn entries<'a>(m: &'a Manifest, pass: &str, frame: u32) -> Vec<&'a Entry> {
    let mut v: Vec<&Entry> = m.passes.iter().filter(|e| e.pass == pass && e.frame == Some(frame)).collect();
    v.sort_by_key(|e| e.eid.or(e.eid_last).unwrap_or(0));
    v
}

/// The peel colour/depth entries of the world phase that were current at `eid` (the last layer
/// finished before it).
fn layer_before<'a>(m: &'a Manifest, frame: u32, eid: u64, pass: &str) -> Option<&'a Entry> {
    m.passes
        .iter()
        .filter(|e| e.pass == pass && e.frame == Some(frame) && e.phase.as_deref() == Some("world") && e.eid_last.map(|l| l < eid).unwrap_or(false))
        .max_by_key(|e| e.eid_last.unwrap_or(0))
}

pub fn run(root: &std::path::Path, co: &CheckOpts) -> Result<(), String> {
    let frame = co.frame;
    let txt = std::fs::read_to_string(root.join("MANIFEST.json")).map_err(|e| format!("MANIFEST.json: {e}"))?;
    let m = read_manifest(&txt)?;
    let draws_bytes = read_entry_bytes(root, &format!("logs/draws-frame{frame}.json"))?;
    let draws: serde_json::Value = serde_json::from_slice(&draws_bytes).map_err(|e| format!("draws json: {e}"))?;
    let acts = draws.as_array().ok_or("draws log is not an array")?;
    // the scissor/sampler record: the manifest's, else the sidecar next to the volumes
    let sidecar = root.join(format!("probe3d/frame{frame}/samplers-scissors.json"));
    let side_val: Option<serde_json::Value> = std::fs::read_to_string(&sidecar).ok().and_then(|t| serde_json::from_str(&t).ok());
    let state = m.probe_draw_state.as_ref().or(side_val.as_ref());

    // ── the safety offsets ──
    let off_e = entries(&m, "probe_safety_offset", frame);
    let offsets = match off_e.first() {
        Some(e) => Some(load_volume(root, e)?),
        None => None,
    };
    match &offsets {
        Some(o) => {
            let nz: Vec<(u32, u32, u32, [f32; 3])> = (0..o.d).flat_map(|z| (0..o.h).flat_map(move |y| (0..o.w).map(move |x| (x, y, z)))).filter_map(|(x, y, z)| { let v = [o.get(x, y, z, 0), o.get(x, y, z, 1), o.get(x, y, z, 2)]; if v.iter().any(|&c| c != 0.0) { Some((x, y, z, v)) } else { None } }).collect();
            println!("TMapProbeSafetyOffset {}×{}×{}: {} non-zero probes{}", o.w, o.h, o.d, nz.len(), if nz.len() <= 8 { format!(": {:?}", nz) } else { String::new() });
        }
        None => println!("no probe_safety_offset entry for frame {frame}: offsets taken as 0"),
    }

    // ── the world-phase probe draws (PS 17151 / 17565) ──
    let is_probe_ps = |a: &serde_json::Value, ids: &[&str]| a.pointer("/Pixel/shader").and_then(|v| v.as_str()).map(|s| ids.contains(&s)).unwrap_or(false);
    let probe_acts: Vec<&serde_json::Value> = acts.iter().filter(|a| is_probe_ps(a, &["17151", "17565"])).collect();
    let aux_acts: Vec<&serde_json::Value> = acts.iter().filter(|a| is_probe_ps(a, &["17154", "17568"])).collect();
    let fold_acts: Vec<&serde_json::Value> = acts.iter().filter(|a| is_probe_ps(a, &["1112"])).collect();
    println!("frame {frame}: {} probe draws (SetILightDir), {} sky-visibility draws, {} folds", probe_acts.len(), aux_acts.len(), fold_acts.len());
    let captured = entries(&m, "probe3d_world", frame);
    let mut target = Volume3::new(32, 16, 32, 4);
    let mut all_closed = true;
    let mut world_pw01: Option<[[f32; 4]; 4]> = None;
    // the world peel's pw01 of this direction (any world-phase peel entry of the frame)
    if let Some(e) = m.passes.iter().find(|e| e.frame == Some(frame) && e.phase.as_deref() == Some("world") && e.pw01.is_some()) {
        world_pw01 = e.pw01;
    }
    for (k, a) in probe_acts.iter().enumerate() {
        let eid = a["eid"].as_u64().unwrap_or(0);
        let sc = scissor_for(state, eid).unwrap_or(Some([22, 4, 7, 8]));
        let d = probe_draw_from_action(a, sc).ok_or_else(|| format!("eid {eid}: no ProbeToShadow cbuffer"))?;
        if k == 0 {
            let cb = a.pointer("/Pixel/cbuffers/ShaderP/g_CBufferP").unwrap();
            let rows = probe_to_shadow_rows(cb).unwrap();
            println!("ProbeToShadow rows {:?}", rows);
            if let Some(pw) = world_pw01 {
                if let Some((s, t)) = ProbeDraw::probe_to_world(&rows, &pw) {
                    println!("  = ProbeToWorld · WorldPw01Shadow(world) with ProbeToWorld: scale ({:.4}, {:.4}, {:.4}) cells → m, translation ({:.3}, {:.3}, {:.3}) = the block's pos", s[0], s[1], s[2], t[0], t[1], t[2]);
                }
            }
            println!("  slices {}..{} (iSliceStart {}, {} triangles), scissor {:?}, OutScale {}", d.slice_start, d.slice_start + d.slice_count, d.slice_start, d.slice_count, d.scissor, d.out_scale);
        }
        let ce = layer_before(&m, frame, eid, "peel_color").ok_or_else(|| format!("eid {eid}: no world peel_color before it"))?;
        let de = layer_before(&m, frame, eid, "peel_depth").ok_or_else(|| format!("eid {eid}: no world peel_depth before it"))?;
        let t0 = std::time::Instant::now();
        let color: Buf = load_entry(root, ce)?;
        let depth: Buf = load_entry(root, de)?;
        let before = target.clone();
        let written = probe_set_ilightdir(&mut target, &d, &color, &depth, offsets.as_ref(), co.opts);
        let changed = (0..target.d).flat_map(|z| (0..target.h).flat_map(move |y| (0..target.w).map(move |x| (x, y, z)))).filter(|&(x, y, z)| (0..4).any(|c| target.get(x, y, z, c) != before.get(x, y, z, c))).count();
        let cap = captured.iter().find(|e| e.eid == Some(eid));
        let line = match cap {
            Some(e) => {
                let theirs = load_volume(root, e)?;
                let r = compare_volumes(&target, &theirs, 4);
                if !r.closed() {
                    all_closed = false;
                }
                format!("{} — {}", if r.closed() { "CLOSED" } else { "OPEN" }, r.line())
            }
            None => "(not captured)".to_string(),
        };
        println!("layer {k} (eid {eid}; peel colour {} + depth {}): {written} probes pass the depth compare, {changed} change; {} non-zero after; {:.1} s\n  vs captured 17157: {line}", ce.file.rsplit('/').next().unwrap_or(""), de.file.rsplit('/').next().unwrap_or(""), target.count_nonzero(), t0.elapsed().as_secs_f32());
        if co.verbose && changed > 0 && changed < 40 {
            for z in 0..target.d { for y in 0..target.h { for x in 0..target.w { if (0..4).any(|c| target.get(x, y, z, c) != before.get(x, y, z, c)) { println!("    probe ({x},{y},{z}): {:?} → {:?}", [before.get(x, y, z, 0), before.get(x, y, z, 1), before.get(x, y, z, 2), before.get(x, y, z, 3)], [target.get(x, y, z, 0), target.get(x, y, z, 1), target.get(x, y, z, 2), target.get(x, y, z, 3)]); } } } }
        }
    }

    // ── the sky-visibility draw (PS 17154 / 17568): after layer 1's peel ──
    for a in &aux_acts {
        let eid = a["eid"].as_u64().unwrap_or(0);
        let sc = scissor_for(state, eid).unwrap_or(Some([22, 4, 7, 8]));
        let d = probe_draw_from_action(a, sc).ok_or_else(|| format!("eid {eid}: no ProbeToShadow cbuffer"))?;
        let de = layer_before(&m, frame, eid, "peel_depth").ok_or_else(|| format!("eid {eid}: no world peel_depth before it"))?;
        let depth: Buf = load_entry(root, de)?;
        // the target accumulates over the bake: the first compute frame starts from the clear
        let mut sky = Volume3::new(32, 16, 32, 1);
        let added = probe_add_sky_visibility(&mut sky, &d, &depth, offsets.as_ref(), co.opts);
        let cap = entries(&m, "probe3d_skyvis", frame).into_iter().find(|e| e.eid == Some(eid));
        let line = match cap {
            Some(e) => {
                let theirs = load_volume(root, e)?;
                let r = compare_volumes(&sky, &theirs, 1);
                if !r.closed() { all_closed = false; }
                format!("{} — {}", if r.closed() { "CLOSED" } else { "OPEN" }, r.line())
            }
            None => "(not captured)".to_string(),
        };
        println!("sky visibility (eid {eid}; depth {}; OutScale {} = 4·D.y/N): {added} probes see the sky\n  vs captured 17160: {line}", de.file.rsplit('/').next().unwrap_or(""), d.out_scale);
    }

    // ── the folds (PS 1112) from our final 17157 ──
    for (i, a) in fold_acts.iter().enumerate() {
        let eid = a["eid"].as_u64().unwrap_or(0);
        let cb = a.pointer("/Pixel/cbuffers/ShaderP/g_CBufferP/ScaleSrc").ok_or_else(|| format!("eid {eid}: no ScaleSrc"))?;
        let mut scale = [0f32; 4];
        for c in 0..4 { scale[c] = cb.get(c).and_then(|v| v.as_f64()).unwrap_or(0.0) as f32; }
        let slice_start = a.pointer("/Geometry/cbuffers/ShaderG/g_CBufferG/iSliceStart").and_then(|v| v.as_u64()).unwrap_or(0) as u32;
        let idx = a.get("idx").and_then(|v| v.as_u64()).unwrap_or(96) as u32;
        let mut fold = Volume3::new(32, 16, 32, 4);
        probe_fold(&mut fold, &target, scale, slice_start, idx / 3);
        let pass = format!("probe3d_fold{i}");
        let cap = entries(&m, &pass, frame).into_iter().find(|e| e.eid == Some(eid));
        let line = match cap {
            Some(e) => {
                let theirs = load_volume(root, e)?;
                let r = compare_volumes(&fold, &theirs, 4);
                if !r.closed() { all_closed = false; }
                format!("{} — {}", if r.closed() { "CLOSED" } else { "OPEN" }, r.line())
            }
            None => "(not captured)".to_string(),
        };
        println!("fold {i} (eid {eid}; ScaleSrc {:?}; slices {}..{}): vs captured: {line}", scale, slice_start, slice_start + idx / 3);
    }
    println!("{}", if all_closed { "ROW 10 GPU side: every captured probe target reproduced bit for bit" } else { "ROW 10 GPU side: differences remain (see above)" });
    Ok(())
}
