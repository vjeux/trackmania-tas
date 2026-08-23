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
        Some("show") => show(&a[1..]),
        _ => die(
            "ghost record rebuild IN OUT --span MS [--period MS] [--template N]\n\
             ghost record shorten IN OUT   -- make the scene end when the car does,\n\
             \x20                                without touching the car's samples\n\
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
    for (i, e) in rd.ents.iter().enumerate() {
        let cls = rd.descs.get(e.type_ as usize).map(|d| d.class_id).unwrap_or(0);
        println!(
            "  ent {i}: desc {} class 0x{cls:08X}  {} samples x {} B  t {} .. {}{}",
            e.type_,
            e.times.len(),
            e.sample_size,
            e.times.first().copied().unwrap_or(0),
            e.times.last().copied().unwrap_or(0),
            if Some(i) == veh { "   <- the car this project reads" } else { "" }
        );
    }
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
        // ONE car, and only one. The other vehicle entities are other people's
        // cars -- 26 of them in 227654's carrier -- and a render draws every
        // one. The non-vehicle entities are what keep the scene alive after our
        // car has finished.
        rd.ents = vec![Ent {
            type_,
            u01,
            u02: 0,
            u03: span_ms as i32,
            u04,
            times,
            raw,
            sample_size: ss,
            deltas2: Vec::new(),
        }];
        rd.bulk_notices.clear();
        rd.custom_modules.clear();
        rd.start_ms = 0;
        rd.end_ms = span_ms as i32;
        Ok(())
    })?;

    // Read it back: this is the check that THIS write produced the grid that
    // was asked for, and it costs one parse.
    let d = gbx::record::decode_ghost(out).map_err(|e| format!("reading {out} back: {e}"))?;
    if d.samples.len() != n {
        return Err(format!("wrote {n} samples and read back {}", d.samples.len()));
    }
    Ok(format!(
        "{before} -> 1 entity, {n} samples every {period} ms spanning 0.000 .. {}",
        secs(span_ms)
    ))
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
