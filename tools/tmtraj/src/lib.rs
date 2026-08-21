//! `tmtraj` -- Trackmania 2020 trajectory decoding and racing-line analysis.
//!
//! Rust port of the Python in `tmtas/trajectories/` (`entrec.py`,
//! `decode_all.py`, `cluster_lines.py`) and `tmtas/code/lines.py`.

pub mod entrec;
pub mod gbx;
pub mod json;
pub mod lines;
pub mod recwrite;
pub mod selftest;
pub mod stats;
pub mod nancmd;
pub mod whlcmd;
pub mod checkcmd;
pub mod tailcmd;
pub mod intgcmd;
pub mod anoncmd;
pub mod setdeclcmd;
pub mod recspancmd;
pub mod facingcmd;
pub mod rectimecmd;
pub mod manifest;
