//! `mapgeom --debug NAME[,NAME…]`: the diagnostic prints an investigation
//! turns on, by name, in one place — what used to be one `TINY_DEBUG_*` /
//! `TINY_DUMP_*` / `TINY_*_LIST` environment variable each (2026-09-08: 155
//! `TINY_*` variables, most of them dead probes, were folded into flags or
//! constants; the few worth keeping are these).
//!
//! | name     | what it prints                                                      |
//! |----------|---------------------------------------------------------------------|
//! | `lookup` | why a block name has no block-info file (the index and store sizes) |
//! | `decls`  | every source Solid2 header word and, per visual, the vertex declarations with the distinct values of each one-word element (a shader's per-vertex inputs) |
//! | `trees`  | every tree the clearance drops, with its deck                       |
//!
//! `mapgeom --debug help` lists them.

use std::collections::BTreeSet;
use std::sync::OnceLock;

/// Every name the flag knows, with its one-line meaning.
pub const NAMES: &[(&str, &str)] = &[
    ("lookup", "why a block name has no block-info file (index and store sizes)"),
    ("decls", "source Solid2 header words and per-visual vertex declarations with distinct element values"),
    ("trees", "every tree the clearance drops, with the deck it hit"),
];

static ENABLED: OnceLock<BTreeSet<String>> = OnceLock::new();

/// Turn on the comma-separated names of a `--debug` value. Unknown names are
/// an error naming the known ones; `help` prints the table and exits.
pub fn set(spec: &str) -> Result<(), String> {
    let mut set = BTreeSet::new();
    for name in spec.split(',').map(str::trim).filter(|s| !s.is_empty()) {
        if name == "help" {
            for (n, what) in NAMES {
                println!("  --debug {n:<8} {what}");
            }
            std::process::exit(0);
        }
        if !NAMES.iter().any(|(n, _)| *n == name) {
            return Err(format!("--debug {name}: not one of {}", NAMES.iter().map(|(n, _)| *n).collect::<Vec<_>>().join(", ")));
        }
        set.insert(name.to_string());
    }
    ENABLED.set(set).map_err(|_| "--debug given twice".to_string())
}

/// Whether `name` was asked for.
pub fn on(name: &str) -> bool {
    ENABLED.get().map(|s| s.contains(name)).unwrap_or(false)
}
