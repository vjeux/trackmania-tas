//! `e7_ray MAP.Map.Gbx ox,oy,oz dx,dy,dz [--max T] [--min T]` — every triangle of the placed scene a ray from `o` along `d` crosses, sorted
//! by distance: the model, the instance's placement, the hit point, the triangle's world normal and whether the hit is a front or a
//! back face (n·d), the material link / cut-out (alpha) index (E7, 2026-09-30: what our k39 item layers at a hill texel's pixel hold
//! 1 km along D where the game keeps the dome).
fn main() {
    let a: Vec<String> = std::env::args().collect();
    if a.len() < 4 { eprintln!("usage: e7_ray MAP.Map.Gbx ox,oy,oz dx,dy,dz [--max T] [--min T]"); std::process::exit(2); }
    let f = |k: &str| a.iter().position(|x| x == k).and_then(|i| a.get(i + 1)).cloned();
    let v3 = |s: &str| -> [f32; 3] { let v: Vec<f32> = s.split(',').filter_map(|t| t.trim().parse().ok()).collect(); [v[0], v[1], v[2]] };
    let o = v3(&a[2]);
    let d = v3(&a[3]);
    let l = (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt();
    let d = [d[0] / l, d[1] / l, d[2] / l];
    let tmax: f32 = f("--max").and_then(|v| v.parse().ok()).unwrap_or(1e9);
    let tmin: f32 = f("--min").and_then(|v| v.parse().ok()).unwrap_or(-1e9);
    let scene = lightmap::geometry::Scene::from_map(&a[1]).unwrap_or_else(|e| panic!("{}: {e}", a[1]));
    let mut hits: Vec<(f32, String)> = Vec::new();
    for inst in &scene.instances {
        let m = &scene.models[inst.model];
        for (ti, t) in m.tris.iter().enumerate() {
            let p: Vec<[f32; 3]> = t.p.iter().map(|q| lightmap::geometry::xf_point(&inst.xf, *q)).collect();
            // Möller–Trumbore
            let e1 = [p[1][0] - p[0][0], p[1][1] - p[0][1], p[1][2] - p[0][2]];
            let e2 = [p[2][0] - p[0][0], p[2][1] - p[0][1], p[2][2] - p[0][2]];
            let pv = [d[1] * e2[2] - d[2] * e2[1], d[2] * e2[0] - d[0] * e2[2], d[0] * e2[1] - d[1] * e2[0]];
            let det = e1[0] * pv[0] + e1[1] * pv[1] + e1[2] * pv[2];
            if det.abs() < 1e-12 { continue; }
            let inv = 1.0 / det;
            let tv = [o[0] - p[0][0], o[1] - p[0][1], o[2] - p[0][2]];
            let u = (tv[0] * pv[0] + tv[1] * pv[1] + tv[2] * pv[2]) * inv;
            if !(0.0..=1.0).contains(&u) { continue; }
            let qv = [tv[1] * e1[2] - tv[2] * e1[1], tv[2] * e1[0] - tv[0] * e1[2], tv[0] * e1[1] - tv[1] * e1[0]];
            let v = (d[0] * qv[0] + d[1] * qv[1] + d[2] * qv[2]) * inv;
            if v < 0.0 || u + v > 1.0 { continue; }
            let tt = (e2[0] * qv[0] + e2[1] * qv[1] + e2[2] * qv[2]) * inv;
            if tt < tmin || tt > tmax { continue; }
            let g = [e1[1] * e2[2] - e1[2] * e2[1], e1[2] * e2[0] - e1[0] * e2[2], e1[0] * e2[1] - e1[1] * e2[0]];
            let gl = (g[0] * g[0] + g[1] * g[1] + g[2] * g[2]).sqrt().max(1e-12);
            let gn = [g[0] / gl, g[1] / gl, g[2] / gl];
            let nd = gn[0] * d[0] + gn[1] * d[1] + gn[2] * d[2];
            let hp = [o[0] + tt * d[0], o[1] + tt * d[1], o[2] + tt * d[2]];
            let mat = m.mat_links.get(t.mat as usize).map(|s| format!("{s:?}")).unwrap_or_else(|| format!("mat {}", t.mat));
            hits.push((tt, format!("t {tt:9.2}  {}  placed ({:.0}, {:.0}, {:.0})  hit ({:.1}, {:.1}, {:.1})  n ({:+.2}, {:+.2}, {:+.2}) n·d {:+.3} → {}  tri {ti} {mat} alpha {} diff {}", inst.model_name, inst.pose.pos[0], inst.pose.pos[1], inst.pose.pos[2], hp[0], hp[1], hp[2], gn[0], gn[1], gn[2], nd, if nd < 0.0 { "FRONT (faces the camera side)" } else { "back" }, if t.alpha == u16::MAX { "-".to_string() } else { t.alpha.to_string() }, if t.diff == u16::MAX { "-".to_string() } else { t.diff.to_string() })));
        }
    }
    hits.sort_by(|x, y| x.0.partial_cmp(&y.0).unwrap());
    println!("{} hits from ({:.1}, {:.1}, {:.1}) along ({:.3}, {:.3}, {:.3}), t in [{tmin}, {tmax}]", hits.len(), o[0], o[1], o[2], d[0], d[1], d[2]);
    for (_, s) in &hits { println!("{s}"); }
}
