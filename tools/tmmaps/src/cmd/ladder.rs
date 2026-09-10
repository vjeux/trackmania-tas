//! `tmmaps` waypoint ladders — the route project's gate-moving experiments:
//! `ladder` (one map with several grid blocks moved), `rungspec`, `cporder`,
//! `move` (one block/item/baked block moved, with the origin control) and
//! `oracle` (stage a map for the oracle server). `Move` is the spelling of
//! one move in a ladder spec.

use std::path::{Path, PathBuf};
use tmmaps::cli::{die, flag, flag_multi, has, jobs_of, server_of};
use tmmaps::{map, oracle, secs};

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

/// `tmmaps ladder`.
pub fn ladder(args: &[String]) {
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

/// `tmmaps move`.
pub fn move_blocks(args: &[String]) {
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

/// `tmmaps oracle`.
pub fn oracle_stage(args: &[String]) {
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

/// `tmmaps cporder`.
pub fn cporder(args: &[String]) {
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

/// `tmmaps rungspec`.
pub fn rungspec(args: &[String]) {
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

/// `tmmaps blockprobe MAP --out F --block ARCHIVE/PATH.Block.Gbx --archetype NAME [--shift-y DY]`
/// — the water-volume probe of an embedded CUSTOM BLOCK (2026-09-10): re-point
/// one embedded `.Block.Gbx` at another ARCHETYPE block info (the pack block
/// whose units, clips and water volumes the custom block borrows; the string
/// must have the SAME LENGTH as the file's, the file is patched in place —
/// the TMX custom blocks are uncompressed Gbx) and, optionally, move its mesh
/// (every vertex position's y, so a solid deck gets out of the volume's way).
/// The map's zip is rebuilt with the patched file; manifest and placements
/// are untouched, so every placement of that block now instantiates the new
/// archetype.
pub fn blockprobe(args: &[String]) {
    let src = PathBuf::from(args.get(2).expect("blockprobe MAP --out F --block P --archetype NAME [--shift-y DY]"));
    let out = PathBuf::from(flag(args, "--out").expect("--out F"));
    let which = flag(args, "--block").expect("--block ARCHIVE/PATH.Block.Gbx").to_string();
    let arche = flag(args, "--archetype").expect("--archetype NAME").to_string();
    let shift_y: f32 = flag(args, "--shift-y").map(|s| s.parse().expect("--shift-y")).unwrap_or(0.0);
    let mut m = map::MapFile::load(&src);
    let (zip, names) = tmmaps::header::embedded_zip_bytes(&m.gbx.body).expect("map has no embedded zip");
    let entries = tmmaps::header::zip_entries(&zip);
    let (name, bytes) = entries
        .iter()
        .find(|(n, _)| n.replace('\\', "/").eq_ignore_ascii_case(&which.replace('\\', "/")))
        .unwrap_or_else(|| panic!("--block {which}: not in the archive ({} entries: {})", names.len(), names.iter().take(5).cloned().collect::<Vec<_>>().join(", ")))
        .clone();
    assert!(bytes.len() > 12 && &bytes[0..3] == b"GBX" && bytes[7] == b'U', "{name}: not an uncompressed Gbx (this probe patches bytes in place)");
    // the archetype: every length-prefixed occurrence of the current one
    let mut patched = bytes.clone();
    let old = find_lookback_strings(&patched);
    let cur = old.iter().find(|(_, s)| s.starts_with("Platform") || s.starts_with("Road") || s.starts_with("Deco")).map(|(_, s)| s.clone()).expect("no archetype-looking string in the block file");
    assert_eq!(cur.len(), arche.len(), "archetype {arche:?} must have the length of the file's {cur:?}");
    let mut n = 0;
    let mut i = 0;
    while i + 4 + cur.len() <= patched.len() {
        if u32::from_le_bytes(patched[i..i + 4].try_into().unwrap()) as usize == cur.len() && &patched[i + 4..i + 4 + cur.len()] == cur.as_bytes() {
            patched[i + 4..i + 4 + cur.len()].copy_from_slice(arche.as_bytes());
            n += 1;
            i += 4 + cur.len();
        } else {
            i += 1;
        }
    }
    println!("  {name}: archetype {cur} -> {arche} ({n} occurrences)");
    if shift_y != 0.0 {
        // CPlugVertexStream positions: after the chunk 0x09056000 header the
        // stream's f32 triplets; the simplest faithful move is every f32 that
        // sits in a position slot — found by the stream's own layout: locate
        // the vertex count and the position array (see mapgeom vstream.rs);
        // here: shift every triplet whose y lies within the block's own
        // height range (0..=8 m for a platform), which spares normals (unit
        // length, |y| <= 1 — also inside the range, so the range starts above 1).
        let mut k = 0usize;
        let mut moved = 0;
        while k + 12 <= patched.len() {
            let y = f32::from_le_bytes(patched[k + 4..k + 8].try_into().unwrap());
            let x = f32::from_le_bytes(patched[k..k + 4].try_into().unwrap());
            let z = f32::from_le_bytes(patched[k + 8..k + 12].try_into().unwrap());
            if y > 1.5 && y <= 8.5 && x.abs() <= 33.0 && z.abs() <= 33.0 && (x.fract() == 0.0 || (x * 8.0).fract() == 0.0) && (z * 8.0).fract() == 0.0 {
                patched[k + 4..k + 8].copy_from_slice(&(y + shift_y).to_le_bytes());
                moved += 1;
                k += 12;
            } else {
                k += 4;
            }
        }
        println!("  {name}: {moved} vertex positions shifted by {shift_y} in y");
    }
    let mut files: std::collections::BTreeMap<String, Vec<u8>> = entries.into_iter().collect();
    files.insert(name.clone(), patched);
    let zip2 = tmmaps::header::deflated_zip(&files);
    m.replace_embedded_zip_keep_manifest(&zip2);
    m.write_to(&out).expect("write");
    println!("  wrote {} ({} archive entries)", out.display(), files.len());
}

/// Every GBX length-prefixed string (u32 len 1..=64 then that many printable
/// bytes) with its offset — good enough to find a block file's archetype id.
fn find_lookback_strings(b: &[u8]) -> Vec<(usize, String)> {
    let mut out = Vec::new();
    let mut i = 0;
    while i + 4 < b.len() {
        let n = u32::from_le_bytes(b[i..i + 4].try_into().unwrap()) as usize;
        if (3..=64).contains(&n) && i + 4 + n <= b.len() && b[i + 4..i + 4 + n].iter().all(|c| c.is_ascii_graphic() || *c == b' ') {
            out.push((i, String::from_utf8_lossy(&b[i + 4..i + 4 + n]).to_string()));
            i += 4 + n;
        } else {
            i += 1;
        }
    }
    out
}
