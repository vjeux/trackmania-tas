//! `e3_modelmats MAP.Map.Gbx [NAME_SUBSTR…]` — per embedded item model: placements, and per game-material link the triangle
//! count and surface AREA share (local units) — which material carries a model's bounce (E3 2026-09-28: g23's AI hills and
//! `Stadium\Media\Material_BlockCustom\CustomPlastic`, the PyPxz_Hue mask drawn as albedo).
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
    for (mi, m) in scene.models.iter().enumerate() {
        if counts[mi] == 0 { continue; }
        if !filters.is_empty() && !filters.iter().any(|f| names[mi].contains(f.as_str())) { continue; }
        let mut per: std::collections::BTreeMap<usize, (usize, f64)> = Default::default();
        let mut total_area = 0f64;
        for t in &m.tris {
            let a = t.p[1].iter().zip(t.p[0].iter()).map(|(x, y)| (x - y) as f64).collect::<Vec<f64>>();
            let b = t.p[2].iter().zip(t.p[0].iter()).map(|(x, y)| (x - y) as f64).collect::<Vec<f64>>();
            let cx = a[1] * b[2] - a[2] * b[1];
            let cy = a[2] * b[0] - a[0] * b[2];
            let cz = a[0] * b[1] - a[1] * b[0];
            let area = 0.5 * (cx * cx + cy * cy + cz * cz).sqrt();
            total_area += area;
            let e = per.entry(t.mat as usize).or_default();
            e.0 += 1;
            e.1 += area;
        }
        println!("model {mi} {} × {} placements, {} tris, area {:.0}", names[mi], counts[mi], m.tris.len(), total_area);
        let mut rows: Vec<(usize, (usize, f64))> = per.into_iter().collect();
        rows.sort_by(|a, b| b.1 .1.partial_cmp(&a.1 .1).unwrap());
        for (k, (n, area)) in rows {
            let link = m.mat_links.get(k).cloned().unwrap_or_else(|| format!("(mat {k})"));
            println!("    {:6} tris {:5.1} % area  {}  albedo {:?}", n, 100.0 * area / total_area.max(1e-9), link, m.mat_albedo.get(k));
        }
    }
}
