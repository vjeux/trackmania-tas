//! THE DIRECTION-RANGE SPLIT OF A SWEEP ACROSS BOXES with an exact ordered merge.
//!
//! Every accumulator of a sweep folds per direction in the game's issue order with a rounding after each
//! add (the H-basis atlas: `acc = q(acc + w·L)`; the probe folds and the sky-visibility adds: One/One f16
//! blends), so the directions cannot be summed out of order — but what a direction CONTRIBUTES is a pure
//! function of the scene and the sweep's input (the previous sweep's field): `sel` (the incoming radiance
//! per sub-sample, the peel lookup), `occl` (whether a surface wrote it), the probe volume of the direction
//! and its sky-visibility adds. A box baking directions `a..b` writes those per direction
//! (`lmtool bake --dir-range a..b --contrib-out DIR`), and the merge (`--merge-contrib DIR...`) replays
//! every direction of the sweep in order through the same accumulate code (the weights `w` recomputed from
//! the same sub-sample normals, the cover counts from the same sets), then finalises as a single-box bake
//! would — bit-identical to it.
//!
//! One file per (sweep, direction): `sweep<s>-dir<d>.contrib` — a header, `sel` for the sub-samples facing
//! the direction (packed R11G11B10 when every value survives the pack exactly, else raw f32×3), the `occl`
//! bits, the probe volume's non-zero probes and the logged sky-visibility adds.

use std::io::{Read, Write};
use std::path::{Path, PathBuf};

pub const MAGIC: &[u8; 8] = b"LMCTRB1\0";

/// A direction's contribution.
#[derive(Clone, Debug, Default)]
pub struct DirContrib {
    pub sweep: u32,
    pub di: u32,
    pub n_subs: u32,
    /// `sel` for every sub-sample of the set with n·D > 0, in set order.
    pub sel: Vec<[f32; 3]>,
    /// `occl` bits for every sub-sample of the set (bit i = sub-sample i).
    pub occl_bits: Vec<u64>,
    /// The direction's probe volume: (flat index, the four channels) of the non-zero probes.
    pub probe_cur: Vec<(u32, [f32; 4])>,
    /// The sky-visibility adds of the direction in execution order: (flat index, src).
    pub sky_adds: Vec<(u32, f32)>,
}

pub fn file_name(sweep: u32, di: u32) -> String {
    format!("sweep{sweep}-dir{di:05}.contrib")
}

fn put_u32(v: &mut Vec<u8>, x: u32) { v.extend_from_slice(&x.to_le_bytes()); }
fn put_f32(v: &mut Vec<u8>, x: f32) { v.extend_from_slice(&x.to_bits().to_le_bytes()); }

impl DirContrib {
    /// The bytes of the file.
    pub fn encode(&self) -> Vec<u8> {
        let mut v = Vec::with_capacity(32 + self.sel.len() * 4 + self.occl_bits.len() * 8 + self.probe_cur.len() * 20 + self.sky_adds.len() * 8);
        v.extend_from_slice(MAGIC);
        put_u32(&mut v, self.sweep);
        put_u32(&mut v, self.di);
        put_u32(&mut v, self.n_subs);
        // sel: packed when exact
        let packed: Option<Vec<u32>> = {
            let mut out = Vec::with_capacity(self.sel.len());
            let mut ok = true;
            for s in &self.sel {
                let p = crate::gpufmt::pack_r11g11b10(*s, crate::gpufmt::Rounding::NearestEven);
                if crate::gpufmt::unpack_r11g11b10(p) != *s { ok = false; break; }
                out.push(p);
            }
            if ok { Some(out) } else { None }
        };
        put_u32(&mut v, if packed.is_some() { 0 } else { 1 });
        put_u32(&mut v, self.sel.len() as u32);
        match &packed {
            Some(p) => for x in p { put_u32(&mut v, *x); },
            None => for s in &self.sel { put_f32(&mut v, s[0]); put_f32(&mut v, s[1]); put_f32(&mut v, s[2]); },
        }
        put_u32(&mut v, self.occl_bits.len() as u32);
        for w in &self.occl_bits { v.extend_from_slice(&w.to_le_bytes()); }
        put_u32(&mut v, self.probe_cur.len() as u32);
        for (i, c) in &self.probe_cur { put_u32(&mut v, *i); for k in 0..4 { put_f32(&mut v, c[k]); } }
        put_u32(&mut v, self.sky_adds.len() as u32);
        for (i, s) in &self.sky_adds { put_u32(&mut v, *i); put_f32(&mut v, *s); }
        v
    }

    pub fn decode(b: &[u8]) -> Result<DirContrib, String> {
        let mut o = 0usize;
        let take = |o: &mut usize, n: usize| -> Result<&[u8], String> { if *o + n > b.len() { return Err("truncated contribution file".into()); } let s = &b[*o..*o + n]; *o += n; Ok(s) };
        if take(&mut o, 8)? != MAGIC { return Err("not a contribution file".into()); }
        let u32_at = |o: &mut usize| -> Result<u32, String> { Ok(u32::from_le_bytes(take(o, 4)?.try_into().unwrap())) };
        let f32_at = |o: &mut usize| -> Result<f32, String> { Ok(f32::from_bits(u32::from_le_bytes(take(o, 4)?.try_into().unwrap()))) };
        let sweep = u32_at(&mut o)?;
        let di = u32_at(&mut o)?;
        let n_subs = u32_at(&mut o)?;
        let mode = u32_at(&mut o)?;
        let n_sel = u32_at(&mut o)? as usize;
        let mut sel = Vec::with_capacity(n_sel);
        for _ in 0..n_sel {
            if mode == 0 { sel.push(crate::gpufmt::unpack_r11g11b10(u32_at(&mut o)?)); } else { sel.push([f32_at(&mut o)?, f32_at(&mut o)?, f32_at(&mut o)?]); }
        }
        let n_occ = u32_at(&mut o)? as usize;
        let mut occl_bits = Vec::with_capacity(n_occ);
        for _ in 0..n_occ { occl_bits.push(u64::from_le_bytes(take(&mut o, 8)?.try_into().unwrap())); }
        let n_pc = u32_at(&mut o)? as usize;
        let mut probe_cur = Vec::with_capacity(n_pc);
        for _ in 0..n_pc { let i = u32_at(&mut o)?; probe_cur.push((i, [f32_at(&mut o)?, f32_at(&mut o)?, f32_at(&mut o)?, f32_at(&mut o)?])); }
        let n_sa = u32_at(&mut o)? as usize;
        let mut sky_adds = Vec::with_capacity(n_sa);
        for _ in 0..n_sa { let i = u32_at(&mut o)?; sky_adds.push((i, f32_at(&mut o)?)); }
        Ok(DirContrib { sweep, di, n_subs, sel, occl_bits, probe_cur, sky_adds })
    }

    pub fn write(&self, dir: &Path) -> std::io::Result<()> {
        std::fs::create_dir_all(dir)?;
        let tmp = dir.join(format!("{}.tmp", file_name(self.sweep, self.di)));
        let mut f = std::fs::File::create(&tmp)?;
        f.write_all(&self.encode())?;
        f.sync_all()?;
        std::fs::rename(&tmp, dir.join(file_name(self.sweep, self.di)))
    }

    /// The direction's file from the first of `dirs` that has it.
    pub fn load(dirs: &[PathBuf], sweep: u32, di: u32) -> Result<DirContrib, String> {
        let name = file_name(sweep, di);
        for d in dirs {
            let p = d.join(&name);
            if p.exists() {
                let mut b = Vec::new();
                std::fs::File::open(&p).and_then(|mut f| f.read_to_end(&mut b)).map_err(|e| format!("{}: {e}", p.display()))?;
                return DirContrib::decode(&b).map_err(|e| format!("{}: {e}", p.display()));
            }
        }
        Err(format!("no contribution for sweep {sweep} direction {di} ({name}) in {:?}", dirs))
    }

    pub fn occl(&self, i: usize) -> bool {
        self.occl_bits.get(i >> 6).map(|w| (w >> (i & 63)) & 1 == 1).unwrap_or(false)
    }
}

/// Which directions of `n` a box with index `k` of `boxes` takes: contiguous ranges of nearly equal length.
pub fn range_of(n: usize, boxes: usize, k: usize) -> (usize, usize) {
    let boxes = boxes.max(1);
    let per = (n + boxes - 1) / boxes;
    ((k * per).min(n), ((k + 1) * per).min(n))
}

/// The direction range from `a..b` (end exclusive) or `k/N` (box k of N over `n` directions).
pub fn parse_range(s: &str, n: usize) -> Result<(usize, usize), String> {
    if let Some((a, b)) = s.split_once("..") {
        let a: usize = a.trim().parse().map_err(|_| format!("--dir-range {s}: not a..b"))?;
        let b: usize = if b.trim().is_empty() { n } else { b.trim().parse().map_err(|_| format!("--dir-range {s}: not a..b"))? };
        return Ok((a.min(n), b.min(n)));
    }
    if let Some((k, m)) = s.split_once('/') {
        let k: usize = k.trim().parse().map_err(|_| format!("--dir-range {s}: not k/N"))?;
        let m: usize = m.trim().parse().map_err(|_| format!("--dir-range {s}: not k/N"))?;
        if k >= m { return Err(format!("--dir-range {s}: k must be below N")); }
        return Ok(range_of(n, m, k));
    }
    Err(format!("--dir-range {s}: a..b or k/N"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_file_round_trips_packed_and_raw() {
        let mut c = DirContrib { sweep: 1, di: 7, n_subs: 130, sel: vec![[0.5, 1.0, 0.25], [2.0, 0.0, 65024.0]], occl_bits: vec![0xdead_beef_0000_0001, 3], probe_cur: vec![(5, [0.1, 0.2, 0.3, 1.0])], sky_adds: vec![(9, 0.0018293475)] };
        let d = DirContrib::decode(&c.encode()).unwrap();
        assert_eq!(d.sel, c.sel);
        assert_eq!(d.occl_bits, c.occl_bits);
        assert_eq!(d.probe_cur, c.probe_cur);
        assert_eq!(d.sky_adds, c.sky_adds);
        assert!(d.occl(0) && !d.occl(1) && d.occl(64) && d.occl(65) && !d.occl(66));
        // a value R11G11B10 cannot hold exactly → the raw form
        c.sel.push([0.1, 0.2, 0.3]);
        let e = c.encode();
        assert_eq!(u32::from_le_bytes(e[20..24].try_into().unwrap()), 1, "raw mode");
        assert_eq!(DirContrib::decode(&e).unwrap().sel, c.sel);
    }

    #[test]
    fn ranges_cover_the_directions_once() {
        for n in [1usize, 9, 256, 1024, 1000] {
            for boxes in [1usize, 2, 3, 4, 7] {
                let mut seen = vec![0u8; n];
                for k in 0..boxes { let (a, b) = range_of(n, boxes, k); for i in a..b { seen[i] += 1; } }
                assert!(seen.iter().all(|s| *s == 1), "n {n} boxes {boxes}: {seen:?}");
            }
        }
        assert_eq!(parse_range("3..10", 256).unwrap(), (3, 10));
        assert_eq!(parse_range("250..", 256).unwrap(), (250, 256));
        assert_eq!(parse_range("1/4", 1000).unwrap(), (250, 500));
        assert!(parse_range("4/4", 10).is_err());
    }
}
