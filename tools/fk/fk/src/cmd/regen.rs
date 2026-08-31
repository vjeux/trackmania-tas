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
    // The ladder is shared with `fk ptr`, which needs the same checkpoints for
    // the same reasons; the reasoning lives on `record::ladder_ticks`.
    let mut ticks: Vec<i64> = crate::record::ladder_ticks(n, bt);
    if let Some(s) = flag("--anchorticks") {
        ticks = s.split(',').filter_map(|v| v.trim().parse().ok()).collect();
        ticks.retain(|t| *t >= 60 && *t < n - 20);
        ticks.dedup();
    }
    let mut anchors: Vec<crate::record::Anchors> = Vec::new();
    // THE VALIDATOR'S CAR: RESOLVED, VALIDATED, AND STILL NOT USABLE HERE.
    // Tried and reverted; `anchors_from_validator` stays in record.rs unused.
    //
    // The route works. On 287431 -- where every scene chain fails --
    // `validator.rs` resolves base-4879052 at every tick, identically, by
    // named hops with a class-id check, and `qualify2` accepts that address
    // inside `ValidatorCar::resolve`: verr and qerr both pass, on a 40-byte
    // window from pos-16 that is quaternion, position, velocity in that order.
    //
    // Handing the same address to the CLEAN RUN as an anchor fails, and fails
    // the same way with quat_kind 0 or 1:
    //
    //     self-check: median |d(pos)/dt - v| is 277.79 m/s at median speed
    //     277.8 -- the sampled window is not the vehicle state
    //
    // |d(pos)/dt - v| equalling the speed exactly means the velocity reads as
    // ZERO in that window. So the same address that carries a live velocity in
    // the probe process carries zeros in the clean run's gather -- a real
    // difference between the two contexts, not a layout guess. Whoever picks
    // this up should start there: dump the 40 bytes at pos-16 in BOTH
    // processes and diff them, rather than trying more quat_kind values.
    if !noanchor && std::env::var("FK_ANCHOR_SERVER").is_err() {
        let mut chains: Vec<String> = crate::ptr::chain_cache_get(&c.server, &c.map);
        if let Ok(v) = std::env::var("FK_CAR_CHAIN") {
            chains = vec![v];
        } else {
            for s in crate::ptr::CAR_CHAINS {
                if !chains.iter().any(|x| x == s) {
                    chains.push(s.to_string());
                }
            }
        }
        // EVERY POOL MEMBER, not just the first. The chain names the engine's
        // vehicle ARRAY and which element is the driven car varies by process
        // and by map -- 203072 answers at member 0, and a map that answers at
        // member 2 would look exactly like a stale chain if only member 0 were
        // tried. `resolve_in` fails cleanly for a member the pool does not
        // have, so an over-long list costs nothing but a skipped entry.
        let members: usize = std::env::var("FK_POOL_MEMBERS")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(2);
        for ch in &chains {
            for m in 0..members {
                let mut a = crate::record::Anchors::from_chain(0, 0, ch);
                a.member = m;
                anchors.push(a);
            }
        }
        println!("{} chain(s) to try, resolved in the clean run itself", anchors.len());

    }
    // Did the anchors come from the on-disk cache? If so a total failure below
    // is a cache miss, not a dead end -- see the fallback after the clean run.
    // The BIAS first, on its own: the clock scan is far more robust than the
    // position locate (its signature is "+10 every tick, no exceptions"), and
    // the bias is what labels every sample. Getting it from a mid-tape
    // checkpoint, where the page-fault probe's tick estimate is exact, is what
    // keeps the early handover usable.
    // THE BIAS COMES FREE WITH THE ANCHORS. It used to be measured here, in a
    // server started for that alone -- ~2 s of a 10.9 s run -- and then
    // measured AGAIN by the clock scan inside every later server start, which
    // reports the identical value ("CLOCK ... bias +2200 ms" appears in each).
    // `measure_anchors` already carries `ck.bias` out on every anchor it
    // returns, so take it from there and start one fewer engine.
    // The bias for this (binary, map), if it has ever been measured. See
    // ptr::bias_cache_get -- it is a scalar property of the pair, not an
    // address, and a stale one aborts the run instead of writing a bad file.
    let mut bias = crate::ptr::bias_cache_get(&c.server, &c.map).unwrap_or(0);
    if bias != 0 {
        println!("bias {} (cached for this binary and map)", bias);
    }
    // THE ANCHORS MUST CARRY THE BIAS. The clean run takes it from
    // `bias_override`, so a zero here is invisible there -- but
    // `gather_fields` reads `a.bias` and a zero sends it to the blind 1.25 MB
    // window, which then fails with "no copy in the field window holds the
    // trajectory the clean run measured". That cost a 2.75 s run a 264 s abort.
    // Stamped again after the miss-path measurement below.
    for a in anchors.iter_mut() {
        a.bias = bias;
    }
    // On a cache MISS the bias is measured once, in a server started for that
    // alone, and written back below -- so this costs a map its first run and
    // nothing after.
    if bias == 0 && !noanchor {
        for t in &ticks {
            if let Ok(b) = crate::record::measure_bias(&c, &f, *t, verbose) {
                bias = b;
                println!("bias {} (measured at tick {}; caching it)", b, t);
                crate::ptr::bias_cache_put(&c.server, &c.map, b);
                for a in anchors.iter_mut() {
                    a.bias = b;
                }
                break;
            }
        }
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
    // `--anchor bias:pos:clock:q:kind:vel` is GONE. It named the car by an
    // offset from the module base, which is only ever valid in the process
    // that measured it -- the car is on the heap. Name it with a CHAIN
    // instead: --car-chain, or FK_CAR_CHAIN, resolved fresh every run.
    if anchors.is_empty() && !noanchor {
        // PAY FOR THE SEARCH ONCE. Each `measure_anchors` starts a server and
        // runs a full `locate_candidates` -- ~7.5 s -- and this loop runs it
        // per anchor tick. Measured on 203072: four ticks, 9 s + 18 s + 9 s +
        // 9 s, and THREE OF THE FOUR return the identical address
        // (base-3453700). That is 45 s of a 50 s regen spent finding one
        // number four times.
        //
        // ONE ANCHOR, FROM THE POINTER. There is no reuse-across-ticks dance
        // any more and no "collect from every checkpoint in case this one is a
        // decoy": `measure_anchors` resolves a chain from static data and
        // cannot return a decoy. It is also cheap -- a clock scan and a
        // handful of pointer reads -- so the first tick that answers is the
        // answer.
        for t in &ticks {
            match measure_anchors(&c, &f, *t, verbose) {
                Ok(mut b) => {
                    println!(
                        "anchors from tick {}: {}",
                        t,
                        b.iter()
                            .map(|a| a.chain.clone())
                            .collect::<Vec<_>>()
                            .join(", ")
                    );
                    if let Some(first) = b.first() {
                        bias = first.bias;
                        println!("bias {} (from the anchor clock scan)", bias);
                    }
                    anchors.append(&mut b);
                    // ONE CHECKPOINT IS ENOUGH, because a chain is not a
                    // measurement. The old loop ran `measure_anchors` at every
                    // tick in the ladder and merged the results: the SEARCH
                    // could return a decoy at one tick and the car at another,
                    // so a long list was the only defence. Measured on 203072:
                    // six ticks, ~2.5 s each, every one returning the SAME
                    // nine chains -- 15 of the run's 18 s spent re-deriving an
                    // answer that cannot vary. A chain is resolved from static
                    // data and says the same thing at every tick.
                    break;
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
    anchors.dedup_by_key(|a| (a.chain.clone(), a.member));
    println!("{} distinct anchor candidates to try", anchors.len());
    let mut o = None;
    // Which anchor the clean run actually used, so a second gather can be
    // centred the same way rather than searching again.
    let mut used_anchor: Option<crate::record::Anchors> = None;
    for (i, a) in anchors.iter().enumerate() {
        // TRIED AND REVERTED: gathering the field window HERE to make the
        // second boot redundant (FK_ONE_GATHER). The output stayed correct
        // (md5 eb1b8a7c) and the run took 125.7 s against 5.5 s -- the clean
        // run's copy-selection scans every offset in the window it is given,
        // so widening it from 452 B to 1244 B multiplies that scan by far more
        // than a 2.24 s boot costs. The two gathers want different windows for
        // good reason: one identifies the car, the other transcribes it.
        let g = crate::record::GatherOpts {
            segs_rel: &segs_rel,
            bias_override: if bias == 0 { None } else { Some(bias) },
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
                    Ok(len) => println!("anchor {}#{}: path {:.1} m over the run", a.chain, a.member, len),
                    Err(e) => {
                        println!("anchor {}#{}: REJECTED -- {}", a.chain, a.member, e);
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
                    a.bias, format!("{}#{}", a.chain, a.member), a.clock_delta, a.quat_off, a.quat_kind, a.vel_off
                );
                used_anchor = Some(a.clone());
                // Remember the chain that PASSED for this map. Safe to cache
                // because a chain is resolved fresh in every process; the
                // address it yields never is.
                crate::ptr::chain_cache_put(&c.server, &c.map, &a.chain);
                o = Some(v);
                break;
            }
            Err(e) => println!("anchor {}: {}", a.chain, e),
        }
    }
    // TRIED, NEVER REACHED, REVERTED. `validator.rs` walks a fully
    // disassembled route to a car the VALIDATOR owns -- controller +0x1a70 ->
    // sim +0x18 -> playground +0x660 -> participant +0x1118 -> CGameVehiclePhy
    // +0x12f0, with a class-id check -- and its layout is the anchor's
    // (quaternion 16 B before the position), not the vis state's 3x3. On paper
    // that is exactly what 287431 needs, since the validator's vehicle does
    // not vanish when the map's 646 m freefall hands the scene car from one
    // entity to another.
    //
    // In practice `ValidatorCar::locate` returned nothing on this map at every
    // tick tried, printing not even its error, and the run still fell through
    // to the memory search at 185 s -- and the output md5 changed from
    // 82339ca8 to b5d4f44f, so something in the ordering also disturbed the
    // result. Reverted rather than shipped. `anchors_from_validator` is left
    // in record.rs, unused, for whoever picks this up: the route is right, the
    // reason it finds nothing here is not yet known.
    // THE LIVE CHAIN: follow the car instead of naming it once (FK_LIVE_CHAIN=1).
    //
    // Every other route resolves an address before the run and is defeated by
    // a map that reallocates its vehicle. This hands the shim the validator's
    // walk and has the SAMPLER redo it at every instant.
    if o.is_none() && std::env::var("FK_LIVE_CHAIN").is_ok() {
        match crate::record::validator_live_chain(&c, &f, ticks[0]) {
            Ok((root, offs, tail)) => {
                println!("live chain: root {:#x}, {} hops, tail {:#x}", root, offs.len(), tail);
                let a = crate::record::Anchors {
                    bias,
                    chain: "live".into(),
                    member: 0,
                    clock_delta: 0,
                    speed: 0.0,
                    quat_off: -16,
                    quat_kind: 0,
                    vel_off: 12,
                };
                let g = crate::record::GatherOpts {
                    segs_rel: &segs_rel,
                    bias_override: if bias == 0 { None } else { Some(bias) },
                    anchors: Some(&a),
                    period,
                    phase_ms: phase,
                    dump: &dump,
                    verbose,
                    live_chain: Some((root, offs, tail)),
                    ..crate::record::GatherOpts::production(&dump)
                };
                match run_clean_anch(&c, &g) {
                    // STILL "0 instants sampled" WITH THE PHASE FIXED. The
                    // gate is not the cause: `gate_phase` is now u32::MAX for
                    // a live chain (the shim's own "take the phase from this
                    // process's clock" form) and the count is unchanged.
                    //
                    // The next suspect is the walk landing somewhere
                    // unreadable, which would make the sampler's copy fault
                    // and emit nothing. That is testable directly: have the
                    // shim's 'C' handler walk the chain ONCE at arm time and
                    // report the address it reaches, then compare it against
                    // the base-4879052 the one-shot validator resolve returns
                    // in the same process. Equal means the walk is right and
                    // the fault is in the sampling; different means the hop
                    // list is wrong.
                    Ok(v) => match crate::record::car_path_len(&dump, v.reclen, v.pos_off) {
                        Ok(len) => {
                            println!("LIVE CHAIN WORKS: path {:.1} m over the run", len);
                            used_anchor = Some(a.clone());
                            o = Some(v);
                        }
                        Err(e) => println!("live chain: REJECTED -- {}", e),
                    },
                    Err(e) => println!("live chain: {}", e),
                }
            }
            Err(e) => println!("live chain: {}", e),
        }
    }

    // Last resort: locate in the clean process itself. It cannot see a
    // stationary car, but when the tape is already moving at the handover it
    // needs no cross-process assumption at all.
    if o.is_none() {
        println!("falling back to an in-process locate");
        let g = crate::record::GatherOpts {
            segs_rel: &segs_rel,
            bias_override: if bias == 0 { None } else { Some(bias) },
            anchors: None,
            period,
            phase_ms: phase,
            dump: &dump,
            verbose,
            ..crate::record::GatherOpts::production(&dump)
        };
        match run_clean_anch(&c, &g) {
            Ok(v) => {
                // Do NOT exit here. This is one candidate among several, and
                // the memory-search fallback below is the last resort -- an
                // exit at this point killed the run before it could run.
                match crate::record::car_path_len(&dump, v.reclen, v.pos_off) {
                    Ok(_) => o = Some(v),
                    Err(e) => println!("in-process locate: REJECTED -- {}", e),
                }
            }
            Err(e) => println!("in-process locate: {}", e),
        }
    }
    // A STALE CALIBRATION MUST FALL BACK, NOT ABORT.
    //
    // The car's offset from the module base is not perfectly stable -- one
    // binary and map gave -4012784, -4012672 and -4890252 across runs -- so a
    // cached offset is sometimes for an allocation this process did not make.
    // When that happens every calibrated anchor fails here, and aborting turns
    // a cache miss into a failed regen: measured 3 of 5 single-try runs
    // succeeding, the other 2 dying on exactly this line with the search never
    // attempted.
    //
    // So when the anchors came from the cache and none of them worked, measure
    // properly and try once more. The cost is the search we were trying to
    // skip; the alternative is a run that fails for no reason but a stale
    // hint.
    // (The "stale calibration, measure instead" block that stood here is gone
    // with the calibration cache itself -- `used_calibration` was set false
    // and never set true, so it was unreachable.)
    // TRIED AND REVERTED: retrying the chain anchors from later checkpoints
    // (4200/12000/20000) to get past 287431'"'"'s car reallocation. It never
    // rescued the chain and cost 718 s against 274 s for going straight to the
    // search. The chain is genuinely not usable on that map by this route;
    // whatever  reads over the whole run, the clean run'"'"'s gather
    // does not see from any checkpoint.
    // EVERY CHAIN REJECTED -- fall back to the search.
    //
    // Chains are per (binary, map) and the built-in list does not cover every
    // map yet, so a map nobody has run `fk ptr find` on has no chain that
    // resolves. When that happens the tool should still produce a file: sweep
    // for the car the old way, and let the same acceptance test judge it.
    // This costs ~7.5 s per anchor tick and can need several tries, which is
    // exactly the cost the pointer removes where it works.
    if o.is_none() {
        println!("no pointer chain passed -- falling back to the memory search");
        let mut searched: Vec<crate::record::Anchors> = Vec::new();
        for t in &ticks {
            if let Ok(mut b) = crate::record::measure_anchors_by_search(&c, &f, *t, verbose) {
                for a in b.iter_mut() {
                    a.bias = bias;
                }
                searched.append(&mut b);
            }
        }
        // COLLECT FROM EVERY CHECKPOINT, and do not stop at the first that
        // answers. A searched anchor is a `base±N` offset measured in ANOTHER
        // process, so it is right only when the allocation happened to repeat;
        // one checkpoint's single candidate is a coin toss, and taking the
        // first one cost 287431 its regeneration. A long list is what the
        // acceptance test needs to work with -- this is the fragility the
        // pointer chain removes, preserved here only because the fallback has
        // no better option.
        for a in &searched {
            let g = crate::record::GatherOpts {
                segs_rel: &segs_rel,
                bias_override: if bias == 0 { None } else { Some(bias) },
                anchors: Some(a),
                period,
                phase_ms: phase,
                dump: &dump,
                verbose,
                ..crate::record::GatherOpts::production(&dump)
            };
            match run_clean_anch(&c, &g) {
                Ok(v) => {
                    match crate::record::car_path_len(&dump, v.reclen, v.pos_off) {
                        Ok(len) => println!("searched anchor {}: path {:.1} m", a.chain, len),
                        Err(e) => {
                            println!("searched anchor {}: REJECTED -- {}", a.chain, e);
                            continue;
                        }
                    }
                    used_anchor = Some(a.clone());
                    o = Some(v);
                    break;
                }
                Err(e) => println!("searched anchor {}: {}", a.chain, e),
            }
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
    // What the gather read, per SAMPLE BYTE, so the file can be checked
    // against it once it exists. See the read-back at the end of this command.
    let mut gathered: std::collections::BTreeMap<usize, std::collections::BTreeSet<u8>> =
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
            // This one passed the acceptance test, so it is worth keeping: the
            // next regen of this (binary, map) can skip the ~7.5 s locate per
            // anchor tick. Saved AFTER acceptance, never before.
            field_anchors.push(a);
        }
        for a in &anchors {
            if !field_anchors.iter().any(|x| x.chain == a.chain) {
                field_anchors.push(a.clone());
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
            field_anchors.dedup_by_key(|a| (a.chain.clone(), a.member));
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
        // THE REFERENCE PATH, ON DISK, WHEN ASKED FOR.
        //
        // `truth` is the only thing the field gather identifies the car
        // against, and until now the single number it produced -- a median
        // distance -- was the whole of what could be known about it. A median
        // cannot tell a decoy's path from this run's read at the wrong instant.
        // The path itself can: two CSVs and `tmtraj csvdiff` settle it.
        if let Some(p) = flag("--dump-truth") {
            let mut ks: Vec<i64> = truth.keys().copied().collect();
            ks.sort_unstable();
            let mut s = String::from("race_ms,x,y,z\n");
            for k in ks {
                let v = truth[&k];
                s.push_str(&format!("{},{:.6},{:.6},{:.6}\n", k, v[0], v[1], v[2]));
            }
            match std::fs::write(&p, s) {
                Ok(()) => println!("--dump-truth: the clean run's reference path -> {p}"),
                Err(e) => println!("--dump-truth: could not write {p}: {e}"),
            }
        }
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
            // SAY HOW FAR, NOT JUST HOW MANY.
            //
            // This printed a COUNT ("1142 of 1150 instants more than 1 m") and
            // went on. A count cannot tell the three things that produce it
            // apart: a transplanted container (metres to hundreds of metres,
            // another route), a clock offset (the same route, paired at the
            // wrong instant), and OUR OWN clean run having anchored on a decoy
            // (the container is right and the reference is wrong). On 227654
            // the third is what happened, and the count read exactly like the
            // first -- so the veto was disabled and the field gather went on
            // scoring against a path that is not this run's.
            let mut ds: Vec<f64> = Vec::new();
            let dist = |r: [f64; 3], p: &[f64; 3]| {
                ((r[0] - p[0]).powi(2) + (r[1] - p[1]).powi(2) + (r[2] - p[2]).powi(2)).sqrt()
            };
            for (i, t) in times.iter().enumerate() {
                let Some(p) = truth.get(t) else { continue };
                let r = gbx::record::read_transform_pub(&raws[i], 47).0;
                let d = dist(r, p);
                ds.push(d);
                n += 1;
                if d > 1.0 {
                    off += 1;
                }
            }
            let ours = n >= 20 && (off as f64) < 0.1 * n as f64;
            if !ours && n > 0 {
                let mut s = ds.clone();
                s.sort_by(|a, b| a.total_cmp(b));
                // IS IT A TIME OFFSET? The same route paired at the wrong
                // instant collapses at some shift; a different route does not.
                // The sweep is over WHOLE SAMPLES of the container's own grid,
                // which is what a pairing error moves by.
                let step = if times.len() > 1 { times[1] - times[0] } else { 50 };
                let mut best = (0i64, s[s.len() / 2]);
                for k in -40i64..=40 {
                    let mut e: Vec<f64> = Vec::new();
                    for (i, t) in times.iter().enumerate() {
                        let Some(p) = truth.get(&(t + k * step)) else { continue };
                        e.push(dist(gbx::record::read_transform_pub(&raws[i], 47).0, p));
                    }
                    if e.len() < 20 {
                        continue;
                    }
                    e.sort_by(|a, b| a.total_cmp(b));
                    if e[e.len() / 2] < best.1 {
                        best = (k, e[e.len() / 2]);
                    }
                }
                println!(
                    "--carrier: the container's own recording is NOT this run ({} of {} \
                     instants more than 1 m from the path the engine just drove: median {:.3} m, \
                     p99 {:.3} m, worst {:.3} m, {:.3} m at the first shared instant; best \
                     pairing shift {}{} samples at median {:.3} m), so its \
                     orientation is not an answer key and the veto is disabled. The \
                     reference-free ranking decides alone, as it must on the transplanted \
                     containers this exists for.",
                    off,
                    n,
                    s[s.len() / 2],
                    s[(s.len() * 99) / 100],
                    s[s.len() - 1],
                    ds[0],
                    if best.0 > 0 { "+" } else { "" },
                    best.0,
                    best.1
                );
                if best.0 == 0 {
                    println!(
                        "  NO SHIFT HELPS: this is not a pairing error. Either the container \
                         carries another run (a transplant, which is normal here) or the clean \
                         run anchored on a decoy and `truth` is not this run's path -- and those \
                         two are told apart by whether the container's recording belongs to its \
                         own tape (`ghost verify --engine`)."
                    );
                }
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
        // THE POINTER, FIRST. `fk ptr find` established that the engine keeps a
        // pointer to the vehicle state in the game binary's own static data, so
        // the field gather does not have to hunt for the car in 1.25 MB of
        // engine memory at 260 instants: it reads a pointer, dereferences it,
        // and gathers the 864 bytes of the struct. Measured on 191465, for the
        // same bytes out: 1.36 GB and 11-12 minutes -> a few MB and seconds.
        //
        // Nothing about this is trusted. `gather_fields` applies exactly the
        // same acceptance to a pointer window as to a blind one -- the copy
        // must reproduce the clean run's own measured path and all four wheel
        // slots must be live -- and when it does not, this falls through to the
        // blind window below and says so. A wrong pointer cannot produce a
        // file; it can only cost the time the search would have cost anyway.
        let chain = flag("--car-chain").unwrap_or_else(|| crate::ptr::DEFAULT_CHAIN.to_string());
        let no_chain = args.iter().any(|a| a == "--no-car-chain") || chain.is_empty();
        if !no_chain && xform_from_fields {
            println!(
                "ABORT: --transform-from-fields needs the orientation hunt, which a pointer \
                 window cannot run -- it is the struct itself, and the struct holds a 3x3 \
                 rotation rather than a quaternion. Re-run with --no-car-chain. No file written."
            );
            std::process::exit(3);
        }
        if !no_chain {
            // The spec names the engine's vehicle ARRAY, so every member is
            // gathered and the copy rule below chooses between them -- which
            // element is the live car varies by process, and an index nobody
            // measured is a coin flip that the acceptance test would catch but
            // that would cost the run.
            let resolve = |pid: i32, _base: u64| -> Result<(u64, Vec<(i64, u32)>), String> {
                let (m, _) =
                    crate::ptr::module_base(pid).ok_or("no module base for the live server")?;
                let states = crate::ptr::resolve_pool(pid, m, &chain)?;
                let anchor = states[0] + 0x50;
                let ex = states[1..]
                    .iter()
                    .map(|s| (*s as i64 - anchor as i64, 0x368u32))
                    .collect();
                Ok((anchor, ex))
            };
            let a = field_anchors[0].clone();
            match crate::cmd::carrier::gather_fields(
                &c, &a, &carrier, &truth, &truth_q, gp, gph, &fdump, 0, 0, Some(&resolve), verbose,
            ) {
                Ok(v) => {
                    println!("--carrier: the fields came from the pointer {}", chain);
                    carrier_vals = v;
                }
                Err(e) => println!(
                    "--carrier: the pointer {} did not produce the car ({}); falling back to the \
                     blind window",
                    chain, e
                ),
            }
        }
        for a in &field_anchors {
            if !carrier_vals.is_empty() {
                break;
            }
            match crate::cmd::carrier::gather_fields(
                &c, a, &carrier, &truth, &truth_q, gp, gph, &fdump, 1_048_576, 262_144, None,
                verbose,
            ) {
                Ok(v) => {
                    carrier_vals = v;
                    break;
                }
                Err(e) => {
                    println!("field gather, anchor {}: {}", a.chain, e);
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
                // AND THE SAME THING PER BYTE, for the read-back after the
                // write. `seen` is keyed by channel name and lives on what was
                // GATHERED; the read-back has to name bytes of the file.
                match ch {
                    crate::carrier::Channel::Byte(b) => {
                        gathered.entry(*b).or_default().insert(*x as u8);
                    }
                    crate::carrier::Channel::U16(b) => {
                        gathered.entry(*b).or_default().insert(*x as u8);
                        gathered.entry(*b + 1).or_default().insert((*x >> 8) as u8);
                    }
                }
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
            // WHICH INSTANT'S FIELDS, and — separately — where the TRANSFORM
            // comes from. These were one binding, and that was the bug.
            //
            // `carrier_vals` carries two things: the transform read from the
            // copy that has the fields, and the fields themselves. Only the
            // FIRST is what `--transform-from-fields` chooses. Gating both on
            // the flag meant that without it the carrier bytes below were
            // written from `&[]` — so a run could gather 99 channels, report
            // them, name which of them vary, and write a file in which every
            // one of those bytes is the carrier's untouched constant. It is
            // silent by construction: the gather's own report is made from
            // `carrier_vals`, which is correct, and nothing downstream looks at
            // the file. `--carrier layout` on untitled 01 read live wheel
            // rotation (byte 6 stepping 0, 8, 26 as the car pulls away) and
            // wrote 0x00 on all 256 samples.
            let fields = carrier_vals.get(&ms);
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
            let ci = if xform_from_fields { fields } else { None };
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
            for (ch, v) in fields.map(|v| v.fields.as_slice()).unwrap_or(&[]) {
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
    // THE READ-BACK. Does the FILE hold what the gather read?
    //
    // Every carrier report above is made from `carrier_vals` -- what came out
    // of engine memory -- and none of it is evidence about the bytes on disk.
    // That gap is not theoretical: `ci` was bound to `carrier_vals` only under
    // `--transform-from-fields`, so without that flag the carrier bytes were
    // written from an empty slice, and the command printed "99 channels over
    // 260 instants", named which of them vary, and wrote a file in which all 99
    // were the carrier's constants. Nothing in the tool looked, because
    // everything in the tool was looking at the gather.
    //
    // So: a channel the gather says VARIES must vary in the written record. The
    // converse is not checked -- a byte can legitimately be constant, and the
    // report above already says which and why.
    if !gathered.is_empty() {
        match gbx::record::decode_ghost(&outp) {
            Err(e) => {
                println!("ABORT: cannot read back {} to check it: {}", outp, e);
                std::process::exit(3);
            }
            Ok(d) => {
                let mut lost: Vec<String> = Vec::new();
                for (b, vals) in &gathered {
                    if vals.len() <= 1 || *b >= d.sample_size {
                        continue;
                    }
                    let mut got: std::collections::BTreeSet<u8> = Default::default();
                    for i in 0..d.samples.len() {
                        let s = &d.raw[i * d.sample_size..][..d.sample_size];
                        got.insert(s[*b]);
                    }
                    if got.len() <= 1 {
                        lost.push(format!(
                            "byte {} (gather saw {} values, file holds one)",
                            b,
                            vals.len()
                        ));
                    }
                }
                if !lost.is_empty() {
                    println!(
                        "ABORT: {} channel(s) the gather read as VARYING are constant in the \
                         written file -- the values were read and then not written: {}. The \
                         file is NOT publishable.",
                        lost.len(),
                        lost.join(", ")
                    );
                    std::process::exit(3);
                }
                println!(
                    "  read-back OK: every channel the gather saw vary also varies in the file"
                );
            }
        }
    }
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
