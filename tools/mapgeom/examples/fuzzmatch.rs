//! Fuzzy vert match: greedy nearest (0.2mm), compare attr keys. Usage: fuzzmatch HIS.ITEM MINE.ITEM SUBSTR
use std::collections::BTreeMap;
use mapgeom::static_item::vstream::Elem;
fn dec(v: u32) -> [f32; 3] {
    let x = ((v & 0x3FF) as i32) << 22 >> 22;
    let y = ((v >> 10 & 0x3FF) as i32) << 22 >> 22;
    let z = ((v >> 20 & 0x3FF) as i32) << 22 >> 22;
    [x as f32 / 511.0, y as f32 / 511.0, z as f32 / 511.0]
}
struct V { p: [f32;3], k: Vec<u32> }
fn main() {
    let a: Vec<String> = std::env::args().collect();
    let load = |path: &str| -> Vec<V> {
        let data = std::fs::read(path).unwrap();
        let f = mapgeom::static_item::file::parse_file(&data).unwrap();
        let so = f.item.static_object().unwrap();
        let s2 = so.solid2().unwrap();
        let mut out = Vec::new();
        for g in &s2.shaded_geoms {
            let vi = g.visual_index.max(0) as usize;
            let mi = g.material_index.max(0) as usize;
            let mat = s2.custom_materials.get(mi).and_then(|m| m.inst()).map(|i| i.link().unwrap_or("").to_string()).unwrap_or("".into());
            let stem = mat.rsplit('\\').next().unwrap_or(&mat).to_string();
            if stem != a[3] { continue; }
            if let Some(vref) = s2.visuals.get(vi) {
                if let Some(mapgeom::static_item::Node::Visual(vis)) = vref.inline.as_deref() {
                    let st = vis.stream().unwrap();
                    let (mut pos, mut nrm, mut uv, mut uv1, mut tu, mut tv) = (Vec::new(), Vec::new(), Vec::new(), Vec::new(), Vec::new(), Vec::new());
                    let mut has_uv1 = false;
                    let mut has_tan = false;
                    for (d, e) in st.decls.iter().zip(st.elems.iter()) {
                        match e {
                            Elem::Float3(p) if d.name() == 0 => pos = p.clone(),
                            Elem::Word(w) if d.name() == 5 => nrm = w.iter().map(|v| dec(*v)).collect(),
                            Elem::Float2(u) if d.name() == 10 => uv = u.clone(),
                            Elem::Float2(u) if d.name() == 11 => { uv1 = u.clone(); has_uv1 = true; }
                            Elem::Word(w) if d.name() == 18 => { tu = w.iter().map(|v| dec(*v)).collect(); has_tan = true; }
                            Elem::Word(w) if d.name() == 20 => tv = w.iter().map(|v| dec(*v)).collect(),
                            _ => {}
                        }
                    }
                    for i in 0..pos.len() {
                        let mut k: Vec<u32> = vec![nrm[i][0].to_bits(), nrm[i][1].to_bits(), nrm[i][2].to_bits(),
                                            uv[i][0].to_bits(), uv[i][1].to_bits()];
                        if has_uv1 && i < uv1.len() { k.push(uv1[i][0].to_bits()); k.push(uv1[i][1].to_bits()); }
                        if has_tan && i < tu.len() {
                            k.push(tu[i][0].to_bits()); k.push(tu[i][1].to_bits()); k.push(tu[i][2].to_bits());
                            k.push(tv[i][0].to_bits()); k.push(tv[i][1].to_bits()); k.push(tv[i][2].to_bits());
                        }
                        out.push(V { p: pos[i], k });
                    }
                }
            }
        }
        out
    };
    let r = load(&a[1]);
    let mut m = load(&a[2]);
    // greedy: for each his vert, nearest unused my vert within 0.2mm
    let mut matched = 0;
    let mut attr_ok = 0;
    let mut posdev_sum = 0.0f64;
    let mut used = vec![false; m.len()];
    let mut hmatched = vec![false; r.len()];
    let dump = std::env::var("TINY_DUMP_UNMATCHED").is_ok();
    let dumpN = std::env::var("TINY_CMPN").is_ok();
    let (mut n_ok, mut uv_ok, mut u_ok, mut v_ok) = (0usize, 0usize, 0usize, 0usize);
    // spatial grid for speed (0.2mm cells)
    let mut grid: BTreeMap<(i64,i64,i64), Vec<usize>> = BTreeMap::new();
    for (j, v) in m.iter().enumerate() {
        grid.entry(((v.p[0]*5000.0).floor() as i64, (v.p[1]*5000.0).floor() as i64, (v.p[2]*5000.0).floor() as i64)).or_default().push(j);
    }
    for (hi, hv) in r.iter().enumerate() {
        let c = ((hv.p[0]*5000.0).floor() as i64, (hv.p[1]*5000.0).floor() as i64, (hv.p[2]*5000.0).floor() as i64);
        let mut best = (1e9f32, usize::MAX);
        for dx in -1..=1 { for dy in -1..=1 { for dz in -1..=1 {
            if let Some(js) = grid.get(&(c.0+dx, c.1+dy, c.2+dz)) {
                for &j in js {
                    if used[j] { continue; }
                    let d = ((hv.p[0]-m[j].p[0]).powi(2)+(hv.p[1]-m[j].p[1]).powi(2)+(hv.p[2]-m[j].p[2]).powi(2)).sqrt();
                    if d < best.0 { best = (d, j); }
                }
            }
        }}}
        if best.0 < 2e-4 && best.1 != usize::MAX {
            used[best.1] = true;
            hmatched[hi] = true;
            matched += 1;
            posdev_sum += best.0 as f64;
            if hv.k == m[best.1].k { attr_ok += 1; }
            // N-words only (first 3 words are decoded-normal bits? No: k holds decoded float bits.
            // For N compare, use raw words: recompute? (V has no raw.) Approximate via k[0..3] float-bit equality already in attr.
            // Instead count per-field: n_ok if k[0..3]==, uv_ok if k[3..5]==, uv1_ok, u_ok, v_ok.
            if dumpN {
                let a = &hv.k;
                let b = &m[best.1].k;
                if a.len() == b.len() && a.len() >= 5 {
                    if a[0..3] == b[0..3] { n_ok += 1; }
                    if a[3..5] == b[3..5] { uv_ok += 1; }
                    // U/V segments (Full/White len13: U[7..10] V[10..13]; Decal len11: U[5..8] V[8..11])
                    let (us, vs) = if a.len() == 13 { (7..10, 10..13) } else if a.len() == 11 { (5..8, 8..11) } else { (0..0, 0..0) };
                    if us.len() == 3 && a[us.clone()] == b[us.clone()] { u_ok += 1; }
                    if vs.len() == 3 && a[vs.clone()] == b[vs.clone()] { v_ok += 1; }
                }
            }
        }
    }
    let munused: usize = used.iter().filter(|u| !**u).count();
    println!("{}: his={} mine={} matched={} attr_exact={} ({:.1}%) unmatched_his={} unmatched_mine={} mean_dev={:.2e}",
        a[3], r.len(), m.len(), matched, attr_ok, 100.0*attr_ok as f32/matched.max(1) as f32,
        r.len()-matched, munused, posdev_sum/matched.max(1) as f64);
    if dumpN {
        println!("  Nwords_exact={} ({:.1}%) UV_exact={} ({:.1}%) U_exact={} ({:.1}%) V_exact={} ({:.1}%) of matched",
            n_ok, 100.0*n_ok as f32/matched.max(1) as f32, uv_ok, 100.0*uv_ok as f32/matched.max(1) as f32,
            u_ok, 100.0*u_ok as f32/matched.max(1) as f32, v_ok, 100.0*v_ok as f32/matched.max(1) as f32);
    }
    if dump {
        for (hi, hv) in r.iter().enumerate() {
            if !hmatched[hi] {
                println!("  HIS_ONLY p=({:.6},{:.6},{:.6})", hv.p[0], hv.p[1], hv.p[2]);
            }
        }
        for (j, v) in m.iter().enumerate() {
            if !used[j] {
                println!("  MINE_ONLY p=({:.6},{:.6},{:.6})", v.p[0], v.p[1], v.p[2]);
            }
        }
    }
}
