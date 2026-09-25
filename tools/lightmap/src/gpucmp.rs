//! Bit-level comparison of a transcribed kernel's output against a captured render target (the
//! "closed row" accounting: bit-identical / within one quantum of the target format / beyond, and
//! the worst texel), shared by the `*-check` commands.

use crate::gpufmt::{decode_f16, encode_f16, Rounding};
use crate::passdiff::Buf;

/// The storage format the two buffers are compared in: the "quantum" of a value is one step of it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Fmt {
    /// IEEE half per channel (RGBA16F / R16F targets).
    F16,
    /// UF11 / UF11 / UF10 (R11G11B10_FLOAT).
    R11G11B10,
    /// k/255 per channel.
    Unorm8,
    /// Exact integers (R8G8_UINT ids …): no tolerance at all.
    Exact,
}

/// One step of `fmt` above `v` (the target's ulp at that value).
pub fn quantum(fmt: Fmt, channel: u32, v: f32) -> f32 {
    match fmt {
        Fmt::F16 => {
            let h = encode_f16(v, Rounding::NearestEven);
            let up = decode_f16(h.wrapping_add(1));
            (up - decode_f16(h)).abs().max(f32::MIN_POSITIVE)
        }
        Fmt::R11G11B10 => {
            let mbits = if channel == 2 { 5 } else { 6 };
            let q = crate::gpufmt::encode_unsigned(v.max(0.0), mbits, Rounding::NearestEven);
            let up = crate::gpufmt::decode_unsigned(q + 1, mbits);
            (up - crate::gpufmt::decode_unsigned(q, mbits)).abs().max(f32::MIN_POSITIVE)
        }
        Fmt::Unorm8 => 1.0 / 255.0,
        Fmt::Exact => 0.0,
    }
}

/// The comparison of one buffer pair.
#[derive(Clone, Debug, Default)]
pub struct Report {
    pub values: usize,
    pub exact: usize,
    pub ulp1: usize,
    pub beyond: usize,
    pub max_abs: f32,
    /// Where the worst value is: (x, y, channel, captured, ours).
    pub worst: (u32, u32, u32, f32, f32),
    /// Texels (all channels) that are not bit-identical.
    pub texels_off: usize,
    pub texels: usize,
    /// NaN/Inf on one side only.
    pub nonfinite: usize,
}

impl Report {
    pub fn line(&self) -> String {
        format!(
            "{} of {} values bit-identical, {} within 1 quantum, {} beyond (max |Δ| {:.6} at ({}, {}) ch {}: captured {:.6} ours {:.6}); {} of {} texels differ{}",
            self.exact,
            self.values,
            self.ulp1,
            self.beyond,
            self.max_abs,
            self.worst.0,
            self.worst.1,
            self.worst.2,
            self.worst.3,
            self.worst.4,
            self.texels_off,
            self.texels,
            if self.nonfinite > 0 { format!("; {} non-finite on one side", self.nonfinite) } else { String::new() }
        )
    }
    pub fn closed(&self) -> bool {
        self.beyond == 0 && self.nonfinite == 0
    }
}

/// Compare `ours` against the captured `theirs` over the first `channels` channels, in `fmt`.
pub fn compare(ours: &Buf, theirs: &Buf, channels: u32, fmt: Fmt) -> Report {
    compare_where(ours, theirs, channels, fmt, &|_, _| true)
}

/// As `compare`, over the texels `mask(x, y)` selects.
pub fn compare_where(ours: &Buf, theirs: &Buf, channels: u32, fmt: Fmt, mask: &dyn Fn(u32, u32) -> bool) -> Report {
    assert_eq!((ours.w, ours.h), (theirs.w, theirs.h), "buffer sizes differ");
    let ch = channels.min(ours.channels).min(theirs.channels);
    let mut r = Report::default();
    for y in 0..ours.h {
        for x in 0..ours.w {
            if !mask(x, y) {
                continue;
            }
            r.texels += 1;
            let mut off = false;
            for c in 0..ch {
                let (o, t) = (ours.get(x, y, c), theirs.get(x, y, c));
                r.values += 1;
                if o.to_bits() == t.to_bits() || (o == t) {
                    r.exact += 1;
                    continue;
                }
                off = true;
                if !o.is_finite() || !t.is_finite() {
                    r.nonfinite += 1;
                    continue;
                }
                let d = (o - t).abs();
                if d <= quantum(fmt, c, t) * 1.0001 && fmt != Fmt::Exact {
                    r.ulp1 += 1;
                } else {
                    r.beyond += 1;
                }
                if d > r.max_abs {
                    r.max_abs = d;
                    r.worst = (x, y, c, t, o);
                }
            }
            if off {
                r.texels_off += 1;
            }
        }
    }
    r
}

/// Print up to `n` differing texels (captured vs ours) — the look at a mismatch before the theory.
pub fn print_diffs(ours: &Buf, theirs: &Buf, channels: u32, n: usize) {
    let ch = channels.min(ours.channels).min(theirs.channels);
    let mut shown = 0;
    'outer: for y in 0..ours.h {
        for x in 0..ours.w {
            let mut off = false;
            for c in 0..ch {
                let (o, t) = (ours.get(x, y, c), theirs.get(x, y, c));
                if o != t && !(o.is_nan() && t.is_nan()) {
                    off = true;
                }
            }
            if off {
                let tv: Vec<String> = (0..ch).map(|c| format!("{:.6}", theirs.get(x, y, c))).collect();
                let ov: Vec<String> = (0..ch).map(|c| format!("{:.6}", ours.get(x, y, c))).collect();
                println!("    ({x}, {y}): captured [{}] ours [{}]", tv.join(", "), ov.join(", "));
                shown += 1;
                if shown >= n {
                    break 'outer;
                }
            }
        }
    }
}

/// How the GPU evaluates the DXBC `div` (D3D11 allows 1 ulp: most hardware multiplies by a reciprocal
/// instead of dividing). `MulRcp` = `a × rcp(b)` with the reciprocal correctly rounded to f32 — what the
/// capture's `w / w = 0.999512` texels (PS 25113, ROW 12) show: an exact division would give 1.0.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DivModel {
    Ieee,
    MulRcp,
}

#[inline]
pub fn div(a: f32, b: f32, m: DivModel) -> f32 {
    match m {
        DivModel::Ieee => a / b,
        DivModel::MulRcp => a * (1.0 / b),
    }
}
