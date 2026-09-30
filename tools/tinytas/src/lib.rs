//! `tinytas` — the per-map pipeline for the half-scale (tiny) Summer 2026
//! campaign, one subcommand per stage. See `tiny-tas/PIPELINE.md`.
//!
//! Everything here is glue over the existing instruments (`tmauto` for
//! containers and the plain oracle, `tmmaps` for map reading, `tmtraj` for
//! JSON and trajectories). Nothing here simulates; the dedicated server does.

pub mod authorghost;
pub mod scale;
