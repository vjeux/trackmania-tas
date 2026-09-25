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
            let _ = course;
            mesh::UNITS_TO_M
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
        "sprites" => cmd_sprites(&args),
        "overlaps" => cmd_overlaps(&args),
        "ghost" => cmd_ghost(&args),
        "clones" => cmd_clones(&args),
        "cpus" => cmd_cpus(&args),
        "ghost-compress" => cmd_ghost_compress(&args),
        "ghost-all" => cmd_ghost_all(&args),
        "mux" => cmd_mux(&args),
        "colours" => cmd_colours(&args),
        "routes" => cmd_routes(&args),
        "skins" => mk64::skins::cmd(&args, &decomp_dir(&args), &rom_path(&args).expect("--rom FILE ($MK64_ROM)")),
        "texture" => cmd_texture(&args),
        "render" => cmd_render(&args),
        "build" => mk64::tm::cmd_build(&args),
        "build-all" => mk64::tm::cmd_build_all(&args),
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
    let vm = mesh::visual_mesh(&c, &pieces, assets.as_ref(), &frame);
    let (flat, grad, ncol, npairs) = mesh::colour_census(&vm, 8);
    println!("vertex colours: {flat} flat tris, {grad} gradient tris, {ncol} distinct colours, {npairs} (material, colour/8) pairs");
    for n in c.notes.iter().take(20) {
        println!("note: {n}");
    }
    if c.notes.len() > 20 {
        println!("... {} notes", c.notes.len());
    }
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
    let (images, missing) = mk64::tm::material_images(&m, &assets, &mut rom);
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
        let mut m = mesh::visual_mesh(&c, &pieces, assets.as_ref(), &frame);
        if !has(args, "--no-actors") {
            for kind in ["tree", "cow", "piranha_plant", "cactus"] {
                mk64::actors::add_billboards(&c, &mut m, &frame, kind);
            }
        }
        if !has(args, "--no-vertex-colours") {
            mesh::bake_vertex_colours(&mut m, 16, 24, 4);
        }
        let images = match (assets.as_ref(), rom_path(args)) {
            (Some(a), Some(_)) => {
                let mut rom = open_rom(args);
                let (imgs, missing) = mk64::tm::material_images(&m, a, &mut rom);
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
        // the alternate routes in other colours, with a dot every 5 % of each
        let colours = [[255, 80, 255], [80, 255, 80], [255, 140, 0], [80, 200, 255]];
        let mut alts: Vec<(&String, &Vec<[i16; 3]>)> = c.other_paths.iter().filter(|(nm, _)| nm.contains("_track_path_")).collect();
        alts.sort();
        for (k, (_, alt)) in alts.iter().enumerate() {
            let col = colours[k % colours.len()];
            let ap: Vec<[f32; 3]> = alt.iter().map(|q| frame.to_tm(*q)).collect();
            for i in 0..ap.len() {
                let (a, b) = (ap[i], ap[(i + 1) % ap.len()]);
                img.line((a[0], a[2]), (b[0], b[2]), col);
            }
        }
        for i in 0..pts.len() {
            if i * 20 % pts.len() < 20 {
                img.dot(pts[i][0], pts[i][2], 4, [255, 255, 255]);
            }
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

/// `mk64 texture SYM [SYM…] --out DIR`: named ROM textures (any asset json
/// symbol: `gTextureLakituRedLights01`, `minimap_luigi_raceway`, …) as PNGs,
/// alpha shown over magenta.
fn cmd_texture(args: &[String]) {
    let decomp = PathBuf::from(flag(args, "--decomp").map(String::from).or_else(|| std::env::var("MK64_DECOMP").ok()).expect("--decomp DIR or MK64_DECOMP"));
    let out = PathBuf::from(flag(args, "--out").expect("--out DIR"));
    std::fs::create_dir_all(&out).expect("create --out");
    let assets = AssetIndex::load(&decomp).expect("asset index");
    let mut rom = open_rom(args);
    for sym in args.iter().skip(2).take_while(|a| !a.starts_with("--")) {
        let Some(loc) = assets.locate(sym) else {
            println!("{sym}: not in the asset index");
            continue;
        };
        let tlut = loc.tlut.as_deref().and_then(|t| assets.locate(t));
        match rom.texture(&loc, tlut.as_ref()) {
            Ok(img) => {
                let rgb: Vec<u8> = img.rgba.chunks(4).flat_map(|p| if p[3] < 128 { [255, 0, 255] } else { [p[0], p[1], p[2]] }).collect();
                let png = mapgeom::render::png(&mapgeom::render::Image { w: img.w as usize, h: img.h as usize, rgb });
                std::fs::write(out.join(format!("{sym}.png")), png).expect("write png");
                println!("{sym:<40} {}×{} {} {}", img.w, img.h, loc.fmt, if img.has_alpha() { "alpha" } else { "" });
            }
            Err(e) => println!("{sym}: {e}"),
        }
    }
}

/// `mk64 sprites CHAR --out DIR [--frames A-B] [--scale N]`: contact sheets of a
/// character's kart sprite frames, select faces and portrait (to pick the
/// view angles the car skins use).
fn cmd_sprites(args: &[String]) {
    let stem = args.get(2).cloned().expect("mk64 sprites CHAR --out DIR");
    let out = PathBuf::from(flag(args, "--out").expect("--out DIR"));
    std::fs::create_dir_all(&out).expect("create out dir");
    let decomp = decomp_dir(args);
    let rom_path = rom_path(args).expect("--rom FILE ($MK64_ROM)");
    let mut rom = mk64::texture::Rom::load(&rom_path).expect("rom");
    let assets = mk64::texture::AssetIndex::load(&decomp).expect("asset index");
    let scale: u32 = flag(args, "--scale").and_then(|s| s.parse().ok()).unwrap_or(2);
    let (a, b) = flag(args, "--frames")
        .and_then(|s| s.split_once('-'))
        .and_then(|(a, b)| Some((a.parse::<usize>().ok()?, b.parse::<usize>().ok()?)))
        .unwrap_or((0, 41));
    let face_stem = mk64::sprites::CHARACTERS.iter().find(|(s, _, _)| *s == stem).map(|(_, f, _)| *f).unwrap_or(stem.as_str());
    let ks = mk64::sprites::KartSprites::load(&decomp, &stem, face_stem).expect("kart sprites");
    println!("{stem}: {} kart frames, {} faces", ks.frame_count(), ks.face_count());
    let mut frames = Vec::new();
    for i in a..=b.min(ks.frame_count().saturating_sub(1)) {
        match ks.frame(&rom, i) {
            Ok(img) => frames.push(img),
            Err(e) => println!("  frame {i}: {e}"),
        }
    }
    let sheet = mk64::sprites::contact_sheet(&frames, 7, scale);
    std::fs::write(out.join(format!("{stem}_frames_{a}-{b}.png")), sheet.png()).expect("write sheet");
    let mut faces = Vec::new();
    for i in 0..ks.face_count() {
        if let Ok(img) = ks.face(&rom, i) {
            faces.push(img);
        }
    }
    if !faces.is_empty() {
        std::fs::write(out.join(format!("{stem}_faces.png")), mk64::sprites::contact_sheet(&faces, 6, scale).png()).expect("write faces");
    }
    match ks.portrait(&mut rom, &assets) {
        Ok(p) => std::fs::write(out.join(format!("{stem}_portrait.png")), mk64::sprites::contact_sheet(&[p], 1, scale * 2).png()).expect("write portrait"),
        Err(e) => println!("  portrait: {e}"),
    }
    println!("wrote sheets to {}", out.display());
}

/// `mk64 overlaps COURSE`: coplanar overlapping triangle pairs between materials
/// (the z-fighting candidates), by material pair.
fn cmd_overlaps(args: &[String]) {
    let (mut c, _decomp) = load_course(args);
    let dir = course_arg(args);
    let pieces = c.visual_pieces();
    let frame = frame_for(&c, args);
    let m = mesh::visual_mesh(&c, &pieces, None, &frame);
    let name = |t: &mk64::mesh::Tri| t.mat.map(|i| m.materials[i].stem()).unwrap_or_else(|| "(untextured)".into());
    // plane of each tri; bucket by (rounded normal, rounded d)
    let mut buckets: std::collections::HashMap<(i32, i32, i32, i32), Vec<usize>> = std::collections::HashMap::new();
    let plane = |t: &mk64::mesh::Tri| -> Option<([f32; 3], f32)> {
        let (a, b, cc) = (t.c[0].pos, t.c[1].pos, t.c[2].pos);
        let u = [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
        let v = [cc[0] - a[0], cc[1] - a[1], cc[2] - a[2]];
        let mut n = [u[1] * v[2] - u[2] * v[1], u[2] * v[0] - u[0] * v[2], u[0] * v[1] - u[1] * v[0]];
        let l = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt();
        if l < 1e-6 {
            return None;
        }
        for k in 0..3 {
            n[k] /= l;
        }
        if n[1] < 0.0 || (n[1] == 0.0 && (n[0] < 0.0 || (n[0] == 0.0 && n[2] < 0.0))) {
            for k in 0..3 {
                n[k] = -n[k];
            }
        }
        Some((n, n[0] * a[0] + n[1] * a[1] + n[2] * a[2]))
    };
    for (i, t) in m.tris.iter().enumerate() {
        if let Some((n, d)) = plane(t) {
            buckets.entry(((n[0] * 50.0).round() as i32, (n[1] * 50.0).round() as i32, (n[2] * 50.0).round() as i32, (d / 0.05).round() as i32)).or_default().push(i);
        }
    }
    let mut pairs: std::collections::BTreeMap<(String, String), (usize, f32)> = std::collections::BTreeMap::new();
    let bbox = |t: &mk64::mesh::Tri| {
        let mut lo = [f32::MAX; 3];
        let mut hi = [f32::MIN; 3];
        for c in &t.c {
            for k in 0..3 {
                lo[k] = lo[k].min(c.pos[k]);
                hi[k] = hi[k].max(c.pos[k]);
            }
        }
        (lo, hi)
    };
    // real overlap: project both triangles onto the plane's dominant axes and
    // test whether one's centroid or an edge midpoint lies strictly inside the other
    let inside = |p: [f32; 2], t: [[f32; 2]; 3]| -> bool {
        let s = |a: [f32; 2], b: [f32; 2], c: [f32; 2]| (b[0] - a[0]) * (c[1] - a[1]) - (b[1] - a[1]) * (c[0] - a[0]);
        let (d1, d2, d3) = (s(t[0], t[1], p), s(t[1], t[2], p), s(t[2], t[0], p));
        let eps = 1e-3;
        (d1 > eps && d2 > eps && d3 > eps) || (d1 < -eps && d2 < -eps && d3 < -eps)
    };
    for (key, idx) in &buckets {
        let n = [key.0 as f32 / 50.0, key.1 as f32 / 50.0, key.2 as f32 / 50.0];
        let (ax, ay) = if n[1].abs() >= n[0].abs() && n[1].abs() >= n[2].abs() { (0usize, 2usize) } else if n[0].abs() >= n[2].abs() { (1, 2) } else { (0, 1) };
        let proj = |t: &mk64::mesh::Tri| [[t.c[0].pos[ax], t.c[0].pos[ay]], [t.c[1].pos[ax], t.c[1].pos[ay]], [t.c[2].pos[ax], t.c[2].pos[ay]]];
        let probes = |t: [[f32; 2]; 3]| {
            let c = [(t[0][0] + t[1][0] + t[2][0]) / 3.0, (t[0][1] + t[1][1] + t[2][1]) / 3.0];
            [c, [(t[0][0] + t[1][0]) / 2.0, (t[0][1] + t[1][1]) / 2.0], [(t[1][0] + t[2][0]) / 2.0, (t[1][1] + t[2][1]) / 2.0], [(t[2][0] + t[0][0]) / 2.0, (t[2][1] + t[0][1]) / 2.0]]
        };
        for a in 0..idx.len() {
            for b in a + 1..idx.len() {
                let (ta, tb) = (&m.tris[idx[a]], &m.tris[idx[b]]);
                let (pa, pb) = (proj(ta), proj(tb));
                let hit = probes(pa).iter().any(|p| inside(*p, pb)) || probes(pb).iter().any(|p| inside(*p, pa));
                if !hit {
                    continue;
                }
                let (la, ha) = bbox(ta);
                let (lb, hb) = bbox(tb);
                let area = |lo: [f32; 3], hi: [f32; 3]| (hi[0] - lo[0]).max(hi[2] - lo[2]);
                let key = {
                    let (x, y) = (name(ta), name(tb));
                    if x <= y { (x, y) } else { (y, x) }
                };
                let e = pairs.entry(key).or_insert((0, 0.0));
                e.0 += 1;
                e.1 = e.1.max(area(la, ha).min(area(lb, hb)));
            }
        }
    }
    println!("{dir}: {} tris; coplanar overlapping pairs by material pair (count, largest extent m):", m.tris.len());
    let mut v: Vec<_> = pairs.into_iter().collect();
    v.sort_by(|a, b| b.1 .0.cmp(&a.1 .0));
    for ((a, b), (n, ext)) in v.iter().take(25) {
        println!("  {n:5}  {ext:6.1} m  {a}  ×  {b}");
    }
}

/// `mk64 colours COURSE [--mat SUBSTR]`: vertex-colour statistics per material
/// (how much the tints vary within a triangle and across the material).
fn cmd_colours(args: &[String]) {
    let (mut c, _decomp) = load_course(args);
    let pieces = c.visual_pieces();
    let frame = frame_for(&c, args);
    let m = mesh::visual_mesh(&c, &pieces, None, &frame);
    let want = flag(args, "--mat").map(String::from);
    let mut per: std::collections::BTreeMap<String, Vec<&mesh::Tri>> = std::collections::BTreeMap::new();
    for t in &m.tris {
        let n = t.mat.map(|i| m.materials[i].stem()).unwrap_or_else(|| "(untextured)".into());
        if let Some(w) = &want {
            if !n.contains(w.as_str()) {
                continue;
            }
        }
        per.entry(n).or_default().push(t);
    }
    println!("{:<34} {:>5} {:>8} {:>8} {:>8} {:>6}", "material", "tris", "mean", "within", "across", "lit");
    for (n, tris) in &per {
        let lum = |c: [u8; 4]| (c[0] as f32 * 0.3 + c[1] as f32 * 0.59 + c[2] as f32 * 0.11);
        let mut within = 0.0f32;
        let mut means = Vec::new();
        let mut lit = 0;
        for t in tris {
            let l: Vec<f32> = t.c.iter().map(|k| lum(k.rgba)).collect();
            let (lo, hi) = (l.iter().cloned().fold(f32::MAX, f32::min), l.iter().cloned().fold(f32::MIN, f32::max));
            within += hi - lo;
            means.push((l[0] + l[1] + l[2]) / 3.0);
            if t.lit {
                lit += 1;
            }
        }
        let mean = means.iter().sum::<f32>() / means.len() as f32;
        let across = (means.iter().map(|x| (x - mean).powi(2)).sum::<f32>() / means.len() as f32).sqrt();
        println!("{:<34} {:>5} {:>8.1} {:>8.1} {:>8.1} {:>6}", n, tris.len(), mean, within / tris.len() as f32, across, lit);
    }
}

/// `mk64 ghost COURSE --host MAP --out FILE.Ghost.Gbx --donor GHOST [--laps N] [--vmax KMH]
///             [--uid U] [--alat A] [--aacc A] [--abrk A]`
/// The naive centreline ghost: see `ghost.rs`. `--host` is the same base map the
/// build used (it fixes the course's frame); `--uid` the map's uid (`tmmaps
/// header MAP`), so the client accepts the ghost in play; the donor is any
/// ghost the client loads (a 50 ms record with one vehicle entity).
fn die(msg: impl std::fmt::Display) -> ! {
    eprintln!("{msg}");
    std::process::exit(2)
}

fn cmd_ghost(args: &[String]) {
    let (mut c, decomp) = load_course(args);
    let dir = course_arg(args);
    let out = flag(args, "--out").unwrap_or_else(|| die("--out FILE.Ghost.Gbx"));
    let donor = flag(args, "--donor").unwrap_or_else(|| die("--donor GHOST (a ghost the client loads)"));
    let host = flag(args, "--host").unwrap_or_else(|| die("--host MAP (the base map the build used)"));
    let laps: u32 = flag(args, "--laps").and_then(|s| s.parse().ok()).unwrap_or(3);
    let mut drive = mk64::ghost::Drive::default();
    if let Some(v) = flag(args, "--vmax").and_then(|s| s.parse::<f32>().ok()) {
        drive.vmax = v / 3.6;
    }
    if let Some(v) = flag(args, "--alat").and_then(|s| s.parse::<f32>().ok()) {
        drive.a_lat = v;
    }
    if let Some(v) = flag(args, "--aacc").and_then(|s| s.parse::<f32>().ok()) {
        drive.a_acc = v;
    }
    if let Some(v) = flag(args, "--abrk").and_then(|s| s.parse::<f32>().ok()) {
        drive.a_brk = v;
    }
    let assets = mk64::texture::AssetIndex::load(&decomp).expect("asset index");
    let pieces = c.visual_pieces();
    let coll = c.collision_pieces();
    let kind = mk64::tm::host_kind(&tmmaps::map::MapFile::load(std::path::Path::new(host)));
    let frame = mk64::tm::course_frame(&c, &pieces, &coll, &assets, &kind, args);
    let soup = mesh::collision_mesh(&c, &coll, &frame);
    let p0 = frame.to_tm(c.path[0].pos);
    let road0 = mk64::ghost::road_y(&soup, p0[0], p0[2], p0[1]);
    println!("{dir}: path[0] at ({:.2}, {:.2}, {:.2}), road under it {:?}", p0[0], p0[1], p0[2], road0);
    let traj = mk64::ghost::centreline(&c, &frame, &soup, laps, &drive, 50);
    let vmax_seen = traj.poses.iter().map(|p| p.speed).fold(0.0f32, f32::max);
    println!("  drive: vmax {:.0} km/h, a_lat {} a_acc {} a_brk {}; top speed reached {:.0} km/h; laps at {:?} s", drive.vmax * 3.6, drive.a_lat, drive.a_acc, drive.a_brk, vmax_seen * 3.6, traj.lap_ms.iter().map(|m| *m as f64 / 1000.0).collect::<Vec<_>>());
    // the uid: --uid, else read from --map (the built map's header)
    let uid: Option<String> = match (flag(args, "--uid"), flag(args, "--map")) {
        (Some(u), _) => Some(u.to_string()),
        (None, Some(m)) => {
            let data = std::fs::read(m).unwrap_or_else(|e| die(format!("{m}: {e}")));
            Some(gbx::map_uid_of(&data).unwrap_or_else(|| die(format!("{m}: no map uid in the header"))))
        }
        (None, None) => None,
    };
    if let Some(u) = &uid {
        println!("  map uid {u}");
    }
    match mk64::ghost::write(&traj, donor, out, uid.as_deref(), 50) {
        Ok(r) => println!("  {r}"),
        Err(e) => die(format!("{out}: {e}")),
    }
}

/// `mk64 ghost-all --host MAP --maps-dir DIR --out-dir DIR --donor GHOST [ghost flags]`:
/// one centreline ghost per built map in `--maps-dir` (`MK64 <Title>.Map.Gbx`,
/// the uid read from each), written as `DIR/MK64 <Title>.Ghost.Gbx`.
fn cmd_ghost_all(args: &[String]) {
    let maps_dir = std::path::PathBuf::from(flag(args, "--maps-dir").unwrap_or_else(|| die("--maps-dir DIR (the built maps)")));
    let out_dir = std::path::PathBuf::from(flag(args, "--out-dir").unwrap_or_else(|| die("--out-dir DIR")));
    std::fs::create_dir_all(&out_dir).unwrap_or_else(|e| die(format!("{}: {e}", out_dir.display())));
    let mut passthrough: Vec<String> = Vec::new();
    let mut i = 2;
    while i < args.len() {
        if args[i] == "--maps-dir" || args[i] == "--out-dir" {
            i += 2;
            continue;
        }
        passthrough.push(args[i].clone());
        i += 1;
    }
    let mut built = 0;
    for (dir, title, laps) in mk64::course::COURSES {
        if laps.is_none() {
            continue;
        }
        let file_title = title.replace('\'', "");
        let map = maps_dir.join(format!("MK64 {file_title}.Map.Gbx"));
        if !map.is_file() {
            eprintln!("=== {dir}: no map at {} — skipped", map.display());
            continue;
        }
        let out = out_dir.join(format!("MK64 {file_title}.Ghost.Gbx"));
        let mut a: Vec<String> = vec!["mk64".into(), "ghost".into(), dir.to_string()];
        a.extend(passthrough.iter().cloned());
        a.push("--map".into());
        a.push(map.to_string_lossy().into_owned());
        a.push("--out".into());
        a.push(out.to_string_lossy().into_owned());
        println!("=== {dir} → {}", out.display());
        cmd_ghost(&a);
        built += 1;
    }
    println!("built {built} ghosts into {}", out_dir.display());
}

/// `mk64 mux --webm-dir DIR --music-dir DIR --out-dir DIR [--crf N] [--ffmpeg BIN]`:
/// every `mk64-<course>-1lap.webm` the render batch produced becomes
/// `DIR/MK64 <Title> - centreline ghost, lap 1.mp4` (H.264, ≤ 50 MB for the
/// artifact store) with the course's theme (`mk64_<key>.ogg`, `tm::music_theme`)
/// faded out over the last 3 s. ffmpeg does the work; this picks the files.
fn cmd_mux(args: &[String]) {
    let webm_dir = std::path::PathBuf::from(flag(args, "--webm-dir").unwrap_or_else(|| die("--webm-dir DIR")));
    let music_dir = std::path::PathBuf::from(flag(args, "--music-dir").unwrap_or_else(|| die("--music-dir DIR")));
    let out_dir = std::path::PathBuf::from(flag(args, "--out-dir").unwrap_or_else(|| die("--out-dir DIR")));
    let crf = flag(args, "--crf").unwrap_or("24");
    let ffmpeg = flag(args, "--ffmpeg").unwrap_or("ffmpeg");
    std::fs::create_dir_all(&out_dir).unwrap_or_else(|e| die(format!("{}: {e}", out_dir.display())));
    let mut n = 0;
    for (dir, title, laps) in mk64::course::COURSES {
        if laps.is_none() {
            continue;
        }
        let stem = if *dir == "luigi_raceway" { "mk64-luigi-1lap".to_string() } else { format!("mk64-{}-1lap", dir.replace('_', "-")) };
        let webm = webm_dir.join(format!("{stem}.webm"));
        if !webm.is_file() {
            println!("{dir}: no {} yet", webm.display());
            continue;
        }
        let out = out_dir.join(format!("MK64 {} - centreline ghost, lap 1.mp4", title.replace('\'', "")));
        if out.is_file() {
            println!("{dir}: {} exists — kept", out.display());
            n += 1;
            continue;
        }
        // the clip's length decides where the fade starts
        let probe = std::process::Command::new(ffmpeg.replace("ffmpeg", "ffprobe"))
            .args(["-v", "error", "-show_entries", "format=duration", "-of", "csv=p=0"])
            .arg(&webm)
            .output()
            .unwrap_or_else(|e| die(format!("ffprobe: {e}")));
        let dur: f64 = String::from_utf8_lossy(&probe.stdout).trim().parse().unwrap_or(0.0);
        let fade_at = (dur - 3.0).max(0.0);
        let music = mk64::tm::music_theme(dir).map(|k| music_dir.join(format!("mk64_{k}.ogg"))).filter(|p| p.is_file());
        let mut cmd = std::process::Command::new(ffmpeg);
        cmd.args(["-y", "-loglevel", "error", "-i"]).arg(&webm);
        match &music {
            Some(m) => {
                cmd.arg("-i").arg(m).args(["-filter_complex", &format!("[1:a]afade=t=out:st={fade_at:.2}:d=3[a]"), "-map", "0:v", "-map", "[a]", "-c:a", "aac", "-b:a", "128k", "-shortest"]);
            }
            None => {
                cmd.args(["-map", "0:v", "-an"]);
            }
        }
        cmd.args(["-c:v", "libx264", "-preset", "slow", "-crf", crf, "-pix_fmt", "yuv420p", "-movflags", "+faststart"]).arg(&out);
        let st = cmd.status().unwrap_or_else(|e| die(format!("ffmpeg: {e}")));
        if !st.success() {
            die(format!("ffmpeg failed on {}", webm.display()));
        }
        let bytes = std::fs::metadata(&out).map(|m| m.len()).unwrap_or(0);
        println!("{dir}: {} ({:.1} MB, {dur:.1} s{})", out.display(), bytes as f64 / 1048576.0, if music.is_some() { ", music" } else { ", silent" });
        if bytes > 50 * 1024 * 1024 {
            println!("  over the 50 MB artifact limit — re-run with --crf {}", crf.parse::<u32>().unwrap_or(24) + 3);
        }
        n += 1;
    }
    println!("{n} clips in {}", out_dir.display());
}

/// `mk64 routes COURSE`: where the course's alternate routes (`_track_path_2..`)
/// diverge from the main path — the lap fractions along the main path where
/// EVERY route passes within 25 units (a checkpoint there catches every
/// branch) and where they do not.
fn cmd_routes(args: &[String]) {
    let (c, _decomp) = load_course(args);
    let dir = course_arg(args);
    let main: Vec<[f32; 3]> = c.path.iter().map(|p| [p.pos[0] as f32, p.pos[1] as f32, p.pos[2] as f32]).collect();
    let mut alts: Vec<(&String, &Vec<[i16; 3]>)> = c.other_paths.iter().filter(|(n, _)| n.contains("_track_path_")).collect();
    alts.sort();
    println!("{dir}: main path {} points; {} alternate route(s): {}", main.len(), alts.len(), alts.iter().map(|(n, v)| format!("{} ({} pts)", n.rsplit('_').next().unwrap_or("?"), v.len())).collect::<Vec<_>>().join(", "));
    if alts.is_empty() {
        return;
    }
    // per main point: the nearest distance from each alternate route
    let n = main.len();
    let mut shared = vec![true; n];
    for (_, alt) in &alts {
        for (i, p) in main.iter().enumerate() {
            let d = alt.iter().map(|q| ((q[0] as f32 - p[0]).powi(2) + (q[2] as f32 - p[2]).powi(2)).sqrt()).fold(f32::MAX, f32::min);
            if d > 25.0 {
                shared[i] = false;
            }
        }
    }
    // runs
    let mut i = 0;
    while i < n {
        let s = shared[i];
        let j0 = i;
        while i < n && shared[i] == s {
            i += 1;
        }
        println!("  {:>5.1}% .. {:>5.1}%  {}", j0 as f32 * 100.0 / n as f32, i as f32 * 100.0 / n as f32, if s { "all routes together" } else { "ROUTES SPLIT" });
    }
    let cps = 4;
    for k in 1..=cps {
        let idx = (k * n) / (cps + 1);
        println!("  checkpoint {k} at {:>5.1}%: {}", idx as f32 * 100.0 / n as f32, if shared[idx] { "ok (all routes pass)" } else { "MISSED by some route" });
    }
}

/// `mk64 clones --map MAP --ghost GHOST.Ghost.Gbx --out OUT [--n 7] [--skin "MK64 Bowser.zip"]
/// [--name Bowser] [--near 2.5 --far 9.5 --slow-near 1.35 --slow-far 1.08]`:
/// the map with the ghost as its validation ghost and `n` collidable clones of
/// it in solo play (the January 2026 Clones feature). The author time becomes
/// the ghost's; gold/silver/bronze ×1.06/1.2/1.5.
fn cmd_clones(args: &[String]) {
    let map = flag(args, "--map").unwrap_or_else(|| die("--map MAP.Map.Gbx"));
    let ghost_path = flag(args, "--ghost").unwrap_or_else(|| die("--ghost GHOST.Ghost.Gbx"));
    let out = flag(args, "--out").unwrap_or_else(|| die("--out OUT.Map.Gbx"));
    let n: usize = flag(args, "--n").and_then(|s| s.parse().ok()).unwrap_or(7);
    let near: f32 = flag(args, "--near").and_then(|s| s.parse().ok()).unwrap_or(2.5);
    let far: f32 = flag(args, "--far").and_then(|s| s.parse().ok()).unwrap_or(9.5);
    let slow_near: f32 = flag(args, "--slow-near").and_then(|s| s.parse().ok()).unwrap_or(1.35);
    let slow_far: f32 = flag(args, "--slow-far").and_then(|s| s.parse().ok()).unwrap_or(1.08);
    let skin = flag(args, "--skin");
    let name = flag(args, "--name");

    // the ghost: skin + name rewritten, its splits = the waypoint times
    let c = ghost::Container::load(&ghost_path).unwrap_or_else(|e| die(format!("{ghost_path}: {e}")));
    let mut body = c.body().to_vec();
    let fields = ghost::ident::scan(&c);
    let mut edits: Vec<(usize, usize, Vec<u8>)> = Vec::new();
    for f in &fields {
        match f.role {
            ghost::ident::Role::Skin if skin.is_some() => edits.push((f.at, f.len, format!("Skins\\Models\\CarSport\\{}", skin.as_deref().unwrap()).into_bytes())),
            ghost::ident::Role::Locator if skin.is_some() => edits.push((f.at, f.len, Vec::new())),
            ghost::ident::Role::Nickname if name.is_some() => edits.push((f.at, f.len, name.as_deref().unwrap().as_bytes().to_vec())),
            _ => {}
        }
    }
    if !edits.is_empty() {
        body = gbx::container::replace_strings(&body, &edits, None).unwrap_or_else(|e| die(format!("ghost strings: {e}")));
    }
    if skin.is_some() {
        // a local skin: zero checksum (the 32 bytes before the skin path's length word)
        if let Some(f) = fields.iter().find(|f| f.role == ghost::ident::Role::Skin) {
            let at = f.at - 32;
            for b in &mut body[at..at + 32] {
                *b = 0;
            }
        }
    }
    let c2 = ghost::Container { path: c.path.clone(), gbx: gbx::container::Gbx { body: body.clone(), ..c.gbx.clone() } };
    let splits = c2.splits();
    let race_ms = c2.declared_times().first().map(|d| d.1).unwrap_or(0);
    if splits.is_empty() || race_ms == 0 {
        die(format!("{ghost_path}: no split list / declared time (rebuild the ghost with the current `mk64 ghost`)"));
    }
    println!("ghost: {} waypoints, race {:.3} s, {} clones", splits.len(), race_ms as f32 / 1000.0, n);

    // the map
    let mut m = tmmaps::map::MapFile::load(std::path::Path::new(&map));
    // --probe ghost|meta: only one of the two edits (which one breaks a load)
    let probe = flag(args, "--probe");
    let blob = if has(args, "--dummy-ghost") { tmmaps::map::DUMMY_GHOST.to_vec() } else { mk64::clones::ghost_blob(&body).unwrap_or_else(|e| die(e)) };
    if probe.as_deref() != Some("meta") {
        mk64::clones::set_skip_chunk(&mut m, 0x0305_B00F, &blob).unwrap_or_else(|e| die(e));
    }
    let clones = mk64::clones::schedule(n, near, far, slow_near, slow_far);
    let meta = mk64::clones::metadata_payload(true, &splits, &clones);
    if probe.as_deref() != Some("ghost") {
        mk64::clones::set_skip_chunk(&mut m, 0x0304_3044, &meta).unwrap_or_else(|e| die(e));
    }
    // laps from the body chunk 0x03043018; checkpoints per lap = splits / laps
    let laps = tmmaps::gbx::all_skip_chunks(&m.gbx.body)
        .iter()
        .find(|c| c.0 == 0x0304_3018)
        .map(|&(_, _, p, _)| {
            let lap_race = u32::from_le_bytes(m.gbx.body[p..p + 4].try_into().unwrap());
            let nb = u32::from_le_bytes(m.gbx.body[p + 4..p + 8].try_into().unwrap());
            if lap_race != 0 { nb.max(1) } else { 1 }
        })
        .unwrap_or(1);
    let per_lap = (splits.len() as u32 / laps).max(1);
    let times = [race_ms, (race_ms as f32 * 1.06) as u32, (race_ms as f32 * 1.2) as u32, (race_ms as f32 * 1.5) as u32];
    mk64::clones::set_header_times(&mut m, times, laps, per_lap);
    // the body medal chunk 0x0305B00A: tip string, bronze, silver, gold, author
    if let Some(&(_, _, payload, size)) = tmmaps::gbx::all_skip_chunks(&m.gbx.body).iter().find(|c| c.0 == 0x0305_B00A) {
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
        Some(format!("{}<times bronze=\"{br}\" silver=\"{s}\" gold=\"{g}\" authortime=\"{a}\" authorscore=\"0\" hasclones=\"1\"/>{}", &x[..i], &x[j..]))
    });
    m.edit_header_xml(&|x: &str| {
        let i = x.find("validated=\"")?;
        Some(format!("{}validated=\"1{}", &x[..i], &x[i + 12..]))
    });
    m.write_to(std::path::Path::new(&out)).unwrap_or_else(|e| die(format!("{out}: {e}")));
    let back = tmmaps::map::MapFile::load(std::path::Path::new(&out));
    let has = |id: u32| tmmaps::gbx::all_skip_chunks(&back.gbx.body).iter().find(|c| c.0 == id).map(|c| c.3);
    println!("{out}: validation ghost {} bytes, metadata {} bytes, author {:.3} s, {laps} laps × {per_lap} waypoints; clones {:?}", has(0x0305_B00F).unwrap_or(0), has(0x0304_3044).unwrap_or(0), a as f32 / 1000.0, clones.iter().map(|c| format!("{:+.1}s ×{:.2}", c.offset_ms as f32 / 1000.0, c.slow_e6 as f32 / 1e6)).collect::<Vec<_>>());
}

/// Rewrite a ghost file's identity: skin (local zip in `Skins\Models\CarSport`,
/// zero checksum, no locator) and display name. In place.
fn ghost_identity(path: &str, skin: &str, name: &str, skin_dir: Option<&std::path::Path>, locator: Option<&str>) -> Result<(), String> {
    let c = ghost::Container::load(path)?;
    let mut body = c.body().to_vec();
    let fields = ghost::ident::scan(&c);
    let mut edits: Vec<(usize, usize, Vec<u8>)> = Vec::new();
    // The PackDesc checksum is the zip's SHA-256: the play-mode ghost loader
    // (Ghost_Download) REFUSED a ghost with a zeroed checksum ("Unable to load
    // ghost file", 2026-09-25 bisect: path+name edits alone load; the zeroed
    // checksum alone fails) — so it is computed from the skin file when the
    // directory is given, else the donor's stays. The locator: `locator` when
    // given (a URL the game downloads the zip from), else the donor's stays.
    for f in &fields {
        match f.role {
            ghost::ident::Role::Skin => edits.push((f.at, f.len, format!("Skins\\Models\\CarSport\\{skin}").into_bytes())),
            ghost::ident::Role::Locator => {
                if let Some(u) = locator {
                    edits.push((f.at, f.len, u.as_bytes().to_vec()));
                }
            }
            ghost::ident::Role::Nickname => edits.push((f.at, f.len, name.as_bytes().to_vec())),
            _ => {}
        }
    }
    if let (Some(dir), Some(f)) = (skin_dir, fields.iter().find(|f| f.role == ghost::ident::Role::Skin)) {
        let zip = dir.join(skin);
        let sum = gbx::sha::sha256_file(&zip).map_err(|e| format!("{}: {e}", zip.display()))?;
        // `f.at` is the path's LENGTH word; the PackDesc is `u8 version(3)`,
        // 32-byte checksum, then the path — the checksum ends right at `f.at`
        // (an earlier `f.at - 36` zeroed the version byte: "Unable to load
        // ghost file")
        let at = f.at - 32;
        if body[at - 1] != 3 {
            return Err(format!("{path}: PackDesc version byte {} at {} (expected 3)", body[at - 1], at - 1));
        }
        body[at..at + 32].copy_from_slice(&sum);
    }
    let body = gbx::container::replace_strings(&body, &edits, None)?;
    write_gbx_compressed(&c.gbx, &body, path)
}

/// A Gbx with an LZO-COMPRESSED body ('C'): the game's `Ghost_Download`
/// loader answered "Unable to load ghost file" to our uncompressed ('U')
/// ghosts and loaded a game-written compressed one (2026-09-25); the
/// MediaTracker import takes both.
fn write_gbx_compressed(g: &gbx::container::Gbx, body: &[u8], out: &str) -> Result<(), String> {
    let mut file = g.header_bytes_u();
    // the body-compression byte: "GBX" + u16 version + format + ref_comp + BODY
    file[7] = b'C';
    let comp = tmmaps::gbx::lzo_compress(body);
    file.extend_from_slice(&(body.len() as u32).to_le_bytes());
    file.extend_from_slice(&(comp.len() as u32).to_le_bytes());
    file.extend_from_slice(&comp);
    std::fs::write(out, file).map_err(|e| format!("{out}: {e}"))
}

/// `mk64 cpus COURSE --host MAP --out-dir DIR --donor GHOST [--map BUILT.Map.Gbx]
/// [--player mario] [--laps 3] [--vmax KMH] [--half-width M]`: the seven MK64 CPU
/// racers as ghosts (`<Course> - <Driver>.Ghost.Gbx`), each wearing its
/// character's skin — routes, lateral behaviours and drift zones from the
/// game's tables (see `cpus.rs`).
fn cmd_cpus(args: &[String]) {
    let (mut c, decomp) = load_course(args);
    let dir = course_arg(args);
    let out_dir = std::path::PathBuf::from(flag(args, "--out-dir").unwrap_or_else(|| die("--out-dir DIR")));
    let donor = flag(args, "--donor").unwrap_or_else(|| die("--donor GHOST"));
    let host = flag(args, "--host").unwrap_or_else(|| die("--host MAP"));
    let laps: u32 = flag(args, "--laps").and_then(|s| s.parse().ok()).unwrap_or(3);
    let player_name = flag(args, "--player").unwrap_or("mario").to_lowercase();
    let player = mk64::cpus::DRIVERS.iter().position(|d| d.0.to_lowercase().starts_with(&player_name)).unwrap_or(0);
    let half_width: f32 = flag(args, "--half-width").and_then(|s| s.parse().ok()).unwrap_or(4.0);
    let mut base = mk64::ghost::Drive::default();
    if let Some(v) = flag(args, "--vmax").and_then(|s| s.parse::<f32>().ok()) {
        base.vmax = v / 3.6;
    }
    // --skin-dir DIR: the skin zips (their SHA-256 goes into the PackDesc);
    // --skin-url-base URL: where the game downloads `<zip>` from (the locator);
    // without it the donor's locator stays (a local skin of the same name wins)
    let skin_dir = flag(args, "--skin-dir").map(std::path::PathBuf::from);
    let url_base = flag(args, "--skin-url-base").map(|s| s.to_string());
    let locator_for = |zip: &str| -> Option<String> { url_base.as_ref().map(|b| format!("{}{}", b, zip.replace(' ', "%20"))) };
    let rom_path = std::env::var("MK64_ROM").unwrap_or_else(|_| die("MK64_ROM=/path/to/baserom.us.z64"));
    let rom = std::fs::read(&rom_path).unwrap_or_else(|e| die(format!("{rom_path}: {e}")));
    let assets = mk64::texture::AssetIndex::load(&decomp).expect("asset index");
    let pieces = c.visual_pieces();
    let coll = c.collision_pieces();
    let kind = mk64::tm::host_kind(&tmmaps::map::MapFile::load(std::path::Path::new(host)));
    let frame = mk64::tm::course_frame(&c, &pieces, &coll, &assets, &kind, args);
    let soup = mesh::collision_mesh(&c, &coll, &frame);
    let uid: Option<String> = flag(args, "--map").map(|m| {
        let data = std::fs::read(m).unwrap_or_else(|e| die(format!("{m}: {e}")));
        gbx::map_uid_of(&data).unwrap_or_else(|| die(format!("{m}: no map uid in the header")))
    });
    let beh = mk64::cpus::behaviours(&rom, &dir);
    let n_routes = mk64::cpus::routes(&c, &frame).len();
    println!("{dir}: {} route(s), {} CPU behaviour rows ({} drift zones, {} lateral); human = {}", n_routes, beh.len(), beh.iter().filter(|b| b.kind == mk64::cpus::BEHAVIOUR_DRIFT).count(), beh.iter().filter(|b| matches!(b.kind, 3..=5)).count(), mk64::cpus::DRIVERS[player].0);
    std::fs::create_dir_all(&out_dir).unwrap_or_else(|e| die(format!("{}: {e}", out_dir.display())));
    let title = mk64::course::COURSES.iter().find(|x| x.0 == dir).map(|x| x.1.to_string()).unwrap_or_else(|| dir.clone());
    for cpu in mk64::cpus::field(&c, &frame, &rom, player, &base, half_width) {
        let traj = mk64::cpus::trajectory(&c, &frame, &soup, &cpu, laps, 50);
        let out = out_dir.join(format!("MK64 {title} - {}.Ghost.Gbx", cpu.name));
        let out_s = out.to_string_lossy().to_string();
        match mk64::ghost::write(&traj, donor, &out_s, uid.as_deref(), 50) {
            Ok(_) => {}
            Err(e) => die(format!("{out_s}: {e}")),
        }
        ghost_identity(&out_s, cpu.skin, cpu.name, skin_dir.as_deref(), locator_for(cpu.skin).as_deref()).unwrap_or_else(|e| die(format!("{out_s}: {e}")));
        println!("  {:<12} route {} grid +{:.0} m lat {:+.1} m vmax {:.0} km/h — laps at {:?} s", cpu.name, cpu.route, cpu.line.start_ahead_m, cpu.line.base_lateral, cpu.drive.vmax * 3.6, traj.lap_ms.iter().map(|m| (*m as f64 / 100.0).round() / 10.0).collect::<Vec<_>>());
    }
}

/// `mk64 ghost-compress IN OUT`: the same ghost with an LZO-compressed body.
fn cmd_ghost_compress(args: &[String]) {
    let inp = args.get(2).unwrap_or_else(|| die("ghost-compress IN OUT"));
    let out = args.get(3).unwrap_or_else(|| die("ghost-compress IN OUT"));
    let c = ghost::Container::load(inp).unwrap_or_else(|e| die(format!("{inp}: {e}")));
    write_gbx_compressed(&c.gbx, c.body(), out).unwrap_or_else(|e| die(e));
    println!("{out}: {} bytes", std::fs::metadata(out).map(|m| m.len()).unwrap_or(0));
}
