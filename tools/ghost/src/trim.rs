//! `ghost trim` -- set a run's WINDOW: cut the head and/or tail, or extend the
//! tape past the end of the recording, and keep the file coherent either way.
//!
//! Coherent means all of this at once, and the command re-reads the file it
//! wrote and checks every line of it before it reports success:
//!
//!   * the INPUT TAPE holds only ticks inside the window, with
//!     `start_offset_ms` moved so every surviving tick keeps its own race time;
//!   * the TELEMETRY record holds only samples inside the window, in EVERY
//!     entity (a container brings more than the vehicle: 165922's donor carried
//!     175 815 samples of an undecoded entity spanning a 2.4-hour session);
//!   * the record's declared SPAN matches the samples it now describes;
//!   * every copy of the DECLARED TIME agrees, and the splits are the ones
//!     inside the window.
//!
//! A HEAD cut is a recording trim, not an input trim: the surviving tape starts
//! mid-run, so replaying it from a standing start is a different run. The
//! command says so, and `--from` refuses to pretend otherwise.
//!
//! LENGTHENING IS THE SAME OPERATION, and it lives here rather than in
//! `ghost tape` for that reason: one command owns a run's length, because the
//! obligations are one set. `--to` past the end of the tape appends copies of
//! the last packet -- what `u02 extend` did, with `u02`'s 15 callsites and none
//! of its coherence. The 173691 landing work is why: it needed a 7000-tick tape
//! to give the car room to brake after touchdown, and `ghost tape inject`
//! refuses a length change by design (a tape and its container must agree).
//!
//! An extension does not touch the telemetry, the declared time or the splits,
//! and that is not an omission: no samples exist past the end of the recording,
//! and the extra ticks are AFTER the finish, so the run's official time is
//! unchanged. The control for that claim is the oracle -- re-simulating the
//! lengthened file must return the same time as the original.

use gbx::container::{secs, Container, GhostResult};
use crate::oracle::{self, MapsMode};
use gbx::tape::{Encoding, StateEnc, Tape};
use crate::cli::{die, flag, has, num};

pub fn cmd(a: &[String]) {
    let inp = a.first().unwrap_or_else(|| die("ghost trim IN OUT [--from MS] [--to MS]"));
    let out = a.get(1).unwrap_or_else(|| die("ghost trim IN OUT [--from MS] [--to MS]"));
    let from = num(a, "--from").unwrap_or(i64::MIN);
    let to = num(a, "--to").unwrap_or(i64::MAX);
    if from == i64::MIN && to == i64::MAX {
        die("nothing to do: give --from MS and/or --to MS (race time, milliseconds)");
    }
    if from >= to {
        die("--from must be before --to");
    }
    let c = Container::load(inp).unwrap_or_else(|e| die(e));
    let t = Tape::from_file(inp).unwrap_or_else(|e| die(e));
    t.verbatim_is_identity()
        .unwrap_or_else(|e| die(format!("refusing to trim a file whose tape does not round-trip: {}", e)));

    // ---- the tape -----------------------------------------------------
    let mut nt = t.clone();
    let mut cut_head = 0usize;
    let mut cut_tail = 0usize;
    let mut added = 0usize;
    let n_arch = nt.archives.len();
    for ar in nt.archives.iter_mut() {
        let so = ar.start_offset_ms as i64;
        let n = ar.packets.len() as i64;
        let lo = if from == i64::MIN { 0 } else { (((from - so) + 9) / 10).clamp(0, n) };
        // The window's end BEFORE clamping: past `n` it is an extension.
        let want_hi = if to == i64::MAX { n } else { ((to - so) / 10) + 1 };
        let hi = want_hi.clamp(0, n);
        if hi <= lo {
            die(format!(
                "the window {} .. {} leaves no ticks (this archive spans {} .. {})",
                secs(from.max(0)),
                secs(to.min(so + 10 * n)),
                secs(so),
                secs(so + 10 * n)
            ));
        }
        cut_head = lo as usize;
        cut_tail = (n - hi) as usize;
        ar.packets = ar.packets[lo as usize..hi as usize].to_vec();
        ar.start_offset_ms = (so + 10 * lo) as i32;
        // The first surviving packet cannot say "same as the previous tick":
        // there is no previous tick any more. Expanding it is the whole fix,
        // and it is exactly the case the format's one-bit form hides.
        if let Some(p) = ar.packets.first_mut() {
            p.vsame = false;
            if let StateEnc::Prev | StateEnc::Prev2(_, _) = p.state {
                // a repeat of a word that is no longer in the file: freeze the
                // word it decoded to into an explicit literal
                let lit = ((p.flags as u64 & 0x3F_FFFF) << 5) | (p.word0 as u64 & 0xF);
                p.state = StateEnc::Lit(lit);
            }
        }
        // ---- the extension ------------------------------------------------
        if want_hi > n {
            if n_arch > 1 {
                die(format!(
                    "this file has {} input archives; extending it would have to choose one to \
                     grow, and nothing in the file says which. Refusing.",
                    n_arch
                ));
            }
            let last = ar
                .packets
                .last()
                .cloned()
                .unwrap_or_else(|| die("cannot extend a tape with no packets"));
            added = (want_hi - n) as usize;
            for _ in 0..added {
                let mut p = last.clone();
                // Repeat the last INPUT, not the last EVENT. A respawn is an
                // input like any other -- bit 31 of the state literal, `word0`
                // bit 5 -- so copying a respawn tick five thousand times would
                // hold the respawn key down for the whole extension. The gate
                // below re-reads the file and asserts no appended tick carries
                // it.
                p.word0 &= !0x20;
                p.state = StateEnc::Prev;
                p.vsame = true;
                ar.packets.push(p);
            }
        }
    }
    let new_span_lo = nt.archives[0].start_offset_ms as i64;
    let new_span_hi = new_span_lo + 10 * nt.archives[0].packets.len() as i64;

    // ---- the declared time and the splits ------------------------------
    let old_decl = c.declared_times().first().map(|x| x.1 as i64);
    let declared: i64 = num(a, "--declare").unwrap_or_else(|| match old_decl {
        Some(d) if to != i64::MAX && d > to => to,
        Some(d) => d,
        None => new_span_hi,
    });
    let splits_before: Vec<i32> = c.splits();
    let mut body = nt.splice_into(c.body(), Encoding::Explicit).unwrap_or_else(|e| die(e));
    set_all_declared(&mut body, declared as u32);
    // Keep the checkpoints inside the window. An EXTENSION keeps all of them:
    // the appended ticks are after the finish and cross nothing.
    let keep = |t: i32| (t as i64) <= declared && (t as i64) >= from.max(0);
    let body = rewrite_result(&body, |r| {
        r.race_ms = declared as i32;
        r.entries.retain(|(t, _)| keep(*t));
    })
    .unwrap_or_else(|e| die(e));
    let splits: Vec<i32> = gbx::container::read_result(&body).map(|r| r.checkpoints()).unwrap_or_default();
    let tmp = format!("{}.trim-stage", out);
    gbx::container::write_gbx(&c.gbx, body, &tmp).unwrap_or_else(|e| die(e));

    // ---- the telemetry --------------------------------------------------
    let lo_ms = if from == i64::MIN { i32::MIN } else { from as i32 };
    let hi_ms = if to == i64::MAX { i32::MAX } else { to as i32 };
    let mut dropped = 0usize;
    let mut kept = 0usize;
    // A pure EXTENSION does not touch the record at all. Recomputing the span
    // there would move `end_ms` from what the game wrote (19.530 on the map-1
    // WR) to the last sample's own time (19.500) -- a change to a recording
    // nobody asked to edit, in a command that only added ticks after it.
    let cutting = cut_head > 0 || cut_tail > 0;
    let had_record =
        cutting && gbx::recwrite::find_rec_site(&Container::load(&tmp).unwrap().gbx.body).is_ok();
    if had_record {
        let r = gbx::recwrite::rewrite_ghost(&tmp, out, |rd| {
            for e in rd.ents.iter_mut() {
                if e.times.is_empty() || e.sample_size == 0 {
                    continue;
                }
                let ss = e.sample_size;
                let keep: Vec<bool> = e.times.iter().map(|t| *t >= lo_ms && *t <= hi_ms).collect();
                let d = keep.iter().filter(|k| !**k).count();
                if d == 0 {
                    kept += e.times.len();
                    continue;
                }
                dropped += d;
                let mut nt2: Vec<i32> = Vec::new();
                let mut nraw: Vec<u8> = Vec::new();
                for (i, k) in keep.iter().enumerate() {
                    if *k {
                        nt2.push(e.times[i]);
                        nraw.extend_from_slice(&e.raw[i * ss..(i + 1) * ss]);
                    }
                }
                kept += nt2.len();
                e.times = nt2;
                e.raw = nraw;
                e.deltas2.retain(|(_, t, _)| *t >= lo_ms && *t <= hi_ms);
            }
            let last = rd.ents.iter().filter_map(|e| e.times.last().copied()).max();
            let first = rd.ents.iter().filter_map(|e| e.times.first().copied()).min();
            rd.start_ms = first.unwrap_or(0).max(if lo_ms == i32::MIN { 0 } else { lo_ms });
            rd.end_ms = last.unwrap_or(declared as i32).min(if hi_ms == i32::MAX { i32::MAX } else { hi_ms });
            Ok(())
        });
        if let Err(e) = r {
            die(format!("rewriting the telemetry record: {}", e));
        }
        let _ = std::fs::remove_file(&tmp);
    } else {
        std::fs::rename(&tmp, out).unwrap_or_else(|e| die(format!("{}: {}", out, e)));
    }

    // ---- the gate: re-read what was written and check every claim -------
    let c2 = Container::load(out).unwrap_or_else(|e| die(e));
    let t2 = Tape::from_file(out).unwrap_or_else(|e| die(e));
    let mut fail: Vec<String> = Vec::new();
    let a2 = &t2.archives[0];
    if a2.packets.len() != nt.archives[0].packets.len() {
        fail.push("tick count changed on write".into());
    }
    if a2.start_offset_ms as i64 != new_span_lo {
        fail.push("start_offset_ms is not what was written".into());
    }
    for (i, (p, q)) in nt.archives[0].packets.iter().zip(a2.packets.iter()).enumerate() {
        if p.steer != q.steer || p.accel != q.accel || p.brake != q.brake {
            fail.push(format!("tick {} does not read back", i));
            break;
        }
    }
    if added > 0 {
        // The appended ticks, read back off the disk: the inputs the last
        // recorded tick carried, held, with no respawn.
        let src = &a2.packets[a2.packets.len() - added - 1];
        let (s, ac, br) = (src.steer, src.accel, src.brake);
        for (k, p) in a2.packets[a2.packets.len() - added..].iter().enumerate() {
            if p.steer != s || p.accel != ac || p.brake != br {
                fail.push(format!("appended tick {} does not hold the last input", k));
                break;
            }
            if p.respawn() {
                fail.push(format!("appended tick {} reads as a respawn", k));
                break;
            }
        }
    }
    let dts: Vec<u32> = c2.declared_times().into_iter().map(|x| x.1).collect();
    if dts.iter().any(|v| *v as i64 != declared) {
        fail.push(format!("declared time copies disagree: {:?}", dts));
    }
    if let Ok(d) = gbx::record::decode_ghost(out) {
        if let Some(s) = d.samples.last() {
            if (s.time_ms as i64) > to {
                fail.push(format!("a telemetry sample survives at {}", secs(s.time_ms as i64)));
            }
        }
        if let Some(s) = d.samples.first() {
            if from != i64::MIN && (s.time_ms as i64) < from {
                fail.push(format!("a telemetry sample survives at {}", secs(s.time_ms as i64)));
            }
        }
        if (d.end_ms as i64) > to && to != i64::MAX && added == 0 {
            fail.push(format!("the record span still ends at {}", secs(d.end_ms as i64)));
        }
    }
    if !fail.is_empty() {
        for f in &fail {
            eprintln!("  FAIL {}", f);
        }
        die("the trimmed file failed its own coherence gate; it has been left in place for inspection");
    }

    println!("wrote {}", out);
    println!("  window          {} .. {}", secs(new_span_lo), secs(new_span_hi));
    if added > 0 {
        println!(
            "  ticks           {} -> {}  (head -{}, tail -{}, appended +{})",
            t.n(),
            t2.n(),
            cut_head,
            cut_tail,
            added
        );
    } else {
        println!("  ticks           {} -> {}  (head -{}, tail -{})", t.n(), t2.n(), cut_head, cut_tail);
    }
    println!("  declared        {}  in {} copies, all equal", secs(declared), dts.len());
    println!(
        "  checkpoints     {:?}  (was {:?})",
        splits.iter().map(|s| secs(*s as i64)).collect::<Vec<_>>(),
        splits_before.iter().map(|s| secs(*s as i64)).collect::<Vec<_>>()
    );
    if had_record {
        println!("  telemetry       {} samples kept, {} dropped", kept, dropped);
    } else if !cutting {
        println!("  telemetry       untouched (nothing was cut)");
    } else {
        println!("  telemetry       none in this container");
    }
    if added > 0 {
        println!(
            "  NOTE: {} ticks were APPENDED, holding the last recorded input. The telemetry, the\n\
             \x20       declared time and the splits are unchanged, because the appended ticks are\n\
             \x20       after the finish. The control for that is the oracle: this file must\n\
             \x20       re-simulate to the same time as the one it came from.",
            added
        );
    }
    if cut_head > 0 {
        println!(
            "  NOTE: this is a HEAD cut. The tape now starts at {} with the car already moving, \n\
             \x20       so re-simulating it from a standing start is a DIFFERENT run. The file is \n\
             \x20       coherent as a RECORDING; it is not a tape you can hand to the oracle.",
            secs(new_span_lo)
        );
    }
    if let Some(sd) = flag(a, "--server").or(std::option_env!("TM_SERVER")) {
        if cut_head == 0 && !has(a, "--no-oracle") {
            let mapf = flag(a, "--map");
            let server = oracle::server_dir(Some(sd));
            let mode = match mapf {
                Some(m) => MapsMode::One(std::path::Path::new(m)),
                None => MapsMode::Empty,
            };
            match oracle::validate(&server, std::path::Path::new(out), mode, "trim") {
                Ok(r) => println!("  oracle          {} (cps {:?})", r.secs(), r.cps),
                Err(e) => println!("  oracle          not run: {}", e),
            }
        }
    }
}

pub fn set_all_declared(body: &mut [u8], ms: u32) {
    let sites: Vec<usize> = gbx::container::all_skip_chunks(body)
        .into_iter()
        .filter(|c| c.0 == gbx::container::RACE_TIME_CHUNK)
        .map(|c| c.2)
        .collect();
    for o in sites {
        body[o..o + 4].copy_from_slice(&ms.to_le_bytes());
    }
}

/// Edit the ghost-result chunk `0x0309202B` and write it back.
///
/// The chunk is NOT a bare split vector -- treating it as one and filtering it
/// scrambles the file. It is decoded ONCE, in `gbx::container::GhostResult`;
/// this is the write half, and every writer in this crate goes through it so
/// the count word, the entry list, the terminator and the chunk's own size stay
/// one fact rather than four.
///
/// A file with no result chunk is not an error: some synthesised containers
/// have none, and there is then nothing to keep coherent.
pub fn rewrite_result(body: &[u8], f: impl FnOnce(&mut GhostResult)) -> Result<Vec<u8>, String> {
    let Some(mut r) = gbx::container::read_result(body) else {
        return Ok(body.to_vec());
    };
    f(&mut r);
    gbx::container::write_result(body, &r)
}
