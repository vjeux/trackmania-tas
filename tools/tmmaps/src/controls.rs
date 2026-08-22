//! The controls. Each answers a question you must be able to ask before you
//! believe a map-surgery result, and each is cheap enough that there is no
//! excuse for skipping it.

use crate::gbx;
use crate::map::MapFile;
use std::path::Path;

pub struct OriginReport {
    pub checked: usize,
    pub failures: usize,
    /// names of the objects whose own mover did not reproduce the file
    pub failed: Vec<String>,
}

/// One object and the mover that owns it.
#[derive(Clone, Debug)]
enum Mover {
    GridBlock(usize),
    FreeBlock(usize),
    BakedFree(usize),
    Item(usize),
}

impl Mover {
    /// Apply this object's own mover with the object's own current placement.
    fn replay(&self, m: &mut MapFile, m0: &MapFile) {
        match *self {
            Mover::GridBlock(i) => {
                m.move_block_cell(i, m0.blocks[i].coords());
                m.set_block_dir(i, m0.blocks[i].dir);
            }
            Mover::FreeBlock(i) => {
                m.move_block_free(i, m0.blocks[i].free_pos.unwrap());
                m.set_block_free_rot(i, m0.blocks[i].free_rot.unwrap());
            }
            Mover::BakedFree(i) => m.move_baked_free(i, m0.baked[i].free_pos.unwrap()),
            Mover::Item(i) => {
                m.move_item_pos(i, m0.items[i].pos);
                m.set_item_yaw(i, m0.items[i].yaw);
            }
        }
    }
    fn label(&self, m0: &MapFile) -> String {
        match *self {
            Mover::GridBlock(i) => format!("block#{} grid {}", i, m0.blocks[i].name),
            Mover::FreeBlock(i) => format!("block#{} free {}", i, m0.blocks[i].name),
            Mover::BakedFree(i) => format!("b{} baked-free {}", i, m0.baked[i].name),
            Mover::Item(i) => format!("item#{} {}", i, m0.items[i].model),
        }
    }
}

fn movers(m0: &MapFile) -> Vec<Mover> {
    let mut v = Vec::new();
    for b in &m0.blocks {
        if b.free_off.is_some() {
            v.push(Mover::FreeBlock(b.index));
        } else if b.waypoint_tag.is_some() {
            // A non-waypoint GRID block has no mover of its own — nothing in
            // this tool moves one — so exercising it would test nothing.
            v.push(Mover::GridBlock(b.index));
        }
    }
    for b in &m0.baked {
        if b.free_off.is_some() {
            v.push(Mover::BakedFree(b.index));
        }
    }
    for it in &m0.items {
        v.push(Mover::Item(it.index));
    }
    v
}

/// RETURN TO ORIGIN, at the byte level.
///
/// For every waypoint block, every free block (unbaked and baked) and every
/// item, drive the SAME mover the ladder uses — including its rotation — with
/// the object's own current placement, and require the rebuilt body to be
/// byte-identical to the untouched one.
///
/// What it catches, in one command and no oracle calls, is the failure this
/// map class actually produces: **a mover writing bytes the game never reads.**
/// What it cannot catch is a trigger-VOLUME change, which is exactly why every
/// mover it exercises is position-only, and why the model-swapping `gate` /
/// `gateat` / `probe` commands were deleted rather than controlled.
///
/// ## Why it runs every mover at once first
///
/// The old version re-parsed the whole map once per object. On map 1 that is
/// five parses; on 173691 it is 91 693, and the control simply never finished
/// — which in practice means it is not run, which is the same as not existing.
/// So: apply **all** the movers to one copy and require byte identity. That
/// single build is a strictly stronger claim than the per-object loop (it also
/// catches two movers that interfere), and it is one parse. Only when it fails
/// does the per-object loop run, to say which mover is at fault, and only then
/// is the quadratic cost worth paying.
pub fn origin(src: &Path, verbose: bool) -> OriginReport {
    let m0 = MapFile::load(src);
    let base = m0.gbx.body.clone();
    let all = movers(&m0);

    let mut m = MapFile::load(src);
    for mv in &all {
        mv.replay(&mut m, &m0);
    }
    if gbx::Gbx::parse(&m.build()).body == base {
        if verbose {
            println!("all {} movers replayed at once: byte-identical", all.len());
        }
        return OriginReport { checked: all.len(), failures: 0, failed: Vec::new() };
    }

    // Something moved. Now pay for the bisection.
    println!("the combined replay is NOT byte-identical — isolating");
    let mut r = OriginReport { checked: 0, failures: 0, failed: Vec::new() };
    for mv in &all {
        let mut m = MapFile::load(src);
        mv.replay(&mut m, &m0);
        let ok = gbx::Gbx::parse(&m.build()).body == base;
        r.checked += 1;
        if !ok {
            r.failures += 1;
            r.failed.push(mv.label(&m0));
        }
        if verbose || !ok {
            println!("{:<56} identical={}", mv.label(&m0), ok);
        }
    }
    if r.failures == 0 {
        // Every mover is fine alone and the combination is not: that is
        // interference, and it is a real failure even though no single row
        // shows it.
        r.failures = 1;
        r.failed.push(
            "no single mover fails, but applying them together does — two movers write the same \
             bytes"
                .to_string(),
        );
    }
    r
}

pub fn cmd_origin(args: &[String]) {
    let src = Path::new(&args[2]);
    let r = origin(src, crate::has(args, "--verbose"));
    println!("origin control: {} movers, {} failures", r.checked, r.failures);
    for f in &r.failed {
        println!("  {}", f);
    }
    if r.failures > 0 {
        std::process::exit(1);
    }
}

/// Parse and re-emit unchanged.
///
/// Compares DECOMPRESSED bodies. LZO recompression is not bit-reproducible, so
/// a file hash is the wrong level and would fail on a correct writer.
pub fn cmd_roundtrip(args: &[String]) {
    let m = MapFile::load(Path::new(&args[2]));
    let out = m.build();
    let g2 = gbx::Gbx::parse(&out);
    let ok = g2.body == m.gbx.body && g2.num_nodes == m.gbx.num_nodes;
    println!(
        "body {} -> {} numNodes {} -> {} identical={}",
        m.gbx.body.len(),
        g2.body.len(),
        m.gbx.num_nodes,
        g2.num_nodes,
        ok
    );
    if !ok {
        std::process::exit(1);
    }
}
