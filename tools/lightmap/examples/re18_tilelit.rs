//! `re18_tilelit ORACLE.Map.Gbx PLANE.dds[.gz] [--grid 96] [--collection RedIsland] [--thresh 1e-4] [--top N] [--class-size W,H]`
//! — which ZONE-TILE charts of the game's own mapping are lit in a captured per-direction SET target (the 2048² R11G11B10
//! ilightdir plane, e.g. passcap/ri-summer07-prepass-compute/tex-frame1122-k736/e069940_15325.dds.gz = k735's final accumulate):
//! per tile chart the lit texel count, the lit tiles' CELLS (the game's tile object ids = the port's `records::build_map_records`
//! convention: tile_obj0 = block_obj0 + the authored blocks, cells = the baked blocks' cells first then x-major), an ASCII map of
//! the grid, and the brightest lit tile texels as DebugPixel candidates (RE 18, 2026-10-01 — the RI tile-normal read).
fn main() {
    let a: Vec<String> = std::env::args().collect();
    let f = |k: &str| a.iter().position(|x| x == k).and_then(|i| a.get(i + 1)).cloned();
    let map_path = a.get(1).expect("ORACLE.Map.Gbx");
    let plane_path = a.get(2).expect("PLANE.dds[.gz]");
    let grid: i32 = f("--grid").and_then(|v| v.parse().ok()).unwrap_or(96);
    let coll = f("--collection").unwrap_or_else(|| "RedIsland".into());
    let thresh: f32 = f("--thresh").and_then(|v| v.parse().ok()).unwrap_or(1e-4);
    let top: usize = f("--top").and_then(|v| v.parse().ok()).unwrap_or(12);
    // the mapping
    let data = std::fs::read(map_path).expect("read map");
    let g = gbx::Gbx::parse(&data);
    let (_, payload, size) = lightmap::find_chunk(&g.body).expect("LM chunk");
    let lm = lightmap::format::LightmapChunk::parse(&g.body[payload..payload + size]).expect("parse LM chunk");
    let d = lm.data.as_ref().expect("has lightmaps");
    let m = d.cache.mapping().expect("mapping");
    println!("mapping: {} charts, atlas {}×{}", m.count, m.atlas_w, m.atlas_h);
    // the plane
    let bytes = std::fs::read(plane_path).expect("read plane");
    let bytes = if plane_path.ends_with(".gz") { lightmap::passdiff::gunzip(&bytes).expect("gunzip") } else { bytes };
    let img = lightmap::passdiff::load_dds_bytes(&bytes, "R11G11B10_FLOAT", 0, 0).expect("dds");
    println!("plane: {}×{} ({} channels)", img.w, img.h, img.channels);
    let lit = |x: u32, y: u32| -> Option<[f32; 3]> {
        if x >= img.w || y >= img.h { return None; }
        let c = [img.get(x, y, 0), img.get(x, y, 1), img.get(x, y, 2)];
        if c[0].max(c[1]).max(c[2]) > thresh { Some(c) } else { None }
    };
    // --dump-rects FILE: every chart (index, obj, sub, x, y, w, h, tile?) as TSV
    if let Some(p) = f("--dump-rects") { use std::io::Write; let mut fh = std::fs::File::create(&p).expect("dump-rects"); for i in 0..m.count as usize { let obj = m.binds[i].obj_group_idx / 4; let sub = m.binds[i].obj_idx & 0x00ff_ffff; let (x, y) = m.pos[i]; let (w, h) = m.size[i]; writeln!(fh, "{x} {y} {w} {h} {i} {obj} {sub}").unwrap(); } }
    // the whole plane census
    let (mut n_all, mut sum_all) = (0usize, [0f64; 3]);
    for y in 0..img.h { for x in 0..img.w { if let Some(c) = lit(x, y) { n_all += 1; for ch in 0..3 { sum_all[ch] += c[ch] as f64; } } } }
    println!("plane lit texels (max channel > {thresh}): {n_all} ({:.2} %), mean over lit ({:.4}, {:.4}, {:.4})", 100.0 * n_all as f64 / (img.w * img.h) as f64, sum_all[0] / n_all.max(1) as f64, sum_all[1] / n_all.max(1) as f64, sum_all[2] / n_all.max(1) as f64);
    // the tile object ids (the port's convention, verified on the game's bind words)
    let mf = tmmaps::map::MapFile::load(std::path::Path::new(map_path));
    let prof = lightmap::layout::CollectionProfile::of(&coll);
    let is_zone_block = |b: &tmmaps::map::BlockRec| b.flags & 0x1000 != 0 && prof.flat_zones.contains(&b.name.as_str());
    let has_authored = mf.blocks.iter().any(|b| !is_zone_block(b));
    let block_obj0: u32 = if has_authored { 16384 } else { 0 };
    let n_auth = mf.blocks.iter().filter(|b| !is_zone_block(b)).count() as u32;
    let tile_obj0 = block_obj0 + n_auth;
    let baked: Vec<(i32, i32)> = mf.baked.iter().map(|b| { let (x, _, z) = b.coords(); (x, z) }).collect();
    let cells = lightmap::layout::tile_cells(&baked, grid, grid);
    let n_tiles = cells.len();
    println!("blocks {} (authored {}; zone blocks {}), baked {}, block_obj0 {block_obj0}, tile_obj0 {tile_obj0}, tiles {n_tiles} (grid {grid})", mf.blocks.len(), n_auth, mf.blocks.len() as u32 - n_auth, mf.baked.len());
    // the baked blocks' names per cell (the zone of the cell, when the editor baked one)
    let mut baked_name: std::collections::HashMap<(i32, i32), Vec<String>> = Default::default();
    for b in &mf.baked { let (x, y, z) = b.coords(); baked_name.entry((x, z)).or_default().push(format!("{}@y{}d{}", b.name, y, b.dir)); }
    // a per-size census of the mapping
    let mut sizes: std::collections::BTreeMap<(u16, u16), usize> = Default::default();
    for i in 0..m.count as usize { *sizes.entry(m.size[i]).or_default() += 1; }
    let mut sv: Vec<_> = sizes.into_iter().collect();
    sv.sort_by_key(|(_, n)| std::cmp::Reverse(*n));
    println!("chart sizes (top 8): {:?}", &sv[..sv.len().min(8)]);
    // per tile chart
    let mut hist = [0usize; 6]; // 0, 1, 2-3, 4-8, 9-15, 16+
    let mut lit_tiles: Vec<(usize, u32, (i32, i32), usize, [f64; 3], (u16, u16), (u16, u16))> = Vec::new();
    let mut n_tile_charts = 0usize;
    let mut tile_texels = 0usize;
    let mut tile_lit = 0usize;
    let mut cand: Vec<(f32, u32, u32, [f32; 3], usize, (i32, i32))> = Vec::new();
    let mut obj_hist: std::collections::BTreeMap<u32, usize> = Default::default();
    for i in 0..m.count as usize {
        let obj = m.binds[i].obj_group_idx / 4;
        let sub = m.binds[i].obj_idx & 0x00ff_ffff;
        if obj < tile_obj0 || obj >= tile_obj0 + n_tiles as u32 { continue; }
        let k = (obj - tile_obj0) as usize;
        let cell = cells[k];
        n_tile_charts += 1;
        *obj_hist.entry(sub).or_default() += 1;
        let (x, y) = m.pos[i];
        let (w, h) = m.size[i];
        let mut n = 0usize;
        let mut acc = [0f64; 3];
        for yy in y..y + h { for xx in x..x + w {
            tile_texels += 1;
            if let Some(c) = lit(xx as u32, yy as u32) {
                n += 1; for ch in 0..3 { acc[ch] += c[ch] as f64; }
                cand.push((c[0].max(c[1]).max(c[2]), xx as u32, yy as u32, c, i, cell));
            }
        } }
        tile_lit += n;
        let bin = match n { 0 => 0, 1 => 1, 2..=3 => 2, 4..=8 => 3, 9..=15 => 4, _ => 5 };
        hist[bin] += 1;
        if n > 0 { lit_tiles.push((i, obj, cell, n, [acc[0] / n as f64, acc[1] / n as f64, acc[2] / n as f64], (x, y), (w, h))); }
    }
    println!("tile charts {n_tile_charts} (sub histogram {:?}), texels {tile_texels}, lit {tile_lit} ({:.2} %)", obj_hist, 100.0 * tile_lit as f64 / tile_texels.max(1) as f64);
    println!("lit-texels-per-tile histogram: 0: {} | 1: {} | 2–3: {} | 4–8: {} | 9–15: {} | 16+: {}", hist[0], hist[1], hist[2], hist[3], hist[4], hist[5]);
    // the ASCII map (row = cz, col = cx): '.' unlit tile, digits = lit count bucket, 'B' marks a baked cell among the lit
    let mut grid_lit: std::collections::HashMap<(i32, i32), usize> = Default::default();
    for t in &lit_tiles { grid_lit.insert(t.2, t.3); }
    println!("grid map (row cz 0..{grid}, col cx 0..{grid}; '.' unlit, 1 = 1 texel, 2 = 2–3, 4 = 4–8, 9 = 9–15, F = all 16, b = baked cell unlit):");
    for cz in 0..grid {
        let mut line = String::with_capacity(grid as usize);
        for cx in 0..grid {
            let ch = match grid_lit.get(&(cx, cz)) {
                Some(&n) => match n { 1 => '1', 2..=3 => '2', 4..=8 => '4', 9..=15 => '9', _ => 'F' },
                None => if baked_name.contains_key(&(cx, cz)) { 'b' } else { '.' },
            };
            line.push(ch);
        }
        println!("{cz:3} {line}");
    }
    // the lit tiles' baked names (what zone block sits on the cell)
    let mut zone_census: std::collections::BTreeMap<String, usize> = Default::default();
    for t in &lit_tiles {
        let key = baked_name.get(&t.2).map(|v| v.join("+")).unwrap_or_else(|| "(no baked block)".into());
        *zone_census.entry(key).or_default() += 1;
    }
    println!("lit tiles by the cell's baked block(s): ");
    let mut zc: Vec<_> = zone_census.into_iter().collect();
    zc.sort_by_key(|(_, n)| std::cmp::Reverse(*n));
    for (k, n) in zc.iter().take(30) { println!("  {n:5}  {k}"); }
    // the same census over ALL tiles (for the base rate)
    let mut all_zone: std::collections::BTreeMap<String, usize> = Default::default();
    for &(cx, cz) in &cells { let key = baked_name.get(&(cx, cz)).map(|v| v.iter().map(|s| s.split('@').next().unwrap_or("").to_string()).collect::<Vec<_>>().join("+")).unwrap_or_else(|| "(no baked block)".into()); *all_zone.entry(key).or_default() += 1; }
    println!("all tiles by baked block NAME: {:?}", all_zone);
    // the lit tile texels by R bin (0.1 steps) with the G/R, B/R means
    let mut rb: std::collections::BTreeMap<u32, (usize, f64, f64)> = Default::default();
    for c in &cand { let e = rb.entry((c.3[0] * 10.0) as u32).or_default(); e.0 += 1; if c.3[0] > 0.0 { e.1 += (c.3[1] / c.3[0]) as f64; e.2 += (c.3[2] / c.3[0]) as f64; } }
    println!("lit tile texels by R (bin = R×10): {}", rb.iter().map(|(k, v)| format!("{:.1}–{:.1}: {} (G/R {:.2} B/R {:.2})", *k as f64 / 10.0, (*k + 1) as f64 / 10.0, v.0, v.1 / v.0.max(1) as f64, v.2 / v.0.max(1) as f64)).collect::<Vec<_>>().join("; "));
    // the candidates
    cand.sort_by(|p, q| q.0.partial_cmp(&p.0).unwrap());
    println!("brightest lit tile texels (atlas x, y in the {}² plane; rgb; chart; cell cx,cz; the cell's baked blocks):", img.w);
    for c in cand.iter().take(top) {
        let bn = baked_name.get(&c.5).map(|v| v.join("+")).unwrap_or_else(|| "-".into());
        let (x, y) = m.pos[c.4]; let (w, h) = m.size[c.4];
        println!("  ({:4}, {:4}) rgb ({:.4}, {:.4}, {:.4}) chart {} rect ({x},{y}) {w}×{h} cell ({}, {}) {bn}", c.1, c.2, c.3[0], c.3[1], c.3[2], c.4, c.5 .0, c.5 .1);
    }
    // --around X,Y [--radius R]: the atlas neighbourhood — per texel the owning chart (its index mod 1000 / "...." = no chart) and the lit mark
    if let Some(ar) = f("--around") {
        let (sx, sy) = ar.split_once(',').expect("--around X,Y");
        let (cx0, cy0): (i64, i64) = (sx.parse().unwrap(), sy.parse().unwrap());
        let r: i64 = f("--radius").and_then(|v| v.parse().ok()).unwrap_or(12);
        let mut owner: std::collections::HashMap<(i64, i64), usize> = Default::default();
        for i in 0..m.count as usize { let (x, y) = m.pos[i]; let (w, h) = m.size[i]; for yy in y as i64..(y + h) as i64 { for xx in x as i64..(x + w) as i64 { if (xx - cx0).abs() <= r && (yy - cy0).abs() <= r { owner.insert((xx, yy), i); } } } }
        println!("atlas neighbourhood of ({cx0}, {cy0}) ± {r}: each cell = chart index (T = tile chart) + lit mark (* lit, . dark)");
        let mut charts_seen: std::collections::BTreeSet<usize> = Default::default();
        for yy in (cy0 - r)..=(cy0 + r) {
            let mut line = format!("{yy:5} ");
            for xx in (cx0 - r)..=(cx0 + r) {
                let l = if xx >= 0 && yy >= 0 { lit(xx as u32, yy as u32).is_some() } else { false };
                match owner.get(&(xx, yy)) {
                    Some(&i) => { charts_seen.insert(i); let obj = m.binds[i].obj_group_idx / 4; let is_tile = obj >= tile_obj0 && obj < tile_obj0 + n_tiles as u32; line.push_str(&format!("{}{:4}{}", if is_tile { "T" } else { " " }, i % 10000, if l { "*" } else { "." })); }
                    None => line.push_str(&format!("  ....{}", if l { "*" } else { "." })),
                }
            }
            println!("{line}");
        }
        for i in charts_seen { let (x, y) = m.pos[i]; let (w, h) = m.size[i]; let obj = m.binds[i].obj_group_idx / 4; let sub = m.binds[i].obj_idx & 0x00ff_ffff; let mut n = 0; let mut acc = [0f64; 3]; for yy in y..y + h { for xx in x..x + w { if let Some(c) = lit(xx as u32, yy as u32) { n += 1; for ch in 0..3 { acc[ch] += c[ch] as f64; } } } } println!("  chart {i}: obj {obj} sub {sub} rect ({x}, {y}) {w}×{h} lit {n}/{} mean ({:.3}, {:.3}, {:.3}){}", w as usize * h as usize, acc[0] / n.max(1) as f64, acc[1] / n.max(1) as f64, acc[2] / n.max(1) as f64, if obj >= tile_obj0 && obj < tile_obj0 + n_tiles as u32 { format!(" TILE cell {:?}", cells[(obj - tile_obj0) as usize]) } else { String::new() }); }
    }
    // --other PLANE2: the joint census (per tile: lit in this plane × lit in the other)
    if let Some(p2) = f("--other") {
        let b2 = std::fs::read(&p2).expect("read other");
        let b2 = if p2.ends_with(".gz") { lightmap::passdiff::gunzip(&b2).expect("gunzip") } else { b2 };
        let img2 = lightmap::passdiff::load_dds_bytes(&b2, "R11G11B10_FLOAT", 0, 0).expect("dds2");
        let lit2 = |x: u32, y: u32| -> Option<[f32; 3]> { if x >= img2.w || y >= img2.h { return None; } let c = [img2.get(x, y, 0), img2.get(x, y, 1), img2.get(x, y, 2)]; if c[0].max(c[1]).max(c[2]) > thresh { Some(c) } else { None } };
        let mut joint = [[0usize; 3]; 3]; // [A: 0 none, 1 partial, 2 full][B: same]
        let mut both_full: Vec<(usize, (i32, i32), [f64; 3], [f64; 3], (u16, u16))> = Vec::new();
        let mut texel_joint = [[0usize; 2]; 2];
        for i in 0..m.count as usize {
            let obj = m.binds[i].obj_group_idx / 4;
            if obj < tile_obj0 || obj >= tile_obj0 + n_tiles as u32 { continue; }
            let cell = cells[(obj - tile_obj0) as usize];
            let (x, y) = m.pos[i]; let (w, h) = m.size[i];
            let (mut na, mut nb) = (0usize, 0usize); let (mut sa, mut sb) = ([0f64; 3], [0f64; 3]);
            for yy in y..y + h { for xx in x..x + w {
                let a = lit(xx as u32, yy as u32); let b = lit2(xx as u32, yy as u32);
                texel_joint[a.is_some() as usize][b.is_some() as usize] += 1;
                if let Some(c) = a { na += 1; for ch in 0..3 { sa[ch] += c[ch] as f64; } }
                if let Some(c) = b { nb += 1; for ch in 0..3 { sb[ch] += c[ch] as f64; } }
            } }
            let full = (w * h) as usize;
            let cls = |n: usize| if n == 0 { 0 } else if n < full { 1 } else { 2 };
            joint[cls(na)][cls(nb)] += 1;
            if na == full && nb == full { both_full.push((i, cell, [sa[0] / na as f64, sa[1] / na as f64, sa[2] / na as f64], [sb[0] / nb as f64, sb[1] / nb as f64, sb[2] / nb as f64], (x, y))); }
        }
        println!("JOINT tile census A = this plane, B = {p2}: tiles [A none/partial/full] × [B none/partial/full] = {:?}", joint);
        println!("  texels: A&B both lit {}, A only {}, B only {}, neither {}", texel_joint[1][1], texel_joint[1][0], texel_joint[0][1], texel_joint[0][0]);
        println!("  tiles fully lit in BOTH ({}; first 15): chart, cell, mean A, mean B, rect", both_full.len());
        for t in both_full.iter().take(15) { println!("    chart {} cell ({}, {}) A ({:.3}, {:.3}, {:.3}) B ({:.3}, {:.3}, {:.3}) rect ({}, {})", t.0, t.1 .0, t.1 .1, t.2[0], t.2[1], t.2[2], t.3[0], t.3[1], t.3[2], t.4 .0, t.4 .1); }
    }
    // the lit tiles with ALL texels lit, a few, with their rect
    println!("fully lit tiles (first 12):");
    for t in lit_tiles.iter().filter(|t| t.3 as u16 >= t.6 .0 * t.6 .1).take(12) {
        let bn = baked_name.get(&t.2).map(|v| v.join("+")).unwrap_or_else(|| "-".into());
        println!("  chart {} obj {} cell ({}, {}) rect ({}, {}) {}×{} lit {} mean ({:.4}, {:.4}, {:.4}) {bn}", t.0, t.1, t.2 .0, t.2 .1, t.5 .0, t.5 .1, t.6 .0, t.6 .1, t.3, t.4[0], t.4[1], t.4[2]);
    }
}
