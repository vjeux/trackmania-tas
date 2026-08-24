//! Time, without a dependency.
//!
//! Unix seconds in, ISO-8601 UTC out, and back. `chrono` would be one line of
//! Cargo.toml and one more thing that has to download on a fresh box before
//! anything works.

pub fn now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

/// Howard Hinnant's civil-from-days, transcribed.
fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = (z - era * 146_097) as u64; // [0, 146096]
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146_096) / 365; // [0, 399]
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100); // [0, 365]
    let mp = (5 * doy + 2) / 153; // [0, 11]
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32; // [1, 31]
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32; // [1, 12]
    (if m <= 2 { y + 1 } else { y }, m, d)
}

fn days_from_civil(y: i64, m: u32, d: u32) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = (y - era * 400) as u64; // [0, 399]
    let mp = if m > 2 { m - 3 } else { m + 9 } as u64;
    let doy = (153 * mp + 2) / 5 + d as u64 - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe as i64 - 719_468
}

pub fn iso(ts: i64) -> String {
    let days = ts.div_euclid(86_400);
    let secs = ts.rem_euclid(86_400);
    let (y, m, d) = civil_from_days(days);
    format!(
        "{y:04}-{m:02}-{d:02}T{:02}:{:02}:{:02}Z",
        secs / 3600,
        (secs % 3600) / 60,
        secs % 60
    )
}

/// Accepts `YYYY-MM-DDTHH:MM:SSZ`, and the same without the `Z`.
pub fn parse_iso(s: &str) -> Option<i64> {
    let s = s.trim().trim_end_matches('Z');
    let (date, time) = s.split_once('T')?;
    let mut dp = date.split('-');
    let y: i64 = dp.next()?.parse().ok()?;
    let mo: u32 = dp.next()?.parse().ok()?;
    let d: u32 = dp.next()?.parse().ok()?;
    if dp.next().is_some() || !(1..=12).contains(&mo) || !(1..=31).contains(&d) {
        return None;
    }
    let mut tp = time.split(':');
    let h: i64 = tp.next()?.parse().ok()?;
    let mi: i64 = tp.next()?.parse().ok()?;
    let se: i64 = tp.next().unwrap_or("0").parse().ok()?;
    if !(0..24).contains(&h) || !(0..60).contains(&mi) || !(0..61).contains(&se) {
        return None;
    }
    Some(days_from_civil(y, mo, d) * 86_400 + h * 3600 + mi * 60 + se)
}

/// `3h 12m` — for a status page a person reads.
pub fn dur(secs: i64) -> String {
    let neg = secs < 0;
    let s = secs.abs();
    let out = if s < 60 {
        format!("{s}s")
    } else if s < 3600 {
        format!("{}m", s / 60)
    } else if s < 86_400 {
        format!("{}h {}m", s / 3600, (s % 3600) / 60)
    } else {
        format!("{}d {}h", s / 86_400, (s % 86_400) / 3600)
    };
    if neg {
        format!("-{out}")
    } else {
        out
    }
}

/// Milliseconds as SECONDS with a decimal — `23.144`, never `23144`.
/// The project's presentation rule, in one function so nobody re-derives it.
pub fn ms_as_seconds(ms: i64) -> String {
    let neg = ms < 0;
    let a = ms.abs();
    let s = format!("{}.{:03}", a / 1000, a % 1000);
    if neg {
        format!("-{s}")
    } else {
        s
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn iso_matches_known_instants() {
        assert_eq!(iso(0), "1970-01-01T00:00:00Z");
        assert_eq!(iso(1_787_596_086), "2026-08-24T18:28:06Z");
        // a leap day, because February is where a hand-rolled calendar dies
        assert_eq!(iso(1_709_208_000), "2024-02-29T12:00:00Z");
    }

    #[test]
    fn parse_is_the_inverse_of_render() {
        for ts in [0i64, 1, 1_787_596_086, 1_709_208_000, 2_000_000_000] {
            assert_eq!(parse_iso(&iso(ts)), Some(ts), "ts={ts}");
        }
    }

    #[test]
    fn parse_rejects_rubbish() {
        assert_eq!(parse_iso("yesterday"), None);
        assert_eq!(parse_iso("2026-13-01T00:00:00Z"), None);
        assert_eq!(parse_iso("2026-08-24T25:00:00Z"), None);
        assert_eq!(parse_iso(""), None);
    }

    #[test]
    fn times_render_as_seconds_with_a_decimal() {
        assert_eq!(ms_as_seconds(23_144), "23.144");
        assert_eq!(ms_as_seconds(5_347), "5.347");
        assert_eq!(ms_as_seconds(-21), "-0.021");
        assert_eq!(ms_as_seconds(0), "0.000");
    }

    #[test]
    fn durations_read_like_english() {
        assert_eq!(dur(45), "45s");
        assert_eq!(dur(3600 * 3 + 720), "3h 12m");
        assert_eq!(dur(86_400 * 2 + 3600 * 5), "2d 5h");
    }
}
