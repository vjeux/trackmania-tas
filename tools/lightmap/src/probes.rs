//! Synthesis of the probe volume (the trailer after `FACADE01` and the
//! four-image "small atlas" blob): world 480 m slots, 16 m probe cells,
//! per-slot occupied ranges, per-level tiles packed into one atlas, the
//! validity mask, the slot table — everything `volume::Volume` decodes,
//! written from our own geometry and lighting.
//!
//! Probe positions: x = 480·i + 16·cx, z = 480·k + 16·cz (cx, cz ∈ [0, 32);
//! cells 30, 31 duplicate the next slot's 0, 1), y = −38 + 16·L (L ∈ [0, 16)):
//! record `pos` = (480·i − 8, −46, 480·k − 8) minus 16 × the label origin,
//! so that probe x, z = pos + 16·(label + ½) and probe y = pos.y + 16·(label − ½)
//! (the tiny 16 editor volume and the 12-blob probe map agree on both).

use crate::bake::BakeParams;
use crate::bvh::Bvh;
use crate::geometry::{add, dot, mul, norm, sub, LightDef, Scene, V3};
use crate::volume::{Block, Volume};

pub const SLOT: f32 = 480.0;
pub const CELL: f32 = 16.0;
pub const Y0: f32 = -54.0;

pub struct ProbeSample {
    /// Frame-0 irradiance (K = 1 units), sky visibility, point-light irradiance.
    pub e: [f32; 3],
    pub sky_vis: f32,
    pub lights: [f32; 3],
    pub inside: bool,
}

struct Rng(u64);
impl Rng {
    fn next(&mut self) -> f32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        ((self.0 >> 40) as f32) / (1u64 << 24) as f32
    }
}

/// Point-light irradiance at `p` (with normal `n`, or None for a probe):
/// Σ c·I·k·att(d/R)·spot·vis·(n·l or 1).
pub fn light_sum(bvh: &Bvh, lights: &[(usize, LightDef)], p: V3, n: Option<V3>, k: f32, skip: u32) -> [f32; 3] {
    let mut acc = [0f32; 3];
    for (li, l) in lights {
        let to = sub(l.pos, p);
        let d2 = dot(to, to);
        if d2 >= l.radius * l.radius || d2 < 1e-3 {
            continue;
        }
        let dist = d2.sqrt();
        let ldir = mul(to, 1.0 / dist);
        let ndl = match n {
            Some(nn) => dot(nn, ldir),
            None => 1.0,
        };
        if ndl <= 0.0 {
            continue;
        }
        // spot: the cone angles are half-angles
        let ang = dot(l.dir, mul(ldir, -1.0)).clamp(-1.0, 1.0).acos().to_degrees();
        let (hi, ho) = (l.cone.0, l.cone.1);
        let sp = if ang <= hi {
            1.0
        } else if ang >= ho {
            continue;
        } else {
            let t = (ho - ang) / (ho - hi).max(1e-3);
            t * t * (3.0 - 2.0 * t)
        };
        let x = dist / l.radius;
        let att = (1.0 - x * x).max(0.0).powi(2);
        // shadow ray from the light, ignoring the lamp's own item near the light
        let _ = skip;
        if bvh.occluded(l.pos, mul(ldir, -1.0), dist - 0.08, *li as u32, 1.5) {
            continue;
        }
        let w = l.intensity * k * att * sp * ndl;
        for c in 0..3 {
            acc[c] += l.color[c] * w;
        }
    }
    acc
}

/// Shade one probe: spherical sky visibility (cosine-free, uniform directions),
/// sun visibility, point lights; `inside` = most axis rays hit a back face.
pub fn shade_probe(bvh: &Bvh, prm: &BakeParams, lights: &[(usize, LightDef)], light_k: f32, p: V3, rng: &mut Rng) -> ProbeSample {
    // inside test: 6 axis rays, back-face hits
    let mut back = 0;
    let mut hits = 0;
    for d in [[1.0, 0.0, 0.0], [-1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, -1.0, 0.0], [0.0, 0.0, 1.0], [0.0, 0.0, -1.0]] {
        if let Some(h) = bvh.closest(p, d, 64.0) {
            hits += 1;
            let t = &bvh.tris[h.tri as usize];
            let fn_ = norm(crate::geometry::cross(t.e1, t.e2));
            if dot(fn_, d) > 0.0 {
                back += 1;
            }
        }
    }
    let inside = hits >= 5 && back >= 4;
    let n = prm.sky_samples.max(8);
    let mut vis = 0f32;
    let mut wsum = 0f32;
    for _ in 0..n {
        // uniform sphere direction
        let z = 1.0 - 2.0 * rng.next();
        let r = (1.0 - z * z).max(0.0).sqrt();
        let phi = 2.0 * std::f32::consts::PI * rng.next();
        let d = [r * phi.cos(), z, r * phi.sin()];
        let wgt = if prm.sky_model == 1 { 0.5 + 0.5 * d[1].max(0.0) } else { 1.0 };
        let mut tmax = 1.0e4f32;
        let mut ground = false;
        if d[1] < 0.0 && p[1] > prm.ground_y {
            let tg = (prm.ground_y - p[1]) / d[1];
            if tg < tmax {
                tmax = tg;
                ground = true;
            }
        }
        wsum += wgt;
        if !ground && !bvh.occluded(p, d, tmax, u32::MAX, 0.0) {
            vis += wgt;
        }
    }
    let sky_vis = if inside { 0.0 } else { vis / wsum.max(1e-6) };
    let mut sun_vis = 0f32;
    if !inside && prm.sun_dir[1] > 0.0 {
        let ns = prm.sun_samples.max(1);
        let (st, sb) = {
            let a = if prm.sun_dir[0].abs() > 0.9 { [0.0, 1.0, 0.0] } else { [1.0, 0.0, 0.0] };
            let t = norm(crate::geometry::cross(a, prm.sun_dir));
            (t, crate::geometry::cross(prm.sun_dir, t))
        };
        for k in 0..ns {
            let d = if k == 0 && ns == 1 {
                prm.sun_dir
            } else {
                let r = prm.sun_radius * rng.next().sqrt();
                let phi = 2.0 * std::f32::consts::PI * rng.next();
                norm(add(prm.sun_dir, add(mul(st, r * phi.cos()), mul(sb, r * phi.sin()))))
            };
            if !bvh.occluded(p, d, 1.0e4, u32::MAX, 0.0) {
                sun_vis += 1.0;
            }
        }
        sun_vis /= ns as f32;
    }
    let mut e = [0f32; 3];
    for c in 0..3 {
        // a probe has no normal: the "upness" term at its mean (0.5), the sun at half its normal-incidence value
        e[c] = if inside { 0.0 } else { prm.ambient[c] + 0.5 * prm.up[c] + prm.sky[c] * sky_vis + 0.5 * prm.sun[c] * sun_vis };
    }
    let lights_e = if inside { [0.0; 3] } else { light_sum(bvh, lights, p, None, light_k, u32::MAX) };
    ProbeSample { e, sky_vis, lights: lights_e, inside }
}

/// One stored slice: its tile and per-probe values.
struct Slice {
    block: usize,
    level: u32,
    w: u32,
    h: u32,
    px: Vec<ProbeSample>,
}

pub struct ProbeOut {
    pub volume: Volume,
    /// The four WEBP images, in blob order.
    pub images: Vec<Vec<u8>>,
    pub blob: Vec<u8>,
    pub atlas_w: u32,
    pub atlas_h: u32,
    pub blocks: usize,
    pub slices: usize,
}

/// Build the probe volume for `scene`; `template` supplies the constants we do
/// not derive (head words, slot geometry, the tail, the unknown counts and the
/// third image's scale).
pub fn build(_scene: &Scene, bvh: &Bvh, prm: &BakeParams, lights: &[(usize, LightDef)], light_k: f32, template: &Volume, vp8_q: u8) -> Result<ProbeOut, String> {
    // world bbox of the geometry
    let (mut lo, mut hi) = ([f32::MAX; 3], [f32::MIN; 3]);
    for t in &bvh.tris {
        for p in [t.p0, add(t.p0, t.e1), add(t.p0, t.e2)] {
            for k in 0..3 {
                lo[k] = lo[k].min(p[k]);
                hi[k] = hi[k].max(p[k]);
            }
        }
    }
    if lo[0] > hi[0] {
        return Err("no geometry".into());
    }
    let slot_lo = |v: f32| (v / SLOT).floor().max(0.0) as i32;
    let (i0, i1) = (slot_lo(lo[0] - 32.0), slot_lo(hi[0]));
    let (k0, k1) = (slot_lo(lo[2] - 32.0), slot_lo(hi[2]));
    if i1 >= 5 || k1 >= 5 {
        return Err(format!("geometry reaches slot ({i1}, {k1}): the 5×3×5 slot grid covers 2400 m from the origin"));
    }
    // occupancy per candidate slot: cells (cx, cz, L) with geometry within the 16 m cell box
    struct Occ {
        i: i32,
        k: i32,
        cells: Vec<bool>, // 32 × 16 × 32 (cx, L, cz)
    }
    let idx = |cx: u32, l: u32, cz: u32| (cz * 32 * 16 + l * 32 + cx) as usize;
    let mut occs: Vec<Occ> = Vec::new();
    for k in k0..=k1 {
        for i in i0..=i1 {
            let mut cells = vec![false; 32 * 16 * 32];
            let mut any = false;
            for cz in 0..32u32 {
                for cx in 0..32u32 {
                    let (x, z) = (SLOT * i as f32 + CELL * cx as f32, SLOT * k as f32 + CELL * cz as f32);
                    // quick column test
                    if !bvh.any_in_box([x - 8.0, -1.0e6, z - 8.0], [x + 8.0, 1.0e6, z + 8.0]) {
                        continue;
                    }
                    for l in 0..16u32 {
                        let y = Y0 + CELL * l as f32;
                        if bvh.any_in_box([x - 8.0, y - 8.0, z - 8.0], [x + 8.0, y + 8.0, z + 8.0]) {
                            cells[idx(cx, l, cz)] = true;
                            any = true;
                        }
                    }
                }
            }
            if any {
                occs.push(Occ { i, k, cells });
            }
        }
    }
    let n = occs.len();
    if n == 0 {
        return Err("no occupied slot".into());
    }
    // label grid: cols = ceil(sqrt(n/2)), rows = ceil(n/cols) (Nadeo: 8 → 2×4, 12 → 3×4, 27 → 4×7)
    let cols = ((n as f32 / 2.0).sqrt().ceil() as u32).max(1);
    let rows = (n as u32 + cols - 1) / cols;
    let mut blocks: Vec<Block> = Vec::new();
    let mut slices: Vec<Slice> = Vec::new();
    let mut slot_table = vec![-1i32; 75];
    for (bi, o) in occs.iter().enumerate() {
        let (col, row) = (bi as u32 % cols, bi as u32 / cols);
        let origin = [32 * col, 16 * row, 0];
        // ranges
        let (mut xlo, mut xhi, mut llo, mut lhi, mut zlo, mut zhi) = (32u32, 0u32, 16u32, 0u32, 32u32, 0u32);
        for cz in 0..32u32 {
            for l in 0..16u32 {
                for cx in 0..32u32 {
                    if o.cells[idx(cx, l, cz)] {
                        xlo = xlo.min(cx);
                        xhi = xhi.max(cx + 1);
                        llo = llo.min(l);
                        lhi = lhi.max(l + 1);
                        zlo = zlo.min(cz);
                        zhi = zhi.max(cz + 1);
                    }
                }
            }
        }
        let (xlo, xhi) = (xlo.saturating_sub(2), (xhi + 2).min(32));
        let (zlo, zhi) = (zlo.saturating_sub(2), (zhi + 2).min(32));
        let (llo_r, lhi_r) = (llo.saturating_sub(2), (lhi + 2).min(16));
        let pos = [SLOT * o.i as f32 - 8.0 - CELL * origin[0] as f32, -46.0 - CELL * origin[1] as f32, SLOT * o.k as f32 - 8.0];
        let mut b = Block {
            origin,
            min: [origin[0] + xlo, origin[1] + llo_r, zlo],
            max: [origin[0] + xhi, origin[1] + lhi_r, zhi],
            cell: [16.0; 3],
            pos,
            slices: Vec::new(),
        };
        for l in llo_r..lhi_r {
            if l < llo {
                b.slices.push(None); // the two levels under the geometry: not stored
            } else {
                b.slices.push(Some((0, 0))); // placed below
                slices.push(Slice { block: bi, level: l, w: xhi - xlo, h: zhi - zlo, px: Vec::new() });
            }
        }
        let j = 0i32;
        let si = o.i + 5 * j + 15 * o.k;
        if (0..75).contains(&si) {
            slot_table[si as usize] = bi as i32;
        }
        blocks.push(b);
    }
    // shade every slice's probes (threads over slices)
    {
        let next = std::sync::atomic::AtomicUsize::new(0);
        let threads = std::thread::available_parallelism().map(|x| x.get()).unwrap_or(8).min(160);
        let results: Vec<std::sync::Mutex<Vec<ProbeSample>>> = (0..slices.len()).map(|_| std::sync::Mutex::new(Vec::new())).collect();
        std::thread::scope(|sc| {
            for _ in 0..threads {
                sc.spawn(|| loop {
                    let si = next.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                    if si >= slices.len() {
                        break;
                    }
                    let s = &slices[si];
                    let b = &blocks[s.block];
                    let o = &occs[s.block];
                    let mut rng = Rng(0x9E37_79B9_7F4A_7C15 ^ ((si as u64 + 1) * 0x2545_F491_4F6C_DD1D));
                    let mut px = Vec::with_capacity((s.w * s.h) as usize);
                    for cz in b.min[2]..b.max[2] {
                        for cx in (b.min[0] - b.origin[0])..(b.max[0] - b.origin[0]) {
                            let p = [SLOT * o.i as f32 + CELL * cx as f32, Y0 + CELL * s.level as f32, SLOT * o.k as f32 + CELL * cz as f32];
                            px.push(shade_probe(bvh, prm, lights, light_k, p, &mut rng));
                        }
                    }
                    *results[si].lock().unwrap() = px;
                });
            }
        });
        for (s, r) in slices.iter_mut().zip(results) {
            s.px = r.into_inner().unwrap();
        }
    }
    // pack the tiles: shelf packer, tallest first, 1-pixel gutters; grow the atlas until it fits
    let area: u64 = slices.iter().map(|s| (s.w + 1) as u64 * (s.h + 1) as u64).sum();
    let mut side = ((area as f32 * 1.15).sqrt().ceil() as u32).max(16);
    let placements: Vec<(u32, u32)>;
    let (aw, ah);
    loop {
        let mut order: Vec<usize> = (0..slices.len()).collect();
        order.sort_by_key(|&i| (std::cmp::Reverse(slices[i].h), std::cmp::Reverse(slices[i].w)));
        let mut pl = vec![(0u32, 0u32); slices.len()];
        let (mut x, mut y, mut row_h) = (0u32, 0u32, 0u32);
        let mut ok = true;
        let mut maxy = 0;
        for &i in &order {
            let (w, h) = (slices[i].w, slices[i].h);
            if x + w > side {
                x = 0;
                y += row_h + 1;
                row_h = 0;
            }
            if y + h > side {
                ok = false;
                break;
            }
            pl[i] = (x, y);
            x += w + 1;
            row_h = row_h.max(h);
            maxy = maxy.max(y + h);
        }
        if ok {
            placements = pl;
            aw = side;
            ah = maxy.max(4);
            break;
        }
        side += 8;
    }
    // images
    let mut imgs: Vec<crate::img::Rgb> = (0..4).map(|_| crate::img::Rgb::new(aw, ah)).collect();
    // fills like Nadeo's: the unused area carries the mean probe colour (harmless), mask bits set
    let e_max = slices.iter().flat_map(|s| s.px.iter()).flat_map(|p| p.e.iter().copied()).fold(0.0f32, f32::max).max(1e-3);
    let l_max = slices.iter().flat_map(|s| s.px.iter()).flat_map(|p| p.lights.iter().copied()).fold(0.0f32, f32::max).max(1e-3);
    let c_scale = template.frame_info.get(2).map(|f| f.0).unwrap_or(0.22);
    let c_tint = [0.75f32, 0.75, 0.80];
    let mut cell4 = vec![0xffffu16; ((aw + 3) / 4 * ((ah + 3) / 4)) as usize];
    let cw4 = (aw + 3) / 4;
    for (si, s) in slices.iter().enumerate() {
        let (tx, ty) = placements[si];
        let sl = (s.level + blocks[s.block].origin[1] - blocks[s.block].min[1]) as usize;
        blocks[s.block].slices[sl] = Some((tx, ty));
        for zz in 0..s.h {
            for xx in 0..s.w {
                let p = &s.px[(zz * s.w + xx) as usize];
                let (ax, ay) = (tx + xx, ty + zz);
                let q = |v: f32, m: f32| (v / m * 255.0).round().clamp(0.0, 255.0) as u8;
                imgs[0].set(ax, ay, [q(p.e[0], e_max), q(p.e[1], e_max), q(p.e[2], e_max)]);
                let b = q(p.sky_vis, 1.0);
                imgs[1].set(ax, ay, [b, b, b]);
                let cv = if p.inside { 0.15 } else { 0.55 + 0.45 * p.sky_vis };
                imgs[2].set(ax, ay, [q(cv * c_tint[0], 1.0), q(cv * c_tint[1], 1.0), q(cv * c_tint[2], 1.0)]);
                imgs[3].set(ax, ay, [q(p.lights[0], l_max), q(p.lights[1], l_max), q(p.lights[2], l_max)]);
                if p.inside {
                    let ci = ((ay / 4) * cw4 + ax / 4) as usize;
                    cell4[ci] &= !(1u16 << ((ay % 4) * 4 + ax % 4));
                }
            }
        }
    }
    // unused pixels: a neutral fill
    let mean = [160u8, 150, 160];
    for y in 0..ah {
        for x in 0..aw {
            let covered = slices.iter().enumerate().any(|(si, s)| {
                let (tx, ty) = placements[si];
                x >= tx && x < tx + s.w && y >= ty && y < ty + s.h
            });
            if !covered {
                imgs[0].set(x, y, mean);
                imgs[1].set(x, y, [149, 149, 149]);
                imgs[2].set(x, y, [186, 186, 200]);
                imgs[3].set(x, y, [0, 0, 0]);
            }
        }
    }
    let images: Vec<Vec<u8>> = imgs.iter().map(|im| crate::vp8enc::encode(&im.px, im.w, im.h, vp8_q)).collect();
    let (blob, ends) = crate::volume::join_probe_blob(&images);
    let mut frame_info = template.frame_info.clone();
    while frame_info.len() < 3 {
        frame_info.push((1.0, 0));
    }
    frame_info[0] = (e_max, ends[0]);
    frame_info[1] = (l_max, ends[1]);
    frame_info[2] = (c_scale, ends[2]);
    let volume = Volume {
        head_consts: template.head_consts.clone(),
        frame_info,
        grid: [32 * cols, 16 * rows, 32],
        blocks,
        cell4_dims: Some((cw4, (ah + 3) / 4)),
        cell4,
        slot_grid: template.slot_grid,
        slot_tile: template.slot_tile,
        block_size: template.block_size,
        inv_scale: template.inv_scale,
        unk_f: template.unk_f,
        slots: slot_table,
        counts: template.counts,
        tail: template.tail.clone(),
    };
    Ok(ProbeOut { volume, images, blob, atlas_w: aw, atlas_h: ah, blocks: n, slices: slices.len() })
}
