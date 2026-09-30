//! `e7_facing VB.bin STRIDE POS_OFF VSOUT.bin VSOUT_STRIDE INDICES.bin [dx dy dz]` — THE FACING OF A CAPTURED DRAW'S TRIANGLES,
//! AS D3D SAW THEM vs the right-hand-rule normal (E7, 2026-09-30; RE 17's 15:00Z read: the game's env layer blacks the WhiteShore
//! skirt from above and shows it fog-lit from below → the skirt's FRONT face is its underside).
//!
//! Per triangle of the draw: the screen-space signed area from the post-VS SV_Position (x/w, y/w in NDC, y up), the right-hand
//! normal n = (p1 − p0) × (p2 − p0) from the input vertex positions, and n·d for the camera's forward d (the third COLUMN of
//! GbxV_WorldToCamera = the direction the camera looks along, or the given vector). THE HARDWARE CALIBRATES THE SIGN: RE 17's pixel
//! debugger on e19327 (frame 557, pixel (2503, 214)) read isfrontface = 0 on a skirt triangle whose NDC area is < 0 and whose n·d > 0
//! (the camera below looking up at the underside) — so on these cameras (FrontCounterClockwise TRUE, the projection's x scale
//! NEGATIVE) D3D's FRONT is the face with NDC area > 0 = n·d < 0 = the right-hand normal pointing TOWARD the camera, exactly the
//! port's `dot(cross(e1,e2), d) < 0` (peel.rs). The census prints the pairing so a new capture can be checked the same way.
fn main() {
    let a: Vec<String> = std::env::args().collect();
    if a.len() < 7 { eprintln!("usage: e7_facing VB.bin STRIDE POS_OFF VSOUT.bin VSOUT_STRIDE INDICES.bin [dx dy dz]"); std::process::exit(2); }
    let vb = std::fs::read(&a[1]).expect("vb");
    let stride: usize = a[2].parse().expect("stride");
    let pos_off: usize = a[3].parse().expect("pos off");
    let vo = std::fs::read(&a[4]).expect("vsout");
    let vstride: usize = a[5].parse().expect("vsout stride");
    let ib = std::fs::read(&a[6]).expect("indices");
    let d: Option<[f32; 3]> = if a.len() >= 10 { Some([a[7].parse().unwrap(), a[8].parse().unwrap(), a[9].parse().unwrap()]) } else { None };
    let f32_at = |b: &[u8], o: usize| f32::from_le_bytes(b[o..o + 4].try_into().unwrap());
    let nv = vb.len() / stride;
    let nvo = vo.len() / vstride;
    let idx: Vec<u16> = ib.chunks_exact(2).map(|c| u16::from_le_bytes([c[0], c[1]])).collect();
    eprintln!("{nv} input vertices (stride {stride}), {nvo} post-VS vertices (stride {vstride}), {} indices = {} triangles", idx.len(), idx.len() / 3);
    let pos = |i: usize| -> [f32; 3] { let o = i * stride + pos_off; [f32_at(&vb, o), f32_at(&vb, o + 4), f32_at(&vb, o + 8)] };
    let clip = |i: usize| -> [f32; 4] { let o = i * vstride; [f32_at(&vo, o), f32_at(&vo, o + 4), f32_at(&vo, o + 8), f32_at(&vo, o + 12)] };
    // the census: (screen sign, n·d sign) pairs, and n.y's sign per screen sign
    let mut tab = std::collections::BTreeMap::<(i8, i8), usize>::new();
    let mut ny_by_front = std::collections::BTreeMap::<(bool, i8), usize>::new();
    let mut w_minmax = (f32::MAX, f32::MIN);
    // the depth axis: NDC z against the vertex's world position projected on d (which way does z grow along d?)
    let mut zd: Vec<(f32, f32)> = Vec::new();
    if let Some(d) = d {
        for i in 0..nv.min(nvo) { let p = pos(i); let c = clip(i); zd.push((p[0] * d[0] + p[1] * d[1] + p[2] * d[2], c[2] / c[3])); }
        zd.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());
        if zd.len() >= 2 { let (lo, hi) = (zd[0], zd[zd.len() - 1]); println!("depth axis: p·d {:.1} → NDC z {:.5}; p·d {:.1} → NDC z {:.5} ({} along d)", lo.0, lo.1, hi.0, hi.1, if hi.1 > lo.1 { "z GROWS" } else { "z FALLS" }); }
    }
    let mut shown = 0;
    for t in 0..idx.len() / 3 {
        let (i0, i1, i2) = (idx[3 * t] as usize, idx[3 * t + 1] as usize, idx[3 * t + 2] as usize);
        let (p0, p1, p2) = (pos(i0), pos(i1), pos(i2));
        let e1 = [p1[0] - p0[0], p1[1] - p0[1], p1[2] - p0[2]];
        let e2 = [p2[0] - p0[0], p2[1] - p0[1], p2[2] - p0[2]];
        let n = [e1[1] * e2[2] - e1[2] * e2[1], e1[2] * e2[0] - e1[0] * e2[2], e1[0] * e2[1] - e1[1] * e2[0]];
        let (c0, c1, c2) = (clip(i0), clip(i1), clip(i2));
        for c in [c0, c1, c2] { w_minmax.0 = w_minmax.0.min(c[3]); w_minmax.1 = w_minmax.1.max(c[3]); }
        let ndc = |c: [f32; 4]| [c[0] / c[3], c[1] / c[3]];
        let (q0, q1, q2) = (ndc(c0), ndc(c1), ndc(c2));
        let area = (q1[0] - q0[0]) * (q2[1] - q0[1]) - (q2[0] - q0[0]) * (q1[1] - q0[1]);
        let front_d3d = area > 0.0; // the hardware (isfrontface = 0 at area < 0 on e19327): FRONT = NDC area > 0 on these cameras
        let ssign = if area > 0.0 { 1 } else if area < 0.0 { -1 } else { 0 };
        let nd = d.map(|d| n[0] * d[0] + n[1] * d[1] + n[2] * d[2]);
        let ndsign = nd.map(|v| if v > 0.0 { 1i8 } else if v < 0.0 { -1 } else { 0 }).unwrap_or(0);
        *tab.entry((ssign, ndsign)).or_default() += 1;
        let nys = if n[1] > 0.0 { 1i8 } else if n[1] < 0.0 { -1 } else { 0 };
        *ny_by_front.entry((front_d3d, nys)).or_default() += 1;
        if shown < 6 { shown += 1; eprintln!("  tri {t}: p0 ({:.1}, {:.1}, {:.1}) n_rh ({:.3e}, {:.3e}, {:.3e}) ndc area {area:.3e} → D3D front {front_d3d}; n·d {:?}", p0[0], p0[1], p0[2], n[0], n[1], n[2], nd); }
    }
    println!("clip w range [{:.4}, {:.4}] (1 = orthographic)", w_minmax.0, w_minmax.1);
    println!("(screen area sign in y-up NDC, sign of n_rh·d) → triangles: {tab:?}");
    println!("(D3D front with FrontCCW=TRUE, sign of n_rh.y) → triangles: {ny_by_front:?}");
    println!("RULE (hardware-calibrated on e19327: isfrontface = 0 where NDC area < 0): D3D front ⇔ NDC area > 0 ⇔ n_rh·d < 0 — the right-hand normal toward the camera, as the port's `dot(cross(e1,e2), d) < 0`; a table pairing area>0 with n·d>0 would mean a differently-handed camera");
}
