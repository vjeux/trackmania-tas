//! The integrity gate's own oracle tests: what the gate believes the dedicated
//! server said.
//!
//! This file exists because the gate reported **four validated checkpoints for
//! a run the server refused**. `intgcmd::parse_oracle` read the checkpoint
//! count with a helper that finds `"ValidatedResult"` and scans FORWARD for the
//! key; on a DNF the object is `null` and carries no `NbCheckpoints`, so the
//! scan ran into `DeclaredResult` and returned the file's own claim. The time
//! was parsed correctly, which is exactly why nothing caught it: every summary
//! line the gate printed looked sane.
//!
//! Two design points, and the second is the one that makes this a test at all.
//!
//! **The transcript is the server's own bytes** (`testdata/oracle_transcript.json`),
//! captured from a run rather than typed: the real format puts spaces around
//! its colons, ends `Desc` with a `\n`, and moves `Desc` between the two rows.
//!
//! **Both rows are asymmetric on purpose.** On an ordinary passing file
//! `ValidatedResult` and `DeclaredResult` carry the SAME numbers, so a fixture
//! built from one cannot fail: a right parser and a wrong one are
//! indistinguishable on it. Row 1 is a DNF whose declaration still claims
//! 19.538 and four checkpoints; row 2 finishes at 19.538 while declaring 30.000
//! and five.
//!
//! And then the part that keeps the fixture honest: the suite runs the **WRONG**
//! parser -- the deleted one, transcribed verbatim -- on the same bytes and
//! REQUIRES it to produce the wrong answer. Three assertions saying the parser
//! is right do not say the transcript could ever have caught it being wrong. If
//! `wrong_parser_still_gets_it_wrong` ever fails, the fixture has stopped being
//! a test and nothing else in this file means anything.

mod common;

use ghost::oracle::{parse_many, SimResult};

fn rows() -> Vec<SimResult> {
    let text = std::fs::read_to_string(common::fixture("oracle_transcript.json"))
        .expect("the captured server transcript");
    parse_many(&text)
}

fn row(name: &str) -> SimResult {
    rows().into_iter().find(|r| r.file == name).unwrap_or_else(|| {
        panic!("no row for {} in the transcript", name);
    })
}

#[test]
fn both_rows_are_attributed_to_their_files() {
    let r = rows();
    assert_eq!(r.len(), 2, "rows parsed: {:?}", r.iter().map(|x| &x.file).collect::<Vec<_>>());
}

/// THE BUG, pinned. A DNF has no validated checkpoint count -- and the number
/// sitting next to it in the file is not one.
#[test]
fn a_dnf_reports_no_validated_checkpoints() {
    let d = row("edited.Ghost.Gbx");
    assert_eq!(d.time_ms, None, "the server validated nothing");
    assert_eq!(
        d.cps, None,
        "a run the server refused reached no VALIDATED checkpoints; {:?} would be the file's own \
         declaration read as the world's answer",
        d.cps
    );
    assert_eq!(d.declared_cps, Some(4), "the file's own claim, kept as its own field");
    assert_eq!(d.declared_ms, Some(19538));
    assert!(d.desc.contains("wrong simu"), "desc {:?}", d.desc);
    assert_eq!(d.is_valid, Some(false));
}

/// The other asymmetry: when the server DID simulate, its numbers win over the
/// file's.
#[test]
fn the_validated_result_wins_over_the_declaration() {
    let f = row("stale_decl.Ghost.Gbx");
    assert_eq!(f.time_ms, Some(19538), "what the server simulated");
    assert_eq!(f.cps, Some(4));
    assert_eq!(f.declared_ms, Some(30000), "what the file claims");
    assert_eq!(f.declared_cps, Some(5));
    assert!(!f.declaration_holds(), "19.538 simulated against 30.000 declared");
}

/// THE FIXTURE'S OWN POSITIVE CONTROL.
///
/// `grab_i` is the deleted parser, transcribed from `intgcmd.rs` as it stood:
/// find the anchor, scan forward, take the first integer after the key. Run it
/// on the same bytes and require the wrong answers -- a checkpoint count of 4
/// for the DNF (`DeclaredResult`'s, four objects further on) and a time for a
/// run that never finished if you drop the `null` test the old code did get
/// right.
#[test]
fn wrong_parser_still_gets_it_wrong() {
    let text = std::fs::read_to_string(common::fixture("oracle_transcript.json")).unwrap();
    let grab_i = |k: &str, after: &str| -> Option<i64> {
        let at = text.find(after)?;
        let seg = &text[at..];
        let p = seg.find(&format!("\"{}\" : ", k))?;
        let rest = &seg[p + k.len() + 5..];
        let num: String = rest
            .chars()
            .skip_while(|c| !c.is_ascii_digit() && *c != '-')
            .take_while(|c| c.is_ascii_digit() || *c == '-')
            .collect();
        num.parse().ok()
    };

    let wrong_cps = grab_i("NbCheckpoints", "\"ValidatedResult\"");
    assert_eq!(
        wrong_cps,
        Some(4),
        "the scan-forward parser must still read 4 checkpoints out of a null ValidatedResult; if \
         it does not, this transcript no longer exercises the bug"
    );
    let wrong_time = grab_i("Time", "\"ValidatedResult\"");
    assert_eq!(
        wrong_time,
        Some(19538),
        "and it must still find a TIME inside a null result -- the file's declaration"
    );

    // The two parsers must DISAGREE on the same bytes. This is the assertion
    // that fails first if anyone ever "simplifies" the fixture into a passing
    // file, where the declared and validated numbers are equal and every
    // parser looks correct.
    let right = row("edited.Ghost.Gbx");
    assert_ne!(
        right.cps.map(|v| v as i64),
        wrong_cps,
        "right and wrong agree -- the fixture is no longer asymmetric"
    );
    assert_ne!(right.time_ms, wrong_time, "right and wrong agree on the time");
}

/// The gate reaches the server through `ghost::oracle` and nothing else: there
/// is no second parser in `tmtraj` to drift against this one. A grep is a
/// strange thing to put in a test suite and it is the only check that survives
/// somebody pasting a "quick" parser back in.
#[test]
fn the_gate_keeps_no_parser_of_its_own() {
    let src = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/src/intgcmd.rs"))
        .expect("read intgcmd.rs");
    let code: String = src
        .lines()
        .filter(|l| !l.trim_start().starts_with("//"))
        .collect::<Vec<_>>()
        .join("\n");
    for needle in ["\"ValidatedResult\"", "DeclaredResult", "NbCheckpoints"] {
        assert!(
            !code.contains(needle),
            "{} is being parsed in intgcmd.rs again; the server's output has ONE reader, \
             ghost::oracle",
            needle
        );
    }
}
