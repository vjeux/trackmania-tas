//! `tinyctl credits` — the U10S map-1000 credits lists (Everios96, 2026-09-15).
//!
//! ```text
//! tinyctl credits --records 'records-original-*.tsv' --names names.tsv --discord discord-people.tsv
//!                 --out-dir DIR [--shots 50,200,1000,2000] [--w-wr 4 --w-top10 2 --w-maps 1 --w-discord 1]
//! ```
//!
//! Inputs: the `nadeo-here leaderboards` shard outputs (`mapUid mapName accountId position
//! score`), its `names.tsv` (`accountId displayName`) and `discordctl people` (`userId username
//! displayName messages …`). Per person: maps with a record, top-10 finishes, WRs (position 1),
//! best position, Discord messages (matched by display name, case-insensitive; a Discord user
//! whose name matches no player is kept with 0 records — they interact, so they count).
//!
//! Importance = a weighted sum of RANK scores (each metric ranked over everyone with a nonzero
//! value; rank score = 1 − rank/n, so 1,000 messages and 300 records are comparable), then cut
//! into shots from the top: `--shots 50,200,1000,2000` = shot 4 (top 50), shot 3 (next 200),
//! shot 2 (next 1,000), shot 1 (next 2,000). Everyone is in at most one shot.
//!
//! Outputs in `--out-dir`: `credits-all.tsv` (every person, all metrics, score, shot),
//! `credits-shot1.txt` … `credits-shot4.txt` (names only, one per line, most important first),
//! `credits-unmatched-discord.tsv` (Discord names that matched no player).

use std::collections::HashMap;
use std::path::Path;

#[derive(Default, Clone)]
struct Person {
    name: String,
    account: String,
    discord_user: String,
    maps: u32,
    top10: u32,
    wr: u32,
    best: u32,
    sum_pos: u64,
    messages: u64,
    score: f64,
    shot: u8,
}

fn flag<'a>(args: &'a [String], k: &str) -> Option<&'a str> {
    args.iter().position(|a| a == k).and_then(|i| args.get(i + 1)).map(|s| s.as_str())
}

fn expand(pattern: &str) -> Vec<std::path::PathBuf> {
    // a shell-style '*' in the file name only
    let p = Path::new(pattern);
    let dir = p.parent().filter(|d| !d.as_os_str().is_empty()).unwrap_or(Path::new("."));
    let fname = p.file_name().map(|f| f.to_string_lossy().to_string()).unwrap_or_default();
    let Some(star) = fname.find('*') else { return vec![p.to_path_buf()] };
    let (pre, post) = (&fname[..star], &fname[star + 1..]);
    let mut out: Vec<_> = std::fs::read_dir(dir)
        .map(|rd| rd.filter_map(|e| e.ok()).map(|e| e.path()).filter(|q| q.file_name().map(|f| { let f = f.to_string_lossy(); f.starts_with(pre) && f.ends_with(post) && f.len() >= pre.len() + post.len() }).unwrap_or(false)).collect())
        .unwrap_or_default();
    out.sort();
    out
}

pub fn cmd(args: &[String]) -> Result<(), String> {
    let records = flag(args, "--records").ok_or("--records 'records-*.tsv'")?;
    let names_p = flag(args, "--names").ok_or("--names names.tsv")?;
    let discord_p = flag(args, "--discord").ok_or("--discord discord-people.tsv")?;
    let out_dir = Path::new(flag(args, "--out-dir").ok_or("--out-dir DIR")?);
    std::fs::create_dir_all(out_dir).map_err(|e| format!("{}: {e}", out_dir.display()))?;
    let shots: Vec<usize> = flag(args, "--shots").unwrap_or("50,200,1000,2000").split(',').filter_map(|s| s.trim().parse().ok()).collect();
    let w = |k: &str, d: f64| flag(args, k).and_then(|v| v.parse().ok()).unwrap_or(d);
    let (w_wr, w_top10, w_maps, w_discord) = (w("--w-wr", 4.0), w("--w-top10", 2.0), w("--w-maps", 1.0), w("--w-discord", 1.0));

    // 1. records → per account
    let mut by_account: HashMap<String, Person> = HashMap::new();
    let mut n_rows = 0usize;
    let mut maps_seen: std::collections::HashSet<String> = Default::default();
    for f in expand(records) {
        let text = std::fs::read_to_string(&f).map_err(|e| format!("{}: {e}", f.display()))?;
        for l in text.lines().skip(1) {
            let c: Vec<&str> = l.split('\t').collect();
            if c.len() < 5 {
                continue;
            }
            let pos: u32 = c[3].parse().unwrap_or(0);
            if pos == 0 {
                continue;
            }
            maps_seen.insert(c[0].to_string());
            let p = by_account.entry(c[2].to_string()).or_default();
            p.account = c[2].to_string();
            p.maps += 1;
            p.sum_pos += pos as u64;
            if pos <= 10 {
                p.top10 += 1;
            }
            if pos == 1 {
                p.wr += 1;
            }
            if p.best == 0 || pos < p.best {
                p.best = pos;
            }
            n_rows += 1;
        }
    }
    eprintln!("{n_rows} records over {} maps, {} accounts", maps_seen.len(), by_account.len());

    // 2. names
    let names_text = std::fs::read_to_string(names_p).unwrap_or_default();
    let mut named = 0usize;
    for l in names_text.lines().skip(1) {
        let mut it = l.split('\t');
        if let (Some(a), Some(n)) = (it.next(), it.next()) {
            if let Some(p) = by_account.get_mut(a) {
                p.name = n.to_string();
                named += 1;
            }
        }
    }
    eprintln!("{named} accounts named ({} without a display name)", by_account.len() - named);

    // 3. discord: match by display name / username (case-insensitive)
    let mut by_lower: HashMap<String, String> = HashMap::new(); // lower name -> account
    for (a, p) in &by_account {
        if !p.name.is_empty() {
            by_lower.entry(p.name.to_lowercase()).or_insert_with(|| a.clone());
        }
    }
    // --discord-accounts found.tsv (tinyctl tmio find): userId → accountId, matched by id first
    let found: HashMap<String, String> = flag(args, "--discord-accounts").and_then(|f| std::fs::read_to_string(f).ok()).map(|t| t.lines().skip(1).filter_map(|l| { let c: Vec<&str> = l.split('\t').collect(); (c.len() >= 5 && c[4].len() > 30).then(|| (c[0].to_string(), c[4].to_string())) }).collect()).unwrap_or_default();
    eprintln!("{} discord users resolved to accounts by trackmania.io", found.len());
    let disc_text = std::fs::read_to_string(discord_p).map_err(|e| format!("{discord_p}: {e}"))?;
    let mut unmatched: Vec<(String, String, u64)> = Vec::new();
    let mut extra: Vec<Person> = Vec::new();
    for l in disc_text.lines().skip(1) {
        let c: Vec<&str> = l.split('\t').collect();
        if c.len() < 4 {
            continue;
        }
        let (user, disp, msgs) = (c[1], c[2], c[3].parse::<u64>().unwrap_or(0));
        let by_id = found.get(c[0]).filter(|a| by_account.contains_key(*a)).cloned();
        let key = [disp, user].iter().map(|s| s.to_lowercase()).find(|k| !k.is_empty() && by_lower.contains_key(k));
        match by_id.or_else(|| key.and_then(|k| by_lower.get(&k)).cloned()) {
            Some(a) => {
                let p = by_account.get_mut(&a).unwrap();
                p.messages += msgs;
                p.discord_user = user.to_string();
            }
            None => {
                unmatched.push((user.to_string(), disp.to_string(), msgs));
                extra.push(Person { name: if disp.is_empty() { user.to_string() } else { disp.to_string() }, discord_user: user.to_string(), messages: msgs, ..Default::default() });
            }
        }
    }
    eprintln!("discord: {} matched to players, {} unmatched (kept with 0 records)", by_account.values().filter(|p| p.messages > 0).count(), unmatched.len());

    // 4. rank scores
    let mut people: Vec<Person> = by_account.into_values().chain(extra).collect();
    let rank_score = |vals: Vec<(usize, f64)>| -> HashMap<usize, f64> {
        // vals: (index, value>0) → 1 - rank/n (ties share the best rank)
        let mut v = vals;
        v.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
        let n = v.len().max(1) as f64;
        let mut out = HashMap::new();
        let mut i = 0;
        while i < v.len() {
            let mut j = i;
            while j + 1 < v.len() && v[j + 1].1 == v[i].1 {
                j += 1;
            }
            for k in i..=j {
                out.insert(v[k].0, 1.0 - (i as f64) / n);
            }
            i = j + 1;
        }
        out
    };
    let m_wr = rank_score(people.iter().enumerate().filter(|(_, p)| p.wr > 0).map(|(i, p)| (i, p.wr as f64)).collect());
    let m_top = rank_score(people.iter().enumerate().filter(|(_, p)| p.top10 > 0).map(|(i, p)| (i, p.top10 as f64)).collect());
    let m_maps = rank_score(people.iter().enumerate().filter(|(_, p)| p.maps > 0).map(|(i, p)| (i, p.maps as f64)).collect());
    let m_disc = rank_score(people.iter().enumerate().filter(|(_, p)| p.messages > 0).map(|(i, p)| (i, p.messages as f64)).collect());
    for (i, p) in people.iter_mut().enumerate() {
        p.score = w_wr * m_wr.get(&i).copied().unwrap_or(0.0) + w_top10 * m_top.get(&i).copied().unwrap_or(0.0) + w_maps * m_maps.get(&i).copied().unwrap_or(0.0) + w_discord * m_disc.get(&i).copied().unwrap_or(0.0);
    }
    people.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap().then(b.maps.cmp(&a.maps)).then(a.name.cmp(&b.name)));

    if let Some(n) = flag(args, "--emit-ids").and_then(|v| v.parse::<usize>().ok()) {
        let ids: Vec<String> = people.iter().filter(|p| !p.account.is_empty()).take(n).map(|p| p.account.clone()).collect();
        let p = out_dir.join("ids-to-name.txt");
        std::fs::write(&p, ids.join("\n") + "\n").map_err(|e| e.to_string())?;
        println!("{} account ids to name -> {}", ids.len(), p.display());
        return Ok(());
    }

    // 5. shots from the top: shots[0] → shot N (highest), …
    let n_shots = shots.len() as u8;
    let mut idx = 0usize;
    for (k, &size) in shots.iter().enumerate() {
        let shot = n_shots - k as u8;
        for p in people.iter_mut().skip(idx).take(size) {
            p.shot = shot;
        }
        idx += size;
    }

    // 6. write
    let mut all = String::from("shot\tname\taccountId\tdiscordUser\tmaps\ttop10\twr\tbestPos\tavgPos\tdiscordMessages\tscore\n");
    for p in &people {
        let avg = if p.maps > 0 { format!("{:.0}", p.sum_pos as f64 / p.maps as f64) } else { String::new() };
        all.push_str(&format!("{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{:.4}\n", if p.shot > 0 { p.shot.to_string() } else { String::new() }, p.name, p.account, p.discord_user, p.maps, p.top10, p.wr, p.best, avg, p.messages, p.score));
    }
    std::fs::write(out_dir.join("credits-all.tsv"), all).map_err(|e| e.to_string())?;
    for shot in 1..=n_shots {
        let names: Vec<String> = people.iter().filter(|p| p.shot == shot).map(|p| if p.name.is_empty() { p.account.clone() } else { p.name.clone() }).collect();
        std::fs::write(out_dir.join(format!("credits-shot{shot}.txt")), names.join("\n") + "\n").map_err(|e| e.to_string())?;
        eprintln!("shot {shot}: {} names", names.len());
    }
    let mut u = String::from("discordUser\tdisplayName\tmessages\n");
    unmatched.sort_by(|a, b| b.2.cmp(&a.2));
    for (a, b, c) in &unmatched {
        u.push_str(&format!("{a}\t{b}\t{c}\n"));
    }
    std::fs::write(out_dir.join("credits-unmatched-discord.tsv"), u).map_err(|e| e.to_string())?;
    println!("{} people scored → {}", people.len(), out_dir.display());
    Ok(())
}
