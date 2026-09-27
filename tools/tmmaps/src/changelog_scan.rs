//! `tmmaps changelog-scan` — the per-map change scan behind the public change log, as a
//! program (it was the box's `changelog.sh`, 2026-09-22..25).
//!
//! ```text
//! tmmaps changelog-scan --live DIR --rounds "thumbs:09-15 reset:09-15 … reset12:09-25" --out DIR
//!     [--sets plastic,ice,grass,dirt,wood,woodwet,magnet] [--fp-cache DIR[,DIR…]]
//!     [--mapgeom BIN] [--paks "--pak F:KEY"] [--jobs 6]
//! ```
//!
//! For every live map of every set (the Sept-14 baseline under `LIVE/<set>/pNN/<file>`), the
//! rounds are walked in order; a round holds the map when `<round>/<set>/pNN/<file>` exists
//! (a `reset*` round stores it under its club name, `U10S_<n> By Everios96 [<Label>]`). Each
//! version's PHYSICS FINGERPRINT (`tmmaps physfp`: block records + embedded hulls + item
//! placements) is diffed against the previous version; a non-empty diff is one row of
//! `OUT/changes.tsv`:
//!
//! ```text
//! set  map  round  date  n_lines  kinds(top 3 record kinds × count)  wiped | after-wipe:yes/no
//! ```
//!
//! A `reset*` round is a records WIPE: its row says `wiped` (even with 0 physics lines), and
//! every later change of that map says `after-wipe:yes`. Fingerprints are cached by the file's
//! md5 in `OUT/fp/<md5>.txt` (and read from `--fp-cache` dirs — the earlier runs' caches), so a
//! rerun only fingerprints new files. `OUT/progress.txt` gets one line per set.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Mutex;
use std::time::Instant;

fn label(set: &str) -> &'static str {
    match set {
        "plastic" => "Plastic",
        "ice" => "Icy",
        "grass" => "Grass",
        "dirt" => "Dirt",
        "wood" => "Wood",
        "woodwet" => "Wet Wood",
        "magnet" => "Magnet",
        "tiny" => "Tiny",
        "giant" => "Giant",
        _ => "?",
    }
}

fn md5_hex(bytes: &[u8]) -> String {
    // md5 through the system binary keeps the cache keys identical to the shell run's
    let mut child = Command::new("md5sum").stdin(std::process::Stdio::piped()).stdout(std::process::Stdio::piped()).spawn().expect("md5sum");
    {
        use std::io::Write;
        child.stdin.take().unwrap().write_all(bytes).expect("md5sum stdin");
    }
    let out = child.wait_with_output().expect("md5sum");
    String::from_utf8_lossy(&out.stdout).chars().take(32).collect()
}

struct Fp<'a> {
    tmmaps: PathBuf,
    mapgeom: Option<String>,
    paks: Option<String>,
    out_fp: PathBuf,
    caches: Vec<PathBuf>,
    _p: std::marker::PhantomData<&'a ()>,
}

impl Fp<'_> {
    /// The fingerprint lines of a file (cached by md5).
    fn of(&self, f: &Path) -> Result<Vec<String>, String> {
        let bytes = std::fs::read(f).map_err(|e| format!("{}: {e}", f.display()))?;
        let h = md5_hex(&bytes);
        let name = format!("{h}.txt");
        for d in std::iter::once(&self.out_fp).chain(self.caches.iter()) {
            let p = d.join(&name);
            if let Ok(t) = std::fs::read_to_string(&p) {
                if !t.is_empty() {
                    return Ok(t.lines().map(|l| l.to_string()).collect());
                }
            }
        }
        let mut cmd = Command::new(&self.tmmaps);
        cmd.arg("physfp").arg(f);
        if let Some(mg) = &self.mapgeom {
            cmd.arg("--mapgeom").arg(mg);
        }
        if let Some(p) = &self.paks {
            cmd.arg("--paks").arg(p);
        }
        let out = cmd.output().map_err(|e| format!("physfp: {e}"))?;
        if !out.status.success() {
            return Err(format!("physfp {}: {}", f.display(), String::from_utf8_lossy(&out.stderr).lines().last().unwrap_or("")));
        }
        let text = String::from_utf8_lossy(&out.stdout).into_owned();
        let tmp = self.out_fp.join(format!("{name}.tmp{}", std::process::id()));
        std::fs::write(&tmp, &text).map_err(|e| e.to_string())?;
        std::fs::rename(&tmp, self.out_fp.join(&name)).map_err(|e| e.to_string())?;
        Ok(text.lines().map(|l| l.to_string()).collect())
    }
}

/// `diff a b | grep -c '^[<>]'` and the top-3 record kinds of the changed lines.
fn fp_diff(a: &[String], b: &[String]) -> (usize, String) {
    let sa: std::collections::BTreeSet<&String> = a.iter().collect();
    let sb: std::collections::BTreeSet<&String> = b.iter().collect();
    let changed: Vec<&String> = sa.symmetric_difference(&sb).copied().collect();
    let mut kinds: BTreeMap<String, usize> = BTreeMap::new();
    for l in &changed {
        let kind = l.split('\t').nth(1).unwrap_or("").trim_end_matches(|c: char| c.is_ascii_digit()).to_string();
        *kinds.entry(kind).or_default() += 1;
    }
    let mut v: Vec<(String, usize)> = kinds.into_iter().collect();
    v.sort_by(|x, y| y.1.cmp(&x.1).then(x.0.cmp(&y.0)));
    (changed.len(), v.iter().take(3).map(|(k, n)| format!("{k}×{n} ")).collect())
}

pub fn cmd(args: &[String]) -> Result<(), String> {
    let f = |k: &str| crate::cli::flag(args, k).map(String::from);
    let live = PathBuf::from(f("--live").ok_or("changelog-scan needs --live DIR")?);
    let rounds: Vec<(String, String)> = f("--rounds").ok_or("changelog-scan needs --rounds \"name:MM-DD …\"")?.split_whitespace().filter_map(|r| r.split_once(':').map(|(a, b)| (a.to_string(), b.to_string()))).collect();
    let out = PathBuf::from(f("--out").ok_or("changelog-scan needs --out DIR")?);
    let sets: Vec<String> = f("--sets").unwrap_or_else(|| "plastic,ice,grass,dirt,wood,woodwet,magnet".into()).split(',').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect();
    let jobs: usize = f("--jobs").and_then(|v| v.parse().ok()).unwrap_or(6).max(1);
    let root = live.parent().map(|p| p.to_path_buf()).unwrap_or_else(|| PathBuf::from("."));
    let out_fp = out.join("fp");
    std::fs::create_dir_all(&out_fp).map_err(|e| e.to_string())?;
    let fp = Fp {
        tmmaps: std::env::current_exe().map_err(|e| e.to_string())?,
        mapgeom: f("--mapgeom"),
        paks: f("--paks"),
        out_fp: out_fp.clone(),
        caches: f("--fp-cache").map(|v| v.split(',').map(PathBuf::from).collect()).unwrap_or_default(),
        _p: std::marker::PhantomData,
    };
    let t0 = Instant::now();
    let progress = out.join("progress.txt");
    std::fs::write(&progress, "").map_err(|e| e.to_string())?;
    let mut all_rows: Vec<String> = Vec::new();
    for set in &sets {
        let set_dir = live.join(set);
        let mut bases: Vec<PathBuf> = Vec::new();
        if let Ok(parts) = std::fs::read_dir(&set_dir) {
            for part in parts.flatten().map(|e| e.path()).filter(|p| p.is_dir()) {
                if let Ok(files) = std::fs::read_dir(&part) {
                    bases.extend(files.flatten().map(|e| e.path()).filter(|p| p.to_string_lossy().ends_with(".Map.Gbx")));
                }
            }
        }
        bases.sort();
        let next = AtomicUsize::new(0);
        let rows = Mutex::new(Vec::<String>::new());
        let errors = Mutex::new(Vec::<String>::new());
        std::thread::scope(|sc| {
            for _ in 0..jobs.min(bases.len().max(1)) {
                let (bases, next, rows, errors, fp, rounds, root, set) = (&bases, &next, &rows, &errors, &fp, &rounds, &root, set);
                sc.spawn(move || loop {
                    let i = next.fetch_add(1, Ordering::SeqCst);
                    if i >= bases.len() {
                        break;
                    }
                    let base = &bases[i];
                    match one(fp, rounds, root, set, base) {
                        Ok(r) => rows.lock().unwrap().extend(r),
                        Err(e) => errors.lock().unwrap().push(format!("{}: {e}", base.display())),
                    }
                });
            }
        });
        let rows = rows.into_inner().unwrap();
        let errors = errors.into_inner().unwrap();
        all_rows.extend(rows.iter().cloned());
        // written after every set (a long scan can be read while it runs)
        std::fs::write(out.join("changes.tsv"), all_rows.join("\n") + if all_rows.is_empty() { "" } else { "\n" }).map_err(|e| e.to_string())?;
        let line = format!("[{:>5.0}s] {set}: {} maps, {} change rows, {} errors", t0.elapsed().as_secs_f64(), bases.len(), rows.len(), errors.len());
        println!("{line}");
        for e in errors.iter().take(5) {
            println!("  {e}");
        }
        let mut p = std::fs::read_to_string(&progress).unwrap_or_default();
        p.push_str(&line);
        p.push('\n');
        std::fs::write(&progress, p).map_err(|e| e.to_string())?;
    }
    let mut p = std::fs::read_to_string(&progress).unwrap_or_default();
    p.push_str("CHANGELOG-DONE\n");
    std::fs::write(&progress, p).map_err(|e| e.to_string())?;
    println!("{} change rows -> {}", all_rows.len(), out.join("changes.tsv").display());
    Ok(())
}

/// One live map through the rounds: the change rows.
fn one(fp: &Fp, rounds: &[(String, String)], root: &Path, set: &str, base: &Path) -> Result<Vec<String>, String> {
    let part = base.parent().and_then(|p| p.file_name()).map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
    let bname = base.file_name().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
    let num: String = bname.split("U10S_").nth(1).map(|s| s.chars().take_while(|c| c.is_ascii_digit()).collect()).unwrap_or_default();
    let mut rows = Vec::new();
    let mut prev: PathBuf = base.to_path_buf();
    let mut prev_fp: Option<Vec<String>> = None;
    let mut wiped = false;
    for (round, date) in rounds {
        let f = if round.starts_with("reset") {
            let club = root.join(round).join(set).join(&part).join(format!("U10S_{num} By Everios96 [{}].Map.Gbx", label(set)));
            // the Tiny club names carry no leading zero (`U10S_7 By Everios96 [Tiny]`) while the
            // live file names do (`07-U10S_07-By-…`): the trimmed number is the fallback
            let trimmed = num.trim_start_matches('0');
            if club.exists() || trimmed == num { club } else { root.join(round).join(set).join(&part).join(format!("U10S_{trimmed} By Everios96 [{}].Map.Gbx", label(set))) }
        } else {
            root.join(round).join(set).join(&part).join(&bname)
        };
        // absent, or a 0-byte leftover of an interrupted shoot (thumbs2/ice/p13, 2026-09-19)
        if std::fs::metadata(&f).map(|m| m.len() == 0).unwrap_or(true) {
            continue;
        }
        let is_wipe = round.starts_with("reset");
        if !is_wipe {
            // byte-identical to the previous version: nothing happened in this round
            let same = std::fs::metadata(&f).ok().zip(std::fs::metadata(&prev).ok()).map(|(a, b)| a.len() == b.len()).unwrap_or(false)
                && std::fs::read(&f).ok() == std::fs::read(&prev).ok();
            if same {
                continue;
            }
        }
        let a = match prev_fp.take() {
            Some(v) => v,
            None => fp.of(&prev)?,
        };
        let c = fp.of(&f)?;
        let (n, kinds) = fp_diff(&a, &c);
        if is_wipe {
            rows.push(format!("{set}\t{num}\t{round}\t{date}\t{n}\t{kinds}\twiped"));
            wiped = true;
        } else if n > 0 {
            rows.push(format!("{set}\t{num}\t{round}\t{date}\t{n}\t{kinds}\tafter-wipe:{}", if wiped { "yes" } else { "no" }));
        }
        prev = f;
        prev_fp = Some(c);
    }
    Ok(rows)
}
