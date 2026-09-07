//! `shootctl shootset` — every comparison view of ONE map for the price of ONE
//! editor load.
//!
//! The tiny-campaign QA loop used to open the map once per view (`cmpshot.sh`:
//! two loads per view, ~105 s each on 2026-09-06). Here the map is opened once,
//! the free camera is aimed through the probe plugin's `cam.txt` for every row
//! of a views file, and one screenshot is taken per row. Both sides of a
//! comparison (`--side o` for the original, `--side t` for the tiny copy, whose
//! camera is the original's mapped through the tiny transform at half the
//! distance) write into the same directory under the names `cmpviews.sh`
//! produced, so everything downstream (`tinyctl compare`, the ffmpeg hstack)
//! keeps working.
//!
//! Views file (`cmpviews.sh` format, tab-separated, `#` comments):
//!
//! ```text
//! NAME    ox,oy,oz    DIST    H    V
//! ```
//!
//! target (source-map world coordinates), orbital distance, HAngle, VAngle —
//! RADIANS, the editor's own: h=0 puts the camera north of the target looking
//! south, v>0 looks down, v≈1.3 is top-down.
//!
//! `--detach` re-runs the command in the background with its output in
//! `OUTDIR/shootset-SIDE.log` and returns at once: the WhiteStick bridge cuts a
//! command at ~90 s and a ten-view set takes minutes. `OUTDIR/done-SIDE.txt`
//! appears when the set is finished (success or failure — the file says which),
//! which is what the driver on the other side of the bridge polls for.
//!
//! Nothing here sleeps for its own sake: the map load is a wait on the game's
//! context, the probe is a wait on the plugin's answer file, and the only fixed
//! delay is the camera settling before the capture (the plugin applies the
//! camera on the next frame; the editor eases to it).

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

const STORE: &str = "/mnt/c/Users/vjeux/OpenplanetNext";
const MAPS_SHOOT: &str = "/mnt/c/Users/vjeux/OneDrive/Documents/Trackmania/Maps/_shoot";
const POWERSHELL: &str = "/mnt/c/Windows/System32/WindowsPowerShell/v1.0/powershell.exe";
const SHOTDPI: &str = "C:\\Users\\vjeux\\shotdpi.ps1";

#[derive(Clone, Debug)]
pub struct View {
    pub name: String,
    pub target: [f32; 3],
    pub dist: f32,
    pub h: f32,
    pub v: f32,
}

/// `sx,sy,sz:tx,ty,tz` — the source spawn and where `tmmaps tiny` put it.
#[derive(Clone, Copy, Debug)]
pub struct Anchor {
    pub src: [f32; 3],
    pub dst: [f32; 3],
    pub scale: f32,
}

impl Anchor {
    pub fn parse(s: &str, scale: f32) -> Result<Anchor, String> {
        let (a, b) = s.split_once(':').ok_or_else(|| format!("--anchor wants sx,sy,sz:tx,ty,tz, got `{s}`"))?;
        Ok(Anchor { src: vec3(a)?, dst: vec3(b)?, scale })
    }
    pub fn map(&self, p: [f32; 3]) -> [f32; 3] {
        [
            self.dst[0] + (p[0] - self.src[0]) * self.scale,
            self.dst[1] + (p[1] - self.src[1]) * self.scale,
            self.dst[2] + (p[2] - self.src[2]) * self.scale,
        ]
    }
}

fn vec3(s: &str) -> Result<[f32; 3], String> {
    let v: Result<Vec<f32>, _> = s.split(',').map(|x| x.trim().parse::<f32>()).collect();
    match v {
        Ok(v) if v.len() == 3 => Ok([v[0], v[1], v[2]]),
        _ => Err(format!("`{s}` is not x,y,z")),
    }
}

pub fn read_views(path: &Path) -> Result<Vec<View>, String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let mut out = Vec::new();
    for (n, line) in text.lines().enumerate() {
        let line = line.trim_end();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let f: Vec<&str> = line.split('\t').collect();
        if f.len() < 5 {
            return Err(format!("{}:{}: want NAME<TAB>x,y,z<TAB>DIST<TAB>H<TAB>V", path.display(), n + 1));
        }
        let num = |s: &str, what: &str| s.trim().parse::<f32>().map_err(|_| format!("{}:{}: {what} `{s}` is not a number", path.display(), n + 1));
        out.push(View {
            name: f[0].trim().to_string(),
            target: vec3(f[1]).map_err(|e| format!("{}:{}: {e}", path.display(), n + 1))?,
            dist: num(f[2], "distance")?,
            h: num(f[3], "h angle")?,
            v: num(f[4], "v angle")?,
        });
    }
    if out.is_empty() {
        return Err(format!("{}: no views", path.display()));
    }
    Ok(out)
}

pub struct Opts {
    pub map: String,
    pub views: PathBuf,
    pub side: String,
    pub tag: String,
    pub outdir: PathBuf,
    pub anchor: Option<Anchor>,
    pub load_timeout_s: u64,
    pub settle_ms: u64,
    pub lock: bool,
    pub detach: bool,
}

pub fn parse_opts(args: &[String]) -> Result<Opts, String> {
    let val = |k: &str| -> Option<String> { args.iter().position(|a| a == k).and_then(|i| args.get(i + 1)).cloned() };
    let map = val("--map").ok_or("shootset needs --map <path>")?;
    let views = PathBuf::from(val("--views").ok_or("shootset needs --views <VIEWS.tsv>")?);
    let side = val("--side").unwrap_or_else(|| "o".into());
    if side != "o" && side != "t" {
        return Err(format!("--side is o (original) or t (tiny), got `{side}`"));
    }
    let outdir = PathBuf::from(val("--outdir").ok_or("shootset needs --outdir <dir under /mnt/c>")?);
    if !outdir.starts_with("/mnt/") {
        return Err(format!("--outdir {} must live under /mnt/<drive>/ — the screenshot is taken by a Windows program", outdir.display()));
    }
    let scale: f32 = val("--scale").map(|s| s.parse().map_err(|_| "--scale number")).transpose()?.unwrap_or(0.5);
    let anchor = match val("--anchor") {
        Some(a) => Some(Anchor::parse(&a, scale)?),
        None => None,
    };
    if side == "t" && anchor.is_none() {
        return Err("--side t needs --anchor sx,sy,sz:tx,ty,tz (the line `tmmaps tiny` printed)".into());
    }
    let num = |k: &str, d: u64| -> Result<u64, String> { val(k).map(|s| s.parse::<u64>().map_err(|_| format!("{k} wants a number"))).transpose().map(|o| o.unwrap_or(d)) };
    Ok(Opts {
        map,
        views,
        side,
        tag: val("--tag").unwrap_or_default(),
        outdir,
        anchor,
        load_timeout_s: num("--load-timeout", 420)?,
        settle_ms: num("--settle-ms", 5000)?,
        lock: !args.iter().any(|a| a == "--no-lock"),
        detach: args.iter().any(|a| a == "--detach"),
    })
}

pub fn run(args: &[String]) -> i32 {
    let opts = match parse_opts(args) {
        Ok(o) => o,
        Err(e) => {
            eprintln!("{e}");
            eprintln!("usage: shootctl shootset --map MAP --views VIEWS.tsv --side o|t --outdir /mnt/c/... [--tag T] [--anchor sx,sy,sz:tx,ty,tz] [--scale 0.5] [--load-timeout S] [--settle-ms MS] [--no-lock] [--detach]");
            return 2;
        }
    };
    if let Err(e) = std::fs::create_dir_all(&opts.outdir) {
        eprintln!("{}: {e}", opts.outdir.display());
        return 2;
    }
    let done = opts.outdir.join(format!("done-{}.txt", opts.side));
    let _ = std::fs::remove_file(&done);
    if opts.detach {
        return detach(&opts);
    }
    let t0 = Instant::now();
    let result = run_set(&opts, t0);
    let summary = match &result {
        Ok(lines) => format!("OK {} views in {:.0}s\n{}\n", lines.len(), t0.elapsed().as_secs_f64(), lines.join("\n")),
        Err(e) => format!("FAILED after {:.0}s: {e}\n", t0.elapsed().as_secs_f64()),
    };
    print!("{summary}");
    // The done file is the contract with the driver across the bridge: it is
    // written LAST, after every screenshot exists, and it says what happened.
    let tmp = opts.outdir.join(format!("done-{}.tmp", opts.side));
    if std::fs::write(&tmp, &summary).and_then(|_| std::fs::rename(&tmp, &done)).is_err() {
        eprintln!("could not write {}", done.display());
        return 1;
    }
    if result.is_ok() { 0 } else { 1 }
}

/// Re-run this very command without `--detach`, its output in the set's log,
/// in its own process group so the bridge's timeout cannot take it down.
fn detach(opts: &Opts) -> i32 {
    use std::os::unix::process::CommandExt;
    let exe = match std::env::current_exe() {
        Ok(e) => e,
        Err(e) => {
            eprintln!("current_exe: {e}");
            return 1;
        }
    };
    let args: Vec<String> = std::env::args().skip(1).filter(|a| a != "--detach").collect();
    let log = opts.outdir.join(format!("shootset-{}.log", opts.side));
    let out = match std::fs::File::create(&log) {
        Ok(f) => f,
        Err(e) => {
            eprintln!("{}: {e}", log.display());
            return 1;
        }
    };
    let err = out.try_clone().expect("clone log handle");
    match std::process::Command::new(exe)
        .args(&args)
        .stdin(std::process::Stdio::null())
        .stdout(out)
        .stderr(err)
        .process_group(0)
        .spawn()
    {
        Ok(child) => {
            println!("detached pid {} — log {} — done file {}", child.id(), log.display(), opts.outdir.join(format!("done-{}.txt", opts.side)).display());
            0
        }
        Err(e) => {
            eprintln!("spawn: {e}");
            1
        }
    }
}

fn run_set(opts: &Opts, t0: Instant) -> Result<Vec<String>, String> {
    let el = || format!("[{:6.1}s]", t0.elapsed().as_secs_f64());
    let views = read_views(&opts.views)?;
    println!("{} {} views from {} (side {}, tag `{}`)", el(), views.len(), opts.views.display(), opts.side, opts.tag);

    // ONE GAME, ONE DRIVER — the same lock `shootctl run` takes. Held for the
    // whole set; released on every exit path below through `_lock`.
    let _lock = if opts.lock {
        let d = super::lock::lock_dir();
        let owner = format!("shootset-{}-{}", opts.tag, opts.side);
        super::lock::acquire(&d, &owner, 600, 0).map_err(|e| format!("lock: {e}"))?;
        Some(LockGuard { dir: d, owner })
    } else {
        None
    };

    // The map must sit under Documents/Trackmania — anywhere else EditMap
    // answers "ok" and loads nothing (measured 2026-08-26). Stage a copy there
    // when it is not already.
    let staged = stage_map(&opts.map)?;
    let game_map = super::game_path(&staged)?;
    let want_uid = super::map_uid(&staged);
    println!("{} map {} (uid {})", el(), game_map, want_uid.clone().unwrap_or_else(|| "?".into()));

    // Game up? `launch` no-ops when the plugin answers.
    if super::launch(180, false) != 0 {
        return Err("the game did not come up".into());
    }
    super::to_menu()?;
    super::await_cond("ready", 60)?;

    // Open the map. A FrameAskYesNo on the way in is the "missing items —
    // load anyway?" prompt: answer yes and remember that it happened, because
    // a map that needed it is a map the game is not showing whole.
    let _ = std::fs::create_dir_all(format!("{STORE}/PluginStorage/GhostShooter"));
    std::fs::write(format!("{STORE}/PluginStorage/GhostShooter/editmap.txt"), &game_map).map_err(|e| format!("editmap.txt: {e}"))?;
    let _ = std::fs::remove_file(format!("{STORE}/probe.txt"));
    let _ = std::fs::remove_file(format!("{STORE}/probe-out.tsv"));
    println!("{} /editmap: {}", el(), super::http_get("/editmap", 30).unwrap_or_default().trim());
    let mut dialogs = Vec::new();
    let load0 = Instant::now();
    loop {
        if load0.elapsed().as_secs() > opts.load_timeout_s {
            return Err(format!("the map did not open in {} s; last ctx {}", opts.load_timeout_s, super::http_get("/ctx", 10).unwrap_or_default().trim()));
        }
        if !super::tm_running() {
            return Err("the game process is gone — the map crashed the client".into());
        }
        let c = super::http_get("/ctx", 10).unwrap_or_default();
        if c.contains("\"ctx\":1") {
            break;
        }
        if c.contains("FrameAskYesNo") {
            let text = super::http_get("/dlgtext", 10).unwrap_or_default().trim().to_string();
            println!("{} DIALOG {}", el(), text);
            dialogs.push(text);
            let _ = super::http_get("/yes", 10);
            let _ = super::await_cond("nodialog", 5);
            let _ = super::http_get("/yes", 10);
        }
        std::thread::sleep(Duration::from_millis(1500));
    }
    println!("{} editor open after {:.1}s", el(), load0.elapsed().as_secs_f64());
    if let (Some(w), Some(h)) = (&want_uid, super::loaded_uid()) {
        if *w != h {
            return Err(format!("the editor opened uid {h}, not the {w} we asked for"));
        }
    }

    // The editor opens with its own camera fly-in; a camera written under it
    // is overridden by the animation (the first smoke test shot the lake,
    // not the gate). Let it finish before the first view.
    std::thread::sleep(Duration::from_millis(opts.settle_ms.max(4000)));
    let mut lines = Vec::new();
    if !dialogs.is_empty() {
        lines.push(format!("dialogs\t{}", dialogs.join(" | ")));
    }
    for (i, v) in views.iter().enumerate() {
        let (target, dist) = match (&opts.side[..], &opts.anchor) {
            ("t", Some(a)) => (a.map(v.target), v.dist * a.scale),
            _ => (v.target, v.dist),
        };
        let cam = format!("{:.2},{:.2},{:.2},{:.2},{:.4},{:.4}", target[0], target[1], target[2], dist, v.h, v.v);
        // cam.txt is read by the probe plugin every frame and applied when it
        // changes; the nonce makes two identical cameras in a row still count.
        let aim = |nonce: &str| -> Result<(), String> {
            let tmp = format!("{STORE}/cam.tmp");
            std::fs::write(&tmp, format!("{cam},{}-{nonce}", std::process::id())).map_err(|e| format!("cam.txt: {e}"))?;
            std::fs::rename(&tmp, format!("{STORE}/cam.txt")).map_err(|e| format!("cam.txt: {e}"))
        };
        aim(&format!("{i}a"))?;
        // Probe: what did the game keep? (a silently dropped item shows here)
        let kept = probe(i)?;
        // Aim again: the orbital camera eases towards its target and anything
        // still animating in the editor can steal the first write; a second
        // identical camera (new nonce) re-applies it, then the settle.
        aim(&format!("{i}b"))?;
        std::thread::sleep(Duration::from_millis(opts.settle_ms));
        let file = opts.outdir.join(format!("cmp-{}{}-{}.png", opts.tag, v.name, opts.side));
        let _ = std::fs::remove_file(&file);
        screenshot(&file)?;
        let size = std::fs::metadata(&file).map(|m| m.len()).unwrap_or(0);
        if size == 0 {
            return Err(format!("{}: the screenshot is empty", file.display()));
        }
        let line = format!("{}\t{}\t{}\t{}\t{}", v.name, opts.side, cam, kept, size);
        println!("{} {line}", el());
        lines.push(line);
    }
    Ok(lines)
}

struct LockGuard {
    dir: PathBuf,
    owner: String,
}
impl Drop for LockGuard {
    fn drop(&mut self) {
        let _ = super::lock::release(&self.dir, &self.owner);
    }
}

/// Copy the map under `Maps/_shoot/` unless it is already there. The copy goes
/// through a plain write so a OneDrive lock on a chunk file cannot leave a
/// half-written map behind under a good name.
fn stage_map(map: &str) -> Result<String, String> {
    let wsl = if let Some(rest) = map.strip_prefix("C:/") { format!("/mnt/c/{rest}") } else { map.to_string() };
    if wsl.starts_with(MAPS_SHOOT) {
        return Ok(wsl);
    }
    let name = Path::new(&wsl).file_name().and_then(|n| n.to_str()).ok_or_else(|| format!("{map}: no file name"))?;
    let dst = format!("{MAPS_SHOOT}/{name}");
    let data = std::fs::read(&wsl).map_err(|e| format!("{wsl}: {e}"))?;
    std::fs::create_dir_all(MAPS_SHOOT).map_err(|e| format!("{MAPS_SHOOT}: {e}"))?;
    let tmp = format!("{MAPS_SHOOT}/.{name}.tmp");
    std::fs::write(&tmp, &data).and_then(|_| std::fs::rename(&tmp, &dst)).map_err(|e| format!("{dst}: {e}"))?;
    let back = std::fs::read(&dst).map_err(|e| format!("{dst}: {e}"))?;
    if back != data {
        return Err(format!("{dst}: the staged copy does not match the source ({} vs {} bytes)", back.len(), data.len()));
    }
    Ok(dst)
}

/// Ask the probe plugin for the editor's item/block census and wait for its
/// answer file. Returns `items=N blocks=M loaded=K` (K = items whose model
/// the game actually loaded).
fn probe(nonce: usize) -> Result<String, String> {
    let out = format!("{STORE}/probe-out.tsv");
    let _ = std::fs::remove_file(&out);
    std::fs::write(format!("{STORE}/probe.txt"), format!("p{}-{nonce}", std::process::id())).map_err(|e| format!("probe.txt: {e}"))?;
    let t0 = Instant::now();
    while t0.elapsed().as_secs() < 40 {
        std::thread::sleep(Duration::from_millis(500));
        if let Ok(text) = std::fs::read_to_string(&out) {
            // the plugin writes the file in one go, but a read can still land
            // mid-write: accept it only once a second read agrees
            std::thread::sleep(Duration::from_millis(300));
            let again = std::fs::read_to_string(&out).unwrap_or_default();
            if again != text {
                continue;
            }
            if text.ends_with('\n') && text.contains("\nblocks\t") {
                let mut items = 0;
                let mut loaded = 0;
                let mut blocks = 0;
                for l in text.lines() {
                    let f: Vec<&str> = l.split('\t').collect();
                    match f[0] {
                        "item" => {
                            items += 1;
                            if f.get(6) == Some(&"1") {
                                loaded += 1;
                            }
                        }
                        "block" => blocks += 1,
                        _ => {}
                    }
                }
                return Ok(format!("items={items} loaded={loaded} blocks={blocks}"));
            }
        }
    }
    Ok("probe=timeout".into())
}

/// One DPI-aware capture of the whole screen into `file` (a WSL path under
/// /mnt/c, handed to PowerShell as `C:\...`).
fn screenshot(file: &Path) -> Result<(), String> {
    let win = super::game_path(file.to_str().ok_or("screenshot path is not utf-8")?)?.replace('/', "\\");
    let status = std::process::Command::new(POWERSHELL)
        .args(["-ExecutionPolicy", "Bypass", "-File", SHOTDPI, &win])
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::piped())
        .output()
        .map_err(|e| format!("powershell: {e}"))?;
    if !status.status.success() {
        return Err(format!("shotdpi.ps1 failed: {}", String::from_utf8_lossy(&status.stderr).trim()));
    }
    // PowerShell returns when the file is closed; make sure it is there.
    let t0 = Instant::now();
    while t0.elapsed().as_secs() < 10 {
        if std::fs::metadata(file).map(|m| m.len() > 0).unwrap_or(false) {
            return Ok(());
        }
        std::thread::sleep(Duration::from_millis(200));
    }
    Err(format!("{}: not written", file.display()))
}
