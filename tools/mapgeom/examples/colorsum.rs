//! Per visual: flags, vert count, color distinct-value summary. Usage: colorsum FILE...
use mapgeom::static_item::vstream::Elem;
fn main() {
    for path in std::env::args().skip(1) {
        let short = path.rsplit('/').next().unwrap_or(&path);
        let data = std::fs::read(&path).unwrap();
        let f = mapgeom::static_item::file::parse_file(&data).unwrap();
        let so = f.item.static_object().unwrap();
        let s2 = so.solid2().unwrap();
        for (i, v) in s2.visuals.iter().enumerate() {
            if let Some(mapgeom::static_item::Node::Visual(vis)) = v.inline.as_deref() {
                let m = vis.main.as_ref().unwrap();
                let mut info = format!("{short} vis{i} flags=0x{:x} n={}", m.chunk_flags, m.count);
                if let Some(st) = vis.stream() {
                    for (d, e) in st.decls.iter().zip(st.elems.iter()) {
                        if d.name() == 8 {
                            if let Elem::Word(w) = e {
                                let mut counts: std::collections::BTreeMap<u32, usize> = std::collections::BTreeMap::new();
                                for x in w {
                                    *counts.entry(*x).or_default() += 1;
                                }
                                info += &format!(" colors_distinct={} total={}", counts.len(), w.len());
                                let mut top: Vec<_> = counts.into_iter().collect();
                                top.sort_by_key(|(_, c)| std::cmp::Reverse(*c));
                                info += &format!(" top={:08x}..", top.iter().take(4).map(|(v, _)| *v).collect::<Vec<_>>()[0]);
                            }
                        }
                    }
                }
                println!("{info}");
            }
        }
    }
}
