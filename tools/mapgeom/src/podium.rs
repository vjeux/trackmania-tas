//! `mapgeom podium-reverse` — Hugo's "Podium Reverse" alteration (2026-10-05):
//!
//! 1. STTF + no finish: every checkpoint AND finish block swapped for its
//!    geometry-verified plain twin (removed with its clips when none exists),
//!    every checkpoint / finish item removed (`sttc::strip_waypoints`).
//! 2. The START becomes a FINISH: a start BLOCK → its finish twin
//!    (`RoadTechStart` → `RoadTechFinish`, the same cell / dir / flags, the
//!    waypoint node's tag `Spawn` → `Goal`); a start GATE ITEM → the finish
//!    gate of the same width and side (`GateStartCenter16m` →
//!    `GateFinishCenter16m`), its tag `Goal`.
//! 3. A NEW START in front of the PODIUM: the pack's `Podium` item (every Fall
//!    2026 map carries one, 09 and 18 two) has, in its own frame, the tiers at
//!    z −3..+2.44 and a front face at z = +2.79 (the collision mesh's frontmost
//!    plane, x ±6.9, from the skirt at y 0.18 to the tier top at 2.65); its
//!    front direction is local +z. The start gate item (`GateStartCenter8m`,
//!    the narrowest poleless one) spawns the car at its local (0, 0, −10.6)
//!    facing +z (the pack prefab's `NPlugTrigger_SSpawn` entity), so with the
//!    gate's +z = the podium's +z and the gate 10.6 + L metres in front of the
//!    face, the car stands L metres in front of the face, facing away from
//!    the podium — L = the car's origin-to-rear-bumper distance (`--rear`,
//!    measured in play by reversing the car into the face) plus `--gap`.
//! 4. Identity: uid `PdRv` + the source's, name "<name> Podium Reverse",
//!    validation ghost dropped (validated 0), the times `--times` (none of
//!    Nadeo's: a sentinel), `tmmaps unlock`; the lightmap kept + renumbered by
//!    the sttc rule (the swapped pieces chartless, the new item chartless).

use crate::sttc::{cell_str, pos_str, wp_str, Ctx, Row, SttfOutcome, WP_FINISH, WP_START, WP_STARTFINISH};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use tmmaps::map::{ItemRec, MapFile, FREE_BLOCK_FLAG};

/// The podium's front face in its own frame (metres): the frontmost collision
/// plane of `Stadium\Items\Podium.Item.Gbx` (measured on the prefab's surface
/// mesh, 2026-10-05: the tier block ends at z 2.789 for |x| < 6.9, the skirt at
/// 2.765; the back ramp reaches z −6.56; the gantry legs at x ±8.3).
pub const PODIUM_FRONT_Z: f32 = 2.79;
/// The start gate's spawn point in the item's frame (the prefab entity).
pub const GATE_SPAWN_Z: f32 = -10.6;

#[derive(Clone, Debug)]
pub struct PodiumOpts {
    pub start_item: String,
    /// the car's origin → rear bumper distance (m)
    pub rear: f32,
    /// extra clearance between the bumper and the face (m)
    pub gap: f32,
    /// which podium when a map has several (0-based, record order); None = the one
    /// nearest the OLD START in a straight line (Hugo, 2026-10-05 06:51 PT)
    pub podium: Option<usize>,
    pub uid_prefix: String,
    pub name_suffix: String,
    /// author/gold/silver/bronze in ms; None = the source's
    pub times: Option<(u32, u32, u32, u32)>,
    pub unlock: bool,
}

pub struct PodiumOutcome {
    pub rows: Vec<Row>,
    pub podium_pos: [f32; 3],
    pub podium_rot: [f32; 3],
    pub face: [f32; 3],
    pub normal: [f32; 3],
    pub start_pos: [f32; 3],
    pub start_yaw: f32,
    pub car_pos: [f32; 3],
    pub start_block: Option<(usize, String, String)>,
    pub start_item: Option<(usize, String, String)>,
    pub new_item_index: usize,
    pub new_name: String,
    pub new_uid: String,
}

/// The pack's finish twin of a start model: `Start` → `Finish` in the name,
/// verified to exist with waypoint type Finish.
fn finish_twin_block(ctx: &mut Ctx, name: &str) -> Result<String, String> {
    if !name.contains("Start") {
        return Err(format!("{name}: no `Start` in the name"));
    }
    let cand = name.replacen("Start", "Finish", 1);
    let Some(path) = ctx.idx.resolve_one(ctx.store, &cand) else {
        return Err(format!("{cand}: not in the pack"));
    };
    let bi = ctx.idx.load(ctx.store, &path).map_err(|e| format!("{cand}: {e}"))?;
    if bi.waypoint_type != Some(WP_FINISH) {
        return Err(format!("{cand}: waypoint type {:?}, not Finish", bi.waypoint_type));
    }
    Ok(cand)
}

/// A start GATE item's finish twin: the pack has `GateStart{Left,Center,Right}{8,16,32}m`
/// but only `GateFinish{8,16,32}m` (both poles) and `GateFinishCenter{8,16,32}m[v2]`
/// (poleless) — so every start gate of width W becomes `GateFinishCenter{W}mv2` (the
/// Fall 2026 finishes' own generation), else the v1 centre, else `GateFinish{W}m`,
/// else the plain `Start`→`Finish` spelling.
fn finish_twin_item(ctx: &mut Ctx, model: &str) -> Result<String, String> {
    if !model.contains("Start") {
        return Err(format!("{model}: no `Start` in the name"));
    }
    let mut cands: Vec<String> = Vec::new();
    if let Some(w) = model.trim_end_matches('m').rsplit(|c: char| !c.is_ascii_digit()).next().filter(|w| !w.is_empty()) {
        cands.push(format!("GateFinishCenter{w}mv2"));
        cands.push(format!("GateFinishCenter{w}m"));
        cands.push(format!("GateFinish{w}m"));
    }
    cands.push(model.replacen("Start", "Finish", 1));
    let mut tried = Vec::new();
    for cand in cands {
        match ctx.item_wp(&cand) {
            (Some(WP_FINISH), _) => return Ok(cand),
            (other, resolved) => tried.push(format!("{cand} ({})", if resolved { format!("type {other:?}") } else { "absent".into() })),
        }
    }
    Err(format!("{model}: no finish gate twin — tried {}", tried.join(", ")))
}

fn rot_apply(rot: [f32; 3], v: [f32; 3]) -> [f32; 3] {
    let m = crate::place::free([0.0; 3], rot);
    crate::geom::apply(&m, v)
}

/// The step after `strip_waypoints`: the start swap and the podium start.
pub fn podium_reverse(ctx: &mut Ctx, src: &Path, out: &Path, o: &PodiumOpts, dry: bool) -> Result<PodiumOutcome, String> {
    let map_label = src.file_name().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
    let mut m = MapFile::try_load(src)?;
    let hdr = tmmaps::header::read(src.to_str().unwrap_or_default())?;
    let mut rows: Vec<Row> = Vec::new();
    // ---- the podium
    let podiums: Vec<ItemRec> = m.items.iter().filter(|it| it.model.eq_ignore_ascii_case("Podium")).cloned().collect();
    // the old start's world position (block: its cell centre / free anchor; item: its pos)
    let old_start: Option<[f32; 3]> = {
        let coll = m.body_collections().map(|c| c[0].1).unwrap_or(26);
        let ground = tmmaps::map::ground_y(coll);
        let mut found: Option<[f32; 3]> = None;
        for b in m.blocks.iter().filter(|b| b.flags != 0xFFFF_FFFF) {
            if ctx.block_class(b).effective() == Some(WP_START) {
                found = Some(match b.free_pos {
                    Some(p) => p,
                    None => {
                        let c = b.coords();
                        [c.0 as f32 * 32.0 + 16.0, c.1 as f32 * 8.0 + ground, c.2 as f32 * 32.0 + 16.0]
                    }
                });
                break;
            }
        }
        if found.is_none() {
            for it in &m.items {
                if ctx.item_class(it).effective() == Some(WP_START) {
                    found = Some(it.pos);
                    break;
                }
            }
        }
        found
    };
    let mut podium_note = String::new();
    let podium = match (podiums.len(), o.podium) {
        (0, _) => return Err(format!("{map_label}: no Podium item")),
        (1, _) => podiums[0].clone(),
        (n, Some(k)) if k < n => podiums[k].clone(),
        (n, Some(k)) => return Err(format!("{map_label}: --podium {k} but the map has {n} podiums")),
        (_, None) => {
            let st = old_start.ok_or_else(|| format!("{map_label}: several podiums and no start to measure from"))?;
            let mut ranked: Vec<(f32, &ItemRec)> = podiums.iter().map(|p| (((p.pos[0] - st[0]).powi(2) + (p.pos[1] - st[1]).powi(2) + (p.pos[2] - st[2]).powi(2)).sqrt(), p)).collect();
            ranked.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());
            podium_note = format!("nearest the old start {}: {}", pos_str(st), ranked.iter().map(|(d, p)| format!("item#{} at {} = {:.1} m", p.index, pos_str(p.pos), d)).collect::<Vec<_>>().join(", "));
            ranked[0].1.clone()
        }
    };
    let prot = [podium.yaw, podium.pitch, podium.roll];
    let up = rot_apply(prot, [0.0, 1.0, 0.0]);
    if up[1] < 0.99 {
        return Err(format!("{map_label}: the podium's up vector is {:?} (rot {:?}) — not standing upright", up, prot));
    }
    let normal = rot_apply(prot, [0.0, 0.0, 1.0]);
    let face = {
        let f = rot_apply(prot, [0.0, 0.0, PODIUM_FRONT_Z]);
        [podium.pos[0] + f[0], podium.pos[1] + f[1], podium.pos[2] + f[2]]
    };
    let yaw_s = normal[0].atan2(normal[2]);
    let d_car = o.rear + o.gap;
    let car_pos = [face[0] + normal[0] * d_car, podium.pos[1], face[2] + normal[2] * d_car];
    let d_gate = d_car - GATE_SPAWN_Z; // the gate stands 10.6 m past the car
    let start_pos = [face[0] + normal[0] * d_gate, podium.pos[1], face[2] + normal[2] * d_gate];
    {
        let mut r = Row::new(&map_label, "podium", "item", podium.index, &podium.model);
        r.action = "reference".into();
        r.from = pos_str(podium.pos);
        r.to = format!("face {} normal ({:.4},{:.4}) yaw {:.4}", pos_str(face), normal[0], normal[2], yaw_s);
        r.note = format!("rot {:?}; {} podium(s) in the map{}", prot, podiums.len(), if podiums.len() > 1 { format!(" — item#{} chosen ({})", podium.index, if podium_note.is_empty() { format!("--podium {}", o.podium.unwrap_or(0)) } else { podium_note.clone() }) } else { String::new() });
        rows.push(r);
    }
    // ---- the start → finish
    let mut start_block: Option<(usize, String, String)> = None;
    let mut start_item: Option<(usize, String, String)> = None;
    let mut starts = 0usize;
    for (i, b) in m.blocks.clone().iter().enumerate() {
        if b.flags == 0xFFFF_FFFF {
            continue;
        }
        let c = ctx.block_class(b);
        match c.effective() {
            Some(WP_START) => {
                starts += 1;
                let twin = finish_twin_block(ctx, &b.name)?;
                let mut r = Row::new(&map_label, "podium", "block", i, &b.name);
                r.tag = b.waypoint_tag.clone().unwrap_or_default();
                r.model_wp = wp_str(c.model);
                r.action = "start-to-finish".into();
                r.to_name = twin.clone();
                r.dir_from = b.dir.to_string();
                r.dir_to = b.dir.to_string();
                r.from = match b.free_pos {
                    Some(p) => format!("free {}", pos_str(p)),
                    None => cell_str(b.coords()),
                };
                r.to = r.from.clone();
                r.note = format!("flags {:08X} kept; waypoint node tag Spawn -> Goal", b.flags);
                rows.push(r);
                start_block = Some((i, b.name.clone(), twin));
            }
            Some(WP_STARTFINISH) => return Err(format!("{map_label}: block#{i} {} is a StartFinish (multilap) — not handled", b.name)),
            _ => {}
        }
    }
    for (i, it) in m.items.clone().iter().enumerate() {
        let c = ctx.item_class(it);
        match c.effective() {
            Some(WP_START) => {
                starts += 1;
                let twin = finish_twin_item(ctx, &it.model)?;
                let mut r = Row::new(&map_label, "podium", "item", i, &it.model);
                r.tag = it.waypoint_tag.clone().unwrap_or_default();
                r.model_wp = wp_str(c.model);
                r.action = "start-to-finish".into();
                r.to_name = twin.clone();
                r.from = pos_str(it.pos);
                r.to = r.from.clone();
                r.note = "model renamed in place; tag Spawn -> Goal".into();
                rows.push(r);
                start_item = Some((i, it.model.clone(), twin));
            }
            Some(WP_STARTFINISH) => return Err(format!("{map_label}: item#{i} {} is a StartFinish — not handled", it.model)),
            _ => {}
        }
    }
    if starts != 1 {
        return Err(format!("{map_label}: {starts} start placements (block + item), want exactly 1"));
    }
    // ---- the new start item row
    let new_item_index = m.items.len();
    {
        let mut r = Row::new(&map_label, "podium", "item", new_item_index, &o.start_item);
        r.tag = "Spawn".into();
        r.model_wp = "Start".into();
        r.action = "added".into();
        r.to = format!("{} yaw {:.4}", pos_str(start_pos), yaw_s);
        r.note = format!("the car at {} ({:.2} m in front of the podium face = rear {:.2} + gap {:.2}); the gate {:.1} m in front of the car", pos_str(car_pos), d_car, o.rear, o.gap, -GATE_SPAWN_Z);
        rows.push(r);
    }
    let new_name = format!("{}{}", hdr.name, o.name_suffix);
    let old_uid = hdr.uid.clone();
    let new_uid = if old_uid.len() > o.uid_prefix.len() { format!("{}{}", o.uid_prefix, &old_uid[..old_uid.len() - o.uid_prefix.len()]) } else { old_uid.clone() };
    {
        let mut r = Row::new(&map_label, "podium", "map", 0, &new_name);
        r.action = "identity".into();
        r.to_name = new_uid.clone();
        r.note = format!("uid {old_uid} -> {new_uid}; times {}; validated 0; {}", o.times.map(|t| format!("author {} gold {} silver {} bronze {} ms", t.3, t.2, t.1, t.0)).unwrap_or_else(|| "the source's".into()), if o.unlock { "unlocked" } else { "password kept" });
        rows.push(r);
    }
    let outcome = |rows: Vec<Row>| PodiumOutcome {
        rows,
        podium_pos: podium.pos,
        podium_rot: prot,
        face,
        normal,
        start_pos,
        start_yaw: yaw_s,
        car_pos,
        start_block: start_block.clone(),
        start_item: start_item.clone(),
        new_item_index,
        new_name: new_name.clone(),
        new_uid: new_uid.clone(),
    };
    if dry {
        return Ok(outcome(rows));
    }
    if let Some(dir) = out.parent() {
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    // pass 1 (splices + patches, no rename): the start block's tag, the ghost, the times, the lock
    if let Some((i, _, _)) = &start_block {
        if !m.set_block_waypoint_tag(*i, "Goal") {
            return Err(format!("{map_label}: block#{i}: the waypoint node is not the plain layout (skinned or shared) — tag not rewritten"));
        }
    }
    m.strip_validation_ghost_to(tmmaps::map::GhostForm::Remove);
    if let Some((b_, s_, g_, a_)) = o.times {
        m.set_times(&hdr, b_, s_, g_, a_, false);
    }
    if o.unlock {
        m.remove_password();
    }
    m.write_to(out).map_err(|e| e.to_string())?;
    // pass 2 (renames: the Id table): the block twin, the item twin
    if start_block.is_some() || start_item.is_some() {
        let mut m2 = MapFile::try_load(out)?;
        if let Some((i, _, twin)) = &start_block {
            m2.set_block_name(*i, twin);
        }
        if let Some((i, _, twin)) = &start_item {
            m2.set_item_model(*i, twin);
        }
        m2.write_to(out).map_err(|e| e.to_string())?;
    }
    // pass 3 (splices): the map name (header + the body's string copies), the start item's tag Goal
    {
        let mut m3 = MapFile::try_load(out)?;
        let (h, b) = m3.set_map_name(&hdr.name, &new_name);
        if h + b == 0 {
            return Err(format!("{map_label}: the map does not declare the name {:?}", hdr.name));
        }
        if let Some((i, _, _)) = &start_item {
            m3.set_item_waypoint(*i, Some("Goal"), 0);
        }
        m3.write_to(out).map_err(|e| e.to_string())?;
    }
    // pass 4: the new start item — a clone appended, then its fields
    {
        let mut m4 = MapFile::try_load(out)?;
        m4.append_item_clones(new_item_index + 1);
        m4.write_to(out).map_err(|e| e.to_string())?;
    }
    {
        // renames: model + author (their own write)
        let mut m5 = MapFile::try_load(out)?;
        m5.set_item_model(new_item_index, &o.start_item);
        m5.set_item_author(new_item_index, "Nadeo");
        m5.write_to(out).map_err(|e| e.to_string())?;
    }
    {
        // patches + the waypoint splice
        let mut m6 = MapFile::try_load(out)?;
        let it = m6.items[new_item_index].clone();
        if it.skin_region.is_some() || it.flags & 4 != 0 {
            m6.set_item_skin(new_item_index, None);
        }
        m6.move_item_pos(new_item_index, start_pos);
        m6.set_item_rotation(new_item_index, yaw_s, 0.0, 0.0);
        m6.set_item_pivot(new_item_index, [0.0, 0.0, 0.0]);
        m6.set_item_flags(new_item_index, 0);
        m6.set_item_scale(new_item_index, 1.0);
        let cell = [((start_pos[0] / 32.0).floor() as i32).clamp(0, 255) as u8, (((start_pos[1] - tmmaps::map::ground_y(it.collection_raw)) / 8.0).floor() as i32).clamp(0, 255) as u8, ((start_pos[2] / 32.0).floor() as i32).clamp(0, 255) as u8];
        m6.set_item_cell(new_item_index, cell);
        m6.set_item_waypoint(new_item_index, Some("Spawn"), 0);
        m6.write_to(out).map_err(|e| e.to_string())?;
    }
    // pass 5: the uid (a rename of the first Id: its own write)
    if new_uid != old_uid {
        let mut m7 = MapFile::try_load(out)?;
        m7.set_map_uid_any_len(&new_uid);
        m7.write_to(out).map_err(|e| e.to_string())?;
    }
    Ok(outcome(rows))
}

/// Round-trip checks of `podium_reverse`'s output against its input.
pub fn verify_podium(src: &Path, out: &Path, oc: &PodiumOutcome, o: &PodiumOpts) -> Result<Vec<String>, String> {
    let a = MapFile::try_load(src)?;
    let b = MapFile::try_load(out)?;
    let mut bad = Vec::new();
    if a.blocks.len() != b.blocks.len() {
        bad.push(format!("block count {} -> {}", a.blocks.len(), b.blocks.len()));
    }
    if a.baked.len() != b.baked.len() {
        bad.push(format!("baked count {} -> {}", a.baked.len(), b.baked.len()));
    }
    if a.items.len() + 1 != b.items.len() {
        bad.push(format!("item count {} -> {} (want +1)", a.items.len(), b.items.len()));
    }
    for (i, (x, y)) in a.blocks.iter().zip(b.blocks.iter()).enumerate() {
        let is_start = oc.start_block.as_ref().map(|(k, _, _)| *k == i).unwrap_or(false);
        let want_name = if is_start { oc.start_block.as_ref().unwrap().2.as_str() } else { x.name.as_str() };
        let want_tag = if is_start { Some("Goal".to_string()) } else { x.waypoint_tag.clone() };
        if y.name != want_name || y.dir != x.dir || y.flags != x.flags || y.coords() != x.coords() || y.free_pos != x.free_pos || y.free_rot != x.free_rot || y.waypoint_tag != want_tag {
            bad.push(format!("block#{i} {} -> {} differs (tag {:?} -> {:?}, flags {:08X} -> {:08X})", x.name, y.name, x.waypoint_tag, y.waypoint_tag, x.flags, y.flags));
        }
    }
    for (x, y) in a.baked.iter().zip(b.baked.iter()) {
        if x.name != y.name || x.dir != y.dir || x.coords() != y.coords() || x.free_pos != y.free_pos || x.flags != y.flags {
            bad.push(format!("baked#{} {} differs", x.index, x.name));
            break;
        }
    }
    for (i, (x, y)) in a.items.iter().zip(b.items.iter()).enumerate() {
        let is_start = oc.start_item.as_ref().map(|(k, _, _)| *k == i).unwrap_or(false);
        let want_model = if is_start { oc.start_item.as_ref().unwrap().2.as_str() } else { x.model.as_str() };
        let want_tag = if is_start { Some("Goal".to_string()) } else { x.waypoint_tag.clone() };
        if y.model != want_model || y.pos != x.pos || (y.yaw - x.yaw).abs() > 1e-6 || y.waypoint_tag != want_tag || y.coords() != x.coords() {
            bad.push(format!("item#{i} {} -> {} differs (tag {:?} -> {:?})", x.model, y.model, x.waypoint_tag, y.waypoint_tag));
        }
    }
    if let Some(n) = b.items.get(oc.new_item_index) {
        let d = ((n.pos[0] - oc.start_pos[0]).powi(2) + (n.pos[1] - oc.start_pos[1]).powi(2) + (n.pos[2] - oc.start_pos[2]).powi(2)).sqrt();
        if n.model != o.start_item || d > 1e-3 || (n.yaw - oc.start_yaw).abs() > 1e-5 || n.pitch.abs() > 1e-6 || n.roll.abs() > 1e-6 || n.waypoint_tag.as_deref() != Some("Spawn") || n.pivot != [0.0, 0.0, 0.0] || n.author.as_deref() != Some("Nadeo") {
            bad.push(format!("new item#{} {} at {} yaw {:.4} pitch {} roll {} tag {:?} pivot {:?} author {:?} — not the planned {} at {} yaw {:.4}", oc.new_item_index, n.model, pos_str(n.pos), n.yaw, n.pitch, n.roll, n.waypoint_tag, n.pivot, n.author, o.start_item, pos_str(oc.start_pos), oc.start_yaw));
        }
        if n.collection_raw != a.items.first().map(|f| f.collection_raw).unwrap_or(26) {
            bad.push(format!("new item collection {} != the map's {}", n.collection_raw, a.items.first().map(|f| f.collection_raw).unwrap_or(26)));
        }
    } else {
        bad.push("the new start item is missing".into());
    }
    // exactly one Spawn and the finish count
    let spawns = b.blocks.iter().filter(|x| x.waypoint_tag.as_deref() == Some("Spawn")).count() + b.items.iter().filter(|x| x.waypoint_tag.as_deref() == Some("Spawn")).count();
    if spawns != 1 {
        bad.push(format!("{spawns} Spawn placements in the output, want 1"));
    }
    let goals = b.blocks.iter().filter(|x| x.waypoint_tag.as_deref() == Some("Goal")).count() + b.items.iter().filter(|x| x.waypoint_tag.as_deref() == Some("Goal")).count();
    if goals != 1 {
        bad.push(format!("{goals} Goal placements in the output, want 1 (the converted start)"));
    }
    let hb = tmmaps::header::read(out.to_str().unwrap_or_default())?;
    if hb.validated != "0" {
        bad.push(format!("header validated={}", hb.validated));
    }
    if hb.name != oc.new_name {
        bad.push(format!("header name {:?} != {:?}", hb.name, oc.new_name));
    }
    if hb.uid != oc.new_uid {
        bad.push(format!("header uid {} != {}", hb.uid, oc.new_uid));
    }
    if let Some(t) = o.times {
        if hb.bronze != t.0.to_string() || hb.silver != t.1.to_string() || hb.gold != t.2.to_string() || hb.authortime != t.3.to_string() {
            bad.push(format!("header times {}/{}/{}/{} != {:?}", hb.bronze, hb.silver, hb.gold, hb.authortime, t));
        }
    }
    if o.unlock && crate::sttc::has_chunk(&b.gbx.body, 0x0304_3029) {
        bad.push("password chunk 0x03043029 still present".into());
    }
    Ok(bad)
}

pub struct PodiumPipelineOpts {
    pub podium: PodiumOpts,
    pub dry: bool,
    pub keep_renumber: bool,
    pub pak_specs: Vec<String>,
    pub out_by_map_name: bool,
}

/// One map: `out_dir/sttf/<stem>.nofinish.Map.Gbx` (checkpoints + finishes
/// gone), then `out_dir/<name>.Map.Gbx`; the lightmap renumbered as in sttc.
pub fn pipeline(store: &mut crate::store::DataStore, src: &Path, out_dir: &Path, o: &PodiumPipelineOpts) -> Result<(Vec<Row>, String, Option<PodiumOutcome>), String> {
    let stem = src.file_name().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
    let stem = stem.trim_end_matches(".Map.Gbx").to_string();
    let hdr = tmmaps::header::read(src.to_str().unwrap_or_default())?;
    let strip_out: PathBuf = out_dir.join("sttf").join(format!("{stem}.nofinish.Map.Gbx"));
    let final_out: PathBuf = if o.out_by_map_name {
        out_dir.join(format!("{}.Map.Gbx", format!("{}{}", hdr.name, o.podium.name_suffix).replace(['/', '\\', ':'], "-")))
    } else {
        out_dir.join(format!("{stem}-Podium-Reverse.Map.Gbx"))
    };
    let m = MapFile::try_load(src)?;
    let mut rows = Vec::new();
    let mut ctx = Ctx::new(store, &m);
    let s: SttfOutcome = crate::sttc::strip_waypoints(&mut ctx, src, &strip_out, crate::sttc::CpMode::Plain, o.dry, &[crate::sttc::WP_CHECKPOINT, WP_FINISH])?;
    let mut bad_s = Vec::new();
    if !o.dry {
        bad_s = crate::sttc::verify_sttf(src, &strip_out, &s)?;
    }
    let step_src: &Path = if o.dry { src } else { &strip_out };
    let m2 = MapFile::try_load(step_src)?;
    let mut ctx2 = Ctx::new(ctx.store, &m2);
    let p = podium_reverse(&mut ctx2, step_src, &final_out, &o.podium, o.dry)?;
    let mut bad_p = Vec::new();
    if !o.dry {
        bad_p = verify_podium(step_src, &final_out, &p, &o.podium)?;
    }
    rows.extend(s.rows.iter().cloned());
    rows.extend(p.rows.iter().cloned());
    let mut r = Row::new(&format!("{stem}.Map.Gbx"), "summary", "map", 0, &format!("{}{}", m.size[0], if m.size[0] == 48 { " (Stadium)" } else { "" }));
    r.action = format!(
        "strip: {} replaced, {} blocks removed, {} items removed, {} baked removed; start {} -> finish {}; new {} at {} yaw {:.4} (car at {}, podium face {}, {} podiums); {} mismatches",
        s.replaced,
        s.removed_blocks,
        s.removed_items,
        s.removed_baked,
        p.start_block.as_ref().map(|(_, a, _)| a.clone()).or_else(|| p.start_item.as_ref().map(|(_, a, _)| a.clone())).unwrap_or_default(),
        p.start_block.as_ref().map(|(_, _, t)| t.clone()).or_else(|| p.start_item.as_ref().map(|(_, _, t)| t.clone())).unwrap_or_default(),
        o.podium.start_item,
        pos_str(p.start_pos),
        p.start_yaw,
        pos_str(p.car_pos),
        pos_str(p.face),
        m.items.iter().filter(|it| it.model.eq_ignore_ascii_case("Podium")).count(),
        s.mismatches
    );
    r.to_name = p.new_name.clone();
    if let Some(pn) = p.rows.iter().find(|x| x.step == "podium" && x.action == "reference") {
        if pn.note.contains("nearest") {
            r.action = format!("{}; podium: {}", r.action, pn.note.splitn(2, " — ").nth(1).unwrap_or(""));
        }
    }
    let mut summary = String::new();
    if !bad_s.is_empty() || !bad_p.is_empty() {
        r.note = format!("VERIFY FAILED: {} {}", bad_s.join("; "), bad_p.join("; "));
        rows.push(r.clone());
        return Err(format!("{stem}: {}", r.note));
    }
    r.note = if o.dry { "dry run".into() } else { "verified".into() };
    rows.push(r.clone());
    summary = r.tsv();
    if !o.dry && o.keep_renumber {
        let m_final_before = MapFile::try_load(&final_out)?;
        let mut ctx3 = Ctx::new(ctx2.store, &m_final_before);
        let rc = crate::sttc::reconcile_clips(&mut ctx3, src, &final_out)?;
        let mut rr = Row::new(&format!("{stem}.Map.Gbx"), "reconcile", "baked", 0, "generated records vs the engine's derived clips");
        rr.action = format!("{} records removed, {} added; source sim {} stale / {} missing, output before {} / {}, residue after {} / {}", rc.removed.len(), rc.added.len(), rc.src_stale, rc.src_missing, rc.out_stale, rc.out_missing, rc.residue_stale, rc.residue_missing);
        rr.note = if rc.residue_stale == 0 && rc.residue_missing == 0 { "reconciled".into() } else { "RESIDUE".into() };
        // the object map: the strip rows (sttf vocabulary) + the start swap as "replaced" rows
        let mut swap_rows: Vec<Row> = Vec::new();
        if let Some((i, a, _)) = &p.start_block {
            let mut x = Row::new(&stem, "sttf", "block", *i, a);
            x.action = "replaced".into();
            swap_rows.push(x);
        }
        if let Some((i, a, _)) = &p.start_item {
            let mut x = Row::new(&stem, "sttf", "item", *i, a);
            x.action = "replaced".into();
            swap_rows.push(x);
        }
        // the swap rows index the STRIP OUTPUT's records; map them back to the source's indices
        let strip_removed_blocks: Vec<usize> = { let mut v: Vec<usize> = s.rows.iter().filter(|x| x.step == "sttf" && x.kind == "block" && x.action == "removed").filter_map(|x| x.index.parse().ok()).collect(); v.sort_unstable(); v };
        let strip_removed_items: Vec<usize> = { let mut v: Vec<usize> = s.rows.iter().filter(|x| x.step == "sttf" && x.kind == "item" && x.action == "removed").filter_map(|x| x.index.parse().ok()).collect(); v.sort_unstable(); v };
        let back = |mid: usize, removed: &[usize]| -> usize {
            let mut i = mid;
            for r in removed {
                if *r <= i { i += 1; } else { break; }
            }
            i
        };
        let mut all_rows: Vec<Row> = s.rows.clone();
        for mut x in swap_rows {
            let mid: usize = x.index.parse().unwrap_or(0);
            x.index = if x.kind == "block" { back(mid, &strip_removed_blocks) } else { back(mid, &strip_removed_items) }.to_string();
            all_rows.push(x);
        }
        let objmap = crate::sttc::objmap_rows(&m, &all_rows, &[], &[], &rc.removed);
        let pth = out_dir.join(format!("{stem}.objmap.tsv"));
        std::fs::write(&pth, objmap).map_err(|e| e.to_string())?;
        let lmtool = std::env::current_exe().ok().and_then(|e| e.parent().map(|d| d.join("lmtool"))).filter(|p| p.exists()).map(|p| p.display().to_string()).unwrap_or_else(|| "lmtool".to_string());
        let lit = out_dir.join(format!("{stem}.lit.tmp.Map.Gbx"));
        let mut cmd = std::process::Command::new(&lmtool);
        cmd.arg("sttc-relight");
        for spec in &o.pak_specs {
            cmd.arg("--pak").arg(spec);
        }
        cmd.arg("--source").arg(src).arg("--map").arg(&final_out).arg("--objmap").arg(&pth).arg("--out").arg(&lit);
        let outp = cmd.output().map_err(|e| format!("{lmtool}: {e}"))?;
        let text = format!("{}{}", String::from_utf8_lossy(&outp.stdout), String::from_utf8_lossy(&outp.stderr));
        if !outp.status.success() {
            return Err(format!("{stem}: lmtool sttc-relight failed: {}", text.trim()));
        }
        std::fs::rename(&lit, &final_out).map_err(|e| e.to_string())?;
        rr.to_name = text.lines().next().unwrap_or("").trim().to_string();
        rows.push(rr);
        if let Some(sum) = rows.iter_mut().find(|r| r.step == "summary") {
            sum.action = format!("{}; lightmap kept: {}", sum.action, text.lines().next().unwrap_or("").trim().trim_start_matches("wrote ").splitn(2, ": ").nth(1).unwrap_or(""));
        }
        summary = rows.iter().find(|r| r.step == "summary").map(|r| r.tsv()).unwrap_or(summary);
    }
    let _ = HashMap::<u8, u8>::new();
    let _ = FREE_BLOCK_FLAG;
    Ok((rows, summary, Some(p)))
}
