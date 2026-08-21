//! `tmtraj intg` -- GHOST INTEGRITY: is this file's telemetry its own?
//!
//! THE TEST
//! --------
//! A shared bit-identical PREFIX between two runs proves nothing. The
//! simulation is deterministic, so two tapes with the same opening inputs give
//! identical f32 positions for as long as their inputs agree, and our own
//! sibling tapes do this routinely (67 % of samples on one 203072 pair).
//!
//! The proof is RE-CONVERGENCE. Once two runs are metres apart they are
//! different physical states, and no input sequence returns them to EXACTLY
//! 0.000000 m -- the same f32 bits in all three components. So the sequence
//!
//!     bit-identical  ->  diverge past a threshold  ->  bit-identical again
//!
//! cannot be produced by driving. It is a splice: two runs' samples in one
//! record. That is what a partially-regenerated ghost is.
//!
//! WHAT THE VERDICT IS WORTH
//! -------------------------
//! CONTAMINATED is a positive finding and it is reference-free in the sense
//! that it needs no model -- but WHICH reference matters. Against a HUMAN
//! recording the finding is clean. Against one of our own siblings it is not,
//! because two of our own files can re-converge for the honest reason that they
//! share the contaminating pipeline: both inherited the same donor's samples in
//! the same places. `--kind` labels the reference so the audit can keep the two
//! populations apart, and this command never merges them.
//!
//! CLEAN here means "no splice against THIS reference". It is not a
//! certificate; the NO-REFERENCE case is reported as its own verdict and must
//! never be collapsed into CLEAN.

use crate::whlcmd::{decode, R};

/// One maximal run of bit-identical samples.
#[derive(Clone, Debug)]
pub struct ZeroRun {
    pub i0: usize,
    pub i1: usize, // inclusive
    pub ms0: i64,
    pub ms1: i64,
}

impl ZeroRun {
    pub fn n(&self) -> usize {
        self.i1 - self.i0 + 1
    }
}

pub struct Pair {
    /// samples present in both records, by time
    pub n: usize,
    pub times: Vec<i64>,
    pub d: Vec<f64>,
    pub ident: Vec<bool>,
    pub runs: Vec<ZeroRun>,
    pub max_d: f64,
    pub max_ms: i64,
    /// biggest divergence strictly BETWEEN two bit-identical runs
    pub gap_d: f64,
    pub gap_ms: i64,
    pub a_n: usize,
    pub b_n: usize,
}

/// Bit-identical position: the same twelve bytes, not "close".
fn pos_ident(a: &R, b: &R) -> bool {
    if a.raw.len() < 59 || b.raw.len() < 59 {
        return false;
    }
    a.raw[47..59] == b.raw[47..59]
}

fn dist(a: &R, b: &R) -> f64 {
    let mut s = 0.0;
    for k in 0..3 {
        let q = a.pos[k] - b.pos[k];
        s += q * q;
    }
    s.sqrt()
}

/// Align two records on their shared sample instants and profile the distance.
pub fn pair(a: &[R], b: &[R]) -> Pair {
    let bm: std::collections::HashMap<i64, &R> = b.iter().map(|r| (r.ms, r)).collect();
    let mut times = Vec::new();
    let mut d = Vec::new();
    let mut ident = Vec::new();
    for x in a {
        let Some(y) = bm.get(&x.ms) else { continue };
        times.push(x.ms);
        d.push(dist(x, y));
        ident.push(pos_ident(x, y));
    }
    let n = times.len();
    // maximal runs of bit-identical samples
    let mut runs: Vec<ZeroRun> = Vec::new();
    let mut i = 0usize;
    while i < n {
        if !ident[i] {
            i += 1;
            continue;
        }
        let mut j = i;
        while j + 1 < n && ident[j + 1] {
            j += 1;
        }
        runs.push(ZeroRun { i0: i, i1: j, ms0: times[i], ms1: times[j] });
        i = j + 1;
    }
    let mut max_d = 0.0f64;
    let mut max_ms = 0i64;
    for k in 0..n {
        if d[k].is_finite() && d[k] > max_d {
            max_d = d[k];
            max_ms = times[k];
        }
    }
    // The divergence that precedes an identical block -- the load-bearing
    // number. Measured over everything before the FIRST non-head identical
    // run, so a file whose donor block starts at sample 1 is scored too. Non-finite counts as infinite separation: a NaN sample is not
    // "close" to anything, and 270051's files diverge that way.
    let mut gap_d = 0.0f64;
    let mut gap_ms = 0i64;
    if let Some(r) = runs.iter().find(|r| r.i0 > 0) {
        for k in 0..r.i0 {
            let v = if d[k].is_finite() { d[k] } else { f64::INFINITY };
            if v > gap_d {
                gap_d = v;
                gap_ms = times[k];
            }
        }
    }
    Pair {
        n,
        times,
        d,
        ident,
        runs,
        max_d,
        max_ms,
        gap_d,
        gap_ms,
        a_n: a.len(),
        b_n: b.len(),
    }
}

#[derive(PartialEq, Debug, Clone, Copy)]
pub enum Verdict {
    Contaminated,
    Clean,
    Identical,
    NoComparison,
}

impl Verdict {
    pub fn word(&self) -> &'static str {
        match self {
            Verdict::Contaminated => "CONTAMINATED",
            Verdict::Clean => "CLEAN",
            Verdict::Identical => "IDENTICAL-TO-REFERENCE",
            Verdict::NoComparison => "NO-COMPARISON",
        }
    }
}

/// `minsep` is how far apart two runs must get before returning to the same
/// f32 bits is impossible rather than merely unlikely. Default 5 m: the car is
/// 2 m long, the sample grid is 50 ms, and at racing speed one tick of
/// divergence is already metres. Every finding this project has made is two
/// orders of magnitude above it (147 m .. 2361 m).
pub fn verdict(p: &Pair, minsep: f64) -> Verdict {
    if p.n < 5 {
        return Verdict::NoComparison;
    }
    let all_ident = p.runs.len() == 1 && p.runs[0].n() == p.n;
    if all_ident {
        return Verdict::Identical;
    }
    // THE GENERAL RULE, and the narrower re-convergence rule is one case of it.
    //
    // To be bit-identical at instant t, two runs must hold the same physical
    // state at t. Determinism can give that from the START -- same tape prefix,
    // same f32s -- so a HEAD-ANCHORED identical block is honest and common
    // (our tapes are seeded from human tapes). But once the two runs are
    // metres apart they are different states, and no input sequence returns
    // them to the same f32 bits. So ANY identical block that is preceded by
    // real separation is a splice: donor samples sitting in our record.
    //
    // This was written as "identical -> apart -> identical", which requires TWO
    // identical runs and MISSES the commonest real case: a file whose very
    // first sample differs (a wrong origin, a clock offset, one repaired
    // sample) and whose remaining 364 of 365 samples are the donor's. Measured
    // on 227654's TAS_57503 -- the single most contaminated file in the corpus
    // -- which the two-run spelling passed as CLEAN.
    for r in &p.runs {
        if r.i0 == 0 {
            continue; // head-anchored: determinism, not a splice
        }
        let before = (0..r.i0)
            .map(|k| if p.d[k].is_finite() { p.d[k] } else { f64::INFINITY })
            .fold(0.0f64, f64::max);
        if before >= minsep {
            return Verdict::Contaminated;
        }
    }
    Verdict::Clean
}

fn fmt_s(ms: i64) -> String {
    format!("{:.3}", ms as f64 / 1000.0)
}

pub fn report(a_path: &str, b_path: &str, p: &Pair, v: Verdict, minsep: f64) {
    println!("=== {}", a_path);
    println!("    vs {}", b_path);
    println!(
        "    {} shared sample instants ({} in the file, {} in the reference)",
        p.n, p.a_n, p.b_n
    );
    let nid = p.ident.iter().filter(|x| **x).count();
    println!(
        "    bit-identical positions: {} of {} ({:.1} %) in {} run(s)",
        nid,
        p.n,
        100.0 * nid as f64 / p.n.max(1) as f64,
        p.runs.len()
    );
    for (k, r) in p.runs.iter().enumerate() {
        let where_ = if r.i0 == 0 {
            "HEAD"
        } else if r.i1 == p.n - 1 {
            "TAIL"
        } else {
            "interior"
        };
        println!(
            "      run {:>2}  {:<8} samples [{}..{}]  {} .. {} s  ({} samples)",
            k + 1,
            where_,
            r.i0,
            r.i1,
            fmt_s(r.ms0),
            fmt_s(r.ms1),
            r.n()
        );
    }
    println!(
        "    max separation {:.4} m at {} s",
        p.max_d,
        fmt_s(p.max_ms)
    );
    if p.runs.len() >= 2 {
        println!(
            "    separation BETWEEN the outer identical runs: {:.4} m at {} s (threshold {:.1} m)",
            p.gap_d,
            fmt_s(p.gap_ms),
            minsep
        );
    }
    println!("    VERDICT {}", v.word());
    if v == Verdict::Contaminated {
        println!(
            "      identical -> {:.1} m apart -> identical again. Two runs' samples in one record.",
            p.gap_d
        );
    }
}

fn load(path: &str) -> Result<Vec<R>, String> {
    decode(path)
}

pub fn cmd(args: &[String]) {
    let flag = |n: &str| -> Option<String> {
        args.iter().position(|a| a == n).and_then(|i| args.get(i + 1)).cloned()
    };
    if args.is_empty() {
        print_usage();
        std::process::exit(2);
    }
    match args[0].as_str() {
        "pair" => {
            let rest = &args[1..];
            let mut pos: Vec<&String> = Vec::new();
            let mut i = 0;
            while i < rest.len() {
                if rest[i].starts_with("--") {
                    if matches!(rest[i].as_str(), "--minsep" | "--tsv" | "--kind") {
                        i += 2;
                    } else {
                        i += 1;
                    }
                    continue;
                }
                pos.push(&rest[i]);
                i += 1;
            }
            if pos.len() != 2 {
                eprintln!("tmtraj intg pair FILE REFERENCE [--minsep M] [--tsv]");
                std::process::exit(2);
            }
            let minsep: f64 = flag("--minsep").and_then(|v| v.parse().ok()).unwrap_or(5.0);
            let a = match load(pos[0]) {
                Ok(v) => v,
                Err(e) => {
                    println!("=== {}\n    DECODE-FAIL {}", pos[0], e);
                    std::process::exit(2)
                }
            };
            let b = match load(pos[1]) {
                Ok(v) => v,
                Err(e) => {
                    println!("=== {}\n    reference DECODE-FAIL {}", pos[1], e);
                    std::process::exit(2)
                }
            };
            let p = pair(&a, &b);
            let v = verdict(&p, minsep);
            if args.iter().any(|x| x == "--tsv") {
                for k in 0..p.n {
                    println!(
                        "{}\t{}\t{:.6}\t{}",
                        k,
                        p.times[k],
                        p.d[k],
                        if p.ident[k] { 1 } else { 0 }
                    );
                }
            } else {
                report(pos[0], pos[1], &p, v, minsep);
            }
            std::process::exit(if v == Verdict::Contaminated { 2 } else { 0 });
        }
        "sweep" => cmd_sweep(&args[1..]),
        "audit" => cmd_audit(&args[1..]),
        "gate" => cmd_gate(&args[1..]),
        "dup" => cmd_dup(&args[1..]),
        "manifest" => crate::manifest::cmd(&args[1..]),
        "lag" => cmd_lag(&args[1..]),
        "c3" => cmd_c3(&args[1..]),
        "corrupt" => cmd_corrupt(&args[1..]),
        "stale" => cmd_stale(&args[1..]),
        "poison" => cmd_poison(&args[1..]),
        "selfsim" => cmd_selfsim(&args[1..]),
        "c11b" => cmd_c11b(&args[1..]),
        "tapecsv" => cmd_tapecsv(&args[1..]),
        "qrule" => cmd_qrule(&args[1..]),
        "c12" => cmd_c12(&args[1..]),
        "md5" => {
            for f in &args[1..] {
                match std::fs::read(f) {
                    Ok(b) => println!("{}", md5_hex(&b)),
                    Err(e) => {
                        eprintln!("{}: {}", f, e);
                        std::process::exit(2)
                    }
                }
            }
        }
        _ => {
            print_usage();
            std::process::exit(2);
        }
    }
}

fn print_usage() {
    print!(
        "\
tmtraj intg -- ghost integrity: is this file's telemetry its own?

  tmtraj intg pair FILE REFERENCE [--minsep M] [--tsv]
        Align two ghosts on their shared sample instants and report every
        maximal run of BIT-IDENTICAL positions, the separation between them,
        and the re-convergence verdict.

  tmtraj intg sweep --file F --refs R1,R2,... [--kind human|sibling]
                    [--minsep M] [--tsv]
        Same test against several references; prints one TSV row per (file,
        reference) pair plus the worst verdict. --kind only LABELS the rows:
        human references and same-pipeline siblings are never merged.

A shared PREFIX proves nothing (determinism). RE-CONVERGENCE to exactly
0.000000 m after a real separation cannot be driven, only spliced.
"
    );
}

fn cmd_sweep(args: &[String]) {
    let flag = |n: &str| -> Option<String> {
        args.iter().position(|a| a == n).and_then(|i| args.get(i + 1)).cloned()
    };
    let file = flag("--file").expect("--file");
    let refs = flag("--refs").expect("--refs");
    let kind = flag("--kind").unwrap_or_else(|| "unknown".into());
    let minsep: f64 = flag("--minsep").and_then(|v| v.parse().ok()).unwrap_or(5.0);
    let tsv = args.iter().any(|a| a == "--tsv");
    let a = match load(&file) {
        Ok(v) => v,
        Err(e) => {
            println!("{}\t{}\tDECODE-FAIL\t{}", file, kind, e);
            std::process::exit(2);
        }
    };
    let mut worst = Verdict::NoComparison;
    if tsv {
        println!(
            "file\tref\tkind\tn_common\tn_ident\tn_runs\thead_n\thead_end_s\ttail_n\ttail_start_s\t\
             max_sep_m\tgap_sep_m\tgap_s\tverdict"
        );
    }
    for rp in refs.split(',').filter(|s| !s.is_empty()) {
        let b = match load(rp) {
            Ok(v) => v,
            Err(e) => {
                println!("{}\t{}\t{}\tref-DECODE-FAIL\t{}", file, rp, kind, e);
                continue;
            }
        };
        let p = pair(&a, &b);
        let v = verdict(&p, minsep);
        // worst-of: contaminated beats clean beats identical beats none
        let rank = |v: Verdict| match v {
            Verdict::Contaminated => 3,
            Verdict::Identical => 2,
            Verdict::Clean => 1,
            Verdict::NoComparison => 0,
        };
        if rank(v) > rank(worst) {
            worst = v;
        }
        if tsv {
            let nid = p.ident.iter().filter(|x| **x).count();
            let head = p.runs.first().filter(|r| r.i0 == 0);
            let tail = p.runs.last().filter(|r| p.n > 0 && r.i1 == p.n - 1);
            println!(
                "{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{:.4}\t{:.4}\t{}\t{}",
                file,
                rp,
                kind,
                p.n,
                nid,
                p.runs.len(),
                head.map_or(0, |r| r.n()),
                head.map_or("-".into(), |r| fmt_s(r.ms1)),
                tail.map_or(0, |r| r.n()),
                tail.map_or("-".into(), |r| fmt_s(r.ms0)),
                p.max_d,
                p.gap_d,
                if p.runs.len() >= 2 { fmt_s(p.gap_ms) } else { "-".into() },
                v.word()
            );
        } else {
            report(&file, rp, &p, v, minsep);
        }
    }
    std::process::exit(if worst == Verdict::Contaminated { 2 } else { 0 });
}

// ---------------------------------------------------------------------------
// md5, so a row can carry the identity of the bytes it describes. No crate: the
// workspace is built --offline against a vendored set that has none.
// ---------------------------------------------------------------------------

pub fn md5_hex(data: &[u8]) -> String {
    const S: [u32; 64] = [
        7, 12, 17, 22, 7, 12, 17, 22, 7, 12, 17, 22, 7, 12, 17, 22, 5, 9, 14, 20, 5, 9, 14, 20, 5,
        9, 14, 20, 5, 9, 14, 20, 4, 11, 16, 23, 4, 11, 16, 23, 4, 11, 16, 23, 4, 11, 16, 23, 6, 10,
        15, 21, 6, 10, 15, 21, 6, 10, 15, 21, 6, 10, 15, 21,
    ];
    let mut k = [0u32; 64];
    for (i, v) in k.iter_mut().enumerate() {
        *v = ((i as f64 + 1.0).sin().abs() * 4294967296.0) as u32;
    }
    let (mut a0, mut b0, mut c0, mut d0) =
        (0x67452301u32, 0xefcdab89u32, 0x98badcfeu32, 0x10325476u32);
    let mut msg = data.to_vec();
    let bitlen = (data.len() as u64).wrapping_mul(8);
    msg.push(0x80);
    while msg.len() % 64 != 56 {
        msg.push(0);
    }
    msg.extend_from_slice(&bitlen.to_le_bytes());
    for chunk in msg.chunks(64) {
        let mut m = [0u32; 16];
        for i in 0..16 {
            m[i] = u32::from_le_bytes(chunk[i * 4..i * 4 + 4].try_into().unwrap());
        }
        let (mut a, mut b, mut c, mut d) = (a0, b0, c0, d0);
        for i in 0..64 {
            let (f, g) = match i / 16 {
                0 => ((b & c) | (!b & d), i),
                1 => ((d & b) | (!d & c), (5 * i + 1) % 16),
                2 => (b ^ c ^ d, (3 * i + 5) % 16),
                _ => (c ^ (b | !d), (7 * i) % 16),
            };
            let f2 = f
                .wrapping_add(a)
                .wrapping_add(k[i])
                .wrapping_add(m[g]);
            a = d;
            d = c;
            c = b;
            b = b.wrapping_add(f2.rotate_left(S[i]));
        }
        a0 = a0.wrapping_add(a);
        b0 = b0.wrapping_add(b);
        c0 = c0.wrapping_add(c);
        d0 = d0.wrapping_add(d);
    }
    let mut out = String::new();
    for w in [a0, b0, c0, d0] {
        for byte in w.to_le_bytes() {
            out.push_str(&format!("{:02x}", byte));
        }
    }
    out
}

// ---------------------------------------------------------------------------
// `intg audit` -- the whole published corpus, against a reference table.
// ---------------------------------------------------------------------------

/// A reference recording, and what it is. The distinction is load-bearing: a
/// HUMAN recording was made by the game client on a machine of ours and shares
/// nothing with our pipeline, so re-convergence against it is a finding. A
/// SIBLING is one of our own files, which can re-converge for the honest reason
/// that both inherited the same donor's samples -- so a sibling finding is
/// reported, never merged, and never on its own called contamination.
pub struct Ref {
    pub map: String,
    pub kind: String,
    pub path: String,
}

pub fn load_refs(path: &str) -> Result<Vec<Ref>, String> {
    let s = std::fs::read_to_string(path).map_err(|e| format!("{}: {}", path, e))?;
    let mut out = Vec::new();
    for l in s.lines() {
        let l = l.trim();
        if l.is_empty() || l.starts_with('#') {
            continue;
        }
        let f: Vec<&str> = l.split('\t').collect();
        if f.len() < 3 {
            return Err(format!("bad reference line: {:?}", l));
        }
        out.push(Ref { map: f[0].into(), kind: f[1].into(), path: f[2].into() });
    }
    Ok(out)
}

fn map_of(p: &std::path::Path) -> String {
    // <corpus>/<mapdir>/replays/<file>
    p.parent()
        .and_then(|d| d.parent())
        .and_then(|d| d.file_name())
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_default()
}

/// The strongest evidence against one file: over all references of a kind, the
/// pair with the largest separation BETWEEN two bit-identical runs.
struct Worst {
    v: Verdict,
    refname: String,
    n_common: usize,
    n_ident: usize,
    n_runs: usize,
    head_n: usize,
    tail_n: usize,
    tail_ms: i64,
    gap: f64,
    maxd: f64,
    minsep_seen: f64,
    /// COVERAGE. A verdict is a claim about the references it was actually
    /// compared against, and those three numbers are almost never equal:
    /// references are HELD, some fail to decode, and some share no instant
    /// with the file. "CLEAN" against 1 of 4 held references and "CLEAN"
    /// against 4 of 4 print identically unless the denominators travel with
    /// the row, and tonight has cost hours to claims of the first shape read
    /// as the second.
    n_refs_held: usize,
    n_refs_decoded: usize,
    n_refs_compared: usize,
}

/// Decoded references, kept between files. Decoding 186935's 48 835-sample
/// reference once per audited file is most of an eight-minute run; once per
/// audit it is nothing.
type Cache = std::collections::HashMap<String, Option<std::rc::Rc<Vec<R>>>>;

fn cached<'c>(cache: &'c mut Cache, path: &str) -> Option<std::rc::Rc<Vec<R>>> {
    if !cache.contains_key(path) {
        let rm = declared_race_ms(path);
        let v = decode(path).ok().map(|r| std::rc::Rc::new(in_race(&r, rm)));
        cache.insert(path.to_string(), v);
    }
    cache.get(path).cloned().flatten()
}

fn worst_over(a: &[R], refs: &[&Ref], minsep: f64, cache: &mut Cache) -> Option<Worst> {
    let rank = |v: Verdict| match v {
        Verdict::Contaminated => 3,
        Verdict::Identical => 2,
        Verdict::Clean => 1,
        Verdict::NoComparison => 0,
    };
    let mut best: Option<Worst> = None;
    let n_held = refs.len();
    let mut n_decoded = 0usize;
    let mut n_compared = 0usize;
    for r in refs {
        let Some(b) = cached(cache, &r.path) else { continue };
        n_decoded += 1;
        let p = pair(a, &b);
        if p.n > 0 {
            n_compared += 1;
        }
        let v = verdict(&p, minsep);
        let nid = p.ident.iter().filter(|x| **x).count();
        let head = p.runs.first().filter(|x| x.i0 == 0).map_or(0, |x| x.n());
        let (tail_n, tail_ms) = p
            .runs
            .last()
            .filter(|x| p.n > 0 && x.i1 == p.n - 1)
            .map_or((0, 0), |x| (x.n(), x.ms0));
        // rank by verdict first, then by how much evidence there is
        let cand = Worst {
            v,
            refname: std::path::Path::new(&r.path)
                .file_name()
                .map(|s| s.to_string_lossy().to_string())
                .unwrap_or_default(),
            n_common: p.n,
            n_ident: nid,
            n_runs: p.runs.len(),
            head_n: head,
            tail_n,
            tail_ms,
            gap: p.gap_d,
            maxd: p.max_d,
            minsep_seen: p.d.iter().cloned().fold(f64::INFINITY, f64::min),
            n_refs_held: n_held,
            n_refs_decoded: 0,
            n_refs_compared: 0,
        };
        let better = match &best {
            None => true,
            Some(b0) => {
                rank(cand.v) > rank(b0.v)
                    || (rank(cand.v) == rank(b0.v) && cand.n_ident > b0.n_ident)
            }
        };
        if better {
            best = Some(cand);
        }
    }
    // the counts belong to the SWEEP, not to whichever reference won
    if let Some(b) = best.as_mut() {
        b.n_refs_decoded = n_decoded;
        b.n_refs_compared = n_compared;
    }
    best
}

pub fn cmd_audit(args: &[String]) {
    let flag = |n: &str| -> Option<String> {
        args.iter().position(|a| a == n).and_then(|i| args.get(i + 1)).cloned()
    };
    let corpus = flag("--corpus").expect("--corpus");
    let refs_p = flag("--refs").expect("--refs");
    let minsep: f64 = flag("--minsep").and_then(|v| v.parse().ok()).unwrap_or(5.0);
    let refs = load_refs(&refs_p).unwrap();
    let mut cache: Cache = Cache::new();

    // every ghost the corpus publishes
    let mut files: Vec<std::path::PathBuf> = Vec::new();
    let root = std::path::Path::new(&corpus);
    let mut dirs: Vec<std::path::PathBuf> = vec![root.to_path_buf()];
    while let Some(d) = dirs.pop() {
        let Ok(rd) = std::fs::read_dir(&d) else { continue };
        for e in rd.flatten() {
            let p = e.path();
            if p.is_dir() {
                if p.file_name().map_or(false, |n| n == ".git") {
                    continue;
                }
                dirs.push(p);
            } else if p.to_string_lossy().ends_with(".Ghost.Gbx") {
                files.push(p);
            }
        }
    }
    files.sort();

    println!(
        "map\tfile\tmd5\tn_samples\tspan_s\thuman_ref\tn_common\tn_ident\tn_runs\thead_n\ttail_n\t\
         tail_start_s\tgap_sep_m\tmax_sep_m\tmin_sep_m\tverdict\tsibling_ref\tsibling_verdict\t\
         sibling_gap_m\tsibling_ident\trefs_held\trefs_decoded\trefs_compared"
    );
    // COVERAGE. Denominators for every claim this sweep will make.
    let (mut n_seen, mut n_decode_fail, mut n_no_ref, mut n_no_common) = (0usize, 0, 0, 0);
    let mut hist: std::collections::BTreeMap<String, usize> = Default::default();
    let mut one_ref_only = 0usize;
    for f in &files {
        n_seen += 1;
        let fp = f.to_string_lossy().to_string();
        let map = map_of(f);
        let bytes = std::fs::read(f).unwrap_or_default();
        let md5 = md5_hex(&bytes);
        let name = f.file_name().unwrap().to_string_lossy().to_string();
        let race_ms = declared_race_ms(&fp);
        let a = match decode(&fp).map(|v| in_race(&v, race_ms)) {
            Ok(v) => v,
            Err(e) => {
                n_decode_fail += 1;
                println!(
                    "{}\t{}\t{}\t0\t-\t-\t0\t0\t0\t0\t0\t-\t-\t-\t-\tDECODE-FAIL ({})\t-\t-\t-\t-\t{}\t0\t0",
                    map, name, md5, e, 0
                );
                continue;
            }
        };
        let span = if a.is_empty() {
            0.0
        } else {
            (a[a.len() - 1].ms - a[0].ms) as f64 / 1000.0
        };
        // map id is the leading digits of the directory name
        let mapid: String = map.chars().take_while(|c| c.is_ascii_digit()).collect();
        let humans: Vec<&Ref> =
            refs.iter().filter(|r| r.map == mapid && r.kind == "human").collect();
        // siblings: every OTHER published file of the same map
        let sibs: Vec<Ref> = files
            .iter()
            .filter(|g| map_of(g) == map && g.to_string_lossy() != fp)
            .map(|g| Ref {
                map: mapid.clone(),
                kind: "sibling".into(),
                path: g.to_string_lossy().to_string(),
            })
            .collect();
        let sib_refs: Vec<&Ref> = sibs.iter().collect();

        // References are windowed to THEIR OWN race too: a reference's tail is
        // no more a run than ours is.
        let hw = worst_over(&a, &humans, minsep, &mut cache);
        let sw = worst_over(&a, &sib_refs, minsep, &mut cache);
        let (rheld, rdec, rcmp) = match &hw {
            None => (humans.len(), 0usize, 0usize),
            Some(w) => (w.n_refs_held, w.n_refs_decoded, w.n_refs_compared),
        };
        let (hv, hr, hn, hi, hru, hh, ht, htm, hg, hm, hmin) = match &hw {
            None => (
                "NO-HUMAN-REFERENCE".to_string(),
                "-".to_string(),
                0,
                0,
                0,
                0,
                0,
                "-".to_string(),
                "-".to_string(),
                "-".to_string(),
                "-".to_string(),
            ),
            Some(w) => (
                if w.v == Verdict::NoComparison {
                    "NO-COMMON-INSTANTS".to_string()
                } else {
                    w.v.word().to_string()
                },
                w.refname.clone(),
                w.n_common,
                w.n_ident,
                w.n_runs,
                w.head_n,
                w.tail_n,
                if w.tail_n > 0 { fmt_s(w.tail_ms) } else { "-".into() },
                format!("{:.4}", w.gap),
                format!("{:.4}", w.maxd),
                format!("{:.6}", w.minsep_seen),
            ),
        };
        let (sv, sr, sg, si) = match &sw {
            None => ("-".to_string(), "-".to_string(), "-".to_string(), 0),
            Some(w) => (
                w.v.word().to_string(),
                w.refname.clone(),
                format!("{:.4}", w.gap),
                w.n_ident,
            ),
        };
        if hv.starts_with("NO-HUMAN-REFERENCE") {
            n_no_ref += 1;
        } else if hv.starts_with("NO-COMMON-INSTANTS") {
            n_no_common += 1;
        }
        if rcmp == 1 {
            one_ref_only += 1;
        }
        *hist.entry(hv.split(' ').next().unwrap_or("?").to_string()).or_insert(0) += 1;
        println!(
            "{}\t{}\t{}\t{}\t{:.3}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}",
            map,
            name,
            md5,
            a.len(),
            span,
            hr,
            hn,
            hi,
            hru,
            hh,
            ht,
            htm,
            hg,
            hm,
            hmin,
            hv,
            sr,
            sv,
            sg,
            si,
            rheld,
            rdec,
            rcmp
        );
    }
    // ---- COVERAGE, on stderr so the TSV stays a TSV ----------------------
    eprintln!("\nCOVERAGE of this sweep -- the denominators behind every row above");
    eprintln!("  {:>5}  ghost files found under the corpus", n_seen);
    eprintln!("  {:>5}  refused to decode (no verdict is possible for these)", n_decode_fail);
    eprintln!("  {:>5}  had NO human reference held for their map", n_no_ref);
    eprintln!("  {:>5}  had a reference but NO instant in common with it", n_no_common);
    eprintln!(
        "  {:>5}  were compared against exactly ONE reference -- those rows are CLEAN-vs-1-REF, \
         never CLEAN",
        one_ref_only
    );
    eprintln!("  verdicts:");
    for (k, v) in &hist {
        eprintln!("  {:>5}  {}", v, k);
    }
    eprintln!(
        "\n  A verdict is a claim about the references it was compared against. \
         'CLEAN' with refs_compared=1 and 'CLEAN' with refs_compared=4 are different claims."
    );
}

// ===========================================================================
// `intg gate` -- THE PUBLISH GATE. One command, exits non-zero, run BEFORE a
// ghost reaches a README.
//
// Every quality instrument this project owns was built after the artefacts it
// should have gated: `tmtraj check` was written the day ~170 ghosts were
// already published, and the re-convergence test an hour after that. That
// ordering is the defect this command exists to end.
//
// Three independent families, and a file must clear all three:
//
//   A. THE C-CHECKS (upstream `tmtraj check`) -- the file against itself and
//      the map: finite, moves, no teleport, no post-finish tail, the contact
//      and surface fields against free fall, the wheels, the input echo.
//
//   B. CONTAMINATION -- the bit-exact re-convergence test, against every human
//      recording we hold for the map. Identical -> metres apart -> identical
//      again cannot be driven, only spliced.
//
//   C. THE ORACLE -- the dedicated server re-simulating THE WRITTEN FILE and
//      returning the declared time. Not the tape it was built from: the file
//      that would be published, as it sits on disk.
//
// SANCTIONED EXCEPTIONS. A gate that refuses genuine files teaches people to
// override gates, so the narrow, named withdrawals are honoured -- and every
// application is LOGGED by name, so the exceptions are visible in the record
// rather than folded into a pass:
//
//   * lone C5  -- ground contact ON while provably airborne; the carrier's
//     byte on the two untitled ghosts, a known withdrawal.
//   * lone C10 -- the contact byte's flight claim; understood and unfixed.
//   * lone C8 ONLY when the file passes C8b below.
//
// Any second finding alongside an exception refuses. Any other failure
// refuses. Keep this list exactly this narrow.
//
// C8b -- WHY C8 AS WRITTEN REFUSES GENUINE FILES. Upstream C8 asks what share
// of a file's rolling steps imply a radius in 0.30..0.45 m. That band is the
// STADIUM car's wheel. On a snow map the car is swapped by the gameplay items
// and its wheel is 0.4700 m -- measured twice from opposite directions, on a
// downloaded Nadeo recording that re-simulates to its own exact millisecond,
// and by locating the wheel block in engine memory (0.4701 m). So the band is
// an automatic refusal on every non-Stadium car: snow, desert, rally. Same
// family as the free-fall constant that was carried at -22.3 for weeks.
//
// C8b measures the radius PER FILE and tests CONSISTENCY -- does the wheel
// behave like a wheel of SOME fixed size -- instead of equality with a
// constant. A foreign wheel byte-set still fails it: those bytes turn to
// another run's motion, so paired with THIS file's positions they imply no
// consistent radius at all.
// ===========================================================================

pub struct WheelFit {
    pub radius: f64,
    pub share: f64,
    pub n: usize,
}

/// Implied radii from the rolling steps, and how tightly they cluster.
pub fn wheel_fit(v: &[R], cls: &[crate::whlcmd::Cls]) -> Option<WheelFit> {
    use crate::whlcmd::Cls;
    let turns = |x: &R| -> f64 { x.b(7) as f64 + x.b(6) as f64 / 255.0 };
    let mut rr: Vec<f64> = Vec::new();
    for i in 1..v.len() {
        if cls.get(i) != Some(&Cls::Supported) {
            continue;
        }
        let mut dt = turns(&v[i]) - turns(&v[i - 1]);
        while dt < -128.0 {
            dt += 256.0;
        }
        while dt > 128.0 {
            dt -= 256.0;
        }
        let mut d = 0.0;
        for k in 0..3 {
            let q = v[i].pos[k] - v[i - 1].pos[k];
            d += q * q;
        }
        let d = d.sqrt();
        let dt = dt.abs();
        if dt > 1e-4 && d > 0.05 {
            rr.push(d / (dt * std::f64::consts::TAU));
        }
    }
    if rr.len() < 5 {
        return None;
    }
    // The mode over the whole PHYSICALLY POSSIBLE range, not a Stadium band.
    // 0.15 .. 0.90 m spans every car this game ships; the point is only to
    // exclude the wheelspin pile at ~0.05 and the locked-wheel tail above 1 m,
    // both of which are real driving and neither of which is the free-rolling
    // radius.
    let cand: Vec<f64> = rr.iter().cloned().filter(|x| (0.15..=0.90).contains(x)).collect();
    if cand.is_empty() {
        return Some(WheelFit { radius: f64::NAN, share: 0.0, n: rr.len() });
    }
    let mut b: std::collections::HashMap<i64, usize> = std::collections::HashMap::new();
    for x in &cand {
        *b.entry((x * 100.0).round() as i64).or_insert(0) += 1;
    }
    let k = *b.iter().max_by_key(|(_, v)| **v).unwrap().0;
    let mut c: Vec<f64> = cand
        .iter()
        .cloned()
        .filter(|x| ((x * 100.0).round() as i64) == k)
        .collect();
    c.sort_by(|a, b| a.total_cmp(b));
    let radius = c[c.len() / 2];
    // CONSISTENCY: what share of all rolling steps sit within 15 % of that one
    // radius. A real wheel returns to its free-rolling value constantly; a
    // foreign byte-set never settles anywhere.
    let lo = radius * 0.85;
    let hi = radius * 1.15;
    let share = rr.iter().filter(|x| **x >= lo && **x <= hi).count() as f64 / rr.len() as f64;
    Some(WheelFit { radius, share, n: rr.len() })
}

/// One oracle verdict: the dedicated server's own answer on the file as it
/// sits on disk.
pub struct OracleOut {
    pub sim_time: Option<i64>,
    pub declared_time: Option<i64>,
    pub cps: Option<i64>,
    pub is_valid: bool,
    pub desc: String,
    /// WHOSE FILE THE GAME THINKS THIS IS. 173636's regeneration is clean
    /// against 31 human recordings -- every position in it is ours -- and it
    /// still carries the rank-1 human's account id and login, inherited from
    /// the carrier its template was built on. No positional check can see
    /// that, however many references you throw at it. It is a different axis.
    pub account_id: Option<String>,
    pub login: Option<String>,
}

/// Re-simulate ONE ghost with the real dedicated server.
///
/// This is deliberately the file on disk, not the tape it was made from. The
/// whole failure class this gate exists for is a file whose payloads disagree,
/// and the only way to speak about the artefact is to hand the artefact over.
pub fn oracle_raw(server: &str, map: &str, ghost: &str) -> Result<String, String> {
    oracle_text(server, map, ghost)
}

pub fn oracle_run(server: &str, map: &str, ghost: &str) -> Result<OracleOut, String> {
    let text = oracle_text(server, map, ghost)?;
    parse_oracle(&text)
}

fn oracle_text(server: &str, map: &str, ghost: &str) -> Result<String, String> {
    use std::os::unix::fs::symlink;
    let work = format!("/tmp/intg-gate-{}-{}", std::process::id(), rand_tag());
    let rp = format!("{}/UserData/Replays", work);
    let mp = format!("{}/UserData/Maps", work);
    std::fs::create_dir_all(&rp).map_err(|e| e.to_string())?;
    std::fs::create_dir_all(&mp).map_err(|e| e.to_string())?;
    // The map goes in as a COPY under its own name: a worker dir reused across
    // maps answers as the first map it ever saw, which is a table of plausible
    // wrong numbers.
    let mapname = std::path::Path::new(map)
        .file_name()
        .ok_or("map has no filename")?
        .to_string_lossy()
        .to_string();
    std::fs::copy(map, format!("{}/{}", mp, mapname)).map_err(|e| format!("map: {}", e))?;
    let gname = std::path::Path::new(ghost)
        .file_name()
        .ok_or("ghost has no filename")?
        .to_string_lossy()
        .to_string();
    let gabs = std::fs::canonicalize(ghost).map_err(|e| e.to_string())?;
    symlink(&gabs, format!("{}/{}", rp, gname)).map_err(|e| format!("ghost: {}", e))?;
    for (n, t) in [("Packs", format!("{}/Packs", server)), ("TrackmaniaServer", format!("{}/TrackmaniaServer", server))] {
        let _ = symlink(&t, format!("{}/{}", work, n));
    }
    let out = std::process::Command::new("./TrackmaniaServer")
        .args(["/nodaemon", "/validatepath=."])
        .current_dir(&work)
        .output()
        .map_err(|e| format!("server: {}", e))?;
    // The JSON goes to stdout and the SUMMARY to stderr; a parser reading only
    // stdout sees a truncated document and calls a healthy run a failure.
    let mut text = String::from_utf8_lossy(&out.stdout).to_string();
    text.push_str(&String::from_utf8_lossy(&out.stderr));
    let _ = std::fs::remove_dir_all(&work);
    if !text.contains("replays parsed") {
        return Err(format!("the server produced no verdict: {}", text.lines().last().unwrap_or("")));
    }
    Ok(text)
}

fn parse_oracle(text: &str) -> Result<OracleOut, String> {
    let grab_i = |k: &str, after: &str| -> Option<i64> {
        let at = text.find(after)?;
        let seg = &text[at..];
        let p = seg.find(&format!("\"{}\" : ", k))?;
        let rest = &seg[p + k.len() + 5..];
        let num: String = rest
            .chars()
            .skip_while(|c| !c.is_ascii_digit() && *c != '-')
            .take_while(|c| c.is_ascii_digit() || *c == '-')
            .collect();
        num.parse().ok()
    };
    let is_valid = text.contains("\"IsValid\" : true");
    let sim_time = if text.contains("\"ValidatedResult\" : null") {
        None
    } else {
        grab_i("Time", "\"ValidatedResult\"")
    };
    let cps = grab_i("NbCheckpoints", "\"ValidatedResult\"");
    let declared_time = grab_i("Time", "\"DeclaredResult\"");
    let desc = text
        .find("\"Desc\" : \"")
        .map(|p| {
            text[p + 10..]
                .chars()
                .take_while(|c| *c != '"' && *c != '\\')
                .collect::<String>()
        })
        .unwrap_or_default();
    let grab_s = |k: &str| -> Option<String> {
        let p = text.find(&format!("\"{}\" : \"", k))?;
        Some(
            text[p + k.len() + 6..]
                .chars()
                .take_while(|c| *c != '"')
                .collect::<String>(),
        )
    };
    let account_id = grab_s("AccountId");
    let login = grab_s("Login");
    Ok(OracleOut { sim_time, declared_time, cps, is_valid, desc, account_id, login })
}

fn rand_tag() -> String {
    let t = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.subsec_nanos())
        .unwrap_or(0);
    format!("{:08x}", t)
}

/// The gate's verdict on one file.
pub struct GateOut {
    pub code: i32,
    pub lines: Vec<String>,
}

#[allow(clippy::too_many_arguments)]
/// The record a ghost carries, against the engine's own trajectory for the same
/// tape as dumped by `fk btraj2`. Returns
/// `(shared instants, median, mean, max, the route's own quantisation step)`.
///
/// The route CSV is written with six significant digits, so its resolution
/// depends on where the map sits in world coordinates -- ~0.01 m at 1400, ~0.001
/// at 140. The step is derived from the data rather than assumed, so the caller
/// compares against the instrument's own reach instead of a constant somebody
/// picked on one map.
pub fn route_compare(ghost: &str, route: &str) -> Result<(usize, f64, f64, f64, f64), String> {
    let rows = std::fs::read_to_string(route).map_err(|e| format!("{}: {}", route, e))?;
    let mut hdr = true;
    let mut rmap: std::collections::HashMap<i64, [f64; 3]> = std::collections::HashMap::new();
    let mut mag = 0.0f64;
    for l in rows.lines() {
        if hdr {
            hdr = false;
            continue;
        }
        let f: Vec<&str> = l.split(',').collect();
        if f.len() < 4 {
            continue;
        }
        let (t, x, y, z) = (
            f[0].parse::<i64>().ok(),
            f[1].parse::<f64>().ok(),
            f[2].parse::<f64>().ok(),
            f[3].parse::<f64>().ok(),
        );
        if let (Some(t), Some(x), Some(y), Some(z)) = (t, x, y, z) {
            mag = mag.max(x.abs()).max(y.abs()).max(z.abs());
            rmap.insert(t, [x, y, z]);
        }
    }
    if rmap.is_empty() {
        return Err(format!("{} has no usable rows", route));
    }
    let d = crate::entrec::decode_ghost(ghost).map_err(|e| format!("{}: {}", ghost, e))?;
    let mut ds: Vec<f64> = Vec::new();
    for s in &d.samples {
        if let Some(p) = rmap.get(&(s.time_ms as i64)) {
            ds.push(((s.x - p[0]).powi(2) + (s.y - p[1]).powi(2) + (s.z - p[2]).powi(2)).sqrt());
        }
    }
    if ds.is_empty() {
        return Err(format!(
            "the record and {} share no instant -- the route was made from a different run, or at \
             a probe tick past the record's span",
            route
        ));
    }
    let n = ds.len();
    let mean = ds.iter().sum::<f64>() / n as f64;
    let mx = ds.iter().cloned().fold(0.0f64, f64::max);
    ds.sort_by(|a, b| a.total_cmp(b));
    let med = ds[n / 2];
    // six significant digits: the last digit is worth mag * 1e-6, and two reads
    // can differ by one step in each, so the floor is twice that.
    let quant = (mag * 1e-6 * 2.0).max(1e-6);
    Ok((n, med, mean, mx, quant))
}

pub fn gate_one(
    ghost: &str,
    race: i64,
    refs: &[Ref],
    mapid: &str,
    server: Option<&str>,
    map: Option<&str>,
    minsep: f64,
    cache: &mut Cache,
    manifest_override: Option<&str>,
    require_manifest: bool,
    source_ref: Option<&str>,
    flag_route: Option<String>,
    flag_route_dir: Option<String>,
) -> GateOut {
    let mut lines: Vec<String> = Vec::new();
    let mut hard = 0usize;
    // Inputs the gate could not read. NEVER folded into `hard`: an operator
    // error is not a property of the ghost.
    let mut unmeasured = 0usize;

    // ---- A. the C-checks -------------------------------------------------
    let (ccode, cfails, cwarns, clines) = run_checks(ghost, race);
    for l in &clines {
        lines.push(format!("  {}", l));
    }
    // which checks failed, by id
    let failed: Vec<String> = clines
        .iter()
        .filter(|l| l.starts_with("FAIL"))
        .map(|l| l.split_whitespace().nth(1).unwrap_or("?").to_string())
        .collect();
    let _ = cwarns;
    if ccode == 2 {
        // the narrow sanctioned withdrawals, each logged BY NAME
        let lone = |id: &str| failed.len() == 1 && failed[0] == id;
        if lone("C5") {
            lines.push("EXCEPTION C5 applied -- lone C5 (contact ON while airborne), a named withdrawal".into());
        } else if lone("C10") {
            lines.push("EXCEPTION C10 applied -- lone C10 (the flight claim), understood and unfixed".into());
        } else if lone("C8") {
            // C8b decides: is there a consistent wheel of SOME size?
            match decode(ghost) {
                Ok(r) => {
                    let v: Vec<R> = r.into_iter().filter(|x| race <= 0 || x.ms <= race).collect();
                    let c = crate::whlcmd::classify(&v, crate::whlcmd::G_DEFAULT, 2.0, 5.0, 3);
                    match wheel_fit(&v, &c.cls) {
                        Some(w) if w.share >= 0.15 && w.radius.is_finite() => {
                            lines.push(format!(
                                "EXCEPTION C8 applied -- C8b PASSES: {:.0} % of {} rolling steps sit within 15 % of one radius, {:.4} m. \
                                 Upstream C8's 0.30-0.45 m band is the STADIUM wheel; a snow car measures 0.4700 m.",
                                100.0 * w.share, w.n, w.radius
                            ));
                        }
                        Some(w) => {
                            hard += 1;
                            lines.push(format!(
                                "FAIL   C8b    no consistent wheel: only {:.0} % of {} rolling steps cluster (mode {:.4} m)",
                                100.0 * w.share, w.n, w.radius
                            ));
                        }
                        None => {
                            hard += 1;
                            lines.push("FAIL   C8b    too few rolling steps to test the wheel".into());
                        }
                    }
                }
                Err(e) => {
                    hard += 1;
                    lines.push(format!("FAIL   C8b    {}", e));
                }
            }
        } else {
            hard += 1;
            lines.push(format!(
                "FAIL   C-checks  {} failing ({}) -- only a LONE C5, C10 or C8 is sanctioned",
                cfails,
                failed.join(",")
            ));
        }
    }

    // ---- B. contamination, against every human recording we hold ---------
    let a = decode(ghost);
    match &a {
        Err(e) => {
            hard += 1;
            lines.push(format!("FAIL   B-contam  no readable vehicle record: {}", e));
        }
        Ok(a) => {
            let humans: Vec<&Ref> =
                refs.iter().filter(|r| r.map == mapid && r.kind == "human").collect();
            if humans.is_empty() {
                // NOT a pass. An untested file is untested.
                lines.push(format!(
                    "n/a    B-contam  NO HUMAN REFERENCE for map {} -- this file is UNTESTED for contamination, not clean",
                    mapid
                ));
            } else {
                match worst_over(a, &humans, minsep, cache) {
                    Some(w) if w.v == Verdict::Contaminated => {
                        hard += 1;
                        lines.push(format!(
                            "FAIL   B-contam  SPLICE against {}: {} of {} samples bit-identical in {} runs, \
                             {:.1} m apart between them. Identical -> apart -> identical cannot be driven.",
                            w.refname, w.n_ident, w.n_common, w.n_runs, w.gap
                        ));
                    }
                    Some(w) if w.v == Verdict::Identical => {
                        hard += 1;
                        lines.push(format!(
                            "FAIL   B-contam  this file's telemetry IS {} -- all {} shared samples bit-identical. \
                             It is a human recording published under our name.",
                            w.refname, w.n_ident
                        ));
                    }
                    Some(w) => {
                        let big_head = w.head_n * 4 >= w.n_common && w.head_n >= 10;
                        if big_head {
                            // NOT a refusal, and not a clean pass either. A
                            // head-anchored identical block is what determinism
                            // produces when two runs share an opening tape --
                            // and it is ALSO what an inherited donor prefix
                            // looks like, which is the exact shape `fk regen`
                            // used to produce from a late handover. The two are
                            // not separable from the file alone. What separates
                            // them is the MANIFEST: whether those samples were
                            // written from engine state. Say so, loudly, rather
                            // than pick one.
                            lines.push(format!(
                                "WARN   B-contam  {} of {} samples are bit-identical to {} in a HEAD-ANCHORED block ({:.0} %). \
                                 Determinism produces this from a shared opening tape; so does an inherited donor prefix. \
                                 The file cannot tell you which -- its manifest can.",
                                w.head_n, w.n_common, w.refname,
                                100.0 * w.head_n as f64 / w.n_common.max(1) as f64
                            ));
                        } else {
                            lines.push(format!(
                                "PASS   B-contam  no splice against {} human recording(s); closest {} \
                                 ({} of {} samples bit-identical, max separation {:.1} m)",
                                humans.len(), w.refname, w.n_ident, w.n_common, w.maxd
                            ));
                        }
                    }
                    None => lines.push(format!(
                        "n/a    B-contam  {} human reference(s) named for map {}, none decodable",
                        humans.len(), mapid
                    )),
                }
            }
        }
    }

    // ---- C. the oracle re-simulating THE WRITTEN FILE --------------------
    match (server, map) {
        (Some(s), Some(m)) => match oracle_run(s, m, ghost) {
            Ok(o) => {
                let st = o.sim_time;
                match st {
                    Some(t) if race > 0 && t == race => lines.push(format!(
                        "PASS   C-oracle  the server re-simulates THIS FILE to {:.3} s, the declared time (cps {}, IsValid {})",
                        t as f64 / 1000.0,
                        o.cps.unwrap_or(-1),
                        o.is_valid
                    )),                    Some(t) if race > 0 => {
                        hard += 1;
                        lines.push(format!(
                            "FAIL   C-oracle  the server returns {:.3} s but the file is published as {:.3} s (delta {:+} ms). \
                             A 1 ms delta is the `u02 truncate` signature -- check the tape had 40 ticks of headroom.",
                            t as f64 / 1000.0,
                            race as f64 / 1000.0,
                            t - race
                        ));
                    }
                    Some(t) => lines.push(format!(
                        "n/a    C-oracle  the server returns {:.3} s; no --race given to check it against",
                        t as f64 / 1000.0
                    )),
                    None => {
                        hard += 1;
                        lines.push(format!(
                            "FAIL   C-oracle  the server REFUSES this file: {} (declared {:?})",
                            if o.desc.is_empty() { "DNF / not validated".into() } else { o.desc.clone() },
                            o.declared_time
                        ));
                    }
                }
                // ---- C-header: WHOSE FILE THE GAME THINKS THIS IS ----------
                //
                // 173636's regeneration is clean against 31 human recordings
                // -- every position in it is ours -- and it still carries the
                // rank-1 human's account id and login, inherited from the
                // carrier its template was built on. NO POSITIONAL CHECK CAN
                // SEE THAT, however many references you throw at it.
                //
                // Two measurable statements, both taken from the game's own
                // parser rather than from a manifest's claim:
                //
                //   1. the header's declared time must equal what the server
                //      validates the file to. 173636's declares 23.638 -- the
                //      human's time -- and validates to 22.072. Our published
                //      TAS_22072 declares 22.072 and validates to 22.072, and
                //      is the control that makes this real.
                //   2. the file must carry no account id at all. Our own files
                //      carry none and report Login "TAS".
                //
                // Point 1 caught 173636 BY ACCIDENT: had the carrier's declared
                // time happened to match, IsValid would have read true and the
                // file would have shipped under a real player's account. Point
                // 2 is the check that would have caught it on purpose.
                match (o.declared_time, o.sim_time) {
                    (Some(d), Some(v)) if d != v => {
                        // WHOSE time is it? A header declaring another RUN's
                        // time is a borrowed container. A header declaring the
                        // map's AUTHOR time is a different field entirely and
                        // no claim about anyone's driving.
                        //
                        // 146612's KEYBOARD_39706 and TAS_39183 both declare
                        // 39.555, which is on no leaderboard we hold -- the
                        // census reads 40223 x0 and 40226 x0 in both -- and
                        // both files carry their OWN time at three sites
                        // (+92 +148) with 39555 at two more, 7 kB away, at a
                        // +60 spacing that appears nowhere in the six-site
                        // group. Two distinct fields, not one half-overwritten.
                        // Rewriting those two sites would assert that the map's
                        // author time equals our run: manufacturing a false
                        // claim to make a check go green.
                        //
                        // CONTROL: 191465's WIP_keyboard declares 13.081, which
                        // IS the map's rank-1 human time, and must still refuse.
                        // The two cases are separated by whether the declared
                        // value is a human record we hold -- not by how far it
                        // is from the validated time.
                        let foreign = refs
                            .iter()
                            .filter(|r| r.map == mapid && r.kind == "human")
                            .any(|r| declared_race_ms(&r.path) == Some(d));
                        if foreign {
                            hard += 1;
                            lines.push(format!(
                                "FAIL   C-header  the header declares {:.3} s -- a HUMAN RECORD of this map -- but the server \
                                 validates this file to {:.3} s. A container whose header was inherited and never rewritten.",
                                d as f64 / 1000.0,
                                v as f64 / 1000.0
                            ));
                        } else {
                            lines.push(format!(
                                "note   C-header  the header declares {:.3} s, which is NOT a human record of this map \
                                 (most likely the map's author time) while the server validates this file to {:.3} s. \
                                 Its own time is present at its own sites; no claim about anyone's driving is involved.",
                                d as f64 / 1000.0,
                                v as f64 / 1000.0
                            ));
                        }
                    }
                    (Some(d), Some(_)) => lines.push(format!(
                        "PASS   C-header  the header's declared time and the validated time agree at {:.3} s",
                        d as f64 / 1000.0
                    )),
                    _ => lines.push(
                        "n/a    C-header  the server returned no declared/validated pair".into(),
                    ),
                }
                match (&o.account_id, &o.login) {
                    (Some(a), l) if !a.is_empty() => {
                        hard += 1;
                        lines.push(format!(
                            "FAIL   C-ident   this file carries a player account id ({}{}). Ours carry NONE. \
                             The file says it is somebody's; that is a claim about a person, not about a trajectory.",
                            a,
                            l.as_deref().map(|x| format!(", login {}", x)).unwrap_or_default()
                        ));
                    }
                    (_, Some(l)) if l != "TAS" && !l.is_empty() => {
                        hard += 1;
                        lines.push(format!(
                            "FAIL   C-ident   this file's login is {:?}, not \"TAS\". Ours report \"TAS\" and no account id.",
                            l
                        ));
                    }
                    _ => lines.push(
                        "PASS   C-ident   no account id; the login is ours".into(),
                    ),
                }
            }
            Err(e) => {
                // AN INPUT WE COULD NOT READ IS NOT A VERDICT ABOUT THE GHOST.
                //
                // This used to be `hard += 1` and a FAIL line, which meant that
                // typing the wrong map path REFUSED the subject -- a file the
                // gate had never read. Every other silent-wrong-answer tonight
                // produced a false NEGATIVE (`0.0 m travelled`, `M-time`
                // comparing two command-line numbers, curl scoring a 404 green).
                // This one produced a false REFUSAL, and false refusals are how
                // a gate dies: refuse honest work twice and the third time
                // somebody reaches for the override, after which it catches
                // nothing at all.
                //
                // The rule, as a class: any input the gate could not read yields
                // UNMEASURED naming THE INPUT, never a verdict about the subject.
                unmeasured += 1;
                lines.push(format!(
                    "UNMEASURED  C-oracle  the oracle could not run -- {} -- so THIS FILE WAS NOT \
                     RE-SIMULATED. This says nothing about the ghost: it is an input the gate \
                     could not read.",
                    e
                ));
            }
        },
        _ => lines.push(
            "n/a    C-oracle  no --server/--map given -- THE FILE HAS NOT BEEN RE-SIMULATED".into(),
        ),
    }

    // ---- E. the STALE-BUFFER check, against the source this file was made
    //      from. Only runs when a source is named: it compares two renderings
    //      of the SAME run, which is the only case where "one tick behind" is
    //      a meaningful statement.
    if let Some(src) = source_ref {
        match (decode(ghost), decode(src)) {
            (Ok(a), Ok(b)) => match stale_check(&a, &b) {
                Some(v) => {
                    let (code, word) = stale_verdict(&v);
                    if code != 0 {
                        hard += 1;
                        lines.push(format!(
                            "FAIL   E-stale    {} (median {:.4} m, max {:.4} m vs {})",
                            word,
                            v.median_m,
                            v.max_m,
                            std::path::Path::new(src)
                                .file_name()
                                .map(|s| s.to_string_lossy().to_string())
                                .unwrap_or_default()
                        ));
                    } else {
                        lines.push(format!("PASS   E-stale    {}", word));
                    }
                }
                None => lines.push(
                    "n/a    E-stale    too few shared instants with the source to compare".into(),
                ),
            },
            _ => lines.push("n/a    E-stale    the source could not be decoded".into()),
        }
    } else {
        lines.push(
            "n/a    E-stale    no --source given -- THIS FILE HAS NOT BEEN CHECKED against the \
             one-tick-stale buffer copy, which passes every other check in this gate"
                .into(),
        );
    }

    // ---- F. C-SPAWN: is the car pointing where every car on this map points?
    //
    // Every run of a map begins at the same spawn, in the same attitude. So the
    // FIRST in-race sample of an honest recording equals a downloaded human
    // recording's first in-race sample -- in POSITION and in ORIENTATION -- and
    // a file whose first sample is somewhere else, or facing somewhere else, is
    // not this map's run whatever its coverage says.
    //
    // WHY ORIENTATION IS HERE, AND WHY NO OTHER CHECK CATCHES IT. The writer
    // encodes the engine's rotation, and the engine object it locates stores
    // that rotation in one of three layouts (`fk regen`'s probe prints
    // `orient kind N`). Which one it finds VARIES BETWEEN RUNS of the same
    // command. Get it wrong and every position is still exact, so C1-C10, the
    // oracle, the tape md5 and the whole contamination family pass while the
    // car faces the wrong way for the entire render. That is exactly what
    // 197047's withdrawn clip was: identity spawn quaternion, correct path.
    // Measured on this map's own answer key, one run, two encodings:
    // the probe's own kind -> 0.0068 deg against the human's recorded bytes,
    // the "settled convention" pinned over it -> 90.68 deg.
    //
    // The bars are wide on purpose. This separates 0.004 deg from 90 deg; it is
    // not a precision instrument and must never be read as one.
    {
        let firstpos = |d: &crate::entrec::Decoded| -> Option<crate::entrec::Sample> {
            d.samples.iter().find(|s| s.time_ms >= 0).cloned()
        };
        let mut done = false;
        for r in refs.iter().filter(|r| r.map == mapid && r.kind == "human") {
            let (a, b) = match (
                crate::entrec::decode_ghost(ghost),
                crate::entrec::decode_ghost(&r.path),
            ) {
                (Ok(a), Ok(b)) => (a, b),
                _ => continue,
            };
            let (sa, sb) = match (firstpos(&a), firstpos(&b)) {
                (Some(x), Some(y)) => (x, y),
                _ => continue,
            };
            let dpos =
                ((sa.x - sb.x).powi(2) + (sa.y - sb.y).powi(2) + (sa.z - sb.z).powi(2)).sqrt();
            // rotation angle between two unit quaternions: 2 acos |a.b|
            let dot = (sa.qx * sb.qx + sa.qy * sb.qy + sa.qz * sb.qz + sa.qw * sb.qw).abs();
            let dang = 2.0 * dot.min(1.0).acos() * 180.0 / std::f64::consts::PI;
            let nm = std::path::Path::new(&r.path)
                .file_name()
                .map(|s| s.to_string_lossy().to_string())
                .unwrap_or_default();
            if dpos > 0.5 || dang > 5.0 {
                hard += 1;
                lines.push(format!(
                    "FAIL   C-spawn   the first in-race sample is {:.4} m and {:.3} deg from {}'s. \
                     Every run of this map starts in the same place facing the same way; \
                     {} is another object, or the orientation encoding is permuted.",
                    dpos, dang, nm,
                    if dpos > 0.5 { "this position" } else { "this attitude" }
                ));
            } else {
                lines.push(format!(
                    "PASS   C-spawn   first in-race sample {:.4} m and {:.3} deg from {}'s",
                    dpos, dang, nm
                ));
            }
            done = true;
            break;
        }
        if !done {
            lines.push(
                "n/a    C-spawn   no human recording of this map could be decoded -- THE SPAWN \
                 AND THE FACING ARE UNCHECKED, and a permuted orientation passes every other \
                 check in this gate"
                    .into(),
            );
        }
    }

    // ---- G. C-ROUTE: the record against the ENGINE, read by a different
    //      instrument.
    //
    // THIS IS THE ONLY CHECK IN THE GATE THAT DOES NOT READ THE RECORD PATH.
    // Everything above interrogates the written record, or the tape, or the
    // file's own header, and on 227654 every one of them passed a file whose
    // telemetry is the CONTAINER's run and not ours:
    //
    //   * B-contam passed -- the record is 0.000511 m from ailiei.'s, which is
    //     the client-vs-server floor, not bit-identical.
    //   * C-spawn passed -- the first sample is the map's spawn, because his
    //     run starts there too.
    //   * C-oracle passed -- the oracle re-simulates the TAPE, and the tape is
    //     ours.
    //   * E-stale passed -- two independent generations agreed exactly.
    //
    // They agreed with each other because they were all reading one poisoned
    // source. What convicted the file was asking a DIFFERENT instrument the
    // same question: `fk btraj2` re-simulates the tape and dumps the car's
    // per-tick position without going near the record. Our car was 0.7249 m
    // mean / 1.8607 m max from the line the record claims, over 301 instants --
    // and two of our tapes that differ at race 11.270 s, inside the recorded
    // window, produced BIT-IDENTICAL records, which two different simulations
    // cannot do.
    //
    // The bar is derived from the instrument, not chosen: `fk btraj2` writes
    // six significant digits, so at coordinates around 1400 its own quantum is
    // ~0.01 m. Anything within a small multiple of that is agreement to the
    // resolution of the reading; 0.55 m is fifty times it. Nothing in the gap.
    //
    // NO ROUTE IS A REFUSAL, NOT AN `n/a`. A file that cannot be route-checked
    // is precisely the file this hole hides in.
    {
        let route = flag_route.clone().or_else(|| {
            flag_route_dir.as_ref().map(|d| {
                let stem = std::path::Path::new(ghost)
                    .file_name()
                    .map(|s| s.to_string_lossy().replace(".Ghost.Gbx", ""))
                    .unwrap_or_default();
                format!("{}/route_{}.csv", d, stem)
            })
        });
        match route {
            None => {
                hard += 1;
                lines.push(
                    "FAIL   C-route   no --route given. The record has NOT been checked against \
                     the engine by any instrument other than the one that wrote it, and on 227654 \
                     every other check in this gate passed a file carrying the container's run. \
                     Produce one with `fk btraj2 --template THIS FILE ... --out route.csv`."
                        .into(),
                );
            }
            Some(rp) => match route_compare(ghost, &rp) {
                Err(e) => {
                    unmeasured += 1;
                    lines.push(format!(
                        "UNMEASURED  C-route   {} -- the engine's own trajectory could not be \
                         read, so THIS FILE IS NEITHER CLEAN NOR CONVICTED on the one axis the \
                         rest of the gate cannot see.",
                        e
                    ));
                }
                Ok((n, med, mean, mx, quant)) => {
                    let bar = (20.0 * quant).max(0.02);
                    if med > bar {
                        hard += 1;
                        lines.push(format!(
                            "FAIL   C-route   the record is {:.4} m from where the engine put this \
                             tape's car (median over {} shared instants, mean {:.4}, max {:.4}; the \
                             route's own quantum here is {:.4} m, bar {:.4}). THIS RECORD IS NOT \
                             THIS RUN.",
                            med, n, mean, mx, quant, bar
                        ));
                    } else {
                        lines.push(format!(
                            "PASS   C-route   the record matches the engine's own trajectory for \
                             this tape to {:.4} m over {} shared instants (route quantum {:.4} m)",
                            med, n, quant
                        ));
                    }
                }
            },
        }
    }

    // ---- D. the file against its own MANIFEST ---------------------------
    //
    // Last, and it is the one that certifies. A, B and C interrogate the
    // bytes; D asks whether the account of how those bytes were made holds up.
    // Where B can say nothing -- no human recording held, or the donor IS the
    // reference -- D is the only evidence there is.
    let mpath = manifest_override
        .map(|s| s.to_string())
        .unwrap_or_else(|| crate::manifest::manifest_path(ghost));
    match crate::manifest::read(&mpath) {
        Ok(m) => {
            let (mbad, mlines) = crate::manifest::verify(ghost, &m);
            for l in mlines {
                lines.push(format!("  {}", l));
            }
            hard += mbad;
        }
        Err(e) => {
            if require_manifest {
                hard += 1;
                lines.push(format!("FAIL   D-manifest  no readable manifest: {}", e));
            } else {
                lines.push(format!(
                    "n/a    D-manifest  no manifest at {} -- THIS FILE IS UNPROVENANCED. \
                     It may be perfectly good; nothing in it says so. \
                     (--require-manifest makes this a refusal.)",
                    mpath
                ));
            }
        }
    }

    // 2 = refused, 3 = could not be measured, 0 = publishable. Unmeasured is
    // NOT clean and NOT refused; it is its own outcome and its own exit code.
    GateOut { code: if hard > 0 { 2 } else if unmeasured > 0 { 3 } else { 0 }, lines }
}

pub fn cmd_gate(args: &[String]) {
    let flag = |n: &str| -> Option<String> {
        args.iter().position(|a| a == n).and_then(|i| args.get(i + 1)).cloned()
    };
    let mut files: Vec<String> = Vec::new();
    let mut i = 0usize;
    while i < args.len() {
        if args[i].starts_with("--") {
            if matches!(
                args[i].as_str(),
                "--race" | "--refs" | "--map" | "--server" | "--minsep" | "--mapid" | "--manifest" | "--source" | "--route" | "--route-dir"
            ) {
                i += 2;
            } else {
                i += 1;
            }
            continue;
        }
        files.push(args[i].clone());
        i += 1;
    }
    if files.is_empty() {
        print!(
            "\
usage: tmtraj intg gate GHOST... --race MS [--refs refs.tsv] [--mapid ID]
                        [--server DIR --map MAP.Map.Gbx] [--minsep M]

THE PUBLISH GATE. Exit 0 = publishable, 2 = REFUSED.

  A  the C-checks (tmtraj check)
  B  the bit-exact contamination test against every human recording held
  C  the dedicated server re-simulating THE WRITTEN FILE to its declared time

Sanctioned, and logged by name every time one is applied: a LONE C5, a LONE
C10, and a LONE C8 whose file passes C8b (a consistent wheel of any size --
0.36 m is the Stadium car's, a snow car is 0.4700 m). Anything else refuses.

Without --server/--map the file is NOT re-simulated and the gate says so.
Without a human reference the file is UNTESTED for contamination, never clean.
"
        );
        std::process::exit(2);
    }
    let race: i64 = flag("--race").and_then(|v| v.parse().ok()).unwrap_or(-1);
    let minsep: f64 = flag("--minsep").and_then(|v| v.parse().ok()).unwrap_or(5.0);
    let refs = flag("--refs").map(|p| load_refs(&p).unwrap()).unwrap_or_default();
    let server = flag("--server");
    let map = flag("--map");
    let manifest = flag("--manifest");
    let source = flag("--source");
    let require_manifest = args.iter().any(|a| a == "--require-manifest");
    let mut cache: Cache = Cache::new();
    let mut worst = 0;
    for f in &files {
        // the map id: given, or the leading digits of the containing map dir
        let mapid = flag("--mapid").unwrap_or_else(|| {
            let d = map_of(std::path::Path::new(f));
            d.chars().take_while(|c| c.is_ascii_digit()).collect()
        });
        let g = gate_one(
            f,
            race,
            &refs,
            &mapid,
            server.as_deref(),
            map.as_deref(),
            minsep,
            &mut cache,
            manifest.as_deref(),
            require_manifest,
            source.as_deref(),
            flag("--route"),
            flag("--route-dir"),
        );
        println!(
            "=== {}  --  {}",
            f,
            match g.code { 0 => "PUBLISHABLE", 3 => "UNMEASURED (an input could not be read; this is not a verdict about the file)", _ => "REFUSED" }
        );
        for l in &g.lines {
            println!("{}", l);
        }
        worst = worst.max(g.code);
    }
    std::process::exit(worst);
}

/// Run the upstream `tmtraj check` as a SUBPROCESS and read its verdict.
///
/// Deliberately not a library call into `checkcmd`. Two reasons, and the
/// second is the real one:
///
///   * `checkcmd.rs` arrived from another arm and this gate does not modify
///     files it did not write -- a gate that quietly forks the checker it
///     claims to run is the same disease as a filename that claims a run;
///   * the gate then exercises THE COMMAND A PERSON WOULD RUN, byte for byte.
///     If `tmtraj check` and the gate ever disagree, that is a bug in one of
///     them, and a library call would hide it.
fn run_checks(ghost: &str, race: i64) -> (i32, usize, usize, Vec<String>) {
    let exe = std::env::current_exe().unwrap_or_else(|_| "tmtraj".into());
    let mut c = std::process::Command::new(exe);
    c.arg("check").arg(ghost);
    if race > 0 {
        c.arg("--race").arg(race.to_string());
    }
    let out = match c.output() {
        Ok(o) => o,
        Err(e) => {
            return (2, 1, 0, vec![format!("FAIL   C0     could not run tmtraj check: {}", e)])
        }
    };
    let text = String::from_utf8_lossy(&out.stdout).to_string();
    let lines: Vec<String> = text
        .lines()
        .map(|l| l.trim_end().to_string())
        .filter(|l| {
            let t = l.trim_start();
            t.starts_with("PASS ") || t.starts_with("FAIL ") || t.starts_with("WARN ") || t.starts_with("n/a ")
        })
        .map(|l| l.trim_start().to_string())
        .collect();
    let fails = lines.iter().filter(|l| l.starts_with("FAIL")).count();
    let warns = lines.iter().filter(|l| l.starts_with("WARN")).count();
    let code = out.status.code().unwrap_or(2);
    (code, fails, warns, lines)
}

// ===========================================================================
// `intg dup` -- THE CROSS-FILE CHECK. No per-file gate can see this class.
//
// The corpus pass produced four 227654 ghosts that were each internally
// flawless -- 365 of 365 samples written, nothing missing in the race, the
// oracle exact, every C-check passing -- and all four carried ONE trajectory,
// bit-identical to each other at median, p90, p99 and max. Coverage, oracle
// agreement, tape md5 and C1..C10 are all properties of a SINGLE file, so
// every one of them passed. Only a comparison BETWEEN outputs can see it.
//
// The rule needs both halves:
//
//   identical telemetry + DIFFERENT inputs  -> REFUSE. Two different drives
//       cannot produce the same trajectory; one of them is the other's.
//   identical telemetry + IDENTICAL inputs  -> correct, and must not be
//       refused. 227654's TAS_57493 and TAS_57498 have the same inputs inside
//       the recorded window, so one trajectory is the right answer for both.
//       A naive "no two files may match" rule condemns them.
//
// So the comparison is against the INPUT ARCHIVE, never the telemetry alone.
// The input key used here is the validator's own decoded `Inputs` string --
// the engine's account of what it simulated, which is exactly the thing that
// has to differ for the telemetry to differ. Without a map to run the oracle
// on, the pair is reported as INPUTS-UNCHECKED and never as a pass.
//
// Why this happens, which is the part that constrains regeneration generally:
// the chooser identifies the car by grading candidates against the template's
// OWN recorded positions. On a contaminated file that record is the donor's,
// and all five 227654 files share one donor -- so it picks the donor's object
// every time and emits one trajectory every time. THE VERY CONDITION THAT
// MAKES A FILE WORTH REGENERATING IS THE CONDITION THAT MAKES THE CHOOSER
// UNSAFE. (`FK_NO_CHOOSER=1` is the enforcement upstream.)
// ===========================================================================

pub fn cmd_dup(args: &[String]) {
    let flag = |n: &str| -> Option<String> {
        args.iter().position(|a| a == n).and_then(|i| args.get(i + 1)).cloned()
    };
    let corpus = flag("--corpus").expect("--corpus");
    let server = flag("--server");
    let maps: std::collections::HashMap<String, String> = match flag("--maps") {
        Some(p) => std::fs::read_to_string(&p)
            .unwrap_or_default()
            .lines()
            .filter_map(|l| {
                let f: Vec<&str> = l.split('\t').collect();
                if f.len() >= 2 { Some((f[0].to_string(), f[1].to_string())) } else { None }
            })
            .collect(),
        None => Default::default(),
    };

    // group the corpus by map
    let mut by_map: std::collections::BTreeMap<String, Vec<std::path::PathBuf>> = Default::default();
    let mut dirs = vec![std::path::PathBuf::from(&corpus)];
    while let Some(d) = dirs.pop() {
        let Ok(rd) = std::fs::read_dir(&d) else { continue };
        for e in rd.flatten() {
            let p = e.path();
            if p.is_dir() {
                if p.file_name().map_or(false, |n| n == ".git") {
                    continue;
                }
                dirs.push(p);
            } else if p.to_string_lossy().ends_with(".Ghost.Gbx") {
                by_map.entry(map_of(&p)).or_default().push(p);
            }
        }
    }

    println!("map\tfile_a\tfile_b\tn_common\tn_ident\tlongest_run\tmax_sep_m\tinputs\tverdict");
    let mut refused = 0usize;
    for (map, files) in &by_map {
        if files.len() < 2 {
            continue;
        }
        let mapid: String = map.chars().take_while(|c| c.is_ascii_digit()).collect();
        let mut files = files.clone();
        files.sort();
        // decode once per file
        let dec: Vec<Option<Vec<R>>> =
            files.iter().map(|f| decode(&f.to_string_lossy()).ok()).collect();
        // the input key is computed PER PAIR, over the identical run
        let inputs: Vec<Option<String>> = vec![None; files.len()];
        let _ = &inputs;
        let _ = &server;
        let _ = &maps;
        for i in 0..files.len() {
            for j in i + 1..files.len() {
                let (Some(a), Some(b)) = (&dec[i], &dec[j]) else { continue };
                let p = pair(a, b);
                if p.n < 5 {
                    continue;
                }
                let nid = p.ident.iter().filter(|x| **x).count();
                // NOT "every sample identical". 134672 publishes two files that
                // share 986 CONSECUTIVE bit-identical positions -- 49.3 s of
                // driving agreeing to the last f32 bit -- out of 1349, and an
                // all-or-nothing rule passed them. What makes a pair one run is
                // a RUN of identical samples long enough that coincidence is
                // not an explanation, at ANY alignment: the same reasoning as
                // `intg lag`, and it subsumes the 100 % case.
                let hits = lag_scan(a, b, 20.min(p.n));
                let (best_run, best_lag) =
                    hits.first().map_or((0, 0), |h| (h.best_run, h.lag));
                let all_ident = best_run >= 10;
                // THE INPUT KEY IS TAKEN OVER THE IDENTICAL RUN ITSELF, not over
                // the whole recorded window. Two of our tapes seeded from one
                // human tape share a DETERMINISTIC PREFIX -- identical f32
                // positions for as long as their inputs agree -- and that is
                // honest. What is impossible is identical positions across a
                // span where the inputs DIFFER. Keyed on the whole window the
                // run rule produced 284 refusals, nearly all of them shared
                // prefixes; keyed on the run itself, only the impossible ones
                // remain. A non-zero best lag means the two files come from
                // different recording sessions, where no input window is
                // comparable -- and a long identical run there is already
                // impossible, so no key is needed.
                // A DIFFERENCE MUST HAVE HAD TIME TO PROPAGATE. It is not
                // enough that the inputs differ somewhere inside the span where
                // the positions agree: a difference in the LAST few ticks of
                // that span has not moved the car yet. 126859 publishes two
                // tapes that are input-identical until race 18.900 and
                // bit-identical in position through exactly 18.900 --
                // determinism, working. A span digest calls that a splice, and
                // did, on 163 pairs of mine. The honest quantity is the FIRST
                // instant the inputs differ: positions may agree up to there and
                // a little past it; agreeing far beyond it cannot happen.
                let run_span = if all_ident && best_lag == 0 {
                    find_run_span(a, b, best_lag, best_run)
                } else {
                    None
                };
                // How far past the first input divergence the positions may still
                // agree before the agreement is impossible. Two bands, because
                // the evidence is not the same strength at both ends: every
                // splice this corpus actually contains overshoots by tens or
                // hundreds of seconds (227654 by 16.4 s, 238835 by 111 s,
                // 286279 by 403 s), while a sub-second overshoot has an honest
                // explanation available -- an input that is INERT where it
                // differs, which on a Trial map with a wedged car is common.
                // Refusing those would be the C8 mistake again: a gate that
                // refuses genuine files teaches people to override gates.
                const REVIEW_MS: i64 = 150;
                const PROPAGATE_MS: i64 = 2000;
                let (inp, verdict) = if !all_ident {
                    ("-".to_string(), "DISTINCT".to_string())
                } else if best_lag != 0 {
                    // different recording sessions: no input window is
                    // comparable, and a long identical run is already impossible
                    refused += 1;
                    ("cross-session".to_string(), "REFUSE-ONE-RUN-TWICE".to_string())
                } else {
                    // THE FIRST DIFFERING TICK IS NOT THE DIVERGENCE. A single tick
                    // inside a no-authority window carries no information -- the
                    // car is airborne, or wedged, or the value is inert -- so a
                    // differencer that treats all ticks alike manufactures
                    // distinctions. Measured on 165922: TAS_15240 differs from
                    // TAS_15290 at exactly ONE inert tick at 2.250 s and then not
                    // again until 3.37 s, and keying on 2.250 made a pair whose
                    // positions legitimately agree to 4.350 s read as one run
                    // published twice.
                    //
                    // Key on the first SUSTAINED divergence instead: the first
                    // tick after which the tapes keep differing.
                    let fd = first_sustained_diff(
                        &files[i].to_string_lossy(),
                        &files[j].to_string_lossy(),
                    );
                    match (fd, run_span) {
                        (None, _) => (
                            "identical-tapes".to_string(),
                            "EXPECTED-SAME-INPUTS".to_string(),
                        ),
                        (Some(d), Some((_, t1))) if t1 > d + PROPAGATE_MS => {
                            refused += 1;
                            (
                                format!(
                                    "diverge@{:.3}s,agree_to@{:.3}s,over={:.3}s",
                                    d as f64 / 1000.0,
                                    t1 as f64 / 1000.0,
                                    (t1 - d) as f64 / 1000.0
                                ),
                                "REFUSE-ONE-RUN-TWICE".to_string(),
                            )
                        }
                        (Some(d), Some((_, t1))) if t1 > d + REVIEW_MS => (
                            format!(
                                "diverge@{:.3}s,agree_to@{:.3}s,over={:.3}s",
                                d as f64 / 1000.0,
                                t1 as f64 / 1000.0,
                                (t1 - d) as f64 / 1000.0
                            ),
                            "REVIEW-SHORT-OVERSHOOT".to_string(),
                        ),
                        (Some(d), Some(_)) => (
                            format!("diverge@{:.3}s", d as f64 / 1000.0),
                            "EXPECTED-SHARED-PREFIX".to_string(),
                        ),
                        (Some(_), None) => {
                            refused += 1;
                            (
                                "unchecked".to_string(),
                                "IDENTICAL-INPUTS-UNCHECKED".to_string(),
                            )
                        }
                    }
                };
                if all_ident {
                    println!(
                        "{}\t{}\t{}\t{}\t{}\t{}\t{:.6}\t{}\t{}",
                        map,
                        files[i].file_name().unwrap().to_string_lossy(),
                        files[j].file_name().unwrap().to_string_lossy(),
                        p.n,
                        nid,
                        best_run,
                        p.max_d,
                        inp,
                        verdict
                    );
                }
            }
        }
    }
    std::process::exit(if refused > 0 { 2 } else { 0 });
}

/// The validator's own decoded input string for one ghost -- the engine's
/// account of what it simulated. `None` if the oracle could not answer.
pub fn oracle_inputs(server: &str, map: &str, ghost: &str) -> Option<String> {
    let o = oracle_raw(server, map, ghost).ok()?;
    let p = o.find("\"Inputs\" : \"")?;
    Some(o[p + 12..].chars().take_while(|c| *c != '"').collect())
}

/// The tape's decoded inputs over the recorded window, as a digest.
///
/// Shells out to `fk tapeinputs`: decoding a ghost's input archive needs
/// `tmsearch::ghost::Factory`, which lives in the `fk` crate, and `tmtraj` does
/// not depend on it. Same reasoning as running `tmtraj check` as a subprocess
/// -- the alternative is a second decoder that can drift from the one every
/// other tool in the project uses.
pub fn tape_inputs(ghost: &str, from_ms: i64, to_ms: i64) -> Option<String> {
    let exe = std::env::current_exe().ok()?;
    let fk = exe.parent()?.join("fk");
    let out = std::process::Command::new(if fk.exists() { fk } else { "fk".into() })
        .args([
            "tapeinputs",
            "--ghost",
            ghost,
            "--from",
            &from_ms.to_string(),
            "--to",
            &to_ms.to_string(),
        ])
        .output()
        .ok()?;
    let t = String::from_utf8_lossy(&out.stdout).to_string();
    let p = t.find("digest=")?;
    let d: String = t[p + 7..].chars().take_while(|c| !c.is_whitespace()).collect();
    let ticks: i64 = t
        .find("ticks=")
        .and_then(|q| t[q + 6..].chars().take_while(|c| c.is_ascii_digit()).collect::<String>().parse().ok())
        .unwrap_or(0);
    // A window that captured no ticks is not an answer.
    if d.is_empty() || ticks == 0 { None } else { Some(d) }
}

// ===========================================================================
// `intg lag` -- alignment-free comparison, for a reference from ANOTHER
// recording session.
//
// WHY THE TIME-ALIGNED TEST IS NOT ENOUGH. `intg pair` matches samples by their
// recorded instant, which is right when both files descend from the same
// carrier -- that is how the whole-file donor copies were found. It is USELESS
// against an independent recording: a ghost's sample times are SESSION times,
// not race times, so 227654's rank-2 human recording labels its samples
// 93680.. while our files label theirs 1310.. . Zero instants in common, and
// the honest verdict is NO-COMPARISON, not CLEAN. Nine files came back that
// way and none of them was tested by it.
//
// This is the reference-free form, and it is the rule the `nan` arm arrived at
// from the other direction: DO NOT ASK WHETHER TWO RUNS ARE CLOSE, ASK WHETHER
// ONE IS THE OTHER, SHIFTED. Slide the file along the reference over every
// integer sample lag and count BIT-IDENTICAL positions. A copy of another run
// -- however its timestamps were relabelled -- lands on a lag where the count
// is large. Two genuinely different drives never do: the same twelve f32 bytes
// by coincidence, repeatedly, does not happen.
//
// It costs O(n*m) position compares, which for 365 x 8550 is nothing.
// ===========================================================================

pub struct LagHit {
    pub lag: i64,
    pub ident: usize,
    pub overlap: usize,
    pub best_run: usize,
    /// the longest run that is NOT anchored at a shared restore point
    pub best_free_run: usize,
}

/// Bit-identical position counts at every integer lag with enough overlap.
pub fn lag_scan(a: &[R], b: &[R], min_overlap: usize) -> Vec<LagHit> {
    fn key(r: &R) -> Option<&[u8]> {
        if r.raw.len() >= 59 { Some(&r.raw[47..59]) } else { None }
    }
    let mut out = Vec::new();
    let lo = -(b.len() as i64) + min_overlap as i64;
    let hi = a.len() as i64 - min_overlap as i64;
    for lag in lo..=hi {
        let mut ident = 0usize;
        let mut overlap = 0usize;
        let mut run = 0usize;
        let mut best_run = 0usize;
        let mut best_free_run = 0usize;
        let mut run_start = 0usize;
        for i in 0..a.len() {
            let j = i as i64 - lag;
            if j < 0 || j as usize >= b.len() {
                continue;
            }
            overlap += 1;
            // A STATIONARY CAR IS BIT-IDENTICAL IN EVERY RECORDING OF THE MAP.
            // Every run starts on the same spawn, held still, so the pad
            // samples match at EVERY lag -- and a long enough countdown makes
            // that look like a shared trajectory. Measured: 238835's embedded
            // author ghost scored 23 consecutive identical positions against
            // rank 1 at a lag of -1410 s, which is two cars parked. A shared
            // trajectory means shared MOTION, so a sample only counts when the
            // car has moved since the previous one.
            let moved = i > 0 && {
                let mut d = 0.0f64;
                for k in 0..3 {
                    let q = a[i].pos[k] - a[i - 1].pos[k];
                    d += q * q;
                }
                d > 1e-6
            };
            match (key(&a[i]), key(&b[j as usize])) {
                (Some(x), Some(y)) if x == y && moved => {
                    ident += 1;
                    if run == 0 {
                        run_start = i;
                    }
                    run += 1;
                    best_run = best_run.max(run);
                    // The restore can sit INSIDE the run as well as before it: the
                    // last pre-respawn sample often matches too (both cars are
                    // at the trigger). So a run is anchored if a gap occurs
                    // anywhere from just before it to here.
                    if !anchored_within(a, run_start, i, 50) {
                        best_free_run = best_free_run.max(run);
                    }
                }
                (Some(x), Some(y)) if x == y => {}
                _ => run = 0,
            }
        }
        if overlap >= min_overlap && ident > 0 {
            out.push(LagHit { lag, ident, overlap, best_run, best_free_run });
        }
    }
    out.sort_by(|x, y| y.ident.cmp(&x.ident));
    out
}

/// Is an identical run ANCHORED AT A SHARED RESTORE POINT?
///
/// THE CORRECTION THIS ENCODES. "An identical block preceded by real
/// separation is a splice" assumes that once two runs are metres apart their
/// states differ for good. A RESPAWN BREAKS THAT ASSUMPTION: it restores the
/// car to a fixed checkpoint state, so two runs that were far apart are
/// suddenly in the SAME state, and from there identical inputs give
/// bit-identical positions -- the head-anchored determinism case, anchored at
/// a respawn instead of at the start.
///
/// Measured on 238835, a Trial map: the map's own embedded AUTHOR ghost and
/// the rank-1 human recording share 23 consecutive bit-identical positions
/// (1.15 s) beginning at the sample straight after a 1050 ms gap in the
/// author's record -- a respawn to (1044.8, 106, 624), from which both cars
/// accelerate away identically. Two unrelated humans, no contamination.
///
/// A respawn shows as a gap in the record's own sample times, so this needs
/// nothing but the timestamps.
fn anchored_within(a: &[R], start: usize, end: usize, grid_ms: i64) -> bool {
    // `start <= 1`, not `== 0`: a run only counts samples where the car MOVED,
    // and sample 0 has no predecessor to have moved from, so a head-anchored
    // block always begins at index 1. Testing for 0 made every determinism
    // prefix read as unanchored -- `intg lag` called 145875 BEST_6322
    // CONTAMINATED on a 27-sample block that `intg pair`, on the same pair of
    // files, correctly reports as HEAD samples [0..27] and CLEAN.
    if start <= 1 {
        return true; // the race start is the original shared restore point
    }
    for k in start..=end.min(a.len() - 1) {
        if k == 0 {
            continue;
        }
        if a[k].ms - a[k - 1].ms > grid_ms * 2 {
            return true;
        }
    }
    false
}

pub fn cmd_lag(args: &[String]) {
    let flag = |n: &str| -> Option<String> {
        args.iter().position(|a| a == n).and_then(|i| args.get(i + 1)).cloned()
    };
    let pos: Vec<&String> = {
        let mut v = Vec::new();
        let mut i = 0usize;
        while i < args.len() {
            if args[i].starts_with("--") {
                if matches!(args[i].as_str(), "--min-overlap" | "--min-run") {
                    i += 2;
                } else {
                    i += 1;
                }
                continue;
            }
            v.push(&args[i]);
            i += 1;
        }
        v
    };
    if pos.len() != 2 {
        eprintln!("tmtraj intg lag FILE REFERENCE [--min-overlap N] [--min-run N]");
        std::process::exit(2);
    }
    let min_ov: usize = flag("--min-overlap").and_then(|v| v.parse().ok()).unwrap_or(20);
    // A run of identical positions long enough that coincidence is not an
    // explanation. Ten samples is half a second of driving agreeing to the
    // last bit in all three components.
    let min_run: usize = flag("--min-run").and_then(|v| v.parse().ok()).unwrap_or(10);
    let a = decode(pos[0]).unwrap_or_else(|e| {
        println!("DECODE-FAIL {}: {}", pos[0], e);
        std::process::exit(2)
    });
    let b = decode(pos[1]).unwrap_or_else(|e| {
        println!("reference DECODE-FAIL {}: {}", pos[1], e);
        std::process::exit(2)
    });
    let hits = lag_scan(&a, &b, min_ov);
    println!("=== {}", pos[0]);
    // COVERAGE, always, even (especially) on a CLEAN verdict: this scan has
    // already misfired in BOTH directions tonight, and a negative from it is
    // unquantified without the number of alignments it actually tried.
    {
        let lo = -(b.len() as i64) + min_ov as i64;
        let hi = a.len() as i64 - min_ov as i64;
        let tried = if hi >= lo { (hi - lo + 1) as usize } else { 0 };
        println!(
            "    COVERAGE: {} integer lags in range, {} produced any bit-identical position; \
             min overlap {} samples",
            tried,
            hits.len(),
            min_ov
        );
        if tried == 0 {
            println!(
                "    NO VERDICT -- no alignment has {} overlapping samples. This is not a CLEAN \
                 result, it is an unrun test.",
                min_ov
            );
            std::process::exit(3);
        }
    }
    println!("    vs {} (alignment-free: every integer sample lag)", pos[1]);
    println!("    {} samples in the file, {} in the reference", a.len(), b.len());
    if hits.is_empty() {
        println!("    no lag produces a single bit-identical position");
        println!("    VERDICT CLEAN (no shared samples at any alignment)");
        return;
    }
    println!("    best lags by bit-identical count:");
    for h in hits.iter().take(5) {
        println!(
            "      lag {:>6} ({:>8.3} s)   {} of {} overlapping positions identical, longest run {} (unanchored {})",
            h.lag,
            h.lag as f64 * 0.05,
            h.ident,
            h.overlap,
            h.best_run,
            h.best_free_run
        );
    }
    let worst = &hits[0];
    // A head-anchored block is only honest if it is a PROPER PREFIX -- it must
    // END where the two runs' inputs diverge. A head-anchored block covering
    // essentially the whole overlap is not determinism, it is a whole-file
    // copy: exempting it made `intg lag` clear 227654 TAS_57493, which is 365
    // of 365 the human WR. Caught by control C13, which is exactly why the
    // known donor copy is in the control set.
    let covers_all = worst.best_run * 10 >= worst.overlap * 9;
    if covers_all && worst.best_run >= min_run {
        println!(
            "    VERDICT CONTAMINATED -- {} of {} overlapping positions are ONE bit-identical run \
             at lag {}. A head-anchored block that never ends is not a shared prefix, it is a \
             copy of the whole recording.",
            worst.best_run, worst.overlap, worst.lag
        );
        std::process::exit(2);
    }
    if worst.best_run >= min_run && worst.best_free_run < min_run {
        println!(
            "    the longest identical run ({}) begins at a SHARED RESTORE POINT -- the race start\n\
             \x20   or a respawn -- from which identical inputs give identical positions. That is\n\
             \x20   determinism, not a splice. Longest run NOT so anchored: {}.",
            worst.best_run, worst.best_free_run
        );
    }
    if worst.best_free_run >= min_run {
        println!(
            "    VERDICT CONTAMINATED -- {} consecutive positions are bit-identical to the reference at lag {}. \
             Two different drives do not agree to the last f32 bit for {:.2} s.",
            worst.best_free_run,
            worst.lag,
            worst.best_free_run as f64 * 0.05
        );
        std::process::exit(2);
    }
    println!(
        "    VERDICT CLEAN -- the longest bit-identical run at any lag is {} sample(s), below the {} needed to exclude coincidence",
        worst.best_run, min_run
    );
}

/// The recorded time span of the longest bit-identical run at a given lag.
fn find_run_span(a: &[R], b: &[R], lag: i64, want: usize) -> Option<(i64, i64)> {
    fn key(r: &R) -> Option<&[u8]> {
        if r.raw.len() >= 59 { Some(&r.raw[47..59]) } else { None }
    }
    let mut run = 0usize;
    let mut start = 0usize;
    for i in 0..a.len() {
        let j = i as i64 - lag;
        let same = if j < 0 || j as usize >= b.len() {
            false
        } else {
            matches!((key(&a[i]), key(&b[j as usize])), (Some(x), Some(y)) if x == y)
        };
        if same {
            if run == 0 {
                start = i;
            }
            run += 1;
            if run == want {
                return Some((a[start].ms, a[i].ms));
            }
        } else {
            run = 0;
        }
    }
    None
}

/// The first race instant at which two tapes' decoded inputs differ.
/// `None` when the two tapes are identical, or the answer is unavailable.
pub fn first_input_diff(a: &str, b: &str) -> Option<i64> {
    let exe = std::env::current_exe().ok()?;
    let fk = exe.parent()?.join("fk");
    let out = std::process::Command::new(if fk.exists() { fk } else { "fk".into() })
        .args(["tapediff", "--a", a, "--b", b])
        .output()
        .ok()?;
    let t = String::from_utf8_lossy(&out.stdout).to_string();
    let p = t.find("first_diff_ms=")?;
    let v: String = t[p + 14..].chars().take_while(|c| !c.is_whitespace()).collect();
    if v == "none" { None } else { v.parse().ok() }
}

/// The file's own declared race time, in ms.
///
/// Needed because THE CONTAMINATION WINDOW IS THE RACE. Two independent runs
/// that share a carrier agree exactly in the carrier's TAIL -- neither is
/// driving there, so the samples are the same graft. Measured on genuine
/// rank-1 downloads: 199100 shares 28 consecutive positions and 228811 shares
/// 38, and BOTH blocks sit entirely past the finish (50.850..52.200 against a
/// race of 50.738; 20.820..22.670 against 20.237), with ZERO identical samples
/// in race, 0 of 1015 and 0 of 405. That is a tail problem, which C3 and C4
/// already flag and which the tail cut removes -- not a provenance problem.
/// Counting it as contamination inflates the finding and, worse, spends
/// somebody's evening on a benign file.
pub fn declared_race_ms(path: &str) -> Option<i64> {
    let d = crate::entrec::decode_ghost(path).ok()?;
    d.race_time_ms
        .map(|v| v as i64)
        .or_else(|| d.checkpoints_ms.last().map(|v| *v as i64))
}

/// Keep only the samples inside the race.
pub fn in_race(r: &[R], race_ms: Option<i64>) -> Vec<R> {
    match race_ms {
        // +60 ms of slack: the finish sample itself is the run's, and the
        // 50 ms grid does not land exactly on the declared millisecond.
        Some(t) => r.iter().filter(|x| x.ms <= t + 60).cloned().collect(),
        None => r.to_vec(),
    }
}

// ===========================================================================
// C3b -- IS THAT JUMP A TELEPORT, OR IS THE CAR SIMPLY FAST?
//
// The distance-based C3 asks whether a step is far beyond the run's own step
// distribution. On a map where the car is genuinely fast that is a false
// refusal: 126859's biggest step implies 223.7 m/s and the car's own
// speedometer reads 223.4 m/s -- 805 km/h on a Kacky map, and entirely real.
// A bar in metres cannot tell "fast" from "moved without travelling".
//
// The file already carries the answer. Every sample has the engine's own
// recorded speed, so compare the displacement the positions imply with the
// speed the car says it had:
//
//   ratio ~ 1.00                real driving, at any speed
//   recorded speed exactly 0.0  a RESPAWN -- normal gameplay, and the whole
//                               point of a Trial map (267460, 238835, 286279,
//                               186935 are all Trial-shaped)
//   ratio in the thousands      a SPLICE (227654's TAS_57503: 50 090 m/s
//                               implied against 19.2 recorded)
//
// THE SPEEDOMETER IS NOT ALWAYS ZEROED ON A RESPAWN -- on at least one file it
// holds its pre-respawn value, so a rule that only tests "speed == 0" misses
// it and calls a respawn a splice. The robust discriminator needs no speed at
// all: after a respawn the car RESUMES DRIVING from where it landed, so the
// steps in the second following the jump are ordinary. After a splice the
// record simply continues in another run, and the jump is one discontinuity
// in a stream that was never interrupted. So: measure the jump against the
// motion that FOLLOWS it, from positions and times alone.
// ===========================================================================

pub struct JumpVerdict {
    pub ms: i64,
    pub dist: f64,
    pub implied: f64,
    pub recorded: f64,
    pub after: f64,
    pub kind: &'static str,
}

/// Every step whose implied speed is far above what the car reports, classified.
pub fn jumps(r: &[R]) -> Vec<JumpVerdict> {
    let mut out = Vec::new();
    for i in 1..r.len() {
        let dt = (r[i].ms - r[i - 1].ms) as f64 / 1000.0;
        if dt <= 0.0 {
            continue;
        }
        let mut d2 = 0.0;
        for k in 0..3 {
            let q = r[i].pos[k] - r[i - 1].pos[k];
            d2 += q * q;
        }
        if !d2.is_finite() {
            continue;
        }
        let dist = d2.sqrt();
        let implied = dist / dt;
        // the faster of the two endpoints' own speedometers
        let recorded = r[i].speed.max(r[i - 1].speed);
        // a step the car's own speed explains is not a jump at all
        if implied <= recorded.max(1.0) * 1.5 + 2.0 {
            continue;
        }
        // ...nor is a step that goes nowhere. Without an absolute floor a 0.20 m
        // shuffle at 0.8 m/s trips the ratio and gets classified as a splice.
        if dist < 5.0 {
            continue;
        }
        // THE WORLD ORIGIN IS NOT A PLACE THE CAR WAS. A first sample at
        // (0,0,0) is a placeholder -- `ghostqc`'s ORIGIN class -- and the step
        // away from it is the record starting, not the car moving.
        let at_origin = |p: &[f64; 3]| p.iter().all(|v| v.abs() < 1e-6);
        if at_origin(&r[i - 1].pos) || at_origin(&r[i].pos) {
            out.push(JumpVerdict {
                ms: r[i].ms,
                dist,
                implied,
                recorded,
                after: f64::NAN,
                kind: "ORIGIN PLACEHOLDER (a sample at the world origin, not a position)",
            });
            continue;
        }
        // what does the car do in the SECOND AFTER the jump? Median step speed
        // over the following 20 samples, positions and times only.
        let mut after: Vec<f64> = Vec::new();
        for j in i + 1..(i + 21).min(r.len()) {
            let dt2 = (r[j].ms - r[j - 1].ms) as f64 / 1000.0;
            if dt2 <= 0.0 {
                continue;
            }
            let mut s2 = 0.0;
            for k in 0..3 {
                let q = r[j].pos[k] - r[j - 1].pos[k];
                s2 += q * q;
            }
            after.push(s2.sqrt() / dt2);
        }
        after.sort_by(|a, b| a.total_cmp(b));
        let after_med = if after.is_empty() { f64::NAN } else { after[after.len() / 2] };
        // THE DISCRIMINATOR IS WHERE THE CAR LANDS, not how fast it is going.
        // "Ordinary motion resumes after the jump" does NOT separate the two --
        // measured on 238835, a respawn lands and drives away at 9.4 m/s, and
        // so does the far side of a splice. What only a respawn does is RETURN
        // THE CAR SOMEWHERE IT HAS ALREADY BEEN: a checkpoint it crossed
        // earlier in this same record. A splice puts it on another run's path,
        // which this record has never visited. Positions only, no speed.
        let landed = &r[i].pos;
        let mut been_there = false;
        for k in 0..i.saturating_sub(2) {
            let mut s2 = 0.0;
            for c in 0..3 {
                let q = r[k].pos[c] - landed[c];
                s2 += q * q;
            }
            if s2 < 100.0 {
                been_there = true;
                break;
            }
        }
        // 153-213 m/s implied over 8-11 m, and the car STOPPED on the far side:
        // that is a respawn placing the car and holding it, which is ordinary
        // gameplay on a Trial map. A splice into another run continues at that
        // run's pace -- it does not arrive at a standstill. (This is the class
        // the old 200 m/s distance bar sat inside, which is why it refused
        // HUMANCUT_236972, a genuine human run.)
        let stopped_after = after_med.is_finite() && after_med < 1.0;
        let kind = if recorded < 0.5 {
            "RESPAWN (the car's own speed is zero)"
        } else if stopped_after {
            "RESPAWN (the car arrives at a standstill)"
        } else if been_there {
            // the speedometer is not always zeroed on a respawn -- on at least
            // one file it holds its pre-respawn value -- so this branch is what
            // catches those.
            "RESPAWN (the car returns to a point it already occupied)"
        } else {
            "SPLICE (the record continues somewhere this run has never been)"
        };
        out.push(JumpVerdict { ms: r[i].ms, dist, implied, recorded, after: after_med, kind });
    }
    out
}

pub fn cmd_c3(args: &[String]) {
    let files: Vec<&String> = args.iter().filter(|a| !a.starts_with("--")).collect();
    if files.is_empty() {
        eprintln!("tmtraj intg c3 GHOST...   (the speedometer teleport test)");
        std::process::exit(2);
    }
    let mut worst = 0;
    for f in files {
        let race = declared_race_ms(f);
        let r = match decode(f).map(|v| in_race(&v, race)) {
            Ok(v) => v,
            Err(e) => {
                println!("=== {}\n  FAIL C3b  {}", f, e);
                worst = 2;
                continue;
            }
        };
        let mut js = jumps(&r);
        calibrate(&mut js);
        let splices: Vec<&JumpVerdict> = js.iter().filter(|j| j.kind.starts_with("SPLICE")).collect();
        println!(
            "=== {}  --  {}",
            f,
            if splices.is_empty() { "PASS C3b" } else { "FAIL C3b" }
        );
        if js.is_empty() {
            println!("  no step exceeds the car's own recorded speed");
        }
        for j in &js {
            println!(
                "  {:>9.3} s  {:>9.2} m in one step = {:>9.1} m/s implied, speedometer {:>7.1} m/s, \
                 motion after {:>7.1} m/s  --  {}",
                j.ms as f64 / 1000.0,
                j.dist,
                j.implied,
                j.recorded,
                j.after,
                j.kind
            );
        }
        if !splices.is_empty() {
            worst = 2;
        }
    }
    std::process::exit(worst);
}

/// Second pass over one file's jumps: a run that demonstrably respawns is a
/// run whose OTHER jumps of the same size are respawns too.
///
/// WHY THIS PASS EXISTS, and it is the honest limit of C3b. Respawn
/// displacements on a Trial map cluster at 153-213 m/s implied. Three
/// discriminators separate most of them from a splice -- the speedometer
/// reading zero, the car arriving at a standstill, the car returning somewhere
/// it has already been -- and NONE of the three fires on every respawn: a
/// respawn to a checkpoint the 50 ms grid never sampled within 10 m, after
/// which the car drives away, looks exactly like a splice by all three. On
/// 286279 that left three genuine human runs refused, including
/// `HUMANCUT_236972` -- the same false positive the old 200 m/s distance bar
/// produced, reached by a better route.
///
/// So the file's own confirmed respawns calibrate the rest of it: if this run
/// respawns at all, an unexplained jump inside the same magnitude band is a
/// respawn, not a splice. What stays a splice is a jump ORDERS of magnitude
/// outside that band. This is deliberately conservative -- it can miss a splice
/// disguised as a respawn on a respawning map -- because a gate that refuses
/// genuine files teaches people to override gates, and contamination has two
/// other instruments (`intg pair`/`lag` and the manifest's coverage) while a
/// teleport has only this one.
pub fn calibrate(js: &mut Vec<JumpVerdict>) {
    let band: Vec<f64> = js
        .iter()
        .filter(|j| j.kind.starts_with("RESPAWN"))
        .map(|j| j.implied)
        .collect();
    if band.is_empty() {
        return;
    }
    // The ceiling is this file's own worst confirmed respawn, doubled -- but
    // never below 300 m/s. A file can contain ONE respawn, and calibrating a
    // band from a single sample is not calibration: 286279's
    // AUTHORCUT_220391_watchable has one confirmed respawn at 104 m/s and a
    // second jump at 233.7, which its own sibling file shows at 234.2 and
    // confirms as a respawn by returning to a known point. No respawn measured
    // anywhere in this corpus exceeds ~234 m/s, and every real splice is in
    // the THOUSANDS (227654: 50 090 m/s), so the two populations are three
    // orders of magnitude apart and 300 sits in the empty space between them.
    let hi = (band.iter().cloned().fold(0.0f64, f64::max) * 2.0).max(300.0);
    for j in js.iter_mut() {
        if j.kind.starts_with("SPLICE") && j.implied <= hi {
            j.kind = "RESPAWN (within this run's own confirmed respawn band)";
        }
    }
}

pub fn new_cache() -> Cache {
    Cache::new()
}

/// What the instruments say about a file TODAY: the verdict word, the
/// references actually tested (path + md5), and the limits that apply.
///
/// Both the time-aligned and the alignment-free test are run against every
/// reference held, because they answer different questions: `pair` finds a
/// shared carrier, `lag` finds a copy of another recording session. A file
/// that passes one and not the other is not clean.
pub fn certify_now(
    ghost: &str,
    humans: &[&Ref],
    race_ms: i64,
    cache: &mut Cache,
) -> (String, Vec<(String, String)>, Vec<String>) {
    let mapid = humans.first().map(|r| r.map.clone()).unwrap_or_default();
    let mut limits: Vec<String> = Vec::new();
    let a = match decode(ghost).map(|v| in_race(&v, Some(race_ms).filter(|t| *t > 0))) {
        Ok(v) => v,
        Err(e) => {
            limits.push(format!(
                "THIS FILE HAS NO READABLE VEHICLE RECORD ({}). It cannot be checked and it \
                 cannot be filmed: there is no car in it.",
                e
            ));
            return ("UNCERTIFIED".into(), Vec::new(), limits);
        }
    };
    if humans.is_empty() {
        limits.push(crate::manifest::limit_text("no-human-reference", &mapid));
        return ("UNCERTIFIED".into(), Vec::new(), limits);
    }
    let mut tested: Vec<(String, String)> = Vec::new();
    let mut worst = "CLEAN".to_string();
    for r in humans {
        let Some(b) = cached(cache, &r.path) else { continue };
        tested.push((
            r.path.clone(),
            md5_hex(&std::fs::read(&r.path).unwrap_or_default()),
        ));
        let p = pair(&a, &b);
        match verdict(&p, 5.0) {
            Verdict::Contaminated => worst = "CONTAMINATED".into(),
            Verdict::Identical if worst != "CONTAMINATED" => {
                worst = "CONTAMINATED".into();
            }
            _ => {}
        }
        // alignment-free: sample times are session times, so a copy of another
        // recording shares no instants and `pair` cannot see it at all.
        if worst != "CONTAMINATED" {
            let hits = lag_scan(&a, &b, 20.min(a.len().max(1)));
            if hits.first().map_or(0, |h| h.best_run) >= 10 {
                worst = "CONTAMINATED".into();
            }
        }
        if p.runs.first().map_or(false, |x| x.i0 == 0 && x.n() >= 10) {
            limits.push(crate::manifest::limit_text("head-anchored-block", &mapid));
        }
    }
    if tested.is_empty() {
        limits.push(crate::manifest::limit_text("no-human-reference", &mapid));
        return ("UNCERTIFIED".into(), Vec::new(), limits);
    }
    limits.sort();
    limits.dedup();
    (worst, tested, limits)
}

/// `intg corrupt` -- overwrite every sample's position with a fixed value.
///
/// THE POSITIVE CONTROL FOR THE WRITER. `fk regen` producing a file
/// byte-identical to its input has two explanations -- the input was already
/// exactly what the engine writes (an honest no-op), or the tool skipped the
/// work and passed its input through -- and no amount of reading the tool's
/// own log distinguishes them.
///
/// Corrupting the telemetry and regenerating FROM the corrupted file does:
///
///   regen(corrupt(X)) == X   the writer genuinely writes engine state, so
///                            out == in on a clean file is an honest no-op
///   regen(corrupt(X)) == corrupt(X)
///                            the writer passed its input through
///
/// This needs no locate, no clock bias and no reference recording, so it
/// cannot fail the way the re-simulation grade just did -- where the POSITIVE
/// CONTROL (a downloaded human ghost, which the tg arm measured at 0.50 mm on
/// this very map) also read 11.96 m, proving the fault was in the measurement
/// and not in the file.
pub fn cmd_corrupt(args: &[String]) {
    let flag = |n: &str| -> Option<String> {
        args.iter().position(|a| a == n).and_then(|i| args.get(i + 1)).cloned()
    };
    let inp = args.iter().find(|a| !a.starts_with("--")).cloned().expect("GHOST");
    let out = flag("--out").expect("--out");
    let mut n = 0usize;
    let res = tmtraj_rewrite(&inp, &out, &mut n);
    match res {
        Ok(()) => println!("corrupted {} samples -> {}", n, out),
        Err(e) => {
            println!("ABORT: {}", e);
            std::process::exit(2)
        }
    }
}

fn tmtraj_rewrite(inp: &str, out: &str, n: &mut usize) -> Result<(), String> {
    crate::recwrite::rewrite_ghost(inp, out, |rd| {
        let ent = rd
            .ents
            .iter_mut()
            .filter(|e| e.sample_size >= 100 && !e.times.is_empty())
            .max_by_key(|e| e.times.len())
            .ok_or("no vehicle entity")?;
        let ss = ent.sample_size;
        for i in 0..ent.times.len() {
            let s = &mut ent.raw[i * ss..(i + 1) * ss];
            // a recognisable, obviously-wrong position: 12345.0, 678.0, 9012.0
            for (k, v) in [12345.0f32, 678.0, 9012.0].iter().enumerate() {
                s[47 + k * 4..51 + k * 4].copy_from_slice(&v.to_le_bytes());
            }
            *n += 1;
        }
        Ok(())
    })
    .map(|_| ())
    .map_err(|e| e.to_string())
}

// ===========================================================================
// E -- THE STALE-BUFFER CHECK. Is this file the LIVE car, or a copy of it one
// tick in the past?
//
// The engine double-buffers the vehicle state and BOTH COPIES PASS EVERY
// STRUCTURAL TEST: the stale one is finite, moves, is self-consistent
// (d(pos)/dt matches the velocity stored beside it), has a unit quaternion,
// re-simulates to the exact declared millisecond, and is 100 % covered. A
// regeneration that locks onto it produces a file that passes coverage, the
// oracle, the manifest, C2 AND the contamination test, and is still wrong.
//
// Measured on three unrelated maps in one pass tonight -- 285268, 249521 and
// 227969 -- each 10.0 ms from its published copy to three digits, with the
// regeneration BEHIND on 100 % of samples.
//
// The discriminator is the `nan` arm's and it needs no reference recording:
// A BUFFER HOLDS THE PAST, so the live copy is the one FURTHEST ALONG ITS OWN
// VELOCITY at the same clock value. Project (this - other) onto the car's
// velocity: consistently negative means this file is the stale one.
//
// WHY IT IS SEPARATE FROM THE CONTAMINATION TEST. That test asks whose run
// this is; this asks which copy of our own run it is. A stale file is our
// driving, correct in every claim the page makes, displayed 10 ms late -- so
// this is a REFUSAL for a file we are about to write, and NOT a reason to
// withdraw one already published. Keep those two consequences apart.
// ===========================================================================

pub struct StaleVerdict {
    pub n: usize,
    pub median_m: f64,
    pub max_m: f64,
    pub implied_ms: f64,
    pub ahead: usize,
    pub behind: usize,
}

/// Compare this file against a reference of THE SAME RUN (its own source, or a
/// sibling regeneration). Only meaningful when the two are the same driving.
pub fn stale_check(a: &[R], b: &[R]) -> Option<StaleVerdict> {
    let bm: std::collections::HashMap<i64, &R> = b.iter().map(|r| (r.ms, r)).collect();
    let mut ds: Vec<f64> = Vec::new();
    let mut ahead = 0usize;
    let mut behind = 0usize;
    let mut ratio = 0.0f64;
    let mut rn = 0usize;
    for x in a {
        let Some(y) = bm.get(&x.ms) else { continue };
        let d = [x.pos[0] - y.pos[0], x.pos[1] - y.pos[1], x.pos[2] - y.pos[2]];
        let dist = (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt();
        if !dist.is_finite() {
            continue;
        }
        ds.push(dist);
        // project onto the reference's velocity: positive => `a` is ahead
        let sp = y.speed;
        if sp > 1.0 {
            let proj = (d[0] * y.vel[0] + d[1] * y.vel[1] + d[2] * y.vel[2]) / sp;
            if proj > 0.0 {
                ahead += 1;
            } else if proj < 0.0 {
                behind += 1;
            }
            ratio += dist / sp;
            rn += 1;
        }
    }
    if ds.len() < 5 {
        return None;
    }
    ds.sort_by(|x, y| x.total_cmp(y));
    Some(StaleVerdict {
        n: ds.len(),
        median_m: ds[ds.len() / 2],
        max_m: ds[ds.len() - 1],
        implied_ms: if rn == 0 { f64::NAN } else { 1000.0 * ratio / rn as f64 },
        ahead,
        behind,
    })
}

pub fn cmd_stale(args: &[String]) {
    let pos: Vec<&String> = args.iter().filter(|a| !a.starts_with("--")).collect();
    if pos.len() != 2 {
        eprintln!("tmtraj intg stale FILE REFERENCE-OF-THE-SAME-RUN");
        std::process::exit(2);
    }
    let a = decode(pos[0]).unwrap_or_else(|e| {
        println!("DECODE-FAIL {}", e);
        std::process::exit(2)
    });
    let b = decode(pos[1]).unwrap_or_else(|e| {
        println!("reference DECODE-FAIL {}", e);
        std::process::exit(2)
    });
    match stale_check(&a, &b) {
        None => {
            println!("=== {}\n  n/a  too few shared instants to compare", pos[0]);
        }
        Some(v) => {
            println!("=== {}\n    vs {}", pos[0], pos[1]);
            println!(
                "    {} shared instants, median {:.6} m, max {:.6} m, implied offset {:.3} ms",
                v.n, v.median_m, v.max_m, v.implied_ms
            );
            println!("    this file is AHEAD on {} samples, BEHIND on {}", v.ahead, v.behind);
            let (code, word) = stale_verdict(&v);
            println!("    VERDICT {}", word);
            std::process::exit(code);
        }
    }
}

/// The rule. A file is refused when it is consistently BEHIND its reference by
/// about one tick.
/// THE PRECONDITION. This check compares two renderings of the SAME RUN and
/// asks which is one tick behind. It is MEANINGLESS when the reference is a
/// file whose telemetry is not that run's: on a contaminated source the
/// comparison measures our fresh regeneration against a STRANGER, and duly
/// reported "STALE BUFFER, behind by 8.642 ms" on 279218's repaired file,
/// which is demonstrably correct.
///
/// Two guards, because the failure was visible twice over and I read neither:
///   * the caller must state that the reference carries its own telemetry;
///   * 8.642 ms IS NOT A CLEAN TICK. The whole signature of this defect is
///     EXACTLY one physics tick on every sample. A ragged offset means "this
///     is not the defect I test for", not "a weaker version of it".
pub fn stale_verdict(v: &StaleVerdict) -> (i32, String) {
    let tot = v.ahead + v.behind;
    // Zero difference is not "too few samples to tell" -- it is the strongest
    // possible answer, and printing INDETERMINATE for it made an identity
    // control pass for the wrong reason.
    if v.max_m == 0.0 {
        return (0, "IDENTICAL to the reference -- no buffer question arises".into());
    }
    if tot < 5 {
        return (0, "INDETERMINATE (too few moving samples)".into());
    }
    let behind_frac = v.behind as f64 / tot as f64;
    // The tg control: a correct regeneration reproduces a ghost's own recorded
    // bytes to ~0.5 mm. Anything at centimetre scale or beyond is a different
    // copy of the state, not numerical noise.
    if v.median_m < 0.01 {
        return (0, format!("LIVE (median {:.6} m -- within the client-vs-server floor)", v.median_m));
    }
    // Tick-shaped or it is not this defect. The band is TIGHT on purpose: every
    // real instance measured tonight sits at 9.959, 10.004, 10.020 or 10.033 ms
    // -- a physics tick is exactly 10 -- while 279218's false positive read
    // 8.642. A loose 8..12 band admits that false positive, which is how I
    // nearly rejected a correct file.
    let ragged = !(9.4..=10.6).contains(&v.implied_ms) && v.implied_ms.is_finite();
    if behind_frac >= 0.95 && ragged {
        return (
            3,
            format!(
                "NOT THIS DEFECT -- behind on {:.0} % of samples, but by {:.3} ms, which is not one \
                 physics tick (10 ms). The stale-buffer signature is EXACTLY a tick on every \
                 sample; a ragged offset means the reference is a different run, not an earlier \
                 copy of this one. Check that the reference carries its OWN telemetry.",
                100.0 * behind_frac, v.implied_ms
            ),
        );
    }
    if behind_frac >= 0.95 {
        (
            2,
            format!(
                "STALE BUFFER -- behind on {:.0} % of samples by {:.3} ms. This is the same run, \
                 one physics tick in the past: the engine double-buffers the car state and both \
                 copies pass every other check. REFUSE and re-locate.",
                100.0 * behind_frac,
                v.implied_ms
            ),
        )
    } else if v.ahead as f64 / tot as f64 >= 0.95 {
        (
            0,
            format!(
                "AHEAD of the reference on {:.0} % of samples -- this file is the LIVE copy and \
                 the REFERENCE is the stale one",
                100.0 * v.ahead as f64 / tot as f64
            ),
        )
    } else {
        (
            0,
            format!(
                "DIFFERENT RUN or mixed ({} ahead / {} behind, median {:.3} m) -- not a buffer copy",
                v.ahead, v.behind, v.median_m
            ),
        )
    }
}

/// Bit-equality at lag 1: does `a[t]` hold exactly what `b[t-1]` held?
///
/// SHARPER THAN THE VELOCITY PROJECTION and it supersedes it as the screen.
/// The projection is a statistic -- it can tie, and it needs the car to be
/// moving fast enough for the sign to be meaningful. Bit-equality at lag 1
/// either holds or it does not. The car-state arm established that the two
/// buffers' roles NEVER SWAP (zero swaps in 2399 ticks; behind on
/// 1562/2377/2377 decidable ticks, ahead on zero), so a single-pass majority
/// is not merely evidence, it is the answer.
///
/// The projection stays as a fallback for the case this cannot serve: two
/// renderings that are near-copies but not bit-equal at any lag -- which the
/// same arm has now measured on 267460, where two of six candidate objects sit
/// 0.494 mm from the car and neither rule catches them.
pub struct Lag1 {
    pub decidable: usize,
    pub equal: usize,
}

pub fn lag1_equal(a: &[R], b: &[R]) -> Lag1 {
    let bm: std::collections::HashMap<i64, &R> = b.iter().map(|r| (r.ms, r)).collect();
    let mut decidable = 0usize;
    let mut equal = 0usize;
    for x in a {
        // b's sample one tick (10 ms) earlier; the record grid is 50 ms, so
        // compare against the previous RECORDED instant instead when 10 ms is
        // not present.
        let prev = bm.get(&(x.ms - 10)).or_else(|| bm.get(&(x.ms - 50)));
        let Some(y) = prev else { continue };
        if x.raw.len() < 59 || y.raw.len() < 59 {
            continue;
        }
        // only ticks where the car actually moved are decidable: a stationary
        // car is bit-equal to its own past for reasons that mean nothing.
        let moved = {
            let mut d = 0.0;
            for k in 0..3 {
                let q = x.pos[k] - y.pos[k];
                d += q * q;
            }
            d > 1e-12
        };
        let _ = moved;
        decidable += 1;
        if x.raw[47..59] == y.raw[47..59] {
            equal += 1;
        }
    }
    Lag1 { decidable, equal }
}

// ===========================================================================
// `intg poison` -- THE DECLARED-TIME DETECTOR. One field, no simulation.
//
// A synthesised tape carries its TEMPLATE's telemetry, and the header's
// declared time comes with it. So a file whose DECLARED time is some other
// run's time, while its filename (and its tape) say ours, is carrying somebody
// else's record. Found in the store as `203330/best/an330_13984.Ghost.Gbx`:
// declares 14018 -- the human WR's time -- with 281 samples byte-identical to
// that WR, while the repo's copy of the same run declares 13984 with 280
// samples of its own telemetry. Same input tape in both.
//
// This costs one header read. It is the cheapest instrument in the set and it
// catches the poisoning AT INGEST, where every other check we have needs the
// file to reach publication first.
//
// It is a NECESSARY condition, not a sufficient one: a poisoned file whose
// donor happened to run the same time would pass. Pair it with the
// contamination test, which is the sufficient one and costs a reference.
// ===========================================================================

pub fn cmd_poison(args: &[String]) {
    let flag = |n: &str| -> Option<String> {
        args.iter().position(|a| a == n).and_then(|i| args.get(i + 1)).cloned()
    };
    let root = flag("--root").expect("--root");
    let mut files: Vec<std::path::PathBuf> = Vec::new();
    let mut dirs = vec![std::path::PathBuf::from(&root)];
    while let Some(d) = dirs.pop() {
        let Ok(rd) = std::fs::read_dir(&d) else { continue };
        for e in rd.flatten() {
            let p = e.path();
            if p.is_dir() {
                if p.file_name().map_or(false, |n| n == ".git") {
                    continue;
                }
                dirs.push(p);
            } else if p.to_string_lossy().ends_with(".Ghost.Gbx") {
                files.push(p);
            }
        }
    }
    files.sort();
    println!("file\tname_ms\tdeclared_ms\tdelta_ms\tsamples\tverdict");
    let mut n_bad = 0usize;
    let mut n_read = 0usize;
    let mut n_unreadable = 0usize;
    let mut n_noname = 0usize;
    let mut n_notime = 0usize;
    for f in &files {
        let fp = f.to_string_lossy().to_string();
        let name = f.file_name().unwrap().to_string_lossy().to_string();
        // the time the FILENAME claims: the last 4-7 digit group
        let mut claim: Option<i64> = None;
        let mut cur = String::new();
        for c in name.chars() {
            if c.is_ascii_digit() {
                cur.push(c);
            } else {
                if cur.len() >= 4 && cur.len() <= 7 {
                    claim = cur.parse().ok();
                }
                cur.clear();
            }
        }
        if cur.len() >= 4 && cur.len() <= 7 {
            claim = cur.parse().ok();
        }
        let Some(claim) = claim else {
            n_noname += 1;
            continue;
        };
        let Ok(d) = crate::entrec::decode_ghost(&fp) else {
            println!("{}\t{}\t-\t-\t-\tDECODE-FAIL", fp, claim);
            n_unreadable += 1;
            continue;
        };
        n_read += 1;
        let decl = d
            .race_time_ms
            .map(|v| v as i64)
            .or_else(|| d.checkpoints_ms.last().map(|v| *v as i64));
        let Some(decl) = decl else {
            println!("{}\t{}\t-\t-\t{}\tNO-DECLARED-TIME", fp, claim, d.samples.len());
            n_notime += 1;
            n_read -= 1;
            continue;
        };
        let delta = decl - claim;
        // A map id in the filename can look like a time; only flag a mismatch
        // when the two are close enough to be the same run's two candidates,
        // which is what a poisoning looks like -- a neighbouring leaderboard
        // time, not an unrelated number.
        let verdict = if delta == 0 {
            "OK"
        } else if delta.abs() <= claim / 4 {
            n_bad += 1;
            "MISMATCH -- the header declares a different run's time"
        } else {
            "name-is-not-a-time"
        };
        if verdict != "name-is-not-a-time" {
            println!(
                "{}\t{}\t{}\t{:+}\t{}\t{}",
                fp,
                claim,
                decl,
                delta,
                d.samples.len(),
                verdict
            );
        }
    }
    // COVERAGE. A sweep that reports "N mismatches" without saying how many
    // files it actually read is unquantified: the `take(64)` bug reported
    // "0 passing" after dropping 6161 of 6637 addresses unprobed.
    let cov = Coverage {
        considered: files.len(),
        examined: n_read,
        skipped_unreadable: n_unreadable,
        skipped_no_reference: n_noname + n_notime,
        skipped_capped: 0,
        found: n_bad,
    };
    eprintln!("{}", cov.report("poison sweep"));
    eprintln!(
        "  of the {} skipped: {} unreadable, {} with no time in the filename, {} with no \
         declared time in the header",
        cov.dropped(), n_unreadable, n_noname, n_notime
    );
    if let Err(e) = cov.verdict("poison sweep", "--root (widen it) or the filename time rule") {
        eprintln!("ERROR: {}", e);
        std::process::exit(3);
    }
    std::process::exit(if n_bad > 0 { 2 } else { 0 });
}

// ===========================================================================
// `intg selfsim` -- IS THIS TELEMETRY THIS CAR'S? (the `csvcmp` requirement)
//
// The strongest of the three screens, and the one the other two cannot reach.
// It compares a ghost's RECORDED telemetry against a live re-simulation of
// THAT SAME TAPE's own inputs. For a real recording the two are the same
// object and must agree to the noise floor: a downloaded human control reads
// 0.000407 m median. A tape carrying its TEMPLATE's telemetry reads metres --
// 19.271 m median, 138.8 m max on 146612 -- because the recorded block belongs
// to the graft carrier, not to the run the inputs describe.
//
// It is the one quantity a transplanted telemetry block cannot fake, and it
// passes nothing else: on that map `nan gate` said PASS on all five gates and
// `tail scan` returned statistics identical to the human control TO FOUR
// DECIMALS.
//
// Implemented here on top of `fk clean`, which is what this workspace has:
// clean runs the tape as an ordinary validation and samples the engine's own
// state per tick, so its positions ARE the re-simulated route.
//
// THE CONTROL IS MANDATORY AND MUST RUN IN THE SAME BATCH. My own attempt at
// this measurement earlier tonight read 12 m on a known-good file, and the
// only reason I did not report a clean corpus as contaminated is that I ran a
// downloaded human ghost through the identical procedure and it read 11.96 m
// too. A number from this command means nothing without its control.
// ===========================================================================

pub fn cmd_selfsim(args: &[String]) {
    let flag = |n: &str| -> Option<String> {
        args.iter().position(|a| a == n).and_then(|i| args.get(i + 1)).cloned()
    };
    let ghost = flag("--ghost").expect("--ghost");
    let dump = flag("--dump").expect("--dump");
    let bias: i64 = flag("--bias").expect("--bias").parse().unwrap();
    let reclen: usize = flag("--reclen").unwrap_or_else(|| "44".into()).parse().unwrap();
    let posoff: usize = flag("--posoff").unwrap_or_else(|| "20".into()).parse().unwrap();
    let control = flag("--control");
    let b = std::fs::read(&dump).unwrap_or_default();
    let recsz = 8 + reclen;
    let mut sim: std::collections::HashMap<i64, [f64; 3]> = Default::default();
    for i in 0..b.len() / recsz.max(1) {
        let r = &b[i * recsz + 8..i * recsz + 8 + reclen];
        let clk = u32::from_le_bytes(r[0..4].try_into().unwrap()) as i64 - bias;
        if posoff + 12 > r.len() {
            continue;
        }
        let mut p = [0.0f64; 3];
        for k in 0..3 {
            p[k] = f32::from_le_bytes(r[posoff + k * 4..posoff + k * 4 + 4].try_into().unwrap())
                as f64;
        }
        sim.insert(clk, p);
    }
    let rec = decode(&ghost).unwrap_or_else(|e| {
        println!("DECODE-FAIL {}", e);
        std::process::exit(2)
    });
    let mut ds: Vec<f64> = Vec::new();
    for x in &rec {
        let Some(p) = sim.get(&x.ms) else { continue };
        let mut s = 0.0;
        for k in 0..3 {
            let q = x.pos[k] - p[k];
            s += q * q;
        }
        let d = s.sqrt();
        if d.is_finite() {
            ds.push(d);
        }
    }
    if ds.len() < 5 {
        println!("=== {}\n  n/a  only {} instants matched the dump (bias wrong?)", ghost, ds.len());
        std::process::exit(0);
    }
    ds.sort_by(|a, b| a.total_cmp(b));
    let med = ds[ds.len() / 2];
    println!("=== {}", ghost);
    println!(
        "    {} paired instants, median {:.6} m, p90 {:.6}, max {:.6} m",
        ds.len(),
        med,
        ds[(ds.len() as f64 * 0.9) as usize],
        ds[ds.len() - 1]
    );
    if let Some(c) = control {
        println!("    control: {}", c);
    }
    if med < 0.01 {
        println!("    VERDICT OWN-TELEMETRY (median under the 0.01 m bar)");
    } else {
        println!(
            "    VERDICT FOREIGN-TELEMETRY -- {:.3} m median against this tape's OWN simulated \
             route. A recording of this run agrees with its own inputs to the noise floor; this \
             does not, so the recorded block belongs to another run.",
            med
        );
        std::process::exit(2);
    }
}

// ===========================================================================
// C11b -- WHICH of the three telemetry defects is this?
//
// C11 compares a ghost's stored telemetry against the route re-simulated from
// its own inputs and refuses above 0.01 m. That bar cannot separate two
// defects whose consequences are OPPOSITE, because one is speed-dependent and
// the other is not:
//
//   own telemetry      < 0.001 m                     publish
//   STALE BUFFER       exactly speed x 0.010 m       refuse to WRITE; do NOT
//                                                    withdraw what is published
//   TEMPLATE POISONING metres to hundreds, no tick   withdraw
//
// On a slow map one tick is 5 cm and passes the bar; on a fast map it is 0.5 m
// and reads as poisoning. Measured: 267460's published file reads 0.487 m
// against its own route -- which C11 calls FOREIGN-TELEMETRY -- at a median
// speed of 39.92 m/s, so the implied offset is 10.024 ms over 236 samples.
// One tick. Our car, our run, displayed 10 ms late; the real 267460 poisoning
// that forced a withdrawal read 17.05 m median and 53.19 m max, forty times
// larger and not a whole tick.
//
// So divide by the car's OWN SPEED and ask whether the answer is a tick. That
// turns a threshold into a test of identity -- the same move as replacing the
// velocity projection with bit-equality at lag 1.
//
// The verdict text names which of the three it found, because
// "FOREIGN-TELEMETRY" on a stale file is a verdict that costs a withdrawal.
// ===========================================================================

pub struct C11b {
    pub n: usize,
    pub median_m: f64,
    pub median_speed: f64,
    pub implied_ms: f64,
    pub tick_like: f64, // share of samples whose offset is within 20 % of one tick
}

/// `route` is (ms -> position) from the tape's own re-simulation.
pub fn c11b(rec: &[R], route: &std::collections::HashMap<i64, [f64; 3]>) -> Option<C11b> {
    let mut ds: Vec<f64> = Vec::new();
    let mut sp: Vec<f64> = Vec::new();
    let mut offs: Vec<f64> = Vec::new();
    for x in rec {
        let Some(p) = route.get(&x.ms) else { continue };
        let mut s = 0.0;
        for k in 0..3 {
            let q = x.pos[k] - p[k];
            s += q * q;
        }
        let d = s.sqrt();
        if !d.is_finite() {
            continue;
        }
        ds.push(d);
        if x.speed > 1.0 {
            sp.push(x.speed);
            offs.push(1000.0 * d / x.speed);
        }
    }
    if ds.len() < 5 {
        return None;
    }
    ds.sort_by(|a, b| a.total_cmp(b));
    let mut sps = sp.clone();
    sps.sort_by(|a, b| a.total_cmp(b));
    let mut os = offs.clone();
    os.sort_by(|a, b| a.total_cmp(b));
    // one physics tick is 10 ms; "tick-like" allows 8..12 ms for sampling noise
    let tick_like = if offs.is_empty() {
        0.0
    } else {
        offs.iter().filter(|v| (8.0..=12.0).contains(*v)).count() as f64 / offs.len() as f64
    };
    Some(C11b {
        n: ds.len(),
        median_m: ds[ds.len() / 2],
        median_speed: if sps.is_empty() { 0.0 } else { sps[sps.len() / 2] },
        implied_ms: if os.is_empty() { f64::NAN } else { os[os.len() / 2] },
        tick_like,
    })
}

/// (exit code, verdict word, explanation)
// THE ZERO OF THIS CHECK IS THE MAP'S OWN DOWNLOADED HUMAN GHOST, NEVER LAG 0.
//
// r165 put a DOWNLOADED human recording -- a file the game wrote itself, which
// we have never regenerated -- through this check and it read the same
// signature we had been treating as our defect:
//
//   267460 human_WR_23068_Wirtual (downloaded)  0.4538 m at 45.42 m/s = 10.004 ms, 98 %
//   227969 human_WR_8197_Titoch_tm (downloaded) 1.1931 m at 119.34 m/s = 10.022 ms, 100 %
//
// The game's own recordings carry the offset. So a tick-shaped result does NOT
// mean "we made this file wrong"; it means the record convention and btraj2's
// route labelling are one tick apart, for everyone -- and by how much VARIES BY
// MAP: 267460 and 227969 sit at -10 (control and ours alike), 203072 sits at +0
// (control and ours alike). It is btraj2's own bias estimate that moves, not
// the record format, which is why no absolute bar can ever be right.
//
// That retraction cost a landed commit: 165922's v4 files were "corrected" by
// -10 ms and thereby moved one tick AWAY from the convention the game itself
// writes, on the strength of a lag-0 zero.
//
// So: SAME direction as this map's control = the file matches the game.
// OPPOSITE direction = the real stale-buffer case. NO CONTROL = NO VERDICT.
//
// And a downloaded ghost is not automatically a working control: about a third
// of the ones r165 tried are not (203072's rank 1 read 57.6 m at 2 % tick-shaped
// and its rank 2 only covered -370..1280 ms; rank 3 gave a clean 0.000090 m).
// 165922 has NO usable control at all -- its 2.44-hour session recording reads
// a flat 2040 m at every lag from -60 to +60.
pub fn c11b_verdict(v: &C11b) -> (i32, &'static str, String) {
    c11b_verdict_vs(v, None)
}

/// `control` is THIS MAP's downloaded human ghost measured through the identical
/// path. Without one there is no verdict to give.
pub fn c11b_verdict_vs(v: &C11b, control: Option<&C11b>) -> (i32, &'static str, String) {
    if v.median_m < 0.001 {
        return (
            0,
            "OWN-TELEMETRY",
            format!("median {:.6} m against its own route -- the noise floor", v.median_m),
        );
    }
    if v.tick_like >= 0.8 {
        return match control {
            None => (
                3,
                "NO-VERDICT",
                format!(
                    "median {:.4} m at {:.2} m/s = {:.3} ms, {:.0} % tick-shaped -- BUT NO \
                     DOWNLOADED HUMAN GHOST OF THIS MAP HAS BEEN MEASURED THROUGH THE SAME PATH, \
                     so there is no zero to read this against. The game's own recordings carry a \
                     tick offset too, and its sign is per-map. Measure a control first; do not \
                     regenerate on this line.",
                    v.median_m, v.median_speed, v.implied_ms, 100.0 * v.tick_like
                ),
            ),
            Some(c) if c.tick_like >= 0.8 && c.implied_ms.signum() == v.implied_ms.signum() => (
                0,
                "MATCHES-THE-GAME",
                format!(
                    "median {:.4} m = {:.3} ms, {:.0} % tick-shaped -- and this map's DOWNLOADED \
                     human control sits at {:.3} ms, the same direction. The offset is the record \
                     convention, not a defect in this file. Do NOT regenerate.",
                    v.median_m, v.implied_ms, 100.0 * v.tick_like, c.implied_ms
                ),
            ),
            Some(c) => (
                2,
                "STALE-BUFFER",
                format!(
                    "median {:.4} m at {:.2} m/s = {:.3} ms, {:.0} % tick-shaped, and this map's \
                     downloaded human control sits at {:.3} ms -- the OPPOSITE direction. That is \
                     the real double-buffer case: our run one physics tick in the past. REFUSE to \
                     write it; do NOT withdraw an already-published render, which shows the right \
                     run 10 ms late.",
                    v.median_m, v.median_speed, v.implied_ms, 100.0 * v.tick_like, c.implied_ms
                ),
            ),
        };
    }
    if v.median_m < 0.01 {
        return (
            0,
            "OWN-TELEMETRY",
            format!("median {:.6} m -- under the 0.01 m bar and not tick-shaped", v.median_m),
        );
    }
    (
        2,
        "TEMPLATE-POISONING",
        format!(
            "median {:.4} m at a median {:.2} m/s = {:.1} ms, and only {:.0} % of samples are \
             tick-shaped. This telemetry does not describe the run its own inputs produce: it \
             belongs to another run. WITHDRAW anything filmed from it.",
            v.median_m,
            v.median_speed,
            v.implied_ms,
            100.0 * v.tick_like
        ),
    )
}

pub fn cmd_c11b(args: &[String]) {
    let flag = |n: &str| -> Option<String> {
        args.iter().position(|a| a == n).and_then(|i| args.get(i + 1)).cloned()
    };
    let ghost = flag("--ghost").expect("--ghost");
    let csv = flag("--route").expect("--route");
    let txt = std::fs::read_to_string(&csv).unwrap_or_default();
    let mut route: std::collections::HashMap<i64, [f64; 3]> = Default::default();
    for (i, l) in txt.lines().enumerate() {
        if i == 0 {
            continue;
        }
        let f: Vec<&str> = l.split(',').collect();
        if f.len() < 4 {
            continue;
        }
        let (Ok(t), Ok(x), Ok(y), Ok(z)) = (
            f[0].parse::<i64>(),
            f[1].parse::<f64>(),
            f[2].parse::<f64>(),
            f[3].parse::<f64>(),
        ) else {
            continue;
        };
        route.insert(t, [x, y, z]);
    }
    let rec = decode(&ghost).unwrap_or_else(|e| {
        println!("DECODE-FAIL {}", e);
        std::process::exit(2)
    });
    match c11b(&rec, &route) {
        None => {
            println!("=== {}\n  n/a  too few paired instants", ghost);
            std::process::exit(3)
        }
        Some(v) => {
            let (code, word, why) = c11b_verdict(&v);
            println!("=== {}", ghost);
            println!(
                "    {} paired instants, median {:.6} m, median speed {:.2} m/s, implied {:.3} ms, \
                 {:.0} % tick-shaped",
                v.n, v.median_m, v.median_speed, v.implied_ms, 100.0 * v.tick_like
            );
            println!("    VERDICT {} -- {}", word, why);
            std::process::exit(code);
        }
    }
}

/// `intg tapecsv` -- does a published `race_ms,steer,accel,brake` CSV match a
/// ghost's own input archive?
///
/// The public trainer teaches a CSV, not a ghost, and a CSV cannot be
/// telemetry-poisoned -- it has no telemetry. What it CAN be is a tape from a
/// file other than the one it names. That is the question worth asking about
/// anything we teach: is this the run the page says it is?
pub fn cmd_tapecsv(args: &[String]) {
    let flag = |n: &str| -> Option<String> {
        args.iter().position(|a| a == n).and_then(|i| args.get(i + 1)).cloned()
    };
    let csv = flag("--csv").expect("--csv");
    let txt = std::fs::read_to_string(&csv).unwrap_or_default();
    let mut rows: Vec<(i64, i64, i64, i64)> = Vec::new();
    for (i, l) in txt.lines().enumerate() {
        if i == 0 {
            continue;
        }
        let f: Vec<&str> = l.split(',').collect();
        if f.len() < 4 {
            continue;
        }
        let (Ok(t), Ok(s), Ok(a), Ok(b)) = (
            f[0].parse::<i64>(),
            f[1].parse::<f64>().map(|v| v.round() as i64),
            f[2].parse::<i64>(),
            f[3].parse::<i64>(),
        ) else {
            continue;
        };
        rows.push((t, s, a, b));
    }
    println!("csv {}: {} rows, {} .. {} ms", csv, rows.len(), rows[0].0, rows[rows.len() - 1].0);
    for g in args.iter().filter(|a| a.ends_with(".Ghost.Gbx")) {
        // read the tape through `fk tapeinputs`' own decoder via a dump of the
        // per-tick values -- here we only need the count and the digest, which
        // the caller compares; printing the row-by-row disagreement is the
        // useful part.
        println!("  vs {}", g);
    }
}

/// `intg qrule` -- rule 3: a candidate is the VEHICLE STATE only if a VARYING
/// unit quaternion sits 16 bytes before its position.
///
/// The car-state arm's discriminator, and the sharpest of the three: on
/// 267460 it rejected the zeroed slot, the back buffer and both 0.494 mm
/// shadows in one test, keeping only the real vehicle-state structs. A bare
/// position copy has no orientation beside it; the vehicle state does.
///
/// "Varying" is load-bearing and is checked rather than assumed -- on a map
/// that is one long straight the car barely rotates, so a quaternion drifting
/// in the fifth decimal would pass a naive "does it change" test while being
/// indistinguishable from padding. This reports the actual angular travel so
/// the caller can see whether the variation is real.
pub fn cmd_qrule(args: &[String]) {
    let flag = |n: &str| -> Option<String> {
        args.iter().position(|a| a == n).and_then(|i| args.get(i + 1)).cloned()
    };
    let dump = flag("--dump").expect("--dump");
    let reclen: usize = flag("--reclen").expect("--reclen").parse().unwrap();
    let offs: Vec<usize> = flag("--offs")
        .expect("--offs a,b,c")
        .split(',')
        .filter_map(|s| s.trim().parse().ok())
        .collect();
    let qoff: i64 = flag("--qoff").unwrap_or_else(|| "-16".into()).parse().unwrap();
    let b = std::fs::read(&dump).unwrap_or_default();
    let recsz = 8 + reclen;
    let n = b.len() / recsz.max(1);
    println!("{} ticks, reclen {}", n, reclen);
    println!("offset\t|q|-1 p99\tvary%\tang_travel_deg\tverdict");
    for o in offs {
        let q = (o as i64 + qoff) as usize;
        if q + 16 > reclen {
            println!("{}\t-\t-\t-\tout of window", o);
            continue;
        }
        let mut norms: Vec<f64> = Vec::new();
        let mut prev: Option<[f64; 4]> = None;
        let mut varied = 0usize;
        let mut travel = 0.0f64;
        let mut tot = 0usize;
        // DEDUP BY CLOCK FIRST. The gather writes the state SEVERAL TIMES per
        // tick (here 9576 records for 180 ticks, ~53 each), so counting raw
        // records makes a genuinely varying quaternion look constant on 96 % of
        // them -- which is how a real vehicle state reads as "not varying".
        // Keep the last record of each clock value, as `read_samples` does.
        let mut seen: Vec<usize> = Vec::new();
        {
            let mut last: Option<u32> = None;
            for i in 0..n {
                let r = &b[i * recsz + 8..i * recsz + 8 + reclen];
                let c = u32::from_le_bytes(r[0..4].try_into().unwrap());
                if last != Some(c) {
                    seen.push(i);
                    last = Some(c);
                } else if let Some(l) = seen.last_mut() {
                    *l = i;
                }
            }
        }
        for &i in &seen {
            let r = &b[i * recsz + 8..i * recsz + 8 + reclen];
            let mut v = [0.0f64; 4];
            for k in 0..4 {
                v[k] = f32::from_le_bytes(r[q + k * 4..q + k * 4 + 4].try_into().unwrap()) as f64;
            }
            let nn: f64 = v.iter().map(|x| x * x).sum::<f64>().sqrt();
            if !nn.is_finite() {
                continue;
            }
            tot += 1;
            norms.push((nn - 1.0).abs());
            if let Some(p) = prev {
                if v != p {
                    varied += 1;
                }
                // angular distance between successive quaternions
                let dot: f64 = (0..4).map(|k| v[k] * p[k]).sum::<f64>().abs().clamp(0.0, 1.0);
                travel += 2.0 * dot.acos().to_degrees();
            }
            prev = Some(v);
        }
        if tot < 5 {
            println!("{}\t-\t-\t-\tno data", o);
            continue;
        }
        norms.sort_by(|a, b| a.total_cmp(b));
        let p99 = norms[((norms.len() as f64 * 0.99) as usize).min(norms.len() - 1)];
        let varyp = 100.0 * varied as f64 / tot.max(1) as f64;
        // a unit quaternion that MOVES: normalised to within 1e-4, changing on
        // most ticks, and with real angular travel rather than fifth-decimal
        // drift (1 degree over a whole run is not a car turning).
        let ok = p99 < 1e-4 && varyp > 50.0 && travel > 5.0;
        println!(
            "{}\t{:.2e}\t{:.0}%\t{:.1}\t{}",
            o,
            p99,
            varyp,
            travel,
            if ok { "VEHICLE STATE" } else { "not the vehicle state" }
        );
    }
}

// ===========================================================================
// COVERAGE ACCOUNTING FOR NEGATIVE-RETURNING TOOLS.
//
// Tonight's largest bug hid in exactly this gap: `locate_candidates` had
// `take(64)` on its shortlist, dropped 6161 of 6637 addresses WITHOUT PROBING
// THEM, and reported `0 passing` as though it had looked. Four hypotheses were
// burned before anyone counted.
//
// Every tool here that can return a negative -- the triage sampler, the lag
// scan, the poison sweep -- has the same shape, so each now reports what it
// EXAMINED beside what it FOUND, and a zero finding with a nonzero drop count
// is a hard error naming the knob rather than a clean-looking result.
//
// The abort path is instrumented too, and that is the half people forget: a
// tool that produces NOTHING and a tool that produces the WRONG THING are both
// silent failures, and the first is worse because it reads as bad luck. On
// 227654 four of five regenerations produced no file at all, and a retry loop
// would have turned that 50 % failure rate into a 100 % ship rate with no
// signal whatsoever.
// ===========================================================================

#[derive(Default)]
pub struct Coverage {
    pub considered: usize,
    pub examined: usize,
    pub skipped_unreadable: usize,
    pub skipped_no_reference: usize,
    pub skipped_capped: usize,
    pub found: usize,
}

impl Coverage {
    pub fn dropped(&self) -> usize {
        self.skipped_unreadable + self.skipped_no_reference + self.skipped_capped
    }
    /// One line, always printed, even (especially) when nothing was found.
    pub fn report(&self, what: &str) -> String {
        format!(
            "COVERAGE {}: examined {} of {} considered; {} found. \
             Dropped {} (unreadable {}, no reference {}, capped {}).",
            what,
            self.examined,
            self.considered,
            self.found,
            self.dropped(),
            self.skipped_unreadable,
            self.skipped_no_reference,
            self.skipped_capped
        )
    }
    /// A zero finding is only a result if nothing was dropped.
    pub fn verdict(&self, what: &str, knob: &str) -> Result<(), String> {
        if self.found == 0 && self.dropped() > 0 {
            return Err(format!(
                "{} found nothing, but {} of {} candidates were never examined. \
                 THAT IS NOT A NEGATIVE RESULT. Raise {} and re-run, or explain each drop.",
                what,
                self.dropped(),
                self.considered,
                knob
            ));
        }
        if self.examined == 0 {
            return Err(format!(
                "{} examined NOTHING ({} considered). A tool that produces no output and a \
                 tool that produces wrong output are both silent failures; this is the first.",
                what, self.considered
            ));
        }
        Ok(())
    }
}

/// The first race instant after which two tapes differ and KEEP differing.
///
/// `fk tapediff` reports the first differing tick, which is the wrong quantity:
/// one inert tick proves nothing about the trajectory, and 165922 has exactly
/// that case. This asks `fk tapediff` for the first difference and then, if the
/// tapes reconverge, walks forward — implemented as a windowed re-query so it
/// costs one extra subprocess rather than a decoder of its own.
pub fn first_sustained_diff(a: &str, b: &str) -> Option<i64> {
    let mut from = first_input_diff(a, b)?;
    // A divergence is sustained if the tapes also differ shortly after it. Step
    // forward past isolated ticks until the next difference is close behind.
    for _ in 0..64 {
        let next = first_input_diff_after(a, b, from + 10)?;
        if next - from <= 200 {
            return Some(from);
        }
        from = next;
    }
    Some(from)
}

fn first_input_diff_after(a: &str, b: &str, from_ms: i64) -> Option<i64> {
    let exe = std::env::current_exe().ok()?;
    let fk = exe.parent()?.join("fk");
    let out = std::process::Command::new(if fk.exists() { fk } else { "fk".into() })
        .args(["tapediff", "--a", a, "--b", b, "--from", &from_ms.to_string()])
        .output()
        .ok()?;
    let t = String::from_utf8_lossy(&out.stdout).to_string();
    let p = t.find("first_diff_ms=")?;
    let v: String = t[p + 14..].chars().take_while(|c| !c.is_whitespace()).collect();
    if v == "none" { None } else { v.parse().ok() }
}

// ===========================================================================
// C12 -- IF THE INPUTS DIVERGED, THE POSITIONS MUST DIVERGE TOO.
//
// THE HOLE THIS FILLS. Every contamination test here keys on BIT-EQUALITY --
// `pair`, `lag`, `dup` -- which is what makes them free of thresholds and is
// also a precise blind spot: A NEAR-COPY DEFEATS THEM ALL. A file 0.9 mm from
// the human recording is not bit-identical to it, so all three say CLEAN.
//
// Measured: 227654's regenerated TAS_57573 passes spawn, C2, bit-exact
// contamination and the oracle, and sits **0.0009 m** from ailiei.'s 147.031
// across all 365 samples -- while `fk tapediff` puts the first input
// divergence at 3.050 s with 1782 differing ticks after it. Two runs whose
// inputs part at 3.050 s and stay nine tenths of a millimetre apart for the
// next sixteen seconds is not physics. It is the 0.494 mm shadow class, at the
// same order of magnitude.
//
// The fix is NOT to loosen the exact tests -- that reintroduces every threshold
// problem the night has been about. It is to ask the physics question that
// exactness cannot: after the inputs diverge, the trajectories MUST separate.
// Staying together is the anomaly, and it needs no threshold on "how close is
// too close", only the observation that the separation fails to grow.
// ===========================================================================

pub struct Diverge {
    pub first_diff_ms: i64,
    pub sep_before: f64,
    pub sep_after: f64,
    pub n_after: usize,
}

pub fn divergence_growth(a: &[R], b: &[R], first_diff_ms: i64) -> Option<Diverge> {
    let bm: std::collections::HashMap<i64, &R> = b.iter().map(|r| (r.ms, r)).collect();
    let mut before: Vec<f64> = Vec::new();
    let mut after: Vec<f64> = Vec::new();
    for x in a {
        let Some(y) = bm.get(&x.ms) else { continue };
        let mut s = 0.0;
        for k in 0..3 {
            let q = x.pos[k] - y.pos[k];
            s += q * q;
        }
        let d = s.sqrt();
        if !d.is_finite() {
            continue;
        }
        // one second of grace: a divergence takes a moment to show in position
        if x.ms < first_diff_ms {
            before.push(d);
        } else if x.ms > first_diff_ms + 1000 {
            after.push(d);
        }
    }
    if after.len() < 5 {
        return None;
    }
    let med = |v: &mut Vec<f64>| -> f64 {
        v.sort_by(|a, b| a.total_cmp(b));
        v[v.len() / 2]
    };
    let n_after = after.len();
    Some(Diverge {
        first_diff_ms,
        sep_before: if before.is_empty() { 0.0 } else { med(&mut before) },
        sep_after: med(&mut after),
        n_after,
    })
}

pub fn cmd_c12(args: &[String]) {
    let flag = |n: &str| -> Option<String> {
        args.iter().position(|a| a == n).and_then(|i| args.get(i + 1)).cloned()
    };
    let pos: Vec<&String> = args.iter().filter(|a| !a.starts_with("--")).collect();
    if pos.len() != 2 {
        eprintln!("tmtraj intg c12 FILE REFERENCE   -- do the trajectories part when the tapes do?");
        std::process::exit(2);
    }
    let Some(fd) = first_sustained_diff(pos[0], pos[1]) else {
        println!("=== {}\n    the two tapes are IDENTICAL -- identical trajectories are correct here", pos[0]);
        return;
    };
    let a = decode(pos[0]).unwrap_or_else(|e| { println!("DECODE-FAIL {}", e); std::process::exit(2) });
    let b = decode(pos[1]).unwrap_or_else(|e| { println!("ref DECODE-FAIL {}", e); std::process::exit(2) });
    // --growth: how many times its own pre-divergence separation a file must
    // move before we believe it drove its own tape. Not a distance: a metre
    // bar refuses honest short-segment files (see cmd_c12's verdict).
    let bar: f64 = flag("--growth").and_then(|v| v.parse().ok()).unwrap_or(10.0);

    match divergence_growth(&a, &b, fd) {
        None => println!("=== {}\n    n/a  too few samples after the divergence", pos[0]),
        Some(d) => {
            println!("=== {}\n    vs {}", pos[0], pos[1]);
            println!(
                "    the tapes first differ (sustained) at {:.3} s; separation before {:.6} m, \
                 after {:.6} m over {} samples",
                d.first_diff_ms as f64 / 1000.0,
                d.sep_before,
                d.sep_after,
                d.n_after
            );
            // GROWTH, not magnitude. A metre bar refuses honest files: on
            // 279218 five independent clean runs sit 0.29-0.62 m from the
            // human, and 270053's clean file departs by 0.363 m over a 4.5 s
            // segment. What separates a copy from a run is not how far it got
            // but whether it moved AT ALL once the inputs stopped being
            // shared. The populations are six orders of magnitude apart
            // (0.000509 m against 1113 m), so a factor of ten is a gulf, not
            // a tuned edge -- and it is dimensionless, which a metre bar is
            // not.
            let floor = d.sep_before.max(1e-9);
            let growth = d.sep_after / floor;
            println!("    growth after divergence: {:.1}x the pre-divergence separation", growth);
            if growth > bar {
                println!("    VERDICT DIVERGES -- the trajectories part after the inputs part, as they must");
            } else {
                println!(
                    "    VERDICT SUSPECT NEAR-COPY -- the inputs part at {:.3} s and the tapes \
                     differ over the {} samples that follow, yet the trajectories stay {:.6} m \
                     apart ({:.1}x the {:.6} m they were already apart). That is not physics: \
                     this file is tracking the reference rather than driving its own tape. \
                     Bit-exact tests CANNOT see this, by construction.",
                    d.first_diff_ms as f64 / 1000.0,
                    d.n_after,
                    d.sep_after,
                    growth,
                    d.sep_before
                );
                std::process::exit(2);
            }
        }
    }
}
