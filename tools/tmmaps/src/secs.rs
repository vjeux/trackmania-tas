//! Times are seconds with a decimal, everywhere, in every printed line.
//!
//! The project's own rule, and it is not cosmetic: a long raw millisecond
//! integer is hard to read and easy to mistake for another quantity. `16316`
//! and `16.316` carry the same information; only one of them is legible next
//! to `-0.101`.

/// `16316` -> `"16.316"`. Negative values keep their sign: `-101` -> `"-0.101"`.
pub fn ms(v: i64) -> String {
    let neg = v < 0;
    let a = v.unsigned_abs();
    format!("{}{}.{:03}", if neg { "-" } else { "" }, a / 1000, a % 1000)
}

/// An optional time, with a marker for "the run never got there".
pub fn opt(v: Option<i64>) -> String {
    match v {
        Some(x) => ms(x),
        None => "DNF".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_as_seconds() {
        assert_eq!(ms(16316), "16.316");
        assert_eq!(ms(0), "0.000");
        assert_eq!(ms(7), "0.007");
        assert_eq!(ms(-101), "-0.101");
        assert_eq!(ms(2672290), "2672.290");
        assert_eq!(opt(None), "DNF");
        assert_eq!(opt(Some(19538)), "19.538");
    }
}

/// A signed delta, in seconds, with an explicit sign: `+0.000`, `-0.101`.
///
/// Kept separate from `ms` because a delta with no sign reads as a time, and
/// the two sit next to each other in every verification table this tool
/// prints.
pub fn signed(v: i64) -> String {
    if v >= 0 {
        format!("+{}", ms(v))
    } else {
        ms(v)
    }
}

/// A header-XML time attribute, which is a STRING of milliseconds and may be
/// absent (`"-"`), rendered as seconds with a decimal.
///
/// It stays a string rather than becoming an `Option<i64>` because "the map
/// does not declare one" and "the map declares zero" are different facts and
/// this project has been bitten by collapsing that distinction.
pub fn secs_str(raw: &str) -> String {
    match raw.trim().parse::<i64>() {
        Ok(v) => ms(v),
        Err(_) => raw.to_string(),
    }
}

#[cfg(test)]
mod secs_str_tests {
    use super::*;

    #[test]
    fn a_header_time_renders_as_seconds() {
        assert_eq!(secs_str("23144"), "23.144");
        assert_eq!(secs_str("0"), "0.000");
    }

    #[test]
    fn an_absent_time_stays_absent_rather_than_becoming_zero() {
        // `"-"` means the map declares no such time. Rendering it as `0.000`
        // would invent an author time of zero, which every run beats.
        assert_eq!(secs_str("-"), "-");
        assert_eq!(secs_str(""), "");
    }
}
