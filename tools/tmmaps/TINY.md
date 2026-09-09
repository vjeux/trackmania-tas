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
  `TDSN` for bark AND leaves, the pack's diffuse (DXT5 with alpha, mips cut to
  256 px a side) in user-texture slot 0, riding next to the items as
  `Items/<image>.dds`. Three facts cost a probe each: a visual without a
  TexCoord1 set is NOT DRAWN under any valid material (the pack leaf visuals
  carry uv0 alone → `ensure_texcoord1`); TDSN alpha-tests a diffuse that has
  an alpha channel while TDOSN/TDOBSN/TIAdd draw such cards invisible and
  TDOSN2Sided is not a model the item loader knows (red); custom materials of
  one NAME are one material to the game (thirteen palms whose `PalmTree_Leaf`
  differed only by model crashed the client at 0x140456513), so the name
  spells the model and the image (`TDSN_ItemPalmTreeBranch_D`).
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
* **Known cosmetic limit — dynamic lights**: the item shading models answer
  local lights (stadium ShowLights rigs, the checkpoint gates' embedded
  lights) with a yellow-white glare the game's vegetation shader never shows;
  bushes standing inside a gate's beams (tiny 24, cp8) look like crumpled
  paper while the tall species escape because their crowns sit above the
  beams. Proven both ways — lights moved out of the box: the same bush goes
  from mean RGB (166,175,100) with 12.5 % blown pixels to (69,77,56) and 0 %;
  at source scale the glare is identical, so it is not a half-radius
  intensity artefact. Nothing material-side moves it (alpha clamps, constant
  textures in every slot, normal maps, vertex colours, back-face variants,
  every other model). Decision 2026-09-08: ship as is; the light itself is
  what is off (the road under those gates is too white in the tiny as well)
  and goes to a lights follow-up, not a tree workaround.

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
always wears its modifier. Its GENERATED CLIPS wear it only when the block also
carries a `MatModifier` PLACEMENT TAG (`MatModifierPlacementTag`, chunk
0x0304E023 v8: ("MatModifier", "Grass"|"Dirt"|…)): DecoHill*, WaterBase,
WaterWall, DecoPlatformBase, DecoCliff*, OpenTechRoad/Zone* carry `Grass`,
OpenDirtRoad/Zone*, DecoHillDirt*, WaterWallDirt, DecoPlatformDirtBase carry
`Dirt`; DecoWallBaseGrass, DecoWallLoopEndGrass, PlatformGrassBase, every
PlatformPlastic* and the plastic checkpoints carry NONE. The generated pillars
(`DecoWallBasePillar`, `WaterWallPillar`… `TrackWallFromParent.Gbx`) take the
block above them (the pillar rule), tag-gated the same way. A vertical clip
belongs to the block ACROSS the side it stands on (`fillers.rs`), else to its
own cell's block, else — a merged Middle×N panel recorded in the bottom cell
of its span — to the first block up the across column, then its own column.

Same-camera frames of the originals (`tinyctl shoot` own10 / ownf20 / col20 /
dc / tg, 2026-09-08): a `DecoWallBaseGrass`'s panel and a pillar's under a
`PlatformPlasticSlope2LoopStart` are GREEN-tinted TrackWall on 10 (the folder's
grey TrackWall had been baked there); a `PlatformPlasticTiltTransition2
DownRight`'s panel is red on 20 and an authored `TrackWallStraightPillar` red
in both worlds; every panel of a `DecoHill*` side, a `WaterBase` pool, a
pillar under a `DecoPlatformBase` is the grey DecoCliff concrete — 20 cp3's
tall wall and hill sides, 10's start pillar and pool walls, 15's pool wall,
which the tiny had painted red / green / blue as plain TrackWall
(`terrain_modifier_base` skipped the TrackWallToDecoCliff ref as "a game
skin", and every filler inherited a modifier tag or no tag). `modifier_links`
resolves the ref to its one link; `terrain_mods` gates the inheritance on the
tag; `inherited_mod` walks across → own → up the columns.

## Which generated fillers the game draws — ALL OF THEM (2026-09-09, from the exe)

Settled by disassembling Trackmania.exe (the profiler strings name the load
pipeline: `CGameCtnChallenge::InitChallengeData_FreeClipsBaked`, `…_Clips`,
`…::CreateFreeClips`). At map load the client does NOT draw the file's
BakedBlocks. `InitChallengeData_Clips` walks every authored block unit (Flat /
Frontier terrain and clip blocks excepted), every face, every clip with
`ClipType != 0`, allocates a brand-new clip block for it (the file's record for
that owner face only donates attributes: variant alternate, lightmap id,
colour) and decides against the neighbour's clips whether it is instantiated:

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

The editor bakes with the same code, so **the file's baked records are exactly
what the game draws**. `mapgeom bake MAP [--collection C] --diff` runs the
simulation and reports it against the file: 0 stale records on all 25 Summer
2026 maps (46 304 records confirmed). `tiny-library` runs the same check on
every conversion (`engine bake check: N confirmed / S stale / M sim-only`;
`TINY_BAKE_STRICT=1` makes S > 0 fatal). Pixel proofs on Summer 20: the
original minus a VISIBLE pool-rim record renders identically (the rim is
regenerated); three records moved into an empty field show nothing there; the
original minus the 679 records the old face rule hid is identical over 204
cameras.

Consequence: every hiding rule this file used to describe (fullfree, face,
covered, occupied, the plate clause) hid pieces the game draws, and they are
gone (3201a7cc). Where an original does not SHOW a record's piece, neighbouring
geometry occludes it (a hill, the engine's volume water, a grass tile); those
are shape/terrain differences of our conversion, to be fixed at the geometry
level — never by leaving a record out. `mapgeom fillers MAP` remains as the
per-record listing (cell occupants, facing clip lists, owner).

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
