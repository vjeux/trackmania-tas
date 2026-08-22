//! `ghost selftest` -- the whole suite, from one command.
//!
//! Three tiers, each printed with its verdict:
//!
//!   PURE    format and codec checks against the checked-in fixtures. No
//!           server, no engine, milliseconds.
//!   ORACLE  the dedicated server, validating files this run just wrote.
//!   ENGINE  the real physics engine, regenerating a trajectory and being
//!           checked against a file that already knew the answer. Minutes.
//!
//! Every check states what it proves. A check that cannot run says SKIP and
//! why, and `--strict` turns a SKIP into a failure so a green run in CI cannot
//! be a green run that tested nothing.

use crate::container::{secs, Container};
use crate::ident::{self, Role};
use crate::oracle::{self, MapsMode};
use crate::tape::{Encoding, StateEnc, Tape};
use crate::cli::{flag, has};
use std::path::{Path, PathBuf};

pub struct Suite {
    pub data: PathBuf,
    pub work: PathBuf,
    pub server: Option<PathBuf>,
    pub results: Vec<(String, &'static str, String)>,
    pub strict: bool,
}

impl Suite {
    fn pass(&mut self, name: &str, detail: impl Into<String>) {
        println!("  PASS {:<26} {}", name, detail.into());
        self.results.push((name.into(), "PASS", String::new()));
    }
    fn fail(&mut self, name: &str, detail: impl Into<String>) {
        let d = detail.into();
        println!("  FAIL {:<26} {}", name, d);
        self.results.push((name.into(), "FAIL", d));
    }
    fn skip(&mut self, name: &str, detail: impl Into<String>) {
        let d = detail.into();
        println!("  SKIP {:<26} {}", name, d);
        self.results.push((name.into(), "SKIP", d));
    }
    fn check(&mut self, name: &str, ok: bool, detail: impl Into<String>) {
        if ok {
            self.pass(name, detail)
        } else {
            self.fail(name, detail)
        }
    }
    fn f(&self, n: &str) -> String {
        self.data.join(n).to_string_lossy().to_string()
    }
    fn w(&self, n: &str) -> String {
        self.work.join(n).to_string_lossy().to_string()
    }
}

const GHOSTS: [&str; 2] = ["human_22730.Ghost.Gbx", "human_23013.Ghost.Gbx"];
const MAP: &str = "map2.Map.Gbx";
const REPLAY: &str = "replay_kacky_7241.Replay.Gbx";
const POISONED: &str = "poisoned_searchtape.Ghost.Gbx";
/// The declared times the fixtures carry, and what the plain oracle returns.
const DONOR_MS: i64 = 22730;
const REPLAY_MS: i64 = 7241;

pub fn cmd(a: &[String]) {
    let data = PathBuf::from(flag(a, "--data").unwrap_or({
        // next to the binary, then the source tree, then the cwd
        if Path::new("ghostapi/testdata").is_dir() {
            "ghostapi/testdata"
        } else if Path::new("testdata").is_dir() {
            "testdata"
        } else {
            "rs/ghostapi/testdata"
        }
    }));
    let work = std::env::temp_dir().join(format!("ghost-selftest-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&work);
    std::fs::create_dir_all(&work).unwrap();
    let sd = oracle::server_dir(flag(a, "--server"));
    let server = if sd.join("TrackmaniaServer").exists() { Some(sd) } else { None };
    let mut s = Suite {
        data,
        work,
        server,
        results: Vec::new(),
        strict: has(a, "--strict"),
    };
    println!("ghost selftest");
    println!("  fixtures  {}", s.data.display());
    println!(
        "  server    {}",
        s.server.as_ref().map(|p| p.display().to_string()).unwrap_or("none".into())
    );
    println!("\nPURE -- format, codec and refusals");
    pure_tier(&mut s);
    println!("\nORACLE -- the dedicated server on files this run wrote");
    oracle_tier(&mut s);
    if has(a, "--engine") {
        println!("\nENGINE -- the real physics engine (minutes)");
        engine_tier(&mut s);
    } else {
        println!("\nENGINE -- skipped, pass --engine to run it (it takes several minutes)");
        s.skip("engine.tier", "not requested");
    }

    let n = s.results.len();
    let f = s.results.iter().filter(|r| r.1 == "FAIL").count();
    let k = s.results.iter().filter(|r| r.1 == "SKIP").count();
    println!("\n{} checks: {} passed, {} failed, {} skipped", n, n - f - k, f, k);
    if f > 0 || (s.strict && k > 0) {
        std::process::exit(1);
    }
}

// ---------------------------------------------------------------------------

fn pure_tier(s: &mut Suite) {
    // --- the codec is an identity -------------------------------------------
    for g in GHOSTS.iter().chain([REPLAY, POISONED].iter()) {
        let p = s.f(g);
        match Tape::from_file(&p) {
            Err(e) => s.fail(&format!("codec.identity[{}]", g), e),
            Ok(t) => match t.verbatim_is_identity() {
                Ok(()) => s.pass(
                    &format!("codec.identity[{}]", g),
                    format!(
                        "a verbatim re-encode of {} ticks reproduces the file's own bitstream, byte for byte",
                        t.n()
                    ),
                ),
                Err(e) => s.fail(&format!("codec.identity[{}]", g), e),
            },
        }
    }

    // --- extract -> inject -> extract ---------------------------------------
    for g in GHOSTS.iter() {
        let src = s.f(g);
        let t1 = s.w(&format!("{}.1.gtape", g));
        let out = s.w(&format!("{}.rt.Ghost.Gbx", g));
        let t2 = s.w(&format!("{}.2.gtape", g));
        let tape = Tape::from_file(&src).unwrap();
        std::fs::write(&t1, tape.to_text(&src)).unwrap();
        let re = Tape::from_text(&std::fs::read_to_string(&t1).unwrap());
        match re {
            Err(e) => s.fail(&format!("tape.roundtrip[{}]", g), e),
            Ok(re) => {
                let c = Container::load(&src).unwrap();
                let body = re.splice_into(c.body(), Encoding::Verbatim).unwrap();
                crate::container::write_gbx(&c.gbx, body, &out).unwrap();
                let back = Tape::from_file(&out).unwrap();
                std::fs::write(&t2, back.to_text(&src)).unwrap();
                let a = std::fs::read_to_string(&t1).unwrap();
                let b = std::fs::read_to_string(&t2).unwrap();
                // the #source line names the file, so compare from the body on
                let strip = |x: &str| -> String {
                    x.lines().filter(|l| !l.starts_with("#source")).collect::<Vec<_>>().join("\n")
                };
                s.check(
                    &format!("tape.roundtrip[{}]", g),
                    strip(&a) == strip(&b),
                    "extract -> inject -> extract is identical, every field of every tick",
                );
                // and the bitstream the injection wrote is the file's own
                let src_bits = Tape::from_file(&src).unwrap().archives[0].orig_bitstream.clone();
                let out_bits = back.archives[0].orig_bitstream.clone();
                s.check(
                    &format!("tape.bitidentity[{}]", g),
                    src_bits == out_bits,
                    format!(
                        "the injected file's input bitstream is byte-identical to the original's ({} B)",
                        src_bits.len()
                    ),
                );
            }
        }
    }

    // --- expansion of "same as the previous tick" packets --------------------
    {
        let src = s.f(GHOSTS[0]);
        let t = Tape::from_file(&src).unwrap();
        let before = t.archives[0].packets.iter().filter(|p| p.vsame).count();
        let c = Container::load(&src).unwrap();
        let out = s.w("expanded.Ghost.Gbx");
        let body = t.splice_into(c.body(), Encoding::Explicit).unwrap();
        crate::container::write_gbx(&c.gbx, body, &out).unwrap();
        let back = Tape::from_file(&out).unwrap();
        let after = back.archives[0].packets.iter().filter(|p| p.vsame).count();
        let same_values = t.archives[0]
            .packets
            .iter()
            .zip(back.archives[0].packets.iter())
            .all(|(p, q)| p.steer == q.steer && p.accel == q.accel && p.brake == q.brake);
        s.check(
            "tape.expand",
            before > 0 && after == 0 && same_values,
            format!(
                "{} one-bit \"same as previous tick\" packets expanded to explicit fields; {} left; every input value unchanged",
                before, after
            ),
        );
        s.check(
            "tape.expand.writable",
            back.archives[0].packets.len() == t.n(),
            format!("all {} ticks are now individually writable", t.n()),
        );
    }

    // --- a respawn is an editable input --------------------------------------
    {
        let src = s.f(GHOSTS[0]);
        let mut t = Tape::from_file(&src).unwrap();
        // find a packet with an explicit literal and set its respawn bit
        let idx = t.archives[0]
            .packets
            .iter()
            .position(|p| matches!(p.state, StateEnc::Lit(_)));
        match idx {
            None => s.skip("tape.respawn", "this fixture has no explicit state literal"),
            Some(i) => {
                let before = t.archives[0].packets[i].respawn();
                if let StateEnc::Lit(l) = t.archives[0].packets[i].state {
                    t.archives[0].packets[i].state = StateEnc::Lit(l | (1 << 31));
                }
                let c = Container::load(&src).unwrap();
                let out = s.w("respawn.Ghost.Gbx");
                let body = t.splice_into(c.body(), Encoding::Explicit).unwrap();
                crate::container::write_gbx(&c.gbx, body, &out).unwrap();
                let back = Tape::from_file(&out).unwrap();
                s.check(
                    "tape.respawn",
                    !before && back.archives[0].packets[i].respawn(),
                    format!("respawn written at tick {} and read back (bit 31 of the state literal)", i),
                );
            }
        }
        // and a respawn on a repeated word is refused rather than dropped
        let txt = "#gtape 1\n@archive 0 format_version=12 field0=0 start_offset_ms=0 packets=1 bitstream_bytes=0 bits_used=0\nt=0 mode=2 w=prev respawn=1 mouse=none vsame=0 steer=0 accel=0 brake=0\n";
        s.check(
            "tape.respawn.refused",
            Tape::from_text(txt).is_err(),
            "respawn=1 on a w=prev packet is REFUSED, not silently dropped -- the bit only exists in a literal",
        );
    }

    // --- the recorded steer byte -------------------------------------------
    {
        let cases: [(i8, u8); 7] = [(0, 127), (127, 255), (-127, 0), (60, 187), (4, 131), (-101, 26), (-22, 105)];
        let ok = cases.iter().all(|(v, b)| crate::regen::steer_byte(*v) == *b);
        s.check(
            "record.steerbyte",
            ok,
            "byte 14 = floor((steer + 127) * 255 / 254) -- the FLOOR and the 254 both matter, and both were measured",
        );
    }

    // --- identity ------------------------------------------------------------
    {
        let c = Container::load(&s.f(GHOSTS[0])).unwrap();
        let f = ident::scan(&c);
        let get = |r: Role| f.iter().find(|x| x.role == r).map(|x| x.s.clone());
        let ok = get(Role::Nickname).as_deref() == Some("hobbi.")
            && get(Role::Trigram).as_deref() == Some("HOB")
            && get(Role::Zone).as_deref() == Some("World|Europe|Germany")
            && get(Role::AccountId).as_deref() == Some("I0PKZ6d8R8iDvXT5nG8KNw")
            && get(Role::Skin).map_or(false, |x| x.contains("Hans Sub Red"))
            && get(Role::Locator).map_or(false, |x| x.starts_with("https://core.trackmania"));
        s.check(
            "identity.read",
            ok,
            "display name, trigram, zone, skin, locator URL and the inline account id are all found",
        );
        // the account id lives in a NON-skippable chunk; a scan that only walks
        // skippable chunks misses it, which is how one survives an anonymiser
        s.check(
            "identity.inline_chunk",
            get(Role::AccountId).is_some() && !c.chunks().iter().any(|k| k.0 == 0x0309200F),
            "the account id is found in inline chunk 0x0309200F, which has no PIKS marker and is not in the chunk table",
        );
    }

    // --- the map is inside the replay ---------------------------------------
    {
        let rc = Container::load(&s.f(REPLAY)).unwrap();
        let gc = Container::load(&s.f(GHOSTS[0])).unwrap();
        s.check(
            "map.embedded.detect",
            rc.embedded_map().is_some() && gc.embedded_map().is_none(),
            format!(
                "the replay carries a {} B map (chunk 0x03093002, no PIKS marker); the pure ghost carries none",
                rc.embedded_map().map(|x| x.1).unwrap_or(0)
            ),
        );
        // put the carried map back and require the body to be unchanged
        let mo = rc.embedded_map().unwrap();
        let mp = s.w("carried.Map.Gbx");
        std::fs::write(&mp, &rc.body()[mo.0..mo.0 + mo.1]).unwrap();
        let out = s.w("same.Replay.Gbx");
        let st = std::process::Command::new(std::env::current_exe().unwrap())
            .args(["map", "set", &s.f(REPLAY), &out, "--map", &mp])
            .output()
            .unwrap();
        let same = Container::load(&out).map(|c2| c2.body() == rc.body()).unwrap_or(false);
        s.check(
            "map.set.roundtrip",
            st.status.success() && same,
            "replacing the carried map with itself leaves the body byte-identical",
        );
    }

    // --- trim ---------------------------------------------------------------
    {
        let src = s.f(GHOSTS[0]);
        let out = s.w("trim15.Ghost.Gbx");
        let st = std::process::Command::new(std::env::current_exe().unwrap())
            .args(["trim", &src, &out, "--to", "15000", "--no-oracle"])
            .output()
            .unwrap();
        if !st.status.success() {
            s.fail("trim.coherent", String::from_utf8_lossy(&st.stderr).to_string());
        } else {
            let t = Tape::from_file(&out).unwrap();
            let c = Container::load(&out).unwrap();
            let d = tmtraj::entrec::decode_ghost(&out).unwrap();
            let a0 = &t.archives[0];
            let last_tick_ms = a0.start_offset_ms as i64 + 10 * (a0.packets.len() as i64 - 1);
            let decl: Vec<u32> = c.declared_times().into_iter().map(|x| x.1).collect();
            let cps = tmtraj::entrec::read_checkpoints(c.body());
            let ok = last_tick_ms <= 15000
                && d.samples.last().map_or(false, |x| x.time_ms <= 15000)
                && d.end_ms <= 15000
                && decl.iter().all(|v| *v == 15000)
                && cps.iter().all(|v| *v <= 15000);
            s.check(
                "trim.coherent",
                ok,
                format!(
                    "tape ends {}, last sample {}, record span ends {}, declared {:?}, checkpoints {:?} -- all inside the window",
                    secs(last_tick_ms),
                    secs(d.samples.last().map(|x| x.time_ms).unwrap_or(0) as i64),
                    secs(d.end_ms as i64),
                    decl.iter().map(|v| secs(*v as i64)).collect::<Vec<_>>(),
                    cps.iter().map(|v| secs(*v as i64)).collect::<Vec<_>>()
                ),
            );
        }
    }

    // --- the tape/record agreement separates the two populations -------------
    {
        let good = crate::verify::tape_record_agreement(&s.f(GHOSTS[0]));
        let bad = crate::verify::tape_record_agreement(&s.f(POISONED));
        match (good, bad) {
            (Some((kg, _, _, _)), Some((kb, _, _, _))) => {
                s.check(
                    "record.agreement",
                    kg > 0.9 && kb < 0.3,
                    format!(
                        "kappa {:.3} on a recording that belongs to its tape, {:.3} on the file this project itself labelled DO_NOT_PUBLISH",
                        kg, kb
                    ),
                );
            }
            _ => s.fail("record.agreement", "could not measure one of the two fixtures"),
        }
    }

    // --- refusals -------------------------------------------------------------
    {
        // injecting a tape of the wrong length must be refused
        let src = s.f(GHOSTS[0]);
        let t = Tape::from_file(&src).unwrap();
        let mut short = t.clone();
        short.archives[0].packets.truncate(100);
        let tp = s.w("short.gtape");
        std::fs::write(&tp, short.to_text("short")).unwrap();
        let out = s.w("short.Ghost.Gbx");
        let st = std::process::Command::new(std::env::current_exe().unwrap())
            .args(["tape", "inject", &src, &out, "--tape", &tp])
            .output()
            .unwrap();
        s.check(
            "refuse.length_mismatch",
            !st.status.success(),
            "injecting a 100-tick tape into a 2432-tick container is refused, not padded",
        );
        // rebinding a file that carries its own map must be refused
        let out2 = s.w("rebound.Replay.Gbx");
        let st2 = std::process::Command::new(std::env::current_exe().unwrap())
            .args(["map", "rebind", &s.f(REPLAY), &out2, "--map", &s.f(MAP)])
            .output()
            .unwrap();
        s.check(
            "refuse.rebind_replay",
            !st2.status.success(),
            "rewriting the uid of a file that CARRIES a map is refused -- it would claim one map and run another",
        );
    }
}

// ---------------------------------------------------------------------------

fn oracle_tier(s: &mut Suite) {
    let Some(server) = s.server.clone() else {
        s.skip("oracle.tier", "no dedicated server (set TM_SERVER or --server DIR)");
        return;
    };
    let map = s.f(MAP);
    let mapp = Path::new(&map);

    // O1 the donor validates to what it declares
    let r = oracle::validate(&server, Path::new(&s.f(GHOSTS[0])), MapsMode::One(mapp), "o1");
    match r {
        Ok(v) => s.check(
            "oracle.donor",
            v.time_ms == Some(DONOR_MS),
            format!("the donor re-simulates to {} (declared {})", v.secs(), secs(DONOR_MS)),
        ),
        Err(e) => s.fail("oracle.donor", e),
    }

    // O2 expansion is a semantic no-op
    {
        let out = s.w("o2.Ghost.Gbx");
        run_self(&["tape", "expand", &s.f(GHOSTS[0]), &out]);
        match oracle::validate(&server, Path::new(&out), MapsMode::One(mapp), "o2") {
            Ok(v) => s.check(
                "oracle.expand_noop",
                v.time_ms == Some(DONOR_MS),
                format!("expanding every same-as-previous packet leaves the run at {}", v.secs()),
            ),
            Err(e) => s.fail("oracle.expand_noop", e),
        }
    }

    // O3 a full extract -> inject round trip is a no-op
    {
        let tp = s.w("o3.gtape");
        let out = s.w("o3.Ghost.Gbx");
        run_self(&["tape", "extract", &s.f(GHOSTS[0]), "--out", &tp]);
        run_self(&["tape", "inject", &s.f(GHOSTS[0]), &out, "--tape", &tp]);
        match oracle::validate(&server, Path::new(&out), MapsMode::One(mapp), "o3") {
            Ok(v) => s.check(
                "oracle.inject_noop",
                v.time_ms == Some(DONOR_MS),
                format!("extract -> inject reproduces the run exactly: {}", v.secs()),
            ),
            Err(e) => s.fail("oracle.inject_noop", e),
        }
    }

    // O4 an edited tick actually changes the run (the writer is not a no-op),
    // and the stale declaration it leaves behind is caught
    {
        let src = s.f(GHOSTS[0]);
        let mut t = Tape::from_file(&src).unwrap();
        // a one-unit steer nudge over 300 ms: small enough to still finish,
        // large enough that the finish moves
        for p in t.archives[0].packets.iter_mut().skip(1800).take(30) {
            p.steer = (p.steer_i8().saturating_add(1)) as u8 as u32;
        }
        let c = Container::load(&src).unwrap();
        let out = s.w("o4.Ghost.Gbx");
        let body = t.splice_into(c.body(), Encoding::Explicit).unwrap();
        crate::container::write_gbx(&c.gbx, body, &out).unwrap();
        let r = oracle::validate(&server, Path::new(&out), MapsMode::One(mapp), "o4");
        match r {
            Ok(ref v) => s.check(
                "oracle.edit_bites",
                v.time_ms == Some(22738),
                format!(
                    "one steer unit for 300 ms moves the finish from {} to {} -- the writer is not a no-op",
                    secs(DONOR_MS),
                    v.secs()
                ),
            ),
            Err(ref e) => s.fail("oracle.edit_bites", e.clone()),
        }
        // THE SERVER PRINTS TWO RESULTS AND THE SECOND IS THE FILE'S OWN CLAIM.
        // This file simulates 22.738 and still declares 22.730, so it is the
        // fixture that catches a parser reading the claim instead of the answer
        // -- which this tool did, until this check was written. NOTE THAT THE
        // TWO NUMBERS MUST DIFFER: on a file that passes they are equal, and no
        // equal-number fixture can fail, whatever the parser does.
        let declared: Vec<u32> = Container::load(&out).unwrap().declared_times().into_iter().map(|x| x.1).collect();
        let r2 = oracle::validate(&server, Path::new(&out), MapsMode::One(mapp), "o4b");
        match r2 {
            Err(e) => s.fail("oracle.reads_the_world", e),
            Ok(v) => {
                s.check(
                    "oracle.reads_the_world",
                    v.time_ms == Some(22738) && v.declared_ms == Some(22730) && declared == vec![22730],
                    format!(
                        "the server reports BOTH: simulated {} and declared {}. They differ, so a parser that took the second would fail this check",
                        v.secs(),
                        v.declared_ms.map(secs).unwrap_or("none".into())
                    ),
                );
                s.check(
                    "oracle.declaration_holds",
                    !v.declaration_holds(),
                    "and `declaration_holds()` says so, which is the one comparison the search layer needs",
                );
            }
        }
        let vr = crate::verify::run(&out, &["--map".into(), map.clone(), "--server".into(), server.to_string_lossy().to_string()]);
        s.check(
            "verify.stale_declaration",
            vr.failed(),
            "and `ghost verify` REFUSES that file, because a run that declares one time and does another is exactly the container bug this project keeps paying for",
        );
    }

    // O4c THE OTHER ASYMMETRIC SHAPE: a DNF whose DeclaredResult still carries
    // a time. `ValidatedResult` is null and `DeclaredResult` says 15.000, so a
    // parser that reads any `"Time"` reports a 15.000 finish for a run that
    // never finished. The trimmed file is exactly that fixture.
    {
        let out = s.w("o4c.Ghost.Gbx");
        run_self(&["trim", &s.f(GHOSTS[0]), &out, "--to", "15000", "--no-oracle"]);
        match oracle::validate(&server, Path::new(&out), MapsMode::One(mapp), "o4c") {
            Err(e) => s.fail("oracle.dnf_with_declared_time", e),
            Ok(v) => s.check(
                "oracle.dnf_with_declared_time",
                v.time_ms.is_none() && v.declared_ms == Some(15000),
                format!(
                    "ValidatedResult is null and DeclaredResult says {} -- reported as DNF, not as a finish ({})",
                    v.declared_ms.map(secs).unwrap_or("none".into()),
                    v.desc
                ),
            ),
        }
    }

    // O4d BATCH VALIDATION. The server validates in batches and the per-launch
    // cost dominates the per-file cost, so this is the entry point the search
    // layer uses; it has to key every result to the right file.
    {
        let a = s.f(GHOSTS[0]);
        let b = s.f(GHOSTS[1]);
        let c = s.w("o4.Ghost.Gbx");
        let files: Vec<&Path> = vec![Path::new(&a), Path::new(&b), Path::new(&c)];
        match oracle::validate_many(&server, &files, MapsMode::One(mapp), "o4d") {
            Err(e) => s.fail("oracle.batch", e),
            Ok(v) => {
                let by = |n: &str| v.iter().find(|r| r.file.contains(n)).and_then(|r| r.time_ms);
                s.check(
                    "oracle.batch",
                    v.len() == 3
                        && by("human_22730") == Some(DONOR_MS)
                        && by("human_23013") == Some(23013)
                        && by("o4") == Some(22738),
                    format!(
                        "three files in ONE server launch, each keyed to its own name: {}",
                        v.iter().map(|r| format!("{} {}", r.file, r.secs())).collect::<Vec<_>>().join(", ")
                    ),
                );
                // The engine echoes the tape it decoded. Two files with the same
                // tape must produce the same echo, and the edited one must not.
                let inp: Vec<&str> = v.iter().map(|r| r.inputs.as_str()).collect();
                s.check(
                    "oracle.inputs_echo",
                    inp.len() == 3 && !inp[0].is_empty() && inp[0] != inp[1],
                    "the server's own echo of the decoded tape comes back with each result",
                );
            }
        }
    }

    // O5 THE MAP IS INSIDE THE REPLAY -- the empty-Maps control
    {
        match oracle::validate(&server, Path::new(&s.f(REPLAY)), MapsMode::Empty, "o5a") {
            Ok(v) => s.check(
                "oracle.map_inside_replay",
                v.time_ms == Some(REPLAY_MS),
                format!(
                    "the replay validates to {} with ZERO maps on disk -- the map it runs on comes out of the file",
                    v.secs()
                ),
            ),
            Err(e) => s.fail("oracle.map_inside_replay", e),
        }
        match oracle::validate(&server, Path::new(&s.f(GHOSTS[0])), MapsMode::Empty, "o5b") {
            Ok(v) => s.check(
                "oracle.pure_ghost_needs_map",
                v.time_ms.is_none(),
                "the pure ghost returns nothing with an empty Maps directory -- for it, --map is real",
            ),
            Err(e) => s.fail("oracle.pure_ghost_needs_map", e),
        }
    }

    // O6 changing the carried map actually moves the recording
    {
        let out = s.w("o6same.Replay.Gbx");
        let mp = s.w("carried.Map.Gbx");
        run_self(&["map", "set", &s.f(REPLAY), &out, "--map", &mp]);
        match oracle::validate(&server, Path::new(&out), MapsMode::Empty, "o6a") {
            Ok(v) => s.check(
                "oracle.map_set_roundtrip",
                v.time_ms == Some(REPLAY_MS),
                format!("putting the carried map back leaves the answer at {}", v.secs()),
            ),
            Err(e) => s.fail("oracle.map_set_roundtrip", e),
        }
        let out2 = s.w("o6moved.Replay.Gbx");
        run_self(&["map", "set", &s.f(REPLAY), &out2, "--map", &map]);
        match oracle::validate(&server, Path::new(&out2), MapsMode::Empty, "o6b") {
            Ok(v) => s.check(
                "oracle.map_set_moves",
                v.time_ms != Some(REPLAY_MS),
                format!(
                    "swapping a DIFFERENT map in changes the answer to {} -- on a box where the original map does not exist at all",
                    v.secs()
                ),
            ),
            Err(e) => s.fail("oracle.map_set_moves", e),
        }
    }

    // O7 identity edits are cosmetic
    {
        let out = s.w("o7.Ghost.Gbx");
        let st = run_self(&[
            "identity", "set", &s.f(GHOSTS[0]), &out, "--name", "TASvjeux", "--trigram", "VJX",
            "--anonymise", "--map", &map,
        ]);
        let v = oracle::validate(&server, Path::new(&out), MapsMode::One(mapp), "o7");
        let idok = Container::load(&out)
            .map(|c| {
                let f = ident::scan(&c);
                f.iter().find(|x| x.role == Role::Trigram).map(|x| x.s.clone()) == Some("VJX".into())
                    && !f.iter().any(|x| x.role == Role::Locator && !x.s.is_empty())
            })
            .unwrap_or(false);
        s.check(
            "oracle.identity_noop",
            st && idok && v.map(|x| x.time_ms) == Ok(Some(DONOR_MS)),
            "renaming the driver, setting a trigram and dropping the account id and locator URL leaves the run at 22.730",
        );
    }

    // O8 trimming
    {
        let out = s.w("o8.Ghost.Gbx");
        run_self(&["trim", &s.f(GHOSTS[0]), &out, "--to", "15000", "--no-oracle"]);
        match oracle::validate(&server, Path::new(&out), MapsMode::One(mapp), "o8a") {
            Ok(v) => s.check(
                "oracle.trim_partial_dnf",
                v.time_ms.is_none(),
                "a run cut at 15.000 does not finish -- which is the truth, and the tool says so instead of declaring a time",
            ),
            Err(e) => s.fail("oracle.trim_partial_dnf", e),
        }
        let out2 = s.w("o8b.Ghost.Gbx");
        run_self(&["trim", &s.f(GHOSTS[0]), &out2, "--to", "22730", "--no-oracle"]);
        match oracle::validate(&server, Path::new(&out2), MapsMode::One(mapp), "o8b") {
            Ok(v) => s.check(
                "oracle.trim_keeps_finish",
                v.time_ms == Some(DONOR_MS),
                format!("cutting the post-finish tail keeps the finish: {}", v.secs()),
            ),
            Err(e) => s.fail("oracle.trim_keeps_finish", e),
        }
    }

    // O9 rebinding a pure ghost: the uid IS the binding, proved both ways
    {
        let carried = s.w("carried.Map.Gbx");
        let away = s.w("o9away.Ghost.Gbx");
        let back = s.w("o9back.Ghost.Gbx");
        let ok1 = run_self(&["map", "rebind", &s.f(GHOSTS[0]), &away, "--map", &carried]);
        let ok2 = run_self(&["map", "rebind", &away, &back, "--map", &map]);
        let a = oracle::validate(&server, Path::new(&away), MapsMode::One(mapp), "o9a");
        let b = oracle::validate(&server, Path::new(&back), MapsMode::One(mapp), "o9b");
        match (a, b) {
            (Ok(av), Ok(bv)) => s.check(
                "oracle.rebind",
                ok1 && ok2 && av.time_ms.is_none() && bv.time_ms == Some(DONOR_MS),
                format!(
                    "rebinding the donor AWAY from this map makes it stop validating on it ({}), and rebinding it BACK restores the run exactly ({}) -- so the uid is the binding, and this command sets it",
                    av.secs(),
                    bv.secs()
                ),
            ),
            _ => s.fail("oracle.rebind", "the oracle did not run"),
        }
    }
}

fn engine_tier(s: &mut Suite) {
    if s.server.is_none() {
        s.skip("engine.tier", "no dedicated server");
        return;
    }
    let map = s.f(MAP);
    let src = s.f(GHOSTS[0]);
    // E1 the engine's own run of this tape must match the recording in it
    match crate::regen::engine_trajectory_agreement(&src, &map) {
        Err(e) => s.fail("engine.trajectory", e),
        Ok((mean, worst, n, shift)) => s.check(
            "engine.trajectory",
            !shift && mean < 0.05,
            format!(
                "running the fixture's own tape through the real engine reproduces its recorded trajectory to {:.4} m mean / {:.4} m worst over {} samples{}",
                mean,
                worst,
                n,
                if shift { " -- BUT a whole sample out of phase" } else { "" }
            ),
        ),
    }
    // E2 THE LOCATE IS DETERMINISTIC. Two regenerations of the same file must
    // produce the same trajectory, bit for bit. This is the check that would
    // have failed on the old locate, which found the car about one run in six
    // and wrote a different answer the rest of the time.
    let a = s.w("e2a.Ghost.Gbx");
    let b = s.w("e2b.Ghost.Gbx");
    let ok1 = run_self(&["regen", &src, &a, "--map", &map]);
    let ok2 = run_self(&["regen", &src, &b, "--map", &map]);
    if !ok1 || !ok2 {
        s.fail("engine.determinism", "a regeneration did not pass its own gate");
        return;
    }
    let (da, db) = (
        tmtraj::entrec::decode_ghost(&a).ok(),
        tmtraj::entrec::decode_ghost(&b).ok(),
    );
    match (da, db) {
        (Some(x), Some(y)) => {
            let n = x.samples.len().min(y.samples.len());
            let mut worst = 0.0f64;
            for i in 0..n {
                let (p, q) = (&x.samples[i], &y.samples[i]);
                worst = worst.max(
                    ((p.x - q.x).powi(2) + (p.y - q.y).powi(2) + (p.z - q.z).powi(2)).sqrt(),
                );
            }
            s.check(
                "engine.determinism",
                worst == 0.0 && n > 100,
                format!(
                    "two independent regenerations of the same file agree to {:.6} m over {} samples",
                    worst, n
                ),
            );
        }
        _ => s.fail("engine.determinism", "could not decode one of the two regenerations"),
    }
}

fn run_self(args: &[&str]) -> bool {
    let st = std::process::Command::new(std::env::current_exe().unwrap())
        .args(args)
        .output()
        .unwrap();
    if !st.status.success() {
        eprintln!("       (ghost {} failed: {})", args[0], String::from_utf8_lossy(&st.stderr).trim());
    }
    st.status.success()
}
