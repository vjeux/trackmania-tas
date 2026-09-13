//! `tinyctl build NN…` — the tiny build of one or more campaign maps, end to
//! end and without a shell: the source map from the campaign directory, the
//! packs for its collection, the recipe environment, `mapgeom tiny-library`,
//! `tmmaps tiny`, the library unzipped next to the map for `publish-map`.
//!
//! ```text
//! tinyctl build 20 21 [--src-dir /tmp/summer2026] [--out-root /tmp] [--tag auto]
//!               [--recipe /tmp/tiny3/recipe.env] [--env K=V …] [--bin-dir DIR]
//!               [--lod-pick N [--lod-pick-min-verts V]] [--debug NAMES]
//! ```
//!
//! Output for map NN goes to `<out-root>/tinyNN/<tag>/` (default tag `auto`):
//! lib.zip, placements.tsv, report.tsv, build.log, Summer-NN-Tiny.Map.Gbx,
//! tiny.log, libx/. The packs are the fixed campaign set: the collection's
//! pack (key 660C…) plus the Stadium pack (key B773…); a Stadium map gets the
//! Stadium pack alone. `--env` adds or overrides variables (TINY_FLAG_TWEEN=0 …);
//! `--lod-pick` / `--lod-pick-min-verts` / `--debug` are handed to `mapgeom`
//! as its global flags.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::views::{collection_name, collection_of};

const STADIUM_PAK: &str = "/tmp/current-Stadium.pak:B773D73047A4104857722366D78D28A6";
const TERRAIN_KEY: &str = "660C4C156B80337E296A1034B0AA05B8";

/// The `--pak F:KEY` arguments for a collection.
pub fn paks_for(collection: u32) -> Result<Vec<String>, String> {
    let terrain = match collection {
        0x1a => None,
        0x1c => Some("/tmp/BlueBay.pak"),
        0x10 => Some("/tmp/RedIsland.pak"),
        0x1d => Some("/tmp/WhiteShore.pak"),
        0xf => Some("/tmp/GreenCoast.pak"),
        other => return Err(format!("collection {other:#x}: no pack table entry")),
    };
    let mut v = Vec::new();
    if let Some(t) = terrain {
        v.push("--pak".to_string());
        v.push(format!("{t}:{TERRAIN_KEY}"));
    }
    v.push("--pak".to_string());
    v.push(STADIUM_PAK.to_string());
    Ok(v)
}

/// `export A=B C="D E"` lines of a recipe file as variables.
fn recipe_env(path: &Path) -> Result<BTreeMap<String, String>, String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let mut out = BTreeMap::new();
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let line = line.strip_prefix("export ").unwrap_or(line);
        // split on spaces outside quotes
        let mut tokens: Vec<String> = Vec::new();
        let mut cur = String::new();
        let mut quote: Option<char> = None;
        for ch in line.chars() {
            match (quote, ch) {
                (Some(q), c) if c == q => quote = None,
                (Some(_), c) => cur.push(c),
                (None, '"') | (None, '\'') => quote = Some(ch),
                (None, ' ') | (None, '\t') => {
                    if !cur.is_empty() {
                        tokens.push(std::mem::take(&mut cur));
                    }
                }
                (None, c) => cur.push(c),
            }
        }
        if !cur.is_empty() {
            tokens.push(cur);
        }
        for t in tokens {
            if let Some((k, v)) = t.split_once('=') {
                out.insert(k.to_string(), v.to_string());
            }
        }
    }
    Ok(out)
}

fn run(cmd: &mut Command, log: &Path) -> Result<String, String> {
    let out = cmd.output().map_err(|e| format!("{:?}: {e}", cmd.get_program()))?;
    let mut text = String::from_utf8_lossy(&out.stdout).into_owned();
    text.push_str(&String::from_utf8_lossy(&out.stderr));
    std::fs::write(log, &text).map_err(|e| format!("{}: {e}", log.display()))?;
    if !out.status.success() {
        return Err(format!("{:?} failed ({}); log {}", cmd.get_program(), out.status, log.display()));
    }
    Ok(text)
}

pub fn cmd(args: &[String]) -> Result<(), String> {
    let f = |k: &str| tmmaps::cli::flag(args, k).map(String::from);
    let src_dir = PathBuf::from(f("--src-dir").unwrap_or_else(|| "/tmp/summer2026".into()));
    let out_root = PathBuf::from(f("--out-root").unwrap_or_else(|| "/tmp".into()));
    let tag = f("--tag").unwrap_or_else(|| "auto".into());
    // --out-prefix P: the output is `<P>-NN-Tiny.Map.Gbx` (default `Summer`, the
    // campaign this was written for; `U10S` for Everios96's club maps, 2026-09-12)
    let out_prefix = out_prefix(args);
    // --scale S (default 0.5, the tiny build): the item scale handed to
    // `mapgeom tiny-library` and `tmmaps tiny`. Above 1 the build is a GIANT one
    // (`<P>-NN-Giant.Map.Gbx`, name "Giant <source>", uid `Gia2…`, the geometry
    // centred in the grid by `--anchor fit`) — 2026-09-13, vjeux: "instead of
    // tiny you make giant maps. Every item is 2x instead of 1/2".
    let scale = scale_of(args);
    let label = variant_label(scale);
    let recipe = PathBuf::from(f("--recipe").unwrap_or_else(|| "/tmp/tiny3/recipe.env".into()));
    // mapgeom's global flags, passed through
    let mut mapgeom_flags: Vec<String> = Vec::new();
    for k in ["--lod-pick", "--lod-pick-min-verts", "--debug"] {
        if let Some(v) = f(k) {
            mapgeom_flags.push(k.to_string());
            mapgeom_flags.push(v);
        }
    }
    let bin_dir = f("--bin-dir").map(PathBuf::from).or_else(|| std::env::current_exe().ok().and_then(|p| p.parent().map(|d| d.to_path_buf()))).ok_or("--bin-dir DIR")?;
    // --env K=V (repeatable)
    let mut extra: Vec<(String, String)> = Vec::new();
    let mut i = 0;
    while i < args.len() {
        if args[i] == "--env" {
            if let Some(kv) = args.get(i + 1) {
                if let Some((k, v)) = kv.split_once('=') {
                    extra.push((k.to_string(), v.to_string()));
                }
            }
            i += 2;
            continue;
        }
        i += 1;
    }
    // map numbers: every bare argument of two digits
    let maps: Vec<String> = args.iter().filter(|a| a.len() == 2 && a.chars().all(|c| c.is_ascii_digit())).cloned().collect();
    if maps.is_empty() {
        return Err("build needs map numbers (tinyctl build 20 21 …)".into());
    }
    let base_env = recipe_env(&recipe)?;
    let mapgeom = bin_dir.join("mapgeom");
    let tmmaps = bin_dir.join("tmmaps");
    for p in [&mapgeom, &tmmaps] {
        if !p.exists() {
            return Err(format!("{}: no such binary", p.display()));
        }
    }
    let mut failed = 0usize;
    for nn in &maps {
        let src = std::fs::read_dir(&src_dir)
            .map_err(|e| format!("{}: {e}", src_dir.display()))?
            .filter_map(|e| e.ok())
            .map(|e| e.path())
            .find(|p| p.file_name().map(|n| n.to_string_lossy().starts_with(&format!("{nn}-")) && n.to_string_lossy().ends_with(".Map.Gbx")).unwrap_or(false));
        let Some(src) = src else {
            eprintln!("{nn}: no {nn}-*.Map.Gbx in {}", src_dir.display());
            failed += 1;
            continue;
        };
        let m = tmmaps::map::MapFile::load(&src);
        let coll = collection_of(&m);
        let paks = match paks_for(coll) {
            Ok(p) => p,
            Err(e) => {
                eprintln!("{nn}: {e}");
                failed += 1;
                continue;
            }
        };
        let out = out_root.join(format!("tiny{nn}")).join(&tag);
        std::fs::create_dir_all(&out).map_err(|e| format!("{}: {e}", out.display()))?;
        let mut env = base_env.clone();
        for (k, v) in &extra {
            env.insert(k.clone(), v.clone());
        }
        // Item names unique per map AND per build: the game caches an embedded
        // item MODEL by its file name for the whole session, so two maps (or two
        // builds of one map) embedding different pieces under "AC00000200.Item.Gbx"
        // show the first-loaded model in the second map. Found 2026-09-09 chasing
        // "slabs" on tiny 20 that were ship13's pieces under ship14's names;
        // vjeux plays several tiny maps in one session. AC{map:02}{minute%1000:03}{idx:03}.
        if !env.contains_key("TINY_ALIAS_BASE") {
            let minutes = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs() / 60).unwrap_or(0) as usize;
            let map_no: usize = nn.parse().unwrap_or(0);
            env.insert("TINY_ALIAS_BASE".to_string(), format!("{}", map_no * 1_000_000 + (minutes % 1000) * 1000));
        }
        // A GIANT build's water is the engine's own (`mapgeom giantwater`: native
        // pool tiles + custom road volumes, TINY_WATER_NATIVE=0 turns it off), so
        // the items drop their water quads — the blocks draw the surface.
        let native_water = scale > 1.0 && env.get("TINY_WATER_NATIVE").map(|v| v != "0").unwrap_or(true);
        if native_water && !env.contains_key("TINY_WATER_VISUAL") {
            env.insert("TINY_WATER_VISUAL".to_string(), "0".to_string());
        }
        // the generated pictures (sign logos, screen picture, trigger FX) are cached
        // by file name the same way: suffix them with the alias base's minute part
        if !env.contains_key("TINY_PICTURE_SUFFIX") {
            let base = env["TINY_ALIAS_BASE"].clone();
            env.insert("TINY_PICTURE_SUFFIX".to_string(), format!("_{}", &base[base.len().saturating_sub(6)..base.len().saturating_sub(3)]));
        }
        println!("{nn}: {} ({}) -> {} (alias base {})", src.file_name().unwrap_or_default().to_string_lossy(), collection_name(coll), out.display(), env["TINY_ALIAS_BASE"]);
        let t0 = std::time::Instant::now();
        let mut lib = Command::new(&mapgeom);
        lib.args(&paks).args(&mapgeom_flags).arg("tiny-library").arg(&src).arg("--library-out").arg(out.join("lib.zip")).arg("--mapping-out").arg(out.join("placements.tsv")).arg("--report").arg(out.join("report.tsv")).arg("--scale").arg(format!("{scale}"));
        lib.envs(env.iter());
        match run(&mut lib, &out.join("build.log")) {
            Ok(text) => {
                for l in text.lines().filter(|l| l.contains("library:") || l.contains("FAILED") || l.contains("pictures:")) {
                    println!("  {}", l.trim());
                }
            }
            Err(e) => {
                eprintln!("  {e}");
                failed += 1;
                continue;
            }
        }
        let tiny_out = out.join(format!("{out_prefix}-{nn}-{label}.Map.Gbx"));
        let run_tiny = |env: &BTreeMap<String, String>, log: &str| -> Result<String, String> {
            let mut tiny = Command::new(&tmmaps);
            tiny.arg("tiny").arg(&src).arg("--mapping").arg(out.join("placements.tsv")).arg("--library").arg(out.join("lib.zip")).arg("--out").arg(&tiny_out).arg("--scale").arg(format!("{scale}"));
            // a giant build: centred in the grid, its own uid head and name
            if scale > 1.0 {
                tiny.arg("--anchor").arg("fit").arg("--uid-prefix").arg(uid_prefix(scale)).arg("--name-prefix").arg(format!("{label} "));
            }
            // --keep-zone-block: one authored zone block survives (the editor's
            // lightmapper crashes on a build with no block at all — GreenCoast 04/09,
            // 2026-09-12; BlueBay builds keep their baked Sea records and bake fine)
            if tmmaps::cli::has(args, "--keep-zone-block") {
                tiny.arg("--keep-zone-block");
            }
            tiny.envs(env.iter());
            run(&mut tiny, &out.join(log))
        };
        let mut anchor_line = String::new();
        match run_tiny(&env, "tiny.log") {
            Ok(text) => {
                for l in text.lines().filter(|l| l.contains("uid:") || l.contains("scaled every") || l.contains("anchor:") || l.contains("deleted")) {
                    println!("  {}", l.trim());
                    if l.contains("anchor: source") {
                        anchor_line = l.trim().to_string();
                    }
                }
            }
            Err(e) => {
                eprintln!("  {e}");
                failed += 1;
                continue;
            }
        }
        // The coplanar pass (mapgeom coplanar-sinks, 2026-09-09): a free clip
        // whose top face lies in an authored deck's top face is lowered a
        // centimetre (`yb@` rows) and the map is written again.
        if let Some(anchor) = anchor_arg(&anchor_line) {
            let sinks = out.join("sinks.tsv");
            let mut cs = Command::new(&mapgeom);
            cs.args(&paks).args(&mapgeom_flags).arg("coplanar-sinks").arg(&tiny_out).arg("--source").arg(&src).arg("--mapping").arg(out.join("placements.tsv")).arg("--anchor").arg(&anchor.0).arg("--scale").arg(&anchor.1).arg("--out").arg(&sinks);
            cs.envs(env.iter());
            match run(&mut cs, &out.join("coplanar.log")) {
                Ok(text) => {
                    let rows = std::fs::read_to_string(&sinks).unwrap_or_default();
                    let n = rows.lines().filter(|l| !l.trim().is_empty()).count();
                    if n > 0 {
                        for l in text.lines().filter(|l| l.contains("coplanar over")) {
                            println!("  {}", l.trim());
                        }
                        let mut mapping = std::fs::read_to_string(out.join("placements.tsv")).map_err(|e| format!("placements.tsv: {e}"))?;
                        mapping.push_str("# coplanar free clips sunk under their deck (mapgeom coplanar-sinks)\n");
                        mapping.push_str(&rows);
                        std::fs::write(out.join("placements.tsv"), mapping).map_err(|e| format!("placements.tsv: {e}"))?;
                        match run_tiny(&env, "tiny2.log") {
                            Ok(text) => {
                                for l in text.lines().filter(|l| l.contains("coplanar free clips sunk")) {
                                    println!("  {}", l.trim());
                                }
                            }
                            Err(e) => {
                                eprintln!("  {e}");
                                failed += 1;
                                continue;
                            }
                        }
                    }
                }
                Err(e) => eprintln!("  coplanar pass skipped: {e}"),
            }
        }
        // The WATER BLOCKS pass (2026-09-11, ship17c's recipe as a build default): the
        // map's water plates (`mapgeom waterline --plates`, no ghost) are tiled with
        // free custom WaterBase blocks where the spill rule allows (`mapgeom
        // waterblocks`), and the map is written again with the block records and the
        // block file in its archive. TINY_WATER_BLOCKS=0 leaves the 13-item form.
        // A GIANT build takes the native pass below instead.
        if native_water {
            // The GIANT water pass (2026-09-13): the source's pool blocks as native
            // grid tiles in the transformed cells, the water roads as free custom
            // volumes — `mapgeom giantwater` (TINY.md "Giant maps: water").
            match anchor_arg(&anchor_line) {
                Some((anchor, scale_s)) => {
                    let template = out.join("water-template.Block.Gbx");
                    std::fs::write(&template, WATER_TEMPLATE).map_err(|e| format!("{}: {e}", template.display()))?;
                    let staged = out.join(format!("{out_prefix}-{nn}-{label}.water.Map.Gbx"));
                    let table = out.join("giant-water.tsv");
                    let mut gw = Command::new(&mapgeom);
                    gw.args(&paks).args(&mapgeom_flags).arg("giantwater").arg(&tiny_out).arg("--source").arg(&src).arg("--anchor").arg(&anchor).arg("--scale").arg(&scale_s).arg("--template").arg(&template).arg("--table").arg(&table).arg("--out").arg(&staged);
                    // The water ROADS stay items unless asked (TINY_GIANT_ROAD_TILES=1):
                    // the volume tiles grow the archetype's 1× fillers in the open —
                    // rounded dead-end caps, the green start tubes — on the ×2 canal
                    // (vjeux, 2026-09-13 17:44Z: "not what I wanted … this changes the
                    // layout"); on a pool the same fillers hide inside the ×2 walls.
                    if env.get("TINY_GIANT_ROAD_TILES").map(|v| v != "1").unwrap_or(true) {
                        gw.arg("--no-roads");
                    }
                    gw.envs(env.iter());
                    match run(&mut gw, &out.join("giantwater.log")) {
                        Ok(text) => {
                            for l in text.lines().filter(|l| l.contains("giantwater") || l.contains("giant water")) {
                                println!("  {}", l.trim());
                            }
                            std::fs::rename(&staged, &tiny_out).map_err(|e| format!("giant water: {e}"))?;
                        }
                        Err(e) => {
                            eprintln!("  giant water pass FAILED: {e}");
                            failed += 1;
                            continue;
                        }
                    }
                }
                None => {
                    eprintln!("  giant water pass FAILED: no anchor line from tmmaps tiny");
                    failed += 1;
                    continue;
                }
            }
        } else if std::env::var("TINY_WATER_BLOCKS").map(|v| v != "0").unwrap_or(true) && scale < 1.0 {
            match anchor_arg(&anchor_line) {
                Some((anchor, _scale)) => {
                    let plates = out.join("water-plates.tsv");
                    let mut wl = Command::new(&mapgeom);
                    wl.args(&paks).args(&mapgeom_flags).arg("waterline").arg(&tiny_out).arg("--report").arg(out.join("report.tsv")).arg("--plates").arg(&plates);
                    wl.envs(env.iter());
                    match run(&mut wl, &out.join("waterline.log")) {
                        Ok(_) => {
                            let template = out.join("water-template.Block.Gbx");
                            std::fs::write(&template, WATER_TEMPLATE).map_err(|e| format!("{}: {e}", template.display()))?;
                            let staged = out.join(format!("{out_prefix}-{nn}-{label}.water.Map.Gbx"));
                            let table = out.join("water-bodies.tsv");
                            let mut wb = Command::new(&mapgeom);
                            wb.args(&paks).args(&mapgeom_flags).arg("waterblocks").arg(&tiny_out).arg("--plates").arg(&plates).arg("--template").arg(&template).arg("--out").arg(&staged).arg("--table").arg(&table).arg("--source").arg(&src).arg("--anchor").arg(&anchor);
                            wb.envs(env.iter());
                            match run(&mut wb, &out.join("waterblocks.log")) {
                                Ok(text) => {
                                    for l in text.lines().filter(|l| l.contains("water bodies handled") || l.contains("free blocks")) {
                                        println!("  {}", l.trim());
                                    }
                                    std::fs::rename(&staged, &tiny_out).map_err(|e| format!("water blocks: {e}"))?;
                                }
                                Err(e) => eprintln!("  water blocks pass skipped: {e}"),
                            }
                        }
                        Err(e) => eprintln!("  water plates skipped: {e}"),
                    }
                }
                None => eprintln!("  water blocks pass skipped: no anchor line"),
            }
        }
        let libx = out.join("libx");
        let _ = std::fs::remove_dir_all(&libx);
        let unzip = Command::new("unzip").arg("-q").arg(out.join("lib.zip")).arg("-d").arg(&libx).output().map_err(|e| format!("unzip: {e}"))?;
        if !unzip.status.success() {
            eprintln!("  unzip failed: {}", String::from_utf8_lossy(&unzip.stderr).trim());
            failed += 1;
            continue;
        }
        let n_items = std::fs::read_dir(libx.join("Items")).map(|rd| rd.count()).unwrap_or(0);
        let size = std::fs::metadata(&tiny_out).map(|m| m.len()).unwrap_or(0);
        println!("  {} ({:.1} MB), {n_items} library files, {:.0} s", tiny_out.display(), size as f64 / 1e6, t0.elapsed().as_secs_f64());
    }
    if failed > 0 {
        return Err(format!("{failed} of {} maps failed", maps.len()));
    }
    Ok(())
}

/// `--out-prefix P` (default `Summer`): the stem of a build's map file,
/// `<P>-NN-Tiny.Map.Gbx`. Shared by `build` and `publish-map --tag`.
pub fn out_prefix(args: &[String]) -> String {
    tmmaps::cli::flag(args, "--out-prefix").unwrap_or("Summer").to_string()
}

/// `--scale S` (default 0.5): the build's item scale.
pub fn scale_of(args: &[String]) -> f32 {
    let s: f32 = tmmaps::cli::flag(args, "--scale").unwrap_or("0.5").parse().unwrap_or(0.5);
    if s.is_finite() && s > 0.0 { s } else { 0.5 }
}

/// The word in a build's file name and map name for its scale: `Tiny` under 1,
/// `Giant` above, `Same` at exactly 1 (a scale-1 rebuild, a diagnostic).
pub fn variant_label(scale: f32) -> &'static str {
    if scale < 1.0 {
        "Tiny"
    } else if scale > 1.0 {
        "Giant"
    } else {
        "Same"
    }
}

/// The 4-byte uid head of a build at `scale` (`tmmaps tiny --uid-prefix`):
/// `Tin2` for the tiny builds (the second tiny uid scheme), `Gia2` for the giant
/// ones, `Sam2` at scale 1.
pub fn uid_prefix(scale: f32) -> &'static str {
    match variant_label(scale) {
        "Giant" => "Gia2",
        "Same" => "Sam2",
        _ => "Tin2",
    }
}

/// The map file a build of map `nn` writes: `<out-root>/tinyNN/<tag>/<P>-NN-<Label>.Map.Gbx`.
pub fn built_map_name(args: &[String], nn: &str) -> String {
    format!("{}-{nn}-{}.Map.Gbx", out_prefix(args), variant_label(scale_of(args)))
}

/// The `--anchor sx,sy,sz:tx,ty,tz` and `--scale S` arguments for the
/// coplanar pass, from `tmmaps tiny`'s "anchor: source [x, y, z] -> target
/// [x, y, z]; scale 0.500" line.
pub fn anchor_arg(line: &str) -> Option<(String, String)> {
    let nums = |s: &str| -> Option<String> {
        let inner = s.split_once('[')?.1.split_once(']')?.0;
        let v: Vec<&str> = inner.split(',').map(|t| t.trim()).collect();
        (v.len() == 3).then(|| v.join(","))
    };
    let (src, rest) = line.split_once("->")?;
    let (tgt, scale) = rest.split_once(';')?;
    let scale = scale.trim().strip_prefix("scale")?.trim().to_string();
    Some((format!("{}:{}", nums(src)?, nums(tgt)?), scale))
}

/// The custom-block file every water block is made from: the TMX 210218 wood
/// platform block (`!WoodPlatform\PlatRegular\PlatformWoodBase.Block.Gbx`) with
/// its deck moved 200 m down (`mapgeom vstream-shift`); `mapgeom waterblocks`
/// re-points its archetype and renames its ident per pool family.
pub const WATER_TEMPLATE: &[u8] = include_bytes!("../assets/water-template.Block.Gbx");
