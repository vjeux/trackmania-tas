//! `tinyctl rollout-publish` — the U10S alteration rollout, from a pre-built tree
//! to the hunt club, driven by a POLICY file. Nothing here is improvised at run
//! time: the plan is written out first, every line of it is checkable, and the
//! apply refuses to start from a plan that does not add up.
//!
//! ```text
//! tinyctl rollout-publish plan  --build ROOT --inventory INV.tsv --touch TOUCH.tsv
//!                               --policy POLICY.tsv --out PLAN.tsv [--sets dirt,…]
//!                               [--records REC.tsv] [--diff "Wet Wood=cmp.tsv;Wood=cmp.tsv"]
//! tinyctl rollout-publish apply --plan PLAN.tsv --club 145872 --token-dir DIR
//!                               [--only-sets dirt] [--dry-run] [--gate LOADLOOP_DIR[,…]]
//! ```
//!
//! Inputs
//! - ROOT: `rollout-build`'s tree (`build.tsv` + `build-retry.tsv`, one OK row per map-set).
//! - INV.tsv: the live inventory (`nadeo-here campaign` rows per hunt-club campaign:
//!   campaignId, campaign, pos, uid, name, …) — where every live map is and under which uid.
//! - TOUCH.tsv: `tmmaps ghost-touch --batch` output (uid, set, num, verdict TOUCH|CLEAR, …).
//! - POLICY.tsv: `set \t rule \t maps`, rules:
//!     `only     <maps>`  the set's scope: maps outside it are left alone (SKIP)
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

/// The new uid for a wipe of `live`: the set's prefix on the live tail — unless the live
/// uid ALREADY carries that prefix (a map wiped on 2026-10-10 wiped again for the fix
/// round of 10-18): then the next prefix in the chain, so the "new" uid is never the live
/// one (the clash check would only catch it after the plan was built; this is the rule).
fn wipe_uid(word: &str, live: &str) -> String {
    let p = uid_prefix(word);
    let tail = &live[4.min(live.len())..];
    if live.starts_with(p) {
        let next = match word {
            "Dirt" => "DirX",
            "Grass" => "GraX",
            "Icy" => "IceX",
            "Plastic" => "PlaX",
            "Magnet" => "MagX",
            "Wood" => "WooX",
            "Wet Wood" => "WetY",
            _ => "TstX",
        };
        return format!("{next}{tail}");
    }
    format!("{p}{tail}")
}

#[derive(Default)]
struct Policy {
    /// `only <maps>`: the set's SCOPE — a map outside it is left alone (SKIP "out of
    /// scope"): a Wet Wood map without a single water block converts to exactly what is
    /// live already (the walls were done in September), and 754 of 975 are like that
    /// (Everios96 2026-10-09: "Why update 912 maps? that seems a lot").
    only: Option<BTreeSet<u32>>,
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
            "only" => e.only.get_or_insert_with(BTreeSet::new).extend(maps),
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
    // `--records R.tsv` (a `nadeo-here leaderboards` pull: mapUid first column): the
    // COMPLETE "has records" set, by live uid. The touch file only knows the maps its
    // batch was built from (the 10-08 scope missed the DecoWall maps), so a live map
    // with records but no touch row would otherwise pass as "no records" — under a
    // wipe-all-with-records rule that is the difference between an update and a wipe.
    let records_by_uid: HashMap<String, usize> = f("--records")
        .map(|p| {
            let mut m: HashMap<String, usize> = HashMap::new();
            for l in read(&p).lines().skip(1) {
                if let Some(uid) = l.split('\t').next() {
                    if !uid.is_empty() {
                        *m.entry(uid.to_string()).or_default() += 1;
                    }
                }
            }
            m
        })
        .unwrap_or_default();
    let have_records_file = f("--records").is_some();
    // `--diff "Wet Wood=cmp/wetwood.tsv;Wood=cmp/wood.tsv;…"`: the live-vs-build files per
    // set word — a map whose row says SAME is identical to the live file and is SKIPped
    // ("live identical"), whatever the policy's scope says. Before this the `only` list
    // was typed from the diff by hand (2026-10-19); the diff IS the scope.
    let same_live: HashMap<(String, u32), bool> = f("--diff")
        .map(|spec| {
            let mut m: HashMap<(String, u32), bool> = HashMap::new();
            for part in spec.split(';').map(str::trim).filter(|s| !s.is_empty()) {
                let Some((word, path)) = part.split_once('=') else { panic!("--diff wants Set=cmp.tsv[;…], got `{part}`") };
                for l in read(path.trim()).lines().skip(1) {
                    let c: Vec<&str> = l.split('\t').collect();
                    if c.len() >= 2 {
                        if let Ok(n) = c[0].parse::<u32>() {
                            m.insert((word.trim().to_string(), n), c[1] == "SAME");
                        }
                    }
                }
            }
            m
        })
        .unwrap_or_default();
    let have_diff = f("--diff").is_some();
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
        let n_rec = records_by_uid.get(uid).copied().unwrap_or(0);
        let v_touch = verdict.get(&key).map(String::as_str);
        // the touch verdict when there is one; else the pull decides between
        // "records (unanalysed)" and "none"; without a pull, the touch file alone
        // With a records pull the PULL is the truth about "has records": a touch verdict
        // for a map whose live uid has no record now is stale (its records were wiped on
        // 2026-10-10 with the uid, the touch file predates that) — 45 ghost wipes on
        // 2026-10-19. The verdict still refines a map that HAS records.
        let v: &str = match (v_touch, have_records_file, n_rec) {
            (_, true, 0) => "none",
            (Some(t), _, _) => t,
            (None, true, n) if n > 0 => "records (unanalysed)",
            _ => "none",
        };
        let in_scope = |p: &Policy| p.only.as_ref().map(|o| o.contains(&num)).unwrap_or(true);
        let out_of_scope = !pol_set.map(in_scope).unwrap_or(true) || !pol_all.map(in_scope).unwrap_or(true);
        let live_same = same_live.get(&key).copied();
        let (action, new_uid, file, reason) = if has(|p| &p.exclude) {
            ("SKIP", String::new(), String::new(), "policy: excluded".to_string())
        } else if have_diff && live_same == Some(true) {
            ("SKIP", String::new(), String::new(), "live identical (live-vs-build SAME)".to_string())
        } else if have_diff && live_same.is_none() {
            ("SKIP", String::new(), String::new(), "not in the live-vs-build file".to_string())
        } else if out_of_scope {
            ("SKIP", String::new(), String::new(), "policy: out of scope (nothing to convert)".to_string())
        } else {
            match built.get(&key) {
                None => ("SKIP", String::new(), String::new(), "no built file".to_string()),
                Some(path) => {
                    let force_wipe = has(|p| &p.wipe) || (has(|p| &p.wipe_all_with_records) && v != "none");
                    let force_keep = has(|p| &p.keep);
                    if force_keep {
                        ("UPDATE", String::new(), path.clone(), format!("policy: keep (records {v})"))
                    } else if force_wipe || v == "TOUCH" {
                        let nu = wipe_uid(word, uid);
                        ("WIPE", nu, path.clone(), if force_wipe { format!("policy: wipe (records {v})") } else { "a record drives a changed surface".to_string() })
                    } else {
                        let why = match v {
                            "none" => "no records".to_string(),
                            "records (unanalysed)" => format!("{n_rec} record(s) NOT analysed by ghost-touch — review before shipping"),
                            _ => "records never touch a changed surface".to_string(),
                        };
                        ("UPDATE", String::new(), path.clone(), why)
                    }
                }
            }
        };
        *counts.entry((word.to_string(), action.to_string())).or_default() += 1;
        rows.push_str(&format!("{word}\t{num}\t{cid}\t{camp}\t{uid}\t{name}\t{action}\t{new_uid}\t{file}\t{reason}\n"));
    }
    // a WIPE's fresh uid must not already be online anywhere in the inventory (a prefix
    // reused from an earlier wipe round would make the "create" an UPDATE of that map)
    let inv_text = read(&inv);
    let online: BTreeSet<&str> = inv_text.lines().filter_map(|l| l.split('\t').nth(3)).collect();
    let mut clashes = 0;
    for l in rows.lines().skip(1) {
        let c: Vec<&str> = l.split('\t').collect();
        if c.len() >= 8 && c[6] == "WIPE" && online.contains(c[7]) {
            println!("CLASH: new uid {} for {} {} is already online — pick another prefix", c[7], c[0], c[1]);
            clashes += 1;
        }
    }
    if clashes > 0 {
        println!("plan NOT written: {clashes} uid clash(es)");
        return 1;
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
    // `--live-dir DIR`: the LIVE map files (any file names; indexed here by the uid in
    // their header). Every staged upload takes its live map's THUMBNAIL: the live wet
    // wood maps carry September's rendered wood view, the built files the original's
    // vanilla one (2026-10-09, U10S_39 side by side). Required: a missing live file
    // stops the run before anything moves.
    // `--live-root DIR` (2026-10-22): the live files of SEVERAL sets under one root
    // (`DIR/<set>/…`, each set dir as `--live-dir` takes it) — one apply over a multi-set
    // plan instead of a shell loop per set (vjeux: no shell logic, ever).
    let live_dir = f("--live-dir").or_else(|| f("--live-root")).map(PathBuf::from);
    let live_depth = if f("--live-root").is_some() { 2 } else { 1 };
    let live_optional = tmmaps::cli::has(args, "--live-optional");
    let mut live_by_uid: HashMap<String, PathBuf> = HashMap::new();
    if let Some(d) = &live_dir {
        // one level of `pNN/` part folders too (the `club-inventory --fetch-root` layout);
        // two under a `--live-root`
        let mut dirs = vec![d.clone()];
        let mut frontier = vec![d.clone()];
        for _ in 0..live_depth {
            let mut next = Vec::new();
            for dir in &frontier {
                for e in std::fs::read_dir(dir).unwrap_or_else(|e| panic!("{}: {e}", dir.display())).flatten() {
                    if e.path().is_dir() {
                        dirs.push(e.path());
                        next.push(e.path());
                    }
                }
            }
            frontier = next;
        }
        for dir in dirs {
            for e in std::fs::read_dir(&dir).into_iter().flatten().flatten() {
                let p = e.path();
                if p.extension().map(|x| x == "Gbx").unwrap_or(false) {
                    if let Some(uid) = header_uid(&p) {
                        live_by_uid.insert(uid, p);
                    }
                }
            }
        }
        println!("live files: {} indexed by uid", live_by_uid.len());
        if live_by_uid.is_empty() {
            eprintln!("--live-dir {}: no map files found (thumbnails would be lost)", d.display());
            return 2;
        }
    }
    let thumb_src = |live_uid: &str| -> Option<String> { live_by_uid.get(live_uid).map(|p| p.to_string_lossy().into_owned()) };

    // `--inventory INV.tsv --skip-foreign`: a live map another ACCOUNT uploaded cannot be
    // updated by this one (Nadeo: only the uploader may) — 23 of 6,094 on 2026-10-19 were
    // Everios96's own early uploads; publish-batch's owner guard refuses them and the
    // campaign STOPs. With the inventory (its `author` column) those rows are set aside
    // up front, listed in `foreign.tsv` in the workdir, and the rest of the campaign
    // proceeds. What to do with them is the club owner's call (new id, or his own
    // re-upload).
    let mut foreign_rows: Vec<Vec<String>> = Vec::new();
    let skip_foreign = tmmaps::cli::has(args, "--skip-foreign");
    let owner_by_uid: HashMap<String, String> = match f("--inventory") {
        Some(inv) => read(&inv)
            .lines()
            .skip(1)
            .filter_map(|l| {
                let c: Vec<&str> = l.split('\t').collect();
                // inventory column 10 is the record's `author` (free metadata); the
                // uploader is `submitter` (column 13 when present) — see batch.rs
                (c.len() > 10).then(|| (c[3].to_string(), c.get(13).filter(|s| !s.is_empty()).unwrap_or(&c[10]).to_string()))
            })
            .collect(),
        None => HashMap::new(),
    };
    let me_account = f("--me").unwrap_or_else(|| "29368eb1-dfa5-4d1d-b392-8b2411349f3c".into());
    if skip_foreign && owner_by_uid.is_empty() {
        eprintln!("--skip-foreign needs --inventory INV.tsv (the live owners)");
        return 2;
    }
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
        if skip_foreign {
            if let Some(owner) = owner_by_uid.get(&c[4]) {
                if owner != &me_account {
                    foreign_rows.push(c);
                    continue;
                }
            }
        }
        by_camp.entry((c[0].clone(), c[2].clone())).or_default().push(c);
    }
    if !foreign_rows.is_empty() {
        std::fs::create_dir_all(&work).ok();
        let mut t = String::from("set\tnum\tcampaignId\tlive_uid\tlive_name\taction\towner\n");
        for c in &foreign_rows {
            t.push_str(&format!("{}\t{}\t{}\t{}\t{}\t{}\t{}\n", c[0], c[1], c[2], c[4], c[5], c[6], owner_by_uid.get(&c[4]).cloned().unwrap_or_default()));
        }
        let fp = work.join("foreign.tsv");
        std::fs::write(&fp, t).ok();
        println!("skip-foreign: {} row(s) owned by another account set aside → {}", foreign_rows.len(), fp.display());
    }
    println!("apply: {} campaigns{}", by_camp.len(), if dry { " (DRY RUN)" } else { "" });
    // `--skip-done RESULTS.tsv` (one or more, comma-separated): campaigns whose every row
    // reads IDENTICAL in an earlier run's results are skipped — a token expiry STOPs the
    // apply mid-set, and the rerun must not touch what already landed (2026-10-19; the
    // 10-10 runs were resumed by hand-filtered plans).
    let mut done_camps: std::collections::HashSet<String> = std::collections::HashSet::new();
    // `--skip-done-prefix PATH`: every `PATH*/results.tsv` (the earlier passes' workdirs),
    // so a rerun names no list by hand (2026-10-22)
    let mut skip_lists: Vec<String> = f("--skip-done").map(|l| l.split(',').map(str::trim).filter(|p| !p.is_empty()).map(String::from).collect()).unwrap_or_default();
    let mut partial_rows: std::collections::HashSet<String> = std::collections::HashSet::new();
    if let Some(prefix) = f("--skip-done-prefix") {
        let pp = PathBuf::from(&prefix);
        let parent = pp.parent().map(|p| p.to_path_buf()).unwrap_or_else(|| PathBuf::from("."));
        let base = pp.file_name().map(|b| b.to_string_lossy().to_string()).unwrap_or_default();
        let mut found: Vec<String> = std::fs::read_dir(&parent)
            .into_iter()
            .flatten()
            .flatten()
            .filter(|e| e.file_name().to_string_lossy().starts_with(&base))
            .map(|e| e.path().join("results.tsv"))
            .filter(|r| r.is_file())
            .map(|r| r.to_string_lossy().to_string())
            .collect();
        found.sort();
        println!("skip-done-prefix: {} earlier results file(s)", found.len());
        skip_lists.extend(found);
        // a pass killed mid-campaign wrote no results.tsv row for that campaign; its
        // per-campaign `update-results.tsv` (publish-batch's own table: path, name, uid,
        // mapId, how, bytes, md5, verdict) still names every row that landed
        let mut partial = 0usize;
        for e in std::fs::read_dir(&parent).into_iter().flatten().flatten().filter(|e| e.file_name().to_string_lossy().starts_with(&base)) {
            for c in std::fs::read_dir(e.path()).into_iter().flatten().flatten() {
                let ur = c.path().join("update-results.tsv");
                if !ur.is_file() {
                    continue;
                }
                for l in read(&ur.to_string_lossy()).lines().skip(1) {
                    let col: Vec<&str> = l.split('\t').collect();
                    if col.len() >= 8 && col[4] == "update" && col[7].trim() == "IDENTICAL" {
                        partial_rows.insert(col[2].to_string());
                        partial += 1;
                    }
                }
            }
        }
        if partial > 0 {
            println!("skip-done-prefix: {partial} landed row(s) from per-campaign tables");
        }
    }
    // rows (by live uid) that an earlier pass already landed IDENTICAL: skipped one by one,
    // so a campaign that lost one upload to a server-side hang (Dirt 206, 2026-10-24: four
    // attempts, 0 bytes back) is not re-uploaded whole on the rerun
    let mut done_rows: std::collections::HashSet<String> = partial_rows;
    if !skip_lists.is_empty() {
        for path in skip_lists.iter().map(|s| s.as_str()) {
            let mut per: HashMap<String, (usize, usize)> = HashMap::new();
            for l in read(path).lines().skip(1) {
                let c: Vec<&str> = l.split('\t').collect();
                if c.len() >= 6 {
                    let e = per.entry(c[2].to_string()).or_insert((0, 0));
                    e.0 += 1;
                    if c[5].contains("IDENTICAL") {
                        e.1 += 1;
                        if c[3] == "UPDATE" {
                            done_rows.insert(c[4].to_string());
                        }
                    }
                }
            }
            for (cid, (n, ok)) in per {
                let planned = by_camp.iter().filter(|((_, c), _)| *c == cid).map(|(_, rows)| rows.len()).sum::<usize>();
                if n == ok && planned > 0 && n >= planned {
                    done_camps.insert(cid);
                }
            }
        }
        println!("skip-done: {} campaign(s) already applied, {} update row(s) already landed", done_camps.len(), done_rows.len());
    }
    let mut failed_updates = 0usize;
    // `--token-refresh` : before each campaign, if the token files are older than 40 min,
    // re-read the pair from the logged-in game on the box (`token --from-game`), so a
    // long apply never dies on the ~1 h expiry.
    let token_refresh = tmmaps::cli::has(args, "--token-refresh");
    // `--gate DIR[,DIR…]`: the load-gate tables (`tinyctl loadloop`: `loadloop-<tag>-cN.tsv`
    // + `links-<tag>.tsv` in each DIR). A campaign is applied only when EVERY file it would
    // publish was OPENED by the game in one of those tables; a campaign with a file not yet
    // gated, or gated as anything but OPENED, is held ("every shipped file passes a
    // play-load" — the 10-01 rule, now a flag instead of a reading of two files by hand).
    // `TINYCTL_BATCH_JOBS` (default 3): parallel uploads per campaign. 2026-10-24 (a Saturday
    // evening in Europe): one upload in six hung for minutes at Nadeo's end; six workers
    // keep the batch moving while two of them wait.
    let batch_jobs = std::env::var("TINYCTL_BATCH_JOBS").unwrap_or_else(|_| "3".into());
    let gated_ok: Option<std::collections::HashSet<String>> = f("--gate").map(|dirs| {
        let mut ok: std::collections::HashSet<String> = std::collections::HashSet::new();
        for dir in dirs.split(',').map(str::trim).filter(|d| !d.is_empty()) {
            // (tag, link name `0001.Map.Gbx`) → source file (the BOX path of the built map).
            // KEYED BY TAG: two loops in one dir (`links-ll12.tsv`, `links-ll12b.tsv`) number
            // their links from 0001 both, and a flat map let the second overwrite the first —
            // every gated file resolved to another loop's map and the whole apply was HELD
            // (2026-10-20 13:02, nothing shipped: the gate failed closed, as it should).
            let mut link_src: HashMap<(String, String), String> = HashMap::new();
            for entry in std::fs::read_dir(dir).into_iter().flatten().flatten() {
                let name = entry.file_name().to_string_lossy().to_string();
                if let Some(tag) = name.strip_prefix("links-").and_then(|n| n.strip_suffix(".tsv")) {
                    for l in read(&entry.path().to_string_lossy()).lines().skip(1) {
                        if let Some((link, src)) = l.split_once('\t') {
                            let base = link.rsplit('/').next().unwrap_or(link).to_string();
                            link_src.insert((tag.to_string(), base), src.to_string());
                        }
                    }
                }
            }
            for entry in std::fs::read_dir(dir).into_iter().flatten().flatten() {
                let name = entry.file_name().to_string_lossy().to_string();
                // `loadloop-<tag>-cN.tsv` (a chunk) or `loadloop-<tag>.tsv` (the merged table)
                if let Some(stem) = name.strip_prefix("loadloop-").and_then(|n| n.strip_suffix(".tsv")) {
                    let tag = match stem.rsplit_once("-c") {
                        Some((t, n)) if n.chars().all(|ch| ch.is_ascii_digit()) => t.to_string(),
                        _ => stem.to_string(),
                    };
                    for l in read(&entry.path().to_string_lossy()).lines().skip(1) {
                        let c: Vec<&str> = l.split('\t').collect();
                        if c.len() >= 3 && c[2] == "OPENED" {
                            if let Some(src) = link_src.get(&(tag.clone(), c[1].to_string())) {
                                // the gate knows the BOX path; the plan the LOCAL one — match by file name
                                ok.insert(src.rsplit('/').next().unwrap_or(src).to_string());
                            }
                        }
                    }
                }
            }
        }
        println!("gate: {} file(s) OPENED by the game", ok.len());
        ok
    });
    let results = work.join("results.tsv");
    let mut res = String::from("set\tnum\tcampaignId\taction\tuid\tverdict\n");
    for ((set, cid), rows) in &by_camp {
        let updates: Vec<&Vec<String>> = rows.iter().filter(|r| r[6] == "UPDATE").collect();
        let wipes: Vec<&Vec<String>> = rows.iter().filter(|r| r[6] == "WIPE").collect();
        if done_camps.contains(cid) {
            println!("[{set} {cid}] already applied — skipped");
            continue;
        }
        if let Some(ok) = &gated_ok {
            let ungated: Vec<String> = rows.iter().filter(|r| r[6] == "UPDATE" || r[6] == "WIPE").filter(|r| !ok.contains(r[8].rsplit('/').next().unwrap_or(&r[8]))).map(|r| r[1].clone()).collect();
            if !ungated.is_empty() {
                println!("[{set} {cid}] HELD: {} file(s) not yet OPENED by the load gate ({})", ungated.len(), ungated.iter().take(6).cloned().collect::<Vec<_>>().join(" "));
                for r in rows.iter().filter(|r| r[6] == "UPDATE" || r[6] == "WIPE") {
                    res.push_str(&format!("{}\t{}\t{}\t{}\t{}\tHELD-GATE\n", r[0], r[1], cid, r[6], r[4]));
                }
                continue;
            }
        }
        println!("[{set} {cid}] {} update, {} wipe", updates.len(), wipes.len());
        if dry {
            continue;
        }
        if token_refresh {
            // By the tokens' OWN expiry, not the file age: a pair read from the game
            // (`token --from-game`) carries the game's remaining life, which can be 30 min
            // on a file written a minute ago — Wood 156483 died on "no fresh token files"
            // 35 min after a refresh the age rule did not think was due (2026-10-20 13:3x).
            let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
            let left = ["NadeoServices", "NadeoLiveServices"]
                .iter()
                .map(|aud| std::fs::read_to_string(PathBuf::from(&token_dir).join(format!("token-{aud}.txt"))).ok().and_then(|t| crate::nadeo::jwt_exp(&t)).map(|e| e.saturating_sub(now)).unwrap_or(0))
                .min()
                .unwrap_or(0);
            if left < 15 * 60 {
                println!("  tokens have {} min left — refreshing from the game", left / 60);
                // the box game is between launches for ~40 s at a loadloop chunk edge
                // (only one of the two tokens in memory) — a few tries, 30 s apart
                // A game's token file holds the pair it got at LOGIN (an hour's life): from
                // a game up for hours `--from-game` hands back a near-dead pair, and with no
                // game it hands back nothing. Either way the standalone mint (its own game,
                // fresh pair, closed again; an agent's leftover game on an idle box is replaced)
                // is the fallback, 3 tries 60 s apart (2026-10-22: the 5-h Grass/Plastic pass).
                let read_left = || {
                    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
                    ["NadeoServices", "NadeoLiveServices"]
                        .iter()
                        .map(|aud| std::fs::read_to_string(PathBuf::from(&token_dir).join(format!("token-{aud}.txt"))).ok().and_then(|t| crate::nadeo::jwt_exp(&t)).map(|e| e.saturating_sub(now)).unwrap_or(0))
                        .min()
                        .unwrap_or(0)
                };
                let mut ok = false;
                for attempt in 1..=2 {
                    let st = std::process::Command::new(&me).args(["token", "--from-game", "--token-dir", &token_dir]).status();
                    if st.map(|s| s.success()).unwrap_or(false) && read_left() >= 20 * 60 {
                        ok = true;
                        break;
                    }
                    println!("  token refresh attempt {attempt} from the game gave no pair with 20 min left — retrying in 30 s");
                    std::thread::sleep(std::time::Duration::from_secs(30));
                }
                if !ok {
                    for attempt in 1..=3 {
                        println!("  minting a fresh pair standalone (attempt {attempt})");
                        let st = std::process::Command::new(&me).args(["mint-tokens", "--standalone", "--token-dir", &token_dir]).status();
                        if st.map(|s| s.success()).unwrap_or(false) && read_left() >= 20 * 60 {
                            ok = true;
                            break;
                        }
                        std::thread::sleep(std::time::Duration::from_secs(60));
                    }
                }
                if !ok {
                    println!("  STOP: token refresh failed (no logged-in game on the box and the standalone mint refused?) — rerun with --skip-done {}", results.display());
                    std::fs::write(&results, &res).ok();
                    return 1;
                }
                println!("  tokens refreshed: {} min left", read_left() / 60);
            }
        }
        let stage = work.join(format!("{}-{cid}", set.replace(' ', "")));
        std::fs::create_dir_all(&stage).ok();
        // UPDATE rows: the built file's uid is the deterministic one (`Wet2` + tail); the
        // LIVE uid may carry an earlier wipe's prefix (`Wet6`/`WetC`/`WetH`/`Wet8` — 974 of
        // 975 Wet Wood tails match, only the prefix differs). Re-id every update file to
        // the live uid first, so the upload is an in-place update of THAT map.
        let (updates, landed): (Vec<_>, Vec<_>) = updates.into_iter().partition(|r| !done_rows.contains(&r[4]));
        if !landed.is_empty() {
            println!("  {} update row(s) landed in an earlier pass — skipped", landed.len());
            for r in &landed {
                res.push_str(&format!("{}\t{}\t{cid}\tUPDATE\t{}\tIDENTICAL (earlier pass)\n", r[0], r[1], r[4]));
            }
        }
        if !updates.is_empty() {
            let mut man = String::new();
            for r in &updates {
                let dst = stage.join(format!("upd-{}.Map.Gbx", r[4]));
                let mut cmd = std::process::Command::new(&me);
                cmd.args(["__tmmaps-reuid", &r[8], &r[4], &dst.to_string_lossy()]);
                if live_dir.is_some() {
                    match thumb_src(&r[4]) {
                        Some(t) => {
                            cmd.arg(t);
                        }
                        None if live_optional => {
                            // `--live-optional` (2026-10-21): a map re-ided earlier today has no
                            // file in an older live dir; its build file carries the original's
                            // thumbnail, which is what every live file started from
                            println!("  note: no live file for {} {} ({}) — the build's own thumbnail stays", r[0], r[1], r[4]);
                        }
                        None => {
                            println!("  STOP: no live file for {} {} ({}) in the live dir", r[0], r[1], r[4]);
                            std::fs::write(&results, &res).ok();
                            return 1;
                        }
                    }
                }
                let st = cmd.status();
                if !st.map(|s| s.success()).unwrap_or(false) {
                    println!("  STOP: re-id to the live uid failed for {} {}", r[0], r[1]);
                    std::fs::write(&results, &res).ok();
                    return 1;
                }
                man.push_str(&format!("{}\t{}\n", dst.display(), r[5]));
            }
            let mp = stage.join("update-manifest.tsv");
            std::fs::write(&mp, man).unwrap();
            let rp = stage.join("update-results.tsv");
            let st = std::process::Command::new(&me)
                .args(["publish-batch", "--manifest", &mp.to_string_lossy(), "--results", &rp.to_string_lossy(), "--outdir", &stage.to_string_lossy(), "--jobs", &batch_jobs, "--token-dir", &token_dir, "--files-only", "--no-gate"])
                .status();
            let ok = count_identical(&rp);
            println!("  updates: {ok}/{} IDENTICAL ({:?})", updates.len(), st.map(|s| s.code()));
            for r in &updates {
                res.push_str(&format!("{}\t{}\t{cid}\tUPDATE\t{}\t{}\n", r[0], r[1], r[4], verdict_of(&rp, &r[4])));
            }
            if ok != updates.len() {
                if wipes.is_empty() {
                    // in-place updates are independent of each other: the rest of the run
                    // goes on, the missing rows are rerun with --skip-done-prefix
                    failed_updates += updates.len() - ok;
                    println!("  WARN: {} update(s) did not land in {set} {cid} — rerun later, see {}", updates.len() - ok, rp.display());
                    std::fs::write(&results, &res).ok();
                    continue;
                }
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
                let mut cmd = std::process::Command::new(&me);
                cmd.args(["__tmmaps-reuid", &r[8], &r[7], &dst.to_string_lossy()]);
                if live_dir.is_some() {
                    match thumb_src(&r[4]) {
                        Some(t) => {
                            cmd.arg(t);
                        }
                        None if live_optional => {
                            // `--live-optional` (2026-10-21): a map re-ided earlier today has no
                            // file in an older live dir; its build file carries the original's
                            // thumbnail, which is what every live file started from
                            println!("  note: no live file for {} {} ({}) — the build's own thumbnail stays", r[0], r[1], r[4]);
                        }
                        None => {
                            println!("  STOP: no live file for {} {} ({}) in the live dir", r[0], r[1], r[4]);
                            std::fs::write(&results, &res).ok();
                            return 1;
                        }
                    }
                }
                let st = cmd.status();
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
                .args(["publish-batch", "--manifest", &mp.to_string_lossy(), "--results", &rp.to_string_lossy(), "--outdir", &stage.to_string_lossy(), "--jobs", &batch_jobs, "--token-dir", &token_dir, "--files-only", "--no-gate"])
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
            // the swap reads its tokens from TINYCTL_TOKEN_DIR: hand it OUR token dir (the
            // parent took `--token-dir`, the child only knows the env — with no env it fell
            // back to the box-side minting path, which does not exist on the devserver, and
            // three playlists stayed on their old uids while the log said "swap: Some(1)";
            // Everios96 found 124 unchanged in the club, 2026-10-20 15:12). A failed swap is
            // a STOP, never a note.
            let st = std::process::Command::new(&me)
                .env("TINYCTL_TOKEN_DIR", &token_dir)
                .args(["nadeo-here", "reuid-swap", "--club", &club, "--pairs", &pp.to_string_lossy(), "--no-lock", "--outdir", &stage.to_string_lossy()])
                .status();
            let swapped = st.as_ref().map(|s| s.success()).unwrap_or(false);
            println!("  swap: {}", if swapped { "OK".to_string() } else { format!("FAILED ({:?})", st.map(|s| s.code())) });
            if !swapped {
                println!("  STOP: the playlist swap failed in {set} {cid} — the new maps exist but the campaign still lists the old uids; rerun `nadeo-here reuid-swap --pairs {}`", pp.display());
                std::fs::write(&results, &res).ok();
                return 1;
            }
        }
        std::fs::write(&results, &res).ok();
    }
    std::fs::write(&results, &res).ok();
    println!("results → {}", results.display());
    if failed_updates > 0 {
        println!("{failed_updates} update(s) did not land — rerun with --skip-done-prefix (the landed rows are skipped)");
        return 1;
    }
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

/// The uid in a map file's header, without loading the body.
fn header_uid(p: &Path) -> Option<String> {
    tmmaps::header::read(&p.to_string_lossy()).ok().map(|h| h.uid)
}

/// The header thumbnail's JPEG bytes of a map file, as stored.
fn thumbnail_of(p: &str) -> Result<Vec<u8>, String> {
    let bytes = std::fs::read(p).map_err(|e| format!("{p}: {e}"))?;
    let start = bytes.windows(15).position(|w| w == b"<Thumbnail.jpg>").ok_or_else(|| format!("{p}: no <Thumbnail.jpg> in the header"))? + 15;
    let end = start + bytes[start..].windows(16).position(|w| w == b"</Thumbnail.jpg>").ok_or_else(|| format!("{p}: no </Thumbnail.jpg>"))?;
    Ok(bytes[start..end].to_vec())
}

/// `tinyctl __tmmaps-reuid SRC NEWUID DST [LIVE]` — the re-id as a subcommand so `apply`
/// never shells out to another binary (one build, one version); with LIVE, the staged
/// file also takes LIVE's header thumbnail.
pub fn reuid(args: &[String]) -> i32 {
    let (src, new, dst) = (&args[1], &args[2], &args[3]);
    let live = args.get(4);
    let cur = tmmaps::header::read(src).map(|h| h.uid).unwrap_or_default();
    if cur == *new && live.is_none() {
        return match std::fs::copy(src, dst) {
            Ok(_) => 0,
            Err(e) => {
                eprintln!("{dst}: {e}");
                1
            }
        };
    }
    let mut m = tmmaps::map::MapFile::load(Path::new(src));
    if cur != *new {
        m.set_map_uid(new);
    }
    if let Some(l) = live {
        let jpeg = match thumbnail_of(l) {
            Ok(j) => j,
            Err(e) => {
                eprintln!("{e}");
                return 1;
            }
        };
        if let Err(e) = m.set_thumbnail(&jpeg) {
            eprintln!("{dst}: thumbnail: {e}");
            return 1;
        }
    }
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

/// `tinyctl rollout-publish create-set --build ROOT --set tech --club 145872 --prefix "Tarmac Part "
/// --token-dir D --workdir W [--parts 1-39] [--per-part 25] [--dry-run]` — a NEW alteration
/// into the hunt club (Everios96's Tarmac, 2026-10-24): per part, the campaign `<prefix>NN`
/// (reused by name when the club already has it, created + published otherwise) and ONE
/// `publish-batch --files-only` of the part's built maps in map order (create or update,
/// stored-bytes md5 readback) that writes the playlist. A part whose results file reads all
/// IDENTICAL and whose playlist holds every uid is skipped on a rerun. Tokens: minted
/// standalone before a part when the files have less than 25 min left.
pub fn create_set(args: &[String]) -> i32 {
    let f = |k: &str| tmmaps::cli::flag(args, k).map(String::from);
    let build = PathBuf::from(f("--build").unwrap_or_else(|| { eprintln!("create-set needs --build ROOT"); std::process::exit(2) }));
    let set = f("--set").unwrap_or_else(|| { eprintln!("create-set needs --set WORD (the build.tsv set column)"); std::process::exit(2) });
    let club = f("--club").unwrap_or_else(|| { eprintln!("create-set needs --club ID"); std::process::exit(2) });
    let prefix = f("--prefix").unwrap_or_else(|| { eprintln!("create-set needs --prefix \"Tarmac Part \""); std::process::exit(2) });
    let token_dir = f("--token-dir").unwrap_or_else(|| "/tmp/tinyctl-tokens".into());
    let workdir = PathBuf::from(f("--workdir").unwrap_or_else(|| build.join("create-set").to_string_lossy().into_owned()));
    let per_part: u32 = f("--per-part").and_then(|v| v.parse().ok()).unwrap_or(25);
    let dry = tmmaps::cli::has(args, "--dry-run");
    let parts: Vec<u32> = match f("--parts") {
        Some(p) => match p.split_once('-') {
            Some((a, b)) => (a.parse().unwrap_or(1)..=b.parse().unwrap_or(39)).collect(),
            None => p.split(',').filter_map(|x| x.trim().parse().ok()).collect(),
        },
        None => (1..=39).collect(),
    };
    std::fs::create_dir_all(&workdir).ok();
    let me = std::env::current_exe().unwrap_or_else(|_| PathBuf::from("tinyctl"));
    // the built files: build.tsv rows of this set, status OK
    let tsv = read(&build.join("build.tsv").to_string_lossy());
    let mut by_num: HashMap<u32, PathBuf> = HashMap::new();
    for l in tsv.lines().skip(1) {
        let c: Vec<&str> = l.split('\t').collect();
        if c.len() >= 5 && c[0] == set && c[4].starts_with("OK") {
            if let Ok(n) = c[1].parse::<u32>() {
                by_num.insert(n, PathBuf::from(c[2]));
            }
        }
    }
    println!("create-set: {} built {set} maps in {}", by_num.len(), build.display());
    let tokens = || -> Option<(String, String)> {
        let left = ["NadeoServices", "NadeoLiveServices"]
            .iter()
            .map(|aud| std::fs::read_to_string(PathBuf::from(&token_dir).join(format!("token-{aud}.txt"))).ok().and_then(|t| crate::nadeo::jwt_exp(&t)).map(|e| e.saturating_sub(std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0))).unwrap_or(0))
            .min()
            .unwrap_or(0);
        // `--files-only` wants files younger than 20 min: mint when under 25 min of life
        // or when the files are older than 15 min
        let age = std::fs::metadata(PathBuf::from(&token_dir).join("token-NadeoServices.txt")).ok().and_then(|m| m.modified().ok()).and_then(|t| t.elapsed().ok()).map(|d| d.as_secs()).unwrap_or(u64::MAX);
        if left < 25 * 60 || age > 15 * 60 {
            for attempt in 1..=3 {
                println!("  minting a fresh pair standalone (attempt {attempt}; {} min left, {} min old)", left / 60, age.min(99 * 60) / 60);
                let st = std::process::Command::new(&me).args(["mint-tokens", "--standalone", "--token-dir", &token_dir]).status();
                if st.map(|s| s.success()).unwrap_or(false) {
                    break;
                }
                std::thread::sleep(std::time::Duration::from_secs(60));
            }
        }
        crate::nadeo::fresh_tokens_in(&token_dir)
    };
    let Some((_core, live)) = tokens() else {
        println!("STOP: no fresh tokens in {token_dir}");
        return 1;
    };
    let auth_live = format!("Authorization: {live}");
    let camps = match crate::nadeo::club_campaigns(&auth_live, &club) {
        Ok(c) => c,
        Err(e) => {
            println!("STOP: club {club} campaigns: {e}");
            return 1;
        }
    };
    let mut failed = 0usize;
    for part in parts {
        let nn = format!("{part:02}");
        let cname = format!("{prefix}{nn}");
        let nums: Vec<u32> = ((part - 1) * per_part + 1..=part * per_part).filter(|n| by_num.contains_key(n)).collect();
        if nums.is_empty() {
            println!("[{cname}] no built maps — skipped");
            continue;
        }
        let stage = workdir.join(format!("part-{nn}"));
        std::fs::create_dir_all(&stage).ok();
        let results = stage.join("results.tsv");
        // the manifest: path<TAB>name (the file's own header name)
        let mut man = String::new();
        let mut uids: Vec<String> = Vec::new();
        let mut bad = false;
        for n in &nums {
            let p = &by_num[n];
            match tmmaps::header::read(&p.to_string_lossy()) {
                Ok(h) => {
                    man.push_str(&format!("{}\t{}\n", p.display(), h.name));
                    uids.push(h.uid.clone());
                }
                Err(e) => {
                    println!("[{cname}] {}: {e}", p.display());
                    bad = true;
                }
            }
        }
        if bad {
            failed += 1;
            continue;
        }
        // done already? every row IDENTICAL and the playlist holds the uids in order
        let done_rows = read(&results.to_string_lossy()).lines().skip(1).filter(|l| !l.starts_with('#') && l.ends_with("IDENTICAL")).count();
        let existing: Option<String> = camps.iter().find(|(_, n)| n == &cname).map(|(id, _)| id.clone());
        if done_rows == nums.len() {
            if let Some(id) = existing.as_ref() {
                if let Ok(pl) = crate::nadeo::campaign_playlist(&auth_live, &club, id) {
                    if pl == uids {
                        println!("[{cname}] {id}: {} maps already live in order — skipped", nums.len());
                        continue;
                    }
                }
            }
        }
        if dry {
            println!("[{cname}] {} maps {}..{} → campaign {}", nums.len(), nums[0], nums[nums.len() - 1], existing.clone().unwrap_or_else(|| "(new)".into()));
            continue;
        }
        let Some((_core, live)) = tokens() else {
            println!("STOP: no fresh tokens in {token_dir}");
            return 1;
        };
        let auth_live = format!("Authorization: {live}");
        let camp_id = match existing {
            Some(id) => id,
            None => match crate::nadeo::campaign_create(&auth_live, &club, &cname) {
                Ok((id, act)) => {
                    println!("[{cname}] created campaign {id} (activity {act})");
                    id
                }
                Err(e) => {
                    println!("[{cname}] campaign create failed: {e}");
                    failed += 1;
                    continue;
                }
            },
        };
        let mp = stage.join("manifest.tsv");
        std::fs::write(&mp, &man).ok();
        println!("[{cname}] {camp_id}: {} maps {}..{}", nums.len(), nums[0], nums[nums.len() - 1]);
        let st = std::process::Command::new(&me)
            .args(["publish-batch", "--manifest", &mp.to_string_lossy(), "--results", &results.to_string_lossy(), "--outdir", &stage.to_string_lossy(), "--files-only", "--token-dir", &token_dir, "--club", &club, "--campaign", &camp_id, "--campaign-name", &cname, "--jobs", &std::env::var("TINYCTL_BATCH_JOBS").unwrap_or_else(|_| "3".into())])
            .status();
        let ok = read(&results.to_string_lossy()).lines().skip(1).filter(|l| !l.starts_with('#') && l.ends_with("IDENTICAL")).count();
        println!("[{cname}] {ok}/{} IDENTICAL ({:?})", nums.len(), st.map(|s| s.code()));
        if ok != nums.len() {
            failed += 1;
        }
    }
    if failed > 0 {
        println!("{failed} part(s) incomplete — rerun the same command (done parts are skipped)");
        return 1;
    }
    0
}

pub fn cmd(args: &[String]) -> i32 {
    match args.get(1).map(String::as_str) {
        Some("plan") => plan(args),
        Some("apply") => apply(args),
        Some("create-set") => create_set(args),
        _ => {
            eprintln!("rollout-publish plan --build ROOT --inventory INV.tsv --touch TOUCH.tsv [--policy POLICY.tsv] --out PLAN.tsv\nrollout-publish apply --plan PLAN.tsv [--club 145872] [--token-dir DIR] [--only-sets dirt] [--dry-run]");
            2
        }
    }
}
