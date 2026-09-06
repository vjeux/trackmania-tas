//! Weld iff angle<θ OR shared-edge<L. Usage: edgeweld SRCFILE
use std::collections::BTreeMap;
use mapgeom::static_item::bake::geometry_layers;
fn main() {
    let a: Vec<String> = std::env::args().collect();
    let targets: Vec<(&str, usize)> = vec![
        ("TrackBorders", 1078), ("TechnicsTrims", 1136),
        ("TechnicsSpecials", 6074), ("Technics", 1280), ("RoadTech", 279),
        ("SpecialFXTurbo", 764),
    ];
    let data = std::fs::read(&a[1]).unwrap();
    let g = tmmaps::gbx::Gbx::parse(&data);
    let loc = mapgeom::crystal_model::locate(&g.body).unwrap();
    let (c, _, _) = mapgeom::crystal_model::CPlugCrystal::parse_with(&g.body, loc.at, loc.lookback.clone()).unwrap();
    let layers = geometry_layers(&c);
    let stems: Vec<String> = c.materials.iter().map(|m| m.inst().map(|x| x.link().unwrap_or("").rsplit('\\').next().unwrap_or("").to_string()).unwrap_or_default()).collect();
    // per material: tris (pos, normal), and edge map (pospair -> length)
    let mut permat: BTreeMap<usize, Vec<([f32; 3], [f32; 3])>> = BTreeMap::new();
    for (cr, vis, _) in &layers {
        if !vis { continue; }
        for f in &cr.faces {
            if f.material < 0 { continue; }
            let pts: Vec<[f32; 3]> = f.verts.iter().map(|i| cr.positions[*i as usize]).collect();
            if pts.len() < 3 { continue; }
            let mut n = [0f32; 3];
            for i in 0..pts.len() {
                let a2 = pts[i];
                let b = pts[(i+1)%pts.len()];
                n[0] += (a2[1]-b[1])*(a2[2]+b[2]);
                n[1] += (a2[2]-b[2])*(a2[0]+b[0]);
                n[2] += (a2[0]-b[0])*(a2[1]+b[1]);
            }
            let l = (n[0]*n[0]+n[1]*n[1]+n[2]*n[2]).sqrt();
            if l < 1e-12 { continue; }
            let nn = [n[0]/l, n[1]/l, n[2]/l];
            let mut tris: Vec<[[f32; 3]; 3]> = Vec::new();
            if pts.len() == 3 { tris.push([pts[0], pts[1], pts[2]]); }
            else { for i in 2..pts.len() { tris.push([pts[1], pts[i], pts[(i+1)%pts.len()]]); } }
            for tri in &tris {
                for cc in 0..3 {
                    permat.entry(f.material as usize).or_default().push((tri[cc], nn));
                }
            }
        }
    }
    let cidx = |stem: &str| -> Option<usize> {
        stems.iter().position(|s| {
            let full = format!("Stadium\\Media\\Material\\{s}");
            let r = mapgeom::static_item::build::resolve_crystal_link(&full);
            r.rsplit('\\').next().unwrap_or(r) == stem
        }).or_else(|| stems.iter().position(|s| s == stem))
    };
    // Precompute: for clustering need shared edge lengths. Simplify: weld iff angle<θ OR (same quad?) -- no.
    // Instead: two-pass: first cluster by angle<θ (seed-largest); then MERGE clusters whose representative faces share an edge < L?
    // Simpler test: single-linkage where link = (angle<θ OR edge<L). Need edges: build per-position corner list with tri ids, then for pairs, find shared edge length.
    // (Implement straightforwardly: corners have tri id; edge length between two corners' tris = length of shared edge if adjacent.)
    for lmm in [0.05f32, 0.2, 0.5, 1.0] {
        for angle in [30.0f32, 44.0] {
            let cos_max = angle.to_radians().cos();
            let mut total_err = 0i64;
            let mut line = format!("edgeL={lmm} deg={angle}:");
            for (stem, want) in &targets {
                let ci = match cidx(stem) { Some(i) => i, None => continue };
                let corners = match permat.get(&ci) { Some(v) => v, None => continue };
                // rebuild tris for edge computation: group corners by tri? corners lost tri id. Redo with tri ids.
                // (Shortcut: approximate edge by min corner-pair distance? No. Skip precise edges; use FACE PAIR min distance? )
                // Fallback: implement properly below if promising. For now use single-linkage + small-face join (already tested). PRINT placeholder.
                let _ = (corners, cos_max);
                line += &format!(" {stem}=?");
            }
            println!("{line} TOTERR=? (needs tri ids)");
            let _ = total_err;
        }
    }
}
