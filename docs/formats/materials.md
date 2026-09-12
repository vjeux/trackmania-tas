# Materials: `CPlugMaterialUserInst`, `CPlugMaterial`, modifiers, game skins, colour tables

Readers/writers: `tools/mapgeom/src/crystal_model.rs` (`CPlugMaterialUserInst`,
byte-exact), `static_item/oldmat.rs` (`CPlugMaterial` `0x09079000`,
`CPlugMaterialCustom` `0x0903A000`, read-only), `static_item/materials.rs`
(physics tables, the modifier/skin rewrites), `light_skin.rs`, `signlogo.rs`;
exe reads in `tools/tmmaps/tiny/re/material-color-usertextures.md` and
`material-collection-folder-whitelist.md`. Confidence **[FILE]**, **[EXE]** for
§1–§4's reflection facts, **[VERIFIED-GAME]** for the render rules.

## 1. `CPlugMaterialUserInst` (`0x090FD000`) — the only material an embedded item can hold

Reflection layout (class `0x090FD000`): `_Name` (id) +0x28, `_LinkFull`
(string) +0x30, `Link`/`MaterialId` (id) +0x40, `Model` (id) +0x48,
`BaseTexture` (string) +0x50, `UserTextures` count +0x60 / entries +0x68 (≤ 8 ×
`{int Slot; CFastString Texture}`), `TilingU/V` +0x138/+0x13c,
`TextureSizeInMeters` +0x140, `IsNatural` +0x144, PhysicsId byte +0x148,
GameplayId byte +0x149, `Csts` +0x14c/+0x150, `Color` +0x1d0/+0x1d8,
`UvAnims` +0x1e0, `HidingGroup` +0x220, `IsUsingGameMaterial` +0x224.

```text
chunk 0x090FD000  (v11 written; v ≥ 12 is ud2 = unsupported by this exe)
  u32 version
  [v≥9]  u8  IsUsingGameMaterial        (written as LinkFull.len != 0)
  Id     MaterialName
  Id     Model                          ("TDSN", "TDOSN", … the shading model; null on a game material)
  string BaseTexture
  [v<10] u8 physics (remapped)  |  [v≥10] u8 SurfacePhysicId, u8 SurfaceGameplayId
  Id     Link                           (v1..8, or v≥11 && !IsUsingGameMaterial)
  string LinkFull                       (v9..10, or v≥11 && IsUsingGameMaterial) — the pack material path
                                        minus ".Material.Gbx", e.g. "Stadium\Media\Material\RoadTech"
  [v≥2]  Cst[] csts { Id Name, Id Type, i32 Count (Offset = running sum) }   — ≤ 8, else REJECTED (count := 0)
         i32[] color                    NOT a colour: the flat VALUE buffer of the csts (Count dwords each);
                                        for Type "Real" IEEE floats; legacy small ints 1..255 → v/255 on load
  [v≥3]  UvAnim[] { Id, Id, f32, u64, [v≥5] Id }
  [v≥4]  Id[]
  [v≥6]  UserTexture[] { i32 Slot, string Texture }   — ≤ 8, else the reader drops them ALL (count 0);
                                        written EMPTY when IsUsingGameMaterial (a game link has no user textures)
  [v≥7]  Id HidingGroup
chunk 0x090FD001  { u32 version, ref CPlugBitmapAtlas, f32 tilingU, f32 tilingV, [v≥3] f32 textureSizeInMeters,
                    [v≥4] i32, [v≥5] bool isNatural }   (v2 = GBX.NET throw)
chunk 0x090FD002  { u32 version, i32 }
```

User-texture SLOT enum (exe getter `0x1404fc5ba`, pre-v8 remap
`0,1,2,…,9 → 0,1,4,5,6,7,8,9,10,11`): **Diffuse 0, DiffuseO 1, BaseColor 2,
BaseColorO 3, Specular 4, Normal 5, Energy 6, TeamMask 7, SelfIllum 8, Damage
9, Dirt 10, Shield 11, RoughMetal 12.**

Which slot a MODEL reads decides everything **[VERIFIED-GAME]**: `TDSN` reads
slot 0 (Diffuse; alpha treated as gloss → opaque cards, pale/shiny crowns);
`TDOSN`/`TDOBSN` read slot 1 (DiffuseO, alpha-TESTED; slot 0 ignored — an atlas
in slot 0 under TDOSN shows the default TRANSPARENT image: invisible leaves);
`TDSNI` → colour 0 + glow in slot 8 (SelfIllum; NOT 5, which is Normal);
`TIAdd` → slot 8 only; every model: S = slot 4, N = slot 5. Item models in the
exe: `MaterialStatic_{TDSN, TDOSN, TDOBSN, TDSNE, TDSNI, TDSNI_Night, TIAdd}`
(also spelled TDSNEM, TDOSN2Sided, TDOSNEM, TDiffuse, TI_AddModCV, TAniso,
TIce, TShield in the table; the `2Sided`/`EM` ones are `MaterialChar_*`, not
for items — they render RED; `TDSNE` black). Per-model slot lists live in the
exe's runtime record array at `0x141fa9d48`.

A custom-texture material that WORKS: `{IsUsingGameMaterial = false, Model
"TDSN", MaterialName <stem>, Link null, UserTextures [{0, path}]}` where the
path is a GAME path (`Stadium\Media\Texture\Image\RaceAd6x1.dds`) or a DDS in
the map archive in the ITEM'S OWN folder (`Items/Ad6x1.dds`, referenced as
`Ad6x1.dds`). NOT: png/jpg/webp/tga, subfolders (`Textures\…`), `Items\…`,
`Skins\…`, `CurrentMap_EmbeddedFiles\…`. The exe has no UV-anim property on a
UserInst (seven encodings of `uv_anims` → no motion); a `.webm` as a texture
= the missing-texture pattern.

**Custom materials of one NAME are one material to the game**: thirteen palms
whose `PalmTree_Leaf` differed only by model CRASHED the client
(`0x140456513`), so a name spells the model and the image
(`TDOSN_ItemPalmTreeBranch_D`). Two slots that draw the same by every field
but the author-side name are one slot (`same_look`; `item-check` refuses
duplicates).

## 2. `CPlugMaterial` (`0x09079000`) and `CPlugMaterialCustom` (`0x0903A000`)

The pre-UserInst classes the pack's prefabs carry: Solid2 materials in the
pack are EXTERNAL `.Material.Gbx` refs; BlueBay terrain visuals carry ONE
private inline `CPlugMaterial` each (1301/1301) whose `0x09079015` shader ref
names the external material, and the terrain id pass uses a shared Techno3
`_Ids` material (drop those visuals or they z-fight as grey stripes). What the
builder reads: the surface physics id (chunk `0x0907900E = { u16 physics, u16
}`) and `0x0907900D`'s trailing array of node refs (the external
`.Material.Gbx` the terrain material stands for). The gameplay-bearing chunk
`0x09079017` is in `collision-cplugsurface.md` §4. `CPlugMaterialCustom` chunk
`0x0903A015` (mode 1/2) carries no layer names on the inline prefab materials.
Any material whose `.Material.Gbx` refs contain `Tween`
(`Tech3_Warp_TDiffSpec_VertexTween`) cannot dress a static visual.

## 3. Modifiers: how one prefab wears many dresses

* A block info's material modifier (chunk `0x0304E031` slot 1; slot 0 is the
  `EDClassic` parent info it copies) is one of two files of class `0x0915D000`,
  both `{ folder, game skin }`: `X.TerrainModifier.Gbx` = folder `Modifier\X\`
  + `Platform.GameSkin` (slots PlatformTech, DecalPlatform, DecoHill,
  OpenTechBorders, DecoHill2, DecoCliff, DecoCliffBase, TrackWall, DecoGrass,
  Deco, Penalty), and `TrackWallToDecoCliff.Gbx` = folder
  `Modifier\PlatformGrass\` + a TrackWall-only skin. The folder holds
  `<slot>.Material.Gbx` for the slots it re-dresses; a material the folder
  lacks stays the prefab's own. Nadeo's typo: `Stadium\Media\Modifier\Reset.TerrainModifier .Gbx`
  (a SPACE before `.Gbx`, in the block info AND the pack entry).
* An item file references `Stadium\Media\Modifier\<X>.Gbx` = folder F +
  suffix; its materials become `Modifier\F\<stem><suffix>`
  (`ItemObstacleLevel1` → orange pushers/rotors AND the Level-1 kinematic
  constraint swap, `prefab-and-dyna.md`).
* Gate kinds: `Modifier\<Kind>\{Collision, Sign, SignOff, SpecialFX, TriggerFX,
  Decal, DecalPlatform}` through the `Specials` / `SpecialsOriented` game
  skins (slot = the pack material the prefab is authored with:
  `Collision ← Effects\Media\Material\CollisionTurbo`, `Sign ←
  SpecialSignTurbo`, `Decal ← DecalSpecialTurbo` …); the shared Special
  prefab wears the Turbo dress (`gate_special_stem`).
* The **StadiumOnTerrain** game skin: in a BlueBay map every Stadium-family
  block draws some materials through `BlueBay\Media\Modifier\StadiumOnTerrain\
  {TrackWallClipsInWorld, TrackWallInWorld, TrackBordersInWorld,
  TrackBordersOffInWorld, StructureInWorld, Deco, DecoHill, DecoHill2,
  DecalPaint*Sponsor4x1D}` (slot table `Stadium\GameSkin\StadiumOnTerrain.GameSkin.gbx`),
  applied AFTER the block's own modifier, by stem; items know nothing of skins,
  so the bake remaps the link or the walls under the stands draw Stadium wood.
* Which block dresses its GENERATED CLIPS, and with what, is in
  `tools/tmmaps/TINY.md` "Placement colours and the clip walls' materials"
  (the folder's materials decide; a `MatModifier` placement-tag gate was tried
  and refuted by pixel means, 2026-09-09).

## 4. The per-environment material WHITELIST **[EXE]**

`NGameItemUtils::LoadPlugCrystalMaterials` (`0x140f4b600`) gates every
non-empty Link by `Link.Split('\\')[0] ∈ GetMaterialCollectionFolders(collection)`
(`0x140ae6b40`), a compiled-in table: always `Test`, `Editors`, `Effects`; plus
`Stadium`, `Stadium256`, `Stadium4` for the Stadium ids; Canyon/Lagoon/Meteor/
StadiumMP4/Storm/Valley families likewise; **BlueBay (idx 19, MwId 0x1c) has
no branch** — in a BlueBay map the only links that pass are `Test\…`,
`Editors\…`, `Effects\…` and the literal `ERROR_MAT`; both `Stadium\…` and
`BlueBay\…` are rejected with `Material: unknown collection folder %s` in
`UGCErrorsLog.txt`, the face set is CULLED (the item draws nothing, no red).
No link syntax escapes it (`..\` fails the lookup; the `:resource:\` prefixes
are only honoured with a NULL start folder). A `MaterialId` (Link empty)
searches `<Name>\Media\Material_BlockCustom\` and `<Name>\Media\Material\` of
the collection's family plus `Effects\Media\Material\`. Collection ids: Canyon
0x0c, Valley 0x0b, Lagoon 0x0d, Stadium 0x1a, Stadium4 0x18, Stadium256 0x19,
Storm 0xca, GreenCoast 0x0f, RedIsland 0x10, BlueBay 0x1c, WhiteShore 0x1d,
TMCommon 0x11, Vehicles 0x2710 (10003), Common 0x2713.

The measured side (2026-09-05): `Editors\MeshEditorMedia\Materials\Rubber`
resolves (checkerboard); Nadeo's archive crystal draws RED there; a render
oracle without eyes is `CGameCtnChallenge.CopperPrice` (5997 base, +1 when a
face set draws).

## 5. Colour: hue masks, colour tables, the placement byte **[VERIFIED-GAME]**

* `*_D_HueMask.dds` = the alpha channel is the mask (RGB ≈ 0, 0xff, 0);
  `TrackBorders_D_HueMask` (128×1024) is 0xff only at rows v 0.02..0.11 (the
  road stripe); `ItemFlag_D_HueMask` over the whole cloth; `ItemPillar_D_HueMask`
  on the yellow drum. The game samples the DDS with v FLIPPED (`v_uv = 1 −
  v_tex`). Tinting sets hue AND saturation (a white texel turns pastel).
* Colour tables (`Stadium\Media\ColorTargetTables\*.ColorTable.gbx.json`):
  Sport `e1e1e1/437256/376088/8f291b/252525`, SportIllum
  `e2e0d3/69e48e/55a9e8/e45549/e9cca0`, SportObstacles
  `f7f7f7/349857/3a85cf/c51818/222222` (White/Green/Blue/Red/Black).
* The placement colour byte (`0x03043062`: 0 Default, 1 White, 2 Green, 3
  Blue, 4 Red, 5 Black) reaches an embedded item exactly like a stock one —
  through the material's hue mask. WhiteShore renders every non-zero colour
  GREEN on stock and ours alike; 0 = natural.
* **The flag's colour is NOT the placement byte**: 08's 39 flags carry byte 5
  and render GREEN; colour 0 shows the ENVIRONMENT's skin (WhiteShore: black
  cloth with the "WHITE SHORE" logo); `ItemFlag_D` is green with a Stadium
  logo, hue-masked; the flags' national/seasonal skins are placement `FileRef`s
  (`Skins\Stadium\ItemFlag\Summer.zip`, CDN `…_NationsARGENTINA.dds`).
* A BLOCK's colour tints `Tech3 Block PyPxz` materials through their `_X2`
  texture on a block-only path an item's byte never reaches (hypothesis for
  the grey `Deco` slope tops of 20 cp3; unmeasured).
* `TDOSN`/`TDSN` do not read `colour0` (a 0x40→0xFF radial ramp rendered
  identically).

## 6. Light and screen skins

* `Skins\Stadium\LightColors\<Name>.dds`: 20 game swatches (16×16 DXT1, one
  colour, from `Packs/Stadium_Skins.zip`); the item header lists the skinnable
  `_I` textures and the swatch replaces each (glow AND projector); `GxLight`
  colour ×= swatch as sRGB 0..1 (not linearised); `Off` drops the light; the
  older `Skins\Stadium\LightTube\<Name>.zip` (an EMPTY zip; the name is the
  colour) colours the stock tubes. Baked: `_I` materials → TDSNI custom
  materials with the swatch as D + I (`Items/LightColor_<Name>.dds`); our tube
  glass glows dimmer than stock (TDSNI illum has no boost).
* Screens: a model's `0x090F4000` header declares `Any\Advertisement6x1\` etc.
  and the game REMAPS the texture FILE by name (default = the collection's ad
  program; else the placement's `packDesc` FileRef, e.g.
  `Skins\Any\Advertisement4x1\Blue.zip`); 164 Stadium items / 254 block infos
  declare skins (48 `Advertisement4x1\Reversible\` = the chevron VIDEO decks).
  A baked static item has no texture fid, so the remap never reaches it: its
  panels show the material's default (`RaceAd6x1.dds` 512×80, `Ad4x1Screen`
  2048×512, `Ad2x1Screen` 1024×512, `Ad1x1Screen` 512×512, `Show4x1` 128×32)
  or the ad-less green LED grid; the original's live ads come from the Anzu
  SDK at run time (creatives from `optimized-prod.prod.anzu-us.com`, cached in
  `C:\ProgramData\Trackmania\Cache\*.jpg|png|dds`). The waypoint decks' red
  chevrons are the `64x10` display-control TV program
  (`BitmapDisplayControlDefaultTVProgram_64x10A/B/C`), unreachable for items;
  the chrono/speedometer FUNCTIONS (`CFuncPlug` InputValId
  `ChallengeStartLight5` / `RaceTime`; `StartSign5`, `ChronoFinish-*`
  FuncShaders) DO run on baked gates.
* Nation MODS (`Skins\Stadium\Mod\…Mod_Nations<X>.zip`, cached as
  `C:\ProgramData\Trackmania\Cache\<hash>.zip`) retexture the same 20 block
  textures per nation (`RoadTech_D`, `RoadIce_D`, `TrackBorders_D`,
  `DecalPlatform_D`, `DecoHill2_D`, `ItemFlag_D`, `ItemPillar*`, … + HueMasks)
  — block materials, seemingly not `CPlugMaterialUserInst` links.

## 7. Not known

* `UvAnim` fields, `HidingGroup`, `0x090FD002`'s int.
* Whether a mod reaches user-inst links at all (observed: platform decks not
  retextured).
* The block-only `_X2` tint path.
