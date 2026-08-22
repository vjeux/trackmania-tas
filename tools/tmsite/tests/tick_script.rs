//! The TICK exporter: byte-identity against committed goldens, exact
//! export -> verify round trips, the respawn input, and the grammar.

mod common;

use common::*;
use gbx::container::{write_gbx, Container};
use gbx::tape::{Encoding, Tape};
use tmsite::tick::{self, Opts};

const PLAIN: &str = "testdata/human_22730.Ghost.Gbx";
const RESPAWN: &str = "testdata/respawn_m2_id_rank10000_23286.Ghost.Gbx";
/// A Trial map: long, and full of respawns. Checked in with the repo.
const TRIAL: &str = "238835-turtle-trial-angustus/replays/AUTHORCUT_246602_watchable.Ghost.Gbx";

fn opts(path: String) -> Opts {
    Opts { path, archive: 0, raw: false, seed: None }
}

// -------------------------------------------------------------- byte identity

#[test]
fn tick_export_of_a_human_ghost_is_byte_identical_to_the_golden() {
    let r = tmsite(&["tick", "--ghost", &fixture_str(PLAIN)]);
    r.ok("tmsite tick");
    assert_golden("testdata/golden/human_22730.tick", r.stdout.as_bytes());
}

#[test]
fn tick_export_of_a_respawn_ghost_is_byte_identical_to_the_golden() {
    let r = tmsite(&["tick", "--ghost", &fixture_str(RESPAWN)]);
    r.ok("tmsite tick");
    assert_golden("testdata/golden/respawn.tick", r.stdout.as_bytes());
}

#[test]
fn the_script_header_reports_times_in_seconds() {
    let r = tmsite(&["tick", "--ghost", &fixture_str(PLAIN)]);
    r.ok("tmsite tick");
    let header = r.stdout.lines().nth(1).unwrap_or_default().to_string();
    assert!(
        header.contains("start offset -1.580 s") && header.contains("declared 22.730 s"),
        "header should carry seconds with a decimal, got: {:?}",
        header
    );
    assert!(
        !header.contains(" ms"),
        "a raw-millisecond time survived in the script header: {:?}",
        header
    );
}

// ------------------------------------------------------------- the round trip

#[test]
fn export_then_verify_is_exact_on_every_ghost_fixture() {
    for g in [fixture_str(PLAIN), fixture_str(RESPAWN), repo_fixture_str(TRIAL)] {
        let o = opts(g.clone());
        let e = tick::export(&o).unwrap_or_else(|x| panic!("export {}: {}", g, x));
        let d = tick::verify(&o, &e.text).unwrap_or_else(|x| panic!("verify {}: {}", g, x));
        assert_eq!(d.ticks, e.ticks);
        assert!(
            d.is_exact(),
            "{}: steer {:?} accel {:?} brake {:?} respawn {:?} srespawn {:?}",
            g,
            &d.steer_bad[..d.steer_bad.len().min(8)],
            &d.accel_bad[..d.accel_bad.len().min(8)],
            &d.brake_bad[..d.brake_bad.len().min(8)],
            &d.respawn_bad[..d.respawn_bad.len().min(8)],
            &d.srespawn_bad[..d.srespawn_bad.len().min(8)]
        );
    }
}

#[test]
fn verify_reads_the_script_from_disk_and_agrees_with_the_fresh_export() {
    let d = scratch("verify-file");
    let f = d.join("s.tick");
    tmsite(&["tick", "--ghost", &fixture_str(RESPAWN), "--out", f.to_str().unwrap()])
        .ok("tmsite tick --out");
    let r = tmsite(&[
        "verify",
        "--ghost",
        &fixture_str(RESPAWN),
        "--script",
        f.to_str().unwrap(),
    ]);
    r.ok("tmsite verify --script");
    assert!(r.stdout.contains("EXACT MATCH"), "{}", r.stdout);
    assert!(r.stdout.contains("respawn mismatch  0"), "{}", r.stdout);
}

// ------------------------------------------------------------------- respawns

#[test]
fn the_respawn_fixture_really_contains_respawns() {
    let e = tick::export(&opts(fixture_str(RESPAWN))).unwrap();
    // Exact counts: a decoder regression that loses or invents a respawn shows
    // up here rather than being absorbed by an "is not empty" check.
    assert_eq!(e.respawns, vec![2, 6, 20, 35, 55, 56]);
    assert_eq!(e.standing_respawns, vec![2, 6, 18, 55, 56]);
    let e = tick::export(&opts(repo_fixture_str(TRIAL))).unwrap();
    assert_eq!(e.respawns, vec![15643]);
    assert_eq!(e.standing_respawns, vec![11299, 21226]);
}

#[test]
fn respawn_lines_are_emitted_at_the_right_millisecond_and_before_that_tick_s_other_inputs() {
    let e = tick::export(&opts(fixture_str(RESPAWN))).unwrap();
    let lines: Vec<&str> = e.text.lines().collect();
    for &t in &e.respawns {
        let want = format!("{} respawn", t * 10);
        assert!(lines.contains(&want.as_str()), "no {:?} in the script", want);
    }
    for &t in &e.standing_respawns {
        let want = format!("{} srespawn", t * 10);
        assert!(lines.contains(&want.as_str()), "no {:?} in the script", want);
    }
    // Within a tick the respawn is written before the steer/accel/brake that
    // apply to the car it puts back on the ground. Tick 56 (560 ms) carries
    // both a respawn and an accel change.
    let i = lines.iter().position(|l| *l == "560 respawn").unwrap();
    let j = lines.iter().position(|l| l.starts_with("560 accel")).unwrap();
    assert!(i < j, "respawn must precede the same tick`s other inputs");
}

/// THE REGRESSION THIS EXISTS FOR: the exporter used to drop respawns with a
/// warning. A script without them must now FAIL verification -- otherwise
/// "export -> verify is exact" would still pass on a script that cannot
/// reproduce the run.
#[test]
fn a_script_with_the_respawn_lines_stripped_fails_verification() {
    let o = opts(fixture_str(RESPAWN));
    let full = tick::export(&o).unwrap();
    let stripped: Vec<&str> = full
        .text
        .lines()
        .filter(|l| !l.ends_with(" respawn") && !l.ends_with(" srespawn"))
        .collect();
    assert!(
        stripped.len() < full.text.lines().count(),
        "nothing was stripped -- this test is not testing anything"
    );
    let d = tick::verify(&o, &stripped.join("\n")).unwrap();
    assert_eq!(d.respawn_bad, full.respawns);
    assert_eq!(d.srespawn_bad, full.standing_respawns);
    assert!(!d.is_exact());
}

#[test]
fn a_respawn_on_a_tick_the_ghost_did_not_respawn_on_also_fails() {
    // The check has to bite in both directions, or it is only counting lines.
    let o = opts(fixture_str(RESPAWN));
    let full = tick::export(&o).unwrap();
    let text = format!("{}\n1000 respawn", full.text);
    let d = tick::verify(&o, &text).unwrap();
    assert_eq!(d.respawn_bad, vec![100]);
}

// ----------------------------------------------------------- the steer clamp

/// The ghost's steer byte is signed, so `0x80` decodes to -128 -- one below
/// what TICK's parser accepts. No ghost in the corpus contains one (measured
/// over 233 files), so the only way to test the clamp is to build a ghost that
/// does, which the shared `gbx` codec can do exactly.
fn ghost_with_steer_0x80(out: &std::path::Path, tick_index: usize) {
    let c = Container::load(&fixture_str(PLAIN)).unwrap();
    let mut tape = Tape::from_body(c.body()).unwrap();
    tape.verbatim_is_identity()
        .expect("the untouched tape must re-encode byte for byte before we edit it");
    {
        let a = &mut tape.archives[0];
        assert_eq!(a.packets[tick_index].mode, 2, "fixture tick is not an 8-bit-steer packet");
        a.packets[tick_index].steer = 0x80;
        a.packets[tick_index].vsame = false;
    }
    let body = tape.splice_into(c.body(), Encoding::Verbatim).unwrap();
    write_gbx(&c.gbx, body, out.to_str().unwrap()).unwrap();
}

#[test]
fn steer_minus_128_is_clamped_to_left_and_reported() {
    let d = scratch("steer-128");
    let g = d.join("m128.Ghost.Gbx");
    ghost_with_steer_0x80(&g, 500);
    let gs = g.to_str().unwrap();

    let r = tmsite(&["tick", "--ghost", gs]);
    r.ok("tmsite tick");
    assert!(
        r.stderr.contains("1 tick(s) hold steer -128") && r.stderr.contains("clamped to -127"),
        "the clamp must be reported, got: {:?}",
        r.stderr
    );
    assert!(r.stdout.lines().any(|l| l == "5000 steer left"), "expected a clamped `left` at 5000 ms");
    assert!(!r.stdout.contains("-128"), "a -128 reached the script");
    // clamped export still round-trips, because verify clamps the same way
    tmsite(&["verify", "--ghost", gs]).ok("verify (clamped)");

    // --raw emits the -128 verbatim, and then the grammar check bites: TICK
    // would reject the script, so verify must too rather than pass it on.
    let raw = tmsite(&["tick", "--ghost", gs, "--raw"]);
    raw.ok("tmsite tick --raw");
    assert!(raw.stdout.lines().any(|l| l == "5000 steer -128"));
    let v = tmsite(&["verify", "--ghost", gs, "--raw"]);
    v.failed("verify --raw on a -128 script");
    assert!(
        v.stderr.contains("outside TICK's -127..127"),
        "verify should name the grammar violation, got: {:?}",
        v.stderr
    );
}

// ------------------------------------------------------------------ the seed

#[test]
fn seed_is_emitted_only_when_asked_for() {
    let plain = tmsite(&["tick", "--ghost", &fixture_str(PLAIN)]);
    plain.ok("tick");
    assert!(!plain.stdout.contains(" seed "));
    let seeded = tmsite(&["tick", "--ghost", &fixture_str(PLAIN), "--seed", "4242"]);
    seeded.ok("tick --seed");
    assert_eq!(
        seeded.stdout.lines().nth(2),
        Some("0 seed 4242"),
        "the seed line goes after the two comment lines"
    );
    // and it does not disturb the round trip
    tmsite(&["verify", "--ghost", &fixture_str(PLAIN)]).ok("verify");
}

// --------------------------------------------------------------- the grammar

#[test]
fn replay_holds_values_between_changes() {
    let s = "# c\n0 accel 1\n0 steer -5\n30 steer right\n50 accel 0\n";
    let r = tick::replay(s, 6).unwrap();
    assert_eq!(r.accel, vec![1, 1, 1, 1, 1, 0]);
    assert_eq!(r.steer, vec![-5, -5, -5, 127, 127, 127]);
    assert_eq!(r.brake, vec![0; 6]);
    assert_eq!(r.respawn, vec![false; 6]);
}

#[test]
fn respawn_is_an_event_not_a_held_value() {
    let r = tick::replay("0 respawn\n20 srespawn\n", 4).unwrap();
    assert_eq!(r.respawn, vec![true, false, false, false]);
    assert_eq!(r.srespawn, vec![false, false, true, false]);
}

#[test]
fn replay_enforces_ticks_range_and_known_actions() {
    let bad = [
        ("5 accel 1", "off the 10 ms grid"),
        ("0 steer -128", "steer below -127"),
        ("0 steer 128", "steer above 127"),
        ("0 gamespeed 5.0", "unknown action"),
        ("0 accel maybe", "unknown accel value"),
        ("0 respawn 1", "respawn takes no argument"),
        ("50 accel 1", "an action past the end of the ghost"),
        ("0", "an incomplete line"),
    ];
    for (s, why) in bad {
        assert!(
            tick::replay(s, 2).is_err(),
            "replay accepted {:?} ({})",
            s,
            why
        );
    }
    assert!(tick::replay("# just a comment\n\n0 accel 1 # trailing\n", 2).is_ok());
    // seed and flags are legal TICK lines with no per-tick input to compare
    assert!(tick::replay("0 seed 7\n0 flags 3\n", 2).is_ok());
}

#[test]
fn last_line_wins_within_a_tick() {
    let r = tick::replay("0 steer 5\n0 steer -5\n", 1).unwrap();
    assert_eq!(r.steer, vec![-5]);
}

#[test]
fn the_steer_byte_is_signed() {
    assert_eq!(tick::sgn(0x81), -127);
    assert_eq!(tick::sgn(0x7F), 127);
    assert_eq!(tick::sgn(0x80), -128);
    assert_eq!(tick::sgn(0), 0);
}

#[test]
fn seconds_formatting() {
    assert_eq!(tick::secs(36049), "36.049");
    assert_eq!(tick::secs(0), "0.000");
    assert_eq!(tick::secs(-1510), "-1.510");
    assert_eq!(tick::secs(2501894), "2501.894");
}

// ------------------------------------------------------------------- errors

#[test]
fn a_file_without_an_input_chunk_is_an_error_that_names_the_file() {
    // Build one: take the ghost fixture and blank the input chunk's id, so the
    // file is a valid GBX with nothing to export. (Fixtures under `tools/` are
    // not referenced here on purpose -- that tree moves.)
    let d = scratch("no-input-chunk");
    let out = d.join("notaghost.Ghost.Gbx");
    let c = Container::load(&fixture_str(PLAIN)).unwrap();
    let (chunk_off, _, _) = gbx::tape::find_inputs_chunk(c.body()).unwrap();
    let mut body = c.body().to_vec();
    body[chunk_off..chunk_off + 4].copy_from_slice(&0xDEAD_BEEFu32.to_le_bytes());
    write_gbx(&c.gbx, body, out.to_str().unwrap()).unwrap();

    let r = tmsite(&["tick", "--ghost", out.to_str().unwrap()]);
    r.failed("tick on a file with no input chunk");
    assert!(r.stderr.contains("no 0x0309201D input chunk"), "{}", r.stderr);
    assert!(
        r.stderr.contains("notaghost.Ghost.Gbx"),
        "the error must name the file: {}",
        r.stderr
    );
}

#[test]
fn asking_for_an_archive_that_is_not_there_is_an_error() {
    let r = tmsite(&["tick", "--ghost", &fixture_str(PLAIN), "--archive", "7"]);
    r.failed("tick --archive 7");
    assert!(r.stderr.contains("no archive 7"), "{}", r.stderr);
}
