//! The standing re-simulation sweep — the harness's first real workload.
//!
//! The project's rule is *"every banked result is re-simulated by the plain
//! oracle before it counts — standing, not a spot check."* Standing means a
//! process, and until now there was none: results were re-simulated when
//! somebody remembered. This walks every banked tape in the repo, asks the
//! plain oracle what it actually does, and compares that against the number
//! the result is published under.
//!
//! It is a good first job for the long-haul harness for reasons beyond being
//! useful: it produces countable work at a steady rate, it takes hours, it
//! reads a corpus that is not in the repo (so it exercises the "what does a
//! fresh box need?" question honestly), and its failure modes are real.
//!
//! ## The no-ghost gate, and why it is here rather than in a comment
//!
//! A human's recording is not read by this project — not as driver input, not
//! as a route, and *not as an evaluation reference, even after the fact*. The
//! repo does contain human recordings, published as context for earlier work.
//! So the sweep runs behind a gate that fails closed: a file whose provenance
//! marks it human is **refused**, and refusing it is recorded as a refusal
//! rather than passed over in silence.
//!
//! The gate is two-sided by construction and tested that way: a human file
//! must be refused and one of our own tapes must be accepted. Either half
//! alone passes for a broken gate.

use std::path::{Path, PathBuf};

pub mod maps;
pub mod sweep;

/// What a tape's name says about where it came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Provenance {
    /// Ours: produced by this project's search or its tooling.
    Ours,
    /// A human drove it. Never read.
    Human,
}

/// Markers that mean a human drove the file. Conservative on purpose: a file
/// we cannot place is treated as ours only if it carries one of the tags our
/// own tooling writes, and otherwise it is refused as unplaceable.
const HUMAN_MARKERS: &[&str] = &["HUMAN", "human_record", "WR_", "_wr_", "AUTHOR"];

/// Prefixes our own tooling writes.
const OURS_MARKERS: &[&str] = &[
    "TAS_", "tas_", "BEST_", "KEYBOARD_", "THIN_", "DDMIN_", "GRAIN", "UNIFORM_", "SEGMENT_",
    "ALPHABET", "CUT_", "ONE_ATTEMPT_", "LOWINPUT", "SYNTH_",
];

pub fn provenance(name: &str) -> Result<Provenance, String> {
    if HUMAN_MARKERS.iter().any(|m| name.contains(m)) {
        return Ok(Provenance::Human);
    }
    if OURS_MARKERS.iter().any(|m| name.starts_with(m) || name.contains(m)) {
        return Ok(Provenance::Ours);
    }
    Err(format!(
        "{name}: nothing in the name places this file. It is not read — an unplaceable \
         tape is refused, not assumed to be ours"
    ))
}

/// The millisecond a published tape is published *under*, taken from its name
/// (`TAS_12759.Ghost.Gbx` → 12.759). `None` when the name carries no number,
/// which is a fact to record rather than a reason to skip the file.
pub fn published_ms(name: &str) -> Option<i64> {
    let stem = name.split(".Ghost").next().unwrap_or(name);
    let mut best: Option<i64> = None;
    for part in stem.split(['_', '-']) {
        if part.len() >= 4 && part.chars().all(|c| c.is_ascii_digit()) {
            if let Ok(v) = part.parse::<i64>() {
                best = Some(v);
            }
        }
    }
    best
}

/// A map directory in the repo: `276874-untitled-01` → id `276874`.
pub fn map_id(dirname: &str) -> Option<String> {
    let id = dirname.split('-').next()?;
    if id.len() >= 5 && id.chars().all(|c| c.is_ascii_digit()) {
        Some(id.to_string())
    } else {
        None
    }
}

/// Human-readable map name from the directory name, because maps are referred
/// to by name and never by id outside paths.
pub fn map_name(dirname: &str) -> String {
    let rest = dirname.splitn(2, '-').nth(1).unwrap_or(dirname);
    rest.replace('-', " ")
}

/// Find the `.Map.Gbx` for an id in one of the corpus roots.
///
/// Two layouts are in use and both are real: `tm-unbeaten` puts each map in a
/// directory named for its id, and the cartographer's bank keeps them flat,
/// named for the uid. A finder that knew only one would report half the
/// corpus MISSING and send somebody refetching maps that are already here.
pub fn find_map(corpus: &[PathBuf], id: &str) -> Option<PathBuf> {
    for root in corpus {
        // flat: <root>/<id>.Map.Gbx
        let flat = root.join(format!("{id}.Map.Gbx"));
        if flat.exists() {
            return Some(flat);
        }
        // per-map directory: <root>/<id>/*.Map.Gbx
        let direct = root.join(id).join(format!("{id}.map.Map.Gbx"));
        if direct.exists() {
            return Some(direct);
        }
        let d = root.join(id);
        if d.is_dir() {
            if let Ok(rd) = std::fs::read_dir(&d) {
                let mut hits: Vec<PathBuf> = rd
                    .filter_map(|e| e.ok())
                    .map(|e| e.path())
                    .filter(|p| p.to_string_lossy().ends_with(".Map.Gbx"))
                    .collect();
                hits.sort();
                if let Some(p) = hits.into_iter().next() {
                    return Some(p);
                }
            }
        }
    }
    None
}

pub fn is_ghost(p: &Path) -> bool {
    p.to_string_lossy().ends_with(".Ghost.Gbx")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_no_ghost_gate_refuses_a_human_recording() {
        assert_eq!(
            provenance("HUMAN_RECORD_retries_cut_1214545.Ghost.Gbx").unwrap(),
            Provenance::Human
        );
        assert_eq!(
            provenance("HUMANWR_plus_early_flick_6342.Ghost.Gbx").unwrap(),
            Provenance::Human
        );
    }

    #[test]
    fn the_no_ghost_gate_accepts_one_of_our_own_tapes() {
        // The other half. A gate that refuses everything is not a gate, and a
        // gate that accepts everything is decoration; both halves are needed
        // and neither is sufficient.
        assert_eq!(provenance("TAS_12759.Ghost.Gbx").unwrap(), Provenance::Ours);
        assert_eq!(provenance("BEST_793893.Ghost.Gbx").unwrap(), Provenance::Ours);
        assert_eq!(provenance("tas_6330.Ghost.Gbx").unwrap(), Provenance::Ours);
    }

    #[test]
    fn an_unplaceable_file_is_refused_rather_than_assumed_ours() {
        // Failing open here would quietly read a human recording the day
        // somebody drops one in with an unfamiliar name.
        assert!(provenance("mystery.Ghost.Gbx").is_err());
    }

    #[test]
    fn the_published_millisecond_comes_out_of_the_name() {
        assert_eq!(published_ms("TAS_12759.Ghost.Gbx"), Some(12_759));
        assert_eq!(published_ms("TAS_15382_deep_landing.Ghost.Gbx"), Some(15_382));
        assert_eq!(published_ms("BEST_793893.Ghost.Gbx"), Some(793_893));
        assert_eq!(published_ms("regen.Ghost.Gbx"), None);
    }

    #[test]
    fn a_declared_time_in_the_name_is_not_mistaken_for_the_result() {
        // `SEGMENT_cp5_32702_DO_NOT_PUBLISH_declares_40226` has two numbers and
        // the *last* one is what the file declares, not what it does. This is
        // recorded as the known trap it is: the sweep reports both numbers and
        // lets the oracle settle it, so the parse being ambiguous here cannot
        // silently become a wrong verdict.
        assert_eq!(
            published_ms("SEGMENT_cp5_32702_DO_NOT_PUBLISH_declares_40226.Ghost.Gbx"),
            Some(40_226)
        );
    }

    #[test]
    fn map_ids_and_names_come_apart_correctly() {
        assert_eq!(map_id("276874-untitled-01").as_deref(), Some("276874"));
        assert_eq!(map_name("276874-untitled-01"), "untitled 01");
        assert_eq!(map_id("tools"), None);
        assert_eq!(map_id("_staging"), None);
    }
}

#[cfg(test)]
mod layout_tests {
    use super::*;

    #[test]
    fn both_corpus_layouts_are_found() {
        // `tm-unbeaten` uses <root>/<id>/<id>.map.Map.Gbx; the cartographer's
        // bank is flat, <root>/<uid>.Map.Gbx. A finder that knew only one
        // reported 28 of 66 maps MISSING and would have sent somebody
        // refetching maps that were already on the box.
        let root = std::env::temp_dir().join(format!("resim-layout-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join("276874")).unwrap();
        std::fs::write(root.join("276874/276874.map.Map.Gbx"), b"a").unwrap();
        std::fs::write(root.join("buNzfsVlp2NF2oWtHM3729dEylg.Map.Gbx"), b"b").unwrap();

        assert!(find_map(&[root.clone()], "276874").is_some(), "per-map directory");
        assert!(find_map(&[root.clone()], "buNzfsVlp2NF2oWtHM3729dEylg").is_some(), "flat");
        assert!(find_map(&[root], "nosuchmap").is_none(), "and it still says no");
    }
}
