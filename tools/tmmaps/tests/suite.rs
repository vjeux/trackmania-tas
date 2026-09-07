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

/// `remove_blocks` re-serialises both block chunks. Dropping NOTHING must
/// reproduce the file byte for byte (the first-use-defines lookback encoding
/// is the game's own), and dropping a slice must leave a file the parser
/// reads back with exactly the kept records, free entries, colours and
/// snapped-on tables in agreement.
#[test]
fn block_removal_roundtrips_and_reparses() {
    for name in ["map1.Map.Gbx", "map2.Map.Gbx", "goth.Map.Gbx"] {
        let fixture = Path::new(env!("CARGO_MANIFEST_DIR")).join("testdata").join(name);
        let original = tmmaps::map::MapFile::load(&fixture);
        let mut same = tmmaps::map::MapFile::load(&fixture);
        let r = same.remove_blocks(|_| false, |_| false);
        assert_eq!(r.blocks + r.baked, 0);
        let body = same.patched_body();
        if body != original.gbx.body {
            let first = body.iter().zip(&original.gbx.body).position(|(a, b)| a != b).unwrap_or(body.len().min(original.gbx.body.len()));
            let field = original.body_ids.iter().filter(|f| f.off <= first).last();
            panic!("{name}: a no-op removal changed the body ({} -> {} bytes), first difference at {first}; the Id field there: {field:?}; original bytes {:02x?} new {:02x?}", original.gbx.body.len(), body.len(), &original.gbx.body[first.saturating_sub(8)..(first + 16).min(original.gbx.body.len())], &body[first.saturating_sub(8)..(first + 16).min(body.len())]);
        }

        // drop every other authored block and every baked block whose name starts with 'D'
        let mut cut = tmmaps::map::MapFile::load(&fixture);
        let r = cut.remove_blocks(|b| b.index % 2 == 1, |b| b.name.starts_with('D'));
        let want_blocks = original.blocks.len() - r.blocks;
        let want_baked = original.baked.len() - r.baked;
        let out = std::env::temp_dir().join(format!("tmmaps-cut-{}-{name}", std::process::id()));
        cut.write_to(&out).expect("write cut map");
        let reread = tmmaps::map::MapFile::load(&out);
        let _ = std::fs::remove_file(&out);
        assert_eq!(reread.blocks.len(), want_blocks, "{name}: authored count");
        assert_eq!(reread.baked.len(), want_baked, "{name}: baked count");
        assert_eq!(reread.items.len(), original.items.len(), "{name}: items untouched");
        let kept: Vec<&tmmaps::map::BlockRec> = original.blocks.iter().filter(|b| b.index % 2 == 0).collect();
        for (a, b) in kept.iter().zip(&reread.blocks) {
            assert_eq!((a.name.as_str(), a.dir, a.raw_coords, a.flags, a.free_pos), (b.name.as_str(), b.dir, b.raw_coords, b.flags, b.free_pos), "{name}: block {} changed", a.index);
        }
        let kept_baked: Vec<&tmmaps::map::BlockRec> = original.baked.iter().filter(|b| !b.name.starts_with('D')).collect();
        for (a, b) in kept_baked.iter().zip(&reread.baked) {
            assert_eq!((a.name.as_str(), a.raw_coords, a.flags, a.free_pos), (b.name.as_str(), b.raw_coords, b.flags, b.free_pos), "{name}: baked {} changed", a.index);
        }
        if let (Some(c0), Some(c1)) = (original.colors(), reread.colors()) {
            for (a, b) in kept.iter().zip(0..) {
                assert_eq!(c0.block(a.index), c1.block(b), "{name}: colour of kept block {}", a.index);
            }
            for i in 0..original.items.len() {
                assert_eq!(c0.item(i), c1.item(i), "{name}: colour of item {i}");
            }
        }
        if let Some(st) = reread.snap_tables() {
            for (k, bi) in st.block_indexes.iter().enumerate() {
                if *bi == -1 {
                    continue;
                }
                let idx = (*bi as u32 & 0x00FF_FFFF) as usize;
                assert!(idx < want_blocks, "{name}: snapped-on group {k} names block {idx} past the {want_blocks} kept");
                // the group's block was an even source index, so it still exists
                assert_eq!(reread.blocks[idx].name, kept[idx].name, "{name}: group {k} re-pointed at the wrong block");
            }
        }
    }
}
