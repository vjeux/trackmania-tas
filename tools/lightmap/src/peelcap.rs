//! The captured PEEL LAYERS, read for their layer-count rule (rows 5/6, port engineer D).
//!
//! `lmtool peel-layers PASSCAP_ROOT [--game-manifest FILE]` — per captured direction and phase (world /
//! fitted) every `peel_depth` layer's pixel statistics: the count of pixels the layer's draws wrote
//! (depth ≠ the clear: 1.0 for the item layers, 0.0 for the environment block), its fraction of the
//! 4094² viewport, the depth range — the numbers the game's stop rule reads
//! (`RenderLightIndirectPeel` 0x140234df0 l.413–424: after every layer `RenderLightIndirectPeel_IsValid`
//! 0x1402363c0 fetches the pixel-count query of the layer render (device vtbl+0x3f0, no wait; the
//! fraction at scene+0x228, kept from the previous layer when the result is not ready), the peel stops
//! when the fraction < 0.001 or the layer counter passes 0x13 = 20 layers).

use crate::passdiff::{load_entry, read_manifest};
use crate::passdump::Manifest;
use std::path::Path;

/// One captured layer's statistics.
#[derive(Clone, Debug)]
pub struct LayerStat {
    pub capture: String,
    pub frame: u32,
    pub sweep: Option<u32>,
    pub direction: Option<u32>,
    pub phase: String,
    pub layer: u32,
    pub eid: u64,
    pub dir: Option<[f32; 3]>,
    /// Pixels the layer wrote (≠ the clear value).
    pub written: usize,
    /// Pixels of the viewport (the 1-px-inset 4094²).
    pub viewport: usize,
    pub min: f32,
    pub max: f32,
}

impl LayerStat {
    /// The fraction the game's query reports for this layer (written / viewport pixels).
    pub fn fraction(&self) -> f64 {
        self.written as f64 / self.viewport.max(1) as f64
    }
}

/// The game's peel stop rule (`RenderLightIndirectPeel` 0x140234df0 l.413–437): after every item layer
/// the pixel-count query of that layer's render is read WITHOUT waiting (0x1402363c0: device vtbl+0x3f0,
/// flag 0; the fraction at scene+0x228, the previous value — initially 1.0 — kept when the GPU has not
/// answered yet); the peel stops when the fraction < `threshold` (0.001 of the viewport) or after
/// `max_layers` (the counter passes 0x13). The no-wait read makes the count timing-dependent: the
/// captured peels rendered `lag` layers past the first one under the threshold (fitted peels: 2, 2, 3;
/// world peels: 3, 4, and ≥ 18 on the very first direction, whose 18 trailing layers were all empty).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PeelStop {
    pub threshold: f64,
    pub lag: usize,
    pub max_layers: usize,
}

impl Default for PeelStop {
    fn default() -> Self {
        PeelStop { threshold: 0.001, lag: 2, max_layers: 20 }
    }
}

impl PeelStop {
    /// How many item layers the game renders for these per-layer written fractions (in render order).
    pub fn layers_rendered(&self, fractions: &[f64]) -> usize {
        layers_rendered_with(fractions, self.lag, self.threshold, self.max_layers)
    }
}

/// The game's peel stop rule on a sequence of per-layer fractions (item layers 1.., in render order):
/// the number of ITEM layers the game renders when the pixel-count query of layer k is read after
/// layer k + `lag` (0 = read synchronously). Layer k's fraction stops the loop when it is < 0.001; the
/// counter cap is 20 layers.
pub fn layers_rendered(fractions: &[f64], lag: usize) -> usize {
    layers_rendered_with(fractions, lag, 0.001, 20)
}

pub fn layers_rendered_with(fractions: &[f64], lag: usize, threshold: f64, max_layers: usize) -> usize {
    let mut valid = 1.0f64;
    let mut rendered = 0usize;
    loop {
        // the loop body in 0x140234df0: render layer `rendered` (item layers count from 0 here), then the
        // IsValid read (the result of the layer `lag` renders back), then the stop tests
        rendered += 1;
        if rendered >= lag + 1 {
            valid = fractions.get(rendered - 1 - lag).copied().unwrap_or(0.0);
        }
        if valid < threshold {
            return rendered;
        }
        if rendered >= max_layers {
            return rendered;
        }
    }
}

/// The captured ITEM-LAYER COUNT of every peel of every direction, for OUR direction list (matched by
/// nearest vector, as `passdiff::peel_frustums_for` matches the frustums): `out[i][p]` = the number of
/// item layers the game rendered in peel `p` (0 = world, 1 = fitted) of the captured direction nearest
/// our direction `i`, `None` where the capture has no such peel or our direction is not captured
/// (cos < 0.9999). A capture whose first snapshot is missing (pwc1's direction 0 starts at its first
/// item layer) still counts its item layers: an entry counts when its `environment_block` flag is false
/// (or absent and its layer index is not 0 of a peel whose layer 0 is flagged).
pub fn captured_layer_counts(m: &Manifest, sweep: u32, ours: &[[f32; 3]]) -> Vec<Vec<Option<usize>>> {
    let mut m = m.clone();
    if m.passes.iter().any(|e| e.pass == "peel_depth" && e.peel.is_none()) {
        crate::passdiff::assign_peels(&mut m);
    }
    // (direction, peel) → (vector, item layer count)
    let mut counts: std::collections::HashMap<(u32, u32), ([f32; 3], usize)> = std::collections::HashMap::new();
    for e in m.passes.iter().filter(|e| e.pass == "peel_depth" && e.sweep.unwrap_or(0) == sweep) {
        let (Some(d), Some(p)) = (e.direction, e.peel) else { continue };
        let Some(v) = e.dir.filter(|v| *v != [0.0; 3]) else { continue };
        let is_env = e.environment_block.unwrap_or(false);
        let ent = counts.entry((d, p)).or_insert((v, 0));
        if !is_env {
            ent.1 += 1;
        }
    }
    ours.iter()
        .map(|od| {
            let mut best: Option<(f32, u32)> = None;
            for ((d, _), (v, _)) in &counts {
                let c = v[0] * od[0] + v[1] * od[1] + v[2] * od[2];
                if best.map(|b| c > b.0).unwrap_or(true) {
                    best = Some((c, *d));
                }
            }
            match best {
                Some((c, d)) if c >= 0.9999 => {
                    let np = counts.keys().filter(|(dd, _)| *dd == d).map(|(_, p)| *p + 1).max().unwrap_or(0);
                    (0..np).map(|p| counts.get(&(d, p)).map(|(_, n)| *n)).collect()
                }
                _ => Vec::new(),
            }
        })
        .collect()
}

/// Gather the statistics of every `peel_depth` entry of a capture.
pub fn layer_stats(root: &Path, m: &Manifest) -> Vec<LayerStat> {
    let mut out = Vec::new();
    for e in m.passes.iter().filter(|e| e.pass == "peel_depth") {
        let b = match load_entry(root, e) {
            Ok(b) => b,
            Err(err) => {
                eprintln!("peel-layers: {}: {err}", e.file);
                continue;
            }
        };
        let layer = e.layer.unwrap_or(0);
        let clear = if layer == 0 { 0.0f32 } else { 1.0f32 };
        let (mut written, mut mn, mut mx) = (0usize, f32::MAX, f32::MIN);
        // the viewport (1, 1, 4094, 4094): the outer ring is never drawn
        let vp: Vec<f32> = e.viewport.clone().unwrap_or_else(|| vec![0.0, 0.0, b.w as f32, b.h as f32]);
        let (x0, y0, vw, vh) = (vp[0] as u32, vp[1] as u32, vp[2] as u32, vp[3] as u32);
        for y in y0..(y0 + vh).min(b.h) {
            for x in x0..(x0 + vw).min(b.w) {
                let d = b.get(x, y, 0);
                if d != clear {
                    written += 1;
                    mn = mn.min(d);
                    mx = mx.max(d);
                }
            }
        }
        let phase = e.notes.as_deref().and_then(|_| None::<String>).unwrap_or_else(|| phase_of(e));
        out.push(LayerStat { capture: e.capture.clone().unwrap_or_default(), frame: e.frame.unwrap_or(0), sweep: e.sweep, direction: e.direction, phase, layer, eid: e.eid_last.or(e.eid).unwrap_or(0), dir: e.dir, written, viewport: (vw * vh) as usize, min: mn, max: mx });
    }
    out.sort_by(|a, b| (a.frame, a.direction.unwrap_or(0), a.phase.clone(), a.eid).cmp(&(b.frame, b.direction.unwrap_or(0), b.phase.clone(), b.eid)));
    out
}

/// The entry's peel phase name (the capture manifest's `phase`, carried in the notes or the peel index).
fn phase_of(e: &crate::passdump::Entry) -> String {
    match e.peel {
        Some(0) => "world".into(),
        Some(1) => "fitted".into(),
        _ => "?".into(),
    }
}

/// `lmtool peel-layers ROOT [--game-manifest FILE]`.
pub fn run(args: &[String]) {
    let f = |k: &str| args.iter().position(|x| x == k).and_then(|i| args.get(i + 1)).cloned();
    let root = std::path::PathBuf::from(&args[1]);
    let mpath = f("--game-manifest").map(std::path::PathBuf::from).unwrap_or_else(|| root.join("MANIFEST.json"));
    let txt = std::fs::read_to_string(&mpath).unwrap_or_else(|e| panic!("{}: {e}", mpath.display()));
    let mut m = read_manifest(&txt).expect("manifest");
    crate::passdiff::assign_peels(&mut m);
    let stats = layer_stats(&root, &m);
    println!("capture frame  dir phase  layer eid     written    fraction   min      max");
    let mut key_prev: Option<(u32, Option<u32>, String)> = None;
    let mut seq: Vec<f64> = Vec::new();
    let flush = |seq: &Vec<f64>, key: &Option<(u32, Option<u32>, String)>| {
        if let Some(k) = key {
            if !seq.is_empty() {
                let lags: Vec<String> = (0..4).map(|lag| format!("lag {lag}: {}", layers_rendered(seq, lag))).collect();
                println!("  → {} item layers captured for frame {} dir {:?} {}; the stop rule (< 0.001) predicts {}", seq.len(), k.0, k.1, k.2, lags.join(", "));
            }
        }
    };
    for s in &stats {
        let key = (s.frame, s.direction, s.phase.clone());
        if key_prev.as_ref() != Some(&key) {
            flush(&seq, &key_prev);
            seq.clear();
            key_prev = Some(key);
        }
        if s.layer > 0 {
            seq.push(s.fraction());
        }
        println!("{:7} {:6} {:4} {:6} {:5} {:7} {:10} {:.6} {:8.4} {:8.4}{}", s.capture, s.frame, s.direction.map(|d| d.to_string()).unwrap_or("-".into()), s.phase, s.layer, s.eid, s.written, s.fraction(), if s.written > 0 { s.min } else { f32::NAN }, if s.written > 0 { s.max } else { f32::NAN }, s.dir.map(|d| format!("  D ({:.3},{:.3},{:.3})", d[0], d[1], d[2])).unwrap_or_default());
    }
    flush(&seq, &key_prev);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_stop_rule_counts_layers_until_a_layer_under_a_tenth_of_a_percent() {
        // synchronous read: layers 0.5, 0.01, 0.0005 → the third layer's fraction stops the loop after it
        assert_eq!(layers_rendered(&[0.5, 0.01, 0.0005, 0.0, 0.0], 0), 3);
        // a lag of two: the stop reads the third layer's result two layers later
        assert_eq!(layers_rendered(&[0.5, 0.01, 0.0005, 0.0, 0.0], 2), 5);
        // nothing ever under the threshold: the cap of 20 layers
        assert_eq!(layers_rendered(&[0.5; 40], 0), 20);
        // an empty scene: the first layer stops it
        assert_eq!(layers_rendered(&[0.0], 0), 1);
        // beyond the captured sequence the fraction is taken as 0
        assert_eq!(layers_rendered(&[0.5, 0.5], 0), 3);
    }
}

/// `lmtool quanta-diff GAME_ROOT GAME_FILE OURS_ROOT OURS_FILE [--top N] [--x0 X --y0 Y --w W --h H]` — two
/// R11G11B10 buffers (the game's DDS, our raw dump) compared in STORAGE STEPS: per channel the
/// histogram of (ours − game) in mantissa steps of the game's value (UF11: 6 bits, UF10: 5 bits),
/// the mean ratio, and the worst pixels. A transcription that is right to the quantum shows every
/// pixel at 0 or ±1 step; a bias shows as a shifted histogram.
pub fn quanta_diff(args: &[String]) {
    let f = |k: &str| args.iter().position(|x| x == k).and_then(|i| args.get(i + 1)).cloned();
    let (groot, gfile, oroot, ofile) = (std::path::PathBuf::from(&args[1]), &args[2], std::path::PathBuf::from(&args[3]), &args[4]);
    let top: usize = f("--top").map(|v| v.parse().unwrap()).unwrap_or(12);
    // --depth: the game's R16 depth (UNORM16) against our R32_FLOAT depth dump, the clear = 1.0 (item
    // layers) or --clear V; the "quanta" are then D16 steps (1/65535)
    let depth = args.iter().any(|x| x == "--depth");
    let clear: f32 = f("--clear").map(|v| v.parse().unwrap()).unwrap_or(1.0);
    let ge = { let mut e = crate::passdump::entry("x", gfile.clone(), "peel"); e.format = if depth { "R16_TYPELESS".into() } else { "R11G11B10_FLOAT".into() }; e };
    let g = load_entry(&groot, &ge).expect("game buffer");
    let oe = { let mut e = crate::passdump::entry("x", ofile.clone(), "peel"); e.format = if depth { "R32_FLOAT".into() } else { "R11G11B10_FLOAT".into() }; e.width = g.w; e.height = g.h; e.row_pitch = g.w * if depth { 4 } else { 4 }; e };
    let o = load_entry(&oroot, &oe).expect("our buffer");
    if depth {
        let (mut n, mut both_clear, mut g_only, mut o_only) = (0usize, 0usize, 0usize, 0usize);
        let mut gb = (u32::MAX, u32::MAX, 0u32, 0u32);
        let mut ob = (u32::MAX, u32::MAX, 0u32, 0u32);
        let mut hist: std::collections::BTreeMap<i64, usize> = std::collections::BTreeMap::new();
        let mut g_only_rows: std::collections::BTreeMap<u32, usize> = std::collections::BTreeMap::new();
        let (mut gmin, mut gmax) = (f32::MAX, f32::MIN);
        for y in 0..g.h {
            for x in 0..g.w {
                let (gv, ov) = (g.get(x, y, 0), o.get(x, y, 0));
                let (gc, oc) = (gv == clear, ov == clear);
                if gc && oc { both_clear += 1; continue; }
                if oc { g_only += 1; gb = (gb.0.min(x), gb.1.min(y), gb.2.max(x), gb.3.max(y)); *g_only_rows.entry(y / 64).or_insert(0) += 1; gmin = gmin.min(gv); gmax = gmax.max(gv); continue; }
                if gc { o_only += 1; ob = (ob.0.min(x), ob.1.min(y), ob.2.max(x), ob.3.max(y)); continue; }
                n += 1;
                let d = ((ov - gv) * 65535.0).round() as i64;
                *hist.entry(d.clamp(-50, 50)).or_insert(0) += 1;
            }
        }
        println!("{n} pixels with a surface on both sides ({both_clear} both clear, {g_only} game-only, {o_only} ours-only)");
        println!("game-only bbox x {}..{} y {}..{} (game depth {gmin:.4}..{gmax:.4}); ours-only bbox x {}..{} y {}..{}", gb.0, gb.2, gb.1, gb.3, ob.0, ob.2, ob.1, ob.3);
        println!("game-only pixels per 64-row band: {}", g_only_rows.iter().map(|(k, v)| format!("{}:{v}", k * 64)).collect::<Vec<_>>().join(" "));
        let within1: usize = hist.iter().filter(|(k, _)| k.abs() <= 1).map(|(_, v)| *v).sum();
        println!("D16 step differences (ours − game): within ±1: {:.3} %; {}", 100.0 * within1 as f64 / n.max(1) as f64, hist.iter().filter(|(_, v)| **v * 10000 >= n).map(|(k, v)| format!("{k:+}: {v}")).collect::<Vec<_>>().join(", "));
        // --png FILE [--x0 X --y0 Y --w W --h H]: the coverage map of the crop — white = both, red = game only,
        // blue = ours only, black = both clear; with --scale S each source pixel is S×S
        if let Some(path) = f("--png") {
            let sc: u32 = f("--scale").map(|v| v.parse().unwrap()).unwrap_or(1);
            let (x0, y0) = (f("--x0").map(|v| v.parse::<u32>().unwrap()).unwrap_or(0), f("--y0").map(|v| v.parse::<u32>().unwrap()).unwrap_or(0));
            let (rw, rh) = (f("--w").map(|v| v.parse::<u32>().unwrap()).unwrap_or(g.w - x0), f("--h").map(|v| v.parse::<u32>().unwrap()).unwrap_or(g.h - y0));
            let mut px = vec![0u8; (rw * sc * rh * sc * 3) as usize];
            for y in 0..rh {
                for x in 0..rw {
                    let (gv, ov) = (g.get(x0 + x, y0 + y, 0), o.get(x0 + x, y0 + y, 0));
                    let c: [u8; 3] = match (gv == clear, ov == clear) { (true, true) => [0, 0, 0], (false, false) => [255, 255, 255], (false, true) => [255, 40, 40], (true, false) => [40, 90, 255] };
                    for dy in 0..sc { for dx in 0..sc { let i = (((y * sc + dy) * rw * sc) + x * sc + dx) as usize * 3; px[i..i + 3].copy_from_slice(&c); } }
                }
            }
            crate::png::write_rgb(&path, rw * sc, rh * sc, &px).expect("png");
            println!("coverage map {path} ({rw}×{rh} at ×{sc}): white both, red game-only, blue ours-only");
        }
        return;
    }
    assert!(g.w == o.w && g.h == o.h, "sizes differ: {}×{} vs {}×{}", g.w, g.h, o.w, o.h);
    let (x0, y0) = (f("--x0").map(|v| v.parse::<u32>().unwrap()).unwrap_or(0), f("--y0").map(|v| v.parse::<u32>().unwrap()).unwrap_or(0));
    let (rw, rh) = (f("--w").map(|v| v.parse::<u32>().unwrap()).unwrap_or(g.w - x0), f("--h").map(|v| v.parse::<u32>().unwrap()).unwrap_or(g.h - y0));
    // the mantissa step of a UF11/UF10 value: 2^(exponent − bits)
    let step = |v: f32, bits: i32| -> f32 { if v <= 0.0 { 2f32.powi(-14 - bits) } else { let e = v.log2().floor() as i32; 2f32.powi(e.max(-14) - bits) } };
    let mut hist: [std::collections::BTreeMap<i64, usize>; 3] = Default::default();
    let mut sum_g = [0f64; 3];
    let mut sum_o = [0f64; 3];
    let mut n = 0usize;
    let (mut both_zero, mut g_only, mut o_only) = (0usize, 0usize, 0usize);
    let mut worst: Vec<(f32, u32, u32, [f32; 3], [f32; 3])> = Vec::new();
    for y in y0..(y0 + rh).min(g.h) {
        for x in x0..(x0 + rw).min(g.w) {
            let gv = [g.get(x, y, 0), g.get(x, y, 1), g.get(x, y, 2)];
            let ov = [o.get(x, y, 0), o.get(x, y, 1), o.get(x, y, 2)];
            let gz = gv.iter().all(|c| *c == 0.0);
            let oz = ov.iter().all(|c| *c == 0.0);
            if gz && oz { both_zero += 1; continue; }
            if oz { g_only += 1; if g_only <= 5 { println!("  game-only pixel ({x}, {y}): game ({:.4}, {:.4}, {:.4})", gv[0], gv[1], gv[2]); } continue; }
            if gz { o_only += 1; if o_only <= 5 || o_only % 1000 == 0 { println!("  ours-only pixel ({x}, {y}): ours ({:.4}, {:.4}, {:.4})", ov[0], ov[1], ov[2]); } continue; }
            n += 1;
            let mut wd = 0f32;
            for c in 0..3 {
                let bits = if c == 2 { 5 } else { 6 };
                let st = step(gv[c], bits);
                let d = ((ov[c] - gv[c]) / st).round() as i64;
                *hist[c].entry(d).or_insert(0) += 1;
                sum_g[c] += gv[c] as f64;
                sum_o[c] += ov[c] as f64;
                wd = wd.max((ov[c] - gv[c]).abs() / gv[c].abs().max(1e-6));
            }
            if worst.len() < top || wd > worst.last().map(|w| w.0).unwrap_or(0.0) {
                worst.push((wd, x, y, gv, ov));
                worst.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap());
                worst.truncate(top);
            }
        }
    }
    println!("{} pixels compared ({both_zero} both black, {g_only} game-only, {o_only} ours-only)", n);
    for c in 0..3 {
        let name = ["R", "G", "B"][c];
        let mut items: Vec<(i64, usize)> = hist[c].iter().map(|(k, v)| (*k, *v)).collect();
        items.sort();
        let within1: usize = items.iter().filter(|(k, _)| k.abs() <= 1).map(|(_, v)| *v).sum();
        let exact: usize = items.iter().filter(|(k, _)| *k == 0).map(|(_, v)| *v).sum();
        println!("{name}: mean ours/game {:.5}; exact {:.3} %, within ±1 step {:.3} %; steps: {}", sum_o[c] / sum_g[c].max(1e-12), 100.0 * exact as f64 / n.max(1) as f64, 100.0 * within1 as f64 / n.max(1) as f64, items.iter().filter(|(_, v)| *v * 100000 >= n).map(|(k, v)| format!("{k:+}: {v}")).collect::<Vec<_>>().join(", "));
    }
    println!("worst pixels (relative): ");
    for (wd, x, y, gv, ov) in &worst {
        println!("  ({x}, {y}) {:.2} %: game ({:.5}, {:.5}, {:.5}) ours ({:.5}, {:.5}, {:.5})", wd * 100.0, gv[0], gv[1], gv[2], ov[0], ov[1], ov[2]);
    }
}

/// `lmtool layer-gap ROOT FILE_A FILE_B [--x0 --y0 --w --h]` — two captured R16 depth layers of one peel
/// (A = layer k, B = layer k+1): the histogram of B − A in D16 steps where both hold a surface (how
/// far apart consecutive layers sit — coincident surfaces the bias separates show as 1–3 steps).
pub fn layer_gap(args: &[String]) {
    let f = |k: &str| args.iter().position(|x| x == k).and_then(|i| args.get(i + 1)).cloned();
    let root = std::path::PathBuf::from(&args[1]);
    let ea = { let mut e = crate::passdump::entry("x", args[2].clone(), "peel"); e.format = "R16_TYPELESS".into(); e };
    let eb = { let mut e = crate::passdump::entry("x", args[3].clone(), "peel"); e.format = "R16_TYPELESS".into(); e };
    let a = load_entry(&root, &ea).expect("A");
    let b = load_entry(&root, &eb).expect("B");
    let (x0, y0) = (f("--x0").map(|v| v.parse::<u32>().unwrap()).unwrap_or(0), f("--y0").map(|v| v.parse::<u32>().unwrap()).unwrap_or(0));
    let (rw, rh) = (f("--w").map(|v| v.parse::<u32>().unwrap()).unwrap_or(a.w - x0), f("--h").map(|v| v.parse::<u32>().unwrap()).unwrap_or(a.h - y0));
    let mut hist: std::collections::BTreeMap<i64, usize> = std::collections::BTreeMap::new();
    let mut n = 0usize;
    for y in y0..(y0 + rh).min(a.h) {
        for x in x0..(x0 + rw).min(a.w) {
            let (va, vb) = (a.get(x, y, 0), b.get(x, y, 0));
            if va == 1.0 || vb == 1.0 { continue; }
            n += 1;
            let d = ((vb - va) * 65535.0).round() as i64;
            *hist.entry(d.clamp(-100, 100)).or_insert(0) += 1;
        }
    }
    println!("{n} pixels with both layers; B − A in D16 steps: {}", hist.iter().filter(|(_, v)| **v * 1000 >= n).map(|(k, v)| format!("{k:+}: {v}")).collect::<Vec<_>>().join(", "));
}
