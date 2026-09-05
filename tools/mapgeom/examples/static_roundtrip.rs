//! Parse every `.Item.Gbx` given (files or directories) with the static-item
//! model, write it back, and demand identical bytes. Prints a per-file line
//! and a summary; exit code 1 when anything fails.
use mapgeom::static_item::{parse_file, write_file};
use std::path::Path;

fn collect(p: &Path, out: &mut Vec<std::path::PathBuf>) {
    if p.is_dir() {
        let mut v: Vec<_> = std::fs::read_dir(p).unwrap().flatten().map(|e| e.path()).collect();
        v.sort();
        for e in v {
            collect(&e, out);
        }
    } else if p.to_string_lossy().ends_with(".Item.Gbx") {
        out.push(p.to_path_buf());
    }
}

/// `--rebuild`: re-BUILD the item from its own static object through
/// `build::static_item_from_item` (ident and author taken from the file) and
/// compare that with the original instead.
fn rebuild_bytes(item: &mapgeom::static_item::StaticItemFile, orig: &[u8]) -> Vec<u8> {
    use mapgeom::static_item::item::ItemChunk;
    use mapgeom::static_item::Id;
    let (path, collection, author) = item
        .item
        .chunks
        .iter()
        .find_map(|c| match c {
            ItemChunk::Ident { path: Id::Str(p), collection, author: Id::Str(a) } => Some((p.clone(), if let Id::Raw(c) = collection { *c } else { 26 }, a.clone())),
            _ => None,
        })
        .unwrap_or_default();
    match mapgeom::static_item::build::static_item_from_item_report(orig, &path, &author, 1.0, collection) {
        Ok((b, _)) => b,
        Err(e) => {
            println!("  rebuild failed: {e}");
            Vec::new()
        }
    }
}

/// Same visuals (stream bytes + indices), same material list, same collision
/// triangles (indices, physics, gameplay) -- ignoring the u16 id list order.
fn equivalent(a: &mapgeom::static_item::StaticItemFile, b: &mapgeom::static_item::StaticItemFile) -> bool {
    use mapgeom::static_item::{surface::Surf, Node};
    let (sa, sb) = (a.item.static_object().unwrap(), b.item.static_object().unwrap());
    let (a2, b2) = (sa.solid2().unwrap(), sb.solid2().unwrap());
    let vis = |s: &mapgeom::static_item::solid2::CPlugSolid2Model| -> Vec<(Vec<mapgeom::static_item::vstream::Elem>, Vec<u32>, u32)> {
        s.visuals
            .iter()
            .filter_map(|v| match v.inline.as_deref() {
                Some(Node::Visual(x)) => Some((x.stream().map(|st| st.elems.clone()).unwrap_or_default(), x.index_buffer.as_ref().map(|i| i.indices.clone()).unwrap_or_default(), x.main.as_ref().unwrap().chunk_flags)),
                _ => None,
            })
            .collect()
    };
    let mats = |s: &mapgeom::static_item::solid2::CPlugSolid2Model| -> Vec<(String, u8)> { s.custom_materials.iter().filter_map(|m| m.inst()).map(|i| (i.link().unwrap_or("").to_string(), i.physics())).collect() };
    let tris = |so: &mapgeom::static_item::item::CPlugStaticObjectModel| -> Vec<([f32; 3], [f32; 3], [f32; 3], u8, u8)> {
        let sf = so.surface().unwrap();
        match &sf.surf {
            Surf::Mesh { vertices, triangles, .. } => triangles.iter().map(|t| (vertices[t.indices[0] as usize], vertices[t.indices[1] as usize], vertices[t.indices[2] as usize], t.material_id, t.u03)).collect(),
            _ => Vec::new(),
        }
    };
    vis(a2) == vis(b2) && mats(a2) == mats(b2) && tris(sa) == tris(sb) && a.num_nodes == b.num_nodes
}

fn main() {
    let mut files = Vec::new();
    let mut rebuild = false;
    for a in std::env::args().skip(1) {
        if a == "--rebuild" {
            rebuild = true;
            continue;
        }
        collect(Path::new(&a), &mut files);
    }
    let (mut pass, mut fail) = (0, 0);
    for f in &files {
        let bytes = std::fs::read(f).unwrap();
        match parse_file(&bytes) {
            Err(e) => {
                fail += 1;
                println!("FAIL parse {}: {e}", f.display());
            }
            Ok(item) => {
                let out = if rebuild { rebuild_bytes(&item, &bytes) } else { write_file(&item) };
                if out == bytes {
                    pass += 1;
                    let so = item.item.static_object();
                    let nvis = so.and_then(|s| s.solid2()).map(|s| s.visuals.len()).unwrap_or(0);
                    let ntri = so.and_then(|s| s.surface()).map(|s| match &s.surf {
                        mapgeom::static_item::surface::Surf::Mesh { triangles, .. } => triangles.len(),
                        _ => 0,
                    });
                    println!("ok   {} ({} bytes, {} nodes, {} visuals, {:?} surf tris)", f.display(), bytes.len(), item.num_nodes, nvis, ntri);
                } else {
                    fail += 1;
                    let first = out.iter().zip(bytes.iter()).position(|(a, b)| a != b).unwrap_or(out.len().min(bytes.len()));
                    println!("FAIL bytes {}: {} vs {} bytes, first difference at 0x{first:x}", f.display(), out.len(), bytes.len());
                    if rebuild {
                        if let Ok(rb) = parse_file(&out) {
                            let sum = |it: &mapgeom::static_item::StaticItemFile| {
                                let so = it.item.static_object().unwrap();
                                let s2 = so.solid2().unwrap();
                                let sf = so.surface().unwrap();
                                let mats: Vec<String> = s2.custom_materials.iter().filter_map(|m| m.inst()).map(|i| format!("{}:{}", i.link().unwrap_or("?").rsplit('\\').next().unwrap_or("?"), i.physics())).collect();
                                let geoms: Vec<(i32, i32)> = s2.shaded_geoms.iter().map(|g| (g.visual_index, g.material_index)).collect();
                                let vis: Vec<u32> = s2.visuals.iter().filter_map(|v| match v.inline.as_deref() { Some(mapgeom::static_item::Node::Visual(x)) => Some(x.main.as_ref().unwrap().chunk_flags), _ => None }).collect();
                                format!("nodes {} mats {:?} geoms {:?} flags {:?} surf ids {:?} header2000 {:?}", it.num_nodes, mats, geoms, vis, sf.material_ids, it.header_chunks.iter().map(|h| (h.id, h.payload.len())).collect::<Vec<_>>())
                            };
                            let same_geom = equivalent(&item, &rb);
                            if same_geom {
                                println!("    (geometry, materials and collision are EQUIVALENT: only id-list dedupe / flag bytes differ)");
                            } else {
                                println!("    orig:    {}", sum(&item));
                                println!("    rebuilt: {}", sum(&rb));
                            }
                        }
                    }
                    if let Some(p) = std::env::var_os("STATIC_RT_DUMP") {
                        std::fs::write(p, &out).unwrap();
                    }
                }
            }
        }
    }
    println!("{pass} identical, {fail} failed, {} files", files.len());
    if fail > 0 {
        std::process::exit(1);
    }
}
