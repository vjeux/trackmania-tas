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
    let views = derive(&m, gate_dist);
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
