//! A damaged file must be REFUSED, never fatal.
//!
//! The decoder reads lengths, counts and offsets out of the file itself. Every
//! one of those is attacker-controlled in the only sense that matters here:
//! ghosts arrive from leaderboards, from other people, from disks that rot. A
//! field that says "4000 bytes follow" when 40 do must produce a verdict, not
//! a Rust panic.
//!
//! Three real panics were found by writing this and are fixed:
//!
//!   * `container.rs` sliced `data[r.o .. r.o + csize]` with `csize` read from
//!     the file -- "range end index out of range" on any truncated ghost.
//!   * `lzo_decompress` ASSERTED that decompression succeeded, so a corrupt
//!     body aborted the process with `lzo1x_decompress_safe -> -4`.
//!   * `bits.rs` asserted `byi < d.len()`, so one flipped bit in a length
//!     field walked the bit reader off the end.
//!
//! Hermetic: builds its own corrupt inputs from a fixture, no server, no
//! network. Exit 101 is a Rust panic and is what this test is looking for.

use std::path::PathBuf;
use std::process::Command;

fn ghost_bin() -> Option<PathBuf> {
    let mut p = std::env::current_exe().ok()?;
    p.pop();
    if p.ends_with("deps") {
        p.pop();
    }
    let g = p.join("ghost");
    g.exists().then_some(g)
}

fn fixture() -> Option<Vec<u8>> {
    let p = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../testdata/human_22730.Ghost.Gbx");
    std::fs::read(p).ok()
}

/// Run `ghost verify` on some bytes and return the exit code.
fn verify_bytes(ghost: &PathBuf, dir: &PathBuf, name: &str, bytes: &[u8]) -> i32 {
    let f = dir.join(name);
    std::fs::write(&f, bytes).expect("write case");
    Command::new(ghost)
        .args(["verify", f.to_str().unwrap()])
        .output()
        .map(|o| o.status.code().unwrap_or(-1))
        .unwrap_or(-1)
}

/// A directory of this test's own. Tests run in PARALLEL threads of one
/// process, so keying only on the pid gave all three the same path and the
/// first one to finish deleted it out from under the others ("write case:
/// NotFound"). The case name makes it unique.
fn tmp(case: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("ghost-fuzz-{}-{}", std::process::id(), case));
    std::fs::create_dir_all(&d).expect("tmp dir");
    d
}

#[test]
fn truncation_never_panics() {
    let (Some(ghost), Some(good)) = (ghost_bin(), fixture()) else {
        return;
    };
    let dir = tmp("trunc");
    let mut panics = Vec::new();
    for pct in [1, 5, 10, 20, 30, 40, 50, 60, 70, 80, 90, 95, 99] {
        let n = good.len() * pct / 100;
        let rc = verify_bytes(&ghost, &dir, "t.Ghost.Gbx", &good[..n]);
        if rc == 101 {
            panics.push(format!("{pct}% ({n} bytes)"));
        }
    }
    let _ = std::fs::remove_dir_all(&dir);
    assert!(
        panics.is_empty(),
        "ghost verify PANICKED on truncated input at: {}",
        panics.join(", ")
    );
}

#[test]
fn single_bit_flips_never_panic() {
    let (Some(ghost), Some(good)) = (ghost_bin(), fixture()) else {
        return;
    };
    let dir = tmp("bitflip");
    let mut panics = Vec::new();
    // Spread across the header, the chunk table, the tape and the record
    // rather than clustering: each region has its own length fields.
    let n = good.len();
    let offsets: Vec<usize> = (0..40).map(|i| i * n / 40).collect();
    for off in offsets {
        for bit in [1u8, 4, 64, 128] {
            let mut b = good.clone();
            b[off] ^= bit;
            let rc = verify_bytes(&ghost, &dir, "b.Ghost.Gbx", &b);
            if rc == 101 {
                panics.push(format!("byte {off} bit {bit}"));
            }
        }
    }
    let _ = std::fs::remove_dir_all(&dir);
    assert!(
        panics.is_empty(),
        "ghost verify PANICKED on a single bit flip at: {}",
        panics.join(", ")
    );
}

#[test]
fn garbage_and_empty_input_never_panic() {
    let Some(ghost) = ghost_bin() else { return };
    let dir = tmp("garbage");
    let mut panics = Vec::new();

    let cases: Vec<(&str, Vec<u8>)> = vec![
        ("empty", Vec::new()),
        ("one byte", vec![0x47]),
        ("magic only", b"GBX".to_vec()),
        ("magic + junk", {
            let mut v = b"GBX".to_vec();
            v.extend(std::iter::repeat(0xFF).take(64));
            v
        }),
        ("all zeros", vec![0u8; 4096]),
        ("all ones", vec![0xFFu8; 4096]),
    ];
    for (name, bytes) in cases {
        if verify_bytes(&ghost, &dir, "g.Ghost.Gbx", &bytes) == 101 {
            panics.push(name);
        }
    }
    let _ = std::fs::remove_dir_all(&dir);
    assert!(
        panics.is_empty(),
        "ghost verify PANICKED on: {}",
        panics.join(", ")
    );
}
