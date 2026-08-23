# Where a TM2020 map's geometry actually lives

*Written 2026-08-22, from a working extraction. Everything marked **MEASURED**
was executed and checked against data. Everything else is marked.*

Until now every geometry result in this project was *inferred*: a deck height
from 35 plumb probes, a route from the block graph, "the ice IS the road" from
deleting caps and watching what broke. This is the direct read. The shapes the
game collides against come out of the game's own data, at the map's own
coordinates, and a run's trajectory can be laid over them.

```
tools/mapgeom            the crate

# the whole map as glTF, with a run drawn through it, plus a picture
mapgeom map M.Map.Gbx --out m.glb --png m.png --ghost G.Ghost.Gbx

# fit the map height and GRADE the model: how far above the surface the
# car sat, and what the surface was
mapgeom check M.Map.Gbx --ghost G.Ghost.Gbx

# every surface in one vertical column -- a plumb probe, without the engine
mapgeom plumb M.Map.Gbx --at X,Z --yoff N

# what a map carries inside itself, and what one of those models is
mapgeom items M.Map.Gbx --out ./emb
mapgeom dump  ./emb/3-1-1-5-Ice-Light.Block.Gbx
```

---

## 1. The four places geometry lives

| where | what is in it | who reads it |
|---|---|---|
| `dedicated_TMStadium.pak` → `.EDClassic.Gbx` → `.Prefab.Gbx` → `CPlugSurface` | every stock block's **collision** triangles, with a physics material per face | `store` + `geom` |
| the same pack, `Stadium\GameCtnDecoration\Map\Deco*.Map.Gbx` | the **stadium**: stands, canopy, outer walls, the grass floor | `assemble::decoration` |
| the map's own chunk `0x03043054` | a ZIP of the **custom items and blocks** the author embedded | `embedded` |
| a custom model's `CPlugCrystal` / `CPlugSolid2Model` | the authored **mesh**, with a material that carries a physics id | `classes` |

The first three are all needed. Leave out the decoration and 173691 has nothing
where its car comes to rest. Leave out the embedded models and 134672 — whose
track is a custom ice ribbon — has 5 % of its run over any surface at all.

### What is NOT there

**The dedicated server's pack ships collision, not appearance.** Every stock
road block's `CPlugStaticObjectModel` reports `mesh = -1` and a `shape`: there
is no `CPlugSolid2Model` behind a Nadeo block in this pack. That is a fact about
the *dedicated server's* data, not about the reader — the same reader pulls 80
visual triangles out of a map-embedded palm tree and 13 628 out of an embedded
ice block (§5). A rendering-grade model of the stock blocks needs the *client's*
packs, which are not on this box.

For the question this project asks, the collision surface is the better one
anyway: it is what the car is actually on.

---

## 2. The pack, and the name hash that blocked this before

`dedicated_TMStadium.pak` is a NadeoPak v18; the container, the Blowfish
variant and the dictionary-seeded LZ4 are described in
`~/persistent/private-30d/tm-loop/GEOMETRY.md` and ported here unchanged. Key:
`870FBE770EE4909C714B18B04D914C17` (a property of the pack file, so it keeps
working until Nadeo re-packs).

The part that was open is name resolution. 11 621 `CPlugPrefab` files — 369 MB,
the bulk of the pack and all of the block geometry — are stored under 34 hex
characters, e.g. `Stadium\Media\Prefab\42F328BF947AC905A1D4FECB9A40E4C6F7`. The
previous attempt concluded the naming scheme was unresolved.

**It is `MD5.Compute136`, and it has two details that each look like a wrong
answer** — MEASURED, every prefab in the pack now resolves by name:

```
h[0]    = the BYTE LENGTH of the lowercased UTF-8 path      <- not 0x00
h[1..]  = md5 of those bytes
hex     = each byte written LOW NIBBLE FIRST                <- not normal hex
```

The earlier attempt used an older `Compute136` whose 136-bit output is
`0x00 ++ md5`, so every hash it computed began `00` and no pack entry does. The
length byte is exactly why real names begin with wildly different pairs. The
nibble swap then turns a correct MD5 into a string that *looks* like a plausible
wrong hash.

What is hashed is a **suffix** of the path, and the entry lives under the
remaining prefix, so resolution walks the split points:

```
Stadium\Media\Prefab\RoadDirt\TiltCurve3_Air.Prefab.Gbx
  -> Stadium\Media\Prefab\22B536CA1A02655C4FE83309400717B4EF
```

`names.rs`, four candidates per lookup, exact.

**Where the names come from at all**: a block model's GBX **reference table**.
`RoadDirtTiltCurve3.EDClassic.Gbx` names `..\..\Media\Prefab\RoadDirt\
TiltCurve3_Air.Prefab.Gbx` in plain text. The ref table's folder index is
**1-based with 0 = the ancestor directory**; getting that off by one puts every
referenced file in a sibling folder, which resolves to nothing and reads exactly
like "the pack does not have it" (it cost an hour here).

---

## 3. Block → triangles

```
CGameCtnBlockInfoClassic (.EDClassic.Gbx)
  reference table names its .Prefab.Gbx files
    CPlugPrefab            entities, each with a quaternion and a position
      CPlugStaticObjectModel     mesh (visual) + shape (collision)
        CPlugSurface             TRIANGLES + EPlugSurfacePhysicsId per face
        CPlugSolid2Model         visual mesh, via CPlugVisual + CPlugVertexStream
```

A prefab tree is deep and crosses files: `RoadDirtTiltCurve3` is two prefabs,
one of which holds 51 entities of its own, and the walk touches 85 files to
produce 24 064 triangles for one 3×3 block.

**Variant selection is not implemented.** A block info names the prefabs of
every variant (ground, air, and the numbered extras) and this takes the union.
That is more than any single placement shows, and it is safe for the question
the model answers — the variants differ in what is *under* the road, not in
where the road is — but it is the first thing to fix. Doing it properly means
walking `0x0304E023 / 0x0304E027 / 0x0304E02C` → `CGameCtnBlockInfoVariant`
`0x0315B005` → `CGameCtnBlockInfoMobil` `0x03122003`, which is a dozen more
chunk readers.

### Placement — MEASURED

```
world = (32*cx + lx,  8*cy + yoff + y,  32*cz + lz)

dir=0: (lx,lz) = (x,      z)          dir=2: (SX - x, SZ - z)
dir=1: (lx,lz) = (SZ - z, x)          dir=3: (z,      SX - x)
```

clockwise looking down, with `SX × SZ` the block's footprint in whole 32 m
cells. The footprint is read off the model's own geometry with a 15 % overhang
tolerance rather than from its unit list — good enough on everything measured,
and the honest place to look first if a map ever comes out visibly wrong.

`yoff` is per map. It is **fitted**, not supplied: `mapgeom check` sweeps whole
8 m rows and keeps the one where the most samples of a run share a ride height
(§4). On map 2 it lands on **−120**, which is the value the earlier arm
calibrated by hand — an independent confirmation of both.

---

## 4. Does the car sit on it? — the grading, MEASURED

A model that looks plausible and puts the car three metres under the track is
worse than no model, and a rendering cannot tell you which one you have. So
`mapgeom check` drops a plumb line from every ghost sample, takes the highest
triangle below it, and reports the distribution of `sample.y − surface.y` plus
the physics material the car was over. It also *fits the map's height* from the
same measurement (§4.2).

### 4.1 Thirty maps

Every map in `tm-unbeaten` with a map file and a ghost — 33 maps, one rank-1
ghost each, no cherry-picking, run in one batch. Full output is banked at
`~/persistent/private-30d/tm-mapgeom/corpus-check/`.

**Thirty-one of the thirty-three fitted a height at all. Twenty of those put
the car within a quarter of a metre of the model, and sixteen within 0.09 m:**

| gap median | n | maps |
|---|---|---|
| 0.009 - 0.086 m | 16 | 197047, 252289, 208024, 134672, 279197, 285268, 203330, 279209, 199100, 153527, 270051, 227969, 126859, 267460, 284238, 270053 |
| 0.13 - 0.24 m | 4 | 146612, 227654, 286279, 203072 |
| 0.34 - 0.62 m | 4 | 238835, 274191, 267859, 145875 |
| 1.0 - 3.4 m | 7 | 285885, 210218, 279218, 228811, 228607, 191465, 249521 |
| no height fits at all | 2 | 173636, 186935 |

A run over its own road reads like this — 153527, 85 811 samples:

```
  yoff -64  (2 152 875 triangles indexed)
  54 748/85 811 samples over a surface (63.8 %)
  gap below the car   median 0.044 m   p10 0.013   p90 0.206   +/-0.020 m
  driven over         Asphalt 72%, Grass 22%, ...
```

Four centimetres, held to ±0.020 m over fifty thousand samples, is the model
and the run agreeing to the width of a tyre.

**The large-gap maps are a COVERAGE result, not a placement error.** Their
windows are loose (±0.3 to ±1.4 m, against ±0.02 for the tight ones) and their
"driven over" is Grass — the stadium floor. That is the probe reporting that
the model has nothing where the car was and it found the ground instead. The
next thing to build is named by that list, not guessed at.

Two independent confirmations that the frame itself is right:

* **map 2 fits `yoff = −120`**, the value the earlier arm calibrated by hand.
* **134672's custom ice ribbon**: `RoadIce 78 %` under the car at 0.030 m
  (§5) — the model reproduces a surface that exists only inside that map file.

### 4.2 Fitting the map height, and two criteria that were confidently wrong

`world_y = 8*cy + yoff`, and `yoff` is per map. It is fitted here rather than
supplied, and the *criterion* is the whole game — both wrong ones produce a
tight, confident, wrong answer:

1. **"How many samples have anything under them."** Grass is everywhere, so
   this picks whichever height drops the run onto the stadium floor. On 134672
   it scored 93 % of samples over a surface at a wandering 3.2 m.
2. **"The largest group of samples sharing a gap."** Better — it demands a
   consistent ride height — but it is *degenerate under a vertical shift*:
   lower the model by a metre and every gap rises by a metre, so the same
   samples still share one. 134672 then fitted −65 and reported 1.021 m
   instead of −64 and 0.030 m. It also picked the wrong cell row where a deck
   sits under the road: 146612 fitted −16 and reported a rock-steady
   **2.048 m ± 0.022**, which is a consistent wrong answer and looked exactly
   like a real finding for about an hour.

The criterion that works is anchored at zero: **how many samples are RESTING —
within 0.25 m of a surface.** A car rests centimetres above what it is on.

The sweep is two passes, whole 8 m cell rows and then metre by metre, because
nothing guarantees a map's height is a whole number of cells — and 252289 fits
−60 with a 0.017 m gap over 100 % of its samples, so it is not.

### 4.3 The independent cross-check — 173691's canopy deck

Banked separately, from 35 plumb probes driven in the engine: the car comes to
rest on the canopy at **(1521.0, 114.16, 588.4)**.

```
mapgeom plumb map173691.Map.Gbx --at 1521.0,588.4 --yoff -64
  y    228.800   Metal        <- roof trusses
  ...
  y    115.313   Asphalt
  y    113.540   Asphalt      <- the deck
```

The model puts a deck at **113.54 m** under that point, out of a column
spanning 115 m — and only once the decoration map is loaded; without it that
column holds nothing below 180 m. The car rests 0.62 m above it, against the
0.45 m that map's road gives, so the two agree to about 0.2 m.


---

## 5. Custom items and blocks — the positive control

A map's own models come out of chunk `0x03043054`: a length-prefixed block
holding `filesMeta`, a ZIP, and a texture list. 134672 carries 17 files.

The map spells an embedded block `FlinkIceBlocks\3-1-1-1-Ice-Light.Block.Gbx_CustomBlock`;
the zip holds `Items/FlinkIceBlocks/3-1-1-1-Ice-Light.Block.Gbx`.

Two model formats turn up, and **both produce triangles** — which is what makes
"the stock pack has no visual meshes" a statement about the pack:

| file | class chain | result |
|---|---|---|
| `3-1-1-5-Ice-Light.Block.Gbx` | `CGameBlockItem` → `CPlugCrystal` | 13 628 triangles, 11 946 of them `RoadIce` |
| `coconut.Item.gbx` | `CGameCommonItemEntityModel` → `CPlugStaticObjectModel` → `CPlugSolid2Model` → `CPlugVertexStream` | 80 triangles, 54 vertices, `Concrete`, bounds 2.3 × 2.4 × 2.4 m |

`CPlugCrystal` is the editor's editable mesh: vertices, n-gon faces, a material
index per face. `CPlugMaterialUserInst` carries `surfacePhysicId`, the same
enum a stock block's collision triangles carry — so a custom mesh is coloured by
what the car *feels*, and 134672's custom ribbon reads as `RoadIce` beside a
stock one. Where the material only LINKS a Nadeo material it has no id of its
own, and the link's own name is used (`Stadium\Media\Material\RoadIce`).

---

## 6. Traps

* **An unknown non-skippable chunk is fatal, and must be.** A GBX body is a
  graph: the only way to reach node 40 is to have parsed nodes 1..39
  byte-exactly. A chunk whose length is not written down cannot be stepped
  over, and guessing desynchronises the walk into somebody else's floats —
  which still produce numbers. `mapgeom` names the class and the chunk and
  stops. `MAPGEOM_TRACE=1` prints every step of the walk, which is how a new
  chunk reader gets written in minutes rather than by bisection.
* **`GbxLoc` is 28 bytes** (a position and a quaternion), not the 48 of an
  `Iso4`. Reading it as an `Iso4` inside a prefab's placement-group parameters
  swallows the rest of the file, and the failure surfaces as *"this prefab ends
  early"* one entity later. 131 prefabs failed this way.
* **`CGameItemModel`'s waypoint chunk `0x2E00201F` is at version 13** in this
  pack; the community spec documents up to 12, and 13 drops the
  `scriptWithSettings` node reference. Read off the bytes and checked against
  every item in the pack.
* **A `Float3` vertex declaration is packed to `Dec3N` only in LOCAL 3D space.**
  Positions are Global3D and stay full floats. Applying the packing to a
  position reads a mesh's coordinates out of the wrong bytes and produces a
  plausible-looking small mesh.
* **The decoration `.Map.Gbx` files are ENCRYPTED in the pack** — nearly
  everything else carries ForceNoCrypt. The same pack key opens them.
* **Terrain blocks are not in the Stadium pack under their map names.**
  `Water`, `Grass`, `Bush`, `DirtCliff*` and friends have no
  `GameCtnBlockInfoClassic` entry; on map 2 that is 4 274 placements of 39
  models. They are terrain/decoration and are simply absent from the model.
  `Water` matters for 284238 and 210218 and is unfinished work.

---

## 7. What is still missing

1. **Eleven maps of thirty-three where the run does not sit on the model**
   (§4.1), and two where no height fits at all. Their windows are loose and their
   "driven over" is Grass, so the model is missing the surface those runs are
   on. That list is the next thing to work through, in order.
2. **Block variant selection** (§3). The union of all variants is drawn.
3. **Terrain blocks** — `Water` above all: on map 2 that is 2 568 placements.
4. **The vertex stream's layout after the Position array.** On a 54-vertex
   stream the normals start 4 bytes later than the declarations predict. The
   reader takes the positions, then scans to the node terminator and *reports*
   the recovery rather than guessing; normals and UVs are not read. Triangles
   and materials are unaffected.
5. **Stock visual meshes**, which need the game client's packs (§1).
6. **The footprint heuristic** (§3) should be the block's unit list.
7. **`yoff` from the decoration** rather than from a fit. The decoration names
   a `CGameCtnDecorationSize` (`0x0303B000`) which almost certainly carries the
   base height; its one chunk has no reader yet. That would remove the need for
   a ghost to build a model at all.
