//! `tinyctl surface NN… --src-dir DIR --out-root DIR [--surface ice] [--campaign ID
//! --campaign-name NAME | --no-publish] [--no-shoot] …` — the SURFACE SWAP of a
//! club map end to end (2026-09-13, vjeux: "input 'ice' and it'll swap all the
//! road, dirt... blocks into their icy version; if one doesn't exist it'll
//! create a custom item version"):
//!
//! 1. `mapgeom surface-plan SRC --surface S --records R.tsv` — every authored
//!    block's fate: `rename` to its same-mesh twin of surface S (the platform,
//!    deco platform, deco hill, open road, deco wall and water families share
//!    their prefabs across surfaces and differ by a modifier folder), `bake`
//!    for a block with a shape of its own that S has no twin for (every road:
//!    RoadIce is a bobsleigh trough, not a re-dressed tech road; TrackWall;
//!    the tilt/wave/ramp pieces), `same`, `keep` (pillars, structures, gates);
//! 2. `mapgeom tiny-library SRC --scale 1 --only <bake names>` with the surface's
//!    dress (`TINY_FORCE_MODIFIER` = the Platform<S> terrain modifier folder,
//!    `TINY_REDRESS` = the road deck materials → the S road material, the water
//!    sheets off) — a full-size static-item copy of each such block, its deck
//!    LOOKING and DRIVING as S (collision physics from the material);
//! 3. `tmmaps surface SRC --plan R.tsv --library --mapping --out F --name "S <name>"`:
//!    renames, the baked blocks replaced by their items at the very same
//!    origin and quarter turn (waypoint tags carried), a new uid (`Ice2` + the
//!    source's tail, same length), the validation ghost off, medal times kept;
//! 4. `tinyctl views` (start, finish, top) + `tinyctl shoot` through the identity
//!    anchor at scale 1: the same editor cameras on the source and the swap;
//! 5. `tinyctl publish-map` (item-check gate over the baked items, Nadeo upload,
//!    campaign playlist position NN-1, stored-bytes md5 readback).
//!
//! One tracker row per map (TSV): counts of renamed / baked / kept records,
//! frames, the Nadeo verdict. `--reuse-build` skips 1–3 when the output exists.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant};

/// The surface's dress for the baked copies: (env, value) pairs.
///
/// | surface | deck material of every baked road / platform / open road / wall ride | modifier folder |
/// |---|---|---|
/// | ice | `Material\RoadIce` (RoadIce 74) on the road decks | `PlatformIce` (platforms, walls, hills) |
/// | dirt | `Material\RoadDirt` (Dirt 6) on roads and wall rides | `PlatformDirt` |
/// | grass | `Modifier\PlatformGrass\PlatformTech` (Green 76) on roads and wall rides | `PlatformGrass` |
/// | plastic | `Modifier\PlatformPlastic\PlatformTech` (Plastic 77) on roads and wall rides | `PlatformPlastic` |
/// | wood | `Material\ThemeSnowRoad` (Wood 14 — the snow theme's plank road, the material the Altered Nadeo wood sets use) on roads AND platform decks; wall rides are wood already | — (the block's own) |
/// | magnet | `Editors\MeshEditorMedia\Materials\TechMagnetic` (the mesh-editor magnet look, physics 55 forced) on the deck stems; EVERY drivable hull triangle forced to 55 (`TINY_ALL_DRIVABLE_PHYSICS`) | — |
///
/// A `wet` form (`woodwet`, `wet-grass`…) changes nothing here: the wetness is
/// the START block's Water twin (`mapgeom surface-plan`).
pub fn surface_env(surface: &str) -> Result<Vec<(String, String)>, String> {
    let (s, wet) = mapgeom::surface::parse_surface(surface).ok_or_else(|| format!("surface `{surface}`: ice, dirt, grass, plastic, wood, magnet or tech (optionally with `wet`)"))?;
    // the Water sheets STAY (the ice pool `WaterBaseIce` keeps its water, so a
    // RoadWater piece baked in it keeps its sheet too: without it the deck reads
    // as a black rectangle through the pool water — U10S_23, 2026-09-13)
    let mut env: Vec<(String, String)> = Vec::new();
    if matches!(s, "Ice" | "Dirt" | "Grass" | "Plastic") {
        // the platform family's modifier folder (Modifier\Platform<S>\{PlatformTech, TrackWall, DecoHill, …})
        env.push(("TINY_FORCE_MODIFIER".to_string(), format!("Stadium\\Media\\Modifier\\Platform{s}.TerrainModifier.Gbx")));
    }
    // OpenTechBorders: the open roads' wide deck margins (drivable asphalt, 2/3
    // of that deck) — a stem like the road decks since 2026-09-14 (plastic 05)
    // ItemInflatableTube: the pipes' one material (Everios96 2026-09-14: pipes too)
    // RoadDirtFullColorize: the castle paths' tinted dirt deck (Everios96 2026-09-14
    // 11:00: "castle parts with dirt on top"); ThemeSnowRoad/Border: the vanilla
    // wood (snow-theme plank) roads ("vanilla wood blocks are also not changed")
    // ThemeRallyCastleWallUV / ThemeRallyCastleWall: the castle road pieces' deck
    // (custom castle roads of U10S_953 stayed in their look, Everios96 2026-09-15)
    // + `Underwater`: the water roads' pool floor and walls (physics Plastic 77) — only ever
    // baked as a visual-only WATER OVERLAY (TINY_WATER_OVERLAY=1, 2026-09-24), the block keeps its water
    let road_stems = ["RoadTech", "ItemRamp", "RoadDirt", "RoadDirtFullColorize", "RoadBump", "RoadIce", "OpenTechBorders", "Underwater", "Waterground", "ItemInflatableTube", "ItemInflatableMat", "ItemInflatableFloor", "ThemeSnowRoad", "ThemeSnowRoadBorder", "ThemeRallyCastleWallUV", "ThemeRallyCastleWall"];
    let mut rows: Vec<String> = match s {
        // the snow PLANK roads take the ice platform look, not the bobsleigh one
        // (Everios96 2026-09-19 08:24 on icy 152: "use the ice platform texture")
        "Ice" => road_stems.iter().filter(|st| **st != "RoadIce").map(|st| if st.starts_with("ThemeSnowRoad") { format!("{st}=Stadium\\Media\\Modifier\\PlatformIce\\PlatformTech") } else { format!("{st}=Stadium\\Media\\Material\\RoadIce") }).collect(),
        // the black walls: the wood sides (`TrackWall`, Wood 14) STAY; the metal top rim
        // (`TrackWallClips` on the generated top caps) takes the dirt PLATFORM look — not the
        // road one (Everios96 2026-10-04: "change the metal surface at the top … use the dirt
        // platform block texture"); the scoped row rides TINY_REDRESS_SCOPED below
        // the POOL deck (`Underwater`, `Waterground`: the floor you drive on inside a water
        // block) takes the dirt PLATFORM look, not the dirt ROAD one, and the pool's SIDES
        // are not touched at all (Everios96 2026-10-06: "youre NOT supposed to texture the
        // sides at all, only the drivable part/the floor and also with platform dirt
        // texture. i said this many times now")
        // ⛔ `OpenTechBorders` carries BOTH the pool's rim AND, on several pools, the pool
        // FLOOR itself (`WaterFCCenter`, `WaterHFCLeft/Right` on U10S 72/306/10). Dropping
        // it to kill the brown rim strip left those floors blue — far worse than the strip
        // (2026-10-08). It stays in the swap; the rim needs a geometry split, not a
        // material rule. `TINY_NO_POOL_BORDER_DIRT=1` drops it again.
        "Dirt" => road_stems
            .iter()
            .filter(|st| !st.starts_with("RoadDirt") && !(std::env::var("TINY_NO_POOL_BORDER_DIRT").map(|v| v == "1").unwrap_or(false) && **st == "OpenTechBorders"))
            .map(|st| if matches!(*st, "Underwater" | "Waterground" | "RoadWater" | "PlatformWater") { format!("{st}=Stadium\\Media\\Modifier\\PlatformDirt\\PlatformTech") } else { format!("{st}=Stadium\\Media\\Material\\RoadDirt") })
            .collect(),
        "Grass" => road_stems.iter().chain(["TrackWall"].iter()).map(|st| format!("{st}=Stadium\\Media\\Modifier\\PlatformGrass\\PlatformTech")).collect(),
        "Plastic" => road_stems.iter().chain(["TrackWall"].iter()).map(|st| format!("{st}=Stadium\\Media\\Modifier\\PlatformPlastic\\PlatformTech")).collect(),
        // + `DecoHill`: the DecoPlatform hills' top (`Modifier\PlatformDirt\DecoHill` on
        // the castle towers of 267 stayed dirt under wood — Everios96 2026-09-15)
        // + `Deco`: the plain DecoPlatform tops (`Material\Deco`, physics Grass — the
        // grass-topped platforms of U10S_01 stayed grass under wood, and grass dries the
        // wet tyres: Everios96 2026-09-20 03:52 "map 1 no snow at all")
        "Wood" => road_stems.iter().filter(|st| !st.starts_with("ThemeSnowRoad")).chain(["PlatformTech", "DecoHill"].iter()).map(|st| format!("{st}=Stadium\\Media\\Material\\ThemeSnowRoad")).collect(),
        // the mesh editor's magnet material (the blackish look every community
        // magnet block wears; physics forced, the packs do not carry it) on the deck
        // stems, and EVERY drivable hull triangle magnetic (sides, borders, loops,
        // pads — Everios96, 2026-09-14: "the sides of them are not magnetic")
        // STRONG magnet since 2026-09-14 18:19Z (Everios96: "the magnet is not that
        // strong … strong magnet would be nice"; vjeux: do): TechSuperMagnetic 62
        // look: a BLACK deck (vjeux 2026-09-14 19:30Z: "strong magnet, with a black
        // texture" — the mesh-editor magnet checker was too busy), the modeler
        // concrete tinted #101010, physics TechSuperMagnetic 62 forced
        // … and the BOBSLEIGH family's sponsor decals (`…OnRoadIce`: 274 m² of the ice start's
        // 350 m² pad is one big ad) go black too — "the bobsleigh's magnet color is purple,
        // and not the black magnet we wanted" (Everios96 2026-09-24 17:57 on magnet 11). The
        // other starts' small deck ads are left as they are.
        "Magnet" => road_stems
            .iter()
            .chain(["PlatformTech", "TrackWall", "DecalSponsor1x1BigAOnRoadIce", "DecalSponsor4x1AOnRoadIce", "DecalSponsor4x1BOnRoadIce", "DecalSponsor4x1COnRoadIce", "DecalSponsor4x1DOnRoadIce"].iter())
            .map(|st| format!("{st}=Stadium\\Media\\Material_BlockCustom\\CustomConcrete@62#101010"))
            .collect(),
        "Tech" => road_stems.iter().filter(|st| **st != "RoadTech").map(|st| format!("{st}=Stadium\\Media\\Material\\RoadTech")).collect(),
        other => return Err(format!("surface `{other}`: no dress table")),
    };
    // the DecoPlatform tops of the ICE set and of WET wood: baked with the vanilla snow-top
    // look at physics Snow 21 (the ice twin block is a null surface in the game, 2026-09-20)
    // every set: the row is what the author-placed ice tops bake with too (2026-09-20 14:37)
    // … in the other sets the row is scoped to the DecoPlatform family by the library
    // (TINY_DECO_SNOW_SCOPE=DecoPlatform): the open roads' small `Deco` grass patches took
    // it too — snow triangles on the grass of wood 53 (Everios96 2026-09-23 12:39)
    rows.push("Deco=Stadium\\Media\\Modifier\\PlatformIce\\Deco@21".to_string());
    if !(s == "Ice" || (s == "Wood" && wet)) {
        env.push(("TINY_DECO_SNOW_SCOPE".to_string(), "DecoPlatform".to_string()));
    }
    // the penalty road frames' FLOOR under a snow penalty: the Ice terrain modifier (wet wood)
    if s == "Wood" && wet && std::env::var("TINY_WET_SNOW_PENALTY").map(|v| v != "0").unwrap_or(true) {
        env.push(("TINY_PENALTY_FLOOR".to_string(), "Ice".to_string()));
        // the frames' floor fillers (`PenaltyHFC*` prefabs) carry the plain `Material\Penalty`
        // grass plate: under a snow penalty it takes the ice modifier's Penalty (Snow 21)
        rows.push("Penalty=Stadium\\Media\\Modifier\\PlatformIce\\Penalty@21".to_string());
    }
    // the open roads' GRASS SHOULDERS (`DecoHill2`, physics Grass) under a snow penalty:
    // the snow modifier's own DecoHill2 (Snow 21) — Everios96 2026-09-20 17:25 "half grass
    // half road" on wet wood 01. Icy and wet wood only (the other sets keep grass shoulders)
    if s == "Ice" || (s == "Wood" && wet && std::env::var("TINY_WET_SNOW_PENALTY").map(|v| v != "0").unwrap_or(true)) {
        rows.push("DecoHill2=Stadium\\Media\\Modifier\\PlatformIce\\DecoHill2@21".to_string());
    }
    if !rows.is_empty() {
        env.push(("TINY_REDRESS".to_string(), rows.join(";")));
        // the black walls' metal top rim, per set (the top caps only; the sides stay wood)
        let wall_top: Option<String> = match s {
            "Dirt" => Some("Stadium\\Media\\Modifier\\PlatformDirt\\PlatformTech".to_string()),
            "Grass" => Some("Stadium\\Media\\Modifier\\PlatformGrass\\PlatformTech".to_string()),
            "Plastic" => Some("Stadium\\Media\\Modifier\\PlatformPlastic\\PlatformTech".to_string()),
            "Ice" => Some("Stadium\\Media\\Modifier\\PlatformIce\\PlatformTech".to_string()),
            // Wood had NO wall-top link: none of the scoped rows below ran, so a wood
            // set's full-block ride faces (loops, bowls, slopes) and metal rims stayed
            // black with wood physics (Everios96 2026-10-10, wet wood 69).
            "Wood" => Some("Stadium\\Media\\Material\\ThemeSnowRoad".to_string()),
            _ => None,
        };
        // the FULL-BLOCK black family (`DecoWall*`) is a surface family by NAME ONLY:
        // `DecoWallCurve5` and `DecoWallCurve5Dirt` share one prefab (`Curve5_Ground`) —
        // the suffix picks the TERRAIN the game grows around the wall, not the wall's own
        // dress, so a rename leaves the block exactly as black as before (Everios96
        // 2026-10-05: "full blocks are not converted at all"). They bake instead, with
        // the same top-rim rule as the road walls. TINY_DECOWALL_BAKE=0 opts out.
        // the water ROADS' drive plane takes the set's physics (2026-10-06/07: the car
        // rides just under the water surface there, so the surface id of a plane at that
        // height is what it feels — measured dirt 41.2 / tech 43.2 / plastic 43.9 / the
        // game's water 38.2 m/s over the same 200 m). Pools are excluded by family.
        // ⛔ The water pipeline runs for EVERY surface. It used to sit inside this
        // `if let`, so any set missing from `deck_physics_id` (magnet) silently converted
        // no water at all — caught by testing 2 maps × 5 alterations before shipping.
        {
            if let Some(id) = deck_physics_id(&surface) {
                env.push(("TINY_OVERLAY_WATER_DECK".to_string(), id.to_string()));
            }
            // …and the rest of the water pipeline, which until 2026-10-07 only ever ran
            // from a hand-written env on the review lanes. A plain `tinyctl surface` now
            // converts every water block the same way the review maps did:
            //   KEEP_BLOCK  the game's block stays (its dirt "twin" shares one prefab, and
            //               a PlatformWater→PlatformDirt rename loses the water outright)
            //   OVERLAY     a visual-only copy beside it: DECK_ONLY + FLOOR_ONLY keep just
            //               the re-dressed floor (a pool's floor and walls share one
            //               material, so only the up-facing triangles survive)
            //   COLLISION   that copy keeps its hull, lifted by the shell, so the floor of
            //               a POOL is drivable dirt; WATER_DECK above does the same for a
            //               water ROAD at the height the car actually rides.
            // `TINY_WATER_PIPELINE=0` opts the whole thing out.
            // the WATER FACE records the game generates for each configuration
            // (`PlatformWater*FC*`, `DecoWallWater*FC*`, `TrackWallWater*FC*`): those ARE
            // the floor the player sees and drives, so they bake and take the set's dress,
            // the same path the black blocks' faces use (Everios96 2026-10-07).
            if std::env::var("TINY_WATER_FACES").map(|v| v != "0").unwrap_or(true) {
                let fams = env.iter().position(|(k, _)| k == "TINY_BAKE_FAMILY");
                let add = "PlatformWater,DecoWallWater,TrackWallWater,RoadWater";
                match fams {
                    Some(i) => {
                        let cur = env[i].1.clone();
                        env[i].1 = format!("{cur},{add}");
                    }
                    None => env.push(("TINY_BAKE_FAMILY".to_string(), add.to_string())),
                }
            }
            if std::env::var("TINY_WATER_PIPELINE").map(|v| v != "0").unwrap_or(true) {
                for (k, v) in [
                    ("TINY_WATER_OVERLAY", "1"),
                    ("TINY_WATER_OVERLAY_ALL", "1"),
                    ("TINY_WATER_KEEP_BLOCK", "1"),
                    ("TINY_OVERLAY_FLOOR_ONLY", "1"),
                    ("TINY_OVERLAY_DECK_ONLY", "1"),
                    ("TINY_OVERLAY_COLLISION", "1"),
                    // 40 mm, not 12: AR (who wrote the other water converter) measured the
                    // overlay sitting ~10 mm up and said "should do at least 0.04 so that
                    // there isn't that much clipping with the surface under" — relayed by
                    // Everios96 2026-10-08. A thin shell z-fights the geometry beneath it,
                    // which is the strip/fin artefact on U10S 276.
                    ("TINY_OVERLAY_SHELL", "0.04"),
                ] {
                    if std::env::var(k).is_err() {
                        env.push((k.to_string(), v.to_string()));
                    }
                }
            }
        }
        if std::env::var("TINY_DECOWALL_BAKE").map(|v| v != "0").unwrap_or(true) {
            env.push(("TINY_BAKE_FAMILY".to_string(), "DecoWall".to_string()));
        }
        // Water ROADS: the prefab (StraightV2) has no bed — the channel floor the player
        // sees is the game's clips: the flat bottom plate `TrackWallWater*FCBInside*` under
        // the middle cell and the `RoadWaterVFC*` end clips, whose "vertical" name hides a
        // 410 m² up-facing `Waterground` bed reaching into the neighbour cell. Those re-dress
        // as floor clips (tmmaps surface `is_water_face`, 2026-10-19). Two wrong turns kept
        // here as history: a `[RoadWater*]TrackBorders` row (2026-10-10) coloured the BANKS
        // ("not what we want. just the blue floor"); a `[RoadWater*]Water` row (2026-10-17)
        // dressed the water SHEET itself (wood on the water — never). The physics stays
        // the overlay's: the water plane (13) re-ided and sunk to the drive line.
        if let Some(link) = wall_top {
            // … and the HALFPIPE walls (`TrackWallSlopeU*`, `TrackWallArchSlopeU*`: the U
            // blocks, normal and upside down): you drive INSIDE the curvature, and that
            // face is the block's own `TrackWall` mesh — one material for the whole block,
            // so the block takes the surface (Everios96 2026-10-05: "the halfpipe blocks
            // (upside down/normal one) dont have dirt inside the curvature"). Their
            // generated caps/panels share the name prefix and follow.
            env.push((
                "TINY_REDRESS_SCOPED".to_string(),
                // the FULL-BLOCK family is the block itself — every face of it is body you
                // see and drive on, so the whole `DecoWall*` bake takes the deck (not just
                // the top caps); the road walls keep their wood sides, and so do the road
                // walls' own faces. The halfpipes' slanted panels (`TrackWall*SlopeUp*`,
                // the inside you ride) convert with their blocks (Everios96 2026-10-05:
                // "the inside of the halfpipe needs to be changed, thats the part you
                // drive on" + "many blocks are not converted").
                // The FULL BLOCKS' bodies and side panels keep their own look: on those
                // blocks the TOP CAP is the drivable face and the sides are the pillar
                // texture ("you changed the entire block to be dirt, but the sides should
                // stay as the pillar texture", Everios96 2026-10-06) — so no `[DecoWall*]`
                // wood row; the cap rule below does the work.
                // The METAL (`TrackWallClips`) goes to the surface wherever it appears —
                // the top rim Everios96 asked for, the caps the game generates (named
                // after the prefab they came from: a DecoWall's cap is `PlatformBaseFCT`,
                // a road wall's `TrackWallStraightFCT`, so a family-scoped rule missed 17
                // of them), and the arch blocks' own rim. The WOOD (`TrackWall`) stays,
                // except where it IS the ride surface: the full blocks and the halfpipes'
                // inside (2026-10-05, Everios96's third review round) — including the
                // panel the game grows for the UPSIDE-DOWN one, `TrackWallVFCSlopeDown`
                // ("there is also an upside down version of the block … its still the same
                // face that needs to change, just upside down"). Its `*SlopeUp*` twin was
                // already covered; the Down spelling was not, so that ride face stayed
                // black while the block around it turned dirt.
                format!("[*]TrackWallClips={link};[DecoWallLoop*]TrackWall={link};[DecoWallSlope*]TrackWall={link};[DecoWallCurve*]TrackWall={link};[DecoWallDiag*]TrackWall={link};[DecoWallBase]TrackWall={link};[DecoWallBaseFCT*]TrackWall={link};[DecoWallSimple]TrackWall={link};[Platform*FCT*]TrackWall={link};[Platform*FCB*]TrackWall={link};[TrackWall*SlopeU*]TrackWall={link};[TrackWall*SlopeUp*]TrackWall={link};[TrackWall*SlopeDown*]TrackWall={link};[TrackWallArchSlope*U*]TrackWall={link}"),
            ));
        }
    }
    // Magnet: the DECK rule like every other alteration (Everios96 2026-09-15 04:30:
    // "penalty surface should stay penalty") — until then every drivable triangle
    // was forced magnetic (the 2026-09-14 "everything magnetic" v2), shoulders included

    // the pipes: baked as re-dressed item copies, physics and texture
    // + the inflatable MATS (`InflatableMat4mToTube6mFlat` is a pipe segment with a mat
    // end: GeraldDuke, wet wood 29, 2026-09-19 "pipe pieces made of metal") — the
    // moving ObstacleTube* rotors are dyna objects (no static mesh to re-dress)
    // TINY_ITEM_REDRESS_EXTRA=glob,glob: per-build additions (the inflatable mats of the
    // maps whose record ghosts drive on them, 2026-09-19)
    let extra = std::env::var("TINY_ITEM_REDRESS_EXTRA").ok().filter(|v| !v.trim().is_empty()).map(|v| format!(",{v}")).unwrap_or_default();
    // + the stock RAMP items (RampLow/Med/High, v2): `ItemRamp` is Metal 4 in every set —
        // "metal preserves wet and icy tire conditions but does not have the same grip"
        // (GeraldDuke 2026-09-27, Everios96 2026-10-03: "those should be all converted")
        env.push(("TINY_ITEM_REDRESS".to_string(), format!("InflatableTube*,*ToTube*,Ramp*{extra}")));
    // the map's own custom BLOCKS take the alteration too (A5); TINY_CUSTOM_BLOCK_REDRESS=0 keeps them
    if std::env::var("TINY_CUSTOM_BLOCK_REDRESS").map(|v| v != "0").unwrap_or(true) {
        env.push(("TINY_CUSTOM_BLOCK_REDRESS".to_string(), "1".to_string()));
        std::env::set_var("TINY_CUSTOM_BLOCK_REDRESS", "1");
    }
    // the swap's deck physics for the deck-like triangles no stem names (Deco strips…)
    let deck_physics = match s {
        "Ice" => Some("74"),
        "Dirt" => Some("6"),
        "Grass" => Some("76"),
        "Plastic" => Some("77"),
        "Wood" => Some("14"),
        "Tech" => Some("16"),
        // Magnet: the deck rule like every other alteration (Everios96 2026-09-15
        // 04:30 "penalty surface should stay penalty"; v2 had forced every drivable
        // triangle magnetic, shoulders included) — TechSuperMagnetic 62
        "Magnet" => Some("62"),
        _ => None,
    };
    if let Some(p) = deck_physics {
        env.push(("TINY_DECK_PHYSICS".to_string(), p.to_string()));
    }
    Ok(env)
}

/// How a surface argument is spelled in names: the bracket of
/// `U10S_N By Everios96 [Wet Wood]`, the campaign "Wet Wood U10S", and the
/// 4-character uid prefix (`Wet2`, `Woo2`, `Mag2`, `Ice2`).
pub fn surface_label(surface: &str) -> Result<(String, String), String> {
    let (s, wet) = mapgeom::surface::parse_surface(surface).ok_or_else(|| format!("surface `{surface}`"))?;
    let label = if wet { format!("Wet {s}") } else { s.to_string() };
    let prefix_word = if wet { "Wet" } else { s };
    let uid_prefix = format!("{}2", &prefix_word[..prefix_word.len().min(3)]);
    Ok((label, uid_prefix))
}

fn run(cmd: &mut Command, log: &Path) -> Result<String, String> {
    let out = cmd.output().map_err(|e| format!("{:?}: {e}", cmd.get_program()))?;
    let mut text = String::from_utf8_lossy(&out.stdout).into_owned();
    text.push_str(&String::from_utf8_lossy(&out.stderr));
    std::fs::write(log, &text).map_err(|e| format!("{}: {e}", log.display()))?;
    if !out.status.success() {
        let tail: Vec<&str> = text.lines().rev().take(3).collect();
        return Err(format!("{:?} failed ({}); log {}; {}", cmd.get_program().to_string_lossy().rsplit('/').next().unwrap_or(""), out.status, log.display(), tail.into_iter().rev().collect::<Vec<_>>().join(" | ")));
    }
    Ok(text)
}

fn first_line(s: &str) -> String {
    s.lines().next().unwrap_or("").to_string()
}

fn append(tracker: &Path, row: &[String]) -> Result<(), String> {
    use std::io::Write;
    let mut f = std::fs::OpenOptions::new().append(true).open(tracker).map_err(|e| format!("{}: {e}", tracker.display()))?;
    writeln!(f, "{}", row.join("\t")).map_err(|e| format!("{}: {e}", tracker.display()))
}

/// The per-record plan's tally: (renamed, baked, same, kept, none) and the
/// names behind `bake`.
fn plan_tally(records: &Path) -> Result<(BTreeMap<String, usize>, Vec<String>), String> {
    OVERLAY_NAMES.with(|o| o.borrow_mut().clear());
    let text = std::fs::read_to_string(records).map_err(|e| format!("{}: {e}", records.display()))?;
    let mut tally: BTreeMap<String, usize> = BTreeMap::new();
    let mut bake_names: Vec<String> = Vec::new();
    for (i, line) in text.lines().enumerate() {
        if i == 0 || line.trim().is_empty() {
            continue;
        }
        let f: Vec<&str> = line.split('\t').collect();
        let (name, action, delete) = (f.get(1).copied().unwrap_or(""), f.get(2).copied().unwrap_or(""), f.get(14).copied().unwrap_or("0"));
        let key = if delete == "1" { "delete" } else { action };
        *tally.entry(key.to_string()).or_default() += 1;
        if (action == "bake" || action == "overlay") && delete != "1" && !bake_names.contains(&name.to_string()) {
            bake_names.push(name.to_string());
        }
        if action == "overlay" && delete != "1" {
            OVERLAY_NAMES.with(|o| { let mut o = o.borrow_mut(); if !o.contains(&name.to_string()) { o.push(name.to_string()); } });
        }
    }
    Ok((tally, bake_names))
}

thread_local! {
    /// the `overlay` rows of the last plan read: the library bakes them visual-only
    /// (TINY_OVERLAY_NAMES), tmmaps surface places them without removing the block
    static OVERLAY_NAMES: std::cell::RefCell<Vec<String>> = const { std::cell::RefCell::new(Vec::new()) };
}

/// The physics id a surface's deck drives as: Dirt 6, Grass 76, Ice 74, Plastic 77,
/// Sand 5, Snow 21, Wood 14 (the engine's surface enum, `tm-player-env-arm`).
fn deck_physics_id(surface: &str) -> Option<u8> {
    let (sf, _wet) = mapgeom::surface::parse_surface(surface)?;
    Some(match &sf[..] {
        // Magnet drives as 62 (`Material_BlockCustom\CustomConcrete@62`, the set's own row);
        // without it the WHOLE water pipeline below was skipped for magnet — both test maps
        // converted zero water blocks (2026-10-08).
        "Magnet" => 62,
        "Dirt" => 6,
        "Grass" => 76,
        "Ice" => 74,
        "Plastic" => 77,
        "Sand" => 5,
        "Snow" => 21,
        "Wood" => 14,
        _ => return None,
    })
}

pub fn cmd(args: &[String]) -> Result<(), String> {
    let f = |k: &str| tmmaps::cli::flag(args, k).map(String::from);
    let maps: Vec<String> = args.iter().take_while(|a| !a.starts_with("--")).cloned().collect();
    if maps.is_empty() || !maps.iter().all(|m| m.len() == 2 && m.chars().all(|c| c.is_ascii_digit())) {
        return Err("surface needs two-digit map numbers first (tinyctl surface 01 02 … --src-dir DIR …)".into());
    }
    let surface = f("--surface").unwrap_or_else(|| "ice".into());
    // the bracket word (`[Wet Wood]`) and the uid prefix (`Wet2`) from the surface; --label overrides the word
    let (word, default_prefix) = surface_label(&surface)?;
    let word = f("--label").unwrap_or(word);
    let src_dir = PathBuf::from(f("--src-dir").ok_or("surface needs --src-dir DIR (NN-<name>.Map.Gbx sources)")?);
    let out_root = PathBuf::from(f("--out-root").unwrap_or_else(|| format!("/tmp/surface-{surface}")));
    let frames_dir = PathBuf::from(f("--frames-dir").unwrap_or_else(|| out_root.join("frames").to_string_lossy().into_owned()));
    let tracker = PathBuf::from(f("--tracker").unwrap_or_else(|| out_root.join("tracker.tsv").to_string_lossy().into_owned()));
    let club = f("--club").unwrap_or_else(|| crate::publish::DEFAULT_CLUB.into());
    let no_shoot = tmmaps::cli::has(args, "--no-shoot");
    let no_publish = tmmaps::cli::has(args, "--no-publish");
    let uid_prefix = f("--uid-prefix").unwrap_or(default_prefix);
    let (campaign, campaign_name) = if no_publish {
        (f("--campaign").unwrap_or_default(), f("--campaign-name").unwrap_or_default())
    } else {
        (f("--campaign").ok_or("surface needs --campaign ID (tinyctl nadeo-here campaign-create) or --no-publish")?, f("--campaign-name").ok_or("surface needs --campaign-name NAME")?)
    };
    let bin_dir = f("--bin-dir").map(PathBuf::from).or_else(|| std::env::current_exe().ok().and_then(|p| p.parent().map(|d| d.to_path_buf()))).ok_or("--bin-dir DIR")?;
    let mapgeom = bin_dir.join("mapgeom");
    let tmmaps = bin_dir.join("tmmaps");
    for p in [&mapgeom, &tmmaps] {
        if !p.exists() {
            return Err(format!("{}: no such binary", p.display()));
        }
    }
    let env = surface_env(&surface)?;
    std::fs::create_dir_all(&frames_dir).map_err(|e| format!("{}: {e}", frames_dir.display()))?;
    if let Some(p) = tracker.parent() {
        std::fs::create_dir_all(p).map_err(|e| format!("{}: {e}", p.display()))?;
    }
    if !tracker.exists() {
        std::fs::write(&tracker, "nn\tsource_file\tsource_name\tsource_uid\tauthortime_ms\tgold\tsilver\tbronze\tout_name\tout_uid\tout_bytes\trenamed\tbaked\tsame\tkept\tnone\tbake_models\tbuild_s\tframes\tflagged_views\tnadeo\tnote\n").map_err(|e| format!("{}: {e}", tracker.display()))?;
    }
    let mut failed = 0usize;
    for nn in &maps {
        let t0 = Instant::now();
        let mut row: Vec<String> = vec![nn.clone()];
        let mut note = String::new();
        let src = std::fs::read_dir(&src_dir)
            .map_err(|e| format!("{}: {e}", src_dir.display()))?
            .filter_map(|e| e.ok())
            .map(|e| e.path())
            .find(|p| p.file_name().map(|n| n.to_string_lossy().starts_with(&format!("{nn}-")) && n.to_string_lossy().ends_with(".Map.Gbx")).unwrap_or(false));
        let Some(src) = src else {
            eprintln!("{nn}: no {nn}-*.Map.Gbx in {}", src_dir.display());
            failed += 1;
            continue;
        };
        let hdr = match tmmaps::header::read(&src.display().to_string()) {
            Ok(h) => h,
            Err(e) => {
                eprintln!("{nn}: source header: {e}");
                failed += 1;
                continue;
            }
        };
        row.push(src.file_name().unwrap_or_default().to_string_lossy().into_owned());
        row.push(hdr.name.clone());
        row.push(hdr.uid.clone());
        row.push(hdr.authortime.clone());
        row.push(hdr.gold.clone());
        row.push(hdr.silver.clone());
        row.push(hdr.bronze.clone());
        println!("\n===== {nn}: {} ({}, AT {}) → {word} =====", hdr.name, hdr.uid, hdr.authortime);
        let m = tmmaps::map::MapFile::load(&src);
        let coll = crate::views::collection_of(&m);
        let paks = match crate::build::paks_for(coll) {
            Ok(p) => p,
            Err(e) => {
                eprintln!("{nn}: {e}");
                failed += 1;
                continue;
            }
        };
        let dir = out_root.join(format!("{surface}{nn}"));
        std::fs::create_dir_all(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
        // --name-style everios: the club's convention (Everios96 to vjeux, 2026-09-13:
        // "U10S_26 By Everios96 [Tiny] — always like this and then the alteration name
        // in the last bracket"; the FILE name too): `U10S_N By Everios96 [Ice]`
        let everios = f("--name-style").as_deref() == Some("everios");
        // --author-tag "By X" (default "By Everios96"): the middle of the club-style name;
        // "-" = no author tag ("<name> [Icy]") — HarrisonTT's UBU10 set, 2026-09-21
        let author_tag = f("--author-tag").unwrap_or_else(|| "By Everios96".to_string());
        let out_name = if everios {
            if author_tag == "-" { format!("{} [{word}]", hdr.name) } else { format!("{} {author_tag} [{word}]", hdr.name) }
        } else { format!("{word} {}", hdr.name) };
        let out_map = if everios { dir.join(format!("{out_name}.Map.Gbx")) } else { dir.join(format!("U10S-{nn}-{word}.Map.Gbx")) };
        let records = dir.join("records.tsv");
        let models = dir.join("models.tsv");
        let build_t = Instant::now();
        let reuse = tmmaps::cli::has(args, "--reuse-build") && out_map.exists() && records.exists();
        let mut bake_models: Vec<String> = Vec::new();
        let mut build_with = |lod_override: Option<&str>| -> Result<(), String> {
            if reuse {
                println!("{nn}: reusing {}", out_map.display());
                return Ok(());
            }
            // 1. the plan
            let mut plan = Command::new(&mapgeom);
            plan.args(&paks).arg("surface-plan").arg(&src).arg("--surface").arg(&surface).arg("--out").arg(&models).arg("--records").arg(&records);
            plan.envs(env.iter().map(|(k, v)| (k.as_str(), v.as_str())));
            let text = run(&mut plan, &dir.join("plan.log"))?;
            for l in text.lines().filter(|l| l.starts_with("records:") || l.starts_with("  conflict")) {
                println!("  {}", l.trim());
            }
            let (_tally, mut names) = plan_tally(&records)?;
            // the ITEMS the swap dresses (TINY_ITEM_REDRESS globs, the pipes): their
            // models join the library bake; the writer re-points their records
            if let Some(globs) = env.iter().find(|(k, _)| k == "TINY_ITEM_REDRESS").map(|(_, v)| v.clone()) {
                let src_map = tmmaps::map::MapFile::load(&src);
                // the map's own embedded custom items (Everios96 2026-09-14 14:31: "change
                // the custom items"): every model whose file the map carries — or, with
                // TINY_CUSTOM_ITEM_LIST=<file> (the WR-ghost classifier, one model per
                // line), only the driven ones
                let embedded_idents: Vec<String> = src_map.embedded_objects().map(|(rows, _)| rows.into_iter().map(|(i, _)| i.to_ascii_lowercase()).collect()).unwrap_or_default();
                let custom_all = std::env::var("TINY_CUSTOM_ITEM_REDRESS").map(|v| v != "0").unwrap_or(false);
                // a list FILE that cannot be read is an error, not "no list": the devserver's
                // /tmp purge ate custom-convert2.txt on 2026-09-20 04:00 and every build until
                // 18:45 silently left the custom items unconverted
                if let Ok(f) = std::env::var("TINY_CUSTOM_ITEM_LIST") {
                    if std::fs::metadata(&f).is_err() {
                        return Err(format!("TINY_CUSTOM_ITEM_LIST={f}: cannot read the list"));
                    }
                }
                let custom_list: Option<Vec<String>> = std::env::var("TINY_CUSTOM_ITEM_LIST").ok().and_then(|f| std::fs::read_to_string(f).ok()).map(|t| t.lines().chain(std::env::var("TINY_CUSTOM_ITEM_PHYSICS_LIST").ok().and_then(|f| std::fs::read_to_string(f).ok()).unwrap_or_default().lines().map(|l| l).collect::<Vec<_>>().into_iter()).map(|l| l.trim().to_ascii_lowercase()).filter(|l| !l.is_empty()).collect());
                let mut item_models: Vec<String> = src_map.items.iter().map(|it| it.model.clone()).filter(|m| {
                    let ml = m.to_ascii_lowercase();
                    let is_custom = embedded_idents.iter().any(|i| *i == ml || i.ends_with(&format!("\\{ml}")) || ml.ends_with(&format!("\\{i}")));
                    globs.split(',').map(str::trim).filter(|g| !g.is_empty()).any(|g| glob_simple(g, m))
                        || (is_custom && (custom_all || custom_list.as_ref().map(|l| l.iter().any(|x| *x == ml)).unwrap_or(false)))
                }).collect();
                item_models.sort();
                item_models.dedup();
                for m in item_models {
                    if !names.contains(&m) {
                        names.push(m);
                    }
                }
            }
            // a penalty frame that bakes takes its generated `<Road>PenaltyHFC*` floor fillers
            // along (the vanilla floor: deck ends, penalty middle, kerbs — tmmaps surface
            // places them as items from the library's `b@` rows; 2026-09-24)
            let frame_names: Vec<String> = names.iter().filter(|n| n.starts_with("Road") && (n.ends_with("Penalty") || n.ends_with("PenaltyIce") || n.ends_with("PenaltyDirt"))).cloned().collect();
            for fnm in frame_names {
                let stem = fnm.trim_end_matches("Dirt").trim_end_matches("Ice").to_string(); // Road<S>Penalty
                for suffix in ["HFC", "HFCEnds"] {
                    let n = format!("{stem}{suffix}");
                    if !names.contains(&n) {
                        names.push(n);
                    }
                }
            }
            // … and a gameplay PAD that bakes (Platform<S>Special<Kind>…) takes its generated
            // side skirts along (`*Special*FC*` records: the LED-sign panels whose dress is the
            // OWNER pad's effect — orphaned records draw with a wrong effect / direction,
            // Everios96 2026-09-24 "effect blocks' side textures often wrong")
            if names.iter().any(|n| n.contains("Special") && !n.contains("FC")) {
                let src_map = tmmaps::map::MapFile::load(&src);
                let mut skirts: Vec<String> = src_map.baked.iter().filter(|b| b.name.contains("Special") && b.name.contains("FC") && !b.name.contains("Water")).map(|b| b.name.clone()).collect();
                skirts.sort();
                skirts.dedup();
                for n in skirts {
                    if !names.contains(&n) {
                        names.push(n);
                    }
                }
            }
            // … and the BLACK WALL family (`TrackWall*`, no surface variants → every
            // non-water, non-pillar wall bakes) takes its generated FACES along: a wall
            // block's prefab is a thin sheet; the body a player sees — the vertical faces
            // (`TrackWall*VFC*`), the top and bottom caps (`*FCT`/`*FCB`) — is generated by
            // the game for the AUTHORED block only. Baked without them the wall is a
            // standing ribbon (Everios96 2026-10-03/04, the dirt review maps and the live
            // "black platforms"). The fillers bake re-dressed like the wall and their
            // records go (tmmaps surface `is_wall_filler`). TINY_WALL_FILLERS=0 opts out.
            // the FULL-BLOCK black family (`DecoWall*`) is a surface family by name only:
            // `DecoWallCurve5` and `DecoWallCurve5Dirt` share ONE prefab (Curve5_Ground) —
            // the suffix picks the TERRAIN the game grows around the wall, not the wall's
            // own dress, so the rename leaves the block looking exactly as black as before
            // (Everios96 2026-10-05: "full blocks are not converted at all"). They bake
            // instead, with the same top-rim rule as the road walls (TINY_DECOWALL_BAKE=0
            // opts out).
            // …a map with WATER blocks needs the same pass even when it holds no black
            // walls at all (U10S 263 is pure water and skipped the whole branch, so its
            // floors were never baked — 2026-10-07).
            // ⛔ tested against the MAP's blocks, not the bake list: a pool built only of
            // KEPT water walls (`DecoWallWaterBase` ×220 on U10S 91) puts no water kind in
            // `names`, the gate stayed shut and the pool floor shipped blue (Everios96
            // 2026-10-19: "Map 91 pool floor is not wood"). The floor clips of a kept
            // owner convert (tmmaps surface, keep_indices) — if they get into the library.
            let has_water_blocks = std::env::var("TINY_WATER_FACES").map(|v| v != "0").unwrap_or(true)
                && (names.iter().any(|n| n.contains("Water") && !n.contains("Pillar"))
                    || std::fs::read_to_string(&records).unwrap_or_default().lines().skip(1).any(|l| l.split('\t').nth(1).map(|n| n.contains("Water") && !n.contains("Pillar")).unwrap_or(false)));
            if (std::env::var("TINY_WALL_FILLERS").map(|v| v != "0").unwrap_or(true) && names.iter().any(|n| (n.starts_with("TrackWall") || n.starts_with("DecoWall")) && !n.contains("Water") && !n.contains("Pillar"))) || has_water_blocks {
                let src_map = tmmaps::map::MapFile::load(&src);
                // only the faces of a wall that itself bakes: a face baked beside a wall
                // that STAYED is drawn twice (the engine regenerates the clip from the
                // authored block) — the flicker Everios96 saw on 2026-10-05
                // every wall-face KIND of the map goes in the library; which RECORD is
                // actually baked is decided per record by the writer, geometrically (a
                // generic panel name like `DecoWallSimpleVFC` cannot name its owner)
                let mut faces: Vec<String> = src_map.baked.iter().filter(|b| (b.name.starts_with("TrackWall") || b.name.starts_with("DecoWall") || b.name.starts_with("Platform")) && b.name.contains("FC") && !b.name.contains("Water") && !b.name.contains("Pillar")).map(|b| b.name.clone()).collect();
                // …and the WATER faces, which are the floor of a water block: the block's
                // own prefab has none and the shape depends on what it connects to, so the
                // game's generated records are the only thing that fits (Everios96
                // 2026-10-07). `TINY_WATER_FACES=0` leaves them alone.
                if std::env::var("TINY_WATER_FACES").map(|v| v != "0").unwrap_or(true) {
                    let water: Vec<String> = src_map
                        .baked
                        .iter()
                        .filter(|b| b.name.contains("Water") && b.name.contains("FC") && !b.name.contains("Pillar"))
                        .map(|b| b.name.clone())
                        .collect();
                    let n0 = faces.len();
                    faces.extend(water);
                    faces.sort();
                    faces.dedup();
                    if faces.len() > n0 {
                        println!("  water floors: {} generated face kinds bake with their blocks", faces.len() - n0);
                    }
                }
                faces.sort();
                faces.dedup();
                if !faces.is_empty() {
                    println!("  black walls: {} generated face kinds bake with their walls", faces.len());
                }
                for n in faces {
                    if !names.contains(&n) {
                        names.push(n);
                    }
                }
            }
            // --bake-only A,B: bake just these models of the plan (an in-place item
            // repair re-bakes one block per map; the writer step still runs the full
            // plan, so the result map is only a reference — the live map takes the
            // item through `tmmaps replace-embedded`, 2026-10-03 bobsleigh finish marks)
            if let Some(only) = f("--bake-only") {
                let keep: Vec<String> = only.split(',').map(|t| t.trim().to_string()).filter(|t| !t.is_empty()).collect();
                names.retain(|n| keep.iter().any(|k| k == n));
                if names.is_empty() {
                    return Err(format!("{nn}: --bake-only {only}: none of these models is in the plan's bake list"));
                }
            }
            bake_models = names.clone();
            // the overlay kinds reach the library through the process environment (the
            // library command inherits it; `env` is shared read-only with the plan step)
            // (handed to the library COMMAND below, never the process environment: surface-round
            // builds several maps in parallel threads)
            let overlay: Vec<String> = OVERLAY_NAMES.with(|o| o.borrow().clone());
            if !overlay.is_empty() {
                println!("  water overlay: {} block kinds baked visual-only beside their kept blocks", overlay.len());
            }
            // 2. the baked copies
            let lib = dir.join("lib.zip");
            let mapping = dir.join("placements.tsv");
            let mut with_items = false;
            if !names.is_empty() {
                let mut libcmd = Command::new(&mapgeom);
                libcmd.args(&paks);
                // the far detail levels off: the copies are few and beside the
                // track, and every byte counts against the 7 MB server cap
                let lod = lod_override.map(String::from).unwrap_or_else(|| f("--lod-pick").unwrap_or_else(|| "0".into()));
                if lod != "-" {
                    libcmd.arg("--lod-pick").arg(&lod);
                }
                libcmd.arg("tiny-library").arg(&src).arg("--scale").arg("1").arg("--only").arg(names.join(",")).arg("--library-out").arg(&lib).arg("--mapping-out").arg(&mapping).arg("--report").arg(dir.join("report.tsv"));
                let map_no: usize = nn.parse().unwrap_or(0);
                libcmd.envs(env.iter().map(|(k, v)| (k.as_str(), v.as_str())));
                let overlay: Vec<String> = OVERLAY_NAMES.with(|o| o.borrow().clone());
                if !overlay.is_empty() {
                    libcmd.env("TINY_OVERLAY_NAMES", overlay.join(","));
                }
                // a custom item in the bake list was PUT there by the convert list (or the
                // all-custom switch): the bake must dress it — otherwise it comes out as a
                // plain copy (U10S_953's castle roads, Everios96 2026-09-15 03:00)
                if std::env::var("TINY_CUSTOM_ITEM_LIST").is_ok() || std::env::var("TINY_CUSTOM_ITEM_REDRESS").is_ok() {
                    libcmd.env("TINY_CUSTOM_ITEM_REDRESS", "1");
                }
                // item names unique per (part, map) across the whole club: the game caches
                // an embedded model by FILE NAME for the session (`--alias-part P`, as the
                // tiny pipeline: base (P*100+NN)*1000 + 500 leaves 500 names per map)
                // --alias-base N: an explicit start (the Altered Nadeo builds number per campaign + map)
                match f("--alias-part").and_then(|p| p.parse::<usize>().ok()) {
                    _ if f("--alias-base").is_some() => {
                        let base = f("--alias-base").unwrap();
                        libcmd.env("TINY_ALIAS_BASE", &base);
                        libcmd.env("TINY_PICTURE_SUFFIX", format!("_{surface}b{base}"));
                    }
                    Some(part) => {
                        // every SURFACE gets its own name range (the game caches embedded models by
                        // FILE NAME for the session — a wood map's pieces would show in a grass map
                        // of the same number): tiny 0.1–4 M, ice the same +500, giant 5.1–9 M,
                        // wood 10 M, wet wood 14 M, grass 18 M, plastic 22 M, dirt 26 M, magnet 30 M
                        libcmd.env("TINY_ALIAS_BASE", format!("{}", surface_alias_offset(&surface) + (part * 100 + map_no) * 1000 + 500));
                        libcmd.env("TINY_PICTURE_SUFFIX", format!("_{surface}p{part:02}{nn}"));
                    }
                    None => {
                        libcmd.env("TINY_ALIAS_BASE", format!("{}", map_no * 1_000_000 + 700_000));
                        libcmd.env("TINY_PICTURE_SUFFIX", format!("_{surface}{nn}"));
                    }
                }
                let text = run(&mut libcmd, &dir.join("library.log"))?;
                for l in text.lines().filter(|l| l.contains("library:") || l.contains("FAILED")) {
                    println!("  {}", l.trim());
                }
                // an EMPTY library (every bake row a mesh-less custom block — AR's
                // `Water Blocks (no mesh)\…RoadWaterStraight` in the Winter/Spring/Summer 2026
                // wet layers, 2026-09-25): nothing to unpack, the rows stay blocks
                let lib_entries = std::fs::read(&lib).map(|b| tmmaps::header::zip_entries(&b).len()).unwrap_or(0);
                with_items = lib.exists() && lib_entries > 0;
                if with_items {
                    // the items unpacked beside the map (publish-map's item-check gate)
                    let libx = dir.join("libx");
                    let _ = std::fs::remove_dir_all(&libx);
                    std::fs::create_dir_all(&libx).map_err(|e| format!("{}: {e}", libx.display()))?;
                    let unzip = Command::new("unzip").arg("-q").arg(&lib).arg("-d").arg(&libx).output().map_err(|e| format!("unzip: {e}"))?;
                    if !unzip.status.success() {
                        return Err(format!("unzip {}: {}", lib.display(), String::from_utf8_lossy(&unzip.stderr)));
                    }
                } else {
                    println!("  library: empty (no baked model produced a mesh) — the bake rows stay blocks");
                }
            }
            // 3. the map
            let mut sw = Command::new(&tmmaps);
            // the writer reads the same knobs as the plan (TINY_BAKE_FAMILY tells it which
            // face-drawn blocks to remove in favour of their baked faces, 2026-10-05)
            sw.envs(env.iter().map(|(k, v)| (k.as_str(), v.as_str())));
            sw.arg("surface").arg(&src).arg("--plan").arg(&records).arg("--out").arg(&out_map).arg("--name").arg(&out_name).arg("--uid-prefix").arg(&uid_prefix);
            // the winter trees are the ICE dress only; every other surface keeps the vegetation
            // … and the WET wood set since 2026-09-19 (GeraldDuke: snow trees like icy)
            let wet_snow = mapgeom::surface::parse_surface(&surface).map(|(sf, w)| w && sf == "Wood").unwrap_or(false) && std::env::var("TINY_WET_SNOW_PENALTY").map(|v| v != "0").unwrap_or(true);
            if (!surface.to_ascii_lowercase().contains("ice") && !wet_snow) || tmmaps::cli::has(args, "--keep-trees") {
                sw.arg("--keep-trees");
            }
            // a wet surface: the pad goes under a start ITEM too (no block row to mark)
            if mapgeom::surface::parse_surface(&surface).map(|(_, wet)| wet).unwrap_or(false) {
                sw.arg("--wet");
            }
            if with_items {
                sw.arg("--library").arg(&lib).arg("--mapping").arg(&mapping);
            }
            if tmmaps::cli::has(args, "--strip-lightmap") {
                sw.arg("--strip-lightmap");
            }
            let text = run(&mut sw, &dir.join("surface.log"))?;
            for l in text.lines().filter(|l| l.starts_with("surface:") || l.contains("stage 3b") || l.contains("stay blocks") || l.starts_with("wrote") || l.trim_start().starts_with("trees:")) {
                println!("  {}", l.trim());
            }
            Ok(())
        };
        // --max-bytes N (the 7 MB server cap): a map over it is rebuilt one detail
        // level coarser at a time (lod-pick 1, 2, 3) until it fits; the note says which
        let max_bytes: Option<u64> = f("--max-bytes").and_then(|v| v.parse().ok());
        let mut build = build_with(None);
        let mut fit_note = String::new();
        if build.is_ok() && !reuse {
            if let Some(cap) = max_bytes {
                let size = || std::fs::metadata(&out_map).map(|m| m.len()).unwrap_or(u64::MAX);
                let start = size();
                if start >= cap {
                    let mut fitted = false;
                    for level in ["1", "2", "3"] {
                        build = build_with(Some(level));
                        if build.is_err() {
                            break;
                        }
                        let s = size();
                        println!("{nn}: {start} B over the {cap} B cap; every baked part at detail level {level}: {s} B");
                        if s < cap {
                            fit_note = format!("fit under {cap} B: {start} -> {s} B (lod-pick {level}); ");
                            fitted = true;
                            break;
                        }
                    }
                    if !fitted && build.is_ok() {
                        fit_note = format!("DOES NOT FIT under {cap} B even at level 3 ({} B); ", size());
                    }
                }
            }
        }
        note.push_str(&fit_note);
        if let Err(e) = build {
            eprintln!("{nn}: build FAILED: {e}");
            row.extend(vec!["-".into(); 9]);
            row.push(format!("{:.0}", build_t.elapsed().as_secs_f64()));
            row.extend(["-".into(), "-".into(), "-".into(), format!("FAILED build: {}", first_line(&e))]);
            append(&tracker, &row)?;
            failed += 1;
            continue;
        }
        let (tally, names) = plan_tally(&records)?;
        if bake_models.is_empty() {
            bake_models = names;
        }
        let ohdr = match tmmaps::header::read(&out_map.display().to_string()) {
            Ok(h) => h,
            Err(e) => {
                eprintln!("{nn}: output header: {e}");
                row.extend(vec!["-".into(); 9]);
                row.push(format!("{:.0}", build_t.elapsed().as_secs_f64()));
                row.extend(["-".into(), "-".into(), "-".into(), format!("FAILED output header: {}", first_line(&e))]);
                append(&tracker, &row)?;
                failed += 1;
                continue;
            }
        };
        let bytes = std::fs::metadata(&out_map).map(|m| m.len()).unwrap_or(0);
        let get = |k: &str| tally.get(k).copied().unwrap_or(0).to_string();
        row.push(ohdr.name.clone());
        row.push(ohdr.uid.clone());
        row.push(bytes.to_string());
        row.push(get("rename"));
        row.push(get("bake"));
        row.push(get("same"));
        row.push(get("keep"));
        row.push(get("none"));
        row.push(bake_models.join(","));
        row.push(format!("{:.0}", build_t.elapsed().as_secs_f64()));
        if ohdr.authortime != hdr.authortime || ohdr.gold != hdr.gold || ohdr.silver != hdr.silver || ohdr.bronze != hdr.bronze {
            note.push_str(&format!("TIMES CHANGED ({} {} {} {} -> {} {} {} {}); ", hdr.authortime, hdr.gold, hdr.silver, hdr.bronze, ohdr.authortime, ohdr.gold, ohdr.silver, ohdr.bronze));
        }
        if bytes >= 7_000_000 {
            note.push_str(&format!("OVER THE 7 MB SERVER CAP ({bytes} B); "));
        }
        println!("  {} ({} B): {} renamed, {} baked, {} same, {} kept, {} none", ohdr.name, bytes, get("rename"), get("bake"), get("same"), get("keep"), get("none"));

        // 4. views + shoot (start, finish, top; the identity anchor at scale 1)
        let mut frames = "-".to_string();
        let mut flagged = "-".to_string();
        if !no_shoot {
            let views_all = dir.join("views-all.tsv");
            let views = dir.join("views.tsv");
            match crate::views::cmd(&[src.display().to_string(), "--out".into(), views_all.display().to_string()]) {
                Ok(()) => {
                    let keep: Vec<&str> = f("--views-only").map(|s| s.split(',').map(|x| x.trim().to_string()).collect::<Vec<_>>()).unwrap_or_else(|| vec!["start".into(), "finish".into(), "top".into()]).leak().iter().map(|s| s.as_str()).collect();
                    let text = std::fs::read_to_string(&views_all).map_err(|e| format!("{}: {e}", views_all.display()))?;
                    let kept: Vec<&str> = text.lines().filter(|l| l.starts_with('#') || l.trim().is_empty() || keep.contains(&l.split('\t').next().unwrap_or("").trim())).collect();
                    std::fs::write(&views, kept.join("\n") + "\n").map_err(|e| format!("{}: {e}", views.display()))?;
                    // the anchor line is the tiny one (target y scaled); the swap is 1:1
                    let anchor = text.lines().find(|l| l.starts_with("# anchor ")).map(|l| l.trim_start_matches("# anchor ").trim().to_string()).and_then(|a| a.split(':').next().map(|s| s.to_string()));
                    match anchor {
                        Some(a) => {
                            let stag = format!("{surface}{nn}");
                            let mut sargs: Vec<String> = vec!["--orig".into(), src.display().to_string(), "--tiny".into(), out_map.display().to_string(), "--views".into(), views.display().to_string(), "--tag".into(), stag.clone(), "--anchor".into(), format!("{a}:{a}"), "--scale".into(), "1".into(), "--outdir".into(), frames_dir.display().to_string()];
                            if tmmaps::cli::has(args, "--fresh") {
                                sargs.push("--fresh".into());
                            }
                            for k in ["--wsx", "--box-shootctl", "--box-tinyctl"] {
                                if let Some(v) = f(k) {
                                    sargs.push(k.into());
                                    sargs.push(v);
                                }
                            }
                            // --orig-frames-box DIR: a directory ON THE BOX holding the ORIGINAL side's
                            // frames (`cmp-origNN<view>-o.png`). The source map and its cameras are the
                            // same for every surface variant, so the original is shot ONCE: when the
                            // frames for this map and these views are there, only the variant side is
                            // shot (`--only t`) and the cached frames stand in for the comparison;
                            // otherwise both sides are shot and the original's frames are cached.
                            let wsx = crate::wsx::Wsx::new(args);
                            let remote_dir = format!("/mnt/c/Users/vjeux/tinyshots/{stag}");
                            let cache = f("--orig-frames-box");
                            let mut cached = false;
                            if let Some(cache) = &cache {
                                let checks: Vec<String> = keep.iter().map(|v| format!("test -f {cache}/cmp-orig{nn}{v}-o.png")).collect();
                                if wsx.sh(&format!("{} && echo CACHED", checks.join(" && "))).map(|s| s.contains("CACHED")).unwrap_or(false) {
                                    let cps: Vec<String> = keep.iter().map(|v| format!("cp -f {cache}/cmp-orig{nn}{v}-o.png {remote_dir}/cmp-{stag}{v}-o.png")).collect();
                                    wsx.sh(&format!("mkdir -p {remote_dir} && {}", cps.join(" && ")))?;
                                    sargs.push("--only".into());
                                    sargs.push("t".into());
                                    cached = true;
                                    println!("  original frames from the box cache {cache} (variant side only)");
                                }
                            }
                            match crate::shoot::cmd(&sargs) {
                                Ok(()) => {
                                    if let (Some(cache), false) = (&cache, cached) {
                                        let cps: Vec<String> = keep.iter().map(|v| format!("cp -f {remote_dir}/cmp-{stag}{v}-o.png {cache}/cmp-orig{nn}{v}-o.png")).collect();
                                        if let Err(e) = wsx.sh(&format!("mkdir -p {cache} && {}", cps.join(" && "))) {
                                            eprintln!("  original frames not cached: {e}");
                                        }
                                    }
                                    let n_jpg = std::fs::read_dir(&frames_dir).map(|rd| rd.filter(|e| e.as_ref().map(|e| e.file_name().to_string_lossy().starts_with(&format!("cmp-{stag}")) && e.file_name().to_string_lossy().ends_with(".jpg")).unwrap_or(false)).count()).unwrap_or(0);
                                    frames = format!("{n_jpg} pairs");
                                    let tsv = frames_dir.join(format!("cmpdiff-{stag}.tsv"));
                                    flagged = std::fs::read_to_string(&tsv).map(|t| t.lines().filter(|l| !l.trim().is_empty() && !l.starts_with('#') && !l.starts_with("view")).count().to_string()).unwrap_or_else(|_| "?".into());
                                }
                                Err(e) => {
                                    frames = "FAILED".into();
                                    note.push_str(&format!("shoot: {}; ", first_line(&e)));
                                }
                            }
                            if !tmmaps::cli::has(args, "--keep-box-files") {
                                let wsx = crate::wsx::Wsx::new(args);
                                let _ = wsx.sh(&format!("rm -rf /mnt/c/Users/vjeux/tinyshots/{stag} /home/vjeux/shoot/_stage/{stag}Orig.Map.Gbx /home/vjeux/shoot/_stage/{stag}Tiny.Map.Gbx /home/vjeux/shoot/_stage/{stag}-views.tsv '/mnt/c/Users/vjeux/OneDrive/Documents/Trackmania/Maps/_shoot/{stag}Orig.Map.Gbx' '/mnt/c/Users/vjeux/OneDrive/Documents/Trackmania/Maps/_shoot/{stag}Tiny.Map.Gbx'"));
                            }
                        }
                        None => note.push_str("views: no anchor line; shoot skipped; "),
                    }
                }
                Err(e) => note.push_str(&format!("views: {}; ", first_line(&e))),
            }
        }
        row.push(frames);
        row.push(flagged);

        // 5. publish
        let mut nadeo = "-".to_string();
        if !no_publish {
            let n: usize = nn.parse().unwrap_or(0);
            let libx_items = dir.join("libx").join("Items");
            let mut pargs: Vec<String> = vec![nn.clone(), "--map".into(), out_map.display().to_string(), "--name".into(), ohdr.name.clone(), "--club".into(), club.clone(), "--campaign".into(), campaign.clone(), "--campaign-name".into(), campaign_name.clone(), "--position".into(), n.saturating_sub(1).to_string(), "--outdir".into(), dir.display().to_string(), "--any-uid".into()];
            if libx_items.exists() {
                pargs.push("--items-dir".into());
                pargs.push(libx_items.display().to_string());
                pargs.push("--paks".into());
                pargs.push(paks.join(" "));
            }
            for k in ["--wsx", "--box-tinyctl"] {
                if let Some(v) = f(k) {
                    pargs.push(k.into());
                    pargs.push(v);
                }
            }
            match crate::publish::publish_one(n, &pargs, None) {
                Ok(line) => {
                    let toks: Vec<&str> = line.split_whitespace().collect();
                    let map_id = toks.iter().position(|t| *t == "mapId").and_then(|i| toks.get(i + 1)).map(|s| s.to_string()).unwrap_or_else(|| "?".into());
                    let how = if line.contains("\tCREATE\t") { "created" } else if line.contains("\tUPDATE\t") { "updated" } else { "uploaded" };
                    nadeo = format!("OK {how} mapId {map_id} {}", if line.contains("stored IDENTICAL") { "stored IDENTICAL" } else { "stored ?" });
                }
                Err(e) => {
                    nadeo = format!("FAILED: {}", first_line(&e));
                    failed += 1;
                }
            }
            if !tmmaps::cli::has(args, "--keep-box-files") {
                let wsx = crate::wsx::Wsx::new(args);
                let _ = wsx.sh(&format!("rm -f /mnt/c/Users/vjeux/tinyshots/publish-{nn}/readback-*.Map.Gbx /home/vjeux/shoot/_stage/Tiny{nn}.Map.Gbx"));
            }
        }
        row.push(nadeo);
        row.push(note.trim_end_matches("; ").to_string());
        append(&tracker, &row)?;
        println!("{nn}: done in {:.0} s", t0.elapsed().as_secs_f64());
    }
    println!("\ntracker: {}", tracker.display());
    if failed > 0 {
        return Err(format!("{failed} map(s) failed"));
    }
    Ok(())
}

/// `tinyctl surface-report --tracker T.tsv --out-root DIR --frames-dir DIR [--uploads uploads.txt]
/// [--out REPORT.md] [--surface ice] [--campaign-url URL]` — the tracker as a
/// Markdown report: one summary table (the LAST row per map wins), then a
/// section per map with its three before/after frames embedded (uploaded
/// through `tinyctl upload` unless `--uploads` already lists `cmp-<surface>NN<view>`)
/// and the block accounting from the map's plan: which models were renamed to
/// which twin, which were baked as re-dressed copies, which stayed and why.
pub fn report_cmd(args: &[String]) -> Result<(), String> {
    let f = |k: &str| tmmaps::cli::flag(args, k).map(String::from);
    let surface = f("--surface").unwrap_or_else(|| "ice".into());
    let (word, uid_prefix) = surface_label(&surface).unwrap_or_else(|_| ({ let mut c = surface.chars(); c.next().map(|x| x.to_ascii_uppercase().to_string() + c.as_str()).unwrap_or_default() }, format!("{}2", &surface[..surface.len().min(3)])));
    let lower = word.to_ascii_lowercase();
    // --intro FILE: a Markdown block placed after the title (the variant's recipe and play proof)
    let intro = f("--intro").and_then(|p| std::fs::read_to_string(p).ok()).unwrap_or_default();
    let out_root = PathBuf::from(f("--out-root").unwrap_or_else(|| format!("/tmp/surface-{surface}")));
    let tracker = PathBuf::from(f("--tracker").unwrap_or_else(|| out_root.join("tracker.tsv").to_string_lossy().into_owned()));
    let frames_dir = PathBuf::from(f("--frames-dir").unwrap_or_else(|| out_root.join("frames").to_string_lossy().into_owned()));
    let uploads_path = PathBuf::from(f("--uploads").unwrap_or_else(|| out_root.join("uploads.txt").to_string_lossy().into_owned()));
    let out = PathBuf::from(f("--out").unwrap_or_else(|| out_root.join("REPORT.md").to_string_lossy().into_owned()));
    let text = std::fs::read_to_string(&tracker).map_err(|e| format!("{}: {e}", tracker.display()))?;
    let mut lines = text.lines();
    let head: Vec<String> = lines.next().unwrap_or("").split('\t').map(|s| s.to_string()).collect();
    let col_of = |n: &str| head.iter().position(|h| h == n).unwrap_or_else(|| panic!("tracker: no column `{n}`"));
    let mut by_nn: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for l in lines.filter(|l| !l.trim().is_empty()) {
        let r: Vec<String> = l.split('\t').map(|s| s.to_string()).collect();
        by_nn.insert(r[0].clone(), r);
    }
    let col = |r: &[String], n: &str| r.get(col_of(n)).cloned().unwrap_or_else(|| "-".into());
    // uploads: NAME ID rows; missing frames are uploaded now
    let mut uploads: BTreeMap<String, String> = std::fs::read_to_string(&uploads_path).ok().map(|t| t.lines().filter_map(|l| l.split_once(char::is_whitespace).map(|(a, b)| (a.trim().to_string(), b.trim().to_string()))).collect()).unwrap_or_default();
    let views = ["start", "finish", "top"];
    let no_upload = tmmaps::cli::has(args, "--no-upload");
    for nn in by_nn.keys() {
        for v in views {
            let key = format!("cmp-{surface}{nn}{v}");
            if uploads.contains_key(&key) || no_upload {
                continue;
            }
            let p = frames_dir.join(format!("{key}.jpg"));
            if !p.exists() {
                continue;
            }
            match crate::upload::cmd(&[p.display().to_string(), "--record".into(), uploads_path.display().to_string()]) {
                Ok(()) => {
                    // the recorder appended `NAME ID`; read it back
                    if let Some(id) = std::fs::read_to_string(&uploads_path).ok().and_then(|t| t.lines().filter_map(|l| l.split_once(char::is_whitespace).map(|(a, b)| (a.trim().to_string(), b.trim().to_string()))).filter(|(a, _)| *a == key).last().map(|(_, b)| b)) {
                        uploads.insert(key, id);
                    }
                }
                Err(e) => eprintln!("upload {key}: {e}"),
            }
        }
    }
    let secs = |ms: &str| ms.parse::<f64>().map(|v| format!("{:.3}", v / 1000.0)).unwrap_or_else(|_| ms.to_string());
    let mut md = String::new();
    md.push_str(&format!("# {word} U10S — the first 25 U10S maps re-surfaced as {lower}\n\n"));
    if !intro.is_empty() {
        md.push_str(intro.trim_end());
        md.push_str("\n\n");
    }
    if let Some(u) = f("--campaign-url") {
        md.push_str(&format!("Campaign: {u}\n\n"));
    }
    let tot = |n: &str| by_nn.values().map(|r| col(r, n).parse::<usize>().unwrap_or(0)).sum::<usize>();
    md.push_str(&format!("{} maps · {} block records renamed to their {lower} twin · {} baked as re-dressed item copies · {} already {lower} · {} left as they are (pillars, structures, gates, decoration) · {} with no {lower} form at all.\n\n", by_nn.len(), tot("renamed"), tot("baked"), tot("same"), tot("kept"), tot("none")));
    md.push_str(&format!("| # | source | AT / gold / silver / bronze (s) | {lower} map | size | renamed | baked | same | kept | none | frames | Nadeo |\n|---|---|---|---|---|---|---|---|---|---|---|---|\n"));
    for (nn, r) in &by_nn {
        let times = format!("{} / {} / {} / {}", secs(&col(r, "authortime_ms")), secs(&col(r, "gold")), secs(&col(r, "silver")), secs(&col(r, "bronze")));
        let size = col(r, "out_bytes").parse::<f64>().map(|b| format!("{:.2} MB", b / 1e6)).unwrap_or_else(|_| col(r, "out_bytes"));
        let nadeo = col(r, "nadeo").replace("OK created mapId ", "created `").replace("OK updated mapId ", "updated `").replace(" stored IDENTICAL", "` · md5 identical");
        // frames: what is on disk (a publish-only re-run's row says "-")
        let n_frames = views.iter().filter(|v| frames_dir.join(format!("cmp-{surface}{nn}{v}.jpg")).exists()).count();
        let frames = if n_frames > 0 { format!("{n_frames} pairs") } else { col(r, "frames") };
        md.push_str(&format!("| {nn} | {} `{}` | {times} | {} `{}` | {size} | {} | {} | {} | {} | {} | {frames} | {nadeo} |\n", col(r, "source_name"), col(r, "source_uid"), col(r, "out_name"), col(r, "out_uid"), col(r, "renamed"), col(r, "baked"), col(r, "same"), col(r, "kept"), col(r, "none")));
    }
    md.push_str(&format!("\nEvery map keeps the source's medal times, items, MediaTracker and thumbnail; the uid is `{uid_prefix}` + the source uid's tail; the validation ghost (an author run on another surface) is removed.\n\n"));
    for (nn, r) in &by_nn {
        md.push_str(&format!("## {nn} — {} → {}\n\n", col(r, "source_name"), col(r, "out_name")));
        let note = col(r, "note");
        if !note.is_empty() && note != "-" {
            md.push_str(&format!("Note: {note}\n\n"));
        }
        for v in views {
            let key = format!("cmp-{surface}{nn}{v}");
            match uploads.get(&key) {
                Some(id) => md.push_str(&format!("**{v}** (left: original · right: {surface})\n\n![{key}](/api/attachments/view?file_id={id})\n\n")),
                None => {}
            }
        }
        // the trees (tmmaps surface's `trees:` line in surface.log)
        if let Some(l) = std::fs::read_to_string(out_root.join(format!("{surface}{nn}")).join("surface.log")).ok().and_then(|t| t.lines().find(|l| l.trim_start().starts_with("trees:")).map(|l| l.trim().to_string())) {
            let body = l.trim_start_matches("trees:").trim();
            md.push_str(&format!("**Trees:** {}\n\n", body.replace(" -> ", " → ")));
        }
        // the block accounting from models.tsv
        let models = out_root.join(format!("{surface}{nn}")).join("models.tsv");
        if let Ok(t) = std::fs::read_to_string(&models) {
            let mut groups: BTreeMap<&str, Vec<String>> = BTreeMap::new();
            for l in t.lines().skip(1).filter(|l| !l.trim().is_empty()) {
                let c: Vec<&str> = l.split('\t').collect();
                let (name, count, action, target, detail) = (c[0], c[1], c[2], c.get(3).copied().unwrap_or(""), c.get(5).copied().unwrap_or(""));
                let entry = match action {
                    "rename" => format!("{name} → {target} ×{count}"),
                    "bake" => format!("{name} ×{count} ({})", detail.split(" — ").next().unwrap_or(detail).replace("different mesh: ", "")),
                    "same" => format!("{name} ×{count}"),
                    "none" => format!("{name} ×{count} ({detail})"),
                    _ => format!("{name} ×{count}"),
                };
                groups.entry(action).or_default().push(entry);
            }
            for (action, label) in [("rename", format!("Renamed to the {lower} twin")), ("bake", format!("Baked as re-dressed item copies (no same-shape {lower} block)")), ("same", format!("Already {lower}")), ("none", format!("No {lower} form — left as is")), ("keep", "Not a surface (left as is)".to_string())] {
                if let Some(v) = groups.get(action) {
                    md.push_str(&format!("**{label}:** {}\n\n", v.join(" · ")));
                }
            }
        }
    }
    std::fs::write(&out, &md).map_err(|e| format!("{}: {e}", out.display()))?;
    println!("wrote {} ({} maps, {} frames linked)", out.display(), by_nn.len(), uploads.len());
    Ok(())
}

/// `tinyctl surface-all --src-root S --out-root O --parts 01-39 [--jobs 12] [--surface ice]
/// [--dist DIR]` — every part directory `S/pNN/` (maps `NN-<name>.Map.Gbx`)
/// converted by `surface` (build only: no shoot, no publish; the club's naming
/// `U10S_N By Everios96 [Ice]`; item names unique per part), N parts at a time,
/// one tracker per part (`O/pNN/tracker.tsv`), then one zip per part in `--dist`
/// (`<Word>U10S-partNN.zip`: the 25 maps in campaign order + `MAPS.tsv`) and a
/// summary `O/CONVERT.tsv` — the GitHub-release layout of the tiny set
/// (2026-09-13, vjeux: "the 1k giant and icy uploaded to github like you did for tiny").
/// `tinyctl surface-round --src-root S --out-root O --maps FILE|N,N,… --sets a,b [--jobs 6]
/// [--env K=V]… [--label L] [--max-bytes B]` — a FIX ROUND: the listed maps (numbers, or a file
/// with one `NN [anything]` per line) built with `surface` for every set, N (set, part)
/// pairs at a time, and each result copied flat into `O/stage/<set>/pNN/<name>.Map.Gbx` —
/// the layout `tinyctl round-publish --stage O/stage` publishes in place from the devserver.
/// `--env` sets process variables the plan/library read (TINY_WATER_OVERLAY=1, …).
/// Rust end to end (vjeux 2026-09-24: no shell scripts).
pub fn round_cmd(args: &[String]) -> Result<(), String> {
    let f = |k: &str| tmmaps::cli::flag(args, k).map(String::from);
    let src_root = PathBuf::from(f("--src-root").ok_or("surface-round needs --src-root S (S/pNN/NN-*.Map.Gbx)")?);
    let out_root = PathBuf::from(f("--out-root").ok_or("surface-round needs --out-root O")?);
    let sets: Vec<String> = f("--sets").ok_or("surface-round needs --sets a,b")?.split(',').map(|x| x.trim().to_string()).filter(|x| !x.is_empty()).collect();
    let maps_arg = f("--maps").ok_or("surface-round needs --maps FILE or N,N,…")?;
    let mut nums: Vec<usize> = if Path::new(&maps_arg).is_file() {
        std::fs::read_to_string(&maps_arg).map_err(|e| format!("{maps_arg}: {e}"))?.lines().filter_map(|l| l.split_whitespace().next().and_then(|w| w.parse().ok())).collect()
    } else {
        maps_arg.split(',').filter_map(|w| w.trim().parse().ok()).collect()
    };
    nums.sort();
    nums.dedup();
    if nums.is_empty() {
        return Err("surface-round: no map numbers".into());
    }
    let jobs: usize = f("--jobs").and_then(|j| j.parse().ok()).unwrap_or(6).max(1);
    // --env K=V (repeatable): the plan and the library read these
    let mut i = 0;
    while i + 1 < args.len() {
        if args[i] == "--env" {
            if let Some((k, v)) = args[i + 1].split_once('=') {
                std::env::set_var(k, v);
                println!("env {k}={v}");
            }
        }
        i += 1;
    }
    // (set, part) → the part's listed maps
    let mut work: Vec<(String, String, Vec<String>)> = Vec::new();
    for set in &sets {
        let mut by_part: std::collections::BTreeMap<String, Vec<String>> = std::collections::BTreeMap::new();
        // map N lives in part (N-1)/25+1 as its two-digit slot ((N-1)%25+1): U10S_37 = p02/12
        for n in &nums {
            let part = format!("{:02}", (n - 1) / 25 + 1);
            by_part.entry(part).or_default().push(format!("{:02}", (n - 1) % 25 + 1));
        }
        for (part, ms) in by_part {
            work.push((set.clone(), part, ms));
        }
    }
    let total = work.len();
    let queue = std::sync::Mutex::new(std::collections::VecDeque::from(work));
    let results = std::sync::Mutex::new(Vec::<String>::new());
    let t0 = Instant::now();
    std::thread::scope(|sc| {
        for _ in 0..jobs.min(total) {
            sc.spawn(|| loop {
                let (set, part, ms) = match queue.lock().unwrap().pop_front() {
                    Some(w) => w,
                    None => break,
                };
                let src_dir = src_root.join(format!("p{part}"));
                let build_out = out_root.join("build").join(&set).join(format!("p{part}"));
                let stage_out = out_root.join("stage").join(&set).join(format!("p{part}"));
                let _ = std::fs::create_dir_all(&build_out);
                let _ = std::fs::create_dir_all(&stage_out);
                let t1 = Instant::now();
                let mut pargs: Vec<String> = ms.clone();
                pargs.extend(["--src-dir".to_string(), src_dir.display().to_string(), "--out-root".into(), build_out.display().to_string(), "--surface".into(), set.clone(), "--name-style".into(), "everios".into(), "--no-publish".into(), "--no-shoot".into()]);
                if set == "ice" {
                    pargs.extend(["--label".to_string(), "Icy".into()]);
                }
                for k in ["--lod-pick", "--max-bytes"] {
                    if let Some(v) = f(k) {
                        pargs.push(k.into());
                        pargs.push(v);
                    }
                }
                let r = cmd(&pargs);
                // flat copies for round-publish
                let mut copied = 0usize;
                for nn in &ms {
                    let dir = build_out.join(format!("{set}{nn}"));
                    if let Ok(rd) = std::fs::read_dir(&dir) {
                        for e in rd.flatten() {
                            let p = e.path();
                            if p.to_string_lossy().ends_with(".Map.Gbx") && std::fs::copy(&p, stage_out.join(p.file_name().unwrap())).is_ok() {
                                copied += 1;
                            }
                        }
                    }
                }
                let line = match r {
                    Ok(()) => format!("{set}\tp{part}\tOK\t{copied}/{} maps\t{:.0}s", ms.len(), t1.elapsed().as_secs_f64()),
                    Err(e) => format!("{set}\tp{part}\tPARTIAL\t{copied}/{} maps\t{:.0}s\t{}", ms.len(), t1.elapsed().as_secs_f64(), e.lines().next().unwrap_or("")),
                };
                eprintln!("[{:>5.0}s] {line}", t0.elapsed().as_secs_f64());
                results.lock().unwrap().push(line);
            });
        }
    });
    let mut lines = results.into_inner().unwrap();
    lines.sort();
    let summary = out_root.join("ROUND.tsv");
    std::fs::write(&summary, lines.join("\n") + "\n").map_err(|e| format!("{}: {e}", summary.display()))?;
    let staged = sets.iter().map(|s| std::fs::read_dir(out_root.join("stage").join(s)).map(|rd| rd.flatten().filter_map(|p| std::fs::read_dir(p.path()).ok()).map(|r| r.flatten().count()).sum::<usize>()).unwrap_or(0)).sum::<usize>();
    println!("surface-round: {} (set, part) jobs, {staged} maps staged under {} in {:.0}s — see {}", total, out_root.join("stage").display(), t0.elapsed().as_secs_f64(), summary.display());
    let bad = lines.iter().filter(|l| !l.contains("\tOK\t")).count();
    if bad > 0 {
        return Err(format!("{bad} of {total} jobs not fully built — see {}", summary.display()));
    }
    Ok(())
}

pub fn all_cmd(args: &[String]) -> Result<(), String> {
    let f = |k: &str| tmmaps::cli::flag(args, k).map(String::from);
    let src_root = PathBuf::from(f("--src-root").ok_or("surface-all needs --src-root S")?);
    let out_root = PathBuf::from(f("--out-root").ok_or("surface-all needs --out-root O")?);
    let surface = f("--surface").unwrap_or_else(|| "ice".into());
    // the zip / release word is the variant's LABEL without spaces (`WetWood`, `Ice`), not the flag spelling
    let word = surface_label(&surface).map(|(l, _)| l.replace(' ', "")).unwrap_or_else(|_| { let mut c = surface.chars(); c.next().map(|x| x.to_ascii_uppercase().to_string() + c.as_str()).unwrap_or_default() });
    let dist = PathBuf::from(f("--dist").unwrap_or_else(|| out_root.join("dist").to_string_lossy().into_owned()));
    let parts_arg = f("--parts").ok_or("surface-all needs --parts 01,02,… or 01-39")?;
    let parts: Vec<String> = if let Some((a, b)) = parts_arg.split_once('-') {
        let (a, b): (usize, usize) = (a.trim().parse().map_err(|_| "--parts A-B")?, b.trim().parse().map_err(|_| "--parts A-B")?);
        (a..=b).map(|n| format!("{n:02}")).collect()
    } else {
        parts_arg.split(',').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect()
    };
    let jobs: usize = f("--jobs").and_then(|j| j.parse().ok()).unwrap_or(8).max(1);
    std::fs::create_dir_all(&out_root).map_err(|e| format!("{}: {e}", out_root.display()))?;
    std::fs::create_dir_all(&dist).map_err(|e| format!("{}: {e}", dist.display()))?;
    let queue = std::sync::Mutex::new(std::collections::VecDeque::from(parts.clone()));
    let results = std::sync::Mutex::new(Vec::<String>::new());
    let t0 = Instant::now();
    std::thread::scope(|s| {
        for _ in 0..jobs.min(parts.len()) {
            s.spawn(|| loop {
                let part = match queue.lock().unwrap().pop_front() {
                    Some(p) => p,
                    None => break,
                };
                let src_dir = src_root.join(format!("p{part}"));
                let part_out = out_root.join(format!("p{part}"));
                let _ = std::fs::create_dir_all(&part_out);
                let mut nums: Vec<String> = std::fs::read_dir(&src_dir)
                    .map(|rd| rd.filter_map(|e| e.ok()).filter_map(|e| e.file_name().to_string_lossy().split('-').next().filter(|s| s.len() == 2 && s.chars().all(|c| c.is_ascii_digit())).map(|s| s.to_string())).collect())
                    .unwrap_or_default();
                nums.sort();
                nums.dedup();
                let t1 = Instant::now();
                let line = if nums.is_empty() {
                    format!("{part}\tFAILED\tno NN-*.Map.Gbx in {}", src_dir.display())
                } else {
                    let mut pargs: Vec<String> = nums.clone();
                    if let Some(l) = f("--label") {
                        pargs.push("--label".into());
                        pargs.push(l);
                    }
                    pargs.extend(["--src-dir".to_string(), src_dir.display().to_string(), "--out-root".into(), part_out.display().to_string(), "--surface".into(), surface.clone(), "--tracker".into(), part_out.join("tracker.tsv").display().to_string(), "--alias-part".into(), part.trim_start_matches('0').to_string(), "--name-style".into(), "everios".into(), "--no-shoot".into(), "--no-publish".into()]);
                    if tmmaps::cli::has(args, "--reuse-build") {
                        pargs.push("--reuse-build".into());
                    }
                    for k in ["--lod-pick", "--bin-dir", "--max-bytes"] {
                        if let Some(v) = f(k) {
                            pargs.push(k.into());
                            pargs.push(v);
                        }
                    }
                    let r = cmd(&pargs);
                    // the part's zip: the maps in campaign order + MAPS.tsv
                    let z = part_zip(&part_out, &part, &word, &surface, &dist);
                    match (r, z) {
                        (Ok(()), Ok((n, bytes, over))) => format!("{part}\tOK\t{n}/{} maps\t{bytes} B zip\t{over} over 7 MB\t{:.0}s", nums.len(), t1.elapsed().as_secs_f64()),
                        (Err(e), Ok((n, bytes, over))) => format!("{part}\tPARTIAL\t{n}/{} maps\t{bytes} B zip\t{over} over 7 MB\t{:.0}s\t{}", nums.len(), t1.elapsed().as_secs_f64(), e.lines().next().unwrap_or("")),
                        (_, Err(e)) => format!("{part}\tFAILED\tzip: {e}"),
                    }
                };
                eprintln!("[{:>5.0}s] {line}", t0.elapsed().as_secs_f64());
                results.lock().unwrap().push(line);
            });
        }
    });
    let mut lines = results.into_inner().unwrap();
    lines.sort();
    let summary = out_root.join("CONVERT.tsv");
    std::fs::write(&summary, lines.join("\n") + "\n").map_err(|e| format!("{}: {e}", summary.display()))?;
    println!("{}", lines.join("\n"));
    let bad = lines.iter().filter(|l| !l.contains("\tOK\t")).count();
    if bad > 0 {
        return Err(format!("{bad} of {} parts not fully built — see {}", parts.len(), summary.display()));
    }
    Ok(())
}

/// One part's release zip from its tracker: `<Word>U10S-partNN.zip` with the
/// maps (the tracker's LAST row per map, in map order) and `MAPS.tsv`.
/// Returns (maps in the zip, zip bytes, maps over 7 000 000 B).
/// `Prefix*`, `*Suffix`, `*Infix*` or the exact name.
fn glob_simple(pat: &str, name: &str) -> bool {
    match (pat.strip_prefix('*'), pat.strip_suffix('*')) {
        (Some(rest), Some(_)) => name.contains(rest.trim_end_matches('*')),
        (Some(suffix), None) => name.ends_with(suffix),
        (None, Some(prefix)) => name.starts_with(prefix),
        (None, None) => name == pat,
    }
}

fn part_zip(part_out: &Path, part: &str, word: &str, surface_flag: &str, dist: &Path) -> Result<(usize, u64, usize), String> {
    let tracker = part_out.join("tracker.tsv");
    let text = std::fs::read_to_string(&tracker).map_err(|e| format!("{}: {e}", tracker.display()))?;
    let mut lines = text.lines();
    let head: Vec<&str> = lines.next().unwrap_or("").split('\t').collect();
    let col = |r: &[String], n: &str| head.iter().position(|h| *h == n).and_then(|i| r.get(i).cloned()).unwrap_or_else(|| "-".into());
    let mut by_nn: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for l in lines.filter(|l| !l.trim().is_empty()) {
        let r: Vec<String> = l.split('\t').map(|s| s.to_string()).collect();
        by_nn.insert(r[0].clone(), r);
    }
    let mut files: BTreeMap<String, Vec<u8>> = BTreeMap::new();
    let mut tsv = String::from("nn\tfile\tsource_name\tsource_uid\tauthortime_ms\tgold\tsilver\tbronze\tuid\tbytes\trenamed\tbaked\tsame\tkept\tnone\tnote\n");
    let (mut n, mut over) = (0usize, 0usize);
    // the per-map dir is `<--surface flag><nn>` (`woodwet01`), not the label
    // (`wetwood01`): the wet-wood zips failed on every part until this used the
    // flag (2026-09-14); the label spelling is tried second for older trees
    let surface = surface_flag.to_ascii_lowercase();
    let word_lc = word.to_ascii_lowercase();
    for (nn, r) in &by_nn {
        let name = col(r, "out_name");
        if name == "-" {
            tsv.push_str(&format!("{nn}\t-\t{}\t{}\t{}\t{}\t{}\t{}\t-\t-\t-\t-\t-\t-\t-\t{}\n", col(r, "source_name"), col(r, "source_uid"), col(r, "authortime_ms"), col(r, "gold"), col(r, "silver"), col(r, "bronze"), col(r, "note")));
            continue;
        }
        let file = format!("{name}.Map.Gbx");
        let mut path = part_out.join(format!("{surface}{nn}")).join(&file);
        if !path.exists() {
            path = part_out.join(format!("{word_lc}{nn}")).join(&file);
        }
        let bytes = std::fs::read(&path).map_err(|e| format!("{}: {e}", path.display()))?;
        if bytes.len() >= 7_000_000 {
            over += 1;
        }
        n += 1;
        // the zip entry is the map's own file name — `U10S_N By Everios96 [Wood].Map.Gbx`,
        // file name == map name (Everios96's convention; the `NN-` order prefix
        // he asked to drop, 2026-09-14). MAPS.tsv's `nn` column keeps the order.
        files.insert(file.clone(), bytes.clone());
        tsv.push_str(&format!("{nn}\t{file}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\n", col(r, "source_name"), col(r, "source_uid"), col(r, "authortime_ms"), col(r, "gold"), col(r, "silver"), col(r, "bronze"), col(r, "out_uid"), bytes.len(), col(r, "renamed"), col(r, "baked"), col(r, "same"), col(r, "kept"), col(r, "none"), col(r, "note")));
    }
    files.insert("MAPS.tsv".into(), tsv.into_bytes());
    // the maps are LZO-compressed GBX already: store them (deflate gains ~1 %
    // and costs a minute per part) — the tmmaps writer deflates; keep it simple
    let zip = tmmaps::header::deflated_zip(&files);
    let out = dist.join(format!("{word}U10S-part{part}.zip"));
    std::fs::write(&out, &zip).map_err(|e| format!("{}: {e}", out.display()))?;
    std::fs::write(part_out.join("MAPS.tsv"), files.get("MAPS.tsv").cloned().unwrap_or_default()).map_err(|e| format!("{}: {e}", part_out.display()))?;
    Ok((n, zip.len() as u64, over))
}

/// `tinyctl release-zips --dist DIR --tag TAG --title T --notes README.md [--repo vjeux/trackmania-tas]
/// [--box-dir /home/vjeux/shoot/ice] [--wsx P]` — the GitHub release of a `surface-all` dist directory,
/// driven from the devserver but run ON THE RENDER BOX (the only host with `gh` logged in and a route
/// to GitHub): the release is created when missing (`gh release create` with the notes), then every
/// `*.zip` of `--dist` is pushed to the box, uploaded (`gh release upload --clobber`), and deleted
/// there (C: is tight), one at a time; at the end `gh release view --json assets` is compared with the
/// local sizes and every mismatch is reported. Assets already present with the right size are skipped.
pub fn release_zips_cmd(args: &[String]) -> Result<(), String> {
    let f = |k: &str| tmmaps::cli::flag(args, k).map(String::from);
    let dist = PathBuf::from(f("--dist").ok_or("release-zips needs --dist DIR")?);
    let tag = f("--tag").ok_or("release-zips needs --tag TAG")?;
    let title = f("--title").ok_or("release-zips needs --title T")?;
    let notes = PathBuf::from(f("--notes").ok_or("release-zips needs --notes README.md")?);
    let repo = f("--repo").unwrap_or_else(|| "vjeux/trackmania-tas".into());
    let box_dir = f("--box-dir").unwrap_or_else(|| "/home/vjeux/shoot/release-stage".into());
    let wsx = crate::wsx::Wsx::new(args);
    let gh = "cd /home/vjeux && ./bin/gh";
    let mut zips: Vec<PathBuf> = std::fs::read_dir(&dist).map_err(|e| format!("{}: {e}", dist.display()))?.filter_map(|e| e.ok()).map(|e| e.path()).filter(|p| p.extension().map(|x| x == "zip").unwrap_or(false)).collect();
    zips.sort();
    if zips.is_empty() {
        return Err(format!("{}: no zips", dist.display()));
    }
    // the release
    let notes_remote = format!("{box_dir}/{}", notes.file_name().unwrap_or_default().to_string_lossy());
    wsx.sh(&format!("mkdir -p {box_dir}"))?;
    wsx.push(&notes, &notes_remote)?;
    let exists = wsx.sh(&format!("{gh} release view {tag} -R {repo} --json tagName 2>/dev/null | grep -c tagName; true"))?;
    if exists.trim() == "0" || exists.trim().is_empty() {
        let out = wsx.sh(&format!("{gh} release create {tag} -R {repo} --title '{}' --notes-file {notes_remote} 2>&1; true", title.replace('\'', "")))?;
        println!("release create: {}", out.trim());
    } else {
        let out = wsx.sh(&format!("{gh} release edit {tag} -R {repo} --title '{}' --notes-file {notes_remote} 2>&1; true", title.replace('\'', "")))?;
        println!("release exists; notes/title refreshed: {}", out.trim());
    }
    // what is up already
    let present = wsx.sh(&format!("{gh} release view {tag} -R {repo} --json assets --jq '.assets[] | \"\\(.name)\\t\\(.size)\"' 2>/dev/null; true"))?;
    let have: BTreeMap<String, u64> = present.lines().filter_map(|l| l.split_once('\t').and_then(|(n, s)| s.trim().parse().ok().map(|s| (n.trim().to_string(), s)))).collect();
    let t0 = Instant::now();
    let mut done = 0usize;
    for z in &zips {
        let name = z.file_name().unwrap_or_default().to_string_lossy().into_owned();
        let size = std::fs::metadata(z).map(|m| m.len()).unwrap_or(0);
        if have.get(&name) == Some(&size) {
            println!("  {name}: already up ({size} B)");
            done += 1;
            continue;
        }
        let remote = format!("{box_dir}/{name}");
        let t1 = Instant::now();
        wsx.push(z, &remote)?;
        let out = wsx.sh(&format!("{gh} release upload {tag} {remote} -R {repo} --clobber 2>&1; rc=$?; rm -f {remote}; echo rc=$rc"))?;
        if !out.contains("rc=0") {
            return Err(format!("{name}: upload failed: {}", out.trim()));
        }
        done += 1;
        println!("  [{:>5.0}s] {name}: {size} B uploaded in {:.0} s ({done}/{})", t0.elapsed().as_secs_f64(), t1.elapsed().as_secs_f64(), zips.len());
    }
    // verify
    let present = wsx.sh(&format!("{gh} release view {tag} -R {repo} --json assets --jq '.assets[] | \"\\(.name)\\t\\(.size)\"' 2>/dev/null; true"))?;
    let have: BTreeMap<String, u64> = present.lines().filter_map(|l| l.split_once('\t').and_then(|(n, s)| s.trim().parse().ok().map(|s| (n.trim().to_string(), s)))).collect();
    let mut bad = 0usize;
    for z in &zips {
        let name = z.file_name().unwrap_or_default().to_string_lossy().into_owned();
        let size = std::fs::metadata(z).map(|m| m.len()).unwrap_or(0);
        match have.get(&name) {
            Some(s) if *s == size => {}
            Some(s) => {
                bad += 1;
                eprintln!("  VERIFY {name}: {s} B on GitHub, {size} B here");
            }
            None => {
                bad += 1;
                eprintln!("  VERIFY {name}: missing on GitHub");
            }
        }
    }
    let total: u64 = zips.iter().map(|z| std::fs::metadata(z).map(|m| m.len()).unwrap_or(0)).sum();
    println!("release https://github.com/{repo}/releases/tag/{tag}: {} assets present of {} ({:.2} GB), {bad} mismatches", have.len(), zips.len(), total as f64 / 1e9);
    if bad > 0 {
        return Err(format!("{bad} assets missing or wrong size"));
    }
    Ok(())
}

/// `tinyctl surface-batch NN… --src-dir DIR --out-root DIR --surface S [--campaign K
/// --campaign-name N | --no-publish] [--no-shoot] --orig-frames-box DIR [--box-tinyctl P]
/// [--club C] [--wsx P]` — the shoot + publish half of `surface` for a set of
/// ALREADY BUILT maps, with the bridge round trips taken out of the loop
/// (2026-09-13: the per-map `surface` spent ~130 s per map on wsx dispatches
/// around a 40 s shoot and a 30 s publish — the bridge holds a minority of
/// dispatches ~15 s each):
///
/// 1. gate: every map's header and its baked items (`item-check` with the
///    collection's packs);
/// 2. views: `tinyctl views` on the source, the `start` view only (the anchor
///    line kept), written beside the map;
/// 3. ONE tar of the maps + views pushed to the box staging dir;
/// 4. ONE detached box job: per map the variant side's `shootset`, the
///    original's frame copied in from the cache (`--orig-frames-box`; shot
///    now and cached when missing), then `compare` — all under the lock only
///    while the game loads (shootset takes and releases it per run);
/// 5. ONE tar of the side-by-side JPGs pulled back;
/// 6. `publish-batch` on the box (one token mint, every map, the playlist
///    written once in map order), results pulled; the tracker gets one row per
///    map with the frames and the Nadeo verdict (the report reads the last row).
pub fn batch_cmd(args: &[String]) -> Result<(), String> {
    let f = |k: &str| tmmaps::cli::flag(args, k).map(String::from);
    let maps: Vec<String> = args.iter().take_while(|a| !a.starts_with("--")).cloned().collect();
    if maps.is_empty() || !maps.iter().all(|m| m.len() == 2 && m.chars().all(|c| c.is_ascii_digit())) {
        return Err("surface-batch needs two-digit map numbers first".into());
    }
    let surface = f("--surface").ok_or("surface-batch needs --surface S")?;
    let src_dir = PathBuf::from(f("--src-dir").ok_or("surface-batch needs --src-dir DIR")?);
    let out_root = PathBuf::from(f("--out-root").ok_or("surface-batch needs --out-root DIR")?);
    let frames_dir = out_root.join("frames");
    let tracker = out_root.join("tracker.tsv");
    let club = f("--club").unwrap_or_else(|| crate::publish::DEFAULT_CLUB.into());
    let no_shoot = tmmaps::cli::has(args, "--no-shoot");
    let no_publish = tmmaps::cli::has(args, "--no-publish");
    let cache = f("--orig-frames-box").unwrap_or_else(|| "/home/vjeux/shoot/variants/origframes".into());
    let box_tinyctl = f("--box-tinyctl").unwrap_or_else(|| "/home/vjeux/shoot/variants/tinyctl".into());
    let box_shootctl = f("--box-shootctl").unwrap_or_else(|| "/home/vjeux/trackmania-tas/tools/target/release/shootctl".into());
    let (campaign, campaign_name) = if no_publish { (String::new(), String::new()) } else { (f("--campaign").ok_or("--campaign K or --no-publish")?, f("--campaign-name").ok_or("--campaign-name N")?) };
    let wsx = crate::wsx::Wsx::new(args);
    std::fs::create_dir_all(&frames_dir).map_err(|e| format!("{}: {e}", frames_dir.display()))?;
    let tag_of = |nn: &str| format!("{surface}{nn}");
    const STAGE: &str = "/home/vjeux/shoot/_stage";
    const SHOTS: &str = "/mnt/c/Users/vjeux/tinyshots";
    const BOX_FFMPEG: &str = "/mnt/c/Users/vjeux/ffmpeg_extracted/ffmpeg-9.0.1-essentials_build/bin/ffmpeg.exe";
    let batch_dir = format!("/home/vjeux/shoot/variants/batch-{surface}");
    let t0 = Instant::now();

    // the tracker's build rows (the last row per map): copied forward with the new frames/nadeo columns
    let text = std::fs::read_to_string(&tracker).map_err(|e| format!("{}: {e}", tracker.display()))?;
    let head: Vec<String> = text.lines().next().unwrap_or("").split('\t').map(String::from).collect();
    let col_of = |n: &str| head.iter().position(|h| h == n).ok_or_else(|| format!("tracker: no column `{n}`"));
    let (c_frames, c_flag, c_nadeo, c_note) = (col_of("frames")?, col_of("flagged_views")?, col_of("nadeo")?, col_of("note")?);
    let mut rows: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for l in text.lines().skip(1).filter(|l| !l.trim().is_empty()) {
        let r: Vec<String> = l.split('\t').map(String::from).collect();
        rows.insert(r[0].clone(), r);
    }

    // 1+2. gate and views, per map
    struct One { nn: String, src: PathBuf, map: PathBuf, name: String, views: PathBuf, anchor: String, items: Option<PathBuf>, paks: Vec<String> }
    let mut ones: Vec<One> = Vec::new();
    for nn in &maps {
        let dir = out_root.join(format!("{surface}{nn}"));
        let src = std::fs::read_dir(&src_dir).map_err(|e| format!("{}: {e}", src_dir.display()))?.filter_map(|e| e.ok()).map(|e| e.path()).find(|p| p.file_name().map(|n| n.to_string_lossy().starts_with(&format!("{nn}-")) && n.to_string_lossy().ends_with(".Map.Gbx")).unwrap_or(false)).ok_or_else(|| format!("{nn}: no source in {}", src_dir.display()))?;
        let map = std::fs::read_dir(&dir).map_err(|e| format!("{}: {e}", dir.display()))?.filter_map(|e| e.ok()).map(|e| e.path()).find(|p| p.extension().map(|x| x == "Gbx").unwrap_or(false) && p.file_name().map(|n| n.to_string_lossy().ends_with(".Map.Gbx") && !n.to_string_lossy().contains(".s1.") && !n.to_string_lossy().contains(".s2.") && !n.to_string_lossy().contains(".s3.")).unwrap_or(false)).ok_or_else(|| format!("{nn}: no built map in {}", dir.display()))?;
        let hdr = tmmaps::header::read(&map.display().to_string())?;
        if hdr.uid == "-" || hdr.authortime == "-" {
            return Err(format!("{nn}: the header lacks uid / authortime"));
        }
        let bytes = std::fs::metadata(&map).map(|m| m.len()).unwrap_or(0);
        if bytes >= 7_000_000 {
            return Err(format!("{nn}: {bytes} bytes is over the 7 MB server cap"));
        }
        let m = tmmaps::map::MapFile::load(&src);
        let coll = crate::views::collection_of(&m);
        let paks = crate::build::paks_for(coll)?;
        let items_dir = dir.join("libx").join("Items");
        let items = if items_dir.exists() {
            let mut its: Vec<String> = std::fs::read_dir(&items_dir).map_err(|e| e.to_string())?.filter_map(|e| e.ok()).map(|e| e.path().to_string_lossy().into_owned()).filter(|p| p.ends_with(".Item.Gbx")).collect();
            its.sort();
            if !its.is_empty() {
                let mut rest: Vec<String> = vec!["item-check".into()];
                rest.extend(its.iter().cloned());
                let paks2 = paks.clone();
                let mut open = || {
                    let mut store = mapgeom::store::DataStore::empty();
                    let mut i = 0;
                    while i < paks2.len() {
                        if paks2[i] == "--pak" {
                            if let Some((p, k)) = paks2.get(i + 1).and_then(|s| s.rsplit_once(':')) {
                                if let Err(e) = store.add_pak(p, k) {
                                    eprintln!("--pak {p}: {e}");
                                }
                            }
                            i += 2;
                        } else {
                            i += 1;
                        }
                    }
                    store
                };
                mapgeom::static_item::check::run(&rest, &mut open).map_err(|e| format!("{nn}: item-check refused the library ({e})"))?;
                println!("{nn}: item-check ok ({} items)", its.len());
            }
            Some(items_dir)
        } else {
            None
        };
        // views (start only) + anchor
        let views_all = dir.join("views-all.tsv");
        let views = dir.join("views.tsv");
        crate::views::cmd(&[src.display().to_string(), "--out".into(), views_all.display().to_string()])?;
        let vt = std::fs::read_to_string(&views_all).map_err(|e| e.to_string())?;
        let kept: Vec<&str> = vt.lines().filter(|l| l.starts_with('#') || l.trim().is_empty() || l.split('\t').next().unwrap_or("").trim() == "start").collect();
        std::fs::write(&views, kept.join("\n") + "\n").map_err(|e| e.to_string())?;
        let anchor = vt.lines().find(|l| l.starts_with("# anchor ")).map(|l| l.trim_start_matches("# anchor ").trim().to_string()).and_then(|a| a.split(':').next().map(|s| s.to_string())).ok_or_else(|| format!("{nn}: views: no anchor line"))?;
        ones.push(One { nn: nn.clone(), src, map, name: hdr.name.clone(), views, anchor, items, paks });
    }
    println!("[{:>4.0}s] {} maps gated; pushing one tar to the box …", t0.elapsed().as_secs_f64(), ones.len());

    // 3. one tar: maps + views (+ sources for the maps whose original frame is not cached)
    let cached_list = wsx.sh(&format!("ls {cache} 2>/dev/null; true"))?;
    let need_orig: Vec<&One> = ones.iter().filter(|o| !cached_list.lines().any(|l| l.trim() == format!("cmp-orig{}start-o.png", o.nn))).collect();
    let tar_local = out_root.join("batch-stage.tar");
    {
        let mut cmd = Command::new("tar");
        cmd.arg("cf").arg(&tar_local).arg("--transform").arg("s,.*/,,");
        // the file names the box job expects
        let tmpd = out_root.join("batch-stage");
        let _ = std::fs::remove_dir_all(&tmpd);
        std::fs::create_dir_all(&tmpd).map_err(|e| e.to_string())?;
        for o in &ones {
            let stag = tag_of(&o.nn);
            std::fs::copy(&o.map, tmpd.join(format!("{stag}Tiny.Map.Gbx"))).map_err(|e| e.to_string())?;
            std::fs::copy(&o.views, tmpd.join(format!("{stag}-views.tsv"))).map_err(|e| e.to_string())?;
            if need_orig.iter().any(|n| n.nn == o.nn) {
                std::fs::copy(&o.src, tmpd.join(format!("{stag}Orig.Map.Gbx"))).map_err(|e| e.to_string())?;
            }
        }
        let mut cmd = Command::new("tar");
        cmd.arg("cf").arg(&tar_local).arg("-C").arg(&tmpd).arg(".");
        let st = cmd.status().map_err(|e| format!("tar: {e}"))?;
        if !st.success() {
            return Err("tar failed".into());
        }
    }
    wsx.sh(&format!("mkdir -p {STAGE} {batch_dir} {cache}; rm -rf {batch_dir}/*"))?;
    wsx.push(&tar_local, &format!("{batch_dir}/stage.tar"))?;
    wsx.sh(&format!("cd {STAGE} && tar xf {batch_dir}/stage.tar && rm -f {batch_dir}/stage.tar"))?;
    println!("[{:>4.0}s] staged on the box ({} originals to shoot, the rest from the cache)", t0.elapsed().as_secs_f64(), need_orig.len());

    // 4. the shoot job
    if !no_shoot {
        let mut script = String::from("#!/bin/sh\n");
        for o in &ones {
            let stag = tag_of(&o.nn);
            let dir = format!("{SHOTS}/{stag}");
            script.push_str(&format!("rm -rf '{dir}'; mkdir -p '{dir}'\n"));
            if need_orig.iter().any(|n| n.nn == o.nn) {
                script.push_str(&format!("{box_shootctl} shootset --map {STAGE}/{stag}Orig.Map.Gbx --views {STAGE}/{stag}-views.tsv --side o --tag {stag} --outdir {dir} > {dir}/shootset-o.log 2>&1; cp -f {dir}/cmp-{stag}start-o.png {cache}/cmp-orig{}start-o.png\n", o.nn));
            } else {
                script.push_str(&format!("cp -f {cache}/cmp-orig{}start-o.png {dir}/cmp-{stag}start-o.png\n", o.nn));
            }
            script.push_str(&format!("{box_shootctl} shootset --map {STAGE}/{stag}Tiny.Map.Gbx --views {STAGE}/{stag}-views.tsv --side t --tag {stag} --outdir {dir} --anchor {a}:{a} --scale 1 > {dir}/shootset-t.log 2>&1\n", a = o.anchor));
            script.push_str(&format!("{box_tinyctl} compare --views {STAGE}/{stag}-views.tsv --dir {dir} --tag {stag} --hstack-ffmpeg {BOX_FFMPEG} > {dir}/compare.log 2>&1\n"));
            script.push_str(&format!("mkdir -p {batch_dir}/logs; for f in {dir}/*.log; do cp -f \"$f\" {batch_dir}/logs/{stag}-$(basename \"$f\"); done\n"));
            script.push_str(&format!("echo '{stag}' >> {batch_dir}/shot.progress\n"));
        }
        script.push_str(&format!("echo OK {} maps > {batch_dir}/shoot.done\n", ones.len()));
        let script_local = out_root.join("batch-shoot.sh");
        std::fs::write(&script_local, &script).map_err(|e| e.to_string())?;
        wsx.push(&script_local, &format!("{batch_dir}/shoot.sh"))?;
        wsx.sh(&format!("rm -f {batch_dir}/shoot.done {batch_dir}/shot.progress; nohup setsid sh {batch_dir}/shoot.sh > {batch_dir}/shoot.log 2>&1 < /dev/null &"))?;
        println!("[{:>4.0}s] shoot job started on the box ({} maps) …", t0.elapsed().as_secs_f64(), ones.len());
        let text = wsx.wait_done(&format!("{batch_dir}/shoot.done"), &format!("{batch_dir}/shot.progress"), Duration::from_secs(7200), "surface-batch shoot")?;
        println!("[{:>4.0}s] shoot: {}", t0.elapsed().as_secs_f64(), text.trim());
        // 5. the JPGs, one tar
        let names: Vec<String> = ones.iter().map(|o| format!("{}/cmp-{}start.jpg", tag_of(&o.nn), tag_of(&o.nn))).collect();
        wsx.sh(&format!("cd {SHOTS} && tar cf {batch_dir}/frames.tar {} 2>/dev/null; true", names.join(" ")))?;
        let tar_frames = out_root.join("batch-frames.tar");
        wsx.pull(&format!("{batch_dir}/frames.tar"), &tar_frames)?;
        let st = Command::new("tar").arg("xf").arg(&tar_frames).arg("-C").arg(&frames_dir).arg("--strip-components=1").status().map_err(|e| e.to_string())?;
        if !st.success() {
            return Err("frames tar failed".into());
        }
        let n = ones.iter().filter(|o| frames_dir.join(format!("cmp-{}start.jpg", tag_of(&o.nn))).exists()).count();
        println!("[{:>4.0}s] {n}/{} frame pairs pulled into {}", t0.elapsed().as_secs_f64(), ones.len(), frames_dir.display());
        // the compare's flagged-cell counts, from the box logs (best effort)
        for o in &ones {
            let stag = tag_of(&o.nn);
            let s = wsx.sh(&format!("grep -o '[0-9]*/144 cells flagged' {SHOTS}/{stag}/compare.log 2>/dev/null | head -1; true")).unwrap_or_default();
            let flagged = s.trim().split('/').next().unwrap_or("?").to_string();
            if let Some(r) = rows.get_mut(&o.nn) {
                r[c_frames] = "1 pairs".into();
                r[c_flag] = if flagged.is_empty() { "?".into() } else { flagged };
            }
        }
        wsx.sh(&format!("cd {SHOTS} && rm -rf {}; true", ones.iter().map(|o| tag_of(&o.nn)).collect::<Vec<_>>().join(" ")))?;
    }

    // 6. publish-batch
    if !no_publish {
        let manifest: String = ones.iter().map(|o| format!("{STAGE}/{}Tiny.Map.Gbx\t{}\n", tag_of(&o.nn), o.name)).collect();
        let manifest_local = out_root.join("batch-manifest.tsv");
        std::fs::write(&manifest_local, &manifest).map_err(|e| e.to_string())?;
        let remote_manifest = format!("{batch_dir}/manifest.tsv");
        let remote_results = format!("{batch_dir}/results.tsv");
        wsx.push(&manifest_local, &remote_manifest)?;
        let cmd = format!("cd /home/vjeux/shoot/u10s && {box_tinyctl} publish-batch --detach --manifest {remote_manifest} --results {remote_results} --club {club} --campaign {campaign} --campaign-name '{}' --outdir {batch_dir}", campaign_name.replace('\'', ""));
        wsx.sh(&cmd)?;
        println!("[{:>4.0}s] publish-batch started on the box ({} maps → campaign {campaign}) …", t0.elapsed().as_secs_f64(), ones.len());
        let done = wsx.wait_done(&format!("{batch_dir}/results.done"), &format!("{batch_dir}/results.log"), Duration::from_secs(5400), "publish-batch");
        let results_local = out_root.join("batch-results.tsv");
        let pulled = wsx.pull(&remote_results, &results_local);
        let text = done?;
        pulled?;
        println!("[{:>4.0}s] publish: {}", t0.elapsed().as_secs_f64(), text.trim().replace('\n', " | "));
        let rt = std::fs::read_to_string(&results_local).map_err(|e| e.to_string())?;
        for l in rt.lines().filter(|l| !l.trim().is_empty()) {
            let c: Vec<&str> = l.split('\t').collect();
            // path name uid mapId how bytes md5 verdict
            let stem = c[0].rsplit('/').next().unwrap_or("");
            let Some(o) = ones.iter().find(|o| stem == format!("{}Tiny.Map.Gbx", tag_of(&o.nn))) else { continue };
            let (map_id, how, verdict) = (c.get(3).copied().unwrap_or("?"), c.get(4).copied().unwrap_or("?"), c.get(7).copied().unwrap_or("?"));
            if let Some(r) = rows.get_mut(&o.nn) {
                r[c_nadeo] = if verdict.starts_with("IDENTICAL") { format!("OK {} mapId {map_id} stored IDENTICAL", if how == "create" { "created" } else { "updated" }) } else { format!("FAILED: {verdict}") };
            }
        }
        let bad = rt.lines().filter(|l| !l.trim().is_empty() && !l.ends_with("\tIDENTICAL")).count();
        if bad > 0 {
            eprintln!("{bad} map(s) did not publish identical — see {}", results_local.display());
        }
        // publish-batch REPLACES the playlist with the maps it uploaded: a partial run
        // (a re-shot map, a retry) would leave the campaign with those maps only, so
        // the playlist is always rewritten from the tracker's last uid per map (every
        // map of the set, in map order)
        let mut uids: Vec<(String, String)> = rows.iter().filter_map(|(nn, r)| head.iter().position(|h| h == "out_uid").and_then(|k| r.get(k)).filter(|u| u.len() > 4 && *u != "-").map(|u| (nn.clone(), u.clone()))).collect();
        uids.sort();
        if uids.len() > ones.len() {
            let list: Vec<String> = uids.iter().map(|(_, u)| u.clone()).collect();
            let out = wsx.sh(&format!("cd /home/vjeux/shoot/u10s && {box_tinyctl} nadeo-here campaign-set --club {club} --campaign {campaign} --name '{}' --uids {} --no-lock 2>&1 | tail -1", campaign_name.replace('\'', ""), list.join(",")))?;
            println!("[{:>4.0}s] playlist rewritten with every map of the set: {}", t0.elapsed().as_secs_f64(), out.trim());
        }
    }
    // the tracker rows
    for o in &ones {
        if let Some(r) = rows.get(&o.nn) {
            let mut r = r.clone();
            if r[c_note] == "-" {
                r[c_note] = String::new();
            }
            append(&tracker, &r)?;
        }
    }
    wsx.sh(&format!("cd {STAGE} && rm -f {}; true", ones.iter().map(|o| format!("{t}Tiny.Map.Gbx {t}Orig.Map.Gbx {t}-views.tsv", t = tag_of(&o.nn))).collect::<Vec<_>>().join(" ")))?;
    println!("[{:>4.0}s] done; tracker {}", t0.elapsed().as_secs_f64(), tracker.display());
    Ok(())
}

/// The item-name range of a surface variant (added to the per-part alias base):
/// a player who opens the wood and the grass form of one map in one game session
/// must not get the first one's cached models under the second one's names.
/// Tiny holds 0.1–4 M, ice the same range +500, giant 5.1–9 M (alias parts 51–89),
/// the 25-map review builds 4.1–4.9 M (alias parts 41–48).
pub fn surface_alias_offset(surface: &str) -> usize {
    match mapgeom::surface::parse_surface(surface) {
        Some(("Wood", false)) => 10_000_000,
        Some(("Wood", true)) => 14_000_000,
        Some(("Grass", _)) => 18_000_000,
        Some(("Plastic", _)) => 22_000_000,
        Some(("Dirt", _)) => 26_000_000,
        Some(("Magnet", _)) => 30_000_000,
        _ => 0,
    }
}
