//! `tinyctl views SRC.Map.Gbx` — the comparison cameras a map deserves, derived
//! from the map itself: the start, every checkpoint, the finish (each looked
//! at along the gate's own axis from behind), the whole map from above and its
//! four quadrants.
//!
//! Output is the `cmpviews.sh` / `shootctl shootset` views file:
//!
//! ```text
//! NAME<TAB>ox,oy,oz<TAB>DIST<TAB>H<TAB>V
//! ```
//!
//! in SOURCE-map coordinates (the tiny side is derived from these through the
//! anchor at shoot time, so one file serves both sides). Angles are the editor
//! orbital camera's, in radians: h=0 puts the camera north of the target
//! (DIST is honoured down to about 10 m: the orbital camera stops there, so a
//! nearer view renders the 10 m frame — 2026-09-10)
//! looking south; v>0 looks down, 1.3 is top-down.
//!
//! The header comment carries the anchor line `tmmaps tiny` would print for
//! this map with the default target, so a shoot can copy it without opening
//! the build log.

use std::collections::BTreeMap;
use std::path::Path;
use tmmaps::map::{Kind, MapFile};

pub struct View {
    pub name: String,
    pub target: [f32; 3],
    pub dist: f32,
    pub h: f32,
    pub v: f32,
}

pub fn collection_of(m: &MapFile) -> u32 {
    m.items.first().map(|it| it.collection_raw).unwrap_or(0x1a)
}

pub fn collection_name(c: u32) -> &'static str {
    match c {
        0x1a => "Stadium",
        0x1c => "BlueBay",
        0x10 => "RedIsland",
        0x1d => "WhiteShore",
        0xf => "GreenCoast",
        _ => "?",
    }
}

/// World position of an authored block: its free position, or the cell's
/// centre with the environment's ground row (the same arithmetic `tmmaps tiny`
/// uses for the spawn anchor).
pub fn block_pos(m: &MapFile, b: &tmmaps::map::BlockRec) -> [f32; 3] {
    b.free_pos.unwrap_or_else(|| {
        let mut p = tmmaps::census::cell_world(b);
        p[1] += 62.0 + tmmaps::map::ground_y(collection_of(m));
        p
    })
}

/// The default anchor `tmmaps tiny` would print: spawn -> spawn with y mapped
/// about the collection's fixed plane.
pub fn default_anchor(m: &MapFile, scale: f32) -> Option<([f32; 3], [f32; 3])> {
    let wps = m.waypoints();
    let src = match wps.iter().find(|w| w.kind == Kind::Block && w.tag == "Spawn") {
        Some(w) => block_pos(m, &m.blocks[w.index]),
        None => m.items[wps.iter().find(|w| w.kind == Kind::Item && w.tag == "Spawn")?.index].pos,
    };
    let plane = tmmaps::tiny::fixed_plane(collection_of(m));
    Some((src, [src[0], plane + (src[1] - plane) * scale, src[2]]))
}

pub fn derive(m: &MapFile, gate_dist: f32) -> Vec<View> {
    let mut out = Vec::new();
    // --- gates: behind the gate, looking through it along its axis
    let mut cps = 0;
    let mut used: BTreeMap<String, usize> = BTreeMap::new();
    for w in m.waypoints() {
        let (pos, yaw) = match w.kind {
            Kind::Block => (block_pos(m, &m.blocks[w.index]), w.yaw.unwrap_or(0.0)),
            Kind::Item => (m.items[w.index].pos, w.yaw.unwrap_or(0.0)),
        };
        let name = match w.tag.as_str() {
            "Spawn" | "Start" => "start".to_string(),
            "Finish" | "Goal" => "finish".to_string(),
            "StartFinish" => "startfinish".to_string(),
            _ => {
                cps += 1;
                format!("cp{cps}")
            }
        };
        // along the gate's own axis: a dir 0/2 block (yaw 0) is a road running
        // north-south, so the camera sits on the z axis (h = 0) and looks
        // through the gate; dir 1/3 (yaw pi/2) puts it on the x axis.
        let h = yaw;
        let n = used.entry(name.clone()).or_default();
        *n += 1;
        let name = if *n > 1 { format!("{name}{n}") } else { name };
        out.push(View { name, target: [pos[0], pos[1] + 3.0, pos[2]], dist: gate_dist, h, v: 0.42 });
    }
    // --- overview: the authored footprint, without the ambient terrain (the
    // sea / lake fills the whole 64x64 grid) and without stray outliers
    let ambient = m.ambient_zone().unwrap_or_default();
    let mut xs = Vec::new();
    let mut ys = Vec::new();
    let mut zs = Vec::new();
    for b in &m.blocks {
        if !ambient.is_empty() && b.name.starts_with(&ambient) {
            continue;
        }
        let p = block_pos(m, b);
        xs.push(p[0]);
        ys.push(p[1]);
        zs.push(p[2]);
    }
    for it in &m.items {
        xs.push(it.pos[0]);
        ys.push(it.pos[1]);
        zs.push(it.pos[2]);
    }
    if xs.is_empty() {
        return out;
    }
    let pct = |v: &[f32], lo: f32, hi: f32| {
        let mut s = v.to_vec();
        s.sort_by(|a, b| a.partial_cmp(b).unwrap());
        let n = s.len();
        (s[((n - 1) as f32 * lo) as usize], s[((n - 1) as f32 * hi) as usize])
    };
    let (x0, x1) = pct(&xs, 0.02, 0.98);
    let (z0, z1) = pct(&zs, 0.02, 0.98);
    let (_, ymid) = pct(&ys, 0.0, 0.5);
    let (cx, cz) = ((x0 + x1) / 2.0, (z0 + z1) / 2.0);
    let ext = (x1 - x0).max(z1 - z0).max(64.0);
    out.push(View { name: "top".into(), target: [cx, ymid, cz], dist: ext * 1.1, h: 0.0, v: 1.3 });
    let q = ext / 4.0;
    for (name, dx, dz) in [("topnw", -q, -q), ("topne", q, -q), ("topsw", -q, q), ("topse", q, q)] {
        out.push(View { name: name.into(), target: [cx + dx, ymid, cz + dz], dist: ext * 0.6, h: 0.0, v: 1.3 });
    }
    out
}

pub fn write_tsv(m: &MapFile, views: &[View], src: &Path) -> String {
    let mut s = String::new();
    s.push_str(&format!("# views for {} ({} {:#x}); NAME<TAB>ox,oy,oz<TAB>DIST<TAB>H<TAB>V (source coords, radians)\n", src.display(), collection_name(collection_of(m)), collection_of(m)));
    if let Some((a, b)) = default_anchor(m, 0.5) {
        s.push_str(&format!("# anchor {},{},{}:{},{},{}\n", a[0], a[1], a[2], b[0], b[1], b[2]));
    }
    for v in views {
        s.push_str(&format!("{}\t{:.1},{:.1},{:.1}\t{:.1}\t{:.4}\t{:.4}\n", v.name, v.target[0], v.target[1], v.target[2], v.dist, v.h, v.v));
    }
    s
}

pub fn cmd(args: &[String]) -> Result<(), String> {
    let src = Path::new(args.get(0).ok_or("views needs SRC.Map.Gbx")?);
    let m = MapFile::load(src);
    let gate_dist: f32 = tmmaps::cli::flag(args, "--gate-dist").unwrap_or("48").parse().map_err(|_| "--gate-dist number")?;
    let mut views = derive(&m, gate_dist);
    // --ghost G --at 4000,16250 [--chase-dist 30] [--chase-v 0.3] [--only-chase]:
    // chase views at instants of a driven lap (ms), see `chase_views`
    if let Some(g) = tmmaps::cli::flag(args, "--ghost") {
        let at: Vec<i32> = tmmaps::cli::flag(args, "--at")
            .ok_or("--ghost needs --at T[,T…] (ms into the lap)")?
            .split(',')
            .map(|s| s.trim().parse::<i32>().map_err(|_| format!("--at: `{s}` is not a millisecond count")))
            .collect::<Result<_, _>>()?;
        let dist: f32 = tmmaps::cli::flag(args, "--chase-dist").unwrap_or("30").parse().map_err(|_| "--chase-dist number")?;
        let v: f32 = tmmaps::cli::flag(args, "--chase-v").unwrap_or("0.3").parse().map_err(|_| "--chase-v number")?;
        let chase = chase_views(&m, Path::new(g), &at, dist, v)?;
        if tmmaps::cli::has(args, "--only-chase") {
            views = chase;
        } else {
            views.extend(chase);
        }
    }
    // --trees N --mapping placements.tsv [--tree-dist 30] [--only-trees]: cameras
    // on the N densest clusters of BAKED trees, see `tree_views`
    if let Some(n) = tmmaps::cli::flag(args, "--trees") {
        let n: usize = n.parse().map_err(|_| "--trees N (clusters)")?;
        let mapping = tmmaps::cli::flag(args, "--mapping").ok_or("--trees needs --mapping placements.tsv (the tiny-library mapping: which source items were baked as trees)")?;
        let dist: f32 = tmmaps::cli::flag(args, "--tree-dist").unwrap_or("30").parse().map_err(|_| "--tree-dist number")?;
        let trees = tree_views(&m, Path::new(mapping), n, dist)?;
        if tmmaps::cli::has(args, "--only-trees") {
            views = trees;
        } else {
            views.extend(trees);
        }
    }
    let text = write_tsv(&m, &views, src);
    match tmmaps::cli::flag(args, "--out") {
        Some(p) => {
            std::fs::write(p, &text).map_err(|e| format!("{p}: {e}"))?;
            eprintln!("{} views -> {p}", views.len());
        }
        None => print!("{text}"),
    }
    // a summary of what the gates are, for the log
    let mut kinds: BTreeMap<String, usize> = BTreeMap::new();
    for w in m.waypoints() {
        *kinds.entry(w.tag.clone()).or_default() += 1;
    }
    eprintln!("waypoints: {kinds:?}");
    Ok(())
}

/// `--ghost G --at T[,T…]`: a chase view at each instant of a driven lap — the
/// camera behind the car along its velocity, at the car's position mapped from
/// the tiny map back into SOURCE coordinates through the default anchor (the
/// views file is in source coordinates; `shootset --side t` maps it back). The
/// spot-check camera: the frame of the clip that looked wrong, shot on both
/// maps from the same place.
pub fn chase_views(m: &MapFile, ghost: &Path, at_ms: &[i32], dist: f32, v: f32) -> Result<Vec<View>, String> {
    let g = gbx::record::decode_ghost(ghost.to_str().ok_or("ghost path is not utf-8")?)?;
    let (a, b) = default_anchor(m, 0.5).ok_or("this map has no Spawn to anchor on")?;
    let mut out = Vec::new();
    for &t in at_ms {
        let s = g.samples.iter().min_by_key(|s| (s.time_ms - t).abs()).ok_or("the ghost has no samples")?;
        let src = [a[0] + (s.x - b[0]) / 0.5, a[1] + (s.y - b[1]) / 0.5, a[2] + (s.z - b[2]) / 0.5];
        // behind the car: the camera sits at target + dist·(−sin h, ·, −cos h),
        // so h = atan2(vx, vz) puts it on the far side of the velocity
        let (vx, vz) = (s.vx, s.vz);
        let h = if vx.hypot(vz) > 0.5 { vx.atan2(vz) } else { 0.0 };
        out.push(View { name: format!("g{t}"), target: [src[0], src[1] + 3.0, src[2]], dist, h, v });
    }
    Ok(out)
}

/// `--trees N --mapping placements.tsv`: a camera on each of the N densest
/// clusters of BAKED trees — the source items the tiny-library mapping turned
/// into `AV…` items (`i@<index><TAB>AV….Item.Gbx` rows). Positions are binned
/// on a 24 m grid; a cluster is a bin plus its eight neighbours, taken
/// greedily by tree count with a 48 m exclusion, the first cluster preferring
/// the neighbourhood of the spawn (the player's first look) when one holds at
/// least three trees. Each cluster gets two rows in SOURCE coordinates: `treeK`
/// at `dist` from the south (h = π, the player's side; v 0.25) aimed 6 m up the
/// trunks, and `treeKc` a close-up at dist/4 aimed 4 m up — the 30 m / 7 m pair
/// the trees thread judged crowns and single leaves by (2026-09-09).
pub fn tree_views(m: &MapFile, mapping: &Path, n: usize, dist: f32) -> Result<Vec<View>, String> {
    let text = std::fs::read_to_string(mapping).map_err(|e| format!("{}: {e}", mapping.display()))?;
    let mut pts: Vec<[f32; 3]> = Vec::new();
    for line in text.lines() {
        let mut cols = line.split('\t');
        let (Some(key), Some(item)) = (cols.next(), cols.next()) else { continue };
        let Some(idx) = key.strip_prefix("i@") else { continue };
        let Ok(idx) = idx.parse::<usize>() else { continue };
        let stem = item.rsplit(['/', '\\']).next().unwrap_or(item);
        if !stem.starts_with("AV") {
            continue;
        }
        if let Some(it) = m.items.get(idx) {
            pts.push(it.pos);
        }
    }
    if pts.is_empty() {
        return Err(format!("{}: no `i@N<TAB>AV…` rows — this map bakes no trees, or the mapping is not a tiny-library one", mapping.display()));
    }
    let cell = 24.0f32;
    let mut bins: BTreeMap<(i32, i32), Vec<usize>> = BTreeMap::new();
    for (i, p) in pts.iter().enumerate() {
        bins.entry(((p[0] / cell).floor() as i32, (p[2] / cell).floor() as i32)).or_default().push(i);
    }
    // a candidate per bin: the trees of the bin and its eight neighbours
    let mut cands: Vec<(usize, [f32; 3], Vec<usize>)> = Vec::new();
    for (&(bx, bz), _) in &bins {
        let mut members: Vec<usize> = Vec::new();
        for dx in -1..=1 {
            for dz in -1..=1 {
                if let Some(v) = bins.get(&(bx + dx, bz + dz)) {
                    members.extend(v.iter().copied());
                }
            }
        }
        let k = members.len() as f32;
        let mut c = [0.0f32; 3];
        for &i in &members {
            for a in 0..3 {
                c[a] += pts[i][a] / k;
            }
        }
        cands.push((members.len(), c, members));
    }
    cands.sort_by(|a, b| b.0.cmp(&a.0));
    let spawn = default_anchor(m, 0.5).map(|(a, _)| a);
    let mut picked: Vec<(usize, [f32; 3])> = Vec::new();
    // the spawn's neighbourhood first, when it has trees
    if let Some(s) = spawn {
        if let Some(c) = cands.iter().filter(|c| c.0 >= 3 && (c.1[0] - s[0]).hypot(c.1[2] - s[2]) < 120.0).max_by_key(|c| c.0) {
            picked.push((c.0, c.1));
        }
    }
    for c in &cands {
        if picked.len() >= n {
            break;
        }
        if picked.iter().any(|(_, p)| (p[0] - c.1[0]).hypot(p[2] - c.1[2]) < 48.0) {
            continue;
        }
        picked.push((c.0, c.1));
    }
    let mut out = Vec::new();
    for (k, (count, c)) in picked.iter().enumerate() {
        eprintln!("tree cluster {}: {count} baked trees around {:.1},{:.1},{:.1}", k + 1, c[0], c[1], c[2]);
        out.push(View { name: format!("tree{}", k + 1), target: [c[0], c[1] + 6.0, c[2]], dist, h: std::f32::consts::PI, v: 0.25 });
        out.push(View { name: format!("tree{}c", k + 1), target: [c[0], c[1] + 4.0, c[2]], dist: (dist / 4.0).max(5.0), h: std::f32::consts::PI, v: 0.12 });
    }
    Ok(out)
}
