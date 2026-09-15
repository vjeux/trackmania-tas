//! `tinyctl wrwipe` — the records-reset preview for Everios96 (option b, 2026-09-15):
//! per alteration, every map whose driving surface changed after first publish AND that
//! has records, with the record count (capped at what was crawled), the WR time and the
//! WR holder. Names come from a `tmio names` file; unknown holders print their account id.
//!
//! ```text
//! tinyctl wrwipe --records 'records-alt-*.tsv' --changes-dir /path/records --names names.tsv
//!                --uids-dir /path/credits --out-dir DIR
//! ```
//! `--changes-dir` holds `<set>-physics-changes.tsv` (`pNN\tNN\tround`), `--uids-dir` the
//! `uids-<set>.tsv` lists (`uid\tname`) in club order (part × 25 + index = map number).
//! Output: `wrwipe-<set>.tsv`, `wrwipe-summary.txt`, and `wr-holders.txt` (account ids to name).

use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::Path;

fn flag<'a>(args: &'a [String], k: &str) -> Option<&'a str> {
    args.iter().position(|a| a == k).and_then(|i| args.get(i + 1)).map(|s| s.as_str())
}

fn ms(score: u64) -> String {
    // seconds with a decimal, never raw milliseconds (house rule)
    format!("{}.{:03}", score / 1000, score % 1000)
}

pub fn cmd(args: &[String]) -> Result<(), String> {
    let records = flag(args, "--records").ok_or("--records 'records-alt-*.tsv'")?;
    let changes_dir = Path::new(flag(args, "--changes-dir").ok_or("--changes-dir")?);
    let uids_dir = Path::new(flag(args, "--uids-dir").ok_or("--uids-dir")?);
    let names_p = flag(args, "--names").unwrap_or("");
    let out_dir = Path::new(flag(args, "--out-dir").ok_or("--out-dir")?);
    std::fs::create_dir_all(out_dir).map_err(|e| e.to_string())?;
    let names: HashMap<String, String> = std::fs::read_to_string(names_p).unwrap_or_default().lines().skip(1).filter_map(|l| { let mut it = l.split('\t'); Some((it.next()?.to_string(), it.next()?.to_string())) }).filter(|(_, n)| !n.is_empty()).collect();

    // records per uid: count, WR (position 1) account + score
    struct Board { n: usize, wr: Option<(String, u64)> }
    let mut boards: HashMap<String, Board> = HashMap::new();
    for f in super::credits_expand(records) {
        let text = std::fs::read_to_string(&f).map_err(|e| format!("{}: {e}", f.display()))?;
        for l in text.lines().skip(1) {
            let c: Vec<&str> = l.split('\t').collect();
            if c.len() < 5 {
                continue;
            }
            let b = boards.entry(c[0].to_string()).or_insert(Board { n: 0, wr: None });
            b.n += 1;
            if c[3] == "1" {
                b.wr = Some((c[2].to_string(), c[4].parse().unwrap_or(0)));
            }
        }
    }
    let sets = ["plastic", "ice", "grass", "dirt", "wood", "woodwet", "magnet"];
    let labels: HashMap<&str, &str> = [("plastic", "Plastic"), ("ice", "Icy"), ("grass", "Grass"), ("dirt", "Dirt"), ("wood", "Wood"), ("woodwet", "Wet Wood"), ("magnet", "Magnet")].into_iter().collect();
    let mut summary = String::new();
    let mut holders: HashSet<String> = HashSet::new();
    for s in sets {
        // the set's uids in club order → map number = (part-1)*25 + index
        let uids_text = std::fs::read_to_string(uids_dir.join(format!("uids-{s}.tsv"))).map_err(|e| format!("uids-{s}.tsv: {e}"))?;
        let uids: Vec<(String, String)> = uids_text.lines().filter_map(|l| { let mut it = l.split('\t'); Some((it.next()?.to_string(), it.next().unwrap_or("").to_string())) }).collect();
        // "U10S_01 By Everios96 [Plastic]" → 1 (the number after `U10S_`, never the 10 of U10S)
        let by_num: HashMap<u32, (String, String)> = uids.iter().filter_map(|(u, n)| n.split("U10S_").nth(1).and_then(|r| r.chars().take_while(|c| c.is_ascii_digit()).collect::<String>().parse::<u32>().ok()).map(|k| (k, (u.clone(), n.clone())))).collect();
        // which map numbers changed physically
        let ch_text = std::fs::read_to_string(changes_dir.join(format!("{s}-physics-changes.tsv"))).unwrap_or_default();
        let mut changed: BTreeMap<u32, String> = BTreeMap::new();
        for l in ch_text.lines() {
            let c: Vec<&str> = l.split('\t').collect();
            if c.len() < 2 {
                continue;
            }
            let (Ok(p), Ok(i)) = (c[0].trim_start_matches('p').parse::<u32>(), c[1].parse::<u32>()) else { continue };
            let num = (p - 1) * 25 + i;
            let e = changed.entry(num).or_default();
            if !e.is_empty() {
                e.push(',');
            }
            e.push_str(c.get(2).unwrap_or(&""));
        }
        // magnet: EVERY map changed this morning (deck rule); wood/wet wood: every map (body record)
        if matches!(s, "magnet" | "wood" | "woodwet") {
            for k in by_num.keys() {
                changed.entry(*k).or_insert_with(|| if s == "magnet" { "fix10-deck-rule".into() } else { "fix9-body-record".into() });
            }
        }
        let mut t = String::from("map\tname\tuid\trecords\twr_time\twr_holder\twr_account\tchanged_in\n");
        let (mut n_maps, mut n_with_records, mut n_records) = (0usize, 0usize, 0usize);
        for (num, rounds) in &changed {
            let Some((uid, name)) = by_num.get(num) else { continue };
            n_maps += 1;
            let Some(b) = boards.get(uid) else { continue };
            if b.n == 0 {
                continue;
            }
            n_with_records += 1;
            n_records += b.n;
            let (wr_acc, wr_time, wr_name) = match &b.wr {
                Some((a, sc)) => {
                    holders.insert(a.clone());
                    (a.clone(), ms(*sc), names.get(a).cloned().unwrap_or_default())
                }
                None => (String::new(), String::new(), String::new()),
            };
            t.push_str(&format!("{num}\t{name}\t{uid}\t{}\t{wr_time}\t{wr_name}\t{wr_acc}\t{rounds}\n", if b.n >= 100 { "100+".to_string() } else { b.n.to_string() }));
        }
        std::fs::write(out_dir.join(format!("wrwipe-{s}.tsv")), t).map_err(|e| e.to_string())?;
        let line = format!("{:<9} {n_maps:>4} maps to re-uid, {n_with_records:>4} of them have records ({n_records}+ records, {} WRs wiped)\n", labels[s], n_with_records);
        summary.push_str(&line);
        eprint!("{line}");
    }
    std::fs::write(out_dir.join("wrwipe-summary.txt"), &summary).map_err(|e| e.to_string())?;
    let mut h: Vec<String> = holders.into_iter().collect();
    h.sort();
    std::fs::write(out_dir.join("wr-holders.txt"), h.join("\n") + "\n").map_err(|e| e.to_string())?;
    println!("{} distinct WR holders to name -> {}", h.len(), out_dir.join("wr-holders.txt").display());
    Ok(())
}
