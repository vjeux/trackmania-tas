//! Tri-matched vertex-partition diff. Matches HIS and MINE triangles 1:1 by
//! their (bit-exact) position triple, then at every position compares how
//! the incident corners are grouped into vertices (the weld partition).
//! Reports positions whose partitions differ, with per-group N/U/V values and
//! the pairwise face-normal angles, so the split rule can be read off.
//! Usage: partdiff HIS.ITEM MINE.ITEM STEM [max_positions]
use mapgeom::static_item::vstream::Elem;
use std::collections::{BTreeMap, BTreeSet};

fn dec(v: u32) -> [f32; 3] {
    let x = ((v & 0x3FF) as i32) << 22 >> 22;
    let y = ((v >> 10 & 0x3FF) as i32) << 22 >> 22;
    let z = ((v >> 20 & 0x3FF) as i32) << 22 >> 22;
    [x as f32 / 511.0, y as f32 / 511.0, z as f32 / 511.0]
}
fn norm(v: [f32; 3]) -> [f32; 3] {
    let l = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt().max(1e-30);
    [v[0] / l, v[1] / l, v[2] / l]
}
fn ang(a: [f32; 3], b: [f32; 3]) -> f32 {
    let a = norm(a);
    let b = norm(b);
    let d = (a[0] * b[0] + a[1] * b[1] + a[2] * b[2]).clamp(-1.0, 1.0);
    d.acos().to_degrees()
}
fn face_normal(p: &[[f32; 3]; 3]) -> [f32; 3] {
    let e1 = [p[1][0] - p[0][0], p[1][1] - p[0][1], p[1][2] - p[0][2]];
    let e2 = [p[2][0] - p[0][0], p[2][1] - p[0][1], p[2][2] - p[0][2]];
    norm([
        e1[1] * e2[2] - e1[2] * e2[1],
        e1[2] * e2[0] - e1[0] * e2[2],
        e1[0] * e2[1] - e1[1] * e2[0],
    ])
}

struct Mesh {
    pos: Vec<[f32; 3]>,
    nrm: Vec<[f32; 3]>,
    uv: Vec<[f32; 2]>,
    uv1: Vec<[f32; 2]>,
    tu: Vec<[f32; 3]>,
    tv: Vec<[f32; 3]>,
    tris: Vec<[u32; 3]>,
}

fn load(path: &str, stem_want: &str) -> Mesh {
    let data = std::fs::read(path).unwrap();
    let f = mapgeom::static_item::file::parse_file(&data).unwrap();
    let so = f.item.static_object().unwrap();
    let s2 = so.solid2().unwrap();
    let mut m = Mesh { pos: vec![], nrm: vec![], uv: vec![], uv1: vec![], tu: vec![], tv: vec![], tris: vec![] };
    for g in &s2.shaded_geoms {
        let vi = g.visual_index.max(0) as usize;
        let mi = g.material_index.max(0) as usize;
        let mat = s2.custom_materials.get(mi).and_then(|m| m.inst()).map(|i| i.link().unwrap_or("").to_string()).unwrap_or("".into());
        let stem = mat.rsplit('\\').next().unwrap_or(&mat).to_string();
        if stem != stem_want {
            continue;
        }
        if let Some(vref) = s2.visuals.get(vi) {
            if let Some(mapgeom::static_item::Node::Visual(vis)) = vref.inline.as_deref() {
                let st = vis.stream().unwrap();
                for (d, e) in st.decls.iter().zip(st.elems.iter()) {
                    match e {
                        Elem::Float3(p) if d.name() == 0 => m.pos = p.clone(),
                        Elem::Word(w) if d.name() == 5 => m.nrm = w.iter().map(|v| dec(*v)).collect(),
                        Elem::Float2(u) if d.name() == 10 => m.uv = u.clone(),
                        Elem::Float2(u) if d.name() == 11 => m.uv1 = u.clone(),
                        Elem::Word(w) if d.name() == 18 => m.tu = w.iter().map(|v| dec(*v)).collect(),
                        Elem::Word(w) if d.name() == 20 => m.tv = w.iter().map(|v| dec(*v)).collect(),
                        _ => {}
                    }
                }
                let idx = vis.index_buffer.as_ref().map(|b| b.indices.clone()).unwrap_or_default();
                for t in idx.chunks(3) {
                    if t.len() == 3 {
                        m.tris.push([t[0], t[1], t[2]]);
                    }
                }
                return m;
            }
        }
    }
    m
}

/// Source crystal: transformed position -> vertex index, vertex -> faces,
/// face -> (material, group). Transform = f32(src*0.5) + TINY_POS_T.
struct Src {
    pos2v: BTreeMap<[u32; 3], u32>,
    v2f: BTreeMap<u32, Vec<usize>>,
    face_mat: Vec<i32>,
    face_group: Vec<u32>,
    face_nverts: Vec<usize>,
    group_parent: Vec<i32>,
    group_name: Vec<String>,
}
fn load_crystal(path: &str) -> Src {
    let data = std::fs::read(path).unwrap();
    let g = tmmaps::gbx::Gbx::parse(&data);
    let loc = mapgeom::crystal_model::locate(&g.body).unwrap();
    let (c, _, _) = mapgeom::crystal_model::CPlugCrystal::parse_with(&g.body, loc.at, loc.lookback.clone()).unwrap();
    let t: [f32; 3] = std::env::var("TINY_POS_T")
        .ok()
        .and_then(|s| {
            let w: Vec<&str> = s.split(',').collect();
            if w.len() != 3 {
                return None;
            }
            let mut t = [0f32; 3];
            for (i, x) in w.iter().enumerate() {
                t[i] = f32::from_bits(u32::from_str_radix(x.trim(), 16).ok()?);
            }
            Some(t)
        })
        .unwrap_or([0.0; 3]);
    let layers = mapgeom::static_item::bake::geometry_layers(&c);
    let (cr, _, _) = layers.iter().find(|(_, vis, _)| *vis).unwrap();
    let mut s = Src { pos2v: BTreeMap::new(), v2f: BTreeMap::new(), face_mat: vec![], face_group: vec![], face_nverts: vec![], group_parent: vec![], group_name: vec![] };
    for (i, p) in cr.positions.iter().enumerate() {
        let q = [p[0] * 0.5 + t[0], p[1] * 0.5 + t[1], p[2] * 0.5 + t[2]];
        s.pos2v.insert(pk(q), i as u32);
    }
    for (fi, f) in cr.faces.iter().enumerate() {
        s.face_mat.push(f.material);
        s.face_group.push(f.group);
        s.face_nverts.push(f.verts.len());
        for v in &f.verts {
            s.v2f.entry(*v).or_default().push(fi);
        }
    }
    for gp in &cr.groups {
        s.group_parent.push(gp.u03);
        s.group_name.push(gp.name.clone());
    }
    s
}
impl Src {
    /// Source face containing all three corner positions (None if unmapped).
    fn face_of(&self, p: [[f32; 3]; 3]) -> Option<usize> {
        let vs: Vec<u32> = p.iter().map(|q| self.pos2v.get(&pk(*q)).copied()).collect::<Option<Vec<u32>>>()?;
        let fs = self.v2f.get(&vs[0])?;
        fs.iter().copied().find(|fi| {
            let f = *fi;
            vs[1..].iter().all(|v| self.v2f.get(v).map(|l| l.contains(&f)).unwrap_or(false))
        })
    }
}

fn pk(p: [f32; 3]) -> [u32; 3] {
    [p[0].to_bits(), p[1].to_bits(), p[2].to_bits()]
}
fn hx(p: [u32; 3]) -> String {
    format!("{:x}{:x}{:x}", p[0], p[1], p[2])
}
fn f3(v: [f32; 3]) -> String {
    format!("({:+.3},{:+.3},{:+.3})", v[0], v[1], v[2])
}

// Fit HIS U for UV-degenerate (|det|<1e-12) tris. Pairs HIS/MINE tris
// 1:1 by sorted position triple (same as partdiff), then for every MINE
// tri with |det|<1e-12 prints HIS U at each corner plus min-angles to
// candidate frames. Usage: detfit HIS.ITEM MINE.ITEM STEM [MAXTRIS]
fn isopair2(us: &[[f32; 2]; 3]) -> Option<(usize, usize)> {
    for (a, b) in [(0usize, 1usize), (1, 2), (2, 0)] {
        if us[a][0] == us[b][0] && us[a][1] == us[b][1] {
            return Some((a, b));
        }
    }
    None
}
fn gs(v: [f32; 3], n: [f32; 3]) -> [f32; 3] {
    let d = v[0] * n[0] + v[1] * n[1] + v[2] * n[2];
    norm([v[0] - d * n[0], v[1] - d * n[1], v[2] - d * n[2]])
}
fn mang(a: [f32; 3], b: [f32; 3]) -> f32 {
    let d = ang(a, b);
    d.min(180.0 - d)
}
fn main() {
    let a: Vec<String> = std::env::args().collect();
    let max_show: usize = a.get(4).and_then(|s| s.parse().ok()).unwrap_or(40);
    let his = load(&a[1], &a[3]);
    let mine = load(&a[2], &a[3]);
    let tkey = |m: &Mesh, t: &[u32; 3]| -> [[u32; 3]; 3] {
        let mut k = [pk(m.pos[t[0] as usize]), pk(m.pos[t[1] as usize]), pk(m.pos[t[2] as usize])];
        k.sort();
        k
    };
    let mut his_by_key: BTreeMap<[[u32; 3]; 3], Vec<usize>> = BTreeMap::new();
    for (i, t) in his.tris.iter().enumerate() {
        his_by_key.entry(tkey(&his, t)).or_default().push(i);
    }
    let mut m2h: Vec<Option<usize>> = vec![None; mine.tris.len()];
    for (i, t) in mine.tris.iter().enumerate() {
        if let Some(v) = his_by_key.get_mut(&tkey(&mine, t)) {
            if let Some(h) = v.pop() {
                m2h[i] = Some(h);
                continue;
            }
        }
    }
    let ups = [[0.0, 1.0, 0.0], [0.0, 0.0, 1.0], [1.0, 0.0, 0.0]];
    let upnames = ["upYxN", "upZxN", "upXxN"];
    let mut shown = 0;
    for (i, t) in mine.tris.iter().enumerate() {
        if shown >= max_show {
            break;
        }
        let ps = [mine.pos[t[0] as usize], mine.pos[t[1] as usize], mine.pos[t[2] as usize]];
        let us = [mine.uv[t[0] as usize], mine.uv[t[1] as usize], mine.uv[t[2] as usize]];
        let e1 = [ps[1][0] - ps[0][0], ps[1][1] - ps[0][1], ps[1][2] - ps[0][2]];
        let e2 = [ps[2][0] - ps[0][0], ps[2][1] - ps[0][1], ps[2][2] - ps[0][2]];
        let (du1, dv1, du2, dv2) = (us[1][0] - us[0][0], us[1][1] - us[0][1], us[2][0] - us[0][0], us[2][1] - us[0][1]);
        let det = du1 * dv2 - du2 * dv1;
        if det.abs() >= 1e-12 {
            continue;
        }
        shown += 1;
        let h = m2h[i];
        // duplicate-triple census + uv compare (overlap forensics)
        {
            let tk = tkey(&mine, t);
            let mut nh = 0;
            for (j, ht) in his.tris.iter().enumerate() {
                if tkey(&his, ht) == tk {
                    nh += 1;
                    let huv: Vec<String> = (0..3).map(|c| format!("({:.4},{:.4})", his.uv[ht[c] as usize][0], his.uv[ht[c] as usize][1])).collect();
                    let hu: Vec<String> = (0..3).map(|c| format!("({:+.2},{:+.2},{:+.2})", his.tu[ht[c] as usize][0], his.tu[ht[c] as usize][1], his.tu[ht[c] as usize][2])).collect();
                    println!("    his-tri{j} uv={} U={}", huv.join(""), hu.join(""));
                }
            }
            let mut nm = 0;
            for mt in mine.tris.iter() {
                if tkey(&mine, mt) == tk {
                    nm += 1;
                }
            }
            let muv: Vec<String> = (0..3).map(|c| format!("({:.4},{:.4})", us[c][0], us[c][1])).collect();
            println!("    triple census: his={} mine={} mine-uv={}", nh, nm, muv.join(""));
        }
        println!("minetri{i} det={det:.2e} paired={}", h.is_some());
        for k in 0..3 {
            let n = mine.nrm[t[k] as usize];
            // candidates (directions, compared min-angle = directionless)
            let mut cands: Vec<(&str, [f32; 3])> = Vec::new();
            for (un, up) in upnames.iter().zip(ups.iter()) {
                cands.push((un, norm([up[1] * n[2] - up[2] * n[1], up[2] * n[0] - up[0] * n[2], up[0] * n[1] - up[1] * n[0]])));
            }
            let dun = [e1[0] * dv2 - e2[0] * dv1, e1[1] * dv2 - e2[1] * dv1, e1[2] * dv2 - e2[2] * dv1];
            let dvn = [e1[0] * du2 - e2[0] * du1, e1[1] * du2 - e2[1] * du1, e1[2] * du2 - e2[2] * du1];
            cands.push(("dunumGS", gs(dun, n)));
            cands.push(("dvnumGS", gs(dvn, n)));
            let ex = [ps[(k + 1) % 3][0] - ps[k][0], ps[(k + 1) % 3][1] - ps[k][1], ps[(k + 1) % 3][2] - ps[k][2]];
            let ey = [ps[(k + 2) % 3][0] - ps[k][0], ps[(k + 2) % 3][1] - ps[k][1], ps[(k + 2) % 3][2] - ps[k][2]];
            cands.push(("edge1GS", gs(ex, n)));
            cands.push(("edge2GS", gs(ey, n)));
            // longest tri edge
            let l1 = e1[0] * e1[0] + e1[1] * e1[1] + e1[2] * e1[2];
            let e3 = [ps[2][0] - ps[1][0], ps[2][1] - ps[1][1], ps[2][2] - ps[1][2]];
            let l3 = e3[0] * e3[0] + e3[1] * e3[1] + e3[2] * e3[2];
            let l2 = e2[0] * e2[0] + e2[1] * e2[1] + e2[2] * e2[2];
            let le = if l1 >= l2 && l1 >= l3 { e1 } else if l2 >= l3 { e2 } else { e3 };
            cands.push(("longGS", gs(le, n)));
            // all 6 directed edges GS N (absolute, not from-corner)
            let eab = [ps[1][0] - ps[0][0], ps[1][1] - ps[0][1], ps[1][2] - ps[0][2]];
            let ebc = [ps[2][0] - ps[1][0], ps[2][1] - ps[1][1], ps[2][2] - ps[1][2]];
            let eca = [ps[0][0] - ps[2][0], ps[0][1] - ps[2][1], ps[0][2] - ps[2][2]];
            cands.push(("eAB", gs(eab, n)));
            cands.push(("eBC", gs(ebc, n)));
            cands.push(("eCA", gs(eca, n)));
            // tri 3D normal and N x trinormal
            let cx = [eab[1] * eca[2] - eab[2] * eca[1], eab[2] * eca[0] - eab[0] * eca[2], eab[0] * eca[1] - eab[1] * eca[0]];
            let tn = norm(cx);
            cands.push(("trin", tn));
            let nxn = [n[1] * tn[2] - n[2] * tn[1], n[2] * tn[0] - n[0] * tn[2], n[0] * tn[1] - n[1] * tn[0]];
            cands.push(("Nxtrin", norm(nxn)));
            // iso-uv edge (between uv-identical corners, if any): the
            // degenerate direction. Both GS and N-crossed.
            let mut iso: Option<[f32; 3]> = None;
            for (a, b) in [(0, 1), (1, 2), (2, 0)] {
                if us[a][0] == us[b][0] && us[a][1] == us[b][1] {
                    iso = Some([ps[b][0] - ps[a][0], ps[b][1] - ps[a][1], ps[b][2] - ps[a][2]]);
                }
            }
            if std::env::var("DETFIT_ISO").is_ok() {
                println!("  tri iso-pair uvs=({:08x},{:08x})({:08x},{:08x})({:08x},{:08x}) iso={:?} N={:?}",
                    us[0][0].to_bits(), us[0][1].to_bits(), us[1][0].to_bits(), us[1][1].to_bits(), us[2][0].to_bits(), us[2][1].to_bits(),
                    iso.map(|v| [v[0], v[1], v[2]]), n);
            }
            // pair membership per corner (for doubled-vs-non-doubled analysis)
            let mut inpair = [false; 3];
            if let Some((a, b)) = isopair2(&us) {
                inpair[a] = true;
                inpair[b] = true;
            }
            // down x N candidate (non-doubled-corner hypothesis)
            let down = [0.0f32, -1.0, 0.0];
            if let Some(iv) = iso {
                cands.push(("isoGS", gs(iv, n)));
                let cr = [n[1] * iv[2] - n[2] * iv[1], n[2] * iv[0] - n[0] * iv[2], n[0] * iv[1] - n[1] * iv[0]];
                cands.push(("Nxiso", norm(cr)));
            }
            // his U at same-position corners of paired tri
            let mut hus: Vec<[f32; 3]> = Vec::new();
            if let Some(h) = h {
                let ht = his.tris[h];
                let p = pk(ps[k]);
                for j in 0..3 {
                    if pk(his.pos[ht[j] as usize]) == p {
                        hus.push(his.tu[ht[j] as usize]);
                    }
                }
            }
            if std::env::var("DETFIT_DBG").is_ok() {
                if let Some(h) = h {
                    let ht = his.tris[h];
                    let hp: Vec<String> = (0..3).map(|c| format!("{:08x}{:08x}{:08x}", pk(his.pos[ht[c] as usize])[0], pk(his.pos[ht[c] as usize])[1], pk(his.pos[ht[c] as usize])[2])).collect();
                    println!("  k{k} query={:08x}{:08x}{:08x} pairedhis={h} hispos={}", pk(ps[k])[0], pk(ps[k])[1], pk(ps[k])[2], hp.join(" "));
                }
            }
            cands.push(("downxN", norm([down[1] * n[2] - down[2] * n[1], down[2] * n[0] - down[0] * n[2], down[0] * n[1] - down[1] * n[0]])));
            for hu in hus {
                let mut scored: Vec<(&str, f32)> = cands.iter().map(|(nm, c)| (*nm, mang(*c, hu))).collect();
                scored.sort_by(|x, y| x.1.partial_cmp(&y.1).unwrap());
                let top: Vec<String> = scored.iter().take(3).map(|(nm, d)| format!("{nm}={d:.1}")).collect();
                println!("  k{k}{} hisU=({:+.3},{:+.3},{:+.3}) best: {}", if inpair[k] { "[D]" } else { "[n]" }, hu[0], hu[1], hu[2], top.join(" "));
            }
        }
    }
    let _ = BTreeSet::<u32>::new();
}
