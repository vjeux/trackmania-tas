//! GPU storage formats the lightmapper's render targets use, bit-exact: `R11G11B10_FLOAT` (the
//! peel colour / ILightDir / LM-space HDR targets — RE child 3's format pin, client-re/NOTES.md
//! "RENDER FORMATS"), IEEE `R16_FLOAT` (the RGBA16F accumulation target) and the D3D11 float32 →
//! small-float conversion rules. A value written into such a target comes back quantised; the port
//! accumulates in f32, so every pass whose game target is one of these must be quantised at the
//! same point before it is compared (or accumulated) — `passdiff` treats the quantiser as a named
//! convention transform, so a mismatch of rounding is reported apart from a mismatch of light.
//!
//! UF11 = 5-bit exponent (bias 15), 6-bit mantissa, no sign; UF10 = 5-bit exponent, 5-bit mantissa.
//! Denormals are kept (exponent 0 → mantissa · 2^−14 / 2^bits), exponent 31 = Inf/NaN, negative
//! inputs clamp to 0, values above the largest finite become +Inf (D3D11 3.2.2 / DirectXMath
//! `XMStoreFloat3PK`). Rounding: round-to-nearest-even by default (what the hardware conversion
//! does on every D3D11 GPU we know; `Rounding::Truncate` is the other rule the spec permits).

/// Which of the two float→small-float rounding rules the D3D11 spec permits.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Rounding {
    NearestEven,
    Truncate,
}

/// Encode one f32 into an unsigned small float of `ebits` = 5 exponent bits and `mbits` mantissa bits
/// (6 → UF11, 5 → UF10); returns the bit pattern in the low `5 + mbits` bits.
pub fn encode_unsigned(v: f32, mbits: u32, r: Rounding) -> u32 {
    let bits = v.to_bits();
    let mask = (1u32 << (5 + mbits)) - 1;
    let inf = 0x1fu32 << mbits;
    if bits & 0x7f80_0000 == 0x7f80_0000 {
        // Inf or NaN
        return if bits & 0x007f_ffff != 0 { mask } else if bits & 0x8000_0000 != 0 { 0 } else { inf };
    }
    if bits & 0x8000_0000 != 0 {
        return 0; // negative (and −0) → 0
    }
    let shift = 23 - mbits; // dropped mantissa bits
    // the largest finite: exponent 30, all mantissa bits set → anything that rounds to exponent 31 is Inf
    let max_finite = (0x8e_u32 << 23) | (((1u32 << mbits) - 1) << shift); // 2^15·(2 − 2^−mbits)
    let mut i = bits;
    if i < 0x3880_0000 {
        // below 2^−14: a denormal of the small format — shift the explicit-leading-one mantissa right
        let sh = 113 - (i >> 23);
        i = (0x0080_0000 | (i & 0x007f_ffff)) >> sh.min(31);
    } else {
        // rebias the exponent 127 → 15 (subtract 112 << 23)
        i = i.wrapping_add(0xc800_0000);
    }
    let _ = max_finite;
    let q = match r {
        Rounding::NearestEven => {
            // round to nearest, ties to even: add half an lsb minus one plus the lsb itself (DirectXMath's
            // `I + 0xFFFF + ((I >> 17) & 1)` for the 17 dropped bits)
            let half = (1u32 << (shift - 1)) - 1;
            (i.wrapping_add(half).wrapping_add((i >> shift) & 1)) >> shift
        }
        Rounding::Truncate => i >> shift,
    };
    // overflow of the exponent field means Inf
    if q >= inf { inf } else { q & mask }
}

/// Decode an unsigned small float (5 exponent bits, `mbits` mantissa bits) into f32 — the value exactly (every
/// small-float value is an f32): a normal `(1 + m/2^mbits) · 2^(e−15)` is the f32 with exponent field e − 15 + 127 and
/// mantissa m << (23 − mbits); a denormal `m · 2^(−14−mbits)` is `m as f32` (exact) times that power of two (exact).
/// `decode_unsigned_slow` is the arithmetic form the port used until perf 8 (`2f32.powi` = a libgcc loop, 2 % of a
/// bake); `decode_bit_exact_over_every_pattern` proves the two equal on every UF11 / UF10 pattern.
#[inline]
pub fn decode_unsigned(q: u32, mbits: u32) -> f32 {
    let e = (q >> mbits) & 0x1f;
    let m = q & ((1u32 << mbits) - 1);
    if e == 31 {
        return if m != 0 { f32::NAN } else { f32::INFINITY };
    }
    if e == 0 {
        // m · 2^(−14 − mbits), both factors exact
        m as f32 * pow2(-14 - mbits as i32)
    } else {
        f32::from_bits(((e + 127 - 15) << 23) | (m << (23 - mbits)))
    }
}

/// The arithmetic form of `decode_unsigned` (kept for the exhaustive equality test).
pub fn decode_unsigned_slow(q: u32, mbits: u32) -> f32 {
    let e = (q >> mbits) & 0x1f;
    let m = q & ((1u32 << mbits) - 1);
    if e == 31 {
        return if m != 0 { f32::NAN } else { f32::INFINITY };
    }
    let scale = 1.0 / (1u32 << mbits) as f32;
    // (2^k as its bit pattern: exactly what `2f32.powi(k)` returns for these k — the powers of two are exact —
    // without the call; the decode runs per fragment channel in the layer derivation)
    #[inline(always)]
    fn pow2(k: i32) -> f32 {
        f32::from_bits(((k + 127) as u32) << 23)
    }
    if e == 0 {
        m as f32 * scale * pow2(-14)
    } else {
        (1.0 + m as f32 * scale) * pow2(e as i32 - 15)
    }
}

/// 2^k as an f32, exactly, for the normal range −126 ≤ k ≤ 127 (what `2f32.powi(k)` returns there, without its loop).
#[inline]
pub fn pow2(k: i32) -> f32 {
    debug_assert!((-126..=127).contains(&k));
    f32::from_bits(((k + 127) as u32) << 23)
}

/// Pack a linear RGB triple into `DXGI_FORMAT_R11G11B10_FLOAT` (R in bits 0–10, G 11–21, B 22–31).
pub fn pack_r11g11b10(rgb: [f32; 3], r: Rounding) -> u32 {
    encode_unsigned(rgb[0], 6, r) | (encode_unsigned(rgb[1], 6, r) << 11) | (encode_unsigned(rgb[2], 5, r) << 22)
}

/// Unpack `DXGI_FORMAT_R11G11B10_FLOAT`.
pub fn unpack_r11g11b10(v: u32) -> [f32; 3] {
    [decode_unsigned(v & 0x7ff, 6), decode_unsigned((v >> 11) & 0x7ff, 6), decode_unsigned(v >> 22, 5)]
}

/// The value an R11G11B10 target holds after `rgb` is written to it.
pub fn quantise_r11g11b10(rgb: [f32; 3], r: Rounding) -> [f32; 3] {
    unpack_r11g11b10(pack_r11g11b10(rgb, r))
}

/// IEEE binary16 encode (sign, 5-bit exponent, 10-bit mantissa) with the D3D11 rules: NaN → NaN,
/// overflow → ±Inf, denormals kept, rounding per `r`.
pub fn encode_f16(v: f32, r: Rounding) -> u16 {
    let bits = v.to_bits();
    let sign = ((bits >> 16) & 0x8000) as u16;
    let mag = bits & 0x7fff_ffff;
    if mag >= 0x7f80_0000 {
        return sign | if mag & 0x007f_ffff != 0 { 0x7e00 } else { 0x7c00 };
    }
    let mut i = mag;
    let shift = 13u32;
    if i < 0x3880_0000 {
        let sh = 113 - (i >> 23);
        i = (0x0080_0000 | (i & 0x007f_ffff)) >> sh.min(31);
    } else {
        i = i.wrapping_add(0xc800_0000);
    }
    let q = match r {
        Rounding::NearestEven => (i.wrapping_add(0xfff).wrapping_add((i >> shift) & 1)) >> shift,
        Rounding::Truncate => i >> shift,
    };
    let q = if q >= 0x7c00 { 0x7c00 } else { q };
    sign | q as u16
}

/// IEEE binary16 decode — the value exactly (every binary16 value is an f32): the sign bit moved up, a normal's
/// exponent rebiased 15 → 127 and its mantissa shifted to 23 bits; a denormal `±m · 2^−24` as `m as f32` (exact) times
/// 2^−24 (exact), the sign applied last as the arithmetic form does. `decode_f16_slow` is that arithmetic form (the port's
/// until perf 8); `decode_bit_exact_over_every_pattern` proves them equal on all 65 536 patterns (NaN → NaN both).
#[inline]
pub fn decode_f16(h: u16) -> f32 {
    let e = ((h >> 10) & 0x1f) as u32;
    let m = (h & 0x3ff) as u32;
    let sign = ((h as u32) & 0x8000) << 16;
    if e == 31 {
        return if m != 0 { f32::NAN } else { f32::from_bits(sign | 0x7f80_0000) };
    }
    if e == 0 {
        let v = m as f32 * pow2(-24);
        // (the arithmetic form multiplies by ±1.0: a −0.0 for m = 0 with the sign set, as here)
        f32::from_bits(v.to_bits() | sign)
    } else {
        f32::from_bits(sign | ((e + 127 - 15) << 23) | (m << 13))
    }
}

/// The arithmetic form of `decode_f16` (kept for the exhaustive equality test).
pub fn decode_f16_slow(h: u16) -> f32 {
    let sign = if h & 0x8000 != 0 { -1.0 } else { 1.0 };
    let e = ((h >> 10) & 0x1f) as i32;
    let m = (h & 0x3ff) as f32;
    if e == 31 {
        return if m != 0.0 { f32::NAN } else { sign * f32::INFINITY };
    }
    if e == 0 {
        sign * m / 1024.0 * 2f32.powi(-14)
    } else {
        sign * (1.0 + m / 1024.0) * 2f32.powi(e - 15)
    }
}

/// The value an R16_FLOAT channel holds after `v` is written to it.
pub fn quantise_f16(v: f32, r: Rounding) -> f32 {
    decode_f16(encode_f16(v, r))
}

/// A render target's storage format, as the quantiser the port applies at the write.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Quant {
    /// f32 (no quantisation).
    None,
    R11G11B10,
    F16,
}

impl Quant {
    pub fn parse(s: &str) -> Option<Quant> {
        match s.to_ascii_lowercase().as_str() {
            "none" | "f32" | "r32g32b32_float" | "r32g32b32a32_float" | "r32_float" => Some(Quant::None),
            "r11g11b10" | "r11g11b10_float" | "r11" => Some(Quant::R11G11B10),
            "f16" | "r16g16b16a16_float" | "r16_float" | "half" => Some(Quant::F16),
            _ => None,
        }
    }
    /// The DXGI name the manifest records for an RGB value stored in this format.
    pub fn dxgi_rgb(self) -> &'static str {
        match self {
            Quant::None => "R32G32B32_FLOAT",
            Quant::R11G11B10 => "R11G11B10_FLOAT",
            Quant::F16 => "R16G16B16A16_FLOAT",
        }
    }
    #[inline]
    pub fn apply(self, rgb: [f32; 3], r: Rounding) -> [f32; 3] {
        match self {
            Quant::None => rgb,
            Quant::R11G11B10 => quantise_r11g11b10(rgb, r),
            Quant::F16 => [quantise_f16(rgb[0], r), quantise_f16(rgb[1], r), quantise_f16(rgb[2], r)],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decode_bit_exact_over_every_pattern() {
        // the fast decoders against the arithmetic forms, every bit pattern (NaN matched as NaN)
        for h in 0..=u16::MAX {
            let (a, b) = (decode_f16(h), decode_f16_slow(h));
            assert!(a.to_bits() == b.to_bits() || (a.is_nan() && b.is_nan()), "f16 {h:#06x}: {a:?} vs {b:?}");
        }
        for mbits in [5u32, 6] {
            for q in 0..(1u32 << (5 + mbits)) {
                let (a, b) = (decode_unsigned(q, mbits), decode_unsigned_slow(q, mbits));
                assert!(a.to_bits() == b.to_bits() || (a.is_nan() && b.is_nan()), "UF{} {q:#x}: {a:?} vs {b:?}", 5 + mbits);
            }
        }
        for k in -126..=127 {
            assert_eq!(pow2(k).to_bits(), 2f32.powi(k).to_bits(), "2^{k}");
        }
    }

    #[test]
    fn uf11_exact_values_round_trip() {
        // powers of two and short mantissas are exact in UF11 (6 mantissa bits) and UF10 (5 bits)
        for v in [0.0f32, 1.0, 2.0, 0.5, 0.25, 1.5, 1.75, 3.0, 65024.0, 2f32.powi(-14), 2f32.powi(-20)] {
            let q = encode_unsigned(v, 6, Rounding::NearestEven);
            assert_eq!(decode_unsigned(q, 6), v, "UF11 {v}");
            // UF10 has one mantissa bit fewer: its smallest denormal is 2^−19, its largest finite 64512
            if v >= 2f32.powi(-19) || v == 0.0 {
                let q10 = encode_unsigned(v.min(64512.0), 5, Rounding::NearestEven);
                assert_eq!(decode_unsigned(q10, 5), v.min(64512.0), "UF10 {v}");
            }
        }
        assert_eq!(encode_unsigned(2f32.powi(-20), 5, Rounding::NearestEven), 0, "half the smallest UF10 denormal rounds to even = 0");
    }

    #[test]
    fn uf11_rounding_rules() {
        // 1 + 1/128 sits exactly halfway between 1 and 1 + 1/64: nearest-even → 1.0 (even mantissa 0), truncate → 1.0
        let v = 1.0 + 1.0 / 128.0;
        assert_eq!(decode_unsigned(encode_unsigned(v, 6, Rounding::NearestEven), 6), 1.0);
        assert_eq!(decode_unsigned(encode_unsigned(v, 6, Rounding::Truncate), 6), 1.0);
        // 1 + 3/128 is halfway between 1 + 1/64 (odd) and 1 + 2/64 (even): nearest-even → 1 + 2/64, truncate → 1 + 1/64
        let v = 1.0 + 3.0 / 128.0;
        assert_eq!(decode_unsigned(encode_unsigned(v, 6, Rounding::NearestEven), 6), 1.0 + 2.0 / 64.0);
        assert_eq!(decode_unsigned(encode_unsigned(v, 6, Rounding::Truncate), 6), 1.0 + 1.0 / 64.0);
        // just above the midpoint rounds up under both nearest rules, not under truncation
        let v = 1.0 + 1.0 / 128.0 + 1e-4;
        assert_eq!(decode_unsigned(encode_unsigned(v, 6, Rounding::NearestEven), 6), 1.0 + 1.0 / 64.0);
        assert_eq!(decode_unsigned(encode_unsigned(v, 6, Rounding::Truncate), 6), 1.0);
    }

    #[test]
    fn uf11_special_values() {
        assert_eq!(encode_unsigned(-1.0, 6, Rounding::NearestEven), 0, "negative clamps to 0");
        assert_eq!(encode_unsigned(-0.0, 6, Rounding::NearestEven), 0);
        assert_eq!(encode_unsigned(f32::INFINITY, 6, Rounding::NearestEven), 0x7c0);
        assert!(decode_unsigned(encode_unsigned(f32::NAN, 6, Rounding::NearestEven), 6).is_nan());
        assert_eq!(encode_unsigned(70000.0, 6, Rounding::NearestEven), 0x7c0, "above the largest finite → Inf");
        assert_eq!(encode_unsigned(65024.0, 6, Rounding::NearestEven), 0x7bf, "the largest finite UF11");
        assert_eq!(decode_unsigned(0x7bf, 6), 65024.0);
        assert_eq!(decode_unsigned(0x3df, 5), 64512.0, "the largest finite UF10");
        // denormals: the smallest UF11 step is 2^−14/64
        assert_eq!(decode_unsigned(1, 6), 2f32.powi(-14) / 64.0);
        assert_eq!(encode_unsigned(2f32.powi(-14) / 64.0, 6, Rounding::NearestEven), 1);
    }

    #[test]
    fn r11g11b10_packing_and_precision() {
        let c = [0.3f32, 1.7, 12.5];
        let q = quantise_r11g11b10(c, Rounding::NearestEven);
        // relative error bounded by half a mantissa step: 2^−7 (R, G), 2^−6 (B)
        assert!((q[0] - c[0]).abs() <= c[0] * 2f32.powi(-7));
        assert!((q[1] - c[1]).abs() <= c[1] * 2f32.powi(-7));
        assert!((q[2] - c[2]).abs() <= c[2] * 2f32.powi(-6));
        // channel placement: R low bits, B high bits
        let p = pack_r11g11b10([1.0, 0.0, 0.0], Rounding::NearestEven);
        assert_eq!(p, 0x3c0, "1.0 in UF11 = exponent 15, mantissa 0");
        let p = pack_r11g11b10([0.0, 0.0, 1.0], Rounding::NearestEven);
        assert_eq!(p, 0x1e0 << 22);
        assert_eq!(unpack_r11g11b10(pack_r11g11b10([1.0, 2.0, 4.0], Rounding::NearestEven)), [1.0, 2.0, 4.0]);
    }

    #[test]
    fn f16_round_trip_and_rounding() {
        for v in [0.0f32, -0.0, 1.0, -2.0, 0.333251953125, 65504.0, 2f32.powi(-24), -2f32.powi(-14)] {
            let h = encode_f16(v, Rounding::NearestEven);
            assert_eq!(decode_f16(h).to_bits(), v.to_bits(), "{v}");
        }
        assert_eq!(encode_f16(1.0, Rounding::NearestEven), 0x3c00);
        assert_eq!(encode_f16(-1.0, Rounding::NearestEven), 0xbc00);
        assert_eq!(encode_f16(70000.0, Rounding::NearestEven), 0x7c00, "overflow → +Inf");
        assert!(decode_f16(encode_f16(f32::NAN, Rounding::NearestEven)).is_nan());
        // 1 + 1/2048 is halfway between 1 and 1 + 1/1024 → even (1.0); 1 + 3/2048 → 1 + 2/1024
        assert_eq!(quantise_f16(1.0 + 1.0 / 2048.0, Rounding::NearestEven), 1.0);
        assert_eq!(quantise_f16(1.0 + 3.0 / 2048.0, Rounding::NearestEven), 1.0 + 2.0 / 1024.0);
        assert_eq!(quantise_f16(1.0 + 3.0 / 2048.0, Rounding::Truncate), 1.0 + 1.0 / 1024.0);
    }

    #[test]
    fn accumulating_in_a_small_float_loses_the_tail() {
        // the reason the accumulation target cannot be R11G11B10: 256 equal adds of 1/256 into a
        // running sum lose every add smaller than half an ulp of the sum
        let mut s11 = 0.0f32;
        let mut s16 = 0.0f32;
        for _ in 0..256 {
            s11 = quantise_r11g11b10([s11 + 1.0 / 256.0, 0.0, 0.0], Rounding::NearestEven)[0];
            s16 = quantise_f16(s16 + 1.0 / 256.0, Rounding::NearestEven);
        }
        assert!(s11 < 0.6, "UF11 accumulation stalls: {s11}");
        assert!((s16 - 1.0).abs() < 1e-3, "f16 accumulation of 256 terms stays exact: {s16}");
    }
}

/// The sRGB transfer curve of a `_UNORM_SRGB` render-target store (D3D11 3.2.3 / IEC 61966-2-1): linear → encoded.
pub fn linear_to_srgb(v: f32) -> f32 {
    if v.is_nan() { return 0.0; }
    let v = v.clamp(0.0, 1.0);
    if v <= 0.0031308 { 12.92 * v } else { 1.055 * v.powf(1.0 / 2.4) - 0.055 }
}

/// The inverse curve: what a `ld`/`sample` of a `_UNORM_SRGB` view returns for the stored byte's k/255.
pub fn srgb_to_linear(v: f32) -> f32 {
    if v <= 0.04045 { v / 12.92 } else { ((v + 0.055) / 1.055).powf(2.4) }
}

/// One f16 ulp at magnitude `v` (the spacing of the half-precision grid there; the subnormal step 2^-24 below 2^-14).
pub fn f16_ulp(v: f32) -> f32 {
    let a = v.abs();
    if a < 6.103_515_6e-5 {
        return 5.960_464_5e-8;
    }
    let e = a.log2().floor() as i32;
    2f32.powi(e - 10)
}

#[cfg(test)]
mod pow2_probe {
    use super::*;
    #[test]
    fn decode_unsigned_matches_the_powi_form() {
        for mbits in [5u32, 6] {
            for q in 0..(1u32 << (5 + mbits)) {
                let e = (q >> mbits) & 0x1f;
                let m = q & ((1u32 << mbits) - 1);
                let scale = 1.0 / (1u32 << mbits) as f32;
                let reference = if e == 31 { if m != 0 { f32::NAN } else { f32::INFINITY } } else if e == 0 { m as f32 * scale * 2f32.powi(-14) } else { (1.0 + m as f32 * scale) * 2f32.powi(e as i32 - 15) };
                let got = decode_unsigned(q, mbits);
                assert!(got.to_bits() == reference.to_bits() || (got.is_nan() && reference.is_nan()), "q {q} mbits {mbits}: {got} vs {reference}");
            }
        }
    }
}
