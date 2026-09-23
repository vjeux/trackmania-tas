//! `mk64` — Mario Kart 64 courses → Trackmania 2020.
//!
//! Inputs: a `n64decomp/mk64` checkout (`--decomp DIR` or `$MK64_DECOMP`) for
//! the course data, the US ROM (`--rom FILE` or `$MK64_ROM`) for the textures.

use mk64::course::{self, Course};
use mk64::mesh::{self, Frame};
use mk64::texture::{AssetIndex, Image, Rom};
use std::collections::HashMap;
use std::path::PathBuf;

const USAGE: &str = "mk64 — Mario Kart 64 courses → Trackmania 2020

  mk64 courses                         every course: vertices, sections, path length, scale
  mk64 stats COURSE                    pieces, triangles, textures, surfaces of one course
  mk64 textures COURSE --out DIR       the course's textures as PNG (and the mirrored variants)
  mk64 render COURSE --out FILE.png    top-down textured render [--px N] [--collision] [--path]
  mk64 build COURSE --host MAP --out MAP   the Trackmania map (see `mk64 build --help`)

common flags: --decomp DIR ($MK64_DECOMP)  --rom FILE ($MK64_ROM)  --scale S (metres per unit;
default = official lap length / centre path length)  --mirror";

fn flag<'a>(args: &'a [String], name: &str) -> Option<&'a str> {
    args.iter().position(|a| a == name).and_then(|i| args.get(i + 1)).map(|s| s.as_str())
}

fn has(args: &[String], name: &str) -> bool {
    args.iter().any(|a| a == name)
}

fn decomp_dir(args: &[String]) -> PathBuf {
    match flag(args, "--decomp").map(String::from).or_else(|| std::env::var("MK64_DECOMP").ok()) {
        Some(d) => PathBuf::from(d),
        None => {
            eprintln!("need --decomp DIR or $MK64_DECOMP (a n64decomp/mk64 checkout)");
            std::process::exit(2);
        }
    }
}

fn rom_path(args: &[String]) -> Option<PathBuf> {
    flag(args, "--rom").map(String::from).or_else(|| std::env::var("MK64_ROM").ok()).map(PathBuf::from)
}

fn course_arg(args: &[String]) -> String {
    match args.get(2) {
        Some(c) if !c.starts_with("--") => c.clone(),
        _ => {
            eprintln!("which course? one of: {}", course::COURSES.iter().map(|c| c.0).collect::<Vec<_>>().join(" "));
            std::process::exit(2);
        }
    }
}

/// The world frame for a course: metres per unit from the official lap
/// length over the centre path, unless `--scale` says otherwise.
pub fn frame_for(course: &Course, args: &[String]) -> Frame {
    let scale = match flag(args, "--scale") {
        Some(s) => s.parse::<f32>().expect("--scale number"),
        None => {
            let len = course.path_length() as f32;
            match course::official_length_m(&course.dir) {
                Some(m) if len > 0.0 => m / len,
                _ => 0.1,
            }
        }
    };
    Frame { scale, mirror: has(args, "--mirror"), offset: [0.0; 3] }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let cmd = args.get(1).map(|s| s.as_str()).unwrap_or("");
    match cmd {
        "courses" => cmd_courses(&args),
        "stats" => cmd_stats(&args),
        "textures" => cmd_textures(&args),
        "render" => cmd_render(&args),
        "build" => mk64::tm::cmd_build(&args),
        _ => {
            eprintln!("{USAGE}");
            std::process::exit(2);
        }
    }
}

fn cmd_courses(args: &[String]) {
    let decomp = decomp_dir(args);
    println!("{:<22} {:>8} {:>8} {:>6} {:>10} {:>8} {:>9}  bbox (units)", "course", "vertices", "sections", "path", "path_len", "official", "m/unit");
    for dir in course::available(&decomp) {
        match Course::load(&decomp, dir) {
            Ok(c) => {
                let len = c.path_length();
                let off = course::official_length_m(dir);
                let (lo, hi) = c.bbox();
                println!(
                    "{:<22} {:>8} {:>8} {:>6} {:>10.0} {:>8} {:>9}  x {}..{} y {}..{} z {}..{}",
                    dir,
                    c.vertices.len(),
                    c.sections.len(),
                    c.path.len(),
                    len,
                    off.map(|m| format!("{m:.0} m")).unwrap_or("-".into()),
                    off.map(|m| format!("{:.5}", m as f64 / len)).unwrap_or("-".into()),
                    lo[0],
                    hi[0],
                    lo[1],
                    hi[1],
                    lo[2],
                    hi[2]
                );
            }
            Err(e) => println!("{dir:<22} ERROR {e}"),
        }
    }
}

fn load_course(args: &[String]) -> (Course, PathBuf) {
    let decomp = decomp_dir(args);
    let dir = course_arg(args);
    match Course::load(&decomp, &dir) {
        Ok(c) => (c, decomp),
        Err(e) => {
            eprintln!("{dir}: {e}");
            std::process::exit(1);
        }
    }
}

fn cmd_stats(args: &[String]) {
    let (mut c, decomp) = load_course(args);
    let frame = frame_for(&c, args);
    let pieces = c.visual_pieces();
    let coll = c.collision_pieces();
    let assets = AssetIndex::load(&decomp).ok();
    let vis_tris: usize = pieces.iter().map(|p| p.tris.len()).sum();
    let coll_tris: usize = coll.iter().map(|(_, p)| p.tris.len()).sum();
    println!("{} ({}): {} vertices, {} display lists, {} render lists", c.dir, course::course_title(&c.dir), c.vertices.len(), c.dls.len(), c.render_lists.len());
    println!("visual: {} pieces, {vis_tris} triangles; collision: {} sections, {coll_tris} triangles", pieces.len(), coll.len());
    println!("path: {} points, {:.0} units; scale {:.5} m/unit → lap {:.0} m", c.path.len(), c.path_length(), frame.scale, c.path_length() as f32 * frame.scale);
    let (lo, hi) = c.bbox();
    println!(
        "extent: {:.0} × {:.0} m (height {:.0} m)",
        (hi[0] - lo[0]) as f32 * frame.scale,
        (hi[2] - lo[2]) as f32 * frame.scale,
        (hi[1] - lo[1]) as f32 * frame.scale
    );
    println!("item boxes: {}; spawn tables: {}", c.item_boxes.len(), c.spawns.iter().map(|(n, v)| format!("{n}={}", v.len())).collect::<Vec<_>>().join(" "));
    // surfaces
    let mut per_surface: HashMap<u8, (usize, usize)> = HashMap::new();
    for (s, p) in &coll {
        let e = per_surface.entry(s.surface).or_insert((0, 0));
        e.0 += 1;
        e.1 += p.tris.len();
    }
    let mut surf: Vec<_> = per_surface.into_iter().collect();
    surf.sort();
    println!("surfaces:");
    for (s, (n, t)) in surf {
        println!("  {:<20} {n:>4} sections {t:>6} tris", course::surface_name(s));
    }
    // section flags
    let mut flags: HashMap<u16, usize> = HashMap::new();
    for (s, _) in &coll {
        *flags.entry(s.flags).or_insert(0) += 1;
    }
    println!("section flags: {}", flags.iter().map(|(f, n)| format!("{f:#06x}×{n}")).collect::<Vec<_>>().join(" "));
    // textures
    println!("textures:");
    let usage = c.texture_usage(&pieces);
    for (sym, n) in &usage {
        let loc = assets.as_ref().and_then(|a| a.locate(sym));
        let where_ = match loc {
            Some(l) => format!("{}×{} {} @{:#x}+{:#x}", l.w, l.h, l.fmt, l.rom_offset, l.block_offset),
            None => "NOT IN THE ASSET INDEX".to_string(),
        };
        println!("  {sym:<40} {n:>6} tris  {where_}");
    }
    // geometry modes
    let two_sided = pieces.iter().flat_map(|p| &p.tris).filter(|t| t.geom & (mesh::G_CULL_BACK | mesh::G_CULL_FRONT) == 0).count();
    let lit = pieces.iter().flat_map(|p| &p.tris).filter(|t| t.geom & mesh::G_LIGHTING != 0).count();
    println!("two-sided tris: {two_sided}; lit tris: {lit}");
    for n in c.notes.iter().take(20) {
        println!("note: {n}");
    }
    if c.notes.len() > 20 {
        println!("... {} notes", c.notes.len());
    }
}

/// Every material's image, mirrored per the material's wrap flags, from the ROM.
pub fn material_images(mesh: &mesh::Mesh, assets: &AssetIndex, rom: &mut Rom) -> (HashMap<usize, Image>, Vec<String>) {
    let mut out = HashMap::new();
    let mut missing = Vec::new();
    for (i, m) in mesh.materials.iter().enumerate() {
        match assets.locate(&m.sym).ok_or_else(|| "not in the asset index".to_string()).and_then(|loc| rom.texture(&loc)) {
            Ok(img) => {
                out.insert(i, img.mirrored(m.mirror_s, m.mirror_t));
            }
            Err(e) => {
                missing.push(format!("{}: {e}", m.sym));
                out.insert(i, Image::solid(m.w.max(1), m.h.max(1), [255, 0, 255, 255]));
            }
        }
    }
    (out, missing)
}

fn open_rom(args: &[String]) -> Rom {
    let p = rom_path(args).unwrap_or_else(|| {
        eprintln!("need --rom FILE or $MK64_ROM");
        std::process::exit(2);
    });
    Rom::load(&p).unwrap_or_else(|e| {
        eprintln!("{e}");
        std::process::exit(1);
    })
}

fn cmd_textures(args: &[String]) {
    let (mut c, decomp) = load_course(args);
    let out = PathBuf::from(flag(args, "--out").expect("--out DIR"));
    std::fs::create_dir_all(&out).expect("create --out");
    let assets = AssetIndex::load(&decomp).expect("asset index");
    let mut rom = open_rom(args);
    let pieces = c.visual_pieces();
    let frame = frame_for(&c, args);
    let m = mesh::visual_mesh(&c, &pieces, Some(&assets), &frame);
    let (images, missing) = material_images(&m, &assets, &mut rom);
    for (i, mat) in m.materials.iter().enumerate() {
        let img = &images[&i];
        let png = mapgeom::render::png(&mapgeom::render::Image { w: img.w as usize, h: img.h as usize, rgb: img.rgba.chunks(4).flat_map(|p| [p[0], p[1], p[2]]).collect() });
        std::fs::write(out.join(format!("{}.png", mat.stem())), png).expect("write png");
        println!("{:<32} {}×{} {}", mat.stem(), img.w, img.h, if img.has_alpha() { "alpha" } else { "" });
    }
    for m in missing {
        println!("MISSING {m}");
    }
}

fn cmd_render(args: &[String]) {
    let (mut c, decomp) = load_course(args);
    let out = PathBuf::from(flag(args, "--out").expect("--out FILE.png"));
    let px: usize = flag(args, "--px").map(|s| s.parse().expect("--px N")).unwrap_or(2048);
    let frame = frame_for(&c, args);
    let assets = AssetIndex::load(&decomp).ok();
    let (m, images) = if has(args, "--collision") {
        let coll = c.collision_pieces();
        let soup = mesh::collision_mesh(&c, &coll, &frame);
        // colour by surface
        let mut mm = mesh::Mesh::default();
        for t in soup {
            let col = surface_colour(t.surface);
            let corner = |p: [f32; 3]| mesh::Corner { pos: p, uv: [0.0, 0.0], rgba: [col[0], col[1], col[2], 255] };
            mm.tris.push(mesh::Tri { c: [corner(t.p[0]), corner(t.p[1]), corner(t.p[2])], mat: None, two_sided: true, lit: false, piece: t.section_id as u32 });
        }
        (mm, HashMap::new())
    } else {
        let pieces = c.visual_pieces();
        let m = mesh::visual_mesh(&c, &pieces, assets.as_ref(), &frame);
        let images = match (assets.as_ref(), rom_path(args)) {
            (Some(a), Some(_)) => {
                let mut rom = open_rom(args);
                let (imgs, missing) = material_images(&m, a, &mut rom);
                for x in missing {
                    eprintln!("texture missing: {x}");
                }
                imgs
            }
            _ => HashMap::new(),
        };
        (m, images)
    };
    let mut img = mk64::render::top_down(&m, &images, px);
    if has(args, "--path") {
        let pts: Vec<[f32; 3]> = c.path.iter().map(|p| frame.to_tm(p.pos)).collect();
        for i in 0..pts.len() {
            let a = pts[i];
            let b = pts[(i + 1) % pts.len()];
            img.line((a[0], a[2]), (b[0], b[2]), [255, 255, 0]);
        }
        if let Some(p0) = pts.first() {
            img.dot(p0[0], p0[2], 6, [255, 0, 0]);
        }
        for ib in &c.item_boxes {
            let p = frame.to_tm(ib.pos);
            img.dot(p[0], p[2], 3, [0, 255, 255]);
        }
    }
    std::fs::write(&out, img.png()).expect("write png");
    println!("{}: {}×{} px, {:.2} m/px, {} tris, {} materials", out.display(), img.w, img.h, img.m_per_px, m.tris.len(), m.materials.len());
}

pub fn surface_colour(s: u8) -> [u8; 3] {
    match s {
        1 => [90, 90, 90],     // asphalt
        2 => [150, 100, 50],   // dirt
        3 => [220, 200, 120],  // sand
        4 => [130, 130, 150],  // stone
        5 => [235, 235, 245],  // snow
        6 => [160, 110, 60],   // bridge
        7 => [200, 170, 90],   // sand offroad
        8 => [60, 160, 60],    // grass
        9 => [170, 220, 255],  // ice
        10 => [170, 150, 90],  // wet sand
        11 => [200, 200, 220], // snow offroad
        12 => [110, 80, 60],   // cliff
        13 => [120, 80, 40],   // dirt offroad
        14 => [120, 100, 90],  // train track
        15 => [70, 60, 60],    // cave
        16 => [150, 120, 80],  // rope bridge
        17 => [170, 120, 70],  // wood bridge
        0xFC | 0xFE => [255, 140, 0], // boost ramps
        0xFD => [255, 0, 0],   // out of bounds
        0xFF => [255, 255, 0], // ramp
        _ => [255, 0, 255],
    }
}
