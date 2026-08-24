//! `tmexplore` — an archive search over savestates, for a deterministic game
//! that can be rewound.
//!
//! # Why this is not a controller
//!
//! The prior attempt at this problem built a cascade controller — pure pursuit
//! plus a lateral term into a desired curvature, a grip clamp, a feed-forward
//! through a bicycle-model gain, an integral trim — and searched its eight
//! gains. Its own honest ablation is the sentence that reframed the project:
//!
//! > **Given the human world record's exact line to track, the controller
//! > still only reached 1154 m of 1631 m.**
//!
//! Not the geometry. The controller. A feedback controller is what you build
//! when you must decide in real time and cannot rewind. We can rewind: the
//! headless dedicated server re-simulates deterministically and the fork
//! server pauses mid-simulation and branches from the paused state. That is a
//! savestate, and **a deterministic game with savestates is a tree search
//! problem, not a control problem.**
//!
//! # The four parts
//!
//! * [`action`] — the macro alphabet. An edge is one `(steer, gas, brake)`
//!   held for `k` ticks, so the branching factor is 12, not 1020.
//! * [`archive`] — the quantized **whole car state at a station**, best entry
//!   per bin, non-overlapping by construction, plus the selection policy that
//!   favours the frontier.
//! * [`trunk`] — the committed input prefixes, shared, so an archive of
//!   100 000 states is not 450 M inputs.
//! * [`explore`] — the loop, which is the least interesting part.
//!
//! [`branch`] holds the two interfaces this is built against — agent B's
//! `Route` and agent D's `Branch` — and [`toy`] holds a stub of both, which is
//! how the whole thing was built and tested before either landed.
//!
//! # The rules this crate is shaped by
//!
//! * **No `FINISH_BASE`.** [`outcome::Reached`] is an enum whose `Ord` puts
//!   every finisher above every non-finisher by construction.
//! * **A result is a written file the plain oracle re-simulates.** A fork
//!   answer makes a candidate; only [`branch::PlainOracle`] makes a result.
//! * **No human ghost, anywhere, for anything.** There is no reference line in
//!   this crate, no demonstration, no imitation term and no yardstick. The
//!   diagnostics are self-referential on purpose: best speed *we* have ever
//!   reached at a station, and the furthest-station histogram.

pub mod action;
pub mod archive;
pub mod branch;
pub mod explore;
pub mod outcome;
pub mod rng;
pub mod toy;
pub mod trunk;
