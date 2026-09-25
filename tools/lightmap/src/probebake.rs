//! THE PROBES IN THE BAKE (row 10 wired): the transcribed probe passes of `probepass.rs` run inside the peel
//! loop over OUR peel layers, exactly where the game runs them (capture pwc2 frame 127448 / pwc6 frame 7534):
//!
//! * per direction the 32×16×32 RGBA16F volume (17157 / 8552) is CLEARED (`ClearRenderTargetView` right after
//!   the direction's start, every direction of both sweeps);
//! * after EVERY layer of the WORLD peel (the environment render = layer 0, then the geometry layers) one
//!   PS 17151 `ProbeGrid_SetILightDir` draw per probe block: `p = utof(cell) + SafetyOffset[cell]`,
//!   `(u, v, z) = (p, 1) · ProbeToShadow`, point SampleCmp GreaterEqual on the layer's depth, the layer's colour
//!   where the layer lies beyond the probe (`probepass::probe_set_ilightdir`, OutScale 1);
//! * after layer 1 (the first geometry layer) of the SKY sweep's UPWARD directions PS 17154 `AddSkyVisibility`
//!   into the R16F volume (17160 / 8555): `+= 4·D.y/N` where the 2×2 PCF says nothing is nearer than the probe;
//! * (the AddAmbient dispatch of the same directions — CS 17125 right after the environment render — is engineer A's
//!   `BakeParams::ambient_out`, wired beside this one; its xyz is the record's LAmbient);
//! * at the direction's end the two folds PS 1112 (`probepass::probe_fold`): the colour volume (17056 / 8451)
//!   `+= cur · (2/N, 2/N, 2/N, sweep 0 ? 1/N : 0)`, the signed volume (17059 / 8454) `+= cur · 4·D.y/N` on all
//!   four channels (frame 127448 eids 7150 / 7175: (1/128, 1/128, 1/128, 1/256) and 4·0.11708/256; frame 7534
//!   eids 13390 / 13415: (1/64, 1/64, 1/64, 0) and 4·(−0.16104)/128) — all 32 slices, no scissor.
//!
//! `ProbeToShadow` = ProbeToWorld · WorldPw01Shadow with ProbeToWorld = diag(cell, cell, cell) + the block's
//! `pos` (the probe cell (x, y, z) sits at world `cell·(x, y, z) + pos` — no half-cell offset: recovered from the
//! captured cbuffer to 1e-5 and, rebuilt here from the accumulate's WorldPw01Shadow, bit-identical on the 12
//! captured coefficients); the draw covers the block's cells: GS iSliceStart = min.z with max.z − min.z
//! triangles, scissor (min.x, min.y, max.x − min.x, max.y − min.y).
//!
//! At the end of the bake `finish` runs the CPU download (`probepass::download_probes`), lays the four atlases
//! out over the trailer's tile table (`probe_atlases`), encodes them (`encode_probe_atlases`: libwebp DEFAULT,
//! q 91 / Y-only q 80) and hands back the blob + the `frame_info` scales for the trailer. Every step above is the one that reproduced the
//! captured buffers / the saved file byte for byte (`lmtool probe-check`, `sweep1-check --direction`,
//! `final-check --probes / --records`).

use crate::passdiff::Buf;
use crate::probepass::{ProbeDraw, ProbeOpts, Volume3};

/// One probe block of the volume: its cell range in the volume (`min` inclusive, `max` exclusive — the
/// trailer's block record), its world position `pos` and the cell size (16 m on the tiny maps).
#[derive(Clone, Debug)]
pub struct ProbeBlockDef {
    pub min: [u32; 3],
    pub max: [u32; 3],
    pub pos: [f32; 3],
    pub cell: f32,
}

impl ProbeBlockDef {
    /// `ProbeToShadow` (the HLSL float4x3 as rows) for a peel whose `WorldPw01Shadow` is `pw01` (row-vector
    /// convention), in the CPU's f32 order: rows 0..2 = cell · pw01 rows, row 3 = pos · pw01[0..3] + pw01[3]
    /// (sum left to right) — bit-identical to the captured cbuffer on frame 127448's world peel.
    pub fn probe_to_shadow(&self, pw01: &[[f32; 4]; 4]) -> [[f32; 3]; 4] {
        let mut rows = [[0f32; 3]; 4];
        for i in 0..3 {
            for j in 0..3 {
                rows[i][j] = self.cell * pw01[i][j];
            }
        }
        for j in 0..3 {
            let a = self.pos[0] * pw01[0][j];
            let b = self.pos[1] * pw01[1][j];
            let c = self.pos[2] * pw01[2][j];
            rows[3][j] = ((a + b) + c) + pw01[3][j];
        }
        rows
    }

    /// The draw of this block into a probe volume: the slices `min.z..max.z`, the scissor over the block's
    /// (x, y) cells.
    pub fn draw(&self, pw01: &[[f32; 4]; 4], out_scale: f32) -> ProbeDraw {
        ProbeDraw {
            eid: 0,
            regs: ProbeDraw::regs_from_rows(&self.probe_to_shadow(pw01)),
            out_scale,
            slice_start: self.min[2],
            slice_count: self.max[2] - self.min[2],
            scissor: Some([self.min[0], self.min[1], self.max[0] - self.min[0], self.max[1] - self.min[1]]),
        }
    }
}

/// The probe state of a bake: the accumulators and the per-direction volume.
pub struct ProbeBake {
    pub dims: [u32; 3],
    pub blocks: Vec<ProbeBlockDef>,
    /// `TMapProbeSafetyOffset` (SNORM16 ×4, in cells): the game nudges probes that sit inside geometry (one
    /// of 392 on pwc-day); None = no offsets (the open item of this row).
    pub offsets: Option<Volume3>,
    /// The colour fold (17056): Σ_sweeps Σ_dirs 2/N · the first surface's colour, α = Σ_{sweep 0} 1/N · [written].
    pub colour: Volume3,
    /// The signed fold (17059): Σ 4·D.y/N · (colour, α).
    pub updown: Volume3,
    /// The sky visibility (17160, R16F): Σ_{sky sweep, D.y > 0} 4·D.y/N · [nothing nearer along D].
    pub skyvis: Volume3,
    /// The direction's volume (17157), cleared per direction.
    pub cur: Volume3,
    pub opts: ProbeOpts,
    /// Counters: directions run, layers run, probes written, sky-visibility adds.
    pub n_dirs: usize,
    pub n_layers: usize,
    pub n_written: usize,
    pub n_sky_adds: usize,
    /// Per direction: (sweep, D, the writes per world layer, the probes non-zero at the end, of which α > 0).
    pub dir_log: Vec<(u32, [f32; 3], Vec<usize>, usize, usize)>,
    /// `LMTOOL_PROBE_TRACE=x,y,z`: the direction (within 0.5°) whose per-probe samples are traced (`trace` lines).
    pub trace_dir: Option<[f32; 3]>,
    pub trace: Vec<String>,
    cur_dir: Option<(u32, [f32; 3])>,
    cur_writes: Vec<usize>,
}

impl ProbeBake {
    pub fn new(dims: [u32; 3], blocks: Vec<ProbeBlockDef>, offsets: Option<Volume3>) -> ProbeBake {
        let [w, h, d] = dims;
        ProbeBake {
            dims,
            blocks,
            offsets,
            colour: Volume3::new(w, h, d, 4),
            updown: Volume3::new(w, h, d, 4),
            skyvis: Volume3::new(w, h, d, 1),
            cur: Volume3::new(w, h, d, 4),
            opts: ProbeOpts::default(),
            n_dirs: 0,
            n_layers: 0,
            n_written: 0,
            n_sky_adds: 0,
            dir_log: Vec::new(),
            trace_dir: std::env::var("LMTOOL_PROBE_TRACE").ok().and_then(|s| { let v: Vec<f32> = s.split(',').filter_map(|t| t.trim().parse().ok()).collect(); if v.len() == 3 { Some([v[0], v[1], v[2]]) } else { None } }),
            trace: Vec::new(),
            cur_dir: None,
            cur_writes: Vec::new(),
        }
    }

    fn tracing(&self, dir: [f32; 3]) -> bool {
        match self.trace_dir { Some(t) => (t[0] * dir[0] + t[1] * dir[1] + t[2] * dir[2]) > 0.99996, None => false }
    }

    /// The direction's start: the volume is cleared to 0.
    pub fn begin_direction(&mut self) {
        self.begin_direction_of(0, [0.0, 0.0, 0.0]);
    }

    /// As `begin_direction`, naming the sweep and the direction for the per-direction log.
    pub fn begin_direction_of(&mut self, sweep: u32, dir: [f32; 3]) {
        for v in self.cur.data.iter_mut() {
            *v = 0.0;
        }
        self.n_dirs += 1;
        self.cur_dir = Some((sweep, dir));
        self.cur_writes.clear();
    }

    /// One WORLD-peel layer `k` (0 = the environment render) of direction `dir` (unit, world) in a sweep of
    /// `n_dirs` directions: the SetILightDir draw of every block; after layer 1 of the sky sweep's upward
    /// directions the sky visibility. `colour` (3 channels) and `depth` (1 channel, z01) are the layer's targets,
    /// `pw01` the peel's WorldPw01Shadow.
    pub fn world_layer(&mut self, k: usize, pw01: &[[f32; 4]; 4], colour: &Buf, depth: &Buf, dir: [f32; 3], n_dirs: usize, sky_sweep: bool) {
        self.n_layers += 1;
        let mut written = 0;
        for b in &self.blocks {
            let d = b.draw(pw01, 1.0);
            if self.tracing(dir) {
                // every probe of the block: its shadow coordinates, the texel, the stored depth, the compare, the colour
                for z in d.slice_start..(d.slice_start + d.slice_count).min(self.cur.d) {
                    for y in b.min[1]..b.max[1] {
                        for x in b.min[0]..b.max[0] {
                            let p = crate::probepass::probe_point(x, y, z, self.offsets.as_ref());
                            let sh = crate::probepass::to_shadow(p, &d.regs, self.opts.fma);
                            let reference = if self.opts.clamp_ref { sh[2].clamp(0.0, 1.0) } else { sh[2] };
                            let (tx, ty) = (crate::probepass::texel_point(sh[0], depth.w), crate::probepass::texel_point(sh[1], depth.h));
                            let stored = depth.get(tx, ty, 0);
                            let c = [colour.get(tx, ty, 0), colour.get(tx, ty, 1), colour.get(tx, ty, 2)];
                            self.trace.push(format!("layer {k}\tprobe {x} {y} {z}\tuv {:.5} {:.5}\tz {:.5}\ttexel {tx} {ty}\tstored {stored:.5}\tpass {}\tcolour {:.5} {:.5} {:.5}", sh[0], sh[1], sh[2], reference >= stored, c[0], c[1], c[2]));
                        }
                    }
                }
            }
            written += crate::probepass::probe_set_ilightdir(&mut self.cur, &d, colour, depth, self.offsets.as_ref(), self.opts);
        }
        self.n_written += written;
        self.cur_writes.push(written);
        let upward = sky_sweep && dir[1] > 0.0;
        if upward && k == 1 {
            // OutScale = 4·D.y/N (frame 127448 eid 3022: 0.0018293475 = 4·0.11707824/256)
            let s = 4.0f32 * dir[1] / n_dirs as f32;
            for b in &self.blocks {
                let d = b.draw(pw01, s);
                self.n_sky_adds += crate::probepass::probe_add_sky_visibility(&mut self.skyvis, &d, depth, self.offsets.as_ref(), self.opts);
            }
        }
    }

    /// The direction's end: the two folds (all slices, the whole face, One/One f16).
    pub fn end_direction(&mut self, dir: [f32; 3], n_dirs: usize, sweep: u32) {
        // the per-direction log: the probes non-zero in the direction's volume and those with α > 0
        let (mut nz, mut na) = (0usize, 0usize);
        for b in &self.blocks {
            for z in b.min[2]..b.max[2] { for y in b.min[1]..b.max[1] { for x in b.min[0]..b.max[0] {
                let a = self.cur.get(x, y, z, 3);
                if a != 0.0 || self.cur.get(x, y, z, 0) != 0.0 || self.cur.get(x, y, z, 1) != 0.0 || self.cur.get(x, y, z, 2) != 0.0 { nz += 1; }
                if a > 0.0 { na += 1; }
            } } }
        }
        let writes = std::mem::take(&mut self.cur_writes);
        self.dir_log.push((sweep, dir, writes, nz, na));
        let n = n_dirs as f32;
        let two = 2.0f32 / n;
        let a = if sweep == 0 { 1.0f32 / n } else { 0.0 };
        let s = 4.0f32 * dir[1] / n;
        let depth = self.dims[2];
        crate::probepass::probe_fold(&mut self.colour, &self.cur, [two, two, two, a], 0, depth);
        crate::probepass::probe_fold(&mut self.updown, &self.cur, [s, s, s, s], 0, depth);
    }

    /// The end of the bake: the CPU download over the blocks' ranges → the four atlases over `tiles` (the
    /// trailer's per-level tile positions, `tiles[b][level − min.y]`) → the WEBPs. Returns the blob, the
    /// `frame_info` end offsets of the first three images, the scales (max0, max2, 1e-5) and the validity map,
    /// or None without libwebp.
    pub fn finish(&self, tiles: &[Vec<Option<(u32, u32)>>], atlas: (u32, u32)) -> Option<ProbeResult> {
        // one download per block range (the scales are over ALL probes of the volume: the download takes the
        // whole volume's maxima, the block ranges only select the probes to write)
        let mut imgs: [Vec<u8>; 4] = { let n = (atlas.0 * atlas.1 * 3) as usize; [vec![128u8; n], vec![128u8; n], vec![127u8; n], vec![128u8; n]] };
        let mut valid = vec![true; (atlas.0 * atlas.1) as usize];
        let mut scales = [0f32; 3];
        let (mut n_probes, mut n_valid) = (0usize, 0usize);
        for (bi, b) in self.blocks.iter().enumerate() {
            let dl = crate::probepass::download_probes(&self.colour, &self.updown, Some(&self.skyvis), (b.min, b.max));
            scales = [dl.max0, dl.max2, 1e-5];
            let tl = tiles.get(bi).map(|v| v.as_slice()).unwrap_or(&[]);
            for ((x, y, z), _rgb, ok, _sky, _sq) in &dl.probes {
                n_probes += 1;
                if *ok { n_valid += 1; }
                if let Some(Some((tx, ty))) = tl.get((*y - b.min[1]) as usize) {
                    let (px, py) = (tx + (x - b.min[0]), ty + (z - b.min[2]));
                    if px < atlas.0 && py < atlas.1 && !*ok { valid[(py * atlas.0 + px) as usize] = false; }
                }
            }
            let part = crate::probepass::probe_atlases(&dl, b.min, tl, atlas.0, atlas.1);
            // merge: the block's tiles are disjoint from the others'
            for k in 0..4 {
                for (o, v) in imgs[k].iter_mut().zip(part[k].iter()) {
                    let fill = if k == 2 { 127 } else { 128 };
                    if *v != fill {
                        *o = *v;
                    }
                }
            }
        }
        let enc = crate::probepass::encode_probe_atlases(&imgs, atlas.0, atlas.1)?;
        let mut blob = Vec::new();
        let mut ends = [0u32; 3];
        for (k, e) in enc.iter().enumerate() {
            blob.extend_from_slice(e);
            if k < 3 {
                ends[k] = blob.len() as u32;
            }
        }
        Some(ProbeResult { blob, ends, scales, images: imgs, valid, n_probes, n_valid })
    }
}

impl std::fmt::Debug for ProbeBake {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "ProbeBake({:?}, {} blocks, {} dirs, {} layers, {} written, {} sky adds)", self.dims, self.blocks.len(), self.n_dirs, self.n_layers, self.n_written, self.n_sky_adds)
    }
}

pub struct ProbeResult {
    pub blob: Vec<u8>,
    pub ends: [u32; 3],
    pub scales: [f32; 3],
    pub images: [Vec<u8>; 4],
    /// Per atlas pixel: false where the probe there is invalid (α < 0.5) — the trailer's cell4 bits.
    pub valid: Vec<bool>,
    pub n_probes: usize,
    pub n_valid: usize,
}

/// The blocks and the tile tables of a `volume::Volume` (the port's `probes::build` output or a template's
/// trailer): per block (min, max, pos, cell) and its `slices` (per level from min.y: the tile's (x, y) or None).
pub fn blocks_of(v: &crate::volume::Volume) -> (Vec<ProbeBlockDef>, Vec<Vec<Option<(u32, u32)>>>) {
    let blocks = v.blocks.iter().map(|b| ProbeBlockDef { min: b.min, max: b.max, pos: b.pos, cell: b.cell[0] }).collect();
    let tiles = v.blocks.iter().map(|b| b.slices.clone()).collect();
    (blocks, tiles)
}

/// The atlas size the tile table needs: the largest (x + w, y + h) over the blocks' tiles.
pub fn atlas_size(v: &crate::volume::Volume) -> (u32, u32) {
    let (mut w, mut h) = (0u32, 0u32);
    for b in &v.blocks {
        let (bw, bh) = (b.max[0] - b.min[0], b.max[2] - b.min[2]);
        for t in b.slices.iter().flatten() {
            w = w.max(t.0 + bw);
            h = h.max(t.1 + bh);
        }
    }
    (w.max(1), h.max(1))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_probe_to_shadow_rows_are_cell_times_pw01_plus_pos_dot_pw01() {
        // frame 127448's world peel: the accumulate's WorldPw01Shadow (eid 2787) → the probe draw's ProbeToShadow (eid 2758)
        let pw01 = [
            [-0.0003559289616532624f32, 0.00009223862434737384, -0.00013109228166285902, 0.0],
            [0.0, -0.002249176148325205, -0.000044425702071748674, 0.0],
            [0.00013206513540353626, 0.0002485924633219838, -0.00035330699756741524, 0.0],
            [0.7292365431785583, 0.31068041920661926, 1.0001277923583984, 1.0],
        ];
        let cap = [
            [-0.005694863386452198f32, 0.0014758179895579815, -0.0020974765066057444],
            [0.0, -0.03598681837320328, -0.0007108112331479788],
            [0.00211304216645658, 0.003977479413151741, -0.005652911961078644],
            [0.5601815581321716, 0.4556904137134552, 0.9431222677230835],
        ];
        let b = ProbeBlockDef { min: [22, 4, 20], max: [29, 12, 27], pos: [472.0, -46.0, -8.0], cell: 16.0 };
        let rows = b.probe_to_shadow(&pw01);
        for i in 0..4 {
            for j in 0..3 {
                assert_eq!(rows[i][j].to_bits(), cap[i][j].to_bits(), "row {i} col {j}: ours {} captured {}", rows[i][j], cap[i][j]);
            }
        }
        let d = b.draw(&pw01, 1.0);
        assert_eq!((d.slice_start, d.slice_count), (20, 7));
        assert_eq!(d.scissor, Some([22, 4, 7, 8]));
    }

    #[test]
    fn a_direction_folds_its_volume_with_the_sweep_scales() {
        let b = ProbeBlockDef { min: [0, 0, 0], max: [2, 1, 1], pos: [0.0, 0.0, 0.0], cell: 16.0 };
        let mut pb = ProbeBake::new([2, 1, 1], vec![b], None);
        // a toy projection (u = 4·cx + 0.5, v = 4·cz + 0.5, depth = 16·cy + 0.5): a layer at depth 0.25 lies in front of
        // the probes (reference 0.5 ≥ 0.25: the SampleCmp GreaterEqual passes, the layer's colour is taken)
        let pw01 = [[0.25, 0.0, 0.0, 0.0], [0.0, 0.0, 1.0, 0.0], [0.0, 0.25, 0.0, 0.0], [0.5, 0.5, 0.5, 1.0]];
        let mut colour = Buf::new(4, 4, 3);
        let mut depth = Buf::new(4, 4, 1);
        for y in 0..4 { for x in 0..4 { colour.set(x, y, 0, 0.5); depth.set(x, y, 0, 0.25); } }
        pb.begin_direction();
        pb.world_layer(0, &pw01, &colour, &depth, [0.0, -1.0, 0.0], 256, true);
        assert_eq!(pb.n_written, 2);
        assert_eq!(pb.cur.get(0, 0, 0, 0), 0.5);
        assert_eq!(pb.cur.get(0, 0, 0, 3), 1.0);
        pb.end_direction([0.0, -1.0, 0.0], 256, 0);
        // colour += 0.5 · 2/256; α += 1/256; updown += 0.5 · 4·(−1)/256
        assert_eq!(pb.colour.get(0, 0, 0, 0), crate::probepass::blend_add_f16(0.0, 0.5 * (2.0 / 256.0)));
        assert_eq!(pb.colour.get(0, 0, 0, 3), crate::probepass::blend_add_f16(0.0, 1.0 / 256.0));
        assert_eq!(pb.updown.get(0, 0, 0, 0), crate::probepass::blend_add_f16(0.0, 0.5 * (4.0 * -1.0 / 256.0)));
        // a downward direction adds no sky visibility
        assert_eq!(pb.n_sky_adds, 0);
        // sweep 1 adds no alpha
        pb.begin_direction();
        pb.world_layer(0, &pw01, &colour, &depth, [0.0, -1.0, 0.0], 128, false);
        let a_before = pb.colour.get(0, 0, 0, 3);
        pb.end_direction([0.0, -1.0, 0.0], 128, 1);
        assert_eq!(pb.colour.get(0, 0, 0, 3), a_before);
    }
}


/// Where the probe blocks / tiles come from: the port's layout (`probes::layout` + the template's trailer constants)
/// or a saved map's trailer (`--probe-layout-from`).
pub struct ProbeLayoutSrc {
    pub dims: [u32; 3],
    pub blocks: Vec<ProbeBlockDef>,
    pub tiles: Vec<Vec<Option<(u32, u32)>>>,
    pub atlas: (u32, u32),
    kind: ProbeLayoutKind,
}

enum ProbeLayoutKind {
    Port { lay: crate::probes::ProbeLayout, template: crate::volume::Volume, grid: crate::probes::SlotGrid },
    Trailer(crate::volume::Volume),
}

impl ProbeLayoutSrc {
    pub fn from_layout(lay: crate::probes::ProbeLayout, template: crate::volume::Volume, grid: crate::probes::SlotGrid) -> ProbeLayoutSrc {
        let v = crate::probes::volume_for(&lay, &template, &grid, [1.0, 1.0, 1e-5], [0, 0, 0], &|_, _| false);
        let (blocks, tiles) = blocks_of(&v);
        let dims = [crate::probes::BLOCK_CELLS[0] * lay.cols, crate::probes::BLOCK_CELLS[1] * lay.rows, crate::probes::BLOCK_CELLS[2]];
        let atlas = (lay.atlas_w, lay.atlas_h);
        ProbeLayoutSrc { dims, blocks, tiles, atlas, kind: ProbeLayoutKind::Port { lay, template, grid } }
    }
    /// A saved map's trailer: its blocks, tile table and grid; the atlas = the four WEBPs' size (the tiles' extent).
    pub fn from_volume(v: crate::volume::Volume) -> ProbeLayoutSrc {
        let (blocks, tiles) = blocks_of(&v);
        let dims = v.grid;
        let atlas = atlas_size(&v);
        ProbeLayoutSrc { dims, blocks, tiles, atlas, kind: ProbeLayoutKind::Trailer(v) }
    }
    /// The trailer with the download's scales / blob ends and validity mask.
    pub fn volume(&self, scales: [f32; 3], ends: [u32; 3], invalid: &dyn Fn(u32, u32) -> bool) -> crate::volume::Volume {
        match &self.kind {
            ProbeLayoutKind::Port { lay, template, grid } => crate::probes::volume_for(lay, template, grid, scales, ends, invalid),
            ProbeLayoutKind::Trailer(v) => {
                let mut out = v.clone();
                while out.frame_info.len() < 3 { out.frame_info.push((1.0, 0)); }
                for k in 0..3 { out.frame_info[k] = (scales[k], ends[k]); }
                if let Some((cw4, ch4)) = out.cell4_dims {
                    let mut cell4 = vec![0xffffu16; (cw4 * ch4) as usize];
                    for ay in 0..self.atlas.1 { for ax in 0..self.atlas.0 { if invalid(ax, ay) { let ci = ((ay / 4) * cw4 + ax / 4) as usize; if ci < cell4.len() { cell4[ci] &= !(1u16 << ((ay % 4) * 4 + ax % 4)); } } } }
                    out.cell4 = cell4;
                }
                out
            }
        }
    }
}

impl ProbeBake {
    /// Dump the three accumulators as raw f32 volumes (`probe-colour.f32`, `probe-updown.f32`, `probe-skyvis.f32`:
    /// x fastest, then y, then z; 4 / 4 / 1 channels) for `lmtool probe-chain-check`.
    pub fn dump(&self, dir: &std::path::Path) -> std::io::Result<()> {
        let w = |name: &str, v: &Volume3| -> std::io::Result<()> {
            let mut b = Vec::with_capacity(v.data.len() * 4 + 16);
            for x in [v.w, v.h, v.d, v.channels] { b.extend_from_slice(&x.to_le_bytes()); }
            for f in &v.data { b.extend_from_slice(&f.to_le_bytes()); }
            std::fs::write(dir.join(name), b)
        };
        w("probe-colour.f32", &self.colour)?;
        w("probe-updown.f32", &self.updown)?;
        w("probe-skyvis.f32", &self.skyvis)?;
        let mut t = String::from("sweep\tdx\tdy\tdz\twrites_per_layer\tnonzero\talpha_pos\n");
        for (sw, d, wr, nz, na) in &self.dir_log { t.push_str(&format!("{sw}\t{:.6}\t{:.6}\t{:.6}\t{}\t{nz}\t{na}\n", d[0], d[1], d[2], wr.iter().map(|w| w.to_string()).collect::<Vec<_>>().join(","))); }
        std::fs::write(dir.join("probe-dirs.tsv"), t)?;
        if !self.trace.is_empty() { std::fs::write(dir.join("probe-trace.tsv"), self.trace.join("\n") + "\n")?; }
        Ok(())
    }
}

/// Read a volume `dump` wrote.
pub fn load_dump(path: &std::path::Path) -> std::io::Result<Volume3> {
    let b = std::fs::read(path)?;
    let u = |o: usize| u32::from_le_bytes([b[o], b[o + 1], b[o + 2], b[o + 3]]);
    let (w, h, d, c) = (u(0), u(4), u(8), u(12));
    let mut v = Volume3::new(w, h, d, c);
    for (i, f) in v.data.iter_mut().enumerate() { let o = 16 + i * 4; *f = f32::from_le_bytes([b[o], b[o + 1], b[o + 2], b[o + 3]]); }
    Ok(v)
}

/// `lmtool probe-chain-check DIR ROOT MAP [--frame 7537]`: the bake's dumped probe accumulators (DIR/probe-*.f32 from
/// `--chain-final-dir`) against the capture's END volumes of `frame` (the folds 8451 / 8454 and the sky visibility
/// 8555 of pwc6's frame 7537 — another run of the same bake), the download of OUR volumes against the saved map's
/// trailer scales, and our four probe WEBPs (rebuilt from the dump over the map's tile table) against the save's.
pub fn chain_check(dir: &std::path::Path, root: &std::path::Path, map: &str, frame: u32) -> Result<(), String> {
    let colour = load_dump(&dir.join("probe-colour.f32")).map_err(|e| format!("probe-colour.f32: {e}"))?;
    let updown = load_dump(&dir.join("probe-updown.f32")).map_err(|e| format!("probe-updown.f32: {e}"))?;
    let skyvis = load_dump(&dir.join("probe-skyvis.f32")).map_err(|e| format!("probe-skyvis.f32: {e}"))?;
    println!("ours: colour {}×{}×{}, {} non-zero probes; skyvis {} non-zero", colour.w, colour.h, colour.d, colour.count_nonzero(), skyvis.count_nonzero());
    let txt = std::fs::read_to_string(root.join("MANIFEST.json")).map_err(|e| format!("MANIFEST.json: {e}"))?;
    let m = crate::passdiff::read_manifest(&txt)?;
    let last = |pass: &str| -> Option<Volume3> {
        let e = m.passes.iter().filter(|e| e.pass == pass && e.frame == Some(frame)).max_by_key(|e| e.eid.unwrap_or(0))?;
        crate::probecheck::load_volume(root, e).ok()
    };
    // the captured volumes are 32×16×32 (one block at the origin); ours may be a bigger label grid — compare the
    // overlapping cells
    let cmp = |name: &str, ours: &Volume3, theirs: Option<Volume3>, channels: u32| {
        let Some(t) = theirs else { println!("{name}: no captured volume in frame {frame}"); return };
        let (w, h, d) = (ours.w.min(t.w), ours.h.min(t.h), ours.d.min(t.d));
        let (mut n, mut exact, mut ulp1, mut beyond, mut maxd) = (0usize, 0usize, 0usize, 0usize, 0f32);
        let mut worst = (0u32, 0u32, 0u32, 0u32, 0f32, 0f32);
        let (mut sum_o, mut sum_t) = (0f64, 0f64);
        for z in 0..d { for y in 0..h { for x in 0..w { for c in 0..channels {
            let (o, g) = (ours.get(x, y, z, c), t.get(x, y, z, c));
            if o == 0.0 && g == 0.0 { continue; }
            n += 1; sum_o += o as f64; sum_t += g as f64;
            let dd = (o - g).abs();
            if dd == 0.0 { exact += 1; } else {
                let q = crate::gpucmp::quantum(crate::gpucmp::Fmt::F16, c, g);
                if dd <= q * 1.001 { ulp1 += 1; } else { beyond += 1; }
                if dd > maxd { maxd = dd; worst = (x, y, z, c, g, o); }
            }
        } } } }
        println!("{name}: {n} values — {exact} bit-identical, {ulp1} within 1 f16 ulp, {beyond} beyond; max |Δ| {maxd:.6} at ({},{},{}) ch {}: captured {:.6} ours {:.6}; mean ours/captured {:.4}", worst.0, worst.1, worst.2, worst.3, worst.4, worst.5, if sum_t != 0.0 { sum_o / sum_t } else { 0.0 });
    };
    cmp("colour fold (17056 / 8451)", &colour, last("probe3d_fold0"), 4);
    cmp("signed fold (17059 / 8454)", &updown, last("probe3d_fold1"), 4);
    cmp("sky visibility (17160 / 8555)", &skyvis, last("probe3d_skyvis"), 1);
    // the file side: the map's trailer → the download of OUR volumes → the atlases → the WEBPs vs the save's
    let mm = crate::mapio::load(map)?;
    let dd = mm.chunk.data.as_ref().ok_or("the map has no lightmap data")?;
    let v = crate::volume::Volume::parse(&dd.cache.trailer)?;
    let src = ProbeLayoutSrc::from_volume(v.clone());
    let mut pb = ProbeBake::new(src.dims, src.blocks.clone(), None);
    pb.colour = colour; pb.updown = updown; pb.skyvis = skyvis;
    let r = pb.finish(&src.tiles, src.atlas).ok_or("no libwebp")?;
    println!("download: {} of {} probes valid; scales max0 {} (file {}) max2 {} (file {})", r.n_valid, r.n_probes, r.scales[0], v.frame_info[0].0, r.scales[1], v.frame_info[1].0);
    let parts = crate::volume::split_probe_blob(&dd.frames[0].images[2], &v.frame_info);
    let ours = crate::volume::split_probe_blob(&r.blob, &[(r.scales[0], r.ends[0]), (r.scales[1], r.ends[1]), (r.scales[2], r.ends[2])]);
    for k in 0..4.min(parts.len()).min(ours.len()) {
        let dec = crate::filecheck::cmp_decoded(&parts[k], &ours[k]);
        println!("probe image {k}: {}{}", crate::filecheck::cmp_bytes(&parts[k], &ours[k]), dec.map(|(n, ex, w1, w2, mx)| format!("; decoded {ex} of {n} exact, {w1} within 1, {w2} within 2, max {mx}")).unwrap_or_default());
    }
    Ok(())
}

/// The probe layout from RE-6's transcribed CHUNKING (`probechunk::for_records`, FUN_14021c980 / FUN_14021d730) — the
/// no-box path's block records: per chunk record the block = (origin = slot, min = amin, max = amax, cell, pos = the
/// world origin of atlas index 0), every level min.y..max.y stored, the tiles (max.x − min.x) × (max.z − min.z) packed
/// per block in DESCENDING level order into the smallest square grid of tile cells (the saves: hill4's 4 levels of
/// 10×10 in a 20×20 atlas at (0,0), (10,0), (0,10), (10,10) for levels 7, 6, 5, 4; pwc-day's 8 levels of 7×7 in 21×21
/// — the game's packer takes (0,0), (7,0), (14,0), (0,7), (0,14), (7,7), (14,7), (7,14): the first row, the first
/// column, then the rest; the rule for more blocks (the giant's 44) is not transcribed, so the table here is
/// self-consistent, not the game's byte for byte). The trailer's slot grid / tile / pitch / origin fields come from the
/// chunking's grid (`slot_origin` = the decoration offset, e.g. (0, −38, 0)).
pub fn layout_from_chunking(c: &crate::probechunk::Chunking, template: &crate::volume::Volume, slot_origin: [f32; 3]) -> Option<ProbeLayoutSrc> {
    if c.records.is_empty() {
        return None;
    }
    let mut blocks: Vec<crate::volume::Block> = Vec::new();
    // the tiles: (block, level, w, h) in placement order
    let mut order: Vec<(usize, u32, u32, u32)> = Vec::new();
    for (bi, r) in c.records.iter().enumerate() {
        let u = |v: [i32; 3]| [v[0].max(0) as u32, v[1].max(0) as u32, v[2].max(0) as u32];
        let (min, max) = (u(r.amin), u(r.amax));
        let b = crate::volume::Block { origin: u(r.slot), min, max, cell: r.cell, pos: r.origin, slices: vec![None; (max[1].saturating_sub(min[1])) as usize] };
        for level in (min[1]..max[1]).rev() {
            order.push((bi, level, max[0] - min[0], max[2] - min[2]));
        }
        blocks.push(b);
    }
    // the packing: a square grid of the largest tile, filled first row → first column → the rest row-major (the pwc-day
    // order); grows until every tile fits
    let (tw, th) = order.iter().fold((1u32, 1u32), |(w, h), t| (w.max(t.2), h.max(t.3)));
    let n = order.len() as u32;
    let cols = ((n as f32).sqrt().ceil() as u32).max(1);
    let rows = (n + cols - 1) / cols;
    let mut cells: Vec<(u32, u32)> = Vec::new();
    for cx in 0..cols { cells.push((cx, 0)); }
    for cy in 1..rows { cells.push((0, cy)); }
    for cy in 1..rows { for cx in 1..cols { cells.push((cx, cy)); } }
    let (aw, ah) = (cols * tw, rows * th);
    for (i, &(bi, level, _w, _h)) in order.iter().enumerate() {
        let (cx, cy) = cells[i];
        let sl = (level - blocks[bi].min[1]) as usize;
        blocks[bi].slices[sl] = Some((cx * tw, cy * th));
    }
    let slot_count = (c.chunk_counts[0] * c.chunk_counts[1] * c.chunk_counts[2]) as usize;
    let mut slots = vec![-1i32; slot_count];
    for (bi, r) in c.records.iter().enumerate() {
        let idx = (r.chunk[0] + c.chunk_counts[0] * (r.chunk[1] + c.chunk_counts[1] * r.chunk[2])) as usize;
        if idx < slots.len() { slots[idx] = bi as i32; }
    }
    let tile = [crate::probechunk::CHUNK[0] as u32, crate::probechunk::CHUNK[1] as u32, crate::probechunk::CHUNK[2] as u32];
    let pitch = [tile[0] as f32 * c.grid.cell[0], tile[1] as f32 * c.grid.cell[1], tile[2] as f32 * c.grid.cell[2]];
    let mut frame_info = template.frame_info.clone();
    while frame_info.len() < 3 { frame_info.push((1.0, 0)); }
    let cw4 = (aw + 3) / 4;
    let v = crate::volume::Volume {
        head_consts: template.head_consts.clone(),
        frame_info,
        grid: c.atlas,
        blocks,
        cell4_dims: Some((cw4, (ah + 3) / 4)),
        cell4: vec![0xffffu16; (cw4 * ((ah + 3) / 4)) as usize],
        slot_grid: c.chunk_counts,
        slot_tile: tile,
        block_size: [crate::probechunk::TILE[0] as u32, crate::probechunk::TILE[1] as u32, crate::probechunk::TILE[2] as u32],
        inv_scale: [1.0 / pitch[0], 1.0 / pitch[1], 1.0 / pitch[2]],
        unk_f: [-slot_origin[0] / pitch[0], -slot_origin[1] / pitch[1], -slot_origin[2] / pitch[2]],
        slots,
        counts: template.counts,
        tail: template.tail.clone(),
    };
    let mut src = ProbeLayoutSrc::from_volume(v);
    src.atlas = (aw, ah);
    Some(src)
}
