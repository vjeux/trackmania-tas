//! `v5_shadowpoints CENSUS.tsv --name SUB1,SUB2,… --dir DX,DZ --elev DEG [--steps N] [--min-y Y] [--heights MODEL=H,…]` — the SUN-SHADOW
//! FOOTPRINT point list of the tall items (E6 21:12Z, V5 2026-09-29): for every census placement whose name contains `--name` (and
//! whose y ≥ `--min-y`), the ground points along the shadow direction (DX, DZ) (the xz projection of −sun, normalised here) from the
//! placement to the shadow's tip at L = H / tan(elev), where H is the model's height (`--heights`, else `--default-height`). The list
//! feeds `classcmp --near FILE --radius R`: the tiles NEAR the footprints (in the tall hills' shadows) vs the open tiles — if ours are
//! darker there too, the sun map is where the term lives. Output: a TSV with x, z, name (the caster), s (the fraction of L).
fn main() {
    let a: Vec<String> = std::env::args().collect();
    let f = |k: &str| a.iter().position(|x| x == k).and_then(|i| a.get(i + 1)).cloned();
    let census = a.get(1).expect("CENSUS.tsv");
    let names: Vec<String> = f("--name").map(|s| s.split(',').map(|x| x.trim().to_string()).filter(|x| !x.is_empty()).collect()).unwrap_or_default();
    let (dx, dz) = { let s = f("--dir").expect("--dir DX,DZ"); let (x, z) = s.split_once(',').expect("--dir DX,DZ"); let (x, z): (f64, f64) = (x.trim().parse().unwrap(), z.trim().parse().unwrap()); let n = (x * x + z * z).sqrt().max(1e-9); (x / n, z / n) };
    let elev: f64 = f("--elev").expect("--elev DEG").parse().unwrap();
    let steps: usize = f("--steps").map(|v| v.parse().unwrap()).unwrap_or(5);
    let min_y: f64 = f("--min-y").map(|v| v.parse().unwrap()).unwrap_or(f64::NEG_INFINITY);
    let default_h: f64 = f("--default-height").map(|v| v.parse().unwrap()).unwrap_or(300.0);
    let heights: std::collections::HashMap<String, f64> = f("--heights").map(|s| s.split(',').filter_map(|kv| kv.split_once('=').map(|(k, v)| (k.trim().to_string(), v.trim().parse::<f64>().unwrap()))).collect()).unwrap_or_default();
    let txt = std::fs::read_to_string(census).unwrap_or_else(|e| panic!("{census}: {e}"));
    let mut lines = txt.lines();
    let head: Vec<&str> = lines.next().unwrap_or("").split('\t').collect();
    let col = |n: &str| head.iter().position(|h| *h == n).unwrap_or_else(|| panic!("no column {n}"));
    let (cn, cx, cy, cz) = (col("name"), col("x"), col("y"), col("z"));
    println!("x\tz\tname\ts");
    let (mut casters, mut points) = (0usize, 0usize);
    for l in lines {
        let v: Vec<&str> = l.split('\t').collect();
        let Some(n) = v.get(cn) else { continue };
        if !names.is_empty() && !names.iter().any(|k| n.contains(k.as_str())) { continue; }
        let (Some(x), Some(y), Some(z)) = (v.get(cx).and_then(|s| s.parse::<f64>().ok()), v.get(cy).and_then(|s| s.parse::<f64>().ok()), v.get(cz).and_then(|s| s.parse::<f64>().ok())) else { continue };
        if y < min_y { continue; }
        let h = heights.iter().find(|(k, _)| n.contains(k.as_str())).map(|(_, v)| *v).unwrap_or(default_h);
        let len = h / elev.to_radians().tan();
        casters += 1;
        for i in 1..=steps { let s = i as f64 / steps as f64; println!("{:.1}\t{:.1}\t{}\t{:.2}", x + dx * len * s, z + dz * len * s, n, s); points += 1; }
    }
    eprintln!("{casters} casters (name ~ {names:?}, y ≥ {min_y}), {points} footprint points; shadow dir ({dx:.3}, {dz:.3}), L = H / tan {elev}°");
}
