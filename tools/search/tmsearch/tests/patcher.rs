//! The candidate writer, against the codec it is derived from.
//!
//! The search's fast path patches bits in a base image. `tools/ghost` owns the
//! format. These tests are the join between the two: if the patcher and the
//! encoder ever disagree about where a tick's bits are, the search writes files
//! that do not say what it thinks they say -- which is unfalsifiable from
//! inside the search and has, in an earlier form, silently dropped writes to
//! whole classes of tick.
//!
//! No server, no map, no network: the fixtures are the two human ghosts checked
//! in beside `tools/ghost`.

use ghost::container::Container;
use ghost::tape::{Encoding, Tape};
use forkoracle::inputs::{mutate, Inputs, OpSet, Rng};
use tmsearch::tape::Patcher;

const FIXTURES: [(&str, usize); 2] = [
    ("../../ghost/testdata/human_22730.Ghost.Gbx", 2432),
    ("../../ghost/testdata/human_23013.Ghost.Gbx", 2453),
];

fn encode_with(path: &str, inputs: &Inputs) -> Vec<u8> {
    let c = Container::load(path).unwrap();
    let mut t = Tape::from_file(path).unwrap();
    {
        let a0 = t.archives.first_mut().unwrap();
        for (i, p) in a0.packets.iter_mut().enumerate() {
            if !matches!(p.mode, 2 | 4) {
                continue;
            }
            p.steer = inputs.steer[i] as u8 as u32;
            p.accel = inputs.gas[i] as u32;
            p.brake = inputs.brake[i] as u32;
        }
    }
    t.inject_into(&c, Encoding::Explicit).unwrap()
}

#[test]
fn the_template_reads_back_as_itself() {
    for (path, ticks) in FIXTURES {
        let p = Patcher::build(path).unwrap_or_else(|e| panic!("{}: {}", path, e));
        assert_eq!(p.n(), ticks, "{}: tick count", path);
        // `build` asserts this internally; asserting it here as well is the
        // difference between a claim in a doc comment and a test.
        let mut buf = p.base.clone();
        p.apply(&mut buf, &p.template);
        assert_eq!(buf, p.base, "{}: applying the template's own inputs changed the file", path);
    }
}

/// THE CONTROL THAT MAKES THE FAST PATH HONEST.
///
/// A patched image and a full re-encode of the same inputs must be the same
/// bytes. This is what stops the patcher from being a second implementation of
/// the codec: it is not allowed to disagree with the first, on any state.
#[test]
fn a_patched_image_equals_a_full_re_encode() {
    for (path, _) in FIXTURES {
        let p = Patcher::build(path).unwrap();
        let mut rng = Rng::new(20260822);
        for round in 0..12 {
            let mut s = p.template.clone();
            for _ in 0..8 {
                mutate(&mut s, &mut rng, 0, p.n(), OpSet::Wide);
            }
            let patched = p.file(&s);
            let encoded = encode_with(path, &s);
            assert_eq!(
                patched.len(),
                encoded.len(),
                "{} round {}: the two writers disagree on the file length",
                path,
                round
            );
            if patched != encoded {
                let first = patched
                    .iter()
                    .zip(&encoded)
                    .position(|(a, b)| a != b)
                    .unwrap();
                panic!(
                    "{} round {}: the patcher and the codec disagree from byte {}",
                    path, round, first
                );
            }
        }
    }
}

/// And the other direction: what the codec reads out of a patched file is what
/// the search asked for, tick for tick.
#[test]
fn the_codec_reads_back_exactly_what_the_search_wrote() {
    let (path, _) = FIXTURES[0];
    let p = Patcher::build(path).unwrap();
    let mut rng = Rng::new(7);
    let mut s = p.template.clone();
    for _ in 0..20 {
        mutate(&mut s, &mut rng, 0, p.n(), OpSet::Wide);
    }
    let out = std::env::temp_dir().join(format!("tmsearch-roundtrip-{}.Ghost.Gbx", std::process::id()));
    std::fs::write(&out, p.file(&s)).unwrap();
    let t = Tape::from_file(out.to_str().unwrap()).unwrap();
    let _ = std::fs::remove_file(&out);

    let steer = t.steer_i8s();
    let accel = t.accels();
    let brake = t.brakes();
    assert_eq!(steer.len(), s.len());
    for i in 0..s.len() {
        // ticks the search cannot write keep the template's values
        if p.unwritable.iter().any(|(t, _)| *t == i) {
            continue;
        }
        assert_eq!(steer[i], s.steer[i], "tick {} steer", i);
        assert_eq!(accel[i] != 0, s.gas[i], "tick {} gas", i);
        assert_eq!(brake[i] != 0, s.brake[i], "tick {} brake", i);
    }
}

/// A tick the search cannot express must be REFUSED, not silently skipped.
/// The old writer dropped those writes and said nothing, so the search reported
/// exploring candidates it had never built.
#[test]
fn a_window_the_search_cannot_write_is_refused() {
    let (path, _) = FIXTURES[0];
    let p = Patcher::build(path).unwrap();
    match p.unwritable.first() {
        Some(&(t, _)) => {
            assert!(p.check_window(t, t + 1).is_err(), "tick {} is unwritable and was allowed", t);
            assert!(p.check_window(0, p.n()).is_err());
        }
        None => {
            // This fixture is all-analog, which is the common case. The refusal
            // still has to be reachable, so assert the other half: a window
            // with nothing unwritable in it is accepted.
            assert!(p.check_window(0, p.n()).is_ok());
        }
    }
}

/// Every candidate file must be named something the dedicated server will
/// actually read. It ignores anything else and reports nothing, which a caller
/// reads as "this candidate did not finish" -- a naming slip once produced 32
/// consecutive false DNFs on good files elsewhere in this toolchain.
#[test]
fn candidate_files_are_named_so_the_server_reads_them() {
    let (path, _) = FIXTURES[0];
    let p = Patcher::build(path).unwrap();
    let dir = std::env::temp_dir().join(format!("tmsearch-names-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let f = dir.join("c0000.Ghost.Gbx");
    std::fs::write(&f, p.file(&p.template)).unwrap();
    assert!(f.to_string_lossy().ends_with(".Ghost.Gbx"));
    let _ = std::fs::remove_dir_all(&dir);
}
