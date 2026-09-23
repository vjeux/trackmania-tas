//! The editor's chart allocation (RE child 2026-09-23, docs/formats/lightmapper-client.md §3.1,
//! typed decompile of `AllocateWithScale_BlockSplit` / `TryPack` / the binary-tree packer):
//! per chart an extent in metres (uv bounds × MeterByUv × block scale), the density
//! `D = W·H / Σarea`, a scale search (shrink by 0.9 until the pack succeeds, then bisect
//! `max_iter − 1` times), and a stable radix order by the float bits of the area — placed from
//! the largest down into the classic binary-tree rect packer. The packer's `w` includes one
//! gutter texel; the 2048-unit layout is `X = 2x + 1`, `W = 2(w − 1)`.

#[derive(Clone, Copy, Debug)]
pub struct ChartExt {
    /// Extent in metres (0 → a fixed `g·mins` chart).
    pub ext: [f32; 2],
    /// Minimum size in granularity units.
    pub mins: [u16; 2],
}

#[derive(Clone, Copy, Debug, Default)]
pub struct Placed {
    pub x: u16,
    pub y: u16,
    pub w: u16,
    pub h: u16,
}

#[derive(Clone, Copy)]
struct Node {
    x: u16,
    y: u16,
    w: u16,
    h: u16,
    used: bool,
    child: [i32; 2],
}

struct Packer {
    nodes: Vec<Node>,
}

impl Packer {
    fn new(w: u16, h: u16) -> Packer {
        Packer { nodes: vec![Node { x: 0, y: 0, w, h, used: false, child: [-1, -1] }] }
    }

    /// The classic binary-tree insert; returns the node index or −1.
    fn insert(&mut self, mut n: usize, w: u16, h: u16) -> i32 {
        loop {
            let node = self.nodes[n];
            if w > node.w || h > node.h {
                return -1;
            }
            if node.child[0] >= 0 {
                let r = self.insert(node.child[0] as usize, w, h);
                if r != -1 {
                    return r;
                }
                n = node.child[1] as usize;
                continue;
            }
            if node.used {
                return -1;
            }
            if w == node.w && h == node.h {
                self.nodes[n].used = true;
                return n as i32;
            }
            let (dw, dh) = (node.w - w, node.h - h);
            let (c0, c1) = if dw > dh {
                (Node { x: node.x, y: node.y, w, h: node.h, used: false, child: [-1, -1] }, Node { x: node.x + w, y: node.y, w: dw, h: node.h, used: false, child: [-1, -1] })
            } else {
                (Node { x: node.x, y: node.y, w: node.w, h, used: false, child: [-1, -1] }, Node { x: node.x, y: node.y + h, w: node.w, h: dh, used: false, child: [-1, -1] })
            };
            let i0 = self.nodes.len() as i32;
            self.nodes.push(c0);
            self.nodes.push(c1);
            self.nodes[n].child = [i0, i0 + 1];
            n = i0 as usize;
        }
    }
}

/// The stable order by the float bits of the area (an LSD radix sort on the u32 bits keeps
/// equal keys in index order).
pub fn area_order(charts: &[ChartExt]) -> Vec<usize> {
    let mut idx: Vec<usize> = (0..charts.len()).collect();
    idx.sort_by_key(|&i| (charts[i].ext[0] * charts[i].ext[1]).to_bits());
    idx
}

/// One packing attempt at `s` layout units per metre (RE child 2, TryPack 0x140295d30, all f32):
/// `a = ((ext.y·s)·ext.x)·s`; Fit FLOORS (`t = sqrt(a/(ext.x·ext.y))`, `w0 = floor(ext.x·t)`);
/// a second fit with the positive carry bumps a side by one; sizes rounded to the granularity
/// `g` (down, or up when the bump asked for more), floored at `m·mins`; `carry += a − w·h`.
/// Zero-extent charts get `m·mins`. Fails outright when `m²·N ≥ W·H`.
pub fn try_pack(charts: &[ChartExt], order: &[usize], s: f32, w_atlas: u16, h_atlas: u16, g: u16, m: u16) -> Option<Vec<Placed>> {
    let n = charts.len();
    if (m as u64) * (m as u64) * n as u64 >= w_atlas as u64 * h_atlas as u64 {
        return None;
    }
    let mut packer = Packer::new(w_atlas, h_atlas);
    let mut out = vec![Placed::default(); n];
    let mut carry = 0f32;
    let g32 = g as i32;
    for k in (0..n).rev() {
        let i = order[k];
        let c = &charts[i];
        let (w, h) = if c.ext[0] == 0.0 || c.ext[1] == 0.0 {
            (m * c.mins[0].max(1), m * c.mins[1].max(1))
        } else {
            let a = ((c.ext[1] * s) * c.ext[0]) * s;
            let fit = |a: f32| -> (i32, i32) {
                let t = (a / (c.ext[0] * c.ext[1])).sqrt();
                ((c.ext[0] * t).floor() as i32, (c.ext[1] * t).floor() as i32)
            };
            let (w0, h0) = fit(a);
            let (w1, h1) = fit(a + carry.max(0.0));
            let round = |v0: i32, v1: i32, min: u16| -> i32 {
                let vp = v0 + (v0 < v1) as i32;
                let v = if vp % g32 != 0 {
                    let mut v = vp - vp % g32;
                    if vp < v1 {
                        v += g32;
                    }
                    v
                } else {
                    vp
                };
                v.max(m as i32 * min.max(1) as i32)
            };
            let w = round(w0, w1, c.mins[0]);
            let h = round(h0, h1, c.mins[1]);
            carry += a - (w * h) as f32;
            (w.clamp(0, u16::MAX as i32) as u16, h.clamp(0, u16::MAX as i32) as u16)
        };
        let r = packer.insert(0, w, h);
        if r < 0 {
            return None;
        }
        let node = packer.nodes[r as usize];
        out[i] = Placed { x: node.x, y: node.y, w, h };
    }
    Some(out)
}

/// The layout granularity and pad from the layout side (FUN_14028f190): `k = (max(W,H) | 1024)
/// >> 10; lg = bsr(k) + 1; g = 1 << (lg − 1); pad = 1 << max(0, lg − 2)`; the minimum chart size
/// `m = roundup(max(4·pad, 6), g)`.
pub fn layout_params(w: u16, h: u16) -> (u16, u16, u16) {
    let k = (w.max(h) as u32 | 1024) >> 10;
    let lg = 32 - k.leading_zeros(); // bsr + 1
    let g = 1u16 << (lg - 1);
    let pad = 1u16 << lg.saturating_sub(2).max(0);
    let m0 = (4 * pad).max(6);
    let m = (m0 + g - 1) / g * g;
    (g, pad, m)
}

/// The full walk: density from the total area, the shrink-then-bisect scale search with
/// `max_iter` steps. Returns (s_final texels/m, placements in packer units).
pub fn allocate(charts: &[ChartExt], w_atlas: u16, h_atlas: u16, g: u16, m: u16, max_iter: u32) -> Option<(f32, Vec<Placed>)> {
    let n = charts.len();
    let sum_area: f64 = charts.iter().map(|c| (c.ext[0] as f64) * (c.ext[1] as f64)).sum();
    if sum_area <= 0.0 {
        return None;
    }
    let d = (w_atlas as f64 * h_atlas as f64 / sum_area) as f32;
    let max_ext = charts.iter().fold([0f32; 2], |m, c| [m[0].max(c.ext[0]), m[1].max(c.ext[1])]);
    let mut scale_hi = 1.0f32;
    let (rx, ry) = (d.sqrt() * max_ext[0] / w_atlas as f32, d.sqrt() * max_ext[1] / h_atlas as f32);
    if rx > 1.0 || ry > 1.0 {
        scale_hi = 1.0 / rx.max(ry).powi(2);
    }
    if (w_atlas as f32 / g as f32) * (h_atlas as f32 / g as f32) * 0.9 < n as f32 {
        scale_hi = 0.01;
    }
    let order = area_order(charts);
    let mut iter = 0u32;
    let mut scale_lo;
    let mut best;
    loop {
        iter += 1;
        scale_lo = scale_hi * 0.9;
        let s = (scale_lo * d).sqrt();
        if let Some(p) = try_pack(charts, &order, s, w_atlas, h_atlas, g, m) {
            best = p;
            break;
        }
        // the shrink loop also stops when the largest-area chart would fall under the minimum size
        if let Some(big) = charts.iter().max_by(|a, b| (a.ext[0] * a.ext[1]).partial_cmp(&(b.ext[0] * b.ext[1])).unwrap()) {
            if big.ext[0] * s < m as f32 || big.ext[1] * s < m as f32 {
                return None;
            }
        }
        scale_hi = scale_lo;
        if iter > 400 {
            return None;
        }
    }
    iter = iter.min(1);
    while iter < max_iter {
        iter += 1;
        let mid = (scale_lo + scale_hi) / 2.0;
        let s = (mid * d).sqrt();
        match try_pack(charts, &order, s, w_atlas, h_atlas, g, m) {
            Some(p) => {
                scale_lo = mid;
                best = p;
            }
            None => scale_hi = mid,
        }
    }
    Some(((scale_lo * d).sqrt(), best))
}
