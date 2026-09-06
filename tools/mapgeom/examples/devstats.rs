//! Stats over devall TSV (stdin). Reports per-mat counts, magnitude bands, scale-error test.
use std::collections::BTreeMap;
use std::io::Read;
fn main() {
    let mut s = String::new();
    std::io::stdin().read_to_string(&mut s).unwrap();
    let mut permat: BTreeMap<String, usize> = BTreeMap::new();
    let mut bands = [0usize; 5]; // <1e-7,1e-7,1e-6,1e-5,>=1e-4
    let mut uniq: BTreeMap<(u32,u32,u32), usize> = BTreeMap::new();
    let mut n = 0;
    // scale test: d_i / src_i ratios for |src|>0.1
    let mut ratios: Vec<f64> = Vec::new();
    for (i, ln) in s.lines().enumerate() {
        if i == 0 { continue; }
        let c: Vec<&str> = ln.split('\t').collect();
        if c.len() < 10 { continue; }
        let (hx, hy, hz): (f64,f64,f64) = (c[1].parse().unwrap(), c[2].parse().unwrap(), c[3].parse().unwrap());
        let (mx, my, mz): (f64,f64,f64) = (c[4].parse().unwrap(), c[5].parse().unwrap(), c[6].parse().unwrap());
        let (dx, dy, dz) = (hx-mx, hy-my, hz-mz);
        let mag = (dx*dx+dy*dy+dz*dz).sqrt();
        *permat.entry(c[0].to_string()).or_insert(0) += 1;
        bands[if mag<1e-7{0}else if mag<1e-6{1}else if mag<1e-5{2}else if mag<1e-4{3}else{4}] += 1;
        // quantize his pos to 1um for uniqueness
        uniq.entry((((hx*1e6).round() as i64) as u32, ((hy*1e6).round() as i64) as u32, ((hz*1e6).round() as i64) as u32)).or_insert(0);
        n += 1;
        for (d, src2) in [(dx,mx*2.0),(dy,my*2.0),(dz,mz*2.0)] {
            if src2.abs() > 0.5 && d.abs() > 1e-9 { ratios.push(d/src2); }
        }
    }
    println!("n={n} unique_his_um_pos={}", uniq.len());
    println!("permat: {permat:?}");
    println!("bands <1e-7,1e-7-1e-6,1e-6-1e-5,1e-5-1e-4,>=1e-4: {bands:?}");
    ratios.sort_by(|a,b| a.partial_cmp(b).unwrap());
    if !ratios.is_empty() {
        println!("ratio d5/d50/d95: {:.3e} {:.3e} {:.3e} (n={})", ratios[ratios.len()/20.min(ratios.len()-1)], ratios[ratios.len()/2], ratios[ratios.len()*19/20.min(ratios.len()-1)], ratios.len());
        // mode: histogram around common values
        let mut hist: BTreeMap<i64, usize> = BTreeMap::new();
        for r in &ratios { hist.entry((r*1e9).round() as i64).or_insert(0); *hist.get_mut(&((r*1e9).round() as i64)).unwrap() += 1; }
        let mut hv: Vec<(i64, usize)> = hist.into_iter().collect();
        hv.sort_by_key(|(_,c)| std::cmp::Reverse(*c));
        println!("top ratio buckets (x1e-9): {:?}", &hv[..hv.len().min(6)]);
    }
}
