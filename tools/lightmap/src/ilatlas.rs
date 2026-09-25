//! The peel's colour from the game's own ILightInput atlas (`--ilightinput-from`): the peel pixel shaders
//! (PS 17131 / 17134 / 17545 / 8526) colour a fragment with `TMapILightInput.Sample(SGbxClamp_Aniso, uv_lm)`
//! — the 2048² R11G11B10 atlas the setup chain produced (sweep 0: the dilated sun × MDiffuse of `e2e.rs`,
//! `lmtool e2e-check --dump-dir`) sampled at the fragment's LIGHTMAP coordinate, `uv_lm = TexCoord1 · st.xy +
//! st.zw` with `st` the object's chart placement from the LM instance stream (vb_17033 of the capture).
//!
//! The port's peel knows a fragment as (instance, triangle, hit point); this module turns that into the game's
//! atlas coordinate: an item's triangle carries its authored TexCoord1 (the model's `uv`) and the item maps to a
//! game instance by its world translation; a zone tile (decoration in the port's scene) maps by its footprint to
//! the game's tile instance (rows built from the quaternion as VS 15183 does), the tile mesh's TexCoord1 being
//! affine in the tile-local x / z (vb_5350: u = 0.04799 + 0.028097·x, v = 0.95945 − 0.028331·z).

use crate::passdiff::Buf;
use crate::sunpass::{parse_instances, rotation_rows, LmInstance};
use std::collections::HashMap;

pub struct IlAtlas {
    /// The atlas (2048² × 3), R11G11B10-valued.
    pub buf: Buf,
    pub insts: Vec<LmInstance>,
    pub rows: Vec<[[f32; 3]; 3]>,
    /// Tile instances by the integer corner of their footprint (x / 32, z / 32).
    pub tile_of: HashMap<(i32, i32), usize>,
    /// The tile mesh's TexCoord1 as an affine map of the local (x, z): u = a·x + b, v = c·z + d.
    pub tile_uv: [f32; 4],
    /// The tile's local extent (m).
    pub tile_size: f32,
    /// The game instances that are items (index < first tile).
    pub n_items: usize,
    /// Every instance by its translation rounded to half a metre (any_by_translation).
    pub by_pos: HashMap<(i32, i32, i32), Vec<usize>>,
    /// Counters (fragments coloured from the atlas, fragments with no mapping).
    pub hits: std::sync::atomic::AtomicUsize,
    pub misses: std::sync::atomic::AtomicUsize,
}

/// A raw R11G11B10 image (2048² u32 LE) or an RGBA16F one (2048² × 4 halves) → the atlas buffer.
pub fn load_atlas(path: &std::path::Path) -> Result<Buf, String> {
    let b = std::fs::read(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let (w, h) = (2048u32, 2048u32);
    let mut out = Buf::new(w, h, 3);
    if b.len() == (w * h * 4) as usize {
        for i in 0..(w * h) as usize {
            let v = u32::from_le_bytes(b[i * 4..i * 4 + 4].try_into().unwrap());
            let rgb = crate::gpufmt::unpack_r11g11b10(v);
            out.data[i * 3..i * 3 + 3].copy_from_slice(&rgb);
        }
    } else if b.len() == (w * h * 8) as usize {
        for i in 0..(w * h) as usize {
            for c in 0..3 {
                out.data[i * 3 + c] = crate::gpufmt::decode_f16(u16::from_le_bytes(b[i * 8 + c * 2..i * 8 + c * 2 + 2].try_into().unwrap()));
            }
        }
    } else {
        return Err(format!("{}: {} bytes is neither a 2048² R11G11B10 nor an RGBA16F image", path.display(), b.len()));
    }
    Ok(out)
}

impl IlAtlas {
    /// `atlas` = the ILightInput image, `instances` = the LM instance stream (vb_17033), `tile_vb` = the tile
    /// mesh's vertex buffer (vb_5350, stride 28: POSITION @0, TEXCOORD1 @20).
    pub fn new(atlas: Buf, instances: &[u8], tile_vb: &[u8], n_items: usize) -> IlAtlas {
        let insts = parse_instances(instances);
        // the tile mesh: an affine TexCoord1 in local x / z from the vertices' extremes
        let verts: Vec<([f32; 3], [f32; 2])> = tile_vb.chunks_exact(28).map(|b| { let f = |o: usize| f32::from_le_bytes(b[o..o + 4].try_into().unwrap()); ([f(0), f(4), f(8)], [f(20), f(24)]) }).collect();
        IlAtlas::from_parts(atlas, insts, &verts, n_items)
    }

    /// The same from an LM scene built without a capture (`lmmesh::lm_scene_from_map`): its instances and the tile mesh's
    /// (position, TexCoord1) vertices.
    pub fn from_lm_scene(atlas: Buf, lm: &crate::lmaccum::LmScene) -> IlAtlas {
        let tile_k = lm.meshes.iter().enumerate().find(|(k, _)| lm.inst_count[*k] >= 1000).map(|(k, _)| k);
        let verts: Vec<([f32; 3], [f32; 2])> = tile_k.map(|k| lm.meshes[k].verts.iter().map(|v| (v.pos, v.uv)).collect()).unwrap_or_default();
        let n_items = tile_k.map(|k| lm.inst_first[k]).unwrap_or(lm.instances.len());
        IlAtlas::from_parts(atlas, lm.instances.clone(), &verts, n_items)
    }

    pub fn from_parts(atlas: Buf, insts: Vec<LmInstance>, verts: &[([f32; 3], [f32; 2])], n_items: usize) -> IlAtlas {
        let rows: Vec<[[f32; 3]; 3]> = insts.iter().map(|i| rotation_rows(i.q)).collect();
        let (mut xmin, mut xmax, mut zmin, mut zmax) = (f32::MAX, f32::MIN, f32::MAX, f32::MIN);
        let (mut u_at_xmin, mut u_at_xmax, mut v_at_zmin, mut v_at_zmax) = (0f32, 0f32, 0f32, 0f32);
        for (p, uv) in verts {
            if p[0] < xmin { xmin = p[0]; u_at_xmin = uv[0]; }
            if p[0] > xmax { xmax = p[0]; u_at_xmax = uv[0]; }
            if p[2] < zmin { zmin = p[2]; v_at_zmin = uv[1]; }
            if p[2] > zmax { zmax = p[2]; v_at_zmax = uv[1]; }
        }
        let tile_size = (xmax - xmin).max(1e-6);
        let a = (u_at_xmax - u_at_xmin) / (xmax - xmin).max(1e-6);
        let c = (v_at_zmax - v_at_zmin) / (zmax - zmin).max(1e-6);
        let tile_uv = [a, u_at_xmin - a * xmin, c, v_at_zmin - c * zmin];
        // the tiles' footprints: the local square [xmin, xmax] × [zmin, zmax] through the instance's rotation and translation
        let mut tile_of = HashMap::new();
        for (k, inst) in insts.iter().enumerate().skip(n_items) {
            let r = &rows[k];
            let corners = [[xmin, 0.0, zmin], [xmax, 0.0, zmin], [xmin, 0.0, zmax], [xmax, 0.0, zmax]];
            let (mut wx, mut wz) = (f32::MAX, f32::MAX);
            for c in corners {
                let w = [r[0][0] * c[0] + r[0][1] * c[1] + r[0][2] * c[2] + inst.t[0], 0.0, r[2][0] * c[0] + r[2][1] * c[1] + r[2][2] * c[2] + inst.t[2]];
                wx = wx.min(w[0]);
                wz = wz.min(w[2]);
            }
            tile_of.insert(((wx / tile_size).round() as i32, (wz / tile_size).round() as i32), k);
        }
        let mut by_pos: HashMap<(i32, i32, i32), Vec<usize>> = HashMap::new();
        for (k, inst) in insts.iter().enumerate() { by_pos.entry(((inst.t[0] * 2.0).round() as i32, (inst.t[1] * 2.0).round() as i32, (inst.t[2] * 2.0).round() as i32)).or_default().push(k); }
        IlAtlas { buf: atlas, insts, rows, tile_of, tile_uv, tile_size, n_items, by_pos, hits: Default::default(), misses: Default::default() }
    }

    /// The atlas coordinate of a point on a zone tile (world x / z), or None off every tile.
    pub fn tile_uv_at(&self, p: [f32; 3]) -> Option<[f32; 2]> {
        let key = ((p[0] / self.tile_size).floor() as i32, (p[2] / self.tile_size).floor() as i32);
        let k = *self.tile_of.get(&key)?;
        let (inst, r) = (&self.insts[k], &self.rows[k]);
        // local = rowsᵀ · (p − t) (the rows are orthonormal)
        let d = [p[0] - inst.t[0], p[1] - inst.t[1], p[2] - inst.t[2]];
        let local = [r[0][0] * d[0] + r[1][0] * d[1] + r[2][0] * d[2], 0.0, r[0][2] * d[0] + r[1][2] * d[1] + r[2][2] * d[2]];
        let uv1 = [self.tile_uv[0] * local[0] + self.tile_uv[1], self.tile_uv[2] * local[2] + self.tile_uv[3]];
        Some([uv1[0] * inst.st[0] + inst.st[2], uv1[1] * inst.st[1] + inst.st[3]])
    }

    /// The atlas coordinate of an item's point from its authored TexCoord1 and the game instance it maps to.
    pub fn item_uv(&self, game_inst: usize, uv1: [f32; 2]) -> Option<[f32; 2]> {
        let inst = self.insts.get(game_inst)?;
        Some([uv1[0] * inst.st[0] + inst.st[2], uv1[1] * inst.st[1] + inst.st[3]])
    }

    /// The game instance whose translation is nearest to `t` among the items (within 0.5 m), for the port's
    /// item instances.
    pub fn item_by_translation(&self, t: [f32; 3]) -> Option<usize> {
        let mut best: Option<(usize, f32)> = None;
        for (k, inst) in self.insts.iter().enumerate().take(self.n_items) {
            let d = ((inst.t[0] - t[0]).powi(2) + (inst.t[1] - t[1]).powi(2) + (inst.t[2] - t[2]).powi(2)).sqrt();
            if best.map_or(true, |b| d < b.1) {
                best = Some((k, d));
            }
        }
        best.filter(|b| b.1 < 0.5).map(|b| b.0)
    }

    /// The game instance at translation `t` among EVERY LM instance (the record scene: the block / clip / wall entities and
    /// the tiles are port instances too) — a hash of the translation rounded to the centimetre, then its 27 neighbours;
    /// the nearest within 0.5 m.
    pub fn any_by_translation(&self, t: [f32; 3]) -> Option<usize> {
        let key = |p: [f32; 3]| ((p[0] * 2.0).round() as i32, (p[1] * 2.0).round() as i32, (p[2] * 2.0).round() as i32);
        let k0 = key(t);
        let mut best: Option<(usize, f32)> = None;
        for dx in -1..=1 { for dy in -1..=1 { for dz in -1..=1 {
            if let Some(list) = self.by_pos.get(&(k0.0 + dx, k0.1 + dy, k0.2 + dz)) {
                for &k in list {
                    let inst = &self.insts[k];
                    let d = ((inst.t[0] - t[0]).powi(2) + (inst.t[1] - t[1]).powi(2) + (inst.t[2] - t[2]).powi(2)).sqrt();
                    if best.map_or(true, |b| d < b.1) { best = Some((k, d)); }
                }
            }
        } } }
        best.filter(|b| b.1 < 0.5).map(|b| b.0)
    }

    /// The peel colour at an atlas coordinate: the bilinear (magnified anisotropic) tap, `g = max(g, 1e-5)`,
    /// the R11G11B10 store (sweep1::peel_color).
    pub fn colour(&self, uv: [f32; 2], front: bool) -> [f32; 3] {
        crate::sweep1::peel_color(&self.buf, uv[0], uv[1], front)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_tile_maps_its_footprint_through_the_instance_st() {
        // one unrotated tile at (64, 4, 96) with ST (0.01, 0.01, 0.5, 0.25); the tile mesh: a 32 m square with
        // u = 0.05 + 0.028·x, v = 0.95 − 0.028·z
        let mut ib = Vec::new();
        for v in [0.0f32, 0.0, 0.0, 1.0, 64.0, 4.0, 96.0, 1.0, 0.01, 0.01, 0.5, 0.25] { ib.extend_from_slice(&v.to_le_bytes()); }
        let mut vb = Vec::new();
        for (x, z) in [(0.0f32, 0.0f32), (32.0, 0.0), (0.0, 32.0), (32.0, 32.0)] {
            for v in [x, 4.0, z, 0.0, 0.0, 0.05 + 0.028 * x, 0.95 - 0.028 * z] { vb.extend_from_slice(&v.to_le_bytes()); }
        }
        let il = IlAtlas::new(Buf::new(2048, 2048, 3), &ib, &vb, 0);
        let uv = il.tile_uv_at([64.0 + 16.0, 4.0, 96.0 + 8.0]).unwrap();
        let (u1, v1) = (0.05 + 0.028 * 16.0, 0.95 - 0.028 * 8.0);
        assert!((uv[0] - (u1 * 0.01 + 0.5)).abs() < 1e-6 && (uv[1] - (v1 * 0.01 + 0.25)).abs() < 1e-6, "{uv:?}");
        assert!(il.tile_uv_at([10.0, 4.0, 10.0]).is_none());
    }
}

impl IlAtlas {
    /// The port's item instances → the game's instances (by world translation); the result indexes
    /// `scene.instances`.
    pub fn map_items(&self, scene: &crate::geometry::Scene) -> Vec<Option<usize>> {
        scene.instances.iter().map(|inst| {
            let t = mapgeom::geom::apply(&inst.xf, [0.0, 0.0, 0.0]);
            if inst.item < scene.item_count { self.item_by_translation(t) } else { self.any_by_translation(t) }
        }).collect()
    }
}

/// Everything the peel needs to colour a fragment from the atlas: the atlas + the port→game instance map.
pub struct IlSource {
    pub atlas: IlAtlas,
    pub item_map: Vec<Option<usize>>,
}

impl IlSource {
    /// The peel colour of a fragment on triangle `wt` (BVH world triangle) at `hit_p`, or None when the fragment
    /// has no lightmap coordinate in the game (the environment block; an unmapped item).
    pub fn colour(&self, scene: &crate::geometry::Scene, wt: &crate::bvh::WTri, hit_p: [f32; 3], front: bool) -> Option<[f32; 3]> {
        let r = self.colour_inner(scene, wt, hit_p, front);
        if r.is_none() {
            self.atlas.misses.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        }
        r
    }

    fn colour_inner(&self, scene: &crate::geometry::Scene, wt: &crate::bvh::WTri, hit_p: [f32; 3], front: bool) -> Option<[f32; 3]> {
        let uv = if wt.inst == crate::geometry::DECOR_INST {
            let dt = scene.decor.get(wt.tri as usize)?;
            if dt.env || dt.water {
                return None;
            }
            self.atlas.tile_uv_at(hit_p)?
        } else {
            let g = (*self.item_map.get(wt.inst as usize)?)?;
            let instance = &scene.instances[wt.inst as usize];
            let m = &scene.models[instance.model];
            let t = m.tris.get(wt.tri as usize)?;
            // barycentrics of the hit on (e1, e2)
            let sub = |a: [f32; 3], b: [f32; 3]| [a[0] - b[0], a[1] - b[1], a[2] - b[2]];
            let dot = |a: [f32; 3], b: [f32; 3]| a[0] * b[0] + a[1] * b[1] + a[2] * b[2];
            let v = sub(hit_p, wt.p0);
            let (d00, d01, d11, d20, d21) = (dot(wt.e1, wt.e1), dot(wt.e1, wt.e2), dot(wt.e2, wt.e2), dot(v, wt.e1), dot(v, wt.e2));
            let den = d00 * d11 - d01 * d01;
            if den.abs() < 1e-12 {
                return None;
            }
            let b1 = ((d11 * d20 - d01 * d21) / den).clamp(0.0, 1.0);
            let b2 = ((d00 * d21 - d01 * d20) / den).clamp(0.0, 1.0);
            let uv1 = [t.uv[0][0] + b1 * (t.uv[1][0] - t.uv[0][0]) + b2 * (t.uv[2][0] - t.uv[0][0]), t.uv[0][1] + b1 * (t.uv[1][1] - t.uv[0][1]) + b2 * (t.uv[2][1] - t.uv[0][1])];
            self.atlas.item_uv(g, uv1)?
        };
        self.atlas.hits.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        Some(self.atlas.colour(uv, front))
    }
}

impl std::fmt::Debug for IlSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "IlSource({} instances, {} tiles, {} items mapped)", self.atlas.insts.len(), self.atlas.tile_of.len(), self.item_map.iter().flatten().count())
    }
}

/// The sweep's H-basis MRTs handed back through `BakeParams::hb_out`.
pub struct HbSlot(pub std::sync::Mutex<Option<crate::lmaccum::HbTargets>>);

impl std::fmt::Debug for HbSlot {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "HbSlot({})", if self.0.lock().map(|g| g.is_some()).unwrap_or(false) { "filled" } else { "empty" })
    }
}
