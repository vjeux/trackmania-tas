//! f32-path vs f64-path on near-zero verts. Usage: zeroy HIS.ITEM SRCFILE
use std::collections::BTreeMap;
use mapgeom::static_item::bake::geometry_layers;
use mapgeom::static_item::vstream::Elem;
fn mk(p: &[f32; 3]) -> (i32, i32, i32) {
    ((p[0]*1000.0).round() as i32, (p[1]*1000.0).round() as i32, (p[2]*1000.0).round() as i32)
}
fn main() {
    let a: Vec<String> = std::env::args().collect();
    let data = std::fs::read(&a[2]).unwrap();
    let g = tmmaps::gbx::Gbx::parse(&data);
    let loc = mapgeom::crystal_model::locate(&g.body).unwrap();
    let (c, _, _) = mapgeom::crystal_model::CPlugCrystal::parse_with(&g.body, loc.at, loc.lookback.clone()).unwrap();
    let layers = geometry_layers(&c);
    let d = [-0.001976f32, -0.000008, -0.023215];
    let t64 = [-0.001976013f64, -0.000007629, -0.023214340];
    let t32 = [t64[0] as f32, t64[1] as f32, t64[2] as f32];
    let mut mmap: BTreeMap<[(i32, i32, i32); 3], Vec<[[f32; 3]; 3]>> = BTreeMap::new();
    for (cr, vis, _) in &layers {
        if !vis { continue; }
        for f in &cr.faces {
            let pts: Vec<[f32; 3]> = f.verts.iter().map(|i| cr.positions[*i as usize]).collect();
            if pts.len() < 3 { continue; }
            let mut tris: Vec<[[f32; 3]; 3]> = Vec::new();
            if pts.len() == 3 { tris.push([pts[0], pts[1], pts[2]]); }
            else { for i in 2..pts.len() { tris.push([pts[1], pts[i], pts[(i+1) % pts.len()]]); } }
            for tri in &tris {
                let h = [[tri[0][0]*0.5+d[0], tri[0][1]*0.5+d[1], tri[0][2]*0.5+d[2]],
                         [tri[1][0]*0.5+d[0], tri[1][1]*0.5+d[1], tri[1][2]*0.5+d[2]],
                         [tri[2][0]*0.5+d[0], tri[2][1]*0.5+d[1], tri[2][2]*0.5+d[2]]];
                let mut k = [mk(&h[0]), mk(&h[1]), mk(&h[2])];
                k.sort();
                mmap.entry(k).or_default().push(*tri);
            }
        }
    }
    let hd = std::fs::read(&a[1]).unwrap();
    let hf = mapgeom::static_item::file::parse_file(&hd).unwrap();
    let so = hf.item.static_object().unwrap();
    let s2 = so.solid2().unwrap();
    let (mut a_hit, mut e_hit, mut tot) = (0, 0, 0);
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
                    for cc in 0..3 {
                        if p[cc][1].abs() > 0.01 { continue; } // near-zero Y only
                        tot += 1;
                        let mut k = [mk(&p[0]), mk(&p[1]), mk(&p[2])];
                        // find src via tri match
                        let _ = k;
                        // global nearest source on Y
                        // (use axis-1 exact-src formula compare)
                    }
                }
            }
        }
    }
    // simpler: global nearest-source assignment for near-zero-Y his verts
    let mut svals: Vec<f32> = Vec::new();
    for (cr, vis, _) in &layers {
        if !vis { continue; }
        for f in &cr.faces {
            for vi in &f.verts {
                let s = cr.positions[*vi as usize][1];
                svals.push(((s as f64 * 0.5 + t64[1]) as f32));
            }
        }
    }
    // also f32 path values
    let mut svals32: Vec<f32> = Vec::new();
    for (cr, vis, _) in &layers {
        if !vis { continue; }
        for f in &cr.faces {
            for vi in &f.verts {
                let s = cr.positions[*vi as usize][1];
                svals32.push(s * 0.5f32 + t32[1]);
            }
        }
    }
    let mut hvals: Vec<f32> = Vec::new();
    for gg in &s2.shaded_geoms {
        let vi = gg.visual_index.max(0) as usize;
        if let Some(vref) = s2.visuals.get(vi) {
            if let Some(mapgeom::static_item::Node::Visual(vis)) = vref.inline.as_deref() {
                let st = vis.stream().unwrap();
                for (dd2, e) in st.decls.iter().zip(st.elems.iter()) {
                    if let Elem::Float3(p) = e {
                        if dd2.name() == 0 {
                            for pp in p {
                                if pp[1].abs() < 0.01 { hvals.push(pp[1]); }
                            }
                        }
                    }
                }
            }
        }
    }
    // for each his near-zero-Y: nearest in svals (f64 path) vs svals32 (f32 path); bit-exact?
    let (mut a64, mut a32) = (0, 0);
    for h in &hvals {
        let mut b64 = f32::MAX;
        let mut b32 = f32::MAX;
        let mut e64 = false;
        let mut e32 = false;
        for s in &svals {
            let dd = (s - h).abs();
            if dd < b64 { b64 = dd; }
            if s.to_bits() == h.to_bits() { e64 = true; }
        }
        for s in &svals32 {
            let dd = (s - h).abs();
            if dd < b32 { b32 = dd; }
            if s.to_bits() == h.to_bits() { e32 = true; }
        }
        if e64 { a64 += 1; }
        if e32 { a32 += 1; }
        let _ = (b64, b32);
    }
    println!("near-zero-Y his verts={} f64path_bitexact={a64} f32path_bitexact={a32}", hvals.len());
    let _ = (a_hit, e_hit, tot);
}
