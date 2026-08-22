//! `tmsearch` -- the TAS search for Trackmania 2020, and the one place a
//! result is allowed to leave it.
//!
//! The layers, bottom up:
//!
//! * [`tape`] turns a template into a base image plus per-tick bit positions,
//!   using `tools/ghost` as the only definition of the file format.
//! * [`inputs`] is the per-tick state and the operators that change it.
//! * [`score`] is what a candidate is worth, in a type where a failure cannot
//!   outrank a finish.
//! * [`batch`] and [`forkeval`] are the two evaluators: the authoritative one
//!   and the fast one.
//! * [`search`] is the island/Metropolis loop over either.
//! * [`guard`] is the phantom guard, and it owns the output directory: the only
//!   way to bank a result is to offer it to the plain oracle first.

pub mod analyze;
pub mod batch;
pub mod forkeval;
pub mod guard;

pub mod refline;
pub mod report;
pub mod root;
pub mod score;
pub mod search;
pub mod tape;
