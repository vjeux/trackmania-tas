//! Numbers, printed the way this project prints them.

/// A duration, in SECONDS with a decimal: `36.049`, never `36049`.
///
/// Standing project rule (FILMING.md §4). Every duration this crate prints goes
/// through here so there is exactly one place it can be got wrong.
pub fn secs(s: f64) -> String {
    format!("{s:.3}")
}

/// The number `ffprobe -show_entries format=duration -of csv=p=0` printed.
///
/// Returns `None` for anything that is not a positive finite number, which
/// includes ffprobe's own `N/A` and the empty output it gives for a file that
/// is not playable. "Does not probe" is a refusal everywhere in this crate, so
/// the parse has to be strict: a file that probes as `0` or `N/A` is exactly
/// the half-written upload the gate exists to catch.
///
/// The Windows ffprobe.exe terminates its line with CRLF; the shell version
/// piped it through `tr -d '\r'`, and dropping that made the value unparseable.
pub fn parse_probe_duration(raw: &str) -> Option<f64> {
    let t = raw.trim().trim_end_matches('\r').trim();
    let v: f64 = t.parse().ok()?;
    if v.is_finite() && v > 0.0 {
        Some(v)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn seconds_with_a_decimal_never_milliseconds() {
        assert_eq!(secs(36.049), "36.049");
        assert_eq!(secs(6.36), "6.360");
        assert_eq!(secs(265.1594), "265.159");
        assert_eq!(secs(1.0), "1.000");
        // the value that must never appear: an integer count of milliseconds
        assert!(!secs(36.049).contains("36049"));
    }

    #[test]
    fn probe_duration_parsing() {
        assert_eq!(parse_probe_duration("36.049000\n"), Some(36.049));
        // CRLF, as the Windows ffprobe.exe writes it
        assert_eq!(parse_probe_duration("6.360000\r\n"), Some(6.36));
        assert_eq!(parse_probe_duration("N/A"), None);
        assert_eq!(parse_probe_duration(""), None);
        assert_eq!(parse_probe_duration("0.000000"), None);
        assert_eq!(parse_probe_duration("-1"), None);
    }
}
