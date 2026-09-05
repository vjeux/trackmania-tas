//! Full per-visual structural dump: flags, count, decls, elem kinds+lengths,
//! index count, tangent lens, box. Usage: static_detail A B ... (compares side by side)
use mapgeom::static_item::vstream::Elem;

fn dump(path: &str) {
    println!("== {path}");
    let data = std::fs::read(path).unwrap();
    let f = mapgeom::static_item::file::parse_file(&data).unwrap();
    let so = f.item.static_object().unwrap();
    let s2 = so.solid2().unwrap();
    println!("  geoms (vis, mat): {:?}", s2.shaded_geoms.iter().map(|g| (g.visual_index, g.material_index)).collect::<Vec<_>>());
    for (i, v) in s2.visuals.iter().enumerate() {
        if let Some(mapgeom::static_item::Node::Visual(vis)) = v.inline.as_deref() {
            let m = vis.main.as_ref().unwrap();
            let mut elems = Vec::new();
            if let Some(st) = vis.stream() {
                for (d, e) in st.decls.iter().zip(st.elems.iter()) {
                    let n = match e {
                        Elem::Float2(v) => v.len(),
                        Elem::Float3(v) => v.len(),
                        Elem::Float4(v) => v.len(),
                        Elem::Word(v) => v.len(),
                        Elem::Raw { size, bytes } => bytes.len() / size.max(&1),
                    };
                    elems.push(format!("(name={} type={} space={} off={} n={})", d.name(), d.ty(), d.space(), d.offset(), n));
                }
            }
            let ni = vis.index_buffer.as_ref().map(|b| b.indices.len()).unwrap_or(0);
            let tg = vis.tangents.as_ref().map(|(a, b)| (a.len(), b.len()));
            println!("  vis{i}: flags=0x{:x} count={} idx={} tangents={:?} box={:?}\n    elems {}", m.chunk_flags, m.count, ni, tg,
                m.bounding_box, elems.join(" "));
        }
    }
    println!("  materials: {:?}", s2.custom_materials.iter().filter_map(|m| m.inst()).map(|x| (x.link().unwrap_or("?").rsplit('\\').next().unwrap_or("?").to_string(), x.physics())).collect::<Vec<_>>());
}

fn main() {
    for a in std::env::args().skip(1) {
        dump(&a);
    }
}
