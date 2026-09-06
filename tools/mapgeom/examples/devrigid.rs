//! Cluster deviations by quantized delta. Usage: devrigid < devall.tsv
use std::collections::BTreeMap;
use std::io::Read;
fn main() {
    let mut s = String::new();
    std::io::stdin().read_to_string(&mut s).unwrap();
    // delta quantized to 0.5um; key=(qdx,qdy,qdz)
    let mut groups: BTreeMap<(i64,i64,i64), Vec<(String,f64,f64,f64)>> = BTreeMap::new();
    for (i, ln) in s.lines().enumerate() {
        if i == 0 { continue; }
        let c: Vec<&str> = ln.split('\t').collect();
        if c.len() < 10 { continue; }
        let (hx, hy, hz): (f64,f64,f64) = (c[1].parse().unwrap(), c[2].parse().unwrap(), c[3].parse().unwrap());
        let (mx, my, mz): (f64,f64,f64) = (c[4].parse().unwrap(), c[5].parse().unwrap(), c[6].parse().unwrap());
        let (dx, dy, dz) = (hx-mx, hy-my, hz-mz);
        let q = ((dx*2e6).round() as i64, (dy*2e6).round() as i64, (dz*2e6).round() as i64);
        groups.entry(q).or_default().push((c[0].to_string(), hx, hy, hz));
    }
    let mut gv: Vec<((i64,i64,i64), Vec<(String,f64,f64,f64)>)> = groups.into_iter().collect();
    gv.sort_by_key(|(_,v)| std::cmp::Reverse(v.len()));
    println!("{} distinct delta groups:", gv.len());
    for (q, v) in gv.iter().take(20) {
        // bounding box of his positions + mats
        let mut mats: BTreeMap<String, usize> = BTreeMap::new();
        let (mut lo, mut hi): ([f64;3],[f64;3]) = ([1e9;3],[ -1e9;3]);
        for (m,x,y,z) in v {
            *mats.entry(m.clone()).or_insert(0) += 1;
            lo[0]=lo[0].min(*x); lo[1]=lo[1].min(*y); lo[2]=lo[2].min(*z);
            hi[0]=hi[0].max(*x); hi[1]=hi[1].max(*y); hi[2]=hi[2].max(*z);
        }
        println!("  d=({:+.1e},{:+.1e},{:+.1e}) n={} mats={:?} bbox=[{:.3},{:.3},{:.3}]-[{:.3},{:.3},{:.3}]",
            q.0 as f64/2e6, q.1 as f64/2e6, q.2 as f64/2e6, v.len(), mats, lo[0],lo[1],lo[2], hi[0],hi[1],hi[2]);
    }
}
