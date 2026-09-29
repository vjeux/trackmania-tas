//! `v5_hilltable MAP.Map.Gbx CLASSCMP_BYNAME.tsv [--min-texels 500] [--out TABLE.tsv]` — THE PER-MODEL G/B TABLE (verification
//! engineer V5, 2026-09-29; brief row 4a): for every embedded item model of a map, the class ratio ours/editor (r/g/b) from a
//! `classcmp --by name` table JOINED with what the model IS — its area-weighted pre-pass albedo (the bounce constant the peel
//! draws it with, `ModelGeom::mat_albedo`, the PyPxz_Hue recolour included), that albedo's SATURATION (1 − min/max) and B/R, its
//! dominant material link, and where it stands (mean placement height, mean horizontal distance to the placements' centroid).
//! Then the reads E6 needs: the G/B deficit (ratio_g/ratio_r, ratio_b/ratio_r) against the albedo saturation, against the
//! height and against the distance — Pearson r over the models with ≥ `--min-texels` lit texels, and the means per saturation
//! quartile / height band / distance band. g23's hills read 1.084/0.951/0.894 as a class: is the deficit a function of how
//! saturated the model's own colour is (the bounce chain on saturated albedo), of how high it stands (the sky share) or of where
//! it stands (the skirt / footprint edge)?
use std::collections::BTreeMap;

#[derive(Default, Clone)]
struct Row {
    model: String,
    placements: usize,
    mean_y: f64,
    mean_dist: f64,
    albedo: [f64; 3],
    area: f64,
    dominant: String,
    dominant_share: f64,
    /// the area-weighted TargetColor instance override over the slots that carry one (the PyPxz_Hue CustomPlastic hills: the colour
    /// the pre-pass recolours the mask toward) and that area's share of the model
    tc: [f64; 3],
    tc_area: f64,
    /// the area share of the slots whose bounce albedo is NOT a constant (textured / HueMask classes: `mat_albedo` NaN)
    textured_area: f64,
    texels: usize,
    lit_pct: f64,
    ratio: [f64; 3],
    mean_ours: [f64; 3],
    mean_editor: [f64; 3],
}

fn parse_rgb(s: &str) -> Option<[f64; 3]> {
    let v: Vec<f64> = s.split('/').map(|x| x.trim().parse::<f64>().unwrap_or(f64::NAN)).collect();
    if v.len() == 3 { Some([v[0], v[1], v[2]]) } else { None }
}

fn pearson(xs: &[f64], ys: &[f64]) -> f64 {
    let n = xs.len() as f64;
    if n < 3.0 { return f64::NAN; }
    let (mx, my) = (xs.iter().sum::<f64>() / n, ys.iter().sum::<f64>() / n);
    let (mut sxy, mut sxx, mut syy) = (0.0, 0.0, 0.0);
    for (x, y) in xs.iter().zip(ys) { sxy += (x - mx) * (y - my); sxx += (x - mx) * (x - mx); syy += (y - my) * (y - my); }
    if sxx <= 0.0 || syy <= 0.0 { f64::NAN } else { sxy / (sxx * syy).sqrt() }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let f = |k: &str| args.iter().position(|x| x == k).and_then(|i| args.get(i + 1)).cloned();
    let map = args.get(1).expect("MAP.Map.Gbx");
    let table = args.get(2).expect("CLASSCMP_BYNAME.tsv");
    let min_texels: usize = f("--min-texels").map(|v| v.parse().expect("--min-texels N")).unwrap_or(500);
    let scene = lightmap::geometry::Scene::from_map(map).expect("scene");
    // the class table: item:<model> rows → texels, lit editor %, the means and the ratio
    let txt = std::fs::read_to_string(table).unwrap_or_else(|e| panic!("{table}: {e}"));
    let mut cls: BTreeMap<String, (usize, f64, [f64; 3], [f64; 3], [f64; 3])> = Default::default();
    for line in txt.lines() {
        if line.starts_with('#') || line.starts_with("class\t") || line.trim().is_empty() { continue; }
        let c: Vec<&str> = line.split('\t').collect();
        if c.len() < 8 { continue; }
        let Some(name) = c[0].strip_prefix("item:") else { continue };
        let name = name.split(" (").next().unwrap_or(name).to_string(); // a --near / --density suffix is dropped
        let texels: usize = c[2].trim().parse().unwrap_or(0);
        let lit: f64 = c[4].trim().parse().unwrap_or(0.0);
        let (Some(mo), Some(me), Some(r)) = (parse_rgb(c[5]), parse_rgb(c[6]), parse_rgb(c[7])) else { continue };
        cls.insert(name, (texels, lit, mo, me, r));
    }
    // per model: placements, mean height, the centroid distance, the area-weighted albedo
    let mut rows: BTreeMap<usize, Row> = Default::default();
    let n_inst = scene.instances.len() as f64;
    let (cx, cz) = scene.instances.iter().fold((0.0, 0.0), |(sx, sz), i| (sx + i.pose.pos[0] as f64 / n_inst, sz + i.pose.pos[2] as f64 / n_inst));
    for inst in &scene.instances {
        let r = rows.entry(inst.model).or_default();
        if r.model.is_empty() { r.model = inst.model_name.clone(); }
        r.placements += 1;
        r.mean_y += inst.pose.pos[1] as f64;
        let (dx, dz) = (inst.pose.pos[0] as f64 - cx, inst.pose.pos[2] as f64 - cz);
        r.mean_dist += (dx * dx + dz * dz).sqrt();
    }
    for (mi, r) in rows.iter_mut() {
        r.mean_y /= r.placements.max(1) as f64;
        r.mean_dist /= r.placements.max(1) as f64;
        let m = &scene.models[*mi];
        let mut per: BTreeMap<usize, f64> = Default::default();
        for t in &m.tris {
            let a: Vec<f64> = (0..3).map(|k| (t.p[1][k] - t.p[0][k]) as f64).collect();
            let b: Vec<f64> = (0..3).map(|k| (t.p[2][k] - t.p[0][k]) as f64).collect();
            let c = [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]];
            let area = 0.5 * (c[0] * c[0] + c[1] * c[1] + c[2] * c[2]).sqrt();
            let k = t.mat as usize;
            *per.entry(k).or_default() += area;
            r.area += area;
            match m.mat_albedo.get(k) { Some(al) if al.iter().all(|v| v.is_finite()) => { for ch in 0..3 { r.albedo[ch] += area * al[ch] as f64; } } _ => r.textured_area += area }
            if let Some(Some(t)) = m.mat_params.get(k) { r.tc_area += area; for ch in 0..3 { r.tc[ch] += area * t[ch] as f64; } }
        }
        // the constant albedo mean runs over the constant-albedo area only; the TargetColor mean over the overridden area
        let const_area = r.area - r.textured_area;
        if const_area > 0.0 { for ch in 0..3 { r.albedo[ch] /= const_area; } }
        if r.tc_area > 0.0 { for ch in 0..3 { r.tc[ch] /= r.tc_area; } }
        if let Some((k, a)) = per.iter().max_by(|x, y| x.1.partial_cmp(y.1).unwrap()) {
            r.dominant = m.mat_links.get(*k).cloned().unwrap_or_else(|| format!("(mat {k})")).rsplit('\\').next().unwrap_or("").to_string();
            r.dominant_share = 100.0 * a / r.area.max(1e-9);
        }
        if let Some((tex, lit, mo, me, ratio)) = cls.get(&r.model) { r.texels = *tex; r.lit_pct = *lit; r.mean_ours = *mo; r.mean_editor = *me; r.ratio = *ratio; }
    }
    let mut list: Vec<Row> = rows.into_values().filter(|r| r.texels > 0).collect();
    list.sort_by(|a, b| b.texels.cmp(&a.texels));
    let sat = |a: &[f64; 3]| { let (mx, mn) = (a[0].max(a[1]).max(a[2]), a[0].min(a[1]).min(a[2])); if mx > 1e-9 { 1.0 - mn / mx } else { 0.0 } };
    let mut out = String::from("model\tplacements\ttexels\tlit_editor_pct\tratio_rgb\tg_over_r\tb_over_r\tmean_ours_rgb\tmean_editor_rgb\teditor_b_over_r\talbedo_rgb\talbedo_sat\talbedo_b_over_r\ttextured_area_pct\ttargetcolor_rgb\ttargetcolor_area_pct\ttargetcolor_sat\tdominant_material\tdominant_area_pct\tmean_y\tmean_dist_m\n");
    println!("{:<15} {:>4} {:>7} {:>5}  {:<21} {:>5} {:>5} {:>6}  {:<19} {:>4} {:>4} {:>5}  {:<19} {:>4} {:>4}  {:<26} {:>6} {:>6}", "model", "plc", "texels", "lit%", "ratio ours/editor r/g/b", "G/R", "B/R", "edB/R", "albedo r/g/b (const)", "sat", "tex%", "TC%", "TargetColor r/g/b", "sat", "B/R", "dominant material (area %)", "mean y", "dist m");
    for r in &list {
        let (gr, br) = (r.ratio[1] / r.ratio[0], r.ratio[2] / r.ratio[0]);
        let s = sat(&r.albedo);
        let abr = if r.albedo[0] > 1e-9 { r.albedo[2] / r.albedo[0] } else { f64::NAN };
        let edbr = if r.mean_editor[0] > 1e-9 { r.mean_editor[2] / r.mean_editor[0] } else { f64::NAN };
        let (tex_pct, tc_pct) = (100.0 * r.textured_area / r.area.max(1e-9), 100.0 * r.tc_area / r.area.max(1e-9));
        let (tcs, tcbr) = (sat(&r.tc), if r.tc[0] > 1e-9 { r.tc[2] / r.tc[0] } else { f64::NAN });
        let r3 = |v: &[f64; 3], p: usize| format!("{:.*} / {:.*} / {:.*}", p, v[0], p, v[1], p, v[2]);
        let opt3 = |v: &[f64; 3], area: f64, p: usize| if area > 0.0 { r3(v, p) } else { "—".to_string() };
        println!("{:<15} {:>4} {:>7} {:>5.1}  {:<21} {:>5.3} {:>5.3} {:>6.3}  {:<19} {:>4.2} {:>4.0} {:>5.0}  {:<19} {:>4.2} {:>4.2}  {:<26} {:>6.1} {:>6.0}", r.model.trim_end_matches(".Item.Gbx"), r.placements, r.texels, r.lit_pct, r3(&r.ratio, 3), gr, br, edbr, opt3(&r.albedo, r.area - r.textured_area, 2), s, tex_pct, tc_pct, opt3(&r.tc, r.tc_area, 2), tcs, tcbr, format!("{} ({:.0} %)", r.dominant, r.dominant_share), r.mean_y, r.mean_dist);
        out.push_str(&format!("{}\t{}\t{}\t{:.1}\t{}\t{:.4}\t{:.4}\t{}\t{}\t{:.4}\t{}\t{:.3}\t{:.3}\t{:.1}\t{}\t{:.1}\t{:.3}\t{}\t{:.1}\t{:.1}\t{:.1}\n", r.model, r.placements, r.texels, r.lit_pct, r3(&r.ratio, 3), gr, br, r3(&r.mean_ours, 4), r3(&r.mean_editor, 4), edbr, r3(&r.albedo, 3), s, abr, tex_pct, r3(&r.tc, 3), tc_pct, tcs, r.dominant, r.dominant_share, r.mean_y, r.mean_dist));
    }
    // the reads: over the models with ≥ min_texels lit texels
    let big: Vec<&Row> = list.iter().filter(|r| (r.texels as f64 * r.lit_pct / 100.0) as usize >= min_texels && r.ratio.iter().all(|v| v.is_finite() && *v > 0.0)).collect();
    let gr: Vec<f64> = big.iter().map(|r| r.ratio[1] / r.ratio[0]).collect();
    let br: Vec<f64> = big.iter().map(|r| r.ratio[2] / r.ratio[0]).collect();
    let ys: Vec<f64> = big.iter().map(|r| r.mean_y).collect();
    let ds: Vec<f64> = big.iter().map(|r| r.mean_dist).collect();
    let edbr: Vec<f64> = big.iter().map(|r| if r.mean_editor[0] > 1e-9 { r.mean_editor[2] / r.mean_editor[0] } else { 0.0 }).collect();
    // the TargetColor read over the models whose overridden (CustomPlastic) area is ≥ 10 %
    let tcm: Vec<&&Row> = big.iter().filter(|r| r.tc_area >= 0.1 * r.area).collect();
    let tc_s: Vec<f64> = tcm.iter().map(|r| sat(&r.tc)).collect();
    let tc_gr: Vec<f64> = tcm.iter().map(|r| r.ratio[1] / r.ratio[0]).collect();
    let tc_br: Vec<f64> = tcm.iter().map(|r| r.ratio[2] / r.ratio[0]).collect();
    // texel-weighted class-level G/R and B/R
    let wsum: f64 = big.iter().map(|r| r.texels as f64).sum();
    let wgr: f64 = big.iter().map(|r| r.texels as f64 * r.ratio[1] / r.ratio[0]).sum::<f64>() / wsum.max(1.0);
    let wbr: f64 = big.iter().map(|r| r.texels as f64 * r.ratio[2] / r.ratio[0]).sum::<f64>() / wsum.max(1.0);
    println!("\n== {} models with ≥ {min_texels} lit texels ({} texels): texel-weighted G/R {wgr:.3}, B/R {wbr:.3}", big.len(), wsum as usize);
    println!("Pearson r of the deficit (G/R, B/R of the ratio) against: mean height {:+.3} / {:+.3}; centroid distance {:+.3} / {:+.3}; the EDITOR's own B/R of the class {:+.3} / {:+.3}",
        pearson(&ys, &gr), pearson(&ys, &br), pearson(&ds, &gr), pearson(&ds, &br), pearson(&edbr, &gr), pearson(&edbr, &br));
    println!("Pearson r against the TargetColor saturation over the {} models with ≥ 10 % overridden (CustomPlastic) area: {:+.3} / {:+.3}", tcm.len(), pearson(&tc_s, &tc_gr), pearson(&tc_s, &tc_br));
    let band = |label: &str, key: &dyn Fn(&Row) -> f64, edges: &[f64]| {
        println!("\nby {label} (bins at {:?}): n models · texels · mean ratio r/g/b (texel-weighted) · G/R · B/R", edges);
        let mut lo = f64::NEG_INFINITY;
        let mut bounds: Vec<f64> = edges.to_vec(); bounds.push(f64::INFINITY);
        for hi in bounds {
            let sel: Vec<&&Row> = big.iter().filter(|r| { let v = key(r); v > lo && v <= hi }).collect();
            if !sel.is_empty() {
                let w: f64 = sel.iter().map(|r| r.texels as f64).sum();
                let m: Vec<f64> = (0..3).map(|c| sel.iter().map(|r| r.texels as f64 * r.ratio[c]).sum::<f64>() / w).collect();
                println!("  {:>8} < v ≤ {:<8}  {:>3} models  {:>9} texels  {:.3}/{:.3}/{:.3}  G/R {:.3}  B/R {:.3}", if lo.is_finite() { format!("{lo:.2}") } else { "-∞".into() }, if hi.is_finite() { format!("{hi:.2}") } else { "∞".into() }, sel.len(), w as usize, m[0], m[1], m[2], m[1] / m[0], m[2] / m[0]);
            }
            lo = hi;
        }
    };
    band("TargetColor saturation (models with ≥ 10 % CustomPlastic area; others = −1)", &|r: &Row| if r.tc_area >= 0.1 * r.area { sat(&r.tc) } else { -1.0 }, &[-0.5, 0.1, 0.3, 0.5, 0.7]);
    band("the EDITOR's class B/R (how blue the game lights the model)", &|r: &Row| if r.mean_editor[0] > 1e-9 { r.mean_editor[2] / r.mean_editor[0] } else { 0.0 }, &[1.0, 1.2, 1.4, 1.6, 1.8]);
    band("mean placement height y (m)", &|r: &Row| r.mean_y, &[8.0, 16.0, 32.0, 64.0, 128.0, 256.0]);
    band("mean distance to the placements' centroid (m)", &|r: &Row| r.mean_dist, &[250.0, 500.0, 1000.0, 2000.0]);
    if let Some(p) = f("--out") { std::fs::write(&p, out).unwrap_or_else(|e| panic!("{p}: {e}")); eprintln!("table → {p}"); }
}
