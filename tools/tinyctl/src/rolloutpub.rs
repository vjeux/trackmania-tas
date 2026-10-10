//! `tinyctl rollout-publish` — the U10S alteration rollout, from a pre-built tree
//! to the hunt club, driven by a POLICY file. Nothing here is improvised at run
//! time: the plan is written out first, every line of it is checkable, and the
//! apply refuses to start from a plan that does not add up.
//!
//! ```text
//! tinyctl rollout-publish plan  --build ROOT --inventory INV.tsv --touch TOUCH.tsv
//!                               --policy POLICY.tsv --out PLAN.tsv [--sets dirt,…]
//! tinyctl rollout-publish apply --plan PLAN.tsv --club 145872 --token-dir DIR
//!                               [--only-sets dirt] [--dry-run]
//! ```
//!
//! Inputs
//! - ROOT: `rollout-build`'s tree (`build.tsv` + `build-retry.tsv`, one OK row per map-set).
//! - INV.tsv: the live inventory (`nadeo-here campaign` rows per hunt-club campaign:
//!   campaignId, campaign, pos, uid, name, …) — where every live map is and under which uid.
//! - TOUCH.tsv: `tmmaps ghost-touch --batch` output (uid, set, num, verdict TOUCH|CLEAR, …).
//! - POLICY.tsv: `set \t rule \t maps`, rules:
//!     `exclude  <maps>`  leave these maps of the set alone entirely (no update, no wipe)
//!     `wipe     <maps>`  force a wipe (new uid) for these, whatever the touch verdict
//!     `keep     <maps>`  force an in-place update (no wipe) for these, whatever the verdict
//!     `wipe-all-with-records <range>`  every map in the range with any record wipes
//!   `<maps>` is `1-50,52,60-70`; `set` is the inventory's set word (Dirt, Wet Wood, …) or `*`.
//!
//! The plan: one row per live map-set — action UPDATE (same uid, publish in place),
//! WIPE (re-uid to a fresh prefix, create, then swap into the campaign playlist),
//! SKIP (excluded, or no build) — with the reason. `apply` runs UPDATE rows through
//! `publish-batch` (the live-map guard allows an update that keeps the name), WIPE
//! rows through create + `reuid-swap`, campaign by campaign, and writes results.tsv.
//! It stops at the first campaign whose creates are not all IDENTICAL (the September
//! rule: never swap a playlist to maps that did not land).

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::{Path, PathBuf};

fn read(p: &str) -> String {
    std::fs::read_to_string(p).unwrap_or_else(|e| panic!("{p}: {e}"))
}

fn parse_maps(spec: &str) -> BTreeSet<u32> {
    let mut v = BTreeSet::new();
    for part in spec.split(',') {
        let part = part.trim();
        if part.is_empty() {
            continue;
        }
        if let Some((a, b)) = part.split_once('-') {
            if let (Ok(a), Ok(b)) = (a.trim().parse::<u32>(), b.trim().parse::<u32>()) {
                v.extend(a..=b);
            }
        } else if let Ok(n) = part.parse::<u32>() {
            v.insert(n);
        }
    }
    v
}

/// the inventory's set word for a build set name
fn set_word(build_set: &str) -> &'static str {
    match build_set {
        "dirt" => "Dirt",
        "grass" => "Grass",
        "ice" => "Icy",
        "plastic" => "Plastic",
        "magnet" => "Magnet",
        "wood" => "Wood",
        "woodwet" => "Wet Wood",
        _ => "?",
    }
}

fn uid_prefix(word: &str) -> &'static str {
    // a FRESH prefix per set for the wipe round of 2026-10-10 (none of these were ever
    // used: the chain so far is X4/X5/X8/Pla7/Gra7/Wet6/WetC/XB…)
    match word {
        "Dirt" => "DirW",
        "Grass" => "GraW",
        "Icy" => "IceW",
        "Plastic" => "PlaW",
        "Magnet" => "MagW",
        "Wood" => "WooW",
        "Wet Wood" => "WetX",
        _ => "TstW",
    }
}

#[derive(Default)]
struct Policy {
    exclude: BTreeSet<u32>,
    wipe: BTreeSet<u32>,
    keep: BTreeSet<u32>,
    wipe_all_with_records: BTreeSet<u32>,
}

fn load_policy(p: &str) -> HashMap<String, Policy> {
    let mut out: HashMap<String, Policy> = HashMap::new();
    for l in read(p).lines() {
        if l.trim().is_empty() || l.starts_with('#') {
            continue;
        }
        let c: Vec<&str> = l.split('\t').collect();
        if c.len() < 3 {
            panic!("policy row needs set<TAB>rule<TAB>maps: {l:?}");
        }
        let maps = parse_maps(c[2]);
        let e = out.entry(c[0].trim().to_string()).or_default();
        match c[1].trim() {
            "exclude" => e.exclude.extend(maps),
            "wipe" => e.wipe.extend(maps),
            "keep" => e.keep.extend(maps),
            "wipe-all-with-records" => e.wipe_all_with_records.extend(maps),
            r => panic!("policy: unknown rule {r:?}"),
        }
    }
    out
}

pub fn plan(args: &[String]) -> i32 {
    let f = |k: &str| tmmaps::cli::flag(args, k).map(String::from);
    let root = PathBuf::from(f("--build").expect("--build ROOT"));
    let inv = f("--inventory").expect("--inventory INV.tsv");
    let touch = f("--touch").expect("--touch TOUCH.tsv");
    let policy = f("--policy").map(|p| load_policy(&p)).unwrap_or_default();
    let out = f("--out").expect("--out PLAN.tsv");
    let sets: Vec<String> = f("--sets").unwrap_or_else(|| "dirt,grass,ice,plastic,magnet,wood,woodwet".into()).split(',').map(|s| s.trim().to_string()).collect();

    // built files: (set word, num) -> path
    let mut built: HashMap<(String, u32), String> = HashMap::new();
    for tsv in ["build.tsv", "build-retry.tsv"] {
        let p = root.join(tsv);
        if let Ok(t) = std::fs::read_to_string(&p) {
            for l in t.lines().skip(1) {
                let c: Vec<&str> = l.split('\t').collect();
                if c.len() >= 5 && c[4].starts_with("OK") {
                    if let Ok(n) = c[1].parse::<u32>() {
                        built.insert((set_word(c[0]).to_string(), n), c[2].to_string());
                    }
                }
            }
        }
    }
    // records: (set word, num) -> TOUCH|CLEAR|none
    let mut verdict: HashMap<(String, u32), String> = HashMap::new();
    for l in read(&touch).lines().skip(1) {
        let c: Vec<&str> = l.split('\t').collect();
        if c.len() >= 4 {
            if let Ok(n) = c[2].parse::<u32>() {
                verdict.insert((c[1].to_string(), n), c[3].to_string());
            }
        }
    }
    // live inventory rows
    let mut rows = String::from("set\tnum\tcampaignId\tcampaign\tlive_uid\tlive_name\taction\tnew_uid\tfile\treason\n");
    let mut counts: BTreeMap<(String, String), usize> = BTreeMap::new();
    let num_re = |name: &str| -> Option<u32> {
        let i = name.find("U10S_")?;
        name[i + 5..].chars().take_while(|c| c.is_ascii_digit()).collect::<String>().parse().ok()
    };
    for l in read(&inv).lines() {
        let c: Vec<&str> = l.split('\t').collect();
        if c.len() < 5 || c[0] == "DONE" {
            continue;
        }
        let (cid, camp, uid, name) = (c[0], c[1], c[3], c[4]);
        let word = camp.rsplit_once(" Part").map(|(w, _)| w.trim()).unwrap_or("");
        if !sets.iter().any(|s| set_word(s) == word) {
            continue;
        }
        let Some(num) = num_re(name) else { continue };
        let pol_set = policy.get(word);
        let pol_all = policy.get("*");
        let has = |g: fn(&Policy) -> &BTreeSet<u32>| pol_set.map(|p| g(p).contains(&num)).unwrap_or(false) || pol_all.map(|p| g(p).contains(&num)).unwrap_or(false);
        let key = (word.to_string(), num);
        let v = verdict.get(&key).map(String::as_str).unwrap_or("none");
        let (action, new_uid, file, reason) = if has(|p| &p.exclude) {
            ("SKIP", String::new(), String::new(), "policy: excluded".to_string())
        } else {
            match built.get(&key) {
                None => ("SKIP", String::new(), String::new(), "no built file".to_string()),
                Some(path) => {
                    let force_wipe = has(|p| &p.wipe) || (has(|p| &p.wipe_all_with_records) && v != "none");
                    let force_keep = has(|p| &p.keep);
                    if force_keep {
                        ("UPDATE", String::new(), path.clone(), format!("policy: keep (records {v})"))
                    } else if force_wipe || v == "TOUCH" {
                        let nu = format!("{}{}", uid_prefix(word), &uid[4.min(uid.len())..]);
                        ("WIPE", nu, path.clone(), if force_wipe { format!("policy: wipe (records {v})") } else { "a record drives a changed surface".to_string() })
                    } else {
                        ("UPDATE", String::new(), path.clone(), if v == "none" { "no records".to_string() } else { "records never touch a changed surface".to_string() })
                    }
                }
            }
        };
        *counts.entry((word.to_string(), action.to_string())).or_default() += 1;
        rows.push_str(&format!("{word}\t{num}\t{cid}\t{camp}\t{uid}\t{name}\t{action}\t{new_uid}\t{file}\t{reason}\n"));
    }
    std::fs::write(&out, &rows).unwrap_or_else(|e| panic!("{out}: {e}"));
    println!("plan → {out}");
    let mut by_set: BTreeMap<String, (usize, usize, usize)> = BTreeMap::new();
    for ((s, a), n) in &counts {
        let e = by_set.entry(s.clone()).or_default();
        match a.as_str() {
            "UPDATE" => e.0 += n,
            "WIPE" => e.1 += n,
            _ => e.2 += n,
        }
    }
    println!("{:<9} {:>7} {:>6} {:>6}", "set", "update", "wipe", "skip");
    let mut tot = (0, 0, 0);
    for (s, (u, w, k)) in &by_set {
        println!("{s:<9} {u:>7} {w:>6} {k:>6}");
        tot.0 += u;
        tot.1 += w;
        tot.2 += k;
    }
    println!("{:<9} {:>7} {:>6} {:>6}", "TOTAL", tot.0, tot.1, tot.2);
    0
}

pub fn apply(args: &[String]) -> i32 {
    let f = |k: &str| tmmaps::cli::flag(args, k).map(String::from);
    let plan = f("--plan").expect("--plan PLAN.tsv");
    let club = f("--club").unwrap_or_else(|| "145872".into());
    let token_dir = f("--token-dir").unwrap_or_else(|| "/tmp/tinyctl-tokens".into());
    let dry = tmmaps::cli::has(args, "--dry-run");
    let only: Option<Vec<String>> = f("--only-sets").map(|s| s.split(',').map(|x| set_word(x.trim()).to_string()).collect());
    let work = PathBuf::from(f("--workdir").unwrap_or_else(|| "/tmp/rollout-apply".into()));
    std::fs::create_dir_all(&work).ok();
    let me = std::env::current_exe().expect("own path");

    // rows by campaign, in plan order
    let mut by_camp: BTreeMap<(String, String), Vec<Vec<String>>> = BTreeMap::new();
    for l in read(&plan).lines().skip(1) {
        let c: Vec<String> = l.split('\t').map(String::from).collect();
        if c.len() < 10 || c[6] == "SKIP" {
            continue;
        }
        if let Some(o) = &only {
            if !o.contains(&c[0]) {
                continue;
            }
        }
        by_camp.entry((c[0].clone(), c[2].clone())).or_default().push(c);
    }
    println!("apply: {} campaigns{}", by_camp.len(), if dry { " (DRY RUN)" } else { "" });
    let results = work.join("results.tsv");
    let mut res = String::from("set\tnum\tcampaignId\taction\tuid\tverdict\n");
    for ((set, cid), rows) in &by_camp {
        let updates: Vec<&Vec<String>> = rows.iter().filter(|r| r[6] == "UPDATE").collect();
        let wipes: Vec<&Vec<String>> = rows.iter().filter(|r| r[6] == "WIPE").collect();
        println!("[{set} {cid}] {} update, {} wipe", updates.len(), wipes.len());
        if dry {
            continue;
        }
        let stage = work.join(format!("{}-{cid}", set.replace(' ', "")));
        std::fs::create_dir_all(&stage).ok();
        // UPDATE rows: the built file carries the live uid already (same source → same uid)
        if !updates.is_empty() {
            let mut man = String::new();
            for r in &updates {
                man.push_str(&format!("{}\t{}\n", r[8], r[5]));
            }
            let mp = stage.join("update-manifest.tsv");
            std::fs::write(&mp, man).unwrap();
            let rp = stage.join("update-results.tsv");
            let st = std::process::Command::new(&me)
                .args(["publish-batch", "--manifest", &mp.to_string_lossy(), "--results", &rp.to_string_lossy(), "--outdir", &stage.to_string_lossy(), "--jobs", "3", "--token-dir", &token_dir, "--files-only", "--no-gate"])
                .status();
            let ok = count_identical(&rp);
            println!("  updates: {ok}/{} IDENTICAL ({:?})", updates.len(), st.map(|s| s.code()));
            for r in &updates {
                res.push_str(&format!("{}\t{}\t{cid}\tUPDATE\t{}\t{}\n", r[0], r[1], r[4], verdict_of(&rp, &r[4])));
            }
            if ok != updates.len() {
                println!("  STOP: not every update landed in {set} {cid}; nothing swapped, see {}", rp.display());
                std::fs::write(&results, &res).ok();
                return 1;
            }
        }
        // WIPE rows: re-uid → create → swap pairs
        if !wipes.is_empty() {
            let mut man = String::new();
            let mut pairs = String::new();
            for r in &wipes {
                let dst = stage.join(format!("{}.Map.Gbx", r[7]));
                let st = std::process::Command::new(&me).args(["__tmmaps-reuid", &r[8], &r[7], &dst.to_string_lossy()]).status();
                if !st.map(|s| s.success()).unwrap_or(false) {
                    println!("  STOP: re-uid failed for {} {}", r[0], r[1]);
                    std::fs::write(&results, &res).ok();
                    return 1;
                }
                man.push_str(&format!("{}\t{}\n", dst.display(), r[5]));
                pairs.push_str(&format!("{}\t{}\n", r[4], r[7]));
            }
            let mp = stage.join("wipe-manifest.tsv");
            std::fs::write(&mp, man).unwrap();
            let pp = stage.join("pairs.tsv");
            std::fs::write(&pp, &pairs).unwrap();
            let rp = stage.join("wipe-results.tsv");
            let _ = std::process::Command::new(&me)
                .args(["publish-batch", "--manifest", &mp.to_string_lossy(), "--results", &rp.to_string_lossy(), "--outdir", &stage.to_string_lossy(), "--jobs", "3", "--token-dir", &token_dir, "--files-only", "--no-gate"])
                .status();
            let ok = count_identical(&rp);
            println!("  wipes: {ok}/{} created IDENTICAL", wipes.len());
            for r in &wipes {
                res.push_str(&format!("{}\t{}\t{cid}\tWIPE\t{}\t{}\n", r[0], r[1], r[7], verdict_of(&rp, &r[7])));
            }
            if ok != wipes.len() {
                println!("  STOP: not every new map was created in {set} {cid}; playlist NOT swapped, see {}", rp.display());
                std::fs::write(&results, &res).ok();
                return 1;
            }
            let st = std::process::Command::new(&me)
                .args(["nadeo-here", "reuid-swap", "--club", &club, "--pairs", &pp.to_string_lossy(), "--no-lock", "--outdir", &stage.to_string_lossy()])
                .status();
            println!("  swap: {:?}", st.map(|s| s.code()));
        }
        std::fs::write(&results, &res).ok();
    }
    std::fs::write(&results, &res).ok();
    println!("results → {}", results.display());
    0
}

fn count_identical(rp: &Path) -> usize {
    std::fs::read_to_string(rp).map(|t| t.lines().skip(1).filter(|l| l.split('\t').nth(7) == Some("IDENTICAL")).count()).unwrap_or(0)
}

fn verdict_of(rp: &Path, uid: &str) -> String {
    std::fs::read_to_string(rp)
        .ok()
        .and_then(|t| t.lines().skip(1).find(|l| l.split('\t').nth(2) == Some(uid)).map(|l| l.split('\t').nth(7).unwrap_or("?").to_string()))
        .unwrap_or_else(|| "missing".into())
}

/// `tinyctl __tmmaps-reuid SRC NEWUID DST` — the re-id as a subcommand so `apply` never
/// shells out to another binary (one build, one version).
pub fn reuid(args: &[String]) -> i32 {
    let (src, new, dst) = (&args[1], &args[2], &args[3]);
    let mut m = tmmaps::map::MapFile::load(Path::new(src));
    m.set_map_uid(new);
    match m.write_to(Path::new(dst)) {
        Ok(()) => {
            let back = tmmaps::header::read(dst).map(|h| h.uid).unwrap_or_default();
            if back == *new {
                0
            } else {
                eprintln!("reuid: wrote {dst} but it reads back as {back}");
                1
            }
        }
        Err(e) => {
            eprintln!("{dst}: {e}");
            1
        }
    }
}

pub fn cmd(args: &[String]) -> i32 {
    match args.get(1).map(String::as_str) {
        Some("plan") => plan(args),
        Some("apply") => apply(args),
        _ => {
            eprintln!("rollout-publish plan --build ROOT --inventory INV.tsv --touch TOUCH.tsv [--policy POLICY.tsv] --out PLAN.tsv\nrollout-publish apply --plan PLAN.tsv [--club 145872] [--token-dir DIR] [--only-sets dirt] [--dry-run]");
            2
        }
    }
}
