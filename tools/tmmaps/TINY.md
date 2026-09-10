# Half-scale campaign maps

## What the GranaTV maps do

The reference video is [Trackmania, but the Maps are Shrinked!](https://www.youtube.com/watch?v=Dlb0rwvvpkU). The maps are Everios96's `U10S TINY` campaigns in the Altered U10S club.

The files establish the implementation, rather than just the visual effect:

- the original `U10S_01` has 330 authored blocks and 152 items;
- `U10S_01 By Everios96 [Tiny]` has **zero authored blocks** and 119 items;
- its route is rebuilt from embedded custom `Tiny*.Item.Gbx` models;
- corresponding placement deltas are exactly halved (for example, the start-to-finish delta goes from `(384, 0, 96)` to `(192, 0, 48)` metres);
- the custom item placement scale remains `1.0`, so the geometry was scaled when each Item.Gbx was authored, not by changing a native block's placement;
- the baked stadium floor remains full size.

In short: native blocks are first converted to items, those item meshes are made half-size, and a second map is assembled with all route placement offsets multiplied by `0.5`.

## Rust automation

`tmmaps tiny` now requires a mapping for **every authored (unbaked) block** and scales every existing item. It refuses to emit a partial map. The baked/generated decoration remains the full-size foundation, matching the reference campaign's treatment of its baked Grass floor.

```text
tmmaps tiny SOURCE.Map.Gbx \
  --out SOURCE-Tiny.Map.Gbx \
  --mapping tiny/roadtech-half.tsv \
  --library tiny/roadtech-half-items.zip \
  --scale 0.5 \
  --anchor 1024,300,1024
```

For a directory containing a campaign:

```text
tmmaps tiny-batch Campaign/ \
  --out Campaign-Tiny/ \
  --mapping tiny/roadtech-half.tsv \
  --library tiny/roadtech-half-items.zip \
  --scale 0.5 \
  --anchor 1024,300,1024
```

The command:

1. finds the source map's block-carried spawn and uses it as the transform origin;
2. refuses unless every authored block model has an Item.Gbx mapping;
3. replaces all authored blocks with mapped item models and scales every existing item;
4. preserves every waypoint explicitly;
5. DELETES the original authored blocks and the generated (baked) non-foundation fillers from the block chunks — `MapFile::remove_blocks` re-serialises 0x0304301F and 0x03043048 with the shared lookback table re-encoded, and rewrites everything that lists blocks (0x0304305F free positions, 0x03043062 colours, 0x03043068 lightmap quality, 0x03043069 macroblock refs, the snapped-on tables of 0x03043040); the baked `Sea` records stay (the water); `--keep-zone-block` keeps ONE authored block for `tinyctl lightmap` (the editor's lightmap pass crashes on a map with none);
6. embeds the supplied converted-item archive;
7. assigns a distinct 27-byte map UID (`Tiny` + the first 23 source UID bytes);
8. reloads the written map and verifies every generated item model, position, scale, and waypoint tag.

The transformation is intentionally split into three write/reload stages. Item model renames change the GBX lookback table, while waypoint nodes and the embedded ZIP are variable-length records. Applying both against one set of offsets is unsafe.

## Current Summer 01 status

Summer 2026 - 01 contains 2,430 authored blocks, 1,704 existing items, and 2,214 baked/generated foundation blocks. The public converted-Nadeo archive provides exact Item.Gbx models for only 23 of the map's 46 distinct authored block models. The remaining 23 are mostly BlueBay terrain (`Land*`, `Beach`, `LandHill*`, `LandCliff12`) plus several support/decor variants.

The command therefore **refuses** Summer 01 today rather than silently dropping half the map. The earlier route-only artifact is not a complete conversion and should not be used as one. Once the 23 missing items are exported from the game, adding them to the mapping/library is enough; the all-object writer and its item-array growth path are implemented and tested.

## Triggers: what the engine reads from an embedded item (measured 2026-09-08)

Three facts, each paid for with a drive on the render box (tiny Summer 20,
full throttle from the spawn through the Boost gate, `tinyctl play --wheels-ms`):

1. **A gameplay gate's effect is an `NPlugTrigger_SGateSpecial` prefab entity
   (class 0x09179000: version 2, trigger shape ref, u32), nothing else.** The
   same slab written as the entity model's `TriggerShape` — with or without a
   material node, gameplay 1/12/18 — does nothing; in the collision hull as
   NotCollidable + gameplay it is a wall (physics 28 does not make hull
   triangles pass-through). So a special-gate item is written in the pack's
   own layout, `CGameItemModel -> CPlugPrefab { static object, SGateSpecial }`
   (`Merged::special`), and fires: 23.6 → 36.8 → 45.6 → 50.9 → 54.9 m/s past the
   gate where the original does 30 → 62.
2. **The effect id is the gate kind's `Modifier\<Kind>\Collision.Material.Gbx`**,
   chunk 0x09079017 = `{ v1, [physics, gameplay, u8, 0x80], f32, u32, string }`:
   Boost 18 (ReactorBoost_Oriented), NoEngine 4 (FreeWheeling), Reset 8; Turbo
   has no Collision file and the prefab's own slab bytes (physics 0, gameplay 1)
   stand. The id table entry is `physics | gameplay << 8`, as everywhere else.
   Item gates carry the slab in their prefab (`Special24m.Prefab.Gbx` entity 1,
   shape `Special_Trigger24m.Shape.Gbx`); BLOCK gates (GateSpecialBoost…) carry
   the disc inline in the block info (CPlugSolid → CPlugTree → a 68-triangle
   surface in `Effects\Media\Material\CollisionTurbo`), the prefab being the
   arch alone. `Reset.TerrainModifier .Gbx` is spelled with a space.
3. **Block waypoint triggers are Compound surfaces** (`Gate\Checkpoint_Trigger
   .Shape.Gbx`: one 36-vertex disc; the platform checkpoints a 0.1 m plane) and
   must be flattened (`Surf::triangulate`), or the bake falls back to the unit
   box and a finish fires on its whole 32 m cube. The item editor writes the
   same 36/68 disc for Granady's `Tiny_Ring1`.

## Trees: half-size static snapshots of the vegetation (2026-09-08)

A `VegetTreeModel` placement cannot be resized — the game instances the
species at its authored size — so until 2026-09-08 every source tree was
placed as a STOCK species one size class down and stood ~2× too tall beside
the half-size track ("the trees look really weird at double the size, can we
make a static model half the size?" — vjeux, after playtesting all 25).
`mapgeom tiny-library --veget bake` (the default) bakes every species the map
places as ONE half-scale static item, `AV%08d.Item.Gbx`, from the pack's
`.VegetTreeModel.Gbx` (`mapgeom veget-info PATH` prints one; all 193 species
files of the five collections decode), placed at the source position × 0.5
with the source yaw like every block: no stand-in species, no sink, and the
tree clearance judges baked trees for the census only (a half-size tree at a
half-size place meets a deck exactly when the original did).

What the bake keeps and what it gives up:

* **Kept**: every detail level's visuals at 0.5, under one item material per
  model material; the trunk hull as the collision (Wood); the model's own LOD
  switch distances, UNSCALED (a half-size tree switches where the full one
  did); `--lod-pick` applies like to blocks (level N alone, no ladder).
* **Materials**: the vegetation materials are inline (name + D/N/R images, no
  pack `.Material.Gbx`), so each becomes an item-editor custom material —
  the bark under `TDSN` with its diffuse in user-texture slot 0 (Diffuse), the
  leaf cards under `TDOSN` with the atlas (DXT5 with alpha, mips cut to 256 px
  a side) in slot 1 (DiffuseO — the diffuse whose alpha is the OPACITY,
  alpha-tested), riding next to the items as `Items/<image>.dds`. The slot
  enum was read off the exe on 2026-09-09 (`crystal_model::USER_TEXTURE_SLOTS`):
  Diffuse 0, DiffuseO 1, BaseColor 2, BaseColorO 3, Specular 4, Normal 5,
  Energy 6, TeamMask 7, SelfIllum 8, Damage 9, Dirt 10, Shield 11, RoughMetal
  12; at most 8 entries (past that the reader drops them all). Which slot a
  model READS decides everything: under `TDSN` the same atlas in slot 0 drew
  every card as an OPAQUE quad with the leaf picture on the atlas' tan/grey
  background — the "origami" crowns vjeux saw on all 25 maps — and under
  `TDOSN` with the atlas in slot 0 the cards were invisible (slot 1 at the
  game's default image, which is transparent). Lineup `y1` on tiny 19
  (2026-09-09, TreeBigA at 14 m and 6 m, BushMediumD at 7 m): TDOSN and
  TDOBSN with the atlas in slot 1 cut every card to its leaves. Earlier facts
  that stand: a visual without a TexCoord1 set is NOT DRAWN under any valid
  material (the pack leaf visuals carry uv0 alone → `ensure_texcoord1`);
  TDOSN2Sided is not a model the item loader knows (red); custom materials of
  one NAME are one material to the game (thirteen palms whose `PalmTree_Leaf`
  differed only by model crashed the client at 0x140456513), so the name
  spells the model and the image (`TDOSN_ItemPalmTreeBranch_D`). `item-check`
  SH-04/SH-05 refuse a material with more than 8 textures or one whose model's
  colour slot is empty.
* **Two-sided**: a reversed copy of every leaf triangle on the same vertices
  (`TINY_TREE_LEAF_BACKFACES=shared`; `flip` duplicates the vertices with
  reversed normals — no visible gain, +60 % leaf bytes).
* **Given up, stated once**: no wind animation (the vertex-colour wind weights
  are stripped), no impostor past the far distance (the last level is drawn
  at every distance), no placement-colour hue mask (the default look is baked
  — Summer 20's palms are placed Red, 10's Spring set Green; those tints are
  gone).
* **Count rule**: a species is baked only if it is ≥ 2 m tall AND carries a
  collision hull. Hull-less species (grass, ferns, BlueBay's JungleForestA/B/C
  — 7 m one-material trunkless cards the game instances by the tens of
  thousands) stay on the stock path exactly as before: tiny 01 is 5 255 item
  placements, not the 34 090 the first bake produced.
* **Local lights — SOLVED 2026-09-09 (it was the cards' lightmap UVs, not the
  lights)**: the item shading models answered local lights (stadium ShowLights
  rigs, the checkpoint gates' embedded lights) with a yellow-white glare the
  game's vegetation never shows — bushes standing inside a gate's beams (tiny
  24, cp8) looked like crumpled paper lanterns, and vjeux read it as "the
  lighting is way over exposed for the bright spots". The lights were never
  the lever: a stock Nadeo `Lamp` at intensity ×0.25, ×0.01 and ×0 blew our
  bush just the same, and with NO light within 40 m the bush stood
  sunlit-bright on a black lawn at night (lineup `bl`). The cause was
  `ensure_texcoord1`, which gave every card uv0 as its uv1 — every leaf of a
  crown on the same lightmap texels, so the game's per-item bake (redone at
  every load) handed the whole bush ONE arbitrary value: the rigs' baked
  light under the rigs, a sunlit value elsewhere. `tree_lightmap_uv1` (default
  `TINY_TREE_UV1=atlas`; `copy` restores the old form, `const` = one texel)
  gives every card and bark strip its own chart in a grid over the unit
  square; the night lineup `uv` shows the copy bush glowing white beside a
  normally lit dark-green atlas bush, and the same-camera cp8 frames go from
  38.7 % saturated bush pixels (mean (188,185,144)) to 2.3 % ((110,110,79))
  against the original's 0 % ((101,104,77)), the deck unchanged.
* **The embedded lights are right as baked** (radii ×0.5, intensity unchanged,
  since 2026-09-07): a half-size `Lamp` lights the ground at half height
  exactly like the stock one at full height (night lineup on a tiny-15 host,
  top-2 % patch colour (138,137,46) vs (137,135,45); intensity ×0.5 dims it,
  ×0.25 is unlit grass). The engine's falloff is a function of d/R, so the
  geometric scaling needs no intensity term — `TINY_LIGHT_INTENSITY_EXP`
  (default 0) and the `static-item --light-*` flags stay as A/B knobs. The
  deck under Poland's cp8 rigs is +10 % vs the original with the lights as
  they are and 20 % DARKER at ×0.25.
* **Merged items: one lightmap cell per PART** (`repack_lightmap_parts`,
  assemble.rs, default on; `TINY_LIGHTMAP_REPACK=0` restores the overlap). A
  pack Solid2 lays its lightmap atlas over the whole unit square, so an item
  merged from several (a block's mobils, a rig's entities, a gate's arch +
  sign + lights) stacked their charts and the game's per-item bake wrote every
  part's light into the same texels — the trees' problem in a milder form.
  Every part now gets its own grid cell (single-part items keep Nadeo's layout
  untouched). Poland at f9039561 + repack, same cameras: cp1 whole-frame
  saturation 8.3 → 2.9 % (original 4.5), the cp8 deck 0.7 → 0.2 % (= the
  original), five other views within noise; 291 of 24's 716 items are
  multi-part and change bytes.)

* **What an item CANNOT do, stated once (2026-09-10, so nobody chases it
  again)**: the stock vegetation shader lights every leaf with the sun per
  pixel and adds a SUBSURFACE term (light through the leaf — the back-lit
  golden glow), waves in the wind, and morphs to an impostor past the far
  distance; an embedded item's leaf cards are static geometry lit by the
  lightmap the game bakes at load, every card an opaque occluder to that
  bake, under a shading model with no translucency — so a crown comes out
  lit by the sky, flat and dark. None of translucency, wind or LOD morphing
  can be had from an item, whatever its material or entity kind (a
  kinematic dyna entity draws no leaves for a 12k-vertex mesh, and a small
  one's leaves come out as dark as the static ones).
* **The look, calibrated per collection (default since 2026-09-10 15:30Z,
  trees quality pass 3; `TINY_TREE_COLOR_TABLE=0` = the pass-2 rule below,
  ship16's bytes)**: `leaf_look_for` in build.rs — (colour gain, saturation,
  hue shift, DEPTH BANDS) per collection, fitted on sky-backed lineups (stock
  at 30 m beside ours at 15 m = the same angular size, sun and shade side,
  `cropstats --fg leaf` on the crown crop): GreenCoast 1.3/1.6/−25° with
  three bands (a card whose normalised radius about the crown centre is under
  0.45 draws the atlas ×0.45, under 0.75 ×0.7, the rest as is), BlueBay
  1.12/1.06/0 one band 0.6→×0.5, RedIsland 1.15/1.9/−15 one band
  0.55→×0.55, Stadium 1.2/1.1/0 one band 0.6→×0.7, WhiteShore 1.1/1.5/−10
  (the pines' setting, unmeasured). Hue is a YIQ chroma rotation, NEGATIVE =
  towards yellow — the first pass-3 bakes had the sign backwards and came out
  GREENER (rendered hue 104–128 vs the stock's 82–91); with the sign right the
  rendered hue sits within 10° of the stock on both sides. An AUTUMN atlas
  (red over green by 12, luma under 150: Populus) keeps its hue and at most
  ×1.2 saturation — the shift turned its gold salmon (the eyes on the 19
  frames). CHECKED under the LIGHTMAP (2026-09-10 17:45Z; `tinyctl shoot
  --shadows 2` computes the editor's Fast lightmap = what play mode bakes at
  load): it does not darken our cards (a little bounce, the ground 128 → 139),
  the stock gets slightly darker; the tip's residuals stay within ±20 % both
  ways with a mean near zero (TreeSmallA +16/+18, TreeBigA +6/+21, BushBigB
  −17/−11, pines 0…−10; ship16's bake under the same lightmap: +27/+50,
  +64/+65, +14/+17), so the table stands for play. `TINY_TREE_COLOR_TABLE=0`
  is the one-flag fallback. Not green to begin with = no yellow shift: an
  atlas with red over green by 4 or more (the RedIsland creosote, the
  quince) keeps its hue (sat ≤ 1.3; the yellow-green hazel at r = g measured
  better WITH the shift: hue 76/80 vs the stock's 78/80 shifted, 70/76 kept;
  the quince was a wash either way) — shifted, the creosote turned brown (leaf-mask
  coverage 0.1 % vs the stock's 5.7 %; kept, 5.4 % with luma 43/46 vs 47/52
  and hue 65 = 65). The PALE-FOLIAGE sheen on the lightest atlases (birch
  110, sous-bois 115, hazel) is a SATURATION deficit, not brightness: at the
  cap they sit at or under the stock's luma (56/52 vs 59/59, 60/58 vs 70/65)
  but 5–8 points less saturated; a lower gain darkens them further — measured,
  rejected. The fix that measured (lineups Z6/Z7): an atlas whose opaque mean
  has HSV saturation ≤ 0.30 (birch 0.25, hazel 0.26, sous-bois 0.21–0.30)
  gets ×1.3 more saturation and half the hue shift — birch 36/31 vs the
  stock's 31/30, sous-bois 35/33 vs 32/33, hazel 37/31 vs 33/32 (from 24/21,
  30/28, 28/23), luma within 15 %; the laurel at 0.32 would overshoot (40 vs
  34) and stays. `TINY_TREE_LOWCHROMA=0` turns it off. PALM FRONDS (pass 3c,
  2026-09-10 19:30Z): the "giant flat frond planes" at ≤15 m are the cards
  rendering too OPAQUE at the item shader's alpha test, not the mesh or the
  atlas — a distance ladder of the stock PalmTreeBigA1 (12→80 m) has no LOD
  step (coverage ∝ 1/d² throughout), and at the same angular size its crown
  box covers 23.6 % with an edge density (perforation) of 13.1 % where ours
  covers 29.8 %/7.4 at ANY mesh level (LOD1 26.2/7.4, LOD2 13.1/13.6) and at
  any atlas size (256/512/1024: 8.3→11.3 edge, coverage unchanged); the stock
  vegetation shader cuts its alpha harder. Per-ATLAS alpha gain on every mip
  level (`leaf_alpha_for`): PalmTreeSugar ×0.6 with its 512-px level back
  (23.9/13.6 n, 26.3/14.1 s vs the stock's 23.6/13.1, 24.8/13.7; +260 KB on
  Summer 06), WhiteBarkPalmAtlas ×0.75 (×0.6 overshoots: 19.7/24.1 vs
  25.6/19.0). The Stadium palm already matches (47.9/8.0 vs 45.3/6.3) and the
  GreenCoast crowns must NOT get it: an alpha cut raises their rendered luma
  10–40 % (the leaf centres are the bright texels) and the bushes are already
  thinner than the stock (BushBigB 8.8 vs 12.2 %). Measuring trap: a crop
  smaller than the crown (the 800×900 centre crop on a 12 m TreeBigA) reports
  coverage of the crop, not of the tree — locate the crown with
  `cropstats --grid` first. The bands are the SELF-SHADOW the per-item lightmap does
  not give (every card gets one value): the inner cards ride under a darkened
  copy of the atlas (one more material and a 128-px atlas per band; +1.4 MB on
  Summer 19). Measured against the stock at 15 m (sky behind, the stock's own
  luma moves ±15 % between shots as clouds pass — it is lit dynamically, our
  items are not), luma sun/shade of the last verification round: GreenCoast
  TreeSmallA −16/−23 % at gain 1.1 → 1.3 now (pass 2 was +22/+13), TreeBigA
  −12/−6 (pass 2 +46/+40), BushBigB −19/−16 (pass 2 +37/+29); BlueBay palms
  +3/0 and 0/+5 (pass 2 +40/+49, +47/+50); RedIsland pines −15/−24 and
  −15/−10 at gain 1.0 → 1.15 now (pass 2 +36/+29, +39/+33); Stadium palms
  +13/+1 and +4/+1, spring tree −10/−20 at band ×0.6 → ×0.7 now. What the lineups
  also settled: our crown is NOT sparser than the stock's (coverage 50 % vs
  35 % at equal size — the LOD0+LOD1 union `TINY_TREE_DENSE` moves away from
  it), and TDOSN/TDSN do not read colour0 (`TINY_TREE_VCOL_AO`, a 0x40→0xFF
  radial ramp, renders identical). Measuring trap: a lineup at y 150 has the
  map's cloud band behind the crown, and warm-grey cloud passes a plain
  green test — the pass-2 numbers were inflated by it; stand the lineup at
  y 300 (sky) and use `--fg leaf`.
* **The pass-2 look (2026-09-10 morning, ship16)**:
  the leaf atlas is brightened and saturated at bake and the cards get
  RADIAL normals. Measured on same-camera lineups (stock species at the
  same angular size beside ours, both sides of the sun, 4K, GreenCoast
  TreeSmallA / BushMediumD / TreeThinSmallA / TreeBigA / BushBigB, then the
  palms, pines, cherry and firs of the other collections): untouched, our
  crown was a flat dark blob (mean luma about half the stock's, hsat 18 %
  vs 40 %); a colour gain of 1.6 matches the small trees, 1.3 the bushes and
  the big oak, autumn foliage wants 1.6 whatever its atlas, 2.0 overshoots
  everything. Rule: gain = clamp(132 / L, 1.0, 1.65), L the atlas' mean
  luma over its opaque texels (HoneyLocust 80 → 1.65, Cercidophylle 92 →
  1.43, BigOak 96 → 1.38, pine 87 → 1.52, the palms 100–113 → 1.2–1.3, a
  snowy fir 183 and the pink cherry 210 → 1.0 — a gain of 1.6 bleached the
  cherry white), at least 1.6 for a warm mid-luma autumn atlas (red over
  green by 12, luma under 150: Populus 118); saturation ×1.15; the pack's
  own mip chain re-encoded
  (`TINY_TREE_LEAF_MIPS=pack-adj`; `pack` = untouched bytes,
  `TINY_TREE_LEAF_COLOR_ADJ=GAIN,SAT` by hand, `1,1` = off). The card
  normals become radial from the crown centre (`TINY_TREE_NORMALS=shell`;
  `model` = the pack's, `up`): the lightmapper takes its irradiance
  direction from them, and the crown gets a lit side and a shaded side
  where the model's mixed normals gave none. Probes that changed nothing
  visible and stay knobs: a black constant in the Specular slot 4, the
  pack's normal map in slot 5, a 512-px atlas, the placement's
  lightmap-quality byte (0/1/2/3), the PreLightGen scale word (8 / 128 /
  none), an own alpha-coverage-preserving mip chain (THINNER than the
  pack's chain, which already doubles the alpha coverage down its levels —
  `coverage` / `plain` modes), a kinematic dyna entity
  (`TINY_TREE_KINEMATIC`). Tooling: `tinyctl treelineup` (a species ×
  variant lineup with its cameras), `tinyctl cropstats --fg green` and the
  hsat/fill/edge/bright columns.

Sizes with `--lod-pick 1 --lod-pick-min-verts 2000`: 01 8.1 MB (5.9 without
trees), 20 13.8, 17 12.3, 24 25.4 (45 MB without the pick), 25 26.9, 21 29.2
(25.5 without trees; 28.3 with `TINY_TREE_TEX_MAX=128 TINY_TREE_LOD_MIN=1`).
Playground open time is unchanged within noise (01: 4.4 s with trees, 4.0 s
without; 20: 4.5 s; 17: 4.6 s).

## Particle effects: the small stock siblings (2026-09-08)

An embedded item cannot carry a live particle emitter in this build: the game
silently DROPS any item whose prefab has an FxSystem entity with a model —
geometry included, no dialog (sixteen one-item probes; `item-check` FX-03
refuses the form). So the foggers, sparklers and torches baked as static
machines with no smoke, sparks or flame — until vjeux asked "can we use a
smaller version of the fog machine so we still get the effect?", the flags'
trick (`Flag16m` → the stock `Flag8m`, whose cloth is the 16 m cloth at ×0.5).

Nadeo ships each particle item in a smaller version whose NAME is the effect's
REACH, not the machine's size, and `stock_half_variant` maps to it:

| source | stock stand-in | the machine | the effect |
|---|---|---|---|
| `ShowFogger16M` | `ShowFogger8M` | same 0.6 m wide box, 0.7 m long for 1.0 | plume carries 8 m for 16 |
| `ShowFoggerWithLight16m` | `ShowFoggerWithLight8m` | same, `FoggerSpot` light at the nozzle | same |
| `Sparkler16m` | `Sparkler8m` | 0.40 m box for 0.51 | sparks reach 8 m for 16 |
| `ShowTorch` | `ShowTorchSmall` | 1.64 m torch for 2.77 (fire at 0.97 m for 1.88) | `TorchSmoke` + `ItemTorchFlame` + `TorchLight`, the game's |
| `Show` variant 28 | `ShowFogger8M` | the generic show rig's fogger variant IS the `Fogger16M` prefab | as the fogger |

Placed as STOCK items (mapping row `model_scale = scale`, so the placement
stays at scale 1 with its pivot halved; the variant byte rewritten by an `iv@`
row where the source's indexes a different list) the game runs its own
particle systems for them: the half-reach effect on the half-size map for
free, no archive bytes (a map loses one embedded item per stand-in, ~20–30 KB).
Pivots need no correction — every pair shares the origin convention (fogger
base at y −0.088, sparkler base at 0, torch stake below 0), measured with
`mapgeom dump` / `mapgeom model … --out X.obj` on the pack prefabs.

The stand-in's NAME is the item file's header ident, case-exact, never the pack
path: `Stadium\Items\ShowFogger8M.Item.Gbx` says `ShowFogger8m` inside, and a
`ShowFogger8M` placement is silently DROPPED by the game (no dialog; the item
census the probe plugin writes after a load is the tell — `items=N` under the
file's placement count). `build` reads every stand-in's ident back from its file.

Verified in the editor on tiny 15 (22 foggers, 13 sparklers) against the
original from the same cameras at half distance: plumes and spark bursts at
the original spots, the machines on their decks, the same apparent size (=
half reach). Sparks are periodic bursts — repeat a view row under several
names to sample the animation; a single frame may miss them.

The FX family, surveyed on the pack (`mapgeom refs` over every `Show*`,
`Sparkler*`, `Torch*` item's prefabs): the emitters live ONLY in the eight
prefabs above (`Fogger16M/8M.FxSys`, `Sparkler16m/8m.FxSys`, `TorchSmoke.FxSys`
shared by both torches). `ShowLights`, `ShowScreen`, `ShowRace`, `ShowSpeakers`,
`ShowLight4Spots`, `ShowLightRamp*`, `ShowSpeaker*`, `ShowRig*` carry none —
nothing is lost baking them.

Given up: the maps' own `Sparkler8m` placements (168 over 03 06 10 19 20 21 24)
have no 4 m sibling and stay static, sparkless (`TINY_FX_SPARK8=stock` keeps
them as the stock item: live sparks at their full 8 m reach — an A/B knob).
The stock machine is the ORIGINAL's size, i.e. twice the relative size of a
baked half copy; at 0.6 m it reads as a small box beside a half-size car.
`TINY_FX_STOCK=0` bakes the family static as before.

## Animated items: what moves by itself and what borrows (2026-09-08)

Measured in the editor on a tiny-18 host with one-item lineups (thread
dc2cf9dc, `/tmp/anim` on devvm63031, frames in `tinyshots/an2..an13`):

**Kinematic parts animate.** Our half-size `ObstaclePusher8mLevel1` piston
travels and its screen texture cycles (four frames beside the stock 8 m and
4 m Level1 pushers); our half-size `ObstacleRotor16mHolesX4Level1` turns. The
kinematic dyna kind (the prefab entity's `SInstanceParams.IsKinematic`, the
game's kind 0x914F000) needs nothing more than what `add_dyna_part` writes:
the inline mesh, both hulls (a missing hull crashes the loader at
`Trackmania.exe+0xb7088c`), the Level modifier's constraint. The plain
`ObstaclePusher8m` is the OFF pusher (its own constraint has zero travel); the
Level modifiers swap in `AnimPusher8mLevel0/1/2` = 8 m over 8/4/2 s.

**The tween cloth borrows.** The flag cloth is a visual-only dyna (kind
0x914E000) whose vertex-tween frame state the game never gives an embedded
copy: alone in a map our cloth is a bare pole. With ANY stock flag DRAWN in the
same view it waves — properly, in its own placement colour — because its draw
reads the per-material frame state the stock's draw filled, and that state
indexes the stock's frame table at the stock's CURRENT detail level: the cloth
is right only while driver and cloth are at the same level, garbage shards or
a giant sail wherever they differ, nothing when the stock is in the map but
out of view. Every byte-level variant of the item tested the same (u13,
collector flags, one level, the dyna and mesh as sidecar files, the cloth as
a kinematic dyna with hulls, the pack flags' `NPlugItem_SVariantList`
wrapper); any reference to a PACK file gets the item dropped.

⚠ **HACK — hidden stock Flag8m driver per Flag8m placement** (`tmmaps tiny`,
`TINY_FLAG_DRIVER`, default `twins`): our cloth keeps the PACK detail ladder
(`[16, 64, 128, 256]` on FlagSmall — `item-check` TW-01 refuses any other) and
a stock `Flag8m` hangs UPSIDE DOWN at every converted placement, pole and cloth
into the ground, so both are the same distance from the camera and switch
level together. Measured: proper waving cloth in the placement's own colour at
10–200 m (green/blue/default cloths over red drivers). The guard in
`tiny-library`: a placement whose two cells below are neither terrain nor
covered by an authored non-pillar block (a flag on a deck over open air) gets
an `xf@INDEX` row — no driver there, and the placement uses a second copy of
the item with the STILL cloth (frame 0 under `ItemFlagNoAnim`, the pseudo
skin key `still`) instead of a bare pole. Both counts are printed as
`⚠ HACK` lines in the build report. Summer 13: 22 driven, 68 still (flags on
elevated roads). The proper form is a self-contained embedded tween; the
registration the pack flag gets (kind-0x16 handler → SInstanceParams →
CHmsMgrVisDyna::InstanceCreate) is still unlocated in the exe.

**Status 2026-09-08 20:07Z — the driver hack is OPT-IN (`TINY_FLAG_TWEEN=1`),
the default is yesterday's still cloth.** The hidden driver is not yet reliable:
stable at ~35 m on the tiny-18 snow (four frames, colour-independent) and
always stable with drivers standing in the OPEN air 8 m below the cloth (10 to
200 m, one or four drivers), but at 12 m the poles are bare and in tiny 13 at
25 m the cloths flicker frame to frame (0.5 m and 3 m of the inverted pole
above the ground alike). Best guess: the buried driver is occlusion-culled on
some frames, and a culled frame is a frame with no state. Open for the
successor: (a) find a hiding place the culling does not reject (a driver
standing upright inside a bigger mass, or measure what the culling tests), or
(b) the proper fix — the registration a pack flag's visual dyna gets (kind-0x16
handler → SInstanceParams → CHmsMgrVisDyna::InstanceCreate, unlocated in the
exe; `tm2020-tween-anim-re.md` has the reading so far).

**Pushers, phase — RESOLVED, no defect (2026-09-08 21:00Z).** The phase of a
kinematic item is the map's per-item **AnimPhaseOffset** byte — chunk
0x03043063, one byte per anchored object in eighths of the period
(`CGameCtnAnchoredObject::AnimPhaseOffset`, EPhaseOffset: 4 = Half); `tmmaps
phases MAP` prints it. Summer 15 is the only campaign map that uses it: the two
facing channel pistons carry 0 and 4 (i131/i132, and i133/i134 on the second
channel), the two Level1 rotors 4 and 2, sixty inflatable mats 2; every other
placement of every map is 0. `tmmaps tiny` keeps the original items in their
slots, so the bytes ride along unchanged, and the game honours them for an
EMBEDDED kinematic item exactly as for a stock one (lineup PH1: four stock
8mL1 and four of our half copies at bytes 0/2/4/6 in one frame extend the same
way; the ship10 tiny 15 pair runs complementary — sum of the two extensions
constant — like the original). The earlier "ours run in phase and meet
mid-channel" was a misreading of a perspective frame: the pads never meet in
either world; the free gap between them is constant (≈7.8 m original, ≈3.9 m
tiny — half, faithful) and slides across the road once per 4 s. What vjeux hit
is the SCALE POLICY, not a phase bug: the car is not halved, so a 3.9 m gap for
a 2.1 m car is a tighter timing window than the original's 7.8 m. What does
NOT set a kinematic part's phase: `SInstanceParams.Phase01` (three copies baked
with Phase01 unset / 0.25 / 0.5 move identically — the `mapgeom static-item
--phase01` knob stays as a probe) and the in-record 0x03101005 word (4 on
every placement; `tmmaps lineup --rec-word5`).

**Flag driver — VERDICT (2026-09-08 21:30Z): the hidden-driver hack cannot pass a
full-map check; the still cloth stays the default.** Lineups PH2–PH4 on the
tiny-18 host: a VISIBLE stock Flag8m drives our half cloth from any relative
place — 8 m below, 8 m above, 6 m beside, top-down or side view, every frame
(the borrowing does not depend on draw order or distance order). A HIDDEN
driver fails for reasons no hiding place cures: (1) frustum — a driver
displaced 4–8 m from our cloth leaves the view whenever the camera is within
~15 m (a chase camera looking at the car: the flags beside the road go bare
exactly where the player looks); (2) the detail bands switch at 16/64/128 m —
a displaced driver sits in the other band in a ring around each threshold
(garbage shards / a giant sail there); (3) a driver under terrain or a deck is
culled on some frames (tiny 13 flicker). The stock item cannot be shrunk
(placement scale is ignored) or made transparent (Flag8m has no skin slot).
The material-as-sidecar form (`TINY_FLAG_MATREF=bare`: the mesh names
`ItemFlag.Material.Gbx` by bare file name, copies of the two pack material
files in the archive next to the item) loads and counts as an item but draws
NOTHING — pole included. The proper form (the registration a pack flag's
visual dyna gets) is still unlocated: the anim handle is the u64 at +8 of the
params struct handed to `CHmsMgrVisDyna::InstanceCreate` (wrapper 0x1401de990,
r9; 14 call sites), the entity-kind → handler table lives in BSS (built at
run time, not readable statically). Our cloth does render its OWN half-size
mesh when driven (PH3 Ah1: half the stock's pole and cloth), so the state is
the only thing borrowed.


**The registration — LOCATED, and it is a wall (2026-09-09 06:00Z, thread
dcb0a83f; `tm2020-tween-anim-re.md` has every address).** Read from the exe
with `asmdig` (now a PE reader) and confirmed in the live game with the
GhostShooter routes `/meshflags` and `/fids` (MeshFlags.as) on one-item
lineups (frames `tinyshots/flt1..flt9`):

1. The spawner of a prefab's entities (`0x140b72d20`, the InstDyna2 branch at
   `0x140b73ae1`) tests ONE bit — `CPlugSolid2Model+0x1f0 & 1` on the entity
   model's Mesh — and only then computes the cloth's period and phase from
   the entity's `SInstanceParams` (`0x14061bdf0`; the item's seed at
   ctx+0x2c) and packs the u32 anim handle (`0x140260ea0`: 11-bit log period
   0.25..60 s, 12-bit phase, 8-bit TextureId) it hands
   `NHmsMgrInstDyna2` (`0x140262cf0`). Our embedded cloth PASSES this: bit 0
   is "some visual has ≥ 2 sub-visual frames", set by `UpdateFlags`
   (`0x140438340`) from `CPlugSolid2Model::OnNodLoaded` (vtable slot 20,
   `0x140438330` → `0x140438670`), which `CMwNod::ReadChunks` (`0x1402d0720`)
   runs at the end of every node's chunk stream — inline nodes included.
   Live: stock cloth `flags=0x3`, ours `flags=0x3`, both `frames0=86`.
2. What our cloth LACKS is **`CPlugVisual+0x24` bit 27** (the VMorph / tween
   visual mark): stock cloth visual `vflags0=0x80800a8`, ours `0x800a8`. The
   visual's device data (`0x14040496d`) builds the morph streams only for a
   visual with bit 27 (or 29); without them the VertexTween shader draws
   nothing on its own and whatever the previous tween draw left bound when a
   stock flag is in view — the "borrowing", the shards across LOD bands, the
   bare pole. Bit 27 is set in exactly one place at run time,
   `0x1404059b0`, called from `OnNodLoaded`'s first loop (and from the old
   `CPlugSolid::OnNodLoaded`), and ONLY when the geom's material, taken from
   the Solid2's per-geom array (+0xb8/+0xc0, empty at load) or else from the
   plain **`Materials` list (+0xc8/+0xd0 — `CPlugMaterial` node refs, class
   0x9079000)** through `0x14040f750` (the material's shader record for the
   current quality) has the vertex-tween property (`[record+0x84] != 0`). The
   version-29 **`CustomMaterials` list (+0xf8/+0x100, the
   `CPlugMaterialUserInst` records every embedded item uses) is never
   consulted** — it is resolved later into +0x1f8/+0x200 (live: stock
   `mats=0/2/0`, ours `mats=0/0/2`). The file cannot carry bit 27 either: the
   visual chunk readers mask the file flags (`and [+0x24],0xff8ffe50`), the
   ctor sets 0x804f0.
3. So the tween mark needs a REAL `CPlugMaterial` in the mesh's plain
   `Materials` list at load time, i.e. an external `.Material.Gbx`
   reference — which is what the pack's `FlagSmall.Mesh.Gbx` has
   (`Stadium\Media\Material\ItemFlag.Material.Gbx`, `ItemFlagNoAnim`). An
   embedded item cannot: its files are mounted at
   `<fake>\MemoryTemp\CurrentMap_EmbeddedFiles\ContentLoaded\Items\` (`/fids`),
   a Fid tree with no path to `GameData`; a reference with the pack's own
   folder chain and any ancestor level (`TINY_FLAG_MATREF=ext`,
   `TINY_REF_ANCESTOR=1|4`, the reference table now nested like the packs')
   drops the item, files carried in the archive under `Stadium/Media/Material/`
   are not found (lineup flt8: `items=7316`, the two items gone), and a
   sidecar copy next to the item (the 2026-09-08 `bare` form) loads a
   material whose own chain — textures, the parent
   `Techno3\Media\Material\Tech3_Warp_TDiffSpec_VertexTween.Material.gbx` in
   a Maniaplanet pack, the colour table — is unreachable, so nothing draws.
   The only material form an embedded item can hold (a user inst by name) is
   exactly the form the tween mark ignores.

**Verdict: a self-animating vertex-tween cloth cannot live in an embedded
item; the game reserves the tween mark for meshes whose materials are pack
files.** What remains for the Flag8m placements is a choice, not a fix: the
still half-size cloth (today's default, skinned, right size, no motion), the
stock `Flag8m` at every Flag8m placement as well (`TINY_FLAG8M=stock`, one
knob in `stock_half_variant`: moving and skinned like the Flag16m stand-ins
already are, at twice the world's scale — frames flt9: stock and half copy
side by side, two instants), or a mechanical flag built
from kinematic strips (the pusher form, which does animate embedded, one
strip item per phase eighth) — not built. The `TINY_FLAG_*` knobs stay as
the probes they are.

**How (c), a mechanical flag of kinematic strips, would be built (not built;
for a future thread).** Everything it needs is measured and in the tree:
the kinematic dyna kind (0x914F000) animates in an embedded item exactly like
a stock one (pushers, rotors, lineup PH1), `add_dyna_part` writes it (inline
mesh, BOTH hulls — a kinematic part without hulls crashes the loader at
`Trackmania.exe+0xb7088c`, so give each strip the pusher piston's hulls at
~5 %, as `TINY_FLAG_KINEMATIC` already does), the constraint is
`NPlugDyna_SKinematicConstraint` (TransAxis + TransMin/TransMax +
TransAnimFunc, RotAxis + AngleMinDeg/AngleMaxDeg + RotAnimFunc; an anim func
is a list of `{FuncBase Constant|Linear|EaseInQuad|EaseOutQuad|EaseInOutQuad,
InverseY, DurationMs}` pieces whose durations sum to the period), and the
phase of a kinematic ITEM is the map's per-placement `AnimPhaseOffset` byte
(chunk 0x03043063, eighths of the period; `MapFile::set_item_phase8`,
`tmmaps lineup --phases8`). A phase per strip is therefore a phase per ITEM:
1. Split the cloth's frame-0 mesh (the `FlagSmall` visual at the pack ladder,
   5 levels) into N vertical strips by x (6–8 strips over the 2 m half cloth,
   UVs untouched so the placement skin still maps); strip k becomes its own
   `CPlugDynaObjectModel` entity with `IsKinematic`, TransAxis = the cloth's
   normal, TransMin/Max = ∓A (A ≈ 0.12–0.15 m at half scale), anim
   `[EaseInOutQuad T/2, EaseInOutQuad T/2 InverseY]`, T ≈ 2 s (the stock
   cloth's `PeriodSc` 8 is the tween's, pick by eye).
2. One item PER STRIP, not one item with N entities: the phase byte is per
   placement, so `tmmaps tiny` emits, for every Flag8m placement, the pole
   item plus N strip items at the same pose with phase bytes k·8/N (the
   `iv@`/`--phases8` machinery). 90 placements on Summer 13 → 90 + 90·N
   items; the models are N+1 shared aliases, the map grows by the
   placements only.
3. Seams: adjacent strips differ by at most A·sin(2π/N) at any instant
   (≈ 0.1 m for N = 8) — invisible past ~20 m, visible up close; a small
   rotation about each strip's inner edge (RotAxis vertical, ±(A/strip
   width) rad, same anim func, same phase) closes most of the gap but needs
   the strip's origin on that edge. Check the result with `tinyctl motion`
   (two instants, same camera) beside a stock Flag8m before touching the
   converter.
What it will never be: cloth. It is a segmented banner whose silhouette
travels; whether that beats a still cloth or a double-size stock flag is
vjeux's call, hence unbuilt.

## Placement colours and the clip walls' materials (2026-09-08)

Chunk 0x03043062 carries one colour byte per block, baked block and item (0
Default, 1 White, 2 Green, 3 Blue, 4 Red, 5 Black). A material tints where its
`_D_HueMask` texture's alpha says so, to the entry its `ColorTargetTable`
(`Stadium\Media\ColorTargetTables\*.ColorTable.gbx.json`) names for the byte:
`TrackWall` (the whole wall — the mask is 0.95 everywhere), `TrackBorders` /
`TrackBordersOff` (the bands), `Technics`, `TrackWallClips`, `RoadTech`,
`DecalPlatform`, the plastic floor; `TechnicsTrims`, `PlatformTech`,
`PoolBorders`, `WaterBorders`, `DecoCliff` (and the `Modifier\PlatformGrass\
TrackWall` = DecoCliffPxz), the terrain skins' `TrackWallInWorld` carry no mask
and never tint. `tmmaps tiny` copies the byte of every authored block, item
AND generated filler onto its clone (`TINY_FILLER_COLOR=file`; `default` /
`owner` / `inherit` are A/B knobs — `inherit`, the rule until 2026-09-08, gave
a colourless filler its neighbours' colour). Summer 20 is all Red, 10 all
Green, 15 all Blue: the authors painted whole maps, and the editor holds the
file's byte for every filler too (`/mapblocks2` prints `color`), so a wrong
hue on a tiny wall was never a wrong byte — it was the wrong MATERIAL.

**Which block dresses its clip fillers, and with what.** A block info's
material modifier (chunk 0x0304E031 slot 1; slot 0 is the `EDClassic` parent
info it copies) is one of two files of class 0x0915D000, both `{folder, game
skin}`: `X.TerrainModifier.Gbx` = folder `Modifier\X\` + `Platform.GameSkin`
(slots PlatformTech, DecalPlatform, DecoHill, OpenTechBorders, DecoHill2,
DecoCliff, DecoCliffBase, TrackWall, DecoGrass, Deco, Penalty), and
`TrackWallToDecoCliff.Gbx` = folder `Modifier\PlatformGrass\` + a TrackWall-only
skin (the Tech-family deco blocks: DecoHill*, DecoPlatformBase, PlatformTechBase,
WaterBase, WaterWall, DecoCliff*, OpenTechRoad/Zone*). The block's OWN prefab
always wears its modifier, and so do its GENERATED CLIPS, for the materials the
modifier's folder carries: `Modifier\PlatformGrass\` and `PlatformDirt\` and
`PlatformIce\` carry a `TrackWall` (grey DecoCliffPxz concrete), so the walls
of DecoHill*, WaterBase, DecoPlatformBase, DecoCliff*, DecoWallBaseGrass,
PlatformGrassBase… are concrete; `Modifier\PlatformPlastic\` carries only
`DecalPlatform` and `PlatformTech`, so a plastic block's TrackWall panels stay
plain TrackWall in the placement colour (Summer 20's red ramp sides). The
generated pillars (`DecoWallBasePillar`, `DecoWallCurve2InPillar`,
`WaterWallPillar`… `TrackWallFromParent.Gbx`) take the block they SUPPORT: the
first non-pillar unit above the stack — any unit of a wide block, not only its
origin cell — and, where several blocks share that cell, the one whose block
info PLACES this pillar kind (`placed pillar`: PlatformPlasticCurve2In places
DecoWallCurve2InPillar, the cliffs place DecoWallBasePillar), the origin
preferred; a pillar under a plastic block stays red, whatever dressed block
also stands in its cells. A vertical clip belongs to the block ACROSS the side
it stands on (`fillers.rs`); a block that dresses nothing (plastic, a pillar
whose parent dresses nothing) does not end the search — the clip then takes
the dress of the block whose cell it stands in (the plastic U-top's
`PlatformSlope2UTopVFC` panels stand in the 4-tall, 8-long
DecoCliff8NoHillStraightSmall's cells and are its beige concrete in the
original), else — a merged Middle×N panel recorded in the bottom cell of its
span — the first block up the across column, then its own column. Every UNIT
cell of an authored block counts as its cell (`unit_cells`).

The 2026-09-08 version of this rule gated the clip dress on a `MatModifier`
PLACEMENT TAG (chunk 0x0304E023 v8: ("MatModifier", "Grass"|"Dirt")) that
DecoWallBaseGrass and the plastic family lack, on one eyes read of Summer 10's
DecoWallBaseGrass walls as "green in the original". Pixel means of 2026-09-09
(same cameras grasswallW / grasswallE / startahead; an eyes session measuring
crops) refuted it: the original's DecoWallBaseGrass panels are the folder's
concrete — beige #ded0ab in the sun, grey-green #62795d in the grass's shade,
smooth panels with seams — where ours were saturated green #276149; the
plastic family's panels are red on 20 in both worlds because its folder has
no TrackWall, not because of a tag. Same-camera frames on 20 cp3 (cpwide /
poolBsideW / poolAsideS, seven walls measured against the original: the ramp's
sides and the pillar walls under the plastic decks red, the wedges', cliffs'
and the U-top's panels beige/grey, the wall behind pool B's far-right corner
red) and on 10's start all agree with the rule above. What a wrong hue on a
tiny wall means, still: the wrong MATERIAL, never a wrong byte.

## Which generated fillers the game draws — all of the editor's list; occlusion is the occupant's mesh, not a rule (2026-09-09)

Two layers, settled separately.

**1. The file's baked list = the editor's free-clip algorithm.** Disassembling
Trackmania.exe (profiler strings: `CGameCtnChallenge::InitChallengeData_FreeClipsBaked`,
`…_Clips`, `…::CreateFreeClips`) gives the algorithm that decides, for every
authored block unit face's clip, whether a clip block exists:

```
for each B in the clips of the neighbour cell's opposite face:
    if !ClipsConnect(A, B) continue           // free & non-exclusive → always
    if IsFreeClipDeletedBy(A, B) deletedA = true
    if IsFreeClipDeletedBy(B, A) remove(B)    // an anti-clip: instantiate(B)
keep(A) = A.IsAntiClip ? deletedA : !deletedA
```

`IsFreeClipDeletedBy(a, b)`: both free; same ground bit; `a.IsAlwaysVisibleFreeClip`
→ never; `b.IsFullFreeClip && a.CanBeDeletedByFullFreeClip` → deleted; else a
Top needs a Bottom (and vice versa) with the two directions compatible under
each clip's `TopBottomMultiDir` (0 SameDir, 1 SymmetricalDirs, 2 AllDir,
3 Opposed, 4 Perpendicular, 5 Next, 6 Previous), a Side needs opposite
directions; then `Match(a, b)`: a's symmetrical group ids ∩ b's group ids if a
has any, else a's group ids ∩ b's, else `a.SymmetricalClipId == b.name`, else
the same name. A side clip block's ground bit is the RECORD's (a ground block's
upper units bake air clips); a top/bottom clip block's direction is (owner dir +
the clip's own 2-bit direction from block-unit chunk 0x0303600C's trailing
words) mod 4 (`mapgeom blockinfo` prints them as `clipdirs … 00c`).

`mapgeom bake MAP [--collection C] --diff` runs this simulation against the
file: 0 stale records on all 25 Summer 2026 maps (46 304 records confirmed).
`tiny-library` runs it on every conversion as the invariant (`engine bake
check: N confirmed / S stale / M sim-only`; `TINY_BAKE_STRICT=1` makes S > 0
fatal). This layer killed the old fitted rules that hid records the editor
keeps AND the game draws (fullfree / face / accepted / ghost / free: Summer
15's arch floor and pool-approach plate, 21's gate floor, 05's water-road
floor came back with 3201a7cc).

**2. What the runtime draws of that list: nothing is hidden by any RECORD
property we can name — a filler is invisible in the original only when the
neighbouring block's MESH covers it.** Settled 2026-09-09 (a night of probes,
recorded below so nobody repeats them). `tiny-library` emits every record; the
knobs `TINY_OCCUPIED_RULE=1|2|3` and `TINY_GHOST_CLIPS=0` build probes and are
off by default.

The evidence, all "cache-safe" (see the two traps below), 29 cameras behind the
author's car on Summer 20 plus ghost-road cameras on 12/13/24, counts of 144
cells different from the original:

| probe | what it removes | better | worse |
|---|---|---|---|
| `TINY_OCCUPIED_RULE=1` | every record in a cell another block's unit occupies (1 905 of 20's 7 287) | d196 94→30, d106 70→65, d391 74→41 | d271 12→51 (pool rims, pillar walls in WaterRampZone cells — the original shows them) |
| `TINY_OCCUPIED_RULE=2` | those whose clip has `CanBeDeletedByFullFreeClip` (1 063) | d196 →31, d106 →30 | d271 →23; d391 unchanged |
| `TINY_GHOST_CLIPS=0` | a ghost-mode block's generated clips, flag bit 28 (1 037) | d196 →38, d136 27→13, d361 78→62 | none on 20 — but on 13 the ghost road the author drives on loses an underside the original shows (u097 4→27) |
| `TINY_OCCUPIED_RULE=3` | vertical free clips (a `vertical` group) in a cell a real non-ghost unit occupies (628) | d196 →33, d106 →60, d271 →15 | d391 unchanged; d421 24→47, d361 78→88, d181 34→43 |

Every predicate that helps one camera breaks another; the one that "hides"
right everywhere is the occupant's solid volume, which no flag encodes. The
pieces involved on 20: SnowRoad*InsideVFC pillar/side walls recorded in the
SnowRoadTilt pieces' own cells (inside the snow road's bank in the original;
`SnowRoadTilt2*` has one mobil per variant — no bank to switch on); a stack of
coplanar panels in the pool-corner cells (25..27, 9..11, 20..21) —
DecoWallBaseVFC pillar walls, the ghost cliffs' DecoCliffCornerOut*VFC faces,
DecoWallSlope2StraightVFC faces — where the occupants are WaterBase, the
prefab-less ghost `DecoCliffMidCornerOut` stack (a pure clip generator, no
prefab in any variant) and the grass bowl's units.

**And none of it is a road block.** The frames that started the hunt were
cameras a player never has: behind the car UNDER the pool's water surface and
UNDER the snow deck. Shot from a chase camera and top-down in a fresh game
(14:30Z, file_ids 1598012818395110 / 1768750454254943 / 1534628425375867 /
3480098368838889), both places show a clear route in ours, identical to the
original but for the boost gates' translucent blue effect volume, which is an
editor-only visualization. vjeux's "road block" on 20 (ship13) was the same
defect as 15's: the solid water surface the route dives into (5380c805).
**Rule for every future A/B: cameras where a player's camera can be.**

TWO TRAPS that produced false "slabs" on the way (2026-09-09 night):

- **The game caches an embedded item model by FILE NAME for the whole
  session.** Two maps — or two builds of one map — embedding different pieces
  as `AC00000200.Item.Gbx` show the FIRST-loaded model in the second. ship14's
  20 loaded after ship13's 20 showed a "white Tech slab with a red LED strip
  across the grass bowl" (37–39/144 vs the original, four shoots) that was
  ship13's piece under ship14's name; the same geometry with unique names
  renders 10/144. Since 20960a1d `tinyctl build` numbers every map and build
  apart (`AC{map:02}{minute%1000:03}{idx:03}`, AI/AV too; `TINY_ALIAS_BASE`),
  which also protects a player who plays several tiny maps in one session; for
  A/Bs `tinyctl shoot --fresh` restarts the game first.
- **The editor's cursor preview.** With a block selected in the inventory its
  preview stands at the camera target — a start block carries a CAR — in the
  tiny (free cells) and not in the original (full cells). `shootctl shootset`
  switches to FreeLook after every load (`/freelook`, openplanet-plugin/Cursor.as).

Older caveats, still true: the E2 pixel probe (679 face-rule records removed
from the ORIGINAL, 203/204 views unchanged) used 110 m-high grid cameras that
cannot see a plate inside a cell — no evidence either way; a visible pool-rim
record removed from the original renders identically (regenerated, layer 1);
three records moved into an empty field show nothing (a record is not what is
drawn; the regenerated clip is).

Cache-safe confirmation at 20 cp3 (colour thread, 2026-09-09 07:00Z), the same
three cameras vjeux's "pool right of the checkpoint is missing its border" frame
came from (cpwide, plus poolBsideW / poolAsideS): the full geometry built with
its own numbering and shot as the FIRST load of a fresh game (`tinyctl shoot
--fresh`) shows pool B's coping on every edge and NO slab; the deployed ship14
file shot minutes earlier in a session that had loaded other AC000-numbered 20
builds differs from that fresh frame in 17 / 5 / 87 of 255 pixel-fractions
(>40 levels) — the slabs, the item cache. And the ship11 face rule reinstated
with a "group-less side clips (WaterFC*/WaterHFC*, hill sides) are always
drawn" clause — which alone made the rim complete where the bare face rule had
bared three of pool B's edges — renders 0 / 0 / 0 of 255 differently from the
full geometry at those cameras: the 555 records it hides are inside the
neighbours' geometry there. No hiding rule is needed for cp3; `fillers::verdict`
stays deleted. vjeux's frame was a ship10–12 build (the `fullfree` / plate rules
dropped the rim; ship13+ draws it) — the deployed ship14 has the border.

## Pool water, overflow spouts and the inflatables (2026-09-08, night)

"The white inflatable tube ends hanging over the pool wall" of Summer 15
(same-camera views fog2 / fogwide at cp2) are not items at all. Measured on
the render box with one-item lineups and item moves on a copy of the original:

* **Every `Inflatable*` item bakes, loads and draws.** The sixteen species of
  Summer 15 (mats, slopes, borders, tubes, `InflatableMat4mToTube6mCenterX2`)
  are plain `CPlugStaticObjectModel` prefabs under `ItemInflatableTube` /
  `ItemInflatableMat` / `ItemInflatableFloor` (`Tech3_Block_TDSN_CubeOut`,
  the ordinary block shader) plus `DecalMarksItems`, `TechnicsTrims` and
  `Pylon` — no dyna part, no tween, no soft body. The `AnimPhaseOffset` byte
  the author left on sixty mats moves nothing (there is no kinematic part for
  it to phase). The game keeps all 5542 placements of the tiny 15 (`/mapitems`
  total = the file), and the same-camera frames show the mat ramp out of the
  pool, the blue tube with its orange collar and the 14 rings in both worlds.
* **The white lips are the ENGINE's water overflow.** A pool block carries a
  water VOLUME (blockinfo variant chunk 0x0315B00B — `WaterBase`, `WaterWall*`,
  `RoadWater*`: id `Shallow`, one box, 32 × 1 × 32 m; `mapgeom blockinfo`
  prints them). The engine renders the volume's surface (the smooth,
  reflective water), the underwater tint, and where a volume's vertical face
  is open it pours: a falling sheet with three white foam lips at the crest —
  the "spouts" on the raised pools of 15. No prefab holds that geometry
  (`Base_Air` is one Water quad at local y 7 over a `Waterground` floor at 4;
  the rim clips `WaterFCCenter` / `WaterHFC*` are the kerb; `WaterWallVFC` is
  the `GlassWaterWall` pane). An ITEM has no water volume, so a pool baked as
  an item keeps the quad — drawn by the plain `Water` material as the busy
  cellular pattern, opaque — and loses the overflow. Not reproducible with
  items; a hand-modelled still stand-in (three lips + a sheet quad per open
  face) is the only way to put the look back, and it would be invented
  geometry.
* **`TunnelSupportArch16m` draws** (in the game's list at 1034,31,487 and
  1034,31,518 on the tiny 15; leg and beam in the same-camera frames, and
  beside its stock twin in a lineup) — an embedded item with embedded LIGHTS
  is NOT dropped (`LightCube*`, `LightCylinderQuarter2m`, the gates: all kept
  and drawn). The blue "twisted inflatable body" on the cp2 deck that only the
  original shows is `Checkpoint_Helper.Prefab` (material `Effects\Media\
  Material\EditorHelpers`, an 8 × 4 × 19 m arrow) of `GateCheckpointCenter24m`
  — an editor-only helper like the yellow FC caps: moving the gate away in a
  copy of the original removes it; play mode never shows it; the bake leaves
  it out on purpose.

## Water collision: a lid to both engines, so none (2026-09-10)

* **Physics 28 (`NotCollidable`) is a SOLID to the car** — in the dedicated
  server (player project: a nose-down drop at 25 m/s onto the 15 pool plate
  stops at the plane, four wheels on material 28) AND in the client
  (`tinyctl startcheck` on ship15's own 15 with the Spawn item moved over the
  plate: the car falls 4 m and rests at y 29.00–29.04). 5380c805's
  "Water → NotCollidable" (ship14/15) turned every water plate into a lid,
  not a hole; "the client honours the flag" was an inference from frames,
  never measured. The only place 28 is honoured is the coverage index
  (`scene::is_collidable`).
* **The original's water is a VOLUME** (above) that floats a flat car at
  0.90 m draft (`probe::WATER_DRAFT`) and lets a nose-down car through —
  Poland's author does both on the Lake (`mapgeom waterline`: ON at 38.0–38.5
  under a 39.20 plane for 1-s stretches; down to the floor at 33.8–34.6 on
  the same lake). No item can carry a volume, so an embedded plate is a lid
  (13 or 28) or nothing. **Nothing** is the choice, per body, by the
  original's observed behaviour on the route: the 19 source authors with a
  ghost never ride ON an item plate; 15's dives UNDER both of his planes
  (pool 29.0 at 14.7–15.4 s, channel 41.0 at 16.5–18.2 s); the Lake Poland
  rides is the regenerated zone — the game's own water, untouched. So since
  cdfa161b every Water-physics collision triangle LEAVES the collision mesh
  (an item with none keeps `build_surface`'s 1 mm sentinel); the visual quad
  stays. Server-verified on 15: the drop passes 29.0 and rests on the
  Concrete floor 21.51–21.55. Scope 25/25 maps, 9 650 placements (the shore
  tiles' sea/lake quads — Beach, LakeShore, WaterShore1, WaterHill,
  Sea_Land0_Land12, SeaCliff13 — coplanar with the zone water; the Stadium
  water blocks on 05/10/15/20/25). The certified 15 lap (48.7) drove ON both
  lids (15.4–18.5, 25.7–31.2 s) and does not validate on ship16.
* `mapgeom waterline TINY --ghost SRC|LAP [--anchor S:T --scale 0.5]
  --report report.tsv`: per 100 ms sample the highest Water-material triangle
  in the column — ON (at draft), LID (resting on it as a solid), UNDER, clear;
  run it on the TINY, where every plate is an item (on a SOURCE the assembler
  has no clip fillers and misses the generated plates).


## Water: the engine's native representations, and what a half-size map can use (research log, 2026-09-10 17:10Z)

The question (coordinator): can a HALF-SIZE water volume exist in our maps — a flat car floats at ~0.9 m
draft, a nose-down car passes — independent of any route? Facts so far, from the pack and the format:

* **Zone water is a BLOCK with a Water-physics quad + a unit surface tag.** BlueBay's `Sea` block info:
  one unit, `surface "Water"`, prefab `Zone\Sea\Base.Prefab` = an 8-triangle `Water` (13) quad at local +7
  over a `SeaFloor` (Sand 5) floor at +4, 32 × 32 m. GreenCoast's `Lake` is the same shape (+7.2). The
  game regenerates these from the genealogy (chunk 0x03043043) at the zone's fixed plane — 32-m cells,
  one height per collection (`tiny::fixed_plane`). Poland's author FLOATS on exactly this quad at 0.9 m
  draft and SINKS through it nose-down (`waterline`). So the fluid behaviour lives on a physics-13 surface
  of a BLOCK; whether it needs the unit's `surface "Water"` tag or a water volume, or is the material id
  alone, is the open question below.
* **Pool water blocks carry water VOLUMES** (variant chunk 0x0315B00B, `mapgeom blockinfo` prints them):
  `WaterBase` "Shallow" box (32 × 1 × 32 at +4 … +6), `DecoWallWaterBase` "Shallow" 32 × 8 × 32 and NO prefab
  (the volume is the block), `PlatformWaterBase` 32 × 2 × 32, `RoadWaterStraight` a 1 × 2 × 1 box per road
  unit; `WaterWallBase` and `WaterRampZone*` have none. The volume is what renders the surface, the underwater
  tint and the overflow lips; the DecoWallWaterFCT clip is the drawn plane of a volume block, a pack plate of
  physics 13.
* **An ITEM plate of physics 28 is a lid** to the client and the server; **an ITEM plate of physics 13 is
  nothing** to both (the table below) — ship13's "road block" on 15 was the plate's LOOK (the plain Water
  material on an item draws as an opaque cream cellular sheet), never its collision.
* **The three forms, measured (2026-09-10, client + dedicated server):**

  | form | collision of the plate | client | server | who shipped it |
  |---|---|---|---|---|
  | pack `13` (Water) | the pack's own id | falls through, no deceleration (vy −12.7 → −13.5 m/s across the plane) | falls through, rests on the floor | ship13; **ship16 default** (`TINY_WATER=pack`) |
  | `28` (NotCollidable) | 5380c805's re-flag | LID — rests at plane +0.0…0.04 | LID — nose-down 25 m/s stops at the plane | ship14, ship15 (`TINY_WATER=lid`) |
  | no triangles | removed | (crashed the load for another reason; untested) | falls through, rests on the floor | probe-15-nowater (`TINY_WATER=open`) |
  | block VOLUME | the original's | floats a flat car at 0.9 m draft, lets a nose-down car through | same | the original; no item carries one |

  Flat-car control on the 13-form (client, Spawn at rest 0.6 m above the plane): sinks to the floor, rests at
  21.51 — where the original floats at 0.9 m draft. The 13-form matches the original for a diving car, not for
  a floating one; nothing an item carries floats.

  So "NotCollidable" is the one id the car DOES collide with, and the pack's `Water` is the one it does not:
  for an embedded item the engine's water is the material id 13, treated as no surface at all — the buoyancy
  belongs to the block's volume. ship13's water was right and ship14's fix created the lids.
* **Half-size volumes:** a water volume is block-info data (per block variant, cell units), pack-side. A map
  cannot scale a block; a custom embedded block (`.Block.Gbx`, a CGameItemModel with block-info chunks) is
  the only per-map vehicle, and whether the loader reads 0x0315B00B from an embedded block is untested.
  Zone water is on the 32-m grid at one collection height: the tiny seas already use it (the anchor keeps
  the sea plane); the pools at arbitrary half-heights cannot.

## OPEN: the ring-spawn entity crashes the client (2026-09-10)

c4ce31c5 gave the block-derived ring checkpoints (GateCheckpoint on
05/12/14/15/16/21) an `NPlugTrigger_SSpawn` entity so a respawn lands on the
road under the ring. **Every map carrying it crashes the client at load**
(startcheck bisect on 15: ship16c-c4ce31c5 and ship16-60e03cbc → "game
process gone" at 12–116 s; the same recipe with `TINY_RING_SPAWN=0` PASSES;
01, no ring block, passes with every other ship16 change). Now opt-in
(`TINY_RING_SPAWN=1`); the default is ship15's trigger-only form (respawn
beside the ring). Byte comparison against the pack's
`Items\Gate\CheckpointCenter8mV2.Prefab` (entities 3 = 0x0917A000, 4 =
0x0917B000): the SSpawn node (v3, identity Iso4, 24 bytes 0,0,0,0,−1.0,0,
FACADE), the entity records (rot, pos, params −1, empty u01) and the layout
are identical; ONE byte-level difference — the 0x0917B000 companion's 8-byte
body is `(0, 11)` in the pack and was `(0, 0)` in ours (now copied; a 15 with
`TINY_RING_SPAWN=1` is built at /tmp/tiny15/ring11 for the startcheck, NOT
run yet). The other structural difference: our spawn transform is the ENTITY
pos (8, 0.85, 5.6) while the pack's entity sits at 0 with an identity Iso4
(its spawn IS the item origin) — if the body word was not it, try the pack's
form (entity at 0, the Iso4 carrying the translation).

## The SHAPE of a generated filler is never in question (2026-09-08, night)

vjeux, on Summer 15's reactor gate: "the ones that are displayed are NOT
correctly shaped pieces — we display entire basic road blocks when it should be
a wall or something." Checked from the other end with `mapgeom shape-audit
MAP… [--collection C] [--game baked.json] [--out TSV] [--all]`: for every
BAKED record the converter's resolution — the block-info file the name means
(`BlockInfoIndex::paths_for`, Classic before Pillar before Clip), the variant the
ground bit picks, the mobil LIST bits 0..5 index, the mobil bits 6..11 index,
the additional variant of bits 21..27, the prefab that comes out — is printed
with every fallback the pick took. On the 25 Summer 2026 sources: 0 names with
two candidate files of different kinds, 0 records indexing past their variant's
lists or past a list, 0 names without a block info; the one fallback is
`WaterShore1_Rocky_FCLeft` (76 air records on 08/13/23) on a clip info that has
only a ground variant — the same `FCLeft` prefab either way. Against the
editor's own list of the original 20 (`/mapblocks2?list=baked`: 7287 records
with the engine's `MobilIndex` / `MobilVariantIndex`), 7280 of 7281 clip
records pick exactly what the file's bits say; the one exception is
`DecoCliffToDecoPlatformStraightVFC` b5356, where the engine took mobil variant
3 (`…_Bottom_AirD`) for the file's 2 (`…_Bottom_AirC`) — the A/B/C/D
alternates of one piece.

So the "whole road / platform blocks" are the prefabs the records name:
`TrackWall\Straight_FCB` is a 29 × 32 m ResonantMetal plate (a road's
underside), `Platform\Base_FCT` / `Base_FCB` are 32 × 32 m plates (a pillar's
cap, a platform's underside), `DecoWallWater\Base_FCT` is a 32 × 32 m WATER
plane — the water surface of a `DecoWallWaterBase`, which has none of its own
(drop it and the channel is dry, frame s15ab-o). What vjeux saw at the gate
was the PlatformBase plates of the channel's pillar caps seen through the
water (the ResonantMetal grating reads as parallel stripes on the pool;
frames s15shape/s15ab/s15pb, 2026-09-09 00:00–00:30Z): a DRAW-rule matter
(the plate clause of 3bab4b43 hides them), not a resolution one. The
converter cannot draw a wrong shape for a record; it can only draw a record
the game would not.
