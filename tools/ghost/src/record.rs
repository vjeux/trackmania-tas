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
        Some("show") => show(&a[1..]),
        _ => die(
            "ghost record rebuild IN OUT --span MS [--period MS] [--template N]\n\
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

fn rebuild(a: &[String]) {
    let inp = a.first().unwrap_or_else(|| die("ghost record rebuild IN OUT --span MS"));
    let out = a.get(1).unwrap_or_else(|| die("ghost record rebuild IN OUT --span MS"));
    let span: i64 = num(a, "--span").unwrap_or_else(|| die("--span MS: how far the record must reach"));
    let period: i64 = num(a, "--period").unwrap_or(50);
    let tmpl_from = flag(a, "--template-from").map(|s| s.to_string());
    let tmpl_idx: i64 = num(a, "--template").unwrap_or(0);
    if period <= 0 {
        die("--period must be positive");
    }
    if span <= 0 {
        die("--span must be positive");
    }

    // The template sample. By default the file's own -- see the module note on
    // why that is safe once the regeneration that follows is neutralised.
    let template: Vec<u8> = {
        let src = tmpl_from.clone().unwrap_or_else(|| inp.clone());
        let body = gbx::record::load_body(&src).unwrap_or_else(|e| die(e));
        let (ver, blob) = gbx::record::find_entrecord_blob(&body).unwrap_or_else(|e| die(e));
        let rd = gbx::record::parse_record_data(&blob, ver).unwrap_or_else(|e| die(e));
        let vi = pick_vehicle(&rd).unwrap_or_else(|| {
            die(format!(
                "{src} has no vehicle entity to take a template sample from -- \
                 pass --template-from FILE naming a ghost of this map that has one"
            ))
        });
        let e = &rd.ents[vi];
        let k = tmpl_idx.clamp(0, e.times.len() as i64 - 1) as usize;
        e.raw[k * e.sample_size..(k + 1) * e.sample_size].to_vec()
    };

    let n = (span / period) as usize + 1;
    let mut before = String::new();
    let (rawlen_before, rawlen_after) = rewrite_ghost(inp, out, |rd| {
        let vi = pick_vehicle(rd);
        // Everything about the entity except its samples comes from the car
        // entity we are replacing, so the record still says it is the same kind
        // of thing recorded the same way. With no vehicle entity at all
        // (186935) there is nothing to copy, and desc 0 with zeroed fields is
        // what the reader needs and all it needs.
        let (type_, u01, u04) = match vi {
            Some(i) => (rd.ents[i].type_, rd.ents[i].u01, rd.ents[i].u04),
            None => {
                let d = rd
                    .descs
                    .iter()
                    .position(|d| d.class_id == gbx::record::CLASS_CSCENEVEHICLEVIS)
                    .unwrap_or_else(|| {
                        die("this record declares no CSceneVehicleVis desc, so a rebuilt \
                             entity would have no class -- rebuild into a container of this \
                             map that does")
                    });
                (d as i32, 0, 0)
            }
        };
        before = format!(
            "{} entities, vehicle {} samples spanning {} .. {}",
            rd.ents.len(),
            vi.map(|i| rd.ents[i].times.len()).unwrap_or(0),
            vi.and_then(|i| rd.ents[i].times.first().copied()).unwrap_or(0),
            vi.and_then(|i| rd.ents[i].times.last().copied()).unwrap_or(0),
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
        // one of them. Dropping them is not tidiness: it is the difference
        // between a clip of our run and a clip of somebody's server session.
        rd.ents = vec![Ent { type_, u01, u02: 0, u03: span as i32, u04, times, raw, sample_size: ss, deltas2: Vec::new() }];
        // Data keyed to entities that no longer exist.
        rd.bulk_notices.clear();
        rd.custom_modules.clear();
        rd.start_ms = 0;
        rd.end_ms = span as i32;
        Ok(())
    })
    .unwrap_or_else(|e| die(e));

    println!("{inp}: {before}");
    println!(
        "{out}: 1 entity, {n} samples every {period} ms spanning 0 .. {span}  \
         (record payload {rawlen_before} -> {rawlen_after} B)"
    );

    // READ IT BACK. The encoder is exercised by a round-trip control elsewhere;
    // this is the check that THIS write produced the grid that was asked for,
    // and it costs one parse.
    let d = gbx::record::decode_ghost(out).unwrap_or_else(|e| die(format!("reading {out} back: {e}")));
    if d.samples.len() != n {
        die(format!("wrote {n} samples and read back {}", d.samples.len()));
    }
    let last = d.samples.last().map(|s| s.time_ms as i64).unwrap_or(-1);
    if last != (n as i64 - 1) * period {
        die(format!("last sample is at {last} ms, expected {}", (n as i64 - 1) * period));
    }
    println!(
        "read back: {} samples, {} .. {} ms -- every one a copy of the template, \
         so this file is NOT yet a run. Next: ghost regen --neutralise --inputs",
        d.samples.len(),
        d.samples.first().map(|s| s.time_ms).unwrap_or(0),
        last
    );
}
