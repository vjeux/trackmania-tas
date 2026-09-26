//! ROW 11 — SWEEP 1 (the 128-direction set) of the game's lightmapper, read off the capture
//! (`passcap/pwc-day`, capture pwc6 frames 7533–7537: all 128 directions, `InvDirCount` 1/128).
//!
//! Structure = sweep 0's, per direction: the environment block, the world peel (colour = the
//! ILightInput texture sampled at the LM uv, front faces only — see `ilight_input`), the accumulate,
//! the probe draws, the H-basis draws with `InvDirCount = 1/128` into the SAME four MRTs (running
//! sums over both sweeps), AddAmbient; the sky-visibility draw (PS 17154) does not run in sweep 1.
//!
//! The direction set: the rotated 128-point set of `Std.PointsInSphere.Gbx` (`dome::rotate_set` of
//! `PointSets::set(128)`): all 127 captured directions match within 0.03° (the f32 table against the
//! f64 rotation); the issue order is a fixed permutation of the set (`SWEEP1_ISSUE_ORDER_128`), not a
//! sort by any coordinate: the first 17 issued are the |x|-dominant points (8 of −x then 9 of +x),
//! like sweep 0's first 9 are the +z-dominant ones (0, 1, 65, 17, 241, 240, 16, 18, 2 of the 256-set).
//! `SPlugGroupOfPointInSphere::Compute` (FUN_140460270) only round-robins the LIST into ss² groups
//! (point i → group i mod 9 while the quotas last), so the list itself carries this order.

use crate::dome::{rotate_set, PointSets};
use crate::passdiff::Buf;
use crate::passdump::Manifest;

/// Sweep 1's issue order at quality 3 (N = 128) as indices into the rotated 128-set, indexed by the
/// game's issue index (the capture manifest's `sweep_direction_index`, 0..127). The capture banked the
/// H-basis of issue indices 1..127; index 0 — the first sweep-1 direction, drawn right after the sweep
/// transition in frame 7533 — is the one set point no captured index took, set index 0 (the point
/// sweep 0 starts with too); marked inferred until its vector is read off frame 7533's layers.
pub const SWEEP1_ISSUE_ORDER_128: [u8; 128] = [
    0, // inferred (see above)
    28, 69, 116, 49, 114, 98, 9, 48, 18, 124, 6, 55, 17, 96, 33, 75, 2, 89, 8, 93, 83, 120, 57, 11, 58, 46, 44, 104, 107, 24, 3, 76, 88, 82, 63, 22, 86, 100, 59, 72, 118, 53, 106, 56, 111, 78, 4, 115, 105, 91, 43, 20, 99, 23, 51, 95, 70, 47, 40, 81, 19, 29, 85, 12, 36, 113, 65, 52, 38, 15, 112, 34, 41, 125, 126, 5, 122, 7, 90, 35, 79, 108, 64, 77, 61, 66, 121, 50, 27, 97, 74, 32, 127, 94, 1, 84, 42, 119, 68, 67, 117, 31, 101, 45, 37, 87, 123, 26, 10, 25, 60, 16, 92, 109, 73, 54, 103, 110, 39, 102, 21, 14, 13, 80, 30, 62, 71,
];

/// The issue indices of `SWEEP1_ISSUE_ORDER_128` read off the capture (the H-basis snapshots' vectors).
pub const SWEEP1_ISSUE_ORDER_CAPTURED: std::ops::Range<usize> = 1..128;

/// Sweep 0's issue order (the 256-set) as far as the captures go: pwc2 gave indices 0–123, pwc6
/// 246–255; the rest is unknown (`None`).
pub fn sweep0_issue_order_known(m: &Manifest, set256: &[[f32; 3]]) -> Vec<Option<u32>> {
    let dirs = captured_directions(m, 0);
    let mut out = vec![None; 256];
    for (k, d) in dirs {
        if (k as usize) < 256 {
            let (i, _) = nearest(d, set256);
            out[k as usize] = Some(i as u32);
        }
    }
    out
}

/// The angle (degrees) between two unit vectors.
pub fn angle_deg(a: [f32; 3], b: [f32; 3]) -> f32 {
    let c = (a[0] as f64 * b[0] as f64 + a[1] as f64 * b[1] as f64 + a[2] as f64 * b[2] as f64).clamp(-1.0, 1.0);
    c.acos().to_degrees() as f32
}

/// The nearest set point to `d`: (index, angle in degrees).
pub fn nearest(d: [f32; 3], set: &[[f32; 3]]) -> (usize, f32) {
    let mut best = (0usize, f32::INFINITY);
    for (i, p) in set.iter().enumerate() {
        let a = angle_deg(d, *p);
        if a < best.1 {
            best = (i, a);
        }
    }
    best
}

/// The captured directions of a sweep with their issue index: (sweep_direction_index, D), one per
/// index (the H-basis snapshot entries carry both; other passes fill gaps).
pub fn captured_directions(m: &Manifest, sweep: u32) -> Vec<(u32, [f32; 3])> {
    let mut map = std::collections::BTreeMap::<u32, [f32; 3]>::new();
    for e in &m.passes {
        if e.sweep == Some(sweep) {
            if let (Some(k), Some(d)) = (e.sweep_direction_index, e.dir) {
                map.entry(k).or_insert(d);
            }
        }
    }
    map.into_iter().collect()
}

/// The dominant axis of a direction as a label (`+x`, `-y`, …).
pub fn dominant_axis(d: [f32; 3]) -> String {
    let a = [d[0].abs(), d[1].abs(), d[2].abs()];
    let k = if a[0] >= a[1] && a[0] >= a[2] { 0 } else if a[1] >= a[2] { 1 } else { 2 };
    format!("{}{}", if d[k] >= 0.0 { '+' } else { '-' }, ['x', 'y', 'z'][k])
}

/// `lmtool sweep-dirs GAME/MANIFEST.json`: per sweep, the point set the captured directions come
/// from (the rotated N-set with the smallest worst angle), the issue order as set indices, the
/// missing ones, and the dominant-axis structure of the order.
pub fn report(m: &Manifest, points: &PointSets) -> String {
    let mut s = String::new();
    let sweeps: std::collections::BTreeSet<u32> = m.passes.iter().filter_map(|e| e.sweep).collect();
    for sw in sweeps {
        let dirs = captured_directions(m, sw);
        if dirs.is_empty() {
            continue;
        }
        let max_idx = dirs.iter().map(|(k, _)| *k).max().unwrap_or(0);
        s.push_str(&format!("sweep {sw}: {} captured directions with an issue index (indices {}..{})\n", dirs.len(), dirs[0].0, max_idx));
        // candidate sets: every table set with at least as many points as the highest index
        let mut best: Option<(usize, f32, Vec<[f32; 3]>)> = None;
        for set in &points.sets {
            if set.len() <= max_idx as usize || set.len() > 1032 {
                continue;
            }
            let rot = rotate_set(set);
            let worst = dirs.iter().map(|(_, d)| nearest(*d, &rot).1).fold(0f32, f32::max);
            if best.as_ref().map(|b| worst < b.1).unwrap_or(true) {
                best = Some((set.len(), worst, rot));
            }
        }
        let Some((n, worst, rot)) = best else { continue };
        let order: Vec<(u32, usize, f32)> = dirs.iter().map(|(k, d)| { let (i, a) = nearest(*d, &rot); (*k, i, a) }).collect();
        let distinct: std::collections::BTreeSet<usize> = order.iter().map(|o| o.1).collect();
        let missing: Vec<usize> = (0..n).filter(|i| !distinct.contains(i)).collect();
        s.push_str(&format!("  = the rotated {n}-set: worst angle {worst:.4}°, {} distinct set indices; set indices never issued (captured): {}{}\n", distinct.len(), missing.len(), if missing.len() <= 12 { format!(" {:?}", missing) } else { String::new() }));
        let idx: Vec<String> = order.iter().map(|o| format!("{}", o.1)).collect();
        s.push_str(&format!("  issue order (set index per sweep_direction_index): {}\n", idx.join(" ")));
        let axes: Vec<String> = order.iter().take(40).map(|o| dominant_axis(rot[o.1])).collect();
        s.push_str(&format!("  dominant axes of the first 40 issued: {}\n", axes.join(" ")));
        // is the order a sort by any simple key?
        let keys: [(&str, fn([f32; 3]) -> f32); 5] = [("x", |d| d[0]), ("y", |d| d[1]), ("z", |d| d[2]), ("|y|", |d| d[1].abs()), ("azimuth", |d| d[0].atan2(d[2]))];
        let mut sorted_by = Vec::new();
        for (name, f) in keys {
            let v: Vec<f32> = order.iter().map(|o| f(rot[o.1])).collect();
            let asc = v.windows(2).all(|w| w[0] <= w[1]);
            let desc = v.windows(2).all(|w| w[0] >= w[1]);
            if asc || desc {
                sorted_by.push(format!("{name} {}", if asc { "ascending" } else { "descending" }));
            }
        }
        s.push_str(&format!("  sorted by a coordinate: {}\n", if sorted_by.is_empty() { "no".to_string() } else { sorted_by.join(", ") }));
        if sw == 1 && n == 128 {
            let table_ok = order.iter().all(|o| (o.0 as usize) < 128 && SWEEP1_ISSUE_ORDER_128[o.0 as usize] as usize == o.1);
            s.push_str(&format!("  matches SWEEP1_ISSUE_ORDER_128: {}\n", if table_ok { "yes (every captured index)" } else { "NO" }));
        }
    }
    s
}


// ───────────────────────────── the sweep-1 ILightInput ─────────────────────────────

/// The bounce decode of the sweep-0 H-basis C0 into the sweep-1 peel colour: 1/√(2π) — pinned on the
/// capture (`lmtool sweep1-check`): with it every (channel, MDiffuse byte) cell of the interior texels
/// admits a consistent GPU sRGB-decode value (κ = 0.4 leaves 30 cells with an empty interval).
pub const KAPPA_BOUNCE: f32 = 0.398_942_28;

/// The MDiffuse atlas (B8G8R8A8 16969, an `_SRGB` view: `ld` decodes) as linear RGB. `table` = a per-byte
/// decode table (the GPU's own, fitted from a capture) or None for the IEC 61966-2-1 curve.
pub fn mdiffuse_linear(mdiffuse8: &Buf, table: Option<&[[f32; 256]; 3]>) -> Buf {
    let mut out = Buf::new(mdiffuse8.w, mdiffuse8.h, 3);
    for y in 0..mdiffuse8.h {
        for x in 0..mdiffuse8.w {
            for c in 0..3u32 {
                let byte = (mdiffuse8.get(x, y, c) * 255.0).round().clamp(0.0, 255.0) as usize;
                let lin = match table {
                    Some(t) => t[c as usize][byte],
                    None => crate::gpufmt::srgb_to_linear(byte as f32 / 255.0),
                };
                out.set(x, y, c, lin);
            }
        }
    }
    out
}

/// The SWEEP-1 `ILightInput` (the texture every sweep-1 peel layer samples as SRV1) from the sweep-0
/// H-basis C0 target, the MDiffuse atlas and the chart coverage mask:
///
/// 1. `finalprep::resolve_ps25113` of C0 (rgb/w with the partial-texel neighbour rule; w = the alpha =
///    Σ 1/N of the sweep), stored RGBA16F (truncation) — frame 7533 eids 13980–14040 (PS 8772 = 25113 on
///    the four MRTs 8356/8475/8478/8481 → 8359/8356/8475/8478), then PS 1109 × (2, 2, 2, 0) into the
///    "previous sweep" targets 8534/8537/8540/8543 (the finalisation's ×2 stage) and the MRTs cleared for
///    sweep 1;
/// 2. PS 1038 with `ColorMat4 = diag(0.3989423)` (κ = 1/√(2π) in f32) on the resolved C0 → 8490, stored
///    R11G11B10 (truncation) — eid 14154;
/// 3. PS 1038 with `ColorMat4` = the alpha column on the resolved C0 → the coverage mask 8499 (R8_UNORM,
///    round to nearest: 1 where w ≥ 0.01, the raw w below) — eid 14177;
/// 4. PS 1109 `ScaleSrc (1, 1, 1, 1)` of the MDiffuse atlas with blend DstColor·Src → 8490, stored R11G11B10
///    (truncation) — eid 14201;
/// 5. `ilightin::dilate_ps1335` × 8 with that mask (8490 ↔ 8493, masks 8499 → 8716 ↔ 8713) — eids 14222–14292.
///
/// `lmtool sweep1-check`: the covered texels reproduce the captured 8490 of frame 7534 exactly up to the
/// GPU's sRGB table; `coverage` = None takes the chain's own mask (step 3), Some(m) another one.
pub fn ilightinput_from_c0(c0: &Buf, mdiffuse_lin: &Buf, coverage: Option<&Buf>, kappa: f32) -> Buf {
    let resolved = crate::finalprep::resolve_ps25113(c0, false, crate::gpufmt::Rounding::Truncate);
    // the coverage mask of the chain: PS 1038 with ColorMat4 = the alpha column on the RESOLVED image (alpha 1 where
    // w ≥ 0.01, the raw w below), stored R8_UNORM (round to nearest) — frame 7533 eid 14177
    let mask_from_resolve = {
        let mut m8 = Buf::new(c0.w, c0.h, 1);
        for y in 0..c0.h { for x in 0..c0.w { m8.set(x, y, 0, crate::ilightin::unorm8_rt(resolved.get(x, y, 3), crate::gpuenc::UnormRounding::NearestEven)); } }
        m8
    };
    let coverage = coverage.unwrap_or(&mask_from_resolve);
    let mut cur = Buf::new(c0.w, c0.h, 3);
    for y in 0..c0.h {
        for x in 0..c0.w {
            let v = crate::gpufmt::quantise_r11g11b10([resolved.get(x, y, 0) * kappa, resolved.get(x, y, 1) * kappa, resolved.get(x, y, 2) * kappa], crate::gpufmt::Rounding::Truncate);
            let m = crate::gpufmt::quantise_r11g11b10([v[0] * mdiffuse_lin.get(x, y, 0), v[1] * mdiffuse_lin.get(x, y, 1), v[2] * mdiffuse_lin.get(x, y, 2)], crate::gpufmt::Rounding::Truncate);
            cur.set(x, y, 0, m[0]);
            cur.set(x, y, 1, m[1]);
            cur.set(x, y, 2, m[2]);
        }
    }
    let mut cov = coverage.clone();
    for _ in 0..8 {
        let (c, w) = crate::ilightin::dilate_ps1335(&cur, &cov);
        cur = crate::ilightin::quantise_r11(&c, crate::gpufmt::Rounding::Truncate);
        let mut w8 = Buf::new(w.w, w.h, 1);
        for y in 0..w.h {
            for x in 0..w.w {
                w8.set(x, y, 0, crate::ilightin::unorm8_rt(w.get(x, y, 0), crate::gpuenc::UnormRounding::NearestEven));
            }
        }
        cov = w8;
    }
    cur
}

/// The peel pixel shader's COLOUR path in every sweep (PS 17131 / 17545 / 8526, l.4–7 of the DXBC):
/// `rgb = TMapILightInput.Sample(SGbxClamp_Aniso, uv_lm)`, `g = max(g, 1e-5)`, `rgb &= isfrontface`,
/// stored R11G11B10 (truncation). At the fitted peel's magnification the anisotropic sampler is a
/// bilinear tap; `uv_lm` = the vertex's TexCoord1 through the chart's ST (`lmaccum::chart_st`),
/// interpolated linearly (an orthographic projection).
pub fn peel_color(ilightinput: &Buf, u: f32, v: f32, front_face: bool) -> [f32; 3] {
    if !front_face {
        return [0.0; 3];
    }
    let s = crate::finalprep::sample_bilinear_clamp(ilightinput, u, v);
    let rgb = [s[0], s[1].max(1e-5), s[2]];
    crate::gpufmt::quantise_r11g11b10(rgb, crate::gpufmt::Rounding::Truncate)
}

/// Fit the GPU's sRGB→linear table from a captured ILightInput: for every (channel, byte) the interval
/// of decode values `L` with `stored ≤ trunc(a · L) < next(stored)` over the interior texels sharing the
/// byte (a = the κ-scaled resolve before the multiply). Returns per cell (lo, hi, count).
pub fn fit_srgb_table(c0: &Buf, mdiffuse8: &Buf, coverage: &Buf, captured: &Buf, kappa: f32) -> Vec<[(f32, f32, usize); 256]> {
    let resolved = crate::finalprep::resolve_ps25113(c0, false, crate::gpufmt::Rounding::Truncate);
    let mut cells = vec![[(0f32, f32::INFINITY, 0usize); 256]; 3];
    for y in 2..captured.h.saturating_sub(2) {
        for x in 2..captured.w.saturating_sub(2) {
            // a covered texel keeps its own value through the dilation
            if coverage.get(x, y, 0) < 0.0001 {
                continue;
            }
            let a = crate::gpufmt::quantise_r11g11b10([resolved.get(x, y, 0) * kappa, resolved.get(x, y, 1) * kappa, resolved.get(x, y, 2) * kappa], crate::gpufmt::Rounding::Truncate);
            for c in 0..3u32 {
                let stored = captured.get(x, y, c);
                if a[c as usize] <= 0.0 || stored <= 0.0 {
                    continue;
                }
                let byte = (mdiffuse8.get(x, y, c) * 255.0).round().clamp(0.0, 255.0) as usize;
                let q = crate::gpucmp::quantum(crate::gpucmp::Fmt::R11G11B10, c, stored);
                let lo = stored / a[c as usize];
                let hi = (stored + q) / a[c as usize];
                let cell = &mut cells[c as usize][byte];
                cell.0 = cell.0.max(lo);
                cell.1 = cell.1.min(hi);
                cell.2 += 1;
            }
        }
    }
    cells
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_issue_order_is_a_permutation_of_the_128_set() {
        let mut seen = [false; 128];
        for &i in SWEEP1_ISSUE_ORDER_128.iter() {
            assert!(!seen[i as usize], "set index {i} issued twice");
            seen[i as usize] = true;
        }
        assert!(seen.iter().all(|&b| b));
    }

    #[test]
    fn the_first_captured_directions_of_sweep_1_are_the_rotated_128_set_points() {
        let Ok(ps) = PointSets::load(&crate::dome::default_path()) else { return }; // the table is banked outside the repo
        let rot = rotate_set(ps.set(128).unwrap());
        // sweep_direction_index 1..5 of capture pwc6 (frame 7533): PeelDirInW as captured
        let captured = [[-0.9900975823402405f32, -0.13565689325332642, 0.03611139580607414], [-0.8956512808799744, 0.37530019879341125, 0.23865997791290283], [-0.8171971440315247, 0.23677007853984833, 0.5254794955253601], [-0.9098978042602539, -0.2575885057449341, 0.32516777515411377], [-0.9827044010162354, 0.18498900532722473, -0.00843580812215805]];
        for (k, d) in captured.iter().enumerate() {
            let (i, a) = nearest(*d, &rot);
            assert_eq!(i, SWEEP1_ISSUE_ORDER_128[k + 1] as usize, "index {}", k + 1);
            assert!(a < 0.035, "index {}: {a}° off the table point", k + 1);
        }
    }

    #[test]
    fn dominant_axis_labels() {
        assert_eq!(dominant_axis([-0.99, -0.14, 0.04]), "-x");
        assert_eq!(dominant_axis([0.1, 0.9, 0.3]), "+y");
        assert_eq!(dominant_axis([0.3, 0.1, -0.9]), "-z");
    }
}

// ───────────────────────────── lmtool sweep1-check ─────────────────────────────

/// `lmtool sweep1-check ROOT [--kappa K] [--fit-srgb]`: the sweep-1 ILightInput chain from the captured
/// sweep-0 end (the H-basis C0 after direction 255, frame 7533), the MDiffuse atlas and the coverage
/// mask (frame 127448), compared R11G11B10 for R11G11B10 with the captured texture the sweep-1 peels
/// sample (frame 7534, eid 32).
pub fn check_ilightinput(root: &std::path::Path, kappa: f32, fit: bool, mask_from_c0: bool) -> Result<(), String> {
    let txt = std::fs::read_to_string(root.join("MANIFEST.json")).map_err(|e| format!("MANIFEST.json: {e}"))?;
    let m = crate::passdiff::read_manifest(&txt)?;
    let find = |pass: &str, frame: u32, pred: &dyn Fn(&crate::passdump::Entry) -> bool| -> Result<crate::passdump::Entry, String> {
        m.passes.iter().find(|e| e.pass == pass && e.frame == Some(frame) && pred(e)).cloned().ok_or_else(|| format!("no {pass} entry for frame {frame}"))
    };
    // the sweep-0 end: hbasis0 with the last issue index of sweep 0
    let last0 = m.passes.iter().filter(|e| e.pass == "hbasis0" && e.sweep == Some(0) && e.banked != Some(false)).max_by_key(|e| e.sweep_direction_index.unwrap_or(0)).cloned().ok_or("no banked sweep-0 hbasis0 snapshot")?;
    println!("sweep-0 end: {} (issue index {:?}, frame {:?})", last0.file, last0.sweep_direction_index, last0.frame);
    let c0 = crate::passdiff::load_entry(root, &last0)?;
    let md = find("setup_ps17043", 127448, &|e| e.file.contains("_16969"))?;
    let mdiffuse8 = crate::passdiff::load_entry(root, &md)?;
    let cov_e = find("setup_ps1038", 127448, &|_| true)?;
    let sun_mask = crate::passdiff::load_entry(root, &cov_e)?;
    // the chain's own mask (PS 1038 alpha of the RESOLVED C0, frame 7533 eid 14177) unless --mask-sun asks for
    // sweep 0's mask 17104
    let coverage: Option<Buf> = if mask_from_c0 { println!("coverage mask: sweep 0's 17104 (the sun_direct alpha)"); Some(sun_mask.clone()) } else { None };
    let target_e = m.passes.iter().filter(|e| e.pass == "ilightinput" && e.frame == Some(7534)).min_by_key(|e| e.eid.unwrap_or(u64::MAX)).cloned().ok_or("no ilightinput entry for frame 7534")?;
    let target = crate::passdiff::load_entry(root, &target_e)?;
    println!("inputs: C0 {}×{} (alpha = Σ 1/N), MDiffuse {} ({}×{} ×{}); target {} ({}×{})", c0.w, c0.h, md.file, mdiffuse8.w, mdiffuse8.h, mdiffuse8.channels, target_e.file, target.w, target.h);
    // a covered texel keeps its own value through the dilation (PS 1335: coverage ≥ 1e-4 → o0 = the input);
    // the uncovered ones are the dilated gutter
    // the chain's mask: 1 where the resolved alpha ≥ 0.01 (a covered texel keeps its own value through PS 1335)
    let resolved_mask = { let r = crate::finalprep::resolve_ps25113(&c0, false, crate::gpufmt::Rounding::Truncate); let mut m8 = Buf::new(c0.w, c0.h, 1); for y in 0..c0.h { for x in 0..c0.w { m8.set(x, y, 0, crate::ilightin::unorm8_rt(r.get(x, y, 3), crate::gpuenc::UnormRounding::NearestEven)); } } m8 };
    let mask_ref: &Buf = coverage.as_ref().unwrap_or(&resolved_mask);
    let interior = |x: u32, y: u32| -> bool { !(mask_ref.get(x, y, 0) < 0.0001) };
    let run = |table: Option<&[[f32; 256]; 3]>, label: &str| {
        let lin = mdiffuse_linear(&mdiffuse8, table);
        let t0 = std::time::Instant::now();
        let ours = ilightinput_from_c0(&c0, &lin, coverage.as_ref(), kappa);
        let all = crate::gpucmp::compare(&ours, &target, 3, crate::gpucmp::Fmt::R11G11B10);
        let inner = crate::gpucmp::compare_where(&ours, &target, 3, crate::gpucmp::Fmt::R11G11B10, &|x, y| interior(x, y));
        let edge = crate::gpucmp::compare_where(&ours, &target, 3, crate::gpucmp::Fmt::R11G11B10, &|x, y| !interior(x, y) && (target.get(x, y, 0) != 0.0 || target.get(x, y, 1) != 0.0 || target.get(x, y, 2) != 0.0 || ours.get(x, y, 0) != 0.0));
        println!("{label} (κ = {kappa}, {:.1} s):\n  all texels:        {}\n  covered texels:    {}\n  dilated gutter:    {}", t0.elapsed().as_secs_f32(), all.line(), inner.line(), edge.line());
    };
    run(None, "IEC sRGB decode");
    if fit {
        let cells = fit_srgb_table(&c0, &mdiffuse8, mask_ref, &target, kappa);
        let mut table = [[0f32; 256]; 3];
        let mut inconsistent = 0;
        let mut fitted = 0;
        let mut off_iec = 0;
        for c in 0..3 {
            for b in 0..256 {
                let iec = crate::gpufmt::srgb_to_linear(b as f32 / 255.0);
                let (lo, hi, n) = cells[c][b];
                if n == 0 { table[c][b] = iec; continue; }
                fitted += 1;
                if lo >= hi { inconsistent += 1; table[c][b] = iec; continue; }
                table[c][b] = if lo <= iec && iec < hi { iec } else { off_iec += 1; 0.5 * (lo + hi) };
                if n >= 1000 || !(lo <= iec && iec < hi) {
                    println!("  byte {b:3} ch {c}: {n:6} texels, L ∈ [{lo:.6}, {hi:.6}) — IEC {iec:.6} {}", if lo <= iec && iec < hi { "inside" } else if iec < lo { "LOW" } else { "HIGH" });
                }
            }
        }
        println!("GPU sRGB table fit: {fitted} (channel, byte) cells observed, {inconsistent} with an empty interval, {off_iec} where the IEC value falls outside (the table's value taken as the interval's midpoint)");
        run(Some(&table), "fitted GPU sRGB decode");
    }
    Ok(())
}

// ───────────────────────────── lmtool sweep1-check --direction K ─────────────────────────────

/// The RenderDoc ids of capture pwc6 (frames 7529–7537): the accumulate PS, its fitted-frustum VS, the
/// H-basis PS, the probe PS (SetILightDir), the peel PSs.
pub struct CaptureIds {
    pub accumulate_ps: &'static [&'static str],
    pub fitted_vs: &'static [&'static str],
    pub hbasis_ps: &'static str,
    pub probe_ps: &'static [&'static str],
}

pub const PWC6_IDS: CaptureIds = CaptureIds { accumulate_ps: &["8507"], fitted_vs: &["8510"], hbasis_ps: "8517", probe_ps: &["8546"] };
pub const PWC2_IDS: CaptureIds = CaptureIds { accumulate_ps: &["17112"], fitted_vs: &["17115"], hbasis_ps: "17122", probe_ps: &["17151"] };

/// One sweep-1 direction end to end on the captured inputs (frame `frame` of pwc6, the direction whose
/// H-basis snapshot carries `sweep_direction_index == index`): every accumulate block on the captured
/// layer colour + depth (E's `lmaccum`, chained from the cleared target) against the captured
/// `TMapILightDir` snapshots, every probe draw (`probepass`) against the captured probe volumes, and the
/// H-basis block added onto the captured MRTs after the previous direction against the captured MRTs
/// after this one.
pub fn check_direction(root: &std::path::Path, frame: u32, index: u32, env_frame: u32, ids: &CaptureIds, capture: &str) -> Result<(), String> {
    use crate::lmaccum::*;
    let t0 = std::time::Instant::now();
    let manifest = root.join("MANIFEST.json");
    let sc = load_lm_scene(root, env_frame)?;
    let entries = load_capture_entries(&manifest)?;
    let draws = load_draws(root, frame)?;
    let hb0: Vec<&CapEntry> = entries.iter().filter(|e| e.pass == "hbasis0" && e.capture == capture && e.banked).collect();
    let target = hb0.iter().find(|e| e.sweep_direction_index == Some(index) && e.frame == frame).copied().ok_or_else(|| format!("no banked hbasis0 with sweep_direction_index {index} in frame {frame}"))?;
    let prev = hb0.iter().filter(|e| e.sweep_direction_index == Some(index.wrapping_sub(1))).max_by_key(|e| (e.frame, e.eid_last)).copied();
    let (hcb, raster) = target.hb_constants().ok_or("the hbasis entry carries no constants")?;
    println!("frame {frame} ({capture}), direction index {index}: D ({:.5}, {:.5}, {:.5}) InvDirCount {} (N = {}), H-basis at eid {}; previous direction's MRTs: {}", hcb.peel_dir[0], hcb.peel_dir[1], hcb.peel_dir[2], hcb.inv_dir_count, (1.0 / hcb.inv_dir_count).round(), target.eid_last, prev.map(|p| format!("index {} frame {} eid {}", index - 1, p.frame, p.eid_last)).unwrap_or_else(|| "none banked".into()));
    // the direction's eid range: from the previous H-basis block of the frame (the log's PS draws, banked or not;
    // the frame start when the direction began in the previous frame) up to this direction's H-basis block
    let range_hi = target.eid_last;
    let mut hb_eids: Vec<u64> = draws.iter().filter(|e| e.pointer("/Pixel/shader").and_then(|v| v.as_str()) == Some(ids.hbasis_ps)).filter_map(|e| e["eid"].as_u64()).collect();
    hb_eids.sort_unstable();
    let block_first = hb_eids.iter().rev().filter(|&&x| x <= range_hi).take(4).last().copied().unwrap_or(range_hi);
    let range_lo = hb_eids.iter().filter(|&&x| x < block_first).max().copied().unwrap_or(0);
    let hb_draws: Vec<&serde_json::Value> = draws.iter().filter(|e| e.pointer("/Pixel/shader").and_then(|v| v.as_str()) == Some(ids.hbasis_ps) && e["eid"].as_u64().map(|x| x >= block_first && x <= range_hi).unwrap_or(false)).collect();
    println!("  {} H-basis draws in the range ({}..={}); {} accumulate blocks; {} probe draws", hb_draws.len(), range_lo, range_hi, set_blocks_with(&draws, &sc, ids.accumulate_ps, ids.fitted_vs)?.iter().filter(|b| b.eid_first > range_lo && b.eid_last < range_hi).count(), draws.iter().filter(|e| e.pointer("/Pixel/shader").and_then(|v| v.as_str()).map(|s| ids.probe_ps.contains(&s)).unwrap_or(false) && e["eid"].as_u64().map(|x| x > range_lo && x < range_hi).unwrap_or(false)).count());
    let layers: Vec<&CapEntry> = entries.iter().filter(|e| (e.pass == "peel_color" || e.pass == "peel_depth") && e.frame == frame && e.eid_last > range_lo && e.eid_last < range_hi).collect();
    let ild: Vec<&CapEntry> = entries.iter().filter(|e| e.pass == "ilightdir" && e.frame == frame && e.eid_last > range_lo && e.eid_last < range_hi).collect();
    let probes: Vec<&CapEntry> = entries.iter().filter(|e| e.pass == "probe3d_world" && e.frame == frame && e.eid_last > range_lo && e.eid_last < range_hi).collect();
    println!("  manifest: {} layer buffers, {} ilightdir snapshots, {} probe volumes in the range ({:.1} s)", layers.len(), ild.len(), probes.len(), t0.elapsed().as_secs_f32());
    if layers.is_empty() {
        return Err("no peel_color/peel_depth entries of this direction in the manifest (the layers are not exported yet)".into());
    }
    let layer_before = |eid: u64, pass: &str| -> Option<&CapEntry> { layers.iter().filter(|e| e.pass == pass && e.eid_last < eid).max_by_key(|e| e.eid_last).copied() };
    // ── the probe draws (world phase) ──
    let probe_state: Option<serde_json::Value> = std::fs::read_to_string(root.join(format!("probe3d/frame{frame}/samplers-scissors.json"))).ok().and_then(|t| serde_json::from_str(&t).ok());
    let offsets_e = entries.iter().find(|e| e.pass == "probe_safety_offset" && e.frame == frame).or_else(|| entries.iter().find(|e| e.pass == "probe_safety_offset"));
    let offsets = match offsets_e { Some(e) => Some(crate::probepass::load_dds_volume(&crate::passdiff::read_entry_bytes(root, &e.file)?, crate::probepass::VolFmt::from_name(&e.format), 32)?), None => None };
    let mut probe_target = crate::probepass::Volume3::new(32, 16, 32, 4);
    let mut probe_closed = (0usize, 0usize);
    let mut cache: Option<(u64, u64, Buf, Buf)> = None; // (colour eid, depth eid, colour, depth)
    let mut load_layer = |eid: u64| -> Result<Option<(Buf, Buf, String)>, String> {
        let (Some(ce), Some(de)) = (layer_before(eid, "peel_color"), layer_before(eid, "peel_depth")) else { return Ok(None) };
        if let Some((c, d, _, _)) = &cache { if *c == ce.eid_last && *d == de.eid_last { let (_, _, cb, db) = cache.as_ref().unwrap(); return Ok(Some((cb.clone(), db.clone(), ce.file.clone()))); } }
        let cb = ce.load(root)?; let db = de.load(root)?;
        cache = Some((ce.eid_last, de.eid_last, cb.clone(), db.clone()));
        Ok(Some((cb, db, ce.file.clone())))
    };
    for a in draws.iter().filter(|e| e.pointer("/Pixel/shader").and_then(|v| v.as_str()).map(|s| ids.probe_ps.contains(&s)).unwrap_or(false) && e["eid"].as_u64().map(|x| x > range_lo && x < range_hi).unwrap_or(false)) {
        let eid = a["eid"].as_u64().unwrap_or(0);
        let sc_rect = crate::probecheck::scissor_for(probe_state.as_ref(), eid).unwrap_or(Some([22, 4, 7, 8]));
        let Some(d) = crate::probecheck::probe_draw_from_action(a, sc_rect) else { continue };
        let Some((cb, db, cname)) = load_layer(eid)? else { println!("  probe draw eid {eid}: no layer before it"); continue };
        let written = crate::probepass::probe_set_ilightdir(&mut probe_target, &d, &crate::lmaccum::LayerTargets { color: &cb, depth: &db }, offsets.as_ref(), crate::probepass::ProbeOpts::default());
        match probes.iter().find(|e| e.eid_last == eid) {
            Some(pe) => {
                let theirs = crate::probepass::load_dds_volume(&crate::passdiff::read_entry_bytes(root, &pe.file)?, crate::probepass::VolFmt::from_name(&pe.format), 32)?;
                let r = crate::probepass::compare_volumes(&probe_target, &theirs, 4);
                if r.closed() { probe_closed.0 += 1; } else { probe_closed.1 += 1; }
                println!("  probe draw eid {eid} (layer {}): {written} probes written, {} non-zero — {} {}", cname.rsplit('/').next().unwrap_or(""), probe_target.count_nonzero(), if r.closed() { "CLOSED" } else { "OPEN" }, r.line());
            }
            None => println!("  probe draw eid {eid}: {written} probes written (no captured volume)"),
        }
    }
    println!("  probe draws: {} closed, {} open", probe_closed.0, probe_closed.1);
    // ── the accumulate blocks, chained from the cleared target ──
    let blocks: Vec<SetBlock> = set_blocks_with(&draws, &sc, ids.accumulate_ps, ids.fitted_vs)?.into_iter().filter(|b| b.eid_first > range_lo && b.eid_last < range_hi).collect();
    let mut chained = DirTarget::cleared(2048, 2048);
    let mut totals = (0usize, 0usize, 0usize, 0usize);
    let mut blocks_done = 0;
    for (bi, b) in blocks.iter().enumerate() {
        let Some((cb, db, cname)) = load_layer(b.eid_first)? else { println!("  block {bi:2} eids {}-{}: no captured layer before it — stopping the chain", b.eid_first, b.eid_last); break };
        let layer = LayerTargets { color: &cb, depth: &db };
        let sx = b.draws[0].cb.world_pw01_shadow[0][0].abs().max(b.draws[0].cb.world_pw01_shadow[2][0].abs());
        run_set_block(&sc.meshes, &sc.instances, &sc.table, &b.draws, &layer, DepthCompare::Unorm16Round, &mut chained);
        blocks_done += 1;
        match ild.iter().find(|e| e.eid_last == b.eid_last || (e.eid_first <= b.eid_first && b.eid_last <= e.eid_last)) {
            Some(snap) => {
                let game = snap.load(root)?;
                let c = compare_dir(&chained, &game);
                totals.0 += c.touched; totals.1 += c.exact; totals.2 += c.quantum; totals.3 += c.worse;
                println!("  block {bi:2} eids {:5}-{:5} {} layer {} chained: {}", b.eid_first, b.eid_last, if sx < 2e-3 { "world " } else { "fitted" }, cname.rsplit('/').next().unwrap_or(""), fmt_dircmp(&c));
            }
            None => println!("  block {bi:2} eids {:5}-{:5} {} layer {} (no captured ilightdir)", b.eid_first, b.eid_last, if sx < 2e-3 { "world " } else { "fitted" }, cname.rsplit('/').next().unwrap_or("")),
        }
    }
    let pct = |n: usize| if totals.0 > 0 { 100.0 * n as f64 / totals.0 as f64 } else { 0.0 };
    println!("  accumulate: {blocks_done} of {} blocks run; over the compared snapshots touched {} exact {} ({:.3} %) ±1 quantum {} ({:.3} %) worse {} ({:.3} %)", blocks.len(), totals.0, totals.1, pct(totals.1), totals.2, pct(totals.2), totals.3, pct(totals.3));
    // ── the H-basis ──
    if blocks_done == blocks.len() {
        let Some(p) = prev else { println!("  H-basis: the previous direction's MRTs are not banked — skipped"); return Ok(()) };
        let mrt_prev: Vec<Buf> = (0..4).map(|m| entries.iter().find(|e| e.pass == format!("hbasis{m}") && e.capture == capture && e.frame == p.frame && e.eid_last == p.eid_last && e.banked).ok_or_else(|| format!("hbasis{m} after direction {} not banked", index - 1)).and_then(|e| e.load(root))).collect::<Result<_, _>>()?;
        let mrt_after: Vec<Buf> = (0..4).map(|m| entries.iter().find(|e| e.pass == format!("hbasis{m}") && e.capture == capture && e.frame == frame && e.eid_last == target.eid_last && e.banked).ok_or_else(|| format!("hbasis{m} after direction {index} not banked")).and_then(|e| e.load(root))).collect::<Result<_, _>>()?;
        let mut tgt = HbTargets::from_bufs([&mrt_prev[0], &mrt_prev[1], &mrt_prev[2], &mrt_prev[3]]);
        let hbd: Vec<HbDraw> = (0..4).map(|m| HbDraw { eid: target.eid_last, mesh: m, instance_first: sc.inst_first[m], instance_count: sc.inst_count[m], raster, cb: hcb }).collect();
        run_hbasis(&sc.meshes, &sc.instances, &sc.table, &hbd, &chained, &mut tgt, crate::sunpass::BlendModel::TruncSrcRoundSum);
        for m in 0..4 {
            for ch in 0..4 {
                let (n, exact, ulp1, worse, maxd, worst) = compare_mrt(&tgt.mrt[m], &mrt_after[m], ch);
                let pc = |v: usize| if n > 0 { 100.0 * v as f64 / n as f64 } else { 0.0 };
                println!("  H-basis C{m}.{}: {n:>8} values  exact {exact:>8} ({:6.2} %)  1 ulp {ulp1:>7} ({:5.2} %)  worse {worse:>6} ({:5.3} %)  max |Δ| {maxd:.6} at ({},{}) game {:.6} ours {:.6}", ["r", "g", "b", "a"][ch], pc(exact), pc(ulp1), pc(worse), worst.0, worst.1, worst.2, worst.3);
            }
        }
    }
    println!("done in {:.1} s", t0.elapsed().as_secs_f32());
    Ok(())
}

/// The GPU's sRGB→linear table as the capture bounds it (the intervals of `fit_srgb_table` on pwc6's sweep-0 end
/// C0 × the MDiffuse against the captured sweep-1 ILightInput; a cell the capture does not observe, or whose
/// interval is empty, keeps the IEC value; an observed cell whose interval excludes the IEC value takes the
/// interval's midpoint). Returns the table and (observed, off-IEC) counts.
pub fn fitted_srgb_table(root: &std::path::Path) -> Result<([[f32; 256]; 3], usize, usize), String> {
    let txt = std::fs::read_to_string(root.join("MANIFEST.json")).map_err(|e| format!("MANIFEST.json: {e}"))?;
    let m = crate::passdiff::read_manifest(&txt)?;
    let last0 = m.passes.iter().filter(|e| e.pass == "hbasis0" && e.sweep == Some(0) && e.banked != Some(false)).max_by_key(|e| e.sweep_direction_index.unwrap_or(0)).cloned().ok_or("no banked sweep-0 hbasis0 snapshot")?;
    let c0 = crate::passdiff::load_entry(root, &last0)?;
    let md = m.passes.iter().find(|e| e.pass == "setup_ps17043" && e.frame == Some(127448) && e.file.contains("_16969")).cloned().ok_or("no setup_ps17043 16969 entry")?;
    let mdiffuse8 = crate::passdiff::load_entry(root, &md)?;
    let target_e = m.passes.iter().filter(|e| e.pass == "ilightinput" && e.frame == Some(7534)).min_by_key(|e| e.eid.unwrap_or(u64::MAX)).cloned().ok_or("no ilightinput entry for frame 7534")?;
    let target = crate::passdiff::load_entry(root, &target_e)?;
    let resolved_mask = {
        let r = crate::finalprep::resolve_ps25113(&c0, false, crate::gpufmt::Rounding::Truncate);
        let mut m8 = Buf::new(c0.w, c0.h, 1);
        for y in 0..c0.h { for x in 0..c0.w { m8.set(x, y, 0, crate::ilightin::unorm8_rt(r.get(x, y, 3), crate::gpuenc::UnormRounding::NearestEven)); } }
        m8
    };
    let cells = fit_srgb_table(&c0, &mdiffuse8, &resolved_mask, &target, 0.3989423);
    let mut table = [[0f32; 256]; 3];
    let (mut observed, mut off_iec) = (0usize, 0usize);
    for c in 0..3 {
        for b in 0..256 {
            let iec = crate::gpufmt::srgb_to_linear(b as f32 / 255.0);
            let (lo, hi, n) = cells[c][b];
            table[c][b] = iec;
            if n == 0 || lo >= hi { continue; }
            observed += 1;
            if !(lo <= iec && iec < hi) { off_iec += 1; table[c][b] = 0.5 * (lo + hi); }
        }
    }
    Ok((table, observed, off_iec))
}
