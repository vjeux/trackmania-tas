//! Compare weld partitions HIS vs MINE at exact-bit positions. Usage: partcomp HIS.ITEM MINE.ITEM SUBSTR [N]
use std::collections::{BTreeMap, VecDeque};
use mapgeom::static_item::vstream::Elem;
fn dec(v: u32) -> [f32; 3] {
    let x = ((v & 0x3FF) as i32) << 22 >> 22;
    let y = ((v >> 10 & 0x3FF) as i32) << 22 >> 22;
    let z = ((v >> 20 & 0x3FF) as i32) << 22 >> 22;
    [x as f32 / 511.0, y as f32 / 511.0, z as f32 / 511.0]
}
struct Mesh {
    vpos: Vec<[u32; 3]>,
    tris: Vec<[usize; 3]>,
}
fn main() {
    let a: Vec<String> = std::env::args().collect();
    let n: usize = a.get(4).and_then(|x| x.parse().ok()).unwrap_or(15);
    let load = |path: &str| -> Mesh {
        let data = std::fs::read(path).unwrap();
        let f = mapgeom::static_item::file::parse_file(&data).unwrap();
        let so = f.item.static_object().unwrap();
        let s2 = so.solid2().unwrap();
        let mut vpos = Vec::new();
        let mut tris = Vec::new();
        for g in &s2.shaded_geoms {
            let vi = g.visual_index.max(0) as usize;
            let mi = g.material_index.max(0) as usize;
            let mat = s2.custom_materials.get(mi).and_then(|m| m.inst()).map(|i| i.link().unwrap_or("").to_string()).unwrap_or("".into());
            let stem = mat.rsplit('\\').next().unwrap_or(&mat).to_string();
            if stem != a[3] { continue; }
            if let Some(vref) = s2.visuals.get(vi) {
                if let Some(mapgeom::static_item::Node::Visual(vis)) = vref.inline.as_deref() {
                    let st = vis.stream().unwrap();
                    let mut pos: Vec<[f32; 3]> = Vec::new();
                    for (d, e) in st.decls.iter().zip(st.elems.iter()) {
                        if let Elem::Float3(p) = e {
                            if d.name() == 0 { pos = p.clone(); }
                        }
                    }
                    let base = vpos.len();
                    for p in &pos {
                        vpos.push([p[0].to_bits(), p[1].to_bits(), p[2].to_bits()]);
                    }
                    let idx = vis.index_buffer.as_ref().map(|b| b.indices.clone()).unwrap_or_default();
                    for t in idx.chunks(3) {
                        if t.len() < 3 { continue; }
                        tris.push([base + t[0] as usize, base + t[1] as usize, base + t[2] as usize]);
                    }
                }
            }
        }
        // (touch dec to keep helper live)
        let _ = dec(0);
        Mesh { vpos, tris }
    };
    let r = load(&a[1]);
    let m = load(&a[2]);
    // tri key: sorted exact-bit position triple
    let tkey = |mesh: &Mesh, t: &[usize; 3]| {
        let mut k = [mesh.vpos[t[0]], mesh.vpos[t[1]], mesh.vpos[t[2]]];
        k.sort();
        k
    };
    let mut mq: BTreeMap<[[u32; 3]; 3], VecDeque<usize>> = BTreeMap::new();
    for (i, t) in m.tris.iter().enumerate() {
        mq.entry(tkey(&m, t)).or_default().push_back(i);
    }
    // pair tris; build corner correspondence at shared positions
    // per position: items = (his_vert, my_vert) pairs per corner occurrence
    let mut at: BTreeMap<[u32; 3], Vec<(usize, usize)>> = BTreeMap::new();
    let mut unpaired = 0;
    for (hi, ht) in r.tris.iter().enumerate() {
        let k = tkey(&r, ht);
        let mi = match mq.get_mut(&k).and_then(|q| q.pop_front()) {
            Some(v) => v,
            None => { unpaired += 1; continue; }
        };
        let mt = m.tris[mi];
        // match corners by position (assume unique within tri)
        for ck in 0..3 {
            let hp = r.vpos[ht[ck]];
            let mut found = None;
            for dk in 0..3 {
                if m.vpos[mt[dk]] == hp { found = Some(dk); break; }
            }
            match found {
                Some(dk) => { at.entry(hp).or_default().push((ht[ck], mt[dk])); }
                None => { unpaired += 1; }
            }
        }
        let _ = hi;
    }
    let mleft: usize = mq.values().map(|q| q.len()).sum();
    println!("paired tris, unpaired_corners={unpaired} mine_left={mleft}");
    // per position: partitions + disagreement pairs
    struct Row { pos: [u32; 3], hv: usize, mv: usize, split: usize, merge: usize }
    let mut rows: Vec<Row> = Vec::new();
    for (p, items) in &at {
        let mut hgroups: BTreeMap<usize, Vec<usize>> = BTreeMap::new();
        let mut mgroups: BTreeMap<usize, Vec<usize>> = BTreeMap::new();
        for (i, (hv, mv)) in items.iter().enumerate() {
            hgroups.entry(*hv).or_default().push(i);
            mgroups.entry(*mv).or_default().push(i);
        }
        // pairs together in his but split in mine (split), together in mine but split in his (merge)
        let mut split = 0;
        let mut merge = 0;
        for i in 0..items.len() {
            for j in (i + 1)..items.len() {
                let ht = items[i].0 == items[j].0;
                let mt = items[i].1 == items[j].1;
                if ht && !mt { split += 1; }
                if mt && !ht { merge += 1; }
            }
        }
        rows.push(Row { pos: *p, hv: hgroups.len(), mv: mgroups.len(), split, merge });
    }
    rows.sort_by_key(|r| std::cmp::Reverse(r.split + r.merge));
    println!("positions={} with_disagreement={}", rows.len(), rows.iter().filter(|r| r.split + r.merge > 0).count());
    for r in rows.iter().take(n) {
        let p = [f32::from_bits(r.pos[0]), f32::from_bits(r.pos[1]), f32::from_bits(r.pos[2])];
        println!("pos=({:.5},{:.5},{:.5}) hisv={} minev={} split_pairs={} merge_pairs={}", p[0], p[1], p[2], r.hv, r.mv, r.split, r.merge);
    }
}
