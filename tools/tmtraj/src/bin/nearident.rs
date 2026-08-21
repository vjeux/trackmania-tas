// nearident -- is this ghost a COPY of that one, after a float re-encode?
//
//   nearident A.Gbx B.Gbx --control C1.Gbx C2.Gbx [--control ...] [--run 100]
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
// TWO DEFECTS FOUND IN THE FIRST VERSION, 2026-08-21. Both are fixed here and
// both were failures of the same kind: the tool answered a question it had not
// measured.
//
// 1. THE FIXED 1 mm BAND CANNOT SEPARATE A COPY FROM A CLOSE LINE, and it cried
//    COPY on four clips that were fine. 1 mm is TWICE our own writer's noise
//    floor: a ghost regenerated from the engine sits ~0.5 mm from the same run
//    recorded by the game client, measured repeatedly (0.482, 0.483, 0.489,
//    0.518 mm on four maps' answer keys). So on any ours-versus-a-download
//    pairing the band is comparing the copy hypothesis against nothing at all --
//    every sample of a genuinely shared PREFIX also sits inside it, and a shared
//    prefix is not a splice, it is determinism. 228811's tape and KappaRiley's
//    share their first 1509 input events; the cars are then bit-for-bit on the
//    same line for 13.5 s because the simulation is deterministic, and no
//    absolute band can tell that from a copy.
//
//    THE FIX IS A CONTROL, NOT A NUMBER. `--control X Y` names two recordings on
//    the same map that are known to be different runs -- human against human is
//    the pairing that works, since neither can have been produced by our
//    pipeline. The verdict is then a RATIO: how much closer the subject pair
//    sits than a pair we know is independent. That is dimensionless, it does not
//    care what the writer's noise floor is on this map, and it cannot be tuned.
//    With no control the tool REFUSES (exit 3). A verdict with no control is the
//    thing that cost four clips.
//
// 2. ZERO COMPARED ROWS READ AS A CLEAN PASS. Sample times are SESSION times, so
//    two recordings from different sessions can share no time key at all; the
//    loop then compared nothing, `overlap` stayed 0, `mean` stayed f64::MAX, and
//    the tool printed VERDICT INDEPENDENT. An empty denominator is not evidence.
//    It is now a hard error naming the cause.
use std::env;
use tmtraj::entrec;

/// The statistic, at the lag that maximises it: (lag, longest run inside the
/// band, overlap, mean separation over the overlap). `None` when the two files
/// share no sample instant at any lag -- which is a measurement failure, not a
/// result.
fn scan(a: &entrec::Decoded, b: &entrec::Decoded, tol: f64) -> Option<(i32, usize, usize, f64)> {
    let bmap: std::collections::HashMap<i32, &entrec::Sample> =
        b.samples.iter().map(|s| (s.time_ms, s)).collect();
    let mut best: Option<(i32, usize, usize, f64)> = None;
    let span = 750i32;
    for lag in -span..=span {
        let (mut run, mut longest, mut overlap, mut sum) = (0usize, 0usize, 0usize, 0.0f64);
        for s in &a.samples {
            if let Some(t) = bmap.get(&(s.time_ms + lag * 50)) {
                overlap += 1;
                let d = ((s.x - t.x).powi(2) + (s.y - t.y).powi(2) + (s.z - t.z).powi(2)).sqrt();
                sum += d;
                if d <= tol {
                    run += 1;
                    if run > longest {
                        longest = run
                    }
                } else {
                    run = 0
                }
            }
        }
        if overlap == 0 {
            continue;
        }
        let cand = (lag, longest, overlap, sum / overlap as f64);
        match best {
            // rank by the longest in-band run, then by the closer mean
            Some(b0) if !(cand.1 > b0.1 || (cand.1 == b0.1 && cand.3 < b0.3)) => {}
            _ => best = Some(cand),
        }
    }
    best
}

fn load(p: &str) -> entrec::Decoded {
    match entrec::decode_ghost(p) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("{}: {}", p, e);
            std::process::exit(3)
        }
    }
}

fn main() {
    let mut files: Vec<String> = Vec::new();
    let mut controls: Vec<(String, String)> = Vec::new();
    let mut mm = 1.0_f64;
    let mut minrun = 100_usize;
    let mut ratio = 10.0_f64;
    let mut args = env::args().skip(1);
    while let Some(a) = args.next() {
        match a.as_str() {
            "--mm" => mm = args.next().and_then(|v| v.parse().ok()).unwrap_or(1.0),
            "--run" => minrun = args.next().and_then(|v| v.parse().ok()).unwrap_or(100),
            "--ratio" => ratio = args.next().and_then(|v| v.parse().ok()).unwrap_or(10.0),
            "--control" => {
                let x = args.next().unwrap_or_default();
                let y = args.next().unwrap_or_default();
                if x.is_empty() || y.is_empty() {
                    eprintln!("--control takes TWO files");
                    std::process::exit(2);
                }
                controls.push((x, y));
            }
            _ => files.push(a),
        }
    }
    if files.len() != 2 {
        eprintln!(
            "usage: nearident A.Gbx B.Gbx --control C1.Gbx C2.Gbx [--control ...] \
             [--mm 1.0] [--run 100] [--ratio 10]"
        );
        std::process::exit(2);
    }
    if controls.is_empty() {
        eprintln!(
            "REFUSED: no --control given.\n\
             \n\
             This tool used to answer without one and the answer was wrong four times. A\n\
             separation of half a millimetre means \"copy\" only if a pair known to be two\n\
             different runs measures much further apart ON THIS MAP -- our own writer's floor\n\
             against a game recording is ~0.5 mm, which is inside the old 1 mm band, so the\n\
             band alone flags every honest regeneration.\n\
             \n\
             Pass two recordings of this map that cannot be the same run. Human against human\n\
             is the pairing that works: neither came out of our pipeline, so whatever they\n\
             measure is what \"independent\" looks like here."
        );
        std::process::exit(3);
    }
    let a = load(&files[0]);
    let b = load(&files[1]);
    let tol = mm / 1000.0;

    let subj = match scan(&a, &b, tol) {
        Some(v) => v,
        None => {
            eprintln!(
                "UNMEASURED: {} and {} share no sample instant at any lag in +-750 samples.\n\
                 Sample times are SESSION times, so two recordings made in different sessions\n\
                 can have no time key in common. Nothing was compared -- this says nothing\n\
                 about either file, and it is NOT a clean result.",
                files[0], files[1]
            );
            std::process::exit(3);
        }
    };
    println!(
        "A {} samples, B {} samples -- band {:.3} mm, min run {}, ratio bar {}x",
        a.samples.len(),
        b.samples.len(),
        mm,
        minrun,
        ratio
    );
    println!(
        "subject  lag={} overlap={} longest_in_band={} mean={:.6} m",
        subj.0, subj.2, subj.1, subj.3
    );

    // The control: what a pair that is definitely two runs measures on this map.
    let mut ctl_mean = f64::MAX;
    let mut ctl_run = usize::MAX;
    let mut measured = 0usize;
    for (x, y) in &controls {
        let cx = load(x);
        let cy = load(y);
        match scan(&cx, &cy, tol) {
            Some(c) => {
                measured += 1;
                println!(
                    "control  {} vs {}: lag={} overlap={} longest_in_band={} mean={:.6} m",
                    x, y, c.0, c.2, c.1, c.3
                );
                // the WEAKEST control is the honest one to compare against
                if c.3 < ctl_mean {
                    ctl_mean = c.3;
                }
                if c.1 < ctl_run {
                    ctl_run = c.1;
                }
            }
            None => println!("control  {} vs {}: NO SHARED INSTANT -- unusable", x, y),
        }
    }
    if measured == 0 {
        eprintln!(
            "UNMEASURED: not one control pair shared a sample instant, so there is no scale to\n\
             judge the subject against. Find two recordings of this map from one session, or\n\
             two the game itself wrote."
        );
        std::process::exit(3);
    }

    let r = ctl_mean / subj.3;
    println!(
        "closeness ratio: control mean {:.6} m / subject mean {:.6} m = {:.1}x",
        ctl_mean, subj.3, r
    );
    if subj.1 >= minrun && r >= ratio {
        println!(
            "VERDICT COPY: {} consecutive samples inside {} mm at lag {}, and the subject sits \
             {:.1}x closer than a pair known to be two runs.",
            subj.1, mm, subj.0, r
        );
        println!(
            "  Before acting on this: COMPARE THE INPUT TAPES. Two runs whose tapes share a \
             long prefix are bit-for-bit on the same line for that prefix BY DETERMINISM, and \
             that is not a splice. `tmtas trace` both and diff."
        );
        std::process::exit(1);
    }
    if subj.1 >= minrun {
        println!(
            "VERDICT INCONCLUSIVE: {} consecutive samples inside {} mm, but only {:.1}x closer \
             than the control (bar {:.1}x). On this map that band does not separate a copy from \
             a shared line. Decide it on the input tapes, not here.",
            subj.1, mm, r, ratio
        );
        std::process::exit(0);
    }
    println!(
        "VERDICT INDEPENDENT: longest in-band run is {} samples, under the {}-sample bar \
         (control's own longest in-band run: {}).",
        subj.1, minrun, ctl_run
    );
}
