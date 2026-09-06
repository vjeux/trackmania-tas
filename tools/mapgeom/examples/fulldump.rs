//! Full scalar dump: solid2 everything, item chunks, header chunks. Usage: fulldump FILE
fn main() {
    let a: Vec<String> = std::env::args().collect();
    let data = std::fs::read(&a[1]).unwrap();
    let f = mapgeom::static_item::file::parse_file(&data).unwrap();
    println!("file: version={} format={} class=0x{:08X} nodes={} header_chunks={:?} reftable_len={}", f.version, f.format, f.class_id, f.num_nodes, f.header_chunks.iter().map(|c| format!("0x{:08X}:{}", c.id, c.payload.len())).collect::<Vec<_>>(), f.ref_table.len());
    for c in &f.item.chunks {
        println!("  item chunk: 0x{:08X}", c.id());
    }
    let so = f.item.static_object().unwrap();
    let s2 = so.solid2().unwrap();
    println!("solid2 chunks={:08X?} version={} u01={:?} nvis={} nmat={} skel_idx={} lodmax={:?} viscst={} filetime={} u03={:?} folder={:?} u04={:?} nlights={} flags=0x{:x} u05={} u06={:?} u07={} damage={} joints={:?} u10={:?} u11={} u12={:?} u13={} u15={} u16={} u17={:?} u18={} boxes={} u19={}", s2.chunks, s2.version, s2.u01, s2.visuals.len(), s2.custom_materials.len(), s2.skel.index, s2.lod_max_dist, s2.vis_cst_type, s2.file_write_time, s2.u03, s2.materials_folder, s2.u04, s2.lights.len(), s2.flags, s2.u05, s2.u06, s2.u07, s2.damage_zone, s2.joints, s2.u10, s2.u11, s2.u12, s2.u13, s2.u15, s2.u16, s2.u17, s2.u18, s2.boxes.len(), s2.u19.len());
    println!("  prelight={:?}", s2.pre_light_gen.as_ref().map(|p| (p.version, p.u01, p.u02, p.u03, p.sprite_count)));
    for (i, v) in s2.visuals.iter().enumerate() {
        if let Some(mapgeom::static_item::Node::Visual(vis)) = v.inline.as_deref() {
            println!("  vis{i}: chunks={} splits={} subvis={} ufloat={} skin={}", vis.chunks.len(), vis.splits.len(), vis.sub_visuals.len(), vis.u_float, vis.main.as_ref().map(|m| m.skin.is_some()).unwrap_or(false));
        }
    }
}
