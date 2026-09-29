//! Graft donor's visuals (+their materials) into recipient's solid, replacing
//! recipient visuals/geoms. Node indices are recycled from the recipient's
//! own freed visual/stream slots (invented indices collide with the lookback
//! table and the loader drops the file). Usage: graft RECIP DONOR OUT [GI...]
//! (default: all donor geoms).
use mapgeom::static_item::NodeRef;

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let rd = std::fs::read(&a[1]).unwrap();
    let dd = std::fs::read(&a[2]).unwrap();
    let mut f = mapgeom::static_item::file::parse_file(&rd).unwrap();
    let g = mapgeom::static_item::file::parse_file(&dd).unwrap();
    // usage: graft RECIP DONOR OUT [GI...]
    let want: Option<Vec<usize>> = if a.len() > 4 { Some(a[4..].iter().map(|s| s.parse().unwrap()).collect()) } else { None };
    let gs = g.item.static_object().unwrap().solid2().unwrap();
    // (donor visual clone, donor visual index, donor stream indices, link, phys)
    let mut donor = Vec::new();
    for (gi, geom) in gs.shaded_geoms.iter().enumerate() {
        if let Some(w) = &want {
            if !w.contains(&gi) {
                continue;
            }
        }
        let vi = geom.visual_index.max(0) as usize;
        let mi = geom.material_index.max(0) as usize;
        if let (Some(vr), Some(mm)) = (gs.visuals.get(vi), gs.custom_materials.get(mi)) {
            if let (Some(mapgeom::static_item::Node::Visual(v)), Some(inst)) = (vr.inline.as_deref(), mm.inst()) {
                let streams: Vec<i32> = v.main.as_ref().map(|m| m.vertex_streams.iter().map(|r| r.index).collect()).unwrap_or_default();
                donor.push((v.clone(), vr.index, streams, inst.link().unwrap_or("").to_string(), inst.physics()));
            }
        }
    }
    let mc = f.item.model_mut().unwrap();
    if let Some(n) = mc.entity_model.inline.as_deref_mut() {
        if let mapgeom::static_item::Node::EntityModel(em) = n {
            if let Some(sn) = em.static_object.inline.as_deref_mut() {
                if let mapgeom::static_item::Node::StaticObject(so) = sn {
                    if let Some(snn) = so.mesh.inline.as_deref_mut() {
                        if let mapgeom::static_item::Node::Solid2(s) = snn {
                            // separate pools: visual indices and stream indices
                            // (donor visuals can carry a different stream count
                            // than the recipient visual at the same slot --
                            // mixing them swaps roles and the loader drops
                            // the file, full-graft test 2026-09-05).
                            let mut vis_pool: Vec<i32> = Vec::new();
                            let mut stream_pool: Vec<i32> = Vec::new();
                            for vr in &s.visuals {
                                vis_pool.push(vr.index);
                                if let Some(mapgeom::static_item::Node::Visual(v)) = vr.inline.as_deref() {
                                    if let Some(main) = v.main.as_ref() {
                                        for r in &main.vertex_streams {
                                            stream_pool.push(r.index);
                                        }
                                    }
                                }
                            }
                            let fresh = std::env::var("GRAFT_FRESH").is_ok();
                            let stem_match = std::env::var("GRAFT_STEM").is_ok();
                            // donor link -> recip slot by stemmed name (avoids
                            // material clones entirely under GRAFT_STEM)
                            let stem = |l: &str| l.replace("SpecialSignTurbo", "Sign").replace("SpecialSignOff", "SignOff").replace("SpecialFXTurbo", "SpecialFX").replace("DecalSpecialTurbo", "Decal").replace("Modifier\\Turbo\\", "").replace("Material\\", "");
                            let mut recip_stem: std::collections::BTreeMap<String, usize> = Default::default();
                            for (i, m) in s.custom_materials.iter().enumerate() {
                                if let Some(inst) = m.inst() {
                                    recip_stem.entry(stem(inst.link().unwrap_or(""))).or_insert(i);
                                }
                            }
                            // fresh indices (GRAFT_FRESH) or recycled pool slots;
                            // a single counter closure serves both (two FnMuts
                            // cannot share one counter).
                            let mut fresh_next = {
                                let reserve: i32 = donor.iter().map(|(v, _, streams, _, _)| {
                                    1 + streams.len() as i32
                                }).sum();
                                let base = f.num_nodes as i32;
                                if fresh {
                                    // reserve the whole visual+stream range up
                                    // front: material clones below allocate
                                    // from f.num_nodes and must not overlap it.
                                    f.num_nodes += reserve as u32;
                                }
                                base
                            };
                            let (mut vi, mut si) = (0, 0);
                            let mut take = |is_vis: bool| {
                                if fresh {
                                    let v = fresh_next;
                                    fresh_next += 1;
                                    v
                                } else if is_vis {
                                    let v = vis_pool.get(vi).copied().unwrap_or(-2);
                                    vi += 1;
                                    v
                                } else {
                                    let v = stream_pool.get(si).copied().unwrap_or(-2);
                                    si += 1;
                                    v
                                }
                            };
                            s.visuals.clear();
                            s.shaded_geoms.clear();
                            for (v, _dvi, dstreams, link, _phys) in &donor {
                                let mut slot = None;
                                for (i, m) in s.custom_materials.iter().enumerate() {
                                    if let Some(inst) = m.inst() {
                                        if inst.link().unwrap_or("") == link {
                                            slot = Some(i);
                                            break;
                                        }
                                    }
                                }
                                if slot.is_none() && stem_match {
                                    slot = recip_stem.get(&stem(link)).copied();
                                }
                                let slot = match slot {
                                    Some(i) => i,
                                    None => {
                                        // fresh node index: cloning the donor
                                        // material's Ref verbatim reuses the
                                        // DONOR's index, which collides with a
                                        // live recipient node -- the writer
                                        // then emits a back-ref and the file
                                        // parses as garbage (full-graft test).
                                        let gs2 = g.item.static_object().unwrap().solid2().unwrap();
                                        let dm = gs2.custom_materials.iter().find(|m| m.inst().map(|i| i.link().unwrap_or("") == link).unwrap_or(false)).unwrap();
                                        let mut node = dm.node.clone();
                                        if let Some(r) = node.as_mut() {
                                            r.index = f.num_nodes as i32;
                                            f.num_nodes += 1;
                                        }
                                        s.custom_materials.push(mapgeom::static_item::solid2::Material { name: String::new(), node });
                                        s.custom_materials.len() - 1
                                    }
                                };
                                let mut vv = v.clone();
                                if let Some(main) = vv.main.as_mut() {
                                    for r in main.vertex_streams.iter_mut() {
                                        if r.inline.is_some() {
                                            r.index = take(false);
                                        }
                                    }
                                    let _ = dstreams;
                                }
                                let vi = take(true);
                                let pos = s.visuals.len() as i32;
                                s.visuals.push(NodeRef { index: vi, inline: Some(Box::new(mapgeom::static_item::Node::Visual(vv))) });
                                s.shaded_geoms.push(mapgeom::static_item::solid2::ShadedGeom { visual_index: pos, material_index: slot as i32, u01: -1, lod_mask: 1, u02: 0 });
                            }
                        }
                    }
                }
            }
        }
    }
    // GRAFT_IDENT=new.Item.Gbx re-identifies the output (path+author) so
    // several grafts can share one map without zip-name collisions.
    // The ident lives in TWO places: body chunk 0x2E00100B and header
    // chunk 0x2E001003 (the one the catalog reads) -- rewrite both.
    if let Ok(ident) = std::env::var("GRAFT_IDENT") {
        let mut collection = 26u32;
        for c in f.item.chunks.iter_mut() {
            if let mapgeom::static_item::item::ItemChunk::Ident { path, author, collection: coll } = c {
                if let mapgeom::static_item::Id::Raw(n) = coll {
                    collection = *n;
                }
                *path = mapgeom::static_item::Id::Str(ident.clone());
                *author = mapgeom::static_item::Id::Str(ident.clone());
            }
        }
        f.header_chunks = mapgeom::static_item::build::header_chunks(&mapgeom::static_item::build::BuildOpts { ident: ident.clone(), author: ident.clone(), scale: 1.0, collection, editors: false });
        println!("re-idented as {ident}");
    }
    std::fs::write(&a[3], mapgeom::static_item::file::write_file(&f)).unwrap();
    println!("grafted {} visuals", donor.len());
}
