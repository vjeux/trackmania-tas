//! Do stubborn positions have coplanar faces in HIS mesh but not mine?
//! Usage: flatcheck HIS.ITEM MINE.ITEM
use std::collections::BTreeMap;
use mapgeom::static_item::vstream::Elem;
fn mk(p: &[f32; 3]) -> (i32, i32, i32) {
    ((p[0]*1000.0).round() as i32, (p[1]*1000.0).round() as i32, (p[2]*1000.0).round() as i32)
}
fn main() {
    let a: Vec<String> = std::env::args().collect();
    let load = |path: &str| -> Vec<(Vec<[f32; 3]>, String)> {
        let data = std::fs::read(path).unwrap();
        let f = mapgeom::static_item::file::parse_file(&data).unwrap();
        let so = f.item.static_object().unwrap();
        let s2 = so.solid2().unwrap();
        let mut out = Vec::new();
        for g in &s2.shaded_geoms {
            let vi = g.visual_index.max(0) as usize;
            let mi = g.material_index.max(0) as usize;
            let mat = s2.custom_materials.get(mi).and_then(|m| m.inst()).map(|i| i.link().unwrap_or("").to_string()).unwrap_or("".into());
            if !mat.ends_with("\\Technics") { continue; }
            if let Some(vref) = s2.visuals.get(vi) {
                if let Some(mapgeom::static_item::Node::Visual(vis)) = vref.inline.as_deref() {
                    let st = vis.stream().unwrap();
                    let mut pos = Vec::new();
                    for (d, e) in st.decls.iter().zip(st.elems.iter()) {
                        if let Elem::Float3(p) = e {
                            if d.name() == 0 { pos = p.clone(); }
                        }
                    }
                    let idx = vis.index_buffer.as_ref().map(|b| b.indices.clone()).unwrap_or_default();
                    for t in idx.chunks(3) {
                        if t.len() < 3 { continue; }
                        out.push((vec![pos[t[0] as usize], pos[t[1] as usize], pos[t[2] as usize]], mat.clone()));
                    }
                }
            }
        }
        out
    };
    let r = load(&a[1]);
    let m = load(&a[2]);
    let mut mmap: BTreeMap<[(i32, i32, i32); 3], Vec<usize>> = BTreeMap::new();
    for (i, (t, _)) in m.iter().enumerate() {
        let mut k = [mk(&t[0]), mk(&t[1]), mk(&t[2])];
        k.sort();
        mmap.entry(k).or_default().push(i);
    }
    // group MY tris by position; find positions where MY faces spread but HIS weld
    // (approximate stubborn via my maxdih>50 and his stored=1 -- need his stored; simplify: my maxdih>50 positions, compare his face spread)
    let mut mybypos: BTreeMap<[u32; 3], Vec<[f32; 3]>> = BTreeMap::new();
    for (t, _) in &m {
        // face normal
        let e1 = [t[1][0]-t[0][0], t[1][1]-t[0][1], t[1][2]-t[0][2]];
        let e2 = [t[2][0]-t[0][0], t[2][1]-t[0][1], t[2][2]-t[0][2]];
        let cr = [e1[1]*e2[2]-e1[2]*e2[1], e1[2]*e2[0]-e1[0]*e2[2], e1[0]*e2[1]-e1[1]*e2[0]];
        let l = (cr[0]*cr[0]+cr[1]*cr[1]+cr[2]*cr[2]).sqrt().max(1e-30);
        let nn = [cr[0]/l, cr[1]/l, cr[2]/l];
        for k in 0..3 {
            mybypos.entry([t[k][0].to_bits(), t[k][1].to_bits(), t[k][2].to_bits()]).or_default().push(nn);
        }
    }
    // his face spread at same positions (via nearest my position)
    let mut cmp: Vec<(f32, f32)> = Vec::new(); // (my_spread_deg, his_spread_deg)
    for (t, _) in &r {
        let e1 = [t[1][0]-t[0][0], t[1][1]-t[0][1], t[1][2]-t[0][2]];
        let e2 = [t[2][0]-t[0][0], t[2][1]-t[0][1], t[2][2]-t[0][2]];
        let cr = [e1[1]*e2[2]-e1[2]*e2[1], e1[2]*e2[0]-e1[0]*e2[2], e1[0]*e2[1]-e1[1]*e2[0]];
        let l = (cr[0]*cr[0]+cr[1]*cr[1]+cr[2]*cr[2]).sqrt().max(1e-30);
        let nn = [cr[0]/l, cr[1]/l, cr[2]/l];
        for k in 0..3 {
            // find my position within 0.5mm
            let mut best: Option<([u32; 3], f32)> = None;
            for dx in -1..=1 {
                for dy in -1..=1 {
                    for dz in -1..=1 {
                        let kk = (mk(&t[k]).0+dx, mk(&t[k]).1+dy, mk(&t[k]).2+dz);
                        // mybypos keyed by bits, not mmkey; need bit scan (slow). Approximate: skip.
                        let _ = kk;
                    }
                }
            }
            let _ = (best, nn);
        }
    }
    println!("todo: bit-scan too slow, use different approach");
    let _ = (mmap, mybypos, cmp);
}
