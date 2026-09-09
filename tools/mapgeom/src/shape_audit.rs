//! `mapgeom shape-audit MAP… [--collection Stadium] [--game FILE.json] [--out TSV] [--all]`
//!
//! The SHAPE side of the generated fillers (2026-09-08, vjeux on Summer 15's
//! reactor gate: "we display entire basic road blocks when it should be a wall
//! or something"): for every BAKED record of a map, what the converter would
//! bake for it — the block info its NAME resolves to (and whether another file
//! of the same stem competes), the variant the flags pick, the mobil list and
//! mobil the flags index, the prefab that comes out — and every place the pick
//! FELL BACK (a variant index past the variant's mobil lists, a mobil index past
//! the list, a ground record on an info without a ground variant): a fallback
//! draws a piece the record did not name. `--game FILE` is the editor's own
//! `/mapblocks2?list=baked` JSON of the same map: the engine's `MobilIndex` /
//! `MobilVariantIndex` per record, set against the flag bits the converter
//! decodes (bits 0..5 / 6..11, `blockmap::FLAG_*`).
//!
//! One row per distinct (name, flags) key, `--all` for one per record. The
//! summary at the end counts records, keys, and every suspect class per map.

use std::collections::BTreeMap;
use std::path::Path;

use crate::blockmap::{BlockInfoIndex, FLAG_ADDITIONAL_SHIFT, FLAG_FREE, FLAG_GHOST, FLAG_GROUND, FLAG_SUBVARIANT_SHIFT, FLAG_VARIANT_MASK};
use crate::store::DataStore;
use tmmaps::fillers::GameList;
use tmmaps::map::MapFile;

/// What the converter would draw for one (name, flags) key.
#[derive(Clone, Debug, Default)]
pub struct Resolution {
    pub path: String,
    pub kind: String,
    /// every block-info file the name could mean, best first
    pub candidates: Vec<String>,
    pub label: String,
    /// mobil lists in the picked variant
    pub lists: usize,
    /// mobils in the picked list (0 = an empty list: the record draws nothing)
    pub list_len: usize,
    pub prefabs: Vec<String>,
    pub notes: Vec<String>,
    pub error: Option<String>,
}

impl Resolution {
    /// The classes a row can be suspect for (empty = clean).
    pub fn suspects(&self) -> Vec<&'static str> {
        let mut out = Vec::new();
        if self.error.is_some() {
            out.push("no-blockinfo");
        }
        let kinds: std::collections::BTreeSet<&str> = self.candidates.iter().map(|p| kind_of_path(p)).collect();
        if kinds.len() > 1 {
            out.push("ambiguous-name");
        }
        if self.label.contains("fallback") {
            out.push("variant-fallback");
        }
        if self.notes.iter().any(|n| n.contains("past")) {
            out.push("index-past-list");
        }
        if self.prefabs.iter().any(|p| p.starts_with("(solid)")) {
            out.push("legacy-solid");
        }
        out
    }
}

pub fn kind_of_path(p: &str) -> &'static str {
    let u = p.to_ascii_uppercase();
    if u.contains("GAMECTNBLOCKINFOCLASSIC\\") {
        "Classic"
    } else if u.contains("GAMECTNBLOCKINFOPILLAR\\") {
        "Pillar"
    } else if u.contains("GAMECTNBLOCKINFOCLIP\\") {
        "Clip"
    } else if u.contains("GAMECTNBLOCKINFOFLAT\\") {
        "Flat"
    } else if u.contains("GAMECTNBLOCKINFOFRONTIER\\") {
        "Frontier"
    } else if u.contains("GAMECTNBLOCKINFOTRANSITION\\") {
        "Transition"
    } else {
        "Other"
    }
}

/// Decode the flag word the way `tiny_library::plan_block` does.
pub fn decode(flags: u32) -> (bool, usize, usize, usize) {
    let ground = flags & FLAG_GROUND != 0;
    let vindex = (flags & FLAG_VARIANT_MASK) as usize;
    let sub = ((flags >> FLAG_SUBVARIANT_SHIFT) & 63) as usize;
    let addv = ((flags >> FLAG_ADDITIONAL_SHIFT) & 0x7F) as usize;
    (ground, vindex, sub, addv)
}

/// Resolve one key exactly as the library build does (`load_block_info` +
/// `pick_placement_add`), keeping the notes the report drops.
pub fn resolve(store: &mut DataStore, idx: &mut BlockInfoIndex, name: &str, flags: u32) -> Resolution {
    let mut r = Resolution { candidates: idx.paths_for(name), ..Default::default() };
    let Some(path) = r.candidates.first().cloned() else {
        r.error = Some("no block info file with this name".into());
        return r;
    };
    r.path = path.clone();
    r.kind = kind_of_path(&path).to_string();
    let bi = match idx.load(store, &path) {
        Ok(b) => b.clone(),
        Err(e) => {
            r.error = Some(format!("block info: {e}"));
            return r;
        }
    };
    let (ground, vindex, sub, addv) = decode(flags);
    let Some(pk) = bi.pick_placement_add(ground, vindex, sub, addv) else {
        r.error = Some("block info has no variant with units or mobils".into());
        return r;
    };
    r.label = pk.label.clone();
    r.lists = pk.variant.mobils.len();
    r.list_len = pk.variant.mobils.get(pk.list).map(|l| l.len()).unwrap_or(0);
    r.prefabs = pk.prefabs().iter().map(|p| p.rsplit('\\').next().unwrap_or(p).trim_end_matches(".Prefab.Gbx").to_string()).collect();
    r.notes = pk.notes.iter().filter(|n| !n.contains("additional") || n.contains("past")).cloned().collect();
    r
}

pub fn cmd(store: &mut DataStore, args: &[String]) {
    let maps: Vec<String> = args.iter().take_while(|a| !a.starts_with("--")).cloned().collect();
    if maps.is_empty() {
        eprintln!("shape-audit needs MAP.Map.Gbx…");
        std::process::exit(2);
    }
    let flag = |k: &str| args.iter().position(|a| a == k).and_then(|i| args.get(i + 1)).cloned();
    let has = |k: &str| args.iter().any(|a| a == k);
    let all = has("--all");
    let game: Option<GameList> = flag("--game").map(|p| GameList::load(Path::new(&p)));
    let mut out = String::from("map\tname\tflags\tghost\tground\tlist\tmobil\tadd\tn\tkind\tlabel\tlists\tlist_len\tprefab\tnotes\tsuspect\tcandidates\tgame_mobil\tgame_mobilvar\tgame_match\n");
    let mut totals: Vec<String> = Vec::new();
    for mp in &maps {
        let m = MapFile::load(Path::new(mp));
        let collection = m.items.first().map(|it| it.collection_raw).unwrap_or(26);
        let coll_name = flag("--collection").unwrap_or_else(|| crate::static_item::build::env_name(collection).to_string());
        let mut idx = BlockInfoIndex::build(store, &coll_name);
        let short = Path::new(mp).file_name().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
        // distinct keys, in file order of first sighting
        let mut keys: BTreeMap<(String, u32), Vec<usize>> = BTreeMap::new();
        for (i, b) in m.baked.iter().enumerate() {
            if b.name == "Sea" || b.flags & FLAG_FREE != 0 {
                continue;
            }
            keys.entry((b.name.clone(), b.flags)).or_default().push(i);
        }
        let mut cache: BTreeMap<(String, u32), Resolution> = BTreeMap::new();
        let (mut n_rec, mut n_keys, mut n_bad_keys, mut n_bad_rec) = (0usize, 0usize, 0usize, 0usize);
        let mut by_class: BTreeMap<&'static str, usize> = BTreeMap::new();
        let (mut g_matched, mut g_mismatch, mut g_absent) = (0usize, 0usize, 0usize);
        let mut mismatches: Vec<String> = Vec::new();
        for ((name, flags), recs) in &keys {
            n_keys += 1;
            n_rec += recs.len();
            let r = cache.entry((name.clone(), *flags)).or_insert_with(|| resolve(store, &mut idx, name, *flags)).clone();
            let sus = r.suspects();
            if !sus.is_empty() {
                n_bad_keys += 1;
                n_bad_rec += recs.len();
                for s in &sus {
                    *by_class.entry(s).or_insert(0) += recs.len();
                }
            }
            let (ground, vindex, sub, addv) = decode(*flags);
            let ghost = flags & FLAG_GHOST != 0;
            // the game's pick for each record of the key
            let mut game_cols: Vec<(String, String, String)> = Vec::new();
            if let Some(g) = &game {
                for &i in recs {
                    let b = &m.baked[i];
                    let key = (b.name.clone(), b.coords(), b.dir & 3);
                    match g.recs.get(&key).and_then(|v| v.first()) {
                        Some(gr) => {
                            let ok = gr.mobil as usize == vindex && gr.mobil_var as usize == sub && gr.ground == ground;
                            if ok {
                                g_matched += 1;
                            } else {
                                g_mismatch += 1;
                                mismatches.push(format!("b{} {} {:08X} cell {:?} side {}: file list {} mobil {} ground {} / game mobil {} mobilVar {} ground {}", b.index, b.name, b.flags, b.coords(), b.dir & 3, vindex, sub, ground, gr.mobil, gr.mobil_var, gr.ground));
                            }
                            game_cols.push((gr.mobil.to_string(), gr.mobil_var.to_string(), if ok { "Y".into() } else { "MISMATCH".into() }));
                        }
                        None => {
                            g_absent += 1;
                            game_cols.push(("-".into(), "-".into(), "absent".into()));
                        }
                    }
                }
            }
            let row = |n: usize, gc: Option<&(String, String, String)>| -> String {
                let (gm, gv, gk) = gc.map(|c| (c.0.as_str(), c.1.as_str(), c.2.as_str())).unwrap_or(("", "", ""));
                format!(
                    "{}\t{}\t{:08X}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\n",
                    short,
                    name,
                    flags,
                    if ghost { "ghost" } else { "" },
                    if ground { "G" } else { "" },
                    vindex,
                    sub,
                    addv,
                    n,
                    if r.kind.is_empty() { "-" } else { &r.kind },
                    if r.label.is_empty() { "-" } else { &r.label },
                    r.lists,
                    r.list_len,
                    if r.prefabs.is_empty() { "-".to_string() } else { r.prefabs.join("+") },
                    if let Some(e) = &r.error { e.clone() } else { r.notes.join(" | ") },
                    sus.join(","),
                    r.candidates.iter().map(|p| format!("{}:{}", kind_of_path(p), p.rsplit('\\').next().unwrap_or(p))).collect::<Vec<_>>().join(" "),
                    gm,
                    gv,
                    gk
                )
            };
            if all {
                for (k, _) in recs.iter().enumerate() {
                    out.push_str(&row(1, game_cols.get(k)));
                }
            } else {
                // one row per key; the game column summarises the key's records
                let gsum = if game.is_some() {
                    let mism = game_cols.iter().filter(|c| c.2 == "MISMATCH").count();
                    let abs = game_cols.iter().filter(|c| c.2 == "absent").count();
                    let gm = game_cols.iter().find(|c| c.2 == "Y").map(|c| c.0.clone()).unwrap_or_else(|| "-".into());
                    let gv = game_cols.iter().find(|c| c.2 == "Y").map(|c| c.1.clone()).unwrap_or_else(|| "-".into());
                    Some((gm, gv, if mism > 0 { format!("{mism} MISMATCH") } else if abs == game_cols.len() { "absent".to_string() } else if abs > 0 { format!("Y ({abs} absent)") } else { "Y".to_string() }))
                } else {
                    None
                };
                out.push_str(&row(recs.len(), gsum.as_ref()));
            }
        }
        let mut line = format!("{short}: {n_rec} baked records in {n_keys} (name, flags) keys; {n_bad_rec} records / {n_bad_keys} keys suspect");
        if !by_class.is_empty() {
            line.push_str(&format!(" ({})", by_class.iter().map(|(k, v)| format!("{k} {v}")).collect::<Vec<_>>().join(", ")));
        }
        if let Some(g) = &game {
            line.push_str(&format!("; game list {} records: {} match the file's bits, {} mismatch, {} absent", g.total, g_matched, g_mismatch, g_absent));
            for mm in mismatches.iter().take(40) {
                line.push_str(&format!("\n    {mm}"));
            }
            if mismatches.len() > 40 {
                line.push_str(&format!("\n    … {} more", mismatches.len() - 40));
            }
        }
        eprintln!("{line}");
        totals.push(line);
    }
    match flag("--out") {
        Some(p) => {
            std::fs::write(&p, &out).unwrap_or_else(|e| panic!("{p}: {e}"));
            eprintln!("wrote {p} ({} rows)", out.lines().count() - 1);
        }
        None => print!("{out}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decode_reads_the_bits_plan_block_reads() {
        // WaterWallHFC 0x0F: list 15 (StrStr), mobil 0, no additional, air
        assert_eq!(decode(0x0000000F), (false, 15, 0, 0));
        // DecoPlatformFCSmall 0x40: list 0, mobil 1 (the v2 alternate)
        assert_eq!(decode(0x00000040), (false, 0, 1, 0));
        // DecoWallBaseVFC 0x1000: ground variant, list 0
        assert_eq!(decode(0x00001000), (true, 0, 0, 0));
        // WaterBase 0x200000: additional variant 1 (add0), ghost bit ignored
        assert_eq!(decode(0x10200000), (false, 0, 0, 1));
    }

    #[test]
    fn suspect_classes() {
        let clean = Resolution { path: "Stadium\\GameCtnBlockInfo\\GameCtnBlockInfoClip\\X.EDClip.Gbx".into(), kind: "Clip".into(), candidates: vec!["Stadium\\GameCtnBlockInfo\\GameCtnBlockInfoClip\\X.EDClip.Gbx".into()], label: "air/base".into(), lists: 1, list_len: 1, prefabs: vec!["X_Air".into()], notes: vec![], error: None };
        assert!(clean.suspects().is_empty());
        let mut two_kinds = clean.clone();
        two_kinds.candidates.push("Stadium\\GameCtnBlockInfo\\GameCtnBlockInfoClassic\\X.EDClassic.Gbx".into());
        assert_eq!(two_kinds.suspects(), vec!["ambiguous-name"]);
        let mut fell_back = clean.clone();
        fell_back.label = "ground/base(fallback)".into();
        assert_eq!(fell_back.suspects(), vec!["variant-fallback"]);
        let mut past = clean.clone();
        past.notes.push("variant 7 past 4 mobil lists, list 0 used".into());
        assert_eq!(past.suspects(), vec!["index-past-list"]);
        let mut missing = clean.clone();
        missing.error = Some("no block info file with this name".into());
        assert_eq!(missing.suspects(), vec!["no-blockinfo"]);
    }
}
