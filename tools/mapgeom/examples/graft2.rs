//! Clean-room graft through the proven assemble pipeline (no node surgery):
//! visuals + materials from DONOR, collision/prelight/trigger/waypoint from
//! RECIP. Usage: graft2 RECIP DONOR OUT IDENT [GI...]
use mapgeom::static_item::build::{assemble, BuildOpts, Merged, MergedVisual};

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let rd = std::fs::read(&a[1]).unwrap();
    let dd = std::fs::read(&a[2]).unwrap();
    let f = mapgeom::static_item::file::parse_file(&rd).unwrap();
    let g = mapgeom::static_item::file::parse_file(&dd).unwrap();
    let want: Option<Vec<usize>> = if a.len() > 5 { Some(a[5..].iter().flat_map(|s| s.split(',')).map(|s| s.parse().unwrap()).collect()) } else { None };
    let mut m = Merged::default();
    // GRAFT2_STEM: donor visuals on the RECIPIENT's full material table
    // (links matched by stem): tests whether our material table greys.
    let stem_mode = std::env::var("GRAFT2_STEM").is_ok();
    let stem = |l: &str| l.replace("SpecialSignTurbo", "Sign").replace("SpecialSignOff", "SignOff").replace("SpecialFXTurbo", "SpecialFX").replace("DecalSpecialTurbo", "Decal").replace("Modifier\\Turbo\\", "").replace("Material\\", "");
    let rs0 = f.item.static_object().unwrap().solid2().unwrap();
    if stem_mode {
        for mm in &rs0.custom_materials {
            if let Some(inst) = mm.inst() {
                m.materials.push(inst.clone());
            }
        }
    }
    let mut recip_stem: std::collections::BTreeMap<String, usize> = Default::default();
    for (i, mm) in rs0.custom_materials.iter().enumerate() {
        if let Some(inst) = mm.inst() {
            recip_stem.entry(stem(inst.link().unwrap_or(""))).or_insert(i);
        }
    }
    // donor visuals + exact material instances
    let gs = g.item.static_object().unwrap().solid2().unwrap();
    for (gi, geom) in gs.shaded_geoms.iter().enumerate() {
        if let Some(w) = &want {
            if !w.contains(&gi) {
                continue;
            }
        }
        let vi = geom.visual_index.max(0) as usize;
        let mi = geom.material_index.max(0) as usize;
        let (vr, mm) = (&gs.visuals[vi], &gs.custom_materials[mi]);
        if let (Some(mapgeom::static_item::Node::Visual(v)), Some(inst)) = (vr.inline.as_deref(), mm.inst()) {
            if stem_mode {
                let slot = recip_stem.get(&stem(inst.link().unwrap_or(""))).copied().unwrap_or(0);
                m.visuals.push(MergedVisual { visual: v.clone(), material: slot });
                continue;
            }
            let mut slot = m.materials.iter().position(|e| e == inst);
            if slot.is_none() {
                m.materials.push(inst.clone());
                slot = Some(m.materials.len() - 1);
            }
            m.visuals.push(MergedVisual { visual: v.clone(), material: slot.unwrap() });
        }
    }
    // recip collision/prelight/trigger/waypoint/collection
    let rs = f.item.static_object().unwrap().solid2().unwrap();
    if let Some(mapgeom::static_item::Node::Surface(s)) = f.item.static_object().unwrap().shape.inline.as_deref() {
        if let mapgeom::static_item::surface::Surf::Mesh { vertices, triangles, .. } = &s.surf {
            for v in vertices {
                m.surf_vertices.push(*v);
            }
            for t in triangles {
                let si = m.surf_id_slot(t.material_id as u16 | ((t.u03 as u16) << 8));
                let mut nt = t.clone();
                nt.surface_index = si;
                m.surf_triangles.push(nt);
            }
        } else {
            m.notes.push("recip surface not a mesh: collision dropped".into());
        }
    }
    m.pre_light_gen = rs.pre_light_gen.clone();
    m.file_write_time = rs.file_write_time;
    let re = f.item.model().unwrap().entity_model().unwrap();
    if let Some(mapgeom::static_item::Node::Surface(t)) = re.trigger_shape.inline.as_deref() {
        m.trigger = Some(t.clone());
    }
    let mut waypoint_type = 3;
    for c in &f.item.chunks {
        if let mapgeom::static_item::item::ItemChunk::Waypoint { waypoint_type: w, .. } = c {
            waypoint_type = *w;
        }
    }
    m.waypoint_type = if waypoint_type == 3 { None } else { Some(waypoint_type) };
    let mut collection = 26u32;
    for c in &f.item.chunks {
        if let mapgeom::static_item::item::ItemChunk::Ident { collection: coll, .. } = c {
            if let mapgeom::static_item::Id::Raw(n) = coll {
                collection = *n;
            }
        }
    }
    let opts = BuildOpts { ident: a[4].clone(), author: a[4].clone(), scale: 1.0, collection, editors: false };
    let out = assemble(&m, &opts).unwrap();
    std::fs::write(&a[3], mapgeom::static_item::file::write_file(&out)).unwrap();
    println!("graft2: {} visuals, {} materials", m.visuals.len(), m.materials.len());
}
