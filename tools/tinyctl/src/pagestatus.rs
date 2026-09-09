//! `tinyctl page-status` — make the page state the TRUE best lap per map, even
//! when its video is not published yet.
//!
//! ```text
//! tinyctl page-status --readme tiny/README.md --ghosts-dir DIR [--build ship15]
//!                     [--write] [--commit --repo DIR]
//! ```
//!
//! The page's caption line names the lap its VIDEO shows. The player project
//! improves laps all day, so between a new certified lap and its published clip
//! the page understates the campaign — on 2026-09-09 it showed 23 = 115.244
//! while the certified lap was 102.541, and 21 and 22 read *no lap yet* hours
//! after both had one. This adds one line under such a row:
//!
//! ```text
//! *latest lap **102.541** (build ship15) — video pending*
//! ```
//!
//! and removes it again when the video catches up, so the page converges on its
//! own. Idempotent: running it twice changes nothing. It also applies the
//! country names to 21–25 (vjeux, 2026-09-09) and turns *no lap yet* into
//! *no video yet* on a map that has one, because that is the difference between
//! "nobody has driven it" and "the clip is queued".
//!
//! Read-only unless `--write`; `--commit` needs `--repo`.

use std::path::{Path, PathBuf};

use crate::video::{lap_label, map_title};

const PENDING_MARK: &str = "— video pending*";

/// The newest certified lap per map on `build`, from the ghosts README: the
/// LAST row naming a map wins (the INPUT arm appends, it does not rewrite).
pub fn newest_laps(readme: &str, build: &str) -> Vec<(String, String)> {
    let mut out: std::collections::BTreeMap<String, String> = std::collections::BTreeMap::new();
    for l in readme.lines() {
        let c: Vec<&str> = l.split('|').map(|x| x.trim()).collect();
        // | map | file | time | … | build | … — the shapes vary, so match on the
        // first three cells and look for the build anywhere in the row
        if c.len() >= 5
            && c[1].len() == 2
            && c[1].chars().all(|x| x.is_ascii_digit())
            && c[2].ends_with(".Ghost.Gbx")
            && c[3].contains('.')
            && l.contains(build)
        {
            out.insert(c[1].to_string(), c[3].to_string());
        }
    }
    out.into_iter().collect()
}

/// What one page row says now.
#[derive(Debug, PartialEq)]
pub struct Row {
    pub nn: String,
    /// The lap the published video shows, if the row has one.
    pub published: Option<String>,
}

/// The page's rows, in page order, with the map number each belongs to.
fn rows_of(page: &str) -> Vec<(usize, Row)> {
    let mut v = Vec::new();
    for (i, l) in page.lines().enumerate() {
        if !l.starts_with("**Tiny ") {
            continue;
        }
        let title = l.trim_start_matches("**").split("**").next().unwrap_or("");
        let nn = (1..=25)
            .map(|n| format!("{n:02}"))
            .find(|nn| map_title(nn) == title || format!("Tiny Summer 2026 - {nn}") == title);
        let Some(nn) = nn else { continue };
        let published = l
            .split("ghost **")
            .nth(1)
            .and_then(|r| r.split("**").next())
            .map(str::to_string);
        v.push((i, Row { nn, published }));
    }
    v
}

/// The page with every row's status line brought up to date. Returns the new
/// text and one line per change, for the log.
pub fn update(page: &str, laps: &[(String, String)], ghosts_readme: &str, build: &str) -> (String, Vec<String>) {
    let mut lines: Vec<String> = page.lines().map(String::from).collect();
    let mut notes = Vec::new();
    // work from the bottom so earlier indices stay valid
    let rows = rows_of(page);
    for (i, row) in rows.into_iter().rev() {
        let nn = &row.nn;
        let newest = laps.iter().find(|(m, _)| m == nn).map(|(_, t)| t.clone());
        // the row's own line: the country names, and "no lap" vs "no video"
        let title = map_title(nn);
        let legacy = format!("**Tiny Summer 2026 - {nn}**");
        if lines[i].starts_with(&legacy) && legacy != format!("**{title}**") {
            lines[i] = lines[i].replacen(&legacy, &format!("**{title}**"), 1);
            notes.push(format!("{nn}: renamed to {title}"));
        }
        if lines[i].contains("*no lap yet*") && newest.is_some() {
            lines[i] = lines[i].replace("*no lap yet*", "*no video yet*");
            notes.push(format!("{nn}: has a lap now — 'no lap yet' → 'no video yet'"));
        }
        // where the existing status line is, if any: the next non-empty line
        // after the row that carries the marker
        let mut j = i + 1;
        while j < lines.len() && lines[j].trim().is_empty() {
            j += 1;
        }
        let existing = (j < lines.len() && lines[j].ends_with(PENDING_MARK)).then_some(j);
        let want = match (&newest, &row.published) {
            // a lap the page's video does not show
            (Some(n), Some(p)) if n != p => Some(n.clone()),
            (Some(n), None) => Some(n.clone()),
            _ => None,
        };
        match (want, existing) {
            (Some(t), Some(j)) => {
                let line = status_line(&t, build, ghosts_readme, nn);
                if lines[j] != line {
                    notes.push(format!("{nn}: pending line → {t}"));
                    lines[j] = line;
                }
            }
            (Some(t), None) => {
                let line = status_line(&t, build, ghosts_readme, nn);
                notes.push(format!("{nn}: pending line added ({t})"));
                lines.insert(i + 1, line);
                lines.insert(i + 1, String::new());
            }
            (None, Some(j)) => {
                notes.push(format!("{nn}: pending line removed (the video is current)"));
                lines.remove(j);
                // and the blank line that was holding it, if that leaves two
                if j > 0 && lines[j - 1].trim().is_empty() && j < lines.len() && lines[j].trim().is_empty() {
                    lines.remove(j - 1);
                }
            }
            (None, None) => {}
        }
    }
    let mut s = lines.join("\n");
    if page.ends_with('\n') && !s.ends_with('\n') {
        s.push('\n');
    }
    (s, notes)
}

fn status_line(time: &str, build: &str, ghosts_readme: &str, nn: &str) -> String {
    let label = lap_label(ghosts_readme, nn, time);
    let who = if label == "tiny ghost" { String::new() } else { format!(", {label}") };
    format!("*latest lap **{time}** (build {build}{who}) — video pending*")
}

pub fn cmd(args: &[String]) -> Result<(), String> {
    let f = |k: &str| tmmaps::cli::flag(args, k).map(String::from);
    let readme = PathBuf::from(f("--readme").ok_or("page-status needs --readme tiny/README.md")?);
    let ghosts_dir = PathBuf::from(f("--ghosts-dir").ok_or("page-status needs --ghosts-dir DIR")?);
    let build = f("--build").unwrap_or_else(|| "ship15".into());
    let page = std::fs::read_to_string(&readme).map_err(|e| format!("{}: {e}", readme.display()))?;
    let gr = std::fs::read_to_string(ghosts_dir.join("README.md")).map_err(|e| format!("{}/README.md: {e}", ghosts_dir.display()))?;
    let laps = newest_laps(&gr, &build);
    println!("{} certified {build} laps: {}", laps.len(), laps.iter().map(|(m, t)| format!("{m} {t}")).collect::<Vec<_>>().join(", "));
    let (new, notes) = update(&page, &laps, &gr, &build);
    if notes.is_empty() {
        println!("the page already states the newest lap of every map");
        return Ok(());
    }
    for n in &notes {
        println!("  {n}");
    }
    if !tmmaps::cli::has(args, "--write") {
        println!("(read-only; --write applies these {} change(s))", notes.len());
        return Ok(());
    }
    std::fs::write(&readme, &new).map_err(|e| format!("{}: {e}", readme.display()))?;
    println!("wrote {}", readme.display());
    if tmmaps::cli::has(args, "--commit") {
        let repo = PathBuf::from(f("--repo").ok_or("--commit needs --repo DIR")?);
        let rel = readme.strip_prefix(&repo).unwrap_or(&readme).display().to_string();
        let msg = format!(
            "tiny page: the true best lap per map — {} (a map whose newest certified lap is not published yet says so under its row)",
            notes.join("; ")
        );
        git(&repo, &["add", &rel])?;
        git(&repo, &["commit", "-q", "-m", &msg])?;
        git(&repo, &["pull", "-q", "--rebase"])?;
        git(&repo, &["push", "-q"])?;
        println!("pushed");
    }
    Ok(())
}

fn git(repo: &Path, args: &[&str]) -> Result<(), String> {
    let out = std::process::Command::new("git").arg("-C").arg(repo).args(args).output().map_err(|e| format!("git: {e}"))?;
    if !out.status.success() {
        return Err(format!("git {}: {}", args.join(" "), String::from_utf8_lossy(&out.stderr).trim()));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const GHOSTS: &str = "| map | file | time | credits | build | md5 | found by | validated |\n\
| 03 | 03.Ghost.Gbx | 28.989 | 5 | ship15 | 4d759865 | GEN | x |\n\
| 03 | 03.Ghost.Gbx | 19.793 | 5 | ship15 | 4d759865 | PPO | x |\n\
| 21 | 21.Ghost.Gbx | 116.384 | 17 | ship14 | 6858b37b | GEN | x |\n\
| 21 | 21.Ghost.Gbx | 122.510 | 17 | ship15 | 4feeaa5f | GEN | x |\n\
| 22 | 22.Ghost.Gbx | 99.912 | 14 | ship15 | f1275f23 | GEN | x |\n\
| 25 | 25.Ghost.Gbx | 121.235 | 15 | ship15 | bd1a146f | GEN | x |\n";

    const PAGE: &str = "# Tiny\n\nintro\n\n\
**Tiny Summer 2026 - 03** — original author time `24.213` · tiny ghost **28.989** (build ship15)\n\n\
https://github.com/user-attachments/assets/aaa\n\n\
**Tiny Summer 2026 - 20** — original author time `50.598` · *no lap yet*\n\n\
**Tiny Summer 2026 - 21** — original author time `78.988` · *no lap yet*\n\n\
**Tiny Japan 2026** — original author time `78.928` · tiny ghost **121.235** (build ship15, controls overlay)\n\n\
https://github.com/user-attachments/assets/ccc\n\n";

    #[test]
    fn the_newest_lap_of_each_map_wins_and_other_builds_are_ignored() {
        let laps = newest_laps(GHOSTS, "ship15");
        assert_eq!(laps.iter().find(|(m, _)| m == "03").unwrap().1, "19.793");
        assert_eq!(laps.iter().find(|(m, _)| m == "21").unwrap().1, "122.510", "the ship14 row must not win");
        assert_eq!(laps.len(), 4);
    }

    /// A map whose video is behind gains a line; a map whose video is current
    /// gains nothing; a map with no lap keeps *no lap yet*; a map with a lap and
    /// no video says *no video yet* and is renamed to its country.
    #[test]
    fn the_page_states_the_true_best_lap() {
        let laps = newest_laps(GHOSTS, "ship15");
        let (out, notes) = update(PAGE, &laps, GHOSTS, "ship15");
        assert!(out.contains("tiny ghost **28.989** (build ship15)\n\n*latest lap **19.793** (build ship15) — video pending*\n\nhttps://github.com/user-attachments/assets/aaa"), "{out}");
        assert!(out.contains("**Tiny Summer 2026 - 20** — original author time `50.598` · *no lap yet*"), "20 has no lap: {out}");
        assert!(!out.contains("20** — original author time `50.598` · *no lap yet*\n\n*latest"), "20 must not get a pending line");
        assert!(out.contains("**Tiny Argentina 2026** — original author time `78.988` · *no video yet*\n\n*latest lap **122.510** (build ship15) — video pending*"), "{out}");
        assert!(!out.contains("Tiny Japan 2026** — original author time `78.928` · tiny ghost **121.235** (build ship15, controls overlay)\n\n*latest"), "25's video is current");
        assert!(notes.iter().any(|n| n.contains("renamed")));
        // IDEMPOTENT: the second run is a no-op
        let (again, notes2) = update(&out, &laps, GHOSTS, "ship15");
        assert_eq!(again, out, "running twice must not change the page");
        assert!(notes2.is_empty(), "{notes2:?}");
    }

    /// And when the video catches up, the line goes away by itself.
    #[test]
    fn a_published_lap_removes_its_pending_line() {
        let laps = newest_laps(GHOSTS, "ship15");
        let (with, _) = update(PAGE, &laps, GHOSTS, "ship15");
        let published = with.replace("tiny ghost **28.989**", "tiny ghost **19.793**");
        let (out, notes) = update(&published, &laps, GHOSTS, "ship15");
        assert!(!out.contains("*latest lap **19.793**"), "{out}");
        assert!(out.contains("https://github.com/user-attachments/assets/aaa"), "the video stays: {out}");
        assert!(notes.iter().any(|n| n.contains("03: pending line removed")), "{notes:?}");
        // the other maps' lines are untouched
        assert!(out.contains("*latest lap **122.510**"));
    }

    /// A playtest lap says who drove it, here as well as in a published row.
    #[test]
    fn a_playtest_lap_is_named_in_the_pending_line() {
        let g = format!("{GHOSTS}| 04 | 04.Ghost.Gbx | 18.476 | regenerated on ship15 (vjeux playtest) | ship15 | x | x | x |\n");
        let page = "**Tiny Summer 2026 - 04** — original author time `26.622` · tiny ghost **29.474** (build ship15)\n";
        let laps = newest_laps(&g, "ship15");
        let (out, _) = update(page, &laps, &g, "ship15");
        assert!(out.contains("*latest lap **18.476** (build ship15, driven by vjeux (playtest)) — video pending*"), "{out}");
    }
}
