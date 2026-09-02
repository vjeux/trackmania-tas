//! Containers that carry an embedded map must be writable, not just readable.
//!
//! A `.Replay.Gbx` — and any ghost saved with its map inside it — holds a whole
//! `.Map.Gbx` in its body: 771,380 of 781,044 bytes in this project's own
//! fixture. That is the majority of the file, and it shapes two things that
//! both went wrong this session:
//!
//!   1. **`synth` reported it as unparseable.** A bare "bytes still unnamed:
//!      774657" reads as "99% of this container is not understood", when synth
//!      simply does not parse maps and does not need to. A peer investigation
//!      lost hours to that reading on a file that was healthy.
//!   2. **Nothing ever wrote one.** The fixture existed and was READ by one
//!      tmtraj test. No test spliced it, split it, or round-tripped it — so the
//!      whole map-carrying class could have been broken by any change to the
//!      record writer and the suite would have stayed green. It nearly was:
//!      `splice_record` refused outright for containers whose record has no
//!      recognised enclosing chunk.
//!
//! These are cheap and hermetic — no server, no engine, no network.

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
    let p = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../testdata/replay_kacky_7241.Replay.Gbx");
    p.exists().then_some(p)
}

fn tmp(case: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("ghost-embedded-{}-{}", std::process::id(), case));
    let _ = std::fs::create_dir_all(&d);
    d
}

fn run(bin: &PathBuf, args: &[&str]) -> (bool, String) {
    let o = Command::new(bin).args(args).output().expect("run ghost");
    (
        o.status.success(),
        format!(
            "{}{}",
            String::from_utf8_lossy(&o.stdout),
            String::from_utf8_lossy(&o.stderr)
        ),
    )
}

#[test]
fn a_container_carrying_a_map_can_be_spliced() {
    let (Some(ghost), Some(rep)) = (ghost(), replay()) else { return };
    let dir = tmp("splice");
    let out = dir.join("split.Ghost.Gbx");

    // `split-car` rewrites the record and changes its length, which is exactly
    // the operation `splice_record` used to refuse when it could not find an
    // enclosing skippable chunk to fix up. Every repair path -- regen, film,
    // the probes -- goes through the same writer, so this one call stands in
    // for all of them.
    let (ok, text) = run(
        &ghost,
        &[
            "debug",
            "split-car",
            rep.to_str().unwrap(),
            out.to_str().unwrap(),
            "--at",
            "5000",
        ],
    );
    assert!(
        ok && out.exists(),
        "a map-carrying container must be writable, not just readable: {text}"
    );

    // And the result must still decode, with the split actually applied.
    let (ok, manifest) = run(&ghost, &["manifest", out.to_str().unwrap()]);
    let _ = std::fs::remove_dir_all(&dir);
    assert!(ok, "the spliced file no longer decodes: {manifest}");
    // The manifest is COMPACT json -- `"sample_bytes":116`, no space after the
    // colon. Matching on a pretty-printed spelling silently counts zero and
    // reports "the split did not take" for a split that took perfectly.
    let cars = manifest.matches("\"sample_bytes\":116").count();
    assert!(
        cars >= 2,
        "the split did not take: expected two car entities in the output, found {cars}"
    );
}

#[test]
fn synth_separates_the_embedded_map_from_what_it_could_not_explain() {
    let (Some(ghost), Some(rep)) = (ghost(), replay()) else { return };
    let dir = tmp("synth");
    let out = dir.join("rebuilt.Ghost.Gbx");
    let (ok, text) = run(
        &ghost,
        &["synth", rep.to_str().unwrap(), out.to_str().unwrap()],
    );
    let _ = std::fs::remove_dir_all(&dir);
    assert!(ok, "synth failed on a map-carrying container: {text}");

    // The point of the report, not the exact numbers: a reader must be able to
    // tell "this file contains a map" from "this file is not understood".
    assert!(
        text.contains("EMBEDDED MAP"),
        "synth must attribute the map rather than counting it as unnamed, so that a \
         healthy replay does not read as 99% unparseable. Got:\n{text}"
    );
    assert!(
        text.contains("Unexplained:"),
        "synth must report what is left after the map -- that is the number worth \
         reasoning about. Got:\n{text}"
    );

    // The residue must be SMALL. If this ever grows to the size of the file
    // again, the attribution has broken and the number is misleading once more.
    let unexplained: usize = text
        .split("Unexplained:")
        .nth(1)
        .and_then(|s| s.trim().split(|c: char| !c.is_ascii_digit()).next())
        .and_then(|s| s.parse().ok())
        .expect("an Unexplained count");
    assert!(
        unexplained < 50_000,
        "synth cannot explain {unexplained} bytes of this container -- the embedded-map \
         attribution has stopped working, or the format changed"
    );
}

#[test]
fn a_plain_ghost_reports_no_embedded_map() {
    let Some(ghost) = ghost() else { return };
    let plain = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../testdata/decoder-goldens/ghosts/p00001_19538.Ghost.Gbx");
    if !plain.exists() {
        return;
    }
    let dir = tmp("plain");
    let out = dir.join("rebuilt.Ghost.Gbx");
    let (ok, text) = run(
        &ghost,
        &["synth", plain.to_str().unwrap(), out.to_str().unwrap()],
    );
    let _ = std::fs::remove_dir_all(&dir);
    assert!(ok, "synth failed on a plain ghost: {text}");
    // The negative half: a file with no map must print the plain count, with
    // no attribution line invented for it.
    assert!(
        !text.contains("EMBEDDED MAP"),
        "a ghost with no embedded map must not claim one:\n{text}"
    );
}
