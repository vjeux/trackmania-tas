//! `ghost declare` -- write the time the file ACTUALLY does into every copy of
//! it, asked of the plain oracle.
//!
//! Moved out of `main.rs` because `regen`'s finishing pass calls it on every
//! run: a regenerated file's declared time is the CARRIER's until something
//! rewrites it, and "something" was a person remembering. A CLI entry point
//! that only main.rs can reach is a step that can be skipped.

use crate::cli::{die, flag, has, num};
use crate::{oracle, trim, Container};
use gbx::container;
use gbx::container::secs;

/// `ghost declare` -- set the time the file DECLARES.
///
/// After the inputs change, the declared time is the old run's. Every check
/// that compares "what the file says" with "what the file does" then compares a
/// stale number, and a file that declares somebody else's time while validating
/// its own is exactly the shape of the container bugs that cost this project
/// five withdrawn clips.
///
/// `--from-oracle` is the form to use: it asks the plain oracle what the file
/// actually does and writes THAT, so the number is never typed by a human and
/// never copied out of a search log.
pub fn cmd(a: &[String]) {
    let inp = a.first().unwrap_or_else(|| die("ghost declare IN OUT (--time MS | --from-oracle --map M) [--splits MS,MS,...] [--cps N]"));
    let out = a.get(1).unwrap_or_else(|| die("ghost declare IN OUT (--time MS | --from-oracle --map M) [--splits MS,MS,...] [--cps N]"));
    let c = Container::load(inp).unwrap_or_else(|e| die(e));
    let ms: i64 = if has(a, "--from-oracle") {
        let server = oracle::server_dir(flag(a, "--server"));
        let mode = match flag(a, "--map") {
            Some(m) => oracle::MapsMode::One(std::path::Path::new(m)),
            None => oracle::MapsMode::Empty,
        };
        match oracle::validate(&server, std::path::Path::new(inp), mode, "declare") {
            Err(e) => die(format!("the oracle could not run: {}", e)),
            Ok(r) => match r.time_ms {
                Some(t) => {
                    println!("the plain oracle re-simulated {} to {}", inp, secs(t));
                    t
                }
                None => die(format!(
                    "the oracle returns DNF (cps {:?}) for this file, so there is no time to \
                     declare. A partial run should keep the window's end: use --time MS.",
                    r.cps
                )),
            },
        }
    } else {
        num(a, "--time").unwrap_or_else(|| die("give --time MS or --from-oracle --map M"))
    };
    let mut body = c.body().to_vec();
    trim::set_all_declared(&mut body, ms as u32);
    // The ghost-result chunk carries the race time AND the checkpoint list, and
    // the server compares the LENGTH of that list with the map's checkpoint
    // count before it simulates anything.
    let want_cps = num(a, "--cps");
    let want_splits: Option<Vec<i32>> = flag(a, "--splits").map(|s| {
        s.split(',')
            .map(|x| {
                x.trim().parse::<i32>().unwrap_or_else(|_| {
                    die(format!("--splits wants milliseconds, comma separated; got {:?}", x))
                })
            })
            .collect()
    });
    let before: Vec<i32> = c.splits();
    let body = trim::rewrite_result(&body, |r| {
        r.race_ms = ms as i32;
        if let Some(sp) = &want_splits {
            // THE SPLITS ARE A CLAIM, AND THEY WERE THE CARRIER'S.
            //
            // `--time` rewrites the race time and the LAST entry and leaves
            // every intermediate one alone -- so a searched tape on a borrowed
            // container declares its own finish beside the donor's checkpoint
            // times. On 134672 that reads 13.906 / 33.106 / 45.437 / 63.812 /
            // 67.200: four of another driver's splits and one of ours, in one
            // list, with nothing in the file to say which is which. The
            // deleted `u02 declare --splits` could write them and its
            // replacement could not, and every regenerated ghost since has
            // carried the gap.
            //
            // Measure them the way this toolchain measures a split -- one
            // verified segment map per checkpoint, `tmmaps segments` then
            // `tmmaps oracle` -- and pass them here. Milliseconds, like
            // `--time`. The read-back control below requires the last one to
            // BE the declared time, which is the one relation that holds on
            // every reference ghost in the corpus.
            r.entries = sp.iter().map(|t| (*t, 1)).collect();
        } else if let Some(n) = want_cps {
            // Every intermediate entry becomes 0.000. This is the borrowed-
            // container case by construction -- the count only changes when the
            // file moved to a map with a different number of checkpoints -- and
            // every intermediate time in the list is then the DONOR map's,
            // measured on a route this file no longer drives. A zero says "this
            // container does not know its intermediate splits"; carrying the
            // donor's numbers forward would say something false and look right.
            //
            // WHAT THIS IS AND IS NOT FOR. The audit that asked for `--cps`
            // believed the server refused a count mismatch as `wrong simu`
            // without simulating. It does not: measured on two maps and six
            // counts, a finishing tape validates whatever its declared count
            // says, and `wrong simu` is what the server returns when the
            // simulation does not reproduce the DECLARED RESULT -- on a
            // partial run it even says how far it got (`wrong simu, but
            // reached some checkpoints (1 out of 2)`), which is a simulation,
            // not a pre-check. The count is a claim this toolchain reads:
            // `tmmaps` builds a segment map per declared split and refuses a
            // ghost whose count is not the map's.
            let n = n.clamp(1, 199) as usize;
            r.entries = (0..n).map(|k| if k + 1 == n { (ms as i32, 1) } else { (0, 0) }).collect();
        } else if let Some(last) = r.entries.last_mut() {
            // The final split is the race time. That holds on every reference
            // ghost in the corpus, so a `--time` that left it behind would
            // produce a file whose own last checkpoint disagrees with its own
            // declared time.
            last.0 = ms as i32;
        }
    })
    .unwrap_or_else(|e| die(e));
    let stage = format!("{}.declare-stage", out);
    container::write_gbx(&c.gbx, body, &stage).unwrap_or_else(|e| die(e));
    // The telemetry record declares its own span, separately from the samples.
    // Leaving it at the old run's is the same defect one level down, and
    // `ghost verify` reports it, so fix it here rather than print it later.
    let mut span_note = String::new();
    if gbx::recwrite::find_rec_site(&Container::load(&stage).unwrap().gbx.body).is_ok() {
        let r = gbx::recwrite::rewrite_ghost(&stage, out, |rd| {
            let last = rd.ents.iter().filter_map(|e| e.times.last().copied()).max().unwrap_or(0);
            rd.end_ms = (ms as i32).max(last);
            Ok(())
        });
        match r {
            Ok(_) => {
                let _ = std::fs::remove_file(&stage);
                span_note = format!("  the record's own span now ends at {}", secs(ms));
            }
            Err(e) => {
                std::fs::rename(&stage, out).ok();
                span_note = format!("  the record span was left alone: {}", e);
            }
        }
    } else {
        std::fs::rename(&stage, out).unwrap_or_else(|e| die(format!("{}: {}", out, e)));
    }
    let c2 = Container::load(out).unwrap_or_else(|e| die(e));
    let dt: Vec<u32> = c2.declared_times().into_iter().map(|x| x.1).collect();
    if dt.iter().any(|v| *v as i64 != ms) {
        die(format!("read-back control FAILED: declared copies are {:?}", dt));
    }
    let after: Vec<i32> = c2.splits();
    if let Some(sp) = &want_splits {
        if after != *sp {
            die(format!(
                "read-back control FAILED: asked for splits {:?}, the file declares {:?}",
                sp, after
            ));
        }
        match sp.last() {
            Some(last) if *last as i64 == ms => {}
            Some(last) => die(format!(
                "the last split is {} and the declared time is {}. On every reference ghost in \
                 this corpus the final entry IS the race time, so a file written this way would \
                 contradict itself.",
                secs(*last as i64),
                secs(ms)
            )),
            None => die("--splits was empty"),
        }
    }
    if let Some(n) = want_cps {
        if after.len() as i64 != n.clamp(1, 199) {
            die(format!(
                "read-back control FAILED: asked for {} checkpoints, the file declares {}",
                n,
                after.len()
            ));
        }
    }
    if let Some(r) = c2.result() {
        if r.race_ms as i64 != ms {
            die(format!(
                "read-back control FAILED: the result chunk declares {} and the header {}",
                secs(r.race_ms as i64),
                secs(ms)
            ));
        }
    }
    println!("wrote {}", out);
    println!("  declared {} in {} copies, all equal (read-back control OK)", secs(ms), dt.len());
    println!("  the ghost-result chunk's race time was set to the same value");
    if want_splits.is_some() {
        println!(
            "  checkpoints {:?}  (was {:?})",
            after.iter().map(|s| secs(*s as i64)).collect::<Vec<_>>(),
            before.iter().map(|s| secs(*s as i64)).collect::<Vec<_>>()
        );
    }
    if want_cps.is_some() {
        println!(
            "  checkpoints {:?}  (was {:?})",
            after.iter().map(|s| secs(*s as i64)).collect::<Vec<_>>(),
            before.iter().map(|s| secs(*s as i64)).collect::<Vec<_>>()
        );
        println!(
            "  NOTE: the intermediate times are written as 0.000 because this file's old ones\n\
             \x20       were another map's, and 0.000 reads as \"this container does not know its\n\
             \x20       splits\" where a donor's number would read as a measurement. MEASURED on\n\
             \x20       the dedicated server (build 2026-05-15): neither the COUNT nor the VALUES\n\
             \x20       gate validation -- counts of 1, 2, 3, 5 and 9 on a 4-checkpoint map all\n\
             \x20       come back IsValid true at the same time, and so do zeroed splits. The\n\
             \x20       count matters to THIS toolchain: a segment map is built against the\n\
             \x20       ghost's declared splits, and tmmaps refuses when there are the wrong\n\
             \x20       number of them."
        );
    }
    if !span_note.is_empty() {
        println!("{}", span_note);
    }
}
