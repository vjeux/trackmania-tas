use std::collections::BTreeMap;
use std::convert::TryInto;
use std::env;
use std::fs;

const REC: usize = 8352;
const CAR: usize = 64 + 0x60;

#[derive(Clone, Copy)]
struct Sample { pos: [f64; 3] }
fn u32at(b: &[u8], o: usize) -> u32 { u32::from_le_bytes(b[o..o+4].try_into().unwrap()) }
fn f32at(b: &[u8], o: usize) -> f64 { f32::from_bits(u32at(b, o)) as f64 }
fn load(path: &str, off: usize) -> BTreeMap<u32, Sample> {
    let bytes = fs::read(path).unwrap();
    assert_eq!(bytes.len() % REC, 0);
    let mut out = BTreeMap::new();
    for rec in bytes.chunks_exact(REC) {
        let t = u32at(rec, 56);
        let c = &rec[CAR..];
        out.entry(t).or_insert(Sample { pos: [f32at(c, off), f32at(c, off+4), f32at(c, off+8)] });
    }
    out
}
fn dist(a: [f64;3], b: [f64;3]) -> f64 {
    let d = [a[0]-b[0], a[1]-b[1], a[2]-b[2]];
    (d[0]*d[0]+d[1]*d[1]+d[2]*d[2]).sqrt()
}
fn t0(m: &BTreeMap<u32, Sample>) -> u32 {
    let p0 = m.values().next().unwrap().pos;
    m.iter().find(|(_,s)| dist(s.pos,p0)>0.05).map(|(&t,_)|t).unwrap()
}
fn sweep(a_name:&str,a:&BTreeMap<u32,Sample>,b_name:&str,b:&BTreeMap<u32,Sample>) {
    let a0=t0(a) as i64; let b0=t0(b) as i64;
    let mut rows=Vec::new();
    // Fixed 2..18 s window leaves room for the entire +/-1 s sweep.
    for lag in (-1000i64..=1000).step_by(10) {
        let mut sum=0.0; let mut max=0.0f64; let mut n=0usize;
        for rel in (2000i64..=18000).step_by(10) {
            let (Some(sa),Some(sb))=(a.get(&((a0+rel) as u32)),b.get(&((b0+rel+lag) as u32))) else { continue; };
            let d=dist(sa.pos,sb.pos); sum+=d; max=max.max(d); n+=1;
        }
        assert_eq!(n,1601);
        rows.push((sum/n as f64,max,lag));
    }
    rows.sort_by(|x,y|x.0.partial_cmp(&y.0).unwrap());
    let (mean,max,lag)=rows[0];
    let zero=rows.iter().find(|r|r.2==0).unwrap();
    println!("SWEEP {a_name} vs {b_name} fixed_window_s=2..18 best_lag_ms={lag} best_mean_pos_m={mean:.9} best_max_pos_m={max:.9} zero_lag_mean_pos_m={:.9} zero_lag_max_pos_m={:.9}",zero.0,zero.1);
}
fn main(){
    let a=env::args().collect::<Vec<_>>(); assert_eq!(a.len(),7);
    let off=|s:&str|usize::from_str_radix(s.trim_start_matches("0x"),16).unwrap();
    let exact=load(&a[1],off(&a[2])); let stock=load(&a[3],off(&a[4])); let v5=load(&a[5],off(&a[6]));
    sweep("exact",&exact,"stock",&stock); sweep("exact",&exact,"v5",&v5); sweep("stock",&stock,"v5",&v5);
}
