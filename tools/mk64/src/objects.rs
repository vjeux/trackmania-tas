//! The course OBJECTS the game places from code rather than from spawn
//! tables (vjeux, 2026-09-24: "What else do we have that you didn't
//! implement? … Please do all of them as suggested"): Mario/Wario signs, the
//! Yoshi egg, the railroad crossings, the falling rocks, the Chain Chomps,
//! the Thwomps' crush, the hot-air balloon, the neon signs, the parked train,
//! the Turnpike traffic, the paddle boat, and the sprite creatures.
//!
//! Two ways an object gets into a map:
//!
//! * STATIC — textured, vertex-tinted, appended to the course mesh like the
//!   trees (`actors::add_billboards`): a course display-list MODEL placed with
//!   a yaw, or a SPRITE as a crossed billboard.
//! * MOVING — a kinematic part of one item per course (the item boxes'
//!   machinery): a moving part shows GAME materials only, so the model's
//!   textures become `CustomPlastic` slots tinted with each texture's mean
//!   colour (× the vertex tint), and a sprite becomes pixel relief (the
//!   moles). Motions are the one axis the constraint has: `Shuttle` along a
//!   direction (forward, then snap back — a chomp charging down a straight,
//!   traffic on the Turnpike), `Bob` up and down (a Thwomp's crush, a mole's
//!   pop, a Cheep-Cheep's jump), `Spin` about an axis.
//!
//! Positions come from the decomp's code (`src/racing/actors.c`,
//! `src/update_objects.c`, `src/data/some_data.c`) — see `course_objects`.

use crate::actors::{model_triangles, model_triangles_with, ModelTri};
use crate::course::{Course, Vertex};
use crate::mesh::{self, Corner, Frame, Material, Mesh, Tri};
use crate::texture::{AssetIndex, Image, Rom};
use mapgeom::crystal_model::{CPlugMaterialUserInst, Cst, Id};
use mapgeom::static_item::assemble::{assemble, BuildOpts};
use mapgeom::static_item::bake::{self, Corner as BCorner, VisualLayout};
use mapgeom::static_item::build::static_item_from_pack_item_report;
use mapgeom::static_item::dyna::AnimSubFunc;
use mapgeom::static_item::merged::{DynaPart, Merged, MergedVisual};
use mapgeom::static_item::write_file;
use mapgeom::store::DataStore;
use std::collections::{BTreeMap, HashMap};

/// How an object moves (one kinematic axis).
#[derive(Clone, Debug)]
pub enum Motion {
    Static,
    /// Forward along `dir` (unit, TM frame) by `len_m` at `speed_mps`, then
    /// back to the start: `snap` = instantly (a stream of chomps), else at the
    /// same speed (a shuttle).
    Shuttle { dir: [f32; 3], len_m: f32, speed_mps: f32, snap: bool },
    /// Up `rise_m` in `up_ms`, hold `hold_ms`, down in `down_ms`, rest `rest_ms`.
    Bob { rise_m: f32, up_ms: u32, hold_ms: u32, down_ms: u32, rest_ms: u32 },
    /// A full turn about `axis` (0 x, 1 y, 2 z) every `period_ms`.
    Spin { axis: u8, period_ms: u32 },
}

/// What an object looks like.
#[derive(Clone, Debug)]
pub enum Look {
    /// Course display lists, each with an offset (model units, before `scale`)
    /// and a uniform scale to course units.
    Model { dls: Vec<(String, [f32; 3])>, scale: f32, initial_tex: Option<(String, u32, u32, bool, bool)> },
    /// A sprite: texture symbol (+ palette), drawn `width_units` wide; `mirror`
    /// = the stored image is the left half.
    Sprite { sym: String, tlut: Option<String>, width_units: f32, mirror: bool },
}

#[derive(Clone, Debug)]
pub struct Placement {
    /// Course units (x before the mirror).
    pub pos: [f32; 3],
    /// Yaw about +y, radians, course frame (0 = the model as authored).
    pub yaw: f32,
    pub motion: Motion,
}

#[derive(Clone, Debug)]
pub struct CourseObject {
    pub name: String,
    pub look: Look,
    pub placements: Vec<Placement>,
    /// Lit models (`gsSPNumLights`) carry normals as colours: draw white.
    pub lit: bool,
}

/// The triangles of a model look, in course units about the object origin.
fn model_tris(course: &Course, look: &Look) -> Vec<ModelTri> {
    let Look::Model { dls, scale, initial_tex } = look else { return Vec::new() };
    let mut out = Vec::new();
    for (dl, off) in dls {
        let tris = match initial_tex {
            Some(t) => model_triangles_with(course, dl, Some(t.clone())),
            None => model_triangles(course, dl),
        };
        for mut t in tris {
            for v in t.c.iter_mut() {
                v.pos = [((v.pos[0] as f32 + off[0]) * scale).round() as i16, ((v.pos[1] as f32 + off[1]) * scale).round() as i16, ((v.pos[2] as f32 + off[2]) * scale).round() as i16];
            }
            out.push(t);
        }
    }
    out
}

/// STATIC objects into the course mesh. Returns (placements, triangles).
pub fn add_static(course: &Course, mesh: &mut Mesh, frame: &Frame, obj: &CourseObject, rom: &mut Rom, assets: &AssetIndex) -> (usize, usize) {
    let statics: Vec<&Placement> = obj.placements.iter().filter(|p| matches!(p.motion, Motion::Static)).collect();
    if statics.is_empty() {
        return (0, 0);
    }
    let piece = mesh.piece_names.len() as u32;
    mesh.piece_names.push(format!("objects:{}", obj.name));
    let mut mats: HashMap<(String, bool, bool), usize> = HashMap::new();
    let mut added = 0;
    match &obj.look {
        Look::Model { .. } => {
            let model = model_tris(course, &obj.look);
            if model.is_empty() {
                println!("  objects: {}: no model triangles", obj.name);
                return (0, 0);
            }
            for p in &statics {
                let (s, c) = (p.yaw.sin(), p.yaw.cos());
                let xu = if frame.mirror { -p.pos[0] } else { p.pos[0] };
                for t in &model {
                    let mat = t.tex.as_ref().map(|(sym, w, h, cs, ct)| {
                        *mats.entry((sym.clone(), *cs, *ct)).or_insert_with(|| {
                            mesh.materials.push(Material { sym: sym.clone(), mirror_s: false, mirror_t: false, clamp_s: *cs, clamp_t: *ct, w: *w, h: *h, fmt: 2, tint: [255, 255, 255] });
                            mesh.materials.len() - 1
                        })
                    });
                    let corner = |v: &Vertex| {
                        let (x, y, z) = (v.pos[0] as f32, v.pos[1] as f32, v.pos[2] as f32);
                        let (rx, rz) = (c * x + s * z, -s * x + c * z);
                        let world = [xu + rx, p.pos[1] + y, p.pos[2] + rz];
                        let (tw, th) = t.tex.as_ref().map(|x| (x.1 as f32, x.2 as f32)).unwrap_or((32.0, 32.0));
                        let mut u = v.tc[0] as f32 / 32.0 / tw;
                        let mut vv = v.tc[1] as f32 / 32.0 / th;
                        if t.tex.as_ref().map(|x| x.3).unwrap_or(false) {
                            u = u.clamp(0.0, 1.0);
                        }
                        if t.tex.as_ref().map(|x| x.4).unwrap_or(false) {
                            vv = vv.clamp(0.0, 1.0);
                        }
                        vv = 1.0 - vv;
                        Corner { pos: frame.to_tm_f(world), uv: [u, vv], rgba: [v.rgb[0], v.rgb[1], v.rgb[2], 255] }
                    };
                    let (a, b, cc) = (corner(&t.c[0]), corner(&t.c[1]), corner(&t.c[2]));
                    let c3 = if frame.mirror { [a, cc, b] } else { [a, b, cc] };
                    mesh.tris.push(Tri { c: c3, mat, two_sided: false, lit: obj.lit || t.lit, piece });
                    added += 1;
                }
            }
        }
        Look::Sprite { sym, tlut, width_units, mirror } => {
            let Some(loc) = assets.locate(sym) else {
                println!("  objects: {}: texture {sym} not in the asset index", obj.name);
                return (0, 0);
            };
            let tl = tlut.as_ref().and_then(|t| assets.locate(t));
            let img = match rom.texture(&loc, tl.as_ref()) {
                Ok(i) => i,
                Err(e) => {
                    println!("  objects: {}: {e}", obj.name);
                    return (0, 0);
                }
            };
            let full_w = if *mirror { img.w * 2 } else { img.w };
            // the material IS the texture symbol: `material_images` resolves the
            // picture by `sym` through the asset index (an invented name drew the
            // magenta placeholder — Rainbow Road's neon signs, 2026-09-25)
            let mi = *mats.entry((sym.clone(), true, true)).or_insert_with(|| {
                mesh.materials.push(Material { sym: sym.clone(), mirror_s: *mirror, mirror_t: false, clamp_s: true, clamp_t: true, w: img.w, h: img.h, fmt: 2, tint: [255, 255, 255] });
                mesh.materials.len() - 1
            });
            let hw = width_units / 2.0;
            let hgt = width_units * img.h as f32 / full_w as f32;
            for p in &statics {
                let xu = if frame.mirror { -p.pos[0] } else { p.pos[0] };
                for (ux, uz) in [(1.0f32, 0.0f32), (0.0, 1.0)] {
                    let corner = |su: f32, sv: f32| Corner { pos: frame.to_tm_f([xu + ux * su * hw, p.pos[1] + (sv + 1.0) / 2.0 * hgt, p.pos[2] + uz * su * hw]), uv: [(su + 1.0) / 2.0 * if *mirror { 2.0 } else { 1.0 }, 1.0 - (sv + 1.0) / 2.0], rgba: [255, 255, 255, 255] };
                    let (a, b, cc, d) = (corner(-1.0, -1.0), corner(1.0, -1.0), corner(1.0, 1.0), corner(-1.0, 1.0));
                    mesh.tris.push(Tri { c: [a, b, cc], mat: Some(mi), two_sided: true, lit: false, piece });
                    mesh.tris.push(Tri { c: [a, cc, d], mat: Some(mi), two_sided: true, lit: false, piece });
                    added += 2;
                }
            }
        }
    }
    (statics.len(), added)
}

/// A `CustomPlastic` game material tinted `rgb` (sRGB bytes → linear floats
/// in the material's `TargetColor` constant).
pub fn plastic(rgb: [u8; 3], physics: u8) -> CPlugMaterialUserInst {
    // MK64_OBJ_MAT: the game material the moving figures wear. CustomPlastic +
    // TargetColor drew NOTHING on a moving part (2026-09-24 22:00: mounds and
    // mole SHADOWS, no moles) — the modeler materials are resolved by the item
    // editor, not by a prefab's dyna part. The item-box line-up's opaque ones
    // (Pylon grey, TechnicsTrims dark) do draw there — 22:52 A/B: both show a
    // Monty Mole silhouette on its mound; the dark one reads as a mole.
    let link = std::env::var("MK64_OBJ_MAT").unwrap_or_else(|_| "Stadium\\Media\\Material\\TechnicsTrims".to_string());
    if !link.contains("Custom") {
        return CPlugMaterialUserInst::game_material(&link, physics);
    }
    let mut m = CPlugMaterialUserInst::game_material(&link, physics);
    if let Some(main) = m.main.as_mut() {
        main.material_name = Id::Str(format!("Plastic_{:02x}{:02x}{:02x}", rgb[0], rgb[1], rgb[2]));
        main.csts = vec![Cst { u01: Id::Str("TargetColor".into()), u02: Id::Str("Real".into()), u03: 3 }];
        let lin = |c: u8| -> f32 {
            let s = c as f32 / 255.0;
            if s <= 0.04045 { s / 12.92 } else { ((s + 0.055) / 1.055).powf(2.4) }
        };
        main.color = [lin(rgb[0]), lin(rgb[1]), lin(rgb[2])].iter().map(|c| c.to_bits() as i32).collect();
    }
    m
}

/// The mean opaque colour of a texture.
fn mean_colour(img: &Image) -> [u8; 3] {
    let (mut r, mut g, mut b, mut n) = (0u64, 0u64, 0u64, 0u64);
    for p in img.rgba.chunks(4) {
        if p[3] >= 128 {
            r += p[0] as u64;
            g += p[1] as u64;
            b += p[2] as u64;
            n += 1;
        }
    }
    if n == 0 { [128, 128, 128] } else { [(r / n) as u8, (g / n) as u8, (b / n) as u8] }
}

/// A model's triangles as plastic-coloured geometry: per (texture mean ×
/// vertex tint, quantised) one material slot. Returns (colour → triangles) in
/// metres about the object origin (course units × scale, mirrored with the
/// frame), plus the model's lowest point in metres.
fn model_relief(course: &Course, look: &Look, frame: &Frame, rom: &mut Rom, assets: &AssetIndex, lit: bool) -> (BTreeMap<[u8; 3], Vec<[BCorner; 3]>>, f32) {
    let model = model_tris(course, look);
    let mut tex_mean: HashMap<String, [u8; 3]> = HashMap::new();
    let mut out: BTreeMap<[u8; 3], Vec<[BCorner; 3]>> = BTreeMap::new();
    let mut min_y = f32::MAX;
    for t in &model {
        let base = match &t.tex {
            Some((sym, ..)) => *tex_mean.entry(sym.clone()).or_insert_with(|| {
                assets.locate(sym).and_then(|loc| rom.texture(&loc, None).ok()).map(|img| mean_colour(&img)).unwrap_or([160, 160, 160])
            }),
            None => [255, 255, 255],
        };
        // vertex tint (a lit model's "colours" are normals: white)
        let tint = if lit || t.lit { [255u8, 255, 255] } else { [((t.c[0].rgb[0] as u32 + t.c[1].rgb[0] as u32 + t.c[2].rgb[0] as u32) / 3) as u8, ((t.c[0].rgb[1] as u32 + t.c[1].rgb[1] as u32 + t.c[2].rgb[1] as u32) / 3) as u8, ((t.c[0].rgb[2] as u32 + t.c[1].rgb[2] as u32 + t.c[2].rgb[2] as u32) / 3) as u8] };
        let col = [
            ((base[0] as u32 * tint[0] as u32 / 255) as u8 / 16) * 16 + 8,
            ((base[1] as u32 * tint[1] as u32 / 255) as u8 / 16) * 16 + 8,
            ((base[2] as u32 * tint[2] as u32 / 255) as u8 / 16) * 16 + 8,
        ];
        let p = |v: &Vertex| -> [f32; 3] {
            let x = if frame.mirror { -(v.pos[0] as f32) } else { v.pos[0] as f32 };
            [x * frame.scale, v.pos[1] as f32 * frame.scale, v.pos[2] as f32 * frame.scale]
        };
        let (a, b, c) = (p(&t.c[0]), p(&t.c[1]), p(&t.c[2]));
        for q in [a, b, c] {
            min_y = min_y.min(q[1]);
        }
        let n = mesh::face_normal(&[a, b, c]);
        let corner = |q: [f32; 3]| BCorner { pos: q, normal: n, uv: [0.0, 0.0], uv1: [0.0, 0.0], tan_u: [1.0, 0.0, 0.0], tan_v: [0.0, 1.0, 0.0], face: 0, group: 0 };
        let tri = if frame.mirror { [corner(a), corner(c), corner(b)] } else { [corner(a), corner(b), corner(c)] };
        out.entry(col).or_default().push(tri);
    }
    (out, if min_y == f32::MAX { 0.0 } else { min_y })
}

/// MOVING objects of one course as one kinematic item. Returns None when the
/// course has none.
pub struct MovingItem {
    pub bytes: Vec<u8>,
    pub pos: [f32; 3],
    pub parts: usize,
}

pub fn build_moving(store: &mut DataStore, name: &str, course: &Course, objs: &[CourseObject], frame: &Frame, rom: &mut Rom, assets: &AssetIndex) -> Result<Option<MovingItem>, String> {
    let moving: Vec<(&CourseObject, &Placement)> = objs.iter().flat_map(|o| o.placements.iter().filter(|p| !matches!(p.motion, Motion::Static)).map(move |p| (o, p))).collect();
    if moving.is_empty() {
        return Ok(None);
    }
    let (_, tmpl) = static_item_from_pack_item_report(store, crate::itembox::TEMPLATE_ITEM, "Template.Item.Gbx", "Template", 1.0, crate::tm::STADIUM, 0)?;
    let part0 = tmpl.dyna.first().ok_or_else(|| format!("{}: no moving part in the template", crate::itembox::TEMPLATE_ITEM))?.clone();
    let (kc0, cparams) = part0.constraint.clone().ok_or("template part has no constraint")?;
    // reliefs per object (built once, shared by its placements)
    let mut reliefs: HashMap<String, (BTreeMap<[u8; 3], Vec<[BCorner; 3]>>, f32)> = HashMap::new();
    let origin = {
        let p0 = &moving[0].1.pos;
        let p = frame.to_tm_f([p0[0], p0[1], p0[2]]);
        [(p[0] * 100.0).round() / 100.0, (p[1] * 100.0).round() / 100.0, (p[2] * 100.0).round() / 100.0]
    };
    let mut merged = Merged::default();
    let unix = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
    merged.file_write_time = unix * 10_000_000 + 116444736000000000;
    let mut parts = 0;
    for (k, (obj, pl)) in moving.iter().enumerate() {
        let (faces, _min_y) = reliefs.entry(obj.name.clone()).or_insert_with(|| match &obj.look {
            Look::Model { .. } => model_relief(course, &obj.look, frame, rom, assets, obj.lit),
            Look::Sprite { sym, tlut, width_units, mirror } => {
                let img = assets.locate(sym).and_then(|loc| {
                    let tl = tlut.as_ref().and_then(|t| assets.locate(t));
                    rom.texture(&loc, tl.as_ref()).ok()
                });
                match img {
                    Some(half) => {
                        let full = if *mirror { crate::moles::full_sprite(&half) } else { half };
                        // a 64-px sprite as relief is ~2500 triangles; fifteen
                        // hedgehogs made a 4 MB item — halve the resolution for
                        // anything wider than 32 px (a 32-px figure reads fine
                        // at kart range), and use six colours
                        let small = if full.w > 32 { downsample2(&full) } else { full };
                        let px_m = width_units * frame.scale / small.w as f32;
                        (crate::moles::relief_generic(&small, px_m, 0.2), 0.0)
                    }
                    None => (BTreeMap::new(), 0.0),
                }
            }
        }).clone();
        if faces.is_empty() {
            println!("  objects: {}: nothing to build", obj.name);
            continue;
        }
        let world = frame.to_tm_f(pl.pos);
        let pos = [world[0] - origin[0], world[1] - origin[1], world[2] - origin[2]];
        let mut kc = kc0.clone();
        kc.trans_axis = 1;
        kc.trans_min = 0.0;
        kc.trans_max = 0.0;
        kc.trans.subs = vec![AnimSubFunc { ease: 1, reverse: 1, duration_ms: 8000 }];
        kc.rot_axis = 1;
        kc.angle_min_deg = 0.0;
        kc.angle_max_deg = 0.0;
        kc.rot.subs = vec![AnimSubFunc { ease: 1, reverse: 1, duration_ms: 8000 }];
        kc.shader_tc_type = 0;
        kc.shader_tc_anim.clear();
        kc.shader_tc_trans_sub = None;
        // the part's yaw: the model's authoring yaw plus, for a shuttle, none —
        // the translation axis is the part's LOCAL axis, so a shuttle's part is
        // turned to put local +z along `dir` and the geometry counter-turned
        let mut yaw = if frame.mirror { -pl.yaw } else { pl.yaw };
        let mut geom_yaw = 0.0f32;
        match &pl.motion {
            Motion::Shuttle { dir, len_m, speed_mps, snap } => {
                let part_yaw = dir[0].atan2(dir[2]);
                geom_yaw = yaw - part_yaw;
                yaw = part_yaw;
                kc.trans_axis = 2;
                kc.trans_min = 0.0;
                kc.trans_max = *len_m;
                let go = ((len_m / speed_mps) * 1000.0).max(1.0) as u32;
                kc.trans.subs = if *snap {
                    vec![AnimSubFunc { ease: 1, reverse: 0, duration_ms: go }, AnimSubFunc { ease: 1, reverse: 1, duration_ms: 1 }]
                } else {
                    vec![AnimSubFunc { ease: 4, reverse: 0, duration_ms: go }, AnimSubFunc { ease: 4, reverse: 1, duration_ms: go }]
                };
            }
            Motion::Bob { rise_m, up_ms, hold_ms, down_ms, rest_ms } => {
                kc.trans_axis = 1;
                kc.trans_min = 0.0;
                kc.trans_max = *rise_m;
                kc.trans.subs = vec![
                    AnimSubFunc { ease: 3, reverse: 0, duration_ms: *up_ms },
                    AnimSubFunc { ease: 0, reverse: 0, duration_ms: *hold_ms },
                    AnimSubFunc { ease: 2, reverse: 1, duration_ms: *down_ms },
                    AnimSubFunc { ease: 0, reverse: 1, duration_ms: *rest_ms },
                ];
            }
            Motion::Spin { axis, period_ms } => {
                kc.rot_axis = *axis;
                kc.angle_min_deg = 180.0;
                kc.angle_max_deg = -180.0;
                kc.rot.subs = vec![AnimSubFunc { ease: 1, reverse: 1, duration_ms: *period_ms }];
            }
            Motion::Static => {}
        }
        let mut mesh = Merged::default();
        mesh.file_write_time = merged.file_write_time;
        let mut per_material: Vec<Vec<[BCorner; 3]>> = Vec::new();
        let mut slots: Vec<usize> = Vec::new();
        let (gs, gc) = (geom_yaw.sin(), geom_yaw.cos());
        for (col, tris) in &faces {
            mesh.materials.push(plastic(*col, crate::tm::PHYS_CONCRETE));
            slots.push(mesh.materials.len() - 1);
            let turned: Vec<[BCorner; 3]> = tris
                .iter()
                .map(|t| {
                    let mut t2 = *t;
                    for c in t2.iter_mut() {
                        let (x, z) = (c.pos[0], c.pos[2]);
                        c.pos = [gc * x + gs * z, c.pos[1], -gs * x + gc * z];
                        let (nx, nz) = (c.normal[0], c.normal[2]);
                        c.normal = [gc * nx + gs * nz, c.normal[1], -gs * nx + gc * nz];
                    }
                    t2
                })
                .collect();
            per_material.push(turned);
        }
        let has_uv1 = vec![true; per_material.len()];
        bake::assign_lightmap_atlas(&mut per_material, &has_uv1);
        for (i, tris) in per_material.iter_mut().enumerate() {
            bake::tangents_vprim(tris, 0);
            for v in bake::make_visuals(tris, VisualLayout::Full, "range") {
                mesh.visuals.push(MergedVisual::every_level(v, slots[i]));
            }
        }
        merged.dyna.push(DynaPart {
            path: format!("mk64:{}:{k}", obj.name),
            rot: [0.0, (yaw / 2.0).sin(), 0.0, (yaw / 2.0).cos()],
            pos,
            mesh,
            move_shape: Some(mapgeom::static_item::surface::CPlugSurface::mesh(
                vec![[0.0, 0.0, 0.0], [0.001, 0.0, 0.0], [0.0, 0.001, 0.0]],
                vec![mapgeom::static_item::surface::Triangle { indices: [0, 1, 2], material_id: crate::tm::PHYS_CONCRETE, gameplay: 0, surface_index: 0 }],
                vec![crate::tm::PHYS_CONCRETE as u16],
                [0.0, 1.0, 0.0],
            )),
            hit_shape: None,
            model: part0.model.clone(),
            instance_params_id: part0.instance_params_id,
            instance_params: part0.instance_params.clone(),
            constraint: Some((kc, cparams.clone())),
            pack_ref: None,
        });
        parts += 1;
    }
    if parts == 0 {
        return Ok(None);
    }
    let opts = BuildOpts { ident: name.to_string(), author: name.to_string(), scale: 1.0, collection: crate::tm::STADIUM, skin: None };
    let f = assemble(&merged, &opts)?;
    Ok(Some(MovingItem { bytes: write_file(&f), pos: origin, parts }))
}

/// A straight run of the centre path through `idx`: the direction (TM frame,
/// unit, ground plane) and the length (m) over which the heading stays within
/// `max_deg` of the heading at `idx`, forward from `idx`.
pub fn straight_from(path_tm: &[[f32; 3]], idx: usize, max_deg: f32, max_len_m: f32) -> ([f32; 3], f32) {
    let n = path_tm.len();
    let at = |i: usize| path_tm[i % n];
    let d0 = {
        let (a, b) = (at(idx), at(idx + 1));
        mesh::normalize([b[0] - a[0], 0.0, b[2] - a[2]])
    };
    let mut len = 0.0f32;
    let mut i = idx;
    loop {
        let (a, b) = (at(i), at(i + 1));
        let d = mesh::normalize([b[0] - a[0], 0.0, b[2] - a[2]]);
        let cosang = d[0] * d0[0] + d[2] * d0[2];
        if cosang < max_deg.to_radians().cos() || len > max_len_m || i > idx + n {
            break;
        }
        len += ((b[0] - a[0]).powi(2) + (b[2] - a[2]).powi(2)).sqrt();
        i += 1;
    }
    (d0, len)
}

// ---------------------------------------------------------------------------
// The per-course object tables (positions from the decomp's code)
// ---------------------------------------------------------------------------

/// Every numeric literal inside the C initializer of `name` in `file`
/// (hex → i16 through u16; decimals as they are), flat.
pub fn c_numbers(decomp: &std::path::Path, file: &str, name: &str) -> Vec<f32> {
    let Ok(text) = std::fs::read_to_string(decomp.join(file)) else { return Vec::new() };
    let Some(start) = text.find(&format!("{name}[] = {{")).or_else(|| text.find(&format!("{name} = {{"))) else { return Vec::new() };
    let rest = &text[start..];
    let Some(end) = rest.find("};") else { return Vec::new() };
    let body = &rest[rest.find('{').unwrap_or(0)..end];
    let mut out = Vec::new();
    for tok in body.split(|c: char| c == ',' || c == '{' || c == '}' || c.is_whitespace()) {
        let t = tok.trim().trim_end_matches('f');
        if t.is_empty() {
            continue;
        }
        if let Some(h) = t.strip_prefix("0x") {
            if let Ok(v) = u16::from_str_radix(h, 16) {
                out.push(v as i16 as f32);
            }
        } else if let Ok(v) = t.parse::<f32>() {
            out.push(v);
        }
    }
    out
}

fn triples(v: &[f32]) -> Vec<[f32; 3]> {
    v.chunks_exact(3).map(|c| [c[0], c[1], c[2]]).collect()
}

/// The centre-path point nearest to `p` (course units, x already mirrored
/// as the path is): its index.
fn nearest_path(path_units: &[[f32; 3]], p: [f32; 3]) -> usize {
    let mut best = (f32::MAX, 0usize);
    for (i, q) in path_units.iter().enumerate() {
        let d = (q[0] - p[0]).powi(2) + (q[2] - p[2]).powi(2);
        if d < best.0 {
            best = (d, i);
        }
    }
    best.1
}

/// Heading of the path at `i` (course units frame): yaw about +y with 0 = +z.
fn path_heading(path_units: &[[f32; 3]], i: usize) -> f32 {
    let n = path_units.len();
    let (a, b) = (path_units[i % n], path_units[(i + 1) % n]);
    (b[0] - a[0]).atan2(b[2] - a[2])
}

/// The objects of `course`. `path_tm` = the centre path in TM space (for the
/// shuttle directions), `decomp` for the tables in the C sources.
pub fn course_objects(course: &Course, frame: &Frame, path_tm: &[[f32; 3]], decomp: &std::path::Path) -> Vec<CourseObject> {
    let dir = course.dir.as_str();
    let path_u: Vec<[f32; 3]> = course.path.iter().map(|p| [p.pos[0] as f32, p.pos[1] as f32, p.pos[2] as f32]).collect();
    let n = path_u.len().max(1);
    let m_per_u = frame.scale;
    let mut out: Vec<CourseObject> = Vec::new();
    // a shuttle along the path's straight from the path point nearest `pos`
    let shuttle_along_path = |pos: [f32; 3], speed_mps: f32, snap: bool, max_len_m: f32| -> Motion {
        let i = nearest_path(&path_u, pos);
        let (d, len) = straight_from(path_tm, i, 12.0, max_len_m);
        Motion::Shuttle { dir: d, len_m: len.max(4.0), speed_mps, snap }
    };
    let model = |dls: &[&str], scale: f32| Look::Model { dls: dls.iter().map(|d| (d.to_string(), [0.0, 0.0, 0.0])).collect(), scale, initial_tex: None };
    let sprite = |sym: &str, tlut: Option<&str>, width_units: f32, mirror: bool| Look::Sprite { sym: sym.to_string(), tlut: tlut.map(String::from), width_units, mirror };
    match dir {
        "mario_raceway" => {
            // two Mario signs (src/racing/actors.c), as authored (rotation 0)
            out.push(CourseObject { name: "mario_sign".into(), look: model(&["d_course_mario_raceway_dl_sign"], 1.0), lit: false, placements: vec![
                Placement { pos: [150.0, 40.0, -1300.0], yaw: 0.0, motion: Motion::Static },
                Placement { pos: [2520.0, 0.0, 1240.0], yaw: 0.0, motion: Motion::Static },
            ] });
        }
        "wario_stadium" => {
            out.push(CourseObject { name: "wario_sign".into(), look: model(&["d_course_wario_stadium_dl_sign"], 1.0), lit: false, placements: vec![Placement { pos: [-131.0, 83.0, 286.0], yaw: 0.0, motion: Motion::Static }] });
        }
        "yoshi_valley" => {
            // the Yoshi egg (actors.c) rolls the valley: a shuttle along the road there
            let pos = [-2300.0, 0.0, 634.0];
            out.push(CourseObject { name: "yoshi_egg".into(), look: model(&["d_course_yoshi_valley_dl_egg_lod0"], 1.0), lit: true, placements: vec![Placement { pos, yaw: 0.0, motion: shuttle_along_path(pos, 6.0, false, 60.0) }] });
            // 15 hedgehogs (sprites, 0.2 × 64 = 12.8 units) crossing the road at
            // 15 spots around the lap, back and forth
            // eight of the game's fifteen: each relief is its own mesh copy
            let mut pl = Vec::new();
            for k in 0..8 {
                let i = (k * n) / 8;
                let p = path_u[i];
                let h = path_heading(&path_u, i);
                // across the road: perpendicular to the heading, 6 m
                let d_tm = [h.cos() * if frame.mirror { -1.0 } else { 1.0 }, 0.0, -h.sin()];
                pl.push(Placement { pos: [p[0] - h.cos() * 20.0, p[1], p[2] + h.sin() * 20.0], yaw: h, motion: Motion::Shuttle { dir: mesh::normalize(d_tm), len_m: 6.0, speed_mps: 1.5, snap: false } });
            }
            out.push(CourseObject { name: "hedgehog".into(), look: sprite("gTextureYoshiValleyHedgehog", Some("gTLUTYoshiValleyHedgehog"), 12.8, false), lit: false, placements: pl });
        }
        "frappe_snowland" => {
            // 19 snowmen (gSnowmanSpawns): head over body, two sprites 64×64
            let nums = c_numbers(decomp, "src/data/some_data.c", "gSnowmanSpawns");
            let mut body = Vec::new();
            let mut head = Vec::new();
            for r in nums.chunks_exact(4) {
                body.push(Placement { pos: [r[0], r[1], r[2]], yaw: 0.0, motion: Motion::Static });
                head.push(Placement { pos: [r[0], r[1] + 10.0, r[2]], yaw: 0.0, motion: Motion::Static });
            }
            out.push(CourseObject { name: "snowman_body".into(), look: sprite("gTextureSnowmanBody", Some("gTLUTSnowman"), 14.0, false), lit: false, placements: body });
            out.push(CourseObject { name: "snowman_head".into(), look: sprite("gTextureSnowmanHead", Some("gTLUTSnowman"), 10.0, false), lit: false, placements: head });
        }
        "koopa_troopa_beach" => {
            // 10 crabs (gCrabSpawns: x1, x2, z1, z2 — each walks between two points)
            let nums = c_numbers(decomp, "src/data/some_data.c", "gCrabSpawns");
            let mut pl = Vec::new();
            for r in nums.chunks_exact(4) {
                let (a, b) = ([r[0], 0.0, r[2]], [r[1], 0.0, r[3]]);
                let d_u = [b[0] - a[0], 0.0, b[2] - a[2]];
                let len = (d_u[0] * d_u[0] + d_u[2] * d_u[2]).sqrt();
                let d_tm = mesh::normalize([if frame.mirror { -d_u[0] } else { d_u[0] }, 0.0, d_u[2]]);
                pl.push(Placement { pos: a, yaw: 0.0, motion: Motion::Shuttle { dir: d_tm, len_m: (len * m_per_u).max(1.0), speed_mps: 1.2, snap: false } });
            }
            out.push(CourseObject { name: "crab".into(), look: sprite("gTextureCrab1", Some("gTLUTCrab"), 9.6, false), lit: false, placements: pl });
        }
        "banshee_boardwalk" => {
            // the trash bin (init_bb_trash_bin: model, at the path's start area)
            // and ten Boos (48×40 sprites) hovering along the lap
            let mut pl = Vec::new();
            for k in 0..10 {
                let i = (k * n) / 10 + n / 20;
                let p = path_u[i % n];
                let h = path_heading(&path_u, i % n);
                pl.push(Placement { pos: [p[0] + h.cos() * 25.0, p[1] + 25.0, p[2] - h.sin() * 25.0], yaw: h, motion: Motion::Bob { rise_m: 1.2, up_ms: 1500 + (k as u32 * 97) % 600, hold_ms: 200, down_ms: 1500 + (k as u32 * 131) % 600, rest_ms: 200 } });
            }
            out.push(CourseObject { name: "boo".into(), look: sprite("gTextureBoo01", Some("gTLUTBoo"), 14.0, false), lit: false, placements: pl });
        }
        "sherbet_land" => {
            // penguins (3D models, update_objects.c func_800845C8 & co): the big
            // spinning one, four on the lower ice, seven small walkers
            // the penguin is an assembled model (unk_data1: body dl_8D00, the
            // wings dl_8730 at ±(0x33, 0x54, −0x0d), feet dl_8930 …)
            {
                let parts: Vec<(String, [f32; 3])> = vec![
                    ("d_course_sherbet_land_dl_8D00".to_string(), [0.0, 0.0, 0.0]),
                    ("d_course_sherbet_land_dl_8730".to_string(), [-51.0, 84.0, -13.0]),
                    ("d_course_sherbet_land_dl_8730".to_string(), [51.0, 84.0, -13.0]),
                    ("d_course_sherbet_land_dl_8930".to_string(), [-38.0, -54.0, -13.0]),
                    ("d_course_sherbet_land_dl_8930".to_string(), [38.0, -54.0, -13.0]),
                ];
                let mk = |s: f32| Look::Model { dls: parts.clone(), scale: s, initial_tex: None };
                out.push(CourseObject { name: "penguin_big".into(), look: mk(0.2), lit: true, placements: vec![Placement { pos: [-383.0, 2.0, -690.0], yaw: 0.0, motion: Motion::Spin { axis: 1, period_ms: 4000 } }] });
                let low = [[-2960.0, -80.0, 1521.0], [-2490.0, -80.0, 1612.0], [-2098.0, -80.0, 1624.0], [-2080.0, -80.0, 1171.0]];
                out.push(CourseObject { name: "penguin_low".into(), look: mk(0.08), lit: true, placements: low.iter().map(|p| Placement { pos: *p, yaw: 0.0, motion: Motion::Static }).collect() });
                let small = [[146.0, 0.0, -380.0], [380.0, 0.0, -535.0], [380.0, 0.0, -766.0], [-2300.0, 0.0, -210.0], [-2500.0, 0.0, -250.0], [-535.0, 0.0, 875.0], [-250.0, 0.0, 953.0]];
                out.push(CourseObject { name: "penguin_small".into(), look: mk(0.15), lit: true, placements: small.iter().enumerate().map(|(k, p)| Placement { pos: *p, yaw: k as f32 * 0.9, motion: Motion::Shuttle { dir: mesh::normalize([(k as f32 * 0.9).sin(), 0.0, (k as f32 * 0.9).cos()]), len_m: 4.0, speed_mps: 1.0, snap: false } }).collect() });
            }
        }
        "rainbow_road" => {
            // ten neon signs (64×64, sizeScaling 8 → 512 units), positions from
            // update_objects.c / D_800E6734
            let fixed = [("gTextureRainbowRoadNeonMushroom", "gTLUTRainbowRoadNeonMushroom1", [-1431.0, 827.0, -2957.0]), ("gTextureRainbowRoadNeonMario", "gTLUTRainbowRoadNeonMario1", [799.0, 1193.0, -5891.0]), ("gTextureRainbowRoadNeonBoo", "gTLUTRainbowRoadNeonBoo1", [-2013.0, 555.0, 0.0])];
            for (sym, tl, pos) in fixed {
                out.push(CourseObject { name: sym.trim_start_matches("gTextureRainbowRoad").to_lowercase(), look: sprite(sym, Some(tl), 512.0, false), lit: false, placements: vec![Placement { pos, yaw: 0.0, motion: Motion::Static }] });
            }
            let statics = ["Peach", "Luigi", "DonkeyKong", "Yoshi", "Bowser", "Wario", "Toad"];
            let pos = triples(&c_numbers(decomp, "src/data/some_data.c", "D_800E6734"));
            for (k, name) in statics.iter().enumerate() {
                if let Some(p) = pos.get(k) {
                    out.push(CourseObject { name: format!("neon_{}", name.to_lowercase()), look: sprite(&format!("gTextureRainbowRoadNeon{name}"), Some(&format!("gTLUTRainbowRoadNeon{name}")), 512.0, false), lit: false, placements: vec![Placement { pos: *p, yaw: 0.0, motion: Motion::Static }] });
                }
            }
            // three Chain Chomps charging down the road from path points 500, 800,
            // 1100 (update_objects.c: arg×300+500), 4 units a frame
            let chomp = Look::Model {
                dls: ["d_course_rainbow_road_dl_15550", "d_course_rainbow_road_dl_151A8", "d_course_rainbow_road_dl_15C68", "d_course_rainbow_road_dl_158C0", "d_course_rainbow_road_dl_15F18"].iter().map(|d| (d.to_string(), [0.0, 570.0, 0.0])).collect(),
                scale: 0.03,
                initial_tex: None,
            };
            let mut pl = Vec::new();
            for k in 0..3usize {
                let i = (k * 300 + 500) % n;
                let p = path_u[i];
                let (d, len) = straight_from(path_tm, i, 15.0, 400.0);
                pl.push(Placement { pos: [p[0], p[1] - 15.0, p[2]], yaw: path_heading(&path_u, i), motion: Motion::Shuttle { dir: d, len_m: len.max(20.0), speed_mps: 240.0 * m_per_u, snap: true } });
            }
            out.push(CourseObject { name: "chain_chomp".into(), look: chomp, lit: true, placements: pl });
        }
        "bowsers_castle" => {
            // the Thwomps crush: the same twelve, rising slowly and slamming down
            let look = Look::Model { dls: vec![("d_course_bowsers_castle_dl_thwomp".to_string(), [0.0, 0.0, 0.0])], scale: 1.0, initial_tex: Some(("gTextureThwompFace1".to_string(), 16, 64, false, true)) };
            let pl = crate::actors::THWOMPS_150CC.iter().enumerate().map(|(k, (x, z))| Placement { pos: [*x as f32, 0.0, *z as f32], yaw: 0.0, motion: Motion::Bob { rise_m: 3.0, up_ms: 1400, hold_ms: 700 + (k as u32 * 173) % 800, down_ms: 250, rest_ms: 900 + (k as u32 * 211) % 700 } }).collect();
            out.push(CourseObject { name: "thwomp".into(), look, lit: true, placements: pl });
        }
        "choco_mountain" => {
            // the falling rocks (spawn table), dropping from 25 units up
            let spawns: Vec<[f32; 3]> = course.spawns.iter().filter(|(nm, _)| nm.ends_with("falling_rock_spawns")).flat_map(|(_, v)| v.iter().map(|s| [s.pos[0] as f32, s.pos[1] as f32, s.pos[2] as f32])).collect();
            let pl = spawns.iter().enumerate().map(|(k, p)| Placement { pos: *p, yaw: 0.0, motion: Motion::Bob { rise_m: 25.0 * m_per_u, up_ms: 1, hold_ms: 1200 + (k as u32 * 300), down_ms: 700, rest_ms: 800 } }).collect();
            out.push(CourseObject { name: "falling_rock".into(), look: model(&["d_course_choco_mountain_dl_falling_rock"], 1.0), lit: true, placements: pl });
        }
        "luigi_raceway" => {
            // the hot-air balloon hovering 18 units up over (−176, −2323)
            out.push(CourseObject { name: "hot_air_balloon".into(), look: model(&["d_course_luigi_raceway_dl_F960"], 1.0), lit: true, placements: vec![Placement { pos: [-176.0, 18.0, -2323.0], yaw: 0.0, motion: Motion::Bob { rise_m: 1.5, up_ms: 3000, hold_ms: 400, down_ms: 3000, rest_ms: 400 } }] });
        }
        "kalimari_desert" => {
            // four railroad crossings (actors.c), gates up
            let cross = [([-1680.0, 2.0, 35.0], 0.0f32), ([-1600.0, 2.0, 35.0], 0.0), ([-2459.0, 2.0, 2263.0], -45f32.to_radians()), ([-2467.0, 2.0, 2375.0], -45f32.to_radians())];
            out.push(CourseObject { name: "crossing".into(), look: model(&["d_course_kalimari_desert_dl_crossing_both_inactive"], 1.0), lit: false, placements: cross.iter().map(|(p, y)| Placement { pos: *p, yaw: *y, motion: Motion::Static }).collect() });
            // the train, parked along its own path: engine, tender, two cars
            if let Some(rail) = course.other_paths.get("d_course_kalimari_desert_train_path") {
                let rail_f: Vec<[f32; 3]> = rail.iter().map(|p| [p[0] as f32, p[1] as f32, p[2] as f32]).collect();
                let cars = [("d_course_kalimari_desert_dl_1C0F0", 0usize), ("d_course_kalimari_desert_dl_1D670", 1), ("d_course_kalimari_desert_dl_1E910", 2), ("d_course_kalimari_desert_dl_1E910", 3)];
                // ~60 units apart along the rail from its first point
                for (k, (dl, slot)) in cars.iter().enumerate() {
                    let mut dist = 0.0f32;
                    let mut idx = 0usize;
                    let want = *slot as f32 * 60.0 + 40.0;
                    while idx + 1 < rail_f.len() && dist < want {
                        let (a, b) = (rail_f[idx], rail_f[idx + 1]);
                        dist += ((b[0] - a[0]).powi(2) + (b[2] - a[2]).powi(2)).sqrt();
                        idx += 1;
                    }
                    let p = rail_f[idx.min(rail_f.len() - 1)];
                    let h = path_heading(&rail_f, idx.min(rail_f.len() - 2));
                    out.push(CourseObject { name: format!("train_{k}"), look: model(&[dl], 1.0), lit: false, placements: vec![Placement { pos: p, yaw: h, motion: Motion::Static }] });
                }
            }
        }
        "toads_turnpike" => {
            // traffic: box truck, bus, tanker, car (course dls 0/3/6/9), twelve
            // vehicles spaced round the lap in three lanes, driving the straights
            // the lod0 bodies (course_data.c: the lists loading *_model_lod0)
            let kinds = ["d_course_toads_turnpike_dl_18DB8", "d_course_toads_turnpike_dl_1B5C8", "d_course_toads_turnpike_dl_1E288", "d_course_toads_turnpike_dl_21648"];
            for (k, dl) in kinds.iter().enumerate() {
                let mut pl = Vec::new();
                for j in 0..3 {
                    let i = ((k * 3 + j) * n) / 12 + n / 24;
                    let p = path_u[i % n];
                    let h = path_heading(&path_u, i % n);
                    let lane = (j as f32 - 1.0) * 9.0;
                    let pos = [p[0] + h.cos() * lane, p[1], p[2] - h.sin() * lane];
                    let (d, len) = straight_from(path_tm, i % n, 10.0, 250.0);
                    pl.push(Placement { pos, yaw: h, motion: Motion::Shuttle { dir: d, len_m: len.max(20.0), speed_mps: 18.0, snap: true } });
                }
                out.push(CourseObject { name: format!("traffic_{k}"), look: model(&[dl], 1.0), lit: false, placements: pl });
            }
        }
        "dks_jungle_parkway" => {
            // the paddle boat on the river, at its path's start
            if let Some(ferry) = course.other_paths.get("d_course_dks_jungle_parkway_ferry_path") {
                let f: Vec<[f32; 3]> = ferry.iter().map(|p| [p[0] as f32, p[1] as f32, p[2] as f32]).collect();
                let i = f.len() / 4;
                let h = path_heading(&f, i.min(f.len().saturating_sub(2)));
                out.push(CourseObject { name: "paddle_boat".into(), look: model(&["d_course_dks_jungle_parkway_boat_dl", "d_course_dks_jungle_parkway_railings_dl", "d_course_dks_jungle_parkway_paddle_wheel_dl"], 1.0), lit: false, placements: vec![Placement { pos: f[i], yaw: h, motion: Motion::Static }] });
            }
        }
        _ => {}
    }
    out
}

/// The image at half resolution: each 2×2 block's majority-opaque colour (the
/// mean of its opaque pixels; transparent when fewer than two are opaque).
fn downsample2(img: &Image) -> Image {
    let (w, h) = (img.w / 2, img.h / 2);
    let mut rgba = Vec::with_capacity((w * h * 4) as usize);
    for y in 0..h {
        for x in 0..w {
            let (mut r, mut g, mut b, mut n) = (0u32, 0u32, 0u32, 0u32);
            for (dx, dy) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
                let p = img.pixel(x * 2 + dx, y * 2 + dy);
                if p[3] >= 128 {
                    r += p[0] as u32;
                    g += p[1] as u32;
                    b += p[2] as u32;
                    n += 1;
                }
            }
            if n >= 2 {
                rgba.extend_from_slice(&[(r / n) as u8, (g / n) as u8, (b / n) as u8, 255]);
            } else {
                rgba.extend_from_slice(&[0, 0, 0, 0]);
            }
        }
    }
    Image { w, h, rgba }
}
