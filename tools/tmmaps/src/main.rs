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

use tmmaps::{census, controls, dropscan, header, rotate, segments, splice, selftest};

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
        "waypoints", "census", "skins", "fillers", "region", "colors", "phases", "genealogy", "tiny-catalog", "lineup", "shared-cells", "tiny", "tiny-batch", "clear", "shift", "segments", "move", "rotate", "ladder",
        "roundtrip",
        "renamecheck", "cporder", "origin", "chunks", "blockrefs", "setuid", "lmquality", "delblocks", "striplightmap", "itembytes", "mediatracker",
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
        "tiny" => tmmaps::tiny::cmd(&args),
        "surface" => tmmaps::surface::cmd(&args),
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
        // tmmaps set-exebuild MAP --out OUT --build 2026-02-02_17_51 [--exever 3.3.0]
        // tmmaps embedded MAP — the embedded-objects manifest rows and zip entries
        "embedded" => {
            let m = tmmaps::map::MapFile::load(std::path::Path::new(&args[2]));
            match m.embedded_objects() {
                Some((rows, zip)) => {
                    println!("{} manifest rows, zip {} bytes", rows.len(), zip.len());
                    for (ident, author) in &rows {
                        println!("  row  {ident}\t{author}");
                    }
                    // --extract DIR: the zip entries written out (A5, the community wood blocks)
                    let extract = tmmaps::cli::flag(&args, "--extract").map(std::path::PathBuf::from);
                    for (name, bytes) in tmmaps::header::zip_entries(&zip) {
                        println!("  zip  {name}\t{} B", bytes.len());
                        if let Some(d) = &extract {
                            let flat = name.rsplit(|c| c == '/' || c == '\\').next().unwrap_or(&name).to_string();
                            std::fs::create_dir_all(d).expect("mkdir");
                            std::fs::write(d.join(flat), &bytes).expect("write");
                        }
                    }
                }
                None => println!("no embedded objects"),
            }
        }
        // tmmaps physfp MAP [--mapgeom BIN --paks "--pak F:HASH"] — a PHYSICS FINGERPRINT of the map:
        // every authored block record as `name|flags|cell|rot` plus every embedded item's hull
        // physics histogram (via mapgeom dump) keyed by the item's placements — two maps with
        // the same fingerprint drive the same. Lines are sorted; diff two outputs to see what
        // changed (Everios96 2026-09-20 Q5: "did the physics change on the never-wiped maps?").
        // the hunters' change log of the altered sets (GeraldDuke 2026-09-22): rows from the
        // box's changelog.sh → README.md + one markdown file per alteration
        // the Altered Nadeo Wood / Wet Wood recipes learned from AR's campaigns (json9841 2026-09-22)
        "altered-learn" => {
            if let Err(e) = tmmaps::altered::learn_cmd(&args[1..]) {
                eprintln!("tmmaps altered-learn: {e}");
                std::process::exit(1);
            }
        }
        "stadium-chunkdump" => {
            if let Err(e) = tmmaps::stadium::chunkdump_cmd(&args[1..]) {
                eprintln!("tmmaps stadium-chunkdump: {e}");
                std::process::exit(1);
            }
        }
        "stadium-strip" => {
            if let Err(e) = tmmaps::stadium::strip_cmd(&args[1..]) {
                eprintln!("tmmaps stadium-strip: {e}");
                std::process::exit(1);
            }
        }
        "stadium-terrain" => {
            if let Err(e) = tmmaps::stadium::terrain_cmd(&args[1..]) {
                eprintln!("tmmaps stadium-terrain: {e}");
                std::process::exit(1);
            }
        }
        "stadium-slopedir" => {
            if let Err(e) = tmmaps::stadium::slopedir_cmd(&args[1..]) {
                eprintln!("tmmaps stadium-slopedir: {e}");
                std::process::exit(1);
            }
        }
        "stadium-verify" => {
            if let Err(e) = tmmaps::stadium::verify_cmd(&args[1..]) {
                eprintln!("tmmaps stadium-verify: {e}");
                std::process::exit(1);
            }
        }
        "stadium-info" => {
            if let Err(e) = tmmaps::stadium::info_cmd(&args[1..]) {
                eprintln!("tmmaps stadium-info: {e}");
                std::process::exit(1);
            }
        }
        "stadium-scan" => {
            if let Err(e) = tmmaps::stadium::scan_cmd(&args[1..]) {
                eprintln!("tmmaps stadium-scan: {e}");
                std::process::exit(1);
            }
        }
        "stadium-port" => {
            if let Err(e) = tmmaps::stadium::port_cmd(&args[1..]) {
                eprintln!("tmmaps stadium-port: {e}");
                std::process::exit(1);
            }
        }
        "altered-route" => {
            if let Err(e) = tmmaps::altered::route_cmd(&args[1..]) {
                eprintln!("tmmaps altered-route: {e}");
                std::process::exit(1);
            }
        }
        "altered-recollect" => {
            if let Err(e) = tmmaps::altered::recollect_cmd(&args[1..]) {
                eprintln!("tmmaps altered-recollect: {e}");
                std::process::exit(1);
            }
        }
        "altered-addblock" => {
            if let Err(e) = tmmaps::altered::addblock_cmd(&args[1..]) {
                eprintln!("tmmaps altered-addblock: {e}");
                std::process::exit(1);
            }
        }
        "altered-unzip" => {
            if let Err(e) = tmmaps::altered::unzip_cmd(&args[1..]) {
                eprintln!("tmmaps altered-unzip: {e}");
                std::process::exit(1);
            }
        }
        "altered-itemdump" => {
            if let Err(e) = tmmaps::altered::itemdump_cmd(&args[1..]) {
                eprintln!("tmmaps altered-itemdump: {e}");
                std::process::exit(1);
            }
        }
        "altered-apply" => {
            if let Err(e) = tmmaps::altered::apply_cmd(&args[1..]) {
                eprintln!("tmmaps altered-apply: {e}");
                std::process::exit(1);
            }
        }
        "changelog" => {
            if let Err(e) = tmmaps::changelog::cmd(&args[1..]) {
                eprintln!("tmmaps changelog: {e}");
                std::process::exit(1);
            }
        }
        "physfp" => {
            let src = std::path::PathBuf::from(&args[2]);
            let m = tmmaps::map::MapFile::load(&src);
            let mut lines: Vec<String> = Vec::new();
            for b in &m.blocks {
                let c = b.coords();
                lines.push(format!("B\t{}\t{:08x}\t{},{},{}\t{}", b.name, b.flags, c.0, c.1, c.2, b.dir));
            }
            // embedded items: hull histogram per model, then the placements of that model
            let mapgeom = tmmaps::cli::flag(&args, "--mapgeom").map(|s| s.to_string());
            let paks: Vec<String> = tmmaps::cli::flag(&args, "--paks").map(|s| s.split_whitespace().map(|x| x.to_string()).collect()).unwrap_or_default();
            let mut hulls: std::collections::BTreeMap<String, String> = std::collections::BTreeMap::new();
            if let (Some(mg), Some((_, zip))) = (&mapgeom, m.embedded_objects()) {
                let dir = std::env::temp_dir().join(format!("physfp-{}", std::process::id()));
                std::fs::create_dir_all(&dir).expect("tmp");
                for (name, bytes) in tmmaps::header::zip_entries(&zip) {
                    let flat = name.rsplit(|c| c == '/' || c == '\\').next().unwrap_or(&name).to_string();
                    if !flat.to_ascii_lowercase().ends_with(".item.gbx") && !flat.to_ascii_lowercase().ends_with(".block.gbx") { continue; }
                    let f = dir.join(&flat);
                    std::fs::write(&f, &bytes).expect("write");
                    let out = std::process::Command::new(mg).args(&paks).arg("dump").arg(&f).output().map(|o| String::from_utf8_lossy(&o.stdout).into_owned()).unwrap_or_default();
                    let ids: Vec<String> = out.lines().filter(|l| l.contains("CPlugSurface")).map(|l| l.split("ids ").nth(1).unwrap_or("").trim().to_string()).collect();
                    hulls.insert(flat.to_ascii_lowercase(), if ids.is_empty() { "nohull".into() } else { ids.join(" | ") });
                }
                let _ = std::fs::remove_dir_all(&dir);
            }
            for it in &m.items {
                let key = it.model.rsplit(|c| c == '/' || c == '\\').next().unwrap_or(&it.model).to_ascii_lowercase();
                let hull = hulls.get(&key).cloned().unwrap_or_else(|| "stock".into());
                lines.push(format!("I\t{}\t{:.3},{:.3},{:.3}\t{:.3}\t{}", if hull == "stock" { it.model.clone() } else { "embedded".into() }, it.pos[0], it.pos[1], it.pos[2], it.yaw, hull));
            }
            lines.sort();
            for l in lines { println!("{l}"); }
        }
        // tmmaps replace-embedded MAP --file A.Item.Gbx[,B.Item.Gbx…] --out OUT — the embedded
        // entries of the same base name swapped for these files (manifest kept): the giant
        // cypress items re-baked with a valid hull physics (2026-09-20), in place
        "replace-embedded" => {
            let src = std::path::PathBuf::from(&args[2]);
            let files: Vec<std::path::PathBuf> = tmmaps::cli::flag(&args, "--file").expect("replace-embedded needs --file A.Item.Gbx[,…]").split(',').map(std::path::PathBuf::from).collect();
            let out = std::path::PathBuf::from(tmmaps::cli::flag(&args, "--out").expect("replace-embedded needs --out OUT"));
            let mut m = tmmaps::map::MapFile::load(&src);
            let (_, zip) = m.embedded_objects().expect("map has no embedded objects");
            let mut kept: std::collections::BTreeMap<String, Vec<u8>> = std::collections::BTreeMap::new();
            for (name, bytes) in tmmaps::header::zip_entries(&zip) {
                kept.insert(name, bytes);
            }
            let mut swapped = 0usize;
            for f in &files {
                let base = f.file_name().unwrap().to_string_lossy().to_string();
                let bytes = std::fs::read(f).unwrap_or_else(|e| { eprintln!("{}: {e}", f.display()); std::process::exit(1) });
                let key = kept.keys().find(|k| k.rsplit(|c| c == '/' || c == '\\').next().unwrap_or(k) == base).cloned();
                match key {
                    Some(k) => { let old = kept.insert(k.clone(), bytes.clone()).map(|v| v.len()).unwrap_or(0); println!("  {k}: {old} -> {} bytes", bytes.len()); swapped += 1; }
                    None => { eprintln!("{base}: not among the embedded entries"); std::process::exit(1); }
                }
            }
            let zip = tmmaps::header::deflated_zip(&kept);
            m.replace_embedded_zip_keep_manifest(&zip);
            m.write_to(&out).expect("write");
            println!("{}: {swapped} embedded file(s) replaced, zip {} bytes", out.display(), zip.len());
        }
        // tmmaps set-thumbnail MAP --jpeg X.jpg --out OUT — the header thumbnail replaced
        "set-thumbnail" => {
            let src = std::path::PathBuf::from(&args[2]);
            let jpeg = std::fs::read(tmmaps::cli::flag(&args, "--jpeg").expect("set-thumbnail needs --jpeg X.jpg")).expect("read jpeg");
            let out = std::path::PathBuf::from(tmmaps::cli::flag(&args, "--out").expect("set-thumbnail needs --out OUT"));
            let mut m = tmmaps::map::MapFile::load(&src);
            let old = m.set_thumbnail(&jpeg).unwrap_or_else(|e| { eprintln!("{e}"); std::process::exit(1) });
            m.write_to(&out).expect("write");
            println!("{}: thumbnail {} -> {} bytes", out.display(), old, jpeg.len());
        }
        // tmmaps chunk MAP 0x0304xxxx — the raw payload of one body chunk (hex + text)
        "chunk" => {
            let m = tmmaps::map::MapFile::load(std::path::Path::new(&args[2]));
            let want = u32::from_str_radix(args.get(3).map(|s| s.trim_start_matches("0x")).expect("chunk id"), 16).expect("hex chunk id");
            for (cid, _, payload, size) in tmmaps::gbx::all_skip_chunks(&m.gbx.body) {
                if cid == want {
                    let d = &m.gbx.body[payload..payload + size];
                    println!("chunk 0x{cid:08X} payload {payload} size {size}");
                    for (i, row) in d.chunks(16).enumerate() {
                        let hex: Vec<String> = row.iter().map(|b| format!("{b:02x}")).collect();
                        let txt: String = row.iter().map(|b| if (0x20..0x7f).contains(b) { *b as char } else { '.' }).collect();
                        println!("  {:04x}  {:<48}  {txt}", i * 16, hex.join(" "));
                    }
                }
            }
        }
        // tmmaps thumbnail MAP --out X.jpg — the header thumbnail's JPEG bytes as stored
        // (the game stores them vertically FLIPPED; flip for viewing, e.g. ffmpeg -vf vflip)
        "thumbnail" => {
            let src = std::path::PathBuf::from(&args[2]);
            let out = std::path::PathBuf::from(tmmaps::cli::flag(&args, "--out").expect("--out X.jpg"));
            let bytes = std::fs::read(&src).expect("read map");
            let start = bytes.windows(15).position(|w| w == b"<Thumbnail.jpg>").expect("no <Thumbnail.jpg> in header") + 15;
            let end = start + bytes[start..].windows(16).position(|w| w == b"</Thumbnail.jpg>").expect("no </Thumbnail.jpg>");
            std::fs::write(&out, &bytes[start..end]).expect("write jpeg");
            println!("{}: {} bytes", out.display(), end - start);
        }
        // tmmaps body FILE.gbx --out F — any Gbx file's DECOMPRESSED body (items, maps, …)
        "body" => {
            let data = std::fs::read(&args[2]).expect("read");
            let g = tmmaps::gbx::Gbx::parse(&data);
            let out = tmmaps::cli::flag(&args, "--out").expect("--out F");
            std::fs::write(out, &g.body).expect("write");
            println!("{out}: {} bytes (class 0x{:08X}, {} nodes)", g.body.len(), g.class_id, g.num_nodes);
        }
        // tmmaps item-physics MAP --model SUBSTR --physics N --out OUT
        // The surface-physics byte of EVERY CPlugMaterialUserInst inside one embedded item,
        // patched in place (one byte per material node: no copy, no size change) and the
        // archive rebuilt around the patched item. U10S_975's roulette wheel: one material
        // node, physics 0x4D Plastic by its author (`TM_ItemSpectator_plastic_physics`);
        // the copy-based conversion made the map 9.8 MB (Everios96, 2026-09-15).
        "item-physics" => {
            let src = std::path::PathBuf::from(&args[2]);
            let model = tmmaps::cli::flag(&args, "--model").expect("--model SUBSTR").to_lowercase();
            let phys: u8 = tmmaps::cli::flag(&args, "--physics").and_then(|v| v.parse().ok()).expect("--physics N");
            let out = std::path::PathBuf::from(tmmaps::cli::flag(&args, "--out").expect("--out OUT"));
            let mut m = tmmaps::map::MapFile::load(&src);
            let (rows, zip) = m.embedded_objects().expect("no embedded objects");
            let mut files: std::collections::BTreeMap<String, Vec<u8>> = tmmaps::header::zip_entries(&zip).into_iter().collect();
            let mut patched_files = 0;
            let mut patched_nodes = 0;
            let names: Vec<String> = files.keys().filter(|k| k.to_lowercase().contains(&model)).cloned().collect();
            for name in &names {
                let bytes = files[name].clone();
                let g = tmmaps::gbx::Gbx::parse(&bytes);
                let mut body = g.body.clone();
                // every 0x090FD000 chunk: u32 version, u8 isUsingGameMaterial (v>=11), Id materialName,
                // Id model, string baseTexture, u8 surfacePhysicId, u8 surfaceGameplayId (v>=10), string link …
                let marker = 0x090F_D000u32.to_le_bytes();
                let mut at = 0usize;
                let mut hits = Vec::new();
                while let Some(p) = body[at..].windows(4).position(|w| w == marker) {
                    let s = at + p;
                    at = s + 4;
                    // the chunk id is preceded by the node's class id (same value) when it opens the node
                    let mut r = tmmaps::gbx::Reader::new(&body[s + 4..]);
                    let ver = r.u32();
                    if !(9..=12).contains(&ver) {
                        continue;
                    }
                    if ver >= 11 {
                        r.u8();
                    }
                    // an Id word: 0xffffffff = none; flags 0x40000000/0x80000000 with index 0 =
                    // a new string follows (u32 length + bytes); with an index = a back-reference
                    let mut read_id = |r: &mut tmmaps::gbx::Reader| {
                        let w = r.u32();
                        if w != 0xffff_ffff && w & 0xC000_0000 != 0 && w & 0x3fff_ffff == 0 {
                            let n = r.u32() as usize;
                            r.bytes(n);
                        }
                    };
                    read_id(&mut r); // materialName
                    read_id(&mut r); // model
                    let n = r.u32() as usize; // baseTexture
                    r.bytes(n);
                    let phys_at = s + 4 + r.o;
                    let old = body[phys_at];
                    hits.push((phys_at, old));
                }
                for (pa, old) in &hits {
                    body[*pa] = phys;
                    println!("{name}: material node physics {old} -> {phys} at body+{pa}");
                    patched_nodes += 1;
                }
                if !hits.is_empty() {
                    let rewritten = g.write_body_recompressed(&body);
                    files.insert(name.clone(), rewritten);
                    patched_files += 1;
                }
            }
            if patched_files == 0 {
                eprintln!("no embedded file matching `{model}` with a material node (files: {})", files.keys().cloned().collect::<Vec<_>>().join(", "));
                std::process::exit(1);
            }
            let new_zip = tmmaps::header::deflated_zip(&files);
            let manifest: Vec<(&str, &str)> = rows.iter().map(|(a, b)| (a.as_str(), b.as_str())).collect();
            m.replace_embedded_objects(&manifest, &new_zip);
            m.write_to(&out).expect("write");
            println!("{}: {patched_nodes} material nodes in {patched_files} file(s) -> physics {phys}; archive {} -> {} bytes", out.display(), zip.len(), new_zip.len());
        }
        // tmmaps reuid MAP --out OUT --prefix Pla3 — a NEW map uid (= a fresh leaderboard):
        // the first 4 characters of the uid are replaced (Pla2… → Pla3…), header and body.
        // Everios96's records reset, option (b), 2026-09-15.
        // tmmaps set-name MAP --out OUT --name NEW [--from OLD] — the map's display name
        // (header + body) rewritten; OLD defaults to the header's current name. The
        // round-6 rebuild (2026-09-15) shipped 1,852 maps as "Wet Wood U10S_31" instead
        // of "U10S_31 By Everios96 [Wet Wood]" (the build ran without --name-style
        // everios); Everios96 noticed 2026-09-18.
        // tmmaps repoint-items MAP --from IDENT --to NAME --out OUT — every placement of the
        // embedded model IDENT re-pointed at the stock model NAME (author Nadeo), the embedded
        // file dropped from the archive. Giant 2026-09-19 (uelen: "all of those cypress-like
        // trees crash the game"): the ×2-baked CypressTall copies → the game's own CypressTall.
        "repoint-items" => {
            let src = std::path::PathBuf::from(&args[2]);
            let out = std::path::PathBuf::from(tmmaps::cli::flag(&args, "--out").expect("--out OUT"));
            let from = tmmaps::cli::flag(&args, "--from").expect("--from IDENT");
            let to = tmmaps::cli::flag(&args, "--to").expect("--to NAME");
            let mut m = tmmaps::map::MapFile::load(&src);
            let mut n = 0usize;
            for i in 0..m.items.len() {
                let model = m.items[i].model.clone();
                if model == from {
                    m.set_item_model(i, &to);
                    m.set_item_author(i, "Nadeo");
                    n += 1;
                }
            }
            // renames first (a re-encoded write), then the archive splice on the reloaded map
            let tmp = out.with_extension("repoint.tmp");
            m.write_to(&tmp).expect("write");
            let mut m = tmmaps::map::MapFile::load(&tmp);
            let mut dropped = false;
            if let Some((rows, zip)) = m.embedded_objects() {
                let keep: Vec<(String, String)> = rows.iter().filter(|(ident, _)| *ident != from).cloned().collect();
                if keep.len() != rows.len() {
                    let entries: std::collections::BTreeMap<String, Vec<u8>> = tmmaps::header::zip_entries(&zip).into_iter().filter(|(name, _)| {
                        let flat = name.rsplit(|c| c == '/' || c == '\\').next().unwrap_or(name).to_string();
                        flat != from
                    }).collect();
                    let newzip = tmmaps::header::stored_zip(&entries);
                    let keep_refs: Vec<(&str, &str)> = keep.iter().map(|(a, b)| (a.as_str(), b.as_str())).collect();
                    m.replace_embedded_objects(&keep_refs, &newzip);
                    dropped = true;
                }
            }
            m.write_to(&out).expect("write");
            let _ = std::fs::remove_file(&tmp);
            println!("{}: {n} placements of {from} -> {to}{}", out.display(), if dropped { ", embedded file dropped" } else { "" });
        }
        "set-name" => {
            let src = std::path::PathBuf::from(&args[2]);
            let out = std::path::PathBuf::from(tmmaps::cli::flag(&args, "--out").expect("--out OUT"));
            let new = tmmaps::cli::flag(&args, "--name").expect("--name NEW");
            let mut m = tmmaps::map::MapFile::load(&src);
            let hdr = tmmaps::header::read(&src.display().to_string()).expect("header");
            let old = tmmaps::cli::flag(&args, "--from").map(|s| s.to_string()).unwrap_or(hdr.name.clone());
            if old == new {
                println!("{}: name already `{new}`", src.display());
                std::fs::copy(&src, &out).expect("copy");
            } else {
                let (h, b) = m.set_map_name(&old, &new);
                assert!(h + b > 0, "the map does not declare the name `{old}`");
                m.write_to(&out).expect("write");
                println!("{}: name `{old}` -> `{new}` ({h} header, {b} body)", out.display());
            }
        }
        "reuid" => {
            let src = std::path::PathBuf::from(&args[2]);
            let out = std::path::PathBuf::from(tmmaps::cli::flag(&args, "--out").expect("--out OUT"));
            let prefix = tmmaps::cli::flag(&args, "--prefix").expect("--prefix XXXX");
            assert!(prefix.len() == 4, "--prefix is 4 characters");
            let mut m = tmmaps::map::MapFile::load(&src);
            let old = m.body_ids.first().and_then(|f| f.name.clone()).expect("map uid Id");
            let new = format!("{prefix}{}", &old[4..]);
            m.set_map_uid(&new);
            m.write_to(&out).expect("write");
            println!("{}: uid {old} -> {new}", out.display());
        }
        // tmmaps carry-embedded MAP --source SRC --out OUT — the SOURCE map's own embedded
        // objects (custom items/blocks) merged into MAP's archive + manifest when missing.
        // The archive bug the giant pipeline still has (Giant 953: 109 models placed, 104
        // archive rows — Everios96 2026-09-15 11:10).
        "carry-embedded" => {
            let src_p = std::path::PathBuf::from(&args[2]);
            let source = tmmaps::map::MapFile::load(std::path::Path::new(tmmaps::cli::flag(&args, "--source").expect("--source SRC")));
            let out = std::path::PathBuf::from(tmmaps::cli::flag(&args, "--out").expect("--out OUT"));
            let mut m = tmmaps::map::MapFile::load(&src_p);
            let (rows, zip) = m.embedded_objects().unwrap_or_default();
            let mut files: std::collections::BTreeMap<String, Vec<u8>> = tmmaps::header::zip_entries(&zip).into_iter().collect();
            let mut manifest: Vec<(String, String)> = rows.clone();
            let (mut carried, mut added_rows) = (0usize, 0usize);
            if let Some((srows, szip)) = source.embedded_objects() {
                for (name, bytes) in tmmaps::header::zip_entries(&szip) {
                    if !files.contains_key(&name) {
                        files.insert(name, bytes);
                        carried += 1;
                    }
                }
                for (ident, author) in srows {
                    if !manifest.iter().any(|(i, _)| *i == ident) {
                        manifest.push((ident, author));
                        added_rows += 1;
                    }
                }
            }
            if carried == 0 && added_rows == 0 {
                println!("{}: nothing to carry (archive already complete)", src_p.display());
                std::process::exit(3);
            }
            let new_zip = tmmaps::header::deflated_zip(&files);
            let mref: Vec<(&str, &str)> = manifest.iter().map(|(a, b)| (a.as_str(), b.as_str())).collect();
            m.replace_embedded_objects(&mref, &new_zip);
            m.write_to(&out).expect("write");
            println!("{}: {carried} files + {added_rows} manifest rows carried from the source; archive {} -> {} bytes", out.display(), zip.len(), new_zip.len());
        }
        "set-exebuild" => {
            let src = std::path::PathBuf::from(&args[2]);
            let out = std::path::PathBuf::from(tmmaps::cli::flag(&args, "--out").expect("set-exebuild needs --out OUT"));
            let build = tmmaps::cli::flag(&args, "--build").expect("set-exebuild needs --build YYYY-MM-DD_HH_MM");
            let exever = tmmaps::cli::flag(&args, "--exever");
            let mut m = tmmaps::map::MapFile::load(&src);
            let hit = m.set_exebuild(build, exever);
            // --record "date=… git=… GameVersion=…": the BODY build record too (the physics key)
            let rec = match tmmaps::cli::flag(&args, "--record") {
                Some(r) => Some(m.set_build_record(r).unwrap_or_else(|e| { eprintln!("{e}"); std::process::exit(1) })),
                None => None,
            };
            m.write_to(&out).expect("write");
            println!("{}: exebuild {} -> {build}{}{}", out.display(), if hit { "rewritten" } else { "NOT FOUND" }, exever.map(|v| format!(", exever {v}")).unwrap_or_default(), rec.map(|(at, old)| format!("; body record at {at}: `{old}` -> `{}`", tmmaps::cli::flag(&args, "--record").unwrap_or(""))).unwrap_or_default());
            if !hit {
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
            // --uid UID: the map uid too (same length as the current one, or it is refused)
            if let Some(uid) = tmmaps::cli::flag(&args, "--uid") {
                m.write_to(std::path::Path::new(&out)).expect("write");
                let mut m2 = tmmaps::map::MapFile::load(std::path::Path::new(&out));
                m2.set_map_uid(uid);
                m2.write_to(std::path::Path::new(&out)).expect("write");
                println!("{out}: {old:?} -> {new:?} ({h} in the header, {b} in the body); uid {uid}");
            } else {
                m.write_to(std::path::Path::new(&out)).expect("write");
                println!("{out}: {old:?} -> {new:?} ({h} in the header, {b} in the body)");
            }
        }
        "seed-item" => {
            let src = args.get(2).cloned().unwrap_or_else(|| { eprintln!("tmmaps seed-item MAP --out OUT"); std::process::exit(2) });
            let out = tmmaps::cli::flag(&args, "--out").map(String::from).unwrap_or_else(|| { eprintln!("--out OUT"); std::process::exit(2) });
            let g = tmmaps::gbx::Gbx::parse(&std::fs::read(&src).expect("read map"));
            let body = tmmaps::map::seed_item_record(&g.body, 26).unwrap_or_else(|e| { eprintln!("tmmaps seed-item: {e}"); std::process::exit(1) });
            std::fs::write(&out, g.write_body_recompressed(&body)).expect("write");
            println!("{out}: body {} -> {} bytes", g.body.len(), body.len());
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
        "lmquality" => surgery::lmquality(&args),
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
        "chunks" => inspect::chunks(&args),
        "blockrefs" => inspect::blockrefs(&args),
        "genealogy" => inspect::genealogy(&args),
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
      [--anchor x,y,z] [--host HOST.Map.Gbx] [--keep-ghost] [--name NAME | --keep-name] [--keep-zone-block]
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
  tmmaps shared-cells MAP [--all] [--mapping placements.tsv]
        cells where a terrain tile shares its cell with another block, and whether the
        tile is hidden by that block in the tiny map (kept = a coplanar pair to watch)
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

