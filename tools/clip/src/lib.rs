//! `clip` -- the video publishing tools, in Rust, std only.
//!
//! This crate replaced three shell scripts (`tools/ship-clip.sh`,
//! `tools/splitscreen.sh`, `trainer/playtest.sh`). Everything they knew that
//! was not obvious from their code is carried in the comments here: why an
//! asset is registered in the release body, why the last fetch scrubs its
//! environment, why the shorter run is held on its final frame, and why Chrome
//! is killed rather than waited for.
//!
//! Nothing in here is a dependency: the render box builds offline.

pub mod cut;
pub mod fmt;
pub mod inventory;
pub mod overlay;
pub mod platform;
pub mod playtest;
pub mod proc;
pub mod ship;
pub mod split;
