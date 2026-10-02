//! SHA-256 lives in `gbx::sha` now (a ghost's skin PackDesc carries the
//! SHA-256 of the skin zip — found 2026-09-12 — so the ghost crate needs it
//! too); this module re-exports it so nothing here moves.
pub use gbx::sha::*;
