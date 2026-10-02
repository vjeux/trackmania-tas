//! `e8_layoutcmp LAYOUT.tsv GAME.Map.Gbx [--s-ours S]` — our layout table (`lmtool bake … --layout-game --layout-tsv`) against the
//! game's mapping, joined by OBJECT id: per class, the game's chart sizes vs ours; the game's implied density s from the item charts
//! above the minimum ((w + 2·pad)/ext per axis, floor fit → a lower bound within 1/ext); the tiles' implied quality from the game's
//! sizes given that s; our tile quality histogram (ext / the q=1 ext) vs the game's size histogram (E8, 2026-10-01).
fn main() {
    let a: Vec<String> = std::env::args().collect();
    if a.len() < 3 { eprintln!("usage: e8_layoutcmp LAYOUT.tsv GAME.Map.Gbx"); std::process::exit(2); }
    let txt = std::fs::read_to_string(&a[1]).expect("layout tsv");
    let mut lines = txt.lines();
    let hdr: Vec<&str> = lines.next().unwrap().split('\t').collect();
    let col = |n: &str| hdr.iter().position(|h| *h == n).unwrap_or_else(|| panic!("no column {n}"));
    let (cc, cobj, cx, cy, cw, ch, cex, cey, cname) = (col("class"), col("obj"), col("x"), col("y"), col("w"), col("h"), col("ext_x"), col("ext_y"), col("name"));
    struct Row { class: String, obj: u32, x: i32, y: i32, w: i32, h: i32, ext: [f32; 2], name: String }
    let mut rows: Vec<Row> = Vec::new();
    for l in lines {
        let f: Vec<&str> = l.split('\t').collect();
        rows.push(Row { class: f[cc].to_string(), obj: f[cobj].parse().unwrap(), x: f[cx].parse().unwrap(), y: f[cy].parse().unwrap(), w: f[cw].parse().unwrap(), h: f[ch].parse().unwrap(), ext: [f[cex].parse().unwrap(), f[cey].parse().unwrap()], name: f[cname].to_string() });
    }
    let lb = lightmap::mapio::load(&a[2]).unwrap_or_else(|e| panic!("{}: {e}", a[2]));
    let db = lb.chunk.data.as_ref().expect("no lightmap");
    let mb = db.cache.mapping().expect("no mapping");
    let mut by_obj: std::collections::HashMap<u32, usize> = Default::default();
    for c in 0..mb.count as usize { by_obj.insert(mb.binds[c].obj_group_idx / 4, c); }
    // the feasible s interval of a mapping under the transcribed fit (floor, a carry bump of at most one unit, granularity 2): the
    // packer side P = w + 2 ∈ {2⌊⌊e·s⌋/2⌋, 2⌊(⌊e·s⌋+1)/2⌋ (+2)} → ⌊e·s⌋ ∈ [P−1, P+1] → s ∈ [(P−1)/e, (P+2)/e); intersected over the
    // item axes above the minimum, for the GAME's sizes and for OURS (ours must hold our s — the check of the bound itself)
    {
        let mut bounds = |label: &str, size_of: &dyn Fn(usize, &Row) -> Option<(i32, i32)>| {
            let (mut lo, mut hi) = (0.0f32, f32::MAX);
            let (mut lo_who, mut hi_who) = (String::new(), String::new());
            let mut n = 0;
            for r in rows.iter().filter(|r| r.class != "tile") {
                let Some(&c) = by_obj.get(&r.obj) else { continue };
                let Some((w, h)) = size_of(c, r) else { continue };
                for (sz, e) in [(w, r.ext[0]), (h, r.ext[1])] {
                    if sz <= 4 || e <= 0.0 { continue; }
                    n += 1;
                    let p = (sz + 2) as f32;
                    let (l, u) = ((p - 1.0) / e, (p + 2.0) / e);
                    if l > lo { lo = l; lo_who = format!("{} obj {} e {e:.3} P {p}", r.name, r.obj); }
                    if u < hi { hi = u; hi_who = format!("{} obj {} e {e:.3} P {p}", r.name, r.obj); }
                }
            }
            println!("{label}: {n} axes → s ∈ [{lo:.5}, {hi:.5}) (lower by {lo_who}; upper by {hi_who})");
        };
        bounds("GAME feasible s", &|c, _r| Some((mb.size[c].0 as i32, mb.size[c].1 as i32)));
        bounds("OURS feasible s", &|_c, r| Some((r.w, r.h)));
    }
    println!("ours: {} rows; game: {} charts; joined by obj: {}", rows.len(), mb.count, rows.iter().filter(|r| by_obj.contains_key(&r.obj)).count());
    // the rects themselves (layout units = the mapping's pos/size): identical / same size / different, per class
    {
        let mut per: std::collections::BTreeMap<String, (usize, usize, usize, usize)> = Default::default();
        let mut first_diff: Vec<String> = Vec::new();
        for r in &rows {
            let Some(&c) = by_obj.get(&r.obj) else { continue };
            let e = per.entry(r.class.clone()).or_default();
            e.0 += 1;
            let (p, s) = (mb.pos[c], mb.size[c]);
            if (r.x, r.y, r.w, r.h) == (p.0 as i32, p.1 as i32, s.0 as i32, s.1 as i32) { e.1 += 1; }
            else if (r.w, r.h) == (s.0 as i32, s.1 as i32) { e.2 += 1; if first_diff.len() < 6 { first_diff.push(format!("{} obj {} ours ({}, {}) {}×{} game ({}, {}) {}×{}", r.name, r.obj, r.x, r.y, r.w, r.h, p.0, p.1, s.0, s.1)); } }
            else { e.3 += 1; if first_diff.len() < 6 { first_diff.push(format!("{} obj {} ours ({}, {}) {}×{} game ({}, {}) {}×{}", r.name, r.obj, r.x, r.y, r.w, r.h, p.0, p.1, s.0, s.1)); } }
        }
        for (cl, (n, same, pos, diff)) in &per { println!("rects {cl}: {n} charts — identical {same}, same size other position {pos}, different size {diff}"); }
        if !first_diff.is_empty() { println!("first differing: {}", first_diff.join(" | ")); }
        // the size differences per model: count, the extents, ours vs the game's sizes (the first three), the implied s range
        let mut bym: std::collections::BTreeMap<String, (usize, usize, Vec<String>)> = Default::default();
        for r in &rows {
            let Some(&c) = by_obj.get(&r.obj) else { continue };
            let s = mb.size[c];
            let e = bym.entry(format!("{} ext {:.2}×{:.2}", r.name, r.ext[0], r.ext[1])).or_default();
            e.0 += 1;
            if (r.w, r.h) != (s.0 as i32, s.1 as i32) { e.1 += 1; if e.2.len() < 3 { e.2.push(format!("{}×{}→{}×{}", r.w, r.h, s.0, s.1)); } }
        }
        let mut v: Vec<_> = bym.iter().filter(|(_, e)| e.1 > 0).collect();
        v.sort_by(|x, y| y.1 .1.cmp(&x.1 .1));
        println!("size differences per model (count differing / count, examples ours→game):");
        for (m, (n, d, ex)) in v.iter().take(30) { println!("   {m}: {d}/{n} {}", ex.join(" ")); }
    }
    // per class: counts, our size histogram, the game's size histogram, Σareas
    let mut classes: Vec<String> = rows.iter().map(|r| r.class.clone()).collect::<std::collections::BTreeSet<_>>().into_iter().collect();
    classes.sort();
    for cl in &classes {
        let rs: Vec<&Row> = rows.iter().filter(|r| &r.class == cl).collect();
        let mut ho: std::collections::HashMap<(i32, i32), usize> = Default::default();
        let mut hg: std::collections::HashMap<(u16, u16), usize> = Default::default();
        let (mut ao, mut ag) = (0u64, 0u64);
        let mut n = 0;
        for r in &rs {
            let Some(&c) = by_obj.get(&r.obj) else { continue };
            n += 1;
            *ho.entry((r.w, r.h)).or_default() += 1;
            *hg.entry(mb.size[c]).or_default() += 1;
            ao += (r.w * r.h) as u64; ag += mb.size[c].0 as u64 * mb.size[c].1 as u64;
        }
        let top = |h: &std::collections::HashMap<(i32, i32), usize>| { let mut v: Vec<_> = h.iter().map(|(k, n)| (*k, *n)).collect(); v.sort_by(|x, y| y.1.cmp(&x.1)); v.into_iter().take(12).collect::<Vec<_>>() };
        let topg = |h: &std::collections::HashMap<(u16, u16), usize>| { let mut v: Vec<_> = h.iter().map(|(k, n)| (*k, *n)).collect(); v.sort_by(|x, y| y.1.cmp(&x.1)); v.into_iter().take(12).collect::<Vec<_>>() };
        println!("class {cl}: {n} charts; Σarea ours {ao} vs game {ag} (ratio {:.3})", ao as f64 / ag.max(1) as f64);
        println!("   ours sizes: {:?}", top(&ho));
        println!("   game sizes: {:?}", topg(&hg));
    }
    // the game's implied s from the item charts (both axes, size > the minimum 4): s_lo = (w + 2)/ext (floor fit: w+2 ≤ ext·s < w+3)
    let mut s_lo: Vec<f32> = Vec::new(); let mut s_hi: Vec<f32> = Vec::new();
    let mut per_item: Vec<(f32, f32, String)> = Vec::new();
    for r in rows.iter().filter(|r| r.class != "tile") {
        let Some(&c) = by_obj.get(&r.obj) else { continue };
        let (gw, gh) = (mb.size[c].0 as f32, mb.size[c].1 as f32);
        for (g, e) in [(gw, r.ext[0]), (gh, r.ext[1])] {
            if g > 4.0 && e > 0.0 { s_lo.push((g + 2.0) / e); s_hi.push((g + 3.0) / e); }
        }
        if gw > 4.0 && r.ext[0] > 0.0 { per_item.push(((gw + 2.0) / r.ext[0], (gw + 3.0) / r.ext[0], format!("{} obj {} ext {:.2}×{:.2} ours {}×{} game {}×{}", r.name, r.obj, r.ext[0], r.ext[1], r.w, r.h, gw, gh))); }
    }
    s_lo.sort_by(|x, y| x.partial_cmp(y).unwrap()); s_hi.sort_by(|x, y| x.partial_cmp(y).unwrap());
    let pct = |v: &Vec<f32>, p: f64| v[((v.len() as f64 - 1.0) * p) as usize];
    if !s_lo.is_empty() {
        println!("game s from {} item axes above the minimum: (w+2)/ext percentiles 5/25/50/75/95 = {:.4} {:.4} {:.4} {:.4} {:.4}; (w+3)/ext 5/50 = {:.4} {:.4}; the max lower bound {:.4}, the min upper bound {:.4}", s_lo.len(), pct(&s_lo, 0.05), pct(&s_lo, 0.25), pct(&s_lo, 0.5), pct(&s_lo, 0.75), pct(&s_lo, 0.95), pct(&s_hi, 0.05), pct(&s_hi, 0.5), s_lo.last().unwrap(), s_hi.first().unwrap());
    }
    // a histogram of (w+2)/ext at 0.02 resolution
    let mut hs: std::collections::BTreeMap<i32, usize> = Default::default();
    for v in &s_lo { *hs.entry((v * 50.0).round() as i32).or_default() += 1; }
    println!("histogram of (w+2)/ext (×50): {:?}", hs.iter().filter(|(_, n)| **n >= 5).map(|(k, n)| (*k as f32 / 50.0, *n)).collect::<Vec<_>>());
    // the outliers: items whose implied s is far from the median
    let med = pct(&s_lo, 0.5);
    per_item.sort_by(|x, y| x.0.partial_cmp(&y.0).unwrap());
    println!("items with the LOWEST implied s (game chart small for its extent): {}", per_item.iter().take(8).map(|(lo, _, d)| format!("{d} → s {lo:.3}")).collect::<Vec<_>>().join(" | "));
    println!("items with the HIGHEST implied s: {}", per_item.iter().rev().take(8).map(|(lo, _, d)| format!("{d} → s {lo:.3}")).collect::<Vec<_>>().join(" | "));
    println!("median implied s {med:.4}");
    // the tiles: our quality from ext (q = ext/ext1, ext1 = the largest tile ext); the game's size per our q
    let tiles: Vec<&Row> = rows.iter().filter(|r| r.class == "tile").collect();
    if !tiles.is_empty() {
        let ext1 = tiles.iter().map(|r| r.ext[0]).fold(0.0f32, f32::max);
        let mut byq: std::collections::BTreeMap<i32, (usize, std::collections::HashMap<(u16, u16), usize>)> = Default::default();
        for r in &tiles {
            let Some(&c) = by_obj.get(&r.obj) else { continue };
            let q = r.ext[0] / ext1;
            let ring = (-(q.ln()) / (0.5f32).ln().abs() * 2.0).round() as i32; // q = 2^(-ring/2)
            let e = byq.entry(ring).or_default();
            e.0 += 1; *e.1.entry(mb.size[c]).or_default() += 1;
        }
        println!("tiles: q=1 ext {ext1:.4} m; per OUR ring r (q = (√2)^−r): count, the GAME's sizes:");
        for (ring, (n, h)) in &byq { let mut v: Vec<_> = h.iter().collect(); v.sort_by(|x, y| y.1.cmp(x.1)); println!("   ring {ring}: {n} tiles → game {:?}", v.into_iter().take(6).map(|(k, n)| (*k, *n)).collect::<Vec<_>>()); }
        // which GAME tile sizes exist at all, with the tiles' positions (cell) — the first few bigger-than-minimum tiles
        let mut big: Vec<(u32, (u16, u16), i32)> = Vec::new();
        for r in &tiles { let Some(&c) = by_obj.get(&r.obj) else { continue }; if mb.size[c].0 > 4 || mb.size[c].1 > 4 { big.push((r.obj, mb.size[c], ((r.ext[0] / ext1).ln() / (0.5f32).ln() * 2.0).round() as i32)); } }
        println!("game tiles above the minimum: {} — first: {:?}", big.len(), big.iter().take(20).collect::<Vec<_>>());
    }
}
