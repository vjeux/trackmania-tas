//! `clip film` -- one run to a published clip, ONE COMMAND on the render box:
//!
//! ```text
//! clip film --map MAP --name NAME --outdir /mnt/c/DIR --mapdir MAPDIR GHOST [OPPONENT...]
//!           [--cam N] [--to S] [--crf Q] [--footage S] [--load-timeout S]
//!           [--shootctl P] [--no-ship] [--quit] [--detach]
//! ```
//!
//! The chain this replaces was three commands and two polls over a bridge that
//! cuts a call at ~90 s (`shootctl render --detach`, watch `done-render.txt`,
//! `clip cut --ghost`, `clip ship --no-mirror`, read the URL off the log), and
//! every re-render of a newer tape paid for it again by hand. It is one job:
//!
//! 1. **`shootctl render`** -- the game: the map staged under `Maps/_shoot`, the
//!    ghosts imported (the FIRST one is the run; the camera follows it), the
//!    MediaTracker shoot at 1080p30, the two contact sheets
//!    (`DIR/NAME-sheet.png`, `DIR/NAME-dense.png`) -- run in the FOREGROUND,
//!    its lines in this log, its `done-render.txt` read back for the webm.
//! 2. **the cut**, with the run's controls drawn on it and the video<->tape
//!    timing checked against the picture ([`crate::cut`], the default since
//!    2026-09-09) -- `DIR/NAME.mp4`.
//! 3. **the ship**, `--no-mirror` (vjeux 2026-09-08: "not in releases" -- the
//!    inline player is the only copy, the release BODY registration is what
//!    makes it public), the anonymous gate included. `--no-ship` stops after
//!    the cut, for a look at the frames before anything is public.
//!
//! `DIR/done-film.txt` = `OK <url|-> <mp4> <webm> <seconds>` or `FAILED after
//! Ns: <why>`, written last and atomically, so a caller on another machine polls
//! a file instead of holding a connection; `--detach` backgrounds the whole
//! thing with the log in `DIR/film.log`, exactly like `shootctl render`.
//!
//! What is checked BEFORE the game is asked for three minutes of rendering: the
//! ghosts and the map exist, `--outdir` is on a Windows drive (the box's ffmpeg
//! is a Windows build and cannot write a WSL path), shootctl is there, and --
//! unless `--no-ship` -- the upload cookie is at hand (`GH_COOKIE`, else
//! `~/.gh-upload/cookie`) and `gh` is reachable (PATH, else `~/bin/gh`). A
//! cookie that turns out expired still fails at the upload (`ghvid.sh` exit 3);
//! nothing here can know that sooner. The pre-flight is what turns "rendered,
//! cut, then died shipping for want of a cookie" into a refusal in a second.
//!
//! **When the timing guard refuses a clip whose picture is right** (`clip cut`
//! measures the video<->tape offset from the yaw and refuses past the chase
//! camera's spring; colon three's full-lock keyboard start pivot drags the camera
//! 470 ms behind the tape, past the 450 ms bar, while the finish-arch clock in the
//! frame reads 07:200 at video 7.20 -- offset 0 to the frame), LOOK, then
//! `--from-webm <the render> --offset-ms 0`: no game, the sheets from the
//! existing webm, the cut at the offset the picture proved, the ship.
//!
//! The md5 of every ghost handed to the game is printed first: FILMING.md's
//! first gate is "which file, exactly", and a clip is only as honest as the
//! answer.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::Instant;

use crate::cut;
use crate::fmt::secs;
use crate::md5::md5_hex;
use crate::platform;
use crate::ship;

/// Where the box keeps its shootctl; `--shootctl` for another.
pub const DEFAULT_SHOOTCTL: &str = "/home/vjeux/trackmania-tas/tools/target/release/shootctl";
/// The browser Cookie header `ghvid.sh` uploads with, banked on the box.
pub const COOKIE_FILE: &str = ".gh-upload/cookie";

pub struct Opts {
    pub map: String,
    pub name: String,
    pub outdir: PathBuf,
    pub mapdir: PathBuf,
    pub cam: u8,
    pub to: Option<f64>,
    pub crf: u32,
    pub footage_s: f64,
    pub load_timeout_s: u64,
    pub shootctl: PathBuf,
    /// `--offset-ms N`: force the video<->tape offset instead of measuring it
    /// against the picture -- for a clip whose picture HAS been looked at
    /// (the finish-arch clock on colon three reads 07:200 at video 7.20, so
    /// the offset is 0 to the frame) and whose yaw fit still sits outside the
    /// guard (a violent full-lock start pivot drags the chase camera 470 ms
    /// behind the tape, past the 450 ms bar).
    pub offset_ms: Option<i64>,
    /// `--from-webm F`: skip the game -- cut, sheets and ship an existing
    /// render (the one a refused timing check left behind, looked at since).
    pub from_webm: Option<String>,
    pub ship: bool,
    pub quit: bool,
    pub detach: bool,
    pub ghosts: Vec<String>,
}

pub const USAGE: &str = "film --map MAP --name NAME --outdir /mnt/c/DIR --mapdir MAPDIR GHOST [OPPONENT...] \
[--cam N] [--to S] [--crf Q] [--footage S] [--load-timeout S] [--shootctl P] [--offset-ms N] [--from-webm F] [--no-ship] [--quit] [--detach]";

pub fn parse(args: &[String]) -> Result<Opts, String> {
    let mut o = Opts {
        map: String::new(),
        name: String::new(),
        outdir: PathBuf::new(),
        mapdir: PathBuf::new(),
        cam: 2,
        to: None,
        crf: 19,
        footage_s: 0.0,
        load_timeout_s: 120,
        shootctl: PathBuf::from(DEFAULT_SHOOTCTL),
        offset_ms: None,
        from_webm: None,
        ship: true,
        quit: false,
        detach: false,
        ghosts: Vec::new(),
    };
    let mut i = 0;
    while i < args.len() {
        let a = args[i].as_str();
        let val = |i: usize| args.get(i + 1).cloned().ok_or_else(|| format!("{a} needs a value"));
        match a {
            "--map" => { o.map = val(i)?; i += 2; }
            "--name" => { o.name = val(i)?; i += 2; }
            "--outdir" => { o.outdir = PathBuf::from(val(i)?); i += 2; }
            "--mapdir" => { o.mapdir = PathBuf::from(val(i)?); i += 2; }
            "--cam" => {
                o.cam = val(i)?.parse::<u8>().ok().filter(|c| *c <= 6).ok_or("--cam takes 0..6 (2 External, 1 Internal, 6 Ext2, 3 Helico)")?;
                i += 2;
            }
            "--to" => { o.to = Some(val(i)?.parse().map_err(|_| "--to wants seconds")?); i += 2; }
            "--crf" => { o.crf = val(i)?.parse().map_err(|_| "--crf wants an integer")?; i += 2; }
            "--footage" => { o.footage_s = val(i)?.parse().map_err(|_| "--footage wants seconds of clip")?; i += 2; }
            "--load-timeout" => { o.load_timeout_s = val(i)?.parse().map_err(|_| "--load-timeout wants seconds")?; i += 2; }
            "--shootctl" => { o.shootctl = PathBuf::from(val(i)?); i += 2; }
            "--offset-ms" => { o.offset_ms = Some(val(i)?.parse().map_err(|_| "--offset-ms wants milliseconds (an integer)")?); i += 2; }
            "--from-webm" => { o.from_webm = Some(val(i)?); i += 2; }
            "--no-ship" => { o.ship = false; i += 1; }
            "--quit" => { o.quit = true; i += 1; }
            "--detach" => { o.detach = true; i += 1; }
            other if other.starts_with("--") => return Err(format!("film: unknown option {other}\n{USAGE}")),
            _ => { o.ghosts.push(args[i].clone()); i += 1; }
        }
    }
    if (o.map.is_empty() && o.from_webm.is_none()) || o.name.is_empty() || o.outdir.as_os_str().is_empty() || o.ghosts.is_empty() {
        return Err(format!("film: --map (or --from-webm), --name, --outdir and at least one GHOST are required\n{USAGE}"));
    }
    if o.ship && o.mapdir.as_os_str().is_empty() {
        return Err(format!("film: --mapdir (the map's directory in the repo; its name labels the registration) is required unless --no-ship\n{USAGE}"));
    }
    if !o.name.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_') {
        return Err(format!("film: --name {}: letters, digits, - and _ only (it becomes the clip's file name)", o.name));
    }
    Ok(o)
}

/// The refusals that cost a second, before the ones that cost a render.
fn preflight(o: &Opts) -> Result<(), String> {
    for g in &o.ghosts {
        if !Path::new(g).is_file() {
            return Err(format!("{g}: no such ghost file"));
        }
    }
    if let Some(w) = &o.from_webm {
        if !Path::new(w).is_file() {
            return Err(format!("{w}: no such webm (--from-webm)"));
        }
    } else {
        let map_wsl = o.map.strip_prefix("C:/").map(|r| format!("/mnt/c/{r}")).unwrap_or_else(|| o.map.clone());
        if !Path::new(&map_wsl).is_file() {
            return Err(format!("{}: no such map file", o.map));
        }
    }
    if !o.shootctl.is_file() {
        return Err(format!("{}: no shootctl there (--shootctl P names another)", o.shootctl.display()));
    }
    if platform::wsl_to_windows(&o.outdir).is_none() {
        return Err(format!(
            "--outdir {} is not on a Windows drive: the box's ffmpeg is a Windows build and cannot write a WSL path. Use /mnt/c/Users/vjeux/tm-video/<map>/",
            o.outdir.display()
        ));
    }
    if o.ship {
        // The cookie: GH_COOKIE as given, else the banked file. Set into this
        // process's environment so ghvid.sh (a child of `ship`) inherits it.
        let have = std::env::var("GH_COOKIE").ok().filter(|v| !v.trim().is_empty());
        if have.is_none() {
            let home = std::env::var("HOME").unwrap_or_else(|_| "/home/vjeux".into());
            let f = Path::new(&home).join(COOKIE_FILE);
            let c = std::fs::read_to_string(&f).map_err(|e| {
                format!("no GH_COOKIE in the environment and {} is not readable ({e}): the user-attachments upload needs the browser Cookie header", f.display())
            })?;
            let c: String = c.chars().filter(|ch| *ch != '\r' && *ch != '\n').collect();
            if c.trim().is_empty() {
                return Err(format!("{} is empty", f.display()));
            }
            std::env::set_var("GH_COOKIE", c);
            println!("film: GH_COOKIE from {}", f.display());
        }
        // `gh`: on PATH, else the box's ~/bin/gh, handed to `ship` via CLIP_GH.
        if std::env::var("CLIP_GH").ok().filter(|v| !v.is_empty()).is_none() && platform::which("gh").is_none() {
            let home = std::env::var("HOME").unwrap_or_else(|_| "/home/vjeux".into());
            let gh = Path::new(&home).join("bin/gh");
            if !gh.is_file() {
                return Err("gh is not on PATH and ~/bin/gh is not there: the release-body registration needs it (CLIP_GH=/path/to/gh)".into());
            }
            std::env::set_var("CLIP_GH", &gh);
            println!("film: gh at {}", gh.display());
        }
    }
    Ok(())
}

/// `OK <webm> <bytes> <seconds>` from shootctl's done file.
pub fn parse_done_render(s: &str) -> Result<(String, u64, f64), String> {
    let line = s.lines().find(|l| !l.trim().is_empty()).unwrap_or("").trim();
    let mut it = line.split_whitespace();
    match it.next() {
        Some("OK") => {}
        _ => return Err(format!("shootctl render did not finish with OK: `{line}`")),
    }
    let webm = it.next().ok_or("done-render.txt: no webm path")?.to_string();
    let bytes: u64 = it.next().and_then(|b| b.parse().ok()).ok_or("done-render.txt: no byte count")?;
    let secs: f64 = it.next().and_then(|b| b.parse().ok()).ok_or("done-render.txt: no duration")?;
    Ok((webm, bytes, secs))
}

pub struct Outcome {
    pub webm: String,
    pub mp4: PathBuf,
    pub secs: f64,
    pub url: Option<String>,
}

pub fn run(args: &[String]) -> Result<(), String> {
    let o = parse(args)?;
    std::fs::create_dir_all(&o.outdir).map_err(|e| format!("{}: {e}", o.outdir.display()))?;
    let done = o.outdir.join("done-film.txt");
    let _ = std::fs::remove_file(&done);
    if o.detach {
        return detach(&o.outdir.join("film.log"), &done);
    }
    let t0 = Instant::now();
    let result = film(&o, t0);
    let summary = match &result {
        Ok(out) => format!(
            "OK {} {} {} {:.3}\n",
            out.url.as_deref().unwrap_or("-"),
            out.mp4.display(),
            out.webm,
            out.secs
        ),
        Err(e) => format!("FAILED after {:.0}s: {e}\n", t0.elapsed().as_secs_f64()),
    };
    print!("{summary}");
    let tmp = o.outdir.join("done-film.tmp");
    std::fs::write(&tmp, &summary)
        .and_then(|_| std::fs::rename(&tmp, &done))
        .map_err(|e| format!("could not write {}: {e}", done.display()))?;
    result.map(|_| ())
}

fn film(o: &Opts, t0: Instant) -> Result<Outcome, String> {
    let el = || format!("[{:6.1}s]", t0.elapsed().as_secs_f64());
    preflight(o)?;
    for g in &o.ghosts {
        let data = std::fs::read(g).map_err(|e| format!("{g}: {e}"))?;
        println!("film: ghost {} md5 {} ({} bytes)", g, md5_hex(&data), data.len());
    }
    let ff = platform::from_env()?;

    // 1. the game -- or, with --from-webm, the render that already exists.
    let (webm, bytes, webm_secs) = if let Some(w) = &o.from_webm {
        let bytes = std::fs::metadata(w).map(|m| m.len()).map_err(|e| format!("{w}: {e}"))?;
        let s = ff.probe_duration(Path::new(w))?;
        println!("{} --from-webm {w} ({bytes} bytes, {} s): no game", el(), secs(s));
        let st = Command::new(&o.shootctl)
            .args(["render", "--sheets-only", w, "--outdir"])
            .arg(&o.outdir)
            .args(["--name", &o.name])
            .stdin(Stdio::null())
            .status()
            .map_err(|e| format!("cannot run {}: {e}", o.shootctl.display()))?;
        if !st.success() {
            return Err(format!("shootctl render --sheets-only failed ({st})"));
        }
        (w.clone(), bytes, s)
    } else {
        let mut c = Command::new(&o.shootctl);
        c.arg("render")
            .args(["--map", &o.map, "--name", &o.name])
            .arg("--outdir")
            .arg(&o.outdir)
            .args(["--cam", &o.cam.to_string(), "--load-timeout", &o.load_timeout_s.to_string()]);
        if o.footage_s > 0.0 {
            c.args(["--footage", &format!("{}", o.footage_s)]);
        }
        if o.quit {
            c.arg("--quit");
        }
        c.args(&o.ghosts);
        println!("{} shootctl render --name {} ({} ghost(s), cam {})", el(), o.name, o.ghosts.len(), o.cam);
        let st = c
            .stdin(Stdio::null())
            .status()
            .map_err(|e| format!("cannot run {}: {e}", o.shootctl.display()))?;
        let done_render = o.outdir.join("done-render.txt");
        let text = std::fs::read_to_string(&done_render).unwrap_or_default();
        if !st.success() {
            return Err(format!("shootctl render failed ({st}): {}", text.trim()));
        }
        let (webm, bytes, webm_secs) = parse_done_render(&text)?;
        println!("{} rendered {webm} ({bytes} bytes, {} s)", el(), secs(webm_secs));
        (webm, bytes, webm_secs)
    };
    let _ = (bytes, webm_secs);

    // 2. the cut, with the run's own controls on it.
    let mp4 = o.outdir.join(format!("{}.mp4", o.name));
    let co = cut::CutOpts {
        to: o.to,
        crf: o.crf,
        ghost: Some(PathBuf::from(&o.ghosts[0])),
        offset_ms: o.offset_ms,
        nominal_ms: 0,
        bare: false,
    };
    cut::run_opts(&ff, Path::new(&webm), &mp4, &co)?;
    let mp4_secs = ff.probe_duration(&mp4)?;
    println!("{} cut {} ({} s)", el(), mp4.display(), secs(mp4_secs));
    println!("{} sheets {}-sheet.png {}-dense.png", el(), o.outdir.join(&o.name).display(), o.outdir.join(&o.name).display());

    // 3. the ship.
    let url = if o.ship {
        let mut cfg = ship::Cfg::from_env();
        cfg.mirror = false;
        let url = ship::run(&ff, &cfg, &mp4, &o.mapdir, None)?;
        println!("{} PUBLISHED {url}", el());
        Some(url)
    } else {
        println!("{} --no-ship: not published. Look at the sheets, then `clip ship {} <mapdir> --no-mirror`.", el(), mp4.display());
        None
    };
    Ok(Outcome { webm, mp4, secs: mp4_secs, url })
}

/// Re-run this very command in the background, `--detach` removed, stdout and
/// stderr into `log`, and return at once -- what `shootctl render --detach`
/// does, for the same reason: the bridge cuts a call at ~90 s and a film takes
/// minutes.
fn detach(log: &Path, done: &Path) -> Result<(), String> {
    use std::os::unix::process::CommandExt;
    let exe = std::env::current_exe().map_err(|e| format!("current_exe: {e}"))?;
    let args: Vec<String> = std::env::args().skip(1).filter(|a| a != "--detach").collect();
    let out = std::fs::File::create(log).map_err(|e| format!("{}: {e}", log.display()))?;
    let err = out.try_clone().map_err(|e| format!("clone log handle: {e}"))?;
    let child = Command::new(exe)
        .args(&args)
        .stdin(Stdio::null())
        .stdout(out)
        .stderr(err)
        .process_group(0)
        .spawn()
        .map_err(|e| format!("spawn: {e}"))?;
    println!("detached pid {} — log {} — done file {}", child.id(), log.display(), done.display());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn a(s: &str) -> Vec<String> {
        s.split_whitespace().map(String::from).collect()
    }

    #[test]
    fn the_first_ghost_is_the_run_and_the_rest_are_opponents() {
        let o = parse(&a("--map m.Map.Gbx --name x --outdir /mnt/c/d --mapdir /r/1-m run.Ghost.Gbx opp.Ghost.Gbx")).unwrap();
        assert_eq!(o.ghosts, vec!["run.Ghost.Gbx", "opp.Ghost.Gbx"]);
        assert_eq!(o.cam, 2);
        assert!(o.ship);
    }

    #[test]
    fn shipping_needs_a_mapdir_and_no_ship_does_not() {
        assert!(parse(&a("--map m --name x --outdir /mnt/c/d g")).is_err());
        assert!(parse(&a("--map m --name x --outdir /mnt/c/d --no-ship g")).is_ok());
    }

    #[test]
    fn a_looked_at_render_is_resumed_without_the_game() {
        let o = parse(&a("--from-webm /mnt/c/x/r.webm --name x --outdir /mnt/c/d --mapdir /r/1-m --offset-ms 0 g.Ghost.Gbx")).unwrap();
        assert_eq!(o.from_webm.as_deref(), Some("/mnt/c/x/r.webm"));
        assert_eq!(o.offset_ms, Some(0));
        assert!(o.map.is_empty(), "--map is not needed when the render exists");
        assert!(parse(&a("--name x --outdir /mnt/c/d --no-ship g")).is_err(), "no map and no webm is nothing to film");
    }

    #[test]
    fn a_clip_name_is_a_file_name() {
        assert!(parse(&a("--map m --name a/b --outdir /mnt/c/d --no-ship g")).is_err());
        assert!(parse(&a("--map m --name c3_tas7627-v2 --outdir /mnt/c/d --no-ship g")).is_ok());
    }

    #[test]
    fn the_done_file_is_read_not_assumed() {
        let (w, b, s) = parse_done_render("OK /mnt/c/x/y.webm 12210918 7.600\n").unwrap();
        assert_eq!((w.as_str(), b), ("/mnt/c/x/y.webm", 12210918));
        assert!((s - 7.6).abs() < 1e-9);
        assert!(parse_done_render("FAILED after 30s: setup failed").is_err());
        assert!(parse_done_render("").is_err());
    }
}
