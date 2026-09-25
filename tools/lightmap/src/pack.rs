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

/// The rounding of the fit (`func_0x14195c7b8` in FUN_14028f570, unresolved by the decompiler): 0 floor (default),
/// 1 round half away from zero, 2 ceil, 3 truncate — `packtest --fit-round N` picks against the editor's table.
pub static FIT_ROUND: std::sync::atomic::AtomicU8 = std::sync::atomic::AtomicU8::new(0);
/// The f32 bits of a Σarea to use instead of the charts' own sum (0 = none).
pub static SUM_AREA_OVERRIDE: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
pub static CARRY_OFFSET: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
/// The carry accumulation's f32 op order under test: 0 c + (a − wh) (the decompile's rendering), 1 (c + a) − wh, 2 in f64, 3 (c − wh) + a.
pub static CARRY_MODE: std::sync::atomic::AtomicU8 = std::sync::atomic::AtomicU8::new(0);
/// The f32 op order of `a = ext·ext·s²` under test: 0 ((y·s)·x)·s (the decompile's rendering), 1 ((x·s)·y)·s, 2 (x·s)·(y·s), 3 (x·y)·(s·s), 4 ((x·y)·s)·s, 5 (y·s)·(x·s).
pub static A_ORDER: std::sync::atomic::AtomicU8 = std::sync::atomic::AtomicU8::new(2);
/// The packer node array's 4·N capacity limit (the game's; `packtest --no-node-cap` lifts it).
pub static PACK_NODE_CAP: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(true);

fn fit_round(v: f32) -> i32 {
    match FIT_ROUND.load(std::sync::atomic::Ordering::Relaxed) {
        1 => v.round() as i32,
        2 => v.ceil() as i32,
        3 => v.trunc() as i32,
        _ => v.floor() as i32,
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
    // (CARRY_OFFSET: an f32 added to the initial carry — the study of the items' a rounding)
    let mut carry = f32::from_bits(CARRY_OFFSET.load(std::sync::atomic::Ordering::Relaxed));
    let g32 = g as i32;
    for k in (0..n).rev() {
        let i = order[k];
        let c = &charts[i];
        let (w, h) = if c.ext[0] == 0.0 || c.ext[1] == 0.0 {
            (m * c.mins[0].max(1), m * c.mins[1].max(1))
        } else {
            let a = match A_ORDER.load(std::sync::atomic::Ordering::Relaxed) { 1 => ((c.ext[0] * s) * c.ext[1]) * s, 2 => (c.ext[0] * s) * (c.ext[1] * s), 3 => (c.ext[0] * c.ext[1]) * (s * s), 4 => ((c.ext[0] * c.ext[1]) * s) * s, 5 => (c.ext[1] * s) * (c.ext[0] * s), _ => ((c.ext[1] * s) * c.ext[0]) * s };
            let fit = |a: f32| -> (i32, i32) {
                let t = (a / (c.ext[0] * c.ext[1])).sqrt();
                (fit_round(c.ext[0] * t), fit_round(c.ext[1] * t))
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
            if let Some(t) = std::env::var_os("LMTOOL_PACK_CARRY_TRACE") { let t: usize = t.to_str().unwrap().parse().unwrap(); let kk = n - k; if kk.abs_diff(t) <= 4 { let t1 = ((a + carry.max(0.0)) / (c.ext[0] * c.ext[1])).sqrt(); eprintln!("pack: chart #{kk} (order slot {k}): a {a} carry before {carry} → fit0 ({w0}, {h0}) fit1 ({w1}, {h1}) [x·t1 {} y·t1 {}] → {w}×{h}; carry after {}", c.ext[0] * t1, c.ext[1] * t1, carry + (a - (w * h) as f32)); } }
            carry = match CARRY_MODE.load(std::sync::atomic::Ordering::Relaxed) { 1 => (carry + a) - (w * h) as f32, 2 => ((carry as f64 + a as f64) - (w * h) as f64) as f32, 3 => (carry - (w * h) as f32) + a, _ => carry + (a - (w * h) as f32) };
            (w.clamp(0, u16::MAX as i32) as u16, h.clamp(0, u16::MAX as i32) as u16)
        };
        // the game's packer node array has a FIXED capacity of 4·N nodes (BlockSplit: FUN_140492660(packer, N << 2) once; TryPack
        // asks for count + 4 before every insert and FAILS when the array cannot hold it): a layout that splits almost every
        // chart twice runs out — `PACK_NODE_CAP` on (default) makes our TryPack fail the same way
        if PACK_NODE_CAP.load(std::sync::atomic::Ordering::Relaxed) && packer.nodes.len() + 4 > 4 * n {
            if std::env::var_os("LMTOOL_PACK_TRACE").is_some() { eprintln!("pack: s {s}: the node array would exceed 4·N = {} at chart {} of {n} ({} nodes)", 4 * n, n - k, packer.nodes.len()); }
            return None;
        }
        let r = packer.insert(0, w, h);
        if r < 0 {
            return None;
        }
        let node = packer.nodes[r as usize];
        out[i] = Placed { x: node.x, y: node.y, w, h };
    }
    if std::env::var_os("LMTOOL_PACK_TRACE").is_some() { let used: u64 = out.iter().map(|p| p.w as u64 * p.h as u64).sum(); let free_leaves: u64 = packer.nodes.iter().filter(|nd| nd.child[0] < 0 && !nd.used).map(|nd| nd.w as u64 * nd.h as u64).sum(); let last: Vec<String> = (0..3).map(|j| { let i = order[j]; format!("({}, {}) {}×{}", out[i].x, out[i].y, out[i].w, out[i].h) }).collect(); eprintln!("pack: s {s}: packed {n} charts with {} nodes (4·N = {}); used {used} of {} ({:.2} %), free leaves {free_leaves}; the last three placed: {}", packer.nodes.len(), 4 * n, w_atlas as u64 * h_atlas as u64, 100.0 * used as f64 / (w_atlas as f64 * h_atlas as f64), last.join(", ")); }
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
    let order = area_order(charts);
    allocate_ordered(charts, &order, w_atlas, h_atlas, g, m, max_iter)
}

/// `allocate` with a caller-supplied ascending-area order (ties resolved by the caller).
pub fn allocate_ordered(charts: &[ChartExt], order: &[usize], w_atlas: u16, h_atlas: u16, g: u16, m: u16, max_iter: u32) -> Option<(f32, Vec<Placed>)> {
    allocate_ordered_forced(charts, order, w_atlas, h_atlas, g, m, max_iter, &[])
}

/// `allocate_ordered` with bisection iterations that are FORCED to fail (`force_fail`, 1-based iteration numbers) —
/// to replay the editor's search path when our TryPack succeeds at a probe where the editor's failed.
pub fn allocate_ordered_forced(charts: &[ChartExt], order: &[usize], w_atlas: u16, h_atlas: u16, g: u16, m: u16, max_iter: u32, force_fail: &[u32]) -> Option<(f32, Vec<Placed>)> {
    let n = charts.len();
    let sum_area: f64 = charts.iter().map(|c| (c.ext[0] as f64) * (c.ext[1] as f64)).sum();
    if sum_area <= 0.0 {
        return None;
    }
    // D = W·H / Σarea in f32 (the game's TotalLmSurfaceMeter is an f32 running sum over the records; `SUM_AREA_OVERRIDE` = the
    // editor's own value when replaying its search)
    let sum_f32: f32 = { let o = f32::from_bits(SUM_AREA_OVERRIDE.load(std::sync::atomic::Ordering::Relaxed)); if o > 0.0 { o } else { charts.iter().fold(0f32, |s, c| s + c.ext[0] * c.ext[1]) } };
    let d = (w_atlas as f32 * h_atlas as f32) / sum_f32;
    if std::env::var_os("LMTOOL_PACK_TRACE").is_some() { eprintln!("pack: Σarea f32 {sum_f32} (f64 {sum_area}), D {d} ({:#010x})", d.to_bits()); }
    let max_ext = charts.iter().fold([0f32; 2], |m, c| [m[0].max(c.ext[0]), m[1].max(c.ext[1])]);
    let mut scale_hi = 1.0f32;
    let (rx, ry) = (d.sqrt() * max_ext[0] / w_atlas as f32, d.sqrt() * max_ext[1] / h_atlas as f32);
    if rx > 1.0 || ry > 1.0 {
        scale_hi = 1.0 / rx.max(ry).powi(2);
    }
    if (w_atlas as f32 / g as f32) * (h_atlas as f32 / g as f32) * 0.9 < n as f32 {
        scale_hi = 0.01;
    }
    let mut iter = 0u32;
    let mut scale_lo;
    let mut best;
    loop {
        iter += 1;
        scale_lo = scale_hi * 0.9;
        let s = (scale_lo * d).sqrt();
        if std::env::var_os("LMTOOL_PACK_TRACE").is_some() { eprintln!("pack: shrink iter {iter}: scale {scale_lo:.9} s {s:.6}"); }
        if let Some(p) = try_pack(charts, order, s, w_atlas, h_atlas, g, m) {
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
        let trace = std::env::var_os("LMTOOL_PACK_TRACE").is_some();
        let attempt = if force_fail.contains(&iter) { None } else { try_pack(charts, order, s, w_atlas, h_atlas, g, m) };
        match attempt {
            Some(p) => {
                if trace { eprintln!("pack: bisect iter {iter}: scale {mid:.9} s {s:.6} succeeds"); }
                scale_lo = mid;
                best = p;
            }
            None => { if trace { eprintln!("pack: bisect iter {iter}: scale {mid:.9} s {s:.6} FAILS"); } scale_hi = mid }
        }
    }
    Some(((scale_lo * d).sqrt(), best))
}
