//! The DISCARD REPORT (2026-10-01, vjeux: "Can you inventory all the things we
//! discard and fix all of them?" — "why do you allow the map to not generate
//! some items and consider it okay?"): one row per element of the source a
//! build DROPS, HIDES, SUBSTITUTES, leaves UNSCALED, or fails to convert —
//! whatever the reason — so that nothing the converter loses is lost silently.
//!
//! Every stage that decides a loss pushes a row here: `mapgeom tiny-library`
//! (the model decisions, the bake notes), `tmmaps tiny` (the placement side:
//! hidden tiles, parked items, the MediaTracker, the ghost), `tinyctl build` /
//! `pipeline` (the size ladder). The rows go to `<prefix>-<stage>.tsv` under
//! `TINY_DISCARD_REPORT=<prefix>` (each stage truncates its own file), and
//! `tinyctl build` concatenates them into `<out>/discard.tsv`.
//!
//! A discard that does not report is a bug of its own: a row carries the
//! stage, a CODE (the rule that decided it), the LOSS KIND the rule is
//! believed to have (`none` / `visible` / `behavioral` / `subst` / `fail`;
//! `loss_of` is the table — the inventory's verdicts refine it), the model or
//! block name, the cell(s), the placement count, and the reason in words.

use std::fmt::Write as _;
use std::path::{Path, PathBuf};

pub const HEADER: &str = "map\tscale\tstage\tcode\tloss\tname\tcells\tcount\treason";

/// The loss kind a code is believed to have, before the inventory's verdict:
/// * `none` — the game draws nothing there either (an editor helper, an
///   ambient zone the genealogy regenerates, a terrain tile under a deck);
/// * `visible` — geometry the original shows is missing, wrong-sized or
///   substituted by something that does not look the same;
/// * `behavioral` — physics / water / moving parts / animation / triggers /
///   cameras differ while the still picture is right;
/// * `subst` — a stand-in that is the game's own equivalent at the scale (a
///   stock half-size twin), believed faithful;
/// * `fail` — the model could not be built at all (the placement is refused,
///   kept unscaled, or silently skipped).
pub fn loss_of(code: &str) -> &'static str {
    match code {
        // ---- library: block plans
        "BLOCK_AMBIENT_ZONE" | "BLOCK_GRASS_FLOOR" | "BLOCK_EMPTY_PREFAB" | "CUSTOM_BLOCK_EMPTY" => "none",
        // an intentionally empty variant: nothing of its own — but a DecoWall /
        // PlatformBase family block is DRAWN by its generated fillers, so the loss
        // is none only if those fillers were converted (the hard gate's rule)
        "BLOCK_EMPTY_VARIANT" => "none",
        "BLOCK_REFUSED" | "BLOCK_BAKE_FAILED" | "BLOCK_NO_VISUALS" | "BLOCK_NO_INFO" | "CUSTOM_BLOCK_FAILED" | "BAKED_NO_MODEL" | "BLOCK_NO_MODEL" => "fail",
        // ---- library: bake notes (per entity / visual)
        "ENTITY_CLASS_UNREADABLE" | "ENTITY_EXTERNAL_FAILED" | "ENTITY_EXTERNAL_SKIPPED" | "FILLER_FOLIAGE_LOST" | "PREFAB_VEGET_LOST" | "VISUAL_LOD_PICK_SKIPPED" | "LIGHT_DROPPED" | "TRIGGERFX_CURTAIN_DROPPED" | "SCREEN_DARKENED" => "visible",
        "VISUAL_ID_PASS_DROPPED" | "VISUAL_WATER_DROPPED" | "SPAWN_SECOND_IGNORED" | "LOD_LADDER_KEPT" => "none",
        "DYNA_AT_REST" | "TWEEN_AT_REST" | "VISUAL_FRAME0_ONLY" | "FX_EMITTER_LEFT_OUT" | "TRIGGER_SKIPPED" | "SPECIAL_TRIGGER_SKIPPED" | "CONSTRAINT_SKIPPED" | "COLLISION_SKIPPED" | "LIGHT_ANIMATED_OFF" | "WATER_PHYSICS_NONE" | "WATER_TRIANGLES_REMOVED" => "behavioral",
        "NOTE_OTHER_SKIP" => "visible",
        // ---- library: trees
        "TREE_STOCK_SMALL" | "TREE_HULLLESS_STOCK" => "subst",
        "TREE_BAKE_FAILED" | "TREE_NO_MODEL" => "fail",
        "TREE_CLEARED" => "visible",
        // ---- library: items
        "ITEM_STOCK_TWIN" => "subst",
        "ITEM_CLUSTER_SPLIT" => "subst",
        "ITEM_VEGET_SUBST" | "ITEM_VEGET_KEPT_UNSCALED" | "ITEM_VEGET_DROPPED" | "ITEM_VEGET_FULLSIZE" => "visible",
        "ITEM_FAILED" | "ITEM_NO_VISUALS" | "ITEM_LIGHT_SKIN_UNKNOWN" | "ITEM_NO_MAPPING" => "fail",
        "FLAG_STILL" | "FLAG_DRIVER_HACK" => "behavioral",
        // ---- library: mapping stage
        // ghost-flagged (bit 28) generated records: DRAWN by the game — the 12 fixer's
        // lightmap oracle charts 577 of them on Fall 12; 668 of Egypt 21's 917 ghost-flagged
        // pillar walls hang on PLAIN owners (2026-10-01: vjeux's "missing wall under
        // elevated blocks"); the Summer 15 pool-rim case is the oracle's to decide per record
        "BAKED_GHOST_CLIP" => "visible",
        "SEA_FOUNDATION_KEPT" => "none",
        "BAKED_OCCUPIED_PROBE" | "KNOB_DROP_BAKED" | "KNOB_DROP_ITEMS" => "visible",
        // ---- place (tmmaps tiny)
        "TILE_HIDDEN_UNDER_BLOCK" | "TILE_HIDDEN_UNDER_BAKED" | "ARCHIVE_PRUNED" | "LIGHTMAP_STALE_KEPT" | "GENEALOGY_FILL" | "GENEALOGY_KEEP" => "none",
        "GENEALOGY_CLEAR" => "visible",
        "BAKED_NO_MAPPING_SKIPPED" | "ITEM_CARRIED_ORIGINAL" => "fail",
        "ITEM_PARKED" => "visible",
        "MT_BLOCK_VERBATIM" | "MT_UNTOUCHED" | "GHOST_REMOVED" | "MT_TRIGGER_GRID" => "behavioral",
        // ---- pipeline
        "MAP_LOD_LADDER" => "visible",
        "MAP_OVER_CAP" => "fail",
        _ => "visible",
    }
}

#[derive(Clone, Debug)]
pub struct Row {
    pub stage: &'static str,
    pub code: &'static str,
    pub name: String,
    pub cells: String,
    pub count: usize,
    pub reason: String,
}

pub struct Discards {
    pub map: String,
    pub scale: f32,
    pub stage: &'static str,
    pub rows: Vec<Row>,
}

/// `x,y,z` of a file cell.
pub fn cell_str(c: [u8; 3]) -> String {
    format!("{},{},{}", c[0], c[1], c[2])
}

/// Up to `max` cells joined by `;`, then `…+N`.
pub fn cells_str(cells: &[[u8; 3]], max: usize) -> String {
    let mut s: Vec<String> = cells.iter().take(max).map(|c| cell_str(*c)).collect();
    if cells.len() > max {
        s.push(format!("…+{}", cells.len() - max));
    }
    s.join(";")
}

fn clean(s: &str) -> String {
    s.replace(['\t', '\n', '\r'], " ")
}

impl Discards {
    pub fn new(map: &Path, scale: f32, stage: &'static str) -> Discards {
        Discards { map: map.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default(), scale, stage, rows: Vec::new() }
    }

    pub fn push(&mut self, code: &'static str, name: &str, cells: &str, count: usize, reason: &str) {
        self.rows.push(Row { stage: self.stage, code, name: clean(name), cells: clean(cells), count, reason: clean(reason) });
    }

    /// The report path of this stage under `TINY_DISCARD_REPORT=<prefix>`
    /// (`<prefix>-<stage>.tsv`), or `None` when no report was asked for.
    pub fn path_from_env(stage: &str) -> Option<PathBuf> {
        let prefix = std::env::var("TINY_DISCARD_REPORT").ok().filter(|p| !p.is_empty() && p != "0")?;
        Some(PathBuf::from(format!("{prefix}-{stage}.tsv")))
    }

    pub fn render(&self) -> String {
        let mut out = String::new();
        out.push_str(HEADER);
        out.push('\n');
        for r in &self.rows {
            let _ = writeln!(out, "{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}", self.map, self.scale, r.stage, r.code, loss_of(r.code), r.name, r.cells, r.count, r.reason);
        }
        out
    }

    /// Writes the stage's file (truncating) when a report was asked for;
    /// returns the path written.
    pub fn write_env(&self) -> Option<PathBuf> {
        let p = Self::path_from_env(self.stage)?;
        if let Some(d) = p.parent() {
            let _ = std::fs::create_dir_all(d);
        }
        std::fs::write(&p, self.render()).unwrap_or_else(|e| panic!("{}: {e}", p.display()));
        Some(p)
    }

    pub fn write_to(&self, p: &Path) {
        std::fs::write(p, self.render()).unwrap_or_else(|e| panic!("{}: {e}", p.display()));
    }
}

/// The code a bake NOTE of `mapgeom static_item` falls under, when it records a
/// loss (an entity, visual, light or trigger the item does not carry): `None`
/// for a note that is information only. Every pattern the baker's notes use
/// for a skip is listed; a skip-like note none of them match is
/// `NOTE_OTHER_SKIP`, never dropped from the report.
pub fn note_code(note: &str) -> Option<&'static str> {
    let n = note;
    let has = |s: &str| n.contains(s);
    if has("model class 0x") && has("skipped") {
        return Some("ENTITY_CLASS_UNREADABLE");
    }
    if has("skipped (vegetation, re-emitted as an item)") || has("re-emitted as items") || has("filler foliage") && has("inlined into the item") || has("of ") && has("filler foliage entities inlined") {
        return None;
    }
    if has("filler foliage") && (has("not inlined") || has("not transformed")) {
        return Some("FILLER_FOLIAGE_LOST");
    }
    if has("moving part") && has("baked at rest") {
        return Some("DYNA_AT_REST");
    }
    if has("tween part") && has("baked at rest") || has("strip part") && has("baked at rest") {
        return Some("TWEEN_AT_REST");
    }
    if has("left out (the game drops an item whose FX entity has an emitter") {
        return Some("FX_EMITTER_LEFT_OUT");
    }
    if has("trigger FX curtain visual") && has("dropped") {
        return Some("TRIGGERFX_CURTAIN_DROPPED");
    }
    if has("special trigger") && (has("skipped") || has("failed") || has("not emitted")) {
        return Some("SPECIAL_TRIGGER_SKIPPED");
    }
    if has("gameplay gate") && has("not emitted") || has("gameplay trigger shape") && (has("no triangles") || has(": ")) && !has("prefab form") {
        return Some("SPECIAL_TRIGGER_SKIPPED");
    }
    if (has("trigger shape") || has("waypoint trigger")) && (has("skipped") || has("failed") || has("next") || has("unit box")) {
        return Some("TRIGGER_SKIPPED");
    }
    if has("constraint") && (has("skipped") || has("does not have")) {
        return Some("CONSTRAINT_SKIPPED");
    }
    if has("external") && has("failed") {
        return Some("ENTITY_EXTERNAL_FAILED");
    }
    if has("external") && has("skipped") {
        return Some("ENTITY_EXTERNAL_SKIPPED");
    }
    if has("second spawn point") && has("ignored") {
        return Some("SPAWN_SECOND_IGNORED");
    }
    if has("visual keeps frame 0") {
        return Some("VISUAL_FRAME0_ONLY");
    }
    if has("id-pass visual") && has("dropped") {
        return Some("VISUAL_ID_PASS_DROPPED");
    }
    if has("water surface visual dropped") {
        return Some("VISUAL_WATER_DROPPED");
    }
    if has("skipped: not level") {
        return Some("VISUAL_LOD_PICK_SKIPPED");
    }
    if has("lod ladder:") {
        return None;
    }
    if has("vegetation bake: level") && has("alone") {
        return Some("VISUAL_LOD_PICK_SKIPPED");
    }
    if has("animated light") && has("not embedded") || has("gameplay-gate light not embedded") {
        return Some("LIGHT_ANIMATED_OFF");
    }
    if has("light") && (has("dropped") || has("not embedded") || has("not carried over")) {
        return Some("LIGHT_DROPPED");
    }
    if has("ad screen face") || has("TINY_SCREENS=dark") {
        return Some("SCREEN_DARKENED");
    }
    if has("Water-physics collision triangles kept at the pack's physics 13") || has("flagged NotCollidable") {
        return Some("WATER_PHYSICS_NONE");
    }
    if has("Water-physics collision triangles removed") {
        return Some("WATER_TRIANGLES_REMOVED");
    }
    if has("collision surf type") && has("skipped") {
        return Some("COLLISION_SKIPPED");
    }
    if has("static part skipped (strip item") {
        return None;
    }
    if has("skipped") || has("failed") || has("dropped") || has("left out") || has("ignored") || has("unnamed") || has("unresolved") || has("not emitted") {
        return Some("NOTE_OTHER_SKIP");
    }
    None
}

/// Rows from a bake's notes: one per DISTINCT loss note, with the placement
/// count of the model it belongs to.
pub fn push_notes(d: &mut Discards, notes: &[String], name: &str, cells: &str, count: usize) {
    let mut seen: std::collections::BTreeSet<&str> = Default::default();
    for n in notes {
        if !seen.insert(n.as_str()) {
            continue;
        }
        if let Some(code) = note_code(n) {
            d.push(code, name, cells, count, n);
        }
    }
}

/// The class × map table of a set of per-map reports (`tinyctl discard-table`):
/// rows = codes (with their loss kind), columns = maps, cells = the summed
/// placement counts; plus a per-map total.
pub fn table(reports: &[(String, String)]) -> String {
    use std::collections::{BTreeMap, BTreeSet};
    let mut maps: Vec<String> = Vec::new();
    let mut codes: BTreeSet<String> = BTreeSet::new();
    let mut counts: BTreeMap<(String, String), usize> = BTreeMap::new();
    let mut rows_n: BTreeMap<(String, String), usize> = BTreeMap::new();
    for (label, text) in reports {
        maps.push(label.clone());
        for line in text.lines().skip(1) {
            let f: Vec<&str> = line.split('\t').collect();
            if f.len() < 9 {
                continue;
            }
            let code = f[3].to_string();
            let n: usize = f[7].parse().unwrap_or(0);
            codes.insert(code.clone());
            *counts.entry((code.clone(), label.clone())).or_default() += n;
            *rows_n.entry((code, label.clone())).or_default() += 1;
        }
    }
    let mut out = String::new();
    out.push_str("code\tloss");
    for m in &maps {
        let _ = write!(out, "\t{m}");
    }
    out.push_str("\ttotal\n");
    for code in &codes {
        let _ = write!(out, "{code}\t{}", loss_of(code));
        let mut total = 0usize;
        for m in &maps {
            let n = counts.get(&(code.clone(), m.clone())).copied().unwrap_or(0);
            total += n;
            if n == 0 {
                out.push_str("\t·");
            } else {
                let _ = write!(out, "\t{n}");
            }
        }
        let _ = writeln!(out, "\t{total}");
    }
    out
}
