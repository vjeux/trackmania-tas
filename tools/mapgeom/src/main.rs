use mapgeom::coverage::RESTING;
use std::collections::BTreeMap;
use mapgeom::node::{Node, Slot};
use mapgeom::{names, store::DataStore, store::STADIUM_KEY};

const USAGE: &str = "\
mapgeom -- TM2020 map geometry

  --packs <dir>     directory holding the game's .pak files
                    (default $TM_SERVER/Packs, else /tmp/tmoracle/server/Packs)
  --key <hex>       Stadium pack key (default: the known one)
  --pak <file[:key]>  a client pack (repeatable; the key is the 32-hex derived key)
  --debug <name,..> diagnostic prints: lookup, decls, trees (`--debug help`)
  --lod-pick <N> [--lod-pick-min-verts <V>]
                    bake ONE detail level of every model (N; 0 = the nearest) at
                    every distance — the size lever for a map over the upload cap;
                    a part whose nearest level has under V vertices keeps it

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
  rename-item IN.Item.Gbx --out F --to NAME [--from NAME]
                                the same item file under another ident (header and
                                body, any length: a pack item as an embedded copy)
  items <file.Map.Gbx> [--out D]   the models a map embeds inside itself
  crash <DUMP.dmp> [--exe Trackmania.exe] [--read ADDR LEN] [--find PAT]
      [--disasm ADDR [N]] [--no-stack] [--quiet]
                                a client crash minidump: exception, registers,
                                faulting instruction, heuristic stack walk as
                                exe RVAs / objdump addresses (CRASH.md)
  tiny-library <file.Map.Gbx> --library-out ZIP --mapping-out TSV [--report TSV]
      [--scale 0.5] [--legacy-zip Nadeo.zip] [--items-dir DIR] [--veget bake|substitute|keep|drop]
      [--collection BlueBay] [--only N,..]
                                every block/item model of the map as a half-scale
                                STATIC item (stage-1 path) + tmmaps tiny mapping
  extract <logical-path> <file>    one pack file, decrypted and decompressed
  blockinfo <logical-path>...    a CGameCtnBlockInfo file, fully typed: kind,
                                variants, units, clips per side, mobil prefabs
  fillers <file.Map.Gbx> [--collection Stadium] [--filter PAT]
      [--cells X0,Z0:X1,Z1] [--covered] [--summary]
                                every recorded clip filler against the block
                                unit face it stands on: occupant, that face's
                                clip list, owner, the piece's clip flags
  shape-audit <file.Map.Gbx>... [--collection Stadium] [--game baked.json]
      [--out TSV] [--all]
                                what every BAKED record would be drawn as: the
                                block info its name resolves to, the variant /
                                mobil its flags index, the prefab, and every
                                fallback; --game sets the editor's own
                                mobil / mobilVar picks against the flag bits
  blockinfo-map <file.Map.Gbx> --out TSV [--no-baked] [--report TSV]
      [--collection BlueBay]
      [--collection BlueBay]
                                every authored block with its picked variant,
                                cells, prefabs and what each side faces
  blockinfo-all [<substring>] [--out TSV] [--clips]
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
  who <file.Map.Gbx> --at X,Y,Z... [--dy M] [--yoff N]
                                WHICH placement owns each surface within M
                                metres (default 4) of Y in the column at X,Z:
                                item index + model, or authored block name
                                (generated fillers are not modelled here)

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
    let mut lod_pick: Option<u32> = None;
    let mut lod_min_verts: i32 = 0;
    let mut it = std::env::args().skip(1);
    while let Some(a) = it.next() {
        match a.as_str() {
            "--packs" => packs = it.next().unwrap_or_default(),
            "--pak" => paks.push(it.next().unwrap_or_default()),
            "--key" => key = it.next().unwrap_or_default(),
            "--debug" => mapgeom::debug::set(&it.next().unwrap_or_default()).unwrap_or_else(|e| die(e)),
            "--lod-pick" => lod_pick = Some(it.next().unwrap_or_default().parse().unwrap_or_else(|_| die("--lod-pick N (a detail level, 0 = nearest)".into()))),
            "--lod-pick-min-verts" => lod_min_verts = it.next().unwrap_or_default().parse().unwrap_or_else(|_| die("--lod-pick-min-verts N (vertices)".into())),
            _ => rest.push(a),
        }
    }
    if let Some(level) = lod_pick {
        mapgeom::static_item::build::set_lod_pick(mapgeom::static_item::build::LodPick { level, min_verts: lod_min_verts }).unwrap_or_else(|e| die(e));
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
            // --dir DIR --out OUT [--where collector|model|both]: the BODY's skin
            // directory fields of one of OUR items — CGameCtnCollector 0x2E001010
            // `SkinDirectory` and/or CGameItemModel 0x2E00201E `SkinDirNameCustom`
            // — set to DIR (`Any\Advertisement6x1\`). The 2026-09-07 probe of
            // which declaration the game reads for a custom item.
            if let Some(dir) = flag(&a.rest, "--dir") {
                let out = flag(&a.rest, "--out").unwrap_or_else(|| die("--out OUT with --dir".to_string()));
                let which = flag(&a.rest, "--where").unwrap_or_else(|| "both".to_string());
                let mut f = mapgeom::static_item::parse_file(&bytes).unwrap_or_else(die);
                let mut set = Vec::new();
                for c in f.item.chunks.iter_mut() {
                    match c {
                        mapgeom::static_item::item::ItemChunk::Skin { skin_directory, extra, .. } if which != "model" => {
                            *skin_directory = dir.clone();
                            // the trailing ref is only written for an EMPTY directory
                            *extra = if dir.is_empty() { Some(mapgeom::static_item::null_ref()) } else { None };
                            set.push("0x2E001010 SkinDirectory");
                        }
                        mapgeom::static_item::item::ItemChunk::Archetype { skin_dir: Some(s), .. } if which != "collector" => {
                            *s = dir.clone();
                            set.push("0x2E00201E SkinDirNameCustom");
                        }
                        _ => {}
                    }
                }
                let written = mapgeom::static_item::write_file(&f);
                std::fs::write(&out, &written).unwrap_or_else(|e| die(e.to_string()));
                let back = mapgeom::static_item::parse_file(&written).unwrap_or_else(die);
                let dirs: Vec<String> = back
                    .item
                    .chunks
                    .iter()
                    .filter_map(|c| match c {
                        mapgeom::static_item::item::ItemChunk::Skin { skin_directory, .. } => Some(format!("collector={skin_directory:?}")),
                        mapgeom::static_item::item::ItemChunk::Archetype { skin_dir, .. } => Some(format!("model={skin_dir:?}")),
                        _ => None,
                    })
                    .collect();
                println!("wrote {out} ({} bytes): set [{}] -> {}", written.len(), set.join(", "), dirs.join(" "));
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
            // --deep [N]: follow the externals transitively (N levels, default
            // 6), each line indented by depth, a file printed once — the
            // survey of what a pack item is made of (its prefab's dyna
            // objects, their meshes, the meshes' materials)
            let deep: Option<usize> = if a.rest.iter().any(|x| x == "--deep") { Some(flag(&a.rest, "--deep").and_then(|v| v.parse().ok()).unwrap_or(6)) } else { None };
            let m = load_any(&mut store, &p);
            println!(
                "{}  class 0x{:08X}  {} nodes",
                m.path, m.class_id, m.num_nodes
            );
            let mut seen: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
            let mut stack: Vec<(usize, u32, String)> = m.externals.iter().rev().map(|(i, p)| (1usize, *i, p.clone())).collect();
            while let Some((depth, idx, path)) = stack.pop() {
                let hit = store.resolve(&path);
                println!(
                    "{}  node {:>4}  {}  {}",
                    "  ".repeat(depth - 1),
                    idx,
                    path,
                    match &hit {
                        Some(h) if *h == path => "(stored by name)".to_string(),
                        Some(h) => format!("-> {}", h),
                        None => "*** NOT IN PACK ***".to_string(),
                    }
                );
                let Some(max) = deep else { continue };
                if depth >= max || hit.is_none() || !seen.insert(path.to_ascii_lowercase()) {
                    continue;
                }
                match store.load_model(&path) {
                    Ok(child) => {
                        let mut kids: Vec<(usize, u32, String)> = child.externals.iter().map(|(i, p)| (depth + 1, *i, p.clone())).collect();
                        kids.reverse();
                        stack.extend(kids);
                    }
                    Err(e) => println!("{}    ({e})", "  ".repeat(depth - 1)),
                }
            }
        }
        "vstream-shift" => {
            // vstream-shift IN.Gbx --out OUT --dy DY: every vertex POSITION of the file's
            // vertex streams moved by DY in y, patched in place (an uncompressed Gbx —
            // the TMX custom blocks). The positions are located by their parsed
            // values (the first two vertices' 24 bytes), so no stream-layout
            // guessing (2026-09-10: the water-volume probe of a custom block needs
            // its solid deck out of the archetype's volume).
            let p = a.rest.get(1).cloned().unwrap_or_else(|| die("vstream-shift IN.Gbx --out OUT --dy DY".into()));
            let out = flag(&a.rest, "--out").unwrap_or_else(|| die("--out FILE".into()));
            let dy: f32 = flag(&a.rest, "--dy").unwrap_or_else(|| die("--dy DY".into())).parse().unwrap_or_else(|_| die("--dy: not a number".into()));
            let mut bytes = std::fs::read(&p).unwrap_or_else(|e| die(e.to_string()));
            if bytes.get(7) != Some(&b'U') {
                die::<()>(format!("{p}: body is compressed; this patches bytes in place"));
            }
            let m = mapgeom::store::Model::parse(&bytes, &p).unwrap_or_else(die);
            let g = m.graph().unwrap_or_else(die);
            let mut streams = 0usize;
            let mut moved = 0usize;
            for s in g.slots.iter() {
                let Slot::Node(Node::VertexStream(v)) = s else { continue };
                if v.positions.len() < 2 {
                    continue;
                }
                streams += 1;
                let mut key = Vec::with_capacity(24);
                for q in &v.positions[..2] {
                    for c in q {
                        key.extend_from_slice(&c.to_le_bytes());
                    }
                }
                let Some(off) = bytes.windows(24).position(|w| w == key.as_slice()) else {
                    eprintln!("  stream with {} positions: bytes not found in the file (compressed Dec3N positions?)", v.positions.len());
                    continue;
                };
                for k in 0..v.positions.len() {
                    let o = off + k * 12 + 4;
                    let y = f32::from_le_bytes(bytes[o..o + 4].try_into().unwrap());
                    bytes[o..o + 4].copy_from_slice(&(y + dy).to_le_bytes());
                    moved += 1;
                }
            }
            std::fs::write(&out, &bytes).unwrap_or_else(|e| die(e.to_string()));
            println!("{p}: {streams} vertex streams, {moved} positions moved by {dy} in y -> {out}");
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
            // a kinematic constraint file (0x2F0CA000) has no chunk framing:
            // the dyna reader prints it as one summary line plus its curves
            if m.class_id == 0x2F0CA000 {
                let k = mapgeom::static_item::dyna::KinematicConstraint::parse_body(&m.body).unwrap_or_else(die);
                println!("{}  class 0x{:08X}  kinematic constraint v{}.{}", m.path, m.class_id, k.version, k.sub_version);
                println!("  {}", k.summary());
                println!("  trans curve: {:?}", k.trans);
                println!("  rot curve:   {:?}", k.rot);
                if !k.shader_tc_anim.is_empty() {
                    println!("  shader tc keyframes (ms, sub texture): {:?}  trans-sub {:?}", k.shader_tc_anim, k.shader_tc_trans_sub);
                }
                return;
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
        "rename-item" => {
            // rename-item IN.Item.Gbx --out OUT --to NAME [--from NAME] : the same file under
            // another ident (header + body, any length; the pack item Flag16m -> FlagCopy.Item.Gbx)
            let inp = a.rest.get(1).unwrap_or_else(|| die("rename-item IN.Item.Gbx --out OUT --to NAME".into()));
            let bytes = std::fs::read(inp).unwrap_or_else(|e| die(format!("{inp}: {e}")));
            let out = flag(&a.rest, "--out").unwrap_or_else(|| die("--out FILE".into()));
            let to = flag(&a.rest, "--to").unwrap_or_else(|| die("--to NAME".into()));
            let from = match flag(&a.rest, "--from") {
                Some(f) => f,
                None => tmmaps::header::item_ident_author(&bytes).map(|(i, _)| i).unwrap_or_else(|| die("no ident in the header; pass --from".into())),
            };
            let renamed = mapgeom::crystal::rename_ident(&bytes, &from, &to);
            let check = tmmaps::header::item_ident_author(&renamed);
            std::fs::write(&out, &renamed).expect("write");
            println!("wrote {out}: ident {from:?} -> {to:?} (header now reads {check:?}, {} -> {} bytes)", bytes.len(), renamed.len());
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
        // fx-dump [--check] FILE…: the typed particle reader on an extracted
        // FxSys / ParticleModel file, with a byte-identical round-trip check.
        "fx-dump" => {
            mapgeom::static_item::cli::fx_dump(&a.rest).unwrap_or_else(die);
        }
        "item-check" => {
            let mut open_store = || open(&a);
            mapgeom::static_item::check::run(&a.rest, &mut open_store).unwrap_or_else(die);
        }
        // surfhist <pack prefab | pack .StaticObject.Gbx | local .Item.Gbx>...:
        // the collision census source by source — per surface its material
        // nodes and id table, per (physics, gameplay, index) the triangles,
        // how many face up, the height range (the Rubber-vs-Asphalt road
        // question of 2026-09-07).
        "surfhist" => {
            let mut open_store = || open(&a);
            mapgeom::static_item::surfhist::run(&a.rest, &mut open_store).unwrap_or_else(die);
        }
        // surf <pack .Shape.Gbx | local file>...: one CPlugSurface as parsed —
        // materials, id table, main direction, triangle (physics, gameplay)
        // histogram (the gate trigger slabs, 2026-09-08).
        "surf" => {
            let mut open_store = || open(&a);
            mapgeom::static_item::surfhist::surf(&a.rest, &mut open_store).unwrap_or_else(die);
        }
        // item-fields <pack .Item.Gbx | file>: every chunk of a CGameItemModel
        // as the item writer's classes read it, one line each (node contents
        // elided) — what a pack item declares that ours does not (the
        // 2026-09-07 detail-level probe).
        // veget-info <VegetTreeModel path | Item.Gbx of a vegetation item>: the
        // tree model's visuals (CPlugVisualIndexedTriangles nodes inline in the
        // table-less CPlugVegetTreeModel struct) and their bounding boxes — the
        // species' height and radius, for sinking a full-size tree so its crown
        // sits where a half tree's would (vjeux, 2026-09-07). An .Item.Gbx is
        // followed to its VegetTreeModel reference.
        // veget-bake <VegetTreeModel path | vegetation Item.Gbx> --out F.Item.Gbx
        //   --ident NAME.Item.Gbx [--author A] [--scale 0.5] [--collection N]
        //   [--pictures DIR]: one species as a half-size static item (the tree
        //   bake of tiny-library, on its own); its textures go to DIR (default:
        //   next to F) — they must ride next to the item in the map archive.
        "veget-bake" => {
            let mut store = open(&a);
            let p = a.rest.get(1).cloned().unwrap_or_else(|| die("veget-bake <path> --out F --ident NAME.Item.Gbx".into()));
            let out = flag(&a.rest, "--out").unwrap_or_else(|| die("--out F.Item.Gbx".into()));
            let ident = flag(&a.rest, "--ident").unwrap_or_else(|| std::path::Path::new(&out).file_name().map(|f| f.to_string_lossy().into_owned()).unwrap_or_default());
            let author = flag(&a.rest, "--author").unwrap_or_else(|| ident.clone());
            let scale: f32 = flag(&a.rest, "--scale").and_then(|s| s.parse().ok()).unwrap_or(0.5);
            let collection: u32 = flag(&a.rest, "--collection").and_then(|s| s.parse().ok()).unwrap_or(26);
            let (bytes, m, bake) = mapgeom::static_item::build::static_item_from_veget_report(&mut store, &p, &ident, &author, scale, collection).unwrap_or_else(|e| die(format!("{p}: {e}")));
            let bytes = mapgeom::tiny_assets::set_ident_collection(&bytes, collection);
            std::fs::write(&out, &bytes).unwrap_or_else(|e| die(format!("{out}: {e}")));
            let dir = flag(&a.rest, "--pictures").map(std::path::PathBuf::from).unwrap_or_else(|| std::path::Path::new(&out).parent().map(|d| d.to_path_buf()).unwrap_or_default());
            for (file, dds) in &m.pictures {
                let target = dir.join(file);
                std::fs::write(&target, dds).unwrap_or_else(|e| die(format!("{}: {e}", target.display())));
            }
            // the sidecar node files of the kinematic `file` form (the dyna object and its mesh)
            let sidecars = mapgeom::static_item::assemble::SIDECARS.with(|s| std::mem::take(&mut *s.borrow_mut()));
            for (name, data) in &sidecars {
                let target = dir.join(name);
                std::fs::write(&target, data).unwrap_or_else(|e| die(format!("{}: {e}", target.display())));
                println!("  sidecar {} ({} bytes)", target.display(), data.len());
            }
            println!("{p}: {} bytes -> {out}; {} visuals in {} levels {:?}, switch {:?}, {} materials, hull {} tris, height {:.2} radius {:.2} (source metres); textures: {}", bytes.len(), m.visuals.len(), bake.levels.len(), bake.levels, bake.switch, m.materials.len(), bake.hull_triangles, bake.height, bake.radius, bake.textures.iter().map(|(f, n)| format!("{f} {n} B")).collect::<Vec<_>>().join(", "));
            for n in &m.notes {
                println!("  {n}");
            }
        }
        "veget-info" => {
            let mut store = open(&a);
            let paths: Vec<String> = a.rest.iter().skip(1).filter(|p| !p.starts_with("--")).cloned().collect();
            if paths.is_empty() {
                die::<()>("veget-info <path>…".into());
            }
            let brief = paths.len() > 1 || a.rest.iter().any(|x| x == "--brief");
            for p in &paths {
                match mapgeom::veget::parse_tree_model(&mut store, p) {
                    Ok(m) => {
                        let s = m.stats();
                        let lods: Vec<String> = m.lods.iter().map(|l| format!("{}", l.iter().map(|e| format!("{}v", e.visual.main.as_ref().map(|mm| mm.count).unwrap_or(0))).collect::<Vec<_>>().join("+"))).collect();
                        println!("{p}: {} levels [{}] switch {:?} far {} ; {} materials ({}); hull {} verts {} tris; bottom {:.2} top {:.2} (height {:.2}) radius {:.2} m", m.lods.len(), lods.join(" | "), m.switch, m.far, m.materials.len(), m.materials.iter().map(|mt| format!("{}{}", mt.name, if mt.leaf { "*" } else { "" })).collect::<Vec<_>>().join(", "), m.hull_vertices.len(), m.hull_triangles.len(), s.bottom, s.top, s.top - s.bottom, s.radius);
                        if brief {
                            continue;
                        }
                        for (i, mt) in m.materials.iter().enumerate() {
                            println!("  material {i} {} leaf {} f {}: images {:?} texture nodes {:?} extra {:?}", mt.name, mt.leaf, mt.f, mt.images.iter().map(|t| t.as_deref().map(|s| s.rsplit('\\').next().unwrap_or(s)).unwrap_or("-")).collect::<Vec<_>>(), mt.texture_nodes.iter().map(|t| t.as_deref().map(|s| s.rsplit('\\').next().unwrap_or(s)).unwrap_or("-")).collect::<Vec<_>>(), mt.extra);
                        }
                        for (l, lod) in m.lods.iter().enumerate() {
                            for e in lod {
                                let mm = e.visual.main.as_ref();
                                let decl = mm.and_then(|mm| mm.vertex_streams.first()).and_then(|r| r.inline.as_deref()).map(|n| if let mapgeom::static_item::Node::VertexStream(s) = n { s.decls.iter().map(|d| format!("{:x}", d.name())).collect::<Vec<_>>().join(",") } else { "?".into() }).unwrap_or_default();
                                println!("  level {l}: material {} ({}) node {} flag {} {} verts {} indices; centre ({:.2}, {:.2}, {:.2}) half ({:.2}, {:.2}, {:.2}); decl [{}]", e.material, m.materials.get(e.material as usize).map(|x| x.name.as_str()).unwrap_or("?"), e.node_index, e.flag, mm.map(|x| x.count).unwrap_or(0), e.visual.index_buffer.as_ref().map(|ib| ib.indices.len()).unwrap_or(0), mm.map(|x| x.bounding_box[0]).unwrap_or(0.0), mm.map(|x| x.bounding_box[1]).unwrap_or(0.0), mm.map(|x| x.bounding_box[2]).unwrap_or(0.0), mm.map(|x| x.bounding_box[3]).unwrap_or(0.0), mm.map(|x| x.bounding_box[4]).unwrap_or(0.0), mm.map(|x| x.bounding_box[5]).unwrap_or(0.0), decl);
                                if std::env::var_os("MAPGEOM_VEGET_NORMALS").is_some() {
                                    if let Some(s) = e.visual.stream() {
                                        let compress = s.compress_local3d.unwrap_or(false);
                                        for (d, el) in s.decls.iter().zip(s.elems.iter()) {
                                            let name = d.name();
                                            if !(name == 5 || name == 0x12 || name == 0x14 || name == 8 || name == 9) {
                                                continue;
                                            }
                                            let vecs: Vec<[f32; 3]> = match el {
                                                mapgeom::static_item::vstream::Elem::Word(w) if d.stored_type(compress) == mapgeom::static_item::vstream::T_DEC3N => w.iter().map(|x| mapgeom::static_item::build::dec3n_unpack(*x)).collect(),
                                                mapgeom::static_item::vstream::Elem::Float3(p) => p.clone(),
                                                mapgeom::static_item::vstream::Elem::Word(w) => {
                                                    let mut hist = std::collections::BTreeMap::new();
                                                    for x in w { *hist.entry(*x).or_insert(0usize) += 1; }
                                                    let mut top: Vec<_> = hist.into_iter().collect();
                                                    top.sort_by(|a, b| b.1.cmp(&a.1));
                                                    println!("      elem 0x{name:x} (word): {} distinct, top {:?}", top.len(), top.iter().take(6).map(|(v, n)| format!("{v:08x} x{n}")).collect::<Vec<_>>());
                                                    continue;
                                                }
                                                _ => continue,
                                            };
                                            let n = vecs.len().max(1) as f32;
                                            let mean_len = vecs.iter().map(|v| (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt()).sum::<f32>() / n;
                                            let short = vecs.iter().filter(|v| (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt() < 0.5).count();
                                            let mean = vecs.iter().fold([0.0f32; 3], |a, v| [a[0] + v[0] / n, a[1] + v[1] / n, a[2] + v[2] / n]);
                                            let up = vecs.iter().filter(|v| v[1] > 0.5).count();
                                            println!("      elem 0x{name:x}: mean |v| {mean_len:.3}, {short} shorter than 0.5, mean ({:.2}, {:.2}, {:.2}), {up} with y > 0.5, first {:?}", mean[0], mean[1], mean[2], vecs.iter().take(3).map(|v| format!("({:.2},{:.2},{:.2})", v[0], v[1], v[2])).collect::<Vec<_>>());
                                        }
                                    }
                                }
                            }
                        }
                        if std::env::var_os("MAPGEOM_VEGET_SHAPE").is_some() {
                            for line in mapgeom::veget::shape_report(&m) {
                                println!("{line}");
                            }
                        }
                        // twin visuals (same level, material and vertex count): the same
                        // mesh again, its mirror (reversed winding: an explicit back face),
                        // or a different one
                        for (l, lod) in m.lods.iter().enumerate() {
                            for a in 0..lod.len() {
                                for b in a + 1..lod.len() {
                                    let (ea, eb) = (&lod[a], &lod[b]);
                                    let same_count = ea.visual.main.as_ref().map(|x| x.count) == eb.visual.main.as_ref().map(|x| x.count);
                                    if ea.material != eb.material || !same_count {
                                        continue;
                                    }
                                    let (ia, ib) = (ea.visual.index_buffer.as_ref().map(|x| x.indices.clone()).unwrap_or_default(), eb.visual.index_buffer.as_ref().map(|x| x.indices.clone()).unwrap_or_default());
                                    let reversed = ia.len() == ib.len() && ia.chunks(3).zip(ib.chunks(3)).all(|(p, q)| p.len() == 3 && q.len() == 3 && p[0] == q[0] && p[1] == q[2] && p[2] == q[1]);
                                    let (pa, pb) = (mapgeom::static_item::build::visual_triangles(&ea.visual).0, mapgeom::static_item::build::visual_triangles(&eb.visual).0);
                                    println!("  level {l}: visuals {a} and {b} share material {} and {} vertices: indices {}, positions {}", ea.material, ea.visual.main.as_ref().map(|x| x.count).unwrap_or(0), if ia == ib { "IDENTICAL" } else if reversed { "REVERSED (a back-face copy)" } else { "different" }, if pa == pb { "identical" } else { "different" });
                                }
                            }
                        }
                        let hull_ids: std::collections::BTreeSet<u32> = m.hull_triangles.iter().map(|(_, id)| *id).collect();
                        println!("  hull material ids {:?}; file time {:#x}; tail {} bytes at {:#x}", hull_ids, m.file_write_time, m.tail.len(), m.tail_at);
                    }
                    Err(e) => {
                        // the box-scan fallback: heights of a species whose typed parse fails
                        match mapgeom::veget::tree_model_stats(&mut store, p) {
                            Ok(s) => println!("{p}: TYPED PARSE FAILED ({e}); scan: {} visuals; bottom {:.2} top {:.2} (height {:.2}) radius {:.2} m", s.visuals.len(), s.bottom, s.top, s.top - s.bottom, s.radius),
                            Err(e2) => println!("{p}: FAILED: {e}; scan: {e2}"),
                        }
                    }
                }
            }
        }
        "item-fields" => {
            let mut store = open(&a);
            let p = a.rest.get(1).cloned().unwrap_or_default();
            let m = load_any(&mut store, &p);
            let mut lb = mapgeom::static_item::LookbackState::default();
            lb.defined_nodes.extend(m.external_indices().iter().copied());
            let mut r = mapgeom::static_item::Rd::new(&m.body, 0, lb);
            let item = mapgeom::static_item::item::CGameItemModel::parse(&mut r).unwrap_or_else(|e| die(format!("{p}: {e}")));
            println!("{p}: {} chunks, {} of {} body bytes read", item.chunks.len(), r.o, m.body.len());
            for c in &item.chunks {
                let s = format!("{c:?}");
                let cap = std::env::var("MAPGEOM_FIELDS_CAP").ok().and_then(|v| v.parse().ok()).unwrap_or(400usize);
                let s = if s.len() > cap { format!("{}…", &s[..cap]) } else { s };
                println!("  {s}");
            }
            for (i, e) in &m.externals {
                println!("  external node {i}: {e}");
            }
        }
        // solid2-roundtrip <pack .Mesh.Gbx | file>: parse a CPlugSolid2Model body
        // with the item writer's classes and write it back — how faithfully the
        // writer reproduces a pack mesh (the flag cloth's inline-vertex frames,
        // 2026-09-07). Prints the two lengths and the first differing offset.
        "solid2-roundtrip" => {
            let mut store = open(&a);
            let p = a.rest.get(1).cloned().unwrap_or_default();
            let m = load_any(&mut store, &p);
            let mut lb = mapgeom::static_item::LookbackState::default();
            lb.defined_nodes.extend(m.external_indices().iter().copied());
            let mut r = mapgeom::static_item::Rd::new(&m.body, 0, lb);
            let s2 = mapgeom::static_item::solid2::CPlugSolid2Model::parse(&mut r).unwrap_or_else(|e| die(format!("{p}: {e}")));
            let consumed = r.o;
            let mut out: Vec<u8> = Vec::new();
            let mut lbw = mapgeom::static_item::LookbackState::default();
            lbw.defined_nodes.extend(m.external_indices().iter().copied());
            let mut w = mapgeom::static_item::Wr { w: &mut out, lb: &mut lbw };
            s2.write(&mut w);
            let first_diff = m.body.iter().zip(out.iter()).position(|(x, y)| x != y);
            let same = m.body.len() == out.len() && first_diff.is_none();
            println!("{p}: body {} bytes ({consumed} consumed by the parser), rewritten {} bytes, {}", m.body.len(), out.len(), if same { "IDENTICAL".to_string() } else { format!("first difference at 0x{:x}", first_diff.unwrap_or(m.body.len().min(out.len()))) });
            println!(
                "  solid2 v{} lod_max_dist {:?} vis_cst_type {} damage_zone {} flags {:#x} u05 {} u06 {:?} u07 {} boxes {:?} joints {} u10 {:?} u11 {} u12 {:?} u13 {} u15 {} u16 {} u17 {:?} u18 {} u19 {:?} pre_light_gen {} file_write_time {} u03 {:?} folder {:?} u04 {:?} material_ids {:?} materials {} custom {} raw {:?}",
                s2.version, s2.lod_max_dist, s2.vis_cst_type, s2.damage_zone, s2.flags, s2.u05, s2.u06, s2.u07, s2.boxes, s2.joints.len(), s2.u10, s2.u11, s2.u12, s2.u13, s2.u15, s2.u16, s2.u17, s2.u18, s2.u19, s2.pre_light_gen.is_some(), s2.file_write_time, s2.u03, s2.materials_folder, s2.u04, s2.material_ids, s2.materials.len(), s2.custom_materials.len(), s2.raw.iter().map(|r| (r.id, r.payload.len())).collect::<Vec<_>>()
            );
            for vr in &s2.visuals {
                if let Some(mapgeom::static_item::Node::Visual(v)) = vr.inline.as_deref() {
                    println!("  visual: {} vertices, {} sub-visuals, inline form {}, {} uv sets (flags {}), chunks {:x?}, u_node {} v3d_node {} u_float {} morph {:?} splits {} chunk_flags {:#x} version {} bbox {:?} bitmap_elems {} uv_groups {} u02 {} u03 {} u04 {:?}", v.main.as_ref().map(|m| m.count).unwrap_or(0), v.sub_visuals.len(), v.inline_form, v.inline_uv_sets, v.inline_uv_flags, v.chunks, v.u_node.index, v.v3d_node.index, v.u_float, v.morph, v.splits.len(), v.main.as_ref().map(|m| m.chunk_flags).unwrap_or(0), v.main.as_ref().map(|m| m.version).unwrap_or(0), v.main.as_ref().map(|m| m.bounding_box).unwrap_or([0.0; 6]), v.main.as_ref().map(|m| m.bitmap_elems.len()).unwrap_or(0), v.main.as_ref().map(|m| m.uv_groups.len()).unwrap_or(0), v.main.as_ref().map(|m| m.u02).unwrap_or(0), v.main.as_ref().map(|m| m.u03).unwrap_or(0), v.main.as_ref().map(|m| m.u04.clone()).unwrap_or_default());
                }
            }
            if let Some(out_path) = flag(&a.rest, "--out") {
                std::fs::write(&out_path, &out).unwrap_or_else(|e| die(e.to_string()));
                println!("wrote {out_path}");
            }
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
            let mut renamed = mapgeom::tiny_assets::rename_item_ident(&bytes, &old_name, &old_author, &ident, &author);
            // --ancestor N: the reference table's ancestor level (how many
            // folders up from the file's own the external paths start) — the
            // 2026-09-07 probe of where an embedded item's folder sits in the
            // game's file tree (its pack refs resolve from `Stadium\Items\`,
            // not from a map archive's `Items\`).
            if let Some(n) = flag(&a.rest, "--ancestor") {
                let n: u32 = n.parse().unwrap_or_else(|_| die("--ancestor N".into()));
                let mut g = tmmaps::gbx::Gbx::parse(&renamed);
                if g.ref_table.len() < 8 || u32::from_le_bytes(g.ref_table[0..4].try_into().unwrap()) == 0 {
                    die::<()>(format!("{inp}: no external references to re-root"));
                }
                let old = u32::from_le_bytes(g.ref_table[4..8].try_into().unwrap());
                g.ref_table[4..8].copy_from_slice(&n.to_le_bytes());
                let body = g.body.clone();
                renamed = if g.comp.is_some() { g.write_body_recompressed(&body) } else { g.write_body_uncompressed(&body) };
                println!("  reference table ancestor level {old} -> {n}");
            }
            std::fs::write(&out, &renamed).expect("write");
            println!("wrote {out}: ident {old_name:?} by {old_author:?} -> {ident:?} by {author:?} ({} bytes)", renamed.len());
        }
        // dds-from-raw IN.rgba WxH OUT.dds: an uncompressed RGBA8 DDS (128-byte
        // header + BGRA rows) from ffmpeg's `-f rawvideo -pix_fmt rgba` bytes —
        // the texture format the game's own images use, for the archive-texture
        // probe of 2026-09-07 (PNG/JPG user textures never resolved).
        "dds-from-raw" => {
            let inp = a.rest.get(1).cloned().unwrap_or_else(|| die("dds-from-raw IN.rgba WxH OUT.dds".to_string()));
            let dims = a.rest.get(2).cloned().unwrap_or_else(|| die("dds-from-raw IN.rgba WxH OUT.dds".to_string()));
            let out = a.rest.get(3).cloned().unwrap_or_else(|| die("dds-from-raw IN.rgba WxH OUT.dds".to_string()));
            let (w, h) = dims.split_once('x').map(|(w, h)| (w.parse::<u32>().unwrap_or(0), h.parse::<u32>().unwrap_or(0))).unwrap_or((0, 0));
            let rgba = std::fs::read(&inp).unwrap_or_else(|e| die(format!("{inp}: {e}")));
            if w == 0 || h == 0 || rgba.len() != (w * h * 4) as usize {
                die::<()>(format!("{inp}: {} bytes is not {w}x{h} RGBA", rgba.len()));
            }
            let mut d = Vec::with_capacity(128 + rgba.len());
            d.extend_from_slice(b"DDS ");
            let mut hdr = [0u32; 31];
            hdr[0] = 124; // header size
            hdr[1] = 0x0000_100F; // CAPS | HEIGHT | WIDTH | PIXELFORMAT | PITCH
            hdr[2] = h;
            hdr[3] = w;
            hdr[4] = w * 4; // pitch
            hdr[18] = 32; // pixel format size
            hdr[19] = 0x41; // RGB | ALPHAPIXELS
            hdr[21] = 32; // bit count
            hdr[22] = 0x00FF_0000; // R
            hdr[23] = 0x0000_FF00; // G
            hdr[24] = 0x0000_00FF; // B
            hdr[25] = 0xFF00_0000; // A
            hdr[26] = 0x1000; // caps: TEXTURE
            for v in hdr {
                d.extend_from_slice(&v.to_le_bytes());
            }
            for px in rgba.chunks(4) {
                d.extend_from_slice(&[px[2], px[1], px[0], px[3]]);
            }
            std::fs::write(&out, &d).unwrap_or_else(|e| die(e.to_string()));
            println!("wrote {out}: {w}x{h} RGBA8 DDS, {} bytes", d.len());
        }
        // The engine's reflection tables off the exe: a class's members in
        // declaration order (name, offset, type fn) — the key to a chunk
        // layout the packs do not explain (the particle classes, 2026-09-08).
        "exe-class" => {
            mapgeom::classinfo::run(&a.rest[1..]).unwrap_or_else(die);
        }
        // A client crash dump: where it died, in objdump addresses. CRASH.md.
        "crash" => {
            mapgeom::minidump::run(&a.rest).unwrap_or_else(die);
        }
        // A WPR trace (tracerpt CSV) of the client: where a map load spends its CPU.
        "etlsum" => {
            mapgeom::etlsum::run(&a.rest).unwrap_or_else(die);
        }
        // Every thread's stack in every minidump of a load (shootctl loadprof --dump-every).
        "threads" => {
            mapgeom::threads::run(&a.rest).unwrap_or_else(die);
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
        // pak-foldhunt <logical-path> [--max-len N]: which dummy-write fold
        // sequence lets a compressed pak file's next chunk decode (the probe
        // for a table-less class the node walker cannot read, e.g. the
        // VegetTreeModel files)
        "pak-foldhunt" => {
            let store = open(&a);
            let p = a.rest.get(1).cloned().unwrap_or_else(|| die("pak-foldhunt <logical-path>".into()));
            let max_len: usize = flag(&a.rest, "--max-len").unwrap_or_else(|| "2".into()).parse().unwrap_or(2);
            let show = |folds: &[(usize, u32)]| {
                for (off, c) in folds {
                    println!("{off:#x}\t{c:08X}");
                }
            };
            match store.fold_hunt(&p, max_len) {
                Ok(folds) => {
                    println!("decodes with {} folds:", folds.len());
                    show(&folds);
                }
                Err((msg, folds)) => {
                    println!("stuck: {msg}; {} folds found so far:", folds.len());
                    show(&folds);
                    std::process::exit(2);
                }
            }
        }
        // crystal-layers <Item.Gbx>: a mesh-editor (crystal) item's layers —
        // kind, faces, bounds, and per material the face count with the
        // material's link / physics / gameplay ids. What the game's own item
        // editor writes for a gameplay gate (Nadeo.zip GateSpecialNoEngine: a
        // Trigger layer in `Modifier\NoEngine\Collision`, physics 0 gameplay 4)
        // — the reference for the static form's trigger (2026-09-08).
        "crystal-layers" => {
            for path in a.rest.iter().skip(1) {
                let bytes = std::fs::read(path).unwrap_or_else(|e| die(format!("{path}: {e}")));
                let it = mapgeom::crystal::ItemCrystal::open(&bytes).unwrap_or_else(|e| die(format!("{path}: {e}")));
                println!("{path}: {} materials, {} layers", it.model.materials.len(), it.model.layers.len());
                for (i, m) in it.model.materials.iter().enumerate() {
                    match m.inst().and_then(|x| x.main.as_ref()) {
                        Some(main) => println!("  material {i}: {:?} physics {} gameplay {} (v{}{})", main.link, main.surface_physic_id, main.surface_gameplay_id, main.version, if main.is_using_game_material { ", game material" } else { "" }),
                        None => println!("  material {i}: {} (no inline user inst)", m.name),
                    }
                }
                for (li, l) in it.model.layers.iter().enumerate() {
                    match l.kind.crystal() {
                        Some(c) => {
                            let mut lo = [f32::MAX; 3];
                            let mut hi = [f32::MIN; 3];
                            for p in &c.positions {
                                for k in 0..3 {
                                    lo[k] = lo[k].min(p[k]);
                                    hi[k] = hi[k].max(p[k]);
                                }
                            }
                            let mut h: std::collections::BTreeMap<i32, usize> = Default::default();
                            for f in &c.faces {
                                *h.entry(f.material).or_default() += 1;
                            }
                            println!("  layer {li} {}: {} positions, {} faces, bounds [{:.2}, {:.2}, {:.2}]..[{:.2}, {:.2}, {:.2}], faces per material {:?}", l.kind.name(), c.positions.len(), c.faces.len(), lo[0], lo[1], lo[2], hi[0], hi[1], hi[2], h);
                        }
                        None => println!("  layer {li} {}", l.kind.name()),
                    }
                }
            }
        }
        // prefab-ents <pack .Prefab.Gbx>: every entity of a prefab — model
        // class (or external file), position, rotation, params chunk id and
        // size — the layout an item in prefab form has to reproduce.
        "prefab-ents" => {
            let mut store = open(&a);
            for path in a.rest.iter().skip(1) {
                let model = store.load_model(path).unwrap_or_else(die);
                let prefab = mapgeom::static_item::prefab::CPlugPrefab::from_model(&model).unwrap_or_else(die);
                println!("{path}: prefab v{} {} entities", prefab.version, prefab.ents.len());
                for (i, e) in prefab.ents.iter().enumerate() {
                    let what = match e.model.inline.as_deref() {
                        Some(n) => format!("inline class 0x{:08X}", n.class_id()),
                        None if e.model.index < 0 => "null".to_string(),
                        None => model.externals.iter().find(|(k, _)| *k as i32 == e.model.index).map(|(_, p)| format!("external {p}")).unwrap_or_else(|| format!("node {}", e.model.index)),
                    };
                    println!("  entity {i}: {what} pos {:?} rot {:?} params_id {} ({} bytes) u01 {} bytes", e.pos, e.rot, e.params_id, e.params.len(), e.u01.len());
                    // the trigger structs' own bytes (NPlugTrigger_SWaypoint
                    // {version, type, shape ref, NoRespawn}, SSpawn, 0x0917B000):
                    // small plain bodies whose every word means something
                    if let Some(mapgeom::static_item::Node::Opaque(o)) = e.model.inline.as_deref() {
                        if o.raw.len() <= 96 {
                            let words: Vec<String> = o.raw.chunks(4).map(|c| c.iter().map(|b| format!("{b:02x}")).collect::<Vec<_>>().join("")).collect();
                            println!("           raw: {}", words.join(" "));
                        }
                    }
                    if let Some(mapgeom::static_item::Node::GateSpecial(g)) = e.model.inline.as_deref() {
                        println!("           NPlugTrigger_SGateSpecial version {} shape node {} u01 {}", g.version, g.shape.index, g.u01);
                    }
                    if let Some(mapgeom::static_item::Node::WaypointTrigger(wp)) = e.model.inline.as_deref() {
                        println!("           NPlugTrigger_SWaypoint version {} type {} shape node {} no_respawn {}", wp.version, wp.wtype, wp.shape.index, wp.no_respawn);
                    }
                }
            }
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
        "collhash" => {
            if a.rest.iter().any(|x| x == "--triage") {
                let mut store = open(&a);
                mapgeom::collhash::triage(&mut store, &a.rest).unwrap_or_else(die);
            } else {
                mapgeom::collhash::run(&a.rest).unwrap_or_else(die);
            }
        }
        // the embedded archive + manifest of a map, verified field by field (zipcheck.rs)
        "zipcheck" => mapgeom::zipcheck::run(&a.rest).unwrap_or_else(die),
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
            // the tree bake is the default since 2026-09-08 (the leaf cards draw
            // under TDOSN with the atlas in the DiffuseO slot once the visuals carry a TexCoord1)
            let veget = flag(&a.rest, "--veget").unwrap_or_else(|| "bake".into());
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
        "blockinfo" => {
            let mut store = open(&a);
            // a bare block NAME (no backslash) resolves through the block-info
            // index of --collection (default Stadium), the way tiny-library does
            let coll = flag(&a.rest, "--collection").unwrap_or_else(|| "Stadium".to_string());
            let paths: Vec<String> = a.rest.iter().skip(1).filter(|x| !x.starts_with("--") && **x != coll).cloned().collect();
            if paths.is_empty() {
                die::<()>("blockinfo needs a logical path or a block name".into());
            }
            let mut bad = 0;
            let idx = mapgeom::blockmap::BlockInfoIndex::build(&store, &coll);
            let paths: Vec<String> = paths.into_iter().map(|p| if p.contains('\\') { p } else { idx.path_for(&p).unwrap_or(p) }).collect();
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
            // --clips: one row per CLIP block info with the fields the clip
            // system decides on (type, full-free / exclusive / deletable, the
            // group ids, the v1 pair, horizontal / vertical group) and which
            // variants carry a prefab — the table the filler draw rule is read
            // off (2026-09-08)
            let clips_only = a.rest.iter().any(|x| x == "--clips");
            let mut rows = if clips_only {
                String::from("name\tkind\tclip_type\tfull_free\texclusive\tdeletable\ttop_bottom_multidir\tasym_id\tgroup\tsym_group\tv1_a\tv1_b\thorizontal\tvertical\tground_prefabs\tair_prefabs\talways_visible\tfct_fcb_ignored_by_vfc\tanti_clip\n")
            } else {
                String::from("path\tclass\tstatus\tconsumed\tbody\tkind\tvariants\tdetail\n")
            };
            let (mut ok, mut short, mut fail) = (0, 0, 0);
            for p in &names {
                match mapgeom::blockinfo::load(&mut store, p) {
                    Ok(b) if clips_only => {
                        let Some(c) = b.clip.as_ref() else { continue };
                        if b.parsed_to_end() { ok += 1 } else { short += 1 }
                        let s = |o: &Option<String>| o.clone().unwrap_or_default();
                        let f = |o: Option<bool>| o.map(|v| if v { "1" } else { "0" }).unwrap_or("-");
                        let prefabs = |v: &Option<mapgeom::blockinfo::Variant>| -> String {
                            v.as_ref().map(|v| v.mobils.iter().map(|l| l.iter().filter_map(|m| m.prefab.as_ref()).map(|p| p.rsplit('\\').next().unwrap_or(p).trim_end_matches(".Prefab.Gbx").to_string()).collect::<Vec<_>>().join("+")).map(|s| if s.is_empty() { "-".to_string() } else { s }).collect::<Vec<_>>().join(",")).unwrap_or_default()
                        };
                        let (v1a, v1b) = c.clip_group_ids_v1.clone().unwrap_or_default();
                        rows.push_str(&format!(
                            "{}\t{:?}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\n",
                            b.name, b.kind, c.clip_type.map(mapgeom::blockinfo::clip_type_name).unwrap_or("-"), f(c.is_full_free_clip), f(c.is_exclusive_free_clip), f(c.can_be_deleted_by_full_free_clip),
                            c.top_bottom_multi_dir.map(mapgeom::blockinfo::multi_dir_name).unwrap_or("-"), s(&c.asym_clip_id), s(&c.clip_group_id), s(&c.symmetrical_clip_group_id), v1a, v1b, s(&c.horizontal_clip_group_id), s(&c.vertical_clip_group_id),
                            prefabs(&b.variant_base_ground), prefabs(&b.variant_base_air),
                            // chunk 0x03053006 v2..4, one byte each, in the engine's member order
                            // after CanBeDeletedByFullFreeClip @624: IsAlwaysVisibleFreeClip @628,
                            // IsFCTOrFCBIgnoredByVFC @632, IsAntiClip @636 (/members?t=CGameCtnBlockInfoClip)
                            c.extra_bytes.first().map(|x| x.to_string()).unwrap_or("-".into()), c.extra_bytes.get(1).map(|x| x.to_string()).unwrap_or("-".into()), c.extra_bytes.get(2).map(|x| x.to_string()).unwrap_or("-".into())
                        ));
                    }
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
        "bake" => {
            let mut store = open(&a);
            mapgeom::bake::cmd(&mut store, &a.rest[1..]);
        }
        "fillers" => {
            let mut store = open(&a);
            mapgeom::fillers::cmd(&mut store, &a.rest[1..]);
        }
        "shape-audit" => {
            let mut store = open(&a);
            mapgeom::shape_audit::cmd(&mut store, &a.rest[1..]);
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
        // tex-stats <pack .dds path>… [--max N]: a pack image decoded (BC1/BC3) at
        // the level `dds_cap` would ship, per-channel histograms in eight bins —
        // where a leaf atlas keeps its cut-out mask (the 2026-09-09 tree thread:
        // GreenCoast leaf cards drew as opaque quads under TDSN)
        "tex-stats" => {
            let mut store = open(&a);
            let max: u32 = flag(&a.rest, "--max").and_then(|s| s.parse().ok()).unwrap_or(4096);
            for p in a.rest.iter().skip(1).filter(|x| !x.starts_with("--")) {
                // a path that exists on disk is read as a file (a shipped Items/*.dds)
                let bytes = match std::fs::read(p) { Ok(b) => b, Err(_) => match store.read(p) { Ok(b) => b, Err(e) => { println!("{p}: {e}"); continue; } } };
                let dims = mapgeom::static_item::texture::dds_dims(&bytes);
                let fourcc = if bytes.len() >= 88 { String::from_utf8_lossy(&bytes[84..88]).to_string() } else { String::new() };
                match mapgeom::static_item::texture::decode_capped_rgba(&bytes, max) {
                    Ok((w, h, rgba)) => {
                        let n = (w * h) as usize;
                        let mut hist = [[0usize; 8]; 4];
                        let mut sum = [0u64; 4];
                        for px in rgba.chunks(4) {
                            for c in 0..4 { hist[c][(px[c] >> 5) as usize] += 1; sum[c] += px[c] as u64; }
                        }
                        let names = ["R", "G", "B", "A"];
                        println!("{p}: {fourcc} {:?} -> {w}x{h} decoded", dims);
                        for c in 0..4 {
                            println!("  {}: mean {:5.1}  bins(0-31..224-255) {}", names[c], sum[c] as f64 / n as f64, hist[c].iter().map(|k| format!("{:5.1}%", 100.0 * *k as f64 / n as f64)).collect::<Vec<_>>().join(" "));
                        }
                        // the colour under the transparent pixels (a < 32)
                        let (mut cnt, mut s) = (0usize, [0u64; 3]);
                        for px in rgba.chunks(4) { if px[3] < 32 { cnt += 1; for c in 0..3 { s[c] += px[c] as u64; } } }
                        if cnt > 0 { println!("  under alpha<32 ({cnt} px): mean RGB ({}, {}, {})", s[0] / cnt as u64, s[1] / cnt as u64, s[2] / cnt as u64); }
                        // the colour of the OPAQUE pixels (a >= 128): what an alpha-tested card shows
                        let (mut cnt, mut s) = (0usize, [0u64; 3]);
                        for px in rgba.chunks(4) { if px[3] >= 128 { cnt += 1; for c in 0..3 { s[c] += px[c] as u64; } } }
                        if cnt > 0 { let (r, g, b) = (s[0] / cnt as u64, s[1] / cnt as u64, s[2] / cnt as u64); println!("  opaque alpha>=128 ({cnt} px, {:.1} %): mean RGB ({r}, {g}, {b}) luma {:.0}", 100.0 * cnt as f64 / n as f64, 0.299 * r as f64 + 0.587 * g as f64 + 0.114 * b as f64); }
                    }
                    Err(e) => println!("{p}: {fourcc} {:?}: {e}", dims),
                }
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
        // `mapgeom embedded MAP --out DIR`: the map's embedded files (custom
        // items under Items\…) written out, one per file, for `dump`.
        "embedded" => {
            let p = a.rest.get(1).cloned().unwrap_or_else(|| die("embedded MAP --out DIR".into()));
            let out = flag(&a.rest, "--out").unwrap_or_else(|| die("--out DIR".into()));
            let m = tmmaps::map::MapFile::load(std::path::Path::new(&p));
            let files = mapgeom::embedded::files(&m).unwrap_or_else(die);
            for (name, bytes) in &files {
                let path = std::path::Path::new(&out).join(name.replace('\\', "/"));
                std::fs::create_dir_all(path.parent().unwrap()).unwrap();
                std::fs::write(&path, bytes).unwrap_or_else(|e| die(e.to_string()));
                println!("{}\t{} bytes", path.display(), bytes.len());
            }
            eprintln!("{} files", files.len());
        }
        // triggers MAP: every gameplay-gate trigger volume (NPlugTrigger_SGateSpecial
        // in an embedded item's prefab) in WORLD space — per placement the effect
        // (gameplay id), the world AABB of the trigger mesh, its centre and the
        // rotated main direction. Answers "did the author pass this booster?".
        "triggers" => {
            let p = a.rest.get(1).cloned().unwrap_or_else(|| die("triggers MAP".into()));
            let m = tmmaps::map::MapFile::load(std::path::Path::new(&p));
            let files = mapgeom::embedded::files(&m).unwrap_or_else(die);
            // model name -> [(gameplay id, entity iso, local vertices, main dir)]
            let mut trig: std::collections::HashMap<String, Vec<(u32, [f32; 12], Vec<[f32; 3]>, Option<[f32; 3]>)>> = Default::default();
            for (name, bytes) in &files {
                if !name.ends_with(".Item.Gbx") { continue; }
                let f = match mapgeom::static_item::parse_file(bytes) { Ok(f) => f, Err(_) => continue };
                let Some(pf) = f.item.prefab() else { continue };
                for e in &pf.ents {
                    if let Some(mapgeom::static_item::Node::GateSpecial(g)) = e.model.inline.as_deref() {
                        if let Some(mapgeom::static_item::Node::Surface(sf)) = g.shape.inline.as_deref() {
                            if let mapgeom::static_item::surface::Surf::Mesh { vertices, .. } = &sf.surf {
                                let gp = sf.material_ids.first().map(|x| (*x as u32) >> 8).unwrap_or(0);
                                let base = name.rsplit(['/', '\\']).next().unwrap_or(name).to_string();
                                trig.entry(base).or_default().push((gp, mapgeom::static_item::prefab::CPlugPrefab::entity_iso(e), vertices.clone(), sf.gameplay_main_dir));
                            }
                        }
                    }
                }
            }
            println!("map {}: {} trigger-bearing item models", p, trig.len());
            println!("model\tgameplay\tplacement(x,y,z)\tyaw\tworld_aabb_min\tworld_aabb_max\tcentre\tmain_dir_world");
            for it in &m.items {
                let base = it.model.rsplit(['/', '\\']).next().unwrap_or(&it.model);
                let Some(list) = trig.get(base) else { continue };
                let xf = mapgeom::place::anchored(it.pos, [it.yaw, it.pitch, it.roll], it.pivot, it.scale);
                for (gp, iso, verts, dir) in list {
                    let full = mapgeom::geom::compose(&xf, iso);
                    let mut lo = [f32::MAX; 3]; let mut hi = [f32::MIN; 3];
                    for v in verts { let w = mapgeom::geom::apply(&full, *v); for k in 0..3 { lo[k] = lo[k].min(w[k]); hi[k] = hi[k].max(w[k]); } }
                    let d = dir.map(|d| { let o = mapgeom::geom::apply(&full, [0.0; 3]); let e = mapgeom::geom::apply(&full, d); [e[0]-o[0], e[1]-o[1], e[2]-o[2]] });
                    // the ids as the pack's `Modifier\<Kind>\Collision.Material.Gbx` files carry them
                    // (0x09079017, read 2026-09-10): NoEngine 4, NoSteering 6, Reset 8, SlowMotion 9,
                    // Fragile 13, NoBrake 16, Cruise 17, Boost 18, Boost2 19; Turbo 1 / Turbo2 2 are the
                    // prefab slabs' own bytes
                    let gname = match gp { 1 => "Turbo", 2 => "Turbo2", 3 => "TurboRoulette", 4 => "NoEngine(FreeWheeling)", 5 => "NoGrip", 6 => "NoSteering", 7 => "ForceAcceleration", 8 => "Reset", 9 => "SlowMotion", 10 => "Bumper", 11 => "Bumper2", 12 => "ReactorBoost", 13 => "Fragile", 14 => "ReactorBoost2", 15 => "Bouncy", 16 => "NoBrake", 17 => "Cruise", 18 => "Boost(ReactorBoost_Oriented)", 19 => "Boost2(ReactorBoost2_Oriented)", _ => "?" };
                    println!("{}\t{} ({})\t({:.2}, {:.2}, {:.2})\t{:.3}\t({:.2}, {:.2}, {:.2})\t({:.2}, {:.2}, {:.2})\t({:.2}, {:.2}, {:.2})\t{}",
                        base, gp, gname, it.pos[0], it.pos[1], it.pos[2], it.yaw, lo[0], lo[1], lo[2], hi[0], hi[1], hi[2],
                        (lo[0]+hi[0])/2.0, (lo[1]+hi[1])/2.0, (lo[2]+hi[2])/2.0,
                        d.map(|d| format!("({:.2}, {:.2}, {:.2})", d[0], d[1], d[2])).unwrap_or_else(|| "-".into()));
                }
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
        "ghostpath" => {
            // the author's validation ghost of a SOURCE map as a path: t (s), x, y, z in
            // the source frame, one row per sample (--every MS thins it) — the route order
            // of a map's waypoints, for framing a shot along the direction of travel
            let src = a.rest.get(1).cloned().unwrap_or_default();
            let every: i32 = flag(&a.rest, "--every").and_then(|s| s.parse().ok()).unwrap_or(0);
            let d = gbx::record::decode_ghost(&src).unwrap_or_else(|e| die(format!("{src}: no validation ghost ({e})")));
            println!("t\tx\ty\tz");
            let mut next = i32::MIN;
            for s in &d.samples {
                if every > 0 && s.time_ms < next {
                    continue;
                }
                next = s.time_ms + every;
                println!("{:.3}\t{:.1}\t{:.1}\t{:.1}", s.time_ms as f64 / 1000.0, s.x, s.y, s.z);
            }
        }
        "waterline" => {
            // The author's validation ghost against the SOURCE's WATER surfaces
            // (physics 13): for every sample, the highest Water triangle in its
            // column; the car RESTS 0.90 m below the drawn water plane
            // (`probe::WATER_DRAFT`, measured on Cobalt Cove: 41.100 under 42.000,
            // four of four), so a sample is ON the water within ±0.35 m of
            // plane − 0.90, UNDER when more than that below, else clear. Decides whether the engine rides Water-physics
            // surfaces (the 15 pool question, 2026-09-10): an author driving ON
            // water for a stretch = ridden; an author passing UNDER water in a
            // basin = not a wall.
            //   waterline <MAP.Map.Gbx> [--every MS] [--rows]
            //     [--ghost G.Ghost.Gbx|SRC.Map.Gbx] [--anchor sx,sy,sz:tx,ty,tz --scale 0.5] [--report report.tsv]
            // On a SOURCE the water is the block models' (the assembler has no clip
            // fillers, so the generated water plates are missed); on a TINY every
            // plate is an ITEM, so run it there with the source ghost mapped
            // through the anchor (--ghost SRC --anchor) or a tiny-frame lap
            // (--ghost LAP.Ghost.Gbx); --report names the item's source block.
            let mut store = open(&a);
            let p = a.rest.get(1).cloned().unwrap_or_default();
            let every: i32 = flag(&a.rest, "--every").and_then(|s| s.parse().ok()).unwrap_or(100);
            let rows = a.rest.iter().any(|s| s == "--rows");
            let m = tmmaps::map::MapFile::load(std::path::Path::new(&p));
            let gpath = flag(&a.rest, "--ghost").unwrap_or_else(|| p.clone());
            let mut d = gbx::record::decode_ghost(&gpath).unwrap_or_else(|e| die(format!("{gpath}: no ghost ({e})")));
            if let Some(anchor) = flag(&a.rest, "--anchor") {
                let scale: f32 = flag(&a.rest, "--scale").and_then(|s| s.parse().ok()).unwrap_or(0.5);
                let (s, t) = anchor.split_once(':').unwrap_or_else(|| die("--anchor sx,sy,sz:tx,ty,tz".into()));
                let v = |q: &str| -> [f32; 3] {
                    let f: Vec<f32> = q.split(',').filter_map(|x| x.trim().parse().ok()).collect();
                    if f.len() != 3 {
                        die::<()>("--anchor needs two x,y,z triples".into());
                    }
                    [f[0], f[1], f[2]]
                };
                let (sa, ta) = (v(s), v(t));
                for smp in d.samples.iter_mut() {
                    smp.x = ta[0] + (smp.x - sa[0]) * scale;
                    smp.y = ta[1] + (smp.y - sa[1]) * scale;
                    smp.z = ta[2] + (smp.z - sa[2]) * scale;
                }
            }
            let names: std::collections::BTreeMap<String, String> = flag(&a.rest, "--report")
                .and_then(|r| std::fs::read_to_string(r).ok())
                .map(|text| text.lines().filter_map(|l| { let c: Vec<&str> = l.split('\t').collect(); (c.len() > 4 && c[0] == "block").then(|| (format!("{}.Item.Gbx", c[1]), c[4].split(' ').next().unwrap_or("").to_string())) }).collect())
                .unwrap_or_default();
            let mut asm = mapgeom::assemble::Assembler::new(&mut store);
            asm.with_embedded(&m).ok();
            // every Water triangle of the map, world frame
            let mut water: Vec<([f32; 3], [f32; 3], [f32; 3], String)> = Vec::new();
            let mut push_model = |xf: &mapgeom::geom::Xform, lm: &mapgeom::assemble::LocalModel, who: &str, water: &mut Vec<([f32; 3], [f32; 3], [f32; 3], String)>| {
                for (mat, g) in &lm.scene.groups {
                    // a ship14/15 tiny re-flags its water plates 28 — they sit in the
                    // "NotCollidable" group; count them as water when the model is a
                    // water block's (05's RoadWater / WaterGrassRampRoad decks have no
                    // Water-material visual, so the name is the only tell — 2026-09-10,
                    // the 18.298 lap read 3 lid samples where the car rode 24)
                    let water_named = who.to_ascii_lowercase().contains("water");
                    if !(mat == "Water" || (mat == "NotCollidable" && water_named)) {
                        continue;
                    }
                    for t in &g.tris {
                        water.push((mapgeom::geom::apply(xf, g.verts[t[0] as usize]), mapgeom::geom::apply(xf, g.verts[t[1] as usize]), mapgeom::geom::apply(xf, g.verts[t[2] as usize]), who.to_string()));
                    }
                }
            };
            for b in m.blocks.iter().chain(m.baked.iter()) {
                let free = b.flags & tmmaps::map::FREE_BLOCK_FLAG != 0;
                let Some(lm) = asm.block_model(&b.name) else { continue };
                let size = lm.size;
                let xf = if free {
                    match (b.free_pos, b.free_rot) {
                        (Some(p), Some(r)) => mapgeom::place::free(p, r),
                        (Some(p), None) => mapgeom::place::free(p, [0.0; 3]),
                        _ => continue,
                    }
                } else {
                    mapgeom::place::grid_block(b.coords(), b.dir, size, 0.0)
                };
                let lm = lm.clone();
                push_model(&xf, &lm, &format!("block {} {}", b.index, b.name), &mut water);
            }
            // --plates OUT.tsv: every water-plate PLACEMENT as a row (item index, model,
            // source block, position, yaw, plane y, x/z footprint) — the machine-readable
            // water table for the player project's trace census (2026-09-10)
            let plates_out = flag(&a.rest, "--plates");
            let mut plate_rows: Vec<String> = Vec::new();
            for it in &m.items {
                let Some(lm) = asm.item_model(&it.model) else { continue };
                let lm = lm.clone();
                let xf = mapgeom::place::anchored(it.pos, [it.yaw, it.pitch, it.roll], it.pivot, it.scale);
                let src = names.get(&it.model).map(|s| format!(" = {s}")).unwrap_or_default();
                let before = water.len();
                push_model(&xf, &lm, &format!("item i{} {}{src}", it.index, it.model), &mut water);
                if plates_out.is_some() && water.len() > before {
                    let (mut lo, mut hi) = ([f32::MAX; 3], [f32::MIN; 3]);
                    for (p0, p1, p2, _) in &water[before..] {
                        for p in [p0, p1, p2] {
                            for k in 0..3 {
                                lo[k] = lo[k].min(p[k]);
                                hi[k] = hi[k].max(p[k]);
                            }
                        }
                    }
                    plate_rows.push(format!("{}\t{}\t{}\t{:.2}\t{:.2}\t{:.2}\t{:.4}\t{:.2}\t{:.2}\t{:.2}\t{:.2}\t{:.2}\t{}", it.index, it.model, names.get(&it.model).cloned().unwrap_or_default(), it.pos[0], it.pos[1], it.pos[2], it.yaw, hi[1], lo[0], hi[0], lo[2], hi[2], water.len() - before));
                }
            }
            if let Some(out) = plates_out {
                let mut text = String::from("item_index\tmodel\tsource_block\tx\ty\tz\tyaw\tplane_y\txmin\txmax\tzmin\tzmax\twater_tris\n");
                for r in &plate_rows { text.push_str(r); text.push('\n'); }
                std::fs::write(&out, text).unwrap_or_else(|e| die(format!("{out}: {e}")));
                eprintln!("{}: {} water-plate placements -> {out}", p, plate_rows.len());
            }
            // (the per-item loop above replaced the plain one)
            if false {
            for it in &m.items {
                let Some(lm) = asm.item_model(&it.model) else { continue };
                let lm = lm.clone();
                let xf = mapgeom::place::anchored(it.pos, [it.yaw, it.pitch, it.roll], it.pivot, it.scale);
                let src = names.get(&it.model).map(|s| format!(" = {s}")).unwrap_or_default();
                push_model(&xf, &lm, &format!("item i{} {}{src}", it.index, it.model), &mut water);
            }
            }
            let (mut on, mut under, mut clear, mut n, mut lid) = (0usize, 0usize, 0usize, 0usize, 0usize);
            let mut on_stretch: Vec<(f64, f64, f32, String)> = Vec::new();
            let mut under_stretch: Vec<(f64, f64, f32, String)> = Vec::new();
            let mut lid_stretch: Vec<(f64, f64, f32, String)> = Vec::new();
            let mut cur: Option<(char, f64, f64, f32, String)> = None;
            let mut next = i32::MIN;
            if rows {
                println!("t\tx\ty\tz\twater_y\tstate\towner");
            }
            let mut flush = |cur: &mut Option<(char, f64, f64, f32, String)>, on_stretch: &mut Vec<(f64, f64, f32, String)>, under_stretch: &mut Vec<(f64, f64, f32, String)>, lid_stretch: &mut Vec<(f64, f64, f32, String)>| {
                if let Some((k, t0, t1, wy, who)) = cur.take() {
                    if t1 - t0 >= 0.3 {
                        match k { 'O' => on_stretch.push((t0, t1, wy, who)), 'L' => lid_stretch.push((t0, t1, wy, who)), _ => under_stretch.push((t0, t1, wy, who)) }
                    }
                }
            };
            for s in &d.samples {
                if every > 0 && s.time_ms < next {
                    continue;
                }
                next = s.time_ms + every;
                n += 1;
                let t = s.time_ms as f64 / 1000.0;
                // the highest water plane within 6 m of the sample, up or down
                let mut best: Option<(f32, &str)> = None;
                for (a3, b3, c3, who) in &water {
                    if let Some(y) = mapgeom::probe::height_at(*a3, *b3, *c3, s.x, s.z) {
                        if (y - s.y).abs() <= 6.0 && best.map(|(by, _)| y > by).unwrap_or(true) {
                            best = Some((y, who.as_str()));
                        }
                    }
                }
                let (state, wy, who) = match best {
                    // L = resting ON A LID: the car sits on the plane as on a solid (origin
                    // 0.0..0.6 above it) — what a 13/28 plate does to an embedded item
                    Some((wy, who)) if s.y >= wy - 0.05 && s.y <= wy + 0.6 => { lid += 1; ('L', wy, who.to_string()) }
                    Some((wy, who)) if (s.y - (wy - mapgeom::probe::WATER_DRAFT)).abs() <= 0.35 => { on += 1; ('O', wy, who.to_string()) }
                    Some((wy, who)) if s.y < wy - mapgeom::probe::WATER_DRAFT - 0.35 => { under += 1; ('U', wy, who.to_string()) }
                    _ => { clear += 1; ('-', f32::NAN, String::new()) }
                };
                if rows {
                    println!("{t:.1}\t{:.1}\t{:.2}\t{:.1}\t{wy:.2}\t{state}\t{who}", s.x, s.y, s.z);
                }
                match (&mut cur, state) {
                    (Some((k, _, t1, _, _)), st) if *k == st => *t1 = t,
                    (c, 'O') | (c, 'U') | (c, 'L') => { flush(c, &mut on_stretch, &mut under_stretch, &mut lid_stretch); *c = Some((state, t, t, wy, who)); }
                    (c, _) => flush(c, &mut on_stretch, &mut under_stretch, &mut lid_stretch),
                }
            }
            flush(&mut cur, &mut on_stretch, &mut under_stretch, &mut lid_stretch);
            println!("{p}: {n} samples ({every} ms): ON water {on} (at draft), ON A LID {lid}, UNDER water {under}, clear {clear}; {} water triangles", water.len());
            for (t0, t1, wy, who) in &lid_stretch {
                println!("  LID   {t0:.1}..{t1:.1} s  water y {wy:.2}  {who}");
            }
            for (t0, t1, wy, who) in &on_stretch {
                println!("  ON    {t0:.1}..{t1:.1} s  water y {wy:.2}  {who}");
            }
            for (t0, t1, wy, who) in &under_stretch {
                println!("  UNDER {t0:.1}..{t1:.1} s  water y {wy:.2}  {who}");
            }
        }
        "ghostclash" => {
            // The ORIGINAL's validation ghost is the author driving the
            // original: every piece of the tiny that the author's car passes
            // THROUGH (positions halved through the anchor) is geometry the
            // game does not have there — a recorded clip filler the game hides
            // (Summer 09's `OpenTechRoadFC` end cap across the first
            // checkpoint's entrance, 2026-09-08), a tree that is not solid, a
            // misplaced piece. Needs no render box: the ghost is in the source
            // file, the geometry in the tiny's items.
            //
            //   ghostclash TINY.Map.Gbx --src SRC.Map.Gbx --anchor sx,sy,sz:tx,ty,tz
            //              [--scale 0.5] [--radius R] [--report report.tsv] [--every MS]
            //
            // A hit = a triangle of an item within R (tiny metres, default 0.9:
            // the car is 2 m wide at full size, 1 m in the tiny's frame) of a
            // sample's centre (the ghost position lifted by 0.35 m, half of the
            // car's 0.7 m body height at the tiny's scale). Rows: item, alias
            // (and the source block name from --report), first/last sample
            // time, nearest distance, the item's tiny and source positions.
            let mut store = open(&a);
            let p = a.rest.get(1).cloned().unwrap_or_default();
            let src = flag(&a.rest, "--src").unwrap_or_else(|| if flag(&a.rest, "--line").is_some() { String::new() } else { die("ghostclash needs --src SRC.Map.Gbx (the file with the author's ghost) or --line PTS".into()) });
            let anchor = flag(&a.rest, "--anchor").unwrap_or_else(|| if flag(&a.rest, "--line").is_some() { "0,0,0:0,0,0".to_string() } else { die("ghostclash needs --anchor sx,sy,sz:tx,ty,tz".into()) });
            let scale: f32 = flag(&a.rest, "--scale").and_then(|s| s.parse().ok()).unwrap_or(0.5);
            let radius: f32 = flag(&a.rest, "--radius").and_then(|s| s.parse().ok()).unwrap_or(0.9);
            let every: i32 = flag(&a.rest, "--every").and_then(|s| s.parse().ok()).unwrap_or(0);
            let (sa, ta) = {
                let (s, t) = anchor.split_once(':').unwrap_or_else(|| die("--anchor sx,sy,sz:tx,ty,tz".into()));
                let v = |q: &str| -> [f32; 3] {
                    let f: Vec<f32> = q.split(',').filter_map(|x| x.trim().parse().ok()).collect();
                    if f.len() != 3 {
                        die::<()>("--anchor needs two x,y,z triples".into());
                    }
                    [f[0], f[1], f[2]]
                };
                (v(s), v(t))
            };
            let names: BTreeMap<String, String> = flag(&a.rest, "--report")
                .and_then(|r| std::fs::read_to_string(r).ok())
                .map(|text| {
                    text.lines()
                        .filter_map(|l| {
                            let f: Vec<&str> = l.split('\t').collect();
                            (f.len() >= 5 && f[0] == "block").then(|| (format!("{}.Item.Gbx", f[1]), f[4].split(' ').next().unwrap_or("").to_string()))
                        })
                        .collect()
                })
                .unwrap_or_default();
            // the car's body centre in the tiny's frame: the ghost position is
            // the ground contact, the body ~0.75 m tall at full size
            let lift = 0.75 * scale;
            let floor = 0.15 * scale;
            let mut pts: Vec<([f32; 3], [f32; 3], i32)> = Vec::new();
            let mut next = i32::MIN;
            // --line FILE: the route project's author line instead of the source's
            // validation ghost — `x,y,z` per line ALREADY in the tiny's frame
            // (their author-line.json `pts`, 100 ms apart), car upright. Summer 20's
            // source carries no ghost; theirs comes from a downloaded run.
            let line_pts: Option<Vec<[f32; 3]>> = flag(&a.rest, "--line").map(|f| {
                std::fs::read_to_string(&f)
                    .unwrap_or_else(|e| die(format!("{f}: {e}")))
                    .lines()
                    .filter_map(|l| {
                        let v: Vec<f32> = l.trim().trim_matches(|c| c == '[' || c == ']').split(',').filter_map(|t| t.trim().parse().ok()).collect();
                        (v.len() == 3).then(|| [v[0], v[1], v[2]])
                    })
                    .collect()
            });
            let d = if line_pts.is_some() { None } else { Some(gbx::record::decode_ghost(&src).unwrap_or_else(|e| die(format!("{src}: no validation ghost ({e})")))) };
            if let Some(lp) = &line_pts {
                for (i, q) in lp.iter().enumerate() {
                    let t = i as i32 * 100;
                    if every > 0 && t < next {
                        continue;
                    }
                    next = t + every;
                    pts.push(([q[0], q[1] + lift, q[2]], [0.0, 1.0, 0.0], t));
                }
            }
            for s in d.as_ref().map(|d| d.samples.as_slice()).unwrap_or(&[]) {
                if every > 0 && s.time_ms < next {
                    continue;
                }
                next = s.time_ms + every;
                // the car's own up axis (a wall-ride's car lies on its side; a
                // loop's hangs upside down): the body centre is `lift` along it
                let up = gbx::record::quat_rotate([s.qx as f64, s.qy as f64, s.qz as f64, s.qw as f64], [0.0, 1.0, 0.0]);
                let up = [up[0] as f32, up[1] as f32, up[2] as f32];
                let ground = [ta[0] + (s.x - sa[0]) * scale, ta[1] + (s.y - sa[1]) * scale, ta[2] + (s.z - sa[2]) * scale];
                pts.push(([ground[0] + up[0] * lift, ground[1] + up[1] * lift, ground[2] + up[2] * lift], up, s.time_ms));
            }
            println!("{} ghost samples ({:.3} s) from {}", pts.len(), d.as_ref().map(|d| d.end_ms as f64 / 1000.0).unwrap_or(pts.len() as f64 * 0.1), if line_pts.is_some() { flag(&a.rest, "--line").unwrap_or_default() } else { src.clone() });
            // spatial hash of the samples, cells of 4 m
            let cell = 4.0f32;
            let mut hash: std::collections::HashMap<(i32, i32, i32), Vec<usize>> = std::collections::HashMap::new();
            let key = |q: [f32; 3]| -> (i32, i32, i32) { ((q[0] / cell).floor() as i32, (q[1] / cell).floor() as i32, (q[2] / cell).floor() as i32) };
            for (i, (q, _, _)) in pts.iter().enumerate() {
                hash.entry(key(*q)).or_default().push(i);
            }
            let m = tmmaps::map::MapFile::load(std::path::Path::new(&p));
            let mut asm = mapgeom::assemble::Assembler::new(&mut store);
            asm.with_embedded(&m).ok();
            let mut rows: Vec<(f32, i32, i32, usize, String, [f32; 3])> = Vec::new();
            for it in &m.items {
                let Some(lm) = asm.item_model(&it.model) else { continue };
                let xf = mapgeom::place::anchored(it.pos, [it.yaw, it.pitch, it.roll], it.pivot, it.scale);
                let mut best: Option<(f32, i32, i32)> = None;
                for g in lm.scene.groups.values() {
                    for t in &g.tris {
                        let a3 = mapgeom::geom::apply(&xf, g.verts[t[0] as usize]);
                        let b3 = mapgeom::geom::apply(&xf, g.verts[t[1] as usize]);
                        let c3 = mapgeom::geom::apply(&xf, g.verts[t[2] as usize]);
                        let lo = [a3[0].min(b3[0]).min(c3[0]) - radius, a3[1].min(b3[1]).min(c3[1]) - radius, a3[2].min(b3[2]).min(c3[2]) - radius];
                        let hi = [a3[0].max(b3[0]).max(c3[0]) + radius, a3[1].max(b3[1]).max(c3[1]) + radius, a3[2].max(b3[2]).max(c3[2]) + radius];
                        let (k0, k1) = (key(lo), key(hi));
                        if (k1.0 - k0.0) * (k1.1 - k0.1) * (k1.2 - k0.2) > 4096 {
                            continue; // a giant triangle (a terrain sheet): not what this looks for
                        }
                        for kx in k0.0..=k1.0 {
                            for ky in k0.1..=k1.1 {
                                for kz in k0.2..=k1.2 {
                                    let Some(list) = hash.get(&(kx, ky, kz)) else { continue };
                                    for &i in list {
                                        let (q, up, tms) = pts[i];
                                        let cp = mapgeom::probe::closest_point_on_triangle(q, a3, b3, c3);
                                        // the surface the car RESTS on is not a clash: only geometry
                                        // above the ground contact (along the car's own up) counts
                                        let h = (cp[0] - q[0]) * up[0] + (cp[1] - q[1]) * up[1] + (cp[2] - q[2]) * up[2];
                                        if h < -lift + floor {
                                            continue;
                                        }
                                        let dd = ((q[0] - cp[0]).powi(2) + (q[1] - cp[1]).powi(2) + (q[2] - cp[2]).powi(2)).sqrt();
                                        if dd < radius {
                                            best = Some(match best {
                                                None => (dd, tms, tms),
                                                Some((bd, t0, t1)) => (bd.min(dd), t0.min(tms), t1.max(tms)),
                                            });
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
                if let Some((dd, t0, t1)) = best {
                    rows.push((dd, t0, t1, it.index, it.model.clone(), it.pos));
                }
            }
            rows.sort_by(|p, q| p.1.cmp(&q.1));
            println!("{} items within {radius} m of the author's path:", rows.len());
            println!("t_first\tt_last\tdist\titem\tmodel\tsource_block\ttiny_pos\tsource_pos");
            for (dd, t0, t1, idx, model, pos) in &rows {
                let sp = [sa[0] + (pos[0] - ta[0]) / scale, sa[1] + (pos[1] - ta[1]) / scale, sa[2] + (pos[2] - ta[2]) / scale];
                println!(
                    "{:.3}\t{:.3}\t{:.2}\ti{}\t{}\t{}\t{:.1},{:.1},{:.1}\t{:.0},{:.0},{:.0}",
                    *t0 as f64 / 1000.0,
                    *t1 as f64 / 1000.0,
                    dd,
                    idx,
                    model,
                    names.get(model).cloned().unwrap_or_default(),
                    pos[0], pos[1], pos[2],
                    sp[0], sp[1], sp[2]
                );
            }
        }
        "who" => {
            // `plumb` says WHAT is in the column; `who` says WHOSE it is.
            // Every item (and every authored grid/free block the packs model)
            // is placed exactly as `Assembler::map` places it, and each of its
            // triangles is tested for a surface at (x, z); the ones within
            // `--dy` of the asked y are listed with their owner. On a tiny
            // map every surface is an item's, so the row names the library
            // alias — the build report maps it back to the source block
            // (2026-09-08: the stray road pieces vjeux drove into).
            let mut store = open(&a);
            let p = a.rest.get(1).cloned().unwrap_or_default();
            let yoff: f32 = flag(&a.rest, "--yoff").and_then(|s| s.parse().ok()).unwrap_or(0.0);
            let dy: f32 = flag(&a.rest, "--dy").and_then(|s| s.parse().ok()).unwrap_or(4.0);
            let m = tmmaps::map::MapFile::load(std::path::Path::new(&p));
            let ats: Vec<[f32; 3]> = a
                .rest
                .iter()
                .enumerate()
                .filter(|(_, s)| *s == "--at")
                .filter_map(|(i, _)| a.rest.get(i + 1))
                .filter_map(|s| {
                    let v: Vec<f32> = s.split(',').filter_map(|t| t.trim().parse().ok()).collect();
                    (v.len() == 3).then(|| [v[0], v[1], v[2]])
                })
                .collect();
            if ats.is_empty() {
                die::<()>("who needs --at X,Y,Z, repeatable".into());
            }
            let mut asm = mapgeom::assemble::Assembler::new(&mut store);
            asm.with_embedded(&m).ok();
            // (owner, placement position, yaw) -> transformed triangles are
            // tested lazily per column; models are small, the map has a few
            // thousand placements, so this is milliseconds per column.
            let mut owners: Vec<(String, [f32; 3], f32, mapgeom::geom::Xform, String)> = Vec::new();
            for b in &m.blocks {
                let free = b.flags & tmmaps::map::FREE_BLOCK_FLAG != 0;
                let Some(lm) = asm.block_model(&b.name) else { continue };
                let size = lm.size;
                let xf = if free {
                    match (b.free_pos, b.free_rot) {
                        (Some(p), Some(r)) => mapgeom::place::free(p, r),
                        (Some(p), None) => mapgeom::place::free(p, [0.0; 3]),
                        _ => continue,
                    }
                } else {
                    mapgeom::place::grid_block(b.coords(), b.dir, size, yoff)
                };
                let c = b.coords();
                owners.push((format!("block {} {} {:?} dir {}", b.index, b.name, c, b.dir), [xf[9], xf[10], xf[11]], 0.0, xf, b.name.clone()));
            }
            for it in &m.items {
                if asm.item_model(&it.model).is_none() {
                    continue;
                }
                let xf = mapgeom::place::anchored(it.pos, [it.yaw, it.pitch, it.roll], it.pivot, it.scale);
                owners.push((format!("item i{} {}", it.index, it.model), it.pos, it.yaw, xf, it.model.clone()));
            }
            for at in &ats {
                let mut hits: Vec<(f32, String, String)> = Vec::new();
                for (who, pos, yaw, xf, model) in &owners {
                    let lm = if who.starts_with("block ") { asm.block_model(model) } else { asm.item_model(model) };
                    let Some(lm) = lm else { continue };
                    for (mat, g) in &lm.scene.groups {
                        for t in &g.tris {
                            let a3 = mapgeom::geom::apply(xf, g.verts[t[0] as usize]);
                            let b3 = mapgeom::geom::apply(xf, g.verts[t[1] as usize]);
                            let c3 = mapgeom::geom::apply(xf, g.verts[t[2] as usize]);
                            if let Some(y) = mapgeom::probe::height_at(a3, b3, c3, at[0], at[2]) {
                                if (y - at[1]).abs() <= dy {
                                    hits.push((y, mat.clone(), format!("{who} at ({:.2}, {:.2}, {:.2}) yaw {:.4}", pos[0], pos[1], pos[2], yaw)));
                                }
                            }
                        }
                    }
                }
                hits.sort_by(|p, q| q.0.partial_cmp(&p.0).unwrap_or(std::cmp::Ordering::Equal));
                hits.dedup_by(|p, q| (p.0 - q.0).abs() < 0.002 && p.1 == q.1 && p.2 == q.2);
                println!("surfaces within {dy} m of y {} at x {} z {}: {}", at[1], at[0], at[2], hits.len());
                for (y, mat, who) in &hits {
                    println!("  y {:>9.3}  {:<14} {}", y, mat, who);
                }
            }
        }
        "coplanar-sinks" => {
            // Which FREE baked records (the clips of free-placed blocks) have a
            // TOP face lying exactly in an authored placement's top face — two
            // items z-fighting where the game draws the deck over the clip.
            // Norway 23, checkpoint 8 (2026-09-09): the two DecoWallBaseVFC caps
            // of the sideways free pillars 1112/1113 lie at 66.000 in the
            // PlatformTechCheckpoint deck's 66.000 over a 13×8 m patch; the
            // original shows a clean deck (the caps appear only with the
            // checkpoint block moved away: frame sp23n), the tiny flickers.
            // Emits `yb@INDEX<TAB>0.01` rows for `tmmaps tiny`, which lowers
            // those records a centimetre so the deck wins like it does in the
            // original.
            //   mapgeom … coplanar-sinks TINY.Map.Gbx --source SRC.Map.Gbx --mapping placements.tsv
            //           --anchor sx,sy,sz:tx,ty,tz [--scale 0.5] [--out sinks.tsv] [--sink 0.01] [--tol 0.005]
            let mut store = open(&a);
            let p = a.rest.get(1).cloned().unwrap_or_default();
            let src = flag(&a.rest, "--source").unwrap_or_else(|| die("coplanar-sinks needs --source SRC.Map.Gbx".into()));
            let mapping = flag(&a.rest, "--mapping").unwrap_or_else(|| die("coplanar-sinks needs --mapping placements.tsv".into()));
            let anchor = flag(&a.rest, "--anchor").unwrap_or_else(|| die("coplanar-sinks needs --anchor sx,sy,sz:tx,ty,tz".into()));
            let scale: f32 = flag(&a.rest, "--scale").and_then(|s| s.parse().ok()).unwrap_or(0.5);
            let sink: f32 = flag(&a.rest, "--sink").and_then(|s| s.parse().ok()).unwrap_or(0.01);
            let tol: f32 = flag(&a.rest, "--tol").and_then(|s| s.parse().ok()).unwrap_or(0.005);
            let out = flag(&a.rest, "--out");
            let nums = |s: &str| -> Vec<f32> { s.split(',').filter_map(|t| t.trim().parse().ok()).collect() };
            let (sa, ta) = anchor.split_once(':').unwrap_or_else(|| die("--anchor sx,sy,sz:tx,ty,tz".into()));
            let (sa, ta) = (nums(sa), nums(ta));
            if sa.len() != 3 || ta.len() != 3 {
                die::<()>("--anchor sx,sy,sz:tx,ty,tz".into());
            }
            let source = tmmaps::map::MapFile::load(std::path::Path::new(&src));
            let tiny = tmmaps::map::MapFile::load(std::path::Path::new(&p));
            let maps = tmmaps::tiny::mapping::read_mapping(std::path::Path::new(&mapping));
            let mut asm = mapgeom::assemble::Assembler::new(&mut store);
            asm.with_embedded(&tiny).ok();
            // the expected tiny position of every FREE baked record with a row
            let mut want: Vec<(usize, String, [f32; 3])> = Vec::new();
            // the FREE BLOCKS' own placements are not decks either: a clip's
            // face lying in its parent's face is the parent's business
            let mut free_blocks: Vec<(String, [f32; 3])> = Vec::new();
            for b in source.blocks.iter().filter(|b| b.flags & tmmaps::map::FREE_BLOCK_FLAG != 0) {
                let (Some(pos), Some(m)) = (b.free_pos, maps.by_index.get(&b.index).or_else(|| maps.by_name.get(&b.name))) else { continue };
                if m.model == "-" {
                    continue;
                }
                free_blocks.push((m.model.clone(), [ta[0] + (pos[0] - sa[0]) * scale, ta[1] + (pos[1] - sa[1]) * scale, ta[2] + (pos[2] - sa[2]) * scale]));
            }
            for b in source.baked.iter().filter(|b| b.flags & tmmaps::map::FREE_BLOCK_FLAG != 0) {
                let (Some(pos), Some(m)) = (b.free_pos, maps.baked_by_index.get(&b.index)) else { continue };
                if m.model == "-" {
                    continue;
                }
                let t = [ta[0] + (pos[0] - sa[0]) * scale, ta[1] + (pos[1] - sa[1]) * scale, ta[2] + (pos[2] - sa[2]) * scale];
                want.push((b.index, m.model.clone(), t));
            }
            // tiny item index -> the free record it stands for
            let mut record_of: std::collections::HashMap<usize, usize> = std::collections::HashMap::new();
            let mut is_free_block: std::collections::HashSet<usize> = std::collections::HashSet::new();
            for it in &tiny.items {
                for (idx, model, t) in &want {
                    if it.model == *model && (it.pos[0] - t[0]).abs() < 0.02 && (it.pos[1] - t[1]).abs() < 0.02 && (it.pos[2] - t[2]).abs() < 0.02 {
                        record_of.insert(it.index, *idx);
                        break;
                    }
                }
                for (model, t) in &free_blocks {
                    if it.model == *model && (it.pos[0] - t[0]).abs() < 0.02 && (it.pos[1] - t[1]).abs() < 0.02 && (it.pos[2] - t[2]).abs() < 0.02 {
                        is_free_block.insert(it.index);
                        break;
                    }
                }
            }
            // top faces (normal +y, within 2.5° of vertical) of every placement:
            // (y to the mm, xz bbox) — a face is the union bbox of the placement's
            // triangles in that plane
            struct Face {
                item: usize,
                y: f32,
                x0: f32,
                z0: f32,
                x1: f32,
                z1: f32,
            }
            let mut faces_free: Vec<Face> = Vec::new();
            let mut faces_other: std::collections::HashMap<i32, Vec<Face>> = std::collections::HashMap::new();
            for it in &tiny.items {
                let Some(lm) = asm.item_model(&it.model) else { continue };
                let xf = mapgeom::place::anchored(it.pos, [it.yaw, it.pitch, it.roll], it.pivot, it.scale);
                let mut planes: std::collections::BTreeMap<i32, [f32; 4]> = std::collections::BTreeMap::new();
                for (_mat, g) in &lm.scene.groups {
                    for t in &g.tris {
                        let a3 = mapgeom::geom::apply(&xf, g.verts[t[0] as usize]);
                        let b3 = mapgeom::geom::apply(&xf, g.verts[t[1] as usize]);
                        let c3 = mapgeom::geom::apply(&xf, g.verts[t[2] as usize]);
                        let u = [b3[0] - a3[0], b3[1] - a3[1], b3[2] - a3[2]];
                        let v = [c3[0] - a3[0], c3[1] - a3[1], c3[2] - a3[2]];
                        let n = [u[1] * v[2] - u[2] * v[1], u[2] * v[0] - u[0] * v[2], u[0] * v[1] - u[1] * v[0]];
                        let len = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt();
                        if len < 1e-6 || n[1] / len < 0.999 {
                            continue;
                        }
                        let y = (a3[1] + b3[1] + c3[1]) / 3.0;
                        let key = (y * 1000.0).round() as i32;
                        let e = planes.entry(key).or_insert([f32::MAX, f32::MAX, f32::MIN, f32::MIN]);
                        for q in [a3, b3, c3] {
                            e[0] = e[0].min(q[0]);
                            e[1] = e[1].min(q[2]);
                            e[2] = e[2].max(q[0]);
                            e[3] = e[3].max(q[2]);
                        }
                    }
                }
                for (key, bb) in planes {
                    if (bb[2] - bb[0]) * (bb[3] - bb[1]) < 0.25 {
                        continue;
                    }
                    let f = Face { item: it.index, y: key as f32 / 1000.0, x0: bb[0], z0: bb[1], x1: bb[2], z1: bb[3] };
                    if record_of.contains_key(&it.index) {
                        faces_free.push(f);
                    } else if !is_free_block.contains(&it.index) {
                        faces_other.entry(key).or_default().push(f);
                    }
                }
            }
            let tol_mm = (tol * 1000.0).round() as i32;
            let mut rows: std::collections::BTreeMap<usize, (usize, f32, String)> = std::collections::BTreeMap::new();
            for f in &faces_free {
                let key = (f.y * 1000.0).round() as i32;
                for k in (key - tol_mm)..=(key + tol_mm) {
                    let Some(cands) = faces_other.get(&k) else { continue };
                    for o in cands {
                        let ix = f.x1.min(o.x1) - f.x0.max(o.x0);
                        let iz = f.z1.min(o.z1) - f.z0.max(o.z0);
                        // a real patch, not an edge touch: both ways ≥ 0.5 m, ≥ 1 m²,
                        // and the partner is a DECK (a face of ≥ 20 m²: the
                        // evidence is a checkpoint deck over pillar caps, not the
                        // 0.7 m-wide bar tops of a structure lattice meeting)
                        let area_o = (o.x1 - o.x0) * (o.z1 - o.z0);
                        if ix > 0.5 && iz > 0.5 && ix * iz > 1.0 && area_o >= 20.0 {
                            let rec = record_of[&f.item];
                            let other = tiny.items.iter().find(|i| i.index == o.item).map(|i| format!("i{} {} at ({:.1}, {:.2}, {:.1})", i.index, i.model, i.pos[0], i.pos[1], i.pos[2])).unwrap_or_default();
                            rows.entry(rec).or_insert((f.item, f.y, format!("{:.1}×{:.1} m with {other}", ix, iz)));
                        }
                    }
                }
            }
            println!("{} free baked records with a top face in an authored placement's top face (tol {tol} m):", rows.len());
            let mut text = String::new();
            for (rec, (item, y, with)) in &rows {
                let name = source.baked.iter().find(|b| b.index == *rec).map(|b| b.name.clone()).unwrap_or_default();
                println!("  b{rec} {name} (tiny item i{item}) top y {y:.3}: coplanar over {with} -> sunk {sink} m");
                text.push_str(&format!("yb@{rec}\t{sink}\n"));
            }
            if let Some(out) = out {
                std::fs::write(&out, text).unwrap_or_else(|e| die(format!("{out}: {e}")));
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
        Node::Solid2(s) => {
            let mut d = format!(
                "CPlugSolid2Model, {} geoms, {} visuals, materials [{}]",
                s.geoms.len(),
                s.visuals.len(),
                s.material_names.join(" ")
            );
            // The detail levels: which geoms (visual node, material slot)
            // each lod-mask bit draws, and the switch distances.
            if !s.lod_max_dist.is_empty() || s.geoms.iter().any(|g| g.lod != 1 || g.u01 != -1 || g.u02 != 0) {
                d.push_str(&format!(
                    "\n      lod_max_dist {:?} vis_cst_type {} geoms [{}]",
                    s.lod_max_dist,
                    s.vis_cst_type,
                    s.geoms
                        .iter()
                        .map(|g| format!("v{}:m{}:lod{:x}{}{}", s.visuals.get(g.visual as usize).copied().unwrap_or(-1), g.material, g.lod, if g.u01 != -1 { format!(":u01={}", g.u01) } else { String::new() }, if g.u02 != 0 { format!(":u02={}", g.u02) } else { String::new() }))
                        .collect::<Vec<_>>()
                        .join(" ")
                ));
            }
            for (name, node, iso) in &s.lights {
                d.push_str(&format!(
                    "\n      light {name:?} node {node} at [{:.3}, {:.3}, {:.3}] rot [{:.2} {:.2} {:.2} | {:.2} {:.2} {:.2} | {:.2} {:.2} {:.2}]",
                    iso[9], iso[10], iso[11], iso[0], iso[1], iso[2], iso[3], iso[4], iso[5], iso[6], iso[7], iso[8]
                ));
            }
            if !s.light_user_models.is_empty() || !s.light_insts.is_empty() {
                d.push_str(&format!("\n      light user models {:?} insts {:?}", s.light_user_models, s.light_insts));
            }
            d
        }
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
                    0x090F9000 => "CPlugLightUserModel",
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
