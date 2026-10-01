//! `re18_uvbox MAP.Gbx PAK:KEY[,…] [--collection RedIsland] [--cover X,Y]` — per embedded item MODEL: the LM mesh's uv1 extent vs
//! the record's PreLightGen box (the chart's ST maps the BOX to the rect; triangles with uv1 outside the box rasterise OUTSIDE the
//! rect = the RASTER SPILL into the neighbouring charts — REAL in the game: baker-8's pixel history 16:32Z, AI06207013/004's
//! fragments 8–128 px below their 3.8-px rects), the overshoot per side in box units and in atlas texels of the map's own mapping
//! (when the map carries one), and the fraction of the model's LM triangles that lie (partly) outside the box. `--cover X,Y`:
//! every chart whose UNCLIPPED uv1 footprint (uv1 extent through the chart ST = rect/box) covers the texel (RE 18, 2026-10-01).
fn main() {
    let a: Vec<String> = std::env::args().collect();
    let f = |k: &str| a.iter().position(|x| x == k).and_then(|i| a.get(i + 1)).cloned();
    let map_path = a.get(1).expect("MAP.Gbx");
    let mut store = mapgeom::store::DataStore::empty();
    for pk in a.get(2).expect("PAK:KEY").split(',') { let (pp, key) = pk.rsplit_once(':').expect("PAK:KEY"); store.add_pak(pp, key).expect("pak"); }
    let coll = f("--collection").unwrap_or_else(|| "RedIsland".into());
    let opts = lightmap::records::BuildOpts { collection: coll.clone(), zone: None, kept: None, tile_level: None, yoff: None, grid: None, items_3d: false, ghost_marks: false, no_block_cells: false, clip_order_sim: false, face_order: None, one_class: Vec::new() };
    let scene = lightmap::geometry::Scene::from_map(map_path).expect("scene");
    let mr = lightmap::records::build_map_records(map_path, &scene, &mut store, &opts).expect("records");
    // the mapping (for the rect sizes), when the map carries one
    let data = std::fs::read(map_path).expect("read map");
    let g = gbx::Gbx::parse(&data);
    let lm = lightmap::find_chunk(&g.body).and_then(|(_, payload, size)| lightmap::format::LightmapChunk::parse(&g.body[payload..payload + size]).ok());
    let rect_of: std::collections::HashMap<(u32, u32), (u16, u16, u16, u16)> = match lm.as_ref().and_then(|lm| lm.data.as_ref()).and_then(|d| d.cache.mapping()) {
        Some(m) => (0..m.count as usize).map(|i| ((m.binds[i].obj_group_idx / 4, m.binds[i].obj_idx & 0x00ff_ffff), (m.pos[i].0, m.pos[i].1, m.size[i].0, m.size[i].1))).collect(),
        None => Default::default(),
    };
    let mf = tmmaps::map::MapFile::load(std::path::Path::new(map_path));
    let mut files = mapgeom::embedded::files(&mf).expect("embedded files");
    // the STOCK models (the campaign maps embed nothing): every item model the records name that no embedded file carries → the pak's Items file
    {
        let have: std::collections::HashSet<String> = files.iter().map(|(n, _)| n.rsplit(['/', '\\']).next().unwrap_or(n).to_string()).collect();
        let mut names: Vec<String> = mr.recs.iter().filter(|r| r.class == "item").filter_map(|r| r.item.as_ref().map(|t| t.1.split(' ').next().unwrap_or("").to_string())).collect();
        names.sort(); names.dedup();
        let mut n_stock = 0usize;
        for n in names { if have.contains(&n) { continue; } let stem = n.trim_end_matches(".Item.Gbx"); if let Some(p) = mapgeom::tiny_library::find_item_file(&store, stem) { if let Ok(b) = store.read(&p) { files.insert(n.clone(), (*b).clone()); n_stock += 1; } } }
        eprintln!("stock item models resolved from the paks: {n_stock}");
    }
    // per model: the record box + one rect + the instance count
    let mut per_model: std::collections::BTreeMap<String, ([f32; 4], Option<(u16, u16)>, usize)> = Default::default();
    for r in mr.recs.iter().filter(|r| r.class == "item") {
        let Some((_, name)) = &r.item else { continue };
        let base = name.split(' ').next().unwrap_or(name).to_string();
        let e = per_model.entry(base).or_insert((r.uv, None, 0));
        e.2 += 1;
        if e.1.is_none() { if let Some(rc) = rect_of.get(&(r.obj, r.sub)) { e.1 = Some((rc.2, rc.3)); } }
    }
    println!("model\tinstances\tbox u0,v0,u1,v1\trect w×h\tLM tris\tuv1 u min..max\tuv1 v min..max\ttris outside box\tspill texels L/R/U/D\tspill box-units L/R/U/D");
    let mut total_spill_tris = 0usize;
    let (mut n_models, mut n_spill_models, mut n_spill_inst) = (0usize, 0usize, 0usize);
    for (name, bytes) in &files {
        let base = name.rsplit(['/', '\\']).next().unwrap_or(name).to_string();
        let Some((bx, rect, n_inst)) = per_model.get(&base) else { continue };
        n_models += 1;
        let Ok(Some(mesh)) = lightmap::lmmesh::lm_mesh_of_item(bytes) else { println!("{base}\t{n_inst}\t{:?}\t-\tno LM mesh", bx); continue };
        let (mut umin, mut umax, mut vmin, mut vmax) = (f32::MAX, f32::MIN, f32::MAX, f32::MIN);
        for v in &mesh.verts { umin = umin.min(v.uv[0]); umax = umax.max(v.uv[0]); vmin = vmin.min(v.uv[1]); vmax = vmax.max(v.uv[1]); }
        let inside = |uv: [f32; 2]| uv[0] >= bx[0] - 1e-4 && uv[0] <= bx[2] + 1e-4 && uv[1] >= bx[1] - 1e-4 && uv[1] <= bx[3] + 1e-4;
        let n_tris = mesh.indices.len() / 3;
        let out_tris = mesh.indices.chunks(3).filter(|t| t.iter().any(|&i| !inside(mesh.verts[i as usize].uv))).count();
        total_spill_tris += out_tris * n_inst;
        if out_tris > 0 { n_spill_models += 1; n_spill_inst += n_inst; }
        let (bw, bh) = ((bx[2] - bx[0]).max(1e-6), (bx[3] - bx[1]).max(1e-6));
        let (rw, rh) = rect.map(|(w, h)| (w as f32, h as f32)).unwrap_or((0.0, 0.0));
        let (bl, br, bu, bd) = ((bx[0] - umin).max(0.0) / bw, (umax - bx[2]).max(0.0) / bw, (bx[1] - vmin).max(0.0) / bh, (vmax - bx[3]).max(0.0) / bh);
        println!("{base}\t{n_inst}\t[{:.4}, {:.4}, {:.4}, {:.4}]\t{}\t{n_tris}\t{umin:.4}..{umax:.4}\t{vmin:.4}..{vmax:.4}\t{out_tris}\t{}/{}/{}/{}\t{bl:.2}/{br:.2}/{bu:.2}/{bd:.2}", bx[0], bx[1], bx[2], bx[3], rect.map(|(w, h)| format!("{w}×{h}")).unwrap_or("-".into()), (bl * rw).round(), (br * rw).round(), (bu * rh).round(), (bd * rh).round());
    }
    println!("total spilling triangles × instances: {total_spill_tris}; models with spill {n_spill_models} of {n_models}; spilling instances {n_spill_inst}");
    // --cover X,Y: every chart whose UNCLIPPED uv1 footprint covers the texel
    if let Some(cv) = f("--cover") {
        let (sx, sy) = cv.split_once(',').unwrap();
        let (px, py): (f32, f32) = (sx.parse::<f32>().unwrap() + 0.5, sy.parse::<f32>().unwrap() + 0.5);
        let by_model: std::collections::HashMap<String, &Vec<u8>> = files.iter().map(|(n, b)| (n.rsplit(['/', '\\']).next().unwrap_or(n).to_string(), b)).collect();
        let mut ext_cache: std::collections::HashMap<String, [f32; 4]> = Default::default();
        println!("charts whose unclipped uv1 footprint covers texel ({sx}, {sy}):");
        for r in mr.recs.iter().filter(|r| r.class == "item") {
            let Some((ii, name)) = &r.item else { continue };
            let base = name.split(' ').next().unwrap_or(name).to_string();
            let ext = match ext_cache.get(&base) {
                Some(e) => *e,
                None => {
                    let Some(b) = by_model.get(&base) else { continue };
                    let Ok(Some(mesh)) = lightmap::lmmesh::lm_mesh_of_item(b) else { continue };
                    let mut e = [f32::MAX, f32::MAX, f32::MIN, f32::MIN];
                    for v in &mesh.verts { e[0] = e[0].min(v.uv[0]); e[1] = e[1].min(v.uv[1]); e[2] = e[2].max(v.uv[0]); e[3] = e[3].max(v.uv[1]); }
                    ext_cache.insert(base.clone(), e);
                    e
                }
            };
            let Some(rc) = rect_of.get(&(r.obj, r.sub)) else { continue };
            let bx = r.uv;
            let (bw, bh) = ((bx[2] - bx[0]).max(1e-6), (bx[3] - bx[1]).max(1e-6));
            let to_x = |u: f32| rc.0 as f32 + (u - bx[0]) / bw * rc.2 as f32;
            let to_y = |v: f32| rc.1 as f32 + (v - bx[1]) / bh * rc.3 as f32;
            let (x0, y0, x1, y1) = (to_x(ext[0]), to_y(ext[1]), to_x(ext[2]), to_y(ext[3]));
            if px >= x0.min(x1) && px <= x0.max(x1) && py >= y0.min(y1) && py <= y0.max(y1) {
                println!("  obj {} sub {} item {ii} {base} rect ({}, {}) {}×{} box {:?} uv1 ext {:?} → footprint x [{:.0}, {:.0}] y [{:.0}, {:.0}]", r.obj, r.sub, rc.0, rc.1, rc.2, rc.3, bx, ext, x0.min(x1), x0.max(x1), y0.min(y1), y0.max(y1));
            }
        }
    }
}
