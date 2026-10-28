//! `tinyctl tile-icons --tiles-root DIR --surfaces Dirt,Grass,… --views VIEWS.tsv --spec SPEC.txt
//! --host HOST.Map.Gbx [--work DIR] [--frac 0.62] [--margin 0.02]` — the inventory icons of a
//! water-tile set, surface by surface: the icon map (every tile once, `SPEC.txt` = the
//! `tmmaps add-items` spec written for one surface's folder, re-pointed per surface) is built on
//! the host map, pushed to the box, shot (`tinyctl shoot`, one oblique view per tile from
//! `VIEWS.tsv`), the captures cropped (`icon-crops`) and embedded (`mapgeom item-icon`) into
//! `DIR/<Surface>/{Pool,Channel}/*.Item.Gbx` in place. The 2026-10-28 one-off driver, as a
//! command (vjeux: drivers are Rust, not shell).
use std::path::{Path, PathBuf};
use std::process::Command;

fn run(cmd: &mut Command, what: &str) -> Result<String, String> {
    let out = cmd.output().map_err(|e| format!("{what}: {e}"))?;
    let text = format!("{}{}", String::from_utf8_lossy(&out.stdout), String::from_utf8_lossy(&out.stderr));
    if !out.status.success() {
        return Err(format!("{what}: exit {:?}\n{}", out.status.code(), text.lines().rev().take(8).collect::<Vec<_>>().join("\n")));
    }
    Ok(text)
}

pub fn cmd(args: &[String]) -> Result<(), String> {
    let f = |k: &str| tmmaps::cli::flag(args, k).map(|s| s.to_string());
    let root = PathBuf::from(f("--tiles-root").ok_or("--tiles-root DIR (holds <Surface>/Pool and <Surface>/Channel)")?);
    let surfaces: Vec<String> = f("--surfaces").ok_or("--surfaces A,B,…")?.split(',').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect();
    let views = f("--views").ok_or("--views VIEWS.tsv")?;
    let spec_path = f("--spec").ok_or("--spec SPEC.txt (an add-items spec for one surface folder)")?;
    let host = f("--host").ok_or("--host HOST.Map.Gbx")?;
    let work = PathBuf::from(f("--work").unwrap_or_else(|| "/tmp/ww/pool/tile-icons".to_string()));
    let frac = f("--frac").unwrap_or_else(|| "0.62".to_string());
    let margin = f("--margin").unwrap_or_else(|| "0.02".to_string());
    std::fs::create_dir_all(&work).map_err(|e| format!("{}: {e}", work.display()))?;
    let bin_dir = std::env::current_exe().ok().and_then(|p| p.parent().map(Path::to_path_buf)).ok_or("exe dir")?;
    let tinyctl = bin_dir.join("tinyctl");
    let tmmaps = bin_dir.join("tmmaps");
    let mapgeom = bin_dir.join("mapgeom");
    let spec_template = std::fs::read_to_string(&spec_path).map_err(|e| format!("{spec_path}: {e}"))?;
    // the spec names ONE surface folder under the root: find it and re-point per surface
    let first_dir = spec_template
        .split(';')
        .next()
        .and_then(|s| s.split('@').next())
        .and_then(|p| Path::new(p).parent().and_then(|p| p.parent()).map(Path::to_path_buf))
        .ok_or("spec: cannot find the surface folder in the first entry")?;
    let names: Vec<String> = std::fs::read_to_string(&views)
        .map_err(|e| format!("{views}: {e}"))?
        .lines()
        .filter_map(|l| l.split('\t').next().map(|s| s.trim().to_string()))
        .filter(|s| !s.is_empty())
        .collect();
    let pass_through: Vec<String> = args.iter().filter(|a| a.starts_with("--wsx") || a.starts_with("--idle")).cloned().collect();
    for surface in &surfaces {
        let sdir = root.join(surface);
        if !sdir.join("Pool").is_dir() {
            return Err(format!("{}: no Pool folder", sdir.display()));
        }
        let spec = spec_template.replace(&format!("{}/", first_dir.display()), &format!("{}/", sdir.display()));
        let map0 = work.join(format!("icon-{surface}.0.Map.Gbx"));
        let map = work.join(format!("IconMap-{surface}.Map.Gbx"));
        run(Command::new(&tmmaps).args(["add-items", &host, "--out"]).arg(&map0).arg("--items").arg(&spec), "tmmaps add-items")?;
        let uid: String = format!("WaterTilesIcon{surface:0<10}ABC").chars().take(27).collect();
        run(Command::new(&tmmaps).arg("rename-map").arg(&map0).arg("--out").arg(&map).args(["--name", &format!("Icon map {surface}"), "--uid", &uid]), "tmmaps rename-map")?;
        let remote = format!("/home/vjeux/ll-beam/IconMap-{surface}.Map.Gbx");
        run(Command::new("/home/vjeux/bin/wsx").arg("push").arg(&map).arg(&remote), "wsx push")?;
        let tag = format!("ic{surface}");
        let shot = run(
            Command::new(&tinyctl)
                .args(["shoot", "--orig"]).arg(&map).arg("--tiny").arg(&map)
                .args(["--views", &views, "--tag", &tag, "--anchor", "0,0,0:0,0,0", "--only", "o", "--pull-full", "--front", "--outdir"])
                .arg(work.join("shoot"))
                .args(&pass_through),
            "tinyctl shoot",
        )?;
        let ok_line = shot.lines().find(|l| l.contains("OK ") && l.contains("views")).unwrap_or("").trim().to_string();
        eprintln!("{surface}: {}", ok_line.chars().take(60).collect::<String>());
        let crops = work.join(format!("crops-{surface}"));
        let _ = std::fs::remove_dir_all(&crops);
        run(Command::new(&tinyctl).args(["icon-crops", "--tag", &tag, "--views", &views, "--out-dir"]).arg(&crops).args(["--size", "256", "--frac", &frac]).args(&pass_through), "tinyctl icon-crops")?;
        // `<Folder>-<Name>.bgra` → `<Folder>/<Name>.bgra`
        for n in &names {
            if let Some((folder, name)) = n.split_once('-') {
                let dst = crops.join(folder);
                std::fs::create_dir_all(&dst).map_err(|e| format!("{}: {e}", dst.display()))?;
                let src = crops.join(format!("{n}.bgra"));
                if src.exists() {
                    std::fs::rename(&src, dst.join(format!("{name}.bgra"))).map_err(|e| format!("{}: {e}", src.display()))?;
                }
            }
        }
        for sub in ["Pool", "Channel"] {
            let out = run(Command::new(&mapgeom).args(["item-icon", "--dir"]).arg(sdir.join(sub)).arg("--crops").arg(crops.join(sub)).args(["--margin", &margin]), "mapgeom item-icon")?;
            eprintln!("  {sub}: {}", out.lines().last().unwrap_or("").trim());
        }
        println!("{surface}: icons done");
    }
    Ok(())
}
