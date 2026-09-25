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
    /// Drawn with lighting on (`gsSPSetLights1`/`gsSPNumLights`, or the geometry
    /// mode): the vertex "colours" are normals — the model is white.
    pub lit: bool,
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
    model_triangles_with(course, dl, None)
}

/// Same, with the texture state the CODE set before drawing (the Thwomp's
/// face quad is drawn before any texture command of its own list).
pub fn model_triangles_with(course: &Course, dl: &str, initial: Option<(String, u32, u32, bool, bool)>) -> Vec<ModelTri> {
    let mut out = Vec::new();
    let Some(cmds) = course.dls.get(dl) else { return out };
    let mut slots: [Option<Vertex>; 64] = [None; 64];
    let mut tex: Option<(String, u32, u32, bool, bool)> = initial;
    let mut lit = false;
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
                let resolved = match sym.strip_prefix('@') {
                    Some(hex) => match u32::from_str_radix(hex.trim_start_matches("0x"), 16).ok().and_then(|a| segment3_symbol(&course.dir, a)) {
                        Some(s) => s.to_string(),
                        None => sym.clone(),
                    },
                    None => course.texture_aliases.get(sym).cloned().unwrap_or_else(|| sym.clone()),
                };
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
                    out.push(ModelTri { c: [a, b, c], tex: tex.clone(), lit });
                }
            }
            Gfx::GeomSet(bits) if bits & crate::mesh::G_LIGHTING != 0 => lit = true,
            Gfx::GeomClear(bits) if bits & crate::mesh::G_LIGHTING != 0 => lit = false,
            Gfx::Other(name) if name.contains("Lights") => lit = true,
            Gfx::Call(callee) => {
                out.extend(model_triangles_with(course, callee, tex.clone()));
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
    // a course whose model is not named after the kind (Bowser's "trees" are
    // the bushes: one textured triangle over `unknown_model`)
    let mut lists = model_lists(course, kind);
    if lists.is_empty() {
        let alt = match (course.dir.as_str(), kind) {
            ("bowsers_castle", "tree") => Some("d_course_bowsers_castle_dl_bush"),
            _ => None,
        };
        if let Some(a) = alt {
            if course.dls.contains_key(a) {
                lists.push(a.to_string());
            }
        }
    }
    let Some(first) = lists.first() else { return (0, 0) };
    // Koopa Troopa Beach draws a tree as `dl_tree_top1` + `dl_tree_trunk1`
    // (three variants): every list sharing the first one's trailing digit is
    // one model; other courses have one list (or animation frames: the first)
    let digit = first.chars().last().filter(|c| c.is_ascii_digit());
    let picked: Vec<&String> = match digit {
        Some(d) if lists.iter().filter(|l| l.ends_with(d)).count() > 1 => lists.iter().filter(|l| l.ends_with(d)).collect(),
        _ => vec![first],
    };
    // a model whose display list loads only the PALETTE and leaves the texture
    // image to the actor code (the piranha plants: `dma_textures(gTexture
    // PiranhaPlant1…)`, the frame picked per tick) gets its first frame here —
    // untextured, they were pure white crossed panels (vjeux, 2026-09-25)
    let initial: Option<(String, u32, u32, bool, bool)> = match kind {
        "piranha_plant" => Some(("gTexturePiranhaPlant1".to_string(), 32, 64, true, false)),
        _ => None,
    };
    let default_model: Vec<ModelTri> = picked.iter().flat_map(|dl| model_triangles_with(course, dl, initial.clone())).collect();
    if default_model.is_empty() {
        return (0, 0);
    }
    let suffix = format!("_{kind}_spawn");
    let suffix_s = format!("_{kind}_spawns");
    let spawns: Vec<([i16; 3], i16)> = course.spawns.iter().filter(|(n, _)| n.ends_with(&suffix) || n.ends_with(&suffix_s)).flat_map(|(_, v)| v.iter().map(|s| (s.pos, s.id))).collect();
    // per-spawn variants (the spawn id's low nibble picks the model: DK's
    // jungle draws tree1/2/3 and the palm by it, actors.c)
    let variant = |id: i16| -> Option<&str> {
        match (course.dir.as_str(), kind, id & 0xF) {
            ("dks_jungle_parkway", "tree", 0) => Some("d_course_dks_jungle_parkway_dl_tree1"),
            ("dks_jungle_parkway", "tree", 4) => Some("d_course_dks_jungle_parkway_dl_tree2"),
            ("dks_jungle_parkway", "tree", 5) => Some("d_course_dks_jungle_parkway_dl_tree3"),
            ("dks_jungle_parkway", "tree", 6) => Some("d_course_dks_jungle_parkway_dl_palm_tree"),
            _ => None,
        }
    };
    let mut variant_models: HashMap<String, Vec<ModelTri>> = HashMap::new();
    if spawns.is_empty() {
        return (0, 0);
    }
    let piece = mesh.piece_names.len() as u32;
    mesh.piece_names.push(format!("actors:{kind}"));
    let mut mats: HashMap<(String, bool, bool), usize> = HashMap::new();
    let mut added = 0;
    for (sp, id) in &spawns {
        let model: &Vec<ModelTri> = match variant(*id) {
            Some(dl) if course.dls.contains_key(dl) => variant_models.entry(dl.to_string()).or_insert_with(|| model_triangles(course, dl)),
            _ => &default_model,
        };
        if model.is_empty() {
            continue;
        }
        let billboard = is_billboard(model);
        let yaws: &[f32] = if billboard { &[0.0, std::f32::consts::FRAC_PI_2] } else { &[0.0] };
        for &yaw in yaws {
            let (s, c) = (yaw.sin(), yaw.cos());
            for t in model {
                // the piranha plant is stored as its LEFT half, mirrored across
                // the quad by the tile (`G_TX_MIRROR` on s), like the Thwomp face
                let mirror_s = t.tex.as_ref().map(|x| x.0.contains("PiranhaPlant")).unwrap_or(false);
                let mat = t.tex.as_ref().map(|(sym, w, h, cs, ct)| {
                    *mats.entry((sym.clone(), *cs, *ct)).or_insert_with(|| {
                        mesh.materials.push(Material { sym: sym.clone(), mirror_s, mirror_t: false, clamp_s: *cs, clamp_t: *ct, w: *w, h: *h, fmt: 2, tint: [255, 255, 255] });
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
                    if mirror_s {
                        u /= 2.0; // the mirrored image is twice as wide
                    }
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
                mesh.tris.push(Tri { c: c3, mat, two_sided: true, lit: t.lit, piece });
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

/// Bowser's Castle Thwomps (150cc list, some_data.c `gThomwpSpawns150CC`:
/// x, z in units; they slam onto the road in the game). Static here — a
/// moving part cannot show its face texture — hovering `clearance_m` above
/// the road so a car passes under. `floor_tm(x, z)` gives the road height
/// (TM frame) under a TM point. Returns the triangle count.
pub const THWOMPS_150CC: [(i16, i16); 12] = [(0x0320, -1750), (0x044c, -1750), (0x02bc, -1700), (0x04b0, -1800), (0x04b0, -2630), (0x04b0, -2670), (0x091a, -2615), (0x091a, -2645), (0x091a, -2675), (0x0596, -1745), (0x082a, -1550), (0x073a, -1550)];

pub fn add_thwomps(course: &Course, mesh: &mut Mesh, frame: &Frame, floor_tm: &dyn Fn(f32, f32) -> Option<f32>, clearance_m: f32) -> usize {
    let dl = "d_course_bowsers_castle_dl_thwomp";
    if !course.dls.contains_key(dl) {
        return 0;
    }
    // the face: a 16×64 half-face MIRRORED across the quad (s runs 0..32 texels)
    let face = Some(("gTextureThwompFace1".to_string(), 16, 64, false, true));
    let model = model_triangles_with(course, dl, face);
    if model.is_empty() {
        return 0;
    }
    let min_y = model.iter().flat_map(|t| t.c.iter().map(|v| v.pos[1] as f32)).fold(f32::MAX, f32::min);
    let piece = mesh.piece_names.len() as u32;
    mesh.piece_names.push("actors:thwomp".to_string());
    let mut mats: HashMap<(String, bool, bool), usize> = HashMap::new();
    let mut n = 0;
    for (x, z) in THWOMPS_150CC {
        let (xu, zu) = (if frame.mirror { -(x as f32) } else { x as f32 }, z as f32);
        let p_tm = frame.to_tm_f([xu, 0.0, zu]);
        let Some(floor) = floor_tm(p_tm[0], p_tm[2]) else { continue };
        // bottom of the model at floor + clearance, in units
        let y_units = (floor + clearance_m - frame.offset[1]) / frame.scale - min_y;
        for t in &model {
            let mat = t.tex.as_ref().map(|(sym, w, h, cs, ct)| {
                *mats.entry((sym.clone(), *cs, *ct)).or_insert_with(|| {
                    let mirror_s = sym.contains("ThwompFace");
                    mesh.materials.push(Material { sym: sym.clone(), mirror_s, mirror_t: false, clamp_s: *cs, clamp_t: *ct, w: *w, h: *h, fmt: 2, tint: [255, 255, 255] });
                    mesh.materials.len() - 1
                })
            });
            let mirror_s = t.tex.as_ref().map(|x| x.0.contains("ThwompFace")).unwrap_or(false);
            let corner = |v: &Vertex| {
                let world = [xu + v.pos[0] as f32, y_units + v.pos[1] as f32, zu + v.pos[2] as f32];
                let (tw, th) = t.tex.as_ref().map(|x| (x.1 as f32, x.2 as f32)).unwrap_or((32.0, 32.0));
                let mut u = v.tc[0] as f32 / 32.0 / tw;
                if mirror_s {
                    u /= 2.0; // the mirrored image is twice as wide
                }
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
            // the model is LIT (gsSPNumLights): its vertex "colours" are normals
            mesh.tris.push(Tri { c: c3, mat, two_sided: false, lit: true || t.lit, piece });
            n += 1;
        }
    }
    n
}
