//! `fk regen` -- rewrite one ghost's per-sample vehicle telemetry from engine
//! state, and re-verify the result.
//!
//! What it writes, per sample:
//!   * bytes 47..69, the transform -- position (f32, exactly the engine's
//!     bits), orientation, speed and velocity direction, encoded through the
//!     inverse of the reader, which reproduces a real ghost's own 22 bytes on
//!     474 of 474 samples;
//!   * bytes 14, 15 and 18, the tape echo, with `--inputs`;
//!   * with `--neutralise`, zeros over every other PER-RUN byte, so no per-run
//!     byte of the donor container survives;
//!   * nothing else. What is left is reported byte by byte, because a file that
//!     is quietly part-carrier is exactly what this exercise exists to end.
//!
//! It is the engine half of `ghost regen`, which calls this and then gates the
//! result. The CLI contract below is that call.

use crate::record::{grid_of, measure_anchors, read_samples_pair, run_clean_anch};
use crate::record::{neutralise, written_bytes};
use tmtraj::recwrite::rewrite_ghost;

pub fn run(args: &[String]) -> Result<(), String> {
    let c = parse_ctx(args)?;
    let flag = |n: &str| -> Option<String> {
        args.iter().position(|a| a == n).and_then(|i| args.get(i + 1)).cloned()
    };
    // Zero every per-run byte the transform encoder and the tape echo do not
    // write, so no per-run byte of the donor container survives. See
    // `record::NEUTRALISE`. This replaces `--fieldmap`, which in every
    // production recipe was either `none` or a file that said exactly this.
    let neutral = args.iter().any(|a| a == "--neutralise");
    let segs_rel = crate::record::parse_segs(&flag("--segs").unwrap_or_else(|| "-16:40".into()));
    let dump = flag("--dump").unwrap_or_else(|| format!("/tmp/fkregen-{}.bin", std::process::id()));
    let outp = flag("--out").expect("--out");
    let biastick: i64 = flag("--biastick").unwrap_or_else(|| "200".into()).parse().unwrap();
    let verbose = args.iter().any(|a| a == "--verbose");
    let tape = args.iter().any(|a| a == "--inputs");
    let ishift: i64 = flag("--inputshift").unwrap_or_else(|| "0".into()).parse().unwrap();

    let f = crate::tape::Tape::load(&c.template)?;
    // 1. the clock bias and the state's offset from the input array, from a
    //    checkpoint far enough in that the probe is exact and the car is moving
    let bt = biastick.min((f.steer.len() as i64) / 3).max(60);
    let noanchor = args.iter().any(|a| a == "--noanchor");
    // Anchor checkpoints to try, in order. One fixed tick is not enough: a
    // trial map is barely moving at tick 200, a short map has no tick 200 at
    // all, and the locate needs a MOVING car (its whole discriminator is
    // d(pos)/dt against the stored velocity).
    let n = f.steer.len() as i64;
    // HALVE DOWNWARD, do not sample the tape uniformly. The anchor tick has to
    // land inside the RUN, and the run is usually SHORTER than the tape it is
    // carried in: a transplanted ghost inherits the carrier's input array, so a
    // 9.4 s run can sit in a 50 s tape and n/2, n/4 and 3n/4 are then all past
    // the finish -- "server never reached the checkpoint", three times, and the
    // ladder is exhausted (measured 2026-08-20 on TMX 276877: n = 5000, finish
    // at tick 1092). Halving reaches any run length in log2 steps.
    // It also puts the probe where the CAR IS DRIVING. The locator qualifies a
    // candidate over the 150 ticks after the probe, against a threshold of 2 %
    // of the speed in that window -- so an early probe is judged where the car
    // is slowest (tightest threshold) and where the first collision usually is
    // (largest real residual). On 276877 the real car scores 1.21 m/s against a
    // 0.58 m/s bar over 0.26-2.6 s, and 0.51 against 1.05 over 2.1-9.5 s.
    let mut ticks: Vec<i64> = vec![bt];
    let mut k = n / 2;
    while k >= 60 {
        ticks.push(k);
        k /= 2;
    }
    if let Some(s) = flag("--anchorticks") {
        ticks = s.split(',').filter_map(|v| v.trim().parse().ok()).collect();
    }
    ticks.retain(|t| *t >= 60 && *t < n - 20);
    ticks.dedup();
    let mut anchors: Vec<crate::record::Anchors> = Vec::new();
    // The BIAS first, on its own: the clock scan is far more robust than the
    // position locate (its signature is "+10 every tick, no exceptions"), and
    // the bias is what labels every sample. Getting it from a mid-tape
    // checkpoint, where the page-fault probe's tick estimate is exact, is what
    // keeps the early handover usable.
    let mut bias = 0i64;
    for t in &ticks {
        match crate::record::measure_bias(&c, &f, *t, verbose) {
            Ok(b) => {
                bias = b;
                println!("bias {} (tick {})", b, t);
                break;
            }
            Err(e) => println!("bias at tick {}: {}", t, e),
        }
    }
    if bias == 0 {
        println!("ABORT: could not measure the clock bias at any checkpoint");
        std::process::exit(3);
    }
    // arm `whl`: with --need-wheels a run is only usable if the gathered
    // record actually contains the wheel block, so collect anchors from EVERY
    // checkpoint in the ladder instead of stopping at the first that yields
    // any. Measured on 276874: the anchor a single checkpoint returns lands on
    // a copy of the car with nothing but zeros around it in 13 runs of 14.
    // A CALIBRATED ANCHOR. The locate is a search, and a search is the wrong
    // shape for something that has one right answer: the car sits at a fixed
    // offset from the module base for a given binary and map, and once that
    // offset has been established by a run that passed the acceptance test
    // there is nothing left to look for. `--anchor` supplies it, the
    // acceptance test still runs, and a stale calibration therefore cannot
    // produce a wrong file -- it can only fail and fall back to searching.
    let explicit = flag("--anchor").and_then(|s| {
        let p: Vec<i64> = s.split(':').filter_map(|v| v.trim().parse().ok()).collect();
        if p.len() == 6 {
            Some(crate::record::Anchors {
                bias: p[0],
                pos_delta: p[1],
                clock_delta: p[2],
                speed: 0.0,
                quat_off: p[3],
                quat_kind: p[4] as u8,
                vel_off: p[5],
            })
        } else {
            None
        }
    });
    if let Some(mut a0) = explicit {
        a0.bias = bias;
        println!("using the calibrated anchor base{:+} (no locate)", a0.pos_delta);
        anchors.push(a0);
    } else if !noanchor {
        for t in &ticks {
            match measure_anchors(&c, &f, *t, verbose) {
                Ok(mut b) => {
                    println!(
                        "anchors from tick {}: {}",
                        t,
                        b.iter()
                            .map(|a| format!("base{:+} ({:.1} m/s)", a.pos_delta, a.speed))
                            .collect::<Vec<_>>()
                            .join(", ")
                    );
                    for a in b.iter_mut() {
                        a.bias = bias;
                    }
                    anchors.append(&mut b);
                    // COLLECT FROM EVERY CHECKPOINT, not just the first that
                    // yields anything. One checkpoint usually returns ONE
                    // candidate, and if that one is a decoy the whole run is
                    // wasted -- which is why this used to succeed about one
                    // time in eight. The acceptance test below is strong
                    // enough to sort a long list, so give it a long list.
                }
                Err(e) => println!("anchor tick {}: {}", t, e),
            }
        }
    }

    // 2. the clean run
    let (times, raws) = match crate::record::targets_from_ghost(&c.template) {
        Ok(v) => v,
        Err(e) => {
            println!("ABORT: {}", e);
            std::process::exit(3)
        }
    };
    // SAMPLE EVERY TICK, not the record's 50 ms grid. The grid gate exists to
    // keep a 1 MB sweep window affordable; the production window is 452 bytes,
    // so gating at 10 ms costs a few megabytes and removes a whole failure
    // mode: 227654's carrier is a multiplayer recording whose entity starts at
    // race 1.310 s with irregular deltas, and a 50 ms gate matched 1 of its 365
    // samples. Every recorded sample time is a whole number of ticks, so a
    // per-tick stream matches all of them.
    // arm `whl`: the tick grid is right for production (every recorded instant
    // is a whole tick) but a WIDE gather at 10 ms fills a disk. --period lets a
    // field-location pass sample the record's own 50 ms grid instead.
    let period: i64 = flag("--period").unwrap_or_else(|| "10".into()).parse().unwrap();
    let phase = 0i64;
    let _ = grid_of(&times);
    // Try each anchor in turn: the clean run's own self-check is what decides,
    // and it is a structural test on the data actually sampled.
    // Two anchors with the same position delta are the same object found twice.
    anchors.dedup_by_key(|a| a.pos_delta);
    println!("{} distinct anchor candidates to try", anchors.len());
    let mut o = None;
    for (i, a) in anchors.iter().enumerate() {
        match run_clean_anch(&c, &segs_rel, Some(bias), Some(a), period, phase, &dump, verbose) {
            Ok(v) => {
                // THE ACCEPTANCE TEST. A frozen slot is perfectly
                // self-consistent, so the clean run's own structural check
                // cannot see it; this one can.
                match crate::record::car_path_len(&dump, v.reclen, v.pos_off) {
                    Ok(len) => println!("anchor base{:+}: path {:.1} m over the run", a.pos_delta, len),
                    Err(e) => {
                        println!("anchor base{:+}: REJECTED -- {}", a.pos_delta, e);
                        continue;
                    }
                }
                if i > 0 {
                    println!("anchor {} accepted (earlier ones failed the self-check)", i);
                }
                // Publish the anchor that worked, in the exact form `--anchor`
                // takes, so the next run on this binary and map does not have
                // to search for it again.
                println!(
                    "ACCEPTED ANCHOR {}:{}:{}:{}:{}:{}",
                    a.bias, a.pos_delta, a.clock_delta, a.quat_off, a.quat_kind, a.vel_off
                );
                o = Some(v);
                break;
            }
            Err(e) => println!("anchor base{:+}: {}", a.pos_delta, e),
        }
    }
    // Last resort: locate in the clean process itself. It cannot see a
    // stationary car, but when the tape is already moving at the handover it
    // needs no cross-process assumption at all.
    if o.is_none() {
        println!("falling back to an in-process locate");
        match run_clean_anch(&c, &segs_rel, Some(bias), None, period, phase, &dump, verbose) {
            Ok(v) => {
                if let Err(e) = crate::record::car_path_len(&dump, v.reclen, v.pos_off) {
                    println!("in-process locate: REJECTED -- {}", e);
                    std::process::exit(3);
                }
                o = Some(v);
            }
            Err(e) => println!("in-process locate: {}", e),
        }
    }
    let o = match o {
        Some(o) => o,
        None => {
            println!("ABORT: no anchor produced a self-consistent clean run");
            std::process::exit(3)
        }
    };
    println!(
        "clean run: {} instants ({} .. {} ms), probe at race {} ms, validator Time {:?}",
        o.instants, o.first_ms, o.last_ms, o.probe_ms, o.sim_time
    );
    println!(
        "record layout: pos +{}, quat +{}, vel +{} (reclen {})",
        o.pos_off, o.quat_off, o.vel_off, o.reclen
    );
    println!("CAR AT {:#x}", o.pos);
    let ss = raws[0].len();
    let recs = read_samples_pair(&dump, o.reclen);
    // `--recshift` USED TO LIVE HERE AND IT WAS WRONG. It shifted the pairing
    // between engine instants and record instants by a whole tick, on the
    // strength of C11b reporting every regenerated file as a clean
    // `speed x 0.010 m` stale-buffer offset. Nine files were rebuilt on it.
    //
    // The measurement was right and the conclusion was not: C11b reports a
    // MAGNITUDE, so it cannot see which side of the tick a file is on, and a
    // DOWNLOADED human ghost the game recorded itself reads exactly the same
    // (267460 human WR 0.4538 m at 45.42 m/s = 10.004 ms, 98% tick-shaped;
    // 227969 human WR 1.1931 m at 119.34 m/s = 10.022 ms, 100%). Our files sit
    // at the same lag as the game's own recording, which is the proof that the
    // unshifted pairing reproduces the game's labelling. The offset is also
    // per-map (-10 on 267460/227969, 0 on 203072), so no constant could have
    // been right anyway.
    //
    // The general form of the lesson, which cost a day: A NEGATIVE RESULT
    // REQUIRES A POSITIVE CONTROL -- including when the negative agrees with a
    // measurement you made yourself and liked. Run a known-good artefact
    // through the instrument before believing any verdict about your own work.
    let by_ms: std::collections::HashMap<i64, (&Vec<u8>, &Vec<u8>)> =
        recs.iter().map(|(c, f, l)| (*c as i64 - bias, (f, l))).collect();

    // 3. rebuild every sample
    let mut done = 0usize;
    let mut missing: Vec<i64> = Vec::new();
    // arm `whl`: --keep-transform rewrites ONLY the field map into a ghost
    // whose position is already regenerated and already validated. Rewriting
    // the transform again would re-run the copy choice and could silently
    // replace a checked trajectory with another one.
    let keepx = args.iter().any(|a| a == "--keep-transform");

    // ---- arm `intg`: A PARTIAL WRITE IS AN ERROR, NEVER A SILENT MERGE ----
    //
    // What this fixes. The loop below skips any recorded instant the clean run
    // did not sample, leaving THE DONOR'S bytes in place, and the old code
    // reported that as a printed count and returned SUCCESS. A file that is
    // quietly part-carrier is the whole defect this exercise exists to end:
    // it re-simulates to the exact millisecond, it is sub-millimetre where it
    // was written, and where it was not written it is a stranger driving.
    // Eleven published ghosts carry a human recording's samples that way.
    //
    // The write is bounded on both sides and the two sides are NOT the same
    // problem:
    //
    //   * INSIDE THE RACE, from race 0 to the finish, every sample must be
    //     ours. If the clean run could not reach one, the file is not ours and
    //     there is nothing honest to write. That is a hard error and no file
    //     is produced. The commonest cause is the handover: nothing before the
    //     shim's checkpoint is recordable, so a run whose record starts before
    //     the handover CANNOT be fully regenerated and must be refused rather
    //     than published with a donor prefix.
    //
    //   * OUTSIDE THE RACE -- the carrier's countdown lead-in and its
    //     post-finish tail -- there is no engine instant because THERE IS NO
    //     RUN THERE, and 34 of 171 files are in this position because the
    //     carrier is a longer recording than ours. Inheriting those samples is
    //     what teleports the car 868 m after the line. The honest answer is to
    //     DROP them, which is what --trim-tail does; the record then describes
    //     exactly the run it claims.
    //
    // Nothing here inherits donor bytes unless a human asks for it in so many
    // words with --inherit-outside, and that choice is printed on its own line
    // so it lands in the manifest and in any log anybody reads later.
    let race_end: i64 = args
        .iter()
        .position(|a| a == "--race")
        .and_then(|i| args.get(i + 1))
        .and_then(|v| v.parse().ok())
        .or(o.sim_time)
        .unwrap_or(-1);
    let allow_partial = args.iter().any(|a| a == "--allow-partial");
    let trim = args.iter().any(|a| a == "--trim-outside" || a == "--trim-tail");
    let inherit = args.iter().any(|a| a == "--inherit-outside");
    let in_race = |t: i64| -> bool { t >= 0 && (race_end < 0 || t <= race_end) };
    let miss_in: Vec<i64> = times
        .iter()
        .map(|t| *t as i64)
        .filter(|t| in_race(*t) && !by_ms.contains_key(t))
        .collect();
    let miss_out: Vec<i64> = times
        .iter()
        .map(|t| *t as i64)
        .filter(|t| !in_race(*t) && !by_ms.contains_key(t))
        .collect();
    if race_end < 0 {
        println!(
            "WARNING: no finish time known (no --race and the clean run reported none) -- \
             every recorded instant is being treated as inside the race"
        );
    }
    if !miss_in.is_empty() && !allow_partial {
        println!(
            "ABORT: {} of {} recorded instants INSIDE THE RACE have no engine instant \
             ({} .. {} ms, finish {} ms). Regenerating these would leave the donor's \
             telemetry in place and return success -- that is the contamination defect. \
             No file written.",
            miss_in.len(),
            times.len(),
            miss_in.first().unwrap(),
            miss_in.last().unwrap(),
            race_end
        );
        println!(
            "  Usually the handover: the shim cannot hand over before its checkpoint, so a \
             record that starts earlier than the handover cannot be fully regenerated. \
             Lower the checkpoint ladder, or accept that this carrier cannot serve this run."
        );
        println!("  --allow-partial overrides this and WILL produce a part-carrier file.");
        std::process::exit(3);
    }
    if !miss_out.is_empty() && !trim && !inherit {
        println!(
            "ABORT: {} recorded instants lie OUTSIDE the race ({} .. {} ms, finish {} ms) and \
             have no engine instant. They belong to the carrier's own recording, not to this \
             run. No file written.",
            miss_out.len(),
            miss_out.first().unwrap(),
            miss_out.last().unwrap(),
            race_end
        );
        println!("  --trim-outside  drop them, so the record describes exactly this run (recommended)");
        println!("  --inherit-outside  keep the carrier's bytes there, explicitly and on the record");
        std::process::exit(3);
    }
    if allow_partial && !miss_in.is_empty() {
        println!(
            "PARTIAL WRITE REQUESTED: {} in-race instants keep the DONOR's telemetry. \
             This file is part-carrier by explicit request.",
            miss_in.len()
        );
    }
    if inherit && !miss_out.is_empty() {
        println!(
            "INHERITED OUTSIDE THE RACE: {} instants keep the carrier's telemetry by explicit request.",
            miss_out.len()
        );
    }
    // arm `r165`: the layout probe reports the SAME quaternion offset every time
    // (-16) but its KIND flips between runs on 165922 -- three of nine files came
    // out with (x,y,z,w) where the other six, and the human recording of the same
    // spawn, say (w,x,y,z). Positions are unaffected, so no gate check sees it and
    // the car simply faces the wrong way for the whole render. --quat-kind pins it.
    let mut o = o;
    if let Some(k) = flag("--quat-kind") {
        let k: u8 = k.parse().expect("--quat-kind 0|1|2");
        if k != o.quat_kind {
            println!("QUAT KIND pinned to {} (the probe said {})", k, o.quat_kind);
        }
        o.quat_kind = k;
    }
    let mut trimmed = 0usize;

    let mut w = written_bytes(ss, !keepx, neutral);
    if tape { w[14] = true; w[15] = true; w[18] = true; }
    let res = rewrite_ghost(&c.template, &outp, |rd| {
        let ent = rd
            .ents
            .iter_mut()
            .filter(|e| e.sample_size >= 100 && !e.times.is_empty())
            .max_by_key(|e| e.times.len())
            .ok_or("no vehicle entity")?;
        let ss = ent.sample_size;
        if trim {
            // Drop every instant with no engine data behind it, rather than
            // let the donor speak for it. Times and raw bytes move together.
            let keep: Vec<bool> =
                ent.times.iter().map(|t| by_ms.contains_key(&(*t as i64))).collect();
            trimmed = keep.iter().filter(|k| !**k).count();
            if trimmed > 0 {
                let mut nt: Vec<i32> = Vec::with_capacity(ent.times.len() - trimmed);
                let mut nr: Vec<u8> = Vec::with_capacity((ent.times.len() - trimmed) * ss);
                for (i, k) in keep.iter().enumerate() {
                    if *k {
                        nt.push(ent.times[i]);
                        nr.extend_from_slice(&ent.raw[i * ss..(i + 1) * ss]);
                    }
                }
                ent.times = nt;
                ent.raw = nr;
            }
        }
        for i in 0..ent.times.len() {
            let ms = ent.times[i] as i64;
            let Some((fst, lst)) = by_ms.get(&ms) else {
                missing.push(ms);
                continue;
            };
            let s = &mut ent.raw[i * ss..(i + 1) * ss];
            let gq = |b: &[u8], o: usize| f32::from_le_bytes(b[o..o + 4].try_into().unwrap()) as f64;
            // The orientation, in whichever form this process turned out to
            // hold it: an (x,y,z,w) quaternion, a (w,x,y,z) one, or an
            // orthonormal 3x3. The record always wants (x,y,z,w).
            let q = match o.quat_kind {
                0 => [
                    gq(fst, o.quat_off),
                    gq(fst, o.quat_off + 4),
                    gq(fst, o.quat_off + 8),
                    gq(fst, o.quat_off + 12),
                ],
                2 => {
                    let mut m = [0.0f64; 9];
                    for k in 0..9 {
                        m[k] = gq(fst, o.quat_off + k * 4);
                    }
                    tmtraj::recwrite::mat_to_quat_pub(&m)
                }
                _ => [
                    gq(fst, o.quat_off + 4),
                    gq(fst, o.quat_off + 8),
                    gq(fst, o.quat_off + 12),
                    gq(fst, o.quat_off),
                ],
            };
            let x = tmtraj::recwrite::Xform {
                pos: [
                    f32::from_le_bytes(fst[o.pos_off..o.pos_off + 4].try_into().unwrap()),
                    f32::from_le_bytes(fst[o.pos_off + 4..o.pos_off + 8].try_into().unwrap()),
                    f32::from_le_bytes(fst[o.pos_off + 8..o.pos_off + 12].try_into().unwrap()),
                ],
                quat: q,
                vel: [
                    gq(fst, o.vel_off),
                    gq(fst, o.vel_off + 4),
                    gq(fst, o.vel_off + 8),
                ],
            };
            if !keepx {
                tmtraj::recwrite::write_transform(s, 47, &x);
            }
            if neutral {
                neutralise(s);
            }
            if tape {
                // The input echo, from the tape itself. These three bytes are
                // the run's own inputs -- the one part of a sample that needs
                // no engine reading at all -- and writing them makes the file
                // self-consistent with the tape it carries. (Stated plainly for
                // the audit's sake: the published detector reads exactly these
                // bytes, so a regenerated file scores perfectly on it BY
                // CONSTRUCTION and that score is no longer evidence about the
                // rest of the sample. The per-field table is.)
                let t = (ms - f.start_offset_ms as i64) / 10 + ishift;
                if t >= 0 && (t as usize) < f.steer.len() {
                    let t = t as usize;
                    // FLOOR, and 254, both measured against the corpus rather
                    // than derived. This used to be
                    // `round((steer/127 + 1) / 2 * 255)`, which is the same
                    // expression with a ROUND, and the difference is not
                    // cosmetic: at steer 0 the exact value is 127.5, so a round
                    // gives 128 where the game writes 127. Steer 0 and steer 60
                    // are the two values it gets wrong, and steer 0 is most of a
                    // real tape -- measured on a regenerated map-2 ghost,
                    // `ghost verify` V6 read kappa 0.467 with the round and
                    // 1.000 with the floor, on the same 455 samples.
                    //
                    // The defect was invisible because the only consumer that
                    // reads these bytes is the contamination detector, and a
                    // file that fails it looks like a file with a contaminated
                    // RECORD rather than one with a mis-encoded ECHO.
                    let st = (f.steer[t] as i8) as i32;
                    s[14] = (((st + 127) * 255 / 254) as u8).min(255);
                    s[15] = if f.accel[t] != 0 { 255 } else { 0 };
                    s[18] = if f.brake[t] != 0 { 255 } else { 0 };
                }
            }
            done += 1;
        }
        Ok(())
    });
    match res {
        Ok((a, b)) => println!(
            "wrote {} : {} of {} samples regenerated ({} had no engine instant, {} dropped), record {} -> {} B",
            outp,
            done,
            times.len(),
            missing.len(),
            trimmed,
            a,
            b
        ),
        Err(e) => {
            println!("ABORT: rewrite: {}", e);
            std::process::exit(3)
        }
    }
    // THE COVERAGE ASSERTION. Everything above reasons about what the clean run
    // sampled; this counts what was actually WRITTEN, after the loop, and is
    // the only statement that cannot be wrong about the file on disk. Coverage
    // is samples_regenerated / samples_in_record -- a per-FILE fraction, never
    // files-per-corpus, which is how a pass shipped 36 part-carrier files whose
    // corpus-level count looked healthy.
    let n_final = done + missing.len();
    let cov = if n_final == 0 { 0.0 } else { 100.0 * done as f64 / n_final as f64 };
    println!("COVERAGE: {} of {} samples in the written record are ours ({:.2} %)", done, n_final, cov);
    if !missing.is_empty() && !allow_partial && !inherit {
        println!(
            "ABORT: {} samples in the WRITTEN file still carry the donor's telemetry \
             ({} .. {} ms). The file has been written and is NOT publishable; \
             this is a bug in the coverage reasoning above, not a permitted outcome.",
            missing.len(),
            missing.first().unwrap(),
            missing.last().unwrap()
        );
        std::process::exit(3);
    }
    println!(
        "bytes written per sample: {} of {} ({} left as the carrier's)",
        w.iter().filter(|b| **b).count(),
        ss,
        w.iter().filter(|b| !**b).count()
    );
    if !missing.is_empty() {
        println!(
            "  missing instants: {} .. {} ({} total)",
            missing.first().unwrap(),
            missing.last().unwrap(),
            missing.len()
        );
    }
    Ok(())
}

/// The engine flags, in the flat form the recorder threads through its ladder.
///
/// `fk regen` is the one command that does NOT go through the shared
/// `--tape`/`--at` parsing: it takes `--template`, it runs that file verbatim,
/// and it chooses its own checkpoints from a ladder rather than being told one.
/// Both differences are real, and `ghost regen` depends on this exact contract.
fn parse_ctx(a: &[String]) -> Result<crate::session::Ctx, String> {
    let flag = |n: &str| -> Option<String> {
        a.iter().position(|x| x == n).and_then(|i| a.get(i + 1)).cloned()
    };
    let template = flag("--template").ok_or("--template FILE is required")?;
    let map = flag("--map").ok_or("--map FILE is required")?;
    let work = flag("--work").unwrap_or_else(|| {
        crate::session::Engine::default_work().to_string_lossy().into()
    });
    let shim = flag("--shim")
        .or_else(|| std::env::var("FK_SHIM").ok())
        .or_else(|| {
            let p = std::env::current_exe().ok()?.parent()?.join("libfkshim.so");
            p.exists().then(|| p.to_string_lossy().into())
        })
        .ok_or("no --shim: pass one, set FK_SHIM, or put libfkshim.so next to fk")?;
    let server = flag("--server")
        .or_else(|| std::env::var("TM_SERVER").ok())
        .unwrap_or_else(|| "/tmp/tmoracle/server".into());
    Ok(crate::session::Ctx {
        template,
        map,
        server,
        work,
        shim,
        ckpt: flag("--ckpt").and_then(|v| v.parse().ok()).unwrap_or(0),
    })
}
