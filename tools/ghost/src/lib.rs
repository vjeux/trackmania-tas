//! `ghost` -- the TM2020 ghost / replay file-format library.
//!
//! This crate owns the ghost and replay format for the whole toolchain: the
//! input tape, the container chunks, the identity fields, the embedded map,
//! and the plain oracle. Everything else in the project calls in here rather
//! than keeping its own copy, because every one of the bugs this crate exists
//! to prevent was a second copy of one of these readers disagreeing with the
//! first.
//!
//! The library surface is the data path -- it returns `Result` and never
//! exits. The `cmd` functions are the CLI's entry points; they parse argv and
//! call `cli::die`, and library users should not call them.
//!
//! ```no_run
//! use gbx::tape::{Tape, Encoding};
//! let t = Tape::from_file("run.Ghost.Gbx")?;
//! t.verbatim_is_identity()?;                       // the codec's own control
//! println!("{} ticks", t.n());
//! # Ok::<(), String>(())
//! ```

pub mod cli;
pub mod engine;
pub mod hdr;
pub mod ident;
pub mod oracle;
pub mod regen;
pub mod script;
pub mod selftest;
pub mod trim;
pub mod verify;

pub use gbx::container::{secs, Container};
pub use gbx::tape::{Encoding, Tape};

pub use gbx::map_uid_of;
