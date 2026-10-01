//! `e8_mapcmp OURS.Map.Gbx GAME.Map.Gbx [--dump N]` — two maps' lightmap MAPPINGS paired by BIND WORD (obj_group_idx/4 = the
//! object id, obj_idx = the sub-object/flags word), not by chart index (E7's e7_mapcmp): how many binds both carry, how many
//! of those rects are identical / same size / a different size, the linear size-ratio histogram per object class (tiles =
//! obj below the first item obj; items above), the unmatched binds on each side, and the first N pairs (E8, 2026-10-01).
fn main() {
    let a: Vec<String> = std::env::args().collect();
    if a.len() < 3 { eprintln!("usage: e8_mapcmp OURS.Map.Gbx GAME.Map.Gbx [--dump N] [--item-base OBJ]"); std::process::exit(2); }
    let flag = |n: &str| a.iter().position(|x| x == n).and_then(|i| a.get(i + 1).cloned());
    let dump: usize = flag("--dump").map(|v| v.parse().unwrap()).unwrap_or(12);
    let la = lightmap::mapio::load(&a[1]).unwrap_or_else(|e| panic!("{}: {e}", a[1]));
    let lb = lightmap::mapio::load(&a[2]).unwrap_or_else(|e| panic!("{}: {e}", a[2]));
    let (da, db) = (la.chunk.data.as_ref().expect("no lightmap A"), lb.chunk.data.as_ref().expect("no lightmap B"));
    let (ma, mb) = (da.cache.mapping().expect("no mapping A"), db.cache.mapping().expect("no mapping B"));
    println!("A (ours): {} charts, atlas {}×{}, bbox {:?}..{:?}", ma.count, ma.atlas_w, ma.atlas_h, ma.bbox_min, ma.bbox_max);
    println!("B (game): {} charts, atlas {}×{}, bbox {:?}..{:?}", mb.count, mb.atlas_w, mb.atlas_h, mb.bbox_min, mb.bbox_max);
    // the bind key → chart index (duplicates counted)
    let key = |m: &lightmap::format::Mapping, c: usize| -> (u32, u32) { (m.binds[c].obj_group_idx, m.binds[c].obj_idx) };
    let mut ib: std::collections::HashMap<(u32, u32), Vec<usize>> = Default::default();
    for c in 0..mb.count as usize { ib.entry(key(mb, c)).or_default().push(c); }
    let mut ia: std::collections::HashMap<(u32, u32), Vec<usize>> = Default::default();
    for c in 0..ma.count as usize { ia.entry(key(ma, c)).or_default().push(c); }
    let dup_a = ia.values().filter(|v| v.len() > 1).count();
    let dup_b = ib.values().filter(|v| v.len() > 1).count();
    println!("distinct binds: A {} ({dup_a} duplicated), B {} ({dup_b} duplicated)", ia.len(), ib.len());
    // the item base: the smallest obj whose bind carries a sub-object word ≠ 0 is not reliable — take --item-base, else the game's
    // tile charts = the objs below the first obj with a non-4×4… no: report per obj decile instead, and a user-given base
    let item_base: Option<u32> = flag("--item-base").map(|v| v.parse().unwrap());
    let (mut both, mut same, mut same_size, mut swapped, mut diff) = (0usize, 0usize, 0usize, 0usize, 0usize);
    let mut same_order = 0usize;
    let mut ratios: std::collections::BTreeMap<i32, usize> = Default::default();
    let mut ratios_tiles: std::collections::BTreeMap<i32, usize> = Default::default();
    let mut ratios_items: std::collections::BTreeMap<i32, usize> = Default::default();
    let mut only_a = 0usize;
    let mut shown = 0usize;
    let mut area_a = 0u64; let mut area_b = 0u64;
    let mut area_a_tiles = 0u64; let mut area_b_tiles = 0u64;
    let mut n_tiles = 0usize;
    for c in 0..ma.count as usize {
        let k = key(ma, c);
        let Some(cs) = ib.get(&k) else { only_a += 1; continue };
        let cb = cs[0];
        both += 1;
        if cb == c { same_order += 1; }
        let (pa, sa, pb, sb) = (ma.pos[c], ma.size[c], mb.pos[cb], mb.size[cb]);
        let obj = k.0 / 4;
        let is_tile = item_base.map(|b| obj < b).unwrap_or(false);
        area_a += sa.0 as u64 * sa.1 as u64; area_b += sb.0 as u64 * sb.1 as u64;
        if is_tile { n_tiles += 1; area_a_tiles += sa.0 as u64 * sa.1 as u64; area_b_tiles += sb.0 as u64 * sb.1 as u64; }
        let r = ((sa.0 as f32 * sa.1 as f32) / (sb.0 as f32 * sb.1 as f32).max(1.0)).sqrt();
        let rb = (r * 10.0).round() as i32;
        *ratios.entry(rb).or_default() += 1;
        if is_tile { *ratios_tiles.entry(rb).or_default() += 1; } else { *ratios_items.entry(rb).or_default() += 1; }
        if pa == pb && sa == sb { same += 1; continue; }
        if sa == sb { same_size += 1; } else if (sa.0, sa.1) == (sb.1, sb.0) { swapped += 1; } else { diff += 1; }
        if shown < dump && sa != sb { println!("  bind obj {obj} sub {:#x}: A chart {c} ({}, {}) {}×{}  B chart {cb} ({}, {}) {}×{}", k.1, pa.0, pa.1, sa.0, sa.1, pb.0, pb.1, sb.0, sb.1); shown += 1; }
    }
    let only_b = (0..mb.count as usize).filter(|&c| !ia.contains_key(&key(mb, c))).count();
    println!("binds in both: {both} (same chart index {same_order}); only in A {only_a}; only in B {only_b}");
    // by INDEX: the charts whose bind word differs from B's at the same index (an object-numbering offset), the first few
    {
        let n = (ma.count as usize).min(mb.count as usize);
        let mut nb = 0usize; let mut nr = 0usize; let mut first: Vec<String> = Vec::new();
        for c in 0..n {
            if key(ma, c) != key(mb, c) { nb += 1; if first.len() < 5 { first.push(format!("chart {c}: A obj {} sub {:#x} / B obj {} sub {:#x} (rects A ({}, {}) {}×{} B ({}, {}) {}×{})", ma.binds[c].obj_group_idx / 4, ma.binds[c].obj_idx, mb.binds[c].obj_group_idx / 4, mb.binds[c].obj_idx, ma.pos[c].0, ma.pos[c].1, ma.size[c].0, ma.size[c].1, mb.pos[c].0, mb.pos[c].1, mb.size[c].0, mb.size[c].1)); } }
            if ma.pos[c] != mb.pos[c] || ma.size[c] != mb.size[c] { nr += 1; }
        }
        println!("by index: {nb} of {n} charts differ in bind word, {nr} in rect; first differing binds: {}", first.join(" | "));
    }
    println!("of the pairs: identical rect {same}, same size other position {same_size}, size swapped {swapped}, different size {diff}");
    println!("Σ layout area A {area_a} vs B {area_b} (ratio {:.4}); tiles ({n_tiles}): A {area_a_tiles} vs B {area_b_tiles}", area_a as f64 / area_b.max(1) as f64);
    println!("linear size ratio A/B (×10) all: {ratios:?}");
    if item_base.is_some() { println!("  tiles: {ratios_tiles:?}"); println!("  items: {ratios_items:?}"); }
    // the first N charts of each side (order + bind + rect)
    println!("first {dump} charts of A: {}", (0..dump.min(ma.count as usize)).map(|c| format!("[{c}: obj {} sub {:#x} ({}, {}) {}×{}]", ma.binds[c].obj_group_idx / 4, ma.binds[c].obj_idx, ma.pos[c].0, ma.pos[c].1, ma.size[c].0, ma.size[c].1)).collect::<Vec<_>>().join(" "));
    println!("first {dump} charts of B: {}", (0..dump.min(mb.count as usize)).map(|c| format!("[{c}: obj {} sub {:#x} ({}, {}) {}×{}]", mb.binds[c].obj_group_idx / 4, mb.binds[c].obj_idx, mb.pos[c].0, mb.pos[c].1, mb.size[c].0, mb.size[c].1)).collect::<Vec<_>>().join(" "));
    // the obj histogram per side: min/max obj, the sub-word values
    let objs = |m: &lightmap::format::Mapping| -> (u32, u32, std::collections::BTreeMap<u32, usize>) { let mut lo = u32::MAX; let mut hi = 0; let mut subs: std::collections::BTreeMap<u32, usize> = Default::default(); for c in 0..m.count as usize { let o = m.binds[c].obj_group_idx / 4; lo = lo.min(o); hi = hi.max(o); *subs.entry(m.binds[c].obj_idx).or_default() += 1; } (lo, hi, subs) };
    let (alo, ahi, asubs) = objs(ma); let (blo, bhi, bsubs) = objs(mb);
    println!("A objs {alo}..{ahi}, sub words {} distinct (top: {:?})", asubs.len(), { let mut v: Vec<_> = asubs.iter().collect(); v.sort_by(|x, y| y.1.cmp(x.1)); v.into_iter().take(6).map(|(k, n)| (format!("{k:#x}"), *n)).collect::<Vec<_>>() });
    println!("B objs {blo}..{bhi}, sub words {} distinct (top: {:?})", bsubs.len(), { let mut v: Vec<_> = bsubs.iter().collect(); v.sort_by(|x, y| y.1.cmp(x.1)); v.into_iter().take(6).map(|(k, n)| (format!("{k:#x}"), *n)).collect::<Vec<_>>() });
    // the size histogram per side (w×h) — the budget shape
    let sizes = |m: &lightmap::format::Mapping| -> Vec<((u16, u16), usize)> { let mut h: std::collections::HashMap<(u16, u16), usize> = Default::default(); for c in 0..m.count as usize { *h.entry(m.size[c]).or_default() += 1; } let mut v: Vec<_> = h.into_iter().collect(); v.sort_by(|x, y| y.1.cmp(&x.1)); v };
    println!("A size histogram (top 16): {:?}", sizes(ma).into_iter().take(16).collect::<Vec<_>>());
    println!("B size histogram (top 16): {:?}", sizes(mb).into_iter().take(16).collect::<Vec<_>>());
}
