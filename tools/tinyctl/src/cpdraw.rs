//! `tinyctl checkpoint-draw MAP --out PNG [--px 40] [--slots SLOTS.tsv]` — the top-down of Hugo's Checkpoint
//! Catalogue: every authored grid block as its turned footprint box (coloured by group: North copy / East copy /
//! South copy / start / finish / plain replacement), the block name lettered inside (two lines when long), the
//! cell grid, a legend. Works on the catalogue and on its cleaned twin (then the names are the replacements).
use std::collections::BTreeMap;

use crate::flagdraw::{Canvas, INK};

const NORTH: [u8; 3] = [120, 170, 230];
const EAST: [u8; 3] = [235, 180, 110];
const SOUTH: [u8; 3] = [170, 210, 130];
const START: [u8; 3] = [90, 200, 90];
const FINISH: [u8; 3] = [220, 80, 80];
const PLAIN: [u8; 3] = [200, 200, 200];
const GRID: [u8; 3] = [222, 222, 222];
const GRASS: [u8; 3] = [236, 242, 228];

fn f(rest: &[String], k: &str) -> Option<String> {
    rest.iter().position(|a| a == k).and_then(|i| rest.get(i + 1).cloned())
}

/// Split a block name into ≤ `width`-character lines at capital letters.
fn wrap_name(name: &str, width: usize) -> Vec<String> {
    let mut words: Vec<String> = Vec::new();
    let mut cur = String::new();
    for ch in name.chars() {
        if ch.is_ascii_uppercase() && !cur.is_empty() && cur.chars().last().map_or(false, |c| !c.is_ascii_uppercase()) {
            words.push(std::mem::take(&mut cur));
        }
        cur.push(ch);
    }
    if !cur.is_empty() {
        words.push(cur);
    }
    let mut lines: Vec<String> = Vec::new();
    for w in words {
        match lines.last_mut() {
            Some(l) if l.len() + w.len() <= width => l.push_str(&w),
            _ => lines.push(w),
        }
    }
    lines
}

pub fn cmd(rest: &[String]) -> Result<(), String> {
    let map = rest.first().cloned().ok_or("checkpoint-draw needs a MAP")?;
    let out = f(rest, "--out").unwrap_or_else(|| "checkpoint-catalogue.png".into());
    let px: f32 = f(rest, "--px").and_then(|v| v.parse().ok()).unwrap_or(40.0);
    let m = tmmaps::map::MapFile::try_load(std::path::Path::new(&map))?;
    // the group per (cell) from the slots table when given (the catalogue), else by waypoint tag
    let mut groups: BTreeMap<(i32, i32), String> = BTreeMap::new();
    if let Some(sl) = f(rest, "--slots") {
        for line in std::fs::read_to_string(&sl).map_err(|e| format!("{sl}: {e}"))?.lines().skip(1) {
            let c: Vec<&str> = line.split('\t').collect();
            if c.len() >= 6 {
                groups.insert((c[2].parse().unwrap_or(0), c[4].parse().unwrap_or(0)), c[1].to_string());
            }
        }
    }
    let sizes = {
        // footprint sizes from the file's own block sizes table when banked beside the mapping, else 1×1 / 3×2 by name
        let mut s: BTreeMap<String, (i32, i32)> = BTreeMap::new();
        if let Some(t) = f(rest, "--sizes") {
            for line in std::fs::read_to_string(&t).map_err(|e| format!("{t}: {e}"))?.lines().skip(1) {
                let c: Vec<&str> = line.split('\t').collect();
                if c.len() >= 3 {
                    s.insert(c[0].to_string(), (c[1].parse().unwrap_or(1), c[2].parse().unwrap_or(1)));
                }
            }
        }
        s
    };
    let (nx, nz) = (m.size[0], m.size[2]);
    let legend_w = 420usize;
    let (w, h) = ((nx as f32 * px) as usize + legend_w, (nz as f32 * px) as usize);
    let mut cv = Canvas::new(w, h.max(700));
    cv.poly(&[(0.0, 0.0), (nx as f32 * px, 0.0), (nx as f32 * px, nz as f32 * px), (0.0, nz as f32 * px)], GRASS, 1.0);
    for x in 0..=nx {
        cv.line(x as f32 * px, 0.0, x as f32 * px, nz as f32 * px, GRID, 1.0, 0.8);
    }
    for z in 0..=nz {
        cv.line(0.0, z as f32 * px, nx as f32 * px, z as f32 * px, GRID, 1.0, 0.8);
    }
    let mut counts: BTreeMap<String, usize> = BTreeMap::new();
    for b in m.blocks.iter().filter(|b| b.flags != 0xFFFF_FFFF && b.free_pos.is_none()) {
        let c = b.coords();
        let size = sizes.get(&b.name).copied().unwrap_or_else(|| if b.name.contains("Diag") && !b.name.contains("Diag1") { (3, 2) } else { (1, 1) });
        let (sx, sz) = if b.dir & 1 == 1 { (size.1, size.0) } else { size };
        let group = groups.get(&(c.0, c.2)).cloned().unwrap_or_else(|| match b.waypoint_tag.as_deref() {
            Some("Spawn") => "start".into(),
            Some("Goal") => "finish".into(),
            Some("Checkpoint") => "north".into(),
            _ => "plain".into(),
        });
        let col = match group.as_str() {
            "start" => START,
            "finish" => FINISH,
            "north" => NORTH,
            "east" | "marker-east" => EAST,
            "south" => SOUTH,
            _ => PLAIN,
        };
        *counts.entry(group.clone()).or_default() += 1;
        let (x0, z0) = (c.0 as f32 * px, c.2 as f32 * px);
        let (x1, z1) = (x0 + sx as f32 * px, z0 + sz as f32 * px);
        cv.poly(&[(x0 + 1.0, z0 + 1.0), (x1 - 1.0, z0 + 1.0), (x1 - 1.0, z1 - 1.0), (x0 + 1.0, z1 - 1.0)], col, 0.85);
        cv.outline(&[(x0 + 1.0, z0 + 1.0), (x1 - 1.0, z0 + 1.0), (x1 - 1.0, z1 - 1.0), (x0 + 1.0, z1 - 1.0)], [90, 90, 90], 1.0);
        // the direction: a tick on the block's "front" edge (dir 0 = +z… the file dir turns clockwise)
        let (tx, tz) = match b.dir & 3 {
            0 => ((x0 + x1) / 2.0, z1 - 4.0),
            1 => (x0 + 4.0, (z0 + z1) / 2.0),
            2 => ((x0 + x1) / 2.0, z0 + 4.0),
            _ => (x1 - 4.0, (z0 + z1) / 2.0),
        };
        cv.poly(&[(tx - 3.0, tz - 3.0), (tx + 3.0, tz - 3.0), (tx + 3.0, tz + 3.0), (tx - 3.0, tz + 3.0)], INK, 1.0);
        let width_chars = ((sx as f32 * px - 6.0) / 6.0).max(4.0) as usize;
        let lines = wrap_name(&b.name, width_chars);
        for (i, l) in lines.iter().take(((sz as f32 * px - 6.0) / 10.0).max(1.0) as usize).enumerate() {
            cv.text(x0 + 3.0, z0 + 3.0 + 10.0 * i as f32, l, 1, INK);
        }
    }
    let lx = nx as f32 * px + 16.0;
    let mut ly = 16.0;
    cv.text(lx, ly, "Checkpoint Catalogue", 2, INK);
    ly += 28.0;
    cv.text(lx, ly, &format!("{} x {} cells, {} blocks", nx, nz, m.blocks.iter().filter(|b| b.flags != 0xFFFF_FFFF).count()), 1, INK);
    ly += 22.0;
    for (name, col, key) in [("start (Spawn)", START, "start"), ("checkpoint, North copy (dir 0)", NORTH, "north"), ("checkpoint, East copy (dir 1) + marker", EAST, "east"), ("multi-cell, South copy (dir 2)", SOUTH, "south"), ("finish (Goal)", FINISH, "finish"), ("plain replacement (cleaned map)", PLAIN, "plain")] {
        cv.poly(&[(lx, ly), (lx + 22.0, ly), (lx + 22.0, ly + 14.0), (lx, ly + 14.0)], col, 0.85);
        let n = counts.get(key).copied().unwrap_or(0) + if key == "east" { counts.get("marker-east").copied().unwrap_or(0) } else { 0 };
        cv.text(lx + 30.0, ly + 1.0, &format!("{name}: {n}"), 1, INK);
        ly += 20.0;
    }
    ly += 10.0;
    cv.text(lx, ly, "black tick = the block's facing edge (file dir)", 1, [90, 90, 90]);
    std::fs::write(&out, crate::png::encode(&cv.img)).map_err(|e| format!("{out}: {e}"))?;
    println!("wrote {out}: {w}x{} px, {} blocks drawn", h.max(700), m.blocks.iter().filter(|b| b.flags != 0xFFFF_FFFF && b.free_pos.is_none()).count());
    Ok(())
}
