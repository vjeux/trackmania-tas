//! `ghost regen` -- from INPUTS + MAP, generate the car state and write it into
//! the ghost, so the recorded trajectory matches the tape.
//!
//! This is the operation that keeps biting the project, and it bites in a
//! specific way: the state reader locates the car in a live engine's memory,
//! and WHICH COPY it lands on varies between runs. A wrong pick produces a
//! file that passes coverage, passes a manifest, passes a two-run agreement
//! test, and carries a frozen memory slot instead of a car.
//!
//! So this wrapper does not just call the regenerator. It runs it, then puts
//! the result through a gate that can IDENTIFY the answer rather than count
//! votes:
//!
//!   G1  the written file's own telemetry agrees with the tape it carries
//!       (the two input channels in one file, no reference needed);
//!   G2  the first sample is at the map's spawn, not somewhere else;
//!   G3  the path length is a plausible driven distance, not 0 m and not 1e28;
//!   G4  the plain oracle re-simulates THE WRITTEN FILE to the declared time;
//!   G5  no sample is NaN and no position is constant for the whole run.
//!
//! and `ghost regen-control` runs the strongest control the write path has: a
//! ghost that already carries its own true telemetry, regenerated from its own
//! inputs, must reproduce that telemetry. If the fixed point does not hold on a
//! known-good file, nothing the regenerator says about an unknown one counts.

use gbx::container::secs;
use crate::verify;
use crate::cli::{die, flag, has, num};
use std::path::Path;
use std::process::Command;

fn fk_binary() -> String {
    if let Ok(v) = std::env::var("FK_BIN") {
        return v;
    }
    if let Ok(me) = std::env::current_exe() {
        if let Some(d) = me.parent() {
            let p = d.join("fk");
            if p.exists() {
                return p.to_string_lossy().into();
            }
        }
    }
    // `fk` is part of the internal fork-server toolchain, not of this crate, so
    // it is normally somewhere else on PATH. Resolve it there and remember the
    // directory: its shim lives beside it.
    if let Ok(paths) = std::env::var("PATH") {
        for d in paths.split(':') {
            let p = std::path::Path::new(d).join("fk");
            if p.exists() {
                return p.to_string_lossy().into();
            }
        }
    }
    "fk".into()
}

/// Where the LD_PRELOAD shim is.
///
/// **Both names are tried.** The crate was renamed `forkshim` and its artefact
/// with it (`libforkshim.so`); `fk`'s own lookup accepts either, and this one
/// did not, so from a clean clone `ghost regen` handed the fork server a path
/// that does not exist. The server then panics inside `forksrv` with a bare
/// `NotFound` — once per attempt, twenty-four times — and the output reads
/// exactly like twenty-four bad locates. That is the failure this crate's own
/// README warns about, reproduced by this crate.
///
/// Returns `None` rather than a guess: a made-up default is what turns a
/// wiring error into a physics story. The caller refuses and names the knob.
fn shim() -> Option<String> {
    if let Ok(v) = std::env::var("FK_SHIM") {
        return Some(v);
    }
    const NAMES: [&str; 2] = ["libforkshim.so", "libfkshim.so"];
    let mut dirs: Vec<std::path::PathBuf> = Vec::new();
    // beside the fk binary that will actually run -- NOT beside this one. They
    // are different crates in different trees.
    let fk = fk_binary();
    if let Some(d) = std::path::Path::new(&fk).parent() {
        dirs.push(d.to_path_buf());
        // `cargo build` leaves the shim in the SEARCH workspace's target dir,
        // never in fk's own: the shim and the driver `#[path]`-include one
        // `pred_core.rs`, so a second copy would be a second judge.
        dirs.push(d.join("../../../search/target/release"));
    }
    if let Ok(me) = std::env::current_exe() {
        if let Some(d) = me.parent() {
            dirs.push(d.to_path_buf());
            dirs.push(d.join("../search/target/release"));
            dirs.push(d.join("../../search/target/release"));
        }
    }
    dirs.push(std::path::PathBuf::from("/tmp/fk/rs/target/release"));
    for d in dirs {
        for n in NAMES {
            let p = d.join(n);
            if p.exists() {
                return Some(p.to_string_lossy().into());
            }
        }
    }
    None
}

pub struct RegenOut {
    pub ok: bool,
    pub log: String,
}

/// Run the regenerator once. `dump` MUST be absolute: a relative path fails
/// inside the forked server with a bare `go ERR open`.
pub fn run_regen(template: &str, map: &str, out: &str, extra: &[String]) -> RegenOut {
    let shim = match shim() {
        Some(s) => s,
        None => {
            // Name the knob rather than launching. A shim that is not there
            // makes 24 launches that never happened report in the words of 24
            // failed locates.
            return RegenOut {
                ok: false,
                log: "the LD_PRELOAD shim (libforkshim.so / libfkshim.so) was not found beside \
                      fk, in tools/search/target/release, or at $FK_SHIM. Build it with \
                      `cd tools/search && cargo build --release -p forkshim`, or set FK_SHIM. \
                      NOT LAUNCHING: a missing shim reads exactly like a failed locate."
                    .into(),
            };
        }
    };
    let dump = std::env::temp_dir()
        .join(format!("ghost-regen-{}-{}.bin", std::process::id(), rand_tag()))
        .to_string_lossy()
        .to_string();
    let mut args: Vec<String> = vec![
        "regen".into(),
        "--template".into(),
        abspath(template),
        "--map".into(),
        abspath(map),
        "--out".into(),
        abspath(out),
        "--dump".into(),
        dump,
        "--shim".into(),
        shim,
        "--server".into(),
        crate::oracle::server_dir(None).to_string_lossy().to_string(),
    ];
    args.extend_from_slice(extra);
    let o = Command::new(fk_binary()).args(&args).output();
    match o {
        Err(e) => RegenOut { ok: false, log: format!("cannot run {}: {}", fk_binary(), e) },
        Ok(o) => {
            let log = format!(
                "{}{}",
                String::from_utf8_lossy(&o.stdout),
                String::from_utf8_lossy(&o.stderr)
            );
            RegenOut { ok: o.status.success() && Path::new(out).exists(), log }
        }
    }
}

fn rand_tag() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    format!("{}", SystemTime::now().duration_since(UNIX_EPOCH).unwrap().subsec_nanos())
}

fn abspath(p: &str) -> String {
    std::fs::canonicalize(p)
        .map(|x| x.to_string_lossy().to_string())
        .unwrap_or_else(|_| {
            let cwd = std::env::current_dir().unwrap();
            cwd.join(p).to_string_lossy().to_string()
        })
}

/// Path length of the written file's own recorded trajectory, and its first
/// sample. `carscan`'s test in one place: a car's path is the map's ribbon, a
/// wrong memory copy's is 0.0 m or 1e28 m.
pub fn path_stats(file: &str) -> Option<(f64, [f64; 3], usize, bool)> {
    let d = gbx::record::decode_ghost(file).ok()?;
    if d.samples.is_empty() {
        return None;
    }
    let mut len = 0.0;
    let mut finite = true;
    let mut moved = false;
    for w in d.samples.windows(2) {
        let (a, b) = (&w[0], &w[1]);
        let dx = b.x - a.x;
        let dy = b.y - a.y;
        let dz = b.z - a.z;
        let s = (dx * dx + dy * dy + dz * dz).sqrt();
        if !s.is_finite() {
            finite = false;
            continue;
        }
        if s > 1e-6 {
            moved = true;
        }
        len += s;
    }
    let f = &d.samples[0];
    Some((len, [f.x, f.y, f.z], d.samples.len(), finite && moved))
}

pub fn cmd(a: &[String]) {
    let inp = a.first().unwrap_or_else(|| die("ghost regen IN OUT --map MAP"));
    let out = a.get(1).unwrap_or_else(|| die("ghost regen IN OUT --map MAP"));
    let map = flag(a, "--map").unwrap_or_else(|| die("--map MAP.Map.Gbx"));
    let extra: Vec<String> = {
        let mut v = Vec::new();
        // Value flags, forwarded verbatim. `--fieldmap` is NOT among them any
        // more: `fk` deleted it along with the field-fitting machinery, and a
        // wrapper that forwards a flag the callee does not know is a silent
        // no-op wearing the shape of a feature.
        for k in ["--anchorticks", "--segs", "--biastick", "--inputshift", "--anchor", "--race", "--quat-kind"] {
            if let Some(x) = flag(a, k) {
                v.push(k.to_string());
                v.push(x.to_string());
            }
        }
        // Switches. `--neutralise` is the one that matters and it was
        // unreachable from here: it is the only way to stop a regenerated file
        // being quietly part-carrier, since the 49 per-run bytes the transform
        // encoder does not write -- ground contact, wheels, rpm, surface
        // effects -- otherwise stay the donor's. Every C5/C6/C7 refusal on this
        // project's published corpus is those bytes.
        for k in ["--verbose", "--trim-outside", "--inherit-outside", "--allow-partial",
                  "--neutralise", "--inputs", "--keep-transform", "--noanchor"] {
            if has(a, k) {
                v.push(k.to_string());
            }
        }
        v
    };
    let tries: i64 = num(a, "--tries").unwrap_or(24);
    let jobs: usize = num(a, "--jobs").unwrap_or(12).max(1) as usize;

    // THE LOCATE IS A CHOOSER AND IT IS NONDETERMINISTIC. Measured on the
    // fixture map: 8 identical runs, one found the car and seven found a
    // neighbouring object -- and the gate below refused all seven. A base rate
    // near 1 in 8 is why this runs a DOZEN attempts at once instead of
    // retrying serially and giving up. So run several
    // attempts AT ONCE, each with a different anchor ladder, and take the first
    // the gate accepts. Diversifying the anchor is not cosmetic: which object
    // the locate finds follows from where it starts looking.
    // AND DIVERSIFYING THE ANCHOR MAKES IT WORSE, WHICH IS NOT WHAT I EXPECTED.
    // Eight runs on the default ladder: 1 found the car. Twenty-four runs
    // spread over seven hand-picked anchor ladders: 0 did. So every attempt
    // uses the default ladder unless the caller asks for one, and the only
    // thing that buys reliability here is running more of them at once.
    let ladders: Vec<Option<&str>> = vec![None];
    let mut accepted: Option<String> = None;
    let mut lastlog = String::new();
    let mut round = 0usize;
    // THE IN-PROCESS LOCATE GOES FIRST, ALONE.
    //
    // There are two locates in the state reader. The default one forks, hunts
    // the child for an object that moves like a car, and hands an address back
    // to the parent; the other locates in the clean process itself and needs no
    // cross-process assumption at all. The forking one is the brittle one, and
    // it was running first and usually winning with a decoy.
    //
    // Measured on the fixture map: six runs with the in-process locate produced
    // BIT-IDENTICAL trajectories in 13.7 s each; six runs on the default path
    // took ~90 s and disagreed. It is not universal -- it cannot see a car that
    // is barely moving at the handover, and on a short tape it finds nothing --
    // so the search is still there behind it. But it is the right thing to try
    // first, and when it works there is no search at all.
    if !has(a, "--no-inprocess") {
        let cand = format!("{}.ip", out);
        let raw = format!("{}.raw", cand);
        let mut ex = extra.clone();
        ex.push("--noanchor".into());
        let r = run_regen(inp, map, &raw, &ex);
        lastlog = r.log.clone();
        if r.ok {
            match write_input_channels(&raw, &cand) {
                Ok((w, sk)) => println!("   [in-process] input channels rewritten from the tape on {} samples ({} outside)", w, sk),
                Err(_) => {
                    let _ = std::fs::rename(&raw, &cand);
                }
            }
            let _ = std::fs::remove_file(&raw);
            match gate(&cand, map, a, inp) {
                Ok(msg) => {
                    println!("   [in-process] accepted -- no cross-process search was run");
                    println!("{}", msg);
                    accepted = Some(cand);
                }
                Err(msg) => {
                    println!("{}", msg);
                    println!("   [in-process] refused; falling back to the anchor search");
                    let _ = std::fs::remove_file(&cand);
                }
            }
        } else {
            let _ = std::fs::remove_file(&raw);
            println!("   [in-process] no file (this tape is probably not moving at the handover); falling back to the anchor search");
        }
    }
    while (round as i64) < tries && accepted.is_none() {
        let batch: Vec<usize> = (round..(round + jobs).min(tries as usize)).collect();
        if batch.is_empty() {
            break;
        }
        println!("== regeneration attempts {:?} of {} (in parallel)", batch, tries);
        let results: Vec<(usize, String, RegenOut)> = std::thread::scope(|sc| {
            let hs: Vec<_> = batch
                .iter()
                .map(|kk| {
                    let k = *kk;
                    let inp = inp.to_string();
                    let mapx = map.to_string();
                    let cand = format!("{}.try{}", out, k);
                    let mut ex = extra.clone();
                    if !ex.iter().any(|x| x == "--anchorticks") {
                        if let Some(Some(l)) = ladders.get(k % ladders.len()) {
                            ex.push("--anchorticks".into());
                            ex.push((*l).to_string());
                        }
                    }
                    sc.spawn(move || {
                        let raw = format!("{}.raw", cand);
                        let r = run_regen(&inp, &mapx, &raw, &ex);
                        (k, cand, r)
                    })
                })
                .collect();
            hs.into_iter().map(|h| h.join().unwrap()).collect()
        });
        for (k, cand, r) in results {
            let raw = format!("{}.raw", cand);
            lastlog = r.log.clone();
            if let Some(l) = r.log.lines().find(|l| l.starts_with("record layout:")) {
                println!("   [{}] {}", k, l);
            }
            if !r.ok {
                println!("   [{}] the regenerator did not produce a file", k);
                for l in r.log.lines().rev().take(2) {
                    println!("   [{}] | {}", k, l);
                }
                let _ = std::fs::remove_file(&raw);
                continue;
            }
            // Widen the write: the recorded steer / gas / brake channels come
            // from the TAPE, not from the engine, and leaving them as the
            // carrier's is how a regenerated file describes somebody else's
            // driving.
            match write_input_channels(&raw, &cand) {
                Ok((w, sk)) => println!(
                    "   [{}] input channels rewritten from the tape on {} samples ({} outside the tape)",
                    k, w, sk
                ),
                Err(e) => {
                    println!("   [{}] could not rewrite the input channels: {}", k, e);
                    let _ = std::fs::rename(&raw, &cand);
                }
            }
            let _ = std::fs::remove_file(&raw);
            if accepted.is_some() {
                let _ = std::fs::remove_file(&cand);
                continue;
            }
            match gate(&cand, map, a, inp) {
                Ok(msg) => {
                    println!("   [{}] accepted", k);
                    println!("{}", msg);
                    accepted = Some(cand);
                }
                Err(msg) => {
                    println!("{}", msg);
                    println!("   [{}] REFUSED -- the locate found something that is not the car.", k);
                    let _ = std::fs::remove_file(&cand);
                }
            }
        }
        round += jobs;
    }
    match accepted {
        None => {
            eprintln!("--- last regenerator log ---");
            eprintln!("{}", lastlog);
            die(format!(
                "no regeneration passed the gate in {} attempts. NOTHING was written to {}: a file \
                 that fails this gate is a frozen memory slot, not a car.",
                tries, out
            ));
        }
        Some(c) => {
            std::fs::rename(&c, out).unwrap_or_else(|e| die(format!("{}: {}", out, e)));
            println!("wrote {}", out);
            println!(
                "  WHAT IS OURS in each 116-byte sample: the 22 transform bytes (position, \n\
                 \x20 orientation, speed, velocity direction) from the engine, and bytes 14 / 15 / 18 \n\
                 \x20 (steer, gas, brake) from the tape. The remaining 91 -- rpm, gear, wheel \n\
                 \x20 rotation and suspension, surface effects -- are still the carrier's. They are \n\
                 \x20 in engine memory too; nothing here has read them yet."
            );
        }
    }
}

/// The acceptance gate for one regenerated candidate.
fn gate(cand: &str, map: &str, a: &[String], template: &str) -> Result<String, String> {
    let mut s = String::new();
    let (len, first, n, ok) = path_stats(cand).ok_or("   G3 the written file has no samples")?;
    if !ok {
        return Err(format!(
            "   G5 the trajectory is not finite or never moves ({} samples, {:.1} m)",
            n, len
        ));
    }
    if !(1.0..=1.0e6).contains(&len) {
        return Err(format!(
            "   G3 path length {:.1} m over {} samples -- not a driven path",
            len, n
        ));
    }
    s.push_str(&format!("   G3 path length {:.1} m over {} samples\n", len, n));
    // G2: THE RUN MUST START WHERE THE RUN STARTS. The template is a recording
    // of the same map from the same spawn, so its own first sample is the
    // answer key -- free, in the file, and no reference elsewhere needed. This
    // is what separates the car from the other moving objects the engine keeps:
    // measured on the fixture map, one candidate in six traces a perfectly
    // plausible 1.6 km path that is nowhere near the track.
    match gbx::record::decode_ghost(template).ok().and_then(|d| d.samples.first().cloned()) {
        None => s.push_str("   G2 no telemetry in the template to check the start against\n"),
        Some(t0) => {
            let d = ((first[0] - t0.x).powi(2) + (first[1] - t0.y).powi(2) + (first[2] - t0.z).powi(2)).sqrt();
            let tol: f64 = flag(a, "--spawn-tol").and_then(|v| v.parse().ok()).unwrap_or(1.0);
            if d > tol {
                return Err(format!(
                    "   G2 the run starts at ({:.2}, {:.2}, {:.2}), {:.1} m from where this map's \
                     runs start ({:.2}, {:.2}, {:.2}) -- this is not the car",
                    first[0], first[1], first[2], d, t0.x, t0.y, t0.z
                ));
            }
            s.push_str(&format!(
                "   G2 starts at ({:.2}, {:.2}, {:.2}), {:.3} m from the template's own start\n",
                first[0], first[1], first[2], d
            ));
        }
    }
    match verify::tape_record_agreement(cand) {
        None => return Err("   G1 the written file has no telemetry to compare with its tape".into()),
        Some((kappa, rate, lag, k)) => {
            let _ = rate;
            if kappa < 0.60 {
                return Err(format!(
                    "   G1 tape/record agreement kappa {:.3} over {} samples (best lag {} ms) -- the record \
                     written is not this tape's run",
                    kappa, k, lag
                ));
            }
            s.push_str(&format!(
                "   G1 tape/record agreement kappa {:.3} over {} samples (best lag {} ms)\n",
                kappa, k, lag
            ));
        }
    }
    // G4: the plain oracle on THE WRITTEN FILE
    if !has(a, "--no-oracle") {
        let server = crate::oracle::server_dir(flag(a, "--server"));
        if server.join("TrackmaniaServer").exists() {
            match crate::oracle::validate(&server, Path::new(cand), crate::oracle::MapsMode::One(Path::new(map)), "regen") {
                Ok(res) => {
                    let decl = gbx::container::Container::load(cand)
                        .ok()
                        .and_then(|c| c.declared_times().first().map(|x| x.1 as i64));
                    match (res.time_ms, decl) {
                        (Some(t), Some(d)) if t == d => {
                            s.push_str(&format!("   G4 oracle on the written file: {}\n", secs(t)))
                        }
                        (Some(t), Some(d)) => {
                            return Err(format!(
                                "   G4 oracle re-simulated the written file to {} but it declares {}",
                                secs(t),
                                secs(d)
                            ))
                        }
                        (Some(t), None) => s.push_str(&format!("   G4 oracle: {}\n", secs(t))),
                        (None, _) => {
                            return Err(format!("   G4 oracle: DNF on the written file (cps {:?})", res.cps))
                        }
                    }
                }
                Err(e) => s.push_str(&format!("   G4 oracle not run: {}\n", e)),
            }
        } else {
            s.push_str("   G4 oracle not run: no dedicated server\n");
        }
    }
    s.push_str("   GATE PASSED");
    Ok(s)
}

/// The fixed-point control: regenerate a file that already carries its own true
/// telemetry, and require the regeneration to reproduce it.
pub fn control(a: &[String]) {
    let inp = a.first().unwrap_or_else(|| die("ghost regen-control FILE --map MAP"));
    let map = flag(a, "--map").unwrap_or_else(|| die("--map MAP.Map.Gbx"));
    let d0 = gbx::record::decode_ghost(inp)
        .unwrap_or_else(|e| die(format!("{} has no telemetry to control against: {}", inp, e)));
    let out = format!("{}.regen-control", inp);
    let r = run_regen(inp, map, &out, &[]);
    if !r.ok {
        eprintln!("{}", r.log);
        die("the regenerator did not produce a file");
    }
    let d1 = gbx::record::decode_ghost(&out).unwrap_or_else(|e| die(e));
    let n = d0.samples.len().min(d1.samples.len());
    let mut sum = 0.0;
    let mut worst = 0.0f64;
    let mut exact = 0usize;
    for i in 0..n {
        let (a2, b) = (&d0.samples[i], &d1.samples[i]);
        let dd = ((a2.x - b.x).powi(2) + (a2.y - b.y).powi(2) + (a2.z - b.z).powi(2)).sqrt();
        sum += dd;
        worst = worst.max(dd);
        if dd == 0.0 {
            exact += 1;
        }
    }
    let mean = if n > 0 { sum / n as f64 } else { f64::NAN };
    println!("fixed-point control on {}", inp);
    println!("  samples        {} original, {} regenerated", d0.samples.len(), d1.samples.len());
    println!("  mean position  {:.6} m", mean);
    println!("  worst          {:.6} m", worst);
    println!("  bit-identical  {} of {}", exact, n);
    // a one-tick offset is a PURE TIME SHIFT: check for it explicitly rather
    // than letting a small mean hide it
    let shifted = |k: i64| -> f64 {
        let mut s = 0.0;
        let mut c = 0usize;
        for i in 0..n {
            let j = i as i64 + k;
            if j < 0 || j >= d1.samples.len() as i64 {
                continue;
            }
            let (a2, b) = (&d0.samples[i], &d1.samples[j as usize]);
            s += ((a2.x - b.x).powi(2) + (a2.y - b.y).powi(2) + (a2.z - b.z).powi(2)).sqrt();
            c += 1;
        }
        if c == 0 {
            f64::INFINITY
        } else {
            s / c as f64
        }
    };
    let m1 = shifted(1);
    let mm1 = shifted(-1);
    println!("  one sample early/late: {:.6} m / {:.6} m", mm1, m1);
    if m1 < mean * 0.5 || mm1 < mean * 0.5 {
        println!(
            "  VERDICT: FAIL -- the regeneration is a WHOLE SAMPLE out of phase. \
             A solo clip cannot look wrong from this, but every frame-synchronous comparison is."
        );
        std::process::exit(2);
    }
    if mean.is_finite() && mean < 0.05 {
        println!("  VERDICT: PASS -- the write path is a fixed point on a file that already knew its own answer");
    } else {
        println!("  VERDICT: FAIL -- regenerating a known-good file did not reproduce it");
        std::process::exit(2);
    }
}

pub const CLASS_CSCENEVEHICLEVIS: u32 = 0x0A018000;

/// The vehicle entity's raw sample block: (sample_size, bytes).
pub fn raw_vehicle_samples(path: &str) -> Result<(usize, Vec<u8>), String> {
    let data = std::fs::read(path).map_err(|e| format!("{}: {}", path, e))?;
    let g = gbx::container::Gbx::parse(&data);
    let (version, blob) = gbx::record::find_entrecord_blob(&g.body)?;
    let rd = gbx::record::parse_record_data(&blob, version)?;
    let mut best: Option<&gbx::record::Ent> = None;
    for e in &rd.ents {
        let cid = rd.descs.get(e.type_.max(0) as usize).filter(|_| e.type_ >= 0).map(|d| d.class_id);
        if cid == Some(CLASS_CSCENEVEHICLEVIS) && best.map_or(true, |b| e.times.len() > b.times.len()) {
            best = Some(e);
        }
    }
    let e = best.ok_or("no CSceneVehicleVis entity")?;
    Ok((e.sample_size, e.raw.clone()))
}

/// The recorded steer byte for a tape steer value.
///
/// MEASURED, not assumed: over the project's whole ghost corpus this
/// reproduces byte 14 of every vehicle sample exactly. The mapping is
/// `floor((s + 127) * 255 / 254)` -- note the 254, and note the FLOOR: a round
/// misses s = 60 and s = 0, which is how a "close enough" encoder ends up one
/// grid step out on half the file.
pub fn steer_byte(s: i8) -> u8 {
    (((s as i32 + 127) as i64 * 255) / 254) as u8
}

/// The recorded gas / brake byte for a digital pedal.
pub fn pedal_byte(on: u32) -> u8 {
    if on != 0 {
        255
    } else {
        0
    }
}

/// Run the real engine on the file's OWN tape and compare the trajectory it
/// produces with the trajectory the file records.
///
/// Returns (mean position error, worst, samples compared, one-sample-shift).
/// The shift flag is separate on purpose: a one-tick offset is a PURE TIME
/// SHIFT, so it hides inside a small mean and only shows when the comparison is
/// re-scored against the neighbouring samples.
pub fn engine_trajectory_agreement(path: &str, map: &str) -> Result<(f64, f64, usize, bool), String> {
    let d0 = gbx::record::decode_ghost(path).map_err(|e| e.to_string())?;
    if d0.samples.is_empty() {
        return Err("this file has no telemetry to compare".into());
    }
    // The locate finds the car about one run in eight, so ask for a dozen at
    // once and take the first that identifies it. Voting between runs is not
    // the test -- two wrong picks have agreed with each other to the metre.
    let out = regen_best(path, map, 12, 24)
        .ok_or("the engine readout did not identify the car in 24 attempts")?;
    let d1 = gbx::record::decode_ghost(&out).map_err(|e| e.to_string())?;
    let n = d0.samples.len().min(d1.samples.len());
    let dist = |a: &gbx::record::Sample, b: &gbx::record::Sample| {
        ((a.x - b.x).powi(2) + (a.y - b.y).powi(2) + (a.z - b.z).powi(2)).sqrt()
    };
    let mut sum = 0.0;
    let mut worst = 0.0f64;
    for i in 0..n {
        let dd = dist(&d0.samples[i], &d1.samples[i]);
        sum += dd;
        worst = worst.max(dd);
    }
    let mean = sum / n.max(1) as f64;
    let shifted = |k: i64| -> f64 {
        let (mut s, mut c) = (0.0, 0usize);
        for i in 0..n {
            let j = i as i64 + k;
            if j < 0 || j >= d1.samples.len() as i64 {
                continue;
            }
            s += dist(&d0.samples[i], &d1.samples[j as usize]);
            c += 1;
        }
        if c == 0 { f64::INFINITY } else { s / c as f64 }
    };
    let shift = mean > 0.02 && (shifted(1) < mean * 0.5 || shifted(-1) < mean * 0.5);
    let _ = std::fs::remove_file(&out);
    Ok((mean, worst, n, shift))
}

/// Write the RECORDED INPUT CHANNELS from the tape.
///
/// `fk regen` rewrites the 22 transform bytes of each sample -- position,
/// orientation, speed, velocity direction -- and leaves the other 94 as the
/// carrier's. Three of those 94 are the recorded steer, gas and brake, and
/// they do not need the engine at all: the TAPE is where they come from. A
/// regenerated file that keeps the carrier's input channels is a file whose
/// analysis tools, overlays and contamination checks all describe somebody
/// else's driving, so this closes them.
///
/// It is not a claim about the other 91 bytes. `ghost regen` names those.
pub fn write_input_channels(inp: &str, out: &str) -> Result<(usize, usize), String> {
    let t = gbx::tape::Tape::from_file(inp)?;
    let a0 = &t.archives[0];
    let so = a0.start_offset_ms as i64;
    let mut written = 0usize;
    let mut skipped = 0usize;
    gbx::recwrite::rewrite_ghost(inp, out, |rd| {
        let mut vehicle: Option<usize> = None;
        for (i, e) in rd.ents.iter().enumerate() {
            let cid = rd.descs.get(e.type_.max(0) as usize).filter(|_| e.type_ >= 0).map(|d| d.class_id);
            if cid == Some(CLASS_CSCENEVEHICLEVIS)
                && vehicle.map_or(true, |v| e.times.len() > rd.ents[v].times.len())
            {
                vehicle = Some(i);
            }
        }
        let Some(vi) = vehicle else { return Err("no CSceneVehicleVis entity".into()) };
        let e = &mut rd.ents[vi];
        let ss = e.sample_size;
        if ss < 20 {
            return Err(format!("vehicle sample size {} is too small for the input bytes", ss));
        }
        for i in 0..e.times.len() {
            let idx = (e.times[i] as i64 - so) / 10;
            if idx < 0 || idx >= a0.packets.len() as i64 {
                skipped += 1;
                continue;
            }
            let p = &a0.packets[idx as usize];
            let d = &mut e.raw[i * ss..(i + 1) * ss];
            d[14] = steer_byte(p.steer_i8());
            d[15] = pedal_byte(p.accel);
            d[18] = pedal_byte(p.brake);
            written += 1;
        }
        Ok(())
    })?;
    Ok((written, skipped))
}

/// Run several regenerations at once and return the first whose result
/// identifies the car: a plausible path length, finite and moving, and a file
/// the dedicated server will still validate. Used by the engine-trajectory
/// check, which needs a trustworthy engine run of a tape and does not care what
/// the file it came from declares.
pub fn regen_best(inp: &str, map: &str, jobs: usize, tries: usize) -> Option<String> {
    let mut round = 0usize;
    while round < tries {
        let batch: Vec<usize> = (round..(round + jobs).min(tries)).collect();
        if batch.is_empty() {
            break;
        }
        let results: Vec<(String, RegenOut)> = std::thread::scope(|sc| {
            let hs: Vec<_> = batch
                .iter()
                .map(|k| {
                    let inp = inp.to_string();
                    let map = map.to_string();
                    let cand = std::env::temp_dir()
                        .join(format!("ghost-v9-{}-{}.Ghost.Gbx", std::process::id(), k))
                        .to_string_lossy()
                        .to_string();
                    sc.spawn(move || {
                        let r = run_regen(&inp, &map, &cand, &[]);
                        (cand, r)
                    })
                })
                .collect();
            hs.into_iter().map(|h| h.join().unwrap()).collect()
        });
        let mut keep: Option<String> = None;
        for (cand, r) in results {
            let good = r.ok && gate(&cand, map, &[], inp).is_ok();
            if good && keep.is_none() {
                keep = Some(cand);
                continue;
            }
            let _ = std::fs::remove_file(&cand);
        }
        if keep.is_some() {
            return keep;
        }
        round += jobs;
    }
    None
}
