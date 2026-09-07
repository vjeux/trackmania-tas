//! `shootctl lightmap --map MAP --out OUT.Map.Gbx [--quality Q] [--detach]` —
//! the EDITOR's own lightmap for a tiny build, saved into a new map file.
//!
//! A tiny map ships without a usable lightmap: the source's was computed for
//! the full-size layout (a parked build shows every converted item BLACK in
//! play under it; a 0-block build makes the game reject it). The editor can
//! compute one for the tiny layout and re-save the map — that is what this
//! does, under the render lock, in one editor session:
//!
//!   probe edit MAP  →  /shadows?q=Q  →  wait for the lightmapper  →
//!   /mapsave OUT    →  wait for the file  →  /back
//!
//! Facts that shaped it (2026-09-07):
//! - `PluginMapType.SaveMap(path)` takes a path relative to the user's `Maps`
//!   folder (a `Maps/_shoot/x` argument lands in `Maps/Maps/_shoot/x`), and
//!   it RENAMES the map to the file's stem — so OUT is spelled with the name
//!   the map should carry, and the file is written under `Maps/`.
//! - The re-save regenerates the parked blocks' neighbours (8581 Lake blocks
//!   for 5737) and drops every item whose collision mesh is EMPTY (the
//!   `_FC_Ground` decals) — the bake gives those a 1 mm triangle now.
//! - A 0-block build with no lightmap crashes the editor's automatic
//!   lightmap pass (STACK_OVERFLOW); `/shadows` on one crashes the same way.
//!   Compute on the PARKED build (`TINY_PARK_BLOCKS=1`, lightmap stripped).
//! - `/shadowsq` reports `ready:true` between the request and the start of
//!   the work; the wait accepts `ready` only after seeing `busy`, or after
//!   20 s without ever seeing it (a Fast pass on a small map is that quick).
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

const MAPS: &str = "/mnt/c/Users/vjeux/OneDrive/Documents/Trackmania/Maps";

struct Opts {
    map: String,
    out: String,
    quality: u32,
    lock: bool,
    detach: bool,
    load_timeout_s: u64,
    compute_timeout_s: u64,
    log_dir: PathBuf,
}

fn parse(args: &[String]) -> Result<Opts, String> {
    let f = |k: &str| args.iter().position(|a| a == k).and_then(|i| args.get(i + 1)).cloned();
    let map = f("--map").ok_or("lightmap needs --map MAP")?;
    let out = f("--out").ok_or("lightmap needs --out NAME.Map.Gbx (relative to the user's Maps folder, e.g. _shoot/Tiny09-lm.Map.Gbx)")?;
    Ok(Opts {
        map,
        out,
        quality: f("--quality").and_then(|q| q.parse().ok()).unwrap_or(2),
        lock: !args.iter().any(|a| a == "--no-lock"),
        detach: args.iter().any(|a| a == "--detach"),
        load_timeout_s: f("--load-timeout").and_then(|s| s.parse().ok()).unwrap_or(420),
        compute_timeout_s: f("--compute-timeout").and_then(|s| s.parse().ok()).unwrap_or(1800),
        log_dir: PathBuf::from(f("--outdir").unwrap_or_else(|| "/mnt/c/Users/vjeux/tinyshots/lightmap".into())),
    })
}

pub fn run(args: &[String]) -> i32 {
    let opts = match parse(args) {
        Ok(o) => o,
        Err(e) => {
            eprintln!("{e}");
            eprintln!("usage: shootctl lightmap --map MAP --out REL.Map.Gbx [--quality 1..5] [--outdir DIR] [--load-timeout S] [--compute-timeout S] [--no-lock] [--detach]");
            return 2;
        }
    };
    let _ = std::fs::create_dir_all(&opts.log_dir);
    if opts.detach {
        return super::shootset::detach_as(&opts.log_dir.join("lightmap.log"), &opts.log_dir.join("done.txt"));
    }
    let done = opts.log_dir.join("done.txt");
    let _ = std::fs::remove_file(&done);
    let code = match go(&opts) {
        Ok(saved) => {
            println!("ok\t{saved}");
            let _ = std::fs::write(&done, format!("ok\t{saved}\n"));
            0
        }
        Err(e) => {
            eprintln!("lightmap: {e}");
            let _ = std::fs::write(&done, format!("fail\t{e}\n"));
            1
        }
    };
    code
}

fn go(opts: &Opts) -> Result<String, String> {
    let t0 = Instant::now();
    let el = |t0: &Instant| format!("[{:6.1}s]", t0.elapsed().as_secs_f64());
    let _lock = if opts.lock {
        let d = super::lock::lock_dir();
        let owner = format!("lightmap-{}", std::process::id());
        super::lock::acquire(&d, &owner, 1500, 0).map_err(|e| format!("lock: {e}"))?;
        Some(super::shootset::LockGuard::new(d, owner))
    } else {
        None
    };
    let staged = super::shootset::stage_map(&opts.map)?;
    let game_map = super::game_path(&staged)?;
    let want_uid = super::map_uid(&staged);
    println!("{} map {} (uid {}) quality {}", el(&t0), game_map, want_uid.clone().unwrap_or_else(|| "?".into()), opts.quality);
    if super::launch(180, false) != 0 {
        return Err("the game did not come up".into());
    }
    super::to_menu()?;
    super::await_cond("ready", 60)?;
    let store = "/mnt/c/Users/vjeux/OpenplanetNext";
    let _ = std::fs::create_dir_all(format!("{store}/PluginStorage/GhostShooter"));
    std::fs::write(format!("{store}/PluginStorage/GhostShooter/editmap.txt"), &game_map).map_err(|e| format!("editmap.txt: {e}"))?;
    println!("{} /editmap: {}", el(&t0), super::http_get("/editmap", 30).unwrap_or_default().trim());
    let load0 = Instant::now();
    loop {
        if load0.elapsed().as_secs() > opts.load_timeout_s {
            return Err(format!("the map did not open in {} s; last ctx {}", opts.load_timeout_s, super::http_get("/ctx", 10).unwrap_or_default().trim()));
        }
        if !super::tm_running() {
            return Err("the game process is gone — the map crashed the client while loading".into());
        }
        let c = super::http_get("/ctx", 10).unwrap_or_default();
        if c.contains("\"ctx\":1") {
            break;
        }
        if c.contains("FrameAskYesNo") {
            println!("{} DIALOG {}", el(&t0), super::http_get("/dlgtext", 10).unwrap_or_default().trim());
            let _ = super::http_get("/yes", 10);
            let _ = super::await_cond("nodialog", 5);
            let _ = super::http_get("/yes", 10);
        }
        std::thread::sleep(Duration::from_millis(1500));
    }
    println!("{} editor open after {:.1}s", el(&t0), load0.elapsed().as_secs_f64());
    if let (Some(w), Some(h)) = (&want_uid, super::loaded_uid()) {
        if *w != h {
            return Err(format!("the editor opened uid {h}, not the {w} we asked for"));
        }
    }
    std::thread::sleep(Duration::from_millis(4000));

    // the lightmap
    let ts = Instant::now();
    println!("{} /shadows q={}: {}", el(&t0), opts.quality, super::http_get(&format!("/shadows?q={}", opts.quality), 10).unwrap_or_default().trim());
    let mut saw_busy = false;
    loop {
        std::thread::sleep(Duration::from_millis(1000));
        if !super::tm_running() {
            return Err(format!("the game process is gone — the lightmapper crashed the client after {:.0}s", ts.elapsed().as_secs_f64()));
        }
        if ts.elapsed().as_secs() > opts.compute_timeout_s {
            return Err(format!("the lightmapper did not finish in {} s", opts.compute_timeout_s));
        }
        let c = super::http_get("/ctx", 10).unwrap_or_default();
        if c.contains("FrameAskYesNo") {
            println!("{} DIALOG {}", el(&t0), super::http_get("/dlgtext", 10).unwrap_or_default().trim());
            let _ = super::http_get("/yes", 10);
        }
        let s = super::http_get("/shadowsq", 10).unwrap_or_default();
        if s.contains("\"ready\":false") {
            saw_busy = true;
        } else if saw_busy || ts.elapsed().as_secs() > 20 {
            println!("{} shadows done in {:.0}s: {}{}", el(&t0), ts.elapsed().as_secs_f64(), s.trim(), if saw_busy { "" } else { " (never saw the editor busy)" });
            break;
        }
    }

    // the save: SaveMap's path is relative to the Maps folder
    let rel = opts.out.trim_start_matches("Maps/").trim_start_matches('/').to_string();
    let target = format!("{MAPS}/{rel}");
    let _ = std::fs::remove_file(&target);
    if let Some(parent) = Path::new(&target).parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    super::set_arg(&rel)?;
    println!("{} /mapsave {}: {}", el(&t0), rel, super::http_get("/mapsave", 25).unwrap_or_default().trim());
    let tsave = Instant::now();
    loop {
        std::thread::sleep(Duration::from_millis(1000));
        let c = super::http_get("/ctx", 10).unwrap_or_default();
        if c.contains("FrameAskYesNo") {
            println!("{} DIALOG {}", el(&t0), super::http_get("/dlgtext", 10).unwrap_or_default().trim());
            let _ = super::http_get("/yes", 10);
        }
        if let Ok(md) = std::fs::metadata(&target) {
            if md.len() > 0 {
                // the writer may still be flushing: wait for a stable size
                std::thread::sleep(Duration::from_millis(2000));
                let len2 = std::fs::metadata(&target).map(|m| m.len()).unwrap_or(0);
                if len2 == md.len() {
                    break;
                }
            }
        }
        if tsave.elapsed().as_secs() > 120 {
            return Err(format!("no file at {target} 120 s after SaveMap; ctx {}", c.trim()));
        }
    }
    let len = std::fs::metadata(&target).map(|m| m.len()).unwrap_or(0);
    println!("{} saved {} ({} bytes)", el(&t0), target, len);
    let _ = super::http_get("/back", 10);
    std::thread::sleep(Duration::from_millis(3000));
    Ok(target)
}
