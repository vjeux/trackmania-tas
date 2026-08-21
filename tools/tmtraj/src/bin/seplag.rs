// seplag -- compare two recordings with NO time alignment, over every lag.
//
//   seplag A.Ghost.Gbx B.Ghost.Gbx [--max-lag N]
//
// WHY THIS EXISTS
//
// `sep` walks the two files index by index and BREAKS OUT when the recorded
// sample times differ, printing a note to stderr. Every pipeline I built
// discards stderr, so a pair whose time grids do not line up produced a short
// table -- or no table at all -- and the caller read that silence as "no
// contamination, no separation". CLEAN.
//
// That is not a hypothetical. In the published-ghost audit, all ten of
// 228607's files were compared against AUTHOR_LAP_20258 and produced ZERO
// rows: the grids diverge at the very first sample. Ten CLEAN verdicts, none
// of which rested on a single compared sample.
//
// Sample times are SESSION times, not race times, so two recordings made in
// different sessions share no instants at all and index alignment is
// meaningless. The honest comparison ignores the labels and asks: is there ANY
// integer offset at which these two describe the same car? A donor graft shows
// up as a run of exactly-zero distances at some lag; two genuinely different
// runs show none at any lag.
//
// This is the same instrument intg calls `intg lag`, written here because the
// audit that needs it is mine.
use std::env;
use tmtraj::entrec;

fn main() {
    let a: Vec<String> = env::args().skip(1).collect();
    if a.len() < 2 {
        eprintln!("usage: seplag A.Ghost.Gbx B.Ghost.Gbx [--max-lag N]");
        std::process::exit(2);
    }
    let max_lag: i64 = a
        .iter()
        .position(|x| x == "--max-lag")
        .and_then(|i| a.get(i + 1))
        .and_then(|v| v.parse().ok())
        .unwrap_or(0);

    let da = entrec::decode_ghost(&a[0]).expect("decode A");
    let db = entrec::decode_ghost(&a[1]).expect("decode B");
    let (sa, sb) = (&da.samples, &db.samples);
    if sa.is_empty() || sb.is_empty() {
        println!("EMPTY");
        return;
    }
    // default: scan every lag that gives at least a quarter of the shorter file
    let shorter = sa.len().min(sb.len()) as i64;
    let lim = if max_lag > 0 {
        max_lag
    } else {
        (sa.len().max(sb.len()) as i64) - shorter / 4
    };

    eprintln!(
        "A {} samples (t {}..{}), B {} samples (t {}..{}) -- scanning lags -{}..{}",
        sa.len(),
        sa[0].time_ms,
        sa[sa.len() - 1].time_ms,
        sb.len(),
        sb[0].time_ms,
        sb[sb.len() - 1].time_ms,
        lim,
        lim
    );

    // best lag = the one with the longest run of EXACTLY-zero distances; ties
    // broken by smallest mean distance. Exact zeros are the donor signature;
    // "close" is just two runs of the same map.
    let (mut best_lag, mut best_run, mut best_mean, mut best_n, mut best_zeros) = (0i64, 0usize, f64::MAX, 0usize, 0usize);
    for lag in -lim..=lim {
        let (mut n, mut sum, mut run, mut mrun, mut zeros) = (0usize, 0.0f64, 0usize, 0usize, 0usize);
        for i in 0..sa.len() {
            let j = i as i64 + lag;
            if j < 0 || j as usize >= sb.len() {
                continue;
            }
            let (p, q) = (&sa[i], &sb[j as usize]);
            let d = ((p.x - q.x).powi(2) + (p.y - q.y).powi(2) + (p.z - q.z).powi(2)).sqrt();
            if !d.is_finite() {
                continue;
            }
            n += 1;
            sum += d;
            if d == 0.0 {
                zeros += 1;
                run += 1;
                if run > mrun {
                    mrun = run;
                }
            } else {
                run = 0;
            }
        }
        if n < (shorter / 4) as usize {
            continue;
        }
        let mean = sum / n as f64;
        if mrun > best_run || (mrun == best_run && mean < best_mean) {
            best_lag = lag;
            best_run = mrun;
            best_mean = mean;
            best_n = n;
            best_zeros = zeros;
        }
    }
    println!(
        "best_lag={} overlap={} exact_zeros={} longest_identical_run={} mean={:.6}",
        best_lag, best_n, best_zeros, best_run, best_mean
    );
    if best_run >= 10 {
        println!("VERDICT DONOR-GRAFT: {} consecutive identical positions at lag {}", best_run, best_lag);
    } else if best_zeros > 0 {
        println!("VERDICT incidental: {} scattered identical positions, longest run {}", best_zeros, best_run);
    } else {
        println!("VERDICT INDEPENDENT: no identical position at any lag");
    }
}
