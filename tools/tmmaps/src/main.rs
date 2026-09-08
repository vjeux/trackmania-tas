//! `tmmaps` — everything this project does to a TM2020 **map**.
//!
//! One binary, one container implementation, a control behind every operation.
//! Its counterpart is `tools/ghost`, which owns the **ghost / replay** format;
//! the two never overlap. A recording that carries its own map is a `ghost`
//! problem until the map is out of it:
//!
//! ```text
//! ghost map extract R.Replay.Gbx --out m.Map.Gbx
//! tmmaps move m.Map.Gbx --out m2.Map.Gbx --move 2089@1520,300,600
//! ghost map set R.Replay.Gbx R2.Replay.Gbx --map m2.Map.Gbx
//! ```
//!
//! That composition is deliberate. `u02` used to reach into the replay's
//! carried map itself (`u02 movefree`), which meant two implementations of the
//! embedded-map chunk and two of the block movers. `u02` is deleted; see
//! `MAPS.md` §"What happened to u02".
//!
//! Times are printed as **seconds with a decimal** (`16.316`), never as raw
//! milliseconds.

use tmmaps::cli::{die, flag, flag_multi, has, jobs_of, server_of};
use tmmaps::{census, controls, dropscan, gbx, header, map, oracle, rotate, secs, segments, splice, selftest};

use std::path::{Path, PathBuf};


fn fnv1a(b: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for x in b {
        h ^= *x as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

/// `prs`: one move in a ladder spec. A waypoint is placed either on the world
/// GRID (three cell bytes in its own block record) or FREE (three f32 in chunk
/// 0x0304305F) — and which one a given map's Goal uses is a property of the
/// map, not a choice. 210218's two `GateExpandableFinish` Goals are free; every
/// grid move written for them lands on bytes the game never reads.
#[derive(Clone, Debug)]
enum Move {
    /// `BLK:cx,cy,cz[:dir]` — world grid cell, optional facing byte.
    Cell(usize, (i32, i32, i32), Option<u8>),
    /// `BLK@x,y,z` — absolute metres, for a FREE block.
    Pos(usize, [f32; 3], Option<f32>),
    /// `iN@x,y,z[/yaw]` — absolute metres, for a gate ITEM. Position only,
    /// plus an optional yaw: an item gate triggers on a PLANE like a block
    /// gate does, so an unrotated relocation is silent whenever the car is
    /// running parallel to it (w612's `dir` finding, in the item regime).
    Item(usize, [f32; 3], Option<f32>),
    /// `bN@x,y,z` — a **baked free block** (chunk `0x03043048`, position in
    /// `0x0304305F`). Movable, by position only.
    ///
    /// This used to be a blanket refusal, and the refusal was half right. The
    /// half that is right: baked indices count from 0 in their **own** list, so
    /// a bare `2461` pasted from a census listing addresses an unrelated block
    /// in `0x0304301F` — and that mover would SUCCEED. The map loads, the
    /// origin control passes (the wrong block is restored just as faithfully),
    /// and the ladder quietly measures the wrong thing. So a baked index needs
    /// a spelling of its own, and `BakedCell` below is still refused outright.
    ///
    /// The half that was wrong: a baked block that is FREE has six f32 of
    /// position exactly like an unbaked free block, and **fifteen of the
    /// sixteen pieces of 173691's added finish gate are baked free blocks.**
    /// Refusing them is what let a pass move one piece of sixteen and report
    /// that it had moved the gate.
    BakedPos(usize, [f32; 3]),
    /// `bN` or `bN:cx,cy,cz` — a baked block addressed by cell. Always
    /// refused: a baked block's cell bytes are dead, so the write would land
    /// on nothing and every control would still pass.
    BakedCell(usize),
}

impl Move {
    fn block(&self) -> usize {
        match self {
            Move::Cell(b, ..)
            | Move::Pos(b, ..)
            | Move::Item(b, ..)
            | Move::BakedPos(b, ..)
            | Move::BakedCell(b) => *b,
        }
    }
    fn is_item(&self) -> bool {
        matches!(self, Move::Item(..))
    }
    /// Every mover calls this first, so no call site can forget the one form
    /// that is always wrong.
    fn reject_baked_cell(&self) {
        if let Move::BakedCell(b) = self {
            die(&format!(
                "b{} names a BAKED block (chunk 0x03043048) by CELL. A baked block's cell bytes \
                 are dead, so that write lands on nothing and every control still passes. A baked \
                 block moves by POSITION only: spell it b{}@x,y,z. And note a baked index is NOT \
                 the same block as unbaked index {} — if you meant the unbaked block, drop the \
                 'b'.",
                b, b, b
            ));
        }
    }
    fn label(&self) -> String {
        match self {
            Move::Cell(_, c, Some(d)) => format!("{},{},{}/{}", c.0, c.1, c.2, d),
            Move::Cell(_, c, None) => format!("{},{},{}", c.0, c.1, c.2),
            Move::Pos(_, p, None) => format!("@{:.1},{:.1},{:.1}", p[0], p[1], p[2]),
            Move::Pos(_, p, Some(y)) => {
                format!("@{:.1},{:.1},{:.1}/{:.3}", p[0], p[1], p[2], y)
            }
            Move::Item(i, p, None) => format!("i{}@{:.1},{:.1},{:.1}", i, p[0], p[1], p[2]),
            Move::Item(i, p, Some(y)) => {
                format!("i{}@{:.1},{:.1},{:.1}/{:.3}", i, p[0], p[1], p[2], y)
            }
            Move::BakedPos(i, p) => format!("b{}@{:.1},{:.1},{:.1}", i, p[0], p[1], p[2]),
            Move::BakedCell(b) => format!("b{}", b),
        }
    }
}

fn parse_move(m: &str) -> Move {
    // BLK:cx,cy,cz  or  BLK:cx,cy,cz:dir  or  BLK@x,y,z  or  iN@x,y,z[/yaw]
    // or bN@x,y,z (a baked FREE block) or bN / bN:... (refused by name)
    if let Some((b, rest)) = m.split_once('@') {
        let b = b.trim();
        let (p, yaw) = match rest.split_once('/') {
            Some((p, y)) => (p, Some(y.trim().parse::<f32>().expect("yaw in radians"))),
            None => (rest, None),
        };
        let v: Vec<f32> = p.split(',').map(|x| x.trim().parse().expect("x,y,z")).collect();
        assert_eq!(v.len(), 3, "free position wants x,y,z in metres");
        if let Some(i) = b.strip_prefix('b') {
            assert!(yaw.is_none(), "a baked free block takes a position, not a yaw");
            return Move::BakedPos(i.parse().expect("baked block index"), [v[0], v[1], v[2]]);
        }
        if let Some(i) = b.strip_prefix('i') {
            return Move::Item(i.parse().expect("item index"), [v[0], v[1], v[2]], yaw);
        }
        return Move::Pos(b.parse().expect("block index"), [v[0], v[1], v[2]], yaw);
    }
    let mut it = m.split(':');
    let bs = it.next().expect("BLK");
    if let Some(i) = bs.trim().strip_prefix('b') {
        return Move::BakedCell(i.parse().expect("baked block index"));
    }
    let b: usize = bs.parse().expect("block index");
    let c = it.next().expect("BLK:cx,cy,cz[:dir] or BLK@x,y,z");
    let v: Vec<i32> = c.split(',').map(|x| x.trim().parse().expect("cx,cy,cz")).collect();
    assert_eq!(v.len(), 3, "cell wants cx,cy,cz");
    let d = it.next().map(|s| s.trim().parse::<u8>().expect("dir 0..3"));
    Move::Cell(b, (v[0], v[1], v[2]), d)
}


/// A tool whose census is 90 000 lines long will be piped into `head`, and a
/// Rust binary ignores SIGPIPE by default — so the write fails, and the
/// failure surfaces as a panic and a backtrace note on a command that worked.
/// Take the Unix default back. (Declared here rather than pulling in `libc`:
/// this crate has zero dependencies and that is worth keeping.)
fn restore_sigpipe() {
    const SIGPIPE: i32 = 13;
    const SIG_DFL: usize = 0;
    extern "C" {
        fn signal(signum: i32, handler: usize) -> usize;
    }
    unsafe {
        signal(SIGPIPE, SIG_DFL);
    }
}

fn main() {
    restore_sigpipe();
    // A refusal that reaches the user as a Rust panic tells them to run with
    // `RUST_BACKTRACE=1`, which is the wrong instruction: the tool worked, the
    // command was wrong. The library keeps panicking — that is what makes the
    // refusals testable with `catch_unwind` in the suite — but at the CLI
    // boundary a panic prints its message and nothing else.
    std::panic::set_hook(Box::new(|info| {
        let msg = info
            .payload()
            .downcast_ref::<String>()
            .cloned()
            .or_else(|| info.payload().downcast_ref::<&str>().map(|s| s.to_string()))
            .unwrap_or_else(|| "internal error".to_string());
        eprintln!("tmmaps: {}", msg);
        // Exit 3, the same code `die` uses, so a refusal has one code whether
        // it was raised at the CLI boundary or deep in the library. This is
        // safe for the suite, which swaps in its own hook around every
        // `catch_unwind` and therefore never reaches this one.
        std::process::exit(3);
    }));
    // --version / -V. Compile-time only: CARGO_PKG_* come from the crate's
    // Cargo.toml (which inherits the one workspace version), and TAS_BUILD is
    // the git hash the release build sets. option_env! means an ordinary
    // `cargo build` still works and simply reports "dev". No dependency.
    if std::env::args().any(|x| x == "--version" || x == "-V") {
        println!(
            "{} {} ({})",
            option_env!("CARGO_BIN_NAME").unwrap_or(env!("CARGO_PKG_NAME")),
            env!("CARGO_PKG_VERSION"),
            option_env!("TAS_BUILD").unwrap_or("dev")
        );
        std::process::exit(0);
    }
    let args: Vec<String> = std::env::args().collect();
    let cmd = args.get(1).map(|s| s.as_str()).unwrap_or("help");
    // Every MAP-taking subcommand reads `args[2]`, and with the argument
    // missing that is `index out of bounds: the len is 2 but the index is 2` —
    // a panic where a usage line belongs. Say what is missing instead.
    const WANTS_MAP: &[&str] = &[
        "waypoints", "census", "fillers", "region", "colors", "genealogy", "tiny-catalog", "lineup", "shared-cells", "tiny", "tiny-batch", "clear", "shift", "segments", "move", "rotate", "ladder",
        "roundtrip",
        "renamecheck", "cporder", "origin", "chunks", "blockrefs", "setuid", "delblocks", "mediatracker",
    ];
    if WANTS_MAP.contains(&cmd) && args.len() < 3 {
        eprintln!("tmmaps {} needs a MAP path.\n\n{}", cmd, USAGE);
        std::process::exit(2);
    }
    match cmd {
        "selftest" => selftest::run(&args),
        "region" => census::cmd_region(&args),
        "tiny-catalog" => tmmaps::tiny::catalog_cmd(&args),
        "lineup" => tmmaps::tiny::lineup_cmd(&args),
        "shared-cells" => tmmaps::tiny::shared_cells_cmd(&args),
        "tiny" => tmmaps::tiny::cmd(&args),
        "tiny-batch" => tmmaps::tiny::cmd_batch(&args),
        "clear" => census::cmd_clear(&args),
        "shift" => census::cmd_shift(&args),
        "recdump" => {
            let m = map::MapFile::load(Path::new(&args[2]));
            for it in m.items.iter().filter(|it| it.waypoint_tag.is_some()) {
                let (s, e) = it.record_region;
                let hex: String = m.gbx.body[s..e].iter().map(|x| format!("{:02x}", x)).collect();
                println!("i{} {} {} pos_off_rel={} model_off_rel={} anchor? {}\n  {}", it.index, it.model, it.waypoint_tag.clone().unwrap_or_default(), it.pos_off - s, m.item_ids[it.model_field].off - s, it.author.clone().unwrap_or_default(), hex);
            }
        }
        "untag" => {
            // remove an item's waypoint node (it stops being a waypoint; record shrinks to a 4-byte null)
            let m = map::MapFile::load(Path::new(&args[2]));
            let out = args.iter().position(|a| a == "--out").map(|i| args[i + 1].clone()).expect("--out F");
            let pa: usize = args.iter().position(|a| a == "--item").map(|i| args[i + 1].trim_start_matches('i').parse().unwrap()).expect("--item iN");
            let ia = m.items.iter().find(|it| it.index == pa).expect("not an item").clone();
            let mut m = m;
            m.raw_splices.push((ia.waypoint_region, vec![0xff, 0xff, 0xff, 0xff]));
            println!("untagged i{} {} {:?}", pa, ia.model, ia.waypoint_tag);
            m.write_to_reporting(Path::new(&out)).expect("write");
            println!("wrote {out}");
        }
        "swapplace" => {
            // table-safe placement swap: exchange the two placements' MODEL names via the rename machinery,
            // and their pose (pos, yaw/pitch/roll, cell) and waypoint node (tag) via patches/splices.
            // Equivalent to swapping the records, without moving any lookback definition.
            let m = map::MapFile::load(Path::new(&args[2]));
            let out = args.iter().position(|a| a == "--out").map(|i| args[i + 1].clone()).expect("--out F");
            let pa: usize = args.iter().position(|a| a == "--a").map(|i| args[i + 1].trim_start_matches('i').parse().unwrap()).expect("--a iN");
            let pb: usize = args.iter().position(|a| a == "--b").map(|i| args[i + 1].trim_start_matches('i').parse().unwrap()).expect("--b iM");
            let ia = m.items.iter().position(|it| it.index == pa).expect("--a not an item");
            let ib = m.items.iter().position(|it| it.index == pb).expect("--b not an item");
            let (ra, rb) = (m.items[ia].clone(), m.items[ib].clone());
            let orig_colors = m.colors();
            let orig_body = m.gbx.body.clone();
            // the waypoint node AND the v8 tail (flags with the variant byte, pivot, scale,
            // skin FileRef, the two trailing Vec3) travel with the model: one span each
            let wa = m.gbx.body[ra.waypoint_region.0..ra.record_region.1].to_vec();
            let wb = m.gbx.body[rb.waypoint_region.0..rb.record_region.1].to_vec();
            let mut m = m;
            // the per-item side bytes (colour 0x62, anim phase 0x63, foreground 0x65,
            // lightmap quality 0x68: the items are the last ni bytes of each) swap too
            {
                let chunks = map::skip_chunks(&m.gbx.body);
                let ni = m.items.len();
                for cid in [0x0304_3062u32, 0x0304_3063, 0x0304_3065, 0x0304_3068] {
                    let Some(&(_, _, payload, size)) = chunks.iter().find(|(c, ..)| *c == cid) else { continue };
                    if size < 4 + ni {
                        continue;
                    }
                    let base = payload + size - ni;
                    let (ba, bb) = (m.gbx.body[base + ia], m.gbx.body[base + ib]);
                    if ba != bb {
                        m.raw_patches.push((base + ia, vec![bb]));
                        m.raw_patches.push((base + ib, vec![ba]));
                    }
                }
            }
            m.set_item_model(ia, &rb.model);
            m.set_item_model(ib, &ra.model);
            // pose: yaw/pitch/roll + cell + pos
            let mut rot = Vec::new(); for v in [rb.yaw, rb.pitch, rb.roll] { rot.extend_from_slice(&v.to_le_bytes()); }
            m.raw_patches.push((ra.yaw_off, rot));
            let mut rot = Vec::new(); for v in [ra.yaw, ra.pitch, ra.roll] { rot.extend_from_slice(&v.to_le_bytes()); }
            m.raw_patches.push((rb.yaw_off, rot));
            m.raw_patches.push((ra.coord_off, rb.raw_coords.to_vec()));
            m.raw_patches.push((rb.coord_off, ra.raw_coords.to_vec()));
            let mut p = Vec::new(); for v in rb.pos { p.extend_from_slice(&v.to_le_bytes()); }
            m.raw_patches.push((ra.pos_off, p));
            let mut p = Vec::new(); for v in ra.pos { p.extend_from_slice(&v.to_le_bytes()); }
            m.raw_patches.push((rb.pos_off, p));
            // pass 1: renames + fixed-length patches; pass 2 (after reload): the variable-length tag swap
            let tmp = format!("{out}.pass1.tmp");
            m.write_to_reporting(Path::new(&tmp)).expect("write pass 1");
            let m2 = map::MapFile::load(Path::new(&tmp));
            let ja = m2.items.iter().position(|it| it.index == pa).unwrap();
            let jb = m2.items.iter().position(|it| it.index == pb).unwrap();
            let (qa, qb) = (m2.items[ja].clone(), m2.items[jb].clone());
            let mut m = m2;
            m.raw_splices.push(((qa.waypoint_region.0, qa.record_region.1), wb));
            m.raw_splices.push(((qb.waypoint_region.0, qb.record_region.1), wa));
            let _ = std::fs::remove_file(&tmp);
            println!("swapped placements i{} {} {:?} <-> i{} {} {:?} (models via rename, pose via patch, waypoint node + v8 tail + side bytes via splice)", pa, ra.model, ra.waypoint_tag, pb, rb.model, rb.waypoint_tag);
            m.write_to_reporting(Path::new(&out)).expect("write");
            println!("wrote {out}");
            // `--check`: re-read the WRITTEN map and assert that every
            // per-placement field arrived. A swap that silently drops one is
            // the dangerous kind — a start moved into the engine's slot must
            // not rescale, re-anchor, recolour or re-variant either item, and
            // the freeze pass depends on that (2026-09-07).
            if args.iter().any(|a| a == "--check") {
                let m3 = map::MapFile::load(Path::new(&out));
                let ga = m3.items.iter().find(|it| it.index == pa).expect("a");
                let gb = m3.items.iter().find(|it| it.index == pb).expect("b");
                let same = |x: [f32; 3], y: [f32; 3]| x.iter().zip(y).all(|(u, v)| (u - v).abs() < 1e-4);
                let mut bad = Vec::new();
                if ga.model != rb.model || gb.model != ra.model { bad.push("model"); }
                if !same(ga.pos, rb.pos) || !same(gb.pos, ra.pos) { bad.push("pos"); }
                if (ga.yaw - rb.yaw).abs() > 1e-6 || (gb.yaw - ra.yaw).abs() > 1e-6 { bad.push("yaw"); }
                if !same(ga.pivot, rb.pivot) || !same(gb.pivot, ra.pivot) { bad.push("pivot"); }
                if (ga.scale - rb.scale).abs() > 1e-6 || (gb.scale - ra.scale).abs() > 1e-6 { bad.push("scale"); }
                if ga.variant() != rb.variant() || gb.variant() != ra.variant() { bad.push("variant"); }
                if ga.waypoint_tag != rb.waypoint_tag || gb.waypoint_tag != ra.waypoint_tag { bad.push("waypoint tag"); }
                if ga.waypoint_order != rb.waypoint_order || gb.waypoint_order != ra.waypoint_order { bad.push("waypoint order"); }
                if ga.skin(&m3.gbx.body) != rb.skin(&orig_body) || gb.skin(&m3.gbx.body) != ra.skin(&orig_body) { bad.push("skin"); }
                if let (Some(c0), Some(c1)) = (orig_colors.as_ref(), m3.colors()) {
                    if c1.item(pa) != c0.item(pb) || c1.item(pb) != c0.item(pa) { bad.push("colour"); }
                }
                if bad.is_empty() {
                    println!("check: every per-placement field arrived (model, pose, pivot, scale, variant, tag, order, colour, skin)");
                } else {
                    eprintln!("check FAILED: {} did not survive the swap", bad.join(", "));
                    std::process::exit(1);
                }
            }
        }
        "swaprec" => {
            // swap two whole item placement RECORDS (file order experiment); the item count is unchanged
            let m = map::MapFile::load(Path::new(&args[2]));
            let out = args.iter().position(|a| a == "--out").map(|i| args[i + 1].clone()).expect("--out F");
            let pa: usize = args.iter().position(|a| a == "--a").map(|i| args[i + 1].trim_start_matches('i').parse().unwrap()).expect("--a iN");
            let pb: usize = args.iter().position(|a| a == "--b").map(|i| args[i + 1].trim_start_matches('i').parse().unwrap()).expect("--b iM");
            let ia = m.items.iter().find(|it| it.index == pa).expect("--a not an item").clone();
            let ib = m.items.iter().find(|it| it.index == pb).expect("--b not an item").clone();
            let mut ra = m.gbx.body[ia.record_region.0..ia.record_region.1].to_vec();
            let mut rb = m.gbx.body[ib.record_region.0..ib.record_region.1].to_vec();
            // the author field is a lookback REFERENCE to the model-name slot this very record defines
            // (slot numbers follow file order), so each moved record must take over the slot word of
            // the position it lands in -- otherwise the file carries a forward reference and the game
            // refuses to load it
            let (aa, ab) = (&m.item_ids[ia.author_field], &m.item_ids[ib.author_field]);
            if aa.len == 4 && ab.len == 4 && m.item_ids[ia.model_field].is_def && m.item_ids[ib.model_field].is_def {
                let oa = aa.off - ia.record_region.0; // author word offset inside record a
                let ob = ab.off - ib.record_region.0;
                // rb goes to a's position: its model def takes a's slot; its author word must be a's word
                rb[ob..ob + 4].copy_from_slice(&aa.raw.to_le_bytes());
                ra[oa..oa + 4].copy_from_slice(&ab.raw.to_le_bytes());
                println!("author slot words re-homed: {:#x} <-> {:#x}", aa.raw, ab.raw);
            }
            let mut m = m;
            m.raw_splices.push((ia.record_region, rb));
            m.raw_splices.push((ib.record_region, ra));
            println!("swapped records: i{} ({} B) <-> i{} ({} B)", pa, ia.record_region.1 - ia.record_region.0, pb, ib.record_region.1 - ib.record_region.0);
            m.write_to_reporting(Path::new(&out)).expect("write");
            println!("wrote {out}");
        }
        "swapcell" => {
            // swap the CELL bytes (raw_coords) of two item placements; positions untouched
            let m = map::MapFile::load(Path::new(&args[2]));
            let out = args.iter().position(|a| a == "--out").map(|i| args[i + 1].clone()).expect("--out F");
            let pa: usize = args.iter().position(|a| a == "--a").map(|i| args[i + 1].trim_start_matches('i').parse().unwrap()).expect("--a iN");
            let pb: usize = args.iter().position(|a| a == "--b").map(|i| args[i + 1].trim_start_matches('i').parse().unwrap()).expect("--b iM");
            let ia = m.items.iter().find(|it| it.index == pa).expect("--a not an item").clone();
            let ib = m.items.iter().find(|it| it.index == pb).expect("--b not an item").clone();
            let mut m = m;
            m.raw_patches.push((ia.coord_off, ib.raw_coords.to_vec()));
            m.raw_patches.push((ib.coord_off, ia.raw_coords.to_vec()));
            println!("swapped cells: i{} {:?} <-> i{} {:?}", pa, ia.raw_coords, pb, ib.raw_coords);
            m.write_to_reporting(Path::new(&out)).expect("write");
            println!("wrote {out}");
        }
        "swapmodel" => {
            // swap the model Id words of two item placements (both must be 4-byte table references)
            let m = map::MapFile::load(Path::new(&args[2]));
            let out = args.iter().position(|a| a == "--out").map(|i| args[i + 1].clone()).expect("--out F");
            let pa: usize = args.iter().position(|a| a == "--a").map(|i| args[i + 1].trim_start_matches('i').parse().unwrap()).expect("--a iN");
            let pb: usize = args.iter().position(|a| a == "--b").map(|i| args[i + 1].trim_start_matches('i').parse().unwrap()).expect("--b iM");
            let ia = m.items.iter().find(|it| it.index == pa).expect("--a not an item").clone();
            let ib = m.items.iter().find(|it| it.index == pb).expect("--b not an item").clone();
            let fa = m.item_ids[ia.model_field].clone();
            let fb = m.item_ids[ib.model_field].clone();
            let mut m = m;
            if fa.len == 4 && fb.len == 4 {
                let (oa, ob, ra, rb) = (fa.off, fb.off, fa.raw, fb.raw);
                m.raw_patches.push((oa, rb.to_le_bytes().to_vec()));
                m.raw_patches.push((ob, ra.to_le_bytes().to_vec()));
            } else if fa.is_def && fb.is_def && fa.len == fb.len {
                // both inline definitions of equal length: swap the name strings (every later
                // reference to either table slot follows the swap: the two MODELS trade places)
                let na = fa.name.clone().unwrap_or_default().into_bytes();
                let nb = fb.name.clone().unwrap_or_default().into_bytes();
                m.raw_patches.push((fa.off + 8, nb));
                m.raw_patches.push((fb.off + 8, na));
                println!("(inline definitions: the two model NAMES were swapped at their definition sites -- every placement of either model trades models)");
            } else {
                panic!("model ids not swappable: a len {} def {} / b len {} def {}", fa.len, fa.is_def, fb.len, fb.is_def);
            }
            println!("swapped model ids: i{} {} <-> i{} {} (words {:#x} <-> {:#x})", pa, ia.model, pb, ib.model, fa.raw, fb.raw);
            m.write_to_reporting(Path::new(&out)).expect("write swapped map");
            println!("wrote {out}");
        }
        "wpdump" => {
            // every waypoint ITEM's raw placement record fields, for diffing maps
            let m = map::MapFile::load(Path::new(&args[2]));
            println!("idx\tmodel\tauthor\tcoll\ttag\torder\tyaw\tpitch\troll\tcoords\tpos\tflags\tpivot\tscale\ttail24\twpnode_bytes\trecord_bytes");
            for it in m.items.iter().filter(|it| it.waypoint_tag.is_some()) {
                let b = &m.gbx.body;
                let (ws, we) = it.waypoint_region;
                // order = the u32 after the tag string inside the waypoint node (v2)
                let order = if we - ws >= 16 { u32::from_le_bytes(b[we - 8..we - 4].try_into().unwrap()) } else { 0 };
                let flags = u16::from_le_bytes(b[we..we + 2].try_into().unwrap());
                let tail_start = it.scale_off + 4 + if flags & 4 != 0 { 0 } else { 0 };
                let tail: Vec<f32> = (0..6).map(|k| f32::from_le_bytes(b[tail_start + 4 * k..tail_start + 4 * k + 4].try_into().unwrap())).collect();
                let wp_hex: String = b[ws..we].iter().map(|x| format!("{:02x}", x)).collect();
                println!("{}\t{}\t{}\t{:#x}\t{}\t{}\t{:.4}\t{:.4}\t{:.4}\t{:?}\t({:.3},{:.3},{:.3})\t{:#06x}\t({:.3},{:.3},{:.3})\t{:.3}\t{:?}\t{}\t{}", it.index, it.model, it.author.clone().unwrap_or_default(), it.collection_raw, it.waypoint_tag.clone().unwrap_or_default(), order, it.yaw, it.pitch, it.roll, it.raw_coords, it.pos[0], it.pos[1], it.pos[2], flags, it.pivot[0], it.pivot[1], it.pivot[2], it.scale, tail, wp_hex, it.record_region.1 - it.record_region.0);
            }
        }
        "waypoints" => {
            let m = map::MapFile::load(Path::new(&args[2]));
            eprintln!(
                "size={:?} decoration={} collection={:#x} blocks={} items={} body_regions={:?} items_region={:?}",
                m.size,
                m.decoration_id,
                m.items.first().map(|it| it.collection_raw).unwrap_or(0),
                m.blocks.len(),
                m.items.len(),
                m.body_regions.clone(),
                m.items_region
            );
            for (i, w) in m.waypoints().iter().enumerate() {
                println!("{} {}", i, w);
            }
        }
        "segat" => segments::cmd_segat(&args),
        "segments" => {
            let src = PathBuf::from(&args[2]);
            let out = PathBuf::from(flag(&args, "--out").unwrap_or("/tmp/segmaps"));
            let g = flag(&args, "--ref-ghost").expect("--ref-ghost is required (order is measured)");
            let ord: Option<Vec<String>> = flag(&args, "--order")
                .map(|s| s.split(',').map(|v| v.trim().to_string()).collect());
            let segs = match segments::make_all_ordered(
                &src,
                &out,
                Path::new(g),
                jobs_of(&args),
                &server_of(&args),
                true,
                ord.as_deref(),
            ) {
                Ok(s) => s,
                Err(e) => {
                    eprintln!("{}", e);
                    std::process::exit(2);
                }
            };
            for s in &segs {
                println!(
                    "seg{} {} cut={} method={} exact={} time={} expect={} verified={}",
                    s.segment,
                    s.map.display(),
                    s.cut,
                    s.method,
                    s.exact,
                    secs::opt(s.time),
                    secs::ms(s.expect),
                    s.verified
                );
            }
        }
        "ladder" => {
            let src = PathBuf::from(&args[2]);
            let out = PathBuf::from(flag(&args, "--out").unwrap_or("/tmp/tmmaps-ladder"));
            let jobs = jobs_of(&args);
            let server = server_of(&args);
            let ghosts: Vec<PathBuf> = flag_multi(&args, "--ghosts")
                .into_iter()
                .map(PathBuf::from)
                .collect();
            assert!(!ghosts.is_empty(), "ladder needs --ghosts");
            let spec = std::fs::read_to_string(
                flag(&args, "--spec").expect("--spec FILE (one rung per line)"),
            )
            .expect("read spec");
            let rungs: Vec<Vec<Move>> = spec
                .lines()
                .map(|l| l.split('#').next().unwrap_or("").trim())
                .filter(|l| !l.is_empty())
                .map(|l| l.split_whitespace().map(parse_move).collect())
                .collect();
            assert!(!rungs.is_empty(), "ladder needs rungs");
            // Refuse baked indices BEFORE anything indexes `m0.blocks`. On a
            // small map a baked index happens to be out of range and panics on
            // bounds; on a big one (210218 has 21 025 unbaked blocks) it is
            // perfectly in range and would silently address the wrong block.
            // The whole point of the `bN` spelling is that this cannot depend
            // on the map's size.
            for r in rungs.iter().flatten() {
                r.reject_baked_cell();
            }

            let m0 = map::MapFile::load(&src);
            let mut movers: Vec<(bool, usize)> =
                rungs.iter().flatten().map(|m| (m.is_item(), m.block())).collect();
            movers.sort_unstable();
            movers.dedup();
            for (is_item, b) in &movers {
                if *is_item {
                    let r = &m0.items[*b];
                    eprintln!(
                        "ladder: mover item#{} {} regime=ITEM pos {:?} yaw {:.4} tag={:?}",
                        b, r.model, r.pos, r.yaw, r.waypoint_tag
                    );
                } else {
                    let r = &m0.blocks[*b];
                    eprintln!(
                        "ladder: mover block#{} {} regime={} dir={} home {:?} pos {:?} tag={:?}",
                        b,
                        r.name,
                        if r.free_off.is_some() { "FREE" } else { "grid" },
                        r.dir,
                        r.coords(),
                        r.free_pos,
                        r.waypoint_tag
                    );
                }
            }
            std::fs::create_dir_all(&out).unwrap();
            let write_moves = |mv: &[Move], path: &PathBuf| {
                let mut m = map::MapFile::load(&src);
                for mo in mv {
                    mo.reject_baked_cell();
                    match mo {
                        Move::Cell(b, c, d) => {
                            // `prs`: refuse the silent case rather than write
                            // dead bytes -- a grid move on a FREE block loads
                            // fine and is silent for every rung.
                            assert!(
                                m.blocks[*b].free_off.is_none(),
                                "block#{} is a FREE block: give it BLK@x,y,z in metres, not a cell",
                                b
                            );
                            m.move_block_cell(*b, *c);
                            if let Some(d) = d {
                                m.set_block_dir(*b, *d);
                            }
                        }
                        Move::Pos(b, p, y) => {
                            m.move_block_free(*b, *p);
                            if let Some(y) = y {
                                // `prs`: the free-block twin of the `dir` byte.
                                // pitchYawRoll = (yaw, pitch, roll); only the
                                // facing is touched, so this stays a rotation
                                // and not a promotion.
                                let r = m0.blocks[*b].free_rot.unwrap();
                                m.set_block_free_rot(*b, [*y, r[1], r[2]]);
                            }
                        }
                        // unreachable: reject_baked_cell() panicked above. Kept
                        // so the compiler enforces that a new mover cannot
                        // forget the case.
                        Move::BakedCell(_) => unreachable!("reject_baked_cell"),
                        Move::BakedPos(b, p) => m.move_baked_free(*b, *p),
                        Move::Item(i, p, y) => {
                            m.move_item_pos(*i, *p);
                            if let Some(y) = y {
                                m.set_item_yaw(*i, *y);
                            }
                        }
                    }
                }
                m.write_to(path).expect("write ladder map");
            };

            eprintln!("ladder: measuring the UNTOUCHED map first");
            let base = oracle::run_maps(&[(src.clone(), ghosts.clone())], jobs, &server);
            let want = oracle::times(&base[0]);
            for (k, v) in &want {
                eprintln!("  untouched {} -> {}", k, secs::opt(*v));
            }

            // origin control: EVERY mover rewritten to its own home cell AND
            // its own dir, by the same surgery the rungs use.
            let homes: Vec<Move> = movers
                .iter()
                .map(|(is_item, b)| {
                    if *is_item {
                        Move::Item(*b, m0.items[*b].pos, Some(m0.items[*b].yaw))
                    } else {
                        match m0.blocks[*b].free_pos {
                            Some(p) => Move::Pos(*b, p, Some(m0.blocks[*b].free_rot.unwrap()[0])),
                            None => Move::Cell(*b, m0.blocks[*b].coords(), Some(m0.blocks[*b].dir)),
                        }
                    }
                })
                .collect();
            let ctrl = out.join("CONTROL_origin.Map.Gbx");
            write_moves(&homes, &ctrl);
            let cres = oracle::run_maps(&[(ctrl.clone(), ghosts.clone())], jobs, &server);
            let got = oracle::times(&cres[0]);
            let mut bad = Vec::new();
            for (k, w) in &want {
                let g = got.get(k).cloned().flatten();
                if g != *w {
                    bad.push(format!(
                        "{}: untouched {} vs rebuilt-at-origin {}",
                        k,
                        secs::opt(*w),
                        secs::opt(g)
                    ));
                }
            }
            if !bad.is_empty() {
                eprintln!("\nLADDER ABORTED -- the return-to-origin control FAILED:");
                for b in &bad {
                    eprintln!("  {}", b);
                }
                std::process::exit(9);
            }
            eprintln!(
                "control OK: rebuilt-at-origin ({} movers) reproduces the untouched map for all {} ghosts",
                movers.len(),
                want.len()
            );

            let mut pairs = Vec::new();
            let mut hashes = std::collections::HashSet::new();
            for (i, mv) in rungs.iter().enumerate() {
                let p = out.join(format!("crung{:03}.Map.Gbx", i));
                write_moves(mv, &p);
                hashes.insert(fnv1a(&std::fs::read(&p).expect("read back rung")));
                pairs.push((p, ghosts.clone()));
            }
            assert_eq!(
                hashes.len(),
                rungs.len(),
                "DISTINCTNESS FAILED: {} rungs produced only {} distinct maps",
                rungs.len(),
                hashes.len()
            );
            eprintln!(
                "distinctness OK: {} rungs -> {} files -> {} distinct hashes",
                rungs.len(),
                pairs.len(),
                hashes.len()
            );

            let res = oracle::run_maps(&pairs, jobs, &server);
            let names: Vec<String> = ghosts
                .iter()
                .map(|g| g.file_name().unwrap().to_string_lossy().into_owned())
                .collect();
            print!("{:<5} {:<38} |", "rung", "gates");
            for n in &names {
                print!(" {:>24} |", &n[..n.len().min(24)]);
            }
            println!();
            for (i, mv) in rungs.iter().enumerate() {
                let t = oracle::times(&res[i]);
                let desc: Vec<String> = mv.iter().map(|m| m.label()).collect();
                print!("{:<5} {:<46} |", i, desc.join(" "));
                for n in &names {
                    let v = t.get(n).cloned().flatten();
                    print!(" {:>24} |", secs::opt(v));
                }
                println!();
            }
            println!();
            println!("(a cell equal to the run's UNTOUCHED time means the rung was SILENT for it)");
        }
        // ---- w612: write ONE map with several grid blocks moved.
        "rotate" => rotate::cmd(&args),
        "move" => {
            let src = PathBuf::from(&args[2]);
            let out = PathBuf::from(flag(&args, "--out").expect("--out F"));
            let mv = flag_multi(&args, "--move");
            assert!(!mv.is_empty(), "--move BLK:cx,cy,cz [BLK:cx,cy,cz ...]");
            let mut m = map::MapFile::load(&src);
            for s in &mv {
                let pm = parse_move(s);
                pm.reject_baked_cell();
                match pm {
                    Move::Cell(bi, c, d) => {
                        assert!(
                            m.blocks[bi].free_off.is_none(),
                            "block#{} is a FREE block: give it BLK@x,y,z in metres, not a cell",
                            bi
                        );
                        let name = m.blocks[bi].name.clone();
                        let home = m.blocks[bi].coords();
                        let hd = m.blocks[bi].dir;
                        m.move_block_cell(bi, c);
                        if let Some(d) = d {
                            m.set_block_dir(bi, d);
                        }
                        println!(
                            "  block#{} {} {:?}/dir{} -> {:?}/dir{}",
                            bi,
                            name,
                            home,
                            hd,
                            c,
                            d.unwrap_or(hd)
                        );
                    }
                    Move::BakedCell(_) => unreachable!("reject_baked_cell"),
                    Move::BakedPos(bi, p) => {
                        let name = m.baked[bi].name.clone();
                        let home = m.baked[bi].free_pos;
                        m.move_baked_free(bi, p);
                        println!("  b{} {} BAKED FREE {:?} -> {:?}", bi, name, home, p);
                    }
                    Move::Item(ii, p, y) => {
                        let model = m.items[ii].model.clone();
                        let home = m.items[ii].pos;
                        m.move_item_pos(ii, p);
                        if let Some(y) = y {
                            m.set_item_yaw(ii, y);
                        }
                        println!("  item#{} {} ITEM {:?} -> {:?} yaw {:?}", ii, model, home, p, y);
                    }
                    Move::Pos(bi, p, _y) => {
                        let name = m.blocks[bi].name.clone();
                        let home = m.blocks[bi].free_pos;
                        m.move_block_free(bi, p);
                        println!("  block#{} {} FREE {:?} -> {:?}", bi, name, home, p);
                    }
                }
            }
            let sp = m.write_to_reporting(&out).expect("write moved map");
            println!("wrote {}\n  {}", out.display(), sp.summary());
        }
        "oracle" => {
            // --map M --ghosts a b c  (repeatable)
            let mut pairs: Vec<(PathBuf, Vec<PathBuf>)> = Vec::new();
            let mut i = 2;
            let mut cur: Option<PathBuf> = None;
            let mut ghosts: Vec<PathBuf> = Vec::new();
            while i < args.len() {
                match args[i].as_str() {
                    "--map" => {
                        if let Some(m) = cur.take() {
                            pairs.push((m, ghosts.clone()));
                        }
                        cur = Some(PathBuf::from(&args[i + 1]));
                        i += 2;
                    }
                    "--ghosts" => {
                        ghosts.clear();
                        let mut j = i + 1;
                        while j < args.len() && !args[j].starts_with("-") {
                            ghosts.push(PathBuf::from(&args[j]));
                            j += 1;
                        }
                        i = j;
                    }
                    _ => i += 1,
                }
            }
            if let Some(m) = cur.take() {
                pairs.push((m, ghosts.clone()));
            }
            // --shard: one map, many ghosts, split across N servers
            if has(&args, "--shard") && pairs.len() == 1 {
                let rows = oracle::run_map_sharded(
                    &pairs[0].0,
                    &pairs[0].1,
                    jobs_of(&args),
                    &server_of(&args),
                );
                for r in rows {
                    println!(
                        "{}\t{}\t{}\tcps={}",
                        pairs[0].0.file_name().unwrap().to_string_lossy(),
                        r.file,
                        secs::opt(r.sim_time),
                        r.reached_cps.map(|v| v.to_string()).unwrap_or("-".into())
                    );
                }
                return;
            }
            let res = oracle::run_maps(&pairs, jobs_of(&args), &server_of(&args));
            for (i, rows) in res.iter().enumerate() {
                for r in rows {
                    println!(
                        "{}\t{}\t{}\tcps={}",
                        pairs[i].0.file_name().unwrap().to_string_lossy(),
                        r.file,
                        secs::opt(r.sim_time),
                        r.reached_cps.map(|v| v.to_string()).unwrap_or("-".into())
                    );
                }
            }
        }
        "roundtrip" => controls::cmd_roundtrip(&args),
        "bodydiff" => splice::cmd_bodydiff(&args),
        "rewrite" => splice::cmd_rewrite(&args),
        "renamecheck" => {
            // `prs`: the RENAMING round-trip. The identity round-trip
            // (`tmmaps roundtrip`) is blind to a whole class of surgery bug,
            // because it never adds or removes a lookback-table slot -- and
            // the table is exactly where a rename can go wrong. The blocks
            // chunk and the baked chunk share one table, and parts of the file
            // downstream of both hold raw indices into it, so a rename that
            // changes the table's LENGTH can silently renumber somebody else's
            // name.
            //
            // So: rename one waypoint, write, re-read, and require that EVERY
            // OTHER block name, item model, waypoint tag and waypoint
            // placement is unchanged. Three renames are tried, because they
            // stress the table in different directions:
            //
            //   same-length  -- content changes, no slot moves
            //   fresh        -- a name the table has never seen (may add a slot)
            //   existing     -- another block's name (may drop a slot)
            //
            // `reemit_regions` already warns "downstream indices may not
            // resolve" when neither encoder preserves the length. This turns
            // that warning into a pass/fail on the actual names.
            let src = PathBuf::from(&args[2]);
            let m0 = map::MapFile::load(&src);
            let wp: Vec<usize> = m0
                .blocks
                .iter()
                .filter(|b| b.waypoint_tag.is_some())
                .map(|b| b.index)
                .collect();
            if wp.is_empty() {
                println!("{}: no waypoint blocks to rename", src.display());
                return;
            }
            let target = wp[0];
            let orig = m0.blocks[target].name.clone();
            // a name the table has never seen, one the same length, and one
            // that another block already owns
            let same: String = {
                let mut s = orig.clone();
                let n = s.len();
                s.replace_range(n - 1.., "Z");
                s
            };
            let fresh = format!("{}_prsRenameCheck", orig);
            let other = m0
                .blocks
                .iter()
                .map(|b| b.name.clone())
                .find(|n| *n != orig && !n.is_empty())
                .unwrap_or_else(|| "RoadTechStraight".into());
            let mut fails = 0;
            // test 0 -- rename to ITSELF. This forces the whole two-region Id
            // stream through the rename re-encoder (`Mode::SlotPreserving` /
            // `Fresh`) instead of the identity memcpy path, and requires the
            // result to be byte-identical. `tmmaps roundtrip` never exercises
            // that code at all.
            {
                let mut m = map::MapFile::load(&src);
                m.set_block_name(target, &orig);
                let built = gbx::Gbx::parse(&m.build()).body;
                let ok = built == m0.gbx.body;
                println!(
                    "  {:<12} {}   (re-encoder exercised, output must be byte-identical)",
                    "self",
                    if ok { "OK  " } else { "FAIL" }
                );
                if !ok {
                    fails += 1;
                }
            }
            for (label, newname) in
                [("same-length", &same), ("fresh", &fresh), ("existing", &other)]
            {
                let mut m = map::MapFile::load(&src);
                m.set_block_name(target, newname);
                let tmp = std::env::temp_dir()
                    .join(format!("prs-renamecheck-{}.Map.Gbx", std::process::id()));
                if m.write_to(&tmp).is_err() {
                    println!("  {:<12} WRITE FAILED", label);
                    fails += 1;
                    continue;
                }
                let m2 = match std::panic::catch_unwind(|| map::MapFile::load(&tmp)) {
                    Ok(v) => v,
                    Err(_) => {
                        println!("  {:<12} RE-READ PANICKED", label);
                        fails += 1;
                        continue;
                    }
                };
                let mut bad: Vec<String> = Vec::new();
                if m2.blocks.len() != m0.blocks.len() {
                    bad.push(format!("block count {} -> {}", m0.blocks.len(), m2.blocks.len()));
                }
                if m2.items.len() != m0.items.len() {
                    bad.push(format!("item count {} -> {}", m0.items.len(), m2.items.len()));
                }
                for (a, b) in m0.blocks.iter().zip(m2.blocks.iter()) {
                    if a.index == target {
                        if b.name != *newname {
                            bad.push(format!(
                                "target block#{} name {:?} != {:?}",
                                a.index, b.name, newname
                            ));
                        }
                        continue;
                    }
                    if a.name != b.name {
                        bad.push(format!(
                            "block#{} name {:?} -> {:?}",
                            a.index, a.name, b.name
                        ));
                    }
                    if a.waypoint_tag != b.waypoint_tag
                        || a.raw_coords != b.raw_coords
                        || a.dir != b.dir
                        || a.free_pos != b.free_pos
                    {
                        bad.push(format!("block#{} placement/tag changed", a.index));
                    }
                    if bad.len() > 6 {
                        break;
                    }
                }
                for (a, b) in m0.items.iter().zip(m2.items.iter()) {
                    if a.model != b.model || a.waypoint_tag != b.waypoint_tag || a.pos != b.pos {
                        bad.push(format!("item#{} {:?} -> {:?}", a.index, a.model, b.model));
                    }
                    if bad.len() > 6 {
                        break;
                    }
                }
                let _ = std::fs::remove_file(&tmp);
                if bad.is_empty() {
                    println!("  {:<12} OK   (renamed block#{})", label, target);
                } else {
                    fails += 1;
                    println!("  {:<12} FAIL {} problem(s):", label, bad.len());
                    for b in bad.iter().take(6) {
                        println!("      {}", b);
                    }
                }
            }
            println!("{}: renamecheck {} failure(s)", src.display(), fails);
            // With --ghosts, add the check the parser cannot make: a
            // mutually-consistent reader/writer error is invisible to a
            // re-read, so ask the GAME. Rename a block that is far from every
            // waypoint -- decoration, not track -- to a fresh name, and
            // require the control ghost's time to be unchanged. A table that
            // renumbered somebody else's name shows up as "Can't load map"
            // (no row at all) or as a different time.
            let ghosts: Vec<PathBuf> =
                flag_multi(&args, "--ghosts").into_iter().map(PathBuf::from).collect();
            if !ghosts.is_empty() {
                let wpos: Vec<(i32, i32, i32)> =
                    wp.iter().map(|i| m0.blocks[*i].coords()).collect();
                let far = m0
                    .blocks
                    .iter()
                    .filter(|b| b.waypoint_tag.is_none() && !b.name.is_empty())
                    .max_by_key(|b| {
                        let (x, y, z) = b.coords();
                        wpos.iter()
                            .map(|(a, c, d)| {
                                (x - a).pow(2) + (y - c).pow(2) + (z - d).pow(2)
                            })
                            .min()
                            .unwrap_or(0)
                    })
                    .map(|b| b.index);
                if let Some(fi) = far {
                    let mut m = map::MapFile::load(&src);
                    let fname = m0.blocks[fi].name.clone();
                    m.set_block_name(fi, &format!("{}_prsRenameCheck", fname));
                    let tmp = std::env::temp_dir()
                        .join(format!("prs-rc-game-{}.Map.Gbx", std::process::id()));
                    m.write_to(&tmp).expect("write renamed map");
                    let base = oracle::run_maps(
                        &[(src.clone(), ghosts.clone())],
                        jobs_of(&args),
                        &server_of(&args),
                    );
                    let got = oracle::run_maps(
                        &[(tmp.clone(), ghosts.clone())],
                        jobs_of(&args),
                        &server_of(&args),
                    );
                    let want = oracle::times(&base[0]);
                    let have = oracle::times(&got[0]);
                    let mut bad = 0;
                    for (k, w) in &want {
                        let g = have.get(k).cloned().flatten();
                        if g != *w {
                            println!("      GAME {}: untouched {:?} vs renamed {:?}", k, w, g);
                            bad += 1;
                        }
                    }
                    if have.is_empty() {
                        println!("      GAME: the renamed map produced NO rows -- it did not load");
                        bad += 1;
                    }
                    println!(
                        "  {:<12} {}   (renamed off-route block#{} {:?}, {} control ghost(s))",
                        "game",
                        if bad == 0 { "OK  " } else { "FAIL" },
                        fi,
                        fname.chars().take(40).collect::<String>(),
                        want.len()
                    );
                    let _ = std::fs::remove_file(&tmp);
                    if bad > 0 {
                        fails += 1;
                    }
                    println!("{}: renamecheck (with game check) {} failure(s)", src.display(), fails);
                }
            }
            if fails > 0 {
                std::process::exit(1);
            }
        }
        "cporder" => {
            // `prs`: map each declared checkpoint SPLIT to the waypoint that
            // produced it, by matching the reference ghost's own position at
            // the split time against every waypoint's position.
            //
            // `segments::order_checkpoints` measures the same thing with
            // O(n^2) oracle runs. This is one trajectory decode and some
            // arithmetic, and unlike the oracle version it reports the
            // matching DISTANCE, so a bad match is visible rather than
            // silently ranked first.
            //
            //   tmmaps cporder MAP TRAJ.csv --splits a,b,c,...
            let m = map::MapFile::load(Path::new(&args[2]));
            let csv = std::fs::read_to_string(&args[3]).expect("read trajectory csv");
            let splits: Vec<f64> = flag(&args, "--splits")
                .expect("--splits t1,t2,... (from `tmmaps splits GHOST`)")
                .trim_matches(|c| c == '[' || c == ']')
                .split(',')
                .map(|x| x.trim().parse().unwrap())
                .collect();
            let mut traj: Vec<(f64, [f64; 3])> = Vec::new();
            for (i, line) in csv.lines().enumerate() {
                if i == 0 {
                    continue;
                }
                let f: Vec<&str> = line.split(',').collect();
                if f.len() < 4 {
                    continue;
                }
                traj.push((
                    f[0].parse().unwrap(),
                    [f[1].parse().unwrap(), f[2].parse().unwrap(), f[3].parse().unwrap()],
                ));
            }
            let at = |t: f64| -> [f64; 3] {
                let j = traj.partition_point(|(ts, _)| *ts <= t).max(1).min(traj.len() - 1);
                let (t0, p0) = traj[j - 1];
                let (t1, p1) = traj[j];
                let f = if (t1 - t0).abs() < 1e-9 { 0.0 } else { (t - t0) / (t1 - t0) };
                [
                    p0[0] + (p1[0] - p0[0]) * f,
                    p0[1] + (p1[1] - p0[1]) * f,
                    p0[2] + (p1[2] - p0[2]) * f,
                ]
            };
            // every waypoint, with a world position however it is placed
            let mut wps: Vec<(String, [f64; 3], String)> = Vec::new();
            for b in &m.blocks {
                let tag = match &b.waypoint_tag {
                    Some(t) => t.clone(),
                    None => continue,
                };
                let p = match b.free_pos {
                    Some(p) => [p[0] as f64, p[1] as f64, p[2] as f64],
                    None => {
                        let (cx, cy, cz) = b.coords();
                        [
                            cx as f64 * 32.0 + 16.0,
                            cy as f64 * 8.0 - 62.0,
                            cz as f64 * 32.0 + 16.0,
                        ]
                    }
                };
                wps.push((format!("block#{}", b.index), p, tag));
            }
            for it in &m.items {
                let tag = match &it.waypoint_tag {
                    Some(t) => t.clone(),
                    None => continue,
                };
                wps.push((
                    format!("item#{}", it.index),
                    [it.pos[0] as f64, it.pos[1] as f64, it.pos[2] as f64],
                    tag,
                ));
            }
            println!("split_s\tcar_x\tcar_y\tcar_z\twaypoint\ttag\tdist_m\trunner_up_m");
            for s in &splits {
                let c = at(*s);
                let mut d: Vec<(f64, usize)> = wps
                    .iter()
                    .enumerate()
                    .map(|(i, (_, p, _))| {
                        let dx = p[0] - c[0];
                        let dy = p[1] - c[1];
                        let dz = p[2] - c[2];
                        ((dx * dx + dy * dy + dz * dz).sqrt(), i)
                    })
                    .collect();
                d.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());
                let (d0, i0) = d[0];
                let d1 = d.get(1).map(|x| x.0).unwrap_or(f64::INFINITY);
                println!(
                    "{}\t{:.1}\t{:.1}\t{:.1}\t{}\t{}\t{:.1}\t{:.1}",
                    secs::ms(*s as i64), c[0], c[1], c[2], wps[i0].0, wps[i0].2, d0, d1
                );
            }
        }
        "rungspec" => {
            // `prs`: emit a `ladder` spec that places a waypoint block ON the
            // reference ghost's own trajectory at a set of known times.
            //
            // This is `ACQUISITION_addendum_vj_gate_trigger_geometry_v1` §1 --
            // "place the gate exactly on the reference ghost's own trajectory
            // at 8-12 known times and check that each fires at that time" --
            // as a command instead of a per-map hand calculation. Because the
            // predicted answer for rung t IS t, the sweep can fail: a rung
            // that fires late, early, or not at all is telling you something
            // about the trigger, not about the tape.
            //
            //   tmmaps rungspec TRAJ.csv --block N --times a,b,c
            //                   [--from MS --to MS --step MS]
            //                   [--offset DX,DY,DZ] [--also B2]
            //
            // --offset shifts every rung by a fixed vector, which is how you
            // calibrate a gate whose origin is not where its trigger is (a
            // gate anchored at the road surface under a car 14 m above it).
            let csv = std::fs::read_to_string(&args[2]).expect("read trajectory csv");
            // A String, not a usize: a rung on THIS map is the Goal ITEM, spelled
            // `i0`, and the movers already understand that spelling. Parsing it as a
            // number made every item-gate ladder impossible to express.
            let bidx: String = flag(&args, "--block").expect("--block N (or iN for an item)").to_string();
            let also: Option<String> = flag(&args, "--also").map(|s| s.to_string());
            let off: [f32; 3] = match flag(&args, "--offset") {
                Some(s) => {
                    let v: Vec<f32> = s.split(',').map(|x| x.trim().parse().unwrap()).collect();
                    [v[0], v[1], v[2]]
                }
                None => [0.0, 0.0, 0.0],
            };
            let also_off: [f32; 3] = match flag(&args, "--also-offset") {
                Some(s) => {
                    let v: Vec<f32> = s.split(',').map(|x| x.trim().parse().unwrap()).collect();
                    [v[0], v[1], v[2]]
                }
                None => [0.0, 0.0, 0.0],
            };
            // time_ms,x,y,z,... -- the first four columns of tmtraj's CSV
            let mut traj: Vec<(f64, [f64; 3])> = Vec::new();
            for (i, line) in csv.lines().enumerate() {
                if i == 0 {
                    continue;
                }
                let f: Vec<&str> = line.split(',').collect();
                if f.len() < 4 {
                    continue;
                }
                traj.push((
                    f[0].parse().unwrap(),
                    [f[1].parse().unwrap(), f[2].parse().unwrap(), f[3].parse().unwrap()],
                ));
            }
            assert!(traj.len() > 1, "trajectory has {} samples", traj.len());
            let at = |t: f64| -> [f64; 3] {
                // linear interpolation between the bracketing samples; the
                // decoder's period is 50 ms and the car moves up to 7 m in one,
                // so interpolating is not optional at racing speed.
                let j = traj.partition_point(|(ts, _)| *ts <= t).max(1).min(traj.len() - 1);
                let (t0, p0) = traj[j - 1];
                let (t1, p1) = traj[j];
                let f = if (t1 - t0).abs() < 1e-9 { 0.0 } else { (t - t0) / (t1 - t0) };
                [
                    p0[0] + (p1[0] - p0[0]) * f,
                    p0[1] + (p1[1] - p0[1]) * f,
                    p0[2] + (p1[2] - p0[2]) * f,
                ]
            };
            let mut times: Vec<f64> = Vec::new();
            if let Some(s) = flag(&args, "--times") {
                times.extend(s.split(',').map(|x| x.trim().parse::<f64>().unwrap()));
            }
            if let (Some(a), Some(b)) = (flag(&args, "--from"), flag(&args, "--to")) {
                let a: f64 = a.parse().unwrap();
                let b: f64 = b.parse().unwrap();
                let st: f64 = flag(&args, "--step").unwrap_or("250").parse().unwrap();
                let mut t = a;
                while t <= b + 1e-9 {
                    times.push(t);
                    t += st;
                }
            }
            assert!(!times.is_empty(), "give --times or --from/--to[/--step]");
            println!("# rungspec from {} block#{}", args[2], bidx);
            println!("# predicted fire time for each rung IS its own t (ms), minus the nose lead");
            // --cells: emit GRID-CELL moves and a CURTAIN, instead of one free
            // placement.
            //
            // Two facts force this and both were measured on 134672. (1) Every
            // waypoint on that map is a GRID block, and a free `@x,y,z` move of
            // one is refused by the mover — correctly, because a grid block
            // ignores a position. So a rung there can only be a cell. (2) A
            // single 32 m cell is SILENT for a large fraction of placements: a
            // gate put in the cell the reference is standing in at 8.000 s did
            // not fire for any of four tapes, while a four-gate curtain over
            // the cells it passes through fired for all four, within 0.2 s of
            // each other.
            //
            // The y cell cannot be computed from the car's height: waypoints on
            // one map sit at cell y 13 under cars at 51-59 m and at 14 under a
            // car at 52.5 m, because the gate's own anchor is at a different
            // height inside the block for each gate model. So the curtain
            // spans the y cells around the car and lets the ladder's own
            // fire-time check say which ones were real.
            let cells = args.iter().any(|a| a == "--cells");
            // Block designators, not numbers: `iN` names an item, so a curtain
            // entry is a string exactly like `--block` is.
            let curtain: Vec<String> = match flag(&args, "--curtain") {
                Some(s) => s.split(',').map(|x| x.trim().to_string()).collect(),
                None => vec![bidx.clone()],
            };
            let win: f64 = flag(&args, "--window").map(|s| s.parse().unwrap()).unwrap_or(400.0);
            if cells {
                for t in &times {
                    // the distinct (x, z) cells the reference occupies from
                    // this rung's time to `win` ms later -- the corridor the
                    // curtain has to close off
                    let mut xz: Vec<(i64, i64)> = Vec::new();
                    let mut ys: Vec<i64> = Vec::new();
                    let mut u = *t;
                    while u <= *t + win {
                        let p = at(u);
                        let c = ((p[0] / 32.0).floor() as i64, (p[2] / 32.0).floor() as i64);
                        if !xz.contains(&c) {
                            xz.push(c);
                        }
                        let yc = (p[1] / 8.0).floor() as i64;
                        for d in [6i64, 7, 8] {
                            if !ys.contains(&(yc + d)) {
                                ys.push(yc + d);
                            }
                        }
                        u += 50.0;
                    }
                    let mut moves: Vec<String> = Vec::new();
                    'outer: for (cx, cz) in &xz {
                        for cy in &ys {
                            if moves.len() >= curtain.len() {
                                break 'outer;
                            }
                            moves.push(format!("{}:{},{},{}", curtain[moves.len()], cx, cy, cz));
                        }
                    }
                    println!("{}   # t={:.0}", moves.join(" "), t);
                }
                return;
            }
            for t in times {
                let p = at(t);
                let mut line = format!(
                    "{}@{:.3},{:.3},{:.3}",
                    bidx,
                    p[0] as f32 + off[0],
                    p[1] as f32 + off[1],
                    p[2] as f32 + off[2]
                );
                if let Some(b2) = &also {
                    line.push_str(&format!(
                        " {}@{:.3},{:.3},{:.3}",
                        b2,
                        p[0] as f32 + also_off[0],
                        p[1] as f32 + also_off[1],
                        p[2] as f32 + also_off[2]
                    ));
                }
                println!("{}   # t={:.0}", line, t);
            }
        }
        "origin" => controls::cmd_origin(&args),
        // `tmmaps setuid MAP --out F [--uid U]`: the map with a fresh (or the
        // given) UID. The game caches a map's computed lightmap and its
        // embedded items by UID: a rebuilt test copy under the published UID
        // plays with the OLD build's baked light (Summer 09's start deck read
        // dark in play mode until this, 2026-09-07).
        "stripghost" => {
            // `tmmaps stripghost MAP --out F`: drop the author's validation ghost
            // (the ORIGINAL map's full-size run, replayed over the tiny map as a
            // car driving in the air) and mark the map unvalidated. See
            // `MapFile::strip_validation_ghost`.
            let path = std::path::Path::new(&args[2]);
            let out = args.iter().position(|a| a == "--out").map(|i| args[i + 1].clone()).expect("--out F");
            let mut m = map::MapFile::load(path);
            let removed = m.strip_validation_ghost();
            if removed == 0 {
                println!("{}: no validation ghost chunk — nothing to strip", path.display());
                std::process::exit(1);
            }
            m.write_to(std::path::Path::new(&out)).expect("write");
            println!("{}: validation ghost dropped ({removed} bytes), header validated=\"0\" -> {out}", path.display());
        }
        "setuid" => {
            let src = std::path::PathBuf::from(&args[2]);
            let out = std::path::PathBuf::from(tmmaps::cli::flag(&args, "--out").expect("setuid needs --out MAP"));
            let uid = tmmaps::cli::flag(&args, "--uid").map(String::from).unwrap_or_else(|| {
                let nanos = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.subsec_nanos()).unwrap_or(0);
                format!("Tst1{:08X}{:07}{:08X}", nanos % 100_000_000, std::process::id() % 10_000_000, (nanos / 7) % 100_000_000)
            });
            let mut m = tmmaps::map::MapFile::load(&src);
            m.set_map_uid(&uid);
            m.write_to(&out).expect("write output");
            println!("wrote {} with uid {uid}", out.display());
        }
        // `tmmaps delblocks MAP --out F [--keep-baked Sea,…] [--strip-lightmap]`: every
        // authored block deleted (the generated ones too, but for --keep-baked),
        // items kept — the 0-block form of `tmmaps tiny` on ANY map, for the
        // lightmapper-crash bisect of 2026-09-07
        "delblocks" => {
            let src = std::path::PathBuf::from(&args[2]);
            let out = std::path::PathBuf::from(tmmaps::cli::flag(&args, "--out").expect("delblocks needs --out MAP"));
            let keep: std::collections::BTreeSet<String> = tmmaps::cli::flag(&args, "--keep-baked").unwrap_or("").split(',').filter(|s| !s.is_empty()).map(|s| s.to_string()).collect();
            // --keep-first N: the first N authored blocks stay (the "does ONE block suffice" probe)
            let keep_first: usize = tmmaps::cli::flag(&args, "--keep-first").and_then(|v| v.parse().ok()).unwrap_or(0);
            let mut m = tmmaps::map::MapFile::load(&src);
            let (nb, nk) = (m.blocks.len(), m.baked.len());
            let r = m.remove_blocks(|b| b.index >= keep_first, |b| !keep.contains(&b.name));
            println!("deleted {} of {nb} authored and {} of {nk} generated blocks; {} free entries, {} snap groups ({} items un-snapped)", r.blocks, r.baked, r.free_entries, r.snap_groups, r.snapped_items_cleared);
            let tmp = out.with_extension("del0.Map.Gbx");
            m.write_to(&tmp).expect("write");
            let mut m = tmmaps::map::MapFile::load(&tmp);
            if tmmaps::cli::has(&args, "--strip-lightmap") {
                println!("lightmap stripped ({} bytes)", m.strip_lightmap());
            }
            m.write_to(&out).expect("write output");
            let _ = std::fs::remove_file(&tmp);
            println!("wrote {} ({} blocks, {} items)", out.display(), m.blocks.len(), m.items.len());
        }
        "census" => census::cmd_census(&args),
        "fillers" => tmmaps::fillers::cmd(&args),
        // archive-only variants of a tiny map for the load-failure bisect (zippad.rs)
        "zippad" => tmmaps::zippad::cmd(&args),
        "header" => header::cmd(&args),
        "dropscan" => dropscan::cmd(&args),
        "mediatracker" => tmmaps::mediatracker::cmd(&args),
        "chunks" => {
            // Every skippable chunk in the body, with its size. Needed to
            // reason about FREE blocks (0x0304305F) and to tell at a glance
            // whether a map even has a baked-blocks chunk (0x03043048).
            // `--only 0x0304305D --hex N` dumps the head of one chunk's payload.
            let g = gbx::Gbx::load(Path::new(&args[2])).unwrap();
            let only: Option<u32> = flag(&args, "--only").map(|s| u32::from_str_radix(s.trim_start_matches("0x").trim_start_matches("0X"), 16).expect("--only CHUNKID (hex)"));
            let hex: usize = flag(&args, "--hex").and_then(|s| s.parse().ok()).unwrap_or(0);
            // `--at OFF --hex N`: the decompressed body at an absolute offset
            // (the non-skippable chunks -- the MediaTracker 0x03043049 -- have
            // no PIKS header and never appear in the table below).
            if let Some(at) = flag(&args, "--at").and_then(|s| s.parse::<usize>().ok()) {
                let end = (at + hex.max(256)).min(g.body.len());
                println!("body {} bytes; {at}..{end}:", g.body.len());
                for (i, row) in g.body[at..end].chunks(16).enumerate() {
                    println!("  {:08x}: {:<48} {}", at + i * 16, row.iter().map(|x| format!("{x:02x}")).collect::<Vec<_>>().join(" "), row.iter().map(|x| if x.is_ascii_graphic() { *x as char } else { '.' }).collect::<String>());
                }
                return;
            }
            println!("chunk\toff\tpayload\tsize");
            for (cid, off, payload, size) in map::skip_chunks(&g.body) {
                if only.map(|c| c != cid).unwrap_or(false) {
                    continue;
                }
                println!("0x{:08X}\t{}\t{}\t{}", cid, off, payload, size);
                if has(&args, "--words") {
                    // the payload as little-endian i32 words, one per line (for a histogram)
                    for w in g.body[payload..payload + size - size % 4].chunks(4) {
                        println!("{}", i32::from_le_bytes(w.try_into().unwrap()));
                    }
                }
                if hex > 0 {
                    let b = &g.body[payload..payload + size.min(hex)];
                    for (i, row) in b.chunks(16).enumerate() {
                        println!("  {:06x}: {:<48} {}", i * 16, row.iter().map(|x| format!("{x:02x}")).collect::<Vec<_>>().join(" "), row.iter().map(|x| if x.is_ascii_graphic() { *x as char } else { '.' }).collect::<String>());
                    }
                }
            }
        }
        "blockrefs" => {
            // Everything in the file that lists blocks by INDEX or per block,
            // against the parsed block and item counts — the audit behind
            // `MapFile::remove_blocks` (2026-09-07): the snapped-on tables of
            // 0x03043040, the free-block entries of 0x0304305F, the per-block
            // bytes of 0x03043062/0x03043068, the macroblock refs of 0x03043069.
            let m = map::MapFile::load(Path::new(&args[2]));
            let nb = m.blocks.len();
            let nk = m.baked.len();
            let ni = m.items.len();
            let placeholders = m.blocks.iter().filter(|b| b.flags == 0xFFFF_FFFF).count();
            println!("blocks {nb} (placeholders {placeholders})  baked {nk}  items {ni}  lookback slots {}", m.body_ids.iter().filter(|f| f.is_def).count());
            println!("block records {:?} ({} bytes), baked records {:?}", m.blocks_records, m.blocks_records.1 - m.blocks_records.0, m.baked_records);
            let free_b = m.blocks.iter().filter(|b| b.free_off.is_some()).count();
            let free_k = m.baked.iter().filter(|b| b.free_off.is_some()).count();
            println!("free blocks {free_b} + free baked {free_k} (0x0304305F entries)");
            let chunks = map::skip_chunks(&m.gbx.body);
            for cid in [0x0304_3062u32, 0x0304_3068] {
                match chunks.iter().find(|(c, ..)| *c == cid) {
                    Some(&(_, _, _, size)) => println!("chunk {cid:#010x}: {size} bytes = 4 + {nb} + {nk} + {ni} -> {}", if size == 4 + nb + nk + ni { "ok" } else { "MISMATCH" }),
                    None => println!("chunk {cid:#010x}: absent"),
                }
            }
            match m.macroblock_refs() {
                Some(mb) => println!("chunk 0x03043069: {} bytes; blocks with a macroblock ref {}, items {}, tail {} bytes", mb.size, mb.blocks.iter().filter(|v| **v != -1).count(), mb.items.iter().filter(|v| **v != -1).count(), mb.tail.len()),
                None => println!("chunk 0x03043069: absent"),
            }
            // the removal re-serialises both block chunks: with nothing dropped it must reproduce the body
            {
                let mut same = map::MapFile::load(Path::new(&args[2]));
                same.remove_blocks(|_| false, |_| false);
                let body = same.patched_body();
                match body.iter().zip(&m.gbx.body).position(|(a, b)| a != b) {
                    None if body.len() == m.gbx.body.len() => println!("no-op block rewrite: byte-identical"),
                    first => println!("no-op block rewrite: DIFFERS ({} -> {} bytes, first at {:?})", m.gbx.body.len(), body.len(), first),
                }
            }
            match chunks.iter().find(|(c, ..)| *c == 0x0304_305D) {
                Some(&(_, _, payload, size)) => match map::octree_chunk_summary(&m.gbx.body[payload..payload + size]) {
                    Ok(None) => println!("chunk 0x0304305D: empty (no tree)"),
                    Ok(Some((grid, origin, n, hist))) => println!("chunk 0x0304305D: octree grid {grid} origin {origin:?}, {n} nodes, leaf flags {hist:?} — node indices only, no block refs"),
                    Err(e) => println!("chunk 0x0304305D: {size} bytes, NOT the octree layout: {e}"),
                },
                None => println!("chunk 0x0304305D: absent"),
            }
            match m.snap_tables() {
                Some(st) => {
                    let bg = st.block_indexes.iter().filter(|v| **v != -1).count();
                    let tagged = st.block_indexes.iter().filter(|v| **v != -1 && (**v as u32) >> 24 != 0).count();
                    let ig = st.item_indexes.iter().filter(|v| **v >= 0).count();
                    let both = st.block_indexes.iter().zip(&st.item_indexes).filter(|(b, i)| **b != -1 && **i >= 0).count();
                    let snapped = st.snapped.iter().filter(|v| **v >= 0).count();
                    println!("chunk 0x03043040 snapped-on tables: {} groups ({bg} name a block — {tagged} with a tag byte —, {ig} an item, {both} both), snap_groups {:?}.., u07 all -1: {}, {snapped} of {} items snapped", st.block_indexes.len(), st.snap_groups.iter().take(6).collect::<Vec<_>>(), st.u07.iter().all(|v| *v == -1), st.snapped.len());
                    if has(&args, "--groups") {
                        for k in 0..st.block_indexes.len() {
                            let users: Vec<usize> = st.snapped.iter().enumerate().filter(|(_, s)| **s == k as i32).map(|(i, _)| i).collect();
                            let target = if st.block_indexes[k] != -1 { let w = st.block_indexes[k] as u32; let idx = (w & 0x00FF_FFFF) as usize; format!("block {idx}{} {}", if w >> 24 != 0 { format!(" (tag {:#04x})", w >> 24) } else { String::new() }, m.blocks.get(idx).map(|b| format!("{} {:?}", b.name, b.coords())).unwrap_or("?".into())) } else { format!("item {} {}", st.item_indexes[k], m.items.get(st.item_indexes[k] as usize).map(|i| i.model.as_str()).unwrap_or("?")) };
                            println!("  group {k}: {target} group {} <- items {:?}", st.snap_groups[k], users);
                        }
                    }
                }
                None => println!("chunk 0x03043040 snapped-on tables: none"),
            }
        }
        "genealogy" => {
            // Chunk 0x03043043 (terrain zone genealogies): version, inner
            // buffer length, record count, then the records — a hex dump of
            // the head to read the record layout off.
            let g = gbx::Gbx::load(Path::new(&args[2])).unwrap();
            let &(_, _, payload, size) = map::skip_chunks(&g.body).iter().find(|(c, ..)| *c == 0x0304_3043).expect("no genealogy chunk");
            let b = &g.body[payload..payload + size];
            println!("version {} buffer {} count {}", u32::from_le_bytes(b[0..4].try_into().unwrap()), u32::from_le_bytes(b[4..8].try_into().unwrap()), u32::from_le_bytes(b[8..12].try_into().unwrap()));
            match map::genealogy_full(b) {
                Ok(recs) => {
                    let mut hist = std::collections::BTreeMap::new();
                    for r in &recs { *hist.entry(r.current.clone()).or_insert(0) += 1; }
                    println!("zones: {hist:?}");
                    // --chains: the distinct full records (zone chain, current
                    // index, direction) with their counts — what the game
                    // regenerates per cell, not just the current zone's name
                    if tmmaps::cli::has(&args, "--chains") {
                        let mut chains: std::collections::BTreeMap<String, usize> = Default::default();
                        for r in &recs { *chains.entry(r.describe()).or_insert(0) += 1; }
                        let mut rows: Vec<_> = chains.into_iter().collect();
                        rows.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
                        println!("{} distinct records:", rows.len());
                        for (chain, n) in rows { println!("  {n:5}  {chain}"); }
                    }
                    if tmmaps::cli::has(&args, "--grid") {
                        // 64 rows of 64 first letters, record order
                        for row in recs.chunks(64) {
                            println!("{}", row.iter().map(|r| r.current.chars().next().unwrap_or('.')).collect::<String>());
                        }
                    }
                }
                Err(e) => println!("records: {e}"),
            }
            let n: usize = tmmaps::cli::flag(&args, "--bytes").and_then(|s| s.parse().ok()).unwrap_or(256);
            for (i, row) in b[12..(12 + n).min(b.len())].chunks(16).enumerate() {
                println!("{:06x}: {}  {}", 12 + i * 16, row.iter().map(|x| format!("{x:02x}")).collect::<Vec<_>>().join(" "), row.iter().map(|x| if x.is_ascii_graphic() { *x as char } else { '.' }).collect::<String>());
            }
        }
        "colors" => {
            // Chunk 0x03043062: version, then one colour byte per block
            // (unbaked, then baked) and per item — 0 Default, 1 White, 2 Green,
            // 3 Blue, 4 Red, 5 Black. Prints a histogram and, with --filter,
            // the colour of every matching block.
            let m = map::MapFile::load(Path::new(&args[2]));
            let chunks = map::skip_chunks(&m.gbx.body);
            let &(_, _, payload, size) = chunks.iter().find(|(c, ..)| *c == 0x0304_3062).expect("no colour chunk");
            let bytes = &m.gbx.body[payload + 4..payload + size];
            let nb = m.blocks.len();
            let nbaked = m.baked.len();
            let ni = m.items.len();
            eprintln!("{} colour bytes for {} blocks + {} baked + {} items", bytes.len(), nb, nbaked, ni);
            let filter = tmmaps::cli::flag(&args, "--filter");
            let mut hist = std::collections::BTreeMap::new();
            for (i, b) in m.blocks.iter().enumerate() {
                let c = bytes.get(i).copied().unwrap_or(255);
                *hist.entry(("block", c)).or_insert(0) += 1;
                if filter.as_deref().map(|f| b.name.contains(f)).unwrap_or(false) {
                    println!("block\t{}\t{}\tcolor {}", i, b.name, c);
                }
            }
            for (i, it) in m.items.iter().enumerate() {
                let c = bytes.get(nb + nbaked + i).copied().unwrap_or(255);
                *hist.entry(("item", c)).or_insert(0) += 1;
                if filter.as_deref().map(|f| it.model.contains(f)).unwrap_or(false) {
                    println!("item\t{}\t{}\tcolor {}", i, it.model, c);
                }
            }
            for (i, b) in m.baked.iter().enumerate() {
                let c = bytes.get(nb + i).copied().unwrap_or(255);
                *hist.entry(("baked", c)).or_insert(0) += 1;
                if filter.as_deref().map(|f| b.name.contains(f)).unwrap_or(false) {
                    println!("baked\tb{}\t{}\tcolor {}", i, b.name, c);
                }
            }
            for ((k, c), n) in hist {
                println!("{k}\tcolor {c}\t{n}");
            }
        }
        "help" | "--help" | "-h" => println!("{}", USAGE),
        other => {
            eprintln!("tmmaps: unknown subcommand `{}`\n\n{}", other, USAGE);
            std::process::exit(2);
        }
    }
}

const USAGE: &str = r#"tmmaps — TM2020 map surgery. `tools/ghost` owns ghosts and replays.

Times print as seconds with a decimal (16.316), never as raw milliseconds.

READING A MAP
  tmmaps waypoints MAP
        the map's waypoints: spawn, checkpoints, goal — block# / item# indices,
        tags, cells, free positions. These indices are what every mover takes.
  tmmaps census MAP [--filter PAT] [--free]
        EVERY block, unbaked (0x0304301F) and BAKED (0x03043048), tagged U/B,
        with its free-block position when it has one, as TSV. `waypoints` and
        any single-chunk listing show only one of the two: across the store
        52.7 % of blocks are baked, and eleven maps read as near-empty without
        this (197047 showed 3 blocks of 2316). A baked index is counted in its
        OWN list, so a bare `2461` pasted from a census row addresses an
        unrelated unbaked block — movers spell baked indices `bN` and REFUSE
        them rather than moving the wrong block.
  tmmaps fillers MAP [--filter PAT] [--cells X0,Z0:X1,Z1] [--summary]
        the generated clip fillers (the game's own pillar walls, skirts, end
        caps: baked records) with their variant word (a0 Middle .. a4 nothing,
        g1 TopBottom_Ground), the side they stand on, what their own cell holds
        (`-` free, `P:` pillars only) and what stands across the side;
        --summary tallies free / pillar / occupied cells per name and variant
  tmmaps region MAP --box X0,Y0,Z0:X1,Y1,Z1 [--filter PAT] [--items] [--blocks]
        everything whose position lies inside a world box. A GATE IS A
        STRUCTURE, NOT A BLOCK: run this before and after any move.
  tmmaps chunks MAP [--only 0xCHUNK --hex N]
        every skippable body chunk with its size (--only/--hex: one chunk, head dump)
  tmmaps blockrefs MAP [--groups]
        every place the file lists blocks by index or per block (the snapped-on
        tables, free-block entries, colour/lightmap bytes, macroblock refs) —
        the audit behind `remove_blocks`
  tmmaps header MAP [MAP ...] [--tsv] [--xml] [--names]
        what the file DECLARES about itself before any block is read: container
        version, node count, EXTERNAL references, the header chunk table, the
        community XML (title, exever/exebuild, envir, maptype, validated), the
        declared `<dep>` files, the embedded-objects zip and its entries, and
        the block/item counts to set them against. `--tsv` is the corpus form —
        one row per map plus the distinct value of every column, because a
        difference only one map has is a lead and one several share is not.
        Written for 146612, which the engine loads and the editor will not open.
        `--names` is the IDENTITY form — `path uid name authorid authortime`,
        one row per map — for auditing what this repo publishes a map as
        against what the map calls itself. Join it on uid against
        trackmania.io; our own documents are not an independent check.

TINY MAPS (half-scale campaign: every authored block/item -> an embedded static item)
  tmmaps tiny MAP --mapping placements.tsv --library ITEMS.zip --out F [--scale 0.5]
      [--anchor x,y,z] [--host HOST.Map.Gbx] [--name NAME | --keep-name] [--keep-zone-block]
        replace every authored block by its library item (mapping rows
        `@index<TAB>ITEM|-<TAB>model_scale<TAB>sx<TAB>sz`; `-` = intentionally
        nothing) and re-point/drop items (`i@index<TAB>ITEM|stock model|-`);
        the baked foundation stays. Inputs come from `mapgeom tiny-library`.
  tmmaps tiny-batch DIR --out DIR [--mapgeom BIN --paks "--pak F:HASH …"] [same flags]
                                                    every .Map.Gbx of a directory; with --mapgeom each map gets
                                                    its own item library (OUT/NAME.lib.zip, .placements.tsv, .report.tsv)
  tmmaps lineup MAP --out F --stock A,B,C --at X,Y,Z [--pitch 16] [--yaw R] [--items F.Item.Gbx,G.Item.Gbx] [--colors 2,3,…]
        the map plus a row of stock (pack) items by name — a vegetation species survey;
        --items continues the row with embedded item files (a test item next to its stock oracle);
        --colors gives each item of the row its placement colour byte (a stock flag at Green next to ours)
  tmmaps shared-cells MAP [--all] [--mapping placements.tsv]
        cells where a terrain tile shares its cell with another block, and whether the
        tile is hidden by that block in the tiny map (kept = a coplanar pair to watch)
  tmmaps tiny-catalog MAP --mapping T --library Z --out F [--only NAME] [--lineup A,B]
        one block per model beside its items (or the listed item files), for a look

CHANGING A MAP — position and ROTATION; no model swap, so no trigger volume changes
  tmmaps move MAP --out F --move SPEC [--move SPEC ...]
        SPEC is  N:cx,cy,cz[:dir]   a grid block, by world cell
                 N@x,y,z[/yaw]      a FREE block, in metres
                 iN@x,y,z[/yaw]     an item
                 bN                 a baked index — always refused, by name
        --cell is correct for either placement regime; --pos/@ is metres and is
        the only correct form for a free block. A free block ignores its cell
        bytes (its position is six f32 in chunk 0x0304305F), so a regime-blind
        cell write produces a map that loads, an origin control that passes,
        and a ladder in which every rung is silent.
  tmmaps rotate MAP --out F --rot BLK:yaw,pitch,roll [...]
                          --drot BLK:dyaw,dpitch,droll [...]
                          --tilt N,N,N --about X,Y,Z --dir DEG --angle RAD
        Tilt FREE blocks. A block's stored rotation turns it about ITS OWN
        ANCHOR, so giving every tile of a surface the same roll SHEARS it into a
        staircase (32 m tiles at 3.4 deg = a 1.9 m step per join, measured). The
        `--tilt` form is the honest one: one axis, position and rotation written
        together. REFUSES when a free block within --group-radius (4 m) is not in
        the rotation -- the ice kicker of 284238 is FOUR blocks sharing an anchor
        and two arms rotated one of them, which reads exactly like a null result.
  tmmaps shift MAP --out F --box X0,Y0,Z0:X1,Y1,Z1 --by DX,DY,DZ [--filter PAT]
        DISPLACE everything in the box, then re-read the written map and require
        each object to be exactly where it was sent. This is how you measure a
        clearance: move an obstacle by known amounts and ask the oracle which
        is the first that lets a tape through. A structure half-moved reads as
        a measurement, so nothing is half-moved.
  tmmaps clear MAP --out F --box X0,Y0,Z0:X1,Y1,Z1 --to X,Y,Z [--filter PAT]
        move EVERYTHING in the box, then re-read the written map and REQUIRE
        the box to be empty. This is the enforced form of the lesson below.
  tmmaps segat MAP --out F --promote W [--neutralise W,W,...] [--force-rename]
        ONE segment map, spelled out: W becomes the finish and each --neutralise
        waypoint stops being a checkpoint. Nothing is inferred and nothing is
        verified -- the caller owns the comparison. This is the only way to cut
        at a LINKED checkpoint (`segments` cannot enumerate one). Rules: every
        checkpoint at or after the cut must be neutralised, AND every other
        member of the cut's own linked group; a linked group EARLIER than the
        cut is left alone.
  tmmaps segments MAP --ref-ghost G [--out DIR] [--order W,W,...] [-j N]
        measure the checkpoint order, then build every segment map + a control.
        The order is MEASURED against the reference ghost and every round is
        checked against the ghost's own declared splits: the tool REFUSES, with
        the probe table, rather than reporting an order it cannot establish.
        Every built segment is re-validated against the ghost and must
        reproduce that checkpoint's declared split — exactly for a gate cut,
        early by <= 0.500 s for the block-rename fallback — and no two segment
        maps may share a decompressed body. Exit 2 on any refusal.
        --order gives the driving order instead of measuring it, as waypoint
        indices in driving order (`--order 439,494,440,633,492`); spell it
        `i439` / `b2089` when block and item indices collide.

MEASURING WITH A MAP
  tmmaps dropscan MAP --out DIR --tape GHOST (--cells CX:CX:S,CY,CZ:CZ:S | --cell
                  CX,CY,CZ[,DIR] | --tapes DIR) [--dirs D,..] [--target X,Y,Z]
                  [--jobs N] [--at frac:F,..] [--fk PATH] [--server DIR] [--keep]
        READ THE ENVIRONMENT'S GEOMETRY WITH THE CAR. Moves the spawn to a cell,
        drives the car off it under a fixed straight-throttle tape, and reads
        the landing out of the live engine (`fk trace`): one probe is one map
        plus one trace, and the summary is apex, resting place, closest approach
        to --target, and when. This is how you ask "what is over there" on a map
        whose surfaces belong to the DECORATION and not to the map file.
        --tapes DIR instead scores a POPULATION OF TAPES on the untouched map,
        with the same readout — the poor man's state objective.
        Controls, both refusals: the spawn moved to its OWN cell must reproduce
        the untouched map byte-for-byte, and a probe at the real spawn cell must
        start where the map's spawn is. A trace that is all zeroes is REFUSED:
        it passes fk's self-check and means the cell produced no car at all
        (every cell outside the map grid does).
  tmmaps ladder MAP --spec F --ghosts G... [-j N]
        arrival-time ladders. One rung per spec line, whitespace-separated
        moves, so a rung is a CURTAIN of gates across the corridor rather than
        one 32 m cell: a single-cell rung is silent for roughly a third of
        well-chosen placements. Aborts unless a rebuild at every gate's own
        origin reproduces the untouched map, and asserts N rungs produced N
        distinct file hashes.
  tmmaps rungspec TRAJ.csv --block N --from A --to B --step S [--offset dx,dy,dz]
        emit a ladder spec placing a gate ON a reference trajectory
  tmmaps oracle --map M --ghosts G... [--map M2 --ghosts ...] [--shard] [-j N]
        validate (map, ghosts) batches; one server per map, as required —
        every segment map keeps the original mapUid, so two of them can never
        share a UserData/Maps. --shard: one map, ghosts split over -j servers.
  tmmaps cporder MAP TRAJ.csv --splits A,B,C
        which waypoint produced which declared split, from one trajectory
        decode; reports match distance and runner-up so a bad match is visible

CONTROLS — run these before you trust a map-surgery result
  tmmaps selftest [--engine] [--strict]
        the whole suite in one command. --engine adds the dedicated server.
  tmmaps bodydiff STOCK EDITED
        what an edit changed, on the DECOMPRESSED bodies, attributed to the
        placement each differing byte belongs to — plus how much of the file
        itself the two share. A SPLICED edit differs in the bytes of the edit
        and nowhere else; a re-emitted map shares the header and then nothing.
  tmmaps rewrite MAP --out F [--reemit]
        write a map back with NO edit. The default output is the stock file
        byte for byte; --reemit rebuilds the compressed stream, which is what
        every map this project wrote before the splice path existed. The pair
        isolates the WRITER from the EDIT — the one question the dedicated
        server cannot be asked, because it accepts both.
  tmmaps roundtrip MAP
        parse and re-emit unchanged; compares DECOMPRESSED bodies (LZO is not
        bit-reproducible, so file hashes are the wrong level)
  tmmaps origin MAP
        return-to-origin at the byte level: every waypoint AND every item
        through its own mover with its own current placement, output required
        byte-identical. No oracle calls. This is what catches a mover writing
        dead bytes.
  tmmaps renamecheck MAP [--ghosts G...]
        the RENAME round-trip, which roundtrip cannot be: renames a waypoint
        four ways — including to ITSELF with byte-identical output required —
        forcing the two-region Id stream through the rename re-encoder.
        --ghosts also renames an off-route decoration and requires the control
        ghosts' times to be UNCHANGED. Required gate for any rename change.
        A `lookback table length N -> N+1 ... may not resolve` warning is
        CONSERVATIVE: it fires on maps whose game check then passes. Do not
        read it as a failure, and do not read it as an all-clear.

LESSONS THE CODE ENFORCES — see MAPS.md for the full list
  A GATE IS A STRUCTURE, NOT A BLOCK.  Moving GothMommy's added finish on
  173691 moved one unbaked anchor and left FIFTEEN baked `GateExpandable*`
  pieces standing in the landing zone; the run drove into them and stopped, and
  a human spotted it in the video before any instrument did. `region` counts
  what is there and `clear` refuses to succeed while anything is left.

  NEVER PROBE BY SWAPPING A GATE MODEL.  The old `gate` / `gateat` / `probe`
  commands relocated a waypoint by swapping its item model to `GateFinish32m`
  first. On 285885 that quadruples the trigger volume (the origin control then
  returns 50.589 instead of 61.229 — it fabricates discoveries); on 279197 it
  deletes a custom Goal item and everything DNFs. Those commands are deleted.
  Every mover here is position-only. `segments` still promotes a gate, because
  a promoted gate is a fine RULER — it is an unsafe OBJECTIVE.

  A MAP-SURGERY CONTROL CAN BE INERT.  Gate-removed, deck-removed and
  road-removed maps once all returned identical output; the road control proved
  the instrument was dead, not the maps identical. `ladder` requires N distinct
  file hashes for N rungs, and `oracle` refuses a candidate the server would
  silently skip (see below).

  THE SERVER IGNORES A FILE WITHOUT A `.Ghost.Gbx` / `.Replay.Gbx` SUFFIX and
  returns a plain DNF, indistinguishable from a run that did not finish. The
  oracle driver refuses such a path instead of staging it.

env: TMMAPS_DEBUG=1 (lookback table sizes)
     TMMAPS_DEBUG_NODES=1 (trace body node refs as INLINE / backref)
     TMMAPS_NO_BAKED=1 — A LANDMINE. Safe ONLY for position-only surgery: with
     it set the baked chunk is not parsed, so its Id words are not renumbered
     and any RENAME silently mis-encodes the map — and every baked block
     disappears from the census. Nothing has needed it since the
     shared-node-index fix.

exit: 0 ok · 1 a control failed · 2 usage · 3 refused (the command was wrong,
      and the message says what to do instead — never a backtrace)
"#;

