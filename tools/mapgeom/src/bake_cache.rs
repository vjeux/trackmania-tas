//! THE BAKE CACHE — `tiny-library` never bakes the same model twice.
//!
//! A tiny build bakes every one of a map's ~400 block models, its item models
//! and its tree species into half-size items, and the result is identical from
//! one build to the next unless the converter or its knobs changed (vjeux,
//! 2026-09-21: "Why don't we cache all these bakes!?"). A bake is a pure
//! function of
//!
//! * WHAT is baked: a block's RECIPE (`BlockBake::recipe`: prefabs + waypoint +
//!   units + modifier — the key that already shares one item between two block
//!   keys of one build) with the water-row flag, an item's pack path (or the
//!   hash of its bytes when the map embeds it) with its variant and light skin,
//!   a tree species' model path; plus the scale and the collection,
//! * the converter build (`MAPGEOM_BUILD_ID`: the git hash and a hash of the
//!   converter's sources, from `build.rs`) and every `TINY_*` knob that steers
//!   a bake (the whole `TINY_*` environment minus the knobs that never change
//!   a bake — naming, the cache itself, the worker count; a knob that does not
//!   matter costs a miss, never a wrong hit),
//!
//! so the key is the SHA-256 of all of that, and the value is the bake's output
//! under a NEUTRAL ident (`AC00000000.Item.Gbx` — same length as every alias):
//! the item bytes, the pictures the item names, and the small sidecar the
//! library builder reads back (visual / moving-part / light / material counts,
//! notes, detail-level switch distances, collision triangles, vegetation
//! entities, waypoint kind, spawn, trigger, a tree's measurements). On a hit the
//! ident is renamed to the build's alias (`crystal::rename_ident`, the same
//! edit `tiny-library` does for pack items) and the sidecar rebuilt; the caller
//! cannot tell a hit from a bake.
//!
//! Location: `$TINY_BAKE_CACHE`, else `~/.cache/tiny-bake`; `TINY_BAKE_CACHE=0`
//! disables it. One directory per key: `item.bin`, `meta.bin`, `pictures/`.
//! Writes are atomic (temp dir + rename), so a killed build leaves no half entry
//! and two workers storing one key cannot corrupt it.

use crate::static_item::surface::{CPlugSurface, Triangle};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// The neutral ident cached bytes are written under.
pub const NEUTRAL_IDENT: &str = "AC00000000.Item.Gbx";

/// The converter build the cache keys on: the git commit and a hash of the
/// converter's sources (`build.rs`), so an uncommitted edit misses too.
pub const BUILD_ID: &str = match option_env!("MAPGEOM_BUILD_ID") {
    Some(id) => id,
    None => env!("CARGO_PKG_VERSION"),
};

/// A tree bake's measurements (`static_item::build::VegetBake`), for the
/// report and the height / hull rules of the tree baker.
#[derive(Clone, Default)]
pub struct TreeMeta {
    pub model: String,
    pub levels: Vec<usize>,
    pub switch: Vec<f32>,
    pub textures: Vec<(String, usize)>,
    pub height: f32,
    pub radius: f32,
    pub hull_triangles: usize,
}

/// What a bake produces that the library builder consumes.
pub struct Baked {
    pub bytes: Vec<u8>,
    pub pictures: BTreeMap<String, Vec<u8>>,
    pub n_visuals: usize,
    pub n_dyna: usize,
    pub n_lights: usize,
    pub n_materials: usize,
    pub lod_max_dist: Vec<f32>,
    pub notes: Vec<String>,
    pub surf_vertices: Vec<[f32; 3]>,
    pub surf_triangles: Vec<Triangle>,
    pub veget: Vec<(String, [f32; 12])>,
    pub waypoint_type: Option<i32>,
    pub spawn: [f32; 3],
    pub trigger: Option<CPlugSurface>,
    pub deepened: bool,
    pub tree: Option<TreeMeta>,
}

impl Baked {
    /// The sidecar of a fresh bake: everything the builder reads off `m` after
    /// a bake, copied.
    pub fn of(bytes: &[u8], m: &crate::static_item::build::Merged, deepened: bool) -> Baked {
        Baked {
            bytes: bytes.to_vec(),
            pictures: m.pictures.iter().cloned().collect(),
            n_visuals: m.visuals.len(),
            n_dyna: m.dyna.len(),
            n_lights: m.lights_out.len(),
            n_materials: m.materials.len(),
            lod_max_dist: m.lod_max_dist.clone(),
            notes: m.notes.clone(),
            surf_vertices: m.surf_vertices.clone(),
            surf_triangles: m.surf_triangles.clone(),
            veget: m.veget.clone(),
            waypoint_type: m.waypoint_type,
            spawn: m.spawn,
            trigger: m.trigger.clone(),
            deepened,
            tree: None,
        }
    }

    /// The `Merged` a cache hit stands in with: the fields the builder reads
    /// (no visuals, materials or moving parts — their COUNTS are `n_*`).
    pub fn merged(&self) -> crate::static_item::build::Merged {
        let mut m = crate::static_item::build::Merged::default();
        m.pictures = self.pictures.iter().map(|(k, v)| (k.clone(), v.clone())).collect();
        m.notes = self.notes.clone();
        m.surf_vertices = self.surf_vertices.clone();
        m.surf_triangles = self.surf_triangles.clone();
        m.veget = self.veget.clone();
        m.waypoint_type = self.waypoint_type;
        m.spawn = self.spawn;
        m.trigger = self.trigger.clone();
        m.lod_max_dist = self.lod_max_dist.clone();
        m
    }
}

pub fn dir() -> Option<PathBuf> {
    match std::env::var("TINY_BAKE_CACHE") {
        Ok(v) if v == "0" => None,
        Ok(v) if !v.is_empty() => Some(PathBuf::from(v)),
        _ => std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".cache").join("tiny-bake")),
    }
}

/// The knobs that never change a bake: the per-build naming knobs (they change
/// the ident, never the bytes), the cache location, the worker count.
const NON_BAKE_KNOBS: &[&str] = &["TINY_ALIAS_BASE", "TINY_PICTURE_SUFFIX", "TINY_BAKE_CACHE", "TINY_BAKE_JOBS"];

/// The bake key: what is baked + scale + collection + water row + converter build + knobs.
pub fn key(recipe: &str, scale: f32, collection: u32, at_water_row: bool, water: Option<(u8, f32)>) -> String {
    let mut h = Sha256::new();
    h.update(BUILD_ID.as_bytes());
    h.update(b"\0");
    h.update(recipe.as_bytes());
    h.update(b"\0");
    h.update(format!("scale={scale} coll={collection} wrow={at_water_row} water={water:?}").as_bytes());
    // the detail pick is a global FLAG, not a TINY_ knob: a build down the size
    // ladder (--lod-pick N [--lod-pick-min-verts V]) bakes other bytes — the
    // first pipeline run of the giant campaigns got its cached full-detail
    // items back at every rung (2026-09-22)
    if let Some(p) = crate::static_item::lod::lod_pick() {
        h.update(format!(" lodpick={} minverts={}", p.level, p.min_verts).as_bytes());
    }
    let mut knobs: Vec<(String, String)> = std::env::vars().filter(|(k, _)| k.starts_with("TINY_") && !NON_BAKE_KNOBS.contains(&k.as_str())).collect();
    knobs.sort();
    for (k, v) in knobs {
        h.update(b"\0");
        h.update(k.as_bytes());
        h.update(b"=");
        h.update(v.as_bytes());
    }
    h.hex()
}

/// The SHA-256 of some bytes, hex: the identity of an embedded or local item
/// file in a key (its name alone could stand for other bytes tomorrow).
pub fn sha256_hex(bytes: &[u8]) -> String {
    let mut h = Sha256::new();
    h.update(bytes);
    h.hex()
}
fn w_u32(v: &mut Vec<u8>, x: u32) {
    v.extend_from_slice(&x.to_le_bytes());
}
fn w_f32(v: &mut Vec<u8>, x: f32) {
    v.extend_from_slice(&x.to_le_bytes());
}
fn w_bytes(v: &mut Vec<u8>, b: &[u8]) {
    w_u32(v, b.len() as u32);
    v.extend_from_slice(b);
}
fn w_str(v: &mut Vec<u8>, s: &str) {
    w_bytes(v, s.as_bytes());
}

struct Cursor<'a> {
    b: &'a [u8],
    o: usize,
}
impl<'a> Cursor<'a> {
    fn take(&mut self, n: usize) -> Result<&'a [u8], String> {
        let end = self.o.checked_add(n).ok_or("overflow")?;
        if end > self.b.len() {
            return Err("truncated cache entry".into());
        }
        let s = &self.b[self.o..end];
        self.o = end;
        Ok(s)
    }
    fn u32(&mut self) -> Result<u32, String> {
        Ok(u32::from_le_bytes(self.take(4)?.try_into().unwrap()))
    }
    fn f32(&mut self) -> Result<f32, String> {
        Ok(f32::from_le_bytes(self.take(4)?.try_into().unwrap()))
    }
    fn bytes(&mut self) -> Result<Vec<u8>, String> {
        let n = self.u32()? as usize;
        Ok(self.take(n)?.to_vec())
    }
    fn str(&mut self) -> Result<String, String> {
        String::from_utf8(self.bytes()?).map_err(|e| e.to_string())
    }
}

const META_MAGIC: u32 = 0x5442_4B32; // "TBK2" (TBK1 had no counts, lod, tree)

fn w_tris(v: &mut Vec<u8>, tris: &[Triangle]) {
    w_u32(v, tris.len() as u32);
    for t in tris {
        for k in 0..3 {
            w_u32(v, t.indices[k]);
        }
        v.push(t.material_id);
        v.push(t.gameplay);
        v.extend_from_slice(&t.surface_index.to_le_bytes());
    }
}

fn encode_meta(b: &Baked) -> Vec<u8> {
    let mut v = Vec::new();
    w_u32(&mut v, META_MAGIC);
    w_u32(&mut v, b.n_visuals as u32);
    w_u32(&mut v, b.n_dyna as u32);
    w_u32(&mut v, b.n_lights as u32);
    w_u32(&mut v, b.n_materials as u32);
    w_u32(&mut v, b.lod_max_dist.len() as u32);
    for d in &b.lod_max_dist {
        w_f32(&mut v, *d);
    }
    w_u32(&mut v, b.notes.len() as u32);
    for n in &b.notes {
        w_str(&mut v, n);
    }
    w_u32(&mut v, b.surf_vertices.len() as u32);
    for p in &b.surf_vertices {
        for k in 0..3 {
            w_f32(&mut v, p[k]);
        }
    }
    w_tris(&mut v, &b.surf_triangles);
    w_u32(&mut v, b.veget.len() as u32);
    for (p, iso) in &b.veget {
        w_str(&mut v, p);
        for x in iso {
            w_f32(&mut v, *x);
        }
    }
    match b.waypoint_type {
        Some(t) => {
            v.push(1);
            w_u32(&mut v, t as u32);
        }
        None => v.push(0),
    }
    for k in 0..3 {
        w_f32(&mut v, b.spawn[k]);
    }
    match &b.trigger {
        Some(s) => {
            v.push(1);
            let mut body = Vec::new();
            let mut lb = crate::crystal_model::LookbackState::default();
            let mut w = crate::crystal_model::Wr { w: &mut body, lb: &mut lb };
            s.write(&mut w);
            w_bytes(&mut v, &body);
        }
        None => v.push(0),
    }
    v.push(b.deepened as u8);
    match &b.tree {
        Some(t) => {
            v.push(1);
            w_str(&mut v, &t.model);
            w_u32(&mut v, t.levels.len() as u32);
            for l in &t.levels {
                w_u32(&mut v, *l as u32);
            }
            w_u32(&mut v, t.switch.len() as u32);
            for s in &t.switch {
                w_f32(&mut v, *s);
            }
            w_u32(&mut v, t.textures.len() as u32);
            for (f, n) in &t.textures {
                w_str(&mut v, f);
                w_u32(&mut v, *n as u32);
            }
            w_f32(&mut v, t.height);
            w_f32(&mut v, t.radius);
            w_u32(&mut v, t.hull_triangles as u32);
        }
        None => v.push(0),
    }
    v
}

/// The sidecar decoded into a `Baked` whose `bytes` and `pictures` are still
/// empty (the caller fills them).
fn decode_meta(buf: &[u8]) -> Result<Baked, String> {
    let mut c = Cursor { b: buf, o: 0 };
    if c.u32()? != META_MAGIC {
        return Err("bad cache magic".into());
    }
    let n_visuals = c.u32()? as usize;
    let n_dyna = c.u32()? as usize;
    let n_lights = c.u32()? as usize;
    let n_materials = c.u32()? as usize;
    let nl = c.u32()? as usize;
    let mut lod_max_dist = Vec::with_capacity(nl);
    for _ in 0..nl {
        lod_max_dist.push(c.f32()?);
    }
    let nn = c.u32()? as usize;
    let mut notes = Vec::with_capacity(nn);
    for _ in 0..nn {
        notes.push(c.str()?);
    }
    let nv = c.u32()? as usize;
    let mut surf_vertices = Vec::with_capacity(nv);
    for _ in 0..nv {
        surf_vertices.push([c.f32()?, c.f32()?, c.f32()?]);
    }
    let nt = c.u32()? as usize;
    let mut surf_triangles = Vec::with_capacity(nt);
    for _ in 0..nt {
        let indices = [c.u32()?, c.u32()?, c.u32()?];
        let material_id = c.take(1)?[0];
        let gameplay = c.take(1)?[0];
        let surface_index = i16::from_le_bytes(c.take(2)?.try_into().unwrap());
        surf_triangles.push(Triangle { indices, material_id, gameplay, surface_index });
    }
    let ng = c.u32()? as usize;
    let mut veget = Vec::with_capacity(ng);
    for _ in 0..ng {
        let p = c.str()?;
        let mut iso = [0f32; 12];
        for x in iso.iter_mut() {
            *x = c.f32()?;
        }
        veget.push((p, iso));
    }
    let waypoint_type = if c.take(1)?[0] == 1 { Some(c.u32()? as i32) } else { None };
    let spawn = [c.f32()?, c.f32()?, c.f32()?];
    let trigger = if c.take(1)?[0] == 1 {
        let body = c.bytes()?;
        let mut r = crate::crystal_model::Rd::new(&body, 0, crate::crystal_model::LookbackState::default());
        Some(CPlugSurface::parse(&mut r)?)
    } else {
        None
    };
    let deepened = c.take(1)?[0] == 1;
    let tree = if c.take(1)?[0] == 1 {
        let model = c.str()?;
        let n = c.u32()? as usize;
        let mut levels = Vec::with_capacity(n);
        for _ in 0..n {
            levels.push(c.u32()? as usize);
        }
        let n = c.u32()? as usize;
        let mut switch = Vec::with_capacity(n);
        for _ in 0..n {
            switch.push(c.f32()?);
        }
        let n = c.u32()? as usize;
        let mut textures = Vec::with_capacity(n);
        for _ in 0..n {
            let f = c.str()?;
            let k = c.u32()? as usize;
            textures.push((f, k));
        }
        let height = c.f32()?;
        let radius = c.f32()?;
        let hull_triangles = c.u32()? as usize;
        Some(TreeMeta { model, levels, switch, textures, height, radius, hull_triangles })
    } else {
        None
    };
    Ok(Baked { bytes: Vec::new(), pictures: BTreeMap::new(), n_visuals, n_dyna, n_lights, n_materials, lod_max_dist, notes, surf_vertices, surf_triangles, veget, waypoint_type, spawn, trigger, deepened, tree })
}

/// A cached bake under `ident`, or None (miss, disabled, or unreadable).
pub fn get(key: &str, ident: &str) -> Option<Baked> {
    let d = dir()?.join(key);
    let item = std::fs::read(d.join("item.bin")).ok()?;
    let meta = std::fs::read(d.join("meta.bin")).ok()?;
    let mut b = decode_meta(&meta).ok()?;
    if let Ok(rd) = std::fs::read_dir(d.join("pictures")) {
        for e in rd.flatten() {
            if let (Some(name), Ok(bytes)) = (e.file_name().to_str().map(String::from), std::fs::read(e.path())) {
                b.pictures.insert(name, bytes);
            }
        }
    }
    // an entry with no item bytes (a vegetation CLUSTER item: no mesh, its trees
    // re-emitted as stock items) has nothing to rename
    b.bytes = if ident == NEUTRAL_IDENT || item.is_empty() { item } else { crate::crystal::rename_ident(&item, NEUTRAL_IDENT, ident) };
    Some(b)
}

/// Store a bake (whose bytes carry `ident`) under `key`, neutralised. Errors are
/// swallowed: a cache that cannot be written costs a re-bake next time, nothing else.
pub fn put(key: &str, ident: &str, b: &Baked) {
    let Some(root) = dir() else { return };
    let final_dir = root.join(key);
    if final_dir.exists() {
        return;
    }
    // (the thread id keeps two workers of one process apart)
    let tmp = root.join(format!(".{key}.{}.{:?}", std::process::id(), std::thread::current().id()).replace(['(', ')', ' '], ""));
    let _ = std::fs::remove_dir_all(&tmp);
    if std::fs::create_dir_all(tmp.join("pictures")).is_err() {
        return;
    }
    // no bytes (a cluster item whose trees ride as `v@` rows): nothing to rename —
    // Summer 20's Spring/SpringCherryTree clusters panicked here (2026-09-22)
    let neutral = if ident == NEUTRAL_IDENT || b.bytes.is_empty() { b.bytes.clone() } else { crate::crystal::rename_ident(&b.bytes, ident, NEUTRAL_IDENT) };
    let ok = std::fs::write(tmp.join("item.bin"), &neutral).is_ok()
        && std::fs::write(tmp.join("meta.bin"), encode_meta(b)).is_ok()
        && b.pictures.iter().all(|(name, bytes)| Path::new(name).file_name().map(|f| std::fs::write(tmp.join("pictures").join(f), bytes).is_ok()).unwrap_or(false));
    if ok && std::fs::rename(&tmp, &final_dir).is_ok() {
        return;
    }
    let _ = std::fs::remove_dir_all(&tmp);
}

// ---- a small SHA-256 (no new crate for one hash) ------------------------------

struct Sha256 {
    state: [u32; 8],
    buf: Vec<u8>,
    len: u64,
}

const K: [u32; 64] = [
    0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4, 0xab1c5ed5, 0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe, 0x9bdc06a7, 0xc19bf174, 0xe49b69c1, 0xefbe4786,
    0x0fc19dc6, 0x240ca1cc, 0x2de92c6f, 0x4a7484aa, 0x5cb0a9dc, 0x76f988da, 0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7, 0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967, 0x27b70a85, 0x2e1b2138, 0x4d2c6dfc, 0x53380d13,
    0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85, 0xa2bfe8a1, 0xa81a664b, 0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070, 0x19a4c116, 0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a,
    0x5b9cca4f, 0x682e6ff3, 0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7, 0xc67178f2,
];

impl Sha256 {
    fn new() -> Self {
        Sha256 { state: [0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab, 0x5be0cd19], buf: Vec::new(), len: 0 }
    }
    fn update(&mut self, data: &[u8]) {
        self.len += data.len() as u64;
        self.buf.extend_from_slice(data);
        while self.buf.len() >= 64 {
            let block: [u8; 64] = self.buf[..64].try_into().unwrap();
            self.compress(&block);
            self.buf.drain(..64);
        }
    }
    fn compress(&mut self, block: &[u8; 64]) {
        let mut w = [0u32; 64];
        for i in 0..16 {
            w[i] = u32::from_be_bytes([block[4 * i], block[4 * i + 1], block[4 * i + 2], block[4 * i + 3]]);
        }
        for i in 16..64 {
            let s0 = w[i - 15].rotate_right(7) ^ w[i - 15].rotate_right(18) ^ (w[i - 15] >> 3);
            let s1 = w[i - 2].rotate_right(17) ^ w[i - 2].rotate_right(19) ^ (w[i - 2] >> 10);
            w[i] = w[i - 16].wrapping_add(s0).wrapping_add(w[i - 7]).wrapping_add(s1);
        }
        let [mut a, mut b, mut c, mut d, mut e, mut f, mut g, mut h] = self.state;
        for i in 0..64 {
            let s1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
            let ch = (e & f) ^ (!e & g);
            let t1 = h.wrapping_add(s1).wrapping_add(ch).wrapping_add(K[i]).wrapping_add(w[i]);
            let s0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
            let maj = (a & b) ^ (a & c) ^ (b & c);
            let t2 = s0.wrapping_add(maj);
            h = g;
            g = f;
            f = e;
            e = d.wrapping_add(t1);
            d = c;
            c = b;
            b = a;
            a = t1.wrapping_add(t2);
        }
        for (s, v) in self.state.iter_mut().zip([a, b, c, d, e, f, g, h]) {
            *s = s.wrapping_add(v);
        }
    }
    fn hex(mut self) -> String {
        let bits = self.len * 8;
        let mut pad = vec![0x80u8];
        while (self.buf.len() + pad.len()) % 64 != 56 {
            pad.push(0);
        }
        pad.extend_from_slice(&bits.to_be_bytes());
        let len_before = self.len;
        self.update(&pad);
        self.len = len_before;
        self.state.iter().map(|w| format!("{w:08x}")).collect()
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn sha256_known_answer() {
        let mut h = super::Sha256::new();
        h.update(b"abc");
        assert_eq!(h.hex(), "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad");
        let mut h = super::Sha256::new();
        h.update(b"");
        assert_eq!(h.hex(), "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855");
    }
}
