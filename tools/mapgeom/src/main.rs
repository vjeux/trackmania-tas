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
  who-refs <substring>          every entry whose reference table names a file matching <substring>
  raw <path> --out F            the decoded bytes of any pack entry (dds, tga, xml)
  terrain-material LINK…        a terrain material's layer names, texture arrays (slices in GPU
                                order), shader ids and g_WorldPosToTc* buffers (mapgeom::terrain)
  collection-water --collection C   the collection's water type (top, floor, FogMaxDepth, fog LUT)
  dump <path> [--body F]        walk a file's node graph and summarise it;
                                --body writes the decompressed body out
  model <path> --out F          a single file's geometry, as .glb or .obj
  scene3d <path> --out F.obj    a decoration Scene3d (<Coll>\\GameCtnDecoration\\Scene3d\\
                                Base64x64.Scene3d.Gbx): its island / sea /
                                shadow-caster solids, grouped by material name
  constraint <path>             a .KinematicConstraint.Gbx, every anim sub-function (ease, reverse, ms)
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
                                every authored block with its picked variant,
                                cells, prefabs and what each side faces
  sttf <file.Map.Gbx> --out OUT [--cp plain|remove] [--dry-run] [--report TSV]
                                Start-To-The-Finish: every checkpoint gone — a
                                checkpoint BLOCK becomes its plain twin (chosen
                                by geometry: RoadTechCheckpoint → RoadTechStraight,
                                …CheckpointTiltLeft → …TiltStraight turned) or,
                                with --cp remove / no twin, is deleted; checkpoint
                                ITEMS are deleted; the validation ghost dropped
  center-finish <file.Map.Gbx> --out OUT [--center auto|bbox|X,Z]
      [--occupied overlap|raise|fail] [--include-startfinish] [--no-rename]
      [--no-reuid] [--lightmap strip|keep|keep-renumber] [--dry-run] [--report TSV]
                                every Finish (block info / item model type 1)
                                and the baked clips it owns moved by one whole-
                                cell offset onto the map centre ((size−1)/2:
                                23.5 on 48, 31.5 on 64), y kept; new uid SttC…,
                                name '<name> Straight to the Center'
  podium-reverse <file.Map.Gbx>|DIR --out-dir DIR [--start-item GateStartCenter8m] [--rear M]
      [--gap M] [--podium N | --podium-map 09=1,18=0] [--uid-prefix PdRv]
      [--name-suffix ' Podium Reverse'] [--times none|keep|A,G,S,B] [--unlock]
      [--lightmap keep-renumber|keep] [--out-name mapname] [--only ..] [--report R.tsv] [--dry-run]
                                Hugo's Podium Reverse: checkpoints + finishes -> plain twins
                                (items removed), the start -> its finish twin, a poleless
                                start gate in front of the Podium item (the car's rear
                                against its face: --rear = the car's origin-to-bumper m)
  sttc <file.Map.Gbx>|DIR --out-dir DIR [--only 01-,05-] [sttf + center-finish flags]
      [--no-sttf] [--out-name mapname] [--rule v1|v2|v3] [--strip-name-suffix S]
      [--uid-table results.tsv] [--originals DIR] [--unlock] [--item-offset exact|rounded|auto]
                                both steps per map: DIR/sttf/<stem>.sttf.Map.Gbx
                                then DIR/<stem>-Straight-to-the-Center.Map.Gbx,
                                one TSV row per record touched, round-trip verified;
                                --no-sttf = the input is already STTF'd (center only);
                                --rule v2 (default) = bbox centre, ties DOWN, items /
                                free blocks by the exact offset; identity overrides:
                                the name's base = input name minus the suffix, uid
                                from the table by the new name, times from the
                                original of that base name, --unlock drops the password
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

fn crate_ground_flag() -> u32 {
    mapgeom::blockmap::FLAG_GROUND
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

/// Hugo's Moth (2026-10-22): the light-source kinds of the census TSV (`LIGHTS.tsv`: group, kind, name, …,
/// local_x, local_y, local_z …) whose group is wanted — `groups` names (default: poles/lamps, light blocks,
/// show-rig = the ShowLights@/Show@ rows of "structural", ground screens = screen ITEMS ≤ 3 m) — minus the
/// `exclude` name prefixes. EMISSIVE rows give the emitting part's bbox, LIGHT rows a half-metre box about the
/// node; a model's rows merge into one box; `Model` and `Model@v0` are one kind. Podium/Stand/Canopy/Tunnel
/// pieces are never sources (structural pieces that happen to carry a screen or a lamp).
fn light_sources_from_tsv(tsv: &str, groups: Option<String>, exclude: Option<String>) -> Vec<mapgeom::flags::LightSource> {
    let groups: Vec<String> = groups.map(|v| v.split(',').map(|s| s.trim().to_ascii_lowercase()).collect()).unwrap_or_else(|| vec!["poles/lamps".into(), "light blocks".into(), "show-rig".into(), "ground screens".into()]);
    let exclude: Vec<String> = exclude.map(|v| v.split(',').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect()).unwrap_or_default();
    let text = std::fs::read_to_string(tsv).unwrap_or_else(|e| die(format!("{tsv}: {e}")));
    // per kind: the EMISSIVE box (the lit geometry) and the LIGHT-node box, merged afterwards — a node
    // joins the box only when it sits within 1 m of the emissive geometry (LampSmallB/C carry their
    // `ItemLampSpotB` node at 6.36 m, the TALL lamp's height, while their head is at 2.8–4.4 m: a pak
    // quirk that would stretch the trigger down the pole); a kind without emissive rows is boxed by its
    // nodes (half a metre about each).
    let mut emis: std::collections::BTreeMap<String, (bool, [f32; 3], [f32; 3])> = Default::default();
    let mut nodes: std::collections::BTreeMap<String, (bool, Vec<([f32; 3], [f32; 3])>)> = Default::default();
    for l in text.lines().skip(1) {
        let f: Vec<&str> = l.split('\t').collect();
        if f.len() < 19 { continue; }
        let (group, kind, name, lk) = (f[0].to_ascii_lowercase(), f[1], f[2], f[4]);
        let h: f32 = f[10].parse().unwrap_or(99.0);
        if name.starts_with("Podium") || name.starts_with("Stand") || name.starts_with("Canopy") || name.starts_with("Tunnel") { continue; }
        if exclude.iter().any(|x| name.starts_with(x.as_str())) { continue; }
        let want = groups.iter().any(|g| match g.as_str() {
            "show-rig" => group.starts_with("structural") && (name.starts_with("ShowLights@") || name.starts_with("Show@") || name.starts_with("ShowLight4Spots") || name.starts_with("ShowLightRamp")),
            "ground screens" => group.starts_with("screens") && kind == "item" && h <= 3.0 && !name.starts_with("TechnicsScreen"),
            other => group.starts_with(other),
        });
        if !want { continue; }
        let rng = |col: &str| -> Option<(f32, f32)> { let (a, b) = col.split_once("..")?; Some((a.parse().ok()?, b.parse().ok()?)) };
        let (Some(x), Some(y), Some(z)) = (rng(f[7]), rng(f[8]), rng(f[9])) else { continue };
        let (lo, hi) = ([x.0, y.0, z.0], [x.1, y.1, z.1]);
        if lk == "LIGHT" {
            nodes.entry(name.to_string()).or_insert((kind == "block", Vec::new())).1.push(([lo[0] - 0.25, lo[1] - 0.25, lo[2] - 0.25], [hi[0] + 0.25, hi[1] + 0.25, hi[2] + 0.25]));
        } else {
            let e = emis.entry(name.to_string()).or_insert((kind == "block", lo, hi));
            for k in 0..3 { e.1[k] = e.1[k].min(lo[k]); e.2[k] = e.2[k].max(hi[k]); }
        }
    }
    let mut by_key: std::collections::BTreeMap<String, (bool, [f32; 3], [f32; 3])> = emis.clone();
    for (name, (is_block, boxes)) in nodes {
        match by_key.get_mut(&name) {
            Some(e) => {
                for (lo, hi) in boxes {
                    let near = (0..3).all(|k| hi[k] >= e.1[k] - 1.0 && lo[k] <= e.2[k] + 1.0);
                    if near {
                        for k in 0..3 { e.1[k] = e.1[k].min(lo[k]); e.2[k] = e.2[k].max(hi[k]); }
                    } else {
                        eprintln!("{name}: light node box [{:.2} {:.2} {:.2}]..[{:.2} {:.2} {:.2}] is > 1 m from the lit geometry [{:.2} {:.2} {:.2}]..[{:.2} {:.2} {:.2}]: ignored", lo[0], lo[1], lo[2], hi[0], hi[1], hi[2], e.1[0], e.1[1], e.1[2], e.2[0], e.2[1], e.2[2]);
                    }
                }
            }
            None => {
                let mut e = (is_block, [f32::INFINITY; 3], [f32::NEG_INFINITY; 3]);
                for (lo, hi) in boxes { for k in 0..3 { e.1[k] = e.1[k].min(lo[k]); e.2[k] = e.2[k].max(hi[k]); } }
                by_key.insert(name, e);
            }
        }
    }
    let keys: Vec<String> = by_key.keys().cloned().collect();
    for k in keys {
        if let Some(base) = k.strip_suffix("@v0") {
            if by_key.contains_key(base) {
                let (_, lo, hi) = by_key.remove(&k).unwrap();
                let e = by_key.get_mut(base).unwrap();
                for i in 0..3 { e.1[i] = e.1[i].min(lo[i]); e.2[i] = e.2[i].max(hi[i]); }
            }
        }
    }
    by_key.into_iter().map(|(key, (is_block, lo, hi))| { let label = key.replace("@v", "V"); mapgeom::flags::LightSource { key, is_block, lo, hi, label, boxes: Vec::new() } }).collect()
}

/// Hugo's Floor Finish (2026-10-23): the OPEN-WATER terrain tiles of a vista collection, each the seat of one
/// full-cell finish slab. Tiles: RedIsland `Water`, `WaterHill` (the lake with the shallow hill rim — open water
/// at the surface); WhiteShore `Water`; BlueBay `Sea`; GreenCoast `Lake`. NOT the shore/transition tiles
/// (WaterShore1, Beach, Sea_Land0_Land12, SeaCliff13, LakeShore: land in the cell) nor the Deco* bays. The slab:
/// x 0..32 × z 0..32 in the block frame (one cell); y from `surface − below` to `surface + above` where surface =
/// the collection's water plane relative to the tile's cell floor — the zone prefabs put their Water quad at
/// local +7.70 (RedIsland), +7.00 (WhiteShore, BlueBay), +7.20 (GreenCoast) — measured on the prefabs
/// 2026-10-23, and the water row is the one row every open-water tile sits on (RedIsland/WhiteShore cy 14,
/// BlueBay cy 5, GreenCoast cy 4), so the surface is ONE world level per map: RedIsland −0.3, WhiteShore −1.0,
/// BlueBay 7.0, GreenCoast −0.8. A Water tile at another row (Fall 06's cy-18 pond) is a raised lake: its slab
/// follows its own cell (the quad is the prefab's), which is exactly what the block frame gives.
fn floor_sources(collection: u32, below: f32, above: f32, label: &str) -> Vec<mapgeom::flags::LightSource> {
    let (tiles, quad): (&[&str], f32) = match collection {
        0x10 => (&["Water", "WaterHill"], 7.70),
        0x1d => (&["Water"], 7.00),
        0x1c => (&["Sea"], 7.00),
        0xf => (&["Lake"], 7.20),
        _ => (&[], 0.0),
    };
    tiles.iter().map(|t| mapgeom::flags::LightSource {
        key: t.to_string(),
        is_block: true,
        lo: [0.0, quad - below, 0.0],
        hi: [32.0, quad + above, 32.0],
        label: label.to_string(),
        boxes: Vec::new(),
    }).collect()
}

/// Hugo's lanterns (2026-10-23): light-source kinds whose trigger is ONE BOX PER LANTERN BODY, read from a
/// per-part census (`lights-census` with LIGHTS_PER_PART=1 LIGHTS_ALL_MATERIALS=1): the rows whose material is
/// in `materials` (the lantern shades: ThemeSnowTempleLamp = the paper lampion, LightShape = the stone-lantern
/// head, ThemeSnowTempleTransparent = its glass) are clustered by overlap (two parts whose boxes intersect
/// after a 0.1 m grow are one body); kinds restricted to `only` names (prefix match) when given. The kind's
/// `lo`/`hi` = the union; the carrier gets one box per cluster.
fn lantern_sources_from_parts(tsv: &str, materials: &[String], only: &[String]) -> Vec<mapgeom::flags::LightSource> {
    let text = std::fs::read_to_string(tsv).unwrap_or_else(|e| die(format!("{tsv}: {e}")));
    let mut parts: std::collections::BTreeMap<String, (bool, Vec<([f32; 3], [f32; 3])>)> = Default::default();
    for l in text.lines().skip(1) {
        let f: Vec<&str> = l.split('\t').collect();
        if f.len() < 9 || f[4] != "EMISSIVE" { continue; }
        let (kind, name, mat) = (f[0], f[1], f[3]);
        if !materials.iter().any(|m| m == mat) { continue; }
        if !only.is_empty() && !only.iter().any(|o| name.starts_with(o.as_str())) { continue; }
        // a lantern body is small: a part wider than 4 m is a roof pane / glass sheet, not a shade
        let span_ok = f[8].split(' ').take(3).all(|col| col.split_once("..").and_then(|(a, b)| Some(b.parse::<f32>().ok()? - a.parse::<f32>().ok()?)).map(|d| d <= 4.0).unwrap_or(false));
        if !span_ok { continue; }
        // "x0..x1 y0..y1 z0..z1"
        let mut lo = [0f32; 3];
        let mut hi = [0f32; 3];
        let mut ok = true;
        for (k, col) in f[8].split(' ').take(3).enumerate() {
            match col.split_once("..").and_then(|(a, b)| Some((a.parse::<f32>().ok()?, b.parse::<f32>().ok()?))) {
                Some((a, b)) => { lo[k] = a; hi[k] = b; }
                None => ok = false,
            }
        }
        if !ok { continue; }
        parts.entry(name.to_string()).or_insert((kind == "block", Vec::new())).1.push((lo, hi));
    }
    let mut out = Vec::new();
    for (name, (is_block, boxes)) in parts {
        // cluster by overlap
        let mut clusters: Vec<([f32; 3], [f32; 3])> = Vec::new();
        for (lo, hi) in boxes {
            let mut merged_into: Option<usize> = None;
            for (ci, c) in clusters.iter_mut().enumerate() {
                let overlaps = (0..3).all(|k| hi[k] + 0.1 >= c.0[k] && lo[k] - 0.1 <= c.1[k]);
                if overlaps {
                    for k in 0..3 { c.0[k] = c.0[k].min(lo[k]); c.1[k] = c.1[k].max(hi[k]); }
                    merged_into = Some(ci);
                    break;
                }
            }
            if merged_into.is_none() { clusters.push((lo, hi)); }
        }
        // a second pass merges clusters that grew into each other
        let mut changed = true;
        while changed {
            changed = false;
            'outer: for i in 0..clusters.len() {
                for j in (i + 1)..clusters.len() {
                    let (a, b) = (clusters[i], clusters[j]);
                    if (0..3).all(|k| b.1[k] + 0.1 >= a.0[k] && b.0[k] - 0.1 <= a.1[k]) {
                        clusters[i] = ([a.0[0].min(b.0[0]), a.0[1].min(b.0[1]), a.0[2].min(b.0[2])], [a.1[0].max(b.1[0]), a.1[1].max(b.1[1]), a.1[2].max(b.1[2])]);
                        clusters.remove(j);
                        changed = true;
                        break 'outer;
                    }
                }
            }
        }
        let (mut lo, mut hi) = ([f32::INFINITY; 3], [f32::NEG_INFINITY; 3]);
        for (a, b) in &clusters { for k in 0..3 { lo[k] = lo[k].min(a[k]); hi[k] = hi[k].max(b[k]); } }
        eprintln!("{name}: {} lantern bod{}", clusters.len(), if clusters.len() == 1 { "y" } else { "ies" });
        let label = name.replace("@v", "V");
        out.push(mapgeom::flags::LightSource { key: name, is_block, lo, hi, label, boxes: clusters });
    }
    out
}

fn main() {
    mapgeom::par::process_started();
    mapgeom::par::tune_malloc();
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
    // --indicator glass|gate: the finish indicator material (static_item::build; default gate = RaceTriggerFXFinish)
    if let Some(i) = a.rest.iter().position(|x| x == "--indicator") {
        let v = a.rest.get(i + 1).map(|s| s.as_str()).unwrap_or("decal");
        mapgeom::static_item::build::set_indicator(match v { "glass" => mapgeom::static_item::build::Indicator::Glass, "gate" => mapgeom::static_item::build::Indicator::Gate, _ => mapgeom::static_item::build::Indicator::Decal });
    }
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
                        "{}\tclass 0x{:08X}\t{} bytes\tflags 0x{:x}{}{}{}",
                        p,
                        e.class_id,
                        e.uncompressed_size,
                        e.flags,
                        if e.is_compressed() { " lz4" } else { " raw" },
                        if e.dont_use_dummy_write() { "" } else { " dummywrite" },
                        if e.compressed_size != e.uncompressed_size { format!(" (stored {} B)", e.compressed_size) } else { String::new() }
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
        "nodescan" => {
            // mapgeom nodescan FILE.gbx [--all]: a (partially) decrypted Gbx file's header refs and every plausible
            // inline-node start in its body — [u32 idx ≤ num_nodes][u32 class, low 12 bits clear][u32 chunk of that
            // class's family] — for extending parents.rs when a pak entry's fold schedule stops short
            let f = a.rest.get(1).cloned().expect("nodescan FILE");
            let data = std::fs::read(&f).expect("read");
            let g = mapgeom::container::Gbx::parse(&data).expect("gbx header");
            let body_start = data.len() - g.body.len();
            println!("{f}: class {:#x}, {} nodes, {} externals, body at {body_start:#x} ({} B present)", g.class_id, g.num_nodes, g.refs.len(), g.body.len());
            for r in &g.refs {
                println!("  ext node {:>3}  {}", r.node_index, r.name);
            }
            let b = &g.body;
            let rd = |o: usize| u32::from_le_bytes([b[o], b[o + 1], b[o + 2], b[o + 3]]);
            let mut i = 0usize;
            while i + 12 <= b.len() {
                let (idx, class, chunk) = (rd(i), rd(i + 4), rd(i + 8));
                if idx >= 1 && idx <= g.num_nodes && class & 0xFFF == 0 && (0x0100_0000..0x4000_0000).contains(&class) && chunk >> 24 == class >> 24 && chunk & 0xFFF < 0x80 {
                    let parent = mapgeom::parents::dummy_write_class(class);
                    let ctx: String = b[i.saturating_sub(16)..i].iter().map(|x| format!("{x:02x}")).collect::<Vec<_>>().join(" ");
                    println!("  node start at body+{i:#06x} (file {:#06x}): idx {idx:>3} class {class:#010x} first chunk {chunk:#010x}  parent {}   before: {ctx}", body_start + i, parent.map(|p| format!("{p:#010x}")).unwrap_or("UNKNOWN → CPlug fallback".into()));
                }
                i += 1;
            }
        }
        // who-refs <substring> : every pack entry whose external reference table names a file containing
        // <substring> (case-insensitive) — "who uses this texture / material" across the whole pack. One
        // pass over the entries' headers (the reference table is in the uncompressed header part).
        "who-refs" => {
            let mut store = open(&a);
            let needle = a.rest.get(1).cloned().unwrap_or_default().to_ascii_lowercase();
            // hashed entries (most of a pack) are stored under a name that is not a path: every entry is
            // read, its GBX header parsed, and the reference table's names resolved relative to the entry's folder
            let paths: Vec<String> = store.entries().map(|e| e.path()).collect();
            let (mut n, mut parsed, mut failed) = (0usize, 0usize, 0usize);
            for p in &paths {
                let Ok(bytes) = store.read(p) else { failed += 1; continue };
                if bytes.len() < 4 || &bytes[..3] != b"GBX" { continue; }
                let m = match mapgeom::store::Model::parse(&bytes, p) { Ok(m) => m, Err(_) => { failed += 1; continue } };
                parsed += 1;
                for (i, ext) in &m.externals {
                    if ext.to_ascii_lowercase().contains(&needle) {
                        println!("{p}  (class 0x{:08X})  node {i}  {ext}", m.class_id);
                        n += 1;
                    }
                }
            }
            println!("{n} references in {parsed} GBX files ({} entries, {failed} unreadable)", paths.len());
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
        "zone-tile-plg" => {
            // zone-tile-plg <prefab logical path>: the zone tile prefab's entity 0 (the tile mesh) as a static object — its
            // Solid2's PreLightGen (MeterByUv = u02, the uv1 bounds u04[0..4]) and every visual's uv0/uv1 range with the
            // exact f32 bits: the factors of the tile's lightmap chart extent (the layout rule, REPORT-5 §4-E)
            use mapgeom::static_item::vstream::{Elem, N_TEXCOORD0};
            let mut store = open(&a);
            let p = a.rest.get(1).cloned().expect("zone-tile-plg <prefab>");
            let model = store.load_model(&p).unwrap_or_else(die);
            let prefab = mapgeom::static_item::prefab::CPlugPrefab::from_model(&model).unwrap_or_else(die);
            for (i, e) in prefab.ents.iter().enumerate() {
                let Some(mapgeom::static_item::Node::StaticObject(so)) = e.model.inline.as_deref() else { continue };
                let Some(s2) = so.solid2() else { println!("  ent {i}: static object without an inline Solid2"); continue };
                match &s2.pre_light_gen {
                    Some(g) => println!("  ent {i}: PreLightGen u02 (MeterByUv) {} ({:#010x}) u04 (uv bounds) {:?} ({:?}) boxes {} uv_groups {}; extents ({}, {})", g.u02, g.u02.to_bits(), &g.u04[..4], g.u04[..4].iter().map(|v| format!("{:#010x}", v.to_bits())).collect::<Vec<_>>(), g.boxes.len(), g.uv_groups.len(), g.u04[2] - g.u04[0], g.u04[3] - g.u04[1]),
                    None => println!("  ent {i}: no PreLightGen"),
                }
                println!("  ent {i}: solid2 v{} {} visuals, {} shaded geoms, boxes {:?}", s2.version, s2.visuals.len(), s2.shaded_geoms.len(), s2.boxes);
                for (vi, vr) in s2.visuals.iter().enumerate() {
                    let Some(mapgeom::static_item::Node::Visual(v)) = vr.inline.as_deref() else { continue };
                    let Some(main) = v.main.as_ref() else { continue };
                    for sr in &main.vertex_streams {
                        let Some(mapgeom::static_item::Node::VertexStream(s)) = sr.inline.as_deref() else { continue };
                        for (name, sem) in [("uv0", N_TEXCOORD0), ("uv1", N_TEXCOORD0 + 1)] {
                            let Some(vals) = s.decls.iter().zip(s.elems.iter()).find(|(d, _)| d.name() == sem).and_then(|(_, e)| if let Elem::Float2(v) = e { Some(v.clone()) } else { None }) else { continue };
                            let (mut lo, mut hi) = ([f32::MAX; 2], [f32::MIN; 2]);
                            for p in &vals { for k in 0..2 { lo[k] = lo[k].min(p[k]); hi[k] = hi[k].max(p[k]); } }
                            println!("    visual {vi} {name}: {} verts, range [{}, {}]..[{}, {}] ({:#010x} {:#010x} {:#010x} {:#010x}); extents ({}, {})", vals.len(), lo[0], lo[1], hi[0], hi[1], lo[0].to_bits(), lo[1].to_bits(), hi[0].to_bits(), hi[1].to_bits(), hi[0] - lo[0], hi[1] - lo[1]);
                        }
                        if let Some(pos) = s.decls.iter().zip(s.elems.iter()).find(|(d, _)| d.name() == mapgeom::static_item::vstream::N_POSITION).and_then(|(_, e)| if let Elem::Float3(v) = e { Some(v.clone()) } else { None }) {
                            let (mut lo, mut hi) = ([f32::MAX; 3], [f32::MIN; 3]);
                            for p in &pos { for k in 0..3 { lo[k] = lo[k].min(p[k]); hi[k] = hi[k].max(p[k]); } }
                            println!("    visual {vi} positions: {} verts, AABB {:?}..{:?}", pos.len(), lo, hi);
                        }
                    }
                }
            }
        }
        "zone-slots" => {
            // zone-slots <prefab logical path>: the zone tile prefab's entities — entity 0 the tile mesh, the rest the
            // vegetation slots (params chunk 0x2F0A9000 = NPlugItemPlacement::SPlacement, printed raw + decoded tags)
            let mut store = open(&a);
            let p = a.rest.get(1).cloned().expect("zone-slots <prefab>");
            let model = store.load_model(&p).unwrap_or_else(die);
            let prefab = mapgeom::static_item::prefab::CPlugPrefab::from_model(&model).unwrap_or_else(die);
            println!("{p}: {} entities (v{})", prefab.ents.len(), prefab.version);
            let raw = a.rest.iter().any(|x| x == "--raw");
            for (i, e) in prefab.ents.iter().enumerate().take(if raw { 6 } else { usize::MAX }) {
                let model_s = if e.model.index < 0 { "null".to_string() } else { format!("node {}", e.model.index) };
                let tags = mapgeom::static_item::prefab::placement_tags(&e.params).unwrap_or_default();
                println!("  ent {i:>3}: model {model_s:<8} pos ({:.2},{:.2},{:.2}) rot ({:.3},{:.3},{:.3},{:.3}) params {:#x} [{} B]{}", e.pos[0], e.pos[1], e.pos[2], e.rot[0], e.rot[1], e.rot[2], e.rot[3], e.params_id, e.params.len(), if tags.is_empty() { String::new() } else { format!(" tags {}", tags.iter().map(|(k, v)| format!("{k}={v}")).collect::<Vec<_>>().join(",")) });
                if raw && !e.params.is_empty() { println!("      {}", e.params.iter().take(96).map(|b| format!("{b:02x}")).collect::<Vec<_>>().join(" ")); }
            }
        }
        "material-albedo" => {
            // material-albedo LINK… [--out TSV]: the MEAN LINEAR RGB of each game material's diffuse texture — the
            // lightmapper's MDiffuse stand-in for the bounce. LINK = "Stadium\Media\Material\RoadTech" (the
            // CPlugMaterialUserInst link); the material file's externals are followed transitively and the
            // diffuse image is the `<name>_D.dds` matching the material's own name, else the first `_D.dds`,
            // else the first .dds. The DDS (BC1/BC3 via the store) is decoded at ≤ 256 px, sRGB → linear,
            // averaged with alpha as the weight when the image has one. Prints link, image, rgb, luminance.
            let mut store = open(&a);
            let out_path = flag(&a.rest, "--out");
            let mut rows: Vec<String> = Vec::new();
            let mut links: Vec<String> = Vec::new();
            let mut it = a.rest.iter().skip(1);
            while let Some(x) = it.next() {
                if x == "--out" { it.next(); continue; }
                if x.starts_with("--") { continue; }
                links.push(x.clone());
            }
            for link in &links {
                let file = format!("{link}.Material.Gbx");
                let name = link.rsplit('\\').next().unwrap_or(link).to_ascii_lowercase();
                let mut seen: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
                let mut dds: Vec<String> = Vec::new();
                let mut stack: Vec<(usize, String)> = vec![(0, file.clone())];
                while let Some((depth, path)) = stack.pop() {
                    if depth > 4 || !seen.insert(path.to_ascii_lowercase()) { continue; }
                    if path.to_ascii_lowercase().ends_with(".dds") { dds.push(path); continue; }
                    if store.resolve(&path).is_none() { continue; }
                    if let Ok(child) = store.load_model(&path) {
                        for (_, p) in child.externals.iter() { stack.push((depth + 1, p.clone())); }
                    }
                }
                let stem = |p: &str| p.rsplit('\\').next().unwrap_or(p).to_ascii_lowercase();
                let pick = dds.iter().find(|p| stem(p) == format!("{name}_d.dds"))
                    .or_else(|| dds.iter().find(|p| stem(p).ends_with("_d.dds")))
                    .or_else(|| dds.first());
                let Some(img) = pick else { println!("{link}\t(no diffuse image among {} externals)", seen.len()); continue };
                let bytes = match store.read(img) { Ok(b) => b.to_vec(), Err(e) => { println!("{link}\t{img}\t(unreadable: {e})"); continue } };
                let (w, h, rgba) = match mapgeom::static_item::texture::decode_capped_rgba(&bytes, 256) { Ok(x) => x, Err(e) => { println!("{link}\t{img}\t(undecodable: {e})"); continue } };
                let to_lin = |c: u8| { let x = c as f64 / 255.0; if x <= 0.04045 { x / 12.92 } else { ((x + 0.055) / 1.055).powf(2.4) } };
                let (mut acc, mut wsum) = ([0f64; 3], 0f64);
                let has_alpha = rgba.chunks_exact(4).any(|p| p[3] < 250);
                for p in rgba.chunks_exact(4) {
                    let wt = if has_alpha { p[3] as f64 / 255.0 } else { 1.0 };
                    for k in 0..3 { acc[k] += to_lin(p[k]) * wt; }
                    wsum += wt;
                }
                if wsum <= 0.0 { println!("{link}\t{img}\t(fully transparent)"); continue; }
                let rgb = [acc[0] / wsum, acc[1] / wsum, acc[2] / wsum];
                let lum = 0.2126 * rgb[0] + 0.7152 * rgb[1] + 0.0722 * rgb[2];
                let row = format!("{link}\t{img}\t{w}x{h}\t{:.4}\t{:.4}\t{:.4}\t{:.4}", rgb[0], rgb[1], rgb[2], lum);
                println!("{row}");
                rows.push(row);
            }
            if let Some(o) = out_path { std::fs::write(&o, rows.join("\n") + "\n").unwrap_or_else(|e| die(format!("{o}: {e}"))); }
        }
        "blockitem-archetype" => {
            // blockitem-archetype IN.Block.Gbx --out OUT --archetype NAME [--no-collide]:
            // an embedded custom block re-pointed at another ARCHETYPE block info, any
            // name length: every length-prefixed occurrence of the current archetype
            // string is spliced; the header chunk that carries it and the header size
            // grow by the delta (the body occurrences sit in non-skippable chunks:
            // 0x2E001009 and the CGameBlockItem lookback string). --no-collide clears
            // the CPlugStaticObjectModel collidable byte so the block's own mesh stops
            // nothing (the archetype's water volume is what we want). Uncompressed
            // Gbx only (the TMX custom blocks). 2026-09-10, the ship17 water blocks.
            let p = a.rest.get(1).cloned().unwrap_or_else(|| die("blockitem-archetype IN.Block.Gbx --out OUT --archetype NAME".into()));
            let out = flag(&a.rest, "--out").unwrap_or_else(|| die("--out FILE".into()));
            let arche = flag(&a.rest, "--archetype").unwrap_or_else(|| die("--archetype NAME".into()));
            let bytes = std::fs::read(&p).unwrap_or_else(|e| die(e.to_string()));
            if bytes.get(7) != Some(&b'U') {
                die::<()>(format!("{p}: body is compressed; this patches bytes in place"));
            }
            let header_size = u32::from_le_bytes(bytes[13..17].try_into().unwrap()) as usize;
            let nchunks = u32::from_le_bytes(bytes[17..21].try_into().unwrap()) as usize;
            let table = 21usize;
            let data0 = table + nchunks * 8;
            let header_end = 17 + header_size;
            // the current archetype = the block-name-looking string that appears at
            // least twice as a length-prefixed field (ident chunk, 0x2E001009, the
            // CGameBlockItem lookback string)
            let mut cands: std::collections::BTreeMap<String, Vec<usize>> = Default::default();
            let mut i = 0usize;
            while i + 4 < bytes.len() {
                let n = u32::from_le_bytes(bytes[i..i + 4].try_into().unwrap()) as usize;
                if (4..=48).contains(&n) && i + 4 + n <= bytes.len() && bytes[i + 4..i + 4 + n].iter().all(|c| c.is_ascii_alphanumeric()) {
                    let s = String::from_utf8_lossy(&bytes[i + 4..i + 4 + n]).to_string();
                    cands.entry(s).or_default().push(i);
                    i += 4 + n;
                } else {
                    i += 1;
                }
            }
            let (cur, offs) = cands
                .iter()
                .filter(|(_, v)| v.len() >= 2)
                .max_by_key(|(_, v)| v.len())
                .map(|(k, v)| (k.clone(), v.clone()))
                .unwrap_or_else(|| die("no repeated length-prefixed block name in the file".into()));
            let delta = arche.len() as i64 - cur.len() as i64;
            let mut out_bytes: Vec<u8> = Vec::with_capacity(bytes.len() + 64);
            let mut last = 0usize;
            let mut header_delta = 0i64;
            let mut chunk_deltas: Vec<(usize, i64)> = Vec::new();
            for o in &offs {
                out_bytes.extend_from_slice(&bytes[last..*o]);
                out_bytes.extend_from_slice(&(arche.len() as u32).to_le_bytes());
                out_bytes.extend_from_slice(arche.as_bytes());
                last = o + 4 + cur.len();
                if *o < header_end {
                    header_delta += delta;
                    let mut start = data0;
                    for k in 0..nchunks {
                        let size = (u32::from_le_bytes(bytes[table + k * 8 + 4..table + k * 8 + 8].try_into().unwrap()) & 0x7FFF_FFFF) as usize;
                        if *o >= start && *o < start + size {
                            chunk_deltas.push((k, delta));
                        }
                        start += size;
                    }
                }
            }
            out_bytes.extend_from_slice(&bytes[last..]);
            let new_header = (header_size as i64 + header_delta) as u32;
            out_bytes[13..17].copy_from_slice(&new_header.to_le_bytes());
            for (k, d) in chunk_deltas {
                let so = table + k * 8 + 4;
                let raw = u32::from_le_bytes(out_bytes[so..so + 4].try_into().unwrap());
                let flagbit = raw & 0x8000_0000;
                let size = ((raw & 0x7FFF_FFFF) as i64 + d) as u32;
                out_bytes[so..so + 4].copy_from_slice(&(size | flagbit).to_le_bytes());
            }
            let mut note = String::new();
            if a.rest.iter().any(|x| x == "--no-collide") {
                // CPlugStaticObjectModel (0x09159000): in the TMX blocks the collidable
                // byte (1) is followed by the shape ref -1; clear the first such byte
                // after each class word
                let mut patched = 0;
                let mut k = 0usize;
                while k + 4 <= out_bytes.len() {
                    if u32::from_le_bytes(out_bytes[k..k + 4].try_into().unwrap()) == 0x0915_9000 {
                        for j in k + 4..(k + 64).min(out_bytes.len() - 5) {
                            if out_bytes[j] == 1 && out_bytes[j + 1..j + 5] == [0xff, 0xff, 0xff, 0xff] {
                                out_bytes[j] = 0;
                                patched += 1;
                                break;
                            }
                        }
                    }
                    k += 1;
                }
                note = format!("; {patched} collidable byte(s) cleared");
            }
            std::fs::write(&out, &out_bytes).unwrap_or_else(|e| die(e.to_string()));
            println!("{p}: archetype {cur} -> {arche} at {} places (header delta {header_delta}){note} -> {out}", offs.len());
        }
        "giantwater" => {
            // giantwater GIANT.Map.Gbx --source SRC.Map.Gbx --anchor sx,sy,sz:tx,ty,tz --scale N --out MAP2
            //   [--template T.Block.Gbx] [--table T.tsv] [--author UID] [--no-roads] — see giantwater.rs
            let p = a.rest.get(1).cloned().unwrap_or_else(|| die("giantwater GIANT --source SRC --anchor S:T --scale N --out F".into()));
            let src = flag(&a.rest, "--source").unwrap_or_else(|| die("--source SRC.Map.Gbx".into()));
            let anchor = flag(&a.rest, "--anchor").unwrap_or_else(|| die("--anchor sx,sy,sz:tx,ty,tz".into()));
            let scale: f32 = flag(&a.rest, "--scale").unwrap_or_else(|| "2".into()).parse().unwrap_or_else(|_| die("--scale number".into()));
            let out = flag(&a.rest, "--out").unwrap_or_else(|| die("--out MAP".into()));
            let template = flag(&a.rest, "--template");
            let table_out = flag(&a.rest, "--table");
            let author = flag(&a.rest, "--author").unwrap_or_else(|| "fHFOZ36-Qt6hMnhWK6bvxw".to_string());
            let roads = !a.rest.iter().any(|x| x == "--no-roads");
            // --rewater: drop the giant map's existing pool tiles first (a water-only
            // rebuild); --anchor from-tiles derives the source→giant transform from those
            // tiles (the min-corner pool block and its min-corner tile); --legacy-stack =
            // the 2026-09-13 variant scheme (a sheet per row)
            let rewater = a.rest.iter().any(|x| x == "--rewater");
            let legacy = a.rest.iter().any(|x| x == "--legacy-stack");
            let source = tmmaps::map::MapFile::load(std::path::Path::new(&src));
            let collection = source.items.first().map(|it| it.collection_raw).unwrap_or(26);
            let ground = tmmaps::map::ground_y(collection);
            let (s, t) = if anchor == "from-tiles" {
                let giant = tmmaps::map::MapFile::load(std::path::Path::new(&p));
                let n = scale.round() as i32;
                let mut found: Option<([f32; 3], [f32; 3])> = None;
                for name in ["DecoWallWaterBase", "WaterBase"] {
                    let sb: Vec<(i32, i32, i32)> = source.blocks.iter().filter(|b| b.free_pos.is_none() && b.name == name).map(|b| b.coords()).collect();
                    let gb: Vec<(i32, i32, i32)> = giant.blocks.iter().filter(|b| b.free_pos.is_none() && b.name == name).map(|b| b.coords()).collect();
                    if sb.is_empty() || gb.is_empty() {
                        continue;
                    }
                    let smin = (sb.iter().map(|c| c.0).min().unwrap(), sb.iter().map(|c| c.1).min().unwrap(), sb.iter().map(|c| c.2).min().unwrap());
                    let gmin = (gb.iter().map(|c| c.0).min().unwrap(), gb.iter().map(|c| c.1).min().unwrap(), gb.iter().map(|c| c.2).min().unwrap());
                    // WaterBase tiles sit in the TOP row of the doubled cell only
                    let gy = if name == "WaterBase" { gmin.1 - (n - 1) } else { gmin.1 };
                    let s = [smin.0 as f32 * tmmaps::map::CELL_XZ, smin.1 as f32 * tmmaps::map::CELL_Y + ground, smin.2 as f32 * tmmaps::map::CELL_XZ];
                    let t = [gmin.0 as f32 * tmmaps::map::CELL_XZ, gy as f32 * tmmaps::map::CELL_Y + ground, gmin.2 as f32 * tmmaps::map::CELL_XZ];
                    found = Some((s, t));
                    println!("  giantwater: anchor from the {name} tiles: source cell {:?} -> giant cell ({}, {}, {}); anchor {:?} -> {:?}", smin, gmin.0, gy, gmin.2, s, t);
                    break;
                }
                found.unwrap_or_else(|| die("--anchor from-tiles: the giant map has no pool tiles to derive the transform from".into()))
            } else {
                mapgeom::giantwater::parse_anchor(&anchor).unwrap_or_else(die)
            };
            let below: u32 = flag(&a.rest, "--below-flags").map(|h| u32::from_str_radix(h.trim_start_matches("0x"), 16).unwrap_or_else(|_| die("--below-flags HEX".into()))).unwrap_or(mapgeom::giantwater::STACKED_BELOW);
            // --no-clip: keep every pool tile whose cell a cell byte can hold (x/z 0..254,
            // y 0..255) instead of only the map grid — the giant Summer probe of what the
            // engine does with grid blocks past the 48-cell arena (2026-09-22)
            // the cells kept: the GIANT map's own grid (its size words — a build with
            // --anchor fit-grid has grown them), or everything a cell byte holds
            let bounds = if a.rest.iter().any(|x| x == "--no-clip") {
                Some([255, 256, 255])
            } else {
                let giant = tmmaps::map::MapFile::load(std::path::Path::new(&p));
                Some(giant.size)
            };
            // --free all|outside: pool tiles as FREE custom blocks (all pools, or the water
            // bodies that reach past the map grid) — needs --template
            let free = match flag(&a.rest, "--free").as_deref() {
                None | Some("none") => mapgeom::giantwater::FreePools::None,
                Some("all") => mapgeom::giantwater::FreePools::All,
                Some("outside") => mapgeom::giantwater::FreePools::Outside,
                Some(o) => die(format!("--free {o}: all, outside or none")),
            };
            // the VOLUME rule (2026-10-01) is the default: every block with a water volume
            // tiled by its surface class; --legacy-pools = the 2026-09-13/22 POOL_BLOCKS form
            let plan = if a.rest.iter().any(|x| x == "--legacy-pools") {
                mapgeom::giantwater::plan_free(&source, ground, s, t, scale, roads, &author, legacy, below, bounds, free).unwrap_or_else(die)
            } else {
                let mut store = open(&a);
                let coll = flag(&a.rest, "--collection").unwrap_or_else(|| "Stadium".to_string());
                let mut idx = mapgeom::blockmap::BlockInfoIndex::build(&store, &coll);
                mapgeom::giantwater::plan_volumes(&mut store, &mut idx, &source, ground, s, t, scale, roads, &author, below, bounds).unwrap_or_else(die)
            };
            for n in &plan.notes {
                println!("  giantwater: {n}");
            }
            for sk in &plan.skipped {
                println!("  giantwater: skipped {sk}");
            }
            if let Some(tp) = &table_out {
                let mut tsv = String::from("kind\tname\tflags\tdir\tcell_or_pos\tyaw\n");
                for g in &plan.grid {
                    tsv.push_str(&format!("grid\t{}\t{:08x}\t{}\t{:?}\t-\n", g.name, g.flags, g.dir, g.grid.unwrap_or([0; 3])));
                }
                for r in &plan.roads {
                    tsv.push_str(&format!("free\t{}\t{:08x}\t-\t({:.1}, {:.1}, {:.1})\t{:.4}\n", r.name, r.flags, r.pos[0], r.pos[1], r.pos[2], r.rot[0]));
                }
                std::fs::write(tp, tsv).unwrap_or_else(|e| die(e.to_string()));
            }
            let (g, r) = mapgeom::giantwater::apply_opt(std::path::Path::new(&p), std::path::Path::new(&out), &plan, template.as_deref().map(std::path::Path::new), &author, rewater).unwrap_or_else(die);
            println!("{p}: giant water: {g} native pool tiles, {r} road volume tiles -> {out}");
        }
        "overflow" => {
            // overflow MAP.Map.Gbx… [--report R.tsv]: does the map HIT the stadium? The
            // world box of every EMBEDDED item's geometry (the assembler's placed model;
            // geometry (the assembler's placed model; blocks are inside the grid by
            // construction) against x/z 0..size·32 m and y up to the grid top
            // (ground + size_y·8): one row per map — in/out, the box, how far out
            // (2026-09-14, Everios96 on the giant set: "some maps are too large and
            // don't fit inside the stadium anymore"; those get the NoStadium mood).
            let mut store = open(&a);
            let maps: Vec<String> = a.rest.iter().skip(1).filter(|x| !x.starts_with("--") && x.ends_with(".Gbx")).cloned().collect();
            if maps.is_empty() {
                die::<()>("overflow MAP.Map.Gbx… [--report R.tsv]".into());
            }
            let report = flag(&a.rest, "--report");
            let mut rows = String::from("map\tname\tverdict\tlo_x\tlo_y\tlo_z\thi_x\thi_y\thi_z\tgrid_x\tgrid_y_top\tgrid_z\tout_x\tout_y\tout_z\titems_out\tdecoration\tworst_item\n");
            let mut n_out = 0usize;
            for p in &maps {
                let m = tmmaps::map::MapFile::load(std::path::Path::new(p));
                let hdr = tmmaps::header::read(p).ok();
                let collection = m.items.first().map(|it| it.collection_raw).unwrap_or(26);
                let ground = tmmaps::map::ground_y(collection); // cell row 0 (Stadium: -64)
                let grass = tmmaps::tiny::fixed_plane(collection); // the ground plane the cars drive on (Stadium: 8)
                let grid = [m.size[0] as f32 * tmmaps::map::CELL_XZ, ground + m.size[1] as f32 * tmmaps::map::CELL_Y, m.size[2] as f32 * tmmaps::map::CELL_XZ];
                let mut asm = mapgeom::assemble::Assembler::new(&mut store);
                asm.with_embedded(&m).ok();
                let (mut lo, mut hi) = ([f32::MAX; 3], [f32::MIN; 3]);
                let mut items_out = 0usize;
                let mut worst: (f32, String) = (0.0, String::new());
                for it in &m.items {
                    let Some(lm) = asm.item_model(&it.model) else { continue };
                    let xf = mapgeom::place::anchored(it.pos, [it.yaw, it.pitch, it.roll], it.pivot, it.scale);
                    let (mut ilo, mut ihi) = ([f32::MAX; 3], [f32::MIN; 3]);
                    for g in lm.scene.groups.values() {
                        for v in &g.verts {
                            let w = mapgeom::geom::apply(&xf, *v);
                            for k in 0..3 {
                                ilo[k] = ilo[k].min(w[k]);
                                ihi[k] = ihi[k].max(w[k]);
                            }
                        }
                    }
                    if ilo[0] > ihi[0] {
                        continue;
                    }
                    // What the STADIUM physically occupies, measured on the decoration's
                    // own mesh (Stadium256\Media\Solid\Warp\Stade1536v2.Prefab, 2026-09-14):
                    // the stands' inner face rises exactly at the grid edge from 16 m
                    // above the ground (the first 16 m are the open apron; the upper tier
                    // recedes to 26 m outside), and the roof / corner towers begin 7 m
                    // OUTSIDE the grid at the 256 m line — the open sky over the pitch
                    // reaches all the way up. So an item HITS the stadium when it crosses
                    // the grid edge sideways above the apron (or by more than 32 m at
                    // ground level: past the apron), or rises above 256 m within 8 m of
                    // the edge. Half a metre of tolerance: the ×2 clips sit flush on the
                    // edge. Pure height inside the grid is reported (out_y), not counted.
                    let side = (-ilo[0]).max(ihi[0] - grid[0]).max(-ilo[2]).max(ihi[2] - grid[2]);
                    let above = ihi[1] - grid[1];
                    let near_edge = ilo[0] < 8.0 || ilo[2] < 8.0 || ihi[0] > grid[0] - 8.0 || ihi[2] > grid[2] - 8.0;
                    let hits_stands = side > 0.5 && (ihi[1] > grass + 16.0 || side > 32.0);
                    let hits_roof = above > 0.5 && near_edge;
                    if hits_stands || hits_roof {
                        items_out += 1;
                        let d = side.max(above);
                        if d > worst.0 {
                            worst = (d, format!("i{} {} at ({:.0}, {:.0}, {:.0}) box [{:.0}, {:.0}, {:.0}]..[{:.0}, {:.0}, {:.0}]{}", it.index, it.model, it.pos[0], it.pos[1], it.pos[2], ilo[0], ilo[1], ilo[2], ihi[0], ihi[1], ihi[2], if hits_roof && !hits_stands { " (roof)" } else { "" }));
                        }
                    }
                    for k in 0..3 {
                        lo[k] = lo[k].min(ilo[k]);
                        hi[k] = hi[k].max(ihi[k]);
                    }
                }
                let out_x = (-lo[0]).max(hi[0] - grid[0]).max(0.0);
                let out_z = (-lo[2]).max(hi[2] - grid[2]).max(0.0);
                let out_y = (hi[1] - grid[1]).max(0.0);
                let verdict = if items_out > 0 { "OUT" } else { "in" };
                if items_out > 0 {
                    n_out += 1;
                }
                let name = hdr.as_ref().map(|h| h.name.clone()).unwrap_or_default();
                let deco = m.decoration_id.clone();
                let line = format!("{p}\t{name}\t{verdict}\t{:.0}\t{:.0}\t{:.0}\t{:.0}\t{:.0}\t{:.0}\t{:.0}\t{:.0}\t{:.0}\t{out_x:.0}\t{out_y:.0}\t{out_z:.0}\t{items_out}\t{deco}\t{}", lo[0], lo[1], lo[2], hi[0], hi[1], hi[2], grid[0], grid[1], grid[2], worst.1);
                println!("{line}");
                rows.push_str(&line);
                rows.push('\n');
            }
            if let Some(r) = report {
                std::fs::write(&r, rows).unwrap_or_else(|e| die(e.to_string()));
            }
            eprintln!("{} of {} maps spill out of the stadium volume", n_out, maps.len());
        }
        "waterblocks" => {
            // waterblocks MAP --plates P.tsv --template T.Block.Gbx --out MAP2 --table T.tsv
            //   [--author UID] [--force-block] — see waterblocks.rs
            let mut store = open(&a);
            let p = a.rest.get(1).cloned().unwrap_or_else(|| die("waterblocks MAP --plates P --template T.Block.Gbx --out F --table T".into()));
            let plates_path = flag(&a.rest, "--plates").unwrap_or_else(|| die("--plates WATER-PLATES/NN.tsv".into()));
            let template = flag(&a.rest, "--template").unwrap_or_else(|| die("--template T.Block.Gbx (an uncompressed custom block)".into()));
            let out = flag(&a.rest, "--out").unwrap_or_else(|| die("--out MAP".into()));
            let table_out = flag(&a.rest, "--table");
            let author = flag(&a.rest, "--author").unwrap_or_else(|| "fHFOZ36-Qt6hMnhWK6bvxw".to_string());
            let force = a.rest.iter().any(|x| x == "--force-block");
            let plates = mapgeom::waterblocks::read_plates(&plates_path).unwrap_or_else(die);
            let m = tmmaps::map::MapFile::load(std::path::Path::new(&p));
            // every upward collision triangle of the map's items, world frame
            let mut asm = mapgeom::assemble::Assembler::new(&mut store);
            asm.with_embedded(&m).ok();
            let mut tris: Vec<mapgeom::waterblocks::UpTri> = Vec::new();
            for it in &m.items {
                let Some(lm) = asm.item_model(&it.model) else { continue };
                let lm = lm.clone();
                let xf = mapgeom::place::anchored(it.pos, [it.yaw, it.pitch, it.roll], it.pivot, it.scale);
                for (mat, g) in &lm.scene.groups {
                    if mat == "Visual" { continue; }
                    for t in &g.tris {
                        let w = [mapgeom::geom::apply(&xf, g.verts[t[0] as usize]), mapgeom::geom::apply(&xf, g.verts[t[1] as usize]), mapgeom::geom::apply(&xf, g.verts[t[2] as usize])];
                        let e1 = [w[1][0] - w[0][0], w[1][1] - w[0][1], w[1][2] - w[0][2]];
                        let e2 = [w[2][0] - w[0][0], w[2][1] - w[0][1], w[2][2] - w[0][2]];
                        let ny = e1[2] * e2[0] - e1[0] * e2[2];
                        let nx = e1[1] * e2[2] - e1[2] * e2[1];
                        let nz = e1[0] * e2[1] - e1[1] * e2[0];
                        let len = (nx * nx + ny * ny + nz * nz).sqrt().max(1e-6);
                        if (ny / len).abs() < 0.5 { continue; }
                        let c = [(w[0][0] + w[1][0] + w[2][0]) / 3.0, (w[0][1] + w[1][1] + w[2][1]) / 3.0, (w[0][2] + w[1][2] + w[2][2]) / 3.0];
                        tris.push(mapgeom::waterblocks::UpTri { c, top: w[0][1].max(w[1][1]).max(w[2][1]), phys: mat.clone() });
                    }
                }
            }
            // --source SRC.Map.Gbx --anchor sx,sy,sz:tx,ty,tz: what the original has under each pool
            let src_map = flag(&a.rest, "--source").map(|s| tmmaps::map::MapFile::load(std::path::Path::new(&s)));
            let anchor: Option<([f32; 3], [f32; 3])> = flag(&a.rest, "--anchor").map(|s| {
                let (l, r) = s.split_once(':').unwrap_or_else(|| die("--anchor sx,sy,sz:tx,ty,tz".into()));
                let v = |t: &str| -> [f32; 3] { let f: Vec<f32> = t.split(',').map(|x| x.trim().parse().unwrap_or_else(|_| die("--anchor: bad number".into()))).collect(); [f[0], f[1], f[2]] };
                (v(l), v(r))
            });
            let source = match (&src_map, anchor) {
                (Some(m), Some((s, t))) => Some(mapgeom::waterblocks::SourceUnder { map: m, anchor_s: s, anchor_t: t }),
                (Some(_), None) => die("--source needs --anchor".into()),
                _ => None,
            };
            let mut decisions = mapgeom::waterblocks::decide(&plates, &tris, source.as_ref());
            decisions.extend(mapgeom::waterblocks::decide_roads(&plates, &tris));
            let mut table = String::from("body\tarchetype\tchoice\treason\tblock_origin\tspill\n");
            let mut specs: Vec<tmmaps::map::FreeBlockSpec> = Vec::new();
            let mut files: std::collections::BTreeMap<String, Vec<u8>> = Default::default();
            let tpl = std::fs::read(&template).unwrap_or_else(|e| die(format!("{template}: {e}")));
            let mut n_block = 0usize;
            for d in &decisions {
                let choice = if force { "block" } else { d.choice };
                table.push_str(&format!("{}\t{}\t{}\t{}\t({:.1}, {:.2}, {:.1})\t{}\n", d.body, d.archetype, choice, d.reason, d.origin[0], d.origin[1], d.origin[2], d.spill));
                if choice != "block" { continue; }
                n_block += 1;
                let ident = format!("Water\\{}.Block.Gbx", d.archetype);
                let zip_path = format!("Blocks/Water/{}.Block.Gbx", d.archetype);
                if !files.contains_key(&zip_path) {
                    // the template re-pointed at this archetype (mapgeom blockitem-archetype's logic, in-process)
                    let tmp_in = std::env::temp_dir().join(format!("wb-{}.in.Block.Gbx", d.archetype));
                    let tmp_out = std::env::temp_dir().join(format!("wb-{}.Block.Gbx", d.archetype));
                    std::fs::write(&tmp_in, &tpl).unwrap_or_else(|e| die(e.to_string()));
                    let status = std::process::Command::new(std::env::current_exe().unwrap())
                        .args(["blockitem-archetype", tmp_in.to_str().unwrap(), "--out", tmp_out.to_str().unwrap(), "--archetype", &d.archetype])
                        .status()
                        .unwrap_or_else(|e| die(e.to_string()));
                    if !status.success() { die::<()>(format!("blockitem-archetype failed for {}", d.archetype)); }
                    // the file's own ident must be the manifest's (the game pairs them by ident,
                    // not by archive path): rename it as `rename-item` does
                    let raw = std::fs::read(&tmp_out).unwrap_or_else(|e| die(e.to_string()));
                    let (old_ident, _) = tmmaps::header::item_ident_author(&raw).unwrap_or_else(|| die("template block: no ident in the header".into()));
                    let renamed = mapgeom::crystal::rename_ident(&raw, &old_ident, &ident);
                    files.insert(zip_path.clone(), renamed);
                }
                specs.push(tmmaps::map::FreeBlockSpec { name: format!("{ident}_CustomBlock"), author: Some(author.clone()), flags: 0x1020_8000, pos: d.origin, rot: [d.yaw, 0.0, 0.0], grid: None, dir: 0, waypoint: None });
            }
            println!("{p}: {} water bodies handled, {} as blocks, {} archetype files", decisions.len(), n_block, files.len());
            if let Some(t) = &table_out {
                std::fs::write(t, &table).unwrap_or_else(|e| die(e.to_string()));
            }
            if n_block == 0 {
                std::fs::copy(&p, &out).unwrap_or_else(|e| die(e.to_string()));
                println!("  no blocks: {out} is a copy");
                return;
            }
            // 1. the archive: add the block files and their manifest rows
            let mut m2 = tmmaps::map::MapFile::load(std::path::Path::new(&p));
            let (mut zip, existing) = tmmaps::header::embedded_zip_bytes(&m2.gbx.body).unwrap_or_default();
            for (path, bytes) in &files {
                zip = tmmaps::header::zip_add(&zip, path, bytes);
            }
            let kept: Vec<String> = existing.iter().filter(|n| n.to_ascii_lowercase().ends_with(".item.gbx")).map(|n| n.rsplit(['/', '\\']).next().unwrap_or(n).to_string()).collect();
            let block_rows: Vec<(String, String)> = files.keys().map(|zp| (zp.trim_start_matches("Blocks/").replace('/', "\\"), author.clone())).collect();
            let mut manifest: Vec<(&str, &str)> = kept.iter().map(|n| (n.as_str(), n.as_str())).collect();
            manifest.extend(block_rows.iter().map(|(i, au)| (i.as_str(), au.as_str())));
            m2.replace_embedded_objects(&manifest, &zip);
            let tmp_map = std::path::PathBuf::from(format!("{out}.stage1.Map.Gbx"));
            m2.write_to(&tmp_map).unwrap_or_else(|e| die(e.to_string()));
            // 2. the free block records (a fresh load: variable-length edits are a separate pass)
            let mut m3 = tmmaps::map::MapFile::load(&tmp_map);
            let r = m3.remove_and_add_blocks(|_| false, |_| false, &specs);
            m3.write_to(std::path::Path::new(&out)).unwrap_or_else(|e| die(e.to_string()));
            let _ = std::fs::remove_file(&tmp_map);
            let m4 = tmmaps::map::MapFile::load(std::path::Path::new(&out));
            println!("  {out}: {} free blocks ({} added; Id table {} -> {}), manifest {} rows", m4.blocks.iter().filter(|b| b.flags & tmmaps::map::FREE_BLOCK_FLAG != 0).count(), specs.len(), r.table_before, r.table_after, manifest.len());
        }
        "raycast" => {
            // raycast MAP --from x,y,z --dir dx,dy,dz [--fan H0:H1:N] [--pitch P0:P1:M] [--max D]
            //   [--report R.tsv] [--source SRC.Map.Gbx --anchor sx,sy,sz:tx,ty,tz]
            // The first hit of camera rays over the map's ITEM triangles (collision and
            // visual groups), for naming what a frame's pixel region shows: per ray the
            // distance, the hit point, the item, its model and source block (--report).
            // --fan/--pitch sweep the direction: yaw offsets H0..H1 (radians, + = right)
            // in N steps and pitch offsets P0..P1 (+ = up) in M steps. With --source, the
            // SOURCE cells the same ray crosses at full scale are listed with the
            // source blocks in them (what the original has along the line of sight).
            let mut store = open(&a);
            let p = a.rest.get(1).cloned().unwrap_or_else(|| die("raycast MAP --from x,y,z --dir dx,dy,dz".into()));
            let v3 = |s: &str| -> [f32; 3] {
                let f: Vec<f32> = s.split(',').map(|x| x.trim().parse().unwrap_or_else(|_| die(format!("bad number in {s}")))).collect();
                if f.len() != 3 { die::<()>(format!("{s}: three numbers")); }
                [f[0], f[1], f[2]]
            };
            let from = v3(&flag(&a.rest, "--from").unwrap_or_else(|| die("--from x,y,z".into())));
            let dir0 = v3(&flag(&a.rest, "--dir").unwrap_or_else(|| die("--dir dx,dy,dz".into())));
            let maxd: f32 = flag(&a.rest, "--max").and_then(|s| s.parse().ok()).unwrap_or(600.0);
            let sweep = |s: Option<String>| -> Vec<f32> {
                match s {
                    None => vec![0.0],
                    Some(t) => {
                        let f: Vec<f32> = t.split(':').map(|x| x.parse().unwrap_or(0.0)).collect();
                        let n = f.get(2).copied().unwrap_or(1.0).max(1.0) as usize;
                        (0..n).map(|i| if n == 1 { f[0] } else { f[0] + (f[1] - f[0]) * i as f32 / (n - 1) as f32 }).collect()
                    }
                }
            };
            let yaws = sweep(flag(&a.rest, "--fan"));
            let pitches = sweep(flag(&a.rest, "--pitch"));
            let m = tmmaps::map::MapFile::load(std::path::Path::new(&p));
            let report: BTreeMap<String, String> = flag(&a.rest, "--report")
                .and_then(|r| std::fs::read_to_string(r).ok())
                .map(|text| text.lines().filter_map(|l| { let c: Vec<&str> = l.split('\t').collect(); ((c[0] == "block" || c[0] == "item" || c[0] == "tree") && c.len() > 4).then(|| (format!("{}.Item.Gbx", c[1]), c[4].to_string())) }).collect())
                .unwrap_or_default();
            // every triangle of every item, world frame, with its item index and kind
            let mut asm = mapgeom::assemble::Assembler::new(&mut store);
            asm.with_embedded(&m).ok();
            struct Tri { v: [[f32; 3]; 3], item: usize, visual: bool }
            let mut tris: Vec<Tri> = Vec::new();
            for (ii, it) in m.items.iter().enumerate() {
                let Some(lm) = asm.item_model(&it.model) else { continue };
                let lm = lm.clone();
                let xf = mapgeom::place::anchored(it.pos, [it.yaw, it.pitch, it.roll], it.pivot, it.scale);
                for (mat, g) in &lm.scene.groups {
                    let visual = mat == "Visual";
                    for t in &g.tris {
                        tris.push(Tri { v: [mapgeom::geom::apply(&xf, g.verts[t[0] as usize]), mapgeom::geom::apply(&xf, g.verts[t[1] as usize]), mapgeom::geom::apply(&xf, g.verts[t[2] as usize])], item: ii, visual });
                    }
                }
            }
            eprintln!("{} triangles over {} items", tris.len(), m.items.len());
            let src_map = flag(&a.rest, "--source").map(|s| tmmaps::map::MapFile::load(std::path::Path::new(&s)));
            let anchor: Option<([f32; 3], [f32; 3])> = flag(&a.rest, "--anchor").map(|s| { let (l, r) = s.split_once(':').unwrap_or_else(|| die("--anchor sx,sy,sz:tx,ty,tz".into())); (v3(l), v3(r)) });
            // --cell-y0 Y: world y of cell row 0 (Stadium −64; BlueBay −40 — the Land deck at
            // world 10 is cell 6, local +2)
            let cell_y0: f32 = flag(&a.rest, "--cell-y0").and_then(|s| s.parse().ok()).unwrap_or(-64.0);
            // base heading and pitch of --dir
            let base_h = dir0[0].atan2(dir0[2]);
            let horiz = (dir0[0] * dir0[0] + dir0[2] * dir0[2]).sqrt();
            let base_p = dir0[1].atan2(horiz);
            println!("yaw_off\tpitch_off\thit\tdist\tx\ty\tz\tkind\titem\tmodel\tsource_block\tsource_cells_crossed");
            for dy in &yaws {
                for dp in &pitches {
                    // + yaw offset = to the RIGHT of the view (right = (−cos h, sin h) for view (−sin h, −cos h); here the
                    // view is (sin h, cos h) so right = (cos h, −sin h) → h decreases to the right)
                    let h = base_h - dy;
                    let pch = base_p + dp;
                    let d = [pch.cos() * h.sin(), pch.sin(), pch.cos() * h.cos()];
                    // first hit (Möller–Trumbore), both kinds
                    let mut best: Option<(f32, usize, bool)> = None;
                    for t in &tris {
                        let e1 = [t.v[1][0] - t.v[0][0], t.v[1][1] - t.v[0][1], t.v[1][2] - t.v[0][2]];
                        let e2 = [t.v[2][0] - t.v[0][0], t.v[2][1] - t.v[0][1], t.v[2][2] - t.v[0][2]];
                        let pv = [d[1] * e2[2] - d[2] * e2[1], d[2] * e2[0] - d[0] * e2[2], d[0] * e2[1] - d[1] * e2[0]];
                        let det = e1[0] * pv[0] + e1[1] * pv[1] + e1[2] * pv[2];
                        if det.abs() < 1e-8 { continue; }
                        let inv = 1.0 / det;
                        let tv = [from[0] - t.v[0][0], from[1] - t.v[0][1], from[2] - t.v[0][2]];
                        let u = (tv[0] * pv[0] + tv[1] * pv[1] + tv[2] * pv[2]) * inv;
                        if !(0.0..=1.0).contains(&u) { continue; }
                        let qv = [tv[1] * e1[2] - tv[2] * e1[1], tv[2] * e1[0] - tv[0] * e1[2], tv[0] * e1[1] - tv[1] * e1[0]];
                        let w = (d[0] * qv[0] + d[1] * qv[1] + d[2] * qv[2]) * inv;
                        if w < 0.0 || u + w > 1.0 { continue; }
                        let dist = (e2[0] * qv[0] + e2[1] * qv[1] + e2[2] * qv[2]) * inv;
                        if dist > 0.5 && dist < maxd && best.map(|b| dist < b.0).unwrap_or(true) {
                            best = Some((dist, t.item, t.visual));
                        }
                    }
                    // the source cells crossed (full scale) up to the hit or maxd
                    let mut crossed = String::new();
                    if let (Some(sm), Some((s, t))) = (&src_map, anchor) {
                        let mut seen: Vec<(i32, i32)> = Vec::new();
                        let end = best.map(|b| b.0).unwrap_or(maxd);
                        let mut k = 0.0f32;
                        while k <= end {
                            let px = from[0] + d[0] * k;
                            let py = from[1] + d[1] * k;
                            let pz = from[2] + d[2] * k;
                            // tiny → source
                            let sx = s[0] + (px - t[0]) * 2.0;
                            let sy = s[1] + (py - t[1]) * 2.0;
                            let sz = s[2] + (pz - t[2]) * 2.0;
                            let (cx, cz) = ((sx / 32.0).floor() as i32, (sz / 32.0).floor() as i32);
                            if !seen.contains(&(cx, cz)) {
                                seen.push((cx, cz));
                                let cy_ray = ((sy - cell_y0) / 8.0).floor() as i32;
                                // the source blocks in this column at or above the ray's cell y (what could block the view)
                                let mut names: Vec<String> = Vec::new();
                                for b in sm.blocks.iter().chain(sm.baked.iter()) {
                                    if b.flags == 0xFFFF_FFFF { continue; }
                                    let (bx, by, bz) = (b.file_cell[0] as i32 - 1, b.file_cell[1] as i32, b.file_cell[2] as i32 - 1);
                                    if bx == cx && bz == cz && by >= cy_ray - 1 && !names.iter().any(|n| n.starts_with(&b.name)) {
                                        names.push(format!("{}@y{}", b.name, by));
                                    }
                                }
                                crossed.push_str(&format!("({cx},{cz}) ray_y{cy_ray}: {}; ", if names.is_empty() { "-".to_string() } else { names.join(" ") }));
                            }
                            k += 8.0;
                        }
                    }
                    match best {
                        Some((dist, ii, visual)) => {
                            let it = &m.items[ii];
                            let hp = [from[0] + d[0] * dist, from[1] + d[1] * dist, from[2] + d[2] * dist];
                            println!("{dy:.3}\t{dp:.3}\thit\t{dist:.1}\t{:.1}\t{:.1}\t{:.1}\t{}\ti{ii}\t{}\t{}\t{crossed}", hp[0], hp[1], hp[2], if visual { "visual" } else { "collision" }, it.model, report.get(&it.model).cloned().unwrap_or_default());
                        }
                        None => println!("{dy:.3}\t{dp:.3}\tMISS\t{maxd:.0}\t\t\t\t\t\t\t\t{crossed}"),
                    }
                }
            }
        }
        "uvcheck" => {
            // uvcheck ITEM.Item.Gbx…: per visual of a static item's Solid2, the vertex
            // declarations and the lightmap set (TexCoord1) against the texture set
            // (TexCoord0): bounds, distinct values, identical-to-uv0. The 2026-09-12
            // shadow-seam work: a lightmap chart that is a copy of a TILING uv0 wraps
            // onto itself and a tile bakes to one tone.
            use mapgeom::static_item::vstream::{Elem, N_TEXCOORD0};
            for p in a.rest.iter().skip(1).filter(|s| !s.starts_with("--")) {
                let bytes = std::fs::read(p).unwrap_or_else(|e| die(format!("{p}: {e}")));
                let f = mapgeom::static_item::parse_file(&bytes).unwrap_or_else(|e| die(format!("{p}: {e}")));
                let Some(s2) = f.item.static_object().and_then(|so| so.solid2()) else { println!("{p}\tno static Solid2"); continue };
                let pl = s2.pre_light_gen.as_ref().map(|g| format!("u02={:.3} u04={:?} boxes={} uv_groups={}", g.u02, &g.u04[..4], g.boxes.len(), g.uv_groups.len())).unwrap_or_else(|| "none".into());
                println!("{p}\tprelight {pl}\tgeoms {} lod_max_dist {:?}", s2.shaded_geoms.len(), s2.lod_max_dist);
                for (vi, vr) in s2.visuals.iter().enumerate() {
                    let Some(mapgeom::static_item::Node::Visual(v)) = vr.inline.as_deref() else { continue };
                    let Some(main) = v.main.as_ref() else { continue };
                    for sr in &main.vertex_streams {
                        let Some(mapgeom::static_item::Node::VertexStream(s)) = sr.inline.as_deref() else { continue };
                        let names: Vec<u32> = s.decls.iter().map(|d| d.name()).collect();
                        let get = |name: u32| s.decls.iter().zip(s.elems.iter()).find(|(d, _)| d.name() == name).and_then(|(_, e)| if let Elem::Float2(v) = e { Some(v.clone()) } else { None });
                        let (uv0, uv1) = (get(N_TEXCOORD0), get(N_TEXCOORD0 + 1));
                        let stats = |v: &Vec<[f32; 2]>| {
                            let (mut lo, mut hi) = ([f32::MAX; 2], [f32::MIN; 2]);
                            for p in v { for k in 0..2 { lo[k] = lo[k].min(p[k]); hi[k] = hi[k].max(p[k]); } }
                            let mut q: Vec<(i32, i32)> = v.iter().map(|p| ((p[0] * 4096.0) as i32, (p[1] * 4096.0) as i32)).collect();
                            q.sort(); q.dedup();
                            format!("[{:.3},{:.3}]..[{:.3},{:.3}] {} distinct", lo[0], lo[1], hi[0], hi[1], q.len())
                        };
                        let same = matches!((&uv0, &uv1), (Some(a), Some(b)) if a == b);
                        println!("  visual {vi}: {} verts, decl names {:?}, uv0 {}, uv1 {}{}", s.count, names, uv0.as_ref().map(&stats).unwrap_or_else(|| "-".into()), uv1.as_ref().map(&stats).unwrap_or_else(|| "-".into()), if same { " (uv1 == uv0)" } else { "" });
                    }
                }
            }
        }
        "trigger-box" => {
            // trigger-box ITEM.Item.Gbx [...]: the waypoint trigger shape of a CGameCommonItemEntityModel
            // item (the finish / checkpoint items the tiny bakes): kind, local AABB, triangle count,
            // and the spawn iso translation. For comparing one item across sets against the source.
            for p in a.rest.iter().skip(1).filter(|s| !s.starts_with("--")) {
                let bytes = std::fs::read(p).unwrap_or_else(|e| die(format!("{p}: {e}")));
                let f = mapgeom::static_item::parse_file(&bytes).unwrap_or_else(|e| die(format!("{p}: {e}")));
                let Some(em) = f.item.model().and_then(|mc| mc.entity_model()) else { println!("{p}\tno CGameCommonItemEntityModel"); continue };
                let spawn = [em.iso[9], em.iso[10], em.iso[11]];
                match em.trigger_shape.inline.as_deref() {
                    Some(mapgeom::static_item::Node::Surface(s)) => match &s.surf {
                        mapgeom::static_item::surface::Surf::Mesh { vertices, triangles, .. } => {
                            let mut lo = [f32::MAX; 3];
                            let mut hi = [f32::MIN; 3];
                            for v in vertices { for k in 0..3 { lo[k] = lo[k].min(v[k]); hi[k] = hi[k].max(v[k]); } }
                            println!("{p}\tmesh\t{} verts {} tris\tlocal aabb [{:.2}, {:.2}, {:.2}]..[{:.2}, {:.2}, {:.2}]\tspawn [{:.2}, {:.2}, {:.2}]\tmain_dir {:?}", vertices.len(), triangles.len(), lo[0], lo[1], lo[2], hi[0], hi[1], hi[2], spawn[0], spawn[1], spawn[2], s.gameplay_main_dir);
                        }
                        mapgeom::static_item::surface::Surf::Box { transform, .. } => println!("{p}\tbox\t{:?}\tspawn {:?}", transform, spawn),
                        other => println!("{p}\t{:?}\tspawn {:?}", std::mem::discriminant(other), spawn),
                    },
                    Some(_) => println!("{p}\ttrigger node is not a surface"),
                    None => println!("{p}\tno trigger shape (null ref {})\tspawn {:?}", em.trigger_shape.index, spawn),
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
        // terrain-material LINK… : a terrain material (`BlueBay\Media\Material\SeaFloor`) resolved through the
        // pack the way the game does (mapgeom::terrain): the layer names of chunk 0x0903A015, the BaseColor /
        // PyX2 / PyH2 texture arrays (ImageArray layers + slice images in GPU order), the shader ids
        // (iPy, iPxz, iPyX2, iPyH2 = SubIndexPyPxz) and the g_WorldPosToTc* buffers as uploaded (4 float4 per
        // layer, printed as floats and bits). `--buffers OUT` writes the BaseColor buffer's bytes (= the
        // captured 5352 for BlueBay) to a file.
        // material-colour-flags LINK… : CPlugMaterial+0xf0 bits 0/1 ("takes the placement colour" / "colour 2") from the
        // material's CPlugMaterialCustom GpuFx parameter names (chunk 0x0903A00A; the finaliser 0x14040ee40 tests list 2
        // for `BaseColorTargetId` / `DeactivableDisplayId`, list 1 for `BaseColorTarget` / `BaseColorTarget_sRGB`) — the
        // Solid2Model's +0x1f0 bits and the item clone key's colour follow from any material with bit 0 (RE 7, 2026-09-25)
        "material-colour-flags" => {
            let mut store = open(&a);
            let links: Vec<String> = a.rest.iter().skip(1).filter(|x| !x.starts_with("--")).cloned().collect();
            for link in &links {
                let file = if link.to_ascii_lowercase().ends_with(".gbx") { link.clone() } else { format!("{link}.Material.Gbx") };
                let m = match store.load_model(&file) { Ok(m) => m, Err(e) => { println!("{link}: {e}"); continue } };
                let g = match m.graph() { Ok(g) => g, Err(e) => { println!("{link}: {e}"); continue } };
                let mut found = false;
                for s in &g.slots {
                    if let mapgeom::node::Slot::Node(mapgeom::node::Node::MaterialCustom(c)) = s {
                        found = true;
                        let f = c.colour_flags();
                        println!("{link}: colour flags {f:#x}{}{} — GpuFx list 1 {:?}, list 2 {:?}", if f & 1 != 0 { " [takes the placement colour]" } else { "" }, if f & 2 != 0 { " [colour 2]" } else { "" }, c.gpufx_names[0], c.gpufx_names[1]);
                    }
                }
                if !found { println!("{link}: no CPlugMaterialCustom node → flags 0"); }
            }
        }
        "terrain-material" => {
            let mut store = open(&a);
            let out_path = flag(&a.rest, "--buffers");
            let links: Vec<String> = a.rest.iter().skip(1).filter(|x| !x.starts_with("--") && *x != out_path.as_ref().unwrap_or(&String::new())).cloned().collect();
            for link in &links {
                let tm = mapgeom::terrain::load_terrain_material(&mut store, link).unwrap_or_else(die);
                println!("{link}\n  parent {}\n  layer names (Pxz, Py, X2, H2) {:?}\n  ids iPy {} iPxz {} iPyX2 {} iPyH2 {}", tm.parent, tm.layer_names, tm.i_py, tm.i_pxz, tm.i_py_x2, tm.i_py_h2);
                for n in &tm.notes { println!("  note: {n}"); }
                for (slot, arr) in [("BaseColor", &tm.base), ("PyX2", &tm.x2), ("PyH2", &tm.h2)] {
                    let Some(arr) = arr else { println!("  {slot}: (no slot)"); continue };
                    println!("  {slot}: {} (gen {:?})\n    image array {} ({} layers)", arr.bitmap_path, arr.gen, arr.image_array_path, arr.layers.len());
                    for (i, sl) in arr.slices.iter().enumerate() {
                        println!("    slice {i}: {sl}");
                    }
                    let buf = mapgeom::terrain::world_pos_to_tc(&arr.layers);
                    for (i, l) in arr.layers.iter().enumerate() {
                        println!("    layer {i} {:<14} Py {:?} rot {}° Pxz {:?} off {:?} blendPxz {:?} blendPy {:?} ids {:?}", l.name, l.py_scale, l.rotation_deg, l.pxz_scale, l.pxz_offset, l.blend_pxz_deg, l.blend_py_deg, l.ids);
                        for k in 0..4 {
                            let v = buf[i * 4 + k];
                            println!("      [{k}] {:>13.9} {:>13.9} {:>13.9} {:>13.9}   {:08x} {:08x} {:08x} {:08x}", v[0], v[1], v[2], v[3], v[0].to_bits(), v[1].to_bits(), v[2].to_bits(), v[3].to_bits());
                        }
                    }
                }
                if let (Some(o), Some(b)) = (&out_path, &tm.base) {
                    let buf = mapgeom::terrain::world_pos_to_tc(&b.layers);
                    let bytes: Vec<u8> = buf.iter().flat_map(|v| v.iter().flat_map(|f| f.to_le_bytes())).collect();
                    std::fs::write(o, &bytes).unwrap_or_else(|e| die(format!("{o}: {e}")));
                    println!("  wrote {o} ({} bytes)", bytes.len());
                }
            }
        }
        // collection-water --collection C : the collection's water type (Collections\C.Collection.Gbx chunk
        // 0x03033038): name, top, floor, FogMaxDepth, the fog / transmittance / normal references, and the two
        // lightmapper table entries (g_WaterTop_ByPlanes = top; g_WaterDepth_FogMaxDepthInv_ByIds =
        // (top − floor, 1/FogMaxDepth)). `--fog-lut OUT` writes the 256 RGBA texels of the fog LUT (column 0 of
        // the fog TGA, top-down — the captured 15075).
        "collection-water" => {
            let mut store = open(&a);
            let coll = flag(&a.rest, "--collection").unwrap_or_else(|| "BlueBay".to_string());
            let w = mapgeom::terrain::collection_water(&mut store, &coll).unwrap_or_else(die);
            let d = w.depth_and_inv();
            println!("{coll}: water type {:?} top {} floor {} FogMaxDepth {} params {:?} tail {:?}\n  fog image {}\n  transmittance {}\n  normal {}\n  g_WaterTop_ByPlanes[0] = {} ; g_WaterDepth_FogMaxDepthInv_ByIds[0] = ({}, {} = 0x{:08x})", w.name, w.top, w.floor, w.fog_max_depth, w.params, w.tail, w.fog_image, w.transmittance, w.normal, w.top, d[0], d[1], d[1].to_bits());
            if !w.transmittance.is_empty() {
                if let Ok(m) = store.load_model(&w.transmittance) {
                    if let Ok(g) = m.graph() {
                        if let Some(root) = &g.root { println!("  transmittance generator: {}", describe(root)); }
                    }
                }
            }
            if !w.fog_image.is_empty() {
                let bytes = store.read(&w.fog_image).unwrap_or_else(die);
                let col = mapgeom::terrain::water_fog_lut(&bytes).unwrap_or_else(die);
                println!("  fog LUT (the first line along the long axis from the top-left, {} texels): [0] = {:?} … [{}] = {:?}", col.len(), col[0], col.len() - 1, col[col.len() - 1]);
                if let Some(o) = flag(&a.rest, "--fog-lut") {
                    let raw: Vec<u8> = col.iter().flat_map(|p| p.iter().copied()).collect();
                    std::fs::write(&o, &raw).unwrap_or_else(|e| die(format!("{o}: {e}")));
                    println!("  wrote {o} ({} bytes RGBA)", raw.len());
                }
            }
        }
        "deco-size" => {
            // mapgeom deco-size [--collection BlueBay]: every CGameCtnDecorationSize (0x0303B000) of the collection —
            // the fields the game's base height comes from (CGameCtnChallenge load 0x140b8cf0c: challenge+0x7e8 =
            // −(OffsetBlockY ? BaseHeightBase + 1 + BaseHeightOffset : BaseHeightOffset) · blockSize.y; the chunk reader
            // 0x140d1bd30: B001 = {BaseHeightBase, SizeX, SizeY, SizeZ, ref Scene}, B002 = {BaseHeightBase, Size xyz,
            // OffsetBlockY, ref Scene}, B003 = {version, BaseHeightOffset})
            let mut store = open(&a);
            let coll = flag(&a.rest, "--collection").unwrap_or_else(|| "BlueBay".to_string());
            // BlueBay-family paks keep it under <Coll>\GameCtnDecoration\<hash>, Stadium.pak at <Coll>\<hash>: filter by class
            let paths: Vec<String> = store.entries().filter(|e| e.class_id == 0x0303_B000).map(|e| e.path()).filter(|p| p.starts_with(&format!("{coll}\\"))).collect();
            for p in &paths {
                let Ok(m) = store.load_model(p) else { continue };
                if m.class_id != 0x0303_B000 { continue; }
                let b = &m.body;
                let rd = |o: usize| -> u32 { u32::from_le_bytes(b[o..o + 4].try_into().unwrap()) };
                let (mut base, mut off, mut offy, mut size) = (None, None, None, None);
                let mut o = 0usize;
                while o + 4 <= b.len() {
                    let id = rd(o);
                    if id == 0xFACA_DE01 { break; }
                    match id {
                        0x0303_B000 => { o += 4 + 20; }
                        0x0303_B001 => { base = Some(rd(o + 4)); size = Some([rd(o + 8), rd(o + 12), rd(o + 16)]); offy = Some(0); o += 4 + 20; }
                        0x0303_B002 => { base = Some(rd(o + 4)); size = Some([rd(o + 8), rd(o + 12), rd(o + 16)]); offy = Some(rd(o + 20)); o += 4 + 24; }
                        0x0303_B003 => { if &b[o + 4..o + 8] == b"PIKS" { let n = rd(o + 8) as usize; off = Some(rd(o + 16)); o += 12 + n; } else { off = Some(rd(o + 8)); o += 12; } }
                        _ => { if &b[o + 4..o + 8] == b"PIKS" { let n = rd(o + 8) as usize; o += 12 + n; } else { break; } }
                    }
                }
                let n = match (base, off, offy) { (Some(bh), Some(bo), Some(oy)) => Some(if oy != 0 { bh + 1 + bo } else { bo }), _ => None };
                println!("{p}: size {:?} BaseHeightBase {:?} BaseHeightOffset {:?} OffsetBlockY {:?} → n {:?} → base height −n·blockSize.y = {:?} (blockSize.y 8) / {:?} (32); externals {:?}", size, base, off, offy, n, n.map(|n| -(n as f32) * 8.0), n.map(|n| -(n as f32) * 32.0), m.externals);
            }
        }
        "raw" => {
            // mapgeom raw <logical-path> --out F : the decoded bytes of any pack
            // entry, GBX or not (textures, XML, tga), for banking mood assets
            let mut store = open(&a);
            let p = a.rest.get(1).cloned().unwrap_or_default();
            let out = flag(&a.rest, "--out").unwrap_or_else(|| die("raw: --out F is required".to_string()));
            let bytes = store.read(&p).unwrap_or_else(die);
            std::fs::write(&out, &bytes[..]).unwrap_or_else(|e| die(e.to_string()));
            println!("wrote {} ({} bytes)", out, bytes.len());
        }
        "dump" => {
            let p = a.rest.get(1).cloned().unwrap_or_default();
            // a local file needs no pack store
            let mut store = if std::path::Path::new(&p).is_file() && a.paks.is_empty() { DataStore::empty() } else { open(&a) };
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
        "reref" => {
            // reref (IN.Gbx | --from-pack LOGICAL) --out OUT [--print] [--ancestor N] [--prefix P]
            //       [--strip K] [--flags F] [--use-file U] [--entry NODE=PATH]... [--ident NAME.Item.Gbx]
            //       [--author LOGIN] [--matrix DIR]
            // The file with its REFERENCE TABLE rewritten and its body byte for byte
            // (mapgeom::reftable). --print only describes the table. --matrix DIR
            // writes the 2026-10-17 pack-reference matrix instead of one file: every
            // (ancestor level, root spelling, useFile) of the table, one item each,
            // ident = file name (the question: from where, if anywhere, can an item
            // in a map archive or the user's Items folder reach a PACK prefab).
            let from_pack = flag(&a.rest, "--from-pack");
            let bytes: Vec<u8> = match &from_pack {
                Some(logical) => {
                    let mut store = open(&a);
                    store.read(logical).unwrap_or_else(|e| die(format!("{logical}: {e}"))).as_ref().clone()
                }
                None => {
                    let inp = a.rest.get(1).unwrap_or_else(|| die("reref IN.Gbx --out OUT (or --from-pack LOGICAL)".into()));
                    std::fs::read(inp).unwrap_or_else(|e| die(format!("{inp}: {e}")))
                }
            };
            let table = mapgeom::reftable::table_of(&bytes);
            print!("{}", mapgeom::reftable::describe(&table));
            if a.rest.iter().any(|x| x == "--print") {
                return;
            }
            let author = flag(&a.rest, "--author").unwrap_or_else(|| mapgeom::tiny_assets::AUTHOR.to_string());
            let rename = |bytes: &[u8], ident: &str| -> Vec<u8> {
                let (from, from_author) = tmmaps::header::item_ident_author(bytes).unwrap_or_else(|| die("no ident in the header".into()));
                let renamed = mapgeom::crystal::rename_ident(bytes, &from, ident);
                if from_author == author {
                    return renamed;
                }
                // the author in the header ident and the body ident, with the header
                // chunk table and skippable chunk sizes following the length change
                // (replace_lp left the chunk sizes stale: the 2026-10-18 matrix of
                // wrappers was refused by the game for THAT, not for its references)
                mapgeom::crystal::rename_ident(&renamed, &from_author, &author)
            };
            if let Some(dir) = flag(&a.rest, "--matrix") {
                std::fs::create_dir_all(&dir).expect("matrix dir");
                let ancestors: Vec<u32> = flag(&a.rest, "--ancestors").unwrap_or_else(|| "0,1,2,3,4,6".into()).split(',').map(|s| s.trim().parse().expect("--ancestors a,b,c")).collect();
                // root spellings: the native one (nothing in front), the collection, the
                // pack mount, the drive names a plugin sees
                let roots: Vec<(String, String)> = flag(&a.rest, "--roots")
                    .unwrap_or_else(|| "nat=,std=Stadium,gd=GameData\\Stadium,game=Game\\Stadium".into())
                    .split(',')
                    .map(|s| {
                        let (tag, p) = s.split_once('=').expect("--roots tag=prefix,...");
                        (tag.to_string(), p.to_string())
                    })
                    .collect();
                let use_files: Vec<u32> = flag(&a.rest, "--use-files").unwrap_or_else(|| "0,1".into()).split(',').map(|s| s.trim().parse().expect("--use-files 0,1")).collect();
                let flags_v: Option<u32> = flag(&a.rest, "--flags").map(|f| f.parse().expect("--flags N"));
                let strip: usize = flag(&a.rest, "--strip").map(|s| s.parse().expect("--strip K")).unwrap_or(0);
                let mut list = String::new();
                let mut n = 0;
                for anc in &ancestors {
                    for (tag, prefix) in &roots {
                        for uf in &use_files {
                            let name = format!("R{tag}A{anc}U{uf}");
                            let ident = format!("{name}.Item.Gbx");
                            let rw = mapgeom::reftable::Rewrite { ancestor: Some(*anc), prefix: prefix.clone(), strip, flags: flags_v, use_file: Some(*uf), entry: vec![] };
                            let t2 = mapgeom::reftable::rewrite(&table, &rw);
                            let out = mapgeom::reftable::with_table(&rename(&bytes, &ident), &t2);
                            let path = std::path::Path::new(&dir).join(&ident);
                            std::fs::write(&path, &out).expect("write matrix item");
                            list += &format!("{ident}\t{anc}\t{tag}\t{uf}\t{}\n", t2.entries.iter().filter_map(|e| e.target.clone().ok()).collect::<Vec<_>>().join(" | "));
                            n += 1;
                        }
                    }
                }
                std::fs::write(std::path::Path::new(&dir).join("matrix.tsv"), format!("ident\tancestor\troot\tuse_file\tentries\n{list}")).expect("write matrix.tsv");
                println!("wrote {n} items + matrix.tsv in {dir}");
                return;
            }
            let out = flag(&a.rest, "--out").unwrap_or_else(|| die("--out FILE".into()));
            let mut rw = mapgeom::reftable::Rewrite::default();
            rw.ancestor = flag(&a.rest, "--ancestor").map(|s| s.parse().expect("--ancestor N"));
            rw.prefix = flag(&a.rest, "--prefix").unwrap_or_default();
            rw.strip = flag(&a.rest, "--strip").map(|s| s.parse().expect("--strip K")).unwrap_or(0);
            rw.flags = flag(&a.rest, "--flags").map(|s| s.parse().expect("--flags N"));
            rw.use_file = flag(&a.rest, "--use-file").map(|s| s.parse().expect("--use-file 0|1"));
            let mut i = 0;
            while i < a.rest.len() {
                if a.rest[i] == "--entry" {
                    let spec = a.rest.get(i + 1).unwrap_or_else(|| die("--entry NODE=PATH".into()));
                    let (node, path) = spec.split_once('=').unwrap_or_else(|| die("--entry NODE=PATH".into()));
                    rw.entry.push((node.parse().expect("--entry: NODE is a number"), path.to_string()));
                    i += 1;
                }
                i += 1;
            }
            let t2 = mapgeom::reftable::rewrite(&table, &rw);
            let src = match flag(&a.rest, "--ident") {
                Some(ident) => rename(&bytes, &ident),
                None => bytes.clone(),
            };
            let outb = mapgeom::reftable::with_table(&src, &t2);
            std::fs::write(&out, &outb).expect("write");
            print!("-> {}", mapgeom::reftable::describe(&t2));
            println!("wrote {out} ({} bytes, ident {:?})", outb.len(), tmmaps::header::item_ident_author(&outb));
        }
        "split-prefab" => {
            // split-prefab IN.Item.Gbx --out-dir D --name NAME
            // The item's inline entity model moved into its own NAME.Prefab.Gbx next to
            // the item, the item left with an EXTERNAL reference to it (ancestor 0, the
            // bare file name): the positive control of the 2026-10-17 pack-reference
            // run — an external entity model the loader CAN reach (same archive folder).
            let inp = a.rest.get(1).unwrap_or_else(|| die("split-prefab IN.Item.Gbx --out-dir D --name NAME".into()));
            let dir = flag(&a.rest, "--out-dir").unwrap_or_else(|| ".".into());
            let name = flag(&a.rest, "--name").unwrap_or_else(|| die("--name NAME".into()));
            let bytes = std::fs::read(inp).unwrap_or_else(|e| die(format!("{inp}: {e}")));
            let mut f = mapgeom::static_item::file::parse_file(&bytes).unwrap_or_else(|e| die(format!("{inp}: {e}")));
            let num_nodes = f.num_nodes;
            let mc = f.item.model_mut().unwrap_or_else(|| die("no model chunk".into()));
            let slot = if mc.entity_model.inline.is_some() { &mut mc.entity_model } else { &mut mc.entity_model_edition };
            let node = slot.inline.take().unwrap_or_else(|| die("the entity model is not inline (already external or null)".into()));
            let idx = slot.index;
            let class_id = node.class_id();
            let mut body = Vec::new();
            let mut lb = mapgeom::static_item::LookbackState::default();
            {
                let mut w = mapgeom::static_item::Wr { w: &mut body, lb: &mut lb };
                mapgeom::static_item::write_node(&mut w, &node);
            }
            let prefab_name = format!("{name}.Prefab.Gbx");
            let prefab_file = mapgeom::static_item::file::write_node_file(class_id, &body, num_nodes, &[]);
            std::fs::create_dir_all(&dir).expect("out dir");
            std::fs::write(std::path::Path::new(&dir).join(&prefab_name), &prefab_file).expect("write prefab");
            f.ref_table = mapgeom::static_item::file::ref_table(&[(idx as u32, prefab_name.clone())]);
            let item_bytes = mapgeom::static_item::file::write_file(&f);
            let ident = format!("{name}.Item.Gbx");
            let (from, _) = tmmaps::header::item_ident_author(&item_bytes).unwrap_or_else(|| die("no ident in the header".into()));
            let item_bytes = mapgeom::crystal::rename_ident(&item_bytes, &from, &ident);
            std::fs::write(std::path::Path::new(&dir).join(&ident), &item_bytes).expect("write item");
            println!("wrote {dir}/{ident} ({} bytes; entity model node {idx} external -> {prefab_name}) and {dir}/{prefab_name} ({} bytes, class 0x{class_id:08X}, {num_nodes} node slots)", item_bytes.len(), prefab_file.len());
            print!("{}", mapgeom::reftable::describe(&mapgeom::reftable::table_of(&item_bytes)));
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
                        println!("{p}: {} levels [{}] switch {:?} far {} ; {} materials ({}); hull {} verts {} tris; bottom {:.2} top {:.2} (height {:.2}) radius {:.2} m; instance variation: scale 1-(k/7)*{} tilt +-{} deg random yaw {}", m.lods.len(), lods.join(" | "), m.switch, m.far, m.materials.len(), m.materials.iter().map(|mt| format!("{}{}", mt.name, if mt.leaf { "*" } else { "" })).collect::<Vec<_>>().join(", "), m.hull_vertices.len(), m.hull_triangles.len(), s.bottom, s.top, s.top - s.bottom, s.radius, m.scale_var01, m.angle_max_rot_xz_deg, m.enable_random_rotation_y);
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
            let p0 = a.rest.get(1).cloned().unwrap_or_default();
            let mut store = if std::path::Path::new(&p0).is_file() && a.paks.is_empty() { DataStore::empty() } else { open(&a) };
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
        // skin-inspect <MainBody.Mesh.gbx>: a car SKIN mesh (a bare CPlugSolid2Model
        // whose materials are NAMES the vehicle vis model resolves) laid out for
        // the skin-scenery study — the joints, every visual's shading material,
        // lod mask, skin data (bones), vertex declarations with their element
        // names/types/spaces/offsets and the value ranges of each element.
        "skin-inspect" => {
            let p = a.rest.get(1).cloned().unwrap_or_default();
            // A skin mesh on disk needs no pack: its materials are NAMES. Only
            // fall back to the store when the argument is a pack path.
            let mut store = if std::path::Path::new(&p).is_file() && a.paks.is_empty() {
                DataStore::empty()
            } else {
                open(&a)
            };
            let m = load_any(&mut store, &p);
            let mut lb = mapgeom::static_item::LookbackState::default();
            lb.defined_nodes.extend(m.external_indices().iter().copied());
            let mut r = mapgeom::static_item::Rd::new(&m.body, 0, lb);
            let s2 = mapgeom::static_item::solid2::CPlugSolid2Model::parse(&mut r).unwrap_or_else(|e| die(format!("{p}: {e}")));
            println!("{p}: solid2 v{} chunks {:x?} damage_zone {} flags {:#x} u05 {} u07 {} vis_cst_type {} lod_max_dist {:?} u13 {} u15 {} u16 {}", s2.version, s2.chunks, s2.damage_zone, s2.flags, s2.u05, s2.u07, s2.vis_cst_type, s2.lod_max_dist, s2.u13, s2.u15, s2.u16);
            println!("  skel ref index {} inline {}", s2.skel.index, s2.skel.inline.is_some());
            if let Some(node) = s2.skel.inline.as_deref() {
                if let mapgeom::static_item::Node::Skel(sk) = node {
                    println!("  skel v{} name {:?}: {} joints, {} sockets, joints_lods {:?}, rotation_order {:?}, u10 {:?}, v20_word {}, c_lod {}, lod_max_dists {:?}, u03 {} u04 {} u05 {:?} raw chunks {:?}",
                        sk.version, sk.name, sk.joints.len(), sk.sockets.len(), sk.joints_lods, sk.rotation_order, sk.u10, sk.v20_word, sk.c_lod, sk.lod_max_dists,
                        sk.u03.is_some(), sk.u04.is_some(), sk.u05, sk.raw.iter().map(|c| format!("{:#x}:{}", c.id, c.payload.len())).collect::<Vec<_>>());
                    for (i, j) in sk.joints.iter().enumerate() {
                        let g = &j.global_loc;
                        println!("    joint {i:2} {:<22} parent {:3} pos ({:.3}, {:.3}, {:.3}) rot [{:.2} {:.2} {:.2} | {:.2} {:.2} {:.2} | {:.2} {:.2} {:.2}]{}",
                            j.name.as_str().unwrap_or("?"), j.parent, g[9], g[10], g[11], g[0], g[1], g[2], g[3], g[4], g[5], g[6], g[7], g[8],
                            j.old.map(|(q, p)| format!(" old q{q:?} p{p:?}")).unwrap_or_default());
                    }
                    for (i, s) in sk.sockets.iter().enumerate() {
                        let g = &s.loc;
                        println!("    socket {i:2} {:<22} joint {:3} pos ({:.3}, {:.3}, {:.3})", s.name.as_str().unwrap_or("?"), s.linked_joint, g[9], g[10], g[11]);
                    }
                    if let Some(u) = &sk.u03 { println!("    u03: a {} b {} c {} d {} e {}", u.a.len(), u.b.len(), u.c.len(), u.d, u.e); }
                    if let Some(u) = &sk.u04 { println!("    u04: names {:?} a {:?} b {:?} quats {}", u.names.iter().map(|i| i.as_str().unwrap_or("?").to_string()).collect::<Vec<_>>(), u.a, u.b, u.quats.len()); }
                } else {
                    println!("  skel inline node is not a CPlugSkel: class {:#x}", node.class_id());
                }
            }
            println!("  material_ids ({}): {:?}", s2.material_ids.len(), s2.material_ids.iter().map(|i| format!("{i:?}")).collect::<Vec<_>>());
            println!("  custom_materials {} materials(refs) {} materials_folder {:?}", s2.custom_materials.len(), s2.materials.len(), s2.materials_folder);
            println!("  joints ({}): {:?}", s2.joints.len(), s2.joints.iter().map(|i| i.as_str().unwrap_or("?").to_string()).collect::<Vec<_>>());
            println!("  u10 {:?} u12 {:?} u19 {:?}", s2.u10, s2.u12, s2.u19);
            for rc in &s2.raw {
                let head: Vec<String> = rc.payload.iter().take(64).map(|b| format!("{b:02x}")).collect();
                println!("  raw chunk {:#x} ({} bytes): {}", rc.id, rc.payload.len(), head.join(" "));
            }
            let names = |n: u32| -> &'static str {
                match n {
                    0 => "Position",
                    1 => "BlendWeight",
                    2 => "BlendIndices",
                    5 => "Normal",
                    8 => "Color0",
                    9 => "Color1",
                    10 => "TexCoord0",
                    11 => "TexCoord1",
                    12 => "TexCoord2",
                    18 => "TangentU",
                    20 => "TangentV",
                    _ => "?",
                }
            };
            let tys = |t: u32| -> &'static str {
                match t {
                    0 => "Float1",
                    1 => "Float2",
                    2 => "Float3",
                    3 => "Float4",
                    4 => "Color",
                    5 => "Int32",
                    14 => "Dec3N",
                    _ => "?",
                }
            };
            for (gi, g) in s2.shaded_geoms.iter().enumerate() {
                let mat = s2.material_ids.get(g.material_index as usize).and_then(|i| i.as_str()).unwrap_or("?");
                let Some(vr) = s2.visuals.get(g.visual_index as usize) else { continue };
                let Some(mapgeom::static_item::Node::Visual(v)) = vr.inline.as_deref() else {
                    println!("  geom {gi}: visual {} not inline", g.visual_index);
                    continue;
                };
                let main = v.main.as_ref();
                let nidx = v.index_buffer.as_ref().map(|b| b.indices.len()).unwrap_or(0);
                println!(
                    "  geom {gi}: visual {} material {} ({}) lod_mask {} u01 {} u02 {} | {} verts {} indices ({} tris) chunk_flags {:#x} bbox {:?}",
                    g.visual_index, g.material_index, mat, g.lod_mask, g.u01, g.u02,
                    main.map(|m| m.count).unwrap_or(0), nidx, nidx / 3,
                    main.map(|m| m.chunk_flags).unwrap_or(0),
                    main.map(|m| m.bounding_box).unwrap_or([0.0; 6])
                );
                if let Some(mn) = main {
                    if let Some(sk) = &mn.skin {
                        println!("      skin data: u01 {} u02 {} u03 {} u04 {} bones ({}) {:?} u07 {:?}", sk.u01, sk.u02, sk.u03, sk.u04, sk.bones.len(), sk.bones.iter().map(|b| b.as_str().unwrap_or("?").to_string()).collect::<Vec<_>>(), sk.u07);
                    } else {
                        println!("      skin data: none");
                    }
                    println!("      tex_coord_sets {} uv_groups {} bitmap_elems {} u02 {} u03 {} u04 {:?}", mn.tex_coord_sets.len(), mn.uv_groups.len(), mn.bitmap_elems.len(), mn.u02, mn.u03, mn.u04);
                    for tc in &mn.tex_coord_sets {
                        let (mut lo, mut hi) = ([f32::MAX; 2], [f32::MIN; 2]);
                        for (uv, _, _) in &tc.coords {
                            for k in 0..2 {
                                lo[k] = lo[k].min(uv[k]);
                                hi[k] = hi[k].max(uv[k]);
                            }
                        }
                        println!("      texcoords v{} flags {:?} n {} u range {:.3}..{:.3} v range {:.3}..{:.3}", tc.version, tc.flags, tc.coords.len(), lo[0], hi[0], lo[1], hi[1]);
                    }
                    for sr in &mn.vertex_streams {
                        let Some(mapgeom::static_item::Node::VertexStream(s)) = sr.inline.as_deref() else {
                            println!("      stream: not inline (index {})", sr.index);
                            continue;
                        };
                        println!("      stream v{} count {} flags {:#x} compress_local3d {:?} decls {}", s.version, s.count, s.flags, s.compress_local3d, s.decls.len());
                        for (di, d) in s.decls.iter().enumerate() {
                            let stride = (d.flags1 >> 20) & 0xFF;
                            let mut range = String::new();
                            if let Some(e) = s.elems.get(di) {
                                match e {
                                    mapgeom::static_item::vstream::Elem::Float3(v) => {
                                        let (mut lo, mut hi) = ([f32::MAX; 3], [f32::MIN; 3]);
                                        for x in v {
                                            for k in 0..3 {
                                                lo[k] = lo[k].min(x[k]);
                                                hi[k] = hi[k].max(x[k]);
                                            }
                                        }
                                        range = format!("range x {:.3}..{:.3} y {:.3}..{:.3} z {:.3}..{:.3} first {:?}", lo[0], hi[0], lo[1], hi[1], lo[2], hi[2], v.first());
                                    }
                                    mapgeom::static_item::vstream::Elem::Float2(v) => {
                                        let (mut lo, mut hi) = ([f32::MAX; 2], [f32::MIN; 2]);
                                        for x in v {
                                            for k in 0..2 {
                                                lo[k] = lo[k].min(x[k]);
                                                hi[k] = hi[k].max(x[k]);
                                            }
                                        }
                                        range = format!("range u {:.3}..{:.3} v {:.3}..{:.3} first {:?}", lo[0], hi[0], lo[1], hi[1], v.first());
                                    }
                                    mapgeom::static_item::vstream::Elem::Float4(v) => {
                                        range = format!("first {:?} second {:?}", v.first(), v.get(1));
                                    }
                                    mapgeom::static_item::vstream::Elem::Word(v) => {
                                        let mut distinct: Vec<u32> = v.clone();
                                        distinct.sort_unstable();
                                        distinct.dedup();
                                        let show: Vec<String> = distinct.iter().take(8).map(|x| format!("{x:#010x}")).collect();
                                        range = format!("{} distinct words, e.g. {}", distinct.len(), show.join(" "));
                                    }
                                    mapgeom::static_item::vstream::Elem::Raw { size, bytes } => {
                                        range = format!("raw {} bytes/elem, first {:02x?}", size, &bytes[..(*size).min(bytes.len())]);
                                    }
                                }
                            }
                            println!(
                                "        decl {di}: name {} ({}) type {} ({}) space {} offset {} stride_words {} flags1 {:#010x} flags2 {:#010x} extra {:?} {}",
                                d.name(), names(d.name()), d.ty(), tys(d.ty()), d.space(), d.offset(), stride, d.flags1, d.flags2, d.extra, range
                            );
                        }
                    }
                }
                if let Some((tu, tv)) = &v.tangents {
                    println!("      inline tangents: {} + {} bytes", tu.len(), tv.len());
                }
                if let Some(ib) = &v.index_buffer {
                    let mx = ib.indices.iter().copied().max().unwrap_or(0);
                    println!("      index buffer chunk {:#x} flags {:#x} max index {}", ib.chunk, ib.flags, mx);
                }
            }
        }
        // skel-roundtrip FILE.Skel.Gbx: parse a CPlugSkel file body with skel.rs and write it
        // back — the byte check behind inlining a skeleton into a skin mesh.
        "skel-roundtrip" => {
            let mut store = open(&a);
            let p = a.rest.get(1).cloned().unwrap_or_default();
            let m = load_any(&mut store, &p);
            let mut r = mapgeom::static_item::Rd::new(&m.body, 0, mapgeom::static_item::LookbackState::default());
            let sk = mapgeom::static_item::skel::CPlugSkel::parse(&mut r).unwrap_or_else(|e| die(format!("{p}: {e}")));
            let consumed = r.o;
            let mut out = Vec::new();
            {
                let mut lb = mapgeom::static_item::LookbackState::default();
                let mut w = mapgeom::static_item::Wr { w: &mut out, lb: &mut lb };
                sk.write(&mut w);
            }
            let same = out == m.body;
            println!("{p}: skel v{} name {:?} {} joints, {} sockets, u03 {} u04 {} joints_lods {} rotation_order {} u10 {} c_lod {} lod_max_dists {:?}; body {} B ({consumed} consumed), rewritten {} B, {}", sk.version, sk.name, sk.joints.len(), sk.sockets.len(), sk.u03.is_some(), sk.u04.is_some(), sk.joints_lods.len(), sk.rotation_order.len(), sk.u10.len(), sk.c_lod, sk.lod_max_dists, m.body.len(), out.len(), if same { "IDENTICAL" } else { "DIFFERENT" });
            println!("  joints: {:?}", sk.joint_names());
            println!("  sockets: {:?}", sk.sockets.iter().map(|s| format!("{}->{}", s.name.as_str().unwrap_or("?"), s.linked_joint)).collect::<Vec<_>>());
        }
        // skin-inline-skel MESH.Mesh.gbx --skel FILE.Skel.Gbx --out F: the stock pak mesh
        // with its skeleton carried inline (the shape `skinfix.py` leaves NadeoImporter
        // output in) — the reference test for the zip loader.
        "skin-inline-skel" => {
            let mut store = open(&a);
            let p = a.rest.get(1).cloned().unwrap_or_default();
            let skp = flag(&a.rest, "--skel").unwrap_or_else(|| die("--skel FILE.Skel.Gbx".into()));
            let out = flag(&a.rest, "--out").unwrap_or_else(|| die("--out F".into()));
            let m = load_any(&mut store, &p);
            let mut lb = mapgeom::static_item::LookbackState::default();
            lb.defined_nodes.extend(m.external_indices().iter().copied());
            let mut r = mapgeom::static_item::Rd::new(&m.body, 0, lb);
            let mut s2 = mapgeom::static_item::solid2::CPlugSolid2Model::parse(&mut r).unwrap_or_else(|e| die(format!("{p}: {e}")));
            let sm = load_any(&mut store, &skp);
            let mut r2 = mapgeom::static_item::Rd::new(&sm.body, 0, mapgeom::static_item::LookbackState::default());
            let sk = mapgeom::static_item::skel::CPlugSkel::parse(&mut r2).unwrap_or_else(|e| die(format!("{skp}: {e}")));
            // node indices: root 0, visuals + streams as written, then the skel
            let mut max_idx: i32 = 0;
            for v in &s2.visuals {
                max_idx = max_idx.max(v.index);
                if let Some(mapgeom::static_item::Node::Visual(vis)) = v.inline.as_deref() {
                    if let Some(mn) = &vis.main {
                        for s in &mn.vertex_streams { max_idx = max_idx.max(s.index); }
                    }
                }
            }
            s2.skel = mapgeom::static_item::NodeRef { index: max_idx + 1, inline: Some(Box::new(mapgeom::static_item::Node::Skel(sk.clone()))) };
            let mut body = Vec::new();
            {
                let mut lbw = mapgeom::static_item::LookbackState::default();
                let mut w = mapgeom::static_item::Wr { w: &mut body, lb: &mut lbw };
                s2.write(&mut w);
            }
            let file = mapgeom::static_item::file::write_node_file(0x090BB000, &body, (max_idx + 2) as u32, &[]);
            std::fs::write(&out, &file).unwrap_or_else(|e| die(e.to_string()));
            println!("wrote {out}: {} bytes, skel inline at node {} ({} joints: {:?}), solid2 joints {:?}", file.len(), max_idx + 1, sk.joints.len(), &sk.joint_names()[..sk.joints.len().min(6)], s2.joints.iter().map(|j| j.as_str().unwrap_or("?")).collect::<Vec<_>>());
        }
        // skin-fbx --out DIR --name NAME (--cube SIZE | --map … as skin-build): the FBX +
        // MeshParams.xml for the NadeoImporter route (MeshType Vehicle, one Body bone).
        "skin-fbx" => {
            use mapgeom::static_item::skin;
            let mut store = open(&a);
            let out_dir = std::path::PathBuf::from(flag(&a.rest, "--out").unwrap_or_else(|| die("--out DIR".into())));
            let name = flag(&a.rest, "--name").unwrap_or_else(|| "MainBody".to_string());
            let texset_default = flag(&a.rest, "--texset").unwrap_or_else(|| "Details".to_string());
            let family = flag(&a.rest, "--family").unwrap_or_else(|| "DetailsDmgNormal".to_string());
            let mut parts: Vec<skin::Part> = Vec::new();
            if let Some(sz) = flag(&a.rest, "--cube") {
                let size: f32 = sz.parse().unwrap_or_else(|_| die("--cube SIZE".into()));
                parts.push(skin::cube(size, &texset_default));
            }
            if let Some(map_path) = flag(&a.rest, "--map") {
                let anchor: Vec<f32> = flag(&a.rest, "--anchor").unwrap_or_else(|| die("--anchor X,Y,Z".into())).split(',').filter_map(|s| s.trim().parse().ok()).collect();
                if anchor.len() != 3 { die::<()>("--anchor X,Y,Z".into()); }
                let anchor = [anchor[0], anchor[1], anchor[2]];
                let want: Vec<usize> = flag(&a.rest, "--items").unwrap_or_default().split(',').filter_map(|s| s.trim().trim_start_matches('i').parse().ok()).collect();
                let texmap: Vec<(String, String)> = flag(&a.rest, "--texmap").unwrap_or_default().split(',').filter(|s| !s.is_empty()).map(|kv| { let (k, v) = kv.split_once('=').unwrap_or_else(|| die(format!("--texmap {kv}: LINK=SET"))); (k.to_string(), v.to_string()) }).collect();
                let strict = a.rest.iter().any(|x| x == "--texmap-strict");
                let planar: Option<f32> = flag(&a.rest, "--planar-uv").map(|s| s.parse().unwrap_or_else(|_| die("--planar-uv METRES".into())));
                let only: Vec<String> = flag(&a.rest, "--planar-sets").map(|s| s.split(',').map(|x| x.trim().to_string()).collect()).unwrap_or_default();
                let m = tmmaps::map::MapFile::load(std::path::Path::new(&map_path));
                let files = mapgeom::embedded::files(&m).unwrap_or_else(die);
                let by_key: std::collections::BTreeMap<String, Vec<u8>> = files.into_iter().map(|(k, v)| (k.replace('\\', "/").to_lowercase(), v)).collect();
                for it in &m.items {
                    if !want.is_empty() && !want.contains(&it.index) { continue; }
                    let key = it.model.replace('\\', "/").to_lowercase();
                    let Some(bytes) = by_key.get(&key).or_else(|| by_key.iter().find(|(k, _)| k.ends_with(&format!("/{key}"))).map(|(_, v)| v)) else { continue };
                    let xf = mapgeom::place::anchored(it.pos, [it.yaw, it.pitch, it.roll], it.pivot, it.scale);
                    let material_of = |link: &str| -> Option<String> {
                        for (k, v) in &texmap { if link == k || link.ends_with(&format!("\\{k}")) || link.contains(k.as_str()) { return if v == "-" { None } else { Some(v.clone()) }; } }
                        if strict { None } else { Some(texset_default.clone()) }
                    };
                    if let Ok(ps) = skin::item_parts(bytes, &it.model, &material_of) {
                        for (_, p) in ps {
                            let mut placed = skin::place(&p, &xf, anchor);
                            if let Some(mpt) = planar { if only.is_empty() || only.contains(&placed.texset) { skin::planar_uv(&mut placed, anchor, mpt); } }
                            parts.push(placed);
                        }
                    }
                }
            }
            if parts.is_empty() { die::<()>("nothing to export".into()); }
            let material_name = |set: &str| -> String { format!("{family}_{set}") };
            let text = mapgeom::static_item::fbx::fbx(&parts, &material_name, &name);
            std::fs::create_dir_all(&out_dir).unwrap_or_else(|e| die(e.to_string()));
            let fbx_path = out_dir.join(format!("{name}.fbx"));
            std::fs::write(&fbx_path, &text).unwrap_or_else(|e| die(e.to_string()));
            let mut mats: Vec<String> = Vec::new();
            for p in &parts { let mn = material_name(&p.texset); if !mats.contains(&mn) { mats.push(mn); } }
            let xml = mapgeom::static_item::fbx::mesh_params(&format!("{name}.fbx"), &mats);
            std::fs::write(out_dir.join(format!("{name}.MeshParams.xml")), &xml).unwrap_or_else(|e| die(e.to_string()));
            let nv: usize = parts.iter().map(|p| p.pos.len()).sum();
            let nt: usize = parts.iter().map(|p| p.idx.len() / 3).sum();
            println!("wrote {} ({} bytes) + {name}.MeshParams.xml: {nv} vertices, {nt} triangles, materials {:?}", fbx_path.display(), text.len(), mats);
        }
        // skin-build --template MainBody.Mesh.gbx --out DIR [--zip F.zip]
        //    (--cube SIZE | --map MAP --items i1,i2,… --anchor X,Y,Z [--texmap LINK=SET,…] [--planar-uv METRES])
        //    [--binding noskel|body] [--family Details|Skin|Glass] [--vis-flags HEX] [--no-skin-data]
        //    [--lod-max N,…] [--max-verts N] [--tex FILE.dds] [--checker N]
        // A CarSport 3D skin (MainBody.Mesh.gbx + <Set>_{B,N,R,I,AO,DirtMask}.dds) from
        // a probe cube or from embedded items of a map placed in the world and
        // re-expressed in the frame of a ghost parked at --anchor (static_item::skin).
        "skin-build" => {
            use mapgeom::static_item::skin;
            // no pack needed unless one is named: the template is a file, textures come
            // from the map, a --tex file or a checker
            let mut store = if a.paks.is_empty() { DataStore::empty() } else { open(&a) };
            let tpl_path = flag(&a.rest, "--template").unwrap_or_else(|| die("--template <stock MainBody.Mesh.gbx>".into()));
            let out_dir = std::path::PathBuf::from(flag(&a.rest, "--out").unwrap_or_else(|| die("--out DIR".into())));
            let tpl_model = load_any(&mut store, &tpl_path);
            let template = skin::template_from_body(&tpl_model.body).unwrap_or_else(|e| die(format!("{tpl_path}: {e}")));
            let mut o = skin::Options::default();
            if let Some(b) = flag(&a.rest, "--binding") {
                o.binding = match b.as_str() { "noskel" => skin::Binding::NoSkel, "body" => skin::Binding::Body, "template" => skin::Binding::Template, x => die(format!("--binding {x}: noskel|body|template")) };
            }
            if let Some(f) = flag(&a.rest, "--family") { o.family = f; }
            if let Some(v) = flag(&a.rest, "--vis-flags") { o.vis_flags = u32::from_str_radix(v.trim_start_matches("0x"), 16).unwrap_or_else(|_| die("--vis-flags HEX".into())); }
            if a.rest.iter().any(|x| x == "--no-skin-data") { o.skin_data = false; }
            if a.rest.iter().any(|x| x == "--prune-materials") { o.prune_materials = true; }
            if let Some(m) = flag(&a.rest, "--matname") { o.material_override = m.split(',').filter_map(|kv| kv.split_once('=').map(|(k, v)| (k.to_string(), v.to_string()))).collect(); }
            if let Some(l) = flag(&a.rest, "--lod-max") { o.lod_max_dist = l.split(',').filter_map(|s| s.trim().parse().ok()).collect(); }
            if let Some(m) = flag(&a.rest, "--max-verts") { o.max_verts = m.parse().unwrap_or_else(|_| die("--max-verts N".into())); }
            if let Some(s) = flag(&a.rest, "--skel") {
                o.skel = Some(if s == "body" { mapgeom::static_item::skel::CPlugSkel::body_only() } else {
                    let sm = load_any(&mut store, &s);
                    let mut r2 = mapgeom::static_item::Rd::new(&sm.body, 0, mapgeom::static_item::LookbackState::default());
                    mapgeom::static_item::skel::CPlugSkel::parse(&mut r2).unwrap_or_else(|e| die(format!("{s}: {e}")))
                });
            }
            let texset_default = flag(&a.rest, "--texset").unwrap_or_else(|| "Details".to_string());
            let mut parts: Vec<skin::Part> = Vec::new();
            let mut report: Vec<String> = Vec::new();
            // the parked ghost's world position: every vertex is written relative to it
            let anchor_world: [f32; 3] = match flag(&a.rest, "--anchor") {
                Some(s) => { let v: Vec<f32> = s.split(',').filter_map(|x| x.trim().parse().ok()).collect(); if v.len() == 3 { [v[0], v[1], v[2]] } else { [0.0; 3] } }
                None => [0.0; 3],
            };
            if let Some(sz) = flag(&a.rest, "--cube") {
                let size: f32 = sz.parse().unwrap_or_else(|_| die("--cube SIZE".into()));
                parts.push(skin::cube(size, &texset_default));
                report.push(format!("probe cube {size} m on y=0, one texture tile per face"));
            }
            // --cubes "SIZE@X,Y,Z;SIZE@X,Y,Z…": several probe cubes at skin-frame offsets — the
            // axis probe (which way is the car's +x/+z in a frame) of 2026-09-12
            if let Some(spec) = flag(&a.rest, "--cubes") {
                for item in spec.split(';').filter(|s| !s.trim().is_empty()) {
                    let (sz, at) = item.split_once('@').unwrap_or_else(|| die(format!("--cubes {item}: SIZE@X,Y,Z")));
                    let size: f32 = sz.trim().parse().unwrap_or_else(|_| die(format!("--cubes {item}: SIZE@X,Y,Z")));
                    let o: Vec<f32> = at.split(',').filter_map(|s| s.trim().parse().ok()).collect();
                    if o.len() != 3 { die::<()>(format!("--cubes {item}: SIZE@X,Y,Z")); }
                    let mut c = skin::cube(size, &texset_default);
                    for p in c.pos.iter_mut() { p[0] += o[0]; p[1] += o[1]; p[2] += o[2]; }
                    c.source = format!("cube {size} m at ({}, {}, {})", o[0], o[1], o[2]);
                    parts.push(c);
                    report.push(format!("probe cube {size} m at skin-frame ({}, {}, {})", o[0], o[1], o[2]));
                }
            }
            if let Some(map_path) = flag(&a.rest, "--map") {
                let anchor: Vec<f32> = flag(&a.rest, "--anchor").unwrap_or_else(|| die("--anchor X,Y,Z (the parked ghost's position)".into())).split(',').filter_map(|s| s.trim().parse().ok()).collect();
                if anchor.len() != 3 { die::<()>("--anchor X,Y,Z".into()); }
                let anchor = [anchor[0], anchor[1], anchor[2]];
                let want: Vec<usize> = flag(&a.rest, "--items").unwrap_or_default().split(',').filter_map(|s| s.trim().trim_start_matches('i').parse().ok()).collect();
                // LINK=SET pairs; a link not listed goes to the default set unless --texmap-strict
                let texmap: Vec<(String, String)> = flag(&a.rest, "--texmap").unwrap_or_default().split(',').filter(|s| !s.is_empty()).map(|kv| {
                    let (k, v) = kv.split_once('=').unwrap_or_else(|| die(format!("--texmap {kv}: LINK=SET")));
                    (k.to_string(), v.to_string())
                }).collect();
                let strict = a.rest.iter().any(|x| x == "--texmap-strict");
                let atlas = a.rest.iter().any(|x| x == "--atlas");
                let planar: Option<f32> = flag(&a.rest, "--planar-uv").map(|s| s.parse().unwrap_or_else(|_| die("--planar-uv METRES".into())));
                let m = tmmaps::map::MapFile::load(std::path::Path::new(&map_path));
                let files = mapgeom::embedded::files(&m).unwrap_or_else(die);
                let by_key: std::collections::BTreeMap<String, Vec<u8>> = files.into_iter().map(|(k, v)| (k.replace('\\', "/").to_lowercase(), v)).collect();
                let mut links_seen: std::collections::BTreeMap<String, usize> = std::collections::BTreeMap::new();
                // A campaign map places STOCK items (`PalmTreeMedium.Item.Gbx`) it does not
                // embed: their bytes come from the packs (or from --models NAME=PATH), so a
                // Nadeo map's own scenery can be rebuilt as skin geometry. --name-filter
                // SUBSTR,… keeps only the models whose name matches (2026-09-13: the trees of
                // Summer 2025 - 01, carried by parked ghosts that swap at every checkpoint).
                let models: Vec<(String, String)> = flag(&a.rest, "--models").unwrap_or_default().split(',').filter(|s| !s.is_empty()).map(|kv| {
                    let (k, v) = kv.split_once('=').unwrap_or_else(|| die(format!("--models {kv}: NAME=PATH")));
                    (k.to_lowercase(), v.to_string())
                }).collect();
                // --mapping TSV --items-dir DIR: the tiny-library contract (i@<index> -> item
                // file), so a campaign map's VEGETATION — baked from its prefabs into static
                // items — can be rebuilt as skin geometry (2026-09-13, the scenery swap).
                let mapping: std::collections::BTreeMap<usize, String> = match flag(&a.rest, "--mapping") {
                    None => Default::default(),
                    Some(p) => std::fs::read_to_string(&p).unwrap_or_else(|e| die(format!("{p}: {e}"))).lines()
                        .filter_map(|l| { let mut it = l.split('\t'); let k = it.next()?; let v = it.next()?; let idx: usize = k.strip_prefix("i@")?.parse().ok()?; if v == "-" { None } else { Some((idx, v.to_string())) } })
                        .collect(),
                };
                let items_dir = flag(&a.rest, "--items-dir").unwrap_or_else(|| ".".to_string());
                let name_filter: Vec<String> = flag(&a.rest, "--name-filter").unwrap_or_default().split(',').filter(|s| !s.is_empty()).map(|s| s.to_lowercase()).collect();
                let mut stock_cache: std::collections::BTreeMap<String, Vec<u8>> = std::collections::BTreeMap::new();
                let mut missing: std::collections::BTreeMap<String, usize> = std::collections::BTreeMap::new();
                for it in &m.items {
                    if !want.is_empty() && !want.contains(&it.index) { continue; }
                    let key = it.model.replace('\\', "/").to_lowercase();
                    if !name_filter.is_empty() && !name_filter.iter().any(|f| key.contains(f.as_str())) { continue; }
                    // --near X,Z,R: only the placements within R metres of (X, Z). One
                    // checkpoint interval's scenery, so the skin stays close to its ghost.
                    // items parked far below the map (a sunk item, `tmmaps sinkitems` / `tiny`)
                    // carry nothing worth a skin: skip them (--skip-below Y, default -500)
                    let skip_below: f32 = flag(&a.rest, "--skip-below").and_then(|s| s.parse().ok()).unwrap_or(-500.0);
                    if it.pos[1] < skip_below { continue; }
                    if let Some(spec) = flag(&a.rest, "--near") {
                        let v: Vec<f32> = spec.split(',').filter_map(|s| s.trim().parse().ok()).collect();
                        if v.len() != 3 { die::<()>("--near X,Z,RADIUS".into()); }
                        let d = ((it.pos[0] - v[0]).powi(2) + (it.pos[2] - v[1]).powi(2)).sqrt();
                        if d > v[2] { continue; }
                    }
                    let mapped: Option<Vec<u8>> = mapping.get(&it.index).and_then(|f| {
                        let p = format!("{items_dir}/{f}");
                        match std::fs::read(&p) { Ok(b) => Some(b), Err(_) => { *missing.entry(p).or_insert(0) += 1; None } }
                    });
                    let embedded = mapped.or_else(|| by_key.get(&key).or_else(|| by_key.iter().find(|(k, _)| k.ends_with(&format!("/{key}"))).map(|(_, v)| v)).cloned());
                    let owned: Vec<u8> = match embedded {
                        Some(b) => b,
                        None => {
                            let stem = key.rsplit('/').next().unwrap_or(&key).to_string();
                            if let Some(b) = stock_cache.get(&stem) {
                                b.clone()
                            } else {
                                let from_flag = models.iter().find(|(n, _)| stem.starts_with(n.as_str())).map(|(_, p)| p.clone());
                                // a stock item record names the model WITHOUT its pack folder
                                // ("PalmTreeMedium"): find the pack entry whose file name is
                                // that name with .Item.Gbx, case-insensitively.
                                let want1 = format!("/{}", stem.trim_end_matches(".item.gbx"));
                                let want2 = format!("{want1}.item.gbx");
                                let in_pack: Option<String> = store.entries().map(|e| e.path().to_string()).find(|p| {
                                    let lp = p.replace('\\', "/").to_lowercase();
                                    lp.ends_with(&want2) || lp.ends_with(&want1)
                                });
                                let loaded = match from_flag {
                                    Some(p) if std::path::Path::new(&p).is_file() => std::fs::read(&p).ok(),
                                    // store.read gives an Arc<Vec<u8>> (the pak
                                    // store caches shared buffers); to_vec to
                                    // match the on-disk arm, as elsewhere here.
                                    Some(p) => store.read(&p).ok().map(|b| b.to_vec()),
                                    None => match store.read(&it.model).ok().map(|b| b.to_vec()) {
                                        Some(b) => Some(b),
                                        None => in_pack.and_then(|p| store.read(&p).ok().map(|b| b.to_vec())),
                                    },
                                };
                                match loaded {
                                    Some(b) => { stock_cache.insert(stem.clone(), b.clone()); b }
                                    None => { *missing.entry(it.model.clone()).or_insert(0) += 1; continue; }
                                }
                            }
                        }
                    };
                    let bytes = &owned[..];
                    let xf = mapgeom::place::anchored(it.pos, [it.yaw, it.pitch, it.roll], it.pivot, it.scale);
                    let material_of = |link: &str| -> Option<String> {
                        // --atlas: keep the material LINK as the part's set; the atlas step
                        // below packs every link's texture into one Details_B and remaps uvs
                        if atlas { return Some(link.to_string()); }
                        for (k, v) in &texmap {
                            if link == k || link.ends_with(&format!("\\{k}")) || link.contains(k.as_str()) { return if v == "-" { None } else { Some(v.clone()) }; }
                        }
                        if strict { None } else { Some(texset_default.clone()) }
                    };
                    match skin::item_parts(bytes, &it.model, &material_of) {
                        Ok(ps) => {
                            for (link, p) in ps {
                                *links_seen.entry(link).or_insert(0) += p.pos.len();
                                let mut placed = skin::place(&p, &xf, anchor);
                                if let Some(mpt) = planar {
                                    // --planar-sets A,B limits the projection to those texture sets (foliage keeps its atlas UVs)
                                    let only: Vec<String> = flag(&a.rest, "--planar-sets").map(|s| s.split(',').map(|x| x.trim().to_string()).collect()).unwrap_or_default();
                                    if only.is_empty() || only.contains(&placed.texset) { skin::planar_uv(&mut placed, anchor, mpt); }
                                }
                                placed.source = format!("i{} {}", it.index, placed.source);
                                parts.push(placed);
                            }
                        }
                        Err(e) => report.push(format!("i{}: {e}", it.index)),
                    }
                }
                for (l, n) in &links_seen { report.push(format!("material {l}: {n} vertices")); }
                for (mdl, n) in &missing { report.push(format!("{mdl}: {n} placement(s) — model not embedded and not in the packs")); }
            }
            if parts.is_empty() {
                for line in &report { eprintln!("  {line}"); }
                die::<()>("nothing to build: --cube SIZE or --map … --items …".into());
            }
            // --atlas: every material of the placed items packed into ONE texture on the default
            // set (the shape proven to render: one material id, Details geometry, Deflate zip).
            // Each distinct material link gets a 512x512 cell of a grid; uvs are wrapped into
            // the cell. --season ice|fall recolours the leaf cells (links containing "Branch",
            // "Leaf" or "Foliage") and lightly frosts the rest for ice. Textures come from
            // --items-dir (`<Name>.dds`, `<Name>_D.dds`, a TDOSN_/TDSN_ prefix stripped);
            // a link without a file gets a flat colour. 2026-09-14: the real palms.
            let mut atlas_tex: Option<(u32, u32, Vec<u8>)> = None;
            if a.rest.iter().any(|x| x == "--atlas") {
                let items_dir = flag(&a.rest, "--items-dir").unwrap_or_else(|| ".".to_string());
                let season = flag(&a.rest, "--season").unwrap_or_else(|| "summer".to_string());
                let mut links: Vec<String> = Vec::new();
                for p in &parts { if !links.contains(&p.texset) { links.push(p.texset.clone()); } }
                links.sort();
                // a SQUARE, power-of-two atlas: the 8x7 grid of 512 px cells (4096x3584) of the
                // straight showcase rendered every tree grey (2026-09-14) — the engine wants
                // power-of-two sides. Grid side = the smallest power of two whose square holds
                // every material; the cell shrinks so the atlas never exceeds 4096.
                let mut side: u32 = 1;
                while side * side < links.len() as u32 { side *= 2; }
                let cell: u32 = (4096 / side).min(512).max(64);
                #[allow(non_snake_case)]
                let CELL: u32 = cell;
                let cols = side;
                let rows = side;
                let (aw, ah) = (cols * CELL, rows * CELL);
                let mut canvas = vec![0u8; (aw * ah * 4) as usize];
                let leafy = |l: &str| { let ll = l.to_lowercase(); ll.contains("branch") || ll.contains("leaf") || ll.contains("foliage") };
                let grassy = |l: &str| { let ll = l.to_lowercase(); ll.contains("grass") || ll.contains("hill") || ll.contains("platform") };
                // --atlas-flat MATCH=RRGGBB,…: the colour of a link WITHOUT a texture file
                // (the hill blocks' pack materials); first substring match wins
                let flats: Vec<(String, [u8; 4])> = flag(&a.rest, "--atlas-flat").unwrap_or_default().split(',').filter(|s| !s.is_empty()).map(|kv| {
                    let (k, v) = kv.split_once('=').unwrap_or_else(|| die(format!("--atlas-flat {kv}: MATCH=RRGGBB")));
                    let n = u32::from_str_radix(v.trim_start_matches('#'), 16).unwrap_or_else(|_| die(format!("--atlas-flat {kv}: RRGGBB")));
                    (k.to_lowercase(), [(n >> 16) as u8, (n >> 8) as u8, n as u8, 255])
                }).collect();
                let mut cell_of: std::collections::BTreeMap<String, (u32, u32)> = Default::default();
                for (i, link) in links.iter().enumerate() {
                    let (col, row) = (i as u32 % cols, i as u32 / cols);
                    cell_of.insert(link.clone(), (col, row));
                    // the texture: <stem>.dds / <stem>_D.dds in --items-dir, prefix stripped
                    let last = link.rsplit(|c| c == '\\' || c == '/').next().unwrap_or(link).to_string();
                    let stripped = match last.split_once('_') { Some((pre, rest)) if pre.chars().all(|c| c.is_ascii_uppercase()) => rest.to_string(), _ => last.clone() };
                    let mut found: Option<(u32, u32, Vec<u8>)> = None;
                    for cand in [format!("{last}.dds"), format!("{last}_D.dds"), format!("{stripped}.dds"), format!("{stripped}_D.dds")] {
                        let path = std::path::Path::new(&items_dir).join(&cand);
                        if path.is_file() {
                            if let Ok(bytes) = std::fs::read(&path) {
                                if let Ok(t) = mapgeom::static_item::texture::decode_capped_rgba(&bytes, CELL) { found = Some(t); report.push(format!("atlas cell {col},{row}: {link} <- {cand}")); break; }
                            }
                        }
                    }
                    let (tw, th, trgba) = found.unwrap_or_else(|| {
                        let ll = link.to_lowercase();
                        let c: [u8; 4] = match flats.iter().find(|(k, _)| ll.contains(k.as_str())) {
                            Some((_, c)) => *c,
                            None if leafy(link) => [62, 150, 40, 255],
                            None if grassy(link) => [78, 140, 52, 255],
                            None if ll.contains("dirt") => [139, 107, 62, 255],
                            None => [110, 110, 105, 255],
                        };
                        report.push(format!("atlas cell {col},{row}: {link} <- flat (no texture file)"));
                        (4, 4, c.iter().cycle().take(64).cloned().collect())
                    });
                    let is_leaf = leafy(link);
                    let is_grass = !is_leaf && grassy(link);
                    for y in 0..CELL {
                        for x in 0..CELL {
                            let sx = (x * tw / CELL).min(tw - 1) as usize;
                            let sy = (y * th / CELL).min(th - 1) as usize;
                            let si = (sy * tw as usize + sx) * 4;
                            let (mut r, mut g, mut b) = (trgba[si] as f32, trgba[si + 1] as f32, trgba[si + 2] as f32);
                            match (season.as_str(), is_leaf, is_grass) {
                                ("ice", true, _) => { let l = 0.3 * r + 0.59 * g + 0.11 * b; r = l * 0.5 + 232.0 * 0.5; g = l * 0.5 + 242.0 * 0.5; b = l * 0.5 + 255.0 * 0.5; }
                                ("ice", _, true) => { let l = 0.3 * r + 0.59 * g + 0.11 * b; r = l * 0.25 + 236.0 * 0.75; g = l * 0.25 + 242.0 * 0.75; b = l * 0.25 + 252.0 * 0.75; }
                                ("ice", _, _) => { r = r * 0.7 + 215.0 * 0.3; g = g * 0.7 + 222.0 * 0.3; b = b * 0.7 + 235.0 * 0.3; }
                                ("fall", true, _) => { let m = r.max(g); r = (m * 1.18).min(255.0); g *= 0.55; b *= 0.25; }
                                ("fall", _, true) => { let l = 0.3 * r + 0.59 * g + 0.11 * b; r = (l * 0.4 + 176.0 * 0.6).min(255.0); g = l * 0.4 + 142.0 * 0.6; b = l * 0.4 + 58.0 * 0.6; }
                                _ => {}
                            }
                            // the game samples v = 0 at the BOTTOM of the texture: write the grid
                            // bottom-up, or every cell lands one mirrored row off (measured
                            // 2026-09-14: the road came out grass-green, the hills road-grey)
                            let dy = ah - 1 - (row * CELL + y);
                            let di = ((dy * aw + col * CELL + x) * 4) as usize;
                            canvas[di] = r as u8; canvas[di + 1] = g as u8; canvas[di + 2] = b as u8; canvas[di + 3] = 255;
                        }
                    }
                }
                // remap every part's uvs into its cell (wrapped, with a small inset), one set
                let inset = 2.0 / CELL as f32;
                for p in parts.iter_mut() {
                    let (col, row) = cell_of[&p.texset];
                    for uv in p.uv.iter_mut() {
                        let fu = (uv[0] - uv[0].floor()).clamp(inset, 1.0 - inset);
                        let fv = (uv[1] - uv[1].floor()).clamp(inset, 1.0 - inset);
                        uv[0] = (col as f32 + fu) / cols as f32;
                        uv[1] = (row as f32 + fv) / rows as f32;
                    }
                    p.texset = texset_default.clone();
                }
                println!("atlas: {} material(s) in a {cols}x{rows} grid of {CELL} px cells ({aw}x{ah}), season {season}", links.len());
                atlas_tex = Some((aw, ah, canvas));
            }
            // --clusters R: a vehicle skin whose geometry reaches far from the car crashes
            // the client (measured 2026-09-13: 20 cards spread over 680 m die, the same 20
            // cards inside ~450 m render), so the scenery ships as one skin per REGION.
            // Each cluster gets its own zip and its own parked ghost at the cluster's
            // centre; `<out>/clusters.tsv` lists name, anchor and vertex count.
            if let Some(r) = flag(&a.rest, "--clusters") {
                let radius: f32 = r.parse().unwrap_or_else(|_| die("--clusters METRES".to_string()));
                let mut centres: Vec<[f32; 3]> = Vec::new();
                let mut assign: Vec<usize> = Vec::new();
                let key_of = |p: &skin::Part| -> [f32; 3] {
                    let (lo, hi) = p.bounds();
                    [(lo[0] + hi[0]) * 0.5, (lo[1] + hi[1]) * 0.5, (lo[2] + hi[2]) * 0.5]
                };
                for p in &parts {
                    let c = key_of(p);
                    let near = centres.iter().position(|q| {
                        let d = ((q[0] - c[0]).powi(2) + (q[2] - c[2]).powi(2)).sqrt();
                        d <= radius
                    });
                    match near {
                        Some(i) => assign.push(i),
                        None => { centres.push(c); assign.push(centres.len() - 1); }
                    }
                }
                let zip_base = flag(&a.rest, "--zip").unwrap_or_else(|| die("--clusters needs --zip BASE.zip".to_string()));
                let base = zip_base.trim_end_matches(".zip").to_string();
                let mut tsv = String::from("# name\tanchor_x\tanchor_y\tanchor_z\tparts\tvertices\n");
                for (k, centre) in centres.iter().enumerate() {
                    // the cluster's own anchor, in world coordinates
                    let world = [anchor_world[0] + centre[0], anchor_world[1] + centre[1], anchor_world[2] + centre[2]];
                    let mut mine: Vec<skin::Part> = parts.iter().zip(&assign).filter(|(_, a)| **a == k).map(|(p, _)| {
                        let mut q = p.clone();
                        for v in q.pos.iter_mut() {
                            v[0] -= centre[0];
                            v[1] -= centre[1];
                            v[2] -= centre[2];
                        }
                        q
                    }).collect();
                    let mut parts = std::mem::take(&mut mine);
                    // --cards: replace each placed item by two crossed quads spanning its own
                    // bounding box — 8 vertices a tree instead of ~1250. A car skin asserts
                    // somewhere between 25k and 54k vertices (measured 2026-09-13: 25,014 renders,
                    // 53,688 dies with the destroyed-object int3), and a campaign map's 260 trees
                    // are 325k, so the whole scenery only fits as cards.
                    if a.rest.iter().any(|x| x == "--cards") {
                        let mut by_src: std::collections::BTreeMap<String, (([f32; 3], [f32; 3]), String)> = std::collections::BTreeMap::new();
                        for p in &parts {
                            let key = p.source.split_whitespace().next().unwrap_or("").to_string();
                            let (lo, hi) = p.bounds();
                            let e = by_src.entry(key).or_insert(((lo, hi), p.texset.clone()));
                            for k in 0..3 {
                                (e.0).0[k] = (e.0).0[k].min(lo[k]);
                                (e.0).1[k] = (e.0).1[k].max(hi[k]);
                            }
                        }
                        let n_src = by_src.len();
                        let set = flag(&a.rest, "--cards-set").unwrap_or_else(|| texset_default.clone());
                        let shrink: f32 = flag(&a.rest, "--cards-width").and_then(|s| s.parse().ok()).unwrap_or(0.8);
                        parts.clear();
                        let mut card = skin::Part { texset: set.clone(), pos: Vec::new(), nrm: Vec::new(), uv: Vec::new(), idx: Vec::new(), source: format!("{n_src} cards") };
                        for (_, ((lo, hi), _)) in by_src {
                            let cx = (lo[0] + hi[0]) * 0.5;
                            let cz = (lo[2] + hi[2]) * 0.5;
                            let w = ((hi[0] - lo[0]).max(hi[2] - lo[2]) * shrink * 0.5).max(0.25);
                            for (dx, dz, nx, nz) in [(w, 0.0f32, 0.0f32, 1.0f32), (0.0, w, 1.0, 0.0)] {
                                let b = card.pos.len() as u32;
                                card.pos.push([cx - dx, lo[1], cz - dz]);
                                card.pos.push([cx + dx, lo[1], cz + dz]);
                                card.pos.push([cx + dx, hi[1], cz + dz]);
                                card.pos.push([cx - dx, hi[1], cz - dz]);
                                for _ in 0..4 { card.nrm.push([nx, 0.0, nz]); }
                                card.uv.push([0.0, 1.0]);
                                card.uv.push([1.0, 1.0]);
                                card.uv.push([1.0, 0.0]);
                                card.uv.push([0.0, 0.0]);
                                // both faces, so a card is visible from either side
                                card.idx.extend_from_slice(&[b, b + 1, b + 2, b, b + 2, b + 3, b, b + 2, b + 1, b, b + 3, b + 2]);
                            }
                        }
                        println!("cards: {n_src} placements -> {} vertices", card.pos.len());
                        parts.push(card);
                    }
                    // --merge: one visual per texture set per --max-verts run, instead of one per
                    // source geom. A campaign map's 260 trees arrive as 1008 little geoms, and a car
                    // skin with 1008 visuals takes the client down on import (measured 2026-09-13:
                    // the connection drops mid-import, no crash dump); merged, the same 325k
                    // vertices are ~12 visuals.
                    if a.rest.iter().any(|x| x == "--merge") {
                        let cap = o.max_verts.max(3);
                        let mut by_set: std::collections::BTreeMap<String, Vec<skin::Part>> = std::collections::BTreeMap::new();
                        for p in parts.drain(..) {
                            by_set.entry(p.texset.clone()).or_default().push(p);
                        }
                        let before: usize = by_set.values().map(|v| v.len()).sum();
                        for (set, group) in by_set {
                            let fresh = || skin::Part { texset: set.clone(), pos: Vec::new(), nrm: Vec::new(), uv: Vec::new(), idx: Vec::new(), source: format!("{set} merged") };
                            let mut cur = fresh();
                            for p in group {
                                if !cur.pos.is_empty() && cur.pos.len() + p.pos.len() > cap {
                                    parts.push(std::mem::replace(&mut cur, fresh()));
                                }
                                let base = cur.pos.len() as u32;
                                cur.pos.extend_from_slice(&p.pos);
                                cur.nrm.extend_from_slice(&p.nrm);
                                cur.uv.extend_from_slice(&p.uv);
                                cur.idx.extend(p.idx.iter().map(|i| i + base));
                            }
                            if !cur.pos.is_empty() {
                                parts.push(cur);
                            }
                        }
                        println!("merged {before} geoms into {} visual(s) of at most {cap} vertices", parts.len());
                    }
                    let mut mine = parts;
                    let nverts: usize = mine.iter().map(|p| p.pos.len()).sum();
                    if false {
                        let cap = o.max_verts.max(3);
                        let mut by_set: std::collections::BTreeMap<String, Vec<skin::Part>> = std::collections::BTreeMap::new();
                        for p in mine.drain(..) { by_set.entry(p.texset.clone()).or_default().push(p); }
                        for (set, group) in by_set {
                            let fresh = || skin::Part { texset: set.clone(), pos: Vec::new(), nrm: Vec::new(), uv: Vec::new(), idx: Vec::new(), source: format!("{set} merged") };
                            let mut cur = fresh();
                            for p in group {
                                if !cur.pos.is_empty() && cur.pos.len() + p.pos.len() > cap { mine.push(std::mem::replace(&mut cur, fresh())); }
                                let b = cur.pos.len() as u32;
                                cur.pos.extend_from_slice(&p.pos);
                                cur.nrm.extend_from_slice(&p.nrm);
                                cur.uv.extend_from_slice(&p.uv);
                                cur.idx.extend(p.idx.iter().map(|i| i + b));
                            }
                            if !cur.pos.is_empty() { mine.push(cur); }
                        }
                    }
                    let built = skin::build(&template, &mine, &o).unwrap_or_else(die);
                    let dir = out_dir.join(format!("c{k}"));
                    std::fs::create_dir_all(&dir).unwrap_or_else(|e| die(e.to_string()));
                    std::fs::write(dir.join("MainBody.Mesh.gbx"), &built.file).unwrap_or_else(|e| die(e.to_string()));
                    println!("cluster {k}: anchor {:.1},{:.1},{:.1}  {} visual(s)  {nverts} vertices", world[0], world[1], world[2], mine.len());
                    tsv.push_str(&format!("{base}-{k}.zip\t{:.3}\t{:.3}\t{:.3}\t{}\t{nverts}\n", world[0], world[1], world[2], mine.len()));
                    // the textures are written once below; this pass only needs the meshes
                }
                std::fs::create_dir_all(&out_dir).unwrap_or_else(|e| die(e.to_string()));
                std::fs::write(out_dir.join("clusters.tsv"), &tsv).unwrap_or_else(|e| die(e.to_string()));
                println!("{} cluster(s) within {radius} m -> {}", centres.len(), out_dir.join("clusters.tsv").display());
            }
            // --cards: replace each placed item by two crossed quads spanning its own
            // bounding box — 8 vertices a tree instead of ~1250. A car skin asserts
            // somewhere between 25k and 54k vertices (measured 2026-09-13: 25,014 renders,
            // 53,688 dies with the destroyed-object int3), and a campaign map's 260 trees
            // are 325k, so the whole scenery only fits as cards.
            // --cones: each placed item becomes a two-cone tree — a brown trunk (texture set
            // --cones-trunk-set, default Skin) and a season-coloured crown (the default set),
            // both CLOSED solids like attempt 2's hills. Cards were the 8-vertex answer;
            // cones read as trees from the chase camera (2026-09-14).
            if a.rest.iter().any(|x| x == "--cones") {
                let mut by_src: std::collections::BTreeMap<String, ([f32; 3], [f32; 3])> = std::collections::BTreeMap::new();
                for p in &parts {
                    let key = p.source.split_whitespace().next().unwrap_or("").to_string();
                    let (lo, hi) = p.bounds();
                    let e = by_src.entry(key).or_insert((lo, hi));
                    for k in 0..3 {
                        e.0[k] = e.0[k].min(lo[k]);
                        e.1[k] = e.1[k].max(hi[k]);
                    }
                }
                let n_src = by_src.len();
                let crown_set = texset_default.clone();
                let trunk_set = flag(&a.rest, "--cones-trunk-set").unwrap_or_else(|| texset_default.clone());
                let segs: usize = flag(&a.rest, "--cones-segments").and_then(|s| s.parse().ok()).unwrap_or(7);
                let fresh = |set: &str| skin::Part { texset: set.to_string(), pos: Vec::new(), nrm: Vec::new(), uv: Vec::new(), idx: Vec::new(), source: format!("{n_src} cone trees ({set})") };
                let mut crown = fresh(&crown_set);
                let mut trunk = fresh(&trunk_set);
                // a closed cone: base ring at y0 (radius r), apex at y1, plus a base fan
                // every cone gets a CONSTANT uv: trunks read the left half of the set's _B texture,
                // crowns the right half (`--flat Details=BROWN|GREEN` writes such a two-half texture),
                // so one texture set carries both colours — the one-set shape the shoot renders.
                let mut cone = |part: &mut skin::Part, cx: f32, cz: f32, y0: f32, y1: f32, r: f32, u: f32| {
                    let b = part.pos.len() as u32;
                    let h = (y1 - y0).max(0.1);
                    for i in 0..segs {
                        let a = i as f32 / segs as f32 * std::f32::consts::TAU;
                        let (s, c) = a.sin_cos();
                        part.pos.push([cx + r * c, y0, cz + r * s]);
                        // outward normal of the slanted side
                        let l = (h * h + r * r).sqrt();
                        part.nrm.push([c * h / l, r / l, s * h / l]);
                        // a varying uv inside the cone's half of the texture: identical uvs on every
                        // vertex give a degenerate tangent basis (NaN after Dec3N packing) and the
                        // client dies on import (QC/QD/QE/QF, 2026-09-14)
                        part.uv.push([u - 0.2 + 0.4 * (i as f32 / segs as f32), 1.0]);
                    }
                    part.pos.push([cx, y1, cz]);
                    part.nrm.push([0.0, 1.0, 0.0]);
                    part.uv.push([u, 0.0]);
                    part.pos.push([cx, y0, cz]);
                    part.nrm.push([0.0, -1.0, 0.0]);
                    part.uv.push([u, 0.9]);
                    let apex = b + segs as u32;
                    let base = apex + 1;
                    for i in 0..segs as u32 {
                        let j = (i + 1) % segs as u32;
                        part.idx.extend_from_slice(&[b + i, apex, b + j]);
                        part.idx.extend_from_slice(&[b + j, base, b + i]);
                    }
                };
                for (_, (lo, hi)) in by_src {
                    let cx = (lo[0] + hi[0]) * 0.5;
                    let cz = (lo[2] + hi[2]) * 0.5;
                    let h = (hi[1] - lo[1]).max(1.0);
                    let w = ((hi[0] - lo[0]).max(hi[2] - lo[2]) * 0.5).max(0.4);
                    cone(&mut trunk, cx, cz, lo[1] - 0.5, lo[1] + 0.45 * h, (w * 0.12).max(0.15), 0.25);
                    cone(&mut crown, cx, cz, lo[1] + 0.3 * h, hi[1], w, 0.75);
                }
                println!("cones: {n_src} placements -> {} vertices", crown.pos.len() + trunk.pos.len());
                parts.clear();
                parts.push(trunk);
                parts.push(crown);
            }
            if a.rest.iter().any(|x| x == "--cards") {
                let mut by_src: std::collections::BTreeMap<String, (([f32; 3], [f32; 3]), String)> = std::collections::BTreeMap::new();
                for p in &parts {
                    let key = p.source.split_whitespace().next().unwrap_or("").to_string();
                    let (lo, hi) = p.bounds();
                    let e = by_src.entry(key).or_insert(((lo, hi), p.texset.clone()));
                    for k in 0..3 {
                        (e.0).0[k] = (e.0).0[k].min(lo[k]);
                        (e.0).1[k] = (e.0).1[k].max(hi[k]);
                    }
                }
                let n_src = by_src.len();
                let set = flag(&a.rest, "--cards-set").unwrap_or_else(|| texset_default.clone());
                let shrink: f32 = flag(&a.rest, "--cards-width").and_then(|s| s.parse().ok()).unwrap_or(0.8);
                parts.clear();
                let mut card = skin::Part { texset: set.clone(), pos: Vec::new(), nrm: Vec::new(), uv: Vec::new(), idx: Vec::new(), source: format!("{n_src} cards") };
                for (_, ((lo, hi), _)) in by_src {
                    let cx = (lo[0] + hi[0]) * 0.5;
                    let cz = (lo[2] + hi[2]) * 0.5;
                    let w = ((hi[0] - lo[0]).max(hi[2] - lo[2]) * shrink * 0.5).max(0.25);
                    for (dx, dz, nx, nz) in [(w, 0.0f32, 0.0f32, 1.0f32), (0.0, w, 1.0, 0.0)] {
                        let b = card.pos.len() as u32;
                        card.pos.push([cx - dx, lo[1], cz - dz]);
                        card.pos.push([cx + dx, lo[1], cz + dz]);
                        card.pos.push([cx + dx, hi[1], cz + dz]);
                        card.pos.push([cx - dx, hi[1], cz - dz]);
                        for _ in 0..4 { card.nrm.push([nx, 0.0, nz]); }
                        card.uv.push([0.0, 1.0]);
                        card.uv.push([1.0, 1.0]);
                        card.uv.push([1.0, 0.0]);
                        card.uv.push([0.0, 0.0]);
                        // both faces, so a card is visible from either side
                        card.idx.extend_from_slice(&[b, b + 1, b + 2, b, b + 2, b + 3, b, b + 2, b + 1, b, b + 3, b + 2]);
                    }
                }
                println!("cards: {n_src} placements -> {} vertices", card.pos.len());
                parts.push(card);
            }
            // --merge: one visual per texture set per --max-verts run, instead of one per
            // source geom. A campaign map's 260 trees arrive as 1008 little geoms, and a car
            // skin with 1008 visuals takes the client down on import (measured 2026-09-13:
            // the connection drops mid-import, no crash dump); merged, the same 325k
            // vertices are ~12 visuals.
            if a.rest.iter().any(|x| x == "--merge") {
                let cap = o.max_verts.max(3);
                let mut by_set: std::collections::BTreeMap<String, Vec<skin::Part>> = std::collections::BTreeMap::new();
                for p in parts.drain(..) {
                    by_set.entry(p.texset.clone()).or_default().push(p);
                }
                let before: usize = by_set.values().map(|v| v.len()).sum();
                for (set, group) in by_set {
                    let fresh = || skin::Part { texset: set.clone(), pos: Vec::new(), nrm: Vec::new(), uv: Vec::new(), idx: Vec::new(), source: format!("{set} merged") };
                    let mut cur = fresh();
                    for p in group {
                        if !cur.pos.is_empty() && cur.pos.len() + p.pos.len() > cap {
                            parts.push(std::mem::replace(&mut cur, fresh()));
                        }
                        let base = cur.pos.len() as u32;
                        cur.pos.extend_from_slice(&p.pos);
                        cur.nrm.extend_from_slice(&p.nrm);
                        cur.uv.extend_from_slice(&p.uv);
                        cur.idx.extend(p.idx.iter().map(|i| i + base));
                    }
                    if !cur.pos.is_empty() {
                        parts.push(cur);
                    }
                }
                println!("merged {before} geoms into {} visual(s) of at most {cap} vertices", parts.len());
            }
            // --dump-obj F: the parts as Wavefront OBJ (skin frame), to look at what a skin
            // holds without a render — which edge of a slope block is the high one, etc.
            if let Some(obj) = flag(&a.rest, "--dump-obj") {
                let mut text = String::new();
                let mut base = 0usize;
                for p in &parts {
                    text.push_str(&format!("o {}\n", p.source.replace(' ', "_")));
                    for v in &p.pos { text.push_str(&format!("v {} {} {}\n", v[0], v[1], v[2])); }
                    for t in p.idx.chunks(3) { if t.len() == 3 { text.push_str(&format!("f {} {} {}\n", base + t[0] as usize + 1, base + t[1] as usize + 1, base + t[2] as usize + 1)); } }
                    base += p.pos.len();
                }
                std::fs::write(&obj, text).unwrap_or_else(|e| die(format!("{obj}: {e}")));
                println!("obj {obj}: {} part(s)", parts.len());
            }
            let built = skin::build(&template, &parts, &o).unwrap_or_else(die);
            std::fs::create_dir_all(&out_dir).unwrap_or_else(|e| die(e.to_string()));
            std::fs::write(out_dir.join("MainBody.Mesh.gbx"), &built.file).unwrap_or_else(|e| die(e.to_string()));
            // textures: one set per material, from --tex (a DDS decoded and re-encoded) or a checker
            let mut sets: Vec<String> = Vec::new();
            for p in &parts { if !sets.contains(&p.texset) { sets.push(p.texset.clone()); } }
            let checker_cells: u32 = flag(&a.rest, "--checker").and_then(|s| s.parse().ok()).unwrap_or(8);
            let mut files: std::collections::BTreeMap<String, Vec<u8>> = std::collections::BTreeMap::new();
            files.insert("MainBody.Mesh.gbx".into(), built.file.clone());
            for s in &sets {
                // --tex SET=FILE.dds (or FILE.dds for the default set)
                let tex_for = flag(&a.rest, "--tex").and_then(|spec| {
                    spec.split(',').find_map(|kv| match kv.split_once('=') {
                        Some((k, v)) if k == s => Some(v.to_string()),
                        None if s == &texset_default => Some(kv.to_string()),
                        _ => None,
                    })
                });
                let atlas_here = if s == &texset_default { atlas_tex.clone() } else { None };
                let (w, h, rgba) = match (atlas_here, tex_for) {
                    (Some(t), _) => t,
                    (None, tex_for) => match tex_for {
                    Some(path) => {
                        let bytes = if std::path::Path::new(&path).is_file() { std::fs::read(&path).unwrap_or_else(|e| die(e.to_string())) } else { store.read(&path).unwrap_or_else(die).to_vec() };
                        let cap: u32 = flag(&a.rest, "--tex-cap").and_then(|s| s.parse().ok()).unwrap_or(1024);
                        mapgeom::static_item::texture::decode_capped_rgba(&bytes, cap).unwrap_or_else(|e| die(format!("{path}: {e}")))
                    }
                    // --flat RRGGBB (or SET=RRGGBB,…): one colour, for scenery tests where
                    // the material's own texture array cannot be carried
                    None => match flag(&a.rest, "--flat").and_then(|spec| spec.split(',').find_map(|kv| match kv.split_once('=') {
                        Some((k, v)) if k == s => Some(v.to_string()),
                        None if s == &texset_default || !kv.contains('=') => Some(kv.to_string()),
                        _ => None,
                    })) {
                        Some(hex) => {
                            // RRGGBB, or LEFT|RIGHT: a 64x64 texture whose left half is one
                            // colour and right half another (cone trees: trunk | crown, one set)
                            let parse = |s: &str| -> [u8; 4] {
                                let h = s.trim().trim_start_matches('#');
                                let v = u32::from_str_radix(h, 16).unwrap_or_else(|_| die(format!("--flat {hex}: RRGGBB")));
                                [(v >> 16) as u8, (v >> 8) as u8, v as u8, 255]
                            };
                            match hex.split_once('|') {
                                Some((l, r)) => {
                                    let (a, b) = (parse(l), parse(r));
                                    let mut rgba = Vec::with_capacity(64 * 64 * 4);
                                    for _y in 0..64 {
                                        for x in 0..64 {
                                            rgba.extend_from_slice(if x < 32 { &a } else { &b });
                                        }
                                    }
                                    (64, 64, rgba)
                                }
                                None => (64, 64, skin::flat_rgba(64, parse(&hex))),
                            }
                        }
                        None => (512, 512, skin::checker_rgba(512, checker_cells)),
                    },
                    },
                };
                // --tint SET=RRGGBB,…  (or one RRGGBB for every set): multiply the decoded
                // texture by a colour. One atlas, three seasons: green x white-blue = icy,
                // green x orange = autumn (2026-09-13, the checkpoint scenery swap).
                let mut rgba = rgba;
                if let Some(hex) = flag(&a.rest, "--tint").and_then(|spec| spec.split(',').find_map(|kv| match kv.split_once('=') {
                    Some((k, v)) if k == s => Some(v.to_string()),
                    None => Some(kv.to_string()),
                    _ => None,
                })) {
                    let h = hex.trim_start_matches('#');
                    let v = u32::from_str_radix(h, 16).unwrap_or_else(|_| die(format!("--tint {hex}: RRGGBB")));
                    let t = [(v >> 16) as u32 & 0xff, (v >> 8) as u32 & 0xff, v as u32 & 0xff];
                    for px in rgba.chunks_mut(4) {
                        for c in 0..3 { px[c] = ((px[c] as u32 * t[c]) / 255) as u8; }
                    }
                }
                let rough: u8 = flag(&a.rest, "--roughness").and_then(|s| s.parse().ok()).unwrap_or(200);
                for (name, bytes) in skin::texture_set(s, w, h, &rgba, rough) {
                    std::fs::write(out_dir.join(&name), &bytes).unwrap_or_else(|e| die(e.to_string()));
                    files.insert(name, bytes);
                }
                report.push(format!("texture set {s}: {w}x{h} base colour"));
            }
            // THE ZIP RULE (measured 2026-09-12, hillB-min / X2 vs X1 / X3): the vehicle
            // loads the Skin, Wheels and Glass sets whether or not the mesh names them,
            // and a set with no files behind it crashes the import. Fill every missing
            // standard set with 4x4 flats by default (--no-fill-sets to opt out).
            if !a.rest.iter().any(|x| x == "--no-fill-sets") {
                for req in ["Details", "Skin", "Wheels", "Glass"] {
                    if sets.iter().any(|s| s == req) { continue; }
                    for (name, bytes) in skin::texture_set(req, 4, 4, &skin::flat_rgba(4, [128, 128, 128, 255]), 200) {
                        std::fs::write(out_dir.join(&name), &bytes).unwrap_or_else(|e| die(e.to_string()));
                        files.insert(name, bytes);
                    }
                    report.push(format!("texture set {req}: 4x4 grey flats (required by the vehicle even when unused)"));
                }
            }
            // --tex-verbatim DIR: ship the texture files of a KNOWN-GOOD skin unchanged (every
            // *.dds in DIR replaces or adds to what was generated, and generated files DIR
            // does not have are dropped) — keeping only our mesh and, if --keep-tex names it,
            // our own Details_B. 2026-09-14: attempt 2's proven zip carries 64x64 _B maps for
            // all four sets and no Glass_D/Glass_T; ours carried 4x4 fillers plus those two, and
            // the client crashed on import or drew nothing.
            if let Some(dir) = flag(&a.rest, "--tex-verbatim") {
                let keep: Vec<String> = flag(&a.rest, "--keep-tex").unwrap_or_default().split(',').filter(|s| !s.is_empty()).map(|s| s.to_string()).collect();
                let mut verbatim: std::collections::BTreeMap<String, Vec<u8>> = std::collections::BTreeMap::new();
                for e in std::fs::read_dir(&dir).unwrap_or_else(|e| die(format!("{dir}: {e}"))).flatten() {
                    let name = e.file_name().to_string_lossy().to_string();
                    if name.to_lowercase().ends_with(".dds") {
                        verbatim.insert(name.clone(), std::fs::read(e.path()).unwrap_or_else(|e| die(e.to_string())));
                    }
                }
                let ours: Vec<String> = files.keys().filter(|k| k.to_lowercase().ends_with(".dds")).cloned().collect();
                for k in ours {
                    if !keep.contains(&k) {
                        files.remove(&k);
                    }
                }
                for (k, v) in verbatim {
                    if !keep.contains(&k) {
                        files.insert(k, v);
                    }
                }
                report.push(format!("textures taken verbatim from {dir} (kept ours: {keep:?}); zip now {} files", files.len()));
            }
            // --mesh-verbatim FILE: ship THAT mesh instead of the one just built (a known-good
            // mesh through our zip writer isolates the writer from the builder)
            if let Some(m) = flag(&a.rest, "--mesh-verbatim") {
                files.insert("MainBody.Mesh.gbx".into(), std::fs::read(&m).unwrap_or_else(|e| die(format!("{m}: {e}"))));
                report.push(format!("mesh taken verbatim from {m}"));
            }
            if let Some(z) = flag(&a.rest, "--zip") {
                let zip = tmmaps::header::stored_zip(&files);
                std::fs::write(&z, &zip).unwrap_or_else(|e| die(e.to_string()));
                report.push(format!("zip {z}: {} bytes, {} files (deflate)", zip.len(), files.len()));
                // --clusters wrote one mesh per region; each becomes its own zip, with the
                // same texture set as the whole-scenery zip beside it.
                if flag(&a.rest, "--clusters").is_some() {
                    let base = z.trim_end_matches(".zip").to_string();
                    let mut k = 0usize;
                    while let Ok(mesh) = std::fs::read(out_dir.join(format!("c{k}")).join("MainBody.Mesh.gbx")) {
                        let mut f = files.clone();
                        f.insert("MainBody.Mesh.gbx".into(), mesh);
                        let zip = tmmaps::header::stored_zip(&f);
                        let path = format!("{base}-{k}.zip");
                        std::fs::write(&path, &zip).unwrap_or_else(|e| die(e.to_string()));
                        let sha: String = gbx::sha::sha256(&zip).iter().map(|b| format!("{b:02x}")).collect();
                        println!("  cluster zip {path}: {} bytes  sha256 {sha}", zip.len());
                        k += 1;
                    }
                }
            }
            println!(
                "wrote {}/MainBody.Mesh.gbx: {} bytes, {} visuals, {} vertices, {} triangles, materials {:?}, bounds x {:.2}..{:.2} y {:.2}..{:.2} z {:.2}..{:.2} (skin frame: metres from the ghost, +z = car front)",
                out_dir.display(), built.file.len(), built.visuals, built.vertices, built.triangles, built.materials,
                built.bounds.0[0], built.bounds.1[0], built.bounds.0[1], built.bounds.1[1], built.bounds.0[2], built.bounds.1[2]
            );
            for r in report { println!("  {r}"); }
        }
        // scenery-split MAP --ghost G [--radius 24] [--out TSV] [--min-bytes 0]
        // The skin-scenery size question (5): every embedded model's placements
        // measured against the driven line (the ghost's samples as a polyline):
        // a placement whose world AABB comes within --radius of the line is
        // DRIVABLE-ADJACENT, anything farther is SCENERY. A model is removable
        // from the map when ALL its placements are scenery; the report sums the
        // embedded bytes (zip entry sizes, stored) that would leave the file.
        "scenery-split" => {
            let p = a.rest.get(1).cloned().unwrap_or_default();
            let gpath = flag(&a.rest, "--ghost").unwrap_or_else(|| die("--ghost G.Ghost.Gbx".into()));
            let radius: f32 = flag(&a.rest, "--radius").and_then(|s| s.parse().ok()).unwrap_or(24.0);
            let m = tmmaps::map::MapFile::load(std::path::Path::new(&p));
            let d = gbx::record::decode_ghost(&gpath).unwrap_or_else(|e| die(format!("{gpath}: {e}")));
            let path: Vec<[f32; 3]> = d.samples.iter().map(|s| [s.x as f32, s.y as f32, s.z as f32]).collect();
            if path.len() < 2 { die::<()>("the ghost has fewer than two samples".into()); }
            let files = mapgeom::embedded::files(&m).unwrap_or_else(die);
            let by_key: std::collections::BTreeMap<String, Vec<u8>> = files.iter().map(|(k, v)| (k.replace('\\', "/").to_lowercase(), v.clone())).collect();
            // the zip's own entry sizes (stored: the bytes the map file carries)
            let zip_sizes: std::collections::BTreeMap<String, usize> = files.iter().map(|(k, v)| (k.replace('\\', "/").to_lowercase(), v.len())).collect();
            // model -> local AABB (from the Solid2 visuals' bounds) and its texture files
            let mut aabb: std::collections::BTreeMap<String, ([f32; 3], [f32; 3])> = std::collections::BTreeMap::new();
            let mut textures_of: std::collections::BTreeMap<String, Vec<String>> = std::collections::BTreeMap::new();
            let mut bytes_of: std::collections::BTreeMap<String, usize> = std::collections::BTreeMap::new();
            let mut model_key: std::collections::BTreeMap<String, String> = std::collections::BTreeMap::new();
            for it in &m.items {
                if aabb.contains_key(&it.model) { continue; }
                let key = it.model.replace('\\', "/").to_lowercase();
                let hit = by_key.get(&key).map(|v| (key.clone(), v)).or_else(|| by_key.iter().find(|(k, _)| k.ends_with(&format!("/{key}"))).map(|(k, v)| (k.clone(), v)));
                let Some((zkey, bytes)) = hit else { continue };
                model_key.insert(it.model.clone(), zkey.clone());
                bytes_of.insert(it.model.clone(), zip_sizes.get(&zkey).copied().unwrap_or(bytes.len()));
                // textures: the item's external refs ending in .dds, resolved to zip entries
                if let Ok(model) = mapgeom::store::Model::parse(bytes, &it.model) {
                    let mut t = Vec::new();
                    for (_, ext) in &model.externals {
                        if ext.to_lowercase().ends_with(".dds") {
                            let bare = ext.rsplit('\\').next().unwrap_or(ext).to_lowercase();
                            if let Some((zk, _)) = zip_sizes.iter().find(|(k, _)| k.ends_with(&format!("/{bare}")) || **k == bare) { t.push(zk.clone()); }
                        }
                    }
                    textures_of.insert(it.model.clone(), t);
                }
                let (mut lo, mut hi) = ([f32::MAX; 3], [f32::MIN; 3]);
                if let Ok(f) = mapgeom::static_item::parse_file(bytes) {
                    if let Some(s2) = f.item.static_object().and_then(|so| so.solid2()) {
                        for vr in &s2.visuals {
                            if let Some(mapgeom::static_item::Node::Visual(v)) = vr.inline.as_deref() {
                                if let Some(mn) = &v.main {
                                    let b = mn.bounding_box;
                                    for k in 0..3 { lo[k] = lo[k].min(b[k] - b[k + 3].abs()); hi[k] = hi[k].max(b[k] + b[k + 3].abs()); }
                                }
                            }
                        }
                    }
                }
                if lo[0] == f32::MAX { lo = [0.0; 3]; hi = [0.0; 3]; }
                aabb.insert(it.model.clone(), (lo, hi));
            }
            // distance from an AABB to the polyline: sample the polyline's segments finely
            let seg_dist = |q: [f32; 3], a: [f32; 3], b: [f32; 3]| -> f32 {
                let ab = [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
                let l2 = ab[0] * ab[0] + ab[1] * ab[1] + ab[2] * ab[2];
                let t = if l2 < 1e-9 { 0.0 } else { (((q[0] - a[0]) * ab[0] + (q[1] - a[1]) * ab[1] + (q[2] - a[2]) * ab[2]) / l2).clamp(0.0, 1.0) };
                let c = [a[0] + t * ab[0], a[1] + t * ab[1], a[2] + t * ab[2]];
                ((q[0] - c[0]).powi(2) + (q[1] - c[1]).powi(2) + (q[2] - c[2]).powi(2)).sqrt()
            };
            let aabb_dist = |lo: [f32; 3], hi: [f32; 3]| -> f32 {
                // min over segments of the distance between the box and the segment,
                // taken as: clamp the segment's closest point to the box (exact for a
                // point; for a segment sample 8 points along it)
                let mut best = f32::MAX;
                for w in path.windows(2) {
                    for k in 0..=8 {
                        let t = k as f32 / 8.0;
                        let q = [w[0][0] + t * (w[1][0] - w[0][0]), w[0][1] + t * (w[1][1] - w[0][1]), w[0][2] + t * (w[1][2] - w[0][2])];
                        let c = [q[0].clamp(lo[0], hi[0]), q[1].clamp(lo[1], hi[1]), q[2].clamp(lo[2], hi[2])];
                        let dd = seg_dist(c, w[0], w[1]);
                        if dd < best { best = dd; }
                    }
                }
                best
            };
            struct Row { model: String, total: usize, scenery: usize, min_d: f32, verts_hint: usize }
            let mut rows: std::collections::BTreeMap<String, Row> = std::collections::BTreeMap::new();
            let mut per_placement: Vec<(usize, String, f32)> = Vec::new();
            for it in &m.items {
                let Some((lo, hi)) = aabb.get(&it.model) else { continue };
                let xf = mapgeom::place::anchored(it.pos, [it.yaw, it.pitch, it.roll], it.pivot, it.scale);
                let (mut wlo, mut whi) = ([f32::MAX; 3], [f32::MIN; 3]);
                for c in 0..8 {
                    let v = [if c & 1 == 0 { lo[0] } else { hi[0] }, if c & 2 == 0 { lo[1] } else { hi[1] }, if c & 4 == 0 { lo[2] } else { hi[2] }];
                    let w = mapgeom::geom::apply(&xf, v);
                    for k in 0..3 { wlo[k] = wlo[k].min(w[k]); whi[k] = whi[k].max(w[k]); }
                }
                let dist = aabb_dist(wlo, whi);
                per_placement.push((it.index, it.model.clone(), dist));
                let r = rows.entry(it.model.clone()).or_insert(Row { model: it.model.clone(), total: 0, scenery: 0, min_d: f32::MAX, verts_hint: 0 });
                r.total += 1;
                if dist > radius { r.scenery += 1; }
                if dist < r.min_d { r.min_d = dist; }
                r.verts_hint = 0;
            }
            let mut removable_bytes = 0usize;
            let mut kept_bytes = 0usize;
            let mut removable_models = 0usize;
            let mut removable_placements = 0usize;
            let mut total_placements = 0usize;
            let mut tex_needed: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
            let mut tex_all: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
            let mut out_rows: Vec<(usize, String)> = Vec::new();
            for r in rows.values() {
                let b = bytes_of.get(&r.model).copied().unwrap_or(0);
                total_placements += r.total;
                let all_scenery = r.scenery == r.total;
                if all_scenery { removable_bytes += b; removable_models += 1; removable_placements += r.total; } else { kept_bytes += b; }
                for t in textures_of.get(&r.model).cloned().unwrap_or_default() {
                    tex_all.insert(t.clone());
                    if !all_scenery { tex_needed.insert(t); }
                }
                out_rows.push((b, format!("{}\t{}\t{}\t{}\t{:.1}\t{}", r.model, b, r.total, r.scenery, r.min_d, if all_scenery { "REMOVABLE" } else if r.scenery > 0 { "mixed" } else { "line" })));
            }
            let tex_removable: usize = tex_all.difference(&tex_needed).map(|t| zip_sizes.get(t).copied().unwrap_or(0)).sum();
            let tex_total: usize = tex_all.iter().map(|t| zip_sizes.get(t).copied().unwrap_or(0)).sum();
            out_rows.sort_by(|x, y| y.0.cmp(&x.0));
            println!("{p}: {} embedded files, {} placed models, {} placements; line = {} samples; radius {radius} m", files.len(), rows.len(), total_placements, path.len());
            println!("model\tzip_bytes\tplacements\tscenery_placements\tmin_dist_to_line_m\tverdict");
            let min_bytes: usize = flag(&a.rest, "--min-bytes").and_then(|s| s.parse().ok()).unwrap_or(0);
            for (b, row) in &out_rows { if *b >= min_bytes { println!("{row}"); } }
            let zip_total: usize = zip_sizes.values().sum();
            println!("TOTAL embedded {} bytes in {} files; models {} bytes; textures {} bytes", zip_total, files.len(), kept_bytes + removable_bytes, tex_total);
            println!("REMOVABLE (every placement > {radius} m from the line): {removable_models} models, {removable_placements} placements, {removable_bytes} bytes of models + {tex_removable} bytes of textures only they use = {} bytes ({:.1} % of the embedded zip)", removable_bytes + tex_removable, 100.0 * (removable_bytes + tex_removable) as f64 / zip_total.max(1) as f64);
            println!("KEPT (some placement within {radius} m): {} models, {kept_bytes} bytes", rows.len() - removable_models);
            if let Some(o) = flag(&a.rest, "--out") {
                let mut s = String::from("index\tmodel\tdist_m\n");
                for (i, mdl, dd) in &per_placement { s.push_str(&format!("i{i}\t{mdl}\t{dd:.2}\n")); }
                std::fs::write(&o, s).unwrap_or_else(|e| die(e.to_string()));
                println!("per-placement distances: {o}");
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
        // crystal-census-pak [--min-corners N]: every pack ITEM whose mesh is a crystal — the tex-coord index
        // array length (face corners) per layer; the ones past --min-corners (default 60000) printed: which width
        // the GAME writes a > 65 535-entry optimized array with (Hugo's Grass crash, 2026-10-18)
        "crystal-census-pak" => {
            let mut store = open(&a);
            let min: usize = flag(&a.rest, "--min-corners").and_then(|v| v.parse().ok()).unwrap_or(60000);
            let names: Vec<String> = store.entries().map(|e| e.path()).filter(|p| p.to_uppercase().ends_with(".ITEM.GBX")).collect();
            let mut n_crystal = 0usize;
            let mut best: Vec<(usize, String)> = Vec::new();
            for p in &names {
                let Ok(bytes) = store.read(p) else { continue };
                let Ok(it) = mapgeom::crystal::ItemCrystal::open(&bytes) else { continue };
                n_crystal += 1;
                for (li, l) in it.model.layers.iter().enumerate() {
                    if let Some(c) = l.kind.crystal() {
                        let corners: usize = c.faces.iter().map(|f| f.verts.len()).sum();
                        best.push((corners, format!("{p} layer {li}: {} positions, {} faces, {} tex coords", c.positions.len(), c.faces.len(), c.tex_coords.len())));
                        if corners >= min {
                            println!("{corners} corners: {p} layer {li} ({} positions, {} faces, {} tex coords)", c.positions.len(), c.faces.len(), c.tex_coords.len());
                        }
                    }
                }
            }
            best.sort_by(|a, b| b.0.cmp(&a.0));
            println!("{n_crystal} crystal items of {} items; the 5 largest layers: {}", names.len(), best.iter().take(5).map(|(c, s)| format!("{c} corners {s}")).collect::<Vec<_>>().join(" | "));
        }
        // vista-deck --list FILE | --names A,B [--out TSV]: for each terrain-integrated variant block (…On<Terrain><Cliff|Hill>N…)
        // the Stadium base block its prefab embeds, and that base prefab's placement in the variant's frame (Hugo's Grass
        // alteration on the vista collections, 2026-10-18). FILE: lines "count<TAB>name" or "name".
        "vista-deck" => {
            let mut store = open(&a);
            let mut names: Vec<(String, u32)> = Vec::new();
            if let Some(f) = flag(&a.rest, "--list") {
                for l in std::fs::read_to_string(&f).unwrap_or_else(|e| die(format!("{f}: {e}"))).lines() {
                    let parts: Vec<&str> = l.split_whitespace().collect();
                    match parts.as_slice() {
                        [c, n] if c.parse::<u32>().is_ok() => names.push((n.to_string(), c.parse().unwrap())),
                        [n] => names.push((n.to_string(), 0)),
                        _ => {}
                    }
                }
            }
            if let Some(n) = flag(&a.rest, "--names") {
                names.extend(n.split(',').map(|s| (s.trim().to_string(), 0)));
            }
            eprintln!("indexing the Stadium block prefabs…");
            let index = mapgeom::vista::base_prefab_index(&mut store).unwrap_or_else(|e| die(e));
            eprintln!("{} prefabs indexed", index.len());
            let mut rows = vec!["variant	base	dx	dy	dz	dyaw	size_cells	deck_match	used	notes".to_string()];
            for (n, used) in &names {
                match mapgeom::vista::deck_of(&mut store, &index, n) {
                    Ok(d) => {
                        let row = format!("{}	{}	{:.3}	{:.3}	{:.3}	{:.4}	{}x{}x{}	{:.1}% of {}	{}	{}", d.variant, d.base, d.dx, d.dy, d.dz, d.dyaw, d.size[0], d.size[1], d.size[2], d.deck_match * 100.0, d.base_tris, used, d.notes.join(" | "));
                        println!("{row}");
                        rows.push(row);
                        for (b, x, y, z, yaw) in &d.extra {
                            let row = format!("{}\t{}\t{:.3}\t{:.3}\t{:.3}\t{:.4}\t{}x{}x{}\t-\t{}\tEXTRA deck piece (also embedded)", d.variant, b, x, y, z, yaw, d.size[0], d.size[1], d.size[2], used);
                            println!("{row}");
                            rows.push(row);
                        }
                    }
                    Err(e) => {
                        println!("{n}	-	-	-	-	-	-	-	{used}	ERROR {e}");
                        rows.push(format!("{n}	-	-	-	-	-	-	-	{used}	ERROR {e}"));
                    }
                }
            }
            if let Some(out) = flag(&a.rest, "--out") {
                std::fs::write(&out, rows.join("\n") + "\n").unwrap_or_else(|e| die(format!("{out}: {e}")));
                eprintln!("wrote {out}");
            }
        }
        // crystal-faces FILE.. : per crystal layer and per material, the face census for Hugo's deck/curb predicate
        // (2026-10-19): count, normal classes (|ny| > 0.8 deck-like / 0.5..0.8 slope / < 0.5 vertical), y range,
        // and the up-facing faces' footprint (their x/z extents) — all in the item frame.
        "crystal-faces" => {
            for path in a.rest.iter().skip(1) {
                let bytes = std::fs::read(path).unwrap_or_else(|e| die(format!("{path}: {e}")));
                let it = mapgeom::crystal::ItemCrystal::open(&bytes).unwrap_or_else(|e| die(format!("{path}: {e}")));
                let mats: Vec<String> = it.model.materials.iter().map(|m| match m.inst().and_then(|x| x.main.as_ref()) {
                    Some(main) => match &main.link { mapgeom::crystal_model::Id::Str(s) => s.clone(), other => format!("{other:?}") },
                    None => m.name.clone(),
                }).collect();
                println!("{path}: {} materials, {} layers", mats.len(), it.model.layers.len());
                for (li, l) in it.model.layers.iter().enumerate() {
                    let Some(c) = l.kind.crystal() else { continue };
                    #[derive(Default)]
                    struct Acc { n: usize, deck: usize, slope: usize, vert: usize, down: usize, ylo: f32, yhi: f32, up_xlo: f32, up_xhi: f32, up_zlo: f32, up_zhi: f32, up_ylo: f32, up_yhi: f32, area_up: f64 }
                    let mut per: std::collections::BTreeMap<i32, Acc> = Default::default();
                    let mut bands: std::collections::BTreeMap<(i32, i32), (usize, f64, f32, f32)> = Default::default();
                    let mut vert: std::collections::BTreeMap<(i32, i32, i32, i32, i32), (usize, f64)> = Default::default();
                    let mut slopes: std::collections::BTreeMap<(i32, i32, i32, i32), (usize, f64, f32, f32, f32, f32)> = Default::default();
                    for f in &c.faces {
                        if f.verts.len() < 3 { continue; }
                        let p: Vec<[f32; 3]> = f.verts.iter().filter_map(|&i| c.positions.get(i as usize).copied()).collect();
                        if p.len() < 3 { continue; }
                        // Newell normal
                        let mut nrm = [0f32; 3];
                        for i in 0..p.len() {
                            let (a, b) = (p[i], p[(i + 1) % p.len()]);
                            nrm[0] += (a[1] - b[1]) * (a[2] + b[2]);
                            nrm[1] += (a[2] - b[2]) * (a[0] + b[0]);
                            nrm[2] += (a[0] - b[0]) * (a[1] + b[1]);
                        }
                        let len = (nrm[0] * nrm[0] + nrm[1] * nrm[1] + nrm[2] * nrm[2]).sqrt();
                        if len < 1e-9 { continue; }
                        let ny = nrm[1] / len;
                        let e = per.entry(f.material).or_insert_with(|| Acc { ylo: f32::INFINITY, yhi: f32::NEG_INFINITY, up_xlo: f32::INFINITY, up_xhi: f32::NEG_INFINITY, up_zlo: f32::INFINITY, up_zhi: f32::NEG_INFINITY, up_ylo: f32::INFINITY, up_yhi: f32::NEG_INFINITY, ..Default::default() });
                        e.n += 1;
                        if ny > 0.8 { e.deck += 1; e.area_up += (len / 2.0) as f64; for v in &p { e.up_xlo = e.up_xlo.min(v[0]); e.up_xhi = e.up_xhi.max(v[0]); e.up_zlo = e.up_zlo.min(v[2]); e.up_zhi = e.up_zhi.max(v[2]); e.up_ylo = e.up_ylo.min(v[1]); e.up_yhi = e.up_yhi.max(v[1]); } }
                        else if ny < -0.8 { e.down += 1; }
                        else if ny.abs() > 0.5 {
                            e.slope += 1;
                            if std::env::var("CRYSTAL_SLOPES").is_ok() {
                                let ylo = p.iter().map(|v| v[1]).fold(f32::INFINITY, f32::min);
                                let yhi = p.iter().map(|v| v[1]).fold(f32::NEG_INFINITY, f32::max);
                                let xlo = p.iter().map(|v| v[0]).fold(f32::INFINITY, f32::min);
                                let xhi = p.iter().map(|v| v[0]).fold(f32::NEG_INFINITY, f32::max);
                                let zlo = p.iter().map(|v| v[2]).fold(f32::INFINITY, f32::min);
                                let zhi = p.iter().map(|v| v[2]).fold(f32::NEG_INFINITY, f32::max);
                                let key = (f.material, (ny * 20.0).round() as i32, (ylo * 10.0).round() as i32, (yhi * 10.0).round() as i32);
                                let b = slopes.entry(key).or_insert((0usize, 0f64, f32::INFINITY, f32::NEG_INFINITY, f32::INFINITY, f32::NEG_INFINITY));
                                b.0 += 1; b.1 += (len / 2.0) as f64; b.2 = b.2.min(xlo); b.3 = b.3.max(xhi); b.4 = b.4.min(zlo); b.5 = b.5.max(zhi);
                            }
                        }
                        else {
                            e.vert += 1;
                            if std::env::var("CRYSTAL_VERT_X").is_ok() {
                                // vertical faces: by x position (10 cm) and y band — where the curb's inner/outer faces stand
                                let xmid = p.iter().map(|v| v[0]).sum::<f32>() / p.len() as f32;
                                let ylo = p.iter().map(|v| v[1]).fold(f32::INFINITY, f32::min);
                                let yhi = p.iter().map(|v| v[1]).fold(f32::NEG_INFINITY, f32::max);
                                let nx = nrm[0] / len;
                                let key = (f.material, (xmid * 10.0).round() as i32, (ylo * 10.0).round() as i32, (yhi * 10.0).round() as i32, if nx > 0.5 { 1 } else if nx < -0.5 { -1 } else { 0 });
                                let b = vert.entry(key).or_insert((0usize, 0f64));
                                b.0 += 1; b.1 += (len / 2.0) as f64;
                            }
                        }
                        for v in &p { e.ylo = e.ylo.min(v[1]); e.yhi = e.yhi.max(v[1]); }
                        // the up-facing TrackBorders faces by height band (the curb top vs the border's own deck)
                        if ny > 0.8 && std::env::var("CRYSTAL_UP_BANDS").is_ok() {
                            let ymid = p.iter().map(|v| v[1]).sum::<f32>() / p.len() as f32;
                            let xmin = p.iter().map(|v| v[0]).fold(f32::INFINITY, f32::min);
                            let xmax = p.iter().map(|v| v[0]).fold(f32::NEG_INFINITY, f32::max);
                            let key = (f.material, (ymid * 20.0).round() as i32);
                            let b = bands.entry(key).or_insert((0usize, 0f64, f32::INFINITY, f32::NEG_INFINITY));
                            b.0 += 1; b.1 += (len / 2.0) as f64; b.2 = b.2.min(xmin); b.3 = b.3.max(xmax);
                        }
                    }
                    if std::env::var("CRYSTAL_SLOPES").is_ok() {
                        println!("    slope faces (0.5 < |ny| < 0.8) by (material, ny, y lo..hi): faces, area m2, x range, z range");
                        for ((m, nyb, ylo, yhi), (n, area, xlo, xhi, zlo, zhi)) in &slopes {
                            if *area < 0.3 { continue; }
                            let name = mats.get(*m as usize).cloned().unwrap_or_else(|| format!("#{m}"));
                            println!("      {}\tny≈{:.2}\ty {:.1}..{:.1}\t{n}\t{area:.1}\tx {xlo:.2}..{xhi:.2}\tz {zlo:.2}..{zhi:.2}", name.rsplit('\\').next().unwrap_or(&name), *nyb as f32 / 20.0, *ylo as f32 / 10.0, *yhi as f32 / 10.0);
                        }
                    }
                    if std::env::var("CRYSTAL_VERT_X").is_ok() {
                        println!("    vertical faces by (material, x 10 cm, y lo..hi, facing ±x): faces, area m2 (area ≥ 0.5 only)");
                        for ((m, xb, ylo, yhi, nx), (n, area)) in &vert {
                            if *area < 0.5 { continue; }
                            let name = mats.get(*m as usize).cloned().unwrap_or_else(|| format!("#{m}"));
                            println!("      {}\tx≈{:.1}\ty {:.1}..{:.1}\tfacing {}\t{n}\t{area:.1}", name.rsplit('\\').next().unwrap_or(&name), *xb as f32 / 10.0, *ylo as f32 / 10.0, *yhi as f32 / 10.0, match nx { 1 => "+x", -1 => "-x", _ => "±z" });
                        }
                    }
                    if std::env::var("CRYSTAL_UP_BANDS").is_ok() {
                        println!("    up-facing faces by (material, y band of 5 cm): faces, area m2, x range");
                        for ((m, yb), (n, area, xlo, xhi)) in &bands {
                            if *area < 0.05 { continue; }
                            let name = mats.get(*m as usize).cloned().unwrap_or_else(|| format!("#{m}"));
                            println!("      {}\ty≈{:.2}\t{n}\t{area:.1}\tx {xlo:.2}..{xhi:.2}", name.rsplit('\\').next().unwrap_or(&name), *yb as f32 / 20.0);
                        }
                    }
                    println!("  layer {li} \"{}\": {} faces", l.base.layer_name, c.faces.len());
                    println!("    material\tfaces\tup(ny>0.8)\tslope\tvertical\tdown\ty_range\tup_faces_y\tup_faces_x\tup_faces_z\tup_area_m2");
                    for (m, e) in &per {
                        let name = mats.get(*m as usize).cloned().unwrap_or_else(|| format!("#{m}"));
                        let upy = if e.deck > 0 { format!("{:.2}..{:.2}", e.up_ylo, e.up_yhi) } else { "-".into() };
                        let upx = if e.deck > 0 { format!("{:.2}..{:.2}", e.up_xlo, e.up_xhi) } else { "-".into() };
                        let upz = if e.deck > 0 { format!("{:.2}..{:.2}", e.up_zlo, e.up_zhi) } else { "-".into() };
                        println!("    {name}\t{}\t{}\t{}\t{}\t{}\t{:.2}..{:.2}\t{upy}\t{upx}\t{upz}\t{:.1}", e.n, e.deck, e.slope, e.vert, e.down, e.ylo, e.yhi, e.area_up);
                    }
                }
            }
        }
        // block-item NAME --collection RedIsland --out F.Item.Gbx [--ident NAME.Item.Gbx] [--author X] [--flags HEX|--ground]
        //   [--scale 1]: ONE block (a vista terrain variant: RoadTechStraightOnDirtCliff4…) as a full-size item through the
        //   tiny bake (deck + terrain prefabs merged, the collection skin's materials, the collision) — Hugo's vista items (2026-10-19)
        "block-item" => {
            let mut store = open(&a);
            let name = a.rest.get(1).cloned().unwrap_or_else(|| die("block-item NAME …".into()));
            let coll = flag(&a.rest, "--collection").unwrap_or_else(|| "Stadium".into());
            let collection = match coll.as_str() { "Stadium" => 0x1a, "RedIsland" => 0x10, "BlueBay" => 0x1c, "WhiteShore" => 0x1d, "GreenCoast" => 0xf, other => die(format!("unknown collection {other}")) };
            let out = flag(&a.rest, "--out").unwrap_or_else(|| die("block-item needs --out F".into()));
            let ident = flag(&a.rest, "--ident").unwrap_or_else(|| format!("{name}.Item.Gbx"));
            let author = flag(&a.rest, "--author").unwrap_or_else(|| "Nadeo".into());
            let scale: f32 = flag(&a.rest, "--scale").and_then(|v| v.parse().ok()).unwrap_or(1.0);
            let mut flags: u32 = flag(&a.rest, "--flags").and_then(|v| u32::from_str_radix(v.trim_start_matches("0x"), 16).ok()).unwrap_or(0);
            if a.rest.iter().any(|x| x == "--ground") { flags |= crate_ground_flag(); }
            let (bytes, m, src) = mapgeom::tiny_library::bake_named_block(&mut store, collection, &name, flags, &ident, &author, scale).unwrap_or_else(|e| die(e));
            std::fs::write(&out, &bytes).unwrap_or_else(|e| die(format!("{out}: {e}")));
            println!("wrote {out}: {} bytes from {src}", bytes.len());
            println!("  visuals {} materials {} collision tris {}", m.visuals.len(), m.materials.len(), m.surf_triangles.len());
            for (i, mat) in m.materials.iter().enumerate() {
                println!("  material {i}: {} physics {:?}", mat.link().unwrap_or("?"), mat.physics());
            }
            for n in m.notes.iter().take(40) {
                println!("  note: {n}");
            }
        }
        // vista-items MAP --out OUT [--only NAME,..] [--author X] [--items-dir D]: the map's vista variant blocks replaced
        //   by whole-variant embedded items (Hugo's vista items, 2026-10-19)
        "vista-items" => {
            let mut store = open(&a);
            let src = a.rest.get(1).cloned().unwrap_or_else(|| die("vista-items MAP …".into()));
            let out = flag(&a.rest, "--out").unwrap_or_else(|| die("vista-items needs --out OUT".into()));
            let only: Vec<String> = flag(&a.rest, "--only").map(|s| s.split(',').map(|x| x.trim().to_string()).collect()).unwrap_or_default();
            let author = flag(&a.rest, "--author").unwrap_or_else(|| "Nadeo".into());
            let items_dir = flag(&a.rest, "--items-dir").map(std::path::PathBuf::from);
            let o = mapgeom::vista_items::vista_items_map(&mut store, std::path::Path::new(&src), std::path::Path::new(&out), &only, &author, items_dir.as_deref()).unwrap_or_else(|e| die(e));
            for n in &o.notes { println!("  {n}"); }
            for r in &o.replaced { println!("  {} at cell {:?} dir {} -> {} at ({:.1}, {:.1}, {:.1}) yaw {:.4}", r.block, r.cell, r.dir, r.ident, r.pos[0], r.pos[1], r.pos[2], r.yaw); }
            println!("wrote {out}: {} item(s) embedded, {} block(s) replaced", o.items.len(), o.replaced.len());
        }
        // lights-census [--collection Stadium] [--blocks] [--items] [--filter SUB,..] [--out TSV]: every block and item of the
        //   loaded paks baked once (the tiny bake at scale 1) and its LIGHT SOURCES listed — real light nodes (pose, colour,
        //   intensity, range) and self-illuminated materials (an `_I` texture / a light-ish name) with the emitting geometry's
        //   local bbox (Hugo's Moth campaign, 2026-10-22)
        "lights-census" => {
            let mut store = open(&a);
            let coll = flag(&a.rest, "--collection").unwrap_or_else(|| "Stadium".into());
            let collection = match coll.as_str() { "Stadium" => 0x1a, "RedIsland" => 0x10, "BlueBay" => 0x1c, "WhiteShore" => 0x1d, "GreenCoast" => 0xf, other => die(format!("unknown collection {other}")) };
            let want_blocks = a.rest.iter().any(|x| x == "--blocks") || !a.rest.iter().any(|x| x == "--items");
            let want_items = a.rest.iter().any(|x| x == "--items") || !a.rest.iter().any(|x| x == "--blocks");
            let filters: Vec<String> = flag(&a.rest, "--filter").map(|s| s.split(',').map(|x| x.trim().to_ascii_lowercase()).collect()).unwrap_or_default();
            let keep = |n: &str| filters.is_empty() || filters.iter().any(|f| n.to_ascii_lowercase().contains(f));
            let mut cache = mapgeom::lights::MatCache::new();
            let mut rows: Vec<String> = vec!["kind\tname\tcollection\tsource\tlight_kind\tdetail\tparts\ttris\tbbox_local(x y z lo..hi)\tcolor\tintensity\trange\tsize_cells".to_string()];
            let mut n_models = 0usize;
            let mut n_with = 0usize;
            let mut errors = 0usize;
            let coll_folder = mapgeom::static_item::materials::env_name(collection);
            if want_blocks {
                // every block info of the collection: the Classic, Clip, Pillar and Flat folders, their `Theme\`
                // sub-folders included (the SnowRoad/Japanese-temple set lives in `…\Theme\`; a Classic-only
                // scan missed it and the StructurePillar family, 2026-10-23)
                let paths: Vec<String> = store.entries().map(|e| e.path()).filter(|p| {
                    let u = p.to_uppercase();
                    (u.ends_with(".EDCLASSIC.GBX") || u.ends_with(".EDCLIP.GBX") || u.ends_with(".EDVERTICALCLIP.GBX") || u.ends_with(".EDFLAT.GBX")) && u.starts_with(&format!("{}\\GAMECTNBLOCKINFO\\", coll_folder.to_uppercase()))
                }).collect();
                eprintln!("{} block infos in {coll_folder}", paths.len());
                for p in &paths {
                    let stem = p.rsplit('\\').next().unwrap_or(p);
                    let name = stem.rsplit_once('.').map(|(a, _)| a).unwrap_or(stem).trim_end_matches(".EDClassic").trim_end_matches(".EDClip").trim_end_matches(".EDVerticalClip").trim_end_matches(".EDFlat").to_string();
                    if !keep(&name) { continue; }
                    n_models += 1;
                    let flags = mapgeom::blockmap::FLAG_GROUND;
                    let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| mapgeom::tiny_library::bake_named_block(&mut store, collection, &name, flags, &format!("{name}.Item.Gbx"), "Nadeo", 1.0)));
                    let (m, size) = match r {
                        Ok(Ok((_, m, _))) => {
                            let size = mapgeom::vista::find_block_info(&store, &name).and_then(|bp| mapgeom::blockinfo::load(&mut store, &bp).ok()).and_then(|bi| {
                                let v = bi.variant_base_ground.as_ref().filter(|v| !v.block_units.is_empty()).or(bi.variant_base_air.as_ref())?;
                                let mut s = [0i32; 3];
                                for u in &v.block_units { for k in 0..3 { s[k] = s[k].max(u.offset[k] + 1); } }
                                Some(s)
                            });
                            (m, size)
                        }
                        Ok(Err(e)) => { errors += 1; rows.push(format!("block\t{name}\t{coll_folder}\t-\tERROR\t{}\t\t\t\t\t\t\t", e.replace('\t', " "))); continue; }
                        Err(_) => { errors += 1; rows.push(format!("block\t{name}\t{coll_folder}\t-\tERROR\tpanic in the bake\t\t\t\t\t\t\t")); continue; }
                    };
                    let (lights, em) = mapgeom::lights::census_of(&mut store, &mut cache, &m);
                    if !lights.is_empty() || !em.is_empty() { n_with += 1; }
                    let sz = size.map(|s| format!("{}x{}x{}", s[0], s[1], s[2])).unwrap_or_default();
                    for l in &lights {
                        rows.push(format!("block\t{name}\t{coll_folder}\t{}\tLIGHT\tat {:.2} {:.2} {:.2}\t1\t\t\t{:.2} {:.2} {:.2}\t{:.2}\t{:.1}\t{sz}", l.source, l.pos[0], l.pos[1], l.pos[2], l.color[0], l.color[1], l.color[2], l.intensity, l.range));
                    }
                    for e in &em {
                        rows.push(format!("block\t{name}\t{coll_folder}\t{}\tEMISSIVE\t{}\t{}\t{}\t{}\t\t\t\t{sz}", e.material.rsplit('\\').next().unwrap_or(&e.material), e.textures, e.parts, e.tris, mapgeom::lights::fmt_bbox(&e.bbox)));
                    }
                }
            }
            if want_items {
                let paths: Vec<String> = store.entries().map(|e| e.path()).filter(|p| {
                    let u = p.to_uppercase();
                    u.ends_with(".ITEM.GBX") && u.starts_with(&format!("{}\\ITEMS\\", coll_folder.to_uppercase()))
                }).collect();
                eprintln!("{} items in {coll_folder}", paths.len());
                for p in &paths {
                    let name = p.rsplit('\\').next().unwrap_or(p).trim_end_matches(".Item.Gbx").to_string();
                    if !keep(&name) { continue; }
                    n_models += 1;
                    // every VARIANT of a multi-variant item (ShowLights = 34 rig/light pieces; the placement's
                    // flags high byte picks one): the bake's note "variant k of N" gives N
                    let mut variant = 0usize;
                    let mut total = 1usize;
                    while variant < total && variant < 64 {
                        let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| mapgeom::static_item::build::pack_item_merged(&mut store, p, 1.0, collection, variant, None)));
                        let m = match r {
                            Ok(Ok(m)) => m,
                            Ok(Err(e)) => { errors += 1; rows.push(format!("item\t{name}\t{coll_folder}\t-\tERROR\t{}\t\t\t\t\t\t\t", e.replace('\t', " "))); break; }
                            Err(_) => { errors += 1; rows.push(format!("item\t{name}\t{coll_folder}\t-\tERROR\tpanic in the bake\t\t\t\t\t\t\t")); break; }
                        };
                        if variant == 0 {
                            for n in &m.notes {
                                if let Some(rest) = n.strip_prefix("variant 0 of ") {
                                    if let Some(t) = rest.split(|c: char| !c.is_ascii_digit()).next().and_then(|x| x.parse::<usize>().ok()) { total = t.max(1); }
                                }
                            }
                        }
                        let vname = if total > 1 { format!("{name}@v{variant}") } else { name.clone() };
                        let (lights, em) = mapgeom::lights::census_of(&mut store, &mut cache, &m);
                        if !lights.is_empty() || !em.is_empty() { n_with += 1; }
                        for l in &lights {
                            rows.push(format!("item\t{vname}\t{coll_folder}\t{}\tLIGHT\tat {:.2} {:.2} {:.2}\t1\t\t\t{:.2} {:.2} {:.2}\t{:.2}\t{:.1}\t", l.source, l.pos[0], l.pos[1], l.pos[2], l.color[0], l.color[1], l.color[2], l.intensity, l.range));
                        }
                        for e in &em {
                            rows.push(format!("item\t{vname}\t{coll_folder}\t{}\tEMISSIVE\t{}\t{}\t{}\t{}\t\t\t\t", e.material.rsplit('\\').next().unwrap_or(&e.material), e.textures, e.parts, e.tris, mapgeom::lights::fmt_bbox(&e.bbox)));
                        }
                        variant += 1;
                    }
                }
            }
            eprintln!("{n_models} models baked, {n_with} with a light source, {errors} errors");
            match flag(&a.rest, "--out") {
                Some(out) => { std::fs::write(&out, rows.join("\n") + "\n").unwrap_or_else(|e| die(format!("{out}: {e}"))); eprintln!("wrote {out} ({} rows)", rows.len() - 1); }
                None => for r in &rows { println!("{r}"); },
            }
        }
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
                if std::env::var("CRYSTAL_DEBUG").is_ok() {
                    for (li, l) in it.model.layers.iter().enumerate() {
                        if let Some(c) = l.kind.crystal() {
                            let mut edges = std::collections::HashSet::new();
                            for f in &c.faces {
                                for k in 0..f.verts.len() {
                                    let (a, b) = (f.verts[k], f.verts[(k + 1) % f.verts.len()]);
                                    edges.insert((a.min(b), a.max(b)));
                                }
                            }
                            let corners: usize = c.faces.iter().map(|f| f.verts.len()).sum();
                            let uniq_corner: std::collections::HashSet<(u32, u32)> = c.faces.iter().flat_map(|f| f.verts.iter().copied().zip(f.uv_index.iter().copied())).collect();
                            println!("  layer {li}: stored edge_count {} vs unique undirected face edges {}; {corners} corners, {} unique (position, texcoord) corner pairs", c.edge_count, edges.len(), uniq_corner.len());
                        }
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
        // veget-instances MAP [--no-rotation]: every vegetation ITEM of the map
        // (a placement whose model is a pack .Item.Gbx referencing
        // .VegetTreeModel.Gbx files), resolved to the species its variant byte
        // names and to the FOREST INSTANCE the game creates for it: the pose
        // hash, the scale draw, the yaw/tilt draws (veget_instance.rs — the exact
        // chain 0x03101002 fields -> 0x140d8d5f0 -> 0x141081910 -> 0x14026b4f0).
        // TSV: item, model, variant, species, pos, item yaw, quaternion after the
        // variation (w,x,y,z), scale, seed, yaw / tilt x / tilt z draws (radians).
        "veget-instances" => {
            let mut store = open(&a);
            let map = a.rest.get(1).cloned().unwrap_or_else(|| die("veget-instances MAP [--no-rotation]".into()));
            let with_rotation = !a.rest.iter().any(|x| x == "--no-rotation");
            // --runtime FILE: the GhostShooter /treeinst dump (i model flag qw qx qy qz x y z scale)
            // of the same map — every computed instance is matched by position and its
            // quaternion + scale compared BIT FOR BIT (the two must be identical f32s)
            let runtime = flag(&a.rest, "--runtime");
            let mut computed: Vec<([f32; 3], [f32; 4], f32, String)> = Vec::new();
            let source = tmmaps::map::MapFile::load(std::path::Path::new(&map));
            let mut species_cache: std::collections::BTreeMap<String, Result<Vec<String>, String>> = Default::default();
            let mut params_cache: std::collections::BTreeMap<String, Result<mapgeom::veget_instance::TreeParams, String>> = Default::default();
            if runtime.is_none() {
                println!("item\tmodel\tvariant\tspecies\tx\ty\tz\titem_yaw\tqw\tqx\tqy\tqz\tscale\tseed\tyaw\ttilt_x\ttilt_z");
            }
            for (i, it) in source.items.iter().enumerate() {
                let list = species_cache
                    .entry(it.model.clone())
                    .or_insert_with(|| {
                        let file = mapgeom::tiny_library::find_item_file(&store, &it.model).ok_or_else(|| format!("no pack item {}", it.model))?;
                        mapgeom::veget::item_species(&mut store, &file)
                    })
                    .clone();
                let Ok(list) = list else { continue };
                if list.is_empty() {
                    continue;
                }
                let v = it.variant() as usize;
                let Some(species) = list.get(v).or_else(|| list.first()) else { continue };
                let params = params_cache
                    .entry(species.clone())
                    .or_insert_with(|| {
                        mapgeom::veget::parse_tree_model(&mut store, species).map(|m| mapgeom::veget_instance::TreeParams { scale_var01: m.scale_var01, angle_max_rot_xz_deg: m.angle_max_rot_xz_deg, enable_random_rotation_y: m.enable_random_rotation_y != 0 })
                    })
                    .clone();
                let params = match params {
                    Ok(p) => p,
                    Err(e) => {
                        eprintln!("{species}: {e}");
                        continue;
                    }
                };
                let (q1, t, seed) = mapgeom::veget_instance::item_pose(it.yaw, it.pitch, it.roll, it.pos, it.pivot);
                let inst = mapgeom::veget_instance::variation(q1, t, seed, params, with_rotation);
                let stem = species.rsplit('\\').next().unwrap_or(species).trim_end_matches(".VegetTreeModel.Gbx");
                let (yaw, tx, tz) = inst.rotation.map(|(a, b, c)| (a.to_string(), b.to_string(), c.to_string())).unwrap_or_else(|| ("-".into(), "-".into(), "-".into()));
                if runtime.is_none() {
                    println!("i{i}\t{}\t{}\t{stem}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{:08x}\t{yaw}\t{tx}\t{tz}", it.model, v, t[0], t[1], t[2], it.yaw, inst.quat[0], inst.quat[1], inst.quat[2], inst.quat[3], inst.scale, inst.seed);
                }
                computed.push((t, inst.quat, inst.scale, format!("i{i} {} v{v} {stem}", it.model)));
            }
            if let Some(rt) = runtime {
                mapgeom::veget_instance::compare_runtime(&computed, &rt);
            }
        }
        // veget-slots --collection BlueBay [ZONE...]: every vegetation slot of
        // the collection's zone tiles (the block infos the zone genealogy
        // regenerates — Flat / Frontier / Transition families), resolved to
        // the VegetTreeModels its tags match in the placement-group items
        // (Items\Vegetation\<Placement>.Item.Gbx, an NPlugItem::SVariantList).
        // TSV: zone, variant, prefab, slot, x, y, z, yaw°, tags, candidates.
        "veget-slots" => {
            let mut store = open(&a);
            let coll = flag(&a.rest, "--collection").unwrap_or_else(|| "BlueBay".to_string());
            let only: Vec<String> = a.rest.iter().skip(1).filter(|x| !x.starts_with("--") && **x != coll).cloned().collect();
            let zones: Vec<String> = store
                .entries()
                .map(|e| e.path())
                .filter(|p| p.starts_with(&format!("{coll}\\GameCtnBlockInfo\\GameCtnBlockInfo")) && (p.contains("\\GameCtnBlockInfoFlat\\") || p.contains("\\GameCtnBlockInfoFrontier\\") || p.contains("\\GameCtnBlockInfoTransition\\")))
                .filter(|p| only.is_empty() || only.iter().any(|z| p.rsplit('\\').next().map(|f| f.starts_with(&format!("{z}."))).unwrap_or(false)))
                .collect();
            // the placement groups: Placement tag value -> [(tags, model path)]
            let mut groups: std::collections::BTreeMap<String, Vec<(Vec<(String, String)>, String)>> = Default::default();
            let group_paths: Vec<String> = store.entries().map(|e| e.path()).filter(|p| p.starts_with(&format!("{coll}\\Items\\Vegetation\\")) && p.ends_with(".Item.Gbx")).collect();
            for gp in &group_paths {
                let m = match store.load_model(gp) {
                    Ok(m) => m,
                    Err(_) => continue,
                };
                let mut lb = mapgeom::static_item::LookbackState::default();
                lb.defined_nodes.extend(m.external_indices().iter().copied());
                let mut r = mapgeom::static_item::Rd::new(&m.body, 0, lb);
                let item = match mapgeom::static_item::item::CGameItemModel::parse(&mut r) {
                    Ok(i) => i,
                    Err(_) => continue,
                };
                let Some(mc) = item.model() else { continue };
                let Some(mapgeom::static_item::Node::VariantList(vl)) = mc.entity_model.inline.as_deref() else { continue };
                let stem = gp.rsplit('\\').next().unwrap_or(gp).trim_end_matches(".Item.Gbx").to_string();
                let list = groups.entry(stem).or_default();
                for v in &vl.variants {
                    let model = m.externals.iter().find(|(k, _)| *k as i32 == v.model.index).map(|(_, p)| p.clone()).unwrap_or_else(|| format!("node {}", v.model.index));
                    list.push((v.tags.clone(), model));
                }
            }
            println!("zone\tvariant\tprefab\tslot\tx\ty\tz\tyaw_deg\ttags\tcandidates");
            for zp in &zones {
                let bi = match mapgeom::blockinfo::load(&mut store, zp) {
                    Ok(b) => b,
                    Err(e) => {
                        eprintln!("{zp}: {e}");
                        continue;
                    }
                };
                let zone = zp.rsplit('\\').next().unwrap_or(zp).split('.').next().unwrap_or("").to_string();
                let mut variants: Vec<(&mapgeom::blockinfo::Variant, String)> = Vec::new();
                if let Some(v) = &bi.variant_base_ground {
                    variants.push((v, "ground".into()));
                }
                for (i, v) in bi.additional_ground.iter().enumerate() {
                    variants.push((v, format!("ground+{}", i + 1)));
                }
                if let Some(v) = &bi.variant_base_air {
                    variants.push((v, "air".into()));
                }
                for (i, v) in bi.additional_air.iter().enumerate() {
                    variants.push((v, format!("air+{}", i + 1)));
                }
                for (v, label) in variants {
                    for layer in &v.mobils {
                        for mb in layer {
                            let Some(pp) = &mb.prefab else { continue };
                            let pm = match store.load_model(pp) {
                                Ok(m) => m,
                                Err(e) => {
                                    eprintln!("{pp}: {e}");
                                    continue;
                                }
                            };
                            let pf = match mapgeom::static_item::prefab::CPlugPrefab::from_model(&pm) {
                                Ok(p) => p,
                                Err(e) => {
                                    eprintln!("{pp}: {e}");
                                    continue;
                                }
                            };
                            for (i, e) in pf.ents.iter().enumerate() {
                                if e.params_id != 0x2F0A9000 {
                                    continue;
                                }
                                let mut r = mapgeom::crystal_model::Rd::new(&e.params, 0, Default::default());
                                let _v = r.u32().unwrap_or(0);
                                let _k = r.i32().unwrap_or(0);
                                let tag_groups: Vec<Vec<(String, String)>> = r.array(|r| r.array(|r| Ok((r.string()?, r.string()?)))).unwrap_or_default();
                                // Options = alternative tag sets; a variant qualifies when its
                                // tags carry every tag of one option (the Placement tag names the
                                // group in BlueBay's files; RedIsland's slots have none and are
                                // matched across every group item)
                                let mut cands: Vec<String> = Vec::new();
                                for option in &tag_groups {
                                    for list in groups.values() {
                                        for (vt, model) in list {
                                            if option.iter().all(|w| vt.iter().any(|t| t == w)) {
                                                let name = model.rsplit('\\').next().unwrap_or(model).trim_end_matches(".VegetTreeModel.Gbx").to_string();
                                                if !cands.contains(&name) {
                                                    cands.push(name);
                                                }
                                            }
                                        }
                                    }
                                }
                                let tags: Vec<(String, String)> = tag_groups.iter().flatten().cloned().collect();
                                // yaw about +Y from the quaternion (x, y, z, w)
                                let [qx, qy, qz, qw] = e.rot;
                                let yaw = (2.0 * (qw * qy + qx * qz)).atan2(1.0 - 2.0 * (qy * qy + qz * qz)).to_degrees();
                                let tag_s: Vec<String> = tag_groups.iter().map(|g| g.iter().map(|(k, v)| format!("{k}={v}")).collect::<Vec<_>>().join(",")).collect();
                                let _ = &tags;
                                println!("{zone}\t{label}\t{}\t{i}\t{:.3}\t{:.3}\t{:.3}\t{:.1}\t{}\t{}", pp.rsplit('\\').next().unwrap_or(pp), e.pos[0], e.pos[1], e.pos[2], yaw, tag_s.join("|"), cands.join(";"));
                            }
                        }
                    }
                }
            }
        }
        // prefab-ents <pack .Prefab.Gbx>: every entity of a prefab — model
        // class (or external file), position, rotation, params chunk id and
        // size — the layout an item in prefab form has to reproduce.
        "prefab-ents" => {
            let mut store = open(&a);
            let hexn = flag(&a.rest, "--params-hex");
            for path in a.rest.iter().skip(1).filter(|x| !x.starts_with("--") && Some(*x) != hexn.as_ref()) {
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
                    // NPlugItemPlacement_SPlacement: version, i32, [[ (key, value) ]] — the
                    // zone vegetation names its species here (the model ref is null)
                    if e.params_id == 0x2F0D8000 && hexn.is_some() {
                        // the spectator (crowd) placement block of the Stand prefabs (2026-10-18)
                        let n = e.params.len().min(hexn.as_ref().and_then(|v| v.parse().ok()).unwrap_or(256));
                        for (k, ch) in e.params[..n].chunks(32).enumerate() {
                            println!("    {:06x}: {}", k * 32, ch.iter().map(|b| format!("{b:02x}")).collect::<Vec<_>>().join(" "));
                        }
                        let floats: Vec<String> = e.params[..n.min(128)].chunks(4).map(|c| { let v = f32::from_le_bytes([c[0], c[1], c[2], c[3]]); if v.is_finite() && v.abs() < 1e5 { format!("{v:.3}") } else { format!("0x{:08x}", u32::from_le_bytes([c[0], c[1], c[2], c[3]])) } }).collect();
                        println!("    as f32/u32: {}", floats.join(" "));
                    }
                    if e.params_id == 0x2F0A9000 {
                        let mut r = mapgeom::crystal_model::Rd::new(&e.params, 0, Default::default());
                        let v = r.u32().unwrap_or(0);
                        let k = r.i32().unwrap_or(0);
                        let groups: Vec<Vec<(String, String)>> = r.array(|r| r.array(|r| Ok((r.string()?, r.string()?)))).unwrap_or_default();
                        println!("           SPlacement v{v} {k} {groups:?}");
                    }
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
        // every sub-function of a pack kinematic constraint (ease, direction, ms):
        // the authoring reference for a constraint of ours (the strip flag, 2026-09-12)
        "constraint" => {
            let mut store = open(&a);
            let p = a.rest.get(1).cloned().unwrap_or_default();
            let model = store.load_model(&p).unwrap_or_else(die);
            let kc = mapgeom::static_item::dyna::KinematicConstraint::parse_body(&model.body).unwrap_or_else(die);
            println!("{p}: {}", kc.summary());
            for (name, f) in [("trans", &kc.trans), ("rot", &kc.rot)] {
                println!("  {name} anim func u01={} ({} subs, {} ms):", f.u01, f.subs.len(), f.period_ms());
                for s in &f.subs {
                    println!("    ease {} reverse {} {} ms", s.ease, s.reverse, s.duration_ms);
                }
            }
            println!("  shader tc type {} version {} keyframes {:?} trans_sub {:?}", kc.shader_tc_type, kc.shader_tc_version, kc.shader_tc_anim, kc.shader_tc_trans_sub);
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
        // the decoration Scene3d's solids (island, sea, shadow casters) as one
        // OBJ, grouped by material name — the baker's `--decoration` input
        "scene3d" => {
            let mut store = open(&a);
            let p = a.rest.get(1).cloned().unwrap_or_default();
            let out = flag(&a.rest, "--out").unwrap_or_else(|| "scene3d.obj".to_string());
            let mut c = mapgeom::geom::Collector::new(&mut store);
            let r = mapgeom::scene3d::collect(&mut c, &p).unwrap_or_else(die);
            for (idx, name, tris) in &r.solids {
                if *idx >= 0 {
                    println!("  solid node {idx} {name:?}: {tris} triangles");
                } else {
                    println!("  external solid {name}: {tris} triangles");
                }
            }
            for (name, tris, bb) in &r.groups {
                println!("  group {name}: {tris} triangles, x {:.1}..{:.1} y {:.1}..{:.1} z {:.1}..{:.1}", bb[0], bb[3], bb[1], bb[4], bb[2], bb[5]);
            }
            println!("  {} triangles in all", r.triangles);
            write_scene(&c.scene, &out);
        }
        // mapgeom envblock --collection C [--out F.obj] [--sky F.obj] [--shadow F.obj] [--gpu DIR] [--compare-capture PASSCAP_ROOT [--frame N]]
        // — the lightmapper's environment block from the pack (envblock.rs): every leaf of the decoration's
        // Scene3d mobils with its role (peel caster + sun shadow / peel only / sky dome / excluded), the
        // OBJ of the peel casters, the dome, the sun-shadow subset, the GPU vertex/index buffers as the game
        // uploads them (--gpu DIR: <leaf>.vb / .ib), and the bit-for-bit comparison with a capture's
        // environment draws (passcap root: env/frame<N>/mesh.json + logs/mesh-frame<N>.json, the VB/IB dumps under env/frame<N>/mesh and mesh/frame<N>).
        "envblock" => {
            let mut store = open(&a);
            let coll = flag(&a.rest, "--collection").unwrap_or_else(|| "BlueBay".to_string());
            let b = mapgeom::envblock::load(&mut store, &coll).unwrap_or_else(die);
            println!("{}: {} leaves", b.layout_path, b.leaves.len());
            for l in &b.leaves {
                println!("  {}", mapgeom::envblock::describe(l));
            }
            let count = |role| b.by_role(role).map(|l| l.triangles()).sum::<usize>();
            println!(
                "  peel layer 0: {} triangles (caster+shadow {} + peel-only {}); sun shadow map: {}; sky dome: {}; excluded: {}",
                b.peel_leaves().map(|l| l.triangles()).sum::<usize>(),
                count(mapgeom::envblock::EnvRole::CasterAndShadow),
                count(mapgeom::envblock::EnvRole::PeelOnly),
                count(mapgeom::envblock::EnvRole::CasterAndShadow),
                count(mapgeom::envblock::EnvRole::SkyDome),
                count(mapgeom::envblock::EnvRole::Excluded)
            );
            use mapgeom::envblock::EnvRole::*;
            if let Some(o) = flag(&a.rest, "--out") {
                std::fs::write(&o, mapgeom::envblock::obj_text(&b, &[CasterAndShadow, PeelOnly])).unwrap_or_else(|e| die(format!("{o}: {e}")));
                println!("  wrote {o} (the peel's layer-0 casters)");
            }
            if let Some(o) = flag(&a.rest, "--shadow") {
                std::fs::write(&o, mapgeom::envblock::obj_text(&b, &[CasterAndShadow])).unwrap_or_else(|e| die(format!("{o}: {e}")));
                println!("  wrote {o} (the sun shadow map's casters)");
            }
            if let Some(o) = flag(&a.rest, "--sky") {
                std::fs::write(&o, mapgeom::envblock::obj_text(&b, &[SkyDome])).unwrap_or_else(|e| die(format!("{o}: {e}")));
                println!("  wrote {o} (the sky dome)");
            }
            if let Some(dir) = flag(&a.rest, "--gpu") {
                std::fs::create_dir_all(&dir).unwrap_or_else(|e| die(format!("{dir}: {e}")));
                for (i, l) in b.leaves.iter().enumerate() {
                    let stem = format!("{dir}/{i:02}_{}_{}", l.mobil, l.path.last().cloned().unwrap_or_default().replace([' ', '-'], "_"));
                    std::fs::write(format!("{stem}.vb"), l.gpu_vertex_buffer()).unwrap_or_else(|e| die(e.to_string()));
                    std::fs::write(format!("{stem}.ib"), l.gpu_index_buffer()).unwrap_or_else(|e| die(e.to_string()));
                }
                println!("  wrote {} vertex/index buffer pairs under {dir}", b.leaves.len());
            }
            if let Some(root) = flag(&a.rest, "--compare-capture") {
                let frame: u32 = flag(&a.rest, "--frame").and_then(|f| f.parse().ok()).unwrap_or(127448);
                let r = mapgeom::envblock::compare_capture(&b, std::path::Path::new(&root), frame).unwrap_or_else(die);
                for line in &r.lines {
                    println!("  {line}");
                }
                println!("  {} captured buffers matched bit for bit, {} differ, {} unmatched", r.exact, r.differ, r.unmatched);
                if r.differ + r.unmatched > 0 {
                    std::process::exit(1);
                }
            }
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
            // --discard-report PREFIX: the discard rows of this stage go to
            // `PREFIX-library.tsv` (tmmaps::discard; the same as TINY_DISCARD_REPORT)
            if let Some(p) = flag(&a.rest, "--discard-report") {
                std::env::set_var("TINY_DISCARD_REPORT", p);
            }
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
        "watercells" => {
            let mut store = open(&a);
            mapgeom::watercells::cmd(&mut store, &a.rest[1..]);
        }
        "shape-audit" => {
            let mut store = open(&a);
            mapgeom::shape_audit::cmd(&mut store, &a.rest[1..]);
        }
        // sttf MAP --out OUT [--dry-run] [--report R.tsv]: every checkpoint gone
        // (blocks → their plain twin by geometry, else removed; items removed;
        // the validation ghost dropped). See mapgeom::sttc.
        "sttf" => {
            let mut store = open(&a);
            let p = a.rest.get(1).cloned().unwrap_or_else(|| die("sttf needs a MAP path".into()));
            let dry = a.rest.iter().any(|x| x == "--dry-run");
            let out = flag(&a.rest, "--out").unwrap_or_else(|| if dry { "/dev/null".into() } else { die("sttf needs --out OUT.Map.Gbx".into()) });
            let m = tmmaps::map::MapFile::load(std::path::Path::new(&p));
            let mut ctx = mapgeom::sttc::Ctx::new(&mut store, &m);
            let o = mapgeom::sttc::sttf(&mut ctx, std::path::Path::new(&p), std::path::Path::new(&out), sttc_cp_mode(&a.rest), dry).unwrap_or_else(die);
            let mut tsv = String::from(mapgeom::sttc::REPORT_HEADER);
            tsv.push('\n');
            for r in &o.rows {
                tsv.push_str(&r.tsv());
                tsv.push('\n');
            }
            match flag(&a.rest, "--report") {
                Some(r) => {
                    std::fs::write(&r, &tsv).unwrap_or_else(|e| die(e.to_string()));
                    println!("wrote {r}");
                }
                None => print!("{tsv}"),
            }
            println!(
                "{p}: {} checkpoint blocks replaced, {} removed (+{} baked), {} checkpoint items removed; {} tag/model mismatches, {} unresolved models{}",
                o.replaced, o.removed_blocks, o.removed_baked, o.removed_items, o.mismatches, o.unresolved,
                if dry { " (dry run)" } else { "" }
            );
            if !dry {
                let bad = mapgeom::sttc::verify_sttf(std::path::Path::new(&p), std::path::Path::new(&out), &o).unwrap_or_else(die);
                if bad.is_empty() {
                    println!("verified {out}");
                } else {
                    die::<()>(format!("VERIFY FAILED on {out}:\n  {}", bad.join("\n  ")));
                }
            }
        }
        // center-finish MAP --out OUT [--center auto|bbox|X,Z] [--occupied overlap|raise|fail]
        //   [--include-startfinish] [--no-rename] [--no-reuid] [--dry-run] [--report R.tsv]
        "center-finish" => {
            let mut store = open(&a);
            let p = a.rest.get(1).cloned().unwrap_or_else(|| die("center-finish needs a MAP path".into()));
            let dry = a.rest.iter().any(|x| x == "--dry-run");
            let out = flag(&a.rest, "--out").unwrap_or_else(|| if dry { "/dev/null".into() } else { die("center-finish needs --out OUT.Map.Gbx".into()) });
            let opts = sttc_center_opts(&a.rest);
            let m = tmmaps::map::MapFile::load(std::path::Path::new(&p));
            let mut ctx = mapgeom::sttc::Ctx::new(&mut store, &m);
            let o = mapgeom::sttc::center_finish(&mut ctx, std::path::Path::new(&p), std::path::Path::new(&out), &opts, dry).unwrap_or_else(die);
            let mut tsv = String::from(mapgeom::sttc::REPORT_HEADER);
            tsv.push('\n');
            for r in &o.rows {
                tsv.push_str(&r.tsv());
                tsv.push('\n');
            }
            match flag(&a.rest, "--report") {
                Some(r) => {
                    std::fs::write(&r, &tsv).unwrap_or_else(|e| die(e.to_string()));
                    println!("wrote {r}");
                }
                None => print!("{tsv}"),
            }
            println!(
                "{p}: {} finish blocks + {} finish items (+{} baked) moved by ({}, {}, {}) cells; centroid ({:.1}, {:.1}) -> centre ({:.1}, {:.1}); {} destination overlaps, {} StartFinish left, {} mismatches; name {:?} uid {}{}",
                o.finish_blocks, o.finish_items, o.moved_baked, o.offset_cells.0, o.offset_cells.1, o.offset_cells.2, o.centroid.0, o.centroid.1, o.center.0, o.center.1, o.overlaps, o.startfinish, o.mismatches, o.new_name, o.new_uid,
                if dry { " (dry run)" } else { "" }
            );
            if !dry {
                let bad = mapgeom::sttc::verify_center(std::path::Path::new(&p), std::path::Path::new(&out), &o).unwrap_or_else(die);
                if bad.is_empty() {
                    println!("verified {out}");
                } else {
                    die::<()>(format!("VERIFY FAILED on {out}:\n  {}", bad.join("\n  ")));
                }
            }
        }
        // sttc MAP|DIR --out-dir DIR [center-finish flags] [--dry-run] [--report R.tsv]:
        // both steps per map (a directory = every *.Map.Gbx in it, sorted);
        // outputs DIR/sttf/<stem>.sttf.Map.Gbx and DIR/<stem>-Straight-to-the-Center.Map.Gbx
        // blockinfo-resolve NAME..: the pack path the block-info index resolves a block name to
        // flag-carriers --out-dir DIR [--models Flag16m,Flag8m] [--ident-prefix Finish] [--collection 26] [--cloth-pad 0.3
        // --cloth-thick-pad 0.35]: the flag carrier items ALONE (no map), the internal ident = `<prefix><model>.Item.Gbx` —
        // Hugo's renamed files (2026-10-18: FinishFlag16m.Item.Gbx whose header still said ClothFinishTriggerFlag16m)
        "flag-carriers" => {
            let mut store = open(&a);
            let out_dir = std::path::PathBuf::from(flag(&a.rest, "--out-dir").unwrap_or_else(|| die("flag-carriers needs --out-dir DIR".into())));
            std::fs::create_dir_all(&out_dir).unwrap_or_else(|e| die(e.to_string()));
            let models: Vec<String> = flag(&a.rest, "--models").map(|v| v.split(',').map(|s| s.trim().to_string()).collect()).unwrap_or_else(|| vec!["Flag16m".into(), "Flag8m".into()]);
            let prefix = flag(&a.rest, "--ident-prefix").unwrap_or_else(|| "Finish".into());
            let collection: u32 = flag(&a.rest, "--collection").and_then(|v| v.parse().ok()).unwrap_or(26);
            let pad: f32 = flag(&a.rest, "--cloth-pad").and_then(|v| v.parse().ok()).unwrap_or(0.3);
            let thick_pad: f32 = flag(&a.rest, "--cloth-thick-pad").and_then(|v| v.parse().ok()).unwrap_or(0.35);
            let mut ctx = mapgeom::sttc::Ctx::for_collection(&mut store, "Stadium");
            println!("model\tident\tbytes\tslab");
            for stem in &models {
                let pack_path = format!("Stadium\\Items\\{stem}.Item.Gbx");
                let ident = format!("{prefix}{stem}.Item.Gbx");
                let (_cx, _cz, pr, _y0, _y1) = mapgeom::flags::pole_of(&mut ctx, &pack_path, collection).unwrap_or_else(|e| die(format!("{stem}: {e}")));
                let c = mapgeom::flags::cloth_of(&mut ctx, &pack_path).unwrap_or_else(|e| die(format!("{stem}: {e}")));
                let mut mg = mapgeom::static_item::build::pack_item_merged(ctx.store, &pack_path, 1.0, collection, 0, None).unwrap_or_else(|e| die(format!("{stem}: {e}")));
                let t = c.tilt_bottom_deg.to_radians().tan();
                let (z_near, z_far) = (c.z0 + pr, c.z0 + c.reach + pad);
                let drop = t * (z_far - z_near);
                let (xa, xb) = (c.x_lo - thick_pad, c.x_hi + thick_pad);
                let (ya, yb) = (c.y_lo - pad, c.y_hi + pad);
                let corners: [[f32; 3]; 8] = [[xa, ya, z_near], [xb, ya, z_near], [xb, yb, z_near], [xa, yb, z_near], [xa, ya - drop, z_far], [xb, ya - drop, z_far], [xb, yb - drop, z_far], [xa, yb - drop, z_far]];
                mapgeom::static_item::build::make_invisible_hexahedron_waypoint(&mut mg, mapgeom::sttc::WP_FINISH, corners).unwrap_or_else(|e| die(format!("{stem}: {e}")));
                let (bytes, _) = mapgeom::static_item::build::finish_item(mg, &ident, &ident, 1.0, collection).unwrap_or_else(|e| die(format!("{stem}: {e}")));
                let p = out_dir.join(&ident);
                std::fs::write(&p, &bytes).unwrap_or_else(|e| die(format!("{}: {e}", p.display())));
                println!("{stem}\t{ident}\t{}\tx {xa:.2}..{xb:.2} y {ya:.2}..{yb:.2} at z {z_near:.2} -> {:.2}..{:.2} at z {z_far:.2}", bytes.len(), ya - drop, yb - drop);
            }
        }
        // cp-effect MAP --out OUT --effect NAME[@dx,dy,dz] [--effect …]: at every CHECKPOINT ITEM GateCheckpoint(Left|
        // Right|Center)<w>m[v2], the effect gate item GateSpecial<w>m<NAME> at the checkpoint's pose (+ the local offset,
        // metres in the checkpoint item's frame: +z = the gate's own forward), same yaw; the checkpoints kept. Hugo's
        // CPEffect reference output (2026-10-18). Pack items (no embedding); the lightmap is left as it is.
        "cp-effect" => {
            let mp = a.rest.get(1).cloned().unwrap_or_else(|| die("cp-effect MAP --out OUT --effect NAME[@dx,dy,dz]..".into()));
            let out = flag(&a.rest, "--out").unwrap_or_else(|| die("cp-effect needs --out OUT.Map.Gbx".into()));
            let mut effects: Vec<(String, [f32; 3])> = Vec::new();
            let mut i = 0;
            while i < a.rest.len() {
                if a.rest[i] == "--effect" {
                    let v = a.rest.get(i + 1).cloned().unwrap_or_default();
                    let (name, off) = match v.split_once('@') {
                        Some((n, o)) => {
                            let f: Vec<f32> = o.split(',').map(|x| x.trim().parse::<f32>().unwrap_or_else(|_| die(format!("--effect {v}: offset dx,dy,dz")))).collect();
                            if f.len() != 3 { die::<()>(format!("--effect {v}: offset dx,dy,dz")); }
                            (n.to_string(), [f[0], f[1], f[2]])
                        }
                        None => (v.clone(), [0.0, 0.0, 0.0]),
                    };
                    effects.push((name, off));
                    i += 2;
                } else {
                    i += 1;
                }
            }
            if effects.is_empty() { die::<()>("cp-effect needs at least one --effect".into()); }
            let re_w = |model: &str| -> Option<u32> {
                let rest = model.strip_prefix("GateCheckpoint")?;
                let rest = rest.strip_prefix("Left").or_else(|| rest.strip_prefix("Right")).or_else(|| rest.strip_prefix("Center"))?;
                let digits: String = rest.chars().take_while(|c| c.is_ascii_digit()).collect();
                let tail = &rest[digits.len()..];
                if tail != "m" && tail != "mv2" { return None; }
                digits.parse().ok()
            };
            let src = std::path::Path::new(&mp);
            let outp = std::path::Path::new(&out);
            // --start-blocks: the StartEffect blocks of Hugo's tool, as his output has them — a grid
            // GateSpecial<Effect> at the start cell (flags 0x8000: author "AutoAltTag"; his 0x00108000 also carries a stray waypoint node) and a FREE
            // GateSpecial<Effect2> 3 m along the start's forward (his Offset(0,0,3) on a −π/2 start: +x) …
            // given explicitly as NAME@cell:cx,cy,cz:dir or NAME@free:x,y,z:yaw (repeatable)
            let mut start_blocks: Vec<tmmaps::map::FreeBlockSpec> = Vec::new();
            {
                let mut i = 0;
                while i < a.rest.len() {
                    if a.rest[i] == "--start-block" {
                        let v = a.rest.get(i + 1).cloned().unwrap_or_default();
                        let parts: Vec<&str> = v.split('@').collect();
                        let (name, spec) = (parts.first().copied().unwrap_or(""), parts.get(1).copied().unwrap_or(""));
                        let sp: Vec<&str> = spec.split(':').collect();
                        match sp.first().copied() {
                            Some("cell") => {
                                let c: Vec<i32> = sp.get(1).unwrap_or(&"").split(',').map(|x| x.trim().parse::<i32>().unwrap_or_else(|_| die(format!("--start-block {v}: cell cx,cy,cz")))).collect();
                                let dir: u8 = sp.get(2).and_then(|d| d.parse().ok()).unwrap_or(0);
                                start_blocks.push(tmmaps::map::FreeBlockSpec { name: name.to_string(), author: Some("AutoAltTag".into()), flags: 0x8000, pos: [0.0; 3], rot: [0.0; 3], grid: Some([c[0], c[1], c[2]]), dir, waypoint: None });
                            }
                            Some("free") => {
                                let p: Vec<f32> = sp.get(1).unwrap_or(&"").split(',').map(|x| x.trim().parse::<f32>().unwrap_or_else(|_| die(format!("--start-block {v}: free x,y,z")))).collect();
                                let yaw: f32 = sp.get(2).and_then(|d| d.parse().ok()).unwrap_or(0.0);
                                start_blocks.push(tmmaps::map::FreeBlockSpec { name: name.to_string(), author: Some("AutoAltTag".into()), flags: 0x2000_8000, pos: [p[0], p[1], p[2]], rot: [yaw, 0.0, 0.0], grid: None, dir: 0, waypoint: None });
                            }
                            _ => die::<()>(format!("--start-block {v}: NAME@cell:cx,cy,cz:dir or NAME@free:x,y,z:yaw")),
                        }
                        i += 2;
                    } else {
                        i += 1;
                    }
                }
            }
            let m0 = tmmaps::map::MapFile::try_load(src).unwrap_or_else(die);
            let collection = m0.body_collections().map(|c| c[0].1).unwrap_or(26);
            let ground = tmmaps::map::ground_y(collection);
            // the plan: (checkpoint index, special model, world pos, yaw)
            let mut plan: Vec<(usize, String, [f32; 3], f32)> = Vec::new();
            for it in &m0.items {
                let Some(w) = re_w(&it.model) else { continue };
                let frame = mapgeom::place::anchored(it.pos, [it.yaw, it.pitch, it.roll], [0.0; 3], 1.0);
                for (name, off) in &effects {
                    let p = mapgeom::geom::apply(&frame, *off);
                    plan.push((it.index, format!("GateSpecial{w}m{name}"), p, it.yaw));
                }
            }
            if plan.is_empty() { die::<()>("no GateCheckpoint(Left|Right|Center)<w>m[v2] item in the map".into()); }
            let base = m0.items.len();
            std::fs::copy(src, outp).unwrap_or_else(|e| die(format!("{out}: {e}")));
            // 0. the start-effect blocks (a variable-length block edit: its own write)
            if !start_blocks.is_empty() {
                let mut mm = tmmaps::map::MapFile::try_load(outp).unwrap_or_else(die);
                mm.remove_and_add_blocks(|_| false, |_| false, &start_blocks);
                mm.write_to(outp).unwrap_or_else(|e| die(e.to_string()));
                let mm = tmmaps::map::MapFile::try_load(outp).unwrap_or_else(die);
                for b in mm.blocks.iter().rev().take(start_blocks.len()) {
                    println!("start block: #{} {} cell {:?} dir {} flags {:08X} free {:?}", b.index, b.name, b.coords(), b.dir, b.flags, b.free_pos);
                }
            }
            // 1. clones appended
            {
                let mut mm = tmmaps::map::MapFile::try_load(outp).unwrap_or_else(die);
                mm.append_item_clones(base + plan.len());
                mm.write_to(outp).unwrap_or_else(|e| die(e.to_string()));
            }
            // 2. the clones' waypoint nodes stripped (the donor may be a checkpoint)
            {
                let mut mm = tmmaps::map::MapFile::try_load(outp).unwrap_or_else(die);
                for k in 0..plan.len() {
                    if mm.items[base + k].waypoint_tag.is_some() {
                        mm.set_item_waypoint(base + k, None, 0);
                    }
                }
                mm.write_to(outp).unwrap_or_else(|e| die(e.to_string()));
            }
            // 3. model + author (renames)
            {
                let mut mm = tmmaps::map::MapFile::try_load(outp).unwrap_or_else(die);
                for (k, (_, model, _, _)) in plan.iter().enumerate() {
                    mm.set_item_model(base + k, model);
                    mm.set_item_author(base + k, "Nadeo");
                    mm.set_item_collection(base + k, collection);
                }
                mm.write_to(outp).unwrap_or_else(|e| die(e.to_string()));
            }
            // 4. pose / pivot / flags / scale / cell (patches)
            {
                let mut mm = tmmaps::map::MapFile::try_load(outp).unwrap_or_else(die);
                for (k, (_, _, pos, yaw)) in plan.iter().enumerate() {
                    let i = base + k;
                    let it = mm.items[i].clone();
                    if it.skin_region.is_some() || it.flags & 4 != 0 {
                        mm.set_item_skin(i, None);
                    }
                    mm.move_item_pos(i, *pos);
                    mm.set_item_rotation(i, *yaw, 0.0, 0.0);
                    mm.set_item_pivot(i, [0.0, 0.0, 0.0]);
                    mm.set_item_flags(i, 0);
                    mm.set_item_scale(i, 1.0);
                    let cell = [((pos[0] / 32.0).floor() as i32).clamp(0, 255) as u8, (((pos[1] - ground) / 8.0).floor() as i32).clamp(0, 255) as u8, ((pos[2] / 32.0).floor() as i32).clamp(0, 255) as u8];
                    mm.set_item_cell(i, cell);
                }
                mm.write_to(outp).unwrap_or_else(|e| die(e.to_string()));
            }
            let m1 = tmmaps::map::MapFile::try_load(outp).unwrap_or_else(die);
            println!("checkpoint\tmodel\tpose\t→ item\teffect model\tpose");
            for (k, (ci, model, pos, yaw)) in plan.iter().enumerate() {
                let c = &m0.items[*ci];
                let n = &m1.items[base + k];
                println!("#{ci}\t{}\t({:.2}, {:.2}, {:.2}) yaw {:.4}\t#{}\t{}\t({:.2}, {:.2}, {:.2}) yaw {:.4} tag {:?} cell {:?}", c.model, c.pos[0], c.pos[1], c.pos[2], c.yaw, base + k, n.model, n.pos[0], n.pos[1], n.pos[2], n.yaw, n.waypoint_tag, n.file_cell);
                if n.model != *model || (n.pos[0] - pos[0]).abs() > 1e-3 || (n.pos[1] - pos[1]).abs() > 1e-3 || (n.pos[2] - pos[2]).abs() > 1e-3 || (n.yaw - yaw).abs() > 1e-5 || n.waypoint_tag.is_some() {
                    die::<()>(format!("item #{} did not land as planned", base + k));
                }
            }
            println!("wrote {out}: {} items ({} checkpoints kept, {} effect gates added)", m1.items.len(), m0.items.len(), plan.len());
        }
        // gate-xhist ITEM..: the visual vertex histogram along local x (1 m bins) and the vertices below y 0 — where a
        // Left/Right checkpoint gate's wall post stands (2026-10-18)
        "gate-xhist" => {
            let mut store = open(&a);
            for stem in a.rest.iter().skip(1).filter(|x| !x.starts_with("--")) {
                let p = format!("Stadium\\Items\\{stem}.Item.Gbx");
                let loaded = store.load_model(&p).unwrap_or_else(die);
                let mut c = mapgeom::geom::Collector::new(&mut store);
                c.model(&loaded, &mapgeom::geom::IDENTITY, 0);
                let mut bins: std::collections::BTreeMap<i32, (usize, f32)> = Default::default();
                let mut below: Vec<[f32; 3]> = Vec::new();
                for g in c.scene.groups.values() {
                    for v in &g.verts {
                        let e = bins.entry(v[0].floor() as i32).or_insert((0, f32::NEG_INFINITY));
                        e.0 += 1;
                        e.1 = e.1.max(v[1]);
                        if v[1] < -0.01 {
                            below.push(*v);
                        }
                    }
                }
                println!("{stem}: x-bin → (vertices, max y): {}", bins.iter().map(|(b, (n, y))| format!("[{b}..{}) {n}/{y:.1}", b + 1)).collect::<Vec<_>>().join("  "));
                // the y histogram too (1 m bins): a start gate's crossbar vs a full arch
                let mut ybins: std::collections::BTreeMap<i32, usize> = Default::default();
                for g in c.scene.groups.values() {
                    for v in &g.verts {
                        *ybins.entry(v[1].floor() as i32).or_insert(0) += 1;
                    }
                }
                println!("  y-bin → vertices: {}", ybins.iter().map(|(b, n)| format!("[{b}..{}) {n}", b + 1)).collect::<Vec<_>>().join("  "));
                if !below.is_empty() {
                    let (lo, hi) = below.iter().fold(([f32::INFINITY; 3], [f32::NEG_INFINITY; 3]), |(l, h), v| ([l[0].min(v[0]), l[1].min(v[1]), l[2].min(v[2])], [h[0].max(v[0]), h[1].max(v[1]), h[2].max(v[2])]));
                    println!("  below y 0: {} vertices in [{:.2} {:.2} {:.2}]..[{:.2} {:.2} {:.2}]", below.len(), lo[0], lo[1], lo[2], hi[0], hi[1], hi[2]);
                }
            }
        }
        // gate-inventory [--filter GateCheckpoint,GateSpecial] [--collection Stadium]: every pack ITEM whose name matches,
        // with its visual bbox (the prefab rendered in the item frame), the trigger shape (waypoint / special gate)
        // extents, and the pivot: where the item origin sits relative to the geometry (Hugo's CPEffect table, 2026-10-18)
        "gate-inventory" => {
            let mut store = open(&a);
            let pats: Vec<String> = flag(&a.rest, "--filter").map(|v| v.split(',').map(|s| s.trim().to_string()).collect()).unwrap_or_else(|| vec!["GateCheckpoint".into(), "GateSpecial".into(), "GateFinish".into(), "GateStart".into()]);
            let coll = flag(&a.rest, "--collection").unwrap_or_else(|| "Stadium".into());
            let mut names: Vec<String> = store.entries().map(|e| e.path()).filter(|p| { let up = p.to_uppercase(); up.starts_with(&format!("{}\\ITEMS\\", coll.to_uppercase())) && up.ends_with(".ITEM.GBX") && pats.iter().any(|q| up.contains(&q.to_uppercase())) }).collect();
            names.sort();
            println!("item\tvisual_bbox_lo\tvisual_bbox_hi\twidth_x\theight_y\tdepth_z\ttrigger_lo\ttrigger_hi\ttrigger_kind\tspawn\tnotes");
            for p in &names {
                let stem = p.rsplit('\\').next().unwrap_or(p).trim_end_matches(".Item.Gbx").to_string();
                let loaded = match store.load_model(p) { Ok(m) => m, Err(e) => { println!("{stem}\tFAILED {e}"); continue; } };
                let mut c = mapgeom::geom::Collector::new(&mut store);
                c.model(&loaded, &mapgeom::geom::IDENTITY, 0);
                let (mut lo, mut hi) = ([f32::INFINITY; 3], [f32::NEG_INFINITY; 3]);
                let mut n = 0usize;
                for g in c.scene.groups.values() {
                    for v in &g.verts {
                        for k in 0..3 {
                            lo[k] = lo[k].min(v[k]);
                            hi[k] = hi[k].max(v[k]);
                        }
                        n += 1;
                    }
                }
                drop(c);
                let vb = if n > 0 { (format!("[{:.2} {:.2} {:.2}]", lo[0], lo[1], lo[2]), format!("[{:.2} {:.2} {:.2}]", hi[0], hi[1], hi[2]), format!("{:.2}", hi[0] - lo[0]), format!("{:.2}", hi[1] - lo[1]), format!("{:.2}", hi[2] - lo[2])) } else { ("-".into(), "-".into(), "-".into(), "-".into(), "-".into()) };
                // the trigger: the merged item's waypoint/special trigger surface
                let (tlo, thi, tkind, spawn, notes) = match mapgeom::static_item::build::pack_item_merged(&mut store, p, 1.0, 26, 0, None) {
                    Ok(mg) => {
                        let tri = mg.trigger.as_ref().or(mg.special.as_ref());
                        match tri.and_then(|sf| sf.surf.triangulate()) {
                            Some((verts, tris)) => {
                                let (mut l, mut h) = ([f32::INFINITY; 3], [f32::NEG_INFINITY; 3]);
                                for v in &verts {
                                    for k in 0..3 {
                                        l[k] = l[k].min(v[k]);
                                        h[k] = h[k].max(v[k]);
                                    }
                                }
                                (format!("[{:.2} {:.2} {:.2}]", l[0], l[1], l[2]), format!("[{:.2} {:.2} {:.2}]", h[0], h[1], h[2]), format!("{} ({} tris)", if mg.trigger.is_some() { "waypoint" } else { "special" }, tris.len()), format!("[{:.2} {:.2} {:.2}]", mg.spawn[0], mg.spawn[1], mg.spawn[2]), format!("wp {:?}", mg.waypoint_type))
                            }
                            None => ("-".into(), "-".into(), "none".into(), format!("[{:.2} {:.2} {:.2}]", mg.spawn[0], mg.spawn[1], mg.spawn[2]), format!("wp {:?}", mg.waypoint_type)),
                        }
                    }
                    Err(e) => ("-".into(), "-".into(), "?".into(), "-".into(), format!("merge failed: {}", e.lines().next().unwrap_or(""))),
                };
                println!("{stem}\t{}\t{}\t{}\t{}\t{}\t{tlo}\t{thi}\t{tkind}\t{spawn}\t{notes}", vb.0, vb.1, vb.2, vb.3, vb.4);
            }
        }
        // stand-carriers --out-dir DIR [--kinds A,B] [--pad-below 0.3] [--thick 1.5] [--collection 26]: Hugo's
        // Manslaughter carriers — one invisible Finish item per stand block kind, trigger = the seating slab
        "stand-carriers" => {
            let mut store = open(&a);
            let out_dir = std::path::PathBuf::from(flag(&a.rest, "--out-dir").unwrap_or_else(|| die("stand-carriers needs --out-dir DIR".into())));
            std::fs::create_dir_all(&out_dir).unwrap_or_else(|e| die(e.to_string()));
            let kinds: Vec<String> = flag(&a.rest, "--kinds").map(|v| v.split(',').map(|s| s.trim().to_string()).collect()).unwrap_or_else(|| mapgeom::stands::STANDS.iter().map(|(k, _)| k.to_string()).collect());
            // Hugo's tweak (05:49 PT): the bottom skin ON the step surface, 0.5 m above it
            let pad: f32 = flag(&a.rest, "--pad-below").and_then(|v| v.parse().ok()).unwrap_or(0.0);
            let thick: f32 = flag(&a.rest, "--thick").and_then(|v| v.parse().ok()).unwrap_or(0.5);
            let collection: u32 = flag(&a.rest, "--collection").and_then(|v| v.parse().ok()).unwrap_or(26);
            // --ident-prefix Finish: Hugo's names (FinishStandStraight.Item.Gbx …) instead of <Kind>FinishTrigger
            let prefix = flag(&a.rest, "--ident-prefix").unwrap_or_default();
            println!("kind\tident\tbytes\trows\tseat_y\tseat_tris\tslab_tris\tbbox\tmaterials");
            for k in &kinds {
                let c = mapgeom::stands::carrier_item_named(&mut store, k, &prefix, collection, pad, thick, None).unwrap_or_else(|e| die(format!("{k}: {e}")));
                let p = out_dir.join(&c.ident);
                std::fs::write(&p, &c.bytes).unwrap_or_else(|e| die(format!("{}: {e}", p.display())));
                let mats: Vec<String> = c.seating.census.iter().map(|(m, (n, up, lo, hi, area))| format!("{m} {n} tris ({up} up) y {lo:.2}..{hi:.2} {area:.0} m2{}", if m.eq_ignore_ascii_case(mapgeom::stands::SEAT_MATERIAL) { " ← SEATING" } else { "" })).collect();
                println!("{k}\t{}\t{}\t{}\t{:.2}..{:.2}\t{}\t{}\t[{:.2} {:.2} {:.2}]..[{:.2} {:.2} {:.2}]\t{}", c.ident, c.bytes.len(), c.seating.rows, c.seating.y_lo, c.seating.y_hi, c.seating.tris.len(), c.slab_tris, c.bbox[0], c.bbox[1], c.bbox[2], c.bbox[3], c.bbox[4], c.bbox[5], mats.join(" | "));
                if a.rest.iter().any(|x| x == "--verbose") {
                    for n in &c.notes {
                        eprintln!("  {k}: {n}");
                    }
                }
                let th = mapgeom::stands::tread_heights(&c.seating);
                eprintln!("  {k}: tread heights ({}): {}", th.len(), th.iter().map(|v| format!("{v:.2}")).collect::<Vec<_>>().join(" "));
            }
        }
        "blockinfo-resolve" => {
            let mut store = open(&a);
            let coll = flag(&a.rest, "--collection").unwrap_or_else(|| "Stadium".into());
            let mut idx = mapgeom::blockmap::BlockInfoIndex::build(&mut store, &coll);
            for n in a.rest.iter().skip(1) {
                match idx.resolve_one(&mut store, n) {
                    Some(p) => {
                        let bi = idx.load(&mut store, &p).map(|b| format!("waypoint {:?} modifier slots {:?} / {:?}", b.waypoint_type, b.material_modifier_slots, b.material_modifier)).unwrap_or_else(|e| e);
                        println!("{n}: {p}\n   {bi}");
                    }
                    None => println!("{n}: NOT RESOLVED"),
                }
            }
        }
        // dyna-frames <Flag.DynaObject.Gbx path> [--prefab-pos X,Y,Z --prefab-rot QX,QY,QZ,QW]: every vertex-animation
        // frame of a dyna mesh (the flag cloth: 86 frames) → per frame the bbox in the given frame; the union bbox
        "dyna-frames" => {
            let mut store = open(&a);
            let p = a.rest.get(1).cloned().unwrap_or_else(|| die("dyna-frames PATH".into()));
            let mut mg = mapgeom::static_item::build::Merged::default();
            let src = mapgeom::static_item::build::load_dyna_source(&mut store, &p, &mut mg, true).unwrap_or_else(die);
            let pos: [f32; 3] = flag(&a.rest, "--prefab-pos").map(|v| { let f: Vec<f32> = v.split(',').map(|x| x.parse().unwrap()).collect(); [f[0], f[1], f[2]] }).unwrap_or([0.0; 3]);
            let q: [f32; 4] = flag(&a.rest, "--prefab-rot").map(|v| { let f: Vec<f32> = v.split(',').map(|x| x.parse().unwrap()).collect(); [f[0], f[1], f[2], f[3]] }).unwrap_or([0.0, 0.0, 0.0, 1.0]);
            // quaternion → rotation (x,y,z,w)
            let (x, y, z, w) = (q[0], q[1], q[2], q[3]);
            let rot = |v: [f32; 3]| -> [f32; 3] {
                let (vx, vy, vz) = (v[0], v[1], v[2]);
                // v' = v + 2w(q×v) + 2 q×(q×v)
                let (cx, cy, cz) = (y * vz - z * vy, z * vx - x * vz, x * vy - y * vx);
                let (dx, dy, dz) = (y * cz - z * cy, z * cx - x * cz, x * cy - y * cx);
                [vx + 2.0 * (w * cx + dx) + pos[0], vy + 2.0 * (w * cy + dy) + pos[1], vz + 2.0 * (w * cz + dz) + pos[2]]
            };
            for vr in &src.s2.visuals {
                let Some(mapgeom::static_item::Node::Visual(v)) = vr.inline.as_deref() else { continue };
                let Some(main) = v.main.as_ref() else { continue };
                let Some(mapgeom::static_item::Node::VertexStream(st)) = main.vertex_streams.first().and_then(|r| r.inline.as_deref()) else { continue };
                let Some(mapgeom::static_item::vstream::Elem::Float3(pts)) = st.elems.first() else { continue };
                let nf = v.sub_visuals.len().max(1);
                let per = pts.len() / nf;
                println!("visual: {} vertices, {} frames x {} ({} tris)", pts.len(), nf, per, v.index_buffer.as_ref().map(|b| b.indices.len() / 3).unwrap_or(0));
                let (mut ulo, mut uhi) = ([f32::INFINITY; 3], [f32::NEG_INFINITY; 3]);
                for f in 0..nf {
                    let (mut lo, mut hi) = ([f32::INFINITY; 3], [f32::NEG_INFINITY; 3]);
                    for pnt in &pts[f * per..(f + 1) * per] {
                        let w = rot(*pnt);
                        for k in 0..3 {
                            lo[k] = lo[k].min(w[k]);
                            hi[k] = hi[k].max(w[k]);
                            ulo[k] = ulo[k].min(w[k]);
                            uhi[k] = uhi[k].max(w[k]);
                        }
                    }
                    if f % 10 == 0 || f == nf - 1 {
                        println!("  frame {f:3}: x {:.2}..{:.2}  y {:.2}..{:.2}  z {:.2}..{:.2}", lo[0], hi[0], lo[1], hi[1], lo[2], hi[2]);
                    }
                }
                println!("  UNION: x {:.3}..{:.3}  y {:.3}..{:.3}  z {:.3}..{:.3}", ulo[0], uhi[0], ulo[1], uhi[1], ulo[2], uhi[2]);
                // the TIME-AVERAGED cloth (per-vertex mean over the frames): its bbox, and its top/bottom edge
                // tilt (max/min y per z bin) — the "stationary flag" rectangle Hugo describes
                {
                    let mut mean: Vec<[f32; 3]> = vec![[0.0; 3]; per];
                    for f in 0..nf {
                        for (k, pnt) in pts[f * per..(f + 1) * per].iter().enumerate() {
                            let w = rot(*pnt);
                            for c in 0..3 {
                                mean[k][c] += w[c] / nf as f32;
                            }
                        }
                    }
                    let (mut lo, mut hi) = ([f32::INFINITY; 3], [f32::NEG_INFINITY; 3]);
                    for w in &mean {
                        for c in 0..3 {
                            lo[c] = lo[c].min(w[c]);
                            hi[c] = hi[c].max(w[c]);
                        }
                    }
                    println!("  TIME-AVERAGE cloth: x {:.3}..{:.3}  y {:.3}..{:.3}  z {:.3}..{:.3}", lo[0], hi[0], lo[1], hi[1], lo[2], hi[2]);
                    let mut bins: std::collections::BTreeMap<i32, (f32, f32)> = std::collections::BTreeMap::new();
                    for w in &mean {
                        let e = bins.entry((w[2] * 2.0).round() as i32).or_insert((f32::INFINITY, f32::NEG_INFINITY));
                        e.0 = e.0.min(w[1]);
                        e.1 = e.1.max(w[1]);
                    }
                    for (b, (ylo, yhi)) in &bins {
                        println!("    z {:.1}: y {:.2}..{:.2}  (height {:.2})", *b as f32 / 2.0, ylo, yhi, yhi - ylo);
                    }
                    // least-squares tilt of the top and bottom edges over z
                    let fit = |sel: &dyn Fn(&(f32, f32)) -> f32| -> (f32, f32) {
                        let n = bins.len() as f32;
                        let (mut sx, mut sy, mut sxx, mut sxy) = (0.0f32, 0.0f32, 0.0f32, 0.0f32);
                        for (b, e) in &bins {
                            let x = *b as f32 / 2.0;
                            let y = sel(e);
                            sx += x; sy += y; sxx += x * x; sxy += x * y;
                        }
                        let slope = (n * sxy - sx * sy) / (n * sxx - sx * sx);
                        (slope, (sy - slope * sx) / n)
                    };
                    let (st, bt) = fit(&|e| e.1);
                    let (sb, bb) = fit(&|e| e.0);
                    println!("    top edge: y = {bt:.2} + {st:.3}·z (tilt {:.1}° down); bottom edge: y = {bb:.2} + {sb:.3}·z (tilt {:.1}° down)", (-st).atan().to_degrees(), (-sb).atan().to_degrees());
                }
                if flag(&a.rest, "--profile").is_some() {
                    for f in [0usize, nf / 4, nf / 2, 3 * nf / 4] {
                        let mut ws: Vec<[f32; 3]> = pts[f * per..(f + 1) * per].iter().map(|p| rot(*p)).collect();
                        ws.sort_by(|a, b| a[2].partial_cmp(&b[2]).unwrap());
                        let n10 = (ws.len() / 10).max(1);
                        let near: f32 = ws[..n10].iter().map(|p| p[1]).sum::<f32>() / n10 as f32;
                        let far: f32 = ws[ws.len() - n10..].iter().map(|p| p[1]).sum::<f32>() / n10 as f32;
                        let znear: f32 = ws[..n10].iter().map(|p| p[2]).sum::<f32>() / n10 as f32;
                        let zfar: f32 = ws[ws.len() - n10..].iter().map(|p| p[2]).sum::<f32>() / n10 as f32;
                        println!("  frame {f:3}: mean y near z {znear:.2} = {near:.2}, far z {zfar:.2} = {far:.2} → tilt {:.1}°", ((near - far) / (zfar - znear)).atan().to_degrees());
                    }
                }
            }
        }
        // item-height MAP MODEL [MODEL..]: the item models' heights as `sandbox` reads them (the pack prefab's geometry)
        "item-height" => {
            let mut store = open(&a);
            let mp = a.rest.get(1).cloned().unwrap_or_else(|| die("item-height MAP MODEL..".into()));
            let m = tmmaps::map::MapFile::try_load(std::path::Path::new(&mp)).unwrap_or_else(die);
            let mut ctx = mapgeom::sttc::Ctx::new(&mut store, &m);
            for model in a.rest.iter().skip(2).filter(|x| !x.starts_with("--")) {
                let refs = ctx.item_refs(model);
                println!("{model}: {} refs{}", refs.len(), refs.iter().take(4).map(|r| format!("\n    {r}")).collect::<String>());
                println!("  height {:?}", mapgeom::sandbox::item_height(&mut ctx, model));
            }
        }
        // moth-carriers --lights LIGHTS.tsv --out DIR [--collection 26] [--light-groups ..] [--light-exclude A,B] [--only K1,K2]
        //   [--source-pad 0.3] [--ident-prefix MothFinish] [--mapping F.tsv]: every light-source kind's carrier item written
        //   to DIR (Hugo's own tool places them), plus the mapping TSV (source model, variant, carrier file, trigger bbox)
        "moth-carriers" => {
            let mut store = open(&a);
            let tsv = flag(&a.rest, "--lights").or_else(|| flag(&a.rest, "--lantern-parts")).or_else(|| flag(&a.rest, "--floor")).unwrap_or_else(|| die("moth-carriers needs --lights LIGHTS.tsv, --lantern-parts PARTS.tsv or --floor COLLECTION".into()));
            let out = flag(&a.rest, "--out").unwrap_or_else(|| die("moth-carriers needs --out DIR".into()));
            let collection: u32 = flag(&a.rest, "--collection").map(|v| v.parse().unwrap_or_else(|_| die("--collection N".into()))).unwrap_or(26);
            let pad: f32 = flag(&a.rest, "--source-pad").map(|v| v.parse().unwrap_or_else(|_| die("--source-pad M".into()))).unwrap_or(if flag(&a.rest, "--floor").is_some() { 0.0 } else { 0.3 });
            let prefix = flag(&a.rest, "--ident-prefix").unwrap_or_default();
            let only: Vec<String> = flag(&a.rest, "--only").map(|v| v.split(',').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect()).unwrap_or_default();
            let mut sources = if let Some(fc) = flag(&a.rest, "--floor") {
                let coll = match fc.as_str() { "RedIsland" => 0x10, "WhiteShore" => 0x1d, "BlueBay" => 0x1c, "GreenCoast" => 0xf, other => die(format!("--floor {other}: RedIsland|WhiteShore|BlueBay|GreenCoast")) };
                let below: f32 = flag(&a.rest, "--floor-below").map(|v| v.parse().unwrap_or_else(|_| die("--floor-below M".into()))).unwrap_or(3.0);
                let above: f32 = flag(&a.rest, "--floor-above").map(|v| v.parse().unwrap_or_else(|_| die("--floor-above M".into()))).unwrap_or(0.5);
                let label = flag(&a.rest, "--floor-label").unwrap_or_else(|| "Floor32m".into());
                let mut v = floor_sources(coll, below, above, &label);
                v.truncate(1); // one carrier per collection (every open-water tile of it shares the slab)
                v
            } else if let Some(parts) = flag(&a.rest, "--lantern-parts") {
                let mats: Vec<String> = flag(&a.rest, "--lantern-materials").map(|v| v.split(',').map(|s| s.trim().to_string()).collect()).unwrap_or_else(|| vec!["ThemeSnowTempleLamp".into(), "LightShape".into(), "ThemeSnowTempleTransparent".into()]);
                let lonly: Vec<String> = flag(&a.rest, "--lantern-only").map(|v| v.split(',').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect()).unwrap_or_default();
                lantern_sources_from_parts(&parts, &mats, &lonly)
            } else {
                light_sources_from_tsv(&tsv, flag(&a.rest, "--light-groups"), flag(&a.rest, "--light-exclude"))
            };
            if !only.is_empty() { sources.retain(|s| only.iter().any(|k| *k == s.key)); }
            std::fs::create_dir_all(&out).unwrap_or_else(|e| die(format!("{out}: {e}")));
            let mut map = String::from("source_model\tvariant\tsource_kind\tcarrier_file\tcarrier_ident\ttrigger_bbox_lo_x\tlo_y\tlo_z\thi_x\thi_y\thi_z\tpose_rule\n");
            for src in &sources {
                let (ident, bb, bytes) = mapgeom::flags::build_light_carrier(&mut store, src, pad, collection, &prefix, None).unwrap_or_else(|e| die(format!("{}: {e}", src.key)));
                let path = std::path::Path::new(&out).join(&ident);
                std::fs::write(&path, &bytes).unwrap_or_else(|e| die(format!("{}: {e}", path.display())));
                let (model, variant) = match src.key.split_once("@v") { Some((m, v)) => (m.to_string(), v.to_string()), None => (src.key.clone(), if src.is_block { "block".into() } else { "any".into() }) };
                let rule = if src.is_block { "block: carrier at the block's cell corner turned by dir ((0,0) (32,0) (32,32) (0,32) in x,z for dir 0..3), yaw = -dir*90deg, y = 8*cy + ground, pivot 0, scale 1" } else { "item: carrier at the source item's position with its yaw/pitch/roll, pivot 0, scale 1" };
                let boxes = if src.boxes.len() > 1 { format!(" | {} boxes (+pad): {}", src.boxes.len(), src.boxes.iter().map(|(l, h)| format!("[{:.2} {:.2} {:.2}]..[{:.2} {:.2} {:.2}]", l[0] - pad, l[1] - pad, l[2] - pad, h[0] + pad, h[1] + pad, h[2] + pad)).collect::<Vec<_>>().join(" ")) } else { String::new() };
                map.push_str(&format!("{model}\t{variant}\t{}\t{ident}\t{ident}\t{:.3}\t{:.3}\t{:.3}\t{:.3}\t{:.3}\t{:.3}\t{rule}{boxes}\n", if src.is_block { "block" } else { "item" }, bb[0], bb[1], bb[2], bb[3], bb[4], bb[5]));
                println!("{ident}\t{} bytes\t[{:.2} {:.2} {:.2}]..[{:.2} {:.2} {:.2}]", bytes.len(), bb[0], bb[1], bb[2], bb[3], bb[4], bb[5]);
            }
            if let Some(m) = flag(&a.rest, "--mapping") { std::fs::write(&m, map).unwrap_or_else(|e| die(format!("{m}: {e}"))); eprintln!("wrote {m}"); }
            eprintln!("{} carriers in {out}", sources.len());
        }
        // water-rects MAP: every terrain tile placement KIND (name, flags) of the map that carries a water quad — the
        //   prefab the flags pick, the quad's rect in the block frame, the surface and the bed under it (Hugo's Floor
        //   Finish v2 on shore tiles, 2026-10-23); --tiles A,B restricts the names
        "water-rects" => {
            let mut store = open(&a);
            let p = a.rest.get(1).cloned().unwrap_or_else(|| die("water-rects needs a MAP".into()));
            let m = tmmaps::map::MapFile::try_load(std::path::Path::new(&p)).unwrap_or_else(die);
            let collection = m.body_collections().map(|c| c[0].1).unwrap_or(26);
            let only: Vec<String> = flag(&a.rest, "--tiles").map(|v| v.split(',').map(|s| s.trim().to_string()).collect()).unwrap_or_default();
            let mut kinds: std::collections::BTreeMap<(String, u32), usize> = Default::default();
            // authored + BAKED (the Deco bays and the zone tiles may be baked)
            for b in m.blocks.iter().chain(m.baked.iter()).filter(|b| b.flags != 0xFFFF_FFFF && b.free_pos.is_none()) {
                if !only.is_empty() && !only.iter().any(|t| t.eq_ignore_ascii_case(&b.name)) { continue; }
                *kinds.entry((b.name.clone(), b.flags)).or_default() += 1;
            }
            // --footprints F.tsv: the per-kind 4 m water mask for Hugo's C# (collection, tile, flags, prefab, bed, mask64:
            // bit (zi*8 + xi) set = the 4 m square at block-frame x 4·xi..4·xi+4, z 4·zi..4·zi+4 is water)
            // deep_beds: per deep square (bit order zi*8+xi, deep squares only, in order) the LOCAL bed under its centre
            // rounded DOWN to 0.25 m, as "y,y,y" — the C# slab bottom follows the bank's ramp like ours
            // the table format Hugo's C# reads (v4.1, one row per kind, 1×1 tiles). `--per-cell` writes one row per UNIT
            // CELL instead (size_x, size_z = the footprint in cells at dir 0; cell = "ux,uz" like the Stadium masks) for
            // multi-cell kinds; without it a multi-cell kind is an ERROR, never a silently truncated row
            let per_cell = a.rest.iter().any(|x| x == "--per-cell");
            let mut fp = String::from(if per_cell {
                "collection\ttile\tflags_hex\tprefab\tsize_x\tsize_z\tcell\tbed_local\tsurface_local\tmask64_hex\twater_squares\tdeep_mask64_hex\tdeep_squares\tdeep_beds\n"
            } else {
                "collection\ttile\tflags_hex\tprefab\tbed_local\tsurface_local\tmask64_hex\twater_squares\tdeep_mask64_hex\tdeep_squares\tdeep_beds\n"
            });
            let (_, open_bed) = mapgeom::floor_finish::water_tiles(collection);
            let coll_name = match collection { 0x10 => "RedIsland", 0x1d => "WhiteShore", 0x1c => "BlueBay", 0xf => "GreenCoast", _ => "Stadium" };
            println!("tile\tflags\tplacements\tprefab\tx0\tx1\tz0\tz1\tsurface\tbed_lo\tbed_hi\tbed_tris\twater_4m_squares");
            for ((name, flags), n) in kinds {
                match mapgeom::floor_finish::water_rect(&mut store, collection, &name, flags) {
                    Ok(Some(w)) => {
                        let cells = mapgeom::floor_finish::cell_footprints(&w, open_bed);
                        let (wsq, dsq) = cells.iter().fold((0u32, 0usize), |(a, b), (_, m, d)| (a + m.count_ones(), b + d.iter().filter(|s| s.is_some()).count()));
                        println!("{name}\t{flags:08x}\t{n}\t{}\t{:.2}\t{:.2}\t{:.2}\t{:.2}\t{:.2}\t{:.2}\t{:.2}\t{}\t{wsq}\tdeep {dsq}\tcells {}x{}", w.prefab, w.x0, w.x1, w.z0, w.z1, w.surface, w.bed_lo, w.bed_hi, w.bed_tris, w.size.0, w.size.1);
                        if !per_cell && w.size != (1, 1) {
                            die::<()>(format!("{name} flags {flags:08x} spans {}×{} cells: the one-row-per-kind table cannot hold it — pass --per-cell (and a C# that reads the per-cell format)", w.size.0, w.size.1));
                        }
                        for ((ux, uz), mask, deep) in &cells {
                            let dmask = deep.iter().enumerate().fold(0u64, |m, (i, d)| if d.is_some() { m | (1u64 << i) } else { m });
                            let beds: Vec<String> = deep.iter().filter_map(|d| d.map(|y| format!("{:.2}", (y * 4.0).floor() / 4.0))).collect();
                            if per_cell {
                                fp.push_str(&format!("{coll_name}\t{name}\t{flags:08x}\t{}\t{}\t{}\t{ux},{uz}\t{:.2}\t{:.2}\t{mask:016x}\t{}\t{dmask:016x}\t{}\t{}\n", w.prefab, w.size.0, w.size.1, w.bed_lo, w.surface, mask.count_ones(), dmask.count_ones(), beds.join(",")));
                            } else {
                                fp.push_str(&format!("{coll_name}\t{name}\t{flags:08x}\t{}\t{:.2}\t{:.2}\t{mask:016x}\t{}\t{dmask:016x}\t{}\t{}\n", w.prefab, w.bed_lo, w.surface, mask.count_ones(), dmask.count_ones(), beds.join(",")));
                            }
                        }
                    }
                    Ok(None) => if !only.is_empty() { println!("{name}\t{flags:08x}\t{n}\t-\tno water quad") },
                    Err(e) => if !only.is_empty() { println!("{name}\t{flags:08x}\t{n}\tERROR {e}") },
                }
            }
            if let Some(f) = flag(&a.rest, "--footprints") {
                // append (several maps feed one table); the header once
                let existing = std::fs::read_to_string(&f).unwrap_or_default();
                let body: String = fp.lines().skip(1).filter(|l| !existing.contains(l)).map(|l| format!("{l}\n")).collect();
                let out = if existing.is_empty() { fp.lines().next().unwrap().to_string() + "\n" + &body } else { existing + &body };
                std::fs::write(&f, out).unwrap_or_else(|e| die(format!("{f}: {e}")));
            }
        }
        // stadium-footprints --out F.tsv [--collection Stadium]: every block info of the collection → its ground-variant
        //   footprint (size x/z in cells + the unit cells at the base row, dir 0) — the static table Hugo's C# reads
        "stadium-footprints" => {
            let mut store = open(&a);
            let out = flag(&a.rest, "--out").unwrap_or_else(|| die("stadium-footprints needs --out F.tsv".into()));
            let coll = flag(&a.rest, "--collection").unwrap_or_else(|| "Stadium".into());
            let prefix = format!("{}\\GAMECTNBLOCKINFO\\", coll.to_uppercase());
            let paths: Vec<String> = store.entries().map(|e| e.path()).filter(|p| { let u = p.to_uppercase(); u.starts_with(&prefix) && u.ends_with(".GBX") }).collect();
            let mut rows: std::collections::BTreeMap<String, (i32, i32, Vec<(i32, i32)>, &'static str, Vec<(i32, i32)>)> = Default::default();
            let mut failed = 0usize;
            for p in &paths {
                let name = p.rsplit('\\').next().unwrap_or(p).split('.').next().unwrap_or("").to_string();
                if name.is_empty() { continue; }
                let Ok(bi) = mapgeom::blockinfo::load(&mut store, p) else { failed += 1; continue };
                // the info's own name (the pak file name differs in CASE for a few: PlatFormTechBase.EDClassic.Gbx)
                let name = if bi.name.is_empty() { name } else { bi.name.clone() };
                let (v, which) = match (&bi.variant_base_ground, &bi.variant_base_air) {
                    (Some(g), _) if !g.block_units.is_empty() => (g, "ground"),
                    (_, Some(air)) if !air.block_units.is_empty() => (air, "air"),
                    _ => { rows.insert(name, (1, 1, vec![(0, 0)], "none", vec![(0, 0)])); continue }
                };
                let (mut sx, mut sz) = (0, 0);
                let mut cells: Vec<(i32, i32)> = Vec::new();
                // every unit's column (any row): the tile-hiding footprint (a slope's top unit hides its column too)
                let mut cols: Vec<(i32, i32)> = Vec::new();
                for u in &v.block_units {
                    sx = sx.max(u.offset[0] + 1);
                    sz = sz.max(u.offset[2] + 1);
                    if u.offset[1] == 0 && !cells.contains(&(u.offset[0], u.offset[2])) { cells.push((u.offset[0], u.offset[2])); }
                    if !cols.contains(&(u.offset[0], u.offset[2])) { cols.push((u.offset[0], u.offset[2])); }
                }
                cells.sort_unstable();
                cols.sort_unstable();
                rows.insert(name, (sx, sz, cells, which, cols));
            }
            // v2 (2026-10-23 12:20): per base cell a 64-bit 4 m mask of the ground-touching geometry ("ux,uz:mask16hex")
            let with_masks = a.rest.iter().any(|x| x == "--masks");
            let mut s = String::from(if with_masks { "block\tsize_x\tsize_z\tbase_cells\tvariant\tmasks\tcolumns\n" } else { "block\tsize_x\tsize_z\tbase_cells\tvariant\tcolumns\n" });
            let mut n_mask = 0usize;
            let mut n_partial = 0usize;
            let mut n_green = 0usize;
            for (name, (sx, sz, cells, which, cols)) in &rows {
                let cells_s = cells.iter().map(|(x, z)| format!("{x},{z}")).collect::<Vec<_>>().join(" ");
                let cols_s = cols.iter().map(|(x, z)| format!("{x},{z}")).collect::<Vec<_>>().join(" ");
                if with_masks {
                    let bp = mapgeom::vista::find_block_info(&store, name);
                    let masks: BTreeMap<(i32, i32), (u64, u64)> = match bp.and_then(|bp| mapgeom::blockinfo::load(&mut store, &bp).ok()) {
                        Some(bi) => {
                            let v = if *which == "air" { bi.variant_base_air.clone() } else { bi.variant_base_ground.clone() };
                            match v { Some(v) => mapgeom::floor_finish::variant_masks(&mut store, &v, 8.0).unwrap_or_default(), None => Default::default() }
                        }
                        None => Default::default(),
                    };
                    let masks: BTreeMap<(i32, i32), (u64, u64)> = if masks.is_empty() { cells.iter().map(|&(x, z)| ((x, z), (u64::MAX, 0))).collect() } else { masks };
                    n_mask += 1;
                    if masks.values().any(|&(o, _)| o != u64::MAX) { n_partial += 1; }
                    if masks.values().any(|&(_, g)| g != 0) { n_green += 1; }
                    // "ux,uz:occupied:green" — two 64-bit masks per base cell
                    s.push_str(&format!("{name}\t{sx}\t{sz}\t{cells_s}\t{which}\t{}\t{cols_s}\n", masks.iter().map(|((x, z), (o, g))| format!("{x},{z}:{o:016x}:{g:016x}")).collect::<Vec<_>>().join(" ")));
                } else {
                    s.push_str(&format!("{name}\t{sx}\t{sz}\t{cells_s}\t{which}\t{cols_s}\n"));
                }
            }
            if with_masks { println!("masks: {n_mask} variants baked, {n_partial} with structure-free 4 m squares inside their footprint, {n_green} carrying a collidable grass fill"); }
            std::fs::write(&out, s).unwrap_or_else(|e| die(format!("{out}: {e}")));
            let multi = rows.values().filter(|r| r.0 > 1 || r.1 > 1).count();
            let _ = &multi;
            let holes = rows.values().filter(|r| (r.2.len() as i32) < r.0 * r.1).count();
            println!("{}: {} block infos ({} failed to load), {} multi-cell, {} with base-row holes (footprint smaller than its size box)", out, rows.len(), failed, multi, holes);
        }
        // block-sizes --out F.tsv [--conflicts F.tsv]: every block info of EVERY loaded pak → size_x, size_z (cells at
        //   dir 0: the footprint box of the ground base variant, air when no ground one) for Hugo's MapUtility
        //   (2026-10-23 22:45); names present in several collections / variants with different sizes are reported
        "block-sizes" => {
            let store = open(&a);
            let out = flag(&a.rest, "--out").unwrap_or_else(|| die("block-sizes needs --out F.tsv".into()));
            let mut store = store;
            let paths: Vec<String> = store.entries().map(|e| e.path()).filter(|p| { let u = p.to_uppercase(); u.contains("\\GAMECTNBLOCKINFO\\") && u.ends_with(".GBX") }).collect();
            // name -> set of (collection, variant label, sx, sz)
            let mut seen: BTreeMap<String, Vec<(String, &'static str, i32, i32)>> = BTreeMap::new();
            let mut failed = 0usize;
            for p in &paths {
                let coll = p.split('\\').next().unwrap_or("").to_string();
                let Ok(bi) = mapgeom::blockinfo::load(&mut store, p) else { failed += 1; continue };
                let name = if bi.name.is_empty() { p.rsplit('\\').next().unwrap_or(p).split('.').next().unwrap_or("").to_string() } else { bi.name.clone() };
                let size_of = |v: &mapgeom::blockinfo::Variant| -> Option<(i32, i32)> {
                    if v.block_units.is_empty() { return None; }
                    Some((v.block_units.iter().map(|u| u.offset[0] + 1).max().unwrap_or(1), v.block_units.iter().map(|u| u.offset[2] + 1).max().unwrap_or(1)))
                };
                let e = seen.entry(name).or_default();
                if let Some(sz) = bi.variant_base_ground.as_ref().and_then(|v| size_of(v)) { e.push((coll.clone(), "ground", sz.0, sz.1)); }
                if let Some(sz) = bi.variant_base_air.as_ref().and_then(|v| size_of(v)) { e.push((coll.clone(), "air", sz.0, sz.1)); }
                for (k, v) in bi.additional_ground.iter().enumerate() { if let Some(sz) = size_of(v) { let _ = k; e.push((coll.clone(), "ground/add", sz.0, sz.1)); } }
                for v in &bi.additional_air { if let Some(sz) = size_of(v) { e.push((coll.clone(), "air/add", sz.0, sz.1)); } }
                if e.is_empty() { e.push((coll.clone(), "none", 1, 1)); }
            }
            let mut s = String::from("block\tsize_x\tsize_z\n");
            let mut conflicts = String::from("block\tcollection\tvariant\tsize_x\tsize_z\n");
            let (mut n, mut nc) = (0usize, 0usize);
            for (name, list) in &seen {
                // the size the placement uses: the ground base variant of the first collection, else air
                let pick = list.iter().find(|(_, v, _, _)| *v == "ground").or_else(|| list.iter().find(|(_, v, _, _)| *v == "air")).or(list.first()).unwrap();
                s.push_str(&format!("{name}\t{}\t{}\n", pick.2, pick.3));
                n += 1;
                let distinct: std::collections::BTreeSet<(i32, i32)> = list.iter().map(|(_, _, x, z)| (*x, *z)).collect();
                if distinct.len() > 1 {
                    nc += 1;
                    for (c, v, x, z) in list { conflicts.push_str(&format!("{name}\t{c}\t{v}\t{x}\t{z}\n")); }
                }
            }
            std::fs::write(&out, &s).unwrap_or_else(|e| die(format!("{out}: {e}")));
            if let Some(cf) = flag(&a.rest, "--conflicts") { std::fs::write(&cf, &conflicts).unwrap_or_else(|e| die(format!("{cf}: {e}"))); }
            println!("{out}: {n} block names from {} infos ({failed} failed to load); {nc} names whose variants / collections disagree on the size", paths.len());
        }
        // stadium-kinds MAP... [--out F.tsv]: Hugo's cutout feasibility (2026-10-23 13:51) — per ground-block KIND placed in
        //   the maps: placements, base cells, and its hidden cells' 4 m squares as (a) structure / (b) standable
        //   grass-level surface / (c) void; the kinds with (b) > 0 are the cutout shapes
        "stadium-kinds" => {
            let mut store = open(&a);
            let out = flag(&a.rest, "--out");
            let maps: Vec<String> = a.rest[1..].iter().filter(|x| !x.starts_with("--") && Some(x.as_str()) != out.as_deref()).cloned().collect();
            // kind (name, flags) -> (placements, maps)
            let mut kinds: BTreeMap<(String, u32), (usize, std::collections::BTreeSet<String>)> = BTreeMap::new();
            for p in &maps {
                let m = tmmaps::map::MapFile::try_load(std::path::Path::new(p)).unwrap_or_else(die);
                let label = std::path::Path::new(p).file_stem().map(|s| s.to_string_lossy().chars().take(2).collect::<String>()).unwrap_or_default();
                for b in m.blocks.iter().filter(|b| b.flags != 0xFFFF_FFFF && b.free_pos.is_none() && b.flags & mapgeom::blockmap::FLAG_GROUND != 0 && !b.name.eq_ignore_ascii_case("Grass")) {
                    let e = kinds.entry((b.name.clone(), b.flags)).or_default();
                    e.0 += 1;
                    e.1.insert(label.clone());
                }
            }
            let mut s = String::from("block\tflags\tplacements\tmaps\tbase_cells\tsq_structure\tsq_standable\tsq_void\tstandable_cells\n");
            let (mut n_kinds, mut n_b, mut n_names_b) = (0usize, 0usize, std::collections::BTreeSet::new());
            for ((name, flags), (n, labels)) in &kinds {
                let masks = mapgeom::floor_finish::placement_masks(&mut store, name, *flags).unwrap_or_default();
                let cells = masks.len();
                let (mut sa, mut sb, mut sc, mut cb) = (0u32, 0u32, 0u32, 0usize);
                for (_, (occ, grn)) in &masks {
                    let b_only = grn & !occ;
                    sa += occ.count_ones();
                    sb += b_only.count_ones();
                    sc += (!(occ | grn)).count_ones();
                    if b_only != 0 { cb += 1; }
                }
                n_kinds += 1;
                if sb > 0 { n_b += 1; n_names_b.insert(name.clone()); }
                s.push_str(&format!("{name}\t{flags:08x}\t{n}\t{}\t{cells}\t{sa}\t{sb}\t{sc}\t{cb}\n", labels.iter().cloned().collect::<Vec<_>>().join(" ")));
            }
            print!("{s}");
            println!("# {n_kinds} ground-block kinds (name, flags) over {} maps; {n_b} kinds / {} distinct block names with standable grass-level squares (the cutout shapes)", maps.len(), n_names_b.len());
            if let Some(o) = out { std::fs::write(&o, s).unwrap_or_else(|e| die(format!("{o}: {e}"))); }
        }
        // stadium-cutouts --out DIR [--blocks A,B,..] [--maps M1 M2 ..] [--inset 0.5] [--collection 26] [--report F.tsv]:
        //   Hugo's per-block-kind cutout items FloorFinish<Block>.Item.Gbx (the ground fill's standable outline, block
        //   frame, 8.0..9.0) for the blocks named, or for every IsGround kind placed in the maps given
        "stadium-cutouts" => {
            let mut store = open(&a);
            let out = flag(&a.rest, "--out").unwrap_or_else(|| die("stadium-cutouts needs --out DIR".into()));
            std::fs::create_dir_all(&out).unwrap_or_else(|e| die(format!("{out}: {e}")));
            let inset: f32 = flag(&a.rest, "--inset").map(|v| v.parse().unwrap_or_else(|_| die("--inset M".into()))).unwrap_or(0.5);
            let coll: u32 = flag(&a.rest, "--collection").map(|v| v.parse().unwrap_or_else(|_| die("--collection N".into()))).unwrap_or(26);
            // kinds = (name, VARIANT byte) — the variant the game draws (flags bits 0..5 | additional 21..27 → one byte
            // as GBX.NET's CGameCtnBlock.Variant exposes it: low 6 bits the variant, high bits the additional index);
            // `--blocks A:V,B` names them (V = the variant byte, 0 when omitted)
            let mut kinds: std::collections::BTreeMap<(String, u32), usize> = Default::default();
            if let Some(v) = flag(&a.rest, "--blocks") {
                for s in v.split(',').map(|s| s.trim()).filter(|s| !s.is_empty()) {
                    let (name, var) = s.split_once(':').map(|(n, v)| (n.to_string(), v.parse::<u32>().unwrap_or(0))).unwrap_or((s.to_string(), 0));
                    kinds.insert((name, mapgeom::floor_finish::flags_of_variant(var)), 0);
                }
            }
            if let Some(i) = a.rest.iter().position(|x| x == "--maps") {
                for p in a.rest[i + 1..].iter().take_while(|x| !x.starts_with("--")) {
                    let m = tmmaps::map::MapFile::try_load(std::path::Path::new(p)).unwrap_or_else(die);
                    for b in m.blocks.iter().filter(|b| b.flags != 0xFFFF_FFFF && b.free_pos.is_none() && b.flags & mapgeom::blockmap::FLAG_GROUND != 0 && !b.name.eq_ignore_ascii_case("Grass")) {
                        *kinds.entry((b.name.clone(), b.flags & mapgeom::floor_finish::VARIANT_FLAG_MASK)).or_default() += 1;
                    }
                }
            }
            // v4.4: EXACT outlines by default (polygons traced from the pak fill mesh, cutout_outline.rs); --squares = the
            // 1 m-raster boxes of v3/v4
            let squares = a.rest.iter().any(|x| x == "--squares");
            let mut rep = String::from(if squares { "block\tvariant\tflags\tplacements\titem\tbytes\tkept_tris\tdropped_tris\tarea_m2\tbbox\n" } else { "block\tvariant\tflags\tplacements\titem\tbytes\tvertices\tpolygons\tholes\ton_mesh\tslivers_dropped\tarea_m2\traster_area_m2\tbbox\tnotes\n" });
            let (mut built, mut none) = (0usize, 0usize);
            for ((name, kflags), n) in &kinds {
                let var = mapgeom::floor_finish::variant_byte(*kflags);
                let ident = if var == 0 { format!("FloorFinish{name}.Item.Gbx") } else { format!("FloorFinish{name}V{var}.Item.Gbx") };
                if !squares {
                    match mapgeom::floor_finish::build_cutout_item_exact(&mut store, name, *kflags, &ident, coll, inset, 0.0, 1.0) {
                        Ok(Some((bytes, c, o))) => {
                            std::fs::write(format!("{out}/{ident}"), &bytes).unwrap_or_else(|e| die(format!("{ident}: {e}")));
                            let raster_area = mapgeom::floor_finish::cutout_for_block(&mut store, name, *kflags, inset, 0.0, 1.0).ok().flatten().map(|r| r.area).unwrap_or(0.0);
                            rep.push_str(&format!("{name}\t{var}\t{kflags:08x}\t{n}\t{ident}\t{}\t{}\t{}\t{}\t{}\t{}\t{:.1}\t{:.1}\t[{:.2} {:.2} {:.2}]..[{:.2} {:.2} {:.2}]\t{}\n", bytes.len(), o.vertices, o.polygons.len(), o.polygons.iter().map(|(_, h)| h.len()).sum::<usize>(), o.on_mesh, c.dropped, o.area, raster_area, c.bbox.0[0], c.bbox.0[1], c.bbox.0[2], c.bbox.1[0], c.bbox.1[1], c.bbox.1[2], o.notes.join(" | ")));
                            built += 1;
                        }
                        Ok(None) => { rep.push_str(&format!("{name}\t{var}\t{kflags:08x}\t{n}\t-\t0\t0\t0\t0\t0\t0\t0.0\t0.0\t-\tno standable fill\n")); none += 1; }
                        Err(e) => { rep.push_str(&format!("{name}\t{var}\t{kflags:08x}\t{n}\t-\t0\t0\t0\t0\t0\t0\t0.0\t0.0\t-\tERROR {e}\n")); none += 1; }
                    }
                    continue;
                }
                match mapgeom::floor_finish::build_cutout_item(&mut store, name, *kflags, &ident, coll, inset, 0.0, 1.0) {
                    Ok(Some((bytes, c))) => {
                        std::fs::write(format!("{out}/{ident}"), &bytes).unwrap_or_else(|e| die(format!("{ident}: {e}")));
                        rep.push_str(&format!("{name}\t{var}\t{kflags:08x}\t{n}\t{ident}\t{}\t{}\t{}\t{:.1}\t[{:.2} {:.2} {:.2}]..[{:.2} {:.2} {:.2}]\n", bytes.len(), c.kept, c.dropped, c.area, c.bbox.0[0], c.bbox.0[1], c.bbox.0[2], c.bbox.1[0], c.bbox.1[1], c.bbox.1[2]));
                        built += 1;
                    }
                    Ok(None) => { rep.push_str(&format!("{name}\t{var}\t{kflags:08x}\t{n}\t-\t0\t0\t0\t0.0\tno standable fill\n")); none += 1; }
                    Err(e) => { rep.push_str(&format!("{name}\t{var}\t{kflags:08x}\t{n}\t-\t0\t0\t0\t0.0\tERROR {e}\n")); none += 1; }
                }
            }
            print!("{rep}");
            println!("# {built} cutout items written to {out}, {none} kinds without a standable fill");
            // --ascii: a 2 m-grid picture of each cutout (block frame, x right, z down) for the eye
            if a.rest.iter().any(|x| x == "--ascii") {
                for ((name, kflags), _) in &kinds {
                    if let Ok(Some(c)) = mapgeom::floor_finish::cutout_for_block(&mut store, name, *kflags, inset, 0.0, 1.0) {
                        let boxes: Vec<[f32; 4]> = c.verts.chunks(8).map(|b| [b[0][0], b[1][0], b[0][2], b[4][2]]).collect();
                        let (w, h) = ((c.bbox.1[0].ceil() / 2.0) as usize, (c.bbox.1[2].ceil() / 2.0) as usize);
                        println!("== {name} ({} boxes, {:.0} m²):", boxes.len(), c.area);
                        for zi in 0..h.max(16) {
                            let mut row = String::new();
                            for xi in 0..w.max(16) {
                                let (x, z) = (2.0 * xi as f32 + 1.0, 2.0 * zi as f32 + 1.0);
                                row.push(if boxes.iter().any(|b| x >= b[0] && x < b[1] && z >= b[2] && z < b[3]) { '#' } else { '.' });
                                if xi % 16 == 15 { row.push('|'); }
                            }
                            println!("  {row}");
                            if zi % 16 == 15 { println!("  {}", "-".repeat(row.len())); }
                        }
                    }
                }
            }
            if let Some(f) = flag(&a.rest, "--report") { std::fs::write(&f, rep).unwrap_or_else(|e| die(format!("{f}: {e}"))); }
        }
        // stadium-floor MAP [--sizes F.tsv] [--cells F.tsv]: Hugo's Stadium floor occupancy (authored IsGround blocks'
        //   footprints turned by dir; Grass excepted) — the counts, the name → (size x, size z) table for his C#,
        //   and the free/occupied cell grid
        "stadium-floor" => {
            let mut store = open(&a);
            let p = a.rest.get(1).cloned().unwrap_or_else(|| die("stadium-floor needs a MAP".into()));
            let m = tmmaps::map::MapFile::try_load(std::path::Path::new(&p)).unwrap_or_else(die);
            let f = mapgeom::floor_finish::stadium_floor(&mut store, &m).unwrap_or_else(die);
            for n in &f.notes { println!("{n}"); }
            if let Some(t) = flag(&a.rest, "--sizes") {
                let existing = std::fs::read_to_string(&t).unwrap_or_default();
                let mut out = if existing.is_empty() { String::from("block\tsize_x\tsize_z\n") } else { existing.clone() };
                for (name, (sx, sz)) in &f.sizes {
                    let line = format!("{name}\t{sx}\t{sz}\n");
                    if !existing.contains(&format!("{name}\t")) { out.push_str(&line); }
                }
                std::fs::write(&t, out).unwrap_or_else(|e| die(format!("{t}: {e}")));
            }
            if let Some(t) = flag(&a.rest, "--cells") {
                let (mut xmax, mut zmax) = (0, 0);
                for &(x, z) in &f.grass { xmax = xmax.max(x); zmax = zmax.max(z); }
                let mut out = String::new();
                for z in 0..=zmax {
                    for x in 0..=xmax {
                        // '.' real grass, '#' hidden & structure, 'o' hidden with a collidable grass fill, 'x' hidden void (no grass, no structure)
                        let occ = f.occupied.get(&(x, z)).copied().unwrap_or(0);
                        let grn = f.green.get(&(x, z)).copied().unwrap_or(0);
                        out.push(if !f.grass.contains(&(x, z)) { ' ' } else if !f.hidden.contains(&(x, z)) { '.' } else if grn & !occ != 0 { 'o' } else if occ == u64::MAX { '#' } else if occ != 0 { 'x' } else { 'x' });
                    }
                    out.push('\n');
                }
                std::fs::write(&t, out).unwrap_or_else(|e| die(format!("{t}: {e}")));
            }
        }
        // floor-template --out F.Item.Gbx [--collection 26]: Hugo's Floor Finish template — ONE unit box (0..32 × 0..1 × 0..32)
        //   trigger + ONE indicator quad (v4.10 flat decal tinted by Color0; --indicator gate|glass) at y 0.02 (the C# swaps both buffers), Finish waypoint, ident = file name
        "floor-template" => {
            let mut store = open(&a);
            let out = flag(&a.rest, "--out").unwrap_or_else(|| die("floor-template needs --out F.Item.Gbx".into()));
            let coll: u32 = flag(&a.rest, "--collection").map(|v| v.parse().unwrap_or_else(|_| die("--collection N".into()))).unwrap_or(26);
            let ident = std::path::Path::new(&out).file_name().map(|s| s.to_string_lossy().to_string()).unwrap_or_else(|| "FinishFloorTemplate.Item.Gbx".into());
            let donor = "Stadium\\Items\\LightCube2m.Item.Gbx";
            let mut mg = mapgeom::static_item::build::pack_item_merged(&mut store, donor, 1.0, coll, 0, None).unwrap_or_else(|e| die(format!("donor {donor}: {e}")));
            let (x0, x1, z0, z1, y0, y1) = (0.0f32, 32.0f32, 0.0f32, 32.0f32, 0.0f32, 1.0f32);
            let verts = vec![[x0, y0, z0], [x1, y0, z0], [x1, y1, z0], [x0, y1, z0], [x0, y0, z1], [x1, y0, z1], [x1, y1, z1], [x0, y1, z1]];
            let quads: [[u32; 4]; 6] = [[0, 3, 2, 1], [4, 5, 6, 7], [0, 1, 5, 4], [2, 3, 7, 6], [1, 2, 6, 5], [0, 4, 7, 3]];
            let mut tris = Vec::new();
            for q in quads { tris.push([q[0], q[1], q[2]]); tris.push([q[0], q[2], q[3]]); }
            mapgeom::static_item::build::make_invisible_mesh_waypoint(&mut mg, mapgeom::sttc::WP_FINISH, verts, tris).unwrap_or_else(die);
            mapgeom::static_item::build::add_finish_plane_visual(&mut mg, &[(x0, x1, z0, z1, 0.0)], mapgeom::floor_finish::INDICATOR_LIFT).unwrap_or_else(die);
            let (bytes, _) = mapgeom::static_item::build::finish_item(mg, &ident, &ident, 1.0, coll).unwrap_or_else(die);
            std::fs::write(&out, &bytes).unwrap_or_else(|e| die(format!("{out}: {e}")));
            println!("wrote {out}: {} bytes, ident {ident}, collection {coll}; trigger = one 32×1×32 box, visual = one indicator quad at y 0.02 (4 vertices, 2 triangles)", bytes.len());
        }
        // checkpoint-catalogue SOURCE.Map.Gbx --out MAP --mapping MAPPING.tsv [--name NAME]: Hugo's test map — every
        //   checkpoint block at North on a 4-cell grid (west half), again at East (east half), the multi-cell ones at
        //   South, a start and a finish; prints the slot table (block, group, cell, dir)
        "checkpoint-catalogue" => {
            let mut store = open(&a);
            let src = a.rest.get(1).cloned().unwrap_or_else(|| die("checkpoint-catalogue needs a SOURCE map".into()));
            let out = flag(&a.rest, "--out").unwrap_or_else(|| die("--out MAP".into()));
            let mapping = flag(&a.rest, "--mapping").unwrap_or_else(|| die("--mapping MAPPING.tsv".into()));
            let rules = mapgeom::checkpoint_catalogue::read_mapping(&mapping).unwrap_or_else(die);
            let slots = mapgeom::checkpoint_catalogue::build_catalogue(&mut store, &src, &out, &rules).unwrap_or_else(die);
            if let Some(name) = flag(&a.rest, "--name") {
                let hdr = tmmaps::header::read(&out).unwrap_or_else(|e| die(e));
                let mut m = tmmaps::map::MapFile::try_load(std::path::Path::new(&out)).unwrap_or_else(die);
                m.set_map_name(&hdr.name, &name);
                m.write_to(std::path::Path::new(&out)).unwrap_or_else(|e| die(e.to_string()));
            }
            if let Some(tsv) = flag(&a.rest, "--slots") {
                let mut s = String::from("block\tgroup\tcell_x\tcell_y\tcell_z\tdir\tvariant\n");
                for sl in &slots { s.push_str(&format!("{}\t{}\t{}\t{}\t{}\t{}\t{}\n", sl.name, sl.group, sl.cell[0], sl.cell[1], sl.cell[2], sl.dir, if sl.ground { "ground" } else { "air" })); }
                std::fs::write(&tsv, s).unwrap_or_else(|e| die(format!("{tsv}: {e}")));
            }
            let m = tmmaps::map::MapFile::try_load(std::path::Path::new(&out)).unwrap_or_else(die);
            let (mut sp, mut cp, mut go) = (0, 0, 0);
            for b in &m.blocks { match b.waypoint_tag.as_deref() { Some("Spawn") => sp += 1, Some("Checkpoint") => cp += 1, Some("Goal") => go += 1, _ => {} } }
            println!("wrote {out}: {} slots ({} rules), {} blocks, {} items; waypoints: {sp} Spawn, {cp} Checkpoint, {go} Goal", slots.len(), rules.len(), m.blocks.iter().filter(|b| b.flags != 0xFFFF_FFFF).count(), m.items.len());
        }
        // checkpoint-clean MAP --out MAP --mapping MAPPING.tsv: apply the mapping (the Rust twin of Hugo's CheckpointsCleaned)
        "checkpoint-clean" => {
            let mut store = open(&a);
            let src = a.rest.get(1).cloned().unwrap_or_else(|| die("checkpoint-clean needs a MAP".into()));
            let out = flag(&a.rest, "--out").unwrap_or_else(|| die("--out MAP".into()));
            let mapping = flag(&a.rest, "--mapping").unwrap_or_else(|| die("--mapping MAPPING.tsv".into()));
            let rules = mapgeom::checkpoint_catalogue::read_mapping(&mapping).unwrap_or_else(die);
            let (replaced, removed, left) = mapgeom::checkpoint_catalogue::clean(&mut store, &src, &out, &rules).unwrap_or_else(die);
            if let Some(name) = flag(&a.rest, "--name") {
                let hdr = tmmaps::header::read(&out).unwrap_or_else(|e| die(e));
                let mut m = tmmaps::map::MapFile::try_load(std::path::Path::new(&out)).unwrap_or_else(die);
                m.set_map_name(&hdr.name, &name);
                m.write_to(std::path::Path::new(&out)).unwrap_or_else(|e| die(e.to_string()));
            }
            let m = tmmaps::map::MapFile::try_load(std::path::Path::new(&out)).unwrap_or_else(die);
            let (mut sp, mut cp, mut go) = (0, 0, 0);
            for b in &m.blocks { match b.waypoint_tag.as_deref() { Some("Spawn") => sp += 1, Some("Checkpoint") => cp += 1, Some("Goal") => go += 1, _ => {} } }
            println!("wrote {out}: {replaced} checkpoint blocks replaced, {removed} removed, {} without a rule {:?}; waypoints: {sp} Spawn, {cp} Checkpoint, {go} Goal", left.len(), left);
        }
        // material-survey --out F.tsv [--filter SUB]: every material entry of the loaded packs (class 0x09079000, hashed
        //   or named): name guess (its textures' stem / the entry name), base shader (the Techno3 material it derives
        //   from), the CPlugMaterialCustom bitmaps / params / switches, its diffuse-or-intensity texture's mean and
        //   standard deviation (uniformity), and the derived flags: unlit (TAdd*/TMod*/Decal*/SpecialFX*/PreLightGen
        //   switch), animated (time / scroll / sequence params), vertex colour ("CV" / "VertexColor"), translucent
        "material-survey" => {
            let mut store = open(&a);
            let out = flag(&a.rest, "--out").unwrap_or_else(|| die("--out F.tsv".into()));
            let filt = flag(&a.rest, "--filter").map(|f| f.to_ascii_lowercase());
            let entries: Vec<String> = store.entries().filter(|e| e.class_id == 0x09079000).map(|e| e.path().to_string()).collect();
            let mut rows: Vec<String> = Vec::new();
            let header = "entry\tname\tbase_shader\tbitmaps\tparams\tswitches\ttexture\tsize\tmean_rgb\tstddev\tunlit\tanimated\tvertex_colour\ttranslucent\tno_lightmap_switch";
            println!("{header}");
            for e in &entries {
                let Ok(m) = store.load_model(e) else { continue };
                let Ok(g) = m.graph() else { continue };
                let mut base = String::new();
                let mut textures: Vec<String> = Vec::new();
                let mut dds: Vec<String> = Vec::new();
                for (_, p) in &m.externals {
                    let low = p.to_ascii_lowercase();
                    if low.contains("\\media\\material\\") && low.ends_with(".material.gbx") && (low.starts_with("techno") || low.starts_with("vehicles") || !low.starts_with("stadium")) { base = p.clone(); }
                    else if low.ends_with(".material.gbx") && base.is_empty() { base = p.clone(); }
                    if low.ends_with(".texture.gbx") || low.ends_with(".dds") { textures.push(p.clone()); }
                }
                // follow the textures to their dds
                for t in textures.clone() {
                    if t.to_ascii_lowercase().ends_with(".dds") { dds.push(t); continue; }
                    if let Ok(tm) = store.load_model(&t) {
                        for (_, p) in &tm.externals { if p.to_ascii_lowercase().ends_with(".dds") { dds.push(p.clone()); } }
                    }
                }
                let (mut bitmaps, mut params, mut switches) = (String::new(), String::new(), String::new());
                for sl in g.slots.iter() {
                    if let mapgeom::node::Slot::Node(mapgeom::node::Node::MaterialCustom(mc)) = sl {
                        bitmaps = mc.bitmaps.iter().map(|(n, _)| n.clone()).collect::<Vec<_>>().join(",");
                        params = mc.params.iter().map(|(n, v)| format!("{n}={:?}", v.iter().map(|x| (x * 1000.0).round() / 1000.0).collect::<Vec<_>>())).collect::<Vec<_>>().join(";");
                        switches = mc.switches.iter().map(|(n, v)| format!("{n}={}", *v as u8)).collect::<Vec<_>>().join(",");
                    }
                }
                let stem = |p: &str| p.rsplit('\\').next().unwrap_or(p).to_string();
                let name = if e.contains("\\Material") && !e.rsplit('\\').next().unwrap_or("").chars().all(|c| c.is_ascii_hexdigit()) {
                    stem(e).trim_end_matches(".Material.Gbx").to_string()
                } else {
                    dds.iter().map(|d| stem(d)).find(|s| s.contains('_')).map(|s| s.rsplit_once('_').map(|(a, _)| a.to_string()).unwrap_or(s)).unwrap_or_else(|| format!("?{}", &stem(e)[..8.min(stem(e).len())]))
                };
                if let Some(f) = &filt { if !name.to_ascii_lowercase().contains(f) && !base.to_ascii_lowercase().contains(f) { continue; } }
                // the diffuse / intensity image statistics
                let pick = dds.iter().find(|p| { let s = stem(p).to_ascii_lowercase(); s.ends_with("_d.dds") }).or_else(|| dds.iter().find(|p| stem(p).to_ascii_lowercase().ends_with("_i.dds"))).or_else(|| dds.first());
                let (mut tex, mut size, mut mean, mut sd) = (String::from("-"), String::from("-"), String::from("-"), String::from("-"));
                if let Some(img) = pick {
                    tex = stem(img);
                    if let Ok(b) = store.read(img) {
                        if let Ok((w, h, rgba)) = mapgeom::static_item::texture::decode_capped_rgba(&b, 128) {
                            size = format!("{w}x{h}");
                            let n = (rgba.len() / 4) as f64;
                            let mut acc = [0f64; 4];
                            for p in rgba.chunks_exact(4) { for k in 0..4 { acc[k] += p[k] as f64; } }
                            let mu: Vec<f64> = acc.iter().map(|v| v / n).collect();
                            let mut var = 0f64;
                            for p in rgba.chunks_exact(4) { for k in 0..3 { var += (p[k] as f64 - mu[k]).powi(2); } }
                            mean = format!("{:.0},{:.0},{:.0},a{:.0}", mu[0], mu[1], mu[2], mu[3]);
                            sd = format!("{:.1}", (var / (3.0 * n)).sqrt());
                        }
                    }
                }
                let bl = base.to_ascii_lowercase();
                let bstem = stem(&base).to_ascii_lowercase();
                let unlit_shader = bstem.contains(" tadd") || bstem.contains(" tmod") || bstem.contains("tadd") || bstem.contains("tmod") || bstem.contains("decal") || bstem.contains("specialfx") || bstem.contains("selfillum") || bstem.contains("unlit") || bstem.contains("tdecal");
                let no_lm_switch = switches.contains("PreLightGen=1");
                let pl = params.to_ascii_lowercase();
                let animated = pl.contains("anim") || pl.contains("scroll") || pl.contains("speed") || pl.contains("time") || pl.contains("sequence") || pl.contains("blink") || pl.contains("flow") || bstem.contains("anim") || bstem.contains("gate") || bitmaps.to_ascii_lowercase().contains("anim") || name.to_ascii_lowercase().contains("anim") && !name.to_ascii_lowercase().contains("noanim");
                let cv = bstem.contains("cv") || pl.contains("vertexcolor") || pl.contains("colorvertex") || bl.contains("cv.");
                let translucent = bstem.contains("tadd") || bstem.contains("tmod") || bstem.contains("talpha") || bstem.contains("trans") || bstem.contains("tdiffa") || bstem.contains("glass") || bstem.contains("tblend") || bstem.contains("alpha") || bstem.starts_with("tech3 block t") || bstem.contains(" t");
                let row = format!("{e}\t{name}\t{}\t{bitmaps}\t{params}\t{switches}\t{tex}\t{size}\t{mean}\t{sd}\t{}\t{}\t{}\t{}\t{}", stem(&base), (unlit_shader || no_lm_switch) as u8, animated as u8, cv as u8, translucent as u8, no_lm_switch as u8);
                println!("{row}");
                rows.push(row);
            }
            std::fs::write(&out, format!("{header}\n{}\n", rows.join("\n"))).unwrap_or_else(|e| die(format!("{out}: {e}")));
            eprintln!("wrote {out}: {} materials", rows.len());
        }
        // outer-terrain MAP.. --out F.tsv: the OUTER RING table (outer_ring.rs) of every map's collection/decoration,
        //   one row per rectangle (collection, decoration, x0, x1, z0, z1, floor, kind) + the measurement notes
        "outer-terrain" => {
            let mut store = open(&a);
            let out = flag(&a.rest, "--out").unwrap_or_else(|| die("--out F.tsv".into()));
            let maps: Vec<String> = a.rest.iter().skip(1).filter(|x| x.to_lowercase().ends_with(".map.gbx")).cloned().collect();
            if let Some(z) = flag(&a.rest, "--zone-step").and_then(|v| v.parse::<f32>().ok()) { mapgeom::outer_ring::set_zone_step(z); }
            let mut tsv = String::from("collection\tdecoration\tx0\tx1\tz0\tz1\tfloor\tkind\n");
            let mut seen: std::collections::BTreeSet<String> = Default::default();
            for mp in &maps {
                let m = tmmaps::map::MapFile::try_load(std::path::Path::new(mp)).unwrap_or_else(die);
                let coll = m.body_collections().map(|c| c[0].1).unwrap_or(26);
                let cname = mapgeom::sttc::collection_name(coll);
                let deco = if coll == 0x1a { m.decoration_id.trim_end_matches("Day").trim_end_matches("Night").trim_end_matches("Sunrise").trim_end_matches("Sunset").to_string() } else { "Base64x64".to_string() };
                let key = format!("{cname}/{deco}");
                if !seen.insert(key.clone()) { continue; }
                let cells: i32 = flag(&a.rest, "--cells").and_then(|v| v.parse().ok()).unwrap_or(mapgeom::outer_ring::ring_cells(coll));
                let r = mapgeom::outer_ring::ring_cells_of(&mut store, &m, coll, cells).unwrap_or_else(|e| die(format!("{mp}: {e}")));
                println!("== {key} ({} rects, {cells} cells)", r.rects.len());
                for n in &r.notes { println!("   {n}"); }
                tsv.push_str(&mapgeom::outer_ring::tsv_rows(cname, &deco, &r));
            }
            std::fs::write(&out, &tsv).unwrap_or_else(|e| die(format!("{out}: {e}")));
            println!("wrote {out}: {} rows", tsv.lines().count() - 1);
        }
        // cutout-verify --blocks A:FLAGS,..: for each exact cutout, sample every trigger-top triangle's centroid + edge
        //   midpoints and report any within the inset of a structure triangle (the "no overlap" check)
        "cutout-verify" => {
            let mut store = open(&a);
            let blocks = flag(&a.rest, "--blocks").unwrap_or_else(|| die("--blocks A:FLAGS,..".into()));
            let inset: f32 = flag(&a.rest, "--inset").and_then(|v| v.parse().ok()).unwrap_or(0.5);
            let mut bad_total = 0usize;
            for spec in blocks.split(',') {
                let (name, fl) = spec.split_once(':').unwrap_or((spec, "1000"));
                let flags = u32::from_str_radix(fl.trim_start_matches("0x"), 16).unwrap_or(0x1000);
                let (floor, structure) = mapgeom::floor_finish::floor_and_structure_of(&mut store, name, flags).unwrap_or_else(die);
                let size = mapgeom::floor_finish::variant_size_of(&mut store, name, flags).unwrap_or_else(die);
                let o = mapgeom::cutout_outline::trace(&floor, &structure, (32.0 * size.0 as f32, 32.0 * size.1 as f32), inset, mapgeom::cutout_outline::CONTOUR_STEP);
                let ((tv, tt), _) = mapgeom::cutout_outline::extrude(&o, 0.0, 1.0, 0.02);
                let tops: Vec<[u32; 3]> = tt.iter().filter(|t| t.iter().all(|&i| tv[i as usize][1] > 0.5)).cloned().collect();
                let (mut bad, mut samples, mut min_d) = (0usize, 0usize, f32::INFINITY);
                for t in &tops {
                    let (a3, b3, c3) = (tv[t[0] as usize], tv[t[1] as usize], tv[t[2] as usize]);
                    let pts = [[(a3[0] + b3[0] + c3[0]) / 3.0, (a3[2] + b3[2] + c3[2]) / 3.0], [(a3[0] + b3[0]) / 2.0, (a3[2] + b3[2]) / 2.0], [(b3[0] + c3[0]) / 2.0, (b3[2] + c3[2]) / 2.0], [(c3[0] + a3[0]) / 2.0, (c3[2] + a3[2]) / 2.0]];
                    for p in pts {
                        samples += 1;
                        let d = structure.iter().map(|s| mapgeom::floor_finish::dist_xz_pub(p, s)).fold(f32::INFINITY, f32::min);
                        min_d = min_d.min(d);
                        if d < inset - 0.02 { bad += 1; }
                    }
                }
                bad_total += bad;
                println!("{name}: {} top triangles, {samples} samples, {bad} within the inset of structure (min distance to structure {:.3} m)", tops.len(), min_d);
            }
            println!("# total violations: {bad_total}");
        }
        // cutout-outlines-tsv --out F.tsv --blocks A:FLAGS,B:FLAGS,..: every block's outline polygons as rows
        //   (block, polygon, ring (outer|hole n), vertex index, x, z) for the drawing tools
        "cutout-outlines-tsv" => {
            let mut store = open(&a);
            let out = flag(&a.rest, "--out").unwrap_or_else(|| die("--out F.tsv".into()));
            let blocks = flag(&a.rest, "--blocks").unwrap_or_else(|| die("--blocks A:FLAGS,..".into()));
            let inset: f32 = flag(&a.rest, "--inset").and_then(|v| v.parse().ok()).unwrap_or(0.5);
            let mut s = String::from("block\tflags\tsize_x\tsize_z\tpolygon\tring\ti\tx\tz\n");
            let mut summary = String::from("block\tflags\tpolygons\tholes\tvertices\ton_mesh\tarea_m2\tfill_tris\tstructure_tris\n");
            for spec in blocks.split(',') {
                let (name, fl) = spec.split_once(':').unwrap_or((spec, "1000"));
                let flags = u32::from_str_radix(fl.trim_start_matches("0x"), 16).unwrap_or(0x1000);
                let (floor, structure) = mapgeom::floor_finish::floor_and_structure_of(&mut store, name, flags).unwrap_or_else(die);
                let size = mapgeom::floor_finish::variant_size_of(&mut store, name, flags).unwrap_or_else(die);
                let o = mapgeom::cutout_outline::trace(&floor, &structure, (32.0 * size.0 as f32, 32.0 * size.1 as f32), inset, mapgeom::cutout_outline::CONTOUR_STEP);
                for (pi, (outer, holes)) in o.polygons.iter().enumerate() {
                    for (i, p) in outer.iter().enumerate() { s.push_str(&format!("{name}\t{flags:08x}\t{}\t{}\t{pi}\touter\t{i}\t{:.4}\t{:.4}\n", size.0, size.1, p[0], p[1])); }
                    for (hi, h) in holes.iter().enumerate() { for (i, p) in h.iter().enumerate() { s.push_str(&format!("{name}\t{flags:08x}\t{}\t{}\t{pi}\thole {hi}\t{i}\t{:.4}\t{:.4}\n", size.0, size.1, p[0], p[1])); } }
                }
                // the structure footprint too (for the drawing): its triangles' xz as rows ring = "structure"
                for (ti, t) in structure.iter().enumerate() { for (i, v) in t.iter().enumerate() { s.push_str(&format!("{name}\t{flags:08x}\t{}\t{}\t{ti}\tstructure\t{i}\t{:.4}\t{:.4}\n", size.0, size.1, v[0], v[2])); } }
                summary.push_str(&format!("{name}\t{flags:08x}\t{}\t{}\t{}\t{}\t{:.1}\t{}\t{}\n", o.polygons.len(), o.polygons.iter().map(|(_, h)| h.len()).sum::<usize>(), o.vertices, o.on_mesh, o.area, floor.len(), structure.len()));
                eprintln!("  {name}: {}", o.notes.join("; "));
            }
            std::fs::write(&out, &s).unwrap_or_else(|e| die(format!("{out}: {e}")));
            let sum_path = format!("{}.summary.tsv", out.trim_end_matches(".tsv"));
            std::fs::write(&sum_path, &summary).unwrap_or_else(|e| die(format!("{sum_path}: {e}")));
            println!("wrote {out} + {sum_path}");
        }
        // cutout-outline BLOCK [--flags HEX] [--inset 0.5] [--step 0.125] [--obj F] [--verts]: the exact-outline cutout
        //   polygon(s) of a block variant (cutout_outline.rs) — loops, vertex counts, area; --obj writes the prism
        "cutout-outline" => {
            let mut store = open(&a);
            let name = a.rest.get(1).cloned().unwrap_or_else(|| die("cutout-outline needs a BLOCK".into()));
            let flags: u32 = flag(&a.rest, "--flags").map(|v| u32::from_str_radix(v.trim_start_matches("0x"), 16).unwrap_or(0x1000)).unwrap_or(0x1000);
            let inset: f32 = flag(&a.rest, "--inset").and_then(|v| v.parse().ok()).unwrap_or(0.5);
            let step: f32 = flag(&a.rest, "--step").and_then(|v| v.parse().ok()).unwrap_or(mapgeom::cutout_outline::CONTOUR_STEP);
            let (floor, structure) = mapgeom::floor_finish::floor_and_structure_of(&mut store, &name, flags).unwrap_or_else(die);
            let size = mapgeom::floor_finish::variant_size_of(&mut store, &name, flags).unwrap_or_else(die);
            let o = mapgeom::cutout_outline::trace(&floor, &structure, (32.0 * size.0 as f32, 32.0 * size.1 as f32), inset, step);
            println!("{name} {flags:08x} ({}×{} cells): {}", size.0, size.1, o.notes.join("; "));
            for (i, (outer, holes)) in o.polygons.iter().enumerate() {
                println!("  polygon {i}: outer {} vertices, area {:.1} m², {} holes {:?}", outer.len(), mapgeom::floor_finish::polygon_area(outer), holes.len(), holes.iter().map(|h| h.len()).collect::<Vec<_>>());
                if a.rest.iter().any(|x| x == "--verts") {
                    for p in outer { println!("    {:.3} {:.3}", p[0], p[1]); }
                }
            }
            if let Some(obj) = flag(&a.rest, "--obj") {
                let ((v, t), _) = mapgeom::cutout_outline::extrude(&o, 0.0, 1.0, 0.02);
                let mut s = String::new();
                for p in &v { s.push_str(&format!("v {} {} {}\n", p[0], p[1], p[2])); }
                for q in &t { s.push_str(&format!("f {} {} {}\n", q[0] + 1, q[1] + 1, q[2] + 1)); }
                std::fs::write(&obj, s).unwrap_or_else(|e| die(format!("{obj}: {e}")));
                println!("  wrote {obj}: {} vertices, {} triangles", v.len(), t.len());
            }
        }
        // cutout-edges BLOCK [--flags HEX]: the standable fill's boundary polylines (edges of the fill triangles not
        //   shared by another fill triangle) — the pak's own curb outline resolution, for the exact-outline cutouts
        "cutout-edges" => {
            let mut store = open(&a);
            let name = a.rest.get(1).cloned().unwrap_or_else(|| die("cutout-edges needs a BLOCK".into()));
            let flags: u32 = flag(&a.rest, "--flags").map(|v| u32::from_str_radix(v.trim_start_matches("0x"), 16).unwrap_or(0x1000)).unwrap_or(0x1000);
            let (floor, structure) = mapgeom::floor_finish::floor_and_structure_of(&mut store, &name, flags).unwrap_or_else(die);
            let loops = mapgeom::floor_finish::boundary_loops(&floor);
            println!("{name}: {} fill triangles, {} structure triangles, {} boundary loops", floor.len(), structure.len(), loops.len());
            for (i, l) in loops.iter().enumerate() {
                let area = mapgeom::floor_finish::polygon_area(l);
                println!("  loop {i}: {} vertices, area {:.1} m², bbox x {:.2}..{:.2} z {:.2}..{:.2}", l.len(), area.abs(), l.iter().map(|p| p[0]).fold(f32::INFINITY, f32::min), l.iter().map(|p| p[0]).fold(f32::NEG_INFINITY, f32::max), l.iter().map(|p| p[1]).fold(f32::INFINITY, f32::min), l.iter().map(|p| p[1]).fold(f32::NEG_INFINITY, f32::max));
                if a.rest.iter().any(|x| x == "--verts") {
                    for p in l { println!("    {:.3} {:.3}", p[0], p[1]); }
                }
            }
        }
        // heightfield BLOCK [--dir D] [--all]: ASCII dump of a block's standable surface (or its collision silhouette with
        //   --all) on the 1 m grid — the checkpoint-map metric made visible (digits = metres, '.' = nothing)
        "heightfield" => {
            let mut store = open(&a);
            let name = a.rest.get(1).cloned().unwrap_or_else(|| die("heightfield needs a BLOCK name".into()));
            let dir: u8 = flag(&a.rest, "--dir").map(|v| v.parse().unwrap_or(0)).unwrap_or(0);
            let all = a.rest.iter().any(|x| x == "--all");
            let blocks = mapgeom::checkpoint_map::all_blocks(&mut store, None);
            let b = blocks.iter().find(|b| b.name.eq_ignore_ascii_case(&name)).unwrap_or_else(|| die(format!("{name}: no block info")));
            let bi = mapgeom::blockinfo::load(&mut store, &b.path).unwrap_or_else(die);
            let v = bi.variant_base_ground.as_ref().or(bi.variant_base_air.as_ref()).unwrap_or_else(|| die("no base variant".into()));
            let f = mapgeom::checkpoint_map::variant_heightfield(&mut store, v).unwrap_or_else(die).turned(dir);
            let w = 32 * f.size.0 as usize;
            println!("{name} dir {dir} size {}x{} standable {} m² ({})", f.size.0, f.size.1, f.covered(), if all { "silhouette" } else { "standable" });
            let field = if all { &f.all } else { &f.h };
            let mats = a.rest.iter().any(|x| x == "--mat");
            for z in (0..32 * f.size.1 as usize).rev() {
                let row: String = (0..w).map(|x| if mats { match field[z * w + x] { None => '.', Some(_) => { let m = f.mat[z * w + x]; if m < 10 { (b'0' + m) as char } else { (b'a' + (m - 10).min(25)) as char } } } } else { match field[z * w + x] { None => '.', Some(y) => { let d = y.round().clamp(0.0, 35.0) as i64; if d < 10 { (b'0' + d as u8) as char } else { (b'a' + (d - 10) as u8) as char } } } }).collect();
                println!("{row}");
            }
        }
        // checkpoint-map --out DIR [--hugo CheckpointsCleaned.cs]: every checkpoint block of the loaded paks → plain block +
        //   DirectionOffset by drivable-surface comparison (checkpoint_map.rs); writes MAPPING.tsv, DETAILS.tsv,
        //   CheckpointsCleaned.cs (Hugo's format, // Family groups, vista-only entries marked)
        //   ONE pak per run, --label = the collection it is (the vista paks re-author Stadium block infos under Stadium\
        //   paths); --merge A/MAPPING.tsv,B/MAPPING.tsv,.. combines per-collection runs into one dictionary instead
        "checkpoint-map" => {
            let out = flag(&a.rest, "--out").unwrap_or_else(|| die("checkpoint-map needs --out DIR".into()));
            std::fs::create_dir_all(&out).unwrap_or_else(|e| die(format!("{out}: {e}")));
            let hugo = flag(&a.rest, "--hugo").map(|f| mapgeom::checkpoint_map::parse_hugo(&std::fs::read_to_string(&f).unwrap_or_else(|e| die(format!("{f}: {e}"))))).unwrap_or_default();
            let label = flag(&a.rest, "--label");
            let mut log = Vec::new();
            let maps: Vec<mapgeom::checkpoint_map::Mapping> = if let Some(merge) = flag(&a.rest, "--merge") {
                let mut v = Vec::new();
                for f in merge.split(',') {
                    let text = std::fs::read_to_string(f.trim()).unwrap_or_else(|e| die(format!("{f}: {e}")));
                    v.extend(mapgeom::checkpoint_map::parse_mapping_tsv(&text));
                }
                v
            } else {
                // the collection = the FIRST --pak: its block infos only (listed from a store holding just that pak); the
                // other paks (Stadium) supply the prefabs those block infos reference
                let only: Option<std::collections::BTreeSet<String>> = a.paks.first().map(|spec| {
                    let (path, key) = match spec.rsplit_once(':') { Some((p, k)) if k.len() == 32 => (p.to_string(), k.to_string()), _ => (spec.clone(), a.key.clone()) };
                    let mut one = mapgeom::store::DataStore::empty();
                    one.add_pak(&path, &key).unwrap_or_else(die);
                    one.entries().map(|e| e.path().to_uppercase()).filter(|u| u.contains("\\GAMECTNBLOCKINFO\\") && u.ends_with(".GBX")).collect()
                });
                let mut store = open(&a);
                mapgeom::checkpoint_map::map_all(&mut store, label.as_deref(), only.as_ref(), &hugo, &mut log).unwrap_or_else(die)
            };
            for l in &log { eprintln!("  {l}"); }
            // MAPPING.tsv: one row per (collection, block)
            let mut tsv = String::from("collection\tblock\tsize\tvariant\twaypoint_type\tcovered_m2\treplacement\toffset\tmismatch\tverdict\tsecond\tsecond_offset\tsecond_mismatch\tcandidates\thugo_replacement\thugo_offset\thugo_mismatch\tagree\troad_mismatch\tzone_mismatch\n");
            let mut details = String::from("collection\tblock\trank\tcandidate\toffset\tmismatch\troad_mismatch\tzone_mismatch\n");
            // the dictionary: block → (replacement, offset, verdict, collections, agree-with-hugo); the first collection's
            // pick wins, others reported when they differ
            let mut dict: BTreeMap<String, (String, u8, &'static str, Vec<String>, Vec<String>)> = BTreeMap::new();
            for m in &maps {
                let v = mapgeom::checkpoint_map::verdict(m);
                let best = m.ranked.first();
                let second = m.ranked.iter().find(|c| best.map_or(true, |b| c.name != b.name || c.dir != b.dir));
                let h = hugo.get(&m.block);
                let hscore = h.and_then(|(hn, hd)| m.ranked.iter().find(|c| &c.name == hn && c.dir == *hd).map(|c| c.mismatch));
                let agree = match (h, best) { (Some((hn, hd)), Some(b)) => if &b.name == hn && b.dir == *hd { "yes" } else if hscore.map_or(false, |s| s <= mapgeom::checkpoint_map::MATCH_MAX_PUB) { "both-match" } else { "NO" }, (Some(_), None) => "NO", _ => "-" };
                tsv.push_str(&format!("{}\t{}\t{}x{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\n",
                    m.collection, m.block, m.size.0, m.size.1, if m.ground { "ground" } else { "air" }, m.waypoint_type.map(|w| w.to_string()).unwrap_or("-".into()), m.covered,
                    best.map(|b| b.name.clone()).unwrap_or("-".into()), best.map(|b| mapgeom::checkpoint_map::offset_name(b.dir)).unwrap_or("-"), best.map(|b| format!("{:.4}", b.mismatch)).unwrap_or("-".into()), v,
                    second.map(|c| c.name.clone()).unwrap_or("-".into()), second.map(|c| mapgeom::checkpoint_map::offset_name(c.dir)).unwrap_or("-"), second.map(|c| format!("{:.4}", c.mismatch)).unwrap_or("-".into()), m.candidates_tried,
                    h.map(|(n, _)| n.clone()).unwrap_or("-".into()), h.map(|(_, d)| mapgeom::checkpoint_map::offset_name(*d)).unwrap_or("-"), hscore.map(|s| format!("{s:.4}")).unwrap_or("-".into()), agree,
                    best.map(|b| format!("{:.4}", b.road_mismatch)).unwrap_or("-".into()), best.map(|b| format!("{:.4}", b.zone_mismatch)).unwrap_or("-".into())));
                for (i, c) in m.ranked.iter().take(6).enumerate() {
                    details.push_str(&format!("{}\t{}\t{}\t{}\t{}\t{:.4}\t{:.4}\t{:.4}\n", m.collection, m.block, i + 1, c.name, mapgeom::checkpoint_map::offset_name(c.dir), c.mismatch, c.road_mismatch, c.zone_mismatch));
                }
                let e = dict.entry(m.block.clone()).or_insert_with(|| (best.map(|b| b.name.clone()).unwrap_or_default(), best.map(|b| b.dir).unwrap_or(0), v, Vec::new(), Vec::new()));
                e.3.push(m.collection.clone());
                if let Some(b) = best { if b.name != e.0 || b.dir != e.1 { e.4.push(format!("{}: {} {}", m.collection, b.name, mapgeom::checkpoint_map::offset_name(b.dir))); } }
            }
            std::fs::write(format!("{out}/MAPPING.tsv"), &tsv).unwrap_or_else(|e| die(format!("{out}: {e}")));
            std::fs::write(format!("{out}/DETAILS.tsv"), &details).unwrap_or_else(|e| die(format!("{out}: {e}")));
            // CheckpointsCleaned.cs in Hugo's format: `// Family` groups in his order of appearance, vista-only marked
            let mut groups: BTreeMap<String, Vec<String>> = BTreeMap::new();
            let mut order: Vec<String> = Vec::new();
            for (block, _) in &dict {
                let fam = mapgeom::checkpoint_map::family_label(block);
                if !groups.contains_key(&fam) { order.push(fam.clone()); }
                groups.entry(fam).or_default().push(block.clone());
            }
            let mut cs = String::new();
            cs.push_str("        private static readonly Dictionary<string, (string Name, DirectionOffset Rotation)> BlockReplacements = new()\n        {\n");
            let mut first = true;
            for fam in &order {
                if !first { cs.push('\n'); }
                first = false;
                cs.push_str(&format!("            // {fam}\n"));
                for block in &groups[fam] {
                    let (rep, dir, v, colls, others) = &dict[block];
                    let vista_only = !colls.iter().any(|c| c == "Stadium");
                    let mut note = String::new();
                    if *v == "approx" { note.push_str(if block.contains("WallCheckpoint") { " // the standing deck = the 32 m wall on the same side (approx: the deck's thick ends differ)" } else { " // closest surface, not exact" }); }
                    if *v == "none" || *v == "no-candidate" { note.push_str(" // NO plain equivalent — remove the block (the closest surface is shown)"); }
                    if vista_only { note.push_str(&format!(" // vista only: {}", colls.join(", "))); }
                    if !others.is_empty() { note.push_str(&format!(" // differs by collection: {}", others.join("; "))); }
                    if rep.is_empty() || *v == "none" || *v == "no-candidate" {
                        cs.push_str(&format!("            // [\"{block}\"] = remove{note}\n"));
                    } else {
                        cs.push_str(&format!("            [\"{block}\"] = (\"{rep}\", DirectionOffset.{}),{note}\n", mapgeom::checkpoint_map::offset_name(*dir)));
                    }
                }
            }
            cs.push_str("        };\n");
            std::fs::write(format!("{out}/CheckpointsCleaned.cs"), &cs).unwrap_or_else(|e| die(format!("{out}: {e}")));
            // SUMMARY.tsv: block, replacement, offset, verified-by, mismatch, collections, size, hugo
            let mut sm = String::from("block\treplacement\toffset\tverdict\tverified_by\tmismatch\tcollections\tsize\thugo\troad_mismatch\tzone_mismatch\n");
            for (block, (rep, dir, v, colls, _)) in &dict {
                let m0 = maps.iter().find(|m| &m.block == block).unwrap();
                let wall = (m0.covered as f32) < 0.2 * (32 * 32 * m0.size.0 * m0.size.1) as f32;
                let by = if m0.ranked.is_empty() { "no candidate" } else if wall { "collision silhouette, 32 m wall squares, 1 m grid, 4 rotations" } else { "standable surface, 1 m grid, lowest height per square, 4 rotations, 0.1 m tolerance" };
                let h = match hugo.get(block) { Some((hn, hd)) => if hn == rep && *hd == *dir { "same".to_string() } else { format!("Hugo: {hn} {}", mapgeom::checkpoint_map::offset_name(*hd)) }, None => "new".into() };
                sm.push_str(&format!("{block}\t{}\t{}\t{v}\t{by}\t{}\t{}\t{}x{}\t{h}\t{}\t{}\n", if *v == "none" || *v == "no-candidate" { "(remove)".to_string() } else { rep.clone() }, mapgeom::checkpoint_map::offset_name(*dir), m0.ranked.first().map(|c| format!("{:.4}", c.mismatch)).unwrap_or("-".into()), colls.join(","), m0.size.0, m0.size.1, m0.ranked.first().map(|c| format!("{:.4}", c.road_mismatch)).unwrap_or("-".into()), m0.ranked.first().map(|c| format!("{:.4}", c.zone_mismatch)).unwrap_or("-".into())));
            }
            for (hn, (hr, hd)) in &hugo {
                if !dict.contains_key(hn) {
                    sm.push_str(&format!("{hn}\t{hr}\t{}\tdead\tno such block in any pak\t-\t-\t-\tHugo only\t-\t-\n", mapgeom::checkpoint_map::offset_name(*hd)));
                }
            }
            std::fs::write(format!("{out}/SUMMARY.tsv"), &sm).unwrap_or_else(|e| die(format!("{out}: {e}")));
            let n_match = maps.iter().filter(|m| mapgeom::checkpoint_map::verdict(m) == "match").count();
            let n_approx = maps.iter().filter(|m| mapgeom::checkpoint_map::verdict(m) == "approx").count();
            println!("wrote {out}/MAPPING.tsv ({} checkpoint blocks over the loaded collections: {n_match} match, {n_approx} approx, {} none), DETAILS.tsv, CheckpointsCleaned.cs ({} distinct blocks, {} families)", maps.len(), maps.len() - n_match - n_approx, dict.len(), order.len());
        }
        // ground-clippers MAP --out ITEM.Item.Gbx [--ident NAME]: Hugo's GroundClippers item — one box under the whole
        //   map, floor (lowest terrain row + bed) − 1.15 .. − 0.15
        "ground-clippers" => {
            let mut store = open(&a);
            let p = a.rest.get(1).cloned().unwrap_or_else(|| die("ground-clippers needs a MAP".into()));
            let out = flag(&a.rest, "--out").unwrap_or_else(|| die("ground-clippers needs --out ITEM.Item.Gbx".into()));
            let m = tmmaps::map::MapFile::try_load(std::path::Path::new(&p)).unwrap_or_else(die);
            let map_coll = m.body_collections().map(|c| c[0].1).unwrap_or(26);
            let hdr0 = tmmaps::header::read(&p).unwrap_or_else(|e| die(e));
            let ident = flag(&a.rest, "--ident").unwrap_or_else(|| format!("GroundClippers_{}.Item.Gbx", hdr0.uid));
            let (bytes, mesh) = mapgeom::floor_finish::build_ground_clippers_item(&mut store, &m, map_coll, &ident).unwrap_or_else(die);
            std::fs::write(&out, &bytes).unwrap_or_else(|e| die(format!("{out}: {e}")));
            for n in &mesh.notes {
                println!("  {n}");
            }
            println!("wrote {out}: {} bytes, ident {ident}, collection {map_coll}", bytes.len());
        }
        // floor-finish MAP --out ITEM.Item.Gbx [--ident NAME.Item.Gbx] [--floor-below 0.1] [--floor-above 0.4] [--collection 26]:
        //   the one-per-map Floor Finish carrier alone (Hugo embeds + places it himself at the map origin)
        "floor-finish" => {
            let mut store = open(&a);
            let p = a.rest.get(1).cloned().unwrap_or_else(|| die("floor-finish needs a MAP".into()));
            let out = flag(&a.rest, "--out").unwrap_or_else(|| die("floor-finish needs --out ITEM".into()));
            let m = tmmaps::map::MapFile::try_load(std::path::Path::new(&p)).unwrap_or_else(die);
            let hdr0 = tmmaps::header::read(&p).unwrap_or_else(|e| die(e));
            let map_coll = m.body_collections().map(|c| c[0].1).unwrap_or(26);
            let item_coll: u32 = flag(&a.rest, "--collection").map(|v| v.parse().unwrap_or_else(|_| die("--collection N".into()))).unwrap_or(26);
            let stadium = map_coll == 0x1a;
            let (vb, va) = mapgeom::floor_finish::vista_slab(map_coll);
            let below: f32 = flag(&a.rest, "--floor-below").map(|v| v.parse().unwrap_or_else(|_| die("--floor-below M".into()))).unwrap_or(if stadium { 0.0 } else { vb });
            let above: f32 = flag(&a.rest, "--floor-above").map(|v| v.parse().unwrap_or_else(|_| die("--floor-above M".into()))).unwrap_or(if stadium { 1.0 } else { va });
            let ident = flag(&a.rest, "--ident").unwrap_or_else(|| format!("FinishFloor_{}.Item.Gbx", hdr0.uid));
            // the mesh is computed in the MAP's collection frame (its ground row), the file carries `item_coll`
            let shores = !a.rest.iter().any(|x| x == "--no-shores");
            let (bytes, mesh) = mapgeom::floor_finish::build_floor_item(&mut store, &m, map_coll, &ident, below, above, shores, None).unwrap_or_else(die);
            for n in &mesh.notes { eprintln!("  {n}"); }
            let bytes = if item_coll != map_coll { mapgeom::tiny_assets::set_ident_collection(&bytes, item_coll) } else { bytes };
            std::fs::write(&out, &bytes).unwrap_or_else(|e| die(format!("{out}: {e}")));
            println!("wrote {out}: {} bytes, ident {ident}, collection {item_coll}; {} open-water cells → {} row strips + {} shore cells' water → {} 4 m boxes, {} vertices, {} triangles; slab bed −{below}..+{above}; world bbox [{:.1} {:.2} {:.1}]..[{:.1} {:.2} {:.1}]", bytes.len(), mesh.cells, mesh.strips, mesh.shore_boxes, mesh.shore_prisms, mesh.verts.len(), mesh.tris.len(), mesh.bbox.0[0], mesh.bbox.0[1], mesh.bbox.0[2], mesh.bbox.1[0], mesh.bbox.1[1], mesh.bbox.1[2]);
            if let Some(t) = flag(&a.rest, "--mesh-tsv") {
                // the row strips (8 vertices each) come first, then the shore prisms (6 each)
                let mut s = String::from("piece\tkind\tx0\tx1\ty_bed\tz0\tz1\n");
                // every piece is an 8-vertex box: the open-water row strips first (32 m rows), then the shore
                // water boxes (4 m rows)
                for (k, c) in mesh.verts.chunks(8).enumerate() {
                    let kind = if k < mesh.strips { "strip" } else { "shore-box" };
                    s.push_str(&format!("{k}\t{kind}\t{:.2}\t{:.2}\t{:.2}\t{:.2}\t{:.2}\n", c[0][0], c[1][0], c[0][1] + below, c[0][2], c[4][2]));
                }
                std::fs::write(&t, s).unwrap_or_else(|e| die(format!("{t}: {e}")));
            }
        }
        // flags MAP --out F [--model Flag16m,..] [--trigger-pad M] [--uid-prefix Flag] [--name-suffix " Flags"]
        //   [--unlock] [--unvalidated] [--author Nadeo] [--report R.tsv] [--dry-run]
        "flags" => {
            let mut store = open(&a);
            let p = a.rest.get(1).cloned().unwrap_or_else(|| die("flags needs a MAP".into()));
            let out = flag(&a.rest, "--out").unwrap_or_default();
            let dry = a.rest.iter().any(|x| x == "--dry-run");
            if out.is_empty() && flag(&a.rest, "--out-dir").is_none() {
                die::<()>("flags needs --out F (one map) or --out-dir DIR (a DIR)".into());
            }
            let o = mapgeom::flags::FlagsOpts {
                models: flag(&a.rest, "--model").map(|v| v.split(',').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect()).unwrap_or_default(),
                trigger_pad: flag(&a.rest, "--trigger-pad").map(|v| v.parse::<f32>().unwrap_or_else(|_| die("--trigger-pad M".into()))).unwrap_or(0.5),
                uid_prefix: flag(&a.rest, "--uid-prefix").unwrap_or_else(|| "Flag".into()),
                name_suffix: flag(&a.rest, "--name-suffix").unwrap_or_else(|| " Flags".into()),
                unlock: a.rest.iter().any(|x| x == "--unlock"),
                unvalidated: a.rest.iter().any(|x| x == "--unvalidated"),
                author: flag(&a.rest, "--author").unwrap_or_else(|| "Nadeo".into()),
                keep_lightmap: !a.rest.iter().any(|x| x == "--strip-lightmap"),
                pak_specs: a.paks.clone(),
                pole_triggers: !a.rest.iter().any(|x| x == "--flags-as-finish"),
                pole_margin: flag(&a.rest, "--pole-margin").map(|v| v.parse::<f32>().unwrap_or_else(|_| die("--pole-margin M".into()))).unwrap_or(0.05),
                pole_sides: flag(&a.rest, "--pole-sides").map(|v| v.parse::<usize>().unwrap_or_else(|_| die("--pole-sides N".into()))).unwrap_or(24),
                sttf: a.rest.iter().any(|x| x == "--sttf"),
                allow_no_finish: a.rest.iter().any(|x| x == "--allow-no-finish"),
                cloth: !a.rest.iter().any(|x| x == "--pole"),
                cloth_pad: flag(&a.rest, "--cloth-pad").map(|v| v.parse::<f32>().unwrap_or_else(|_| die("--cloth-pad M".into()))).unwrap_or(0.3),
                cloth_thick_pad: flag(&a.rest, "--cloth-thick-pad").map(|v| v.parse::<f32>().unwrap_or_else(|_| die("--cloth-thick-pad M".into()))).unwrap_or(0.35),
                stands: a.rest.iter().any(|x| x == "--stands"),
                stand_pad_below: flag(&a.rest, "--pad-below").map(|v| v.parse::<f32>().unwrap_or_else(|_| die("--pad-below M".into()))).unwrap_or(0.0),
                stand_thick: flag(&a.rest, "--thick").map(|v| v.parse::<f32>().unwrap_or_else(|_| die("--thick M".into()))).unwrap_or(0.5),
                ident_prefix: flag(&a.rest, "--ident-prefix").unwrap_or_default(),
                sources: Vec::new(),
                source_pad: flag(&a.rest, "--source-pad").map(|v| v.parse::<f32>().unwrap_or_else(|_| die("--source-pad M".into()))).unwrap_or(0.3),
                floor_item: None,
                floor_shores: !a.rest.iter().any(|x| x == "--no-shores"),
                ground_clippers: a.rest.iter().any(|x| x == "--ground-clippers"),
                cutout_squares: a.rest.iter().any(|x| x == "--squares"),
                no_ring: a.rest.iter().any(|x| x == "--no-ring"),
                floor_cutouts: a.rest.iter().any(|x| x == "--cutouts"),
            };
            // --lights SOURCES.tsv (Hugo's Moth, 2026-10-22): the light sources are the carriers' seats. The TSV = the
            // census LIGHTS.tsv (group, kind, name, …, local_x, local_y, local_z …); rows are taken when their group
            // is in --light-groups (default: the recommended set) — EMISSIVE rows give the emitting part's bbox, LIGHT
            // rows (a light node only) a 0.5-m box about the node; several rows of one model merge into one box.
            // --floor (Hugo's Floor Finish): every open-water tile of the map gets a full-cell finish slab at the water
            // surface; --floor-below M / --floor-above M set the slab's span under / over the surface (default 3 / 0.5),
            // --floor-label NAME the carrier ident stem (default Floor32m → FinishFloor32m.Item.Gbx with --ident-prefix Finish)
            // --floor (v2, Hugo 2026-10-23 08:20): ONE finish item per map, trigger = every open-water cell as a slab on the
            // lake bed (bed −0.1 .. +0.4 by default: --floor-below / --floor-above), world frame, placed at the origin;
            // ident FinishFloor_<map uid>.Item.Gbx (--floor-label overrides the stem). --floor-slabs = the v1 form (one
            // surface slab per tile, N items).
            let o = if a.rest.iter().any(|x| x == "--floor") {
                let hdr0 = tmmaps::header::read(&p).unwrap_or_else(|e| die(e));
                // Stadium (Hugo 12:00): the slab sits ON the grass, 1 m thick (grass .. +1.0); vistas (Hugo 2026-10-24
                // 00:12): bottom = bed − 0.1, top = water surface − 2.5 (a constant depth on every collection)
                let stadium = hdr0.envir.eq_ignore_ascii_case("Stadium");
                let m0 = tmmaps::map::MapFile::try_load(std::path::Path::new(&p)).unwrap_or_else(die);
                let coll0 = m0.body_collections().map(|c| c[0].1).unwrap_or(26);
                let (vb, va) = mapgeom::floor_finish::vista_slab(coll0);
                let below: f32 = flag(&a.rest, "--floor-below").map(|v| v.parse().unwrap_or_else(|_| die("--floor-below M".into()))).unwrap_or(if stadium { 0.0 } else { vb });
                let above: f32 = flag(&a.rest, "--floor-above").map(|v| v.parse().unwrap_or_else(|_| die("--floor-above M".into()))).unwrap_or(if stadium { 1.0 } else { va });
                let stem = flag(&a.rest, "--floor-label").unwrap_or_else(|| if a.rest.iter().any(|x| x == "--ground-clippers") { format!("GroundClippers_{}", hdr0.uid) } else { format!("FinishFloor_{}", hdr0.uid) });
                mapgeom::flags::FlagsOpts {
                    uid_prefix: flag(&a.rest, "--uid-prefix").unwrap_or_else(|| "Floor".into()),
                    name_suffix: flag(&a.rest, "--name-suffix").unwrap_or_else(|| " Floor Finish".into()),
                    pole_triggers: true,
                    floor_item: Some((stem, below, above)),
                    ..o
                }
            } else if a.rest.iter().any(|x| x == "--floor-slabs") {
                let m0 = tmmaps::map::MapFile::try_load(std::path::Path::new(&p)).unwrap_or_else(die);
                let collection = m0.body_collections().map(|c| c[0].1).unwrap_or(26);
                let below: f32 = flag(&a.rest, "--floor-below").map(|v| v.parse().unwrap_or_else(|_| die("--floor-below M".into()))).unwrap_or(3.0);
                let above: f32 = flag(&a.rest, "--floor-above").map(|v| v.parse().unwrap_or_else(|_| die("--floor-above M".into()))).unwrap_or(0.5);
                let label = flag(&a.rest, "--floor-label").unwrap_or_else(|| "Floor32m".into());
                let sources = floor_sources(collection, below, above, &label);
                if sources.is_empty() { die::<()>(format!("--floor: collection {collection:#x} has no open-water tile table (vista collections only)")); }
                eprintln!("floor finish: tiles {:?}, slab y {:.2}..{:.2} in the block frame", sources.iter().map(|s| s.key.as_str()).collect::<Vec<_>>(), sources[0].lo[1], sources[0].hi[1]);
                mapgeom::flags::FlagsOpts {
                    uid_prefix: flag(&a.rest, "--uid-prefix").unwrap_or_else(|| "Floor".into()),
                    name_suffix: flag(&a.rest, "--name-suffix").unwrap_or_else(|| " Floor Finish".into()),
                    pole_triggers: true,
                    source_pad: flag(&a.rest, "--source-pad").map(|v| v.parse::<f32>().unwrap_or_else(|_| die("--source-pad M".into()))).unwrap_or(0.0),
                    ident_prefix: flag(&a.rest, "--ident-prefix").unwrap_or_else(|| "Finish".into()),
                    sources,
                    ..o
                }
            } else if let Some(tsv) = flag(&a.rest, "--lights") {
                let mut sources = light_sources_from_tsv(&tsv, flag(&a.rest, "--light-groups"), flag(&a.rest, "--light-exclude"));
                if let Some(parts) = flag(&a.rest, "--lantern-parts") {
                    let mats: Vec<String> = flag(&a.rest, "--lantern-materials").map(|v| v.split(',').map(|s| s.trim().to_string()).collect()).unwrap_or_else(|| vec!["ThemeSnowTempleLamp".into(), "LightShape".into(), "ThemeSnowTempleTransparent".into()]);
                    let lonly: Vec<String> = flag(&a.rest, "--lantern-only").map(|v| v.split(',').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect()).unwrap_or_default();
                    let lant = lantern_sources_from_parts(&parts, &mats, &lonly);
                    sources.retain(|s| !lant.iter().any(|l| l.key == s.key));
                    sources.extend(lant);
                }
                eprintln!("{} light-source kinds from {tsv}", sources.len());
                mapgeom::flags::FlagsOpts {
                    uid_prefix: flag(&a.rest, "--uid-prefix").unwrap_or_else(|| "Moth".into()),
                    name_suffix: flag(&a.rest, "--name-suffix").unwrap_or_else(|| " Moth".into()),
                    pole_triggers: true,
                    sources,
                    ..o
                }
            } else {
                o
            };

            // --stands (Hugo's Manslaughter): the stand BLOCKS are the sources; identity "<name> Manslaughter", uid Mans…
            let o = if o.stands {
                mapgeom::flags::FlagsOpts {
                    uid_prefix: flag(&a.rest, "--uid-prefix").unwrap_or_else(|| "Mans".into()),
                    name_suffix: flag(&a.rest, "--name-suffix").unwrap_or_else(|| " Manslaughter".into()),
                    pole_triggers: true,
                    ..o
                }
            } else {
                o
            };
            // a DIR: every *.Map.Gbx in it → --out-dir DIR (names "<map name> Flags.Map.Gbx")
            let srcp = std::path::Path::new(&p);
            let inputs: Vec<std::path::PathBuf> = if srcp.is_dir() {
                let mut v: Vec<std::path::PathBuf> = std::fs::read_dir(srcp).unwrap_or_else(|e| die(e.to_string())).filter_map(|e| e.ok()).map(|e| e.path()).filter(|q| q.to_string_lossy().ends_with(".Map.Gbx")).collect();
                v.sort();
                if let Some(o) = flag(&a.rest, "--only") {
                    let pats: Vec<String> = o.split(',').map(|s| s.trim().to_string()).collect();
                    v.retain(|q| { let n = q.file_name().map(|s| s.to_string_lossy().to_string()).unwrap_or_default(); pats.iter().any(|pp| n.starts_with(pp.as_str())) });
                }
                v
            } else {
                vec![srcp.to_path_buf()]
            };
            let out_dir = flag(&a.rest, "--out-dir");
            if srcp.is_dir() && out_dir.is_none() {
                die::<()>("a DIR needs --out-dir DIR".into());
            }
            let mut tsv = String::from(mapgeom::sttc::REPORT_HEADER);
            tsv.push('\n');
            let mut failed = 0usize;
            let t0 = std::time::Instant::now();
            for src in &inputs {
                let label = src.file_name().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
                let outp: std::path::PathBuf = match &out_dir {
                    Some(d) => {
                        let hdr = tmmaps::header::read(src.to_str().unwrap_or_default()).unwrap_or_else(|e| die(e));
                        std::path::Path::new(d).join(format!("{}{}.Map.Gbx", hdr.name, o.name_suffix))
                    }
                    None => std::path::PathBuf::from(&out),
                };
                let oc = match mapgeom::flags::flags(&mut store, src, &outp, &o, dry) {
                    Ok(oc) => oc,
                    Err(e) => {
                        failed += 1;
                        let mut r = mapgeom::sttc::Row::new(&label, "summary", "map", 0, "-");
                        r.action = "FAILED".into();
                        r.note = e.clone();
                        tsv.push_str(&r.tsv());
                        tsv.push('\n');
                        eprintln!("{label}: FAILED: {e}");
                        continue;
                    }
                };
                for r in &oc.rows {
                    tsv.push_str(&r.tsv());
                    tsv.push('\n');
                }
                let mut sum = mapgeom::sttc::Row::new(&label, "summary", "map", 0, &oc.new_name);
                sum.action = format!("{} {} {} ({}); strip: {} blocks -> plain twins, {} blocks / {} items / {} generated removed{}", oc.converted, if o.stands { "stand blocks" } else { "flags" }, if o.stands { "kept + seating-slab finish carriers appended" } else if o.pole_triggers { "kept + pole finish triggers appended" } else { "-> finish items" }, oc.items.iter().map(|(s, i, _)| format!("{s} -> {i}")).collect::<Vec<_>>().join(", "), oc.replaced_blocks, oc.removed_blocks, oc.finishes_removed, oc.removed_baked, if o.sttf { " (finishes + checkpoints)" } else { " (finishes)" });
                if !dry {
                    let bad = mapgeom::flags::verify_flags(src, &outp, &oc, &o).unwrap_or_else(|e| die(e));
                    sum.note = if bad.is_empty() { "verified".into() } else { failed += 1; format!("VERIFY FAILED: {}", bad.join("; ")) };
                } else {
                    sum.note = "dry run".into();
                }
                tsv.push_str(&sum.tsv());
                tsv.push('\n');
                println!("{}", sum.tsv());
            }
            if inputs.len() > 1 {
                println!("{} maps, {} failed, {:.1}s", inputs.len(), failed, t0.elapsed().as_secs_f32());
            }
            if let Some(r) = flag(&a.rest, "--report") {
                if let Some(d) = std::path::Path::new(&r).parent() {
                    let _ = std::fs::create_dir_all(d);
                }
                std::fs::write(&r, &tsv).unwrap_or_else(|e| die(e.to_string()));
                println!("wrote {r}");
            }
            if failed > 0 {
                std::process::exit(2);
            }
        }
        // sandbox <MAP|DIR> --out-dir DIR [--fill PlatformTechBase] [--fill-flags HEX] [--row N] [--uid-prefix Sbox]
        //   [--name-suffix " Sandbox"] [--unlock] [--unvalidated] [--keep-lightmap] [--keep-genealogy]
        //   [--out-name mapname] [--only ..] [--report R.tsv] [--inventory I.tsv] [--dry-run]
        "sandbox" => {
            let mut store = open(&a);
            let p = a.rest.get(1).cloned().unwrap_or_else(|| die("sandbox needs a MAP or a directory".into()));
            let dry = a.rest.iter().any(|x| x == "--dry-run");
            let out_dir = flag(&a.rest, "--out-dir").unwrap_or_else(|| die("sandbox needs --out-dir DIR".into()));
            let sb = mapgeom::sandbox::SandboxOpts {
                fill: flag(&a.rest, "--fill").unwrap_or_else(|| "PlatformTechBase".into()),
                fill_flags: flag(&a.rest, "--fill-flags").map(|v| u32::from_str_radix(v.trim_start_matches("0x"), 16).unwrap_or_else(|_| die("--fill-flags HEX".into()))).unwrap_or(0x1000),
                clear_genealogy: a.rest.iter().any(|x| x == "--clear-genealogy"),
                row: flag(&a.rest, "--row").map(|v| v.parse::<i32>().unwrap_or_else(|_| die("--row N".into()))),
                sink_items: !a.rest.iter().any(|x| x == "--items-on-deck"),
                keep_ring_blocks: !a.rest.iter().any(|x| x == "--rings-to-platform"),
                uid_prefix: flag(&a.rest, "--uid-prefix").unwrap_or_else(|| "Sbox".into()),
                name_suffix: flag(&a.rest, "--name-suffix").unwrap_or_else(|| " Sandbox".into()),
                unlock: a.rest.iter().any(|x| x == "--unlock"),
                unvalidated: a.rest.iter().any(|x| x == "--unvalidated"),
                strip_lightmap: !a.rest.iter().any(|x| x == "--keep-lightmap"),
            };
            let opts = mapgeom::sandbox::PipelineOpts { sandbox: sb, dry, out_by_map_name: flag(&a.rest, "--out-name").as_deref() == Some("mapname") };
            let path = std::path::Path::new(&p);
            let mut maps: Vec<std::path::PathBuf> = if path.is_dir() {
                let mut v: Vec<std::path::PathBuf> = std::fs::read_dir(path).unwrap_or_else(|e| die(e.to_string())).filter_map(|e| e.ok()).map(|e| e.path()).filter(|q| q.to_string_lossy().ends_with(".Map.Gbx")).collect();
                v.sort();
                v
            } else {
                vec![path.to_path_buf()]
            };
            if let Some(only) = flag(&a.rest, "--only") {
                let keep: Vec<String> = only.split(',').map(|s| s.trim().to_string()).collect();
                maps.retain(|q| keep.iter().any(|k| q.file_name().map(|f| f.to_string_lossy().starts_with(k.as_str())).unwrap_or(false)));
            }
            let mut tsv = String::from(mapgeom::sttc::REPORT_HEADER);
            tsv.push('\n');
            let mut inv_all = String::new();
            let mut failed = 0;
            let t0 = std::time::Instant::now();
            for mp in &maps {
                let t = std::time::Instant::now();
                match mapgeom::sandbox::pipeline(&mut store, mp, std::path::Path::new(&out_dir), &opts) {
                    Ok((rows, summary, oc)) => {
                        for r in &rows {
                            tsv.push_str(&r.tsv());
                            tsv.push('\n');
                        }
                        if inv_all.is_empty() {
                            inv_all.push_str(&oc.inventory);
                        } else {
                            inv_all.push_str(oc.inventory.splitn(2, '\n').nth(1).unwrap_or(""));
                        }
                        println!("{summary}\t{:.1}s", t.elapsed().as_secs_f32());
                    }
                    Err(e) => {
                        failed += 1;
                        let mut r = mapgeom::sttc::Row::default();
                        r.map = mp.file_name().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
                        r.step = "summary".into();
                        r.kind = "map".into();
                        r.action = "FAILED".into();
                        r.note = e.clone();
                        tsv.push_str(&r.tsv());
                        tsv.push('\n');
                        eprintln!("{}: FAILED: {e}", mp.display());
                    }
                }
            }
            if let Some(r) = flag(&a.rest, "--report") {
                if let Some(d) = std::path::Path::new(&r).parent() {
                    let _ = std::fs::create_dir_all(d);
                }
                std::fs::write(&r, &tsv).unwrap_or_else(|e| die(e.to_string()));
                println!("wrote {r}");
            }
            if let Some(i) = flag(&a.rest, "--inventory") {
                if let Some(d) = std::path::Path::new(&i).parent() {
                    let _ = std::fs::create_dir_all(d);
                }
                std::fs::write(&i, &inv_all).unwrap_or_else(|e| die(e.to_string()));
                println!("wrote {i}");
            }
            println!("{} maps, {} failed, {:.1}s{}", maps.len(), failed, t0.elapsed().as_secs_f32(), if dry { " (dry run)" } else { "" });
            if failed > 0 {
                std::process::exit(2);
            }
        }
        // podium-reverse <MAP|DIR> --out-dir DIR [--start-item GateStartCenter8m] [--rear M] [--gap M]
        //   [--podium N] [--uid-prefix PdRv] [--name-suffix " Podium Reverse"] [--times none|keep|A,G,S,B]
        //   [--unlock] [--lightmap keep-renumber|keep] [--out-name mapname] [--only ..] [--report R.tsv] [--dry-run]
        "podium-reverse" => {
            let mut store = open(&a);
            let p = a.rest.get(1).cloned().unwrap_or_else(|| die("podium-reverse needs a MAP or a directory".into()));
            let dry = a.rest.iter().any(|x| x == "--dry-run");
            let out_dir = flag(&a.rest, "--out-dir").unwrap_or_else(|| die("podium-reverse needs --out-dir DIR".into()));
            let keep_renumber = matches!(flag(&a.rest, "--lightmap").as_deref(), None | Some("keep-renumber"));
            // --times none (default): 59:59.999 for every medal — a placeholder no player reads as a
            // Nadeo time (Hugo: "I don't want Nadeo times"; the club upload needs SOME author time)
            let times = match flag(&a.rest, "--times").as_deref() {
                None | Some("none") => Some((3_599_999u32, 3_599_999u32, 3_599_999u32, 3_599_999u32)),
                Some("keep") => None,
                Some(spec) => {
                    let v: Vec<u32> = spec.split(',').map(|x| x.trim().parse::<u32>().unwrap_or_else(|_| die(format!("--times A,G,S,B in ms, not `{spec}`")))).collect();
                    if v.len() != 4 {
                        die::<()>("--times none | keep | AUTHOR,GOLD,SILVER,BRONZE (ms)".into());
                    }
                    Some((v[3], v[2], v[1], v[0]))
                }
            };
            let podium = mapgeom::podium::PodiumOpts {
                start_item: flag(&a.rest, "--start-item").unwrap_or_else(|| "GateStartCenter8m".into()),
                rear: flag(&a.rest, "--rear").map(|v| v.parse::<f32>().unwrap_or_else(|_| die("--rear M".into()))).unwrap_or(2.0),
                gap: flag(&a.rest, "--gap").map(|v| v.parse::<f32>().unwrap_or_else(|_| die("--gap M".into()))).unwrap_or(0.0),
                podium: flag(&a.rest, "--podium").map(|v| v.parse::<usize>().unwrap_or_else(|_| die("--podium N".into()))),
                uid_prefix: flag(&a.rest, "--uid-prefix").unwrap_or_else(|| "PdRv".into()),
                name_suffix: flag(&a.rest, "--name-suffix").unwrap_or_else(|| " Podium Reverse".into()),
                times,
                unlock: a.rest.iter().any(|x| x == "--unlock"),
            };
            // --podium-map "03=1,09=1": per-map podium picks by file-name prefix
            let podium_map: Vec<(String, usize)> = flag(&a.rest, "--podium-map").map(|v| v.split(',').filter_map(|kv| kv.split_once('=').map(|(k, n)| (k.trim().to_string(), n.trim().parse::<usize>().unwrap_or(0)))).collect()).unwrap_or_default();
            let path = std::path::Path::new(&p);
            let mut maps: Vec<std::path::PathBuf> = if path.is_dir() {
                let mut v: Vec<std::path::PathBuf> = std::fs::read_dir(path).unwrap_or_else(|e| die(e.to_string())).filter_map(|e| e.ok()).map(|e| e.path()).filter(|q| q.to_string_lossy().ends_with(".Map.Gbx")).collect();
                v.sort();
                v
            } else {
                vec![path.to_path_buf()]
            };
            if let Some(only) = flag(&a.rest, "--only") {
                let keep: Vec<String> = only.split(',').map(|s| s.trim().to_string()).collect();
                maps.retain(|q| keep.iter().any(|k| q.file_name().map(|f| f.to_string_lossy().starts_with(k.as_str())).unwrap_or(false)));
            }
            let mut tsv = String::from(mapgeom::sttc::REPORT_HEADER);
            tsv.push('\n');
            let mut failed = 0;
            let t0 = std::time::Instant::now();
            for mp in &maps {
                let t = std::time::Instant::now();
                let mut po = podium.clone();
                if let Some((_, n)) = podium_map.iter().find(|(k, _)| mp.file_name().map(|f| f.to_string_lossy().starts_with(k.as_str())).unwrap_or(false)) {
                    po.podium = Some(*n);
                }
                let opts = mapgeom::podium::PodiumPipelineOpts { podium: po, dry, keep_renumber, pak_specs: a.paks.clone(), out_by_map_name: flag(&a.rest, "--out-name").as_deref() == Some("mapname") };
                match mapgeom::podium::pipeline(&mut store, mp, std::path::Path::new(&out_dir), &opts) {
                    Ok((rows, summary, _)) => {
                        for r in &rows {
                            tsv.push_str(&r.tsv());
                            tsv.push('\n');
                        }
                        println!("{summary}\t{:.1}s", t.elapsed().as_secs_f32());
                        if summary.contains("VERIFY FAILED") {
                            failed += 1;
                        }
                    }
                    Err(e) => {
                        failed += 1;
                        let mut r = mapgeom::sttc::Row::default();
                        r.map = mp.file_name().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
                        r.step = "summary".into();
                        r.kind = "map".into();
                        r.action = "FAILED".into();
                        r.note = e.clone();
                        tsv.push_str(&r.tsv());
                        tsv.push('\n');
                        eprintln!("{}: FAILED: {e}", mp.display());
                    }
                }
            }
            if let Some(r) = flag(&a.rest, "--report") {
                if let Some(d) = std::path::Path::new(&r).parent() {
                    let _ = std::fs::create_dir_all(d);
                }
                std::fs::write(&r, &tsv).unwrap_or_else(|e| die(e.to_string()));
                println!("wrote {r}");
            } else if maps.len() == 1 {
                print!("{tsv}");
            }
            println!("{} maps, {} failed, {:.1}s{}", maps.len(), failed, t0.elapsed().as_secs_f32(), if dry { " (dry run)" } else { "" });
            if failed > 0 {
                std::process::exit(2);
            }
        }
        "sttc" => {
            let mut store = open(&a);
            let p = a.rest.get(1).cloned().unwrap_or_else(|| die("sttc needs a MAP or a directory".into()));
            let dry = a.rest.iter().any(|x| x == "--dry-run");
            let out_dir = flag(&a.rest, "--out-dir").unwrap_or_else(|| die("sttc needs --out-dir DIR".into()));
            let keep_renumber = matches!(flag(&a.rest, "--lightmap").as_deref(), None | Some("keep-renumber"));
            let opts = mapgeom::sttc::PipelineOpts { cp: sttc_cp_mode(&a.rest), center: sttc_center_opts(&a.rest), dry, keep_sttf: true, keep_renumber, pak_specs: a.paks.clone(), no_sttf: a.rest.iter().any(|x| x == "--no-sttf"), out_by_map_name: flag(&a.rest, "--out-name").as_deref() == Some("mapname") };
            let path = std::path::Path::new(&p);
            let mut maps: Vec<std::path::PathBuf> = if path.is_dir() {
                let mut v: Vec<std::path::PathBuf> = std::fs::read_dir(path).unwrap_or_else(|e| die(e.to_string())).filter_map(|e| e.ok()).map(|e| e.path()).filter(|q| q.to_string_lossy().ends_with(".Map.Gbx")).collect();
                v.sort();
                v
            } else {
                vec![path.to_path_buf()]
            };
            if let Some(only) = flag(&a.rest, "--only") {
                let keep: Vec<String> = only.split(',').map(|s| s.trim().to_string()).collect();
                maps.retain(|q| keep.iter().any(|k| q.file_name().map(|f| f.to_string_lossy().starts_with(k.as_str())).unwrap_or(false)));
            }
            let mut tsv = String::from(mapgeom::sttc::REPORT_HEADER);
            tsv.push('\n');
            let mut failed = 0;
            let t0 = std::time::Instant::now();
            for mp in &maps {
                let t = std::time::Instant::now();
                match mapgeom::sttc::pipeline(&mut store, mp, std::path::Path::new(&out_dir), &opts) {
                    Ok((rows, summary)) => {
                        for r in &rows {
                            tsv.push_str(&r.tsv());
                            tsv.push('\n');
                        }
                        println!("{summary}\t{:.1}s", t.elapsed().as_secs_f32());
                        if summary.contains("VERIFY FAILED") {
                            failed += 1;
                        }
                    }
                    Err(e) => {
                        failed += 1;
                        let mut r = mapgeom::sttc::Row::default();
                        r.map = mp.file_name().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
                        r.step = "summary".into();
                        r.kind = "map".into();
                        r.action = "FAILED".into();
                        r.note = e.clone();
                        tsv.push_str(&r.tsv());
                        tsv.push('\n');
                        eprintln!("{}: FAILED: {e}", mp.display());
                    }
                }
            }
            if let Some(r) = flag(&a.rest, "--report") {
                if let Some(d) = std::path::Path::new(&r).parent() {
                    let _ = std::fs::create_dir_all(d);
                }
                std::fs::write(&r, &tsv).unwrap_or_else(|e| die(e.to_string()));
                println!("wrote {r}");
            } else if maps.len() == 1 {
                print!("{tsv}");
            }
            println!("{} maps, {} failed, {:.1}s{}", maps.len(), failed, t0.elapsed().as_secs_f32(), if dry { " (dry run)" } else { "" });
            if failed > 0 {
                std::process::exit(1);
            }
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
                let bytes = match std::fs::read(p) { Ok(b) => b, Err(_) => match store.read(p) { Ok(b) => b.to_vec(), Err(e) => { println!("{p}: {e}"); continue; } } };
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
            std::fs::write(&out, &bytes[..]).unwrap_or_else(|e| die(e.to_string()));
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
            // --box X0,Y0,Z0:X1,Y1,Z1: only the triangles with a vertex inside the
            // world box are written (a corner of the map for `rend`)
            if let Some(b) = flag(&a.rest, "--box") {
                let (lo, hi) = b.split_once(':').ok_or_else(|| "--box X0,Y0,Z0:X1,Y1,Z1".to_string()).unwrap_or_else(|e| die(e));
                let p = |s: &str| -> [f32; 3] {
                    let v: Vec<f32> = s.split(',').filter_map(|x| x.trim().parse().ok()).collect();
                    if v.len() != 3 {
                        die::<()>(format!("--box: `{s}` is not X,Y,Z"));
                    }
                    [v[0], v[1], v[2]]
                };
                let (lo, hi) = (p(lo), p(hi));
                scene = scene.cropped([lo[0].min(hi[0]), lo[1].min(hi[1]), lo[2].min(hi[2])], [lo[0].max(hi[0]), lo[1].max(hi[1]), lo[2].max(hi[2])]);
                println!("--box {b}: {} triangles kept", scene.tri_count());
            }
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
            // --plates without --ghost (the build pipeline wants the plate table only): an
            // empty ghost — the per-sample legs come out empty, the plates are complete
            let mut d = match gbx::record::decode_ghost(&gpath) {
                Ok(d) => d,
                Err(e) if flag(&a.rest, "--ghost").is_none() && flag(&a.rest, "--plates").is_some() => {
                    eprintln!("{gpath}: no ghost ({e}); plates only");
                    gbx::record::Decoded { path: gpath.clone(), name: String::new(), version: 0, start_ms: 0, end_ms: 0, sample_period_ms: None, ..Default::default() }
                }
                Err(e) => die(format!("{gpath}: no ghost ({e})")),
            };
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

/// `--cp plain|remove`: what a checkpoint block becomes (sttf, sttc).
fn sttc_cp_mode(rest: &[String]) -> mapgeom::sttc::CpMode {
    match flag(rest, "--cp").as_deref() {
        None | Some("plain") => mapgeom::sttc::CpMode::Plain,
        Some("remove") => mapgeom::sttc::CpMode::Remove,
        Some(o) => die(format!("--cp plain | remove, not `{o}`")),
    }
}

/// The center-finish options shared by `center-finish` and `sttc`.
fn sttc_center_opts(rest: &[String]) -> mapgeom::sttc::CenterOpts {
    use mapgeom::sttc::{Center, CenterOpts, Occupied};
    let center = match flag(rest, "--center").as_deref() {
        None | Some("auto") => Center::Auto,
        Some("bbox") => Center::Bbox,
        Some(s) => {
            let (x, z) = s.split_once(',').unwrap_or_else(|| die("--center auto | bbox | X,Z (cells: 23.5,23.5)".into()));
            Center::At(x.trim().parse().unwrap_or_else(|_| die(format!("--center: `{x}` is not a number"))), z.trim().parse().unwrap_or_else(|_| die(format!("--center: `{z}` is not a number"))))
        }
    };
    let occupied = match flag(rest, "--occupied").as_deref() {
        None | Some("overlap") => Occupied::Overlap,
        Some("raise") => Occupied::Raise,
        Some("fail") => Occupied::Fail,
        Some(o) => die(format!("--occupied overlap | raise | fail, not `{o}`")),
    };
    CenterOpts {
        center,
        occupied,
        include_startfinish: rest.iter().any(|x| x == "--include-startfinish"),
        rename: !rest.iter().any(|x| x == "--no-rename"),
        reuid: !rest.iter().any(|x| x == "--no-reuid"),
        strip_lightmap: match flag(rest, "--lightmap").as_deref() {
            Some("strip") => true,
            None | Some("keep") | Some("keep-renumber") => false,
            Some(o) => die(format!("--lightmap strip | keep | keep-renumber, not `{o}`")),
        },
        // --rule v1|v2 (default v2: bbox centre, ties down, items/free exact — Hugo's final rule 2026-10-02)
        rule: match flag(rest, "--rule").as_deref() {
            None | Some("v3") => mapgeom::sttc::Rule::V3,
            Some("v2") => mapgeom::sttc::Rule::V2,
            Some("v1") => mapgeom::sttc::Rule::V1,
            Some(o) => die(format!("--rule v1 | v2 | v3, not `{o}`")),
        },
        strip_name_suffix: flag(rest, "--strip-name-suffix"),
        // --uid-table FILE: TSV with a name column and a uid column (publish results: path name uid …; or name uid)
        uid_table: flag(rest, "--uid-table").map(|p| {
            let text = std::fs::read_to_string(&p).unwrap_or_else(|e| die(format!("--uid-table {p}: {e}")));
            let mut t = std::collections::HashMap::new();
            for l in text.lines() {
                let c: Vec<&str> = l.split('\t').collect();
                if c.len() >= 3 && c[2].len() >= 20 && !c[2].contains(' ') {
                    t.insert(c[1].to_string(), c[2].to_string());
                } else if c.len() >= 2 && c[1].len() >= 20 && !c[1].contains(' ') && !c[0].contains('/') {
                    t.insert(c[0].to_string(), c[1].to_string());
                }
            }
            if t.is_empty() {
                die::<()>(format!("--uid-table {p}: no name/uid rows found"));
            }
            t
        }),
        // --originals DIR: the original maps (times + the base name's existence) by header name
        originals: flag(rest, "--originals").map(|d| {
            let mut t = std::collections::HashMap::new();
            for e in std::fs::read_dir(&d).unwrap_or_else(|e| die(format!("--originals {d}: {e}"))).flatten() {
                let p = e.path();
                if !p.to_string_lossy().ends_with(".Map.Gbx") {
                    continue;
                }
                if let Ok(h) = tmmaps::header::read(p.to_str().unwrap_or_default()) {
                    let n = |s: &str| s.parse::<u32>().unwrap_or_else(|_| die(format!("{}: time `{s}`", p.display())));
                    t.insert(h.name.clone(), (p.clone(), (n(&h.bronze), n(&h.silver), n(&h.gold), n(&h.authortime))));
                }
            }
            if t.is_empty() {
                die::<()>(format!("--originals {d}: no maps"));
            }
            t
        }),
        unlock: rest.iter().any(|x| x == "--unlock"),
        item_offset: match flag(rest, "--item-offset").as_deref() {
            None | Some("auto") => mapgeom::sttc::ItemOffset::Auto,
            Some("exact") => mapgeom::sttc::ItemOffset::Exact,
            Some("rounded") => mapgeom::sttc::ItemOffset::Rounded,
            Some(o) => die(format!("--item-offset exact | rounded | auto, not `{o}`")),
        },
    }
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
        Node::Layout(l) => {
            let mut s = format!("CSceneLayout v{} {} lights, {} mobils, extra {}", l.version, l.lights.len(), l.mobils.len(), l.extra);
            for (i, li) in l.lights.iter().enumerate() {
                s.push_str(&format!(
                    "\n      light {i} {:?} pos [{:.3}, {:.3}, {:.3}] rot xyzw [{:.4}, {:.4}, {:.4}, {:.4}] v{} bitmaps {:?} light node {} u01 {}",
                    li.name, li.pos[0], li.pos[1], li.pos[2], li.rot[0], li.rot[1], li.rot[2], li.rot[3], li.version, li.bitmaps, li.light, li.u01
                ));
            }
            for (i, m) in l.mobils.iter().enumerate() {
                s.push_str(&format!(
                    "\n      mobil {i} {:?} pos [{:.3}, {:.3}, {:.3}] rot xyzw [{:.4}, {:.4}, {:.4}, {:.4}] u01 {} flags 0x{:x} solid node {} u02 {} prefab {}",
                    m.name, m.pos[0], m.pos[1], m.pos[2], m.rot[0], m.rot[1], m.rot[2], m.rot[3], m.u01, m.flags, m.solid, m.u02, m.prefab
                ));
            }
            s.push_str(&format!("\n      weather node {} env bitmaps (water fog, cube reflect hard-spec A, cube reflect hdr-alpha2) {:?} params {:?} u03 {:?} u04 {}", l.weather, l.env_bitmaps, l.params, l.u03, l.u04));
            s
        }
        Node::Tree(t) => format!(
            "CPlugTree {:?} children {:?} visual {} shader {} surface {} flags 0x{:x} transform {}",
            t.name, t.children, t.visual, t.shader, t.surface, t.flags,
            t.transform.map(|m| format!("rot {:?} pos [{}, {}, {}]", &m[..9], m[9], m[10], m[11])).unwrap_or_else(|| "-".into())
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
        Node::ImageArray(a) => {
            let mut s = format!("CPlugImageArray v{} folder {:?} ({} layers; decal folder {:?})", a.version, a.folder, a.layers.len(), a.decal_folder);
            for (i, l) in a.layers.iter().enumerate() {
                s.push_str(&format!(
                    "\n      [{i}] {:<14} Py {:?} off {:?}  Pxz {:?} off {:?}  rot {}°  blendPxz {:?}°  blendPy {:?}°  ids {:?}",
                    l.name, l.py_scale, l.py_offset, l.pxz_scale, l.pxz_offset, l.rotation_deg, l.blend_pxz_deg, l.blend_py_deg, l.ids
                ));
            }
            s
        }
        Node::Bitmap(b) => format!(
            "CPlugBitmap image {} tc_scale_trans {:?} array: image-array {} suffix {:?} slices {:?}",
            b.image,
            b.tc_scale_trans.map(|v| [f32::from_bits(v[0]), f32::from_bits(v[1]), f32::from_bits(v[2]), f32::from_bits(v[3])]),
            b.array_image_array,
            b.array_suffix,
            b.array_slices
        ),
        Node::FileGen(g) => format!("CPlugFileGen v{} kind {} u32s {:?} float4s {:?} f32s {:?} name {:?}", g.version, g.kind, g.u32s, g.float4s, g.f32s, g.name),
        Node::MaterialCustom(m) => format!("CPlugMaterialCustom layer names (Pxz, Py, X2, H2) {:?} (mode {}) bitmaps {:?} params {:?} switches {:?}", m.layer_names, m.layer_mode, m.bitmaps, m.params, m.switches),
        Node::Other(c) => format!("class 0x{:08X}", c),
    }
}
