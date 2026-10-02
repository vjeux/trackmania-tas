//! `shootctl rdcensus --set NAME [--set NAME …] [--table-only] [--timeout-s 300] [--detach]` — a RenderDoc CENSUS of
//! EVERY .rdc of a capture set (rdexport.py `census`: the root-action walk per (flags, render targets, depth) class —
//! seconds per frame, no replay, no lock, no game), one qrenderdoc.exe at a time (cmd.exe /c qr.cmd waits for it),
//! written as <set>/census/st_frameN/census.json, then TABULATED: <set>/census/census.tsv (one row per frame × class:
//! set, frame, MB, actions, n, inst, idx, first, last, class) and <set>/census/census.md (one row per frame: MB,
//! actions, classes, the three biggest classes) — the frame table a README needs before a set is deleted
//! (baker-8, 2026-10-01: the day-3/4 stpad sets stfin / stprep / stfin2, 195 frames, 10.7 GB). Resumable: a frame
//! whose census.json exists is skipped; `--table-only` re-tabulates without exporting; `--detach` runs in the
//! background with the log in C:\tools\cap\census-<set>.log (poll it; "ALL DONE" ends it).
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

const CAP_WSL: &str = "/mnt/c/tools/cap";
const CAP_WIN: &str = "C:/tools/cap";
const RDPY_WIN: &str = "C:\\tools\\rdpy";

fn flag_all<'a>(args: &'a [String], k: &str) -> Vec<&'a str> {
    args.iter().enumerate().filter(|(_, a)| a.as_str() == k).filter_map(|(i, _)| args.get(i + 1)).map(|s| s.as_str()).collect()
}

pub fn run(args: &[String]) -> i32 {
    let sets: Vec<String> = flag_all(args, "--set").iter().map(|s| s.to_string()).collect();
    let table_only = args.iter().any(|a| a == "--table-only");
    let timeout = Duration::from_secs(flag_all(args, "--timeout-s").first().and_then(|v| v.parse().ok()).unwrap_or(300));
    if sets.is_empty() {
        eprintln!("usage: shootctl rdcensus --set NAME [--set NAME …] [--table-only] [--timeout-s 300] [--detach]");
        return 2;
    }
    if args.iter().any(|a| a == "--detach") {
        let log = PathBuf::from(format!("{CAP_WSL}/census-{}.log", sets.join("-")));
        let done = PathBuf::from(format!("{CAP_WSL}/census-{}.done", sets.join("-")));
        return super::shootset::detach_as(&log, &done);
    }
    for set in &sets {
        if let Err(e) = census_set(set, table_only, timeout) {
            eprintln!("rdcensus {set}: {e}");
            return 1;
        }
    }
    println!("ALL DONE ({} set(s))", sets.len());
    0
}

/// `st_frame4591.rdc` → 4591
fn frame_no(p: &Path) -> Option<u32> {
    let s = p.file_name()?.to_str()?;
    s.strip_prefix("st_frame")?.strip_suffix(".rdc")?.parse().ok()
}

fn census_set(set: &str, table_only: bool, timeout: Duration) -> Result<(), String> {
    let dir = PathBuf::from(format!("{CAP_WSL}/{set}"));
    let cdir = dir.join("census");
    std::fs::create_dir_all(&cdir).map_err(|e| format!("mkdir {}: {e}", cdir.display()))?;
    let mut rdcs: Vec<(u32, PathBuf, u64)> = std::fs::read_dir(&dir).map_err(|e| format!("{}: {e}", dir.display()))?
        .filter_map(|e| e.ok()).map(|e| e.path())
        .filter_map(|p| { let n = frame_no(&p)?; let sz = std::fs::metadata(&p).ok()?.len(); Some((n, p, sz)) })
        .collect();
    rdcs.sort_by_key(|r| r.0);
    println!("{set}: {} .rdc, {} MB", rdcs.len(), rdcs.iter().map(|r| r.2).sum::<u64>() / 1_048_576);
    if !table_only {
        for (i, (fr, _p, sz)) in rdcs.iter().enumerate() {
            let out = cdir.join(format!("st_frame{fr}"));
            if out.join("census.json").exists() { continue; }
            std::fs::create_dir_all(&out).map_err(|e| format!("mkdir {}: {e}", out.display()))?;
            let t0 = Instant::now();
            let rd_args = format!("{CAP_WIN}/{set}/st_frame{fr}.rdc|{CAP_WIN}/{set}/census/st_frame{fr}|census");
            let mut child = std::process::Command::new("/mnt/c/Windows/System32/cmd.exe")
                .args(["/c", &format!("{RDPY_WIN}\\qr.cmd"), &format!("{RDPY_WIN}\\rdexport.py")])
                .env("WSLENV", "RD_ARGS:RD_LOG").env("RD_ARGS", &rd_args).env("RD_LOG", format!("{CAP_WIN}/{set}/census/rdexport-census.log"))
                .stdout(std::process::Stdio::null()).stderr(std::process::Stdio::null())
                .spawn().map_err(|e| format!("cmd.exe: {e}"))?;
            // cmd.exe /c qr.cmd waits for qrenderdoc.exe (verified 2026-10-01: census.json is there when it returns); the
            // timeout is the guard against a hung replay — kill cmd.exe, note it, go on (the frame stays uncensused)
            let status = loop {
                match child.try_wait() {
                    Ok(Some(st)) => break Some(st),
                    Ok(None) => {
                        if t0.elapsed() > timeout { let _ = child.kill(); let _ = child.wait(); break None; }
                        std::thread::sleep(Duration::from_millis(250));
                    }
                    Err(e) => return Err(format!("wait cmd.exe: {e}")),
                }
            };
            let ok = out.join("census.json").exists();
            println!("[{}/{}] {set} st_frame{fr} ({} MB): {} in {:.1} s{}", i + 1, rdcs.len(), sz / 1_048_576,
                if ok { "census.json" } else { "NO census.json" }, t0.elapsed().as_secs_f64(),
                match status { Some(st) if !st.success() => format!(" (cmd.exe exit {st})"), None => " (TIMEOUT, killed)".to_string(), _ => String::new() });
        }
    }
    tabulate(set, &cdir, &rdcs)
}

#[derive(Default, Clone)]
struct Class { key: String, n: u64, inst: u64, idx: u64, first: u64, last: u64 }

/// census.json = {"actions": N, "classes": [{"key": "…", "n": …, "inst": …, "idx": …, "first": …, "last": …}, …]} —
/// the keys carry no quotes (RenderDoc flag names, resource ids, sizes, format names), so a scan suffices
fn parse_census(text: &str) -> Option<(u64, Vec<Class>)> {
    let actions = num_after(text, "\"actions\":")?;
    let mut classes = Vec::new();
    let mut rest = text;
    while let Some(i) = rest.find("\"key\": \"") {
        let after = &rest[i + 8..];
        let j = after.find('"')?;
        let key = after[..j].to_string();
        let obj = &after[j..];
        let end = obj.find('}').unwrap_or(obj.len());
        let o = &obj[..end];
        classes.push(Class {
            key,
            n: num_after(o, "\"n\":").unwrap_or(0),
            inst: num_after(o, "\"inst\":").unwrap_or(0),
            idx: num_after(o, "\"idx\":").unwrap_or(0),
            first: num_after(o, "\"first\":").unwrap_or(0),
            last: num_after(o, "\"last\":").unwrap_or(0),
        });
        rest = &obj[end..];
    }
    Some((actions, classes))
}

fn num_after(text: &str, key: &str) -> Option<u64> {
    let i = text.find(key)? + key.len();
    let t = text[i..].trim_start();
    let d: String = t.chars().take_while(|c| c.is_ascii_digit()).collect();
    d.parse().ok()
}

fn tabulate(set: &str, cdir: &Path, rdcs: &[(u32, PathBuf, u64)]) -> Result<(), String> {
    let mut tsv = String::from("set\tframe\tmb\tactions\tn\tinst\tidx\tfirst\tlast\tclass\n");
    let mut md = format!("| frame | MB | actions | classes | the three biggest classes (actions × class) |\n|---|---|---|---|---|\n");
    let (mut done, mut missing) = (0usize, Vec::new());
    // frame GROUPS: consecutive frames whose class-key SET (counts ignored) is the same = one phase of the compute
    // (pre-pass / lamps / sweep / finalisation / editor); each group = (first, last, n, MB lo..hi, actions lo..hi, keys)
    let mut groups: Vec<(u32, u32, usize, u64, u64, u64, u64, Vec<String>)> = Vec::new();
    for (fr, _p, sz) in rdcs {
        let f = cdir.join(format!("st_frame{fr}")).join("census.json");
        let Ok(text) = std::fs::read_to_string(&f) else { missing.push(*fr); continue; };
        let Some((actions, classes)) = parse_census(&text) else { missing.push(*fr); continue; };
        done += 1;
        let mb = sz / 1_048_576;
        let mut keys: Vec<String> = classes.iter().map(|c| c.key.clone()).collect();
        keys.sort();
        match groups.last_mut() {
            Some(g) if g.7 == keys && g.1 + 1 == *fr => { g.1 = *fr; g.2 += 1; g.3 = g.3.min(mb); g.4 = g.4.max(mb); g.5 = g.5.min(actions); g.6 = g.6.max(actions); }
            _ => groups.push((*fr, *fr, 1, mb, mb, actions, actions, keys)),
        }
        for c in &classes {
            tsv.push_str(&format!("{set}\t{fr}\t{mb}\t{actions}\t{}\t{}\t{}\t{}\t{}\t{}\n", c.n, c.inst, c.idx, c.first, c.last, c.key));
        }
        let mut top = classes.clone();
        top.sort_by(|a, b| b.n.cmp(&a.n));
        let top3: Vec<String> = top.iter().take(3).map(|c| format!("{}× {}", c.n, c.key.replace('|', "/"))).collect();
        md.push_str(&format!("| {fr} | {mb} | {actions} | {} | {} |\n", classes.len(), top3.join(" · ")));
    }
    let head = format!("# census — set {set}: {} frames censused of {} .rdc ({} MB){}\n\n", done, rdcs.len(),
        rdcs.iter().map(|r| r.2).sum::<u64>() / 1_048_576,
        if missing.is_empty() { String::new() } else { format!("; MISSING: {missing:?}") });
    let mut gmd = String::from("## frame groups (consecutive frames with the same class set)\n\n| frames | n | MB | actions | classes (flags / render targets / depth) |\n|---|---|---|---|---|\n");
    for (a, b, n, mblo, mbhi, aclo, achi, keys) in &groups {
        let ks: Vec<String> = keys.iter().map(|k| k.replace('|', "/")).collect();
        gmd.push_str(&format!("| {} | {n} | {} | {} | {} |\n", if a == b { a.to_string() } else { format!("{a}–{b}") },
            if mblo == mbhi { mblo.to_string() } else { format!("{mblo}–{mbhi}") },
            if aclo == achi { aclo.to_string() } else { format!("{aclo}–{achi}") }, ks.join(" · ")));
    }
    write_whole(&cdir.join("census.tsv"), &tsv)?;
    write_whole(&cdir.join("census.md"), &format!("{head}{gmd}\n## per frame\n\n{md}"))?;
    println!("{set}: {} frames tabulated → {}/census.{{tsv,md}}{}", done, cdir.display(), if missing.is_empty() { String::new() } else { format!("; missing {missing:?}") });
    Ok(())
}

fn write_whole(p: &Path, s: &str) -> Result<(), String> {
    let tmp = PathBuf::from(format!("{}.tmp", p.display()));
    std::fs::write(&tmp, s).map_err(|e| format!("{}: {e}", tmp.display()))?;
    std::fs::rename(&tmp, p).map_err(|e| format!("{}: {e}", p.display()))
}
