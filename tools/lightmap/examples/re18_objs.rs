//! `re18_objs ORACLE.Map.Gbx PAK:KEY[,PAK:KEY…] --collection RedIsland [--obj N …] [--plane PLANE.dds.gz]` — the game's object ids of a
//! map's LM records through the port's `records::build_map_records` convention (tile_obj0 / clip_obj0 / item_obj0), the record
//! (class, name, centre, half) behind each asked obj id, and — with a captured SET plane — every chart's lit texels INSIDE its
//! rect vs the lit texels the atlas holds in NO rect (the pads: a raster spill) grouped by the nearest chart (RE 18, 2026-10-01).
fn main() {
    let a: Vec<String> = std::env::args().collect();
    let f = |k: &str| a.iter().position(|x| x == k).and_then(|i| a.get(i + 1)).cloned();
    let map_path = a.get(1).expect("ORACLE.Map.Gbx");
    let mut store = mapgeom::store::DataStore::empty();
    for pk in a.get(2).expect("PAK:KEY").split(',') { let (pp, key) = pk.rsplit_once(':').expect("PAK:KEY"); store.add_pak(pp, key).expect("pak"); }
    let coll = f("--collection").unwrap_or_else(|| "RedIsland".into());
    let opts = lightmap::records::BuildOpts { collection: coll.clone(), zone: None, kept: None, tile_level: None, yoff: None, grid: None, items_3d: false, ghost_marks: false, no_block_cells: false, clip_order_sim: false, face_order: None, one_class: Vec::new() };
    let scene = lightmap::geometry::Scene::from_map(map_path).expect("scene");
    let mr = lightmap::records::build_map_records(map_path, &scene, &mut store, &opts).expect("records");
    for n in &mr.notes { println!("note: {n}"); }
    println!("records {}: blocks {} (obj0 {}), tiles {} (obj0 {}), clips {} (obj0 {}), items {} (obj0 {})", mr.recs.len(), mr.n_blocks, mr.block_obj0, mr.n_tiles, mr.tile_obj0, mr.n_clips, mr.clip_obj0, mr.n_items, mr.item_obj0);
    let by_obj: std::collections::HashMap<(u32, u32), &lightmap::records::Rec> = mr.recs.iter().map(|r| ((r.obj, r.sub), r)).collect();
    let asked: Vec<u32> = a.iter().enumerate().filter(|(_, x)| x.as_str() == "--obj").filter_map(|(i, _)| a.get(i + 1)).filter_map(|v| v.parse().ok()).collect();
    for obj in &asked {
        let mut found = false;
        for r in mr.recs.iter().filter(|r| r.obj == *obj) {
            found = true;
            println!("obj {obj} sub {}: class {} item {:?} centre ({:.2}, {:.2}, {:.2}) half ({:.2}, {:.2}, {:.2}) uv {:?} meter_by_uv {} quality {} scale {} wall {:?}", r.sub, r.class, r.item, r.centre[0], r.centre[1], r.centre[2], r.half[0], r.half[1], r.half[2], r.uv, r.meter_by_uv, r.quality, r.scale, r.wall);
            if let Some((ii, _)) = &r.item { if let Some(it) = scene.instances.iter().find(|x| x.item == *ii) { println!("    item {ii}: model {} pose {:?} lmq {} colour {}", it.model_name, it.pose, it.lm_quality, it.colour); } }
        }
        if !found { println!("obj {obj}: no record"); }
    }
    // the mapping + plane
    let Some(plane_path) = f("--plane") else { return };
    let data = std::fs::read(map_path).expect("read map");
    let g = gbx::Gbx::parse(&data);
    let (_, payload, size) = lightmap::find_chunk(&g.body).expect("LM chunk");
    let lm = lightmap::format::LightmapChunk::parse(&g.body[payload..payload + size]).expect("parse LM chunk");
    let d = lm.data.as_ref().expect("has lightmaps");
    let m = d.cache.mapping().expect("mapping");
    let bytes = std::fs::read(&plane_path).expect("read plane");
    let bytes = if plane_path.ends_with(".gz") { lightmap::passdiff::gunzip(&bytes).expect("gunzip") } else { bytes };
    let img = lightmap::passdiff::load_dds_bytes(&bytes, "R11G11B10_FLOAT", 0, 0).expect("dds");
    let thresh: f32 = f("--thresh").and_then(|v| v.parse().ok()).unwrap_or(0.0);
    // owner map
    let (w, h) = (img.w as usize, img.h as usize);
    let mut owner = vec![u32::MAX; w * h];
    for i in 0..m.count as usize { let (x, y) = m.pos[i]; let (cw, ch) = m.size[i]; for yy in y as usize..(y + ch) as usize { for xx in x as usize..(x + cw) as usize { if xx < w && yy < h { owner[yy * w + xx] = i as u32; } } } }
    let (mut lit_owned, mut lit_unowned, mut owned, mut unowned) = (0usize, 0usize, 0usize, 0usize);
    let mut per_class: std::collections::BTreeMap<String, (usize, usize)> = Default::default(); // class → (texels, lit)
    for yy in 0..h { for xx in 0..w {
        let c = [img.get(xx as u32, yy as u32, 0), img.get(xx as u32, yy as u32, 1), img.get(xx as u32, yy as u32, 2)];
        let l = c[0].max(c[1]).max(c[2]) > thresh;
        let o = owner[yy * w + xx];
        if o == u32::MAX { unowned += 1; if l { lit_unowned += 1; } } else {
            owned += 1; if l { lit_owned += 1; }
            let obj = m.binds[o as usize].obj_group_idx / 4; let sub = m.binds[o as usize].obj_idx & 0x00ff_ffff;
            let key = by_obj.get(&(obj, sub)).map(|r| if r.class == "item" { format!("item:{}", r.item.as_ref().map(|t| t.1.clone()).unwrap_or_default()) } else { r.class.to_string() }).unwrap_or_else(|| format!("obj {obj}?"));
            let e = per_class.entry(key).or_default(); e.0 += 1; if l { e.1 += 1; }
        }
    } }
    println!("plane {}×{}: owned texels {owned} (lit {lit_owned}), UNOWNED (pads/gaps) {unowned} (lit {lit_unowned} — spill)", img.w, img.h);
    let mut pc: Vec<_> = per_class.into_iter().collect();
    pc.sort_by_key(|(_, (_, l))| std::cmp::Reverse(*l));
    println!("lit texels per class (inside the class's rects): ");
    for (k, (t, l)) in pc.iter().take(25) { println!("  {l:7} / {t:7}  {k}"); }
    // --flagged: every mapping chart whose obj_idx carries flag bits in the top byte (bind flags), with its record
    if a.iter().any(|x| x == "--flagged") { for i in 0..m.count as usize { let flags = m.binds[i].obj_idx >> 24; if flags == 0 { continue; } let obj = m.binds[i].obj_group_idx / 4; let sub = m.binds[i].obj_idx & 0x00ff_ffff; let (x, y) = m.pos[i]; let (cw, ch) = m.size[i]; let r = by_obj.get(&(obj, sub)); println!("flagged chart {i}: flags {:#x} obj {obj} sub {sub} group_word {:#x} rect ({x}, {y}) {cw}×{ch} {}", flags, m.binds[i].obj_group_idx, r.map(|r| format!("{} ii {:?} centre ({:.2}, {:.2}, {:.2}) half ({:.2}, {:.2}, {:.2}) q {} scale {}", r.class, r.item.as_ref().map(|t| t.0), r.centre[0], r.centre[1], r.centre[2], r.half[0], r.half[1], r.half[2], r.quality, r.scale)).unwrap_or_else(|| "(no record)".into())); } }
    // --chart I: the mapping entry I → its bind (obj, sub), rect and the record behind it
    for ci in a.iter().enumerate().filter(|(_, x)| x.as_str() == "--chart").filter_map(|(i, _)| a.get(i + 1)).filter_map(|v| v.parse::<usize>().ok()) { let obj = m.binds[ci].obj_group_idx / 4; let sub = m.binds[ci].obj_idx & 0x00ff_ffff; let (x, y) = m.pos[ci]; let (cw, ch) = m.size[ci]; let r = by_obj.get(&(obj, sub)); println!("mapping chart {ci}: obj {obj} sub {sub} rect ({x}, {y}) {cw}×{ch} {}", r.map(|r| format!("{} {:?} centre ({:.1}, {:.1}, {:.1}) half ({:.1}, {:.1}, {:.1}) uv {:?}", r.class, r.item, r.centre[0], r.centre[1], r.centre[2], r.half[0], r.half[1], r.half[2], r.uv)).unwrap_or_else(|| "(no record)".into())); }
    // --chart-lit I: the lit texels inside chart I's rect (first 40) and the rect's lit count
    if let Some(ci) = f("--chart-lit") { let i: usize = ci.parse().unwrap(); let (x, y) = m.pos[i]; let (cw, ch) = m.size[i]; let mut n = 0; let mut listed = 0; let mut rows: Vec<String> = Vec::new(); for yy in y..y + ch { let mut line = format!("{yy:5} "); for xx in x..x + cw { let c = [img.get(xx as u32, yy as u32, 0), img.get(xx as u32, yy as u32, 1), img.get(xx as u32, yy as u32, 2)]; let l = c[0].max(c[1]).max(c[2]) > thresh; if l { n += 1; if listed < 40 { listed += 1; println!("  lit ({xx}, {yy}) rgb ({:.4}, {:.4}, {:.4})", c[0], c[1], c[2]); } } line.push(if l { '*' } else { '.' }); } rows.push(line); } println!("chart {i} rect ({x}, {y}) {cw}×{ch}: lit {n}/{}", cw as usize * ch as usize); for r in rows { println!("{r}"); } }
    // the charts whose rect is adjacent to lit unowned texels: for every lit unowned texel, the nearest chart within 2 texels left/up/right/down
    let mut spill_by_chart: std::collections::BTreeMap<u32, usize> = Default::default();
    for yy in 0..h { for xx in 0..w {
        if owner[yy * w + xx] != u32::MAX { continue; }
        let c = [img.get(xx as u32, yy as u32, 0), img.get(xx as u32, yy as u32, 1), img.get(xx as u32, yy as u32, 2)];
        if c[0].max(c[1]).max(c[2]) <= thresh { continue; }
        // walk left up to 40 texels to the first owned texel whose chart is an item (the spill source candidate)
        let mut src = u32::MAX;
        for dx in 1..=40usize { if xx >= dx { let o = owner[yy * w + xx - dx]; if o != u32::MAX { src = o; break; } } }
        *spill_by_chart.entry(src).or_default() += 1;
    } }
    let mut sv: Vec<_> = spill_by_chart.into_iter().collect();
    sv.sort_by_key(|(_, n)| std::cmp::Reverse(*n));
    println!("lit UNOWNED texels by the first chart found walking LEFT (≤ 40 texels): ");
    for (c, n) in sv.iter().take(12) {
        if *c == u32::MAX { println!("  {n:7}  (no chart within 40 to the left)"); continue; }
        let i = *c as usize; let obj = m.binds[i].obj_group_idx / 4; let sub = m.binds[i].obj_idx & 0x00ff_ffff; let (x, y) = m.pos[i]; let (cw, ch) = m.size[i];
        let r = by_obj.get(&(obj, sub));
        println!("  {n:7}  chart {i} obj {obj} sub {sub} rect ({x}, {y}) {cw}×{ch} {}", r.map(|r| format!("{} {:?} centre ({:.1}, {:.1}, {:.1}) half ({:.1}, {:.1}, {:.1}) uv {:?}", r.class, r.item, r.centre[0], r.centre[1], r.centre[2], r.half[0], r.half[1], r.half[2], r.uv)).unwrap_or_default());
    }
}
