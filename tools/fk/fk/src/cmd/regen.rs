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
use gbx::recwrite::rewrite_ghost;

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
    // THE CARRIER BYTES, IN THE SAME ENGINE RUN.
    //
    // `--carrier TABLE` writes the sample bytes named in a frozen carrier table
    // (see `tools/fk/CARRIER.md`) from the SAME gathered state the transform is
    // written from. It is one flag on the one invocation rather than a second
    // pass, and that is not tidiness: a second pass has to identify the car all
    // over again, and on a file whose transform has just been regenerated the
    // recorded positions it would identify it BY are the ones the first pass
    // wrote, so the copy it finds is whichever copy the first pass read. Doing
    // both from one gather removes that question instead of answering it.
    // THE TRANSFORM FROM THE FIELD COPY: measured, half confirmed, OFF.
    //
    // The copy that holds the fields is also the copy the GAME records, and
    // reading the transform from it instead of from the located copy is a
    // correctness fix in two ways -- it removes half a millimetre of position
    // error that this project had been calling a "client-vs-server floor", and
    // it stops a sample pairing a position from one object with a wheel angle
    // from another a tick away.
    //
    // MEASURED on map 2 against the game's own recording of the same run:
    //
    //     position  worst separation 0.001 m -> 0.000 m, and the count of
    //               samples reproducing the recorded bytes EXACTLY goes from
    //               0 of 455 to 227 of 455. Byte 47 goes 57 -> 335, byte 51
    //               0 -> 396, byte 55 27 -> 350.
    //     orientation  WORSE. Bytes 59-64 go from 237/222/453/209/452 of 455
    //               identical to 2/8/1/3/5. Both quaternion orders were tried
    //               and score the same, so it is not the (w,x,y,z) flip; the
    //               quaternion is simply not at the anchor's relative offset on
    //               this copy, and nothing here has found where it is.
    //
    // Half a confirmation is not a confirmation, and this is the publish path,
    // so it is a flag rather than the default until the orientation is found.
    // The fields do NOT depend on it: they come from the right copy either way.
    let xform_from_fields = args.iter().any(|a| a == "--transform-from-fields");
    let carrier = flag("--carrier")
        .map(|t| crate::carrier::read_table(&t).unwrap_or_else(|e| crate::die(e)))
        .unwrap_or_default();
    // Extra ground for the carrier fields, and the cap that keeps it inert.
    //
    // The production window is `car-192 .. car+256` and the table reaches
    // `car+344`, so one more segment is gathered right after it — contiguous in
    // memory, so a record offset stays `car_off + rel` with no per-segment
    // arithmetic. `copy_scan_hi` then holds the LIVE-COPY SEARCH to the
    // production window, so the copy the transform comes from is chosen from
    // exactly the candidates it is chosen from today. Extra ground must not
    // move the transform, and this is what makes that true rather than hoped.
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
    // Which anchor the clean run actually used, so a second gather can be
    // centred the same way rather than searching again.
    let mut used_anchor: Option<crate::record::Anchors> = None;
    for (i, a) in anchors.iter().enumerate() {
        let g = crate::record::GatherOpts {
            segs_rel: &segs_rel,
            bias_override: Some(bias),
            anchors: Some(a),
            period,
            phase_ms: phase,
            dump: &dump,
            verbose,
            ..crate::record::GatherOpts::production(&dump)
        };
        match run_clean_anch(&c, &g) {
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
                used_anchor = Some(*a);
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
        let g = crate::record::GatherOpts {
            segs_rel: &segs_rel,
            bias_override: Some(bias),
            anchors: None,
            period,
            phase_ms: phase,
            dump: &dump,
            verbose,
            ..crate::record::GatherOpts::production(&dump)
        };
        match run_clean_anch(&c, &g) {
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
    // `--pair-shift-ms N` MOVES THE PAIRING, AND IT IS NOT THE OLD `--recshift`.
    //
    // The old flag was removed for a good reason and this one exists for a
    // better one. `--recshift` was driven by C11b, which reports a MAGNITUDE:
    // it cannot see which side of the tick a file is on, a downloaded human
    // ghost the game recorded itself reads the same, and nine files were
    // rebuilt on that reading before the control caught it. The lesson stands.
    //
    // What is different now is the measurement. `ghost phase` regenerates a
    // DOWNLOADED recording -- one the game made itself, so it is on the game's
    // phase by definition -- and decomposes the residual along and across the
    // direction of travel. On five maps it comes back as displacement along
    // the track and nothing across it:
    //
    //     267859  +0.1357 m along, 0.0067 across  =  +9.83 ms
    //     279209  +0.3603 m along, 0.0066 across  =  +9.72 ms
    //     279218  +0.3652 m along, 0.0077 across  =  +9.67 ms
    //     285268  +0.5876 m along, 0.0115 across  =  +9.96 ms
    //     228607  -0.0000 m along, 0.0004 across  =  -0.00 ms   (a clean map)
    //
    // That is not a magnitude and it is not ambiguous: it is a SIGNED time,
    // one physics tick, on the same curve, with the cross-track component at
    // the position encoder's floor. The regenerated record is sampled a tick
    // LATE, so each record instant must pair with an engine instant one tick
    // EARLIER.
    //
    // It stays a flag rather than a constant because the offset is per map --
    // eight of thirteen maps measure zero -- and because the only honest way
    // to set it is to measure the control on THAT map and check the correction
    // returns the control to zero. `ghost phase` prints the value to pass.
    let pair_shift: i64 = flag("--pair-shift-ms").unwrap_or_else(|| "0".into()).parse().unwrap_or(0);
    let by_ms: std::collections::HashMap<i64, (&Vec<u8>, &Vec<u8>)> =
        recs.iter().map(|(c, f, l)| (*c as i64 - bias + pair_shift, (f, l))).collect();

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
    // WHICH BYTES THE CARRIER CLAIMS. In table mode that is one per row. In
    // LAYOUT mode there are no rows -- the sentinel stands for "the writer's
    // whole transcription" -- so the mask has to come from what the gather
    // actually returned, which it does below once `carrier_vals` is in hand.
    // Marking only the sentinel's byte 0 here left every other channel unmarked
    // and therefore unwritten, and the acceptance gate then correctly refused a
    // file whose wheel rotations were all zero.
    let layout_mode = carrier.len() == 1 && carrier[0].rel == i64::MIN;
    if !layout_mode {
        for r in &carrier {
            match r.ch {
                crate::carrier::Channel::Byte(b) if b < ss => w[b] = true,
                crate::carrier::Channel::U16(b) if b + 1 < ss => {
                    w[b] = true;
                    w[b + 1] = true;
                }
                _ => {}
            }
        }
    }
    // THE CARRIER FIELDS, from a second gather paired to this one by the race
    // clock. See `carrier::gather_fields` for why it is a second gather and why
    // that is what removes the "regenerate the transform first" ordering rule.
    //
    // Computed BEFORE anything is written, so a dead read is refused rather
    // than deleted afterwards -- and it has to be refused, because a file full
    // of zeroed wheels passes the entire acceptance gate. None of these bytes
    // affects the simulation.
    let mut carrier_vals: std::collections::HashMap<i64, crate::cmd::carrier::Instant> =
        Default::default();
    if !carrier.is_empty() {
        // WHICH ANCHOR. The field gather only has to put the car somewhere in a
        // 1.25 MB window and then finds it there, so it is far less fussy than
        // the clean run -- an anchor the clean run REJECTED (its self-check is
        // a structural test on a 452-byte window) still serves perfectly well.
        // So: the anchor the clean run used if there was one, then every other
        // candidate, and a fresh measurement if the run came in on --noanchor
        // and there are none.
        let mut field_anchors: Vec<crate::record::Anchors> = Vec::new();
        if let Some(a) = used_anchor {
            field_anchors.push(a);
        }
        for a in &anchors {
            if !field_anchors.iter().any(|x| x.pos_delta == a.pos_delta) {
                field_anchors.push(*a);
            }
        }
        if field_anchors.is_empty() {
            for t in &ticks {
                if let Ok(mut b) = crate::record::measure_anchors(&c, &f, *t, verbose) {
                    for x in b.iter_mut() {
                        x.bias = bias;
                    }
                    field_anchors.append(&mut b);
                }
            }
            field_anchors.dedup_by_key(|a| a.pos_delta);
            println!("--carrier: {} anchors measured for the field gather", field_anchors.len());
        }
        if field_anchors.is_empty() {
            println!("ABORT: no anchor at all for the field gather. No file written.");
            std::process::exit(3);
        }
        // The trajectory the clean run just measured, per millisecond. This is
        // the reference the field gather identifies the car against -- the
        // engine's own answer, not the file's, which is what makes this work on
        // a transplanted container.
        let truth: std::collections::HashMap<i64, [f64; 3]> = recs
            .iter()
            .map(|(clk, fst, _)| {
                let g = |k: usize| {
                    f32::from_le_bytes(
                        fst[o.pos_off + k * 4..o.pos_off + k * 4 + 4].try_into().unwrap(),
                    ) as f64
                };
                (*clk as i64 - bias, [g(0), g(1), g(2)])
            })
            .collect();
        // The recording's own orientation, when the container carries this
        // run's. Reported against, never chosen by -- see `gather_fields`.
        //
        // AND ONLY WHEN IT REALLY IS THIS RUN'S. On a transplanted container
        // the recorded samples are the DONOR's driving, so this "answer key" is
        // a different car on a different line -- and the veto in
        // `gather_fields` then refuses the correct orientation for disagreeing
        // with a stranger. Measured on 276874: the ranking's top candidate is
        // 3.02957 rad from the container's recording, which is 174 degrees --
        // not a near miss, a different run.
        //
        // The clean run has just measured THIS run's own positions per
        // millisecond, so the test is cheap and needs no recording: if the
        // container's recorded path does not follow the path the engine just
        // drove, its orientation is not an answer key either. One metre is far
        // wider than any pairing error (the copies sit half a millimetre apart)
        // and far tighter than a different route.
        let key_is_ours = {
            let mut n = 0usize;
            let mut off = 0usize;
            for (i, t) in times.iter().enumerate() {
                let Some(p) = truth.get(t) else { continue };
                let r = gbx::record::read_transform_pub(&raws[i], 47).0;
                let d = ((r[0] - p[0]).powi(2) + (r[1] - p[1]).powi(2) + (r[2] - p[2]).powi(2))
                    .sqrt();
                n += 1;
                if d > 1.0 {
                    off += 1;
                }
            }
            let ours = n >= 20 && (off as f64) < 0.1 * n as f64;
            if !ours {
                println!(
                    "--carrier: the container's own recording is NOT this run ({} of {} \
                     instants more than 1 m from the path the engine just drove), so its \
                     orientation is not an answer key and the veto is disabled. The \
                     reference-free ranking decides alone, as it must on the transplanted \
                     containers this exists for.",
                    off, n
                );
            }
            ours
        };
        let truth_q: std::collections::HashMap<i64, [f64; 4]> = if key_is_ours {
            times
                .iter()
                .enumerate()
                .map(|(i, t)| (*t, gbx::record::read_transform_pub(&raws[i], 47).1))
                .collect()
        } else {
            std::collections::HashMap::new()
        };
        let (gp, gph) = grid_of(&times);
        let fdump = format!("{}.fields", dump);
        let mut last = String::new();
        for a in &field_anchors {
            match crate::cmd::carrier::gather_fields(
                &c, a, &carrier, &truth, &truth_q, gp, gph, &fdump, 1_048_576, 262_144, verbose,
            ) {
                Ok(v) => {
                    carrier_vals = v;
                    break;
                }
                Err(e) => {
                    println!("field gather, anchor base{:+}: {}", a.pos_delta, e);
                    last = e;
                }
            }
        }
        let _ = std::fs::remove_file(&fdump);
        if carrier_vals.is_empty() {
            println!("ABORT: the carrier fields could not be read: {}. No file written.", last);
            std::process::exit(3);
        }
        // In layout mode the claimed bytes are whatever the writer's
        // transcription produced -- see the note at `w`'s construction.
        if layout_mode {
            if let Some(v) = carrier_vals.values().next() {
                for (ch, _) in &v.fields {
                    if let crate::carrier::Channel::Byte(b) = ch {
                        if *b < ss {
                            w[*b] = true;
                        }
                    }
                }
                println!(
                    "--carrier layout: {} of {} sample bytes come from the writer's own \
                     transcription",
                    v.fields.len(),
                    ss
                );
            }
        }
        let mut seen: std::collections::BTreeMap<String, std::collections::BTreeSet<u32>> =
            Default::default();
        for (t, v) in &carrier_vals {
            if !in_race(*t) {
                continue;
            }
            for (ch, x) in &v.fields {
                seen.entry(ch.name()).or_default().insert(*x);
            }
        }
        // VARIANCE, NOT VALUE -- AND ONLY WHERE STILLNESS IS IMPOSSIBLE.
        //
        // Zero is a legal wheel angle at one instant; what no driven run does is
        // hold one for the length of a lap. But that argument is only available
        // for the WHEEL ROTATIONS: turbo_time is legitimately constant on a map
        // with no turbo (measured: it refused human_22730 on exactly that), gear
        // can hold on a short run, and a surface byte holds whenever the car
        // never leaves one surface. Refusing those would be a check that fires
        // on correct files, which costs more than the defect it guards.
        //
        // So the guard is armed on the four channels whose deadness diagnoses a
        // wrong copy, and the rest are reported.
        let must_move = |n: &str| matches!(n, "u16@6" | "u16@8" | "u16@10" | "u16@12");
        let dead: Vec<&String> = seen
            .iter()
            .filter(|(k, v)| v.len() <= 1 && must_move(k))
            .map(|(k, _)| k)
            .collect();
        if !dead.is_empty() {
            println!(
                "ABORT: the wheel rotations {:?} came out CONSTANT over the run -- the gathered \
                 slots are dead memory, and nothing downstream would catch it because these \
                 bytes do not affect the simulation. No file written.",
                dead
            );
            std::process::exit(3);
        }
        let resting: Vec<&String> =
            seen.iter().filter(|(_, v)| v.len() <= 1).map(|(k, _)| k).collect();
        if !resting.is_empty() {
            println!(
                "  {:?} never move on this run -- written, and legitimately constant (no turbo, \
                 one surface, one gear)",
                resting
            );
        }
        println!(
            "carrier: {} channels over {} instants, fewest distinct values {}",
            seen.len(),
            carrier_vals.len(),
            seen.values().map(|v| v.len()).min().unwrap_or(0)
        );
    }

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
            // The engine writes the vehicle state twice inside one tick and
            // which of the two the game's own recorder captured is a
            // measurable question per field, not a matter of taste (measured:
            // steer wants the FIRST write, suspension the last). The transform
            // encoder uses the first; nothing here writes a last-write field
            // any more, so `_lst` is carried rather than dropped so the pairing
            // stays visible to whoever adds one.
            let Some((fst, _lst)) = by_ms.get(&ms) else {
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
                    gbx::recwrite::mat_to_quat_pub(&m)
                }
                _ => [
                    gq(fst, o.quat_off + 4),
                    gq(fst, o.quat_off + 8),
                    gq(fst, o.quat_off + 12),
                    gq(fst, o.quat_off),
                ],
            };
            // THE TRANSFORM COMES FROM THE SAME OBJECT AS THE FIELDS.
            //
            // With `--carrier`, from the copy the field gather identified: the
            // one with a live wheel block, which is the copy the GAME ITSELF
            // records. Without it, from the clean run's own window as before.
            //
            // This is not a refinement, it is a correctness fix in two ways.
            // The copies sit half a millimetre apart -- one tick's travel --
            // so writing a position from one and a wheel angle from the other
            // is a pure time shift between two channels of one sample. And the
            // half-millimetre itself was being read as physics: three maps
            // regenerate to 0.489, 0.511 and 0.501 mm of their own recordings
            // and that agreement was quoted as a "client-vs-server floor". It
            // is the distance between these two structs, and it is the same on
            // three maps because it is one quantity measured three times.
            let ci = if xform_from_fields { carrier_vals.get(&ms) } else { None };
            let x = match ci {
                Some(v) => gbx::recwrite::Xform { pos: v.pos, quat: v.quat, vel: v.vel },
                None => gbx::recwrite::Xform {
                    pos: [
                        f32::from_le_bytes(fst[o.pos_off..o.pos_off + 4].try_into().unwrap()),
                        f32::from_le_bytes(fst[o.pos_off + 4..o.pos_off + 8].try_into().unwrap()),
                        f32::from_le_bytes(fst[o.pos_off + 8..o.pos_off + 12].try_into().unwrap()),
                    ],
                    quat: q,
                    vel: [gq(fst, o.vel_off), gq(fst, o.vel_off + 4), gq(fst, o.vel_off + 8)],
                },
            };
            if !keepx {
                gbx::recwrite::write_transform(s, 47, &x);
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
            // THE CARRIER BYTES, from the same instant as the transform.
            //
            // `fst`, not `lst`, and not because first is generally right: it is
            // right because the transform above is written from `fst`, and a
            // sample is ONE INSTANT of one car. Reading a wheel angle from the
            // other write of the tick would pair a position with a suspension
            // state a fraction of a tick apart -- a defect that looks like
            // nothing, since neither byte affects the simulation.
            for (ch, v) in ci.map(|v| v.fields.as_slice()).unwrap_or(&[]) {
                match ch {
                    crate::carrier::Channel::Byte(b) if *b < ss => s[*b] = *v as u8,
                    crate::carrier::Channel::U16(b) if *b + 1 < ss => {
                        s[*b] = *v as u8;
                        s[*b + 1] = (*v >> 8) as u8;
                    }
                    _ => {}
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
    // NAME what is still not ours, by number. "42 left as the carrier's" is a
    // count, and a count is what let a published clip run on dirt tyres because
    // bytes 93/95/97/99 were the donor's -- nobody could see WHICH bytes those
    // were without reading the source. With --neutralise the unwritten ones are
    // zeros rather than a stranger's run, which is honest and is not the same
    // as correct: a zero is an absence, not a measurement, and it should be
    // possible to say so on a page.
    let unwritten: Vec<usize> = (0..ss).filter(|b| !w[*b]).collect();
    if !unwritten.is_empty() {
        println!(
            "  NOT WRITTEN (zeroed by --neutralise, or the carrier's without it): {:?}",
            unwritten
        );
    }
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
        .or_else(|| crate::session::default_shim().map(|p| p.to_string_lossy().into()))
        .ok_or("no --shim: pass one, set FK_SHIM, or build tools/search (which produces \
              libforkshim.so)")?;
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
