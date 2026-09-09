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

## Which generated fillers the game draws — the editor's list; what the runtime skips is open (2026-09-09)

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

**2. What the runtime draws of that list — no hiding rule is established; the
"slabs" were the game's ITEM CACHE (2026-09-09 night).** vjeux on ship13: "15,
20 — road blocks in the middle of the path". 15's was physics (water surfaces
solid, 5380c805). For 20, drive-through cameras behind a run line showed, in
ours only, a "grey road slab over cp3", a "white Tech slab with a red LED strip
and the TM logo across the grass bowl", a "dark deck underside at the pool". A
probe that removed every record standing in a cell occupied by another block's
unit (`mapgeom fillers` class `covered:*`, 1 905 of 20's 7 287) made those frames
match the original, and an occupied-cell rule was nearly shipped. It was
wrong. The same full geometry rebuilt with UNIQUE item names (`TINY_ALIAS_BASE`)
renders the bowl camera at 10/144 cells from the original — the slab was never
in our file: **the game caches an embedded item model by its file name for the
whole session**, so ship14's 20, loaded after ship13's 20 in one editor session,
showed ship13's pieces wherever the two libraries' `AC000xxxxx` numbering had
drifted apart. Every A/B of this campaign that loaded two builds with colliding
names in one game session is suspect; the probe's "clean" frames were clean
because its numbering happened to differ. Since ba36675c+ `tinyctl build` gives
every map and every build its own numbering (`AC{map:02}{minute%1000:03}{idx:03}`,
same for AI/AV) — which also protects a player who plays several tiny maps in
one session. What remains open for 20: one camera at the pool (~39 s) where the
full geometry and the probe still differ (75 vs 43 cells, both cache-safe);
whatever that is will be named by eye, not by a rule.

The editor's cursor was a second red herring on the way: with a block selected
in the inventory its preview (a start block carries a car) stands at the camera
target; `shootctl shootset` now switches to FreeLook after every load
(`/freelook`, openplanet-plugin/Cursor.as). `TINY_OCCUPIED_RULE=1` still builds
the occupied-cell probe; it is not a rule and off by default.

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

Caveats worth keeping: the E2 pixel probe (679 face-rule records removed from
the ORIGINAL, 203/204 views unchanged) used 110 m-high grid cameras that cannot
see a plate inside a cell — it is not evidence about occupied cells either way.
Two probes stand: a visible pool-rim record removed from the original renders
identically (regenerated, layer 1); three records moved into an empty field
show nothing (a record is not what is drawn; the regenerated clip is).

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
