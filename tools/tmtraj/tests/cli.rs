//! The COMMAND LINE, over checked-in fixtures.
//!
//! ## Why this file exists
//!
//! Before it, the crate's five suites pinned the decoder's 29 CSV columns, the
//! path JSON and the racing-line maths, byte for byte, against the retired
//! Python — a genuinely strong control, and one that covered **five of about
//! sixty commands**. Everything else could be changed freely with a green
//! suite. I measured that rather than asserting it: moving the publish gate's
//! wheel-consistency acceptance bar from `w.share >= 0.15` to `w.share >= 0.95`
//! — which changes who is publishable — left all eight tests passing.
//!
//! The two controls in the other direction, run the same way, did fire:
//! flipping `is_ground_contact` from `d[89] & 0x01` to `& 0x02`, and
//! `turbo_time` from `/255` to `/254`, each failed `golden_decode`. So the
//! decoder is pinned and the tools around it were not.
//!
//! ## What these tests are
//!
//! They run the built binary and compare its stdout to a golden committed under
//! `tests/golden/`. That is a behaviour lock: it says "this output changed",
//! not "this output is right". Correctness lives in `golden_decode` (against a
//! second implementation), in `ghost selftest` (against the dedicated server)
//! and in the controls each check carries. What a lock buys is that a refactor
//! cannot change an answer silently, which is the only claim a refactor can
//! honestly make about itself.
//!
//! Regenerate deliberately: `TMTRAJ_BLESS=1 cargo test --release --test cli`.

mod common;

use std::path::PathBuf;
use std::process::Command;

fn bin() -> PathBuf {
    // The test binary lives in target/<profile>/deps; the tool is two up.
    let mut p = std::env::current_exe().expect("current exe");
    p.pop();
    if p.ends_with("deps") {
        p.pop();
    }
    p.join("tmtraj")
}

fn golden_dir() -> PathBuf {
    PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/golden"))
}

/// Run `tmtraj ARGS...` and compare stdout+exit code with `tests/golden/NAME`.
fn check(name: &str, args: &[&str]) {
    check_norm(name, args, &[])
}

/// `subs` normalises paths that vary per run (a temp directory) so the golden
/// is stable across machines.
fn check_norm(name: &str, args: &[&str], subs: &[(&str, &str)]) {
    let exe = bin();
    assert!(
        exe.is_file(),
        "{} not built. `cargo test` builds it as a dependency of this test; if you are \
         running the test binary by hand, run `cargo build --release` first.",
        exe.display()
    );
    let out = Command::new(&exe).args(args).output().expect("run tmtraj");
    let got = format!(
        "$ tmtraj {}\n[exit {}]\n{}{}",
        args.join(" "),
        out.status.code().unwrap_or(-1),
        String::from_utf8_lossy(&out.stdout),
        {
            let e = String::from_utf8_lossy(&out.stderr);
            if e.is_empty() { String::new() } else { format!("--- stderr ---\n{}", e) }
        }
    )
    // fixture paths are absolute and differ per checkout
    .replace(common::TESTDATA, "$TESTDATA");
    let got = subs.iter().fold(got, |acc, (from, to)| acc.replace(from, to));
    let g = golden_dir().join(name);
    if std::env::var("TMTRAJ_BLESS").is_ok() {
        std::fs::create_dir_all(golden_dir()).unwrap();
        std::fs::write(&g, &got).unwrap();
        return;
    }
    let want = std::fs::read_to_string(&g).unwrap_or_else(|e| {
        panic!("{}: {} -- run TMTRAJ_BLESS=1 cargo test --test cli", g.display(), e)
    });
    if got != want {
        // show the first differing line rather than two walls of text
        let first = got
            .lines()
            .zip(want.lines())
            .enumerate()
            .find(|(_, (a, b))| a != b)
            .map(|(i, (a, b))| format!("line {}:\n  now  {}\n  was  {}", i + 1, a, b))
            .unwrap_or_else(|| "output length differs".into());
        panic!("`tmtraj {}` changed:\n{}", args.join(" "), first);
    }
}

fn human() -> String {
    common::fixture("human_22730.Ghost.Gbx").to_string_lossy().to_string()
}
fn human2() -> String {
    common::fixture("human_23013.Ghost.Gbx").to_string_lossy().to_string()
}
fn poisoned() -> String {
    common::fixture("poisoned_searchtape.Ghost.Gbx").to_string_lossy().to_string()
}
fn replay() -> String {
    common::fixture("replay_kacky_7241.Replay.Gbx").to_string_lossy().to_string()
}

#[test]
fn show_prints_the_header_and_the_two_second_tell() {
    // The SAMPLE COUNT and the declared checkpoints are on the first two lines
    // deliberately: a synthesised tape carrying its template's telemetry shows
    // up there (a poisoned file had 281 samples and declared 14.018 where the
    // clean regeneration of the same run had 280 and 13.984).
    check("show_human", &["show", &human(), "--head", "3"]);
    check("show_replay", &["show", &replay(), "--head", "2"]);
}

#[test]
fn fields_is_the_documentation() {
    // `tmtraj fields` is what this project publishes about the format. A change
    // to it is a change to a claim, and this makes that change visible in a
    // diff rather than in a paragraph somebody wrote a month ago.
    check("fields", &["fields"]);
}

#[test]
fn check_grades_a_human_recording_as_publishable() {
    check("check_human", &["check", &human(), "--race", "22730"]);
}

#[test]
fn check_refuses_the_file_this_project_labelled_do_not_publish() {
    // The fixture is a search tape: its inputs are its own and its telemetry is
    // the template's. A gate that passes it is not a gate.
    check("check_poisoned", &["check", &poisoned(), "--race", "13984"]);
}

#[test]
fn diff_states_its_coverage_and_refuses_an_empty_comparison() {
    // Two recordings from different sessions share no sample instant, because
    // sample times are SESSION times. The old instrument emitted zero rows and
    // a stderr line every pipeline discarded, and ten such silences were
    // recorded as ten CLEAN verdicts. This must be UNMEASURED, exit 3.
    check("diff_shared", &["diff", &human(), &human2()]);
    check("diff_lag", &["diff", &human(), &human2(), "--lag"]);
    check("diff_bytes", &["diff", &human(), &human2(), "--bytes"]);
}

#[test]
fn diff_near_refuses_without_a_control() {
    // A verdict with no control is the thing that cost four clips.
    check("diff_near_no_control", &["diff", &human(), &human2(), "--near"]);
}

#[test]
fn spawn_compares_orientation_as_a_rotation() {
    // q and -q are the SAME rotation; a byte comparison condemns five perfectly
    // correct 199100 files. The test is |dot| ~= 1.
    check("spawn", &["spawn", &human2(), "--ref", &human()]);
}

#[test]
fn motion_reads_the_trajectory_not_the_flag() {
    check("motion", &["motion", &human(), "--race", "22730"]);
}

#[test]
fn wheels_answers_both_questions_separately() {
    // "is there a wheel radius" and "are the wheel bytes alive" are different
    // questions; conflating them produced a false refusal of Nadeo's own
    // recording.
    check("wheels", &["wheels", &human(), "--race", "22730"]);
}

#[test]
fn an_unknown_flag_is_an_error_and_not_a_shrug() {
    // `intg pair --kind X` used to parse and discard --kind; `intg dup` read
    // --server and --maps and dropped both. A flag that does nothing is
    // indistinguishable from a flag that failed.
    check("unknown_flag", &["show", &human(), "--nonsense", "1"]);
}

#[test]
fn a_bad_enumerated_value_names_the_alternatives() {
    // `--sort xyz` used to match `_ => Sort::Time`, so any typo silently meant
    // Time; `--metric xyz` reached `.expect("bad --metric")`, a panic where a
    // usage error belonged.
    check("bad_metric", &["lines", "report", "--metric", "xyz"]);
}

#[test]
fn every_command_in_the_usage_text_exists() {
    // The old --help listed 9 of 21 dispatched commands, and the README
    // documented four more that had been renamed. Both drifted because nothing
    // compared them.
    let out = Command::new(bin()).arg("--help").output().expect("run");
    let usage = String::from_utf8_lossy(&out.stdout).to_string();
    let mut named: Vec<&str> = Vec::new();
    for line in usage.lines() {
        let t = line.trim_start();
        if line.starts_with("  ") && !t.is_empty() && t.chars().next().unwrap().is_lowercase() {
            if let Some(w) = t.split_whitespace().next() {
                if w.chars().all(|c| c.is_ascii_lowercase()) && w.len() > 2 {
                    named.push(w);
                }
            }
        }
    }
    named.sort();
    named.dedup();
    assert!(named.len() >= 10, "the usage text names only {:?}", named);
    for c in &named {
        let out = Command::new(bin()).arg(c).output().expect("run");
        let code = out.status.code().unwrap_or(-1);
        let all = format!(
            "{}{}",
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        );
        assert!(
            !all.contains("unknown command"),
            "--help names `{}` and the dispatcher does not know it",
            c
        );
        assert!(code == 0 || code == 2 || code == 3, "`tmtraj {}` exited {}", c, code);
    }
}

/// The publish gate, end to end, at the tier this box can reach: no dedicated
/// server, no engine route, no manifest. Every one of those is reported as
/// UNMEASURED or n/a and NEVER folded into a pass — which is the property being
/// pinned here, because the failure mode a gate dies of is a missing input
/// quietly reading as a clean one.
#[test]
fn the_gate_reports_what_it_could_not_measure() {
    let dir = std::env::temp_dir().join(format!("tmtraj-gate-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let refs = dir.join("refs.tsv");
    std::fs::write(
        &refs,
        format!("2\thuman\t{}\n2\thuman\t{}\n", human(), human2()),
    )
    .unwrap();
    let r = refs.to_string_lossy().to_string();
    // the file this project itself labelled DO_NOT_PUBLISH
    check_norm(
        "gate_poisoned",
        &["gate", &poisoned(), "--race", "13984", "--refs", &r, "--mapid", "2"],
        &[(r.as_str(), "$TMP/refs.tsv")],
    );
    let _ = std::fs::remove_dir_all(&dir);
}

/// A gate whose refs file names a map it holds nothing for must say UNTESTED,
/// not PASS. "No human reference" is a statement about our corpus, never about
/// the file.
#[test]
fn no_reference_is_untested_and_not_clean() {
    let dir = std::env::temp_dir().join(format!("tmtraj-gate2-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let refs = dir.join("refs.tsv");
    std::fs::write(&refs, "999\thuman\t/nonexistent.Ghost.Gbx\n").unwrap();
    let r = refs.to_string_lossy().to_string();
    check_norm(
        "gate_no_refs",
        &["gate", &human(), "--race", "22730", "--refs", &r, "--mapid", "2"],
        &[(r.as_str(), "$TMP/refs.tsv")],
    );
    let _ = std::fs::remove_dir_all(&dir);
}
