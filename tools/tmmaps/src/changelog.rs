//! `tmmaps changelog CHANGES.tsv --out DIR [--title T] [--rounds ROUNDS.tsv]`
//!
//! Renders the hunters' change log of the Altered U10S sets (GeraldDuke, wet wood
//! 2026-09-22 03:04: "is it possible for changelogs to be made?") from the rows
//! the box's `changelog.sh` writes — one row per (set, map, published version
//! that differed from the previous one):
//!
//! `set \t map \t round-dir \t MM-DD \t n-diff \t kinds \t after-wipe:yes|no`
//!
//! where `kinds` is `Kind×N Kind×N …` (the block / item names whose physics
//! fingerprint rows changed, most frequent first; `embedded` = custom items and
//! custom blocks). Output: `DIR/README.md` (method + totals) and one
//! `DIR/<set>.md` per alteration, a map per line, newest version first.
//! `--rounds` maps a round-dir to a human label (`thumbs23 \t what the round did`).

use std::collections::BTreeMap;
use std::path::Path;

#[derive(Clone, Debug)]
struct Row {
    set: String,
    map: u32,
    round: String,
    date: String,
    n: u32,
    kinds: Vec<(String, u32)>,
    after_wipe: bool,
    /// the version that RESET the records (new uid = new leaderboard)
    wipe: bool,
}

/// The reader-facing name of an alteration folder.
fn set_label(set: &str) -> &str {
    match set {
        "ice" => "Icy",
        "plastic" => "Plastic",
        "grass" => "Grass",
        "dirt" => "Dirt",
        "wood" => "Wood",
        "woodwet" => "Wet Wood",
        "magnet" => "Magnet",
        "tiny" => "Tiny",
        "giant" => "Giant",
        other => other,
    }
}

/// A plain-English family for a changed block / item name — the hunters read
/// "platform tops (5)", not `DecoPlatformIceSlope2Straight×5`.
pub fn kind_family(kind: &str) -> &'static str {
    let k = kind;
    if k == "embedded" {
        return "custom items / custom blocks";
    }
    if k.contains("Pillar") || k.contains("Filler") {
        return "pillars (not driven)";
    }
    if k.contains("Penalty") {
        return "penalty road frames";
    }
    if k.contains("Special") || k.contains("Turbo") || k.contains("Boost") || k.contains("Reset") || k.contains("Fragile") || k.contains("NoEngine") || k.contains("NoBrake") || k.contains("SlowMotion") || k.contains("Cruise") {
        return "gameplay pads (turbo / boost / reset / fragile …)";
    }
    if k.starts_with("Gate") {
        return "gates";
    }
    if k.starts_with("Water") || k.contains("DecoCliff") {
        return "pool edges / cliffs";
    }
    if k.starts_with("DecoPlatform") || k.starts_with("Platform") {
        return "platform decks and tops";
    }
    if k.starts_with("Road") || k.starts_with("Open") || k.contains("Road") {
        return "road pieces";
    }
    if k.starts_with("TrackWall") || k.starts_with("Structure") {
        return "wall-rides / structures";
    }
    if k.starts_with("Deco") {
        return "decoration blocks";
    }
    "other blocks"
}

fn parse(path: &Path) -> Result<Vec<Row>, String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let mut rows = Vec::new();
    for line in text.lines() {
        let f: Vec<&str> = line.split('\t').collect();
        if f.len() < 7 {
            continue;
        }
        let map: u32 = match f[1].trim().parse() {
            Ok(v) => v,
            Err(_) => continue,
        };
        let n: u32 = f[4].trim().parse().unwrap_or(0);
        let kinds = f[5]
            .split_whitespace()
            .filter_map(|t| {
                let (k, c) = t.rsplit_once('×')?;
                Some((k.to_string(), c.parse::<u32>().unwrap_or(0)))
            })
            .collect();
        let tag = f[6].trim();
        rows.push(Row { set: f[0].to_string(), map, round: f[2].to_string(), date: f[3].to_string(), n, kinds, after_wipe: tag == "after-wipe:yes", wipe: tag == "wiped" });
    }
    Ok(rows)
}

/// round-dir → (publish rank, human label); the rank is the line order of the
/// rounds file, which lists the rounds in publish order (a string sort would put
/// `thumbs7` after `thumbs23`)
fn read_rounds(path: Option<&str>) -> BTreeMap<String, (usize, String)> {
    let mut m = BTreeMap::new();
    if let Some(p) = path {
        if let Ok(t) = std::fs::read_to_string(p) {
            for (i, l) in t.lines().enumerate() {
                if let Some((k, v)) = l.split_once('\t') {
                    m.insert(k.trim().to_string(), (i, v.trim().to_string()));
                }
            }
        }
    }
    m
}

/// One version's description: the families of what changed, largest first,
/// with the raw block names in parentheses (capped).
fn describe(kinds: &[(String, u32)]) -> String {
    let mut fam: BTreeMap<&'static str, (u32, Vec<String>)> = BTreeMap::new();
    for (k, c) in kinds {
        let e = fam.entry(kind_family(k)).or_insert((0, Vec::new()));
        e.0 += c;
        if e.1.len() < 4 {
            e.1.push(k.clone());
        }
    }
    let mut v: Vec<(&str, u32, Vec<String>)> = fam.into_iter().map(|(f, (c, names))| (f, c, names)).collect();
    v.sort_by(|a, b| b.1.cmp(&a.1));
    v.into_iter().map(|(f, c, names)| format!("{f} ({c}: {})", names.join(", "))).collect::<Vec<_>>().join("; ")
}

pub fn cmd(args: &[String]) -> Result<(), String> {
    let input = args.iter().skip(1).find(|a| !a.starts_with("--")).ok_or("tmmaps changelog CHANGES.tsv --out DIR [--title T] [--rounds ROUNDS.tsv] [--generated 'when']")?;
    let out = crate::cli::flag(args, "--out").ok_or("--out DIR")?;
    let title = crate::cli::flag(args, "--title").unwrap_or("Altered U10S — change log");
    let generated = crate::cli::flag(args, "--generated").map(|s| s.to_string()).unwrap_or_else(|| "now".to_string());
    let rounds = read_rounds(crate::cli::flag(args, "--rounds"));
    let rows = parse(Path::new(input))?;
    std::fs::create_dir_all(out).map_err(|e| format!("{out}: {e}"))?;

    let mut by_set: BTreeMap<String, Vec<Row>> = BTreeMap::new();
    for r in rows {
        by_set.entry(r.set.clone()).or_default().push(r);
    }
    let order = ["ice", "wood", "woodwet", "plastic", "grass", "dirt", "magnet", "tiny", "giant"];
    let mut readme = String::new();
    readme.push_str(&format!("# {title}\n\nGenerated {generated}. One file per alteration; one line per map per published version that changed something in the MAP itself (blocks, items, physics) since the previous version.\n\n"));
    readme.push_str("**How to read a line:** `U10S_26 — 09-20 · after last wipe · 2 changes: custom items / custom blocks (1: …)` means: on Sept 20 the published Wet Wood 26 was replaced by a version whose custom block changed, the records were NOT reset for it, so the records on the board were driven on the previous version. `wiped` marks the version where the records WERE reset (new leaderboard).\n\n");
    readme.push_str("**What counts as a change:** the physics fingerprint of the map — every authored block (name, variant, cell, rotation) and every embedded item's collision hulls per placement. Thumbnail-only, name-only or lighting-only republishes are not listed. `pillars (not driven)` are generated filler pillars re-emitted by the converter — visual, never touched by the car.\n\n");
    readme.push_str("| Alteration | maps with listed changes | changed versions |\n|---|---|---|\n");
    let mut total_maps = 0usize;
    let mut total_rows = 0usize;
    for s in order.iter().chain(by_set.keys().filter(|k| !order.contains(&k.as_str())).map(|k| k.as_str()).collect::<Vec<_>>().iter()) {
        let Some(rs) = by_set.get(*s) else { continue };
        let maps: std::collections::BTreeSet<u32> = rs.iter().filter(|r| !r.wipe).map(|r| r.map).collect();
        let changes = rs.iter().filter(|r| !r.wipe).count();
        total_maps += maps.len();
        total_rows += changes;
        readme.push_str(&format!("| [{}]({}.md) | {} | {} |\n", set_label(s), s, maps.len(), changes));

        let mut md = String::new();
        md.push_str(&format!("# {} — change log\n\nGenerated {generated}. Newest version first per map. `wiped` = records reset with that version; `after last wipe` = the change went live on top of existing records (the times on the board were set on the earlier version).\n\n", set_label(s)));
        let mut by_map: BTreeMap<u32, Vec<&Row>> = BTreeMap::new();
        for r in rs {
            by_map.entry(r.map).or_default().push(r);
        }
        for (map, mut versions) in by_map {
            let rank = |r: &Row| rounds.get(&r.round).map(|(i, _)| *i).unwrap_or(usize::MAX);
            versions.sort_by(|a, b| b.date.cmp(&a.date).then(rank(b).cmp(&rank(a))));
            // a map never wiped: its records span every change (Tiny's in-place rounds)
            let ever_wiped = versions.iter().any(|v| v.wipe);
            md.push_str(&format!("### U10S_{map:02}\n"));
            for v in versions {
                let label = rounds.get(&v.round).map(|(_, l)| format!(" · {l}")).unwrap_or_default();
                if v.wipe {
                    let with = if v.n > 0 { format!(" · came with {} change{}: {}", v.n, if v.n == 1 { "" } else { "s" }, describe(&v.kinds)) } else { String::new() };
                    md.push_str(&format!("- **{}**{} · **records reset — new leaderboard from here**{with}\n", v.date, label));
                    continue;
                }
                let flag = if v.after_wipe { "after last wipe" } else if ever_wiped { "before the last wipe (records already reset since)" } else { "records kept across this change (never reset)" };
                md.push_str(&format!("- **{}**{} · {} · {} change{}: {}\n", v.date, label, flag, v.n, if v.n == 1 { "" } else { "s" }, describe(&v.kinds)));
            }
            md.push('\n');
        }
        std::fs::write(Path::new(out).join(format!("{s}.md")), md).map_err(|e| e.to_string())?;
    }
    readme.push_str(&format!("| **total** | **{total_maps}** | **{total_rows}** |\n"));
    // WIPES.md — every records reset ever, per alteration: one line per wipe round (date,
    // label, how many maps, which), then per map the dates its leaderboard was reset
    // (Everios96 2026-09-26: "give a list of all record wipes")
    let mut wipes = String::new();
    wipes.push_str(&format!("# {title} — record wipes\n\nGenerated {generated}. A wipe = the map re-created under a new id: the leaderboard starts empty from that date. Maps not listed under an alteration were never wiped there (their records run since the first publish).\n\n"));
    let mut grand = 0usize;
    for s in order.iter().chain(by_set.keys().filter(|k| !order.contains(&k.as_str())).map(|k| k.as_str()).collect::<Vec<_>>().iter()) {
        let Some(rs) = by_set.get(*s) else { continue };
        let ws: Vec<&Row> = rs.iter().filter(|r| r.wipe).collect();
        if ws.is_empty() {
            continue;
        }
        let mut by_round: BTreeMap<(String, String), Vec<u32>> = BTreeMap::new();
        for r in &ws {
            by_round.entry((r.date.clone(), r.round.clone())).or_default().push(r.map);
        }
        let wiped_maps: std::collections::BTreeSet<u32> = ws.iter().map(|r| r.map).collect();
        grand += ws.len();
        wipes.push_str(&format!("## {}\n\n{} wipe round{}, {} map-resets, {} distinct maps ever wiped (of 975).\n\n", set_label(s), by_round.len(), if by_round.len() == 1 { "" } else { "s" }, ws.len(), wiped_maps.len()));
        for ((date, round), mut maps) in by_round {
            maps.sort_unstable();
            maps.dedup();
            let label = rounds.get(&round).map(|(_, l)| l.clone()).unwrap_or_else(|| round.clone());
            let list = if maps.len() == 975 { "all 975".to_string() } else { maps.iter().map(|m| m.to_string()).collect::<Vec<_>>().join(", ") };
            wipes.push_str(&format!("- **{date}** · {label} · {} map{}: {list}\n", maps.len(), if maps.len() == 1 { "" } else { "s" }));
        }
        wipes.push_str("\n<details><summary>per map</summary>\n\n");
        let mut per_map: BTreeMap<u32, Vec<String>> = BTreeMap::new();
        for r in &ws {
            per_map.entry(r.map).or_default().push(r.date.clone());
        }
        for (m, mut dates) in per_map {
            dates.sort();
            dates.dedup();
            wipes.push_str(&format!("U10S_{m}: {}  \n", dates.join(", ")));
        }
        wipes.push_str("\n</details>\n\n");
    }
    wipes.push_str(&format!("**Total: {grand} map-resets across all alterations.**\n"));
    std::fs::write(Path::new(out).join("WIPES.md"), wipes).map_err(|e| e.to_string())?;
    readme.push_str("\nEvery records reset, per alteration and per map: [WIPES.md](WIPES.md).\n");
    std::fs::write(Path::new(out).join("README.md"), readme).map_err(|e| e.to_string())?;
    println!("changelog: {} alterations, {total_maps} maps, {total_rows} versions -> {out}", by_set.len());
    Ok(())
}
