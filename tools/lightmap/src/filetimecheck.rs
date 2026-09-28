//! `lmtool filetime-check MAP [MAP…] [--tsv OUT]` — THE CACHE FILETIME RULE (the MK64 flat-lightmap lane's read, coordinator
//! 2026-09-28 19:58Z): cache chunk 0x06022013's FILETIME word = TimeWriteMostRecentSolid = the MAX CPlugSolid2Model.FileWriteTime
//! over the map's EMBEDDED item models; the game REJECTS the lightmap chunk in play when the word differs (→ the coarse load-time
//! bake). A byte-identical lightmap can therefore still be refused in play by this one word — the matrix compares files, this row
//! compares the word with its rule. Per map: the word, the max solid time over the embedded items (with the item that carries it),
//! the verdict (EQUAL / OFF by Δ seconds / no embedded solid), and, for a pair `OURS --against EDITOR`, ours vs the editor's word.
//! Times print as FILETIME ticks (100 ns since 1601-01-01) and as UTC.

use std::collections::BTreeMap;

pub struct FileTimeRead {
    pub path: String,
    /// the 0x06022013 word (None = no such cache chunk / too short)
    pub word: Option<u64>,
    /// (max FileWriteTime, the item carrying it, embedded items read, items without a solid) over EVERY embedded item file
    pub solids: Option<(u64, String, usize, usize)>,
    /// the same over the PLACED items only
    pub placed: Option<(u64, String, usize, usize)>,
}

fn filetime_to_utc(ft: u64) -> String {
    // 100-ns ticks since 1601-01-01 → unix seconds → a civil date (proleptic Gregorian, Howard Hinnant's algorithm)
    let unix = (ft / 10_000_000) as i64 - 11_644_473_600;
    let days = unix.div_euclid(86_400);
    let secs = unix.rem_euclid(86_400);
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    format!("{y:04}-{m:02}-{d:02}T{:02}:{:02}:{:02}Z", secs / 3600, (secs / 60) % 60, secs % 60)
}

/// The cache word of a saved map's lightmap chunk.
pub fn cache_word(m: &crate::mapio::MapLightmap) -> Option<u64> {
    let d = m.chunk.data.as_ref()?;
    for c in &d.cache.chunks {
        if c.id == 0x0602_2013 {
            if let crate::format::ChunkBody::Raw(b) = &c.body { if b.len() >= 16 { return Some(u64::from_le_bytes(b[8..16].try_into().ok()?)); } }
        }
    }
    None
}

/// The max CPlugSolid2Model.FileWriteTime over the map's embedded `.Item.Gbx` files (a static-object item's solid, or every
/// static-object entity of a prefab item). `placed_only`: only the models the map's item list PLACES (an embedded zip can carry
/// more files than the map uses — np-tk3 embeds tiny16's 496-item library and places 30).
pub fn max_solid_time(map_path: &str, placed_only: bool) -> Result<Option<(u64, String, usize, usize)>, String> {
    let mf = tmmaps::map::MapFile::load(std::path::Path::new(map_path));
    let items: BTreeMap<String, Vec<u8>> = mapgeom::embedded::items(&mf)?;
    let placed: std::collections::HashSet<String> = mf.items.iter().map(|it| it.model.rsplit(['/', '\\']).next().unwrap_or(&it.model).to_lowercase()).collect();
    let mut best: Option<(u64, String)> = None;
    let (mut read, mut no_solid) = (0usize, 0usize);
    for (name, bytes) in &items {
        if placed_only && !placed.contains(name) { continue; }
        read += 1;
        let Ok(f) = mapgeom::static_item::file::parse_file(bytes) else { no_solid += 1; continue };
        let mut times: Vec<u64> = Vec::new();
        if let Some(so) = f.item.static_object() { if let Some(s2) = so.solid2() { times.push(s2.file_write_time); } }
        if let Some(pf) = f.item.prefab() {
            for e in &pf.ents {
                if let Some(mapgeom::static_item::Node::StaticObject(so)) = e.model.inline.as_deref() { if let Some(s2) = so.solid2() { times.push(s2.file_write_time); } }
            }
        }
        let Some(t) = times.into_iter().max() else { no_solid += 1; continue };
        if best.as_ref().map_or(true, |(b, _)| t > *b) { best = Some((t, name.clone())); }
    }
    Ok(best.map(|(t, n)| (t, n, read, no_solid)))
}

pub fn read(path: &str) -> Result<FileTimeRead, String> {
    let m = crate::mapio::load(path)?;
    Ok(FileTimeRead { path: path.to_string(), word: cache_word(&m), solids: max_solid_time(path, false)?, placed: max_solid_time(path, true)? })
}

pub fn verdict(r: &FileTimeRead) -> String {
    let w = match r.word { Some(w) => w, None => return "no 0x06022013 word".to_string() };
    match (&r.solids, &r.placed) {
        (Some((t, ..)), Some((p, ..))) if w == *t && w == *p => "EQUAL to the max solid time (all embedded = placed)".to_string(),
        (Some((t, ..)), Some((p, ..))) if w == *p && w != *t => "EQUAL to the max over the PLACED items' models (not over every embedded file)".to_string(),
        (Some((t, ..)), Some((p, ..))) if w == *t && w != *p => "EQUAL to the max over EVERY embedded file (not the placed set)".to_string(),
        (Some((t, ..)), Some((p, ..))) => format!("OFF: {:+} s from the placed max, {:+} s from the all-embedded max (the game rejects the chunk in play)", (w as i128 - *p as i128) / 10_000_000, (w as i128 - *t as i128) / 10_000_000),
        (Some((t, ..)), None) => format!("no placed embedded item; {:+} s from the all-embedded max", (w as i128 - *t as i128) / 10_000_000),
        (None, _) => "no embedded solid (stock-only map: the rule's word is the game's own)".to_string(),
    }
}
pub fn run(paths: &[String], against: Option<&str>, tsv: Option<&str>) -> Result<(), String> {
    let mut out = String::from("file\tword_ticks\tword_utc\tmax_solid_ticks\tmax_solid_utc\tsolid_item\titems_read\titems_without_solid\tverdict\n");
    let other = match against { Some(p) => Some(read(p)?), None => None };
    for p in paths {
        let r = read(p)?;
        let (wt, wu) = match r.word { Some(w) => (w.to_string(), filetime_to_utc(w)), None => ("-".into(), "-".into()) };
        let (st, su, si, nr, ns) = match &r.solids { Some((t, n, nr, ns)) => (t.to_string(), filetime_to_utc(*t), n.clone(), *nr, *ns), None => ("-".into(), "-".into(), "-".into(), 0, 0) };
        let placed = match &r.placed { Some((t, n, nr, _)) => format!("{t} ({}) from {n}; {nr} placed models", filetime_to_utc(*t)), None => "-".into() };
        let v = verdict(&r);
        println!("{p}\n  0x06022013 word: {wt} ({wu})\n  max CPlugSolid2Model.FileWriteTime over EVERY embedded item: {st} ({su}) from {si}; {nr} embedded items read, {ns} without a solid\n  max over the PLACED items' models: {placed}\n  → {v}");
        if let Some(o) = &other {
            match (r.word, o.word) {
                (Some(a), Some(b)) if a == b => println!("  vs {}: the words are EQUAL", o.path),
                (Some(a), Some(b)) => println!("  vs {}: ours {a} vs {b} ({:+} s) — {}", o.path, (a as i128 - b as i128) / 10_000_000, if o.solids.as_ref().map_or(false, |(t, ..)| *t == b) { "the editor's word = its max solid time (the rule holds there)" } else { "the editor's word ≠ its max solid time" }),
                _ => println!("  vs {}: a word is missing", o.path),
            }
        }
        out.push_str(&format!("{p}\t{wt}\t{wu}\t{st}\t{su}\t{si}\t{nr}\t{ns}\t{v}\n"));
    }
    if let Some(t) = tsv { std::fs::write(t, out).map_err(|e| format!("{t}: {e}"))?; }
    Ok(())
}
