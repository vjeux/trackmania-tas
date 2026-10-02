//! `lmtool loadcheck MAP [MAP…] [--json OUT.json] [--tsv OUT.tsv] [--word17-max N] [--quiet]` — THE LOADS-IN-PLAY STATIC
//! PRE-CHECK (integrator baker-9, 2026-10-02): every word of the lightmap chunk the Fall 2026 night proved the client reads at
//! load, checked against the EDITOR's rule, so a file the game would hang on or reject is named BEFORE the box sees it. The real
//! check is the box (`tinyctl lightmap-run --check-only`: a PlayMap that shows a playground); this one refuses only what the
//! game is known to refuse. Rules — each a named row, the verdict PASS when every one holds, FAIL naming the rows that do not:
//!
//!   RK  record kinds: the mapping head's frame records (66 B each after the 60 constants) carry, in their first word, [2, 3]
//!       with two records and [3, 3, 2] with three (77 editor / Nadeo files, the Fall 2026 bisect); the record count = the
//!       image frame count. The port's [3, 3] with two frames = THE PLAY-LOAD HANG (A4a: the one word alone flips it).
//!   CW  cache words, the EDITOR's form over the 42 corpus oracles (baker-9's census 2026-10-02 15:00Z): 0x06022017 = (0, n) on 42/42
//!       (n = 0 on 35, a per-bake count 230 … 78 029 on 7 — no bound the game could check; `--word17-max N` adds one); 0x06022018 = 0
//!       on every terrain-collection bake (32/32) and 132393650849529910 (2020-07-16T09:24:44Z) on EVERY Stadium bake (10/10).
//!       The Nadeo template's FILETIME rode in 0x18 of every port-lit file until 2026-10-01; not the hang (the F1 hybrid hung with
//!       both zeroed), never proven read at load → a mismatch WARNS (named in `warned`), it does not fail.
//!   FT  the 0x06022013 FILETIME word = the map's TimeWriteMostRecentSolid over the PLACED embedded items (`filetime-check`'s
//!       rule): the game REJECTS the chunk in play when it differs (the coarse load-time bake, 11 of 48 Fall files). A map that
//!       embeds no solid has no rule here (n/a, not a failure).
//!   OB  the chart binds: every object index < the object count. The index space (docs/formats/map-lightmap.md §2): a map with
//!       authored (unbaked) blocks = [unbaked][baked][items]; a block-less build = [grid = size_x · size_z slots][items]; a
//!       Stadium-decoration map = 4 decoration objects + the map's objects at 16384 + index. An index past the end binds a chart
//!       to nothing the game instantiates.
//!   FR  frames: at least one image frame, frame 0 with its three images (colour, directional, probes) and a mapping chunk.
//!
//! Verdicts: `status` PASS | WARN | FAIL | NO-LIGHTMAP; the sidecar contract (`verdict` "pass" | "fail", `ok`, `failed[]`, `md5`) is what the
//! trust matrix reads (V8's loadsinplay) — a WARN is "pass" there. Output: one line per map; `--json OUT` writes the per-map objects
//! (one object for one map, an array for several — the corpus gate's `W/<tip>/<cell>/loadcheck.json`), `--tsv OUT` one row per map.
//! Exit 1 when any map FAILs.

use std::path::Path;

/// 0x06022018 on every Stadium editor bake (10/10 corpus oracles: stpad ×8, giant20x2 ×2): 2020-07-16T09:24:44Z as FILETIME ticks.
pub const STADIUM_W18: u64 = 132_393_650_849_529_910;

#[derive(Clone, Debug, serde::Serialize)]
pub struct LoadCheck {
    pub path: String,
    pub file: String,
    pub md5: String,
    pub lm_md5: Option<String>,
    /// PASS | WARN | FAIL | NO-LIGHTMAP | UNREADABLE (the human verdict; `verdict` below is the sidecar contract)
    pub status: String,
    /// "pass" | "fail" — the `loadcheck.json` contract the trust matrix reads (V8's `loadsinplay::read_loadcheck_json`: pass/ok = loads);
    /// a WARN (an unproven word off the editor's form) is "pass" here and names its rows in `warned`
    pub verdict: String,
    pub ok: bool,
    pub failed: Vec<String>,
    pub warned: Vec<String>,
    pub notes: Vec<String>,
    pub collection: String,
    pub frames: usize,
    pub frame0_images: usize,
    pub records: usize,
    pub kinds: Vec<u32>,
    pub kinds_wanted: Vec<u32>,
    pub w17: Option<(u32, u32)>,
    pub w18: Option<u64>,
    pub ft_word: Option<u64>,
    pub ft_placed_max: Option<u64>,
    pub ft_item: Option<String>,
    pub ft_verdict: String,
    pub n_unbaked: usize,
    pub n_baked: usize,
    pub n_items: usize,
    pub grid: i64,
    pub item_base: Option<u64>,
    pub objects: Option<u64>,
    pub max_obj: Option<u64>,
    pub binds: usize,
    pub binds_beyond: usize,
    pub binds_items: usize,
}

/// The kinds the frame records must carry for their count (the editor's rule).
pub fn kinds_wanted(n_records: usize) -> Vec<u32> {
    match n_records { 0 => vec![], 1 => vec![2], 2 => vec![2, 3], _ => vec![3, 3, 2] }
}

fn md5_file(bytes: &[u8]) -> String { crate::corpusgate::md5_hex(bytes) }

pub fn check(path: &str, word17_max: u32) -> LoadCheck {
    let file = path.rsplit('/').next().unwrap_or(path).to_string();
    let bytes = std::fs::read(path).unwrap_or_default();
    let md5 = md5_file(&bytes);
    let mut lc = LoadCheck {
        path: path.to_string(), file, md5, lm_md5: None, status: String::new(), verdict: String::new(), ok: false, failed: Vec::new(), warned: Vec::new(), notes: Vec::new(), collection: String::new(),
        frames: 0, frame0_images: 0, records: 0, kinds: Vec::new(), kinds_wanted: Vec::new(), w17: None, w18: None,
        ft_word: None, ft_placed_max: None, ft_item: None, ft_verdict: "n/a".into(),
        n_unbaked: 0, n_baked: 0, n_items: 0, grid: 0, item_base: None, objects: None, max_obj: None, binds: 0, binds_beyond: 0, binds_items: 0,
    };
    if bytes.is_empty() { lc.status = "UNREADABLE".into(); lc.verdict = "fail".into(); lc.failed.push("read".into()); return lc; }
    let m = match crate::mapio::load(path) { Ok(m) => m, Err(e) => { lc.status = "NO-LIGHTMAP".into(); lc.verdict = "fail".into(); lc.failed.push("FR".into()); lc.notes.push(e); return lc; } };
    let Some(d) = m.chunk.data.as_ref() else { lc.status = "NO-LIGHTMAP".into(); lc.verdict = "fail".into(); lc.failed.push("FR".into()); lc.notes.push("has_lightmaps = 0".into()); return lc; };
    lc.lm_md5 = crate::corpusgate::lightmap_chunk_md5(Path::new(path)).as_str().map(|s| s.to_string());
    // FR — frames and the mapping
    lc.frames = d.frames.len();
    lc.frame0_images = d.frames.first().map_or(0, |f| f.images.len());
    let Some(mp) = d.cache.mapping() else { lc.failed.push("FR".into()); lc.notes.push("no mapping chunk 0x0602201A".into()); lc.status = "FAIL".into(); lc.verdict = "fail".into(); return lc; };
    if lc.frames == 0 || lc.frame0_images < 3 { lc.failed.push("FR".into()); lc.notes.push(format!("{} image frames, frame 0 carries {} images", lc.frames, lc.frame0_images)); }
    // RK — the record kinds
    let h = &mp.head;
    let have = h.len().saturating_sub(72) / 66;
    if h.len() != 60 + 66 * have + 12 || have == 0 {
        lc.failed.push("RK".into());
        lc.notes.push(format!("mapping head {} B is not 60 + 66·k + 12", h.len()));
    } else {
        lc.records = have;
        lc.kinds = (0..have).map(|i| u32::from_le_bytes(h[60 + 66 * i..64 + 66 * i].try_into().unwrap())).collect();
        lc.kinds_wanted = kinds_wanted(have);
        if lc.kinds != lc.kinds_wanted { lc.failed.push("RK".into()); lc.notes.push(format!("record kinds {:?}, the editor's for {have} records {:?}", lc.kinds, lc.kinds_wanted)); }
        if have != lc.frames { lc.failed.push("RK".into()); lc.notes.push(format!("{have} records for {} image frames", lc.frames)); }
    }
    // CW — the small cache words
    for c in &d.cache.chunks {
        if let crate::format::ChunkBody::Raw(b) = &c.body {
            match c.id {
                0x0602_2017 if b.len() >= 8 => lc.w17 = Some((u32::from_le_bytes(b[0..4].try_into().unwrap()), u32::from_le_bytes(b[4..8].try_into().unwrap()))),
                0x0602_2018 if b.len() >= 8 => lc.w18 = Some(u64::from_le_bytes(b[0..8].try_into().unwrap())),
                _ => {}
            }
        }
    }
    lc.collection = tmmaps::header::read(path).ok().map(|h| h.envir).unwrap_or_default();
    // the editor's words over the 42 corpus oracles (baker-9's census, 2026-10-02 15:00Z): 0x17 = (0, n) on 42/42 with n = 0 on 35, else a
    // per-bake count (230 … 78 029 — no bound the game could check); 0x18 = 0 on every terrain-collection bake (32/32) and
    // 132393650849529910 (2020-07-16T09:24:44Z) on EVERY Stadium bake (10/10) — a collection solid time the port does not write yet.
    // Neither word is proven to be read at load (the F1 hybrid hung with them zeroed): a mismatch WARNS, it does not fail.
    let w18_editor: u64 = if lc.collection == "Stadium" { STADIUM_W18 } else { 0 };
    match lc.w17 {
        Some((0, b)) if word17_max == 0 || b <= word17_max => {}
        Some((a, b)) => { lc.warned.push("CW".into()); lc.notes.push(format!("0x06022017 = ({a}, {b}), the editor's (0, n{})", if word17_max > 0 { format!(" ≤ {word17_max}") } else { String::new() })); }
        None => { lc.warned.push("CW".into()); lc.notes.push("no cache chunk 0x06022017".into()); }
    }
    match lc.w18 {
        Some(v) if v == w18_editor => {}
        Some(v) => { lc.warned.push("CW".into()); lc.notes.push(format!("0x06022018 = {v}{}, the editor's {w18_editor} on {}", if v != 0 { format!(" (a FILETIME {})", ft_utc(v)) } else { String::new() }, if lc.collection.is_empty() { "this collection" } else { lc.collection.as_str() })); }
        None => { lc.warned.push("CW".into()); lc.notes.push("no cache chunk 0x06022018".into()); }
    }
    // FT — the FILETIME rule over the placed embedded items
    match crate::filetimecheck::read(path) {
        Ok(r) => {
            lc.ft_word = r.word;
            if let Some((t, item, _, _)) = &r.placed { lc.ft_placed_max = Some(*t); lc.ft_item = Some(item.clone()); }
            lc.ft_verdict = match (r.word, r.placed.as_ref().map(|p| p.0)) {
                (Some(w), Some(t)) if w == t => "EQUAL".into(),
                (Some(w), Some(t)) => { let d = (w as i128 - t as i128) as f64 / 1e7; lc.failed.push("FT".into()); lc.notes.push(format!("0x06022013 = {w} ({}) vs the placed solids' max {t} ({}, {}): OFF by {d:+.3} s", ft_utc(w), ft_utc(t), lc.ft_item.as_deref().unwrap_or("?"))); format!("OFF {d:+.3} s") }
                (None, _) => { lc.failed.push("FT".into()); lc.notes.push("no cache chunk 0x06022013".into()); "no word".into() }
                (Some(_), None) => "n/a (no embedded solid)".into(),
            };
        }
        Err(e) => { lc.ft_verdict = format!("unreadable: {e}"); lc.notes.push(format!("filetime: {e}")); }
    }
    // OB — the object index space
    let mf = tmmaps::map::MapFile::load(Path::new(path));
    lc.n_unbaked = mf.blocks.len();
    lc.n_baked = mf.baked.len();
    lc.n_items = mf.items.len();
    lc.grid = mf.size[0] as i64 * mf.size[2] as i64;
    lc.binds = mp.binds.len();
    let max_obj = mp.binds.iter().map(|b| (b.obj_group_idx / 4) as u64).max();
    lc.max_obj = max_obj;
    // the game's object list at load (docs/formats/map-lightmap.md §2 + the 42-oracle census): a Stadium map numbers its objects from
    // 16384 (4 decoration objects sit at 0..3); a map with a BAKED list = [unbaked][baked][items] (stpad: 180 + 10 244 + 21 →
    // 16384 + 10 445 = max 26828 + 1 exact); a block-less build = [grid slots (the regenerated ground, max(size_x·size_z, baked))][items]
    // (tiny16: 2 412 baked, items from 4096; g23: 254² = 64 516); a map with authored blocks and NO baked list gets its generated
    // list (tiles, clips) at load — unknowable here: n/a.
    let off: u64 = if lc.collection == "Stadium" { 16384 } else { 0 };
    let slots = (lc.grid.max(0) as u64).max(lc.n_baked as u64);
    let space: Option<(u64, u64, String)> = if lc.n_unbaked == 0 {
        Some((off + slots, off + slots + lc.n_items as u64, format!("{}{} slots + {} items", if off > 0 { "16384 + " } else { "" }, slots, lc.n_items)))
    } else if lc.n_baked > 0 {
        Some((off + (lc.n_unbaked + lc.n_baked) as u64, off + (lc.n_unbaked + lc.n_baked + lc.n_items) as u64, format!("{}{} unbaked + {} baked + {} items", if off > 0 { "16384 + " } else { "" }, lc.n_unbaked, lc.n_baked, lc.n_items)))
    } else { None };
    match space {
        Some((item_base, total, how)) => {
            lc.item_base = Some(item_base);
            lc.objects = Some(total);
            for b in &mp.binds {
                let o = (b.obj_group_idx / 4) as u64;
                if o >= total { lc.binds_beyond += 1; } else if o >= item_base { lc.binds_items += 1; }
            }
            if lc.binds_beyond > 0 { lc.failed.push("OB".into()); lc.notes.push(format!("{} binds past the {} objects ({}; max object {})", lc.binds_beyond, total, how, max_obj.unwrap_or(0))); }
        }
        None => lc.notes.push(format!("OB n/a: {} authored blocks and no baked list — the generated list is the game's at load (max object {})", lc.n_unbaked, max_obj.unwrap_or(0))),
    }
    if std::env::var("LMTOOL_LOADCHECK_OBJECTS").is_ok() {
        // the object index runs (contiguous ranges of bound object indices with their chart counts) — the index-space census
        let mut objs: Vec<u64> = mp.binds.iter().map(|b| (b.obj_group_idx / 4) as u64).collect();
        objs.sort_unstable();
        let mut runs: Vec<(u64, u64, usize)> = Vec::new();
        for o in objs { match runs.last_mut() { Some(r) if o == r.1 || o == r.1 + 1 => { r.1 = o; r.2 += 1; } _ => runs.push((o, o, 1)) } }
        eprintln!("{}: object runs [first..last × charts]: {}", lc.file, runs.iter().map(|(a, b, n)| format!("{a}..{b}×{n}")).collect::<Vec<_>>().join(" "));
    }
    lc.failed.sort();
    lc.failed.dedup();
    lc.warned.sort();
    lc.warned.dedup();
    lc.status = if !lc.failed.is_empty() { "FAIL".into() } else if !lc.warned.is_empty() { "WARN".into() } else { "PASS".into() };
    lc.ok = lc.failed.is_empty();
    lc.verdict = if lc.ok { "pass".into() } else { "fail".into() };
    lc
}

fn ft_utc(ft: u64) -> String {
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

pub fn line(lc: &LoadCheck) -> String {
    format!("{}: {} — {} fr, {} rec {:?}, 0x17 {}, 0x18 {}, FT {}, objects {} (base {}, {} binds: {} items, {} beyond, max {}){}",
        lc.file, lc.status, lc.frames, lc.records, lc.kinds,
        lc.w17.map_or("—".to_string(), |(a, b)| format!("({a}, {b})")), lc.w18.map_or("—".to_string(), |v| v.to_string()),
        lc.ft_verdict, lc.objects.map_or("?".to_string(), |v| v.to_string()), lc.item_base.map_or("?".to_string(), |v| v.to_string()),
        lc.binds, lc.binds_items, lc.binds_beyond, lc.max_obj.map_or("—".to_string(), |v| v.to_string()),
        if lc.failed.is_empty() && lc.warned.is_empty() { String::new() } else { format!(" ← {}{}{} [{}]", lc.failed.join(","), if !lc.failed.is_empty() && !lc.warned.is_empty() { " · warn " } else if !lc.warned.is_empty() { "warn " } else { "" }, lc.warned.join(","), lc.notes.join("; ")) })
}

pub fn tsv_header() -> &'static str {
    "file\tstatus\tfailed\twarned\tcollection\tmd5\tlm_md5\tframes\trecords\tkinds\tw17\tw18\tft\tobjects\titem_base\tbinds\tbinds_items\tbinds_beyond\tmax_obj\tnotes\n"
}

pub fn tsv_row(lc: &LoadCheck) -> String {
    format!("{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\n",
        lc.file, lc.status, lc.failed.join(","), lc.warned.join(","), lc.collection, lc.md5, lc.lm_md5.as_deref().unwrap_or("-"), lc.frames, lc.records,
        lc.kinds.iter().map(|k| k.to_string()).collect::<Vec<_>>().join(","),
        lc.w17.map_or("-".to_string(), |(a, b)| format!("{a},{b}")), lc.w18.map_or("-".to_string(), |v| v.to_string()),
        lc.ft_verdict, lc.objects.map_or("-".to_string(), |v| v.to_string()), lc.item_base.map_or("-".to_string(), |v| v.to_string()),
        lc.binds, lc.binds_items, lc.binds_beyond, lc.max_obj.map_or("-".to_string(), |v| v.to_string()), lc.notes.join("; ").replace('\t', " "))
}

pub fn run(args: &[String]) -> Result<(), String> {
    let f = |k: &str| args.iter().position(|x| x == k).and_then(|i| args.get(i + 1)).cloned();
    let word17_max: u32 = f("--word17-max").map(|v| v.parse().map_err(|e| format!("--word17-max: {e}"))).transpose()?.unwrap_or(0);
    let quiet = args.iter().any(|x| x == "--quiet");
    let mut paths: Vec<String> = Vec::new();
    let mut i = 1;
    while i < args.len() {
        if args[i] == "--quiet" { i += 1; continue; }
        if args[i].starts_with("--") { i += 2; continue; }
        paths.push(args[i].clone());
        i += 1;
    }
    if paths.is_empty() { return Err("loadcheck MAP [MAP…] [--json OUT] [--tsv OUT] [--word17-max N] [--quiet]".into()); }
    let checks: Vec<LoadCheck> = paths.iter().map(|p| check(p, word17_max)).collect();
    let (mut failed, mut warned) = (0, 0);
    for lc in &checks {
        if !quiet || lc.status != "PASS" { println!("{}", line(lc)); }
        if !lc.ok { failed += 1; } else if lc.status == "WARN" { warned += 1; }
    }
    if let Some(p) = f("--json") {
        let v = if checks.len() == 1 { serde_json::to_value(&checks[0]).unwrap() } else { serde_json::to_value(&checks).unwrap() };
        std::fs::write(&p, serde_json::to_string_pretty(&v).unwrap()).map_err(|e| format!("{p}: {e}"))?;
    }
    if let Some(p) = f("--tsv") {
        let mut s = tsv_header().to_string();
        for lc in &checks { s += &tsv_row(lc); }
        std::fs::write(&p, s).map_err(|e| format!("{p}: {e}"))?;
    }
    println!("loadcheck: {} of {} PASS{}{}", checks.len() - failed - warned, checks.len(), if warned > 0 { format!(", {warned} WARN") } else { String::new() }, if failed > 0 { format!(", {failed} FAIL") } else { String::new() });
    if failed > 0 { std::process::exit(1); }
    Ok(())
}

#[cfg(test)]
mod tests {
    #[test]
    fn the_editor_kinds_by_record_count() {
        assert_eq!(super::kinds_wanted(2), vec![2, 3]);
        assert_eq!(super::kinds_wanted(3), vec![3, 3, 2]);
        assert_eq!(super::kinds_wanted(1), vec![2]);
    }

    #[test]
    fn filetime_prints_as_utc() {
        // 2024-07-29T20:37:18Z (the Nadeo template's 0x18 word the port shipped): 133667446380000000 ticks
        let s = super::ft_utc(133_667_446_380_000_000);
        assert!(s.starts_with("2024-07-"), "{s}");
        assert_eq!(super::ft_utc(116_444_736_000_000_000), "1970-01-01T00:00:00Z");
    }
}
