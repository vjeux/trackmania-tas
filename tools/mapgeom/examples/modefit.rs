//! Fit his quant mode: uniform float -> his words under modes. Usage: modefit HIS.ITEM MINE.ITEM SUBSTR
use std::collections::BTreeMap;
use mapgeom::static_item::vstream::Elem;
fn dec(v: u32) -> [f32; 3] {
    let x = ((v & 0x3FF) as i32) << 22 >> 22;
    let y = ((v >> 10 & 0x3FF) as i32) << 22 >> 22;
    let z = ((v >> 20 & 0x3FF) as i32) << 22 >> 22;
    [x as f32 / 511.0, y as f32 / 511.0, z as f32 / 511.0]
}
fn pw(v: &[u32]) -> [i32; 3] {
    // raw dec3n words of a Word elem value triple? We compare per-component words from stored u32.
    let _ = v;
    [0, 0, 0]
}
fn main() {
    let a: Vec<String> = std::env::args().collect();
    // load his + my N words (raw u32) and my faces
    let loadw = |path: &str| -> (BTreeMap<[u32;3], Vec<u32>>, BTreeMap<[u32;3], Vec<[[f32;3];3]>>) {
        let data = std::fs::read(path).unwrap();
        let f = mapgeom::static_item::file::parse_file(&data).unwrap();
        let so = f.item.static_object().unwrap();
        let s2 = so.solid2().unwrap();
        let mut words: BTreeMap<[u32;3], Vec<u32>> = BTreeMap::new();
        let mut tris: BTreeMap<[u32;3], Vec<[[f32;3];3]>> = BTreeMap::new();
        // need raw words: re-read Word elems
        for g in &s2.shaded_geoms {
            let vi = g.visual_index.max(0) as usize;
            let mi = g.material_index.max(0) as usize;
            let mat = s2.custom_materials.get(mi).and_then(|m| m.inst()).map(|i| i.link().unwrap_or("").to_string()).unwrap_or("".into());
            let stem = mat.rsplit('\\').next().unwrap_or(&mat).to_string();
            if stem != a[3] { continue; }
            if let Some(vref) = s2.visuals.get(vi) {
                if let Some(mapgeom::static_item::Node::Visual(vis)) = vref.inline.as_deref() {
                    let st = vis.stream().unwrap();
                    let (mut pos, mut nrmw) = (Vec::new(), Vec::new());
                    for (d, e) in st.decls.iter().zip(st.elems.iter()) {
                        match e {
                            Elem::Float3(p) if d.name() == 0 => pos = p.clone(),
                            Elem::Word(w) if d.name() == 5 => nrmw = w.clone(),
                            _ => {}
                        }
                    }
                    for i in 0..pos.len() {
                        words.entry([pos[i][0].to_bits(), pos[i][1].to_bits(), pos[i][2].to_bits()]).or_default().push(nrmw[i]);
                    }
                    let idx = vis.index_buffer.as_ref().map(|b| b.indices.clone()).unwrap_or_default();
                    for t in idx.chunks(3) {
                        if t.len() < 3 { continue; }
                        let ps = [pos[t[0] as usize], pos[t[1] as usize], pos[t[2] as usize]];
                        // associate tri with each of its corners' positions? For uniform we need incident faces per position.
                        for k in 0..3 {
                            tris.entry([ps[k][0].to_bits(), ps[k][1].to_bits(), ps[k][2].to_bits()]).or_default().push(ps);
                        }
                    }
                }
            }
        }
        (words, tris)
    };
    let (hw, _) = loadw(&a[1]);
    let (mw, mtris) = loadw(&a[2]);
    // modes: round, floor, ceil, trunc
    let qmode = |x: f32, mode: usize| -> i32 {
        let v = (x.clamp(-1.0, 1.0) * 511.0) as f64;
        match mode {
            0 => v.round() as i32,
            1 => v.floor() as i32,
            2 => v.ceil() as i32,
            3 => if v >= 0.0 { v.floor() as i32 } else { v.ceil() as i32 },
            _ => v.round() as i32,
        }
    };
    let mut hit = [0usize; 4];
    let mut tot = 0;
    for (k, hv) in &hw {
        if hv.len() != 1 { continue; }
        if mw.get(k).map(|v| v.len()).unwrap_or(0) != 1 { continue; }
        let tris = match mtris.get(k) { Some(t) => t, None => continue };
        // uniform over unique tris (dedupe: same tri recorded 1-3x, once per corner at p; use unique by sorted corners)
        let mut seen = std::collections::BTreeSet::new();
        let mut acc = [0.0f64; 3];
        let mut nf = 0;
        for ps in tris {
            let mut ck = [((ps[0][0]*1000.0).round() as i32, (ps[0][1]*1000.0).round() as i32, (ps[0][2]*1000.0).round() as i32),
                ((ps[1][0]*1000.0).round() as i32, (ps[1][1]*1000.0).round() as i32, (ps[1][2]*1000.0).round() as i32),
                ((ps[2][0]*1000.0).round() as i32, (ps[2][1]*1000.0).round() as i32, (ps[2][2]*1000.0).round() as i32)];
            ck.sort();
            if !seen.insert(ck) { continue; }
            let e1 = [ps[1][0]-ps[0][0], ps[1][1]-ps[0][1], ps[1][2]-ps[0][2]];
            let e2 = [ps[2][0]-ps[0][0], ps[2][1]-ps[0][1], ps[2][2]-ps[0][2]];
            let cr = [e1[1]*e2[2]-e1[2]*e2[1], e1[2]*e2[0]-e1[0]*e2[2], e1[0]*e2[1]-e1[1]*e2[0]];
            let l = (cr[0]*cr[0]+cr[1]*cr[1]+cr[2]*cr[2]).sqrt().max(1e-30);
            for d in 0..3 { acc[d] += (cr[d]/l) as f64; }
            nf += 1;
        }
        if nf == 0 { continue; }
        let l = (acc[0]*acc[0]+acc[1]*acc[1]+acc[2]*acc[2]).sqrt().max(1e-30);
        let un = [(acc[0]/l) as f32, (acc[1]/l) as f32, (acc[2]/l) as f32];
        // his raw words
        let hword = hv[0];
        let hx = ((hword & 0x3FF) as i32) << 22 >> 22;
        let hy = ((hword >> 10 & 0x3FF) as i32) << 22 >> 22;
        let hz = ((hword >> 20 & 0x3FF) as i32) << 22 >> 22;
        for mode in 0..4 {
            let q = [qmode(un[0], mode), qmode(un[1], mode), qmode(un[2], mode)];
            if q == [hx, hy, hz] { hit[mode] += 1; }
        }
        tot += 1;
    }
    println!("{}: 1-1={tot} round={} floor={} ceil={} trunc={}", a[3], hit[0], hit[1], hit[2], hit[3]);
    let _ = (dec, pw);
}
