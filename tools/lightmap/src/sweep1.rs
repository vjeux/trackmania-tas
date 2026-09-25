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

/// Sweep 1's issue order at quality 3 (N = 128) as indices into the rotated 128-set, in the order
/// the game draws them (sweep_direction_index 0..127). The last entry was not captured (the capture
/// ends at index 126); it is the one set point no other index took (set index 0) — marked inferred.
pub const SWEEP1_ISSUE_ORDER_128: [u8; 128] = [
    28, 69, 116, 49, 114, 98, 9, 48, 18, 124, 6, 55, 17, 96, 33, 75, 2, 89, 8, 93, 83, 120, 57, 11, 58, 46, 44, 104, 107, 24, 3, 76, 88, 82, 63, 22, 86, 100, 59, 72, 118, 53, 106, 56, 111, 78, 4, 115, 105, 91, 43, 20, 99, 23, 51, 95, 70, 47, 40, 81, 19, 29, 85, 12, 36, 113, 65, 52, 38, 15, 112, 34, 41, 125, 126, 5, 122, 7, 90, 35, 79, 108, 64, 77, 61, 66, 121, 50, 27, 97, 74, 32, 127, 94, 1, 84, 42, 119, 68, 67, 117, 31, 101, 45, 37, 87, 123, 26, 10, 25, 60, 16, 92, 109, 73, 54, 103, 110, 39, 102, 21, 14, 13, 80, 30, 62, 71,
    0, // inferred: the only set index the 127 captured directions leave
];

/// How many entries of `SWEEP1_ISSUE_ORDER_128` were read off the capture (the rest inferred).
pub const SWEEP1_ISSUE_ORDER_CAPTURED: usize = 127;

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
///    Σ 1/N of the sweep), stored RGBA16F (truncation);
/// 2. × `kappa` (1/√(2π)), stored R11G11B10 (truncation);
/// 3. × the linear MDiffuse (PS 1109, blend DstColor·Src), stored R11G11B10 (truncation);
/// 4. `ilightin::dilate_ps1335` × 8 with the coverage mask (the chart interiors keep their value).
///
/// Interior texels reproduce the captured 8490 of frame 7534 exactly up to the GPU's sRGB table
/// (`lmtool sweep1-check --ilightinput`); the draw sequence between the last sweep-0 direction and the
/// first sweep-1 one (frame 7533) is what places κ — the numbers are the same either way.
pub fn ilightinput_from_c0(c0: &Buf, mdiffuse_lin: &Buf, coverage: &Buf, kappa: f32) -> Buf {
    let resolved = crate::finalprep::resolve_ps25113(c0, false, crate::gpufmt::Rounding::Truncate);
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
        // sweep_direction_index 0..4 of capture pwc6 (frame 7533): PeelDirInW as captured
        let captured = [[-0.9900975823402405f32, -0.13565689325332642, 0.03611139580607414], [-0.8956512808799744, 0.37530019879341125, 0.23865997791290283], [-0.8171971440315247, 0.23677007853984833, 0.5254794955253601], [-0.9098978042602539, -0.2575885057449341, 0.32516777515411377], [-0.9827044010162354, 0.18498900532722473, -0.00843580812215805]];
        for (k, d) in captured.iter().enumerate() {
            let (i, a) = nearest(*d, &rot);
            assert_eq!(i, SWEEP1_ISSUE_ORDER_128[k] as usize, "index {k}");
            assert!(a < 0.03, "index {k}: {a}° off the table point");
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
    let mut coverage = crate::passdiff::load_entry(root, &cov_e)?;
    if mask_from_c0 {
        // the mask recomputed from the C0 target's alpha: PS 1038 with ColorMat4 = the alpha column → R8_UNORM (RTNE)
        let mut m8 = Buf::new(c0.w, c0.h, 1);
        for y in 0..c0.h { for x in 0..c0.w { m8.set(x, y, 0, crate::ilightin::unorm8_rt(c0.get(x, y, 3), crate::gpuenc::UnormRounding::NearestEven)); } }
        coverage = m8;
        println!("coverage mask: PS 1038 of the C0 alpha (not the sweep-0 mask 17104)");
    }
    let target_e = m.passes.iter().filter(|e| e.pass == "ilightinput" && e.frame == Some(7534)).min_by_key(|e| e.eid.unwrap_or(u64::MAX)).cloned().ok_or("no ilightinput entry for frame 7534")?;
    let target = crate::passdiff::load_entry(root, &target_e)?;
    println!("inputs: C0 {}×{} (alpha = Σ 1/N), MDiffuse {} ({}×{} ×{}), coverage {}; target {} ({}×{})", c0.w, c0.h, md.file, mdiffuse8.w, mdiffuse8.h, mdiffuse8.channels, cov_e.file, target_e.file, target.w, target.h);
    // a covered texel keeps its own value through the dilation (PS 1335: coverage ≥ 1e-4 → o0 = the input);
    // the uncovered ones are the dilated gutter
    let interior = |x: u32, y: u32| -> bool { !(coverage.get(x, y, 0) < 0.0001) };
    let run = |table: Option<&[[f32; 256]; 3]>, label: &str| {
        let lin = mdiffuse_linear(&mdiffuse8, table);
        let t0 = std::time::Instant::now();
        let ours = ilightinput_from_c0(&c0, &lin, &coverage, kappa);
        let all = crate::gpucmp::compare(&ours, &target, 3, crate::gpucmp::Fmt::R11G11B10);
        let inner = crate::gpucmp::compare_where(&ours, &target, 3, crate::gpucmp::Fmt::R11G11B10, &|x, y| interior(x, y));
        let edge = crate::gpucmp::compare_where(&ours, &target, 3, crate::gpucmp::Fmt::R11G11B10, &|x, y| !interior(x, y) && (target.get(x, y, 0) != 0.0 || target.get(x, y, 1) != 0.0 || target.get(x, y, 2) != 0.0 || ours.get(x, y, 0) != 0.0));
        println!("{label} (κ = {kappa}, {:.1} s):\n  all texels:        {}\n  covered texels:    {}\n  dilated gutter:    {}", t0.elapsed().as_secs_f32(), all.line(), inner.line(), edge.line());
    };
    run(None, "IEC sRGB decode");
    if fit {
        let cells = fit_srgb_table(&c0, &mdiffuse8, &coverage, &target, kappa);
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
