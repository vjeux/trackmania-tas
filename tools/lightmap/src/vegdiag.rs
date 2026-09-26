//! `lmtool veg-diag PASSCAP_ROOT --ours OURS_DUMP --hb-dump OURS_HB.bin [--frame 127448] [--env-frame 127448]
//! [--capture pwc2] [--manifest M] [--max 200] [--frags BAKE_STDERR.log] [--pixels-out PIXELS.txt] [--out TABLE.md]`
//! — THE VEGETATION TEXEL DIAGNOSTIC (perf engineer 5 for engineer E): the LM texels of the vegetation card where OUR
//! H-basis MRT-0 after direction 0 is 0 and the GAME's is > 0, each mapped to its peel pixel(s) through the LM
//! fragment list (the card's world position at the texel) and the peel frames' WorldPw01Shadow (PS 17112's own
//! projection), and at every such pixel: the GAME's captured layers (frame 127448, world and fitted phase — stored
//! depth, colour, and whether the texel's depth passes each), OUR dumped layers at the same pixel, and — from a
//! bake run with `LMTOOL_ABUF_DEBUG_LIST=PIXELS.txt` — the raw fragments our A-buffer saw there (triangle, alpha
//! verdict, front/back, depth, the layer depth q they would carry). Each texel is classified against the phase that
//! writes it last (the fitted phase where it applies): extra layer (ours resolves to a depth the game has no layer
//! at) / no fragment / alpha-failed / deeper than rendered / back-face black / layer mismatch / depth quantum mismatch.
//!
//! Three steps: (1) `veg-diag … --pixels-out PIXELS.txt` writes the peel pixels to watch; (2) the harness bake with
//! `LMTOOL_ABUF_DEBUG_LIST=PIXELS.txt 2> LOG`; (3) `veg-diag … --frags LOG --out TABLE.md`.

use crate::lmaccum::{self, CapEntry, SetCb};
use crate::passdiff::Buf;
use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};

#[derive(Clone, Debug)]
struct DbgFrag {
    kind: String,
    ti: u32,
    inst: u32,
    mtri: u32,
    mask: u32,
    u: f32,
    v: f32,
    op: bool,
    z01: f32,
    q: u32,
    front: bool,
}

fn parse_frags(path: &Path) -> Result<HashMap<(u32, u32, u32), Vec<DbgFrag>>, String> {
    let txt = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let mut out: HashMap<(u32, u32, u32), Vec<DbgFrag>> = HashMap::new();
    for line in txt.lines() {
        let Some(rest) = line.strip_prefix("ABUFDBG ") else { continue };
        let mut kv: HashMap<&str, &str> = HashMap::new();
        for tok in rest.split_whitespace() {
            if let Some((k, v)) = tok.split_once('=') { kv.insert(k, v); }
        }
        let g = |k: &str| -> Option<&str> { kv.get(k).copied() };
        let (Some(peel), Some(x), Some(y)) = (g("peel"), g("x"), g("y")) else { continue };
        let f = DbgFrag {
            kind: g("kind").unwrap_or("?").to_string(),
            ti: g("ti").and_then(|s| s.parse().ok()).unwrap_or(0),
            inst: g("inst").and_then(|s| s.parse().ok()).unwrap_or(0),
            mtri: g("mtri").and_then(|s| s.parse().ok()).unwrap_or(0),
            mask: g("mask").and_then(|s| s.parse().ok()).unwrap_or(0),
            u: g("u").and_then(|s| s.parse().ok()).unwrap_or(f32::NAN),
            v: g("v").and_then(|s| s.parse().ok()).unwrap_or(f32::NAN),
            op: g("op") == Some("1"),
            z01: g("z01").and_then(|s| s.parse().ok()).unwrap_or(f32::NAN),
            q: g("q").and_then(|s| s.parse().ok()).unwrap_or(0),
            front: g("front") == Some("1"),
        };
        let key = (peel.parse().unwrap_or(0), x.parse().unwrap_or(0), y.parse().unwrap_or(0));
        out.entry(key).or_default().push(f);
    }
    Ok(out)
}

/// The H-basis dump of `LMTOOL_HB_DUMP`: (w, h, then 4 MRTs × w·h × 4 f32) → MRT 0 as rgba per pixel.
fn read_hb_dump(path: &Path) -> Result<(u32, u32, Vec<[f32; 4]>), String> {
    let b = std::fs::read(path).map_err(|e| format!("{}: {e}", path.display()))?;
    if b.len() < 8 { return Err("hb dump too short".into()); }
    let w = u32::from_le_bytes(b[0..4].try_into().unwrap());
    let h = u32::from_le_bytes(b[4..8].try_into().unwrap());
    let n = (w * h) as usize;
    if b.len() < 8 + n * 16 { return Err(format!("hb dump: {} bytes for {w}×{h}", b.len())); }
    let mut mrt0 = Vec::with_capacity(n);
    for i in 0..n {
        let o = 8 + i * 16;
        mrt0.push([f32::from_le_bytes(b[o..o + 4].try_into().unwrap()), f32::from_le_bytes(b[o + 4..o + 8].try_into().unwrap()), f32::from_le_bytes(b[o + 8..o + 12].try_into().unwrap()), f32::from_le_bytes(b[o + 12..o + 16].try_into().unwrap())]);
    }
    Ok((w, h, mrt0))
}

/// One phase (world / fitted) of the direction: its constants, its accumulate blocks in order with the game layer
/// each block read (colour + depth buffers, the layer index the manifest gives it).
struct Phase {
    name: &'static str,
    cb: SetCb,
    world_box: Option<[[f32; 2]; 2]>,
    layers: Vec<(u32, Buf, Buf)>, // (manifest layer index, colour, depth)
}

fn dot3(a: [f32; 3], b: [f32; 3]) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

pub fn run(args: &[String]) -> Result<(), String> {
    let f = |k: &str| args.iter().position(|x| x == k).and_then(|i| args.get(i + 1)).cloned();
    let root = PathBuf::from(args.get(1).ok_or("usage: lmtool veg-diag PASSCAP_ROOT --ours OURS_DUMP --hb-dump OURS_HB.bin [--frame N] [--max N] [--frags LOG] [--pixels-out FILE] [--out TABLE.md]")?);
    let ours_root = PathBuf::from(f("--ours").ok_or("--ours OURS_DUMP")?);
    let hb_path = PathBuf::from(f("--hb-dump").ok_or("--hb-dump OURS_HB.bin")?);
    let frame: u32 = f("--frame").map(|v| v.parse().unwrap()).unwrap_or(127448);
    let env_frame: u32 = f("--env-frame").map(|v| v.parse().unwrap()).unwrap_or(127448);
    let capture = f("--capture").unwrap_or_else(|| "pwc2".into());
    let max: usize = f("--max").map(|v| v.parse().unwrap()).unwrap_or(200);
    // --reverse: the other population — OUR MRT-0 > 0 where the GAME's is 0
    let reverse = args.iter().any(|x| x == "--reverse");
    let manifest = f("--manifest").map(PathBuf::from).unwrap_or_else(|| root.join("MANIFEST.json"));
    let frags: Option<HashMap<(u32, u32, u32), Vec<DbgFrag>>> = match f("--frags") { Some(p) => Some(parse_frags(Path::new(&p))?), None => None };
    let t0 = std::time::Instant::now();

    // the LM scene, the accumulate blocks of the frame and their phases
    let sc = lmaccum::load_lm_scene(&root, env_frame)?;
    let draws = lmaccum::load_draws(&root, frame)?;
    let blocks = lmaccum::set_blocks(&draws, &sc)?;
    let entries = lmaccum::load_capture_entries(&manifest)?;
    let layer_entries: Vec<&CapEntry> = entries.iter().filter(|e| (e.pass == "peel_color" || e.pass == "peel_depth") && e.frame == frame && e.capture == capture).collect();
    eprintln!("veg-diag: {} LM meshes, {} instances; {} accumulate blocks in frame {frame}; {} layer buffers", sc.meshes.len(), sc.instances.len(), blocks.len(), layer_entries.len());
    // the vegetation mesh: the one drawn with 5751 indices (pwc-day's card), else --veg-mesh
    let veg_mesh: usize = match f("--veg-mesh") { Some(m) => m.parse().unwrap(), None => sc.meshes.iter().position(|m| m.indices.len() == 5751).ok_or("no LM mesh with 5751 indices — pass --veg-mesh M")? };
    eprintln!("veg-diag: the vegetation is LM mesh {veg_mesh} ({} indices, instances {}..+{})", sc.meshes[veg_mesh].indices.len(), sc.inst_first[veg_mesh], sc.inst_count[veg_mesh]);
    // the phases: consecutive blocks with the same world_box / constants; each block's layer = the last colour +
    // depth snapshot before the block (ilightdir-check's pairing)
    let mut phases: Vec<Phase> = Vec::new();
    for b in &blocks {
        let d0 = &b.draws[0];
        let color = layer_entries.iter().filter(|e| e.pass == "peel_color" && e.eid_last < b.eid_first).max_by_key(|e| e.eid_last).copied();
        let depth = layer_entries.iter().filter(|e| e.pass == "peel_depth" && e.eid_last < b.eid_first).max_by_key(|e| e.eid_last).copied();
        let (Some(ce), Some(de)) = (color, depth) else { eprintln!("  block eids {}-{}: no captured layer before it — skipped", b.eid_first, b.eid_last); continue };
        let same = phases.last().map(|p| p.world_box == d0.world_box && p.cb.world_pw01_shadow == d0.cb.world_pw01_shadow).unwrap_or(false);
        if !same {
            phases.push(Phase { name: if d0.world_box.is_some() { "fitted" } else { "world" }, cb: d0.cb.clone(), world_box: d0.world_box, layers: Vec::new() });
        }
        let ph = phases.last_mut().unwrap();
        ph.layers.push((ce.layer.unwrap_or(u32::MAX), ce.load(&root)?, de.load(&root)?));
    }
    for p in &phases { eprintln!("  phase {}: {} blocks/layers (manifest layer indices {:?}), world box {:?}", p.name, p.layers.len(), p.layers.iter().map(|l| l.0).collect::<Vec<_>>(), p.world_box); }

    // the MRT-0 after the direction: the game's banked hbasis0 of the frame, ours from the dump
    let hb0: &CapEntry = entries.iter().filter(|e| e.pass == "hbasis0" && e.capture == capture && e.banked && e.frame == frame).min_by_key(|e| e.eid_last).ok_or("no banked hbasis0 in the frame")?;
    let game_mrt0 = hb0.load(&root)?;
    // the game.s TMapILightDir after the direction.s last accumulate (the emulation.s self-check)
    let game_ild: Option<Buf> = entries.iter().filter(|e| (e.pass == "ilightdir_final" || e.pass == "ilightdir") && e.capture == capture && e.frame == frame && e.eid_last <= hb0.eid_first).max_by_key(|e| (e.eid_last, e.pass == "ilightdir_final")).map(|e| e.load(&root)).transpose()?;
    let mut emu_agree = 0usize;
    let mut emu_total = 0usize;
    let (hw, hh, ours_mrt0) = read_hb_dump(&hb_path)?;
    if (hw, hh) != (game_mrt0.w, game_mrt0.h) { return Err(format!("MRT sizes differ: ours {hw}×{hh}, game {}×{}", game_mrt0.w, game_mrt0.h)); }
    eprintln!("veg-diag: game MRT-0 = {} (eids {}-{}), ours = {} ({hw}×{hh})", hb0.file, hb0.eid_first, hb0.eid_last, hb_path.display());

    // the LM fragment list of raster offset 0 (direction 0): per texel the card's fragments
    let fl = lmaccum::build_frag_list(&sc, 0, hw, hh);
    eprintln!("veg-diag: LM fragment list: {} fragments", fl.frags.len());
    let mut cands: Vec<(u32, f32, f32)> = Vec::new(); // (pixel, game max rgb, ours max rgb)
    let (mut n_veg, mut n_both_zero, mut n_ours_only) = (0usize, 0usize, 0usize);
    for p in 0..(hw * hh) as usize {
        let fr = &fl.frags[fl.start[p] as usize..fl.start[p + 1] as usize];
        if !fr.iter().any(|f| fl.pairs[f.pair as usize].0 as usize == veg_mesh) { continue; }
        n_veg += 1;
        let (x, y) = (p as u32 % hw, p as u32 / hw);
        let g = game_mrt0.get(x, y, 0).max(game_mrt0.get(x, y, 1)).max(game_mrt0.get(x, y, 2));
        let o = ours_mrt0[p][0].max(ours_mrt0[p][1]).max(ours_mrt0[p][2]);
        if o == 0.0 && g > 0.0 { if !reverse { cands.push((p as u32, g, o)); } else { n_ours_only += 1; } } else if o == 0.0 && g == 0.0 { n_both_zero += 1; } else if o > 0.0 && g == 0.0 { if reverse { cands.push((p as u32, o, g)); } else { n_ours_only += 1; } }
    }
    cands.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    let n_cands = cands.len();
    eprintln!("veg-diag: {n_veg} vegetation texels; ours 0 & game > 0: {n_cands} (both 0: {n_both_zero}; ours > 0 & game 0: {n_ours_only}); max game {:.4} at ({}, {})", cands.first().map(|c| c.1).unwrap_or(0.0), cands.first().map(|c| c.0 % hw).unwrap_or(0), cands.first().map(|c| c.0 / hw).unwrap_or(0));
    let cands: Vec<(u32, f32, f32)> = cands.into_iter().take(max).collect();

    // OUR dumped layers of direction 0 per peel: the sky layer, then (layer k, colour, depth) in order
    let ours_m = crate::passdiff::read_manifest(&std::fs::read_to_string(ours_root.join("MANIFEST.json")).map_err(|e| e.to_string())?)?;
    let mut ours_layers: BTreeMap<u32, Vec<(String, Buf, Buf)>> = BTreeMap::new();
    for pi in 0..phases.len() as u32 {
        let mut v: Vec<(String, Buf, Buf)> = Vec::new();
        let sky_c = ours_m.passes.iter().find(|e| e.pass == "peel_sky" && e.sweep.unwrap_or(0) == 0 && e.direction == Some(0) && e.peel == Some(pi));
        let sky_d = ours_m.passes.iter().find(|e| e.pass == "peel_sky_depth" && e.sweep.unwrap_or(0) == 0 && e.direction == Some(0) && e.peel == Some(pi));
        if let (Some(c), Some(d)) = (sky_c, sky_d) { v.push(("env".into(), crate::passdiff::load_entry(&ours_root, c)?, crate::passdiff::load_entry(&ours_root, d)?)); }
        let mut ks: Vec<u32> = ours_m.passes.iter().filter(|e| e.pass == "peel_depth" && e.sweep.unwrap_or(0) == 0 && e.direction == Some(0) && e.peel == Some(pi)).filter_map(|e| e.layer).collect();
        ks.sort(); ks.dedup();
        for k in ks {
            let c = ours_m.passes.iter().find(|e| e.pass == "peel_color" && e.sweep.unwrap_or(0) == 0 && e.direction == Some(0) && e.peel == Some(pi) && e.layer == Some(k));
            let d = ours_m.passes.iter().find(|e| e.pass == "peel_depth" && e.sweep.unwrap_or(0) == 0 && e.direction == Some(0) && e.peel == Some(pi) && e.layer == Some(k));
            if let (Some(c), Some(d)) = (c, d) { v.push((format!("{k}"), crate::passdiff::load_entry(&ours_root, c)?, crate::passdiff::load_entry(&ours_root, d)?)); }
        }
        eprintln!("  ours peel {pi}: {} layers ({})", v.len(), v.iter().map(|l| l.0.clone()).collect::<Vec<_>>().join(", "));
        ours_layers.insert(pi, v);
    }

    // the table
    let mut md = String::new();
    md.push_str(&format!("# veg-diag — pwc-day frame {frame}, direction 0: vegetation texels with {}\n\n", if reverse { "OUR MRT-0 > 0 and the GAME's = 0 (--reverse)" } else { "OUR MRT-0 = 0 and the GAME's > 0" }));
    md.push_str(&format!("{n_veg} vegetation texels (LM mesh {veg_mesh}); ours 0 & game > 0: {n_cands}, the first {} by the game's value listed below; both 0: {n_both_zero}; ours > 0 & game 0: {n_ours_only}.\n\n", cands.len()));
    md.push_str("Depths are the UNORM16 quanta q = round(z01·65535) the accumulate compares (PS 17112: the texel's q ≥ the layer's stored q passes; the LAST passing layer in block order wins; the fitted phase's blocks run after the world phase's and overwrite where the texel is inside the fitted box). Game layer indices are the manifest's (0 = the environment block); ours are the dump's (env = peel_sky, then 0.. far → near). The class is judged at the phase that writes the texel last.\n\n");
    let mut classes: BTreeMap<String, usize> = BTreeMap::new();
    let mut summary: Vec<(u32, u32, f32, String, String, String)> = Vec::new();
    let mut extra_uv: Vec<(f32, f32)> = Vec::new();
    let mut pixels_out: Vec<(u32, u32, u32)> = Vec::new();
    for (ci, &(p, gval, _)) in cands.iter().enumerate() {
        let (x, y) = (p % hw, p / hw);
        let fr: Vec<&lmaccum::LmFrag> = fl.frags[fl.start[p as usize] as usize..fl.start[p as usize + 1] as usize].iter().filter(|f| fl.pairs[f.pair as usize].0 as usize == veg_mesh).collect();
        md.push_str(&format!("## {}. texel ({x}, {y}) — game MRT-0 max {gval:.4}, ours 0; {} card fragment(s) at the texel\n\n", ci + 1, fr.len()));
        let mut texel_class: Option<String> = None;
        let mut game_res: Option<(String, u32, [f32; 3])> = None;
        let mut ours_res: Option<(String, String, [f32; 3])> = None;
        // per LM fragment and phase: the pixel, the texel's q, the game's and our layers there (q, colour)
        struct At { pi: usize, fi: usize, tx: u32, ty: u32, rq: u32, game: Vec<(u32, u32, [f32; 3])>, ours: Vec<(String, u32, [f32; 3])> }
        let mut ats: Vec<At> = Vec::new();
        for (fi, lf) in fr.iter().enumerate() {
            md.push_str(&format!("- LM fragment {fi}: triangle {} of pair {}, bary ({:.3}, {:.3}, {:.3}), world ({:.3}, {:.3}, {:.3}), normal ({:.3}, {:.3}, {:.3})\n", lf.tri, lf.pair, lf.b[0], lf.b[1], lf.b[2], lf.pos[0], lf.pos[1], lf.pos[2], lf.nrm[0], lf.nrm[1], lf.nrm[2]));
            for (pi, ph) in phases.iter().enumerate() {
                if let Some(wb) = ph.world_box {
                    if !(lf.pos[0] >= wb[0][0] && lf.pos[2] >= wb[0][1] && lf.pos[0] <= wb[1][0] && lf.pos[2] <= wb[1][1]) {
                        md.push_str(&format!("  - {} phase: the texel is outside the fitted world box {:?} — clipped (VS 17115)\n", ph.name, wb));
                        continue;
                    }
                }
                if dot3(lf.nrm, ph.cb.peel_dir) < 0.0 { md.push_str(&format!("  - {} phase: n·PeelDir < 0 — discarded by PS 17112\n", ph.name)); continue; }
                let m = &ph.cb.world_pw01_shadow;
                let pp = lf.pos;
                let z = pp[0] * m[0][2] + pp[1] * m[1][2] + pp[2] * m[2][2] + m[3][2];
                let u = pp[0] * m[0][0] + pp[1] * m[1][0] + pp[2] * m[2][0] + m[3][0];
                let v = pp[0] * m[0][1] + pp[1] * m[1][1] + pp[2] * m[2][1] + m[3][1];
                let (lw, lh) = (ph.layers[0].2.w, ph.layers[0].2.h);
                let (tx, ty) = (lmaccum::point_texel(u, lw), lmaccum::point_texel(v, lh));
                let rq = (z.clamp(0.0, 1.0) * 65535.0).round() as u32;
                pixels_out.push((pi as u32, tx, ty));
                md.push_str(&format!("  - {} phase: (u, v) = ({u:.6}, {v:.6}) → peel pixel ({tx}, {ty}), texel depth z01 {z:.6} (q {rq})\n", ph.name));
                let mut at = At { pi, fi, tx, ty, rq, game: Vec::new(), ours: Vec::new() };
                let mut gl = String::new();
                for (k, col, dep) in &ph.layers {
                    let sq = (dep.get(tx, ty, 0) * 65535.0).round() as u32;
                    let c = [col.get(tx, ty, 0), col.get(tx, ty, 1), col.get(tx, ty, 2)];
                    gl.push_str(&format!(" L{k}: q {sq} ({:.4},{:.4},{:.4}){}", c[0], c[1], c[2], if rq >= sq { " PASS" } else { "" }));
                    at.game.push((*k, sq, c));
                }
                md.push_str(&format!("    - GAME layers:{gl}\n"));
                if let Some(ol) = ours_layers.get(&(pi as u32)) {
                    let mut s = String::new();
                    for (name, col, dep) in ol {
                        if tx >= dep.w || ty >= dep.h { continue; }
                        let sq = (dep.get(tx, ty, 0) * 65535.0).round() as u32;
                        let c = [col.get(tx, ty, 0), col.get(tx, ty, 1), col.get(tx, ty, 2)];
                        s.push_str(&format!(" L{name}: q {sq} ({:.4},{:.4},{:.4}){}", c[0], c[1], c[2], if rq >= sq { " PASS" } else { "" }));
                        at.ours.push((name.clone(), sq, c));
                    }
                    md.push_str(&format!("    - OURS layers:{s}\n"));
                }
                if let Some(fm) = &frags {
                    match fm.get(&(pi as u32, tx, ty)) {
                        Some(list) if !list.is_empty() => {
                            md.push_str(&format!("    - OUR A-BUFFER at ({tx}, {ty}), {} fragment(s):\n", list.len()));
                            for d in list {
                                md.push_str(&format!("      - {} ti {} inst {} model tri {}{} z01 {:.6} (q {}) {}{}\n", d.kind, d.ti, d.inst, d.mtri, if d.kind == "card" { format!(" mask {} uv ({:.4}, {:.4}) alpha {}", d.mask, d.u, d.v, if d.op { "PASS" } else { "FAIL" }) } else { String::new() }, d.z01, d.q, if d.front { "front" } else { "BACK" }, if d.q <= rq { "" } else { " (beyond the texel: never a layer for it)" }));
                            }
                        }
                        _ => md.push_str(&format!("    - OUR A-BUFFER at ({tx}, {ty}): no fragment at all\n")),
                    }
                }
                ats.push(at);
            }
        }
        // THE ACCUMULATE IN BLOCK ORDER (LmILightDir_Set: per phase, per layer block, every LM fragment of the texel in
        // draw order; the last fragment passing a block's depth test writes the target) — the game's final write and ours
        let mut game_final: Option<(usize, u32, usize, u32, [f32; 3])> = None; // (phase, layer, fragment, q, colour)
        let mut ours_final: Option<(usize, String, usize, u32, [f32; 3])> = None;
        for pi in 0..phases.len() {
            let n_game = phases[pi].layers.len();
            for li in 0..n_game {
                for at in ats.iter().filter(|a| a.pi == pi) {
                    if let Some((k, sq, c)) = at.game.get(li) { if at.rq >= *sq { game_final = Some((pi, *k, at.fi, *sq, *c)); } }
                }
            }
            let n_ours = ours_layers.get(&(pi as u32)).map(|v| v.len()).unwrap_or(0);
            for li in 0..n_ours {
                for at in ats.iter().filter(|a| a.pi == pi) {
                    if let Some((name, sq, c)) = at.ours.get(li) { if at.rq >= *sq { ours_final = Some((pi, name.clone(), at.fi, *sq, *c)); } }
                }
            }
        }
        if let Some(g) = &game_final { game_res = Some((phases[g.0].name.into(), g.1, g.4)); md.push_str(&format!("- GAME's final write: {} phase layer {} through LM fragment {} (q {}): ({:.4}, {:.4}, {:.4})\n", phases[g.0].name, g.1, g.2, g.3, g.4[0], g.4[1], g.4[2])); } else { md.push_str("- GAME's final write: none\n"); }
        if let Some(ild) = &game_ild {
            let cap = [ild.get(x, y, 0), ild.get(x, y, 1), ild.get(x, y, 2)];
            let emu = game_final.map(|g| g.4).unwrap_or([0.0; 3]);
            let agree = (0..3).all(|c| (cap[c] - emu[c]).abs() <= 1e-3 * cap[c].abs().max(1e-3));
            emu_total += 1; if agree { emu_agree += 1; }
            md.push_str(&format!("- the CAPTURED ilightdir at the texel after the direction: ({:.4}, {:.4}, {:.4}) — the emulation {}\n", cap[0], cap[1], cap[2], if agree { "agrees" } else { "DISAGREES (a residue of the transcription itself at this texel: LM raster coverage or the compare at the margin)" }));
            if !agree { texel_class = Some(format!("emulation ≠ capture: the game itself wrote ({:.4}, {:.4}, {:.4}) here, the transcribed accumulate over its layers gives ({:.4}, {:.4}, {:.4})", cap[0], cap[1], cap[2], emu[0], emu[1], emu[2])); }
        }
        if let Some(o) = &ours_final { ours_res = Some((phases[o.0].name.into(), o.1.clone(), o.4)); md.push_str(&format!("- OUR final write: {} phase layer {} through LM fragment {} (q {}): ({:.4}, {:.4}, {:.4})\n", phases[o.0].name, o.1, o.2, o.3, o.4[0], o.4[1], o.4[2])); } else { md.push_str("- OUR final write: none\n"); }
        // the class, from the two final writes and our A-buffer at the pixels concerned
        if let (Some(fm), true) = (&frags, texel_class.is_none()) {
            let near = |a: u32, b: u32| (a as i64 - b as i64).abs() <= 1;
            let at_of = |pi: usize, fi: usize| ats.iter().find(|a| a.pi == pi && a.fi == fi);
            let frag_desc = |list: &Vec<DbgFrag>, q: u32| -> String { list.iter().filter(|d| near(d.q, q)).map(|d| format!("{} tri {} uv ({:.4}, {:.4}) alpha {} {}", d.kind, d.mtri, d.u, d.v, if d.op { "PASS" } else { "FAIL" }, if d.front { "front" } else { "back" })).collect::<Vec<_>>().join("; ") };
            let empty: Vec<DbgFrag> = Vec::new();
            let cls = if reverse { match (&game_final, &ours_final) {
                // the game's final write is a layer we have no layer at: what did our A-buffer see there?
                (Some(g), _) if g.1 > 0 && at_of(g.0, g.2).map(|a| !a.ours.iter().any(|(_, sq, _)| near(*sq, g.3))).unwrap_or(false) => {
                    let a = at_of(g.0, g.2).unwrap();
                    let list = fm.get(&(g.0 as u32, a.tx, a.ty)).unwrap_or(&empty);
                    let at_q: Vec<&DbgFrag> = list.iter().filter(|d| near(d.q, g.3)).collect();
                    if list.is_empty() { format!("missing layer, no fragment: the game's {} layer {} (q {}) at pixel ({}, {}) — our A-buffer is empty there", phases[g.0].name, g.1, g.3, a.tx, a.ty) }
                    else if at_q.is_empty() { format!("missing layer, no fragment at its depth: the game's {} layer {} (q {}) at pixel ({}, {}) — nothing of ours within a quantum", phases[g.0].name, g.1, g.3, a.tx, a.ty) }
                    else if at_q.iter().all(|d| d.kind == "card" && !d.op) { format!("missing layer, alpha-failed in ours (the GPU passes): {}", frag_desc(list, g.3)) }
                    else { format!("missing layer, dropped by our derivation: fragments present at q {} at pixel ({}, {}) — {}", g.3, a.tx, a.ty, frag_desc(list, g.3)) }
                }
                (_, Some(o)) if o.1 != "env" => {
                    let a = at_of(o.0, o.2).unwrap();
                    let list = fm.get(&(o.0 as u32, a.tx, a.ty)).unwrap_or(&empty);
                    let game_has = a.game.iter().any(|(_, sq, _)| near(*sq, o.3));
                    if game_has { format!("colour mismatch: the same layer q {} at pixel ({}, {}), ours lit ({:.4}, {:.4}, {:.4}), the game's black — {}", o.3, a.tx, a.ty, o.4[0], o.4[1], o.4[2], frag_desc(list, o.3)) }
                    else { format!("extra layer of ours (lit): our final write is layer {} (q {}) at pixel ({}, {}), the game has no layer there — {}", o.1, o.3, a.tx, a.ty, frag_desc(list, o.3)) }
                }
                (Some(_), Some(_)) => "environment colour: both final writes are the environment layer, the colours differ".to_string(),
                (None, Some(_)) => "no write of the game's: every block's compare fails for the game where we write".to_string(),
                (_, None) => "no write of ours: our MRT value comes from elsewhere".to_string(),
            } } else { match (&ours_final, &game_final) {
                (Some(o), _) if o.1 != "env" && at_of(o.0, o.2).map(|a| !a.game.iter().any(|(_, sq, _)| near(*sq, o.3))).unwrap_or(false) => {
                    let a = at_of(o.0, o.2).unwrap();
                    let list = fm.get(&(o.0 as u32, a.tx, a.ty)).unwrap_or(&empty);
                    // WHY has the game no layer there? The GPU peels by BIASED depth: layer j+1 = the smallest biased depth
                    // among the fragments whose UNBIASED depth ≥ layer j's stored depth. Our fragment (biased q_o, unbiased
                    // u_o) is skipped by the game's layer list g_1 < g_2 < … either because the game layer just before the
                    // gap (g_{j−1} < q_o < g_j) already exceeds u_o — the fragment sat between its own unbiased and biased
                    // depths, so the GPU rejected it by the previous-layer compare while OUR derivation (ordered by the
                    // unbiased z) made it a layer first: THE ORDER — or because nothing rejected it and it still is not a
                    // layer: the GPU dropped it before the depth test (the alpha test, or coverage): ALPHA.
                    let u_o = list.iter().filter(|d| near(d.q, o.3)).map(|d| (d.z01.clamp(0.0, 1.0) * 65535.0).round() as u32).max().unwrap_or(o.3);
                    let mut game_sorted: Vec<u32> = a.game.iter().filter(|(k, _, _)| *k > 0).map(|(_, sq, _)| *sq).filter(|sq| *sq < 65535).collect();
                    game_sorted.sort();
                    let prev = game_sorted.iter().filter(|g| **g < o.3).max().copied();
                    let why = match prev {
                        Some(g) if g > u_o => format!("THE ORDER: the game's layer q {g} lies between this fragment's unbiased depth q {u_o} and its biased q {} — the GPU's previous-layer compare rejected it, our unbiased (z, tri) order made it a layer first", o.3),
                        _ => format!("ALPHA/COVERAGE: no game layer between its unbiased q {u_o} and biased q {} — the GPU dropped the fragment before the depth test", o.3),
                    };
                    format!("extra layer, {why}: our final write is {} layer {} (q {}) at pixel ({}, {}) where the game has no layer — {}", phases[o.0].name, o.1, o.3, a.tx, a.ty, frag_desc(list, o.3))
                }
                (_, Some(g)) if g.1 > 0 => {
                    let a = at_of(g.0, g.2).unwrap();
                    let list = fm.get(&(g.0 as u32, a.tx, a.ty)).unwrap_or(&empty);
                    let at_q: Vec<&DbgFrag> = list.iter().filter(|d| d.kind == "card" && near(d.q, g.3)).collect();
                    if list.is_empty() { format!("no fragment: our A-buffer is empty at pixel ({}, {}) of the {} phase where the game's layer {} (q {}) is", a.tx, a.ty, phases[g.0].name, g.1, g.3) }
                    else if at_q.is_empty() { format!("no fragment: nothing in our A-buffer at the game's layer depth q {} at pixel ({}, {}) of the {} phase", g.3, a.tx, a.ty, phases[g.0].name) }
                    else if at_q.iter().all(|d| !d.op) { format!("alpha-failed: {}", frag_desc(list, g.3)) }
                    else if !a.ours.iter().any(|(_, sq, _)| near(*sq, g.3)) { format!("deeper than rendered: the fragment at q {} is in no dumped layer of ours at pixel ({}, {}) — {}", g.3, a.tx, a.ty, frag_desc(list, g.3)) }
                    else if at_q.iter().filter(|d| d.op).all(|d| !d.front) { format!("back-face black: {}", frag_desc(list, g.3)) }
                    else { format!("layer mismatch: the same layer q {} at pixel ({}, {}), our colour differs — {}", g.3, a.tx, a.ty, frag_desc(list, g.3)) }
                }
                (Some(o), Some(_)) if o.1 != "env" => {
                    let a = at_of(o.0, o.2).unwrap();
                    let list = fm.get(&(o.0 as u32, a.tx, a.ty)).unwrap_or(&empty);
                    format!("depth quantum mismatch: our final write is layer {} (q {}) at pixel ({}, {}), a layer the game also has but which fails the game's compare — {}", o.1, o.3, a.tx, a.ty, frag_desc(list, o.3))
                }
                (Some(_), Some(_)) => "environment colour: both final writes are the environment layer, the colours differ".to_string(),
                (None, Some(_)) => "no write of ours: every block's compare fails for us where the game writes".to_string(),
                (_, None) => "no write of the game's: the game's MRT value comes from elsewhere".to_string(),
            } };
            texel_class = Some(cls);
        }
        let cls = texel_class.unwrap_or_else(|| if frags.is_some() { "unclassified".into() } else { "(run with --frags for the class)".into() });
        md.push_str(&format!("- **class: {cls}**\n\n"));
        let coarse = cls.split(':').next().unwrap_or(&cls).to_string();
        let coarse = if let Some(i) = coarse.find(", THE ORDER") { format!("{}, THE ORDER", &coarse[..i]) } else if let Some(i) = coarse.find(", ALPHA") { format!("{}, ALPHA/COVERAGE", &coarse[..i]) } else { coarse };
        *classes.entry(coarse.clone()).or_insert(0) += 1;
        summary.push((x, y, gval, game_res.as_ref().map(|g| format!("{} L{} ({:.3},{:.3},{:.3})", g.0, g.1, g.2[0], g.2[1], g.2[2])).unwrap_or_else(|| "—".into()), ours_res.as_ref().map(|o| format!("{} L{} ({:.3},{:.3},{:.3})", o.0, o.1, o.2[0], o.2[1], o.2[2])).unwrap_or_else(|| "—".into()), coarse));
        if cls.starts_with("extra layer") {
            for tok in cls.split("uv (").skip(1) {
                let nums: Vec<f32> = tok.split(')').next().unwrap_or("").split(',').filter_map(|s| s.trim().parse().ok()).collect();
                if nums.len() == 2 { extra_uv.push((nums[0], nums[1])); }
            }
        }
    }
    md.push_str("## Classes\n\n");
    if emu_total > 0 { md.push_str(&format!("The emulated game accumulate agrees with the captured ilightdir at {emu_agree} of {emu_total} texels.\n\n")); }
    for (c, n) in &classes { md.push_str(&format!("- {c}: {n}\n")); }
    if !extra_uv.is_empty() {
        let (mut umin, mut umax, mut vmin, mut vmax) = (f32::MAX, f32::MIN, f32::MAX, f32::MIN);
        for (u, v) in &extra_uv { umin = umin.min(*u); umax = umax.max(*u); vmin = vmin.min(*v); vmax = vmax.max(*v); }
        md.push_str(&format!("\nThe extra layers' alpha-passing card fragments sample the mask at u ∈ [{umin:.3}, {umax:.3}], v ∈ [{vmin:.3}, {vmax:.3}] ({} fragments); a 16×16 histogram of (u, v) (rows = v from 0, columns = u from 0):\n\n```\n", extra_uv.len()));
        let mut hist = [[0u32; 16]; 16];
        for (u, v) in &extra_uv { let i = (u.clamp(0.0, 0.9999) * 16.0) as usize; let j = (v.clamp(0.0, 0.9999) * 16.0) as usize; hist[j][i] += 1; }
        for j in 0..16 { md.push_str(&format!("v {:.2}: {}\n", j as f32 / 16.0, hist[j].iter().map(|n| format!("{n:4}")).collect::<String>())); }
        md.push_str("```\n");
    }
    md.push_str("\n## Summary table\n\n| # | texel | game MRT-0 | game resolves | ours resolves | class |\n|---|---|---|---|---|---|\n");
    for (i, s) in summary.iter().enumerate() { md.push_str(&format!("| {} | ({}, {}) | {:.4} | {} | {} | {} |\n", i + 1, s.0, s.1, s.2, s.3, s.4, s.5)); }
    if let Some(p) = f("--pixels-out") {
        pixels_out.sort(); pixels_out.dedup();
        let txt: String = pixels_out.iter().map(|(pi, x, y)| format!("{pi} {x} {y}\n")).collect();
        std::fs::write(&p, txt).map_err(|e| e.to_string())?;
        eprintln!("veg-diag: {} peel pixels written to {p} (LMTOOL_ABUF_DEBUG_LIST)", pixels_out.len());
    }
    match f("--out") { Some(o) => { std::fs::write(&o, &md).map_err(|e| e.to_string())?; eprintln!("veg-diag: table written to {o} ({} texels) in {:.1} s", cands.len(), t0.elapsed().as_secs_f32()); } None => print!("{md}") }
    Ok(())
}
