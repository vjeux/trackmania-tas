//! The whole corpus in one batch: every map with a map file and a rank-1
//! ghost, graded, in parallel, into one directory of transcripts and one
//! summary table.
//!
//! This exists so a change to the geometry is judged the same way every time.
//! A model change that raises coverage on the map you were looking at and
//! quietly lowers it on nine others is the normal outcome of guessing, and the
//! only defence is to re-run all of them and diff the table.
//!
//! Each map runs as its own process: a fit sweeps fifty-odd whole scenes and
//! the big maps index two million triangles, so isolation is worth more than
//! the shared pack cache would be.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Mutex;

pub struct Job {
    pub id: String,
    pub map: PathBuf,
    pub ghost: PathBuf,
}

/// Every subdirectory of `root` that holds a `.Map.Gbx` and a rank-1 ghost.
///
/// A map directory in this project accumulates repairs, scratch copies and
/// segment maps, so the map file is chosen as the shortest name ending
/// `.Map.Gbx` at the top level — the untouched original — and directories
/// whose name is not a bare map id are skipped, because `173636_arm2_src` and
/// `208024_container_repair` are the same map twice.
pub fn jobs(root: &Path) -> Vec<Job> {
    let mut out = Vec::new();
    let Ok(rd) = std::fs::read_dir(root) else { return out };
    let mut dirs: Vec<PathBuf> = rd.filter_map(|e| e.ok()).map(|e| e.path()).collect();
    dirs.sort();
    for d in dirs {
        let Some(id) = d.file_name().map(|s| s.to_string_lossy().to_string()) else { continue };
        if !d.is_dir() || !id.chars().all(|c| c.is_ascii_digit()) {
            continue;
        }
        let map = pick(&d, ".Map.Gbx");
        let ghost = pick(&d.join("ghosts"), ".Ghost.Gbx");
        if let (Some(map), Some(ghost)) = (map, ghost) {
            out.push(Job { id, map, ghost });
        }
    }
    out
}

/// The shortest file name in `dir` ending in `suffix`, preferring one that
/// starts `rank00001` when there is a ranked set.
fn pick(dir: &Path, suffix: &str) -> Option<PathBuf> {
    let mut best: Option<PathBuf> = None;
    for e in std::fs::read_dir(dir).ok()?.filter_map(|e| e.ok()) {
        let p = e.path();
        let n = p.file_name()?.to_string_lossy().to_string();
        if !n.ends_with(suffix) {
            continue;
        }
        let rank1 = n.starts_with("rank00001");
        let better = match &best {
            None => true,
            Some(b) => {
                let bn = b.file_name().unwrap().to_string_lossy().to_string();
                let brank1 = bn.starts_with("rank00001");
                (rank1 && !brank1) || (rank1 == brank1 && n.len() < bn.len())
            }
        };
        if better {
            best = Some(p);
        }
    }
    best
}

/// Run every job, `jobs_n` at a time, writing `<out>/<id>.txt` per map and
/// returning each map's `SUMMARY` line.
pub fn run(js: &[Job], out: &Path, jobs_n: usize, extra: &[String]) -> BTreeMap<String, String> {
    std::fs::create_dir_all(out).ok();
    let exe = std::env::current_exe().expect("current exe");
    let next = AtomicUsize::new(0);
    let results: Mutex<BTreeMap<String, String>> = Mutex::new(BTreeMap::new());
    std::thread::scope(|s| {
        for _ in 0..jobs_n.max(1) {
            s.spawn(|| loop {
                let i = next.fetch_add(1, Ordering::SeqCst);
                let Some(j) = js.get(i) else { return };
                let o = std::process::Command::new(&exe)
                    .arg("check")
                    .arg(&j.map)
                    .arg("--ghost")
                    .arg(&j.ghost)
                    .args(extra)
                    .output();
                let (text, summary) = match o {
                    Ok(o) => {
                        let mut t = String::from_utf8_lossy(&o.stdout).to_string();
                        t.push_str(&String::from_utf8_lossy(&o.stderr));
                        let sum = t
                            .lines()
                            .find(|l| l.starts_with("SUMMARY\t"))
                            .map(|l| l.to_string())
                            .unwrap_or_else(|| {
                                format!("NOFIT\t{}", t.lines().last().unwrap_or("").trim())
                            });
                        (t, sum)
                    }
                    Err(e) => (e.to_string(), format!("NOFIT\t{}", e)),
                };
                std::fs::write(out.join(format!("{}.txt", j.id)), &text).ok();
                eprintln!("{}  {}", j.id, summary);
                results.lock().unwrap().insert(j.id.clone(), summary);
            });
        }
    });
    results.into_inner().unwrap()
}
