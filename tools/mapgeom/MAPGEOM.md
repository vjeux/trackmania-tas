# Where a TM2020 map's geometry actually lives

*Written 2026-08-22 from a working extraction; revised the same day after the
first corpus run turned out to be measuring a model with half the track in the
wrong place. Everything marked **MEASURED** was executed and checked against
data. Everything else is marked.*

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
# car sat, what the surface was, and what is MISSING
mapgeom check M.Map.Gbx --ghost G.Ghost.Gbx

# every stretch of a run the model has nothing under, and how far the nearest
# triangle is -- absent, or merely too narrow
mapgeom holes M.Map.Gbx --ghost G.Ghost.Gbx --yoff -64

# every record the map places near a point, and where each one landed
mapgeom where M.Map.Gbx --at 777.8,328.9 --yoff -64

# the whole corpus in one batch, and the before/after table
mapgeom corpus --root ~/persistent/private-30d/tm-unbeaten --out ./run --jobs 12
mapgeom compare --before ./corpus-check --after ./run

# every surface in one vertical column -- a plumb probe, without the engine
mapgeom plumb M.Map.Gbx --at X,Z --yoff N

# what a map carries inside itself, and what one of those models is
mapgeom items M.Map.Gbx --out ./emb
mapgeom dump  ./emb/3-1-1-5-Ice-Light.Block.Gbx [--body raw.bin]
```

---

## 0. What changed, and why the first numbers were wrong

The first version of this document reported that 31 of 33 maps fitted a height,
that 20 of them put the car within a quarter of a metre of the model, and that
the rest were a **coverage** problem — the model missing surface. Two of those
three claims were wrong, and the third was right for the wrong reason.

**The largest single defect was in twelve lines of placement code.** A block
turned a quarter or three quarters of the way round has to be shifted back onto
its own cells after the rotation, and the shift was paired with the wrong
rotation. `dir = 0` and `dir = 2` blocks were exactly right; **every `dir = 1`
and `dir = 3` block was placed one whole footprint away.** That is invisible to
a height fit — the misplaced blocks are at the correct HEIGHT, which is all the
fit scores — and it is invisible in a top-down picture unless you already know
what the track is supposed to look like. It took roughly a third of every run
off the model.

Four more defects, each found by asking the same question — *what does the map
say is here, and did that model produce any triangles?* — are in §3 and §5, and
two more physical facts a plumb probe cannot see are in §4.3 and §4.4.

**And the metric itself was measuring the wrong thing.** "Samples with a
surface beneath them" counts a car twelve metres in the air as a hole in the
model. §4 replaces it.

**Across the corpus, the median map went from 47.1 % of its run over a surface
to 79.8 %, and twenty of the thirty-three now sit within 0.05 m of the model
against sixteen within 0.09 m before.** The full before/after table is banked
beside the transcripts as `compare-20260822.md`.

One sentence to keep, because it is the ethic the rest of this document is
made of: **a measured effect of nothing is worth knowing, and it is not the
same as not having checked.**

---

## 1. The four places geometry lives

| where | what is in it | who reads it |
|---|---|---|
| `dedicated_TMStadium.pak` → `.EDClassic.Gbx` → `.Prefab.Gbx` → `CPlugSurface` | every stock block's **collision** triangles, with a physics material per face | `store` + `geom` |
| the same pack, `Stadium\GameCtnDecoration\Map\Deco*.Map.Gbx` | the **stadium**: stands, canopy, outer walls, the grass floor | `assemble::decoration` |
| the map's own chunk `0x03043054` | a ZIP of the **custom items and blocks** the author embedded | `embedded` |
| a custom model's `CPlugCrystal` / `CPlugSolid2Model` | the authored **mesh**, with a material that carries a physics id | `classes` |

All four are needed. Leave out the decoration and 173691 has nothing where its
car comes to rest. Leave out the embedded models and 134672 — whose track is a
custom ice ribbon — has 5 % of its run over any surface at all, and 197047 has
2.6 %.

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

## 3. Block → triangles, and where a block model actually lives

```
CGameCtnBlockInfoClassic (.EDClassic.Gbx)
  reference table names its .Prefab.Gbx files
    CPlugPrefab            entities, each with a quaternion and a position
      CPlugStaticObjectModel     mesh (visual) + shape (collision)
        CPlugSurface             TRIANGLES + EPlugSurfaceMaterialId per face
        CPlugSolid2Model         visual mesh, via CPlugVisual + CPlugVertexStream
```

A prefab tree is deep and crosses files: `RoadDirtTiltCurve3` is two prefabs,
one of which holds 51 entities of its own, and the walk touches 85 files to
produce 24 064 triangles for one 3×3 block.

### A block name resolves through FIVE families, not one — MEASURED

Looking only in `GameCtnBlockInfoClassic` left **125 537 placements across the
33-map corpus with no geometry at all**, 92 619 of them `DecoWallBasePillar`.
The pack has five block-info folders and the extension changes with the folder:

| family | extension | what is in it |
|---|---|---|
| `GameCtnBlockInfoClassic` | `.EDClassic.Gbx` | roads, walls, platforms |
| `GameCtnBlockInfoPillar` | `.EDClassic.Gbx` | the supports under everything (103 Stadium entries) |
| `GameCtnBlockInfoFlat` | `.EDFlat.Gbx` | the terrain sheet — Stadium has `Grass`; the other environments add `Water`, `Lake`, `Sea`, `Dirt`, `Land` |
| `GameCtnBlockInfoFrontier` | `.EDFrontier.Gbx` | cliffs and hills |
| `GameCtnBlockInfoTransition` | `.EDTransition.Gbx` | the joins between them |

Classic and Pillar each carry a `Theme\` subfolder holding 122 more models —
`SnowRoadStraight`, `RallyCastleRoadStraight` — which are otherwise invisible.

**A map's block list is also not only blocks.** The gates, the rotors and the
seasonal props are ITEMS placed on the grid, so a name that resolves as neither
a block nor an embedded model gets one more lookup as an item.

On 134672 this took "342 placements of 14 models had no geometry" to 82.

### Placement — MEASURED, and it was wrong

```
world = (32*cx + lx,  8*cy + yoff + y,  32*cz + lz)

dir=0: (lx,lz) = (x,      z)          dir=2: (SX - x, SZ - z)
dir=1: (lx,lz) = (SZ - z, x)          dir=3: (z,      SX - x)
```

clockwise looking down, with `SX × SZ` the block's footprint in whole 32 m
cells. **The code did not implement this.** The shift that puts a turned block
back on its own cells was paired with the opposite quarter turn, so `dir = 1`
and `dir = 3` landed one footprint away — and `dir = 0` and `dir = 2`, which
cannot tell the two pairings apart, stayed exactly right.

Measured, on 134672, all three pairings at the same fitted height:

| pairing | samples over a surface | median ride height | p90 |
|---|---|---|---|
| the shipped one | 55.8 % | 0.030 m | 2.521 m |
| the other handedness | 76.9 % | 0.033 m | 1.044 m |
| **the one above** | **87.1 %** | **0.029 m** | **0.287 m** |

and 252289, which was already at 100 %, is unchanged by all three. That is what
makes it a measurement rather than a preference: **the accuracy does not move,
the coverage triples, and the map that had nothing to gain gains nothing.**

**A placed ITEM is positioned by its PIVOT, and the pivot belongs to the
PLACEMENT.** The map's `CGameCtnAnchoredObject` record carries it, along with
the item's pitch and roll and its scale, and it has to be the placement's
rather than the model's because an item can declare **several** — the tube
`InflatableTubeCurve4` declares two, 28 m apart, and Cobalt Cove uses a
different one at each of the three placements around a single corner. The
field is the vector from the pivot to the mesh origin, i.e. minus the model's
own pivot, exactly: 197047's platform declares `(4, 0, 4)` and every placement
of it records `(-4, 0, -4)`. Placing by the mesh origin instead put 197047's
whole 100 s run 1.5 m from its road and cost that map 97 % of its samples.

A GRID block is placed by its cell. A FREE block — one the author dragged off
the grid — is placed by an absolute position and carries no pivot.

`yoff` is per map. It is **fitted**, not supplied (§4.2). On map 2 it lands on
**−120**, the value the earlier arm calibrated by hand.

### Variant selection is still not implemented

A block info names the prefabs of every variant and this takes the union. It is
the first thing left to fix; see §7.

---

## 4. Does the car sit on it? — the grading, MEASURED

### 4.1 The question the first metric could not answer

`check` used to report *how many ghost samples have a surface within reach
beneath them*, and treat the rest as holes in the model. On a map driven with
big air that is simply false: a car twelve metres above a road the model DOES
have is not a coverage failure, and a car on the inside of a loop has its road
beside it rather than under it.

Two things fix that and **neither of them is the model**:

* **Ask the recording whether the car was touching anything.**
  `is_ground_contact` is a DERIVED bit that nothing in this project had
  cross-checked, so it is used **with its control printed beside it**: `vy` is
  VERIFIED, and differentiating it gives the map's own gravity — about
  −24.6 m/s² — in free flight against near zero under support.
* **Probe along the car's own down axis, not straight down.** The quaternion is
  VERIFIED too. On flat ground the two are the same question; on a loop only
  one of them is right. Its control is printed too: the median angle between
  the car's up axis and world up, which reads **0.6°** on the flat 252289 and
  **12.0°** on 134672, driven permanently sideways on an ice ribbon.

**The control fired, on the first corpus run that used it.** On 153527 the
contact bit reads `false` on all 85 809 samples — and the mean vertical
acceleration of that supposedly airborne population is **0.0 m/s²**, a car
sitting on a road. Trusting it left the height fit with nothing to score and
the map died with *"no map height puts this run on a surface"*, on a map it had
been fitting to four centimetres. So the bit now has to earn its use: if the
population it calls airborne does not fall at gravity, it is **rejected for
that recording** and the free-fall measurement stands in. Rejected on 153527,
286279 and 284238; used everywhere else; every run says which.

Every sample is then one of four things, and only one of them is a hole:

| class | meaning |
|---|---|
| `Resting` | standing on the model, within 0.25 m |
| `Loose` | supported by something the model has, further off than a car rests |
| `Airborne` | the recording says in flight — **the model owes nothing here** |
| `Missing` | the recording says standing on something and the model has nothing there — **this is the coverage failure** |

**The first thing this measured, and it is a negative result.** On 134672 —
the map vjeux looked at and asked why the car drives over black — 525 of the
562 samples with no surface were samples the recording says were **in contact**.
Only 72 were airborne. The honest coverage was 55.6 % against the raw 55.8 %:
the black was not air, it was a hole, and the hole was the placement bug above.

### 4.2 Fitting the map height, and three criteria, two of them confidently wrong

`world_y = 8*cy + yoff`, and `yoff` is per map. The *criterion* is the whole
game — the wrong ones produce a tight, confident, wrong answer:

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
3. **"How many samples are RESTING — within 0.25 m of a surface, straight
   down."** Anchored at zero, and it got 31 of 33 maps right. But it counts
   flight samples that a wrong height happens to catch, and it cannot see a
   road the car is on the side of.

What is used now is (3) with the two corrections of §4.1: **only the samples
the recording says were standing on something, probed along the car's own down
axis.** A car rests centimetres above what it is on; measured ride heights run
0.013–0.073 m.

The sweep is two passes, whole 8 m cell rows and then metre by metre, because
nothing guarantees a map's height is a whole number of cells — 252289 fits −60
with a 0.017 m gap over 100 % of its samples, so it is not.

### 4.3 Water, and the surfaces a plumb line cannot see

A plumb line only looks down. A car **on water sits under the surface**, so
every water sample used to read as a hole with the water triangle overhead.

MEASURED: Cobalt Cove's water planes sit on exact 8 m cell boundaries —
42.000, 130.000 — and the car reads **41.100** and **129.100**. That is
**0.900 m under, four times out of four**. The probe index lowers every `Water`
triangle by exactly that, so what it holds is the plane a car RESTS on rather
than the plane the water is drawn at; the glTF and the render are untouched.

The control is that **the accuracy improves at the same time**: on Cobalt Cove
the median gap goes 0.144 → 0.101 m as the coverage goes 75.4 → 80.3 %. A
coverage gain bought at accuracy's expense would be a fudge; this is not one.
And the negative control beside it: 134672 and 252289, which have no water, are
unchanged **to the last digit**.

One more thing a strict plumb line loses: `Index::below` demanded the surface
be at or below the sample to within a millimetre. A resting car can read a hair
under its surface, and on water it reads *exactly on* it — the plane sits at a
cell boundary minus the draft and the sample sits at the same number, so a
strict test loses whichever side the last bit falls. `TOUCH` is two centimetres
of slack, and it is worth 7.6 points of coverage on 134672 with the median
*improving* 0.029 → 0.025 m.

### 4.4 Moving blocks: a pose, not a swept hull

`CPlugDynaObjectModel` (`0x09144000`) is the rotors, turnstiles, tubes and
flags. Their surface is somewhere different at every instant, so there is no
pose that is simply correct — and a swept hull is *worse* than useless for a
ride-height probe, because a rotor sweeps a disc the car is inside for a few
hundredths of a second and outside for the rest.

The decision, stated here and in the code: **the block is drawn at its authored
rest pose, and the moving hull's triangles are named `<material> (moving)`**,
so `check` reports how many samples rest on one separately and the two never
average into one number. Where a block gives the same node for both shapes —
the tube does — it is drawn once, as static.

On Cobalt Cove, the Platform map this mattered most for, that number is **1 of
1457 samples**: reading these classes was worth doing and was not the cause of
anything.

### 4.5 The independent cross-check — 173691's canopy deck

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

### 4.6 The physics material table was wrong from id 26 up

`EPlugSurfaceMaterialId` is enumerated in the game's own class reference
(`next.openplanet.dev/MetaNotPersistent/GmSurfaceIds`). Transcribed from there,
ids 0–25 matched what this crate had inferred and **everything from 26 on did
not.** Two of the errors were not cosmetic:

* 27 was called `RoadIce`. 27 is `Bumper_Deprecated`; **RoadIce is 74**, which
  the old table had no name for at all. So the previous headline "134672's ice
  ribbon reads `RoadIce 78 %`" was carried entirely by the map's EMBEDDED
  blocks, whose material is a *link* and takes its name from the link path —
  the stock collision beside it was reading as an unnamed id. With the real
  table the two agree and the map reads **`RoadIce 97 %`**.
* 28 was called `Bumper`. **28 is `NotCollidable`.** Triangles the car cannot
  touch were eligible to be the surface underneath it.

So the probe index now leaves `NotCollidable` and `OffZone` out. Measured
effect on this corpus: **nothing** — no map has a sample resting on either,
which is worth knowing and is not the same as not having checked. An id past
the end of the enum prints as `Unknown(74)` rather than a bare `Unknown`,
because a class of ids is not a diagnosis and the number is free.

---

## 5. Custom items and blocks — the positive control, and three ways to lose one

A map's own models come out of chunk `0x03043054`: a length-prefixed block
holding `filesMeta`, a ZIP, and a texture list. 134672 carries 17 files, 210218
carries 83.

Two model formats turn up, and **both produce triangles** — which is what makes
"the stock pack has no visual meshes" a statement about the pack:

| file | class chain | result |
|---|---|---|
| `3-1-1-5-Ice-Light.Block.Gbx` | `CGameBlockItem` → `CPlugCrystal` | 13 628 triangles, 11 946 of them `RoadIce` |
| `coconut.Item.gbx` | `CGameCommonItemEntityModel` → `CPlugStaticObjectModel` → `CPlugSolid2Model` → `CPlugVertexStream` | 80 triangles, 54 vertices, `Concrete`, bounds 2.3 × 2.4 × 2.4 m |

Those two are the standing control: every reader change in this document was
checked against them and neither number moved.

**Three separate ways an embedded model was being lost**, all found by looking
at the map with the worst coverage and asking what the map says is under the
car:

1. **The zip's container folder is not always `Items/`.** 197047 stores its
   models under `Blocks/`, so both of them resolved to nothing and the map read
   **2.6 %**. The map's spelling is now matched as a *suffix* of the zip path —
   shortest match, because that map carries the same platform under
   `…/StupsKiesel/MiniPlatform/…` and `…/StupsKiesel/StupsKiesel/MiniPlatform/…`
   and the shorter name is a suffix of both.
2. **`CGameBlockItem` version 1 hides its geometry in a second table.** A v1
   block hands out a NULL node in its variant list and puts the shape in a
   table after it: a byte saying the table is present, then per variant a byte
   of flags, then whichever of mesh / collision hull / box / offset it claims.
   The reader had been written with no v1 file to check against and read that
   byte as a 32-bit word, so it stepped two bytes late into a garbage node
   reference and the file failed to open at all. **210218's track is 83
   embedded wood platforms and every one was lost that way** — the map fitted
   −62 with the car 2.171 m above the model.
3. **`CPlugVisual3D`'s inline vertex array is an ALTERNATIVE to a vertex
   stream.** The condition that chose the 40-byte inline form did not check for
   a stream, so on a visual that had one it ate 40 bytes per vertex of the next
   thing in the file. That is why 210218's ice-and-wood blocks could not be
   opened either.

`CPlugCrystal` is the editor's editable mesh: vertices, n-gon faces, a material
index per face. `CPlugMaterialUserInst` carries `surfacePhysicId`, the same
enum a stock block's collision triangles carry — so a custom mesh is coloured by
what the car *feels*. Where the material only LINKS a Nadeo material it has no
id of its own, and the link's own name is used (`Stadium\Media\Material\RoadIce`).

---

## 6. Traps

* **An unknown non-skippable chunk is fatal, and must be.** A GBX body is a
  graph: the only way to reach node 40 is to have parsed nodes 1..39
  byte-exactly. A chunk whose length is not written down cannot be stepped
  over, and guessing desynchronises the walk into somebody else's floats —
  which still produce numbers. `mapgeom` names the class and the chunk and
  stops. `MAPGEOM_TRACE=1` prints every step of the walk and `dump --body`
  writes the decompressed body out, which is how every chunk layout in §5 was
  read: find where the walk stopped, find the *next* thing whose shape you
  already know (a prefab entity's identity quaternion and its `-1` parameter
  chunk are unmistakable), and the bytes between them are the record.
* **A reader written against no example is a guess wearing a reader's clothes.**
  Both §5.2 and §5.3 were code that had never met the case it handled. Neither
  failed quietly — that part of the design works — but neither was true either,
  and both sat there being the reason a whole map was empty.
* **`GbxLoc` is 28 bytes** (a position and a quaternion), not the 48 of an
  `Iso4`. Reading it as an `Iso4` inside a prefab's placement-group parameters
  swallows the rest of the file, and the failure surfaces as *"this prefab ends
  early"* one entity later. 131 prefabs failed this way.
* **`CGameItemModel`'s waypoint chunk `0x2E00201F` is at version 13** in this
  pack; the community spec documents up to 12, and 13 drops the
  `scriptWithSettings` node reference.
* **A `Float3` vertex declaration is packed to `Dec3N` only in LOCAL 3D space.**
  Positions are Global3D and stay full floats.
* **The decoration `.Map.Gbx` files are ENCRYPTED in the pack** — nearly
  everything else carries ForceNoCrypt. The same pack key opens them.
* **A blame that says `(has geometry)` is a different problem from one that
  does not.** `mapgeom check` names the block under every hole and marks
  whether that model produced any triangles anywhere. No triangles means a
  reader or a lookup is missing; triangles somewhere else means a PLACEMENT
  problem, and those are the ones worth an hour.

---

## 7. What is still missing

Each of these is named by a measurement, not guessed at. Per-map detail is in
the run directory's transcripts.

1. **The decoration's `dir = 2` halves are misplaced, because the footprint
   heuristic cannot measure a stadium.** A `Deco48x48` is four blocks — two
   `Stade4096` whose mesh spans 4028 m in x, two `Stade1536` spanning 2048 —
   and reading those spans as footprints puts the two `dir = 2` copies at the
   wrong shift, so the stadium comes out smeared across four kilometres instead
   of closed around the map (`pics-20260822/decoration_48x48_MISPLACED.png`).
   That is why a 48 × 48 map's assembled model stops at z ≈ 1700 while its
   playfield runs past 1760. The answer is the block's real **unit list** —
   `CGameCtnBlockInfo`'s ground and air unit arrays — which needs body readers
   this crate does not have; a zero shift is worse (the model then spans
   −4028..4028), so there is no cheap substitute. This is the single largest
   remaining item and it plausibly explains several low-coverage maps at once.
2. **285885 is outside the model, not missing from it**, and that is now
   reported rather than silently counted as a hole: its run spans z 649..2025
   against a model that ends at 1706, and **664 of its 1225 samples (54 %) are
   past the model's own extent**. Found by `f9c585b3`, who located the real
   surface with a live-engine drop probe — a deck at y ≈ 50, z ≈ 1585, and the
   rim carrying the finish at y 145..158, z 1620..1670 — all of it beyond where
   the assembled decoration stops. Same root cause as (1).
3. **Water roads still give a little back.** Lowering `Water` by the measured
   0.900 m draft is right in the mean and 227654's median gap moves the wrong
   way by 0.010 m while its coverage gains 27 points. Either the draft varies
   with the water block, or the car on a `RoadWater*` ramp is not floating.
4. **Block variant selection.** The union of every variant is drawn. Safe for a
   height probe and wrong for a picture, and it may also be *adding* surface
   where a placement has none. Doing it properly means walking
   `0x0304E023 / 0x0304E027 / 0x0304E02C` → `CGameCtnBlockInfoVariant`
   `0x0315B005` → `CGameCtnBlockInfoMobil` `0x03122003` — the same body readers
   item (1) needs.
5. **`VegetTreeModel` (`0x2F086000`) has no reader** — 1 800 placements of
   `WinterFrozenTree` and 536 of `FirSnowTall` on 210218 alone. Trees are
   almost certainly not collidable, so this is probably worth nothing to
   coverage; it is listed because the count is large enough to look alarming in
   a transcript and should not be mistaken for track.
6. **Two crystal layer types, 13 and 18**, `KinematicConstraint`
   (`0x2F0CA000`), and `LightRay.DynaObject.Gbx`, which fails in the pack
   reader rather than the chunk walk (`bad match offset 49599`) and is the one
   remaining LZ4 case.
7. **Stock visual meshes**, which need the game client's packs (§1). Not wanted
   here: collision is the surface the car is on.
8. **`yoff` from the decoration** rather than from a fit. The decoration names
   a `CGameCtnDecorationSize` (`0x0303B000`) which almost certainly carries the
   base height; its one chunk has no reader yet. That would remove the need for
   a ghost to build a model at all — and 173636, which fits a height and still
   sits 1.399 m above it, is the map that wants it.
