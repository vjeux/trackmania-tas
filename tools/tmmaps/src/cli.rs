//! Command-line plumbing shared by the binary and the commands themselves.
//!
//! These used to live in `main.rs`, which was fine while `tmmaps` was only a
//! binary. It is now also a library — `mapgeom` places the blocks and items
//! that `map.rs` reads, and a second map reader in a second crate is exactly
//! the failure this toolchain has already paid for once — so the helpers the
//! command modules call have to be reachable from the library root.

pub fn flag<'a>(args: &'a [String], name: &str) -> Option<&'a str> {
    args.iter()
        .position(|a| a == name)
        .and_then(|i| args.get(i + 1))
        .map(|s| s.as_str())
}

pub fn flag_multi(args: &[String], name: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut i = 0;
    while i < args.len() {
        if args[i] == name {
            let mut j = i + 1;
            while j < args.len() && !args[j].starts_with('-') {
                out.push(args[j].clone());
                j += 1;
            }
            i = j;
        } else {
            i += 1;
        }
    }
    out
}

pub fn has(args: &[String], name: &str) -> bool {
    args.iter().any(|a| a == name)
}

pub fn jobs_of(args: &[String]) -> usize {
    flag(args, "-j")
        .or_else(|| flag(args, "--jobs"))
        .and_then(|s| s.parse().ok())
        .unwrap_or(8)
}

pub fn server_of(args: &[String]) -> String {
    flag(args, "--server").unwrap_or(crate::oracle::DEFAULT_SERVER).to_string()
}

/// A refusal the user can act on: exit 3, message on stderr, no backtrace.
///
/// A refusal that arrives as a Rust panic tells the reader to run with
/// `RUST_BACKTRACE=1`, which is exactly the wrong instruction — the tool is
/// working, the command was wrong. Panics stay for invariants nobody can
/// trigger from the command line.
pub fn die(msg: &str) -> ! {
    eprintln!("tmmaps: {}", msg);
    std::process::exit(3);
}
