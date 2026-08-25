//! tmsite as a library, so `tests/` can reach the pieces the binary drives.
//!
//! The GBX container, the ghost input tape and the bit codec are NOT here:
//! they live once, in the workspace's `gbx` crate. tmsite used to carry a
//! private third copy of all three (`gbx.rs`, `ghost.rs`, `bits.rs`); they are
//! gone. See `tick.rs` for the one thing that copy did which the shared crate
//! does not: pick the declared race time out of the body.

pub mod compact;
pub mod json;
pub mod pyfmt;
pub mod names;
pub mod records;
pub mod serve;
pub mod site;
pub mod stats;
pub mod tick;
pub mod traj;
