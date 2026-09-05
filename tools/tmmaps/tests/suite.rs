//! `cargo test` and `tmmaps selftest` are the same suite.
//!
//! There is exactly one place the checks live — `src/selftest.rs` — and this
//! runs the shipped binary rather than a second copy of the logic compiled
//! into a test harness. If the binary is broken, this fails; a test that
//! passes against a binary nobody runs is decoration.
//!
//! The oracle tier needs a dedicated server. Set `TM_SERVER=/path/to/dir`.
//! Without one those checks SKIP and this still passes — which is why CI, and
//! anyone reporting a result, should use `--strict`, where a skip is a
//! failure.

use std::path::Path;
use std::process::Command;

fn run(extra: &[&str]) -> (bool, String) {
    let mut c = Command::new(env!("CARGO_BIN_EXE_tmmaps"));
    c.arg("selftest").args(extra);
    if let Ok(s) = std::env::var("TM_SERVER") {
        c.args(["--server", &s]);
    }
    let out = c.output().expect("run tmmaps selftest");
    let mut text = String::from_utf8_lossy(&out.stdout).into_owned();
    text.push_str(&String::from_utf8_lossy(&out.stderr));
    (out.status.success(), text)
}

#[test]
fn fixed_length_item_model_patch_survives_write() {
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR")).join("testdata/map1.Map.Gbx");
    let original = tmmaps::map::MapFile::load(&fixture);
    let donor = original
        .items
        .iter()
        .find(|it| original.item_ids[it.model_field].is_def)
        .expect("inline item model");
    let alias = "X".repeat(donor.model.len());
    let mut changed = tmmaps::map::MapFile::load(&fixture);
    changed.set_item_model_same_len(donor.index, &alias);
    let out = std::env::temp_dir().join(format!("tmmaps-model-{}.Map.Gbx", std::process::id()));
    changed.write_to(&out).expect("write model patch");
    let reread = tmmaps::map::MapFile::load(&out);
    let _ = std::fs::remove_file(out);
    assert_eq!(reread.items[donor.index].model, alias);
}

#[test]
fn item_array_can_grow_and_reparse() {
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR")).join("testdata/map1.Map.Gbx");
    let original = tmmaps::map::MapFile::load(&fixture);
    let want = original.items.len() + 7;
    let mut grown = tmmaps::map::MapFile::load(&fixture);
    grown.append_item_clones(want);
    let out = std::env::temp_dir().join(format!("tmmaps-grow-{}.Map.Gbx", std::process::id()));
    grown.write_to(&out).expect("write grown map");
    let reread = tmmaps::map::MapFile::load(&out);
    let _ = std::fs::remove_file(out);
    assert_eq!(reread.items.len(), want);
    assert_eq!(reread.blocks.len(), original.blocks.len());
    assert_eq!(reread.baked.len(), original.baked.len());
}

#[test]
fn selftest_passes() {
    let (ok, text) = run(&[]);
    println!("{}", text);
    assert!(ok, "tmmaps selftest failed");
    assert!(text.contains(", 0 failed"), "no summary line in output");
}

/// `--strict` must be able to fail. A suite whose only evidence is its own
/// green line proves nothing, and the previous version of this suite returned
/// early from every oracle test when the data was missing and reported
/// `7 passed`. So: point the tool at a server that does not exist and require
/// a non-zero exit. That is the positive control for the flag itself.
#[test]
fn strict_is_not_vacuous() {
    let out = Command::new(env!("CARGO_BIN_EXE_tmmaps"))
        .args([
            "selftest",
            "--strict",
            "--server",
            "/nonexistent/tmmaps-no-server",
        ])
        .output()
        .expect("run tmmaps selftest");
    let text = String::from_utf8_lossy(&out.stdout).into_owned();
    println!("{}", text);
    assert!(
        !out.status.success(),
        "--strict passed with no dedicated server: skips are not being counted, so a green suite \
         would mean nothing"
    );
    assert!(
        text.contains("SKIP under --strict"),
        "expected the oracle tier to report skips under --strict"
    );
    // ...and the same run WITHOUT --strict must pass, or the failure above is
    // about something other than the skips.
    let out = Command::new(env!("CARGO_BIN_EXE_tmmaps"))
        .args(["selftest", "--server", "/nonexistent/tmmaps-no-server"])
        .output()
        .expect("run tmmaps selftest");
    assert!(
        out.status.success(),
        "the pure tier must pass with no server at all: {}",
        String::from_utf8_lossy(&out.stdout)
    );
}
