//! `mapgeom flags` — Hugo's "every flag is a finish" prototype (2026-10-07, the
//! Manslaughter mechanism): a custom FINISH item wearing the flag's own look and a
//! bounding-box trigger replaces every flag placement 1:1; the map's real finish
//! pieces go (blocks → their plain twins, items removed). The item is baked by
//! `static_item` (`pack_item_merged` + `make_waypoint_bbox`), embedded in the
//! map's 0x03043054 zip under the map's collection, and every flag record is
//! renamed to it in place (so its lightmap chart stays bound) with a `Goal` tag.

use crate::sttc::{pos_str, Ctx, Row, WP_FINISH};
use std::collections::{BTreeMap, HashSet};
use std::path::Path;
use tmmaps::map::MapFile;

#[derive(Clone, Debug)]
pub struct FlagsOpts {
    /// the flag models to convert (pack item stems, e.g. `Flag16m`); empty = every item whose model contains "Flag"
    pub models: Vec<String>,
    pub trigger_pad: f32,
    pub uid_prefix: String,
    pub name_suffix: String,
    pub unlock: bool,
    pub unvalidated: bool,
    pub author: String,
    /// keep the source lightmap, its chart table renumbered (the flags keep their charts)
    pub keep_lightmap: bool,
    pub pak_specs: Vec<String>,
}

pub struct FlagsOutcome {
    pub rows: Vec<Row>,
    pub converted: usize,
    pub finishes_removed: usize,
    pub finish_blocks_replaced: usize,
    pub items: Vec<(String, String, [f32; 6])>, // (flag model, finish ident, trigger box)
    pub new_name: String,
    pub new_uid: String,
}

pub fn flags(store: &mut crate::store::DataStore, src: &Path, out: &Path, o: &FlagsOpts, dry: bool) -> Result<FlagsOutcome, String> {
    let map_label = src.file_name().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
    let m = MapFile::try_load(src)?;
    let hdr = tmmaps::header::read(src.to_str().unwrap_or_default())?;
    let collection = m.body_collections().map(|c| c[0].1).unwrap_or(26);
    let mut rows: Vec<Row> = Vec::new();
    // ---- the flag models present
    let mut models: Vec<String> = Vec::new();
    for it in &m.items {
        let stem = it.model.trim_end_matches(".Item.Gbx").to_string();
        let wanted = if o.models.is_empty() { stem.contains("Flag") } else { o.models.iter().any(|w| w.eq_ignore_ascii_case(&stem)) };
        if wanted && !models.contains(&stem) {
            models.push(stem);
        }
    }
    if models.is_empty() {
        return Err(format!("{map_label}: no flag items"));
    }
    // ---- the finish pieces (classified from the packs)
    let mut ctx = Ctx::new(store, &m);
    let mut finish_items: HashSet<usize> = HashSet::new();
    for it in &m.items {
        if ctx.item_class(it).effective() == Some(WP_FINISH) {
            finish_items.insert(it.index);
            let mut r = Row::new(&map_label, "flags", "item", it.index, &it.model);
            r.tag = it.waypoint_tag.clone().unwrap_or_default();
            r.action = "removed".into();
            r.from = pos_str(it.pos);
            r.note = "the map's finish item".into();
            rows.push(r);
        }
    }
    let finish_blocks: Vec<usize> = m.blocks.iter().filter(|b| b.flags != 0xFFFF_FFFF && ctx.block_class(b).effective() == Some(WP_FINISH)).map(|b| b.index).collect();
    // ---- the items: one per flag model
    let mut built: Vec<(String, String, [f32; 6], Vec<u8>)> = Vec::new();
    for stem in &models {
        let pack_path = format!("Stadium\\Items\\{stem}.Item.Gbx");
        let ident = format!("{stem}Finish.Item.Gbx");
        let mut mg = crate::static_item::build::pack_item_merged(ctx.store, &pack_path, 1.0, collection, 0, None).map_err(|e| format!("{stem}: {e}"))?;
        let pts: Vec<[f32; 3]> = {
            let loaded = ctx.store.load_model(&pack_path)?;
            let mut c = crate::geom::Collector::new(ctx.store);
            c.model(&loaded, &crate::geom::IDENTITY, 0);
            c.scene.groups.values().flat_map(|g| g.verts.iter().copied()).collect()
        };
        let bb = crate::static_item::build::make_waypoint_bbox(&mut mg, WP_FINISH, o.trigger_pad, &pts).map_err(|e| format!("{stem}: {e}"))?;
        let (bytes, mg) = crate::static_item::build::finish_item(mg, &ident, &o.author, 1.0, collection).map_err(|e| format!("{stem}: {e}"))?;
        let mut r = Row::new(&map_label, "flags", "model", 0, stem);
        r.action = "finish-item-built".into();
        r.to_name = ident.clone();
        r.note = format!("{} bytes; trigger bbox {:.2}..{:.2} × {:.2}..{:.2} × {:.2}..{:.2} (pad {}); {} visuals; notes: {}", bytes.len(), bb[0], bb[3], bb[1], bb[4], bb[2], bb[5], o.trigger_pad, mg.visuals.len(), mg.notes.iter().filter(|n| n.contains("waypoint") || n.contains("frame 0") || n.contains("skin")).cloned().collect::<Vec<_>>().join(" | "));
        rows.push(r);
        built.push((stem.clone(), ident, bb, bytes));
    }
    // ---- the placements
    let mut converted = 0usize;
    for it in &m.items {
        let stem = it.model.trim_end_matches(".Item.Gbx");
        if let Some((_, ident, _, _)) = built.iter().find(|(s, ..)| s == stem) {
            let mut r = Row::new(&map_label, "flags", "item", it.index, &it.model);
            r.action = "to-finish".into();
            r.to_name = ident.trim_end_matches(".Item.Gbx").to_string();
            r.from = format!("{} yaw {:.4}", pos_str(it.pos), it.yaw);
            r.to = r.from.clone();
            r.note = "model renamed in place (pose, scale, colour kept); tag Goal".into();
            rows.push(r);
            converted += 1;
        }
    }
    let new_name = format!("{}{}", hdr.name, o.name_suffix);
    let old_uid = hdr.uid.clone();
    let new_uid = if old_uid.len() > o.uid_prefix.len() { format!("{}{}", o.uid_prefix, &old_uid[..old_uid.len() - o.uid_prefix.len()]) } else { old_uid.clone() };
    {
        let mut r = Row::new(&map_label, "flags", "map", 0, &new_name);
        r.action = "identity".into();
        r.to_name = new_uid.clone();
        r.note = format!("{converted} flags -> finishes ({} model(s)); {} finish items removed, {} finish blocks -> plain twins; collection {collection}", built.len(), finish_items.len(), finish_blocks.len());
        rows.push(r);
    }
    let outcome = |rows: Vec<Row>| FlagsOutcome { rows, converted, finishes_removed: finish_items.len(), finish_blocks_replaced: finish_blocks.len(), items: built.iter().map(|(s, i, b, _)| (s.clone(), i.clone(), *b)).collect(), new_name: new_name.clone(), new_uid: new_uid.clone() };
    if dry {
        return Ok(outcome(rows));
    }
    if let Some(dir) = out.parent() {
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    // pass 0: the finish BLOCKS → plain twins (sttc's strip pass on the finish type only)
    let mut cur = src.to_path_buf();
    if !finish_blocks.is_empty() {
        let tmp = out.with_extension("nofinish.tmp.Map.Gbx");
        let s = crate::sttc::strip_waypoints(&mut ctx, src, &tmp, crate::sttc::CpMode::Plain, false, &[WP_FINISH])?;
        // the strip pass also removed the finish ITEMS; nothing left for pass 1
        rows.extend(s.rows.iter().cloned());
        cur = tmp;
    }
    // pass 1: the finish items go (when the strip pass did not run)
    let m1 = MapFile::try_load(&cur)?;
    let still: HashSet<usize> = m1.items.iter().filter(|it| finish_items.contains(&it.index) && m1.items.len() == m.items.len()).map(|it| it.index).collect();
    {
        let mut mm = MapFile::try_load(&cur)?;
        if !still.is_empty() {
            mm.remove_items(|it| still.contains(&it.index));
        }
        mm.strip_validation_ghost_to(tmmaps::map::GhostForm::Remove);
        if o.unlock {
            mm.remove_password();
        }
        mm.write_to(out).map_err(|e| e.to_string())?;
    }
    // pass 2: embed the item(s) (a splice of 0x03043054)
    {
        let mut mm = MapFile::try_load(out)?;
        let mut files: BTreeMap<String, Vec<u8>> = BTreeMap::new();
        let mut manifest: Vec<(String, String)> = Vec::new();
        for (_, ident, _, bytes) in &built {
            let b = crate::tiny_assets::set_ident_collection(bytes, collection);
            files.insert(ident.clone(), b);
            manifest.push((ident.clone(), o.author.clone()));
        }
        let zip = tmmaps::header::stored_zip(&files);
        let refs: Vec<(&str, &str)> = manifest.iter().map(|(a, b)| (a.as_str(), b.as_str())).collect();
        mm.replace_embedded_objects(&refs, &zip);
        mm.write_to(out).map_err(|e| e.to_string())?;
    }
    // pass 3: the flag placements renamed to the finish item (Id-table renames) + author
    {
        let mut mm = MapFile::try_load(out)?;
        for (i, it) in mm.items.clone().iter().enumerate() {
            let stem = it.model.trim_end_matches(".Item.Gbx");
            if let Some((_, ident, _, _)) = built.iter().find(|(s, ..)| s == stem) {
                mm.set_item_model(i, ident.trim_end_matches(".Item.Gbx"));
                mm.set_item_author(i, &o.author);
            }
        }
        mm.write_to(out).map_err(|e| e.to_string())?;
    }
    // pass 4 (splices): the Goal tags, the name, the times
    {
        let mut mm = MapFile::try_load(out)?;
        let stems: Vec<String> = built.iter().map(|(_, i, _, _)| i.trim_end_matches(".Item.Gbx").to_string()).collect();
        for (i, it) in mm.items.clone().iter().enumerate() {
            if stems.iter().any(|s| *s == it.model) {
                mm.set_item_waypoint(i, Some("Goal"), 0);
            }
        }
        let (h, b) = mm.set_map_name(&hdr.name, &new_name);
        if h + b == 0 {
            return Err(format!("{map_label}: the map does not declare the name {:?}", hdr.name));
        }
        if o.unvalidated {
            mm.set_unvalidated(&hdr);
        }
        mm.write_to(out).map_err(|e| e.to_string())?;
    }
    // pass 5: the uid
    if new_uid != old_uid {
        let mut mm = MapFile::try_load(out)?;
        mm.set_map_uid_any_len(&new_uid);
        mm.write_to(out).map_err(|e| e.to_string())?;
    }
    if cur != src {
        let _ = std::fs::remove_file(&cur);
    }
    // pass 6: the lightmap chart table renumbered positionally (the removed finish items shift
    // every later item by their count; the flags KEEP their slots — same object, same look —
    // so their charts stay; the finish items' charts drop). The sttc object map vocabulary:
    // "sttf"-step rows, "removed" items = the finish items; everything else kept.
    if o.keep_lightmap {
        let mut srows: Vec<Row> = Vec::new();
        for i in &finish_items {
            let mut r = Row::new(&map_label, "sttf", "item", *i, "finish");
            r.action = "removed".into();
            srows.push(r);
        }
        for i in &finish_blocks {
            let mut r = Row::new(&map_label, "sttf", "block", *i, "finish");
            r.action = "replaced".into();
            srows.push(r);
        }
        let objmap = crate::sttc::objmap_rows(&m, &srows, &[], &[], &[]);
        let pth = out.with_extension("objmap.tsv");
        std::fs::write(&pth, objmap).map_err(|e| e.to_string())?;
        let lmtool = std::env::current_exe().ok().and_then(|e| e.parent().map(|d| d.join("lmtool"))).filter(|p| p.exists()).map(|p| p.display().to_string()).unwrap_or_else(|| "lmtool".to_string());
        let lit = out.with_extension("lit.tmp.Map.Gbx");
        let mut cmd = std::process::Command::new(&lmtool);
        cmd.arg("sttc-relight");
        for spec in &o.pak_specs {
            cmd.arg("--pak").arg(spec);
        }
        let outp = cmd.arg("--source").arg(src).arg("--map").arg(out).arg("--objmap").arg(&pth).arg("--out").arg(&lit).output().map_err(|e| format!("{lmtool}: {e}"))?;
        let text = format!("{}{}", String::from_utf8_lossy(&outp.stdout), String::from_utf8_lossy(&outp.stderr));
        if !outp.status.success() {
            return Err(format!("{map_label}: lmtool sttc-relight failed: {}", text.trim()));
        }
        std::fs::rename(&lit, out).map_err(|e| e.to_string())?;
        let mut rr = Row::new(&map_label, "lightmap", "map", 0, "chart table renumbered");
        rr.action = text.lines().next().unwrap_or("").trim().to_string();
        rows.push(rr);
    }
    Ok(outcome(rows))
}

pub fn verify_flags(src: &Path, out: &Path, oc: &FlagsOutcome, o: &FlagsOpts) -> Result<Vec<String>, String> {
    let a = MapFile::try_load(src)?;
    let b = MapFile::try_load(out)?;
    let mut bad = Vec::new();
    if a.items.len() - oc.finishes_removed != b.items.len() {
        bad.push(format!("items {} - {} removed != {}", a.items.len(), oc.finishes_removed, b.items.len()));
    }
    let goals = b.items.iter().filter(|x| x.waypoint_tag.as_deref() == Some("Goal")).count() + b.blocks.iter().filter(|x| x.waypoint_tag.as_deref() == Some("Goal")).count();
    if goals != oc.converted {
        bad.push(format!("{goals} Goal placements, want {} (the flags)", oc.converted));
    }
    let idents: Vec<String> = oc.items.iter().map(|(_, i, _)| i.trim_end_matches(".Item.Gbx").to_string()).collect();
    let n = b.items.iter().filter(|x| idents.contains(&x.model)).count();
    if n != oc.converted {
        bad.push(format!("{n} placements wear the finish item, want {}", oc.converted));
    }
    for x in b.items.iter().filter(|x| idents.contains(&x.model)) {
        if x.waypoint_tag.as_deref() != Some("Goal") {
            bad.push(format!("item#{} {} has tag {:?}", x.index, x.model, x.waypoint_tag));
            break;
        }
        if x.author.as_deref() != Some(o.author.as_str()) {
            bad.push(format!("item#{} author {:?} != {}", x.index, x.author, o.author));
            break;
        }
    }
    let emb = crate::embedded::items(&b)?;
    for (_, ident, _) in &oc.items {
        if !emb.keys().any(|k| k.to_lowercase().ends_with(&ident.to_lowercase())) {
            bad.push(format!("{ident} not in the embedded zip ({} files)", emb.len()));
        }
    }
    let hb = tmmaps::header::read(out.to_str().unwrap_or_default())?;
    if hb.validated != "0" || hb.name != oc.new_name || hb.uid != oc.new_uid {
        bad.push(format!("header validated={} name={:?} uid={}", hb.validated, hb.name, hb.uid));
    }
    Ok(bad)
}
