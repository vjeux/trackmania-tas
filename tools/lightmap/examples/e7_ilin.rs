//! `e7_ilin ORACLE.Map.Gbx GAME_ILIGHTINPUT.dds[.gz] [--charts a..b] [--chart N …]` — THE GAME'S SWEEP-0 EMISSION (the dilated
//! ILightInput plane of the first compute frame, 2048² R11G11B10) over the charts of the oracle map's mapping: per chart range the
//! texel count, the non-zero count, the mean colour and the mean of the non-zero texels — the emitter colour a receiver sees from
//! that class (E7, 2026-09-30: g23's 64 516 seabed tiles under 5 m of water: sand or water colour?). The mapping's positions are in
//! the 2048-wide layout (the pre-pass raster = the ILightInput plane's grid).
fn main() {
    let a: Vec<String> = std::env::args().collect();
    if a.len() < 3 { eprintln!("usage: e7_ilin ORACLE.Map.Gbx GAME_ILIGHTINPUT.dds[.gz] [--charts a..b] [--chart N …]"); std::process::exit(2); }
    let f = |k: &str| a.iter().position(|x| x == k).and_then(|i| a.get(i + 1)).cloned();
    let m = lightmap::mapio::load(&a[1]).unwrap_or_else(|e| panic!("{}: {e}", a[1]));
    let d = m.chunk.data.as_ref().expect("no lightmap");
    let mp = d.cache.mapping().expect("no mapping");
    eprintln!("mapping: {} charts, atlas {}×{}", mp.count, mp.atlas_w, mp.atlas_h);
    let p = &a[2];
    let bytes = std::fs::read(p).unwrap_or_else(|e| panic!("{p}: {e}"));
    let bytes = if p.ends_with(".gz") { lightmap::passdiff::gunzip(&bytes).expect("gunzip") } else { bytes };
    // a DDS export, or the port's raw f32 plane (`--chain-final-dir`: frommap-ilightinput.f32 = 2048² × 3 f32, little-endian)
    let buf = if bytes.len() >= 4 && &bytes[..4] == b"DDS " { lightmap::passdiff::load_dds_bytes(&bytes, "", 0, 0).unwrap_or_else(|e| panic!("{p}: {e}")) } else {
        let n = bytes.len() / 4;
        let ch = (n / (2048 * 2048)).max(1) as u32;
        let fmt = match ch { 1 => "R32_FLOAT", 3 => "R32G32B32_FLOAT", _ => "R32G32B32A32_FLOAT" };
        lightmap::passdiff::decode_raw(&bytes, lightmap::passdiff::parse_format(fmt), 2048, 2048, 0).unwrap_or_else(|e| panic!("{p}: {e}"))
    };
    eprintln!("plane {}×{} × {}", buf.w, buf.h, buf.channels);
    let scale = buf.w / mp.atlas_w.max(1);
    let stat = |lo: usize, hi: usize| {
        let (mut n, mut nz) = (0usize, 0usize);
        let (mut s, mut snz) = ([0f64; 3], [0f64; 3]);
        for c in lo..hi.min(mp.count as usize) {
            let (x0, y0) = mp.pos[c];
            let (w, h) = mp.size[c];
            for y in (y0 as u32 * scale)..((y0 as u32 + h as u32) * scale).min(buf.h) {
                for x in (x0 as u32 * scale)..((x0 as u32 + w as u32) * scale).min(buf.w) {
                    let q = [buf.get(x, y, 0), buf.get(x, y, 1), buf.get(x, y, 2)];
                    n += 1;
                    for k in 0..3 { s[k] += q[k] as f64; }
                    if q.iter().any(|v| *v != 0.0) { nz += 1; for k in 0..3 { snz[k] += q[k] as f64; } }
                }
            }
        }
        (n, nz, s, snz)
    };
    let print = |label: &str, (n, nz, s, snz): (usize, usize, [f64; 3], [f64; 3])| {
        println!("{label}: {n} texels, {nz} non-zero ({:.1} %); mean ({:.4}, {:.4}, {:.4}); mean over non-zero ({:.4}, {:.4}, {:.4}); G/R {:.2} B/R {:.2}", 100.0 * nz as f64 / n.max(1) as f64, s[0] / n.max(1) as f64, s[1] / n.max(1) as f64, s[2] / n.max(1) as f64, snz[0] / nz.max(1) as f64, snz[1] / nz.max(1) as f64, snz[2] / nz.max(1) as f64, snz[1] / snz[0].max(1e-9), snz[2] / snz[0].max(1e-9));
    };
    if let Some(r) = f("--charts") { let v: Vec<usize> = r.split("..").filter_map(|t| t.trim().parse().ok()).collect(); if v.len() == 2 { print(&format!("charts {}..{}", v[0], v[1]), stat(v[0], v[1])); } }
    // --records records.tsv --name SUBSTR [--name …]: every chart whose record name contains SUBSTR, aggregated (the bake's records.tsv:
    // chart \t class \t obj \t sub \t name …)
    if let Some(rp) = f("--records") {
        let txt = std::fs::read_to_string(&rp).unwrap_or_else(|e| panic!("{rp}: {e}"));
        let mut names: Vec<String> = a.iter().enumerate().filter(|(_, x)| *x == "--name").filter_map(|(i, _)| a.get(i + 1).cloned()).collect();
        // --all-names: every distinct record name (one aggregate row each, as a TSV: name, charts, texels, non-zero, mean r g b)
        if a.iter().any(|x| x == "--all-names") {
            let mut set: Vec<String> = txt.lines().skip(1).filter_map(|l| l.split('\t').nth(4).map(|s| s.to_string())).collect();
            set.sort(); set.dedup();
            names = set;
        }
        let tsv = a.iter().any(|x| x == "--tsv");
        if tsv { println!("name\tcharts\ttexels\tnonzero\tmean_r\tmean_g\tmean_b\tnz_r\tnz_g\tnz_b"); }
        for nm in names {
            let charts: Vec<usize> = txt.lines().skip(1).filter_map(|l| { let c: Vec<&str> = l.split('\t').collect(); if c.len() > 4 && (if tsv || a.iter().any(|x| x == "--all-names") { c[4] == nm } else { c[4].contains(&nm) }) { c[0].parse().ok() } else { None } }).collect();
            let mut tot = (0usize, 0usize, [0f64; 3], [0f64; 3]);
            let mut seen = std::collections::HashSet::new();
            for c in &charts { if !seen.insert(*c) { continue; } let s = stat(*c, *c + 1); tot.0 += s.0; tot.1 += s.1; for k in 0..3 { tot.2[k] += s.2[k]; tot.3[k] += s.3[k]; } }
            if tsv { let n = tot.0.max(1) as f64; let nz = tot.1.max(1) as f64; println!("{nm}\t{}\t{}\t{}\t{:.5}\t{:.5}\t{:.5}\t{:.5}\t{:.5}\t{:.5}", seen.len(), tot.0, tot.1, tot.2[0] / n, tot.2[1] / n, tot.2[2] / n, tot.3[0] / nz, tot.3[1] / nz, tot.3[2] / nz); }
            else { print(&format!("{nm}: {} charts", seen.len()), tot); }
        }
    }
    for (i, x) in a.iter().enumerate() { if x == "--chart" { if let Some(c) = a.get(i + 1).and_then(|v| v.parse::<usize>().ok()) { let (x0, y0) = mp.pos[c]; let (w, h) = mp.size[c]; print(&format!("chart {c} at ({x0}, {y0}) {w}×{h}"), stat(c, c + 1)); } } }
}
