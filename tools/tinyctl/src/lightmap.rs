//! `tinyctl lightmap MAP… (--out OUT.Map.Gbx | --out-dir DIR) [--quality Q] [--name NAME] [--resaved [--keep-uid|--uid U]] [--allow-unbusy]`
//! — the editor's lightmap for a tiny build, computed on the render box and
//! TRANSPLANTED into the input file (the devserver half of `shootctl lightmap`).
//!
//! Why a transplant and not the editor's own save (2026-09-12, the map 11
//! "shadow seam"): a tiny map ships with the SOURCE's stale lightmap, which the
//! game rejects (0 authored blocks), so play mode bakes a coarse per-item
//! lightmap at every load — big flat items come out as lighter/darker
//! rectangles with straight edges (the player report on 11's first-corner
//! ramp), undersides black. The editor can compute a real lightmap for the
//! tiny layout, but its `SaveMap` rewrites the file: it DROPS every embedded
//! .dds (the trees' colour/alpha atlases, LightColor — 46 files on 11), drops
//! the validation ghost, re-mints the uid and re-serialises the items. So the
//! default output is the INPUT file with only chunk 0x0304305B (the lightmap)
//! replaced by the editor's — verified on 11: the game accepts it in play (the
//! item order and count are unchanged, and the zone tiles the editor saw are
//! the ones the game regenerates at load), the ramp is uniform, the textures
//! and the ghost stay. `--resaved` keeps the old behaviour (the editor's file,
//! with `--keep-uid` / `--uid`).
//!
//! The bake must actually RUN: `shootctl lightmap` reports "(never saw the
//! editor busy)" when `/shadows?q=` did not start a compute (the editor skips
//! a quality it already has; q=2 and q=5 came back with the load-time lightmap
//! of the editor session, byte-identical) — that output is refused unless
//! `--allow-unbusy`. `--name` is the map name the saved file carries (SaveMap
//! renames the map to the file's stem); default: the input's own name. The
//! box-side file lands in `Maps/_lightmap/<name>.Map.Gbx`.
use std::path::{Path, PathBuf};
use std::time::Duration;

use crate::wsx::Wsx;

const STAGE: &str = "/home/vjeux/shoot/_stage";
const SHOTS: &str = "/mnt/c/Users/vjeux/tinyshots";
const BOX_TOOLS: &str = "/home/vjeux/trackmania-tas/tools/target/release";
const LIGHTMAP_CHUNK: u32 = 0x0304_305B;

pub fn cmd(args: &[String]) -> Result<(), String> {
    let f = |k: &str| tmmaps::cli::flag(args, k).map(|s| s.to_string());
    // every positional that is not a flag value is a map
    let flag_with_value = ["--out", "--out-dir", "--quality", "--name", "--uid", "--box-shootctl", "--wsx"];
    let mut maps: Vec<PathBuf> = Vec::new();
    let mut i = 0;
    while i < args.len() {
        let a = &args[i];
        if flag_with_value.contains(&a.as_str()) {
            i += 2;
            continue;
        }
        if a.starts_with("--") || a == "-v" {
            i += 1;
            continue;
        }
        maps.push(PathBuf::from(a));
        i += 1;
    }
    if maps.is_empty() {
        return Err("lightmap needs MAP.Map.Gbx (one or more)".into());
    }
    let out_dir = f("--out-dir").map(PathBuf::from);
    let out_one = f("--out").map(PathBuf::from);
    if maps.len() > 1 && out_dir.is_none() {
        return Err("several maps need --out-dir DIR".into());
    }
    if out_one.is_none() && out_dir.is_none() {
        return Err("lightmap needs --out OUT.Map.Gbx or --out-dir DIR".into());
    }
    if let Some(d) = &out_dir {
        std::fs::create_dir_all(d).map_err(|e| format!("{}: {e}", d.display()))?;
    }
    let mut failures = Vec::new();
    for map in &maps {
        let out = match (&out_one, &out_dir) {
            (Some(o), None) => o.clone(),
            (_, Some(d)) => d.join(map.file_name().ok_or("map path has no file name")?),
            _ => unreachable!(),
        };
        match one(args, map, &out) {
            Ok(()) => {}
            Err(e) => {
                eprintln!("lightmap {}: {e}", map.display());
                failures.push(format!("{}: {e}", map.display()));
            }
        }
    }
    if failures.is_empty() {
        Ok(())
    } else {
        Err(format!("{} of {} maps failed:\n  {}", failures.len(), maps.len(), failures.join("\n  ")))
    }
}

fn one(args: &[String], map: &Path, out: &Path) -> Result<(), String> {
    let f = |k: &str| tmmaps::cli::flag(args, k).map(|s| s.to_string());
    let quality: u32 = f("--quality").and_then(|q| q.parse().ok()).unwrap_or(3);
    let resaved = tmmaps::cli::has(args, "--resaved");
    if !map.exists() {
        return Err(format!("{}: no such file", map.display()));
    }
    let name = match f("--name") {
        Some(n) => n,
        None => {
            let h = tmmaps::header::read(map.to_str().unwrap_or_default())?;
            if h.name.is_empty() { "Tiny".to_string() } else { h.name }
        }
    };
    // the stem is the map name; keep it file-system clean
    let stem: String = name.chars().map(|c| if c == '/' || c == '\\' || c == ':' { '-' } else { c }).collect();
    let tag = format!("lm{}-{}", std::process::id(), stem.chars().filter(|c| c.is_ascii_alphanumeric()).collect::<String>());
    let shootctl = f("--box-shootctl").unwrap_or_else(|| format!("{BOX_TOOLS}/shootctl"));
    let wsx = Wsx::new(args);
    let r_map = format!("{STAGE}/{tag}.Map.Gbx");
    let r_dir = format!("{SHOTS}/{tag}");
    let rel = format!("_lightmap/{stem}.Map.Gbx");
    // The game CACHES a computed lightmap by map uid: a second bake of the same uid at
    // the same or a lower quality is skipped by the editor (the q=2/q=5 "never busy"
    // runs of 2026-09-12). The copy that goes to the box carries a fresh uid; the
    // transplant lands in the input, whose uid is untouched.
    let bake_copy = out.with_extension("bakecopy.Map.Gbx");
    {
        let mut m = tmmaps::map::MapFile::load(map);
        let nanos = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.subsec_nanos()).unwrap_or(0);
        let fresh = format!("Tlm1{:08X}{:07}{:08X}", nanos % 100_000_000, std::process::id() % 10_000_000, (nanos / 7) % 100_000_000);
        m.set_map_uid(&fresh);
        m.write_to(&bake_copy).map_err(|e| format!("{}: {e}", bake_copy.display()))?;
        // The stale lightmap STAYS in the copy: without any lightmap the editor leaves to
        // the menu right after the compute (ctx 0, nothing saved — twice on 11, 2026-09-12);
        // the skip-if-cached problem is handled by the fresh uid plus the game-cache drop.
        eprintln!("bake copy {} with a fresh uid {fresh} (the game caches lightmaps by uid)", bake_copy.display());
    }
    eprintln!("pushing {} to the box …", bake_copy.display());
    wsx.push(&bake_copy, &r_map)?;
    // The game also caches computed lightmaps by CONTENT in
    // C:\ProgramData\Trackmania\Cache\<hash>_<hash>_<Collection>_<mood>.Bump.LightMap.zip and
    // serves a cached one instead of computing (a fresh uid does not help — 2026-09-12,
    // 11 with different quality bytes came back "never busy" at q=4). Drop them first.
    let dropped = wsx.sh("ls /mnt/c/ProgramData/Trackmania/Cache/ | grep -c LightMap.zip; rm -f /mnt/c/ProgramData/Trackmania/Cache/*.LightMap.zip").unwrap_or_default();
    eprintln!("game lightmap cache: {} entries dropped", dropped.trim());
    let cmd = format!("{shootctl} lightmap --detach --map {r_map} --out '{rel}' --quality {quality} --outdir {r_dir}");
    eprintln!("computing the lightmap (quality {quality}) …");
    let started = wsx.sh(&cmd)?;
    if wsx.verbose {
        eprintln!("{}", started.trim());
    }
    let done = wsx.wait_done(&format!("{r_dir}/done.txt"), &format!("{r_dir}/lightmap.log"), Duration::from_secs(3600), "lightmap")?;
    let line = done.lines().next().unwrap_or("").to_string();
    if !line.starts_with("OK") {
        return Err(format!("lightmap: {line}"));
    }
    // did the bake run? the box log says when the editor never went busy
    let log = wsx.cat(&format!("{r_dir}/lightmap.log")).unwrap_or_default();
    let shadows_line = log.lines().find(|l| l.contains("shadows done")).unwrap_or("").trim().to_string();
    if shadows_line.contains("never saw the editor busy") && !tmmaps::cli::has(args, "--allow-unbusy") {
        return Err(format!("the lightmapper never ran on the box ({shadows_line}); the editor skips a quality it already holds — try another --quality, or --allow-unbusy to take the file anyway"));
    }
    eprintln!("{shadows_line}");
    let saved = line.split('\t').nth(1).ok_or("lightmap: no path in the done file")?.to_string();
    let resaved_path: PathBuf = if resaved { out.to_path_buf() } else { out.with_extension("resaved.Map.Gbx") };
    let n = wsx.pull(&saved, &resaved_path)?;
    eprintln!("pulled {} ({n} bytes)", resaved_path.display());

    if resaved {
        // --keep-uid: the editor's re-save mints a new uid; the input's (or --uid U)
        // goes back in, so a publish is an UPDATE of the existing record
        let want_uid = match f("--uid") {
            Some(u) => Some(u),
            None if tmmaps::cli::has(args, "--keep-uid") => Some(tmmaps::header::read(map.to_str().unwrap_or_default())?.uid),
            None => None,
        };
        if let Some(u) = want_uid {
            let mut m = tmmaps::map::MapFile::load(out);
            m.set_map_uid(&u);
            m.write_to(out).map_err(|e| format!("{}: {e}", out.display()))?;
            eprintln!("uid set back to {u}");
        }
        report(out, "the editor's re-save")?;
        return Ok(());
    }

    // the transplant: the input file with only the lightmap chunk replaced
    let orig = tmmaps::map::MapFile::load(map);
    let re = tmmaps::map::MapFile::load(&resaved_path);
    // the lightmap is applied by object index: the two files must list the same items
    let (n_orig, n_re) = (orig.items.len(), re.items.len());
    if n_orig != n_re {
        return Err(format!("item count changed across the editor's save ({n_orig} -> {n_re}); the lightmap would be misaligned — not transplanted (the re-save is at {})", resaved_path.display()));
    }
    let find = |body: &[u8]| tmmaps::gbx::all_skip_chunks(body).into_iter().find(|c| c.0 == LIGHTMAP_CHUNK);
    let ca = find(&orig.gbx.body).ok_or_else(|| format!("{}: no lightmap chunk 0x{LIGHTMAP_CHUNK:08X} to replace", map.display()))?;
    let cb = find(&re.gbx.body).ok_or_else(|| format!("{}: the editor's save has no lightmap chunk", resaved_path.display()))?;
    let mut body = Vec::with_capacity(orig.gbx.body.len() + cb.3);
    body.extend_from_slice(&orig.gbx.body[..ca.1]);
    body.extend_from_slice(&re.gbx.body[cb.1..cb.2 + cb.3]);
    body.extend_from_slice(&orig.gbx.body[ca.2 + ca.3..]);
    // the body goes back LZO-compressed, the shipped form (an uncompressed body is ~1 MB bigger — the Nadeo cap)
    std::fs::write(out, orig.gbx.write_body_recompressed(&body)).map_err(|e| format!("{}: {e}", out.display()))?;
    eprintln!("lightmap chunk {} -> {} bytes, transplanted into a copy of the input (textures, ghost, header, uid untouched)", ca.3, cb.3);
    let _ = std::fs::remove_file(&resaved_path);
    let _ = std::fs::remove_file(&bake_copy);
    report(out, "the transplant")?;
    Ok(())
}

fn report(out: &Path, what: &str) -> Result<(), String> {
    let m = tmmaps::map::MapFile::load(out);
    let lm = tmmaps::map::skip_chunks(&m.gbx.body).into_iter().find(|(id, ..)| *id == LIGHTMAP_CHUNK).map(|(_, _, _, size)| size).unwrap_or(0);
    let h = tmmaps::header::read(out.to_str().unwrap_or_default())?;
    let size = std::fs::metadata(out).map(|m| m.len()).unwrap_or(0);
    println!(
        "wrote {} ({what}) — {size} bytes, map name {:?}, uid {}, lightmap chunk {lm} bytes, {} items, {} blocks, {} embedded files",
        out.display(),
        h.name,
        h.uid,
        m.items.len(),
        m.blocks.len(),
        tmmaps::header::embedded_zip(&m.gbx.body).map(|(_, names)| names.len()).unwrap_or(0)
    );
    Ok(())
}
