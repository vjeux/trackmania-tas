//! `e6_modelbands MAP.Map.Gbx NAME_SUBSTR [--band 0.5]` — one embedded model's triangles by LOCAL-y band: per band the triangle
//! count, the area, the area facing up / down / sideways, split by material class (cut-out card / opaque-textured / linked),
//! with the material link or texture name — what geometry sits just above the ground under a fir (E6, 2026-09-29: tiny03's
//! roads read a black back face 0.6 m above them on every steep direction — E3).
fn main() {
    let args: Vec<String> = std::env::args().collect();
    let f = |k: &str| args.iter().position(|x| x == k).and_then(|i| args.get(i + 1)).cloned();
    let scene = lightmap::geometry::Scene::from_map(&args[1]).expect("scene");
    let filt = args.get(2).cloned().unwrap_or_default();
    let band: f32 = f("--band").map(|v| v.parse().unwrap()).unwrap_or(0.5);
    for (mi, m) in scene.models.iter().enumerate() {
        let name = scene.model_names.get(mi).cloned().unwrap_or_default();
        if !name.contains(filt.as_str()) { continue; }
        let n_inst = scene.instances.iter().filter(|i| i.model == mi).count();
        println!("model {mi} {name} × {n_inst} placements: {} tris; links {:?}; cut-out {:?}; diffuse {:?}", m.tris.len(), m.mat_links, m.alpha_tex, m.diff_tex);
        // (band index, material key) → (tris, area, up, down, side)
        let mut acc: std::collections::BTreeMap<(i32, String), (usize, f64, f64, f64, f64)> = Default::default();
        for t in &m.tris {
            let a = lightmap::geometry::sub(t.p[1], t.p[0]);
            let b = lightmap::geometry::sub(t.p[2], t.p[0]);
            let c = lightmap::geometry::cross(a, b);
            let area = 0.5 * lightmap::geometry::dot(c, c).sqrt() as f64;
            let n = lightmap::geometry::norm(c);
            let yc = (t.p[0][1] + t.p[1][1] + t.p[2][1]) / 3.0;
            let bi = (yc / band).floor() as i32;
            let key = if t.alpha != u16::MAX { format!("CARD {}", m.alpha_tex.get(t.alpha as usize).cloned().unwrap_or_default()) }
                else if t.diff != u16::MAX { format!("TEX {}", m.diff_tex.get(t.diff as usize).cloned().unwrap_or_default()) }
                else { format!("LINK {}", m.mat_links.get(t.mat as usize).cloned().unwrap_or_else(|| "?".into())) };
            let e = acc.entry((bi, key)).or_insert((0, 0.0, 0.0, 0.0, 0.0));
            e.0 += 1; e.1 += area;
            if n[1] > 0.7 { e.2 += area } else if n[1] < -0.7 { e.3 += area } else { e.4 += area }
        }
        println!("{:>14}  {:>6} {:>9} {:>9} {:>9} {:>9}  material", "local y band", "tris", "area", "up", "down", "side");
        for ((bi, key), (n, area, up, down, side)) in &acc {
            println!("[{:6.2},{:6.2})  {:>6} {:>9.2} {:>9.2} {:>9.2} {:>9.2}  {key}", *bi as f32 * band, (*bi + 1) as f32 * band, n, area, up, down, side);
        }
    }
}
