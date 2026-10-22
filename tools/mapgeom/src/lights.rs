//! Hugo's "Moth" campaign (2026-10-22): every LIGHT SOURCE a block or item carries — (a) real light
//! nodes (the prefab's `.Light.Gbx` sockets: GxLight colour / intensity / range at a pose), (b)
//! self-illuminated materials (a material whose `.Material.Gbx` names an `_I` (illumination) texture —
//! LightSpot, RaceScreen*, LightSpot2…; the `_L` map is noted too) — with the LOCAL bbox of the
//! emitting geometry per material (the lamp head, not the pole), read off the same bake the tiny
//! campaign and the vista items use (`Merged` at scale 1, the block frame at dir 0 / the item frame).
use std::collections::{BTreeMap, HashMap};

use crate::static_item::build::Merged;
use crate::store::DataStore;

#[derive(Clone, Debug)]
pub struct EmissiveRow {
    pub material: String,
    pub parts: usize,
    pub tris: usize,
    pub bbox: [f32; 6],
    pub textures: String,
}

#[derive(Clone, Debug)]
pub struct LightRow {
    pub source: String,
    pub pos: [f32; 3],
    pub color: [f32; 3],
    pub intensity: f32,
    pub range: f32,
}

#[derive(Clone, Debug, Default)]
pub struct Census {
    pub name: String,
    pub kind: &'static str,
    pub collection: String,
    pub lights: Vec<LightRow>,
    pub emissive: Vec<EmissiveRow>,
    pub size_cells: Option<[i32; 3]>,
    pub error: Option<String>,
    pub notes: usize,
}

/// `(illumination texture, light texture)` of a material link, cached.
pub struct MatCache {
    pub map: HashMap<String, Option<(Vec<String>, Vec<String>)>>,
}

impl MatCache {
    pub fn new() -> Self {
        MatCache { map: HashMap::new() }
    }
    /// The `_I` / `_L` textures the material names; None when the material file is not in the packs.
    pub fn emissive(&mut self, store: &mut DataStore, link: &str) -> Option<(Vec<String>, Vec<String>)> {
        if let Some(v) = self.map.get(link) {
            return v.clone();
        }
        let path = format!("{link}.Material.Gbx");
        let v = store.load_model(&path).ok().map(|m| {
            let mut i = Vec::new();
            let mut l = Vec::new();
            for (_, p) in &m.externals {
                let stem = p.rsplit('\\').next().unwrap_or(p).to_string();
                if let Some(s) = stem.strip_suffix(".Texture.gbx") {
                    if s.ends_with("_I") {
                        i.push(s.to_string());
                    } else if s.ends_with("_L") {
                        l.push(s.to_string());
                    }
                }
            }
            (i, l)
        });
        self.map.insert(link.to_string(), v.clone());
        v
    }
}

/// Name patterns that mark a material as a light even without an `_I` map (screens whose glow is a
/// FuncShader, LED strips…).
pub fn name_says_light(link: &str) -> bool {
    let s = link.rsplit('\\').next().unwrap_or(link).to_ascii_lowercase();
    ["light", "emissive", "screen", "neon", "led", "lamp", "glow", "cells", "speedometer", "racead"].iter().any(|k| s.contains(k))
}

/// The census of one baked model.
pub fn census_of(store: &mut DataStore, cache: &mut MatCache, m: &Merged) -> (Vec<LightRow>, Vec<EmissiveRow>) {
    let mut lights = Vec::new();
    for l in &m.lights_out {
        let (color, intensity, range) = l.light.gx_light().map(|g| g.summary()).unwrap_or_default();
        lights.push(LightRow { source: l.source.rsplit('\\').next().unwrap_or(&l.source).to_string(), pos: [l.socket.u05[9], l.socket.u05[10], l.socket.u05[11]], color, intensity, range });
    }
    // per material slot: is it emissive?
    let mut per: BTreeMap<usize, (String, String, usize, usize, [f32; 3], [f32; 3])> = BTreeMap::new();
    for mv in &m.visuals {
        let Some(mat) = m.materials.get(mv.material) else { continue };
        let link = mat.link().unwrap_or("").to_string();
        let tex = cache.emissive(store, &link);
        let (ill, lmap) = tex.clone().unwrap_or_default();
        let by_name = name_says_light(&link);
        if ill.is_empty() && !by_name {
            continue;
        }
        let Some(main) = mv.visual.main.as_ref() else { continue };
        let b = main.bounding_box;
        let (lo, hi) = ([b[0] - b[3], b[1] - b[4], b[2] - b[5]], [b[0] + b[3], b[1] + b[4], b[2] + b[5]]);
        let tris = mv.visual.index_buffer.as_ref().map(|ib| ib.indices.len() / 3).unwrap_or(0);
        let e = per.entry(mv.material).or_insert_with(|| {
            let mut t = ill.clone();
            t.extend(lmap.iter().map(|s| format!("{s} (L)")));
            if ill.is_empty() {
                t.push("(by name)".into());
            }
            (link.clone(), t.join(" "), 0, 0, [f32::INFINITY; 3], [f32::NEG_INFINITY; 3])
        });
        e.2 += 1;
        e.3 += tris;
        for k in 0..3 {
            e.4[k] = e.4[k].min(lo[k]);
            e.5[k] = e.5[k].max(hi[k]);
        }
    }
    let emissive = per
        .into_values()
        .map(|(link, textures, parts, tris, lo, hi)| EmissiveRow { material: link, parts, tris, bbox: [lo[0], lo[1], lo[2], hi[0], hi[1], hi[2]], textures })
        .collect();
    (lights, emissive)
}

pub fn fmt_bbox(b: &[f32; 6]) -> String {
    format!("{:.2}..{:.2} {:.2}..{:.2} {:.2}..{:.2}", b[0], b[3], b[1], b[4], b[2], b[5])
}
