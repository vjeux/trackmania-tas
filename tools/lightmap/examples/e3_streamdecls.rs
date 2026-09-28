//! `e3_streamdecls ITEM.Item.Gbx…` — every LOD-0 visual's vertex-stream declaration names per embedded item file (positions 0,
//! normals 5, COLOR0 8, TEXCOORD0 10 …; `mapgeom::static_item::vstream`): does an item carry a vertex COLOR0 stream — the
//! PyPxz_Hue pre-pass multiplies its albedo by |COLOR0|^2.2 (RE 16 read, 2026-09-28), 1 without the element.
use mapgeom::static_item::vstream::{Elem, N_COLOR0};
use mapgeom::static_item::Node;

fn main() {
    let mut with_colour = 0usize;
    let mut total = 0usize;
    for path in std::env::args().skip(1) {
        let bytes = match std::fs::read(&path) { Ok(b) => b, Err(e) => { eprintln!("{path}: {e}"); continue; } };
        let file = match mapgeom::static_item::parse_file(&bytes) { Ok(f) => f, Err(e) => { eprintln!("{path}: {e}"); continue; } };
        let Some(s2) = file.item.static_object().and_then(|so| so.solid2()) else { println!("{path}: no static object Solid2Model"); continue };
        let mut names: std::collections::BTreeMap<u32, usize> = Default::default();
        let mut visuals = 0usize;
        let mut colour_vals: Vec<String> = Vec::new();
        for vr in &s2.visuals {
            let Some(Node::Visual(v)) = vr.inline.as_deref() else { continue };
            visuals += 1;
            if let Some(st) = v.stream() {
                for (d, e) in st.decls.iter().zip(st.elems.iter()) {
                    *names.entry(d.name()).or_default() += 1;
                    if d.name() == N_COLOR0 {
                        colour_vals.push(match e {
                            Elem::Float4(c) => format!("f4 {:?}", c.first()),
                            Elem::Word(c) => { let mut h: std::collections::BTreeMap<u32, usize> = Default::default(); for w in c { *h.entry(*w).or_default() += 1; } format!("words {:?}", h.iter().map(|(k, n)| format!("{k:#010x}×{n}")).collect::<Vec<_>>()) }
                            _ => "other".to_string(),
                        });
                    }
                }
            }
        }
        total += 1;
        let has = names.contains_key(&N_COLOR0);
        if has { with_colour += 1; }
        println!("{}\t{} visuals\tdecls {:?}\tCOLOR0 {}{}", path.rsplit('/').next().unwrap_or(&path), visuals, names, if has { "YES" } else { "no" }, if colour_vals.is_empty() { String::new() } else { format!(" first {:?}", &colour_vals[..colour_vals.len().min(3)]) });
    }
    println!("{with_colour} of {total} files carry a COLOR0 element");
}
