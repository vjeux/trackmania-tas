//! The Trackmania side: course pieces → embedded static items (Solid2 visuals
//! with the course's own textures, a collision surface with MK64 physics) and
//! a map that places them with a multilap start/finish and checkpoints.
//!
//! Every display-list piece becomes one item (its collision, when the
//! `TrackSections` table lists the same piece, rides in it); the start line
//! and the checkpoints are attached to the road pieces under them, so no item
//! is ever without a visual. Items and their `.dds` textures are embedded in
//! the map (`Items/<name>` — the custom-texture material resolves a bare file
//! name in the item's own folder).

use crate::course::{self, Course};
use crate::mesh::{self, Frame, Mesh};
use crate::texture::{AssetIndex, Image, Rom};
use mapgeom::crystal_model::{CPlugMaterialUserInst, Id, UserTexture};
use mapgeom::static_item::assemble::{assemble, BuildOpts};
use mapgeom::static_item::bake::{self, Corner, VisualLayout};
use mapgeom::static_item::merged::{Merged, MergedVisual};
use mapgeom::static_item::surface::{CPlugSurface, Triangle};
use mapgeom::static_item::texture::write_dds_picture;
use mapgeom::static_item::write_file;
use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};
use tmmaps::map::MapFile;

/// Stadium collection id.
pub const STADIUM: u32 = 26;
/// World y of the Stadium grass (cell 9 floor 8 m + the 2 m deck); the
/// lowest collision point of a course sits just above it.
pub const STADIUM_GROUND_Y: f32 = 10.0;
/// TM2020 physics ids (mapgeom `MATERIAL_PHYSICS`: Asphalt 16, Green 76, Dirt 6,
/// Ice 74, Snow 21, Metal 4; the enum: Concrete 0, Sand 5).
pub const PHYS_CONCRETE: u8 = 0;
pub const PHYS_METAL: u8 = 4;
pub const PHYS_SAND: u8 = 5;
pub const PHYS_DIRT: u8 = 6;
pub const PHYS_ASPHALT: u8 = 16;
pub const PHYS_SNOW: u8 = 21;
pub const PHYS_ICE: u8 = 74;
pub const PHYS_GRASS: u8 = 76;
/// Gameplay ids: Turbo 1.
pub const GAMEPLAY_TURBO: u8 = 1;

/// MK64 surface type → (TM physics, gameplay). None = no collision.
pub fn physics_for_surface(s: u8) -> Option<(u8, u8)> {
    Some(match s {
        0 => return None,                       // AIRBORNE
        1 => (PHYS_ASPHALT, 0),                 // ASPHALT
        2 | 13 => (PHYS_DIRT, 0),               // DIRT, DIRT_OFFROAD
        3 | 7 | 10 => (PHYS_SAND, 0),           // SAND, SAND_OFFROAD, WET_SAND
        4 | 12 | 15 => (PHYS_CONCRETE, 0),      // STONE, CLIFF, CAVE
        5 | 11 => (PHYS_SNOW, 0),               // SNOW, SNOW_OFFROAD
        6 | 16 | 17 => (PHYS_CONCRETE, 0),      // BRIDGE, ROPE_BRIDGE, WOOD_BRIDGE
        8 => (PHYS_GRASS, 0),                   // GRASS
        9 => (PHYS_ICE, 0),                     // ICE
        14 => (PHYS_METAL, 0),                  // TRAIN_TRACK
        0xFC | 0xFE => (PHYS_ASPHALT, GAMEPLAY_TURBO), // BOOST_RAMP_WOOD / _ASPHALT
        0xFD => (PHYS_GRASS, 0),                // OUT_OF_BOUNDS: slow ground for now
        0xFF => (PHYS_CONCRETE, 0),             // RAMP (the walls in Luigi Raceway)
        _ => (PHYS_CONCRETE, 0),
    })
}

/// Texture upscale before DXT (nearest): at ×4 every 4×4 DXT block is ONE
/// texel, so the compression is exact and the N64 pixels stay crisp.
pub const TEXTURE_UPSCALE: u32 = 4;

fn flag<'a>(args: &'a [String], name: &str) -> Option<&'a str> {
    args.iter().position(|a| a == name).and_then(|i| args.get(i + 1)).map(|s| s.as_str())
}

/// One item to embed and place.
pub struct ItemSpec {
    pub name: String,
    pub bytes: Vec<u8>,
    pub pos: [f32; 3],
    pub yaw: f32,
    pub tag: Option<String>,
    pub order: u32,
}

/// A waypoint to attach to the piece under a path point.
struct WaypointReq {
    /// TM world position on the path.
    pos: [f32; 3],
    /// Travel direction (unit, xz).
    dir: [f32; 3],
    /// 4 = start+finish, 2 = checkpoint.
    kind: i32,
    tag: &'static str,
}

pub fn cmd_build(args: &[String]) {
    let usage = "mk64 build COURSE --host HOST.Map.Gbx --out OUT.Map.Gbx [--decomp DIR] [--rom FILE]
      [--scale M_PER_UNIT] [--mirror] [--name NAME] [--laps N] [--cps N] [--stadium] [--skirt|--no-skirt] [--no-vertex-colours] [--no-actors] [--author-ms MS]
      [--items-out DIR]  (also write every item + texture as loose files)";
    let dir = match args.get(2) {
        Some(c) if !c.starts_with("--") => c.clone(),
        _ => {
            eprintln!("{usage}");
            std::process::exit(2);
        }
    };
    let decomp = PathBuf::from(flag(args, "--decomp").map(String::from).or_else(|| std::env::var("MK64_DECOMP").ok()).expect("--decomp DIR or $MK64_DECOMP"));
    let rom = PathBuf::from(flag(args, "--rom").map(String::from).or_else(|| std::env::var("MK64_ROM").ok()).expect("--rom FILE or $MK64_ROM"));
    let host = PathBuf::from(flag(args, "--host").unwrap_or_else(|| {
        eprintln!("{usage}");
        std::process::exit(2)
    }));
    let out = PathBuf::from(flag(args, "--out").unwrap_or_else(|| {
        eprintln!("{usage}");
        std::process::exit(2)
    }));
    let laps: u32 = flag(args, "--laps").map(|s| s.parse().expect("--laps N")).unwrap_or(3);
    let n_cps: usize = flag(args, "--cps").map(|s| s.parse().expect("--cps N")).unwrap_or(4);
    let name = flag(args, "--name").map(String::from).unwrap_or_else(|| format!("MK64 {}", course::course_title(&dir)));
    let no_stadium = !args.iter().any(|a| a == "--stadium");
    let items_out = flag(args, "--items-out").map(PathBuf::from);

    let mut c = Course::load(&decomp, &dir).unwrap_or_else(|e| {
        eprintln!("{dir}: {e}");
        std::process::exit(1)
    });
    let assets = AssetIndex::load(&decomp).expect("asset index");
    let mut rom = Rom::load(&rom).expect("ROM");
    let pieces = c.visual_pieces();
    let coll = c.collision_pieces();

    // frame: scale from the flag or the calibrated constant, offset so the
    // course is centred in the map with its lowest collision point on the grass
    let scale: f32 = flag(args, "--scale").map(|s| s.parse().expect("--scale")).unwrap_or(crate::mesh::UNITS_TO_M);
    let mirror = args.iter().any(|a| a == "--mirror");
    let f0 = Frame { scale, mirror, offset: [0.0; 3] };
    let soup0 = mesh::collision_mesh(&c, &coll, &f0);
    let mesh0 = mesh::visual_mesh(&c, &pieces, Some(&assets), &f0);
    let all_pts = mesh0.tris.iter().flat_map(|t| t.c.iter().map(|k| k.pos));
    let (lo_v, hi_v) = mesh::bbox(all_pts).expect("course has geometry");
    // the lowest DRIVABLE point (upward-facing collision; the invisible walls
    // reach further down) clears the grass
    let min_coll_y = soup0.iter().filter(|t| mesh::face_normal(&t.p)[1] > 0.5).flat_map(|t| t.p.iter().map(|p| p[1])).fold(f32::INFINITY, f32::min);
    let min_y = if min_coll_y.is_finite() { min_coll_y } else { lo_v[1] };
    // the host decides the map centre (a 48-cell Stadium map centres at 768,
    // the 128³ void base at 2048) and whether the course needs a skirt
    let kind = host_kind(&MapFile::load(&host));
    let centre = [kind.size[0] as f32 * 16.0, kind.size[2] as f32 * 16.0];
    let offset = [centre[0] - (lo_v[0] + hi_v[0]) / 2.0, STADIUM_GROUND_Y + 0.3 - min_y, centre[1] - (lo_v[2] + hi_v[2]) / 2.0];
    let frame = Frame { scale, mirror, offset };
    let mut m = mesh::visual_mesh(&c, &pieces, Some(&assets), &frame);
    if !args.iter().any(|a| a == "--no-actors") {
        for kind in ["tree", "cow"] {
            let (n, t) = crate::actors::add_billboards(&c, &mut m, &frame, kind);
            if n > 0 {
                println!("  actors: {n} {kind}s ({t} triangles)");
            }
        }
    }
    let (splits, variants) = if args.iter().any(|a| a == "--no-vertex-colours") { (0, m.materials.len()) } else { mesh::bake_vertex_colours(&mut m, 16, 24, 4) };
    println!("  vertex colours baked: {splits} triangle splits, {variants} texture variants");
    let want_skirt = if args.iter().any(|a| a == "--no-skirt") { false } else if args.iter().any(|a| a == "--skirt") { true } else { !kind.void };
    let skirts = if want_skirt { mesh::add_skirt(&mut m, STADIUM_GROUND_Y - 0.2) } else { 0 };
    let soup = mesh::collision_mesh(&c, &coll, &frame);
    println!(
        "{}: {} visual tris in {} pieces ({skirts} skirt quads), {} collision tris; scale {scale:.5} m/unit, offset ({:.1}, {:.1}, {:.1}); extent x {:.0}..{:.0} z {:.0}..{:.0}",
        dir,
        m.tris.len(),
        pieces.len(),
        soup.len(),
        offset[0],
        offset[1],
        offset[2],
        lo_v[0] + offset[0],
        hi_v[0] + offset[0],
        lo_v[2] + offset[2],
        hi_v[2] + offset[2]
    );

    // textures → DDS pictures (one per material)
    let (images, missing) = material_images(&m, &assets, &mut rom);
    let alpha: Vec<bool> = (0..m.materials.len()).map(|i| images[&i].has_alpha()).collect();
    for x in &missing {
        println!("  texture missing: {x}");
    }
    let mut pictures: BTreeMap<String, Vec<u8>> = BTreeMap::new();
    for (i, mat) in m.materials.iter().enumerate() {
        let img = images[&i].upscale(TEXTURE_UPSCALE);
        pictures.insert(format!("{}.dds", mat.stem()), write_dds_picture(img.w, img.h, &img.rgba));
    }

    // waypoints along the centre path
    let path: Vec<[f32; 3]> = c.path.iter().map(|p| frame.to_tm(p.pos)).collect();
    let mut reqs: Vec<WaypointReq> = Vec::new();
    let n = path.len();
    let dir_at = |i: usize| -> [f32; 3] {
        let a = path[i];
        let b = path[(i + 1) % n];
        mesh::normalize([b[0] - a[0], 0.0, b[2] - a[2]])
    };
    reqs.push(WaypointReq { pos: path[0], dir: dir_at(0), kind: 4, tag: "StartFinish" });
    for k in 1..=n_cps {
        let i = (k * n) / (n_cps + 1);
        reqs.push(WaypointReq { pos: path[i], dir: dir_at(i), kind: 2, tag: "Checkpoint" });
    }

    // items: one per visual piece; collision by matching piece name
    // collision triangles carry their section index through `section_id`
    // only; rebuild per dl from the (section, piece) pairs directly
    let mut coll_tris_by_dl: HashMap<String, Vec<mesh::CollTri>> = HashMap::new();
    {
        let mut k = 0usize;
        for (sec, piece) in &coll {
            let n = piece.tris.len();
            coll_tris_by_dl.entry(sec.dl.clone()).or_default().extend_from_slice(&soup[k..k + n]);
            k += n;
        }
    }
    let piece_index: HashMap<&str, usize> = m.piece_names.iter().enumerate().map(|(i, n)| (n.as_str(), i)).collect();
    let mut tris_by_piece: Vec<Vec<&mesh::Tri>> = vec![Vec::new(); m.piece_names.len()];
    for t in &m.tris {
        tris_by_piece[t.piece as usize].push(t);
    }
    // which piece a waypoint attaches to: the collision piece whose
    // triangles pass under the path point, else the nearest visual piece
    let mut attach: HashMap<usize, Vec<&WaypointReq>> = HashMap::new();
    let mut unattached = Vec::new();
    for r in &reqs {
        let mut best: Option<(f32, usize)> = None;
        for (dl, tris) in &coll_tris_by_dl {
            let Some(&pi) = piece_index.get(dl.as_str()) else { continue };
            for t in tris {
                if let Some(y) = height_under(t, r.pos[0], r.pos[2]) {
                    let d = (r.pos[1] - y).abs();
                    if d < 6.0 && best.map(|b| d < b.0).unwrap_or(true) {
                        best = Some((d, pi));
                    }
                }
            }
        }
        match best {
            Some((_, pi)) => attach.entry(pi).or_default().push(r),
            None => unattached.push(r),
        }
    }
    for r in &unattached {
        println!("  WARNING: no collision piece under {} at ({:.1}, {:.1}, {:.1}) — waypoint skipped", r.tag, r.pos[0], r.pos[1], r.pos[2]);
    }

    let mut specs: Vec<ItemSpec> = Vec::new();
    let mut coll_used: std::collections::HashSet<String> = Default::default();
    let mut stats_items = 0usize;
    for (pi, pname) in m.piece_names.iter().enumerate() {
        let tris = &tris_by_piece[pi];
        if tris.is_empty() {
            continue;
        }
        let ctris: Vec<mesh::CollTri> = coll_tris_by_dl.get(pname).cloned().unwrap_or_default();
        if !ctris.is_empty() {
            coll_used.insert(pname.clone());
        }
        let wps: Vec<&WaypointReq> = attach.get(&pi).cloned().unwrap_or_default();
        // a piece can carry one waypoint; extra ones go unplaced
        if wps.len() > 1 {
            println!("  WARNING: piece {pname} would carry {} waypoints; keeping the first", wps.len());
        }
        let wp = wps.first().copied();
        let name = format!("MK64_{}_{:03}.Item.Gbx", dir, stats_items);
        match build_item(&name, tris, &ctris, &m, &alpha, wp) {
            Ok((bytes, pos, yaw)) => {
                specs.push(ItemSpec { name, bytes, pos, yaw, tag: wp.map(|w| w.tag.to_string()), order: 0 });
                stats_items += 1;
            }
            Err(e) => println!("  piece {pname}: item build failed: {e}"),
        }
    }
    // collision pieces never drawn: collision-only items
    for (dl, ctris) in &coll_tris_by_dl {
        if coll_used.contains(dl) || ctris.is_empty() {
            continue;
        }
        let name = format!("MK64_{}_{:03}.Item.Gbx", dir, stats_items);
        match build_item(&name, &[], ctris, &m, &alpha, None) {
            Ok((bytes, pos, yaw)) => {
                specs.push(ItemSpec { name, bytes, pos, yaw, tag: None, order: 0 });
                stats_items += 1;
            }
            Err(e) => println!("  collision piece {dl}: item build failed: {e}"),
        }
    }
    let n_wp = specs.iter().filter(|s| s.tag.is_some()).count();
    println!("  {} items ({} with waypoints: {}), {} textures", specs.len(), n_wp, specs.iter().filter_map(|s| s.tag.as_deref()).collect::<Vec<_>>().join(","), pictures.len());
    if !specs.iter().any(|s| s.tag.as_deref() == Some("StartFinish")) {
        eprintln!("no start/finish could be attached — refusing to write a map without a spawn");
        std::process::exit(1);
    }

    if let Some(d) = &items_out {
        std::fs::create_dir_all(d).expect("--items-out");
        for s in &specs {
            std::fs::write(d.join(&s.name), &s.bytes).expect("write item");
        }
        for (n, b) in &pictures {
            std::fs::write(d.join(n), b).expect("write dds");
        }
    }

    // medals: --author-ms, else an estimate (the lap at 150 km/h × laps); gold/silver/bronze = ×1.06/1.2/1.5 to the second
    let lap_m = c.path_length() as f32 * scale;
    let author_ms: u32 = flag(args, "--author-ms").map(|s| s.parse().expect("--author-ms N")).unwrap_or(((lap_m / 41.7) * laps as f32 * 1000.0) as u32);
    let medal = |f: f32| ((author_ms as f32 * f / 1000.0).ceil() * 1000.0) as u32;
    let times = [author_ms, medal(1.06), medal(1.2), medal(1.5)];
    println!("  medals (ms): author {} gold {} silver {} bronze {}{}", times[0], times[1], times[2], times[3], if flag(args, "--author-ms").is_some() { "" } else { " (estimate: 150 km/h average)" });
    write_map(&host, &out, &specs, &pictures, &name, laps, no_stadium, &dir, times);
}

/// y of the triangle's plane at (x, z) when the point is inside it (top view).
fn height_under(t: &mesh::CollTri, x: f32, z: f32) -> Option<f32> {
    let p = &t.p;
    let e = |a: [f32; 3], b: [f32; 3]| (b[0] - a[0]) * (z - a[2]) - (b[2] - a[2]) * (x - a[0]);
    let area = e(p[0], p[1]) + e(p[1], p[2]) + e(p[2], p[0]);
    // barycentric via sub-areas
    let w0 = e(p[1], p[2]);
    let w1 = e(p[2], p[0]);
    let w2 = e(p[0], p[1]);
    let total = w0 + w1 + w2;
    if total.abs() < 1e-9 {
        return None;
    }
    let _ = area;
    let (u, v, w) = (w0 / total, w1 / total, w2 / total);
    if u < -1e-4 || v < -1e-4 || w < -1e-4 {
        return None;
    }
    Some(u * p[0][1] + v * p[1][1] + w * p[2][1])
}

/// The item's local frame: origin at the piece's footprint centre and lowest
/// point, yaw = the waypoint's travel direction (a start item's spawn faces
/// the item's own +z), local = R(−yaw)·(world − origin).
fn local_frame(pts: impl Iterator<Item = [f32; 3]>, wp: Option<&WaypointReq>) -> ([f32; 3], f32) {
    let (lo, hi) = mesh::bbox(pts).unwrap_or(([0.0; 3], [0.0; 3]));
    let snap = |v: f32| (v * 100.0).round() / 100.0;
    let origin = match wp {
        Some(w) if w.kind == 4 => [snap(w.pos[0]), snap(lo[1]), snap(w.pos[2])],
        _ => [snap((lo[0] + hi[0]) / 2.0), snap(lo[1]), snap((lo[2] + hi[2]) / 2.0)],
    };
    // the game spawns the car facing the item's local −z (Luigi Raceway,
    // 2026-09-22: yaw = atan2(dx, dz) put the car backwards on the straight)
    let yaw = match wp {
        Some(w) if w.kind == 4 => {
            let y = w.dir[0].atan2(w.dir[2]) + std::f32::consts::PI;
            if y > std::f32::consts::PI { y - 2.0 * std::f32::consts::PI } else { y }
        }
        _ => 0.0,
    };
    (origin, yaw)
}

fn to_local(p: [f32; 3], origin: [f32; 3], yaw: f32) -> [f32; 3] {
    let d = [p[0] - origin[0], p[1] - origin[1], p[2] - origin[2]];
    // rotate by −yaw about y: yaw maps local +z → world (sin yaw, 0, cos yaw)
    let (s, c) = (yaw.sin(), yaw.cos());
    [c * d[0] - s * d[2], d[1], s * d[0] + c * d[2]]
}

/// One static item from a piece's visual triangles and its collision.
/// Returns the item bytes and its placement (position, yaw).
fn build_item(name: &str, tris: &[&mesh::Tri], ctris: &[mesh::CollTri], m: &Mesh, alpha: &[bool], wp: Option<&WaypointReq>) -> Result<(Vec<u8>, [f32; 3], f32), String> {
    let pts = tris.iter().flat_map(|t| t.c.iter().map(|c| c.pos)).chain(ctris.iter().flat_map(|t| t.p.iter().copied()));
    let (origin, yaw) = local_frame(pts, wp);
    let mut merged = Merged::default();
    let unix = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
    merged.file_write_time = unix * 10_000_000 + 116444736000000000;

    // materials: one custom-texture instance per mesh material used here
    let mut slot_of: HashMap<Option<usize>, usize> = HashMap::new();
    let mut per_material: Vec<Vec<[Corner; 3]>> = Vec::new();
    let mut slots: Vec<usize> = Vec::new();
    for t in tris {
        let key = t.mat;
        let mi = *slot_of.entry(key).or_insert_with(|| {
            let inst = match key {
                Some(k) => custom_material(&m.materials[k], alpha[k], PHYS_ASPHALT),
                None => flat_material(),
            };
            merged.materials.push(inst);
            slots.push(merged.materials.len() - 1);
            per_material.push(Vec::new());
            per_material.len() - 1
        });
        let corner = |k: &mesh::Corner, n: [f32; 3]| Corner { pos: to_local(k.pos, origin, yaw), normal: n, uv: k.uv, uv1: k.uv, tan_u: [0.0; 3], tan_v: [0.0; 3], face: 0, group: 0 };
        let p = [to_local(t.c[0].pos, origin, yaw), to_local(t.c[1].pos, origin, yaw), to_local(t.c[2].pos, origin, yaw)];
        let n = mesh::face_normal(&p);
        let tri = [corner(&t.c[0], n), corner(&t.c[1], n), corner(&t.c[2], n)];
        per_material[mi].push(tri);
        if t.two_sided {
            let nn = [-n[0], -n[1], -n[2]];
            per_material[mi].push([corner(&t.c[0], nn), corner(&t.c[2], nn), corner(&t.c[1], nn)]);
        }
    }
    // face ids for the smoothing (one vote per triangle)
    for tris in per_material.iter_mut() {
        for (i, t) in tris.iter_mut().enumerate() {
            for c in t.iter_mut() {
                c.face = i as u32 + 1;
            }
        }
    }
    // a collision-only piece (MK64's invisible boundary walls) still needs
    // a visual to be an item at all: one millimetre of nothing, 4 m under
    // the origin, like the no-collision placeholder surface
    if per_material.is_empty() {
        merged.materials.push(flat_material());
        slots.push(merged.materials.len() - 1);
        let n = [0.0, 1.0, 0.0];
        let c = |p: [f32; 3], uv: [f32; 2]| Corner { pos: p, normal: n, uv, uv1: uv, tan_u: [1.0, 0.0, 0.0], tan_v: [0.0, 0.0, 1.0], face: 1, group: 0 };
        per_material.push(vec![[c([0.0, -4.0, 0.0], [0.0, 0.0]), c([0.0, -4.0, 0.001], [0.0, 1.0]), c([0.001, -4.0, 0.0], [1.0, 0.0])]]);
    }
    let has_uv1 = vec![true; per_material.len()];
    if !per_material.is_empty() {
        bake::assign_lightmap_atlas(&mut per_material, &has_uv1);
    }
    for (i, tris) in per_material.iter_mut().enumerate() {
        if tris.is_empty() {
            continue;
        }
        bake::smooth_normals_angle(tris, 50.0, true);
        bake::tangents_vprim(tris, 0);
        bake::smooth_u_vprim(tris, 40.0, true, false);
        for v in bake::make_visuals(tris, VisualLayout::Full, "range") {
            merged.visuals.push(MergedVisual::every_level(v, slots[i]));
        }
    }
    // collision
    if !ctris.is_empty() {
        let mut verts: Vec<[f32; 3]> = Vec::new();
        let mut seen: HashMap<[u32; 3], u32> = HashMap::new();
        let mut stris: Vec<Triangle> = Vec::new();
        for t in ctris {
            let Some((phys, gameplay)) = physics_for_surface(t.surface) else { continue };
            let mut ix = [0u32; 3];
            for (k, p) in t.p.iter().enumerate() {
                let lp = to_local(*p, origin, yaw);
                ix[k] = *seen.entry([lp[0].to_bits(), lp[1].to_bits(), lp[2].to_bits()]).or_insert_with(|| {
                    verts.push(lp);
                    (verts.len() - 1) as u32
                });
            }
            stris.push(Triangle { indices: ix, material_id: phys, gameplay, surface_index: 0 });
        }
        merged.add_surface_mesh(&verts, &stris, &mapgeom::geom::IDENTITY, 1.0);
    }
    // waypoint
    if let Some(w) = wp {
        merged.waypoint_type = Some(w.kind);
        let center = to_local(w.pos, origin, yaw);
        let ldir = {
            let d = to_local([w.pos[0] + w.dir[0], w.pos[1], w.pos[2] + w.dir[2]], origin, yaw);
            mesh::normalize([d[0] - center[0], 0.0, d[2] - center[2]])
        };
        merged.trigger = Some(trigger_box(center, ldir, 40.0, 12.0, 4.0));
        if w.kind == 4 {
            merged.spawn = [center[0], center[1] + 0.5, center[2]];
        }
    }
    let opts = BuildOpts { ident: name.to_string(), author: name.to_string(), scale: 1.0, collection: STADIUM, skin: None };
    let f = assemble(&merged, &opts)?;
    Ok((write_file(&f), origin, yaw))
}

/// A closed box centred on `c`, `along` its travel direction (thickness
/// `depth`), `width` across, `height` up from 1 m below the centre.
fn trigger_box(c: [f32; 3], along: [f32; 3], width: f32, height: f32, depth: f32) -> CPlugSurface {
    let across = [along[2], 0.0, -along[0]];
    let mut verts: Vec<[f32; 3]> = Vec::new();
    for (sy, sd, sw) in [(0.0, -1.0, -1.0), (0.0, -1.0, 1.0), (0.0, 1.0, 1.0), (0.0, 1.0, -1.0), (1.0, -1.0, -1.0), (1.0, -1.0, 1.0), (1.0, 1.0, 1.0), (1.0, 1.0, -1.0)] {
        let y = c[1] - 1.0 + sy * height;
        verts.push([c[0] + along[0] * sd * depth / 2.0 + across[0] * sw * width / 2.0, y, c[2] + along[2] * sd * depth / 2.0 + across[2] * sw * width / 2.0]);
    }
    let faces: [[u32; 3]; 12] = [[0, 2, 1], [0, 3, 2], [4, 5, 6], [4, 6, 7], [0, 1, 5], [0, 5, 4], [1, 2, 6], [1, 6, 5], [2, 3, 7], [2, 7, 6], [3, 0, 4], [3, 4, 7]];
    let tris = faces.iter().map(|f| Triangle { indices: *f, material_id: 0, gameplay: 0, surface_index: 0 }).collect();
    CPlugSurface::mesh(verts, tris, vec![0], [0.0, 0.0, 1.0])
}

/// The custom-texture material: the item-editor form (`IsUsingGameMaterial`
/// off, shading model `TDSN` — or `TDOSN` for an alpha-cut texture, which
/// reads the DiffuseO slot), the texture named by its bare file name.
pub fn custom_material(mat: &mesh::Material, alpha: bool, physics: u8) -> CPlugMaterialUserInst {
    let mut inst = CPlugMaterialUserInst::game_material("Stadium\\Media\\Material\\PlatformTech", physics);
    let stem = mat.stem();
    let file = format!("{stem}.dds");
    if let Some(main) = inst.main.as_mut() {
        main.is_using_game_material = false;
        main.model = Id::Str(if alpha { "TDOSN".into() } else { "TDSN".into() });
        main.material_name = Id::Str(stem.clone());
        main.link = Id::Null;
        main.user_textures = vec![UserTexture { u01: if alpha { 1 } else { 0 }, texture: file }];
    }
    inst
}

/// Untextured (vertex-coloured) pieces: a plain game material for now.
fn flat_material() -> CPlugMaterialUserInst {
    CPlugMaterialUserInst::game_material("Stadium\\Media\\Material\\DecoHill", PHYS_GRASS)
}

fn cell_for(p: [f32; 3]) -> (i32, i32, i32) {
    let c = |v: f32, d: f32| (v / d).floor().clamp(0.0, 254.0) as i32;
    (c(p[0], 32.0), c(p[1] + 64.0, 8.0), c(p[2], 32.0))
}

/// The map: the host's blocks deleted, its items re-pointed at ours (and
/// grown as needed), our items + textures embedded, multilap set.
/// What the host map is: a campaign map to hollow out, or a VOID base (TMX
/// 117600 "128³ Day Void Base": every cell an embedded `GrassRemover` custom
/// block, so the map has no ground at all — the course floats like on the
/// N64). A void host keeps its blocks, its genealogy and its embedded block.
pub struct HostKind {
    pub void: bool,
    pub size: [i32; 3],
}

pub fn host_kind(m: &MapFile) -> HostKind {
    HostKind { void: m.blocks.iter().any(|b| b.name.contains("GrassRemover")), size: m.size }
}

/// The rows of the host's embedded-objects manifest (ident, author): the
/// custom block a void base carries must stay listed.
fn existing_manifest(body: &[u8]) -> Vec<(String, String)> {
    let chunks = tmmaps::gbx::all_skip_chunks(body);
    let Some(&(_, _, payload, size)) = chunks.iter().find(|(c, ..)| *c == 0x0304_3054) else { return Vec::new() };
    let b = &body[payload..payload + size];
    if b.len() < 20 {
        return Vec::new();
    }
    let u32_at = |o: usize| u32::from_le_bytes(b[o..o + 4].try_into().unwrap());
    let n = u32_at(12) as usize;
    if n == 0 {
        return Vec::new();
    }
    let mut o = 20; // version, u01, byte count, n, lookback version
    let mut table: Vec<String> = Vec::new();
    let mut rows = Vec::new();
    let mut read_str = |o: &mut usize| -> Option<String> {
        let w = u32_at(*o);
        *o += 4;
        if w & 0x4000_0000 == 0 {
            return None;
        }
        let idx = w & 0x3FFF_FFFF;
        if idx == 0 {
            let len = u32_at(*o) as usize;
            *o += 4;
            let s = String::from_utf8_lossy(&b[*o..*o + len]).into_owned();
            *o += len;
            table.push(s.clone());
            Some(s)
        } else {
            table.get(idx as usize - 1).cloned()
        }
    };
    for _ in 0..n {
        let Some(id) = read_str(&mut o) else { break };
        o += 4; // collection
        let Some(author) = read_str(&mut o) else { break };
        rows.push((id, author));
    }
    rows
}

/// The map: the host's blocks deleted (a void base keeps its GrassRemovers),
/// its items re-pointed at ours (and grown as needed), our items + textures
/// embedded, multilap set.
fn write_map(host: &Path, out: &Path, specs: &[ItemSpec], pictures: &BTreeMap<String, Vec<u8>>, name: &str, laps: u32, no_stadium: bool, dir: &str, times: [u32; 4]) {
    let tmp = |tag: &str| out.with_extension(format!("mk64-{}.{tag}.Map.Gbx", std::process::id()));
    let t_seed = tmp("seeded");
    let t0 = tmp("slots");
    let t1 = tmp("blocks");
    let t2 = tmp("models");
    let t3 = tmp("waypoints");
    let t4 = tmp("decoration");

    let probe = MapFile::load(host);
    let kind = host_kind(&probe);
    let host_items = probe.items.len();
    let carried: Vec<(String, Vec<u8>)> = tmmaps::header::embedded_zip_bytes(&probe.gbx.body).map(|(zip, _)| tmmaps::header::zip_entries(&zip)).unwrap_or_default();
    let carried_rows = existing_manifest(&probe.gbx.body);
    drop(probe);
    // a host with no item record has nothing to clone: seed one first
    let mut base = host.to_path_buf();
    if host_items == 0 {
        let g = tmmaps::gbx::Gbx::parse(&std::fs::read(host).expect("read host"));
        let body = tmmaps::map::seed_item_record(&g.body, STADIUM).expect("seed an item record");
        std::fs::write(&t_seed, g.write_body_recompressed(&body)).expect("write seeded host");
        base = t_seed.clone();
    }
    let mut m = MapFile::load(&base);
    let total = specs.len().max(m.items.len());
    m.strip_validation_ghost();
    m.append_item_clones(total);
    m.write_to(&t0).expect("write slot stage");

    let mut m = MapFile::load(&t0);
    let old_uid = m.body_ids.first().and_then(|f| f.name.clone()).expect("map uid");
    if !kind.void {
        let r = m.remove_blocks(|_| true, |_| true);
        println!("  host: {} blocks, {} baked blocks deleted; {} item slots ({} from the host)", r.blocks, r.baked, total, host_items);
        m.write_to(&t1).expect("write block stage");
        m = MapFile::load(&t1);
        assert_eq!(m.blocks.len(), 0);
    } else {
        println!("  host: void base {}×{}×{} ({} GrassRemover cells kept); {} item slots", kind.size[0], kind.size[1], kind.size[2], m.blocks.len(), total);
    }
    // a uid of the same length: MK + course number + the host's tail
    let idx = course::COURSES.iter().position(|(d, _, _)| *d == dir).unwrap_or(0);
    let new_uid = format!("MK{idx:02}{}", &old_uid[4..]);
    m.set_map_uid(&new_uid);
    let xml = tmmaps::header::user_chunks(&m.gbx.user_data).and_then(|c| tmmaps::header::header_xml(&c)).unwrap_or_default();
    let old_name = tmmaps::header::attr_pub(&xml, "ident", "name").unwrap_or_default();
    m.write_to(&t1).expect("write uid stage");

    // models + placements; the surplus host slots park a copy of the first item far below
    let mut m = MapFile::load(&t1);
    for i in 0..total {
        let s = &specs[i.min(specs.len() - 1)];
        m.set_item_model(i, &s.name);
        m.set_item_author(i, &s.name);
        m.set_item_collection(i, STADIUM);
        let (pos, yaw) = if i < specs.len() { (s.pos, s.yaw) } else { ([16.0, -1000.0, 16.0], 0.0) };
        m.move_item(i, pos, yaw, cell_for(pos));
        m.set_item_scale(i, 1.0);
        m.clear_item_variant(i);
        m.set_item_color(i, 0);
    }
    m.write_to(&t2).expect("write model stage");

    let mut m = MapFile::load(&t2);
    for (i, s) in specs.iter().enumerate() {
        m.set_item_waypoint(i, s.tag.as_deref(), s.order);
    }
    for i in specs.len()..total {
        m.set_item_waypoint(i, None, 0);
    }
    m.write_to(&t3).expect("write waypoint stage");

    // the decoration is a lookback rename: its own write (splices come next)
    let mut m = MapFile::load(&t3);
    if no_stadium && !kind.void {
        let deco = "NoStadium48x48Day";
        m.set_decoration(deco);
        m.set_header_decoration("48x48Screen155Day", deco);
    }
    m.write_to(&t4).expect("write decoration stage");

    let mut m = MapFile::load(&t4);
    let mut files: BTreeMap<String, Vec<u8>> = BTreeMap::new();
    for (n, b) in &carried {
        files.insert(n.clone(), b.clone());
    }
    for s in specs {
        files.insert(format!("Items/{}", s.name), s.bytes.clone());
    }
    for (n, b) in pictures {
        files.insert(format!("Items/{n}"), b.clone());
    }
    let zip = mapgeom::tiny_assets::zip(&files);
    let mut manifest: Vec<(&str, &str)> = carried_rows.iter().map(|(a, b)| (a.as_str(), b.as_str())).collect();
    manifest.extend(specs.iter().map(|s| (s.name.as_str(), s.name.as_str())));
    m.replace_embedded_objects(&manifest, &zip);
    // the host's MediaTracker clips (its intro flies over a map that is gone)
    if let Some(Ok(mut mt)) = m.mediatracker() {
        mt.strip = true;
        m.set_mediatracker(&mt);
    }
    if !old_name.is_empty() {
        m.set_map_name(&old_name, name);
    }
    // multilap: chunk 0x03043018 (IsLapRace, NbLaps) and the header's nblaps
    {
        let chunks = tmmaps::gbx::all_skip_chunks(&m.gbx.body);
        if let Some(&(_, _, payload, size)) = chunks.iter().find(|(c, ..)| *c == 0x0304_3018) {
            if size >= 8 {
                let mut b = Vec::new();
                b.extend_from_slice(&1u32.to_le_bytes());
                b.extend_from_slice(&laps.to_le_bytes());
                m.raw_patches.push((payload, b));
            }
        } else {
            println!("  WARNING: no laps chunk 0x03043018 in the host; laps not set");
        }
        let laps_s = laps.to_string();
        m.edit_header_xml(&|x: &str| {
            let re = x.find("nblaps=\"")?;
            let end = x[re + 8..].find('"')? + re + 8;
            Some(format!("{}nblaps=\"{}\"{}", &x[..re], laps_s, &x[end + 1..]))
        });
    }
    // medal times: chunk 0x0305B00A (tip string, bronze, silver, gold, author in
    // ms) and the header's <times>; a driven lap replaces these estimates
    {
        let chunks = tmmaps::gbx::all_skip_chunks(&m.gbx.body);
        if let Some(&(_, _, payload, size)) = chunks.iter().find(|(c, ..)| *c == 0x0305_B00A) {
            let tip_len = u32::from_le_bytes(m.gbx.body[payload..payload + 4].try_into().unwrap()) as usize;
            let at = payload + 4 + tip_len;
            if at + 16 <= payload + size {
                let mut b = Vec::new();
                for t in [times[3], times[2], times[1], times[0]] {
                    b.extend_from_slice(&t.to_le_bytes());
                }
                m.raw_patches.push((at, b));
            }
        }
        let (a, g, s, br) = (times[0], times[1], times[2], times[3]);
        m.edit_header_xml(&|x: &str| {
            let i = x.find("<times ")?;
            let j = x[i..].find("/>")? + i + 2;
            Some(format!("{}<times bronze=\"{br}\" silver=\"{s}\" gold=\"{g}\" authortime=\"{a}\" authorscore=\"0\" hasclones=\"0\"/>{}", &x[..i], &x[j..]))
        });
    }
    m.remove_password();
    m.write_to(out).expect("write map");
    if !kind.void {
        match MapFile::clear_genealogy_file(out) {
            Ok(n) => println!("  genealogy cleared ({n} zone records)"),
            Err(e) => println!("  genealogy: {e}"),
        }
    }
    for t in [&t_seed, &t0, &t1, &t2, &t3, &t4] {
        let _ = std::fs::remove_file(t);
    }
    // read back
    let check = MapFile::load(out);
    let wps = check.waypoints();
    println!(
        "  wrote {} ({} bytes): {} items, {} waypoints [{}], uid {}, zip {} files",
        out.display(),
        std::fs::metadata(out).map(|m| m.len()).unwrap_or(0),
        check.items.len(),
        wps.len(),
        wps.iter().map(|w| w.tag.clone()).collect::<Vec<_>>().join(","),
        new_uid,
        files.len()
    );
}

/// Every material's image from the ROM, mirrored per the material's wrap
/// flags (magenta for a texture the index does not know).
pub fn material_images(mesh: &Mesh, assets: &AssetIndex, rom: &mut Rom) -> (HashMap<usize, Image>, Vec<String>) {
    let mut out = HashMap::new();
    let mut missing = Vec::new();
    for (i, m) in mesh.materials.iter().enumerate() {
        let base = if m.is_flat() {
            Ok(Image::solid(4, 4, [255, 255, 255, 255]))
        } else {
            assets.locate(&m.sym).ok_or_else(|| "not in the asset index".to_string()).and_then(|loc| {
                let tlut = loc.tlut.as_deref().and_then(|t| assets.locate(t));
                rom.texture(&loc, tlut.as_ref())
            })
        };
        match base {
            Ok(img) => {
                out.insert(i, img.mirrored(m.mirror_s, m.mirror_t).tinted(m.tint));
            }
            Err(e) => {
                missing.push(format!("{}: {e}", m.sym));
                out.insert(i, Image::solid(m.w.max(1), m.h.max(1), [255, 0, 255, 255]));
            }
        }
    }
    (out, missing)
}
