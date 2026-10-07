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
    /// round 2 (Hugo 04:01 PT): the flags stay untouched; an INVISIBLE `PoleFinishTrigger`
    /// item (cylinder trigger about the pole) is ADDED at every flag's pose
    pub pole_triggers: bool,
    /// the cylinder: radius margin over the pole radius (m), sides
    pub pole_margin: f32,
    pub pole_sides: usize,
    /// also strip the CHECKPOINTS (the sttf rule: blocks → plain twins by geometry, rings/arches + items removed)
    pub sttf: bool,
    /// a map with NO flag item is built anyway — zero carriers, no finish left (Hugo 05:15 PT: "I want the
    /// complete campaign locally"); the refusal stays the default
    pub allow_no_finish: bool,
    /// round 3 (Hugo 05:35 PT): the trigger is the CLOTH's resting rectangle (a sheared slab from the
    /// time-averaged animation), not the pole; `cloth_pad` grows the rectangle's edges, `cloth_thick_pad`
    /// its sway band
    pub cloth: bool,
    pub cloth_pad: f32,
    pub cloth_thick_pad: f32,
}

/// The CLOTH of a flag model, measured on its vertex animation (the dyna mesh's 86 frames, placed
/// by the prefab entity's pose): the time-averaged cloth's attachment edge at the pole (y range),
/// its reach along the local +z axis, its sway band in x, and the least-squares tilt of its
/// bottom edge over z — Hugo's "slightly tilted down rectangle" (round 3, 2026-10-07 05:35 PT).
pub struct Cloth {
    /// the cloth starts at this z (the pole surface)
    pub z0: f32,
    /// attachment edge y range at the pole
    pub y_lo: f32,
    pub y_hi: f32,
    /// horizontal reach along +z of the time-averaged cloth
    pub reach: f32,
    /// the time-averaged cloth's x band (the sway axis)
    pub x_lo: f32,
    pub x_hi: f32,
    /// the full animation's x band
    pub x_lo_anim: f32,
    pub x_hi_anim: f32,
    /// bottom-edge downward tilt (degrees) and the top edge's (the free corner droops more)
    pub tilt_bottom_deg: f32,
    pub tilt_top_deg: f32,
    pub frames: usize,
}

pub fn cloth_of(ctx: &mut Ctx, pack_path: &str) -> Result<Cloth, String> {
    // the item → its prefab → the dyna entity (pos + quaternion)
    let item = ctx.store.load_model(pack_path)?;
    let prefab_path = item.externals.iter().map(|(_, p)| p.clone()).find(|p| p.to_lowercase().ends_with(".prefab.gbx")).ok_or_else(|| format!("{pack_path}: no prefab external"))?;
    let pm = ctx.store.load_model(&prefab_path)?;
    let prefab = crate::static_item::prefab::CPlugPrefab::from_model(&pm)?;
    let mut dyna: Option<(String, [f32; 3], [f32; 4])> = None;
    for e in &prefab.ents {
        if let Some((_, path)) = pm.externals.iter().find(|(k, _)| *k as i32 == e.model.index) {
            if path.to_lowercase().ends_with(".dynaobject.gbx") {
                dyna = Some((path.clone(), e.pos, e.rot));
                break;
            }
        }
    }
    let (dpath, pos, q) = dyna.ok_or_else(|| format!("{prefab_path}: no dyna-object entity (the cloth)"))?;
    let mut scratch = crate::static_item::build::Merged::default();
    let src = crate::static_item::build::load_dyna_source(ctx.store, &dpath, &mut scratch, true)?;
    let (x, y, z, w) = (q[0], q[1], q[2], q[3]);
    let rot = |v: [f32; 3]| -> [f32; 3] {
        let (vx, vy, vz) = (v[0], v[1], v[2]);
        let (cx, cy, cz) = (y * vz - z * vy, z * vx - x * vz, x * vy - y * vx);
        let (dx, dy, dz) = (y * cz - z * cy, z * cx - x * cz, x * cy - y * cx);
        [vx + 2.0 * (w * cx + dx) + pos[0], vy + 2.0 * (w * cy + dy) + pos[1], vz + 2.0 * (w * cz + dz) + pos[2]]
    };
    // the finest visual (most vertices)
    let mut best: Option<(usize, Vec<[f32; 3]>, usize)> = None;
    for vr in &src.s2.visuals {
        let Some(crate::static_item::Node::Visual(v)) = vr.inline.as_deref() else { continue };
        let Some(main) = v.main.as_ref() else { continue };
        let Some(crate::static_item::Node::VertexStream(st)) = main.vertex_streams.first().and_then(|r| r.inline.as_deref()) else { continue };
        let Some(crate::static_item::vstream::Elem::Float3(pts)) = st.elems.first() else { continue };
        let nf = v.sub_visuals.len().max(1);
        if best.as_ref().map(|b| pts.len() > b.1.len()).unwrap_or(true) {
            best = Some((nf, pts.clone(), pts.len() / nf));
        }
    }
    let (nf, pts, per) = best.ok_or_else(|| format!("{dpath}: no vertex-animated visual"))?;
    let mut mean: Vec<[f32; 3]> = vec![[0.0; 3]; per];
    let (mut alo, mut ahi) = ([f32::INFINITY; 3], [f32::NEG_INFINITY; 3]);
    for f in 0..nf {
        for (k, pnt) in pts[f * per..(f + 1) * per].iter().enumerate() {
            let wv = rot(*pnt);
            for c in 0..3 {
                mean[k][c] += wv[c] / nf as f32;
                alo[c] = alo[c].min(wv[c]);
                ahi[c] = ahi[c].max(wv[c]);
            }
        }
    }
    let (mut lo, mut hi) = ([f32::INFINITY; 3], [f32::NEG_INFINITY; 3]);
    for wv in &mean {
        for c in 0..3 {
            lo[c] = lo[c].min(wv[c]);
            hi[c] = hi[c].max(wv[c]);
        }
    }
    // per-z bins (0.5 m) of the time-averaged cloth: min/max y
    let mut bins: std::collections::BTreeMap<i32, (f32, f32)> = std::collections::BTreeMap::new();
    for wv in &mean {
        let e = bins.entry((wv[2] * 2.0).round() as i32).or_insert((f32::INFINITY, f32::NEG_INFINITY));
        e.0 = e.0.min(wv[1]);
        e.1 = e.1.max(wv[1]);
    }
    let fit = |sel: &dyn Fn(&(f32, f32)) -> f32| -> f32 {
        let n = bins.len() as f32;
        let (mut sx, mut sy, mut sxx, mut sxy) = (0.0f32, 0.0f32, 0.0f32, 0.0f32);
        for (b, e) in &bins {
            let xx = *b as f32 / 2.0;
            let yy = sel(e);
            sx += xx; sy += yy; sxx += xx * xx; sxy += xx * yy;
        }
        (n * sxy - sx * sy) / (n * sxx - sx * sx)
    };
    let (first_lo, first_hi) = bins.values().next().copied().unwrap_or((lo[1], hi[1]));
    Ok(Cloth { z0: lo[2], y_lo: first_lo, y_hi: first_hi, reach: hi[2] - lo[2], x_lo: lo[0], x_hi: hi[0], x_lo_anim: alo[0], x_hi_anim: ahi[0], tilt_bottom_deg: (-fit(&|e| e.0)).atan().to_degrees(), tilt_top_deg: (-fit(&|e| e.1)).atan().to_degrees(), frames: nf })
}

/// The pole of a flag model, in the item frame: (axis x, axis z, radius, base y, top y) — the
/// collision mesh IS the pole (a 120-vertex tube). None when the collision is not a single tube.
pub fn pole_of(ctx: &mut Ctx, pack_path: &str, collection: u32) -> Result<(f32, f32, f32, f32, f32), String> {
    let mg = crate::static_item::build::pack_item_merged(ctx.store, pack_path, 1.0, collection, 0, None)?;
    let pts = &mg.surf_vertices;
    if pts.len() < 6 {
        return Err(format!("{pack_path}: collision has {} vertices, not a pole tube", pts.len()));
    }
    let (mut y0, mut y1) = (f32::INFINITY, f32::NEG_INFINITY);
    let (mut sx, mut sz) = (0.0f64, 0.0f64);
    for p in pts {
        y0 = y0.min(p[1]);
        y1 = y1.max(p[1]);
        sx += p[0] as f64;
        sz += p[2] as f64;
    }
    let cx = (sx / pts.len() as f64) as f32;
    let cz = (sz / pts.len() as f64) as f32;
    let r = pts.iter().map(|p| ((p[0] - cx).powi(2) + (p[2] - cz).powi(2)).sqrt()).fold(0.0f32, f32::max);
    if r > 1.0 {
        return Err(format!("{pack_path}: collision radius {r:.2} m about ({cx:.2},{cz:.2}) — not a pole"));
    }
    Ok((cx, cz, r, y0, y1))
}

pub struct FlagsOutcome {
    pub rows: Vec<Row>,
    pub converted: usize,
    /// items removed by the strip pass (finish items; with sttf the checkpoint items too)
    pub finishes_removed: usize,
    pub finish_blocks_replaced: usize,
    /// the strip pass: blocks swapped for plain twins / removed (rings, arches) / generated records removed
    pub replaced_blocks: usize,
    pub removed_blocks: usize,
    pub removed_baked: usize,
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
    if models.is_empty() && !o.allow_no_finish {
        return Err(format!("{map_label}: NO FLAG ITEMS — a Flags map of it would have no finish; not built (--allow-no-finish overrides)"));
    }
    let no_finish = models.is_empty();
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
    // THE CACHE FILETIME RULE (lmtool filetimecheck, 2026-09-28): the game keeps a map's lightmap in play only
    // when the cache chunk 0x06022013's FILETIME word equals the MAX CPlugSolid2Model.FileWriteTime over the
    // map's EMBEDDED items. 01 embeds nothing, so its word is the editor's own; our carrier item would bring a
    // different (pack) write time and get the lightmap dropped → the carrier's solid is stamped with the word.
    let lm_word: Option<u64> = if o.keep_lightmap { lightmap_cache_word(src) } else { None };
    // ---- the items: one per flag model
    let mut built: Vec<(String, String, [f32; 6], Vec<u8>)> = Vec::new();
    for stem in &models {
        let pack_path = format!("Stadium\\Items\\{stem}.Item.Gbx");
        if o.pole_triggers {
            // round 2: an invisible carrier with a cylinder trigger about the pole
            let ident = if o.cloth {
                if models.len() == 1 { "ClothFinishTrigger.Item.Gbx".to_string() } else { format!("ClothFinishTrigger{stem}.Item.Gbx") }
            } else if models.len() == 1 { "PoleFinishTrigger.Item.Gbx".to_string() } else { format!("PoleFinishTrigger{stem}.Item.Gbx") };
            let (cx, cz, pr, y0, y1) = pole_of(&mut ctx, &pack_path, collection).map_err(|e| format!("{stem}: {e}"))?;
            let r = pr + o.pole_margin;
            let mut mg = crate::static_item::build::pack_item_merged(ctx.store, &pack_path, 1.0, collection, 0, None).map_err(|e| format!("{stem}: {e}"))?;
            let cloth_note = if o.cloth {
                // THE CLOTH SLAB: a parallelogram prism in the item frame — attached along the pole at
                // z0 with the cloth's height, reaching `reach` along +z, both edges sheared down by the
                // cloth's bottom-edge tilt (the free top corner droops more in the animation; a flag
                // held still by the wind is a parallelogram), `cloth_pad` on the height/length edges,
                // the thickness = the time-averaged sway band ± `cloth_thick_pad`; the pole is NOT in it
                let c = cloth_of(&mut ctx, &pack_path).map_err(|e| format!("{stem}: {e}"))?;
                let t = c.tilt_bottom_deg.to_radians().tan();
                let (z_near, z_far) = (c.z0 + pr, c.z0 + c.reach + o.cloth_pad);
                let drop = t * (z_far - z_near);
                let (xa, xb) = (c.x_lo - o.cloth_thick_pad, c.x_hi + o.cloth_thick_pad);
                let (ya, yb) = (c.y_lo - o.cloth_pad, c.y_hi + o.cloth_pad);
                let corners: [[f32; 3]; 8] = [[xa, ya, z_near], [xb, ya, z_near], [xb, yb, z_near], [xa, yb, z_near], [xa, ya - drop, z_far], [xb, ya - drop, z_far], [xb, yb - drop, z_far], [xa, yb - drop, z_far]];
                crate::static_item::build::make_invisible_hexahedron_waypoint(&mut mg, WP_FINISH, corners).map_err(|e| format!("{stem}: {e}"))?;
                format!("CLOTH slab (local +z = the cloth direction; {} animation frames averaged): attached at z {z_near:.3} along the pole, y {ya:.2}..{yb:.2} ({:.2} m tall incl. pad {}), reaching z {z_far:.2} ({:.2} m), both edges sheared down {:.1}° (the cloth's bottom-edge tilt; its top edge droops {:.1}° at the free corner; the far edge sits {drop:.2} m lower), thickness x {xa:.2}..{xb:.2} (the time-averaged sway band {:.2}..{:.2} ± {}; the full swing spans x {:.2}..{:.2})", c.frames, yb - ya, o.cloth_pad, z_far - z_near, c.tilt_bottom_deg, c.tilt_top_deg, c.x_lo, c.x_hi, o.cloth_thick_pad, c.x_lo_anim, c.x_hi_anim)
            } else {
                crate::static_item::build::make_invisible_cylinder_waypoint(&mut mg, WP_FINISH, cx, cz, r, y0, y1, o.pole_sides).map_err(|e| format!("{stem}: {e}"))?;
                format!("trigger cylinder r {r:.3} ({} sides), same y span", o.pole_sides)
            };
            if let Some(w) = lm_word {
                mg.file_write_time = w;
                mg.notes.push(format!("solid FileWriteTime stamped with the map's lightmap cache word {w} (the cache FILETIME rule)"));
            }
            let (bytes, mg) = crate::static_item::build::finish_item(mg, &ident, &ident, 1.0, collection).map_err(|e| format!("{stem}: {e}"))?;
            let mut row = Row::new(&map_label, "flags", "model", 0, stem);
            row.action = if o.cloth { "cloth-trigger-item-built".into() } else { "pole-trigger-item-built".into() };
            row.to_name = ident.clone();
            row.note = format!("{} bytes; pole axis ({cx:.3},{cz:.3}) radius {pr:.3} m, y {y0:.3}..{y1:.3}; {cloth_note}; invisible (one sub-mm visual 4 m under the origin), no collision", bytes.len());
            rows.push(row);
            built.push((stem.clone(), ident, [cx - r, y0, cz - r, cx + r, y1, cz + r], bytes));
            continue;
        }
        let ident = format!("{stem}Finish.Item.Gbx");
        let mut mg = crate::static_item::build::pack_item_merged(ctx.store, &pack_path, 1.0, collection, 0, None).map_err(|e| format!("{stem}: {e}"))?;
        let pts: Vec<[f32; 3]> = {
            let loaded = ctx.store.load_model(&pack_path)?;
            let mut c = crate::geom::Collector::new(ctx.store);
            c.model(&loaded, &crate::geom::IDENTITY, 0);
            c.scene.groups.values().flat_map(|g| g.verts.iter().copied()).collect()
        };
        let bb = crate::static_item::build::make_waypoint_bbox(&mut mg, WP_FINISH, o.trigger_pad, &pts).map_err(|e| format!("{stem}: {e}"))?;
        if let Some(w) = lm_word {
            mg.file_write_time = w;
        }
        let (bytes, mg) = crate::static_item::build::finish_item(mg, &ident, &ident, 1.0, collection).map_err(|e| format!("{stem}: {e}"))?;
        let mut r = Row::new(&map_label, "flags", "model", 0, stem);
        r.action = "finish-item-built".into();
        r.to_name = ident.clone();
        r.note = format!("{} bytes; trigger bbox {:.2}..{:.2} × {:.2}..{:.2} × {:.2}..{:.2} (pad {}); {} visuals; notes: {}", bytes.len(), bb[0], bb[3], bb[1], bb[4], bb[2], bb[5], o.trigger_pad, mg.visuals.len(), mg.notes.iter().filter(|n| n.contains("waypoint") || n.contains("frame 0") || n.contains("skin")).cloned().collect::<Vec<_>>().join(" | "));
        rows.push(r);
        built.push((stem.clone(), ident, bb, bytes));
    }
    // ---- the placements
    let mut converted = 0usize;
    // (flag item index, trigger ident stem, pose) for the appended triggers
    let mut appended: Vec<(usize, String, [f32; 3], [f32; 3])> = Vec::new();
    for it in &m.items {
        let stem = it.model.trim_end_matches(".Item.Gbx");
        if let Some((_, ident, _, _)) = built.iter().find(|(s, ..)| s == stem) {
            let mut r = Row::new(&map_label, "flags", "item", it.index, &it.model);
            if o.pole_triggers {
                r.action = "kept + trigger-added".into();
                r.to_name = ident.trim_end_matches(".Item.Gbx").to_string();
                r.from = format!("{} yaw {:.4} pitch {:.4} roll {:.4}", pos_str(it.pos), it.yaw, it.pitch, it.roll);
                r.to = format!("new item#{} at the same pose", m.items.len() + appended.len());
                r.note = format!("the flag record untouched (waving cloth, chart kept); an invisible {} appended at its pose (it turns with the flag's yaw), tag Goal", if o.cloth { "ClothFinishTrigger" } else { "PoleFinishTrigger" });
                appended.push((it.index, ident.trim_end_matches(".Item.Gbx").to_string(), it.pos, [it.yaw, it.pitch, it.roll]));
            } else {
                r.action = "to-finish".into();
                r.to_name = ident.trim_end_matches(".Item.Gbx").to_string();
                r.from = format!("{} yaw {:.4}", pos_str(it.pos), it.yaw);
                r.to = r.from.clone();
                r.note = "model renamed in place (pose, scale, colour kept); tag Goal".into();
            }
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
        r.note = format!("{converted} flags {} ({} model(s)); {} finish items removed, {} finish blocks -> plain twins; collection {collection}", if o.pole_triggers { "kept + invisible pole finish triggers appended" } else { "-> finishes" }, built.len(), finish_items.len(), finish_blocks.len());
        rows.push(r);
    }
    if dry {
        return Ok(FlagsOutcome { rows, converted, finishes_removed: finish_items.len(), finish_blocks_replaced: finish_blocks.len(), replaced_blocks: 0, removed_blocks: 0, removed_baked: 0, items: built.iter().map(|(s, i, b, _)| (s.clone(), i.clone(), *b)).collect(), new_name: new_name.clone(), new_uid: new_uid.clone() });
    }
    if let Some(dir) = out.parent() {
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    // pass 0: the strip pass (sttc's): the FINISH pieces — blocks → plain twins by the geometry rule,
    // items removed — and, with `sttf`, the CHECKPOINTS the same way. Its rows carry the objmap
    // vocabulary (step "sttf": replaced / removed per block / item / baked) for the lightmap pass.
    let targets: Vec<i32> = if o.sttf { vec![WP_FINISH, crate::sttc::WP_CHECKPOINT] } else { vec![WP_FINISH] };
    let cur = out.with_extension("strip.tmp.Map.Gbx");
    let strip = crate::sttc::strip_waypoints(&mut ctx, src, &cur, crate::sttc::CpMode::Plain, false, &targets)?;
    let strip_rows: Vec<Row> = strip.rows.clone();
    rows.extend(strip.rows.iter().cloned());
    let finishes_removed_total = strip.removed_items;
    {
        let mut mm = MapFile::try_load(&cur)?;
        mm.strip_validation_ghost_to(tmmaps::map::GhostForm::Remove);
        if o.unlock {
            mm.remove_password();
        }
        mm.write_to(out).map_err(|e| e.to_string())?;
    }
    // pass 2: embed the item(s) (a splice of 0x03043054) — MERGED with what the map already embeds
    // (Fall 21–25 carry Nadeo's TME_* items: their rows and entries stay, collection word as is)
    if !no_finish {
        let mut mm = MapFile::try_load(out)?;
        let (old_rows, old_zip) = mm.embedded_manifest();
        let mut files: BTreeMap<String, Vec<u8>> = BTreeMap::new();
        for (name, data) in tmmaps::header::zip_entries(&old_zip) {
            files.insert(name, data);
        }
        let mut manifest: Vec<(String, u32, String)> = old_rows.clone();
        // THE PROVEN FORM (the tiny campaign's embedded library items, 2026-09-08 "Missing Items"
        // lesson; Hugo's game refused round 1 with exactly that dialog): the archive entry is
        // `Items/<ident>`, the manifest row (ident, collection = the map's, author = ident), the
        // item file's header + body ident (ident, collection, author = ident), and every placement
        // names the FULL ident with its `.Item.Gbx` extension, author = ident, collection = the map's
        for (_, ident, _, bytes) in &built {
            let b = crate::tiny_assets::set_ident_collection(bytes, collection);
            files.insert(format!("Items/{ident}"), b);
            manifest.push((ident.clone(), collection, ident.clone()));
        }
        let zip = tmmaps::header::stored_zip(&files);
        let refs: Vec<(&str, u32, &str)> = manifest.iter().map(|(a, c, b)| (a.as_str(), *c, b.as_str())).collect();
        mm.replace_embedded_objects_rows(&refs, &zip);
        mm.write_to(out).map_err(|e| e.to_string())?;
        if !old_rows.is_empty() {
            let mut r = Row::new(&map_label, "flags", "map", 0, "embedded");
            r.action = "merged".into();
            r.note = format!("{} existing embedded item(s) kept ({} zip entries) + {} of ours", old_rows.len(), files.len() - built.len(), built.len());
            rows.push(r);
        }
    }
    // pass 3: the trigger carriers
    let base_items = MapFile::try_load(out)?.items.len();
    if no_finish {
        let mut r = Row::new(&map_label, "flags", "map", 0, "no flags");
        r.action = "NO FINISH".into();
        r.note = "the map has no flag item: nothing embedded, no carrier placed; the map has NO finish (built on --allow-no-finish)".into();
        rows.push(r);
    } else if o.pole_triggers {
        // 3a: clones appended (one per flag)
        {
            let mut mm = MapFile::try_load(out)?;
            mm.append_item_clones(base_items + appended.len());
            mm.write_to(out).map_err(|e| e.to_string())?;
        }
        // 3b: model + author (renames): the full ident, author = ident
        {
            let mut mm = MapFile::try_load(out)?;
            for (k, (_, ident_stem, _, _)) in appended.iter().enumerate() {
                let ident = format!("{ident_stem}.Item.Gbx");
                mm.set_item_model(base_items + k, &ident);
                mm.set_item_author(base_items + k, &ident);
                mm.set_item_collection(base_items + k, collection);
            }
            mm.write_to(out).map_err(|e| e.to_string())?;
        }
        // 3c: pose / pivot / flags / scale / cell / colour (patches)
        {
            let mut mm = MapFile::try_load(out)?;
            let ground = tmmaps::map::ground_y(collection);
            for (k, (flag_index, _, pos, rot)) in appended.iter().enumerate() {
                let i = base_items + k;
                let it = mm.items[i].clone();
                if it.skin_region.is_some() || it.flags & 4 != 0 {
                    mm.set_item_skin(i, None);
                }
                mm.move_item_pos(i, *pos);
                mm.set_item_rotation(i, rot[0], rot[1], rot[2]);
                mm.set_item_pivot(i, [0.0, 0.0, 0.0]);
                mm.set_item_flags(i, 0);
                mm.set_item_scale(i, 1.0);
                let cell = [((pos[0] / 32.0).floor() as i32).clamp(0, 255) as u8, (((pos[1] - ground) / 8.0).floor() as i32).clamp(0, 255) as u8, ((pos[2] / 32.0).floor() as i32).clamp(0, 255) as u8];
                mm.set_item_cell(i, cell);
                let _ = flag_index;
            }
            mm.write_to(out).map_err(|e| e.to_string())?;
        }
    } else {
        // round 1: the flag placements renamed to the finish item (Id-table renames): the full
        // ident, author = ident, collection = the map's
        let mut mm = MapFile::try_load(out)?;
        for (i, it) in mm.items.clone().iter().enumerate() {
            let stem = it.model.trim_end_matches(".Item.Gbx");
            if let Some((_, ident, _, _)) = built.iter().find(|(s, ..)| s == stem) {
                mm.set_item_model(i, ident);
                mm.set_item_author(i, ident);
                mm.set_item_collection(i, collection);
            }
        }
        mm.write_to(out).map_err(|e| e.to_string())?;
    }
    // pass 4 (splices): the Goal tags, the name, the times
    {
        let mut mm = MapFile::try_load(out)?;
        let idents: Vec<String> = built.iter().map(|(_, i, _, _)| i.clone()).collect();
        for (i, it) in mm.items.clone().iter().enumerate() {
            if idents.iter().any(|s| *s == it.model) {
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
    let _ = std::fs::remove_file(&cur);
    // pass 6: the lightmap chart table renumbered positionally (the removed finish items shift
    // every later item by their count; the flags KEEP their slots — same object, same look —
    // so their charts stay; the finish items' charts drop). The sttc object map vocabulary:
    // "sttf"-step rows, "removed" items = the finish items; everything else kept.
    if o.keep_lightmap {
        let objmap = crate::sttc::objmap_rows(&m, &strip_rows, &[], &[], &[]);
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
    Ok(FlagsOutcome { rows, converted, finishes_removed: finishes_removed_total, finish_blocks_replaced: finish_blocks.len(), replaced_blocks: strip.replaced, removed_blocks: strip.removed_blocks, removed_baked: strip.removed_baked, items: built.iter().map(|(s, i, b, _)| (s.clone(), i.clone(), *b)).collect(), new_name: new_name.clone(), new_uid: new_uid.clone() })
}

/// The map's lightmap cache FILETIME word (chunk 0x06022013 inside 0x0304305B), through `lmtool filetime-check --tsv`.
pub fn lightmap_cache_word(map: &Path) -> Option<u64> {
    let lmtool = std::env::current_exe().ok().and_then(|e| e.parent().map(|d| d.join("lmtool"))).filter(|p| p.exists()).map(|p| p.display().to_string()).unwrap_or_else(|| "lmtool".to_string());
    let tsv = std::env::temp_dir().join(format!("flags-ftcheck-{}.tsv", std::process::id()));
    let ok = std::process::Command::new(&lmtool).arg("filetime-check").arg(map).arg("--tsv").arg(&tsv).output().ok()?.status.success();
    if !ok {
        return None;
    }
    let text = std::fs::read_to_string(&tsv).ok()?;
    let _ = std::fs::remove_file(&tsv);
    let line = text.lines().nth(1)?;
    line.split('\t').nth(1)?.parse::<u64>().ok()
}

pub fn verify_flags(src: &Path, out: &Path, oc: &FlagsOutcome, o: &FlagsOpts) -> Result<Vec<String>, String> {
    let a = MapFile::try_load(src)?;
    let b = MapFile::try_load(out)?;
    let mut bad = Vec::new();
    let want_items = if o.pole_triggers { a.items.len() - oc.finishes_removed + oc.converted } else { a.items.len() - oc.finishes_removed };
    if want_items != b.items.len() {
        bad.push(format!("items: want {want_items}, got {}", b.items.len()));
    }
    if o.pole_triggers {
        // the flags untouched: the source's flag records, in order, equal the output's flag records in order
        let fa: Vec<&tmmaps::map::ItemRec> = a.items.iter().filter(|x| x.model.contains("Flag")).collect();
        let fb: Vec<&tmmaps::map::ItemRec> = b.items.iter().filter(|x| x.model.contains("Flag") && !x.model.contains("FinishTrigger")).collect();
        if fa.len() != fb.len() {
            bad.push(format!("{} flag records in, {} out", fa.len(), fb.len()));
        }
        for (x, y) in fa.iter().zip(fb.iter()) {
            if !(y.model == x.model && y.pos == x.pos && y.yaw == x.yaw && y.pitch == x.pitch && y.roll == x.roll && y.waypoint_tag == x.waypoint_tag && y.scale == x.scale) {
                bad.push(format!("flag item#{} -> #{}: {} at {} differs ({} at {})", x.index, y.index, x.model, pos_str(x.pos), y.model, pos_str(y.pos)));
                break;
            }
        }
        // every appended trigger sits exactly at a flag's pose
        let idents: Vec<String> = oc.items.iter().map(|(_, i, _)| i.clone()).collect();
        for y in b.items.iter().filter(|y| idents.contains(&y.model)) {
            if !a.items.iter().any(|x| x.model.contains("Flag") && x.pos == y.pos && (x.yaw - y.yaw).abs() < 1e-6) {
                bad.push(format!("trigger item#{} at {} matches no flag pose", y.index, pos_str(y.pos)));
                break;
            }
            if y.pivot != [0.0; 3] || y.scale != 1.0 {
                bad.push(format!("trigger item#{} pivot {:?} scale {}", y.index, y.pivot, y.scale));
                break;
            }
        }
    }
    let goals = b.items.iter().filter(|x| x.waypoint_tag.as_deref() == Some("Goal")).count() + b.blocks.iter().filter(|x| x.waypoint_tag.as_deref() == Some("Goal")).count();
    if goals != oc.converted {
        bad.push(format!("{goals} Goal placements, want {} (the flags)", oc.converted));
    }
    let idents: Vec<String> = oc.items.iter().map(|(_, i, _)| i.clone()).collect();
    let n = b.items.iter().filter(|x| idents.contains(&x.model)).count();
    if n != oc.converted {
        bad.push(format!("{n} placements wear the finish item, want {}", oc.converted));
    }
    for x in b.items.iter().filter(|x| idents.contains(&x.model)) {
        if x.waypoint_tag.as_deref() != Some("Goal") {
            bad.push(format!("item#{} {} has tag {:?}", x.index, x.model, x.waypoint_tag));
            break;
        }
        if x.author.as_deref() != Some(x.model.as_str()) {
            bad.push(format!("item#{} author {:?} != its ident {}", x.index, x.author, x.model));
            break;
        }
        if x.collection_raw != b.body_collections().map(|c| c[0].1).unwrap_or(26) {
            bad.push(format!("item#{} collection {} != the map's", x.index, x.collection_raw));
            break;
        }
    }
    let emb = crate::embedded::files(&b)?;
    let (rows_a, _) = a.embedded_manifest();
    let (rows_b, _) = b.embedded_manifest();
    for ra in &rows_a {
        if !rows_b.contains(ra) {
            bad.push(format!("the source's embedded manifest row {ra:?} is gone"));
            break;
        }
    }
    let emb_a = crate::embedded::files(&a).unwrap_or_default();
    for (k, v) in &emb_a {
        match emb.get(k) {
            Some(w) if w == v => {}
            _ => { bad.push(format!("the source's embedded entry {k} is gone or changed")); break; }
        }
    }
    for (_, ident, _) in &oc.items {
        let key = format!("Items/{ident}");
        if !emb.keys().any(|k| k.replace('\\', "/").eq_ignore_ascii_case(&key)) {
            bad.push(format!("{key} not in the embedded zip ({} files: {})", emb.len(), emb.keys().cloned().collect::<Vec<_>>().join(", ")));
        }
        // the embedded file's header ident: (ident, the map's collection, author = ident)
        if let Some(bytes) = emb.iter().find(|(k, _)| k.replace('\\', "/").eq_ignore_ascii_case(&key)).map(|(_, v)| v) {
            match tmmaps::header::item_ident_author(bytes) {
                Some((n, a)) if n == *ident && a == *ident => {}
                other => bad.push(format!("{key}: header (ident, author) {other:?}, want ({ident}, {ident})")),
            }
        }
    }
    if o.sttf {
        let cps = b.items.iter().filter(|x| x.waypoint_tag.as_deref() == Some("Checkpoint")).count() + b.blocks.iter().filter(|x| x.waypoint_tag.as_deref() == Some("Checkpoint")).count();
        if cps != 0 {
            bad.push(format!("{cps} checkpoint placements remain (sttf)"));
        }
    }
    let spawns = b.items.iter().filter(|x| x.waypoint_tag.as_deref() == Some("Spawn")).count() + b.blocks.iter().filter(|x| x.waypoint_tag.as_deref() == Some("Spawn")).count();
    if spawns != 1 {
        bad.push(format!("{spawns} Spawn placements, want 1"));
    }
    let hb = tmmaps::header::read(out.to_str().unwrap_or_default())?;
    if hb.validated != "0" || hb.name != oc.new_name || hb.uid != oc.new_uid {
        bad.push(format!("header validated={} name={:?} uid={}", hb.validated, hb.name, hb.uid));
    }
    Ok(bad)
}
