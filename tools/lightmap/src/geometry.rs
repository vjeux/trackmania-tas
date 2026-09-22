//! Geometry for the baker: every item of a tiny map as world-space triangles
//! carrying the model's lightmap UVs (TexCoord1), grouped by item so each item
//! gets its own chart.

use std::collections::BTreeMap;

use mapgeom::static_item::vstream::{Elem, N_NORMAL, N_POSITION, N_TEXCOORD0};
use mapgeom::static_item::Node;

pub type V3 = [f32; 3];

#[derive(Clone, Copy, Debug)]
pub struct Tri {
    pub p: [V3; 3],
    /// Per-vertex normals (face normal when the stream has none usable).
    pub n: [V3; 3],
    /// TexCoord1 of each vertex.
    pub uv: [[f32; 2]; 3],
}

/// A light socket of a model (`CPlugSolid2Model.lights`): position and axis in
/// model space, the `GxLight` parameters. Spot angles in degrees (a pack spot
/// is 120–170°, nearly a hemisphere), `radius` = the ball radius (falloff range).
#[derive(Clone, Copy, Debug)]
pub struct LightDef {
    pub pos: V3,
    /// The socket's forward axis (the spot direction).
    pub dir: V3,
    pub color: [f32; 3],
    pub intensity: f32,
    pub radius: f32,
    /// (inner, outer) cone angles in degrees; (180, 180) for a ball light.
    pub cone: (f32, f32),
    pub animated: bool,
}

#[derive(Clone, Debug, Default)]
pub struct ModelGeom {
    pub tris: Vec<Tri>,
    pub lights: Vec<LightDef>,
    /// PreLightGen: u02 (the metres-per-uv the game sizes the chart with) and the uv1 bounds u04[0..4].
    pub plg_u02: f32,
    pub plg_bounds: Option<[f32; 4]>,
    /// √(world area / uv area): metres per uv unit — the chart's world size.
    pub metres_per_uv: f32,
    pub uv_min: [f32; 2],
    pub uv_max: [f32; 2],
}

pub fn sub(a: V3, b: V3) -> V3 {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}
pub fn add(a: V3, b: V3) -> V3 {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}
pub fn mul(a: V3, s: f32) -> V3 {
    [a[0] * s, a[1] * s, a[2] * s]
}
pub fn dot(a: V3, b: V3) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}
pub fn cross(a: V3, b: V3) -> V3 {
    [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]]
}
pub fn norm(a: V3) -> V3 {
    let l = dot(a, a).sqrt();
    if l > 1e-12 {
        mul(a, 1.0 / l)
    } else {
        [0.0, 1.0, 0.0]
    }
}

/// Decode a Dec3N packed normal (10:10:10 signed).
fn dec3n(w: u32) -> V3 {
    let f = |s: u32| {
        let v = ((w >> s) & 0x3ff) as i32;
        let v = if v >= 512 { v - 1024 } else { v };
        v as f32 / 511.0
    };
    norm([f(0), f(10), f(20)])
}

/// The lightmappable triangles of one item file (static objects; prefabs
/// contribute nothing here — moving parts have no stable chart).
pub fn load_model(bytes: &[u8]) -> Result<ModelGeom, String> {
    let f = mapgeom::static_item::file::parse_file(bytes)?;
    let mut g = ModelGeom::default();
    let Some(so) = f.item.static_object() else {
        return Ok(g);
    };
    let Some(s2) = so.solid2() else {
        return Ok(g);
    };
    if let Some(plg) = &s2.pre_light_gen {
        g.plg_u02 = plg.u02;
        if plg.u04[2] > plg.u04[0] && plg.u04[3] > plg.u04[1] && plg.u04[2].is_finite() {
            g.plg_bounds = Some([plg.u04[0], plg.u04[1], plg.u04[2], plg.u04[3]]);
        }
    }
    for l in &s2.lights {
        let Some(Node::Light(pl)) = l.node.inline.as_deref() else { continue };
        let Some(gx) = pl.gx_light() else { continue };
        let (color, intensity, radius) = gx.summary();
        let mut cone = (180.0f32, 180.0f32);
        for ch in &gx.chunks {
            match ch {
                mapgeom::static_item::light::GxChunk::Spot { angle_inner, angle_outer, .. } => cone = (*angle_inner, *angle_outer),
                mapgeom::static_item::light::GxChunk::Spot01 { angle_inner, angle_outer, .. } => cone = (*angle_inner, *angle_outer),
                _ => {}
            }
        }
        let t = &l.u05;
        g.lights.push(LightDef { pos: [t[9], t[10], t[11]], dir: norm([t[6], t[7], t[8]]), color, intensity, radius, cone, animated: pl.is_animated() });
    }
    let (mut aw, mut au) = (0f64, 0f64);
    let (mut umin, mut umax) = ([f32::MAX; 2], [f32::MIN; 2]);
    for sg in &s2.shaded_geoms {
        // lod 0 only: the lightmap is computed on the highest detail
        if sg.lod_mask > 0 && sg.lod_mask & 1 == 0 {
            continue;
        }
        let Some(vr) = s2.visuals.get(sg.visual_index as usize) else { continue };
        let Some(Node::Visual(v)) = vr.inline.as_deref() else { continue };
        let Some(ib) = v.index_buffer.as_ref() else { continue };
        let Some(st) = v.stream() else { continue };
        let get = |name: u32| st.decls.iter().zip(st.elems.iter()).find(|(d, _)| d.name() == name).map(|(_, e)| e);
        let Some(Elem::Float3(pos)) = get(N_POSITION) else { continue };
        let uv1: Option<&Vec<[f32; 2]>> = match get(N_TEXCOORD0 + 1) {
            Some(Elem::Float2(u)) => Some(u),
            _ => None,
        };
        // TexCoord1 may also live in the visual's own tex_coord_sets (set 1)
        let uv1_alt: Option<Vec<[f32; 2]>> = if uv1.is_none() {
            v.main.as_ref().and_then(|m| m.tex_coord_sets.get(1)).map(|s| s.coords.iter().map(|c| c.0).collect())
        } else {
            None
        };
        let uv1: Option<&Vec<[f32; 2]>> = uv1.or(uv1_alt.as_ref());
        let Some(uv1) = uv1 else { continue };
        let normals: Option<Vec<V3>> = match get(N_NORMAL) {
            Some(Elem::Float3(n)) => Some(n.clone()),
            Some(Elem::Word(w)) => Some(w.iter().map(|&x| dec3n(x)).collect()),
            _ => None,
        };
        for t in ib.indices.chunks_exact(3) {
            let (a, b, c) = (t[0] as usize, t[1] as usize, t[2] as usize);
            if a >= pos.len() || b >= pos.len() || c >= pos.len() || a >= uv1.len() || b >= uv1.len() || c >= uv1.len() {
                continue;
            }
            let p = [pos[a], pos[b], pos[c]];
            let fnrm = norm(cross(sub(p[1], p[0]), sub(p[2], p[0])));
            let n = match &normals {
                Some(nv) if a < nv.len() && b < nv.len() && c < nv.len() => [nv[a], nv[b], nv[c]],
                _ => [fnrm, fnrm, fnrm],
            };
            let uv = [uv1[a], uv1[b], uv1[c]];
            for u in &uv {
                umin[0] = umin[0].min(u[0]);
                umin[1] = umin[1].min(u[1]);
                umax[0] = umax[0].max(u[0]);
                umax[1] = umax[1].max(u[1]);
            }
            au += (((uv[1][0] - uv[0][0]) as f64) * ((uv[2][1] - uv[0][1]) as f64) - ((uv[2][0] - uv[0][0]) as f64) * ((uv[1][1] - uv[0][1]) as f64)).abs() / 2.0;
            let cr = cross(sub(p[1], p[0]), sub(p[2], p[0]));
            aw += (dot(cr, cr) as f64).sqrt() / 2.0;
            g.tris.push(Tri { p, n, uv });
        }
    }
    g.metres_per_uv = if au > 1e-9 && aw > 1e-9 { (aw / au).sqrt() as f32 } else { 0.0 };
    g.uv_min = umin;
    g.uv_max = umax;
    Ok(g)
}

#[derive(Clone, Debug)]
pub struct Instance {
    pub item: usize,
    pub model: usize,
    pub xf: mapgeom::geom::Xform,
    pub model_name: String,
}

pub struct Scene {
    pub models: Vec<ModelGeom>,
    pub model_names: Vec<String>,
    pub instances: Vec<Instance>,
    pub item_count: usize,
}

impl Scene {
    pub fn from_map(path: &str) -> Result<Scene, String> {
        let m = tmmaps::map::MapFile::load(std::path::Path::new(path));
        let files = mapgeom::embedded::files(&m)?;
        // model name -> bytes (the zip keys carry a folder prefix)
        let mut by_name: BTreeMap<String, &Vec<u8>> = BTreeMap::new();
        for (k, v) in &files {
            let base = k.rsplit(['/', '\\']).next().unwrap_or(k).to_string();
            by_name.insert(base, v);
        }
        let mut models = Vec::new();
        let mut model_names = Vec::new();
        let mut index: BTreeMap<String, usize> = BTreeMap::new();
        let mut instances = Vec::new();
        let mut missing: BTreeMap<String, usize> = BTreeMap::new();
        for (i, it) in m.items.iter().enumerate() {
            let mi = match index.get(&it.model) {
                Some(&k) => k,
                None => match by_name.get(&it.model) {
                    Some(bytes) => {
                        let g = load_model(bytes).map_err(|e| format!("{}: {e}", it.model))?;
                        models.push(g);
                        model_names.push(it.model.clone());
                        index.insert(it.model.clone(), models.len() - 1);
                        models.len() - 1
                    }
                    None => {
                        *missing.entry(it.model.clone()).or_insert(0) += 1;
                        continue;
                    }
                },
            };
            let xf = mapgeom::place::anchored(it.pos, [it.yaw, it.pitch, it.roll], it.pivot, it.scale);
            instances.push(Instance { item: i, model: mi, xf, model_name: it.model.clone() });
        }
        if !missing.is_empty() {
            eprintln!("  {} item models are not embedded (stock items?): {:?}", missing.len(), missing.iter().take(8).collect::<Vec<_>>());
        }
        Ok(Scene { models, model_names, instances, item_count: m.items.len() })
    }

    pub fn tri_count(&self) -> usize {
        self.instances.iter().map(|i| self.models[i.model].tris.len()).sum()
    }

    /// Every light of every instance in world space (position, direction, and
    /// the radius scaled like the instance).
    pub fn world_lights(&self) -> Vec<(usize, LightDef)> {
        let mut out = Vec::new();
        for (ii, inst) in self.instances.iter().enumerate() {
            let m = &self.models[inst.model];
            let scale = {
                let c0 = [inst.xf[0], inst.xf[1], inst.xf[2]];
                dot(c0, c0).sqrt()
            };
            for l in &m.lights {
                let mut w = *l;
                w.pos = xf_point(&inst.xf, l.pos);
                w.dir = xf_normal(&inst.xf, l.dir);
                w.radius = l.radius * scale;
                out.push((ii, w));
            }
        }
        out
    }
}

/// Transform a point and a normal (rotation part only; assumes uniform scale).
pub fn xf_point(m: &mapgeom::geom::Xform, v: V3) -> V3 {
    mapgeom::geom::apply(m, v)
}
pub fn xf_normal(m: &mapgeom::geom::Xform, n: V3) -> V3 {
    norm([
        m[0] * n[0] + m[3] * n[1] + m[6] * n[2],
        m[1] * n[0] + m[4] * n[1] + m[7] * n[2],
        m[2] * n[0] + m[5] * n[1] + m[8] * n[2],
    ])
}
