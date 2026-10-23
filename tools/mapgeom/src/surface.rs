//! Surface swap plan: which block MODEL of a map has a same-footprint twin of
//! another surface (`RoadTechStraight` -> `RoadIceStraight`), verified in the
//! pack (2026-09-13, vjeux: "input 'ice' and it'll swap all the road, dirt...
//! blocks into their icy version. If one doesn't exist it'll create a custom
//! item version").
//!
//! Stadium spells a block's surface as ONE WORD inside its name, in a place
//! that depends on the family:
//!
//! | family | Tech form | Ice form | rule |
//! |---|---|---|---|
//! | `Road…` | `RoadTechStraight` | `RoadIceStraight` | the word after `Road` (Tech, Dirt, Bump, Water, Ice) |
//! | `Platform…` | `PlatformTechBase` | `PlatformIceBase` | the word after `Platform` (Tech, Dirt, Grass, Plastic, Ice) |
//! | `DecoPlatform…` | `DecoPlatformBase` | `DecoPlatformIceBase` | Tech has NO word: insert / replace after `DecoPlatform` |
//! | `DecoHill…` | `DecoHillSlope2Straight` | `DecoHillIceSlope2Straight` | same, after `DecoHill` |
//! | `Open…Road`/`Hills`/`Zone` | `OpenTechRoadCurve1` | `OpenIceRoadCurve1` | the word after `Open` |
//! | `DecoWall…` | `DecoWallBase` | `DecoWallBaseIce` | SUFFIX (pillars excepted) |
//! | `Water…` | `WaterBase` | `WaterBaseIce` | SUFFIX — the pool keeps its water, the rim is dressed with the `PlatformIce` modifier |
//! | `Road<A>ToRoad<B>` | `RoadTechToRoadDirt` | `RoadIceStraight` | a one-cell join of two surfaces is a straight of the one surface |
//!
//! A candidate is only a twin when the pack HAS it and it agrees with the
//! source model on kind, waypoint type and the footprint (unit cells) of the
//! base ground and air variants; the map's records are checked against the
//! twin's variant tables too (ground/air, additional-variant index, mobil list
//! and mobil index) so a rename never asks for a shape the twin lacks.
//!
//! What has NO twin in the 2026 Stadium pack (measured on the first 25 u10s
//! maps): every `TrackWall*` (the wooden wall-ride family), `RoadIce` has no
//! `Tilt*`, `Wave*`, `RampLow` pieces, and `DecoWallWaterBase` has no ice
//! form. Those rows come back as `none`; the map side decides between keeping
//! the block and baking a re-dressed item (`tmmaps surface`).

use crate::blockinfo::BlockInfo;
use crate::blockmap::BlockInfoIndex;
use crate::store::DataStore;
use std::collections::{BTreeMap, HashMap};
use std::path::Path;

/// The surface words Stadium block names carry, longest first so `Plastic`
/// wins over a hypothetical `Plas`.
pub const SURFACE_WORDS: &[&str] = &["Plastic", "Magnet", "Grass", "Water", "Tech", "Dirt", "Bump", "Wood", "Ice"];

/// The target surfaces that have NO block family of their own in the 2026
/// Stadium pack (`PlatformWood…`, `RoadMagnet…` do not exist): every deck
/// block of theirs is a baked re-dressed copy, and the terrain-like families
/// (deco platforms, hills, deco walls, pools) are left as they are — the
/// Altered Nadeo convention (their "Wood" sets re-dress roads and platforms;
/// `OpenIceZone…-Wood` keeps its snow hill). Plastic has platform twins only.
pub fn deck_only(s: &str) -> bool {
    matches!(s, "Wood" | "Magnet" | "Plastic")
}

/// The families a `deck_only` surface leaves alone.
fn is_terrain_family(name: &str) -> bool {
    ["DecoPlatform", "DecoHill", "DecoWall", "Water"].iter().any(|p| name.starts_with(p))
}

/// Canonical spelling of a surface argument (`ice` -> `Ice`).
pub fn surface_word(s: &str) -> Option<&'static str> {
    parse_surface(s).map(|(w, _)| w)
}

/// `(surface word, wet)`: a `wet` prefix or suffix (`wetwood`, `woodwet`,
/// `wood+wet`, `wet-wood`) asks for the WET form of a surface. The wet form
/// keeps the surface's plan and turns the START block into its Water twin
/// (`RoadWaterStart`, `PlatformWaterStart`: the same deck at 2.0 m under a
/// shallow water volume), so the car spawns in water and carries wet tyres
/// for the whole run — the mechanism of the Altered Nadeo "Wet …" maps
/// (Fall 2024-25 "Pure Wet Icy Wood": a `PlatformWaterRampBase` under the
/// spawn; its WR's telemetry reads wetness 0 → 1.0 over the first 1.5 s and
/// 1.0 for the remaining 94 s over dry wood, ice and road — TM2020 tyres do
/// not dry).
pub fn parse_surface(s: &str) -> Option<(&'static str, bool)> {
    let mut l = s.trim().to_ascii_lowercase();
    for sep in ["+", "-", "_", " "] {
        l = l.replace(sep, "");
    }
    let mut wet = false;
    if let Some(rest) = l.strip_prefix("wet") {
        l = rest.to_string();
        wet = true;
    } else if let Some(rest) = l.strip_suffix("wet") {
        l = rest.to_string();
        wet = true;
    }
    SURFACE_WORDS.iter().find(|w| w.to_ascii_lowercase() == l).copied().map(|w| (w, wet))
}

fn strip_surface_word(rest: &str) -> Option<(&'static str, &str)> {
    for w in SURFACE_WORDS {
        if let Some(tail) = rest.strip_prefix(w) {
            return Some((w, tail));
        }
    }
    None
}

/// Whether `name` is a block of a SURFACE family at all (roads, platforms,
/// deco platforms, deco hills, open roads, deco walls, water) — the families
/// the swap touches. Pillars, structures, stands, gates, canopies are not.
pub fn is_surface_family(name: &str) -> bool {
    // ⛔ EVERY `…Pillar` stays the game's own, `TrackWall…Pillar` included. The Sept 18
    // carve-out (its prefab is the wall ride's, so it looked like drivable surface) was
    // wrong in practice: on U10S 23 those 47 blocks stack vertically under the track —
    // support columns — and converting them recoloured the wood. Everios96 2026-10-08,
    // asked explicitly: "yes they should stay wood".
    if name.ends_with("Pillar") && !name.starts_with("RallyCastleRoad") {
        return false;
    }
    // the snow theme's plank roads (`SnowRoad…`, the vanilla WOOD blocks — Dyl_Chill's
    // 152/159), the rally theme's dirt roads and castle roads (dirt decks): surface
    // families too; their support pillars (`SnowRoadPillar…`) are structure, left alone
    if name.starts_with("SnowRoadPillar") {
        return false;
    }
    // the concert STAGE decks (metal `TechnicsTrims` floors players drive on — Everios96's
    // own wood 39 gave them a wood deck; GeraldDuke's wet wood 39): the deck pieces, not
    // the light rigs or the supports
    if is_stage_deck(name) {
        return true;
    }
    // the black structure road (`StructureBase`, `Structure.Material`, physics 22):
    // scenery in most maps, a precision-driving surface in some — opt-in per build
    // (TINY_STRUCTURE_BASE=1) for the maps whose record ghosts drive on it
    // (Everios96 2026-09-19 19:14 "do the same for all the black road blocks")
    if is_structure_base(name) {
        return true;
    }
    ["Road", "Platform", "DecoPlatform", "DecoHill", "Open", "DecoWall", "Water", "TrackWall", "SnowRoad", "RallyRoadDirt", "RallyCastleRoad"].iter().any(|p| name.starts_with(p))
}

/// A stage block whose top is a driving deck: `Stage<Shape>` and `StageStructure<Shape>`
/// (the stage on its truss); `StageSupport*` (pillars) and `StageTechnicsLight*` (rigs) are not.
pub fn is_structure_base(name: &str) -> bool {
    name == "StructureBase" && std::env::var("TINY_STRUCTURE_BASE").map(|v| v == "1").unwrap_or(false)
}

pub fn is_stage_deck(name: &str) -> bool {
    // opt-in per build (TINY_STAGE_DECKS=1): the stage is scenery far more often than
    // a road — Everios96 picks the maps where it is driven (2026-09-18 22:32 PT)
    std::env::var("TINY_STAGE_DECKS").map(|v| v == "1").unwrap_or(false) && name.starts_with("Stage") && !name.starts_with("StageSupport") && !name.starts_with("StageTechnicsLight")
}

/// The name a block WOULD have with surface `s`, by the family rules above —
/// unverified. `None` for a block outside the surface families (pillars,
/// structures, gates …). The result may equal `name` (already that surface).
pub fn candidate(name: &str, s: &str) -> Option<String> {
    if name.ends_with("Pillar") && !name.starts_with("RallyCastleRoad") {
        return None;
    }
    if name.starts_with("SnowRoadPillar") {
        return None;
    }
    // no family of these exists in another surface: the candidate is spelled so the
    // verification says so in the row, and the block is baked as a re-dressed copy
    // (its deck stems — ThemeSnowRoad, RoadDirt, RoadDirtFullColorize — are swap rows)
    if name.starts_with("SnowRoad") || name.starts_with("RallyRoadDirt") || name.starts_with("RallyCastleRoad") || is_stage_deck(name) || is_structure_base(name) {
        return Some(format!("{name}{s}"));
    }
    // Road<A>ToRoad<B>[Down|SlopeUp|…]: the join of two road surfaces. In a
    // one-surface world the flat join is a straight; a stepped/sloped join
    // (`RoadTechToRoadIceDown`) has no one-surface counterpart — verified
    // later against the pack (it will not exist) and reported as `none`.
    if let Some(rest) = name.strip_prefix("Road") {
        if let Some((_a, tail)) = strip_surface_word(rest) {
            if let Some(after_to) = tail.strip_prefix("ToRoad") {
                if let Some((b, tail2)) = strip_surface_word(after_to) {
                    if _a == s && b == s {
                        return Some(name.to_string()); // both halves already that surface
                    }
                    return Some(if tail2.is_empty() { format!("Road{s}Straight") } else { format!("Road{s}Straight{tail2}") });
                }
            }
            return Some(format!("Road{s}{tail}"));
        }
        return None; // `Road` + something that is not a surface word
    }
    for fam in ["DecoPlatform", "DecoHill", "Platform"] {
        if let Some(rest) = name.strip_prefix(fam) {
            // Tech is spelled with NO word in the Deco families — and the Tech
            // deco platform / deco hill IS the grass-topped one (`Deco` /
            // `DecoHill` materials, Grass 2), so Grass spells them the same way
            let word = if fam != "Platform" && (s == "Tech" || s == "Grass") { "" } else { s };
            return Some(match strip_surface_word(rest) {
                Some((_w, tail)) => format!("{fam}{word}{tail}"),
                None if fam != "Platform" => format!("{fam}{word}{rest}"),
                None => return None,
            });
        }
    }
    if let Some(rest) = name.strip_prefix("Open") {
        if let Some((_w, tail)) = strip_surface_word(rest) {
            return Some(format!("Open{s}{tail}"));
        }
        return None;
    }
    // `Water<S>Zone…`, `Water<S>RampRoad…`, `Water<S>StraightToWaterWall`: the
    // pool-edge family spells the surface right after `Water` (WaterGrassZoneStraight
    // → WaterIceZoneStraight, the same `WaterGrass\ZoneStraight_Air` prefab under the
    // PlatformIce modifier); `WaterBase` / `WaterWall…` / `WaterRampZone…` take a SUFFIX
    if let Some(rest) = name.strip_prefix("Water") {
        if let Some((_w, tail)) = strip_surface_word(rest) {
            if !tail.is_empty() {
                return Some(format!("Water{s}{tail}"));
            }
        }
    }
    if name.starts_with("DecoWall") || name.starts_with("Water") {
        // suffix form; an existing suffix of another surface is replaced
        for w in SURFACE_WORDS {
            if let Some(base) = name.strip_suffix(w) {
                if !base.is_empty() && base != "DecoWall" && base != "Water" {
                    return Some(format!("{base}{s}"));
                }
            }
        }
        return Some(format!("{name}{s}"));
    }
    if name.starts_with("TrackWall") {
        // the wooden wall-ride family has no surface twins at all; the
        // candidate is spelled so the verification says so in the row
        return Some(format!("{name}{s}"));
    }
    None
}

/// What the plan says about one block model.
#[derive(Clone, Debug, PartialEq)]
pub enum Action {
    /// rename to the verified twin
    Rename,
    /// already of the asked-for surface
    Same,
    /// outside the surface families (pillars, structures, gates …)
    Keep,
    /// a surface block whose twin the pack does not have
    None_,
    /// a twin exists but disagrees (footprint, waypoint, variant tables)
    Mismatch,
    /// a surface block with no same-shape twin: baked as a re-dressed ITEM copy
    /// (`tmmaps surface --library --mapping`)
    Bake,
    /// a WATER block kept as the game's block (its volume) PLUS a visual-only re-dressed
    /// item copy of everything but its water (Everios96 2026-09-24: "change the plastic parts
    /// of those blocks to something else without losing the water physics"); TINY_WATER_OVERLAY=1
    Overlay,
}

impl Action {
    pub fn as_str(&self) -> &'static str {
        match self {
            Action::Rename => "rename",
            Action::Same => "same",
            Action::Keep => "keep",
            Action::None_ => "none",
            Action::Mismatch => "mismatch",
            Action::Bake => "bake",
            Action::Overlay => "overlay",
        }
    }
    pub fn parse(s: &str) -> Option<Action> {
        Some(match s {
            "rename" => Action::Rename,
            "same" => Action::Same,
            "keep" => Action::Keep,
            "none" => Action::None_,
            "mismatch" => Action::Mismatch,
            "bake" => Action::Bake,
            "overlay" => Action::Overlay,
            _ => return None,
        })
    }
}

#[derive(Clone, Debug)]
pub struct PlanRow {
    pub name: String,
    pub count: usize,
    pub action: Action,
    pub target: String,
    /// deck offset in cells: the twin is placed `dy` cells LOWER than the source
    /// record so its deck lands where the source deck was (RoadIce: 1)
    pub dy: i32,
    pub detail: String,
    /// record flags the twin cannot resolve as they are, and the flags it takes
    /// instead: the Tech special pads have an `add0` (no-pillar) variant the
    /// Ice/Grass/Plastic/Dirt twins do not — the same 1-unit pad under
    /// `air/base`. Until 2026-09-14 the model was a Mismatch and 337 boosters
    /// per set stayed Tech (boasy: "ice is ice, but the boosters are not ice").
    pub flag_rewrites: Vec<(u32, u32)>,
}

fn cells_of(v: &Option<crate::blockinfo::Variant>) -> Vec<[i32; 3]> {
    let mut c = v.as_ref().map(|v| v.cells()).unwrap_or_default();
    c.sort();
    c
}

/// Compare a source model with its candidate twin: kind, waypoint, footprint
/// of the base ground and air variants, spawn of a start block. The deck
/// offset `dy` (cells; the twin's deck sits `8·dy` m higher in its own frame,
/// RoadIce = 1) relaxes the footprint rule to "same x/z columns": the ice
/// road's extra unit is its bank, and the record is placed `dy` cells lower.
/// `Ok(detail)` when they agree, `Err(why)` otherwise.
fn compare(src: &BlockInfo, twin: &BlockInfo, dy: i32) -> Result<String, String> {
    if src.kind != twin.kind {
        return Err(format!("kind {:?} vs {:?}", src.kind, twin.kind));
    }
    if src.waypoint_type != twin.waypoint_type {
        return Err(format!("waypoint {:?} vs {:?}", src.waypoint_type, twin.waypoint_type));
    }
    let columns = |c: &[[i32; 3]]| -> Vec<[i32; 2]> {
        let mut v: Vec<[i32; 2]> = c.iter().map(|c| [c[0], c[2]]).collect();
        v.sort();
        v.dedup();
        v
    };
    let (sg, tg) = (cells_of(&src.variant_base_ground), cells_of(&twin.variant_base_ground));
    let (sa, ta) = (cells_of(&src.variant_base_air), cells_of(&twin.variant_base_air));
    let mut detail;
    if sg == tg && sa == ta {
        detail = format!("cells {}", if !sg.is_empty() { sg.len() } else { sa.len() });
        if dy != 0 {
            detail.push_str(&format!("; deck offset dy {dy} with an equal footprint"));
        }
    } else if dy != 0 && columns(&sg) == columns(&tg) && columns(&sa) == columns(&ta) {
        detail = format!("deck +{} m (dy {dy}): {} -> {} cells over the same columns", 8 * dy, sg.len().max(sa.len()), tg.len().max(ta.len()));
    } else if sg != tg {
        return Err(format!("ground footprint {:?} vs {:?}", sg, tg));
    } else {
        return Err(format!("air footprint {:?} vs {:?}", sa, ta));
    }
    if src.waypoint_type == Some(0) || src.waypoint_type == Some(4) {
        let s = src.variant_base_ground.as_ref().or(src.variant_base_air.as_ref()).map(|v| v.spawn_loc).unwrap_or_default();
        let t = twin.variant_base_ground.as_ref().or(twin.variant_base_air.as_ref()).map(|v| v.spawn_loc).unwrap_or_default();
        let shifted = [t[0], t[1] - 8.0 * dy as f32, t[2]];
        if [s[0], s[1], s[2]] != shifted {
            detail.push_str(&format!("; spawn {:?} -> {:?} (in the source's frame)", &s[..3], shifted));
        } else {
            detail.push_str(&format!("; spawn {:?}", &s[..3]));
        }
    }
    if src.additional_ground.len() != twin.additional_ground.len() || src.additional_air.len() != twin.additional_air.len() {
        detail.push_str(&format!(
            "; additional variants {}g/{}a vs {}g/{}a",
            src.additional_ground.len(),
            src.additional_air.len(),
            twin.additional_ground.len(),
            twin.additional_air.len()
        ));
    }
    Ok(detail)
}

/// Whether the twin resolves a record's flags the way the source does:
/// ground/air family, the additional index (bits 21..27), the mobil list
/// (bits 0..5) and the mobil (bits 6..11) — the same variant label, the same
/// footprint, and no fallback note the source's own pick did not have.
fn record_fits(src: &BlockInfo, twin: &BlockInfo, flags: u32) -> Result<(), String> {
    let ground = flags & 0x1000 != 0;
    let add = ((flags >> 21) & 0x7f) as usize;
    let list = (flags & 63) as usize;
    let mobil = ((flags >> 6) & 63) as usize;
    let Some(t) = twin.pick_placement_add(ground, list, mobil, add) else {
        return Err(format!("flags {flags:08x}: twin has no variant (ground {ground}, add {add}, list {list}, mobil {mobil})"));
    };
    let Some(s) = src.pick_placement_add(ground, list, mobil, add) else {
        return Ok(()); // the source itself resolves nothing here; nothing to hold the twin to
    };
    if s.label != t.label {
        return Err(format!("flags {flags:08x}: variant {} vs {}", s.label, t.label));
    }
    let (mut sc, mut tc) = (s.variant.cells(), t.variant.cells());
    sc.sort();
    tc.sort();
    if sc != tc {
        return Err(format!("flags {flags:08x}: variant footprint {:?} vs {:?}", sc, tc));
    }
    if s.mobils.len() != t.mobils.len() {
        return Err(format!("flags {flags:08x}: {} vs {} mobils picked", s.mobils.len(), t.mobils.len()));
    }
    Ok(())
}

/// The flags word with the subvariant the EDITOR would pick for `twin`: when the
/// picked mobil list has exactly one mobil of frequency > 0 and the record's
/// subvariant points at a frequency-0 mobil, bits 6..11 move to the live one.
fn editor_subvariant(twin: &BlockInfo, flags: u32) -> Option<u32> {
    let ground = flags & 0x1000 != 0;
    let add = ((flags >> 21) & 0x7f) as usize;
    let list = (flags & 63) as usize;
    let mobil = ((flags >> 6) & 63) as usize;
    let t = twin.pick_placement_add(ground, list, mobil, add)?;
    let mobils = t.variant.mobils.get(list)?;
    if mobils.len() < 2 || mobils.get(mobil).map(|m| m.solid_frequency != 0).unwrap_or(true) {
        return None;
    }
    let live: Vec<usize> = mobils.iter().enumerate().filter(|(_, m)| m.solid_frequency > 0).map(|(i, _)| i).collect();
    if live.len() != 1 {
        return None;
    }
    Some((flags & !(63 << 6)) | ((live[0] as u32) << 6))
}

/// Build the plan over the authored blocks of `maps` (aggregated by name).
pub fn plan(store: &mut DataStore, idx: &mut BlockInfoIndex, maps: &[std::path::PathBuf], surface: &str) -> Result<Vec<PlanRow>, String> {
    let (s, wet) = parse_surface(surface).ok_or_else(|| format!("unknown surface `{surface}` (one of {}, optionally with `wet`)", SURFACE_WORDS.join(", ")))?;
    // name -> (count, flags seen)
    let mut seen: BTreeMap<String, (usize, Vec<u32>)> = BTreeMap::new();
    for p in maps {
        let m = tmmaps::map::MapFile::try_load(p).map_err(|e| format!("{}: {e}", p.display()))?;
        for b in &m.blocks {
            let e = seen.entry(b.name.clone()).or_default();
            e.0 += 1;
            if !e.1.contains(&b.flags) {
                e.1.push(b.flags);
            }
        }
    }
    let mut rows = Vec::new();
    for (name, (count, flags)) in seen {
        let row = plan_one(store, idx, &name, count, &flags, s, wet);
        rows.push(row);
    }
    Ok(rows)
}

/// The plan for one block model; under a WET surface the START block's row
/// carries `wetpad deck=<h>` — the height of its driving deck in the block's
/// frame — for `tmmaps surface`, which puts a free `PlatformWaterRampBase`
/// under the start so its shallow water (a 32×2×32 volume, the sheet at +2.0)
/// stands 0.4 m over the deck: the car spawns with its wheels in water and
/// carries wet tyres for the whole run (TM2020 tyres do not dry: the Fall
/// 2024-25 "Pure Wet Icy Wood" WR reads wetness 1.0 for 94 s over dry wood
/// and ice after exactly this kind of start — a PlatformWaterRampBase under
/// the spawn; measured here: wetness 0 → 1.0 in 1.5 s of water contact, and
/// a car that left the water at 0.81 held 0.81 for the 7 s it kept falling).
/// The Water START twins were tried and refused: `RoadWaterStart` drives on
/// a floor 1 m under the tech deck (the car exits under the next block),
/// `PlatformWaterStart` renamed in place has no floor at all (the car sinks).
fn plan_one(store: &mut DataStore, idx: &mut BlockInfoIndex, name: &str, count: usize, flags: &[u32], s: &str, wet: bool) -> PlanRow {
    // WET wood: the penalty terrain (hills, the walls' grass tops) takes the SNOW form
    // like the icy set — grass and dirt dry the tyres, snow does not (GeraldDuke via
    // Everios96, 2026-09-19 19:44); the decks stay wood, the DecoPlatform tops too
    // + the DecoPlatform tops (`Material\Deco`, grass/sand): re-dressing them culled
    // the faces (a two-sided material's winding under a one-sided one: every
    // platform top invisible, Everios96 2026-09-20 06:27) — the ice twin is a real
    // block, the icy set's own penalty look
    let snow_env = std::env::var("TINY_WET_SNOW_PENALTY").map(|v| v != "0").unwrap_or(true);
    // … and (2026-09-20, Everios96 Q2 "every block with any sand or grass anywhere → snow"):
    // the POOL blocks (`Water*`: grass/sand rims, the ramp roads out of the pool) and the
    // CLIFFS (`DecoCliff*`: DecoHill grass on top) take their Ice twins too — the game's own
    // ice variants redress DecoHill/DecoHill2 → Snow, PoolBorders stay plastic as in vanilla
    let snow_penalty = wet && s == "Wood" && (name.starts_with("DecoHill") || name.starts_with("DecoWall") || name.starts_with("Water") || name.starts_with("DecoCliff")) && !name.ends_with("Pillar") && snow_env;
    // The DecoPlatform TOPS cannot take the ice twin: the game's own DecoPlatformIce*
    // blocks resolve to a NULL surface under the wheels (EPlugSurfaceMaterialId 80,
    // no penalty, no marks — measured 2026-09-20 with the VehicleState readout on the
    // original U10S_112; the ice HILLS read Snow 21 as they should). They are baked
    // instead, the top re-dressed with `Modifier\PlatformIce\Deco` at physics 21 —
    // the same look as the vanilla snow top, with the snow the vanilla block lacks.
    let snow_top_env = std::env::var("TINY_SNOW_TOP_BAKE").map(|v| v != "0").unwrap_or(true);
    let snow_top = (s == "Ice" || (wet && s == "Wood" && snow_env)) && name.starts_with("DecoPlatform") && !name.ends_with("Pillar") && snow_top_env;
    if snow_top {
        let mut r = plan_one_dry(store, idx, name, count, flags, s);
        // (Keep too: the deck-only rule leaves DecoPlatform alone since 2026-09-23 — the wet
        // wood snow top must still bake)
        if matches!(r.action, Action::Rename | Action::Same | Action::Keep) {
            r.action = Action::Bake;
            r.target = name.replacen("DecoPlatform", "DecoPlatformSnow", 1);
            r.detail.push_str("; snow top: baked with Modifier\\PlatformIce\\Deco @Snow (the ice twin's top is a null surface in the game)");
        }
        return r;
    }
    // … and an AUTHOR-PLACED ice top in any other set (vjeux 2026-09-20 14:37 "make all these
    // real snow"): the same bake, so it drives as snow instead of the game's null surface
    if snow_top_env && name.starts_with("DecoPlatformIce") && !name.ends_with("Pillar") && s != "Ice" {
        let mut r = plan_one_dry(store, idx, name, count, flags, s);
        r.action = Action::Bake;
        r.target = name.replacen("DecoPlatformIce", "DecoPlatformSnow", 1);
        r.detail = "author's ice top: baked with Modifier\\PlatformIce\\Deco @Snow (a null surface as the game's block)".to_string();
        return r;
    }
    let mut row = if snow_penalty {
        let mut r = plan_one_dry(store, idx, name, count, flags, "Ice");
        r.detail.push_str("; wet wood: snow penalty (the icy twin)");
        r
    } else {
        plan_one_dry(store, idx, name, count, flags, s)
    };
    if wet && !name.contains('\\') && !name.ends_with("_CustomBlock") {
        if let Some(p) = idx.path_for(name) {
            if let Ok(bi) = idx.load(store, &p).cloned() {
                if bi.waypoint_type == Some(0) {
                    // a start that is itself a water block (RoadWaterStart, U10S_23) wets the car already
                    let has_water = bi.variant_base_ground.as_ref().map(|v| v.water_volumes > 0).unwrap_or(false) || bi.variant_base_air.as_ref().map(|v| v.water_volumes > 0).unwrap_or(false);
                    if has_water {
                        row.detail.push_str("; wet already (a water start)");
                    } else {
                        // the height the CAR spawns at, from the variant's own spawn point —
                        // the largest flat area is not it on a sausage start (RoadBumpStart:
                        // pad plateau 2.75, spawn 4.5 → the sheet stood 1.35 m under the wheels;
                        // GeraldDuke's wet wood 35/45 "no wet tires", 2026-09-16)
                        let spawn_y = bi.variant_base_ground.as_ref().or(bi.variant_base_air.as_ref()).map(|v| v.spawn_loc[1]).filter(|y| *y > 0.0);
                        let deck = spawn_y.or_else(|| base_prefab(&bi).and_then(|pf| deck_level(store, &pf))).unwrap_or(2.0);
                        row.detail.push_str(&format!("; wetpad deck={deck}"));
                    }
                }
            }
        }
    }
    row
}


/// `Road<Surface>[Shape]Penalty[Ice|Dirt]` → (surface word, shape, floor word).
pub fn split_penalty(name: &str) -> Option<(&str, &str, &str)> {
    let rest = name.strip_prefix("Road")?;
    let (word, tail) = strip_surface_word(rest)?;
    let (shape, floor) = tail.split_once("Penalty")?;
    if !matches!(floor, "" | "Ice" | "Dirt") {
        return None;
    }
    Some((word, shape, floor))
}

fn plan_one_dry(store: &mut DataStore, idx: &mut BlockInfoIndex, name: &str, count: usize, flags: &[u32], s: &str) -> PlanRow {
    let mk = |action: Action, target: &str, detail: String| PlanRow { name: name.to_string(), count, action, target: target.to_string(), dy: 0, detail, flag_rewrites: Vec::new() };

    // The PENALTY road frames (`Road<Surface>[Shape]Penalty[Ice|Dirt]`): the prefab is
    // only the rails + border spots, the FLOOR inside is the map's terrain zone tile,
    // re-dressed by the block's terrain modifier (PenaltyIce → snow, PenaltyDirt → sand,
    // none → the zone's grass). An item copy can never carry that floor (wet wood 37,
    // GeraldDuke 2026-09-20: "road block with grass penalty"), so the frame stays a
    // BLOCK RECORD and is renamed: Ice → `Road<Ice>…PenaltyIce`; every other alteration
    // keeps its own road word (the rails are what the re-dress would change — the game
    // has no wood/plastic/… frame) and takes the snow floor under a snow penalty
    // (TINY_PENALTY_FLOOR=Ice|Dirt|"" overrides; the wet wood build sets Ice).
    if let Some((road, shape, _floor)) = split_penalty(name) {
        let floor = std::env::var("TINY_PENALTY_FLOOR").ok().unwrap_or_else(|| if s == "Ice" { "Ice".to_string() } else if s == "Dirt" { "Dirt".to_string() } else if s == "Grass" { String::new() } else { _floor.to_string() });
        // a deck surface with no frame of its own (wood, plastic, magnet…) and a FLAT
        // frame: the re-dressed item copy (rails in the surface) plus a GENERATED snow
        // floor quad (`Merged::add_flat_quad` in the library bake) — Everios96
        // 2026-09-21 "you forgot to turn the road part of the block into wood"
        // … and since 2026-09-23 the GRASS-floor frame too (plastic 37: the frame's road
        // strips stayed asphalt under plastic — shcr): the library generates the plain
        // `Material\Penalty` (Grass) floor
        // … and the grass set too (Everios96 2026-09-24: "for other alterations the road part
        // is still road, it should be the alteration; the penalty stays"): the deck ends take
        // the grass platform look, the penalty floor stays the zone's grass
        // … and under DIRT with TINY_DIRT_KEEP_GEOMETRY=1 every frame bakes like wood's (the
        // vanilla `RoadDirt…Penalty…` frame has the trough deck ends)
        let dirt_keep_geometry = s == "Dirt" && std::env::var("TINY_DIRT_KEEP_GEOMETRY").map(|v| v == "1").unwrap_or(false);
        if (shape.is_empty() && !matches!(s, "Ice" | "Dirt" | "Tech")) || dirt_keep_geometry {
            // fall through to the bake path below; the library adds the floor
        } else {
        let road_word = match s { "Ice" | "Dirt" | "Tech" => s.to_string(), _ => road.to_string() };
        let target = format!("Road{road_word}{shape}Penalty{floor}");
        if target == name {
            return mk(Action::Same, &target, "penalty frame: already that road + floor".into());
        }
        if idx.path_for(&target).is_some() {
            return mk(Action::Rename, &target, format!("penalty frame: rails {road_word}, floor {} (the game's terrain modifier)", if floor.is_empty() { "grass" } else { floor.as_str() }));
        }
        return mk(Action::Keep, "", format!("penalty frame: no `{target}` in the pack"));
        }
    }    // an embedded custom block record (`X.Block.Gbx_CustomBlock`) is its author's
    // own mesh: no pack twin
    if name.contains('\\') || name.ends_with("_CustomBlock") {
        // TINY_CUSTOM_BLOCK_REDRESS=1 (Everios96's A5, 2026-09-19): the author's own block
        // bakes as a re-dressed copy like a custom item — except a block that already IS
        // the surface (a community wood block in the wood sets: its name says so)
        let redress = std::env::var("TINY_CUSTOM_BLOCK_REDRESS").map(|v| v == "1").unwrap_or(false);
        // the NAME shortcut stays for wood only (the community wood roads' own planks): a
        // `…RoadDirtToRoadHalfB_Plastic` block is dirt-textured with an asphalt half
        // (plastic 26, shcr 2026-09-23) — every other surface re-dresses regardless
        let already = s == "Wood" && name.to_ascii_lowercase().contains("wood");
        // a waypoint-carrying custom block (U10S 354's finish) must stay a BLOCK — but the
        // tag lives on the MAP record, not on the block info, so that rule is applied by
        // `tmmaps surface` where the record is known (2026-10-09), not here
        if redress && !already {
            return mk(Action::Bake, "", "embedded custom block: re-dressed copy".into());
        }
        return mk(Action::Keep, "", if already { "embedded custom block: already the surface".into() } else { "embedded custom block".into() });
    }
    // pillars, structures, gates … are outside the surface families
    // Nadeo's own typo `PlatformGrasssSlope2UTop` spells its twins with the plain word
    let cand_name = name.replace("Grasss", "Grass").replace("DiagLeftWithWall", "WithWallDiagLeft").replace("DiagRightWithWall", "WithWallDiagRight");
    let Some(cand) = candidate(&cand_name, s) else {
        // TINY_WATER_OVERLAY=1: a water block OUTSIDE the surface families (`WaterBase`, the
        // pool walls and floors — Everios96 2026-09-24: "all blocks with water present")
        // keeps its volume and gets the visual-only re-dressed copy too
        // (TINY_WATER_OVERLAY_ALL=1 only: Everios96 2026-09-24 23:48 "the wood shouldn't be
        // where the water is, it should be where the drivable ground is" — the deep pools stay)
        if std::env::var("TINY_WATER_OVERLAY_ALL").map(|v| v == "1").unwrap_or(false) && s != "Water" {
            if let Some(b) = idx.path_for(name).and_then(|p| idx.load(store, &p).ok().cloned()) {
                let water_here = b.variant_base_ground.as_ref().map(|v| v.water_volumes > 0).unwrap_or(false) || b.variant_base_air.as_ref().map(|v| v.water_volumes > 0).unwrap_or(false);
                if water_here && base_prefab(&b).is_some() {
                    return mk(Action::Overlay, "", "water volume outside the surface families: kept as water + visual-only re-dressed overlay".into());
                }
            }
        }
        return mk(Action::Keep, "", "not a surface family".into());
    };
    // The ICE START (`RoadIceStart`, the bobsleigh family's raised pad) has no
    // same-shape twin anywhere and its re-dressed copy loses the pad's centre
    // (the start marks are the deck there, two-sided in the original; every
    // one-sided deck material culls them to a hole — Dirt 11's "invisible
    // start", Everios96 2026-09-15). It is a regular start block players race
    // from in every ice map: kept as the game's own block, look and physics as
    // the author placed it (6 maps: 11, 98, 124, 178, 219, 267's neighbour).
    // 2026-09-20: the hole was the plain start-marks decal's missing vertex colour
    // (see `harmonize_layouts_full`), fixed — the start is converted like every other
    // block (verified in the editor on plastic 11: deck + marks + arch). TINY_ICE_START_BAKE=0
    // restores the kept ice block.
    if name == "RoadIceStart" && s != "Ice" && std::env::var("TINY_ICE_START_BAKE").map(|v| v == "0").unwrap_or(false) {
        return mk(Action::Keep, "", "ice start: the author's own start block, kept (its copy loses the pad centre)".into());
    }
    // a deck-only surface (no block family of its own) leaves the terrain-like
    // families alone — except the DecoPlatform hills under WOOD: their dirt tops
    // stayed dirt on the castle towers of 267 ("castle blocks with dirt on top
    // were not properly updated", Everios96 2026-09-15); they take the wood deck
    // like the Platform family does
    // (2026-09-15 → 2026-09-23 the DecoPlatform pieces under WOOD were baked for "the castle
    // towers of 267" — those are ITEMS (Everios96 2026-09-22), and the bake picked up the
    // every-set snow-top row: grass platforms turned to snow across the wood set —
    // Everios96 2026-09-23 11:12 "only wet wood". The author-placed ice tops still bake above.)
    // …but never a member of the BAKE family (`TINY_BAKE_FAMILY`, the black full blocks):
    // under Wood / Wet Wood / Plastic / Magnet this Keep fired first, the block stayed a
    // kept vanilla block and its generated faces were never placed (only FREE-placed black
    // blocks converted, by position) — Everios96 2026-10-21 "the black blocks on the magnet
    // alterations are not converted" (magnet 12). The bake-family rule below handles them.
    let bake_family_member = std::env::var("TINY_BAKE_FAMILY").ok().map(|v| v.split(',').any(|f| !f.trim().is_empty() && name.starts_with(f.trim()))).unwrap_or(false) && !(name.contains("Water") && name.contains("VFC") && !name.starts_with("RoadWater"));
    if deck_only(s) && is_terrain_family(name) && !bake_family_member {
        // TINY_WATER_OVERLAY=1: a WATER piece of the terrain families (`WaterBase`, the pool
        // walls and floors) keeps its volume and gets the visual-only re-dressed copy
        if name.contains("Water") && std::env::var("TINY_WATER_OVERLAY_ALL").map(|v| v == "1").unwrap_or(false) {
            if let Some(b) = idx.path_for(name).and_then(|p| idx.load(store, &p).ok().cloned()) {
                let water_here = b.variant_base_ground.as_ref().map(|v| v.water_volumes > 0).unwrap_or(false) || b.variant_base_air.as_ref().map(|v| v.water_volumes > 0).unwrap_or(false);
                if water_here && base_prefab(&b).is_some() {
                    return mk(Action::Overlay, "", "water piece of a terrain family: kept as water + visual-only re-dressed overlay".into());
                }
            }
        }
        return mk(Action::Keep, "", format!("terrain family: no {s} form, left as is"));
    }
    // the wall-ride family IS wood (TrackWall = Wood 14), and so is the snow theme's
    // plank road (ThemeSnowRoad = Wood 14)
    if s == "Wood" && name.starts_with("TrackWall") {
        return mk(Action::Same, name, "wall ride: wood already".into());
    }
    if s == "Wood" && name.starts_with("SnowRoad") {
        return mk(Action::Same, name, "snow plank road: wood already".into());
    }
    // the rally theme's roads have DIRT decks (RoadDirt / RoadDirtFullColorize)
    if s == "Dirt" && (name.starts_with("RallyRoadDirt") || name.starts_with("RallyCastleRoad")) {
        return mk(Action::Same, name, "rally road: dirt deck already".into());
    }
    if cand == name {
        return mk(Action::Same, name, format!("already {s}"));
    }
    let Some(src_path) = idx.path_for(name) else {
        return mk(Action::Keep, "", "source model not in the pack".into());
    };
    // TINY_DIRT_KEEP_GEOMETRY=1 (Everios96 2026-09-26 14:42Z, U10S_57: "change the surface of
    // that flat road block to dirt, but keep its geometry"): under DIRT a ROAD never takes
    // its vanilla dirt twin — the RoadDirt family is a TROUGH (deck 0.5 m at the centre vs
    // the tech deck's flat 2.0; a platform→road joint dropped 1.5 m) — it bakes as a
    // re-dressed copy of its OWN shape with dirt physics, like the wood/plastic decks.
    // The platform families keep their twins (same flat mesh, another dress).
    if s == "Dirt" && (name.starts_with("Road") || name.contains("ToRoad")) && std::env::var("TINY_DIRT_KEEP_GEOMETRY").map(|v| v == "1").unwrap_or(false) {
        if let Ok(src) = idx.load(store, &src_path).cloned() {
                let has_water = src.variant_base_ground.as_ref().map(|v| v.water_volumes > 0).unwrap_or(false) || src.variant_base_air.as_ref().map(|v| v.water_volumes > 0).unwrap_or(false);
            if !has_water && base_prefab(&src).is_some() {
                return mk(Action::Bake, "", "dirt keeps geometry: the road's own shape, dirt physics (the vanilla dirt road is a trough)".into());
            }
        }
    }
    // the index answers `…Right` with its `…Left` file when the pack lacks the Right
    // one (OpenIceRoadToZoneRight): a MIRROR — good enough to verify a same-family
    // twin against, never a geometry to copy
    let mirrored_source = {
        let stem = src_path.rsplit('\\').next().unwrap_or(&src_path).to_ascii_uppercase();
        let stem = stem.split('.').next().unwrap_or("").to_string();
        stem != name.to_ascii_uppercase() && stem != cand_name.to_ascii_uppercase() && name.to_ascii_uppercase().ends_with("RIGHT") && stem.ends_with("LEFT")
    };
    let src = match idx.load(store, &src_path) {
        Ok(b) => b.clone(),
        Err(e) => return mk(Action::Keep, "", format!("source model unreadable: {e}")),
    };
    if mirrored_source {
        // the twin of the same family is the same shape by construction (the mirror
        // pair exists in every Open family); the mesh compare would see Left vs Right
        return match idx.path_for(&cand) {
            Some(_) => mk(Action::Rename, &cand, "no file for this block in the pack (only its mirror twin): renamed to the same-family twin unverified".into()),
            None => mk(Action::Keep, "", format!("no file for this block in the pack (only its mirror twin), no {s} twin: kept")),
        };
    }
    // A block with a WATER VOLUME (pool floors `PlatformWater*`, `RoadWater*`, the
    // pool edges) is never baked: an item carries no volume, so the copy would
    // drain the pool (the tiny converter's lesson, TINY.md "Water"). Renamed when
    // a same-mesh twin exists (`WaterBaseIce`, `WaterIceZoneStraight`), else kept.
    // TINY_WATER_KEEP_BLOCK=1 (the pool rule, Everios96 2026-10-06: "youre NOT supposed to
    // texture the sides at all, only the drivable part/the floor"): a block with a WATER
    // volume is never renamed to a surface twin — the twin re-dresses the whole block,
    // sides and rim included (and a `PlatformWater* -> PlatformDirt*` rename loses the
    // water entirely). The block stays exactly as it is and only its FLOOR is overlaid.
    let water_here = src.variant_base_ground.as_ref().map(|v| v.water_volumes > 0).unwrap_or(false) || src.variant_base_air.as_ref().map(|v| v.water_volumes > 0).unwrap_or(false);
    // …but a water TERRAIN ZONE (`WaterRampZone*`: the sloped shore pieces) has no mesh of
    // its own AND a real surface twin — the game draws it from the terrain, so the rename
    // IS the conversion and keeping it leaves a half-blue ramp (Everios96 2026-10-06:
    // "they also have the blue part, but not all the way"). Those take the normal path.
    let zone_like = base_prefab(&src).is_none() && idx.path_for(&cand).is_some();
    // …and the POOL family (`Water*`) has REAL vanilla dirt/grass/ice twins — `WaterBase`
    // → `WaterBaseDirt` is the game's own dirt-floored pool, water volume included. That
    // rename is the only thing that gives genuine dirt PHYSICS under water (an added hull
    // never wins against the block's water volume: measured 13 on U10S_23 with the hull at
    // the floor, dropped, and lifted to the water plane alike — 2026-10-06). The families
    // that have no such twin (`RoadWater*`, `PlatformWater*`, `DecoWallWater*`) keep the
    // block and take a visual overlay only.
    // ⚠ NOT the `Water*` family: `WaterBase` and `WaterBaseDirt` share ONE prefab
    // (`Water\Base_Air`), so that rename is a visual no-op exactly like the DecoWall twins
    // — the suffix picks the terrain, not the dress (Everios96 2026-10-06: "now the pools
    // have the blue texture again, so clearly the game doesnt have that version"). They
    // keep the block and take the floor overlay, which is what actually shows.
    // TINY_WATER_FORCE_BAKE=Fam[,Fam…]: those water families are REBUILT instead of kept —
    // with `TINY_WATER=open` the copy carries no water collision, so the block loses its
    // water volume and drives on the new surface. The only way a water ROAD can feel like
    // dirt (the game has no dirt water road, and inside a water volume the wheels always
    // report water): the trade is that the water is gone from those blocks.
    let force_bake = std::env::var("TINY_WATER_FORCE_BAKE").ok().map(|v| v.split(',').any(|f| !f.trim().is_empty() && name.starts_with(f.trim()))).unwrap_or(false);
    if water_here && force_bake && base_prefab(&src).is_some() {
        return mk(Action::Bake, "", "water family forced to a re-dressed copy (TINY_WATER_FORCE_BAKE)".into());
    }
    // ⛔ A WATER WALL IS NOT A FLOOR: `WaterWall*` (and the deco walls around water) are
    // the vertical sides of a pool, and Everios96 2026-10-07 settled it twice — "They stay
    // the same, just the driving floor is changed. where you actually drive on", then "if
    // the face is 90 degrees vertically … you leave it as is". Overlaying them painted 111
    // pool walls dirt on U10S 72. `TINY_WATER_WALLS=1` converts them anyway.
    let water_wall = (name.starts_with("WaterWall") || name.starts_with("DecoWallWater")) && !std::env::var("TINY_WATER_WALLS").map(|v| v == "1").unwrap_or(false);
    // A water WALL block is treated exactly like a pool: a FLOOR-ONLY overlay. Keeping the
    // whole block left its TOP — the surface you look down on and drive — as the game's
    // blue (Everios96 2026-10-08 on U10S 306: "these blocks are not converted at the top"),
    // while converting the whole block painted the sides dirt (map 72, 10-07). The overlay
    // path re-dresses only up-facing geometry, which is precisely the distinction.
    if water_here && water_wall {
        if base_prefab(&src).is_some() && std::env::var("TINY_WATER_OVERLAY").map(|v| v == "1").unwrap_or(false) {
            return mk(Action::Overlay, "", "water wall: the block is kept; only its top floor is overlaid".into());
        }
        return mk(Action::Keep, "", "water WALL: the vertical side stays the game's".into());
    }
    // A water TERRAIN ZONE (`WaterRampZone*`) is KEPT too when the water pipeline runs: the
    // rename to its `…Dirt` twin removes the WATER (the suffix is the terrain, the twin is a
    // dry dirt shore — Everios96 2026-10-09, picture 5: "the sloped water block is missing
    // water and looks weird"), and the 10-06 half-blue ramp was the kept block WITHOUT its
    // floor clips re-dressed. Now its `…HFC` floor clips take the dress like a pool's.
    let zone_kept = zone_like && std::env::var("TINY_WATER_OVERLAY").map(|v| v == "1").unwrap_or(false) && name.starts_with("WaterRampZone");
    if water_here && zone_kept {
        return mk(Action::Keep, "", "water ramp zone: kept with its water; its floor clips take the dress".into());
    }
    if water_here && !zone_like && std::env::var("TINY_WATER_KEEP_BLOCK").map(|v| v == "1").unwrap_or(false) {
        if s != "Water" && base_prefab(&src).is_some() && std::env::var("TINY_WATER_OVERLAY").map(|v| v == "1").unwrap_or(false) {
            return mk(Action::Overlay, "", "water volume: the block is kept untouched; only its drivable floor is overlaid".into());
        }
        return mk(Action::Keep, "", "water volume: kept as the game's block".into());
    }
    // TINY_BAKE_FAMILY=Fam[,Fam…]: a family whose "surface twin" is a twin in NAME ONLY —
    // `DecoWallCurve5` and `DecoWallCurve5Dirt` share one prefab (`Curve5_Ground`), the
    // suffix picking the TERRAIN the game grows around the wall and not the wall's own
    // dress, so a rename leaves the block looking untouched (Everios96 2026-10-05: "full
    // blocks are not converted at all"). Those bake as a re-dressed copy instead.
    // …except a WATER block's vertical face clip (`DecoWallWaterBaseVFC`, `WaterWallVFC`):
    // that is the pool's side wall, kept blue (Everios96 2026-10-09). Only its floor
    // clips (HFC/FC/FCT/FCB) are re-dressed.
    // (`RoadWaterVFC*` is NOT a side: the water roads' "vertical" clips carry the channel
    // bed — 410 m² of up-facing floor in `RoadWaterVFCInside` — 2026-10-19)
    let water_side = name.contains("Water") && name.contains("VFC") && !name.starts_with("RoadWater");
    if !water_side && std::env::var("TINY_BAKE_FAMILY").ok().map(|v| v.split(',').any(|f| !f.trim().is_empty() && name.starts_with(f.trim()))).unwrap_or(false) {
        if base_prefab(&src).is_some() {
            return mk(Action::Bake, "", "name-only surface twins (one prefab per suffix): re-dressed copy".into());
        }
        // A PREFAB-LESS member (38 of the 95 `DecoWall` kinds): the block draws nothing
        // itself — the game grows its whole body as face clips around it, and it
        // regenerates them from the authored record at load, so they stay black while the
        // block is there (Everios96 2026-10-05, after the flicker fix: "yea try to convert
        // them"). The record goes and its generated faces bake in its place
        // (TINY_FACE_BLOCK_DELETE=0 leaves the block and the faces black).
        if std::env::var("TINY_FACE_BLOCK_DELETE").map(|v| v != "0").unwrap_or(true) {
            return mk(Action::Bake, "", "no mesh of its own: the block goes, its generated faces bake in its place".into());
        }
        return mk(Action::None_, "", "no mesh of its own: the game draws it from generated faces".into());
    }
    let has_water = src.variant_base_ground.as_ref().map(|v| v.water_volumes > 0).unwrap_or(false) || src.variant_base_air.as_ref().map(|v| v.water_volumes > 0).unwrap_or(false);
    let Some(twin_path) = idx.path_for(&cand) else {
        let fam = if name.starts_with("TrackWall") { "TrackWall family has no surface variants" } else { "no such block in the pack" };
        let detail = format!("{fam}; cells {}", cells_of(&src.variant_base_ground).len().max(cells_of(&src.variant_base_air).len()));
        if has_water {
            if s != "Water" && base_prefab(&src).is_some() && std::env::var("TINY_WATER_OVERLAY").map(|v| v == "1").unwrap_or(false) {
                return mk(Action::Overlay, "", format!("water volume, no {s} twin: kept as water + visual-only re-dressed overlay ({fam})"));
            }
            // TINY_WATER_CUSTOM_BLOCK=1: baked as a re-dressed copy that BECOMES a custom
            // block derived from this block (`tmmaps custom-block`); the water volume test
            if s != "Water" && base_prefab(&src).is_some() && std::env::var("TINY_WATER_CUSTOM_BLOCK").map(|v| v == "1").unwrap_or(false) {
                return mk(Action::Bake, "", format!("water volume, no {s} twin: re-dressed copy for a custom block ({fam})"));
            }
            return mk(Action::Keep, "", format!("water volume, no {s} twin: kept as water ({fam})"));
        }
        // A GAMEPLAY PAD (`Platform<Fam>Special<Effect>`: Turbo, Turbo2, Boost,
        // SlowMotion, Cruise, NoSteering, …) is never baked: its base prefab is
        // only the deck + the effect decal — the booster's rails and lights are
        // further entities of the block the copy does not carry, and the decal
        // sits on the deck's own colour. Baked, the pad came out as a bare dim
        // pad with no hardware (Everios96, wood 273/155, 2026-09-15). Kept as the
        // game's own block: effect intact, the one-cell deck keeps its family's
        // surface.
        if name.contains("Special") && name.starts_with("Platform") && std::env::var("TINY_BAKE_PADS").map(|v| v == "0").unwrap_or(false) {
            return mk(Action::Keep, "", format!("gameplay pad: kept as the game's block ({fam})"));
        }
        // a block with geometry of its own is baked as a re-dressed copy; a
        // prefab-less one (its faces are generated fillers) has nothing to bake
        return if base_prefab(&src).is_some() { mk(Action::Bake, &cand, detail) } else { mk(Action::None_, &cand, detail) };
    };
    let twin = match idx.load(store, &twin_path) {
        Ok(b) => b.clone(),
        Err(e) => return mk(Action::Mismatch, &cand, format!("twin unreadable: {e}")),
    };
    // A twin is the SAME MESH in another dress (the modifier folder): the
    // Platform / DecoPlatform / DecoHill / Open / Water families share their
    // prefabs across surfaces, the DecoWall family has none. A different
    // prefab is a different SHAPE — RoadIce is a bobsleigh trough with its
    // floor 2.5 m above the tech deck and 6 m rims, RoadIceStart a pad 8 m up —
    // and no cell shift makes it drop in; such a block is baked as a
    // re-dressed copy of its own geometry instead (`Bake`).
    let (sp, tp) = (base_prefab(&src), base_prefab(&twin));
    // the deck levels of the two models' base prefabs (the flat collision area per height)
    let level = |store: &mut DataStore, bi: &BlockInfo| -> Option<f32> { base_prefab(bi).and_then(|pf| deck_level(store, &pf)) };
    let (ls, lt) = (level(store, &src), level(store, &twin));
    let stem = |p: &Option<String>| p.as_deref().map(|p| p.rsplit('\\').next().unwrap_or(p).to_string());
    let mut dress_note = String::new();
    if sp != tp {
        // Nadeo keeps a per-surface prefab FOLDER for some families (the platform
        // specials: `PlatformGrass\Special_Air` / `PlatformIce\Special_Air`): the
        // same file name under another folder with the deck at the same height is
        // the same shape in another dress; a road's `RoadTech\Straight_Air` vs
        // `RoadIce\Straight_Air` has its deck 2.5 m higher (the trough) and is not
        let same_shape = stem(&sp) == stem(&tp) && matches!((ls, lt), (Some(a), Some(b)) if (a - b).abs() < 0.3) || (stem(&sp) == stem(&tp) && ls.is_none() && lt.is_none());
        if !same_shape && has_water {
            // the same overlay option as the no-twin branch: a RoadWater piece under a
            // DIRT swap has a RoadDirt twin of another shape (the trough), so it stayed
            // bare water while the wood sets (no RoadWood) got the overlay (2026-10-03)
            if s != "Water" && base_prefab(&src).is_some() && std::env::var("TINY_WATER_OVERLAY").map(|v| v == "1").unwrap_or(false) {
                return mk(Action::Overlay, "", format!("water volume, twin {cand} is another shape: kept as water + visual-only re-dressed overlay"));
            }
            return mk(Action::Keep, "", format!("water volume, twin {cand} is another shape: kept as water"));
        }
        if !same_shape {
            return mk(Action::Bake, &cand, format!("different mesh: {} vs {} (deck {:?} vs {:?}) — re-dressed copy", sp.as_deref().and_then(|p| p.strip_prefix("Stadium\\Media\\Prefab\\")).unwrap_or("none"), tp.as_deref().and_then(|p| p.strip_prefix("Stadium\\Media\\Prefab\\")).unwrap_or("none"), ls, lt));
        }
        dress_note = format!("; same shape in another prefab folder ({} deck {:?})", stem(&sp).unwrap_or_default(), ls);
    }
    let dy = match (ls, lt) {
        (Some(a), Some(b)) => ((b - a) / 8.0).round() as i32,
        _ => 0,
    };
    match compare(&src, &twin, dy) {
        Ok(mut detail) => {
            let mut flag_rewrites: Vec<(u32, u32)> = Vec::new();
            for f in flags {
                // the twin's mobil of frequency 0 is one the editor never places: Nadeo's
                // plastic and grass pads keep their old `Special_Air` (hull physics 34 =
                // Player!) at mobil 0, frequency 0, and place `SpecialV2_Air` (mobil 1,
                // frequency 1). A renamed record inherits the source's subvariant 0 —
                // "the booster is not plastic" (shcr, Everios96 2026-09-20). Take the
                // subvariant the editor would give: the one mobil with frequency > 0.
                if let Some(nf) = editor_subvariant(&twin, *f) {
                    if record_fits(&src, &twin, *f).is_ok() {
                        flag_rewrites.push((*f, nf));
                        continue;
                    }
                }
                if let Err(e) = record_fits(&src, &twin, *f) {
                    // the twin lacks this ADDITIONAL variant (bits 21..27): try its base
                    // variant of the same ground/air family — the special pads' add0 is
                    // the pillar-less form of the very same 1-unit pad
                    let base = *f & !(0x7f << 21);
                    if base != *f && record_fits(&src, &twin, base).is_ok() {
                        flag_rewrites.push((*f, base));
                        continue;
                    }
                    return mk(Action::Mismatch, &cand, e);
                }
            }
            if !flag_rewrites.is_empty() {
                detail.push_str(&format!("; {} flag form(s) taken to the twin's base variant ({})", flag_rewrites.len(), flag_rewrites.iter().map(|(a, b)| format!("{a:08x}->{b:08x}")).collect::<Vec<_>>().join(",")));
            }
            if src.mat_modifier != twin.mat_modifier {
                detail.push_str(&format!("; modifier {:?} -> {:?}", src.mat_modifier.as_ref().map(|m| m.1.as_str()).unwrap_or("-"), twin.mat_modifier.as_ref().map(|m| m.1.as_str()).unwrap_or("-")));
            }
            // the record is renamed to the block info's OWN name, not the file stem:
            // Nadeo's `PlatformGrassSlope2UTop.EDClassic.Gbx` declares itself
            // `PlatformGrasssSlope2UTop` (three s), and the game resolves a record
            // by that name — the stem spelling is "Missing Resources" (Grass 15/16)
            let target = if !twin.name.is_empty() && twin.name != cand { twin.name.clone() } else { cand.clone() };
            if target != cand {
                detail.push_str(&format!("; the block info spells itself {target}"));
            }
            let mut r = mk(Action::Rename, &target, format!("{detail}{dress_note}"));
            r.dy = dy;
            r.flag_rewrites = flag_rewrites;
            r
        }
        // a water block whose twin differs: kept as water (an item has no volume)
        Err(why) if has_water => mk(Action::Keep, "", format!("water volume, twin {cand} differs ({why}): kept as water")),
        // a shape of its own that the twin does not match: baked as a re-dressed copy
        Err(why) if base_prefab(&src).is_some() => mk(Action::Bake, &cand, format!("{why} — re-dressed copy")),
        Err(why) => mk(Action::Mismatch, &cand, why),
    }
}

pub fn write_plan(path: &Path, rows: &[PlanRow]) -> std::io::Result<()> {
    let mut s = String::from("name\tcount\taction\ttarget\tdy\tdetail\n");
    for r in rows {
        s.push_str(&format!("{}\t{}\t{}\t{}\t{}\t{}\n", r.name, r.count, r.action.as_str(), r.target, r.dy, r.detail));
    }
    std::fs::write(path, s)
}

/// Read a plan TSV back (the map side).
pub fn read_plan(path: &Path) -> Result<Vec<PlanRow>, String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let mut out = Vec::new();
    for (i, line) in text.lines().enumerate() {
        if i == 0 || line.trim().is_empty() {
            continue;
        }
        let f: Vec<&str> = line.split('\t').collect();
        if f.len() < 4 {
            return Err(format!("{}:{}: short row", path.display(), i + 1));
        }
        let action = Action::parse(f[2]).ok_or_else(|| format!("{}:{}: action `{}`", path.display(), i + 1, f[2]))?;
        out.push(PlanRow { name: f[0].to_string(), count: f[1].parse().unwrap_or(0), action, target: f[3].to_string(), dy: f.get(4).and_then(|v| v.parse().ok()).unwrap_or(0), flag_rewrites: Vec::new(), detail: f.get(5).unwrap_or(&"").to_string() });
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names() {
        let c = |n: &str| candidate(n, "Ice").unwrap_or_else(|| "-".into());
        assert_eq!(c("RoadTechStraight"), "RoadIceStraight");
        assert_eq!(c("RoadDirtFinish"), "RoadIceFinish");
        assert_eq!(c("RoadBumpTiltCurve3"), "RoadIceTiltCurve3");
        assert_eq!(c("RoadWaterStart"), "RoadIceStart");
        assert_eq!(c("RoadIceStraight"), "RoadIceStraight");
        assert_eq!(c("RoadTechToRoadDirt"), "RoadIceStraight");
        assert_eq!(c("RoadTechToRoadBumpX2"), "RoadIceStraightX2");
        assert_eq!(c("RoadIceToRoadIceWithWallLeft"), "RoadIceToRoadIceWithWallLeft");
        assert_eq!(c("PlatformTechBase"), "PlatformIceBase");
        assert_eq!(c("PlatformPlasticWallStraight4"), "PlatformIceWallStraight4");
        assert_eq!(c("PlatformGrassSpecialTurbo"), "PlatformIceSpecialTurbo");
        assert_eq!(c("DecoPlatformBase"), "DecoPlatformIceBase");
        assert_eq!(c("DecoPlatformDirtSlope2UTop"), "DecoPlatformIceSlope2UTop");
        assert_eq!(c("DecoHillSlope2Straight"), "DecoHillIceSlope2Straight");
        assert_eq!(c("DecoHillDirtSlope2Curve1Out"), "DecoHillIceSlope2Curve1Out");
        assert_eq!(c("OpenTechRoadCurve1"), "OpenIceRoadCurve1");
        assert_eq!(c("OpenGrassRoadEnd"), "OpenIceRoadEnd");
        assert_eq!(c("DecoWallBase"), "DecoWallBaseIce");
        assert_eq!(c("DecoWallLoopOutStartCurve1"), "DecoWallLoopOutStartCurve1Ice");
        assert_eq!(c("DecoWallBasePillar"), "-");
        assert_eq!(c("WaterBase"), "WaterBaseIce");
        assert_eq!(c("WaterGrassZoneStraight"), "WaterIceZoneStraight");
        assert_eq!(c("WaterGrassRampRoadStraight"), "WaterIceRampRoadStraight");
        assert_eq!(c("WaterRampZoneStraight"), "WaterRampZoneStraightIce");
        assert_eq!(c("WaterWallStraight"), "WaterWallStraightIce");
        assert_eq!(c("DecoWallWaterBase"), "DecoWallWaterBaseIce");
        assert_eq!(c("TrackWallStraight"), "TrackWallStraightIce");
        assert_eq!(c("StructureBase"), "-");
        assert_eq!(c("GateFinish"), "-");
        assert_eq!(c("StageSupportCrossPillar"), "-");
        assert_eq!(candidate("PlatformDirtBase", "Tech").unwrap(), "PlatformTechBase");
        assert_eq!(candidate("DecoPlatformIceBase", "Tech").unwrap(), "DecoPlatformBase");
    }
}

// ------------------------------------------------------------------ records

/// One authored block record's fate.
#[derive(Clone, Debug)]
pub struct RecordRow {
    /// for a GENERATED FACE record: the index of the block record that owns it, resolved
    /// the way the game does (`fillers::owner_cell_face` + the owner's face list) — a
    /// face's own name is generic, so this is the only exact answer (2026-10-06)
    pub owner: Option<usize>,
    /// every grid cell this record's block covers (its footprint, in map coordinates):
    /// the writer needs it to tell which block a GENERATED FACE belongs to, since a face's
    /// name is generic (`DecoWallSimpleVFC`) and a 14-cell wall has one origin cell
    /// (2026-10-05)
    pub cells: Vec<[i32; 3]>,
    pub index: usize,
    pub name: String,
    pub action: Action,
    pub target: String,
    pub dir: u8,
    pub flags: u32,
    pub free: bool,
    /// the record's cell (grid) — for a free block its position rounded to a cell
    pub cell: [i32; 3],
    /// where the twin goes: the same cell, or one cell down for a family whose
    /// deck sits a unit higher in its own frame (RoadIce: bank + deck at +8)
    pub new_cell: [i32; 3],
    /// a free block's new position (the same y shift, in metres)
    pub new_pos: Option<[f32; 3]>,
    /// `delete`: a pillar record swallowed by the twin's extra cells
    pub delete: bool,
    pub detail: String,
    /// the flags word the renamed record takes (an additional variant the twin
    /// lacks, folded onto its base variant); `None` = unchanged
    pub new_flags: Option<u32>,
}

/// The height of a prefab's DECK: the y level (0.25 m bins) carrying the most
/// upward-facing collision area. RoadTech's straight: 0; RoadIce's: 8 (its
/// deck rides on an 8 m bank). `None` for a prefab with no collision.
pub fn deck_level(store: &mut DataStore, prefab: &str) -> Option<f32> {
    let model = store.load_model(prefab).ok()?;
    let mut c = crate::geom::Collector::new(store);
    c.model(&model, &crate::geom::IDENTITY, 0);
    let mut bins: BTreeMap<i32, f64> = BTreeMap::new();
    for (name, g) in &c.scene.groups {
        if name == "Visual" || name.starts_with("NotCollidable") || name.contains("(moving)") {
            continue;
        }
        for t in &g.tris {
            let (a, b, cc) = (g.verts[t[0] as usize], g.verts[t[1] as usize], g.verts[t[2] as usize]);
            let u = [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
            let v = [cc[0] - a[0], cc[1] - a[1], cc[2] - a[2]];
            let n = [u[1] * v[2] - u[2] * v[1], u[2] * v[0] - u[0] * v[2], u[0] * v[1] - u[1] * v[0]];
            let len = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt();
            if len < 1e-6 || n[1] / len < 0.95 {
                continue;
            }
            let y = (a[1] + b[1] + cc[1]) / 3.0;
            *bins.entry((y * 4.0).round() as i32).or_default() += (len / 2.0) as f64;
        }
    }
    if crate::debug::on("deck") {
        let mut v: Vec<(&i32, &f64)> = bins.iter().collect();
        v.sort_by(|a, b| b.1.partial_cmp(a.1).unwrap());
        eprintln!("deck {prefab}: {}", v.iter().take(8).map(|(k, a)| format!("y {} = {:.0} m2", **k as f32 / 4.0, a)).collect::<Vec<_>>().join(", "));
    }
    bins.iter().max_by(|a, b| a.1.partial_cmp(b.1).unwrap()).map(|(k, _)| *k as f32 / 4.0)
}

fn base_prefab(bi: &BlockInfo) -> Option<String> {
    bi.variant_base_ground.as_ref().or(bi.variant_base_air.as_ref()).and_then(|v| v.prefabs().into_iter().next())
}

/// Plan every authored record of one map. Returns the model rows (for the
/// report) and the record rows (for `tmmaps surface`).
/// `baked index -> owning block record index` for every GENERATED FACE of the map,
/// resolved the way the game lists it. Written beside records.tsv as `faces.tsv`.
pub fn face_owners(store: &mut DataStore, idx: &mut BlockInfoIndex, m: &tmmaps::map::MapFile) -> Vec<(usize, String, Option<usize>, String)> {
    let f = crate::fillers::faces(store, idx, m);
    let mut out = Vec::new();
    for b in &m.baked {
        let me = f.stem_of(&b.name);
        let ty = f.clips.get(&me).and_then(|c| c.ty);
        let owner = crate::fillers::owner_cell_face(b.file_cell, ty, b.dir).and_then(|(c, of)| {
            f.occupants.get(&c).and_then(|v| v.iter().find(|o| o.faces[of].iter().any(|s| *s == me)).map(|o| (o.index, o.name.clone())))
        });
        out.push((b.index, b.name.clone(), owner.as_ref().map(|(i, _)| *i), owner.map(|(_, n)| n).unwrap_or_default()));
    }
    out
}

pub fn write_face_owners(path: &Path, rows: &[(usize, String, Option<usize>, String)]) -> std::io::Result<()> {
    let mut s = String::from("baked	name	owner	owner_name
");
    for (i, name, owner, oname) in rows {
        s.push_str(&format!("{i}	{name}	{}	{oname}
", owner.map(|o| o.to_string()).unwrap_or_else(|| "-".into())));
    }
    std::fs::write(path, s)
}

pub fn plan_records(store: &mut DataStore, idx: &mut BlockInfoIndex, map: &Path, surface: &str, collection: &str) -> Result<(Vec<PlanRow>, Vec<RecordRow>), String> {
    let (s, wet) = parse_surface(surface).ok_or_else(|| format!("unknown surface `{surface}`"))?;
    let m = tmmaps::map::MapFile::try_load(map).map_err(|e| format!("{}: {e}", map.display()))?;
    // model rows
    let mut seen: BTreeMap<String, (usize, Vec<u32>)> = BTreeMap::new();
    for b in &m.blocks {
        let e = seen.entry(b.name.clone()).or_default();
        e.0 += 1;
        if !e.1.contains(&b.flags) {
            e.1.push(b.flags);
        }
    }
    let mut models: BTreeMap<String, PlanRow> = BTreeMap::new();
    for (name, (count, flags)) in &seen {
        models.insert(name.clone(), plan_one(store, idx, name, *count, flags, s, wet));
    }
    let dy_of: BTreeMap<String, i32> = models.iter().map(|(n, r)| (n.clone(), r.dy)).collect();
    // occupancy of the map as it is
    let (placements, _idx2) = crate::blockmap::walk(store, &m, false, collection);
    let mut occ: HashMap<[i32; 3], Vec<usize>> = HashMap::new(); // world cell -> block indices
    for p in &placements {
        if p.free || p.baked {
            continue;
        }
        for (_, rel) in &p.cells {
            occ.entry([p.cell[0] + rel[0], p.cell[1] + rel[1], p.cell[2] + rel[2]]).or_default().push(p.index);
        }
    }
    let by_index: HashMap<usize, &crate::blockmap::Placement> = placements.iter().map(|p| (p.index, p)).collect();
    let is_pillar = |b: &tmmaps::map::BlockRec| b.flags & 0x4000 != 0 || b.name.ends_with("Pillar");
    let mut rows: Vec<RecordRow> = Vec::new();
    let mut deletions: BTreeMap<usize, String> = BTreeMap::new();
    for b in &m.blocks {
        let row = &models[&b.name];
        let c = b.coords();
        let cell = [c.0, c.1, c.2];
        let free = b.flags & 0x2000_0000 != 0;
        let mut r = RecordRow {
            owner: None,
            cells: by_index.get(&b.index).map(|p| p.cells.iter().map(|(_, rel)| [cell[0] + rel[0], cell[1] + rel[1], cell[2] + rel[2]]).collect()).unwrap_or_else(|| vec![cell]),
            index: b.index,
            name: b.name.clone(),
            action: row.action.clone(),
            target: row.target.clone(),
            dir: b.dir,
            flags: b.flags,
            free,
            cell,
            new_cell: cell,
            new_pos: None,
            delete: false,
            detail: row.detail.clone(),
            new_flags: row.flag_rewrites.iter().find(|(from, _)| *from == b.flags).map(|(_, to)| *to),
        };
        // A generated PILLAR (flag 0x4000) under a parent block that gets RENAMED to another
        // surface: the game dresses the pillar's faces at load from the parent it finds
        // above — a plastic platform turned PlatformDirtBase paints its 3x3 pillar stack
        // in the Dirt pillar look (orange stone; Dirt 26, Everios96 2026-10-21 "the black
        // blocks still have orange stone instead of wood pillar texture on the side").
        // Baked instead, the pillar record goes and its generated faces bake as items
        // from the ORIGINAL map's view (the library reads the original: a plastic parent
        // dresses nothing → plain TrackWall = pillar wood; a grass hill's green stays).
        // `TINY_PILLAR_BAKE=0` keeps the old behaviour (pillars always the game's own).
        if !free && (b.flags & 0x4000 != 0) && b.name.starts_with("DecoWall") && b.name.ends_with("Pillar") && std::env::var("TINY_PILLAR_BAKE").map(|v| v != "0").unwrap_or(true) {
            let mut renamed_parent: Option<String> = None;
            'up: for dy in 1..=3 {
                let above = [cell[0], cell[1] + dy, cell[2]];
                for o in occ.get(&above).into_iter().flatten() {
                    let ob = &m.blocks[*o];
                    if is_pillar(ob) {
                        continue;
                    }
                    if models.get(&ob.name).map(|orow| orow.action == Action::Rename && orow.target != ob.name).unwrap_or(false) {
                        renamed_parent = Some(ob.name.clone());
                    }
                    break 'up;
                }
            }
            if let Some(parent) = renamed_parent {
                r.action = Action::Bake;
                r.target = String::new();
                r.detail = format!("pillar under a re-surfaced parent ({parent}): the block goes, its generated faces bake with the original look");
                rows.push(r);
                continue;
            }
        }
        if row.action == Action::Rename {
            let dy = dy_of.get(&b.name).copied().unwrap_or(0);
            if free {
                if let Some(p) = b.free_pos {
                    r.new_pos = Some([p[0], p[1] - 8.0 * dy as f32, p[2]]);
                }
                r.detail = format!("free; dy {dy}; {}", row.detail);
            } else {
                r.new_cell = [cell[0], cell[1] - dy, cell[2]];
                if r.new_cell[1] < 0 {
                    r.action = Action::Mismatch;
                    r.detail = format!("shifted below the grid (dy {dy})");
                } else if dy != 0 {
                    // the twin's footprint at its new cell, against everything else
                    let twin_path = idx.path_for(&row.target);
                    let footprint: Vec<[i32; 3]> = twin_path
                        .and_then(|tp| idx.load(store, &tp).ok().cloned())
                        .and_then(|tb| {
                            let ground = b.flags & 0x1000 != 0;
                            tb.pick_placement_add(ground, (b.flags & 63) as usize, ((b.flags >> 6) & 63) as usize, ((b.flags >> 21) & 0x7f) as usize).map(|pk| crate::blockmap::footprint(pk.variant, b.dir).into_iter().map(|(_, rel)| [r.new_cell[0] + rel[0], r.new_cell[1] + rel[1], r.new_cell[2] + rel[2]]).collect())
                        })
                        .unwrap_or_default();
                    let own: Vec<[i32; 3]> = by_index.get(&b.index).map(|p| p.cells.iter().map(|(_, rel)| [cell[0] + rel[0], cell[1] + rel[1], cell[2] + rel[2]]).collect()).unwrap_or_default();
                    let mut conflicts: Vec<String> = Vec::new();
                    let mut swallowed: Vec<usize> = Vec::new();
                    for w in &footprint {
                        if own.contains(w) {
                            continue;
                        }
                        for o in occ.get(w).into_iter().flatten() {
                            if *o == b.index {
                                continue;
                            }
                            let ob = &m.blocks[*o];
                            if is_pillar(ob) {
                                swallowed.push(*o);
                            } else {
                                conflicts.push(format!("#{} {} at {:?}", ob.index, ob.name, w));
                            }
                        }
                    }
                    if !conflicts.is_empty() {
                        r.action = Action::Mismatch;
                        r.detail = format!("dy {dy}: the twin's cells are taken by {}", conflicts.join(", "));
                    } else {
                        r.detail = format!("dy {dy}; {}", row.detail);
                        for o in swallowed {
                            deletions.insert(o, format!("inside the {} bank of #{}", row.target, b.index));
                        }
                    }
                }
            }
        }
        rows.push(r);
    }
    // resolve each generated FACE record to the block that owns it, exactly as the game
    // lists it (the owner's own face list names the clip): the writer then bakes a face
    // only when its owner was converted, and keeps it when the owner stayed — a face's own
    // name is generic (`DecoWallSimpleVFC` serves dozens of blocks), so nothing else is
    // exact (Everios96 2026-10-06: "still some flickering, still some blocks with missing
    // faces. CHECK THEM ALL").
    {
        let f = crate::fillers::faces(store, idx, &m);
        for r in rows.iter_mut() {
            if !r.name.contains("FC") {
                continue;
            }
            let me = f.stem_of(&r.name);
            let ty = f.clips.get(&me).and_then(|c| c.ty);
            let cell8 = [r.cell[0] as u8, r.cell[1] as u8, r.cell[2] as u8];
            r.owner = crate::fillers::owner_cell_face(cell8, ty, r.dir).and_then(|(c, of)| {
                f.occupants.get(&c).and_then(|v| v.iter().find(|o| o.faces[of].iter().any(|s| *s == me)).map(|o| o.index))
            });
        }
    }
    for r in rows.iter_mut() {
        if let Some(why) = deletions.get(&r.index) {
            r.delete = true;
            r.action = Action::Keep;
            r.detail = format!("delete: {why}");
        }
    }
    // TINY_DIRT_FLAT_JOINS=1 (Dirt only): Nadeo's dirt roads are a TROUGH (deck 0.5 m at the
    // centre; tech and every platform are flat at 2.0 m), so a platform→dirt-road joint is a
    // 1.5 m drop and road→platform a step — U10S_57's start-platform→slope jump lands past its
    // platform (shcr, 2026-09-26). Every `Road*` record that shares a FACE (same row) with a
    // flat deck (Platform*/DecoPlatform*, a baked deck, a start pad) is baked FLAT with dirt
    // physics instead of renamed, and the flag walks along face-adjacent road records so a
    // run keeps one profile up to its next platform. The `Road<A>ToRoad<B>` joins and the
    // dirt-native records (RoadDirt*) are left alone — they are the author's own shape.
    if s == "Dirt" && std::env::var("TINY_DIRT_FLAT_JOINS").map(|v| v == "1").unwrap_or(false) {
        let is_flat_deck = |n: &str| (n.starts_with("Platform") || n.starts_with("DecoPlatform")) && !n.contains("Water") && !n.contains("Wall") && !n.contains("Slope") && !n.contains("Ramp");
        let is_road = |n: &str| n.starts_with("Road") && !n.starts_with("RoadDirt") && !n.starts_with("RoadWater") && !n.starts_with("RoadIce") && !n.contains("ToRoad");
        // cell → record indices, records → their cells (unit cells from the walk)
        let cells_of: HashMap<usize, Vec<[i32; 3]>> = placements.iter().filter(|p| !p.free && !p.baked).map(|p| (p.index, p.cells.iter().map(|(_, rel)| [p.cell[0] + rel[0], p.cell[1] + rel[1], p.cell[2] + rel[2]]).collect())).collect();
        let name_of: HashMap<usize, &str> = m.blocks.iter().map(|b| (b.index, b.name.as_str())).collect();
        let neighbours = |c: &[i32; 3]| [[c[0] + 1, c[1], c[2]], [c[0] - 1, c[1], c[2]], [c[0], c[1], c[2] + 1], [c[0], c[1], c[2] - 1]];
        let mut flat: std::collections::BTreeSet<usize> = std::collections::BTreeSet::new();
        let mut queue: Vec<usize> = Vec::new();
        for (i, cells) in &cells_of {
            let Some(n) = name_of.get(i) else { continue };
            if !is_road(n) {
                continue;
            }
            let touches_deck = cells.iter().any(|c| neighbours(c).iter().any(|nb| occ.get(nb).into_iter().flatten().any(|o| o != i && name_of.get(o).map(|on| is_flat_deck(on)).unwrap_or(false))));
            if touches_deck && flat.insert(*i) {
                queue.push(*i);
            }
        }
        while let Some(i) = queue.pop() {
            let Some(cells) = cells_of.get(&i) else { continue };
            for c in cells {
                for nb in neighbours(c) {
                    for o in occ.get(&nb).into_iter().flatten() {
                        if *o != i && name_of.get(o).map(|on| is_road(on)).unwrap_or(false) && flat.insert(*o) {
                            queue.push(*o);
                        }
                    }
                }
            }
        }
        let mut n_flat = 0usize;
        for r in rows.iter_mut() {
            if flat.contains(&r.index) && !r.delete && r.action == Action::Rename {
                r.action = Action::Bake;
                r.new_cell = r.cell;
                r.new_pos = None;
                r.detail = format!("flat join: a road run touching a platform keeps the tech profile, dirt physics (was rename → {})", r.target);
                r.target.clear();
                n_flat += 1;
            }
        }
        if n_flat > 0 {
            eprintln!("  dirt flat joins: {n_flat} road record(s) baked flat instead of renamed");
        }
    }
    Ok((models.into_values().collect(), rows))
}

pub fn write_records(path: &Path, rows: &[RecordRow]) -> std::io::Result<()> {
    let mut s = String::from("index\tname\taction\ttarget\tdir\tflags\tfree\tcell_x\tcell_y\tcell_z\tnew_x\tnew_y\tnew_z\tnew_pos\tdelete\tnew_flags\tdetail\tcells\towner\n");
    for r in rows {
        s.push_str(&format!(
            "{}\t{}\t{}\t{}\t{}\t{:08x}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\n",
            r.index,
            r.name,
            r.action.as_str(),
            r.target,
            r.dir,
            r.flags,
            if r.free { 1 } else { 0 },
            r.cell[0],
            r.cell[1],
            r.cell[2],
            r.new_cell[0],
            r.new_cell[1],
            r.new_cell[2],
            r.new_pos.map(|p| format!("{},{},{}", p[0], p[1], p[2])).unwrap_or_else(|| "-".into()),
            if r.delete { 1 } else { 0 },
            r.new_flags.map(|f| format!("{f:08x}")).unwrap_or_else(|| "-".into()),
            r.detail,
            r.cells.iter().map(|c| format!("{},{},{}", c[0], c[1], c[2])).collect::<Vec<_>>().join(";"),
            r.owner.map(|i| i.to_string()).unwrap_or_else(|| "-".into())
        ));
    }
    std::fs::write(path, s)
}

/// Debug: the deck level of a block model's base prefab.
pub fn debug_deck(store: &mut DataStore, idx: &mut BlockInfoIndex, name: &str) -> String {
    let Some(p) = idx.path_for(name) else { return format!("{name}: no block info") };
    let bi = match idx.load(store, &p) {
        Ok(b) => b.clone(),
        Err(e) => return format!("{name}: {e}"),
    };
    let Some(pf) = base_prefab(&bi) else { return format!("{name}: no base prefab") };
    format!("{name}: prefab {pf} deck {:?}", deck_level(store, &pf))
}
