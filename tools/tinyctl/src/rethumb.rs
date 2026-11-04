//! `tinyctl rethumb` — new thumbnails for LIVE maps, from the devserver, end to end:
//!
//! ```text
//! tinyctl rethumb --uids-file U.tsv --work DIR [--token-dir /tmp/tinyctl-tokens]
//!                 [--no-promote] [--publish] [--jobs 4] [--resume]
//! ```
//!
//! `U.tsv`: `uid<TAB>name` per live map (an inventory slice). Per map:
//!
//! 1. the live file (`nadeo-here maps --fetch`, skipped when `DIR/live/<uid>.Map.Gbx` exists);
//! 2. a RENDER COPY with the game's generated records promoted to authored blocks
//!    (`tmmaps promote-baked`): the editor rebuilds pillars, stage bases and wall clips
//!    from the placed blocks when it loads a map, and a piece whose parent became one of
//!    our items never comes back — the September thumbnails lost them (Everios96
//!    2026-11-02, "parts of blocks missing, like on 39") while play mode draws the saved
//!    pieces as they are. Promoted, the editor renders them too. `--no-promote` renders
//!    the live file itself;
//! 3. the render copies pushed to the box, `tinyctl thumbs --editor` run there under
//!    `tmdrive` (the game's own thumbnail camera, square, no UI; ~7 s a map);
//! 4. the new JPEG lifted from each rendered copy and spliced into the LIVE file
//!    (`tmmaps set-thumbnail`) → `DIR/final/<name>.Map.Gbx`: the shipped bytes are the
//!    live map's, thumbnail aside (checked: the body hash is the live file's);
//! 5. with `--publish`: `publish-batch` UPDATE in place (same uid — records kept; no
//!    playlist touched), results in `DIR/results.tsv`, every row expected IDENTICAL.
//!
//! The box binaries are `/home/vjeux/bin/tinyctl-fix4` and `tmmaps-fix4` (this tree's
//! builds; the box repo is another branch). Nothing here wipes, renames or re-ids a map.
use crate::wsx::Wsx;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Instant;

/// The box-side binaries: `RETHUMB_BOX_BIN=<dir>` (tinyctl + tmmaps built ON the box from
/// the same commit — `/home/vjeux/bin/tt5` since 2026-11-04, when the relay's upstream could
/// no longer carry a 10 MB binary in reasonable time), else the Nov-2 pushed copies.
fn box_tinyctl() -> String {
    std::env::var("RETHUMB_BOX_BIN").ok().filter(|v| !v.is_empty()).map(|d| format!("{d}/tinyctl")).unwrap_or_else(|| "/home/vjeux/bin/tinyctl-fix4".into())
}
fn box_tmmaps() -> String {
    std::env::var("RETHUMB_BOX_BIN").ok().filter(|v| !v.is_empty()).map(|d| format!("{d}/tmmaps")).unwrap_or_else(|| "/home/vjeux/bin/tmmaps-fix4".into())
}
/// The box-side scratch lives on the WINDOWS disk (DrvFs), not inside the WSL VHDX: four
/// WSL deaths on 2026-11-03 were host-side VHDX write failures (`hv_storvsc … cmd 0x2a …
/// host 0xc00000a1` = STATUS_WORKING_SET_QUOTA) that hit exactly when a chunk's tar landed;
/// an NTFS directory takes the same bytes without touching the VHDX at all.
const BOX_ROOT: &str = "/mnt/c/Users/vjeux/rethumb-work";

fn tool(name: &str) -> PathBuf {
    std::env::current_exe().ok().and_then(|p| p.parent().map(|d| d.join(name))).filter(|p| p.exists()).unwrap_or_else(|| PathBuf::from(name))
}

/// The header thumbnail's JPEG bytes, as stored.
fn thumbnail_of(p: &Path) -> Result<Vec<u8>, String> {
    let bytes = std::fs::read(p).map_err(|e| format!("{}: {e}", p.display()))?;
    let start = bytes.windows(15).position(|w| w == b"<Thumbnail.jpg>").ok_or_else(|| format!("{}: no <Thumbnail.jpg> in the header", p.display()))? + 15;
    let end = start + bytes[start..].windows(16).position(|w| w == b"</Thumbnail.jpg>").ok_or_else(|| format!("{}: no </Thumbnail.jpg>", p.display()))?;
    Ok(bytes[start..end].to_vec())
}

/// The bytes after the header (the body, compressed as stored): what must not change.
fn body_hash(p: &Path) -> Result<String, String> {
    let bytes = std::fs::read(p).map_err(|e| format!("{}: {e}", p.display()))?;
    let end = bytes.windows(16).position(|w| w == b"</Thumbnail.jpg>").ok_or("no thumbnail end")? + 16;
    Ok(crate::batch::md5_hex(&bytes[end..]))
}

pub fn cmd(args: &[String]) -> Result<(), String> {
    let f = |k: &str| tmmaps::cli::flag(args, k).map(String::from);
    let uids_file = PathBuf::from(f("--uids-file").ok_or("rethumb needs --uids-file U.tsv (uid<TAB>name)")?);
    let work = PathBuf::from(f("--work").ok_or("rethumb needs --work DIR")?);
    let token_dir = f("--token-dir").unwrap_or_else(|| "/tmp/tinyctl-tokens".into());
    let promote = !tmmaps::cli::has(args, "--no-promote");
    // --orig-root: the ORIGINAL maps (`<root>/p<P>/<NN>-U10S_<N>.Map.Gbx`, the 2026-10-09 bank):
    // the render copy puts a converted block's original back so the editor regenerates its
    // pillars and bases (`tmmaps render-copy`); the game refuses promoted clip records, so
    // `promote-baked` is not the way (tested 2026-11-02 on Plastic 39)
    let orig_root = PathBuf::from(f("--orig-root").unwrap_or_else(|| "/home/vjeux/tm-bank/u10s-originals-2026-10-09".into()));
    let publish = tmmaps::cli::has(args, "--publish");
    let jobs = f("--jobs").unwrap_or_else(|| "4".into());
    let t0 = Instant::now();
    let rows: Vec<(String, String)> = std::fs::read_to_string(&uids_file)
        .map_err(|e| format!("{}: {e}", uids_file.display()))?
        .lines()
        .filter(|l| !l.trim().is_empty() && !l.starts_with('#'))
        .map(|l| {
            let mut it = l.split('\t');
            (it.next().unwrap_or("").trim().to_string(), it.next().unwrap_or("").trim().to_string())
        })
        .filter(|(u, n)| !u.is_empty() && !n.is_empty())
        .collect();
    if rows.is_empty() {
        return Err("rethumb: no uid rows".into());
    }
    let live_dir = work.join("live");
    let render_dir = work.join("render");
    let final_dir = work.join("final");
    for d in [&live_dir, &render_dir, &final_dir] {
        std::fs::create_dir_all(d).map_err(|e| format!("{}: {e}", d.display()))?;
    }
    let tinyctl = std::env::current_exe().map_err(|e| e.to_string())?;
    let tmmaps = tool("tmmaps");

    // 1+2. the live files and the render copies (the same code the box runs in `box-prep`)
    prep(&rows, &live_dir, &render_dir, &orig_root, promote, &tinyctl, &tmmaps, &token_dir, &work, &t0)?;

    // 3. the box: push + the editor renders (resumable on the box side)
    let wsx = Wsx::new(args);
    // THIS RUN IS NOT A PERSON: logged in the host's box-run log with its heartbeat, so the
    // devserver gate (`box-idle`, rethumb-all) measures idle from the last HUMAN input before
    // the run instead of our own launch/render input (the mechanism every shoot/play run
    // uses; the rethumb chain had skipped it — Everios96 2026-11-02 22:22: "this happens way
    // too often that you detect yourself")
    if std::env::var("RETHUMB_RUN_REGISTERED").is_err() {
        let idle_now = Command::new(&tinyctl).args(["box-idle", "--window-s", "60"]).output().ok().map(|o| String::from_utf8_lossy(&o.stdout).to_string()).unwrap_or_default();
        let idle_at_start = idle_now.split_whitespace().find_map(|w| w.strip_prefix("raw_kbm_idle_s=").or_else(|| w.strip_prefix("kbm_idle_s="))).and_then(|v| v.parse::<f64>().ok()).unwrap_or(0.0);
        crate::inputidle::runlog::start(&format!("tinyctl rethumb {}", work.display()), idle_at_start);
    }
    // the box dir is named after SET and chunk (`dirt-w006`): chunk dirs repeat their names
    // across sets (every set has a w000) and the box keeps one scratch tree for all of them
    let batch = match (work.parent().and_then(|p| p.file_name()), work.file_name()) {
        (Some(set), Some(chunk)) => format!("{}-{}", set.to_string_lossy(), chunk.to_string_lossy()),
        (_, Some(chunk)) => chunk.to_string_lossy().to_string(),
        _ => "batch".into(),
    };
    let remote = format!("{BOX_ROOT}/{batch}");
    wsx.sh(&format!("mkdir -p {remote}/in {remote}/out"))?;
    // RETHUMB_BOX_PREP=<dir of tinyctl+tmmaps on the box>: the box makes the render copies
    // itself (`box-prep`: fetches the LIVE files from Nadeo over its own internet, originals
    // from /home/vjeux/rollout/orig) — only the uid list and the two token files travel up.
    // The relay's upstream fell to ~10 KB/s on 2026-11-04 (a 50 MB tar = 85 min, pushes
    // timing out); downstream (pull) stayed at ~1 MB/s.
    let box_prep_bin = std::env::var("RETHUMB_BOX_PREP").ok().filter(|v| !v.is_empty()).map(|v| if v == "1" { std::env::var("RETHUMB_BOX_BIN").unwrap_or_else(|_| "/home/vjeux/bin/tt5".into()) } else { v });
    let mut prep_cmd = String::new();
    if let Some(bin) = &box_prep_bin {
        let uids_tsv = work.join("box-uids.tsv");
        std::fs::write(&uids_tsv, rows.iter().map(|(u, n)| format!("{u}\t{n}\n")).collect::<String>()).map_err(|e| e.to_string())?;
        wsx.push(&uids_tsv, &format!("{remote}/box-uids.tsv"))?;
        wsx.sh(&format!("mkdir -p {remote}/tokens"))?;
        for aud in ["NadeoServices", "NadeoLiveServices"] {
            wsx.push(Path::new(&format!("{token_dir}/token-{aud}.txt")), &format!("{remote}/tokens/token-{aud}.txt"))?;
        }
        let np = if promote { "" } else { " --no-promote" };
        prep_cmd = format!("{bin}/tinyctl box-prep --uids-file {remote}/box-uids.tsv --live {remote}/live --render {remote}/in --token-dir {remote}/tokens{np} > {remote}/prep.log 2>&1 && ls {remote}/in/*.Map.Gbx > {remote}/list.tsv && ");
        println!("[{:>4.0}s] box-prep mode: {} uids + tokens pushed; the box fetches and copies", t0.elapsed().as_secs_f64(), rows.len());
    } else {
        let tar = work.join("render.tar");
        let st = Command::new("tar").args(["cf", &tar.to_string_lossy(), "-C", &render_dir.to_string_lossy(), "."]).status().map_err(|e| e.to_string())?;
        if !st.success() {
            return Err("tar of the render copies failed".into());
        }
        wsx.push(&tar, &format!("{remote}/render.tar"))?;
        wsx.sh(&format!("cd {remote}/in && tar xf {remote}/render.tar && rm -f {remote}/render.tar && ls {remote}/in/*.Map.Gbx > {remote}/list.tsv && wc -l < {remote}/list.tsv"))?;
        println!("[{:>4.0}s] pushed {} render copies to {remote}/in", t0.elapsed().as_secs_f64(), rows.len());
    }
    let env = crate::remoteshots::ENV;
    let (bt, bm) = (box_tinyctl(), box_tmmaps());
    let mut attempt = 0;
    loop {
        attempt += 1;
        // the game, if not up, as its own lock step; then the loop under the lock
        // no game → launch it (tmdrive reclaims our own stale hold; a foreign hold makes it wait)
        let up = wsx.sh("/mnt/c/Windows/System32/tasklist.exe | grep -ci trackmania.exe").map(|t| t.trim() != "0").unwrap_or(false);
        let mut launch_ago = 0u64;
        if !up {
            let tl = Instant::now();
            let l = wsx.sh(&format!("{env}; tmdrive launch --timeout 180 --purpose rethumb --wait 100 2>&1 | tail -1")).unwrap_or_default();
            println!("[{:>4.0}s] launch: {}", t0.elapsed().as_secs_f64(), l.trim().chars().take(100).collect::<String>());
            launch_ago = tl.elapsed().as_secs() + 1;
        }
        // the launch's own input (Alt press, pointer warps) is not a person's: the box run
        // gets the launch moment, so only input NEWER than it stops the run.
        // DETACHED: a bridge call is capped at 150 s and a 40-map render is ~10 min — the
        // in-call form timed out, the driver counted a failure and started a SECOND run while
        // the first kept rendering: two runs fought over the game (18 stragglers in one chunk)
        // and the orphan's own input read as a person (2026-11-02 22:19, Everios96: "are you
        // sure you're not detecting yourself again?"). One run at a time: nohup + a done file.
        let done_file = format!("{remote}/run.done");
        let _ = wsx.sh(&format!("rm -f {done_file}; pgrep -f 'tinyctl.* thumbs --editor --resume --list {remote}/list.tsv' >/dev/null && echo STILL-RUNNING"));
        let run = format!(
            "{env}; export THUMBS_LAUNCH_AGO_S={launch_ago}; cd {remote} && nohup sh -c '{prep_cmd}tmdrive run --purpose \"rethumb {batch}\" --wait 100 -- {bt} thumbs --editor --resume --list {remote}/list.tsv --outdir {remote}/out --tmmaps {bm} > {remote}/run.log 2>&1; echo OK $? > {done_file}' > /dev/null 2>&1 & echo started"
        );
        let started = wsx.sh(&run).unwrap_or_else(|e| format!("bridge: {e}"));
        if !started.contains("started") {
            println!("[{:>4.0}s] box: could not start the run: {}", t0.elapsed().as_secs_f64(), started.trim().chars().take(120).collect::<String>());
        }
        // one probe every 30 s (the fleet's bridge budget), up to 40 min for a 40-map chunk
        let out = match wsx.wait_done(&done_file, &format!("{remote}/run.log"), std::time::Duration::from_secs(3600), "rethumb render") {
            Ok(_) => wsx.sh(&format!("tail -3 {remote}/run.log")).unwrap_or_default(),
            Err(e) => {
                // no done file in 40 min: the run is wedged — stop it so the next attempt is alone
                let _ = wsx.sh(&format!("pkill -f 'tinyctl.* thumbs --editor --resume --list {remote}/list.tsv'; pkill -f 'rethumb {batch}'; true"));
                format!("{e}")
            }
        };
        println!("[{:>4.0}s] box: {}", t0.elapsed().as_secs_f64(), out.lines().last().unwrap_or("").chars().take(140).collect::<String>());
        // a person took the box: close our game and hand the decision back to the caller
        if out.contains("a person is using the box") {
            let _ = Command::new(&tinyctl).args(["box-close-game", "--idle-s", "1"]).output();
            return Err("rethumb: HUMAN on the box — stopped; rerun when free (resumes)".into());
        }
        let done: usize = wsx.sh(&format!("ls {remote}/out/*.Map.Gbx 2>/dev/null | wc -l")).ok().and_then(|t| t.trim().parse().ok()).unwrap_or(0);
        if done >= rows.len() {
            break;
        }
        if attempt >= 4 {
            // a map the editor would not save in four runs (transient on the box: the same
            // file renders alone — 2026-11-02, Tarmac 53) is LEFT OUT of this batch, not a
            // reason to drop the other 39: the rest goes on, the stragglers are listed for a
            // later pass (`DIR/stragglers.tsv`)
            println!("[{:>4.0}s] {done}/{} rendered after {attempt} attempts — going on without the rest", t0.elapsed().as_secs_f64(), rows.len());
            break;
        }
        // the game restarted between attempts: a stuck editor is what the retries hit
        let _ = wsx.sh(&format!("{env}; tmdrive kill --purpose \"rethumb restart\" --wait 100 >/dev/null 2>&1; true"));
        std::thread::sleep(std::time::Duration::from_secs(5));
        println!("[{:>4.0}s] {done}/{} rendered — again (attempt {})", t0.elapsed().as_secs_f64(), rows.len(), attempt + 1);
    }
    // the rendered copies back (one tar)
    wsx.sh(&format!("cd {remote}/out && tar cf {remote}/out.tar *.Map.Gbx thumbs.tsv"))?;
    let back = work.join("out.tar");
    wsx.pull(&format!("{remote}/out.tar"), &back)?;
    let out_dir = work.join("rendered");
    std::fs::create_dir_all(&out_dir).map_err(|e| e.to_string())?;
    let st = Command::new("tar").args(["xf", &back.to_string_lossy(), "-C", &out_dir.to_string_lossy()]).status().map_err(|e| e.to_string())?;
    if !st.success() {
        return Err("untar of the rendered copies failed".into());
    }
    println!("[{:>4.0}s] pulled the rendered copies", t0.elapsed().as_secs_f64());

    // 4. splice each new JPEG into the LIVE file
    let mut manifest = String::new();
    let mut n_same = 0usize;
    let mut stragglers = String::new();
    let mut n_final = 0usize;
    for (u, n) in &rows {
        let rendered = out_dir.join(format!("{u}.Map.Gbx"));
        if !rendered.exists() {
            stragglers.push_str(&format!("{u}\t{n}\n"));
            continue;
        }
        n_final += 1;
        let jpeg = thumbnail_of(&rendered)?;
        let live = live_dir.join(format!("{u}.Map.Gbx"));
        if thumbnail_of(&live)? == jpeg {
            n_same += 1;
        }
        let jpg = work.join(format!("{u}.jpg"));
        std::fs::write(&jpg, &jpeg).map_err(|e| e.to_string())?;
        let fin = final_dir.join(format!("{n}.Map.Gbx"));
        let out = Command::new(&tmmaps).args(["set-thumbnail", &live.to_string_lossy(), "--jpeg", &jpg.to_string_lossy(), "--out", &fin.to_string_lossy()]).output().map_err(|e| e.to_string())?;
        if !out.status.success() || !fin.exists() {
            return Err(format!("set-thumbnail {n}: {}", String::from_utf8_lossy(&out.stderr).lines().last().unwrap_or("")));
        }
        let _ = std::fs::remove_file(&jpg);
        if body_hash(&fin)? != body_hash(&live)? {
            return Err(format!("{n}: the final file's body differs from the live file's — refusing"));
        }
        if thumbnail_of(&fin)? != jpeg {
            return Err(format!("{n}: the spliced thumbnail does not read back"));
        }
        manifest.push_str(&format!("{}\t{n}\n", fin.display()));
    }
    let man = work.join("manifest.tsv");
    std::fs::write(&man, &manifest).map_err(|e| e.to_string())?;
    let strag = work.join("stragglers.tsv");
    std::fs::write(&strag, &stragglers).map_err(|e| e.to_string())?;
    let n_strag = stragglers.lines().count();
    println!("[{:>4.0}s] final files: {n_final} (body = live, thumbnail new; {n_same} unchanged pictures; {n_strag} stragglers → {})", t0.elapsed().as_secs_f64(), strag.display());
    if n_final == 0 {
        return Err("rethumb: nothing rendered".into());
    }
    if !publish {
        println!("not published (--publish); manifest {}", man.display());
        return Ok(());
    }

    // 5. the in-place update
    let results = work.join("results.tsv");
    let st = Command::new(&tinyctl)
        .args(["publish-batch", "--manifest", &man.to_string_lossy(), "--results", &results.to_string_lossy(), "--no-campaign", "--files-only", "--token-dir", &token_dir, "--jobs", &jobs])
        .status()
        .map_err(|e| e.to_string())?;
    if !st.success() {
        return Err("publish-batch failed".into());
    }
    let text = std::fs::read_to_string(&results).unwrap_or_default();
    let mut by: HashMap<String, usize> = HashMap::new();
    for l in text.lines().skip(1) {
        *by.entry(l.split('\t').nth(7).unwrap_or("?").to_string()).or_default() += 1;
    }
    println!("[{:>4.0}s] published: {:?}", t0.elapsed().as_secs_f64(), by);
    if by.get("IDENTICAL").copied().unwrap_or(0) != n_final {
        return Err(format!("rethumb: {} of {} IDENTICAL — see {}", by.get("IDENTICAL").copied().unwrap_or(0), n_final, results.display()));
    }
    Ok(())
}

/// Steps 1–2 of a rethumb: fetch the LIVE files (`nadeo-here maps --fetch`, tokens from
/// `token_dir`) and make the render copies (`tmmaps render-copy` against the ORIGINAL, or a
/// plain copy with `--no-promote`). Runs on the devserver, or on the box as `tinyctl box-prep`
/// when the relay's upstream is too slow to push 50 MB of copies (2026-11-04: 10 KB/s).
#[allow(clippy::too_many_arguments)]
pub fn prep(rows: &[(String, String)], live_dir: &Path, render_dir: &Path, orig_root: &Path, promote: bool, tinyctl: &Path, tmmaps: &Path, token_dir: &str, work: &Path, t0: &Instant) -> Result<(), String> {
    std::fs::create_dir_all(live_dir).map_err(|e| e.to_string())?;
    std::fs::create_dir_all(render_dir).map_err(|e| e.to_string())?;
    let missing: Vec<&(String, String)> = rows.iter().filter(|(u, _)| !live_dir.join(format!("{u}.Map.Gbx")).exists()).collect();
    if !missing.is_empty() {
        let list = work.join("fetch.tsv");
        std::fs::write(&list, missing.iter().map(|(u, n)| format!("{u}\t{n}\n")).collect::<String>()).map_err(|e| e.to_string())?;
        let out = Command::new(tinyctl)
            .env("TINYCTL_TOKEN_DIR", token_dir)
            .args(["nadeo-here", "maps", "--uids-file", &list.to_string_lossy(), "--fetch", &live_dir.to_string_lossy(), "--no-lock"])
            .output()
            .map_err(|e| e.to_string())?;
        let text = String::from_utf8_lossy(&out.stdout);
        println!("[{:>4.0}s] fetch: {}", t0.elapsed().as_secs_f64(), text.lines().last().unwrap_or("").trim());
    }
    let still: Vec<&(String, String)> = rows.iter().filter(|(u, _)| !live_dir.join(format!("{u}.Map.Gbx")).exists()).collect();
    if !still.is_empty() {
        return Err(format!("rethumb: {} live file(s) missing after the fetch ({} …)", still.len(), still[0].0));
    }

    // 2. the render copies
    let mut n_prom = 0usize;
    for (u, n) in rows {
        let live = live_dir.join(format!("{u}.Map.Gbx"));
        let render = render_dir.join(format!("{u}.Map.Gbx"));
        if render.exists() {
            continue;
        }
        if promote {
            let num: usize = n.strip_prefix("U10S_").and_then(|x| x.split(' ').next()).and_then(|x| x.parse().ok()).ok_or_else(|| format!("{n}: no U10S_<N> number in the name"))?;
            let part = (num - 1) / 25 + 1;
            // the bank names: `01-U10S_01` … `25-U10S_25` (two digits below 100), `01-U10S_101` …
            let orig = orig_root.join(format!("p{part}")).join(format!("{:02}-U10S_{:02}.Map.Gbx", (num - 1) % 25 + 1, num));
            if !orig.exists() {
                return Err(format!("{n}: no original at {}", orig.display()));
            }
            let out = Command::new(tmmaps).args(["render-copy", &live.to_string_lossy(), "--orig", &orig.to_string_lossy(), "--out", &render.to_string_lossy()]).output().map_err(|e| e.to_string())?;
            if !out.status.success() || !render.exists() {
                return Err(format!("render-copy {u}: {}", String::from_utf8_lossy(&out.stderr).lines().last().unwrap_or("")));
            }
            n_prom += 1;
        } else {
            std::fs::copy(&live, &render).map_err(|e| e.to_string())?;
        }
    }
    println!("[{:>4.0}s] render copies: {} ({} made this run)", t0.elapsed().as_secs_f64(), rows.len(), n_prom);

    Ok(())
}

/// `tinyctl box-prep --uids-file L --live DIR --render DIR --orig-root R --token-dir D
/// [--no-promote]`: steps 1–2 on the box itself (its own internet fetches from Nadeo).
pub fn box_prep(args: &[String]) -> Result<(), String> {
    let f = |k: &str| tmmaps::cli::flag(args, k).map(String::from);
    let uids_file = PathBuf::from(f("--uids-file").ok_or("box-prep needs --uids-file L.tsv")?);
    let live_dir = PathBuf::from(f("--live").ok_or("box-prep needs --live DIR")?);
    let render_dir = PathBuf::from(f("--render").ok_or("box-prep needs --render DIR")?);
    let orig_root = PathBuf::from(f("--orig-root").unwrap_or_else(|| "/home/vjeux/rollout/orig".into()));
    let token_dir = f("--token-dir").ok_or("box-prep needs --token-dir DIR")?;
    let promote = !tmmaps::cli::has(args, "--no-promote");
    let rows: Vec<(String, String)> = std::fs::read_to_string(&uids_file)
        .map_err(|e| format!("{}: {e}", uids_file.display()))?
        .lines()
        .filter(|l| !l.trim().is_empty())
        .map(|l| {
            let mut it = l.split('\t');
            (it.next().unwrap_or("").trim().to_string(), it.next().unwrap_or("").trim().to_string())
        })
        .collect();
    let tinyctl = std::env::current_exe().unwrap_or_else(|_| PathBuf::from("tinyctl"));
    let tmmaps = tinyctl.parent().map(|d| d.join("tmmaps")).filter(|p| p.exists()).unwrap_or_else(|| PathBuf::from("tmmaps"));
    let work = live_dir.parent().map(Path::to_path_buf).unwrap_or_else(|| PathBuf::from("."));
    let t0 = Instant::now();
    prep(&rows, &live_dir, &render_dir, &orig_root, promote, &tinyctl, &tmmaps, &token_dir, &work, &t0)?;
    println!("box-prep: {} render copies in {}", rows.len(), render_dir.display());
    Ok(())
}
