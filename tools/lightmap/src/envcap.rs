//! The game's ENVIRONMENT BLOCK of a peel, read off the capture (pwc-day frame 127448, the world peel of
//! its first direction): the sea box (draw eid 1009 — an inverted box, black on its GPU-back faces), the
//! four terrain patches (eids 1028 / 1033 / 1071 / 1076, VS 16748: world = VB position × GbxVisualToWorld
//! = identity) and — elsewhere — the sky dome (`skygrad::dome_radiance`). The terrain's colour in the
//! peel is black in the first sweep (its ILightInput is 0 there); what it shows in later sweeps is not
//! transcribed yet.
//!
//! Sources: `logs/mesh-frame127448.json` (input layouts, VB files), `mesh/frame127448/e00XXXX_vb0_*.bin.gz`
//! (VB bytes), `env/frame127448/mesh/e00XXXX_vsout.bin` + `_vsout_indices.bin` (post-VS positions in the
//! peel's clip space and the index buffer — the patches whose VB was not banked are recovered by inverting
//! the peel camera `GbxV_WorldPrCamera` of that draw, an affine orthographic map).

use crate::geometry::{DecorTri, V3};
use std::path::Path;

/// A world-space triangle list with a name, for the peel scene.
pub struct EnvMesh {
    pub name: String,
    pub tris: Vec<[V3; 3]>,
    /// The triangles' vertex normals (parallel to `tris`; empty when the source has none — the OBJ export, the captured VB path here).
    pub norms: Vec<[V3; 3]>,
}

fn read_maybe_gz(p: &Path) -> Result<Vec<u8>, String> {
    if p.exists() {
        return std::fs::read(p).map_err(|e| format!("{}: {e}", p.display()));
    }
    let gz = p.with_extension(format!("{}.gz", p.extension().and_then(|e| e.to_str()).unwrap_or("")));
    if gz.exists() {
        let d = std::fs::read(&gz).map_err(|e| format!("{}: {e}", gz.display()))?;
        return crate::passdiff::gunzip(&d);
    }
    Err(format!("{}: not found (nor .gz)", p.display()))
}

fn u16s(b: &[u8]) -> Vec<u16> {
    b.chunks_exact(2).map(|c| u16::from_le_bytes([c[0], c[1]])).collect()
}

fn f32_at(b: &[u8], off: usize) -> f32 {
    f32::from_le_bytes([b[off], b[off + 1], b[off + 2], b[off + 3]])
}

/// Invert the row-vector affine map clip = [p, 1]·M (M 4×4, the last column (0,0,0,1)) for p.
fn unproject_affine(m: &[[f32; 4]; 4], clip: [f32; 3]) -> V3 {
    // p·A + t = c  →  p = (c − t)·A⁻¹, A = the 3×3 of rows 0..2, t = row 3
    let a = [[m[0][0] as f64, m[0][1] as f64, m[0][2] as f64], [m[1][0] as f64, m[1][1] as f64, m[1][2] as f64], [m[2][0] as f64, m[2][1] as f64, m[2][2] as f64]];
    let c = [clip[0] as f64 - m[3][0] as f64, clip[1] as f64 - m[3][1] as f64, clip[2] as f64 - m[3][2] as f64];
    // solve p·A = c  ⇔  Aᵀ·pᵀ = cᵀ
    let at = [[a[0][0], a[1][0], a[2][0]], [a[0][1], a[1][1], a[2][1]], [a[0][2], a[1][2], a[2][2]]];
    let det = at[0][0] * (at[1][1] * at[2][2] - at[1][2] * at[2][1]) - at[0][1] * (at[1][0] * at[2][2] - at[1][2] * at[2][0]) + at[0][2] * (at[1][0] * at[2][1] - at[1][1] * at[2][0]);
    let inv = |i: usize, j: usize| -> f64 {
        // cofactor-based inverse element (j, i)
        let (r0, r1) = ((i + 1) % 3, (i + 2) % 3);
        let (c0, c1) = ((j + 1) % 3, (j + 2) % 3);
        (at[r0][c0] * at[r1][c1] - at[r0][c1] * at[r1][c0]) / det
    };
    let mut p = [0f64; 3];
    for i in 0..3 {
        p[i] = c[0] * inv(0, i) + c[1] * inv(1, i) + c[2] * inv(2, i);
    }
    [p[0] as f32, p[1] as f32, p[2] as f32]
}

/// `load_env` through a binary cache in the temp dir (the capture sits on a network file system and its
/// draw log is a 15 MB JSON: the first load takes seconds, the cached one milliseconds).
pub fn load_env(passcap: &Path) -> Result<Vec<EnvMesh>, String> {
    let key = { let mut h: u64 = 0xcbf2_9ce4_8422_2325; for b in passcap.to_string_lossy().bytes() { h ^= b as u64; h = h.wrapping_mul(0x0100_0000_01b3); } h };
    let cache = std::env::temp_dir().join(format!("lmtool-env-{key:016x}.bin"));
    if let Ok(bytes) = std::fs::read(&cache) {
        if let Some(m) = decode_cache(&bytes) {
            return Ok(m);
        }
    }
    let meshes = load_env_uncached(passcap)?;
    let _ = std::fs::write(&cache, encode_cache(&meshes));
    Ok(meshes)
}

fn encode_cache(meshes: &[EnvMesh]) -> Vec<u8> {
    let mut out = Vec::new();
    out.extend_from_slice(b"LMENV1\0\0");
    out.extend_from_slice(&(meshes.len() as u32).to_le_bytes());
    for m in meshes {
        let name = m.name.as_bytes();
        out.extend_from_slice(&(name.len() as u32).to_le_bytes());
        out.extend_from_slice(name);
        out.extend_from_slice(&(m.tris.len() as u32).to_le_bytes());
        for t in &m.tris {
            for p in t {
                for c in p {
                    out.extend_from_slice(&c.to_le_bytes());
                }
            }
        }
    }
    out
}

fn decode_cache(b: &[u8]) -> Option<Vec<EnvMesh>> {
    if b.len() < 12 || &b[..8] != b"LMENV1\0\0" {
        return None;
    }
    let mut o = 8usize;
    let rd_u32 = |o: &mut usize| -> Option<u32> { let v = u32::from_le_bytes(b.get(*o..*o + 4)?.try_into().ok()?); *o += 4; Some(v) };
    let n = rd_u32(&mut o)? as usize;
    let mut out = Vec::with_capacity(n);
    for _ in 0..n {
        let nl = rd_u32(&mut o)? as usize;
        let name = String::from_utf8(b.get(o..o + nl)?.to_vec()).ok()?;
        o += nl;
        let nt = rd_u32(&mut o)? as usize;
        let mut tris = Vec::with_capacity(nt);
        for _ in 0..nt {
            let mut t = [[0f32; 3]; 3];
            for p in t.iter_mut() {
                for c in p.iter_mut() {
                    *c = f32::from_le_bytes(b.get(o..o + 4)?.try_into().ok()?);
                    o += 4;
                }
            }
            tris.push(t);
        }
        out.push(EnvMesh { name, tris, norms: Vec::new() });
    }
    Some(out)
}

/// Load the environment meshes of the capture under `passcap` (the pwc-day layout).
pub fn load_env_uncached(passcap: &Path) -> Result<Vec<EnvMesh>, String> {
    let frame = 127448u32;
    let mesh_json: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(passcap.join(format!("logs/mesh-frame{frame}.json"))).map_err(|e| format!("mesh json: {e}"))?).map_err(|e| format!("mesh json: {e}"))?;
    // the peel camera of the environment draws (any of them shares it): GbxV_WorldPrCamera from draws.json
    let draws_gz = std::fs::read(passcap.join(format!("logs/draws-frame{frame}.json.gz"))).map_err(|e| format!("draws: {e}"))?;
    let draws: serde_json::Value = serde_json::from_slice(&crate::passdiff::gunzip(&draws_gz)?).map_err(|e| format!("draws json: {e}"))?;
    let mut cam: Option<[[f32; 4]; 4]> = None;
    if let Some(arr) = draws.as_array() {
        for a in arr {
            if a.get("eid").and_then(|v| v.as_u64()) == Some(1028) {
                if let Some(m) = a.pointer("/Vertex/cbuffers/SceneV/GbxV_WorldPrCamera").and_then(|v| v.as_array()) {
                    let mut out = [[0f32; 4]; 4];
                    for (i, row) in m.iter().enumerate().take(4) {
                        for (j, x) in row.as_array().into_iter().flatten().enumerate().take(4) {
                            out[i][j] = x.as_f64().unwrap_or(0.0) as f32;
                        }
                    }
                    cam = Some(out);
                }
            }
        }
    }
    let mut out = Vec::new();
    // (eid, name): the sea box and the four terrain patches
    for (eid, name) in [(1009u32, "sea_box"), (1028, "terrain_0"), (1033, "terrain_1"), (1071, "terrain_2"), (1076, "terrain_3")] {
        // indices: the vsout index buffer (u16), from either dump location
        let idx_bytes = read_maybe_gz(&passcap.join(format!("env/frame{frame}/mesh/e{eid:06}_vsout_indices.bin"))).or_else(|_| read_maybe_gz(&passcap.join(format!("mesh/frame{frame}/e{eid:06}_vsout_indices.bin"))))?;
        let indices = u16s(&idx_bytes);
        // positions: the VB (world space, VisualToWorld = identity) when banked, else the post-VS clip positions
        // inverted through the peel camera
        let mut positions: Vec<V3> = Vec::new();
        let entry = mesh_json.as_array().and_then(|a| a.iter().find(|e| e.get("eid").and_then(|v| v.as_u64()) == Some(eid as u64)));
        let mut from_vb = false;
        if let Some(e) = entry {
            if let Some(vb) = e.get("vertex_buffers").and_then(|v| v.as_array()).and_then(|v| v.first()) {
                let file = vb.get("file").and_then(|v| v.as_str()).unwrap_or("");
                let stride = vb.get("stride").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
                let pos_off = e.get("input_layout").and_then(|v| v.as_array()).and_then(|l| l.iter().find(|x| x.get("semantic").and_then(|s| s.as_str()) == Some("POSITION"))).and_then(|x| x.get("offset")).and_then(|v| v.as_u64()).unwrap_or(0) as usize;
                if let Ok(bytes) = read_maybe_gz(&passcap.join(format!("mesh/frame{frame}/{file}"))) {
                    if stride >= 12 {
                        let n = bytes.len() / stride;
                        for i in 0..n {
                            let o = i * stride + pos_off;
                            positions.push([f32_at(&bytes, o), f32_at(&bytes, o + 4), f32_at(&bytes, o + 8)]);
                        }
                        from_vb = true;
                    }
                }
            }
        }
        if !from_vb {
            let Some(m) = &cam else { return Err("no GbxV_WorldPrCamera at eid 1028 in draws.json".into()) };
            let vs = read_maybe_gz(&passcap.join(format!("env/frame{frame}/mesh/e{eid:06}_vsout.bin")))?;
            // the post-VS stride: from mesh json (vsout.vertexByteStride) else 96 (the terrain's), 16 (the box's)
            let stride = entry.and_then(|e| e.pointer("/vsout/vertexByteStride")).and_then(|v| v.as_u64()).map(|v| v as usize).unwrap_or(if eid == 1009 { 16 } else { 96 });
            let n = vs.len() / stride;
            for i in 0..n {
                let o = i * stride;
                let clip = [f32_at(&vs, o), f32_at(&vs, o + 4), f32_at(&vs, o + 8)];
                positions.push(unproject_affine(m, clip));
            }
        }
        let mut tris = Vec::new();
        for t in indices.chunks_exact(3) {
            let (a, b, c) = (t[0] as usize, t[1] as usize, t[2] as usize);
            if a < positions.len() && b < positions.len() && c < positions.len() {
                tris.push([positions[a], positions[b], positions[c]]);
            }
        }
        let (mut lo, mut hi) = ([f32::MAX; 3], [f32::MIN; 3]);
        for p in &positions { for k in 0..3 { lo[k] = lo[k].min(p[k]); hi[k] = hi[k].max(p[k]); } }
        eprintln!("env: {name} (eid {eid}): {} vertices ({}), {} triangles, bbox ({:.1}, {:.1}, {:.1})..({:.1}, {:.1}, {:.1})", positions.len(), if from_vb { "VB, world space" } else { "post-VS, un-projected" }, tris.len(), lo[0], lo[1], lo[2], hi[0], hi[1], hi[2]);
        out.push(EnvMesh { name: name.into(), tris, norms: Vec::new() });
    }
    Ok(out)
}

/// The environment as peel occluders: black (the first sweep's ILightInput), not water.
pub fn env_decor(meshes: &[EnvMesh]) -> Vec<DecorTri> {
    let mut out = Vec::new();
    for m in meshes {
        let is_box = m.name == "sea_box";
        // the box's centre, to orient every face's winding outward
        let centre = if is_box {
            let (mut lo, mut hi) = ([f32::MAX; 3], [f32::MIN; 3]);
            for t in &m.tris { for p in t { for k in 0..3 { lo[k] = lo[k].min(p[k]); hi[k] = hi[k].max(p[k]); } } }
            [(lo[0] + hi[0]) * 0.5, (lo[1] + hi[1]) * 0.5, (lo[2] + hi[2]) * 0.5]
        } else { [0.0; 3] };
        let is_water = m.name == "water_surface";
        for t in &m.tris {
            let mut tri = *t;
            if is_water {
                // the water plane's winding normal UP: its back face (seen from below) is the drawn one
                let e1 = [tri[1][0] - tri[0][0], tri[1][1] - tri[0][1], tri[1][2] - tri[0][2]];
                let e2 = [tri[2][0] - tri[0][0], tri[2][1] - tri[0][1], tri[2][2] - tri[0][2]];
                let ny = e1[2] * e2[0] - e1[0] * e2[2];
                if ny < 0.0 { tri.swap(1, 2); }
            }
            if is_box {
                let e1 = [tri[1][0] - tri[0][0], tri[1][1] - tri[0][1], tri[1][2] - tri[0][2]];
                let e2 = [tri[2][0] - tri[0][0], tri[2][1] - tri[0][1], tri[2][2] - tri[0][2]];
                let n = [e1[1] * e2[2] - e1[2] * e2[1], e1[2] * e2[0] - e1[0] * e2[2], e1[0] * e2[1] - e1[1] * e2[0]];
                let c = [(tri[0][0] + tri[1][0] + tri[2][0]) / 3.0 - centre[0], (tri[0][1] + tri[1][1] + tri[2][1]) / 3.0 - centre[1], (tri[0][2] + tri[1][2] + tri[2][2]) / 3.0 - centre[2]];
                if n[0] * c[0] + n[1] * c[1] + n[2] * c[2] < 0.0 { tri.swap(1, 2); }
            }
            out.push(DecorTri { p: tri, albedo: [0.0; 3], water: false, env: true, env_far_only: is_box || is_water, sun_caster: is_box || !(m.name.to_ascii_lowercase().contains("warp") || m.name.to_ascii_lowercase().contains("water")), warp: 0 });
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unproject_inverts_a_row_vector_affine_map() {
        let m = [[2.0, 0.0, 0.0, 0.0], [0.0, 3.0, 0.0, 0.0], [0.0, 0.0, 0.5, 0.0], [1.0, -2.0, 4.0, 1.0]];
        let p = [3.0f32, -1.0, 7.0];
        let clip = [p[0] * m[0][0] + m[3][0], p[1] * m[1][1] + m[3][1], p[2] * m[2][2] + m[3][2]];
        let q = unproject_affine(&m, clip);
        for k in 0..3 {
            assert!((q[k] - p[k]).abs() < 1e-4, "{q:?} vs {p:?}");
        }
    }
}

/// THE GAME'S ENVIRONMENT BLOCK FROM THE DECORATION'S SCENE3D (no capture): the Scene3d export's mobils by material —
/// `InvisibleShadowCaster` = the sea box (the far faces only, wound outward), `WarpSand` = the four terrain patches, the
/// `Water` surface is NOT part of the block (pwc-day: 262 + 1 120 = the captured 1 382 triangles; the 336 Water triangles
/// dropped). The same `DecorTri` flags `env_decor` gives the captured block, so the peels' environment layer and the
/// shadow casters see the game's geometry with or without `--env-from`.
pub fn env_block_from_scene3d(path: &str, scale: f32, offset: [f32; 3]) -> Result<(Vec<DecorTri>, usize), String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("{path}: {e}"))?;
    let mut verts: Vec<[f32; 3]> = Vec::new();
    let mut meshes: Vec<EnvMesh> = Vec::new();
    let mut cur: Option<EnvMesh> = None;
    let mut dropped = 0usize;
    let mut skip = false;
    for line in text.lines() {
        let mut it = line.split_whitespace();
        match it.next() {
            Some("v") => {
                let c: Vec<f32> = it.take(3).map(|x| x.parse().unwrap_or(0.0)).collect();
                if c.len() == 3 { verts.push([c[0] * scale + offset[0], c[1] * scale + offset[1], c[2] * scale + offset[2]]); }
            }
            Some("usemtl") => {
                let name = it.next().unwrap_or("").to_string();
                if let Some(m) = cur.take() { if !m.tris.is_empty() { meshes.push(m); } }
                skip = name.to_ascii_lowercase().contains("water");
                cur = Some(EnvMesh { name: if name.to_ascii_lowercase().contains("invisible") { "sea_box".into() } else { name }, tris: Vec::new(), norms: Vec::new() });
            }
            Some("f") => {
                let idx: Vec<usize> = it.map(|x| x.split('/').next().unwrap_or("0").parse::<i64>().unwrap_or(0)).map(|i| if i < 0 { (verts.len() as i64 + i) as usize } else { (i - 1).max(0) as usize }).collect();
                for k in 1..idx.len().saturating_sub(1) {
                    let (a, b, c) = (idx[0], idx[k], idx[k + 1]);
                    if a < verts.len() && b < verts.len() && c < verts.len() {
                        if skip { dropped += 1; continue; }
                        if let Some(m) = cur.as_mut() { m.tris.push([verts[a], verts[b], verts[c]]); }
                    }
                }
            }
            _ => {}
        }
    }
    if let Some(m) = cur.take() { if !m.tris.is_empty() { meshes.push(m); } }
    Ok((env_decor(&meshes), dropped))
}

/// The same environment block straight from the packs (no OBJ export): the decoration Scene3d through mapgeom's model walk,
/// its groups by material — `InvisibleShadowCaster` = the sea box, `WarpSand` = the terrain patches; `Water` and the sky
/// dome (`Tech3 Sky`, rasterised by domemesh.rs) are not part of the block.
pub fn env_block_from_pak(store: &mut mapgeom::store::DataStore, scene3d_path: &str) -> Result<(Vec<DecorTri>, usize), String> {
    env_meshes_from_pak(store, scene3d_path).map(|(m, d)| (env_decor(&m), d))
}

/// The environment block's meshes (with the vertex normals) from the packs — `env_block_from_pak` before the DecorTri flattening,
/// for the Warp terrain shading (warpterrain.rs) that needs the normals and the material names.
pub fn env_meshes_from_pak(store: &mut mapgeom::store::DataStore, scene3d_path: &str) -> Result<(Vec<EnvMesh>, usize), String> {
    let model = store.load_model(scene3d_path)?;
    let mut c = mapgeom::geom::Collector::new(store);
    c.model(&model, &mapgeom::geom::IDENTITY, 0);
    let mut meshes: Vec<EnvMesh> = Vec::new();
    let mut dropped = 0usize;
    // THE WATER SURFACE IN THE PEEL (RE 15, NOTES 07:57Z, from PS 6178 = the water material's lit shader with NoFrontBounce = 1: `discard if
    // isfrontface`, back faces o0 = (0, 0, 0, 1)): seen from ABOVE the water plane is not in the peel (the floor behind it shows); seen from
    // BELOW it is a BLACK back face — the sea-floor tiles' upward directions end at it (no sky), the platforms' downward ones pass through.
    // = the sea box's far-face rule with the plane's normal UP (`env_far_only`). MEASURED ON THE CORPUS (08:20Z, 5ff2be202c + G2's
    // 0001–0004): as a back-face occluder the decoration water moves np-tk3 BlueBay Day q3 tiles 1.000 → 0.996 (identity 63.3 → 56.1 % —
    // RE 15: BlueBay's Square64Water leaves are never drawn), tiny03 WhiteShore tiles 0.988/0.973/0.979 → 0.985/0.971/0.977, tiny04ac
    // GreenCoast 0.981 → 0.980, g23 tiles 1.015 → 1.011 with items 0.680 → 0.677; two-sided black is worse still. So the DEFAULT stays
    // "dropped" (the pre-07:57Z block); LMTOOL_ENV_WATER=backface = RE 15's rule as a study, =black the two-sided study. Stadium's pool
    // water (f1617: 4 906 black back-face texels) belongs to the WaterBase BLOCKS' prefabs, not to this decoration block — a record-scene
    // material rule (Water → peel-only back-face black), open.
    let env_water = std::env::var("LMTOOL_ENV_WATER").unwrap_or_default();
    for (name, g) in &c.scene.groups {
        let lower = name.to_ascii_lowercase();
        if lower.contains("sky") || (lower.contains("water") && env_water != "backface" && env_water != "black") {
            dropped += g.tris.len();
            continue;
        }
        let tris: Vec<[[f32; 3]; 3]> = g.tris.iter().map(|t| [g.verts[t[0] as usize], g.verts[t[1] as usize], g.verts[t[2] as usize]]).collect();
        let norms: Vec<[[f32; 3]; 3]> = if g.norms.len() == g.verts.len() { g.tris.iter().map(|t| [g.norms[t[0] as usize], g.norms[t[1] as usize], g.norms[t[2] as usize]]).collect() } else { Vec::new() };
        // LMTOOL_ENV_WARP_MAXDIST=metres (STUDY, E5 2026-09-28 22:40Z — default off): a Warp-terrain triangle whose three vertices all lie
        // farther than this Chebyshev distance from the decoration footprint's centre (1024, ·, 1024) is left out of the env layer. The
        // WarpGround mesh runs to ±97 km in rings (e5_envextent); under g23's hills (3–5 km out) its underside owns layer 0 for every light
        // direction from above and blacks 40–47 % of a vertical face's hemisphere (the 22:27Z traces). The game's hills read a neutral term
        // there, so its env raster lacks the skirt at those pixels by a selection we do not apply (RE 16 reads RenderLightIndirectDome's
        // draw list). This knob bounds the term's magnitude; the rule is the read, never the fit.
        let (tris, norms) = match std::env::var("LMTOOL_ENV_WARP_MAXDIST").ok().and_then(|v| v.parse::<f32>().ok()) {
            Some(maxd) if lower.contains("warp") => {
                let keep: Vec<bool> = tris.iter().map(|t| t.iter().any(|v| (v[0] - 1024.0).abs().max((v[2] - 1024.0).abs()) <= maxd)).collect();
                let n_keep = keep.iter().filter(|&&k| k).count();
                eprintln!("decoration: STUDY LMTOOL_ENV_WARP_MAXDIST={maxd}: {} of {} {name} triangles kept (a vertex within {maxd} m of the footprint centre)", n_keep, tris.len());
                let t2: Vec<[[f32; 3]; 3]> = tris.iter().zip(keep.iter()).filter(|(_, &k)| k).map(|(t, _)| *t).collect();
                let n2: Vec<[[f32; 3]; 3]> = if norms.is_empty() { Vec::new() } else { norms.iter().zip(keep.iter()).filter(|(_, &k)| k).map(|(t, _)| *t).collect() };
                (t2, n2)
            }
            _ => (tris, norms),
        };
        meshes.push(EnvMesh { name: if lower.contains("invisible") { "sea_box".into() } else if lower.contains("water") && env_water != "black" { "water_surface".into() } else { name.clone() }, tris, norms });
    }
    Ok((meshes, dropped))
}

/// `env_decor` with the WARP TERRAIN SHADED (warpterrain.rs): every triangle of a mesh whose material name contains "warp" and whose
/// normals are known gets its three VS 16748 outputs appended to `warp_vs` and `DecorTri::warp` = 1 + that index; the sea box and
/// the other leaves stay black. Returns the tris and the number shaded.
pub fn env_decor_warp(meshes: &[EnvMesh], consts: &crate::warpterrain::WarpConsts, warp_vs: &mut Vec<[crate::warpterrain::VsOut; 3]>) -> (Vec<DecorTri>, usize) {
    let mut out = env_decor(meshes);
    let mut shaded = 0usize;
    let mut k = 0usize;
    for m in meshes {
        let is_warp = m.name.to_ascii_lowercase().contains("warp") && m.norms.len() == m.tris.len();
        for (ti, t) in m.tris.iter().enumerate() {
            if is_warp {
                let n = m.norms[ti];
                warp_vs.push([crate::warpterrain::vs_16748(consts, t[0], n[0]), crate::warpterrain::vs_16748(consts, t[1], n[1]), crate::warpterrain::vs_16748(consts, t[2], n[2])]);
                out[k].warp = warp_vs.len() as u32;
                shaded += 1;
            }
            k += 1;
        }
    }
    (out, shaded)
}

/// THE GRID FIT OF THE ENVIRONMENT (the Fall 2026 giants, 2026-10-01): the collection's environment block and sky dome are
/// authored for the 64-cell decoration; a bigger grid's map lies outside them and the lightmapper — the game's and ours —
/// darkens everything beyond ~1024 m of the decoration centre. `p' = c2 + k·(p − c)` with `c` = the decoration centre
/// (1024, 0, 1024), `c2` = the grid's centre (16·size, 0, 16·size) and `k` = size / 64: a 64-cell map is the identity.
#[derive(Clone, Copy, Debug)]
pub struct EnvFit {
    pub k: f32,
    pub c: V3,
    pub c2: V3,
}

impl EnvFit {
    /// None for a 64-cell grid (the identity) or an unreadable size.
    pub fn for_map(mf: &tmmaps::map::MapFile) -> Option<EnvFit> {
        let s = mf.size[0].max(mf.size[2]);
        if s <= 0 || s == 64 { return None; }
        let k = s as f32 / 64.0;
        Some(EnvFit { k, c: [1024.0, 0.0, 1024.0], c2: [16.0 * s as f32, 0.0, 16.0 * s as f32] })
    }
    pub fn map(&self, p: V3) -> V3 {
        [self.c2[0] + self.k * (p[0] - self.c[0]), self.c2[1] + self.k * (p[1] - self.c[1]), self.c2[2] + self.k * (p[2] - self.c[2])]
    }
}
