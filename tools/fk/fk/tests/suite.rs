//! The whole suite, one command.
//!
//! ```text
//! cargo test --release                      # pure tests only, ~1 s
//! FK_ENGINE=1 cargo test --release          # + the real dedicated server
//! ```
//!
//! `fk` had **no tests at all** before this file. That is worth stating plainly
//! rather than fixing quietly, because it explains the shape of what is here:
//! these are not unit tests of the arithmetic, they are the checks that would
//! have caught the defects this project actually shipped.
//!
//! # Two tiers, and why the engine one is opt-in rather than absent
//!
//! **PURE** needs nothing but the checked-in fixtures and runs in about a
//! second. **ENGINE** needs a TrackmaniaServer install and takes minutes.
//! Making the engine tier opt-in is a real risk — an opt-in test is a test that
//! does not run — so the suite prints a loud line saying the tier was skipped,
//! and `FK_ENGINE_STRICT=1` turns a skip into a failure for anyone wiring this
//! into something automatic.
//!
//! # The fixtures are `ghost`'s
//!
//! `tools/testdata/` already holds anonymised ghosts, a replay that
//! carries its own map, and a map. A second corpus of the same files in
//! `tools/fk/testdata/` would be one more thing to keep in step, so this suite
//! reads ghost's. What lives in `fk`'s own testdata is only what is `fk`'s:
//! captured dedicated-server output, which is the one input `fk` parses that
//! nothing else does.

use std::path::{Path, PathBuf};

// ---------------------------------------------------------------- fixtures

fn ghost_testdata() -> PathBuf {
    // tools/fk/fk/tests/ -> tools/testdata/
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../testdata")
        .canonicalize()
        .expect("tools/testdata must exist -- fk shares ghost's fixture corpus")
}

fn fk_testdata() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("testdata")
}

fn human_ghost() -> String {
    ghost_testdata().join("human_22730.Ghost.Gbx").to_string_lossy().into()
}

fn map2() -> PathBuf {
    ghost_testdata().join("map2.Map.Gbx")
}

fn engine_tier() -> Option<PathBuf> {
    let dir = std::env::var("TM_SERVER").unwrap_or_else(|_| "/tmp/tmoracle/server".into());
    let p = PathBuf::from(dir);
    if std::env::var("FK_ENGINE").is_ok() && p.join("TrackmaniaServer").exists() {
        return Some(p);
    }
    if std::env::var("FK_ENGINE_STRICT").is_ok() {
        panic!(
            "FK_ENGINE_STRICT is set but there is no dedicated server at {} -- \
             a strict run with the engine tier absent is not a pass",
            p.display()
        );
    }
    eprintln!(
        "SKIP engine tier (set FK_ENGINE=1 and TM_SERVER=<dir>). \
         The pure tier cannot see a resume, a locate or an oracle answer."
    );
    None
}

fn shim() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../search/target/release/libforkshim.so")
        .canonicalize()
        .expect("build the workspace first: cargo build --release")
}

fn scratch(tag: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("fktest-{}-{}", tag, std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

// =========================================================================
// PURE -- the oracle's own output
// =========================================================================

/// **The server prints TWO times per file and only one of them is an answer.**
///
/// `ValidatedResult` is what the engine simulated; `DeclaredResult` is what the
/// FILE CLAIMS. A parser that reads to the end of the block and keeps the last
/// `"Time"` it sees reports the file's own declaration back as though it were
/// the oracle's — i.e. it confirms whatever the file says about itself, which is
/// the phantom-result failure mode with the oracle removed from the loop.
///
/// **No fixture in which the two numbers agree can fail this test**, and until
/// now every fixture anyone had was a passing file where they agree exactly.
/// This one is a real capture of the real server on a candidate that finishes
/// 8 ms away from what it declares.
#[test]
fn oracle_reports_what_was_simulated_not_what_the_file_claims() {
    let raw = std::fs::read_to_string(fk_testdata().join("server_finish_declared_differs.txt"))
        .expect("fixture");
    let r = ghost::oracle::parse_many(&raw);
    assert_eq!(r.len(), 1, "one file in, one result out");
    assert_eq!(r[0].time_ms, Some(22_754), "the SIMULATED time");
    assert_eq!(r[0].declared_ms, Some(22_730), "the DECLARED time");
    assert!(
        !r[0].declaration_holds(),
        "a file that runs 22.738 and declares 22.730 does not do what it says"
    );
    assert_eq!(fk::record::sim_time_of(&raw), Some(22_754));
}

/// **A DNF's only `"Time"` is the file's declaration.**
///
/// `"ValidatedResult" : null` carries no time at all, so "find the first line
/// starting with `Time`" — which is what `fk`'s regen path did until this audit
/// — answers *this run finished at the time written in the file* for a run that
/// did not finish. That value fed `race_end`, which decides which recorded
/// instants count as inside the race, so the error propagated into which
/// samples a regenerated ghost was allowed to inherit from its donor.
#[test]
fn oracle_dnf_does_not_report_the_declared_time() {
    let raw = std::fs::read_to_string(fk_testdata().join("server_dnf_with_declared_time.txt"))
        .expect("fixture");
    let r = ghost::oracle::parse_many(&raw);
    assert_eq!(r.len(), 1);
    assert_eq!(r[0].time_ms, None, "this run did not finish");
    assert!(r[0].declared_ms.is_some(), "and the file still declares a time");
    assert_eq!(
        fk::record::sim_time_of(&raw),
        None,
        "a DNF must read as DNF, not as the number written in the file"
    );
}

// =========================================================================
// PURE -- the tape
// =========================================================================

/// Extract, inject, extract: the three input channels survive a round trip.
#[test]
fn tape_round_trips_byte_identically() {
    let t = fk::tape::Tape::load(&human_ghost()).unwrap();
    t.codec_is_lossless()
        .expect("a verbatim re-encode must reproduce the file's own bitstream");
    let out = scratch("rt").join("rt.Ghost.Gbx");
    t.write_candidate(&t.steer, &t.accel, &t.brake, &out).unwrap();
    let b = fk::tape::Tape::load(&out.to_string_lossy()).unwrap();
    assert_eq!(t.steer, b.steer);
    assert_eq!(t.accel, b.accel);
    assert_eq!(t.brake, b.brake);
    assert_eq!(t.start_offset_ms, b.start_offset_ms);
    assert_eq!(t.n(), b.n());
}

/// **EVERY tick is writable, including the "same as previous tick" ones.**
///
/// The codec `fk` used before this audit re-encoded mode-12 same-input packets
/// as a one-bit form it could not patch, and a write to such a tick was
/// silently dropped. It never bit because those ticks sat below every resume
/// boundary — a property of the boundaries we happen to use, not a guarantee.
///
/// Writing a distinct value into every tick and reading them all back is the
/// only shape of test that can see this: a test that patches one tick passes on
/// a codec that drops 3 ticks in 2432.
#[test]
fn tape_every_tick_is_writable() {
    let t = fk::tape::Tape::load(&human_ghost()).unwrap();
    let n = t.n();
    let want: Vec<u8> = (0..n).map(|i| ((i % 200) as i32 - 100) as i8 as u8).collect();
    let accel: Vec<u8> = (0..n).map(|i| (i % 2) as u8).collect();
    let brake: Vec<u8> = (0..n).map(|i| ((i + 1) % 2) as u8).collect();
    let out = scratch("allticks").join("all.Ghost.Gbx");
    t.write_candidate(&want, &accel, &brake, &out).unwrap();
    let b = fk::tape::Tape::load(&out.to_string_lossy()).unwrap();
    let bad: Vec<usize> = (0..n)
        .filter(|&i| b.steer[i] != want[i] || b.accel[i] != accel[i] || b.brake[i] != brake[i])
        .collect();
    assert!(
        bad.is_empty(),
        "{} of {} ticks did not take the value they were written: first at {:?}",
        bad.len(),
        n,
        bad.first()
    );
}

/// A tape with more than one input archive is refused, not silently read as
/// archive 0.
#[test]
fn tape_refuses_a_file_it_would_have_to_guess_about() {
    // The replay fixture is the only multi-archive-shaped container to hand; if
    // it happens to hold exactly one, the refusal is still worth asserting on
    // the code path, so assert only when the premise holds.
    let p = ghost_testdata().join("replay_kacky_7241.Replay.Gbx");
    if let Ok(g) = gbx::tape::Tape::from_file(&p.to_string_lossy()) {
        if g.archives.len() > 1 {
            let e = match fk::tape::Tape::load(&p.to_string_lossy()) {
                Err(e) => e,
                Ok(_) => panic!("a multi-archive file must be refused, not read as archive 0"),
            };
            assert!(e.contains("will not guess"), "{}", e);
        }
    }
}

/// **The extension guard.** The dedicated server ignores a file that is not
/// named `*.Ghost.Gbx` / `*.Replay.Gbx` and returns a plain DNF you cannot tell
/// from a real one. The `ghost` arm lost 32 consecutive GOOD regenerations to
/// exactly this before anyone noticed the gate was refusing files that were
/// fine.
#[test]
fn a_file_the_oracle_cannot_read_is_an_error_not_a_dnf() {
    use fk::tape::check_oracle_readable_name;
    assert!(check_oracle_readable_name(Path::new("/tmp/a.Ghost.Gbx")).is_ok());
    assert!(check_oracle_readable_name(Path::new("/tmp/a.Replay.Gbx")).is_ok());
    for bad in ["/tmp/a.gbx", "/tmp/a.Ghost.gbx", "/tmp/cand0001", "/tmp/a.Ghost.Gbx.tmp"] {
        let e = check_oracle_readable_name(Path::new(bad)).unwrap_err();
        assert!(e.contains("bare DNF"), "{} should be refused: {}", bad, e);
    }
}

// =========================================================================
// PURE -- the encodings, pinned to what was MEASURED
// =========================================================================

/// **`floor`, and 254 — checked against the corpus, not against my arithmetic.**
///
/// The steer echo in a telemetry sample is `floor((steer_i8 + 127) * 255 / 254)`.
/// `fk` wrote `round((steer_i8 / 127 + 1) / 2 * 255)`, and a regenerated ghost
/// therefore disagreed with its own inputs on half its samples: `ghost verify`
/// V6 read kappa 0.467, and 1.000 after the fix.
///
/// **The first version of this test asserted that the two encodings differ at
/// exactly steer 0 and steer 60, and that was wrong** — they differ at 127 of
/// the 255 values. I had taken "a round misses steer = 0 and steer = 60" out of
/// a write-up and turned it into a claim about all steer values without
/// measuring it. So the test now measures: every sample of a real recording,
/// against that recording's own tape. A known-answer test on a file the game
/// wrote cannot be wrong about the game in the way I just was.
#[test]
fn steer_echo_matches_a_real_recording_byte_for_byte() {
    let g = human_ghost();
    let tape = fk::tape::Tape::load(&g).unwrap();
    let (times, raws) = fk::record::targets_from_ghost(&g).unwrap();
    let echo = |s: i32| -> u8 { (((s + 127) * 255 / 254) as u8).min(255) };
    let round = |s: i32| -> u8 { ((((s as f64 / 127.0) + 1.0) / 2.0 * 255.0).round()) as u8 };

    let (mut n, mut floor_bad, mut round_bad) = (0usize, 0usize, 0usize);
    for (i, &ms) in times.iter().enumerate() {
        let t = (ms - tape.start_offset_ms as i64) / 10;
        if t < 0 || t as usize >= tape.n() {
            continue;
        }
        let s = tape.steer[t as usize] as i8 as i32;
        let recorded = raws[i][14];
        n += 1;
        if echo(s) != recorded {
            floor_bad += 1;
        }
        if round(s) != recorded {
            round_bad += 1;
        }
    }
    assert!(n > 400, "only {} samples lined up with the tape", n);
    assert_eq!(floor_bad, 0, "the floor encoding must reproduce all {} recorded bytes", n);
    assert!(
        round_bad > n / 4,
        "POSITIVE CONTROL: the round encoding must visibly fail on the same data, \
         otherwise this test cannot tell the two apart. It missed {} of {}.",
        round_bad,
        n
    );
}

/// The neutralise list must not fight the writers.
///
/// A byte cannot be both zeroed and written from engine state. This is a
/// structural check on the list rather than on any file, so it catches an
/// offset added by hand later — which is how the list would go wrong, since it
/// is the residue of a byte-by-byte census nobody will redo.
#[test]
fn the_neutralise_list_is_disjoint_from_everything_we_write() {
    let n = fk::record::NEUTRALISE;
    assert_eq!(n.len(), 49, "the census found 49 per-run bytes");
    let mut sorted = n.to_vec();
    sorted.sort_unstable();
    sorted.dedup();
    assert_eq!(sorted.len(), n.len(), "no duplicates");
    for &o in n {
        assert!(!(47..69).contains(&o), "byte {} is inside the transform (47..69)", o);
        assert!(![14usize, 15, 18].contains(&o), "byte {} is the tape echo", o);
        assert!(o < 116, "byte {} is past the end of a 116-byte sample", o);
    }
    let mut w = fk::record::written_bytes(116, true, true);
    assert_eq!(w.iter().filter(|b| **b).count(), 22 + 49);
    w = fk::record::written_bytes(116, false, true);
    assert_eq!(w.iter().filter(|b| **b).count(), 49, "--keep-transform");
}

// =========================================================================
// PURE -- reporting
// =========================================================================

#[test]
fn times_print_as_seconds_with_a_decimal() {
    assert_eq!(fk::secs(22730), "22.730");
    assert_eq!(fk::secs(4492), "4.492");
    assert_eq!(fk::secs(36049), "36.049");
    assert_eq!(fk::secs_opt(None), "DNF");
}

/// **Quantiles, not an rms.** A respawn teleports the car tens of metres in one
/// tick, and that is a legitimate difference between the engine state and the
/// ghost's telemetry for about a second. A record with 31 respawns drags an
/// otherwise centimetre-exact match to an 8 m rms — the rms condemns a perfect
/// measurement. The median and the within-5-cm fraction do not.
#[test]
fn agreement_is_robust_to_a_respawn_sized_outlier() {
    use forkoracle::layout::Row;
    let row = |ms: i64, x: f64| Row {
        time_ms: ms,
        x,
        y: 0.0,
        z: 0.0,
        vx: 0.0,
        vy: 0.0,
        vz: 0.0,
        qx: 0.0,
        qy: 0.0,
        qz: 0.0,
        qw: 1.0,
        // Added to `Row` by the Cobalt Cove wetness arm; this test predates it
        // and does not exercise it. The test is about agreement being robust to
        // one respawn-sized outlier in x, so the value is irrelevant -- but the
        // field is not optional and leaving it out stopped `fk`'s whole suite
        // from COMPILING, which is worse than any failing test: a suite that
        // does not build reports nothing at all.
        wetness: 0.0,
    };
    let samples: Vec<fk::traj::Sample> = (0..100)
        .map(|i| fk::traj::Sample {
            time_ms: i * 50,
            x: i as f64,
            ..Default::default()
        })
        .collect();
    let reference = fk::traj::Reference { s: samples };
    // dead on everywhere except one 40 m jump, which is what a respawn looks
    // like from the outside.
    let mut rows: Vec<Row> = (0..99).map(|i| row(i * 50, i as f64)).collect();
    rows[40].x += 40.0;
    let a = fk::traj::compare(&rows, &reference).unwrap();
    assert!(a.median < 1e-9, "median {}", a.median);
    assert!(a.p90 < 1e-9, "p90 {}", a.p90);
    assert!((a.max - 40.0).abs() < 1e-9, "the outlier is still REPORTED: {}", a.max);
    assert!(a.within_5cm_pct > 98.0, "{}", a.within_5cm_pct);
}

/// The CSV `tmtraj decode --csv` writes, which the analysis tools index BY
/// NAME. The count is asserted so a column cannot be dropped silently; it moved
/// 29 -> 30 when the wetness arm added `wetness`, and the assertion is meant to
/// make that a deliberate edit rather than a surprise.
#[test]
fn trajectory_csv_has_the_columns_the_rest_of_the_project_reads() {
    assert_eq!(fk::traj::COLS.len(), 30);
    assert_eq!(fk::traj::COLS[0], "time_ms");
    for c in ["x", "y", "z", "vx", "vy", "vz", "qx", "qy", "qz", "qw", "steer", "gas", "brake",
              "wetness"] {
        assert!(fk::traj::COLS.contains(&c), "missing column {}", c);
    }
}

/// The fitted line is `clock = 36141 + 25.483 * race_ms`, and it is only ever
/// used to CHOOSE a checkpoint — never to label a sample, because the count is
/// not a fixed simulation point.
#[test]
fn the_checkpoint_line_is_the_fitted_one() {
    assert_eq!(fk::session::clock_for_race_ms(22730), 615_369);
    assert_eq!(fk::session::clock_for_race_ms(0), 36_141);
    assert!(fk::session::clock_for_race_ms(-100_000) >= 1000, "clamped, never negative");
}

// =========================================================================
// ENGINE -- needs a real dedicated server
// =========================================================================

/// **THE CONTROL.** A fork resume must give the same answer as a full
/// from-tick-0 validation, on the same candidates, including the DNFs.
///
/// This also exercises the identity resume, the page-fault probe, the boundary
/// calibration and the oracle's own repeatability, each of which fails the run
/// on its own.
#[test]
fn engine_fork_resume_reproduces_a_full_validation() {
    let Some(server) = engine_tier() else { return };
    let work = scratch("check");
    let engine = fk::session::Engine {
        server,
        map: map2(),
        shim: shim(),
        work: work.clone(),
        work_is_temporary: false,
    };
    let tape = fk::tape::Tape::load(&human_ghost()).unwrap();
    let ok = fk::cmd::server::check(
        &engine,
        tape,
        fk::session::Checkpoint::Fraction(0.949),
        fk::cmd::server::CheckOpts { n: 12, seed: 1, span: 60 },
    )
    .expect("the check ran");
    assert!(ok, "the fork server did not reproduce the full validation");
}

/// **The locate must FAIL LOUDLY when it cannot find the car, not return
/// garbage.**
///
/// This is the test the brief asked for: *when the game binary moves*. The shim
/// finds the engine's decoded input array by scanning the heap for the tape's
/// own steer sequence as `f32` at stride 32. If the binary's layout changes —
/// or, equivalently for the purposes of this test, if the key does not describe
/// what the server is running — there is nothing to find.
///
/// The negative alone would prove nothing: a run that aborts for an unrelated
/// reason looks the same. So the same server, the same map and the same tape
/// are started twice, once with the right key and once with a wrong one, and
/// the pair is the evidence.
#[test]
fn engine_locate_fails_loudly_when_the_key_does_not_match() {
    let Some(server) = engine_tier() else { return };
    let work = scratch("badkey");
    let tape = fk::tape::Tape::load(&human_ghost()).unwrap();
    let mk = |dir: &Path, steer: &[u8]| -> Result<(), String> {
        std::fs::create_dir_all(dir).unwrap();
        let g = dir.join("r.Ghost.Gbx");
        tape.write_reference(&g)?;
        let key = dir.join("key.bin");
        forkoracle::forksrv::write_key(&key, steer);
        forkoracle::forksrv::ForkServer::start(
            &dir.join("srv"),
            &server,
            &map2(),
            &g,
            &key,
            &shim(),
            fk::session::clock_for_race_ms(11_000),
        )
        .map(|s| s.quit())
    };

    // POSITIVE CONTROL: the right key finds the array.
    mk(&work.join("good"), &tape.steer)
        .expect("with the tape's own key the server must come up -- \
                 without this the negative below means nothing");

    // The binary moved / the key is not what is running: nothing to find.
    let mut wrong = tape.steer.clone();
    for (i, s) in wrong.iter_mut().enumerate() {
        *s = ((i % 251) as u8).wrapping_add(3);
    }
    let e = mk(&work.join("bad"), &wrong)
        .expect_err("a server that cannot locate the input array must refuse, not hand back \
                     an address it did not verify");
    assert!(
        e.contains("notfound") || e.to_lowercase().contains("handshake"),
        "the refusal should say the array was not found, got: {}",
        e
    );
}

/// A trajectory read out of engine memory must land on the reference ghost's own
/// recorded path.
///
/// The known-answer control: the fork stops with the reference's prefix behind
/// it, so the true position at that instant is known to ~mm from the ghost's own
/// telemetry, and every row after it can be compared with the interpolated path.
#[test]
fn engine_trace_lands_on_the_reference_path() {
    let Some(server) = engine_tier() else { return };
    let refcsv = fk_testdata().join("human_22730_telemetry.csv");
    if !refcsv.exists() {
        eprintln!("SKIP trace control: no reference telemetry fixture");
        return;
    }
    let work = scratch("trace");
    let engine = fk::session::Engine {
        server,
        map: map2(),
        shim: shim(),
        work: work.clone(),
        work_is_temporary: false,
    };
    let tape = fk::tape::Tape::load(&human_ghost()).unwrap();
    fk::cmd::trace::run(
        &engine,
        tape,
        fk::session::Checkpoint::Tick(60),
        fk::cmd::trace::TraceOpts {
            reference: Some(refcsv.to_string_lossy().into()),
            out: Some(work.join("t.csv").to_string_lossy().into()),
            nth: 1,
        },
    )
    .expect("the trace passed its own self-check and its known-answer control");

    // REPRODUCIBILITY. The locate re-derives every address at each server
    // start, because the heap layout is bimodal run to run -- two identical
    // runs of the server can differ by 87 MB. Five consecutive runs gave five
    // different addresses and byte-identical output, and that is the property
    // worth holding onto: if the locate starts picking a different copy of the
    // car, this is what says so. A trajectory that is merely PLAUSIBLE is the
    // failure this whole crate is built around.
    let first = std::fs::read(work.join("t.csv")).unwrap();
    fk::cmd::trace::run(
        &engine,
        fk::tape::Tape::load(&human_ghost()).unwrap(),
        fk::session::Checkpoint::Tick(60),
        fk::cmd::trace::TraceOpts {
            reference: Some(refcsv.to_string_lossy().into()),
            out: Some(work.join("t2.csv").to_string_lossy().into()),
            nth: 1,
        },
    )
    .expect("the second trace also passed its controls");
    let second = std::fs::read(work.join("t2.csv")).unwrap();
    assert_eq!(
        first, second,
        "two traces of the same tape at the same checkpoint must be byte-identical; \
         they are not, so the locate is choosing between copies of the car"
    );
}

/// **The whole regeneration path, end to end, pinning the two defects it had.**
///
/// `fk regen` runs the real engine on a ghost's own inputs and rewrites the
/// telemetry from what it read. Two things must hold on the file it writes, and
/// both were broken:
///
/// * the recording must agree with the tape it carries — `ghost`'s Cohen's
///   kappa on the exact steer byte. This read **0.467** while the echo was
///   written with a `round`, and **1.000** with the `floor` the game uses.
/// * the plain oracle must re-simulate the WRITTEN FILE to its declared time. A
///   banked incumbent is not a result until it does.
///
/// It runs the in-process locate (`--noanchor`), which is the path
/// `ghost regen` tries first and the one measured bit-identical across repeated
/// runs; the searching locate behind it is nondeterministic by nature (about
/// one run in eight finds a decoy on some maps) and is not something a test can
/// assert on without measuring a rate.
#[test]
fn engine_regen_writes_a_file_that_agrees_with_its_own_tape() {
    let Some(server) = engine_tier() else { return };
    let work = scratch("regen");
    let out = work.join("regen.Ghost.Gbx");
    let fk_bin = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../target/release/fk")
        .canonicalize()
        .expect("build the workspace first: cargo build --release");
    let st = std::process::Command::new(&fk_bin)
        .args([
            "regen",
            "--template",
            &human_ghost(),
            "--map",
            &map2().to_string_lossy(),
            "--out",
            &out.to_string_lossy(),
            "--dump",
            &work.join("d.bin").to_string_lossy(),
            "--shim",
            &shim().to_string_lossy(),
            "--server",
            &server.to_string_lossy(),
            "--work",
            &work.join("wk").to_string_lossy(),
            "--noanchor",
            "--inputs",
            "--trim-outside",
        ])
        .output()
        .expect("run fk regen");
    let log = String::from_utf8_lossy(&st.stdout).to_string();
    assert!(st.status.success(), "fk regen failed:\n{}", log);
    assert!(out.exists(), "no file written:\n{}", log);

    // the tape echo, which is what the round-vs-floor defect broke
    let (kappa, _pct, lag, n) = ghost::verify::tape_record_agreement(&out.to_string_lossy())
        .expect("the written file has both channels");
    assert!(n > 400, "only {} samples compared", n);
    assert_eq!(lag, 0, "the recording must line up with the tape at zero lag");
    assert!(
        kappa > 0.999,
        "the recording disagrees with the tape it carries: kappa {:.3} over {} samples \
         (0.467 was the round-vs-floor defect; 1.000 is a file that agrees with itself)",
        kappa,
        n
    );

    // and the plain oracle on the file as written
    let r = ghost::oracle::validate(
        &server,
        &out,
        ghost::oracle::MapsMode::One(&map2()),
        "fktest-regen",
    )
    .expect("the oracle ran");
    assert_eq!(
        r.time_ms,
        Some(22_730),
        "the written file must still run the time it claims (it said {})",
        r.secs()
    );
}

// =========================================================================
// PURE -- the carrier-byte fitter
//
// These are the checks on the instrument that named 25 of the 91 sample bytes
// a regenerated ghost used to inherit. Every one of them pins a way the fitter
// was observed to be wrong while it was being built, and every one of those
// ways looked like a result at the time.
// =========================================================================

use fk::carrier::{fit, score, Channel, Fit, Kind, Write};

/// The wheel-rotation encoding, synthesised and recovered.
///
/// The channel that matters most here is an ANGLE that wraps: a wheel turns
/// twice between two 50 ms samples at racing speed, so the recorded `u16` runs
/// through its whole range several times a second. A fitter that regresses on
/// raw target increments fits noise on it. This builds exactly that signal from
/// a known coefficient and requires the fit back, to the last sample.
#[test]
fn a_wrapping_channel_is_fitted_back_exactly() {
    let k = 40.743_043_733_253;
    let c = 0.0;
    // 4 rad per sample: past a full turn, so the u16 wraps every other step.
    let v: Vec<f64> = (0..400).map(|i| i as f64 * 4.0 + 0.137).collect();
    let t: Vec<u32> = v.iter().map(|x| ((k * x + c).floor() as i64).rem_euclid(65536) as u32).collect();
    let f = fit(&v, &t, 65536).expect("a fit");
    assert_eq!(f.exact, t.len(), "every sample must come back exact");
    // 1e-6, not 1e-9, and the looseness is the measurement: over a run this
    // long several coefficients reproduce every sample, so `k` is determined to
    // about a part in a million and no better. That is exactly the spread the
    // eight answer keys showed for the real wheel constant (40.743028 to
    // 40.743071), which is how we know the spread is the fitter's resolution
    // and not eight different engines.
    assert!(
        (f.k - k).abs() / k < 1e-6,
        "recovered k {} against {}",
        f.k,
        k
    );
}

/// **The grid offset is a real number and forcing it onto an integer costs
/// exactness.**
///
/// Measured while building this: rpm fitted 81.3 % with an integer `c` and
/// 92.7 % with a real one, on the same slot with the same `k` — the difference
/// between `floor` and `round` and every offset in between. The project has
/// been bitten from the other side too, by an input echo written with a `round`
/// where the game writes a `floor`, worth a Cohen's kappa of 0.467 against
/// 1.000. So the offset is fitted, and this is the fixture that says so.
#[test]
fn the_grid_offset_is_fitted_as_a_real_number() {
    let (k, c) = (0.5, 0.5);
    let v: Vec<f64> = (0..200).map(|i| i as f64).collect();
    let t: Vec<u32> = v.iter().map(|x| ((k * x + c).floor() as i64).rem_euclid(256) as u32).collect();
    let f = fit(&v, &t, 256).expect("a fit");
    assert_eq!(f.exact, t.len(), "the real offset reproduces every sample");
    // and the two integer offsets either side do not
    for ci in [0.0f64, 1.0] {
        let g = score(&v, &t, 256, f.k, ci);
        assert!(
            g.exact < t.len(),
            "an integer offset {} should not reproduce a half-step grid",
            ci
        );
    }
}

/// **An integer read as a float is a lookup table, not a law.**
///
/// Gear is stored as a small integer and recorded as `4 * gear + 1`. Read as an
/// `f32` that integer is a denormal of about 1e-45, and the affine fit returned
/// `k = 2.85e45` — a flawless 100 % on all eight answer keys, with a
/// coefficient that means nothing, transfers to nothing, and would have gone
/// into the table as the encoding of gear. The guard is a plausibility bound on
/// `k`; this is the input that motivated it.
#[test]
fn an_integer_read_as_a_float_is_refused_rather_than_fitted() {
    let gears: Vec<u32> = (0..300).map(|i| 1 + (i / 40) as u32 % 5).collect();
    let t: Vec<u32> = gears.iter().map(|g| 4 * g + 1).collect();
    // as the engine stores it: an integer
    let as_int: Vec<f64> = gears.iter().map(|g| *g as f64).collect();
    let good = fit(&as_int, &t, 256).expect("a fit on the integer");
    assert_eq!(good.exact, t.len(), "4*gear+1 is exactly fittable");
    assert!(good.k.abs() < 1e3, "and with a coefficient a human can read: {}", good.k);
    // as the sweep would see it if it read the same bytes as an f32
    let as_f32: Vec<f64> = gears.iter().map(|g| f32::from_bits(*g).into()).collect();
    match fit(&as_f32, &t, 256) {
        None => {}
        Some(f) => assert!(
            f.k.abs() <= 1e9,
            "a coefficient of {:e} is the fitter compensating for the wrong type, and must \
             not be returned",
            f.k
        ),
    }
}

/// **A permutation of the target destroys the fit, which is what makes the
/// permutation floor a floor.**
///
/// The sweep scores every channel twice — once against the recording and once
/// against a row-permuted copy of the same column — because at ~460 instants
/// the best of tens of thousands of candidates sits about four standard
/// deviations high for free. That number is only a floor if shuffling really
/// does destroy the relationship, and this is the check that it does.
#[test]
fn shuffling_the_target_destroys_the_fit() {
    let k = 40.743_043_733_253;
    let v: Vec<f64> = (0..400).map(|i| i as f64 * 4.0).collect();
    let t: Vec<u32> = v.iter().map(|x| ((k * x).floor() as i64).rem_euclid(65536) as u32).collect();
    let mut sh = t.clone();
    // a fixed, reproducible shuffle
    let mut s: u64 = 0x5eed;
    for i in (1..sh.len()).rev() {
        s ^= s << 13;
        s ^= s >> 7;
        s ^= s << 17;
        sh.swap(i, (s % (i as u64 + 1)) as usize);
    }
    let real = fit(&v, &t, 65536).expect("a fit").rate();
    let null = fit(&v, &sh, 65536).map(|f| f.rate()).unwrap_or(0.0);
    assert!(real > 0.99, "the real relationship: {:.3}", real);
    assert!(
        null < 0.05,
        "a shuffled target must not fit: {:.3} -- if it does, the permutation floor is not a \
         floor and every 'candidate' in a scan is unjudged",
        null
    );
}

/// The frozen table survives a round trip, including the write column.
///
/// The write column is the one that is easy to get wrong and expensive to get
/// wrong: it names the instant WITHIN a tick relative to the car, not an
/// absolute first-or-last, because which absolute write the recorder captured
/// moves between runs on the same binary. A table written with one meaning and
/// read with the other scores 100 % on five keys and 1 % on three.
#[test]
fn the_frozen_table_round_trips() {
    for (s, ch) in [("b91", Channel::Byte(91)), ("u16@6", Channel::U16(6))] {
        assert_eq!(Channel::parse(s), Some(ch));
        assert_eq!(ch.name(), s);
    }
    for (s, w) in [("car", Write::Car), ("other", Write::Other)] {
        assert_eq!(Write::parse(s), Some(w));
        assert_eq!(w.name(), s);
    }
    for (s, k) in [("raw", Kind::Raw), ("affine", Kind::Affine), ("affineu8", Kind::AffineU8)] {
        assert_eq!(Kind::parse(s), Some(k));
        assert_eq!(k.name(), s);
    }
    assert_eq!(Channel::U16(6).modulus(), 65536);
    assert_eq!(Channel::Byte(6).modulus(), 256);
    let sample: Vec<u8> = (0..116u8).collect();
    assert_eq!(Channel::Byte(6).value(&sample), Some(6));
    assert_eq!(Channel::U16(6).value(&sample), Some(6 | 7 << 8));
    let f = Fit { k: 1.0, c: 0.0, exact: 3, n: 4 };
    assert!((f.rate() - 0.75).abs() < 1e-12);
}

/// **The checked-in table is the result, so it is checked.**
///
/// `tools/fk/carrier-bytes.tsv` is what `fk carrier write` reads and what
/// `ghost regen` will stop inheriting 25 bytes because of. A typo in it is a
/// silently wrong file, so its shape is pinned here: every row parses, every
/// offset is inside the window the commands gather, and the four wheels really
/// are a block at stride 44 rather than four numbers that happen to be near
/// each other.
#[test]
fn the_checked_in_carrier_table_is_well_formed() {
    let p = Path::new(env!("CARGO_MANIFEST_DIR")).join("../carrier-bytes.tsv");
    let rows = fk::carrier::read_table(&p.to_string_lossy()).expect("the table parses");
    assert!(rows.len() >= 19, "only {} rows", rows.len());
    let mut by_channel = std::collections::BTreeMap::new();
    for r in &rows {
        assert!(
            (-1_000_000..1_000_000).contains(&r.rel),
            "{} sits at {} from the car, outside any window these commands gather",
            r.ch.name(),
            r.rel
        );
        assert!(r.k.is_finite() && r.k.abs() < 1e9, "{} has k {:e}", r.ch.name(), r.k);
        assert!(
            by_channel.insert(r.ch.name(), r.rel).is_none(),
            "{} appears twice; a channel has one encoding or none",
            r.ch.name()
        );
    }
    // the wheel block: four rotations at stride 44, and each wheel's suspension
    // travel four bytes ahead of its own rotation
    let rot: Vec<i64> = [6, 8, 10, 12]
        .iter()
        .map(|b| by_channel[&format!("u16@{}", b)])
        .collect();
    for w in rot.windows(2) {
        assert_eq!(w[1] - w[0], 44, "the wheel rotations are a block at stride 44: {:?}", rot);
    }
    for (b, r) in [23usize, 25, 27, 29].iter().zip(rot.iter()) {
        assert_eq!(
            by_channel[&format!("b{}", b)],
            r - 4,
            "byte {} is the suspension of the wheel whose rotation is at {}",
            b,
            r
        );
    }
    // ... and the rest of the wheel's own 44-byte record, at the slots the
    // class reference names: the ground material 12 bytes past the rotation and
    // `Icing01` 12 past that. The point of asserting the STRUCTURE rather than
    // the numbers is that a typo in one offset shows up as a broken stride
    // rather than as a byte that quietly reads its neighbour.
    for (b, r) in [24usize, 26, 28, 30].iter().zip(rot.iter()) {
        assert_eq!(by_channel[&format!("b{}", b)], r + 12, "byte {} is a ground material", b);
    }
    for (b, r) in [81usize, 82, 83, 84].iter().zip(rot.iter()) {
        assert_eq!(by_channel[&format!("b{}", b)], r + 24, "byte {} is an Icing01", b);
    }
}

/// **Every offset in the table is relative to ONE anchor, and the table says
/// which.**
///
/// Not a style rule. Another arm located the same `gear` slot on the same day
/// and reported it 408 bytes away, because its anchor was the locator's own
/// position address and this one is the copy with a live wheel block — two
/// correct measurements that read as a contradiction, for a day, because
/// neither table named its anchor. The `write` column is the same hazard at the
/// scale of one tick, and it is checked the same way.
#[test]
fn the_table_names_its_anchor_and_its_write() {
    let doc = std::fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../CARRIER.md"),
    )
    .expect("CARRIER.md");
    assert!(
        doc.contains("WHEEL-ROTATION SLOTS HOLD LIVE FLOATS"),
        "CARRIER.md must define what `car` is; an offset without its anchor is not a \
         measurement"
    );
    let p = Path::new(env!("CARGO_MANIFEST_DIR")).join("../carrier-bytes.tsv");
    let rows = fk::carrier::read_table(&p.to_string_lossy()).expect("the table parses");
    for r in &rows {
        assert_eq!(
            r.write,
            fk::carrier::Write::Car,
            "{} is read on the `{}` write; the table is written in the car's own frame",
            r.ch.name(),
            r.write.name()
        );
    }
}

/// **The carrier table, scored against the real engine on a checked-in
/// recording.**
///
/// This is the whole claim of `tools/fk/CARRIER.md` reduced to one assertion:
/// take the frozen table — offsets from the car, encodings, coefficients, all
/// fixed before this test ran — read the slots out of a live engine, and
/// require them to reproduce what the GAME wrote in `human_22730`'s own
/// telemetry.
///
/// The wheel rotations are the row to watch. They are the channel a viewer
/// sees, they are an angle that wraps twice between samples, and they were the
/// channel a previous attempt at this could not anchor. The bar is 99 %; the
/// eight keys measured 99.25–100 %.
///
/// It is not a tautology that this passes on the key it is run on. The bytes
/// come from engine memory at a frozen offset with a frozen coefficient; the
/// only thing the recording supplies is which copy of the car to read, and that
/// is a position match to a micron.
#[test]
fn engine_carrier_table_reproduces_a_real_recording() {
    let Some(server) = engine_tier() else { return };
    let work = scratch("carrier");
    let out = work.join("res.tsv");
    let table = Path::new(env!("CARGO_MANIFEST_DIR")).join("../carrier-bytes.tsv");
    let fk_bin = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../target/release/fk")
        .canonicalize()
        .expect("build the workspace first: cargo build --release");
    let st = std::process::Command::new(&fk_bin)
        .args([
            "carrier", "confirm",
            "--template", &human_ghost(),
            "--map", &map2().to_string_lossy(),
            "--table", &table.to_string_lossy(),
            "--out", &out.to_string_lossy(),
            "--dump", &work.join("d.bin").to_string_lossy(),
            "--shim", &shim().to_string_lossy(),
            "--server", &server.to_string_lossy(),
            "--work", &work.join("wk").to_string_lossy(),
        ])
        .output()
        .expect("run fk carrier confirm");
    let log = String::from_utf8_lossy(&st.stdout).to_string();
    assert!(st.status.success(), "fk carrier confirm failed:\n{}", log);

    let text = std::fs::read_to_string(&out).expect("a result table");
    let mut rate: std::collections::BTreeMap<String, f64> = Default::default();
    for l in text.lines().skip(1) {
        let f: Vec<&str> = l.split('\t').collect();
        if f.len() >= 11 {
            rate.insert(f[0].into(), f[8].parse().unwrap_or(0.0));
        }
    }
    assert!(rate.len() >= 14, "only {} channels scored:\n{}", rate.len(), log);
    for w in ["u16@6", "u16@8", "u16@10", "u16@12"] {
        assert!(
            rate[w] > 0.99,
            "{} reproduced {:.2}% of the game's own wheel rotations -- the eight keys \
             measured 99.25-100%, so anything below 99% means the anchor moved\n{}",
            w,
            100.0 * rate[w],
            log
        );
    }
    for d in ["b23", "b25", "b27", "b29"] {
        assert!(rate[d] > 0.99, "{} suspension travel: {:.2}%\n{}", d, 100.0 * rate[d], log);
    }
    assert!(rate["b91"] > 0.999, "gear is exact or it is wrong: {:.2}%", 100.0 * rate["b91"]);
    for (ch, bar) in [("u16@0", 0.99), ("u16@2", 0.99), ("u16@4", 0.96), ("b22", 0.92), ("b31", 0.91)] {
        assert!(rate[ch] > bar, "{} scored {:.2}%\n{}", ch, 100.0 * rate[ch], log);
    }
}

/// **Is the transform encoder the inverse of the transform reader?**
///
/// This is the whole of what is left of the orientation half of §6 in
/// CARRIER.md, reduced to a test that needs no engine. The quaternion written
/// from the live-wheel copy is EXACT against the one the game recorded —
/// 0.00000 rad, same instant, same sign — and the bytes it encodes to still
/// disagree with the recorded bytes on 453 of 455 samples. A correct value and
/// a wrong encoding of it leaves exactly one suspect.
///
/// So: take a real recording's own samples, read the transform out, write it
/// straight back, and require the bytes. Any sample that does not round-trip is
/// a defect in `gbx::recwrite`, and its size here is the size of the problem.
#[test]
fn the_transform_encoder_round_trips_a_real_recording() {
    let d = gbx::record::decode_ghost(&human_ghost()).expect("decode");
    let (mut exact, mut n) = (0usize, 0usize);
    let mut worst: Option<(usize, Vec<u8>, Vec<u8>)> = None;
    for s in d.raw_samples() {
        let (pos, quat, _speed, vel) = gbx::record::read_transform_pub(s, 47);
        let mut out = s.to_vec();
        gbx::recwrite::write_transform(
            &mut out,
            47,
            &gbx::recwrite::Xform {
                pos: [pos[0] as f32, pos[1] as f32, pos[2] as f32],
                quat,
                vel,
            },
        );
        n += 1;
        if out[47..69] == s[47..69] {
            exact += 1;
        } else if worst.is_none() {
            worst = Some((n - 1, s[47..69].to_vec(), out[47..69].to_vec()));
        }
    }
    assert!(n > 400, "only {} samples", n);
    // MEASURED: 453 of 455. The encoder IS the inverse of the reader, and the
    // two that are not are the degenerate case -- an identity rotation, where
    // the reader hands back an angle of zero and the writer's `sin(ang)` guard
    // takes the (0, 0) branch, so the heading and pitch words come back zeroed
    // rather than as whatever the game left in them.
    //
    // The result matters more than the two: it CLEARS the encoder. The
    // orientation half of CARRIER.md §6 -- a quaternion measured exact against
    // the game's own and still writing different bytes -- cannot be blamed on
    // the encoding step, which leaves the instant the value is read at. That is
    // a much smaller haystack, and it is why this test exists as a measurement
    // rather than as an aspiration.
    assert!(
        exact >= n - 2,
        "the encoder stopped being the inverse of the reader: {} of {} round-trip \
         (was 453 of 455). First disagreement at sample {:?}:\n  read  {:?}\n  wrote {:?}",
        exact,
        n,
        worst.as_ref().map(|w| w.0),
        worst.as_ref().map(|w| &w.1),
        worst.as_ref().map(|w| &w.2),
    );
    assert!(
        exact < n,
        "the identity-rotation samples now round-trip too -- someone fixed the (0, 0) \
         branch. Good: tighten this to equality and delete this assertion."
    );
}
