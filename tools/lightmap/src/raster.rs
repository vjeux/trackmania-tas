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
#[inline]
fn edge(a: [f32; 2], b: [f32; 2], p: [f32; 2]) -> f32 {
    (b[0] - a[0]) * (p[1] - a[1]) - (b[1] - a[1]) * (p[0] - a[0])
}

/// Rasterise one triangle over a w×h target, calling `f(x, y, bary)` for every pixel whose centre it
/// covers under the top-left rule. Degenerate (zero-area) triangles produce nothing. Both windings are
/// accepted (the sign is normalised); `f` receives barycentrics in the vertex order given.
pub fn triangle<F: FnMut(u32, u32, Bary)>(w: u32, h: u32, p: [[f32; 2]; 3], f: F) {
    triangle_clipped(w, h, p, (0, 0, w as i32 - 1, h as i32 - 1), f)
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
    for y in y0..=y1 {
        let py = y as f32 + 0.5;
        let mut x = x0;
        while x <= x1 {
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
