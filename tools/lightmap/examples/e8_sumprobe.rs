//! `e8_sumprobe LAYOUT.tsv` — candidate f32 summation orders of TotalLmSurfaceMeter over the layout's entries (the `area` bits
//! column) and records (ext_x·ext_y), against the game's value window derived from its probe s (E8, 2026-10-01: the RI x2 bake
//! copy's game s = 0x3f8c3476 ⇔ Σ_f32 ∈ [3 067 611.5, 3 067 612.0]; ours sums 3 067 615 along the ascending-area order).
fn main() {
    let a: Vec<String> = std::env::args().collect();
    let txt = std::fs::read_to_string(&a[1]).expect("layout tsv");
    let mut lines = txt.lines();
    let hdr: Vec<&str> = lines.next().unwrap().split('\t').collect();
    let col = |n: &str| hdr.iter().position(|h| *h == n).unwrap_or_else(|| panic!("no column {n}"));
    let (centry, cwalk, carea, cex, cey, cnb, cna, cname) = (col("entry"), col("walk"), col("area"), col("ext_x"), col("ext_y"), col("nb"), col("na"), col("name"));
    let mut entry_area: std::collections::BTreeMap<usize, (f32, usize, u32, u32, String)> = Default::default(); // entry → (area, walk, nb, na, name)
    let mut rec_areas: Vec<f32> = Vec::new();
    let mut rec_entry: Vec<usize> = Vec::new();
    for l in lines {
        let f: Vec<&str> = l.split('\t').collect();
        let e: usize = f[centry].parse().unwrap();
        let area = f32::from_bits(u32::from_str_radix(f[carea].trim_start_matches("0x"), 16).unwrap());
        let walk: usize = f[cwalk].parse().unwrap();
        entry_area.entry(e).or_insert((area, walk, f[cnb].parse().unwrap(), f[cna].parse().unwrap(), f[cname].to_string()));
        let (ex, ey): (f32, f32) = (f[cex].parse().unwrap(), f[cey].parse().unwrap());
        rec_areas.push(ex * ey);
        rec_entry.push(e);
    }
    let n_e = entry_area.len();
    let areas_by_entry: Vec<f32> = entry_area.values().map(|v| v.0).collect();
    let mut by_walk: Vec<(usize, f32)> = entry_area.values().map(|v| (v.1, v.0)).collect();
    by_walk.sort_by_key(|v| v.0);
    // walk 0 = placed first = the END of the ascending order → ascending = reverse walk
    let asc: Vec<f32> = by_walk.iter().rev().map(|v| v.1).collect();
    let desc: Vec<f32> = by_walk.iter().map(|v| v.1).collect();
    let sum = |v: &[f32]| v.iter().fold(0f32, |s, x| s + x);
    let sum64 = |v: &[f32]| v.iter().fold(0f64, |s, x| s + *x as f64);
    println!("{n_e} entries, {} records", rec_areas.len());
    println!("Σ entries ascending-area (ours)      {:.3} ({:#010x})", sum(&asc), sum(&asc).to_bits());
    println!("Σ entries descending-area            {:.3}", sum(&desc));
    println!("Σ entries in entry-index order       {:.3}", sum(&areas_by_entry));
    println!("Σ entries f64                        {:.3}", sum64(&asc));
    println!("Σ records in record order            {:.3}", sum(&rec_areas));
    println!("Σ records f64                        {:.3}", sum64(&rec_areas));
    // records sorted ascending by their own area
    let mut rs = rec_areas.clone(); rs.sort_by(|x, y| x.partial_cmp(y).unwrap());
    println!("Σ records ascending                  {:.3}", sum(&rs));
    rs.reverse();
    println!("Σ records descending                 {:.3}", sum(&rs));
    // the entry area as the SUM of its members' areas (not count × first member's), in ascending entry order
    let mut member_sum: std::collections::BTreeMap<usize, f32> = Default::default();
    for (k, e) in rec_entry.iter().enumerate() { *member_sum.entry(*e).or_default() += rec_areas[k]; }
    let mut ms: Vec<(usize, f32)> = entry_area.iter().map(|(e, v)| (v.1, member_sum[e])).collect();
    ms.sort_by_key(|v| v.0);
    let ms_asc: Vec<f32> = ms.iter().rev().map(|v| v.1).collect();
    println!("Σ entries as Σmembers, ascending     {:.3}", sum(&ms_asc));
    // the entry area with the grid's EMPTY slots excluded: count × first-member area where count = members (ours already) — and with
    // nb·na slots (the grid's full area)
    let mut slots_asc: Vec<f32> = by_walk.iter().rev().map(|(w, _)| { let v = entry_area.values().find(|v| v.1 == *w).unwrap(); let per = v.0 / (entry_area.values().find(|x| x.1 == *w).map(|_| 1.0).unwrap()); per }).collect();
    let _ = &mut slots_asc;
    // pairwise (tree) summation of the ascending list
    fn pairwise(v: &[f32]) -> f32 { if v.len() <= 8 { v.iter().fold(0f32, |s, x| s + x) } else { let m = v.len() / 2; pairwise(&v[..m]) + pairwise(&v[m..]) } }
    println!("Σ entries pairwise                   {:.3}", pairwise(&asc));
    // Kahan
    let (mut s, mut c) = (0f32, 0f32);
    for x in &asc { let y = x - c; let t = s + y; c = (t - s) - y; s = t; }
    println!("Σ entries Kahan                      {:.3}", s);
    // the window
    println!("game window from its probe s: [3067611.5, 3067612.0]");
    // --kind0 ex,ey: replace our kind-0 (class item0) entries by the GAME's five entries (3×4, 3×3, 3×2, 2×6, 2×1 grids of the
    // record extent ex×ey) and re-sum ascending — do the legacy tree records' entries explain the game's Σ?
    if let Some(i) = a.iter().position(|x| x == "--kind0") {
        let v: Vec<f32> = a[i + 1].split(',').map(|t| t.parse().unwrap()).collect();
        let (ex, ey) = (v[0], v[1]);
        let per = ey * ex;
        let game_k0: Vec<f32> = [12i32, 9, 6, 12, 2].iter().map(|n| (*n as f32) * per).collect();
        // our kind-0 entries = the entries whose area equals n·per for n in {12, 9} and whose name is the item0 class: take the TSV's class
        // column is not loaded here — identify them by area value instead (the only entries below 5 m² with these exact products)
        let ours_k0: Vec<f32> = [12i32, 12, 12, 9].iter().map(|n| (*n as f32) * per).collect();
        let mut rest: Vec<f32> = asc.clone();
        for v in &ours_k0 { if let Some(p) = rest.iter().position(|x| x.to_bits() == v.to_bits()) { rest.remove(p); } else { println!("  (our kind-0 entry area {v} not found among the entries)"); } }
        let mut with_game: Vec<f32> = rest.clone(); with_game.extend(game_k0.iter().copied());
        with_game.sort_by(|x, y| x.to_bits().cmp(&y.to_bits()));
        println!("Σ ascending with the game's kind-0 entries  {:.3} (ours' kind-0 removed: {} entries, +5 game entries {:?})", sum(&with_game), rest.len(), game_k0);
        let mut desc_g = with_game.clone(); desc_g.reverse();
        println!("Σ descending with the game's kind-0 entries {:.3}", sum(&desc_g));
        println!("Σ ascending without any kind-0 entry         {:.3}", sum(&rest));
    }
    // per-entry area alternatives: n·(ey·ex) (ours), (n·ey)·ex, (n·ex)·ey — recomputed from the TSV's first member ext (approximate: the
    // TSV ext is printed with 7 digits) — skipped; the summation order is the lever tested here
}
