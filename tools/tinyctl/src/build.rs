//! `tinyctl build NN…` — the tiny build of one or more campaign maps, end to
//! end and without a shell: the source map from the campaign directory, the
//! packs for its collection, the recipe environment, `mapgeom tiny-library`,
//! `tmmaps tiny`, the library unzipped next to the map for `publish-map`.
//!
//! ```text
//! tinyctl build 20 21 [--src-dir /tmp/summer2026] [--out-root /tmp] [--tag auto]
//!               [--recipe /tmp/tiny3/recipe.env] [--env K=V …] [--bin-dir DIR]
//! ```
//!
//! Output for map NN goes to `<out-root>/tinyNN/<tag>/` (default tag `auto`):
//! lib.zip, placements.tsv, report.tsv, build.log, Summer-NN-Tiny.Map.Gbx,
//! tiny.log, libx/. The packs are the fixed campaign set: the collection's
//! pack (key 660C…) plus the Stadium pack (key B773…); a Stadium map gets the
//! Stadium pack alone. `--env` adds or overrides variables (TINY_FLAG_TWEEN=0 …).

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
    let recipe = PathBuf::from(f("--recipe").unwrap_or_else(|| "/tmp/tiny3/recipe.env".into()));
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
        println!("{nn}: {} ({}) -> {}", src.file_name().unwrap_or_default().to_string_lossy(), collection_name(coll), out.display());
        let t0 = std::time::Instant::now();
        let mut lib = Command::new(&mapgeom);
        lib.args(&paks).arg("tiny-library").arg(&src).arg("--library-out").arg(out.join("lib.zip")).arg("--mapping-out").arg(out.join("placements.tsv")).arg("--report").arg(out.join("report.tsv"));
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
        let tiny_out = out.join(format!("Summer-{nn}-Tiny.Map.Gbx"));
        let mut tiny = Command::new(&tmmaps);
        tiny.arg("tiny").arg(&src).arg("--mapping").arg(out.join("placements.tsv")).arg("--library").arg(out.join("lib.zip")).arg("--out").arg(&tiny_out);
        tiny.envs(env.iter());
        match run(&mut tiny, &out.join("tiny.log")) {
            Ok(text) => {
                for l in text.lines().filter(|l| l.contains("uid:") || l.contains("scaled every") || l.contains("anchor:") || l.contains("deleted")) {
                    println!("  {}", l.trim());
                }
            }
            Err(e) => {
                eprintln!("  {e}");
                failed += 1;
                continue;
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
