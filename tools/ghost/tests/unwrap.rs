//! `ghost unwrap`: the ghost node of a `.Replay.Gbx` as a standalone `.Ghost.Gbx`.
//!
//! The command carries its own control (the written body must be the replay's
//! ghost node byte-for-byte except the telemetry node index, and must read back
//! the same tape, result and declared times), so the test's job is to prove the
//! control RUNS on the project's replay fixture and that the output is a plain
//! ghost the rest of the toolchain reads: same race time, same uid, no embedded
//! map. Hermetic — no server, no engine, no network.

use std::path::PathBuf;
use std::process::Command;

fn ghost() -> Option<PathBuf> {
    let mut p = std::env::current_exe().ok()?;
    p.pop();
    if p.ends_with("deps") {
        p.pop();
    }
    let b = p.join("ghost");
    b.exists().then_some(b)
}

fn replay() -> Option<PathBuf> {
    let p = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../testdata/replay_kacky_7241.Replay.Gbx");
    p.exists().then_some(p)
}

fn tmp(case: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("ghost-unwrap-{}-{}", std::process::id(), case));
    let _ = std::fs::create_dir_all(&d);
    d
}

fn run(bin: &PathBuf, args: &[&str]) -> (bool, String) {
    let o = Command::new(bin).args(args).output().expect("run ghost");
    (o.status.success(), format!("{}{}", String::from_utf8_lossy(&o.stdout), String::from_utf8_lossy(&o.stderr)))
}

#[test]
fn a_replay_unwraps_to_a_plain_ghost_that_reads_the_same_run() {
    let (Some(ghost), Some(rep)) = (ghost(), replay()) else { return };
    let d = tmp("plain");
    let out = d.join("kacky.Ghost.Gbx");
    let (ok, log) = run(&ghost, &["unwrap", rep.to_str().unwrap(), out.to_str().unwrap()]);
    assert!(ok, "unwrap refused the fixture:\n{log}");
    assert!(log.contains("race 7.241"), "unwrap did not report the fixture's race time:\n{log}");

    let (ok, before) = run(&ghost, &["inspect", rep.to_str().unwrap()]);
    assert!(ok, "inspect on the replay failed:\n{before}");
    let (ok, after) = run(&ghost, &["inspect", out.to_str().unwrap()]);
    assert!(ok, "inspect on the unwrapped ghost failed:\n{after}");
    assert!(before.contains("EMBEDDED MAP  yes"), "the fixture is expected to carry its map:\n{before}");
    assert!(after.contains("EMBEDDED MAP  none"), "the unwrapped ghost still carries a map:\n{after}");
    for line in ["declared      7.241", "BMWE8nGL9v6ho1B9nmYt6ijf7p8", "1037 ticks"] {
        assert!(after.contains(line), "the unwrapped ghost does not read `{line}`:\n{after}");
    }

    // The two tapes are the same tape.
    let (ta, tb) = (d.join("a.gtape"), d.join("b.gtape"));
    let (ok, l1) = run(&ghost, &["tape", "extract", rep.to_str().unwrap(), "--out", ta.to_str().unwrap()]);
    assert!(ok, "tape extract (replay):\n{l1}");
    let (ok, l2) = run(&ghost, &["tape", "extract", out.to_str().unwrap(), "--out", tb.to_str().unwrap()]);
    assert!(ok, "tape extract (ghost):\n{l2}");
    // the .gtape text names its source file in a `#source` line; everything else must match
    let body = |p: &PathBuf| -> String { std::fs::read_to_string(p).unwrap().lines().filter(|l| !l.starts_with("#source")).collect::<Vec<_>>().join("\n") };
    assert!(body(&ta) == body(&tb), "the tapes differ");

    // A plain ghost is not a replay: unwrapping it again is refused, not faked.
    let again = d.join("again.Ghost.Gbx");
    let (ok, log) = run(&ghost, &["unwrap", out.to_str().unwrap(), again.to_str().unwrap()]);
    assert!(!ok, "unwrap accepted a plain ghost as a replay:\n{log}");
    assert!(log.contains("not a CGameCtnReplayRecord"), "unexpected refusal text:\n{log}");
    assert!(!again.exists(), "a refused unwrap must not write");
    let _ = std::fs::remove_dir_all(&d);
}
