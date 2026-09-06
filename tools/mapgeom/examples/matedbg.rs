//! Debug mate key overlap. Usage: matedbg MINE.ITEM SRCFILE
use std::collections::BTreeSet;
use mapgeom::static_item::bake::geometry_layers;
use mapgeom::static_item::vstream::Elem;
fn mk(p: &[f32; 3]) -> (i32, i32, i32) {
    ((p[0]*1000.0).round() as i32, (p[1]*1000.0).round() as i32, (p[2]*1000.0).round() as i32)
}
fn main() {
    let a: Vec<String> = std::env::args().collect();
    let t = [-0.0019760131836f32, -0.0000076293945312, -0.023214340210];
    let data = std::fs::read(&a[2]).unwrap();
    let g = tmmaps::gbx::Gbx::parse(&data);
    let loc = mapgeom::crystal_model::locate(&g.body).unwrap();
    let (c, _, _) = mapgeom::crystal_model::CPlugCrystal::parse_with(&g.body, loc.at, loc.lookback.clone()).unwrap();
    let layers = geometry_layers(&c);
    let mut matekeys: BTreeSet<[(i32, i32, i32); 3]> = BTreeSet::new();
    let mut nq = 0;
    for (cr, vis, _) in &layers {
        if !vis { continue; }
        for f in &cr.faces {
            if f.verts.len() != 4 { continue; }
            nq += 1;
            let pts: Vec<[f32; 3]> = f.verts.iter().map(|i| cr.positions[*i as usize]).collect();
            for tri in [[pts[0], pts[1], pts[2]], [pts[0], pts[2], pts[3]]] {
                let h = [[tri[0][0]*0.5+t[0], tri[0][1]*0.5+t[1], tri[0][2]*0.5+t[2]],
                         [tri[1][0]*0.5+t[0], tri[1][1]*0.5+t[1], tri[1][2]*0.5+t[2]],
                         [tri[2][0]*0.5+t[0], tri[2][1]*0.5+t[1], tri[2][2]*0.5+t[2]]];
                let mut k = [mk(&h[0]), mk(&h[1]), mk(&h[2])];
                k.sort();
                matekeys.insert(k);
            }
        }
    }
    let hd = std::fs::read(&a[1]).unwrap();
    let hf = mapgeom::static_item::file::parse_file(&hd).unwrap();
    let so = hf.item.static_object().unwrap();
    let s2 = so.solid2().unwrap();
    let mut mykeys: BTreeSet<[(i32, i32, i32); 3]> = BTreeSet::new();
    for gg in &s2.shaded_geoms {
        let vi = gg.visual_index.max(0) as usize;
        if let Some(vref) = s2.visuals.get(vi) {
            if let Some(mapgeom::static_item::Node::Visual(vis)) = vref.inline.as_deref() {
                let st = vis.stream().unwrap();
                let mut pos = Vec::new();
                for (dd2, e) in st.decls.iter().zip(st.elems.iter()) {
                    if let Elem::Float3(p) = e {
                        if dd2.name() == 0 { pos = p.clone(); }
                    }
                }
                let idx = vis.index_buffer.as_ref().map(|b| b.indices.clone()).unwrap_or_default();
                for tt in idx.chunks(3) {
                    if tt.len() < 3 { continue; }
                    let p = [pos[tt[0] as usize], pos[tt[1] as usize], pos[tt[2] as usize]];
                    let mut k = [mk(&p[0]), mk(&p[1]), mk(&p[2])];
                    k.sort();
                    mykeys.insert(k);
                }
            }
        }
    }
    let inter = matekeys.intersection(&mykeys).count();
    println!("quads={nq} matekeys={} mykeys={} overlap={inter}", matekeys.len(), mykeys.len());
}
