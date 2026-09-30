//! `e7_hillnormals MAP.Map.Gbx NAME_SUBSTR [NAME_SUBSTR …]` — THE FACING OF AN ITEM'S LM GEOMETRY AS PLACED (E7, 2026-09-30; V6-1c:
//! the g23 hills' down-facing texel bands take the game's brightest light): per matching model (by file name) and its placements,
//! over every triangle: the world GEOMETRIC normal (from the winding, p0→p1→p2, right-hand rule), the STORED normals (the vertex
//! stream's, rotated by the placement), their agreement (sign of geometric·stored), the AREA fractions pointing down (n.y < 0)
//! by either normal, and where the down-facing area sits in height (relative to the placement's y). A model whose stored normals
//! disagree with its winding, or whose down-facing area is high on the item, says where a two-sided / flipped rule would bite.
fn main() {
    let a: Vec<String> = std::env::args().collect();
    if a.len() < 3 { eprintln!("usage: e7_hillnormals MAP.Map.Gbx NAME_SUBSTR …"); std::process::exit(2); }
    let scene = lightmap::geometry::Scene::from_map(&a[1]).unwrap_or_else(|e| panic!("{}: {e}", a[1]));
    let names: Vec<&str> = a[2..].iter().map(|s| s.as_str()).collect();
    println!("model\tplacement (x, y, z)\tpose\ttris\tarea m²\tgeom·stored<0 (area %)\tdown by GEOMETRIC (area %)\tdown by STORED (area %)\tmean y of down-facing − placement y\tmean y of up-facing − placement y\tstored n·y mean");
    for (ii, inst) in scene.instances.iter().enumerate() {
        if !names.iter().any(|n| inst.model_name.contains(n)) { continue; }
        let m = &scene.models[inst.model];
        let (mut area, mut a_disagree, mut a_down_g, mut a_down_s, mut y_down, mut y_up, mut a_up, mut ny_sum) = (0f64, 0f64, 0f64, 0f64, 0f64, 0f64, 0f64, 0f64);
        for t in &m.tris {
            let p: Vec<[f32; 3]> = t.p.iter().map(|q| lightmap::geometry::xf_point(&inst.xf, *q)).collect();
            let e1 = [p[1][0] - p[0][0], p[1][1] - p[0][1], p[1][2] - p[0][2]];
            let e2 = [p[2][0] - p[0][0], p[2][1] - p[0][1], p[2][2] - p[0][2]];
            let g = [e1[1] * e2[2] - e1[2] * e2[1], e1[2] * e2[0] - e1[0] * e2[2], e1[0] * e2[1] - e1[1] * e2[0]];
            let ar = 0.5 * (g[0] * g[0] + g[1] * g[1] + g[2] * g[2]).sqrt() as f64;
            if ar <= 0.0 { continue; }
            let gn = [g[0] / (2.0 * ar as f32), g[1] / (2.0 * ar as f32), g[2] / (2.0 * ar as f32)];
            let sn: Vec<[f32; 3]> = t.n.iter().map(|n| lightmap::geometry::xf_normal(&inst.xf, *n)).collect();
            let s = [(sn[0][0] + sn[1][0] + sn[2][0]) / 3.0, (sn[0][1] + sn[1][1] + sn[2][1]) / 3.0, (sn[0][2] + sn[1][2] + sn[2][2]) / 3.0];
            let cy = (p[0][1] + p[1][1] + p[2][1]) as f64 / 3.0;
            area += ar;
            if gn[0] * s[0] + gn[1] * s[1] + gn[2] * s[2] < 0.0 { a_disagree += ar; }
            if gn[1] < 0.0 { a_down_g += ar; }
            if s[1] < 0.0 { a_down_s += ar; y_down += ar * cy; } else { a_up += ar; y_up += ar * cy; }
            ny_sum += ar * s[1] as f64;
        }
        let py = inst.pose.pos[1] as f64;
        println!("{}\t({:.0}, {:.0}, {:.0})\t{:?}\t{}\t{:.0}\t{:.1}\t{:.1}\t{:.1}\t{:+.1}\t{:+.1}\t{:+.3}", inst.model_name, inst.pose.pos[0], inst.pose.pos[1], inst.pose.pos[2], inst.pose, m.tris.len(), area, 100.0 * a_disagree / area.max(1e-9), 100.0 * a_down_g / area.max(1e-9), 100.0 * a_down_s / area.max(1e-9), if a_down_s > 0.0 { y_down / a_down_s - py } else { 0.0 }, if a_up > 0.0 { y_up / a_up - py } else { 0.0 }, ny_sum / area.max(1e-9));
        let _ = ii;
    }
}
