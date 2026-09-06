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

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let src = a.get(5).map(|p| load_crystal(p));
    let max_show: usize = a.get(4).and_then(|s| s.parse().ok()).unwrap_or(12);
    let his = load(&a[1], &a[3]);
    let mine = load(&a[2], &a[3]);
    println!("{}: his verts={} tris={} | mine verts={} tris={}", a[3], his.pos.len(), his.tris.len(), mine.pos.len(), mine.tris.len());

    // Tri key: sorted position triple.
    let tkey = |m: &Mesh, t: &[u32; 3]| -> [[u32; 3]; 3] {
        let mut k = [pk(m.pos[t[0] as usize]), pk(m.pos[t[1] as usize]), pk(m.pos[t[2] as usize])];
        k.sort();
        k
    };
    let mut his_by_key: BTreeMap<[[u32; 3]; 3], Vec<usize>> = BTreeMap::new();
    for (i, t) in his.tris.iter().enumerate() {
        his_by_key.entry(tkey(&his, t)).or_default().push(i);
    }
    // mine tri -> his tri
    let mut m2h: Vec<Option<usize>> = vec![None; mine.tris.len()];
    let mut unmatched = 0;
    for (i, t) in mine.tris.iter().enumerate() {
        if let Some(v) = his_by_key.get_mut(&tkey(&mine, t)) {
            if let Some(h) = v.pop() {
                m2h[i] = Some(h);
                continue;
            }
        }
        unmatched += 1;
    }
    let his_left: usize = his_by_key.values().map(|v| v.len()).sum();
    println!("tri match: mine unmatched={} his unmatched={}", unmatched, his_left);

    // corners per position: (mine tri, k) with his vertex + mine vertex.
    let mut by_pos: BTreeMap<[u32; 3], Vec<(usize, usize, u32, u32)>> = BTreeMap::new();
    for (i, t) in mine.tris.iter().enumerate() {
        let Some(h) = m2h[i] else { continue };
        let ht = his.tris[h];
        for k in 0..3 {
            let mv = t[k];
            let p = pk(mine.pos[mv as usize]);
            // find his corner with same position
            let hv = (0..3).map(|j| ht[j]).find(|&hv| pk(his.pos[hv as usize]) == p).unwrap();
            by_pos.entry(p).or_default().push((i, k, hv, mv));
        }
    }

    let mut n_pos = 0usize;
    let mut n_same = 0usize;
    let mut n_finer = 0usize; // mine splits more (mine partition refines his)
    let mut n_coarser = 0usize; // his refines mine
    let mut n_crossed = 0usize;
    let mut shown = 0usize;
    let mut sum_his = 0usize;
    let mut sum_mine = 0usize;
    // Hypothesis counters (need src): his partition == partition by source group / by source face.
    let mut n_grp_eq = 0usize;
    let mut n_grp_refines_his = 0usize; // grouping-by-group is finer than his
    let mut n_his_refines_grp = 0usize;
    let mut n_unmapped = 0usize;
    let (mut attr_n, mut attr_u, mut attr_v, mut attr_uv1, mut attr_none) = (0usize, 0usize, 0usize, 0usize, 0usize);
    let (mut corners_total, mut n_exact, mut u_exact, mut v_exact) = (0usize, 0usize, 0usize, 0usize);
    let b3f = |v: [f32; 3]| [v[0].to_bits(), v[1].to_bits(), v[2].to_bits()];
    for (p, corners) in &by_pos {
        n_pos += 1;
        let src_faces: Vec<Option<usize>> = corners
            .iter()
            .map(|c| {
                src.as_ref().and_then(|s| {
                    let t = mine.tris[c.0];
                    s.face_of([mine.pos[t[0] as usize], mine.pos[t[1] as usize], mine.pos[t[2] as usize]])
                })
            })
            .collect();
        if let Some(s) = &src {
            if src_faces.iter().any(|f| f.is_none()) {
                n_unmapped += 1;
            } else {
                let mut gmap: BTreeMap<u32, BTreeSet<usize>> = BTreeMap::new();
                let mut hmap2: BTreeMap<u32, BTreeSet<usize>> = BTreeMap::new();
                for (ci, c) in corners.iter().enumerate() {
                    gmap.entry(s.face_group[src_faces[ci].unwrap()]).or_default().insert(ci);
                    hmap2.entry(c.2).or_default().insert(ci);
                }
                let gs: BTreeSet<&BTreeSet<usize>> = gmap.values().collect();
                let hs: BTreeSet<&BTreeSet<usize>> = hmap2.values().collect();
                let refines = |fine: &BTreeMap<u32, BTreeSet<usize>>, coarse: &BTreeMap<u32, BTreeSet<usize>>| fine.values().all(|f| coarse.values().any(|c| f.is_subset(c)));
                if gs == hs {
                    n_grp_eq += 1;
                } else if refines(&gmap, &hmap2) {
                    n_grp_refines_his += 1;
                } else if refines(&hmap2, &gmap) {
                    n_his_refines_grp += 1;
                }
            }
        }
        let hg: BTreeSet<u32> = corners.iter().map(|c| c.2).collect();
        let mg: BTreeSet<u32> = corners.iter().map(|c| c.3).collect();
        sum_his += hg.len();
        sum_mine += mg.len();
        // partition equality: same grouping of corners
        let mut hmap: BTreeMap<u32, BTreeSet<usize>> = BTreeMap::new();
        let mut mmap: BTreeMap<u32, BTreeSet<usize>> = BTreeMap::new();
        for (ci, c) in corners.iter().enumerate() {
            hmap.entry(c.2).or_default().insert(ci);
            mmap.entry(c.3).or_default().insert(ci);
            let (hv_, mv_) = (c.2 as usize, c.3 as usize);
            corners_total += 1;
            if b3f(his.nrm[hv_]) == b3f(mine.nrm[mv_]) {
                n_exact += 1;
            }
            if his.tu.get(hv_).map(|v| b3f(*v)) == mine.tu.get(mv_).map(|v| b3f(*v)) {
                u_exact += 1;
            }
            if his.tv.get(hv_).map(|v| b3f(*v)) == mine.tv.get(mv_).map(|v| b3f(*v)) {
                v_exact += 1;
            }
        }
        let hset: BTreeSet<&BTreeSet<usize>> = hmap.values().collect();
        let mset: BTreeSet<&BTreeSet<usize>> = mmap.values().collect();
        // Per-attribute partitions (his vs mine) to attribute the disagreement.
        let part_by = |f: &dyn Fn(&(usize, usize, u32, u32)) -> Vec<u32>| -> BTreeSet<BTreeSet<usize>> {
            let mut m: BTreeMap<Vec<u32>, BTreeSet<usize>> = BTreeMap::new();
            for (ci, c) in corners.iter().enumerate() {
                m.entry(f(c)).or_default().insert(ci);
            }
            m.into_values().collect()
        };
        let b3 = |v: [f32; 3]| vec![v[0].to_bits(), v[1].to_bits(), v[2].to_bits()];
        let hn = part_by(&|c| b3(his.nrm[c.2 as usize]));
        let mn = part_by(&|c| b3(mine.nrm[c.3 as usize]));
        let hu = part_by(&|c| his.tu.get(c.2 as usize).map(|v| b3(*v)).unwrap_or_default());
        let mu = part_by(&|c| mine.tu.get(c.3 as usize).map(|v| b3(*v)).unwrap_or_default());
        let hv = part_by(&|c| his.tv.get(c.2 as usize).map(|v| b3(*v)).unwrap_or_default());
        let mv = part_by(&|c| mine.tv.get(c.3 as usize).map(|v| b3(*v)).unwrap_or_default());
        let h1 = part_by(&|c| his.uv1.get(c.2 as usize).map(|v| vec![v[0].to_bits(), v[1].to_bits()]).unwrap_or_default());
        let m1 = part_by(&|c| mine.uv1.get(c.3 as usize).map(|v| vec![v[0].to_bits(), v[1].to_bits()]).unwrap_or_default());
        if hn != mn {
            attr_n += 1;
        }
        if hu != mu {
            attr_u += 1;
        }
        if hv != mv {
            attr_v += 1;
        }
        if h1 != m1 {
            attr_uv1 += 1;
        }
        if hset != mset && hn == mn && hu == mu && hv == mv && h1 == m1 {
            attr_none += 1;
        }
        if hset == mset {
            n_same += 1;
            if std::env::var("PARTDIFF_ATTR").ok().as_deref() != Some("N") || hn == mn {
                continue;
            }
        }
        if std::env::var("PARTDIFF_ATTR").ok().as_deref() == Some("N") && hn == mn {
            continue;
        }
        // refinement tests
        let refines = |fine: &BTreeMap<u32, BTreeSet<usize>>, coarse: &BTreeMap<u32, BTreeSet<usize>>| {
            fine.values().all(|f| coarse.values().any(|c| f.is_subset(c)))
        };
        let kind = if refines(&mmap, &hmap) {
            n_finer += 1;
            "MINE-FINER"
        } else if refines(&hmap, &mmap) {
            n_coarser += 1;
            "MINE-COARSER"
        } else {
            n_crossed += 1;
            "CROSSED"
        };
        if shown >= max_show {
            continue;
        }
        shown += 1;
        println!("\npos {} {} his_groups={} mine_groups={} corners={}", hx(*p), kind, hg.len(), mg.len(), corners.len());
        // per corner line
        let mut hids: Vec<u32> = hg.iter().copied().collect();
        hids.sort();
        let mut mids: Vec<u32> = mg.iter().copied().collect();
        mids.sort();
        let fnorms: Vec<[f32; 3]> = corners
            .iter()
            .map(|c| {
                let t = mine.tris[c.0];
                face_normal(&[mine.pos[t[0] as usize], mine.pos[t[1] as usize], mine.pos[t[2] as usize]])
            })
            .collect();
        for (ci, c) in corners.iter().enumerate() {
            let hgi = hids.iter().position(|&x| x == c.2).unwrap();
            let mgi = mids.iter().position(|&x| x == c.3).unwrap();
            let hv = c.2 as usize;
            let mv = c.3 as usize;
            let uvs = format!("uv=({:.4},{:.4})", mine.uv[mv][0], mine.uv[mv][1]);
            let uv1s = if !mine.uv1.is_empty() { format!(" uv1=({:.4},{:.4})", mine.uv1[mv][0], mine.uv1[mv][1]) } else { String::new() };
            let srcs = match (&src, src_faces[ci]) {
                (Some(s), Some(fi)) => format!(" src=f{} g{}({}) n{}", fi, s.face_group[fi], s.group_name.get(s.face_group[fi] as usize).cloned().unwrap_or_default(), s.face_nverts[fi]),
                (Some(_), None) => " src=?".to_string(),
                _ => String::new(),
            };
            println!(
                "  c{} tri{} k{} H{} M{} fn={} {}{}{srcs}\n      hisN={} myN={} dN={:.2}  hisU={} myU={} dU={:.2}  hisV={} myV={} dV={:.2}",
                ci, c.0, c.1, hgi, mgi, f3(fnorms[ci]), uvs, uv1s,
                f3(his.nrm[hv]), f3(mine.nrm[mv]), ang(his.nrm[hv], mine.nrm[mv]),
                f3(his.tu.get(hv).copied().unwrap_or([0.0; 3])), f3(mine.tu.get(mv).copied().unwrap_or([0.0; 3])),
                ang(his.tu.get(hv).copied().unwrap_or([1.0, 0.0, 0.0]), mine.tu.get(mv).copied().unwrap_or([1.0, 0.0, 0.0])),
                f3(his.tv.get(hv).copied().unwrap_or([0.0; 3])), f3(mine.tv.get(mv).copied().unwrap_or([0.0; 3])),
                ang(his.tv.get(hv).copied().unwrap_or([1.0, 0.0, 0.0]), mine.tv.get(mv).copied().unwrap_or([1.0, 0.0, 0.0])),
            );
        }
        // pairwise face angles
        let mut s = String::from("  face-angles:");
        for i in 0..corners.len() {
            for j in (i + 1)..corners.len() {
                s.push_str(&format!(" c{}-c{}={:.1}", i, j, ang(fnorms[i], fnorms[j])));
            }
        }
        println!("{}", s);
    }
    println!(
        "\npositions={} same={} mine-finer={} mine-coarser={} crossed={} | sum his groups={} mine groups={}",
        n_pos, n_same, n_finer, n_coarser, n_crossed, sum_his, sum_mine
    );
    println!("attribute partitions differ at: N={} U={} V={} uv1={} positions; partition differs with all attrs agreeing: {}", attr_n, attr_u, attr_v, attr_uv1, attr_none);
    println!(
        "corner values bit-exact: N {}/{} ({:.1}%)  U {}/{} ({:.1}%)  V {}/{} ({:.1}%)",
        n_exact, corners_total, 100.0 * n_exact as f64 / corners_total.max(1) as f64,
        u_exact, corners_total, 100.0 * u_exact as f64 / corners_total.max(1) as f64,
        v_exact, corners_total, 100.0 * v_exact as f64 / corners_total.max(1) as f64
    );
    if src.is_some() {
        println!(
            "source-group hypothesis: his==by-group at {} positions, by-group finer than his at {}, his finer than by-group at {}, neither at {}, unmapped {}",
            n_grp_eq, n_grp_refines_his, n_his_refines_grp, n_pos - n_unmapped - n_grp_eq - n_grp_refines_his - n_his_refines_grp, n_unmapped
        );
    }
}
