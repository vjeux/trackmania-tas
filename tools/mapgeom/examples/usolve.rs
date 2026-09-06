//! Solve U formula: source faces at a spot vs his U frames. Usage: usolve SRC.ITEM HIS.HALFITEM HX HY HZ NX NY NZ U1X U1Y U1Z ...
//! (his frames passed for reference; tool prints candidate U per source tri)
fn main() {
    let a: Vec<String> = std::env::args().collect();
    let data = std::fs::read(&a[1]).unwrap();
    let g = tmmaps::gbx::Gbx::parse(&data);
    let loc = mapgeom::crystal_model::locate(&g.body).unwrap();
    let (c, _, _) = mapgeom::crystal_model::CPlugCrystal::parse_with(&g.body, loc.at, loc.lookback.clone()).unwrap();
    let hq: [f32;3] = [a[3].parse().unwrap(), a[4].parse().unwrap(), a[5].parse().unwrap()];
    // source query = ? (need t; approximate by searching near hq*2)
    let sq = [hq[0]*2.0, hq[1]*2.0, hq[2]*2.0];
    println!("search near source ({:.4},{:.4},{:.4})", sq[0], sq[1], sq[2]);
    let layers = mapgeom::static_item::bake::geometry_layers(&c);
    for (cr, vis, _col) in &layers {
        if !vis { continue; }
        for f in &cr.faces {
            // any vert within 0.1 of sq?
            let mut near = false;
            for vi in &f.verts {
                let p = cr.positions[*vi as usize];
                let d = ((p[0]-sq[0]).powi(2)+(p[1]-sq[1]).powi(2)+(p[2]-sq[2]).powi(2)).sqrt();
                if d < 0.1 { near = true; break; }
            }
            if !near { continue; }
            let pts: Vec<[f32;3]> = f.verts.iter().map(|i| cr.positions[*i as usize]).collect();
            let fuv = cr.face_uvs(f);
            println!("face nv={} mat={}", pts.len(), f.material);
            for (i, p) in pts.iter().enumerate() {
                let uv = fuv.get(i).copied().unwrap_or([0.0,0.0]);
                println!("   p=({:.6},{:.6},{:.6}) uv=({:.6},{:.6})", p[0], p[1], p[2], uv[0], uv[1]);
            }
        }
    }
}
