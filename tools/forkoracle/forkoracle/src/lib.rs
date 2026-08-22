//! The fork server's DRIVER side: everything the SEARCH and `fk` both do to a
//! live dedicated server.
//!
//! The split is by CALLER, not by topic. What is here is what `tmsearch` runs
//! on every candidate — start a server, arm the watchdog, resume a fork, locate
//! the car without a reference, score it — and `fk` drives the same code, so
//! there is one definition of what a resume is.
//!
//! What is NOT here is the CLOCK-FIRST locator (`fk::locate`): nothing in the
//! search calls it. It finds the engine's race clock first and keys every
//! sample on it, which is what makes it correct when the car does not move (a
//! respawn) or the engine writes the state twice inside one tick. `blind` keys
//! on the 24-byte position+velocity window instead and is what a search needs,
//! because an evolved candidate has no recorded telemetry to match against.
//!
//! `pred_core` is the same source the LD_PRELOAD shim compiles into the child,
//! so a predicate means exactly one thing on both sides of the fork.

pub mod pred_core;
pub mod forksrv;
pub mod pred;
pub mod layout;
pub mod blind;
pub mod mutate;
pub mod procmem;
