//! `tmmaps fillers MAP` — the generated (baked) clip fillers of a map, each
//! with what its own cell holds and what stands across the side it hangs on.
//!
//! The game builds a pillar's walls, a platform's skirts and a road's end caps
//! at edit time from the block infos' CLIP references and records the result
//! as BAKED blocks (chunk 0x03043048): one record per clip piece, in the cell
//! the piece is drawn in, its `dir` naming the cell side the piece stands on
//! and its flags' variant index (bits 0..5) naming the mobil list — for a
//! `DecoWallBaseVFC` 0 Middle, 1 Top, 2 Bottom, 3 TopBottom, 4 NOTHING (the
//! cell is covered by a merged piece below), 5..10 Middle×2/3/4/8/16/32; the
//! ground bit (0x1000) picks the ground variant (0 Bottom_Ground, 1
//! TopBottom_Ground); bit 28 is set on the fillers of ghost-mode blocks.
//!
//! A vertical clip in cell C with dir d is the wall on C's side d — North (+z),
//! East (−x), South (−z), West (+x) — and the block it belongs to stands in the
//! cell across that side. This listing is the ground truth the tiny
//! converter's filler rules are checked against (Summer 20 cp3, 2026-09-07:
//! eleven `DecoWall*VFC*` in the DecoPlatform cells drew a bar the game does
//! not show; Summer 10, 2026-09-08: dropping every `DecoWall*VFC*` for it
//! removed all the pillar towers).
//!
//!   tmmaps fillers MAP [--filter PAT] [--cells X0,Z0:X1,Z1] [--summary]
//!
//! Rows: id, name, flags, variant, ground, ghost, cell, side, own cell's
//! authored blocks (`-` free, `P` pillars only), the blocks across the side.

use crate::map::{BlockRec, MapFile, FREE_BLOCK_FLAG};
use std::collections::BTreeMap;
use std::path::Path;

pub const FLAG_GROUND: u32 = 1 << 12;
pub const FLAG_PILLAR: u32 = 1 << 14;
pub const FLAG_GHOST: u32 = 1 << 28;
pub const SIDE_NAMES: [&str; 4] = ["N", "E", "S", "W"];
/// Local side vectors in (x, z): North +z, East −x, South −z, West +x
/// (mapgeom's `blockmap::SIDE_VEC`; RoadTechCurve1 hangs its clips on North
/// and East, and a dir-1 `DecoWallBaseVFC` faces the pillar at x − 1).
pub const SIDE_VEC: [(i32, i32); 4] = [(0, 1), (-1, 0), (0, -1), (1, 0)];

/// The authored (unbaked, grid) blocks of every cell.
pub fn occupants(m: &MapFile) -> BTreeMap<[u8; 3], Vec<&BlockRec>> {
    let mut occ: BTreeMap<[u8; 3], Vec<&BlockRec>> = BTreeMap::new();
    for b in m.blocks.iter().filter(|b| b.flags & FREE_BLOCK_FLAG == 0) {
        occ.entry(b.file_cell).or_default().push(b);
    }
    occ
}

/// `-` for a free cell, `P` when only pillars stand there, else the names.
pub fn describe(cell: Option<&Vec<&BlockRec>>) -> String {
    match cell {
        None => "-".to_string(),
        Some(v) if v.iter().all(|b| b.flags & FLAG_PILLAR != 0) => format!("P:{}", v.iter().map(|b| b.name.as_str()).collect::<Vec<_>>().join("+")),
        Some(v) => v.iter().filter(|b| b.flags & FLAG_PILLAR == 0).map(|b| format!("{}[{:X}]", b.name, b.flags)).collect::<Vec<_>>().join("+"),
    }
}

/// The cell across side `dir` of `cell`, in raw file coordinates (None off the grid).
pub fn across(cell: [u8; 3], dir: u8) -> Option<[u8; 3]> {
    let (dx, dz) = SIDE_VEC[(dir & 3) as usize];
    let x = cell[0] as i32 + dx;
    let z = cell[2] as i32 + dz;
    if !(0..=255).contains(&x) || !(0..=255).contains(&z) {
        return None;
    }
    Some([x as u8, cell[1], z as u8])
}

pub fn cmd(args: &[String]) {
    let m = MapFile::load(Path::new(&args[2]));
    let pat = crate::cli::flag(args, "--filter").map(|s| s.to_string());
    let summary = crate::cli::has(args, "--summary");
    // --game FILE: the editor's own baked list (GhostShooter `/mapblocks2?list=baked`
    // JSON) — every file record is looked up there by (name, cell, side) and the
    // listing gains a `game` column (Y kept / N absent); the summary counts them,
    // and the game records no file record explains are listed at the end.
    let game: Option<GameList> = crate::cli::flag(args, "--game").map(|p| GameList::load(Path::new(p)));
    let mut game_left: BTreeMap<(String, (i32, i32, i32), u8), usize> = game.as_ref().map(|g| g.counts.clone()).unwrap_or_default();
    // --cells X0,Z0:X1,Z1 in the census's (gbx-py) cell numbers
    let cells = crate::cli::flag(args, "--cells").map(|s| {
        let (a, b) = s.split_once(':').expect("--cells X0,Z0:X1,Z1");
        let p = |t: &str| -> (i32, i32) {
            let v: Vec<i32> = t.split(',').map(|x| x.trim().parse().expect("a cell number")).collect();
            (v[0], v[1])
        };
        (p(a), p(b))
    });
    let occ = occupants(&m);
    let mut rows = 0usize;
    // (name, variant word) -> [free, pillar-only, occupied] counts, ghost count, [game kept, game absent]
    let mut tally: BTreeMap<(String, String), ([usize; 3], usize, [usize; 2])> = BTreeMap::new();
    if !summary {
        println!("id\tname\tflags\tvariant\tground\tghost\tcx\tcy\tcz\tside\town_cell\tacross{}", if game.is_some() { "\tgame" } else { "" });
    }
    for b in m.baked.iter().filter(|b| b.name != "Sea" && b.flags & FREE_BLOCK_FLAG == 0) {
        if let Some(p) = &pat {
            if !b.name.contains(p.as_str()) {
                continue;
            }
        }
        let c = b.coords();
        if let Some(((x0, z0), (x1, z1))) = cells {
            if c.0 < x0.min(x1) || c.0 > x0.max(x1) || c.2 < z0.min(z1) || c.2 > z0.max(z1) {
                continue;
            }
        }
        let own = occ.get(&b.file_cell);
        let acr = across(b.file_cell, b.dir).and_then(|k| occ.get(&k));
        let class = match own {
            None => 0,
            Some(v) if v.iter().all(|x| x.flags & FLAG_PILLAR != 0) => 1,
            Some(_) => 2,
        };
        let variant = b.flags & 63;
        let ground = b.flags & FLAG_GROUND != 0;
        let ghost = b.flags & FLAG_GHOST != 0;
        let vword = format!("{}{}", if ground { "g" } else { "a" }, variant);
        let e = tally.entry((b.name.clone(), vword)).or_default();
        e.0[class] += 1;
        if ghost {
            e.1 += 1;
        }
        let kept = game.as_ref().map(|_| {
            let key = (b.name.clone(), c, b.dir & 3);
            match game_left.get_mut(&key) {
                Some(n) if *n > 0 => {
                    *n -= 1;
                    true
                }
                _ => false,
            }
        });
        if let Some(k) = kept {
            e.2[if k { 0 } else { 1 }] += 1;
        }
        rows += 1;
        if !summary {
            println!(
                "b{}\t{}\t{:08X}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}{}",
                b.index,
                b.name,
                b.flags,
                variant,
                if ground { "G" } else { "" },
                if ghost { "ghost" } else { "" },
                c.0,
                c.1,
                c.2,
                SIDE_NAMES[(b.dir & 3) as usize],
                describe(own),
                describe(acr),
                match kept {
                    Some(true) => "\tY",
                    Some(false) => "\tN",
                    None => "",
                }
            );
        }
    }
    if summary {
        println!("name\tvariant\ttotal\tfree_cell\tpillar_cell\toccupied_cell\tghost{}", if game.is_some() { "\tgame_kept\tgame_absent" } else { "" });
        for ((name, v), (n, g, k)) in &tally {
            println!("{name}\t{v}\t{}\t{}\t{}\t{}\t{g}{}", n[0] + n[1] + n[2], n[0], n[1], n[2], if game.is_some() { format!("\t{}\t{}", k[0], k[1]) } else { String::new() });
        }
    }
    if let Some(g) = &game {
        let extra: usize = game_left.values().sum();
        eprintln!("game list: {} records, {} matched a file record, {} unexplained by the file", g.total, g.total - extra, extra);
        for ((name, c, dir), n) in game_left.iter().filter(|(_, n)| **n > 0) {
            eprintln!("  game-only x{n}: {name} at {:?} side {}", c, SIDE_NAMES[*dir as usize]);
        }
    }
    eprintln!("{rows} fillers listed ({} baked records, {} authored blocks)", m.baked.len(), m.blocks.len());
}

/// The editor's list of blocks as GhostShooter's `/mapblocks2` prints it:
/// `{"list":..,"total":N,"blocks":[{"i":N,"name":"..","cell":[x,y,z],"dir":N,...}, ...]}`.
/// Read by scanning for the fields (the shape is fixed; no JSON library here).
pub struct GameList {
    pub total: usize,
    /// (name, cell, dir & 3) -> how many the game holds
    pub counts: BTreeMap<(String, (i32, i32, i32), u8), usize>,
}

impl GameList {
    pub fn load(p: &Path) -> GameList {
        let text = std::fs::read_to_string(p).unwrap_or_else(|e| panic!("{}: {e}", p.display()));
        let field = |obj: &str, key: &str| -> Option<String> {
            let k = format!("\"{key}\":");
            let at = obj.find(&k)? + k.len();
            let rest = &obj[at..];
            let end = rest.find(|ch: char| ch == ',' || ch == '}').unwrap_or(rest.len());
            Some(rest[..end].trim().trim_matches('"').to_string())
        };
        let mut counts: BTreeMap<(String, (i32, i32, i32), u8), usize> = BTreeMap::new();
        let mut total = 0usize;
        for obj in text.split("{\"i\":").skip(1) {
            let name = field(obj, "name").unwrap_or_default();
            let cell = {
                let k = "\"cell\":[";
                let at = obj.find(k).map(|a| a + k.len());
                let v: Vec<i32> = at.map(|a| obj[a..].split(']').next().unwrap_or("").split(',').filter_map(|s| s.trim().parse().ok()).collect()).unwrap_or_default();
                if v.len() == 3 {
                    (v[0], v[1], v[2])
                } else {
                    continue;
                }
            };
            let dir: u8 = field(obj, "dir").and_then(|s| s.parse::<i32>().ok()).map(|d| (d & 3) as u8).unwrap_or(0);
            *counts.entry((name, cell, dir)).or_insert(0) += 1;
            total += 1;
        }
        GameList { total, counts }
    }
}
