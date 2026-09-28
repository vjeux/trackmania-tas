//! `e3_lmuv MAP.Map.Gbx [NAME_SUBSTR…]` — per embedded item model: the LIGHTMAP uv bounds the port rasterises with
//! (`ModelGeom.uv_min/uv_max`, the PreLightGen bounds/u02, metres per uv) and the count of triangles whose lm uv leaves
//! [0, 1] — a model whose lm uvs spill past its chart rectangle writes its fragments into its neighbours' charts (the
//! Stadium giant's AI06220000 × 26 lands on tile 20095's texels; E3 2026-09-28).
fn main() {
    let args: Vec<String> = std::env::args().collect();
    let map = args.get(1).expect("MAP.Map.Gbx");
    let filters: Vec<String> = args[2..].to_vec();
    let scene = lightmap::geometry::Scene::from_map(map).expect("scene");
    let mut counts = vec![0usize; scene.models.len()];
    let mut names: Vec<String> = vec![String::new(); scene.models.len()];
    for inst in &scene.instances {
        counts[inst.model] += 1;
        if names[inst.model].is_empty() { names[inst.model] = inst.model_name.clone(); }
    }
    println!("{:28} {:>5} {:>7} {:>26} {:>26} {:>10} {:>10} {:>7}", "model", "inst", "tris", "uv_min", "uv_max", "plg_u02", "m/uv", "spill");
    for (mi, m) in scene.models.iter().enumerate() {
        if counts[mi] == 0 { continue; }
        if !filters.is_empty() && !filters.iter().any(|f| names[mi].contains(f.as_str())) { continue; }
        let mut spill = 0usize;
        let (mut lo, mut hi) = ([f32::MAX; 2], [f32::MIN; 2]);
        for t in &m.tris {
            let mut out = false;
            for k in 0..3 {
                let uv = t.uv[k];
                for c in 0..2 { lo[c] = lo[c].min(uv[c]); hi[c] = hi[c].max(uv[c]); if !(uv[c] >= -1e-4 && uv[c] <= 1.0 + 1e-4) { out = true; } }
            }
            if out { spill += 1; }
        }
        println!("{:28} {:>5} {:>7} ({:>10.4},{:>10.4}) ({:>10.4},{:>10.4}) {:>10.4} {:>10.3} {:>7}   plg {:?}", names[mi], counts[mi], m.tris.len(), lo[0], lo[1], hi[0], hi[1], m.plg_u02, m.metres_per_uv, spill, m.plg_bounds);
        // --per-mat: the uv range per material slot (which geoms leave the PreLightGen's box)
        if args.iter().any(|a| a == "--per-mat") {
            let mut per: std::collections::BTreeMap<u16, ([f32; 2], [f32; 2], usize, f64)> = Default::default();
            for t in &m.tris {
                let e = per.entry(t.mat).or_insert(([f32::MAX; 2], [f32::MIN; 2], 0, 0.0));
                for k in 0..3 { for c in 0..2 { e.0[c] = e.0[c].min(t.uv[k][c]); e.1[c] = e.1[c].max(t.uv[k][c]); } }
                e.2 += 1;
                let a: Vec<f64> = (0..3).map(|c| (t.p[1][c] - t.p[0][c]) as f64).collect();
                let b: Vec<f64> = (0..3).map(|c| (t.p[2][c] - t.p[0][c]) as f64).collect();
                let cx = a[1] * b[2] - a[2] * b[1]; let cy = a[2] * b[0] - a[0] * b[2]; let cz = a[0] * b[1] - a[1] * b[0];
                e.3 += 0.5 * (cx * cx + cy * cy + cz * cz).sqrt();
            }
            for (mat, (lo, hi, n, area)) in &per {
                let link = m.mat_links.get(*mat as usize).cloned().unwrap_or_else(|| format!("(mat {mat})"));
                println!("      slot {:>3} {:>6} tris area {:>9.2}  uv ({:.4},{:.4})–({:.4},{:.4})  {}", mat, n, area, lo[0], lo[1], hi[0], hi[1], link);
            }
        }
    }
}
