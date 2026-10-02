//! The SOURCE map's own editor lightmap as an oracle for "does the game draw
//! this record" — offline, no game needed (2026-10-01, Fall 2026 - 12).
//!
//! The editor's lightmapper renders exactly the scene the client draws, and
//! its chart table (`0x0304305B` → zlib `CHmsLightMapCache` → PIKS chunk
//! `0x0602201A`, table z1) binds every chart to an OBJECT index in
//! `AutoSetIdsForLightMap` order (`docs/formats/map-lightmap.md`; measured
//! with `lmtool itembase`): `P` (16384 on a Stadium decoration — the deco map
//! takes the ids below it; 0 on the terrain collections) + the authored
//! blocks in file order + the baked records in file order + the items. A
//! baked record WITH a chart was rendered by the lightmapper = drawn in play;
//! one WITHOUT a chart drew nothing (an empty picked variant, an empty
//! prefab, a mesh without TexCoord1). Fall 2026 - 12: every one of the 261
//! ghost-mode `DecoWallBaseVFC` panels the converter left out is charted
//! (the dark TRACKMANIA walls under the floating wall vjeux photographed);
//! the 191 variant-4 "nothing" records are not; `StadiumStructurePillarToFlatACB`
//! (empty prefab) 0 of 29.
//!
//! The object space must fit the file exactly (`max object + 1 == P + authored
//! + baked + items`), else the mapping is not this file's (a transplanted or
//! stale chunk) and there is no oracle.

use std::collections::HashSet;
use std::path::Path;

use tmmaps::map::MapFile;

pub const LIGHTMAP_CHUNK: u32 = 0x0304_305B;
const PIKS: u32 = 0x534B_4950;
const FACADE: u32 = 0xFACA_DE01;
const MAPPING_CHUNK: u32 = 0x0602_201A;

pub struct Oracle {
    /// The first authored block's object index (16384 on Stadium, 0 elsewhere).
    pub base: u32,
    pub n_authored: u32,
    pub n_baked: u32,
    pub n_items: u32,
    /// Object indices that own at least one chart.
    pub charted: HashSet<u32>,
    pub charts: usize,
}

impl Oracle {
    pub fn authored_charted(&self, index: usize) -> bool {
        self.charted.contains(&(self.base + index as u32))
    }
    pub fn baked_charted(&self, index: usize) -> bool {
        self.charted.contains(&(self.base + self.n_authored + index as u32))
    }
    pub fn item_charted(&self, index: usize) -> bool {
        self.charted.contains(&(self.base + self.n_authored + self.n_baked + index as u32))
    }
}

struct Cur<'a> {
    b: &'a [u8],
    o: usize,
}

impl<'a> Cur<'a> {
    fn u32(&mut self) -> Result<u32, String> {
        let s = self.b.get(self.o..self.o + 4).ok_or_else(|| format!("eof at {:#x}", self.o))?;
        self.o += 4;
        Ok(u32::from_le_bytes(s.try_into().unwrap()))
    }
    fn take(&mut self, n: usize) -> Result<&'a [u8], String> {
        let s = self.b.get(self.o..self.o + n).ok_or_else(|| format!("eof at {:#x} ({n} bytes)", self.o))?;
        self.o += n;
        Ok(s)
    }
}

fn inflate(z: &[u8], expect: usize) -> Result<Vec<u8>, String> {
    let out = miniz_oxide::inflate::decompress_to_vec_zlib(z).map_err(|e| format!("zlib: {e:?}"))?;
    if expect != 0 && out.len() != expect {
        return Err(format!("zlib: {} bytes inflated, {expect} expected", out.len()));
    }
    Ok(out)
}

/// The chart → object binds of a lightmap chunk payload: `None` when the chunk
/// says `has_lightmaps = 0`.
fn binds_of_chunk(p: &[u8]) -> Result<Option<Vec<u32>>, String> {
    let mut c = Cur { b: p, o: 0 };
    let _version = c.u32()?;
    if c.u32()? == 0 {
        return Ok(None);
    }
    let _u01 = c.u32()?;
    let _u02 = c.u32()?;
    let lm_version = c.u32()?;
    if lm_version < 10 {
        return Err(format!("lightmap version {lm_version}: only 10 is decoded"));
    }
    let frames = c.u32()? as usize;
    for _ in 0..frames * 3 {
        let n = c.u32()? as usize;
        c.take(n)?;
    }
    let raw_len = c.u32()? as usize;
    let zlen = c.u32()? as usize;
    let cache = inflate(c.take(zlen)?, raw_len)?;
    // the cache node: PIKS chunks to FACADE01
    let mut k = Cur { b: &cache, o: 0 };
    loop {
        let id = k.u32()?;
        if id == FACADE {
            return Err("no mapping chunk in the cache".into());
        }
        if k.u32()? != PIKS {
            return Err(format!("cache chunk {id:#010x} is not skippable"));
        }
        let size = k.u32()? as usize;
        let payload = k.take(size)?;
        if id == MAPPING_CHUNK {
            return mapping_binds(payload).map(Some);
        }
    }
}

/// The z1 table of the mapping chunk (`lightmap::format::Mapping::parse`
/// decodes the whole chunk; only the binds matter here).
fn mapping_binds(p: &[u8]) -> Result<Vec<u32>, String> {
    let mut c = Cur { b: p, o: 0 };
    let version = c.u32()?;
    if version != 13 {
        return Err(format!("mapping chunk version {version}: only 13 is decoded"));
    }
    // 60 bytes of constants, one 66-byte frame record per frame (3; 2 without local
    // lights), `u32 2, 0, 0`, then the struct version word 9
    let base = c.o;
    let head_len = [3usize, 2, 1, 4]
        .into_iter()
        .map(|k| 60 + 66 * k + 12)
        .find(|&len| {
            let o = base + len;
            let at = |o: usize| p.get(o..o + 4).map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]));
            at(o) == Some(9) && at(o - 12) == Some(2)
        })
        .ok_or("mapping head: no struct version 9 after the frame records")?;
    c.take(head_len)?;
    let map_version = c.u32()?;
    if map_version != 9 {
        return Err(format!("mapping struct version {map_version}"));
    }
    let _u01 = c.u32()?;
    let _atlas_w = c.u32()?;
    let _atlas_h = c.u32()?;
    for _ in 0..6 {
        c.u32()?; // bbox
    }
    let _u02 = c.u32()?;
    let count = c.u32()? as usize;
    // z0: f32 per chart (skipped), z1: (obj_idx, obj_group_idx) per chart
    let mut ztable = |c: &mut Cur| -> Result<Vec<u8>, String> {
        let u = c.u32()? as usize;
        let z = c.u32()? as usize;
        inflate(c.take(z)?, u)
    };
    let _z0 = ztable(&mut c)?;
    let z1 = ztable(&mut c)?;
    if z1.len() != count * 8 {
        return Err(format!("z1: {} bytes for {count} charts", z1.len()));
    }
    Ok(z1.chunks(8).map(|b| u32::from_le_bytes(b[4..8].try_into().unwrap()) / 4).collect())
}

/// The oracle of a map file, or `Ok(None)` when the file carries no usable
/// lightmap (no chunk, `has_lightmaps = 0`, or a mapping whose object space
/// is not this file's).
pub fn read(path: &Path, m: &MapFile, collection: u32) -> Result<Option<Oracle>, String> {
    let data = std::fs::read(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let g = gbx::Gbx::parse(&data);
    let Some((_cid, _off, payload, size)) = gbx::all_skip_chunks(&g.body).into_iter().find(|c| c.0 == LIGHTMAP_CHUNK) else {
        return Ok(None);
    };
    let Some(objects) = binds_of_chunk(&g.body[payload..payload + size])? else {
        return Ok(None);
    };
    let base: u32 = if collection == 0x1a { 16384 } else { 0 };
    let (na, nb, ni) = (m.blocks.len() as u32, m.baked.len() as u32, m.items.len() as u32);
    let max = objects.iter().copied().max().unwrap_or(0);
    let space = base + na + nb + ni;
    // THE FIT RULE (relaxed 2026-10-02, Ludde A08 #11): the mapping is this file's bake when its
    // object indices fit the file's object space and reach at least its last baked record — the
    // trailing objects may be UNCHARTED (the last 73 items of A08 #11 are ObstacleTurnstile8m
    // dynamic items the lightmapper never charts, so max + 1 fell 73 short of the space); a bake
    // whose indices run PAST the space, or stop inside the authored blocks, is not this file's.
    if max + 1 > space || max + 1 < base + na + nb {
        return Err(format!("lightmap mapping: object space {} (max object {max}) does not fit {base} + {na} authored + {nb} baked + {ni} items = {space}: not this file's bake, no oracle", max + 1));
    }
    if max + 1 != space {
        eprintln!("  lightmap oracle: the last {} object(s) of the file (items) are uncharted — the mapping's max object {max} + 1 = {}, the object space {space}", space - (max + 1), max + 1);
    }
    let charts = objects.len();
    Ok(Some(Oracle { base, n_authored: na, n_baked: nb, n_items: ni, charted: objects.into_iter().collect(), charts }))
}
