//! `lmtool passdiff GAME_DIR OURS_DIR [--pass P] [--tol T] [--floor F] [--heat DIR] [--all-heat]
//! [--game-map MAP] [--stride S] [--report FILE.md]` — the per-pass differential between the game's
//! captured render targets and the port's `--dump-passes` intermediates, in pipeline order. Both
//! sides are the MANIFEST.json layout of `passdump.rs`.
//!
//! Per pass: texels compared, max |Δ|, mean |Δ|, RMSE, mean signed Δ (a bias), the percentage of texels
//! within the tolerance (`|a − b| ≤ tol·max(|a|, |b|) + floor`), and the first pass in pipeline order
//! that falls under the pass threshold. The game's TEXEL CONVENTIONS are explicit, NAMED transforms
//! applied before the comparison, each reported on its own line when it is what makes the two sides
//! agree — so a mismatch of convention (mirrored frame, layer order, a missing dome layer, the
//! one-texel inset, reversed depth, the R11G11B10 rounding rule, the ss resolve, a chart size) is
//! reported apart from a mismatch of LIGHT:
//!
//! * `frustum_remap`: a peel-space buffer rendered in another frustum is resampled through the world
//!   (game pixel → world point → our pixel); depths are compared in METRES along the game's forward
//!   axis (`depth_to_metres`), never in z01 units, so the frustum's depth range drops out.
//! * `mirror_x` / `mirror_y` / `transpose`: the orientation of ours that fits the game's peel target.
//! * `layer_order`: the game's k = 0 nearest → reversed; `strip_dome`: the game has no dome layer.
//! * `quantise_r11g11b10_rtne` / `_rtz` / `quantise_f16`: the storage rounding that explains a residual.
//! * `ss_resolve` / `box_down N`: a supersampled target box-averaged to the other side's resolution.
//! * `chart_cut`: the game's atlas cut into charts by its layout (or the baked map's mapping).
//! * `direction_permute`: the game's direction list matched to ours by nearest vector.

use crate::gpufmt::{Quant, Rounding};
use crate::passdump::{ChartRect, Entry, Frustum, Manifest};
use std::collections::HashMap;

/// Close the brackets of a JSON text cut off mid-write (the capture writer streams its manifest): the
/// depth is scanned outside strings, a dangling comma dropped, the missing closers appended.
pub fn repair_truncated_json(txt: &str) -> String {
    let mut stack: Vec<char> = Vec::new();
    let mut in_str = false;
    let mut esc = false;
    let mut last_sig = 0usize;
    for (i, ch) in txt.char_indices() {
        if in_str {
            if esc { esc = false; } else if ch == '\\' { esc = true; } else if ch == '"' { in_str = false; }
            last_sig = i + ch.len_utf8();
            continue;
        }
        match ch {
            '"' => { in_str = true; last_sig = i + 1; }
            '{' => { stack.push('}'); last_sig = i + 1; }
            '[' => { stack.push(']'); last_sig = i + 1; }
            '}' | ']' => { stack.pop(); last_sig = i + 1; }
            c if c.is_whitespace() => {}
            _ => { last_sig = i + ch.len_utf8(); }
        }
    }
    let mut out = txt[..last_sig].to_string();
    if in_str {
        out.push('"');
    }
    // a dangling comma or colon before the cut
    while out.ends_with(',') || out.ends_with(':') {
        out.pop();
        if out.ends_with(':') { out.push_str(":null"); break; }
    }
    // inside an object, a key left without its value (`"key"` or `"key":` at the cut): drop it with its comma
    if stack.last() == Some(&'}') && out.ends_with('"') {
        let trimmed = out.trim_end();
        if let Some(q) = trimmed[..trimmed.len() - 1].rfind('"') {
            let before = trimmed[..q].trim_end();
            if before.ends_with('{') || before.ends_with(',') {
                let mut cut = before.to_string();
                if cut.ends_with(',') { cut.pop(); }
                out = cut;
            }
        }
    }
    while let Some(c) = stack.pop() {
        out.push(c);
    }
    out
}

/// Read a MANIFEST.json (ours or the game's — tolerant of missing fields, of numbers written as
/// strings, of a `dir` given as {x, y, z}, of `layer`/`direction`/`sweep` written as null, and of a
/// file cut off mid-write).
pub fn read_manifest(txt: &str) -> Result<Manifest, String> {
    let mut v: serde_json::Value = match serde_json::from_str(txt) {
        Ok(v) => v,
        Err(e) => {
            // cut off mid-write: close the brackets; if the tail is an unfinished token, back up to the
            // previous line end and try again (a few hundred lines at most)
            let mut cut = txt.len();
            let mut parsed: Option<serde_json::Value> = None;
            for _ in 0..400 {
                let fixed = repair_truncated_json(&txt[..cut]);
                if let Ok(v) = serde_json::from_str(&fixed) {
                    parsed = Some(v);
                    break;
                }
                match txt[..cut.saturating_sub(1)].rfind('\n') {
                    Some(p) if p > 0 => cut = p,
                    _ => break,
                }
            }
            let v = parsed.ok_or_else(|| format!("MANIFEST.json: {e} (and no repair of the cut-off tail parsed)"))?;
            eprintln!("passdiff: MANIFEST.json is cut off ({e}) — read after closing its brackets at byte {cut} of {}", txt.len());
            v
        }
    };
    fn num(v: &mut serde_json::Value) {
        if let Some(s) = v.as_str() {
            if let Ok(n) = s.trim().parse::<f64>() {
                *v = serde_json::json!(n);
            }
        }
    }
    fn vec3(v: &mut serde_json::Value) {
        if let Some(o) = v.as_object() {
            let get = |k: &str| o.get(k).and_then(|x| x.as_f64()).unwrap_or(0.0);
            if o.contains_key("x") && o.contains_key("y") && o.contains_key("z") {
                *v = serde_json::json!([get("x"), get("y"), get("z")]);
            }
        }
        if let Some(a) = v.as_array_mut() {
            for x in a.iter_mut() {
                num(x);
            }
        }
    }
    // a null field is an absent field (the defaults apply)
    fn strip_nulls(v: &mut serde_json::Value) {
        match v {
            serde_json::Value::Object(o) => {
                o.retain(|_, x| !x.is_null());
                for x in o.values_mut() {
                    strip_nulls(x);
                }
            }
            serde_json::Value::Array(a) => {
                for x in a.iter_mut() {
                    strip_nulls(x);
                }
            }
            _ => {}
        }
    }
    strip_nulls(&mut v);
    if let Some(passes) = v.get_mut("passes").and_then(|p| p.as_array_mut()) {
        for e in passes.iter_mut() {
            let Some(o) = e.as_object_mut() else { continue };
            for k in ["sweep", "direction", "layer", "width", "height", "row_pitch"] {
                if let Some(x) = o.get_mut(k) {
                    if x.is_null() {
                        o.remove(k);
                    } else {
                        num(x);
                        match x.as_f64() {
                            // a negative index (the capture's "-1" = none) is no index
                            Some(f) if f < 0.0 => { o.remove(k); }
                            Some(f) => { *x = serde_json::json!(f.round() as i64); }
                            None => {}
                        }
                    }
                }
            }
            if let Some(d) = o.get_mut("dir") { vec3(d); }
            if let Some(f) = o.get_mut("frustum").and_then(|f| f.as_object_mut()) {
                for k in ["center", "half", "right", "up", "forward"] {
                    if let Some(x) = f.get_mut(k) { vec3(x); }
                }
            }
            if let Some(c) = o.get_mut("chart").and_then(|c| c.as_object_mut()) {
                for k in ["obj", "item", "sub"] {
                    if let Some(x) = c.get_mut(k) { num(x); if let Some(f) = x.as_f64() { *x = serde_json::json!(f.round() as i64); } }
                }
            }
            // the peel camera's matrix may sit inside the recorded cbuffers (`WorldPw01Shadow` of the
            // accumulate / sun pass, `GbxV_WorldTo01ShadowLDir0` of the shadow-map pass) rather than at
            // the top level: lift the first non-degenerate 4×4 (or 4×3) found
            if !o.contains_key("view_proj_bias_GbxWorldPw01Shadow") {
                fn find_mat(v: &serde_json::Value, depth: usize) -> Option<serde_json::Value> {
                    if depth > 8 { return None; }
                    if let Some(obj) = v.as_object() {
                        for (k, x) in obj {
                            if k == "WorldPw01Shadow" || k == "GbxWorldPw01Shadow" || k == "GbxV_WorldTo01ShadowLDir0" {
                                if let Some(rows) = x.as_array() {
                                    if rows.len() == 4 {
                                        // degenerate (all-zero linear part) matrices are skipped
                                        let lin: f64 = rows.iter().take(3).filter_map(|r| r.as_array()).flat_map(|r| r.iter().filter_map(|c| c.as_f64())).map(|c| c.abs()).sum();
                                        if lin > 1e-12 {
                                            // a 4×3 (row-vector affine, w column dropped) is padded to 4×4
                                            let padded: Vec<serde_json::Value> = rows.iter().enumerate().map(|(i, r)| { let mut c: Vec<serde_json::Value> = r.as_array().cloned().unwrap_or_default(); while c.len() < 4 { c.push(serde_json::json!(if i == 3 { 1.0 } else { 0.0 })); } serde_json::Value::Array(c) }).collect();
                                            return Some(serde_json::Value::Array(padded));
                                        }
                                    }
                                }
                            }
                            if let Some(f) = find_mat(x, depth + 1) { return Some(f); }
                        }
                    }
                    None
                }
                if let Some(cb) = o.get("cbuffers") {
                    if let Some(mtx) = find_mat(cb, 0) {
                        o.insert("view_proj_bias_GbxWorldPw01Shadow".into(), mtx);
                    }
                }
            }
        }
    }
    if let Some(sweeps) = v.get_mut("sweeps").and_then(|p| p.as_array_mut()) {
        for s in sweeps.iter_mut() {
            if let Some(dirs) = s.get_mut("dirs").and_then(|d| d.as_array_mut()) {
                for d in dirs.iter_mut() { vec3(d); }
            }
            if let Some(x) = s.get_mut("sweep") { num(x); if let Some(f) = x.as_f64() { *x = serde_json::json!(f.round() as i64); } }
            if let Some(x) = s.get_mut("n_dirs") { num(x); if let Some(f) = x.as_f64() { *x = serde_json::json!(f.round() as i64); } }
        }
    }
    if let Some(x) = v.get_mut("sun_dir") { vec3(x); }
    let mut m = serde_json::from_value::<Manifest>(v).map_err(|e| format!("MANIFEST.json: {e}"))?;
    // a capture entry without a `frustum` but with the peel camera's matrix gets its frustum from it
    for e in m.passes.iter_mut() {
        if e.frustum.is_none() {
            if let Some(pw) = &e.pw01 {
                e.frustum = Frustum::from_pw01(pw);
            }
        }
        if e.dir.is_none() {
            if let Some(f) = &e.frustum {
                if e.space == "peel" && e.pass != "sun_shadow" {
                    e.dir = Some(f.forward);
                }
            }
        }
    }
    // the shadow-map pass's own cbuffer may be degenerate in a capture; the direct-sun pass looks the
    // map up through `WorldPw01Shadow`, which is the same frustum
    let sun_fr = m.passes.iter().find(|e| e.pass == "sun_direct" && e.frustum.is_some()).and_then(|e| e.frustum.clone());
    if let Some(fr) = sun_fr {
        for e in m.passes.iter_mut() {
            if e.pass == "sun_shadow" && e.frustum.is_none() {
                e.frustum = Some(fr.clone());
                e.notes = Some(format!("{}frustum taken from the direct-sun pass's WorldPw01Shadow", e.notes.as_deref().map(|n| format!("{n}; ")).unwrap_or_default()));
            }
        }
    }
    // a per-direction entry without a sweep index belongs to the first sweep (the capture's directions
    // are the 256-set's)
    for e in m.passes.iter_mut() {
        if e.sweep.is_none() && e.direction.is_some() && matches!(e.pass.as_str(), "peel_depth" | "peel_color" | "peel_sky" | "ilightdir" | "hbasis0" | "hbasis1" | "hbasis2" | "hbasis3" | "probe" | "probe_aux" | "probe_fold") {
            e.sweep = Some(0);
        }
    }
    // (a capture's entries carry their run's name; our own dump's indices are already one space)
    if m.passes.iter().any(|e| e.capture.is_some()) {
        reindex_directions(&mut m);
    }
    assign_peels(&mut m);
    Ok(m)
}

/// The per-direction peel frustums of a sweep, indexed by direction (from the `peel_depth` entries,
/// else `peel_color`); directions without an entry are filled from the nearest lower direction so
/// the list is dense up to the last captured direction.
/// The game's peels of one direction (ordered by first event id): each distinct frustum among the
/// direction's `peel_depth` / `peel_color` entries is one peel; the entries' `peel` field is filled
/// from it by `read_manifest`.
fn peels_of(m: &Manifest, sweep: u32, direction: u32) -> Vec<Frustum> {
    let mut es: Vec<&Entry> = m.passes.iter().filter(|e| (e.pass == "peel_depth" || e.pass == "peel_color") && e.sweep.unwrap_or(0) == sweep && e.direction == Some(direction) && e.frustum.is_some()).collect();
    es.sort_by_key(|e| e.eid_last.unwrap_or(0));
    let mut out: Vec<Frustum> = Vec::new();
    for e in es {
        let f = e.frustum.as_ref().unwrap();
        if !out.iter().any(|g| same_frustum(g, f)) {
            out.push(f.clone());
        }
    }
    out
}

/// A capture's direction indices are its own (two capture runs both start at 0 with different
/// vectors): give every distinct direction VECTOR one index across the manifest, in order of first
/// appearance, so the harness's (sweep, direction) keys mean one direction. Entries without a vector
/// take the index of a sibling entry of the same (capture, sweep, direction).
pub fn reindex_directions(m: &mut Manifest) {
    let quant = |d: [f32; 3]| -> (i32, i32, i32) { ((d[0] * 2000.0).round() as i32, (d[1] * 2000.0).round() as i32, (d[2] * 2000.0).round() as i32) };
    let mut ids: HashMap<(i32, i32, i32), u32> = HashMap::new();
    let mut vec_of: HashMap<(Option<String>, Option<u32>, u32), [f32; 3]> = HashMap::new();
    for e in &m.passes {
        if let (Some(d), Some(v)) = (e.direction, e.dir) {
            if v != [0.0; 3] { vec_of.entry((e.capture.clone(), e.sweep, d)).or_insert(v); }
        }
    }
    let mut order: Vec<(i32, i32, i32)> = Vec::new();
    for e in &m.passes {
        if let Some(d) = e.direction {
            let v = e.dir.filter(|v| *v != [0.0; 3]).or_else(|| vec_of.get(&(e.capture.clone(), e.sweep, d)).copied());
            if let Some(v) = v { let q = quant(v); if !ids.contains_key(&q) { ids.insert(q, order.len() as u32); order.push(q); } }
        }
    }
    if ids.is_empty() { return; }
    let mut unresolved = 0;
    for e in m.passes.iter_mut() {
        if let Some(d) = e.direction {
            let v = e.dir.filter(|v| *v != [0.0; 3]).or_else(|| vec_of.get(&(e.capture.clone(), e.sweep, d)).copied());
            match v.and_then(|v| ids.get(&quant(v)).copied()) {
                Some(id) => { e.direction = Some(id); if e.dir.is_none() { e.dir = v; } }
                None => unresolved += 1,
            }
        }
    }
    if unresolved > 0 { eprintln!("passdiff: {unresolved} entries with a direction index but no vector kept their capture's index"); }
    // the sweep lists: rebuilt from the vectors in index order
    for sw in m.sweeps.iter_mut() {
        if !sw.dirs.is_empty() {
            let mut dirs: Vec<[f32; 3]> = vec![[0.0; 3]; order.len()];
            for (q, id) in &ids { dirs[*id as usize] = [q.0 as f32 / 2000.0, q.1 as f32 / 2000.0, q.2 as f32 / 2000.0]; }
            sw.dirs = dirs;
            sw.n_dirs = order.len() as u32;
        }
    }
}

/// Fill every peel entry's `peel` index (0 = the first frustum seen for the direction, 1 = the next…).
/// An entry without a frustum (the dome draw's snapshot carries no peel camera) joins the peel that
/// FOLLOWS it in event order — it is that peel's opening sky layer.
pub fn assign_peels(m: &mut Manifest) {
    let keys: std::collections::BTreeSet<(u32, u32)> = m.passes.iter().filter(|e| (e.pass == "peel_depth" || e.pass == "peel_color") && e.direction.is_some()).map(|e| (e.sweep.unwrap_or(0), e.direction.unwrap())).collect();
    for (sw, d) in keys {
        let peels = peels_of(m, sw, d);
        // (eid, peel) of the entries with a frustum, to place the frustum-less ones
        let mut placed: Vec<(u64, u32)> = Vec::new();
        for e in m.passes.iter_mut() {
            if (e.pass == "peel_depth" || e.pass == "peel_color") && e.sweep.unwrap_or(0) == sw && e.direction == Some(d) && e.peel.is_none() {
                if let Some(f) = &e.frustum {
                    e.peel = peels.iter().position(|g| same_frustum(g, f)).map(|i| i as u32);
                    if let Some(p) = e.peel { placed.push((e.eid_last.unwrap_or(0), p)); }
                }
            }
        }
        placed.sort();
        for e in m.passes.iter_mut() {
            if (e.pass == "peel_depth" || e.pass == "peel_color") && e.sweep.unwrap_or(0) == sw && e.direction == Some(d) && e.peel.is_none() && e.frustum.is_none() {
                let eid = e.eid_last.unwrap_or(0);
                e.peel = placed.iter().find(|(x, _)| *x > eid).map(|(_, p)| *p).or_else(|| placed.last().map(|(_, p)| *p));
                // it also takes that peel's frustum (the dome is drawn into the same targets)
                if let Some(p) = e.peel { e.frustum = peels.get(p as usize).cloned(); }
            }
        }
    }
}

/// The game's DOME colours: for every peel of every direction in the capture whose first snapshot
/// is the sky layer (≥ 30 % of its depth at 0), the colour target's central value (the median of a
/// 9×9 block at the frame centre, R11G11B10 as stored) with the direction vector — the sky radiance
/// the game renders along D, as many directions as the capture holds.
pub fn dome_colours(game: &Manifest, root: &std::path::Path) -> Vec<([f32; 3], [f32; 3], u32, u32, Option<Frustum>)> {
    let mut groups: std::collections::BTreeMap<(Option<u32>, Option<u32>, Option<u32>), Vec<usize>> = Default::default();
    for (i, e) in game.passes.iter().enumerate() {
        if e.pass == "peel_depth" && e.direction.is_some() {
            groups.entry((e.sweep, e.direction, e.peel)).or_default().push(i);
        }
    }
    let mut out = Vec::new();
    for (key, idx) in &groups {
        let first = idx.iter().copied().min_by_key(|&i| (game.passes[i].eid_last.unwrap_or(0), game.passes[i].layer.unwrap_or(0))).unwrap();
        let e = &game.passes[first];
        let Ok(b) = load_entry(root, e) else { continue };
        let n = (b.data.len() / 3).max(1);
        let zero_frac = b.data.iter().step_by(3).filter(|v| **v == 0.0).count() as f64 / n as f64;
        if zero_frac < 0.3 {
            continue;
        }
        // the colour snapshot of the same event
        let Some(c) = game.passes.iter().find(|c| c.pass == "peel_color" && (c.sweep, c.direction, c.peel) == *key && c.eid_last == e.eid_last) else { continue };
        let Ok(cb) = load_entry(root, c) else { continue };
        let (cx, cy) = (cb.w / 2, cb.h / 2);
        let mut med = [0f32; 3];
        for ch in 0..3 {
            let mut v: Vec<f32> = Vec::new();
            for y in cy.saturating_sub(4)..(cy + 5).min(cb.h) { for x in cx.saturating_sub(4)..(cx + 5).min(cb.w) { v.push(cb.get(x, y, ch)); } }
            v.sort_by(|a, b| a.partial_cmp(b).unwrap());
            med[ch as usize] = v[v.len() / 2];
        }
        let dir = c.dir.or(e.dir).or_else(|| e.frustum.as_ref().map(|f| f.forward)).unwrap_or([0.0; 3]);
        out.push((dir, med, key.1.unwrap_or(0), key.2.unwrap_or(0), e.frustum.clone().or_else(|| c.frustum.clone())));
    }
    out
}

/// `peel_frustums_for` with our direction list empty: indexed by the game's own direction index.
pub fn peel_frustums(m: &Manifest, sweep: u32) -> Vec<Vec<Frustum>> {
    peel_frustums_for(m, sweep, &[])
}

/// The captured PEELS for OUR direction list: each of our directions takes the ordered peel frustums
/// of the game direction with the nearest VECTOR (the game draws its list in an interleaved order, so
/// the indices differ); without vectors on the game side the indices are matched directly. A direction
/// the capture lacks takes the nearest captured direction's peels re-oriented to our direction (a peel
/// frustum only differs per direction by its orientation and its fitted extents).
pub fn peel_frustums_for(m: &Manifest, sweep: u32, ours: &[[f32; 3]]) -> Vec<Vec<Frustum>> {
    let mut by_dir: HashMap<u32, (Vec<Frustum>, Option<[f32; 3]>)> = HashMap::new();
    for e in &m.passes {
        if (e.pass == "peel_depth" || e.pass == "peel_color") && e.sweep.unwrap_or(0) == sweep {
            if let (Some(d), Some(f)) = (e.direction, &e.frustum) {
                by_dir.entry(d).or_insert_with(|| (peels_of(m, sweep, d), e.dir.or(Some(f.forward))));
            }
        }
    }
    if by_dir.is_empty() {
        return Vec::new();
    }
    // THE FRUSTUM FIT, from the captured frustums (three directions, world + fitted): each peel's frustum
    // is an axis-aligned box projected onto the direction's frame — right = normalize(D × Y), up = right ×
    // D, forward = D; the centre is the box centre, the half-extents Σ_k |axis_k|·H_k. The boxes read off
    // the capture (least squares over the 18 extents, residuals ≤ 1.7 m): the world peel's = (1024.5,
    // 72.0, 1024.5) ± (1025.5, 66.95, 1025.4) — the map's block volume; the fitted peel's = (871.0, 50.7,
    // 353.5) ± (10.8, 45.9, 16.4) — the items' box. (The decompile of the fit is engineer B's row; these
    // numbers stand in until then.) A direction the capture lacks takes them projected onto its frame.
    let reorient = |fs: &Vec<Frustum>, v: [f32; 3], od: [f32; 3]| -> Vec<Frustum> {
        let c = v[0] * od[0] + v[1] * od[1] + v[2] * od[2];
        if c >= 0.999_99 {
            return fs.clone();
        }
        let r = { let x = crate::geometry::cross(od, [0.0, 1.0, 0.0]); if x[0].abs() + x[2].abs() < 1e-6 { [1.0, 0.0, 0.0] } else { crate::geometry::norm(x) } };
        let u = crate::geometry::cross(r, od);
        let boxes: [([f32; 3], [f32; 3]); 2] = [([1024.5, 72.0, 1024.5], [1025.5, 66.95, 1025.4]), ([871.0, 50.7, 353.5], [10.8, 45.9, 16.4])];
        fs.iter()
            .enumerate()
            .map(|(i, f0)| {
                let mut f = f0.clone();
                f.forward = od;
                f.right = r;
                f.up = u;
                // which box: the peel with the larger extents is the world's
                let is_world = f0.half[0] > 500.0;
                let (bc, bh) = if is_world { boxes[0] } else { boxes[1] };
                f.center = bc;
                let ext = |a: [f32; 3]| -> f32 { a[0].abs() * bh[0] + a[1].abs() * bh[1] + a[2].abs() * bh[2] };
                f.half = [ext(r), ext(u), ext(od)];
                f
            })
            .collect()
    };
    if !ours.is_empty() && by_dir.values().all(|(_, v)| v.is_some()) {
        return ours
            .iter()
            .map(|od| {
                let best = by_dir.values().max_by(|a, b| { let ca = a.1.map(|v| v[0] * od[0] + v[1] * od[1] + v[2] * od[2]).unwrap_or(-2.0); let cb = b.1.map(|v| v[0] * od[0] + v[1] * od[1] + v[2] * od[2]).unwrap_or(-2.0); ca.partial_cmp(&cb).unwrap_or(std::cmp::Ordering::Equal) }).unwrap();
                reorient(&best.0, best.1.unwrap(), *od)
            })
            .collect();
    }
    let max = *by_dir.keys().max().unwrap();
    (0..=max)
        .map(|d| {
            let nearest = by_dir.keys().min_by_key(|k| ((**k as i64 - d as i64).abs(), **k)).copied().unwrap();
            by_dir[&nearest].0.clone()
        })
        .collect()
}

/// A decoded buffer: `channels` f32 per pixel, row-major, top row first.
#[derive(Clone, Debug)]
pub struct Buf {
    pub w: u32,
    pub h: u32,
    pub channels: u32,
    pub data: Vec<f32>,
}

impl Buf {
    pub fn new(w: u32, h: u32, channels: u32) -> Buf {
        Buf { w, h, channels, data: vec![0.0; (w * h * channels) as usize] }
    }
    #[inline]
    pub fn get(&self, x: u32, y: u32, c: u32) -> f32 {
        self.data[((y * self.w + x) * self.channels + c) as usize]
    }
    #[inline]
    pub fn set(&mut self, x: u32, y: u32, c: u32, v: f32) {
        self.data[((y * self.w + x) * self.channels + c) as usize] = v;
    }
    /// Crop a pixel rectangle (clamped to the buffer).
    pub fn crop(&self, x0: i64, y0: i64, w: u32, h: u32) -> Buf {
        let mut out = Buf::new(w, h, self.channels);
        for y in 0..h {
            for x in 0..w {
                let (sx, sy) = (x0 + x as i64, y0 + y as i64);
                if sx >= 0 && sy >= 0 && sx < self.w as i64 && sy < self.h as i64 {
                    for c in 0..self.channels {
                        out.set(x, y, c, self.get(sx as u32, sy as u32, c));
                    }
                }
            }
        }
        out
    }
    /// Box-average by an integer factor per axis (the ss resolve: the mean of the non-zero sub-samples
    /// when `weighted`, else the plain mean).
    pub fn box_down(&self, fx: u32, fy: u32, weighted: bool) -> Buf {
        let (w, h) = (self.w / fx.max(1), self.h / fy.max(1));
        let mut out = Buf::new(w, h, self.channels);
        for y in 0..h {
            for x in 0..w {
                for c in 0..self.channels {
                    let (mut s, mut n) = (0.0f64, 0.0f64);
                    for yy in 0..fy {
                        for xx in 0..fx {
                            let v = self.get(x * fx + xx, y * fy + yy, c);
                            let covered = !weighted || (0..self.channels).any(|k| self.get(x * fx + xx, y * fy + yy, k) != 0.0);
                            if covered {
                                s += v as f64;
                                n += 1.0;
                            }
                        }
                    }
                    out.set(x, y, c, if n > 0.0 { (s / n) as f32 } else { 0.0 });
                }
            }
        }
        out
    }
    /// Nearest-neighbour resample to another size.
    pub fn resample(&self, w: u32, h: u32) -> Buf {
        let mut out = Buf::new(w, h, self.channels);
        for y in 0..h {
            let sy = (((y as f32 + 0.5) * self.h as f32 / h as f32) as u32).min(self.h.saturating_sub(1));
            for x in 0..w {
                let sx = (((x as f32 + 0.5) * self.w as f32 / w as f32) as u32).min(self.w.saturating_sub(1));
                for c in 0..self.channels {
                    out.set(x, y, c, self.get(sx, sy, c));
                }
            }
        }
        out
    }
    pub fn mirror_x(&self) -> Buf {
        let mut out = self.clone();
        for y in 0..self.h {
            for x in 0..self.w {
                for c in 0..self.channels {
                    out.set(x, y, c, self.get(self.w - 1 - x, y, c));
                }
            }
        }
        out
    }
    pub fn mirror_y(&self) -> Buf {
        let mut out = self.clone();
        for y in 0..self.h {
            for x in 0..self.w {
                for c in 0..self.channels {
                    out.set(x, y, c, self.get(x, self.h - 1 - y, c));
                }
            }
        }
        out
    }
    pub fn transpose(&self) -> Buf {
        let mut out = Buf::new(self.h, self.w, self.channels);
        for y in 0..self.h {
            for x in 0..self.w {
                for c in 0..self.channels {
                    out.set(y, x, c, self.get(x, y, c));
                }
            }
        }
        out
    }
    /// Apply a per-pixel RGB quantiser (channels ≥ 3; other channels untouched).
    pub fn quantised(&self, q: Quant, r: Rounding) -> Buf {
        let mut out = self.clone();
        if self.channels >= 3 {
            for i in 0..(self.w * self.h) as usize {
                let b = i * self.channels as usize;
                let v = q.apply([self.data[b], self.data[b + 1], self.data[b + 2]], r);
                out.data[b..b + 3].copy_from_slice(&v);
            }
        } else {
            for v in out.data.iter_mut() {
                *v = q.apply([*v, 0.0, 0.0], r)[0];
            }
        }
        out
    }
}

/// The storage format of a raw dump, by its (DXGI / RenderDoc) name.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Fmt {
    F32(u32),
    R11G11B10,
    F16(u32),
    Unorm8(u32),
    Unorm8Srgb(u32),
    Unorm16(u32),
    D24S8,
    /// Unsigned integers, one byte per channel (R8_UINT, R8G8_UINT): the value itself, not k/255.
    Uint8(u32),
    /// B8G8R8A8: four UNORM bytes in memory order B, G, R, A — decoded into R, G, B, A channels.
    Bgra8,
    Unknown,
}

pub fn parse_format(name: &str) -> Fmt {
    let n = name.trim().to_ascii_uppercase();
    let n = n.strip_prefix("DXGI_FORMAT_").unwrap_or(&n).to_string();
    match n.as_str() {
        "R32_FLOAT" | "D32_FLOAT" | "R32_TYPELESS" | "D32_FLOAT_S8X24_UINT" | "R32_FLOAT_X8X24_TYPELESS" => Fmt::F32(1),
        "R32G32_FLOAT" => Fmt::F32(2),
        "R32G32B32_FLOAT" => Fmt::F32(3),
        "R32G32B32A32_FLOAT" => Fmt::F32(4),
        "R11G11B10_FLOAT" => Fmt::R11G11B10,
        "R16_FLOAT" => Fmt::F16(1),
        "R16G16_FLOAT" => Fmt::F16(2),
        "R16G16B16A16_FLOAT" => Fmt::F16(4),
        "R8_UNORM" => Fmt::Unorm8(1),
        "R8G8_UNORM" => Fmt::Unorm8(2),
        "R8G8B8_UNORM" => Fmt::Unorm8(3),
        "R8G8B8A8_UNORM" | "B8G8R8A8_UNORM" | "R8G8B8A8_TYPELESS" => Fmt::Unorm8(4),
        "B8G8R8A8_TYPELESS" => Fmt::Bgra8,
        "R8_UINT" => Fmt::Uint8(1),
        "R8G8_UINT" => Fmt::Uint8(2),
        "R8G8B8A8_UINT" => Fmt::Uint8(4),
        "R8G8B8A8_UNORM_SRGB" | "B8G8R8A8_UNORM_SRGB" => Fmt::Unorm8Srgb(4),
        "R16_UNORM" | "D16_UNORM" | "R16_TYPELESS" => Fmt::Unorm16(1),
        "R16G16B16A16_UNORM" => Fmt::Unorm16(4),
        "D24_UNORM_S8_UINT" | "R24_UNORM_X8_TYPELESS" | "R24G8_TYPELESS" => Fmt::D24S8,
        _ => Fmt::Unknown,
    }
}

fn srgb_to_linear(v: f32) -> f32 {
    if v <= 0.04045 { v / 12.92 } else { ((v + 0.055) / 1.055).powf(2.4) }
}

/// Decode raw target bytes of `w`×`h` pixels with the given row pitch (0 = tight).
pub fn decode_raw(bytes: &[u8], fmt: Fmt, w: u32, h: u32, row_pitch: u32) -> Result<Buf, String> {
    let (bpp, channels) = match fmt {
        Fmt::F32(c) => (4 * c, c),
        Fmt::R11G11B10 => (4, 3),
        Fmt::F16(c) => (2 * c, c),
        Fmt::Unorm8(c) | Fmt::Unorm8Srgb(c) | Fmt::Uint8(c) => (c, c),
        Fmt::Bgra8 => (4, 4),
        Fmt::Unorm16(c) => (2 * c, c),
        Fmt::D24S8 => (4, 1),
        Fmt::Unknown => return Err("unknown format".into()),
    };
    let pitch = if row_pitch == 0 { w * bpp } else { row_pitch } as usize;
    if bytes.len() < pitch * (h as usize - 1) + (w * bpp) as usize {
        return Err(format!("buffer too short: {} B for {w}×{h} × {bpp} B (pitch {pitch})", bytes.len()));
    }
    let mut out = Buf::new(w, h, channels);
    for y in 0..h as usize {
        let row = &bytes[y * pitch..];
        for x in 0..w as usize {
            let px = &row[x * bpp as usize..(x + 1) * bpp as usize];
            let o = (y * w as usize + x) * channels as usize;
            match fmt {
                Fmt::F32(c) => { for k in 0..c as usize { out.data[o + k] = f32::from_le_bytes(px[k * 4..k * 4 + 4].try_into().unwrap()); } }
                Fmt::R11G11B10 => { let v = crate::gpufmt::unpack_r11g11b10(u32::from_le_bytes(px[..4].try_into().unwrap())); out.data[o..o + 3].copy_from_slice(&v); }
                Fmt::F16(c) => { for k in 0..c as usize { out.data[o + k] = crate::gpufmt::decode_f16(u16::from_le_bytes(px[k * 2..k * 2 + 2].try_into().unwrap())); } }
                Fmt::Unorm8(c) => { for k in 0..c as usize { out.data[o + k] = px[k] as f32 / 255.0; } }
                Fmt::Uint8(c) => { for k in 0..c as usize { out.data[o + k] = px[k] as f32; } }
                Fmt::Bgra8 => { out.data[o] = px[2] as f32 / 255.0; out.data[o + 1] = px[1] as f32 / 255.0; out.data[o + 2] = px[0] as f32 / 255.0; out.data[o + 3] = px[3] as f32 / 255.0; }
                Fmt::Unorm8Srgb(c) => { for k in 0..c as usize { out.data[o + k] = if k < 3 { srgb_to_linear(px[k] as f32 / 255.0) } else { px[k] as f32 / 255.0 }; } }
                Fmt::Unorm16(c) => { for k in 0..c as usize { out.data[o + k] = u16::from_le_bytes(px[k * 2..k * 2 + 2].try_into().unwrap()) as f32 / 65535.0; } }
                Fmt::D24S8 => { let v = u32::from_le_bytes(px[..4].try_into().unwrap()) & 0x00ff_ffff; out.data[o] = v as f32 / 16_777_215.0; }
                Fmt::Unknown => unreachable!(),
            }
        }
    }
    Ok(out)
}

/// A DDS file's first mip: (dxgi format id, width, height, row pitch or 0, payload offset).
fn parse_dds(b: &[u8]) -> Result<(u32, u32, u32, u32, usize), String> {
    if b.len() < 128 || &b[..4] != b"DDS " {
        return Err("not a DDS file".into());
    }
    let u = |o: usize| u32::from_le_bytes(b[o..o + 4].try_into().unwrap());
    let flags = u(8);
    let h = u(12);
    let w = u(16);
    let pitch = if flags & 0x8 != 0 { u(20) } else { 0 };
    let pf_flags = u(80);
    let fourcc = &b[84..88];
    let mut off = 128usize;
    let mut dxgi = 0u32;
    if pf_flags & 0x4 != 0 && fourcc == b"DX10" {
        if b.len() < 148 {
            return Err("truncated DX10 header".into());
        }
        dxgi = u(128);
        off = 148;
    } else if pf_flags & 0x4 != 0 {
        dxgi = match fourcc {
            b"\x74\x00\x00\x00" => 2,  // D3DFMT_A32B32G32R32F = 116
            b"\x72\x00\x00\x00" => 41, // D3DFMT_R32F = 114
            b"\x71\x00\x00\x00" => 10, // D3DFMT_A16B16G16R16F = 113
            b"\x6f\x00\x00\x00" => 54, // D3DFMT_R16F = 111
            _ => 0,
        };
    } else if pf_flags & 0x40 != 0 {
        // uncompressed RGB(A) 8-bit
        let bits = u(88);
        dxgi = if bits == 32 { 28 } else if bits == 24 { 0xffff } else if bits == 8 { 61 } else { 0 };
    }
    Ok((dxgi, w, h, pitch, off))
}

/// DXGI format id → Fmt.
pub fn dxgi_fmt(id: u32) -> Fmt {
    match id {
        2 => Fmt::F32(4),
        6 => Fmt::F32(3),
        10 => Fmt::F16(4),
        16 => Fmt::F32(2),
        26 => Fmt::R11G11B10,
        28 | 87 => Fmt::Unorm8(4),
        29 | 91 => Fmt::Unorm8Srgb(4),
        34 => Fmt::F16(2),
        39 | 40 | 41 => Fmt::F32(1),
        44 | 45 | 46 => Fmt::D24S8,
        54 => Fmt::F16(1),
        53 | 55 => Fmt::Unorm16(1),
        56 => Fmt::Unorm16(1),
        61 => Fmt::Unorm8(1),
        50 => Fmt::Uint8(2),
        62 => Fmt::Uint8(1),
        30 => Fmt::Uint8(4),
        90 => Fmt::Bgra8,
        0xffff => Fmt::Unorm8(3),
        _ => Fmt::Unknown,
    }
}

/// The bytes of an entry's file: `file`, or `file.gz` (a gzip member, inflated).
pub fn read_entry_bytes(root: &std::path::Path, file: &str) -> Result<Vec<u8>, String> {
    let p = root.join(file);
    let (bytes, gz) = match std::fs::read(&p) {
        Ok(b) => (b, false),
        Err(e) => {
            let pg = root.join(format!("{file}.gz"));
            match std::fs::read(&pg) {
                Ok(b) => (b, true),
                Err(_) => return Err(format!("{}: {e} (no .gz either)", p.display())),
            }
        }
    };
    if gz || (bytes.len() > 2 && bytes[0] == 0x1f && bytes[1] == 0x8b) {
        return gunzip(&bytes).map_err(|e| format!("{}: {e}", p.display()));
    }
    Ok(bytes)
}

/// Inflate one gzip member (RFC 1952 header, raw deflate body).
pub fn gunzip(b: &[u8]) -> Result<Vec<u8>, String> {
    if b.len() < 18 || b[0] != 0x1f || b[1] != 0x8b || b[2] != 8 {
        return Err("not a gzip member".into());
    }
    let flg = b[3];
    let mut o = 10usize;
    if flg & 4 != 0 {
        let xlen = u16::from_le_bytes([b[o], b[o + 1]]) as usize;
        o += 2 + xlen;
    }
    if flg & 8 != 0 {
        while o < b.len() && b[o] != 0 { o += 1; }
        o += 1;
    }
    if flg & 16 != 0 {
        while o < b.len() && b[o] != 0 { o += 1; }
        o += 1;
    }
    if flg & 2 != 0 {
        o += 2;
    }
    if o >= b.len() {
        return Err("truncated gzip header".into());
    }
    let body = &b[o..b.len() - 8];
    let isize = u32::from_le_bytes(b[b.len() - 4..].try_into().unwrap()) as usize;
    let out = miniz_oxide::inflate::decompress_to_vec(body).map_err(|e| format!("gzip inflate: {e:?}"))?;
    if isize != 0 && out.len() % (1usize << 32) != isize {
        return Err(format!("gzip: inflated {} B, header says {isize}", out.len()));
    }
    Ok(out)
}

/// Decode a DDS file's first mip (DX10 header or a legacy fourcc): the manifest's `format` string wins over the
/// header's when it names a known format; `width`/`height` > 0 override the header's.
pub fn load_dds_bytes(bytes: &[u8], format: &str, width: u32, height: u32) -> Result<Buf, String> {
    let (dxgi, w, h, pitch, off) = parse_dds(bytes)?;
    let fmt = if !format.is_empty() && parse_format(format) != Fmt::Unknown { parse_format(format) } else { dxgi_fmt(dxgi) };
    if fmt == Fmt::Unknown {
        return Err(format!("DDS format {dxgi} not supported"));
    }
    let (w, h) = if width > 0 && height > 0 { (width, height) } else { (w, h) };
    decode_raw(&bytes[off..], fmt, w, h, pitch)
}

/// Load one entry's buffer from its root: a raw dump by the manifest's format, or a DDS (optionally gzipped).
pub fn load_entry(root: &std::path::Path, e: &Entry) -> Result<Buf, String> {
    load_file(root, &e.file, &e.format, e.width, e.height, e.row_pitch)
}

/// `load_entry` by file name: `format` / `width` / `height` / `row_pitch` as the manifest gives them (a DDS
/// header supplies what is missing; a raw dump needs all of them).
pub fn load_file(root: &std::path::Path, file: &str, format: &str, width: u32, height: u32, row_pitch: u32) -> Result<Buf, String> {
    let p = root.join(file);
    let bytes = read_entry_bytes(root, file)?;
    if bytes.len() >= 4 && &bytes[..4] == b"DDS " {
        let (dxgi, w, h, pitch, off) = parse_dds(&bytes)?;
        let fmt = if !format.is_empty() && parse_format(format) != Fmt::Unknown { parse_format(format) } else { dxgi_fmt(dxgi) };
        if fmt == Fmt::Unknown {
            return Err(format!("{}: DDS format {dxgi} not supported", p.display()));
        }
        let (w, h) = if width > 0 && height > 0 { (width, height) } else { (w, h) };
        return decode_raw(&bytes[off..], fmt, w, h, pitch);
    }
    let fmt = parse_format(format);
    if fmt == Fmt::Unknown {
        return Err(format!("{}: format {:?} not supported", p.display(), format));
    }
    if width == 0 || height == 0 {
        return Err(format!("{}: no width/height in the manifest", p.display()));
    }
    decode_raw(&bytes, fmt, width, height, row_pitch)
}

/// The comparison statistics of two equally shaped buffers over the compared channels.
#[derive(Clone, Debug, Default)]
pub struct Stats {
    pub n: usize,
    pub max_abs: f32,
    pub mean_abs: f64,
    pub rmse: f64,
    pub mean_signed: f64,
    pub within: usize,
    /// The mean of |game| over the compared texels — the scale the errors are relative to.
    pub mean_ref: f64,
    /// Where the worst texel is (x, y, channel, game value, our value).
    pub worst: (u32, u32, u32, f32, f32),
    /// Per channel: the mean of the game's values and of ours (the ratio per channel tells a colour cast
    /// from a level difference).
    pub ch_game: [f64; 4],
    pub ch_ours: [f64; 4],
    pub ch_n: [usize; 4],
}

impl Stats {
    pub fn pct_within(&self) -> f64 {
        if self.n == 0 { 0.0 } else { 100.0 * self.within as f64 / self.n as f64 }
    }
}

/// Compare `ours` against `game` (same w, h, channels) over the pixels where `mask` (game, ours) says
/// so; tolerance `|a − b| ≤ tol·max(|a|, |b|) + floor`.
pub fn compare(game: &Buf, ours: &Buf, channels: u32, tol: f32, floor: f32, stride: u32, mask: &dyn Fn(u32, u32) -> bool) -> Stats {
    let mut s = Stats::default();
    let (mut sum_abs, mut sum_sq, mut sum_signed, mut sum_ref) = (0.0f64, 0.0f64, 0.0f64, 0.0f64);
    let ch = channels.min(game.channels).min(ours.channels);
    let stride = stride.max(1);
    let mut y = 0;
    while y < game.h.min(ours.h) {
        let mut x = 0;
        while x < game.w.min(ours.w) {
            if mask(x, y) {
                for c in 0..ch {
                    let (a, b) = (game.get(x, y, c), ours.get(x, y, c));
                    if !a.is_finite() || !b.is_finite() {
                        continue;
                    }
                    let d = b - a;
                    s.n += 1;
                    if (c as usize) < 4 { s.ch_game[c as usize] += a as f64; s.ch_ours[c as usize] += b as f64; s.ch_n[c as usize] += 1; }
                    sum_abs += d.abs() as f64;
                    sum_sq += (d as f64) * (d as f64);
                    sum_signed += d as f64;
                    sum_ref += a.abs() as f64;
                    if d.abs() <= tol * a.abs().max(b.abs()) + floor {
                        s.within += 1;
                    }
                    if d.abs() > s.max_abs {
                        s.max_abs = d.abs();
                        s.worst = (x, y, c, a, b);
                    }
                }
            }
            x += stride;
        }
        y += stride;
    }
    if s.n > 0 {
        s.mean_abs = sum_abs / s.n as f64;
        s.rmse = (sum_sq / s.n as f64).sqrt();
        s.mean_signed = sum_signed / s.n as f64;
        s.mean_ref = sum_ref / s.n as f64;
    }
    s
}

/// The pipeline order of the passes.
pub const PIPELINE: &[&str] = &["lm_pos", "lm_nrm", "mdiffuse", "sun_shadow", "sun_direct", "ilightinput", "peel_sky_depth", "peel_sky", "peel_depth", "peel_color", "ilightdir", "lightsum", "lightsum_resolved", "probe_skyvis", "probe_ilightdir", "probe_isvalid", "hbasis0", "hbasis1", "hbasis2", "hbasis3", "final_hdr", "final_atlas"];

pub fn pipeline_rank(pass: &str) -> usize {
    PIPELINE.iter().position(|p| *p == pass).unwrap_or(PIPELINE.len())
}

/// One compared buffer pair's report row.
#[derive(Clone, Debug)]
pub struct Row {
    pub pass: String,
    pub sweep: Option<u32>,
    pub direction: Option<u32>,
    pub peel: Option<u32>,
    pub layer: Option<u32>,
    pub chart: Option<u32>,
    pub stats: Stats,
    /// The named transforms applied to make the two sides comparable, and convention findings.
    pub transforms: Vec<String>,
    pub note: String,
    /// The two buffers as compared (for the heat dump), when kept.
    pub pair: Option<(Buf, Buf, u32)>,
}

impl Row {
    pub fn key(&self) -> String {
        let mut k = self.pass.clone();
        if let Some(s) = self.sweep { k += &format!(" s{s}"); }
        if let Some(d) = self.direction { k += &format!(" d{d:03}"); }
        if let Some(p) = self.peel { k += &format!(" p{p}"); }
        if let Some(l) = self.layer { k += &format!(" l{l:02}"); }
        if let Some(c) = self.chart { k += &format!(" obj{c}"); }
        k
    }
}

/// The chart rectangles of a side: from its manifest's `layout`, else from a baked map's mapping.
pub fn chart_rects(m: &Manifest, map: Option<&str>) -> Vec<ChartRect> {
    if !m.layout.is_empty() {
        return m.layout.clone();
    }
    let Some(path) = map else { return Vec::new() };
    let Ok(lm) = crate::mapio::load(path) else { eprintln!("passdiff: {path}: no lightmap chunk"); return Vec::new() };
    let Some(d) = lm.chunk.data.as_ref() else { return Vec::new() };
    let Some(mp) = d.cache.mapping() else { return Vec::new() };
    (0..mp.count as usize)
        .map(|i| {
            let (x, y) = mp.pos[i];
            let (w, h) = mp.size[i];
            ChartRect { obj: mp.binds[i].obj_group_idx / 4, item: 0, sub: mp.binds[i].obj_idx, x: x as i32, y: y as i32, w: w as i32, h: h as i32, chart_w: (w as u32) / 2, chart_h: (h as u32) / 2 }
        })
        .collect()
}

/// Cut a chart out of an atlas-space buffer: the chart's stored footprint is texels `(x+1)/2 …` of
/// width `w/2`; a buffer of width `W` over the 1024² stored atlas is at scale `W/1024`.
pub fn cut_chart(atlas: &Buf, r: &ChartRect, stored_w: u32) -> Option<(Buf, u32)> {
    if stored_w == 0 || atlas.w % stored_w != 0 {
        return None;
    }
    let sc = atlas.w / stored_w;
    let (tx0, ty0) = (((r.x + 1) / 2) as i64, ((r.y + 1) / 2) as i64);
    let (tw, th) = ((r.w / 2).max(0) as u32, (r.h / 2).max(0) as u32);
    if tw == 0 || th == 0 {
        return None;
    }
    Some((atlas.crop(tx0 * sc as i64, ty0 * sc as i64, tw * sc, th * sc), sc))
}

/// Bring two chart buffers to the same size: box-average the finer one down by an integer factor
/// (the ss resolve, weighted by coverage), else nearest-resample ours to the game's grid.
fn align_chart(game: Buf, ours: Buf, transforms: &mut Vec<String>) -> (Buf, Buf) {
    if game.w == ours.w && game.h == ours.h {
        return (game, ours);
    }
    if game.w > ours.w && game.w % ours.w == 0 && game.h % ours.h == 0 {
        let (fx, fy) = (game.w / ours.w, game.h / ours.h);
        transforms.push(format!("ss_resolve(game ÷{fx}×{fy})"));
        return (game.box_down(fx, fy, true), ours);
    }
    if ours.w > game.w && ours.w % game.w == 0 && ours.h % game.h == 0 {
        let (fx, fy) = (ours.w / game.w, ours.h / game.h);
        transforms.push(format!("ss_resolve(ours ÷{fx}×{fy})"));
        return (game, ours.box_down(fx, fy, true));
    }
    transforms.push(format!("chart_size_mismatch(game {}×{}, ours {}×{} → nearest)", game.w, game.h, ours.w, ours.h));
    let (w, h) = (game.w, game.h);
    (game, ours.resample(w, h))
}

/// The orientation of `ours` that best fits `game` (identity, mirror x, mirror y, both, transpose):
/// tried on a coarse stride; returns the transformed buffer and the transform's name (None = identity).
fn best_orientation(game: &Buf, ours: &Buf, channels: u32, floor: f32) -> (Buf, Option<String>) {
    let cands: Vec<(&str, Buf)> = vec![("mirror_x", ours.mirror_x()), ("mirror_y", ours.mirror_y()), ("mirror_xy", ours.mirror_x().mirror_y())];
    let all = |_x: u32, _y: u32| true;
    let base = compare(game, ours, channels, 0.02, floor, 8, &all);
    let mut best: Option<(String, Buf, f64)> = None;
    for (name, b) in cands {
        let s = compare(game, &b, channels, 0.02, floor, 8, &all);
        if s.n > 0 && s.rmse < base.rmse * 0.5 && best.as_ref().map(|x| s.rmse < x.2).unwrap_or(true) {
            best = Some((name.to_string(), b, s.rmse));
        }
    }
    if game.w == game.h {
        let t = ours.transpose();
        let s = compare(game, &t, channels, 0.02, floor, 8, &all);
        if s.n > 0 && s.rmse < base.rmse * 0.5 && best.as_ref().map(|x| s.rmse < x.2).unwrap_or(true) {
            best = Some(("transpose".into(), t, s.rmse));
        }
    }
    match best {
        Some((n, b, _)) => (b, Some(n)),
        None => (ours.clone(), None),
    }
}

/// The clear value of a depth target: the entry's `cleared_to` when given, else inferred from the
/// buffer (a reversed-z peel clears to 1.0 = near; a target cleared to 0 shows exact zeros).
pub fn depth_clear(e: &Entry, b: &Buf) -> f32 {
    if let Some(c) = e.cleared_to.as_ref().and_then(|v| v.as_f64()) {
        return c as f32;
    }
    let (mut ones, mut zeros) = (0usize, 0usize);
    for v in b.data.iter().step_by(7) {
        if *v == 1.0 { ones += 1; } else if *v == 0.0 { zeros += 1; }
    }
    if ones > zeros { 1.0 } else { 0.0 }
}

/// Resample a peel-space buffer of ours into the game's pixel grid through the world: for every
/// game pixel centre, the world point on the game's near plane → our pixel (nearest). Depth channels
/// (channels == 1 and `depth`) are converted to metres along the game's forward axis on both sides,
/// a cleared pixel (`clear_g` / `clear_o`) becoming NaN. The third buffer marks (1.0) the game pixels
/// to compare: inside our frame and a surface on BOTH sides; the counts are the one-sided pixels
/// (a surface in the game where ours is clear, and the reverse) — the coverage divergence.
fn remap_peel(game: &Buf, gf: &Frustum, ours: &Buf, of: &Frustum, depth: bool, clear_g: f32, clear_o: f32) -> (Buf, Buf, Buf, (usize, usize)) {
    let mut o2 = Buf::new(game.w, game.h, ours.channels);
    let mut g2 = game.clone();
    let mut valid = Buf::new(game.w, game.h, 1);
    let (mut only_g, mut only_o) = (0usize, 0usize);
    let dot = |a: [f32; 3], b: [f32; 3]| a[0] * b[0] + a[1] * b[1] + a[2] * b[2];
    for y in 0..game.h {
        for x in 0..game.w {
            let pw = gf.unproject(x as f32 + 0.5, y as f32 + 0.5, 0.5, game.w, game.h);
            let (ox, oy, _) = of.project(pw, ours.w, ours.h);
            let (oxi, oyi) = (ox.floor() as i64, oy.floor() as i64);
            if oxi < 0 || oyi < 0 || oxi >= ours.w as i64 || oyi >= ours.h as i64 {
                if depth { g2.set(x, y, 0, f32::NAN); }
                continue;
            }
            if depth {
                let (zg, zo) = (game.get(x, y, 0), ours.get(oxi as u32, oyi as u32, 0));
                let (cg, co) = (zg == clear_g, zo == clear_o);
                match (cg, co) {
                    (true, true) => { g2.set(x, y, 0, f32::NAN); o2.set(x, y, 0, f32::NAN); }
                    (true, false) => { only_o += 1; g2.set(x, y, 0, f32::NAN); o2.set(x, y, 0, f32::NAN); }
                    (false, true) => { only_g += 1; g2.set(x, y, 0, f32::NAN); o2.set(x, y, 0, f32::NAN); }
                    (false, false) => {
                        // both depths → the world point's coordinate along the game's forward axis (metres)
                        let po = of.unproject(oxi as f32 + 0.5, oyi as f32 + 0.5, zo, ours.w, ours.h);
                        let pg = gf.unproject(x as f32 + 0.5, y as f32 + 0.5, zg, game.w, game.h);
                        o2.set(x, y, 0, dot(po, gf.forward));
                        g2.set(x, y, 0, dot(pg, gf.forward));
                        valid.set(x, y, 0, 1.0);
                    }
                }
            } else {
                let both_clear = (0..ours.channels).all(|c| ours.get(oxi as u32, oyi as u32, c) == 0.0) && (0..game.channels).all(|c| game.get(x, y, c) == 0.0);
                valid.set(x, y, 0, if both_clear { 0.0 } else { 1.0 });
                for c in 0..ours.channels {
                    o2.set(x, y, c, ours.get(oxi as u32, oyi as u32, c));
                }
            }
        }
    }
    (g2, o2, valid, (only_g, only_o))
}

fn same_frustum(a: &Frustum, b: &Frustum) -> bool {
    let close = |p: [f32; 3], q: [f32; 3], tol: f32| (0..3).all(|k| (p[k] - q[k]).abs() <= tol);
    close(a.center, b.center, 1e-3 * a.half[0].max(1.0)) && close(a.half, b.half, 1e-4 * a.half[0].max(1.0)) && close(a.right, b.right, 1e-5) && close(a.up, b.up, 1e-5) && close(a.forward, b.forward, 1e-5)
}

/// Depth buffers (z01) to metres along the frustum's forward axis, same grid.
fn depth_to_metres(b: &Buf, f: &Frustum) -> Buf {
    let mut o = b.clone();
    for v in o.data.iter_mut() {
        *v = f.depth_metres(*v);
    }
    o
}

/// Options of a run.
pub struct Opts {
    /// The scale applied to the game's final `hbasis0` when it stands in for our `final_hdr` (default 1).
    pub hbasis_scale: f32,
    pub pass: Option<String>,
    pub tol: f32,
    pub floor: f32,
    pub stride: u32,
    pub pass_threshold: f64,
    pub game_map: Option<String>,
    /// The capture manifest to read instead of GAME_DIR/MANIFEST.json (a frozen copy).
    pub game_manifest: Option<String>,
    /// Compare accumulation snapshots as increments since the snapshot after this direction.
    pub delta_from: Option<u32>,
    pub keep_pairs: bool,
    pub quiet: bool,
}

impl Default for Opts {
    fn default() -> Self {
        Opts { hbasis_scale: 1.0, game_manifest: None, delta_from: None, pass: None, tol: 0.02, floor: 1e-3, stride: 1, pass_threshold: 99.0, game_map: None, keep_pairs: true, quiet: false }
    }
}

fn is_depth_pass(p: &str) -> bool {
    p == "peel_depth" || p == "sun_shadow" || p == "peel_sky_depth"
}

/// Match the game's direction list to ours (nearest vector) for a sweep: game index → our index.
/// The game's direction list of a sweep: its manifest's `sweeps`, else the per-entry `dir` vectors of
/// its peel / ilightdir entries (indexed by their `direction`).
fn game_dirs(game: &Manifest, sweep: u32) -> Vec<[f32; 3]> {
    if let Some(gs) = game.sweeps.iter().find(|s| s.sweep == sweep) {
        if !gs.dirs.is_empty() {
            return gs.dirs.clone();
        }
    }
    let mut by: std::collections::BTreeMap<u32, [f32; 3]> = Default::default();
    for e in &game.passes {
        if e.sweep.unwrap_or(0) == sweep {
            if let (Some(d), Some(v)) = (e.direction, e.dir) {
                by.entry(d).or_insert(v);
            }
        }
    }
    let Some((&max, _)) = by.iter().next_back() else { return Vec::new() };
    (0..=max).map(|d| by.get(&d).copied().unwrap_or([0.0; 3])).collect()
}

/// The game's ISSUE ORDER of a sweep's directions, as our indices: the accumulation snapshots (`hbasis0`
/// with a sweep_direction_index, banked or not) carry the direction just accumulated; each is matched
/// to our nearest vector. Returns (our index per issue position) — positions the capture lacks are
/// left out, so the caller appends the unmatched directions of ours after them.
pub fn game_issue_order(game: &Manifest, sweep: u32, ours: &[[f32; 3]]) -> Vec<(u32, u32)> {
    let mut es: Vec<(u32, [f32; 3])> = game.passes.iter().filter(|e| (e.pass == "hbasis0" || e.pass == "lightsum") && e.sweep.unwrap_or(0) == sweep && e.sweep_direction_index.is_some() && e.dir.is_some()).map(|e| (e.sweep_direction_index.unwrap(), e.dir.unwrap())).collect();
    es.sort_by_key(|e| e.0);
    es.dedup_by_key(|e| e.0);
    es.iter()
        .filter_map(|(k, gd)| {
            if ours.is_empty() { return None; }
            let best = (0..ours.len()).max_by(|&a, &b| { let ca = ours[a][0] * gd[0] + ours[a][1] * gd[1] + ours[a][2] * gd[2]; let cb = ours[b][0] * gd[0] + ours[b][1] * gd[1] + ours[b][2] * gd[2]; ca.partial_cmp(&cb).unwrap_or(std::cmp::Ordering::Equal) }).unwrap();
            Some((*k, best as u32))
        })
        .collect()
}

/// Our indices of the directions the game captured in a sweep (by nearest vector), sorted.
pub fn game_dir_indices(game: &Manifest, sweep: u32, ours: &[[f32; 3]]) -> Vec<u32> {
    let mut out: Vec<u32> = Vec::new();
    for gd in game_dirs(game, sweep) {
        if gd == [0.0; 3] || ours.is_empty() {
            continue;
        }
        let best = (0..ours.len()).max_by(|&a, &b| { let ca = ours[a][0] * gd[0] + ours[a][1] * gd[1] + ours[a][2] * gd[2]; let cb = ours[b][0] * gd[0] + ours[b][1] * gd[1] + ours[b][2] * gd[2]; ca.partial_cmp(&cb).unwrap_or(std::cmp::Ordering::Equal) }).unwrap();
        out.push(best as u32);
    }
    out.sort();
    out.dedup();
    out
}

fn direction_map(game: &Manifest, ours: &Manifest, sweep: u32) -> (HashMap<u32, u32>, Option<String>) {
    let gdirs = game_dirs(game, sweep);
    let os = ours.sweeps.iter().find(|s| s.sweep == sweep);
    let mut map = HashMap::new();
    let Some(os) = os else { return (map, None) };
    if gdirs.is_empty() || os.dirs.is_empty() {
        return (map, None);
    }
    let mut permuted = 0usize;
    let mut worst_deg = 0.0f32;
    let mut unknown = 0usize;
    for (gi, gd) in gdirs.iter().enumerate() {
        if *gd == [0.0; 3] {
            unknown += 1;
            continue;
        }
        let mut best = (0usize, -2.0f32);
        for (oi, od) in os.dirs.iter().enumerate() {
            let c = gd[0] * od[0] + gd[1] * od[1] + gd[2] * od[2];
            if c > best.1 {
                best = (oi, c);
            }
        }
        if best.0 != gi {
            permuted += 1;
        }
        worst_deg = worst_deg.max(best.1.clamp(-1.0, 1.0).acos().to_degrees());
        map.insert(gi as u32, best.0 as u32);
    }
    let note = if permuted > 0 { Some(format!("direction_permute(sweep {sweep}: {permuted} of {} directions re-matched by nearest vector, worst {worst_deg:.2}°{})", gdirs.len(), if unknown > 0 { format!(", {unknown} without a vector kept by index") } else { String::new() })) } else if worst_deg > 0.05 { Some(format!("direction_set(sweep {sweep}: same order, worst angle {worst_deg:.2}°)")) } else { None };
    (map, note)
}

/// Run the differential. Returns the rows in pipeline order and the convention findings.
pub fn run(game_root: &std::path::Path, ours_root: &std::path::Path, opts: &Opts) -> Result<(Vec<Row>, Vec<String>), String> {
    // (--game-manifest FILE: a frozen copy of the capture's manifest — the live one grows while the baker
    // banks; the dump that was matched against a copy is compared against the same copy)
    let gm_path = opts.game_manifest.clone().map(std::path::PathBuf::from).unwrap_or_else(|| game_root.join("MANIFEST.json"));
    let mut game = read_manifest(&std::fs::read_to_string(&gm_path).map_err(|e| format!("{}: {e}", gm_path.display()))?)?;
    let mut findings: Vec<String> = Vec::new();
    // THE GAME'S DOME LAYER: a peel whose first layer's depth is 0 over (nearly) the whole frame is the sky
    // dome drawn into the peel targets before the geometry layers — relabelled `peel_sky` (its colour is
    // the sky radiance the accumulate fills the facing texels with) and the later layers re-indexed from 0
    {
        let mut groups: std::collections::BTreeMap<(Option<u32>, Option<u32>, Option<u32>), Vec<usize>> = Default::default();
        for (i, e) in game.passes.iter().enumerate() {
            if e.pass == "peel_depth" && e.direction.is_some() {
                groups.entry((e.sweep, e.direction, e.peel)).or_default().push(i);
            }
        }
        // every peel opens with the dome drawn over (or after) its first geometry render: that first
        // snapshot is the sky layer (depth 0 where the dome covers; the fraction is reported)
        let mut dome_groups: Vec<((Option<u32>, Option<u32>, Option<u32>), f64)> = Vec::new();
        for (key, idx) in &groups {
            let first = idx.iter().copied().min_by_key(|&i| (game.passes[i].eid_last.unwrap_or(0), game.passes[i].layer.unwrap_or(0))).unwrap();
            let e = &game.passes[first];
            let zero_frac = load_entry(game_root, e).map(|b| { let n = (b.data.len() / 3).max(1); b.data.iter().step_by(3).filter(|v| **v == 0.0).count() as f64 / n as f64 }).unwrap_or(-1.0);
            dome_groups.push((*key, zero_frac));
        }
        for (key, zero_frac) in &dome_groups {
            // a first snapshot without the dome's depth-0 footprint is a geometry layer: the capture began
            // after the peel's sky render (direction 0's world peel in frame 40648) — nothing to relabel
            if *zero_frac < 0.3 {
                findings.push(format!("dome_layer(game sweep {:?} direction {:?} peel {:?}: no sky layer captured — its first snapshot has {:.1} % of its pixels at depth 0; the layers are taken as geometry layers 0..)", key.0, key.1, key.2, 100.0 * zero_frac));
                continue;
            }
            // the group's entries in event order: the first (depth + colour pair) is the sky layer
            let mut eids: Vec<u64> = game.passes.iter().filter(|e| (e.pass == "peel_depth" || e.pass == "peel_color") && (e.sweep, e.direction, e.peel) == *key).map(|e| e.eid_last.unwrap_or(0)).collect();
            eids.sort();
            eids.dedup();
            let first_eid = eids.first().copied().unwrap_or(0);
            for e in game.passes.iter_mut() {
                if (e.pass == "peel_depth" || e.pass == "peel_color") && (e.sweep, e.direction, e.peel) == *key {
                    let eid = e.eid_last.unwrap_or(0);
                    if eid == first_eid {
                        if e.pass == "peel_color" { e.pass = "peel_sky".into(); e.layer = None; } else { e.pass = "peel_sky_depth".into(); e.layer = None; }
                    } else {
                        let rank = eids.iter().position(|x| *x == eid).unwrap_or(1);
                        e.layer = Some((rank - 1) as u32);
                    }
                }
            }
            findings.push(format!("dome_layer(game sweep {:?} direction {:?} peel {:?}: the peel's first snapshot is the sky dome ({:.1} % of its pixels at depth 0) → `peel_sky`; its {} geometry layers re-indexed from 0)", key.0, key.1, key.2, 100.0 * zero_frac, eids.len().saturating_sub(1)));
        }
    }
    // THE GAME'S ACCUMULATION SNAPSHOTS: an `hbasis0` entry with a sweep_direction_index is the H-basis
    // constant term after that direction (issue order) — our `lightsum` after the same direction when ours
    // was baked in the game's order (--dir-order); the value is the raw C0 (ours dumped with κ = 1)
    {
        let mut n = 0;
        for e in game.passes.iter_mut() {
            if e.pass == "hbasis0" && e.sweep_direction_index.is_some() && e.banked.unwrap_or(true) {
                e.pass = "lightsum".into();
                e.direction = e.sweep_direction_index;
                n += 1;
            }
        }
        if n > 0 { findings.push(format!("accumulation_snapshots(the game's {n} banked `hbasis0` snapshots after direction k → `lightsum` after direction k; compare with ours baked in the game's order (--dir-order) and κ = 1)")); }
        if std::env::var_os("LMTOOL_PASSDIFF_DEBUG").is_some() {
            for e in game.passes.iter().filter(|e| e.pass == "lightsum") { eprintln!("game lightsum: sweep {:?} direction {:?} peel {:?} layer {:?} chart {:?} eid {:?} {}", e.sweep, e.direction, e.peel, e.layer, e.chart.as_ref().map(|c| c.obj), e.eid_last, e.file); }
        }
    }
    let ours = read_manifest(&std::fs::read_to_string(ours_root.join("MANIFEST.json")).map_err(|e| format!("{}: {e}", ours_root.join("MANIFEST.json").display()))?)?;
    // the game's chart rects (its layout, or the baked map's mapping); ours from our layout
    let game_map_path: Option<String> = opts.game_map.clone().or_else(|| game.baked_map.clone()).or(if game.map.is_empty() { None } else { Some(game.map.clone()) });
    let game_rects = chart_rects(&game, game_map_path.as_deref());
    if !game.layout.is_empty() { findings.push(format!("layout: the game's chart rects from its manifest ({} charts)", game.layout.len())); } else if let Some(p) = &game_map_path { findings.push(format!("layout: the game's chart rects from the mapping of {p} ({} charts)", game_rects.len())); } else { findings.push("layout: NO chart rects for the game side (no `layout`, no `baked_map`, no --game-map) — atlas-space passes cannot be cut".into()); }
    let game_rect_of: HashMap<u32, ChartRect> = game_rects.iter().map(|r| (r.obj, r.clone())).collect();
    let our_rect_of: HashMap<u32, ChartRect> = ours.layout.iter().map(|r| (r.obj, r.clone())).collect();
    // layout comparison (a convention pass of its own)
    let mut layout_rows: Vec<String> = Vec::new();
    for (obj, orr) in &our_rect_of {
        match game_rect_of.get(obj) {
            Some(gr) => { if gr.w != orr.w || gr.h != orr.h { layout_rows.push(format!("obj {obj}: size game {}×{} vs ours {}×{} (layout units)", gr.w, gr.h, orr.w, orr.h)); } if gr.x != orr.x || gr.y != orr.y { layout_rows.push(format!("obj {obj}: position game ({}, {}) vs ours ({}, {})", gr.x, gr.y, orr.x, orr.y)); } }
            None if !game_rect_of.is_empty() => layout_rows.push(format!("obj {obj}: not in the game's layout")),
            None => {}
        }
    }
    if !layout_rows.is_empty() {
        findings.push(format!("layout: {} differences (the packer / chart sizes — compared per chart by object id, resampled where the sizes differ): {}", layout_rows.len(), layout_rows.iter().take(6).cloned().collect::<Vec<_>>().join("; ")));
    }
    // direction sets per sweep
    let sweeps: Vec<u32> = { let mut v: Vec<u32> = ours.sweeps.iter().map(|s| s.sweep).chain(game.sweeps.iter().map(|s| s.sweep)).collect(); v.sort(); v.dedup(); v };
    let mut dir_maps: HashMap<u32, HashMap<u32, u32>> = HashMap::new();
    for &sw in &sweeps {
        let (m, note) = direction_map(&game, &ours, sw);
        if let Some(n) = note { findings.push(n); }
        dir_maps.insert(sw, m);
    }
    // layer order / dome layer conventions from the game's manifest
    let game_near_first = game.conventions.get("layer_order").and_then(|v| v.as_str()).map(|s| s.to_ascii_lowercase().contains("near-to-far") || s.to_ascii_lowercase().starts_with("nearest")).unwrap_or(false);
    let game_has_dome = game.conventions.get("layer0").and_then(|v| v.as_str()).map(|s| s.to_ascii_lowercase().contains("dome") || s.to_ascii_lowercase().contains("sky")).unwrap_or(true);
    let ours_has_dome = ours.conventions.get("layer0").and_then(|v| v.as_str()).map(|s| s.to_ascii_lowercase().contains("dome")).unwrap_or(false);
    if game_near_first { findings.push("layer_order: the game's k = 0 is the NEAREST layer — its layers are re-indexed far-to-near before the comparison".into()); }
    // index the game's entries by (pass, sweep, our-direction, layer, chart)
    let mut game_idx: HashMap<(String, Option<u32>, Option<u32>, Option<u32>, Option<u32>, Option<u32>), Vec<usize>> = HashMap::new();
    let game_layer_count: HashMap<(Option<u32>, Option<u32>, Option<u32>), u32> = {
        let mut m: HashMap<(Option<u32>, Option<u32>, Option<u32>), u32> = HashMap::new();
        for e in game.passes.iter().filter(|e| e.pass == "peel_depth") { let k = (e.sweep, e.direction, e.peel); let c = m.entry(k).or_insert(0); *c = (*c).max(e.layer.unwrap_or(0) + 1); }
        m
    };
    for (i, e) in game.passes.iter().enumerate() {
        let sweep = e.sweep;
        // (an accumulation snapshot's direction is the issue-order index, which ours shares through
        // --dir-order — no vector re-matching for it)
        let dir = if e.pass == "lightsum" { e.direction } else { e.direction.map(|d| dir_maps.get(&sweep.unwrap_or(0)).and_then(|m| m.get(&d).copied()).unwrap_or(d)) };
        let mut layer = e.layer;
        if let (Some(l), true) = (layer, game_near_first) {
            let n = game_layer_count.get(&(e.sweep, e.direction, e.peel)).copied().unwrap_or(l + 1);
            layer = Some(n - 1 - l);
        }
        if let Some(l) = layer {
            // our layer 0 is the synthetic dome: the game's real layer k ↔ our k + 1 when the game has none
            if ours_has_dome && !game_has_dome { layer = Some(l + 1); }
        }
        game_idx.entry((e.pass.clone(), sweep, dir, e.peel, layer, e.chart.as_ref().map(|c| c.obj))).or_default().push(i);
    }
    if ours_has_dome && !game_has_dome { findings.push("strip_dome: the game has no dome layer — our synthetic layer 0 is skipped, our layer k + 1 ↔ the game's layer k".into()); }
    // walk OUR entries in pipeline order
    let mut order: Vec<usize> = (0..ours.passes.len()).collect();
    order.sort_by_key(|&i| { let e = &ours.passes[i]; (pipeline_rank(&e.pass), e.sweep.unwrap_or(0), e.direction.unwrap_or(0), e.peel.unwrap_or(0), e.layer.unwrap_or(0), e.chart.as_ref().map(|c| c.obj).unwrap_or(0)) });
    let mut rows: Vec<Row> = Vec::new();
    let mut compared_passes: std::collections::BTreeSet<String> = Default::default();
    let mut missing: HashMap<String, usize> = HashMap::new();
    // one pair: the row, or Err(Some(pass)) when the game has no entry for it (Err(None) = skipped)
    let compare_one = |i: usize| -> Result<Row, Option<String>> {
        let oe = &ours.passes[i];
        if let Some(p) = &opts.pass { if &oe.pass != p { return Err(None); } }
        if pipeline_rank(&oe.pass) == PIPELINE.len() { return Err(None); }
        let obj = oe.chart.as_ref().map(|c| c.obj);
        // the game's matching entry: same chart, or an atlas-space one to cut; our `final_hdr` (E) also
        // matches the game's `hbasis0` (C0 = √(2π)·E for a flat normal) through a named scale
        let key_chart = (oe.pass.clone(), oe.sweep, oe.direction, oe.peel, oe.layer, obj);
        let key_atlas = (oe.pass.clone(), oe.sweep, oe.direction, oe.peel, oe.layer, None);
        let mut pre_scale: Option<(f32, &str)> = None;
        // several capture snapshots of one target (the accumulation after every layer) → the LAST one
        let pick = |v: &Vec<usize>| -> Option<usize> { v.iter().copied().max_by_key(|&i| game.passes[i].eid_last.unwrap_or(0)) };
        let mut ge_i = game_idx.get(&key_chart).and_then(pick).or_else(|| if obj.is_some() { game_idx.get(&key_atlas).and_then(pick) } else { None });
        if ge_i.is_none() && oe.pass == "final_hdr" {
            // the game's final hbasis0 (no snapshot index): C0 in the target's units; --hbasis-scale S scales it
            // into ours (1 when ours is dumped with κ = 1, 1/√(2π) for the port's E units)
            ge_i = game_idx.get(&("hbasis0".to_string(), None, None, None, None, None)).and_then(pick);
            if ge_i.is_some() && (opts.hbasis_scale - 1.0).abs() > 1e-6 { pre_scale = Some((opts.hbasis_scale, "hbasis_c0_scale(game C0 × --hbasis-scale)")); }
        }
        let Some(ge_i) = ge_i else {
            if std::env::var_os("LMTOOL_PASSDIFF_DEBUG").is_some() { eprintln!("no game entry for ours: {} sweep {:?} direction {:?} peel {:?} layer {:?} chart {:?} ({})", oe.pass, oe.sweep, oe.direction, oe.peel, oe.layer, obj, oe.file); }
            return Err(Some(oe.pass.clone()));
        };
        let ge = &game.passes[ge_i];
        let mut transforms: Vec<String> = Vec::new();
        let mut ob = match load_entry(ours_root, oe) { Ok(b) => b, Err(e) => { eprintln!("passdiff: ours {}: {e}", oe.file); return Err(None); } };
        let mut gb = match load_entry(game_root, ge) { Ok(b) => b, Err(e) => { eprintln!("passdiff: game {}: {e}", ge.file); return Err(None); } };
        // --delta-from J: an accumulation snapshot compared as its INCREMENT since the snapshot after
        // direction J (both sides) — the directions between the two, isolated
        if let (Some(j), "lightsum", Some(k)) = (opts.delta_from, oe.pass.as_str(), oe.direction) {
            if k <= j { return Err(None); }
            let op = ours.passes.iter().find(|e| e.pass == "lightsum" && e.sweep == oe.sweep && e.direction == Some(j) && e.chart.as_ref().map(|c| c.obj) == obj);
            let gp = game_idx.get(&("lightsum".to_string(), oe.sweep, Some(j), None, None, None)).and_then(pick).map(|i| &game.passes[i]);
            let (Some(op), Some(gp)) = (op, gp) else { return Err(None) };
            let (Ok(opb), Ok(gpb)) = (load_entry(ours_root, op), load_entry(game_root, gp)) else { return Err(None) };
            if opb.data.len() == ob.data.len() && gpb.data.len() == gb.data.len() {
                for (a, b) in ob.data.iter_mut().zip(opb.data.iter()) { *a -= *b; }
                for (a, b) in gb.data.iter_mut().zip(gpb.data.iter()) { *a -= *b; }
                transforms.push(format!("delta_from(direction {j}: the increment of directions {}..={k})", j + 1));
            }
        }
        if let Some((s, name)) = pre_scale { for v in gb.data.iter_mut() { *v *= s; } transforms.push(name.into()); }
        let depth = is_depth_pass(&oe.pass);
        let channels = if depth { 1 } else { ob.channels.min(gb.channels).min(3) };
        let floor = if depth { opts.floor.max(1e-3) } else { opts.floor };
        let (mut g, mut o): (Buf, Buf);
        let mut remap_valid: Option<Buf> = None;
        let mut coverage_note = String::new();
        // a depth target's clear (1.0 = near for the reversed-z LESS peel) is not a surface
        let (clear_g, clear_o) = if depth { (depth_clear(ge, &gb), depth_clear(oe, &ob)) } else { (f32::NAN, f32::NAN) };
        if oe.space == "peel" || ge.space == "peel" {
            // peel space: the same frustum → pixel to pixel; else remap through the world
            match (&ge.frustum, &oe.frustum) {
                (Some(gf), Some(of)) if !same_frustum(gf, of) || gb.w != ob.w || gb.h != ob.h => {
                    transforms.push(format!("frustum_remap(game centre {:?} half {:?} {}×{} ← ours centre {:?} half {:?} {}×{})", gf.center, gf.half, gb.w, gb.h, of.center, of.half, ob.w, ob.h));
                    if depth { transforms.push(format!("depth_to_metres(along the game's forward; clears {clear_g} / {clear_o})")); }
                    let (g2, o2, v, (only_g, only_o)) = remap_peel(&gb, gf, &ob, of, depth, clear_g, clear_o);
                    if depth && (only_g > 0 || only_o > 0) { coverage_note = format!("coverage: {only_g} px with a game surface where ours is clear, {only_o} the reverse"); }
                    g = g2; o = o2; remap_valid = Some(v);
                }
                (Some(gf), Some(_)) if depth => {
                    transforms.push(format!("depth_to_metres(clears {clear_g} / {clear_o})"));
                    // the same grid: clears → NaN (skipped), one-sided pixels counted as coverage
                    let (mut g2, mut o2) = (depth_to_metres(&gb, gf), depth_to_metres(&ob, gf));
                    let (mut only_g, mut only_o) = (0usize, 0usize);
                    for i in 0..(gb.w * gb.h) as usize {
                        let (cg, co) = (gb.data[i] == clear_g, ob.data[i] == clear_o);
                        if cg || co { g2.data[i] = f32::NAN; o2.data[i] = f32::NAN; }
                        if cg && !co { only_o += 1; }
                        if co && !cg { only_g += 1; }
                    }
                    if only_g > 0 || only_o > 0 { coverage_note = format!("coverage: {only_g} px with a game surface where ours is clear, {only_o} the reverse"); }
                    g = g2; o = o2;
                }
                (None, _) | (_, None) if gb.w != ob.w || gb.h != ob.h => {
                    transforms.push(format!("resample(no frustum: ours {}×{} → game {}×{})", ob.w, ob.h, gb.w, gb.h));
                    g = gb.clone(); o = ob.resample(gb.w, gb.h);
                }
                _ => { g = gb.clone(); o = ob.clone(); }
            }
            // orientation
            let (o3, name) = best_orientation(&g, &o, channels, floor);
            if let Some(n) = name { transforms.push(format!("{n}(the game's target is ours mirrored/transposed)")); o = o3; }
            // a layer's COLOUR is meaningful only where the layer has a fragment on both sides: mask the
            // peel_color comparison by the two depth buffers of the same layer (through the same remap)
            if oe.pass == "peel_color" && remap_valid.is_none() {
                let od = ours.passes.iter().find(|e| e.pass == "peel_depth" && e.sweep == oe.sweep && e.direction == oe.direction && e.peel == oe.peel && e.layer == oe.layer);
                let gd = game_idx.get(&("peel_depth".to_string(), oe.sweep, oe.direction, oe.peel, oe.layer, None)).and_then(pick).map(|i| &game.passes[i]);
                if let (Some(od), Some(gd), Some(gf), Some(of)) = (od, gd, &ge.frustum, &oe.frustum) {
                    if let (Ok(odb), Ok(gdb)) = (load_entry(ours_root, od), load_entry(game_root, gd)) {
                        let (cg, co) = (depth_clear(gd, &gdb), depth_clear(od, &odb));
                        let (_, _, v, (only_g, only_o)) = remap_peel(&gdb, gf, &odb, of, true, cg, co);
                        transforms.push(format!("depth_mask(colour compared where both layers have a fragment; {only_g} px game-only, {only_o} ours-only)"));
                        remap_valid = Some(v);
                    }
                }
            } else if oe.pass == "peel_color" {
                // remapped through the frustums already: mask by the depth pair through the same remap
                let od = ours.passes.iter().find(|e| e.pass == "peel_depth" && e.sweep == oe.sweep && e.direction == oe.direction && e.peel == oe.peel && e.layer == oe.layer);
                let gd = game_idx.get(&("peel_depth".to_string(), oe.sweep, oe.direction, oe.peel, oe.layer, None)).and_then(pick).map(|i| &game.passes[i]);
                if let (Some(od), Some(gd), Some(gf), Some(of)) = (od, gd, &ge.frustum, &oe.frustum) {
                    if let (Ok(odb), Ok(gdb)) = (load_entry(ours_root, od), load_entry(game_root, gd)) {
                        let (cg, co) = (depth_clear(gd, &gdb), depth_clear(od, &odb));
                        let (_, _, v, (only_g, only_o)) = remap_peel(&gdb, gf, &odb, of, true, cg, co);
                        transforms.push(format!("depth_mask(colour compared where both layers have a fragment; {only_g} px game-only, {only_o} ours-only)"));
                        remap_valid = Some(v);
                    }
                }
            }
        } else {
            // chart space: cut the game's atlas by the chart rect when the game entry is atlas-wide
            if ge.chart.is_none() {
                let Some(obj) = obj else { return Err(None) };
                let Some(r) = game_rect_of.get(&obj) else { return Err(Some(format!("{} (no game rect for obj {obj})", oe.pass))) };
                let stored_w = if game.atlas.stored_w > 0 { game.atlas.stored_w } else { 1024 };
                match cut_chart(&gb, r, stored_w) {
                    Some((cut, sc)) => { transforms.push(format!("chart_cut(obj {obj}: rect ({}, {}) {}×{} at scale {sc})", r.x, r.y, r.w, r.h)); g = cut; }
                    None => { eprintln!("passdiff: {}: cannot cut obj {obj} from a {}×{} buffer", oe.pass, gb.w, gb.h); return Err(None); }
                }
            } else { g = gb.clone(); }
            o = ob.clone();
            let (g2, o2) = align_chart(g, o, &mut transforms);
            g = g2; o = o2;
        }
        // masks: compare where either side is non-zero (uncovered texels / clears are skipped); a depth
        // buffer's clear is z01 = 0 BEFORE the metre conversion, so the mask is taken on the raw buffers
        let (gc, oc) = (g.clone(), o.clone());
        let mask = move |x: u32, y: u32| -> bool {
            if let Some(v) = &remap_valid {
                return v.get(x, y, 0) != 0.0;
            }
            if depth {
                // clears became NaN above; compare() skips non-finite values
                return gc.get(x, y, 0).is_finite() && oc.get(x, y, 0).is_finite();
            }
            (0..channels).any(|c| gc.get(x, y, c) != 0.0 || oc.get(x, y, c) != 0.0)
        };
        let mut stats = compare(&g, &o, channels, opts.tol, floor, opts.stride, &mask);
        // quantisation conventions for colour passes: does a storage rounding explain the residual?
        // (only when the residual is small — a storage rounding is a few percent at most)
        if !depth && channels >= 3 && stats.n > 0 && stats.pct_within() < 99.99 && stats.mean_abs <= 0.05 * stats.mean_ref.max(1e-6) {
            let cands = [("quantise_r11g11b10_rtne", Quant::R11G11B10, Rounding::NearestEven), ("quantise_r11g11b10_rtz", Quant::R11G11B10, Rounding::Truncate), ("quantise_f16_rtne", Quant::F16, Rounding::NearestEven)];
            let mut best: Option<(&str, Stats, Buf)> = None;
            for (name, q, r) in cands {
                let oq = o.quantised(q, r);
                let s = compare(&g, &oq, channels, opts.tol, floor, opts.stride.max(2), &mask);
                if s.n > 0 && s.rmse < stats.rmse * 0.7 && best.as_ref().map(|b| s.rmse < b.1.rmse).unwrap_or(true) {
                    best = Some((name, s, oq));
                }
            }
            if let Some((name, _s, oq)) = best {
                let s_full = compare(&g, &oq, channels, opts.tol, floor, opts.stride, &mask);
                transforms.push(format!("{name}(ours re-quantised: RMSE {:.4} → {:.4})", stats.rmse, s_full.rmse));
                stats = s_full;
                o = oq;
            }
        }
        // the bias line (a systematic offset is a convention smell: depth bias, a scale, the sky ×2)
        let mut note = if stats.n > 0 && stats.mean_ref > 0.0 && stats.mean_signed.abs() > 0.25 * stats.mean_abs && stats.mean_abs > opts.floor as f64 {
            let per_ch: Vec<String> = (0..channels as usize).filter(|&c| stats.ch_n[c] > 0 && stats.ch_game[c] != 0.0).map(|c| format!("{:.3}", stats.ch_ours[c] / stats.ch_game[c])).collect();
            format!("systematic: mean Δ {:+.4} ({:+.1} % of the game's mean {:.4}; ours/game per channel {})", stats.mean_signed, 100.0 * stats.mean_signed / stats.mean_ref, stats.mean_ref, per_ch.join("/"))
        } else { String::new() };
        if !coverage_note.is_empty() { if !note.is_empty() { note += "; "; } note += &coverage_note; }
        Ok(Row { pass: oe.pass.clone(), sweep: oe.sweep, direction: oe.direction, peel: oe.peel, layer: oe.layer, chart: obj, stats, transforms, note, pair: if opts.keep_pairs { Some((g, o, channels)) } else { None } })
    };
    // the pairs are independent: 16 workers (each pair holds two buffers of up to 200 MB)
    let n_workers = 16usize.min(order.len().max(1));
    let results: Vec<(usize, Result<Row, Option<String>>)> = std::thread::scope(|sc| {
        let hs: Vec<_> = (0..n_workers)
            .map(|wk| {
                let order = &order;
                let compare_one = &compare_one;
                sc.spawn(move || -> Vec<(usize, Result<Row, Option<String>>)> {
                    order.iter().enumerate().filter(|(k, _)| k % n_workers == wk).map(|(k, &i)| (k, compare_one(i))).collect()
                })
            })
            .collect();
        let mut all: Vec<(usize, Result<Row, Option<String>>)> = hs.into_iter().flat_map(|h| h.join().unwrap()).collect();
        all.sort_by_key(|(k, _)| *k);
        all
    });
    for (_, r) in results {
        match r {
            Ok(row) => { compared_passes.insert(row.pass.clone()); rows.push(row); }
            Err(Some(p)) => { *missing.entry(p).or_insert(0) += 1; }
            Err(None) => {}
        }
    }
    for (p, n) in &missing {
        findings.push(format!("not compared: {n} of our `{p}` buffers have no game entry"));
    }
    Ok((rows, findings))
}

/// Per-pass aggregate over rows.
#[derive(Clone, Debug, Default)]
pub struct PassSummary {
    pub pass: String,
    pub rows: usize,
    pub n: usize,
    pub max_abs: f32,
    pub mean_abs: f64,
    pub rmse: f64,
    pub within: usize,
    pub mean_signed: f64,
    pub mean_ref: f64,
    pub transforms: std::collections::BTreeSet<String>,
}

impl PassSummary {
    pub fn pct_within(&self) -> f64 {
        if self.n == 0 { 0.0 } else { 100.0 * self.within as f64 / self.n as f64 }
    }
}

pub fn summarise(rows: &[Row]) -> Vec<PassSummary> {
    let mut by: std::collections::BTreeMap<usize, PassSummary> = Default::default();
    for r in rows {
        let s = by.entry(pipeline_rank(&r.pass)).or_insert_with(|| PassSummary { pass: r.pass.clone(), ..Default::default() });
        s.rows += 1;
        let n = r.stats.n as f64;
        s.max_abs = s.max_abs.max(r.stats.max_abs);
        s.mean_abs = (s.mean_abs * s.n as f64 + r.stats.mean_abs * n) / (s.n as f64 + n).max(1.0);
        s.rmse = ((s.rmse * s.rmse * s.n as f64 + r.stats.rmse * r.stats.rmse * n) / (s.n as f64 + n).max(1.0)).sqrt();
        s.mean_signed = (s.mean_signed * s.n as f64 + r.stats.mean_signed * n) / (s.n as f64 + n).max(1.0);
        s.mean_ref = (s.mean_ref * s.n as f64 + r.stats.mean_ref * n) / (s.n as f64 + n).max(1.0);
        s.n += r.stats.n;
        s.within += r.stats.within;
        for t in &r.transforms {
            // keep the transform's name (before the parenthesis) so the set stays small
            s.transforms.insert(t.split('(').next().unwrap_or(t).to_string());
        }
    }
    by.into_values().collect()
}

/// The markdown report: the pass table, the convention findings, the first divergent pass.
pub fn report(rows: &[Row], findings: &[String], threshold: f64, tol: f32) -> String {
    let mut out = String::new();
    out += &format!("| pass | buffers | texels | max abs | mean abs | RMSE | mean Δ (ours − game) | within ±{:.0} % | conventions |\n|---|---|---|---|---|---|---|---|---|\n", tol * 100.0);
    let sums = summarise(rows);
    let mut first: Option<&PassSummary> = None;
    for s in &sums {
        out += &format!("| {} | {} | {} | {:.4} | {:.4} | {:.4} | {:+.4} ({:+.1} %) | {:.2} % | {} |\n", s.pass, s.rows, s.n, s.max_abs, s.mean_abs, s.rmse, s.mean_signed, if s.mean_ref > 0.0 { 100.0 * s.mean_signed / s.mean_ref } else { 0.0 }, s.pct_within(), s.transforms.iter().cloned().collect::<Vec<_>>().join(", "));
        if first.is_none() && s.n > 0 && s.pct_within() < threshold {
            first = Some(s);
        }
    }
    if !findings.is_empty() {
        out += "\nConventions:\n";
        for f in findings {
            out += &format!("* {f}\n");
        }
    }
    match first {
        Some(s) => {
            out += &format!("\n**FIRST DIVERGENT PASS: `{}`** — {:.2} % of {} texels within ±{:.0} % (threshold {threshold} %), RMSE {:.4}, mean Δ {:+.4} ({:+.1} % of the game's mean {:.4}), max |Δ| {:.4}.\n", s.pass, s.pct_within(), s.n, tol * 100.0, s.rmse, s.mean_signed, if s.mean_ref > 0.0 { 100.0 * s.mean_signed / s.mean_ref } else { 0.0 }, s.mean_ref, s.max_abs);
            // the worst buffers of that pass
            let mut worst: Vec<&Row> = rows.iter().filter(|r| r.pass == s.pass).collect();
            worst.sort_by(|a, b| b.stats.rmse.partial_cmp(&a.stats.rmse).unwrap_or(std::cmp::Ordering::Equal));
            for r in worst.iter().take(5) {
                out += &format!("  * {}: {:.2} % within, RMSE {:.4}, worst texel ({}, {}) ch {} game {:.4} ours {:.4}{}{}\n", r.key(), r.stats.pct_within(), r.stats.rmse, r.stats.worst.0, r.stats.worst.1, r.stats.worst.2, r.stats.worst.3, r.stats.worst.4, if r.note.is_empty() { String::new() } else { format!("; {}", r.note) }, if r.transforms.is_empty() { String::new() } else { format!("; transforms: {}", r.transforms.join(", ")) });
            }
        }
        None if !rows.is_empty() => out += &format!("\nNo divergent pass: every compared pass has ≥ {threshold} % of its texels within ±{:.0} %.\n", tol * 100.0),
        None => out += "\nNothing compared.\n",
    }
    out
}

/// Heat maps of a compared pair: game | ours | |Δ| as one PNG (tonemapped by the pair's max).
pub fn heat_png(path: &str, g: &Buf, o: &Buf, channels: u32, tol: f32) -> std::io::Result<()> {
    let (w, h) = (g.w.min(o.w), g.h.min(o.h));
    let mut max = 1e-6f32;
    for y in 0..h { for x in 0..w { for c in 0..channels { let (a, b) = (g.get(x, y, c), o.get(x, y, c)); if a.is_finite() { max = max.max(a.abs()); } if b.is_finite() { max = max.max(b.abs()); } } } }
    let tone = |v: f32| -> u8 { ((v.max(0.0) / max).powf(1.0 / 2.2) * 255.0).round().clamp(0.0, 255.0) as u8 };
    let mut px = vec![0u8; (w * 3 * h * 3) as usize];
    let stride = (w * 3 * 3) as usize;
    for y in 0..h {
        for x in 0..w {
            let (mut gc, mut oc) = ([0u8; 3], [0u8; 3]);
            let mut dmax = 0.0f32;
            let mut rel = 0.0f32;
            for c in 0..3.min(channels) {
                let (a, b) = (g.get(x, y, c), o.get(x, y, c));
                gc[c as usize] = tone(a);
                oc[c as usize] = tone(b);
                if a.is_finite() && b.is_finite() {
                    dmax = dmax.max((a - b).abs());
                    rel = rel.max((a - b).abs() / (a.abs().max(b.abs()) + 1e-3));
                }
            }
            if channels == 1 { gc = [gc[0]; 3]; oc = [oc[0]; 3]; }
            // the heat: relative error, black = 0, blue ≤ tol, green 2·tol, yellow 4·tol, red ≥ 8·tol
            let t = (rel / (8.0 * tol)).min(1.0);
            let heat: [u8; 3] = if rel <= tol { [0, 0, (64.0 + 191.0 * (rel / tol)) as u8] } else if t < 0.5 { [(255.0 * (t - 0.125) / 0.375).clamp(0.0, 255.0) as u8, 255, 0] } else { [255, (255.0 * (1.0 - (t - 0.5) / 0.5)) as u8, 0] };
            let _ = dmax;
            let o0 = y as usize * stride + x as usize * 3;
            px[o0..o0 + 3].copy_from_slice(&gc);
            let o1 = y as usize * stride + (w + x) as usize * 3;
            px[o1..o1 + 3].copy_from_slice(&oc);
            let o2 = y as usize * stride + (2 * w + x) as usize * 3;
            px[o2..o2 + 3].copy_from_slice(&heat);
        }
    }
    crate::png::write_rgb(path, w * 3, h, &px)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fr(center: [f32; 3], half: [f32; 3]) -> Frustum {
        Frustum { ortho: true, center, half, right: [1.0, 0.0, 0.0], up: [0.0, 0.0, 1.0], forward: [0.0, -1.0, 0.0], depth: Frustum::REVERSED.into() }
    }

    #[test]
    fn r11g11b10_raw_round_trip_through_the_loader() {
        let px = [[0.5f32, 1.25, 3.0], [0.0, 0.0, 0.0]];
        let mut bytes = Vec::new();
        for p in px { bytes.extend_from_slice(&crate::gpufmt::pack_r11g11b10(p, Rounding::NearestEven).to_le_bytes()); }
        let b = decode_raw(&bytes, parse_format("DXGI_FORMAT_R11G11B10_FLOAT"), 2, 1, 0).unwrap();
        assert_eq!(b.channels, 3);
        assert_eq!(&b.data[..3], &[0.5, 1.25, 3.0]);
        assert_eq!(parse_format("r32_float"), Fmt::F32(1));
        assert_eq!(parse_format("D32_FLOAT"), Fmt::F32(1));
        assert_eq!(parse_format("R16G16B16A16_FLOAT"), Fmt::F16(4));
    }

    #[test]
    fn f16_and_d24_decoding() {
        let mut bytes = Vec::new();
        for v in [1.0f32, 0.5, -2.0, 1.0] { bytes.extend_from_slice(&crate::gpufmt::encode_f16(v, Rounding::NearestEven).to_le_bytes()); }
        let b = decode_raw(&bytes, Fmt::F16(4), 1, 1, 0).unwrap();
        assert_eq!(b.data, vec![1.0, 0.5, -2.0, 1.0]);
        let d = decode_raw(&0x00ff_ffffu32.to_le_bytes(), Fmt::D24S8, 1, 1, 0).unwrap();
        assert_eq!(d.data[0], 1.0);
    }

    #[test]
    fn a_manifest_cut_inside_a_key_is_repaired() {
        // the capture's manifest cut after a key's colon, and after a bare key
        for cut in ["{\"passes\": [{\"pass\": \"a\", \"viewport\": [{\"x\": 0.0, \"minDepth\":", "{\"passes\": [{\"pass\": \"a\", \"viewport\": [{\"x\": 0.0, \"minDepth\"", "{\"passes\": [{\"pass\": \"a\", \"viewport\": [{\"x\": 0.0,"] {
            let fixed = repair_truncated_json(cut);
            let v: serde_json::Value = serde_json::from_str(&fixed).unwrap_or_else(|e| panic!("{cut:?} → {fixed:?}: {e}"));
            assert_eq!(v["passes"][0]["viewport"][0]["x"].as_f64(), Some(0.0));
        }
    }

    #[test]
    fn dds_dx10_header_is_read() {
        // a 2×1 R32_FLOAT DDS with a DX10 header
        let mut b = vec![0u8; 148 + 8];
        b[..4].copy_from_slice(b"DDS ");
        b[4..8].copy_from_slice(&124u32.to_le_bytes());
        b[8..12].copy_from_slice(&(0x1u32 | 0x2 | 0x4 | 0x1000).to_le_bytes());
        b[12..16].copy_from_slice(&1u32.to_le_bytes());
        b[16..20].copy_from_slice(&2u32.to_le_bytes());
        b[76..80].copy_from_slice(&32u32.to_le_bytes());
        b[80..84].copy_from_slice(&0x4u32.to_le_bytes());
        b[84..88].copy_from_slice(b"DX10");
        b[128..132].copy_from_slice(&41u32.to_le_bytes()); // DXGI_FORMAT_R32_FLOAT
        b[148..152].copy_from_slice(&0.25f32.to_le_bytes());
        b[152..156].copy_from_slice(&0.75f32.to_le_bytes());
        let (dxgi, w, h, pitch, off) = parse_dds(&b).unwrap();
        assert_eq!((dxgi, w, h, pitch, off), (41, 2, 1, 0, 148));
        let buf = decode_raw(&b[off..], dxgi_fmt(dxgi), w, h, pitch).unwrap();
        assert_eq!(buf.data, vec![0.25, 0.75]);
    }

    #[test]
    fn cut_chart_uses_the_stored_footprint_at_the_buffer_scale() {
        // a 2048-wide buffer over the 1024² atlas (scale 2): chart x = 1 (layout), w = 100 → texels 1..51 → pixels 2..102
        let mut atlas = Buf::new(2048, 2048, 1);
        for y in 0..2048 { for x in 0..2048 { atlas.set(x, y, 0, (x + 10_000 * y) as f32); } }
        let r = ChartRect { obj: 4096, item: 0, sub: 0, x: 1, y: 3, w: 100, h: 60, chart_w: 50, chart_h: 30 };
        let (cut, sc) = cut_chart(&atlas, &r, 1024).unwrap();
        assert_eq!(sc, 2);
        assert_eq!((cut.w, cut.h), (100, 60));
        assert_eq!(cut.get(0, 0, 0), (2 + 10_000 * 4) as f32);
    }

    #[test]
    fn box_down_is_the_coverage_weighted_mean() {
        let mut b = Buf::new(2, 2, 3);
        for c in 0..3 { b.set(0, 0, c, 1.0); b.set(1, 0, c, 3.0); }
        // (0,1) and (1,1) uncovered (all zero)
        let w = b.box_down(2, 2, true);
        assert_eq!(w.get(0, 0, 0), 2.0, "weighted: the mean of the two covered sub-samples");
        let u = b.box_down(2, 2, false);
        assert_eq!(u.get(0, 0, 0), 1.0, "plain: the mean over all four");
    }

    #[test]
    fn compare_counts_tolerance_and_bias() {
        let mut g = Buf::new(4, 1, 1);
        let mut o = Buf::new(4, 1, 1);
        for x in 0..4 { g.set(x, 0, 0, 1.0); o.set(x, 0, 0, if x == 3 { 1.5 } else { 1.01 }); }
        let s = compare(&g, &o, 1, 0.02, 0.0, 1, &|_, _| true);
        assert_eq!(s.n, 4);
        assert_eq!(s.within, 3);
        assert!((s.max_abs - 0.5).abs() < 1e-6);
        assert_eq!(s.worst.0, 3);
        assert!(s.mean_signed > 0.0);
        assert!((s.pct_within() - 75.0).abs() < 1e-9);
    }

    #[test]
    fn mirror_transform_is_detected_as_a_convention() {
        let mut g = Buf::new(16, 16, 1);
        for y in 0..16 { for x in 0..16 { g.set(x, y, 0, x as f32 + 0.1 * y as f32); } }
        let o = g.mirror_x();
        let (fixed, name) = best_orientation(&g, &o, 1, 1e-3);
        assert_eq!(name.as_deref(), Some("mirror_x"));
        assert_eq!(fixed.data, g.data);
        let (same, none) = best_orientation(&g, &g, 1, 1e-3);
        assert!(none.is_none());
        assert_eq!(same.data, g.data);
    }

    #[test]
    fn remap_peel_compares_depths_in_metres_through_the_world() {
        // the game looks straight down over a 20 m square, 8×8 px, depth range 40 m; ours over the same
        // square at 16×16 px with another depth range: a plane at y = 3 must agree to the metre
        let gf = fr([10.0, 5.0, 10.0], [10.0, 10.0, 20.0]);
        let of = fr([10.0, 0.0, 10.0], [10.0, 10.0, 8.0]);
        let mut g = Buf::new(8, 8, 1);
        let mut o = Buf::new(16, 16, 1);
        let plane = [0.0, 3.0, 0.0];
        for y in 0..8 { for x in 0..8 { let (_, _, z) = gf.project(plane, 8, 8); g.set(x, y, 0, z); } }
        for y in 0..16 { for x in 0..16 { let (_, _, z) = of.project(plane, 16, 16); o.set(x, y, 0, z); } }
        assert!((g.get(0, 0, 0) - 0.45).abs() < 1e-6, "game z01 of y = 3 (below the centre, farther from a camera looking down): 0.5 + (−5 + 3)/40");
        assert!((o.get(0, 0, 0) - (0.5 + 3.0 / 16.0)).abs() < 1e-6);
        let (g2, o2, _valid, _cov) = remap_peel(&g, &gf, &o, &of, true, 1.0, 1.0);
        let s = compare(&g2, &o2, 1, 0.0, 1e-4, 1, &|_, _| true);
        assert_eq!(s.n, 64);
        assert_eq!(s.within, 64, "both sides → −3 m along the game's forward (0, −1, 0): {:?}", s);
        assert!((g2.get(0, 0, 0) + 3.0).abs() < 1e-4);
    }

    #[test]
    fn pipeline_order_and_report_name_the_first_divergent_pass() {
        assert!(pipeline_rank("sun_shadow") < pipeline_rank("peel_depth"));
        assert!(pipeline_rank("peel_depth") < pipeline_rank("lightsum"));
        let ok = Row { pass: "sun_shadow".into(), sweep: None, direction: None, peel: None, layer: None, chart: None, stats: Stats { n: 100, within: 100, ..Default::default() }, transforms: vec![], note: String::new(), pair: None };
        let bad = Row { pass: "peel_color".into(), sweep: Some(0), direction: Some(3), peel: None, layer: Some(1), chart: None, stats: Stats { n: 100, within: 50, rmse: 0.2, max_abs: 0.9, mean_ref: 1.0, mean_signed: -0.1, ..Default::default() }, transforms: vec!["mirror_x(...)".into()], note: String::new(), pair: None };
        let r = report(&[ok, bad], &[], 99.0, 0.02);
        assert!(r.contains("FIRST DIVERGENT PASS: `peel_color`"), "{r}");
        assert!(r.contains("| sun_shadow | 1 | 100 |"), "{r}");
        assert!(r.contains("mirror_x"), "{r}");
    }

    #[test]
    fn manifest_reader_tolerates_a_sparse_capture_manifest() {
        let txt = r#"{"passes":[{"pass":"peel_depth","sweep":0,"direction":2,"layer":0,"file":"peel_depth/s0/d002/l00.dds","format":"D32_FLOAT","width":2048,"height":2048,"space":"peel","dir":[0,1,0],"frustum":{"center":[0,0,0],"half":[1,1,1],"right":[1,0,0],"up":[0,0,1],"forward":[0,-1,0]}}],"sweeps":[{"sweep":0,"dirs":[[0,1,0],[1,0,0],[0,0,1]]}]}"#;
        let m = read_manifest(txt).unwrap();
        assert_eq!(m.passes.len(), 1);
        assert_eq!(m.passes[0].frustum.as_ref().unwrap().depth, "reversed_z01");
        assert!(m.passes[0].frustum.as_ref().unwrap().ortho);
        let fs = peel_frustums(&m, 0);
        assert_eq!(fs.len(), 3, "dense up to direction 2, filled from the nearest lower captured direction");
    }
}

#[cfg(test)]
mod lenient_tests {
    use super::*;

    #[test]
    fn manifest_reader_coerces_strings_nulls_and_xyz_objects() {
        let txt = r#"{"passes":[{"pass":"peel_color","sweep":"1","direction":7,"layer":null,"file":"a.dds","format":"R11G11B10_FLOAT","width":"2048","height":2048,"space":"peel","dir":{"x":0.1,"y":0.9,"z":0.0},"frustum":{"center":{"x":1,"y":2,"z":3},"half":[1,1,1],"right":[1,0,0],"up":[0,1,0],"forward":[0,0,1]}}],"sweeps":[{"sweep":"0","dirs":[{"x":0,"y":1,"z":0}]}]}"#;
        let m = read_manifest(txt).unwrap();
        let e = &m.passes[0];
        assert_eq!(e.sweep, Some(1));
        assert_eq!(e.layer, None);
        assert_eq!(e.width, 2048);
        assert_eq!(e.dir, Some([0.1, 0.9, 0.0]));
        assert_eq!(e.frustum.as_ref().unwrap().center, [1.0, 2.0, 3.0]);
        assert_eq!(m.sweeps[0].dirs, vec![[0.0, 1.0, 0.0]]);
        assert_eq!(game_dirs(&m, 1), vec![[0.0; 3], [0.0; 3], [0.0; 3], [0.0; 3], [0.0; 3], [0.0; 3], [0.0; 3], [0.1, 0.9, 0.0]], "sweep 1 has no list: built from the entries' dir, dense up to direction 7");
    }
}
