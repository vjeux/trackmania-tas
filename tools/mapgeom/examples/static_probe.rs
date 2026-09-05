//! Print the raw placement-param chunks and the visual bounding boxes of a
//! static item (data for `static_item::build`).
use mapgeom::static_item::item::ItemChunk;
use mapgeom::static_item::{parse_file, vstream::Elem, Node};
fn main() {
    let f = parse_file(&std::fs::read(std::env::args().nth(1).unwrap()).unwrap()).unwrap();
    for c in &f.item.chunks {
        if let ItemChunk::DefaultPlacement { placement, .. } = c {
            if let Some(Node::Placement(p)) = placement.inline.as_deref() {
                for rc in &p.chunks {
                    println!("0x{:08X}: {}", rc.id, rc.payload.iter().map(|b| format!("{b:02x}")).collect::<String>());
                }
            }
        }
    }
    let so = f.item.static_object().unwrap();
    let s2 = so.solid2().unwrap();
    for (i, v) in s2.visuals.iter().enumerate() {
        if let Some(Node::Visual(vis)) = v.inline.as_deref() {
            let m = vis.main.as_ref().unwrap();
            if let Some(Elem::Float3(p)) = vis.stream().and_then(|s| s.elems.first()) {
                let mut lo = [f32::MAX; 3];
                let mut hi = [f32::MIN; 3];
                for q in p {
                    for k in 0..3 {
                        lo[k] = lo[k].min(q[k]);
                        hi[k] = hi[k].max(q[k]);
                    }
                }
                println!("visual {i}: flags 0x{:x} box {:?} min {:?} max {:?} uvgroups {:?} u02 {} u03 {} tangents {:?}", m.chunk_flags, m.bounding_box, lo, hi, m.uv_groups, m.u02, m.u03, vis.tangents.as_ref().map(|(a, b)| (a.len(), b.len())));
            }
        }
    }
    println!("solid2 prelight {:?} flags {} filetime {} u07 {}", s2.pre_light_gen, s2.flags, s2.file_write_time, s2.u07);
    for m in &s2.custom_materials {
        println!("material {:?}", m.inst().map(|i| (i.link().map(|s| s.to_string()), i.physics(), i.main.as_ref().map(|m| m.version), i.tiling.clone(), i.chunk2)));
    }
}
