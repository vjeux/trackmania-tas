//! The item boxes: MK64's spinning "?" cubes at every `<x>_item_box_spawns`
//! position, as KINEMATIC parts of one item per course — a visual only (no
//! collision, no gameplay), spinning about the vertical like the game's own
//! rotors (vjeux, 2026-09-22: "moving blocks for the item boxes … would look
//! super cool").
//!
//! The moving-part machinery is `mapgeom`'s: a `CPlugDynaObjectModel` entity
//! with an `NPlugDyna_SKinematicConstraint` in a prefab-form item. The dyna
//! model bytes and the entity params come from a pack obstacle
//! (`ObstaclePusher8mLevel1`), the constraint is the pack rotor's shape (a
//! full turn, linear, looping) turned onto the Y axis.

use crate::mesh::{self, Frame, Material, FLAT_SYM};
use crate::texture::{AssetIndex, Image, Rom};
use mapgeom::crystal_model::CPlugMaterialUserInst;
use mapgeom::static_item::assemble::{assemble, BuildOpts};
use mapgeom::static_item::bake::{self, Corner, VisualLayout};
use mapgeom::static_item::build::static_item_from_pack_item_report;
use mapgeom::static_item::merged::{DynaPart, Merged, MergedVisual};
use mapgeom::static_item::write_file;
use mapgeom::store::DataStore;
use std::collections::BTreeMap;

/// The pack item whose dyna part lends its model bytes and entity params.
pub const TEMPLATE_ITEM: &str = "Stadium\\Items\\ObstaclePusher8mLevel1.Item.Gbx";
/// Side of the "?" cube (MK64's box is about 10 units across).
pub const OUTER_M: f32 = 10.0 * mesh::UNITS_TO_M;
pub const INNER_M: f32 = 8.0 * mesh::UNITS_TO_M;
/// The box hovers this much above its spawn point (the game floats them
/// at kart height).
pub const HOVER_UNITS: f32 = 9.0;
pub const SPIN_MS: u32 = 3000;
/// The game material of the spinning cube (a moving part shows no embedded
/// texture) and whether it is a full cube or an edge frame.
/// Line-up of 24 game materials on the spinning cube (2026-09-23, editor shots
/// ibsc/ibsd, box 0 at the RIGHT): `SpecialFXTurbo` — the turbo gate's glowing
/// translucent gold, the "?" inside shows through — is the closest to the N64
/// box; `LightShape` (milky white glow, opaque up close) second; the TriggerFX/Inflatable/Gate ones are invisible or
/// flat, Pylon/GlossyFloor opaque grey, SpecialSignTurbo a flat yellow.
pub const DEFAULT_LINK: &str = "Stadium\\Media\\Material\\SpecialFXTurbo";
pub const DEFAULT_FULL: bool = true;
/// The rainbow of the inner cube: one tint per face.
pub const FACE_TINTS: [[u8; 3]; 6] = [[255, 64, 64], [255, 220, 0], [64, 220, 64], [64, 220, 255], [80, 96, 255], [255, 96, 255]];

pub struct ItemBoxes {
    /// The spinning frames (prefab form, a game material).
    pub bytes: Vec<u8>,
    /// The "?" marks: a plain static item (prefab-form items resolve no
    /// embedded texture at all — static part included, probed 2026-09-22).
    pub marks: Vec<u8>,
    pub pos: [f32; 3],
    pub pictures: BTreeMap<String, Vec<u8>>,
    pub count: usize,
}

/// One item holding every item box of the course, or None when the course
/// has no item box spawns.
pub fn build(store: &mut DataStore, name: &str, spawns: &[[i16; 3]], frame: &Frame, assets: &AssetIndex, rom: &mut Rom, tag: &str) -> Result<Option<ItemBoxes>, String> {
    build_faces(store, name, spawns, frame, assets, rom, tag, None)
}

/// The colour material: `ItemInflatableMat` follows the PLACEMENT's colour
/// byte (measured 2026-09-25 on a six-item line-up: Default yellow, White,
/// Green, Blue, Red, Black); none of the FX/light/tech materials did.
pub const COLOUR_LINK: &str = "Stadium\\Media\\Material\\ItemInflatableMat";

/// `face`: Some(k) builds only face k of every cube — the six-faces-six-colours
/// item box is six such items, each placed with its own colour byte (vjeux
/// 2026-09-25: "in the real game they are all sorts of colors").
pub fn build_faces(store: &mut DataStore, name: &str, spawns: &[[i16; 3]], frame: &Frame, assets: &AssetIndex, rom: &mut Rom, tag: &str, face: Option<usize>) -> Result<Option<ItemBoxes>, String> {
    if spawns.is_empty() {
        return Ok(None);
    }
    // the template dyna part
    let (_, tmpl) = static_item_from_pack_item_report(store, TEMPLATE_ITEM, "Template.Item.Gbx", "Template", 1.0, crate::tm::STADIUM, 0)?;
    let part0 = tmpl.dyna.first().ok_or_else(|| format!("{TEMPLATE_ITEM}: no moving part in the template"))?.clone();
    let (mut kc, cparams) = part0.constraint.clone().ok_or("template part has no constraint")?;
    // a full turn about Y every SPIN_MS, looping; no translation
    kc.trans_axis = 0;
    kc.trans_min = 0.0;
    kc.trans_max = 0.0;
    kc.trans.subs = vec![mapgeom::static_item::dyna::AnimSubFunc { ease: 1, reverse: 1, duration_ms: 8000 }];
    kc.rot_axis = 1;
    kc.angle_min_deg = 180.0;
    kc.angle_max_deg = -180.0;
    kc.rot.subs = vec![mapgeom::static_item::dyna::AnimSubFunc { ease: 1, reverse: 1, duration_ms: SPIN_MS }];
    kc.shader_tc_type = 0;
    kc.shader_tc_anim.clear();
    kc.shader_tc_trans_sub = None;

    // textures: the "?" (alpha) and the six flat tints
    let mut pictures: BTreeMap<String, Vec<u8>> = BTreeMap::new();
    let q_loc = assets.by_path.iter().find(|(k, _)| k.ends_with("/common_texture_item_box_question_mark.rgba16.png")).map(|(_, v)| v.clone()).ok_or("item box question mark texture not in the asset index")?;
    let q_img = rom.texture(&q_loc, None)?.upscale(crate::tm::TEXTURE_UPSCALE);
    let q_mat = Material { sym: "ItemBoxQ".into(), mirror_s: false, mirror_t: false, clamp_s: true, clamp_t: true, w: q_loc.w, h: q_loc.h, fmt: 0, tint: [255, 255, 255] };
    pictures.insert(format!("{tag}_{}.dds", q_mat.stem()), mapgeom::static_item::texture::write_dds_picture(q_img.w, q_img.h, &q_img.rgba));
    let flat_mats: Vec<Material> = FACE_TINTS.iter().map(|t| Material { sym: FLAT_SYM.into(), mirror_s: false, mirror_t: false, clamp_s: false, clamp_t: false, w: 4, h: 4, fmt: 0, tint: *t }).collect();
    for m in &flat_mats {
        let img = Image::solid(4, 4, [255, 255, 255, 255]).tinted(m.tint).upscale(crate::tm::TEXTURE_UPSCALE);
        pictures.insert(format!("{tag}_{}.dds", m.stem()), mapgeom::static_item::texture::write_dds_picture(img.w, img.h, &img.rgba));
    }

    // the item's origin: the first box, at ground level
    let origin = {
        let p = frame.to_tm(spawns[0]);
        [(p[0] * 100.0).round() / 100.0, (p[1] * 100.0).round() / 100.0, (p[2] * 100.0).round() / 100.0]
    };
    let mut merged = Merged::default();
    let unix = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
    merged.file_write_time = unix * 10_000_000 + 116444736000000000;
    for (box_index, sp) in spawns.iter().enumerate() {
        let world = frame.to_tm_f([sp[0] as f32, sp[1] as f32 + HOVER_UNITS, sp[2] as f32]);
        let pos = [world[0] - origin[0], world[1] - origin[1], world[2] - origin[2]];
        let mut mesh = Merged::default();
        mesh.file_write_time = merged.file_write_time;
        // outer "?" cube (alpha-cut faces) and inner rainbow cube
        // MK64_IB_TEXPREFIX: how a dyna part's user texture is spelled (probe:
        // the static objects resolve the bare name; a moving part showed the
        // missing-texture checker with it, 2026-09-22)
        let prefix = std::env::var("MK64_IB_TEXPREFIX").unwrap_or_default();
        let with_prefix = |mut m: mapgeom::crystal_model::CPlugMaterialUserInst| {
            if let Some(main) = m.main.as_mut() {
                for t in main.user_textures.iter_mut() {
                    t.texture = format!("{prefix}{}", t.texture);
                }
            }
            m
        };
        // A moving part does NOT resolve user textures (probed 2026-09-22: bare,
        // `Items\`, `Items/` and the sidecar-file form all drew the missing-
        // texture checker; `..\Items\` crashed the client). So the spinning
        // cube wears a GAME material (MK64_IB_LINK, default a glowing sign
        // material) and the "?" is a STATIC crossed billboard inside it.
        // MK64_IB_SHARED=1: the cube wears the custom-texture materials the item's
        // STATIC part defines (shared material nodes, `Merged::share_materials`)
        let shared = std::env::var("MK64_IB_SHARED").as_deref() == Ok("1");
        mesh.share_materials = shared;
        let (mut per_material, slots): (Vec<Vec<[Corner; 3]>>, Vec<usize>) = if shared {
            let mut pm = Vec::new();
            let mut sl = Vec::new();
            mesh.materials.push(with_prefix(crate::tm::custom_material(&q_mat, true, crate::tm::PHYS_CONCRETE, tag)));
            sl.push(mesh.materials.len() - 1);
            pm.push(cube_faces(OUTER_M, None));
            for (k, fm) in flat_mats.iter().enumerate() {
                mesh.materials.push(with_prefix(crate::tm::custom_material(fm, false, crate::tm::PHYS_CONCRETE, tag)));
                sl.push(mesh.materials.len() - 1);
                pm.push(cube_faces(INNER_M, Some(k)));
            }
            (pm, sl)
        } else {
            // MK64_IB_SURVEY="link,link,…": box k wears candidate k (mod n) as a
            // FULL cube — the material line-up shot. MK64_IB_LINK: one material
            // for all; MK64_IB_FULL=1: a full cube instead of the frame.
            let survey: Vec<String> = std::env::var("MK64_IB_SURVEY").map(|s| s.split(',').map(|x| x.trim().to_string()).filter(|x| !x.is_empty()).collect()).unwrap_or_default();
            let link = match face {
                Some(_) => COLOUR_LINK.to_string(),
                None => {
                    if survey.is_empty() { std::env::var("MK64_IB_LINK").unwrap_or_else(|_| DEFAULT_LINK.to_string()) } else { survey[box_index % survey.len()].clone() }
                }
            };
            mesh.materials.push(CPlugMaterialUserInst::game_material(&link, crate::tm::PHYS_CONCRETE));
            let full = !survey.is_empty() || std::env::var("MK64_IB_FULL").is_ok() || DEFAULT_FULL;
            // a colour item is the BORDER band of its face, just outside the
            // translucent gold cube (which stays, with the "?" showing through)
            let geometry = match face {
                Some(k) => face_ring(OUTER_M * 1.02, k, 0.16 * OUTER_M),
                None if full => cube_faces(OUTER_M, None),
                None => cube_frame(OUTER_M, 0.12 * OUTER_M),
            };
            (vec![geometry], vec![mesh.materials.len() - 1])
        };
        let has_uv1 = vec![true; per_material.len()];
        bake::assign_lightmap_atlas(&mut per_material, &has_uv1);
        for (i, tris) in per_material.iter_mut().enumerate() {
            if tris.is_empty() {
                continue;
            }
            bake::tangents_vprim(tris, 0);
            for v in bake::make_visuals(tris, VisualLayout::Full, "range") {
                mesh.visuals.push(MergedVisual::every_level(v, slots[i]));
            }
        }
        merged.dyna.push(DynaPart {
            path: "mk64:itembox".to_string(),
            rot: [0.0, 0.0, 0.0, 1.0],
            pos,
            mesh,
            // a moving part wants a hull (item-check: "no collision surface"): a
            // 1 mm triangle at the cube's centre, unreachable
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
            constraint: Some((kc.clone(), cparams.clone())),
            // MK64_IB_REF=file: the dyna object and its mesh as sidecar files next
            // to the item (probe: does a moving part find user textures then?)
            pack_ref: if std::env::var("MK64_IB_REF").as_deref() == Ok("file") { Some(mapgeom::static_item::merged::PackRef::File) } else { None },
        });
    }
    // the "?" marks: a static crossed billboard (alpha-cut custom texture) at
    // every box centre — its own plain static item
    let shared = std::env::var("MK64_IB_SHARED").as_deref() == Ok("1");
    let mut marks_item = Merged::default();
    marks_item.file_write_time = merged.file_write_time;
    merged.share_materials = shared;
    {
        // shared: the marks live in the moving item's STATIC part, defining
        // the material nodes the cubes back-reference (plus a 1 mm triangle
        // per tint so every cube material is defined there)
        let marks: &mut Merged = if shared { &mut merged } else { &mut marks_item };
        let q_slot = {
            marks.materials.push(crate::tm::custom_material(&q_mat, true, crate::tm::PHYS_CONCRETE, tag));
            marks.materials.len() - 1
        };
        if shared {
            for fm in &flat_mats {
                marks.materials.push(crate::tm::custom_material(fm, false, crate::tm::PHYS_CONCRETE, tag));
                let slot = marks.materials.len() - 1;
                let c = |x: f32, z: f32| Corner { pos: [x, -3.0, z], normal: [0.0, 1.0, 0.0], uv: [0.0, 0.0], uv1: [0.0, 0.0], tan_u: [1.0, 0.0, 0.0], tan_v: [0.0, 0.0, 1.0], face: 0, group: 0 };
                let mut pm = vec![vec![[c(0.0, 0.0), c(0.001, 0.0), c(0.0, 0.001)]]];
                bake::assign_lightmap_atlas(&mut pm, &[true]);
                bake::tangents_vprim(&mut pm[0], 0);
                for v in bake::make_visuals(&mut pm[0], VisualLayout::Full, "range") {
                    marks.visuals.push(MergedVisual::every_level(v, slot));
                }
            }
        }
        let mut tris: Vec<[Corner; 3]> = Vec::new();
        let hw = INNER_M * 0.5 * 0.5; // the texture is 32×64: half as wide as tall
        let hh = INNER_M * 0.5;
        for sp in spawns {
            let world = frame.to_tm_f([sp[0] as f32, sp[1] as f32 + HOVER_UNITS, sp[2] as f32]);
            let c = [world[0] - origin[0], world[1] - origin[1], world[2] - origin[2]];
            for (ux, uz) in [(1.0f32, 0.0f32), (0.0, 1.0)] {
                let n = [uz, 0.0, -ux];
                let corner = |su: f32, sv: f32| Corner { pos: [c[0] + ux * su * hw, c[1] + sv * hh, c[2] + uz * su * hw], normal: n, uv: [(su + 1.0) / 2.0, (sv + 1.0) / 2.0], uv1: [(su + 1.0) / 2.0, (sv + 1.0) / 2.0], tan_u: [ux, 0.0, uz], tan_v: [0.0, 1.0, 0.0], face: 0, group: 0 };
                let (a, b, cc, d) = (corner(-1.0, -1.0), corner(1.0, -1.0), corner(1.0, 1.0), corner(-1.0, 1.0));
                // both windings: visible from either side
                tris.push([a, b, cc]);
                tris.push([a, cc, d]);
                tris.push([a, cc, b]);
                tris.push([a, d, cc]);
            }
        }
        let mut per_material = vec![tris];
        bake::assign_lightmap_atlas(&mut per_material, &[true]);
        bake::tangents_vprim(&mut per_material[0], 0);
        for v in bake::make_visuals(&mut per_material[0], VisualLayout::Full, "range") {
            marks.visuals.push(MergedVisual::every_level(v, q_slot));
        }
    }
    let opts = BuildOpts { ident: name.to_string(), author: name.to_string(), scale: 1.0, collection: crate::tm::STADIUM, skin: None };
    mapgeom::static_item::assemble::SIDECARS.with(|s| s.borrow_mut().clear());
    let f = assemble(&merged, &opts)?;
    // sidecar files (the file form) ride along as embedded files next to the item
    for (n, b) in mapgeom::static_item::assemble::SIDECARS.with(|s| std::mem::take(&mut *s.borrow_mut())) {
        pictures.insert(n, b);
    }
    let marks_bytes = if shared || face.is_some() {
        Vec::new()
    } else {
        let marks_name = name.replace("_itemboxes.Item.Gbx", "_itemmarks.Item.Gbx");
        let mopts = BuildOpts { ident: marks_name.clone(), author: marks_name, scale: 1.0, collection: crate::tm::STADIUM, skin: None };
        write_file(&assemble(&marks_item, &mopts)?)
    };
    Ok(Some(ItemBoxes { bytes: write_file(&f), marks: marks_bytes, pos: origin, pictures, count: spawns.len() }))
}

/// The 12 triangles of an axis-aligned cube of side `s` centred at the
/// origin, outward normals. `face_only` keeps one face (the inner cube's
/// per-face tints); None keeps all six, each mapped to the whole texture.
fn cube_faces(s: f32, face_only: Option<usize>) -> Vec<[Corner; 3]> {
    let h = s / 2.0;
    // (normal, u axis, v axis) per face
    let faces: [([f32; 3], [f32; 3], [f32; 3]); 6] = [
        ([0.0, 0.0, 1.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]),
        ([0.0, 0.0, -1.0], [-1.0, 0.0, 0.0], [0.0, 1.0, 0.0]),
        ([1.0, 0.0, 0.0], [0.0, 0.0, -1.0], [0.0, 1.0, 0.0]),
        ([-1.0, 0.0, 0.0], [0.0, 0.0, 1.0], [0.0, 1.0, 0.0]),
        ([0.0, 1.0, 0.0], [1.0, 0.0, 0.0], [0.0, 0.0, -1.0]),
        ([0.0, -1.0, 0.0], [1.0, 0.0, 0.0], [0.0, 0.0, 1.0]),
    ];
    let mut out = Vec::new();
    for (fi, (n, ua, va)) in faces.iter().enumerate() {
        if let Some(k) = face_only {
            if k != fi {
                continue;
            }
        }
        let c = |su: f32, sv: f32| {
            let pos = [n[0] * h + ua[0] * su * h + va[0] * sv * h, n[1] * h + ua[1] * su * h + va[1] * sv * h, n[2] * h + ua[2] * su * h + va[2] * sv * h];
            // u right, v up in the image (the game samples bottom-up)
            let uv = [(su + 1.0) / 2.0, (sv + 1.0) / 2.0];
            Corner { pos, normal: *n, uv, uv1: uv, tan_u: *ua, tan_v: *va, face: fi as u32 + 1, group: 0 }
        };
        let (a, b, cc, d) = (c(-1.0, -1.0), c(1.0, -1.0), c(1.0, 1.0), c(-1.0, 1.0));
        // counter-clockwise seen from outside: normal = cross(b−a, c−a) = +n
        let t1 = [a, b, cc];
        let t2 = [a, cc, d];
        let check = mesh::face_normal(&[t1[0].pos, t1[1].pos, t1[2].pos]);
        if check[0] * n[0] + check[1] * n[1] + check[2] * n[2] > 0.0 {
            out.push(t1);
            out.push(t2);
        } else {
            out.push([a, cc, b]);
            out.push([a, d, cc]);
        }
    }
    out
}

/// A cube's 12 edges as square bars of thickness `t` (the "?" inside stays
/// visible through the open faces).
fn cube_frame(s: f32, t: f32) -> Vec<[Corner; 3]> {
    let h = s / 2.0;
    let mut out = Vec::new();
    // an axis-aligned box from lo to hi, outward faces
    let mut push_box = |lo: [f32; 3], hi: [f32; 3]| {
        let c = [(lo[0] + hi[0]) / 2.0, (lo[1] + hi[1]) / 2.0, (lo[2] + hi[2]) / 2.0];
        let e = [(hi[0] - lo[0]) / 2.0, (hi[1] - lo[1]) / 2.0, (hi[2] - lo[2]) / 2.0];
        for tri in cube_faces(1.0, None) {
            let mut tri2 = tri;
            for corner in tri2.iter_mut() {
                // cube_faces(1.0) spans −0.5..0.5: stretch to the box
                corner.pos = [c[0] + corner.pos[0] * 2.0 * e[0], c[1] + corner.pos[1] * 2.0 * e[1], c[2] + corner.pos[2] * 2.0 * e[2]];
            }
            out.push(tri2);
        }
    };
    for &y in &[-h, h] {
        for &z in &[-h, h] {
            push_box([-h, y - t / 2.0, z - t / 2.0], [h, y + t / 2.0, z + t / 2.0]); // along x
        }
        for &x in &[-h, h] {
            push_box([x - t / 2.0, y - t / 2.0, -h], [x + t / 2.0, y + t / 2.0, h]); // along z
        }
    }
    for &x in &[-h, h] {
        for &z in &[-h, h] {
            push_box([x - t / 2.0, -h, z - t / 2.0], [x + t / 2.0, h, z + t / 2.0]); // along y
        }
    }
    out
}

/// The border band of face `k` of a cube of side `s`: four quads `band` wide
/// along the face's edges (the face's centre stays open), outward normal.
fn face_ring(s: f32, k: usize, band: f32) -> Vec<[Corner; 3]> {
    let h = s / 2.0;
    let faces: [([f32; 3], [f32; 3], [f32; 3]); 6] = [
        ([0.0, 0.0, 1.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]),
        ([0.0, 0.0, -1.0], [-1.0, 0.0, 0.0], [0.0, 1.0, 0.0]),
        ([1.0, 0.0, 0.0], [0.0, 0.0, -1.0], [0.0, 1.0, 0.0]),
        ([-1.0, 0.0, 0.0], [0.0, 0.0, 1.0], [0.0, 1.0, 0.0]),
        ([0.0, 1.0, 0.0], [1.0, 0.0, 0.0], [0.0, 0.0, -1.0]),
        ([0.0, -1.0, 0.0], [1.0, 0.0, 0.0], [0.0, 0.0, 1.0]),
    ];
    let (n, ua, va) = faces[k % 6];
    let mut out = Vec::new();
    let c = |su: f32, sv: f32| {
        let pos = [n[0] * h + ua[0] * su + va[0] * sv, n[1] * h + ua[1] * su + va[1] * sv, n[2] * h + ua[2] * su + va[2] * sv];
        let uv = [(su / h + 1.0) / 2.0, (sv / h + 1.0) / 2.0];
        Corner { pos, normal: n, uv, uv1: uv, tan_u: ua, tan_v: va, face: k as u32 + 1, group: 0 }
    };
    let inner = h - band;
    // four bands: (u0,u1,v0,v1) rectangles in face coordinates
    let rects = [(-h, h, inner, h), (-h, h, -h, -inner), (-h, -inner, -inner, inner), (inner, h, -inner, inner)];
    for (u0, u1, v0, v1) in rects {
        let (a, b, cc, d) = (c(u0, v0), c(u1, v0), c(u1, v1), c(u0, v1));
        let t1 = [a, b, cc];
        let check = mesh::face_normal(&[t1[0].pos, t1[1].pos, t1[2].pos]);
        if check[0] * n[0] + check[1] * n[1] + check[2] * n[2] > 0.0 {
            out.push(t1);
            out.push([a, cc, d]);
        } else {
            out.push([a, cc, b]);
            out.push([a, d, cc]);
        }
    }
    out
}
