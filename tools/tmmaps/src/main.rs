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
        "waypoints", "census", "region", "colors", "tiny-catalog", "tiny", "tiny-batch", "clear", "shift", "segments", "move", "rotate", "ladder",
        "roundtrip",
        "renamecheck", "cporder", "origin", "chunks",
    ];
    if WANTS_MAP.contains(&cmd) && args.len() < 3 {
        eprintln!("tmmaps {} needs a MAP path.\n\n{}", cmd, USAGE);
        std::process::exit(2);
    }
    match cmd {
        "selftest" => selftest::run(&args),
        "region" => census::cmd_region(&args),
        "tiny-catalog" => tmmaps::tiny::catalog_cmd(&args),
        "tiny" => tmmaps::tiny::cmd(&args),
        "tiny-batch" => tmmaps::tiny::cmd_batch(&args),
        "clear" => census::cmd_clear(&args),
        "shift" => census::cmd_shift(&args),
        "waypoints" => {
            let m = map::MapFile::load(Path::new(&args[2]));
            eprintln!(
                "size={:?} decoration={} blocks={} items={} body_regions={:?} items_region={:?}",
                m.size,
                m.decoration_id,
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
        "untag" => {
            // STOP AN ITEM BEING A WAYPOINT, in place: the anchored object's
            // waypoint property (tag + order) is replaced by the null node, so
            // the engine no longer counts it as a checkpoint the run must
            // collect. Everything else about the placement -- model, position,
            // rotation, the embedded item bytes -- is untouched. Written for
            // the MK64 Koopa cave search maps: a section cut that keeps the
            // course's 39 zone gates has 39 checkpoints the car never passes,
            // so its Finish can never fire (a finish counts only once every
            // checkpoint has been collected). `segat --neutralise` is NOT this:
            // it turns a checkpoint into another finish.
            let src = PathBuf::from(&args[2]);
            let out = PathBuf::from(flag(&args, "--out").expect("--out F"));
            let spec = flag(&args, "--items").expect("--items iN,iN,... (item indices as `tmmaps waypoints` prints them)");
            let mut m = map::MapFile::load(&src);
            let mut n = 0usize;
            for tok in spec.split(',').map(|s| s.trim()).filter(|s| !s.is_empty()) {
                let ii: usize = tok
                    .trim_start_matches('i')
                    .parse()
                    .unwrap_or_else(|_| panic!("--items: {:?} is not an item index (want iN)", tok));
                assert!(ii < m.items.len(), "item#{} does not exist ({} items)", ii, m.items.len());
                let tag = m.items[ii].waypoint_tag.clone();
                assert!(
                    tag.is_some(),
                    "item#{} {} carries no waypoint property -- nothing to remove",
                    ii,
                    m.items[ii].model
                );
                m.set_item_waypoint_tag(ii, None);
                println!("  item#{} {} {:?} -> no waypoint", ii, m.items[ii].model, tag.unwrap());
                n += 1;
            }
            assert!(n > 0, "--items named nothing");
            let sp = m.write_to_reporting(&out).expect("write untagged map");
            println!("wrote {} ({} waypoint properties removed)\n  {}", out.display(), n, sp.summary());
            // READ-BACK CONTROL: the written map must list exactly the
            // waypoints that were not untagged.
            let back = map::MapFile::load(&out);
            let left: Vec<usize> = back
                .items
                .iter()
                .enumerate()
                .filter(|(_, it)| it.waypoint_tag.is_some())
                .map(|(i, _)| i)
                .collect();
            println!("  read-back: {} item waypoint(s) remain: {:?}", left.len(), left);
        }
        "retag" => {
            // Rewrite an item's waypoint TAG in place (order stays 0): the
            // engine's vocabulary is Spawn / Goal / Checkpoint / StartFinish /
            // LinkedCheckpoint, and a cut written with `Start` / `Finish`
            // spawns the car but never fires its finish.
            let src = PathBuf::from(&args[2]);
            let out = PathBuf::from(flag(&args, "--out").expect("--out F"));
            let spec = flag(&args, "--set").expect("--set iN=TAG,iN=TAG,...");
            let mut m = map::MapFile::load(&src);
            let mut n = 0usize;
            for tok in spec.split(',').map(|s| s.trim()).filter(|s| !s.is_empty()) {
                let (a, tag) = tok.split_once('=').unwrap_or_else(|| panic!("--set: {:?} is not iN=TAG", tok));
                let ii: usize = a
                    .trim_start_matches('i')
                    .parse()
                    .unwrap_or_else(|_| panic!("--set: {:?} is not an item index (want iN)", a));
                assert!(ii < m.items.len(), "item#{} does not exist ({} items)", ii, m.items.len());
                let old = m.items[ii].waypoint_tag.clone();
                m.set_item_waypoint_tag(ii, Some(tag));
                println!("  item#{} {} {:?} -> {:?}", ii, m.items[ii].model, old, tag);
                n += 1;
            }
            assert!(n > 0, "--set named nothing");
            let sp = m.write_to_reporting(&out).expect("write retagged map");
            println!("wrote {} ({} tags rewritten)\n  {}", out.display(), n, sp.summary());
            let back = map::MapFile::load(&out);
            for it in back.items.iter().filter(|it| it.waypoint_tag.is_some()) {
                println!("  read-back: item#{} {} tag {:?}", it.index, it.model, it.waypoint_tag.as_deref().unwrap());
            }
        }
        "dropcp" => {
            // TAKE CHECKPOINTS OUT OF A MAP'S REQUIRED SET. The engine decides a
            // landmark's kind from the ITEM MODEL's waypoint type (a Checkpoint
            // model stays a required checkpoint whatever the placement's tag
            // says -- measured on the MK64 Koopa cut: tags rewritten to Goal
            // still counted as checkpoints and the finish never fired), so the
            // only in-place cure is to swap each checkpoint placement onto a
            // model the engine does not require -- a FINISH model -- and park
            // it where no car can cross it. A finish is never required, so the
            // map's real finish then fires on its own.
            let src = PathBuf::from(&args[2]);
            let out = PathBuf::from(flag(&args, "--out").expect("--out F"));
            let spec = flag(&args, "--items").expect("--items iN,iN,...");
            let model = flag(&args, "--model").expect("--model NAME.Item.Gbx (a Finish-type model the map carries)");
            let park: Vec<f32> = flag(&args, "--park")
                .unwrap_or("8,-900,8")
                .split(',')
                .map(|s| s.trim().parse::<f32>().expect("--park X,Y,Z"))
                .collect();
            assert_eq!(park.len(), 3, "--park wants X,Y,Z");
            let mut m = map::MapFile::load(&src);
            // An embedded model must be one the map carries; a STOCK item ident
            // (no `.Item.Gbx`, e.g. `GateFinish32m`) comes from the game's own
            // collection and needs no manifest row -- measured: a Koopa cut with
            // its finish placement swapped onto GateFinish32m validates a tape
            // at 5.882 (its own finish item: 5.834).
            assert!(
                !model.ends_with(".Item.Gbx") || m.items.iter().any(|it| it.model == model),
                "--model {:?} is not a model this map places; the engine could not load it",
                model
            );
            let mut n = 0usize;
            let mut dropped: Vec<usize> = Vec::new();
            for tok in spec.split(',').map(|s| s.trim()).filter(|s| !s.is_empty()) {
                let ii: usize = tok
                    .trim_start_matches('i')
                    .parse()
                    .unwrap_or_else(|_| panic!("--items: {:?} is not an item index (want iN)", tok));
                assert!(ii < m.items.len(), "item#{} does not exist ({} items)", ii, m.items.len());
                let old = m.items[ii].model.clone();
                let home = m.items[ii].pos;
                m.set_item_model(ii, &model);
                m.move_item_pos(ii, [park[0], park[1], park[2]]);
                println!("  item#{} {} at {:?} -> {} parked at {:?}", ii, old, home, model, park);
                dropped.push(ii);
                n += 1;
            }
            assert!(n > 0, "--items named nothing");
            // Two passes: a model rename re-encodes the Id stream and the
            // variable-length tag splice cannot ride the same write, so the
            // renamed+moved map is written, reloaded, and only then untagged.
            let sp1 = m.write_to_reporting(&out).expect("write map (pass 1: models + positions)");
            println!("  pass 1: {}", sp1.summary());
            let mut m = map::MapFile::load(&out);
            for &ii in &dropped {
                m.set_item_waypoint_tag(ii, None);
            }
            let sp = m.write_to_reporting(&out).expect("write map (pass 2: waypoint properties)");
            println!("wrote {} ({} checkpoints dropped)\n  {}", out.display(), n, sp.summary());
            let back = map::MapFile::load(&out);
            for it in back.items.iter().filter(|it| it.waypoint_tag.is_some()) {
                println!("  read-back: item#{} {} tag {:?} at {:?}", it.index, it.model, it.waypoint_tag.as_deref().unwrap(), it.pos);
            }
        }
        "setmodel" => {
            // Point an item placement at another model (and optionally another
            // author), in place. Diagnostic: swap a map's own finish item for
            // the stock `GateFinish32m` to ask the oracle whether the ITEM or
            // the PLACEMENT is why a finish never fires.
            let src = PathBuf::from(&args[2]);
            let out = PathBuf::from(flag(&args, "--out").expect("--out F"));
            let spec = flag(&args, "--set").expect("--set iN=MODEL,...");
            let author = flag(&args, "--author");
            let mut m = map::MapFile::load(&src);
            for tok in spec.split(',').map(|s| s.trim()).filter(|s| !s.is_empty()) {
                let (a, model) = tok.split_once('=').unwrap_or_else(|| panic!("--set: {:?} is not iN=MODEL", tok));
                let ii: usize = a.trim_start_matches('i').parse().unwrap_or_else(|_| panic!("--set: {:?} is not iN", a));
                let old = m.items[ii].model.clone();
                m.set_item_model(ii, model);
                if let Some(au) = author.as_deref() {
                    m.set_item_author(ii, au);
                }
                println!("  item#{} {} -> {} (author {:?})", ii, old, model, author);
            }
            let sp = m.write_to_reporting(&out).expect("write map");
            println!("wrote {}\n  {}", out.display(), sp.summary());
            let back = map::MapFile::load(&out);
            for it in back.items.iter().filter(|it| it.waypoint_tag.is_some()) {
                println!("  read-back: item#{} {} tag {:?} at {:?}", it.index, it.model, it.waypoint_tag.as_deref().unwrap(), it.pos);
            }
        }
        "setblock" => {
            // Rename a block model in place (diagnostic sibling of `setmodel`):
            // e.g. RoadTechStart -> RoadTechMultilap to ask the oracle what a
            // lap-race start does to a map's finish.
            let src = PathBuf::from(&args[2]);
            let out = PathBuf::from(flag(&args, "--out").expect("--out F"));
            let spec = flag(&args, "--set").expect("--set N=NAME,...");
            let mut m = map::MapFile::load(&src);
            for tok in spec.split(',').map(|s| s.trim()).filter(|s| !s.is_empty()) {
                let (a, name) = tok.split_once('=').unwrap_or_else(|| panic!("--set: {:?} is not N=NAME", tok));
                let bi: usize = a.parse().unwrap_or_else(|_| panic!("--set: {:?} is not a block index", a));
                let old = m.blocks[bi].name.clone();
                m.set_block_name(bi, name);
                println!("  block#{} {} -> {}", bi, old, name);
            }
            let sp = m.write_to_reporting(&out).expect("write map");
            println!("wrote {}\n  {}", out.display(), sp.summary());
        }
        "reembed" => {
            // Replace one embedded item model's BYTES inside the map's
            // embedded-objects zip, keeping the manifest and every placement.
            // `--replace NAME=FILE` names the zip entry (as `header` lists it,
            // e.g. Items/Foo.Item.Gbx) and the file whose bytes go in its
            // place. Used to swap the Koopa cut's StartFinish-type start item
            // for the same item with waypoint type Start, so the map stops
            // being a lap race and its Finish can fire.
            let src = PathBuf::from(&args[2]);
            let out = PathBuf::from(flag(&args, "--out").expect("--out F"));
            let spec = flag(&args, "--replace").expect("--replace NAME=FILE[,NAME=FILE]");
            let mut m = map::MapFile::load(&src);
            let zip = header::embedded_zip_bytes(&m.gbx.body).expect("map has no embedded-objects zip");
            let manifest = m.embedded_manifest().expect("map has no readable embedded-objects manifest");
            let mut zip2 = zip.clone();
            for tok in spec.split(',').map(|s| s.trim()).filter(|s| !s.is_empty()) {
                let (name, file) = tok.split_once('=').unwrap_or_else(|| panic!("--replace: {:?} is not NAME=FILE", tok));
                let bytes = std::fs::read(file).unwrap_or_else(|e| panic!("{file}: {e}"));
                let (before, names) = header::embedded_zip(&m.gbx.body).unwrap();
                assert!(names.iter().any(|n| n == name), "{:?} is not an entry of the embedded zip ({} entries, {} bytes)", name, names.len(), before);
                zip2 = header::zip_add(&zip2, name, &bytes);
                println!("  {} <- {} ({} bytes)", name, file, bytes.len());
            }
            let rows: Vec<(&str, &str)> = manifest.iter().map(|(a, b)| (a.as_str(), b.as_str())).collect();
            println!("  manifest: {} item idents kept; zip {} -> {} bytes", rows.len(), zip.len(), zip2.len());
            m.replace_embedded_objects(&rows, &zip2);
            let sp = m.write_to_reporting(&out).expect("write map");
            println!("wrote {}\n  {}", out.display(), sp.summary());
            let back = map::MapFile::load(&out);
            let (zb, zn) = header::embedded_zip(&back.gbx.body).unwrap_or((0, Vec::new()));
            println!("  read-back: zip {} bytes, {} entries, manifest {} rows", zb, zn.len(), back.embedded_manifest().map_or(0, |r| r.len()));
        }
        "pathline" => {
            // A REFERENCE LINE from the exporter's ROM-path CSV
            // (`path,index,section,x,y,z,heading_deg`, world metres): rows of the
            // named path between two indices, then optional appended points,
            // resampled to one row per 10 ms at a constant speed, written as
            // `time_ms,x,y,z` -- the shape `--refcsv` reads. Also prints the
            // arclength of every appended point and of the named gate positions
            // so the legs of a chained gate search can be placed on it.
            let csv = PathBuf::from(&args[2]);
            let out = PathBuf::from(flag(&args, "--out").expect("--out F.csv"));
            let path_name: String = flag(&args, "--path").map(|s| s.to_string()).unwrap_or_else(|| "track_path".to_string());
            let (i0, i1) = {
                let r: String = flag(&args, "--idx").map(|s| s.to_string()).unwrap_or_else(|| "0..".to_string());
                let (a, b) = r.split_once("..").expect("--idx A..B");
                (a.parse::<usize>().unwrap_or(0), b.parse::<usize>().unwrap_or(usize::MAX))
            };
            let speed_ms: f64 = flag(&args, "--speed").map(|s| s.parse().expect("--speed m/s")).unwrap_or(30.0);
            let text = std::fs::read_to_string(&csv).unwrap_or_else(|e| panic!("{}: {e}", csv.display()));
            let mut pts: Vec<[f64; 3]> = Vec::new();
            for line in text.lines().skip(1) {
                let f: Vec<&str> = line.split(',').collect();
                if f.len() < 6 || f[0] != path_name {
                    continue;
                }
                let idx: usize = f[1].parse().unwrap_or(usize::MAX);
                if idx < i0 || idx > i1 {
                    continue;
                }
                pts.push([f[3].parse().unwrap(), f[4].parse().unwrap(), f[5].parse().unwrap()]);
            }
            assert!(pts.len() >= 2, "no points of path {:?} in {:?}", path_name, csv.display());
            let mut appended: Vec<usize> = Vec::new();
            if let Some(ap) = flag(&args, "--append") {
                for p in ap.split(';').map(|s| s.trim()).filter(|s| !s.is_empty()) {
                    let v: Vec<f64> = p.split(',').map(|x| x.trim().parse().expect("--append x,y,z;x,y,z")).collect();
                    assert_eq!(v.len(), 3, "--append wants x,y,z triples");
                    pts.push([v[0], v[1], v[2]]);
                    appended.push(pts.len() - 1);
                }
            }
            // --idx2 A..B: a SECOND stretch of the same path, appended after the
            // --append points (a shortcut leaves the path and rejoins it later:
            // the lap line is [main path to the branch] + [the shortcut] + [main
            // path from the rejoin to the finish]).
            if let Some(r) = flag(&args, "--idx2") {
                let (a, b) = r.split_once("..").expect("--idx2 A..B");
                let (j0, j1) = (a.parse::<usize>().unwrap_or(0), b.parse::<usize>().unwrap_or(usize::MAX));
                for line in text.lines().skip(1) {
                    let f: Vec<&str> = line.split(',').collect();
                    if f.len() < 6 || f[0] != path_name {
                        continue;
                    }
                    let idx: usize = f[1].parse().unwrap_or(usize::MAX);
                    if idx < j0 || idx > j1 {
                        continue;
                    }
                    pts.push([f[3].parse().unwrap(), f[4].parse().unwrap(), f[5].parse().unwrap()]);
                }
            }
            let mut s = vec![0.0f64; pts.len()];
            for i in 1..pts.len() {
                let (a, b) = (pts[i - 1], pts[i]);
                s[i] = s[i - 1] + ((b[0] - a[0]).powi(2) + (b[1] - a[1]).powi(2) + (b[2] - a[2]).powi(2)).sqrt();
            }
            let total = *s.last().unwrap();
            let mut rows = String::from("time_ms,x,y,z\n");
            let mut t_ms: i64 = 0;
            let mut seg = 0usize;
            loop {
                let d = t_ms as f64 / 1000.0 * speed_ms;
                if d > total {
                    break;
                }
                while seg + 1 < s.len() - 1 && s[seg + 1] < d {
                    seg += 1;
                }
                let (a, b) = (pts[seg], pts[seg + 1]);
                let len = (s[seg + 1] - s[seg]).max(1e-9);
                let f = ((d - s[seg]) / len).clamp(0.0, 1.0);
                rows.push_str(&format!(
                    "{},{:.3},{:.3},{:.3}\n",
                    t_ms,
                    a[0] + (b[0] - a[0]) * f,
                    a[1] + (b[1] - a[1]) * f,
                    a[2] + (b[2] - a[2]) * f
                ));
                t_ms += 10;
            }
            std::fs::write(&out, rows).unwrap_or_else(|e| panic!("{}: {e}", out.display()));
            println!(
                "wrote {} : {} points of {} [{}..{}] + {} appended = {:.1} m, {} rows at {} m/s",
                out.display(), pts.len() - appended.len(), path_name, i0, i1.min(pts.len()), appended.len(), total, t_ms / 10, speed_ms
            );
            for &k in &appended {
                println!("  appended ({:.1}, {:.1}, {:.1}) at s {:.1} m  t {:.2} s", pts[k][0], pts[k][1], pts[k][2], s[k], s[k] / speed_ms);
            }
            if let Some(g) = flag(&args, "--gates") {
                for p in g.split(';').map(|s| s.trim()).filter(|s| !s.is_empty()) {
                    let v: Vec<f64> = p.split(',').map(|x| x.trim().parse().expect("--gates x,z;x,z")).collect();
                    let mut best = (f64::INFINITY, 0usize);
                    for (i, q) in pts.iter().enumerate() {
                        let d = ((q[0] - v[0]).powi(2) + (q[2] - v[1]).powi(2)).sqrt();
                        if d < best.0 {
                            best = (d, i);
                        }
                    }
                    let i = best.1;
                    let dir = if i + 1 < pts.len() { [pts[i + 1][0] - pts[i][0], pts[i + 1][2] - pts[i][2]] } else { [pts[i][0] - pts[i - 1][0], pts[i][2] - pts[i - 1][2]] };
                    let n = (dir[0] * dir[0] + dir[1] * dir[1]).sqrt().max(1e-9);
                    println!(
                        "  gate ({:.1}, {:.1}) -> path idx {} at s {:.1} m  t {:.2} s  ({:.1} m off)  dir ({:.4}, {:.4}) heading {:.1} deg  path point ({:.1}, {:.1}, {:.1})",
                        v[0], v[1], i, s[i], s[i] / speed_ms, best.0, dir[0] / n, dir[1] / n, (dir[0]).atan2(dir[1]).to_degrees(), pts[i][0], pts[i][1], pts[i][2]
                    );
                }
            }
        }
        "centreline" => {
            // A `tmreach lap` CENTRELINE from a reference line CSV (`tmmaps pathline`)
            // plus the map's own gate positions: {pts, s, half_width, speed_hint,
            // order_groups, segments[{i0,i1,to_pos}]}. The route-guided driver follows
            // this instead of a chain of hand-placed gate boxes -- which is what a
            // WHOLE LAP needs (the chain was built for 5-second stretches).
            let csv = PathBuf::from(&args[2]);
            let out = PathBuf::from(flag(&args, "--out").expect("--out F.json"));
            let map = PathBuf::from(flag(&args, "--map").expect("--map MAP.Map.Gbx"));
            let hw: f64 = flag(&args, "--half-width").map(|s| s.parse().unwrap()).unwrap_or(9.0);
            let speed: f64 = flag(&args, "--speed-hint").map(|s| s.parse().unwrap()).unwrap_or(40.0);
            let text = std::fs::read_to_string(&csv).unwrap_or_else(|e| panic!("{}: {e}", csv.display()));
            let mut pts: Vec<[f64; 3]> = Vec::new();
            for line in text.lines().skip(1) {
                let f: Vec<&str> = line.split(',').collect();
                if f.len() < 4 {
                    continue;
                }
                let p: [f64; 3] = [f[1].parse().unwrap(), f[2].parse().unwrap(), f[3].parse().unwrap()];
                if pts.last().map(|q: &[f64; 3]| (q[0] - p[0]).hypot(q[2] - p[2]) < 1.0).unwrap_or(false) {
                    continue;
                }
                pts.push(p);
            }
            assert!(pts.len() >= 2, "{}: fewer than two points", csv.display());
            let mut s_arr = vec![0.0f64; pts.len()];
            for i in 1..pts.len() {
                let (a, b) = (pts[i - 1], pts[i]);
                s_arr[i] = s_arr[i - 1] + ((b[0] - a[0]).powi(2) + (b[2] - a[2]).powi(2)).sqrt();
            }
            // the map's waypoints, in CREDIT order: group ids (a LinkedCheckpoint group is
            // one credit), each mapped to the nearest centreline point
            let m = map::MapFile::load(&map);
            let wps = m.waypoints();
            let mut groups: Vec<(u32, [f64; 3])> = Vec::new();
            for w in &wps {
                if w.tag == "Checkpoint" || w.tag == "LinkedCheckpoint" {
                    let Some(p) = w.pos else { continue };
                    let g = w.order as u32;
                    if let Some(e) = groups.iter_mut().find(|(id, _)| *id == g) {
                        // a linked group: keep the member nearest the line
                        let d_new = pts.iter().map(|q| (q[0] - p[0] as f64).hypot(q[2] - p[2] as f64)).fold(f64::INFINITY, f64::min);
                        let d_old = pts.iter().map(|q| (q[0] - e.1[0]).hypot(q[2] - e.1[2])).fold(f64::INFINITY, f64::min);
                        if d_new < d_old {
                            e.1 = [p[0] as f64, p[1] as f64, p[2] as f64];
                        }
                    } else {
                        groups.push((g, [p[0] as f64, p[1] as f64, p[2] as f64]));
                    }
                }
            }
            // order the groups ALONG THE LINE, then the finish (the start line) last
            let along = |p: [f64; 3]| -> (f64, usize) {
                let mut best = (f64::INFINITY, 0usize);
                for (i, q) in pts.iter().enumerate() {
                    let d = (q[0] - p[0]).hypot(q[2] - p[2]);
                    if d < best.0 {
                        best = (d, i);
                    }
                }
                (s_arr[best.1], best.1)
            };
            let mut ordered: Vec<(f64, u32, [f64; 3])> = groups.iter().map(|(g, p)| { let (sv, _) = along(*p); (sv, *g, *p) }).collect();
            ordered.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());
            // the finish gate: the StartFinish / Goal placement
            let fin = wps.iter().find(|w| w.tag == "Goal" || w.tag == "Finish" || w.tag == "StartFinish").and_then(|w| w.pos);
            let mut segs: Vec<String> = Vec::new();
            let mut ids: Vec<String> = Vec::new();
            let mut i0 = 0usize;
            for (sv, g, p) in &ordered {
                let (_, idx) = along(*p);
                if idx <= i0 {
                    continue;
                }
                segs.push(format!("{{\"i0\":{},\"i1\":{},\"to_pos\":[{:.2},{:.2},{:.2}]}}", i0, idx, p[0], p[1], p[2]));
                ids.push(g.to_string());
                i0 = idx;
                let _ = sv;
            }
            if let Some(f) = fin {
                segs.push(format!("{{\"i0\":{},\"i1\":{},\"to_pos\":[{:.2},{:.2},{:.2}]}}", i0, pts.len() - 1, f[0], f[1], f[2]));
                ids.push("0".to_string());
            }
            let j = format!(
                "{{\"pts\":[{}],\"s\":[{}],\"half_width\":[{}],\"speed_hint\":[{}],\"order_groups\":[{}],\"segments\":[{}]}}",
                pts.iter().map(|p| format!("[{:.2},{:.2},{:.2}]", p[0], p[1], p[2])).collect::<Vec<_>>().join(","),
                s_arr.iter().map(|x| format!("{:.2}", x)).collect::<Vec<_>>().join(","),
                vec![format!("{:.1}", hw); pts.len()].join(","),
                vec![format!("{:.1}", speed); pts.len()].join(","),
                ids.join(","),
                segs.join(",")
            );
            std::fs::write(&out, &j).unwrap_or_else(|e| panic!("{}: {e}", out.display()));
            println!("wrote {} : {} points, {:.0} m, {} ordered legs (groups {}), half-width {} m, speed hint {} m/s", out.display(), pts.len(), s_arr.last().unwrap(), segs.len(), ids.join(" "), hw, speed);
        }
        "setlaps" => {
            // The LAP COUNT of a multilap map (chunk 0x03043018: u32 isLapRace,
            // u32 nbLaps), for a certification copy: the validator only says
            // FINISHED after nbLaps crossings of a StartFinish line, and a 3-lap
            // MK64 course cannot be certified one lap at a time otherwise. The
            // header XML's nblaps="" is rewritten too. Geometry untouched.
            let src = PathBuf::from(&args[2]);
            let out = PathBuf::from(flag(&args, "--out").expect("--out F"));
            let laps: u32 = flag(&args, "--laps").expect("--laps N").parse().expect("--laps N");
            let bytes = std::fs::read(&src).unwrap_or_else(|e| panic!("{}: {e}", src.display()));
            let g = gbx::Gbx::parse(&bytes);
            let mut body = g.body.clone();
            // a SKIPPABLE chunk: id, "PIKS", size, payload
            let hits: Vec<usize> = gbx::all_skip_chunks(&body).into_iter().filter(|(cid, _, _, _)| *cid == 0x0304_3018).map(|(_, _, p, _)| p).collect();
            assert!(!hits.is_empty(), "no chunk 0x03043018 in the body");
            let mut patched = 0;
            for h in &hits {
                let o = *h;
                if o + 8 > body.len() {
                    continue;
                }
                let is_lap = u32::from_le_bytes(body[o..o + 4].try_into().unwrap());
                let n = u32::from_le_bytes(body[o + 4..o + 8].try_into().unwrap());
                if is_lap <= 1 && (1..=99).contains(&n) {
                    println!("chunk 0x03043018 at body {:#x}: isLapRace={} nbLaps={} -> {}", h, is_lap, n, laps);
                    body[o + 4..o + 8].copy_from_slice(&laps.to_le_bytes());
                    patched += 1;
                }
            }
            assert_eq!(patched, 1, "expected exactly one plausible 0x03043018 payload, found {patched} of {} id hits", hits.len());
            let mut file = g.write_body_recompressed(&body);
            // header XML: nblaps="N"
            let ud = &g.user_data;
            if let Some(i) = ud.windows(8).position(|w| w == b"nblaps=\"") {
                let start = i + 8;
                let end = start + ud[start..].iter().position(|b| *b == b'"').unwrap();
                let old = String::from_utf8_lossy(&ud[start..end]).to_string();
                let new = laps.to_string();
                if old.len() == new.len() {
                    // same length: patch in place in the written file's user data region
                    let fpos = file.windows(end - i).position(|w| w == &ud[i..end]).expect("xml in file");
                    file[fpos + 8..fpos + 8 + new.len()].copy_from_slice(new.as_bytes());
                    println!("header XML nblaps=\"{old}\" -> \"{new}\"");
                } else {
                    println!("header XML nblaps=\"{old}\" left (length differs; the body chunk is what the validator reads)");
                }
            }
            std::fs::write(&out, &file).unwrap_or_else(|e| panic!("{}: {e}", out.display()));
            let back = gbx::Gbx::parse(&std::fs::read(&out).unwrap());
            assert_eq!(back.body, body, "read-back body differs");
            println!("wrote {}", out.display());
        }
        "gbxcompress" => {
            // Rewrite ANY Gbx file with its body LZO-compressed ('C'). The
            // dedicated server accepts an uncompressed ('U') body, which is
            // what the synthesiser writes -- the GAME CLIENT does not
            // ("Unable to load ghost file"; the plugin's Replay_Load / Ghost_Add
            // drop the handler). Same header, same body bytes, one fresh LZO
            // stream; the round-trip is asserted by the writer.
            let src = PathBuf::from(&args[2]);
            let out = PathBuf::from(&args[3]);
            let bytes = std::fs::read(&src).unwrap_or_else(|e| panic!("{}: {e}", src.display()));
            let was = bytes.get(7).copied().unwrap_or(b'?') as char;
            let g = gbx::Gbx::parse(&bytes);
            let w = g.write_body_recompressed(&g.body);
            std::fs::write(&out, &w).unwrap_or_else(|e| panic!("{}: {e}", out.display()));
            let back = gbx::Gbx::parse(&w);
            assert_eq!(back.body, g.body, "read-back body differs");
            println!("wrote {} : body {} -> C, {} body bytes, file {} -> {} bytes", out.display(), was, g.body.len(), bytes.len(), w.len());
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
        "census" => census::cmd_census(&args),
        "header" => header::cmd(&args),
        "dropscan" => dropscan::cmd(&args),
        "chunks" => {
            // Every skippable chunk in the body, with its size. Needed to
            // reason about FREE blocks (0x0304305F) and to tell at a glance
            // whether a map even has a baked-blocks chunk (0x03043048).
            let g = gbx::Gbx::load(Path::new(&args[2])).unwrap();
            println!("chunk\toff\tpayload\tsize");
            for (cid, off, payload, size) in map::skip_chunks(&g.body) {
                println!("0x{:08X}\t{}\t{}\t{}", cid, off, payload, size);
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
  tmmaps region MAP --box X0,Y0,Z0:X1,Y1,Z1 [--filter PAT] [--items] [--blocks]
        everything whose position lies inside a world box. A GATE IS A
        STRUCTURE, NOT A BLOCK: run this before and after any move.
  tmmaps chunks MAP
        every skippable body chunk with its size
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
      [--anchor x,y,z] [--host HOST.Map.Gbx]
        replace every authored block by its library item (mapping rows
        `@index<TAB>ITEM|-<TAB>model_scale<TAB>sx<TAB>sz`; `-` = intentionally
        nothing) and re-point/drop items (`i@index<TAB>ITEM|stock model|-`);
        the baked foundation stays. Inputs come from `mapgeom tiny-library`.
  tmmaps tiny-batch DIR --out DIR [same flags]      every .Map.Gbx of a directory
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

