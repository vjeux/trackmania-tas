//! RE 7 scratch: per item model of a baked map, the PreLightGen fields the game's
//! chart filter reads against the editor's chart table (which items got a chart).
//!
//!   cargo run --release -p lightmap --example re7_items -- MAP.Gbx [--base 4096] [--all]
use std::collections::BTreeMap;

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let base: u32 = a.iter().position(|x| x == "--base").and_then(|i| a.get(i + 1)).map(|s| s.parse().unwrap()).unwrap_or(4096);
    let all = a.iter().any(|x| x == "--all");
    let m = tmmaps::map::MapFile::load(std::path::Path::new(&a[1]));
    let files = mapgeom::embedded::files(&m).expect("embedded");
    let mut by_name: BTreeMap<String, &Vec<u8>> = BTreeMap::new();
    for (k, v) in &files {
        by_name.insert(k.rsplit(['/', '\\']).next().unwrap_or(k).to_string(), v);
    }
    let own = lightmap::mapio::load(&a[1]).expect("own");
    let d = own.chunk.data.as_ref().unwrap();
    let mp = d.cache.mapping().unwrap();
    let mut charts: BTreeMap<u32, Vec<(u32, u16, u16, u16, u16)>> = BTreeMap::new();
    for i in 0..mp.count as usize {
        let obj = mp.binds[i].obj_group_idx / 4;
        charts.entry(obj).or_default().push((mp.binds[i].obj_idx, mp.pos[i].0, mp.pos[i].1, mp.size[i].0, mp.size[i].1));
    }
    // per-item lightmap quality bytes (chunk 0x03043068: blocks, baked, items)
    let lm_quality: Vec<u8> = tmmaps::gbx::all_skip_chunks(&m.gbx.body).iter().find(|(c, ..)| *c == 0x0304_3068).map(|&(_, _, payload, size)| {
        let start = payload + 4 + m.blocks.len() + m.baked.len();
        m.gbx.body[start.min(payload + size)..(payload + size).min(start + m.items.len())].to_vec()
    }).unwrap_or_default();
    #[derive(Default, Debug)]
    struct Agg { with: usize, without: usize, info: String, items_without: Vec<usize>, items_with: Vec<usize>, q_with: BTreeMap<u8, usize>, q_without: BTreeMap<u8, usize> }
    let mut agg: BTreeMap<String, Agg> = BTreeMap::new();
    for (i, it) in m.items.iter().enumerate() {
        let obj = base + i as u32;
        let has = charts.contains_key(&obj);
        let e = agg.entry(it.model.clone()).or_default();
        if has { e.with += 1; e.items_with.push(i); *e.q_with.entry(lm_quality.get(i).copied().unwrap_or(255)).or_default() += 1; } else { e.without += 1; e.items_without.push(i); *e.q_without.entry(lm_quality.get(i).copied().unwrap_or(255)).or_default() += 1; }
        if e.info.is_empty() {
            e.info = match by_name.get(&it.model) {
                None => "NOT EMBEDDED".to_string(),
                Some(bytes) => describe(bytes),
            };
        }
    }
    let mut n_with = 0; let mut n_without = 0;
    for (name, e) in &agg {
        n_with += e.with; n_without += e.without;
        if all || e.without > 0 {
            println!("{name}: charts {}/{} (q with {:?} without {:?}) {}", e.with, e.with + e.without, e.q_with, e.q_without, e.info);
            if e.without > 0 && e.with > 0 { println!("   MIXED: with {:?} without {:?}", &e.items_with[..e.items_with.len().min(6)], &e.items_without[..e.items_without.len().min(6)]); }
        }
    }
    println!("items with a chart {n_with}, without {n_without}, models {}", agg.len());
    if a.iter().any(|x| x == "--holes") { holes(mp, 1); }
    if a.iter().any(|x| x == "--area") { area_check(&m, &by_name, &charts, base); }
    if a.iter().any(|x| x == "--implied-s") { implied_s(&m, &by_name, &charts, base); }
    if let Some(i) = a.iter().position(|x| x == "--scan") { let missing: f64 = a[i + 1].parse().unwrap(); per_model_scan(&m, &by_name, &charts, base, missing); }
    if let Some(i) = a.iter().position(|x| x == "--kept") { kept_check(&m, &charts, base, &a[i + 1]); }
    if a.iter().any(|x| x == "--sum") {
        let mut total = 0f32;
        for c in &d.cache.chunks { if c.id == 0x0602_200B { if let lightmap::format::ChunkBody::Raw(b) = &c.body { if b.len() >= 8 { total = f32::from_le_bytes([b[4], b[5], b[6], b[7]]); } } } }
        sum_area(&m, &by_name, &charts, base, total);
    }
    // multi-chart objects
    for (obj, v) in &charts { if v.len() > 1 { println!("obj {obj} has {} charts: {:?}", v.len(), v); } }
}

fn describe(bytes: &[u8]) -> String {
    use mapgeom::static_item::Node;
    let f = match mapgeom::static_item::file::parse_file(bytes) { Ok(f) => f, Err(e) => return format!("PARSE ERROR {e}") };
    let Some(so) = f.item.static_object() else {
        let kind = f.item.model().and_then(|mc| mc.entity_model.inline.as_deref().map(|n| format!("{:?}", std::mem::discriminant(n)))).unwrap_or("?".into());
        return format!("NO STATIC OBJECT (prefab {}) entity {kind}", f.item.prefab().is_some());
    };
    let Some(s2) = so.solid2() else { return "NO SOLID2".to_string() };
    let mut s = String::new();
    match &s2.pre_light_gen {
        None => s += "PLG none",
        Some(p) => {
            s += &format!("PLG v{} u01 {} u02 {:.4} u03 {} uv0 [{:.4} {:.4} {:.4} {:.4}] uv1 [{:.4} {:.4} {:.4} {:.4}] sprite {:?} boxes {} uvgroups {}", p.version, p.u01, p.u02, p.u03, p.u04[0], p.u04[1], p.u04[2], p.u04[3], p.u04[4], p.u04[5], p.u04[6], p.u04[7], p.sprite_count, p.boxes.len(), p.uv_groups.len());
            if !p.uv_groups.is_empty() { s += &format!(" groups {:?}", &p.uv_groups[..p.uv_groups.len().min(4)]); }
        }
    }
    s += &format!(" | geoms {} visuals {} vis_cst {} mats {} custom {}", s2.shaded_geoms.len(), s2.visuals.len(), s2.vis_cst_type, s2.materials.len(), s2.custom_materials.len());
    let mut lods: Vec<i32> = s2.shaded_geoms.iter().map(|g| g.lod_mask).collect(); lods.sort(); lods.dedup();
    s += &format!(" lods {:?}", lods);
    // per visual: texcoord set count (stream decl TEXCOORD1 present?)
    let mut uvsets: Vec<String> = Vec::new();
    for vr in s2.visuals.iter().take(20) {
        let Some(Node::Visual(v)) = vr.inline.as_deref() else { uvsets.push("?".into()); continue };
        let n_inline = v.main.as_ref().map(|m| m.tex_coord_sets.len()).unwrap_or(0);
        let n_stream = v.stream().map(|st| st.decls.iter().filter(|d| d.name() >= mapgeom::static_item::vstream::N_TEXCOORD0 && d.name() < mapgeom::static_item::vstream::N_TEXCOORD0 + 8).count()).unwrap_or(0);
        let nv = v.main.as_ref().map(|m| m.count).unwrap_or(-1);
        let ug = v.main.as_ref().map(|m| m.uv_groups.len()).unwrap_or(0);
        uvsets.push(format!("{n_inline}/{n_stream}:{nv}:g{ug}"));
    }
    s += &format!(" uv(inline/stream:verts:uvgroups) {}", uvsets.join(","));
    // material links
    let mut links: Vec<String> = Vec::new();
    for cm in &s2.custom_materials { let l = cm.inst().and_then(|m| m.link().map(|l| l.to_string())).unwrap_or_else(|| cm.name.clone()); if !links.contains(&l) { links.push(l); } }
    for mr in &s2.materials { if let Some(Node::Material(mm)) = mr.inline.as_deref() { if let Some(l) = mm.link() { let l = l.to_string(); if !links.contains(&l) { links.push(l); } } } }
    s += &format!(" links {:?}", links);
    s
}

/// Free-space analysis of the editor's layout: occupancy of the 2048² layout by the chart rects (+pad), then
/// the maximal free rectangles found greedily (largest first). Big square holes = packed-but-unbound charts.
pub fn holes(mp: &lightmap::format::Mapping, pad: u32) {
    let w = 2048usize; let h = 2048usize;
    let mut occ = vec![false; w * h];
    let mut area = 0usize;
    for i in 0..mp.count as usize {
        let (x, y) = (mp.pos[i].0 as usize, mp.pos[i].1 as usize);
        let (cw, ch) = (mp.size[i].0 as usize + 2 * pad as usize, mp.size[i].1 as usize + 2 * pad as usize);
        let x0 = x.saturating_sub(pad as usize); let y0 = y.saturating_sub(pad as usize);
        for yy in y0..(y0 + ch).min(h) { for xx in x0..(x0 + cw).min(w) { if !occ[yy * w + xx] { occ[yy * w + xx] = true; area += 1; } } }
    }
    println!("occupied {} of {} layout cells ({:.2} %)", area, w * h, 100.0 * area as f64 / (w * h) as f64);
    // greedy maximal free rectangles: scan for the largest free square-ish rect repeatedly (cheap version:
    // per free cell, the largest square with that top-left corner via a DP, then take the top 40 disjoint)
    let mut sq = vec![0u16; w * h];
    for y in (0..h).rev() { for x in (0..w).rev() {
        if occ[y * w + x] { continue; }
        let r = if x + 1 < w { sq[y * w + x + 1] } else { 0 };
        let d = if y + 1 < h { sq[(y + 1) * w + x] } else { 0 };
        let rd = if x + 1 < w && y + 1 < h { sq[(y + 1) * w + x + 1] } else { 0 };
        sq[y * w + x] = 1 + r.min(d).min(rd);
    } }
    let mut hist: std::collections::BTreeMap<u16, usize> = Default::default();
    let mut taken = vec![false; w * h];
    let mut cells: Vec<(u16, usize)> = sq.iter().enumerate().filter(|(_, &s)| s >= 6).map(|(i, &s)| (s, i)).collect();
    cells.sort_by(|a, b| b.0.cmp(&a.0));
    let mut shown = 0;
    for (s, i) in cells {
        let (x, y) = (i % w, i / w);
        if taken[i] { continue; }
        let s = s as usize;
        if (y..y + s).any(|yy| (x..x + s).any(|xx| taken[yy * w + xx])) { continue; }
        for yy in y..y + s { for xx in x..x + s { taken[yy * w + xx] = true; } }
        *hist.entry(s as u16).or_default() += 1;
        if shown < 30 { println!("  free square {s}×{s} at ({x}, {y})"); shown += 1; }
    }
    println!("free squares ≥ 6 (disjoint, greedy): {:?}", hist);
}

/// Per charted item: predicted PLG area (uvExt × MeterByUv)² with and without the placement scale, against the
/// editor's chart size w·h: the ratio must be one constant (1/s²) for the right formula.
pub fn area_check(m: &tmmaps::map::MapFile, by_name: &BTreeMap<String, &Vec<u8>>, charts: &BTreeMap<u32, Vec<(u32, u16, u16, u16, u16)>>, base: u32) {
    let mut rows: Vec<(f64, f64, f64, usize, String)> = Vec::new(); // (ratio_noscale, ratio_scale, wh, item, model)
    let mut cache: BTreeMap<String, Option<(f32, [f32; 4])>> = BTreeMap::new();
    for (i, it) in m.items.iter().enumerate() {
        let Some(v) = charts.get(&(base + i as u32)) else { continue };
        let (_, _, _, w, h) = v[0];
        let plg = cache.entry(it.model.clone()).or_insert_with(|| {
            let bytes = by_name.get(&it.model)?;
            let f = mapgeom::static_item::file::parse_file(bytes).ok()?;
            let s2 = f.item.static_object()?.solid2()?;
            let p = s2.pre_light_gen.as_ref()?;
            Some((p.u02, [p.u04[0], p.u04[1], p.u04[2], p.u04[3]]))
        });
        let Some((mbu, b)) = plg else { continue };
        let ext = [(b[2] - b[0]) * *mbu, (b[3] - b[1]) * *mbu];
        let a0 = ext[0] as f64 * ext[1] as f64;
        let sc = it.scale as f64;
        let wh = w as f64 * h as f64;
        if wh > 0.0 { rows.push((a0 / wh, a0 * sc * sc / wh, wh, i, it.model.clone())); }
    }
    let stats = |k: usize| { let mut v: Vec<f64> = rows.iter().map(|r| if k == 0 { r.0 } else { r.1 }).collect(); v.sort_by(|a, b| a.partial_cmp(b).unwrap()); let n = v.len(); (v[n / 100], v[n / 4], v[n / 2], v[3 * n / 4], v[n - 1 - n / 100]) };
    println!("{} charted items; area/(w·h) percentiles 1/25/50/75/99: no scale {:?}, with scale² {:?}", rows.len(), stats(0), stats(1));
    let scales: std::collections::BTreeSet<String> = m.items.iter().map(|it| format!("{:.3}", it.scale)).collect();
    println!("placement scales present: {:?}", scales);
    // the biggest charts
    rows.sort_by(|a, b| b.2.partial_cmp(&a.2).unwrap());
    for r in rows.iter().take(8) { println!("  item {} {} w·h {} ratio {:.5} (scaled {:.5})", r.3, r.4, r.2, r.0, r.1); }
}

/// Σarea over the charted items with the game's per-record formula (uvExt × (MeterByUv × q), q = 1 for Normal),
/// f32 in the game's order, plus the per-kind tile counts the editor's table has — the residual is the tiles' share.
pub fn sum_area(m: &tmmaps::map::MapFile, by_name: &BTreeMap<String, &Vec<u8>>, charts: &BTreeMap<u32, Vec<(u32, u16, u16, u16, u16)>>, base: u32, total: f32) {
    let mut cache: BTreeMap<String, Option<(f32, [f32; 4])>> = BTreeMap::new();
    let (mut sum_f32, mut sum_f64, mut n) = (0f32, 0f64, 0usize);
    let mut dropped = 0f64; let mut nd = 0usize;
    for (i, it) in m.items.iter().enumerate() {
        let plg = cache.entry(it.model.clone()).or_insert_with(|| {
            let bytes = by_name.get(&it.model)?;
            let f = mapgeom::static_item::file::parse_file(bytes).ok()?;
            let s2 = f.item.static_object()?.solid2()?;
            let p = s2.pre_light_gen.as_ref()?;
            Some((p.u02, [p.u04[0], p.u04[1], p.u04[2], p.u04[3]]))
        });
        let Some((mbu, b)) = plg else { continue };
        let f = *mbu * 1.0f32;
        let ext = [(b[2] - b[0]) * f, (b[3] - b[1]) * f];
        let a = ext[0] * ext[1];
        if charts.contains_key(&(base + i as u32)) { sum_f32 += a; sum_f64 += a as f64; n += 1; } else { dropped += a as f64; nd += 1; }
    }
    println!("charted items {n}: Σarea f32 {sum_f32} (f64 {sum_f64:.1}); dropped items {nd}: Σ {dropped:.1}; editor TotalLmSurfaceMeter {total} → tiles' residual {:.1} m² over {} tile charts = {:.2} m²/tile", total as f64 - sum_f64, charts.keys().filter(|&&o| o < base).count(), (total as f64 - sum_f64) / charts.keys().filter(|&&o| o < base).count().max(1) as f64);
}

pub fn kept_check(m: &tmmaps::map::MapFile, charts: &BTreeMap<u32, Vec<(u32, u16, u16, u16, u16)>>, base: u32, kept_path: &str) {
    let kept: std::collections::BTreeSet<usize> = std::fs::read_to_string(kept_path).unwrap().split(',').filter_map(|s| s.trim().parse().ok()).collect();
    let (mut kept_nochart, mut dropped_chart) = (Vec::new(), Vec::new());
    for (i, it) in m.items.iter().enumerate() {
        let has = charts.contains_key(&(base + i as u32));
        if kept.contains(&i) && !has { kept_nochart.push((i, it.model.clone())); }
        if !kept.contains(&i) && has { dropped_chart.push((i, it.model.clone())); }
    }
    println!("kept {} items; kept-but-no-chart {:?}; dropped-but-charted {:?}", kept.len(), kept_nochart, dropped_chart.len());
}

/// Per charted item with a chart ≥ 20 layout units: the implied s = sqrt((w+2)(h+2)/area) with OUR area; a model
/// class whose implied s deviates from the bulk has a different game-side area.
pub fn implied_s(m: &tmmaps::map::MapFile, by_name: &BTreeMap<String, &Vec<u8>>, charts: &BTreeMap<u32, Vec<(u32, u16, u16, u16, u16)>>, base: u32) {
    let mut cache: BTreeMap<String, Option<(f32, [f32; 4])>> = BTreeMap::new();
    let mut per_model: BTreeMap<String, Vec<f64>> = BTreeMap::new();
    let mut all: Vec<f64> = Vec::new();
    for (i, it) in m.items.iter().enumerate() {
        let Some(v) = charts.get(&(base + i as u32)) else { continue };
        let (_, _, _, w, h) = v[0];
        if w < 20 || h < 20 { continue; }
        let plg = cache.entry(it.model.clone()).or_insert_with(|| {
            let bytes = by_name.get(&it.model)?;
            let f = mapgeom::static_item::file::parse_file(bytes).ok()?;
            let s2 = f.item.static_object()?.solid2()?;
            let p = s2.pre_light_gen.as_ref()?;
            Some((p.u02, [p.u04[0], p.u04[1], p.u04[2], p.u04[3]]))
        });
        let Some((mbu, b)) = plg else { continue };
        let ext = [(b[2] - b[0]) * *mbu, (b[3] - b[1]) * *mbu];
        let area = ext[0] as f64 * ext[1] as f64;
        let s = (((w as f64 + 2.0) * (h as f64 + 2.0)) / area).sqrt();
        per_model.entry(it.model.clone()).or_default().push(s);
        all.push(s);
    }
    all.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let n = all.len();
    println!("{n} charts ≥ 20: implied s percentiles 5/25/50/75/95: {:.4} {:.4} {:.4} {:.4} {:.4}", all[n / 20], all[n / 4], all[n / 2], all[3 * n / 4], all[n * 19 / 20]);
    let mut rows: Vec<(String, f64, usize)> = per_model.iter().map(|(k, v)| (k.clone(), v.iter().sum::<f64>() / v.len() as f64, v.len())).collect();
    rows.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap());
    for r in rows.iter().take(6) { println!("  low  {:.4} ×{} {}", r.1, r.2, r.0); }
    for r in rows.iter().rev().take(6) { println!("  high {:.4} ×{} {}", r.1, r.2, r.0); }
}

/// Hypothesis scan: the missing Σ term as a per-MODEL extra record (area counted once more per model) for models
/// with ≥ T charted instances, or per instance ×(k) … prints Σ_models area for thresholds T.
pub fn per_model_scan(m: &tmmaps::map::MapFile, by_name: &BTreeMap<String, &Vec<u8>>, charts: &BTreeMap<u32, Vec<(u32, u16, u16, u16, u16)>>, base: u32, missing: f64) {
    let mut cache: BTreeMap<String, Option<(f32, [f32; 4], usize, usize, Vec<i32>)>> = BTreeMap::new();
    let mut per_model: BTreeMap<String, (usize, f64)> = BTreeMap::new(); // (charted instances, area)
    for (i, it) in m.items.iter().enumerate() {
        if !charts.contains_key(&(base + i as u32)) { continue; }
        let plg = cache.entry(it.model.clone()).or_insert_with(|| {
            let bytes = by_name.get(&it.model)?;
            let f = mapgeom::static_item::file::parse_file(bytes).ok()?;
            let s2 = f.item.static_object()?.solid2()?;
            let p = s2.pre_light_gen.as_ref()?;
            let mut lods: Vec<i32> = s2.shaded_geoms.iter().map(|g| g.lod_mask).collect(); lods.sort(); lods.dedup();
            Some((p.u02, [p.u04[0], p.u04[1], p.u04[2], p.u04[3]], s2.visuals.len(), s2.lights.len(), lods))
        });
        let Some((mbu, b, _nv, _nl, _lods)) = plg else { continue };
        let ext = [(b[2] - b[0]) * *mbu, (b[3] - b[1]) * *mbu];
        let e = per_model.entry(it.model.clone()).or_insert((0, ext[0] as f64 * ext[1] as f64));
        e.0 += 1;
    }
    println!("missing {missing:.0} m²; models {}", per_model.len());
    for t in [1usize, 2, 4, 8, 16, 32, 64, 128] {
        let s: f64 = per_model.values().filter(|(n, _)| *n >= t).map(|(_, a)| a).sum();
        let n = per_model.values().filter(|(n, _)| *n >= t).count();
        println!("  per-model extra for models with ≥ {t:>3} instances: {n:>3} models, Σarea {s:.0} m² (ratio to missing {:.3})", s / missing);
    }
    // per-instance fraction: missing / Σ(items) and per LOD-count
    let items: f64 = per_model.values().map(|(n, a)| *n as f64 * a).sum();
    println!("  Σitems {items:.0}; missing/Σitems = {:.4}", missing / items);
    // by visual/lod class
    let mut by_lods: BTreeMap<String, f64> = BTreeMap::new();
    for (name, (n, a)) in &per_model { if let Some(Some((_, _, nv, nl, lods))) = cache.get(name) { *by_lods.entry(format!("lods {:?} lights {} visuals {}", lods, nl, nv.min(&99))).or_default() += *n as f64 * a; } }
    let mut v: Vec<(String, f64)> = by_lods.into_iter().collect(); v.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
    for (k, s) in v.iter().take(12) { println!("  class {k}: Σ {s:.0} ({:.3} of missing)", s / missing); }
}
