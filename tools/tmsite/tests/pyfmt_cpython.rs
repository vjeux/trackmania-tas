//! The three CPython behaviours the page builder's byte-identity rests on.
//!
//! Each case here is one a NAIVE implementation gets wrong; the naive answer is
//! written next to it. Cases that both a naive and a correct implementation
//! pass are not evidence and are not here.

use tmsite::pyfmt::{b64, b64_decode, json_str, pyformat, repr_f64, round_half_even, round_nd, Val};

// ---------------------------------------------------------------- repr(float)

#[test]
fn repr_float_keeps_the_trailing_dot_zero() {
    // naive `format!("{}", v)`: "1584", "-0", "100"
    assert_eq!(repr_f64(1584.0), "1584.0");
    assert_eq!(repr_f64(-0.0), "-0.0");
    assert_eq!(repr_f64(0.0), "0.0");
    assert_eq!(repr_f64(100.0), "100.0");
}

#[test]
fn repr_float_is_the_shortest_round_trip_not_a_rounded_one() {
    // naive `format!("{:.17}", v)`: "0.30000000000000004" only by luck, and
    // "0.10000000000000001" for 0.1, which CPython prints as "0.1".
    assert_eq!(repr_f64(0.1), "0.1");
    assert_eq!(repr_f64(0.1 + 0.2), "0.30000000000000004");
    assert_eq!(repr_f64(18.002), "18.002");
    assert_eq!(repr_f64(1.0 / 3.0), "0.3333333333333333");
    for v in [0.1_f64, 1584.7, -0.30000000000000004, 1e-7, 9.87e21, f64::MAX] {
        let round_tripped: f64 = repr_f64(v).parse().unwrap();
        assert_eq!(round_tripped.to_bits(), v.to_bits(), "repr of {:?} does not round-trip", v);
    }
}

#[test]
fn repr_float_switches_to_an_exponent_where_cpython_does() {
    // Rust's `{}` NEVER uses an exponent, so every one of these is a fork
    // between the port and the Python it claims to reproduce.
    //   python3: repr(1e16) -> '1e+16'   rust "{}" -> 10000000000000000
    assert_eq!(repr_f64(1e16), "1e+16");
    assert_eq!(repr_f64(1e15), "1000000000000000.0"); // the boundary stays positional
    assert_eq!(repr_f64(1e-5), "1e-05"); // two exponent digits, like CPython
    assert_eq!(repr_f64(1e-4), "0.0001"); // the boundary stays positional
    assert_eq!(repr_f64(-2.5e-7), "-2.5e-07");
    assert_eq!(repr_f64(1.7976931348623157e308), "1.7976931348623157e+308");
    assert_eq!(repr_f64(5e-324), "5e-324");
    assert_eq!(repr_f64(f64::NAN), "NaN"); // json.dumps' non-JSON spelling
    assert_eq!(repr_f64(f64::INFINITY), "Infinity");
    assert_eq!(repr_f64(f64::NEG_INFINITY), "-Infinity");
}

// ------------------------------------------------------------ round(x, n)

#[test]
fn round_nd_rounds_against_the_exact_binary_value() {
    // 2.675 is really 2.67499999999999982236431605997495353221893310546875, so
    // CPython gives 2.67. The naive `(x*100).round()/100` computes 267.5 ->
    // 268 -> 2.68 and is wrong.
    assert_eq!(round_nd(2.675, 2), 2.67);
    assert_eq!((2.675_f64 * 100.0).round() / 100.0, 2.68); // the naive answer
    // 0.045 is really 0.04499999999999999833... -> 0.04, naive says 0.05
    assert_eq!(round_nd(0.045, 2), 0.04);
    assert_eq!((0.045_f64 * 100.0).round() / 100.0, 0.05);
    // 0.35 is really 0.34999999999999997779... -> 0.3, naive says 0.4. This one
    // is at ONE decimal, which is exactly what the page builder rounds to.
    assert_eq!(round_nd(0.35, 1), 0.3);
    assert_eq!((0.35_f64 * 10.0).round() / 10.0, 0.4);
    // 0.125 IS exactly representable, so it is a true tie and goes to EVEN:
    // 0.12, where Rust's own `.round()` (half away from zero) says 0.13.
    assert_eq!(round_nd(0.125, 2), 0.12);
    assert_eq!((0.125_f64 * 100.0).round() / 100.0, 0.13);
    assert_eq!(round_nd(0.25, 1), 0.2);
    assert_eq!(round_nd(0.75, 1), 0.8);
    assert_eq!(round_nd(1584.0001, 1), 1584.0);
    assert_eq!(round_nd(-0.25, 1), -0.2);
}

#[test]
fn round_half_even_is_not_round_half_up() {
    // naive `v.round()` (Rust) rounds halves AWAY FROM ZERO: 0.5 -> 1, 2.5 -> 3
    assert_eq!(round_half_even(0.5), 0.0);
    assert_eq!(round_half_even(1.5), 2.0);
    assert_eq!(round_half_even(2.5), 2.0);
    assert_eq!(round_half_even(3.5), 4.0);
    assert_eq!(round_half_even(-0.5), 0.0);
    assert_eq!(round_half_even(-1.5), -2.0);
    assert_eq!(round_half_even(-2.5), -2.0);
    // non-ties are unaffected
    assert_eq!(round_half_even(2.4999999999999996), 2.0);
    assert_eq!(round_half_even(2.5000000000000004), 3.0);
    // and it is used on packing arithmetic, so integral inputs must not drift
    assert_eq!(round_half_even(4096.0), 4096.0);
}

// ------------------------------------------------------- %-formatting a dict

#[test]
fn pyformat_handles_percent_percent_and_typed_conversions() {
    // A naive `tmpl.replace("%(n)d", ...)` chain leaves `%%` alone -- and the
    // page templates carry `%%` in their JavaScript ("50%%" width strings), so
    // the escape is load-bearing.
    let t = "a %(n)d b %% c %(s)s d %(f)f e %(n)d";
    let got = pyformat(
        t,
        &[
            ("n", Val::Int(3)),
            ("s", Val::Str("x".into())),
            ("f", Val::Float(984.3141)),
        ],
    );
    assert_eq!(got, "a 3 b % c x d 984.314100 e 3");
}

#[test]
fn pyformat_percent_d_truncates_a_float_like_python() {
    // python3: "%(v)d" % {"v": 500.9} -> "500"   (int(), not round())
    let got = pyformat("%(v)d", &[("v", Val::Float(500.9))]);
    assert_eq!(got, "500");
}

#[test]
#[should_panic(expected = "not supplied")]
fn pyformat_refuses_a_missing_key() {
    // CPython raises KeyError; silently leaving the placeholder in the page
    // would ship a broken page that still looks built.
    pyformat("%(missing)s", &[("n", Val::Int(1))]);
}

// ---------------------------------------------------------- json.dumps bits

#[test]
fn json_str_escapes_like_ensure_ascii() {
    // A naive Rust escaper passes UTF-8 through; CPython's json.dumps default
    // is ensure_ascii=True, and a run name with an accent in it is normal.
    assert_eq!(json_str("plain"), "\"plain\"");
    assert_eq!(json_str("a\"b\\c"), "\"a\\\"b\\\\c\"");
    assert_eq!(json_str("é"), "\"\\u00e9\"");
    assert_eq!(json_str("\n\t"), "\"\\n\\t\"");
    assert_eq!(json_str("\u{1}"), "\"\\u0001\"");
    // astral plane -> surrogate pair, exactly as CPython writes it
    assert_eq!(json_str("🏁"), "\"\\ud83c\\udfc1\"");
}

#[test]
fn base64_matches_the_python_encoder() {
    assert_eq!(b64(b"any carnal pleasure."), "YW55IGNhcm5hbCBwbGVhc3VyZS4=");
    assert_eq!(b64(b"a"), "YQ==");
    assert_eq!(b64(b"ab"), "YWI=");
    assert_eq!(b64(b""), "");
    assert_eq!(b64_decode(&b64(&[0u8, 1, 2, 250, 255])), vec![0u8, 1, 2, 250, 255]);
    // the compact page's blob is 6 bytes per sample: exercise a length that is
    // not a multiple of 3 in both directions
    let blob: Vec<u8> = (0..=255u8).cycle().take(6 * 7).collect();
    assert_eq!(b64_decode(&b64(&blob)), blob);
}
