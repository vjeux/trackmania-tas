//! `re17_itemlods ITEM.Item.Gbx…` — the Solid2Model's shaded geoms (visual, material, lod) and, per visual, the vertex-stream
//! elements present (POSITION / NORMAL / TEXCOORD0 / TEXCOORD1 …), so the LM-charted visuals (TEXCOORD1) per LOD are visible.
//! RE 17 2026-09-30 16:20Z (V6-1c option (b): does the game's LM mesh take another visual/LOD than the port's?).
use mapgeom::static_item::Node;
fn main() {
    for path in std::env::args().skip(1) {
        let bytes = std::fs::read(&path).expect("read");
        let file = mapgeom::static_item::parse_file(&bytes).expect("parse");
        let name = path.rsplit('/').next().unwrap_or(&path).to_string();
        let Some(s2) = file.item.static_object().and_then(|so| so.solid2()) else { println!("{name}: no Solid2Model"); continue };
        println!("{name}: {} visuals, {} shaded geoms", s2.visuals.len(), s2.shaded_geoms.len());
        for g in &s2.shaded_geoms { println!("  geom visual {} material {} lod_mask {} (u01 {} u02 {})", g.visual_index, g.material_index, g.lod_mask, g.u01, g.u02); }
        for (vi, vr) in s2.visuals.iter().enumerate() {
            let Some(Node::Visual(v)) = vr.inline.as_deref() else { continue };
            let Some(st) = v.stream() else { continue };
            let names: Vec<String> = st.decls.iter().map(|d| format!("{}", d.name())).collect();
            let ntri = v.index_buffer.as_ref().map(|ib| ib.indices.len() / 3).unwrap_or(0);
            println!("  visual {vi}: {ntri} tris, {} verts, elements {:?}", st.decls.first().map(|_| st.elems.first().map(|e| match e { mapgeom::static_item::vstream::Elem::Float3(p) => p.len(), _ => 0 }).unwrap_or(0)).unwrap_or(0), names);
        }
    }
}
