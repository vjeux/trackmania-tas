// E2 scratch: an item's PreLightGen (both uv-set bounds) + per geom every TEXCOORD set's range
use mapgeom::static_item::vstream::Elem;
fn range(u: &[[f32; 2]]) -> String { let (mut lo, mut hi) = ([f32::MAX; 2], [f32::MIN; 2]); for v in u { for k in 0..2 { lo[k] = lo[k].min(v[k]); hi[k] = hi[k].max(v[k]); } } format!("[{:.4} {:.4}]..[{:.4} {:.4}]", lo[0], lo[1], hi[0], hi[1]) }
fn main() {
    for p in std::env::args().skip(1) {
        let bytes = std::fs::read(&p).expect("read");
        let f = mapgeom::static_item::file::parse_file(&bytes).expect("parse");
        let Some(so) = f.item.static_object() else { println!("{p}: no static object"); continue };
        let Some(s2) = so.solid2() else { println!("{p}: no solid2"); continue };
        let name = p.rsplit('/').next().unwrap();
        match &s2.pre_light_gen {
            Some(plg) => println!("{name}: PLG v{} u01 {} MeterByUv {} u03 {} set0 [{:.6} {:.6} {:.6} {:.6}] set1 [{:.3e} {:.3e} {:.3e} {:.3e}] sprites {:?} boxes {} uv_groups {}", plg.version, plg.u01, plg.u02, plg.u03, plg.u04[0], plg.u04[1], plg.u04[2], plg.u04[3], plg.u04[4], plg.u04[5], plg.u04[6], plg.u04[7], plg.sprite_count, plg.boxes.len(), plg.uv_groups.len()),
            None => println!("{name}: no PLG"),
        }
        for (gi, sg) in s2.shaded_geoms.iter().enumerate() {
            let Some(vr) = s2.visuals.get(sg.visual_index as usize) else { continue };
            let Some(mapgeom::static_item::Node::Visual(vis)) = vr.inline.as_deref() else { continue };
            let link = lightmap::lmmesh::geom_material_link_ext(s2, sg, None);
            let mut sets = String::new();
            if let Some(st) = vis.stream() {
                for (d, e) in st.decls.iter().zip(st.elems.iter()) {
                    let n = d.name();
                    if n >= mapgeom::static_item::vstream::N_TEXCOORD0 && n < mapgeom::static_item::vstream::N_TEXCOORD0 + 8 {
                        if let Elem::Float2(u) = e { sets += &format!(" tc{} {}", n - mapgeom::static_item::vstream::N_TEXCOORD0, range(u)); } else { sets += &format!(" tc{} (not Float2)", n - mapgeom::static_item::vstream::N_TEXCOORD0); }
                    }
                }
                let decls: Vec<String> = st.decls.iter().map(|d| format!("{:#x}", d.name())).collect();
                sets += &format!(" decls {:?}", decls);
            }
            if let Some(m) = vis.main.as_ref() { for (k, s) in m.tex_coord_sets.iter().enumerate() { let u: Vec<[f32; 2]> = s.coords.iter().map(|c| c.0).collect(); sets += &format!(" main.set{k} {}", range(&u)); } }
            println!("  geom {gi}: visual {} lod {:#x} u01 {} u02 {} mat {} '{}':{sets}", sg.visual_index, sg.lod_mask, sg.u01, sg.u02, sg.material_index, link.rsplit('\\').next().unwrap_or(""));
        }
    }
}
