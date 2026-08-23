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
///
/// `pins` overrides the ghost for a map id, which is how a re-run is made
/// comparable with an earlier one that chose a different rank-1 file.
pub fn jobs(root: &Path, pins: &BTreeMap<String, String>) -> Vec<Job> {
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
        let ghost = match pins.get(&id) {
            Some(g) => Some(d.join("ghosts").join(g)),
            None => pick(&d.join("ghosts"), ".Ghost.Gbx"),
        };
        if let (Some(map), Some(ghost)) = (map, ghost) {
            out.push(Job { id, map, ghost });
        }
    }
    out
}

/// The best file in `dir` ending in `suffix`.
///
/// For ghosts that means **the lowest rank**: this project's ghost files are
/// named `rank00001_…`, `rank01_…`, `r001_…`, `p00001_…`, `hl_rank00001_…`,
/// so the rank is read as the first run of digits in the name rather than by
/// matching any one of those spellings. Ties, and files with no number at all,
/// fall back to the shortest name — which for a map file picks the untouched
/// original out of a directory that has accumulated repairs and segment maps.
fn pick(dir: &Path, suffix: &str) -> Option<PathBuf> {
    let key = |n: &str| -> (u32, usize) {
        let rank = n
            .split(|c: char| !c.is_ascii_digit())
            .find(|s| !s.is_empty())
            .and_then(|s| s.parse::<u32>().ok())
            .unwrap_or(u32::MAX);
        (rank, n.len())
    };
    let mut best: Option<(PathBuf, (u32, usize))> = None;
    for e in std::fs::read_dir(dir).ok()?.filter_map(|e| e.ok()) {
        let p = e.path();
        let n = p.file_name()?.to_string_lossy().to_string();
        if !n.ends_with(suffix) {
            continue;
        }
        let k = key(&n);
        if best.as_ref().map_or(true, |(_, bk)| k < *bk) {
            best = Some((p, k));
        }
    }
    best.map(|(p, _)| p)
}

/// What one map's transcript says, whichever format it is in.
///
/// The first corpus run banked human-readable transcripts and no machine
/// summary, so the before/after table has to read those too — and reading them
/// is the whole point: a change to the geometry is judged by the diff against
/// the numbers that were banked before it, not against a memory of them.
#[derive(Default, Clone)]
pub struct Row {
    pub ghost: String,
    pub yoff: String,
    pub samples: usize,
    pub raw: f32,
    pub median: f32,
    /// present only in the newer format
    pub owed: Option<usize>,
    pub covered: Option<f32>,
    pub missing: Option<usize>,
    pub blame: String,
}

pub fn read_row(text: &str) -> Option<Row> {
    if let Some(l) = text.lines().find(|l| l.starts_with("SUMMARY\t")) {
        let f: Vec<&str> = l.split('\t').collect();
        if f.len() >= 15 {
            return Some(Row {
                ghost: f[1].trim_start_matches("path_").to_string(),
                yoff: f[2].to_string(),
                samples: f[3].parse().ok()?,
                raw: f[4].parse().ok()?,
                median: f[7].parse().ok()?,
                owed: f[5].parse().ok(),
                covered: f[6].parse().ok(),
                missing: f[12].parse().ok(),
                blame: f[14].to_string(),
            });
        }
    }
    // The banked format:
    //   yoff -64  (1948026 triangles indexed)
    //   path_x.Ghost: 709/1271 samples over a surface (55.8 %)
    //     gap below the car   median 0.030 m   p10 ...
    let mut r = Row::default();
    let mut found = false;
    for l in text.lines() {
        let t = l.trim();
        if let Some(rest) = t.strip_prefix("yoff ") {
            if rest.contains("triangles indexed") {
                r.yoff = rest.split_whitespace().next().unwrap_or("").to_string();
            }
        }
        if let Some((head, tail)) = t.split_once(": ") {
            if tail.contains("samples over a surface") {
                if let Some((hits, rest)) = tail.split_once('/') {
                    r.ghost = head.trim_start_matches("path_").to_string();
                    let total: usize = rest.split_whitespace().next()?.parse().ok()?;
                    let hits: usize = hits.parse().ok()?;
                    r.samples = total;
                    r.raw = hits as f32 / total.max(1) as f32;
                    found = true;
                }
            }
        }
        if let Some(rest) = t.strip_prefix("gap below the car") {
            let w: Vec<&str> = rest.split_whitespace().collect();
            if let Some(i) = w.iter().position(|x| *x == "median") {
                r.median = w.get(i + 1).and_then(|x| x.parse().ok()).unwrap_or(f32::NAN);
            }
        }
    }
    if found {
        Some(r)
    } else {
        None
    }
}

/// The before/after table, as markdown, from two directories of transcripts.
pub fn compare(before: &Path, after: &Path) -> String {
    let load = |d: &Path| -> BTreeMap<String, Row> {
        let mut out = BTreeMap::new();
        if let Ok(rd) = std::fs::read_dir(d) {
            for e in rd.filter_map(|e| e.ok()) {
                let p = e.path();
                let Some(name) = p.file_stem().map(|s| s.to_string_lossy().to_string()) else {
                    continue;
                };
                if p.extension().map(|x| x != "txt").unwrap_or(true) {
                    continue;
                }
                let Ok(t) = std::fs::read_to_string(&p) else { continue };
                match read_row(&t) {
                    Some(r) => {
                        out.insert(name, r);
                    }
                    None => {
                        out.insert(name, Row { ghost: "NO FIT".into(), ..Row::default() });
                    }
                }
            }
        }
        out
    };
    let (b, a) = (load(before), load(after));
    let mut keys: Vec<&String> = b.keys().chain(a.keys()).collect();
    keys.sort();
    keys.dedup();
    let mut s = String::from(
        "| map | ghost | samples | over a surface, before | after | median gap, before | after | \
         of the samples the model owes | what is still missing |\n\
         |---|---|---|---|---|---|---|---|---|\n",
    );
    let pc = |r: Option<&Row>| match r {
        None => "-".to_string(),
        Some(r) if r.ghost == "NO FIT" => "no height fits".to_string(),
        Some(r) => format!("{:.1} %", 100.0 * r.raw),
    };
    let md = |r: Option<&Row>| match r {
        None => "-".to_string(),
        Some(r) if r.ghost == "NO FIT" || !r.median.is_finite() => "-".to_string(),
        Some(r) => format!("{:.3} m", r.median),
    };
    for k in keys {
        let (bb, aa) = (b.get(k), a.get(k));
        s.push_str(&format!(
            "| {} | {} | {} | {} | {} | {} | {} | {} | {} |\n",
            k,
            match (bb.map(|r| &r.ghost), aa.map(|r| &r.ghost)) {
                (Some(x), Some(y)) if x == y => y.trim_end_matches(".Ghost").to_string(),
                (Some(x), Some(y)) => format!(
                    "**{} vs {}**",
                    x.trim_end_matches(".Ghost"),
                    y.trim_end_matches(".Ghost")
                ),
                (_, Some(y)) | (Some(y), _) => y.trim_end_matches(".Ghost").to_string(),
                _ => "-".to_string(),
            },
            aa.or(bb).map(|r| r.samples).unwrap_or(0),
            pc(bb),
            pc(aa),
            md(bb),
            md(aa),
            aa.and_then(|r| r.covered.map(|c| format!("{:.1} % of {}", 100.0 * c, r.owed.unwrap_or(0))))
                .unwrap_or_else(|| "-".into()),
            aa.map(|r| {
                match r.missing {
                    Some(0) | None => "-".to_string(),
                    Some(n) => format!("{} samples: {}", n, r.blame),
                }
            })
            .unwrap_or_else(|| "-".into()),
        ));
    }
    s
}

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
