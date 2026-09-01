//! `ghost` presents two tiers: product verbs, and forensic probes under
//! `ghost debug`.
//!
//! The flat namespace had grown to 30 top-level verbs, and **13 of them
//! appeared nowhere in `ghost --help`** -- including `film` and `synth`, which
//! are real capabilities, alongside six probes built to answer one question
//! each during an investigation. A namespace where a third of the entries are
//! undiscoverable is not a namespace, and a probe sitting beside `verify`
//! reads like a promise the probe does not make.
//!
//! Nothing was removed. What this pins is the presentation: the probes are
//! reachable as `ghost debug <verb>`, still reachable under their old names
//! for one release with a notice, and the real verbs are all documented.

use std::path::PathBuf;
use std::process::Command;

/// Probes: forensic, no compatibility promise.
const PROBES: &[&str] = &[
    "codeccheck",
    "swap-samples",
    "car-first",
    "split-car",
    "set-u01",
    "strip-events",
    "trajdiff",
];

/// Verbs that ARE the tool. Every one of these must appear in `ghost --help`;
/// several did not, which is what this list exists to prevent recurring.
const PRODUCT: &[&str] = &[
    "inspect", "manifest", "chunks", "tape", "map", "trim", "splice", "declare", "identity",
    "header", "census", "phase", "record", "film", "regen", "roundtrip", "verify", "synth",
    "dump", "engine",
];

fn ghost() -> Option<PathBuf> {
    let mut p = std::env::current_exe().ok()?;
    p.pop();
    if p.ends_with("deps") {
        p.pop();
    }
    let b = p.join("ghost");
    b.exists().then_some(b)
}

fn run(bin: &PathBuf, args: &[&str]) -> (i32, String, String) {
    let o = Command::new(bin).args(args).output().expect("run ghost");
    (
        o.status.code().unwrap_or(-1),
        String::from_utf8_lossy(&o.stdout).to_string(),
        String::from_utf8_lossy(&o.stderr).to_string(),
    )
}

#[test]
fn every_product_verb_is_documented_in_help() {
    let Some(bin) = ghost() else { return };
    let (code, help, _) = run(&bin, &["--help"]);
    assert_eq!(code, 0, "ghost --help must exit 0");
    let missing: Vec<&str> = PRODUCT
        .iter()
        .filter(|v| !help.contains(&format!("ghost {v}")))
        .copied()
        .collect();
    assert!(
        missing.is_empty(),
        "product verbs absent from `ghost --help` -- undiscoverable: {:?}",
        missing
    );
}

#[test]
fn every_probe_is_documented_under_debug() {
    let Some(bin) = ghost() else { return };
    let (code, help, _) = run(&bin, &["debug", "--help"]);
    assert_eq!(code, 0, "ghost debug --help must exit 0");
    let missing: Vec<&str> = PROBES
        .iter()
        .filter(|v| !help.contains(&format!("ghost debug {v}")))
        .copied()
        .collect();
    assert!(missing.is_empty(), "probes absent from `ghost debug --help`: {:?}", missing);
}

#[test]
fn the_two_tiers_do_not_overlap() {
    let both: Vec<&str> = PRODUCT.iter().filter(|v| PROBES.contains(v)).copied().collect();
    assert!(both.is_empty(), "a verb cannot be both product and probe: {:?}", both);
}

#[test]
fn a_product_verb_is_refused_under_debug() {
    let Some(bin) = ghost() else { return };
    // `ghost debug verify` must not quietly work: the two tiers mean something
    // only if the boundary is enforced in both directions.
    let (code, _, err) = run(&bin, &["debug", "verify", "/nonexistent.Ghost.Gbx"]);
    assert_eq!(code, 2, "a product verb under `debug` is a usage error");
    assert!(
        err.contains("not a debug probe"),
        "the refusal should say why and name the right spelling, got: {err:?}"
    );
}

#[test]
fn the_old_probe_spelling_still_works_and_says_where_it_went() {
    let Some(bin) = ghost() else { return };
    // Deprecation, not removal: a script that used the old name keeps working
    // for this release, and its operator is told once, on stderr.
    let (_, _, err) = run(&bin, &["split-car"]);
    assert!(
        err.contains("ghost debug split-car"),
        "the old spelling must name its replacement, got: {err:?}"
    );
    // ...and the notice must NOT appear when the new spelling is used.
    let (_, _, err2) = run(&bin, &["debug", "split-car"]);
    assert!(
        !err2.contains("has moved"),
        "the new spelling must not warn, got: {err2:?}"
    );
}
