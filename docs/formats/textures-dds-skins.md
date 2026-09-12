# Textures, skins, mods: `.dds`, `.Texture.gbx`, skin zips, `CPlugBitmap`

What the game reads as an image, where skins live, what an embedded item can
and cannot feed. Code: `tools/mapgeom/src/static_item/{texture,signlogo}.rs`
(DDS reader/writer, mip chains, caps), `tools/mapgeom/src/light_skin.rs`,
`mapgeom tex-stats`, `dds-from-raw`; facts from the tiny campaign (memory
`tm2020-tiny-campaign.md`, extract §3). Confidence **[FILE]** for the DDS
layout, **[VERIFIED-GAME]** for the render rules.

## 1. DDS as the game wants it

```text
0    "DDS "           4    u32 124 (header size)        8    u32 flags
12   u32 height       16   u32 width                    20   u32 pitchOrLinearSize
24   u32 depth        28   u32 mipMapCount              32   reserved[11]
76   pixel format: u32 32, u32 flags (0x4 fourCC | 0x40 RGB | 0x1 alpha), fourCC at 84 ("DXT1" "DXT5" "DX10"),
     u32 rgbBitCount at 88, four masks (A8R8G8B8: R 0x00FF0000, G 0x0000FF00, B 0x000000FF, A 0xFF000000)
108  caps (0x1000 TEXTURE, + 0x8 COMPLEX | 0x400000 MIPMAP with a chain)
128  data; a "DX10" fourCC adds a 20-byte extension (DXGI format at 128)
DXT1/5 level size = ceil(w/4)·ceil(h/4)·8 or 16
```

**[VERIFIED-GAME]** rules, each learned from a panel that drew wrong:

* A user picture must be **block-compressed WITH a full mip chain** (BC3/DXT5
  as `write_dds_picture` writes it). The uncompressed A8R8G8B8 no-mips form
  drew as the green/purple missing-texture checkerboard (sign logos, the
  screen picture, 2026-09-09/10) while every custom texture that DID render —
  tree atlases, light swatches — was DXT with mips.
* The green/magenta pattern is ALSO the "still streaming" look — shoot late;
  and a material's MISSING texture renders NEON GREEN with magenta fringes
  (a lineup must name every texture the items use).
* The game samples with **v flipped** (`v_uv = 1 − v_tex`); a screen picture
  keeps the pack's row order (a flip drew it upside-down / mirrored).
* Leaf atlases: the pack chain re-encoded as DXT5 is kept (own
  coverage-preserving mips come out THINNER — the pack already doubles alpha
  coverage down the levels); a 256/512 palm atlas shows leaflet slits as
  polka-dot holes at 10 m, 1024 resolves them; the item shader's alpha test
  cuts less hard than the vegetation shader, so a per-atlas alpha gain
  (`leaf_alpha_for`) is applied; pack leaf atlases are ~80 % transparent, the
  big oak 1024×512.
* Textures are cached by NAME for the whole session (like item models): a
  re-encoded picture under a fixed name stayed stale; per-build name suffixes
  (`TINY_PICTURE_SUFFIX`) or a fresh game.
* `UGCErrorsLog.txt` logs `File not found: 'SignLogoTurbo.dds.png'` per load —
  the game wants a `.png` beside a sign-logo DDS (cosmetic).
* Any user picture on the gate SIGN panel (`TDSN`) draws as checkerboard in
  both RGBA32 and DXT5 forms — the pack's own `SpecialSign<Kind>` material
  draws the chevrons itself (`TINY_SIGNLOGO` off since 2026-09-11).
* Stock picture sizes: `RaceAd6x1` 512×80, `Ad4x1Screen` 2048×512,
  `Ad2x1Screen` 1024×512, `Ad1x1Screen` 512×512, `Show4x1` 128×32;
  `SpecialSign<Kind>_I` 256² DXT1; light swatches 16×16 DXT1.
* ffmpeg reads a pak `.dds` directly; `mapgeom dds-from-raw IN.rgba WxH
  OUT.dds` writes an RGBA8 one; `dds_cap` cuts the top levels of a chain so a
  2048×1024 leaf atlas (2.8 MB) rides as its 512×256 level (175 KB) under the
  25 MiB upload cap.

## 2. `.Texture.gbx` (`CPlugBitmap`, `0x09011000`) and what an item's reference table may name

The pak's `.Texture.gbx` is a 363-byte wrapper around the `.dds` path. A
texture the engine can read FROM USER SPACE (an embedded item's own archive
folder) is the pack `CPlugBitmap` with the legacy chunks
`0x09011019/020/023/025/028/02A` stripped and chunk `0x09011036` typed
`{ version, ref, LOOKBACK Id, ref }` (a standalone body carries its own
lookback version word), its image ref → `Image\<name>.dds` beside it
(`<name>.Texture.gbx` + `Image/<name>.dds`; `TINY_FX_TEXTURE=file`). The
user-file `CPlugBitmap` reader of this exe dispatches only chunks `0x2B–0x2E,
0x30, 0x32–0x3A` (`exe+0x3f78eb`); the pack files' `0x19..0x2D` come from a
descriptor table at `0x141e9a49c` — an INLINE bitmap is read differently from
a file and crashes (`Corrupted ReadString? Length = 0x09011023`). A bare
`.dds` named as a Gbx crashes (class id `0x40000000` read off the DDS bytes);
a missing referenced file drops the item silently; loose `.dds` files in the
archive are harmless.

`CPlugMaterialUserInst.UserTextures` paths that work: a GAME path
(`Stadium\Media\Texture\Image\RaceAd6x1.dds`) or a DDS in the map archive in
the ITEM'S OWN folder, referenced by bare name (`materials.md` §1). Not:
png/jpg/webp/tga, subfolders, `Items\…`, `Skins\…`.

## 3. Skins

| skin | where | what |
|---|---|---|
| **car skin** | ghost chunk `0x03092000`: `Skins\Models\CarSport\<Name>_<uuid>.zip` + locator `https://core.trackmania.nadeo.live/storageObjects/<uuid>` (`ghost-cgamectnghost.md` §3) | the driver's car livery; the zip itself is never opened by this project; the path carries the uuid the anonymiser must clear |
| **advertisement / screen skins** | header `<dep file="Skins\Any\Advertisement4x1\Summer4x1.zip"/>`, `…\Bottom+000A.webm`, `…\Left+000A.webm`; a placement's `packDesc` FileRef (`Skins\Any\Advertisement4x1\Blue.zip`, v3 with a 32-byte checksum) | applied through the model's `0x090F4000` skin declaration (`item-cgameitemmodel.md` §4); `Advertisement4x1\Reversible\` = the chevron VIDEO decks; unreachable by a baked item |
| **light colours** | `Skins\Stadium\LightColors\<Name>.dds` (20 swatches, 16×16 DXT1, one colour, from `Packs/Stadium_Skins.zip`); older `Skins\Stadium\LightTube\<Name>.zip` (EMPTY zips; the name is the colour) | `GxLight` colour ×= swatch as sRGB (not linearised); `Off` drops the light; `materials.md` §6 |
| **flags** | `Skins\Stadium\ItemFlag\Summer.zip` / `Winter.zip` (not in the pak, GameData or Documents/Skins); CDN `Skins\Stadium\ItemFlag\_nadeo-download-cdn…_NationsARGENTINA.dds` from `nadeo-download.cdn.ubi.com/trackmania/assets/2026/ItemFlag/` | placement FileRefs on 12/13/15/21–24; a baked flag item cannot take one (comes out default blue) |
| **nation mods** | header `mod=` / `Skins\Stadium\Mod\…Mod_Nations<X>.zip`, cached `C:\ProgramData\Trackmania\Cache\<hash>.zip` | retexture 20 block textures per nation (`materials.md` §6); not the platform decks, seemingly not user-inst links |
| **block skins** | `CGameCtnBlockSkin` node on a `0x8000`-flagged block record (`map-blocks.md` §4) | strings + FileRefs; checkpoint blocks carry none (the yellow NADEO panel is the default ad) |
| **game skins** (`*.GameSkin.gbx`) | `Stadium\GameSkin\{Platform, Specials, SpecialsOriented, Penalty, TrackWallToDecoCliff, GateGameplay, ItemObstacle*, ColorBlindness, StadiumOnTerrain}` | slot tables `slot name = the pack material the prefab is authored with`; the modifier mechanism (`materials.md` §3) |

## 4. Where the game keeps pictures

`C:\ProgramData\Trackmania\Cache\*.jpg|png|dds` = map thumbnails, club
banners, community sign packs, mod zips (`C0C53E74…zip`), lightmap caches
(`map-lightmap.md` §2.1); the live in-game ads come from the Anzu SDK at run
time (`optimized-prod.prod.anzu-us.com`), not from disk;
`C:\ProgramData\Trackmania\Anzu\anzu.db` is the SDK's JS.

## 5. Not known

* The car-skin zip's internal layout (never opened here).
* `CPlugBitmap` chunk semantics beyond "strip these, type 036".
* Whether a user-inst link can be reached by a mod at all.
