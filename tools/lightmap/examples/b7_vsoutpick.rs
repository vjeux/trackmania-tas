//! `b7_vsoutpick VSOUT.bin INDICES.bin [--stride 96] [--vp X,Y,W,H] [--n 3]` — from a RenderDoc `mesh` export of a
//! draw (post-VS positions, float4 clip-space at offset 0 of each vertex; u16 indices) list N triangles' centroids as
//! VIEWPORT PIXELS (after the perspective divide; y down), so a `pixeldbg` export can debug a pixel this draw really
//! covers (baker-7, 2026-09-30: the terrain draw e19327 of g23's frame 557 does not cover (2048, 2048)).
fn main() {
    let a: Vec<String> = std::env::args().skip(1).collect();
    let f = |k: &str| a.iter().position(|x| x == k).and_then(|i| a.get(i + 1)).cloned();
    let stride: usize = f("--stride").map(|v| v.parse().unwrap()).unwrap_or(96);
    let n: usize = f("--n").map(|v| v.parse().unwrap()).unwrap_or(3);
    let vp: Vec<f32> = f("--vp").map(|v| v.split(',').map(|x| x.parse().unwrap()).collect()).unwrap_or_else(|| vec![1.0, 1.0, 4094.0, 4094.0]);
    let pos: Vec<String> = a.iter().filter(|x| !x.starts_with("--")).cloned().collect();
    let vb = std::fs::read(&pos[0]).expect("vsout");
    let ib = std::fs::read(&pos[1]).expect("indices");
    let nv = vb.len() / stride;
    let idx: Vec<u16> = ib.chunks_exact(2).map(|c| u16::from_le_bytes([c[0], c[1]])).collect();
    let v = |i: usize| -> [f32; 4] { let o = i * stride; let g = |k: usize| f32::from_le_bytes([vb[o + 4 * k], vb[o + 4 * k + 1], vb[o + 4 * k + 2], vb[o + 4 * k + 3]]); [g(0), g(1), g(2), g(3)] };
    println!("{nv} vertices, {} indices, {} triangles; viewport {:?}", idx.len(), idx.len() / 3, vp);
    let mut shown = 0;
    let mut best: Vec<(f32, usize, [f32; 2], [f32; 3])> = Vec::new();
    for t in 0..idx.len() / 3 {
        let p: Vec<[f32; 4]> = (0..3).map(|k| v(idx[3 * t + k] as usize)).collect();
        if p.iter().any(|q| q[3].abs() < 1e-6 || !q[3].is_finite()) { continue; }
        let ndc: Vec<[f32; 3]> = p.iter().map(|q| [q[0] / q[3], q[1] / q[3], q[2] / q[3]]).collect();
        let cx = (ndc[0][0] + ndc[1][0] + ndc[2][0]) / 3.0;
        let cy = (ndc[0][1] + ndc[1][1] + ndc[2][1]) / 3.0;
        let cz = (ndc[0][2] + ndc[1][2] + ndc[2][2]) / 3.0;
        if cx.abs() > 1.0 || cy.abs() > 1.0 { continue; }
        // screen-space area (pixels²) as the pick order: the biggest triangles first (a centroid safely inside)
        let sx = |x: f32| vp[0] + (x * 0.5 + 0.5) * vp[2];
        let sy = |y: f32| vp[1] + (1.0 - (y * 0.5 + 0.5)) * vp[3];
        let (ax, ay, bx, by, qx, qy) = (sx(ndc[0][0]), sy(ndc[0][1]), sx(ndc[1][0]), sy(ndc[1][1]), sx(ndc[2][0]), sy(ndc[2][1]));
        let area = ((bx - ax) * (qy - ay) - (qx - ax) * (by - ay)).abs() * 0.5;
        best.push((area, t, [sx(cx), sy(cy)], [cx, cy, cz]));
    }
    best.sort_by(|x, y| y.0.partial_cmp(&x.0).unwrap());
    for (area, t, px, ndc) in best.iter() {
        if shown >= n { break; }
        println!("tri {t:5}: area {area:12.1} px²  pixel ({:.0}, {:.0})  ndc ({:.4}, {:.4}, z {:.5})", px[0], px[1], ndc[0], ndc[1], ndc[2]);
        shown += 1;
    }
    println!("{} triangles inside the clip box", best.len());
}
