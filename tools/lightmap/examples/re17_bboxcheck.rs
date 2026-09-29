//! `re17_bboxcheck ITEM.Item.Gbx…` — per embedded item: the LOD-0 visuals' STORED bounding boxes (CPlugVisual main.bounding_box
//! {c, h}, the numbers the game's scene-box fold and sun camera use through the SModelRef box) vs the MESH's actual vertex
//! extents (the POSITION element of each visual's vertex stream); prints the per-axis undercut (mesh beyond the stored box) in
//! metres. RE 17 2026-09-29 22:25Z (E6's SUNCAM candidate: a hill whose stored box undercuts its mesh would have its top clipped
//! from the game's sun map).
use mapgeom::static_item::vstream::{Elem, N_POSITION};
use mapgeom::static_item::Node;

fn main() {
    let mut worst = 0.0f32;
    for path in std::env::args().skip(1) {
        let bytes = match std::fs::read(&path) { Ok(b) => b, Err(e) => { eprintln!("{path}: {e}"); continue; } };
        let file = match mapgeom::static_item::parse_file(&bytes) { Ok(f) => f, Err(e) => { eprintln!("{path}: {e}"); continue; } };
        let name = path.rsplit('/').next().unwrap_or(&path).to_string();
        let Some(s2) = file.item.static_object().and_then(|so| so.solid2()) else { println!("{name}: no static object Solid2Model"); continue };
        let mut stored: Option<([f32; 3], [f32; 3])> = None;
        let mut mesh: Option<([f32; 3], [f32; 3])> = None;
        let mut n_vis = 0usize;
        for vr in &s2.visuals {
            let Some(Node::Visual(v)) = vr.inline.as_deref() else { continue };
            n_vis += 1;
            if let Some(mm) = v.main.as_ref() {
                let b = mm.bounding_box;
                if b[3] >= 0.0 && b[3].is_finite() {
                    let (lo, hi) = ([b[0] - b[3], b[1] - b[4], b[2] - b[5]], [b[0] + b[3], b[1] + b[4], b[2] + b[5]]);
                    stored = Some(match stored { None => (lo, hi), Some((a, c)) => ([a[0].min(lo[0]), a[1].min(lo[1]), a[2].min(lo[2])], [c[0].max(hi[0]), c[1].max(hi[1]), c[2].max(hi[2])]) });
                }
            }
            if let Some(st) = v.stream() {
                for (d, e) in st.decls.iter().zip(st.elems.iter()) {
                    if d.name() != N_POSITION { continue; }
                    if let Elem::Float3(ps) = e {
                        for p in ps {
                            mesh = Some(match mesh { None => (*p, *p), Some((a, c)) => ([a[0].min(p[0]), a[1].min(p[1]), a[2].min(p[2])], [c[0].max(p[0]), c[1].max(p[1]), c[2].max(p[2])]) });
                        }
                    }
                }
            }
        }
        match (stored, mesh) {
            (Some((slo, shi)), Some((mlo, mhi))) => {
                let under: Vec<f32> = (0..3).map(|k| (slo[k] - mlo[k]).max(0.0).max((mhi[k] - shi[k]).max(0.0))).collect();
                let w = under.iter().cloned().fold(0.0f32, f32::max);
                worst = worst.max(w);
                println!("{name}: {n_vis} visuals; stored [{:.2},{:.2},{:.2}]..[{:.2},{:.2},{:.2}]  mesh [{:.2},{:.2},{:.2}]..[{:.2},{:.2},{:.2}]  undercut xyz ({:.3}, {:.3}, {:.3}) m{}",
                    slo[0], slo[1], slo[2], shi[0], shi[1], shi[2], mlo[0], mlo[1], mlo[2], mhi[0], mhi[1], mhi[2], under[0], under[1], under[2], if w > 0.05 { "  <-- UNDERCUT" } else { "" });
            }
            (s, m) => println!("{name}: {n_vis} visuals; stored {} mesh {}", s.is_some(), m.is_some()),
        }
    }
    eprintln!("worst undercut {worst:.3} m");
}
