//! `CGameItemPlacementParam` (0x2E020000), typed: what the editor does with
//! an item — the grid it snaps to, the fly step, the pivots, the magnet
//! points and the placement class (`NPlugItemPlacement_SClass`, 0x09187000).
//! Layouts per GBX.NET (`CGameItemPlacementParam.chunkl`,
//! `NPlugItemPlacement_SClass.chunkl`) and the pack's own `.PlaceParam.Gbx`
//! files (`Stadium\Media\PlaceParam\GateRacing32m`: grid 2 m / 2 m, fly 1 m,
//! two magnet points at the gate's ends, size group `1x1`).
//!
//! Every chunk is skippable. The static-item writer keeps the node as raw
//! chunks (`item::CGameItemPlacementParam`); this module converts both ways.

use super::item::CGameItemPlacementParam;
use super::RawChunk;

pub const FLAG_YAW_ONLY: u16 = 1 << 1;
pub const FLAG_NOT_ON_OBJECT: u16 = 1 << 2;
pub const FLAG_AUTO_ROTATION: u16 = 1 << 3;
pub const FLAG_SWITCH_PIVOT_MANUALLY: u16 = 1 << 4;

/// One patch layout of the placement class (the editor's item-along-a-curve
/// tool); kept as the raw record it is read as.
#[derive(Clone, Debug, PartialEq)]
pub struct PatchLayout {
    pub item_count: i32,
    pub item_spacing: f32,
    pub fill_align: i32,
    pub fill_dir: i32,
    pub normed_pos: f32,
    pub dist_from_normed_pos: f32,
    pub only_on_groups: Vec<Option<String>>,
    pub altitude: f32,
    pub fill_border_offset: f32,
}

/// `NPlugItemPlacement_SClass` (version 10).
#[derive(Clone, Debug, PartialEq)]
pub struct SClass {
    pub version: u32,
    pub size_group: Option<String>,
    pub compatible_groups: Vec<Option<String>>,
    pub always_up: bool,
    pub align_to_interior: bool,
    pub align_to_world_dir: bool,
    pub world_dir: [f32; 3],
    pub patch_layouts: Vec<PatchLayout>,
    pub group_cur_patch_layouts: Vec<i32>,
}

impl Default for SClass {
    /// What the item editor writes for a new item (the 26 reference items).
    fn default() -> SClass {
        SClass { version: 10, size_group: None, compatible_groups: Vec::new(), always_up: false, align_to_interior: true, align_to_world_dir: false, world_dir: [0.0, 0.0, 1.0], patch_layouts: Vec::new(), group_cur_patch_layouts: Vec::new() }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct PlacementParam {
    pub version: u32,
    pub flags: u16,
    pub cube_center: [f32; 3],
    pub cube_size: f32,
    pub grid_h_step: f32,
    pub grid_v_step: f32,
    pub grid_h_offset: f32,
    pub grid_v_offset: f32,
    pub fly_v_step: f32,
    pub fly_v_offset: f32,
    pub pivot_snap_distance: f32,
    pub pivot_positions: Vec<[f32; 3]>,
    pub pivot_rotations: Vec<[f32; 4]>,
    /// Chunk 0x2E020004: (position, rotation in degrees) per magnet point.
    pub magnet_locs: Vec<([f32; 3], [f32; 3])>,
    pub magnet_version: u32,
    /// Chunk 0x2E020005: the node index the inline class is written under,
    /// and the class.
    pub sclass: Option<(i32, SClass)>,
    /// Chunks this module does not model (0x2E020003, …), in file order
    /// relative to the modelled ones: (position in the chunk list, chunk).
    pub extra: Vec<(usize, RawChunk)>,
}

impl Default for PlacementParam {
    /// The item editor's defaults: grid 1 m / 0, fly 1 m, pivot snap off.
    fn default() -> PlacementParam {
        PlacementParam {
            version: 0,
            flags: 1,
            cube_center: [0.0; 3],
            cube_size: 0.0,
            grid_h_step: 1.0,
            grid_v_step: 0.0,
            grid_h_offset: 0.0,
            grid_v_offset: 0.0,
            fly_v_step: 1.0,
            fly_v_offset: 0.0,
            pivot_snap_distance: -1.0,
            pivot_positions: Vec::new(),
            pivot_rotations: Vec::new(),
            magnet_locs: Vec::new(),
            magnet_version: 0,
            sclass: None,
            extra: Vec::new(),
        }
    }
}

struct Cur<'a> {
    b: &'a [u8],
    o: usize,
}

impl<'a> Cur<'a> {
    fn take(&mut self, n: usize) -> Result<&'a [u8], String> {
        let s = self.b.get(self.o..self.o + n).ok_or_else(|| format!("placement chunk truncated at 0x{:x}", self.o))?;
        self.o += n;
        Ok(s)
    }
    fn u32(&mut self) -> Result<u32, String> {
        Ok(u32::from_le_bytes(self.take(4)?.try_into().unwrap()))
    }
    fn i32(&mut self) -> Result<i32, String> {
        Ok(self.u32()? as i32)
    }
    /// An element count: bounded by the bytes left (garbage tails read as
    /// counts of billions and `with_capacity` aborts the process).
    fn count(&mut self) -> Result<usize, String> {
        let n = self.u32()? as usize;
        if n > self.b.len().saturating_sub(self.o) {
            return Err(format!("absurd count {n} at 0x{:x}", self.o - 4));
        }
        Ok(n)
    }
    fn u16(&mut self) -> Result<u16, String> {
        Ok(u16::from_le_bytes(self.take(2)?.try_into().unwrap()))
    }
    fn f32(&mut self) -> Result<f32, String> {
        Ok(f32::from_bits(self.u32()?))
    }
    fn vec3(&mut self) -> Result<[f32; 3], String> {
        Ok([self.f32()?, self.f32()?, self.f32()?])
    }
    fn string(&mut self) -> Result<String, String> {
        let n = self.u32()? as usize;
        Ok(String::from_utf8_lossy(self.take(n)?).into_owned())
    }
    /// A lookback Id: null, a new string, or a reference into `table`.
    fn id(&mut self, table: &mut Vec<String>) -> Result<Option<String>, String> {
        let w = self.u32()?;
        if w == 0xFFFF_FFFF {
            return Ok(None);
        }
        if w & 0xC000_0000 == 0 {
            return Ok(Some(w.to_string()));
        }
        let idx = w & 0x3FFF_FFFF;
        if idx == 0 {
            let s = self.string()?;
            table.push(s.clone());
            return Ok(Some(s));
        }
        table.get(idx as usize - 1).cloned().map(Some).ok_or_else(|| format!("lookback index {idx} past the table"))
    }
}

fn put_u32(v: &mut Vec<u8>, x: u32) {
    v.extend_from_slice(&x.to_le_bytes());
}
fn put_f32(v: &mut Vec<u8>, x: f32) {
    v.extend_from_slice(&x.to_le_bytes());
}
fn put_vec3(v: &mut Vec<u8>, x: &[f32; 3]) {
    for f in x {
        put_f32(v, *f);
    }
}

impl SClass {
    /// `first_id_in_body`: a standalone `.PlaceParam.Gbx` puts the lookback
    /// version word before its first Id; inside an item body the ident chunk
    /// wrote it long before.
    pub fn parse(c: &mut Cur, table: &mut Vec<String>, first_id_in_body: bool) -> Result<SClass, String> {
        let version = c.u32()?;
        if first_id_in_body {
            let lbver = c.u32()?;
            if lbver != 3 {
                return Err(format!("placement class: lookback version {lbver}"));
            }
        }
        let size_group = c.id(table)?;
        let n = c.count()?;
        let mut compatible_groups = Vec::with_capacity(n);
        for _ in 0..n {
            compatible_groups.push(c.id(table)?);
        }
        let always_up = c.u32()? != 0;
        let align_to_interior = c.u32()? != 0;
        let align_to_world_dir = c.u32()? != 0;
        let world_dir = c.vec3()?;
        let n = c.count()?;
        let mut patch_layouts = Vec::with_capacity(n);
        for _ in 0..n {
            let item_count = c.i32()?;
            let item_spacing = c.f32()?;
            let fill_align = c.i32()?;
            let fill_dir = c.i32()?;
            let normed_pos = c.f32()?;
            let dist_from_normed_pos = c.f32()?;
            let k = c.count()?;
            let mut only_on_groups = Vec::with_capacity(k);
            for _ in 0..k {
                only_on_groups.push(c.id(table)?);
            }
            let altitude = c.f32()?;
            let fill_border_offset = c.f32()?;
            patch_layouts.push(PatchLayout { item_count, item_spacing, fill_align, fill_dir, normed_pos, dist_from_normed_pos, only_on_groups, altitude, fill_border_offset });
        }
        let n = c.count()?;
        let mut group_cur_patch_layouts = Vec::with_capacity(n);
        for _ in 0..n {
            group_cur_patch_layouts.push(c.i32()?);
        }
        Ok(SClass { version, size_group, compatible_groups, always_up, align_to_interior, align_to_world_dir, world_dir, patch_layouts, group_cur_patch_layouts })
    }

    /// Written for an item body: Ids are null only (a string would need the
    /// body's shared lookback table).
    pub fn write(&self, v: &mut Vec<u8>) -> Result<(), String> {
        put_u32(v, self.version);
        if self.size_group.is_some() || self.compatible_groups.iter().any(|g| g.is_some()) || self.patch_layouts.iter().any(|p| p.only_on_groups.iter().any(|g| g.is_some())) {
            return Err("placement class with a named size group: not writable as a raw chunk (needs the body's lookback table)".into());
        }
        put_u32(v, 0xFFFF_FFFF);
        put_u32(v, self.compatible_groups.len() as u32);
        for _ in &self.compatible_groups {
            put_u32(v, 0xFFFF_FFFF);
        }
        put_u32(v, self.always_up as u32);
        put_u32(v, self.align_to_interior as u32);
        put_u32(v, self.align_to_world_dir as u32);
        put_vec3(v, &self.world_dir);
        put_u32(v, self.patch_layouts.len() as u32);
        for p in &self.patch_layouts {
            put_u32(v, p.item_count as u32);
            put_f32(v, p.item_spacing);
            put_u32(v, p.fill_align as u32);
            put_u32(v, p.fill_dir as u32);
            put_f32(v, p.normed_pos);
            put_f32(v, p.dist_from_normed_pos);
            put_u32(v, p.only_on_groups.len() as u32);
            for _ in &p.only_on_groups {
                put_u32(v, 0xFFFF_FFFF);
            }
            put_f32(v, p.altitude);
            put_f32(v, p.fill_border_offset);
        }
        put_u32(v, self.group_cur_patch_layouts.len() as u32);
        for g in &self.group_cur_patch_layouts {
            put_u32(v, *g as u32);
        }
        Ok(())
    }
}

impl PlacementParam {
    /// From the raw chunks of an item's placement node (or of a standalone
    /// `.PlaceParam.Gbx` body: `standalone` = its first Id carries the
    /// lookback version word).
    pub fn from_chunks(chunks: &[RawChunk], standalone: bool) -> Result<PlacementParam, String> {
        let mut p = PlacementParam { extra: Vec::new(), ..PlacementParam::default() };
        let mut table: Vec<String> = Vec::new();
        for (k, ch) in chunks.iter().enumerate() {
            let mut c = Cur { b: &ch.payload, o: 0 };
            match ch.id {
                0x2E020000 => {
                    p.version = c.u32()?;
                    p.flags = c.u16()?;
                    p.cube_center = c.vec3()?;
                    p.cube_size = c.f32()?;
                    p.grid_h_step = c.f32()?;
                    p.grid_v_step = c.f32()?;
                    p.grid_h_offset = c.f32()?;
                    p.grid_v_offset = c.f32()?;
                    p.fly_v_step = c.f32()?;
                    p.fly_v_offset = c.f32()?;
                    p.pivot_snap_distance = c.f32()?;
                }
                0x2E020001 => {
                    let n = c.count()?;
                    p.pivot_positions.clear();
                    for _ in 0..n {
                        p.pivot_positions.push(c.vec3()?);
                    }
                    let n = c.count()?;
                    p.pivot_rotations.clear();
                    for _ in 0..n {
                        p.pivot_rotations.push([c.f32()?, c.f32()?, c.f32()?, c.f32()?]);
                    }
                }
                0x2E020004 => {
                    p.magnet_version = c.u32()?;
                    let n = c.count()?;
                    p.magnet_locs.clear();
                    for _ in 0..n {
                        p.magnet_locs.push((c.vec3()?, c.vec3()?));
                    }
                }
                0x2E020005 => {
                    let index = c.i32()?;
                    if index == -1 {
                        p.sclass = None;
                        continue;
                    }
                    let cid = c.u32()?;
                    if cid != 0x09187000 {
                        return Err(format!("placement class node is class 0x{cid:08X}, not NPlugItemPlacement_SClass"));
                    }
                    p.sclass = Some((index, SClass::parse(&mut c, &mut table, standalone)?));
                }
                _ => p.extra.push((k, ch.clone())),
            }
            if c.o != ch.payload.len() && matches!(ch.id, 0x2E020000 | 0x2E020001 | 0x2E020004 | 0x2E020005) {
                return Err(format!("placement chunk 0x{:08X}: {} of {} bytes read", ch.id, c.o, ch.payload.len()));
            }
        }
        Ok(p)
    }

    pub fn from_node(n: &CGameItemPlacementParam) -> Result<PlacementParam, String> {
        PlacementParam::from_chunks(&n.chunks, false)
    }

    /// The raw chunks for an item body, in the reference order 000, 001,
    /// 004, 005 (extra chunks re-inserted at their positions).
    pub fn to_chunks(&self) -> Result<Vec<RawChunk>, String> {
        let mut out = Vec::new();
        let mut p0 = Vec::new();
        put_u32(&mut p0, self.version);
        p0.extend_from_slice(&self.flags.to_le_bytes());
        put_vec3(&mut p0, &self.cube_center);
        put_f32(&mut p0, self.cube_size);
        for f in [self.grid_h_step, self.grid_v_step, self.grid_h_offset, self.grid_v_offset, self.fly_v_step, self.fly_v_offset, self.pivot_snap_distance] {
            put_f32(&mut p0, f);
        }
        out.push(RawChunk { id: 0x2E020000, payload: p0 });
        let mut p1 = Vec::new();
        put_u32(&mut p1, self.pivot_positions.len() as u32);
        for p in &self.pivot_positions {
            put_vec3(&mut p1, p);
        }
        put_u32(&mut p1, self.pivot_rotations.len() as u32);
        for q in &self.pivot_rotations {
            for f in q {
                put_f32(&mut p1, *f);
            }
        }
        out.push(RawChunk { id: 0x2E020001, payload: p1 });
        let mut p4 = Vec::new();
        put_u32(&mut p4, self.magnet_version);
        put_u32(&mut p4, self.magnet_locs.len() as u32);
        for (pos, rot) in &self.magnet_locs {
            put_vec3(&mut p4, pos);
            put_vec3(&mut p4, rot);
        }
        out.push(RawChunk { id: 0x2E020004, payload: p4 });
        let mut p5 = Vec::new();
        match &self.sclass {
            Some((index, s)) => {
                put_u32(&mut p5, *index as u32);
                put_u32(&mut p5, 0x09187000);
                s.write(&mut p5)?;
            }
            None => put_u32(&mut p5, 0xFFFF_FFFF),
        }
        out.push(RawChunk { id: 0x2E020005, payload: p5 });
        for (k, ch) in &self.extra {
            let at = (*k).min(out.len());
            out.insert(at, ch.clone());
        }
        Ok(out)
    }

    pub fn to_node(&self) -> Result<CGameItemPlacementParam, String> {
        Ok(CGameItemPlacementParam { chunks: self.to_chunks()? })
    }

    /// One line for a report.
    pub fn summary(&self) -> String {
        let mut f = Vec::new();
        if self.flags & 1 != 0 {
            f.push("bit0");
        }
        if self.flags & FLAG_YAW_ONLY != 0 {
            f.push("YawOnly");
        }
        if self.flags & FLAG_NOT_ON_OBJECT != 0 {
            f.push("NotOnObject");
        }
        if self.flags & FLAG_AUTO_ROTATION != 0 {
            f.push("AutoRotation");
        }
        if self.flags & FLAG_SWITCH_PIVOT_MANUALLY != 0 {
            f.push("SwitchPivotManually");
        }
        let sc = match &self.sclass {
            Some((_, s)) => format!(
                "class{{group {:?} compat {:?} up {} interior {} worlddir {} {:?} patches {}}}",
                s.size_group,
                s.compatible_groups,
                s.always_up as u8,
                s.align_to_interior as u8,
                s.align_to_world_dir as u8,
                s.world_dir,
                s.patch_layouts.len()
            ),
            None => "class -".to_string(),
        };
        format!(
            "flags 0x{:x} [{}] grid h {} v {} off {}/{} fly {} off {} pivotsnap {} pivots {:?} rots {} magnets {:?} {}",
            self.flags,
            f.join(","),
            self.grid_h_step,
            self.grid_v_step,
            self.grid_h_offset,
            self.grid_v_offset,
            self.fly_v_step,
            self.fly_v_offset,
            self.pivot_snap_distance,
            self.pivot_positions,
            self.pivot_rotations.len(),
            self.magnet_locs,
            sc
        )
    }
}

/// Parse a standalone `.PlaceParam.Gbx` body (uncompressed; chunks up to
/// the FACADE). TOLERANT: some pack files come out of the reader with a
/// garbage tail (`GateRacing32m`, `Flag`, `Screen`: intact for ~250 bytes,
/// noise after — a pak-reader gap, 2026-09-22); the chunks read before the
/// first malformed one are kept, so at least the grid / fly / flags chunk
/// (0x2E020000, the first ~50 bytes) comes through. An error only when not
/// even that chunk parsed. `was_truncated` says whether the tail was lost.
pub fn parse_place_param_file(bytes: &[u8]) -> Result<PlacementParam, String> {
    let g = tmmaps::gbx::Gbx::parse(bytes);
    if g.class_id != 0x2E020000 {
        return Err(format!("class 0x{:08X} is not CGameItemPlacementParam", g.class_id));
    }
    let b = &g.body;
    let mut o = 0usize;
    let mut chunks: Vec<RawChunk> = Vec::new();
    let mut truncated = false;
    loop {
        let Some(idb) = b.get(o..o + 4) else {
            truncated = true;
            break;
        };
        let id = u32::from_le_bytes(idb.try_into().unwrap());
        if id == super::FACADE {
            break;
        }
        if id >> 12 != 0x2E020 || b.get(o + 4..o + 8) != Some(&b"PIKS"[..]) {
            truncated = true;
            break;
        }
        let Some(nb) = b.get(o + 8..o + 12) else {
            truncated = true;
            break;
        };
        let n = u32::from_le_bytes(nb.try_into().unwrap()) as usize;
        let Some(payload) = b.get(o + 12..o + 12 + n) else {
            truncated = true;
            break;
        };
        chunks.push(RawChunk { id, payload: payload.to_vec() });
        o += 12 + n;
    }
    // chunk by chunk: the first one that does not parse ends the file (garbage from there)
    let mut good: Vec<RawChunk> = Vec::new();
    for c in chunks {
        let mut trial = good.clone();
        trial.push(c.clone());
        if PlacementParam::from_chunks(&trial, true).is_ok() {
            good.push(c);
        } else {
            truncated = true;
            break;
        }
    }
    if !good.iter().any(|c| c.id == 0x2E020000) {
        return Err(format!("no readable placement chunk (0x2E020000) in {} body bytes", b.len()));
    }
    let mut p = PlacementParam::from_chunks(&good, true)?;
    if truncated {
        p.extra.push((usize::MAX, RawChunk { id: 0, payload: Vec::new() }));
    }
    Ok(p)
}

/// Whether `parse_place_param_file` stopped before the file's end (a garbage
/// tail): marked by a zero-id pseudo chunk in `extra`, which `to_chunks`
/// must not write — strip it with `without_marker`.
pub fn was_truncated(p: &PlacementParam) -> bool {
    p.extra.iter().any(|(_, c)| c.id == 0)
}

pub fn without_marker(p: &PlacementParam) -> PlacementParam {
    let mut q = p.clone();
    q.extra.retain(|(_, c)| c.id != 0);
    q
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_default() {
        let p = PlacementParam { grid_h_step: 16.0, grid_v_step: 4.0, fly_v_step: 4.0, pivot_positions: vec![[0.0; 3], [16.0, 0.0, 16.0]], sclass: Some((7, SClass::default())), ..PlacementParam::default() };
        let chunks = p.to_chunks().unwrap();
        let q = PlacementParam::from_chunks(&chunks, false).unwrap();
        assert_eq!(p, q);
    }

    #[test]
    fn reference_bytes() {
        // the static-item writer's reference payloads (assemble::placement_param)
        let chunks = crate::static_item::assemble::placement_param(9).chunks;
        let p = PlacementParam::from_chunks(&chunks, false).unwrap();
        assert_eq!(p.flags, 1);
        assert_eq!(p.grid_h_step, 1.0);
        assert_eq!(p.fly_v_step, 1.0);
        assert_eq!(p.pivot_snap_distance, -1.0);
        let (idx, s) = p.sclass.clone().unwrap();
        assert_eq!(idx, 9);
        assert_eq!(s, SClass::default());
        assert_eq!(p.to_chunks().unwrap(), chunks);
    }
}
