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
//! use ghost::tape::{Tape, Encoding};
//! let t = Tape::from_file("run.Ghost.Gbx")?;
//! t.verbatim_is_identity()?;                       // the codec's own control
//! println!("{} ticks", t.n());
//! # Ok::<(), String>(())
//! ```

pub mod bits;
pub mod cli;
pub mod container;
pub mod engine;
pub mod ident;
pub mod oracle;
pub mod regen;
pub mod selftest;
pub mod tape;
pub mod trim;
pub mod verify;

pub use container::{secs, Container};
pub use tape::{Encoding, Tape};

/// The map uid a `.Map.Gbx` declares.
///
/// Two encodings, because there are two kinds of map file: one written by the
/// editor, which carries an XML header, and the copy carried inside a replay,
/// which has a binary header instead. Returning `None` for the second and
/// calling it "no uid" is a harness limit reported as a fact about the file.
pub fn map_uid_of(data: &[u8]) -> Option<String> {
    let head = &data[..data.len().min(60000)];
    let s = String::from_utf8_lossy(head);
    if let Some(i) = s.find("uid=\"") {
        let rest = &s[i + 5..];
        if let Some(j) = rest.find('"') {
            return Some(rest[..j].to_string());
        }
    }
    let mut i = 0usize;
    while i + 31 <= head.len() {
        if u32::from_le_bytes(head[i..i + 4].try_into().unwrap()) == 27 {
            if let Ok(t) = std::str::from_utf8(&head[i + 4..i + 31]) {
                if t.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-') {
                    return Some(t.to_string());
                }
            }
        }
        i += 1;
    }
    None
}
