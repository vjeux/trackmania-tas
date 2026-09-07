use mapgeom::coverage::RESTING;
use mapgeom::node::{Node, Slot};
use mapgeom::{names, store::DataStore, store::STADIUM_KEY};

const USAGE: &str = "\
mapgeom -- TM2020 map geometry

  --packs <dir>     directory holding the game's .pak files
                    (default $TM_SERVER/Packs, else /tmp/tmoracle/server/Packs)
  --key <hex>       Stadium pack key (default: the known one)

COMMANDS
  ls [<substring>]              pack entries whose path contains <substring>
  resolve <logical-path>        which pack entry a logical path is stored under
  refs <logical-path>           a file's external reference table
  dump <path> [--body F]        walk a file's node graph and summarise it;
                                --body writes the decompressed body out
  model <path> --out F          a single file's geometry, as .glb or .obj
  static-item <prefab-or-item> --out F --ident NAME.Item.Gbx --author X
      [--scale 0.5] [--collection 26]
                                a static-object item from every static object
                                of a pack prefab, or from a static/crystal item
  items <file.Map.Gbx> [--out D]   the models a map embeds inside itself
  crash <DUMP.dmp> [--exe Trackmania.exe] [--read ADDR LEN] [--find PAT]
      [--disasm ADDR [N]] [--no-stack] [--quiet]
                                a client crash minidump: exception, registers,
                                faulting instruction, heuristic stack walk as
                                exe RVAs / objdump addresses (CRASH.md)
  tiny-library <file.Map.Gbx> --library-out ZIP --mapping-out TSV [--report TSV]
      [--scale 0.5] [--legacy-zip Nadeo.zip] [--items-dir DIR] [--veget substitute|keep|drop]
      [--collection BlueBay] [--only N,..]
                                every block/item model of the map as a half-scale
                                STATIC item (stage-1 path) + tmmaps tiny mapping
  tiny-assets <file.Map.Gbx> --out F --library-out ZIP --catalog TSV
      --footprints TSV --nadeo-zip ZIP --empty-template ITEM --blue-pak PAK
      --stadium-pak PAK [--scale 0.5] [--keep-unscaled]
                                build exact scalable wrappers and the tiny map
  extract <logical-path> <file>    one pack file, decrypted and decompressed
  blockinfo <logical-path>...    a CGameCtnBlockInfo file, fully typed: kind,
                                variants, units, clips per side, mobil prefabs
  blockinfo-map <file.Map.Gbx> --out TSV [--no-baked] [--report TSV]
      [--collection BlueBay]
      [--collection BlueBay]
                                every authored block with its picked variant,
                                cells, prefabs and what each side faces
  blockinfo-all [<substring>] [--out TSV]
                                parse every block info in the packs and report
  map <file.Map.Gbx> --out F [--yoff N] [--no-items] [--no-deco]
      [--ghost G]... [--png P] [--clip-y Y]
                                a whole map, with any ghosts as polylines
  check <file.Map.Gbx> --ghost G... [--yoff N] [--reach M]
                                fit the map height and grade the model: how
                                far above the surface the car sat, and what
                                the surface was
  compare --before DIR --after DIR [--out F]
                                the before/after coverage table, as markdown,
                                from two directories of transcripts
  corpus --root DIR --out DIR [--jobs N] [--maps a,b] [--pin id=ghost]
         [-- <check flags>]
                                grade every map in a tree, in parallel, into
                                one directory of transcripts + summary.tsv
  where <file.Map.Gbx> --at X,Z [--yoff N]
                                every block and item record the map places
                                near a point, with where it lands and how many
                                triangles it produced
  holes <file.Map.Gbx> --ghost G... --yoff N [--radius M]
                                every stretch of the run the model has no
                                surface under, and how far the nearest
                                triangle is -- absent, or merely too narrow
  plumb <file.Map.Gbx> --at X,Z... [--yoff N]
                                every surface in one vertical column

`dump` and `model` take either a pack path or a local file, so a model pulled
out of a map with `items --out` can be inspected directly.
MAPGEOM_TRACE=1 prints every step of a body walk.
";

struct Args {
    packs: String,
    /// Explicit pack files, each `PATH` or `PATH:KEYHEX` (`--pak`, repeatable).
    /// Set, they win over the directory scan; the client packs (`BlueBay.pak`,
    /// `Stadium.pak`) each need their own key, and a BlueBay prefab can name a
    /// Stadium file, so both are usually given together.
    paks: Vec<String>,
    key: String,
    rest: Vec<String>,
}

fn parse_args() -> Args {
    let mut packs = std::env::var("TM_SERVER")
        .map(|s| format!("{}/Packs", s))
        .unwrap_or_else(|_| "/tmp/tmoracle/server/Packs".to_string());
    let mut paks = Vec::new();
    let mut key = STADIUM_KEY.to_string();
    let mut rest = Vec::new();
    let mut it = std::env::args().skip(1);
    while let Some(a) = it.next() {
        match a.as_str() {
            "--packs" => packs = it.next().unwrap_or_default(),
            "--pak" => paks.push(it.next().unwrap_or_default()),
            "--key" => key = it.next().unwrap_or_default(),
            _ => rest.push(a),
        }
    }
    Args { packs, paks, key, rest }
}

fn open(a: &Args) -> DataStore {
    if !a.paks.is_empty() {
        let mut store = DataStore::empty();
        for spec in &a.paks {
            let (path, key) = match spec.rsplit_once(':') {
                Some((p, k)) if k.len() == 32 => (p.to_string(), k.to_string()),
                _ => (spec.clone(), a.key.clone()),
            };
            store.add_pak(&path, &key).unwrap_or_else(|e| die(e));
        }
        return store;
    }
    let mut paths: Vec<String> = Vec::new();
    for name in ["dedicated_TMStadium.pak", "dedicated.pak", "resource.pak"] {
        let p = format!("{}/{}", a.packs, name);
        if std::path::Path::new(&p).exists() {
            paths.push(p);
        }
    }
    if paths.is_empty() {
        eprintln!("no .pak files in {} (pass --pak FILE for a client pack)", a.packs);
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

/// Build the whole scene for one map at one height: its blocks and items, the
/// models it embeds, and the stadium it sits in. The third return is the
/// assembler's model table — which models were placed, and which of them
/// produced no triangles — which is what `blame` turns a hole into a name
/// with.
fn build(
    store: &mut DataStore,
    m: &tmmaps::map::MapFile,
    yoff: f32,
    with_items: bool,
    deco: bool,
    verbose: bool,
) -> (
    mapgeom::scene::Scene,
    mapgeom::geom::Stats,
    std::collections::BTreeMap<String, (usize, bool)>,
) {
    let mut asm = mapgeom::assemble::Assembler::new(store);
    match asm.with_embedded(m) {
        Ok(0) => {}
        Ok(n) if verbose => println!("  {} models embedded in the map itself", n),
        Ok(_) => {}
        Err(e) => eprintln!("  embedded models: {}", e),
    }
    let mut scene = asm.map(m, yoff, with_items);
    if deco {
        if let Some((path, d)) = asm.decoration(m, yoff) {
            if verbose {
                println!("  decoration {}: {} triangles", path, d.tri_count());
            }
            scene.append(&d, &mapgeom::geom::IDENTITY);
        }
    }
    if verbose {
        let mut miss: Vec<(&String, usize)> = asm
            .used
            .iter()
            .filter(|(_, (_, ok))| !*ok)
            .map(|(k, (n, _))| (k, *n))
            .collect();
        miss.sort_by_key(|(_, n)| std::cmp::Reverse(*n));
        if !miss.is_empty() {
            let total: usize = miss.iter().map(|(_, n)| *n).sum();
            println!(
                "  {} placements of {} models had no geometry:",
                total,
                miss.len()
            );
            for (name, n) in miss.iter().take(15) {
                println!("    {:>5} x {}", n, name);
            }
        }
    }
    let stats = std::mem::take(&mut asm.stats);
    let used = std::mem::take(&mut asm.used);
    (scene, stats, used)
}

/// How well one candidate map height explains a set of runs: how many samples
/// are RESTING on a surface, and the median of those gaps.
fn score_at(scene: &mapgeom::scene::Scene, runs: &[Run], reach: f32) -> (usize, f32) {
    let idx = mapgeom::probe::Index::build(scene, 32.0);
    let mut score = 0usize;
    let mut centre = f32::NAN;
    for r in runs {
        let (n, c) = mapgeom::coverage::resting(&idx, &r.motions, reach, RESTING);
        score += n;
        if centre.is_nan() {
            centre = c;
        }
    }
    (score, centre)
}

fn main() {
    // --version / -V. Compile-time only: CARGO_PKG_* come from the crate's
    // Cargo.toml (which inherits the one workspace version), and TAS_BUILD is
    // the git hash the release build sets. option_env! means an ordinary
    // `cargo build` still works and simply reports "dev". No dependency.
    if std::env::args().any(|x| x == "--version" || x == "-V") {
        println!(
            "{} {} ({})",
            option_env!("CARGO_BIN_NAME").unwrap_or(env!("CARGO_PKG_NAME")),
            env!("CARGO_PKG_VERSION"),
            option_env!("TAS_BUILD").unwrap_or("dev")
        );
        std::process::exit(0);
    }
    // --help / -h prints usage on STDOUT and exits 0. A tool that prints its
    // usage to stderr and exits non-zero is indistinguishable from a tool that
    // rejected your flags, which is what most of these did before the release.
    if std::env::args().any(|x| x == "--help" || x == "-h") {
        print!("{}", USAGE);
        std::process::exit(0);
    }
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
                    println!(
                        "{}\tclass 0x{:08X}\t{} bytes\tflags 0x{:x}{}{}",
                        p,
                        e.class_id,
                        e.uncompressed_size,
                        e.flags,
                        if e.is_compressed() { " lz4" } else { " raw" },
                        if e.dont_use_dummy_write() { "" } else { " dummywrite" }
                    );
                    n += 1;
                }
            }
            eprintln!("{} entries", n);
        }
        // skins [SUBSTR] [--raw]: every pack entry (items, block infos) whose
        // path contains SUBSTR and whose HEADER carries a CPlugGameSkin chunk
        // (0x090F4000) — the declaration that lets the game feed a model's
        // texture slot with a skin: the in-game advertisement on screens and
        // gates (`Any\Advertisement6x1\`), a placement's own skin file
        // (`Stadium\LightColors\`). One row per entry: path, skin folder,
        // texture slots (class:name=default file), the trailing words; `--raw`
        // adds the chunk bytes as hex. A row that does not decode to the exact
        // byte count says so — the layout is measured, not assumed.
        "skins" => {
            let mut store = open(&a);
            let pat = a.rest.get(1).filter(|s| !s.starts_with("--")).cloned().unwrap_or_default().to_uppercase();
            let raw = a.rest.iter().any(|x| x == "--raw");
            let paths: Vec<String> = store
                .entries()
                .filter(|e| {
                    let p = e.path();
                    (pat.is_empty() || p.to_uppercase().contains(&pat))
                        && (e.class_id == 0x2E00_2000 || (e.class_id >> 12) == 0x03051 || (e.class_id >> 12) == 0x03053 || (e.class_id >> 12) == 0x03055 || (e.class_id >> 12) == 0x0305B)
                })
                .map(|e| e.path())
                .collect();
            let mut n = 0;
            println!("path\tdir\tfids\ttail\tbytes");
            for p in &paths {
                let Ok(bytes) = store.read(p) else { continue };
                let Some(chunk) = tmmaps::header::game_skin_chunk(&bytes) else { continue };
                n += 1;
                match tmmaps::header::GameSkin::decode(&chunk) {
                    Some(s) => {
                        let fids: Vec<String> = s.fids.iter().map(|f| format!("{:08X}:{}={}({})", f.class, f.name, f.file, f.flag)).collect();
                        let tail: Vec<String> = s.tail.chunks(4).map(|c| c.iter().map(|b| format!("{b:02x}")).collect::<String>()).collect();
                        let exact = if s.encode() == chunk { "" } else { " REENCODE-MISMATCH" };
                        println!("{}\t{}\t{}\t{}\t{}{}", p, s.dir, fids.join(" "), tail.join(" "), chunk.len(), exact);
                    }
                    None => println!("{}\tUNDECODED\t\t\t{}", p, chunk.len()),
                }
                if raw {
                    println!("  {}", chunk.iter().map(|b| format!("{b:02x}")).collect::<String>());
                }
            }
            eprintln!("{} of {} candidate entries carry a 0x090F4000 header chunk", n, paths.len());
        }
        // item-rename IN.Item.Gbx --out OUT --ident NAME.Item.Gbx [--author A]: the
        // game's own item file under a new Ident (every length-prefixed ident /
        // name / author string replaced in header and body) so it can be
        // EMBEDDED in a map as is — every reference it carries still points
        // into the packs, which is the question such a copy asks the game.
        "item-rename" => {
            let inp = a.rest.get(1).cloned().unwrap_or_else(|| die("item-rename IN.Item.Gbx --out OUT --ident NAME.Item.Gbx [--author A]".to_string()));
            let bytes = std::fs::read(&inp).unwrap_or_else(|e| die(format!("{inp}: {e}")));
            let out = flag(&a.rest, "--out").unwrap_or_else(|| die("--out FILE".to_string()));
            let ident = flag(&a.rest, "--ident").unwrap_or_else(|| die("--ident NAME.Item.Gbx".to_string()));
            let author = flag(&a.rest, "--author").unwrap_or_else(|| ident.clone());
            let (old_name, old_author) = tmmaps::header::item_ident_author(&bytes).unwrap_or_else(|| die(format!("{inp}: no header ident")));
            let renamed = mapgeom::tiny_assets::rename_item_ident(&bytes, &old_name, &old_author, &ident, &author);
            std::fs::write(&out, &renamed).unwrap_or_else(|e| die(e.to_string()));
            let back = tmmaps::header::item_ident_author(&renamed).map(|(n, a)| format!("{n} by {a}")).unwrap_or_else(|| "NOT READ BACK".into());
            println!("wrote {out}: ident {old_name:?} by {old_author:?} -> {back} ({} bytes)", renamed.len());
        }
        // skin FILE [--from SRC --out OUT]: print a GBX file's skin declaration
        // (header chunk 0x090F4000), or graft SRC's declaration onto FILE and
        // write OUT — the one-item experiment before the bake carried it.
        "skin" => {
            let p = a.rest.get(1).cloned().unwrap_or_else(|| die("skin FILE [--from SRC --out OUT]".to_string()));
            let bytes = std::fs::read(&p).unwrap_or_else(|e| die(format!("{p}: {e}")));
            match tmmaps::header::game_skin_chunk(&bytes) {
                Some(c) => match tmmaps::header::GameSkin::decode(&c) {
                    Some(s) => println!("{p}: {}", s.summary()),
                    None => println!("{p}: 0x090F4000 chunk of {} bytes, undecoded: {}", c.len(), c.iter().map(|b| format!("{b:02x}")).collect::<String>()),
                },
                None => println!("{p}: no skin declaration"),
            }
            if let Some(from) = flag(&a.rest, "--from") {
                let out = flag(&a.rest, "--out").unwrap_or_else(|| die("--out OUT with --from".to_string()));
                let src = std::fs::read(&from).unwrap_or_else(|e| die(format!("{from}: {e}")));
                let chunk = tmmaps::header::game_skin_chunk(&src).unwrap_or_else(|| die(format!("{from}: no skin declaration to copy")));
                let grafted = tmmaps::header::set_game_skin_chunk(&bytes, &chunk);
                std::fs::write(&out, &grafted).unwrap_or_else(|e| die(e.to_string()));
                let check = tmmaps::header::game_skin(&grafted).map(|s| s.summary()).unwrap_or_else(|| "NOT READ BACK".into());
                println!("wrote {out} ({} bytes): {check}", grafted.len());
            }
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
            println!(
                "{}  class 0x{:08X}  {} nodes",
                m.path, m.class_id, m.num_nodes
            );
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
            // The decompressed body, for when a chunk layout has to be read
            // off the bytes. `MAPGEOM_TRACE=1` says where in it the walk was.
            if let Some(out) = flag(&a.rest, "--body") {
                std::fs::write(&out, &m.body).unwrap_or_else(|e| die(e.to_string()));
                println!("wrote {} ({} bytes of body)", out, m.body.len());
            }
            let g = m.graph().unwrap_or_else(die);
            println!(
                "{}  class 0x{:08X}  {} nodes",
                m.path, m.class_id, m.num_nodes
            );
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
            for r in &g.recovered {
                println!("  RECOVERED past an unknown layout: {}", r);
            }
        }
        // Bake a scale into copies of a prefab tree and prove it: the copies
        // re-walk identically with every marked float scaled, and their
        // collision bounds are the original's times the factor.
        "rescale" => {
            let mut store = open(&a);
            let p = a.rest.get(1).cloned().unwrap_or_default();
            let factor: f32 = flag(&a.rest, "--factor").unwrap_or_else(|| "0.5".into()).parse().unwrap_or_else(|_| die("--factor number".into()));
            let suffix = flag(&a.rest, "--suffix").unwrap_or_else(|| "_half".into());
            let out_dir = flag(&a.rest, "--out-dir");
            let mut rs = mapgeom::rescale::Rescale::new(factor, &suffix);
            let top = rs.file(&mut store, &p).unwrap_or_else(die);
            for rep in &rs.reports {
                println!("{} -> {}: {} marks, {} floats, {} nested", rep.logical, rep.out, rep.marks, rep.floats, rep.nested);
            }
            // Re-walk every copy against its source.
            for rep in &rs.reports {
                let orig = store.read(&rep.logical).unwrap_or_else(die);
                let n = mapgeom::rescale::verify(&orig, &rs.files[&rep.out], &rep.logical, factor).unwrap_or_else(die);
                println!("  verified {}: {} floats re-read at x{}", rep.out, n, factor);
            }
            // Collision bounds before and after, for the top file.
            let bounds = |bytes: &[u8], logical: &str, store: &mut DataStore| -> Option<[f32; 6]> {
                let m = mapgeom::store::Model::parse(bytes, logical).ok()?;
                let mut c = mapgeom::geom::Collector::new(store);
                c.model(&m, &mapgeom::geom::IDENTITY, 0);
                if c.scene.tri_count() == 0 {
                    return None;
                }
                let (lo, hi) = c.scene.bounds()?;
                Some([lo[0], lo[1], lo[2], hi[0], hi[1], hi[2]])
            };
            let orig = store.read(&p).unwrap_or_else(die);
            let b0 = bounds(&orig, &p, &mut store);
            for (name, bytes) in &rs.files {
                store.add_overlay(name, bytes.clone());
            }
            let b1 = bounds(&rs.files[&top], &p, &mut store);
            println!("  bounds before {:?}", b0);
            println!("  bounds after  {:?}", b1);
            if let (Some(x), Some(y)) = (b0, b1) {
                for i in 0..6 {
                    if (y[i] - x[i] * factor).abs() > 1e-3 * x[i].abs().max(1.0) {
                        die::<()>(format!("bounds component {} is {} not {}", i, y[i], x[i] * factor));
                    }
                }
                println!("  bounds scaled exactly by {}", factor);
            }
            if let Some(dir) = out_dir {
                for (name, bytes) in &rs.files {
                    let path = std::path::Path::new(&dir).join(name.replace('\\', "/"));
                    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
                    std::fs::write(&path, bytes).unwrap();
                    println!("  wrote {}", path.display());
                }
            }
        }
        // A crystal item from a pack prefab's visual geometry, written around a
        // known-good crystal item as template.
        "crystal-item" => {
            let mut store = open(&a);
            let p = a.rest.get(1).cloned().unwrap_or_default();
            let template = std::fs::read(flag(&a.rest, "--template").unwrap_or_else(|| die("--template ITEM".into()))).unwrap();
            let out = flag(&a.rest, "--out").unwrap_or_else(|| die("--out FILE".into()));
            let ident = flag(&a.rest, "--ident").unwrap_or_else(|| die("--ident NAME.Item.Gbx".into()));
            if !a.rest.iter().any(|x| x == "--visual") {
                // Default: the generator's own path (collision geometry).
                let coll: u32 = flag(&a.rest, "--collection").map(|c| c.parse().unwrap()).unwrap_or(26);
                let (item, faces) = mapgeom::tiny_assets::crystal_from_model_in(&mut store, &p, &template, &ident, coll).unwrap_or_else(die);
                let item = if coll == 26 { item } else { mapgeom::tiny_assets::set_ident_collection(&item, coll) };
                std::fs::write(&out, &item).unwrap();
                println!("wrote {out} ({} bytes, {faces} faces from collision surfaces)", item.len());
                return;
            }
            let author = flag(&a.rest, "--author").unwrap_or_else(|| mapgeom::tiny_assets::AUTHOR.to_string());
            let scale: f32 = flag(&a.rest, "--scale").unwrap_or_else(|| "1".into()).parse().unwrap_or_else(|_| die("--scale number".into()));
            let m = store.load_model(&p).unwrap_or_else(die);
            let mut c = mapgeom::geom::Collector::new(&mut store);
            c.link_labels = true;
            c.finest_lod_only = !a.rest.iter().any(|x| x == "--all-lods");
            c.model(&m, &mapgeom::geom::IDENTITY, 0);
            let surface_links = c.surface_links.clone();
            let scene = c.scene;
            let mut mesh = mapgeom::crystal::CrystalMesh::default();
            let mut materials = Vec::new();
            for (label, g) in &scene.groups {
                // Visual groups only (`LINK|PHYS`); collision groups carry a bare physics name.
                if g.tris.is_empty() || !label.contains('|') {
                    continue;
                }
                // Terrain visuals shade through a shared id material; the look
                // material is the one the collision surface names.
                let label: &str = if label.starts_with("Techno3\\") && !surface_links.is_empty() { &surface_links[0] } else { label };
                let coll: u32 = flag(&a.rest, "--collection").map(|c| c.parse().unwrap()).unwrap_or(26);
                let Some(mut spec) = mapgeom::tiny_assets::visual_material_for(label, coll) else {
                    println!("  dropped {} ({} tris: decal/light)", label, g.tris.len());
                    continue;
                };
                if let Some(link) = flag(&a.rest, "--material") {
                    spec.link = link; // one known material for every face: isolates geometry from material lookups
                }
                let verts: Vec<[f32; 3]> = g.verts.iter().map(|v| [v[0] * scale, v[1] * scale, v[2] * scale]).collect();
                let reversed: Vec<[u32; 3]> = g.tris.iter().map(|t| [t[0], t[2], t[1]]).collect();
                let tris: &[[u32; 3]] = if a.rest.iter().any(|x| x == "--keep-winding") { &g.tris } else { &reversed };
                mesh.add_tris(&verts, tris, materials.len() as u32, 8.0 * scale);
                if a.rest.iter().any(|x| x == "--flip") {
                    let flipped: Vec<[u32; 3]> = g.tris.iter().map(|t| [t[0], t[2], t[1]]).collect();
                    mesh.add_tris(&verts, &flipped, materials.len() as u32, 8.0 * scale);
                }
                println!("  material {} <- {} ({} tris, physics {})", materials.len(), label, g.tris.len(), spec.physics);
                materials.push(spec);
            }
            println!("  {} positions, {} faces, {} materials", mesh.positions.len(), mesh.faces.len(), materials.len());
            let coll: u32 = flag(&a.rest, "--collection").map(|c| c.parse().unwrap()).unwrap_or(26);
            let item = mapgeom::crystal::build_item(&template, &ident, &author, &materials, &mesh);
            let item = if coll == 26 { item } else { mapgeom::tiny_assets::set_ident_collection(&item, coll) };
            std::fs::write(&out, &item).unwrap();
            println!("wrote {out} ({} bytes)", item.len());
        }
        // A synthetic 32x32x2 m box in ONE material: the material probe.
        "catalog-lib" => {
            // catalog-lib LIB1.zip LIB0.5.zip --out OUT.zip : LIB1 as is, plus
            // every item of the scaled library renamed AC->AS (same length).
            let l1 = std::fs::read(a.rest.get(1).unwrap_or_else(|| die("catalog-lib LIB1 LIB2 --out OUT".into()))).expect("lib1");
            let l2 = std::fs::read(a.rest.get(2).unwrap_or_else(|| die("catalog-lib LIB1 LIB2 --out OUT".into()))).expect("lib2");
            let out = flag(&a.rest, "--out").unwrap_or_else(|| die("--out FILE".into()));
            let mut files = mapgeom::embedded::unzip(&l1).expect("lib1 zip");
            for (name, bytes) in mapgeom::embedded::unzip(&l2).expect("lib2 zip") {
                let short = name.trim_start_matches("Items/");
                if !short.starts_with("AC") { continue; }
                let to = format!("AS{}", &short[2..]);
                let renamed = mapgeom::crystal::rename_ident_same_len(&bytes, short, &to);
                files.insert(format!("Items/{to}"), renamed);
            }
            let n = files.len();
            std::fs::write(&out, mapgeom::tiny_assets::zip(&files)).expect("write");
            println!("wrote {out} ({n} items)");
        }
        "scale-item" => {
            // scale-item IN.Item.Gbx --out OUT --scale S : geometry-scaled copy
            let inp = std::fs::read(a.rest.get(1).unwrap_or_else(|| die("scale-item IN.Item.Gbx".into()))).expect("read item");
            let out = flag(&a.rest, "--out").unwrap_or_else(|| die("--out FILE".into()));
            let s: f32 = flag(&a.rest, "--scale").unwrap_or_else(|| "0.5".into()).parse().expect("--scale");
            let mut bytes = mapgeom::crystal::scale_item(&inp, s);
            if let (Some(from), Some(to)) = (flag(&a.rest, "--from-ident"), flag(&a.rest, "--ident")) {
                bytes = mapgeom::crystal::rename_ident_same_len(&bytes, &from, &to);
            }
            std::fs::write(&out, &bytes).expect("write");
            println!("wrote {out} ({} bytes, geometry x{s})", bytes.len());
        }
        "crystal-box" => {
            let template = std::fs::read(flag(&a.rest, "--template").unwrap_or_else(|| die("--template ITEM".into()))).unwrap();
            let out = flag(&a.rest, "--out").unwrap_or_else(|| die("--out FILE".into()));
            let ident = flag(&a.rest, "--ident").unwrap_or_else(|| die("--ident NAME.Item.Gbx".into()));
            let link = flag(&a.rest, "--material").unwrap_or_else(|| "Stadium\\Media\\Material\\RoadTech".into());
            let phys: u8 = flag(&a.rest, "--physics").unwrap_or_else(|| "16".into()).parse().unwrap();
            let reverse = a.rest.iter().any(|x| x == "--reverse");
            let (sx, sy, sz) = (32.0f32, 2.0f32, 32.0f32);
            let v = [[0.0, 0.0, 0.0], [sx, 0.0, 0.0], [sx, 0.0, sz], [0.0, 0.0, sz], [0.0, sy, 0.0], [sx, sy, 0.0], [sx, sy, sz], [0.0, sy, sz]];
            // counter-clockwise seen from outside (right-handed, +y up)
            let mut tris: Vec<[u32; 3]> = vec![
                [4, 6, 5], [4, 7, 6], // top
                [0, 1, 2], [0, 2, 3], // bottom
                [0, 4, 5], [0, 5, 1], // -z side
                [3, 2, 6], [3, 6, 7], // +z side
                [0, 3, 7], [0, 7, 4], // -x side
                [1, 5, 6], [1, 6, 2], // +x side
            ];
            if reverse { for t in &mut tris { t.swap(1, 2); } }
            let mut mesh = mapgeom::crystal::CrystalMesh::default();
            mesh.add_tris(&v, &tris, 0, 32.0);
            let mut materials = vec![mapgeom::crystal::MaterialSpec { link, physics: phys }];
            // --unused N: N extra materials no face refers to (remap-loop probe).
            if let Some(n) = flag(&a.rest, "--unused") {
                for _ in 0..n.parse::<usize>().unwrap() {
                    materials.push(mapgeom::crystal::MaterialSpec { link: "Editors\\MeshEditorMedia\\Materials\\Concrete".into(), physics: 0 });
                }
            }
            let item = mapgeom::crystal::build_item(&template, &ident, &ident, &materials, &mesh);
            std::fs::write(&out, &item).unwrap();
            println!("wrote {out} ({} bytes) reverse={reverse}", item.len());
        }
        // Round-trip oracle: the template's own crystal, re-emitted by our writer.
        "static-item" => {
            let mut open_store = || open(&a);
            mapgeom::static_item::cli::run(&a.rest, &mut open_store).unwrap_or_else(die);
        }
        "item-check" => {
            let mut open_store = || open(&a);
            mapgeom::static_item::check::run(&a.rest, &mut open_store).unwrap_or_else(die);
        }
        // item-rename IN.Item.Gbx --out OUT --ident NAME.Item.Gbx [--author A]: the
        // game's own item file under a new Ident (header chunk 0x2E001003 rebuilt,
        // the body's ident/name strings replaced) so it can be EMBEDDED in a map
        // as is — every reference it carries still points into the packs, which
        // is the question such a copy asks the game.
        "item-rename" => {
            let inp = a.rest.get(1).unwrap_or_else(|| die("item-rename IN.Item.Gbx --out OUT --ident NAME.Item.Gbx [--author A]".into()));
            let bytes = std::fs::read(inp).unwrap_or_else(|e| die(format!("{inp}: {e}")));
            let out = flag(&a.rest, "--out").unwrap_or_else(|| die("--out FILE".into()));
            let ident = flag(&a.rest, "--ident").unwrap_or_else(|| die("--ident NAME.Item.Gbx".into()));
            let author = flag(&a.rest, "--author").unwrap_or_else(|| ident.clone());
            let (old_name, old_author) = tmmaps::header::item_ident_author(&bytes).unwrap_or_else(|| die(format!("{inp}: no header ident")));
            let renamed = mapgeom::tiny_assets::rename_item_ident(&bytes, &old_name, &old_author, &ident, &author);
            std::fs::write(&out, &renamed).expect("write");
            println!("wrote {out}: ident {old_name:?} by {old_author:?} -> {ident:?} by {author:?} ({} bytes)", renamed.len());
        }
        // A client crash dump: where it died, in objdump addresses. CRASH.md.
        "crash" => {
            mapgeom::minidump::run(&a.rest).unwrap_or_else(die);
        }
        // A full memory dump of the running client: the pack keys it derived.
        "pak-keyhunt" => {
            mapgeom::keyhunt::run(&a.rest).unwrap_or_else(die);
        }
        "pak-basekey" => {
            mapgeom::keyhunt::basekey_scan(&a.rest).unwrap_or_else(die);
        }
        "pak-trykeys" => {
            mapgeom::keyhunt::try_keys(&a.rest).unwrap_or_else(die);
        }
        "crystal-roundtrip" => {
            let template = std::fs::read(a.rest.get(1).cloned().unwrap_or_default()).unwrap();
            let out = flag(&a.rest, "--out").unwrap_or_else(|| die("--out FILE".into()));
            let ident = flag(&a.rest, "--ident").unwrap_or_else(|| die("--ident NAME.Item.Gbx".into()));
            let (materials, mesh) = mapgeom::crystal::decode_template(&template);
            println!("  {} materials, {} positions, {} faces", materials.len(), mesh.positions.len(), mesh.faces.len());
            let keep: u8 = flag(&a.rest, "--keep").unwrap_or_else(|| "0".into()).parse().unwrap();
            let item = mapgeom::crystal::build_item_with(&template, &ident, &ident, &materials, &mesh, keep);
            std::fs::write(&out, &item).unwrap();
            println!("wrote {out} ({} bytes) keep={keep}", item.len());
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
        "items" => {
            let p = a.rest.get(1).cloned().unwrap_or_default();
            let m = tmmaps::map::MapFile::load(std::path::Path::new(&p));
            let files = mapgeom::embedded::files(&m).unwrap_or_else(die);
            println!("{}: {} embedded files", p, files.len());
            for (name, bytes) in &files {
                println!("  {:>9} bytes  {}", bytes.len(), name);
            }
            if let Some(dir) = flag(&a.rest, "--out") {
                std::fs::create_dir_all(&dir).ok();
                for (name, bytes) in &files {
                    if name.ends_with(['/', '\\']) {
                        continue;
                    }
                    let rel = name
                        .strip_prefix("C:/Users/vjeux/Documents/Trackmania/")
                        .unwrap_or(name);
                    let path = std::path::Path::new(&dir).join(rel.replace('\\', "/"));
                    if let Some(parent) = path.parent() {
                        std::fs::create_dir_all(parent).unwrap_or_else(|e| die(e.to_string()));
                    }
                    std::fs::write(&path, bytes).unwrap_or_else(|e| die(e.to_string()));
                }
                println!("extracted {} files to {}", files.len(), dir);
            }
        }
        "tiny-library" => {
            let mut store = open(&a);
            let map = std::path::Path::new(a.rest.get(1).expect("tiny-library needs MAP"));
            let req = |name: &str| std::path::PathBuf::from(flag(&a.rest, name).unwrap_or_else(|| die(format!("tiny-library needs {name}"))));
            let report = flag(&a.rest, "--report").map(std::path::PathBuf::from);
            let items_dir = flag(&a.rest, "--items-dir").map(std::path::PathBuf::from);
            let only = flag(&a.rest, "--only");
            let legacy = flag(&a.rest, "--legacy-zip").map(std::path::PathBuf::from);
            let veget = flag(&a.rest, "--veget").unwrap_or_else(|| "substitute".into());
            let coll = flag(&a.rest, "--collection").unwrap_or_default();
            mapgeom::tiny_library::build(
                &mut store,
                map,
                &req("--library-out"),
                &req("--mapping-out"),
                report.as_deref(),
                flag(&a.rest, "--scale").unwrap_or_else(|| "0.5".into()).parse().unwrap_or_else(|_| die("--scale number".into())),
                legacy.as_deref(),
                items_dir.as_deref(),
                &veget,
                &coll,
                only.as_deref(),
            );
        }
        "tiny-assets" => {
            let map = std::path::Path::new(a.rest.get(1).expect("tiny-assets needs MAP"));
            let req = |name: &str| {
                std::path::PathBuf::from(
                    flag(&a.rest, name).unwrap_or_else(|| die(format!("tiny-assets needs {name}"))),
                )
            };
            mapgeom::tiny_assets::build(
                map,
                &req("--catalog"),
                &req("--footprints"),
                &req("--nadeo-zip"),
                &req("--empty-template"),
                &req("--blue-pak"),
                &req("--stadium-pak"),
                &req("--library-out"),
                &req("--out"),
                flag(&a.rest, "--scale")
                    .unwrap_or_else(|| "0.5".into())
                    .parse()
                    .unwrap_or_else(|_| die("--scale number".into())),
                a.rest.iter().any(|x| x == "--keep-unscaled"),
            );
        }
        "blockinfo" => {
            let mut store = open(&a);
            let paths: Vec<String> = a.rest.iter().skip(1).filter(|x| !x.starts_with("--")).cloned().collect();
            if paths.is_empty() {
                die::<()>("blockinfo needs a logical path".into());
            }
            let mut bad = 0;
            for p in &paths {
                match mapgeom::blockinfo::load(&mut store, p) {
                    Ok(b) => {
                        print!("{}", b.render());
                        if !b.parsed_to_end() {
                            bad += 1;
                        }
                    }
                    Err(e) => {
                        println!("{}\n  FAILED: {}", p, e);
                        bad += 1;
                    }
                }
            }
            if bad > 0 {
                std::process::exit(1);
            }
        }
        "blockinfo-all" => {
            let mut store = open(&a);
            let pat = a.rest.get(1).filter(|x| !x.starts_with("--")).cloned().unwrap_or_default().to_uppercase();
            let names: Vec<String> = store
                .entries()
                .map(|e| e.path())
                .filter(|p| p.to_uppercase().contains("\\GAMECTNBLOCKINFO\\") && (pat.is_empty() || p.to_uppercase().contains(&pat)))
                .collect();
            let mut rows = String::from("path\tclass\tstatus\tconsumed\tbody\tkind\tvariants\tdetail\n");
            let (mut ok, mut short, mut fail) = (0, 0, 0);
            for p in &names {
                match mapgeom::blockinfo::load(&mut store, p) {
                    Ok(b) => {
                        let st = if b.parsed_to_end() { ok += 1; "OK" } else { short += 1; "SHORT" };
                        rows.push_str(&format!(
                            "{}\t{:08X}\t{}\t{}\t{}\t{:?}\t{}\t{}\n",
                            p, b.class_id, st, b.consumed.0, b.consumed.1, b.kind, b.all_variants().len(),
                            if b.skipped_chunks.is_empty() { String::new() } else { format!("skipped {:?}", b.skipped_chunks) }
                        ));
                    }
                    Err(e) => {
                        fail += 1;
                        rows.push_str(&format!("{}\t\tFAIL\t\t\t\t\t{}\n", p, e.replace('\t', " ")));
                    }
                }
            }
            if let Some(out) = flag(&a.rest, "--out") {
                std::fs::write(&out, &rows).unwrap_or_else(|e| die(e.to_string()));
                println!("wrote {}", out);
            } else {
                print!("{}", rows);
            }
            println!("{} files: {} parsed to the end, {} short, {} failed", names.len(), ok, short, fail);
        }
        "blockinfo-map" => {
            let mut store = open(&a);
            let p = a.rest.get(1).cloned().unwrap_or_default();
            let out = flag(&a.rest, "--out").unwrap_or_else(|| die("blockinfo-map needs --out TSV".into()));
            let m = tmmaps::map::MapFile::load(std::path::Path::new(&p));
            let with_baked = !a.rest.iter().any(|x| x == "--no-baked");
            let collection = flag(&a.rest, "--collection").unwrap_or_else(|| "BlueBay".into());
            let (placements, idx) = mapgeom::blockmap::walk(&mut store, &m, with_baked, &collection);
            std::fs::write(&out, mapgeom::blockmap::tsv(&placements)).unwrap_or_else(|e| die(e.to_string()));
            let authored = placements.iter().filter(|q| !q.baked).count();
            let errs = placements.iter().filter(|q| !q.baked && q.error.is_some()).count();
            println!("wrote {}: {} authored blocks ({} with errors), {} baked", out, authored, errs, placements.len() - authored);
            print!("{}", mapgeom::blockmap::summary(&placements));
            let rep = mapgeom::blockmap::parse_report(&idx);
            if let Some(r) = flag(&a.rest, "--report") {
                std::fs::write(&r, &rep).unwrap_or_else(|e| die(e.to_string()));
                println!("wrote {}", r);
            }
            let n_ok = rep.lines().filter(|l| l.contains("\tOK\t")).count();
            let n_bad = rep.lines().count() - n_ok;
            println!("{} block info files loaded: {} parsed to the end, {} did not", rep.lines().count(), n_ok, n_bad);
            for l in rep.lines().filter(|l| !l.contains("\tOK\t")) {
                println!("  {}", l);
            }
        }
        "extract" => {
            let mut store = open(&a);
            let p = a.rest.get(1).cloned().unwrap_or_default();
            let out = a
                .rest
                .get(2)
                .cloned()
                .unwrap_or_else(|| "out.bin".to_string());
            let bytes = store.read(&p).unwrap_or_else(die);
            std::fs::write(&out, &bytes).unwrap_or_else(|e| die(e.to_string()));
            println!("{} -> {} ({} bytes)", p, out, bytes.len());
        }
        "map" => {
            let mut store = open(&a);
            let p = a.rest.get(1).cloned().unwrap_or_default();
            let out = flag(&a.rest, "--out").unwrap_or_else(|| "map.glb".to_string());
            let yoff: f32 = flag(&a.rest, "--yoff")
                .and_then(|s| s.parse().ok())
                .unwrap_or(0.0);
            let with_items = !a.rest.iter().any(|x| x == "--no-items");
            let deco = !a.rest.iter().any(|x| x == "--no-deco");
            let m = tmmaps::map::MapFile::load(std::path::Path::new(&p));
            println!(
                "{}: {} blocks, {} items, yoff {}",
                p,
                m.blocks.len(),
                m.items.len(),
                yoff
            );
            let (mut scene, stats, _) = build(&mut store, &m, yoff, with_items, deco, true);
            for g in ghost_runs(&a.rest) {
                scene.add_line(&g.name, g.points, g.colour);
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
            let deco = !a.rest.iter().any(|x| x == "--no-deco");
            let reach: f32 = flag(&a.rest, "--reach")
                .and_then(|s| s.parse().ok())
                .unwrap_or(6.0);
            let m = tmmaps::map::MapFile::load(std::path::Path::new(&p));
            let runs = ghost_runs(&a.rest);
            if runs.is_empty() {
                die::<()>("check needs at least one --ghost".into());
            }
            let given: Option<f32> = flag(&a.rest, "--yoff").and_then(|s| s.parse().ok());

            // Two passes: whole cell rows, then metre by metre around the
            // winner. The map height is NOT a whole number of cells, and
            // fitting only to cells leaves the car sitting an integer number
            // of metres above the model — see MAPGEOM.md §4.
            let yoff = match given {
                Some(y) => y,
                None => {
                    let mut best = (f32::NAN, 0usize);
                    for pass in 0..2 {
                        let cands: Vec<f32> = if pass == 0 {
                            mapgeom::place::Yoff::coarse().collect()
                        } else {
                            mapgeom::place::Yoff::refine(best.0).collect()
                        };
                        let mut pass_best = (f32::NAN, 0usize);
                        for y in cands {
                            let (scene, _, _) = build(&mut store, &m, y, with_items, deco, false);
                            let (score, centre) = score_at(&scene, &runs, reach);
                            if score > pass_best.1 {
                                pass_best = (y, score);
                            }
                            if score > 0 {
                                println!(
                                    "  yoff {:>7.1}  {} samples resting, median gap {:.3} m",
                                    y, score, centre
                                );
                            }
                        }
                        if pass_best.1 == 0 {
                            die::<()>(
                                "no map height puts this run on a surface -- the model is \
                                 missing whatever it drove on"
                                    .into(),
                            );
                        }
                        best = pass_best;
                    }
                    best.0
                }
            };

            let (scene, stats, used) = build(&mut store, &m, yoff, with_items, deco, true);
            let idx = mapgeom::probe::Index::build(&scene, 32.0);
            report(&stats, &scene);
            println!(
                "{}\n  yoff {}  ({} triangles indexed)",
                p,
                yoff,
                idx.triangle_count()
            );
            for run in &runs {
                let v = mapgeom::coverage::Verdict::of(&idx, &run.motions, reach);
                grade(&run.name, &v);
                if let Some(c) = containment(scene.bounds(), &run.points) {
                    if c.outside > 0 {
                        println!(
                            "    OUTSIDE THE MODEL   {} samples ({:.1} %) are past the model's \
                             own extent -- not a hole, there is nothing there to find",
                            c.outside,
                            100.0 * c.outside as f32 / run.points.len().max(1) as f32
                        );
                        println!(
                            "      model x {:.0}..{:.0}  y ..{:.0}  z {:.0}..{:.0}    \
                             run x {:.0}..{:.0}  y ..{:.0}  z {:.0}..{:.0}",
                            c.model.0[0],
                            c.model.1[0],
                            c.model.1[1],
                            c.model.0[2],
                            c.model.1[2],
                            c.run.0[0],
                            c.run.1[0],
                            c.run.1[1],
                            c.run.0[2],
                            c.run.1[2],
                        );
                    }
                }
                let b = mapgeom::blame::of(&m, &used, &v, &run.points, yoff);
                if b.total > 0 {
                    println!("    what the map has where the model does not:");
                    for (name, n) in b.ranked().iter().take(10) {
                        let label = if name.is_empty() {
                            "(no block or item in that cell)"
                        } else {
                            name
                        };
                        println!("      {:>6} samples  {}", n, label);
                    }
                }
                // One machine-readable line per run, for `corpus` to collect.
                let mats = v.materials();
                let top = mats.iter().max_by_key(|(_, n)| **n).map(|(k, _)| k.clone());
                println!(
                    "SUMMARY\t{}\t{}\t{}\t{:.4}\t{}\t{:.4}\t{:.4}\t{:.4}\t{:.4}\t{:.4}\t{}\t{}\t{}\t{}",
                    run.name,
                    yoff,
                    v.classes.len(),
                    v.raw_fraction(),
                    v.owed(),
                    v.covered_fraction(),
                    v.median_gap(),
                    v.gap_pct(0.90),
                    v.tightest_half(),
                    v.median_ride(),
                    v.count(mapgeom::coverage::Class::Airborne),
                    v.count(mapgeom::coverage::Class::Missing),
                    top.unwrap_or_else(|| "-".to_string()),
                    b.ranked().first().map(|(n, _)| if n.is_empty() { "(empty cell)" } else { n }).unwrap_or("-"),
                );
            }
        }
        "holes" => {
            let mut store = open(&a);
            let p = a.rest.get(1).cloned().unwrap_or_default();
            let yoff: f32 = flag(&a.rest, "--yoff")
                .and_then(|s| s.parse().ok())
                .unwrap_or(0.0);
            let reach: f32 = flag(&a.rest, "--reach")
                .and_then(|s| s.parse().ok())
                .unwrap_or(6.0);
            let radius: f32 = flag(&a.rest, "--radius")
                .and_then(|s| s.parse().ok())
                .unwrap_or(48.0);
            let m = tmmaps::map::MapFile::load(std::path::Path::new(&p));
            let runs = ghost_runs(&a.rest);
            let (scene, _, used) = build(
                &mut store,
                &m,
                yoff,
                !a.rest.iter().any(|x| x == "--no-items"),
                !a.rest.iter().any(|x| x == "--no-deco"),
                false,
            );
            let idx = mapgeom::probe::Index::build(&scene, 32.0);
            for run in &runs {
                let v = mapgeom::coverage::Verdict::of(&idx, &run.motions, reach);
                grade(&run.name, &v);
                let b = mapgeom::blame::of(&m, &used, &v, &run.points, yoff);
                println!("  {} holes, by what the map has there:", b.total);
                for (name, n) in b.ranked() {
                    let label = if name.is_empty() {
                        "(no block or item in that cell)"
                    } else {
                        name
                    };
                    println!("    {:>6} samples  {}", n, label);
                }
                // Consecutive missing samples are one hole; 525 lines is not
                // a diagnosis and a dozen spans is.
                println!("  each stretch, and how far the nearest triangle is:");
                let mut i = 0usize;
                let mut shown = 0;
                while i < v.classes.len() {
                    if v.classes[i] != mapgeom::coverage::Class::Missing {
                        i += 1;
                        continue;
                    }
                    let start = i;
                    while i < v.classes.len() && v.classes[i] == mapgeom::coverage::Class::Missing {
                        i += 1;
                    }
                    let mid = run.points[(start + i) / 2];
                    let near = idx.nearest(mid, radius);
                    let col = idx.column(mid[0], mid[2]);
                    shown += 1;
                    if shown > 24 {
                        continue;
                    }
                    println!(
                        "    samples {:>5}..{:<5} ({:>3}) at ({:.1}, {:.1}, {:.1})  nearest \
                         triangle {}  deepest column entry {}",
                        start,
                        i - 1,
                        i - start,
                        mid[0],
                        mid[1],
                        mid[2],
                        match &near {
                            Some((d, mat)) => format!("{:.2} m ({})", d, mat),
                            None => format!("none within {:.0} m", radius),
                        },
                        match col.iter().find(|(y, _)| *y <= mid[1]) {
                            Some((y, mat)) => format!("{:.2} m below ({})", mid[1] - y, mat),
                            None => "nothing below at any depth".to_string(),
                        }
                    );
                }
                if shown > 24 {
                    println!("    ... {} more stretches", shown - 24);
                }
            }
        }
        "corpus" => {
            let root = flag(&a.rest, "--root").unwrap_or_else(|| die("corpus needs --root".into()));
            let out = flag(&a.rest, "--out").unwrap_or_else(|| die("corpus needs --out".into()));
            let jobs_n: usize = flag(&a.rest, "--jobs")
                .and_then(|s| s.parse().ok())
                .unwrap_or(12);
            let only: Vec<String> = flag(&a.rest, "--maps")
                .map(|s| s.split(',').map(|x| x.trim().to_string()).collect())
                .unwrap_or_default();
            let mut js = mapgeom::corpus::jobs(std::path::Path::new(&root), &{
                let mut pins = std::collections::BTreeMap::new();
                for (i, x) in a.rest.iter().enumerate() {
                    if x == "--pin" {
                        if let Some((id, g)) = a.rest.get(i + 1).and_then(|s| s.split_once('=')) {
                            pins.insert(id.to_string(), g.to_string());
                        }
                    }
                }
                pins
            });
            if !only.is_empty() {
                js.retain(|j| only.contains(&j.id));
            }
            // Everything after `--` is handed to each `check`.
            let extra: Vec<String> = a
                .rest
                .iter()
                .position(|x| x == "--")
                .map(|i| a.rest[i + 1..].to_vec())
                .unwrap_or_default();
            eprintln!("{} maps, {} at a time", js.len(), jobs_n);
            let res = mapgeom::corpus::run(&js, std::path::Path::new(&out), jobs_n, &extra);
            let table = std::path::Path::new(&out).join("summary.tsv");
            let mut s = String::from(
                "map\tghost\tyoff\tsamples\traw\towed\tcovered\tmedian\tp90\thalfwin\tride\t\
                 airborne\tmissing\ttop_material\tworst_blame\n",
            );
            for (id, line) in &res {
                s.push_str(id);
                s.push('\t');
                s.push_str(line.trim_start_matches("SUMMARY\t"));
                s.push('\n');
            }
            std::fs::write(&table, &s).unwrap_or_else(|e| die(e.to_string()));
            println!("{}", s);
            println!("wrote {}", table.display());
        }
        "compare" => {
            let before =
                flag(&a.rest, "--before").unwrap_or_else(|| die("compare needs --before".into()));
            let after =
                flag(&a.rest, "--after").unwrap_or_else(|| die("compare needs --after".into()));
            let s = mapgeom::corpus::compare(
                std::path::Path::new(&before),
                std::path::Path::new(&after),
            );
            print!("{}", s);
            if let Some(out) = flag(&a.rest, "--out") {
                std::fs::write(&out, &s).unwrap_or_else(|e| die(e.to_string()));
                println!("wrote {}", out);
            }
        }
        "where" => {
            let mut store = open(&a);
            let p = a.rest.get(1).cloned().unwrap_or_default();
            let yoff: f32 = flag(&a.rest, "--yoff")
                .and_then(|s| s.parse().ok())
                .unwrap_or(0.0);
            let at: Vec<f32> = flag(&a.rest, "--at")
                .unwrap_or_default()
                .split(',')
                .filter_map(|s| s.trim().parse().ok())
                .collect();
            // No --at lists every record the map places, which is how a
            // decoration's handful of enormous blocks gets looked at.
            let all = at.len() < 2;
            let (x, z) = if all {
                (0.0, 0.0)
            } else {
                (at[0], at[at.len() - 1])
            };
            let m = tmmaps::map::MapFile::load(std::path::Path::new(&p));
            let mut asm = mapgeom::assemble::Assembler::new(&mut store);
            asm.with_embedded(&m).ok();
            let (cx, cz) = ((x / 32.0).floor() as i32, (z / 32.0).floor() as i32);
            if all {
                println!("every record the map places:");
            } else {
                println!(
                    "records within one cell of x {} z {} (cell {},{}):",
                    x, z, cx, cz
                );
            }
            for b in &m.blocks {
                let free = b.flags & tmmaps::map::FREE_BLOCK_FLAG != 0;
                let c = b.coords();
                let (bx, bz) = if free {
                    match b.free_pos {
                        Some(q) => ((q[0] / 32.0).floor() as i32, (q[2] / 32.0).floor() as i32),
                        None => continue,
                    }
                } else {
                    (c.0, c.2)
                };
                if !all && ((bx - cx).abs() > 1 || (bz - cz).abs() > 1) {
                    continue;
                }
                let lm = asm.block_model(&b.name);
                let size = lm.map(|l| l.size).unwrap_or((f32::NAN, f32::NAN));
                let tris = lm.map(|l| l.scene.tri_count()).unwrap_or(0);
                let origin_y = if free {
                    b.free_pos.map(|q| q[1]).unwrap_or(f32::NAN)
                } else {
                    8.0 * c.1 as f32 + yoff
                };
                println!(
                    "  block {:<52} {} cell {:?} dir {}  world y {:.2}  footprint {:.0}x{:.0}  \
                     {} triangles",
                    b.name,
                    if free { "FREE" } else { "grid" },
                    c,
                    b.dir,
                    origin_y,
                    size.0,
                    size.1,
                    tris
                );
            }
            for it in &m.items {
                let (ix, iz) = (
                    (it.pos[0] / 32.0).floor() as i32,
                    (it.pos[2] / 32.0).floor() as i32,
                );
                if !all && ((ix - cx).abs() > 1 || (iz - cz).abs() > 1) {
                    continue;
                }
                let tris = asm
                    .item_model(&it.model)
                    .map(|l| l.scene.tri_count())
                    .unwrap_or(0);
                println!(
                    "  item  {:<52} at ({:.2}, {:.2}, {:.2}) yaw {:.3} pivot {:?} scale {}  {} triangles",
                    it.model, it.pos[0], it.pos[1], it.pos[2], it.yaw, it.pivot, it.scale, tris
                );
            }
        }
        "plumb" => {
            let mut store = open(&a);
            let p = a.rest.get(1).cloned().unwrap_or_default();
            let yoff: f32 = flag(&a.rest, "--yoff")
                .and_then(|s| s.parse().ok())
                .unwrap_or(0.0);
            let m = tmmaps::map::MapFile::load(std::path::Path::new(&p));
            let ats: Vec<[f32; 3]> = a
                .rest
                .iter()
                .enumerate()
                .filter(|(_, s)| *s == "--at")
                .filter_map(|(i, _)| a.rest.get(i + 1))
                .filter_map(|s| {
                    let v: Vec<f32> = s.split(',').filter_map(|t| t.trim().parse().ok()).collect();
                    match v.len() {
                        2 => Some([v[0], 0.0, v[1]]),
                        3 => Some([v[0], v[1], v[2]]),
                        _ => None,
                    }
                })
                .collect();
            if ats.is_empty() {
                die::<()>("plumb needs --at X,Z (or X,Y,Z), repeatable".into());
            }
            let (scene, _, _) = build(
                &mut store,
                &m,
                yoff,
                !a.rest.iter().any(|x| x == "--no-items"),
                !a.rest.iter().any(|x| x == "--no-deco"),
                false,
            );
            let idx = mapgeom::probe::Index::build(&scene, 32.0);
            for at in &ats {
                let col = idx.column(at[0], at[2]);
                println!(
                    "column at x {} z {} (yoff {}): {} surfaces  (Water is at the plane a car\n  RESTS on, {} m below where it is drawn -- see probe::WATER_DRAFT)",
                    at[0],
                    at[2],
                    yoff,
                    col.len(),
                    mapgeom::probe::WATER_DRAFT
                );
                for (y, mat) in col.iter().take(40) {
                    println!("  y {:>10.3}   {}", y, mat);
                }
            }
        }
        _ => {
            print!("{}", USAGE);
            std::process::exit(2);
        }
    }
}

/// One driven run: its trajectory, and the motion the recording itself
/// reports at every sample. The second half is what lets a hole in the model
/// be told apart from a car in the air.
struct Run {
    name: String,
    points: Vec<[f32; 3]>,
    motions: Vec<mapgeom::coverage::Motion>,
    colour: [f32; 4],
}

/// `--ghost F` (repeatable): a driven trajectory in the same world frame as
/// the model, so the two can be looked at together.
fn ghost_runs(args: &[String]) -> Vec<Run> {
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
                let points: Vec<[f32; 3]> = d
                    .samples
                    .iter()
                    .map(|s| [s.x as f32, s.y as f32, s.z as f32])
                    .collect();
                let name = std::path::Path::new(p)
                    .file_stem()
                    .map(|s| s.to_string_lossy().to_string())
                    .unwrap_or_else(|| "ghost".to_string());
                println!(
                    "  ghost {}: {} samples, first {:?}, last {:?}",
                    name,
                    points.len(),
                    points.first().copied().unwrap_or_default(),
                    points.last().copied().unwrap_or_default()
                );
                out.push(Run {
                    name: format!("path_{}", name),
                    motions: mapgeom::coverage::motions(&d.samples),
                    points,
                    colour: COLOURS[i % COLOURS.len()],
                });
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

/// Where the run is relative to the model's own extent.
///
/// A hole in the model and a run that LEAVES the model are different
/// diagnoses, they want different work, and a coverage number cannot tell them
/// apart — both read as "no surface here". 285885 is the case that named this:
/// its ghost spans z 656..1760 and the assembled model stops at z 1632, so the
/// map's whole endgame, including a finish item at (419.0, 144.0, 1704.6), is
/// **outside the model** rather than missing from it. Reported by
/// `f9c585b3`, who then found the surface with a live-engine drop probe.
pub struct Containment {
    pub outside: usize,
    pub model: ([f32; 3], [f32; 3]),
    pub run: ([f32; 3], [f32; 3]),
}

pub fn containment(
    scene_bounds: Option<([f32; 3], [f32; 3])>,
    pts: &[[f32; 3]],
) -> Option<Containment> {
    let (lo, hi) = scene_bounds?;
    if pts.is_empty() {
        return None;
    }
    let mut rlo = [f32::INFINITY; 3];
    let mut rhi = [f32::NEG_INFINITY; 3];
    let mut outside = 0usize;
    for p in pts {
        for a in 0..3 {
            rlo[a] = rlo[a].min(p[a]);
            rhi[a] = rhi[a].max(p[a]);
        }
        // Only x and z, and only the TOP in y: a car above everything the
        // model has is over nothing, a car below the model's floor is not a
        // thing that happens, and the y floor is the stadium's foundations.
        if p[0] < lo[0] || p[0] > hi[0] || p[2] < lo[2] || p[2] > hi[2] || p[1] > hi[1] {
            outside += 1;
        }
    }
    Some(Containment {
        outside,
        model: (lo, hi),
        run: (rlo, rhi),
    })
}

/// The grading of one run against the model.
///
/// Two coverage numbers are printed and both are needed. **raw** is every
/// sample with any surface straight below within reach, over every sample —
/// the number the first corpus run reported, kept so a before/after comparison
/// is like for like. **owed** counts only the samples the model is answerable
/// for: the recording says the car was standing on something. A sample the
/// recording says was in flight is not a hole in the model.
///
/// Both controls on that split are printed beside it. The mean vertical
/// acceleration under each value of the recording's contact bit: if the bit
/// means what its name says, the airborne rows read the map's gravity (about
/// −24.6 m/s²) and the contact rows read near zero. And how much of the run
/// was upright, which is the check on the quaternion the down-axis probe is
/// aimed by — a flat map that is not nearly all upright has a broken
/// quaternion, not an interesting road.
fn grade(name: &str, v: &mapgeom::coverage::Verdict) {
    use mapgeom::coverage::Class;
    let n = v.classes.len();
    println!(
        "  {}: {}/{} samples over a surface ({:.1} % raw)",
        name,
        v.gaps.iter().filter(|g| g.is_finite()).count(),
        n,
        100.0 * v.raw_fraction()
    );
    println!(
        "    accounted for       {} resting, {} loose, {} airborne, {} MISSING SURFACE",
        v.count(Class::Resting),
        v.count(Class::Loose),
        v.count(Class::Airborne),
        v.count(Class::Missing),
    );
    if v.owed() > 0 {
        println!(
            "    of the {} samples the model owes, {:.1} % have a surface \
             ({} of them on a block that MOVES, drawn at its rest pose)",
            v.owed(),
            100.0 * v.covered_fraction(),
            v.on_moving(),
        );
    }
    println!(
        "    controls            contact bit: in contact {:.1} m/s^2 (n {}), airborne {:.1} \
         m/s^2 (n {}), agrees with free-fall on {:.1} % -- {}; median car tilt {:.1} deg",
        v.accel_contact.0,
        v.accel_contact.1,
        v.accel_air.0,
        v.accel_air.1,
        100.0 * v.bit_vs_freefall.0 as f32 / v.bit_vs_freefall.1.max(1) as f32,
        if v.trusted_bit {
            "BIT USED"
        } else {
            "BIT REJECTED, free-fall used instead"
        },
        v.median_tilt(),
    );
    if v.gaps.iter().any(|g| g.is_finite()) {
        println!(
            "    gap below the car   median {:.3} m   p10 {:.3}   p90 {:.3}   \
             tightest half-window +/-{:.3} m",
            v.median_gap(),
            v.gap_pct(0.10),
            v.gap_pct(0.90),
            v.tightest_half()
        );
        println!(
            "    ride height on the car's own down axis   median {:.3} m   p90 {:.3}",
            v.median_ride(),
            v.ride_pct(0.90)
        );
        let mats = v.materials();
        let total: usize = mats.values().sum();
        let mut mats: Vec<(&String, &usize)> = mats.iter().collect();
        mats.sort_by_key(|(_, n)| std::cmp::Reverse(**n));
        let line: Vec<String> = mats
            .iter()
            .take(6)
            .map(|(m, k)| format!("{} {:.0}%", m, 100.0 * **k as f32 / total.max(1) as f32))
            .collect();
        println!("    driven over         {}", line.join(", "));
    }
}

fn flag(args: &[String], name: &str) -> Option<String> {
    args.iter()
        .position(|a| a == name)
        .and_then(|i| args.get(i + 1))
        .cloned()
}

/// What the walk found, and -- just as loudly -- what it could not open.
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
    let mut mats: Vec<(&String, usize)> = scene
        .groups
        .iter()
        .map(|(k, g)| (k, g.tris.len()))
        .collect();
    mats.sort_by_key(|(_, n)| std::cmp::Reverse(*n));
    for (m, n) in mats.iter().take(12) {
        println!("  {:>9} {}", n, m);
    }
    if s.recovered > 0 {
        println!(
            "  {} nodes recovered past a layout with no reader",
            s.recovered
        );
    }
    if !s.unhandled.is_empty() {
        let mut u: Vec<(&u32, &usize)> = s.unhandled.iter().collect();
        u.sort();
        let list: Vec<String> = u
            .iter()
            .map(|(c, n)| format!("0x{:08X} x{}", c, n))
            .collect();
        println!("  classes with no geometry reader: {}", list.join(", "));
    }
    if !s.missing.is_empty() {
        let mut seen = std::collections::BTreeSet::new();
        let mut lines = Vec::new();
        for (f, e) in &s.missing {
            if seen.insert(f.clone()) {
                lines.push(format!("    {}\n      {}", f, e));
            }
        }
        println!("  {} files could NOT be opened:", lines.len());
        for l in lines.iter().take(20) {
            println!("{}", l);
        }
    }
}

fn write_scene(scene: &mapgeom::scene::Scene, out: &str) {
    if out.to_lowercase().ends_with(".obj") {
        let mtl = format!("{}.mtl", out.trim_end_matches(".obj"));
        let (o, m) = scene.obj(
            std::path::Path::new(&mtl)
                .file_name()
                .unwrap()
                .to_string_lossy()
                .as_ref(),
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
        Node::Prefab(p) => {
            // entity poses (model ref, position, rotation quaternion) up to 12:
            // where a prefab puts its parts is what decides how a dyna object
            // (a rotor wheel, a piston) stands before its animation runs
            let mut s = format!("CPlugPrefab, {} entities", p.ents.len());
            for (i, e) in p.ents.iter().take(12).enumerate() {
                s.push_str(&format!("\n      entity {i}: model node {} pos [{:.3}, {:.3}, {:.3}] rot xyzw [{:.4}, {:.4}, {:.4}, {:.4}]", e.model, e.pos[0], e.pos[1], e.pos[2], e.rot[0], e.rot[1], e.rot[2], e.rot[3]));
            }
            if p.ents.len() > 12 {
                s.push_str(&format!("\n      … {} more", p.ents.len() - 12));
            }
            s
        }
        Node::Dyna(d) => format!(
            "CPlugDynaObjectModel mesh={} moving shape={} static shape={}",
            d.mesh, d.dyna_shape, d.static_shape
        ),
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
        Node::VertexStream(v) => {
            format!(
                "CPlugVertexStream, {} positions, {} normals",
                v.positions.len(),
                v.normals.len()
            )
        }
        Node::Crystal(c) => format!(
            "CPlugCrystal, {} meshes, {} faces, materials [{}]",
            c.meshes.len(),
            c.meshes.iter().map(|m| m.faces.len()).sum::<usize>(),
            c.materials
                .iter()
                .map(|(n, i)| if n.is_empty() {
                    format!("node{}", i)
                } else {
                    n.clone()
                })
                .collect::<Vec<_>>()
                .join(" ")
        ),
        Node::Material(n, p) => {
            format!("material {} ({})", n, mapgeom::scene::physics_name(*p))
        }
        Node::ItemModel(i) => format!("item model -> node {}", i),
        Node::BlockInfo(b) => format!(
            "CGameCtnBlockInfo, base variants ground={} air={}, additional ground {} air {}, waypoint {:?}{}",
            b.variant_base_ground, b.variant_base_air, b.additional_ground.len(), b.additional_air.len(), b.waypoint_type,
            if b.clip_type.is_some() || b.asym_clip_id.is_some() { " (CLIP)" } else { "" }
        ),
        Node::Variant(v) => format!(
            "CGameCtnBlockInfoVariant {:?}, {} units, mobils {:?}, cardinal {}, sym {}",
            v.name, v.block_units.len(), v.mobils, v.cardinal_dir, v.symmetrical_variant_index
        ),
        Node::BlockUnit(u) => format!(
            "CGameCtnBlockUnitInfo offset {:?} clips N{:?} E{:?} S{:?} W{:?} T{:?} B{:?} terrain {:?}",
            u.offset, u.clips[0], u.clips[1], u.clips[2], u.clips[3], u.clips[4], u.clips[5], u.terrain_modifier_id
        ),
        Node::Mobil(m) => format!(
            "CGameCtnBlockInfoMobil v{} prefab=node {} solid=node {} translation {:?} rotation {:?} road chunks {:?} u16 {:?}",
            m.version, m.prefab_fid, m.solid_fid, m.geom_translation, m.geom_rotation, m.road_chunks, m.u16
        ),
        Node::AutoTerrain(a) => format!("CGameCtnAutoTerrain offset {:?} genealogy=node {}", a.offset, a.genealogy),
        Node::Genealogy(z) => format!("CGameCtnZoneGenealogy zones {:?} current {} dir {} id {:?}", z.zone_ids, z.current_index, z.dir, z.current_zone_id),
        Node::RoadChunk(r) => format!(
            "CPlugRoadChunk v{} {:?} points {}/{}/{}/{} id {:?}/{:?} left {:?}..{:?} right {:?}..{:?}",
            r.version, (r.u01, r.u02), r.u03.len(), r.u04.len(), r.u05.len(), r.u07.len(), r.u14, r.u17,
            r.u04.first(), r.u04.last(), r.u05.first(), r.u05.last()
        ),
        Node::Light(c, l) => {
            if *c == 0x0901D000 {
                format!(
                    "CPlugLight gx=node {} flags 0x{:x} image_anim={} period {:?} tail {:?}",
                    l.gx_node, l.flags, l.image_anim, l.anim_period, l.tail
                )
            } else {
                let kind = match *c {
                    0x0400B000 => "GxLightSpot",
                    0x04002000 => "GxLightBall",
                    0x0400A000 => "GxLightFrustum",
                    0x04007000 => "GxLightDirectional",
                    0x04005000 => "GxLightAmbient",
                    0x04003000 => "GxLightPoint",
                    _ => "GxLight",
                };
                format!(
                    "{kind} color [{:.3}, {:.3}, {:.3}] intensity {} diffuse {} shadow {} flare {} gxflags 0x{:x} shadow_rgb {:?}\n      flare size {} bias_z {} | ball flags 0x{:x} radius {} spec {} shadow {} flare {} emitting r {} cyl {} attHTnLR {:?} ambient {:?} hyper2 {:?} u09 {} u0a {}\n      spot flags 0x{:x} inner {} outer {} flare {} inner_sh {} outer_sh {} falloff {} bytes {:?}",
                    l.color[0], l.color[1], l.color[2], l.intensity, l.diffuse_intensity, l.shadow_intensity, l.flare_intensity, l.gx_flags, l.shadow_rgb,
                    l.flare_size, l.flare_bias_z, l.ball_flags, l.radius, l.radius_specular, l.radius_shadow, l.radius_flare, l.emitting_radius, l.emitting_cylinder_len_z, l.att_htnlr, l.ambient_rgb, l.att_hyper2, l.ball_u09, l.ball_u0a,
                    l.spot_flags, l.angle_inner, l.angle_outer, l.angle_flare, l.angle_inner_shadow, l.angle_outer_shadow, l.falloff_exponent, l.spot_bytes
                )
            }
        }
        Node::Other(c) => format!("class 0x{:08X}", c),
    }
}
