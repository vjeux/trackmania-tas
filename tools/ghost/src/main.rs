//! `ghost` -- the TM2020 ghost/replay API.
//!
//! One binary, one job: every operation this project performs on a `.Ghost.Gbx`
//! or `.Replay.Gbx`, each with a control that proves it did what it says.
//!
//! Rust only. There is no interpreter anywhere in this pipeline and no shell
//! script carries any logic.

use ghost::cli::{die, has, need, num};
use gbx::container::{secs, set_embedded_map, Container};
use ghost::regen::raw_vehicle_samples;
use gbx::tape::{Encoding, Tape};
use gbx::{container, tape};
use ghost::{census, declare, engine, hdr, ident, map_uid_of, phase, record, regen, roundtrip, selftest, trim, verify};

const HELP: &str = r#"ghost -- the TM2020 ghost / replay API

  Every command that WRITES a file runs a control first and refuses rather than
  writing something that would be silently wrong. Times print as seconds.

INSPECT
  ghost inspect FILE [--ticks]
        Everything the file says about itself: container kind, the map it will
        actually run on, declared time and every copy of it, splits, identity,
        the input tape, and the telemetry record.
  ghost chunks FILE
        The skippable-chunk table, for forensics.

INPUTS  (operation 1 and 2)
  ghost tape extract FILE --out TAPE.gtape
        Full fidelity: every tick, every field the packet carries -- state word,
        respawn bit, mouse axes, steer / accel / brake, trigger fields, mode and
        flags. Round-trips byte-for-byte.
  ghost tape inject IN OUT --tape TAPE.gtape [--verbatim] [--allow-telemetry-mismatch]
        Write a tape back into a ghost. Default writes every vehicle field
        EXPLICITLY so no tick inherits the previous one's inputs; --verbatim
        reproduces the file's original coding exactly.
  ghost tape expand IN OUT
        Rewrite every "same as the previous tick" packet in its explicit form.
        Semantically a no-op -- and the oracle control says so -- but afterwards
        every tick is writable.
  ghost tape diff A.gtape B.gtape
        Per-tick differences between two tapes.
  ghost tape stats TAPE.gtape
        Tick count, input events, packet modes, respawns.
  ghost tape bits FILE...
        Which bits of the state literal actually vary across a corpus: the
        census that says what is still unnamed in the packet.

CAR STATE  (operation 3)
  ghost regen IN OUT --map MAP [--neutralise] [--inputs] [--trim-outside]
                              [--anchorticks a,b,c] [--noanchor]
        Run the real engine on this file's own inputs, capture per-sample car
        state and write it into the ghost, so the recorded trajectory MATCHES
        the tape. Refuses unless the acceptance gate passes.
        --neutralise ALSO zeros the 49 per-run bytes the transform encoder does
        not write (ground contact, wheels, rpm, suspension, surface effects).
        Without it those bytes stay the donor container's, which is what every
        C5/C6/C7 refusal in this corpus is; with it they are honestly absent,
        and `tmtraj check` C10 then fails by design.
        --inputs also rewrites the record's steer/gas/brake echo from the tape.
        --spawn-ref FILE names the recording G2 checks the start against, for a
        template whose own record is a rebuilt grid (a constant identifies
        nothing, and G2 says UNMEASURED rather than passing).
  ghost roundtrip GHOST --map MAP [--bar MM] [--keep]
        The end-to-end control: regenerate a recording the game itself wrote,
        from its own inputs, and require its own trajectory back. The answer key
        is in the file and nothing about it can be tuned. Floor 0.48-0.52 mm
        (client vs dedicated server); default bar 5 mm.
  ghost regen-control FILE --map MAP
        The fixed-point control: regenerate a ghost that already carries its own
        true telemetry and require the result to reproduce it.

MAP  (operation 4)
  ghost map show FILE
        Which map this file will actually run on -- and whether --map is real
        for it at all.
  ghost map extract FILE --out MAP.Map.Gbx
  ghost map set IN OUT --map MAP.Map.Gbx
        Replace the CARRIED map. This is the only thing that moves a recording
        onto another map: rewriting the uid does not.

TRIM  (operation 5)
  ghost trim IN OUT [--from MS] [--to MS] [--declare MS]
        Set the run's WINDOW, in race-time milliseconds. Cut the head and/or
        the tail, keeping the file coherent: inputs, telemetry samples, the
        record span, the splits and every copy of the declared time. A --to
        PAST the end of the tape LENGTHENS it instead, appending ticks that
        hold the last recorded input -- room for the car to keep driving after
        the recording stopped. One command owns a run's length in both
        directions.

DECLARE
  ghost declare IN OUT (--time MS | --from-oracle --map M) [--cps N]
        Set the time the file DECLARES, in every copy of it, and in the
        ghost-result chunk. --cps N also sets the NUMBER of checkpoint entries,
        which a container borrowed from another map gets wrong: it declares the
        DONOR map's checkpoint list. MEASURED, so that nobody re-derives it from
        the tool: the dedicated server does NOT gate on this count -- a file
        declaring 1, 2, 3, 5 or 9 checkpoints on a 4-checkpoint map still
        validates, and the mismatch shows up only as ValidatedResult and
        DeclaredResult disagreeing. What the count breaks is THIS toolchain:
        `tmmaps` refuses to build a segment map against a ghost whose split
        count is not the map's, because every rung would be verified against
        the wrong checkpoint.

IDENTITY  (operation 6)
  ghost identity show FILE
  ghost identity set IN OUT [--name N] [--trigram XXX] [--skin PATH|default]
                            [--login L] [--zone Z] [--clubtag T] [--anonymise]
                            [--pad-ids]
        Car skin, display name and 3-letter trigram. --anonymise also drops the
        account id and the storage locator URL, which are the two foreign
        identifiers a strip-list usually misses -- AND the driver block in the
        replay header and in body chunk 0x03093018, which are the two a
        body-only anonymiser misses. --pad-ids keeps the old behaviour of
        padding an unresizable id to `xxxx...` instead of removing it.
  ghost header show FILE
        The replay HEADER: its chunk table, the driver fields, the map's own
        attribution (which stays), and every copy of the race time the header
        holds -- the copies the body census cannot see. A plain .Ghost.Gbx has
        no replay header and says so.

VERIFY
  ghost verify FILE [--map MAP] [--expect-ms MS] [--server DIR]
        The acceptance gate: codec identity, tape/telemetry agreement, declared
        time census, container identity, and the plain oracle re-simulating THE
        WRITTEN FILE.
  ghost selftest [--server DIR] [--data DIR]
        The whole test suite, from one command.
"#;

fn main() {
    { gbx::container::lzo_init(); };
    let a: Vec<String> = std::env::args().skip(1).collect();
    if a.is_empty() || a[0] == "-h" || a[0] == "--help" || a[0] == "help" {
        println!("{}", HELP);
        std::process::exit(if a.is_empty() { 2 } else { 0 });
    }
    let sub = a[0].as_str();
    let rest = &a[1..];
    match sub {
        "inspect" => cmd_inspect(rest),
        "codeccheck" => {
            let t = Tape::from_file(&rest[0]).unwrap_or_else(|e| die(e));
            match t.verbatim_is_identity() {
                Ok(()) => println!("codec identity OK ({} ticks)", t.n()),
                Err(e) => {
                    println!("codec identity FAILED: {}", e);
                    if let Some(k) = t.first_divergent_packet(0) {
                        println!("first divergent packet: {}", k);
                        for i in k.saturating_sub(2)..(k + 3).min(t.archives[0].packets.len()) {
                            println!("  {}", {
                                let p = &t.archives[0].packets[i];
                                format!("t={} {:?}", i, p)
                            });
                        }
                    }
                    std::process::exit(1);
                }
            }
        }
        "trajdiff" => {
            // Compare two files' recorded trajectories, at every shift from
            // -3 to +3 samples. A one-sample offset is a PURE TIME SHIFT and
            // hides inside a small mean, so the shift is always reported.
            let a0 = gbx::record::decode_ghost(&rest[0]).unwrap_or_else(|e| die(e));
            let b0 = gbx::record::decode_ghost(&rest[1]).unwrap_or_else(|e| die(e));
            let n = a0.samples.len().min(b0.samples.len());
            println!("{} vs {}  ({} / {} samples)", rest[0], rest[1], a0.samples.len(), b0.samples.len());
            for k in -3i64..=3 {
                let (mut s, mut c, mut worst) = (0.0f64, 0usize, 0.0f64);
                for i in 0..n {
                    let j = i as i64 + k;
                    if j < 0 || j >= b0.samples.len() as i64 { continue }
                    let (p, q) = (&a0.samples[i], &b0.samples[j as usize]);
                    let d = ((p.x - q.x).powi(2) + (p.y - q.y).powi(2) + (p.z - q.z).powi(2)).sqrt();
                    s += d; worst = worst.max(d); c += 1;
                }
                if c > 0 {
                    println!("  shift {:+}: mean {:.6} m  worst {:.6} m  over {} samples", k, s / c as f64, worst, c);
                }
            }
        }
        "engine" => engine::cmd(rest),
        "chunks" => cmd_chunks(rest),
        "dump" => {
            let c = Container::load(&rest[0]).unwrap_or_else(|e| die(e));
            let at = num(rest, "--at").unwrap_or(0) as usize;
            let len = num(rest, "--len").unwrap_or(256) as usize;
            let b = &c.body()[at..(at + len).min(c.body().len())];
            for (i, row) in b.chunks(16).enumerate() {
                let hex: Vec<String> = row.iter().map(|x| format!("{:02x}", x)).collect();
                let asc: String = row
                    .iter()
                    .map(|x| if (0x20..0x7f).contains(x) { *x as char } else { '.' })
                    .collect();
                println!("{:>8}  {:<48}  {}", at + i * 16, hex.join(" "), asc);
            }
        }
        "tape" => cmd_tape(rest),
        "map" => cmd_map(rest),
        "trim" => trim::cmd(rest),
        "declare" => declare::cmd(rest),
        "identity" => ident::cmd(rest),
        "header" => {
            let what = rest.first().map(|s| s.as_str()).unwrap_or("show");
            match what {
                "show" => hdr::cmd_show(rest.get(1).map(|s| s.as_str()).unwrap_or_else(|| die("ghost header show FILE"))),
                o => die(format!("unknown `ghost header` operation {:?}", o)),
            }
        }
        "census" => census::cmd(rest),
        "phase" => phase::cmd(rest),
        "record" => record::cmd(rest),
        "regen" => regen::cmd(rest),
        "regen-control" => regen::control(rest),
        "roundtrip" => roundtrip::cmd(rest),
        "verify" => verify::cmd(rest),
        "selftest" => selftest::cmd(rest),
        o => die(format!("unknown command {:?} (try `ghost --help`)", o)),
    }
}

// ---------------------------------------------------------------------------

fn cmd_chunks(a: &[String]) {
    let c = Container::load(&a[0]).unwrap_or_else(|e| die(e));
    println!("body {} B, {} skippable chunks", c.body().len(), c.chunks().len());
    for (cid, off, poff, sz) in c.chunks() {
        println!("  0x{:08X} at {:>9} payload {:>9} size {:>9}", cid, off, poff, sz);
    }
}

fn cmd_inspect(a: &[String]) {
    let path = a.first().unwrap_or_else(|| die("ghost inspect FILE"));
    let c = Container::load(path).unwrap_or_else(|e| die(e));
    println!("file          {} ({} B on disk)", path, std::fs::metadata(path).map(|m| m.len()).unwrap_or(0));
    println!("body          {} B, {} skippable chunks", c.body().len(), c.chunks().len());

    match c.embedded_map() {
        Some((o, n)) => {
            println!("EMBEDDED MAP  yes -- {} B at body offset {}", n, o);
            println!("              --map is DECORATION for this file: the server simulates the");
            println!("              copy inside it. `ghost map set` is the only way to change it.");
        }
        None => println!("EMBEDDED MAP  none -- --map is real for this file"),
    }
    let uids = c.uids();
    if !uids.is_empty() {
        let mut d: Vec<String> = uids.iter().map(|(_, s)| s.clone()).collect();
        d.sort();
        d.dedup();
        println!("map uids      {:?} ({} literal copies)", d, uids.len());
    }

    let dt = c.declared_times();
    if dt.is_empty() {
        println!("declared      none");
    } else {
        let mut vals: Vec<u32> = dt.iter().map(|(_, v)| *v).collect();
        vals.sort();
        vals.dedup();
        let shown: Vec<String> = vals.iter().map(|v| secs(*v as i64)).collect();
        println!(
            "declared      {} in {} copies{}",
            shown.join(" / "),
            dt.len(),
            if vals.len() > 1 { "   <-- DISAGREE, this file declares two different times" } else { "" }
        );
    }
    // The DECODED checkpoint list, not the chunk's raw words. Printing the raw
    // array through the seconds formatter is what made this line read
    // `splits 0.001 19.538 0.000 0.000 0.003 0.004 7.617 ...` -- a version
    // number as 0.001 and a per-entry tag as 0.002.
    if let Some(r) = c.result() {
        let s: Vec<String> = r.checkpoints().iter().map(|v| secs(*v as i64)).collect();
        println!(
            "splits        {}   ({} checkpoints, the last is the finish)",
            s.join(" "),
            r.entries.len()
        );
        println!(
            "              the result chunk declares race {} and {} respawns",
            secs(r.race_ms as i64),
            r.nb_respawns
        );
        if let Some((_, d)) = c.declared_times().first() {
            if *d as i32 != r.race_ms {
                println!(
                    "              <-- DISAGREE: the header declares {} and the result chunk {}",
                    secs(*d as i64),
                    secs(r.race_ms as i64)
                );
            }
        }
    }

    match Tape::from_file(path) {
        Err(e) => println!("input tape    NONE ({})", e),
        Ok(t) => {
            let a0 = &t.archives[0];
            let n = a0.packets.len();
            let same = a0.packets.iter().filter(|p| p.vsame).count();
            let resp = a0.packets.iter().filter(|p| p.respawn()).count();
            let mut modes: Vec<u32> = a0.packets.iter().map(|p| p.mode).collect();
            modes.sort();
            modes.dedup();
            println!(
                "input tape    {} archives, archive 0: {} ticks, start_offset {} ms, format v{}",
                t.archives.len(),
                n,
                a0.start_offset_ms,
                a0.format_version
            );
            println!(
                "              modes {:?}, {} same-as-previous packets, {} respawn ticks",
                modes, same, resp
            );
            match t.verbatim_is_identity() {
                Ok(()) => println!("              codec identity: OK (verbatim re-encode reproduces the file)"),
                Err(e) => println!("              codec identity: FAILED -- {}", e),
            }
        }
    }

    match gbx::record::decode_ghost(path) {
        Err(e) => println!("telemetry     NONE ({})", e),
        Ok(d) => {
            println!(
                "telemetry     {} samples, {} entities, record span {} .. {}",
                d.samples.len(),
                d.ents.len(),
                secs(d.start_ms as i64),
                secs(d.end_ms as i64)
            );
            if let (Some(f), Some(l)) = (d.samples.first(), d.samples.last()) {
                println!(
                    "              first {} at ({:.2}, {:.2}, {:.2})   last {} at ({:.2}, {:.2}, {:.2})",
                    secs(f.time_ms as i64),
                    f.x,
                    f.y,
                    f.z,
                    secs(l.time_ms as i64),
                    l.x,
                    l.y,
                    l.z
                );
            }
        }
    }
    ident::print(&c);
    if has(a, "--ticks") {
        if let Ok(t) = Tape::from_file(path) {
            print!("{}", t.to_text(path));
        }
    }
}

fn cmd_tape(a: &[String]) {
    let what = a.first().map(|s| s.as_str()).unwrap_or_else(|| die("ghost tape <extract|inject|script|expand|graft|set|diff|stats|bits>"));
    let rest = &a[1..];
    match what {
        "script" => {
            let base = rest.first().unwrap_or_else(|| die("ghost tape script BASE.gtape --events F --out T.gtape"));
            let ev = need(rest, "--events");
            let out = need(rest, "--out");
            let btxt = std::fs::read_to_string(base).unwrap_or_else(|e| die(format!("{}: {}", base, e)));
            let etxt = std::fs::read_to_string(ev).unwrap_or_else(|e| die(format!("{}: {}", ev, e)));
            let events = ghost::script::parse_events(&etxt).unwrap_or_else(|e| die(e));
            let keep = has(rest, "--keep-before");
            let txt = ghost::script::apply_from(&btxt, &events, keep).unwrap_or_else(|e| die(e));
            let txt = match rest.iter().position(|x| x == "--signature-at") {
                Some(i) => {
                    let ms: i64 = rest[i + 1].parse().unwrap_or_else(|_| die("--signature-at wants a race time in ms"));
                    ghost::script::signature(&txt, ms).unwrap_or_else(|e| die(e))
                }
                None => txt,
            };
            // control: the text we are about to write must parse as a tape,
            // and re-emit as the same text. A script that produced something
            // the codec cannot express is a failure here, not at inject time.
            let t = Tape::from_text(&txt).unwrap_or_else(|e| die(format!("the scripted tape does not parse: {}", e)));
            std::fs::write(&out, &txt).unwrap_or_else(|e| die(format!("{}: {}", out, e)));
            println!("wrote {} ({} events, {} ticks)", out, events.len(), t.n());
        }
        "graft" => {
            // HEAD's first `--at` ticks, then TAIL's ticks from `--from`, as one
            // tape. This project grafts tapes constantly -- a route from one run,
            // a technique from another -- and until now every arm did it by hand
            // in the text format, renumbering `t=` with an editor. A renumbering
            // that slips by one is invisible and produces a run that simply goes
            // somewhere else.
            //
            // The output keeps HEAD's archive framing (its format, its
            // `start_offset_ms` and its tail bytes), because the container it
            // will be injected into is HEAD's. TAIL contributes vehicle inputs
            // only.
            let head = need(rest, "--head");
            let tail = need(rest, "--tail");
            let out = need(rest, "--out");
            let at: usize = num(rest, "--at").unwrap_or_else(|| die("--at TICK: where HEAD stops")) as usize;
            let from: usize = num(rest, "--from").unwrap_or(0) as usize;
            // HEAD and TAIL may each be a `.Ghost.Gbx` or an already-extracted
            // `.gtape`. Guessing by extension is how a tool ends up asserting
            // "not a GBX file" at the user; the first byte says which it is.
            let load = |p: &str| -> Tape {
                let raw = std::fs::read(p).unwrap_or_else(|e| die(format!("{}: {}", p, e)));
                if raw.first() == Some(&b'#') {
                    Tape::from_text(&String::from_utf8_lossy(&raw)).unwrap_or_else(|e| die(e))
                } else {
                    Tape::from_file(p).unwrap_or_else(|e| die(e))
                }
            };
            let h = load(head);
            let t = load(tail);
            let ht = h.to_text(head);
            let tt = t.to_text(tail);
            let mut lines: Vec<String> = Vec::new();
            let mut n_head = 0usize;
            for l in ht.lines() {
                if let Some(rest2) = l.strip_prefix("t=") {
                    let idx: usize = rest2
                        .split_whitespace()
                        .next()
                        .and_then(|v| v.parse().ok())
                        .unwrap_or_else(|| die(format!("cannot read a tick index from {:?}", l)));
                    if idx >= at {
                        break;
                    }
                    lines.push(l.to_string());
                    n_head += 1;
                } else {
                    lines.push(l.to_string());
                }
            }
            if n_head != at {
                die(format!(
                    "--at {} but HEAD only has {} ticks before it",
                    at, n_head
                ));
            }
            let mut n_tail = 0usize;
            for l in tt.lines() {
                let Some(rest2) = l.strip_prefix("t=") else { continue };
                let mut it = rest2.splitn(2, ' ');
                let idx: usize = it.next().and_then(|v| v.parse().ok()).unwrap_or(usize::MAX);
                let body = it.next().unwrap_or("");
                if idx < from {
                    continue;
                }
                lines.push(format!("t={} {}", at + n_tail, body));
                n_tail += 1;
            }
            if n_tail == 0 {
                die(format!("--from {} leaves TAIL with no ticks", from));
            }
            // The `@archive` line's own packet count must match, or the reader
            // trusts a number the body contradicts.
            let total = at + n_tail;
            for l in lines.iter_mut() {
                if l.starts_with("@archive ") {
                    *l = l
                        .split_whitespace()
                        .map(|f| {
                            if let Some(v) = f.strip_prefix("packets=") {
                                let _ = v;
                                format!("packets={}", total)
                            } else {
                                f.to_string()
                            }
                        })
                        .collect::<Vec<_>>()
                        .join(" ");
                }
            }
            let text = lines.join("\n") + "\n";
            // read it back rather than trusting the string we just built
            let re = Tape::from_text(&text).unwrap_or_else(|e| die(format!("the graft does not parse: {}", e)));
            if re.n() != total {
                die(format!("read-back control FAILED: {} ticks written, {} read", total, re.n()));
            }
            std::fs::write(out, &text).unwrap_or_else(|e| die(format!("{}: {}", out, e)));
            let off: i64 = h.archives.first().map(|a| a.start_offset_ms as i64).unwrap_or(0);
            println!(
                "wrote {}  {} ticks = {} from {} (race {} .. {}) + {} from {} (its tick {} on)",
                out,
                total,
                at,
                head,
                secs(off),
                secs(at as i64 * 10 + off),
                n_tail,
                tail,
                from
            );
            println!("  read-back control OK: the graft parses to {} ticks", re.n());
            println!(
                "  the handover is at tick {} = race {} of {}",
                at,
                secs(at as i64 * 10 + off),
                head
            );
        }
        "set" => {
            // Overwrite the vehicle inputs over a tick RANGE. The sweep
            // primitive: "hold this steer for these 200 ticks and see where
            // the car ends up" is the shape of almost every experiment on this
            // project, and doing it by editing `.gtape` text is how a window
            // ends up one tick out.
            //
            // Ranges are TICKS, not race ms, because a tape's own indexing is
            // ticks and `start_offset_ms` is a per-file trap. The command
            // prints the race window it corresponds to so the two are never
            // confused.
            let src = rest.first().unwrap_or_else(|| die("ghost tape set IN --out T.gtape --from A --to B [--steer S] [--accel 0|1] [--brake 0|1]"));
            let out = need(rest, "--out");
            let from: usize = num(rest, "--from").unwrap_or(0) as usize;
            let to: usize = num(rest, "--to").unwrap_or_else(|| die("--to TICK (exclusive)")) as usize;
            let steer = num(rest, "--steer");
            let accel = num(rest, "--accel");
            let brake = num(rest, "--brake");
            if steer.is_none() && accel.is_none() && brake.is_none() {
                die("nothing to set: give at least one of --steer / --accel / --brake");
            }
            if let Some(s) = steer {
                if !(-127..=127).contains(&s) {
                    die(format!("--steer is an i8 over 127; {} is out of range", s));
                }
            }
            let raw = std::fs::read(src).unwrap_or_else(|e| die(format!("{}: {}", src, e)));
            let t = if raw.first() == Some(&b'#') {
                Tape::from_text(&String::from_utf8_lossy(&raw)).unwrap_or_else(|e| die(e))
            } else {
                Tape::from_file(src).unwrap_or_else(|e| die(e))
            };
            if to > t.n() || from >= to {
                die(format!("--from {} --to {} does not fit a {}-tick tape", from, to, t.n()));
            }
            let mut n = 0usize;
            let text: String = t
                .to_text(src)
                .lines()
                .map(|l| {
                    let Some(rest2) = l.strip_prefix("t=") else { return l.to_string() };
                    let idx: usize =
                        rest2.split_whitespace().next().and_then(|v| v.parse().ok()).unwrap_or(usize::MAX);
                    if idx < from || idx >= to {
                        return l.to_string();
                    }
                    n += 1;
                    // Rewrite the named fields in place; `vsame=1` is forced to
                    // 0 because a changed value cannot be coded as "same as the
                    // previous tick", and the writer expands it anyway.
                    l.split_whitespace()
                        .map(|f| {
                            if f.starts_with("steer=") {
                                steer.map_or(f.to_string(), |s| format!("steer={}", s))
                            } else if f.starts_with("accel=") {
                                accel.map_or(f.to_string(), |s| format!("accel={}", s))
                            } else if f.starts_with("brake=") {
                                brake.map_or(f.to_string(), |s| format!("brake={}", s))
                            } else if f.starts_with("vsame=") {
                                "vsame=0".to_string()
                            } else {
                                f.to_string()
                            }
                        })
                        .collect::<Vec<_>>()
                        .join(" ")
                })
                .collect::<Vec<_>>()
                .join("\n")
                + "\n";
            if n != to - from {
                die(format!("expected to rewrite {} ticks, rewrote {}", to - from, n));
            }
            let re = Tape::from_text(&text).unwrap_or_else(|e| die(format!("the edit does not parse: {}", e)));
            if re.n() != t.n() {
                die(format!("read-back control FAILED: {} ticks in, {} out", t.n(), re.n()));
            }
            if let Some(s) = steer {
                let got = re.steer_i8s();
                if (from..to).any(|i| got[i] as i64 != s) {
                    die("read-back control FAILED: a tick in the window does not carry the steer asked for");
                }
            }
            std::fs::write(out, &text).unwrap_or_else(|e| die(format!("{}: {}", out, e)));
            let off: i64 = t.archives.first().map(|a| a.start_offset_ms as i64).unwrap_or(0);
            println!(
                "wrote {}  {} ticks rewritten, t={}..{} = race {} .. {}  (read-back control OK)",
                out,
                n,
                from,
                to,
                secs(from as i64 * 10 + off),
                secs(to as i64 * 10 + off)
            );
        }
        "extract" => {
            let src = rest.first().unwrap_or_else(|| die("ghost tape extract FILE --out T"));
            let out = need(rest, "--out");
            let t = Tape::from_file(src).unwrap_or_else(|e| die(e));
            t.verbatim_is_identity().unwrap_or_else(|e| {
                die(format!(
                    "REFUSING to extract: this file's input chunk does not survive a verbatim \
                     re-encode ({}). Extracting it would produce a tape that cannot be injected \
                     back losslessly.",
                    e
                ))
            });
            std::fs::write(out, t.to_text(src)).unwrap_or_else(|e| die(format!("{}: {}", out, e)));
            println!(
                "wrote {}  ({} archives, {} ticks, codec identity OK)",
                out,
                t.archives.len(),
                t.n()
            );
        }
        "inject" => {
            let inp = rest.first().unwrap_or_else(|| die("ghost tape inject IN OUT --tape T"));
            let out = rest.get(1).unwrap_or_else(|| die("ghost tape inject IN OUT --tape T"));
            let tp = need(rest, "--tape");
            let txt = std::fs::read_to_string(tp).unwrap_or_else(|e| die(format!("{}: {}", tp, e)));
            let newt = Tape::from_text(&txt).unwrap_or_else(|e| die(e));
            let c = Container::load(inp).unwrap_or_else(|e| die(e));
            let oldt = Tape::from_file(inp).unwrap_or_else(|e| die(e));
            if newt.archives.len() != oldt.archives.len() {
                die(format!(
                    "tape has {} archives, {} has {}",
                    newt.archives.len(),
                    inp,
                    oldt.archives.len()
                ));
            }
            for (i, (na, oa)) in newt.archives.iter().zip(oldt.archives.iter()).enumerate() {
                if na.packets.len() != oa.packets.len() {
                    die(format!(
                        "archive {}: tape has {} ticks, {} has {}. A tape is injected into a \
                         container of the same length -- use `ghost trim` to change the length.",
                        i,
                        na.packets.len(),
                        inp,
                        oa.packets.len()
                    ));
                }
            }
            let enc = if has(rest, "--verbatim") { Encoding::Verbatim } else { Encoding::Explicit };
            let body = newt.splice_into(c.body(), enc).unwrap_or_else(|e| die(e));
            container::write_gbx(&c.gbx, body, out).unwrap_or_else(|e| die(e));
            // control: read it straight back and require the tape to match
            let back = Tape::from_file(out).unwrap_or_else(|e| die(e));
            let mut bad = 0usize;
            for (na, ba) in newt.archives.iter().zip(back.archives.iter()) {
                for (i, (p, q)) in na.packets.iter().zip(ba.packets.iter()).enumerate() {
                    if p.steer != q.steer
                        || p.accel != q.accel
                        || p.brake != q.brake
                        || p.respawn() != q.respawn()
                        || p.mode != q.mode
                        || p.mouse != q.mouse
                        || p.tri != q.tri
                    {
                        if bad < 5 {
                            eprintln!("  tick {}: written back differently", i);
                        }
                        bad += 1;
                    }
                }
            }
            if bad > 0 {
                die(format!("read-back control FAILED on {} ticks -- {} is not trustworthy", bad, out));
            }
            let n = newt.n();
            let writable = back.archives[0].packets.iter().filter(|p| !p.vsame).count();
            println!(
                "wrote {}  ({} ticks, {} explicitly coded, read-back control OK)",
                out, n, writable
            );
            let dec = gbx::record::decode_ghost(out).ok();
            if let Some(d) = dec {
                if !d.samples.is_empty() && !has(rest, "--allow-telemetry-mismatch") {
                    println!(
                        "NOTE: this file still carries {} telemetry samples from BEFORE the edit. \
                         They describe the old inputs. Run `ghost regen` to rebuild them, or the \
                         file will render the old run.",
                        d.samples.len()
                    );
                }
            }
        }
        "expand" => {
            let inp = rest.first().unwrap_or_else(|| die("ghost tape expand IN OUT"));
            let out = rest.get(1).unwrap_or_else(|| die("ghost tape expand IN OUT"));
            let c = Container::load(inp).unwrap_or_else(|e| die(e));
            let t = Tape::from_file(inp).unwrap_or_else(|e| die(e));
            let before = t.archives[0].packets.iter().filter(|p| p.vsame).count();
            let body = t.splice_into(c.body(), Encoding::Explicit).unwrap_or_else(|e| die(e));
            container::write_gbx(&c.gbx, body, out).unwrap_or_else(|e| die(e));
            let back = Tape::from_file(out).unwrap_or_else(|e| die(e));
            for (p, q) in t.archives[0].packets.iter().zip(back.archives[0].packets.iter()) {
                if p.steer != q.steer || p.accel != q.accel || p.brake != q.brake {
                    die("expansion changed an input value -- refusing");
                }
            }
            println!(
                "wrote {}  ({} same-as-previous packets expanded; every one of {} ticks is now writable)",
                out,
                before,
                t.n()
            );
        }
        "diff" => {
            let x = Tape::from_text(&std::fs::read_to_string(&rest[0]).unwrap()).unwrap_or_else(|e| die(e));
            let y = Tape::from_text(&std::fs::read_to_string(&rest[1]).unwrap()).unwrap_or_else(|e| die(e));
            let n = x.n().min(y.n());
            let mut d = 0;
            for i in 0..n {
                let (p, q) = (&x.archives[0].packets[i], &y.archives[0].packets[i]);
                if p.steer != q.steer || p.accel != q.accel || p.brake != q.brake || p.respawn() != q.respawn() {
                    if d < 80 {
                        println!(
                            "t={:<6} steer {:>4} -> {:<4} accel {} -> {}  brake {} -> {}  respawn {} -> {}",
                            i,
                            p.steer_i8(),
                            q.steer_i8(),
                            p.accel,
                            q.accel,
                            p.brake,
                            q.brake,
                            p.respawn() as u8,
                            q.respawn() as u8
                        );
                    }
                    d += 1;
                }
            }
            println!("{} ticks differ (of {} / {})", d, x.n(), y.n());
            if x.n() != y.n() || d > 0 {
                std::process::exit(1);
            }
        }
        "stats" => {
            let t = if rest[0].ends_with(".gtape") {
                Tape::from_text(&std::fs::read_to_string(&rest[0]).unwrap()).unwrap_or_else(|e| die(e))
            } else {
                Tape::from_file(&rest[0]).unwrap_or_else(|e| die(e))
            };
            let a0 = &t.archives[0];
            let n = a0.packets.len();
            let mut ev = 0;
            for i in 1..n {
                let (p, q) = (&a0.packets[i - 1], &a0.packets[i]);
                if p.steer != q.steer || p.accel != q.accel || p.brake != q.brake {
                    ev += 1;
                }
            }
            println!("ticks          {}", n);
            println!("tape span      {} .. {}", secs(a0.start_offset_ms as i64), secs(a0.start_offset_ms as i64 + 10 * n as i64));
            println!("input events   {}", ev);
            println!("respawn ticks  {}", a0.packets.iter().filter(|p| p.respawn()).count());
            println!("same-as-prev   {}", a0.packets.iter().filter(|p| p.vsame).count());
            println!("accel on       {}", a0.packets.iter().filter(|p| p.accel != 0).count());
            println!("brake on       {}", a0.packets.iter().filter(|p| p.brake != 0).count());
            println!("mouse packets  {}", a0.packets.iter().filter(|p| p.mouse.is_some()).count());
        }
        "recinputs" => {
            let f = &rest[0];
            let t = Tape::from_file(f).unwrap_or_else(|e| die(e));
            let d = gbx::record::decode_ghost(f).unwrap_or_else(|e| die(e));
            let raw = raw_vehicle_samples(f).unwrap_or_else(|e| die(e));
            let a0 = &t.archives[0];
            let so = a0.start_offset_ms as i64;
            let (ss, r) = &raw;
            if has(rest, "--check") {
                // The FIT CONTROL for the recorded input channels: predict the
                // three telemetry bytes from the tape and count exact hits.
                let (mut n, mut h14, mut h15, mut h18) = (0usize, 0usize, 0usize, 0usize);
                let mut seen_lift = 0usize;
                let mut seen_brake = 0usize;
                for (i, s) in d.samples.iter().enumerate() {
                    let idx = (s.time_ms as i64 - so) / 10;
                    if idx < 0 || idx >= a0.packets.len() as i64 {
                        continue;
                    }
                    let p = &a0.packets[idx as usize];
                    let d0 = &r[i * ss..(i + 1) * ss];
                    n += 1;
                    if d0[14] == regen::steer_byte(p.steer_i8()) {
                        h14 += 1;
                    }
                    if d0[15] == regen::pedal_byte(p.accel) {
                        h15 += 1;
                    }
                    if d0[18] == regen::pedal_byte(p.brake) {
                        h18 += 1;
                    }
                    if p.accel == 0 {
                        seen_lift += 1;
                    }
                    if p.brake != 0 {
                        seen_brake += 1;
                    }
                }
                println!(
                    "{} samples  steer {:.2}%  gas {:.2}%  brake {:.2}%   (lift ticks {}, brake ticks {})",
                    n,
                    100.0 * h14 as f64 / n.max(1) as f64,
                    100.0 * h15 as f64 / n.max(1) as f64,
                    100.0 * h18 as f64 / n.max(1) as f64,
                    seen_lift,
                    seen_brake
                );
                // Where do the misses come from -- the byte encoding or the
                // tick the sample is paired with? Score every phase from -2 to
                // +2 ticks; if a neighbouring phase is perfect, the encoding is
                // right and the pairing was wrong.
                for ph in -2i64..=2 {
                    let (mut m, mut hit) = (0usize, 0usize);
                    for (i, s) in d.samples.iter().enumerate() {
                        let idx = (s.time_ms as i64 - so) / 10 + ph;
                        if idx < 0 || idx >= a0.packets.len() as i64 {
                            continue;
                        }
                        let p = &a0.packets[idx as usize];
                        let d0 = &r[i * ss..(i + 1) * ss];
                        m += 1;
                        if d0[14] == regen::steer_byte(p.steer_i8()) {
                            hit += 1;
                        }
                    }
                    println!("   phase {:+}: steer {:.2}%", ph, 100.0 * hit as f64 / m.max(1) as f64);
                }
                if h14 < n || h15 < n || h18 < n {
                    std::process::exit(1);
                }
                return;
            }
            println!("{:>8} {:>6} {:>5} {:>5}   {:>4} {:>4} {:>4}", "t_ms", "steer", "gas", "brk", "b14", "b15", "b18");
            let step = (d.samples.len() / 40).max(1);
            for (i, s) in d.samples.iter().enumerate() {
                if i % step != 0 {
                    continue;
                }
                let idx = (s.time_ms as i64 - so) / 10;
                if idx < 0 || idx >= a0.packets.len() as i64 {
                    continue;
                }
                let p = &a0.packets[idx as usize];
                let d0 = &r[i * ss..(i + 1) * ss];
                println!(
                    "{:>8} {:>6} {:>5} {:>5}   {:>4} {:>4} {:>4}",
                    s.time_ms, p.steer_i8(), p.accel, p.brake, d0[14], d0[15], d0[18]
                );
            }
        }
        "sync-record" => {
            // Write the RECORDED input channels from the tape. Useful on its
            // own: after `ghost tape inject`, the telemetry's steer / gas /
            // brake bytes are still the old run's even though they are fully
            // determined by the tape.
            let inp = rest.first().unwrap_or_else(|| die("ghost tape sync-record IN OUT"));
            let out = rest.get(1).unwrap_or_else(|| die("ghost tape sync-record IN OUT"));
            match regen::write_input_channels(inp, out) {
                Err(e) => die(e),
                Ok((w, sk)) => {
                    println!("wrote {} ({} samples rewritten, {} outside the tape)", out, w, sk);
                    if let Some((k, _, lag, n)) = verify::tape_record_agreement(out) {
                        println!(
                            "  tape/record agreement is now kappa {:.3} over {} samples (lag {} ms)",
                            k, n, lag
                        );
                    }
                }
            }
        }
        "bits" => cmd_bits(rest),
        o => die(format!("unknown `ghost tape` operation {:?}", o)),
    }
}

/// The state literal is 33 or 34 bits and only a few of them have names. This
/// counts, over a whole corpus, which bits ever vary -- so "unnamed" is an
/// enumerated set rather than a shrug.
fn cmd_bits(a: &[String]) {
    let files: Vec<&String> = a.iter().filter(|s| !s.starts_with("--")).collect();
    let mut ones = [0u64; 34];
    let mut zeros = [0u64; 34];
    let mut lits = 0u64;
    for f in &files {
        let t = match Tape::from_file(f) {
            Ok(t) => t,
            Err(e) => {
                eprintln!("  skip {}: {}", f, e);
                continue;
            }
        };
        for ar in &t.archives {
            for p in &ar.packets {
                if let tape::StateEnc::Lit(l) = p.state {
                    lits += 1;
                    for b in 0..34 {
                        if l >> b & 1 == 1 {
                            ones[b] += 1
                        } else {
                            zeros[b] += 1
                        }
                    }
                }
            }
        }
    }
    println!("{} explicit state literals over {} files", lits, files.len());
    println!("bit  ones      zeros     what it is");
    for b in 0..34 {
        if ones[b] == 0 && zeros[b] == 0 {
            continue;
        }
        let name = match b {
            0..=3 => "mode (word0 & 0xF)",
            31 => "RESPAWN",
            5..=26 => "flags",
            _ => "",
        };
        let varies = ones[b] > 0 && zeros[b] > 0;
        println!(
            "{:>3}  {:>9} {:>9}  {}{}",
            b,
            ones[b],
            zeros[b],
            name,
            if varies { "   <-- varies" } else { "" }
        );
    }
}

fn cmd_map(a: &[String]) {
    let what = a.first().map(|s| s.as_str()).unwrap_or_else(|| die("ghost map <show|extract|set>"));
    let rest = &a[1..];
    match what {
        "show" => {
            let c = Container::load(&rest[0]).unwrap_or_else(|e| die(e));
            match c.embedded_map() {
                None => {
                    println!("NO embedded map chunk 0x03093002.");
                    println!("--map is REAL for this file: the server reads UserData/Maps.");
                    let mut u: Vec<String> = c.uids().into_iter().map(|(_, s)| s).collect();
                    u.sort();
                    u.dedup();
                    println!("declared map uid(s): {:?}", u);
                }
                Some((o, n)) => {
                    println!("EMBEDDED MAP: {} B at body offset {}", n, o);
                    println!("The dedicated server simulates THIS copy. --map, UserData/Maps and");
                    println!("the uid in the header are decoration for this file.");
                    let bytes = c.embedded_map_bytes().unwrap();
                    println!("carried map uid: {:?}", map_uid_of(&bytes));
                    let mut u: Vec<String> = c.uids().into_iter().map(|(_, s)| s).collect();
                    u.sort();
                    u.dedup();
                    println!("uid literals in the file: {:?}", u);
                }
            }
        }
        "extract" => {
            let c = Container::load(&rest[0]).unwrap_or_else(|e| die(e));
            let out = need(rest, "--out");
            match c.embedded_map_bytes() {
                None => die("this file carries no embedded map (nothing to extract)"),
                Some(b) => {
                    std::fs::write(out, &b).unwrap_or_else(|e| die(format!("{}: {}", out, e)));
                    println!("wrote {} ({} B), uid {:?}", out, b.len(), map_uid_of(&b));
                }
            }
        }
        "rebind" => {
            // Change which map a PURE GHOST is bound to, by rewriting the uid
            // it declares (chunk 0x03092010 and its copies).
            //
            // This is the right operation for a file with no embedded map, and
            // the WRONG one for a file that carries one: there, rewriting the
            // uid makes the file CLAIM the other map while the server goes on
            // simulating the copy inside it. So it refuses.
            let inp = &rest[0];
            let out = &rest[1];
            let mapf = need(rest, "--map");
            let c = Container::load(inp).unwrap_or_else(|e| die(e));
            if c.embedded_map().is_some() && !has(rest, "--force") {
                die(
                    "REFUSED: this file carries an embedded map, so the server will simulate THAT \
                     copy whatever uid the file declares. Rewriting the uid here would produce a \
                     file that claims one map and runs another -- the exact failure this API \
                     exists to prevent. Use `ghost map set` to replace the carried map.",
                );
            }
            let mapdata = std::fs::read(mapf).unwrap_or_else(|e| die(format!("{}: {}", mapf, e)));
            let newuid = map_uid_of(&mapdata).unwrap_or_else(|| die("no uid in that map"));
            let olds: Vec<String> = {
                let mut v: Vec<String> = c.uids().into_iter().map(|(_, s)| s).collect();
                v.sort();
                v.dedup();
                v
            };
            if olds.is_empty() {
                die("this file declares no map uid at all");
            }
            if olds.len() > 1 {
                die(format!("this file declares more than one uid ({:?}); refusing to guess", olds));
            }
            let old = &olds[0];
            if old.len() != newuid.len() {
                die(format!(
                    "uid length differs ({} vs {}): rewriting would change the chunk size",
                    old.len(),
                    newuid.len()
                ));
            }
            let mut body = c.body().to_vec();
            let ob = old.as_bytes();
            let nb = newuid.as_bytes();
            let mut n = 0;
            let mut i = 0usize;
            while i + 4 + ob.len() <= body.len() {
                if u32::from_le_bytes(body[i..i + 4].try_into().unwrap()) as usize == ob.len()
                    && &body[i + 4..i + 4 + ob.len()] == ob
                {
                    body[i + 4..i + 4 + ob.len()].copy_from_slice(nb);
                    n += 1;
                    i += 4 + ob.len();
                    continue;
                }
                i += 1;
            }
            container::write_gbx(&c.gbx, body, out).unwrap_or_else(|e| die(e));
            let c2 = Container::load(out).unwrap_or_else(|e| die(e));
            let after: Vec<String> = {
                let mut v: Vec<String> = c2.uids().into_iter().map(|(_, s)| s).collect();
                v.sort();
                v.dedup();
                v
            };
            if after != vec![newuid.clone()] {
                die(format!("read-back control FAILED: uids are now {:?}", after));
            }
            println!("wrote {}", out);
            println!("  uid {} -> {}  ({} literal copies rewritten, read-back OK)", old, newuid, n);
            println!(
                "  PROVE IT: validate with ONLY {} in UserData/Maps. The control that makes the \n\
                 \x20         answer mean something is the same tape UNREBOUND against that map -- \n\
                 \x20         it must return nothing at all."
                , mapf
            );
        }
        "set" => {
            let inp = &rest[0];
            let out = &rest[1];
            let mapf = need(rest, "--map");
            let newmap = std::fs::read(mapf).unwrap_or_else(|e| die(format!("{}: {}", mapf, e)));
            if newmap.len() < 16 || &newmap[0..3] != b"GBX" {
                die(format!("{} is not a GBX map file", mapf));
            }
            let c = Container::load(inp).unwrap_or_else(|e| die(e));
            let newuid = map_uid_of(&newmap).unwrap_or_else(|| die("no uid in the replacement map"));
            let body = set_embedded_map(&c, &newmap, &newuid).unwrap_or_else(|e| die(e));
            container::write_gbx(&c.gbx, body, out).unwrap_or_else(|e| die(e));
            // control: read it back
            let c2 = Container::load(out).unwrap_or_else(|e| die(e));
            match c2.embedded_map_bytes() {
                None => die("wrote a file with no embedded map -- refusing to claim success"),
                Some(b) => {
                    if b != newmap {
                        die("the map read back is not the map written -- refusing");
                    }
                }
            }
            let u: Vec<String> = {
                let mut v: Vec<String> = c2.uids().into_iter().map(|(_, s)| s).collect();
                v.sort();
                v.dedup();
                v
            };
            println!("wrote {}", out);
            println!("  embedded map replaced: {} B -> {} B, uid {}", c.embedded_map().map(|x| x.1).unwrap_or(0), newmap.len(), newuid);
            println!("  uid literals now: {:?}", u);
            println!("  PROVE IT with an EMPTY UserData/Maps: `ghost verify {} --empty-maps`", out);
        }
        o => die(format!("unknown `ghost map` operation {:?}", o)),
    }
}

