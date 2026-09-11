//! From the sampled track to a `.Map.Gbx`: the lap becomes road items, the
//! start / checkpoints / finish become waypoint items, and a small Stadium
//! host map is re-sized and re-populated with them (the host contributes the
//! container: header, decoration, grass, one start block that is parked).

use crate::edges::Edges;
use crate::mesh::{self, MeshBuilder, Waypoint, WaypointKind};
use crate::track::Track;
use std::collections::BTreeMap;
use std::path::Path;
use tmmaps::map::MapFile;

pub const AUTHOR: &str = "KTaOsd-lTR2zkoskETSfPA";

/// BNG metres -> Trackmania metres. x = east, z = south (north is -z), y up.
pub struct Frame {
    pub e0: f64,
    pub n0: f64,
    pub z_ref: f64,
}

impl Frame {
    pub fn to_tm(&self, e: f64, n: f64, z: f64) -> [f32; 3] {
        [(e - self.e0) as f32, (z - self.z_ref) as f32, (self.n0 - n) as f32]
    }
}

#[derive(Clone)]
pub struct Placement {
    pub ident: String,
    pub bytes: Vec<u8>,
    pub pos: [f32; 3],
    /// Yaw in radians about +y.
    pub yaw: f32,
    pub tag: Option<&'static str>,
    /// Hash of what the car can feel in this item (collision + waypoint).
    pub physics: u64,
}

/// The map uid: `Silverstone1to1` + 12 hex digits of a hash over every
/// placement's ident, position, yaw and physics hash — so a rebuild that only
/// changes looks keeps the uid (and every ghost validated on it), and one that
/// moves anything the car can touch gets a new one.
pub fn map_uid(placements: &[Placement]) -> String {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    let mut feed = |bytes: &[u8]| {
        for b in bytes {
            h ^= *b as u64;
            h = h.wrapping_mul(0x0100_0000_01b3);
        }
    };
    for p in placements {
        feed(p.ident.as_bytes());
        for c in p.pos {
            feed(&((c * 1000.0).round() as i64).to_le_bytes());
        }
        feed(&((p.yaw * 100_000.0).round() as i64).to_le_bytes());
        feed(&p.physics.to_le_bytes());
        feed(p.tag.unwrap_or("").as_bytes());
    }
    let uid = format!("Silverstone1to1{:012x}", h & 0xffff_ffff_ffff);
    assert_eq!(uid.len(), 27);
    uid
}

/// Item yaw for a heading vector (dx, dz) in TM world space: the item's
/// local +z is turned onto it.
pub fn yaw_for(dx: f32, dz: f32) -> f32 {
    dx.atan2(dz)
}

/// World -> item-local for an item at `pos` with `yaw` (inverse of the
/// placement transform, assuming local +z turns onto (sin yaw, cos yaw)).
pub fn to_local(p: [f32; 3], pos: [f32; 3], yaw: f32) -> [f32; 3] {
    let (s, c) = yaw.sin_cos();
    let dx = p[0] - pos[0];
    let dz = p[2] - pos[2];
    // inverse rotation
    [dx * c - dz * s, p[1] - pos[1], dx * s + dz * c]
}

/// The lap surface: one item per `seg_len` metres of lap, tarmac between
/// the edges, kerb strips where the intensity saw a bright band, thin side
/// skirts down to hide the terrain seam. `skip` marks stations covered by
/// waypoint items instead (their road is inside those items).
pub fn road_items(tr: &Track, ed: &Edges, fr: &Frame, seg_len: f64, skip: &[bool]) -> Vec<Placement> {
    let n = tr.len();
    let per = (seg_len / tr.ds).round() as usize;
    let mut out = Vec::new();
    let mut seg = 0usize;
    let mut i = 0usize;
    while i < n {
        let end = (i + per).min(n);
        // anchor: the segment's first centreline point, at road height
        let a0 = fr.to_tm(tr.stations[i].e, tr.stations[i].n, tr.stations[i].z);
        let mut mb = MeshBuilder::new();
        let asphalt = mb.material(&mesh::ASPHALT);
        let kerb = mb.material(&mesh::KERB);
        let mut any = false;
        for k in i..end {
            let k1 = (k + 1) % n;
            if skip[k] {
                continue;
            }
            any = true;
            road_slice(&mut mb, tr, ed, fr, a0, k, k1, asphalt, kerb, 0.0);
        }
        if any {
            let ident = format!("Silverstone\\Road{seg:03}.Item.Gbx");
            let physics = mb.physics_hash(None);
            let bytes = mb.build(&ident, AUTHOR, None);
            out.push(Placement { ident, bytes, pos: a0, yaw: 0.0, tag: None, physics });
        }
        seg += 1;
        i = end;
    }
    out
}

/// One station-to-station slice of road into `mb`, in the frame of `anchor`
/// (world coordinates minus the anchor), lifted by `lift`.
#[allow(clippy::too_many_arguments)]
fn road_slice(mb: &mut MeshBuilder, tr: &Track, ed: &Edges, fr: &Frame, anchor: [f32; 3], k: usize, k1: usize, asphalt: usize, kerb: usize, lift: f32) {
    let p = |i: usize, off: f64| -> [f32; 3] {
        let w = tr.offset(i, off);
        let t = fr.to_tm(w[0], w[1], w[2]);
        [t[0] - anchor[0], t[1] - anchor[1] + lift, t[2] - anchor[2]]
    };
    let (l0, l1) = (ed.left[k], ed.left[k1]);
    let (r0, r1) = (ed.right[k], ed.right[k1]);
    // Texture: Nadeo's road atlases run ALONG the road in u (one tile per
    // 32 m) and ACROSS it in v, whose lit band is 0.06..0.94 (the deck with
    // its edge lines; outside it the atlas is black). Kerbs: TrackBorders,
    // one tile per 8 m along, the same band across.
    let (s0, s1) = (tr.stations[k].s as f32, if k1 == 0 { (tr.stations[k].s + tr.ds) as f32 } else { tr.stations[k1].s as f32 });
    let (u0, u1) = (s0 / 32.0, s1 / 32.0);
    // tarmac: left edge to right edge (left is +offset)
    let (a, b, c, d) = (p(k, l0), p(k1, l1), p(k1, -r1), p(k, -r0));
    mb.quad_uv_up(asphalt, [a, b, c, d], [[u0, 0.06], [u1, 0.06], [u1, 0.94], [u0, 0.94]], true);
    // kerbs: bright band outside each edge
    let (ku0, ku1) = (s0 / 8.0, s1 / 8.0);
    let (kl0, kl1) = (ed.kerb_left[k].min(4.0), ed.kerb_left[k1].min(4.0));
    if kl0 > 0.4 || kl1 > 0.4 {
        let (e, f) = (p(k, l0 + kl0.max(0.5)), p(k1, l1 + kl1.max(0.5)));
        mb.quad_uv_up(kerb, [e, f, b, a], [[ku0, 0.06], [ku1, 0.06], [ku1, 0.94], [ku0, 0.94]], true);
    }
    let (kr0, kr1) = (ed.kerb_right[k].min(4.0), ed.kerb_right[k1].min(4.0));
    if kr0 > 0.4 || kr1 > 0.4 {
        let (e, f) = (p(k, -(r0 + kr0.max(0.5))), p(k1, -(r1 + kr1.max(0.5))));
        mb.quad_uv_up(kerb, [d, c, f, e], [[ku0, 0.06], [ku1, 0.06], [ku1, 0.94], [ku0, 0.94]], true);
    }
    // skirts: 0.4 m down at the outer limits, so the seam to the terrain
    // never shows daylight
    let ol0 = l0 + if kl0 > 0.4 { kl0.max(0.5) } else { 0.0 };
    let ol1 = l1 + if kl1 > 0.4 { kl1.max(0.5) } else { 0.0 };
    let or0 = r0 + if kr0 > 0.4 { kr0.max(0.5) } else { 0.0 };
    let or1 = r1 + if kr1 > 0.4 { kr1.max(0.5) } else { 0.0 };
    let down = |q: [f32; 3]| [q[0], q[1] - 0.4, q[2]];
    let (e, f) = (p(k, ol0), p(k1, ol1));
    mb.quad(asphalt, [e, down(e), down(f), f], false);
    let (g, h) = (p(k, -or0), p(k1, -or1));
    mb.quad(asphalt, [h, down(h), down(g), g], false);
}

pub struct WaypointPlan {
    /// Station indices of the checkpoints, in lap order.
    pub checkpoints: Vec<usize>,
    /// Station of the start line (spawn just past it) and of the finish line.
    pub start: usize,
    pub finish: usize,
}

/// Checkpoints roughly every `spacing` metres, each nudged onto the
/// straightest station within 60 m so no gate sits mid-corner; the finish
/// 25 m before the start line.
pub fn plan_waypoints(tr: &Track, start: usize, spacing: f64) -> WaypointPlan {
    let n = tr.len();
    let count = ((n as f64 * tr.ds) / spacing).round().max(3.0) as usize;
    let mut checkpoints = Vec::new();
    for c in 1..count {
        let s = start as f64 + c as f64 * (n as f64 / count as f64);
        let centre = (s.round() as usize) % n;
        let win = (60.0 / tr.ds) as i64;
        let best = (-win..=win)
            .map(|d| ((centre as i64 + d).rem_euclid(n as i64)) as usize)
            .min_by(|&a, &b| tr.stations[a].curvature.abs().partial_cmp(&tr.stations[b].curvature.abs()).unwrap())
            .unwrap();
        checkpoints.push(best);
    }
    let finish = (start + n - (25.0 / tr.ds) as usize) % n;
    WaypointPlan { checkpoints, start, finish }
}

/// A waypoint item: `half` metres of road either side of station `at`,
/// built in the item's own frame (local +z = direction of travel, origin at
/// the station's centreline point), with the trigger/spawn at the origin.
fn waypoint_item(tr: &Track, ed: &Edges, fr: &Frame, at: usize, half: f64, kind: WaypointKind, ident: &str) -> Placement {
    let n = tr.len();
    let st = &tr.stations[at];
    let pos = fr.to_tm(st.e, st.n, st.z);
    // heading in TM space: east = +x, north = -z
    let (sh, ch) = st.heading.sin_cos();
    let yaw = yaw_for(ch as f32, -sh as f32);
    let mut mb = MeshBuilder::new();
    let asphalt = mb.material(&mesh::ASPHALT);
    let kerb = mb.material(&mesh::KERB);
    let paint = mb.material(&mesh::CONCRETE);
    let hs = (half / tr.ds) as usize;
    // build in WORLD frame relative to pos, then rotate into local
    let mut world = MeshBuilder::new();
    let wa = world.material(&mesh::ASPHALT);
    let wk = world.material(&mesh::KERB);
    let wp_paint = world.material(&mesh::CONCRETE);
    for d in 0..2 * hs {
        let k = (at + n - hs + d) % n;
        let k1 = (k + 1) % n;
        road_slice(&mut world, tr, ed, fr, pos, k, k1, wa, wk, 0.0);
    }
    // a painted line across the track at the waypoint (2 m for the start
    // and finish, 1 m for a checkpoint), a hair above the tarmac
    {
        let len = if kind == WaypointKind::Checkpoint { 1.0 } else { 2.0 };
        let (k0, k1) = ((at + n - (len / 2.0 / tr.ds) as usize) % n, (at + (len / 2.0 / tr.ds).ceil() as usize) % n);
        let p = |i: usize, off: f64| -> [f32; 3] {
            let w = tr.offset(i, off);
            let t = fr.to_tm(w[0], w[1], w[2]);
            [t[0] - pos[0], t[1] - pos[1] + 0.02, t[2] - pos[2]]
        };
        let (a, b, c, d) = (p(k0, ed.left[k0]), p(k1, ed.left[k1]), p(k1, -ed.right[k1]), p(k0, -ed.right[k0]));
        world.quad_uv_up(wp_paint, [a, b, c, d], [[0.1, 0.1], [0.1, 0.2], [0.9, 0.2], [0.9, 0.1]], false);
    }
    // re-emit rotated into the item frame
    let rot = |q: [f32; 3]| to_local([q[0] + pos[0], q[1] + pos[1], q[2] + pos[2]], pos, yaw);
    world.replay_into(&mut mb, &[(wa, asphalt), (wk, kerb), (wp_paint, paint)], &rot);
    let width = (ed.left[at] + ed.right[at]) as f32;
    let wp = Waypoint {
        kind,
        spawn: [0.0, 0.5, 2.0],
        trigger: if kind == WaypointKind::Start { None } else { Some(([-(width / 2.0 + 6.0), -1.0, -2.0], [width / 2.0 + 6.0, 9.0, 2.0])) },
    };
    let physics = mb.physics_hash(Some(&wp));
    let bytes = mb.build(ident, AUTHOR, Some(&wp));
    let tag = Some(match kind {
        WaypointKind::Start => "Spawn",
        WaypointKind::Finish => "Goal",
        WaypointKind::Checkpoint => "Checkpoint",
    });
    Placement { ident: ident.to_string(), bytes, pos, yaw, tag, physics }
}

pub fn waypoint_items(tr: &Track, ed: &Edges, fr: &Frame, plan: &WaypointPlan, half: f64) -> (Vec<Placement>, Vec<bool>) {
    let n = tr.len();
    let mut skip = vec![false; n];
    let mut mark = |at: usize| {
        let hs = (half / tr.ds) as usize;
        for d in 0..2 * hs {
            skip[(at + n - hs + d) % n] = true;
        }
    };
    let mut out = Vec::new();
    mark(plan.start);
    out.push(waypoint_item(tr, ed, fr, plan.start, half, WaypointKind::Start, "Silverstone\\Start.Item.Gbx"));
    mark(plan.finish);
    out.push(waypoint_item(tr, ed, fr, plan.finish, half, WaypointKind::Finish, "Silverstone\\Finish.Item.Gbx"));
    for (i, &c) in plan.checkpoints.iter().enumerate() {
        mark(c);
        out.push(waypoint_item(tr, ed, fr, c, half, WaypointKind::Checkpoint, &format!("Silverstone\\Checkpoint{:02}.Item.Gbx", i + 1)));
    }
    (out, skip)
}

/// Write the map: the host re-sized, its block parked, its items replaced
/// by `placements`, the item files embedded.
/// A map name of exactly `len` bytes that says what this is.
pub fn name_of_len(len: usize) -> String {
    let candidates = ["Silverstone", "Silverstone 1:1", "Silverstone Circuit 1:1", "Silverstone Circuit 1to1", "Silverstone GP Circuit 1:1", "Silverstone Circuit (1:1 scale)"];
    if let Some(c) = candidates.iter().find(|c| c.len() == len) {
        return c.to_string();
    }
    let base = "Silverstone Circuit 1:1 scale, real LIDAR ground";
    if len <= base.len() {
        base[..len].to_string()
    } else {
        format!("{base:<len$}")
    }
}

/// Medal times from a lap time (ms): author = the lap, gold +8 %, silver
/// +20 %, bronze +50 %, rounded to 10 ms. None = "not validated" placeholders
/// (a bronze of 10 minutes, so the header still parses).
pub fn medals(author_ms: Option<u32>) -> (u32, u32, u32, u32, bool) {
    match author_ms {
        Some(at) => {
            let r = |x: f64| ((x / 10.0).round() as u32) * 10;
            (r(at as f64 * 1.5), r(at as f64 * 1.2), r(at as f64 * 1.08), at, true)
        }
        None => (600_000, 600_000, 600_000, 600_000, false),
    }
}

pub fn assemble(host: &Path, out: &Path, placements: &[Placement], size: Option<[i32; 3]>, uid: &str, author_ms: Option<u32>) {
    let stage1 = out.with_extension("stage1.Map.Gbx");
    let stage2 = out.with_extension("stage2.Map.Gbx");
    // Stage 1: fixed-size patches + lookback renames: uid, name, author,
    // size, park the host's blocks (renamed to a plain road so none is a
    // spawn or a gate any more).
    let mut m = MapFile::load(host);
    m.set_map_uid(uid);
    let old_name = m.map_name();
    let new_name = name_of_len(old_name.len());
    m.set_map_name_same_len(&old_name, &new_name);
    if m.map_author().as_deref() != Some(AUTHOR) {
        m.set_map_author_same_len(AUTHOR);
    }
    let size = size.unwrap_or(m.size);
    let resized = size != m.size;
    if resized {
        m.set_size(size);
    }
    let (b, sv, g, at, validated) = medals(author_ms);
    m.set_times(b, sv, g, at, validated);
    println!("host {}: {old_name:?} -> {new_name:?}, size {:?}", host.display(), size);
    for i in 0..m.blocks.len() {
        m.move_block_cell(i, (0, 0, 0));
        if m.blocks[i].waypoint_tag.is_some() || m.blocks[i].name != "RoadTechStraight" {
            m.set_block_name(i, "RoadTechStraight");
        }
    }
    m.write_to(&stage1).expect("stage 1");
    // Stage 2: grow the item array and point every slot at our items.
    let mut m = MapFile::load(&stage1);
    let total = placements.len().max(m.items.len());
    m.append_item_clones(total);
    m.write_to(&stage2).expect("stage 2");
    let mut m = MapFile::load(&stage2);
    assert_eq!(m.items.len(), total);
    if placements.is_empty() {
        // nothing to place: leave the host's items alone (bisection aid)
        m.write_to(out).expect("final write");
        if resized {
            let cells = (size[0] * size[2]) as usize;
            let (zone, n) = MapFile::fill_genealogy_file_n(out, Some(cells)).expect("genealogy fill");
            let added = MapFile::extend_baked_file(out, &zone, size[0], size[2]).expect("baked extension");
            println!("genealogy {n} cells of {zone}, {added} baked added");
        }
        let _ = std::fs::remove_file(&stage1);
        let _ = std::fs::remove_file(&stage2);
        println!("{}: size {:?}, host items kept", out.display(), m.size);
        return;
    }
    for i in 0..total {
        if let Some(p) = placements.get(i) {
            let cell = ((p.pos[0] / 32.0).floor() as i32, ((p.pos[1] + 64.0) / 8.0).floor() as i32, (p.pos[2] / 32.0).floor() as i32);
            m.move_item(i, p.pos, p.yaw, cell);
            m.set_item_frame(i, [p.yaw, 0.0, 0.0], [0.0, 0.0, 0.0]);
            m.set_item_scale(i, 1.0);
            m.clear_item_variant(i);
            m.set_item_model(i, &p.ident);
            m.set_item_author(i, AUTHOR);
        } else {
            // a spare host slot: park it far below, as a harmless road item
            m.move_item(i, [16.0, -1000.0, 16.0], 0.0, (0, 0, 0));
            m.set_item_scale(i, 1.0);
            m.clear_item_variant(i);
            m.set_item_model(i, &placements[0].ident);
            m.set_item_author(i, AUTHOR);
        }
    }
    m.write_to(&stage1).expect("stage 3");
    // Stage 3: variable-length records: waypoint tags and the embedded zip.
    let mut m = MapFile::load(&stage1);
    for i in 0..total {
        m.set_item_waypoint_tag(i, placements.get(i).and_then(|p| p.tag));
    }
    m.remove_password();
    let cells = (size[0] * size[2]) as usize;
    let mut files = BTreeMap::new();
    let mut manifest: Vec<(String, String)> = Vec::new();
    for p in placements {
        files.insert(format!("Items/{}", p.ident.replace('\\', "/")), p.bytes.clone());
        manifest.push((p.ident.clone(), AUTHOR.to_string()));
    }
    let refs: Vec<(&str, &str)> = manifest.iter().map(|(a, b)| (a.as_str(), b.as_str())).collect();
    m.replace_embedded_objects(&refs, &mapgeom::tiny_assets::zip(&files));
    m.write_to(out).expect("final write");
    // Stage 4 (resized hosts only): one genealogy record and one baked
    // ground block per cell of the bigger grid.
    if resized {
        let (zone, n) = MapFile::fill_genealogy_file_n(out, Some(cells)).expect("genealogy fill");
        let added = MapFile::extend_baked_file(out, &zone, size[0], size[2]).expect("baked extension");
        println!("genealogy: {n} cells of {zone}; {added} baked {zone} blocks added");
    }
    let _ = std::fs::remove_file(&stage1);
    let _ = std::fs::remove_file(&stage2);
    // read back
    let m = MapFile::load(out);
    let wps = m.waypoints();
    println!("{}: size {:?}, {} items, {} waypoints, {} bytes", out.display(), m.size, m.items.len(), wps.len(), std::fs::metadata(out).map(|x| x.len()).unwrap_or(0));
    for w in wps.iter().take(4) {
        println!("  {w}");
    }
}
