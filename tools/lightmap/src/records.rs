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
    /// The map item index for an item record, and its model / species name.
    pub item: Option<(usize, String)>,
    /// The placement's scale word (1.0 unless the item is scaled): the chart extent is uv · MeterByUv · quality · scale.
    pub scale: f32,
    /// The record's LM mesh source for a prefab entity record (blocks, clips, walls): the prefab file, the entity index and
    /// the entity's world transform — the bake's LM scene (lmmesh::lm_scene_add_entities) builds the mesh from the entity's
    /// Solid2Model and places it with this transform.
    pub mesh: Option<MeshRef>,
}

/// Where a prefab entity record's LM mesh comes from.
#[derive(Clone, Debug)]
pub struct MeshRef {
    pub prefab: String,
    pub entity: usize,
    pub xf: mapgeom::geom::Xform,
    /// THE OWNER BLOCK'S MATERIAL MODIFIER (G2, 2026-09-28; read from the block info's chunk 0x0304E031): the modifier file's stem, e.g.
    /// `TrackWallToDecoCliff` — the block's mobils AND its generated clips are drawn with the modifier's material substitution
    /// (stpad, f4468: the 212 Base_VFCMiddle_Air walls bind DecoCliff's textures — PxzBaseColor trans 0.25, the 4×4 DisabledModX2,
    /// PS 9517 — where their prefab names TrackWall; WaterBase.EDClassic carries `Stadium\Media\Modifier\TrackWallToDecoCliff.Gbx`).
    pub modifier: Option<String>,
}

thread_local! {
    /// The material modifier of the block whose records are being generated (`with_material_modifier`), read by
    /// `prefab_entity_records_in` into every MeshRef it pushes.
    static CURRENT_MODIFIER: std::cell::RefCell<Option<String>> = std::cell::RefCell::new(None);
}

/// The material modifier REF of a block info (the first of its three 0x0304E031 slots that names one; `Stadium\Media\Modifier\
/// TrackWallToDecoCliff.Gbx`, `…\Reset.TerrainModifier.Gbx`) — the game's mechanism is a FOLDER of replacement materials by name:
/// `modifier_folder` names it, `apply_material_modifier` substitutes every link whose material the folder holds.
pub fn material_modifier_of(bi: &mapgeom::blockinfo::BlockInfo) -> Option<String> {
    bi.material_modifier_slots.iter().flatten().map(|p| p.replace(' ', "")).find(|p| modifier_folder(p).is_some())
}

/// The folder a modifier ref stands for (mapgeom's reading of the modifier files: `TrackWallToDecoCliff.Gbx` → its chunk 0x0915D000's
/// string `Stadium\Media\Modifier\PlatformGrass\`; `X.TerrainModifier.Gbx` → `X\`).
pub fn modifier_folder(r: &str) -> Option<String> {
    if mapgeom::tiny_library::is_track_wall_to_deco_cliff(r) { return Some(mapgeom::tiny_library::TRACK_WALL_TO_DECO_CLIFF_FOLDER.to_string()); }
    mapgeom::tiny_library::terrain_modifier_base(r).map(|b| format!("{b}\\"))
}

pub fn with_material_modifier<T>(m: Option<String>, f: impl FnOnce() -> T) -> T {
    let prev = CURRENT_MODIFIER.with(|c| c.replace(m));
    let r = f();
    CURRENT_MODIFIER.with(|c| *c.borrow_mut() = prev);
    r
}

/// The material links a modifier substitutes: `<dir>\<Name>` → `<folder><Name>` for every Name whose `<folder><Name>.Material.Gbx` the
/// pack holds (stpad: `Stadium\Media\Material\TrackWall` → `Stadium\Media\Modifier\PlatformGrass\TrackWall` = Pxz DecoCliffPxz_D, X2
/// disabled, parent PyPxzDiff_Spec_Norm_LM1 — the textures the captured pre-pass draws bind). Returns the substituted count.
pub fn apply_material_modifier(store: &mapgeom::store::DataStore, modifier: &str, links: &mut [String]) -> usize {
    let Some(folder) = modifier_folder(modifier) else { return 0 };
    let mut n = 0;
    for l in links.iter_mut() {
        let Some((_, name)) = l.rsplit_once('\\') else { continue };
        let cand = format!("{folder}{name}");
        let file = format!("{cand}.Material.Gbx");
        if store.entries().any(|e| e.path().eq_ignore_ascii_case(&file)) { *l = cand; n += 1; }
    }
    n
}

impl Rec {
    pub fn ext(&self) -> [f32; 2] {
        // the placement's scale word rides with the quality (layout::for_map's `plg_u02 * (q * sc)` — the same rounding)
        let f = self.meter_by_uv * (self.quality * self.scale);
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
    /// The model / PLG pointer columns (the game's clone identity).
    pub model: String,
    pub plg: String,
}

pub fn read_dump(path: &str) -> Result<Vec<DumpRec>, String> {
    let txt = std::fs::read_to_string(path).map_err(|e| format!("{path}: {e}"))?;
    let mut lines = txt.lines();
    let header: Vec<&str> = lines.next().ok_or("empty dump")?.split('\t').collect();
    let col = |name: &str| header.iter().position(|h| *h == name).ok_or_else(|| format!("{path}: no column {name}"));
    let (ci, cm, cu0, cv0, cu1, cv1, cq, cx, cy, cz, chx, chy, chz, ck) = (col("i")?, col("meterByUv")?, col("u0")?, col("v0")?, col("u1")?, col("v1")?, col("quality")?, col("centerX")?, col("centerY")?, col("centerZ")?, col("halfX")?, col("halfY")?, col("halfZ")?, col("key")?);
    let (ccx, ccy, ccw, cch) = (col("cx")?, col("cy")?, col("cw")?, col("ch")?);
    let (cmodel, cplg) = (header.iter().position(|h| *h == "model"), header.iter().position(|h| *h == "plg"));
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
            model: cmodel.map(|c| f[c].trim().to_string()).unwrap_or_default(),
            plg: cplg.map(|c| f[c].trim().to_string()).unwrap_or_default(),
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
            out.push(Rec { class: "tile", obj: 0, sub: 0, meter_by_uv: mbu, uv, quality: q, centre: w.c, half: w.h, group: 0xF000_0000_0000_0000 | q.to_bits() as u64, key_centre: None, pos_rank: None, wall: None, item: None, scale: 1.0, mesh: None });
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
    let groups: Vec<u64> = recs.iter().map(|r| crate::layout::isolate_group_key(r.group, r.obj)).collect();
    let any_pos = recs.iter().any(|r| r.pos_rank.is_some());
    let pos: Vec<u32> = recs.iter().enumerate().map(|(i, r)| r.pos_rank.unwrap_or(i as u32)).collect();
    let walls: Vec<Option<(u8, f32)>> = recs.iter().map(|r| if std::env::var("LMTOOL_NO_WALLS").is_ok() { None } else { r.wall }).collect();
    crate::layout::STRIP_RECS.with(|s| { let mut s = s.borrow_mut(); s.clear(); for (k, r) in recs.iter().enumerate() { if r.class == "item0" { s.insert(k); } } });
    let pos_opt = if any_pos { Some(&pos[..]) } else { None };
    // THE FIRST ALLOCATION (RE 14 2026-09-27 20:55Z; F): the game packs the same records into 3 072 × 2 048 first — the LightId
    // accumulation target's layout (RenderLighting_Frames' outer state 0x3477) — and lm218+0x488 keeps THAT s through the sweeps
    // (UpdateMapping #2 on the 2 048² file layout skips the write on its "mapping already valid" branch), so FUN_140230080 sizes the
    // world peel and the fitted tiles with it: giant20x2 3 117 m × 1.04 = 3 251 > 3 072 → 4 096², n 1 → NO fitted pass (measured:
    // FCB 1.57 → 1.14, record 1.63 → 1.42); tiny03 1.62 → 4 096², n 1 (AV palms 0.44 → 0.71, record −26 % → −1.7 %); stpad 2.60 →
    // 4 096², n 2 (the two captured cells either way). The file's layout stays the 2 048² one. LMTOOL_NO_FIRST_PASS=1 skips it.
    let mut gl = crate::layout::allocate_grouped_walls(&crate::layout::LayoutInput { tiles: Vec::new(), items: items.clone(), w_atlas: 2048, quality_index, h_atlas: 0, d1_side: 0 }, &groups, pos_opt, Some(&walls))?;
    // THE VALUE: s_first = s_file · √1.5 — the same Σ in 1.5× the area (RE 14's reading of the first target). The real pack's bisection
    // can land a step away; what pins it: stpad's CAPTURED two cells need ext = 3 117 m · s_first ≤ 8 192 → s_first ≤ 2.628 (√1.5 gives
    // 2.596 ✓), giant20x2 and tiny03 need no tiles → s_first ∈ (0.986, 1.314] and (1.5, 2.0] (√1.5 gives 1.043 and 1.624 ✓), so the
    // ratio k = s_first / s_file lies in (1.157, 1.24]. Our own re-packs into 3 072 × 2 048 come out at k 1.25 (the 2 048² entries,
    // LMTOOL_FIRST_PASS=pack: stpad 2.649 → n 3 ✗) or 1.28 (entries regrouped at the 3 072 × 2 048 density, =pack-own-d1: 2.717 ✗),
    // both over the window — so the game's first pack differs from a plain re-pack in some input (its record set? its start
    // scale?) and the formula stands until a dump of lm218+0x488 (stpad: 2.596 predicted; RE 7's c0 read 2.1158 would give the
    // giant tiles, which the classes refute). LMTOOL_NO_FIRST_PASS=1 → the port's density heuristic as before.
    if std::env::var_os("LMTOOL_NO_FIRST_PASS").is_none() {
        // THE DEFAULT (F 2026-09-27 22:45Z, after the g23 regression of the √1.5 formula) — AN EMPIRICAL RULE, not a read: the re-pack
        // into 3 072 × 2 048 at ONE shrink step (the first scale of the 0.9 ladder that packs, no bisection — `max_iter_for_quality(0)`).
        // RE 14 (23:00Z) retracted the "wider pack" mechanism: the 3 072 width is the same 2 048² chart layout in a 1.5×-wide LightId
        // target, the s search's inputs are identical for every UpdateMapping call (no per-call iteration count), and with a shipped
        // LightMapCache call #1 does not write +0x488 at all — so by the code the s live at bounce time would be the file's (k 1),
        // which the measured structures refute on the giants (k 1 tiles giant20x2 at 2 048² → FCB 1.57, untiles g23 → 9 720 unlit
        // tiles, tiles tiny03 → record −6.8 %). The mechanism RE 14 can name is the CHART SET of the allocation live at bounce
        // time (records created so far / the LightId charts — sparser on item-heavy maps → a larger s); RE 7's /lmrecords dump at
        // c0 vs c1 on giant20x2 (record count + s) is the read that settles it when the box frees. Until then this rule is the one
        // variant that meets every measured structure at once — stpad's captured two cells (ext 8 078 ≤ 8 192), giant20x2 and tiny03 without tiles (3 245 and
        // 3 246 > 3 072, ≤ 4 096), tiny16's 2 048² world peel (1 227), and g23 TILED (4 718 → n 2; the world pass alone leaves
        // 9 700 sea-floor tiles unlit at any resolution while the editor lights 95 % of them). The full-iteration re-pack lands
        // stpad at 8 258 (n 3 ✗) and the √1.5 formula g23 at 4 057 (n 1 ✗). RE 7's stpad dump (first pack s 2.1158 ≈ s_file)
        // fits none of these and is an open question to RE 14 (the search's iteration count at the LightId allocation).
        // LMTOOL_FIRST_PASS=formula → s_file·√1.5; =pack → the re-pack at the map's quality; =pack-own-d1 → regrouped at the wide
        // density; LMTOOL_FIRST_PASS_Q=n → the re-pack's search at quality n.
        // THE DEFAULT IS THE READ (k = 1, RE 15 read 2; coordinator 2026-09-28 04:30Z after G2's modifier routing landed): the peel tiling
        // scale = the FILE's s. F's pack-q0 (k ≈ 1.25) stays as a knob — it was an empirical compensation for the fitted pass's underside
        // sky leak (G2's open cell): under it the giants' tiles were off and the card residue hidden. LMTOOL_FIRST_PASS=pack-q0 restores it.
        let mode = std::env::var("LMTOOL_FIRST_PASS").unwrap_or_else(|_| "file".to_string());
        if mode == "file" {
            // RE 15 (read 2, NOTES 03:30Z): the compute's UpdateMapping runs with a NULL cache, so the allocation live at bounce time IS
            // the file's 2 048² pack — s_live = s_file (k = 1): stpad n 2 (the captured two cells), giant20x2 n 2, tiny03 n 2, g23 n 1.
            // The read, beside F's empirical default (pack-q0) until the structures under it are measured against the editor (E2,
            // 2026-09-28 03:10Z, the coordinator's cell 3).
            gl.s_first = Some(gl.s);
            if std::env::var_os("LMTOOL_LAYOUT_TRACE").is_some() { eprintln!("layout first pass (=file, RE 15 read 2): s_first = s_file = {} — the peel tiling scale", gl.s); }
            return Ok(gl);
        }
        if mode == "ungrouped-q0" || mode == "ungrouped" {
            // RE 15 (00:45Z, read 2): the pack live at bounce time is ONE UNGROUPED whole-set pack on the SAME (S, S) target with maxIter 1
            // (desc[0] = L[0] = 0 → the quality-0 search: the first fitting 0.9-ladder step, no bisection; the > 8 192-block grouping off) —
            // stpad's dump c0 2.1158 = √(0.9 · 2048²/Σ) to the digit. `layout::allocate` (the ungrouped search) at quality 0 (=ungrouped-q0) or
            // at the map's quality (=ungrouped).
            match crate::layout::allocate(&crate::layout::LayoutInput { tiles: Vec::new(), items, w_atlas: 2048, quality_index: if mode == "ungrouped-q0" { 0 } else { quality_index }, h_atlas: 0, d1_side: 0 }) {
                Ok(first) => { gl.s_first = Some(first.s); if std::env::var_os("LMTOOL_LAYOUT_TRACE").is_some() { eprintln!("layout first pass (ungrouped whole-set pack on 2048², {mode}): s {} Σ {} — the peel tiling scale; the file layout's s {}", first.s, first.sum_area, gl.s); } }
                Err(e) => eprintln!("layout first pass (ungrouped) FAILED: {e} — the peel tiling falls back to the port's density heuristic"),
            }
        } else if mode.starts_with("pack") {
            crate::layout::STRIP_RECS.with(|s| { let mut s = s.borrow_mut(); s.clear(); for (k, r) in recs.iter().enumerate() { if r.class == "item0" { s.insert(k); } } });
            match crate::layout::allocate_grouped_walls(&crate::layout::LayoutInput { tiles: Vec::new(), items, w_atlas: 3072, quality_index: std::env::var("LMTOOL_FIRST_PASS_Q").ok().and_then(|v| v.parse().ok()).unwrap_or(if mode == "pack-q0" { 0 } else { quality_index }), h_atlas: 2048, d1_side: if mode == "pack-own-d1" { 0 } else { 2048 } }, &groups, pos_opt, Some(&walls)) {
                Ok(first) => { gl.s_first = Some(first.s); if std::env::var_os("LMTOOL_LAYOUT_TRACE").is_some() { eprintln!("layout first pass (3072 × 2048 re-pack, {mode}): s {} Σ {} ({} entries) — the peel tiling scale; the file layout's s {}", first.s, first.sum_area, first.entries.len(), gl.s); } }
                Err(e) => eprintln!("layout first pass (3072 × 2048) FAILED: {e} — the peel tiling falls back to the port's density heuristic"),
            }
        } else {
            gl.s_first = Some(gl.s * 1.5f32.sqrt());
            if std::env::var_os("LMTOOL_LAYOUT_TRACE").is_some() { eprintln!("layout first pass: s_first = s · √1.5 = {} — the peel tiling scale (the file layout's s {})", gl.s_first.unwrap(), gl.s); }
        }
    }
    Ok(gl)
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
    // the centre / half precision per class: exact f32 matches and the max |Δ| (the Morton key rounds the centre to metres)
    {
        let mut prec: std::collections::BTreeMap<&str, (usize, usize, f32, f32, usize, f32, usize)> = Default::default();
        let mut uv_examples = 0;
        for &(i, j) in &order_pairs {
            let (r, d) = (&ours[i], &dump[j]);
            let e = prec.entry(r.class).or_insert((0, 0, 0.0, 0.0, 0, 0.0, 0));
            if r.centre == d.centre { e.0 += 1; }
            if r.half == d.half { e.1 += 1; }
            for k in 0..3 { e.2 = e.2.max((r.centre[k] - d.centre[k]).abs()); e.3 = e.3.max((r.half[k] - d.half[k]).abs()); }
            if r.uv == d.uv { e.4 += 1; } else if uv_examples < 6 { uv_examples += 1; println!("  {} #{i}: uv {:?} vs dump {:?} (MBU {} vs {})", r.class, r.uv, d.uv, r.meter_by_uv, d.meter_by_uv); }
            for k in 0..4 { e.5 = e.5.max((r.uv[k] - d.uv[k]).abs()); }
            if r.meter_by_uv == d.meter_by_uv { e.6 += 1; }
        }
        for (c, (ce, he, dc, dh, ue, du, me)) in &prec { println!("  {c}: centre bit-exact {ce}, half bit-exact {he}, max |Δcentre| {dc:.6}, max |Δhalf| {dh:.6}; uv bounds bit-exact {ue} (max |Δ| {du:.6}), MeterByUv bit-exact {me}"); }
    }
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
                out.push(Rec { class, obj, sub: *sub, meter_by_uv: plg.u02, uv: [plg.u04[0], plg.u04[1], plg.u04[2], plg.u04[3]], quality, centre: w.c, half: w.h, group, key_centre: None, pos_rank: None, wall, item: None, scale: 1.0, mesh: Some(MeshRef { prefab: prefab_path.to_string(), entity: ei, xf: e_xf, modifier: CURRENT_MODIFIER.with(|c| c.borrow().clone()) }) });
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
/// THE LIGHTS OF A PLACED BLOCK (RE 7, 2026-09-25 — stpad's lightmap frame 1): every CPlugLight of every static object in
/// the picked placement's mobil prefabs, nested prefabs included (`Water\FCCenter_Air` → `TreeGen\RoadBorderSpot.Prefab` →
/// its solid's `RoadBorderSpot.Light.Gbx`), in WORLD space through the same transforms as `block_records`.
pub fn block_lights(store: &mut mapgeom::store::DataStore, bi: &mapgeom::blockinfo::BlockInfo, cell: [i32; 3], dir: u8, ground: bool, variant: usize, subvariant: usize, additional: usize, yoff: f32) -> Result<Vec<crate::geometry::LightDef>, String> {
    let Some(picked) = bi.pick_placement_add(ground, variant, subvariant, additional) else { return Ok(Vec::new()) };
    let xf = mapgeom::place::grid_block((cell[0], cell[1], cell[2]), dir, (32.0, 32.0), yoff);
    let mut out = Vec::new();
    for mb in &picked.mobils {
        let Some(pp) = &mb.prefab else { continue };
        let mxf = match (mb.translation, mb.rotation) {
            (Some(t), r) => { let yaw = r.map(|v| v[1]).unwrap_or(0.0); mapgeom::geom::compose(&xf, &mapgeom::geom::yaw(yaw, t)) }
            _ => xf,
        };
        prefab_entity_lights_in(store, pp, &mxf, 0, &mut out)?;
    }
    Ok(out)
}

/// The lights of a prefab file's entities in the frame `xf`, nested prefabs recursed (depth-bounded).
pub fn prefab_entity_lights_in(store: &mut mapgeom::store::DataStore, prefab_path: &str, xf: &mapgeom::geom::Xform, depth: u32, out: &mut Vec<crate::geometry::LightDef>) -> Result<(), String> {
    let pm = store.load_model(prefab_path)?;
    let pf = mapgeom::static_item::prefab::CPlugPrefab::from_model(&pm)?;
    for e in pf.ents.iter() {
        let e_xf = mapgeom::geom::compose(xf, &mapgeom::geom::from_quat(e.rot, e.pos));
        match e.model.inline.as_deref() {
            Some(mapgeom::static_item::Node::StaticObject(so)) => {
                let Some(s2) = so.solid2() else { continue };
                for mut l in crate::geometry::solid2_lights_ext(s2, None, Some((store, &pm.externals))) {
                    let p = mapgeom::geom::apply(&e_xf, l.pos);
                    let tip = mapgeom::geom::apply(&e_xf, [l.pos[0] + l.dir[0], l.pos[1] + l.dir[1], l.pos[2] + l.dir[2]]);
                    let tl = mapgeom::geom::apply(&e_xf, [l.pos[0] + l.left[0], l.pos[1] + l.left[1], l.pos[2] + l.left[2]]);
                    let tu = mapgeom::geom::apply(&e_xf, [l.pos[0] + l.up[0], l.pos[1] + l.up[1], l.pos[2] + l.up[2]]);
                    l.pos = p;
                    l.dir = crate::geometry::norm([tip[0] - p[0], tip[1] - p[1], tip[2] - p[2]]);
                    l.left = crate::geometry::norm([tl[0] - p[0], tl[1] - p[1], tl[2] - p[2]]);
                    l.up = crate::geometry::norm([tu[0] - p[0], tu[1] - p[1], tu[2] - p[2]]);
                    out.push(l);
                }
            }
            None if e.model.index >= 0 && depth < 8 => {
                let Some((_, path)) = pm.externals.iter().find(|(i, _)| *i == e.model.index as u32) else { continue };
                if path.to_ascii_lowercase().ends_with(".prefab.gbx") {
                    let path = path.clone();
                    prefab_entity_lights_in(store, &path, &e_xf, depth + 1, out)?;
                }
            }
            _ => {}
        }
    }
    Ok(())
}

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
    /// THE LOCAL LIGHTS of the blocks and the engine's clips (RE 7, 2026-09-25 — lightmap frame 1): every CPlugLight of the
    /// placed prefabs, nested prefabs and external `.Light.Gbx` sockets recursed (stpad: 424 RoadBorderSpot lamps, 2 per drawn
    /// WaterFCCenter clip), in world space, tagged with the owner ("block N name" / "clipX name of block N"). The items' lights
    /// come with the scene (`geometry::Scene::lights`).
    pub block_lights: Vec<(String, crate::geometry::LightDef)>,
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
    let mut block_lights_out: Vec<(String, crate::geometry::LightDef)> = Vec::new();
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
        // the block's material modifier (WaterBase: TrackWallToDecoCliff) rides into its records' MeshRefs
        let modifier = material_modifier_of(&bi);
        n_block_recs += with_material_modifier(modifier, || block_records_class(store, &bi, [x, y, z], b.dir, ground, variant, subvariant, additional, yoff, "block", obj, 1.0, b.flags & 0x1000_0000 != 0, &mut recs))?;
        if let Ok(ls) = block_lights(store, &bi, [x, y, z], b.dir, ground, variant, subvariant, additional, yoff) { for l in ls { block_lights_out.push((format!("block {} {}", b.index, b.name), l)); } }
    }
    // 2. the tiles: the map's GENEALOGY (chunk 0x03043043) names every cell's zone and direction (BlueBay tiny16: Sea, Land,
    // frontier and transition tiles) — each cell's record is that zone prefab's PLG with its box through the cell's rotation
    // (lmtiles::tile_records); an empty genealogy (stpad, the resaved tiny maps) means the ground zone everywhere
    let n_tiles_before = recs.len();
    {
        let cells_of: Vec<(i32, i32)> = cells.clone();
        let chunks = tmmaps::gbx::all_skip_chunks(&mf.gbx.body);
        let gen: Vec<(String, u32)> = chunks.iter().find(|(c, ..)| *c == 0x0304_3043).and_then(|&(_, _, payload, size)| tmmaps::map::genealogy_full(&mf.gbx.body[payload..payload + size]).ok()).map(|recs| recs.into_iter().map(|r| (r.current, r.dir)).collect()).unwrap_or_default();
        let tiles = crate::lmtiles::tile_records(store, coll, [grid, 1, grid], &gen, &zone, tile_y as f32, yoff, 1.0)?;
        let mut by_cell: std::collections::HashMap<(i32, i32), (String, crate::lmtiles::BlockRecord)> = tiles.into_iter().map(|(cx, cz, z, _dir, rec)| ((cx as i32, cz as i32), (z, rec))).collect();
        let mut plg_of_zone: std::collections::HashMap<String, Rec> = Default::default();
        // THE TILE CHART is the GROUND zone prefab's PLG for every cell (tiny16, 12 214 / 12 214: one extent over Sea, Land and
        // frontier cells alike — the lightmapper's tile records share the decoration's flat tile model), one clone → one group
        // per quality; the record BOX is the cell's own zone box through its rotation (the genealogy's zone / dir)
        {
            let one = zone_tiles(store, coll, &zone, 1, tile_y as f32, yoff, &|_, _| 1.0)?;
            let Some(r0) = one.into_iter().next() else { return Err(format!("{coll}/{zone}: no zone prefab PLG")) };
            plg_of_zone.insert(zone.clone(), r0);
        }
        let r0 = plg_of_zone[&zone].clone();
        for (k, c) in cells_of.iter().enumerate() {
            let Some((_z, rec)) = by_cell.remove(c) else { return Err(format!("tile cell {c:?} missing")) };
            // tq is indexed like `cells` (the baked cells first, then x-major) — not by coordinates
            let q = tq[k];
            recs.push(Rec { class: "tile", obj: tile_obj0 + k as u32, sub: 0, meter_by_uv: r0.meter_by_uv, uv: r0.uv, quality: q, centre: rec.world.c, half: rec.world.h, group: 0xF000_0000_0000_0000 | q.to_bits() as u64, key_centre: None, pos_rank: None, wall: None, item: None, scale: 1.0, mesh: None });
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
            // a clip is drawn with its OWNER block's material modifier (the clip block infos carry none; stpad's VFC walls = DecoCliff)
            let owner_modifier = owner_b.and_then(|b| idx.path_for(&b.name)).and_then(|p| idx.load(store, &p).ok().map(|obi| material_modifier_of(obi)));
            n_clip_recs += with_material_modifier(owner_modifier.flatten(), || block_records_class(store, &bi, cell, d, c.ground, variant, 0, 0, yoff, class, clip_obj0 + n_clip_objs, 1.0, owner_free, &mut recs))?;
            if let Ok(ls) = block_lights(store, &bi, cell, d, c.ground, variant, 0, 0, yoff) { for l in ls { block_lights_out.push((format!("{class} {} of block {}", c.name, c.owner_index), l)); } }
            n_clip_objs += 1;
        }
    }
    // 4. THE ITEMS, IN ITEM ORDER (tiny03's dump interleaves them: a kind-0 tree and a kind-2 item follow the map's item order) —
    // a kind-2 record for an item the scene loaded (a PreLightGen with non-degenerate uv-0 bounds and a lightmap material), a
    // kind-0 record for a vegetation placement (RE 7, 19:50Z + 20:15Z, bit-exact 380 / 380 on tiny03): its pack item model
    // references .VegetTreeModel.Gbx files, the species = the variant byte into the species list, a record iff the species'
    // VegetTreeModel carries a PreLightGen with u01 ≠ 0 (bushes: hasPLG 0 → none); the quality byte clamp(int(255·G·√2^e), 1,
    // 255) / 255; the box = the LOD-0 non-leaf visuals' fold (VegetTreeModel::lightmap_record_box) through the Iso4 of the
    // VARIED quaternion (veget_instance::variation: the pose-hash seed, the yaw / tilt draws; the scale draw is not in it) with
    // the item position as translation (itemrule::legacy_tree_record_box).
    let item_obj0 = clip_obj0 + n_clip_objs;
    let mut n_items = 0usize;
    let inst_of_item: std::collections::HashMap<usize, usize> = scene.instances.iter().enumerate().map(|(k, i)| (i.item, k)).collect();
    let lm_quality: Vec<u8> = tmmaps::gbx::all_skip_chunks(&mf.gbx.body).iter().find(|(c, ..)| *c == 0x0304_3068).map(|&(_, _, payload, size)| { let start = payload + 4 + mf.blocks.len() + mf.baked.len(); mf.gbx.body[start.min(payload + size)..(payload + size).min(start + mf.items.len())].to_vec() }).unwrap_or_default();
    let irecs = crate::lmtiles::item_records(scene, 1.0, false);
    // THE ITEM CLONE IDENTITY = (item model, placement COLOUR byte — chunk 0x03043062): tiny03's dump shows one Solid2Model / PLG
    // pointer per colour of the same item file (AC00000190: colour 1 → 0x…856600, colour 4 → 0x…85c440; five files split so),
    // so the records of two colours never share a group
    let colours = mf.colors();
    let no_lm: Vec<String> = std::env::var("LMTOOL_NO_LM_MATERIALS").map(|v| v.split(',').map(|t| t.to_string()).collect()).unwrap_or_else(|_| vec!["RaceTriggerFX".into(), "\\Decal".into()]);
    // LMTOOL_ITEM_FILTER_TRACE=1 (port engineer G): which placed items the record walk skips and why — the item the game drops on
    // load (tiny03: the editor's resave keeps 2 919 of 2 943) against ours
    let filter_trace = std::env::var_os("LMTOOL_ITEM_FILTER_TRACE").is_some();
    let mut species_cache: std::collections::HashMap<String, Option<Vec<String>>> = Default::default();
    let mut model_cache: std::collections::HashMap<String, Option<(Option<mapgeom::static_item::solid2::PreLightGen>, Option<([f32; 3], [f32; 3])>, mapgeom::veget_instance::TreeParams)>> = Default::default();
    let mut n_kind0 = 0usize;
    // THE GAME'S ITEM INDEX (port engineer G, 2026-09-26; the WhiteShore "object order" row): the game DROPS an embedded item whose
    // every material is a decal (no-LM) from its item list on load — tiny03's editor resave holds 2 919 of the source's 2 943 items,
    // the 24 missing are exactly the placements of AC00000063/64/72/75/76/79/82/101/102/106/140/141 (Stadium\Media\Material\
    // DecalPlatform, …\Modifier\PlatformGrass\DecalPlatform: nothing else on the model), and the editor's chart obj ids run
    // compactly over the 2 919 — so every obj id after the first dropped item is one lower per dropped item before it. The chart's
    // obj id is the game's index into ITS list; `Rec.item` keeps the file index for everything on our side. LMTOOL_ITEM_DROP=none
    // keeps the file numbering (the pre-rule form). Whether the game's test is "decal-only" or "no material resolves in the
    // collection's pak set" is RE 13's to pin — on this map the two readings name the same 24.
    // ⚠ REVERTED TO THE FILE NUMBERING BY DEFAULT (G, 2026-09-26 23:20Z, V's fix9 tables): the chart's obj id indexes the item list
    // OF THE FILE THAT CARRIES THE MAPPING. The editor's DIRECT bakes keep every item (tiny04ac-GreenCoast-Day-q4-editor: 2 106 items,
    // the 75 decal-only ones included, obj ids over all 2 106 — V's Day comparison was 6 082 / 6 082 by file numbering and fell to
    // 5 709 under the compaction); only the RESAVED / "-reduced" refs (2 031 / 2 919 items, U 4096 materialised ground) carry a
    // compacted list, and those were the files 0009 was verified against. Our output keeps every item of the source, so its obj ids
    // must be file indices. LMTOOL_ITEM_DROP=decal compacts (for a comparison against a resaved ref only); "none" is the default.
    let drop_rule = std::env::var("LMTOOL_ITEM_DROP").unwrap_or_else(|_| "none".into());
    let game_index: Vec<u32> = {
        let mut out = Vec::with_capacity(mf.items.len());
        let mut next = 0u32;
        for (ii, _it) in mf.items.iter().enumerate() {
            // DECAL-ONLY, not no-LM-only: tiny16 (Stadium) keeps its two RaceTriggerFXFinish-only placements (8 619 = 8 619 in the editor's
            // bake) while tiny03 / tiny04ac drop every DecalPlatform-only one; and the kept PlatformTech-only items of tiny03 sit in
            // Stadium.pak exactly as DecalPlatform does, so pak resolution is not the test — the material KIND is (a decal has no surface)
            let dropped = drop_rule != "none" && inst_of_item.get(&ii).map(|&ki| { let m = &scene.models[scene.instances[ki].model]; !m.mat_links.is_empty() && m.mat_links.iter().all(|l| l.contains("\\Decal")) }).unwrap_or(false);
            out.push(next);
            if !dropped { next += 1; }
        }
        let n_dropped = mf.items.len() as u32 - next;
        if n_dropped > 0 { notes.push(format!("{n_dropped} decal-only items are not in the game's item list: the chart obj ids of the {} items after the first of them are renumbered (LMTOOL_ITEM_DROP=none keeps the file numbering)", next)); }
        out
    };
    for (ii, it) in mf.items.iter().enumerate() {
        if let Some(k) = &opts.kept { if !k.contains(&ii) { continue; } }
        // (a STOCK VEGETATION instance — stockveg, E5 — is a scene instance now, but its record is the kind-0 legacy path's below)
        if let Some(&ki) = inst_of_item.get(&ii).filter(|&&ki| scene.models[scene.instances[ki].model].veget.is_none()) {
            let inst = &scene.instances[ki];
            let m = &scene.models[inst.model];
            let Some(b) = m.plg_bounds else { if filter_trace { eprintln!("item-filter: item {ii} {} skipped: no PLG bounds; materials {:?}", it.model, m.mat_links); } continue };
            if !(b[2] > b[0] && b[3] > b[1]) { if filter_trace { eprintln!("item-filter: item {ii} {} skipped: empty PLG bounds {b:?}; materials {:?}", it.model, m.mat_links); } continue; }
            // the compiled material's own answer first (lmmesh::lm_uv_index_cached — the shader's PreLightGen binding through the bake's pack
            // store, RE 11; `Some(None)` = known not lightmapped), the name substrings as the fallback for links the store cannot resolve
            // (F 2026-09-27: g23's four AC06423108 placements = Stadium\Media\Modifier\Reset\TriggerFX, chartless in the editor's bake)
            let link_no_lm = |l: &str| -> bool { matches!(crate::lmmesh::lm_uv_index_cached(l), Some(None)) || no_lm.iter().any(|n| l.contains(n.as_str())) };
            if !m.mat_links.is_empty() && m.mat_links.iter().all(|l| link_no_lm(l)) { if filter_trace { eprintln!("item-filter: item {ii} {} skipped: every material is no-LM {:?}", it.model, m.mat_links); } continue; }
            if filter_trace && std::env::var_os("LMTOOL_ITEM_FILTER_TRACE_ALL").is_some() { eprintln!("item-filter: item {ii} {} kept; materials {:?}", it.model, m.mat_links); }
            let Some(ir) = irecs.iter().find(|r| r.item == inst.item) else { if filter_trace { eprintln!("item-filter: item {ii} {} skipped: no item record", it.model); } continue };
            let Some(rec) = &ir.record else { if filter_trace { eprintln!("item-filter: item {ii} {} skipped: item record without a box", it.model); } continue };
            let q = crate::layout::item_quality(inst.lm_quality);
            let colour = colours.as_ref().map(|c| c.item(ii)).unwrap_or(0) as u64;
            let group = if std::env::var_os("LMTOOL_NO_COLOUR_CLONES").is_some() || !crate::itemrule::colour_cloned(&m.mat_links) { ((inst.model as u64) << 32) | q.to_bits() as u64 } else { ((inst.model as u64) << 40) | (colour << 32) | q.to_bits() as u64 };
            recs.push(Rec { class: "item", obj: item_obj0 + game_index[ii], sub: 0, meter_by_uv: m.plg_u02, uv: b, quality: q, centre: rec.world.c, half: rec.world.h, group, key_centre: None, pos_rank: None, wall: None, item: Some((ii, format!("{} v{} flags {:#x}", it.model, it.variant(), it.flags))), scale: if inst.pose.scale > 0.0 { inst.pose.scale } else { 1.0 }, mesh: None });
            n_items += 1;
            continue;
        }
        let list = species_cache.entry(it.model.clone()).or_insert_with(|| { let file = mapgeom::tiny_library::find_item_file(store, &it.model)?; mapgeom::veget::item_species(store, &file).ok() }).clone();
        let Some(list) = list else { if filter_trace && inst_of_item.get(&ii).is_none() { eprintln!("item-filter: item {ii} {} skipped: no scene instance and no species list", it.model); } continue };
        if list.is_empty() { continue; }
        let v = it.variant() as usize;
        let Some(species) = list.get(v).or_else(|| list.first()).cloned() else { continue };
        let md = model_cache.entry(species.clone()).or_insert_with(|| {
            let m = mapgeom::veget::parse_tree_model(store, &species).ok()?;
            let params = mapgeom::veget_instance::TreeParams { scale_var01: m.scale_var01, angle_max_rot_xz_deg: m.angle_max_rot_xz_deg, enable_random_rotation_y: m.enable_random_rotation_y != 0 };
            let bx = m.lightmap_record_box();
            let plg = store.read(&species).ok().and_then(|bytes| mapgeom::static_item::legacy_plg::veget_tree_prelight(&bytes).ok().flatten());
            Some((plg, bx, params))
        }).clone();
        let Some((Some(plg), Some((c, h)), params)) = md else { continue };
        if plg.u01 == 0 { continue; }
        let e = crate::itemrule::quality_exponent(lm_quality.get(ii).copied().unwrap_or(0));
        let byte = ((255.0f32 * 2f32.powf(e as f32 * 0.5)) as i32).clamp(1, 255);
        let q = byte as f32 / 255.0;
        let (q1, t, seed) = mapgeom::veget_instance::item_pose(it.yaw, it.pitch, it.roll, it.pos, it.pivot);
        let inst = mapgeom::veget_instance::variation(q1, t, seed, params, true);
        let (centre, half) = crate::itemrule::legacy_tree_record_box(inst.quat, it.pos, c, h);
        let mut hh = std::collections::hash_map::DefaultHasher::new();
        use std::hash::{Hash, Hasher};
        // LMTOOL_KIND0_GROUP=item: every kind-0 record its own group (the study); default one group per species
        if std::env::var("LMTOOL_KIND0_GROUP").map(|v| v == "item").unwrap_or(false) { ("legacy", ii).hash(&mut hh); } else { ("legacy", species.as_str()).hash(&mut hh); }
        // LMTOOL_KIND0_KEY=pos|posc: the Morton key from the item position (+ the model box centre) instead of the record centre
        let key_centre = match std::env::var("LMTOOL_KIND0_KEY").ok().as_deref() { Some("pos") => Some(it.pos), Some("posc") => Some([it.pos[0] + c[0], it.pos[1] + c[1], it.pos[2] + c[2]]), Some("zero") => Some([0.0, 0.0, 0.0]), Some("cell") => Some([it.file_cell[0] as f32 * 32.0, it.file_cell[1] as f32 * 8.0, it.file_cell[2] as f32 * 32.0]), _ => None };
        recs.push(Rec { class: "item0", obj: item_obj0 + game_index[ii], sub: 0, meter_by_uv: plg.u02, uv: [plg.u04[0], plg.u04[1], plg.u04[2], plg.u04[3]], quality: q, centre, half, group: (hh.finish() & 0x0000_FFFF_FFFF_0000) | q.to_bits() as u64, key_centre, pos_rank: None, wall: None, item: Some((ii, species.clone())), scale: 1.0, mesh: None });
        n_items += 1;
        n_kind0 += 1;
    }
    if n_kind0 > 0 { notes.push(format!("{n_kind0} kind-0 legacy tree records")); }
    let _ = n_block_recs;
    Ok(MapRecords { recs, n_blocks: n_blocks_authored as usize, n_tiles, n_clips: n_clip_recs, n_items, tile_cells: cells, tile_quality: tq, block_obj0, tile_obj0, clip_obj0, item_obj0, notes, block_lights: block_lights_out })
}




// ─────────────────────────────────────────────────────────────────────────────────────────────────────
// THE RECORD SCENE: the peel / shadow / pre-pass geometry of every record that is not a placed item — the authored blocks'
// and the clips' prefab entities (MeshRef) and the zone tiles — as scene instances with the record's mesh (TexCoord1 = the LM
// uv, so the peel colours them from the ILightInput atlas through the chart ST like an item) placed by the record's world
// transform. Returns, per record, the scene instance index it became (None for item records, which the scene already holds).

pub fn add_record_geometry(scene: &mut crate::geometry::Scene, store: &mut mapgeom::store::DataStore, mr: &MapRecords, collection: &str, zone: &str, tile_y: f32, yoff: f32) -> Result<Vec<Option<usize>>, String> {
    let mut model_of: std::collections::HashMap<(String, usize, Option<String>), usize> = Default::default();
    let mut n_modified = 0usize;
    let mut out: Vec<Option<usize>> = vec![None; mr.recs.len()];
    let mut next_item = scene.item_count;
    let mut n_ent = 0usize;
    let mut n_tiles = 0usize;
    // the zone tile: the ground zone prefab's first static object entity (records::zone_tiles' model)
    let mut tile_model: Option<usize> = None;
    let mut ti = 0usize;
    // LMTOOL_RECORD_SCENE_SKIP_CLASS=a,b (study): record classes left out of the peel geometry (e.g. clipB — the down-facing
    // Base_FCB plane at a WaterBase block's top, which occludes the water surface below in our peel)
    let skip_classes: Vec<String> = std::env::var("LMTOOL_RECORD_SCENE_SKIP_CLASS").map(|v| v.split(',').map(|t| t.trim().to_string()).collect()).unwrap_or_default();
    for (k, r) in mr.recs.iter().enumerate() {
        if skip_classes.iter().any(|c| c == r.class) { continue; }
        let (mi, xf) = if let Some(m) = &r.mesh {
            let key = (m.prefab.clone(), m.entity, m.modifier.clone());
            let mi = match model_of.get(&key) {
                Some(&mi) => mi,
                None => {
                    let pm = store.load_model(&m.prefab)?;
                    let pf = mapgeom::static_item::prefab::CPlugPrefab::from_model(&pm)?;
                    let Some(e) = pf.ents.get(m.entity) else { continue };
                    let Some(mapgeom::static_item::Node::StaticObject(so)) = e.model.inline.as_deref() else { continue };
                    let Some(s2) = so.solid2() else { continue };
                    let mut g = crate::geometry::geom_from_solid2_ext(s2, None, std::env::var_os("LMTOOL_RECORD_SCENE_SKIP_NO_LMUV").is_some(), Some((store, &pm.externals)));
                    // THE MATERIAL MODIFIER (G2, 2026-09-28): the owner block's `XToY` substitution on the prefab's material links — the
                    // pre-pass constant, the bounce albedo and the material class then follow the substituted material (stpad: the
                    // WaterBase clips' TrackWall → DecoCliff, the game's f4468 draws; LMTOOL_NO_MATERIAL_MODIFIER=1 = the study's off switch)
                    if let Some(md) = m.modifier.as_deref().filter(|_| std::env::var_os("LMTOOL_NO_MATERIAL_MODIFIER").is_none()) {
                        let n = apply_material_modifier(store, md, &mut g.mat_links);
                        if n > 0 { for (k, l) in g.mat_links.iter().enumerate() { g.mat_albedo[k] = crate::albedo::for_link(l).unwrap_or([f32::NAN; 3]); } n_modified += n; }
                    }
                    scene.models.push(g);
                    scene.model_names.push(match &m.modifier { Some(md) => format!("{}#{}@{}", m.prefab, m.entity, md.rsplit('\\').next().unwrap_or(md).trim_end_matches(".Gbx")), None => format!("{}#{}", m.prefab, m.entity) });
                    model_of.insert(key, scene.models.len() - 1);
                    scene.models.len() - 1
                }
            };
            n_ent += 1;
            (mi, m.xf)
        } else if r.class == "tile" {
            let (cx, cz) = mr.tile_cells[ti]; ti += 1;
            let mi = match tile_model {
                Some(mi) => mi,
                None => {
                    let mut found: Option<usize> = None;
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
                            let g = crate::geometry::geom_from_solid2_ext(s2, None, std::env::var_os("LMTOOL_RECORD_SCENE_SKIP_NO_LMUV").is_some(), Some((store, &pm.externals)));
                            scene.models.push(g);
                            scene.model_names.push(format!("{pp}#tile"));
                            found = Some(scene.models.len() - 1);
                            break;
                        }
                        if found.is_some() { break; }
                    }
                    let Some(mi) = found else { return Err(format!("{collection}/{zone}: no zone tile static object")) };
                    tile_model = Some(mi);
                    mi
                }
            };
            n_tiles += 1;
            let mut xf = mapgeom::geom::IDENTITY;
            xf[9] = cx as f32 * 32.0; xf[10] = tile_y * 8.0 + yoff; xf[11] = cz as f32 * 32.0;
            (mi, xf)
        } else {
            continue;
        };
        let pose = crate::geometry::ItemPose { yaw: 0.0, pitch: 0.0, roll: 0.0, pos: [xf[9], xf[10], xf[11]], pivot: [0.0; 3], scale: 1.0 };
        scene.instances.push(crate::geometry::Instance { item: next_item, model: mi, xf, model_name: scene.model_names[mi].clone(), pose, lm_quality: 0, colour: 0 });
        out[k] = Some(scene.instances.len() - 1);
        next_item += 1;
    }
    eprintln!("record scene: {n_ent} prefab entity instances + {n_tiles} zone tile instances added ({} models now, {} triangles){}", scene.models.len(), scene.tri_count(), if n_modified > 0 { format!("; material modifiers: {n_modified} link(s) substituted (the owner blocks' XToY, e.g. TrackWallToDecoCliff)") } else { String::new() });
    Ok(out)
}
