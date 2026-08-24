//! `tmauto` — the TM2020 autopilot's oracle, provenance and container layer.
//!
//! This crate owns three things, and everything else in the project depends on
//! them:
//!
//! * [`verdict`] — what the oracle says about a tape, and how verdicts order.
//! * [`tape`] — the input sequence and its `PROV` record.
//! * [`synth`] — manufacturing a `.Ghost.Gbx` container **from nothing**, and
//!   [`gate`] — refusing to load any file that is not chain-rooted at one.
//!
//! The no-ghost rule, in one sentence: **no component reads a `.Ghost.Gbx` a
//! human drove** — not as driver input, not for the route, not for tuning, not
//! as an evaluation reference. This crate is where that rule is either kept or
//! broken, because it is the only writer of containers and the only loader of
//! input files.

pub mod gate;
pub mod oracle;
pub mod sha;
pub mod synth;
pub mod tape;
pub mod verdict;

pub use gate::{Decision, Gate, Ledger, Refusal};
pub use tape::{Input, Producer, Prov, Tape};
pub use verdict::{Eval, ForkDistance, OracleSource, Score, TapeHash, Verdict};
