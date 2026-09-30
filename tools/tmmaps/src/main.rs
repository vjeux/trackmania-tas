//! `tmmaps` — everything this project does to a TM2020 **map**.
//!
//! One binary, one container implementation, a control behind every operation.
//! Its counterpart is `tools/ghost`, which owns the **ghost / replay** format;
//! the two never overlap. A recording that carries its own map is a `ghost`
//! problem until the map is out of it:
//!
//! ```text
//! ghost map extract R.Replay.Gbx --out m.Map.Gbx
//! tmmaps move m.Map.Gbx --out m2.Map.Gbx --move 2089@1520,300,600
//! ghost map set R.Replay.Gbx R2.Replay.Gbx --map m2.Map.Gbx
//! ```
//!
//! That composition is deliberate. `u02` used to reach into the replay's
//! carried map itself (`u02 movefree`), which meant two implementations of the
//! embedded-map chunk and two of the block movers. `u02` is deleted; see
//! `MAPS.md` §"What happened to u02".
//!
//! Times are printed as **seconds with a decimal** (`16.316`), never as raw
//! milliseconds.

use tmmaps::{census, controls, dropscan, gbx, header, map, rotate, segments, splice, selftest};
use std::path::PathBuf;
use tmmaps::cli::flag;

mod cmd;
use cmd::{inspect, ladder, surgery};

/// A tool whose census is 90 000 lines long will be piped into `head`, and a
/// Rust binary ignores SIGPIPE by default — so the write fails, and the
/// failure surfaces as a panic and a backtrace note on a command that worked.
/// Take the Unix default back. (Declared here rather than pulling in `libc`:
/// this crate has zero dependencies and that is worth keeping.)
fn restore_sigpipe() {
    const SIGPIPE: i32 = 13;
    const SIG_DFL: usize = 0;
    extern "C" {
        fn signal(signum: i32, handler: usize) -> usize;
    }
    unsafe {
        signal(SIGPIPE, SIG_DFL);
    }
}

fn main() {
    restore_sigpipe();
    // A refusal that reaches the user as a Rust panic tells them to run with
    // `RUST_BACKTRACE=1`, which is the wrong instruction: the tool worked, the
    // command was wrong. The library keeps panicking — that is what makes the
    // refusals testable with `catch_unwind` in the suite — but at the CLI
    // boundary a panic prints its message and nothing else.
    std::panic::set_hook(Box::new(|info| {
        let msg = info
            .payload()
            .downcast_ref::<String>()
            .cloned()
            .or_else(|| info.payload().downcast_ref::<&str>().map(|s| s.to_string()))
            .unwrap_or_else(|| "internal error".to_string());
        eprintln!("tmmaps: {}", msg);
        // Exit 3, the same code `die` uses, so a refusal has one code whether
        // it was raised at the CLI boundary or deep in the library. This is
        // safe for the suite, which swaps in its own hook around every
        // `catch_unwind` and therefore never reaches this one.
        std::process::exit(3);
    }));
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
    let args: Vec<String> = std::env::args().collect();
    let cmd = args.get(1).map(|s| s.as_str()).unwrap_or("help");
    // Every MAP-taking subcommand reads `args[2]`, and with the argument
    // missing that is `index out of bounds: the len is 2 but the index is 2` —
    // a panic where a usage line belongs. Say what is missing instead.
    const WANTS_MAP: &[&str] = &[
        "waypoints", "census", "gridinfo", "skins", "fillers", "region", "colors", "phases", "genealogy", "tiny-catalog", "lineup", "shared-cells", "ponds", "tiny", "tiny-batch", "clear", "shift", "segments", "move", "rotate", "ladder",
        "roundtrip",
        "renamecheck", "cporder", "origin", "chunks", "blockrefs", "setuid", "settimes", "lmquality", "ghostchunk", "genealogy-fill", "delblocks", "striplightmap", "itembytes", "mediatracker", "music", "strings",
    ];
    if WANTS_MAP.contains(&cmd) && args.len() < 3 {
        eprintln!("tmmaps {} needs a MAP path.\n\n{}", cmd, USAGE);
        std::process::exit(2);
    }
    match cmd {
        "selftest" => selftest::run(&args),
        "region" => census::cmd_region(&args),
        "tiny-catalog" => tmmaps::tiny::catalog_cmd(&args),
        "lineup" => tmmaps::tiny::lineup_cmd(&args),
        "shared-cells" => tmmaps::tiny::shared_cells_cmd(&args),
        "ponds" => tmmaps::tiny::ponds_cmd(&args),
        "tiny" => tmmaps::tiny::cmd(&args),
        "tiny-batch" => tmmaps::tiny::cmd_batch(&args),
        "clear" => census::cmd_clear(&args),
        "shift" => census::cmd_shift(&args),
        "recdump" => surgery::recdump(&args),
        "untag" => surgery::untag(&args),
        "swapplace" => surgery::swapplace(&args),
        "swaprec" => surgery::swaprec(&args),
        "swapcell" => surgery::swapcell(&args),
        "swapmodel" => surgery::swapmodel(&args),
        "wpdump" => surgery::wpdump(&args),
        "waypoints" => surgery::waypoints(&args),
        // seed-item MAP --out OUT: one template item record into a 0-item map (the
        // tiny writer's clone donor; U10S_113, 2026-09-13)
        // rename-map MAP --out OUT --name NEW: the map's own name (header + body), nothing else
        // set-decoration MAP --out OUT --name NoStadium48x48Day: the map's decoration
        // ident (the mood + the arena) in the body's Common chunk (the Id table's
        // slot 3, `set_decoration`) and the header's copy of it
        // (`set_header_decoration`); nothing else — the header XML's mood attribute
        // is the same mood. The Stadium arena → its NoStadium twin (2026-09-14,
        // Everios96: giant maps "don't fit inside the stadium anymore").
        "set-decoration" => {
            let src = args.get(2).cloned().unwrap_or_else(|| { eprintln!("tmmaps set-decoration MAP --out OUT --name NEW"); std::process::exit(2) });
            let out = tmmaps::cli::flag(&args, "--out").map(String::from).unwrap_or_else(|| { eprintln!("--out OUT"); std::process::exit(2) });
            let new = tmmaps::cli::flag(&args, "--name").map(String::from).unwrap_or_else(|| { eprintln!("--name NEW"); std::process::exit(2) });
            let mut m = tmmaps::map::MapFile::load(std::path::Path::new(&src));
            let old = m.decoration_id.clone();
            if old == new {
                println!("{out}: decoration already {new:?}");
                std::fs::copy(&src, &out).expect("copy");
            } else {
                m.set_decoration(&new);
                let h = m.set_header_decoration(&old, &new);
                m.write_to(std::path::Path::new(&out)).expect("write");
                let back = tmmaps::map::MapFile::load(std::path::Path::new(&out));
                println!("{out}: decoration {old:?} -> {new:?} (header {}; readback {:?})", if h { "rewritten" } else { "NOT FOUND" }, back.decoration_id);
                if back.decoration_id != new || !h {
                    std::process::exit(1);
                }
            }
        }
        "set-size" => {
            // set-size MAP --out OUT --size X,Y,Z: the map grid's three size words in
            // chunk 0x0304301F (the 12 bytes before needUnlock/version/nbBlocks). The
            // giant grid question (2026-09-22): does the engine keep grid blocks whose
            // cells lie past the 48-cell arena when the words say the grid is bigger?
            let src = args.get(2).cloned().unwrap_or_else(|| { eprintln!("tmmaps set-size MAP --out OUT --size X,Y,Z"); std::process::exit(2) });
            let out = tmmaps::cli::flag(&args, "--out").map(String::from).unwrap_or_else(|| { eprintln!("--out OUT"); std::process::exit(2) });
            let size = tmmaps::cli::flag(&args, "--size").map(String::from).unwrap_or_else(|| { eprintln!("--size X,Y,Z"); std::process::exit(2) });
            let v: Vec<u32> = size.split(',').map(|t| t.trim().parse::<u32>().unwrap_or_else(|_| { eprintln!("--size: bad number `{t}`"); std::process::exit(2) })).collect();
            if v.len() != 3 {
                eprintln!("--size wants three numbers");
                std::process::exit(2);
            }
            let mut m = tmmaps::map::MapFile::load(std::path::Path::new(&src));
            let off = m.blocks_count_off - 20;
            let words: Vec<u32> = (0..3).map(|k| u32::from_le_bytes(m.gbx.body[off + 4 * k..off + 4 * k + 4].try_into().unwrap())).collect();
            if words != m.size.iter().map(|x| *x as u32).collect::<Vec<u32>>() {
                eprintln!("{src}: the words at body {off:#x} read {words:?}, the parser has {:?} — layout not as assumed, nothing written", m.size);
                std::process::exit(1);
            }
            let mut bytes = Vec::new();
            for w in &v {
                bytes.extend_from_slice(&w.to_le_bytes());
            }
            m.raw_patches.push((off, bytes));
            m.write_to(std::path::Path::new(&out)).expect("write");
            let back = tmmaps::map::MapFile::load(std::path::Path::new(&out));
            println!("{out}: size {:?} -> {:?} (readback {:?}; {} blocks)", m.size, v, back.size, back.blocks.len());
            if back.size.iter().map(|x| *x as u32).collect::<Vec<u32>>() != v {
                std::process::exit(1);
            }
        }
        "rename-map" => {
            let src = args.get(2).cloned().unwrap_or_else(|| { eprintln!("tmmaps rename-map MAP --out OUT --name NEW"); std::process::exit(2) });
            let out = tmmaps::cli::flag(&args, "--out").map(String::from).unwrap_or_else(|| { eprintln!("--out OUT"); std::process::exit(2) });
            let new = tmmaps::cli::flag(&args, "--name").map(String::from).unwrap_or_else(|| { eprintln!("--name NEW"); std::process::exit(2) });
            let old = tmmaps::header::read(&src).map(|h| h.name).unwrap_or_default();
            let mut m = tmmaps::map::MapFile::load(std::path::Path::new(&src));
            let (h, b) = m.set_map_name(&old, &new);
            m.write_to(std::path::Path::new(&out)).expect("write");
            println!("{out}: {old:?} -> {new:?} ({h} in the header, {b} in the body)");
        }
        "seed-item" => {
            let src = args.get(2).cloned().unwrap_or_else(|| { eprintln!("tmmaps seed-item MAP --out OUT"); std::process::exit(2) });
            let out = tmmaps::cli::flag(&args, "--out").map(String::from).unwrap_or_else(|| { eprintln!("--out OUT"); std::process::exit(2) });
            let g = tmmaps::gbx::Gbx::parse(&std::fs::read(&src).expect("read map"));
            let body = tmmaps::map::seed_item_record(&g.body, 26).unwrap_or_else(|e| { eprintln!("tmmaps seed-item: {e}"); std::process::exit(1) });
            std::fs::write(&out, g.write_body_recompressed(&body)).expect("write");
            println!("{out}: body {} -> {} bytes", g.body.len(), body.len());
        }
        "retag" => {
            // Rewrite an item's waypoint TAG in place (order stays 0): the
            // engine's vocabulary is Spawn / Goal / Checkpoint / StartFinish /
            // LinkedCheckpoint, and a cut written with `Start` / `Finish`
            // spawns the car but never fires its finish.
            let src = PathBuf::from(&args[2]);
            let out = PathBuf::from(flag(&args, "--out").expect("--out F"));
            let spec = flag(&args, "--set").expect("--set iN=TAG,iN=TAG,...");
            let mut m = map::MapFile::load(&src);
            let mut n = 0usize;
            for tok in spec.split(',').map(|s| s.trim()).filter(|s| !s.is_empty()) {
                let (a, tag) = tok.split_once('=').unwrap_or_else(|| panic!("--set: {:?} is not iN=TAG", tok));
                let ii: usize = a
                    .trim_start_matches('i')
                    .parse()
                    .unwrap_or_else(|_| panic!("--set: {:?} is not an item index (want iN)", a));
                assert!(ii < m.items.len(), "item#{} does not exist ({} items)", ii, m.items.len());
                let old = m.items[ii].waypoint_tag.clone();
                m.set_item_waypoint_tag(ii, Some(tag));
                println!("  item#{} {} {:?} -> {:?}", ii, m.items[ii].model, old, tag);
                n += 1;
            }
            assert!(n > 0, "--set named nothing");
            let sp = m.write_to_reporting(&out).expect("write retagged map");
            println!("wrote {} ({} tags rewritten)\n  {}", out.display(), n, sp.summary());
            let back = map::MapFile::load(&out);
            for it in back.items.iter().filter(|it| it.waypoint_tag.is_some()) {
                println!("  read-back: item#{} {} tag {:?}", it.index, it.model, it.waypoint_tag.as_deref().unwrap());
            }
        }
        "dropcp" => {
            // TAKE CHECKPOINTS OUT OF A MAP'S REQUIRED SET. The engine decides a
            // landmark's kind from the ITEM MODEL's waypoint type (a Checkpoint
            // model stays a required checkpoint whatever the placement's tag
            // says -- measured on the MK64 Koopa cut: tags rewritten to Goal
            // still counted as checkpoints and the finish never fired), so the
            // only in-place cure is to swap each checkpoint placement onto a
            // model the engine does not require -- a FINISH model -- and park
            // it where no car can cross it. A finish is never required, so the
            // map's real finish then fires on its own.
            let src = PathBuf::from(&args[2]);
            let out = PathBuf::from(flag(&args, "--out").expect("--out F"));
            let spec = flag(&args, "--items").expect("--items iN,iN,...");
            let model = flag(&args, "--model").expect("--model NAME.Item.Gbx (a Finish-type model the map carries)");
            let park: Vec<f32> = flag(&args, "--park")
                .unwrap_or("8,-900,8")
                .split(',')
                .map(|s| s.trim().parse::<f32>().expect("--park X,Y,Z"))
                .collect();
            assert_eq!(park.len(), 3, "--park wants X,Y,Z");
            let mut m = map::MapFile::load(&src);
            // An embedded model must be one the map carries; a STOCK item ident
            // (no `.Item.Gbx`, e.g. `GateFinish32m`) comes from the game's own
            // collection and needs no manifest row -- measured: a Koopa cut with
            // its finish placement swapped onto GateFinish32m validates a tape
            // at 5.882 (its own finish item: 5.834).
            assert!(
                !model.ends_with(".Item.Gbx") || m.items.iter().any(|it| it.model == model),
                "--model {:?} is not a model this map places; the engine could not load it",
                model
            );
            let mut n = 0usize;
            let mut dropped: Vec<usize> = Vec::new();
            for tok in spec.split(',').map(|s| s.trim()).filter(|s| !s.is_empty()) {
                let ii: usize = tok
                    .trim_start_matches('i')
                    .parse()
                    .unwrap_or_else(|_| panic!("--items: {:?} is not an item index (want iN)", tok));
                assert!(ii < m.items.len(), "item#{} does not exist ({} items)", ii, m.items.len());
                let old = m.items[ii].model.clone();
                let home = m.items[ii].pos;
                m.set_item_model(ii, &model);
                m.move_item_pos(ii, [park[0], park[1], park[2]]);
                println!("  item#{} {} at {:?} -> {} parked at {:?}", ii, old, home, model, park);
                dropped.push(ii);
                n += 1;
            }
            assert!(n > 0, "--items named nothing");
            // Two passes: a model rename re-encodes the Id stream and the
            // variable-length tag splice cannot ride the same write, so the
            // renamed+moved map is written, reloaded, and only then untagged.
            let sp1 = m.write_to_reporting(&out).expect("write map (pass 1: models + positions)");
            println!("  pass 1: {}", sp1.summary());
            let mut m = map::MapFile::load(&out);
            for &ii in &dropped {
                m.set_item_waypoint_tag(ii, None);
            }
            let sp = m.write_to_reporting(&out).expect("write map (pass 2: waypoint properties)");
            println!("wrote {} ({} checkpoints dropped)\n  {}", out.display(), n, sp.summary());
            let back = map::MapFile::load(&out);
            for it in back.items.iter().filter(|it| it.waypoint_tag.is_some()) {
                println!("  read-back: item#{} {} tag {:?} at {:?}", it.index, it.model, it.waypoint_tag.as_deref().unwrap(), it.pos);
            }
        }
        "setmodel" => {
            // Point an item placement at another model (and optionally another
            // author), in place. Diagnostic: swap a map's own finish item for
            // the stock `GateFinish32m` to ask the oracle whether the ITEM or
            // the PLACEMENT is why a finish never fires.
            let src = PathBuf::from(&args[2]);
            let out = PathBuf::from(flag(&args, "--out").expect("--out F"));
            let spec = flag(&args, "--set").expect("--set iN=MODEL,...");
            let author = flag(&args, "--author");
            let mut m = map::MapFile::load(&src);
            for tok in spec.split(',').map(|s| s.trim()).filter(|s| !s.is_empty()) {
                let (a, model) = tok.split_once('=').unwrap_or_else(|| panic!("--set: {:?} is not iN=MODEL", tok));
                let ii: usize = a.trim_start_matches('i').parse().unwrap_or_else(|_| panic!("--set: {:?} is not iN", a));
                let old = m.items[ii].model.clone();
                m.set_item_model(ii, model);
                if let Some(au) = author.as_deref() {
                    m.set_item_author(ii, au);
                }
                println!("  item#{} {} -> {} (author {:?})", ii, old, model, author);
            }
            let sp = m.write_to_reporting(&out).expect("write map");
            println!("wrote {}\n  {}", out.display(), sp.summary());
            let back = map::MapFile::load(&out);
            for it in back.items.iter().filter(|it| it.waypoint_tag.is_some()) {
                println!("  read-back: item#{} {} tag {:?} at {:?}", it.index, it.model, it.waypoint_tag.as_deref().unwrap(), it.pos);
            }
        }
        "setblock" => {
            // Rename a block model in place (diagnostic sibling of `setmodel`):
            // e.g. RoadTechStart -> RoadTechMultilap to ask the oracle what a
            // lap-race start does to a map's finish.
            let src = PathBuf::from(&args[2]);
            let out = PathBuf::from(flag(&args, "--out").expect("--out F"));
            let spec = flag(&args, "--set").expect("--set N=NAME,...");
            let mut m = map::MapFile::load(&src);
            for tok in spec.split(',').map(|s| s.trim()).filter(|s| !s.is_empty()) {
                let (a, name) = tok.split_once('=').unwrap_or_else(|| panic!("--set: {:?} is not N=NAME", tok));
                let bi: usize = a.parse().unwrap_or_else(|_| panic!("--set: {:?} is not a block index", a));
                let old = m.blocks[bi].name.clone();
                m.set_block_name(bi, name);
                println!("  block#{} {} -> {}", bi, old, name);
            }
            let sp = m.write_to_reporting(&out).expect("write map");
            println!("wrote {}\n  {}", out.display(), sp.summary());
        }
        "reembed" => {
            // Replace one embedded item model's BYTES inside the map's
            // embedded-objects zip, keeping the manifest and every placement.
            // `--replace NAME=FILE` names the zip entry (as `header` lists it,
            // e.g. Items/Foo.Item.Gbx) and the file whose bytes go in its
            // place. Used to swap the Koopa cut's StartFinish-type start item
            // for the same item with waypoint type Start, so the map stops
            // being a lap race and its Finish can fire.
            let src = PathBuf::from(&args[2]);
            let out = PathBuf::from(flag(&args, "--out").expect("--out F"));
            let spec = flag(&args, "--replace").expect("--replace NAME=FILE[,NAME=FILE]");
            let mut m = map::MapFile::load(&src);
            let (zip, _names) = header::embedded_zip_bytes(&m.gbx.body).expect("map has no embedded-objects zip");
            let manifest = m.embedded_manifest().expect("map has no readable embedded-objects manifest");
            let mut zip2 = zip.clone();
            for tok in spec.split(',').map(|s| s.trim()).filter(|s| !s.is_empty()) {
                let (name, file) = tok.split_once('=').unwrap_or_else(|| panic!("--replace: {:?} is not NAME=FILE", tok));
                let bytes = std::fs::read(file).unwrap_or_else(|e| panic!("{file}: {e}"));
                let (before, names) = header::embedded_zip(&m.gbx.body).unwrap();
                assert!(names.iter().any(|n| n == name), "{:?} is not an entry of the embedded zip ({} entries, {} bytes)", name, names.len(), before);
                zip2 = header::zip_add(&zip2, name, &bytes);
                println!("  {} <- {} ({} bytes)", name, file, bytes.len());
            }
            let rows: Vec<(&str, &str)> = manifest.iter().map(|(a, b)| (a.as_str(), b.as_str())).collect();
            println!("  manifest: {} item idents kept; zip {} -> {} bytes", rows.len(), zip.len(), zip2.len());
            m.replace_embedded_objects(&rows, &zip2);
            let sp = m.write_to_reporting(&out).expect("write map");
            println!("wrote {}\n  {}", out.display(), sp.summary());
            let back = map::MapFile::load(&out);
            let (zb, zn) = header::embedded_zip(&back.gbx.body).unwrap_or((0, Vec::new()));
            println!("  read-back: zip {} bytes, {} entries, manifest {} rows", zb, zn.len(), back.embedded_manifest().map_or(0, |r| r.len()));
        }
        "pathline" => {
            // A REFERENCE LINE from the exporter's ROM-path CSV
            // (`path,index,section,x,y,z,heading_deg`, world metres): rows of the
            // named path between two indices, then optional appended points,
            // resampled to one row per 10 ms at a constant speed, written as
            // `time_ms,x,y,z` -- the shape `--refcsv` reads. Also prints the
            // arclength of every appended point and of the named gate positions
            // so the legs of a chained gate search can be placed on it.
            let csv = PathBuf::from(&args[2]);
            let out = PathBuf::from(flag(&args, "--out").expect("--out F.csv"));
            let path_name: String = flag(&args, "--path").map(|s| s.to_string()).unwrap_or_else(|| "track_path".to_string());
            let (i0, i1) = {
                let r: String = flag(&args, "--idx").map(|s| s.to_string()).unwrap_or_else(|| "0..".to_string());
                let (a, b) = r.split_once("..").expect("--idx A..B");
                (a.parse::<usize>().unwrap_or(0), b.parse::<usize>().unwrap_or(usize::MAX))
            };
            let speed_ms: f64 = flag(&args, "--speed").map(|s| s.parse().expect("--speed m/s")).unwrap_or(30.0);
            let text = std::fs::read_to_string(&csv).unwrap_or_else(|e| panic!("{}: {e}", csv.display()));
            let mut pts: Vec<[f64; 3]> = Vec::new();
            for line in text.lines().skip(1) {
                let f: Vec<&str> = line.split(',').collect();
                if f.len() < 6 || f[0] != path_name {
                    continue;
                }
                let idx: usize = f[1].parse().unwrap_or(usize::MAX);
                if idx < i0 || idx > i1 {
                    continue;
                }
                pts.push([f[3].parse().unwrap(), f[4].parse().unwrap(), f[5].parse().unwrap()]);
            }
            assert!(pts.len() >= 2, "no points of path {:?} in {:?}", path_name, csv.display());
            let mut appended: Vec<usize> = Vec::new();
            if let Some(ap) = flag(&args, "--append") {
                for p in ap.split(';').map(|s| s.trim()).filter(|s| !s.is_empty()) {
                    let v: Vec<f64> = p.split(',').map(|x| x.trim().parse().expect("--append x,y,z;x,y,z")).collect();
                    assert_eq!(v.len(), 3, "--append wants x,y,z triples");
                    pts.push([v[0], v[1], v[2]]);
                    appended.push(pts.len() - 1);
                }
            }
            // --idx2 A..B: a SECOND stretch of the same path, appended after the
            // --append points (a shortcut leaves the path and rejoins it later:
            // the lap line is [main path to the branch] + [the shortcut] + [main
            // path from the rejoin to the finish]).
            if let Some(r) = flag(&args, "--idx2") {
                let (a, b) = r.split_once("..").expect("--idx2 A..B");
                let (j0, j1) = (a.parse::<usize>().unwrap_or(0), b.parse::<usize>().unwrap_or(usize::MAX));
                for line in text.lines().skip(1) {
                    let f: Vec<&str> = line.split(',').collect();
                    if f.len() < 6 || f[0] != path_name {
                        continue;
                    }
                    let idx: usize = f[1].parse().unwrap_or(usize::MAX);
                    if idx < j0 || idx > j1 {
                        continue;
                    }
                    pts.push([f[3].parse().unwrap(), f[4].parse().unwrap(), f[5].parse().unwrap()]);
                }
            }
            let mut s = vec![0.0f64; pts.len()];
            for i in 1..pts.len() {
                let (a, b) = (pts[i - 1], pts[i]);
                s[i] = s[i - 1] + ((b[0] - a[0]).powi(2) + (b[1] - a[1]).powi(2) + (b[2] - a[2]).powi(2)).sqrt();
            }
            let total = *s.last().unwrap();
            let mut rows = String::from("time_ms,x,y,z\n");
            let mut t_ms: i64 = 0;
            let mut seg = 0usize;
            loop {
                let d = t_ms as f64 / 1000.0 * speed_ms;
                if d > total {
                    break;
                }
                while seg + 1 < s.len() - 1 && s[seg + 1] < d {
                    seg += 1;
                }
                let (a, b) = (pts[seg], pts[seg + 1]);
                let len = (s[seg + 1] - s[seg]).max(1e-9);
                let f = ((d - s[seg]) / len).clamp(0.0, 1.0);
                rows.push_str(&format!(
                    "{},{:.3},{:.3},{:.3}\n",
                    t_ms,
                    a[0] + (b[0] - a[0]) * f,
                    a[1] + (b[1] - a[1]) * f,
                    a[2] + (b[2] - a[2]) * f
                ));
                t_ms += 10;
            }
            std::fs::write(&out, rows).unwrap_or_else(|e| panic!("{}: {e}", out.display()));
            println!(
                "wrote {} : {} points of {} [{}..{}] + {} appended = {:.1} m, {} rows at {} m/s",
                out.display(), pts.len() - appended.len(), path_name, i0, i1.min(pts.len()), appended.len(), total, t_ms / 10, speed_ms
            );
            for &k in &appended {
                println!("  appended ({:.1}, {:.1}, {:.1}) at s {:.1} m  t {:.2} s", pts[k][0], pts[k][1], pts[k][2], s[k], s[k] / speed_ms);
            }
            if let Some(g) = flag(&args, "--gates") {
                for p in g.split(';').map(|s| s.trim()).filter(|s| !s.is_empty()) {
                    let v: Vec<f64> = p.split(',').map(|x| x.trim().parse().expect("--gates x,z;x,z")).collect();
                    let mut best = (f64::INFINITY, 0usize);
                    for (i, q) in pts.iter().enumerate() {
                        let d = ((q[0] - v[0]).powi(2) + (q[2] - v[1]).powi(2)).sqrt();
                        if d < best.0 {
                            best = (d, i);
                        }
                    }
                    let i = best.1;
                    let dir = if i + 1 < pts.len() { [pts[i + 1][0] - pts[i][0], pts[i + 1][2] - pts[i][2]] } else { [pts[i][0] - pts[i - 1][0], pts[i][2] - pts[i - 1][2]] };
                    let n = (dir[0] * dir[0] + dir[1] * dir[1]).sqrt().max(1e-9);
                    println!(
                        "  gate ({:.1}, {:.1}) -> path idx {} at s {:.1} m  t {:.2} s  ({:.1} m off)  dir ({:.4}, {:.4}) heading {:.1} deg  path point ({:.1}, {:.1}, {:.1})",
                        v[0], v[1], i, s[i], s[i] / speed_ms, best.0, dir[0] / n, dir[1] / n, (dir[0]).atan2(dir[1]).to_degrees(), pts[i][0], pts[i][1], pts[i][2]
                    );
                }
            }
        }
        "setlaps" => {
            // The LAP COUNT of a multilap map (chunk 0x03043018: u32 isLapRace,
            // u32 nbLaps), for a certification copy: the validator only says
            // FINISHED after nbLaps crossings of a StartFinish line, and a 3-lap
            // MK64 course cannot be certified one lap at a time otherwise. The
            // header XML's nblaps="" is rewritten too. Geometry untouched.
            let src = PathBuf::from(&args[2]);
            let out = PathBuf::from(flag(&args, "--out").expect("--out F"));
            let laps: u32 = flag(&args, "--laps").expect("--laps N").parse().expect("--laps N");
            let bytes = std::fs::read(&src).unwrap_or_else(|e| panic!("{}: {e}", src.display()));
            let g = gbx::Gbx::parse(&bytes);
            let mut body = g.body.clone();
            // a SKIPPABLE chunk: id, "PIKS", size, payload
            let hits: Vec<usize> = gbx::all_skip_chunks(&body).into_iter().filter(|(cid, _, _, _)| *cid == 0x0304_3018).map(|(_, _, p, _)| p).collect();
            assert!(!hits.is_empty(), "no chunk 0x03043018 in the body");
            let mut patched = 0;
            for h in &hits {
                let o = *h;
                if o + 8 > body.len() {
                    continue;
                }
                let is_lap = u32::from_le_bytes(body[o..o + 4].try_into().unwrap());
                let n = u32::from_le_bytes(body[o + 4..o + 8].try_into().unwrap());
                if is_lap <= 1 && (1..=99).contains(&n) {
                    println!("chunk 0x03043018 at body {:#x}: isLapRace={} nbLaps={} -> {}", h, is_lap, n, laps);
                    body[o + 4..o + 8].copy_from_slice(&laps.to_le_bytes());
                    patched += 1;
                }
            }
            assert_eq!(patched, 1, "expected exactly one plausible 0x03043018 payload, found {patched} of {} id hits", hits.len());
            let mut file = g.write_body_recompressed(&body);
            // header XML: nblaps="N"
            let ud = &g.user_data;
            if let Some(i) = ud.windows(8).position(|w| w == b"nblaps=\"") {
                let start = i + 8;
                let end = start + ud[start..].iter().position(|b| *b == b'"').unwrap();
                let old = String::from_utf8_lossy(&ud[start..end]).to_string();
                let new = laps.to_string();
                if old.len() == new.len() {
                    // same length: patch in place in the written file's user data region
                    let fpos = file.windows(end - i).position(|w| w == &ud[i..end]).expect("xml in file");
                    file[fpos + 8..fpos + 8 + new.len()].copy_from_slice(new.as_bytes());
                    println!("header XML nblaps=\"{old}\" -> \"{new}\"");
                } else {
                    println!("header XML nblaps=\"{old}\" left (length differs; the body chunk is what the validator reads)");
                }
            }
            std::fs::write(&out, &file).unwrap_or_else(|e| panic!("{}: {e}", out.display()));
            let back = gbx::Gbx::parse(&std::fs::read(&out).unwrap());
            assert_eq!(back.body, body, "read-back body differs");
            println!("wrote {}", out.display());
        }
        "gbxcompress" => {
            // Rewrite ANY Gbx file with its body LZO-compressed ('C'). The
            // dedicated server accepts an uncompressed ('U') body, which is
            // what the synthesiser writes -- the GAME CLIENT does not
            // ("Unable to load ghost file"; the plugin's Replay_Load / Ghost_Add
            // drop the handler). Same header, same body bytes, one fresh LZO
            // stream; the round-trip is asserted by the writer.
            let src = PathBuf::from(&args[2]);
            let out = PathBuf::from(&args[3]);
            let bytes = std::fs::read(&src).unwrap_or_else(|e| panic!("{}: {e}", src.display()));
            let was = bytes.get(7).copied().unwrap_or(b'?') as char;
            let g = gbx::Gbx::parse(&bytes);
            let w = g.write_body_recompressed(&g.body);
            std::fs::write(&out, &w).unwrap_or_else(|e| panic!("{}: {e}", out.display()));
            let back = gbx::Gbx::parse(&w);
            assert_eq!(back.body, g.body, "read-back body differs");
            println!("wrote {} : body {} -> C, {} body bytes, file {} -> {} bytes", out.display(), was, g.body.len(), bytes.len(), w.len());
        }
        "segat" => segments::cmd_segat(&args),
        "segments" => inspect::segment_table(&args),
        "ladder" => ladder::ladder(&args),
        // ---- w612: write ONE map with several grid blocks moved.
        "rotate" => rotate::cmd(&args),
        "move" => ladder::move_blocks(&args),
        "blockprobe" => ladder::blockprobe(&args),
        "addblock" => ladder::addblock(&args),
        "recdump" => ladder::recdump(&args),
        "oracle" => ladder::oracle_stage(&args),
        "roundtrip" => controls::cmd_roundtrip(&args),
        "bodydiff" => splice::cmd_bodydiff(&args),
        "rewrite" => splice::cmd_rewrite(&args),
        "renamecheck" => surgery::renamecheck(&args),
        "cporder" => ladder::cporder(&args),
        "rungspec" => ladder::rungspec(&args),
        "origin" => controls::cmd_origin(&args),
        // `tmmaps setuid MAP --out F [--uid U]`: the map with a fresh (or the
        // given) UID. The game caches a map's computed lightmap and its
        // embedded items by UID: a rebuilt test copy under the published UID
        // plays with the OLD build's baked light (Summer 09's start deck read
        // dark in play mode until this, 2026-09-07).
        "stripghost" => surgery::stripghost(&args),
        "validate" => surgery::validate(&args),
        "setuid" => surgery::setuid(&args),
        "settimes" => surgery::settimes(&args),
        "sinkitems" => surgery::sinkitems(&args),
        "straight" => cmd::straight::straight(&args),
        "wpprobe" => cmd::straight::wpprobe(&args),
        "lmquality" => surgery::lmquality(&args),
        "ghostchunk" => surgery::ghostchunk(&args),
        "genealogy-fill" => surgery::genealogy_fill(&args),
        // `tmmaps delblocks MAP --out F [--keep-baked Sea,…] [--strip-lightmap]`: every
        // authored block deleted (the generated ones too, but for --keep-baked),
        // items kept — the 0-block form of `tmmaps tiny` on ANY map, for the
        // lightmapper-crash bisect of 2026-09-07
        "delblocks" => surgery::delblocks(&args),
        // `tmmaps striplightmap MAP --out F`: the ORIGINAL map with its stored
        // lightmap dropped (HasLightmaps = 0), blocks and items untouched — the
        // 2026-09-09 probe of whether the tiny maps' blown-out light spots are the
        // absent lightmap rather than the scaled lights
        "striplightmap" => surgery::striplightmap(&args),
        "itembytes" => surgery::itembytes(&args),
        "dropbaked" => surgery::dropbaked(&args),
        "movebaked" => surgery::movebaked(&args),
        "census" => census::cmd_census(&args),
        "skins" => census::cmd_skins(&args),
        "fillers" => tmmaps::fillers::cmd(&args),
        // archive-only variants of a tiny map for the load-failure bisect (zippad.rs)
        "zippad" => tmmaps::zippad::cmd(&args),
        "header" => header::cmd(&args),
        "dropscan" => dropscan::cmd(&args),
        "mediatracker" => tmmaps::mediatracker::cmd(&args),
        "strings" => {
            // tmmaps strings MAP [--grep S] [--min N]: length-prefixed strings of the
            // decompressed body (and the header), with their offsets
            let path = std::path::Path::new(&args[2]);
            let m = tmmaps::map::MapFile::load(path);
            let want = tmmaps::cli::flag(&args, "--grep").map(String::from);
            let min: usize = tmmaps::cli::flag(&args, "--min").and_then(|s| s.parse().ok()).unwrap_or(4);
            let scan = |label: &str, b: &[u8]| {
                let mut i = 0usize;
                while i + 4 <= b.len() {
                    let n = u32::from_le_bytes(b[i..i + 4].try_into().unwrap()) as usize;
                    if n >= min && n <= 512 && i + 4 + n <= b.len() {
                        let s = &b[i + 4..i + 4 + n];
                        if s.iter().all(|c| (*c >= 0x20 && *c != 0x7f) || *c == b'\t') {
                            if let Ok(t) = std::str::from_utf8(s) {
                                if want.as_deref().map(|w| t.contains(w)).unwrap_or(true) {
                                    println!("{label} {i}: {t:?}");
                                }
                                i += 4 + n;
                                continue;
                            }
                        }
                    }
                    i += 1;
                }
            };
            scan("header", &m.gbx.user_data);
            scan("body", &m.gbx.body);
        }
        "music" => {
            // tmmaps music MAP [--set URL [--path P] --out F]: the custom music FileRef
            let path = std::path::Path::new(&args[2]);
            let mut m = tmmaps::map::MapFile::load(path);
            match m.custom_music() {
                None => println!("{}: no CustomMusicPackDesc chunk", path.display()),
                Some((fr, span)) => println!("{}: music v{} path {:?} url {:?} (body {}..{})", path.display(), fr.version, fr.path, fr.url, span.0, span.1),
            }
            if let Some(url) = tmmaps::cli::flag(&args, "--set") {
                let out = tmmaps::cli::flag(&args, "--out").unwrap_or_else(|| tmmaps::cli::die("--set wants --out F"));
                let p = tmmaps::cli::flag(&args, "--path").map(String::from).unwrap_or_else(|| format!("Skins\\Any\\Music\\{}", url.rsplit('/').next().unwrap_or("music.ogg")));
                m.set_custom_music(&p, url).unwrap_or_else(|e| tmmaps::cli::die(&e));
                m.write_to(std::path::Path::new(out)).expect("write");
                println!("  wrote {out}");
            }
        }
        "chunks" => inspect::chunks(&args),
        "blockrefs" => inspect::blockrefs(&args),
        "genealogy" => inspect::genealogy(&args),
        "gridinfo" => inspect::gridinfo(&args),
        "genealogy-cells" => inspect::genealogy_cells(&args),
        "colors" => inspect::colors(&args),
        "phases" => inspect::phases(&args),
        "help" | "--help" | "-h" => println!("{}", USAGE),
        other => {
            eprintln!("tmmaps: unknown subcommand `{}`\n\n{}", other, USAGE);
            std::process::exit(2);
        }
    }
}

const USAGE: &str = r#"tmmaps — TM2020 map surgery. `tools/ghost` owns ghosts and replays.

Times print as seconds with a decimal (16.316), never as raw milliseconds.

READING A MAP
  tmmaps waypoints MAP
        the map's waypoints: spawn, checkpoints, goal — block# / item# indices,
        tags, cells, free positions. These indices are what every mover takes.
  tmmaps census MAP [--filter PAT] [--free]
  tmmaps skins MAP [--name SUBSTR]      item placements carrying a skin FileRef (path, url) + per-model counts
        EVERY block, unbaked (0x0304301F) and BAKED (0x03043048), tagged U/B,
        with its free-block position when it has one, as TSV. `waypoints` and
        any single-chunk listing show only one of the two: across the store
        52.7 % of blocks are baked, and eleven maps read as near-empty without
        this (197047 showed 3 blocks of 2316). A baked index is counted in its
        OWN list, so a bare `2461` pasted from a census row addresses an
        unrelated unbaked block — movers spell baked indices `bN` and REFUSE
        them rather than moving the wrong block.
  tmmaps fillers MAP [--filter PAT] [--cells X0,Z0:X1,Z1] [--summary]
        the generated clip fillers (the game's own pillar walls, skirts, end
        caps: baked records) with their variant word (a0 Middle .. a4 nothing,
        g1 TopBottom_Ground), the side they stand on, what their own cell holds
        (`-` free, `P:` pillars only) and what stands across the side;
        --summary tallies free / pillar / occupied cells per name and variant
  tmmaps dropbaked SRC --out MAP [--baked b12,b40,…] [--name PAT[,PAT…]] [--flags HEX]
        the ORIGINAL minus exact generated records (by bN index, by name
        substring and/or an exact flags word) — the ground-truth probe: shoot
        SRC and MAP from one camera; no changed pixel ⇒ the engine draws
        nothing for those records
  tmmaps region MAP --box X0,Y0,Z0:X1,Y1,Z1 [--filter PAT] [--items] [--blocks]
        everything whose position lies inside a world box. A GATE IS A
        STRUCTURE, NOT A BLOCK: run this before and after any move.
  tmmaps phases MAP [--filter PAT] [--all]
        the per-item ANIMATION PHASE OFFSET (chunk 0x03043063, one byte per item
        in eighths of the period: 4 = half) — what the editor stores when the
        author phases a pusher/rotor/tube; non-zero items, or --filter/--all
  tmmaps chunks MAP [--only 0xCHUNK --hex N]
        every skippable body chunk with its size (--only/--hex: one chunk, head dump)
  tmmaps blockrefs MAP [--groups]
        every place the file lists blocks by index or per block (the snapped-on
        tables, free-block entries, colour/lightmap bytes, macroblock refs) —
        the audit behind `remove_blocks`
  tmmaps header MAP [MAP ...] [--tsv] [--xml] [--names]
        what the file DECLARES about itself before any block is read: container
        version, node count, EXTERNAL references, the header chunk table, the
        community XML (title, exever/exebuild, envir, maptype, validated), the
        declared `<dep>` files, the embedded-objects zip and its entries, and
        the block/item counts to set them against. `--tsv` is the corpus form —
        one row per map plus the distinct value of every column, because a
        difference only one map has is a lead and one several share is not.
        Written for 146612, which the engine loads and the editor will not open.
        `--names` is the IDENTITY form — `path uid name authorid authortime`,
        one row per map — for auditing what this repo publishes a map as
        against what the map calls itself. Join it on uid against
        trackmania.io; our own documents are not an independent check.

TINY MAPS (half-scale campaign: every authored block/item -> an embedded static item)
  tmmaps tiny MAP --mapping placements.tsv --library ITEMS.zip --out F [--scale 0.5]
      [--anchor x,y,z | --anchor fit] [--host HOST.Map.Gbx] [--keep-ghost] [--name NAME | --keep-name] [--keep-zone-block]
      [--uid-prefix Tin2] [--name-prefix "Tiny "]
        --scale 2 with --anchor fit, --uid-prefix Gia2 and --name-prefix "Giant " is the GIANT
        (double-size) build: the transformed extent is centred in the map grid by whole cells,
        an overflow above the grid's top row is reported (TINY.md "Giant maps")
        replace every authored block by its library item (mapping rows
        `@index<TAB>ITEM|-<TAB>model_scale<TAB>sx<TAB>sz`; `-` = intentionally
        nothing) and re-point/drop items (`i@index<TAB>ITEM|stock model|-`);
        the baked foundation stays. Inputs come from `mapgeom tiny-library`.
  tmmaps tiny-batch DIR --out DIR [--mapgeom BIN --paks "--pak F:HASH …"] [same flags]
                                                    every .Map.Gbx of a directory; with --mapgeom each map gets
                                                    its own item library (OUT/NAME.lib.zip, .placements.tsv, .report.tsv)
  tmmaps lineup MAP --out F --stock A,B,C --at X,Y,Z [--pitch 16] [--yaw R] [--items F.Item.Gbx,G.Item.Gbx] [--colors 2,3,…]
        the map plus a row of stock (pack) items by name — a vegetation species survey;
        --items continues the row with embedded item files (a test item next to its stock oracle);
        --colors gives each item of the row its placement colour byte (a stock flag at Green next to ours)
  tmmaps shared-cells MAP [--all] [--mapping placements.tsv] [--trace LAP.csv --anchor …]
        cells where a terrain tile shares its cell with another block, and whether the
        tile is hidden by that block in the tiny map (kept = a coplanar pair to watch)
  tmmaps ponds MAP [--trace LAP.csv --anchor sx,sy,sz:tx,ty,tz [--scale 0.5]]
        the enclosed sea cells (Sea records not connected to the open sea) that get a
        half-size sea floor in the tiny map, with the blocks standing in them; --trace (a
        tmtraj export --csv of a tiny lap) lists the pond cells the car crosses
  tmmaps tiny-catalog MAP --mapping T --library Z --out F [--only NAME] [--lineup A,B]
        one block per model beside its items (or the listed item files), for a look

CHANGING A MAP — position and ROTATION; no model swap, so no trigger volume changes
  tmmaps move MAP --out F --move SPEC [--move SPEC ...]
        SPEC is  N:cx,cy,cz[:dir]   a grid block, by world cell
                 N@x,y,z[/yaw]      a FREE block, in metres
                 iN@x,y,z[/yaw]     an item
                 bN                 a baked index — always refused, by name
        --cell is correct for either placement regime; --pos/@ is metres and is
        the only correct form for a free block. A free block ignores its cell
        bytes (its position is six f32 in chunk 0x0304305F), so a regime-blind
        cell write produces a map that loads, an origin control that passes,
        and a ladder in which every rung is silent.
  tmmaps rotate MAP --out F --rot BLK:yaw,pitch,roll [...]
                          --drot BLK:dyaw,dpitch,droll [...]
                          --tilt N,N,N --about X,Y,Z --dir DEG --angle RAD
        Tilt FREE blocks. A block's stored rotation turns it about ITS OWN
        ANCHOR, so giving every tile of a surface the same roll SHEARS it into a
        staircase (32 m tiles at 3.4 deg = a 1.9 m step per join, measured). The
        `--tilt` form is the honest one: one axis, position and rotation written
        together. REFUSES when a free block within --group-radius (4 m) is not in
        the rotation -- the ice kicker of 284238 is FOUR blocks sharing an anchor
        and two arms rotated one of them, which reads exactly like a null result.
  tmmaps shift MAP --out F --box X0,Y0,Z0:X1,Y1,Z1 --by DX,DY,DZ [--filter PAT]
        DISPLACE everything in the box, then re-read the written map and require
        each object to be exactly where it was sent. This is how you measure a
        clearance: move an obstacle by known amounts and ask the oracle which
        is the first that lets a tape through. A structure half-moved reads as
        a measurement, so nothing is half-moved.
  tmmaps clear MAP --out F --box X0,Y0,Z0:X1,Y1,Z1 --to X,Y,Z [--filter PAT]
        move EVERYTHING in the box, then re-read the written map and REQUIRE
        the box to be empty. This is the enforced form of the lesson below.
  tmmaps segat MAP --out F --promote W [--neutralise W,W,...] [--force-rename]
        ONE segment map, spelled out: W becomes the finish and each --neutralise
        waypoint stops being a checkpoint. Nothing is inferred and nothing is
        verified -- the caller owns the comparison. This is the only way to cut
        at a LINKED checkpoint (`segments` cannot enumerate one). Rules: every
        checkpoint at or after the cut must be neutralised, AND every other
        member of the cut's own linked group; a linked group EARLIER than the
        cut is left alone.
  tmmaps segments MAP --ref-ghost G [--out DIR] [--order W,W,...] [-j N]
        measure the checkpoint order, then build every segment map + a control.
        The order is MEASURED against the reference ghost and every round is
        checked against the ghost's own declared splits: the tool REFUSES, with
        the probe table, rather than reporting an order it cannot establish.
        Every built segment is re-validated against the ghost and must
        reproduce that checkpoint's declared split — exactly for a gate cut,
        early by <= 0.500 s for the block-rename fallback — and no two segment
        maps may share a decompressed body. Exit 2 on any refusal.
        --order gives the driving order instead of measuring it, as waypoint
        indices in driving order (`--order 439,494,440,633,492`); spell it
        `i439` / `b2089` when block and item indices collide.

MEASURING WITH A MAP
  tmmaps dropscan MAP --out DIR --tape GHOST (--cells CX:CX:S,CY,CZ:CZ:S | --cell
                  CX,CY,CZ[,DIR] | --tapes DIR) [--dirs D,..] [--target X,Y,Z]
                  [--jobs N] [--at frac:F,..] [--fk PATH] [--server DIR] [--keep]
        READ THE ENVIRONMENT'S GEOMETRY WITH THE CAR. Moves the spawn to a cell,
        drives the car off it under a fixed straight-throttle tape, and reads
        the landing out of the live engine (`fk trace`): one probe is one map
        plus one trace, and the summary is apex, resting place, closest approach
        to --target, and when. This is how you ask "what is over there" on a map
        whose surfaces belong to the DECORATION and not to the map file.
        --tapes DIR instead scores a POPULATION OF TAPES on the untouched map,
        with the same readout — the poor man's state objective.
        Controls, both refusals: the spawn moved to its OWN cell must reproduce
        the untouched map byte-for-byte, and a probe at the real spawn cell must
        start where the map's spawn is. A trace that is all zeroes is REFUSED:
        it passes fk's self-check and means the cell produced no car at all
        (every cell outside the map grid does).
  tmmaps ladder MAP --spec F --ghosts G... [-j N]
        arrival-time ladders. One rung per spec line, whitespace-separated
        moves, so a rung is a CURTAIN of gates across the corridor rather than
        one 32 m cell: a single-cell rung is silent for roughly a third of
        well-chosen placements. Aborts unless a rebuild at every gate's own
        origin reproduces the untouched map, and asserts N rungs produced N
        distinct file hashes.
  tmmaps rungspec TRAJ.csv --block N --from A --to B --step S [--offset dx,dy,dz]
        emit a ladder spec placing a gate ON a reference trajectory
  tmmaps oracle --map M --ghosts G... [--map M2 --ghosts ...] [--shard] [-j N]
        validate (map, ghosts) batches; one server per map, as required —
        every segment map keeps the original mapUid, so two of them can never
        share a UserData/Maps. --shard: one map, ghosts split over -j servers.
  tmmaps cporder MAP TRAJ.csv --splits A,B,C
        which waypoint produced which declared split, from one trajectory
        decode; reports match distance and runner-up so a bad match is visible

CONTROLS — run these before you trust a map-surgery result
  tmmaps selftest [--engine] [--strict]
        the whole suite in one command. --engine adds the dedicated server.
  tmmaps bodydiff STOCK EDITED
        what an edit changed, on the DECOMPRESSED bodies, attributed to the
        placement each differing byte belongs to — plus how much of the file
        itself the two share. A SPLICED edit differs in the bytes of the edit
        and nowhere else; a re-emitted map shares the header and then nothing.
  tmmaps rewrite MAP --out F [--reemit]
        write a map back with NO edit. The default output is the stock file
        byte for byte; --reemit rebuilds the compressed stream, which is what
        every map this project wrote before the splice path existed. The pair
        isolates the WRITER from the EDIT — the one question the dedicated
        server cannot be asked, because it accepts both.
  tmmaps roundtrip MAP
        parse and re-emit unchanged; compares DECOMPRESSED bodies (LZO is not
        bit-reproducible, so file hashes are the wrong level)
  tmmaps origin MAP
        return-to-origin at the byte level: every waypoint AND every item
        through its own mover with its own current placement, output required
        byte-identical. No oracle calls. This is what catches a mover writing
        dead bytes.
  tmmaps renamecheck MAP [--ghosts G...]
        the RENAME round-trip, which roundtrip cannot be: renames a waypoint
        four ways — including to ITSELF with byte-identical output required —
        forcing the two-region Id stream through the rename re-encoder.
        --ghosts also renames an off-route decoration and requires the control
        ghosts' times to be UNCHANGED. Required gate for any rename change.
        A `lookback table length N -> N+1 ... may not resolve` warning is
        CONSERVATIVE: it fires on maps whose game check then passes. Do not
        read it as a failure, and do not read it as an all-clear.

LESSONS THE CODE ENFORCES — see MAPS.md for the full list
  A GATE IS A STRUCTURE, NOT A BLOCK.  Moving GothMommy's added finish on
  173691 moved one unbaked anchor and left FIFTEEN baked `GateExpandable*`
  pieces standing in the landing zone; the run drove into them and stopped, and
  a human spotted it in the video before any instrument did. `region` counts
  what is there and `clear` refuses to succeed while anything is left.

  NEVER PROBE BY SWAPPING A GATE MODEL.  The old `gate` / `gateat` / `probe`
  commands relocated a waypoint by swapping its item model to `GateFinish32m`
  first. On 285885 that quadruples the trigger volume (the origin control then
  returns 50.589 instead of 61.229 — it fabricates discoveries); on 279197 it
  deletes a custom Goal item and everything DNFs. Those commands are deleted.
  Every mover here is position-only. `segments` still promotes a gate, because
  a promoted gate is a fine RULER — it is an unsafe OBJECTIVE.

  A MAP-SURGERY CONTROL CAN BE INERT.  Gate-removed, deck-removed and
  road-removed maps once all returned identical output; the road control proved
  the instrument was dead, not the maps identical. `ladder` requires N distinct
  file hashes for N rungs, and `oracle` refuses a candidate the server would
  silently skip (see below).

  THE SERVER IGNORES A FILE WITHOUT A `.Ghost.Gbx` / `.Replay.Gbx` SUFFIX and
  returns a plain DNF, indistinguishable from a run that did not finish. The
  oracle driver refuses such a path instead of staging it.

env: TMMAPS_DEBUG=1 (lookback table sizes)
     TMMAPS_DEBUG_NODES=1 (trace body node refs as INLINE / backref)
     TMMAPS_NO_BAKED=1 — A LANDMINE. Safe ONLY for position-only surgery: with
     it set the baked chunk is not parsed, so its Id words are not renumbered
     and any RENAME silently mis-encodes the map — and every baked block
     disappears from the census. Nothing has needed it since the
     shared-node-index fix.

exit: 0 ok · 1 a control failed · 2 usage · 3 refused (the command was wrong,
      and the message says what to do instead — never a backtrace)
"#;

