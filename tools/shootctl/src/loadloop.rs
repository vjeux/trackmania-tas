//! `shootctl loadloop --maps A.Map.Gbx[,B.Map.Gbx…] --outdir /mnt/c/DIR [--tag T]
//! [--seq 0,1,0,1 | --n 10] [--how play|edit] [--timeout 300] [--settle-ms 3000]
//! [--fresh | --fresh-first] [--shot-on-fail] [--detach]`
//!
//! THE LOAD-RELIABILITY INSTRUMENT. One map (or a sequence of maps) opened
//! over and over on the render box, every load classified from the game's own
//! object graph — not from a screenshot somebody reads afterwards:
//!
//! * `OPENED` — the playground (or editor) came up, after how many seconds,
//!   and whether a car exists in it (`/wheel`: the VehicleState readout);
//! * `DIALOG` — the game raised a modal while loading (the "Error while
//!   retrieving map! Missing Items: …" FrameMessage of the big archives, a
//!   FrameAskYesNo, anything on CGameDialogs): its frame id and its TEXT are
//!   recorded, it is acknowledged (OK / yes), and the load counts as failed;
//! * `TIMEOUT` — nothing after `--timeout` seconds: no playground, no dialog
//!   (the black screen); the last `/ctx` is recorded;
//! * `CRASH` — the game process is gone.
//!
//! `--seq` is the order the maps are opened in, as indices into `--maps`
//! (`0,1,0,1` = A B A B); `--n` repeats that (or the single map) N times. The
//! switch ORDER is one of the suspects (fresh → 21 passed, 20 → 21 slow, 21 → 20
//! black), so it is a first-class parameter, and `--fresh` restarts the game
//! before EVERY load (a cold state per load), `--fresh-first` only before the
//! first. Between loads the driver goes back to the menu the way every other
//! driver does (`/back`, dialogs dismissed).
//!
//! Output: `OUTDIR/loadloop-<tag>.tsv` — one row per load:
//! `iter  map  outcome  seconds  car  dialog_frame  dialog_text  ctx  note`
//! — and the usual `loadloop.log` / `done-loadloop.txt` of a detached job.

use std::path::PathBuf;
use std::time::{Duration, Instant};

pub struct Opts {
    pub maps: Vec<String>,
    pub seq: Vec<usize>,
    pub outdir: PathBuf,
    pub tag: String,
    pub how: String,
    pub timeout_s: u64,
    pub settle_ms: u64,
    pub fresh: bool,
    pub fresh_first: bool,
    pub shot_on_fail: bool,
    pub detach: bool,
}

pub fn parse_opts(args: &[String]) -> Result<Opts, String> {
    let val = |k: &str| -> Option<String> { args.iter().position(|a| a == k).and_then(|i| args.get(i + 1)).cloned() };
    let num = |k: &str, d: u64| -> Result<u64, String> { val(k).map(|s| s.parse::<u64>().map_err(|_| format!("{k} wants a number"))).transpose().map(|o| o.unwrap_or(d)) };
    let outdir = PathBuf::from(val("--outdir").ok_or("loadloop needs --outdir <dir under /mnt/c>")?);
    if !outdir.starts_with("/mnt/") {
        return Err(format!("--outdir {} must live under /mnt/<drive>/ — the screenshot is taken by a Windows program", outdir.display()));
    }
    let maps: Vec<String> = val("--maps").ok_or("loadloop needs --maps A[,B,…]")?.split(',').filter(|s| !s.is_empty()).map(String::from).collect();
    if maps.is_empty() {
        return Err("--maps names no map".into());
    }
    let n = num("--n", 0)? as usize;
    let mut seq: Vec<usize> = match val("--seq") {
        Some(s) => s.split(',').filter(|s| !s.is_empty()).map(|s| s.parse::<usize>().map_err(|_| format!("--seq: `{s}` is not an index"))).collect::<Result<_, _>>()?,
        None => (0..maps.len()).collect(),
    };
    if let Some(bad) = seq.iter().find(|&&i| i >= maps.len()) {
        return Err(format!("--seq index {bad} but only {} maps", maps.len()));
    }
    if n > 0 {
        let one = seq.clone();
        seq = (0..n).flat_map(|_| one.iter().copied()).collect();
    }
    let how = val("--how").unwrap_or_else(|| "play".into());
    if how != "play" && how != "edit" {
        return Err("--how play|edit".into());
    }
    Ok(Opts {
        maps,
        seq,
        outdir,
        tag: val("--tag").unwrap_or_else(|| "loadloop".into()),
        how,
        timeout_s: num("--timeout", 300)?,
        settle_ms: num("--settle-ms", 3000)?,
        fresh: args.iter().any(|a| a == "--fresh"),
        fresh_first: args.iter().any(|a| a == "--fresh-first"),
        shot_on_fail: args.iter().any(|a| a == "--shot-on-fail"),
        detach: args.iter().any(|a| a == "--detach"),
    })
}

pub fn run(args: &[String]) -> i32 {
    let opts = match parse_opts(args) {
        Ok(o) => o,
        Err(e) => {
            eprintln!("{e}");
            eprintln!("usage: shootctl loadloop --maps A[,B…] --outdir /mnt/c/... [--tag T] [--seq 0,1,…] [--n N] [--how play|edit] [--timeout S] [--settle-ms MS] [--fresh|--fresh-first] [--shot-on-fail] [--detach]");
            return 2;
        }
    };
    if let Err(e) = std::fs::create_dir_all(&opts.outdir) {
        eprintln!("{}: {e}", opts.outdir.display());
        return 2;
    }
    let done = opts.outdir.join("done-loadloop.txt");
    let _ = std::fs::remove_file(&done);
    if opts.detach {
        return super::shootset::detach_as(&opts.outdir.join("loadloop.log"), &done);
    }
    let t0 = Instant::now();
    let result = run_loop(&opts, t0);
    let summary = match &result {
        Ok(lines) => format!("OK {} loads in {:.0}s\n{}\n", lines.len(), t0.elapsed().as_secs_f64(), lines.join("\n")),
        Err(e) => format!("FAILED after {:.0}s: {e}\n", t0.elapsed().as_secs_f64()),
    };
    print!("{summary}");
    let tmp = opts.outdir.join("done-loadloop.tmp");
    if std::fs::write(&tmp, &summary).and_then(|_| std::fs::rename(&tmp, &done)).is_err() {
        eprintln!("could not write {}", done.display());
        return 1;
    }
    if result.is_ok() { 0 } else { 1 }
}

/// One load's verdict.
struct Load {
    outcome: &'static str,
    seconds: f64,
    car: String,
    frame: String,
    text: String,
    ctx: String,
    note: String,
}

fn tsv_clean(s: &str) -> String {
    s.replace(['\t', '\n', '\r'], " ")
}

fn run_loop(opts: &Opts, t0: Instant) -> Result<Vec<String>, String> {
    let el = || format!("[{:6.1}s]", t0.elapsed().as_secs_f64());
    let d = super::lock::lock_dir();
    let owner = format!("loadloop-{}", opts.tag);
    super::lock::acquire(&d, &owner, 1500, 0).map_err(|e| format!("lock: {e}"))?;
    let _guard = super::shootset::LockGuard::new(d, owner);
    // stage every map once
    let mut staged = Vec::new();
    for m in &opts.maps {
        let s = super::shootset::stage_map(m)?;
        let g = super::game_path(&s)?;
        println!("{} map {} -> {}", el(), m, g);
        staged.push(g);
    }
    let tsv_path = opts.outdir.join(format!("loadloop-{}.tsv", opts.tag));
    let mut tsv = String::from("iter\tmap\toutcome\tseconds\tcar\tdialog_frame\tdialog_text\tctx\tnote\n");
    std::fs::write(&tsv_path, &tsv).map_err(|e| format!("{}: {e}", tsv_path.display()))?;
    let store = "/mnt/c/Users/vjeux/OpenplanetNext/PluginStorage/GhostShooter";
    let _ = std::fs::create_dir_all(store);
    let (door, want_ctx) = match opts.how.as_str() {
        "play" => ("/playmap?mode=", 3i64),
        _ => ("/editmap", 1i64),
    };
    let mut lines = Vec::new();
    let mut crashes = 0u32;
    for (iter, &mi) in opts.seq.iter().enumerate() {
        let game_map = &staged[mi];
        let short = std::path::Path::new(&opts.maps[mi]).file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        if opts.fresh || (opts.fresh_first && iter == 0) {
            println!("{} #{iter} restarting the game (fresh state)", el());
            super::quit_game();
            std::thread::sleep(Duration::from_secs(3));
        }
        if super::launch(180, false) != 0 {
            return Err("the game did not come up".into());
        }
        // THE MENU, PROVEN: ctx 0 is not enough (a playground the plugin reads
        // as ctx 0 / playground:false was on screen for minutes on 2026-09-08
        // while every /playmap into it raised an AskYesNo within a second);
        // the title API's IsReady is the fact that a load can be asked for.
        // Not ready after the menu round trip → one Escape (a menu popup),
        // then a game restart — never a load into an unknown state.
        super::to_menu()?;
        if let Err(e) = super::await_cond("ready", 20) {
            println!("{} #{iter} title not ready after the menu round trip ({e}); tapping ESC", el());
            let f = opts.outdir.join(format!("notready-{}-{iter:02}.png", opts.tag));
            let _ = super::shootset::screenshot(&f);
            let _ = super::playshots::tap_key("ESC", 60);
            let _ = super::to_menu();
            if super::await_cond("ready", 15).is_err() {
                println!("{} #{iter} still not ready; restarting the game", el());
                super::quit_game();
                std::thread::sleep(Duration::from_secs(3));
                if super::launch(180, false) != 0 {
                    return Err("the game did not come up after the restart".into());
                }
                super::to_menu()?;
                super::await_cond("ready", 60)?;
            }
        }
        std::fs::write(format!("{store}/editmap.txt"), game_map).map_err(|e| format!("editmap.txt: {e}"))?;
        let load0 = Instant::now();
        let ack = super::http_get(door, 30).unwrap_or_default().trim().to_string();
        println!("{} #{iter} {short}: {door} -> {ack}", el());
        let mut last = String::new();
        let mut stable = 0u32;
        let mut dead_polls = 0u32;
        let mut v = Load { outcome: "TIMEOUT", seconds: 0.0, car: "-".into(), frame: "-".into(), text: "-".into(), ctx: "-".into(), note: String::new() };
        loop {
            if load0.elapsed().as_secs() > opts.timeout_s {
                v.seconds = load0.elapsed().as_secs_f64();
                v.ctx = tsv_clean(&last);
                println!("{} #{iter} TIMEOUT after {:.1}s; last ctx {last}", el(), v.seconds);
                break;
            }
            if !super::tm_running() {
                v.outcome = "CRASH";
                v.seconds = load0.elapsed().as_secs_f64();
                println!("{} #{iter} CRASH: the game process is gone at +{:.1}s", el(), v.seconds);
                break;
            }
            let ctx_reply = super::http_get("/ctx", 10);
            // THE GAME DIED BUT THE PROCESS IS STILL THERE: after a crash the
            // crash reporter keeps Trackmania.exe alive (a report window,
            // WerFault writing its dump) while the plugin is gone, so
            // `tm_running()` stays true and the plugin refuses every connect.
            // Three refusals in a row (~30 s) are that, not a slow load — the
            // first version sat on the render lock for the whole timeout with
            // a dead game and every other driver queued behind it
            // (2026-09-08, 14 minutes).
            if ctx_reply.is_err() {
                dead_polls += 1;
                if dead_polls >= 3 {
                    v.outcome = "CRASH";
                    v.seconds = load0.elapsed().as_secs_f64();
                    v.note = format!("plugin unreachable {dead_polls} polls; process {}", if super::tm_running() { "still present (crash reporter?) — killed" } else { "gone" });
                    println!("{} #{iter} CRASH: the plugin stopped answering at +{:.1}s ({})", el(), v.seconds, v.note);
                    super::quit_game();
                    break;
                }
            } else {
                dead_polls = 0;
            }
            let c = ctx_reply.unwrap_or_default().trim().to_string();
            // the timeline: every change of /ctx or /ready, like loadprof
            let ready = super::http_get("/ready", 10).unwrap_or_default().trim().to_string();
            let line = format!("{c} | {ready}");
            if line != last {
                println!("{} #{iter} +{:6.1}s {line}", el(), load0.elapsed().as_secs_f64());
                last = line;
            }
            // any modal on CGameDialogs while loading
            if let Some(frame) = dialog_frame(&c) {
                let text = super::http_get("/dlgtext", 10).unwrap_or_default().trim().to_string();
                v.outcome = "DIALOG";
                v.seconds = load0.elapsed().as_secs_f64();
                v.frame = frame.clone();
                v.text = tsv_clean(&text);
                v.ctx = tsv_clean(&c);
                println!("{} #{iter} DIALOG {frame} after {:.1}s: {text}", el(), v.seconds);
                if opts.shot_on_fail {
                    let f = opts.outdir.join(format!("fail-{}-{iter:02}.png", opts.tag));
                    if super::shootset::screenshot(&f).is_ok() {
                        v.note = format!("shot {}", f.display());
                    }
                }
                let ans = match frame.as_str() {
                    "FrameAskYesNo" => super::http_get("/yes", 10),
                    _ => super::http_get("/dlgok", 10),
                };
                println!("{} #{iter} answered: {}", el(), ans.unwrap_or_default().trim());
                let _ = super::await_cond("nodialog", 10);
                break;
            }
            // the playground is ctx 3 (CurrentPlayground, no editor), the map
            // editor ctx 1 — and 0.3 s after /playmap the game shows a TRANSIENT
            // ctx 3 with playground:true and map:null before dropping back to 0
            // for the load itself (measured 2026-09-08, 3 of 3 loads). So the
            // open is the SNAPSHOT saying ctx == want with RootMap set, held
            // over three consecutive polls a second apart.
            if ctx_of(&c) == Some(want_ctx) && !c.contains("\"map\":null") {
                stable += 1;
            } else {
                stable = 0;
            }
            if stable >= 3 {
                v.outcome = "OPENED";
                v.seconds = load0.elapsed().as_secs_f64();
                v.ctx = tsv_clean(&c);
                println!("{} #{iter} OPENED after {:.1}s ({c})", el(), v.seconds);
                break;
            }
            std::thread::sleep(Duration::from_millis(if stable > 0 { 1000 } else { 250 }));
        }
        if v.outcome == "OPENED" {
            std::thread::sleep(Duration::from_millis(opts.settle_ms));
            if opts.how == "play" {
                // a car in the playground? (the VehicleState readout)
                let w = super::http_get("/wheel", 10).unwrap_or_default();
                let row = w.lines().find(|l| !l.starts_with('#') && !l.starts_with("wall_ms") && l.split('\t').count() > 4);
                v.car = match row {
                    Some(r) => {
                        let c: Vec<&str> = r.split('\t').collect();
                        format!("yes [{} {} {}]", c[2], c[3], c[4])
                    }
                    None => format!("NO ({})", tsv_clean(w.trim().lines().next().unwrap_or(""))),
                };
            } else {
                v.car = "n/a".into();
            }
            // a dialog that came up AFTER the open (the editor's, the intro's)
            let c = super::http_get("/ctx", 10).unwrap_or_default();
            if let Some(frame) = dialog_frame(&c) {
                let text = super::http_get("/dlgtext", 10).unwrap_or_default().trim().to_string();
                v.note = format!("post-open dialog {frame}: {}", tsv_clean(&text));
            }
            println!("{} #{iter} car {}", el(), v.car);
        } else if v.outcome == "TIMEOUT" && opts.shot_on_fail {
            let f = opts.outdir.join(format!("fail-{}-{iter:02}.png", opts.tag));
            if super::shootset::screenshot(&f).is_ok() {
                v.note = format!("shot {}", f.display());
            }
        }
        let row = format!("{iter}\t{short}\t{}\t{:.1}\t{}\t{}\t{}\t{}\t{}", v.outcome, v.seconds, v.car, v.frame, v.text, v.ctx, v.note);
        tsv.push_str(&row);
        tsv.push('\n');
        std::fs::write(&tsv_path, &tsv).map_err(|e| format!("{}: {e}", tsv_path.display()))?;
        lines.push(row);
        if v.outcome == "CRASH" {
            crashes += 1;
            if crashes >= 2 {
                // a map that crashes the client twice is a finding, not a
                // retry: stop here (the lock guard releases on return)
                println!("{} #{iter} second crash of this run — stopping the loop", el());
                lines.push(format!("stopped\tafter {crashes} crashes"));
                break;
            }
            // the next iteration relaunches; give the crash handler its time
            std::thread::sleep(Duration::from_secs(5));
            continue;
        }
        if let Err(e) = super::to_menu() {
            println!("{} #{iter} back to menu: {e}", el());
        }
    }
    let opened = lines.iter().filter(|l| l.split('\t').nth(2) == Some("OPENED")).count();
    println!("{} {} of {} loads OPENED; table {}", el(), opened, lines.len(), tsv_path.display());
    lines.push(format!("table\t{}", tsv_path.display()));
    Ok(lines)
}

/// The `"ctx":N` of a `/ctx` reply.
fn ctx_of(ctx: &str) -> Option<i64> {
    let key = "\"ctx\":";
    let i = ctx.find(key)? + key.len();
    let rest = &ctx[i..];
    let end = rest.find(|c: char| !c.is_ascii_digit()).unwrap_or(rest.len());
    rest[..end].parse().ok()
}

/// The `"dialog":"FrameX"` of a `/ctx` reply, `None` for `"dialog":null`.
fn dialog_frame(ctx: &str) -> Option<String> {
    let key = "\"dialog\":\"";
    let i = ctx.find(key)? + key.len();
    let rest = &ctx[i..];
    let end = rest.find('"')?;
    Some(rest[..end].to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ctx_of_reads_the_number() {
        assert_eq!(ctx_of(r#"{"ctx":3,"editor":"none","playground":true,"map":null}"#), Some(3));
        assert_eq!(ctx_of("nonsense"), None);
    }

    #[test]
    fn dialog_frame_reads_ctx() {
        assert_eq!(dialog_frame(r#"{"ctx":0,"editor":"none","playground":false,"map":null,"dialog":"FrameMessage"}"#).as_deref(), Some("FrameMessage"));
        assert_eq!(dialog_frame(r#"{"ctx":0,"dialog":null}"#), None);
    }

    #[test]
    fn seq_and_n_compose() {
        let a: Vec<String> = ["--maps", "a,b", "--outdir", "/mnt/c/x", "--seq", "0,1", "--n", "2"].iter().map(|s| s.to_string()).collect();
        let o = parse_opts(&a).unwrap();
        assert_eq!(o.seq, vec![0, 1, 0, 1]);
        let a: Vec<String> = ["--maps", "a", "--outdir", "/mnt/c/x", "--n", "3"].iter().map(|s| s.to_string()).collect();
        assert_eq!(parse_opts(&a).unwrap().seq, vec![0, 0, 0]);
    }
}
