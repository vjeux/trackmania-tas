//! `CPlugMaterial` (0x09079000) and `CPlugMaterialCustom` (0x0903A000):
//! the pre-UserInst material classes that BlueBay terrain prefabs embed
//! inline. Reference tiny items and everything this tool WRITES use
//! `CPlugMaterialUserInst`; these two are READ-ONLY prefab sources (their
//! `write` panics: prefabs are never rewritten).
//!
//! Chunk layouts are ported from `classes.rs` (which follows GBX.NET's
//! `CPlugMaterial.chunkl` and was checked against `Zone\Land\Base`).
//! What the builder needs per material: the surface physics id
//! (chunk 0x0907900E) and every referenced node index (shaders, the custom
//! node, and — chunk 0x0907900D's trailing array — the external
//! `.Material.Gbx` the terrain material stands for).

use super::{Rd, R, FACADE};

/// After a chunk id, a `PIKS` + size + payload wrapping means the chunk's
/// struct lives in the payload (prefabs wrap e.g. 0x0903A00F). Returns the
/// payload end for the caller to snap to, or `None` for the plain form.
fn wrapped(r: &mut Rd) -> R<Option<usize>> {
    if r.b.get(r.o..r.o + 4) == Some(b"PIKS") {
        r.o += 4;
        let n = r.count()?;
        Ok(Some(r.o + n))
    } else {
        Ok(None)
    }
}

/// A GBX lookback string on this reader (same encoding as `Id`).
fn lookback(r: &mut Rd) -> R<String> {
    if !r.lb.version_seen {
        let v = r.u32()?;
        if v != 3 {
            return Err(format!("lookback version {} (expected 3) at 0x{:x}", v, r.o - 4));
        }
        r.lb.version_seen = true;
    }
    let raw = r.u32()?;
    let flags = raw >> 30;
    let idx = raw & 0x3FFF_FFFF;
    if idx == 0x3FFF_FFFF {
        return Ok(if flags == 2 { "Unassigned".into() } else { String::new() });
    }
    if flags == 0 || flags == 3 {
        return Ok(format!("collection#{idx}"));
    }
    if idx == 0 {
        let s = r.string()?;
        r.lb.table.push(s.clone());
        return Ok(s);
    }
    match r.lb.table.get(idx as usize - 1) {
        Some(s) => Ok(s.clone()),
        None => Err(format!("lookback index {} out of range at 0x{:x}", idx, r.o - 4)),
    }
}

/// A node reference as a bare index; inline first appearances are parsed via
/// `super::read_node` (registers `defined_nodes`, so later refs skip) and
/// their indices collected alongside every other ref.
fn ri(r: &mut Rd, refs: &mut Vec<i32>) -> R<i32> {
    let idx = r.i32()?;
    refs.push(idx);
    if idx >= 0 && !r.lb.defined_nodes.contains(&(idx as u32)) {
        r.lb.defined_nodes.insert(idx as u32);
        let class_id = r.u32()?;
        let _ = super::read_node(r, class_id)
            .map_err(|e| format!("node {} (class 0x{:08X}): {}", idx, class_id, e))?;
    }
    Ok(idx)
}

/// `CPlugMaterial::DeviceMat[]` at the given chunk version.
fn device_materials(r: &mut Rd, refs: &mut Vec<i32>, version: u32) -> R<()> {
    let n = r.count()?;
    for _ in 0..n {
        r.take(4)?; // two shorts
        if version >= 4 {
            r.bool32()?;
        }
        ri(r, refs)?; // Shader1
        if version >= 9 {
            ri(r, refs)?; // Shader2
            ri(r, refs)?; // Shader3
        }
    }
    Ok(())
}

fn bitmaps(r: &mut Rd, refs: &mut Vec<i32>, version: u32) -> R<()> {
    let n = r.count()?;
    for _ in 0..n {
        lookback(r)?;
        r.i32()?;
        ri(r, refs)?; // texture
        if version >= 1 {
            r.take(8)?;
        }
    }
    Ok(())
}

/// A parsed old-style material: physics + every referenced node index.
#[derive(Clone, Debug, PartialEq)]
pub struct OldMaterial {
    pub physics: u8,
    pub refs: Vec<i32>,
}

impl OldMaterial {
    pub fn parse(r: &mut Rd) -> R<OldMaterial> {
        let mut m = OldMaterial { physics: 0, refs: Vec::new() };
        loop {
            let at = r.o;
            let cid = r.u32()?;
            if cid == FACADE {
                break;
            }
            let end = wrapped(r)?;
            match cid {
                0x09079001 | 0x09079007 => {
                    ri(r, &mut m.refs)?;
                }
                0x09079002 | 0x0907900A | 0x0907900F => {
                    r.u32()?;
                }
                0x09079004 => {
                    device_materials(r, &mut m.refs, 4)?;
                }
                0x09079009 => {
                    let shader = ri(r, &mut m.refs)?;
                    if shader == -1 {
                        device_materials(r, &mut m.refs, 9)?;
                    }
                }
                0x0907900D => {
                    let shader = ri(r, &mut m.refs)?;
                    if shader == -1 {
                        device_materials(r, &mut m.refs, 0xD)?;
                        for i in r.array(|r| r.i32())? {
                            m.refs.push(i);
                        }
                    }
                }
                0x0907900E => {
                    // SurfaceId (the physics the car feels), U01: two shorts.
                    m.physics = r.u16()? as u8;
                    r.u16()?;
                }
                0x09079010 => {
                    r.f32()?;
                }
                0x09079011 => {
                    r.array(|r| lookback(r))?;
                }
                0x09079015 => {
                    let v = r.u32()?;
                    let shader = ri(r, &mut m.refs)?;
                    if shader == -1 {
                        device_materials(r, &mut m.refs, 0x15)?;
                        r.array(|r| r.i32())?;
                        if v >= 3 {
                            r.i32()?;
                        }
                    } else {
                        let n = r.u32()? as usize;
                        for _ in 0..n {
                            ri(r, &mut m.refs)?; // CPlugMaterialColorTargetTable
                        }
                        if v >= 7 {
                            ri(r, &mut m.refs)?;
                        }
                    }
                }
                0x09079016 => {
                    r.take(8)?; // version, uint
                }
                0x09079017 => {
                    let v = r.u32()?;
                    r.u32()?;
                    if v >= 1 {
                        r.take(8)?;
                        r.string()?;
                    }
                }
                // Unknown but PIKS-wrapped: stepped over like the walk
                // does (e.g. custom 0x0903A011). Unknown and NOT wrapped is
                // fatal: the layout is genuinely unknown.
                c => {
                    if end.is_none() {
                        return Err(format!("CPlugMaterial chunk 0x{c:08X} at 0x{at:x} has no reader"));
                    }
                }
            }
            if let Some(end) = end {
                r.o = end;
            }
        }
        Ok(m)
    }

    pub fn write(&self, _w: &mut super::Wr) {
        panic!("OldMaterial is a read-only prefab source; prefabs are never rewritten");
    }
}

/// A parsed `CPlugMaterialCustom`: only its refs are kept (building needs
/// nothing else from it).
#[derive(Clone, Debug, PartialEq)]
pub struct OldCustom {
    pub refs: Vec<i32>,
}

impl OldCustom {
    pub fn parse(r: &mut Rd) -> R<OldCustom> {
        let mut refs = Vec::new();
        loop {
            let at = r.o;
            let cid = r.u32()?;
            if cid == FACADE {
                break;
            }
            let end = wrapped(r)?;
            match cid {
                0x0903A004 => {
                    r.array(|r| r.i32())?;
                }
                0x0903A006 => bitmaps(r, &mut refs, 0)?,
                0x0903A00A => {
                    for _ in 0..2 {
                        let n = r.u32()? as usize;
                        for _ in 0..n {
                            lookback(r)?;
                            let c1 = r.u32()? as usize;
                            let c2 = r.u32()? as usize;
                            r.bool32()?;
                            r.take(4 * c1 * c2)?;
                        }
                    }
                }
                0x0903A00B => {
                    let u01 = r.u32()?;
                    r.take(8)?;
                    if u01 & 1 != 0 {
                        r.take(4)?;
                    }
                }
                0x0903A00C => {
                    r.array(|r| {
                        lookback(r)?;
                        r.bool32()
                    })?;
                }
                0x0903A00D | 0x0903A016 => {
                    let v = if cid == 0x0903A016 { r.u32()? } else { 0 };
                    let u01 = r.u32()?;
                    r.take(4 + 8)?;
                    if cid == 0x0903A016 && v >= 1 {
                        r.i32()?;
                    }
                    if u01 & 1 != 0 {
                        r.take(4)?;
                    }
                }
                // Skippable in the walk, but DEFINES lookback ids the bitmap
                // list refers to: parse for the table side effects.
                0x0903A00F => {
                    let v = r.u32()?;
                    r.take(8)?;
                    if v >= 1 {
                        r.take(4)?;
                    }
                    if v >= 2 {
                        r.array(|r| {
                            lookback(r)?;
                            r.i32()
                        })?;
                    }
                }
                0x0903A010 | 0x0903A012 => {
                    ri(r, &mut refs)?;
                }
                0x0903A013 => {
                    let _v = r.u32()?;
                    bitmaps(r, &mut refs, 1)?;
                }
                0x0903A014 => {
                    let _v = r.u32()?;
                    let n = r.u32()? as usize;
                    for _ in 0..n {
                        r.i32()?;
                        let m = r.u32()? as usize;
                        r.take(m)?;
                    }
                }
                0x0903A015 => {
                    let v = r.u32()?;
                    let u01 = if v >= 1 { r.i32()? } else { 0 };
                    if u01 == 0 {
                        r.string()?;
                        r.string()?;
                        if v >= 2 {
                            r.string()?;
                            r.string()?;
                        }
                    }
                }
                c => {
                    if end.is_none() {
                        return Err(format!("CPlugMaterialCustom chunk 0x{c:08X} at 0x{at:x} has no reader"));
                    }
                }
            }
            if let Some(end) = end {
                r.o = end;
            }
        }
        Ok(OldCustom { refs })
    }

    pub fn write(&self, _w: &mut super::Wr) {
        panic!("OldCustom is a read-only prefab source; prefabs are never rewritten");
    }
}
