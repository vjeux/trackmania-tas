//! THE RECORD LIST OF A MAP THE WAY THE LIGHTMAPPER BUILDS IT — `lmtool records-check MAP --pak FILE:KEY --dump TSV
//! [--collection Stadium] [--zone Grass] [--grid 96] [--cell-y 1] [--yoff 0]`: our records (the zone tiles, the blocks'
//! prefab entities, the items) against the baker's /lmrecords dump (passcap/*-records/lmrecords-*-H.tsv), matched by
//! centre + MeterByUv; per class: matched / missing / extra, the order, the quality.

/// One record as the game keeps it (the fields the layout needs).
#[derive(Clone, Debug)]
pub struct Rec {
    /// A label for the class (tile / block / clip / item).
    pub class: &'static str,
    /// The object id the game gives it (when known) and the sub index.
    pub obj: u32,
    pub sub: u32,
    pub meter_by_uv: f32,
    pub uv: [f32; 4],
    pub quality: f32,
    pub centre: [f32; 3],
    pub half: [f32; 3],
    /// The group key: (the PreLightGen's identity, q bits).
    pub group: u64,
    /// The centre the Morton ordinal is taken from when it is not the record box's (the study of the clip records' key).
    pub key_centre: Option<[f32; 3]>,
    /// The record's rank in the static pool (its group position) when that is not the record order.
    pub pos_rank: Option<u32>,
    /// A WALL record (RE 7: the Base_VFCMiddle_Air model with an axis-aligned third Iso4 row): (facing code, height 2·h.y).
    pub wall: Option<(u8, f32)>,
}

impl Rec {
    pub fn ext(&self) -> [f32; 2] {
        let f = self.meter_by_uv * self.quality;
        [(self.uv[2] - self.uv[0]) * f, (self.uv[3] - self.uv[1]) * f]
    }
}

/// A dump row (the columns `records-check` reads).
#[derive(Clone, Debug)]
pub struct DumpRec {
    pub i: usize,
    pub meter_by_uv: f32,
    pub uv: [f32; 4],
    pub quality: f32,
    pub centre: [f32; 3],
    pub half: [f32; 3],
    pub key: u64,
    pub chart: (i32, i32, i32, i32),
}

pub fn read_dump(path: &str) -> Result<Vec<DumpRec>, String> {
    let txt = std::fs::read_to_string(path).map_err(|e| format!("{path}: {e}"))?;
    let mut lines = txt.lines();
    let header: Vec<&str> = lines.next().ok_or("empty dump")?.split('\t').collect();
    let col = |name: &str| header.iter().position(|h| *h == name).ok_or_else(|| format!("{path}: no column {name}"));
    let (ci, cm, cu0, cv0, cu1, cv1, cq, cx, cy, cz, chx, chy, chz, ck) = (col("i")?, col("meterByUv")?, col("u0")?, col("v0")?, col("u1")?, col("v1")?, col("quality")?, col("centerX")?, col("centerY")?, col("centerZ")?, col("halfX")?, col("halfY")?, col("halfZ")?, col("key")?);
    let (ccx, ccy, ccw, cch) = (col("cx")?, col("cy")?, col("cw")?, col("ch")?);
    let mut out = Vec::new();
    for l in lines {
        let f: Vec<&str> = l.split('\t').collect();
        if f.len() < header.len() { continue; }
        let p = |k: usize| -> f32 { f[k].trim().parse::<f32>().unwrap_or(f32::NAN) };
        out.push(DumpRec {
            i: f[ci].trim().parse().unwrap_or(0),
            meter_by_uv: p(cm),
            uv: [p(cu0), p(cv0), p(cu1), p(cv1)],
            quality: p(cq),
            centre: [p(cx), p(cy), p(cz)],
            half: [p(chx), p(chy), p(chz)],
            key: u64::from_str_radix(f[ck].trim(), 16).unwrap_or(0),
            chart: (f[ccx].trim().parse().unwrap_or(-1), f[ccy].trim().parse().unwrap_or(-1), f[ccw].trim().parse().unwrap_or(-1), f[cch].trim().parse().unwrap_or(-1)),
        });
    }
    Ok(out)
}

/// The zone tiles of a collection's ground: `grid` × `grid` cells of 32 m from the world origin, the zone's prefab entity 0
/// as the tile (PLG + visual boxes), placed at y = cell_y·8 + yoff with the Dir rotation about the cell centre. Returns the
/// records in cell order (z rows, x columns — the game's generated-tile order is x-major: x outer, z inner, as the dump's
/// tiles show (x constant over 96 consecutive records)).
pub fn zone_tiles(store: &mut mapgeom::store::DataStore, collection: &str, zone: &str, grid: usize, cell_y: f32, yoff: f32, quality: &dyn Fn(usize, usize) -> f32) -> Result<Vec<Rec>, String> {
    // the zone prefab: PLG + boxes (as lmtiles::tile_records finds them)
    let mut plg: Option<(f32, [f32; 4])> = None;
    let mut boxes: Vec<crate::lmtiles::CBox> = Vec::new();
    for (fam, ext) in [("GameCtnBlockInfoFlat", "EDFlat"), ("GameCtnBlockInfoFrontier", "EDFrontier"), ("GameCtnBlockInfoTransition", "EDTransition"), ("GameCtnBlockInfoClassic", "EDClassic")] {
        let path = format!("{collection}\\GameCtnBlockInfo\\{fam}\\{zone}.{ext}.Gbx");
        let Ok(bi) = mapgeom::blockinfo::load(store, &path) else { continue };
        let Some(v) = bi.variant_base_ground.as_ref() else { continue };
        let Some(pp) = v.mobils.iter().flatten().find_map(|m| m.prefab.clone()) else { continue };
        let pm = store.load_model(&pp)?;
        let pf = mapgeom::static_item::prefab::CPlugPrefab::from_model(&pm)?;
        if std::env::var("LMTOOL_ZONE_TRACE").is_ok() {
            eprintln!("zone {collection}/{zone}: {path} → {pp}: {} entities", pf.ents.len());
            for (i, e) in pf.ents.iter().enumerate() {
                if let Some(mapgeom::static_item::Node::StaticObject(so)) = e.model.inline.as_deref() { if let Some(s2) = so.solid2() { eprintln!("  entity {i}: PLG {:?} at {:?}, {} shaded geoms", s2.pre_light_gen.as_ref().map(|p| (p.u01, p.u02, p.u04)), e.pos, s2.shaded_geoms.len()); } else { eprintln!("  entity {i}: static object without solid2"); } } else { eprintln!("  entity {i}: model index {} (external / other)", e.model.index); }
            }
        }
        for e in &pf.ents {
            let Some(mapgeom::static_item::Node::StaticObject(so)) = e.model.inline.as_deref() else { continue };
            let Some(s2) = so.solid2() else { continue };
            if let Some(p) = &s2.pre_light_gen { plg = Some((p.u02, [p.u04[0], p.u04[1], p.u04[2], p.u04[3]])); }
            for sg in &s2.shaded_geoms {
                let Some(vr) = s2.visuals.get(sg.visual_index as usize) else { continue };
                let Some(mapgeom::static_item::Node::Visual(vis)) = vr.inline.as_deref() else { continue };
                if let Some(mm) = vis.main.as_ref() { let b = mm.bounding_box; boxes.push(crate::lmtiles::CBox::new([b[0], b[1], b[2]], [b[3], b[4], b[5]])); }
            }
            break;
        }
        break;
    }
    let Some((mbu, uv)) = plg else { return Err(format!("{collection}/{zone}: no zone prefab PLG found")) };
    let Some(mb) = crate::lmtiles::model_box(&boxes) else { return Err(format!("{collection}/{zone}: no visual boxes")) };
    let mut out = Vec::with_capacity(grid * grid);
    for cx in 0..grid {
        for cz in 0..grid {
            let iso: crate::lmtiles::Iso4 = [1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0, cx as f32 * 32.0, cell_y * 8.0 + yoff, cz as f32 * 32.0];
            let w = mb.transformed(&iso);
            let q = quality(cx, cz);
            out.push(Rec { class: "tile", obj: 0, sub: 0, meter_by_uv: mbu, uv, quality: q, centre: w.c, half: w.h, group: 0xF000_0000_0000_0000 | q.to_bits() as u64, key_centre: None, pos_rank: None, wall: None });
        }
    }
    Ok(out)
}

/// Our record → the dump record it stands for (by centre + MeterByUv), or None.
pub fn match_dump(ours: &[Rec], dump: &[DumpRec]) -> Vec<Option<usize>> {
    let mut used = vec![false; dump.len()];
    let mut idx: std::collections::HashMap<(i64, i64, i64), Vec<usize>> = Default::default();
    for (j, d) in dump.iter().enumerate() { idx.entry(((d.centre[0] * 10.0).round() as i64, (d.centre[1] * 10.0).round() as i64, (d.centre[2] * 10.0).round() as i64)).or_default().push(j); }
    let mut out = Vec::with_capacity(ours.len());
    for r in ours {
        let key = ((r.centre[0] * 10.0).round() as i64, (r.centre[1] * 10.0).round() as i64, (r.centre[2] * 10.0).round() as i64);
        let mut found: Option<usize> = None;
        for dk in [key, (key.0 + 1, key.1, key.2), (key.0 - 1, key.1, key.2), (key.0, key.1, key.2 + 1), (key.0, key.1, key.2 - 1), (key.0, key.1 + 1, key.2), (key.0, key.1 - 1, key.2)] {
            if let Some(cands) = idx.get(&dk) {
                for &j in cands {
                    if used[j] { continue; }
                    let d = &dump[j];
                    if ((d.meter_by_uv - r.meter_by_uv) / d.meter_by_uv.max(1e-6)).abs() < 1e-4 && (d.centre[0] - r.centre[0]).abs() < 0.05 && (d.centre[1] - r.centre[1]).abs() < 0.05 && (d.centre[2] - r.centre[2]).abs() < 0.05 { found = Some(j); break; }
                }
            }
            if found.is_some() { break; }
        }
        if let Some(j) = found { used[j] = true; }
        out.push(found);
    }
    out
}

/// The grouped allocation over a record list (records.rs → layout::allocate_grouped): every record an "item" of the
/// LayoutInput in record order, its key the record box (centre, |h|²), its group the record's.
pub fn layout_of(recs: &[Rec], quality_index: u32) -> Result<crate::layout::GameLayout, String> {
    let items: Vec<(u32, [f32; 2], crate::layout::ChartKey, crate::layout::Charted)> = recs.iter().enumerate().map(|(i, r)| (i as u32, r.ext(), crate::layout::ChartKey { centre: r.key_centre.unwrap_or(r.centre), h2: (r.half[0] * r.half[0] + r.half[1] * r.half[1]) + r.half[2] * r.half[2] }, crate::layout::Charted::Bound)).collect();
    let groups: Vec<u64> = recs.iter().map(|r| r.group).collect();
    let any_pos = recs.iter().any(|r| r.pos_rank.is_some());
    let pos: Vec<u32> = recs.iter().enumerate().map(|(i, r)| r.pos_rank.unwrap_or(i as u32)).collect();
    let walls: Vec<Option<(u8, f32)>> = recs.iter().map(|r| if std::env::var("LMTOOL_NO_WALLS").is_ok() { None } else { r.wall }).collect();
    crate::layout::allocate_grouped_walls(&crate::layout::LayoutInput { tiles: Vec::new(), items, w_atlas: 2048, quality_index }, &groups, if any_pos { Some(&pos) } else { None }, Some(&walls))
}

/// Match our records to the dump's by centre (within 1e-2 m) and MeterByUv (within 1e-4 relative); prints the per-class
/// tallies and the first differences.
pub fn compare(ours: &[Rec], dump: &[DumpRec]) {
    let mut used = vec![false; dump.len()];
    let mut by_class: std::collections::BTreeMap<&str, (usize, usize, usize)> = Default::default(); // matched, q differs, missing
    let mut examples: Vec<String> = Vec::new();
    // an index by rounded centre
    let mut idx: std::collections::HashMap<(i64, i64, i64), Vec<usize>> = Default::default();
    for (j, d) in dump.iter().enumerate() { idx.entry(((d.centre[0] * 10.0).round() as i64, (d.centre[1] * 10.0).round() as i64, (d.centre[2] * 10.0).round() as i64)).or_default().push(j); }
    let mut order_pairs: Vec<(usize, usize)> = Vec::new();
    for (i, r) in ours.iter().enumerate() {
        let key = ((r.centre[0] * 10.0).round() as i64, (r.centre[1] * 10.0).round() as i64, (r.centre[2] * 10.0).round() as i64);
        let mut found: Option<usize> = None;
        for dk in [key, (key.0 + 1, key.1, key.2), (key.0 - 1, key.1, key.2), (key.0, key.1, key.2 + 1), (key.0, key.1, key.2 - 1), (key.0, key.1 + 1, key.2), (key.0, key.1 - 1, key.2)] {
            if let Some(cands) = idx.get(&dk) {
                for &j in cands {
                    if used[j] { continue; }
                    let d = &dump[j];
                    if ((d.meter_by_uv - r.meter_by_uv) / d.meter_by_uv.max(1e-6)).abs() < 1e-4 && (d.centre[0] - r.centre[0]).abs() < 0.05 && (d.centre[1] - r.centre[1]).abs() < 0.05 && (d.centre[2] - r.centre[2]).abs() < 0.05 { found = Some(j); break; }
                }
            }
            if found.is_some() { break; }
        }
        let e = by_class.entry(r.class).or_default();
        match found {
            Some(j) => {
                used[j] = true;
                e.0 += 1;
                if (dump[j].quality - r.quality).abs() > 1e-6 { e.1 += 1; if examples.len() < 8 { examples.push(format!("{} #{i} q {} vs dump #{j} q {} at {:?}", r.class, r.quality, dump[j].quality, r.centre)); } }
                order_pairs.push((i, j));
            }
            None => {
                e.2 += 1;
                if examples.len() < 12 {
                    // the nearest dump record of the same MeterByUv (unused or not): where the game put this piece
                    let nearest = dump.iter().filter(|d| ((d.meter_by_uv - r.meter_by_uv) / d.meter_by_uv.max(1e-6)).abs() < 1e-4).min_by(|a, b| { let da = (0..3).map(|k| (a.centre[k] - r.centre[k]).powi(2)).sum::<f32>(); let db = (0..3).map(|k| (b.centre[k] - r.centre[k]).powi(2)).sum::<f32>(); da.partial_cmp(&db).unwrap() });
                    examples.push(format!("{} #{i} (obj {} sub {}) MISSING: MBU {} centre {:?} half {:?}; nearest same-MBU dump record #{}: centre {:?} half {:?}", r.class, r.obj, r.sub, r.meter_by_uv, r.centre, r.half, nearest.map(|d| d.i).unwrap_or(0), nearest.map(|d| d.centre).unwrap_or([0.0; 3]), nearest.map(|d| d.half).unwrap_or([0.0; 3])));
                }
            }
        }
    }
    // the clip records by MeterByUv: matched / missing per piece kind
    let mut kinds: std::collections::BTreeMap<String, (usize, usize)> = Default::default();
    let matched_set: std::collections::HashSet<usize> = order_pairs.iter().map(|(a, _)| *a).collect();
    for (i, r) in ours.iter().enumerate() { if r.class.starts_with("clip") { let e = kinds.entry(format!("{} {:.4}", r.class, r.meter_by_uv)).or_default(); if matched_set.contains(&i) { e.0 += 1; } else { e.1 += 1; } } }
    if !kinds.is_empty() { println!("  clip kinds (MeterByUv: matched / missing): {}", kinds.iter().map(|(k, (m, n))| format!("{k}: {m}/{n}")).collect::<Vec<_>>().join(", ")); }
    let extra: Vec<usize> = (0..dump.len()).filter(|j| !used[*j]).collect();
    println!("records-check: {} ours vs {} dump", ours.len(), dump.len());
    for (c, (m, qd, miss)) in &by_class { println!("  {c}: {m} matched ({qd} with another quality), {miss} missing"); }
    println!("  dump records not matched by ours: {}", extra.len());
    let mut extra_classes: std::collections::BTreeMap<String, usize> = Default::default();
    for &j in &extra { *extra_classes.entry(format!("MBU {:.4} uv [{:.4} {:.4} {:.4} {:.4}] q {:.4}", dump[j].meter_by_uv, dump[j].uv[0], dump[j].uv[1], dump[j].uv[2], dump[j].uv[3], dump[j].quality)).or_default() += 1; }
    let mut v: Vec<_> = extra_classes.into_iter().collect(); v.sort_by_key(|(_, n)| std::cmp::Reverse(*n));
    for (k, n) in v.iter().take(12) { println!("    {n:5} × {k}"); }
    for ex in &examples { println!("  {ex}"); }
    // the order: is our record order the dump's (monotone dump indices along ours)?
    let inversions = order_pairs.windows(2).filter(|w| w[1].1 < w[0].1).count();
    println!("  order: {} matched pairs, {} descents of the dump index along our order (0 = the same order)", order_pairs.len(), inversions);
    if inversions > 0 { for w in order_pairs.windows(2).filter(|w| w[1].1 < w[0].1).take(5) { println!("    ours #{} → dump #{}, then ours #{} → dump #{}", w[0].0, w[0].1, w[1].0, w[1].1); } }
}

/// THE PREFAB ENTITIES' RECORDS (RE 7, 15:30Z): every entity with a PreLightGen of the prefab at `xf` (world), an
/// EXTERNAL prefab entity recursing in place, the records appended in entity order with running `sub`; the record box =
/// the entity's static object's visual boxes folded (`lmtiles::model_box`) through the entity pose × `xf`.
pub fn prefab_entity_records(store: &mut mapgeom::store::DataStore, prefab_path: &str, xf: &mapgeom::geom::Xform, class: &'static str, obj: u32, sub: &mut u32, quality: f32, out: &mut Vec<Rec>) -> Result<(), String> {
    prefab_entity_records_in(store, false, prefab_path, xf, class, obj, sub, quality, out)
}

/// THE GROUP KEY IS THE CLONE (RE 7, 17:55Z): the game instantiates one Solid2Model (one PreLightGen pointer) per
/// (prefab ENTITY PATH, block CLASS normal | free) — the same nested BarrierSupport reached through FCCenter_Air and
/// through an HFC piece of the same class shares the clone, the two placement paths (normal / free-mode blocks) clone
/// independently (stpad: 40 pointers = 11 entities × 2 classes + the L/R end clips 3 × 4 + 6 singles; BarrierSupport
/// 912 normal + 360 free; 735 model entries). `free` = the owner block's flag bit 28.
pub fn prefab_entity_records_in(store: &mut mapgeom::store::DataStore, free: bool, prefab_path: &str, xf: &mapgeom::geom::Xform, class: &'static str, obj: u32, sub: &mut u32, quality: f32, out: &mut Vec<Rec>) -> Result<(), String> {
    let pm = store.load_model(prefab_path)?;
    let pf = mapgeom::static_item::prefab::CPlugPrefab::from_model(&pm)?;
    for (ei, e) in pf.ents.iter().enumerate() {
        let e_xf = mapgeom::geom::compose(xf, &mapgeom::geom::from_quat(e.rot, e.pos));
        match e.model.inline.as_deref() {
            Some(mapgeom::static_item::Node::StaticObject(so)) => {
                let Some(s2) = so.solid2() else { continue };
                let Some(plg) = &s2.pre_light_gen else { continue };
                if plg.u01 == 0 { continue; }
                let mut boxes = Vec::new();
                for sg in &s2.shaded_geoms {
                    let Some(vr) = s2.visuals.get(sg.visual_index as usize) else { continue };
                    let Some(mapgeom::static_item::Node::Visual(vis)) = vr.inline.as_deref() else { continue };
                    if let Some(mm) = vis.main.as_ref() { let b = mm.bounding_box; boxes.push(crate::lmtiles::CBox::new([b[0], b[1], b[2]], [b[3], b[4], b[5]])); }
                }
                let Some(mb) = crate::lmtiles::model_box(&boxes) else { continue };
                let w = mb.transformed(&crate::lmtiles::from_xform(&e_xf));
                let mut h = std::collections::hash_map::DefaultHasher::new();
                use std::hash::{Hash, Hasher};
                (free, prefab_path, ei).hash(&mut h);
                let group = (h.finish() & 0x0000_FFFF_FFFF_0000) | quality.to_bits() as u64;
                // THE WALL TEST (RE 7's FUN_14028ff90): this exact model file, and the entity's world Iso4 third row (m2, m5, m8) axis-aligned
                let iso = crate::lmtiles::from_xform(&e_xf);
                let wall = if prefab_path == crate::itemrule::WALL_MODEL_FILE { crate::itemrule::wall_facing(iso[2], iso[5], iso[8]).map(|code| (code, 2.0 * w.h[1])) } else { None };
                out.push(Rec { class, obj, sub: *sub, meter_by_uv: plg.u02, uv: [plg.u04[0], plg.u04[1], plg.u04[2], plg.u04[3]], quality, centre: w.c, half: w.h, group, key_centre: None, pos_rank: None, wall });
                *sub += 1;
            }
            None if e.model.index >= 0 => {
                // an external reference: a nested prefab (recurse) or a static object file
                let Some((_, path)) = pm.externals.iter().find(|(i, _)| *i == e.model.index as u32) else { continue };
                if path.ends_with(".Prefab.Gbx") {
                    let path = path.clone();
                    prefab_entity_records_in(store, free, &path, &e_xf, class, obj, sub, quality, out)?;
                }
            }
            _ => {}
        }
    }
    Ok(())
}

/// The block at `cell` with `dir` (the placement transform of mapgeom::place::grid_block for a 32 × 32 footprint) —
/// the picked variant's mobils' prefabs, every entity a record.
pub fn block_records(store: &mut mapgeom::store::DataStore, bi: &mapgeom::blockinfo::BlockInfo, cell: [i32; 3], dir: u8, ground: bool, variant: usize, subvariant: usize, additional: usize, yoff: f32, class: &'static str, obj: u32, quality: f32, out: &mut Vec<Rec>) -> Result<usize, String> {
    block_records_class(store, bi, cell, dir, ground, variant, subvariant, additional, yoff, class, obj, quality, false, out)
}

/// `free` = the block class (normal | free-mode) the clones are keyed by.
pub fn block_records_class(store: &mut mapgeom::store::DataStore, bi: &mapgeom::blockinfo::BlockInfo, cell: [i32; 3], dir: u8, ground: bool, variant: usize, subvariant: usize, additional: usize, yoff: f32, class: &'static str, obj: u32, quality: f32, free: bool, out: &mut Vec<Rec>) -> Result<usize, String> {
    let Some(picked) = bi.pick_placement_add(ground, variant, subvariant, additional) else { return Ok(0) };
    let xf = mapgeom::place::grid_block((cell[0], cell[1], cell[2]), dir, (32.0, 32.0), yoff);
    let mut sub = 0u32;
    let n0 = out.len();
    for mb in &picked.mobils {
        let Some(pp) = &mb.prefab else { continue };
        // the mobil's own offset inside the block (translation / rotation), when any
        let mxf = match (mb.translation, mb.rotation) {
            (Some(t), r) => { let yaw = r.map(|v| v[1]).unwrap_or(0.0); mapgeom::geom::compose(&xf, &mapgeom::geom::yaw(yaw, t)) }
            _ => xf,
        };
        prefab_entity_records_in(store, free, pp, &mxf, class, obj, &mut sub, quality, out)?;
    }
    Ok(out.len() - n0)
}

// ─────────────────────────────────────────────────────────────────────────────────────────────────────
// THE MAP'S RECORD LIST (the closed rules of REPORT-5 §4-E.10, gathered for the bake's --layout-game)

/// The knobs of `build_map_records` — the defaults are the closed rules; the others are the study switches records-check keeps.
#[derive(Clone, Debug, Default)]
pub struct BuildOpts {
    pub collection: String,
    /// The ground zone (None = layout::ground_zone).
    pub zone: Option<String>,
    /// The kept item set (RE 7's reductions).
    pub kept: Option<std::collections::HashSet<usize>>,
    pub tile_level: Option<i32>,
    pub yoff: Option<f32>,
    pub grid: Option<usize>,
    /// Items mark in 3-D too (default: at the tiles' row only).
    pub items_3d: bool,
    /// Ghost blocks mark (default: no).
    pub ghost_marks: bool,
    pub no_block_cells: bool,
    /// The clip order: "sim" keeps mapgeom's simulate order (default: the cell-hash owner walk).
    pub clip_order_sim: bool,
    pub face_order: Option<Vec<usize>>,
    /// Clip kinds whose entities are one clone across the classes (the study).
    pub one_class: Vec<String>,
}

/// What `build_map_records` returns besides the records.
pub struct MapRecords {
    pub recs: Vec<Rec>,
    pub n_blocks: usize,
    pub n_tiles: usize,
    pub n_clips: usize,
    pub n_items: usize,
    /// The tile cells in record order.
    pub tile_cells: Vec<(i32, i32)>,
    pub tile_quality: Vec<f32>,
    /// obj ids: the blocks from `block_obj0`, the tiles from `tile_obj0`, the clips from `clip_obj0`, the items from `item_obj0`.
    pub block_obj0: u32,
    pub tile_obj0: u32,
    pub clip_obj0: u32,
    pub item_obj0: u32,
    pub notes: Vec<String>,
}

/// The map's records in the lightmapper's order: the authored blocks' prefab entities (file order), the zone tiles (the
/// baked records first, then x-major), the engine's clips (the cell-hash owner walk), the items — with the game's object ids
/// (a map with authored blocks numbers from 16384: stpad's blocks 16384.., tiles 16564.., clips 25780.., items 26808..;
/// a tiny map without one from 0: tiles 0.., items from the tile count).
pub fn build_map_records(map_path: &str, scene: &crate::geometry::Scene, store: &mut mapgeom::store::DataStore, opts: &BuildOpts) -> Result<MapRecords, String> {
    let mf = tmmaps::map::MapFile::load(std::path::Path::new(map_path));
    let coll = opts.collection.as_str();
    let prof = crate::layout::CollectionProfile::of(coll);
    let zone = opts.zone.clone().unwrap_or_else(|| crate::layout::ground_zone(&mf, coll));
    let grid: usize = opts.grid.unwrap_or(if coll == "Stadium" && (mf.size[0].max(0) * mf.size[2].max(0)) as i32 != prof.grid * prof.grid { prof.grid as usize } else { mf.size[0].max(1) as usize });
    let tile_y: i32 = opts.tile_level.unwrap_or_else(|| crate::layout::tile_level(&mf, coll));
    let yoff: f32 = opts.yoff.unwrap_or(prof.yoff);
    let is_zone_block = |b: &tmmaps::map::BlockRec| b.flags & 0x1000 != 0 && prof.flat_zones.contains(&b.name.as_str());
    let mut notes = Vec::new();
    // THE MARKS: the items at the tiles' row, the blocks in 3-D (ghosts and flat zone blocks mark nothing)
    let mut marks: std::collections::HashSet<(i32, i32, i32)> = mf.items.iter().map(|it| (it.file_cell[0] as i32, it.file_cell[1] as i32, it.file_cell[2] as i32)).filter(|c| opts.items_3d || c.1 == tile_y).collect();
    if !opts.no_block_cells {
        for b in &mf.blocks {
            if b.flags & 0x1000_0000 != 0 && !opts.ghost_marks { continue; }
            if is_zone_block(b) { continue; }
            let (x, y, z) = b.coords();
            marks.insert((x, y, z));
        }
    }
    // the tile cells: the baked records first, then x-major
    let baked: Vec<(i32, i32)> = mf.baked.iter().map(|b| { let (x, _, z) = b.coords(); (x, z) }).collect();
    let cells = crate::layout::tile_cells(&baked, grid as i32, grid as i32);
    let tq = crate::layout::tile_quality(&cells, tile_y, &marks);
    let has_authored = mf.blocks.iter().any(|b| !is_zone_block(b));
    let block_obj0: u32 = if has_authored { 16384 } else { 0 };
    let n_blocks_authored = mf.blocks.iter().filter(|b| !is_zone_block(b)).count() as u32;
    let tile_obj0 = block_obj0 + n_blocks_authored;
    let mut recs: Vec<Rec> = Vec::new();
    // 1. the authored blocks
    let mut idx = mapgeom::blockmap::BlockInfoIndex::build(store, coll);
    let mut n_block_recs = 0usize;
    let mut bobj = block_obj0;
    for b in mf.blocks.iter() {
        if is_zone_block(b) { continue; }
        let obj = bobj; bobj += 1;
        let Some(path) = idx.path_for(&b.name) else { notes.push(format!("block {}: no block info", b.name)); continue };
        let bi = match idx.load(store, &path) { Ok(bi) => bi.clone(), Err(e) => { notes.push(format!("block {}: {e}", b.name)); continue } };
        let (x, y, z) = b.coords();
        let ground = b.flags & mapgeom::blockmap::FLAG_GROUND != 0;
        let variant = (b.flags & mapgeom::blockmap::FLAG_VARIANT_MASK) as usize;
        let subvariant = ((b.flags >> mapgeom::blockmap::FLAG_SUBVARIANT_SHIFT) & 63) as usize;
        let additional = ((b.flags >> mapgeom::blockmap::FLAG_ADDITIONAL_SHIFT) & 127) as usize;
        n_block_recs += block_records_class(store, &bi, [x, y, z], b.dir, ground, variant, subvariant, additional, yoff, "block", obj, 1.0, b.flags & 0x1000_0000 != 0, &mut recs)?;
    }
    // 2. the tiles (the zone prefab at the tile row)
    let n_tiles_before = recs.len();
    {
        let cells_of: Vec<(i32, i32)> = cells.clone();
        let tiles_all = zone_tiles(store, coll, &zone, grid, tile_y as f32, yoff, &|cx, cz| tq[cx * grid + cz])?;
        // zone_tiles is x-major over the whole grid; take them in `cells` order
        let mut by_cell: std::collections::HashMap<(i32, i32), Rec> = tiles_all.into_iter().enumerate().map(|(i, r)| (((i / grid) as i32, (i % grid) as i32), r)).collect();
        for (k, c) in cells_of.iter().enumerate() {
            let Some(mut r) = by_cell.remove(c) else { return Err(format!("tile cell {c:?} missing")) };
            r.obj = tile_obj0 + k as u32;
            recs.push(r);
        }
    }
    let n_tiles = recs.len() - n_tiles_before;
    // 3. the engine's clips
    let clip_obj0: u32 = tile_obj0 + n_tiles as u32;
    let mut n_clip_recs = 0usize;
    let mut n_clip_objs = 0u32;
    if has_authored {
        let faces = mapgeom::fillers::faces(store, &mut idx, &mf);
        let dirs: std::collections::HashMap<usize, u8> = mf.blocks.iter().map(|b| (b.index, b.dir)).collect();
        let grounds = mapgeom::bake::record_grounds(&faces, &mf);
        let mut clips = mapgeom::bake::simulate(&faces, &dirs, &grounds);
        if !opts.clip_order_sim {
            let cells_b: Vec<[i32; 3]> = mf.blocks.iter().map(|b| { let (x, y, z) = b.coords(); [x, y, z] }).collect();
            let frees: Vec<bool> = mf.blocks.iter().map(|b| b.flags & 0x1000_0000 != 0).collect();
            let owner_order = crate::itemrule::clip_owner_order(&cells_b, &frees);
            let owner_rank: std::collections::HashMap<usize, usize> = owner_order.iter().enumerate().map(|(r, &bi)| (mf.blocks[bi].index, r)).collect();
            let face_order: Vec<usize> = opts.face_order.clone().unwrap_or_else(|| vec![0, 1, 2, 3, 4, 5]);
            let face_rank = |face: usize| face_order.iter().position(|&x| x == face).unwrap_or(9);
            let pos_in_list: Vec<usize> = clips.iter().map(|c| faces.occupants.get(&c.cell).and_then(|os| os.iter().find(|o| o.index == c.owner_index && o.unit == c.unit)).and_then(|o| o.faces[c.face].iter().position(|n| *n == c.name)).unwrap_or(0)).collect();
            let mut order: Vec<usize> = (0..clips.len()).collect();
            order.sort_by_key(|&i| (owner_rank.get(&clips[i].owner_index).copied().unwrap_or(usize::MAX), clips[i].unit, face_rank(clips[i].face), pos_in_list[i]));
            clips = order.iter().map(|&i| clips[i].clone()).collect();
        }
        for c in clips.iter().filter(|c| c.drawn()) {
            let Some(path) = idx.path_for(&c.name) else { notes.push(format!("clip {}: no block info", c.name)); continue };
            let bi = match idx.load(store, &path) { Ok(bi) => bi.clone(), Err(e) => { notes.push(format!("clip {}: {e}", c.name)); continue } };
            let owner_b = mf.blocks.iter().find(|b| b.index == c.owner_index);
            let owner_cell = match owner_b { Some(b) => { let (x, y, z) = b.coords(); [x, y, z] } None => [c.cell[0] as i32, c.cell[1] as i32, c.cell[2] as i32] };
            let st = mapgeom::bake::step(c.face);
            let (cell, d): ([i32; 3], u8) = if c.face < 4 {
                ([owner_cell[0] + st.0, owner_cell[1] + st.1, owner_cell[2] + st.2], mapgeom::bake::opposite(c.face) as u8)
            } else {
                ([owner_cell[0] + st.0, owner_cell[1] + st.1, owner_cell[2] + st.2], c.dir_word() as u8)
            };
            let class: &'static str = match c.face { 0 => "clipN", 1 => "clipE", 2 => "clipS", 3 => "clipW", 4 => "clipT", _ => "clipB" };
            // the horizontal clip's shape from the owner's neighbours at the piece's end (ghosts count)
            let mut variant = 0usize;
            if c.face < 4 && (c.name == "waterhfcleft" || c.name == "waterhfcright") {
                let (lx, lz) = if c.name == "waterhfcleft" { (-st.2, st.0) } else { (st.2, -st.0) };
                let occ = |dx: i32, dz: i32| mf.blocks.iter().any(|b| { let (x, y, z) = b.coords(); (x, y, z) == (owner_cell[0] + dx, owner_cell[1], owner_cell[2] + dz) });
                let shape = if !occ(lx, lz) { 1 } else if occ(st.0 + lx, st.2 + lz) { 2 } else { 3 };
                variant = if c.name == "waterhfcleft" { shape * 4 } else { shape };
            }
            let mut owner_free = owner_b.map(|b| b.flags & 0x1000_0000 != 0).unwrap_or(false);
            if opts.one_class.iter().any(|n| *n == c.name) { owner_free = false; }
            n_clip_recs += block_records_class(store, &bi, cell, d, c.ground, variant, 0, 0, yoff, class, clip_obj0 + n_clip_objs, 1.0, owner_free, &mut recs)?;
            n_clip_objs += 1;
        }
    }
    // 4. the items
    let item_obj0 = clip_obj0 + n_clip_objs;
    let irecs = crate::lmtiles::item_records(scene, 1.0, false);
    let mut n_items = 0usize;
    for inst in scene.instances.iter() {
        if let Some(k) = &opts.kept { if !k.contains(&inst.item) { continue; } }
        let m = &scene.models[inst.model];
        let Some(b) = m.plg_bounds else { continue };
        if !(b[2] > b[0] && b[3] > b[1]) { continue; }
        let no_lm: Vec<String> = std::env::var("LMTOOL_NO_LM_MATERIALS").map(|v| v.split(',').map(|t| t.to_string()).collect()).unwrap_or_else(|_| vec!["RaceTriggerFX".into(), "\\Decal".into()]);
        if !m.mat_links.is_empty() && m.mat_links.iter().all(|l| no_lm.iter().any(|n| l.contains(n.as_str()))) { continue; }
        let Some(ir) = irecs.iter().find(|r| r.item == inst.item) else { continue };
        let Some(rec) = &ir.record else { continue };
        let q = crate::layout::item_quality(inst.lm_quality);
        recs.push(Rec { class: "item", obj: item_obj0 + n_items as u32, sub: 0, meter_by_uv: m.plg_u02, uv: b, quality: q, centre: rec.world.c, half: rec.world.h, group: ((inst.model as u64) << 32) | q.to_bits() as u64, key_centre: None, pos_rank: None, wall: None });
        n_items += 1;
    }
    let _ = n_block_recs;
    Ok(MapRecords { recs, n_blocks: n_blocks_authored as usize, n_tiles, n_clips: n_clip_recs, n_items, tile_cells: cells, tile_quality: tq, block_obj0, tile_obj0, clip_obj0, item_obj0, notes })
}
