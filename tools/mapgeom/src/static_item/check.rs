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
                        if facts && rest.iter().any(|a| a == "--values") {
                            decl_desc.push_str(&elem_summary(e));
                            if let (5 | 18 | 20, super::vstream::Elem::Word(w)) = (d.name(), e) {
                                let mut lens: Vec<f32> = w.iter().map(|x| { let n = super::build::dec3n_unpack(*x); (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt() }).collect();
                                lens.sort_by(|a, b| a.partial_cmp(b).unwrap());
                                let first: Vec<String> = w.iter().take(3).map(|x| { let n = super::build::dec3n_unpack(*x); format!("[{:.2},{:.2},{:.2}]", n[0], n[1], n[2]) }).collect();
                                decl_desc.push_str(&format!("<len {:.2}..{:.2} first {}>", lens.first().copied().unwrap_or(0.0), lens.last().copied().unwrap_or(0.0), first.join("")));
                            }
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
            if facts && rest.iter().any(|a| a == "--uvhist") {
                if let Some(s) = v.stream() {
                    let pos = match s.decls.iter().zip(s.elems.iter()).find(|(d, _)| d.name() == 0).map(|(_, e)| e) {
                        Some(super::vstream::Elem::Float3(p)) => p.clone(),
                        _ => Vec::new(),
                    };
                    let uv = match s.decls.iter().zip(s.elems.iter()).find(|(d, _)| d.name() == 10).map(|(_, e)| e) {
                        Some(super::vstream::Elem::Float2(p)) => p.clone(),
                        _ => Vec::new(),
                    };
                    if let (Some(ib), false, false) = (v.index_buffer.as_ref(), pos.is_empty(), uv.is_empty()) {
                        let nrm: Vec<[f32; 3]> = match s.decls.iter().zip(s.elems.iter()).find(|(d, _)| d.name() == 5).map(|(_, e)| e) {
                            Some(super::vstream::Elem::Word(w)) => w.iter().map(|x| super::build::dec3n_unpack(*x)).collect(),
                            _ => Vec::new(),
                        };
                        let (mut agree, mut disagree) = (0usize, 0usize);
                        if nrm.len() == pos.len() {
                            for t in ib.indices.chunks(3) {
                                if t.len() < 3 { continue; }
                                let [a, b, c] = [pos[t[0] as usize], pos[t[1] as usize], pos[t[2] as usize]];
                                let e1 = [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
                                let e2 = [c[0] - a[0], c[1] - a[1], c[2] - a[2]];
                                let n = [e1[1] * e2[2] - e1[2] * e2[1], e1[2] * e2[0] - e1[0] * e2[2], e1[0] * e2[1] - e1[1] * e2[0]];
                                let vn = [0, 1, 2].map(|k| nrm[t[0] as usize][k] + nrm[t[1] as usize][k] + nrm[t[2] as usize][k]);
                                if n[0] * vn[0] + n[1] * vn[1] + n[2] * vn[2] >= 0.0 { agree += 1 } else { disagree += 1 }
                            }
                        }
                        println!("{path}: visual {vi} winding vs vertex normals: {agree} agree, {disagree} disagree");
                        let tan = |name: u32| -> Vec<[f32; 3]> { match s.decls.iter().zip(s.elems.iter()).find(|(d, _)| d.name() == name).map(|(_, e)| e) { Some(super::vstream::Elem::Word(w)) => w.iter().map(|x| super::build::dec3n_unpack(*x)).collect(), _ => Vec::new() } };
                        let (tu, tv) = (tan(18), tan(20));
                        if tu.len() == nrm.len() && tv.len() == nrm.len() {
                            let (mut pos_h, mut neg_h, mut perp) = (0usize, 0usize, 0usize);
                            for i in 0..nrm.len() {
                                let c = [tu[i][1] * tv[i][2] - tu[i][2] * tv[i][1], tu[i][2] * tv[i][0] - tu[i][0] * tv[i][2], tu[i][0] * tv[i][1] - tu[i][1] * tv[i][0]];
                                let d = c[0] * nrm[i][0] + c[1] * nrm[i][1] + c[2] * nrm[i][2];
                                let un = tu[i][0] * nrm[i][0] + tu[i][1] * nrm[i][1] + tu[i][2] * nrm[i][2];
                                if un.abs() > 0.3 { perp += 1 }
                                if d > 0.0 { pos_h += 1 } else { neg_h += 1 }
                            }
                            println!("{path}: visual {vi} tangent frame: cross(tu,tv).n > 0 for {pos_h}, < 0 for {neg_h}; tu not perpendicular to n: {perp}");
                        }
                        // (uv cell 1/20, dominant normal axis) -> area
                        let mut cells: std::collections::BTreeMap<(i32, i32, &'static str), f32> = Default::default();
                        for t in ib.indices.chunks(3) {
                            if t.len() < 3 {
                                continue;
                            }
                            let [a, b, c] = [pos[t[0] as usize], pos[t[1] as usize], pos[t[2] as usize]];
                            let e1 = [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
                            let e2 = [c[0] - a[0], c[1] - a[1], c[2] - a[2]];
                            let n = [e1[1] * e2[2] - e1[2] * e2[1], e1[2] * e2[0] - e1[0] * e2[2], e1[0] * e2[1] - e1[1] * e2[0]];
                            let area = 0.5 * (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt();
                            let ax = if n[0].abs() >= n[1].abs() && n[0].abs() >= n[2].abs() {
                                if n[0] > 0.0 { "+x" } else { "-x" }
                            } else if n[1].abs() >= n[2].abs() {
                                if n[1] > 0.0 { "+y" } else { "-y" }
                            } else if n[2] > 0.0 { "+z" } else { "-z" };
                            let cu = (uv[t[0] as usize][0] + uv[t[1] as usize][0] + uv[t[2] as usize][0]) / 3.0;
                            let cv = (uv[t[0] as usize][1] + uv[t[1] as usize][1] + uv[t[2] as usize][1]) / 3.0;
                            *cells.entry(((cu * 20.0).floor() as i32, (cv * 20.0).floor() as i32, ax)).or_default() += area;
                        }
                        let mut per_axis: std::collections::BTreeMap<&str, f32> = Default::default();
                        for ((_, _, ax), a) in cells.iter() {
                            *per_axis.entry(ax).or_default() += a;
                        }
                        println!("{path}: visual {vi} area by normal axis {per_axis:?}");
                        let mut rows: Vec<_> = cells.into_iter().collect();
                        rows.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
                        for ((cu, cv, ax), area) in rows.iter().take(12) {
                            println!("{path}: visual {vi} uv0 cell u{:.2} v{:.2} {ax} area {area:.3}", *cu as f32 / 20.0, *cv as f32 / 20.0);
                        }
                    }
                }
            }
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

/// One-line value summary of a vertex element: per-component min..max for
/// float elements, distinct count and first values for one-word ones.
fn elem_summary(e: &super::vstream::Elem) -> String {
    use super::vstream::Elem;
    fn range<const N: usize>(v: &[[f32; N]]) -> String {
        let mut lo = [f32::INFINITY; N];
        let mut hi = [f32::NEG_INFINITY; N];
        for x in v {
            for i in 0..N {
                lo[i] = lo[i].min(x[i]);
                hi[i] = hi[i].max(x[i]);
            }
        }
        (0..N).map(|i| format!("{:.3}..{:.3}", lo[i], hi[i])).collect::<Vec<_>>().join(",")
    }
    match e {
        Elem::Float2(v) => format!("{{{}}}", range(v)),
        Elem::Float3(v) => format!("{{{}}}", range(v)),
        Elem::Float4(v) => format!("{{{}}}", range(v)),
        Elem::Word(v) => {
            let mut d: Vec<u32> = v.clone();
            d.sort_unstable();
            d.dedup();
            let first: Vec<String> = d.iter().take(6).map(|x| format!("{x:08x}")).collect();
            format!("{{{} distinct: {}}}", d.len(), first.join(" "))
        }
        Elem::Raw { size, bytes } => format!("{{raw {size}x{}}}", bytes.len() / size.max(&1)),
    }
}
