//! Raw structural dump of one visual: chunk ids, main scalar fields, stream
//! version/flags/compress, decl flags words, index chunk/flags. Usage:
//! structdump FILE VISIDX
fn main() {
    let a: Vec<String> = std::env::args().collect();
    let vi: usize = a[2].parse().unwrap();
    let data = std::fs::read(&a[1]).unwrap();
    let f = mapgeom::static_item::file::parse_file(&data).unwrap();
    let so = f.item.static_object().unwrap();
    let s2 = so.solid2().unwrap();
    if let Some(mapgeom::static_item::Node::Visual(vis)) = s2.visuals[vi].inline.as_deref() {
        println!("chunks: {:08X?}", vis.chunks);
        let m = vis.main.as_ref().unwrap();
        println!("main version={} flags=0x{:x} count={} u02={} u03={} u04len={} texsets={:?} uvgroups={:?} bitmaps={}", m.version, m.chunk_flags, m.count, m.u02, m.u03, m.u04.len(), m.tex_coord_sets, m.uv_groups, m.bitmap_elems.len());
        println!("skin={} morph={:?} v3d_idx={} tangents={:?}", m.skin.is_some(), vis.morph, vis.v3d_node.index, vis.tangents.as_ref().map(|(a, b)| (a.len(), b.len())));
        if let Some(ib) = &vis.index_buffer {
            println!("indexbuffer chunk=0x{:08X} flags={} n={}", ib.chunk, ib.flags, ib.indices.len());
        } else {
            println!("indexbuffer NONE");
        }
        if let Some(st) = vis.stream() {
            println!("stream version={} count={} flags={} compress={:?}", st.version, st.count, st.flags, st.compress_local3d);
            for d in &st.decls {
                println!("  decl flags1=0x{:08X} flags2=0x{:08X} extra={:?} v0len={}", d.flags1, d.flags2, d.extra, d.v0_data.len());
            }
        }
    }
}
