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

/// The shim `fk` will load, when the caller names one.
///
/// This used to be a SECOND copy of `fk`'s own lookup, and it knew only the
/// shim's old name (`libfkshim.so`) with a hard-coded `/tmp/fk/rs/...`
/// fallback. `tools/search` builds `libforkshim.so`, so the copy here never
/// found the current shim and always handed `fk` the stale one left in `/tmp`
/// by an old bundle -- and a mismatched shim does not fail, it HANGS: the
/// child answers the `G` command with something that is not the `GO` ack, and
/// four regenerations sat in that state for forty-five minutes each before
/// anyone looked (measured 2026-08-22; the same run against the repo's shim
/// finishes in 10 s).
///
/// So there is no lookup here any more. `fk` owns resolving the shim, because
/// `fk` is what loads it; this only forwards an explicit choice.
///
/// This was fixed twice on the same day, independently, and the other fix
/// taught this one both names and both search roots. That version is the reason
/// the diagnosis below is precise -- but it left TWO lookups in the tree, which
/// is the defect that produced the bug in the first place. One owner.
fn explicit_shim() -> Option<String> {
    std::env::var("FK_SHIM").ok()
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
        "--server".into(),
        crate::oracle::server_dir(None).to_string_lossy().to_string(),
    ];
    if let Some(s) = explicit_shim() {
        args.push("--shim".into());
        args.push(s);
    }
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
        for k in ["--verbose", "--inherit-outside", "--allow-partial",
                  "--keep-transform", "--noanchor"] {
            if has(a, k) {
                v.push(k.to_string());
            }
        }
        // ALWAYS ON, no flag, no way to turn them off.
        //
        // `--neutralise` zeros the 49 per-run bytes the transform encoder does
        // not write; `--inputs` rewrites the record's steer/gas/brake echo from
        // the tape; `--trim-outside` drops samples with no engine instant
        // behind them. All three were opt-in, all three were forgotten, and
        // each omission shipped: the tyres throwing a stranger's dirt is the
        // first, a record describing the donor's driving is the second.
        //
        // Making them options was the mistake. There is no case for a file that
        // is publishable-except-for-one-of-these -- that is just a defect with
        // a flag in front of it.
        v.push("--neutralise".into());
        v.push("--inputs".into());
        v.push("--trim-outside".into());
        v
    };
    let tries: i64 = num(a, "--tries").unwrap_or(24);
    let jobs: usize = num(a, "--jobs").unwrap_or(12).max(1) as usize;
    let force = has(a, "--force");
    // The container this file is built in, kept before `inp` becomes the
    // rebuilt grid: the finishing pass measures per-byte provenance against it,
    // and against the grid the answer would be meaningless.
    let template_for_provenance = inp.clone();

    // ---- STEP 0: THE GRID THIS RUN WILL BE WRITTEN INTO IS OURS -------------
    //
    // The regenerator writes engine state into the record the template already
    // has, so a template carrying the donor's span produces a file carrying the
    // donor's span -- 33 of 159 published ghosts do, and the symptom is a clip
    // twice as long as the run with the camera stranded at the top of the map
    // after our car's entity ends. This used to be `ghost record rebuild`, a
    // separate command run afterwards by whoever knew to.
    //
    // It is also what makes 227654 regenerable at all: that carrier is ONE CAR
    // SPLIT INTO 27 ENTITIES at its respawns, so every reader in this project
    // sees "365 samples spanning 1.310 .. 19.480" for a 57.482 run and the
    // regenerator hard-errors on the 38 seconds it cannot fill.
    //
    // The span is the file's own simulated time, asked of the plain oracle --
    // never the filename and never the header, which on a synthesised tape is
    // the seed's.
    let inp = {
        let t = crate::oracle::server_dir(flag(a, "--server"));
        let simulated = if t.join("TrackmaniaServer").exists() {
            crate::oracle::validate(&t, std::path::Path::new(inp), crate::oracle::MapsMode::One(std::path::Path::new(&map)), "regen-span")
                .ok()
                .and_then(|r| r.time_ms)
        } else {
            None
        };
        match simulated {
            None => {
                println!("== step 0: no oracle available, so the record's span is left as it is");
                inp.clone()
            }
            Some(ms) => {
                // .Ghost.Gbx, not `{out}.grid`: the dedicated server IGNORES a
                // file with any other extension and returns a bare DNF that
                // cannot be told from a genuine one -- the oracle wrapper says
                // so in as many words, and it caught this staging name on the
                // first run.
                let staged = format!("{}.grid.Ghost.Gbx", out.trim_end_matches(".Ghost.Gbx"));
                match crate::record::rebuild_to(inp, &staged, ms, None, 50) {
                    Ok(msg) => println!("== step 0: the record grid is ours\n   {msg}"),
                    Err(e) => die(format!("could not rebuild the record grid: {e}")),
                }
                // AND THE DECLARED TIME, HERE, BEFORE THE ENGINE RUNS.
                //
                // It has to be before: G4 re-simulates the WRITTEN file and
                // compares against what that file declares, so a grid still
                // carrying the carrier's 147.031 makes every one of 24 correct
                // regenerations fail a check about a number none of them wrote.
                // Declaring it afterwards would mean the gate ran against a
                // claim we already knew was wrong.
                let dstage = format!("{}.decl.Ghost.Gbx", out.trim_end_matches(".Ghost.Gbx"));
                let mut d: Vec<String> = vec![staged.clone(), dstage.clone(), "--from-oracle".into(),
                                              "--map".into(), map.to_string()];
                if let Some(sv) = flag(a, "--server") {
                    d.push("--server".into());
                    d.push(sv.to_string());
                }
                crate::declare::cmd(&d);
                let _ = std::fs::remove_file(&staged);
                dstage
            }
        }
    };
    let inp = &inp;

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
            finish(out, &template_for_provenance, &map, a, force);
        }
    }
}

/// Everything between "the engine state is in the record" and "this file is
/// publishable" -- run on every regeneration, not as a follow-up somebody has
/// to remember.
///
/// The list used to live in people's heads and every item on it shipped at
/// least once: a stranger's dirt-tyre bytes, a 441 s span on a 218 s run, a
/// header declaring the donor's time, a foreign login in a field nothing read.
/// Each was found days later by someone noticing something odd in a video.
///
/// Every step here refuses on its own terms, and this refuses on theirs.
fn finish(out: &str, carrier: &str, map: &str, a: &[String], force: bool) {
    println!("\n== the finishing pass");

    // The declared time is written in step 0, BEFORE the engine runs -- G4
    // re-simulates the written file and checks it against what the file
    // claims, so a claim fixed afterwards would mean the gate ran against a
    // number we already knew was wrong.

    // 1. THE IDENTITY. Nine fields, two of them found the day this was written
    //    -- the ranked badge and the zone -- by raw-stringing published files.
    //    `--anonymise` is position-based, so a field nobody has named yet is
    //    still found by the same scan.
    let tmp = format!("{out}.fin");
    let mut i: Vec<String> = vec!["set".into(), out.to_string(), tmp.clone(), "--anonymise".into(),
                                  "--map".into(), map.to_string()];
    if let Some(s) = flag(a, "--server") {
        i.push("--server".into());
        i.push(s.to_string());
    }
    crate::ident::cmd(&i);
    let _ = std::fs::rename(&tmp, out);

    // 2. THE ACCEPTANCE TEST: what, if anything, is still the donor's.
    println!("\n== what this file is made of");
    let mut refused: Vec<String> = Vec::new();
    match crate::finish::inherited_bytes(out, carrier) {
        Ok(v) if v.is_empty() => {
            println!("   no sample byte is bit-identical to the container donor throughout.");
        }
        Ok(v) => refused.push(format!(
            "{} sample byte(s) are still bit-identical to the donor on EVERY sample: {:?}. \
             Those describe THEIR run -- every tyre effect and contact spark in a render fires \
             at the instant they had it, not ours.",
            v.len(),
            v
        )),
        Err(e) => println!("   per-byte provenance could not be measured: {e}"),
    }
    match crate::finish::outlives_the_car(out) {
        Ok(None) => println!("   nothing in the file outlives the car."),
        Ok(Some(w)) => refused.push(format!("something outlives the car: {w}")),
        Err(e) => println!("   the span could not be measured: {e}"),
    }

    // 3. WHAT WE DID NOT WRITE, BY NUMBER. A harness limit, said as one.
    let un = crate::finish::unwritten_channels();
    println!(
        "   UNWRITTEN, zeroed rather than inherited ({} channels): {}",
        un.len(),
        un.iter().map(|(o, n)| format!("{o} {n}")).collect::<Vec<_>>().join(", ")
    );
    println!(
        "   Those quantities ARE in engine memory -- fitted against a real recording (gear,\n\
         \x20  turbo and wetness exact on every sample, rpm on 92.6 %). What is missing is an\n\
         \x20  anchor that survives a change of map, so they are written as ZERO and named here\n\
         \x20  rather than passed through as the donor's."
    );

    if !refused.is_empty() {
        for r in &refused {
            eprintln!("REFUSED: {r}");
        }
        if !force {
            let _ = std::fs::remove_file(out);
            die(format!(
                "{} check(s) failed and {out} has been DELETED. Pass --force to keep the file \
                 anyway; it is not publishable.",
                refused.len()
            ));
        }
        eprintln!("--force: keeping {out} despite {} failed check(s).", refused.len());
    }
    println!("\n{out} is finished.");
}

/// The acceptance gate for one regenerated candidate.
fn gate(cand: &str, map: &str, a: &[String], template: &str) -> Result<String, String> {
    let mut s = String::new();
    let (len, _first, n, ok) = path_stats(cand).ok_or("   G3 the written file has no samples")?;
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
    //
    // AND THE ANSWER KEY IS ONLY AN ANSWER KEY IF THE TEMPLATE STARTS AT RACE
    // ZERO. Compare two samples taken at DIFFERENT race times and the check
    // measures how far the car drove in between: 227654's carrier begins at
    // race 1.310 s doing 66 km/h, so a candidate that is correctly at the spawn
    // reads 11.2 m out and is refused, while a candidate that is 11.2 m down
    // the road passes. That is the check inverted, on the exact class of file
    // this pipeline exists to repair. So compare at a COMMON INSTANT: the
    // candidate's own sample nearest the template's first, and refuse to judge
    // at all when there is no overlap rather than judging against the wrong
    // moment.
    // The reference defaults to the template, which is right when the template
    // is a real recording of this map. It is NOT right when the template is a
    // grid `ghost record rebuild` laid down -- every sample there is a copy of
    // one, so the record does not move and identifies nothing. Name a
    // downloaded recording with `--spawn-ref` for those.
    let spawn_ref = flag(a, "--spawn-ref").unwrap_or(template);
    let ref_moves = gbx::record::decode_ghost(spawn_ref)
        .ok()
        .map(|d| {
            d.samples
                .windows(2)
                .map(|w| {
                    ((w[1].x - w[0].x).powi(2) + (w[1].y - w[0].y).powi(2) + (w[1].z - w[0].z).powi(2)).sqrt()
                })
                .sum::<f64>()
                > 5.0
        })
        .unwrap_or(false);
    match gbx::record::decode_ghost(spawn_ref)
        .ok()
        .filter(|_| ref_moves)
        .and_then(|d| d.samples.first().cloned())
    {
        None => s.push_str(
            "   G2 UNMEASURED: the reference record has no motion in it (a rebuilt grid is a \
             constant), so it identifies nothing. Name a downloaded recording of this map with \
             --spawn-ref. The start position has NOT been checked.\n",
        ),
        Some(t0) => {
            let here = gbx::record::decode_ghost(cand)
                .ok()
                .and_then(|d| {
                    d.samples
                        .iter()
                        .min_by_key(|s| (s.time_ms - t0.time_ms).abs())
                        .map(|s| (s.time_ms, [s.x, s.y, s.z]))
                });
            let tol: f64 = flag(a, "--spawn-tol").and_then(|v| v.parse().ok()).unwrap_or(1.0);
            match here {
                Some((t, p)) if (t - t0.time_ms).abs() <= 60 => {
                    let d = ((p[0] - t0.x).powi(2) + (p[1] - t0.y).powi(2) + (p[2] - t0.z).powi(2)).sqrt();
                    if d > tol {
                        return Err(format!(
                            "   G2 at {} ms the run is at ({:.2}, {:.2}, {:.2}), {:.1} m from where \
                             the template is at the same instant ({:.2}, {:.2}, {:.2}) -- this is \
                             not the car",
                            t, p[0], p[1], p[2], d, t0.x, t0.y, t0.z
                        ));
                    }
                    s.push_str(&format!(
                        "   G2 at {} ms, {:.3} m from the template at the same instant\n",
                        t, d
                    ));
                }
                _ => s.push_str(&format!(
                    "   G2 UNMEASURED: the template's first sample is at {} ms and this record has \
                     none within 60 ms of it, so there is no common instant to compare. The start \
                     position has NOT been checked.\n",
                    t0.time_ms
                )),
            }
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
