//! What is under the car when the model has nothing there.
//!
//! A coverage number says the model is missing surface; it does not say what
//! surface. This turns each `Class::Missing` sample into the map's own record
//! for the cell the car was over — the block or item the author placed there,
//! and whether that model produced any triangles. The output is a list of
//! model names ordered by how many samples of the run they cost, which is the
//! work queue: the top line is the single block worth reading next.
//!
//! A cell with no record at all is reported too, and means something
//! different: the surface the car was on is not in the map file, so it is the
//! decoration's, or the car was outside the map.

use crate::coverage::{Class, Verdict};
use crate::place::{CELL_XZ, CELL_Y};
use std::collections::BTreeMap;
use tmmaps::map::{MapFile, FREE_BLOCK_FLAG};

/// One record the map places, reduced to where it is and whether it drew.
struct Placed {
    name: String,
    cell: (i32, i32, i32),
    drew: bool,
}

pub struct Blame {
    /// model name -> samples it could account for; `""` is "no record here"
    pub by_model: BTreeMap<String, usize>,
    /// how many missing samples had no map record in their cell column at all
    pub empty_cells: usize,
    pub total: usize,
}

/// `used` is the assembler's model -> (placements, produced geometry) table.
pub fn of(
    m: &MapFile,
    used: &BTreeMap<String, (usize, bool)>,
    v: &Verdict,
    points: &[[f32; 3]],
    yoff: f32,
) -> Blame {
    let drew = |name: &String| used.get(name).map(|(_, ok)| *ok).unwrap_or(false);
    let mut placed: Vec<Placed> = Vec::new();
    for b in &m.blocks {
        let cell = if b.flags & FREE_BLOCK_FLAG != 0 {
            match b.free_pos {
                Some(p) => cell_of(p, yoff),
                None => continue,
            }
        } else {
            b.coords()
        };
        placed.push(Placed { name: b.name.clone(), cell, drew: drew(&b.name) });
    }
    for it in &m.items {
        placed.push(Placed {
            name: it.model.clone(),
            cell: cell_of(it.pos, yoff),
            drew: drew(&it.model),
        });
    }

    // (cx, cz) -> the records in that column, so a lookup is one hash.
    let mut column: BTreeMap<(i32, i32), Vec<&Placed>> = BTreeMap::new();
    for p in &placed {
        column.entry((p.cell.0, p.cell.2)).or_default().push(p);
    }

    let mut b = Blame { by_model: BTreeMap::new(), empty_cells: 0, total: 0 };
    for (i, c) in v.classes.iter().enumerate() {
        if *c != Class::Missing {
            continue;
        }
        b.total += 1;
        let (cx, cy, cz) = cell_of(points[i], yoff);
        // The 3x3 of columns around the sample: a block's anchor cell is not
        // always the cell the car is over (a curve's anchor is a corner of its
        // footprint), and a 32 m cell is small next to a three-cell block.
        let mut best: Option<&Placed> = None;
        let mut best_key = (i32::MAX, 0i32);
        for dz in -1..=1 {
            for dx in -1..=1 {
                for p in column.get(&(cx + dx, cz + dz)).into_iter().flatten() {
                    // Prefer a record at the car's own height, then a nearer
                    // column, and among equals prefer one that drew NOTHING —
                    // that is the one that can explain a hole.
                    let key = ((p.cell.1 - cy).abs() + dx.abs() + dz.abs(), p.drew as i32);
                    if key < best_key {
                        best_key = key;
                        best = Some(p);
                    }
                }
            }
        }
        match best {
            Some(p) => {
                let tag = if p.drew {
                    format!("{} (has geometry)", p.name)
                } else {
                    p.name.clone()
                };
                *b.by_model.entry(tag).or_insert(0) += 1;
            }
            None => {
                b.empty_cells += 1;
                *b.by_model.entry(String::new()).or_insert(0) += 1;
            }
        }
    }
    b
}

fn cell_of(p: [f32; 3], yoff: f32) -> (i32, i32, i32) {
    (
        (p[0] / CELL_XZ).floor() as i32,
        ((p[1] - yoff) / CELL_Y).floor() as i32,
        (p[2] / CELL_XZ).floor() as i32,
    )
}

impl Blame {
    /// Model names ordered by the samples they cost, worst first.
    pub fn ranked(&self) -> Vec<(&str, usize)> {
        let mut v: Vec<(&str, usize)> =
            self.by_model.iter().map(|(k, n)| (k.as_str(), *n)).collect();
        v.sort_by_key(|(_, n)| std::cmp::Reverse(*n));
        v
    }
}
