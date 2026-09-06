//! N bit-exact on unambiguous (1-1 exact-bit) positions. Usage: n1to1 HIS.ITEM MINE.ITEM SUBSTR
use std::collections::BTreeMap;
use mapgeom::static_item::vstream::Elem;
fn dec(v: u32) -> [f32; 3] {
    let x = ((v & 0x3FF) as i32) << 22 >> 22;
    let y = ((v >> 10 & 0x3FF) as i32) << 22 >> 22;
    let z = ((v >> 20 & 0x3FF) as i32) << 22 >> 22;
    [x as f32 / 511.0, y as f32 / 511.0, z as f32 / 511.0]
}
fn main() {
    let a: Vec<String> = std::env::args().collect();
    let load = |path: &str| -> BTreeMap<[u32;3], Vec<([f32;3],[f32;3],[f32;3])>> {
        let data = std::fs::read(path).unwrap();
        let f = mapgeom::static_item::file::parse_file(&data).unwrap();
        let so = f.item.static_object().unwrap();
        let s2 = so.solid2().unwrap();
        let mut out: BTreeMap<[u32;3], Vec<([f32;3],[f32;3],[f32;3])>> = BTreeMap::new();
        for g in &s2.shaded_geoms {
            let vi = g.visual_index.max(0) as usize;
            let mi = g.material_index.max(0) as usize;
            let mat = s2.custom_materials.get(mi).and_then(|m| m.inst()).map(|i| i.link().unwrap_or("").to_string()).unwrap_or("".into());
            let stem = mat.rsplit('\\').next().unwrap_or(&mat).to_string();
            if stem != a[3] { continue; }
            if let Some(vref) = s2.visuals.get(vi) {
                if let Some(mapgeom::static_item::Node::Visual(vis)) = vref.inline.as_deref() {
                    let st = vis.stream().unwrap();
                    let (mut pos, mut nrm, mut tu, mut tv) = (Vec::new(), Vec::new(), Vec::new(), Vec::new());
                    for (d, e) in st.decls.iter().zip(st.elems.iter()) {
                        match e {
                            Elem::Float3(p) if d.name() == 0 => pos = p.clone(),
                            Elem::Word(w) if d.name() == 5 => nrm = w.iter().map(|v| dec(*v)).collect(),
                            Elem::Word(w) if d.name() == 18 => tu = w.iter().map(|v| dec(*v)).collect(),
                            Elem::Word(w) if d.name() == 20 => tv = w.iter().map(|v| dec(*v)).collect(),
                            _ => {}
                        }
                    }
                    for i in 0..pos.len() {
                        let u = if i < tu.len() { tu[i] } else { [0.0;3] };
                        let v = if i < tv.len() { tv[i] } else { [0.0;3] };
                        out.entry([pos[i][0].to_bits(), pos[i][1].to_bits(), pos[i][2].to_bits()]).or_default().push((nrm[i], u, v));
                    }
                }
            }
        }
        out
    };
    let r = load(&a[1]);
    let m = load(&a[2]);
    let (mut n11, mut n11ok, mut u11ok, mut v11ok, mut dot_sum, mut maxang) = (0usize, 0usize, 0usize, 0usize, 0.0f64, 0.0f32);
    for (k, rvs) in &r {
        if rvs.len() != 1 { continue; }
        if let Some(mvs) = m.get(k) {
            if mvs.len() != 1 { continue; }
            n11 += 1;
            let (rn, mn) = (rvs[0].0, mvs[0].0);
            if rn[0].to_bits()==mn[0].to_bits() && rn[1].to_bits()==mn[1].to_bits() && rn[2].to_bits()==mn[2].to_bits() { n11ok += 1; }
            let (ru, mu) = (rvs[0].1, mvs[0].1);
            if ru[0].to_bits()==mu[0].to_bits() && ru[1].to_bits()==mu[1].to_bits() && ru[2].to_bits()==mu[2].to_bits() { u11ok += 1; }
            let (rv, mv) = (rvs[0].2, mvs[0].2);
            if rv[0].to_bits()==mv[0].to_bits() && rv[1].to_bits()==mv[1].to_bits() && rv[2].to_bits()==mv[2].to_bits() { v11ok += 1; }
            let d = (rn[0]*mn[0]+rn[1]*mn[1]+rn[2]*mn[2]).clamp(-1.0,1.0).acos().to_degrees();
            dot_sum += d as f64;
            maxang = maxang.max(d);
        }
    }
    println!("{}: 1-1 pos n={n11} Nbit={n11ok} ({:.1}%) Ubit={u11ok} ({:.1}%) Vbit={v11ok} ({:.1}%) meanNang={:.4}deg maxNang={:.2}deg",
        a[3], 100.0*n11ok as f32/n11.max(1) as f32, 100.0*u11ok as f32/n11.max(1) as f32, 100.0*v11ok as f32/n11.max(1) as f32, dot_sum/n11.max(1) as f64, maxang);
}
