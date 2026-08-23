//! segments.rs -- cut a TM2020 map at each checkpoint, so that a partial run
//! becomes a FINISHING run and the dedicated-server oracle reports its split to
//! the millisecond. Port of make_segments.py; the module docstring there is
//! hard-won measured knowledge and is carried over here in full.
//!
//! WHY
//! ===
//! `TrackmaniaServer /validatepath=.` re-simulates a ghost's input bitstream
//! and returns `ValidatedResult.Time` -- but only for a run that crosses the
//! finish. A run that dies half way returns no time at all, only
//! `wrong simu, but reached some checkpoints (N out of M)`. Most search
//! candidates DNF, so most of the search budget buys a 2-bit integer.
//! Segment map k moves the finish to checkpoint k: a candidate that only
//! reaches CP k now finishes, and the oracle prints an exact ms time for it.
//!
//! HOW A TM2020 MAP STORES A WAYPOINT
//! ==================================
//! Two carriers, and they behave DIFFERENTLY:
//!
//! * a **block** in `CGameCtnChallenge` chunk 0x0304301F, with
//!   `flags & 0x100000` and a `waypointParams` node
//!   (CGameWaypointSpecialProperty, class 0x2E009000, `tag` in
//!   {Spawn, Checkpoint, Goal});
//! * an **anchored object** (item) in chunk 0x03043040, with the same property
//!   under `waypointSpecialProperty`.
//!
//! MEASURED: for BOTH carriers the tag in the map file is **ignored by the
//! game** -- what a waypoint does is decided by the block model
//! (`RoadTechCheckpoint` vs `RoadTechFinish`) or the item model
//! (`GateCheckpointLeft32m` vs `GateFinish32m`). Retagging a block
//! Checkpoint -> Goal, or deleting its `waypointParams` outright, changes
//! nothing at all. (Four separate experiments, each validating the WR ghost:
//! all four returned 19538 ms / 4 CPs, i.e. no change.)
//!
//! So the surgery is done on MODELS, and there are two moves:
//!
//! 1. *Neutralise* a checkpoint at or after the cut (it must stop being a
//!    required checkpoint, because TM only ends a race at a finish once every
//!    checkpoint has been collected -- measured: a gate moved 400 m into the
//!    air still gave `reached some checkpoints (2 out of 4)`):
//!      - block: rename `<X>Checkpoint` -> `<X>Finish`;
//!      - item : swap the model to `GateFinish32m`.
//!    Both turn the waypoint into an extra finish, which is harmless: the race
//!    has already ended at the cut. (Renaming an EARLY checkpoint block is NOT
//!    safe: the finish block's mesh is not the checkpoint block's, and the car
//!    crashes a few metres later. Everything at or after the cut is never
//!    driven, so this only matters for the cut itself -- see below.)
//!
//! 2. *Promote* the cut checkpoint to the finish, at exactly its own trigger
//!    plane:
//!      - if it is an item, swap the model in place: `GateFinish32m`'s trigger
//!        is geometrically identical to `GateCheckpointLeft32m`'s -> the
//!        reported time equals the declared CP split exactly;
//!      - if it is a block, do NOT rename it (the finish block triggers
//!        ~100-150 ms early, at the cell entry instead of the cell centre --
//!        measured -101/-102 ms on map 1 CP3 and -147/-147 ms on CP1).
//!        Instead relocate a spare waypoint gate ITEM onto the block's cell
//!        centre and swap it to `GateFinish32m`. MEASURED: a gate at the
//!        horizontal centre of a checkpoint block's 32 m cell fires at exactly
//!        the block's checkpoint split, to the millisecond, for every ghost
//!        tried.
//!
//! Checkpoint ORDER is not in the file (every waypoint's `order` is 0), so it
//! is measured: promote a candidate set and see whether the reference ghost
//! finishes. A promoted checkpoint only ends the race once all un-promoted
//! checkpoints are collected, so "promote X alone -> finishes EARLIER than the
//! full race time" identifies the LAST checkpoint; repeat with it always
//! promoted to peel off the next-to-last, and so on.
//!
//! CONTAINER GOTCHAS
//! =================
//! The Python's three (gbx-py rewriting collection id 28 as "U28", gbx-py
//! recomputing `numNodes` as 10 instead of 39, and chunk 0x03043040's internal
//! size field) are discussed in map.rs: the byte patcher makes the first two
//! impossible and reduces the third to one u32 write. Two more still apply:
//!
//! * The mapUid is not recomputed and not checked by the server against the
//!   map's content: every segment map keeps the original uid, so unmodified
//!   ghosts resolve to it. The consequence is that two segment maps can never
//!   share a `UserData/Maps` directory -- each needs its own (see oracle.rs).
//! * The map's own embedded author-validation ghost is harmless and is left
//!   untouched; `/validatepath` only validates what is in `UserData/Replays`.
//!
//! DEVIATIONS FROM THE PYTHON (both are bugs there, see the report)
//! ================================================================
//! * The Python hardcodes `FINISH_BLOCK = "RoadTechFinish"`, so on any map
//!   whose checkpoints are not Tech road (map 2 is Dirt) the fallback rename
//!   would have written a block model that does not exist on that map's
//!   surface. Here the finish model is derived from the checkpoint's own model
//!   name: `RoadDirtCheckpoint` -> `RoadDirtFinish`.
//! * The Python trusts the relocated gate blindly. On map 2 no placement of
//!   the one spare gate fires at all (measured: a 20-placement sweep across
//!   the cut block's cell, every one returned the full race time), so the
//!   segment map would silently have been a copy of the full map. Here a block
//!   cut that uses a relocated gate is VERIFIED against the reference ghost
//!   and falls back to the block rename when the gate never fires.

use crate::map::{Kind, MapFile, Waypoint, CELL_XZ, CELL_Y, FINISH_GATE};
use crate::secs::signed;
use crate::oracle;
use std::path::{Path, PathBuf};

/// The finish model that corresponds to a checkpoint block model.
pub fn finish_block_name(cp_name: &str) -> String {
    if let Some(p) = cp_name.rfind("Checkpoint") {
        format!("{}Finish{}", &cp_name[..p], &cp_name[p + "Checkpoint".len()..])
    } else {
        // not a recognisable checkpoint model: leave the caller's default
        format!("{}Finish", cp_name)
    }
}

/// Where to park a finish gate so it fires exactly where a checkpoint BLOCK's
/// own trigger fires: the horizontal centre of the block's cell, at the gate's
/// usual height above its cell floor.
pub fn block_gate_pos(block: &Waypoint, gate: &Waypoint) -> [f32; 3] {
    let (bx, by, bz) = block.coords;
    let (_gx, gy, _gz) = gate.coords;
    let gpos = gate.pos.expect("gate has no position");
    let y = gpos[1] + (by - gy) as f32 * CELL_Y;
    [
        bx as f32 * CELL_XZ + CELL_XZ / 2.0,
        y,
        bz as f32 * CELL_XZ + CELL_XZ / 2.0,
    ]
}

pub fn neutralise(m: &mut MapFile, wp: &Waypoint) {
    match wp.kind {
        Kind::Block => {
            let name = finish_block_name(&wp.name);
            m.set_block_name(wp.index, &name);
        }
        Kind::Item => m.set_item_model(wp.index, FINISH_GATE),
    }
}

/// Relocate a waypoint gate item and turn it into a finish gate.
pub fn move_gate(m: &mut MapFile, gate: &Waypoint, pos: [f32; 3], yaw: f32, cell: (i32, i32, i32)) {
    m.set_item_model(gate.index, FINISH_GATE);
    m.move_item(gate.index, pos, yaw, cell);
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Method {
    GateSwappedInPlace,
    GateRelocated,
    BlockRenamed,
    Control,
}

impl Method {
    pub fn label(&self) -> &'static str {
        match self {
            Method::GateSwappedInPlace => "gate item swapped in place",
            Method::GateRelocated => "finish gate relocated to cell centre",
            Method::BlockRenamed => "block renamed (approximate, fires ~100-170 ms early)",
            Method::Control => "unmodified (rebuilt through the same writer)",
        }
    }
    pub fn exact(&self) -> bool {
        !matches!(self, Method::BlockRenamed)
    }
}

/// Build the segment map cutting at `cps[k]`. `gate` is a spare waypoint gate
/// item to relocate when the cut is a block; `force_rename` skips it.
pub fn build_segment(
    src: &Path,
    cps: &[Waypoint],
    k: usize,
    out: &Path,
    gate: Option<&Waypoint>,
    force_rename: bool,
) -> Method {
    let mut m = MapFile::load(src);
    let cut = &cps[k];
    let mut later: Vec<Waypoint> = cps[k + 1..].to_vec();
    let method;
    if cut.kind == Kind::Item {
        neutralise(&mut m, cut); // swap the model in place
        method = Method::GateSwappedInPlace;
    } else {
        match if force_rename { None } else { gate } {
            None => {
                neutralise(&mut m, cut); // fallback: fires ~100-170 ms early
                method = Method::BlockRenamed;
            }
            Some(g) => {
                let pos = block_gate_pos(cut, g);
                move_gate(&mut m, g, pos, cut.yaw.unwrap_or(0.0), cut.coords);
                later.retain(|w| !(w.kind == g.kind && w.index == g.index));
                method = Method::GateRelocated;
            }
        }
    }
    for w in &later {
        neutralise(&mut m, w);
    }
    m.write_to(out).expect("write segment map");
    method
}

/// Parse + rebuild with no edits: the control / last segment.
pub fn rebuild(src: &Path, out: &Path) {
    let m = MapFile::load(src);
    m.write_to(out).expect("write control map");
}

// ------------------------------------------------------------------- ORDER
//
// THE DEFECT THIS SECTION REPLACES, and why it survived so long
// =============================================================
// The peel is right and the comparator was wrong. Round `r` (with `rest.len()
// == r` candidates still unplaced and `tail` already promoted) promotes each
// candidate `w` in turn and asks the reference ghost for a time. Exactly two
// answers are possible, and BOTH are predicted, to the millisecond, by the
// ghost's own declared splits:
//
//   * `w` IS the last of `rest`  -> every still-required checkpoint is
//     collected before the car reaches `w`, so the race ends AT `w`:
//         t == splits[r - 1]
//   * `w` is NOT the last        -> the race runs on to the real last of
//     `rest`, and then ends at the first ALREADY-promoted gate after it,
//     which is `tail[0]` (or the map's own Goal in round one):
//         t == splits[r]
//
// So the winner is the SMALLEST time and every loser shares one larger time.
// The shipped code took `max_by_key`. In round one that is harmless -- the
// losers do not finish early at all, they run to the Goal, and the single
// finisher is both the max and the min -- and from round two on it picks a
// loser every time, silently.
//
// WHY THE ACCEPTANCE TESTS STAYED GREEN. On a map whose checkpoints are
// BLOCKS, `neutralise` renames the block, and renaming a checkpoint block the
// car still has to drive through swaps its mesh: the car crashes a few metres
// later (module docs above) and the probe DNFs. So on a block map the losers
// mostly return None, the single finisher is again both max and min, and the
// wrong comparator cannot be seen. On a map whose checkpoints are ITEMS the
// model swap is harmless -- it is a gate you drive through -- every candidate
// finishes, and the comparator decides. 146612 is that map: five item gates,
// every probe a finisher, and `max_by_key` (which returns the LAST of equal
// maxima) walked straight down the tie.
//
// MEASURED, shipped v6.3 build, 146612, reference ghost rank00001_40223:
//
//   round 1 (r=5)  439:40223 440:40223 492:33584 494:40223 633:40223 -> 492 ok
//   round 2 (r=4)  439:33584 440:33584 494:33584 633:27834  -> picked 494, WRONG
//   round 3 (r=3)  439:33584 440:33584 633:27834            -> picked 440, WRONG
//   round 4 (r=2)  439:33584 633:15718                      -> picked 439, WRONG
//   order 633,439,440,494,492   (correct: 439,494,440,633,492)
//
// and the six segment maps it then built returned
// 7.311 / 33.584 / 33.584 / 33.584 / 33.584 / 40.223 -- four of six the same
// time, which is the symptom an arm noticed by hand.
//
// WHAT IS DIFFERENT NOW. The comparator is the least of it. The round's two
// legal answers are both PREDICTIONS, so every round is a test that can fail:
// the winner must be the unique minimum, it must be separated from the runner
// up, and its time must match `splits[r-1]` and no other declared split. If
// any of those does not hold the tool REFUSES and prints the whole probe
// table. A map it cannot resolve gets a loud refusal and a pointer to
// `--order`, which now actually works.

/// How far a measured fire time may sit from the declared split it is matched
/// to. A checkpoint BLOCK neutralised by rename fires at the cell ENTRY
/// instead of the cell centre -- measured -101/-102 ms on map 1 CP3,
/// -147/-147 ms on CP1, -168 ms on map 2 CP1 -- so an exact match cannot be
/// required. The nearest-split test below is the scale-free one; this is only
/// a backstop for a map whose splits are further apart than this window.
pub const MATCH_TOL_MS: i64 = 500;

/// The winner must beat the runner-up by at least this much. Two candidates
/// closer than this did not discriminate, and that is a refusal, not a
/// coin toss. Two ticks.
pub const SEPARATION_MS: i64 = 20;

/// One candidate's probe in one peeling round.
#[derive(Clone, Debug)]
pub struct OrderProbe {
    pub wp: Waypoint,
    pub time: Option<i64>,
}

#[derive(Clone, Debug)]
pub struct OrderReport {
    pub order: Vec<Waypoint>,
    /// things that did not justify a refusal but that a reader should see
    pub notes: Vec<String>,
}

/// The declared split nearest `t`, and the signed deviation `t - splits[j]`.
pub fn nearest_split(t: i64, splits: &[i64]) -> (usize, i64) {
    let mut best = (0usize, t - splits[0]);
    for (j, s) in splits.iter().enumerate() {
        if (t - s).abs() < best.1.abs() {
            best = (j, t - s);
        }
    }
    best
}

fn round_table(position: usize, ew: i64, el: i64, probes: &[OrderProbe], splits: &[i64]) -> String {
    let mut s = format!(
        "  round for driving position {} (0-based): the reference ghost predicts\n    \
         winner {} (declared split #{}), every loser {} (declared split #{})\n",
        position,
        crate::secs::ms(ew),
        position,
        crate::secs::ms(el),
        position + 1
    );
    for p in probes {
        let t = match p.time {
            None => "DNF".to_string(),
            Some(v) => crate::secs::ms(v),
        };
        let cls = match p.time {
            None => "did not finish".to_string(),
            Some(v) => {
                let (j, d) = nearest_split(v, splits);
                format!(
                    "nearest declared split #{} ({}), deviation {}{}",
                    j,
                    crate::secs::ms(splits[j]),
                    signed(d),
                    if j == position {
                        "  <- WINNER shape"
                    } else if j == position + 1 {
                        "  <- loser shape"
                    } else {
                        "  <- NEITHER shape"
                    }
                )
            }
        };
        s.push_str(&format!("    {:<52} {:>10}   {}\n", p.wp.to_string(), t, cls));
    }
    s
}

/// Measure the driving order of the checkpoints (see the module docs and the
/// section comment above). `splits` is the reference ghost's own declared
/// split list, `n_checkpoints + 1` long, last entry the race time.
///
/// Returns `Err` -- loudly, with the probe table -- rather than guessing.
#[allow(clippy::too_many_arguments)]
pub fn order_checkpoints(
    src: &Path,
    wps: &[Waypoint],
    ref_ghost: &Path,
    workdir: &Path,
    splits: &[i64],
    jobs: usize,
    server: &str,
    verbose: bool,
) -> Result<OrderReport, String> {
    let cps: Vec<Waypoint> = wps.iter().filter(|w| w.tag == "Checkpoint").cloned().collect();
    let n = cps.len();
    if splits.len() != n + 1 {
        let linked: Vec<&Waypoint> = wps.iter().filter(|w| w.tag == "LinkedCheckpoint").collect();
        let cause = if !linked.is_empty() {
            format!(
                "\n  THIS MAP HAS {} waypoint(s) tagged LinkedCheckpoint: {}.\n  \
                 A linked group is several gates spanning one wide road that the game counts \
                 as ONE checkpoint number, and this tool filters on `tag == \"Checkpoint\"`, \
                 so it neither counts them nor can order them. Measured across the banked \
                 store: every map whose counts disagree has LinkedCheckpoint waypoints, and \
                 every map without them agrees -- {} plain + {} group(s) = {} declared \
                 checkpoints on this map. Grouping them is unbuilt work, not a setting.",
                linked.len(),
                linked
                    .iter()
                    .map(|w| format!(
                        "{}{}",
                        if w.kind == Kind::Block { "b" } else { "i" },
                        w.index
                    ))
                    .collect::<Vec<_>>()
                    .join(","),
                n,
                (splits.len() as i64 - 1) - n as i64,
                splits.len() - 1
            )
        } else {
            String::new()
        };
        return Err(format!(
            "REFUSING to measure the checkpoint order.\n  \
             the map declares {} waypoint(s) tagged Checkpoint, so the reference ghost \
             should declare {} split(s) (the checkpoints plus the finish).\n  \
             it declares {}: {:?}{}\n  \
             the two do not describe the same run, so every prediction this measurement \
             checks itself against would be meaningless. Other causes seen on this project: \
             a multi-lap map, a ghost recorded on a different revision of the map, or a \
             ghost that did not finish.\n  \
             pass the order explicitly with --order once you know it \
             (`tmtraj decode G --csv T.csv` then \
             `tmmaps cporder MAP T.csv --splits ...` reads it off one trajectory decode and \
             prints the match distance and the runner-up, so a bad match is visible).",
            n,
            n + 1,
            splits.len(),
            splits,
            cause
        ));
    }
    let mut tail: Vec<Waypoint> = Vec::new();
    let mut rest = cps;
    let mut notes: Vec<String> = Vec::new();
    std::fs::create_dir_all(workdir).unwrap();
    while rest.len() > 1 {
        let r = rest.len();
        let position = r - 1;
        let expect_winner = splits[position];
        let expect_loser = splits[r];
        let mut pairs: Vec<(PathBuf, Vec<PathBuf>)> = Vec::new();
        for w in &rest {
            let p = workdir.join(format!(
                "ord_{}{}.Map.Gbx",
                if w.kind == Kind::Block { "block" } else { "item" },
                w.index
            ));
            let mut m = MapFile::load(src);
            neutralise(&mut m, w);
            for x in &tail {
                neutralise(&mut m, x);
            }
            m.write_to(&p).unwrap();
            pairs.push((p, vec![ref_ghost.to_path_buf()]));
        }
        let res = oracle::run_maps(&pairs, jobs, server);
        let gname = ref_ghost.file_name().unwrap().to_string_lossy().into_owned();
        for (i, rows) in res.iter().enumerate() {
            if let Some(r) = rows.first() {
                if r.file != gname {
                    return Err(format!(
                        "REFUSING: the probe for {} came back attributed to '{}', not '{}'. \
                         The oracle mis-attributed a result; the order this round would \
                         report is meaningless.",
                        rest[i], r.file, gname
                    ));
                }
            }
        }
        let probes: Vec<OrderProbe> = res
            .iter()
            .enumerate()
            .map(|(i, rows)| {
                let t = rows.first().and_then(|r| r.sim_time);
                OrderProbe { wp: rest[i].clone(), time: t }
            })
            .collect();
        let table = round_table(position, expect_winner, expect_loser, &probes, splits);
        if verbose {
            print!("{}", table);
        }

        // --- the three tests, each of which can fail -------------------
        let mut finishers: Vec<(i64, usize)> = probes
            .iter()
            .enumerate()
            .filter_map(|(i, p)| p.time.map(|t| (t, i)))
            .collect();
        finishers.sort_by_key(|(t, _)| *t);
        if finishers.is_empty() {
            return Err(format!(
                "REFUSING: no candidate finished in the round for driving position {}.\n{}  \
                 every probe DNF'd, so nothing identifies the checkpoint driven last among \
                 those still unplaced. On a map whose checkpoints are BLOCKS this is the \
                 expected failure -- neutralising a checkpoint block renames it, which swaps \
                 a mesh the car is still driving through. Use --order.",
                position, table
            ));
        }
        let (best_t, best_i) = finishers[0];
        if let Some(&(second_t, second_i)) = finishers.get(1) {
            if second_t - best_t < SEPARATION_MS {
                return Err(format!(
                    "REFUSING: the round for driving position {} is AMBIGUOUS.\n{}  \
                     {} fired at {} and {} fired at {} -- {} apart, under the {} \
                     separation this measurement requires. Two checkpoints this close cannot \
                     be ordered by their fire times. Use --order.",
                    position,
                    table,
                    probes[best_i].wp,
                    crate::secs::ms(best_t),
                    probes[second_i].wp,
                    crate::secs::ms(second_t),
                    crate::secs::ms(second_t - best_t),
                    crate::secs::ms(SEPARATION_MS)
                ));
            }
        }
        let (j, dev) = nearest_split(best_t, splits);
        if j != position || dev.abs() > MATCH_TOL_MS {
            return Err(format!(
                "REFUSING: the fastest candidate in the round for driving position {} does not \
                 match the time the reference ghost declares for that position.\n{}  \
                 {} fired at {}; declared split #{} is {} ({}), and the nearest \
                 declared split to that time is #{} ({}).\n  \
                 the peel and the ghost disagree, so the order is not established. Use --order.",
                position,
                table,
                probes[best_i].wp,
                crate::secs::ms(best_t),
                position,
                crate::secs::ms(expect_winner),
                signed(best_t - expect_winner),
                j,
                crate::secs::ms(splits[j])
            ));
        }
        if dev != 0 {
            notes.push(format!(
                "position {}: {} fired at {} against declared split {} ({})",
                position,
                probes[best_i].wp,
                crate::secs::ms(best_t),
                crate::secs::ms(expect_winner),
                signed(dev)
            ));
        }
        for (i, p) in probes.iter().enumerate() {
            if i == best_i {
                continue;
            }
            if let Some(t) = p.time {
                if (t - expect_loser).abs() > MATCH_TOL_MS {
                    notes.push(format!(
                        "position {}: loser {} fired at {}, not the predicted {} ({}) \
                         -- neutralising it changed the run",
                        position,
                        p.wp,
                        crate::secs::ms(t),
                        crate::secs::ms(expect_loser),
                        signed(t - expect_loser)
                    ));
                }
            }
        }

        tail.insert(0, rest[best_i].clone());
        rest.remove(best_i);
    }
    rest.extend(tail);
    Ok(OrderReport { order: rest, notes })
}

/// Resolve an explicit `--order` spec against the map's own checkpoints.
///
/// A token is a waypoint index, optionally carrying its carrier: `439`,
/// `i439`, `item#439`, `b2089`, `block#2089`. A bare number must name exactly
/// one checkpoint across BOTH carriers -- block and item indices are separate
/// spaces and can collide, so an ambiguous bare number is refused rather than
/// resolved by a rule the caller cannot see.
pub fn resolve_order(cps: &[Waypoint], spec: &[String]) -> Result<Vec<Waypoint>, String> {
    let names = |v: &[Waypoint]| {
        v.iter()
            .map(|w| {
                format!(
                    "{}{}",
                    if w.kind == Kind::Block { "b" } else { "i" },
                    w.index
                )
            })
            .collect::<Vec<_>>()
            .join(",")
    };
    if spec.len() != cps.len() {
        return Err(format!(
            "REFUSING --order: it names {} waypoint(s) but the map has {} checkpoint(s) ({}).",
            spec.len(),
            cps.len(),
            names(cps)
        ));
    }
    let mut out: Vec<Waypoint> = Vec::new();
    for tok in spec {
        let t = tok.trim();
        let (want_kind, digits) = if let Some(d) = t.strip_prefix("item#") {
            (Some(Kind::Item), d)
        } else if let Some(d) = t.strip_prefix("block#") {
            (Some(Kind::Block), d)
        } else if let Some(d) = t.strip_prefix('i') {
            (Some(Kind::Item), d)
        } else if let Some(d) = t.strip_prefix('b') {
            (Some(Kind::Block), d)
        } else {
            (None, t)
        };
        let idx: usize = digits.parse().map_err(|_| {
            format!(
                "REFUSING --order: cannot read '{}' as a waypoint index. \
                 Write `439`, `i439`/`item#439` for an item, `b2089`/`block#2089` for a block.",
                t
            )
        })?;
        let hits: Vec<&Waypoint> = cps
            .iter()
            .filter(|w| w.index == idx && want_kind.as_ref().map(|k| *k == w.kind).unwrap_or(true))
            .collect();
        match hits.len() {
            1 => out.push(hits[0].clone()),
            0 => {
                return Err(format!(
                    "REFUSING --order: '{}' names no checkpoint of this map. Its checkpoints are {}.",
                    t,
                    names(cps)
                ))
            }
            _ => {
                return Err(format!(
                    "REFUSING --order: '{}' is AMBIGUOUS -- it matches {} checkpoints ({}). \
                     Block and item indices are separate spaces; spell it `b{}` or `i{}`.",
                    t,
                    hits.len(),
                    hits.iter().map(|w| w.to_string()).collect::<Vec<_>>().join(" and "),
                    idx,
                    idx
                ))
            }
        }
    }
    let key = |w: &Waypoint| (if w.kind == Kind::Block { 0 } else { 1 }, w.index);
    let mut got: Vec<_> = out.iter().map(key).collect();
    let mut want: Vec<_> = cps.iter().map(key).collect();
    got.sort_unstable();
    want.sort_unstable();
    if got != want {
        return Err(format!(
            "REFUSING --order: it is not a permutation of the map's checkpoints. \
             given {}, map has {}.",
            names(&out),
            names(cps)
        ));
    }
    Ok(out)
}

#[derive(Clone, Debug)]
pub struct Segment {
    pub segment: usize,
    pub map: PathBuf,
    pub cut: String,
    pub method: &'static str,
    /// the cut is at the checkpoint's own trigger plane (not the ~100-170 ms
    /// early block-rename fallback)
    pub exact: bool,
    /// what the reference ghost actually got on this segment map
    pub time: Option<i64>,
    /// what the reference ghost's own declared splits say it must get
    pub expect: i64,
    /// measured == declared (exactly, for an exact method; within
    /// `MATCH_TOL_MS` and early, for the block-rename fallback)
    pub verified: bool,
    /// FNV-1a 64 of the DECOMPRESSED body -- LZO is not bit-reproducible, so
    /// file hashes are the wrong level to compare two maps at
    pub body_hash: u64,
}

fn fnv1a64(bytes: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in bytes {
        h ^= *b as u64;
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    h
}

fn body_hash(p: &Path) -> u64 {
    match crate::gbx::Gbx::load(p) {
        Ok(g) => fnv1a64(&g.body),
        Err(_) => 0,
    }
}

/// Build every segment map for `src` and CHECK each one against the reference
/// ghost's own declared splits.
///
/// `order`, when given, is the driving order as `--order` spelt it (see
/// `resolve_order`); when absent it is measured by `order_checkpoints`, which
/// refuses rather than guessing.
///
/// Returns `Err` -- with the table -- when the order cannot be established,
/// when two segment maps come out identical, or when a built segment does not
/// reproduce the split it is supposed to cut at. The maps are left on disk in
/// `out_dir` either way, so a refusal is diagnosable.
#[allow(clippy::too_many_arguments)]
pub fn make_all_ordered(
    src: &Path,
    out_dir: &Path,
    ref_ghost: &Path,
    jobs: usize,
    server: &str,
    verbose: bool,
    order: Option<&[String]>,
) -> Result<Vec<Segment>, String> {
    std::fs::create_dir_all(out_dir).unwrap();
    let base = src
        .file_name()
        .unwrap()
        .to_string_lossy()
        .split('.')
        .next()
        .unwrap()
        .to_string();
    let m = MapFile::load(src);
    let wps = m.waypoints();
    if verbose {
        println!("  waypoints:");
        for w in &wps {
            println!("    {}", w);
        }
    }
    let gates: Vec<Waypoint> = wps.iter().filter(|w| w.kind == Kind::Item).cloned().collect();
    let splits: Vec<i64> = crate::ghost::splits(ref_ghost)
        .ok_or_else(|| {
            format!(
                "REFUSING: {} carries no 0x0309202B split chunk, so there is nothing to check \
                 the segment maps against.",
                ref_ghost.display()
            )
        })?
        .iter()
        .map(|v| *v as i64)
        .collect();
    let full_time = *splits.last().unwrap();
    let declared: Vec<Waypoint> = wps.iter().filter(|w| w.tag == "Checkpoint").cloned().collect();

    let cps: Vec<Waypoint> = match order {
        Some(spec) => {
            let o = resolve_order(&declared, spec)?;
            if verbose {
                println!("  driving order: GIVEN with --order (not measured)");
            }
            o
        }
        None => {
            let rep = order_checkpoints(
                src,
                &wps,
                ref_ghost,
                &out_dir.join("_order"),
                &splits,
                jobs,
                server,
                verbose,
            )?;
            for n in &rep.notes {
                println!("  note: {}", n);
            }
            rep.order
        }
    };
    if verbose {
        println!(
            "  driving order: {}",
            cps.iter().map(|w| w.to_string()).collect::<Vec<_>>().join(", ")
        );
    }
    if cps.len() + 1 != splits.len() {
        return Err(format!(
            "REFUSING: {} checkpoint(s) in the order but the reference ghost declares {} \
             split(s); every segment would be checked against the wrong split.",
            cps.len(),
            splits.len()
        ));
    }

    let mut out: Vec<Segment> = Vec::new();
    for (k, w) in cps.iter().enumerate() {
        let gate = if w.kind == Kind::Block { gates.first() } else { None };
        let p = out_dir.join(format!("{}_seg{}.Map.Gbx", base, k + 1));
        let mut method = build_segment(src, &cps, k, &p, gate, false);
        if method == Method::GateRelocated {
            // VERIFY the relocated gate actually fires for the reference ghost
            // (map 2: it never does, and the map would silently be the full
            // map). Fall back to the block rename if it does not.
            let rows = oracle::run_maps(
                &[(p.clone(), vec![ref_ghost.to_path_buf()])],
                jobs,
                server,
            );
            let t = rows[0].first().and_then(|r| r.sim_time);
            let fired = matches!(t, Some(v) if v < full_time);
            if verbose {
                println!(
                    "  seg{} gate probe -> {} ({})",
                    k + 1,
                    crate::secs::opt(t),
                    if fired { "fires" } else { "never fires -- falling back to block rename" }
                );
            }
            if !fired {
                method = build_segment(src, &cps, k, &p, gate, true);
            }
        }
        out.push(Segment {
            segment: k + 1,
            map: p.clone(),
            cut: w.to_string(),
            method: method.label(),
            exact: method.exact(),
            time: None,
            expect: splits[k],
            verified: false,
            body_hash: body_hash(&p),
        });
        if verbose {
            println!(
                "  seg{} -> {} ({})",
                k + 1,
                p.file_name().unwrap().to_string_lossy(),
                method.label()
            );
        }
    }
    let p = out_dir.join(format!("{}_seg{}.Map.Gbx", base, cps.len() + 1));
    rebuild(src, &p);
    out.push(Segment {
        segment: cps.len() + 1,
        map: p.clone(),
        cut: "original finish".to_string(),
        method: Method::Control.label(),
        exact: true,
        time: None,
        expect: full_time,
        verified: false,
        body_hash: body_hash(&p),
    });
    if verbose {
        println!(
            "  seg{} -> {} (control: the real finish)",
            cps.len() + 1,
            p.file_name().unwrap().to_string_lossy()
        );
    }

    // ---- CONTROL 1: the maps must be DISTINCT ------------------------
    // Four of 146612's six came out behaviourally identical under the old
    // ordering. Their FILES differed -- a wrong promotion is still a
    // different edit -- so this catches only the degenerate case; control 2
    // is the one that caught 146612, and both are cheap.
    let mut seen: std::collections::HashMap<u64, usize> = std::collections::HashMap::new();
    for s in &out {
        if let Some(prev) = seen.insert(s.body_hash, s.segment) {
            return Err(format!(
                "REFUSING: seg{} and seg{} are the SAME map (decompressed body FNV-1a \
                 {:016x}). A segment map that duplicates another cuts at the same place \
                 and reports the same time.",
                prev, s.segment, s.body_hash
            ));
        }
    }

    // ---- CONTROL 2: every segment must report its own declared split ---
    let pairs: Vec<(PathBuf, Vec<PathBuf>)> = out
        .iter()
        .map(|s| (s.map.clone(), vec![ref_ghost.to_path_buf()]))
        .collect();
    let res = oracle::run_maps(&pairs, jobs, server);
    let mut bad: Vec<String> = Vec::new();
    let ghost_name = ref_ghost.file_name().unwrap().to_string_lossy().into_owned();
    for (i, rows) in res.iter().enumerate() {
        // ATTRIBUTION GUARD. Every pair here is one map and one ghost in its
        // own server directory, so a row can only be that ghost -- but a
        // sibling tool has been shown to mis-attribute results WITHIN a batch,
        // and a verification that trusts row order is exactly the instrument
        // that cannot see it. Check the name the server printed.
        if let Some(r) = rows.first() {
            if r.file != ghost_name {
                return Err(format!(
                    "REFUSING: seg{} came back attributed to '{}', not '{}'. The oracle \
                     mis-attributed a result and nothing below this line can be trusted.",
                    out[i].segment, r.file, ghost_name
                ));
            }
        }
        let t = rows.first().and_then(|r| r.sim_time);
        let want = out[i].expect;
        let ok = match t {
            None => false,
            Some(v) if out[i].exact => v == want,
            // the block-rename fallback fires at the cell ENTRY: EARLY, and
            // by less than MATCH_TOL_MS. Late is a failure; early by more is a
            // failure; and EXACT is the best case, not a failure.
            //
            // It was written `(want - v) > 0`, which refuses `v == want`. On
            // 134672 all four rungs reproduced the reference's own split to
            // the millisecond and the tool REFUSED all four, with the deviation
            // printed as `+0.000` right beside the word FAIL. An acceptance
            // test that rejects a perfect result does not fail safe: it sends
            // the next arm off to build the ruler by hand.
            Some(v) => (want - v) >= 0 && (want - v) <= MATCH_TOL_MS,
        };
        out[i].time = t;
        out[i].verified = ok;
        if !ok {
            bad.push(format!(
                "    seg{:<3} {:<52} got {:>12}  want {:>10}  ({})",
                out[i].segment,
                out[i].cut,
                crate::secs::opt(t),
                crate::secs::ms(want),
                out[i].method
            ));
        }
    }
    if verbose {
        println!("  verification against {}:", ref_ghost.file_name().unwrap().to_string_lossy());
        for s in &out {
            println!(
                "    seg{:<3} {:>12} want {:>10}  {:>9}  {}  {}",
                s.segment,
                crate::secs::opt(s.time),
                crate::secs::ms(s.expect),
                signed(s.time.map(|v| v - s.expect).unwrap_or(0)),
                if s.verified { "OK  " } else { "FAIL" },
                s.method
            );
        }
    }
    let json: Vec<String> = out
        .iter()
        .map(|s| {
            format!(
                "  {{\n    \"segment\": {},\n    \"map\": \"{}\",\n    \"cut\": \"{}\",\n    \"method\": \"{}\",\n    \"exact\": {},\n    \"time\": {},\n    \"expect\": {},\n    \"verified\": {},\n    \"body_fnv1a64\": \"{:016x}\"\n  }}",
                s.segment,
                s.map.display(),
                s.cut.replace('"', "'"),
                s.method,
                s.exact,
                match s.time { None => "null".to_string(), Some(v) => v.to_string() },
                s.expect,
                s.verified,
                s.body_hash
            )
        })
        .collect();
    // written BEFORE the refusal below: a caller that gets exit 2 needs the
    // per-segment table more than a caller that gets exit 0 does.
    std::fs::write(
        out_dir.join("segments.json"),
        format!("[\n{}\n]\n", json.join(",\n")),
    )
    .unwrap();
    if !bad.is_empty() {
        return Err(format!(
            "REFUSING: {} of {} segment maps do not reproduce the reference ghost's own \
             declared split.\n{}\n  the maps and segments.json are left in {} for diagnosis \
             (`verified` is per segment, so the good ones are usable and named). An exact cut \
             must match to the millisecond; the block-rename fallback must fire EARLY by at \
             most {}.",
            bad.len(),
            out.len(),
            bad.join("\n"),
            out_dir.display(),
            crate::secs::ms(MATCH_TOL_MS)
        ));
    }

    Ok(out)
}

/// `make_all_ordered` with the order measured. Kept because the acceptance
/// tests and `probe.rs` call it by this name.
pub fn make_all(
    src: &Path,
    out_dir: &Path,
    ref_ghost: &Path,
    jobs: usize,
    server: &str,
    verbose: bool,
) -> Result<Vec<Segment>, String> {
    make_all_ordered(src, out_dir, ref_ghost, jobs, server, verbose, None)
}
