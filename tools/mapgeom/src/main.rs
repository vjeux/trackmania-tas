//! `mapgeom` — get real 3D geometry out of a TM2020 map.

use mapgeom::node::{Node, Slot};
use mapgeom::{names, store::DataStore, store::STADIUM_KEY};

/// How tightly a run's ride height has to agree for a candidate map height
/// to count as a fit. A car on a road varies by centimetres; 0.30 m is loose
/// enough for a bumpy surface and far tighter than the 8 m between candidates.
const BAND: f32 = 0.30;

const USAGE: &str = "\
mapgeom -- TM2020 map geometry

  --packs <dir>     directory holding the game's .pak files
                    (default $TM_SERVER/Packs, else /tmp/tmoracle/server/Packs)
  --key <hex>       Stadium pack key (default: the known one)

COMMANDS
  ls [<substring>]              pack entries whose path contains <substring>
  resolve <logical-path>        which pack entry a logical path is stored under
  refs <logical-path>           a file's external reference table
  dump <logical-path>           walk a file's node graph and summarise it
  model <logical-path> --out F  a single file's geometry, as .glb or .obj
  map <file.Map.Gbx> --out F [--yoff N] [--no-items] [--ghost G]... [--png P]
                                a whole map, with any ghosts as polylines
";

struct Args {
    packs: String,
    key: String,
    rest: Vec<String>,
}

fn parse_args() -> Args {
    let mut packs = std::env::var("TM_SERVER")
        .map(|s| format!("{}/Packs", s))
        .unwrap_or_else(|_| "/tmp/tmoracle/server/Packs".to_string());
    let mut key = STADIUM_KEY.to_string();
    let mut rest = Vec::new();
    let mut it = std::env::args().skip(1);
    while let Some(a) = it.next() {
        match a.as_str() {
            "--packs" => packs = it.next().unwrap_or_default(),
            "--key" => key = it.next().unwrap_or_default(),
            _ => rest.push(a),
        }
    }
    Args { packs, key, rest }
}

/// A logical pack path, or -- when the name exists on disk -- a local file.
/// Lets `dump` and `model` be pointed at an embedded model that has been
/// pulled out of a map with `items --out`.
fn load_any(store: &mut DataStore, name: &str) -> mapgeom::store::Model {
    if std::path::Path::new(name).is_file() {
        let bytes = std::fs::read(name).unwrap_or_else(|e| die(e.to_string()));
        return mapgeom::store::Model::parse(&bytes, name).unwrap_or_else(die);
    }
    store.load_model(name).unwrap_or_else(die)
}

fn open(a: &Args) -> DataStore {
    let mut paths: Vec<String> = Vec::new();
    for name in ["dedicated_TMStadium.pak", "dedicated.pak", "resource.pak"] {
        let p = format!("{}/{}", a.packs, name);
        if std::path::Path::new(&p).exists() {
            paths.push(p);
        }
    }
    if paths.is_empty() {
        eprintln!("no .pak files in {}", a.packs);
        std::process::exit(2);
    }
    match DataStore::open(&paths, &a.key) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("{}", e);
            std::process::exit(2);
        }
    }
}

fn main() {
    let a = parse_args();
    let cmd = a.rest.first().cloned().unwrap_or_default();
    match cmd.as_str() {
        "ls" => {
            let store = open(&a);
            let pat = a.rest.get(1).cloned().unwrap_or_default().to_uppercase();
            let mut n = 0;
            for e in store.entries() {
                let p = e.path();
                if pat.is_empty() || p.to_uppercase().contains(&pat) {
                    println!("{}\tclass 0x{:08X}\t{} bytes", p, e.class_id, e.uncompressed_size);
                    n += 1;
                }
            }
            eprintln!("{} entries", n);
        }
        "resolve" => {
            let store = open(&a);
            let p = a.rest.get(1).cloned().unwrap_or_default();
            match store.resolve(&p) {
                Some(hit) => println!("{}\n  -> {}", p, hit),
                None => {
                    println!("{}\n  NOT FOUND; tried:", p);
                    for c in names::candidates(&p) {
                        println!("    {}", c);
                    }
                    std::process::exit(1);
                }
            }
        }
        "refs" => {
            let mut store = open(&a);
            let p = a.rest.get(1).cloned().unwrap_or_default();
            let m = load_any(&mut store, &p);
            println!("{}  class 0x{:08X}  {} nodes", m.path, m.class_id, m.num_nodes);
            for (idx, path) in &m.externals {
                let hit = store.resolve(path);
                println!(
                    "  node {:>4}  {}  {}",
                    idx,
                    path,
                    match hit {
                        Some(h) if h == *path => "(stored by name)".to_string(),
                        Some(h) => format!("-> {}", h),
                        None => "*** NOT IN PACK ***".to_string(),
                    }
                );
            }
        }
        "dump" => {
            let mut store = open(&a);
            let p = a.rest.get(1).cloned().unwrap_or_default();
            let m = load_any(&mut store, &p);
            let g = m.graph().unwrap_or_else(die);
            println!("{}  class 0x{:08X}  {} nodes", m.path, m.class_id, m.num_nodes);
            if let Some(root) = &g.root {
                println!("  root: {}", describe(root));
            }
            for (i, s) in g.slots.iter().enumerate() {
                match s {
                    Slot::Node(n) => println!("  [{:>4}] {}", i, describe(n)),
                    Slot::External(p) => println!("  [{:>4}] external {}", i, p),
                    _ => {}
                }
            }
        }
        "model" => {
            let mut store = open(&a);
            let p = a.rest.get(1).cloned().unwrap_or_default();
            let out = flag(&a.rest, "--out").unwrap_or_else(|| "model.glb".to_string());
            let loaded = load_any(&mut store, &p);
            let mut c = mapgeom::geom::Collector::new(&mut store);
            c.model(&loaded, &mapgeom::geom::IDENTITY, 0);
            report(&c.stats, &c.scene);
            write_scene(&c.scene, &out);
        }
        "map" => {
            let mut store = open(&a);
            let p = a.rest.get(1).cloned().unwrap_or_default();
            let out = flag(&a.rest, "--out").unwrap_or_else(|| "map.glb".to_string());
            let yoff: f32 = flag(&a.rest, "--yoff").and_then(|s| s.parse().ok()).unwrap_or(0.0);
            let with_items = !a.rest.iter().any(|x| x == "--no-items");
            let m = tmmaps::map::MapFile::load(std::path::Path::new(&p));
            let mut asm = mapgeom::assemble::Assembler::new(&mut store);
            match asm.with_embedded(&m) {
                Ok(0) => {}
                Ok(n) => println!("  {} models embedded in the map itself", n),
                Err(e) => eprintln!("  embedded models: {}", e),
            }
            let mut scene = asm.map(&m, yoff, with_items);
            if !a.rest.iter().any(|x| x == "--no-deco") {
                if let Some((path, deco)) = asm.decoration(&m, yoff) {
                    println!("  decoration {}: {} triangles", path, deco.tri_count());
                    scene.append(&deco, &mapgeom::geom::IDENTITY);
                }
            }
            println!("{}: {} blocks, {} items, yoff {}", p, m.blocks.len(), m.items.len(), yoff);
            let mut miss: Vec<(&String, usize)> = asm
                .used
                .iter()
                .filter(|(_, (_, ok))| !*ok)
                .map(|(k, (n, _))| (k, *n))
                .collect();
            miss.sort_by_key(|(_, n)| std::cmp::Reverse(*n));
            if !miss.is_empty() {
                let total: usize = miss.iter().map(|(_, n)| *n).sum();
                println!("  {} placements of {} models had no geometry:", total, miss.len());
                for (name, n) in miss.iter().take(15) {
                    println!("    {:>5} x {}", n, name);
                }
            }
            let stats = std::mem::take(&mut asm.stats);
            for g in ghost_lines(&a.rest) {
                scene.add_line(&g.0, g.1, g.2);
            }
            report(&stats, &scene);
            write_scene(&scene, &out);
            if let Some(png) = flag(&a.rest, "--png") {
                // Clip just above the highest point the run reached, so the
                // stadium roof does not become the picture.
                let clip = flag(&a.rest, "--clip-y")
                    .and_then(|s| s.parse::<f32>().ok())
                    .unwrap_or_else(|| {
                        scene
                            .lines
                            .iter()
                            .flat_map(|l| l.points.iter())
                            .map(|p| p[1])
                            .fold(f32::NEG_INFINITY, f32::max)
                            + 8.0
                    });
                let img = mapgeom::render::top_down(&scene, 1.0, 4000, clip);
                std::fs::write(&png, mapgeom::render::png(&img))
                    .unwrap_or_else(|e| die(e.to_string()));
                println!("wrote {} ({} x {} px)", png, img.w, img.h);
            }
        }
        "check" => {
            let mut store = open(&a);
            let p = a.rest.get(1).cloned().unwrap_or_default();
            let with_items = !a.rest.iter().any(|x| x == "--no-items");
            let reach: f32 = flag(&a.rest, "--reach").and_then(|s| s.parse().ok()).unwrap_or(6.0);
            let m = tmmaps::map::MapFile::load(std::path::Path::new(&p));
            let ghosts = ghost_lines(&a.rest);
            if ghosts.is_empty() {
                die::<()>("check needs at least one --ghost".into());
            }
            let deco = !a.rest.iter().any(|x| x == "--no-deco");
            let fit = a.rest.iter().any(|x| x == "--fit-yoff");
            let given: Option<f32> = flag(&a.rest, "--yoff").and_then(|s| s.parse().ok());
            let offsets: Vec<f32> = if fit || given.is_none() {
                mapgeom::place::Yoff::candidates().collect()
            } else {
                vec![given.unwrap()]
            };
            let mut best: Option<(f32, f32, usize)> = None;
            for yoff in offsets {
                let mut asm = mapgeom::assemble::Assembler::new(&mut store);
                let _ = asm.with_embedded(&m);
            match asm.with_embedded(&m) {
                Ok(0) => {}
                Ok(n) => println!("  {} models embedded in the map itself", n),
                Err(e) => eprintln!("  embedded models: {}", e),
            }
                let mut scene = asm.map(&m, yoff, with_items);
                if deco {
                    if let Some((_, d)) = asm.decoration(&m, yoff) {
                        scene.append(&d, &mapgeom::geom::IDENTITY);
                    }
                }
                let idx = mapgeom::probe::Index::build(&scene, 32.0);
                let mut score = 0usize;
                let mut centre = f32::NAN;
                for (_, pts, _) in &ghosts {
                    let r = mapgeom::probe::Report::of(&idx, pts, reach);
                    let (n, c) = r.band(BAND);
                    score += n;
                    if centre.is_nan() {
                        centre = c;
                    }
                }
                if offsets_are_many(&a.rest) && score > 0 {
                    println!(
                        "  yoff {:>6.0}  {} samples at a common ride height of {:.3} m",
                        yoff, score, centre
                    );
                }
                if best.map_or(true, |(_, _, bs)| score > bs) {
                    best = Some((yoff, centre, score));
                }
            }
            let yoff = best.unwrap().0;
            let mut asm = mapgeom::assemble::Assembler::new(&mut store);
            match asm.with_embedded(&m) {
                Ok(0) => {}
                Ok(n) => println!("  {} models embedded in the map itself", n),
                Err(e) => eprintln!("  embedded models: {}", e),
            }
            let mut scene = asm.map(&m, yoff, with_items);
            if deco {
                if let Some((p, d)) = asm.decoration(&m, yoff) {
                    println!("  decoration {}: {} triangles", p, d.tri_count());
                    scene.append(&d, &mapgeom::geom::IDENTITY);
                }
            }
            let idx = mapgeom::probe::Index::build(&scene, 32.0);
            println!(
                "{}\n  yoff {}  ({} triangles indexed)",
                p,
                yoff,
                idx.triangle_count()
            );
            for (name, pts, _) in &ghosts {
                let r = mapgeom::probe::Report::of(&idx, pts, reach);
                println!(
                    "  {}: {}/{} samples over a surface ({:.1} %)",
                    name,
                    r.hits,
                    r.samples,
                    100.0 * r.hits as f32 / r.samples.max(1) as f32
                );
                if r.hits > 0 {
                    println!(
                        "    gap below the car   median {:.3} m   p10 {:.3}   p90 {:.3}   \
                         tightest half-window +/-{:.3} m",
                        r.median(),
                        r.pct(0.10),
                        r.pct(0.90),
                        r.tightest_half()
                    );
                    let mut mats: Vec<(&String, &usize)> = r.materials.iter().collect();
                    mats.sort_by_key(|(_, n)| std::cmp::Reverse(**n));
                    let line: Vec<String> = mats
                        .iter()
                        .take(6)
                        .map(|(m, n)| {
                            format!("{} {:.0}%", m, 100.0 * **n as f32 / r.hits as f32)
                        })
                        .collect();
                    println!("    driven over         {}", line.join(", "));
                }
            }
        }
        "plumb" => {
            let mut store = open(&a);
            let p = a.rest.get(1).cloned().unwrap_or_default();
            let yoff: f32 = flag(&a.rest, "--yoff").and_then(|s| s.parse().ok()).unwrap_or(0.0);
            let at: Vec<f32> = flag(&a.rest, "--at")
                .unwrap_or_default()
                .split(',')
                .filter_map(|s| s.trim().parse().ok())
                .collect();
            if at.len() < 2 {
                die::<()>("plumb needs --at X,Z (or X,Y,Z)".into());
            }
            let (x, z) = if at.len() >= 3 { (at[0], at[2]) } else { (at[0], at[1]) };
            let m = tmmaps::map::MapFile::load(std::path::Path::new(&p));
            let mut asm = mapgeom::assemble::Assembler::new(&mut store);
            match asm.with_embedded(&m) {
                Ok(0) => {}
                Ok(n) => println!("  {} models embedded in the map itself", n),
                Err(e) => eprintln!("  embedded models: {}", e),
            }
            let mut scene = asm.map(&m, yoff, !a.rest.iter().any(|x| x == "--no-items"));
            if !a.rest.iter().any(|x| x == "--no-deco") {
                if let Some((_, d)) = asm.decoration(&m, yoff) {
                    scene.append(&d, &mapgeom::geom::IDENTITY);
                }
            }
            let idx = mapgeom::probe::Index::build(&scene, 32.0);
            let col = idx.column(x, z);
            println!("column at x {} z {} (yoff {}): {} surfaces", x, z, yoff, col.len());
            for (y, mat) in col.iter().take(40) {
                println!("  y {:>10.3}   {}", y, mat);
            }
        }
        "items" => {
            let p = a.rest.get(1).cloned().unwrap_or_default();
            let m = tmmaps::map::MapFile::load(std::path::Path::new(&p));
            let files = mapgeom::embedded::files(&m).unwrap_or_else(die);
            if let Some(dir) = flag(&a.rest, "--out") {
                for (name, bytes) in &files {
                    let base = name.rsplit(['/', '\\']).next().unwrap_or(name);
                    let p = format!("{}/{}", dir, base);
                    std::fs::create_dir_all(&dir).ok();
                    std::fs::write(&p, bytes).unwrap_or_else(|e| die(e.to_string()));
                }
                println!("extracted {} files to {}", files.len(), dir);
            }
            println!("{}: {} embedded files", p, files.len());
            for (name, bytes) in &files {
                println!("  {:>9} bytes  {}", bytes.len(), name);
            }
        }
        "extract" => {
            let mut store = open(&a);
            let p = a.rest.get(1).cloned().unwrap_or_default();
            let out = a.rest.get(2).cloned().unwrap_or_else(|| "out.bin".to_string());
            let bytes = store.read(&p).unwrap_or_else(die);
            std::fs::write(&out, &bytes).unwrap_or_else(|e| die(e.to_string()));
            println!("{} -> {} ({} bytes)", p, out, bytes.len());
        }
        _ => {
            print!("{}", USAGE);
            std::process::exit(2);
        }
    }
}

fn offsets_are_many(args: &[String]) -> bool {
    args.iter().any(|x| x == "--fit-yoff") || !args.iter().any(|x| x == "--yoff")
}

/// `--ghost F` (repeatable): a driven trajectory as a polyline in the same
/// world frame as the model, so the two can be looked at together.
fn ghost_lines(args: &[String]) -> Vec<(String, Vec<[f32; 3]>, [f32; 4])> {
    const COLOURS: [[f32; 4]; 4] = [
        [1.0, 0.15, 0.15, 1.0],
        [1.0, 0.85, 0.10, 1.0],
        [0.20, 1.00, 0.35, 1.0],
        [0.95, 0.25, 0.95, 1.0],
    ];
    let mut out = Vec::new();
    for (i, p) in args
        .iter()
        .enumerate()
        .filter(|(_, a)| *a == "--ghost")
        .filter_map(|(i, _)| args.get(i + 1))
        .enumerate()
    {
        match gbx::decode_ghost(p) {
            Ok(d) => {
                let pts: Vec<[f32; 3]> =
                    d.samples.iter().map(|s| [s.x as f32, s.y as f32, s.z as f32]).collect();
                let name = std::path::Path::new(p)
                    .file_stem()
                    .map(|s| s.to_string_lossy().to_string())
                    .unwrap_or_else(|| "ghost".to_string());
                println!(
                    "  ghost {}: {} samples, first {:?}, last {:?}",
                    name,
                    pts.len(),
                    pts.first().copied().unwrap_or_default(),
                    pts.last().copied().unwrap_or_default()
                );
                out.push((format!("path_{}", name), pts, COLOURS[i % COLOURS.len()]));
            }
            Err(e) => eprintln!("  ghost {}: {}", p, e),
        }
    }
    out
}

fn die<T>(e: String) -> T {
    eprintln!("{}", e);
    std::process::exit(1);
}

fn flag(args: &[String], name: &str) -> Option<String> {
    args.iter().position(|a| a == name).and_then(|i| args.get(i + 1)).cloned()
}

/// What the walk found, and — just as loudly — what it could not open.
fn report(s: &mapgeom::geom::Stats, scene: &mapgeom::scene::Scene) {
    println!(
        "{} files, {} collision surfaces, {} visual meshes, {} triangles in, {} out, {} vertices",
        s.files,
        s.surfaces,
        s.visual_meshes,
        s.triangles,
        scene.tri_count(),
        scene.vert_count()
    );
    if let Some((lo, hi)) = scene.bounds() {
        println!(
            "  bounds  x {:.2}..{:.2}  y {:.2}..{:.2}  z {:.2}..{:.2}  (metres, TM world axes)",
            lo[0], hi[0], lo[1], hi[1], lo[2], hi[2]
        );
    }
    let mut mats: Vec<(&String, usize)> =
        scene.groups.iter().map(|(k, g)| (k, g.tris.len())).collect();
    mats.sort_by_key(|(_, n)| std::cmp::Reverse(*n));
    for (m, n) in mats.iter().take(12) {
        println!("  {:>9} {}", n, m);
    }
    if !s.unhandled.is_empty() {
        let mut u: Vec<(&u32, &usize)> = s.unhandled.iter().collect();
        u.sort();
        let list: Vec<String> =
            u.iter().map(|(c, n)| format!("0x{:08X} x{}", c, n)).collect();
        println!("  classes with no geometry reader: {}", list.join(", "));
    }
    if !s.missing.is_empty() {
        println!("  {} files could NOT be opened:", s.missing.len());
        let mut seen = std::collections::BTreeSet::new();
        for (f, e) in &s.missing {
            if seen.insert(f.clone()) {
                println!("    {}\n      {}", f, e);
            }
        }
    }
}

fn write_scene(scene: &mapgeom::scene::Scene, out: &str) {
    if out.to_lowercase().ends_with(".obj") {
        let mtl = format!("{}.mtl", out.trim_end_matches(".obj"));
        let (o, m) = scene.obj(
            std::path::Path::new(&mtl).file_name().unwrap().to_string_lossy().as_ref(),
        );
        std::fs::write(out, o).unwrap_or_else(|e| die(e.to_string()));
        std::fs::write(&mtl, m).unwrap_or_else(|e| die(e.to_string()));
        println!("wrote {} and {}", out, mtl);
    } else {
        std::fs::write(out, scene.glb()).unwrap_or_else(|e| die(e.to_string()));
        println!("wrote {}", out);
    }
}

fn describe(n: &Node) -> String {
    match n {
        Node::Prefab(p) => format!("CPlugPrefab, {} entities", p.ents.len()),
        Node::StaticObject(s) => format!(
            "CPlugStaticObjectModel mesh={} collidable={} shape={}",
            s.mesh, s.mesh_collidable, s.shape
        ),
        Node::Surface(s) => format!(
            "CPlugSurface, {} meshes, {} triangles, {} primitives",
            s.meshes.len(),
            s.meshes.iter().map(|m| m.tris.len()).sum::<usize>(),
            s.primitives.len()
        ),
        Node::Solid2(s) => format!(
            "CPlugSolid2Model, {} geoms, {} visuals, materials [{}]",
            s.geoms.len(),
            s.visuals.len(),
            s.material_names.join(" ")
        ),
        Node::Visual(v) => format!(
            "CPlugVisual, {} verts, {} indices, {} streams",
            v.count,
            v.indices.len(),
            v.vertex_streams.len()
        ),
        Node::VertexStream(v) => format!(
            "CPlugVertexStream, {} positions, {} normals",
            v.positions.len(),
            v.normals.len()
        ),
        Node::Material(n, p) => format!("material {} ({})", n, mapgeom::scene::physics_name(*p)),
        Node::Crystal(c) => format!(
            "CPlugCrystal, {} meshes, {} faces, materials [{}]",
            c.meshes.len(),
            c.meshes.iter().map(|m| m.faces.len()).sum::<usize>(),
            c.materials.iter().map(|(n, i)| if n.is_empty() { format!("node{}", i) } else { n.clone() }).collect::<Vec<_>>().join(" ")
        ),
        Node::ItemModel(i) => format!("item model -> node {}", i),
        Node::Other(c) => format!("class 0x{:08X}", c),
    }
}
