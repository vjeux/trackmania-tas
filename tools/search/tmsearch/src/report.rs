//! Printing. Times are **seconds with a decimal**, everywhere, with no
//! exceptions -- `36.049`, never `36049`.

pub use ghost::secs;

/// A signed millisecond difference, as seconds: `-0.021`, `+1.250`.
pub fn delta(ms: i64) -> String {
    format!("{}{}", if ms > 0 { "+" } else { "" }, secs(ms))
}

/// A duration of wall-clock work, in minutes and seconds.
pub fn elapsed(s: f64) -> String {
    let m = (s / 60.0).floor();
    format!("{:.0}m{:04.1}s", m, s - m * 60.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_raw_millisecond_integer_ever_reaches_a_human() {
        assert_eq!(secs(36049), "36.049");
        assert_eq!(delta(-21), "-0.021");
        assert_eq!(delta(1250), "+1.250");
    }
}
