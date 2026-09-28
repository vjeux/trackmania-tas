//! `re15_tilefloor EDITOR.Map.Gbx --cells CENSUS.tsv [--inside X0,X1] [--ring]` — the editor's frame-0 tile-chart means by
//! CELL: the mean decoded HDR (Σrgb) of each ZONE TILE chart (bind obj < tile count), grouped by the cell's distance to the
//! decoration footprint (cells X0..=X1 in both axes = inside), so the "buried outside" question reads off the oracle itself
//! (RE 15, read 3: the giant sea floor beyond the Base64x64 footprint). CENSUS.tsv = `tmmaps census SOURCE.Map.Gbx` of the
//! map whose block list the tile charts are numbered by (block k ↔ chart with bind obj k).
use std::collections::HashMap;

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let f = |k: &str| a.iter().position(|x| x == k).and_then(|i| a.get(i + 1)).cloned();
    let map = lightmap::mapio::load(&a[1]).unwrap_or_else(|e| panic!("{e}"));
    let d = map.chunk.data.as_ref().expect("no lightmap");
    let m = d.cache.mapping().expect("no mapping");
    let img = lightmap::img::decode_webp(d.frames[0].images.first().expect("image 0")).unwrap_or_else(|e| panic!("{e}"));
    let maxhdr = lightmap::classcmp::record_maxhdr(&m, 0).expect("record");
    println!("{}: {} charts, image {}×{}, frame-0 MaxHDR {maxhdr}", a[1], m.count, img.w, img.h);
    // block index → (cx, cz) from the census (rows: src id name cx cy cz ...; blocks tagged U/B with ids like u12 / b12)
    let mut cell: HashMap<u32, (i32, i32, i32)> = HashMap::new();
    if let Some(c) = f("--cells") {
        let txt = std::fs::read_to_string(&c).expect("census");
        let mut k = 0u32;
        for line in txt.lines().skip(1) {
            let cols: Vec<&str> = line.split('\t').collect();
            if cols.len() < 6 || cols[0] == "I" { continue; }
            let (cx, cy, cz) = (cols[3].parse::<i32>().unwrap_or(-1), cols[4].parse::<i32>().unwrap_or(-1), cols[5].parse::<i32>().unwrap_or(-1));
            cell.insert(k, (cx, cy, cz));
            k += 1;
        }
        println!("{} blocks from {c}", cell.len());
    }
    if let Some(g) = f("--grid") { grid(&m, &img, maxhdr, &cell, g.parse().unwrap()); return; }
    let inside: (i32, i32) = f("--inside").map(|s| { let v: Vec<i32> = s.split(',').map(|x| x.parse().unwrap()).collect(); (v[0], v[1]) }).unwrap_or((95, 158));
    // per chart: mean decoded HDR over the chart's own pixels (fb0 byte × texel byte → (t·fb/255²)²·MaxHDR)
    let mut by_dist: HashMap<i32, (f64, f64, f64, usize, usize)> = HashMap::new(); // dist → Σr Σg Σb texels charts
    let mut by_cy: HashMap<i32, usize> = HashMap::new();
    let ntiles = cell.len() as u32;
    for i in 0..m.count as usize {
        let obj = m.binds[i].obj_group_idx / 4;
        if ntiles > 0 && obj >= ntiles { continue; }
        let Some(&(cx, cy, cz)) = cell.get(&obj) else { continue };
        *by_cy.entry(cy).or_insert(0) += 1;
        let fb = m.frame_bytes[0][i];
        let (px, py, pw, ph) = lightmap::classcmp::chart_own_px(m.pos[i], m.size[i]);
        let mut acc = [0f64; 3];
        let mut n = 0usize;
        for yy in py..(py + ph).min(img.h) {
            for xx in px..(px + pw).min(img.w) {
                let idx = ((yy * img.w + xx) * 3) as usize;
                for c in 0..3 { acc[c] += lightmap::classcmp::texel_hdr(0, img.px[idx + c], fb, maxhdr); }
                n += 1;
            }
        }
        // distance (in cells) outside the inside square, 0 = inside
        let dx = if cx < inside.0 { inside.0 - cx } else if cx > inside.1 { cx - inside.1 } else { 0 };
        let dz = if cz < inside.0 { inside.0 - cz } else if cz > inside.1 { cz - inside.1 } else { 0 };
        let dist = dx.max(dz);
        let bucket = if dist == 0 { 0 } else if dist <= 2 { 1 } else if dist <= 8 { 2 } else if dist <= 32 { 3 } else { 4 };
        let e = by_dist.entry(bucket).or_insert((0.0, 0.0, 0.0, 0, 0));
        e.0 += acc[0]; e.1 += acc[1]; e.2 += acc[2]; e.3 += n; e.4 += 1;
    }
    println!("tile charts by block cy: {:?}", by_cy);
    let names = ["inside the footprint", "1–2 cells outside", "3–8 cells outside", "9–32 cells outside", "> 32 cells outside"];
    for b in 0..5 {
        if let Some(&(r, g, bl, n, ch)) = by_dist.get(&b) {
            let nn = n.max(1) as f64;
            println!("{:22}: {:6} charts {:9} texels  mean HDR (r g b) = ({:.5}, {:.5}, {:.5})  Σrgb {:.5}", names[b as usize], ch, n, r / nn, g / nn, bl / nn, (r + g + bl) / nn);
        }
    }
}

/// `--grid N`: a coarse map (N×N cells per bin) of the tile charts' mean Σrgb over the whole map, printed as digits of
/// log10 scale: '.' < 0.003, then 0–9 for 0.003·2^k (k = 0..9); called from main when given.
pub fn grid(m: &lightmap::format::Mapping, img: &lightmap::img::Rgb, maxhdr: f32, cell: &HashMap<u32, (i32, i32, i32)>, n: i32) {
    let ntiles = cell.len() as u32;
    let bins = (254 + n - 1) / n;
    let mut acc = vec![(0f64, 0usize); (bins * bins) as usize];
    for i in 0..m.count as usize {
        let obj = m.binds[i].obj_group_idx / 4;
        if obj >= ntiles { continue; }
        let Some(&(cx, _cy, cz)) = cell.get(&obj) else { continue };
        let fb = m.frame_bytes[0][i];
        let (px, py, pw, ph) = lightmap::classcmp::chart_own_px(m.pos[i], m.size[i]);
        let mut s = 0f64;
        let mut k = 0usize;
        for yy in py..(py + ph).min(img.h) { for xx in px..(px + pw).min(img.w) {
            let idx = ((yy * img.w + xx) * 3) as usize;
            for c in 0..3 { s += lightmap::classcmp::texel_hdr(0, img.px[idx + c], fb, maxhdr); }
            k += 1;
        } }
        if k == 0 { continue; }
        let b = ((cz / n) * bins + (cx / n)) as usize;
        acc[b].0 += s / k as f64;
        acc[b].1 += 1;
    }
    println!("tile mean Σrgb, {n}×{n} cells per character (x →, z ↓); '.' < 0.003, digit k = 0.003·2^k");
    for bz in 0..bins {
        let mut line = format!("z {:3}: ", bz * n);
        for bx in 0..bins {
            let (s, c) = acc[(bz * bins + bx) as usize];
            let ch = if c == 0 { ' ' } else { let v = s / c as f64; if v < 0.003 { '.' } else { let k = ((v / 0.003).log2().floor() as i32).clamp(0, 9); (b'0' + k as u8) as char } };
            line.push(ch);
        }
        println!("{line}");
    }
}
