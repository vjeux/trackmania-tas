//! `tinyctl port-layers` — the Wood and Wet Wood layers of the Stadium ports, rebuilt from the
//! FIXED ports on the devserver (json9841 2026-09-25: "rebuild and test wood and wet wood").
//!
//! ```text
//! tinyctl port-layers --ports DIR --out DIR --rules R1,R2,… --items LIB [--only NN,…] [--no-wood]
//!     [--tmmaps BIN] [--jobs 4]
//! ```
//!
//! Per port `NN-<Name>-Stadium.Map.Gbx` (the anport5 recipe of 2026-09-22, as a program):
//!   1. `tmmaps altered-apply PORT --rules … --items LIB --out W --wet --name "<Name> Wet Wood"`
//!      and, unless `--no-wood`, the same without `--wet` → `<Name> Wood`;
//!   2. the leftover vanilla drivable blocks baked by `tinyctl surface wood` (no shoot, no publish),
//!      the result renamed back to the layer's name and uid (`tmmaps rename-map --uid`);
//!   3. the 2026-02-02 exe-build stamp (the wood physics key).
//! Outputs: `OUT/<camp>-wetwood/NN-<Name>-Wet-Wood.Map.Gbx`, `OUT/<camp>-wood/…-Wood.Map.Gbx`,
//! one summary line per map. Rust end to end; the child commands are this repo's own binaries.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Instant;

const WOOD_BUILD: &str = "2026-02-02_17_51";
const WOOD_RECORD: &str = "date=2026-02-02_17_51 git=128149-c7d05ad2551 GameVersion=3.3.0";

pub fn cmd(args: &[String]) -> Result<(), String> {
    let f = |k: &str| tmmaps::cli::flag(args, k).map(String::from);
    let ports = PathBuf::from(f("--ports").ok_or("port-layers needs --ports DIR")?);
    let out_root = PathBuf::from(f("--out").ok_or("port-layers needs --out DIR")?);
    let rules = f("--rules").ok_or("port-layers needs --rules a.tsv,b.tsv")?;
    let items = f("--items").ok_or("port-layers needs --items LIB")?;
    let only: Option<Vec<String>> = f("--only").map(|v| v.split(',').map(|s| s.trim().to_string()).collect());
    let no_wood = tmmaps::cli::has(args, "--no-wood");
    let exe_dir = std::env::current_exe().ok().and_then(|p| p.parent().map(|d| d.to_path_buf())).unwrap_or_default();
    let tmmaps_bin = f("--tmmaps").map(PathBuf::from).unwrap_or_else(|| exe_dir.join("tmmaps"));
    let tinyctl_bin = std::env::current_exe().map_err(|e| e.to_string())?;
    let jobs: usize = f("--jobs").and_then(|v| v.parse().ok()).unwrap_or(4).max(1);
    let camp = ports.file_name().map(|s| s.to_string_lossy().trim_end_matches("-stadium").to_string()).unwrap_or_else(|| "camp".into());
    let wet_dir = out_root.join(format!("{camp}-wetwood"));
    let wood_dir = out_root.join(format!("{camp}-wood"));
    std::fs::create_dir_all(&wet_dir).map_err(|e| e.to_string())?;
    std::fs::create_dir_all(&wood_dir).map_err(|e| e.to_string())?;
    let mut list: Vec<PathBuf> = std::fs::read_dir(&ports).map_err(|e| format!("{}: {e}", ports.display()))?.flatten().map(|e| e.path()).filter(|p| p.to_string_lossy().ends_with(".Map.Gbx")).collect();
    list.sort();
    if let Some(o) = &only {
        list.retain(|p| o.iter().any(|nn| p.file_name().unwrap().to_string_lossy().starts_with(&format!("{nn}-"))));
    }
    let t0 = Instant::now();
    let next = std::sync::atomic::AtomicUsize::new(0);
    let lines = std::sync::Mutex::new(Vec::<String>::new());
    std::thread::scope(|sc| {
        for _ in 0..jobs.min(list.len().max(1)) {
            let (list, next, lines, tmmaps_bin, tinyctl_bin, wet_dir, wood_dir, rules, items) = (&list, &next, &lines, &tmmaps_bin, &tinyctl_bin, &wet_dir, &wood_dir, &rules, &items);
            sc.spawn(move || loop {
                let i = next.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                if i >= list.len() {
                    break;
                }
                let port = &list[i];
                let nn: String = port.file_name().unwrap().to_string_lossy().chars().take(2).collect();
                let hdr = match tmmaps::header::read(port.to_str().unwrap()) {
                    Ok(h) => h,
                    Err(e) => {
                        lines.lock().unwrap().push(format!("{nn}\tFAILED header: {e}"));
                        continue;
                    }
                };
                // the port's name is "<Name> [Stadium]" (with markup): the layer names build on <Name>
                let base = hdr.name.replace("$z", "").replace(" [Stadium]", "").trim().to_string();
                let file_base = base.replace(' ', "-");
                let mut parts: Vec<String> = Vec::new();
                for (wet, dir, suffix) in [(true, wet_dir, "Wet Wood"), (false, wood_dir, "Wood")] {
                    if !wet && no_wood {
                        continue;
                    }
                    let out = dir.join(format!("{nn}-{file_base}-{}.Map.Gbx", suffix.replace(' ', "-")));
                    let name = format!("{base} {suffix}");
                    let r = layer(tmmaps_bin, tinyctl_bin, port, &out, &name, wet, rules, items);
                    parts.push(match r {
                        Ok(s) => format!("{suffix}: {s}"),
                        Err(e) => format!("{suffix}: FAILED {}", e.lines().next().unwrap_or("").chars().take(120).collect::<String>()),
                    });
                }
                let line = format!("{nn}\t{base}\t{}", parts.join(" | "));
                eprintln!("[{:>5.0}s] {line}", t0.elapsed().as_secs_f64());
                lines.lock().unwrap().push(line);
            });
        }
    });
    let mut lines = lines.into_inner().unwrap();
    lines.sort();
    let summary = out_root.join(format!("{camp}-layers.tsv"));
    std::fs::write(&summary, lines.join("\n") + "\n").map_err(|e| e.to_string())?;
    let bad = lines.iter().filter(|l| l.contains("FAILED")).count();
    println!("port-layers {camp}: {} ports, {bad} with a failure, {:.0}s — {}", list.len(), t0.elapsed().as_secs_f64(), summary.display());
    if bad > 0 {
        return Err(format!("{bad} layers failed"));
    }
    Ok(())
}

fn run(bin: &Path, args: &[&str]) -> Result<String, String> {
    let out = Command::new(bin).args(args).output().map_err(|e| format!("{}: {e}", bin.display()))?;
    let text = format!("{}{}", String::from_utf8_lossy(&out.stdout), String::from_utf8_lossy(&out.stderr));
    if !out.status.success() {
        return Err(format!("{} {}: {}", bin.file_name().unwrap().to_string_lossy(), args.first().unwrap_or(&""), text.lines().last().unwrap_or("").to_string()));
    }
    Ok(text)
}

/// One layer of one port: altered-apply → wood bake of the leftovers → name/uid back → stamp.
fn layer(tmmaps_bin: &Path, tinyctl_bin: &Path, port: &Path, out: &Path, name: &str, wet: bool, rules: &str, items: &str) -> Result<String, String> {
    let port_s = port.to_str().unwrap();
    let out_s = out.to_str().unwrap();
    let mut a: Vec<&str> = vec!["altered-apply", port_s, "--rules", rules, "--items", items, "--out", out_s, "--name", name];
    if wet {
        a.push("--wet");
    }
    let applied = run(tmmaps_bin, &a)?;
    let applied_line = applied.lines().find(|l| l.contains("renamed") || l.contains("swapped") || l.contains("blocks")).unwrap_or("").trim().to_string();
    // the bake of the leftover vanilla drivable blocks
    let hdr = tmmaps::header::read(out_s).map_err(|e| e.to_string())?;
    let work = std::env::temp_dir().join(format!("port-layer-{}-{}", std::process::id(), out.file_name().unwrap().to_string_lossy().replace(' ', "_")));
    let _ = std::fs::remove_dir_all(&work);
    let src_dir = work.join("src");
    std::fs::create_dir_all(&src_dir).map_err(|e| e.to_string())?;
    std::fs::copy(out, src_dir.join("01-x.Map.Gbx")).map_err(|e| e.to_string())?;
    let out_root = work.join("out");
    let baked = run(tinyctl_bin, &["surface", "01", "--src-dir", src_dir.to_str().unwrap(), "--out-root", out_root.to_str().unwrap(), "--surface", "wood", "--no-shoot", "--no-publish"])?;
    let bake_line = baked.lines().rev().find(|l| l.contains("baked,")).map(|l| l.trim().to_string()).unwrap_or_default();
    let built = std::fs::read_dir(out_root.join("wood01")).ok().and_then(|rd| rd.flatten().map(|e| e.path()).find(|p| p.to_string_lossy().ends_with(".Map.Gbx"))).ok_or("surface wood: no output map")?;
    run(tmmaps_bin, &["rename-map", built.to_str().unwrap(), "--out", out_s, "--name", &hdr.name, "--uid", &hdr.uid])?;
    // the exe-build stamp (a variable-length edit on a fresh load)
    let mut m = tmmaps::map::MapFile::load(out);
    m.set_exebuild(WOOD_BUILD, None);
    let _ = m.set_build_record(WOOD_RECORD);
    m.write_to(out).map_err(|e| e.to_string())?;
    let _ = std::fs::remove_dir_all(&work);
    let size = std::fs::metadata(out).map(|m| m.len()).unwrap_or(0);
    Ok(format!("{size} B; {}; bake {}", applied_line.chars().take(60).collect::<String>(), bake_line.chars().take(60).collect::<String>()))
}
