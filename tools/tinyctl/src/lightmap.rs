//! `tinyctl lightmap MAP --out OUT.Map.Gbx [--quality Q] [--name NAME] [--keep-uid|--uid U]` — the
//! editor's lightmap for a tiny build, computed on the render box and the
//! re-saved map pulled back (the devserver half of `shootctl lightmap`).
//!
//! MAP should be a build with ONE authored block kept (`tmmaps tiny … --keep-zone-block`; lightmap
//! stripped): a 0-block build crashes the lightmapper. `--name` is the map
//! name the saved file carries (SaveMap renames the map to the file's stem);
//! default: the source map's name with a `Tiny ` prefix, as `tmmaps tiny`
//! writes it. The box-side file lands in `Maps/_lightmap/<name>.Map.Gbx`.
use std::path::PathBuf;
use std::time::Duration;

use crate::wsx::Wsx;

const STAGE: &str = "/home/vjeux/shoot/_stage";
const SHOTS: &str = "/mnt/c/Users/vjeux/tinyshots";
const BOX_TOOLS: &str = "/home/vjeux/trackmania-tas/tools/target/release";

pub fn cmd(args: &[String]) -> Result<(), String> {
    let map = PathBuf::from(args.first().filter(|a| !a.starts_with("--")).ok_or("lightmap needs MAP.Map.Gbx")?);
    let f = |k: &str| tmmaps::cli::flag(args, k).map(|s| s.to_string());
    let out = PathBuf::from(f("--out").ok_or("lightmap needs --out OUT.Map.Gbx")?);
    let quality: u32 = f("--quality").and_then(|q| q.parse().ok()).unwrap_or(2);
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
    let tag = format!("lm{}", std::process::id());
    let shootctl = f("--box-shootctl").unwrap_or_else(|| format!("{BOX_TOOLS}/shootctl"));
    let wsx = Wsx::new(args);
    let r_map = format!("{STAGE}/{tag}.Map.Gbx");
    let r_dir = format!("{SHOTS}/{tag}");
    let rel = format!("_lightmap/{stem}.Map.Gbx");
    eprintln!("pushing {} to the box …", map.display());
    wsx.push(&map, &r_map)?;
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
    let saved = line.split('\t').nth(1).ok_or("lightmap: no path in the done file")?.to_string();
    let n = wsx.pull(&saved, &out)?;
    eprintln!("pulled {} ({n} bytes)", out.display());
    // --keep-uid: the editor's re-save mints a new uid; the input's (or --uid U)
    // goes back in, so a publish is an UPDATE of the existing record
    let want_uid = match f("--uid") {
        Some(u) => Some(u),
        None if tmmaps::cli::has(args, "--keep-uid") => Some(tmmaps::header::read(map.to_str().unwrap_or_default())?.uid),
        None => None,
    };
    if let Some(u) = want_uid {
        let mut m = tmmaps::map::MapFile::load(&out);
        m.set_map_uid(&u);
        m.write_to(&out).map_err(|e| format!("{}: {e}", out.display()))?;
        eprintln!("uid set back to {u}");
    }
    // what the editor made of it
    let m = tmmaps::map::MapFile::load(&out);
    let lm = tmmaps::map::skip_chunks(&m.gbx.body).into_iter().find(|(id, ..)| *id == 0x0304_305B).map(|(_, _, _, size)| size).unwrap_or(0);
    let h = tmmaps::header::read(out.to_str().unwrap_or_default())?;
    println!("wrote {} — map name {:?}, uid {}, lightmap chunk {} bytes, {} items, {} blocks", out.display(), h.name, h.uid, lm, m.items.len(), m.blocks.len());
    Ok(())
}
