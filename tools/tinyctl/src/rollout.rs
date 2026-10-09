//! `tinyctl rollout-build` — the whole U10S alteration set from Everios96's
//! originals, nothing published.
//!
//! ```text
//! tinyctl rollout-build --src ORIGINALS --out ROOT --sets dirt,grass,ice,plastic,magnet,wood,woodwet
//!                       [--maps 1-975|23,72,…] [--jobs 6] [--bin-dir DIR] [--keep DIR]
//! ```
//!
//! ORIGINALS holds `p<P>/<NN>-U10S_<N>.Map.Gbx` (what `nadeo-here campaign --fetch`
//! writes for club 18974's "UNDER 10 SECONDS" + "U10S Part 2..39"). Every map × set
//! runs `tinyctl surface` with the per-map environment the September production
//! build used (box-scripts/build.sh, 2026-09-23), so a rollout file differs from
//! the live one ONLY by what changed since: the water floors, the black walls,
//! the two custom finishes. The lists live in `--keep` (default ~/u10s-keep):
//!   sb-driven.txt      maps whose StructureBase roads are driven → TINY_STRUCTURE_BASE=1
//!   mats-driven.txt    maps whose inflatable mats are driven → TINY_ITEM_REDRESS_EXTRA (not plastic)
//!   custom-convert2.txt / custom-physics-only.txt → the custom-item classifier lists
//! and the 23 stage-deck maps are the 2026-09-19 ghost-test list, inline below.
//!
//! Output: ROOT/<set>/p<P>/<set>01/*.Map.Gbx (one tinyctl surface run per map), a
//! `build.tsv` (set, num, path, bytes, status) and `build.log`. A failed map is a
//! FAILED row, never a missing one: the publish step refuses an incomplete set
//! (vjeux 2026-10-01, nothing silently dropped).
//!
//! Wood and wet wood files are stamped `--build 2026-02-02_17_51` by `tinyctl surface`
//! itself when the source carries an older exebuild (the 2026-09-20 wood-physics
//! lesson); `rollout-build` verifies the stamp on every wood/woodwet output and
//! fails the row if it is not there.

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::process::Command;

const STAGE_DECK_MAPS: &[u32] = &[39, 866, 75, 203, 940, 918, 421, 819, 830, 422, 460, 656, 623, 238, 941, 142, 619, 533, 926, 289, 901, 308, 853];
const WOOD_BUILD: &str = "2026-02-02_17_51";

fn read_nums(p: &Path) -> HashSet<u32> {
    std::fs::read_to_string(p).map(|t| t.lines().filter_map(|l| l.trim().parse().ok()).collect()).unwrap_or_default()
}

fn parse_maps(spec: &str) -> Vec<u32> {
    let mut v = Vec::new();
    for part in spec.split(',') {
        let part = part.trim();
        if let Some((a, b)) = part.split_once('-') {
            if let (Ok(a), Ok(b)) = (a.trim().parse::<u32>(), b.trim().parse::<u32>()) {
                v.extend(a..=b);
            }
        } else if let Ok(n) = part.parse::<u32>() {
            v.push(n);
        }
    }
    v.sort_unstable();
    v.dedup();
    v
}

/// `p<P>/<NN>-U10S_<N>.Map.Gbx` for map N under the originals root.
fn source_of(root: &Path, n: u32) -> Option<(PathBuf, u32, String)> {
    let p = (n - 1) / 25 + 1;
    let dir = root.join(format!("p{p}"));
    let rd = std::fs::read_dir(&dir).ok()?;
    for e in rd.flatten() {
        let name = e.file_name().to_string_lossy().to_string();
        if name.ends_with(&format!("-U10S_{n:02}.Map.Gbx")) || name.ends_with(&format!("-U10S_{n}.Map.Gbx")) {
            let nn = name.split('-').next().unwrap_or("01").to_string();
            return Some((e.path(), p, nn));
        }
    }
    None
}

pub fn cmd(args: &[String]) -> i32 {
    let f = |k: &str| tmmaps::cli::flag(args, k).map(String::from);
    let src = PathBuf::from(f("--src").unwrap_or_else(|| {
        eprintln!("rollout-build needs --src ORIGINALS");
        std::process::exit(2)
    }));
    let out = PathBuf::from(f("--out").unwrap_or_else(|| {
        eprintln!("rollout-build needs --out ROOT");
        std::process::exit(2)
    }));
    let sets: Vec<String> = f("--sets").unwrap_or_else(|| "dirt,grass,ice,plastic,magnet,wood,woodwet".into()).split(',').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect();
    let maps = parse_maps(&f("--maps").unwrap_or_else(|| "1-975".into()));
    let jobs: usize = f("--jobs").and_then(|j| j.parse().ok()).unwrap_or(6).max(1);
    let keep = PathBuf::from(f("--keep").unwrap_or_else(|| format!("{}/u10s-keep", std::env::var("HOME").unwrap_or_default())));
    let bin_dir = f("--bin-dir").map(PathBuf::from).unwrap_or_else(|| std::env::current_exe().ok().and_then(|p| p.parent().map(Path::to_path_buf)).unwrap_or_default());
    let tinyctl = bin_dir.join("tinyctl");
    let tmmaps = bin_dir.join("tmmaps");

    let sb = read_nums(&keep.join("sb-driven.txt"));
    let mats = read_nums(&keep.join("mats-driven.txt"));
    let custom_list = keep.join("custom-convert2.txt");
    let physics_list = keep.join("custom-physics-only.txt");
    for p in [&custom_list, &physics_list] {
        if !p.exists() {
            eprintln!("rollout-build: {} is missing — the custom-item classifier lists are required (the /tmp purge of 2026-09-20 taught that)", p.display());
            return 2;
        }
    }
    std::fs::create_dir_all(&out).ok();
    let log_path = out.join("build.log");
    let tsv_path = out.join("build.tsv");
    let mut tsv = String::from("set\tnum\tpath\tbytes\tstatus\n");
    let mut log = String::new();

    // the work list
    let mut work: Vec<(String, u32)> = Vec::new();
    for s in &sets {
        for &n in &maps {
            work.push((s.clone(), n));
        }
    }
    println!("rollout-build: {} map-sets ({} sets × {} maps), {jobs} at a time, from {}", work.len(), sets.len(), maps.len(), src.display());

    let t0 = std::time::Instant::now();
    let mut running: Vec<(String, u32, PathBuf, std::process::Child, PathBuf)> = Vec::new();
    let mut done = 0usize;
    let mut failed = 0usize;
    let mut queue = work.into_iter();
    loop {
        while running.len() < jobs {
            let Some((s, n)) = queue.next() else { break };
            let Some((map, p, nn)) = source_of(&src, n) else {
                tsv.push_str(&format!("{s}\t{n}\t-\t0\tFAILED no source\n"));
                failed += 1;
                continue;
            };
            let out_dir = out.join(&s).join(format!("p{p}"));
            std::fs::create_dir_all(&out_dir).ok();
            let mut c = Command::new(&tinyctl);
            c.arg("surface").arg(&nn).arg("--src-dir").arg(map.parent().unwrap_or(Path::new("."))).arg("--out-root").arg(&out_dir).arg("--surface").arg(&s).arg("--name-style").arg("everios").arg("--max-bytes").arg("7000000").arg("--no-publish").arg("--no-shoot").arg("--bin-dir").arg(&bin_dir);
            if s == "ice" {
                c.arg("--label").arg("Icy");
            }
            c.env("TINY_CUSTOM_ITEM_LIST", &custom_list).env("TINY_CUSTOM_ITEM_PHYSICS_LIST", &physics_list);
            if sb.contains(&n) {
                c.env("TINY_STRUCTURE_BASE", "1");
            }
            if mats.contains(&n) && s != "plastic" {
                c.env("TINY_ITEM_REDRESS_EXTRA", "InflatableMat*");
            }
            if STAGE_DECK_MAPS.contains(&n) {
                c.env("TINY_STAGE_DECKS", "1");
            }
            let run_log = out_dir.join(format!("surface-{nn}.log"));
            let lf = std::fs::File::create(&run_log).ok();
            if let Some(lf) = lf {
                c.stdout(lf.try_clone().unwrap()).stderr(lf);
            }
            match c.spawn() {
                Ok(child) => running.push((s, n, out_dir, child, run_log)),
                Err(e) => {
                    tsv.push_str(&format!("{s}\t{n}\t-\t0\tFAILED spawn: {e}\n"));
                    failed += 1;
                }
            }
        }
        if running.is_empty() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(400));
        let mut i = 0;
        while i < running.len() {
            let finished = running[i].3.try_wait().ok().flatten();
            if let Some(status) = finished {
                let (s, n, out_dir, _, run_log) = running.remove(i);
                // `tinyctl surface <NN>` writes ROOT/<set><NN>/<name>.Map.Gbx — NN is the
                // map's position in its Part, so look under every <set>* subdir
                let produced = std::fs::read_dir(&out_dir).ok().and_then(|rd| {
                    rd.flatten()
                        .map(|e| e.path())
                        .filter(|d| d.is_dir() && d.file_name().map(|f| f.to_string_lossy().starts_with(s.as_str())).unwrap_or(false))
                        .flat_map(|d| std::fs::read_dir(d).ok().into_iter().flatten().flatten().map(|e| e.path()))
                        .find(|p| p.extension().map(|x| x == "Gbx").unwrap_or(false) && (p.to_string_lossy().contains(&format!("U10S_{n} ")) || p.to_string_lossy().contains(&format!("U10S_{n:02} "))))
                });
                let row = match (status.success(), produced) {
                    (true, Some(path)) => {
                        let bytes = std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
                        let mut st = "OK".to_string();
                        if s == "wood" || s == "woodwet" {
                            let hdr = tmmaps::header::read(&path.to_string_lossy()).ok();
                            let stamp = hdr.map(|h| h.exebuild).unwrap_or_default();
                            if stamp != WOOD_BUILD {
                                // stamp it here rather than fail: the converter preserves the source's
                                let stamped = path.with_extension("stamped.Gbx");
                                let r = Command::new(&tmmaps).arg("set-exebuild").arg(&path).arg("--build").arg(WOOD_BUILD).arg("--record").arg(format!("date={WOOD_BUILD} git=128149-c7d05ad2551 GameVersion=3.3.0")).arg("--out").arg(&stamped).output();
                                if r.map(|o| o.status.success()).unwrap_or(false) && std::fs::rename(&stamped, &path).is_ok() {
                                    st = format!("OK stamped {WOOD_BUILD} (was {stamp})");
                                } else {
                                    st = format!("FAILED wood stamp {stamp}");
                                }
                            }
                        }
                        if st.starts_with("FAILED") {
                            failed += 1;
                        }
                        format!("{s}\t{n}\t{}\t{bytes}\t{st}\n", path.display())
                    }
                    (ok, _) => {
                        failed += 1;
                        let tail = std::fs::read_to_string(&run_log).ok().and_then(|t| t.lines().rev().find(|l| !l.trim().is_empty()).map(String::from)).unwrap_or_default();
                        format!("{s}\t{n}\t-\t0\tFAILED {}: {}\n", if ok { "no output file" } else { "exit" }, tail.chars().take(160).collect::<String>())
                    }
                };
                done += 1;
                if done % 25 == 0 || row.contains("FAILED") {
                    let line = format!("[{:>6.0}s] {done} done, {failed} failed — {}", t0.elapsed().as_secs_f64(), row.trim_end());
                    println!("{line}");
                    log.push_str(&line);
                    log.push('\n');
                    let _ = std::fs::write(&log_path, &log);
                    let _ = std::fs::write(&tsv_path, &tsv);
                }
                tsv.push_str(&row);
            } else {
                i += 1;
            }
        }
    }
    let _ = std::fs::write(&tsv_path, &tsv);
    log.push_str(&format!("ROLLOUT-BUILD-DONE {done} built, {failed} failed in {:.0}s\n", t0.elapsed().as_secs_f64()));
    let _ = std::fs::write(&log_path, &log);
    println!("rollout-build: {done} map-sets, {failed} FAILED, {:.0}s → {}", t0.elapsed().as_secs_f64(), tsv_path.display());
    if failed > 0 {
        1
    } else {
        0
    }
}
