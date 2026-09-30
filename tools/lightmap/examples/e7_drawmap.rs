//! `e7_drawmap draws-frameN.json.gz ORACLE.Map.Gbx records.tsv [--name SUBSTR …] [--top N]` — THE PRE-PASS DRAW → CHART → ITEM MAP of a
//! captured compute frame (E7, 2026-09-30): every draw's `DrawV.GbxVTexCoordToRasterLM` names its chart rect in the 2048² atlas
//! (x0 = (tx + 1)·1024, w = sx·1024; y from the flipped NDC); the oracle map's mapping gives the chart, records.tsv its item name. Per
//! item name: the pixel shaders used, the SRVs bound (id, size, format) and the ShaderP constants that carry a colour (RgbTargetColor,
//! …) — what the GAME shades each item's pre-pass with, beside what the port picked.
use std::collections::BTreeMap;
fn main() {
    let a: Vec<String> = std::env::args().collect();
    if a.len() < 4 { eprintln!("usage: e7_drawmap draws-frameN.json.gz ORACLE.Map.Gbx records.tsv [--name SUBSTR …] [--top N]"); std::process::exit(2); }
    let f = |k: &str| a.iter().position(|x| x == k).and_then(|i| a.get(i + 1)).cloned();
    let top: usize = f("--top").and_then(|v| v.parse().ok()).unwrap_or(60);
    let names: Vec<String> = a.iter().enumerate().filter(|(_, x)| *x == "--name").filter_map(|(i, _)| a.get(i + 1).cloned()).collect();
    let bytes = std::fs::read(&a[1]).unwrap_or_else(|e| panic!("{}: {e}", a[1]));
    let bytes = if a[1].ends_with(".gz") { lightmap::passdiff::gunzip(&bytes).expect("gunzip") } else { bytes };
    let txt = String::from_utf8_lossy(&bytes).to_string();
    let v: serde_json::Value = serde_json::from_str(&lightmap::passdiff::nan_free_json(&txt)).expect("json");
    let m = lightmap::mapio::load(&a[2]).unwrap_or_else(|e| panic!("{}: {e}", a[2]));
    let mp = m.chunk.data.as_ref().expect("lightmap").cache.mapping().expect("mapping");
    let rec = std::fs::read_to_string(&a[3]).expect("records");
    let mut name_of: Vec<String> = vec![String::new(); mp.count as usize];
    for l in rec.lines().skip(1) { let c: Vec<&str> = l.split('\t').collect(); if c.len() > 4 { if let Ok(i) = c[0].parse::<usize>() { if i < name_of.len() && name_of[i].is_empty() { name_of[i] = c[4].to_string(); } } } }
    // chart by (x0, y0)
    let mut by_pos: std::collections::HashMap<(u32, u32), usize> = Default::default();
    for c in 0..mp.count as usize { by_pos.entry((mp.pos[c].0 as u32, mp.pos[c].1 as u32)).or_insert(c); }
    #[derive(Default)] struct Tally { draws: usize, ps: BTreeMap<String, usize>, srvs: BTreeMap<String, usize>, consts: BTreeMap<String, usize>, blend: BTreeMap<String, usize> }
    let mut per: BTreeMap<String, Tally> = Default::default();
    let (mut n_draw, mut n_mapped, mut n_unmapped) = (0usize, 0usize, 0usize);
    for d in v.as_array().expect("array") {
        let Some(t) = d.pointer("/Vertex/cbuffers/DrawV/GbxVTexCoordToRasterLM") else { continue };
        n_draw += 1;
        let g = |i: usize, j: usize| t[i][j].as_f64().unwrap_or(0.0);
        let (sx, sy, tx, ty) = (g(0, 0), g(1, 1), g(3, 0), g(3, 1));
        // x0 = (tx + 1)·1024 ; y: NDC y up, raster y down: the chart's top row y0 = (1 − ty)·1024 − h where h = −sy·1024 … or ty is the top:
        // try both readings against the mapping
        let w = (sx.abs() * 1024.0).round() as u32;
        let h = (sy.abs() * 1024.0).round() as u32;
        let x0 = ((tx + 1.0) * 1024.0).round() as u32;
        let ya = ((1.0 - ty) * 1024.0).round() as u32;
        let yb = ((1.0 - ty) * 1024.0).round() as i64 - h as i64;
        let cand = [(x0, ya), (x0, yb.max(0) as u32), (x0, ((ty + 1.0) * 1024.0).round() as u32)];
        let chart = cand.iter().find_map(|k| by_pos.get(k).copied());
        let Some(c) = chart else { n_unmapped += 1; continue };
        n_mapped += 1;
        let name = name_of.get(c).cloned().unwrap_or_default();
        if !names.is_empty() && !names.iter().any(|n| name.contains(n)) { continue; }
        let e = per.entry(name).or_default();
        e.draws += 1;
        let ps = d.pointer("/Pixel/shader").and_then(|x| x.as_str()).unwrap_or("-").to_string();
        let vs = d.pointer("/Vertex/shader").and_then(|x| x.as_str()).unwrap_or("-").to_string();
        *e.ps.entry(format!("VS {vs} PS {ps}")).or_default() += 1;
        if let Some(srvs) = d.pointer("/Pixel/srvs").and_then(|x| x.as_array()) {
            let s: Vec<String> = srvs.iter().map(|s| format!("t{}={} {}×{} {}", s["slot"], s["tex"]["id"].as_str().unwrap_or("?"), s["tex"]["w"], s["tex"]["h"], s["tex"]["format"].as_str().unwrap_or("?").replace("_TYPELESS", "T").replace("_UNORM", "U"))).collect();
            *e.srvs.entry(s.join(" | ")).or_default() += 1;
        }
        if let Some(sp) = d.pointer("/Pixel/cbuffers/ShaderP").and_then(|x| x.as_object()) {
            let mut parts = Vec::new();
            for (k, val) in sp { if k.to_ascii_lowercase().contains("color") || k.to_ascii_lowercase().contains("colour") || k.contains("Hue") { parts.push(format!("{k}={}", serde_json::to_string(val).unwrap_or_default())); } }
            if !parts.is_empty() { *e.consts.entry(parts.join(" ")).or_default() += 1; }
        }
        if let Some(b) = d.pointer("/blend/0") { *e.blend.entry(format!("{}/{} a2c {}", b["src"].as_str().unwrap_or("?").replace("BlendMultiplier.", ""), b["dst"].as_str().unwrap_or("?").replace("BlendMultiplier.", ""), d["alphaToCoverage"].as_bool().map(|x| x.to_string()).unwrap_or("?".into()))).or_default() += 1; }
        let _ = w;
    }
    eprintln!("{n_draw} LM draws, {n_mapped} mapped to a chart, {n_unmapped} unmapped");
    let mut rows: Vec<(&String, &Tally)> = per.iter().collect();
    rows.sort_by_key(|(_, t)| std::cmp::Reverse(t.draws));
    for (name, t) in rows.iter().take(top) {
        println!("{name}: {} draws", t.draws);
        for (k, n) in &t.ps { println!("    {k} ×{n}"); }
        for (k, n) in t.srvs.iter().take(6) { println!("    srv {k} ×{n}"); }
        for (k, n) in t.consts.iter().take(6) { println!("    {k} ×{n}"); }
        for (k, n) in &t.blend { println!("    blend {k} ×{n}"); }
    }
}
