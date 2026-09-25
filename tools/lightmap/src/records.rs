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
            out.push(Rec { class: "tile", obj: 0, sub: 0, meter_by_uv: mbu, uv, quality: q, centre: w.c, half: w.h, group: 0xF000_0000_0000_0000 | q.to_bits() as u64 });
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
    let items: Vec<(u32, [f32; 2], crate::layout::ChartKey, crate::layout::Charted)> = recs.iter().enumerate().map(|(i, r)| (i as u32, r.ext(), crate::layout::ChartKey { centre: r.centre, h2: (r.half[0] * r.half[0] + r.half[1] * r.half[1]) + r.half[2] * r.half[2] }, crate::layout::Charted::Bound)).collect();
    let groups: Vec<u64> = recs.iter().map(|r| r.group).collect();
    crate::layout::allocate_grouped(&crate::layout::LayoutInput { tiles: Vec::new(), items, w_atlas: 2048, quality_index }, &groups)
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
    prefab_entity_records_in(store, prefab_path, prefab_path, xf, class, obj, sub, quality, out)
}

/// `root` = the mobil's own prefab: THE GROUP KEY IS THE CLONE — the game instantiates one Solid2Model (one PreLightGen
/// pointer) per (mobil prefab, entity path), so the same nested BarrierSupport reached through FCCenter_Air and through an
/// HFC piece is two groups (RE 7's 40 model pointers on stpad: 912 + 360).
fn prefab_entity_records_in(store: &mut mapgeom::store::DataStore, root: &str, prefab_path: &str, xf: &mapgeom::geom::Xform, class: &'static str, obj: u32, sub: &mut u32, quality: f32, out: &mut Vec<Rec>) -> Result<(), String> {
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
                (root, prefab_path, ei).hash(&mut h);
                let group = (h.finish() & 0x0000_FFFF_FFFF_0000) | quality.to_bits() as u64;
                out.push(Rec { class, obj, sub: *sub, meter_by_uv: plg.u02, uv: [plg.u04[0], plg.u04[1], plg.u04[2], plg.u04[3]], quality, centre: w.c, half: w.h, group });
                *sub += 1;
            }
            None if e.model.index >= 0 => {
                // an external reference: a nested prefab (recurse) or a static object file
                let Some((_, path)) = pm.externals.iter().find(|(i, _)| *i == e.model.index as u32) else { continue };
                if path.ends_with(".Prefab.Gbx") {
                    let path = path.clone();
                    prefab_entity_records_in(store, root, &path, &e_xf, class, obj, sub, quality, out)?;
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
        prefab_entity_records(store, pp, &mxf, class, obj, &mut sub, quality, out)?;
    }
    Ok(out.len() - n0)
}
