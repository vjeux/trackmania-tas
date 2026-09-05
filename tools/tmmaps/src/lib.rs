//! `tmmaps` — everything this project does to a TM2020 **map**.
//!
//! One container implementation, a control behind every operation. Its
//! counterpart is `tools/ghost`, which owns the **ghost / replay** format; the
//! two never overlap. A recording that carries its own map is a `ghost`
//! problem until the map is out of it:
//!
//! ```text
//! ghost map extract R.Replay.Gbx --out m.Map.Gbx
//! tmmaps move m.Map.Gbx --out m2.Map.Gbx --move 2089@1520,300,600
//! ghost map set R.Replay.Gbx R2.Replay.Gbx --map m2.Map.Gbx
//! ```
//!
//! That composition is deliberate. `u02` used to reach into the replay's
//! carried map itself (`u02 movefree`), which meant two implementations of the
//! embedded-map chunk and two of the block movers. `u02` is deleted; see
//! `MAPS.md` §"What happened to u02".
//!
//! Times are printed as **seconds with a decimal** (`16.316`), never as raw
//! milliseconds.
//!
//! This is a library as well as a binary. `mapgeom` places the blocks and
//! items `map.rs` reads into world space and turns them into a 3D model; it
//! calls in here rather than keeping a second `.Map.Gbx` reader, for the same
//! reason `tmmaps` calls into `ghost` for a reference ghost's splits.

pub mod census;
pub mod cli;
pub mod controls;
pub mod dropscan;
pub mod gbx;
pub mod ghost;
pub mod header;
pub mod map;
pub mod oracle;
pub mod rotate;
pub mod secs;
pub mod segments;
pub mod selftest;
pub mod splice;
pub mod tiny;
