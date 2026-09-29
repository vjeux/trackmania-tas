//! `e6_itemwater MAP.Map.Gbx --pak FILE:KEY … [NAME_SUBSTR]` — the embedded ITEM models' WATER geoms (a shaded geom whose
//! material's shader is the water shader, `waterid::is_water_material` through the packs): per model the geom, its material
//! link, the LOD mask, triangle count, the vertex-y range in model space, the BLENDINDICES water type, and the PLACEMENTS'
//! world surface heights (y of the placement + the geom's mean y). Why (E6 2026-09-29, cell 3 / V5's row 4c): RedIsland's
//! campaign lake is LakeBottom ITEM placements under water and the bake's water-id map knows only the collection's sea plane
//! (−0.3): `waterid::water_quads_of_records` scans the block RECORDS' prefab entities, never the items — an item carrying the
//! lake's own water surface would be the plane the LmBlendWaterFog tint needs.
fn main() {
    let a: Vec<String> = std::env::args().collect();
    let mut store = mapgeom::store::DataStore::empty();
    let mut path = String::new();
    let mut filt = String::new();
    let mut i = 1;
    while i < a.len() {
        if a[i] == "--pak" { let (p, k) = a[i + 1].rsplit_once(':').expect("--pak FILE:KEY"); store.add_pak(p, k).expect("pak"); i += 2; }
        else if path.is_empty() { path = a[i].clone(); i += 1; } else { filt = a[i].clone(); i += 1; }
    }
    let m = tmmaps::map::MapFile::load(std::path::Path::new(&path));
    let files = mapgeom::embedded::files(&m).expect("embedded files");
    let mut by_name: std::collections::BTreeMap<String, &Vec<u8>> = Default::default();
    for (k, v) in &files { by_name.insert(k.rsplit(['/', '\\']).next().unwrap_or(k).to_string(), v); }
    // placements per model name: (x, y, z)
    let mut places: std::collections::BTreeMap<String, Vec<[f32; 3]>> = Default::default();
    for it in &m.items { places.entry(it.model.clone()).or_default().push([it.pos[0], it.pos[1], it.pos[2]]); }
    let mut is_water_cache: std::collections::HashMap<String, bool> = Default::default();
    let mut n_models = 0usize; let mut n_water_models = 0usize; let mut n_water_geoms = 0usize;
    for (name, bytes) in &by_name {
        if !name.to_ascii_lowercase().ends_with(".item.gbx") { continue; }
        if !filt.is_empty() && !name.contains(filt.as_str()) { continue; }
        n_models += 1;
        let Ok(f) = mapgeom::static_item::file::parse_file(bytes) else { continue };
        let Some(so) = f.item.static_object() else { continue };
        let Some(s2) = so.solid2() else { continue };
        let mut lines: Vec<String> = Vec::new();
        for (gi, sg) in s2.shaded_geoms.iter().enumerate() {
            let link = lightmap::waterid::geom_material_link(s2, sg.material_index, &[]);
            let is_water = match is_water_cache.get(&link) { Some(v) => *v, None => { let v = lightmap::waterid::is_water_material(&mut store, &link) || link.rsplit('\\').next().map(|s| s.eq_ignore_ascii_case("water") || s.to_ascii_lowercase().contains("water")).unwrap_or(false); is_water_cache.insert(link.clone(), v); v } };
            if !is_water { continue; }
            let Some(vr) = s2.visuals.get(sg.visual_index as usize) else { continue };
            let Some(mapgeom::static_item::Node::Visual(vis)) = vr.inline.as_deref() else { continue };
            let (mut ylo, mut yhi, mut n) = (f32::MAX, f32::MIN, 0usize);
            if let Some(st) = vis.stream() {
                if let Some((_, mapgeom::static_item::vstream::Elem::Float3(pos))) = st.decls.iter().zip(st.elems.iter()).find(|(d, _)| d.name() == mapgeom::static_item::vstream::N_POSITION) {
                    for p in pos { ylo = ylo.min(p[1]); yhi = yhi.max(p[1]); n += 1; }
                }
            }
            let ntri = vis.index_buffer.as_ref().map(|ib| ib.indices.len() / 3).unwrap_or(0);
            let pl = places.get(name).cloned().unwrap_or_default();
            let (mut wlo, mut whi) = (f32::MAX, f32::MIN);
            for p in &pl { wlo = wlo.min(p[1] + ylo); whi = whi.max(p[1] + yhi); }
            lines.push(format!("    geom {gi} visual {} lod {} mat {link}: {ntri} tris, {n} verts, local y [{ylo:.3}, {yhi:.3}]; {} placements → world surface y [{wlo:.2}, {whi:.2}]", sg.visual_index, sg.lod_mask, pl.len()));
            n_water_geoms += 1;
        }
        if !lines.is_empty() {
            n_water_models += 1;
            println!("{name} ({} shaded geoms, {} placements):\n{}", s2.shaded_geoms.len(), places.get(name).map(|v| v.len()).unwrap_or(0), lines.join("\n"));
        }
    }
    println!("{n_models} embedded item models scanned; {n_water_models} carry a water geom ({n_water_geoms} geoms)");
}
