//! `re7_treebox --pak PAK:KEY TREE.VegetTreeModel.Gbx…` — the candidate local boxes of a VegetTreeModel: the fold of every
//! visual's stored bounding box (per LOD level and over all), and the fold of the vertex positions (RE 7).
fn main() {
    let a: Vec<String> = std::env::args().collect();
    let (pp, key) = a[2].split_once(':').expect("PAK:KEY");
    let mut store = mapgeom::store::DataStore::empty();
    store.add_pak(pp, key).expect("pak");
    for p in &a[3..] {
        let m = match mapgeom::veget::parse_tree_model(&mut store, p) { Ok(m) => m, Err(e) => { println!("{p}: {e}"); continue } };
        println!("{p}: lightmap_record_box = {:?}", m.lightmap_record_box().map(|(c, h)| (c.map(|v| format!("{v} ({:#010x})", v.to_bits())), h.map(|v| format!("{v} ({:#010x})", v.to_bits())))));
        for (li, lod) in m.lods.iter().enumerate() {
            let mut mn = [f32::MAX; 3];
            let mut mx = [f32::MIN; 3];
            let mut vmn = [f32::MAX; 3];
            let mut vmx = [f32::MIN; 3];
            let mut nv = 0usize;
            for e in lod {
                if let Some(main) = &e.visual.main {
                    // the visual's box is stored as (centre, half extents)
                    let b = main.bounding_box;
                    let mat = m.materials.get(e.material as usize);
                    println!("   visual {} material {} {:?} leaf {:?} box centre {:?} half {:?} ({} verts)", e.node_index, e.material, mat.map(|x| x.name.as_str()), mat.map(|x| x.leaf), &b[..3], &b[3..], main.count);
                    for k in 0..3 { mn[k] = mn[k].min(b[k] - b[3 + k]); mx[k] = mx[k].max(b[k] + b[3 + k]); }
                    for s in &main.vertex_streams {
                        if let Some(mapgeom::static_item::Node::VertexStream(vs)) = s.inline.as_deref() {
                            for (d, el) in vs.decls.iter().zip(vs.elems.iter()) {
                                if d.name() != mapgeom::static_item::vstream::N_POSITION { continue; }
                                if let mapgeom::static_item::vstream::Elem::Float3(v) = el {
                                    for pos in v {
                                        nv += 1;
                                        for k in 0..3 { vmn[k] = vmn[k].min(pos[k]); vmx[k] = vmx[k].max(pos[k]); }
                                    }
                                }
                            }
                        }
                    }
                }
            }
            // the lightmapper's record box: the fold of the LOD-0 visuals whose material is NOT a leaf material
            let mut bmn = [f32::MAX; 3];
            let mut bmx = [f32::MIN; 3];
            for e in lod {
                let leaf = m.materials.get(e.material as usize).map(|x| x.leaf).unwrap_or(false);
                if leaf { continue; }
                if let Some(main) = &e.visual.main {
                    let b = main.bounding_box;
                    for k in 0..3 { bmn[k] = bmn[k].min(b[k] - b[3 + k]); bmx[k] = bmx[k].max(b[k] + b[3 + k]); }
                }
            }
            // FUN_140184fa0's fold in f32: first box copied, then per axis min = minss(b.c − b.h, acc.c − acc.h), max =
            // maxss(b.h + b.c, acc.c + acc.h), c = (max + min)·0.5, h = (max − min)·0.5 — printed as exact bits too
            let mut acc: Option<([f32; 3], [f32; 3])> = None;
            for e in lod {
                let leaf = m.materials.get(e.material as usize).map(|x| x.leaf).unwrap_or(false);
                if leaf { continue; }
                if let Some(main) = &e.visual.main {
                    let b = main.bounding_box;
                    let (bc, bh) = ([b[0], b[1], b[2]], [b[3], b[4], b[5]]);
                    acc = Some(match acc {
                        None => (bc, bh),
                        Some((ac, ah)) => {
                            let mut c = [0f32; 3];
                            let mut h = [0f32; 3];
                            for k in 0..3 {
                                let mn = (bc[k] - bh[k]).min(ac[k] - ah[k]);
                                let mx = (bh[k] + bc[k]).max(ac[k] + ah[k]);
                                c[k] = (mx + mn) * 0.5;
                                h[k] = (mx - mn) * 0.5;
                            }
                            (c, h)
                        }
                    });
                }
            }
            // the recompute from the bark visuals' vertices (CPlugVisualIndexedTriangles vtbl+0x200: min/max over the
            // indexed vertices, c = (max + min)·0.5, h = (max − min)·0.5) folded the same way
            let mut acc2: Option<([f32; 3], [f32; 3])> = None;
            for e in lod {
                let leaf = m.materials.get(e.material as usize).map(|x| x.leaf).unwrap_or(false);
                if leaf { continue; }
                if let Some(main) = &e.visual.main {
                    let mut vmn = [f32::MAX; 3];
                    let mut vmx = [f32::MIN; 3];
                    for s in &main.vertex_streams {
                        if let Some(mapgeom::static_item::Node::VertexStream(vs)) = s.inline.as_deref() {
                            for (d, el) in vs.decls.iter().zip(vs.elems.iter()) {
                                if d.name() != mapgeom::static_item::vstream::N_POSITION { continue; }
                                if let mapgeom::static_item::vstream::Elem::Float3(v) = el {
                                    for pos in v { for k in 0..3 { vmn[k] = vmn[k].min(pos[k]); vmx[k] = vmx[k].max(pos[k]); } }
                                }
                            }
                        }
                    }
                    let bc = [(vmx[0] + vmn[0]) * 0.5, (vmx[1] + vmn[1]) * 0.5, (vmx[2] + vmn[2]) * 0.5];
                    let bh = [(vmx[0] - vmn[0]) * 0.5, (vmx[1] - vmn[1]) * 0.5, (vmx[2] - vmn[2]) * 0.5];
                    println!("   visual {} vertex-recomputed box centre {:?} half {:?} (stored {:?} {:?})", e.node_index, bc, bh, &main.bounding_box[..3], &main.bounding_box[3..]);
                    acc2 = Some(match acc2 {
                        None => (bc, bh),
                        Some((ac, ah)) => {
                            let mut c = [0f32; 3];
                            let mut h = [0f32; 3];
                            for k in 0..3 {
                                let mn = (bc[k] - bh[k]).min(ac[k] - ah[k]);
                                let mx = (bh[k] + bc[k]).max(ac[k] + ah[k]);
                                c[k] = (mx + mn) * 0.5;
                                h[k] = (mx - mn) * 0.5;
                            }
                            (c, h)
                        }
                    });
                }
            }
            if let Some((c, h)) = acc2 {
                println!("   bark-only RECOMPUTED fold: centre {c:?} ({:?}) half {h:?} ({:?})", c.iter().map(|v| format!("{:#010x}", v.to_bits())).collect::<Vec<_>>(), h.iter().map(|v| format!("{:#010x}", v.to_bits())).collect::<Vec<_>>());
            }
            if let Some((c, h)) = acc {
                println!("   bark-only game fold: centre {c:?} ({:?}) half {h:?} ({:?})", c.iter().map(|v| format!("{:#010x}", v.to_bits())).collect::<Vec<_>>(), h.iter().map(|v| format!("{:#010x}", v.to_bits())).collect::<Vec<_>>());
            }
            println!("   bark-only fold: centre {:?} half {:?}", (0..3).map(|k| (bmn[k] + bmx[k]) * 0.5).collect::<Vec<f32>>(), (0..3).map(|k| (bmx[k] - bmn[k]) * 0.5).collect::<Vec<f32>>());
            let c: Vec<f32> = (0..3).map(|k| (mn[k] + mx[k]) * 0.5).collect();
            let h: Vec<f32> = (0..3).map(|k| (mx[k] - mn[k]) * 0.5).collect();
            println!("{p} LOD {li}: {} visuals, stored (c±h) fold min {mn:?} max {mx:?} → centre {c:?} half {h:?}; vertex fold ({nv} verts) min {vmn:?} max {vmx:?}", lod.len());
        }
    }
}
