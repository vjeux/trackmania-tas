//! `e7_mapcmp A.Map.Gbx B.Map.Gbx` — the two maps' lightmap MAPPINGS chart by chart: how many rects are identical, how the sizes relate
//! (a scale, a rotation, neither), the first differences (E7, 2026-10-01: RI's port layout vs the game's).
fn main() {
    let a: Vec<String> = std::env::args().collect();
    if a.len() < 3 { eprintln!("usage: e7_mapcmp A.Map.Gbx B.Map.Gbx"); std::process::exit(2); }
    let la = lightmap::mapio::load(&a[1]).unwrap_or_else(|e| panic!("{}: {e}", a[1])); let lb = lightmap::mapio::load(&a[2]).unwrap_or_else(|e| panic!("{}: {e}", a[2]));
    let (da, db) = (la.chunk.data.as_ref().expect("no lightmap A"), lb.chunk.data.as_ref().expect("no lightmap B"));
    let (ma, mb) = (da.cache.mapping().expect("no mapping A"), db.cache.mapping().expect("no mapping B"));
    println!("A: {} charts, atlas {}×{}; B: {} charts, atlas {}×{}", ma.count, ma.atlas_w, ma.atlas_h, mb.count, mb.atlas_w, mb.atlas_h);
    let n = (ma.count as usize).min(mb.count as usize);
    let (mut same, mut same_size, mut swapped, mut diff) = (0usize, 0usize, 0usize, 0usize);
    let mut ratios: std::collections::BTreeMap<i32, usize> = Default::default();
    let mut shown = 0;
    for c in 0..n {
        let (pa, sa, pb, sb) = (ma.pos[c], ma.size[c], mb.pos[c], mb.size[c]);
        if pa == pb && sa == sb { same += 1; continue; }
        if sa == sb { same_size += 1; } else if (sa.0, sa.1) == (sb.1, sb.0) { swapped += 1; } else { diff += 1; }
        let r = ((sa.0 as f32 * sa.1 as f32) / (sb.0 as f32 * sb.1 as f32).max(1.0)).sqrt();
        *ratios.entry((r * 10.0).round() as i32).or_default() += 1;
        if shown < 8 && sa != sb { println!("  chart {c}: A ({}, {}) {}×{}  B ({}, {}) {}×{}", pa.0, pa.1, sa.0, sa.1, pb.0, pb.1, sb.0, sb.1); shown += 1; }
    }
    println!("{n} charts compared: identical rect {same}, same size other position {same_size}, size swapped (rotated) {swapped}, different size {diff}");
    println!("linear size ratio A/B (×10, rounded) histogram: {:?}", ratios);
}
