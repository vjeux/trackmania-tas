//! Materials of a baked item: the physics of a game material (`MATERIAL_PHYSICS`,
//! `physics_for_link`, the `.Material.Gbx` surface ids), the gameplay gates'
//! kind-less modifier stems, and the rewrites a material gets on the way out —
//! the terrain collections' StadiumOnTerrain game skin (`skinned_material`),
//! a picture of our own (`custom_texture_material`), the gate sign logos
//! (`sign_logo_material`), the light colour skins' glass (`light_skin_material`).

use super::merged::Merged;
use super::surface::Surf;
use crate::crystal_model::CPlugMaterialUserInst;

/// Physics id of an external `.Material.Gbx` (its `CPlugMaterial` surface
/// id), through the store; `None` when it cannot be read.
pub fn material_physics(store: &mut crate::store::DataStore, path: &str) -> Option<u8> {
    let m = store.load_model(path).ok()?;
    let g = m.graph().ok()?;
    match g.root.as_ref()? {
        crate::node::Node::Material(_, phys) => Some(*phys),
        _ => None,
    }
}

/// A `.Material.Gbx`'s (physics, gameplay) surface ids, read off the chunks
/// that carry them: `0x09079017` = { version 1, [physics u8, gameplay u8,
/// u8, flags u8], f32, u32, string } (`Modifier\Boost\Collision`: 00 12 00
/// 80 = Concrete, ReactorBoost_Oriented; `RoadTech`: 10 00 0f 80 = Asphalt,
/// none) — and, for the older files that lack it, `0x0907900E` = { physics
/// u16, u16 } (TechnicsTrims: Metal). The bodies are short and chunk-framed
/// without sizes for these chunks, so the ids are located by their chunk
/// header rather than by a full walk.
pub fn material_surface_ids(store: &mut crate::store::DataStore, path: &str) -> Option<(u8, u8)> {
    let model = store.load_model(path).ok()?;
    let b = &model.body;
    let find = |pat: &[u8]| b.windows(pat.len()).position(|w| w == pat);
    if let Some(i) = find(&[0x17, 0x90, 0x07, 0x09, 0x01, 0x00, 0x00, 0x00]) {
        if let Some(w) = b.get(i + 8..i + 12) {
            return Some((w[0], w[1]));
        }
    }
    if let Some(i) = find(&[0x0e, 0x90, 0x07, 0x09]) {
        if let Some(w) = b.get(i + 4..i + 8) {
            return Some((w[0], 0));
        }
    }
    None
}


/// The special gate's effect: the item's modifier folder names a
/// `Collision` material (`Stadium\Media\Modifier\Boost\Collision`), whose
/// surface ids are what the trigger slab carries under that dress. `None`
/// when the modifier has no such file (Turbo: the prefab's own dress) or
/// there is no modifier.
pub fn special_collision_ids(store: &mut crate::store::DataStore, m: &Merged) -> Option<(String, (u8, u8))> {
    let want = format!("collision{}", m.modifier_suffix.to_ascii_lowercase());
    let link = m.modifier.iter().find(|l| l.rsplit('\\').next().map(|s| s.to_ascii_lowercase()) == Some(want.clone()))?.clone();
    let ids = material_surface_ids(store, &format!("{link}.Material.Gbx"))?;
    Some((link, ids))
}

/// `Stadium\Media\Material\RoadTech.Material.Gbx` -> `Stadium\Media\Material\RoadTech`.
pub fn material_link(path: &str) -> String {
    let lower = path.to_ascii_lowercase();
    match lower.rfind(".material.gbx") {
        Some(i) => path[..i].to_string(),
        None => path.to_string(),
    }
}

/// Two material instances draw the same: every field of the main chunk but
/// the author-side `material_name` (`TM_Argentina_CustomPlastic43` vs
/// `…43S1` — the same plastic, the same colour), and the tiling chunk, agree.
/// The dedup key of [`Merged::material_inst_slot`]; `item-check` refuses two
/// slots that are the same by this measure.
pub fn same_look(a: &CPlugMaterialUserInst, b: &CPlugMaterialUserInst) -> bool {
    let strip = |m: &CPlugMaterialUserInst| {
        m.main.clone().map(|mut main| {
            main.material_name = crate::crystal_model::Id::Null;
            main
        })
    };
    strip(a) == strip(b) && a.tiling == b.tiling
}


/// A gameplay gate's LED sign panel as a plain picture of the kind's logo on
/// black (signlogo.rs: the live gate feeds the panels' `_DispIn` shader a
/// display the static item cannot; the game material alone is a dark row and
/// a ⊗ on the beam).
pub fn sign_logo_material(inst: &CPlugMaterialUserInst, m: &Merged) -> CPlugMaterialUserInst {
    let Some(link) = inst.link().map(|s| s.to_string()) else { return inst.clone() };
    let Some(kind) = super::signlogo::kind_of_pseudo(&link).map(|s| s.to_string()) else { return inst.clone() };
    let file = super::signlogo::logo_file(&kind);
    if !m.pictures.iter().any(|(f, _)| *f == file) {
        // no picture was produced for this kind (no pack texture): the pseudo
        // link would resolve to nothing — fall back to the kind's Sign material
        let mut owned = inst.clone();
        if let Some(main) = owned.main.as_mut() {
            main.link = crate::crystal_model::Id::Str(format!("Stadium\\Media\\Modifier\\{kind}\\Sign"));
        }
        return owned;
    }
    let mut owned = inst.clone();
    if let Some(main) = owned.main.as_mut() {
        main.is_using_game_material = false;
        // The shading model of the panel: TDSN, the picture in slot 0.
        // Measured on a lineup of the 16m Turbo gate (Summer 20 host, close-up
        // frames, panel corners sampled): TDSN with the picture in slot 0
        // alone gives BLACK cells (0x0a0e12) and a sunlit logo (0x919712);
        // ANY use of slot 5 (the self-illumination, in TDSN or TDSNI) turns the
        // black cells into a sky-coloured grey (0x21303e here, 0x5d under
        // Summer 19's hazy sky against the original's 0x18) — the illum term
        // adds an ambient over the whole panel, not just where the picture
        // is lit. Slots 1–4, 6, 7 change nothing; 8 brightens everything; the
        // picture's alpha (full/mask/zero) changes nothing; BaseTexture and
        // TDSNE draw a checker/lighter panel, TDSNEM a flat colour. So the
        // panel is a plain diffuse: black cells like the original, the logo
        // lit by the sun instead of glowing.
        main.model = crate::crystal_model::Id::Str("TDSN".to_string());
        main.material_name = crate::crystal_model::Id::Str(format!("SignLogo{kind}"));
        main.link = crate::crystal_model::Id::Null;
        main.user_textures = vec![crate::crystal_model::UserTexture { u01: 0, texture: file }];
    }
    owned
}

/// Crystal virtual links resolved to the real game-material paths the
/// editor's bake writes (harvested against Granady's items + pak
/// existence). A crystal `Material\Special<Kind><Mod>` link has no
/// `.Material.Gbx` of its own; the baked item carries the existing
/// `Modifier\<Mod>\<Kind>` file instead (both the 2025.7.4 and the current
/// pak contain `Modifier\Turbo\{Sign,SignOff,Decal}.Material.Gbx`; tri-count
/// correspondence on Road_17 is exact: Sign 654idx/218tris, SignOff 54/18,
/// Decal 384/128). `SpecialFXTurbo` has no `Modifier\Turbo\SpecialFX` file,
/// so it stays virtual -- matching all 26 references.
pub const MATERIAL_LINK_RESOLVE: &[(&str, &str)] = &[
    ("Stadium\\Media\\Material\\SpecialSignTurbo", "Stadium\\Media\\Modifier\\Turbo\\Sign"),
    ("Stadium\\Media\\Material\\SpecialSignOff", "Stadium\\Media\\Modifier\\Turbo\\SignOff"),
    ("Stadium\\Media\\Material\\DecalSpecialTurbo", "Stadium\\Media\\Modifier\\Turbo\\Decal"),
];

/// The editor-resolved link for a crystal material link.
pub fn resolve_crystal_link(link: &str) -> &str {
    MATERIAL_LINK_RESOLVE.iter().find(|(v, _)| *v == link).map(|(_, r)| *r).unwrap_or(link)
}

/// Physics id the game gives a library material (harvested from the 26
/// reference items: every link there carries exactly one id). The
/// `.Material.Gbx` files themselves carry none (classes.rs reads their
/// surface id as 0), so this table is what makes a prefab-built item's
/// `CPlugMaterialUserInst.surface_physic_id` match the game's own items.
pub const MATERIAL_PHYSICS: &[(&str, u8)] = &[
    ("Stadium\\Media\\Material\\ChronoFinish", 32),
    ("Stadium\\Media\\Material\\DecoHill", 2),
    ("Stadium\\Media\\Material\\DecoHill2", 2),
    ("Stadium\\Media\\Material\\LightSpot", 32),
    ("Stadium\\Media\\Material\\LightSpot2", 32),
    ("Stadium\\Media\\Material\\PlatformTech", 16),
    ("Stadium\\Media\\Material\\RaceAd6x1", 32),
    ("Stadium\\Media\\Material\\RaceArchFinish", 4),
    ("Stadium\\Media\\Material\\RaceScreenStart", 32),
    ("Stadium\\Media\\Material\\RaceScreenStartSmall", 32),
    ("Stadium\\Media\\Material\\RoadTech", 16),
    ("Stadium\\Media\\Material\\Speedometer", 4),
    ("Stadium\\Media\\Material\\SpeedometerLight", 4),
    ("Stadium\\Media\\Material\\Technics", 4),
    ("Stadium\\Media\\Material\\TechnicsSpecials", 4),
    ("Stadium\\Media\\Material\\TechnicsTrims", 4),
    ("Stadium\\Media\\Material\\TrackBorders", 9),
    ("Stadium\\Media\\Material\\TrackBordersOff", 9),
    ("Stadium\\Media\\Material\\TrackWall", 14),
    ("Stadium\\Media\\Material\\TrackWallClips", 22),
    ("Stadium\\Media\\Modifier\\PlatformGrass\\OpenTechBorders", 76),
    ("Stadium\\Media\\Modifier\\PlatformGrass\\PlatformTech", 76),
    ("Stadium\\Media\\Modifier\\PlatformIce\\DecoHill", 21),
    ("Stadium\\Media\\Modifier\\PlatformDirt\\PlatformTech", 6),
    ("Stadium\\Media\\Modifier\\PlatformIce\\PlatformTech", 74),
    ("Stadium\\Media\\Modifier\\Turbo\\Sign", 32),
    ("Stadium\\Media\\Modifier\\Turbo\\SignOff", 32),
];

/// Physics for a material link: the table, then the rules the table shows
/// (`Decal*`/`SpecialFX*`/`Turbo\Decal` -> NotCollidable 28, `ChronoFinish-*`
/// -> 32), else `None`.
/// The kind-less name a gameplay-gate material has inside a
/// `Modifier\<Kind>\` folder: the prefab's `SpecialSignTurbo` is the folder's
/// `Sign`, `SpecialSignOff` → `SignOff`, `SpecialFXTurbo` → `SpecialFX`,
/// `TriggerFXTurbo` → `TriggerFX`, `DecalSpecialTurbo` → `Decal` (the base
/// prefab wears the Turbo dress; the pak's `Modifier\Turbo\` holds exactly
/// these files for it). Anything else: `None`.
pub fn gate_special_stem(stem: &str) -> Option<&'static str> {
    Some(match stem {
        "SpecialSignTurbo" => "Sign",
        "SpecialSignOff" => "SignOff",
        "SpecialFXTurbo" => "SpecialFX",
        "TriggerFXTurbo" => "TriggerFX",
        "DecalSpecialTurbo" => "Decal",
        _ => return None,
    })
}

pub fn physics_for_link(link: &str) -> Option<u8> {
    let l = link.to_ascii_lowercase();
    if let Some((_, p)) = MATERIAL_PHYSICS.iter().find(|(k, _)| k.to_ascii_lowercase() == l) {
        return Some(*p);
    }
    let base = l.rsplit('\\').next().unwrap_or(&l);
    if base.starts_with("decal") || base.starts_with("specialfx") || base.starts_with("racetriggerfx") {
        return Some(28);
    }
    if base.starts_with("chronofinish") {
        return Some(32);
    }
    // every gameplay kind's `Modifier\<Kind>\Sign|SignOff` is the Turbo one's (32)
    if l.contains("\\modifier\\") && (base == "sign" || base == "signoff") {
        return Some(32);
    }
    None
}

/// The physics id most of a static object's collision triangles carry.
pub fn most_common_physics(so: &super::item::CPlugStaticObjectModel) -> Option<u8> {
    let sf = so.surface()?;
    let Surf::Mesh { triangles, .. } = &sf.surf else { return None };
    let mut counts = [0usize; 256];
    for t in triangles {
        counts[t.material_id as usize] += 1;
    }
    let (best, n) = counts.iter().enumerate().max_by_key(|(_, n)| **n)?;
    (*n > 0).then_some(best as u8)
}


/// The "StadiumOnTerrain" game skin: in a BlueBay map every Stadium-family
/// block draws some of its materials through
/// `BlueBay\Media\Modifier\StadiumOnTerrain\<slot>.Material.Gbx` instead of
/// `Stadium\Media\Material\<name>` (the slot table is
/// `Stadium\GameSkin\StadiumOnTerrain.GameSkin.gbx`). Items know nothing of
/// skins, so the link is remapped here: without it the wall faces under the
/// stands drew Stadium's wooden `TrackWallClips` where the original shows
/// BlueBay's concrete `TrackWallClipsInWorld` (2026-09-06).
/// Whether a collection's `Water` visuals stay in the bake: Stadium's pools
/// are drawn by the `WaterBase` blocks themselves (no water zone to fall back
/// on); BlueBay / RedIsland / WhiteShore / GreenCoast regenerate their sea or
/// lake from the genealogy at that very height.
pub fn keep_water_for(collection: u32) -> bool {
    collection == 0x1a
}

/// The environment folder of a map collection id (the pack's root folder).
pub fn env_name(collection: u32) -> &'static str {
    match collection {
        0x1c => "BlueBay",
        0x1a => "Stadium",
        0x10 => "RedIsland",
        0x1d => "WhiteShore",
        0xf => "GreenCoast",
        _ => "BlueBay",
    }
}

/// A material link rewritten into the engine's CUSTOM-texture form (the
/// ManiaPlanet item-editor material: `IsUsingGameMaterial` off, a shading
/// `Model`, textures named by file) when a picture for its stem is provided —
/// the one way a texture of our own reaches an embedded item (the 2026-09-07
/// probes: a texture named by pack PATH becomes no fid the skin remap can
/// reach; a bare file name is resolved in the item's own archive folder).
pub fn custom_texture_material(inst: &CPlugMaterialUserInst, ident: &str) -> CPlugMaterialUserInst {
    // TINY_PICTURES=DIR: the production form. A material whose link stem has a
    // `<stem>.dds` in DIR draws that picture — a custom-texture material with
    // the texture named by file name, which the game resolves in the item's
    // own archive folder (tiny-library puts every DIR/*.dds into the library
    // zip as Items/<stem>.dds). Several pictures per stem (`<stem>.dds`,
    // `<stem>.2.dds`, …) are spread over the models by ident hash — every
    // placement of one model shows the same one. TINY_PICTURES_MODEL=TDSN|TDSNI
    // (default TDSNI: slot 0 diffuse + slot 5 self-illumination, the lit-screen
    // look).
    if let Some(dir) = std::env::var_os("TINY_PICTURES") {
        if let Some(link) = inst.link().map(|s| s.to_string()) {
            let stem = link.rsplit('\\').next().unwrap_or(&link).to_string();
            let mut choices: Vec<String> = std::fs::read_dir(&dir)
                .map(|rd| {
                    rd.filter_map(|e| e.ok())
                        .map(|e| e.file_name().to_string_lossy().into_owned())
                        .filter(|n| n.to_ascii_lowercase().ends_with(".dds") && (n == &format!("{stem}.dds") || n.starts_with(&format!("{stem}."))))
                        .collect()
                })
                .unwrap_or_default();
            choices.sort();
            if !choices.is_empty() {
                let h: usize = ident.bytes().fold(5381usize, |h, b| h.wrapping_mul(33).wrapping_add(b as usize));
                let file = choices[h % choices.len()].clone();
                let model = std::env::var("TINY_PICTURES_MODEL").unwrap_or_else(|_| "TDSNI".into());
                let mut owned = inst.clone();
                if let Some(main) = owned.main.as_mut() {
                    main.is_using_game_material = false;
                    main.model = crate::crystal_model::Id::Str(model.clone());
                    main.material_name = crate::crystal_model::Id::Str(stem.clone());
                    main.link = crate::crystal_model::Id::Null;
                    main.user_textures = vec![crate::crystal_model::UserTexture { u01: 0, texture: file.clone() }];
                    if model == "TDSNI" {
                        main.user_textures.push(crate::crystal_model::UserTexture { u01: 5, texture: file.clone() });
                    }
                }
                return owned;
            }
        }
    }
    inst.clone()
}

/// The collection skin's slot table (`Stadium\GameSkin\StadiumOnTerrain.GameSkin.gbx`):
/// material stem → `<Env>\Media\Modifier\StadiumOnTerrain\<slot>`.
pub const SKIN_SLOTS: &[(&str, &str)] = &[
        ("TrackWallClips", "TrackWallClipsInWorld"),
        ("TrackWall", "TrackWallInWorld"),
        ("TrackBorders", "TrackBordersInWorld"),
        ("TrackBordersOff", "TrackBordersOffInWorld"),
        ("Structure", "StructureInWorld"),
        ("Deco", "Deco"),
        ("DecoHill", "DecoHill"),
        ("DecoHill2", "DecoHill2"),
        ("DecalPaintSponsor4x1D", "DecalPaintSponsor4x1D"),
        ("DecalPaint2Sponsor4x1D", "DecalPaint2Sponsor4x1D"),
        ("DecalPaint2Sponsor4x1NoColorizeD", "DecalPaint2Sponsor4x1NoColorizeD"),
        ("DecalPaintSponsor4x1NoColorizeD", "DecalPaintSponsor4x1NoColorizeD"),
    ];

pub fn skinned_material(inst: &CPlugMaterialUserInst, collection: u32) -> CPlugMaterialUserInst {
    const SKIN: &[(&str, &str)] = SKIN_SLOTS;
    // every terrain environment carries `<Env>\Media\Modifier\StadiumOnTerrain\`
    // with the same slots (BlueBay and RedIsland checked); Stadium itself has none
    if collection == 0x1a {
        return inst.clone();
    }
    let env = env_name(collection);
    let Some(link) = inst.link() else { return inst.clone() };
    // The skin is applied AFTER the block's material modifier, by material
    // stem: Summer 16's OpenDirtZone blocks (modifier PlatformDirt, whose
    // folder has its own `Deco` = DecoHillDirt) draw BlueBay grass in the
    // game, not dirt — `Modifier\PlatformDirt\Deco` still lands on the skin's
    // `Deco` slot. So a `Stadium\Media\Modifier\<X>\<stem>` link is skinned
    // like the plain material of the same stem.
    let stem = match link.strip_prefix("Stadium\\Media\\Material\\") {
        Some(stem) => stem,
        None => match link.strip_prefix("Stadium\\Media\\Modifier\\").and_then(|rest| rest.split_once('\\')) {
            Some((_folder, stem)) if !stem.contains('\\') => stem,
            _ => return inst.clone(),
        },
    };
    let Some((_, slot)) = SKIN.iter().find(|(name, _)| *name == stem) else { return inst.clone() };
    let mut owned = inst.clone();
    if let Some(main) = owned.main.as_mut() {
        main.link = crate::crystal_model::Id::Str(format!("{env}\\Media\\Modifier\\StadiumOnTerrain\\{slot}"));
    }
    owned
}


/// The glass of a light item under a light colour skin (light_skin.rs): a
/// material whose pack file carries a self-illumination texture becomes a
/// self-lit custom-texture material with the swatch as diffuse AND
/// illumination (`Items/LightColor_<Name>.dds`), the way the skin replaces the
/// stock item's `_I` textures. `Off` glows black.
pub fn light_skin_material(inst: &CPlugMaterialUserInst, m: &Merged) -> CPlugMaterialUserInst {
    let Some(skin) = m.light_skin.as_ref() else { return inst.clone() };
    let Some(link) = inst.link().map(|s| s.to_string()) else { return inst.clone() };
    if !m.illum_links.iter().any(|l| *l == link) {
        return inst.clone();
    }
    let stem = link.rsplit('\\').next().unwrap_or(&link).to_string();
    let file = skin.file();
    // The shading model of the glass: TDSNI. Probed 2026-09-07 on a Red
    // LightTubeBig4m against the stock skinned tube (whose pack material has
    // SelfIllumScale 1.5 + a refract layer + a _G glow map): TDSNI and TDSNEM
    // glow red but dimmer, TDSNE/TDSNI_Night dimmer still, TIAdd invisible, a
    // SelfIllumScale material constant turned the glass BLACK (the row is
    // read, the encoding is not this).
    let mut owned = inst.clone();
    if let Some(main) = owned.main.as_mut() {
        main.is_using_game_material = false;
        main.model = crate::crystal_model::Id::Str("TDSNI".to_string());
        main.material_name = crate::crystal_model::Id::Str(format!("{stem}{}", skin.name));
        main.link = crate::crystal_model::Id::Null;
        main.user_textures = vec![crate::crystal_model::UserTexture { u01: 0, texture: file.clone() }, crate::crystal_model::UserTexture { u01: 5, texture: file }];
    }
    owned
}


/// The ad SCREEN face materials of the Technics screen blocks: LED-cell
/// panels whose picture the game replaces at run time with a served ad
/// (`DeactivableDisplayId`); with none served — every non-official map — they
/// show the pack's default picture on green cells (`Ad2x3Screen` → a driver
/// with crossed arms), which from afar reads as a flat emerald cut-out of the
/// landscape (Summer 01 at 8 s, vjeux 2026-09-10: "the mountain on the right
/// is missing part of the model and you can see through"). Six links on the 25
/// maps (454 screen items): the frames are TechnicsTrims/Pylon, the backs
/// ScreenBack — untouched.
pub const AD_SCREEN_LINKS: &[&str] = &[
    "Stadium\\Media\\Material\\Ad155Screen",
    "Stadium\\Media\\Material\\Ad1x1Screen",
    "Stadium\\Media\\Material\\Ad2x1Screen",
    "Stadium\\Media\\Material\\Ad2x3Screen",
    "Stadium\\Media\\Material\\Ad4x1Screen",
    "Stadium\\Media\\Material\\RaceAd6x1",
];

/// `TINY_SCREENS`: what an ad screen face shows. `default` (unset) keeps the
/// game material; `dark` links the face to `ScreenBack` (the screens' own
/// dark casing look — what the Screen4x1 ITEMS already show); `logo` puts the
/// TRACKMANIA picture on it as a plain lit panel (the sign-logo path:
/// `add_screen_logo_pictures` produces `ScreenLogo.dds` from the pack's
/// `RaceAd6x1` default texture).
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum ScreenMode {
    Default,
    Dark,
    Logo,
}

pub fn screen_mode() -> ScreenMode {
    match std::env::var("TINY_SCREENS").unwrap_or_default().to_ascii_lowercase().as_str() {
        "dark" => ScreenMode::Dark,
        "logo" => ScreenMode::Logo,
        _ => ScreenMode::Default,
    }
}


pub fn is_ad_screen_link(link: &str) -> bool {
    AD_SCREEN_LINKS.iter().any(|l| l.eq_ignore_ascii_case(link))
}

pub fn screen_face_material(inst: &CPlugMaterialUserInst, m: &Merged) -> CPlugMaterialUserInst {
    // `logo` only: the face becomes a plain lit picture (TDSN, the picture in
    // slot 0 — black cells and a sunlit logo, measured on the gate sign panels
    // in `sign_logo_material`). `dark` is done before assembly by
    // `Merged::darken_screen_faces` (a slot merge, so item-check sees one
    // ScreenBack slot, not two of the same look).
    if screen_mode() != ScreenMode::Logo {
        return inst.clone();
    }
    let Some(link) = inst.link().map(|s| s.to_string()) else { return inst.clone() };
    if !is_ad_screen_link(&link) || !m.pictures.iter().any(|(f, _)| *f == screen_logo_file()) {
        return inst.clone();
    }
    let mut owned = inst.clone();
    if let Some(main) = owned.main.as_mut() {
        main.is_using_game_material = false;
        main.model = crate::crystal_model::Id::Str("TDSN".to_string());
        main.material_name = crate::crystal_model::Id::Str("ScreenLogo".to_string());
        main.link = crate::crystal_model::Id::Null;
        main.user_textures = vec![crate::crystal_model::UserTexture { u01: 0, texture: screen_logo_file() }];
    }
    owned
}

/// The gameplay gates' trigger CURTAIN: the `ExpandableSpecial_Air` /
/// `Special*_Air` prefabs draw the trigger volume with
/// `Stadium\Media\Modifier\<Kind>\TriggerFX` — a `TAddModCV` additive material
/// whose colour comes through `Func\FuncShader\SpecialFXGate.FuncShader.Gbx`
/// from the LIVE gate (the icon `Texture\TriggerFX<Kind>_I`). An embedded item
/// has no live gate driving the FuncShader, and the engine draws the unfed
/// sampler as its green/purple checkerboard (vjeux, 2026-09-10 17:45Z, Argentina's
/// gate stacks: "the icon panels show a green/purple checkerboard").
/// `TINY_TRIGGERFX`: `game` (default) keeps the checkerboard; `off` drops the
/// curtain visual — but a visual-less trigger item is NOT emitted by the
/// library today, so `off` loses the gameplay trigger too (do not ship it); `picture` draws the icon as a static
/// self-lit additive quad (`TIAdd`, the pak's `TriggerFX<Kind>_I.dds` as
/// `Items/TriggerFX<Kind>.dds`) — unverified in a frame yet; `game` keeps the
/// checkerboard.
pub fn trigger_fx_kind(link: &str) -> Option<String> {
    if let Some(rest) = link.strip_prefix("Stadium\\Media\\Modifier\\") {
        let (kind, name) = rest.split_once('\\')?;
        return (name == "TriggerFX").then(|| kind.to_string());
    }
    // the prefab's own form before the modifier re-dress: `Material\TriggerFXTurbo`
    let rest = link.strip_prefix("Stadium\\Media\\Material\\TriggerFX")?;
    (!rest.is_empty() && !rest.contains('\\')).then(|| rest.to_string())
}

pub fn trigger_fx_mode() -> String {
    // default `game` until the picture form is verified in a frame: `off` would
    // leave the gate trigger item with no visual, and the library emits no item
    // for a visual-less variant — the GAMEPLAY TRIGGER would vanish with the
    // curtain (21 build, 2026-09-10 17:53Z: "no geometry in this variant").
    std::env::var("TINY_TRIGGERFX").unwrap_or_else(|_| "game".to_string())
}

pub fn trigger_fx_file(kind: &str) -> String {
    format!("TriggerFX{kind}{}.dds", picture_suffix())
}

pub fn trigger_fx_material(inst: &CPlugMaterialUserInst, m: &Merged) -> CPlugMaterialUserInst {
    if trigger_fx_mode() != "picture" {
        return inst.clone();
    }
    let Some(kind) = inst.link().and_then(trigger_fx_kind) else { return inst.clone() };
    let file = trigger_fx_file(&kind);
    if !m.pictures.iter().any(|(f, _)| *f == file) {
        return inst.clone();
    }
    let mut owned = inst.clone();
    if let Some(main) = owned.main.as_mut() {
        main.is_using_game_material = false;
        main.model = crate::crystal_model::Id::Str("TIAdd".to_string());
        main.material_name = crate::crystal_model::Id::Str(format!("TriggerFX{kind}"));
        main.link = crate::crystal_model::Id::Null;
        main.user_textures = vec![crate::crystal_model::UserTexture { u01: 0, texture: file.clone() }, crate::crystal_model::UserTexture { u01: 5, texture: file }];
    }
    owned
}

/// The per-build suffix of every GENERATED picture file (`SignLogoTurbo<sfx>.dds`,
/// `ScreenLogo<sfx>.dds`, `TriggerFX<Kind><sfx>.dds`): the game caches an
/// embedded texture by its file name for the whole session exactly as it
/// caches item models (TINY_ALIAS_BASE, 2026-09-09), so a rebuilt picture
/// under the old name shows the OLD bytes until the game restarts — the
/// 2026-09-10 DXT5 re-encode of the screen picture drew black in the same
/// session that had loaded the uncompressed one. `TINY_PICTURE_SUFFIX` (tinyctl
/// sets it from the alias base); empty = the bare names.
pub fn picture_suffix() -> String {
    std::env::var("TINY_PICTURE_SUFFIX").unwrap_or_default()
}

pub fn screen_logo_file() -> String {
    format!("ScreenLogo{}.dds", picture_suffix())
}
