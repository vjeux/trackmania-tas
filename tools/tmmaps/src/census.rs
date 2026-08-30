//! The map's contents, and the surgery that operates on a REGION rather than
//! on one index.
//!
//! ## A gate is a structure, not a block
//!
//! On 173691 the map's author had added a finish gate. It reads, in every
//! listing this project had, as one unbaked `GateExpandable*` anchor at
//! (1311, 113, 434). Moving that anchor — which is what the first pass did —
//! produced a map that loaded, an origin control that passed, an oracle that
//! answered, and a car that drove into the gate and stopped 77.8 m onto the
//! deck. The gate is **sixteen** blocks: the anchor plus fifteen **baked**
//! pieces spanning x 1271…1375, y 96…121, z 412…466. vjeux saw the car bump
//! the invisible remains in the video before any instrument reported it.
//!
//! So the operation people actually want is not "move block N". It is "empty
//! this region and prove it is empty". `region` counts, `clear` moves and then
//! **re-reads the file it wrote** and fails while anything is left.
//!
//! ## Baked blocks move if — and only if — they are FREE
//!
//! The mover used to refuse every baked index with "baked terrain is not
//! relocatable". That is right for a *cell* move: a baked block's cell bytes
//! are dead, and a baked index pasted from a census row is not the same block
//! as the unbaked block of that number. It is **wrong** for a free baked
//! block, whose position is six f32 in chunk `0x0304305F` exactly like an
//! unbaked free block's — and fifteen of 173691's sixteen gate pieces are
//! precisely that. Refusing them is what made the first pass move one block of
//! sixteen and believe it had moved the gate.
//!
//! `Move::Baked` is therefore now a refusal with a *reason that can be
//! satisfied*: a baked block is movable by position (`bN@x,y,z`) and never by
//! cell.

use crate::map::{BlockRec, ItemRec, MapFile, FREE_BLOCK_FLAG};
use std::path::{Path, PathBuf};

/// An axis-aligned world box, in metres.
#[derive(Clone, Copy, Debug)]
pub struct Box3 {
    pub lo: [f32; 3],
    pub hi: [f32; 3],
}

impl Box3 {
    /// `X0,Y0,Z0:X1,Y1,Z1`; the two corners may be given in any order.
    pub fn parse(s: &str) -> Box3 {
        let (a, b) = s.split_once(':').expect("--box X0,Y0,Z0:X1,Y1,Z1");
        let p = |t: &str| -> [f32; 3] {
            let v: Vec<f32> = t.split(',').map(|x| x.trim().parse().expect("a number")).collect();
            assert_eq!(v.len(), 3, "--box corners are x,y,z");
            [v[0], v[1], v[2]]
        };
        let (a, b) = (p(a), p(b));
        Box3 {
            lo: [a[0].min(b[0]), a[1].min(b[1]), a[2].min(b[2])],
            hi: [a[0].max(b[0]), a[1].max(b[1]), a[2].max(b[2])],
        }
    }
    /// Everything, for a control that has to find an object wherever it went.
    pub const WORLD: Box3 = Box3 { lo: [-1e9; 3], hi: [1e9; 3] };
    pub fn holds(&self, p: [f32; 3]) -> bool {
        (0..3).all(|i| p[i] >= self.lo[i] && p[i] <= self.hi[i])
    }
}

/// Where a thing is, in metres, and how sure we are of that.
///
/// A grid block has no stored position at all: the cell is the placement, and
/// the world point below is the cell's own centre-of-floor. Saying so matters,
/// because a 32 m cell is a coarse answer to "is it in this box".
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Placement {
    /// six f32 in chunk 0x0304305F — the exact position
    Free,
    /// derived from the world cell; accurate to the cell, not to the metre
    Cell,
}

#[derive(Clone, Debug)]
pub struct Entry {
    /// `123`, `b123`, `i123` — the form a mover spec takes
    pub id: String,
    pub name: String,
    pub pos: [f32; 3],
    pub placement: Placement,
    pub baked: bool,
    pub item: bool,
    pub waypoint: Option<String>,
}

/// A grid block's cell, as a world point: the horizontal centre of the cell at
/// its floor. `8*cy - 62` is the map-independent part of the vertical formula
/// used everywhere else in this tool.
pub fn cell_world(b: &BlockRec) -> [f32; 3] {
    let (cx, cy, cz) = b.coords();
    [
        cx as f32 * crate::map::CELL_XZ + crate::map::CELL_XZ / 2.0,
        cy as f32 * crate::map::CELL_Y - 62.0,
        cz as f32 * crate::map::CELL_XZ + crate::map::CELL_XZ / 2.0,
    ]
}

fn block_entry(b: &BlockRec, baked: bool) -> Entry {
    let (pos, placement) = match b.free_pos {
        Some(p) => (p, Placement::Free),
        None => (cell_world(b), Placement::Cell),
    };
    Entry {
        id: if baked { format!("b{}", b.index) } else { format!("{}", b.index) },
        name: b.name.clone(),
        pos,
        placement,
        baked,
        item: false,
        waypoint: b.waypoint_tag.clone(),
    }
}

fn item_entry(it: &ItemRec) -> Entry {
    Entry {
        id: format!("i{}", it.index),
        name: it.model.clone(),
        pos: it.pos,
        placement: Placement::Free,
        baked: false,
        item: true,
        waypoint: it.waypoint_tag.clone(),
    }
}

/// Every block (unbaked then baked) and every item, in mover-spec form.
pub fn entries(m: &MapFile) -> Vec<Entry> {
    let mut out: Vec<Entry> = Vec::with_capacity(m.blocks.len() + m.baked.len() + m.items.len());
    out.extend(m.blocks.iter().map(|b| block_entry(b, false)));
    out.extend(m.baked.iter().map(|b| block_entry(b, true)));
    out.extend(m.items.iter().map(item_entry));
    out
}

fn filter_of(args: &[String]) -> Option<String> {
    crate::cli::flag(args, "--filter").map(|s| s.to_string())
}

fn matches(e: &Entry, pat: &Option<String>) -> bool {
    match pat {
        None => true,
        Some(p) => e.name.contains(p.as_str()),
    }
}

// ---------------------------------------------------------------- census

pub fn cmd_census(args: &[String]) {
    let m = MapFile::load(Path::new(&args[2]));
    let pat = filter_of(args);
    let free_only = crate::cli::has(args, "--free");
    println!("src\tid\tname\tcx\tcy\tcz\tflags\tplacement\tx\ty\tz\trx\try\trz\twp");
    let rows = m
        .blocks
        .iter()
        .map(|b| ("U", b, false))
        .chain(m.baked.iter().map(|b| ("B", b, true)));
    let (mut n, mut nfree) = (0usize, 0usize);
    for (src, b, baked) in rows {
        let e = block_entry(b, baked);
        if !matches(&e, &pat) {
            continue;
        }
        let is_free = b.flags & FREE_BLOCK_FLAG != 0;
        if free_only && !is_free {
            continue;
        }
        n += 1;
        if is_free {
            nfree += 1;
        }
        let c = b.coords();
        println!(
            "{}\t{}\t{}\t{}\t{}\t{}\t{:08X}\t{}\t{:.3}\t{:.3}\t{:.3}\t{:.4}\t{:.4}\t{:.4}\t{}",
            src,
            e.id,
            b.name,
            c.0,
            c.1,
            c.2,
            b.flags,
            if is_free { "FREE" } else { "grid" },
            e.pos[0],
            e.pos[1],
            e.pos[2],
            b.free_rot.unwrap_or([0.0;3])[0],
            b.free_rot.unwrap_or([0.0;3])[1],
            b.free_rot.unwrap_or([0.0;3])[2],
            b.waypoint_tag.clone().unwrap_or_default()
        );
    }
    for it in &m.items {
        let e = item_entry(it);
        if !matches(&e, &pat) {
            continue;
        }
        n += 1;
        nfree += 1;
        let c = it.coords();
        println!(
            "I\t{}\t{}\t{}\t{}\t{}\t{:08X}\tITEM\t{:.3}\t{:.3}\t{:.3}\t{:.4}\t{:.4}\t{:.4}\t{}",
            e.id,
            it.model,
            c.0,
            c.1,
            c.2,
            0,
            e.pos[0],
            e.pos[1],
            e.pos[2],
            it.yaw,
            it.pitch,
            it.roll,
            it.waypoint_tag.clone().unwrap_or_default()
        );
    }
    eprintln!(
        "rows={} of blocks={} baked={} items={}; {} carry a real position (the rest are grid \
         cells, accurate to 32 m)",
        n,
        m.blocks.len(),
        m.baked.len(),
        m.items.len(),
        nfree
    );
}

// ---------------------------------------------------------------- region

/// Everything inside the box. Returns the entries so `clear` can use the same
/// answer the human sees.
pub fn in_box(m: &MapFile, b: Box3, pat: &Option<String>) -> Vec<Entry> {
    entries(m)
        .into_iter()
        .filter(|e| matches(e, pat) && b.holds(e.pos))
        .collect()
}

fn print_region(found: &[Entry]) {
    println!("id\tname\tplacement\tx\ty\tz\twaypoint");
    for e in found {
        println!(
            "{}\t{}\t{}\t{:.3}\t{:.3}\t{:.3}\t{}",
            e.id,
            e.name,
            if e.placement == Placement::Free { "free" } else { "cell" },
            e.pos[0],
            e.pos[1],
            e.pos[2],
            e.waypoint.clone().unwrap_or_default()
        );
    }
}

pub fn cmd_region(args: &[String]) {
    let m = MapFile::load(Path::new(&args[2]));
    let b = Box3::parse(crate::cli::flag(args, "--box").expect("--box X0,Y0,Z0:X1,Y1,Z1"));
    let pat = filter_of(args);
    let mut found = in_box(&m, b, &pat);
    if crate::cli::has(args, "--items") {
        found.retain(|e| e.item);
    }
    if crate::cli::has(args, "--blocks") {
        found.retain(|e| !e.item);
    }
    print_region(&found);
    let cellish = found.iter().filter(|e| e.placement == Placement::Cell).count();
    eprintln!(
        "{} in the box ({} free/exact, {} placed by 32 m cell)",
        found.len(),
        found.len() - cellish,
        cellish
    );
}

// ---------------------------------------------------------------- clear

/// Move everything in `b` to `to`, then RE-READ the written map and require
/// the box to be empty.
///
/// The re-read is the whole point. Every earlier version of this operation
/// reported what it intended to do; this one reports what the file it wrote
/// actually contains. Exit 3 if anything is left.
pub fn cmd_shift(args: &[String]) {
    let src = PathBuf::from(&args[2]);
    let out = PathBuf::from(crate::cli::flag(args, "--out").expect("--out F"));
    let b = Box3::parse(crate::cli::flag(args, "--box").expect("--box X0,Y0,Z0:X1,Y1,Z1"));
    let by: Vec<f32> = crate::cli::flag(args, "--by")
        .expect("--by DX,DY,DZ (metres to displace what is in the box)")
        .split(',')
        .map(|x| x.trim().parse().expect("a number"))
        .collect();
    assert_eq!(by.len(), 3, "--by is dx,dy,dz in metres");
    let by = [by[0], by[1], by[2]];
    let pat = filter_of(args);

    let mut m = MapFile::load(&src);
    let found = in_box(&m, b, &pat);
    println!("in the box before:");
    print_region(&found);

    // Positions BEFORE, keyed by id, so the control can require each object to
    // have moved by exactly `by` -- not merely "the box is different now".
    let mut want: Vec<(String, [f32; 3])> = Vec::new();
    let mut stuck: Vec<String> = Vec::new();
    for e in &found {
        if e.placement == Placement::Cell {
            stuck.push(format!(
                "{} {} is a GRID block: it has no stored position, so `shift` cannot move it.",
                e.id, e.name
            ));
            continue;
        }
        let p = [e.pos[0] + by[0], e.pos[1] + by[1], e.pos[2] + by[2]];
        if e.item {
            let i: usize = e.id[1..].parse().unwrap();
            m.move_item_pos(i, p);
        } else if e.baked {
            let i: usize = e.id[1..].parse().unwrap();
            m.move_baked_free(i, p);
        } else {
            let i: usize = e.id.parse().unwrap();
            m.move_block_free(i, p);
        }
        want.push((e.id.clone(), p));
    }
    m.write_to(&out).expect("write shifted map");

    // THE CONTROL, on the map as WRITTEN: every object must be where it was
    // asked to go. A structure half-moved is the 173691 failure -- the run
    // drives into the pieces that stayed.
    let after = MapFile::load(&out);
    let now = in_box(&after, Box3::WORLD, &pat);
    let mut wrong = 0usize;
    for (id, p) in &want {
        match now.iter().find(|e| &e.id == id) {
            Some(e) => {
                let d = ((e.pos[0] - p[0]).powi(2)
                    + (e.pos[1] - p[1]).powi(2)
                    + (e.pos[2] - p[2]).powi(2))
                .sqrt();
                if d > 1e-3 {
                    println!("  WRONG: {} wanted {:?}, got {:?}", id, p, e.pos);
                    wrong += 1;
                }
            }
            None => {
                println!("  LOST: {} is not in the written map at all", id);
                wrong += 1;
            }
        }
    }
    println!(
        "\nshifted {} of {} by {:?}; wrote {}",
        want.len(),
        found.len(),
        by,
        out.display()
    );
    for s in &stuck {
        println!("  refused: {}", s);
    }
    if wrong > 0 || !stuck.is_empty() {
        eprintln!(
            "REFUSING: {} object(s) did not land where they were sent and {} could not be \
             moved at all. A partially shifted structure is worse than an unshifted one, \
             because it looks like a measurement.",
            wrong,
            stuck.len()
        );
        std::process::exit(3);
    }
    println!("every object in the box moved by exactly {:?} in the written file", by);
}

pub fn cmd_clear(args: &[String]) {
    let src = PathBuf::from(&args[2]);
    let out = PathBuf::from(crate::cli::flag(args, "--out").expect("--out F"));
    let b = Box3::parse(crate::cli::flag(args, "--box").expect("--box X0,Y0,Z0:X1,Y1,Z1"));
    let to: Vec<f32> = crate::cli::flag(args, "--to")
        .expect("--to X,Y,Z (where to park what is in the box)")
        .split(',')
        .map(|x| x.trim().parse().expect("a number"))
        .collect();
    assert_eq!(to.len(), 3, "--to is x,y,z in metres");
    let to = [to[0], to[1], to[2]];
    assert!(!b.holds(to), "--to is INSIDE --box; that clears nothing");
    let pat = filter_of(args);

    let mut m = MapFile::load(&src);
    let found = in_box(&m, b, &pat);
    println!("in the box before:");
    print_region(&found);

    let mut moved = 0usize;
    let mut stuck: Vec<String> = Vec::new();
    for e in &found {
        if e.placement == Placement::Cell {
            // A grid block's placement is its cell, and a cell move is a
            // different operation with different failure modes. Say so rather
            // than half-doing it.
            stuck.push(format!(
                "{} {} is a GRID block: it has no stored position, so `clear` cannot move it. \
                 Move it by cell with `tmmaps move --move {}:cx,cy,cz`.",
                e.id, e.name, e.id
            ));
            continue;
        }
        if e.item {
            let i: usize = e.id[1..].parse().unwrap();
            m.move_item_pos(i, to);
        } else if e.baked {
            let i: usize = e.id[1..].parse().unwrap();
            m.move_baked_free(i, to);
        } else {
            let i: usize = e.id.parse().unwrap();
            m.move_block_free(i, to);
        }
        moved += 1;
    }
    let sp = m.write_to_reporting(&out).expect("write cleared map");
    println!("  {}", sp.summary());

    // THE CONTROL: read back the map we just wrote, not the one in memory.
    let after = MapFile::load(&out);
    let left = in_box(&after, b, &pat);
    println!("\nmoved {} of {} to {:?}; wrote {}", moved, found.len(), to, out.display());
    if !left.is_empty() {
        println!("\nSTILL IN THE BOX AFTER THE WRITE:");
        print_region(&left);
    }
    for s in &stuck {
        println!("  refused: {}", s);
    }
    if !left.is_empty() || !stuck.is_empty() {
        eprintln!(
            "REFUSING: {} of {} still in the box. A GATE IS A STRUCTURE, NOT A BLOCK — moving \
             the anchor and leaving the pieces is the 173691 failure, and the run drives into \
             what is left.",
            left.len(),
            found.len()
        );
        std::process::exit(3);
    }
    println!("box is empty in the written file: {} objects, 0 left", found.len());
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn box_parses_either_corner_order() {
        let a = Box3::parse("10,0,10:0,-5,20");
        assert_eq!(a.lo, [0.0, -5.0, 10.0]);
        assert_eq!(a.hi, [10.0, 0.0, 20.0]);
        assert!(a.holds([5.0, -1.0, 15.0]));
        assert!(!a.holds([5.0, 1.0, 15.0]));
        // the faces are inside: a gate piece sitting exactly on the boundary of
        // a box a human typed off a census listing must not be missed
        assert!(a.holds([0.0, -5.0, 10.0]));
        assert!(a.holds([10.0, 0.0, 20.0]));
    }
}
