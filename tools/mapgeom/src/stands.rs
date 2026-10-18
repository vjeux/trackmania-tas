//! Hugo's "Manslaughter" stands (2026-10-18): the SEATING of a Stadium spectator-stand BLOCK as a
//! finish trigger, carried by an invisible item placed in the block's own frame.
//!
//! The five stand blocks (StandStraight, StandCornerOut, StandCornerIn, StandDiagIn, StandDiagOut;
//! their OnDecoWall twins reuse the same prefabs) are prefabs whose collision names the step rows
//! with their own material: `TechnicsStep.Material.Gbx` — the treads AND the risers, i.e. the
//! staircase people sit on. The black side walls and frame are `TechnicsTrims`/`Technics`, the
//! roof `CanopyGlass`/`PlatformTech`/`TrackWall`, the top deck `Show4x1`: none of those is in the
//! trigger. The trigger = that staircase surface padded `pad_below` down (so a car whose wheels
//! touch any step crosses it) and `thick` up (so a car landing on the steps from above crosses the
//! top), closed along the surface's own outline — a slab in the exact shape of the seating, for
//! any plan shape (straight, quarter circle, triangle).
//!
//! Frame: the prefab's = the block's local frame (dir 0: x 0..32 east, z 0..32 north, y 0 at the
//! block's cell floor). A placement of the carrier at `place::grid_block(cell, dir, (32, 32), yoff)`
//! — position = the cell's origin corner turned by `dir` (dir 0 (0,0), dir 1 (+32,0), dir 2
//! (+32,+32), dir 3 (0,+32) in (x,z)), yaw = −dir·π/2 — puts the slab on the stand.
use std::collections::{BTreeMap, HashMap};

use crate::geom::{apply, compose, Xform, IDENTITY};
use crate::static_item::surface::{CPlugSurface, SurfMaterial};
use crate::static_item::Node;
use crate::store::DataStore;

/// The seating material, as the stand prefabs name it.
pub const SEAT_MATERIAL: &str = "TechnicsStep.Material.Gbx";

/// The five stand block kinds → the base prefab that carries the seating + the crowd.
pub const STANDS: &[(&str, &str)] = &[
    ("StandStraight", "Stadium\\Media\\Prefab\\Stand\\Straight.Prefab.Gbx"),
    ("StandCornerOut", "Stadium\\Media\\Prefab\\Stand\\CornerOut.Prefab.Gbx"),
    ("StandCornerIn", "Stadium\\Media\\Prefab\\Stand\\CornerIn.Prefab.Gbx"),
    ("StandDiagIn", "Stadium\\Media\\Prefab\\Stand\\DiagIn.Prefab.Gbx"),
    ("StandDiagOut", "Stadium\\Media\\Prefab\\Stand\\DiagOut.Prefab.Gbx"),
];

/// The block kind of a stand block name (the OnDecoWall twins map to their base kind).
pub fn stand_kind(block_name: &str) -> Option<&'static str> {
    let base = block_name.trim_end_matches("OnDecoWall");
    STANDS.iter().find(|(k, _)| *k == base).map(|(k, _)| *k)
}

pub fn prefab_of(kind: &str) -> Option<&'static str> {
    STANDS.iter().find(|(k, _)| *k == kind).map(|(_, p)| *p)
}

/// One collision triangle of a prefab, in the prefab frame, with its material name.
#[derive(Clone, Debug)]
pub struct MatTri {
    pub v: [[f32; 3]; 3],
    pub material: String,
    pub physics: u8,
}

/// Every collision triangle of a pack prefab (sub-prefabs walked, static objects and dyna
/// static shapes), with the material each triangle indexes.
pub fn prefab_triangles(store: &mut DataStore, path: &str, at: &Xform, depth: usize, out: &mut Vec<MatTri>) -> Result<(), String> {
    if depth > 6 {
        return Ok(());
    }
    let model = store.load_model(path)?;
    let pf = crate::static_item::prefab::CPlugPrefab::from_model(&model)?;
    let ext = model.externals.clone();
    let ext_name = |i: i32| ext.iter().find(|(k, _)| *k as i32 == i).map(|(_, p)| p.clone());
    for e in &pf.ents {
        let iso = compose(at, &crate::static_item::prefab::CPlugPrefab::entity_iso(e));
        match e.model.inline.as_deref() {
            Some(Node::StaticObject(so)) => {
                if let Some(sf) = so.surface() {
                    surface_triangles(sf, &iso, &ext_name, out);
                }
            }
            Some(Node::Dyna(d)) => {
                if let Some(Node::Surface(sf)) = d.static_shape.inline.as_deref() {
                    surface_triangles(sf, &iso, &ext_name, out);
                }
            }
            None => {
                if let Some(p) = ext_name(e.model.index) {
                    if p.to_ascii_lowercase().ends_with(".prefab.gbx") {
                        prefab_triangles(store, &p, &iso, depth + 1, out)?;
                    } else if p.to_ascii_lowercase().ends_with(".staticobject.gbx") {
                        let sm = store.load_model(&p)?;
                        let mut lb = crate::static_item::LookbackState::default();
                        lb.defined_nodes.extend(sm.external_indices().iter().copied());
                        let mut r = crate::static_item::Rd::new(&sm.body, 0, lb);
                        if let Ok(so) = crate::static_item::item::CPlugStaticObjectModel::parse(&mut r) {
                            if let Some(sf) = so.surface() {
                                let sext = sm.externals.clone();
                                surface_triangles(sf, &iso, &|i| sext.iter().find(|(k, _)| *k as i32 == i).map(|(_, p)| p.clone()), out);
                            }
                        }
                    }
                }
            }
            _ => {}
        }
    }
    Ok(())
}

fn surface_triangles(sf: &CPlugSurface, at: &Xform, mat_name: &dyn Fn(i32) -> Option<String>, out: &mut Vec<MatTri>) {
    let mats: Vec<String> = sf
        .materials
        .iter()
        .map(|m| match m {
            SurfMaterial::Node(r) => match &r.inline {
                Some(n) => format!("inline 0x{:08X}", n.class_id()),
                None => mat_name(r.index).map(|p| p.rsplit('\\').next().unwrap_or(&p).to_string()).unwrap_or_else(|| format!("node {}", r.index)),
            },
            SurfMaterial::Id(i) => format!("id {i}"),
        })
        .collect();
    let Some((verts, tris)) = sf.surf.triangulate() else { return };
    let verts: Vec<[f32; 3]> = verts.iter().map(|v| apply(at, *v)).collect();
    for t in &tris {
        let Some(v) = t.indices.iter().map(|i| verts.get(*i as usize).copied()).collect::<Option<Vec<_>>>() else { continue };
        let material = mats.get(t.surface_index.max(0) as usize).cloned().unwrap_or_else(|| format!("idx {}", t.surface_index));
        out.push(MatTri { v: [v[0], v[1], v[2]], material, physics: t.material_id });
    }
}

/// Per material: triangle count, up-facing count, y range, area.
pub fn material_census(tris: &[MatTri]) -> BTreeMap<String, (usize, usize, f32, f32, f64)> {
    let mut m: BTreeMap<String, (usize, usize, f32, f32, f64)> = BTreeMap::new();
    for t in tris {
        let e = m.entry(t.material.clone()).or_insert((0, 0, f32::INFINITY, f32::NEG_INFINITY, 0.0));
        e.0 += 1;
        let (n, area) = normal_area(&t.v);
        if n[1] > 0.7 {
            e.1 += 1;
        }
        for p in &t.v {
            e.2 = e.2.min(p[1]);
            e.3 = e.3.max(p[1]);
        }
        e.4 += area as f64;
    }
    m
}

fn normal_area(v: &[[f32; 3]; 3]) -> ([f32; 3], f32) {
    let (a, b, c) = (v[0], v[1], v[2]);
    let e1 = [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
    let e2 = [c[0] - a[0], c[1] - a[1], c[2] - a[2]];
    let n = [e1[1] * e2[2] - e1[2] * e2[1], e1[2] * e2[0] - e1[0] * e2[2], e1[0] * e2[1] - e1[1] * e2[0]];
    let len = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt();
    if len > 0.0 { ([n[0] / len, n[1] / len, n[2] / len], len / 2.0) } else { ([0.0; 3], 0.0) }
}

/// The seating surface of a stand prefab: the `SEAT_MATERIAL` triangles welded into one indexed
/// mesh (treads + risers), in the prefab frame.
pub struct Seating {
    pub verts: Vec<[f32; 3]>,
    pub tris: Vec<[u32; 3]>,
    pub census: BTreeMap<String, (usize, usize, f32, f32, f64)>,
    pub y_lo: f32,
    pub y_hi: f32,
    pub rows: usize,
}

pub fn seating_of(store: &mut DataStore, prefab: &str) -> Result<Seating, String> {
    let mut all = Vec::new();
    prefab_triangles(store, prefab, &IDENTITY, 0, &mut all)?;
    let census = material_census(&all);
    let seat: Vec<&MatTri> = all.iter().filter(|t| t.material.eq_ignore_ascii_case(SEAT_MATERIAL)).collect();
    if seat.is_empty() {
        return Err(format!("{prefab}: no {SEAT_MATERIAL} triangle (materials: {:?})", census.keys().collect::<Vec<_>>()));
    }
    let (mut verts, mut tris, mut index): (Vec<[f32; 3]>, Vec<[u32; 3]>, HashMap<[i64; 3], u32>) = (Vec::new(), Vec::new(), HashMap::new());
    let key = |p: [f32; 3]| [(p[0] * 1000.0).round() as i64, (p[1] * 1000.0).round() as i64, (p[2] * 1000.0).round() as i64];
    for t in &seat {
        let mut idx = [0u32; 3];
        for k in 0..3 {
            let kk = key(t.v[k]);
            idx[k] = *index.entry(kk).or_insert_with(|| {
                verts.push(t.v[k]);
                (verts.len() - 1) as u32
            });
        }
        if idx[0] != idx[1] && idx[1] != idx[2] && idx[0] != idx[2] {
            tris.push(idx);
        }
    }
    let (y_lo, y_hi) = verts.iter().fold((f32::INFINITY, f32::NEG_INFINITY), |(lo, hi), v| (lo.min(v[1]), hi.max(v[1])));
    let mut out = Seating { verts, tris, census, y_lo, y_hi, rows: 0 };
    out.rows = tread_heights(&out).len();
    Ok(out)
}

/// The slab: the seating surface shifted `pad_below` down (bottom skin, normals down) and
/// `thick - pad_below` up (top skin, normals up), closed by walls along the surface's boundary
/// edges (an edge used by exactly one triangle). Returns (vertices, triangles).
pub fn slab_of(s: &Seating, pad_below: f32, thick: f32) -> (Vec<[f32; 3]>, Vec<[u32; 3]>) {
    let n = s.verts.len() as u32;
    let mut verts: Vec<[f32; 3]> = Vec::with_capacity(2 * s.verts.len());
    verts.extend(s.verts.iter().map(|v| [v[0], v[1] - pad_below, v[2]]));
    verts.extend(s.verts.iter().map(|v| [v[0], v[1] - pad_below + thick, v[2]]));
    let mut tris: Vec<[u32; 3]> = Vec::with_capacity(4 * s.tris.len());
    // orient every seating triangle with its normal up (the risers are vertical: kept as they are)
    for t in &s.tris {
        let v = [s.verts[t[0] as usize], s.verts[t[1] as usize], s.verts[t[2] as usize]];
        let (nrm, _) = normal_area(&v);
        let up = if nrm[1] >= 0.0 { *t } else { [t[0], t[2], t[1]] };
        // bottom skin: facing down
        tris.push([up[0], up[2], up[1]]);
        // top skin: facing up
        tris.push([up[0] + n, up[1] + n, up[2] + n]);
    }
    // boundary edges (used by exactly one triangle) → walls, wound outward from the up-oriented
    // triangles (bottom edge a→b, then up)
    let ups: Vec<[u32; 3]> = s
        .tris
        .iter()
        .map(|t| {
            let v = [s.verts[t[0] as usize], s.verts[t[1] as usize], s.verts[t[2] as usize]];
            if normal_area(&v).0[1] >= 0.0 { *t } else { [t[0], t[2], t[1]] }
        })
        .collect();
    let mut uses: HashMap<(u32, u32), usize> = HashMap::new();
    for t in &ups {
        for k in 0..3 {
            let (a, b) = (t[k], t[(k + 1) % 3]);
            *uses.entry((a.min(b), a.max(b))).or_insert(0) += 1;
        }
    }
    for t in &ups {
        for k in 0..3 {
            let (a, b) = (t[k], t[(k + 1) % 3]);
            if uses.get(&(a.min(b), a.max(b))).copied().unwrap_or(0) == 1 {
                tris.push([a, b, b + n]);
                tris.push([a, b + n, a + n]);
            }
        }
    }
    (verts, tris)
}

/// The distinct tread heights (up-facing triangle centroids, 5 cm bins), ascending.
pub fn tread_heights(s: &Seating) -> Vec<f32> {
    let mut h: Vec<i64> = s
        .tris
        .iter()
        .filter_map(|t| {
            let v = [s.verts[t[0] as usize], s.verts[t[1] as usize], s.verts[t[2] as usize]];
            (normal_area(&v).0[1].abs() > 0.7).then(|| ((v[0][1] + v[1][1] + v[2][1]) / 3.0 * 20.0).round() as i64)
        })
        .collect();
    h.sort();
    h.dedup();
    h.into_iter().map(|x| x as f32 / 20.0).collect()
}

/// The carrier item bytes for one stand kind: ident `<Kind>FinishTrigger.Item.Gbx` (full-ident
/// form: author = ident), collection as given, Finish waypoint, the slab trigger.
pub struct Carrier {
    pub kind: String,
    pub ident: String,
    pub bytes: Vec<u8>,
    pub seating: Seating,
    pub slab_tris: usize,
    pub bbox: [f32; 6],
    pub notes: Vec<String>,
}

pub fn carrier_item(store: &mut DataStore, kind: &str, collection: u32, pad_below: f32, thick: f32, lm_word: Option<u64>) -> Result<Carrier, String> {
    let prefab = prefab_of(kind).ok_or_else(|| format!("{kind}: not a stand kind"))?;
    let seating = seating_of(store, prefab)?;
    let (verts, tris) = slab_of(&seating, pad_below, thick);
    let (mut lo, mut hi) = ([f32::INFINITY; 3], [f32::NEG_INFINITY; 3]);
    for v in &verts {
        for k in 0..3 {
            lo[k] = lo[k].min(v[k]);
            hi[k] = hi[k].max(v[k]);
        }
    }
    // the carrier's visual donor: the stand prefab itself (one visual kept, shrunk to a speck)
    let mut mg = crate::static_item::build::Merged::default();
    crate::static_item::build::add_prefab(store, prefab, &IDENTITY, 1.0, &mut mg, 0)?;
    let slab_tris = tris.len();
    crate::static_item::build::make_invisible_mesh_waypoint(&mut mg, crate::sttc::WP_FINISH, verts, tris)?;
    if let Some(w) = lm_word {
        mg.file_write_time = w;
        mg.notes.push(format!("solid FileWriteTime stamped with the map's lightmap cache word {w}"));
    }
    let ident = format!("{kind}FinishTrigger.Item.Gbx");
    let (bytes, mg) = crate::static_item::build::finish_item(mg, &ident, &ident, 1.0, collection)?;
    Ok(Carrier { kind: kind.to_string(), ident, bytes, seating, slab_tris, bbox: [lo[0], lo[1], lo[2], hi[0], hi[1], hi[2]], notes: mg.notes.clone() })
}
