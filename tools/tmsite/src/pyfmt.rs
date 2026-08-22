//! Python-compatible formatting helpers.
//!
//! The port has to reproduce CPython's output byte for byte (the check is a
//! `cmp` against the shipped page), so three CPython behaviours are modelled
//! explicitly here rather than approximated:
//!
//!   * `repr(float)` -- shortest round-tripping decimal, always with a `.0` on
//!     integral values (Rust's `{}` prints `1584`, Python prints `1584.0`).
//!   * `round(x, n)` -- correctly rounded, ties to even, against the *exact*
//!     binary value. Rust's `{:.n}` has the same contract, so it is used as the
//!     rounding primitive and the result parsed back, exactly as CPython does.
//!   * `%`-formatting of a template dict (`%(key)s`, `%(key)d`, `%(key)f`, `%%`).

pub enum Val {
    Str(String),
    Int(i64),
    Float(f64),
}

/// `tmpl % {key: value}` for the subset of conversions the two templates use.
pub fn pyformat(tmpl: &str, vals: &[(&str, Val)]) -> String {
    let b = tmpl.as_bytes();
    let mut out = String::with_capacity(tmpl.len() + 1024);
    let mut i = 0usize;
    while i < b.len() {
        if b[i] != b'%' {
            let start = i;
            while i < b.len() && b[i] != b'%' {
                i += 1;
            }
            out.push_str(&tmpl[start..i]);
            continue;
        }
        // b[i] == '%'
        if i + 1 < b.len() && b[i + 1] == b'%' {
            out.push('%');
            i += 2;
            continue;
        }
        assert!(i + 1 < b.len() && b[i + 1] == b'(', "unsupported % at {}", i);
        let close = tmpl[i..].find(')').expect("unterminated %(") + i;
        let key = &tmpl[i + 2..close];
        let conv = b[close + 1];
        let v = vals
            .iter()
            .find(|(k, _)| *k == key)
            .unwrap_or_else(|| panic!("template key {:?} not supplied", key));
        match (conv, &v.1) {
            (b's', Val::Str(s)) => out.push_str(s),
            (b'd', Val::Int(n)) => out.push_str(&n.to_string()),
            (b'd', Val::Float(f)) => out.push_str(&(*f as i64).to_string()),
            (b'f', Val::Float(f)) => out.push_str(&format!("{:.6}", f)),
            _ => panic!("bad conversion %{} for key {}", conv as char, key),
        }
        i = close + 2;
    }
    out
}

/// CPython `repr(float)` / `json.dumps` float form.
///
/// Shortest round-tripping digits (Rust's `{:e}` and CPython's `repr` both
/// produce those), then CPython's LAYOUT rule, which Rust does not share:
/// with the value written as `0.<digits> * 10^decpt`, CPython uses the
/// exponent form when `decpt > 16 || decpt < -3` and the positional form
/// otherwise, always with a `.0` on an integral value and always with at least
/// two exponent digits.
///
/// ```text
/// python3           rust format!("{}")     here
/// 1584.0            1584                   1584.0
/// 1e+16             10000000000000000      1e+16
/// 1e-05             0.00001                1e-05
/// 0.0001            0.0001                 0.0001
/// ```
///
/// Rust never switches to an exponent, so the two disagree outside
/// 1e-4 .. 1e16. Nothing tmsite currently puts in a page reaches that far --
/// coordinates are hundreds of metres, rounded to one decimal -- but a port
/// that is only right on the data we happened to look at is not a port.
pub fn repr_f64(v: f64) -> String {
    if v.is_nan() {
        return "NaN".into();
    }
    if v.is_infinite() {
        return if v > 0.0 { "Infinity".into() } else { "-Infinity".into() };
    }
    let e = format!("{:e}", v); // shortest round-trip, always d[.ddd]e<exp>
    let (mant, exp) = e.split_once('e').expect("{:e} always has an exponent");
    let exp: i32 = exp.parse().expect("{:e} exponent is an integer");
    let sign = if mant.starts_with('-') { "-" } else { "" };
    let digits: String = mant.chars().filter(|c| c.is_ascii_digit()).collect();
    let decpt = exp + 1;
    if decpt > 16 || decpt < -3 {
        let mut m = digits[..1].to_string();
        if digits.len() > 1 {
            m.push('.');
            m.push_str(&digits[1..]);
        }
        format!(
            "{}{}e{}{:02}",
            sign,
            m,
            if exp < 0 { "-" } else { "+" },
            exp.abs()
        )
    } else if decpt <= 0 {
        format!("{}0.{}{}", sign, "0".repeat(-decpt as usize), digits)
    } else if decpt as usize >= digits.len() {
        format!(
            "{}{}{}.0",
            sign,
            digits,
            "0".repeat(decpt as usize - digits.len())
        )
    } else {
        format!("{}{}.{}", sign, &digits[..decpt as usize], &digits[decpt as usize..])
    }
}

/// CPython `round(x, nd)`: correctly rounded, ties to even.
pub fn round_nd(v: f64, nd: usize) -> f64 {
    format!("{:.*}", nd, v).parse().unwrap()
}

/// CPython `round(x)` -> integer, ties to even.
pub fn round_half_even(v: f64) -> f64 {
    let f = v.floor();
    let frac = v - f;
    if frac > 0.5 {
        f + 1.0
    } else if frac < 0.5 {
        f
    } else if (f / 2.0).floor() * 2.0 == f {
        f
    } else {
        f + 1.0
    }
}

/// `json.dumps` string escaping with the default `ensure_ascii=True`.
pub fn json_str(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c if (c as u32) < 0x7f => out.push(c),
            c => {
                let cp = c as u32;
                if cp > 0xFFFF {
                    let v = cp - 0x10000;
                    out.push_str(&format!(
                        "\\u{:04x}\\u{:04x}",
                        0xD800 + (v >> 10),
                        0xDC00 + (v & 0x3FF)
                    ));
                } else {
                    out.push_str(&format!("\\u{:04x}", cp));
                }
            }
        }
    }
    out.push('"');
    out
}

/// Standard base64 with padding (`base64.b64encode`).
pub fn b64(data: &[u8]) -> String {
    const A: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity((data.len() + 2) / 3 * 4);
    for c in data.chunks(3) {
        let b0 = c[0] as u32;
        let b1 = *c.get(1).unwrap_or(&0) as u32;
        let b2 = *c.get(2).unwrap_or(&0) as u32;
        let n = (b0 << 16) | (b1 << 8) | b2;
        out.push(A[(n >> 18) as usize & 63] as char);
        out.push(A[(n >> 12) as usize & 63] as char);
        out.push(if c.len() > 1 { A[(n >> 6) as usize & 63] as char } else { '=' });
        out.push(if c.len() > 2 { A[n as usize & 63] as char } else { '=' });
    }
    out
}

pub fn b64_decode(s: &str) -> Vec<u8> {
    let mut rev = [255u8; 256];
    const A: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    for (i, &c) in A.iter().enumerate() {
        rev[c as usize] = i as u8;
    }
    let mut out = Vec::with_capacity(s.len() / 4 * 3);
    let mut acc = 0u32;
    let mut n = 0;
    for &c in s.as_bytes() {
        if c == b'=' || rev[c as usize] == 255 {
            continue;
        }
        acc = (acc << 6) | rev[c as usize] as u32;
        n += 1;
        if n == 4 {
            out.push((acc >> 16) as u8);
            out.push((acc >> 8) as u8);
            out.push(acc as u8);
            acc = 0;
            n = 0;
        }
    }
    if n == 3 {
        out.push((acc >> 10) as u8);
        out.push((acc >> 2) as u8);
    } else if n == 2 {
        out.push((acc >> 4) as u8);
    }
    out
}

