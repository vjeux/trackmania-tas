//! `tmmaps ghost-touch` — does a driven record actually touch the blocks a
//! surface conversion changes?
//!
//! Everios96 2026-10-08, before the 975 × 6 republish: "check the records of
//! these maps and analyze the ghost of the records, and see if they touch any
//! of the surfaces we are changing". A map whose records never touch a
//! converted block needs no record wipe; one whose records do, does.
//!
//! ```text
//! tmmaps ghost-touch MAP.Map.Gbx --ghost G.Ghost.Gbx [--ghost …] --tmtraj PATH
//! tmmaps ghost-touch --batch LIST.tsv --ghosts-dir DIR --tmtraj PATH --out OUT.tsv
//!        LIST.tsv rows: uid \t set \t num \t path-of-the-ORIGINAL-map
//!        ghosts: DIR/<uid>-<rank>.Ghost.Gbx (what `tinyctl nadeo-here ghosts` writes)
//! ```
//!
//! The method is the one the 2026-09-19 stage-deck check used (positions, not
//! surface readouts: under water the wheel material reads Water whatever the
//! floor is — Everios' own warning, and the trap that cost two days this week):
//! `tmtraj export --csv` gives the car's position every tick plus a ground-
//! contact flag; the ORIGINAL map's census gives every converted block's cell.
//! A grounded sample inside a converted block's footprint, in the height band
//! where a car stands on that block, is a touch.
//!
//! Footprints: `census::cell_world` is the cell's horizontal CENTRE at its
//! floor, so a cell spans [x-16, x+16) × [z-16, z+16) — the September awk took
//! the centre for a corner and tested half the block plus half its neighbour.
//! Multi-cell blocks (`RoadWaterCurve3` is 3 × 3 cells) take their size from
//! the name and a margin of size-1 cells on every side, which covers every
//! rotation without knowing it. Conservative on purpose: a doubtful map is
//! wiped, never silently kept.
//!
//! What counts as converted (what `tinyctl surface` changes in the six
//! alterations): black walls `DecoWall*` (not pillars, not the pool pieces),
//! water roads `RoadWater*` / `PlatformWater*` (not the sloped ramp base, which
//! stays vanilla) / `WaterRampZone*`, and pool blocks (anything else carrying
//! `Water`, pillars excepted — their floor converts). Pools count a sample
//! anywhere inside the basin volume, grounded or not: a car in the water is on
//! the floor a moment later.

use crate::census::{self, Placement};
use crate::map::MapFile;
use std::path::{Path, PathBuf};

const CELL: f32 = 32.0;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Kind {
    Black,
    Road,
    Pool,
}

struct Conv {
    name: String,
    kind: Kind,
    /// horizontal footprint, world metres
    x0: f32,
    x1: f32,
    z0: f32,
    z1: f32,
    /// height band a touching car's position falls in
    y0: f32,
    y1: f32,
    /// a wall's whole height, for SIDE contact (the Magnet set drives on it)
    side_y0: f32,
    side_y1: f32,
}

fn is_pillar(n: &str) -> bool {
    n.contains("Pillar")
}

/// Cells per side a block spans, read off its name: a `Curve<r>` is radius r,
/// so r+1 cells; `Diag` 2; else 1. Conservative — covers every rotation.
fn span_cells(n: &str) -> i32 {
    for key in ["Curve", "Diag"] {
        if let Some(i) = n.find(key) {
            let rest = &n[i + key.len()..];
            if let Some(d) = rest.chars().next().and_then(|c| c.to_digit(10)) {
                return d as i32 + 1;
            }
            if key == "Diag" {
                return 2;
            }
        }
    }
    1
}

/// Cells tall a wall block is, read off its name: the digit after `Straight`
/// (`WallStraight4`) or after `x` (`WallOutCurve0x4`, `WallCurve1x4`); else 1.
/// The curve digit is a radius, not a height.
fn wall_height_cells(n: &str) -> i32 {
    if let Some(i) = n.rfind('x') {
        if let Some(d) = n[i + 1..].chars().next().and_then(|c| c.to_digit(10)) {
            return d.max(1) as i32;
        }
    }
    if let Some(i) = n.find("Straight") {
        if let Some(d) = n[i + "Straight".len()..].chars().next().and_then(|c| c.to_digit(10)) {
            return d.max(1) as i32;
        }
    }
    1
}

pub fn classify(n: &str) -> Option<Kind> {
    if is_pillar(n) {
        return None;
    }
    if n.starts_with("DecoWall") {
        return if n.contains("Water") { Some(Kind::Pool) } else { Some(Kind::Black) };
    }
    // the "black blocks": the platform WALL family (`PlatformPlasticWallStraight4`,
    // `PlatformTechWallOutCurve0x4`…) — no twin in the pack, baked with the set's
    // surface; 270 of the 975 maps carry them (~/u10s-keep/wall-maps.txt)
    if n.starts_with("Platform") && n.contains("Wall") && !n.contains("Water") {
        return Some(Kind::Black);
    }
    if n.starts_with("PlatformWaterRamp") {
        return None; // exempt since 2026-10-08: the generated flat deck broke the slope
    }
    if n.starts_with("RoadWater") || n.starts_with("PlatformWater") || n.starts_with("WaterRampZone") {
        return Some(Kind::Road);
    }
    if n.contains("Water") {
        return Some(Kind::Pool);
    }
    None
}

fn converted_blocks(m: &MapFile) -> Vec<Conv> {
    let mut out = Vec::new();
    // walls stack: a column of DecoWallBase at y 10/18/26/34 is ONE wall whose drivable
    // top is the highest block's — the lower ones are faces, never driven. Without this
    // the start pad of Plastic 14 "sat on" a wall two cells underground (shcr 2026-10-09).
    let entries = census::entries(m);
    let mut top_of_column: std::collections::HashMap<(i64, i64, String), f32> = Default::default();
    for e in &entries {
        if e.item || e.baked || classify(&e.name) != Some(Kind::Black) {
            continue;
        }
        let key = ((e.pos[0] * 2.0) as i64, (e.pos[2] * 2.0) as i64, e.name.clone());
        let t = top_of_column.entry(key).or_insert(f32::MIN);
        if e.pos[1] > *t {
            *t = e.pos[1];
        }
    }
    // a wall with ANOTHER block standing on its top (a road, a start pad, a platform —
    // grid or free-placed) is a support column: the car drives that block, never the
    // wall. Plastic 96's start pad sits on a 16-high DecoWallBase column and every
    // record "touched" the wall for its whole first second (shcr 2026-10-09: no wipe).
    // "on its top" = any non-wall block whose floor is at the wall's top height (±0.5 m)
    // and whose cell overlaps or neighbours the wall's: a DecoPlatformBase plateau over a
    // DecoWallBase field (Plastic 96) is recorded one cell over from the walls it covers
    let covered = |x: f32, top: f32, z: f32| -> bool {
        entries.iter().any(|o| {
            !o.item && !o.baked && classify(&o.name) != Some(Kind::Black) && !o.name.contains("Pillar")
                && (o.pos[0] - x).abs() <= 33.0 && (o.pos[2] - z).abs() <= 33.0
                && (o.pos[1] - top).abs() <= 0.5
        })
    };
    for e in entries.clone() {
        // author blocks only: the game's generated `B` records are face clips
        // (`DecoWallBaseVFC`, `WaterWallVFC`: vertical, never driven, never
        // converted) and pool floor pieces that sit inside their owner's cell
        // anyway — counting them doubled map 23's "black" from 0 to 260
        if e.item || e.baked {
            continue;
        }
        let Some(kind) = classify(&e.name) else { continue };
        if kind == Kind::Black {
            let key = ((e.pos[0] * 2.0) as i64, (e.pos[2] * 2.0) as i64, e.name.clone());
            if top_of_column.get(&key).map(|t| e.pos[1] < *t).unwrap_or(false) {
                continue; // a lower block of a wall column: its "top" is inside the wall above
            }
        }
        let span = span_cells(&e.name);
        // a wall's drivable top is its OWN cell (a Curve wall's extra cells are the wall
        // face sweeping round, not deck): no horizontal margin for walls. Pools and water
        // roads keep the span margin — their floor really spans the curve.
        let margin = if kind == Kind::Black { 0.0 } else { (span - 1) as f32 * CELL };
        let (hx, hz) = match e.placement {
            Placement::Cell => (CELL / 2.0, CELL / 2.0),
            // a free block's pos is its pivot: give it a full cell either way
            Placement::Free => (CELL, CELL),
        };
        let [x, y, z] = e.pos;
        let top = y + 8.0 * wall_height_cells(&e.name) as f32;
        if kind == Kind::Black && covered(x, top, z) {
            continue; // something is built on this wall: it is a pillar for that, not a deck
        }
        let (y0, y1) = match kind {
            // ON the top: the car's position sits ~0.5–1.5 m above it when it is standing
            // there. Plastic 147's record crosses the top height while sliding DOWN the
            // wall face (y 64 → 59 in 250 ms, "grounded" against the face): a narrow
            // band, and `touch` additionally wants the car level (|vy| small) — see there.
            Kind::Black => (top - 0.5, top + 2.0),
            // IN the channel: a car on a water road rides ~0.9 m below the water sheet,
            // i.e. ~0.1–2 m above the cell floor (measured 2026-10-06). A car 8–9 m up
            // (Plastic 87, the whole run) is on a block BUILT OVER the water curves and
            // never meets the deck. Band: floor to +4 m.
            Kind::Road => (y - 1.5, y + 4.0),
            // ON THE FLOOR, not anywhere in the basin: a car driving the pool floor sits
            // ~0.1–1 m above the cell floor (measured 2026-10-07, standstill in the pool);
            // a car BOUNCING ON THE WATER rides a full cell up (8.0–9.8 m on Plastic
            // 33/43/55, shcr 2026-10-09 "bouncing on water but not touching the pool
            // floor") and never meets the converted surface. Band: floor to +4 m.
            Kind::Pool => (y - 1.5, y + 4.0),
        };
        out.push(Conv {
            name: e.name,
            kind,
            x0: x - hx - margin,
            x1: x + hx + margin,
            z0: z - hz - margin,
            z1: z + hz + margin,
            y0,
            y1,
            // a car riding the wall FACE sits above the wall's base; a car on the
            // ground beside the wall does not (its position is ~0.4 m up): start
            // the side band a metre above the floor so passing by is not a touch
            side_y0: y + 1.0,
            side_y1: top + 3.5,
        });
    }
    out
}

struct Sample {
    t: i64,
    x: f32,
    y: f32,
    z: f32,
    vy: f32,
    ground: bool,
}

fn export_samples(tmtraj: &Path, ghost: &Path) -> Result<Vec<Sample>, String> {
    let tmp = std::env::temp_dir().join(format!("ghost-touch-{}-{}.csv", std::process::id(), ghost.file_name().and_then(|f| f.to_str()).unwrap_or("g")));
    let st = std::process::Command::new(tmtraj)
        .arg("export")
        .arg(ghost)
        .arg("--csv")
        .arg(&tmp)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::piped())
        .output()
        .map_err(|e| format!("{}: {e}", tmtraj.display()))?;
    if !st.status.success() {
        let _ = std::fs::remove_file(&tmp);
        return Err(format!("tmtraj export failed: {}", String::from_utf8_lossy(&st.stderr).lines().last().unwrap_or("")));
    }
    let text = std::fs::read_to_string(&tmp).map_err(|e| format!("{}: {e}", tmp.display()))?;
    let _ = std::fs::remove_file(&tmp);
    let mut lines = text.lines();
    let header: Vec<&str> = lines.next().unwrap_or("").split(',').collect();
    let col = |n: &str| header.iter().position(|h| *h == n).ok_or_else(|| format!("tmtraj csv has no `{n}` column"));
    let (it, ix, iy, iz, ig) = (col("time_ms")?, col("x")?, col("y")?, col("z")?, col("is_ground_contact")?);
    let ivy = col("vy").ok();
    let mut out = Vec::new();
    for l in lines {
        let f: Vec<&str> = l.trim_end().split(',').collect();
        if f.len() <= ig {
            continue;
        }
        let num = |i: usize| f[i].trim().parse::<f32>().unwrap_or(f32::NAN);
        out.push(Sample {
            t: f[it].trim().parse::<f32>().unwrap_or(0.0) as i64,
            x: num(ix),
            y: num(iy),
            z: num(iz),
            vy: ivy.map(num).unwrap_or(0.0),
            ground: f[ig].trim() == "True" || f[ig].trim() == "1" || f[ig].trim() == "true",
        });
    }
    Ok(out)
}

#[derive(Default)]
struct Touch {
    samples: usize,
    grounded: usize,
    black: usize,
    /// grounded beside a wall, any height: only the Magnet set drives there
    black_side: usize,
    road: usize,
    pool: usize,
    /// first few distinct blocks touched, for the eye
    names: Vec<String>,
    end_ms: i64,
}

fn touch(samples: &[Sample], conv: &[Conv]) -> Touch {
    let mut t = Touch { samples: samples.len(), ..Default::default() };
    t.end_ms = samples.last().map(|s| s.t).unwrap_or(0);
    for s in samples {
        if s.x.is_nan() {
            continue;
        }
        if s.ground {
            t.grounded += 1;
        }
        let mut side_hit = false;
        let mut hit = false;
        for c in conv {
            if s.x < c.x0 - 2.5 || s.x >= c.x1 + 2.5 || s.z < c.z0 - 2.5 || s.z >= c.z1 + 2.5 {
                continue;
            }
            let inside_xz = s.x >= c.x0 && s.x < c.x1 && s.z >= c.z0 && s.z < c.z1;
            if c.kind == Kind::Black && s.ground && !side_hit && s.y >= c.side_y0 && s.y <= c.side_y1 && !(inside_xz && s.y >= c.y0) {
                // beside (or under the lip of) a wall, not on its top
                t.black_side += 1;
                side_hit = true;
            }
            if hit || !inside_xz || s.y < c.y0 || s.y > c.y1 {
                continue;
            }
            match c.kind {
                // level on the top (not sliding down the face through the top height)
                Kind::Black if s.ground && s.vy.abs() < 2.0 => t.black += 1,
                Kind::Road if s.ground => t.road += 1,
                Kind::Pool if s.ground => t.pool += 1,
                _ => continue,
            }
            hit = true;
            if !t.names.iter().any(|n| *n == c.name) && t.names.len() < 6 {
                t.names.push(c.name.clone());
            }
        }
    }
    t
}

fn ghosts_for(dir: &Path, uid: &str) -> Vec<PathBuf> {
    let mut v: Vec<PathBuf> = std::fs::read_dir(dir)
        .map(|rd| {
            rd.filter_map(|e| e.ok().map(|e| e.path()))
                .filter(|p| p.file_name().and_then(|f| f.to_str()).map(|f| f.starts_with(&format!("{uid}-")) && f.ends_with(".Ghost.Gbx")).unwrap_or(false))
                .collect()
        })
        .unwrap_or_default();
    v.sort();
    v
}

/// SIDE contact with a converted wall counts in EVERY set (shcr 2026-10-09: Plastic
/// 69/12/71 drive the black walls' FACES — wall-rides, bounces — and the re-dressed
/// face is a different surface). `_magnet` is kept for the caller's report line.
fn verdict(rows: &[Touch], _magnet: bool) -> &'static str {
    if rows.is_empty() {
        return "NO-GHOST";
    }
    if rows.iter().any(|t| t.black > 0 || t.road > 0 || t.pool > 0 || t.black_side >= 3) {
        "TOUCH"
    } else {
        "CLEAR"
    }
}

/// `--records R.tsv --sets S.tsv --src-index I.tsv --batch-out LIST.tsv`: build the
/// batch list from the leaderboard pull (`tinyctl nadeo-here leaderboards`:
/// mapUid, mapName, accountId, position, score), the uid → "Set num" file the
/// pull was fed, and a num → original-map index. Only uids with at least one
/// record are listed — a map nobody has driven needs no wipe whatever changes.
fn build_batch(records: &str, sets: &str, src_index: &str) -> Result<String, String> {
    let read = |p: &str| std::fs::read_to_string(p).map_err(|e| format!("{p}: {e}"));
    let mut counted: std::collections::BTreeMap<String, usize> = Default::default();
    for l in read(records)?.lines().skip(1) {
        if let Some(uid) = l.split('\t').next() {
            if !uid.is_empty() {
                *counted.entry(uid.to_string()).or_default() += 1;
            }
        }
    }
    let idx: std::collections::HashMap<String, String> = read(src_index)?
        .lines()
        .filter_map(|l| l.split_once('\t').map(|(a, b)| (a.to_string(), b.to_string())))
        .collect();
    let mut out = String::from("# uid\tset\tnum\tsource\trecords\n");
    let mut n = 0;
    for l in read(sets)?.lines() {
        let Some((uid, setnum)) = l.split_once('\t') else { continue };
        let Some(&records_n) = counted.get(uid) else { continue };
        let Some((set, num)) = setnum.rsplit_once(' ') else { continue };
        // Wet Wood IS a target (Everios96 2026-10-08 "Didnt you forget wet wood"): its
        // black walls were converted in September, its water floors were not — so its
        // records are checked against the WATER blocks only (`--wet-wood-water-only`
        // is implicit: the batch row carries the set and `touch` skips Black there).
        let key = if num.len() < 2 { format!("0{num}") } else { num.to_string() };
        let Some(src) = idx.get(&key) else {
            eprintln!("no source for map {key} ({set}) — skipped");
            continue;
        };
        out.push_str(&format!("{uid}\t{set}\t{key}\t{src}\t{records_n}\n"));
        n += 1;
    }
    eprintln!("batch: {n} map-sets with records");
    Ok(out)
}

pub fn cmd(args: &[String]) -> i32 {
    let tmtraj = PathBuf::from(crate::cli::flag(args, "--tmtraj").unwrap_or("tmtraj"));
    if let (Some(r), Some(s), Some(i)) = (crate::cli::flag(args, "--records"), crate::cli::flag(args, "--sets"), crate::cli::flag(args, "--src-index")) {
        let out = crate::cli::flag(args, "--batch-out").expect("--records needs --batch-out LIST.tsv");
        match build_batch(r, s, i) {
            Ok(text) => {
                std::fs::write(out, text).unwrap_or_else(|e| panic!("{out}: {e}"));
                return 0;
            }
            Err(e) => {
                eprintln!("ghost-touch: {e}");
                return 2;
            }
        }
    }
    if let Some(list) = crate::cli::flag(args, "--batch") {
        let dir = PathBuf::from(crate::cli::flag(args, "--ghosts-dir").expect("--batch needs --ghosts-dir DIR"));
        let out_path = crate::cli::flag(args, "--out").map(PathBuf::from);
        let text = std::fs::read_to_string(list).unwrap_or_else(|e| panic!("{list}: {e}"));
        let mut out = String::from("uid\tset\tnum\tverdict\tghosts\tblack_top\tblack_side\troad\tpool\tconverted_blocks\ttouched\n");
        let mut n_touch = 0;
        let mut n_clear = 0;
        let mut n_none = 0;
        for l in text.lines() {
            let f: Vec<&str> = l.split('\t').collect();
            if f.len() < 4 || f[0].starts_with('#') || f[0] == "uid" {
                continue;
            }
            let (uid, set, num, map) = (f[0], f[1], f[2], Path::new(f[3]));
            let m = MapFile::load(map);
            let mut conv = converted_blocks(&m);
            if set.eq_ignore_ascii_case("wet wood") {
                conv.retain(|c| c.kind != Kind::Black);
            }
            let mut rows = Vec::new();
            let mut names: Vec<String> = Vec::new();
            for g in ghosts_for(&dir, uid) {
                match export_samples(&tmtraj, &g) {
                    Ok(s) => {
                        let t = touch(&s, &conv);
                        for n in &t.names {
                            if !names.contains(n) {
                                names.push(n.clone());
                            }
                        }
                        rows.push(t);
                    }
                    Err(e) => eprintln!("{uid} {}: {e}", g.display()),
                }
            }
            let v = verdict(&rows, set.to_ascii_lowercase().contains("magnet"));
            match v {
                "TOUCH" => n_touch += 1,
                "CLEAR" => n_clear += 1,
                _ => n_none += 1,
            }
            let (b, bs, r, p) = rows.iter().fold((0, 0, 0, 0), |a, t| (a.0 + t.black, a.1 + t.black_side, a.2 + t.road, a.3 + t.pool));
            let line = format!("{uid}\t{set}\t{num}\t{v}\t{}\t{b}\t{bs}\t{r}\t{p}\t{}\t{}\n", rows.len(), conv.len(), names.join(","));
            print!("{line}");
            out.push_str(&line);
        }
        eprintln!("ghost-touch: {n_touch} TOUCH, {n_clear} CLEAR, {n_none} without a ghost");
        if let Some(p) = out_path {
            std::fs::write(&p, out).unwrap_or_else(|e| panic!("{}: {e}", p.display()));
        }
        return 0;
    }
    // single map
    let map = args.get(2).filter(|a| !a.starts_with("--")).expect("ghost-touch MAP.Map.Gbx --ghost G … | --batch LIST.tsv");
    let m = MapFile::load(Path::new(map));
    let conv = converted_blocks(&m);
    let (nb, nr, np) = conv.iter().fold((0, 0, 0), |a, c| match c.kind {
        Kind::Black => (a.0 + 1, a.1, a.2),
        Kind::Road => (a.0, a.1 + 1, a.2),
        Kind::Pool => (a.0, a.1, a.2 + 1),
    });
    println!("{map}: {} converted blocks ({nb} black, {nr} water road, {np} pool)", conv.len());
    println!("ghost\tend_ms\tsamples\tgrounded\tblack_top\tblack_side\troad\tpool\ttouched");
    let mut rows = Vec::new();
    for g in crate::cli::flag_multi(args, "--ghost") {
        match export_samples(&tmtraj, Path::new(&g)) {
            Ok(s) => {
                let t = touch(&s, &conv);
                println!("{g}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}", t.end_ms, t.samples, t.grounded, t.black, t.black_side, t.road, t.pool, t.names.join(","));
                rows.push(t);
            }
            Err(e) => eprintln!("{g}: {e}"),
        }
    }
    println!("verdict\t{}  (magnet-side: {})", verdict(&rows, false), verdict(&rows, true));
    0
}
