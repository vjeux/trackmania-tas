//! Match each reference tiny item to its source Nadeo item by material-set
//! signature, then bake the source at 0.5 with the reference's own
//! ident/author and byte-compare. Usage:
//!   match_refs NADEO.zip REFDIR
//! Prints per reference: ident, author, n materials, candidate sources,
//! and bake-vs-reference byte comparison.
use std::collections::{BTreeMap, BTreeSet};

fn ref_materials(path: &str) -> (String, String, Vec<(String, u8)>, ([f32; 3], [f32; 3])) {
    let data = std::fs::read(path).unwrap();
    let (ident, author) = tmmaps::header::item_ident_author(&data).unwrap_or(("?".into(), "?".into()));
    let f = mapgeom::static_item::file::parse_file(&data).unwrap();
    let so = f.item.static_object().unwrap();
    let s2 = so.solid2().unwrap();
    let mats: Vec<(String, u8)> = s2
        .custom_materials
        .iter()
        .map(|m| m.inst().map(|i| (i.link().unwrap_or("?").to_string(), i.physics())).unwrap_or(("?".into(), 99)))
        .collect();
    // bounds over first float3 elem of each visual
    let mut lo = [f32::MAX; 3];
    let mut hi = [f32::MIN; 3];
    for v in &s2.visuals {
        if let Some(mapgeom::static_item::Node::Visual(vis)) = v.inline.as_deref() {
            if let Some(st) = vis.stream() {
                for e in &st.elems {
                    if let mapgeom::static_item::vstream::Elem::Float3(p) = e {
                        for q in p {
                            for k in 0..3 {
                                lo[k] = lo[k].min(q[k]);
                                hi[k] = hi[k].max(q[k]);
                            }
                        }
                        break;
                    }
                }
            }
        }
    }
    (ident, author, mats, (lo, hi))
}

fn pos_key(p: &[f32; 3]) -> (i32, i32, i32) {
    ((p[0] * 1000.0).round() as i32, (p[1] * 1000.0).round() as i32, (p[2] * 1000.0).round() as i32)
}

fn ref_positions(path: &str) -> BTreeSet<(i32, i32, i32)> {
    let data = std::fs::read(path).unwrap();
    let f = mapgeom::static_item::file::parse_file(&data).unwrap();
    let so = f.item.static_object().unwrap();
    let s2 = so.solid2().unwrap();
    let mut set = BTreeSet::new();
    for v in &s2.visuals {
        if let Some(mapgeom::static_item::Node::Visual(vis)) = v.inline.as_deref() {
            if let Some(st) = vis.stream() {
                for e in &st.elems {
                    if let mapgeom::static_item::vstream::Elem::Float3(p) = e {
                        for q in p {
                            // scale back up x2 (half-scale item -> full-size source)
                            set.insert(pos_key(&[q[0] * 2.0, q[1] * 2.0, q[2] * 2.0]));
                        }
                        break;
                    }
                }
            }
        }
    }
    set
}

fn main() {
    let a: Vec<String> = std::env::args().collect();
    if a.iter().any(|x| x == "--geom") {
        geom_main(&a);
        return;
    }
    let zip = std::fs::read(&a[1]).expect("nadeo zip");
    let refdir = &a[2];
    // Nadeo side: material-set signature per item (crystal decode)
    let files = mapgeom::embedded::unzip(&zip).expect("zip");
    let mut nadeo: BTreeMap<String, Vec<(String, u8)>> = BTreeMap::new();
    let mut nadeo_list: Vec<String> = Vec::new();
    for (name, bytes) in &files {
        if !name.ends_with(".Item.Gbx") {
            continue;
        }
        let (mats, _mesh) = mapgeom::crystal::decode_template(bytes);
        let sig: Vec<(String, u8)> = mats.iter().map(|m| (m.link.clone(), m.physics)).collect();
        nadeo.insert(name.clone(), sig);
        nadeo_list.push(name.clone());
    }
    nadeo_list.sort();
    println!("nadeo items: {}", nadeo_list.len());
    // Reference side
    let mut refs: Vec<String> = std::fs::read_dir(refdir)
        .unwrap()
        .flatten()
        .map(|e| e.path().to_string_lossy().to_string())
        .filter(|p| p.ends_with(".Item.Gbx"))
        .collect();
    refs.sort();
    for r in &refs {
        let short = r.rsplit('/').next().unwrap();
        let (ident, author, mats, (lo, hi)) = ref_materials(r);
        let rset: BTreeSet<(String, u8)> = mats.iter().cloned().collect();
        let mut cands = Vec::new();
        for n in &nadeo_list {
            let nset: BTreeSet<(String, u8)> = nadeo[n].iter().cloned().collect();
            if nset == rset {
                cands.push(n.clone());
            }
        }
        println!("{short}: ident={ident} author={author} nmats={} bounds=[{:.1},{:.1},{:.1}]-[{:.1},{:.1},{:.1}] exact_matset_matches={}",
            mats.len(), lo[0], lo[1], lo[2], hi[0], hi[1], hi[2], cands.len());
        for c in cands.iter().take(5) {
            println!("    cand {c}");
        }
        // bake first candidate (or all if few) with ref ident/author at 0.5 and compare
        for c in cands.iter().take(3) {
            let src = &files[c.as_str()];
            match mapgeom::static_item::build::static_item_from_item(src, &ident, &author, 0.5) {
                Ok(baked) => {
                    let orig = std::fs::read(r).unwrap();
                    if baked == orig {
                        println!("    BAKE {c}: BYTE-IDENTICAL ({} bytes)", baked.len());
                    } else {
                        let n = baked.len().min(orig.len());
                        let mut first = None;
                        for i in 0..n {
                            if baked[i] != orig[i] {
                                first = Some(i);
                                break;
                            }
                        }
                        println!("    BAKE {c}: DIFF len {} vs {} first_diff@{:?} head_orig={} head_baked={}",
                            baked.len(), orig.len(), first,
                            orig.get(first.unwrap_or(0).saturating_sub(0)..first.unwrap_or(0)+16).map(|s| s.iter().map(|b| format!("{b:02x}")).collect::<String>()).unwrap_or_default(),
                            baked.get(first.unwrap_or(0).saturating_sub(0)..first.unwrap_or(0)+16).map(|s| s.iter().map(|b| format!("{b:02x}")).collect::<String>()).unwrap_or_default());
                    }
                }
                Err(e) => println!("    BAKE {c}: build failed: {e}"),
            }
        }
    }
}

/// `--geom`: match references to Nadeo sources by geometry. For each ref,
/// scale verts x2 and score every Nadeo crystal mesh by position-set overlap.
/// Then bake the top candidate with the ref's ident/author at 0.5 and compare.
fn geom_main(a: &[String]) {
    let zip = std::fs::read(&a[1]).expect("nadeo zip");
    let refdir = &a[3];
    let files = mapgeom::embedded::unzip(&zip).expect("zip");
    // decode all Nadeo meshes once
    let mut nadeo_pos: BTreeMap<String, BTreeSet<(i32, i32, i32)>> = BTreeMap::new();
    let mut nadeo_nmats: BTreeMap<String, usize> = BTreeMap::new();
    for (name, bytes) in &files {
        if !name.ends_with(".Item.Gbx") {
            continue;
        }
        let (mats, mesh) = mapgeom::crystal::decode_template(bytes);
        let set: BTreeSet<(i32, i32, i32)> = mesh.positions.iter().map(pos_key).collect();
        nadeo_pos.insert(name.clone(), set);
        nadeo_nmats.insert(name.clone(), mats.len());
    }
    println!("nadeo items: {}", nadeo_pos.len());
    let mut refs: Vec<String> = std::fs::read_dir(refdir)
        .unwrap()
        .flatten()
        .map(|e| e.path().to_string_lossy().to_string())
        .filter(|p| p.ends_with(".Item.Gbx"))
        .collect();
    refs.sort();
    for r in &refs {
        let short = r.rsplit('/').next().unwrap();
        let rp = ref_positions(r);
        let (ident, author) = tmmaps::header::item_ident_author(&std::fs::read(r).unwrap()).unwrap_or(("?".into(), "?".into()));
        // score: |ref ∩ nadeo| / |ref|
        let mut scored: Vec<(u32, usize, String)> = Vec::new();
        for (n, np) in &nadeo_pos {
            let inter = rp.intersection(np).count();
            if inter > 0 {
                scored.push((inter as u32, np.len(), n.clone()));
            }
        }
        scored.sort_by(|x, y| y.0.cmp(&x.0).then(x.1.cmp(&y.1)));
        let cov = |inter: u32| 100.0 * inter as f64 / rp.len().max(1) as f64;
        println!("{short}: ref_verts={} top:", rp.len());
        for (inter, nn, n) in scored.iter().take(4) {
            println!("    {:.1}% of ref in {n} (nverts={} nmats={})", cov(*inter), nn, nadeo_nmats[n.as_str()]);
        }
        // bake top candidate with ref ident/author, compare
        if let Some((inter, _, n)) = scored.first() {
            if cov(*inter) > 50.0 {
                let src = &files[n.as_str()];
                match mapgeom::static_item::build::static_item_from_item(src, &ident, &author, 0.5) {
                    Ok(baked) => {
                        let orig = std::fs::read(r).unwrap();
                        if baked == orig {
                            println!("    BAKE {n}: BYTE-IDENTICAL ({} bytes)", baked.len());
                        } else {
                            let m = baked.len().min(orig.len());
                            let first = (0..m).find(|&i| baked[i] != orig[i]);
                            println!("    BAKE {n}: DIFF len {} vs {} first_diff@0x{:x}", baked.len(), orig.len(), first.unwrap_or(m));
                        }
                    }
                    Err(e) => println!("    BAKE {n}: build failed: {e}"),
                }
            }
        }
    }
}
