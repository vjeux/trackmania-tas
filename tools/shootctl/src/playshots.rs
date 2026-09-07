//! `shootctl playshots --map MAP --outdir /mnt/c/DIR [--tag T] [--shots N]
//! [--every-ms MS] [--first-ms MS] [--timeout S] [--detach]`: open the map in
//! PLAY mode (a playground, the car at the start) under the render lock and
//! take N timed screenshots — the test of whether a moving item's collision
//! moves with it (four half-size pushers around the spawn, 2026-09-07): if
//! their pistons shove the car, it is not where it spawned by the third shot.
//!
//! Like `shootset`, `--detach` re-runs in the background with the log in
//! `OUTDIR/playshots.log` and `OUTDIR/done-play.txt` written last.

use std::path::PathBuf;
use std::time::{Duration, Instant};

pub struct Opts {
    pub map: String,
    pub outdir: PathBuf,
    pub tag: String,
    pub shots: usize,
    pub every_ms: u64,
    pub first_ms: u64,
    pub timeout_s: u64,
    pub detach: bool,
}

pub fn parse_opts(args: &[String]) -> Result<Opts, String> {
    let val = |k: &str| -> Option<String> { args.iter().position(|a| a == k).and_then(|i| args.get(i + 1)).cloned() };
    let num = |k: &str, d: u64| -> Result<u64, String> { val(k).map(|s| s.parse::<u64>().map_err(|_| format!("{k} wants a number"))).transpose().map(|o| o.unwrap_or(d)) };
    let outdir = PathBuf::from(val("--outdir").ok_or("playshots needs --outdir <dir under /mnt/c>")?);
    if !outdir.starts_with("/mnt/") {
        return Err(format!("--outdir {} must live under /mnt/<drive>/ — the screenshot is taken by a Windows program", outdir.display()));
    }
    Ok(Opts {
        map: val("--map").ok_or("playshots needs --map <path>")?,
        outdir,
        tag: val("--tag").unwrap_or_else(|| "play".into()),
        shots: num("--shots", 4)? as usize,
        every_ms: num("--every-ms", 3000)?,
        first_ms: num("--first-ms", 4000)?,
        timeout_s: num("--timeout", 300)?,
        detach: args.iter().any(|a| a == "--detach"),
    })
}

pub fn run(args: &[String]) -> i32 {
    let opts = match parse_opts(args) {
        Ok(o) => o,
        Err(e) => {
            eprintln!("{e}");
            eprintln!("usage: shootctl playshots --map MAP --outdir /mnt/c/... [--tag T] [--shots N] [--every-ms MS] [--first-ms MS] [--timeout S] [--detach]");
            return 2;
        }
    };
    if let Err(e) = std::fs::create_dir_all(&opts.outdir) {
        eprintln!("{}: {e}", opts.outdir.display());
        return 2;
    }
    let done = opts.outdir.join("done-play.txt");
    let _ = std::fs::remove_file(&done);
    if opts.detach {
        return super::shootset::detach_as(&opts.outdir.join("playshots.log"), &done);
    }
    let t0 = Instant::now();
    let result = run_shots(&opts, t0);
    let summary = match &result {
        Ok(lines) => format!("OK {} shots in {:.0}s\n{}\n", lines.len(), t0.elapsed().as_secs_f64(), lines.join("\n")),
        Err(e) => format!("FAILED after {:.0}s: {e}\n", t0.elapsed().as_secs_f64()),
    };
    print!("{summary}");
    let tmp = opts.outdir.join("done-play.tmp");
    if std::fs::write(&tmp, &summary).and_then(|_| std::fs::rename(&tmp, &done)).is_err() {
        eprintln!("could not write {}", done.display());
        return 1;
    }
    if result.is_ok() { 0 } else { 1 }
}

fn run_shots(opts: &Opts, t0: Instant) -> Result<Vec<String>, String> {
    let el = || format!("[{:6.1}s]", t0.elapsed().as_secs_f64());
    let d = super::lock::lock_dir();
    let owner = format!("playshots-{}", opts.tag);
    super::lock::acquire(&d, &owner, 600, 0).map_err(|e| format!("lock: {e}"))?;
    let _guard = super::shootset::LockGuard::new(d, owner);
    let staged = super::shootset::stage_map(&opts.map)?;
    let game_map = super::game_path(&staged)?;
    println!("{} map {}", el(), game_map);
    if super::launch(180, false) != 0 {
        return Err("the game did not come up".into());
    }
    super::to_menu()?;
    super::await_cond("ready", 60)?;
    let store = "/mnt/c/Users/vjeux/OpenplanetNext/PluginStorage/GhostShooter";
    let _ = std::fs::create_dir_all(store);
    std::fs::write(format!("{store}/editmap.txt"), &game_map).map_err(|e| format!("editmap.txt: {e}"))?;
    println!("{} /playmap: {}", el(), super::http_get("/playmap?mode=", 30).unwrap_or_default().trim());
    let load0 = Instant::now();
    loop {
        if load0.elapsed().as_secs() > opts.timeout_s {
            return Err(format!("no playground in {} s; last ctx {}", opts.timeout_s, super::http_get("/ctx", 10).unwrap_or_default().trim()));
        }
        if !super::tm_running() {
            return Err("the game process is gone — the map crashed the client".into());
        }
        let c = super::http_get("/ctx", 10).unwrap_or_default();
        if c.contains("FrameAskYesNo") {
            let text = super::http_get("/dlgtext", 10).unwrap_or_default().trim().to_string();
            println!("{} DIALOG {}", el(), text);
            let _ = super::http_get("/yes", 10);
        }
        // anywhere but the menu: the playground is up (ctx 2 in play)
        if matches!(super::ctx(), Some(n) if n != 0) {
            break;
        }
        std::thread::sleep(Duration::from_millis(500));
    }
    let opened = load0.elapsed();
    println!("{} playground after {:.1}s (ctx {})", el(), opened.as_secs_f64(), super::http_get("/ctx", 10).unwrap_or_default().trim());
    let mut lines = Vec::new();
    std::thread::sleep(Duration::from_millis(opts.first_ms));
    for k in 0..opts.shots {
        if k > 0 {
            std::thread::sleep(Duration::from_millis(opts.every_ms));
        }
        let file = opts.outdir.join(format!("play-{}-{k}.png", opts.tag));
        let _ = std::fs::remove_file(&file);
        let at = load0.elapsed().as_secs_f64();
        super::shootset::screenshot(&file)?;
        let size = std::fs::metadata(&file).map(|m| m.len()).unwrap_or(0);
        let line = format!("shot {k}\t{at:.1}s after the playground opened\t{}\t{size}", file.display());
        println!("{} {line}", el());
        lines.push(line);
    }
    let _ = super::to_menu();
    Ok(lines)
}
