//! `e7_envcmp --ours-color peel_sky/s0/dNNN/p0/color.bin --ours-depth …/depth.bin --game-color RT.dds[.gz] --game-depth DS.dds[.gz]
//!   [--no-rot180] [--at x,y …] [--top N]` — OUR environment raster (a `--dump-passes` peel_sky dump: colour R11G11B10 packed u32,
//! depth R32 z01 with 0 = the dome) against the game's captured env layer 0 (baker's tex export: the R11G11B10 colour target + the
//! R16 depth, whose DDS header says R16_FLOAT but whose bits are the UNORM16 z01). The port's peel frame is the game's render
//! target rotated by 180° (the projection's negative x scale and the target's y-down: game (x, y) = ours (W−1−x, H−1−y)); --no-rot180
//! compares in place. Census: game class × ours class (dome = depth 0 / surface), the dome pixels' colour in R11G11B10 quanta, the
//! surface pixels' depth in R16 steps and colour, the worst pixels, and the game-pixel probes (E7, 2026-09-30).
fn main() {
    let a: Vec<String> = std::env::args().collect();
    let f = |k: &str| a.iter().position(|x| x == k).and_then(|i| a.get(i + 1)).cloned();
    let load = |p: &str, fmt: &str| -> lightmap::passdiff::Buf {
        let bytes = std::fs::read(p).unwrap_or_else(|e| panic!("{p}: {e}"));
        let bytes = if p.ends_with(".gz") { lightmap::passdiff::gunzip(&bytes).expect("gunzip") } else { bytes };
        if bytes.len() >= 4 && &bytes[..4] == b"DDS " { lightmap::passdiff::load_dds_bytes(&bytes, fmt, 0, 0).unwrap_or_else(|e| panic!("{p}: {e}")) }
        else { let (rw, rh) = f("--ours-size").map(|s| { let v: Vec<u32> = s.split(',').filter_map(|t| t.trim().parse().ok()).collect(); (v[0], v[1]) }).unwrap_or((4096, 4096)); lightmap::passdiff::decode_raw(&bytes, lightmap::passdiff::parse_format(fmt), rw, rh, 0).unwrap_or_else(|e| panic!("{p}: {e}")) }
    };
    let oc = load(&f("--ours-color").expect("--ours-color"), "R11G11B10_FLOAT");
    let od = load(&f("--ours-depth").expect("--ours-depth"), "R32_FLOAT");
    let gc = load(&f("--game-color").expect("--game-color"), "R11G11B10_FLOAT");
    let gd = load(&f("--game-depth").expect("--game-depth"), "R16_UNORM");
    // --map rot180 (default) | none | flipx | flipy: how a game pixel maps to ours
    let map = f("--map").unwrap_or_else(|| if a.iter().any(|x| x == "--no-rot180") { "none".into() } else { "rot180".into() });
    // --absent V: the depth value meaning "no surface" (0 = the dome of an env layer, the default; 1 = an item layer's clear)
    let absent: f32 = f("--absent").and_then(|v| v.parse().ok()).unwrap_or(0.0);
    let top: usize = f("--top").and_then(|v| v.parse().ok()).unwrap_or(10);
    let (w, h) = (gc.w, gc.h);
    // ours may be a coarser raster (RI: the port peels at 2048², the game at 4096²): an integer factor maps game → ours
    assert!(oc.w == od.w && gd.w == w && w % oc.w == 0 && h % oc.h == 0, "sizes: ours {}×{} / {}×{}, game {}×{} / {}×{}", oc.w, oc.h, od.w, od.h, gc.w, gc.h, gd.w, gd.h);
    let (fx, fy) = (w / oc.w, h / oc.h);
    if fx != 1 || fy != 1 { eprintln!("ours is {}×{}: game pixel (x, y) → ours (x/{fx}, y/{fy})", oc.w, oc.h); }
    eprintln!("{}×{}; map {map}", w, h);
    let ours_at = |gx: u32, gy: u32| -> (u32, u32) { let (x, y) = match map.as_str() { "rot180" => (w - 1 - gx, h - 1 - gy), "flipx" => (w - 1 - gx, gy), "flipy" => (gx, h - 1 - gy), _ => (gx, gy) }; (x / fx, y / fy) };
    // the census
    let mut cls = [[0usize; 2]; 2]; // [game dome? 0/1][ours dome? 0/1] → 0 = surface, 1 = dome
    let mut dome_stats = lightmap::domecheck::QuantaStats::default();
    let (mut n_surf, mut d_exact, mut d_within1, mut d_within4, mut d_sum_abs) = (0usize, 0usize, 0usize, 0usize, 0f64);
    let (mut surf_ours_black, mut surf_game_black, mut surf_both_lit) = (0usize, 0usize, 0usize);
    let (mut sum_o, mut sum_g) = ([0f64; 3], [0f64; 3]);
    let mut worst_dome: Vec<(i64, u32, u32, [f32; 3], [f32; 3])> = Vec::new();
    let mut worst_depth: Vec<(i64, u32, u32, f32, f32)> = Vec::new();
    let inset = 1u32;
    for gy in inset..h - inset {
        for gx in inset..w - inset {
            let (ox, oy) = ours_at(gx, gy);
            let zg = gd.get(gx, gy, 0);
            let zo = od.get(ox, oy, 0);
            let gdome = zg == absent;
            let odome = zo == absent;
            cls[gdome as usize][odome as usize] += 1;
            let cg = [gc.get(gx, gy, 0), gc.get(gx, gy, 1), gc.get(gx, gy, 2)];
            let co = [oc.get(ox, oy, 0), oc.get(ox, oy, 1), oc.get(ox, oy, 2)];
            if gdome && odome {
                let oq = lightmap::gpufmt::quantise_r11g11b10(co, lightmap::gpufmt::Rounding::Truncate);
                lightmap::domecheck::compare_quanta(oq, cg, &mut dome_stats);
                let dsum: i64 = (0..3).map(|k| (lightmap::domecheck::r11_steps(oq[k], k == 2) - lightmap::domecheck::r11_steps(cg[k], k == 2)).abs()).sum();
                if dsum > 0 && worst_dome.len() < 20000 { worst_dome.push((dsum, gx, gy, co, cg)); }
            } else if !gdome && !odome {
                n_surf += 1;
                let steps = ((zg - zo) * 65535.0).round() as i64;
                if steps == 0 { d_exact += 1; }
                if steps.abs() <= 1 { d_within1 += 1; }
                if steps.abs() <= 4 { d_within4 += 1; }
                d_sum_abs += steps.abs() as f64;
                if steps.abs() > 4 && worst_depth.len() < 20000 { worst_depth.push((steps.abs(), gx, gy, zo, zg)); }
                let ob = co == [0.0; 3];
                let gb = cg == [0.0; 3];
                if ob && !gb { surf_ours_black += 1; }
                if gb && !ob { surf_game_black += 1; }
                if !ob && !gb { surf_both_lit += 1; for k in 0..3 { sum_o[k] += co[k] as f64; sum_g[k] += cg[k] as f64; } }
            }
        }
    }
    let n = ((w - 2 * inset) * (h - 2 * inset)) as f64;
    println!("class census (game × ours): both DOME {} ({:.2} %), both SURFACE {} ({:.2} %), game dome / ours surface {} ({:.3} %), game surface / ours dome {} ({:.3} %)", cls[1][1], 100.0 * cls[1][1] as f64 / n, cls[0][0], 100.0 * cls[0][0] as f64 / n, cls[1][0], 100.0 * cls[1][0] as f64 / n, cls[0][1], 100.0 * cls[0][1] as f64 / n);
    let pct = |v: usize| 100.0 * v as f64 / dome_stats.n.max(1) as f64;
    println!("DOME pixels compared {}: R exact {} ({:.2} %) ±1 {} ({:.2} %) | G exact {} ({:.2} %) ±1 {} ({:.2} %) | B exact {} ({:.2} %) ±1 {} ({:.2} %)", dome_stats.n, dome_stats.exact[0], pct(dome_stats.exact[0]), dome_stats.within1[0], pct(dome_stats.within1[0]), dome_stats.exact[1], pct(dome_stats.exact[1]), dome_stats.within1[1], pct(dome_stats.within1[1]), dome_stats.exact[2], pct(dome_stats.exact[2]), dome_stats.within1[2], pct(dome_stats.within1[2]));
    worst_dome.sort_by_key(|x| std::cmp::Reverse(x.0));
    for wd in worst_dome.iter().take(top) { println!("  dome miss ({}, {}): ours ({:.5}, {:.5}, {:.5}) game ({:.5}, {:.5}, {:.5}) — {} quanta", wd.1, wd.2, wd.3[0], wd.3[1], wd.3[2], wd.4[0], wd.4[1], wd.4[2], wd.0); }
    println!("SURFACE pixels {n_surf}: depth exact {} ({:.2} %), ±1 step {} ({:.2} %), ±4 {} ({:.2} %), mean |Δ| {:.2} R16 steps; colour: both black {}, ours black / game lit {}, game black / ours lit {}, both lit {} (mean ours ({:.4}, {:.4}, {:.4}) vs game ({:.4}, {:.4}, {:.4}))", d_exact, 100.0 * d_exact as f64 / n_surf.max(1) as f64, d_within1, 100.0 * d_within1 as f64 / n_surf.max(1) as f64, d_within4, 100.0 * d_within4 as f64 / n_surf.max(1) as f64, d_sum_abs / n_surf.max(1) as f64, n_surf - surf_ours_black - surf_game_black - surf_both_lit, surf_ours_black, surf_game_black, surf_both_lit, sum_o[0] / surf_both_lit.max(1) as f64, sum_o[1] / surf_both_lit.max(1) as f64, sum_o[2] / surf_both_lit.max(1) as f64, sum_g[0] / surf_both_lit.max(1) as f64, sum_g[1] / surf_both_lit.max(1) as f64, sum_g[2] / surf_both_lit.max(1) as f64);
    worst_depth.sort_by_key(|x| std::cmp::Reverse(x.0));
    for wd in worst_depth.iter().take(top) { println!("  depth miss ({}, {}): ours z01 {:.5} game {:.5} — {} steps", wd.1, wd.2, wd.3, wd.4, wd.0); }
    for arg in a.iter().enumerate().filter(|(i, x)| *x == "--at" && *i + 1 < a.len()).map(|(i, _)| a[i + 1].clone()) {
        let v: Vec<u32> = arg.split(',').filter_map(|t| t.trim().parse().ok()).collect();
        if v.len() != 2 { continue; }
        let (gx, gy) = (v[0], v[1]);
        let (ox, oy) = ours_at(gx, gy);
        println!("  probe game ({gx}, {gy}) = ours ({ox}, {oy}): game z {:.5} rgb ({:.5}, {:.5}, {:.5}) | ours z {:.5} rgb ({:.5}, {:.5}, {:.5})", gd.get(gx, gy, 0), gc.get(gx, gy, 0), gc.get(gx, gy, 1), gc.get(gx, gy, 2), od.get(ox, oy, 0), oc.get(ox, oy, 0), oc.get(ox, oy, 1), oc.get(ox, oy, 2));
    }
}
