# Map blocks: `0x0304301F`, baked blocks `0x03043048`, free positions `0x0304305F`

Readers/writers: `tools/tmmaps/src/map.rs` (`parse_blocks`, `parse_baked`,
free-position walk, `remove_and_add_blocks`), `tools/tmmaps/src/census.rs`,
`tools/mapgeom/src/place.rs`, `blockmap.rs`, `bake.rs`, `fillers.rs`.
Confidence **[FILE]**; the placement conventions and the flag meanings are
**[VERIFIED-GAME]** (measured against driven trajectories and the editor's own
block list, `/mapblocks2`).

## 1. Chunk `0x0304301F` (non-skippable)

```text
u32 chunk id 0x0304301F
[u32 3]                         lookback version word — normally absent here because an
                                earlier body chunk already wrote it; handle both
Ident  mapInfo                  (uid lookback string, collection number, author lookback)
string mapName
Ident  decoration               (e.g. "48x48Screen155Day", collection, author)
Int3   size                     map grid dimensions (64³ on the control maps)
u32    needUnlock               the password gate (body copy; the header has another)
u32    version
u32    nbBlocks
nbBlocks × BlockRecord          (+ trailing records: see below)
```

`plausible_blocks` finds the chunk by requiring its first Ident to be a fresh
lookback definition (`0x40000000`): the chunk opens the body-level table.

### 1.1 A block record

| field | type | meaning |
|---|---|---|
| name | lookback Id | the block MODEL name (`RoadTechStraight`, `DecoWallBasePillar`, `Water\WaterBase.Block.Gbx_CustomBlock` for an embedded custom block) |
| dir | `u8` | quarter turn 0..3, clockwise looking down |
| cell | `u8[3]` | x, y, z; the file stores x and z ONE HIGHER than the world grid cell (`coords()` subtracts (1,0,1)); dead for a FREE block |
| flags | `u32` | see §1.2; `0xFFFFFFFF` = an "unassigned" placeholder that does not count towards `nbBlocks` |
| author | lookback Id | only when `flags & 0x8000` |
| skin | node ref | only when `flags & 0x8000`: a `CGameCtnBlockSkin` node (§4) or null |
| waypointParams | node ref | only when `flags & 0x100000`: a `CGameWaypointSpecialProperty` node (§5) or null |

Records past `nbBlocks` are listed while the next word still has its top bits
set (a lookback word always does; a chunk id does not).

### 1.2 Block flags (`tools/mapgeom/src/blockmap.rs`, `tools/tmmaps/src/fillers.rs`)

| bits | name | meaning |
|---|---|---|
| 0..5 | variant index (`FLAG_VARIANT_MASK 63`) | which MOBIL LIST of the picked variant; for a `DecoWallBaseVFC` filler: 0 Middle, 1 Top, 2 Bottom, 3 TopBottom, 4 NOTHING (covered by a merged piece below), 5..10 Middle×2/3/4/8/16/32 |
| 6..11 | sub-variant (`FLAG_SUBVARIANT_SHIFT 6`) | the mobil index inside that list (the A/B/C/D alternates of one piece) |
| 12 | `0x1000` GROUND | picks the ground variant (a ground block's upper units bake AIR clips; the clip block takes the record's ground bit) |
| 13 | `0x2000` CLIP | a clip piece |
| 14 | `0x4000` PILLAR | |
| 15 | `0x8000` SKINNABLE | the record carries author + skin ref |
| 16 | `0x10000` REPLACEMENT | |
| 17 | `0x20000` DECAL | |
| 20 | `0x100000` WAYPOINT | the record carries a waypoint node |
| 21..27 | additional-variant index (`FLAG_ADDITIONAL_SHIFT 21`) | index into the block info's additional ground/air variant list |
| 28 | `0x10000000` GHOST | ghost-mode (drive-through) block; also set on the FILLERS generated for a ghost block |
| 29 | `0x20000000` FREE | the block is FREE-placed: position and rotation in `0x0304305F`, cell bytes dead |

A custom embedded block record as the TMX maps write it carries
`flags = 0x10208000 | FREE`, `author = the manifest author`, a null skin ref
(`tools/mapgeom/src/waterblocks.rs`, `tools/tmmaps/TINY.md` "Water, solved").

Against the editor's own list (`/mapblocks2?list=baked`, 7287 records of
Summer 20): 7280 of 7281 clip records pick exactly the mobil the flag bits say;
the one exception took mobil variant 3 for the file's 2 (an A/B/C/D alternate)
(`tools/mapgeom/src/shape_audit.rs`).

## 2. Placement of a GRID block **[VERIFIED-GAME]**

```text
world = (32*cx + lx,  8*cy + yoff + y,  32*cz + lz)

dir=0: (lx,lz) = (x,      z)          dir=2: (SX - x, SZ - z)
dir=1: (lx,lz) = (SZ - z, x)          dir=3: (z,      SX - x)
```

`SX × SZ` is the block's footprint in whole 32 m cells; in unit cells
(`blockmap.rs`): `dir=1: (D-1-z, x)`, `dir=2: (W-1-x, D-1-z)`, `dir=3: (z, W-1-x)`
with `W × D` the variant's footprint, and a unit's local side `s` faces world
side `(s + dir) % 4` (North +z = 0, East −x = 1, South −z = 2, West +x = 3).
The shift back onto the footprint was paired with the wrong quarter turn until
2026-08-22: every `dir 1` and `dir 3` block sat one whole footprint away, which
a height fit cannot see (`tools/mapgeom/MAPGEOM.md` §3: 55.8 % → 87.1 % of a
run over a surface on 134672, with the accuracy unchanged).

`yoff` per environment is in `map-cgamectnchallenge.md` §3. World point of a
grid cell's centre: `(32·cx + 16, 8·cy − 62, 32·cz + 16)` in Stadium.

## 3. FREE blocks and chunk `0x0304305F`

```text
u32 version
24 B per free block: f32 x, y, z ; f32 pitch, yaw, roll      (the order is x,y,z,YAW?,…: see below)
   — every FREE block of 0x0304301F in record order, THEN every FREE block of 0x03043048 in its order
```

The walk is required to land exactly on the chunk end (an assertion that only
counts entries cannot fail on a wrong ordering; measured: 3 148 of 3 148 payload
bytes on 267460 = 24 unbaked + 107 baked; on 210218 11 762 of 14 542 entries are
baked and writing the unbaked Goal's entry produced 13 predicted gate crossings
with 13 hits at max 6 ms).

**The rotation triple, measured 2026-09-11 with a water-volume block** (`TINY.md`
"Free block rotation"): the entry is `x, y, z, YAW, pitch, roll`. Yaw 0 places
the block frame from the origin corner; yaw +π/2 rotates about the ORIGIN
CORNER with local `(x, z) → world (x₀ + z, z₀ − x)`; a 90° value in the second
angle slot is a pitch and puts the volume nowhere useful. So a source grid
block of direction d in cell (cx, cz) becomes a free block at corner
d0 (x₀, z₀), d1 (x₀, z₀+32), d2 (x₀+32, z₀+32), d3 (x₀+32, z₀) with yaw d·π/2.
(`tools/tmmaps/src/rotate.rs` treats the three floats as pitch/yaw/roll in the
game's own naming; `rotate --tilt` decomposes a world tilt into them at first
order: `droll = angle·cos(yaw−dir)`, `dpitch = angle·sin(yaw−dir)`.)

Traps, all measured (`tools/tmmaps/MAPS.md` §3):

* a cell write on a free block is SILENT: the map loads, the origin control
  passes, and every ladder rung is silent (210218's two `GateExpandableFinish`
  Goals sit at raw cell (0,0,0));
* a per-block rotation turns a block about ITS OWN anchor: tilting a 32 m-tile
  road per block makes a staircase (3.4° → 1.9 m steps);
* "one block" is often several free blocks sharing an anchor to the millimetre
  (284238's ice kicker is four); `rotate` refuses when a free block within
  `--group-radius` is left out.

## 4. `CGameCtnBlockSkin` (`0x03059000`)

Written inline at the block's skin node ref. Chunks: `0x03059000` (two
strings), `0x03059001` (string + FileRef), `0x03059002` (string + two
FileRefs), `0x03059003` (`u32` + FileRef), skippable ones stepped over,
`0xFACADE01`. No Id fields, so the bytes are copied through untouched
(`map.rs::read_skin_node`). The advertisement skins the header declares as
`<dep file="Skins\Any\Advertisement4x1\Summer4x1.zip"/>` are referenced from
here.

## 5. `CGameWaypointSpecialProperty` (`0x2E009000`)

```text
u32 class 0x2E009000              (items write it with the class id and no index)
u32 chunk 0x2E009000
u32 version
v ≥ 2:  string tag ("Spawn" | "Checkpoint" | "LinkedCheckpoint" | "Goal"), u32 order
v < 2:  u32 order, u32 (spawn)
[0x2E009001 skippable, 8 B]       seen on every waypoint node of map2
0xFACADE01
```

**The tag in the file is ignored by the game [VERIFIED-GAME]**: what a waypoint
does is decided by the MODEL (`RoadTechCheckpoint` vs `RoadTechFinish`,
`GateCheckpointLeft32m` vs `GateFinish32m`). Retagging Checkpoint → Goal, or
deleting the node outright, changes nothing (four experiments, all 19.538 / 4
CPs) (`tools/tmmaps/src/segments.rs`). `order` is the checkpoint's number in a
linked group; our writer once emitted 0 everywhere and the route finder read
"unset" on 25 maps (2026-09-07). Checkpoint ORDER in the race is not in the
file; it is measured by promotion (`segments.rs`).

A parked or renamed waypoint BLOCK still defines the spawn/checkpoints — the
tiny converter renames parked waypoint blocks to a plain block (memory
`tm2020-embedded-items-rules.md`).

## 6. Baked blocks `0x03043048`

```text
u32 version, u32 u01, u32 nbBakedBlocks
nbBakedBlocks × BlockRecord     (same record layout, same flag meanings)
u32 u02, u32 nbBakedClips        (0 on every map read; entries would carry Idents — refused)
```

**It CONTINUES the body-level lookback table** (§ container 6.1): its first
record defines `Sea` and its next references slot 50, a string defined in
`0x0304301F`. Both chunks must be re-encoded as one stream; renumbering one
alone is `Can't load map`.

What the records are: the pieces the EDITOR bakes at edit time — every clip
piece (a pillar's walls, a platform's skirts, a road's end caps) as one record
in the cell it is drawn in, its `dir` naming the cell side it stands on
(`Base_VFCMiddle_Air` is the local z = 32 = North plane), the terrain sheet
(`Sea`, `Grass`, `Land`, `LandHill3`…) and the decoration. Baked index N is
**not** unbaked index N; a baked block's cell bytes are dead, but a FREE baked
block has its six floats in `0x0304305F` like an unbaked one — twelve of
173691's sixteen finish-gate pieces are exactly that (`tmmaps move bN@x,y,z`).

The file's baked list is exactly what the engine's free-clip algorithm derives
(`mapgeom bake --diff`: 0 stale records on all 25 Summer 2026 maps, 46 304
records); the algorithm itself is in `blockinfo-cgamectnblockinfo.md` §7.
**The client does not draw the file's BakedBlocks; it re-derives them at load**
(`InitChallengeData_Clips`, `tools/mapgeom/src/bake.rs`) — the record only
donates attributes (variant alternate, lightmap id, colour) when the grid
holds one for that owner face.

Filler conventions (`tools/mapgeom/src/fillers.rs`): a FreeClipBottom piece
(FCB, the underside of the block ABOVE) is recorded in the cell below and lies
at the top of its cell (`Straight_FCBInside` is y 8..9); a FreeClipTop piece
(FCT, the top plate of the block BELOW) is recorded in the cell above; world
side w of a unit holds the block's local side `(w − dir) mod 4`.

## 7. Adding and removing records (`map.rs::remove_and_add_blocks`)

Everything that lists blocks moves together: `0x0304301F` (records +
`nbBlocks`), `0x03043048` (records + count + chunk size), the shared lookback
table re-encoded first-use-defines, `0x0304305F` (only kept FREE blocks),
`0x03043062` and `0x03043068` (per-block bytes; per-item bytes stay),
`0x03043069` (per-block refs), `0x03043040`'s snapped-on tables (a group naming
a dropped block goes, its items are un-snapped, block indices renumbered). A
record's skin/waypoint node is written inline at its FIRST reference, so a
dropped record that defined a shared node has that node re-inlined at the
first surviving reference. Variable-length → staged as raw splices, never
combined with a rename in one write.

## 8. Related measurements

* `tmmaps census MAP` lists both chunks with real positions; `tmmaps region
  --box` counts a structure: 173691's added finish gate is SIXTEEN blocks (4
  unbaked, 12 baked, y down to 64), and moving its one visible anchor left a
  car bumping the invisible remains 77.8 m onto the deck (`MAPS.md` §3).
* Terrain tiles (Flat/Frontier zone blocks) are hidden in every cell a block
  UNIT covers — and only there; the raw file cell of a rotated multi-cell
  block is its footprint's min corner and may be an EMPTY corner
  (`RoadTechCurve4`'s units are 11 of 16 cells) — `tools/tmmaps/src/tiny/tiles.rs`,
  `TINY.md` "Terrain tiles under blocks" (2026-09-11).
* `mapgeom collhash` hashes name, cell, dir, flags, free position/rotation of
  every authored and baked block: a parked block still carries collision and a
  waypoint (`tools/mapgeom/src/collhash.rs`).

## 9. Not known

* The exact meaning of flag bits 16, 17, 21 (beyond GBX.NET's names) and of
  `u01`/`u02` in `0x03043048`.
* The baked-clip tail (`nbBakedClips > 0`).
* Whether the free rotation triple order is `yaw,pitch,roll` for ALL blocks or
  only measured on the water-volume block (one measurement, 2026-09-11).
