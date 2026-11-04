//! `tinyctl rethumb-wipe-all` — the thumbnail wipes over every set, one set at a time, each
//! set started when its census is complete:
//!
//! ```text
//! tinyctl rethumb-wipe-all --lists DIR --census REC.tsv --done DONE.txt --work DIR
//!                          [--sets dirt,giant,…] [--plain-sets giant,tiny]
//! ```
//!
//! Per set (in order): wait until every uid of `DIR/<set>.tsv` is in DONE.txt (the census's
//! progress file), write `WORK/<set>-norecord.tsv` (uids absent from REC.tsv = no record),
//! tell Everios96 the count, run `rethumb-wipe` on it (giant/tiny plain renders), tell him
//! when the set is done. Everios96 2026-11-03 09:53: "Do it for maps that dont have any
//! records."
use std::collections::HashSet;
use std::path::PathBuf;
use std::process::Command;
use std::time::Duration;

fn tinyctl() -> PathBuf {
    std::env::current_exe().unwrap_or_else(|_| PathBuf::from("tinyctl"))
}

fn discord(msg: &str) {
    let cmd = format!("~/bin/discordctl send 1548403109309579264 '{}'", msg.replace('\'', ""));
    let _ = Command::new("/home/vjeux/bin/whitestick").arg(&cmd).output();
}

fn census_sets(rec: &std::path::Path, done: &std::path::Path) -> (HashSet<String>, HashSet<String>) {
    let d: HashSet<String> = std::fs::read_to_string(done).unwrap_or_default().lines().map(|l| l.trim().to_string()).filter(|l| !l.is_empty()).collect();
    let r: HashSet<String> = std::fs::read_to_string(rec).unwrap_or_default().lines().skip(1).filter_map(|l| l.split('\t').next().map(String::from)).collect();
    (d, r)
}

pub fn cmd(args: &[String]) -> Result<(), String> {
    let f = |k: &str| tmmaps::cli::flag(args, k).map(String::from);
    let lists = PathBuf::from(f("--lists").ok_or("rethumb-wipe-all needs --lists DIR")?);
    let rec = PathBuf::from(f("--census").ok_or("rethumb-wipe-all needs --census REC.tsv")?);
    let done = PathBuf::from(f("--done").ok_or("rethumb-wipe-all needs --done DONE.txt")?);
    let work = PathBuf::from(f("--work").ok_or("rethumb-wipe-all needs --work DIR")?);
    let sets: Vec<String> = f("--sets").map(|s| s.split(',').map(|x| x.trim().to_string()).collect()).unwrap_or_else(|| vec!["dirt", "giant", "grass", "icy", "magnet", "plastic", "tarmac", "tiny", "wetwood", "wood"].into_iter().map(String::from).collect());
    let plain: Vec<String> = f("--plain-sets").map(|s| s.split(',').map(|x| x.trim().to_string()).collect()).unwrap_or_else(|| vec!["giant".into(), "tiny".into()]);
    let log_path = work.join("wipe-all.log");
    let log = |line: &str| {
        let stamp = crate::rethumball::chrono_like();
        println!("[{stamp}] {line}");
        if let Ok(mut fh) = std::fs::OpenOptions::new().create(true).append(true).open(&log_path) {
            use std::io::Write;
            let _ = writeln!(fh, "[{stamp}] {line}");
        }
    };
    for set in &sets {
        let rows: Vec<(String, String)> = std::fs::read_to_string(lists.join(format!("{set}.tsv")))
            .map_err(|e| format!("{set}: {e}"))?
            .lines()
            .filter(|l| !l.trim().is_empty())
            .map(|l| {
                let mut it = l.split('\t');
                (it.next().unwrap_or("").to_string(), it.next().unwrap_or("").to_string())
            })
            .collect();
        let set_work = work.join(set);
        if set_work.join("set.done").exists() {
            continue;
        }
        // the census for this set
        let mut announced_wait = false;
        let norec: Vec<(String, String)> = loop {
            let (d, r) = census_sets(&rec, &done);
            let counted = rows.iter().filter(|(u, _)| d.contains(u)).count();
            if counted == rows.len() {
                break rows.iter().filter(|(u, _)| !r.contains(u)).cloned().collect();
            }
            if !announced_wait {
                log(&format!("{set}: census {counted}/{} — waiting", rows.len()));
                announced_wait = true;
            }
            std::thread::sleep(Duration::from_secs(120));
        };
        std::fs::create_dir_all(&set_work).map_err(|e| e.to_string())?;
        let list = work.join(format!("{set}-norecord.tsv"));
        std::fs::write(&list, norec.iter().map(|(u, n)| format!("{u}\t{n}\n")).collect::<String>()).map_err(|e| e.to_string())?;
        log(&format!("{set}: {} maps, {} without a record → wipe, {} keep", rows.len(), norec.len(), rows.len() - norec.len()));
        if !set_work.join("announced").exists() {
            discord(&format!("{} counted: {} of {} maps have no record - those get new uids + new menu pictures now; {} keep uid and records. -- ai", cap(set), norec.len(), rows.len(), rows.len() - norec.len()));
            let _ = std::fs::write(set_work.join("announced"), "1");
        }
        if norec.is_empty() {
            let _ = std::fs::write(set_work.join("set.done"), "0");
            continue;
        }
        let mut attempt = 0;
        loop {
            attempt += 1;
            let mut c = Command::new(tinyctl());
            // 80 per chunk: the render is ~7 s/map whatever the chunk, but the swap scan, the
            // token check and the gate are per chunk (~6 min) — 40-map chunks ran 24 s/map,
            // 80-map chunks ~15 s/map (2026-11-04)
            c.args(["rethumb-wipe", "--uids-file", &list.to_string_lossy(), "--work", &set_work.to_string_lossy(), "--chunk", "80"]);
            if plain.iter().any(|p| p == set) {
                c.arg("--no-promote");
            }
            let st = c.status().map_err(|e| e.to_string())?;
            if st.success() {
                break;
            }
            log(&format!("{set}: rethumb-wipe exited {st} (attempt {attempt}) — again in 5 min"));
            if attempt >= 12 {
                log(&format!("{set}: GIVING UP after {attempt} attempts"));
                discord(&format!("Thumbnails ({}): stopped after repeated failures; I will look at it. -- ai", cap(set)));
                return Err(format!("{set}: rethumb-wipe failed {attempt} times"));
            }
            std::thread::sleep(Duration::from_secs(300));
        }
        let swapped: usize = std::fs::read_dir(&set_work)
            .map(|rd| rd.flatten().filter(|e| e.path().join("swap.done").exists()).map(|e| std::fs::read_to_string(e.path().join("pairs.tsv")).map(|t| t.lines().count()).unwrap_or(0)).sum())
            .unwrap_or(0);
        let _ = std::fs::write(set_work.join("set.done"), swapped.to_string());
        log(&format!("{set}: done — {swapped} maps wiped to new uids"));
        discord(&format!("Thumbnails: {} done - {} maps now have new uids and new menu pictures (swapped into their campaigns and rooms). -- ai", cap(set), swapped));
    }
    log("rethumb-wipe-all: every set done");
    Ok(())
}

fn cap(s: &str) -> String {
    match s {
        "wetwood" => "Wet Wood".to_string(),
        _ => {
            let mut c = s.chars();
            match c.next() {
                Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
                None => String::new(),
            }
        }
    }
}
