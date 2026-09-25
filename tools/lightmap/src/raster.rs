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
    let area = edge(p[0], p[1], p[2]);
    if area == 0.0 || !area.is_finite() {
        return false;
    }
    let (a, b, c) = if area > 0.0 { (p[0], p[1], p[2]) } else { (p[0], p[2], p[1]) };
    let tl = [top_left(a, b), top_left(b, c), top_left(c, a)];
    let q = [x as f32 + 0.5, y as f32 + 0.5];
    let e0 = edge(a, b, q);
    let e1 = edge(b, c, q);
    let e2 = edge(c, a, q);
    (e0 > 0.0 || (e0 == 0.0 && tl[0])) && (e1 > 0.0 || (e1 == 0.0 && tl[1])) && (e2 > 0.0 || (e2 == 0.0 && tl[2]))
}

/// `triangle` visiting only the pixels inside `clip` = (x0, y0, x1, y1) inclusive — the same pixels,
/// the same barycentrics, just the rows and columns outside the clip skipped.
pub fn triangle_clipped<F: FnMut(u32, u32, Bary)>(w: u32, h: u32, p: [[f32; 2]; 3], clip: (i32, i32, i32, i32), f: F) {
    triangle_clipped_masked(w, h, p, clip, None, f)
}

/// `triangle_clipped` that also skips, 64 pixels at a time, the runs of a row whose word of `mask`
/// (one bit per pixel, row-major, pixel id = y·w + x) is zero — the pixels nobody wants are never
/// tested; the visited pixels get exactly the tests and barycentrics of the unmasked walk.
pub fn triangle_clipped_masked<F: FnMut(u32, u32, Bary)>(w: u32, h: u32, p: [[f32; 2]; 3], clip: (i32, i32, i32, i32), mask: Option<&[u64]>, mut f: F) {
    let area = edge(p[0], p[1], p[2]);
    if area == 0.0 || !area.is_finite() {
        return;
    }
    // orient so the inside is positive; remember the permutation to hand back the original order
    let (a, b, c, swapped) = if area > 0.0 { (p[0], p[1], p[2], false) } else { (p[0], p[2], p[1], true) };
    let area = area.abs();
    let Some((x0, x1, y0, y1)) = bounds(&p, w, h) else { return };
    let (x0, x1, y0, y1) = (x0.max(clip.0), x1.min(clip.2), y0.max(clip.1), y1.min(clip.3));
    if x0 > x1 || y0 > y1 {
        return;
    }
    let tl = [top_left(a, b), top_left(b, c), top_left(c, a)];
    let inv = 1.0 / area;
    // THE ROW SPANS: per row the x-interval the three half-planes leave for the pixel centres, from the
    // edges' crossings (f64, widened by a pixel each side) — the exact per-pixel test below is unchanged and
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
    for y in y0..=y1 {
        let py = y as f32 + 0.5;
        let (mut lo, mut hi) = (x0, x1);
        for sl in &slopes {
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
        let (lo, hi) = (lo.max(x0), hi.min(x1));
        if lo > hi {
            continue;
        }
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
            let inside = (e0 > 0.0 || (e0 == 0.0 && tl[0])) && (e1 > 0.0 || (e1 == 0.0 && tl[1])) && (e2 > 0.0 || (e2 == 0.0 && tl[2]));
            if !inside {
                x += 1;
                continue;
            }
            // barycentrics: weight of a = e1/area (the edge opposite a), etc.
            let (wa, wb, wc) = (e1 * inv, e2 * inv, e0 * inv);
            let bary = if swapped { [wa, wc, wb] } else { [wa, wb, wc] };
            f(x as u32, y as u32, bary);
            x += 1;
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

    /// The span-limited clipped raster visits exactly the pixels (with the same barycentrics) the plain
    /// `triangle` does, over random triangles of every shape: tiny, huge, thin, axis-aligned, degenerate.
    #[test]
    fn the_row_spans_skip_only_outside_pixels() {
        let mut seed = 0x9e3779b97f4a7c15u64;
        let mut rnd = || { seed ^= seed << 13; seed ^= seed >> 7; seed ^= seed << 17; (seed % 1_000_000) as f32 / 1_000_000.0 };
        let (w, h) = (96u32, 80u32);
        for k in 0..3000 {
            let scale = match k % 4 { 0 => 3.0, 1 => 30.0, 2 => 150.0, _ => 1.0 };
            let (cx, cy) = (rnd() * 110.0 - 7.0, rnd() * 90.0 - 5.0);
            let mut p = [[0.0f32; 2]; 3];
            for v in p.iter_mut() { *v = [cx + (rnd() - 0.5) * scale, cy + (rnd() - 0.5) * scale * if k % 5 == 0 { 0.02 } else { 1.0 }]; }
            if k % 7 == 0 { p[1][1] = p[0][1]; } // a horizontal edge
            if k % 11 == 0 { p[2][0] = p[0][0]; } // a vertical edge
            if k % 13 == 0 { p[0] = [p[0][0].round() + 0.5, p[0][1].round() + 0.5]; } // a vertex on a pixel centre
            let mut plain: Vec<(u32, u32, [f32; 3])> = Vec::new();
            triangle(w, h, p, |x, y, b| plain.push((x, y, b)));
            let mut spanned: Vec<(u32, u32, [f32; 3])> = Vec::new();
            triangle_clipped_masked(w, h, p, (0, 0, w as i32 - 1, h as i32 - 1), None, |x, y, b| spanned.push((x, y, b)));
            assert_eq!(plain, spanned, "triangle {k}: {p:?}");
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
