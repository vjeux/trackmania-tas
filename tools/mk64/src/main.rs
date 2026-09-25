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
        "colours" => cmd_colours(&args),
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
    match mk64::ghost::write(&traj, donor, out, flag(args, "--uid"), 50) {
        Ok(r) => println!("  {r}"),
        Err(e) => die(format!("{out}: {e}")),
    }
}
