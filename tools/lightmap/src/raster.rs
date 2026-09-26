//! A software triangle rasteriser with Direct3D 11 rules — the lightmapper's passes are GPU rasters
//! (the chart raster into the supersampled atlas, the orthographic depth peels per dome direction), so
//! the port reproduces the GPU's sampling: pixel centres at (x + 0.5, y + 0.5), the top-left fill rule
//! on shared edges (a pixel centre exactly on an edge belongs to the triangle whose top or left edge it
//! is, so adjacent triangles never double-cover or leave a gap), no culling unless asked, linear
//! (orthographic) attribute interpolation through the barycentrics.
//!
//! Coordinates are in pixels of the target, y down (D3D's viewport convention); a triangle's pixels are
//! delivered to a closure as (x, y, barycentrics w.r.t. the vertices as given). Fixed-point sub-pixel
//! precision (the GPU snaps vertices to 1/256 pixel) is not modelled — only a pixel centre exactly on an
//! edge is affected, and the fill rule below makes that case deterministic in the same way.

/// The barycentric weights of a point, as (w0, w1, w2) with respect to the triangle's vertices.
pub type Bary = [f32; 3];

/// A rasterised triangle's pixel bounds, clipped to the target.
#[inline(always)]
fn bounds(p: &[[f32; 2]; 3], w: u32, h: u32) -> Option<(i32, i32, i32, i32)> {
    let minx = p.iter().map(|q| q[0]).fold(f32::MAX, f32::min);
    let maxx = p.iter().map(|q| q[0]).fold(f32::MIN, f32::max);
    let miny = p.iter().map(|q| q[1]).fold(f32::MAX, f32::min);
    let maxy = p.iter().map(|q| q[1]).fold(f32::MIN, f32::max);
    if !(minx.is_finite() && maxx.is_finite() && miny.is_finite() && maxy.is_finite()) {
        return None;
    }
    // pixel centres x + 0.5 in [minx, maxx] ⇒ x in [ceil(minx − 0.5), floor(maxx − 0.5)]
    let x0 = ((minx - 0.5).ceil() as i64).max(0) as i32;
    let x1 = ((maxx - 0.5).floor() as i64).min(w as i64 - 1) as i32;
    let y0 = ((miny - 0.5).ceil() as i64).max(0) as i32;
    let y1 = ((maxy - 0.5).floor() as i64).min(h as i64 - 1) as i32;
    if x0 > x1 || y0 > y1 {
        return None;
    }
    Some((x0, x1, y0, y1))
}

/// Is the directed edge a→b a "top" or "left" edge of a triangle wound so that the inside is on the
/// positive side of `edge()` (D3D: y down)? Top: horizontal with the inside below; left: going up.
#[inline(always)]
fn top_left(a: [f32; 2], b: [f32; 2]) -> bool {
    let dy = b[1] - a[1];
    let dx = b[0] - a[0];
    // with inside = positive edge function and y down, the inside lies to the RIGHT of the direction of
    // travel (edge(a,b,q) > 0 for q below a rightward a→b): a top edge runs right (dy == 0, dx > 0) with
    // the triangle below it, a left edge runs UP the screen (dy < 0) with the triangle to its right
    dy < 0.0 || (dy == 0.0 && dx > 0.0)
}

/// The edge function: positive when p is to the left of a→b (in a y-up frame) — the sign is what
/// matters; `triangle()` orients the triangle so inside is positive.
#[inline(always)]
fn edge(a: [f32; 2], b: [f32; 2], p: [f32; 2]) -> f32 {
    (b[0] - a[0]) * (p[1] - a[1]) - (b[1] - a[1]) * (p[0] - a[0])
}

/// Rasterise one triangle over a w×h target, calling `f(x, y, bary)` for every pixel whose centre it
/// covers under the top-left rule. Degenerate (zero-area) triangles produce nothing. Both windings are
/// accepted (the sign is normalised); `f` receives barycentrics in the vertex order given.
pub fn triangle<F: FnMut(u32, u32, Bary)>(w: u32, h: u32, p: [[f32; 2]; 3], f: F) {
    triangle_clipped(w, h, p, (0, 0, w as i32 - 1, h as i32 - 1), f)
}

/// How many pixels `triangle_clipped_masked` would test for `p` (its bounding box within the clip) — stats.
pub fn bbox_pixels(p: [[f32; 2]; 3], w: u32, h: u32, clip: (i32, i32, i32, i32)) -> u64 {
    let Some((x0, x1, y0, y1)) = bounds(&p, w, h) else { return 0 };
    let (x0, x1, y0, y1) = (x0.max(clip.0), x1.min(clip.2), y0.max(clip.1), y1.min(clip.3));
    if x0 > x1 || y0 > y1 { return 0; }
    (x1 - x0 + 1) as u64 * (y1 - y0 + 1) as u64
}

/// Whether `triangle` would visit pixel (x, y) for `p`: the same orientation, edge functions and
/// top-left rule (the lazy dome raster asks per pixel instead of filling the frame).
#[inline]
pub fn covers(p: [[f32; 2]; 3], x: u32, y: u32) -> bool {
    match CoverTest::new(p) { Some(c) => c.covers(x, y), None => false }
}

/// Per box width w (index; 0 unused) the lane offsets (l mod w, l div w) of `CoverTest::covers_box`'s lanes.
static LANE_OFFSETS: [([i32; 16], [i32; 16]); 17] = {
    let mut t = [([0i32; 16], [0i32; 16]); 17];
    let mut w = 1;
    while w <= 16 {
        let mut l = 0;
        while l < 16 {
            t[w].0[l] = (l % w) as i32;
            t[w].1[l] = (l / w) as i32;
            l += 1;
        }
        w += 1;
    }
    t
};

/// `covers` with the per-triangle part (the orientation and the top-left flags) done once: the binning's exact
/// cull between pixel centres asks the same question at every candidate centre of a small triangle. Bit-for-bit
/// the raster's decision: `edge` on the same oriented vertices, the same inside predicate.
#[derive(Clone, Copy)]
pub struct CoverTest {
    a: [f32; 2],
    b: [f32; 2],
    c: [f32; 2],
    tl: [bool; 3],
}

impl CoverTest {
    /// None for a degenerate (zero-area or non-finite) triangle — the raster visits nothing for it.
    #[inline(always)]
    pub fn new(p: [[f32; 2]; 3]) -> Option<CoverTest> {
        let area = edge(p[0], p[1], p[2]);
        if area == 0.0 || !area.is_finite() {
            return None;
        }
        let (a, b, c) = if area > 0.0 { (p[0], p[1], p[2]) } else { (p[0], p[2], p[1]) };
        Some(CoverTest { a, b, c, tl: [top_left(a, b), top_left(b, c), top_left(c, a)] })
    }
    #[inline(always)]
    pub fn covers(&self, x: u32, y: u32) -> bool {
        let q = [x as f32 + 0.5, y as f32 + 0.5];
        let e0 = edge(self.a, self.b, q);
        let e1 = edge(self.b, self.c, q);
        let e2 = edge(self.c, self.a, q);
        (e0 > 0.0 || (e0 == 0.0 && self.tl[0])) && (e1 > 0.0 || (e1 == 0.0 && self.tl[1])) && (e2 > 0.0 || (e2 == 0.0 && self.tl[2]))
    }

    /// The coverage of the pixel centres of the box [x0, x1] × [y0, y1] (at most 16 of them), row-major: bit
    /// (y − y0)·w + (x − x0) set when the centre is covered — the sixteen tests in one 16-lane pass on the
    /// AVX-512 build (each lane the scalar `edge` operations at its own centre), scalar otherwise.
    #[inline(always)]
    pub fn covers_box(&self, x0: i32, y0: i32, x1: i32, y1: i32) -> u16 {
        let w = (x1 - x0 + 1).max(0) as usize;
        let h = (y1 - y0 + 1).max(0) as usize;
        let n = w * h;
        debug_assert!(n <= 16);
        if n == 0 { return 0; }
        #[cfg(all(target_arch = "x86_64", target_feature = "avx512f", target_feature = "avx512bw"))]
        {
            // SAFETY: the build enables avx512f/bw for every function
            return unsafe { self.covers_box_avx512(x0, y0, w, h) };
        }
        #[cfg(not(all(target_arch = "x86_64", target_feature = "avx512f", target_feature = "avx512bw")))]
        {
            let mut m = 0u16;
            for j in 0..h {
                for i in 0..w {
                    if self.covers((x0 + i as i32) as u32, (y0 + j as i32) as u32) { m |= 1 << (j * w + i); }
                }
            }
            m
        }
    }

    #[cfg(target_arch = "x86_64")]
    // (no target_feature attribute: under the avx512 cfg the build has the features everywhere, and the attribute
    // kept LLVM from inlining this into the binning — three calls per micro triangle)
    #[cfg_attr(all(target_feature = "avx512f", target_feature = "avx512bw"), inline(always))]
    #[cfg_attr(not(all(target_feature = "avx512f", target_feature = "avx512bw")), target_feature(enable = "avx512f,avx512bw,avx512dq,avx512vl"))]
    pub unsafe fn covers_box_avx512(&self, x0: i32, y0: i32, w: usize, h: usize) -> u16 {
        use std::arch::x86_64::*;
        let n = (w * h).min(16);
        // lane l = centre (x0 + l mod w, y0 + l div w): the offsets from a table per box width
        let (lx, ly) = &LANE_OFFSETS[w.min(16)];
        let half = _mm512_set1_ps(0.5);
        let qx = _mm512_add_ps(_mm512_cvtepi32_ps(_mm512_add_epi32(_mm512_set1_epi32(x0), _mm512_loadu_si512(lx.as_ptr() as *const _))), half);
        let qy = _mm512_add_ps(_mm512_cvtepi32_ps(_mm512_add_epi32(_mm512_set1_epi32(y0), _mm512_loadu_si512(ly.as_ptr() as *const _))), half);
        let zero = _mm512_setzero_ps();
        let lanes: u16 = if n >= 16 { u16::MAX } else { ((1u32 << n) - 1) as u16 };
        let mut inside: __mmask16 = lanes;
        let es: [([f32; 2], [f32; 2]); 3] = [(self.a, self.b), (self.b, self.c), (self.c, self.a)];
        for k in 0..3 {
            let (a, b) = es[k];
            // e = (b−a).x·(q−a).y − (b−a).y·(q−a).x, the scalar `edge` operation order per lane
            let p1 = _mm512_mul_ps(_mm512_set1_ps(b[0] - a[0]), _mm512_sub_ps(qy, _mm512_set1_ps(a[1])));
            let p2 = _mm512_mul_ps(_mm512_set1_ps(b[1] - a[1]), _mm512_sub_ps(qx, _mm512_set1_ps(a[0])));
            let e = _mm512_sub_ps(p1, p2);
            let gt = _mm512_cmp_ps_mask::<_CMP_GT_OQ>(e, zero);
            let on = if self.tl[k] { _mm512_cmp_ps_mask::<_CMP_EQ_OQ>(e, zero) } else { 0 };
            inside &= gt | on;
        }
        inside
    }
}

/// `triangle` visiting only the pixels inside `clip` = (x0, y0, x1, y1) inclusive — the same pixels,
/// the same barycentrics, just the rows and columns outside the clip skipped.
pub fn triangle_clipped<F: FnMut(u32, u32, Bary)>(w: u32, h: u32, p: [[f32; 2]; 3], clip: (i32, i32, i32, i32), f: F) {
    triangle_clipped_masked(w, h, p, clip, None, f)
}

/// The per-triangle setup of the clipped raster: the oriented vertices, the pixel bounds within the clip,
/// the top-left flags, 1/area, and the three edges' row-crossing slopes (f64) for the span limits.
struct Setup {
    a: [f32; 2],
    b: [f32; 2],
    c: [f32; 2],
    swapped: bool,
    x0: i32,
    x1: i32,
    y0: i32,
    y1: i32,
    tl: [bool; 3],
    inv: f32,
    /// Per edge: (origin x, origin y, dx/dy, going down) — None for a horizontal edge or a narrow box.
    slopes: [Option<(f64, f64, f64, bool)>; 3],
}

#[inline(always)]
fn setup(w: u32, h: u32, p: [[f32; 2]; 3], clip: (i32, i32, i32, i32)) -> Option<Setup> {
    let area = edge(p[0], p[1], p[2]);
    if area == 0.0 || !area.is_finite() {
        return None;
    }
    // orient so the inside is positive; remember the permutation to hand back the original order
    let (a, b, c, swapped) = if area > 0.0 { (p[0], p[1], p[2], false) } else { (p[0], p[2], p[1], true) };
    let area = area.abs();
    let (x0, x1, y0, y1) = bounds(&p, w, h)?;
    let (x0, x1, y0, y1) = (x0.max(clip.0), x1.min(clip.2), y0.max(clip.1), y1.min(clip.3));
    if x0 > x1 || y0 > y1 {
        return None;
    }
    let tl = [top_left(a, b), top_left(b, c), top_left(c, a)];
    let inv = 1.0 / area;
    // THE ROW SPANS: per row the x-interval the three half-planes leave for the pixel centres, from the
    // edges' crossings (f64, widened by a pixel each side) — the exact per-pixel test is unchanged and
    // still decides every pixel of the span; the span only skips pixels more than a pixel outside an edge
    // (a large ground quad's bounding box is half outside the triangle: 2.9 G pixel tests per direction on
    // the giant's tiles before this, 0.6 G after). `None` slope = a horizontal edge (no x constraint).
    // (straight-line: the array `map` with a closure was an out-of-line call per triangle with every live
    // value spilled around it — 6 % of the raster)
    #[inline(always)]
    fn slope_of(p: [f32; 2], q: [f32; 2]) -> Option<(f64, f64, f64, bool)> {
        let dy = q[1] as f64 - p[1] as f64;
        if dy == 0.0 { None } else { Some((p[0] as f64, p[1] as f64, (q[0] as f64 - p[0] as f64) / dy, dy > 0.0)) }
    }
    // a narrow bounding box (most leaf triangles: a few pixels wide) is tested pixel by pixel — the three
    // crossings per row cost more than the exact tests they would skip
    let narrow = x1 - x0 < 4;
    let slopes: [Option<(f64, f64, f64, bool)>; 3] = if narrow { [None, None, None] } else { [slope_of(a, b), slope_of(b, c), slope_of(c, a)] };
    Some(Setup { a, b, c, swapped, x0, x1, y0, y1, tl, inv, slopes })
}

impl Setup {
    /// The pixel columns [lo, hi] of row `y` the three edges leave (a pixel outside it is more than a pixel
    /// outside an edge); None = an empty row.
    #[inline(always)]
    fn span(&self, w: u32, py: f32) -> Option<(i32, i32)> {
        let (mut lo, mut hi) = (self.x0, self.x1);
        for sl in &self.slopes {
            if let Some((ax, ay, m, upward)) = sl {
                let xc = ax + (py as f64 - ay) * m;
                if *upward {
                    // inside ⇔ q.x ≤ xc ⇔ x ≤ floor(xc − 0.5)
                    let h = (xc - 0.5).floor();
                    if h < hi as f64 { hi = (h.max(-1.0) as i32).saturating_add(1); }
                } else {
                    let l = (xc - 0.5).ceil();
                    if l > lo as f64 { lo = (l.min(w as f64 + 1.0) as i32).saturating_sub(1); }
                }
            }
        }
        let (lo, hi) = (lo.max(self.x0), hi.min(self.x1));
        if lo > hi { None } else { Some((lo, hi)) }
    }
}

/// THE 16-WIDE ROW TEST (AVX-512): a span's pixels tested sixteen at a time — every lane performs the
/// scalar test's operations in the scalar test's order ((b−a).x·(q−a).y, (b−a).y·(q−a).x, their
/// difference; e·inv for the weights), each an IEEE single operation rounded to nearest even exactly as
/// the scalar `vsubss`/`vmulss` round it, no fused multiply-add, so the sixteen results are the sixteen
/// scalar results bit for bit — the visited set and the barycentrics are unchanged by construction (the
/// randomised test below checks it against the plain scan). Every row goes through it (a narrow row is
/// one partial block: about the cost of two scalar tests). The choice is the BUILD's (`target_feature
/// avx512f`, i.e. `-C target-cpu=znver4`; the x86-64-v3 build walks scalar): with both walks instantiated
/// for one visit closure LLVM outlined the 30-capture visit body — a call per visit, 5 % of the raster —
/// so exactly one walk exists per build and the body inlines into it.
#[cfg(all(target_arch = "x86_64", target_feature = "avx512f", target_feature = "avx512bw"))]
pub const SIMD_ROWS: bool = true;
#[cfg(not(all(target_arch = "x86_64", target_feature = "avx512f", target_feature = "avx512bw")))]
pub const SIMD_ROWS: bool = false;

/// Whether the 16-wide walk can run on this processor (the tests exercise it whatever the build).
#[cfg(test)]
fn avx512_here() -> bool {
    #[cfg(target_arch = "x86_64")]
    {
        std::arch::is_x86_feature_detected!("avx512f") && std::arch::is_x86_feature_detected!("avx512bw")
    }
    #[cfg(not(target_arch = "x86_64"))]
    {
        false
    }
}

/// The wanted bits of the sixteen pixels from id `i` on (bit l = pixel i + l), out of the row-major bitmap.
#[inline(always)]
fn wanted16(m: &[u64], i: usize) -> u16 {
    let (word, off) = (i >> 6, (i & 63) as u32);
    let lo = m[word] >> off;
    if off <= 48 {
        lo as u16
    } else {
        let hi = m.get(word + 1).copied().unwrap_or(0);
        (lo | (hi << (64 - off))) as u16
    }
}

/// `triangle_clipped` that also skips, 64 pixels at a time, the runs of a row whose word of `mask`
/// (one bit per pixel, row-major, pixel id = y·w + x) is zero — the pixels nobody wants are never
/// tested; the visited pixels get exactly the tests and barycentrics of the unmasked walk.
#[inline]
pub fn triangle_clipped_masked<F: FnMut(u32, u32, Bary)>(w: u32, h: u32, p: [[f32; 2]; 3], clip: (i32, i32, i32, i32), mask: Option<&[u64]>, f: F) {
    #[cfg(all(target_arch = "x86_64", target_feature = "avx512f", target_feature = "avx512bw"))]
    {
        // SAFETY: the build enables avx512f/bw for every function, so the processor has them
        unsafe { triangle_rows_avx512(w, h, p, clip, mask, PerPixel(f)) }
    }
    #[cfg(not(all(target_arch = "x86_64", target_feature = "avx512f", target_feature = "avx512bw")))]
    {
        triangle_rows_scalar(w, h, p, clip, mask, f)
    }
}

/// The scalar walk: every pixel of every row span tested one at a time.
#[inline(never)]
pub fn triangle_rows_scalar<F: FnMut(u32, u32, Bary)>(w: u32, h: u32, p: [[f32; 2]; 3], clip: (i32, i32, i32, i32), mask: Option<&[u64]>, mut f: F) {
    let Some(s) = setup(w, h, p, clip) else { return };
    let (a, b, c) = (s.a, s.b, s.c);
    for y in s.y0..=s.y1 {
        let py = y as f32 + 0.5;
        let Some((lo, hi)) = s.span(w, py) else { continue };
        let mut x = lo;
        while x <= hi {
            if let Some(m) = mask {
                let i = y as usize * w as usize + x as usize;
                if (m[i >> 6] >> (i & 63)) == 0 {
                    // no wanted pixel from here to the end of this word
                    x += 64 - (i & 63) as i32;
                    continue;
                }
                if (m[i >> 6] >> (i & 63)) & 1 == 0 {
                    x += 1;
                    continue;
                }
            }
            let q = [x as f32 + 0.5, py];
            // e0 opposite c (edge a→b), e1 opposite a (b→c), e2 opposite b (c→a)
            let e0 = edge(a, b, q);
            let e1 = edge(b, c, q);
            let e2 = edge(c, a, q);
            let inside = (e0 > 0.0 || (e0 == 0.0 && s.tl[0])) && (e1 > 0.0 || (e1 == 0.0 && s.tl[1])) && (e2 > 0.0 || (e2 == 0.0 && s.tl[2]));
            if !inside {
                x += 1;
                continue;
            }
            // barycentrics: weight of a = e1/area (the edge opposite a), etc.
            let (wa, wb, wc) = (e1 * s.inv, e2 * s.inv, e0 * s.inv);
            let bary = if s.swapped { [wa, wc, wb] } else { [wa, wb, wc] };
            f(x as u32, y as u32, bary);
            x += 1;
        }
    }
}

/// The 16-wide walk (see `SIMD_ROWS`): the same rows and spans, each span in blocks of sixteen lanes;
/// `emit(x0, y, cov, bary)` once per block with a set bit — `SpanFn`'s contract.
#[cfg(target_arch = "x86_64")]
#[inline(never)]
#[target_feature(enable = "avx512f,avx512bw,avx512dq,avx512vl")]
pub unsafe fn triangle_rows_avx512<G: SpanFn>(w: u32, h: u32, p: [[f32; 2]; 3], clip: (i32, i32, i32, i32), mask: Option<&[u64]>, mut emit: G) {
    use std::arch::x86_64::*;
    let Some(s) = setup(w, h, p, clip) else { return };
    let (a, b, c) = (s.a, s.b, s.c);
    // per edge (a→b, b→c, c→a): the scalar test's (b−a).x and (b−a).y, and the origin
    let es: [([f32; 2], f32, f32); 3] = [(a, b[0] - a[0], b[1] - a[1]), (b, c[0] - b[0], c[1] - b[1]), (c, a[0] - c[0], a[1] - c[1])];
    let ax = [_mm512_set1_ps(es[0].0[0]), _mm512_set1_ps(es[1].0[0]), _mm512_set1_ps(es[2].0[0])];
    let dy = [_mm512_set1_ps(es[0].2), _mm512_set1_ps(es[1].2), _mm512_set1_ps(es[2].2)];
    let zero = _mm512_setzero_ps();
    let half = _mm512_set1_ps(0.5);
    let iota = _mm512_setr_epi32(0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15);
    let invv = _mm512_set1_ps(s.inv);
    let tl = s.tl;
    // the weights in the caller's vertex order: a ← e1/area, b ← e2/area, c ← e0/area, b and c swapped
    // back when the orientation swapped them
    let (wi_b, wi_c) = if s.swapped { (2usize, 1usize) } else { (1, 2) };
    // (LMTOOL_EDGE_AUDIT) the specification's snapped integer triangle beside the f32 one
    let audit: Option<IntTri> = if *EDGE_AUDIT { let it = IntTri::new(p); if it.degenerate { EDGE_AUDIT_TALLY[3].fetch_add(1, std::sync::atomic::Ordering::Relaxed); } Some(it) } else { None };
    let mut bary = [[0f32; 16]; 3];
    for y in s.y0..=s.y1 {
        let py = y as f32 + 0.5;
        let Some((lo, hi)) = s.span(w, py) else { continue };
        // the scalar test's (b−a).x·(q−a).y — one value per row per edge
        let t1v = [_mm512_set1_ps(es[0].1 * (py - es[0].0[1])), _mm512_set1_ps(es[1].1 * (py - es[1].0[1])), _mm512_set1_ps(es[2].1 * (py - es[2].0[1]))];
        let row_id = y as usize * w as usize;
        let mut x = lo;
        while x <= hi {
            let n = (hi - x + 1).min(16);
            let mut lanes: u16 = if n >= 16 { u16::MAX } else { ((1u32 << n) - 1) as u16 };
            if let Some(m) = mask {
                let i = row_id + x as usize;
                if m[i >> 6] == 0 {
                    // no wanted pixel from here to the end of this word
                    x += 64 - (i & 63) as i32;
                    continue;
                }
                lanes &= wanted16(m, i);
                if lanes == 0 {
                    x += 16;
                    continue;
                }
            }
            // q.x = x + 0.5 per lane, exactly as `x as f32 + 0.5`
            let qx = _mm512_add_ps(_mm512_cvtepi32_ps(_mm512_add_epi32(_mm512_set1_epi32(x), iota)), half);
            let mut e = [zero; 3];
            let mut inside: __mmask16 = lanes;
            for k in 0..3 {
                // e = (b−a).x·(q−a).y − (b−a).y·(q−a).x, the scalar operation order
                let d = _mm512_sub_ps(qx, ax[k]);
                let p2 = _mm512_mul_ps(dy[k], d);
                e[k] = _mm512_sub_ps(t1v[k], p2);
                let gt = _mm512_cmp_ps_mask::<_CMP_GT_OQ>(e[k], zero);
                let on = if tl[k] { _mm512_cmp_ps_mask::<_CMP_EQ_OQ>(e[k], zero) } else { 0 };
                inside &= gt | on;
            }
            if let Some(it) = &audit {
                // every candidate lane of the block against the integer rule
                let (mut n, mut f_only, mut i_only) = (0u64, 0u64, 0u64);
                let mut m = lanes;
                while m != 0 {
                    let l = m.trailing_zeros();
                    m &= m - 1;
                    let f = (inside >> l) & 1 == 1;
                    let i = it.covers(x + l as i32, y);
                    n += 1;
                    if f && !i { f_only += 1; }
                    if i && !f { i_only += 1; }
                }
                EDGE_AUDIT_TALLY[0].fetch_add(n, std::sync::atomic::Ordering::Relaxed);
                EDGE_AUDIT_TALLY[1].fetch_add(f_only, std::sync::atomic::Ordering::Relaxed);
                EDGE_AUDIT_TALLY[2].fetch_add(i_only, std::sync::atomic::Ordering::Relaxed);
            }
            if inside != 0 {
                // the scalar `e * inv` per lane
                _mm512_storeu_ps(bary[0].as_mut_ptr(), _mm512_mul_ps(e[1], invv));
                _mm512_storeu_ps(bary[wi_b].as_mut_ptr(), _mm512_mul_ps(e[2], invv));
                _mm512_storeu_ps(bary[wi_c].as_mut_ptr(), _mm512_mul_ps(e[0], invv));
                emit.span(x as u32, y as u32, inside, &bary);
            }
            x += 16;
        }
    }
}

/// LMTOOL_EDGE_AUDIT=1: every candidate pixel the 16-wide walk tests is ALSO decided by Direct3D 11's own
/// coverage rule — the vertices snapped to 1/256 pixel (round to nearest even) and the edge function as an
/// exact 64-bit integer at the pixel centre, top-left on integer zero — and the disagreements with the port's
/// f32 test are tallied (`EDGE_AUDIT_TALLY`: candidates, f32-inside-only, integer-inside-only, triangles the
/// snapping degenerates). A measurement of how far the f32 test sits from the specification, not a change.
pub static EDGE_AUDIT: std::sync::LazyLock<bool> = std::sync::LazyLock::new(|| std::env::var("LMTOOL_EDGE_AUDIT").map(|v| v == "1").unwrap_or(false));
pub static EDGE_AUDIT_TALLY: [std::sync::atomic::AtomicU64; 4] = [std::sync::atomic::AtomicU64::new(0), std::sync::atomic::AtomicU64::new(0), std::sync::atomic::AtomicU64::new(0), std::sync::atomic::AtomicU64::new(0)];

/// The specification's triangle: vertices in 1/256-pixel integers, oriented so the inside is positive.
struct IntTri {
    v: [[i64; 2]; 3],
    tl: [bool; 3],
    degenerate: bool,
}

impl IntTri {
    fn new(p: [[f32; 2]; 3]) -> IntTri {
        // n.8 fixed point, round to nearest even (D3D11.3 §3.4.1 / §15.16)
        let snap = |v: f32| -> i64 { (v as f64 * 256.0).round_ties_even() as i64 };
        let q = [[snap(p[0][0]), snap(p[0][1])], [snap(p[1][0]), snap(p[1][1])], [snap(p[2][0]), snap(p[2][1])]];
        let area = Self::edge(q[0], q[1], q[2]);
        if area == 0 {
            return IntTri { v: q, tl: [false; 3], degenerate: true };
        }
        let (a, b, c) = if area > 0 { (q[0], q[1], q[2]) } else { (q[0], q[2], q[1]) };
        let tl = |a: [i64; 2], b: [i64; 2]| -> bool { let (dx, dy) = (b[0] - a[0], b[1] - a[1]); dy < 0 || (dy == 0 && dx > 0) };
        IntTri { v: [a, b, c], tl: [tl(a, b), tl(b, c), tl(c, a)], degenerate: false }
    }
    #[inline]
    fn edge(a: [i64; 2], b: [i64; 2], q: [i64; 2]) -> i64 {
        (b[0] - a[0]) * (q[1] - a[1]) - (b[1] - a[1]) * (q[0] - a[0])
    }
    /// The specification's coverage of pixel (x, y): its centre is (256x + 128, 256y + 128).
    fn covers(&self, x: i32, y: i32) -> bool {
        if self.degenerate { return false; }
        let q = [x as i64 * 256 + 128, y as i64 * 256 + 128];
        let [a, b, c] = self.v;
        let e = [Self::edge(a, b, q), Self::edge(b, c, q), Self::edge(c, a, q)];
        (0..3).all(|k| e[k] > 0 || (e[k] == 0 && self.tl[k]))
    }
}

/// THE SPAN INTERFACE: a block of up to sixteen consecutive pixels of one row — `span(x0, y, cov, bary)`:
/// lane l is pixel (x0 + l, y); `cov` bit l is set when that pixel is inside the triangle (top-left rule),
/// wanted (the mask) and within the row span; `bary[k][l]` is vertex k's weight there (the caller's vertex
/// order), bit-identical to the per-pixel raster's — lanes with a clear bit hold values without meaning.
/// A block starts at the row span's first pixel and every sixteenth after it (no alignment).
pub trait SpanFn {
    fn span(&mut self, x0: u32, y: u32, cov: u16, bary: &[[f32; 16]; 3]);
}

/// The per-pixel visit as a span emitter: `f(x, y, bary)` for every set lane, left to right.
struct PerPixel<F>(F);
impl<F: FnMut(u32, u32, Bary)> SpanFn for PerPixel<F> {
    #[inline(always)]
    fn span(&mut self, x0: u32, y: u32, cov: u16, bary: &[[f32; 16]; 3]) {
        let mut bits = cov;
        while bits != 0 {
            let l = bits.trailing_zeros() as usize;
            bits &= bits - 1;
            (self.0)(x0 + l as u32, y, [bary[0][l], bary[1][l], bary[2][l]]);
        }
    }
}

/// A closure as a span emitter.
pub struct Spans<G>(pub G);
impl<G: FnMut(u32, u32, u16, &[[f32; 16]; 3])> SpanFn for Spans<G> {
    #[inline(always)]
    fn span(&mut self, x0: u32, y: u32, cov: u16, bary: &[[f32; 16]; 3]) {
        (self.0)(x0, y, cov, bary)
    }
}

/// `triangle_clipped_masked` delivering blocks of sixteen pixels (`SpanFn`) instead of pixels — the visit
/// body can then work sixteen lanes wide (z, the range test, the count and wanted bits) and fall back to
/// per-lane work for the set bits. The 16-wide build hands the blocks straight from the kernel; the scalar
/// build assembles them from the per-pixel walk (the same pixels, the same weights).
#[inline]
pub fn triangle_clipped_masked_spans<G: FnMut(u32, u32, u16, &[[f32; 16]; 3])>(w: u32, h: u32, p: [[f32; 2]; 3], clip: (i32, i32, i32, i32), mask: Option<&[u64]>, g: G) {
    #[cfg(all(target_arch = "x86_64", target_feature = "avx512f", target_feature = "avx512bw"))]
    {
        // SAFETY: the build enables avx512f/bw for every function, so the processor has them
        unsafe { triangle_rows_avx512(w, h, p, clip, mask, Spans(g)) }
    }
    #[cfg(not(all(target_arch = "x86_64", target_feature = "avx512f", target_feature = "avx512bw")))]
    {
        triangle_rows_scalar_spans(w, h, p, clip, mask, g)
    }
}

/// The scalar walk's pixels grouped into the kernel's blocks (the block starts at the row span's first
/// pixel, then every sixteenth) — for the scalar build and the tests.
pub fn triangle_rows_scalar_spans<G: FnMut(u32, u32, u16, &[[f32; 16]; 3])>(w: u32, h: u32, p: [[f32; 2]; 3], clip: (i32, i32, i32, i32), mask: Option<&[u64]>, mut g: G) {
    let Some(s) = setup(w, h, p, clip) else { return };
    let (a, b, c) = (s.a, s.b, s.c);
    let mut bary = [[0f32; 16]; 3];
    for y in s.y0..=s.y1 {
        let py = y as f32 + 0.5;
        let Some((lo, hi)) = s.span(w, py) else { continue };
        let mut x0 = lo;
        while x0 <= hi {
            let n = (hi - x0 + 1).min(16);
            let mut cov = 0u16;
            for l in 0..n {
                let x = x0 + l;
                if let Some(m) = mask {
                    let i = y as usize * w as usize + x as usize;
                    if (m[i >> 6] >> (i & 63)) & 1 == 0 { continue; }
                }
                let q = [x as f32 + 0.5, py];
                let e0 = edge(a, b, q);
                let e1 = edge(b, c, q);
                let e2 = edge(c, a, q);
                let inside = (e0 > 0.0 || (e0 == 0.0 && s.tl[0])) && (e1 > 0.0 || (e1 == 0.0 && s.tl[1])) && (e2 > 0.0 || (e2 == 0.0 && s.tl[2]));
                if !inside { continue; }
                let (wa, wb, wc) = (e1 * s.inv, e2 * s.inv, e0 * s.inv);
                let bw = if s.swapped { [wa, wc, wb] } else { [wa, wb, wc] };
                bary[0][l as usize] = bw[0];
                bary[1][l as usize] = bw[1];
                bary[2][l as usize] = bw[2];
                cov |= 1 << l;
            }
            if cov != 0 {
                g(x0 as u32, y as u32, cov, &bary);
            }
            x0 += 16;
        }
    }
}

/// A depth buffer with a "nearest wins" test (smaller z is nearer), f32 per pixel, cleared to +inf.
pub struct Depth {
    pub w: u32,
    pub h: u32,
    pub z: Vec<f32>,
}

impl Depth {
    pub fn new(w: u32, h: u32) -> Depth {
        Depth { w, h, z: vec![f32::INFINITY; (w * h) as usize] }
    }
    /// Depth-test-and-write: true when `z` is nearer than what is stored (and stores it).
    #[inline]
    pub fn test_write(&mut self, x: u32, y: u32, z: f32) -> bool {
        let i = (y * self.w + x) as usize;
        if z < self.z[i] {
            self.z[i] = z;
            true
        } else {
            false
        }
    }
    #[inline]
    pub fn get(&self, x: u32, y: u32) -> f32 {
        self.z[(y * self.w + x) as usize]
    }
}

/// Interpolate a per-vertex attribute with barycentrics.
#[inline]
pub fn lerp3(v: [f32; 3], b: Bary) -> f32 {
    v[0] * b[0] + v[1] * b[1] + v[2] * b[2]
}

#[inline]
pub fn lerp3v(v: [[f32; 3]; 3], b: Bary) -> [f32; 3] {
    [lerp3([v[0][0], v[1][0], v[2][0]], b), lerp3([v[0][1], v[1][1], v[2][1]], b), lerp3([v[0][2], v[1][2], v[2][2]], b)]
}

#[cfg(test)]
mod span_tests {
    use super::*;

    /// The plain scan: every pixel of the bounding box ∩ clip tested with `covers`, the barycentrics from
    /// the same edge values — the definition the spans and the 16-wide test must reproduce.
    fn reference(w: u32, h: u32, p: [[f32; 2]; 3], clip: (i32, i32, i32, i32), mask: Option<&[u64]>) -> Vec<(u32, u32, [f32; 3])> {
        let mut out = Vec::new();
        let area = edge(p[0], p[1], p[2]);
        if area == 0.0 || !area.is_finite() {
            return out;
        }
        let (a, b, c, swapped) = if area > 0.0 { (p[0], p[1], p[2], false) } else { (p[0], p[2], p[1], true) };
        let inv = 1.0 / area.abs();
        let Some((x0, x1, y0, y1)) = bounds(&p, w, h) else { return out };
        let (x0, x1, y0, y1) = (x0.max(clip.0), x1.min(clip.2), y0.max(clip.1), y1.min(clip.3));
        for y in y0..=y1 {
            for x in x0..=x1 {
                if let Some(m) = mask {
                    let i = y as usize * w as usize + x as usize;
                    if (m[i >> 6] >> (i & 63)) & 1 == 0 { continue; }
                }
                if !covers(p, x as u32, y as u32) { continue; }
                let q = [x as f32 + 0.5, y as f32 + 0.5];
                let (e0, e1, e2) = (edge(a, b, q), edge(b, c, q), edge(c, a, q));
                let (wa, wb, wc) = (e1 * inv, e2 * inv, e0 * inv);
                out.push((x as u32, y as u32, if swapped { [wa, wc, wb] } else { [wa, wb, wc] }));
            }
        }
        out
    }

    fn random_triangles(n: usize, w: u32, h: u32) -> Vec<[[f32; 2]; 3]> {
        let mut seed = 0x9e3779b97f4a7c15u64;
        let mut rnd = || { seed ^= seed << 13; seed ^= seed >> 7; seed ^= seed << 17; (seed % 1_000_000) as f32 / 1_000_000.0 };
        (0..n).map(|k| {
            let scale = match k % 4 { 0 => 3.0, 1 => 30.0, 2 => 150.0, _ => 1.0 };
            let (cx, cy) = (rnd() * (w as f32 + 14.0) - 7.0, rnd() * (h as f32 + 10.0) - 5.0);
            let mut p = [[0.0f32; 2]; 3];
            for v in p.iter_mut() { *v = [cx + (rnd() - 0.5) * scale, cy + (rnd() - 0.5) * scale * if k % 5 == 0 { 0.02 } else { 1.0 }]; }
            if k % 7 == 0 { p[1][1] = p[0][1]; } // a horizontal edge
            if k % 11 == 0 { p[2][0] = p[0][0]; } // a vertical edge
            if k % 13 == 0 { p[0] = [p[0][0].round() + 0.5, p[0][1].round() + 0.5]; } // a vertex on a pixel centre
            if k % 17 == 0 { p[1] = [p[1][0].round() + 0.5, p[1][1]]; } // a vertex column on the centres
            p
        }).collect()
    }

    /// The span-limited clipped raster visits exactly the pixels (with the same barycentrics) the plain
    /// scan does, over random triangles of every shape: tiny, huge, thin, axis-aligned, degenerate.
    #[test]
    fn the_row_spans_skip_only_outside_pixels() {
        let (w, h) = (96u32, 80u32);
        for (k, p) in random_triangles(3000, w, h).into_iter().enumerate() {
            let plain = reference(w, h, p, (0, 0, w as i32 - 1, h as i32 - 1), None);
            let mut spanned: Vec<(u32, u32, [f32; 3])> = Vec::new();
            triangle_rows_scalar(w, h, p, (0, 0, w as i32 - 1, h as i32 - 1), None, |x, y, b| spanned.push((x, y, b)));
            assert_eq!(plain, spanned, "triangle {k}: {p:?}");
        }
    }

    /// The 16-wide test (every threshold from 1 up, so partial blocks, whole blocks and the scalar tail
    /// mix) visits the plain scan's pixels with the plain scan's barycentrics — bit for bit — under random
    /// clips and random wanted bitmaps (dense, sparse, and whole zero words).
    #[test]
    fn the_sixteen_wide_test_is_the_scalar_test() {
        if !avx512_here() {
            eprintln!("no avx512 here: the 16-wide path is not exercised");
            return;
        }
        let (w, h) = (200u32, 60u32);
        let words = (w as usize * h as usize + 63) / 64;
        let mut seed = 0x2545f4914f6cdd1du64;
        let mut rnd = || { seed ^= seed << 13; seed ^= seed >> 7; seed ^= seed << 17; seed };
        for (k, p) in random_triangles(4000, w, h).into_iter().enumerate() {
            let clip = match k % 3 {
                0 => (0, 0, w as i32 - 1, h as i32 - 1),
                1 => (17, 5, 150, 41),
                _ => ((rnd() % 100) as i32, (rnd() % 30) as i32, 100 + (rnd() % 100) as i32, 30 + (rnd() % 30) as i32),
            };
            let bits: Option<Vec<u64>> = match k % 4 {
                0 => None,
                1 => Some((0..words).map(|_| rnd()).collect()),
                2 => Some((0..words).map(|_| rnd() & rnd() & rnd()).collect()),
                _ => Some((0..words).map(|i| if i % 3 == 0 { 0 } else { rnd() | rnd() }).collect()),
            };
            let mask = bits.as_deref();
            let plain = reference(w, h, p, clip, mask);
            let mut scalar: Vec<(u32, u32, [f32; 3])> = Vec::new();
            triangle_rows_scalar(w, h, p, clip, mask, |x, y, b| scalar.push((x, y, b)));
            assert_eq!(plain, scalar, "scalar walk, triangle {k}: {p:?} clip {clip:?}");
            let mut got: Vec<(u32, u32, [f32; 3])> = Vec::new();
            unsafe { triangle_rows_avx512(w, h, p, clip, mask, PerPixel(|x, y, b| got.push((x, y, b)))) };
            assert_eq!(plain.len(), got.len(), "triangle {k}: {p:?} clip {clip:?}");
            for (i, (pp, g)) in plain.iter().zip(got.iter()).enumerate() {
                assert_eq!((pp.0, pp.1), (g.0, g.1), "triangle {k} visit {i}");
                assert_eq!(pp.2.map(f32::to_bits), g.2.map(f32::to_bits), "triangle {k} visit {i}: barycentrics differ");
            }
            // the span forms, both builds' kernels: the same pixels and weights, blocks starting at the
            // span's first pixel then every sixteenth
            let expand = |x0: u32, y: u32, cov: u16, bary: &[[f32; 16]; 3], out: &mut Vec<(u32, u32, [f32; 3])>| {
                assert!(cov != 0, "an empty block was emitted");
                for l in 0..16 {
                    if cov >> l & 1 == 1 { out.push((x0 + l, y, [bary[0][l as usize], bary[1][l as usize], bary[2][l as usize]])); }
                }
            };
            let mut simd_spans: Vec<(u32, u32, [f32; 3])> = Vec::new();
            let mut simd_blocks: Vec<(u32, u32)> = Vec::new();
            unsafe { triangle_rows_avx512(w, h, p, clip, mask, Spans(|x0, y, cov, bary: &[[f32; 16]; 3]| { simd_blocks.push((x0, y)); expand(x0, y, cov, bary, &mut simd_spans) })) };
            let mut scalar_spans: Vec<(u32, u32, [f32; 3])> = Vec::new();
            let mut scalar_blocks: Vec<(u32, u32)> = Vec::new();
            triangle_rows_scalar_spans(w, h, p, clip, mask, |x0, y, cov, bary| { scalar_blocks.push((x0, y)); expand(x0, y, cov, bary, &mut scalar_spans) });
            assert_eq!(simd_blocks, scalar_blocks, "triangle {k}: the two span walks cut different blocks");
            for (label, spans) in [("16-wide spans", &simd_spans), ("scalar spans", &scalar_spans)] {
                assert_eq!(plain.len(), spans.len(), "{label}, triangle {k}: {p:?} clip {clip:?}");
                for (i, (pp, g)) in plain.iter().zip(spans.iter()).enumerate() {
                    assert_eq!((pp.0, pp.1), (g.0, g.1), "{label}, triangle {k} visit {i}");
                    assert_eq!(pp.2.map(f32::to_bits), g.2.map(f32::to_bits), "{label}, triangle {k} visit {i}: barycentrics differ");
                }
            }
        }
    }

    /// `CoverTest::covers_box` (the 16-lane pass on the AVX-512 build) agrees with `covers` at every centre of
    /// random small boxes around random small triangles.
    #[test]
    fn the_box_coverage_is_the_per_centre_test() {
        let (w, h) = (64u32, 64u32);
        let mut seed = 0x1234_5678_9abc_def1u64;
        let mut rnd = || { seed ^= seed << 13; seed ^= seed >> 7; seed ^= seed << 17; seed };
        for (k, p) in random_triangles(6000, w, h).into_iter().enumerate() {
            let Some(c) = CoverTest::new(p) else { continue };
            let bw = 1 + (rnd() % 4) as i32;
            let bh = 1 + (rnd() % (16 / bw as u64)) as i32;
            let (x0, y0) = ((p[0][0].floor() as i32 - (rnd() % 3) as i32).max(0), (p[0][1].floor() as i32 - (rnd() % 3) as i32).max(0));
            let m = c.covers_box(x0, y0, x0 + bw - 1, y0 + bh - 1);
            for j in 0..bh {
                for i in 0..bw {
                    let want = covers(p, (x0 + i) as u32, (y0 + j) as u32);
                    assert_eq!((m >> (j * bw + i)) & 1 == 1, want, "triangle {k} {p:?} centre ({}, {})", x0 + i, y0 + j);
                }
            }
        }
    }

    /// A per-call cost probe (not a test of anything: `cargo test --release -- --ignored --nocapture
    /// raster_call_cost`): millions of leaf-sized triangles through the masked raster with an empty body,
    /// single-threaded — the setup cost per band-triangle pair the giant pays 64 M times per direction.
    #[test]
    #[ignore]
    fn raster_call_cost() {
        let (w, h) = (4096u32, 4096u32);
        let clip = (1, 3000, 4094, 3006);
        for (scale, label) in [(2.5f32, "leaf 2.5 px"), (12.0, "12 px"), (300.0, "300 px quad")] {
            let n = if scale > 100.0 { 20_000 } else { 4_000_000 };
            let tris: Vec<[[f32; 2]; 3]> = random_triangles(n, w, h).into_iter().map(|p| {
                let (cx, cy) = (p[0][0] * 40.0, 3003.0);
                [[cx, cy], [cx + (p[1][0] - p[0][0]) * scale, cy + (p[1][1] - p[0][1]) * scale], [cx + (p[2][0] - p[0][0]) * scale, cy + (p[2][1] - p[0][1]) * scale]]
            }).collect();
            let mut visits = 0u64;
            let t = std::time::Instant::now();
            for p in &tris {
                triangle_clipped_masked(w, h, *p, clip, None, |_, _, _| visits += 1);
            }
            let dt = t.elapsed();
            eprintln!("{label}: {n} calls, {visits} visits, {:.0} ns per call, {:.1} ns per visit", dt.as_nanos() as f64 / n as f64, dt.as_nanos() as f64 / visits.max(1) as f64);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn count(w: u32, h: u32, p: [[f32; 2]; 3]) -> Vec<(u32, u32)> {
        let mut v = Vec::new();
        triangle(w, h, p, |x, y, _| v.push((x, y)));
        v.sort();
        v
    }

    #[test]
    fn full_quad_covers_every_pixel_once() {
        // two triangles sharing the diagonal of an 8×8 quad: 64 pixels, none twice, none missing
        let a = count(8, 8, [[0.0, 0.0], [8.0, 0.0], [8.0, 8.0]]);
        let b = count(8, 8, [[0.0, 0.0], [8.0, 8.0], [0.0, 8.0]]);
        let mut all = a.clone();
        all.extend(b.iter().cloned());
        all.sort();
        let n = all.len();
        all.dedup();
        assert_eq!(n, 64, "each pixel exactly once (top-left rule on the shared diagonal)");
        assert_eq!(all.len(), 64);
    }

    #[test]
    fn shared_edge_through_pixel_centres_is_not_double_counted() {
        // a vertical shared edge at x = 4.5 runs exactly through the centres of column 4
        let a = count(8, 8, [[0.5, 0.5], [4.5, 0.5], [4.5, 7.5]]);
        let b = count(8, 8, [[4.5, 0.5], [7.5, 0.5], [4.5, 7.5]]);
        for px in &a {
            assert!(!b.contains(px), "pixel {px:?} in both triangles");
        }
        let on_edge_a = a.iter().filter(|(x, _)| *x == 4).count();
        let on_edge_b = b.iter().filter(|(x, _)| *x == 4).count();
        assert!(on_edge_a + on_edge_b > 0, "the centres on the shared edge belong to exactly one side");
        assert!(on_edge_a == 0 || on_edge_b == 0);
    }

    #[test]
    fn winding_does_not_matter_and_barycentrics_follow_vertex_order() {
        let mut got = Vec::new();
        triangle(4, 4, [[0.0, 0.0], [4.0, 0.0], [0.0, 4.0]], |x, y, b| got.push((x, y, b)));
        let mut got2 = Vec::new();
        triangle(4, 4, [[0.0, 0.0], [0.0, 4.0], [4.0, 0.0]], |x, y, b| got2.push((x, y, [b[0], b[2], b[1]])));
        got.sort_by(|a, b| (a.0, a.1).cmp(&(b.0, b.1)));
        got2.sort_by(|a, b| (a.0, a.1).cmp(&(b.0, b.1)));
        assert_eq!(got.len(), got2.len());
        for (p, q) in got.iter().zip(got2.iter()) {
            assert_eq!((p.0, p.1), (q.0, q.1));
            for k in 0..3 {
                assert!((p.2[k] - q.2[k]).abs() < 1e-5);
            }
        }
        // the pixel (0,0) centre (0.5,0.5) has weights (0.75, 0.125, 0.125)
        let p00 = got.iter().find(|g| g.0 == 0 && g.1 == 0).unwrap();
        assert!((p00.2[0] - 0.75).abs() < 1e-5 && (p00.2[1] - 0.125).abs() < 1e-5);
    }

    #[test]
    fn sub_pixel_triangle_covers_only_a_centre_it_contains() {
        // a tiny triangle around the centre of pixel (2,3)
        let hit = count(8, 8, [[2.4, 3.4], [2.7, 3.4], [2.5, 3.7]]);
        assert_eq!(hit, vec![(2, 3)]);
        // the same triangle shifted off every centre: nothing
        let miss = count(8, 8, [[2.1, 3.1], [2.4, 3.1], [2.2, 3.4]]);
        assert!(miss.is_empty());
    }

    #[test]
    fn depth_nearest_wins() {
        let mut d = Depth::new(2, 1);
        assert!(d.test_write(0, 0, 5.0));
        assert!(!d.test_write(0, 0, 6.0));
        assert!(d.test_write(0, 0, 4.0));
        assert_eq!(d.get(0, 0), 4.0);
        assert_eq!(d.get(1, 0), f32::INFINITY);
    }
}
