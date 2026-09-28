//! `e3_guttercensus MAP.Map.Gbx --records RECORDS.tsv [--min 1]` — the NON-ZERO GUTTER TEXELS of a lightmap image (frame 0): texels
//! outside every chart rect whose max byte ≥ min, attributed to the nearest chart rect (Chebyshev distance to the rect) and summed per
//! record NAME (records.tsv = `bake --records-tsv`, chart k ↔ mapping entry k) — which items the GAME's own raster spilled outside their
//! rects (E3 2026-09-28, RE 16's closer (b) for the flag-pole spill: do only card / multi-chart items spill in the editor's tiny03?).
use std::collections::BTreeMap;

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let f = |k: &str| a.iter().position(|x| x == k).and_then(|i| a.get(i + 1)).cloned();
    let min: u8 = f("--min").map(|s| s.parse().expect("min")).unwrap_or(1);
    let map = lightmap::mapio::load(&a[1]).unwrap_or_else(|e| panic!("{e}"));
    let d = map.chunk.data.as_ref().expect("no lightmap");
    let m = d.cache.mapping().expect("no mapping");
    let img = lightmap::img::decode_webp(d.frames[0].images.first().expect("image 0")).unwrap_or_else(|e| panic!("{e}"));
    let names: Vec<(String, String)> = f("--records").map(|p| {
        std::fs::read_to_string(&p).expect("records").lines().skip(1).map(|l| { let c: Vec<&str> = l.split('\t').collect(); (c.get(1).unwrap_or(&"").to_string(), c.get(4).unwrap_or(&"").to_string()) }).collect()
    }).unwrap_or_default();
    let rects: Vec<(u32, u32, u32, u32)> = (0..m.count as usize).map(|i| lightmap::classcmp::chart_own_px(m.pos[i], m.size[i])).collect();
    // a coverage grid: chart index per texel (u32::MAX = gutter)
    let mut owner = vec![u32::MAX; (img.w * img.h) as usize];
    for (i, r) in rects.iter().enumerate() {
        for y in r.1..(r.1 + r.3).min(img.h) { for x in r.0..(r.0 + r.2).min(img.w) { owner[(y * img.w + x) as usize] = i as u32; } }
    }
    let mut per: BTreeMap<String, (usize, u64, u32)> = BTreeMap::new(); // name → (texels, Σ max byte, max dist)
    let (mut gutter, mut lit) = (0usize, 0usize);
    for y in 0..img.h {
        for x in 0..img.w {
            let p = (y * img.w + x) as usize;
            if owner[p] != u32::MAX { continue; }
            gutter += 1;
            let mx = img.px[p * 3].max(img.px[p * 3 + 1]).max(img.px[p * 3 + 2]);
            if mx < min { continue; }
            lit += 1;
            // the nearest rect by Chebyshev distance (search a widening window for speed: rects are small, the atlas 2048²)
            let mut best: Option<(u32, usize)> = None;
            for (i, r) in rects.iter().enumerate() {
                let dx = if x < r.0 { r.0 - x } else if x >= r.0 + r.2 { x + 1 - (r.0 + r.2) } else { 0 };
                let dy = if y < r.1 { r.1 - y } else if y >= r.1 + r.3 { y + 1 - (r.1 + r.3) } else { 0 };
                let dd = dx.max(dy);
                if best.map_or(true, |(b, _)| dd < b) { best = Some((dd, i)); if dd <= 1 { break; } }
            }
            let (dist, ci) = best.unwrap();
            let name = names.get(ci).map(|(cls, n)| if n.is_empty() { cls.clone() } else { n.clone() }).unwrap_or_else(|| format!("chart {ci}"));
            let e = per.entry(name).or_insert((0, 0, 0));
            e.0 += 1;
            e.1 += mx as u64;
            e.2 = e.2.max(dist);
        }
    }
    println!("{}: {} gutter texels, {} with max byte ≥ {min} (attributed to the nearest rect's record):", a[1], gutter, lit);
    let mut rows: Vec<(String, (usize, u64, u32))> = per.into_iter().collect();
    rows.sort_by(|p, q| q.1 .0.cmp(&p.1 .0));
    for (name, (n, s, dmax)) in rows.iter().take(40) { println!("  {:>7} texels  mean max byte {:>5.1}  farthest {:>3} px  {}", n, *s as f64 / *n as f64, dmax, name); }
}
