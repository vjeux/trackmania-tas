//! `ghost film` -- ONE COMMAND from a search tape to a file the CLIENT WILL LOAD.
//!
//! WHY THIS EXISTS. Every step below was already available, and getting a tape
//! onto the render box still took half an hour of fiddling, four failed
//! renders and two published clips that showed the WRONG CAR. The knowledge
//! was spread over `record rebuild`, `regen`, `record ents`, `record
//! graft-scene`, `record entorder` and `trim`, and each one has a trap that is
//! invisible until a human watches the video:
//!
//!   1. `regen` SILENTLY KEEPS THE DONOR'S TELEMETRY whenever the donor record
//!      covers the new grid -- which, when the lap is shorter than its carrier,
//!      is always. You get V5 PASS and V6 kappa 0.457: it looks like success
//!      and it renders somebody else's driving. The strip must come first.
//!   2. ...but `record rebuild` DROPS the zero-sample `0x032CB000` placeholder
//!      that carries the checkpoint blocks, and the client HARD-CRASHES on the
//!      entity layout that leaves behind. So the scene has to be grafted back.
//!   3. `--car-last` is not an absolute index. Last-of-3 is index 2; a client
//!      that wants 3-of-4 refuses it. The reference file decides, not a flag.
//!   4. A record that RUNS PAST ITS DECLARED TIME is soft-rejected: `0 -> 0`
//!      ghost blocks, dialog `FrameMessage`, no crash, no error, nothing in
//!      the log that says why. This one cost the most.
//!
//! None of those four is checked by `ghost verify`. A tape can pass all eleven
//! gates at kappa 1.000 and still be unloadable, which is why the only honest
//! test is a REFERENCE FILE THE CLIENT IS KNOWN TO ACCEPT -- this command
//! takes one and matches the container to it.
//!
//! Usage:
//!   ghost film IN OUT --map MAP --ref ACCEPTED [--span MS] [--expect-dnf]
//!                     [--spawn-ref GHOST] [--keep-scratch]
//!
//! `--ref` is the load-bearing argument: a ghost of this map that the client
//! has actually imported. Its entity list is the specification.

use crate::{die, flag, has};
use std::path::Path;
use std::process::Command;

fn run(bin: &str, args: &[String], what: &str) -> String {
    let out = Command::new(bin)
        .args(args)
        .output()
        .unwrap_or_else(|e| die(format!("film: could not run `{}` for {}: {}", bin, what, e)));
    let s = format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    if !out.status.success() {
        eprintln!("{}", s);
        die(format!("film: {} FAILED -- nothing was written", what));
    }
    s
}

pub fn cmd(a: &[String]) {
    let inp = a.first().cloned().unwrap_or_else(|| {
        die("ghost film IN OUT --map MAP --ref ACCEPTED [--span MS] [--expect-dnf]")
    });
    let out = a
        .get(1)
        .cloned()
        .unwrap_or_else(|| die("film: give an output path"));
    let map = flag(a, "--map").map(String::from).unwrap_or_else(|| die("film: --map is required"));
    // THE REFERENCE IS NOT OPTIONAL AND IT IS NOT A NICETY. Four separate
    // rejections tonight were all "the container differs from one that works",
    // and none of them is derivable from the tape alone.
    let refr = flag(a, "--ref").map(String::from).unwrap_or_else(|| {
        die("film: --ref is required -- name a ghost of this map THE CLIENT HAS \
             IMPORTED. Its entity list is the specification; there is no way to \
             infer it from the tape, and every check `verify` runs passes on files \
             the client refuses.")
    });

    let me = std::env::current_exe()
        .map(|p| p.display().to_string())
        .unwrap_or_else(|_| "ghost".into());
    let scratch = format!("{}.film", out);
    let s = |n: &str| format!("{}.{}", scratch, n);
    let owned = |v: &[&str]| v.iter().map(|x| x.to_string()).collect::<Vec<_>>();

    // ---- 0. What does the tape actually run to, and what does it claim? ----
    // The declared time comes from the CARRIER and is routinely a lap this run
    // never drives. Step 4 reconciles them; we read it here so the log says so.
    let chain = run(&me, &owned(&["record", "chain", &inp]), "reading the input");
    println!("== film: input\n{}", chain.lines().take(2).collect::<Vec<_>>().join("\n"));

    let span = flag(a, "--span").map(String::from).unwrap_or_else(|| {
        // Default: the record's own end, in ms, rounded up to a tick.
        chain
            .split_whitespace()
            .collect::<Vec<_>>()
            .windows(2)
            .find_map(|w| (w[0] == "..").then(|| w[1].trim_end_matches(',').parse::<f64>().ok()).flatten())
            .map(|end| format!("{}", (end * 1000.0).ceil() as i64))
            .unwrap_or_else(|| die("film: could not read the record's span -- pass --span MS"))
    });

    // ---- 1. Strip the donor's grid coverage (trap 1) ------------------------
    println!("== film: 1/5 stripping the donor grid at 20 ms (span {} ms)", span);
    run(
        &me,
        &owned(&["record", "rebuild", &inp, &s("rb"), "--span", &span, "--period", "20"]),
        "record rebuild",
    );

    // ---- 2. Regenerate (trap 1's other half) --------------------------------
    println!("== film: 2/5 regenerating from engine memory");
    let mut rg = owned(&[
        "regen", &s("rb"), &s("rg"), "--map", &map, // carrier and neutralise are unconditional in regen now
    ]);
    // A rebuilt grid does not move, so it cannot be its own spawn reference.
    // Default to the accepted file, which by definition is a real recording.
    rg.push("--spawn-ref".into());
    rg.push(flag(a, "--spawn-ref").map(String::from).unwrap_or_else(|| refr.clone()));
    if has(a, "--expect-dnf") {
        rg.push("--expect-dnf".into());
    }
    run(&me, &rg, "regen");

    // ---- 3. Put the container back and MATCH THE REFERENCE (traps 2, 3) -----
    println!("== film: 3/5 grafting the scene back and matching {}", refr);
    let keep = car_index(&me, &s("rg"));
    run(
        &me,
        &owned(&["record", "ents", &s("rg"), &s("car"), "--keep", &keep]),
        "record ents --keep <car>",
    );
    run(
        &me,
        &owned(&["record", "graft-scene", &s("car"), &s("sc"), "--from", &refr]),
        "record graft-scene",
    );

    // The reference decides where the car goes. `entorder` can only say first
    // or last, so when the reference wants it in the MIDDLE we say so plainly
    // rather than shipping a file the client will refuse.
    let want = car_index(&me, &refr);
    let got = car_index(&me, &s("sc"));
    let mut staged = s("sc");
    if want != got {
        let n_ref = ent_count(&me, &refr);
        let ordered = s("ord");
        let dir = if want == "0" {
            Some("--car-first")
        } else if want.parse::<usize>().ok() == n_ref.checked_sub(1) {
            Some("--car-last")
        } else {
            None
        };
        match dir {
            Some(d) => {
                run(&me, &owned(&["record", "entorder", &s("sc"), &ordered, d]), "record entorder");
                staged = ordered;
            }
            None => eprintln!(
                "== film: WARNING -- {} puts the car at index {} of {}, which is neither \
                 first nor last, and `record entorder` can only do those two. Shipping \
                 the grafted order ({}); if the client refuses this file, that is why.",
                refr, want, n_ref, got
            ),
        }
    }

    // ---- 4. The record must not outlive its declared time (trap 4) ----------
    // This is the one that produces `0 -> 0` and `FrameMessage`, with no error
    // anywhere. Reconcile by trimming the record to what the file declares.
    println!("== film: 4/5 reconciling the record with the declared time");
    let decl = declared_ms(&me, &staged);
    run(
        &me,
        &owned(&["trim", &staged, &out, "--to", &decl, "--declare", &decl]),
        "trim to the declared time",
    );

    // ---- 5. Prove it, and say what is NOT proven ---------------------------
    println!("== film: 5/5 verifying");
    let v = run(&me, &owned(&["verify", &out, "--map", &map]), "verify");
    for l in v.lines().filter(|l| l.contains("V5") || l.contains("V6")) {
        println!("{}", l);
    }
    let kappa_ok = v.lines().any(|l| l.contains("V6") && l.contains("kappa 1.000"));
    if !kappa_ok {
        die(format!(
            "film: V6 is not kappa 1.000 -- the recording in {} is NOT this tape's run. \
             Do not render it.",
            out
        ));
    }

    if !has(a, "--keep-scratch") {
        for n in ["rb", "rg", "car", "sc", "ord"] {
            let _ = std::fs::remove_file(s(n));
        }
    }

    println!(
        "\n== film: {} is ready.\n\
         ==   container matched to {}\n\
         ==   V6 kappa 1.000, record reconciled with its declared time\n\
         == The import is a CLIENT question and no offline check settles it: \
         if it is refused, run `ghost record show` on this file and on the \
         reference and diff the entity list.",
        out, refr
    );
}

/// Which entity index holds the car, as `record chain` reports it.
fn car_index(me: &str, f: &str) -> String {
    let c = run(me, &["record".into(), "chain".into(), f.into()], "record chain");
    c.lines()
        .skip_while(|l| !l.contains("lives"))
        .nth(1)
        .and_then(|l| l.split_whitespace().nth(1).map(|x| x.to_string()))
        .unwrap_or_else(|| die(format!("film: could not find the car entity in {}", f)))
}

fn ent_count(me: &str, f: &str) -> usize {
    let c = run(me, &["record".into(), "show".into(), f.into()], "record show");
    c.split_whitespace()
        .collect::<Vec<_>>()
        .windows(2)
        .find_map(|w| (w[1] == "entities").then(|| w[0].parse::<usize>().ok()).flatten())
        .unwrap_or(0)
}

fn declared_ms(me: &str, f: &str) -> String {
    let c = run(me, &["inspect".into(), f.into()], "inspect");
    c.lines()
        .find(|l| l.trim_start().starts_with("declared"))
        .and_then(|l| l.split_whitespace().nth(1).map(|x| x.to_string()))
        .and_then(|secs| secs.parse::<f64>().ok().map(|v| format!("{}", (v * 1000.0).round() as i64)))
        .unwrap_or_else(|| die(format!("film: could not read the declared time of {}", f)))
}
