//! Weight-law test for N averages. Usage: wavtest HIS.ITEM MINE.ITEM SUBSTR
use std::collections::BTreeMap;
use mapgeom::static_item::vstream::Elem;
fn dec(v: u32) -> [f32; 3] {
    let x = ((v & 0x3FF) as i32) << 22 >> 22;
    let y = ((v >> 10 & 0x3FF) as i32) << 22 >> 22;
    let z = ((v >> 20 & 0x3FF) as i32) << 22 >> 22;
    [x as f32 / 511.0, y as f32 / 511.0, z as f32 / 511.0]
}
fn norm(v: [f32; 3]) -> [f32; 3] {
    let l = (v[0]*v[0]+v[1]*v[1]+v[2]*v[2]).sqrt().max(1e-30);
    [v[0]/l, v[1]/l, v[2]/l]
}
fn tang(a: [f32;3], b: [f32;3]) -> f32 {
    let la = (a[0]*a[0]+a[1]*a[1]+a[2]*a[2]).sqrt().max(1e-30);
    let lb = (b[0]*b[0]+b[1]*b[1]+b[2]*b[2]).sqrt().max(1e-30);
    ((a[0]*b[0]+a[1]*b[1]+a[2]*b[2])/(la*lb)).clamp(-1.0,1.0).acos().to_degrees()
}
fn main() {
    let a: Vec<String> = std::env::args().collect();
    let load = |path: &str| -> (BTreeMap<[u32;3], Vec<[f32;3]>>, Vec<[[f32;3];3]>) {
        let data = std::fs::read(path).unwrap();
        let f = mapgeom::static_item::file::parse_file(&data).unwrap();
        let so = f.item.static_object().unwrap();
        let s2 = so.solid2().unwrap();
        let mut map: BTreeMap<[u32;3], Vec<[f32;3]>> = BTreeMap::new();
        let mut tris: Vec<[[f32;3];3]> = Vec::new();
        for g in &s2.shaded_geoms {
            let vi = g.visual_index.max(0) as usize;
            let mi = g.material_index.max(0) as usize;
            let mat = s2.custom_materials.get(mi).and_then(|m| m.inst()).map(|i| i.link().unwrap_or("").to_string()).unwrap_or("".into());
            let stem = mat.rsplit('\\').next().unwrap_or(&mat).to_string();
            if stem != a[3] { continue; }
            if let Some(vref) = s2.visuals.get(vi) {
                if let Some(mapgeom::static_item::Node::Visual(vis)) = vref.inline.as_deref() {
                    let st = vis.stream().unwrap();
                    let (mut pos, mut nrm) = (Vec::new(), Vec::new());
                    for (d, e) in st.decls.iter().zip(st.elems.iter()) {
                        match e {
                            Elem::Float3(p) if d.name() == 0 => pos = p.clone(),
                            Elem::Word(w) if d.name() == 5 => nrm = w.iter().map(|v| dec(*v)).collect(),
                            _ => {}
                        }
                    }
                    for i in 0..pos.len() {
                        map.entry([pos[i][0].to_bits(), pos[i][1].to_bits(), pos[i][2].to_bits()]).or_default().push(nrm[i]);
                    }
                    let idx = vis.index_buffer.as_ref().map(|b| b.indices.clone()).unwrap_or_default();
                    for t in idx.chunks(3) {
                        if t.len() < 3 { continue; }
                        tris.push([pos[t[0] as usize], pos[t[1] as usize], pos[t[2] as usize]]);
                    }
                }
            }
        }
        (map, tris)
    };
    let (r, _) = load(&a[1]);
    let (m, tris) = load(&a[2]);
    let (mut n, mut uok, mut aok, mut wok) = (0, 0, 0, 0);
    let (mut usum, mut asum, mut wsum) = (0.0f64, 0.0f64, 0.0f64);
    let (mut awin, mut uwin) = (0, 0);
    for (k, rvs) in &r {
        if rvs.len() != 1 { continue; }
        if m.get(k).map(|v| v.len()).unwrap_or(0) != 1 { continue; }
        let p = [f32::from_bits(k[0]), f32::from_bits(k[1]), f32::from_bits(k[2])];
        let mut fns: Vec<([f32;3], f32, f32)> = Vec::new();
        for t in &tris {
            for ci in 0..3 {
                let d = ((t[ci][0]-p[0]).powi(2)+(t[ci][1]-p[1]).powi(2)+(t[ci][2]-p[2]).powi(2)).sqrt();
                if d > 1e-6 { continue; }
                let e1 = [t[1][0]-t[0][0], t[1][1]-t[0][1], t[1][2]-t[0][2]];
                let e2 = [t[2][0]-t[0][0], t[2][1]-t[0][1], t[2][2]-t[0][2]];
                let cr = [e1[1]*e2[2]-e1[2]*e2[1], e1[2]*e2[0]-e1[0]*e2[2], e1[0]*e2[1]-e1[1]*e2[0]];
                let l = (cr[0]*cr[0]+cr[1]*cr[1]+cr[2]*cr[2]).sqrt().max(1e-30);
                let o1 = (ci+1)%3;
                let o2 = (ci+2)%3;
                let v1 = [t[o1][0]-p[0], t[o1][1]-p[1], t[o1][2]-p[2]];
                let v2 = [t[o2][0]-p[0], t[o2][1]-p[1], t[o2][2]-p[2]];
                let l1 = (v1[0]*v1[0]+v1[1]*v1[1]+v1[2]*v1[2]).sqrt().max(1e-30);
                let l2 = (v2[0]*v2[0]+v2[1]*v2[1]+v2[2]*v2[2]).sqrt().max(1e-30);
                let an = ((v1[0]*v2[0]+v1[1]*v2[1]+v1[2]*v2[2])/(l1*l2)).clamp(-1.0,1.0).acos();
                fns.push(([cr[0]/l, cr[1]/l, cr[2]/l], l/2.0, an));
                break;
            }
        }
        if fns.len() < 2 { continue; }
        let avgw = |w: &dyn Fn(f32, f32) -> f64| {
            let mut acc = [0.0f64; 3];
            let mut s = 0.0f64;
            for (fn_, ar, an) in &fns {
                let ww = w(*ar, *an);
                for d in 0..3 { acc[d] += fn_[d] as f64 * ww; }
                s += ww;
            }
            norm([(acc[0]/s) as f32, (acc[1]/s) as f32, (acc[2]/s) as f32])
        };
        let un = avgw(&|_, _| 1.0);
        let an = avgw(&|ar, _| ar as f64);
        let wn = avgw(&|_, an_| an_ as f64);
        let (du, da, dw) = (tang(rvs[0], un), tang(rvs[0], an), tang(rvs[0], wn));
        n += 1;
        if du < 0.3 { uok += 1; }
        if da < 0.3 { aok += 1; }
        if dw < 0.3 { wok += 1; }
        usum += du as f64; asum += da as f64; wsum += dw as f64;
        if da + 0.05 < du { awin += 1; }
        if du + 0.05 < da { uwin += 1; }
    }
    println!("{}: 1-1={n} uniform ok={uok} ({:.1}%) mean={:.4}deg | area ok={aok} ({:.1}%) mean={:.4}deg | angle ok={wok} ({:.1}%) mean={:.4}deg | area_wins={awin} uniform_wins={uwin}",
        a[3], 100.0*uok as f32/n.max(1) as f32, usum/n.max(1) as f64,
        100.0*aok as f32/n.max(1) as f32, asum/n.max(1) as f64,
        100.0*wok as f32/n.max(1) as f32, wsum/n.max(1) as f64);
}
