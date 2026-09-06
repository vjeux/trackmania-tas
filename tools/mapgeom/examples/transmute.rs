//! Single-transform mutations of a static item (bisect the bake grey-out).
//! Usage: transmute BASE OUT IDENT MODE [ARG]
//!   MODE=scale S        multiply positions by S
//!   MODE=uvpack         replace decl-11 with per-triangle planar pack
//!                       (vertex split, mimics bake::assign_lightmap_uvs)
//!   MODE=reorder        order materials by triangle count, most first
//!                       (mimics the bake's material order)
use mapgeom::static_item::vstream::Elem;

fn sub(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}
fn cross(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]]
}
fn norm(v: [f32; 3]) -> [f32; 3] {
    let l = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
    if l < 1e-12 {
        [0.0, 1.0, 0.0]
    } else {
        [v[0] / l, v[1] / l, v[2] / l]
    }
}

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let data = std::fs::read(&a[1]).unwrap();
    let mut f = mapgeom::static_item::file::parse_file(&data).unwrap();
    let mode = a[4].as_str();
    let mc = f.item.model_mut().unwrap();
    if let Some(n) = mc.entity_model.inline.as_deref_mut() {
        if let mapgeom::static_item::Node::EntityModel(em) = n {
            if let Some(sn) = em.static_object.inline.as_deref_mut() {
                if let mapgeom::static_item::Node::StaticObject(so) = sn {
                    if let Some(snn) = so.mesh.inline.as_deref_mut() {
                        if let mapgeom::static_item::Node::Solid2(s) = snn {
                            match mode {
                                "keep" => {
                                    // keep only the shaded-geom indices listed
                                    // in ARG (comma "4,5"): drop other visuals
                                    // + geoms, keep materials. No positions or
                                    // indices of kept nodes change.
                                    let keep: Vec<usize> = a[5].split(',').map(|x| x.parse().unwrap()).collect();
                                    // visual positions kept
                                    let mut keepvis = std::collections::BTreeSet::new();
                                    let mut newgeoms = Vec::new();
                                    for (gi, geom) in s.shaded_geoms.iter().enumerate() {
                                        if keep.contains(&gi) {
                                            keepvis.insert(geom.visual_index.max(0) as usize);
                                            newgeoms.push(geom.clone());
                                        }
                                    }
                                    let mut newvis = Vec::new();
                                    for (pi, vr) in s.visuals.iter().enumerate() {
                                        if keepvis.contains(&pi) {
                                            newvis.push(vr.clone());
                                        }
                                    }
                                    // remap visual_index to new positions
                                    let mut posmap = std::collections::BTreeMap::new();
                                    let mut np = 0;
                                    for (pi, vr) in s.visuals.iter().enumerate() {
                                        if keepvis.contains(&pi) {
                                            posmap.insert(pi as i32, np);
                                            np += 1;
                                            let _ = vr;
                                        }
                                    }
                                    for g in newgeoms.iter_mut() {
                                        g.visual_index = *posmap.get(&g.visual_index).unwrap();
                                    }
                                    println!("kept {} geoms / {} visuals", newgeoms.len(), newvis.len());
                                    s.shaded_geoms = newgeoms;
                                    s.visuals = newvis;
                                }
                                "lift" => {
                                    // add DY to Y of every position in visuals
                                    // whose material link contains SUBSTR.
                                    let sub = &a[5];
                                    let dy: f32 = a[6].parse().unwrap();
                                    // visual position -> material link
                                    let mut vlink: Vec<Option<String>> = vec![None; s.visuals.len()];
                                    for geom in s.shaded_geoms.iter() {
                                        let pi = geom.visual_index.max(0) as usize;
                                        let mi = geom.material_index.max(0) as usize;
                                        if pi < vlink.len() {
                                            vlink[pi] = s.custom_materials.get(mi).and_then(|m| m.inst()).map(|i| i.link().unwrap_or("").to_string());
                                        }
                                    }
                                    let mut n = 0;
                                    for (pi, vr) in s.visuals.iter_mut().enumerate() {
                                        if !vlink.get(pi).and_then(|o| o.as_ref()).map(|l| l.contains(sub.as_str())).unwrap_or(false) {
                                            continue;
                                        }
                                        if let Some(mapgeom::static_item::Node::Visual(vis)) = vr.inline.as_deref_mut() {
                                            if let Some(main) = vis.main.as_mut() {
                                                for r in main.vertex_streams.iter_mut() {
                                                    if let Some(mapgeom::static_item::Node::VertexStream(st)) = r.inline.as_deref_mut() {
                                                        for (d, e) in st.decls.iter().zip(st.elems.iter_mut()) {
                                                            if let Elem::Float3(p) = e {
                                                                if d.name() == 0 {
                                                                    for q in p.iter_mut() {
                                                                        q[1] += dy;
                                                                    }
                                                                    n += p.len();
                                                                }
                                                            }
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                    }
                                    println!("lifted {n} verts by {dy}");
                                    // refresh the lifted visual's bbox (center +
                                    // half-extents), else the game culls it at
                                    // its stale box (lift test 2026-09-05).
                                    for (pi, vr) in s.visuals.iter_mut().enumerate() {
                                        if !vlink.get(pi).and_then(|o| o.as_ref()).map(|l| l.contains(sub.as_str())).unwrap_or(false) {
                                            continue;
                                        }
                                        if let Some(mapgeom::static_item::Node::Visual(vis)) = vr.inline.as_deref_mut() {
                                            if let Some(main) = vis.main.as_mut() {
                                                let mut lo = [f32::MAX; 3];
                                                let mut hi = [f32::MIN; 3];
                                                for r in main.vertex_streams.iter() {
                                                    if let Some(mapgeom::static_item::Node::VertexStream(st)) = r.inline.as_deref() {
                                                        for (d, e) in st.decls.iter().zip(st.elems.iter()) {
                                                            if let Elem::Float3(p) = e {
                                                                if d.name() == 0 {
                                                                    for q in p {
                                                                        for k in 0..3 {
                                                                            lo[k] = lo[k].min(q[k]);
                                                                            hi[k] = hi[k].max(q[k]);
                                                                        }
                                                                    }
                                                                }
                                                            }
                                                        }
                                                    }
                                                }
                                                main.bounding_box = [(lo[0] + hi[0]) / 2.0, (lo[1] + hi[1]) / 2.0, (lo[2] + hi[2]) / 2.0, (hi[0] - lo[0]) / 2.0, (hi[1] - lo[1]) / 2.0, (hi[2] - lo[2]) / 2.0];
                                            }
                                        }
                                    }
                                }
                                "noop" => {
                                    println!("ident only");
                                }
                                "scale" => {
                                    let k: f32 = a[5].parse().unwrap();
                                    for vr in s.visuals.iter_mut() {
                                        if let Some(mapgeom::static_item::Node::Visual(vis)) = vr.inline.as_deref_mut() {
                                            if let Some(main) = vis.main.as_mut() {
                                                for r in main.vertex_streams.iter_mut() {
                                                    if let Some(mapgeom::static_item::Node::VertexStream(st)) = r.inline.as_deref_mut() {
                                                        for (d, e) in st.decls.iter().zip(st.elems.iter_mut()) {
                                                            if let Elem::Float3(p) = e {
                                                                if d.name() == 0 {
                                                                    for q in p.iter_mut() {
                                                                        *q = [q[0] * k, q[1] * k, q[2] * k];
                                                                    }
                                                                }
                                                            }
                                                        }
                                                    }
                                                }
                                                // recompute bbox from the mutated stream arrays
                                                let mut lo = [f32::MAX; 3];
                                                let mut hi = [f32::MIN; 3];
                                                for r in main.vertex_streams.iter() {
                                                    if let Some(mapgeom::static_item::Node::VertexStream(st)) = r.inline.as_deref() {
                                                        for (d, e) in st.decls.iter().zip(st.elems.iter()) {
                                                            if let Elem::Float3(p) = e {
                                                                if d.name() == 0 {
                                                                    for q in p {
                                                                        for k in 0..3 {
                                                                            lo[k] = lo[k].min(q[k]);
                                                                            hi[k] = hi[k].max(q[k]);
                                                                        }
                                                                    }
                                                                }
                                                            }
                                                        }
                                                    }
                                                }
                                                // bbox is CENTER + HALF-EXTENTS (not min/max)
                                                main.bounding_box = [(lo[0] + hi[0]) / 2.0, (lo[1] + hi[1]) / 2.0, (lo[2] + hi[2]) / 2.0, (hi[0] - lo[0]) / 2.0, (hi[1] - lo[1]) / 2.0, (hi[2] - lo[2]) / 2.0];
                                            }
                                        }
                                    }
                                    println!("scaled positions by {k}");
                                }
                                "uvpack" => {
                                    for vr in s.visuals.iter_mut() {
                                        if let Some(mapgeom::static_item::Node::Visual(vis)) = vr.inline.as_deref_mut() {
                                            if let Some(main) = vis.main.as_mut() {
                                                for r in main.vertex_streams.iter_mut() {
                                                    if let Some(mapgeom::static_item::Node::VertexStream(st)) = r.inline.as_deref_mut() {
                                                        // gather arrays
                                                        let mut pi = None;
                                                        let mut uvi = None;
                                                        for (di, (d, e)) in st.decls.iter().zip(st.elems.iter()).enumerate() {
                                                            if d.name() == 0 {
                                                                if matches!(e, Elem::Float3(_)) {
                                                                    pi = Some(di);
                                                                }
                                                            }
                                                            if d.name() == 11 {
                                                                if matches!(e, Elem::Float2(_)) {
                                                                    uvi = Some(di);
                                                                }
                                                            }
                                                        }
                                                        let (Some(pdi), Some(udi)) = (pi, uvi) else { continue };
                                                        let pos = match &st.elems[pdi] {
                                                            Elem::Float3(p) => p.clone(),
                                                            _ => continue,
                                                        };
                                                        let idx = vis.index_buffer.as_ref().map(|b| b.indices.clone()).unwrap_or_default();
                                                        if idx.is_empty() {
                                                            continue;
                                                        }
                                                        let ntri = idx.len() / 3;
                                                        let grid = (ntri as f64).sqrt().ceil() as usize;
                                                        // per-tri planar uv1; split shared verts
                                                        // skip visuals with exotic element types (Raw/Float4):
                                                        // per-corner copy only covers the standard three.
                                                        if st.elems.iter().any(|e| !matches!(e, Elem::Float3(_) | Elem::Float2(_) | Elem::Word(_))) {
                                                            continue;
                                                        }
                                                        let mut new_elems: Vec<Elem> = st.elems.iter().map(|e| match e {
                                                            Elem::Float3(_) => Elem::Float3(Vec::new()),
                                                            Elem::Float2(_) => Elem::Float2(Vec::new()),
                                                            _ => Elem::Word(Vec::new()),
                                                        }).collect();
                                                        let mut new_idx = Vec::new();
                                                        for (ti, t) in idx.chunks(3).enumerate() {
                                                            if t.len() < 3 {
                                                                continue;
                                                            }
                                                            let p = [pos[t[0] as usize], pos[t[1] as usize], pos[t[2] as usize]];
                                                            let n = norm(cross(sub(p[1], p[0]), sub(p[2], p[0])));
                                                            let ax = n[0].abs();
                                                            let ay = n[1].abs();
                                                            let pr = |q: [f32; 3]| -> [f32; 2] {
                                                                if ay >= ax && ay >= n[2].abs() {
                                                                    [q[0], q[2]]
                                                                } else if ax >= n[2].abs() {
                                                                    [q[2], q[1]]
                                                                } else {
                                                                    [q[0], q[1]]
                                                                }
                                                            };
                                                            let q = [pr(p[0]), pr(p[1]), pr(p[2])];
                                                            let (mut lo, mut hi) = (q[0], q[0]);
                                                            for v in &q[1..] {
                                                                for k in 0..2 {
                                                                    lo[k] = lo[k].min(v[k]);
                                                                    hi[k] = hi[k].max(v[k]);
                                                                }
                                                            }
                                                            let span = [(hi[0] - lo[0]).max(1e-6), (hi[1] - lo[1]).max(1e-6)];
                                                            let (col, row) = (ti % grid, ti / grid);
                                                            for k in 0..3 {
                                                                let viold = t[k] as usize;
                                                                // copy every array's corner
                                                                for (di, e) in st.elems.iter().enumerate() {
                                                                    match e {
                                                                        Elem::Float3(pp) => {
                                                                            if let Elem::Float3(np) = &mut new_elems[di] {
                                                                                np.push(pp[viold]);
                                                                            }
                                                                        }
                                                                        Elem::Float2(uu) => {
                                                                            if let Elem::Float2(nu) = &mut new_elems[di] {
                                                                                if di == udi {
                                                                                    let u = (q[k][0] - lo[0]) / span[0];
                                                                                    let v = (q[k][1] - lo[1]) / span[1];
                                                                                    nu.push([(col as f32 + 0.05 + 0.9 * u) / grid as f32, (row as f32 + 0.05 + 0.9 * v) / grid as f32]);
                                                                                } else {
                                                                                    nu.push(uu[viold]);
                                                                                }
                                                                            }
                                                                        }
                                                                        Elem::Word(ww) => {
                                                                            if let Elem::Word(nw) = &mut new_elems[di] {
                                                                                nw.push(ww[viold]);
                                                                            }
                                                                        }
                                                                        _ => {}
                                                                    }
                                                                }
                                                                new_idx.push((new_elems[0].len() as u32) - 1);
                                                            }
                                                        }
                                                        // fix counts
                                                        let nv = match &new_elems[pdi] {
                                                            Elem::Float3(p) => p.len() as i32,
                                                            _ => 0,
                                                        };
                                                        st.elems = new_elems;
                                                        st.count = nv;
                                                        if let Some(ib) = vis.index_buffer.as_mut() {
                                                            ib.indices = new_idx;
                                                        }
                                                        main.count = nv;
                                                    }
                                                }
                                            }
                                        }
                                    }
                                    println!("uv-packed visuals");
                                }
                                "reorder" => {
                                    // tri counts per material
                                    let mut counts = vec![0usize; s.custom_materials.len()];
                                    for g in &s.shaded_geoms {
                                        let mi = g.material_index.max(0) as usize;
                                        let vi = g.visual_index.max(0) as usize;
                                        let ntri = s.visuals.get(vi).and_then(|v| v.inline.as_deref()).and_then(|n| match n {
                                            mapgeom::static_item::Node::Visual(v) => v.index_buffer.as_ref().map(|b| b.indices.len() / 3),
                                            _ => None,
                                        }).unwrap_or(0);
                                        if mi < counts.len() {
                                            counts[mi] += ntri;
                                        }
                                    }
                                    let mut order: Vec<usize> = (0..s.custom_materials.len()).collect();
                                    order.sort_by_key(|&i| std::cmp::Reverse(counts[i]));
                                    // permute materials; remap geoms
                                    let mut new_mats = Vec::new();
                                    let mut remap = vec![0i32; s.custom_materials.len()];
                                    for (ni, &oi) in order.iter().enumerate() {
                                        remap[oi] = ni as i32;
                                        new_mats.push(s.custom_materials[oi].clone());
                                    }
                                    s.custom_materials = new_mats;
                                    for g in s.shaded_geoms.iter_mut() {
                                        g.material_index = remap[g.material_index.max(0) as usize];
                                    }
                                    println!("reordered materials by {:?} tris", counts);
                                }
                                _ => panic!("unknown mode {mode}"),
                            }
                        }
                    }
                }
            }
        }
    }
    // re-ident
    let ident = &a[3];
    let mut collection = 26u32;
    // TRANSMUTE_COLLECTION overrides the ident collection (BlueBay maps
    // need 28 everywhere: placements, manifest, and file idents).
    if let Ok(c) = std::env::var("TRANSMUTE_COLLECTION") {
        collection = c.parse().expect("TRANSMUTE_COLLECTION u32");
    }
    for c in f.item.chunks.iter_mut() {
        if let mapgeom::static_item::item::ItemChunk::Ident { path, author, collection: coll } = c {
            if std::env::var("TRANSMUTE_COLLECTION").is_err() {
                if let mapgeom::static_item::Id::Raw(n) = coll {
                    collection = *n;
                }
            } else if let mapgeom::static_item::Id::Raw(n) = coll {
                *n = collection;
            }
            // TRANSMUTE_KEEP_NAME: foreign item -- keep path+author, and
            // keep them in the header chunk too.
            if std::env::var("TRANSMUTE_KEEP_NAME").is_err() {
                *path = mapgeom::static_item::Id::Str(ident.clone());
                *author = mapgeom::static_item::Id::Str(ident.clone());
            }
        }
    }
    let (hident, hauthor) = if std::env::var("TRANSMUTE_KEEP_NAME").is_ok() {
        // read back the (kept) body path+author for the header chunk
        let mut hp = ident.clone();
        let mut ha = ident.clone();
        for c in f.item.chunks.iter() {
            if let mapgeom::static_item::item::ItemChunk::Ident { path, author, .. } = c {
                if let mapgeom::static_item::Id::Str(s) = path {
                    hp = s.clone();
                }
                if let mapgeom::static_item::Id::Str(s) = author {
                    ha = s.clone();
                }
            }
        }
        (hp, ha)
    } else {
        (ident.clone(), ident.clone())
    };
    f.header_chunks = mapgeom::static_item::build::header_chunks(&mapgeom::static_item::build::BuildOpts { ident: hident, author: hauthor, scale: 1.0, collection, editors: false });
    std::fs::write(&a[2], mapgeom::static_item::file::write_file(&f)).unwrap();
    println!("wrote {}", &a[2]);
}
