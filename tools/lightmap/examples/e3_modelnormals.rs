//! `e3_modelnormals MAP.Map.Gbx NAME_SUBSTR` — one embedded model's triangles: area-weighted normal histogram (which way its
//! faces point) and the vertex positions' bbox — is a "Land" quad up- or down-facing? (E3 2026-09-28, g23's AC06423111)
fn main() {
    let args: Vec<String> = std::env::args().collect();
    let scene = lightmap::geometry::Scene::from_map(&args[1]).expect("scene");
    let filt = args.get(2).cloned().unwrap_or_default();
    for (mi, m) in scene.models.iter().enumerate() {
        let name = scene.model_names.get(mi).cloned().unwrap_or_default();
        if !name.contains(filt.as_str()) { continue; }
        let mut up = 0f64; let mut down = 0f64; let mut side = 0f64;
        let (mut lo, mut hi) = ([f32::MAX; 3], [f32::MIN; 3]);
        for t in &m.tris {
            let a = lightmap::geometry::sub(t.p[1], t.p[0]);
            let b = lightmap::geometry::sub(t.p[2], t.p[0]);
            let c = lightmap::geometry::cross(a, b);
            let area = 0.5 * lightmap::geometry::dot(c, c).sqrt() as f64;
            let n = lightmap::geometry::norm(c);
            if n[1] > 0.7 { up += area } else if n[1] < -0.7 { down += area } else { side += area }
            for p in t.p { for k in 0..3 { lo[k] = lo[k].min(p[k]); hi[k] = hi[k].max(p[k]); } }
        }
        println!("model {mi} {name}: {} tris; area up {:.0} down {:.0} side {:.0}; bbox {:?}..{:?}; first tri {:?} n {:?}", m.tris.len(), up, down, side, lo, hi, m.tris.first().map(|t| t.p), m.tris.first().map(|t| t.n));
    }
}
