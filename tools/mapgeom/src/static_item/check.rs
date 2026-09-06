//! `mapgeom item-check [--packs …] FILE.Item.Gbx…`: offline structural
//! validation of static items — everything the game is known to trip over,
//! checked in milliseconds instead of a two-minute load:
//!   * every index < vertex count, every stream element sized to the count,
//!     tangent arrays sized to the count, no empty visual / geom
//!   * shaded geoms sorted by material, every material index in range
//!   * every material used by some geom, no duplicate (link, physics) slot
//!   * every material link resolvable as `<link>.Material.Gbx` in the packs
//!   * collision: every triangle index < vertex count, surface index in range
//! With `--facts` it also prints one line per visual / material (the
//! structural profile used to diff a crashing item against a loading one).

use crate::store::DataStore;

pub fn run(rest: &[String], open: &mut dyn FnMut() -> DataStore) -> Result<(), String> {
    let facts = rest.iter().any(|a| a == "--facts");
    let files: Vec<&String> = rest.iter().skip(1).filter(|a| !a.starts_with("--")).collect();
    if files.is_empty() {
        return Err("item-check [--facts] FILE.Item.Gbx…".into());
    }
    let mut store: Option<DataStore> = None;
    let mut bad = 0usize;
    for path in files {
        let data = std::fs::read(path).map_err(|e| format!("{path}: {e}"))?;
        let f = match super::parse_file(&data) {
            Ok(f) => f,
            Err(e) => {
                println!("{path}: FAIL does not parse: {e}");
                bad += 1;
                continue;
            }
        };
        let mut problems: Vec<String> = Vec::new();
        let Some(so) = f.item.static_object() else {
            println!("{path}: FAIL no static object");
            bad += 1;
            continue;
        };
        let Some(s2) = so.solid2() else {
            println!("{path}: FAIL no solid2");
            bad += 1;
            continue;
        };
        let nmat = s2.custom_materials.len();
        let mut used = vec![0usize; nmat];
        let mut prev_mat = -1i32;
        if s2.shaded_geoms.is_empty() {
            problems.push("no shaded geoms".into());
        }
        for (gi, g) in s2.shaded_geoms.iter().enumerate() {
            if g.material_index < 0 || g.material_index as usize >= nmat {
                problems.push(format!("geom {gi}: material index {} of {nmat}", g.material_index));
            } else {
                used[g.material_index as usize] += 1;
            }
            if g.material_index < prev_mat {
                problems.push(format!("geom {gi}: material index {} after {} (geoms not sorted by material)", g.material_index, prev_mat));
            }
            prev_mat = g.material_index;
            if g.visual_index < 0 || g.visual_index as usize >= s2.visuals.len() {
                problems.push(format!("geom {gi}: visual index {} of {}", g.visual_index, s2.visuals.len()));
            }
            if g.lod_mask != 1 {
                problems.push(format!("geom {gi}: lod mask {}", g.lod_mask));
            }
        }
        for (mi, n) in used.iter().enumerate() {
            if *n == 0 {
                problems.push(format!("material {mi} used by no geom"));
            }
        }
        let mut slots: Vec<(String, u8)> = Vec::new();
        for (mi, m) in s2.custom_materials.iter().enumerate() {
            let Some(inst) = m.inst() else {
                problems.push(format!("material {mi}: not a CPlugMaterialUserInst"));
                continue;
            };
            let link = inst.link().unwrap_or("").to_string();
            let key = (link.clone(), inst.physics());
            if slots.contains(&key) {
                problems.push(format!("material {mi}: duplicate slot {link} ({})", inst.physics()));
            }
            slots.push(key.clone());
            if link.is_empty() {
                problems.push(format!("material {mi}: empty link"));
            } else {
                let st = store.get_or_insert_with(|| open());
                let file = format!("{link}.Material.Gbx");
                if st.resolve(&file).is_none() {
                    problems.push(format!("material {mi}: link {link} resolves to no .Material.Gbx in the packs"));
                }
            }
            if facts {
                println!("{path}: material {mi} {link} phys {} used by {} geoms", inst.physics(), used.get(mi).copied().unwrap_or(0));
            }
        }
        for (vi, vr) in s2.visuals.iter().enumerate() {
            let Some(super::Node::Visual(v)) = vr.inline.as_deref() else {
                problems.push(format!("visual {vi}: not an inline visual"));
                continue;
            };
            let Some(m) = v.main.as_ref() else {
                problems.push(format!("visual {vi}: no chunk 0x0900600F"));
                continue;
            };
            let count = m.count.max(0) as usize;
            if count == 0 {
                problems.push(format!("visual {vi}: 0 vertices"));
            }
            let per = (((!(m.flags() >> 17)) & 8) | 4) as usize;
            let mut decl_desc = String::new();
            match v.stream() {
                Some(s) => {
                    if s.count.max(0) as usize != count {
                        problems.push(format!("visual {vi}: stream count {} != visual count {count}", s.count));
                    }
                    let compress = s.compress_local3d.unwrap_or(false);
                    for (d, e) in s.decls.iter().zip(s.elems.iter()) {
                        if e.len() != count {
                            problems.push(format!("visual {vi}: element name{} has {} entries for {count} vertices", d.name(), e.len()));
                        }
                        decl_desc.push_str(&format!(" n{}t{}", d.name(), d.stored_type(compress)));
                        if facts && rest.iter().any(|a| a == "--decls") {
                            decl_desc.push_str(&format!("[{:x}/{:x}/{:?}]", d.flags1, d.flags2, d.extra));
                        }
                    }
                    if s.decls.len() != s.elems.len() {
                        problems.push(format!("visual {vi}: {} decls but {} element arrays", s.decls.len(), s.elems.len()));
                    }
                }
                None => problems.push(format!("visual {vi}: no inline vertex stream")),
            }
            if let Some((a, b)) = v.tangents.as_ref() {
                for t in [a, b] {
                    if !t.is_empty() && t.len() != count * per {
                        problems.push(format!("visual {vi}: tangent array {} bytes != {count} x {per}", t.len()));
                    }
                }
            }
            let ntri = match v.index_buffer.as_ref() {
                Some(ib) => {
                    if ib.indices.is_empty() {
                        problems.push(format!("visual {vi}: empty index buffer"));
                    }
                    if ib.indices.len() % 3 != 0 {
                        problems.push(format!("visual {vi}: {} indices is not a triangle list", ib.indices.len()));
                    }
                    if let Some(mx) = ib.indices.iter().max() {
                        if *mx as usize >= count {
                            problems.push(format!("visual {vi}: index {mx} past {count} vertices"));
                        }
                    }
                    let mut seen = vec![false; count];
                    for i in &ib.indices {
                        if let Some(s) = seen.get_mut(*i as usize) {
                            *s = true;
                        }
                    }
                    let unused = seen.iter().filter(|s| !**s).count();
                    if unused > 0 && facts {
                        println!("{path}: visual {vi}: {unused} of {count} vertices referenced by no triangle");
                    }
                    ib.indices.len() / 3
                }
                None => {
                    problems.push(format!("visual {vi}: no index buffer"));
                    0
                }
            };
            if facts {
                let mat = s2.shaded_geoms.iter().find(|g| g.visual_index as usize == vi).map(|g| g.material_index).unwrap_or(-1);
                println!("{path}: visual {vi} mat {mat} verts {count} tris {ntri} decls{decl_desc} cflags {:x} sflags {:x} u03 {} tangents {:?} bbox {:?}", m.chunk_flags, v.stream().map(|s| s.flags).unwrap_or(0), m.u03, v.tangents.as_ref().map(|(a, b)| (a.len(), b.len())), m.bounding_box);
            }
        }
        if let Some(sf) = so.surface() {
            if let super::surface::Surf::Mesh { vertices, triangles, .. } = &sf.surf {
                for (ti, t) in triangles.iter().enumerate() {
                    if t.indices.iter().any(|i| *i as usize >= vertices.len()) {
                        problems.push(format!("collision triangle {ti}: index past {} vertices", vertices.len()));
                        break;
                    }
                    if t.surface_index < 0 || t.surface_index as usize >= sf.material_ids.len().max(1) {
                        problems.push(format!("collision triangle {ti}: surface index {} of {}", t.surface_index, sf.material_ids.len()));
                        break;
                    }
                }
                if facts {
                    println!("{path}: collision {} vertices {} triangles physics {:?}", vertices.len(), triangles.len(), sf.material_ids);
                }
            }
        } else {
            problems.push("no collision surface".into());
        }
        if problems.is_empty() {
            println!("{path}: ok ({} visuals, {} materials)", s2.visuals.len(), nmat);
        } else {
            bad += 1;
            for p in &problems {
                println!("{path}: FAIL {p}");
            }
        }
    }
    if bad > 0 {
        return Err(format!("{bad} item(s) failed"));
    }
    Ok(())
}
