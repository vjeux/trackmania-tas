//! Custom items a map carries inside itself.
//!
//! A TM2020 map can embed the `.Item.Gbx` files it uses, as a ZIP in chunk
//! `0x03043054`. This matters much more than it sounds: 134672's track is made
//! of 41 custom "cap" items, so a model built only from stock blocks has the
//! map's actual driving surface missing entirely — 5 % of that run's samples
//! find anything under them.
//!
//! An embedded item is also the one place in this pipeline where a **visual**
//! mesh turns up. Stock blocks in the dedicated server's pack carry collision
//! only (`CPlugStaticObjectModel.mesh == -1`); an item authored in the editor
//! carries a `CPlugSolid2Model`, and often marks it mesh-collidable, so the
//! visual mesh IS the collision. That makes this module the positive control
//! for the visual reader: if `mapgeom items` finds triangles here, "no visual
//! meshes in the pack" is a statement about the pack and not about the reader.

use std::collections::BTreeMap;
use tmmaps::map::MapFile;

pub const EMBEDDED_CHUNK: u32 = 0x03043054;

/// Every embedded file, keyed by its path inside the zip.
pub fn files(m: &MapFile) -> Result<BTreeMap<String, Vec<u8>>, String> {
    let body = &m.gbx.body;
    let Some((_, _, payload, size)) =
        tmmaps::map::skip_chunks(body).into_iter().find(|(id, ..)| *id == EMBEDDED_CHUNK)
    else {
        return Ok(BTreeMap::new());
    };
    let chunk = &body[payload..payload + size];
    let mut r = crate::reader::Reader::new(chunk);
    let _version = r.u32()?;
    let _u01 = r.i32()?;
    let inner_len = r.u32()? as usize;
    let inner = r.take(inner_len)?;
    let mut ir = crate::reader::Reader::new(inner);
    // The inner block restarts the lookback table, which is why it is read
    // through its own Reader rather than continuing the outer one.
    let n_meta = ir.u32()? as usize;
    for _ in 0..n_meta {
        ir.meta()?;
    }
    let zip_len = ir.u32()? as usize;
    let zip = ir.take(zip_len)?;
    unzip(zip)
}

/// Only the `.Item.Gbx` entries, keyed by lowercase base name.
pub fn items(m: &MapFile) -> Result<BTreeMap<String, Vec<u8>>, String> {
    let mut out = BTreeMap::new();
    for (path, bytes) in files(m)? {
        let base = path.rsplit(['/', '\\']).next().unwrap_or(&path).to_lowercase();
        if base.ends_with(".item.gbx") {
            out.insert(base, bytes);
        }
    }
    Ok(out)
}

// --------------------------------------------------------------------- zip

/// A minimal ZIP reader: the central directory, stored and deflated entries.
///
/// Written rather than pulled in, because the whole toolchain has one
/// third-party crate and `miniz_oxide` (already here through `gbx`) supplies
/// the only hard part.
fn unzip(data: &[u8]) -> Result<BTreeMap<String, Vec<u8>>, String> {
    let mut out = BTreeMap::new();
    if data.len() < 22 {
        return Ok(out);
    }
    // End of central directory: scan back for the signature.
    let mut eocd = None;
    let start = data.len().saturating_sub(66_000);
    for i in (start..=data.len() - 22).rev() {
        if data[i..i + 4] == [0x50, 0x4b, 0x05, 0x06] {
            eocd = Some(i);
            break;
        }
    }
    let eocd = eocd.ok_or("no end-of-central-directory record")?;
    let count = u16::from_le_bytes(data[eocd + 10..eocd + 12].try_into().unwrap()) as usize;
    let mut off = u32::from_le_bytes(data[eocd + 16..eocd + 20].try_into().unwrap()) as usize;
    for _ in 0..count {
        if off + 46 > data.len() || data[off..off + 4] != [0x50, 0x4b, 0x01, 0x02] {
            return Err(format!("bad central directory entry at {}", off));
        }
        let method = u16::from_le_bytes(data[off + 10..off + 12].try_into().unwrap());
        let csize = u32::from_le_bytes(data[off + 20..off + 24].try_into().unwrap()) as usize;
        let usize_ = u32::from_le_bytes(data[off + 24..off + 28].try_into().unwrap()) as usize;
        let nlen = u16::from_le_bytes(data[off + 28..off + 30].try_into().unwrap()) as usize;
        let elen = u16::from_le_bytes(data[off + 30..off + 32].try_into().unwrap()) as usize;
        let clen = u16::from_le_bytes(data[off + 32..off + 34].try_into().unwrap()) as usize;
        let lho = u32::from_le_bytes(data[off + 42..off + 46].try_into().unwrap()) as usize;
        let name = String::from_utf8_lossy(&data[off + 46..off + 46 + nlen]).into_owned();
        off += 46 + nlen + elen + clen;

        if lho + 30 > data.len() || data[lho..lho + 4] != [0x50, 0x4b, 0x03, 0x04] {
            return Err(format!("bad local header for {}", name));
        }
        let lnlen = u16::from_le_bytes(data[lho + 26..lho + 28].try_into().unwrap()) as usize;
        let lelen = u16::from_le_bytes(data[lho + 28..lho + 30].try_into().unwrap()) as usize;
        let ds = lho + 30 + lnlen + lelen;
        if ds + csize > data.len() {
            return Err(format!("{}: data past end of zip", name));
        }
        let raw = &data[ds..ds + csize];
        let bytes = match method {
            0 => raw.to_vec(),
            8 => miniz_oxide::inflate::decompress_to_vec(raw)
                .map_err(|e| format!("{}: inflate failed: {:?}", name, e))?,
            m => return Err(format!("{}: zip method {} is not understood", name, m)),
        };
        if bytes.len() != usize_ && usize_ != 0 {
            return Err(format!("{}: inflated to {} bytes, header says {}", name, bytes.len(), usize_));
        }
        out.insert(name, bytes);
    }
    Ok(out)
}
