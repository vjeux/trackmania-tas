//! `e6_tracelayers TRACE.log [--top N] [--all]` — the per-direction LAYER TABLE of one `LMTOOL_SET_TEXEL_TRACE` bake log (the WORLD
//! peel lines): for every direction with n·d > 0 whose read is `none` or black, the receiver's stored z, every layer's stored depth as
//! `Δ = (z_r − d)·65535` in R16 units (Δ > 0: the layer is IN FRONT of the receiver toward the light and PASSES; Δ ≤ 0: behind it,
//! fails), the layer's colour, and the winner — E6 2026-09-29 for RE 17's box 1(g) (the accumulate's last-writer rule at the g23 hill
//! texel 068: which layer kills each lost direction and by how many depth units). Sorted by the cosine weight lost (n·d).
fn main() {
    let a: Vec<String> = std::env::args().collect();
    let txt = std::fs::read_to_string(&a[1]).unwrap_or_else(|e| panic!("{}: {e}", a[1]));
    let f = |k: &str| a.iter().position(|x| x == k).and_then(|i| a.get(i + 1)).cloned();
    let top: usize = f("--top").and_then(|v| v.parse().ok()).unwrap_or(20);
    let all = a.iter().any(|x| x == "--all");
    let lines: Vec<&str> = txt.lines().collect();
    struct Dir { k: u32, line: usize, d: [f32; 3], ndl: f32, z_r: f32, layers: Vec<(f32, bool, [f32; 3])>, result: String, nfrag: usize }
    let mut dirs: Vec<Dir> = Vec::new();
    let num = |s: &str| -> f32 { s.trim().parse().unwrap_or(f32::NAN) };
    let mut i = 0;
    while i < lines.len() {
        let l = lines[i];
        if let Some(rest) = l.strip_prefix("set-texel-trace: dir ") {
            if rest.contains(" world peel, ") {
                let k: u32 = rest.split(' ').next().unwrap_or("0").parse().unwrap_or(0);
                let d: Vec<f32> = rest[rest.find('(').unwrap() + 1..rest.find(')').unwrap()].split(',').map(num).collect();
                let nfrag: usize = rest.split(" layers (walk").next().and_then(|s| s.rsplit(", ").next()).and_then(|s| s.parse().ok()).unwrap_or(0);
                // the frag line follows; then the → L line
                let frag = lines.get(i + 1).copied().unwrap_or("");
                let mut z_r = f32::NAN;
                let mut n = [0f32; 3];
                let mut layers = Vec::new();
                if let Some(p) = frag.find(" n (") {
                    let s = &frag[p + 4..];
                    let v: Vec<f32> = s[..s.find(')').unwrap()].split(',').map(num).collect();
                    n = [v[0], v[1], v[2]];
                }
                if let Some(p) = frag.find("→ z ") {
                    let s = &frag[p + "→ z ".len()..];
                    z_r = num(s.split(',').next().unwrap_or(""));
                    let mut rest2 = s;
                    while let Some(q) = rest2.find("[L") {
                        let seg = &rest2[q..];
                        let end = seg.find(']').unwrap_or(seg.len());
                        let body = &seg[..end];
                        // "[L3 d 0.5800564 fail rgb (0.0000,0.0000,0.0000)"
                        let dv = body.find(" d ").map(|x| num(body[x + 3..].split(' ').next().unwrap_or(""))).unwrap_or(f32::NAN);
                        let pass = body.contains(" PASS ");
                        let rgb: [f32; 3] = body.find("rgb (").map(|x| { let s2 = &body[x + 5..]; let v: Vec<f32> = s2[..s2.find(')').unwrap_or(s2.len())].split(',').map(num).collect(); [v[0], v[1], v[2]] }).unwrap_or([f32::NAN; 3]);
                        layers.push((dv, pass, rgb));
                        rest2 = &seg[end..];
                    }
                }
                let res_line = lines.iter().skip(i + 1).take(4).find(|x| x.contains(&format!("dir {k} (")) && x.contains("world peel → L")).copied().unwrap_or("");
                let result = res_line.split("→ L ").nth(1).unwrap_or("?").to_string();
                let ndl = n[0] * d[0] + n[1] * d[1] + n[2] * d[2];
                dirs.push(Dir { k, line: i + 1, d: [d[0], d[1], d[2]], ndl, z_r, layers, result, nfrag });
            }
        }
        i += 1;
    }
    let lost = |r: &str| r.starts_with("None") || r.contains("Some([0.0, 0.0, 0.0])") || r.contains("Some([0.0, 9.5");
    let mut rows: Vec<&Dir> = dirs.iter().filter(|x| x.ndl > 0.0 && (all || lost(&x.result))).collect();
    rows.sort_by(|p, q| q.ndl.partial_cmp(&p.ndl).unwrap());
    let front: Vec<&Dir> = dirs.iter().filter(|x| x.ndl > 0.0).collect();
    let cos_all: f32 = front.iter().map(|x| x.ndl).sum();
    let cos_lost: f32 = front.iter().filter(|x| lost(&x.result)).map(|x| x.ndl).sum();
    println!("{}: {} world-peel directions, {} front-facing (Σcos {cos_all:.3}), {} lost (none/black, Σcos {cos_lost:.3} = {:.1} %)", a[1], dirs.len(), front.len(), front.iter().filter(|x| lost(&x.result)).count(), 100.0 * cos_lost / cos_all.max(1e-9));
    println!("dir    n·d    elev°   z_r        frags  layers: Δ = (z_r − d)·65535 [R16 units] (PASS/fail) colour … → result");
    for x in rows.iter().take(top) {
        let el = x.d[1].clamp(-1.0, 1.0).asin().to_degrees();
        let ls: Vec<String> = x.layers.iter().enumerate().map(|(j, (d, p, c))| format!("L{j}:{:+.0}{}({:.2},{:.2},{:.2})", (x.z_r - d) * 65535.0, if *p { "P" } else { "f" }, c[0], c[1], c[2])).collect();
        println!("{:>4} l{:<6} D({:.4},{:.4},{:.4}) {:.3}  {:>6.1}  {:.6}  {:>4}  {}  → {}", x.k, x.line, x.d[0], x.d[1], x.d[2], x.ndl, el, x.z_r, x.nfrag, ls.join(" "), x.result);
    }
}
