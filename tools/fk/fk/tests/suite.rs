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
//! `tools/ghost/testdata/` already holds anonymised ghosts, a replay that
//! carries its own map, and a map. A second corpus of the same files in
//! `tools/fk/testdata/` would be one more thing to keep in step, so this suite
//! reads ghost's. What lives in `fk`'s own testdata is only what is `fk`'s:
//! captured dedicated-server output, which is the one input `fk` parses that
//! nothing else does.

use std::path::{Path, PathBuf};

// ---------------------------------------------------------------- fixtures

fn ghost_testdata() -> PathBuf {
    // tools/fk/fk/tests/ -> tools/ghost/testdata/
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../ghost/testdata")
        .canonicalize()
        .expect("tools/ghost/testdata must exist -- fk shares ghost's fixture corpus")
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
        .join("../../forkoracle/target/release/libfkshim.so")
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
    if let Ok(g) = ghost::tape::Tape::from_file(&p.to_string_lossy()) {
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

/// The 29-column CSV is the format `tmtraj decode --csv` writes, and the
/// analysis tools index it by name.
#[test]
fn trajectory_csv_has_the_columns_the_rest_of_the_project_reads() {
    assert_eq!(fk::traj::COLS.len(), 29);
    assert_eq!(fk::traj::COLS[0], "time_ms");
    for c in ["x", "y", "z", "vx", "vy", "vz", "qx", "qy", "qz", "qw", "steer", "gas", "brake"] {
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
}
