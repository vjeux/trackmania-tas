//! Moo Moo Farm's Monty Moles (vjeux, 2026-09-24: "Why don't we have moles in
//! moo moo farm?" — they were never ported). The game keeps 31 spawn points in
//! three groups (`gMoleSpawns`, src/data/some_data.c), seven 32×64 sprite
//! frames of the mole's LEFT half (mirrored at draw time, an arm-wave), a
//! 16×16 dirt texture for the mound, and pops a mole 9 units up out of its
//! mound, holds, and sinks it back (`func_80081924`).
//!
//! Here: every spawn gets a textured dirt mound (a plain static item, like the
//! item boxes' "?" marks — prefab-form items resolve no embedded texture) and
//! a MOVING mole: a kinematic part translating along Y between "buried" and
//! "up" on a per-mole rhythm. A moving part shows game materials only, so the
//! mole is its sprite (frame 0, mirrored to the full 64-wide figure) built as
//! pixel-art relief in `Material_BlockCustom\CustomPlastic` — one slot per
//! quantised sprite colour, tinted with the material's `TargetColor` constant
//! (the way the mesh-modeler nation items colour their parts): the brown coat,
//! cream belly, beige face, black shades, white claws all read at kart range.

use crate::mesh::{self, Frame, Material};
use crate::texture::{AssetIndex, Image, Rom};
use mapgeom::crystal_model::{CPlugMaterialUserInst, Cst, Id};
use mapgeom::static_item::assemble::{assemble, BuildOpts};
use mapgeom::static_item::bake::{self, Corner, VisualLayout};
use mapgeom::static_item::build::static_item_from_pack_item_report;
use mapgeom::static_item::dyna::AnimSubFunc;
use mapgeom::static_item::merged::{DynaPart, Merged, MergedVisual};
use mapgeom::static_item::write_file;
use mapgeom::store::DataStore;
use std::collections::BTreeMap;
use std::path::Path;

/// The mole's height in the game: it rises 9 units out of the ground.
pub const RISE_UNITS: f32 = 9.0;
/// The full sprite (64 px wide after the mirror) spans about 12 units — a mole
/// a little narrower than a kart.
pub const SPRITE_UNITS_WIDE: f32 = 12.0;
/// Relief depth of the pixel-art figure, metres.
pub const DEPTH_M: f32 = 0.22;
/// The dirt mound's diameter, units.
pub const MOUND_UNITS: f32 = 11.0;

pub struct Moles {
    /// The moving moles (prefab form, game materials).
    pub bytes: Vec<u8>,
    /// The dirt mounds (a plain static item with the textured quads).
    pub mounds: Vec<u8>,
    pub pos: [f32; 3],
    pub pictures: BTreeMap<String, Vec<u8>>,
    pub count: usize,
}

/// `gMoleSpawns` from the decomp's source: the 31 `{ x, y, z }` triples of the
/// `MoleSpawnUnion gMoleSpawns = { { … } };` initializer (hex i16s).
pub fn spawns(decomp: &Path) -> Result<Vec<[i16; 3]>, String> {
    let src = decomp.join("src/data/some_data.c");
    let text = std::fs::read_to_string(&src).map_err(|e| format!("{}: {e}", src.display()))?;
    let start = text.find("gMoleSpawns = {").ok_or("gMoleSpawns not found in some_data.c")?;
    let rest = &text[start..];
    let end = rest.find("};").ok_or("gMoleSpawns: unterminated initializer")?;
    let body = &rest[..end];
    let mut out = Vec::new();
    // every `{ 0x…, 0x…, 0x… }` inside
    let mut cursor = body;
    while let Some(i) = cursor.find("{ 0x") {
        let seg = &cursor[i + 1..];
        let j = seg.find('}').ok_or("gMoleSpawns: unbalanced braces")?;
        let nums: Vec<i16> = seg[..j]
            .split(',')
            .filter_map(|t| {
                let t = t.trim().trim_start_matches("0x");
                u16::from_str_radix(t, 16).ok().map(|v| v as i16)
            })
            .collect();
        if nums.len() == 3 {
            out.push([nums[0], nums[1], nums[2]]);
        }
        cursor = &seg[j..];
    }
    if out.is_empty() {
        return Err("gMoleSpawns: no triples parsed".into());
    }
    Ok(out)
}

/// The mole's colours snapped to a seven-colour palette (the sprite's 254
/// shades are shading of these): white claws, cream belly, beige face, three
/// browns of the coat, black shades. Per-channel quantisation kept 15–24
/// colours and 7–9k triangles a mole — the shading gradients made every run
/// a few pixels long.
const PALETTE: [[u8; 3]; 7] = [[255, 255, 255], [240, 236, 214], [232, 190, 118], [200, 62, 10], [140, 42, 8], [60, 18, 8], [10, 10, 10]];

fn quantise(rgb: [u8; 3]) -> [u8; 3] {
    let mut best = PALETTE[0];
    let mut bd = i32::MAX;
    for p in PALETTE {
        let d = (p[0] as i32 - rgb[0] as i32).pow(2) + (p[1] as i32 - rgb[1] as i32).pow(2) + (p[2] as i32 - rgb[2] as i32).pow(2);
        if d < bd {
            bd = d;
            best = p;
        }
    }
    best
}

/// A `CustomPlastic` game material tinted `rgb` through its `TargetColor`.
fn plastic(rgb: [u8; 3], physics: u8) -> CPlugMaterialUserInst {
    // see objects::plastic — CustomPlastic drew nothing on a moving part
    let link = std::env::var("MK64_OBJ_MAT").unwrap_or_else(|_| "Stadium\\Media\\Material\\TechnicsTrims".to_string());
    if !link.contains("Custom") {
        return CPlugMaterialUserInst::game_material(&link, physics);
    }
    let mut m = CPlugMaterialUserInst::game_material(&link, physics);
    if let Some(main) = m.main.as_mut() {
        main.material_name = Id::Str(format!("Mole_{:02x}{:02x}{:02x}", rgb[0], rgb[1], rgb[2]));
        main.csts = vec![Cst { u01: Id::Str("TargetColor".into()), u02: Id::Str("Real".into()), u03: 3 }];
        // sRGB bytes to the shader's linear floats
        let lin = |c: u8| -> f32 {
            let s = c as f32 / 255.0;
            if s <= 0.04045 { s / 12.92 } else { ((s + 0.055) / 1.055).powf(2.4) }
        };
        main.color = [lin(rgb[0]), lin(rgb[1]), lin(rgb[2])].iter().map(|c| c.to_bits() as i32).collect();
    }
    m
}

/// The pixel-art relief of an RGBA image: per quantised colour, the front and
/// back faces of every horizontal run of that colour and a side wall on every
/// boundary pixel edge. Image x right, y DOWN (row 0 at the top); the figure
/// stands with its bottom row at local y = 0, centred on x, facing −z (the
/// viewer at −z sees the image the right way round: image left = viewer's left
/// = local +x… so image x maps to −x).
pub fn relief(img: &Image, px_m: f32, depth: f32) -> BTreeMap<[u8; 3], Vec<[Corner; 3]>> {
    relief_with(img, px_m, depth, quantise)
}

pub fn relief_with(img: &Image, px_m: f32, depth: f32, quant: impl Fn([u8; 3]) -> [u8; 3]) -> BTreeMap<[u8; 3], Vec<[Corner; 3]>> {
    let (w, h) = (img.w as i32, img.h as i32);
    let at = |x: i32, y: i32| -> Option<[u8; 3]> {
        if x < 0 || y < 0 || x >= w || y >= h {
            return None;
        }
        let p = img.pixel(x as u32, y as u32);
        if p[3] < 128 { None } else { Some(quant([p[0], p[1], p[2]])) }
    };
    let mut out: BTreeMap<[u8; 3], Vec<[Corner; 3]>> = BTreeMap::new();
    // pixel (x, y) occupies local x in [-(x+1-w/2), -(x-w/2)]·px, y in [(h-1-y), (h-y)]·px
    let lx = |x: i32| -(x as f32 - w as f32 / 2.0) * px_m;
    let ly = |y: i32| (h - y) as f32 * px_m;
    let hz = depth / 2.0;
    let quad = |list: &mut Vec<[Corner; 3]>, a: [f32; 3], b: [f32; 3], c: [f32; 3], d: [f32; 3], n: [f32; 3]| {
        let corner = |p: [f32; 3], uv: [f32; 2]| Corner { pos: p, normal: n, uv, uv1: uv, tan_u: [1.0, 0.0, 0.0], tan_v: [0.0, 1.0, 0.0], face: 0, group: 0 };
        let (ca, cb, cc, cd) = (corner(a, [0.0, 0.0]), corner(b, [1.0, 0.0]), corner(c, [1.0, 1.0]), corner(d, [0.0, 1.0]));
        // winding so the face normal agrees with n
        let t = [ca, cb, cc];
        let fnrm = mesh::face_normal(&[t[0].pos, t[1].pos, t[2].pos]);
        if fnrm[0] * n[0] + fnrm[1] * n[1] + fnrm[2] * n[2] >= 0.0 {
            list.push([ca, cb, cc]);
            list.push([ca, cc, cd]);
        } else {
            list.push([ca, cc, cb]);
            list.push([ca, cd, cc]);
        }
    };
    // runs per row: (x0, x1, colour); identical runs in consecutive rows merge
    // into one rectangle (the belly and coat are flat colour areas)
    let mut runs: Vec<Vec<(i32, i32, [u8; 3])>> = Vec::with_capacity(h as usize);
    for y in 0..h {
        let mut row = Vec::new();
        let mut x = 0;
        while x < w {
            let Some(col) = at(x, y) else { x += 1; continue };
            let x0 = x;
            while x < w && at(x, y) == Some(col) {
                x += 1;
            }
            row.push((x0, x, col));
        }
        runs.push(row);
    }
    let mut consumed: Vec<Vec<bool>> = runs.iter().map(|r| vec![false; r.len()]).collect();
    for y in 0..h as usize {
        for ri in 0..runs[y].len() {
            if consumed[y][ri] {
                continue;
            }
            let (x0, x1, col) = runs[y][ri];
            // extend downward while the row below holds the identical run
            let mut y1 = y;
            while y1 + 1 < h as usize {
                match runs[y1 + 1].iter().position(|r| *r == (x0, x1, col)) {
                    Some(j) if !consumed[y1 + 1][j] => {
                        consumed[y1 + 1][j] = true;
                        y1 += 1;
                    }
                    _ => break,
                }
            }
            consumed[y][ri] = true;
            let list = out.entry(col).or_default();
            let (xl, xr) = (lx(x1), lx(x0));
            let (yb, yt) = (ly(y1 as i32 + 1), ly(y as i32));
            quad(list, [xl, yb, -hz], [xr, yb, -hz], [xr, yt, -hz], [xl, yt, -hz], [0.0, 0.0, -1.0]);
            quad(list, [xl, yb, hz], [xr, yb, hz], [xr, yt, hz], [xl, yt, hz], [0.0, 0.0, 1.0]);
        }
    }
    // side walls: every pixel edge on the SILHOUETTE (a wall between two
    // colours is inside the slab and never seen), merged along the edge
    for y in 0..h {
        // top and bottom edges along x
        for (dy, ny, side) in [(-1i32, 1.0f32, true), (1, -1.0, false)] {
            let mut x = 0;
            while x < w {
                let Some(col) = at(x, y) else { x += 1; continue };
                if at(x, y + dy).is_some() {
                    x += 1;
                    continue;
                }
                let x0 = x;
                while x < w && at(x, y) == Some(col) && at(x, y + dy).is_none() {
                    x += 1;
                }
                let list = out.entry(col).or_default();
                let (pl, pr) = (lx(x), lx(x0));
                let ey = if side { ly(y) } else { ly(y + 1) };
                quad(list, [pl, ey, -hz], [pr, ey, -hz], [pr, ey, hz], [pl, ey, hz], [0.0, ny, 0.0]);
            }
        }
    }
    for x in 0..w {
        // left and right edges along y
        for (dx, nx) in [(-1i32, 1.0f32), (1, -1.0)] {
            let mut y = 0;
            while y < h {
                let Some(col) = at(x, y) else { y += 1; continue };
                if at(x + dx, y).is_some() {
                    y += 1;
                    continue;
                }
                let y0 = y;
                while y < h && at(x, y) == Some(col) && at(x + dx, y).is_none() {
                    y += 1;
                }
                let list = out.entry(col).or_default();
                let ex = if dx < 0 { lx(x) } else { lx(x + 1) };
                let (yb, yt) = (ly(y), ly(y0));
                quad(list, [ex, yb, -hz], [ex, yb, hz], [ex, yt, hz], [ex, yt, -hz], [nx, 0.0, 0.0]);
            }
        }
    }
    out
}

/// The full mole: the stored left half and its mirror, side by side.
pub fn full_sprite(half: &Image) -> Image {
    let (w, h) = (half.w, half.h);
    let mut rgba = Vec::with_capacity((w * 2 * h * 4) as usize);
    for y in 0..h {
        for x in 0..w {
            rgba.extend_from_slice(&half.pixel(x, y));
        }
        for x in (0..w).rev() {
            rgba.extend_from_slice(&half.pixel(x, y));
        }
    }
    Image { w: w * 2, h, rgba }
}

pub fn build(store: &mut DataStore, name: &str, decomp: &Path, path_tm: &[[f32; 3]], frame: &Frame, assets: &AssetIndex, rom: &mut Rom, tag: &str) -> Result<Option<Moles>, String> {
    let spawns = spawns(decomp)?;
    // sprite frame 0 and the dirt
    let mole_loc = assets.locate("gTextureMole1").ok_or("gTextureMole1 not in the asset index")?;
    let tlut_loc = assets.locate("gTLUTMole").ok_or("gTLUTMole not in the asset index")?;
    let half = rom.texture(&mole_loc, Some(&tlut_loc))?;
    let sprite = full_sprite(&half);
    let dirt_loc = assets.locate("gTextureMooMooFarmDirt").ok_or("gTextureMooMooFarmDirt not in the asset index")?;
    let dirt = rom.texture(&dirt_loc, None)?.mirrored(true, true);
    // the template dyna part (the pusher's model bytes and entity params)
    let (_, tmpl) = static_item_from_pack_item_report(store, crate::itembox::TEMPLATE_ITEM, "Template.Item.Gbx", "Template", 1.0, crate::tm::STADIUM, 0)?;
    let part0 = tmpl.dyna.first().ok_or_else(|| format!("{}: no moving part in the template", crate::itembox::TEMPLATE_ITEM))?.clone();
    let (kc0, cparams) = part0.constraint.clone().ok_or("template part has no constraint")?;
    let px_m = SPRITE_UNITS_WIDE * frame.scale / sprite.w as f32;
    let rise_m = RISE_UNITS * frame.scale;
    let faces = relief(&sprite, px_m, DEPTH_M);
    let tris_total: usize = faces.values().map(|v| v.len()).sum();
    // the item's origin: the first mound, at ground level
    let origin = {
        let p = frame.to_tm(spawns[0]);
        [(p[0] * 100.0).round() / 100.0, (p[1] * 100.0).round() / 100.0, (p[2] * 100.0).round() / 100.0]
    };
    let mut merged = Merged::default();
    let unix = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
    merged.file_write_time = unix * 10_000_000 + 116444736000000000;
    let mut mounds = Merged::default();
    mounds.file_write_time = merged.file_write_time;
    let mut pictures: BTreeMap<String, Vec<u8>> = BTreeMap::new();
    let dirt_mat = Material { sym: "MoleDirt".into(), mirror_s: false, mirror_t: false, clamp_s: true, clamp_t: true, w: dirt.w, h: dirt.h, fmt: 0, tint: [255, 255, 255], tlut: None, additive: false };
    pictures.insert(format!("{tag}_{}.dds", dirt_mat.stem()), mapgeom::static_item::texture::write_dds_picture(dirt.w, dirt.h, &dirt.rgba));
    let dirt_slot = {
        mounds.materials.push(crate::tm::custom_material(&dirt_mat, false, crate::tm::PHYS_CONCRETE, tag));
        mounds.materials.len() - 1
    };
    let mut mound_tris: Vec<[Corner; 3]> = Vec::new();
    for (k, sp) in spawns.iter().enumerate() {
        let world = frame.to_tm(*sp);
        let pos = [world[0] - origin[0], world[1] - origin[1], world[2] - origin[2]];
        // face the karts: opposite to the travel direction at the nearest path point
        let yaw = {
            let (mut best, mut bi) = (f32::MAX, 0usize);
            for (i, p) in path_tm.iter().enumerate() {
                let d = (p[0] - world[0]).powi(2) + (p[2] - world[2]).powi(2);
                if d < best {
                    best = d;
                    bi = i;
                }
            }
            let a = path_tm[bi];
            let b = path_tm[(bi + 1) % path_tm.len()];
            let dir = [b[0] - a[0], b[2] - a[2]];
            // the figure faces local −z; yaw about y turns −z onto −dir
            (-dir[0]).atan2(-dir[1])
        };
        // --- the moving mole
        let mut kc = kc0.clone();
        kc.trans_axis = 1;
        kc.trans_min = -rise_m;
        kc.trans_max = 0.0;
        // up in 0.4 s, wave for a second or two, down in 0.4 s, hide for a
        // while — every mole on its own rhythm
        let hold_up = 800 + (k as u32 * 137) % 900;
        let hide = 1500 + (k as u32 * 331) % 2500;
        kc.trans.subs = vec![
            AnimSubFunc { ease: 3, reverse: 0, duration_ms: 400 },
            AnimSubFunc { ease: 0, reverse: 0, duration_ms: hold_up },
            AnimSubFunc { ease: 2, reverse: 1, duration_ms: 400 },
            AnimSubFunc { ease: 0, reverse: 1, duration_ms: hide },
        ];
        kc.rot_axis = 1;
        kc.angle_min_deg = 0.0;
        kc.angle_max_deg = 0.0;
        kc.rot.subs = vec![AnimSubFunc { ease: 1, reverse: 1, duration_ms: 8000 }];
        kc.shader_tc_type = 0;
        kc.shader_tc_anim.clear();
        kc.shader_tc_trans_sub = None;
        let mut mesh = Merged::default();
        mesh.file_write_time = merged.file_write_time;
        let mut per_material: Vec<Vec<[Corner; 3]>> = Vec::new();
        let mut slots: Vec<usize> = Vec::new();
        for (col, tris) in &faces {
            mesh.materials.push(plastic(*col, crate::tm::PHYS_CONCRETE));
            slots.push(mesh.materials.len() - 1);
            per_material.push(tris.clone());
        }
        let has_uv1 = vec![true; per_material.len()];
        bake::assign_lightmap_atlas(&mut per_material, &has_uv1);
        for (i, tris) in per_material.iter_mut().enumerate() {
            bake::tangents_vprim(tris, 0);
            for v in bake::make_visuals(tris, VisualLayout::Full, "range") {
                mesh.visuals.push(MergedVisual::every_level(v, slots[i]));
            }
        }
        let (s, c) = (yaw.sin(), yaw.cos());
        merged.dyna.push(DynaPart {
            path: format!("mk64:mole{k}"),
            // a yaw about +y as (x, y, z, w)
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
        let _ = (s, c);
        // --- the mound: a textured quad on the ground, 3 cm up
        let r = MOUND_UNITS * frame.scale / 2.0;
        let y = pos[1] + 0.03;
        let corner = |dx: f32, dz: f32| Corner { pos: [pos[0] + dx, y, pos[2] + dz], normal: [0.0, 1.0, 0.0], uv: [(dx / r + 1.0) / 2.0, (dz / r + 1.0) / 2.0], uv1: [(dx / r + 1.0) / 2.0, (dz / r + 1.0) / 2.0], tan_u: [1.0, 0.0, 0.0], tan_v: [0.0, 0.0, 1.0], face: k as u32 + 1, group: 0 };
        let (a, b, cc, d) = (corner(-r, -r), corner(r, -r), corner(r, r), corner(-r, r));
        let t1 = [a, b, cc];
        if mesh::face_normal(&[t1[0].pos, t1[1].pos, t1[2].pos])[1] > 0.0 {
            mound_tris.push(t1);
            mound_tris.push([a, cc, d]);
        } else {
            mound_tris.push([a, cc, b]);
            mound_tris.push([a, d, cc]);
        }
    }
    let mut pm = vec![mound_tris];
    bake::assign_lightmap_atlas(&mut pm, &[true]);
    bake::tangents_vprim(&mut pm[0], 0);
    for v in bake::make_visuals(&mut pm[0], VisualLayout::Full, "range") {
        mounds.visuals.push(MergedVisual::every_level(v, dirt_slot));
    }
    let opts = BuildOpts { ident: name.to_string(), author: name.to_string(), scale: 1.0, collection: crate::tm::STADIUM, skin: None };
    mapgeom::static_item::assemble::SIDECARS.with(|s| s.borrow_mut().clear());
    let f = assemble(&merged, &opts)?;
    for (n, b) in mapgeom::static_item::assemble::SIDECARS.with(|s| std::mem::take(&mut *s.borrow_mut())) {
        pictures.insert(n, b);
    }
    let mounds_name = name.replace("_moles.Item.Gbx", "_molemounds.Item.Gbx");
    let mopts = BuildOpts { ident: mounds_name.clone(), author: mounds_name, scale: 1.0, collection: crate::tm::STADIUM, skin: None };
    let mounds_bytes = write_file(&assemble(&mounds, &mopts)?);
    println!("  moles: {} at {:.1} units wide, {} colours, {} triangles each, rise {:.2} m; first three at {:?}", spawns.len(), SPRITE_UNITS_WIDE, faces.len(), tris_total, rise_m, spawns.iter().take(3).map(|sp| { let p = frame.to_tm(*sp); [(p[0] * 10.0).round() / 10.0, (p[1] * 10.0).round() / 10.0, (p[2] * 10.0).round() / 10.0] }).collect::<Vec<_>>());
    Ok(Some(Moles { bytes: write_file(&f), mounds: mounds_bytes, pos: origin, pictures, count: spawns.len() }))
}

/// The relief with a palette fitted to the image: its opaque colours clustered
/// (k-means, k ≤ 8) so any sprite — a penguin, a crab, a Boo — gets its own
/// handful of plastic slots.
pub fn relief_generic(img: &Image, px_m: f32, depth: f32) -> BTreeMap<[u8; 3], Vec<[Corner; 3]>> {
    let pal = palette_of(img, 6);
    let snapped = Image {
        w: img.w,
        h: img.h,
        rgba: img
            .rgba
            .chunks(4)
            .flat_map(|p| {
                if p[3] < 128 {
                    return [0u8, 0, 0, 0];
                }
                let c = nearest(&pal, [p[0], p[1], p[2]]);
                [c[0], c[1], c[2], 255]
            })
            .collect(),
    };
    // `relief` quantises through the mole palette; feed it an image already
    // on its own palette and bypass by identity: the snapped colours ARE the
    // slots — so run the same builder with the palette-snapped image and no
    // further quantisation
    relief_with(&snapped, px_m, depth, |c| c)
}

fn nearest(pal: &[[u8; 3]], rgb: [u8; 3]) -> [u8; 3] {
    let mut best = pal[0];
    let mut bd = i64::MAX;
    for p in pal {
        let d = (p[0] as i64 - rgb[0] as i64).pow(2) + (p[1] as i64 - rgb[1] as i64).pow(2) + (p[2] as i64 - rgb[2] as i64).pow(2);
        if d < bd {
            bd = d;
            best = *p;
        }
    }
    best
}

/// k-means over the opaque pixels (seeded on the most frequent colours).
pub fn palette_of(img: &Image, k: usize) -> Vec<[u8; 3]> {
    let mut count: BTreeMap<[u8; 3], usize> = BTreeMap::new();
    for p in img.rgba.chunks(4) {
        if p[3] >= 128 {
            *count.entry([p[0], p[1], p[2]]).or_default() += 1;
        }
    }
    if count.is_empty() {
        return vec![[128, 128, 128]];
    }
    let mut by_freq: Vec<([u8; 3], usize)> = count.iter().map(|(c, n)| (*c, *n)).collect();
    by_freq.sort_by(|a, b| b.1.cmp(&a.1));
    // seeds: frequent colours far enough from the ones already taken
    let mut cents: Vec<[f32; 3]> = Vec::new();
    for (c, _) in &by_freq {
        let cf = [c[0] as f32, c[1] as f32, c[2] as f32];
        if cents.iter().all(|s| (s[0] - cf[0]).powi(2) + (s[1] - cf[1]).powi(2) + (s[2] - cf[2]).powi(2) > 40.0f32.powi(2)) {
            cents.push(cf);
        }
        if cents.len() >= k {
            break;
        }
    }
    for _ in 0..12 {
        let mut acc = vec![([0.0f32; 3], 0usize); cents.len()];
        for (c, n) in &by_freq {
            let cf = [c[0] as f32, c[1] as f32, c[2] as f32];
            let (bi, _) = cents.iter().enumerate().map(|(i, s)| (i, (s[0] - cf[0]).powi(2) + (s[1] - cf[1]).powi(2) + (s[2] - cf[2]).powi(2))).min_by(|a, b| a.1.partial_cmp(&b.1).unwrap()).unwrap();
            for ch in 0..3 {
                acc[bi].0[ch] += cf[ch] * *n as f32;
            }
            acc[bi].1 += n;
        }
        for (i, (sum, n)) in acc.iter().enumerate() {
            if *n > 0 {
                cents[i] = [sum[0] / *n as f32, sum[1] / *n as f32, sum[2] / *n as f32];
            }
        }
    }
    cents.iter().map(|c| [c[0].round() as u8, c[1].round() as u8, c[2].round() as u8]).collect()
}
