//! `tinyctl terrain-round` — the whole terrain loop for one Stadium-port campaign, as one
//! program (json9841 2026-09-25/26: "some terrain blocks required for the route are missing",
//! "ray cast down from the car"):
//!
//! ```text
//! tinyctl terrain-round --camp wi26 --src-dir /tmp/anaudit/an/wi26 --port-dir /tmp/anaudit/an-port/wi26-stadium
//!     --pak /tmp/WhiteShore.pak:KEY --out /tmp/anaudit/round2/wi26 [--jobs 3] [--only 03,19]
//! ```
//!
//! Per map: `mapgeom route-fix` (v2 prune + corridor) → `mapgeom route-audit --patch-item`
//! (the ROUTE PATCH strip where the port's support is missing/off) → `tmmaps add-items` →
//! `mapgeom route-audit` again (the verdicts written to `OUT/audit/NN.txt`). The final map
//! lands in `OUT/final/<port file name>`; `OUT/summary.tsv` has one row per map (removed
//! items, patch runs/sections, final INTERSECT/MISSING/SHIFTED/lateral counts, changed vs the
//! original port). Publishing stays a separate step (`port-fix-publish`, `port-layers`).

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Mutex;
use std::time::Instant;

const STADIUM_PAK: &str = "/tmp/current-Stadium.pak:B773D73047A4104857722366D78D28A6";

fn run(bin: &Path, args: &[&str]) -> (bool, String) {
    match Command::new(bin).args(args).output() {
        Ok(o) => (o.status.success(), format!("{}{}", String::from_utf8_lossy(&o.stdout), String::from_utf8_lossy(&o.stderr))),
        Err(e) => (false, format!("{}: {e}", bin.display())),
    }
}

fn nn_of(p: &Path) -> String {
    p.file_name().map(|f| f.to_string_lossy().chars().take(2).collect()).unwrap_or_default()
}

fn find_nn(dir: &Path, nn: &str) -> Option<PathBuf> {
    std::fs::read_dir(dir).ok()?.flatten().map(|e| e.path()).find(|p| p.file_name().map(|f| f.to_string_lossy().starts_with(&format!("{nn}-")) && f.to_string_lossy().ends_with(".Map.Gbx")).unwrap_or(false))
}

/// The audit's one-line verdict + lateral line, condensed.
fn condense(text: &str) -> (String, String) {
    let verdict = text.lines().find(|l| l.contains("samples (")).map(|l| l.split("samples (").nth(1).unwrap_or("").split("; surface").next().unwrap_or("").trim().to_string()).unwrap_or_else(|| "no audit".into());
    let lateral = text.lines().find(|l| l.trim_start().starts_with("lateral (")).map(|l| l.trim().replace("lateral (3 m each side): ", "")).unwrap_or_default();
    (verdict, lateral)
}

pub fn cmd(args: &[String]) -> Result<(), String> {
    let f = |k: &str| tmmaps::cli::flag(args, k).map(String::from);
    let camp = f("--camp").ok_or("terrain-round needs --camp wi26|sp26|su26")?;
    let src_dir = PathBuf::from(f("--src-dir").ok_or("--src-dir DIR")?);
    let port_dir = PathBuf::from(f("--port-dir").ok_or("--port-dir DIR")?);
    let pak = f("--pak").ok_or("--pak FILE:KEY (the vista collection's pack)")?;
    let out = PathBuf::from(f("--out").ok_or("--out DIR")?);
    let jobs: usize = f("--jobs").and_then(|v| v.parse().ok()).unwrap_or(3).max(1);
    let only: Option<Vec<String>> = f("--only").map(|v| v.split(',').map(|s| s.trim().to_string()).collect());
    let exe_dir = std::env::current_exe().ok().and_then(|p| p.parent().map(|d| d.to_path_buf())).unwrap_or_default();
    let mapgeom = exe_dir.join("mapgeom");
    let tmmaps = exe_dir.join("tmmaps");
    for d in ["fixed", "patch", "final", "audit", "report"] {
        std::fs::create_dir_all(out.join(d)).map_err(|e| e.to_string())?;
    }
    let mut ports: Vec<PathBuf> = std::fs::read_dir(&port_dir).map_err(|e| format!("{}: {e}", port_dir.display()))?.flatten().map(|e| e.path()).filter(|p| p.to_string_lossy().ends_with(".Map.Gbx")).collect();
    ports.sort();
    if let Some(o) = &only {
        ports.retain(|p| o.contains(&nn_of(p)));
    }
    let t0 = Instant::now();
    let next = AtomicUsize::new(0);
    let rows = Mutex::new(Vec::<String>::new());
    std::thread::scope(|sc| {
        for _ in 0..jobs.min(ports.len().max(1)) {
            let (ports, next, rows, mapgeom, tmmaps, src_dir, out, pak, camp) = (&ports, &next, &rows, &mapgeom, &tmmaps, &src_dir, &out, &pak, &camp);
            sc.spawn(move || loop {
                let i = next.fetch_add(1, Ordering::SeqCst);
                if i >= ports.len() {
                    break;
                }
                let port = &ports[i];
                let nn = nn_of(port);
                let fname = port.file_name().unwrap().to_string_lossy().into_owned();
                let Some(src) = find_nn(src_dir, &nn) else {
                    rows.lock().unwrap().push(format!("{nn}\tno source\t\t\t\t\t"));
                    continue;
                };
                let fixed = out.join("fixed").join(&fname);
                let report = out.join("report").join(format!("{nn}-fix.tsv"));
                // 1. the v2 fix (prune + corridor); a map without a ghost still gets the prune
                let (ok, text) = run(mapgeom, &["--pak", pak, "--pak", STADIUM_PAK, "route-fix", src.to_str().unwrap(), port.to_str().unwrap(), "--out", fixed.to_str().unwrap(), "--report", report.to_str().unwrap()]);
                if !ok {
                    rows.lock().unwrap().push(format!("{nn}\tFIX FAILED {}\t\t\t\t\t", text.lines().last().unwrap_or("").chars().take(100).collect::<String>()));
                    continue;
                }
                let removed = text.lines().find(|l| l.contains("terrain items removed")).and_then(|l| l.split(": ").nth(1)).and_then(|l| l.split(' ').next()).unwrap_or("0").to_string();
                // 2. the route patch
                let item = out.join("patch").join(format!("RoutePatch{}{nn}.Item.Gbx", camp.chars().take(2).collect::<String>().to_uppercase()));
                let place = out.join("patch").join(format!("{nn}-place.tsv"));
                let (_ok, atext) = run(mapgeom, &["--pak", pak, "--pak", STADIUM_PAK, "route-audit", src.to_str().unwrap(), fixed.to_str().unwrap(), "--every", "200", "--patch-item", item.to_str().unwrap(), "--patch-place", place.to_str().unwrap()]);
                let patch_line = atext.lines().find(|l| l.contains("route patch")).map(|l| l.trim().to_string()).unwrap_or_else(|| "no ghost".into());
                let final_p = out.join("final").join(&fname);
                let placed = std::fs::read_to_string(&place).ok().filter(|t| !t.trim().is_empty());
                let mut patch_note = String::from("-");
                if let Some(pl) = placed {
                    let pos = pl.trim().split('\t').nth(1).unwrap_or("0,0,0").to_string();
                    let spec = format!("{}@{pos}", item.display());
                    let (ok2, t2) = run(tmmaps, &["add-items", fixed.to_str().unwrap(), "--out", final_p.to_str().unwrap(), "--items", &spec]);
                    if !ok2 {
                        rows.lock().unwrap().push(format!("{nn}\tADD-ITEMS FAILED {}\t\t\t\t\t", t2.lines().last().unwrap_or("").chars().take(100).collect::<String>()));
                        continue;
                    }
                    patch_note = patch_line.replace("route patch: ", "").split(" -> ").next().unwrap_or("").to_string();
                } else {
                    let _ = std::fs::copy(&fixed, &final_p);
                }
                // 3. the audit of the final file
                let (_ok3, ftext) = run(mapgeom, &["--pak", pak, "--pak", STADIUM_PAK, "route-audit", src.to_str().unwrap(), final_p.to_str().unwrap(), "--every", "200", "--out", out.join("audit").join(format!("{nn}.tsv")).to_str().unwrap()]);
                let _ = std::fs::write(out.join("audit").join(format!("{nn}.txt")), &ftext);
                let (verdict, lateral) = condense(&ftext);
                let changed = std::fs::read(&final_p).ok() != std::fs::read(port).ok();
                let line = format!("{nn}\tremoved {removed}\t{patch_note}\t{verdict}\t{lateral}\t{}", if changed { "changed" } else { "same as original" });
                eprintln!("[{:>5.0}s] {line}", t0.elapsed().as_secs_f64());
                rows.lock().unwrap().push(line);
            });
        }
    });
    let mut rows = rows.into_inner().unwrap();
    rows.sort();
    let text = format!("map\tprune\tpatch\tverdict\tlateral\tstate\n{}\n", rows.join("\n"));
    std::fs::write(out.join("summary.tsv"), &text).map_err(|e| e.to_string())?;
    print!("{text}");
    println!("terrain-round {camp}: {} maps in {:.0}s -> {}", rows.len(), t0.elapsed().as_secs_f64(), out.join("summary.tsv").display());
    Ok(())
}
