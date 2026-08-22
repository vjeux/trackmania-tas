//! Argument handling for the `ghost` command line.
//!
//! Kept apart from the data path on purpose: `die` exits the process, which is
//! right for a CLI and wrong for a library, so nothing under `tape`,
//! `container`, `oracle` or `verify` may call it. Those return `Result`.

pub fn die(m: impl AsRef<str>) -> ! {
    eprintln!("ghost: {}", m.as_ref());
    std::process::exit(2)
}

pub fn flag<'a>(a: &'a [String], name: &str) -> Option<&'a str> {
    a.iter().position(|x| x == name).and_then(|i| a.get(i + 1)).map(|s| s.as_str())
}

pub fn need<'a>(a: &'a [String], name: &str) -> &'a str {
    flag(a, name).unwrap_or_else(|| die(format!("missing {} VALUE", name)))
}

pub fn has(a: &[String], name: &str) -> bool {
    a.iter().any(|x| x == name)
}

pub fn num(a: &[String], name: &str) -> Option<i64> {
    flag(a, name).map(|v| v.parse().unwrap_or_else(|_| die(format!("{} wants a number", name))))
}

