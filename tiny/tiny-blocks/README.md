# Tiny Blocks — the Stadium block set at half scale, as items

Every block of the Trackmania (2020) Stadium block browser at scale 0.5, as
items you place freely in the editor: 3,757 blocks, 7,229 items counting the
ground variants. The same pieces the *Tiny Summer 2026* campaign (club
**Vjeux**) is built from, so you can build tiny maps of your own.

## Install

Unzip into `Documents\Trackmania\Items\` so that you get
`Documents\Trackmania\Items\TinyBlocks\Roads\…`, then **restart the game** —
the editor indexes the Items folder at launch, not while it runs.

Two ways to get it:

* **GitHub** (everything, one zip per block family — they install
  independently — plus the Tiny Items zip): <https://github.com/vjeux/trackmania-tas/releases/tag/tiny-blocks-v2>
* **item.exchange** (the same items as ~130 sets of at most 28 MB, one per
  block family or part of one, each named *Tiny Blocks - Family (part: the
  sub-folders it holds)*, plus 9 *Tiny Items* sets): the sets of
  <https://item.exchange/setsearch/search?userid=56665>

| zip | folder | items | on disk |
|---|---|---|---|
| TinyBlocks-Roads.zip | Roads (Tech, Dirt, Bump, Ice, Water) | 1,947 | 1.4 GB |
| TinyBlocks-Platforms.zip | Platforms (Tech, Dirt, Ice, Grass, Plastic, Water) | 1,787 | 0.86 GB |
| TinyBlocks-Opens.zip | Opens (Tech, Dirt, Ice, Grass) | 844 | 0.14 GB |
| TinyBlocks-Terrain.zip | Terrain (Grass, Dirt, Ice, Water) | 1,094 | 0.33 GB |
| TinyBlocks-Themes.zip | Themes (Snow, Rally) | 237 | 0.09 GB |
| TinyBlocks-Technics.zip | Technics (gates, structures, lights, screens) | 307 | 0.14 GB |
| TinyBlocks-Walls.zip | Walls (TrackWall, DecoWall) | 1,013 | 0.19 GB |
| TinyItems.zip | Nadeo's items at half scale (see below) | 596 | 0.08 GB |

(Sizes on disk; the zips are about a third smaller.)

## In the editor

Item mode → **Custom → TinyBlocks** → the same folders as the block browser
(Roads / RoadTech / Main / Main / RoadTechStraight …), with the blocks' own
icons and names. A tiny block unit is **16 × 4 × 16 m** (half of 32 × 8 × 32).

* **Grid**: items snap every 8 m horizontally and 2 m vertically (half a tiny
  unit), the fly step is 2 m — any two tiny blocks tile edge to edge, in every
  90° turn. The Stadium grass is at 8 m, so a piece put on the ground lands
  exactly where a block would.
* **Pivot**: the centre of the footprint, so a piece rotates in place.
* `Block` is the block as it looks anywhere but on the ground row (the *air*
  variant); `Block_Ground` is the ground variant (the grass or dirt skirt the
  game draws around a block sitting on the ground). Only blocks whose ground
  variant shows something the air one does not have both (a ground variant
  that merely lacks the underside is not shipped: the air item stands for it).
* Every item includes what the game draws around a **lone** block — end caps,
  undersides, side skirts. Two tiles side by side hide those faces inside the
  joint, like the original blocks do.
* Starts, checkpoints, finishes, multilaps and the gameplay gates (turbo,
  boost, reset, no-engine, fragile, …) work as they do on the blocks — a
  tiny start, straights, checkpoint and finish placed in a row validate
  in-game. A checkpoint placed as an item is a free checkpoint: give the map
  a start and a finish and it validates like any item map.

## Known limits

* The pillars the game grows under floating blocks are not part of a block;
  place tiny pillars (`Technics/StructureSupport`) yourself.
* Water blocks are visual: an item cannot carry the game's water volume.
* The animated "curtain" of the gameplay gates is not drawn on items.
* Detail levels switch at half the original distances (all of the block's
  levels are kept).
* An item is the whole block; a block's sub-objects that repeat (borders,
  supports, lights) are stored once and placed many times inside the item,
  as in the game's own files.

## How it was made

Generated from the game's own block data by the converter behind the Tiny
campaign, `mapgeom item-set` in <https://github.com/vjeux/trackmania-tas>
(`tools/mapgeom`): each block's prefabs and the free-clip fillers the engine
derives for it, at scale 0.5, as one item: every distinct sub-object once
with its placements, the block's materials, collision, waypoint trigger,
spawn point and icon. Report a wrong piece with its
name and a screenshot.

## Tiny Items — Nadeo's items at half scale (TinyItems-*.zip)

The same for the **item** browser: every Nadeo item of the Stadium item
browser at scale 0.5 (596 items — decorations, flags, signs, screens, light
tubes, gates, podiums, inflatables, moving obstacles, supports; the 36
vegetation items are left out: as loose items the trees lose their leaf and
bark textures), under `Items\TinyItems\<the item browser's folders>\`, with the
items' own icons. Each keeps Nadeo's own placement settings scaled by half
(a 32 m gate snaps every 1 m instead of 2, its magnets at ±8 m instead of ±16).
Where the game data did not yield the settings, the item-editor defaults
(0.5 m snap, free rotation) stand. Moving obstacles keep their animation;
the size groups that make Nadeo items snap to one another are not carried
over.
