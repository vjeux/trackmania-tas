//! THE FINISH, READ FROM THE ENGINE — so a candidate stops when its answer is
//! known instead of when the validator has finished formatting it.
//!
//! `fk tickhook cost` measures a candidate as **9.9 ms fixed + 27.6 µs per
//! tick**, and of that fixed part **5.8 ms is spent after the child's last
//! simulated tick and before its first byte of output** — constant, 44 % of a
//! late-checkpoint candidate. It is not transport (that is 0.06 ms) and it is
//! barely simulation (the engine runs 7 ticks past the finish, 0.26 ms): it is
//! the validator's own finish-and-print path, run to deliver one integer.
//!
//! The engine already knows that integer, several milliseconds earlier. It
//! writes the finish time — in SIMULATION ms, sub-tick interpolation included —
//! into a per-race block one tick after the tick that detects the crossing.
//! A child that watches that word can report it and `_exit`.
//!
//! ## Why this is calibrated and not a constant
//!
//! The obvious thing is a hardcoded offset, and it is wrong. A backward pointer
//! scan (`fk tickhook finish --chain`) named `[[controller+0x1a88]+0xa4]`, and
//! that survived three tapes on map 2 with three different finish times — but:
//!
//! * on **145875** the same offset is right, and the state word beside it
//!   counts `0 -> 1` where map 2 counts `2 -> 3`;
//! * on **126859** the record is not in that block's first kilobyte at all;
//! * the "nothing yet" marker is `0xffffffff` on map 2 and `0` on 126859.
//!
//! Every one of those made a hardcoded rule fail SILENTLY — the lever simply
//! never fired, and a search would have kept paying the 5.8 ms while believing
//! it had been fixed. So the driver calibrates instead: one fork of the tape
//! whose finish time it already knows, gathering the block as it changes, and
//! it keeps the word that ends up holding exactly that time. Children are
//! forked from the same process, so the ADDRESS is the same in every one of
//! them and no offset has to be portable.

use crate::forksrv::{ForkServer, Rec};
use crate::procmem;

/// `controller + this` -> the block the engine fills at the finish. This one
/// offset IS structural (the backward scan found it on three tapes); what it
/// points at is what varies, which is why the rest is measured.
pub const RESULT_PTR_IN_CONTROLLER: u64 = 0x1a88;

/// `result_block + this` -> the finish time in simulation ms. Structural on
/// every map measured; the calibration fork exists for the ones where it is not.
pub const FINISH_SIM_MS_IN_RESULT: u64 = 0xa4;

/// THE TAPE'S LAST TICK IS ARITHMETIC, NOT A MEASUREMENT.
///
/// The clock IS the record index: a child resuming at boundary tick `probe`
/// with `n` records consumes its last one at `probe + n - 1`. The driver knows
/// both numbers before the child exists.
///
/// The first version of this READ A WORD instead -- `participant+0x188`, which
/// goes 2 -> 0 one tick after the last record on the maps `fk tickhook dnf` was
/// run on. It cost a regression: on Kacky Reloaded #290 that word also moves at
/// the FINISH, so a genuine finish 4-7 ticks inside the tape tripped the
/// past-the-end guard and was scored a DNF -- in two runs of three, because
/// whether the word had settled by the finish tick depends on the engine's
/// wall-clock frame partition. A measured stand-in for a known fact is a bug
/// waiting for a map.
pub fn last_tape_clock(probe_clock: u64, n_records: usize) -> u64 {
    probe_clock + n_records as u64 - 1
}

/// How much of the block to watch. The record was at +0xa4 on two maps and
/// past +0x400 on a third; 32 KB covers every one seen and costs nothing,
/// because the gather is deduplicated and the block barely changes.
const WINDOW: u32 = std::mem::size_of::<u8>() as u32 * 32 * 1024;

/// How far to look when 32 KB was not enough. 126859 keeps its finish record
/// outside every 32 KB window tried, so the second pass sweeps 256 KB of each
/// base -- one extra fork per base, and only on a map that needs it.
const WIDE_WINDOW: u32 = 256 * 1024;

/// Find the word this engine writes its finish time into, and hand it to the
/// shim. Returns the address and the "no result yet" value it currently holds.
///
/// `declared_ms` is the RACE time the tape is known to validate to — the
/// incumbent's own answer, which the driver has before it starts the server.
pub fn calibrate(
    srv: &mut ForkServer,
    probe: usize,
    recs: &[Rec],
    declared_ms: i64,
) -> Result<(u64, u32), String> {
    let pid = srv.pid();
    let want = (declared_ms + srv.race_start as i64) as u32;
    // WHERE TO LOOK, in order. The controller's block is where the backward
    // pointer scan found it and where it is on two of the three maps tried; on
    // 126859 it is not there at all, so the other typed objects follow. Each
    // costs one fork of a tape that is going to be simulated anyway.
    let chain = crate::car::locate(srv)?;
    let block = procmem::read_at(pid, srv.validator_controller + RESULT_PTR_IN_CONTROLLER, 8)
        .map(|b| u64::from_le_bytes(b[..8].try_into().unwrap()))
        .unwrap_or(0);
    // EVERY VEHICLE SLOT, not just the live one. The participant holds four
    // (Stadium, Snow, Rally, Desert) and on 126859 the finish record is in a
    // slot that is NOT the one being driven -- which only showed up when the
    // chain started picking the live slot correctly and calibration stopped
    // finding the word it had found the day before. Searching a 32 KB window of
    // each costs one fork apiece and removes the guess entirely.
    let mut bases: Vec<(String, u64)> = vec![
        ("the controller's result block".into(), block),
        ("the participant".into(), chain.participant),
    ];
    for (i, v) in chain.vehicles.iter().enumerate() {
        bases.push((format!("vehicle slot {}", i), *v));
    }
    bases.push(("the playground".into(), chain.playground));
    bases.push(("the simulation".into(), chain.sim));
    // THE STRUCTURAL PATH FIRST: no fork at all.
    //
    // vjeux: "We shouldn't have to do 51 fork boundary calibrations or any of
    // this kind of things." The block IS reachable by a pointer
    // (`controller+0x1a88`, allocated at `0x118c22d`), and on the maps where
    // the record lives at `+0xa4` of it the address needs no measuring: read
    // the pointer, add the offset, and CHECK -- while the race is running the
    // word must hold one of the two "nothing yet" markers. That check is what
    // keeps this honest: a build or a map that puts the record elsewhere fails
    // it and falls through to the measurement below, rather than reporting a
    // wrong number.
    if block >= 0x1000 {
        let w = block + FINISH_SIM_MS_IN_RESULT;
        if let Some(v) = procmem::read_at(pid, w, 4)
            .map(|b| u32::from_le_bytes(b[..4].try_into().unwrap()))
        {
            // ONLY the unambiguous marker. A word holding 0 looks exactly like a
            // word that has never been written, and taking it is not a
            // near-miss: on 126859 this accepted a zeroed word at the right
            // offset of the right block and reported 46 wrong finish times in
            // 150 candidates. `0xffffffff` is a value someone chose; `0` is
            // what memory is. A map whose sentinel is 0 pays the calibration
            // fork below, which proves the word by watching it take this
            // race's own answer.
            if v == u32::MAX {
                let ack = srv.set_finish_word(
                    w,
                    last_tape_clock(srv.clock, recs.len()),
                    chain.participant + crate::car::CP_COUNT_IN_PARTICIPANT,
                );
                if ack.starts_with("FINISH") {
                    return Ok((w, v));
                }
            }
        }
    }
    let mut tried: Vec<String> = Vec::new();
    // 32 KB of each base first -- that is where it is on every map but one --
    // then 256 KB, which costs a fork per base and only happens on a map the
    // narrow pass could not answer.
    let passes: [(u32, u64); 2] = [(WINDOW, 0), (WIDE_WINDOW, 0)];
    for (window, _) in passes {
    for (what, base) in &bases {
        let what = what.as_str();
        let base = *base;
        if base < 0x1000 {
            continue;
        }
        match calibrate_in(srv, probe, recs, want, base, window) {
            Ok(v) => {
                let (addr, sentinel) = v;
                // and the two words that make a DNF answerable: the one the
                // engine zeroes when the tape runs out, and the checkpoint
                // counter (`car::CP_COUNT_IN_PARTICIPANT`).
                let ack = srv.set_finish_word(
                    addr,
                    last_tape_clock(srv.clock, recs.len()),
                    chain.participant + crate::car::CP_COUNT_IN_PARTICIPANT,
                );
                if !ack.starts_with("FINISH") {
                    return Err(format!("the shim refused the finish word: {}", ack));
                }
                return Ok((addr, sentinel));
            }
            Err(e) => tried.push(format!("{} ({} KB): {}", what, window / 1024, e)),
        }
    }
    }
    Err(format!(
        "no object holds a word that ends at {} (race {} + start {}): {}",
        want,
        declared_ms,
        srv.race_start,
        tried.join("; ")
    ))
}

fn calibrate_in(
    srv: &mut ForkServer,
    probe: usize,
    recs: &[Rec],
    want: u32,
    block: u64,
    window: u32,
) -> Result<(u64, u32), String> {

    // ONE fork, gathering the block as it CHANGES: the dedup key is the whole
    // window, so a block that sits still costs nothing and we still see every
    // distinct state it passes through -- including the last one, which is the
    // only one this needs.
    let nseg = 8u64;
    let chunk = window / nseg as u32;
    let segs: Vec<(u64, u32)> = (0..nseg).map(|i| (block + i * chunk as u64, chunk)).collect();
    let (_j, blob) = srv.run_sampled_segs_ex(
        probe,
        recs,
        &segs,
        1,
        4096,
        (0, window),
        crate::clock::budget_for_ticks(recs.len() as u32 + 16),
    );
    let recsz = 8 + window as usize;
    let n = blob.len() / recsz;
    if n < 2 {
        return Err(format!(
            "the calibration fork produced {} distinct states of the result block -- it never \
             reached the finish",
            n
        ));
    }
    let word = |i: usize, o: usize| {
        u32::from_le_bytes(blob[i * recsz + 8 + o..i * recsz + 8 + o + 4].try_into().unwrap())
    };
    // the word that ENDS at this run's finish time and did not start there
    let mut hit: Option<(usize, u32)> = None;
    for o in (0..window as usize - 4).step_by(4) {
        if word(n - 1, o) == want && word(0, o) != want {
            let first = word(0, o);
            // and the marker it started from must be one of the two the engine
            // uses, never a live number we would mistake for a result
            if first == 0 || first == u32::MAX {
                hit = Some((o, first));
                break;
            }
        }
    }
    let (off, sentinel) = hit.ok_or_else(|| {
        format!("no word ends at {}", want)
    })?;
    Ok((block + off as u64, sentinel))
}
