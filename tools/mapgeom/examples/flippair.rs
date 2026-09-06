//! Are flips paired (both tris of quad)? Usage: flippair HIS.ITEM MINE.ITEM SRCFILE
use std::collections::{BTreeMap, BTreeSet};
use mapgeom::static_item::bake::geometry_layers;
use mapgeom::static_item::vstream::Elem;
fn mk(p: &[f32; 3]) -> (i32, i32, i32) {
    ((p[0]*1000.0).round() as i32, (p[1]*1000.0).round() as i32, (p[2]*1000.0).round() as i32)
}
fn sub(a: [f32; 3], b: [f32; 3]) -> [f32; 3] { [a[0]-b[0], a[1]-b[1], a[2]-b[2]] }
fn cross(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[1]*b[2]-a[2]*b[1], a[2]*b[0]-a[0]*b[2], a[0]*b[1]-a[1]*b[2]]
}
fn main() {
    let a: Vec<String> = std::env::args().collect();
    let t = [-0.0019760131836f32, -0.0000076293945312, -0.023214340210];
    let load = |path: &str| -> Vec<Vec<[f32; 3]>> {
        let data = std::fs::read(path).unwrap();
        let f = mapgeom::static_item::file::parse_file(&data).unwrap();
        let so = f.item.static_object().unwrap();
        let s2 = so.solid2().unwrap();
        let mut out = Vec::new();
        for g in &s2.shaded_geoms {
            let vi = g.visual_index.max(0) as usize;
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
                    for tt in idx.chunks(3) {
                        if tt.len() < 3 { continue; }
                        out.push(vec![pos[tt[0] as usize], pos[tt[1] as usize], pos[tt[2] as usize]]);
                    }
                }
            }
        }
        out
    };
    let r = load(&a[1]);
    let m = load(&a[2]);
    let mut mmap: BTreeMap<[(i32, i32, i32); 3], Vec<usize>> = BTreeMap::new();
    for (i, tt) in m.iter().enumerate() {
        let mut k = [mk(&tt[0]), mk(&tt[1]), mk(&tt[2])];
        k.sort();
        mmap.entry(k).or_default().push(i);
    }
    let mut flipkeys: BTreeSet<[(i32, i32, i32); 3]> = BTreeSet::new();
    for tt in &r {
        let mut k = [mk(&tt[0]), mk(&tt[1]), mk(&tt[2])];
        k.sort();
        if let Some(v) = mmap.get(&k) {
            let u = &m[v[0]];
            let mut perm = [0, 1, 2];
            'outer: for cand in [[0, 1, 2], [0, 2, 1], [1, 0, 2], [1, 2, 0], [2, 0, 1], [2, 1, 0]] {
                let mut ok = true;
                for cc in 0..3 {
                    if (u[cand[cc]][0]-tt[cc][0]).abs() > 0.002 || (u[cand[cc]][1]-tt[cc][1]).abs() > 0.002 || (u[cand[cc]][2]-tt[cc][2]).abs() > 0.002 {
                        ok = false; break;
                    }
                }
                if ok { perm = cand; break 'outer; }
            }
            let fnr = cross(sub(tt[1], tt[0]), sub(tt[2], tt[0]));
            let fnm = cross(sub(u[perm[1]], u[perm[0]]), sub(u[perm[2]], u[perm[0]]));
            let lr = (fnr[0]*fnr[0]+fnr[1]*fnr[1]+fnr[2]*fnr[2]).sqrt();
            let lm = (fnm[0]*fnm[0]+fnm[1]*fnm[1]+fnm[2]*fnm[2]).sqrt();
            if lr < 1e-15 || lm < 1e-15 { continue; }
            if (fnr[0]*fnm[0]+fnr[1]*fnm[1]+fnr[2]*fnm[2])/(lr*lm) < 0.0 {
                flipkeys.insert(k);
            }
        }
    }
    // quad mates from source (correct diagonal v1,v3)
    let data = std::fs::read(&a[3]).unwrap();
    let g = tmmaps::gbx::Gbx::parse(&data);
    let loc = mapgeom::crystal_model::locate(&g.body).unwrap();
    let (c, _, _) = mapgeom::crystal_model::CPlugCrystal::parse_with(&g.body, loc.at, loc.lookback.clone()).unwrap();
    let layers = geometry_layers(&c);
    let mut mate: BTreeMap<[(i32, i32, i32); 3], [(i32, i32, i32); 3]> = BTreeMap::new();
    for (cr, vis, _) in &layers {
        if !vis { continue; }
        for f in &cr.faces {
            if f.verts.len() != 4 { continue; }
            let pts: Vec<[f32; 3]> = f.verts.iter().map(|i| cr.positions[*i as usize]).collect();
            let mut ks = Vec::new();
            for tri in [[pts[1], pts[2], pts[3]], [pts[1], pts[3], pts[0]]] {
                let h = [[tri[0][0]*0.5+t[0], tri[0][1]*0.5+t[1], tri[0][2]*0.5+t[2]],
                         [tri[1][0]*0.5+t[0], tri[1][1]*0.5+t[1], tri[1][2]*0.5+t[2]],
                         [tri[2][0]*0.5+t[0], tri[2][1]*0.5+t[1], tri[2][2]*0.5+t[2]]];
                let mut k = [mk(&h[0]), mk(&h[1]), mk(&h[2])];
                k.sort();
                ks.push(k);
            }
            mate.insert(ks[0], ks[1]);
            mate.insert(ks[1], ks[0]);
        }
    }
    let (mut paired, mut single) = (0, 0);
    for k in &flipkeys {
        match mate.get(k) {
            Some(mk2) if flipkeys.contains(mk2) => paired += 1,
            _ => single += 1,
        }
    }
    println!("flips={} paired(both mate flipped)={paired} single={single}", flipkeys.len());
}
