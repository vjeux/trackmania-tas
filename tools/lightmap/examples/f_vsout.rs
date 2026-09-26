//! `f_vsout DIR EID [W H]` — the post-VS export of one local-light draw (stsun f4936 `mesh/e<EID>_vsout.bin`, stride 80:
//! o0 clip xyzw, o1 world xyz, o2 xyzw, o3 normal xyz, o4 xyz, o5 xyz): per instance (the draw's index count splits the
//! vertices) the clip → pixel range on the W × H target and the world box (engineer F).
fn main() {
    let a: Vec<String> = std::env::args().collect();
    let dir = &a[1];
    let eid: u32 = a[2].parse().unwrap();
    let w: f32 = a.get(3).filter(|v| !v.starts_with('-')).map(|v| v.parse().unwrap()).unwrap_or(3072.0);
    let h: f32 = a.get(4).filter(|v| !v.starts_with('-')).map(|v| v.parse().unwrap()).unwrap_or(2048.0);
    let vs = std::fs::read(format!("{dir}/mesh/e{eid:06}_vsout.bin")).expect("vsout");
    let idx = std::fs::read(format!("{dir}/mesh/e{eid:06}_vsout_indices.bin")).expect("indices");
    let n = vs.len() / 80;
    let ni = idx.len() / 2;
    let f = |i: usize, k: usize| f32::from_le_bytes(vs[i * 80 + k * 4..i * 80 + k * 4 + 4].try_into().unwrap());
    let maxi = (0..ni).map(|i| u16::from_le_bytes([idx[2 * i], idx[2 * i + 1]]) as usize).max().unwrap_or(0);
    let per = maxi + 1;
    let inst = if per > 0 { n / per } else { 1 };
    println!("eid {eid}: {n} vertices, {ni} indices (max index {maxi} → {per} vertices per instance, {inst} instances)");
    for ii in 0..inst {
        let (mut px0, mut px1, mut py0, mut py1) = (f32::MAX, f32::MIN, f32::MAX, f32::MIN);
        let (mut b0, mut b1) = ([f32::MAX; 3], [f32::MIN; 3]);
        let mut zw = (0.0f32, 0.0f32);
        let mut o2 = [0.0f32; 4];
        for v in ii * per..(ii + 1) * per {
            let (cx, cy) = (f(v, 0), f(v, 1));
            zw = (f(v, 2), f(v, 3));
            let px = (cx + 1.0) * 0.5 * w;
            let py = (1.0 - cy) * 0.5 * h;
            px0 = px0.min(px); px1 = px1.max(px); py0 = py0.min(py); py1 = py1.max(py);
            for k in 0..3 { let p = f(v, 4 + k); b0[k] = b0[k].min(p); b1[k] = b1[k].max(p); }
            o2 = [f(v, 7), f(v, 8), f(v, 9), f(v, 10)];
        }
        println!("  inst {ii}: px [{px0:.3}, {px1:.3}] py [{py0:.3}, {py1:.3}] zw {zw:?} world [{:.3} {:.3} {:.3}]..[{:.3} {:.3} {:.3}] o2 {o2:?}", b0[0], b0[1], b0[2], b1[0], b1[1], b1[2]);
    }
    if a.iter().any(|x| x == "-v") {
        for v in 0..n.min(a.iter().position(|x| x == "-n").and_then(|i| a.get(i + 1)).and_then(|s| s.parse().ok()).unwrap_or(12)) {
            println!("  v{v}: clip ({:.6}, {:.6}) world ({:.4}, {:.4}, {:.4}) n ({:.4}, {:.4}, {:.4})", f(v, 0), f(v, 1), f(v, 4), f(v, 5), f(v, 6), f(v, 11), f(v, 12), f(v, 13));
        }
    }
}
