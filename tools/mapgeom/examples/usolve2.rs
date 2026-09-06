//! U formula solver at a half-size position. Usage: usolve2 SRC.ITEM HX HY HZ NX NY NZ
//! Finds source faces mapping near (hx,hy,hz), fans them, prints per-tri (uvgrad, NewellN)
//! and candidate U values (raw, GS-smoothN, GS-faceN) for comparison with his frames.
fn sub(a: [f32;3], b: [f32;3]) -> [f32;3] { [a[0]-b[0], a[1]-b[1], a[2]-b[2]] }
fn norm(v: [f32;3]) -> [f32;3] {
    let l = (v[0]*v[0]+v[1]*v[1]+v[2]*v[2]).sqrt().max(1e-30);
    [v[0]/l, v[1]/l, v[2]/l]
}
fn main() {
    let a: Vec<String> = std::env::args().collect();
    let data = std::fs::read(&a[1]).unwrap();
    let g = tmmaps::gbx::Gbx::parse(&data);
    let loc = mapgeom::crystal_model::locate(&g.body).unwrap();
    let (c, _, _) = mapgeom::crystal_model::CPlugCrystal::parse_with(&g.body, loc.at, loc.lookback.clone()).unwrap();
    let hq: [f32;3] = [a[2].parse().unwrap(), a[3].parse().unwrap(), a[4].parse().unwrap()];
    let sn: [f32;3] = [a[5].parse().unwrap(), a[6].parse().unwrap(), a[7].parse().unwrap()];
    // t for Road_17 (half-size = src*0.5 + t)
    let t = [f32::from_bits(0xbb018000), f32::from_bits(0xb7000000), f32::from_bits(0xbcbe2c00)];
    // source query = (hq - t)*2
    let sq = [(hq[0]-t[0])*2.0, (hq[1]-t[1])*2.0, (hq[2]-t[2])*2.0];
    println!("src query ({:.5},{:.5},{:.5}) smoothN=({:.4},{:.4},{:.4})", sq[0], sq[1], sq[2], sn[0], sn[1], sn[2]);
    let layers = mapgeom::static_item::bake::geometry_layers(&c);
    for (cr, vis, _col) in &layers {
        if !vis { continue; }
        for f in &cr.faces {
            let pts: Vec<[f32;3]> = f.verts.iter().map(|i| cr.positions[*i as usize]).collect();
            // map to half-size
            let hps: Vec<[f32;3]> = pts.iter().map(|p| [p[0]*0.5+t[0], p[1]*0.5+t[1], p[2]*0.5+t[2]]).collect();
            let mut near = false;
            for p in &hps {
                let d = ((p[0]-hq[0]).powi(2)+(p[1]-hq[1]).powi(2)+(p[2]-hq[2]).powi(2)).sqrt();
                if d < 1e-3 { near = true; break; }
            }
            if !near { continue; }
            let fuv = cr.face_uvs(f);
            // fan tris
            let mut tris: Vec<(Vec<[f32;3]>, Vec<[f32;2]>)> = Vec::new();
            if pts.len() == 3 {
                tris.push((hps.clone(), fuv.clone()));
            } else {
                for i in 2..pts.len() {
                    tris.push((vec![hps[1], hps[i], hps[(i+1)%pts.len()]], vec![fuv[1], fuv[i], fuv[(i+1)%fuv.len()]]));
                }
            }
            for (tp, tu_) in &tris {
                // only tris touching hq
                if !tp.iter().any(|p| ((p[0]-hq[0]).powi(2)+(p[1]-hq[1]).powi(2)+(p[2]-hq[2]).powi(2)).sqrt() < 1e-3) { continue; }
                let e1 = sub(tp[1], tp[0]);
                let e2 = sub(tp[2], tp[0]);
                let (du1, dv1, du2, dv2) = (tu_[1][0]-tu_[0][0], tu_[1][1]-tu_[0][1], tu_[2][0]-tu_[0][0], tu_[2][1]-tu_[0][1]);
                let det = du1*dv2-du2*dv1;
                // face normal (cross, half-size)
                let crx = [e1[1]*e2[2]-e1[2]*e2[1], e1[2]*e2[0]-e1[0]*e2[2], e1[0]*e2[1]-e1[1]*e2[0]];
                let l = (crx[0]*crx[0]+crx[1]*crx[1]+crx[2]*crx[2]).sqrt().max(1e-30);
                let fn_ = [crx[0]/l, crx[1]/l, crx[2]/l];
                println!("tri fn=({:.4},{:.4},{:.4}) det={:.3e} uvs=({:.4},{:.4})({:.4},{:.4})({:.4},{:.4})",
                    fn_[0], fn_[1], fn_[2], det, tu_[0][0], tu_[0][1], tu_[1][0], tu_[1][1], tu_[2][0], tu_[2][1]);
                if det.abs() > 1e-12 {
                    let r = 1.0/det;
                    let raw = norm([(e1[0]*dv2-e2[0]*dv1)*r, (e1[1]*dv2-e2[1]*dv1)*r, (e1[2]*dv2-e2[2]*dv1)*r]);
                    // GS vs smoothed N
                    let d = raw[0]*sn[0]+raw[1]*sn[1]+raw[2]*sn[2];
                    let gs = norm([raw[0]-d*sn[0], raw[1]-d*sn[1], raw[2]-d*sn[2]]);
                    // GS vs face N
                    let d2 = raw[0]*fn_[0]+raw[1]*fn_[1]+raw[2]*fn_[2];
                    let gf = norm([raw[0]-d2*fn_[0], raw[1]-d2*fn_[1], raw[2]-d2*fn_[2]]);
                    println!("   raw=({:.4},{:.4},{:.4}) GSsmooth=({:.4},{:.4},{:.4}) GSface=({:.4},{:.4},{:.4})",
                        raw[0], raw[1], raw[2], gs[0], gs[1], gs[2], gf[0], gf[1], gf[2]);
                    // f64 path (his pipeline may use doubles)
                    let e1d = [[e1[0] as f64, e1[1] as f64, e1[2] as f64]];
                    let e2d = [e2[0] as f64, e2[1] as f64, e2[2] as f64];
                    let dud = [[du1 as f64, dv1 as f64], [du2 as f64, dv2 as f64]];
                    let detd = dud[0][0]*dud[1][1]-dud[1][0]*dud[0][1];
                    let rd = 1.0/detd;
                    let txd = (e1d[0][0]*dud[1][1]-e2d[0]*dud[0][1])*rd;
                    let tyd = (e1d[0][1]*dud[1][1]-e2d[1]*dud[0][1])*rd;
                    let tzd = (e1d[0][2]*dud[1][1]-e2d[2]*dud[0][1])*rd;
                    let ld = (txd*txd+tyd*tyd+tzd*tzd).sqrt().max(1e-30);
                    let (txd, tyd, tzd) = (txd/ld, tyd/ld, tzd/ld);
                    let snd = [sn[0] as f64, sn[1] as f64, sn[2] as f64];
                    let dd = txd*snd[0]+tyd*snd[1]+tzd*snd[2];
                    let (oxd, oyd, ozd) = (txd-dd*snd[0], tyd-dd*snd[1], tzd-dd*snd[2]);
                    let l2d = (oxd*oxd+oyd*oyd+ozd*ozd).sqrt().max(1e-30);
                    println!("   f64 GSsmooth=({:.4},{:.4},{:.4})", oxd/l2d, oyd/l2d, ozd/l2d);
                    // V-primary variants: V from dv-grad, U derived.
                    // V raw (dv gradient, normalized)
                    let vx = (e1[0]*du2-e2[0]*du1)*r;
                    let vy = (e1[1]*du2-e2[1]*du1)*r;
                    let vz = (e1[2]*du2-e2[2]*du1)*r;
                    let vl = (vx*vx+vy*vy+vz*vz).sqrt().max(1e-30);
                    let vr = [vx/vl, vy/vl, vz/vl];
                    // U = norm(cross(V_raw, N_smooth))
                    let cx1 = [vr[1]*sn[2]-vr[2]*sn[1], vr[2]*sn[0]-vr[0]*sn[2], vr[0]*sn[1]-vr[1]*sn[0]];
                    let cl1 = (cx1[0]*cx1[0]+cx1[1]*cx1[1]+cx1[2]*cx1[2]).sqrt().max(1e-30);
                    // U = norm(cross(N_smooth, V_raw))
                    let cx2 = [sn[1]*vr[2]-sn[2]*vr[1], sn[2]*vr[0]-sn[0]*vr[2], sn[0]*vr[1]-sn[1]*vr[0]];
                    // U = GS(du vs V_raw) then GS vs N
                    let dotv = raw[0]*vr[0]+raw[1]*vr[1]+raw[2]*vr[2];
                    let px = raw[0]-dotv*vr[0];
                    let py = raw[1]-dotv*vr[1];
                    let pz = raw[2]-dotv*vr[2];
                    let pl = (px*px+py*py+pz*pz).sqrt().max(1e-30);
                    let (px, py, pz) = (px/pl, py/pl, pz/pl);
                    let dq = px*sn[0]+py*sn[1]+pz*sn[2];
                    let qx = px-dq*sn[0];
                    let qy = py-dq*sn[1];
                    let qz = pz-dq*sn[2];
                    let ql = (qx*qx+qy*qy+qz*qz).sqrt().max(1e-30);
                    println!("   Vraw=({:.4},{:.4},{:.4}) UxV=({:.4},{:.4},{:.4}) NxV=({:.4},{:.4},{:.4}) UperpV=({:.4},{:.4},{:.4})",
                        vr[0], vr[1], vr[2], cx1[0]/cl1, cx1[1]/cl1, cx1[2]/cl1, cx2[0], cx2[1], cx2[2], qx/ql, qy/ql, qz/ql);
                    println!("   his U4=(0.002,-0.387,-0.920)");
                }
            }
        }
    }
}
