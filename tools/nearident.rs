// nearident -- is this ghost a COPY of that one, after a float re-encode?
//
//   nearident A.Gbx B.Gbx [--mm 1.0] [--run 100]
//
// WHY THIS EXISTS
// `seplag` asks whether two ghosts share positions that are EXACTLY equal. On
// 199100 it answered "INDEPENDENT: no identical position at any lag" for a pair
// that is one run:
//
//   input-tape md5   ours 47.483 == uelen.'s 47.838   (byte-identical)
//   t <  40.000 s    800 samples   mean 0.000476 m   max 0.000906 m
//   t >= 40.100 s    157 samples   mean 18.71 m      max 52.86 m
//
// Half a millimetre for 800 consecutive samples is a COPY that has been through
// a float re-encode -- and a re-encode never reproduces the bits, so an equality
// test is structurally blind to it.
//
// This is the `sep {:.2}` bug inverted. That one rounded real differences away
// and called distinct files identical; this one demanded exact equality and
// called one run two. BOTH FAILED TOWARD "CLEAN", which is the direction that
// publishes.
//
// So: a BAND, not an equality. Two ghosts are flagged when their positions stay
// within --mm for --run consecutive compared samples, scanned at every integer
// lag the way seplag does.
//
// CALIBRATE BEFORE TRUSTING. A band that flags everything is as useless as one
// that flags nothing. The defaults were set against known answers:
//   positive (must flag)  199100 51_TAS_47483_clean vs 91_HUMAN_uelen_47838
//   negative (must pass)  two of our own independent runs on the same map
// Re-run those two whenever the defaults change.
use std::env;
use tmtraj::entrec;

fn main() {
    let mut files: Vec<String> = Vec::new();
    let mut mm = 1.0_f64;
    let mut minrun = 100_usize;
    let mut args = env::args().skip(1);
    while let Some(a) = args.next() {
        match a.as_str() {
            "--mm" => mm = args.next().and_then(|v| v.parse().ok()).unwrap_or(1.0),
            "--run" => minrun = args.next().and_then(|v| v.parse().ok()).unwrap_or(100),
            _ => files.push(a),
        }
    }
    if files.len() != 2 {
        eprintln!("usage: nearident A.Gbx B.Gbx [--mm 1.0] [--run 100]");
        std::process::exit(2);
    }
    let a = match entrec::decode_ghost(&files[0]) { Ok(d) => d, Err(e) => { eprintln!("{}: {}", files[0], e); std::process::exit(2) } };
    let b = match entrec::decode_ghost(&files[1]) { Ok(d) => d, Err(e) => { eprintln!("{}: {}", files[1], e); std::process::exit(2) } };
    let tol = mm / 1000.0;

    // index B by sample time so we compare like with like, as seplag does:
    // sample times are session times, so index alignment across files is
    // meaningless and only the time key is sound.
    let bmap: std::collections::HashMap<i32, &entrec::Sample> = b.samples.iter().map(|s| (s.time_ms, s)).collect();

    let mut best = (0i32, 0usize, 0usize, f64::MAX); // lag, longest run, overlap, mean
    let span = 750i32;
    for lag in -span..=span {
        let (mut run, mut longest, mut overlap, mut sum) = (0usize, 0usize, 0usize, 0.0f64);
        for s in &a.samples {
            if let Some(t) = bmap.get(&(s.time_ms + lag * 50)) {
                overlap += 1;
                let d = ((s.x - t.x).powi(2) + (s.y - t.y).powi(2) + (s.z - t.z).powi(2)).sqrt();
                sum += d;
                if d <= tol { run += 1; if run > longest { longest = run } } else { run = 0 }
            }
        }
        if overlap > 0 && longest > best.1 {
            best = (lag, longest, overlap, sum / overlap as f64);
        }
    }
    let (lag, longest, overlap, mean) = best;
    println!("A {} samples, B {} samples -- band {:.3} mm, min run {}", a.samples.len(), b.samples.len(), mm, minrun);
    println!("best_lag={} overlap={} longest_near_identical_run={} mean={:.6} m", lag, overlap, longest, mean);
    if longest >= minrun {
        println!("VERDICT COPY: {} consecutive samples within {} mm at lag {} -- a re-encoded copy, not an independent run", longest, mm, lag);
        println!("  cross-check the input tapes: two runs driven separately do not share a sample-CSV md5");
        std::process::exit(1);
    }
    println!("VERDICT INDEPENDENT: longest near-identical run is {} samples, under the {}-sample bar", longest, minrun);
}
