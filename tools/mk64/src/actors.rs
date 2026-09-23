//! The course ACTORS the decomp spawns at fixed places — the trees, the
//! shrubs, the cacti: `spawn_foliage(d_course_<x>_tree_spawn)` with a model
//! drawn by a small display list of course_data.c over a full-format `Vtx`
//! array and textures the game DMAs into segment 3 (`init_actors_and_load_
//! textures`, actors.c). Item boxes and animated actors (balloons, boats,
//! trains) are not built.
//!
//! A tree in MK64 is a billboard that always faces the camera; a static item
//! cannot, so every billboard is placed as two crossed two-sided quads.

use crate::course::{Course, Gfx, Vertex};
use crate::mesh::{Corner, Frame, Material, Mesh, Tri};
use std::collections::HashMap;

/// The course-specific textures `init_actors_and_load_textures` DMAs into
/// segment 3, in order, with their allocation (0x800 or 0x400): the k-th one
/// sits at segment-3 offset 0x9000 + Σ sizes before it. (actors.c, US.)
pub fn segment3_textures(dir: &str) -> Vec<(&'static str, u32)> {
    let p = |n: usize| -> Vec<(&'static str, u32)> {
        const PIRANHA: [&str; 9] = [
            "gTexturePiranhaPlant1",
            "gTexturePiranhaPlant2",
            "gTexturePiranhaPlant3",
            "gTexturePiranhaPlant4",
            "gTexturePiranhaPlant5",
            "gTexturePiranhaPlant6",
            "gTexturePiranhaPlant7",
            "gTexturePiranhaPlant8",
            "gTexturePiranhaPlant9",
        ];
        PIRANHA[..n].iter().map(|s| (*s, 0x800)).collect()
    };
    match dir {
        "mario_raceway" => {
            let mut v = vec![("gTextureTrees1", 0x800)];
            v.extend(p(9));
            v
        }
        "bowsers_castle" => vec![("gTextureShrub", 0x800)],
        "yoshi_valley" => vec![("gTextureTrees2", 0x800)],
        "frappe_snowland" => vec![("gTextureFrappeSnowlandTreeLeft", 0x800), ("gTextureFrappeSnowlandTreeRight", 0x800)],
        "royal_raceway" => {
            let mut v = vec![("gTextureTrees3", 0x800), ("gTextureTrees7", 0x800)];
            v.extend(p(9));
            v
        }
        "luigi_raceway" => vec![("gTextureTrees5Left", 0x800), ("gTextureTrees5Right", 0x800)],
        "moo_moo_farm" => {
            let mut v = vec![("gTextureTrees4Left", 0x800), ("gTextureTrees4Right", 0x800)];
            for k in 1..=5 {
                v.push((Box::leak(format!("gTextureCow0{k}Left").into_boxed_str()), 0x800));
                v.push((Box::leak(format!("gTextureCow0{k}Right").into_boxed_str()), 0x800));
            }
            v
        }
        "kalimari_desert" => vec![
            ("gTextureCactus1Left", 0x800),
            ("gTextureCactus1Right", 0x800),
            ("gTextureCactus2Left", 0x800),
            ("gTextureCactus2Right", 0x800),
            ("gTextureCactus3", 0x800),
        ],
        "dks_jungle_parkway" => vec![
            ("gTextureDksJungleParkwayKiwanoFruit1", 0x400),
            ("gTextureDksJungleParkwayKiwanoFruit2", 0x400),
            ("gTextureDksJungleParkwayKiwanoFruit3", 0x400),
        ],
        _ => Vec::new(),
    }
}

/// The texture at a segment-3 address, by the DMA order.
pub fn segment3_symbol(dir: &str, addr: u32) -> Option<&'static str> {
    if addr >> 24 != 3 {
        return None;
    }
    let mut off = 0x9000u32;
    for (sym, size) in segment3_textures(dir) {
        if (addr & 0x00FF_FFFF) == off {
            return Some(sym);
        }
        off += size;
    }
    None
}

/// One textured triangle of an object model, in model space (units).
#[derive(Clone, Debug)]
pub struct ModelTri {
    pub c: [Vertex; 3],
    /// (texture symbol, texel w, h, clamp s, clamp t)
    pub tex: Option<(String, u32, u32, bool, bool)>,
}

/// The display lists of course_data.c that draw a named object model: the
/// ones loading vertices from a `Vtx` array whose name contains `what`.
pub fn model_lists(course: &Course, what: &str) -> Vec<String> {
    let mut out: Vec<String> = course
        .dls
        .iter()
        .filter(|(_, cmds)| cmds.iter().any(|g| matches!(g, Gfx::VertexSym { sym, .. } if sym.contains(what))))
        .map(|(n, _)| n.clone())
        .collect();
    out.sort();
    out
}

/// Walk one object display list: textures by segment-3 address.
pub fn model_triangles(course: &Course, dl: &str) -> Vec<ModelTri> {
    let mut out = Vec::new();
    let Some(cmds) = course.dls.get(dl) else { return out };
    let mut slots: [Option<Vertex>; 64] = [None; 64];
    let mut tex: Option<(String, u32, u32, bool, bool)> = None;
    for g in cmds {
        match g {
            Gfx::VertexSym { sym, n, v0 } => {
                if let Some(arr) = course.objects.get(sym) {
                    for k in 0..*n {
                        if v0 + k < slots.len() {
                            slots[v0 + k] = arr.get(k).copied();
                        }
                    }
                }
            }
            Gfx::LoadTextureBlock { addr, w, h, cms, cmt, .. } => {
                tex = segment3_symbol(&course.dir, *addr).map(|s| (s.to_string(), *w, *h, cms & 2 != 0, cmt & 2 != 0));
            }
            Gfx::TexImage { sym, .. } => {
                let resolved = course.texture_aliases.get(sym).cloned().unwrap_or_else(|| sym.clone());
                tex = Some((resolved, 32, 32, false, false));
            }
            Gfx::TileSize { tile: 0, uls, ult, lrs, lrt } => {
                if let Some(t) = tex.as_mut() {
                    t.1 = ((lrs - uls) >> 2) + 1;
                    t.2 = ((lrt - ult) >> 2) + 1;
                }
            }
            Gfx::Tri(s) => {
                if let (Some(a), Some(b), Some(c)) = (slots[s[0]], slots[s[1]], slots[s[2]]) {
                    out.push(ModelTri { c: [a, b, c], tex: tex.clone() });
                }
            }
            Gfx::Call(callee) => {
                out.extend(model_triangles(course, callee));
            }
            Gfx::End => break,
            _ => {}
        }
    }
    out
}

/// Is the model a flat card (all z ≈ 0)? Then it was a billboard in the game.
pub fn is_billboard(tris: &[ModelTri]) -> bool {
    tris.iter().all(|t| t.c.iter().all(|v| v.pos[2].abs() <= 1))
}

/// Place the course's trees: every spawn of `<x>_tree_spawn` gets the tree
/// model (crossed twice when it is a billboard). Appends to `mesh` under the
/// piece name `actors:tree`. Returns (spawns placed, triangles added).
pub fn add_trees(course: &Course, mesh: &mut Mesh, frame: &Frame) -> (usize, usize) {
    add_billboards(course, mesh, frame, "tree")
}

/// Every spawn of `<x>_<kind>_spawn` gets the model of the first display
/// list that loads a `Vtx` array named after `kind` (the cows' first
/// animation frame, the trees), crossed when it is a billboard.
pub fn add_billboards(course: &Course, mesh: &mut Mesh, frame: &Frame, kind: &str) -> (usize, usize) {
    let lists = model_lists(course, kind);
    let Some(first) = lists.first() else { return (0, 0) };
    // Koopa Troopa Beach draws a tree as `dl_tree_top1` + `dl_tree_trunk1`
    // (three variants): every list sharing the first one's trailing digit is
    // one model; other courses have one list (or animation frames: the first)
    let digit = first.chars().last().filter(|c| c.is_ascii_digit());
    let picked: Vec<&String> = match digit {
        Some(d) if lists.iter().filter(|l| l.ends_with(d)).count() > 1 => lists.iter().filter(|l| l.ends_with(d)).collect(),
        _ => vec![first],
    };
    let model: Vec<ModelTri> = picked.iter().flat_map(|dl| model_triangles(course, dl)).collect();
    if model.is_empty() {
        return (0, 0);
    }
    let billboard = is_billboard(&model);
    let suffix = format!("_{kind}_spawn");
    let spawns: Vec<[i16; 3]> = course.spawns.iter().filter(|(n, _)| n.ends_with(&suffix)).flat_map(|(_, v)| v.iter().map(|s| s.pos)).collect();
    if spawns.is_empty() {
        return (0, 0);
    }
    let piece = mesh.piece_names.len() as u32;
    mesh.piece_names.push(format!("actors:{kind}"));
    let mut mats: HashMap<(String, bool, bool), usize> = HashMap::new();
    let mut added = 0;
    for sp in &spawns {
        let yaws: &[f32] = if billboard { &[0.0, std::f32::consts::FRAC_PI_2] } else { &[0.0] };
        for &yaw in yaws {
            let (s, c) = (yaw.sin(), yaw.cos());
            for t in &model {
                let mat = t.tex.as_ref().map(|(sym, w, h, cs, ct)| {
                    *mats.entry((sym.clone(), *cs, *ct)).or_insert_with(|| {
                        mesh.materials.push(Material { sym: sym.clone(), mirror_s: false, mirror_t: false, clamp_s: *cs, clamp_t: *ct, w: *w, h: *h, fmt: 2, tint: [255, 255, 255] });
                        mesh.materials.len() - 1
                    })
                });
                let corner = |v: &Vertex| {
                    // model space → rotated about y → world units at the spawn → TM
                    let (x, y, z) = (v.pos[0] as f32, v.pos[1] as f32, v.pos[2] as f32);
                    let (rx, rz) = (c * x + s * z, -s * x + c * z);
                    let world = [sp[0] as f32 + rx, sp[1] as f32 + y, sp[2] as f32 + rz];
                    let (tw, th) = t.tex.as_ref().map(|x| (x.1 as f32, x.2 as f32)).unwrap_or((32.0, 32.0));
                    let mut u = v.tc[0] as f32 / 32.0 / tw;
                    let mut vv = v.tc[1] as f32 / 32.0 / th;
                    if t.tex.as_ref().map(|x| x.3).unwrap_or(false) {
                        u = u.clamp(0.0, 1.0);
                    }
                    if t.tex.as_ref().map(|x| x.4).unwrap_or(false) {
                        vv = vv.clamp(0.0, 1.0);
                    }
                    vv = 1.0 - vv; // the game samples v from the bottom row
                    Corner { pos: frame.to_tm_f(world), uv: [u, vv], rgba: [v.rgb[0], v.rgb[1], v.rgb[2], 255] }
                };
                let (a, b, cc) = (corner(&t.c[0]), corner(&t.c[1]), corner(&t.c[2]));
                let c3 = if frame.mirror { [a, cc, b] } else { [a, b, cc] };
                mesh.tris.push(Tri { c: c3, mat, two_sided: true, lit: false, piece });
                added += 1;
            }
        }
    }
    (spawns.len(), added)
}

/// Lakitu with the start light (`gTextureLakituBlueLight1`, the GO frame) as
/// a crossed two-sided billboard hovering over the start line: `at` is the
/// start point (units), `dir` the travel direction (unit vector, units frame).
/// The sprite is 56×72 texels drawn ~31×40 units — about three kart widths.
pub fn add_lakitu(mesh: &mut Mesh, frame: &Frame, at: [f32; 3], dir: [f32; 3]) -> usize {
    const SYM: &str = "gTextureLakituBlueLight1";
    const W: f32 = 31.0;
    const H: f32 = 40.0;
    const UP: f32 = 30.0;
    const AHEAD: f32 = 60.0;
    let centre = [at[0] + dir[0] * AHEAD, at[1] + UP, at[2] + dir[2] * AHEAD];
    let piece = mesh.piece_names.len() as u32;
    mesh.piece_names.push("actors:lakitu".to_string());
    mesh.materials.push(Material { sym: SYM.to_string(), mirror_s: false, mirror_t: false, clamp_s: true, clamp_t: true, w: 56, h: 72, fmt: 2, tint: [255, 255, 255] });
    let mat = Some(mesh.materials.len() - 1);
    // the first quad faces the travel direction (the drivers see him), the
    // second is perpendicular
    let right = [-dir[2], 0.0, dir[0]];
    let mut n = 0;
    for axis in [right, dir] {
        let corner = |su: f32, sv: f32| {
            let world = [centre[0] + axis[0] * su * W / 2.0, centre[1] + sv * H / 2.0, centre[2] + axis[2] * su * W / 2.0];
            // u left→right, v: the game samples bottom-up (sv = −1 is the sprite's bottom row)
            Corner { pos: frame.to_tm_f(world), uv: [(su + 1.0) / 2.0, (sv + 1.0) / 2.0], rgba: [255, 255, 255, 255] }
        };
        let (a, b, c, d) = (corner(-1.0, -1.0), corner(1.0, -1.0), corner(1.0, 1.0), corner(-1.0, 1.0));
        for tri in [[a, b, c], [a, c, d]] {
            let c3 = if frame.mirror { [tri[0], tri[2], tri[1]] } else { tri };
            mesh.tris.push(Tri { c: c3, mat, two_sided: true, lit: false, piece });
            n += 1;
        }
    }
    n
}
