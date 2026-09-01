//! `clip inventory --verify` -- is each page's headline ghost actually its own run?
//!
//! WHY THIS EXISTS. Of the first seven headline ghosts checked by hand, FIVE
//! carried a recording that is not their tape's run -- kappa 0.151, 0.382,
//! 0.495, 0.500, 0.521 -- while the plain oracle re-simulated every one of
//! those tapes to the time on the page. So the results are sound and the films
//! show a different car. That is not a tail of stragglers, it is most of the
//! sample, and it means no page can be trusted to be filmable without a check.
//!
//! Verification costs seconds per file and needs no render box, so the whole
//! corpus is checked at once and the answer written down, rather than
//! discovered one map at a time by whoever next tries to film one.
//!
//! HOW THE HEADLINE GHOST IS FOUND, and why it can say "ambiguous": the page's
//! caption gives the run's time, and the file whose name carries those
//! milliseconds is the headline file (`TAS_23416`, `best_7998`,
//! `m270051_4830`). Nothing in the repo records the mapping explicitly. Where
//! no filename carries the time, or more than one does, this says so instead of
//! picking -- verifying the wrong file would produce a confident answer about a
//! file nobody publishes.

use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::inventory::{read_root, MapPage};

/// What `ghost verify` said about one page's headline ghost.
pub struct Audited {
    pub page: MapPage,
    pub ghost: Option<PathBuf>,
    pub map: Option<PathBuf>,
    /// V6, the chance-corrected agreement between the tape and the recording.
    pub kappa: Option<f64>,
    /// V7, the plain oracle on the WRITTEN file.
    pub oracle: Option<String>,
    pub verdict: String,
}

/// Milliseconds from a caption time like `23.416`.
fn ms_of(t: &str) -> Option<u64> {
    let t: String = t.trim().chars().take_while(|c| c.is_ascii_digit() || *c == '.').collect();
    let f: f64 = t.parse().ok()?;
    Some((f * 1000.0).round() as u64)
}

/// The replay whose name carries these milliseconds.
fn headline_ghost(dir: &Path, ms: u64) -> Result<PathBuf, String> {
    let rd = std::fs::read_dir(dir.join("replays")).map_err(|e| format!("replays: {e}"))?;
    let key = ms.to_string();
    let mut hits: Vec<PathBuf> = rd
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| {
            p.to_string_lossy().ends_with(".Ghost.Gbx")
                && p.file_name()
                    .and_then(|n| n.to_str())
                    .is_some_and(|n| n.contains(&key))
        })
        .collect();
    hits.sort();
    match hits.len() {
        0 => Err(format!("no replay filename carries {key}")),
        1 => Ok(hits.remove(0)),
        // A regenerated replacement sits beside the original it replaces, and
        // it is the one that gets filmed.
        _ => {
            if let Some(r) = hits.iter().find(|p| p.to_string_lossy().contains("regen")) {
                return Ok(r.clone());
            }
            Err(format!(
                "{} replays carry {key}: {}",
                hits.len(),
                hits.iter()
                    .filter_map(|p| p.file_name().and_then(|n| n.to_str()))
                    .collect::<Vec<_>>()
                    .join(", ")
            ))
        }
    }
}

/// The map file for a page, on the shared store.
fn map_for(store: &Path, id: &str) -> Option<PathBuf> {
    let d = store.join(id);
    let mut hits: Vec<PathBuf> = std::fs::read_dir(&d)
        .ok()?
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.to_string_lossy().ends_with(".Map.Gbx"))
        .collect();
    hits.sort();
    // A segment or rig map is not the map the page is about.
    hits.iter().find(|p| {
        let n = p.file_name().unwrap_or_default().to_string_lossy().to_lowercase();
        !n.contains("seg") && !n.contains("spawn") && !n.contains("backup")
    })
    .cloned()
    .or_else(|| hits.first().cloned())
}

/// The `ghost` binary beside this one.
fn ghost_binary() -> PathBuf {
    if let Ok(v) = std::env::var("GHOST_BIN") {
        return PathBuf::from(v);
    }
    std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|d| d.join("ghost")))
        .unwrap_or_else(|| PathBuf::from("ghost"))
}

fn audit_one(page: MapPage, root: &Path, store: &Path) -> Audited {
    let id = page.dir.split('-').next().unwrap_or("").to_string();
    let Some(ms) = page.headline.as_ref().and_then(|c| ms_of(&c.tas)) else {
        return Audited {
            page,
            ghost: None,
            map: None,
            kappa: None,
            oracle: None,
            verdict: "the page states no headline time".into(),
        };
    };
    let ghost = match headline_ghost(&root.join(&page.dir), ms) {
        Ok(g) => g,
        Err(e) => {
            return Audited { page, ghost: None, map: None, kappa: None, oracle: None, verdict: e }
        }
    };
    let Some(map) = map_for(store, &id) else {
        return Audited {
            page,
            ghost: Some(ghost),
            map: None,
            kappa: None,
            oracle: None,
            verdict: format!("no map on the store at {}/{id}", store.display()),
        };
    };
    let out = Command::new(ghost_binary())
        .arg("verify")
        .arg(&ghost)
        .arg("--map")
        .arg(&map)
        // Ask for the MACHINE rendering. This used to read the human one and
        // split on the literal strings "kappa ", "file: " and "re-simulated",
        // so rewording any gate message silently produced `None` here instead
        // of an error -- an audit that reports "unchecked" because a sentence
        // changed is worse than one that fails.
        .arg("-o")
        .arg("json")
        .output();
    let text = match out {
        Ok(o) => String::from_utf8_lossy(&o.stdout).to_string(),
        Err(e) => {
            return Audited {
                page,
                ghost: Some(ghost),
                map: Some(map),
                kappa: None,
                oracle: None,
                verdict: format!("cannot run ghost: {e}"),
            }
        }
    };
    let mut kappa = None;
    let mut oracle = None;
    let mut verdict = "no verdict line".to_string();
    // The report is `{"checks": [{"id", "verdict", "message"}, ...], "pass": N,
    // "fail": N, ...}`. Gate IDENTITY and VERDICT now come from the structure;
    // only the two numbers still come out of a message, because they live
    // nowhere else yet. Those two are the remaining fragility, and they are
    // now confined to the gates that own them rather than to line scanning.
    let checks = crate::audit_json::checks(&text);
    for (id, v, msg) in &checks {
        if id == "V6" {
            if let Some(k) = msg.split("kappa ").nth(1).and_then(|s| s.split_whitespace().next())
            {
                kappa = k.parse().ok();
            }
        }
        if id == "V7" {
            oracle = Some(match v.as_str() {
                "na" => "n/a".to_string(),
                _ if msg.contains("DNF") => "DNF".to_string(),
                _ => msg
                    .split("file: ")
                    .nth(1)
                    .unwrap_or("?")
                    .split_whitespace()
                    .next()
                    .unwrap_or("?")
                    .to_string(),
            });
        }
    }
    if !checks.is_empty() {
        let fails: Vec<&String> = checks
            .iter()
            .filter(|(_, v, _)| v == "fail")
            .map(|(id, _, _)| id)
            .collect();
        verdict = if fails.is_empty() {
            "OK".into()
        } else {
            format!("REFUSED: {} failed", fails.iter().map(|s| s.as_str()).collect::<Vec<_>>().join(" "))
        };
    }
    Audited { page, ghost: Some(ghost), map: Some(map), kappa, oracle, verdict }
}

/// `clip inventory --verify [--store D]`
pub fn main(root: &Path, store: &Path, markdown: bool) -> Result<(), String> {
    let pages = read_root(root)?;
    let mut rows = Vec::new();
    for p in pages {
        rows.push(audit_one(p, root, store));
    }

    let mut out = String::new();
    if markdown {
        out.push_str("| map | TAS | kappa (V6) | oracle (V7) | verdict |\n|---|---|---|---|---|\n");
    }
    let (mut sound, mut foreign, mut unchecked) = (0, 0, 0);
    for r in &rows {
        let tas = r.page.headline.as_ref().map(|c| c.tas.clone()).unwrap_or_else(|| "?".into());
        let k = r.kappa.map(|k| format!("{k:.3}")).unwrap_or_else(|| "-".into());
        let o = r.oracle.clone().unwrap_or_else(|| "-".into());
        match (r.kappa, r.verdict.as_str()) {
            (Some(k), _) if k >= 0.999 => sound += 1,
            (Some(_), _) => foreign += 1,
            _ => unchecked += 1,
        }
        if markdown {
            let flag = match r.kappa {
                Some(k) if k >= 0.999 => "",
                Some(_) => " **CARRIES ANOTHER RUN**",
                None => "",
            };
            let _ = writeln!(
                out,
                "| {} | {} | {} | {} | {}{} |",
                r.page.name, tas, k, o, r.verdict, flag
            );
        } else {
            let _ = writeln!(out, "{:<40} TAS {:>10}  kappa {:>6}  oracle {:>10}  {}", r.page.dir, tas, k, o, r.verdict);
        }
    }
    print!("{out}");
    eprintln!(
        "\n{} pages: {sound} whose recording IS their tape's run, {foreign} carrying another run, \
         {unchecked} not checkable.",
        rows.len()
    );
    Ok(())
}
