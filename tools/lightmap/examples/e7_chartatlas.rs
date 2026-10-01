//! `e7_chartatlas MAP.Map.Gbx DUMPDIR --pass P --sweep S --direction D --records records.tsv --out plane.f32` — compose a `--dump-passes`
//! per-chart pass (ilightdir: one small `chart_ss` raster per item chart) into the 2048² atlas plane (raw f32 × 3, the e7_ilin input):
//! the manifest's chart `obj` → the records table's chart index → the map's mapping rect (E7, 2026-09-30: RI's k736 contribution per
//! record against the game's accumulate difference).
fn main() {
    let a: Vec<String> = std::env::args().collect();
    let f = |k: &str| a.iter().position(|x| x == k).and_then(|i| a.get(i + 1)).cloned();
    if a.len() < 3 { eprintln!("usage: e7_chartatlas MAP DUMPDIR --pass P --sweep S --direction D --records TSV --out F"); std::process::exit(2); }
    let m = lightmap::mapio::load(&a[1]).unwrap_or_else(|e| panic!("{}: {e}", a[1]));
    let d = m.chunk.data.as_ref().expect("no lightmap");
    let mp = d.cache.mapping().expect("no mapping");
    let dump = std::path::PathBuf::from(&a[2]);
    let pass = f("--pass").unwrap_or_else(|| "ilightdir".into());
    let sweep: u64 = f("--sweep").and_then(|v| v.parse().ok()).unwrap_or(0);
    let dir: u64 = f("--direction").and_then(|v| v.parse().ok()).expect("--direction");
    let out = f("--out").expect("--out");
    // obj → chart index (the records table)
    let mut chart_of_obj: std::collections::HashMap<u64, usize> = Default::default();
    let tsv = std::fs::read_to_string(f("--records").expect("--records")).expect("records");
    let mut cols: Vec<String> = Vec::new();
    for (i, l) in tsv.lines().enumerate() {
        let v: Vec<&str> = l.split('\t').collect();
        if i == 0 { cols = v.iter().map(|s| s.to_string()).collect(); continue; }
        let ci = cols.iter().position(|c| c == "chart").expect("chart col");
        let oi = cols.iter().position(|c| c == "obj").expect("obj col");
        if let (Ok(c), Ok(o)) = (v[ci].parse::<usize>(), v[oi].parse::<u64>()) { chart_of_obj.entry(o).or_insert(c); }
    }
    let manifest: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(dump.join("MANIFEST.json")).expect("manifest")).expect("json");
    // --increment N: the plane becomes the accumulate's INCREMENT for this direction, 4/N · max(0, n·D) × radiance, with n from the
    // lm_nrm chart raster and D the entry's `dir` (the manifest's own rule; N = the sweep's direction count)
    let inc_n: Option<f32> = f("--increment").and_then(|v| v.parse().ok());
    let nrm_file_of: std::collections::HashMap<u64, (String, u32, u32)> = manifest["passes"].as_array().into_iter().flatten().filter(|e| e["pass"].as_str() == Some("lm_nrm")).filter_map(|e| Some((e["chart"]["obj"].as_u64()?, (e["file"].as_str()?.to_string(), e["width"].as_u64()? as u32, e["height"].as_u64()? as u32)))).collect();
    let (w, h) = (2048u32, 2048u32);
    let scale = w / mp.atlas_w.max(1) as u32;
    let mut plane = vec![0f32; (w * h * 3) as usize];
    let (mut n, mut placed, mut missing) = (0usize, 0usize, 0usize);
    for e in manifest["passes"].as_array().into_iter().flatten() {
        if e["pass"].as_str() != Some(pass.as_str()) || e["sweep"].as_u64() != Some(sweep) || e["direction"].as_u64() != Some(dir) { continue; }
        n += 1;
        let obj = e["chart"]["obj"].as_u64().unwrap_or(u64::MAX);
        let Some(&c) = chart_of_obj.get(&obj) else { missing += 1; continue };
        let (x0, y0) = mp.pos[c];
        let (cw, ch) = mp.size[c];
        let (ew, eh) = (e["width"].as_u64().unwrap() as u32, e["height"].as_u64().unwrap() as u32);
        let bytes = std::fs::read(dump.join(e["file"].as_str().unwrap())).expect("chart file");
        let buf = lightmap::passdiff::decode_raw(&bytes, lightmap::passdiff::parse_format(e["format"].as_str().unwrap()), ew, eh, 0).expect("decode");
        let dvec: Option<[f32; 3]> = e["dir"].as_array().map(|v| [v[0].as_f64().unwrap() as f32, v[1].as_f64().unwrap() as f32, v[2].as_f64().unwrap() as f32]);
        let nrm: Option<lightmap::passdiff::Buf> = match (inc_n, nrm_file_of.get(&obj)) { (Some(_), Some((nf, nw, nh))) => { let b = std::fs::read(dump.join(nf)).expect("nrm file"); Some(lightmap::passdiff::decode_raw(&b, lightmap::passdiff::parse_format("R32G32B32_FLOAT"), *nw, *nh, 0).expect("nrm")) } _ => None };
        // the chart raster is the chart's rect at the plane's resolution (chart_ss at ss 1 = stored size × scale)
        for y in 0..eh.min(ch as u32 * scale) { for x in 0..ew.min(cw as u32 * scale) {
            let (ax, ay) = (x0 as u32 * scale + x, y0 as u32 * scale + y);
            let wgt = match (inc_n, &nrm, dvec) { (Some(n), Some(nb), Some(dv)) => { let nn = [nb.get(x, y, 0), nb.get(x, y, 1), nb.get(x, y, 2)]; 4.0 / n * (nn[0] * dv[0] + nn[1] * dv[1] + nn[2] * dv[2]).max(0.0) } _ => 1.0 };
            if ax < w && ay < h { for k in 0..3 { plane[((ay * w + ax) * 3 + k) as usize] = buf.get(x, y, k) * wgt; } }
        } }
        placed += 1;
    }
    let mut bytes = Vec::with_capacity(plane.len() * 4);
    for v in &plane { bytes.extend_from_slice(&v.to_le_bytes()); }
    std::fs::write(&out, bytes).expect("write");
    eprintln!("{pass} s{sweep} d{dir}: {n} chart entries, {placed} placed, {missing} without a record → {out} ({w}×{h} × 3 f32)");
}
