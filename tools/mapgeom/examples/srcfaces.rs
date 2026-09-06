//! Source crystal faces near a full-size position. Usage: srcfaces SRC.ITEM X Y Z
fn main() {
    let a: Vec<String> = std::env::args().collect();
    let data = std::fs::read(&a[1]).unwrap();
    let g = tmmaps::gbx::Gbx::parse(&data);
    let loc = mapgeom::crystal_model::locate(&g.body).unwrap();
    let (c, _, _) = mapgeom::crystal_model::CPlugCrystal::parse_with(&g.body, loc.at, loc.lookback.clone()).unwrap();
    let q: [f32;3] = [a[2].parse().unwrap(), a[3].parse().unwrap(), a[4].parse().unwrap()];
    println!("crystal materials={}", c.materials.len());
    let layers = mapgeom::static_item::bake::geometry_layers(&c);
    for (cr, vis, col) in &layers {
        println!("- layer crystal version={} nfaces={}", cr.version, cr.faces.len());
        // material names for face.material indices: crystal's own materials
        for f in &cr.faces {
            let mut near = false;
            for vi in &f.verts {
                let p = cr.positions[*vi as usize];
                let d = ((p[0]-q[0]).powi(2)+(p[1]-q[1]).powi(2)+(p[2]-q[2]).powi(2)).sqrt();
                if d < 5e-2 { near = true; break; }
            }
            if !near { continue; }
            let matname = c.materials.get(f.material.max(0) as usize).map(|m| m.name.clone()).unwrap_or("?".into());
            // Newell
            let pts: Vec<[f32;3]> = f.verts.iter().map(|i| cr.positions[*i as usize]).collect();
            let mut n = [0f32; 3];
            for i in 0..pts.len() {
                let aa = pts[i];
                let b = pts[(i + 1) % pts.len()];
                n[0] += (aa[1] - b[1]) * (aa[2] + b[2]);
                n[1] += (aa[2] - b[2]) * (aa[0] + b[0]);
                n[2] += (aa[0] - b[0]) * (aa[1] + b[1]);
            }
            let l = (n[0]*n[0]+n[1]*n[1]+n[2]*n[2]).sqrt().max(1e-30);
            // cross of first tri
            let e1 = [pts[1][0]-pts[0][0], pts[1][1]-pts[0][1], pts[1][2]-pts[0][2]];
            let e2 = [pts[2][0]-pts[0][0], pts[2][1]-pts[0][1], pts[2][2]-pts[0][2]];
            let crx = [e1[1]*e2[2]-e1[2]*e2[1], e1[2]*e2[0]-e1[0]*e2[2], e1[0]*e2[1]-e1[1]*e2[0]];
            let lc = (crx[0]*crx[0]+crx[1]*crx[1]+crx[2]*crx[2]).sqrt().max(1e-30);
            println!("mat={} nv={} vis={} col={} u01={:?}", matname, f.verts.len(), vis, col, f.u01);
            println!("   newell=({:.5},{:.5},{:.5}) cross=({:.5},{:.5},{:.5})", n[0]/l, n[1]/l, n[2]/l, crx[0]/lc, crx[1]/lc, crx[2]/lc);
            for vi in &f.verts {
                let p = cr.positions[*vi as usize];
                println!("   v=({:.6},{:.6},{:.6})", p[0], p[1], p[2]);
            }
            let fuv = cr.face_uvs(f);
            for (i, vi) in f.verts.iter().enumerate() {
                let uv = fuv.get(i).copied().unwrap_or([0.0, 0.0]);
                println!("   uv[{}]=({:.6},{:.6})", vi, uv[0], uv[1]);
            }
        }
    }
}
