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
    // Scratch names: strip the output's own extension before appending, so we
    // get `out.film.rg.Ghost.Gbx` and not `out.Ghost.Gbx.film.rg.Ghost.Gbx`.
    // A DOUBLED extension is not cosmetic here -- the server's own name
    // handling keys off the suffix, and the first version of this command
    // produced scratch files it silently ignored. Keep exactly one.
    let stem = out
        .strip_suffix(".Ghost.Gbx")
        .or_else(|| out.strip_suffix(".Replay.Gbx"))
        .unwrap_or(&out)
        .to_string();
    let scratch = format!("{}.film", stem);
    let s = |n: &str| format!("{}.{}.Ghost.Gbx", scratch, n);
    let owned = |v: &[&str]| v.iter().map(|x| x.to_string()).collect::<Vec<_>>();

    // ---- 0. What does the tape actually run to, and what does it claim? ----
    // The declared time comes from the CARRIER and is routinely a lap this run
    // never drives. Step 4 reconciles them; we read it here so the log says so.
    let chain = run(&me, &owned(&["record", "chain", &inp]), "reading the input");
    println!("== film: input\n{}", chain.lines().take(2).collect::<Vec<_>>().join("\n"));

    let span = flag(a, "--span").map(String::from).unwrap_or_else(|| {
        // Default: the file's DECLARED time, not the last sample's timestamp.
        //
        // The record's own end is the last SAMPLE, which lands on the tick
        // before the finish -- 10.600 for a 10.640 run at a 20 ms period. A
        // grid rebuilt to 10600 is 40 ms short of the lap, so the regenerated
        // car stops before the line and the last frames of the film show the
        // finish being crossed by nothing. Measured on 203072.
        //
        // `declared_ms` reads the same number `verify`'s V2 census gates on,
        // which is the run's own time.
        declared_ms(&me, &inp)
    });

    // ---- 1. Strip the donor's grid coverage (trap 1) ------------------------
    println!("== film: 1/6 stripping the donor grid at 20 ms (span {} ms)", span);
    run(
        &me,
        &owned(&["record", "rebuild", &inp, &s("rb"), "--span", &span, "--period", "20"]),
        "record rebuild",
    );

    // ---- 2. Regenerate (trap 1's other half) --------------------------------
    println!("== film: 2/6 regenerating from engine memory");
    let mut rg = owned(&[
        "regen", &s("rb"), &s("rg"), "--map", &map, // carrier and neutralise are unconditional in regen now
    ]);
    // NO --spawn-ref. It was passed here defensively (a rebuilt grid does not
    // move, so it cannot be its own spawn reference) and it silently COSTS THE
    // CARRIER: measured on 203072, the same rebuild regenerates with b76
    // sweeping to 16 and b89 to 73 without the flag, and frozen at 0 and 1
    // with it. A dead reactor in every film is a far worse failure than the
    // spawn check it was buying, and G2's own refusals on 287431 were false
    // (2.8 m at the landing after a 646 m freefall). The spawn is checked
    // where it belongs -- against the reference's entity shape in step 3 --
    // and the carrier gate in step 6 now catches the very thing this flag
    // broke.
    if let Some(sr) = flag(a, "--spawn-ref") {
        rg.push("--spawn-ref".into());
        rg.push(sr.to_string());
    }
    if has(a, "--expect-dnf") {
        rg.push("--expect-dnf".into());
    }
    run(&me, &rg, "regen");

    // ---- 3. Put the container back and MATCH THE REFERENCE (traps 2, 3) -----
    println!("== film: 3/6 grafting the scene back and matching {}", refr);
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
    println!("== film: 4/6 reconciling the record with the declared time");
    let decl = declared_ms(&me, &staged);
    let trimmed = s("tr");
    run(
        &me,
        &owned(&["trim", &staged, &trimmed, "--to", &decl, "--declare", &decl]),
        "trim to the declared time",
    );

    // ---- 5. NOTHING OF THE DONOR'S MAY SHIP (the traps nobody had numbered) --
    //
    // A container we inherited carries its owner, not just their car. On the
    // YEET donor `ghost inspect` lists: two skin archive paths, a
    // storageObjects locator URL, a ranked badge ("Prestige=Yes&Level=6&..."),
    // the display name "Bonobo.e" and the trigram "DIH". Every clip built on
    // that container published a stranger's identity next to our run, and no
    // gate said a word.
    //
    // The splits are the same defect one layer down. The result chunk carries
    // the DONOR's intermediate checkpoint times; `declare` only ever rewrote
    // the final one, so a file would assert it matched a human's first six
    // checkpoints to the millisecond and then gained three seconds in the last
    // segment. `--cps N` writes 0.000 for the intermediates, which honestly
    // reads as "this file does not know its splits" -- far better than a
    // number that is someone else's.
    println!("== film: 5/6 stripping the donor's identity and splits");
    // `identity set` EXITS NONZERO when there is nothing to change, and a
    // grafted container often is already clean. That is a success for our
    // purposes, so run it permissively and let the leak check below be the
    // judge -- it reads the OUTPUT, so it cannot be fooled by which branch we
    // took here.
    let ided = s("id");
    let id_args = owned(&[
        "identity", "set", &trimmed, &ided, "--name", "TAS", "--trigram", "TAS", "--skin",
        "default",
    ]);
    let id_ok = Command::new(&me)
        .args(&id_args)
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);
    let pre_declare = if id_ok && Path::new(&ided).exists() {
        ided.clone()
    } else {
        println!("   identity: nothing to change (already clean)");
        trimmed.clone()
    };
    let cps = cp_count(&me, &refr);
    run(
        &me,
        &owned(&["declare", &pre_declare, &out, "--time", &decl, "--cps", &cps]),
        "declare (blank the donor's splits)",
    );
    let leaked = run(&me, &owned(&["identity", "show", &out]), "identity show");
    for bad in ["Prestige=", "storageObjects", "trackmania.live"] {
        if leaked.contains(bad) {
            die(format!(
                "film: {} still carries the donor's {} -- do not publish it. \
                 Run `ghost identity show` on the output to see what survived.",
                out, bad
            ));
        }
    }

    // ---- 6. Prove it, and say what is NOT proven ---------------------------
    println!("== film: 6/6 verifying");
    let v = run(&me, &owned(&["verify", &out, "--map", &map]), "verify");
    for l in v.lines().filter(|l| l.contains("V5") || l.contains("V6")) {
        println!("{}", l);
    }
    carrier_check(&me, &out, Some(&refr));
    let kappa_ok = v.lines().any(|l| l.contains("V6") && l.contains("kappa 1.000"));
    if !kappa_ok {
        die(format!(
            "film: V6 is not kappa 1.000 -- the recording in {} is NOT this tape's run. \
             Do not render it.",
            out
        ));
    }

    if !has(a, "--keep-scratch") {
        for n in ["rb", "rg", "car", "sc", "ord", "tr", "id"] {
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

/// Which entity index holds the car, read from `record show`'s OWN marker.
///
/// This used to count lines after "lives" in `record chain` and take the
/// second whitespace token, which on a rebuilt grid picks up a word and
/// panics downstream in `record ents` with `ParseIntError` -- a failure two
/// commands away from its cause. `record show` labels the car explicitly
/// ("<- the car this project reads"); parse the label, not a position.
fn car_index(me: &str, f: &str) -> String {
    let c = run(me, &["record".into(), "show".into(), f.into()], "record show");
    c.lines()
        .find(|l| l.contains("the car this project reads"))
        .and_then(|l| {
            l.split_whitespace()
                .nth(1)
                .map(|x| x.trim_end_matches(':').to_string())
        })
        .filter(|x| x.parse::<usize>().is_ok())
        .unwrap_or_else(|| {
            die(format!(
                "film: could not find the car entity in {} -- `record show` printed no \
                 line marked as the car. Run it by hand and look at the entity list.",
                f
            ))
        })
}

/// REFUSE a file whose visual channels never move.
///
/// The bug this exists for: `regen` writing 22 of the 116 sample bytes and
/// zeroing the other 94. The car then reports no speed, no rpm, gear 0 and no
/// ground contact, and the CLIENT DRAWS IT AS A TRANSPARENT WIREFRAME -- or,
/// with the reactor members dead, drives the whole run with the booster
/// unlit. Three clips shipped invisible and one shipped with no reactor, and
/// every one of them passed V2, V5 and V6 at kappa 1.000: kappa compares the
/// tape to the record and has no opinion about whether a car is rendered.
///
/// Bytes 89, 90, 91 and 76 carry the five packed reactor members; a real run
/// moves them constantly. If they hold one value for the whole lap, the
/// carrier did not run, and the only honest thing to do is say so BEFORE a
/// human watches the video.
fn carrier_is_live(me: &str, f: &str) {
    carrier_check(me, f, None)
}

/// The same check with a POSITIVE CONTROL: a byte the reference moves and we
/// do not is under-authored, and no absolute threshold can tell you that.
///
/// Without the control this check can only catch a TOTAL freeze. The reactor
/// bug on 203072 froze three of the four bytes and left one twitching, which
/// reads as "live" on its own and as an obvious defect beside a real
/// recording: the reference swept b76 to 16 and b89 to 73 over the same lap
/// where ours held 0 and 1.
fn carrier_check(me: &str, f: &str, control: Option<&str>) {
    let read = |p: &str| -> Vec<Vec<u32>> {
        let d = run(
            me,
            &[
                "record".into(),
                "dumpbytes".into(),
                p.into(),
                "--bytes".into(),
                "76,89,90,91".into(),
            ],
            "record dumpbytes",
        );
        d.lines()
            .filter_map(|l| {
                let v: Vec<u32> = l.split_whitespace().filter_map(|x| x.parse().ok()).collect();
                (v.len() == 5).then_some(v[1..].to_vec())
            })
            .collect()
    };
    let spread = |rows: &[Vec<u32>], i: usize| -> u32 {
        let (mut lo, mut hi) = (u32::MAX, 0u32);
        for r in rows {
            lo = lo.min(r[i]);
            hi = hi.max(r[i]);
        }
        hi.saturating_sub(lo)
    };

    let rows = read(f);
    if rows.len() < 8 {
        println!("   carrier: only {} sample rows read -- not checked", rows.len());
        return;
    }
    let names = ["b76", "b89", "b90", "b91"];
    let ours: Vec<u32> = (0..4).map(|i| spread(&rows, i)).collect();

    if ours.iter().all(|s| *s == 0) {
        die(format!(
            "film: THE CARRIER DID NOT RUN. Bytes 76, 89, 90 and 91 hold one value for \
             all {} samples of {}, so the reactor and the wheels are frozen: the render \
             will show a car that never fires its booster, and with the rest of the 94 \
             carrier bytes dead the client may draw it as a TRANSPARENT WIREFRAME. \
             That is what `regen` produces when the field gather is skipped -- check \
             that its log mentions the carrier and that `want_fields` is true in \
             regen.rs. Every other gate passes on this file, which is the whole \
             problem.",
            rows.len(),
            f
        ));
    }

    if let Some(c) = control {
        let cr = read(c);
        if cr.len() >= 8 {
            let dead: Vec<&str> = (0..4)
                .filter(|i| ours[*i] == 0 && spread(&cr, *i) > 4)
                .map(|i| names[i])
                .collect();
            if !dead.is_empty() {
                die(format!(
                    "film: UNDER-AUTHORED CARRIER. {} never move in {}, but the \
                     reference {} sweeps them over the same lap -- so these are \
                     channels this map's car really does drive, and ours are dead. \
                     The render will be missing whatever they carry (the reactor \
                     flame, the wheel rotation). This is a partial version of the \
                     total freeze above and no absolute threshold catches it; only \
                     the comparison does.",
                    dead.join(", "),
                    f,
                    c
                ));
            }
            println!(
                "PASS carrier: every channel the reference drives is driven here too \
                 ({} samples, control {})",
                rows.len(),
                c
            );
            return;
        }
    }

    let dead: Vec<&str> = (0..4).filter(|i| ours[*i] == 0).map(|i| names[i]).collect();
    if dead.is_empty() {
        println!("PASS carrier: bytes 76/89/90/91 all move across {} samples", rows.len());
    } else {
        println!(
            "   carrier: {} never move -- NOT PROVEN either way without a control; \
             pass --ref a real recording of this map to settle it",
            dead.join(", ")
        );
    }
}

/// How many checkpoints this map's runs cross, read from the reference.
///
/// `declare --cps N` needs it to blank the right number of intermediate
/// splits. Reading it from the reference rather than a flag means one less
/// thing for a caller to get wrong, and the reference is by definition a real
/// run of this map.
fn cp_count(me: &str, f: &str) -> String {
    let c = run(me, &["inspect".into(), f.into()], "inspect");
    c.lines()
        .find(|l| l.contains("checkpoints, the last is the finish"))
        .and_then(|l| {
            l.split('(')
                .nth(1)
                .and_then(|r| r.split_whitespace().next())
                .map(|x| x.to_string())
        })
        .filter(|x| x.parse::<usize>().is_ok())
        .unwrap_or_else(|| "1".to_string())
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
