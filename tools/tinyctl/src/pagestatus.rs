//! `tinyctl page-status` — make the page state the TRUE best lap per map, even
//! when its video is not published yet.
//!
//! ```text
//! tinyctl page-status --readme tiny/README.md --ghosts-dir DIR [--build ship15] [--out DIR (for holds.tsv)]
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

use crate::video::PENDING_MARK;

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
    update_with_gain(page, laps, ghosts_readme, build, 0.1)
}

/// [`update`] with the re-render threshold spelled out: a newest lap that beats
/// the published one by less than `min_gain` seconds gets the "within" note (the
/// loop will not render it), anything else the "video pending" note.
pub fn update_with_gain(page: &str, laps: &[(String, String)], ghosts_readme: &str, build: &str, min_gain: f64) -> (String, Vec<String>) {
    update_full(page, laps, ghosts_readme, build, min_gain, &std::collections::HashMap::new())
}

/// [`update_with_gain`] plus the publish holds (`holds.tsv`): a held map's newer
/// lap reads "held (<reason>)" whatever its gain — the loop renders and ships
/// nothing for it until the hold is lifted.
pub fn update_full(page: &str, laps: &[(String, String)], ghosts_readme: &str, build: &str, min_gain: f64, holds: &std::collections::HashMap<String, String>) -> (String, Vec<String>) {
    update_all(page, laps, ghosts_readme, build, min_gain, holds, &std::collections::HashSet::new())
}

/// [`update_full`] plus the STAGED laps (rendered, banked, waiting for the
/// opening-check receipt): such a lap reads "staged — awaiting opening check".
pub fn update_all(page: &str, laps: &[(String, String)], ghosts_readme: &str, build: &str, min_gain: f64, holds: &std::collections::HashMap<String, String>, staged: &std::collections::HashSet<(String, String)>) -> (String, Vec<String>) {
    update_page(page, laps, ghosts_readme, build, min_gain, holds, staged, &std::collections::HashMap::new())
}

/// [`update_all`] plus the LID ROWS (`lidrows.tsv`: maps whose lap rides the
/// ship15 water lid, from INPUT's census): each gets its own italic line under
/// the row, kept beside the status note, removed when the map leaves the list.
#[allow(clippy::too_many_arguments)]
pub fn update_page(page: &str, laps: &[(String, String)], ghosts_readme: &str, build: &str, min_gain: f64, holds: &std::collections::HashMap<String, String>, staged: &std::collections::HashSet<(String, String)>, lidrows: &std::collections::HashMap<String, String>) -> (String, Vec<String>) {
    update_rows(page, laps, ghosts_readme, build, min_gain, holds, staged, lidrows, &std::collections::HashMap::new())
}

/// [`update_page`] plus the per-row BUILDS (`rowbuilds.tsv`): the row's build
/// tag, the downloadable map file of that build, and a note under the row.
#[allow(clippy::too_many_arguments)]
pub fn update_rows(page: &str, laps: &[(String, String)], ghosts_readme: &str, build: &str, min_gain: f64, holds: &std::collections::HashMap<String, String>, staged: &std::collections::HashSet<(String, String)>, lidrows: &std::collections::HashMap<String, String>, rowbuilds: &std::collections::HashMap<String, RowBuild>) -> (String, Vec<String>) {
    update_rows_with_clips(page, laps, ghosts_readme, build, min_gain, holds, staged, lidrows, rowbuilds, &std::collections::HashMap::new())
}

/// [`update_rows`] knowing which CLIP each row's published video is (map → clip
/// name, from ships.tsv), so a row build applies only when the video is a clip
/// of that build.
#[allow(clippy::too_many_arguments)]
pub fn update_rows_with_clips(page: &str, laps: &[(String, String)], ghosts_readme: &str, build: &str, min_gain: f64, holds: &std::collections::HashMap<String, String>, staged: &std::collections::HashSet<(String, String)>, lidrows: &std::collections::HashMap<String, String>, rowbuilds: &std::collections::HashMap<String, RowBuild>, ships_names: &std::collections::HashMap<String, String>) -> (String, Vec<String>) {
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
        // where the existing status line is, if any: ANYWHERE in the row's
        // block (to the next row). It used to be "the next non-empty line",
        // and a row swap that put the new URL right under the row hid the line
        // below it — neither updated nor removed, and a second one added on
        // top (2026-09-10). Extra pending lines and extra asset lines (the
        // previous video, left by the same swap) are repaired here as well:
        // one pending line at most, the FIRST asset line kept.
        let end = crate::video::block_end(&lines, i);
        let is_note = |l: &str| is_status_note(l);
        let pendings: Vec<usize> = (i + 1..end).filter(|&j| is_note(&lines[j])).collect();
        let assets: Vec<usize> = (i + 1..end).filter(|&j| lines[j].starts_with(crate::video::ASSET_PREFIX)).collect();
        let mut remove: Vec<usize> = Vec::new();
        if assets.len() > 1 {
            notes.push(format!("{nn}: {} asset lines in the block — keeping the first (the newest video)", assets.len()));
            remove.extend(assets[1..].iter().copied());
        }
        if pendings.len() > 1 {
            notes.push(format!("{nn}: {} pending lines — keeping one", pendings.len()));
            remove.extend(pendings[1..].iter().copied());
        }
        let existing = pendings.first().copied();
        // WHICH NOTE. A newest lap the video does not show is either one the
        // render loop WILL render ("video pending": the map's first lap, or a
        // gain of at least the re-render threshold over the published lap) or a
        // sliver under the threshold ("within 0.1 s of the published clip") —
        // the parent project read a skipped sliver's "video pending" as a stall
        // (coordinator, 2026-09-10 14:15Z). The threshold is the loop's
        // `--min-gain-s` default; the gate is the loop's own (`render_gate`).
        let want = match (&newest, &row.published) {
            (Some(n), Some(p)) if n != p && !holds.contains_key(nn.as_str()) && staged.contains(&(nn.clone(), n.clone())) => Some((n.clone(), staged_note(ghosts_readme, nn, n))),
            (Some(n), None) if !holds.contains_key(nn.as_str()) && staged.contains(&(nn.clone(), n.clone())) => Some((n.clone(), staged_note(ghosts_readme, nn, n))),
            // a held map whose newest certified lap is NOT faster than the published
            // one (INPUT reverted the alias to the public lap; 96.297 vs 96.298 is a
            // rounding twin) carries the hold's reason as a plain records line —
            // never "latest lap 96.297 — held", which reads as a stall
            (Some(n), Some(p)) if n != p && holds.contains_key(nn.as_str()) && secs(n) >= secs(p) - 0.0015 => Some((String::new(), Note::Held(holds[nn.as_str()].clone()))),
            (Some(n), Some(p)) if n != p && holds.contains_key(nn.as_str()) => Some((n.clone(), held_note(&holds[nn.as_str()], staged.contains(&(nn.clone(), n.clone()))))),
            (Some(n), Some(p)) if n == p && holds.contains_key(nn.as_str()) && holds[nn.as_str()].starts_with("records:") => Some((String::new(), Note::Held(holds[nn.as_str()].clone()))),
            (Some(n), None) if holds.contains_key(nn.as_str()) => Some((n.clone(), held_note(&holds[nn.as_str()], staged.contains(&(nn.clone(), n.clone()))))),
            // a held map with a records-form reason and NO newest lap for its build
            // (15: its ship17c row left the README; the record still stands)
            (None, Some(_)) if holds.contains_key(nn.as_str()) && holds[nn.as_str()].starts_with("records:") => Some((String::new(), Note::Held(holds[nn.as_str()].clone()))),
            (Some(n), Some(p)) if n != p => {
                let will_render = matches!(
                    crate::video::render_gate(secs(n), Some((secs(p), &format!("x-{build}"))), Some(build), min_gain),
                    crate::video::Gate::Render(_)
                );
                Some((n.clone(), if will_render { Note::Pending } else { Note::Within(secs(p) - secs(n)) }))
            }
            (Some(n), None) => Some((n.clone(), Note::Pending)),
            _ => None,
        };
        match (want, existing) {
            (Some((t, note)), Some(j)) => {
                let line = status_line(&t, build, ghosts_readme, nn, &note, min_gain);
                if lines[j] != line {
                    notes.push(format!("{nn}: {} line → {t}", note.word()));
                    lines[j] = line;
                }
            }
            (Some((t, note)), None) => {
                let line = status_line(&t, build, ghosts_readme, nn, &note, min_gain);
                notes.push(format!("{nn}: {} line added ({t})", note.word()));
                lines.insert(i + 1, line);
                lines.insert(i + 1, String::new());
                remove.iter_mut().for_each(|r| *r += 2);
            }
            (None, Some(j)) => {
                notes.push(format!("{nn}: pending line removed (the video is current)"));
                remove.push(j);
            }
            (None, None) => {}
        }
        remove.sort_unstable();
        remove.dedup();
        for j in remove.into_iter().rev() {
            lines.remove(j);
            // and the blank line that was holding it, if that leaves two
            if j > 0 && j < lines.len() && lines[j - 1].trim().is_empty() && lines[j].trim().is_empty() {
                lines.remove(j);
            }
        }
        // THE LID LINE (parent project via the coordinator, 2026-09-10 20:45Z):
        // a map in lidrows.tsv carries its note as its own line right under the
        // row (before the status note and the video), verbatim from the file;
        // one line at most, rewritten when the note changes, removed when the
        // map leaves the list. The video stays up.
        // A LID NOTE IS TIED TO THE ship15 VIDEO (coordinator, 2026-09-11 03:10Z): it
        // leaves the row when the row's video becomes a clip of a drag-carrying
        // build (ship16 or later — ships.tsv names the clip), whatever lidrows.tsv
        // still lists. The file entry can stay; it simply no longer applies.
        let lid_applies = match ships_names.get(nn.as_str()) {
            Some(clip) => clip.rsplit_once("-ship").map(|(_, b)| !build_at_least_16(b)).unwrap_or(true),
            None => true,
        };
        maintain_line(&mut lines, &mut notes, i, nn, "lid", is_lid_note, lidrows.get(nn.as_str()).filter(|_| lid_applies).map(|n| lid_line(n)));
        // THE ROW'S BUILD (parent's decision for the burst, 2026-09-10 22:45Z):
        // rowbuilds.tsv names each row's build, the downloadable map file of
        // that build and a note. The caption's "(build X" is rewritten to the
        // row's build, a " · map: [shipNN](link)" segment is kept at the end of
        // the row line, and the note is its own line (`*↻ …*`) under the row.
        // A row build applies only once the row's VIDEO is a clip of that build (or
        // the entry has a `video=` marker saying the existing clip counts — the 15
        // laps whose frames are identical on ship16): a ship17c row over a ship15
        // video would caption the wrong thing (2026-09-11 01:21Z, 05). Until then
        // the entry waits, said once.
        let row_ready = rowbuilds.get(nn.as_str()).map(|rb| row_build_applies(rb, &lines[i], row.published.as_deref(), ships_names)).unwrap_or(false);
        if let Some(rb) = rowbuilds.get(nn.as_str()).filter(|_| row_ready) {
            let before = lines[i].clone();
            lines[i] = apply_row_build(&lines[i], rb);
            if lines[i] != before {
                notes.push(format!("{nn}: row build → {}{}", rb.build, if rb.link.is_empty() { "" } else { " (+ map link)" }));
            }
        } else if let Some(rb) = rowbuilds.get(nn.as_str()) {
            notes.push(format!("{nn}: row build {} waits — the row's video is not a {} clip yet", rb.build, rb.build));
        }
        let build_note = rowbuilds.get(nn.as_str()).filter(|_| row_ready).filter(|rb| !rb.note.trim().is_empty()).map(|rb| format!("{BUILD_NOTE_PREFIX}{}*", note_text(&rb.note).trim_end_matches('*')));
        maintain_line(&mut lines, &mut notes, i, nn, "build note", is_build_note, build_note);
    }
    let mut s = lines.join("\n");
    if page.ends_with('\n') && !s.ends_with('\n') {
        s.push('\n');
    }
    (s, notes)
}

/// Which note a row gets under it.
enum Note {
    /// The loop will render this lap.
    Pending,
    /// A sliver under the re-render threshold: the gain in seconds.
    Within(f64),
    /// The map is under a publish hold (holds.tsv): the reason.
    Held(String),
    /// Rendered and banked; the upload waits for the opening-check receipt.
    Staged,
    /// Receipt on file, every gate passed; the upload waits for a GitHub session
    /// on the box (ships.tsv `pending`, no URL yet).
    AwaitingSession,
}

impl Note {
    fn word(&self) -> &'static str {
        match self {
            Note::Pending => "pending",
            Note::Within(_) => "within",
            Note::Held(_) => "held",
            Note::Staged => "staged",
            Note::AwaitingSession => "awaiting-session",
        }
    }
}

fn secs(s: &str) -> f64 {
    s.trim().parse::<f64>().unwrap_or(f64::NAN)
}

fn status_line(time: &str, build: &str, ghosts_readme: &str, nn: &str, note: &Note, min_gain: f64) -> String {
    // the lap's OWN build, from its README row (`| nn | file | time | … | shipNN… |`),
    // falls back to the page default (15's ship17c lap under a ship15 page)
    // the page label the coordinator set (rowbuilds.tsv) outranks the README's tag
    // (05: certified on ship17-d9549f05, published as ship17c — the same bytes)
    let build = ROW_LABELS.with(|r| r.borrow().get(nn).cloned()).or_else(|| lap_build(ghosts_readme, nn, time)).unwrap_or_else(|| build.to_string());
    let build = build.as_str();
    let label = lap_label(ghosts_readme, nn, time);
    let who = if label == "tiny ghost" { String::new() } else { format!(", {label}") };
    match note {
        Note::Pending => format!("*latest lap **{time}** (build {build}{who}) — video pending*"),
        Note::Within(_) => format!("*latest lap **{time}** (build {build}{who}) — within {min_gain:.1} s of the published clip*"),
        // a records-form hold reads as the map's record line, without "latest lap"
        Note::Held(reason) if reason.starts_with("records:") && time.is_empty() => format!("*{} — held (opening rework)*", reason.trim_end_matches('*')),
        Note::Held(reason) => format!("*latest lap **{time}** (build {build}{who}) — held ({reason})*"),
        Note::Staged => format!("*latest lap **{time}** (build {build}{who}) — staged, awaiting the opening check*"),
        Note::AwaitingSession => format!("*latest lap **{time}** (build {build}{who}) — staged, awaiting the upload session*"),
    }
}

/// The tail of a "within" note, whatever the threshold printed in it.
pub const WITHIN_MARK: &str = "s of the published clip*";

pub fn cmd(args: &[String]) -> Result<(), String> {
    let f = |k: &str| tmmaps::cli::flag(args, k).map(String::from);
    let readme = PathBuf::from(f("--readme").ok_or("page-status needs --readme tiny/README.md")?);
    let ghosts_dir = PathBuf::from(f("--ghosts-dir").ok_or("page-status needs --ghosts-dir DIR")?);
    let build = f("--build").unwrap_or_else(|| "ship15".into());
    let min_gain: f64 = f("--min-gain-s").and_then(|s| s.parse().ok()).unwrap_or(0.1);
    let page = std::fs::read_to_string(&readme).map_err(|e| format!("{}: {e}", readme.display()))?;
    let gr = std::fs::read_to_string(ghosts_dir.join("README.md")).map_err(|e| format!("{}/README.md: {e}", ghosts_dir.display()))?;
    // per-map render builds (builds.tsv): a map whose build differs from --build
    // takes its newest lap from rows naming ITS build (15 on ship17c while the
    // page default is ship15 — otherwise its held/pending line never appears)
    let per_map = f("--out").map(|o| crate::video::read_builds(Path::new(&o))).unwrap_or_default();
    let mut laps = newest_laps(&gr, &build);
    for (nn, (b, _)) in &per_map {
        if *b != build {
            laps.retain(|(m, _)| m != nn);
            // the exact tag first (ship17c), then its family (ship17: 05's row says
            // ship17-d9549f05 while it renders on ship17c — the same map bytes,
            // which the render loop verified by md5)
            let family = build_family(b);
            let found = newest_laps(&gr, b).into_iter().find(|(m, _)| m == nn).or_else(|| newest_laps(&gr, &family).into_iter().find(|(m, _)| m == nn));
            if let Some((_, t)) = found {
                laps.push((nn.clone(), t));
            }
        }
    }
    laps.sort();
    println!("{} certified {build} laps: {}", laps.len(), laps.iter().map(|(m, t)| format!("{m} {t}")).collect::<Vec<_>>().join(", "));
    let holds = f("--out").map(|o| crate::video::read_holds(Path::new(&o))).unwrap_or_default();
    let ships_text = f("--out").map(|o| std::fs::read_to_string(Path::new(&o).join("ships.tsv")).unwrap_or_default()).unwrap_or_default();
    set_pending_rows(&ships_text);
    let staged = staged_laps(&ships_text);
    let lidrows = f("--out").map(|o| crate::video::parse_holds(&std::fs::read_to_string(Path::new(&o).join("lidrows.tsv")).unwrap_or_default())).unwrap_or_default();
    let rowbuilds = f("--out").map(|o| parse_rowbuilds(&std::fs::read_to_string(Path::new(&o).join("rowbuilds.tsv")).unwrap_or_default())).unwrap_or_default();
    set_row_labels(&rowbuilds);
    // the clip each row's video IS: the last URL row per map — never a pending one
    // (a queued clip is not on the page yet; 2026-09-11 01:34Z relabelled 05 twice on that)
    let clips: std::collections::HashMap<String, String> = f("--out").map(|o| published_clip_names(&std::fs::read_to_string(Path::new(&o).join("ships.tsv")).unwrap_or_default())).unwrap_or_default();
    let (new, notes) = update_rows_with_clips(&page, &laps, &gr, &build, min_gain, &holds, &staged, &lidrows, &rowbuilds, &clips);
    // informational notes ("row build X waits …") change nothing: say them, but
    // never write/commit on their account (an empty commit failed every tick)
    for n in notes.iter().filter(|n| n.contains(" waits ")) {
        println!("  {n}");
    }
    let notes: Vec<String> = notes.into_iter().filter(|n| !n.contains(" waits ")).collect();
    if notes.is_empty() || new == page {
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

#[cfg(test)]
mod repair_tests {
    use super::*;

    const GHOSTS: &str = "| map | file | time | credits | build | md5 | found by | validated |\n\
| 18 | 18.Ghost.Gbx | 44.593 | 9 | ship15 | cc138b11 | PPO | x |\n\
| 15 | 15.Ghost.Gbx | 49.097 | 8 | ship15 | 395f89a8 | PPO | x |\n";

    /// The shape the 2026-09-10 swaps left: the new URL right under the row, a
    /// stale pending line below it, and the previous video's URL below that.
    /// The pending line is found wherever it is in the block and removed (the
    /// video is current), and the extra asset line goes; the new URL stays.
    #[test]
    fn a_stale_pending_line_and_an_old_video_below_the_new_url_are_repaired() {
        let page = "**Tiny Summer 2026 - 18** — original author time `51.352` · tiny ghost **44.593** (build ship15, controls overlay)\n\n\
https://github.com/user-attachments/assets/new18\n\n\
*latest lap **44.593** (build ship15) — video pending*\n\n\
https://github.com/user-attachments/assets/old18\n\n\
**Tiny Summer 2026 - 15** — original author time `36.888` · tiny ghost **49.097** (build ship15, controls overlay)\n\n\
https://github.com/user-attachments/assets/new15\n\n\
*latest lap **49.825** (build ship15) — video pending*\n\n\
https://github.com/user-attachments/assets/old15\n\n";
        let laps = newest_laps(GHOSTS, "ship15");
        let (out, notes) = update(page, &laps, GHOSTS, "ship15");
        assert_eq!(
            out,
            "**Tiny Summer 2026 - 18** — original author time `51.352` · tiny ghost **44.593** (build ship15, controls overlay)\n\n\
https://github.com/user-attachments/assets/new18\n\n\
**Tiny Summer 2026 - 15** — original author time `36.888` · tiny ghost **49.097** (build ship15, controls overlay)\n\n\
https://github.com/user-attachments/assets/new15\n",
            "{notes:?}"
        );
        assert!(notes.iter().any(|n| n.contains("18: 2 asset lines")), "{notes:?}");
        assert!(notes.iter().any(|n| n.contains("15: pending line removed")), "{notes:?}");
        let (again, notes2) = update(&out, &laps, GHOSTS, "ship15");
        assert_eq!(again, out);
        assert!(notes2.is_empty(), "{notes2:?}");
    }

    /// And the swap itself no longer creates that shape: a row with a pending
    /// line under it and an old video below gets ONE url, the new one, and no
    /// pending line.
    #[test]
    fn a_swap_replaces_the_old_video_and_drops_the_pending_line() {
        let page = "**Tiny Summer 2026 - 18** — original author time `51.352` · tiny ghost **46.335** (build ship15, controls overlay)\n\n\
*latest lap **44.593** (build ship15) — video pending*\n\n\
https://github.com/user-attachments/assets/old18\n\n\
**Tiny Summer 2026 - 19** — original author time `43.841` · *no video yet*\n\n\
*latest lap **46.445** (build ship15) — video pending*\n\n\
**Tiny Summer 2026 - 20** — original author time `50.598` · *no lap yet*\n";
        let out = crate::video::page_swap(page, "18", "44.593", "tiny ghost", "build ship15, controls overlay", "https://github.com/user-attachments/assets/new18").unwrap();
        let out = crate::video::page_swap(&out, "19", "46.445", "tiny ghost", "build ship15, controls overlay", "https://github.com/user-attachments/assets/new19").unwrap();
        assert_eq!(
            out,
            "**Tiny Summer 2026 - 18** — original author time `51.352` · tiny ghost **44.593** (build ship15, controls overlay)\n\n\
https://github.com/user-attachments/assets/new18\n\n\
**Tiny Summer 2026 - 19** — original author time `43.841` · tiny ghost **46.445** (build ship15, controls overlay)\n\n\
https://github.com/user-attachments/assets/new19\n\n\
**Tiny Summer 2026 - 20** — original author time `50.598` · *no lap yet*\n"
        );
    }
}

#[cfg(test)]
mod within_tests {
    use super::*;

    const GHOSTS: &str = "| map | file | time | credits | build | md5 | found by | validated |\n\
| 15 | 15.Ghost.Gbx | 48.738 | 8 | ship15 | 395f89a8 | PPO | x |\n\
| 25 | 25.Ghost.Gbx | 102.115 | 15 | ship15 | bd1a146f | PPO | x |\n\
| 22 | 22.Ghost.Gbx | 96.297 | 14 | ship15 | f1275f23 | PPO | x |\n";

    /// A sliver under the re-render threshold says "within 0.1 s of the
    /// published clip" (the loop will not render it); a real gain says "video
    /// pending"; the note flips when the gain crosses the line; a swap drops
    /// either note.
    #[test]
    fn a_sliver_is_within_and_a_real_gain_is_pending() {
        let page = "**Tiny Summer 2026 - 15** — original author time `36.888` · tiny ghost **48.748** (build ship15, controls overlay)\n\n\
https://github.com/user-attachments/assets/a\n\n\
**Tiny Japan 2026** — original author time `78.928` · tiny ghost **102.424** (build ship15, controls overlay)\n\n\
https://github.com/user-attachments/assets/b\n\n\
**Tiny Saudi Arabia 2026** — original author time `73.418` · tiny ghost **96.298** (build ship15, controls overlay)\n\n\
*latest lap **96.297** (build ship15) — video pending*\n\n\
https://github.com/user-attachments/assets/c\n";
        let laps = newest_laps(GHOSTS, "ship15");
        let (out, notes) = update(page, &laps, GHOSTS, "ship15");
        assert!(out.contains("tiny ghost **48.748** (build ship15, controls overlay)\n\n*latest lap **48.738** (build ship15) — within 0.1 s of the published clip*\n\nhttps://github.com/user-attachments/assets/a"), "{out}");
        assert!(out.contains("tiny ghost **102.424** (build ship15, controls overlay)\n\n*latest lap **102.115** (build ship15) — video pending*\n\nhttps://github.com/user-attachments/assets/b"), "{out}");
        // an old-style pending note on a sliver is rewritten as a within note
        assert!(out.contains("tiny ghost **96.298** (build ship15, controls overlay)\n\n*latest lap **96.297** (build ship15) — within 0.1 s of the published clip*\n\nhttps://github.com/user-attachments/assets/c"), "{out}");
        assert!(notes.iter().any(|n| n == "15: within line added (48.738)"), "{notes:?}");
        assert!(notes.iter().any(|n| n == "25: pending line added (102.115)"), "{notes:?}");
        assert!(notes.iter().any(|n| n == "22: within line → 96.297"), "{notes:?}");
        // idempotent
        let (again, notes2) = update(&out, &laps, GHOSTS, "ship15");
        assert_eq!(again, out);
        assert!(notes2.is_empty(), "{notes2:?}");
        // the swap drops a within note like a pending one
        let swapped = crate::video::page_swap(&out, "15", "48.738", "tiny ghost", "build ship15, controls overlay", "https://github.com/user-attachments/assets/n").unwrap();
        assert!(!swapped.contains("*latest lap **48.738**"), "15's note must go: {swapped}");
        assert!(swapped.contains("96.297** (build ship15) — within 0.1 s"), "22's note stays: {swapped}");
        assert!(swapped.contains("tiny ghost **48.738** (build ship15, controls overlay)\n\nhttps://github.com/user-attachments/assets/n\n\n**Tiny Japan"), "{swapped}");
        // the opt-out: every newer lap is pending
        let (all, _) = update_with_gain(page, &laps, GHOSTS, "ship15", 0.0);
        assert!(all.contains("*latest lap **48.738** (build ship15) — video pending*"), "{all}");
    }
}

/// Is this page line one of the status notes page-status maintains under a row
/// (`— video pending*`, `— within 0.1 s of the published clip*`, `— held (…)*`)?
pub fn is_status_note(l: &str) -> bool {
    let l = l.trim_end();
    (l.starts_with("*latest lap **") && (l.ends_with(PENDING_MARK) || l.ends_with(WITHIN_MARK) || l.contains(") — held (") || l.ends_with(STAGED_MARK) || l.ends_with(SESSION_MARK)))
        || (l.starts_with("*records:") && l.ends_with("— held (opening rework)*"))
}

#[cfg(test)]
mod hold_tests {
    use super::*;

    /// A held map's newer lap reads "held (reason)" whatever its gain, the note
    /// is maintained like the others (updated, removed on swap), and a lifted
    /// hold turns it back into pending/within on the next run.
    #[test]
    fn a_held_map_says_so_and_a_lifted_hold_restores_the_gate() {
        let ghosts = "| 21 | 21.Ghost.Gbx | 112.000 | 17 | ship15 | 4feeaa5f | GEN | x |\n";
        let page = "**Tiny Argentina 2026** — original author time `78.988` · tiny ghost **115.478** (build ship15, controls overlay)\n\n\
https://github.com/user-attachments/assets/a\n";
        let laps = newest_laps(ghosts, "ship15");
        let mut holds = std::collections::HashMap::new();
        holds.insert("21".to_string(), "opening rework".to_string());
        let (held, notes) = update_full(page, &laps, ghosts, "ship15", 0.1, &holds);
        assert!(held.contains("tiny ghost **115.478** (build ship15, controls overlay)\n\n*latest lap **112.000** (build ship15) — held (opening rework)*\n\nhttps://github.com/user-attachments/assets/a"), "{held}");
        assert!(notes.iter().any(|n| n == "21: held line added (112.000)"), "{notes:?}");
        assert!(is_status_note("*latest lap **112.000** (build ship15) — held (opening rework)*"));
        // idempotent while held
        let (again, n2) = update_full(&held, &laps, ghosts, "ship15", 0.1, &holds);
        assert_eq!(again, held);
        assert!(n2.is_empty(), "{n2:?}");
        // lifted: the same line becomes a pending note (3.5 s gain)
        let (lifted, n3) = update_full(&held, &laps, ghosts, "ship15", 0.1, &std::collections::HashMap::new());
        assert!(lifted.contains("*latest lap **112.000** (build ship15) — video pending*"), "{lifted}");
        assert!(n3.iter().any(|n| n == "21: pending line → 112.000"), "{n3:?}");
        // a swap drops a held note too
        let swapped = crate::video::page_swap(&held, "21", "112.000", "tiny ghost", "build ship15, controls overlay", "https://github.com/user-attachments/assets/n").unwrap();
        assert!(!swapped.contains("held ("), "{swapped}");
        // the holds file parses
        let h = crate::video::parse_holds("# nn\treason\n21\topening rework (vjeux)\n\n07\n");
        assert_eq!(h.get("21").map(String::as_str), Some("opening rework (vjeux)"));
        assert_eq!(h.get("07").map(String::as_str), Some("held"));
        assert_eq!(h.len(), 2);
    }
}

pub const STAGED_MARK: &str = "— staged, awaiting the opening check*";
pub const SESSION_MARK: &str = "— staged, awaiting the upload session*";

/// The (map, lap) pairs whose ships.tsv row is `staged`.
pub fn staged_laps(ships: &str) -> std::collections::HashSet<(String, String)> {
    ships
        .lines()
        .filter(|l| !l.starts_with('#'))
        .filter_map(|l| {
            let c: Vec<&str> = l.split('\t').map(str::trim).collect();
            (c.len() >= 5 && (c[4] == "staged" || c[4] == "held" || c[4] == "pending")).then(|| (c[0].to_string(), c[1].to_string()))
        })
        .collect()
}

#[cfg(test)]
mod staged_tests {
    use super::*;

    /// A rendered clip waiting for its receipt reads "staged, awaiting the
    /// opening check"; a receipt (the row turning pending, then published)
    /// removes it; a hold outranks it.
    #[test]
    fn a_staged_clip_says_it_awaits_the_opening_check() {
        let ghosts = "| 22 | 22.Ghost.Gbx | 82.652 | 14 | ship15 | f1275f23 | GEN | x |\n- 22 82.652: below 8 m/s: 1.00 s, respawns: 0, inverted: 0.00 s, 1 slow + 0 attitude intervals\n";
        let page = "**Tiny Saudi Arabia 2026** — original author time `73.418` · tiny ghost **96.298** (build ship15, controls overlay)\n\n\
https://github.com/user-attachments/assets/a\n";
        let laps = newest_laps(ghosts, "ship15");
        let staged = staged_laps("# nn\ttime\tname\tdone\tstatus\n22\t82.652\t22-ghost-82.652-ship15\t/x\tstaged\n");
        let no_holds = std::collections::HashMap::new();
        let (out, notes) = update_all(page, &laps, ghosts, "ship15", 0.1, &no_holds, &staged);
        assert!(out.contains("*latest lap **82.652** (build ship15) — staged, awaiting the opening check*"), "{out}");
        assert!(notes.iter().any(|n| n == "22: staged line added (82.652)"), "{notes:?}");
        assert!(is_status_note("*latest lap **82.652** (build ship15) — staged, awaiting the opening check*"));
        // receipt given → the row is pending/published → the note becomes "video pending" until the swap
        let (after, _) = update_all(&out, &laps, ghosts, "ship15", 0.1, &no_holds, &std::collections::HashSet::new());
        assert!(after.contains("*latest lap **82.652** (build ship15) — video pending*"), "{after}");
        // a hold outranks staged
        let mut holds = std::collections::HashMap::new();
        holds.insert("22".to_string(), "opening".to_string());
        let (held, _) = update_all(page, &laps, ghosts, "ship15", 0.1, &holds, &staged);
        assert!(held.contains("— held (staged — opening)*"), "a render-mode hold with a staged clip: {held}");
        // receipts: exact lap or a standing `*`
        let a = "# nn\ttime\tby\tnote\n22\t82.652\tcoordinator\topening ok\n15\t*\tparent\tstanding\n";
        assert!(crate::video::find_approval(a, "22", "82.652").is_some());
        assert!(crate::video::find_approval(a, "22", "82.000").is_none());
        assert!(crate::video::find_approval(a, "15", "48.738").is_some());
        assert_eq!(crate::video::parse_prechecked("# maps\n07\n\n12\tfoo\n"), ["07", "12"].into_iter().map(String::from).collect());
    }
}

/// A held map's note: "staged — held (reason)" when its clip is rendered and
/// waiting (a `render`-mode hold, the ship row `held`/`staged`), plain
/// "held (reason)" when nothing was rendered.
fn held_note(reason: &str, is_staged: bool) -> Note {
    if is_staged { Note::Held(format!("staged — {reason}")) } else { Note::Held(reason.to_string()) }
}

/// A staged clip's note: "staged, awaiting the opening check" when its lap's
/// attitude is clean (a receipt can release it), "staged — held (attitude:
/// …)" when it is not (no receipt can — INPUT's README says the lap rolls or
/// inverts; the next lap of the map must be clean).
fn staged_note(ghosts_readme: &str, nn: &str, time: &str) -> Note {
    // a row that shipwatch already accepted (receipt + gates) waits only for a
    // GitHub session on the box: ships.tsv says `pending` for it
    if PENDING_ROWS.with(|p| p.borrow().contains(&(nn.to_string(), time.to_string()))) {
        return Note::AwaitingSession;
    }
    match crate::video::attitude_verdict(ghosts_readme, nn, time) {
        crate::video::Attitude::Clean => Note::Staged,
        v => Note::Held(format!("staged — attitude: {}", v.describe())),
    }
}

thread_local! {
    /// (map, time) rows whose ships.tsv status is `pending` — set by `cmd` from
    /// --out before the page pass (the row set is otherwise "staged or held").
    static PENDING_ROWS: std::cell::RefCell<std::collections::HashSet<(String, String)>> = std::cell::RefCell::new(std::collections::HashSet::new());
}

thread_local! {
    /// map → page build label from rowbuilds.tsv, set by `cmd` before the pass.
    static ROW_LABELS: std::cell::RefCell<std::collections::HashMap<String, String>> = std::cell::RefCell::new(std::collections::HashMap::new());
}

pub fn set_row_labels(rowbuilds: &std::collections::HashMap<String, RowBuild>) {
    ROW_LABELS.with(|r| *r.borrow_mut() = rowbuilds.iter().map(|(k, v)| (k.clone(), v.build.clone())).collect());
}

pub fn set_pending_rows(ships: &str) {
    let set: std::collections::HashSet<(String, String)> = ships
        .lines()
        .filter(|l| !l.starts_with('#'))
        .filter_map(|l| {
            let c: Vec<&str> = l.split('\t').map(str::trim).collect();
            (c.len() >= 5 && c[4] == "pending").then(|| (c[0].to_string(), c[1].to_string()))
        })
        .collect();
    PENDING_ROWS.with(|p| *p.borrow_mut() = set);
}

#[cfg(test)]
mod attitude_note_tests {
    use super::*;

    /// A staged clip whose lap the README calls dirty (or does not list) reads
    /// "held (staged — attitude: …)"; a clean one "staged, awaiting the opening check".
    #[test]
    fn a_dirty_staged_clip_is_held_on_attitude() {
        let ghosts = "| 22 | 22.Ghost.Gbx | 82.652 | 14 | ship15 | f1275f23 | GEN | x |\n\
| 19 | 19.Ghost.Gbx | 46.000 | 16 | ship15 | 5522d061 | PPO | x |\n\
- 22 82.652: below 8 m/s: 8.45 s, respawns: 0, inverted: 2.69 s, 11 slow + 2 attitude intervals\n\
- 19 46.000: below 8 m/s: 0.07 s, respawns: 0, inverted: 0.00 s, 2 slow + 0 attitude intervals\n";
        let page = "**Tiny Summer 2026 - 19** — original author time `43.841` · tiny ghost **46.362** (build ship15, controls overlay)\n\n\
https://github.com/user-attachments/assets/a\n\n\
**Tiny Saudi Arabia 2026** — original author time `73.418` · tiny ghost **96.298** (build ship15, controls overlay)\n\n\
https://github.com/user-attachments/assets/b\n";
        let laps = newest_laps(ghosts, "ship15");
        let staged = staged_laps("22\t82.652\tx\t/x\tstaged\n19\t46.000\tx\t/x\tstaged\n");
        let (out, _) = update_all(page, &laps, ghosts, "ship15", 0.1, &std::collections::HashMap::new(), &staged);
        assert!(out.contains("*latest lap **46.000** (build ship15) — staged, awaiting the opening check*"), "{out}");
        assert!(out.contains("*latest lap **82.652** (build ship15) — held (staged — attitude: not clean: inverted 2.69 s, 2 attitude interval(s) (> 0.3 s of |roll|/|pitch| > 60°))*"), "{out}");
    }
}

/// The lid note's line: `*⚠ <note>*` — its own line, not a status note.
pub const LID_PREFIX: &str = "*⚠ ";

fn lid_line(note: &str) -> String {
    format!("{LID_PREFIX}{}*", note.trim().trim_end_matches('*'))
}

pub fn is_lid_note(l: &str) -> bool {
    l.trim_end().starts_with(LID_PREFIX)
}

#[cfg(test)]
mod lid_tests {
    use super::*;

    /// A lid row gets its note as its own line under the row, beside the status
    /// note; idempotent; updated when the note changes; removed when the map
    /// leaves the list; a swap keeps it.
    #[test]
    fn lid_rows_carry_their_note_under_the_row() {
        let ghosts = "| 15 | 15.Ghost.Gbx | 48.738 | 8 | ship15 | 395f89a8 | PPO | x |\n\
| 05 | 05.Ghost.Gbx | 18.298 | 4 | ship15 | e0cb1188 | PPO | x |\n";
        let page = "**Tiny Summer 2026 - 05** — original author time `27.795` · tiny ghost **18.298** (build ship15, controls overlay)\n\n\
https://github.com/user-attachments/assets/a\n\n\
**Tiny Summer 2026 - 15** — original author time `36.888` · tiny ghost **48.748** (build ship15, controls overlay)\n\n\
*latest lap **48.738** (build ship15) — within 0.1 s of the published clip*\n\n\
https://github.com/user-attachments/assets/b\n";
        let laps = newest_laps(ghosts, "ship15");
        let mut lid = std::collections::HashMap::new();
        let note = "lap rides the ship15 water lid; the original's water would stop the car; being re-searched with zero water contact";
        lid.insert("05".to_string(), note.to_string());
        lid.insert("15".to_string(), note.to_string());
        let empty_h = std::collections::HashMap::new();
        let empty_s = std::collections::HashSet::new();
        let (out, notes) = update_page(page, &laps, ghosts, "ship15", 0.1, &empty_h, &empty_s, &lid);
        assert!(out.contains(&format!("tiny ghost **18.298** (build ship15, controls overlay)\n\n*⚠ {note}*\n\nhttps://github.com/user-attachments/assets/a")), "{out}");
        assert!(out.contains(&format!("tiny ghost **48.748** (build ship15, controls overlay)\n\n*⚠ {note}*\n\n*latest lap **48.738** (build ship15) — within 0.1 s of the published clip*\n\nhttps://github.com/user-attachments/assets/b")), "{out}");
        assert!(notes.iter().any(|n| n == "05: lid line added") && notes.iter().any(|n| n == "15: lid line added"), "{notes:?}");
        // idempotent
        let (again, n2) = update_page(&out, &laps, ghosts, "ship15", 0.1, &empty_h, &empty_s, &lid);
        assert_eq!(again, out);
        assert!(n2.is_empty(), "{n2:?}");
        // the note changes → rewritten; the map leaves → removed
        lid.insert("05".to_string(), "resolved on ship16".to_string());
        lid.remove("15");
        let (changed, n3) = update_page(&out, &laps, ghosts, "ship15", 0.1, &empty_h, &empty_s, &lid);
        assert!(changed.contains("*⚠ resolved on ship16*"), "{changed}");
        assert!(!changed.contains(&format!("48.748** (build ship15, controls overlay)\n\n*⚠ {note}*")), "{changed}");
        assert!(n3.iter().any(|n| n == "05: lid line → updated") && n3.iter().any(|n| n == "15: lid line removed"), "{n3:?}");
        // a swap of 15 keeps the lid line and drops the status note
        let swapped = crate::video::page_swap(&out, "15", "48.738", "tiny ghost", "build ship15, controls overlay", "https://github.com/user-attachments/assets/n").unwrap();
        assert!(swapped.contains(&format!("tiny ghost **48.738** (build ship15, controls overlay)\n\n*⚠ {note}*\n\nhttps://github.com/user-attachments/assets/n")), "{swapped}");
        assert!(!swapped.contains("within 0.1 s"), "{swapped}");
    }
}

/// A row's build, from `rowbuilds.tsv` (`nn<TAB>build<TAB>map_link<TAB>note`).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct RowBuild {
    pub build: String,
    /// The downloadable map file of that build (a URL); empty = no link yet.
    pub link: String,
    /// A note under the row (`*↻ …*`); empty = none.
    pub note: String,
}

pub fn parse_rowbuilds(text: &str) -> std::collections::HashMap<String, RowBuild> {
    text.lines()
        .filter(|l| !l.trim().is_empty() && !l.starts_with('#'))
        .filter_map(|l| {
            let c: Vec<&str> = l.split('\t').map(str::trim).collect();
            let nn = c[0];
            if nn.len() != 2 || !nn.chars().all(|ch| ch.is_ascii_digit()) || c.len() < 2 || c[1].is_empty() {
                return None;
            }
            Some((nn.to_string(), RowBuild { build: c[1].to_string(), link: c.get(2).copied().unwrap_or("").to_string(), note: c.get(3).copied().unwrap_or("").to_string() }))
        })
        .collect()
}

/// The build-note line's prefix (a re-drive / identical-frames note under the row).
pub const BUILD_NOTE_PREFIX: &str = "*↻ ";

pub fn is_build_note(l: &str) -> bool {
    l.trim_end().starts_with(BUILD_NOTE_PREFIX)
}

/// The map-file segment kept at the end of a row line.
const MAP_SEG: &str = " · map: ";

/// The row line with the row's build applied: the caption's `(build X` becomes
/// `(build <rb.build>` and a trailing ` · map: [shipNN](link)` (or ` · map:
/// shipNN` without a link) is kept current.
pub fn apply_row_build(line: &str, rb: &RowBuild) -> String {
    let mut s = line.trim_end().to_string();
    // drop an existing map segment
    if let Some(k) = s.find(MAP_SEG) {
        s.truncate(k);
    }
    // the caption's build
    if let Some(k) = s.find("(build ") {
        let start = k + "(build ".len();
        let rest = &s[start..];
        let stop = rest.find(|c: char| c == ',' || c == ')').unwrap_or(rest.len());
        s = format!("{}{}{}", &s[..start], rb.build, &rest[stop..]);
    }
    let seg = if rb.link.is_empty() { format!("{MAP_SEG}{}", rb.build) } else { format!("{MAP_SEG}[{}]({})", rb.build, rb.link) };
    s.push_str(&seg);
    s
}

/// Keep ONE maintained line of a kind under row `i`: `wanted` = the line's
/// exact text (added right under the row, rewritten when it differs) or
/// `None` (every such line removed). `is_kind` recognises the kind's lines.
fn maintain_line(lines: &mut Vec<String>, notes: &mut Vec<String>, i: usize, nn: &str, kind: &str, is_kind: fn(&str) -> bool, wanted: Option<String>) {
    let end = crate::video::block_end(lines, i);
    let have: Vec<usize> = (i + 1..end).filter(|&j| is_kind(&lines[j])).collect();
    match (wanted, have.first().copied()) {
        (Some(line), Some(j)) => {
            if lines[j] != line {
                notes.push(format!("{nn}: {kind} line → updated"));
                lines[j] = line;
            }
            for &k in have[1..].iter().rev() {
                lines.remove(k);
            }
        }
        (Some(line), None) => {
            notes.push(format!("{nn}: {kind} line added"));
            lines.insert(i + 1, line);
            lines.insert(i + 1, String::new());
        }
        (None, Some(_)) => {
            notes.push(format!("{nn}: {kind} line removed"));
            for &k in have.iter().rev() {
                lines.remove(k);
                if k > 0 && k < lines.len() && lines[k - 1].trim().is_empty() && lines[k].trim().is_empty() {
                    lines.remove(k);
                }
            }
        }
        (None, None) => {}
    }
}

#[cfg(test)]
mod rowbuild_tests {
    use super::*;

    /// rowbuilds.tsv: the caption's build follows the row, a map-file link is
    /// kept at the end of the row line, the note is its own line; a row without
    /// an entry is untouched; a swap keeps the segment and the note.
    #[test]
    fn a_row_carries_its_build_its_map_file_and_its_note() {
        let ghosts = "| 15 | 15.Ghost.Gbx | 48.738 | 8 | ship15 | 395f89a8 | PPO | x |\n\
| 13 | 13.Ghost.Gbx | 24.769 | 6 | ship15 | c1d1e1f1 | PPO | x |\n\
| 05 | 05.Ghost.Gbx | 18.298 | 4 | ship15 | e0cb1188 | PPO | x |\n";
        let page = "**Tiny Summer 2026 - 05** — original author time `27.795` · tiny ghost **18.298** (build ship15, controls overlay)\n\n\
https://github.com/user-attachments/assets/a\n\n\
**Tiny Summer 2026 - 13** — original author time `31.7` · tiny ghost **24.769** (build ship15, controls overlay)\n\n\
https://github.com/user-attachments/assets/b\n\n\
**Tiny Summer 2026 - 15** — original author time `36.888` · tiny ghost **48.748** (build ship15, controls overlay)\n\n\
*latest lap **48.738** (build ship15) — within 0.1 s of the published clip*\n\n\
https://github.com/user-attachments/assets/c\n";
        let laps = newest_laps(ghosts, "ship15");
        let rb = parse_rowbuilds("# nn\tbuild\tmap_link\tnote\n\
05\tship16\thttps://example.test/ship16/05.zip\tvideo=ok video rendered on ship15 — frames identical on ship16\n\
13\tship15\t\truns on un-skinned ship15 surfaces at 12.4 s — re-drive pending\n");
        let none_h = std::collections::HashMap::new();
        let none_s = std::collections::HashSet::new();
        let none_l = std::collections::HashMap::new();
        let (out, notes) = update_rows(page, &laps, ghosts, "ship15", 0.1, &none_h, &none_s, &none_l, &rb);
        assert!(out.contains("**Tiny Summer 2026 - 05** — original author time `27.795` · tiny ghost **18.298** (build ship16, controls overlay) · map: [ship16](https://example.test/ship16/05.zip)\n\n*↻ video rendered on ship15 — frames identical on ship16*\n\nhttps://github.com/user-attachments/assets/a"), "{out}");
        assert!(out.contains("tiny ghost **24.769** (build ship15, controls overlay) · map: ship15\n\n*↻ runs on un-skinned ship15 surfaces at 12.4 s — re-drive pending*\n\nhttps://github.com/user-attachments/assets/b"), "{out}");
        assert!(out.contains("tiny ghost **48.748** (build ship15, controls overlay)\n\n*latest lap **48.738**"), "15 untouched: {out}");
        assert!(notes.iter().any(|n| n == "05: row build → ship16 (+ map link)"), "{notes:?}");
        // idempotent
        let (again, n2) = update_rows(&out, &laps, ghosts, "ship15", 0.1, &none_h, &none_s, &none_l, &rb);
        assert_eq!(again, out);
        assert!(n2.is_empty(), "{n2:?}");
        // the link arrives later → the segment is rewritten in place
        let rb2 = parse_rowbuilds("13\tship15\thttps://example.test/ship15/13.zip\truns on un-skinned ship15 surfaces at 12.4 s — re-drive pending\n05\tship16\thttps://example.test/ship16/05.zip\tvideo=ok video rendered on ship15 — frames identical on ship16\n");
        let (linked, _) = update_rows(&out, &laps, ghosts, "ship15", 0.1, &none_h, &none_s, &none_l, &rb2);
        assert!(linked.contains("(build ship15, controls overlay) · map: [ship15](https://example.test/ship15/13.zip)\n"), "{linked}");
        assert!(!linked.contains("· map: ship15\n"), "{linked}");
        // a swap keeps the segment? page_swap rewrites the row line from its own template —
        // page-status re-applies the segment on the next run (checked here)
        let swapped = crate::video::page_swap(&linked, "05", "18.298", "tiny ghost", "build ship16, controls overlay", "https://github.com/user-attachments/assets/n").unwrap();
        let (fixed, _) = update_rows(&swapped, &laps, ghosts, "ship15", 0.1, &none_h, &none_s, &none_l, &rb2);
        assert!(fixed.contains("tiny ghost **18.298** (build ship16, controls overlay) · map: [ship16](https://example.test/ship16/05.zip)\n\n*↻ video rendered on ship15 — frames identical on ship16*\n\nhttps://github.com/user-attachments/assets/n"), "{fixed}");
    }
}

/// `tinyctl final-table --out DIR --readme PAGE --ghosts-dir DIR [--write F]`:
/// the delivery table — one row per map: lap (the page's video), build,
/// asset URL, approval (receipt / prechecked / published before the gate /
/// held / staged / none), notes (lid, build note, attitude, hold reason).
/// Markdown to stdout, and to `--write` when given.
pub fn final_table_cmd(args: &[String]) -> Result<(), String> {
    let f = |k: &str| tmmaps::cli::flag(args, k);
    let out = PathBuf::from(f("--out").ok_or("--out DIR (ships.tsv, holds.tsv, approvals.tsv, rowbuilds.tsv, lidrows.tsv)")?);
    let readme = std::fs::read_to_string(f("--readme").ok_or("--readme tiny/README.md")?).map_err(|e| format!("readme: {e}"))?;
    let ghosts = f("--ghosts-dir").map(|d| std::fs::read_to_string(Path::new(&d).join("README.md")).unwrap_or_default()).unwrap_or_default();
    let ships = std::fs::read_to_string(out.join("ships.tsv")).unwrap_or_default();
    let holds = crate::video::read_holds(&out);
    let approvals = std::fs::read_to_string(out.join("approvals.tsv")).unwrap_or_default();
    let prechecked = crate::video::read_prechecked(&out);
    let rowbuilds = parse_rowbuilds(&std::fs::read_to_string(out.join("rowbuilds.tsv")).unwrap_or_default());
    let lidrows = crate::video::parse_holds(&std::fs::read_to_string(out.join("lidrows.tsv")).unwrap_or_default());
    set_pending_rows(&ships);
    let table = final_table(&readme, &ghosts, &ships, &holds, &approvals, &prechecked, &rowbuilds, &lidrows);
    println!("{table}");
    if let Some(p) = f("--write") {
        std::fs::write(&p, &table).map_err(|e| format!("{p}: {e}"))?;
        eprintln!("wrote {p}");
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub fn final_table(page: &str, ghosts_readme: &str, ships: &str, holds: &std::collections::HashMap<String, String>, approvals: &str, prechecked: &std::collections::HashSet<String>, rowbuilds: &std::collections::HashMap<String, RowBuild>, lidrows: &std::collections::HashMap<String, String>) -> String {
    let published = crate::video::published_laps(ships);
    let staged = staged_laps(ships);
    let lines: Vec<&str> = page.lines().collect();
    let mut rows: Vec<String> = Vec::new();
    for (i, row) in rows_of(page) {
        let nn = row.nn.clone();
        let title = map_title(&nn);
        let lap = row.published.clone().unwrap_or_else(|| "—".into());
        // the build of the VIDEO the row shows: the caption's "(build X" (what the
        // page states); a rowbuilds label counts only once it applies to the row
        let build = lines[i]
            .find("(build ")
            .map(|k| lines[i][k + 7..].split(|c: char| c == ',' || c == ')').next().unwrap_or("").to_string())
            .filter(|b| !b.is_empty())
            .unwrap_or_else(|| "—".into());
        let end = crate::video::block_end(&lines.iter().map(|s| s.to_string()).collect::<Vec<_>>(), i);
        let url = (i + 1..end).map(|j| lines[j]).find(|l| l.starts_with(crate::video::ASSET_PREFIX)).unwrap_or("—").to_string();
        // approval: how this row's video got (or did not get) its place
        let approval = if row.published.is_none() {
            "no video".to_string()
        } else if let Some(a) = crate::video::find_approval(approvals, &nn, &lap) {
            let c: Vec<&str> = a.split('\t').map(str::trim).collect();
            format!("receipt ({}{})", c.get(2).copied().unwrap_or("?"), c.get(3).filter(|n| !n.is_empty()).map(|n| format!(": {n}")).unwrap_or_default())
        } else if prechecked.contains(nn.as_str()) {
            "prechecked".to_string()
        } else {
            "published before the receipt gate (2026-09-10 18:45Z)".to_string()
        };
        // notes: hold, staged newer lap, attitude of the published lap, lid, build note
        let mut notes: Vec<String> = Vec::new();
        if let Some(r) = holds.get(nn.as_str()) {
            notes.push(format!("HELD: {r}"));
        }
        for (m, t) in staged.iter().filter(|(m, _)| *m == nn) {
            let _ = m;
            // receipted + gates passed (ships.tsv `pending`) → waits for the session;
            // else awaiting the opening check
            let pending = PENDING_ROWS.with(|p| p.borrow().contains(&(m.clone(), t.clone())));
            let receipt = crate::video::find_approval(approvals, &nn, t).map(|a| a.split('\t').nth(2).unwrap_or("?").trim().to_string());
            match (pending, receipt) {
                (true, Some(by)) => notes.push(format!("staged {t}: receipt on file ({by}), gates passed — awaiting the upload session")),
                (true, None) => notes.push(format!("staged {t}: gates passed — awaiting the upload session")),
                (false, Some(by)) => notes.push(format!("staged {t}: receipt on file ({by}); held by a gate or a hold")),
                (false, None) => notes.push(format!("staged {t} awaiting the opening check")),
            }
        }
        if let Some((t, _)) = published.get(nn.as_str()) {
            let ts = format!("{:.3}", t);
            match crate::video::attitude_verdict(ghosts_readme, &nn, &ts) {
                crate::video::Attitude::Clean => notes.push("attitude clean".into()),
                crate::video::Attitude::NoTable => {}
                v => notes.push(format!("attitude: {}", v.describe())),
            }
        }
        if let Some(l) = lidrows.get(nn.as_str()) {
            notes.push(format!("⚠ {l}"));
        }
        if let Some(rb) = rowbuilds.get(nn.as_str()) {
            if !rb.note.trim().is_empty() {
                notes.push(format!("↻ {}", note_text(&rb.note)));
            }
        }
        let esc = |s: &str| s.replace('|', "\\|");
        rows.push(format!("| {nn} | {} | {lap} | {build} | {url} | {} | {} |", esc(&title), esc(&approval), esc(&notes.join("; "))));
    }
    let when = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
    format!(
        "# Tiny Summer 2026 — delivery table (generated by `tinyctl final-table`, unix {when})\n\n| map | title | lap (video) | build | asset URL | approval | notes |\n|---|---|---|---|---|---|---|\n{}\n",
        rows.join("\n")
    )
}

#[cfg(test)]
mod final_table_tests {
    use super::*;

    #[test]
    fn the_delivery_table_names_lap_build_url_approval_and_notes_per_row() {
        let page = "**Tiny Summer 2026 - 05** — original author time `27.795` · tiny ghost **18.298** (build ship16, controls overlay) · map: [ship16](https://x/05.zip)\n\n\
https://github.com/user-attachments/assets/a\n\n\
**Tiny Saudi Arabia 2026** — original author time `73.418` · tiny ghost **96.298** (build ship15, controls overlay)\n\n\
https://github.com/user-attachments/assets/b\n\n\
**Tiny Argentina 2026** — original author time `78.988` · *no video yet*\n";
        let ghosts = "- 05 18.298: below 8 m/s: 0.00 s, respawns: 0, inverted: 0.00 s, 0 slow + 0 attitude intervals\n\
- 22 96.298: below 8 m/s: 8.45 s, respawns: 0, inverted: 2.69 s, 11 slow + 2 attitude intervals\n";
        let ships = "05\t18.298\t05-ghost-18.298-ship15\t/x\thttps://github.com/user-attachments/assets/a\n22\t96.298\t22-ghost-96.298-ship15\t/x\thttps://github.com/user-attachments/assets/b\n22\t82.652\t22-ghost-82.652-ship15\t/x\theld\n";
        let mut holds = std::collections::HashMap::new();
        holds.insert("22".to_string(), "opening rework".to_string());
        let approvals = "05\t18.298\tcoordinator\topening ok\n";
        let rb = parse_rowbuilds("05\tship16\thttps://x/05.zip\tvideo rendered on ship15 — frames identical on ship16\n");
        let mut lid = std::collections::HashMap::new();
        lid.insert("05".to_string(), "lap rides the water lid".to_string());
        let t = final_table(page, ghosts, ships, &holds, approvals, &std::collections::HashSet::new(), &rb, &lid);
        assert!(t.contains("| 05 | Tiny Summer 2026 - 05 | 18.298 | ship16 | https://github.com/user-attachments/assets/a | receipt (coordinator: opening ok) | attitude clean; ⚠ lap rides the water lid; ↻ video rendered on ship15 — frames identical on ship16 |"), "{t}");
        assert!(t.contains(&"| 22 | Tiny Saudi Arabia 2026 | 96.298 | ship15 | https://github.com/user-attachments/assets/b | published before the receipt gate (2026-09-10 18:45Z) | HELD: opening rework; staged 82.652 awaiting the opening check; attitude: not clean: inverted 2.69 s, 2 attitude interval(s) (> 0.3 s of |roll|/|pitch| > 60°) |".replace("|roll|/|pitch|", "\\|roll\\|/\\|pitch\\|")), "{t}");
        assert!(t.contains("| 21 | Tiny Argentina 2026 | — | — | — | no video |  |"), "{t}");
    }
}

/// Does this rowbuilds entry apply to the row as it stands? Yes when the row's
/// published video is a clip of that build (its ships.tsv name ends in
/// `-<build>`), when the row's caption already says that build, or when the
/// entry's note starts with `video=ok` (the existing clip counts — the laps whose
/// frames are identical on the new build). Otherwise the entry waits.
pub fn row_build_applies(rb: &RowBuild, row_line: &str, published: Option<&str>, ships_names: &std::collections::HashMap<String, String>) -> bool {
    let _ = published;
    if rb.note.trim_start().starts_with("video=ok") {
        return true;
    }
    if row_line.contains(&format!("(build {}", rb.build)) {
        return true;
    }
    let title = row_line.trim_start_matches("**").split("**").next().unwrap_or("");
    let nn = (1..=25).map(|n| format!("{n:02}")).find(|n| map_title(n) == title || format!("Tiny Summer 2026 - {n}") == title);
    match nn.and_then(|n| ships_names.get(&n)) {
        // the row's video is a clip of that build — or the clip the entry NAMES
        // (`clip=<name>` at the start of the note: 05's ship17b render labelled
        // ship17c on the page, the same map bytes)
        Some(clip) => clip.ends_with(&format!("-{}", rb.build)) || note_clip(&rb.note).map(|c| c == *clip).unwrap_or(false),
        None => false,
    }
}

#[cfg(test)]
mod row_ready_tests {
    use super::*;

    #[test]
    fn a_row_build_waits_until_the_video_is_a_clip_of_that_build() {
        let rb = RowBuild { build: "ship17c".into(), link: String::new(), note: "road-through-water section without drag".into() };
        let row15 = "**Tiny Summer 2026 - 05** — original author time `27.795` · tiny ghost **18.298** (build ship15, controls overlay)";
        let mut ships = std::collections::HashMap::new();
        ships.insert("05".to_string(), "05-ghost-18.298-ship15".to_string());
        assert!(!row_build_applies(&rb, row15, Some("18.298"), &ships), "the video is a ship15 clip");
        ships.insert("05".to_string(), "05-ghost-16.395-ship17c".to_string());
        assert!(row_build_applies(&rb, row15, Some("16.395"), &ships), "now the video is a ship17c clip");
        let rb_ok = RowBuild { build: "ship16".into(), link: String::new(), note: "video=ok frames identical on ship16".into() };
        assert!(row_build_applies(&rb_ok, row15, Some("18.298"), &std::collections::HashMap::new()), "video=ok applies at once");
        let row17 = "**Tiny Summer 2026 - 05** — original author time `27.795` · tiny ghost **16.395** (build ship17c, controls overlay)";
        assert!(row_build_applies(&rb, row17, Some("16.395"), &std::collections::HashMap::new()), "already labelled");
    }
}

/// Is this clip build suffix (`15`, `16`, `17c`, …) ship16 or later — a build
/// that carries the water drag, where a lid note no longer applies?
fn build_at_least_16(suffix: &str) -> bool {
    let n: String = suffix.chars().take_while(|c| c.is_ascii_digit()).collect();
    n.parse::<u32>().map(|v| v >= 16).unwrap_or(false)
}

#[cfg(test)]
mod lid_leaves_tests {
    use super::*;

    /// The ⚠ lid line goes when the row's video becomes a ship16+ clip, even
    /// while lidrows.tsv still lists the map.
    #[test]
    fn the_lid_note_leaves_with_the_ship15_video() {
        let ghosts = "| 05 | 05.Ghost.Gbx | 16.395 | 5 | ship17c | 4303b199 | GEN | x |\n";
        let page = "**Tiny Summer 2026 - 05** — original author time `27.795` · tiny ghost **16.395** (build ship17c, controls overlay)\n\n\
*⚠ lap rides the ship15 water lid; this build has no water drag*\n\n\
https://github.com/user-attachments/assets/n\n";
        let laps = newest_laps(ghosts, "ship17c");
        let mut lid = std::collections::HashMap::new();
        lid.insert("05".to_string(), "lap rides the ship15 water lid; this build has no water drag".to_string());
        let mut clips = std::collections::HashMap::new();
        clips.insert("05".to_string(), "05-ghost-16.395-ship17c".to_string());
        let none_h = std::collections::HashMap::new();
        let none_s = std::collections::HashSet::new();
        let none_rb = std::collections::HashMap::new();
        let (out, notes) = update_rows_with_clips(page, &laps, ghosts, "ship17c", 0.1, &none_h, &none_s, &lid, &none_rb, &clips);
        assert!(!out.contains("⚠"), "{out}");
        assert!(notes.iter().any(|n| n == "05: lid line removed"), "{notes:?}");
        // while the video is still the ship15 clip, the line stays
        clips.insert("05".to_string(), "05-ghost-18.298-ship15".to_string());
        let (kept, _) = update_rows_with_clips(page, &laps, ghosts, "ship17c", 0.1, &none_h, &none_s, &lid, &none_rb, &clips);
        assert!(kept.contains("⚠"), "{kept}");
        assert!(build_at_least_16("16") && build_at_least_16("17c") && !build_at_least_16("15") && !build_at_least_16(""));
    }
}

/// map → clip name of the row's PUBLISHED video (the last ships.tsv row per map
/// whose status is a URL). Pending/staged/held rows are not on the page.
pub fn published_clip_names(ships: &str) -> std::collections::HashMap<String, String> {
    let mut out = std::collections::HashMap::new();
    for l in ships.lines().filter(|l| !l.starts_with('#')) {
        let c: Vec<&str> = l.split('\t').map(str::trim).collect();
        if c.len() >= 5 && c[4].starts_with("https://") {
            out.insert(c[0].to_string(), c[2].to_string());
        }
    }
    out
}

#[cfg(test)]
mod published_clip_tests {
    use super::*;

    #[test]
    fn only_a_url_row_names_the_rows_video() {
        let s = "05\t18.298\t05-ghost-18.298-ship15\t/x\thttps://github.com/user-attachments/assets/a\n05\t16.395\t05-ghost-16.395-ship17c\t/x\tpending\n19\t38.276\t19-ghost-38.276-ship15\t/x\tstaged\n";
        let m = published_clip_names(s);
        assert_eq!(m.get("05").map(String::as_str), Some("05-ghost-18.298-ship15"));
        assert_eq!(m.get("19"), None);
    }
}

/// `clip=<name>` at the start of a rowbuilds note: the exact clip that counts
/// as this row's video of that build (when its own suffix says another build).
pub fn note_clip(note: &str) -> Option<String> {
    let t = note.trim_start();
    let rest = t.strip_prefix("clip=")?;
    Some(rest.split_whitespace().next()?.to_string())
}

/// The note without its leading `clip=…` / `video=ok` markers.
pub fn note_text(note: &str) -> String {
    let mut t = note.trim();
    loop {
        if let Some(r) = t.strip_prefix("video=ok") {
            t = r.trim_start();
        } else if t.starts_with("clip=") {
            t = t.split_once(char::is_whitespace).map(|(_, r)| r).unwrap_or("").trim_start();
        } else {
            break;
        }
    }
    t.to_string()
}

#[cfg(test)]
mod clip_marker_tests {
    use super::*;

    /// `clip=<name>` in a rowbuilds note: that clip counts as the row's video of
    /// the row's build even when its own suffix says another build (05's ship17b
    /// render published as ship17c), and the marker is not printed.
    #[test]
    fn a_named_clip_counts_as_the_rows_build() {
        let rb = RowBuild { build: "ship17c".into(), link: String::new(), note: "clip=05-ghost-16.395-ship17b road-through-water section without drag — the original slows the car there".into() };
        let row = "**Tiny Summer 2026 - 05** — original author time `27.795` · tiny ghost **16.395** (build ship15, controls overlay)";
        let mut ships = std::collections::HashMap::new();
        ships.insert("05".to_string(), "05-ghost-16.395-ship17b".to_string());
        assert!(row_build_applies(&rb, row, Some("16.395"), &ships));
        ships.insert("05".to_string(), "05-ghost-18.298-ship15".to_string());
        assert!(!row_build_applies(&rb, row, Some("18.298"), &ships), "another clip does not count");
        assert_eq!(note_clip(&rb.note).as_deref(), Some("05-ghost-16.395-ship17b"));
        assert_eq!(note_text(&rb.note), "road-through-water section without drag — the original slows the car there");
        assert_eq!(note_text("video=ok clip=x-ship17b  frames identical"), "frames identical");
        // the water rule follows the named clip's ROW build (from the receipt)
        let receipt = "05\t16.395\tparent\tPUBLISHABLE (ship17c) water_ok=B";
        assert!(crate::video::water_b_rule(receipt, "05-ghost-16.395-ship17b", &rb.note), "17b-named clip, row build 17c via clip=");
        assert!(!crate::video::water_b_rule(receipt, "05-ghost-16.395-ship17b", "road water"), "without clip= the 17b suffix decides");
    }
}

/// The build tag a lap's README row names (`ship15`, `ship17c-a6a82a45` → `ship17c`).
pub fn lap_build(readme: &str, nn: &str, time: &str) -> Option<String> {
    for l in readme.lines() {
        let c: Vec<&str> = l.split('|').map(str::trim).collect();
        if c.len() >= 5 && c[1] == nn && c[2].ends_with(".Ghost.Gbx") && c[3] == time {
            for cell in &c[4..] {
                if let Some(rest) = cell.strip_prefix("ship") {
                    let tag: String = rest.chars().take_while(|ch| ch.is_ascii_alphanumeric()).collect();
                    if tag.chars().next().map(|ch| ch.is_ascii_digit()).unwrap_or(false) {
                        return Some(format!("ship{tag}"));
                    }
                }
            }
        }
    }
    None
}

#[cfg(test)]
mod lap_build_tests {
    use super::*;

    #[test]
    fn the_status_line_names_the_laps_own_build() {
        let r = "| 15 | 15.Ghost.Gbx | 42.454 | 8 | ship17c-a6a82a45 | a6a82a45 | GEN | x |\n| 19 | 19.Ghost.Gbx | 38.276 | 16 | ship15 | 5522d061 | PPO | y |\n";
        assert_eq!(lap_build(r, "15", "42.454").as_deref(), Some("ship17c"));
        assert_eq!(lap_build(r, "19", "38.276").as_deref(), Some("ship15"));
        assert_eq!(lap_build(r, "19", "46.362"), None);
        let line = status_line("42.454", "ship15", r, "15", &Note::Held("records".into()), 0.1);
        assert_eq!(line, "*latest lap **42.454** (build ship17c) — held (records)*");
    }
}

#[cfg(test)]
mod records_hold_tests {
    use super::*;

    /// A held map whose README newest lap is the public lap (or its 1-ms
    /// rounding twin) carries the hold's records line, not "latest lap … held".
    #[test]
    fn a_records_hold_reads_as_a_records_line_when_no_newer_lap_stands() {
        let ghosts = "| 22 | 22.Ghost.Gbx | 96.297 | 14 | ship15 | f1275f23 | GEN | x |\n| 25 | 25.Ghost.Gbx | 102.115 | 15 | ship15 | bd1a146f | GEN | y |\n";
        let page = "**Tiny Saudi Arabia 2026** — original author time `73.418` · tiny ghost **96.298** (build ship15, controls overlay)\n\n\
*latest lap **96.297** (build ship15) — held (records: 78.051 (certified; to be re-driven forwards))*\n\n\
https://github.com/user-attachments/assets/b\n\n\
**Tiny Japan 2026** — original author time `78.928` · tiny ghost **102.115** (build ship15, controls overlay)\n\n\
https://github.com/user-attachments/assets/c\n";
        let laps = newest_laps(ghosts, "ship15");
        assert_eq!(laps.iter().find(|(m, _)| m == "25").map(|(_, t)| t.as_str()), Some("102.115"), "{laps:?}");
        let mut holds = std::collections::HashMap::new();
        holds.insert("22".to_string(), "records: 78.051 (certified; to be re-driven forwards)".to_string());
        holds.insert("25".to_string(), "records: 83.772, 90.202, 86.518 (certified; to be re-driven forwards)".to_string());
        let rows = rows_of(page);
        assert_eq!(rows.iter().map(|(_, r)| (r.nn.as_str(), r.published.as_deref())).collect::<Vec<_>>(), vec![("22", Some("96.298")), ("25", Some("102.115"))]);
        let (out, notes) = update_all(page, &laps, ghosts, "ship15", 0.1, &holds, &std::collections::HashSet::new());
        assert!(out.contains("tiny ghost **96.298** (build ship15, controls overlay)\n\n*records: 78.051 (certified; to be re-driven forwards) — held (opening rework)*\n\nhttps://github.com/user-attachments/assets/b"), "{out}");
        assert!(out.contains("tiny ghost **102.115** (build ship15, controls overlay)\n\n*records: 83.772, 90.202, 86.518 (certified; to be re-driven forwards) — held (opening rework)*\n\nhttps://github.com/user-attachments/assets/c"), "{out}");
        assert!(!out.contains("latest lap **96.297**"), "{out}");
        let _ = notes;
        // idempotent
        let (again, n2) = update_all(&out, &laps, ghosts, "ship15", 0.1, &holds, &std::collections::HashSet::new());
        assert_eq!(again, out);
        assert!(n2.is_empty(), "{n2:?}");
        // a genuinely newer held lap still gets the latest-lap form
        let ghosts2 = "| 22 | 22.Ghost.Gbx | 84.379 | 14 | ship15 | f1275f23 | GEN | x |\n";
        let (newer, _) = update_all(&out, &newest_laps(ghosts2, "ship15"), ghosts2, "ship15", 0.1, &holds, &std::collections::HashSet::new());
        assert!(newer.contains("*latest lap **84.379** (build ship15) — held (records: 78.051 (certified; to be re-driven forwards))*"), "{newer}");
    }
}

/// `ship17c` → `ship17`, `ship17-d9549f05` → `ship17`: the build's numeric family.
pub fn build_family(b: &str) -> String {
    let rest = b.strip_prefix("ship").unwrap_or(b);
    let digits: String = rest.chars().take_while(|c| c.is_ascii_digit()).collect();
    if digits.is_empty() { b.to_string() } else { format!("ship{digits}") }
}

#[cfg(test)]
mod awaiting_session_tests {
    use super::*;

    #[test]
    fn build_family_drops_the_letter_and_the_hash() {
        assert_eq!(build_family("ship17c"), "ship17");
        assert_eq!(build_family("ship17-d9549f05"), "ship17");
        assert_eq!(build_family("ship15"), "ship15");
    }

    /// A row shipwatch accepted (receipt + gates) that waits for a GitHub
    /// session reads "staged, awaiting the upload session".
    #[test]
    fn a_pending_row_reads_awaiting_the_upload_session() {
        let ghosts = "| 19 | 19.Ghost.Gbx | 38.276 | 16 | ship15 | 5522d061 | PPO | x |\n- 19 38.276: stop: below 8 m/s: 0.02 s, respawns: 0 · attitude: PASS (0) · water: on-lid s A 0.00 · B 0.00 · S 0.00 — clean\n";
        let page = "**Tiny Summer 2026 - 19** — original author time `44.6` · tiny ghost **46.362** (build ship15, controls overlay)\n\nhttps://github.com/user-attachments/assets/a\n";
        set_pending_rows("19\t38.276\t19-ghost-38.276-ship15\t/x\tpending\n");
        let laps = newest_laps(ghosts, "ship15");
        let staged = staged_laps("19\t38.276\t19-ghost-38.276-ship15\t/x\tpending\n");
        let (out, _) = update_all(page, &laps, ghosts, "ship15", 0.1, &std::collections::HashMap::new(), &staged);
        assert!(out.contains("*latest lap **38.276** (build ship15) — staged, awaiting the upload session*"), "{out}");
        set_pending_rows("");
    }
}
