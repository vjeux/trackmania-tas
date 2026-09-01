//! Every gate gets a POSITIVE control it passes and a NEGATIVE control it must
//! refuse.
//!
//! Before this there were none. `ghost` is 13,581 lines, produces every
//! publishable artifact, and had five unit tests; not one of them had ever
//! seen a gate say FAIL. A gate whose red state has never been observed is an
//! assumption, and this session already found two that were not doing what
//! their name claimed:
//!
//!   * the **dead-channel** gate had never fired in a test;
//!   * **V6** passed telemetry swapped in from a different run (measured:
//!     0.58–0.81 kappa against a 0.60 threshold, so three forgeries in four
//!     cleared it) while reporting a bare "PASS".
//!
//! The negative controls here are built from real corpus files by real tools,
//! so they cannot drift away from what the gates actually see.
//!
//! Hermetic: no server, no engine, no network. Gates that need one report NA,
//! and NA is neither a pass nor a failure.

use std::path::PathBuf;
use std::process::Command;

fn bin(name: &str) -> Option<PathBuf> {
    let mut p = std::env::current_exe().ok()?;
    p.pop();
    if p.ends_with("deps") {
        p.pop();
    }
    let b = p.join(name);
    b.exists().then_some(b)
}

fn testdata() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../testdata")
}

fn corpus(name: &str) -> Option<PathBuf> {
    let p = testdata().join("decoder-goldens/ghosts").join(format!("{name}.Ghost.Gbx"));
    p.exists().then_some(p)
}

fn tmp(case: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("ghost-gates-{}-{}", std::process::id(), case));
    let _ = std::fs::create_dir_all(&d);
    d
}

/// `(verdict, message)` for one gate id, from a `ghost verify` run.
fn gate(ghost: &PathBuf, file: &PathBuf, id: &str) -> Option<(String, String)> {
    let out = Command::new(ghost)
        .arg("verify")
        .arg(file)
        .arg("-o")
        .arg("json")
        .output()
        .ok()?;
    let text = String::from_utf8_lossy(&out.stdout).to_string();
    // The report is flat; find the object carrying this id.
    let needle = format!("\"id\": \"{id}\"");
    let at = text.find(&needle)?;
    let rest = &text[at..];
    let end = rest.find('}')?;
    let obj = &rest[..end];
    let verdict = obj.split("\"verdict\": \"").nth(1)?.split('"').next()?.to_string();
    let message = obj.split("\"message\": \"").nth(1)?.split('"').next()?.to_string();
    Some((verdict, message))
}

#[test]
fn v6_passes_a_clean_recording_and_does_not_call_foreign_telemetry_a_pass() {
    let (Some(ghost), Some(good), Some(donor)) =
        (bin("ghost"), corpus("p00001_19538"), corpus("p00043_19581"))
    else {
        return;
    };

    // POSITIVE control: the file's own telemetry.
    let (v, m) = gate(&ghost, &good, "V6").expect("V6 on a clean file");
    assert_eq!(v, "pass", "a clean recording must pass V6, got {v}: {m}");

    // NEGATIVE control: another run's telemetry, on the same map, in this
    // file's container. `swap-samples` changes nothing else -- so if V6 still
    // says "pass", the gate is not doing the job its name claims.
    let dir = tmp("v6");
    let forged = dir.join("foreign-telemetry.Ghost.Gbx");
    let ok = Command::new(&ghost)
        .args(["debug", "swap-samples"])
        .arg(&good)
        .arg(&forged)
        .arg("--donor")
        .arg(&donor)
        .status()
        .map(|s| s.success())
        .unwrap_or(false);
    assert!(ok, "could not build the negative control");

    let (v, m) = gate(&ghost, &forged, "V6").expect("V6 on the forged file");
    let _ = std::fs::remove_dir_all(&dir);
    assert_ne!(
        v, "pass",
        "V6 called FOREIGN telemetry a pass -- the gate exists to catch exactly this: {m}"
    );
    // It is allowed to be `warn`: the metric provably cannot separate a swap
    // from a clean recording at the top of its range (this project's own
    // poisoned search tapes score 0.83, as does a human run), so `warn` is the
    // honest verdict and V9 is what settles it. What is NOT allowed is `pass`.
    assert!(
        v == "warn" || v == "fail",
        "expected warn or fail from V6 on foreign telemetry, got {v}: {m}"
    );
}

#[test]
fn v6_fails_outright_on_the_poisoned_fixture() {
    let Some(ghost) = bin("ghost") else { return };
    let poisoned = testdata().join("poisoned_searchtape.Ghost.Gbx");
    if !poisoned.exists() {
        return;
    }
    // The tree ships this fixture precisely because it is known-bad. A gross
    // mismatch must be a FAIL, not a warn: the band is for cases the metric
    // cannot resolve, and this is not one of them.
    let (v, m) = gate(&ghost, &poisoned, "V6").expect("V6 on the poisoned fixture");
    assert_eq!(v, "fail", "the poisoned fixture must FAIL V6, got {v}: {m}");
}

#[test]
fn v1_is_a_round_trip_check_and_cannot_refuse_a_damaged_file() {
    let (Some(ghost), Some(good)) = (bin("ghost"), corpus("p00001_19538")) else {
        return;
    };
    let (v, m) = gate(&ghost, &good, "V1").expect("V1 on a clean file");
    assert_eq!(v, "pass", "a real file must pass codec identity, got {v}: {m}");

    // THIS GATE HAS NO NEGATIVE CONTROL, and that is the finding rather than a
    // gap in the test.
    //
    // "codec identity" reads like an integrity guarantee. It is not. It asks
    // whether a verbatim RE-ENCODE of the ticks reproduces the bitstream they
    // were decoded from -- a round trip against the file itself, with no
    // external truth anywhere in it. A damaged tape that still decodes will
    // re-encode to the same damaged bytes and pass.
    //
    // Measured: a bit flipped at every tenth of the file, nine positions
    // spanning header, tape and record, and V1 said `pass` at all nine. The
    // tree's own known-bad `poisoned_searchtape.Ghost.Gbx` also passes V1 (it
    // is V6 that catches it).
    //
    // So this test pins the true reach of the gate: V1 catches a tape whose
    // ENCODING our encoder would not have produced -- a real thing, since the
    // game and this toolchain can make different encoding choices for the same
    // inputs -- and it catches nothing about whether the inputs are right.
    // Anyone reading "PASS V1" as "this file is intact" is reading too much.
    let dir = tmp("v1");
    let bytes = std::fs::read(&good).expect("read fixture");
    let mut still_passing = 0;
    for pct in [10usize, 30, 50, 70, 90] {
        let mut broken = bytes.clone();
        broken[bytes.len() * pct / 100] ^= 0x01;
        let path = dir.join(format!("flip{pct}.Ghost.Gbx"));
        std::fs::write(&path, &broken).expect("write");
        if let Some((v, _)) = gate(&ghost, &path, "V1") {
            if v == "pass" {
                still_passing += 1;
            }
        }
    }
    let _ = std::fs::remove_dir_all(&dir);
    assert_eq!(
        still_passing, 5,
        "V1's reach has CHANGED -- it now refuses a damaged file it used to accept. \
         That is an improvement, not a bug: update this test and the comment above, \
         which record that V1 is a round trip with no external truth in it."
    );
}

#[test]
fn the_absence_gates_do_not_carry_a_verdict_on_their_own() {
    let Some(ghost) = bin("ghost") else { return };
    // 67 bytes of "GBX" + 0xFF used to verify OK: every gate that examines a
    // RUN reported NA, and three absence checks ("no account id in the body",
    // "no embedded map", the raw-bytes backstop) passed, because a file with
    // no body contains nothing bad. The whole report came back clean.
    let dir = tmp("absence");
    let junk = dir.join("junk.Ghost.Gbx");
    let mut bytes = b"GBX".to_vec();
    bytes.extend(std::iter::repeat(0xFF).take(64));
    std::fs::write(&junk, &bytes).expect("write");

    let out = Command::new(&ghost).arg("verify").arg(&junk).output().expect("run");
    let code = out.status.code().unwrap_or(-1);
    let _ = std::fs::remove_dir_all(&dir);
    assert_eq!(
        code, 1,
        "garbage must be REFUSED (exit 1). A verifier that passes 67 bytes of \
         junk is worse than one that panics on it."
    );
}
