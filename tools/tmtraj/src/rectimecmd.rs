//! `tmtraj rectime` -- shift the RECORD's sample instants, leaving every
//! sample's bytes untouched.
//!
//! WHY (arm `r165`, 2026-08-21)
//! ---------------------------
//! `fk regen` fills the record's sample at instant `t` from the engine state it
//! read at engine-clock `t + bias`, and `fk btraj2` labels its own rows from a
//! SEPARATELY measured bias. When the two biases differ by a tick, the
//! regenerated file is a whole physics tick out and C11b calls it STALE-BUFFER
//! -- our run, our car, ten milliseconds late. On 165922 every file was.
//!
//! `fk regen --recshift` fixes it at the source, but regeneration is expensive
//! (a live engine, a locate, and a copy chooser that lands on a garbage slot
//! about one time in ten). If the SAMPLES are right and only their LABELS are
//! late, the repair is arithmetic:
//!
//!     regen at recshift -10 puts the state of race t+10 at instant t
//!     regen at recshift   0 puts the state of race t    at instant t
//!     => shifted(t) == unshifted(t + 10), i.e. shift the times by -10
//!
//! So this command must reproduce a `--recshift -10` regeneration from a
//! `--recshift 0` one exactly, and that equality is the control it ships with:
//! run it, then `cmp` the sample block against the regenerated file. If they
//! disagree, this tool is wrong and the regeneration is the only repair.
//!
//! It shifts EVERY entity, because they are all labelled off the same clock,
//! and it drops any sample the shift would move outside `[0, end]` rather than
//! let a sample claim an instant the run does not contain.

use crate::recwrite::rewrite_ghost;

pub fn cmd(args: &[String]) {
    let flag = |n: &str| -> Option<String> {
        args.iter().position(|a| a == n).and_then(|i| args.get(i + 1)).cloned()
    };
    let inp = flag("--in").expect("--in GHOST.Ghost.Gbx");
    let out = flag("--out").expect("--out OUT.Ghost.Gbx");
    let shift: i32 = flag("--shift").expect("--shift MS").parse().expect("--shift MS");
    let keep_span = args.iter().any(|a| a == "--keep-span");
    let mut report: Vec<String> = Vec::new();
    let r = rewrite_ghost(&inp, &out, |rd| {
        let lo = 0i32;
        let hi = rd.end_ms;
        for e in rd.ents.iter_mut() {
            if e.times.is_empty() {
                continue;
            }
            let ss = e.sample_size;
            let before = e.times.len();
            let mut nt: Vec<i32> = Vec::with_capacity(before);
            let mut nr: Vec<u8> = Vec::with_capacity(before * ss);
            for i in 0..e.times.len() {
                let t = e.times[i] + shift;
                if t < lo || (hi > 0 && t > hi) {
                    continue;
                }
                nt.push(t);
                if ss > 0 {
                    nr.extend_from_slice(&e.raw[i * ss..(i + 1) * ss]);
                }
            }
            report.push(format!(
                "  entity type {}: {} -> {} samples, {} .. {} ms",
                e.type_,
                before,
                nt.len(),
                nt.first().copied().unwrap_or(0),
                nt.last().copied().unwrap_or(0)
            ));
            e.times = nt;
            e.raw = nr;
        }
        if !keep_span {
            rd.start_ms = (rd.start_ms + shift).max(0);
        }
        Ok(())
    });
    match r {
        Ok((a, b)) => {
            for l in &report {
                println!("{}", l);
            }
            println!(
                "rectime: {} -> {}: sample instants shifted {:+} ms, record {} -> {} B",
                inp, out, shift, a, b
            );
        }
        Err(e) => {
            println!("ABORT: {}", e);
            std::process::exit(3);
        }
    }
}

/// `tmtraj rectime cmp A B` -- are two ghosts' vehicle records the same run?
///
/// Prints, per shared instant, whether the 116 sample bytes are identical. This
/// is the control for the shift above: a shifted file and an independently
/// regenerated one must agree BYTE FOR BYTE on every instant they share, not
/// merely to some distance.
pub fn cmd_cmp(args: &[String]) -> i32 {
    let files: Vec<String> =
        args.iter().filter(|a| a.ends_with(".Ghost.Gbx")).cloned().collect();
    if files.len() != 2 {
        eprintln!("usage: tmtraj rectime cmp A.Ghost.Gbx B.Ghost.Gbx");
        return 2;
    }
    let load = |p: &str| -> Option<Vec<(i32, Vec<u8>)>> {
        let body = crate::entrec::load_body(p).ok()?;
        let (version, blob) = crate::entrec::find_entrecord_blob(&body).ok()?;
        let rd = crate::entrec::parse_record_data(&blob, version).ok()?;
        let e = rd
            .ents
            .iter()
            .filter(|e| e.sample_size >= 100 && !e.times.is_empty())
            .max_by_key(|e| e.times.len())?;
        let ss = e.sample_size;
        Some(
            (0..e.times.len())
                .map(|i| (e.times[i], e.raw[i * ss..(i + 1) * ss].to_vec()))
                .collect(),
        )
    };
    let (Some(a), Some(b)) = (load(&files[0]), load(&files[1])) else {
        println!("DECODE-FAIL");
        return 3;
    };
    let bm: std::collections::HashMap<i32, &Vec<u8>> = b.iter().map(|(t, v)| (*t, v)).collect();
    let (mut shared, mut same) = (0usize, 0usize);
    let mut first_diff: Option<(i32, usize)> = None;
    for (t, va) in &a {
        let Some(vb) = bm.get(t) else { continue };
        shared += 1;
        if va == *vb {
            same += 1;
        } else if first_diff.is_none() {
            let k = va.iter().zip(vb.iter()).position(|(x, y)| x != y).unwrap_or(0);
            first_diff = Some((*t, k));
        }
    }
    println!(
        "{} ({} samples) vs {} ({} samples): {} shared instants, {} BYTE-IDENTICAL",
        files[0].rsplit('/').next().unwrap_or(&files[0]),
        a.len(),
        files[1].rsplit('/').next().unwrap_or(&files[1]),
        b.len(),
        shared,
        same
    );
    if let Some((t, k)) = first_diff {
        println!("  first difference at {} ms, sample byte {}", t, k);
    }
    if shared == 0 {
        println!("  VERDICT NO-OVERLAP");
        return 3;
    }
    if same == shared {
        println!("  VERDICT IDENTICAL-RECORDS over every shared instant");
        0
    } else {
        println!("  VERDICT RECORDS-DIFFER ({} of {} shared instants)", shared - same, shared);
        2
    }
}

/// `tmtraj rectime lag --ghost G --route R.csv` -- the SIGNED offset between a
/// ghost's stored record and a re-simulated route.
///
/// WHY THIS EXISTS, and it is the most important thing in this file
/// ---------------------------------------------------------------
/// C11b reports `1000 * distance / speed`, which is a MAGNITUDE. Two files can
/// both read "10.000 ms, 100 % tick-shaped" while being a tick apart in
/// OPPOSITE directions, and no amount of staring at that number separates them.
///
/// That is not hypothetical. On 267460 a DOWNLOADED human ghost -- recorded by
/// the game itself, never regenerated by us -- reads 0.4538 m at 45.42 m/s =
/// 10.004 ms, 98 % tick-shaped: the identical signature our regenerated files
/// show. So "STALE-BUFFER" cannot mean "we made this file wrong" until the sign
/// is known, because the game's own recordings carry the same offset against
/// `fk btraj2`'s labelling.
///
/// This prints the median distance at every integer tick of lag, so the
/// question becomes which lag lands at the noise floor and in which direction.
/// The reference for what is CORRECT is then the game's own recording, not our
/// instrument: whatever lag a downloaded human ghost sits at is the convention
/// the format uses, and a regenerated file must sit at the same one.
pub fn cmd_lag(args: &[String]) -> i32 {
    let flag = |n: &str| -> Option<String> {
        args.iter().position(|a| a == n).and_then(|i| args.get(i + 1)).cloned()
    };
    let ghost = flag("--ghost").expect("--ghost G.Ghost.Gbx");
    let route = flag("--route").expect("--route R.csv");
    let span: i64 = flag("--span").and_then(|v| v.parse().ok()).unwrap_or(3);

    let txt = std::fs::read_to_string(&route).unwrap_or_default();
    let mut rp: std::collections::HashMap<i64, [f64; 3]> = Default::default();
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
        rp.insert(t, [x, y, z]);
    }
    let Ok(d) = crate::entrec::decode_ghost(&ghost) else {
        println!("DECODE-FAIL {}", ghost);
        return 3;
    };
    println!("=== {}", ghost.rsplit('/').next().unwrap_or(&ghost));
    println!("  lag_ms   paired   median_m");
    let mut best = (f64::INFINITY, 0i64, 0usize);
    for k in -span..=span {
        let lag = k * 10;
        let mut ds: Vec<f64> = Vec::new();
        for s in &d.samples {
            let Some(p) = rp.get(&(s.time_ms as i64 + lag)) else { continue };
            let mut q = 0.0;
            for (a, b) in [(s.x, p[0]), (s.y, p[1]), (s.z, p[2])] {
                let e = a - b;
                q += e * e;
            }
            let v = q.sqrt();
            if v.is_finite() {
                ds.push(v);
            }
        }
        if ds.len() < 5 {
            println!("  {:+6}   {:>6}   (too few)", lag, ds.len());
            continue;
        }
        ds.sort_by(|a, b| a.total_cmp(b));
        let m = ds[ds.len() / 2];
        let mark = if m < 0.01 { "   <- noise floor" } else { "" };
        println!("  {:+6}   {:>6}   {:.6}{}", lag, ds.len(), m, mark);
        if m < best.0 {
            best = (m, lag, ds.len());
        }
    }
    println!(
        "  BEST lag {:+} ms at {:.6} m over {} instants",
        best.1, best.0, best.2
    );
    println!(
        "  read this as: the record's sample at instant t holds the engine state at t{:+} ms,\n  \
         as `fk btraj2` labels race time. Compare against a DOWNLOADED human ghost of the same\n  \
         map put through this same command -- that is the convention the game itself writes.",
        best.1
    );
    0
}
