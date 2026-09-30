//! `re17_itemnormals ITEM.Item.Gbx…` — per static item: each visual's index count, vertex count, and the stored NORMAL stream's
//! statistics (how many vertex normals point down (n.y < 0) in MODEL space, and the winding-vs-normal agreement: for each triangle,
//! sign of (right-hand face normal · mean vertex normal)) — the read behind V6-1c's "pitched hills lit from the visible side".
//! RE 17 2026-09-30 16:05Z.
use mapgeom::static_item::vstream::{Elem, N_NORMAL, N_POSITION};
use mapgeom::static_item::Node;
fn main() {
    for path in std::env::args().skip(1) {
        let bytes = match std::fs::read(&path) { Ok(b) => b, Err(e) => { eprintln!("{path}: {e}"); continue; } };
        let file = match mapgeom::static_item::parse_file(&bytes) { Ok(f) => f, Err(e) => { eprintln!("{path}: {e}"); continue; } };
        let name = path.rsplit('/').next().unwrap_or(&path).to_string();
        let Some(s2) = file.item.static_object().and_then(|so| so.solid2()) else { println!("{name}: no Solid2Model"); continue };
        for (vi, vr) in s2.visuals.iter().enumerate() {
            let Some(Node::Visual(v)) = vr.inline.as_deref() else { continue };
            let Some(st) = v.stream() else { println!("{name} visual {vi}: no stream"); continue };
            let empty: Vec<u32> = Vec::new();
            let idx: &Vec<u32> = v.index_buffer.as_ref().map(|ib| &ib.indices).unwrap_or(&empty);
            let mut pos: Vec<[f32; 3]> = Vec::new(); let mut nrm: Vec<[f32; 3]> = Vec::new();
            for (d, e) in st.decls.iter().zip(st.elems.iter()) {
                if d.name() == N_POSITION { if let Elem::Float3(p) = e { pos = p.clone(); } }
                if d.name() == N_NORMAL { match e { Elem::Float3(n) => nrm = n.clone(), Elem::Word(w) => nrm = w.iter().map(|x| lightmap::geometry::dec3n(*x)).collect(), _ => { eprintln!("{name} visual {vi}: normal elem not Float3/Word"); } } }
            }
            let down = nrm.iter().filter(|n| n[1] < 0.0).count();
            let (mut agree, mut disagree, mut degen) = (0usize, 0usize, 0usize);
                        for t in 0..idx.len() / 3 {
                let (a, b, c) = (idx[3*t] as usize, idx[3*t+1] as usize, idx[3*t+2] as usize);
                if a >= pos.len() || b >= pos.len() || c >= pos.len() { degen += 1; continue; }
                let (p0, p1, p2) = (pos[a], pos[b], pos[c]);
                let e1 = [p1[0]-p0[0], p1[1]-p0[1], p1[2]-p0[2]]; let e2 = [p2[0]-p0[0], p2[1]-p0[1], p2[2]-p0[2]];
                let fn_ = [e1[1]*e2[2]-e1[2]*e2[1], e1[2]*e2[0]-e1[0]*e2[2], e1[0]*e2[1]-e1[1]*e2[0]];
                if nrm.len() <= c.max(a).max(b) { degen += 1; continue; }
                let m = [nrm[a][0]+nrm[b][0]+nrm[c][0], nrm[a][1]+nrm[b][1]+nrm[c][1], nrm[a][2]+nrm[b][2]+nrm[c][2]];
                let d = fn_[0]*m[0] + fn_[1]*m[1] + fn_[2]*m[2];
                if d > 0.0 { agree += 1 } else if d < 0.0 { disagree += 1 } else { degen += 1 }
            }
            println!("{name} visual {vi}: {} indices ({} tris), {} verts, {} normals; model-space n.y < 0: {} ({:.1} %); winding·normal agree {} / disagree {} / degenerate {}", idx.len(), idx.len() / 3, pos.len(), nrm.len(), down, 100.0 * down as f64 / nrm.len().max(1) as f64, agree, disagree, degen);
        }
    }
}
