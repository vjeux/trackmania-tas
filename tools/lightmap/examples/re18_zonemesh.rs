//! `re18_zonemesh PAK:KEY COLLECTION ZONE [ZONE…]` — the zone tile's LM mesh as the port builds it from the pak (the game's
//! 9-vertex / 8-triangle ground quad the SET pass instances 9 216×): every vertex's position, NORMAL, LM uv, PSIZE mode and
//! tangent, plus the triangle face normals (RE 18, 2026-10-01 — the RI tile-normal read: is the stored normal up, down, zero
//! or tilted?).
fn main() {
    let a: Vec<String> = std::env::args().collect();
    let (pp, key) = a[1].rsplit_once(':').expect("PAK:KEY");
    let coll = &a[2];
    let mut store = mapgeom::store::DataStore::empty();
    store.add_pak(pp, key).expect("pak");
    for zone in &a[3..] {
        println!("== {coll} / {zone}");
        match lightmap::lmmesh::lm_mesh_of_zone(&mut store, coll, zone) {
            Err(e) => println!("  error: {e}"),
            Ok(None) => println!("  no LM mesh (no lightmapped visual)"),
            Ok(Some(m)) => {
                println!("  {} vertices, {} indices ({} triangles)", m.verts.len(), m.indices.len(), m.indices.len() / 3);
                for (i, v) in m.verts.iter().enumerate() {
                    println!("  v{i:2} pos ({:8.3}, {:8.3}, {:8.3}) normal ({:7.4}, {:7.4}, {:7.4}) |n| {:.4} uv ({:.4}, {:.4}) psize {} tangent ({:.3}, {:.3}, {:.3}, {:.3}) chart_idx {}", v.pos[0], v.pos[1], v.pos[2], v.normal[0], v.normal[1], v.normal[2], (v.normal[0] * v.normal[0] + v.normal[1] * v.normal[1] + v.normal[2] * v.normal[2]).sqrt(), v.uv[0], v.uv[1], v.psize, v.tangent[0], v.tangent[1], v.tangent[2], v.tangent[3], v.chart_idx);
                }
                for t in m.indices.chunks(3) {
                    let p = |i: u16| m.verts[i as usize].pos;
                    let (a0, b0, c0) = (p(t[0]), p(t[1]), p(t[2]));
                    let e1 = [b0[0] - a0[0], b0[1] - a0[1], b0[2] - a0[2]];
                    let e2 = [c0[0] - a0[0], c0[1] - a0[1], c0[2] - a0[2]];
                    let n = [e1[1] * e2[2] - e1[2] * e2[1], e1[2] * e2[0] - e1[0] * e2[2], e1[0] * e2[1] - e1[1] * e2[0]];
                    let l = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt().max(1e-12);
                    println!("  tri ({:2}, {:2}, {:2}) face normal (e1×e2) ({:6.3}, {:6.3}, {:6.3})", t[0], t[1], t[2], n[0] / l, n[1] / l, n[2] / l);
                }
            }
        }
    }
}
