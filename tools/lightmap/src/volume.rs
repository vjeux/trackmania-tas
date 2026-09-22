//! The trailer after `FACADE01` in the `CHmsLightMapCache` blob: the
//! **probe volume** — a 3D grid of light probes on 16 m cells, in 480 m
//! world SLOTS (5×3×5 of them from the origin), one record per occupied slot
//! with its occupied cell range, each height level of the range a 2D tile in
//! the "small" third atlas (frame 0 image 2). Decoded 2026-09-22 (session
//! lightmap-baker) from the 25 Summer sources, the editor's tiny bakes and two
//! probe bakes (maps with isolated blobs at known places); `parse` reads the
//! layout below and `write` reproduces it byte for byte.
//!
//! ```text
//! u32 91                       version / magic
//! u32 1024 ×4                  (constant)
//! u32 30, 5, 0, 0, 6           (constant)
//! 3 × { f32 scale, u32 end }   per probe image k = 0..2: its value scale and the END
//!                              byte offset of its WEBP inside the third atlas blob —
//!                              that blob is a CONCATENATION of four WEBP files
//!                              (0 probe colour, 1 occlusion, 2 pale colour, 3 point
//!                              lights; the fourth runs to the end of the blob)
//! u32 g0, g1, g2               label grid: 32·cols, 16·rows, 32 (cols = ceil(√(n/2)))
//! u32 nblocks
//! nblocks × Block (60 B)       { u32 origin[3] (label cell of the block: 32·col, 16·row, 0),
//!                                u32 min[3], u32 max[3] (occupied cell range, label space,
//!                                2 cells of margin), f32 cell[3] = 16,16,16,
//!                                f32 pos[3] (world offset: probe x,z = pos + 16·(cell+½),
//!                                probe y = pos.y + 16·(cell−½); pos = 480·(i,·,k) − 8 minus
//!                                16 × origin, pos.y = −46 for the ground slot row) }
//! u32 npairs                   = Σ over blocks of (max[1] − min[1])
//! npairs × { u32 x, u32 y }    per block, per height level: the level's tile position in
//!                              the third atlas (pixels; tile = (max0−min0) × (max2−min2),
//!                              rows = z), or (−1, −1) for a level that is not stored
//!                              (the two margin levels under the geometry)
//! u32 ncell4                   = ceil(w/4) × ceil(h/4) of the third atlas
//! ncell4 × u16                 per 4×4 pixel cell: bit (y%4)·4 + x%4 = 0 for a probe INSIDE
//!                              geometry (dark, not to be interpolated), 1 otherwise
//! u32 5, 3, 5                  slot grid (i, j, k): 480 m × 224 m × 480 m from the origin
//! u32 30, 14, 30               usable cells per slot (block size minus the 2-cell overlap)
//! u32 32, 16, 32               block size in cells
//! f32 1/480, 1/224, 1/480      1 / slot pitch
//! f32 −0, 0.1696, −0
//! u32 75; 75 × i32             slot table: block index per slot i + 5·j + 15·k (−1 = empty)
//! u32 a, u32 b, u32 0, 0, 0    two counts (meaning unknown; copied from the template)
//! u32 6, f32 1.0, u8[128] 0    (constant)
//! u32 6, f32 1.0, u8[128] 0    (constant, second copy — sometimes absent)
//! u32 5
//! ```

use crate::Cur;

#[derive(Clone, Debug, PartialEq)]
pub struct Block {
    pub origin: [u32; 3],
    pub min: [u32; 3],
    pub max: [u32; 3],
    pub cell: [f32; 3],
    pub pos: [f32; 3],
    /// One entry per slice along axis 1 (min[1]..max[1]): atlas tile (x, y) or None.
    pub slices: Vec<Option<(u32, u32)>>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Volume {
    pub head_consts: Vec<u32>,
    pub frame_info: Vec<(f32, u32)>,
    pub grid: [u32; 3],
    pub blocks: Vec<Block>,
    pub cell4_dims: Option<(u32, u32)>,
    pub cell4: Vec<u16>,
    pub slot_grid: [u32; 3],
    pub slot_tile: [u32; 3],
    pub block_size: [u32; 3],
    pub inv_scale: [f32; 3],
    pub unk_f: [f32; 3],
    pub slots: Vec<i32>,
    pub counts: [u32; 2],
    pub tail: Vec<u8>,
}

/// The probe blob (frame 0 image 2) is a CONCATENATION of WEBP files, one per
/// probe image; `frame_info[k].1` is the END byte offset of image k (the
/// fourth image runs to the end of the blob) and `frame_info[k].0` its scale.
/// Seen on every file: 4 images of the same size — [0] the probe colour
/// (sky + sun irradiance), [1] a grey occlusion mask (0 inside geometry, 255 open),
/// [2] a pale bluish colour image, [3] the point-light probes.
pub fn split_probe_blob(blob: &[u8], frame_info: &[(f32, u32)]) -> Vec<Vec<u8>> {
    let mut out = Vec::new();
    let mut start = 0usize;
    for &(_, end) in frame_info {
        let end = (end as usize).min(blob.len());
        if end < start {
            break;
        }
        out.push(blob[start..end].to_vec());
        start = end;
    }
    if start < blob.len() {
        out.push(blob[start..].to_vec());
    }
    out
}

/// The inverse of `split_probe_blob`: concatenate and return the end offsets of
/// the first `images.len() - 1` parts (what `frame_info[k].1` must hold).
pub fn join_probe_blob(images: &[Vec<u8>]) -> (Vec<u8>, Vec<u32>) {
    let mut blob = Vec::new();
    let mut ends = Vec::new();
    for (i, im) in images.iter().enumerate() {
        blob.extend_from_slice(im);
        if i + 1 < images.len() {
            ends.push(blob.len() as u32);
        }
    }
    (blob, ends)
}

impl Volume {
    pub fn parse(p: &[u8]) -> Result<Volume, String> {
        let mut c = Cur::new(p);
        let v = c.u32()?;
        if v != 91 {
            return Err(format!("trailer version {v} (expected 91)"));
        }
        let mut head_consts = Vec::new();
        for _ in 0..9 {
            head_consts.push(c.u32()?);
        }
        let mut frame_info = Vec::new();
        for _ in 0..3 {
            let a = c.f32()?;
            let b = c.u32()?;
            frame_info.push((a, b));
        }
        let grid = [c.u32()?, c.u32()?, c.u32()?];
        let nb = c.u32()? as usize;
        let mut blocks = Vec::with_capacity(nb);
        for _ in 0..nb {
            let mut w = [0u32; 9];
            for x in w.iter_mut() {
                *x = c.u32()?;
            }
            let cell = [c.f32()?, c.f32()?, c.f32()?];
            let pos = [c.f32()?, c.f32()?, c.f32()?];
            blocks.push(Block { origin: [w[0], w[1], w[2]], min: [w[3], w[4], w[5]], max: [w[6], w[7], w[8]], cell, pos, slices: Vec::new() });
        }
        let npairs = c.u32()? as usize;
        let expect: usize = blocks.iter().map(|b| (b.max[1] - b.min[1]) as usize).sum();
        if npairs != expect {
            return Err(format!("slice table has {npairs} entries, blocks need {expect}"));
        }
        for b in blocks.iter_mut() {
            for _ in b.min[1]..b.max[1] {
                let x = c.u32()?;
                let y = c.u32()?;
                b.slices.push(if x == u32::MAX { None } else { Some((x, y)) });
            }
        }
        let ncell4 = c.u32()? as usize;
        let mut cell4 = Vec::with_capacity(ncell4);
        for _ in 0..ncell4 {
            cell4.push(c.u16()?);
        }
        let slot_grid = [c.u32()?, c.u32()?, c.u32()?];
        let slot_tile = [c.u32()?, c.u32()?, c.u32()?];
        let block_size = [c.u32()?, c.u32()?, c.u32()?];
        let inv_scale = [c.f32()?, c.f32()?, c.f32()?];
        let unk_f = [c.f32()?, c.f32()?, c.f32()?];
        let ns = c.u32()? as usize;
        let mut slots = Vec::with_capacity(ns);
        for _ in 0..ns {
            slots.push(c.i32()?);
        }
        let counts = [c.u32()?, c.u32()?];
        let tail = c.take(c.left())?.to_vec();
        Ok(Volume { head_consts, frame_info, grid, blocks, cell4_dims: None, cell4, slot_grid, slot_tile, block_size, inv_scale, unk_f, slots, counts, tail })
    }

    pub fn write(&self) -> Vec<u8> {
        let mut o = Vec::new();
        let w32 = |o: &mut Vec<u8>, v: u32| o.extend_from_slice(&v.to_le_bytes());
        let wf = |o: &mut Vec<u8>, v: f32| o.extend_from_slice(&v.to_le_bytes());
        w32(&mut o, 91);
        for &v in &self.head_consts {
            w32(&mut o, v);
        }
        for &(a, b) in &self.frame_info {
            wf(&mut o, a);
            w32(&mut o, b);
        }
        for &g in &self.grid {
            w32(&mut o, g);
        }
        w32(&mut o, self.blocks.len() as u32);
        for b in &self.blocks {
            for &v in b.origin.iter().chain(b.min.iter()).chain(b.max.iter()) {
                w32(&mut o, v);
            }
            for &v in b.cell.iter().chain(b.pos.iter()) {
                wf(&mut o, v);
            }
        }
        let npairs: usize = self.blocks.iter().map(|b| b.slices.len()).sum();
        w32(&mut o, npairs as u32);
        for b in &self.blocks {
            for s in &b.slices {
                match s {
                    Some((x, y)) => {
                        w32(&mut o, *x);
                        w32(&mut o, *y);
                    }
                    None => {
                        w32(&mut o, u32::MAX);
                        w32(&mut o, u32::MAX);
                    }
                }
            }
        }
        w32(&mut o, self.cell4.len() as u32);
        for &v in &self.cell4 {
            o.extend_from_slice(&v.to_le_bytes());
        }
        for &v in self.slot_grid.iter().chain(self.slot_tile.iter()).chain(self.block_size.iter()) {
            w32(&mut o, v);
        }
        for &v in self.inv_scale.iter().chain(self.unk_f.iter()) {
            wf(&mut o, v);
        }
        w32(&mut o, self.slots.len() as u32);
        for &s in &self.slots {
            o.extend_from_slice(&s.to_le_bytes());
        }
        for &v in &self.counts {
            w32(&mut o, v);
        }
        o.extend_from_slice(&self.tail);
        o
    }

    /// The slot grid's world origin: `unk_f` is −origin / slot pitch.
    pub fn world_origin(&self) -> [f32; 3] {
        let pitch = self.slot_pitch();
        [-self.unk_f[0] * pitch[0], -self.unk_f[1] * pitch[1], -self.unk_f[2] * pitch[2]]
    }

    /// Slot pitch in metres: usable cells × 16 (480, 224, 480).
    pub fn slot_pitch(&self) -> [f32; 3] {
        [self.slot_tile[0] as f32 * 16.0, self.slot_tile[1] as f32 * 16.0, self.slot_tile[2] as f32 * 16.0]
    }

    /// The slot (i, j, k) a block's `pos` encodes: pos = origin + pitch·slot − 8 − 16·label origin
    /// (the y term uses the label origin too; x/z labels carry the column, y the row).
    pub fn block_slot(&self, b: &Block) -> (i32, i32, i32) {
        let o = self.world_origin();
        let p = self.slot_pitch();
        let f = |k: usize| ((b.pos[k] + 8.0 + 16.0 * b.origin[k] as f32 - o[k]) / p[k]).round() as i32;
        (f(0), f(1), f(2))
    }

    /// World bounds of a block's occupied cell range: probe centres x,z = pos + 16·(cell + ½),
    /// y = pos.y + 16·(cell − ½); the range spans [min, max) cells.
    pub fn block_world_range(&self, b: &Block) -> ([f32; 3], [f32; 3]) {
        let lo = [b.pos[0] + 16.0 * b.min[0] as f32, b.pos[1] + 16.0 * (b.min[1] as f32 - 1.0), b.pos[2] + 16.0 * b.min[2] as f32];
        let hi = [b.pos[0] + 16.0 * b.max[0] as f32, b.pos[1] + 16.0 * (b.max[1] as f32 - 1.0), b.pos[2] + 16.0 * b.max[2] as f32];
        (lo, hi)
    }

    pub fn describe(&self) -> String {
        let mut s = String::new();
        s.push_str(&format!("head {:?}\nframe_info {:?}\ngrid {:?} blocks {}\n", self.head_consts, self.frame_info, self.grid, self.blocks.len()));
        let origin = self.world_origin();
        let mut table_ok = 0;
        let mut table_bad = Vec::new();
        for (i, b) in self.blocks.iter().enumerate() {
            let stored = b.slices.iter().filter(|s| s.is_some()).count();
            let (si, sj, sk) = self.block_slot(b);
            let idx = si + self.slot_grid[0] as i32 * sj + self.slot_grid[0] as i32 * self.slot_grid[1] as i32 * sk;
            if self.slots.get(idx as usize).copied() == Some(i as i32) {
                table_ok += 1;
            } else {
                table_bad.push(format!("block {i} slot ({si},{sj},{sk}) idx {idx} table {:?}", self.slots.get(idx as usize)));
            }
            let (lo, hi) = self.block_world_range(b);
            s.push_str(&format!(
                "  block {i:>2}: origin {:?} min {:?} max {:?} ext {:?} pos {:?} slot ({si},{sj},{sk}) world x [{:.0},{:.0}) y [{:.0},{:.0}) z [{:.0},{:.0}) slices {}/{}: {:?}\n",
                b.origin,
                b.min,
                b.max,
                [b.max[0] - b.min[0], b.max[1] - b.min[1], b.max[2] - b.min[2]],
                b.pos,
                lo[0], hi[0], lo[1], hi[1], lo[2], hi[2],
                stored,
                b.slices.len(),
                b.slices
            ));
        }
        s.push_str(&format!("world origin ({:.0}, {:.0}, {:.0}); slot table consistent for {table_ok} blocks{}\n", origin[0], origin[1], origin[2], if table_bad.is_empty() { String::new() } else { format!(", NOT for: {}", table_bad.join("; ")) }));
        let nz = self.cell4.iter().filter(|&&v| v != 0xffff).count();
        s.push_str(&format!("cell4 table: {} entries, {} not 0xffff\n", self.cell4.len(), nz));
        s.push_str(&format!("slot grid {:?} tile {:?} block {:?} inv_scale {:?} unk_f {:?}\n", self.slot_grid, self.slot_tile, self.block_size, self.inv_scale, self.unk_f));
        let used: Vec<String> = self.slots.iter().enumerate().filter(|(_, &v)| v >= 0).map(|(i, &v)| format!("{i}:{v}")).collect();
        s.push_str(&format!("slots {} used {}: {}\n", self.slots.len(), used.len(), used.join(" ")));
        s.push_str(&format!("counts {:?} tail {} bytes: {}\n", self.counts, self.tail.len(), crate::hexdump(&self.tail, 0, 64)));
        s
    }
}
