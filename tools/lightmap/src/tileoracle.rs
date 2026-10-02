//! `lmtool tile-oracle SRC.Map.Gbx [--mapping placements.tsv] [--out ORACLE.tsv] [--all]`
//!
//! The game's own verdict on every terrain tile of a Nadeo map — DRAWN or NOT —
//! read off the map's editor lightmap (2026-10-02, Fall 06's "missing block on
//! the floor next to the reactor").
//!
//! The lightmapper gives every object it draws a chart (`docs/formats/
//! lightmapper-client.md` §4: `AutoSetIdsForLightMap` numbers the authored
//! blocks, then the baked records, then the items; `IdsForLightMap_BindToScene`
//! binds each chart to its object's id). A terrain tile the game does not draw
//! — the Dirt under a ground road, the Water under a pillar's plate — gets an
//! id but NO chart; the tile in the next cell, drawn, gets one. So for a map
//! that ships Nadeo's own bake the chart table is a per-record answer to the
//! question the tiny converter has been inferring from block infos
//! (`tmmaps::tiny::tiles::hidden_tiles`): `U<idx>`/`B<idx>` → hidden/drawn.
//!
//! Fall 06, RedIsland: every tile the unit rule hides has 0 charts and its open
//! neighbours have 1 — except the DirtCliff4 at (45,19,29) under a GHOST-mode
//! DecoWallDiag1, which the game charts (draws) and the rule hid: vjeux's hole.
//!
//! The alignment (block idx → object id) is checked on the map itself: the
//! OPEN tiles — a tile alone in its cell — must carry charts; a wrong P (the
//! Stadium decoration's 16384 offset) or a skipped block (a `lightmappable`
//! filter hit) scatters chartless ids over them. `exact` is false when fewer
//! than 98 % of the open tiles are charted; the converter then keeps its rule.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use tmmaps::map::{BlockRec, MapFile};

pub struct Oracle {
    /// the first authored block's object id (0; 16384 under a Stadium decoration that ships a map)
    pub p: usize,
    pub n_authored: usize,
    pub n_baked: usize,
    pub n_items: usize,
    /// objects with at least one chart, by object id
    charted: Vec<bool>,
    /// charts bound to ids past the expected object space (a misalignment sign)
    pub beyond: usize,
    /// the share of OPEN tiles (alone in their cell) that carry a chart, authored + baked
    pub open_charted: f64,
    pub open_total: usize,
    pub exact: bool,
}

impl Oracle {
    fn has_chart(&self, obj: usize) -> bool {
        self.charted.get(obj).copied().unwrap_or(false)
    }
    /// Is this authored block drawn (charted) by the game?
    pub fn authored_drawn(&self, idx: usize) -> bool {
        self.has_chart(self.p + idx)
    }
    /// Is this baked record drawn (charted) by the game?
    pub fn baked_drawn(&self, bidx: usize) -> bool {
        self.has_chart(self.p + self.n_authored + bidx)
    }
}

/// The oracle for a map: `None` when the file carries no lightmap.
pub fn read(path: &Path) -> Result<Option<(MapFile, Oracle)>, String> {
    let map = MapFile::load(path);
    let lm = match crate::mapio::load(&path.display().to_string()) {
        Ok(l) => l,
        Err(e) if e.contains("no lightmap chunk") => return Ok(None),
        Err(e) => return Err(e),
    };
    let Some(d) = lm.chunk.data.as_ref() else { return Ok(None) };
    let Some(mp) = d.cache.mapping() else { return Ok(None) };
    let (n_authored, n_baked, n_items) = (map.blocks.len(), map.baked.len(), map.items.len());
    let zones: BTreeSet<String> = map.genealogy_zones().into_iter().collect();
    // the tiles alone in their cell (no other authored or baked record there), per list
    let mut per_cell: BTreeMap<[u8; 3], usize> = BTreeMap::new();
    for b in map.blocks.iter().chain(map.baked.iter()).filter(|b| b.free_pos.is_none()) {
        *per_cell.entry(b.file_cell).or_default() += 1;
    }
    let is_open_tile = |b: &BlockRec| zones.contains(&b.name) && b.free_pos.is_none() && per_cell.get(&b.file_cell).copied().unwrap_or(0) == 1;
    let maxo = mp.binds.iter().map(|b| (b.obj_group_idx / 4) as usize).max().unwrap_or(0);
    let mut best: Option<Oracle> = None;
    for p in [0usize, 16384] {
        let space = p + n_authored + n_baked + n_items;
        let mut charted = vec![false; space.max(maxo + 1)];
        let mut beyond = 0usize;
        for b in &mp.binds {
            let o = (b.obj_group_idx / 4) as usize;
            if o >= space {
                beyond += 1;
            }
            charted[o] = true;
        }
        let mut open_total = 0usize;
        let mut open_ok = 0usize;
        for b in map.blocks.iter().filter(|b| is_open_tile(b)) {
            open_total += 1;
            if charted.get(p + b.index).copied().unwrap_or(false) {
                open_ok += 1;
            }
        }
        for (bi, b) in map.baked.iter().enumerate().filter(|(_, b)| is_open_tile(b)) {
            open_total += 1;
            if charted.get(p + n_authored + bi).copied().unwrap_or(false) {
                open_ok += 1;
            }
        }
        let open_charted = if open_total == 0 { 0.0 } else { open_ok as f64 / open_total as f64 };
        let exact = open_total > 0 && open_charted >= 0.98 && beyond == 0;
        let cand = Oracle { p, n_authored, n_baked, n_items, charted, beyond, open_charted, open_total, exact };
        if best.as_ref().map(|b| cand.open_charted > b.open_charted || (cand.beyond == 0 && b.beyond > 0 && cand.open_charted >= b.open_charted)).unwrap_or(true) {
            best = Some(cand);
        }
    }
    Ok(best.map(|o| (map, o)))
}

/// `--out`: one row per tile record — `U|B <idx> <name> <x,y,z> hidden|drawn` — the
/// file `tmmaps tiny --tiles-oracle` reads. Only authored and baked ZONE tiles.
pub fn write_oracle(map: &MapFile, o: &Oracle, out: &Path) -> Result<(usize, usize), String> {
    let zones: BTreeSet<String> = map.genealogy_zones().into_iter().collect();
    let mut s = String::from("# lmtool tile-oracle: the game's lightmap charts per terrain tile record (hidden = no chart = not drawn)\nlist\tindex\tname\tcell\tverdict\n");
    let (mut hidden, mut drawn) = (0usize, 0usize);
    for b in map.blocks.iter().filter(|b| zones.contains(&b.name)) {
        let v = if o.authored_drawn(b.index) { drawn += 1; "drawn" } else { hidden += 1; "hidden" };
        s.push_str(&format!("U\t{}\t{}\t{},{},{}\t{v}\n", b.index, b.name, b.file_cell[0], b.file_cell[1], b.file_cell[2]));
    }
    for (bi, b) in map.baked.iter().enumerate().filter(|(_, b)| zones.contains(&b.name)) {
        let v = if o.baked_drawn(bi) { drawn += 1; "drawn" } else { hidden += 1; "hidden" };
        s.push_str(&format!("B\t{bi}\t{}\t{},{},{}\t{v}\n", b.name, b.file_cell[0], b.file_cell[1], b.file_cell[2]));
    }
    std::fs::write(out, s).map_err(|e| format!("{}: {e}", out.display()))?;
    Ok((hidden, drawn))
}

pub fn cmd(a: &[String]) {
    let f = |k: &str| tmmaps::cli::flag(a, k).map(String::from);
    let Some(src) = a.get(1).filter(|s| !s.starts_with("--")) else {
        eprintln!("lmtool tile-oracle SRC.Map.Gbx [--mapping placements.tsv] [--out ORACLE.tsv] [--all]");
        std::process::exit(2);
    };
    let path = Path::new(src);
    let (map, o) = match read(path) {
        Ok(Some(x)) => x,
        Ok(None) => {
            println!("{src}: no baked lightmap — no oracle (the converter keeps its unit rule)");
            std::process::exit(3);
        }
        Err(e) => {
            eprintln!("{e}");
            std::process::exit(1);
        }
    };
    println!(
        "{src}: {} authored blocks, {} baked records, {} items; object ids from P = {}; open tiles charted {}/{} ({:.1} %); {} charts beyond the object space → alignment {}",
        o.n_authored,
        o.n_baked,
        o.n_items,
        o.p,
        (o.open_charted * o.open_total as f64).round() as usize,
        o.open_total,
        o.open_charted * 100.0,
        o.beyond,
        if o.exact { "EXACT" } else { "INEXACT (oracle not usable)" }
    );
    let zones: BTreeSet<String> = map.genealogy_zones().into_iter().collect();
    // our rule, when a mapping is given
    let mapping = f("--mapping").map(|p| tmmaps::tiny::mapping::read_mapping(Path::new(&p)));
    let hidden = mapping.as_ref().map(|m| {
        let info_of = |b: &BlockRec| -> Option<(String, Vec<[i32; 3]>, Option<(Vec<([i32; 3], String)>, i32)>)> {
            m.by_index.get(&b.index).or_else(|| m.by_name.get(&b.name)).map(|x| (x.model.clone(), x.units.clone(), x.auto_terrain.clone()))
        };
        tmmaps::tiny::tiles::hidden_tiles(&map, &zones, &info_of)
    });
    let all = tmmaps::cli::has(a, "--all");
    // the other records in a cell, for the listing
    let mut occupants: BTreeMap<[u8; 3], Vec<String>> = BTreeMap::new();
    for b in map.blocks.iter().filter(|b| !zones.contains(&b.name) && b.free_pos.is_none()) {
        occupants.entry(b.file_cell).or_default().push(format!("{} ({:08X}{})", b.name, b.flags, if b.flags & tmmaps::fillers::FLAG_GHOST != 0 { " ghost" } else { "" }));
    }
    // the blocks whose UNITS cover the cell (needs the mapping), as `name:ground|air:ghost|solid:geom|-`
    let covering = mapping.as_ref().map(|m| {
        let units_of = |b: &BlockRec| -> Vec<[i32; 3]> { m.by_index.get(&b.index).or_else(|| m.by_name.get(&b.name)).map(|x| x.units.clone()).unwrap_or_default() };
        tmmaps::tiny::tiles::covering_blocks(&map, &zones, &units_of)
    });
    let describe = |idx: usize| -> String {
        let b = &map.blocks[idx];
        let model = mapping.as_ref().and_then(|m| m.by_index.get(&idx).or_else(|| m.by_name.get(&b.name))).map(|x| if x.model == "-" { "-" } else { "geom" }).unwrap_or("?");
        format!("{}:{}:{}:{}", b.name, if b.flags & (1 << 12) != 0 { "ground" } else { "air" }, if b.flags & tmmaps::fillers::FLAG_GHOST != 0 { "ghost" } else { "solid" }, model)
    };
    let (mut game_hidden, mut game_drawn) = (0usize, 0usize);
    let (mut agree_h, mut agree_d, mut hole, mut coplanar) = (0usize, 0usize, 0usize, 0usize);
    println!("list\tindex\tname\tcell\tgame\trule\tverdict\tother blocks in the cell\tblocks whose units cover the cell (name:ground|air:ghost|solid:geom|-)");
    let mut row = |list: &str, idx: usize, b: &BlockRec, drawn: bool| {
        if drawn { game_drawn += 1 } else { game_hidden += 1 }
        let rule = hidden.as_ref().map(|h| h.hides(b));
        let verdict = match (drawn, rule) {
            (false, Some(true)) => { agree_h += 1; "agree hidden" }
            (true, Some(false)) => { agree_d += 1; "agree drawn" }
            (true, Some(true)) => { hole += 1; "HOLE: the rule hides a tile the game draws" }
            (false, Some(false)) => { coplanar += 1; "COPLANAR: the rule keeps a tile the game hides" }
            (false, None) => "game hidden",
            (true, None) => "game drawn",
        };
        let interesting = all || verdict.starts_with("HOLE") || verdict.starts_with("COPLANAR") || (rule.is_none() && !drawn);
        if interesting {
            let occ = occupants.get(&b.file_cell).map(|v| v.join(", ")).unwrap_or_else(|| "-".into());
            let cov = covering.as_ref().and_then(|c| c.get(&b.file_cell)).map(|v| v.iter().map(|i| describe(*i)).collect::<Vec<_>>().join(", ")).unwrap_or_else(|| "-".into());
            println!("{list}\t{idx}\t{}\t{},{},{}\t{}\t{}\t{verdict}\t{occ}\t{cov}", b.name, b.file_cell[0], b.file_cell[1], b.file_cell[2], if drawn { "drawn" } else { "hidden" }, rule.map(|r| if r { "hidden" } else { "kept" }).unwrap_or("-"));
        }
    };
    for b in map.blocks.iter().filter(|b| zones.contains(&b.name)) {
        row("U", b.index, b, o.authored_drawn(b.index));
    }
    for (bi, b) in map.baked.iter().enumerate().filter(|(_, b)| zones.contains(&b.name)) {
        row("B", bi, b, o.baked_drawn(bi));
    }
    println!("tiles: game hides {game_hidden}, draws {game_drawn}{}", if hidden.is_some() { format!("; vs the unit rule: {agree_h} agree hidden, {agree_d} agree drawn, {hole} HOLE (rule hides, game draws), {coplanar} COPLANAR (rule keeps, game hides)") } else { String::new() });
    if let Some(out) = f("--out") {
        match write_oracle(&map, &o, Path::new(&out)) {
            Ok((h, d)) => println!("wrote {out}: {h} hidden + {d} drawn tile verdicts{}", if o.exact { "" } else { " (INEXACT alignment — the converter should not use it)" }),
            Err(e) => {
                eprintln!("{e}");
                std::process::exit(1);
            }
        }
    }
    if !o.exact {
        std::process::exit(4);
    }
}
