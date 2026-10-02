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

/// A map time scaled by `k` (the giant / tiny campaigns' "ATs ×k the existing
/// times"): the result in whole milliseconds, rounded; a time that WAS a whole
/// number of seconds (every Nadeo medal) stays one — rounded UP to the next
/// second, so a medal never gets harder than k × the source's by more than
/// the rounding. `k` may be fractional (the tiny campaign at 0.5, 2026-10-01);
/// a whole `k` reproduces the plain product exactly.
pub fn scale_time_ms(old_ms: u32, k: f64) -> u32 {
    let exact = old_ms as f64 * k;
    if old_ms % 1000 == 0 && old_ms > 0 {
        return ((exact / 1000.0).ceil() * 1000.0).round() as u32;
    }
    exact.round() as u32
}

#[cfg(test)]
mod scale_time_tests {
    use super::*;

    #[test]
    fn whole_k_is_the_plain_product() {
        assert_eq!(scale_time_ms(23144, 2.0), 46288);
        assert_eq!(scale_time_ms(25000, 2.0), 50000);
        assert_eq!(scale_time_ms(25000, 1.0), 25000);
    }

    #[test]
    fn half_k_rounds_the_author_time_and_keeps_medals_whole_seconds() {
        assert_eq!(scale_time_ms(46288, 0.5), 23144);
        assert_eq!(scale_time_ms(46289, 0.5), 23145); // 23144.5 rounds half away from zero
        assert_eq!(scale_time_ms(46000, 0.5), 23000);
        assert_eq!(scale_time_ms(47000, 0.5), 24000); // 23.5 s → 24 s, never a fractional medal
        assert_eq!(scale_time_ms(0, 0.5), 0);
    }
}
