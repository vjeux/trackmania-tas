// asmshape -- function-level shape index over a flat `objdump -d` text.
//
// Built for one question: does a single Trackmania binary carry TWO copies of
// the vehicle solver (one per physics era)? A second era would show up as
// near-duplicate float-heavy functions inside one binary. This tool finds them.
//
//   asmshape dups  FILE            [--min 150] [--fmin 0.20] [--thresh 0.97]
//   asmshape match FILE_A FILE_B   [--min 150] [--fmin 0.20] [--thresh 0.97]
//   asmshape stats FILE            [--min 150] [--fmin 0.20]
//
// `dups`  : near-duplicate pairs WITHIN one binary (the test).
// `match` : best cross-binary match per function (the POSITIVE CONTROL -- the
//           same similarity measure must pair a 2022 function with its 2026
//           self, or a null `dups` result proves nothing).
//
// Shape = the histogram of instruction mnemonics, cosine-compared. Addresses,
// immediates and rip-relative targets are ignored on purpose: a recompiled or
// relocated copy of the same code keeps its mnemonic profile.
use std::collections::HashMap;

struct Func {
    addr: String,
    n: usize,
    fl: usize,
    hist: Vec<(u32, f32)>, // (mnemonic id, count), sorted by id
    norm: f32,
}

fn is_float(m: &str) -> bool {
    m.starts_with("mulss") || m.starts_with("addss") || m.starts_with("subss")
        || m.starts_with("divss") || m.starts_with("movss") || m.starts_with("mulps")
        || m.starts_with("addps") || m.starts_with("subps") || m.starts_with("movaps")
        || m.starts_with("movups") || m.starts_with("sqrtss") || m.starts_with("cvtsi")
        || m.starts_with("cvttss") || m.starts_with("comiss") || m.starts_with("ucomiss")
        || m.starts_with("maxss") || m.starts_with("minss") || m.starts_with("shufps")
        || m.starts_with("andps") || m.starts_with("xorps") || m.starts_with("unpck")
}

fn cosine(a: &Func, b: &Func) -> f32 {
    let (mut i, mut j, mut dot) = (0usize, 0usize, 0f32);
    while i < a.hist.len() && j < b.hist.len() {
        let (ka, kb) = (a.hist[i].0, b.hist[j].0);
        if ka == kb {
            dot += a.hist[i].1 * b.hist[j].1;
            i += 1;
            j += 1;
        } else if ka < kb {
            i += 1;
        } else {
            j += 1;
        }
    }
    dot / (a.norm * b.norm)
}

fn parse(path: &str, ids: &mut HashMap<String, u32>) -> Vec<Func> {
    let text = std::fs::read_to_string(path).expect("read");
    let mut out: Vec<Func> = Vec::new();
    let mut cur: HashMap<u32, f32> = HashMap::new();
    let mut addr = String::new();
    let mut n = 0usize;
    let mut fl = 0usize;
    let mut flush = |addr: &mut String, cur: &mut HashMap<u32, f32>, n: &mut usize, fl: &mut usize, out: &mut Vec<Func>| {
        if *n > 0 {
            let mut hist: Vec<(u32, f32)> = cur.iter().map(|(k, v)| (*k, *v)).collect();
            hist.sort_by_key(|x| x.0);
            let norm = hist.iter().map(|x| x.1 * x.1).sum::<f32>().sqrt();
            out.push(Func { addr: addr.clone(), n: *n, fl: *fl, hist, norm });
        }
        cur.clear();
        *n = 0;
        *fl = 0;
        addr.clear();
    };
    for line in text.lines() {
        // "  4011f0:\tmov    rax,rbx"
        let Some((left, right)) = line.split_once(":\t") else { continue };
        let a = left.trim();
        if a.is_empty() || !a.chars().all(|c| c.is_ascii_hexdigit()) {
            continue;
        }
        let mn = right.split_whitespace().next().unwrap_or("");
        if mn.is_empty() {
            continue;
        }
        if mn == "int3" || mn == "(bad)" {
            flush(&mut addr, &mut cur, &mut n, &mut fl, &mut out);
            continue;
        }
        if addr.is_empty() {
            addr = a.to_string();
        }
        let next = ids.len() as u32;
        let id = *ids.entry(mn.to_string()).or_insert(next);
        *cur.entry(id).or_insert(0.0) += 1.0;
        n += 1;
        if is_float(mn) {
            fl += 1;
        }
    }
    flush(&mut addr, &mut cur, &mut n, &mut fl, &mut out);
    out
}

fn main() {
    let a: Vec<String> = std::env::args().skip(1).collect();
    if a.is_empty() {
        eprintln!("usage: asmshape (dups FILE | match A B | stats FILE) [--min N] [--fmin F] [--thresh T]");
        std::process::exit(2);
    }
    let mut min = 150usize;
    let mut fmin = 0.20f32;
    let mut thresh = 0.97f32;
    let mut pos: Vec<String> = Vec::new();
    let mut i = 0;
    while i < a.len() {
        match a[i].as_str() {
            "--min" => { min = a[i + 1].parse().unwrap(); i += 2 }
            "--fmin" => { fmin = a[i + 1].parse().unwrap(); i += 2 }
            "--thresh" => { thresh = a[i + 1].parse().unwrap(); i += 2 }
            _ => { pos.push(a[i].clone()); i += 1 }
        }
    }
    let mut ids: HashMap<String, u32> = HashMap::new();
    let keep = |f: &Func| f.n >= min && (f.fl as f32) / (f.n as f32) >= fmin;
    match pos[0].as_str() {
        "stats" => {
            let fs = parse(&pos[1], &mut ids);
            let k: Vec<&Func> = fs.iter().filter(|f| keep(f)).collect();
            println!("functions {} ; float-heavy(>= {} insns, >= {:.0}% float) {}", fs.len(), min, fmin * 100.0, k.len());
            println!("total insns {}", fs.iter().map(|f| f.n).sum::<usize>());
        }
        "dups" => {
            let fs = parse(&pos[1], &mut ids);
            let k: Vec<&Func> = fs.iter().filter(|f| keep(f)).collect();
            eprintln!("comparing {} float-heavy functions", k.len());
            let mut pairs = 0;
            for x in 0..k.len() {
                for y in (x + 1)..k.len() {
                    let c = cosine(k[x], k[y]);
                    if c >= thresh {
                        println!("{:.4}  {} ({} insns)  <->  {} ({} insns)", c, k[x].addr, k[x].n, k[y].addr, k[y].n);
                        pairs += 1;
                    }
                }
            }
            eprintln!("pairs >= {}: {}", thresh, pairs);
        }
        "match" => {
            let fa = parse(&pos[1], &mut ids);
            let fb = parse(&pos[2], &mut ids);
            let ka: Vec<&Func> = fa.iter().filter(|f| keep(f)).collect();
            let kb: Vec<&Func> = fb.iter().filter(|f| keep(f)).collect();
            eprintln!("A {} float-heavy, B {} float-heavy", ka.len(), kb.len());
            let mut exact = 0;
            for x in ka.iter() {
                let mut best = 0f32;
                let mut bi = usize::MAX;
                for (j, y) in kb.iter().enumerate() {
                    let c = cosine(x, y);
                    if c > best { best = c; bi = j; }
                }
                if best >= thresh { exact += 1; }
                if bi != usize::MAX {
                    println!("{:.4}  A:{} ({})  ->  B:{} ({})", best, x.addr, x.n, kb[bi].addr, kb[bi].n);
                }
            }
            eprintln!("A functions with a B match >= {}: {} of {}", thresh, exact, ka.len());
        }
        other => { eprintln!("unknown command {}", other); std::process::exit(2) }
    }
}
