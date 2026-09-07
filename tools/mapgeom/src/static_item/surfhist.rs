//! `mapgeom surfhist <pack prefab | pack .StaticObject.Gbx | local .Item.Gbx>...
//! [--depth N] [--up 0.7]`: the collision census of a model, SOURCE BY SOURCE.
//!
//! A pack prefab is walked like the bake walks it (every entity, sub-prefabs
//! included, in the root prefab's frame); a local item is read as written. For
//! every collision surface met: the material nodes and the u16 id table it
//! carries, then per (physics byte, gameplay byte, surface index) the triangle
//! count, how many faces point up, the height range and the area — what the
//! car FEELS per source, so a road prefab's deck and its borders can be told
//! apart before a bake merges them into one hull (the Rubber-vs-Asphalt
//! question of 2026-09-07).

use std::collections::BTreeMap;

use super::surface::{CPlugSurface, SurfMaterial};
use super::Node;
use crate::geom::{apply, compose, Xform, IDENTITY};
use crate::store::DataStore;

fn flag(args: &[String], name: &str) -> Option<String> {
    args.iter().position(|a| a == name).and_then(|i| args.get(i + 1).cloned())
}

#[derive(Default)]
struct Row {
    count: usize,
    up: usize,
    down: usize,
    ymin: f32,
    ymax: f32,
    area: f64,
    up_area: f64,
}

/// One surface's census, printed under `label`; `mat_name` names an external
/// material node of the surface. Returns per-physics (count, up count).
pub fn census(label: &str, sf: &CPlugSurface, at: &Xform, up_cos: f32, mat_name: &dyn Fn(i32) -> Option<String>) -> BTreeMap<u8, (usize, usize)> {
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
    let mut totals: BTreeMap<u8, (usize, usize)> = BTreeMap::new();
    let Some((verts, tris)) = sf.surf.triangulate() else {
        println!("{label}: surf type {} (not meshable) materials [{}] ids {:?}", sf.surf.type_id(), mats.join(", "), sf.material_ids);
        return totals;
    };
    println!("{label}: surf type {} v{}/{} {} vertices {} triangles materials [{}] ids {:?}", sf.surf.type_id(), sf.version, sf.surf_version, verts.len(), tris.len(), mats.join(", "), sf.material_ids);
    let verts: Vec<[f32; 3]> = verts.iter().map(|v| apply(at, *v)).collect();
    let mut rows: BTreeMap<(u8, u8, i16), Row> = BTreeMap::new();
    for t in &tris {
        let Some([a, b, c]) = t.indices.iter().map(|i| verts.get(*i as usize).copied()).collect::<Option<Vec<_>>>().map(|v| [v[0], v[1], v[2]]) else { continue };
        let e1 = [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
        let e2 = [c[0] - a[0], c[1] - a[1], c[2] - a[2]];
        let n = [e1[1] * e2[2] - e1[2] * e2[1], e1[2] * e2[0] - e1[0] * e2[2], e1[0] * e2[1] - e1[1] * e2[0]];
        let len = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt();
        let ny = if len > 0.0 { n[1] / len } else { 0.0 };
        let area = (len / 2.0) as f64;
        let row = rows.entry((t.material_id, t.u03, t.surface_index)).or_insert_with(|| Row { ymin: f32::MAX, ymax: f32::MIN, ..Default::default() });
        row.count += 1;
        row.area += area;
        if ny > up_cos {
            row.up += 1;
            row.up_area += area;
        }
        if ny < -up_cos {
            row.down += 1;
        }
        for p in [a, b, c] {
            row.ymin = row.ymin.min(p[1]);
            row.ymax = row.ymax.max(p[1]);
        }
        let tot = totals.entry(t.material_id).or_insert((0, 0));
        tot.0 += 1;
        if ny > up_cos {
            tot.1 += 1;
        }
    }
    for ((phys, gp, si), r) in &rows {
        let table = sf.material_ids.get((*si).max(0) as usize).copied();
        let note = match table {
            Some(id) if (id & 0xFF) as u8 != *phys || (id >> 8) as u8 != *gp => "  ** BYTE/TABLE DISAGREE **",
            None if !sf.material_ids.is_empty() => "  ** INDEX PAST TABLE **",
            _ => "",
        };
        let mat = match sf.materials.get((*si).max(0) as usize) {
            Some(SurfMaterial::Node(r)) if r.inline.is_none() => mat_name(r.index).map(|p| p.rsplit('\\').next().unwrap_or(&p).to_string()).unwrap_or_default(),
            _ => String::new(),
        };
        println!(
            "  byte {phys:>3} {:<14} gameplay {gp} idx {si} -> table {} {}: {:>6} tris ({:>5} up, {:>5} down), y {:.2}..{:.2}, area {:.1} m2 (up {:.1}){note}",
            crate::scene::physics_name(*phys),
            table.map(|t| t.to_string()).unwrap_or_else(|| "-".into()),
            mat,
            r.count,
            r.up,
            r.down,
            r.ymin,
            r.ymax,
            r.area,
            r.up_area
        );
    }
    totals
}

fn add_totals(into: &mut BTreeMap<u8, (usize, usize)>, from: BTreeMap<u8, (usize, usize)>) {
    for (k, (c, u)) in from {
        let e = into.entry(k).or_insert((0, 0));
        e.0 += c;
        e.1 += u;
    }
}

/// A pack static object file: its surface (inline, or the external shape
/// file), placed by `at`.
fn static_object_file(store: &mut DataStore, path: &str, at: &Xform, up_cos: f32, label: &str, totals: &mut BTreeMap<u8, (usize, usize)>) -> Result<(), String> {
    let model = store.load_model(path)?;
    if model.class_id != super::C_STATIC_OBJECT_MODEL {
        return Err(format!("{path}: class 0x{:08X} is not CPlugStaticObjectModel", model.class_id));
    }
    let mut lb = super::LookbackState::default();
    lb.defined_nodes.extend(model.external_indices().iter().copied());
    let mut r = super::Rd::new(&model.body, 0, lb);
    let so = super::item::CPlugStaticObjectModel::parse(&mut r).map_err(|e| format!("{path}: {e}"))?;
    let ext = model.externals.clone();
    let name_in = |tbl: &[(u32, String)], i: i32| tbl.iter().find(|(k, _)| *k as i32 == i).map(|(_, p)| p.clone());
    if let Some(sf) = so.surface() {
        add_totals(totals, census(label, sf, at, up_cos, &|i| name_in(&ext, i)));
    } else if !so.is_mesh_collidable && so.shape.index >= 0 {
        let sp = name_in(&ext, so.shape.index).ok_or_else(|| format!("{path}: shape node {} is neither inline nor external", so.shape.index))?;
        let sm = store.load_model(&sp)?;
        let mut lb = super::LookbackState::default();
        lb.defined_nodes.extend(sm.external_indices().iter().copied());
        let mut r = super::Rd::new(&sm.body, 0, lb);
        let sf = CPlugSurface::parse(&mut r).map_err(|e| format!("{sp}: {e}"))?;
        let sext = sm.externals.clone();
        add_totals(totals, census(&format!("{label} shape {sp}"), &sf, at, up_cos, &|i| name_in(&sext, i)));
    } else if so.is_mesh_collidable {
        println!("{label}: mesh-collidable static object (the visuals are the hull; not counted here)");
    } else {
        println!("{label}: static object without collision");
    }
    Ok(())
}

/// A pack prefab: every entity, sub-prefabs recursively, in `at`'s frame.
fn prefab(store: &mut DataStore, path: &str, at: &Xform, depth: usize, max_depth: usize, up_cos: f32, totals: &mut BTreeMap<u8, (usize, usize)>) -> Result<(), String> {
    if depth > max_depth {
        println!("{path}: deeper than --depth {max_depth}, not walked");
        return Ok(());
    }
    let model = store.load_model(path)?;
    let pf = super::prefab::CPlugPrefab::from_model(&model)?;
    let ext = model.externals.clone();
    let ext_name = |i: i32| ext.iter().find(|(k, _)| *k as i32 == i).map(|(_, p)| p.clone());
    let indent = "  ".repeat(depth);
    println!("{indent}{path}: {} entities", pf.ents.len());
    for (i, e) in pf.ents.iter().enumerate() {
        let iso = compose(at, &super::prefab::CPlugPrefab::entity_iso(e));
        let label = format!("{indent}#{i} at [{:.2}, {:.2}, {:.2}]", iso[9], iso[10], iso[11]);
        match e.model.inline.as_deref() {
            Some(Node::StaticObject(so)) => match so.surface() {
                Some(sf) => add_totals(totals, census(&format!("{label} inline static object"), sf, &iso, up_cos, &ext_name)),
                None if so.is_mesh_collidable => println!("{label} inline static object: mesh-collidable (visuals are the hull; not counted)"),
                None => println!("{label} inline static object: no collision"),
            },
            Some(Node::Dyna(d)) => {
                for (what, r) in [("static shape", &d.static_shape), ("dyna shape", &d.dyna_shape)] {
                    if let Some(Node::Surface(sf)) = r.inline.as_deref() {
                        add_totals(totals, census(&format!("{label} inline dyna {what}"), sf, &iso, up_cos, &ext_name));
                    }
                }
            }
            Some(other) => println!("{label} inline class 0x{:08X}: skipped", other.class_id()),
            None => match ext_name(e.model.index) {
                Some(p) if p.to_ascii_lowercase().ends_with(".prefab.gbx") => {
                    println!("{label} sub-prefab:");
                    prefab(store, &p, &iso, depth + 1, max_depth, up_cos, totals)?;
                }
                Some(p) if p.to_ascii_lowercase().ends_with(".staticobject.gbx") => {
                    if let Err(err) = static_object_file(store, &p, &iso, up_cos, &format!("{label} {p}"), totals) {
                        println!("{label} {p}: {err}");
                    }
                }
                Some(p) => println!("{label} external {p}: not walked"),
                None => println!("{label} external node {}: unnamed", e.model.index),
            },
        }
    }
    Ok(())
}

pub fn run(rest: &[String], open: &mut dyn FnMut() -> DataStore) -> Result<(), String> {
    let max_depth: usize = flag(rest, "--depth").unwrap_or_else(|| "8".into()).parse().map_err(|e| format!("--depth: {e}"))?;
    let up_cos: f32 = flag(rest, "--up").unwrap_or_else(|| "0.7".into()).parse().map_err(|e| format!("--up: {e}"))?;
    let mut paths: Vec<String> = Vec::new();
    let mut skip = false;
    for a in rest.iter().skip(1) {
        if skip {
            skip = false;
            continue;
        }
        if a == "--depth" || a == "--up" {
            skip = true;
            continue;
        }
        paths.push(a.clone());
    }
    if paths.is_empty() {
        return Err("surfhist <pack prefab | pack .StaticObject.Gbx | local .Item.Gbx>... [--depth N] [--up 0.7]".into());
    }
    let mut store: Option<DataStore> = None;
    for path in &paths {
        let mut totals: BTreeMap<u8, (usize, usize)> = BTreeMap::new();
        if std::path::Path::new(path).is_file() {
            let data = std::fs::read(path).map_err(|e| format!("{path}: {e}"))?;
            let f = super::parse_file(&data).map_err(|e| format!("{path}: {e}"))?;
            let none = |_: i32| -> Option<String> { None };
            if let Some(so) = f.item.static_object() {
                match so.surface() {
                    Some(sf) => add_totals(&mut totals, census(&format!("{path} static object"), sf, &IDENTITY, up_cos, &none)),
                    None => println!("{path}: static object without collision"),
                }
            } else if let Some(p) = f.item.prefab() {
                for (i, e) in p.ents.iter().enumerate() {
                    let iso = super::prefab::CPlugPrefab::entity_iso(e);
                    match e.model.inline.as_deref() {
                        Some(Node::StaticObject(so)) => {
                            if let Some(sf) = so.surface() {
                                add_totals(&mut totals, census(&format!("{path} entity {i} static object"), sf, &iso, up_cos, &none));
                            }
                        }
                        Some(Node::Dyna(d)) => {
                            for (what, r) in [("static shape", &d.static_shape), ("dyna shape", &d.dyna_shape)] {
                                if let Some(Node::Surface(sf)) = r.inline.as_deref() {
                                    add_totals(&mut totals, census(&format!("{path} entity {i} dyna {what}"), sf, &iso, up_cos, &none));
                                }
                            }
                        }
                        _ => {}
                    }
                }
            } else {
                return Err(format!("{path}: neither a static object nor a prefab entity model"));
            }
        } else {
            let st = store.get_or_insert_with(|| open());
            if path.to_ascii_lowercase().ends_with(".staticobject.gbx") {
                static_object_file(st, path, &IDENTITY, up_cos, path, &mut totals)?;
            } else {
                prefab(st, path, &IDENTITY, 0, max_depth, up_cos, &mut totals)?;
            }
        }
        let total: usize = totals.values().map(|(c, _)| *c).sum();
        println!(
            "{path}: TOTAL {total} triangles: {}",
            totals.iter().map(|(p, (c, u))| format!("{} {p}: {c} ({u} up)", crate::scene::physics_name(*p))).collect::<Vec<_>>().join(", ")
        );
    }
    Ok(())
}
