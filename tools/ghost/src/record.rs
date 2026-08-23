//! `ghost record` -- operations on the telemetry record itself, as a container
//! for samples, independent of what the samples say.
//!
//! `ghost regen` writes engine state into the record a file ALREADY HAS. That
//! is the right shape when the carrier's record covers the run, and 34 of the
//! project's 171 published ghosts show why it usually does: a transplanted tape
//! inherits a carrier that is a LONGER recording than our run.
//!
//! It is the wrong shape when the carrier's record does not cover the run, and
//! two of this project's headline results are exactly that case:
//!
//!   * **227654** -- the carrier is a 27-player multiplayer replay whose
//!     largest vehicle entity holds 365 samples spanning race 1.310 .. 19.480,
//!     for a run that finishes at 57.482. There is no grid to write 38 seconds
//!     of car onto, so `regen` hard-errors on the missing in-race samples --
//!     correctly, and with no way forward. It also carries 26 OTHER PEOPLE'S
//!     CARS, which a render would draw.
//!   * **186935** -- `BEST_793893` has a record node with no `CSceneVehicleVis`
//!     entity at all. Nothing to render, and nothing to write into.
//!
//! `rebuild` makes the grid: it throws the vehicle entities away and lays down
//! a fresh one, `0, period, 2*period, ...` out to the span, every sample a copy
//! of one template sample. That file is not yet a run -- every sample is
//! identical, so it is a car frozen at one point, and `regen`'s own G5 refuses
//! exactly that. It is scaffolding, and the next step is
//! `ghost regen --neutralise --inputs`, which writes the transform from engine
//! state, the input echo from the tape, and zeros every other per-run byte.
//!
//! **Which is what makes the template harmless.** After a neutralised regen the
//! only bytes still carrying the template's values are the format constants
//! that are identical in every ghost of every driver. So the template is taken
//! from the file's own record by default, and the two steps together produce a
//! record with no per-run byte from anybody.
//!
//! What is NOT touched: the input chunk, the declared time, the checkpoint
//! chunk, the uid, the identity strings. The oracle reads the input archive, so
//! rebuilding the record cannot change what the file simulates to -- and
//! `ghost verify` re-simulates the written file to prove that rather than
//! assuming it.

use gbx::container::secs;
use gbx::record::{Ent, RecordData};
use gbx::recwrite::rewrite_ghost;
use crate::cli::{die, flag, num};

/// The vehicle entity to rebuild from: the one with the most samples among
/// those whose samples are big enough to be a `CSceneVehicleVis` (>= 100 B).
/// Same rule as `fk`'s reader, so the two cannot disagree about which entity is
/// the car.
fn pick_vehicle(rd: &RecordData) -> Option<usize> {
    rd.ents
        .iter()
        .enumerate()
        .filter(|(_, e)| e.sample_size >= 100 && !e.times.is_empty())
        .max_by_key(|(_, e)| e.times.len())
        .map(|(i, _)| i)
}

pub fn cmd(a: &[String]) {
    match a.first().map(String::as_str) {
        Some("rebuild") => rebuild(&a[1..]),
        Some("shorten") => {
            let inp = a.get(1).unwrap_or_else(|| die("ghost record shorten IN OUT"));
            let out = a.get(2).unwrap_or_else(|| die("ghost record shorten IN OUT"));
            match shorten_scene(inp, out) {
                Ok(m) => println!("{out}: {m}"),
                Err(e) => die(e),
            }
        }
        Some("channels") => {
            let inp = a.get(1).unwrap_or_else(|| {
                die("ghost record channels IN OUT [--from DONOR --bytes N,N] [--set N=V,N=V]")
            });
            let out = a.get(2).unwrap_or_else(|| {
                die("ghost record channels IN OUT [--from DONOR --bytes N,N] [--set N=V,N=V]")
            });
            if let Some(set) = flag(a, "--set") {
                let pairs: Vec<(usize, u8)> = set
                    .split(',')
                    .filter(|s| !s.trim().is_empty())
                    .map(|s| {
                        let (b, v) = s.split_once('=').unwrap_or_else(|| {
                            die(format!("--set wants N=V pairs, got {s:?}"))
                        });
                        (
                            b.trim().parse().unwrap_or_else(|_| die(format!("--set: {b} is not a byte index"))),
                            v.trim().parse().unwrap_or_else(|_| die(format!("--set: {v} is not a 0..255 value"))),
                        )
                    })
                    .collect();
                match set_channels(inp, out, &pairs) {
                    Ok(m) => println!("{out}: {m}"),
                    Err(e) => die(e),
                }
            } else {
                let donor = flag(a, "--from").unwrap_or_else(|| {
                    die("--from DONOR.Ghost.Gbx: the file to take the named sample bytes from")
                });
                let list = flag(a, "--bytes")
                    .unwrap_or_else(|| die("--bytes N,N,... : which of the 116 sample bytes to copy"));
                let bytes: Vec<usize> = list
                    .split(',')
                    .filter(|s| !s.trim().is_empty())
                    .map(|s| s.trim().parse::<usize>().unwrap_or_else(|_| die(format!("--bytes: {s} is not a number"))))
                    .collect();
                match copy_channels(inp, out, donor, &bytes) {
                    Ok(m) => println!("{out}: {m}"),
                    Err(e) => die(e),
                }
            }
        }
        Some("show") => show(&a[1..]),
        Some("graft-scene") => {
            let inp = a.get(1).unwrap_or_else(|| {
                die("ghost record graft-scene IN OUT --from DONOR.Ghost.Gbx")
            });
            let out = a.get(2).unwrap_or_else(|| {
                die("ghost record graft-scene IN OUT --from DONOR.Ghost.Gbx")
            });
            let donor = flag(a, "--from")
                .unwrap_or_else(|| die("--from DONOR.Ghost.Gbx: the container the car was rebuilt out of"));
            match graft_scene(inp, out, donor, crate::cli::has(a, "--car-deltas")) {
                Ok(m) => println!("{out}: {m}"),
                Err(e) => die(e),
            }
        }
        Some("entorder") => {
            let inp = a.get(1).unwrap_or_else(|| {
                die("ghost record entorder IN OUT --car-first | --car-last")
            });
            let out = a.get(2).unwrap_or_else(|| {
                die("ghost record entorder IN OUT --car-first | --car-last")
            });
            let first = crate::cli::has(a, "--car-first");
            let last = crate::cli::has(a, "--car-last");
            if first == last {
                die("exactly one of --car-first or --car-last -- the two directions of the same \
                     experiment, and a run that does neither, or both, measures nothing");
            }
            match set_ent_order(inp, out, first) {
                Ok(m) => println!("{out}: {m}"),
                Err(e) => die(e),
            }
        }
        Some("notices") => {
            let inp = a.get(1).unwrap_or_else(|| {
                die("ghost record notices IN OUT --from DONOR.Ghost.Gbx | --strip")
            });
            let out = a.get(2).unwrap_or_else(|| {
                die("ghost record notices IN OUT --from DONOR.Ghost.Gbx | --strip")
            });
            let from = flag(a, "--from");
            let strip = crate::cli::has(a, "--strip");
            if from.is_some() == strip {
                die("exactly one of --from DONOR (restore) or --strip (remove) -- the two \
                     directions of the same experiment, and a run that does neither, or both, \
                     measures nothing");
            }
            match set_notices(inp, out, from, strip) {
                Ok(m) => println!("{out}: {m}"),
                Err(e) => die(e),
            }
        }
        Some("entfields") => {
            let inp = a.get(1).unwrap_or_else(|| {
                die("ghost record entfields IN OUT [--u01 N] [--u02 N] [--u04 N]")
            });
            let out = a.get(2).unwrap_or_else(|| {
                die("ghost record entfields IN OUT [--u01 N] [--u02 N] [--u04 N]")
            });
            let g = |k: &str| num(a, k).map(|v| v as i32);
            match set_ent_fields(inp, out, g("--u01"), g("--u02"), g("--u04")) {
                Ok(m) => println!("{out}: {m}"),
                Err(e) => die(e),
            }
        }
        _ => die(
            "ghost record rebuild IN OUT --span MS [--period MS] [--template N]\n\
             ghost record shorten IN OUT   -- make the scene end when the car does,\n\
             \x20                                without touching the car's samples\n\
             ghost record entfields IN OUT [--u01 N] [--u02 N] [--u04 N]\n\
             \x20                            -- the vehicle entity's UNIDENTIFIED header\n\
             \x20                               fields, for the client-import bisect\n\
             ghost record graft-scene IN OUT --from DONOR\n\
             \x20                            -- put the container's non-vehicle entities back\n\
             ghost record notices IN OUT --from DONOR | --strip\n\
             \x20                            -- restore or remove the record's notice lists:\n\
             \x20                               the two directions of one experiment\n\
             ghost record entorder IN OUT --car-first | --car-last\n\
             \x20                            -- move the car to the front or the back: in every\n\
             \x20                               ghost the game wrote it is entity 0\n\
             ghost record show FILE",
        ),
    }
}

fn show(a: &[String]) {
    let path = a.first().unwrap_or_else(|| die("ghost record show FILE"));
    let body = gbx::record::load_body(path).unwrap_or_else(|e| die(e));
    let (ver, blob) = gbx::record::find_entrecord_blob(&body).unwrap_or_else(|e| die(e));
    let rd = gbx::record::parse_record_data(&blob, ver).unwrap_or_else(|e| die(e));
    println!(
        "version {}  span {} .. {} ms  {} descs  {} entities  {} notices  {} bulk  {} custom",
        rd.version,
        rd.start_ms,
        rd.end_ms,
        rd.descs.len(),
        rd.ents.len(),
        rd.notices.len(),
        rd.bulk_notices.len(),
        rd.custom_modules.len()
    );
    for (i, d) in rd.descs.iter().enumerate() {
        println!("  desc {i}: class 0x{:08X}", d.class_id);
    }
    let veh = pick_vehicle(&rd);
    let mut live_scene = 0usize;
    for (i, e) in rd.ents.iter().enumerate() {
        let cls = rd.descs.get(e.type_ as usize).map(|d| d.class_id).unwrap_or(0);
        let is_car = Some(i) == veh;
        let live = !e.times.is_empty();
        // NON-VEHICLE IS A CLASS, NOT "NOT THE ONE THE PICKER CHOSE". A
        // multi-car container (227654 has 29 entities, five of them cars) has
        // exactly one entity the picker returns, so an index test counts the
        // OTHER FOUR CARS as scene records and reports "live non-vehicle
        // records: 5" about a file with one. The count then reads clean in
        // exactly the case it exists to catch.
        if cls != gbx::record::CLASS_CSCENEVEHICLEVIS && live {
            live_scene += 1;
        }
        println!(
            "  ent {i}: desc {} class 0x{cls:08X}  {} samples x {} B  t {} .. {}  u01 {} u02 {} \
             u03 {} u04 {}  {} delta2 block(s)  {}{}",
            e.type_,
            e.times.len(),
            e.sample_size,
            e.times.first().copied().unwrap_or(0),
            e.times.last().copied().unwrap_or(0),
            e.u01,
            e.u02,
            e.u03,
            e.u04,
            e.deltas2.len(),
            if live { "LIVE" } else { "placeholder (0 samples)" },
            if is_car { "   <- the car this project reads" } else { "" }
        );
    }
    // THE COUNT THAT WAS NOT PRINTED WHILE TWO CRASHERS AND A CLEAN FILE READ
    // AS "STRUCTURALLY INDISTINGUISHABLE". Restoring a live non-vehicle record
    // is what repaired both ghosts that crash the game client on import
    // (measured 2026-08-23) — and `TAS_67319` has none and imports cleanly, so
    // this is a lead, not a rule. `ghost verify` V11 says the same in one line.
    println!(
        "live non-vehicle records: {}{}",
        live_scene,
        if live_scene == 0 {
            "   <- the shape of the two files that crash the client on import: see ghost verify V11"
        } else {
            ""
        }
    );
}


/// Make the scene end when the car does, WITHOUT touching the car's samples.
///
/// The repair for the 31 published ghosts whose record outlives their run. Those
/// files' telemetry is fine -- the car's samples are its own and stop where the
/// run stops. What is wrong is the frame around them: the record node's declared
/// `end_ms` is the CONTAINER DONOR's, and the donor's non-vehicle entities
/// (0x2D001000, 13 bytes a sample) are still there at their own full length.
///
/// So this drops those and shortens the span, and does not regenerate anything.
/// That matters for three reasons: it needs no engine and no map, so it can be
/// run over a whole checkout in seconds; it cannot change a trajectory, so it
/// cannot introduce the defects a regeneration can; and re-regenerating 31
/// verified files to fix a number in their header would be the more dangerous
/// operation by far.
///
/// It refuses rather than guessing when the vehicle entity is not obvious --
/// a file with several live cars in it is a different problem, and 227654's
/// 27-entity carrier is the reason to say so out loud.
pub fn shorten_scene(inp: &str, out: &str) -> Result<String, String> {
    let mut before = String::new();
    let mut after = String::new();
    rewrite_ghost(inp, out, |rd| {
        let vi = pick_vehicle(rd).ok_or("no vehicle entity: nothing to shorten to")?;
        let last = rd.ents[vi].times.last().copied().unwrap_or(0);
        let scene = rd
            .ents
            .iter()
            .filter_map(|e| e.times.last().copied())
            .max()
            .unwrap_or(0)
            .max(rd.end_ms);
        before = format!(
            "{} entities, scene to {}, car to {}",
            rd.ents.len(),
            secs(scene as i64),
            secs(last as i64)
        );
        let dropped: Vec<String> = rd
            .ents
            .iter()
            .enumerate()
            .filter(|(i, _)| *i != vi)
            .map(|(_, e)| {
                format!(
                    "0x{:08X} x{}@{}",
                    rd.descs.get(e.type_ as usize).map(|d| d.class_id).unwrap_or(0),
                    e.times.len(),
                    secs(e.times.last().copied().unwrap_or(0) as i64)
                )
            })
            .collect();
        let car = rd.ents.swap_remove(vi);
        rd.ents = vec![car];
        rd.bulk_notices.clear();
        rd.custom_modules.clear();
        rd.end_ms = last;
        after = format!("1 entity, scene to {} (dropped {})", secs(last as i64), dropped.join(", "));
        Ok(())
    })?;

    // The car must come through untouched. This is the whole claim of the
    // operation, so it is checked rather than asserted: same sample count, same
    // times, same bytes.
    let a = gbx::record::decode_ghost(inp)?;
    let b = gbx::record::decode_ghost(out)?;
    if a.samples.len() != b.samples.len() || a.raw != b.raw {
        return Err(format!(
            "the car's samples changed ({} -> {} samples, {} -> {} bytes) -- refusing, this \
             operation must not touch the trajectory",
            a.samples.len(),
            b.samples.len(),
            a.raw.len(),
            b.raw.len()
        ));
    }
    Ok(format!("{before} -> {after}; the car's {} samples are byte-identical", a.samples.len()))
}

/// Write a fixed value into named per-sample bytes of the vehicle entity.
///
/// The other half of the bisect, and the one that separates *"the game needs
/// THIS driver's values"* from *"the game chokes on the value we write"*. A
/// channel a regeneration fills with raw zero is not neutral when its encoding
/// is offset-binary -- the dampen bytes decode 0 as **-2, fully extended
/// wheels**, and byte 32/33 hold 128/42 at rest in every recording the game
/// made itself.
pub fn set_channels(inp: &str, out: &str, pairs: &[(usize, u8)]) -> Result<String, String> {
    if pairs.is_empty() {
        return Err("no bytes named: --set N=V,N=V is the experiment".into());
    }
    let mut n = 0usize;
    let mut changed = 0usize;
    rewrite_ghost(inp, out, |rd| {
        let vi = pick_vehicle(rd).ok_or("no vehicle entity to write into")?;
        let e = &mut rd.ents[vi];
        let ss = e.sample_size;
        for (b, _) in pairs {
            if *b >= ss {
                return Err(format!("byte {b} is past the {ss}-byte sample"));
            }
        }
        n = e.times.len();
        for i in 0..n {
            for (b, v) in pairs {
                if e.raw[i * ss + b] != *v {
                    e.raw[i * ss + b] = *v;
                    changed += 1;
                }
            }
        }
        Ok(())
    })?;
    Ok(format!("{n} sample(s), set {pairs:?}: {changed} byte(s) changed"))
}

/// Copy named per-sample bytes of the vehicle entity from another file,
/// matching samples by their own timestamps.
///
/// **A bisect instrument, in the family of `record entfields`.** `ghost regen`
/// writes the transform and the tape echo and ZEROES eleven channels it cannot
/// read out of the engine — and one of them is read by something outside this
/// project. Measured 2026-08-23 on 276877: a regenerated 9.415 whose position,
/// velocity and quaternion are bit-identical to the file the published clip was
/// shot from renders with the chase camera BURIED IN THE RAMP for the last
/// second of the run, and re-filming that older file on the same build in the
/// same session reproduces the good camera. So the game reads a channel the
/// gate cannot see, and finding WHICH one is a one-byte-at-a-time question that
/// needs one file per hypothesis.
///
/// It refuses on a length change and reports the count it actually altered, so
/// "the copy did nothing" cannot be mistaken for "the byte does not matter" --
/// the null result of a bisect step has to be distinguishable from a no-op.
pub fn copy_channels(inp: &str, out: &str, donor: &str, bytes: &[usize]) -> Result<String, String> {
    if bytes.is_empty() {
        return Err("no bytes named: --bytes N,N,... is the experiment".into());
    }
    let d = gbx::record::decode_ghost(donor)?;
    let dss = d.sample_size;
    let src: std::collections::HashMap<i32, &[u8]> = d
        .samples
        .iter()
        .enumerate()
        .map(|(i, s)| (s.time_ms, &d.raw[i * dss..(i + 1) * dss]))
        .collect();
    let mut changed = 0usize;
    let mut missing = 0usize;
    let mut same = 0usize;
    let mut n = 0usize;
    rewrite_ghost(inp, out, |rd| {
        let vi = pick_vehicle(rd).ok_or("no vehicle entity to write into")?;
        let e = &mut rd.ents[vi];
        let ss = e.sample_size;
        if ss != dss {
            return Err(format!(
                "sample size {ss} here and {dss} in the donor -- these are not the same shape of \
                 record"
            ));
        }
        for b in bytes {
            if *b >= ss {
                return Err(format!("byte {b} is past the {ss}-byte sample"));
            }
        }
        n = e.times.len();
        for (i, t) in e.times.clone().iter().enumerate() {
            match src.get(t) {
                None => missing += 1,
                Some(s) => {
                    for b in bytes {
                        if e.raw[i * ss + b] == s[*b] {
                            same += 1;
                        } else {
                            e.raw[i * ss + b] = s[*b];
                            changed += 1;
                        }
                    }
                }
            }
        }
        Ok(())
    })?;
    Ok(format!(
        "{} sample(s), bytes {:?} from {}: {} byte(s) changed, {} already equal, {} sample(s) had \
         no counterpart in the donor and were left alone",
        n,
        bytes,
        donor,
        changed,
        same,
        missing
    ))
}

/// Lay a fresh 50 ms grid out to `span_ms`, one vehicle entity, every sample a
/// copy of one template. The library form of `ghost record rebuild`; `regen`
/// calls it on every run so the grid the engine writes into is ours.
///
/// `template_from` names another ghost to take the template sample from, for a
/// file whose own record has no vehicle entity at all.
pub fn rebuild_to(
    inp: &str,
    out: &str,
    span_ms: i64,
    template_from: Option<&str>,
    period: i64,
) -> Result<String, String> {
    if period <= 0 || span_ms <= 0 {
        return Err("a rebuilt record needs a positive span and period".into());
    }
    let template: Vec<u8> = {
        let src = template_from.unwrap_or(inp);
        let body = gbx::record::load_body(src)?;
        let (ver, blob) = gbx::record::find_entrecord_blob(&body)?;
        let rd = gbx::record::parse_record_data(&blob, ver)?;
        let vi = pick_vehicle(&rd).ok_or_else(|| {
            format!(
                "{src} has no vehicle entity to take a template sample from -- name one with \
                 --template-from"
            )
        })?;
        let e = &rd.ents[vi];
        e.raw[..e.sample_size].to_vec()
    };

    let n = (span_ms / period) as usize + 1;
    let mut before = String::new();
    let mut note = String::new();
    rewrite_ghost(inp, out, |rd| {
        let vi = pick_vehicle(rd);
        let (type_, u01, u04) = match vi {
            Some(i) => (rd.ents[i].type_, rd.ents[i].u01, rd.ents[i].u04),
            None => {
                let d = rd
                    .descs
                    .iter()
                    .position(|d| d.class_id == gbx::record::CLASS_CSCENEVEHICLEVIS)
                    .ok_or(
                        "this record declares no CSceneVehicleVis desc, so a rebuilt entity would \
                         have no class",
                    )?;
                (d as i32, 0, 0)
            }
        };
        before = format!(
            "{} entities, car {} samples spanning {} .. {}",
            rd.ents.len(),
            vi.map(|i| rd.ents[i].times.len()).unwrap_or(0),
            secs(vi.and_then(|i| rd.ents[i].times.first().copied()).unwrap_or(0) as i64),
            secs(vi.and_then(|i| rd.ents[i].times.last().copied()).unwrap_or(0) as i64),
        );
        let ss = template.len();
        let mut raw = Vec::with_capacity(n * ss);
        let mut times = Vec::with_capacity(n);
        for k in 0..n {
            times.push((k as i64 * period) as i32);
            raw.extend_from_slice(&template);
        }
        // ONE CAR, AND EVERY NON-VEHICLE RECORD THE CONTAINER HAD.
        //
        // The vehicle entities go: on 227654 the carrier holds 29 of them and
        // 28 are the SAME driver across 26 restarts (contiguous, non-overlapping
        // time ranges), so a render that kept them would draw a phantom car per
        // restart. That half was always right.
        //
        // What was wrong was dropping everything else. **Restoring a LIVE
        // non-vehicle record is what repaired both ghosts that crash the game
        // client on ghost import** — measured on the render box 2026-08-23 with
        // a same-session control on every run: the container this project's
        // 173691 film is built in imports fine with its live 0x2D001000 record
        // intact (X0); the same file rebuilt without it crashes, three times
        // across two revisions; grafting that one record back makes it import
        // (X5, X7); and three other single-field repairs each crashed with the
        // graft absent (X1, X3, X6).
        //
        // **It is not known to be a rule**: TAS_67319 has no live non-vehicle
        // record either and imports cleanly, so what makes those two files
        // different is still open (`ghost verify` V11 says so in one line).
        // Keeping what the container had is the conservative choice either way
        // — the game wrote those records and nothing here has a reason to
        // remove them.
        //
        // No headless check can see this: the dedicated server re-simulates the
        // input chunk and never reads the scene, so TAS_57482 passes
        // `ghost verify` V1..V10, re-simulates to 57.482, and kills the client.
        //
        // The kept records are clipped to the run's span, so this cannot put
        // back the scene-outlives-the-run defect that `shorten` exists to
        // remove.
        let car = Ent {
            type_,
            u01,
            u02: 0,
            u03: span_ms as i32,
            u04,
            times,
            raw,
            sample_size: ss,
            deltas2: Vec::new(),
        };
        // Every entity that is not a car, clipped to the span, in its original
        // order; then the car.
        let veh: std::collections::HashSet<i32> = rd
            .descs
            .iter()
            .enumerate()
            .filter(|(_, d)| d.class_id == gbx::record::CLASS_CSCENEVEHICLEVIS)
            .map(|(i, _)| i as i32)
            .collect();
        let mut kept: Vec<Ent> = Vec::new();
        let mut live = 0usize;
        for e in rd.ents.drain(..) {
            if veh.contains(&e.type_) {
                continue;
            }
            let mut e = e;
            let keep = e.times.iter().take_while(|t| **t <= span_ms as i32).count();
            if keep < e.times.len() && e.sample_size > 0 {
                e.times.truncate(keep);
                e.raw.truncate(keep * e.sample_size);
            }
            e.u03 = e.u03.min(span_ms as i32);
            e.deltas2.retain(|(_, t, _)| *t <= span_ms as i32);
            if !e.times.is_empty() {
                live += 1;
            }
            kept.push(e);
        }
        let n_kept = kept.len();
        // THE CAR GOES FIRST. It was pushed LAST here, and in every ghost the
        // game itself wrote the car is entity 0 while every file this rebuild
        // produced had it at the end -- a four-row separation the render arm
        // isolated across four maps, where the repo's own file imports and our
        // rebuild of that same file does not (two of them raise a FrameMessage
        // and never gain a ghost block, two kill the process outright), with
        // V1..V11 including the oracle passing on all four.
        //
        // Order is not cosmetic to a consumer that resolves entities by index.
        // `ghost record entorder IN OUT --car-first | --car-last` is the same
        // change in both directions, because one direction proves nothing:
        // tonight the scene-record lead, the notice lead and the u01 lead each
        // looked decisive from one side and died from the other.
        kept.insert(0, car);
        rd.ents = kept;
        // KEEP THE NOTICE LISTS -- but NOT because they fix the import crash.
        // They do not. THE HYPOTHESIS THAT PUT THIS LINE HERE IS DEAD, killed
        // in both directions by the command written to test it (2026-08-23,
        // render box, each behind a same-session `scene ready` control):
        //
        //   forward  blev_regen + the container's 82 notices  -> STILL CRASHES
        //   mirror   TAS_67319 with its 82 notices stripped   -> STILL IMPORTS
        //
        // Neither necessary nor sufficient. The six-specimen table that made
        // notices look like the separating field was a majority-class
        // coincidence, the same shape as the `u01` lead that survived until
        // somebody ran its reverse swap. It would have been written down as a
        // mechanism if the mirror had not been run.
        //
        // The lines stay because the CONSERVATIVE choice is to carry what the
        // container had: the game wrote these, the rebuild has no reason to
        // remove them, and clearing them was never justified by a measurement
        // either. That is the whole claim -- do not let it grow back into a
        // fix.
        //
        // The crash is still open. What survives every refutation so far is the
        // car entity's own encoding: the donor's car carries 31 `delta2` blocks
        // and every rebuilt car carries NONE (on 227654's untouched file the
        // car segments carry 11, 13 and 4; ours carries 0). That is the only
        // structural difference left between a file that imports and one that
        // does not, and it is the same difference on both crashers.
        rd.start_ms = 0;
        rd.end_ms = span_ms as i32;
        note = format!(
            ", kept {} non-vehicle record(s) of which {} live",
            n_kept, live
        );
        Ok(())
    })?;

    // Read it back: this is the check that THIS write produced the grid that
    // was asked for, and it costs one parse.
    let d = gbx::record::decode_ghost(out).map_err(|e| format!("reading {out} back: {e}"))?;
    if d.samples.len() != n {
        return Err(format!("wrote {n} samples and read back {}", d.samples.len()));
    }
    Ok(format!(
        "{before} -> the car regridded to {n} samples every {period} ms spanning 0.000 .. {}{note}",
        secs(span_ms)
    ))
}

/// Set the UNIDENTIFIED header fields of the record's vehicle entity.
///
/// This exists for one investigation and says so: two of this project's
/// regenerated ghosts crash the game client on import while four others from
/// the same path import cleanly, and the only field that separates them in any
/// dump we have is the car entity's `u01` — `0x02000006` on all four that
/// import, `0x0200000B` and `0x020000F1` on the two that crash. The value is
/// inherited from the container donor and nothing in this project knows what it
/// means, so the only way to find out is to move it, one field at a time, in
/// both directions, and ask the client.
///
/// It touches no sample and no time: the car's trajectory is required to come
/// back byte-identical, exactly as `shorten` requires.
/// Move the car entity to the front or the back of the record — the two
/// directions of the entity-order experiment.
///
/// In every ghost the game itself wrote, the car is **entity 0**. Every file
/// `rebuild_to` produced before 2026-08-23 had it **last**, and four of four
/// such rebuilds failed to import while the repo's own file for the same map
/// imported in the same session. That is a suggestive separation and nothing
/// more until it is run BOTH ways, which is what this is for:
///
/// * `--car-first` — a rebuilt file with the car put back at index 0.
/// * `--car-last` — a file that imports, with its car moved to the end.
///
/// If the first imports and the second crashes, order is the mechanism. If both
/// import, order was never it, and the rebuild's other differences — the
/// dropped `delta2` blocks, the scene record one sample short — are next.
///
/// Nothing but a client import can see any of this: the dedicated server
/// re-simulates the input chunk and never reads the entity list, so both
/// outputs re-simulate to the same time as their input. The car itself is
/// required to come through untouched.
pub fn set_ent_order(inp: &str, out: &str, car_first: bool) -> Result<String, String> {
    let mut before = String::new();
    let mut after = String::new();
    rewrite_ghost(inp, out, |rd| {
        let vi = pick_vehicle(rd).ok_or("no vehicle entity: nothing to reorder")?;
        if rd.ents.len() < 2 {
            return Err(format!(
                "this record has {} entity: order is not a property of it",
                rd.ents.len()
            ));
        }
        before = format!("car at index {} of {}", vi, rd.ents.len());
        let car = rd.ents.remove(vi);
        if car_first {
            rd.ents.insert(0, car);
        } else {
            rd.ents.push(car);
        }
        let vj = pick_vehicle(rd).unwrap_or(usize::MAX);
        after = format!("car at index {} of {}", vj, rd.ents.len());
        if vi == vj {
            return Err(format!(
                "the car is already {} ({}): writing this file would produce an import result \
                 about nothing",
                if car_first { "first" } else { "last" },
                before
            ));
        }
        Ok(())
    })?;
    let a = gbx::record::decode_ghost(inp)?;
    let b = gbx::record::decode_ghost(out)?;
    if a.samples.len() != b.samples.len() {
        return Err(format!(
            "the car changed: {} samples in, {} out",
            a.samples.len(),
            b.samples.len()
        ));
    }
    Ok(format!("{before} -> {after}; car unchanged ({} samples)", a.samples.len()))
}

/// Restore or remove the record's NOTICE LISTS — the two directions of one
/// experiment.
///
/// **The hypothesis this was written to test is DEAD, and this command is what
/// killed it.** The notice list looked like the single field separating every
/// importable ghost from `blev_regen`, which crashes the game client with
/// V1..V11 passing and its scene record present. Both directions, run on the
/// render box 2026-08-23, each behind a same-session `scene ready` control:
///
/// * forward — `blev_regen` **plus** the container's 82 notices: still crashes.
/// * mirror — `TAS_67319` with its 82 notices **stripped**: still imports.
///
/// So notices are neither necessary nor sufficient, and the six-specimen table
/// that made them look decisive was a majority-class coincidence — the same
/// shape as the `u01` lead that survived until its reverse swap was run. It
/// would have been written into `GHOSTS.md` as a mechanism if the mirror had
/// not been run.
///
/// The command stays, because the next candidate wants the same treatment:
///
/// * `--from DONOR` — put the container's notices back into a file that crashes.
/// * `--strip` — take them out of a file that imports.
///
/// It refuses unless exactly one direction is named, and it requires the car to
/// come through untouched: an import result means nothing if the tape moved.
///
/// Nothing headless can see any of this: the dedicated server re-simulates the
/// input chunk and never reads the record, so both outputs re-simulate to the
/// same time as their input. A client import is the only instrument.
pub fn set_notices(
    inp: &str,
    out: &str,
    donor: Option<&str>,
    strip: bool,
) -> Result<String, String> {
    let taken = match donor {
        None => None,
        Some(d) => {
            let dbody = gbx::record::load_body(d)?;
            let (dver, dblob) = gbx::record::find_entrecord_blob(&dbody)?;
            let drd = gbx::record::parse_record_data(&dblob, dver)?;
            if drd.notices.is_empty() && drd.bulk_notices.is_empty() {
                return Err(format!(
                    "{d} has no notices to give: restoring nothing would look like a passing \
                     experiment"
                ));
            }
            Some((drd.notices, drd.bulk_notices, drd.custom_modules))
        }
    };
    let mut before = String::new();
    let mut after = String::new();
    rewrite_ghost(inp, out, |rd| {
        before = format!(
            "{} notices, {} bulk, {} custom",
            rd.notices.len(),
            rd.bulk_notices.len(),
            rd.custom_modules.len()
        );
        if strip {
            rd.notices.clear();
            rd.bulk_notices.clear();
            rd.custom_modules.clear();
        } else if let Some((n, b, c)) = taken.clone() {
            rd.notices = n;
            rd.bulk_notices = b;
            rd.custom_modules = c;
        }
        after = format!(
            "{} notices, {} bulk, {} custom",
            rd.notices.len(),
            rd.bulk_notices.len(),
            rd.custom_modules.len()
        );
        Ok(())
    })?;
    // The car must be untouched: this changes the record's notices and nothing
    // about the driving, and an import result means nothing if the tape moved.
    let a = gbx::record::decode_ghost(inp)?;
    let b = gbx::record::decode_ghost(out)?;
    let av = a.samples.len();
    let bv = b.samples.len();
    if av != bv {
        return Err(format!(
            "the car changed: {av} samples in, {bv} out -- refusing to write a file whose import \
             result would not be about notices"
        ));
    }
    Ok(format!("{before} -> {after}; car unchanged ({av} samples)"))
}

pub fn set_ent_fields(
    inp: &str,
    out: &str,
    u01: Option<i32>,
    u02: Option<i32>,
    u04: Option<i32>,
) -> Result<String, String> {
    if u01.is_none() && u02.is_none() && u04.is_none() {
        return Err("nothing to set: give --u01, --u02 or --u04".into());
    }
    let mut before = String::new();
    let mut after = String::new();
    rewrite_ghost(inp, out, |rd| {
        let vi = pick_vehicle(rd).ok_or("no vehicle entity to set fields on")?;
        let e = &mut rd.ents[vi];
        before = format!("u01 {} u02 {} u04 {}", e.u01, e.u02, e.u04);
        if let Some(v) = u01 {
            e.u01 = v;
        }
        if let Some(v) = u02 {
            e.u02 = v;
        }
        if let Some(v) = u04 {
            e.u04 = v;
        }
        after = format!("u01 {} u02 {} u04 {}", e.u01, e.u02, e.u04);
        Ok(())
    })?;
    let a = gbx::record::decode_ghost(inp)?;
    let b = gbx::record::decode_ghost(out)?;
    if a.samples.len() != b.samples.len() || a.raw != b.raw {
        return Err("the car's samples changed -- refusing, this operation must not touch the \
                    trajectory"
            .into());
    }
    Ok(format!("{before} -> {after}; the car's {} samples are byte-identical", a.samples.len()))
}

/// Put the container's NON-VEHICLE entities back, from the file the car was
/// rebuilt out of.
///
/// The inverse of what `rebuild_to` does, and it exists because of a measured
/// fork. `ghost regen` lays a fresh grid for the car and, in doing so, keeps
/// **one** entity and drops everything else the record had. Two regenerated
/// files crash the game client on import; the container one of them was built
/// in — the same version, the same 107-byte samples, the game's own bytes —
/// **imports fine with its three entities intact** (measured on the render box,
/// 2026-08-23, five variants each behind a same-session control). Three other
/// candidate causes died in that batch: the entity's `u01`, the declared
/// checkpoint count, and the container generation. What is left is what the
/// rebuild removed.
///
/// So this grafts those entities back, from `donor`, at their own times, and:
///
/// * it copies **only** entities the vehicle picker does not choose — another
///   car is not scenery, and 227654's container is one driver across 26
///   restarts rather than 26 opponents, so "everything else" would be wrong
///   there;
/// * it clips their samples to the record's span, because a scene entity that
///   outlives the run is the defect `shorten` exists to remove and this must not
///   reintroduce it;
/// * it requires the car's samples back **byte-identical**.
///
/// **These samples are the DONOR's**, and the file says so: they are 13 bytes
/// per sample of somebody else's session state, not a trajectory. Run
/// `ghost verify` afterwards and read V3 and the raw-bytes backstop before
/// publishing anything built this way.
pub fn graft_scene(inp: &str, out: &str, donor: &str, car_deltas: bool) -> Result<String, String> {
    let dbody = gbx::record::load_body(donor)?;
    let (dver, dblob) = gbx::record::find_entrecord_blob(&dbody)?;
    let drd = gbx::record::parse_record_data(&dblob, dver)?;
    let dveh = pick_vehicle(&drd);
    // GRAFT THE SCENE, NOT THE OTHER CARS. This filtered on "not the index the
    // vehicle picker chose", which is right only for a container holding one
    // car. 227654's holds 29 entities and FIVE cars, so the index test grafted
    // four live CSceneVehicleVis records back — phantom cars in the render,
    // the exact hazard `rebuild_to` documents. `rebuild_to` filters on the desc
    // class; so does this now, and so does the `show` counter above.
    let scene: Vec<Ent> = drd
        .ents
        .iter()
        .enumerate()
        .filter(|(i, e)| {
            let cls = drd.descs.get(e.type_ as usize).map(|d| d.class_id).unwrap_or(0);
            cls != gbx::record::CLASS_CSCENEVEHICLEVIS && Some(*i) != dveh
        })
        .map(|(_, e)| e.clone())
        .collect();
    if scene.is_empty() {
        return Err(format!("{donor} has no non-vehicle entity to graft"));
    }
    let mut before = String::new();
    let mut after = String::new();
    rewrite_ghost(inp, out, |rd| {
        let vi = pick_vehicle(rd).ok_or("no vehicle entity: nothing to graft onto")?;
        if rd.ents.len() > 1 {
            return Err(format!(
                "this file already has {} entities -- graft-scene is for a record the rebuild \
                 reduced to one",
                rd.ents.len()
            ));
        }
        let span = rd.end_ms;
        before = format!("1 entity, scene to {}", secs(span as i64));
        let mut car = rd.ents.remove(vi);
        // The car's own delta2 blocks are dropped by the rebuild too — 31 of
        // them on this donor, none on any rebuilt file. They are per-entity
        // side-channel records, not samples, so restoring them cannot move the
        // trajectory; whether the client needs them is exactly the open
        // question this flag exists to ask.
        let mut car_note = String::new();
        if car_deltas {
            if let Some(di) = dveh {
                let mut d = drd.ents[di].deltas2.clone();
                d.retain(|(_, t, _)| *t <= span);
                car_note = format!(", car deltas2 0 -> {}", d.len());
                car.deltas2 = d;
            }
        }
        let mut added = Vec::new();
        for e in &scene {
            let mut e = e.clone();
            // Clip to the span: a scene entity that outlives the run is the
            // defect `shorten` removes, and this must not put one back.
            let keep = e.times.iter().take_while(|t| **t <= span).count();
            if keep < e.times.len() && e.sample_size > 0 {
                e.times.truncate(keep);
                e.raw.truncate(keep * e.sample_size);
            }
            e.u03 = e.u03.min(span);
            e.deltas2.retain(|(_, t, _)| *t <= span);
            let cls = drd.descs.get(e.type_ as usize).map(|d| d.class_id).unwrap_or(0);
            added.push(format!(
                "0x{:08X} x{}@{}B",
                cls,
                e.times.len(),
                e.sample_size
            ));
            rd.ents.push(e);
        }
        rd.ents.push(car);
        after = format!("{} entities (grafted {}){}", rd.ents.len(), added.join(", "), car_note);
        Ok(())
    })?;
    let a = gbx::record::decode_ghost(inp)?;
    let b = gbx::record::decode_ghost(out)?;
    if a.samples.len() != b.samples.len() || a.raw != b.raw {
        return Err(format!(
            "the car's samples changed ({} -> {} samples) -- refusing, this operation must not \
             touch the trajectory",
            a.samples.len(),
            b.samples.len()
        ));
    }
    Ok(format!("{before} -> {after}; the car's {} samples are byte-identical", a.samples.len()))
}

fn rebuild(a: &[String]) {
    let inp = a.first().unwrap_or_else(|| die("ghost record rebuild IN OUT --span MS"));
    let out = a.get(1).unwrap_or_else(|| die("ghost record rebuild IN OUT --span MS"));
    let span: i64 =
        num(a, "--span").unwrap_or_else(|| die("--span MS: how far the record must reach"));
    let period: i64 = num(a, "--period").unwrap_or(50);
    let tf = flag(a, "--template-from");
    match rebuild_to(inp, out, span, tf, period) {
        Ok(msg) => println!(
            "{out}: {msg}\nThis file is NOT yet a run -- every sample is a copy of the template. \
             `ghost regen` is what makes it one, and it does this step itself."
        ),
        Err(e) => die(e),
    }
}
