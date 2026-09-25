//! The PreLightGen of the LEGACY model classes — what the lightmapper's kind-0 (CHmsItem mobil) records
//! read (RE 7, 2026-09-25; NOTES 19:50Z):
//!
//! * a `CPlugSolid` (0x09005000) carries it in chunk `0x09005017` (reader 0x140412620): `u32 v; if v >= 3
//!   { u32 hasPLG; [PreLightGen as in CPlugSolid2Model: u32 version, u32 u01, f32 MeterByUv, u32 flag,
//!   f32 uv0[4], f32 uv1[4], i32 sprite[2], box[] (24 B), version ≥ 1: uvGroup[] (20 B)] } else { u8 u01,
//!   f32 MeterByUv, u32 flag, f32 uv0[4], f32 uv1[4], i32 sprite[2], v ≥ 1: box[] }; v ≥ 2: u64
//!   FileWriteTime`. The u01 byte lands at PLG+0x50 and the flag's bit 0 at bit 8 of the same word.
//! * a `CPlugVegetTreeModel` (0x2F086000, a class WITHOUT chunk framing) ends with `u32 hasPLG,
//!   [PreLightGen v1], u32 0, u32 1` — `WhiteShore\Media\VegetTreeModel\TreeFirSmallA1` has hasPLG 1,
//!   MeterByUv 5.46512127, uv0 [0.0206467 0.0122045 0.4564332 0.9874748]; the bushes (BushSmallA/B) have
//!   hasPLG 0 and get no lightmap record.
//!
//! Both readers locate the block by its signature (the CPlugSolid one by the chunk id in the body, the
//! tree one from the file's tail) and validate what they parse — a chunkless class has no other anchor.

use super::solid2::{read_prelight_pub, PreLightGen};
use super::{LookbackState, Rd};

fn plausible(p: &PreLightGen) -> bool {
    p.version <= 4
        && (0..=1).contains(&p.u01)
        && p.u02.is_finite()
        && p.u02 > 1e-3
        && p.u02 < 1e6
        && p.u04[..4].iter().all(|v| v.is_finite() && (-8.0..=8.0).contains(v))
        && p.sprite_count.iter().all(|&s| (0..=4096).contains(&s))
        && p.boxes.len() <= 4096
        && p.uv_groups.len() <= 4096
}

/// Parse a `0x09005017` chunk body (the bytes after the chunk id). Returns the PLG (None when the chunk
/// says there is none) and the number of bytes consumed.
pub fn parse_solid_017(body: &[u8]) -> Result<(Option<PreLightGen>, usize), String> {
    let mut r = Rd::new(body, 0, LookbackState::default());
    let v = r.u32()?;
    let plg = if v >= 3 {
        if r.bool32()? {
            Some(read_prelight_pub(&mut r)?)
        } else {
            None
        }
    } else {
        let u01 = r.u8()? as i32;
        let u02 = r.f32()?;
        let flag = r.u32()?;
        let u04 = r.floats::<8>()?;
        let sprite_count = [r.i32()?, r.i32()?];
        let boxes = if v >= 1 { r.array(|r| r.floats::<6>())? } else { Vec::new() };
        Some(PreLightGen { version: 0, u01, u02, u03: flag & 1 != 0, u04, sprite_count, boxes, uv_groups: Vec::new() })
    };
    if v >= 2 {
        r.take(8)?; // FileWriteTime
    }
    Ok((plg, r.o))
}

/// The PreLightGen of a CPlugSolid file body (the decompressed GBX body of a `.Solid.Gbx` / the inline
/// solid of a block info): finds chunk `0x09005017` by its id and parses it. `Ok(None)` when the chunk is
/// absent or says "no PLG".
pub fn solid_prelight(body: &[u8]) -> Result<Option<PreLightGen>, String> {
    let id = 0x0900_5017u32.to_le_bytes();
    let mut i = 0usize;
    while i + 4 <= body.len() {
        if body[i..i + 4] == id {
            match parse_solid_017(&body[i + 4..]) {
                Ok((Some(p), _)) if plausible(&p) => return Ok(Some(p)),
                Ok((None, _)) => return Ok(None),
                _ => {}
            }
        }
        i += 1;
    }
    Ok(None)
}

/// The PreLightGen of a `CPlugVegetTreeModel` file (the whole file's bytes, header included): the block
/// before the trailing `u32 0, u32 1`. `Ok(None)` when the tail says hasPLG = 0.
pub fn veget_tree_prelight(file: &[u8]) -> Result<Option<PreLightGen>, String> {
    let n = file.len();
    if n < 16 {
        return Err("veget tree model: file too short".into());
    }
    let tail_ok = |end: usize| file.get(end..end + 8) == Some(&[0, 0, 0, 0, 1, 0, 0, 0][..]);
    if !tail_ok(n - 8) {
        return Err(format!("veget tree model: the file does not end with (0, 1) but {:?}", &file[n - 8..]));
    }
    let end = n - 8;
    // hasPLG = 0: the tail is exactly [u32 0][0][1]
    if end >= 4 && file[end - 4..end] == [0, 0, 0, 0] {
        // could still be a PLG whose last field (uvGroups count) is 0 — try the PLG candidates first
    }
    // candidates: a PLG starts at s with hasPLG (=1) at s − 4 and ends exactly at `end`
    let max_back = 4096.min(end);
    for back in 60..=max_back {
        let s = end - back;
        if s < 4 || file[s - 4..s] != [1, 0, 0, 0] {
            continue;
        }
        let mut r = Rd::new(&file[s..end], 0, LookbackState::default());
        if let Ok(p) = read_prelight_pub(&mut r) {
            if r.o == end - s && plausible(&p) {
                return Ok(Some(p));
            }
        }
    }
    if end >= 4 && file[end - 4..end] == [0, 0, 0, 0] {
        return Ok(None);
    }
    Err("veget tree model: no PreLightGen block found before the tail".into())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The last 76 bytes of `WhiteShore\Media\VegetTreeModel\TreeFirSmallA1.VegetTreeModel.Gbx`.
    const TREE_FIR_SMALL_A1_TAIL: [u32; 20] = [
        0x40000000, 0, 0, 1, 1, 1, 0x40aee246, 1, 0x3ca92335, 0x3c47f53a, 0x3ee9b19d, 0x3f7ccb26, 0x7f7fffff, 0x7f7fffff, 0xff7fffff,
        0xff7fffff, 0, 0, 0, 0,
    ];

    #[test]
    fn tree_fir_small_a1_tail() {
        let mut bytes: Vec<u8> = TREE_FIR_SMALL_A1_TAIL.iter().flat_map(|v| v.to_le_bytes()).collect();
        bytes.extend_from_slice(&0u32.to_le_bytes());
        bytes.extend_from_slice(&1u32.to_le_bytes());
        let p = veget_tree_prelight(&bytes).unwrap().expect("has a PLG");
        assert_eq!(p.version, 1);
        assert_eq!(p.u01, 1);
        assert_eq!(p.u02.to_bits(), 0x40aee246); // 5.46512127
        assert!(p.u03);
        assert_eq!(p.u04[2].to_bits(), 0x3ee9b19d); // u1 0.4564332
        assert_eq!(p.sprite_count, [0, 0]);
        assert!(p.boxes.is_empty() && p.uv_groups.is_empty());
        // a bush: hasPLG 0
        let mut bush: Vec<u8> = vec![7, 0, 0, 0];
        bush.extend_from_slice(&0u32.to_le_bytes());
        bush.extend_from_slice(&0u32.to_le_bytes());
        bush.extend_from_slice(&1u32.to_le_bytes());
        assert!(veget_tree_prelight(&bush).unwrap().is_none());
    }

    #[test]
    fn solid_017_both_layouts() {
        // v3 with a PLG
        let mut body: Vec<u8> = Vec::new();
        for v in [0x0900_5017u32, 3, 1, 1, 1, 0x40aee246, 1, 0x3ca92335, 0x3c47f53a, 0x3ee9b19d, 0x3f7ccb26, 0x7f7fffff, 0x7f7fffff, 0xff7fffff, 0xff7fffff, 0, 0, 0, 0] {
            body.extend_from_slice(&v.to_le_bytes());
        }
        body.extend_from_slice(&[0u8; 8]); // FileWriteTime
        let p = solid_prelight(&body).unwrap().expect("plg");
        assert_eq!(p.u02.to_bits(), 0x40aee246);
        // v1 legacy: u8 u01, f32, u32 flag, 8 f32, 2 i32, boxes
        let mut body: Vec<u8> = 0x0900_5017u32.to_le_bytes().to_vec();
        body.extend_from_slice(&1u32.to_le_bytes());
        body.push(1);
        for v in [0x40aee246u32, 1, 0x3ca92335, 0x3c47f53a, 0x3ee9b19d, 0x3f7ccb26, 0x7f7fffff, 0x7f7fffff, 0xff7fffff, 0xff7fffff, 0, 0, 0] {
            body.extend_from_slice(&v.to_le_bytes());
        }
        let p = solid_prelight(&body).unwrap().expect("plg");
        assert_eq!(p.u01, 1);
        assert_eq!(p.u04[3].to_bits(), 0x3f7ccb26);
    }
}
