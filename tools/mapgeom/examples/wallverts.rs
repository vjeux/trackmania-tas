//! Per-vert (pos, normal) of ref wall visual + crystal adjacent face normals.
//! Usage: wallverts REF SRC
use mapgeom::static_item::vstream::Elem;
use std::collections::BTreeMap;

fn key(p: &[f32; 3]) -> (i32, i32, i32) {
    ((p[0]*1000.0).round() as i32, ((p[1]*1000.0).round() as i32), ((p[2]*1000.0).round() as i32))
}
fn sub(a: [f32; 3], b: [f32; 3]) -> [f32; 3] { [a[0]-b[0], a[1]-b[1], a[2]-b[2]] }
fn cross(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[1]*b[2]-a[2]*b[1], a[2]*b[0]-a[0]*b[2], a[0]*b[1]-a[1]*b[0]]
}
fn norm(v: [f32; 3]) -> [f32; 3] {
    let l = (v[0]*v[0]+v[1]*v[1]+v[2]*v[2]).sqrt();
    if l < 1e-12 { [0.0, 1.0, 0.0] } else { [v[0]/l, v[1]/l, v[2]/l] }
}
fn dec3n_unpack(v: u32) -> [f32; 3] {
    let x = ((v & 0x3FF) as i32) << 22 >> 22;
    let y = ((v >> 10 & 0x3FF) as i32) << 22 >> 22;
    let z = ((v >> 20 & 0x3FF) as i32) << 22 >> 22;
    [x as f32 / 511.0, y as f32 / 511.0, z as f32 / 511.0]
}

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let refdata = std::fs::read(&a[1]).unwrap();
    let f = mapgeom::static_item::file::parse_file(&refdata).unwrap();
    let so = f.item.static_object().unwrap();
    let s2 = so.solid2().unwrap();
    let vi = s2.shaded_geoms.iter().find(|g| g.material_index == 1).unwrap().visual_index as usize;
    let mut verts: Vec<([f32; 3], [f32; 3])> = Vec::new();
    if let Some(mapgeom::static_item::Node::Visual(vis)) = s2.visuals[vi].inline.as_deref() {
        let st = vis.stream().unwrap();
        let (mut pos, mut nrm) = (Vec::new(), Vec::new());
        for (d, e) in st.decls.iter().zip(st.elems.iter()) {
            match e {
                Elem::Float3(p) if d.name() == 0 => pos = p.clone(),
                Elem::Word(w) if d.name() == 5 => nrm = w.clone(),
                _ => {}
            }
        }
        for (p, n) in pos.iter().zip(nrm.iter()) {
            verts.push((*p, dec3n_unpack(*n)));
        }
    }
    // crystal face normals per position (v1-diagonal triangulation)
    let srcdata = std::fs::read(&a[2]).unwrap();
    let it = mapgeom::crystal::ItemCrystal::open(&srcdata).unwrap();
    let layer = it.model.first_geometry().unwrap();
    let c = layer.kind.crystal().unwrap();
    let mut adj: BTreeMap<(i32,i32,i32), Vec<[f32; 3]>> = BTreeMap::new();
    for fa in &c.faces {
        if fa.material != 0 { continue; }
        let pts: Vec<[f32; 3]> = fa.verts.iter().map(|i| { let p = c.positions[*i as usize]; [p[0]*0.5, p[1]*0.5, p[2]*0.5] }).collect();
        let mut tris = Vec::new();
        if pts.len() == 3 { tris.push([pts[0], pts[1], pts[2]]); }
        else { for i in 2..pts.len() { tris.push([pts[1], pts[i], pts[(i+1)%pts.len()]]); } }
        for t in &tris {
            let fn_ = norm(cross(sub(t[1], t[0]), sub(t[2], t[0])));
            for k in 0..3 {
                adj.entry(key(&t[k])).or_default().push(fn_);
            }
        }
    }
    // group ref verts by position
    let mut bypos: BTreeMap<(i32,i32,i32), Vec<[f32; 3]>> = BTreeMap::new();
    for (p, n) in &verts {
        bypos.entry(key(p)).or_default().push(*n);
    }
    for (k, ns) in &bypos {
        let ad = adj.get(k).map(|v| v.clone()).unwrap_or_default();
        let mut akeys: Vec<String> = ad.iter().map(|n| format!("({:.2},{:.2},{:.2})", n[0], n[1], n[2])).collect();
        akeys.sort(); akeys.dedup();
        let nsstr: Vec<String> = ns.iter().map(|n| format!("({:.2},{:.2},{:.2})", n[0], n[1], n[2])).collect();
        println!("pos{k:?}: nverts={} normals={nsstr:?} | adj faces={akeys:?}", ns.len());
    }
}
