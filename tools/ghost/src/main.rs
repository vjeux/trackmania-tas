//! `ghost` -- the TM2020 ghost/replay API.
//!
//! One binary, one job: every operation this project performs on a `.Ghost.Gbx`
//! or `.Replay.Gbx`, each with a control that proves it did what it says.
//!
//! Rust only. There is no interpreter anywhere in this pipeline and no shell
//! script carries any logic.

use ghost::cli::{die, flag, has, need, num};
use ghost::container::{secs, set_embedded_map, Container};
use ghost::regen::raw_vehicle_samples;
use ghost::tape::{Encoding, Tape};
use ghost::{container, engine, ident, map_uid_of, oracle, regen, selftest, tape, trim, verify};

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
  ghost regen IN OUT --map MAP [--fieldmap F] [--anchorticks a,b,c]
        Run the real engine on this file's own inputs, capture per-sample car
        state and write it into the ghost, so the recorded trajectory MATCHES
        the tape. Refuses unless the acceptance gate passes.
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
        Cut the head and/or tail of a run, keeping the file coherent: inputs,
        telemetry samples, the record span, the splits and every copy of the
        declared time.

IDENTITY  (operation 6)
  ghost identity show FILE
  ghost identity set IN OUT [--name N] [--trigram XXX] [--skin PATH|default]
                            [--login L] [--zone Z] [--clubtag T] [--anonymise]
        Car skin, display name and 3-letter trigram. --anonymise also drops the
        account id and the storage locator URL, which are the two foreign
        identifiers a strip-list usually misses.

VERIFY
  ghost verify FILE [--map MAP] [--expect-ms MS] [--server DIR]
        The acceptance gate: codec identity, tape/telemetry agreement, declared
        time census, container identity, and the plain oracle re-simulating THE
        WRITTEN FILE.
  ghost selftest [--server DIR] [--data DIR]
        The whole test suite, from one command.
"#;

fn main() {
    { tmtraj::gbx::lzo_init(); };
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
            let a0 = tmtraj::entrec::decode_ghost(&rest[0]).unwrap_or_else(|e| die(e));
            let b0 = tmtraj::entrec::decode_ghost(&rest[1]).unwrap_or_else(|e| die(e));
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
        "declare" => cmd_declare(rest),
        "identity" => ident::cmd(rest),
        "regen" => regen::cmd(rest),
        "regen-control" => regen::control(rest),
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
    let sp = c.splits();
    if !sp.is_empty() {
        let s: Vec<String> = sp.iter().map(|v| secs(*v as i64)).collect();
        println!("splits        {}", s.join(" "));
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

    match tmtraj::entrec::decode_ghost(path) {
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
    let what = a.first().map(|s| s.as_str()).unwrap_or_else(|| die("ghost tape <extract|inject|expand|diff|stats|bits>"));
    let rest = &a[1..];
    match what {
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
            let dec = tmtraj::entrec::decode_ghost(out).ok();
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
            let d = tmtraj::entrec::decode_ghost(f).unwrap_or_else(|e| die(e));
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

/// a human and never copied from a search log.
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
fn cmd_declare(a: &[String]) {
    let inp = a.first().unwrap_or_else(|| die("ghost declare IN OUT (--time MS | --from-oracle --map M)"));
    let out = a.get(1).unwrap_or_else(|| die("ghost declare IN OUT (--time MS | --from-oracle --map M)"));
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
    trim::set_result_race_time(&mut body, ms as u32);
    let stage = format!("{}.declare-stage", out);
    container::write_gbx(&c.gbx, body, &stage).unwrap_or_else(|e| die(e));
    // The telemetry record declares its own span, separately from the samples.
    // Leaving it at the old run's is the same defect one level down, and
    // `ghost verify` reports it, so fix it here rather than print it later.
    let mut span_note = String::new();
    if tmtraj::recwrite::find_rec_site(&Container::load(&stage).unwrap().gbx.body).is_ok() {
        let r = tmtraj::recwrite::rewrite_ghost(&stage, out, |rd| {
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
    println!("wrote {}", out);
    println!("  declared {} in {} copies, all equal (read-back control OK)", secs(ms), dt.len());
    println!("  the ghost-result chunk's race time was set to the same value");
    if !span_note.is_empty() {
        println!("{}", span_note);
    }
}
