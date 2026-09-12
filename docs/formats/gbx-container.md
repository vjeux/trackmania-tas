# The GBX container

Every Trackmania file this project touches — `.Map.Gbx`, `.Ghost.Gbx`,
`.Replay.Gbx`, `.Item.Gbx`, `.Block.Gbx`, `.EDClassic.Gbx`, `.Prefab.Gbx`,
`.Material.Gbx`, `.Light.Gbx`, `.LaunchedCP.gbx` … — is one container format,
"GBX". This page is the container; the classes inside it have their own pages.

Confidence: **[FILE]** unless marked. The layout has been round-tripped
byte-for-byte on hundreds of files by three independent readers in the tree
(`tools/gbx/src/container.rs`, `tools/tmmaps/src/gbx.rs`,
`tools/mapgeom/src/container.rs` + `reader.rs`); the community transcription it
was checked against is GBX.NET and gbx-py.

## 1. File layout

| offset | type | name | meaning |
|---|---|---|---|
| 0 | `char[3]` | magic | `GBX` |
| 3 | `u16` | version | 6 on every TM2020 file seen |
| 5 | `u8` | format | `'B'` binary (`'T'` text exists in the format, never seen here) |
| 6 | `u8` | refTableCompression | `'U'` on every file seen |
| 7 | `u8` | bodyCompression | `'C'` = LZO1X-compressed body, `'U'` = raw body |
| 8 | `u8` | unknown | present when version ≥ 4; `'R'` on every file seen |
| 9 | `u32` | classId | the main node's class, e.g. `0x03043000` map, `0x03092000` ghost, `0x03093000` replay, `0x2E002000` item |
| 13 | `u32` | userDataSize | version ≥ 6 only |
| 17 | `u8[userDataSize]` | user data | the **header chunk table + header chunks** (§2) |
| … | `u32` | numNodes | number of nodes in the body's node table (the game's writer counts them; a patcher must not touch it — a rebuilt map with `numNodes` 10 instead of 39 is `Can't load map` in the dedicated server) |
| … | ref table | reference table | §3 |
| … | body | body | §4 |

Sources: `tools/gbx/src/container.rs::Gbx::parse`, `tools/tmmaps/src/gbx.rs`
(module doc), memory `gbx-map-surgery-lookback-tables.md`.

The game ships uncompressed-body files itself (`CarRally.Item.Gbx` is
`B U U`), so `'U'` is a legal encoding a reader must accept. What the DEDICATED
SERVER accepts differs by class: a ghost/replay with a `'U'` body validates
(every candidate `tools/ghost` writes is `'U'`), a **map with a `'U'` body is
refused with `Can't load map`** **[VERIFIED-GAME]** (`tools/tmmaps/src/gbx.rs`
tests; the 285-map sweep in `tools/tmmaps/MAPS.md` §1a found 21 such maps
written by an older writer and re-wrote them compressed).

## 2. Header user data: the header chunk table

```text
user_data := u32 n_chunks
             n_chunks × { u32 chunk_id, u32 size_with_heavy_bit }
             the chunk bodies, concatenated, in table order
```

The top bit of the size word (`0x80000000`) is the "heavy" flag and is not part
of the size. A string edit inside a header chunk must therefore also fix that
chunk's size word; the outer `userDataSize` follows from the rebuilt table
(`tools/gbx/src/header.rs`, `tools/tmmaps/src/header.rs::user_chunks`). A table
that does not account for every byte of the user data is treated as unparsed
rather than partially parsed.

Header chunks seen per class (each class page lists them in detail):

| class | header chunks |
|---|---|
| map `0x03043000` | `0x03043002` (times/description), `0x03043003` (common: uid, name, author), `0x03043004` (version), `0x03043005` (the community XML string), `0x03043007` (thumbnail JPEG + comments), `0x03043008` (author) |
| replay `0x03093000` | `0x03093000` (map meta, race time, driver), `0x03093001` (XML), `0x03093002` (driver login/nickname/zone) |
| plain ghost `0x03092000` | **none** — 0 of the 158 published `.Ghost.Gbx` have a header chunk table; a synthesised ghost header is `GBX` v6 `B U C R`, class `0x03092000`, `userDataSize 0`, ref table `u32 0` and it comes out byte-identical to Nadeo's (memory `tm2020-embedded-author-ghost.md`) |
| item `0x2E002000` / block `0x2E025000`-family | `0x2E001003` (collector ident: id, collection, author), `0x2E001004` (icon), `0x2E001006` (file time), `0x090F4000` (game skin declaration, only on skinnable models) |

## 3. Reference table

```text
u32 n_external
if n_external > 0:
    u32 ancestorLevel
    u32 nFolders; recursively per folder: string name, u32 nSubfolders, subfolders
    n_external × { u32 flags;
                   if flags & 4 == 0: string fileName  else: u32 resourceIndex;
                   u32 nodeIndex;
                   u32 useFile        (version ≥ 5)
                   if flags & 4 == 0: u32 folderIndex }
```

The folder index is **1-based with 0 = the ancestor directory** — getting that
off by one puts every referenced file in a sibling folder and resolves to
nothing, which reads exactly like "the pack does not have it"
(`tools/mapgeom/MAPGEOM.md` §2). A `.EDClassic.Gbx`'s ref table is where a block
model names the `.Prefab.Gbx` files that carry its shape
(`tools/mapgeom/src/container.rs`). Maps and ghosts in this corpus carry zero
external refs; an embedded item that references a PACK file by path is dropped
by the game (see `item-cgameitemmodel.md`).

A node index owned by an external ref entry is read as a reference with **no
class id and no inline body** (memory `tm-hpl-pack-cipher-SOLVED.md`).

## 4. Body

```text
if bodyCompression == 'C':  u32 uncompressedSize, u32 compressedSize, LZO1X stream
else:                        raw bytes to end of file
```

The decompressed body is the main node's chunk stream: a sequence of
`u32 chunk_id` + payload, terminated by the node end marker **`0xFACADE01`**.
A body has to be parsed serially; there is no index (§6).

### LZO1X

Decompression is `lzo1x_decompress_safe` from the system `liblzo2`, reached with
`dlopen` at run time (`tools/gbx/src/container.rs::lzo_init`). A failure is
data, not a crash: the reader returns what decoded and the structural gates
refuse the file.

**Recompression is not bit-reproducible and the game's own compressor is
stronger than `lzo1x_1_compress`**: re-emitting map 173691 returns a body
29 763 bytes longer that shares nothing after the header with the download. A
literal-only LZO stream (one literal run + the `11 00 00` end marker) is legal,
so a `'C'` body can be written without a compressor at all (memory
`gbx-map-surgery-lookback-tables.md`). How `tmmaps` edits a compressed map
without recompressing it is in `lzo-body-splice.md`.

## 5. Chunk framing: skippable and non-skippable

A chunk is **skippable** iff the word after its id is `PIKS` (`0x534B4950`,
"SKIP" little-endian):

```text
skippable:      u32 chunk_id, "PIKS", u32 payload_size, payload
non-skippable:  u32 chunk_id, payload            (no size anywhere)
```

Peeking for `PIKS` is a reliable walker rule. A non-skippable chunk whose layout
the reader does not know is **fatal, and must be**: its length is not written
down, and guessing desynchronises the walk into somebody else's floats — which
still produce numbers (`tools/mapgeom/src/node.rs`). "Terminating exactly on
`0xFACADE01` having consumed every byte" is the structural proof a parse is
right; every reader here asserts it.

`gbx::container::all_skip_chunks` byte-scans a whole body for `PIKS` with a
plausibility filter on the id's top byte (`0x03 | 0x0B | 0x24 | 0x2E | 0x30`).
Two artefacts of that shortcut are worth knowing:

* it reports **phantom chunks** inside non-skippable payloads and inside
  compressed blobs (the "27 chunks, 21 opaque" ghost picture was this);
  `tools/ghost/src/synth.rs::split` merges spans that are STRICTLY inside a
  previous one and keeps adjacent ones separate (merging on `<=` glued the
  4-byte `0x0303F007` onto the 10 KB `0x03092000`);
* it walks straight past the **inline** chunks a class carries between its
  skippable ones — a ghost's account id `0x0309200F`, its map uid `0x03092010`
  and a replay's whole embedded map `0x03093002` are all inline.

## 6. The two stateful encodings

A GBX body is a graph, not a stream of records. Two encodings carry state
across the whole body, and a reader that fabricates the state reads plausible
garbage (`tools/mapgeom/src/reader.rs`).

### 6.1 Lookback strings ("Id", `MwId`)

```text
word u32:  0xFFFFFFFF            -> null / unassigned
           top 2 bits == 0       -> a predefined number (a collection id: 26 = Stadium,
                                    0x1C BlueBay, 0x10 RedIsland, 0x1D WhiteShore, 0xF GreenCoast,
                                    10003 = the "Vehicles" catalog chapter), no bytes follow
           0x40000000            -> a NEW string follows (u32 len + bytes), APPENDED to the table
           0x40000000 | n        -> the n-th table entry, 1-based and ABSOLUTE
           0x80000000 flag       -> read like 0x40000000 by our readers; no corpus file uses it
```

The **lookback version word `3`** (`03 00 00 00`) is written once, immediately
before the first Id of a table. Duplicate strings are legal.

**Tables are NOT uniformly scoped, and this is the trap of the format.**
Measured on maps (`tools/tmmaps/src/map.rs` module doc, memory
`gbx-map-surgery-lookback-tables.md`):

| stream | table |
|---|---|
| body-level chunks: `0x0304301F` blocks … `0x03043048` baked blocks, `0x03043051` | ONE shared table; the baked chunk opens with `0x40000000 "Sea"` and its next records reference slot 50, a string defined back in the blocks chunk |
| skippable `0x03043040` (items), `0x03043043` (genealogy), `0x03043054` (embedded objects) | each opens its OWN table: re-writes the version word 3 and re-defines `Nadeo` |
| a map's `0x0305B00F` validation ghost | its OWN chunk-local table (the Summer 21 skeleton→real insertion test, 2026-09-08; `map-validation-ghost.md` §1) |
| an item body (`.Item.Gbx`) | body-wide: the ident strings come first, the crystal's `Layer0`… and material ids join the same table |
| a ghost body | body-wide; the first lookback string of the ghost node carries the version marker |

Consequences, all measured: the dedicated server says `Can't load map` whenever
the shared table's LENGTH changes (downstream chunks hold raw indices into it —
`Fresh` vs `SlotPreserving` re-encoders in `map.rs`); **never insert a string at
the front of a shared table and renumber** — append at the tail instead.

### 6.2 Node references

```text
u32 index:  0xFFFFFFFF -> null
            an index seen before -> back-reference, nothing follows
            a NEW index -> u32 classId, then the node's chunks, ended by 0xFACADE01
```

Node indices are assigned in write order, so a new node's index is the next
free one — the MediaTracker walker leans on that to find the end of a block
whose layout it does not know (`tools/tmmaps/src/mediatracker.rs`). Gaps in the
numbering are fine: the reader fills its node table by index as nodes appear
(`map.rs::remove_blocks`). Two variants exist:

* **written with class id, no index** — how the items sub-archive writes
  `CGameWaypointSpecialProperty` (`0x2E009000`) and how a map's `0x0305B00F`
  writes its ghost (`u32 0 | u32 len | classId | chunks | FACADE`). A reader
  that expects an index there takes `0x0911F000` as a node index and the file
  "loads 0 ghosts" (memory `tm2020-embedded-author-ghost.md`);
* **`WriteNode` inline nodes** — chunk ids directly, no class id, no index:
  the `CPlugIndexBuffer` inside a visual (`tools/mapgeom/src/static_item/visual.rs`).

### 6.3 Archive classes (no chunks at all)

Some classes are serialised as a bare struct with no chunk ids and no FACADE:
`CPlugStaticObjectModel` (`0x09159000`), `CPlugDynaObjectModel` (`0x09144000`),
`NPlugDyna_SKinematicConstraint` (`0x2F0CA000`), `CPlugVegetTreeModel`
(`0x2F086000`) (`tools/mapgeom/src/static_item/{item,dyna}.rs`, `veget.rs`).

## 7. Primitive types

| type | encoding |
|---|---|
| `string` | `u32 length` + bytes, UTF-8 (display names carry a BOM, `$`-colour codes, circled letters) |
| `bool` | `u32` 0/1 (`bool32`), or a single `u8` where the class page says so |
| `Vec2/Vec3` | f32s |
| `Int3` | three i32 |
| `Iso4` | 12 f32: a 3×3 rotation then a translation (48 bytes) |
| `GbxLoc` | **28 bytes**: position + quaternion — reading it as an `Iso4` inside a prefab's placement-group parameters swallows the rest of the file and surfaces as "this prefab ends early" one entity later (131 prefabs failed that way) |
| `Ident` / meta | three lookback ids: `id`, `collection` (a predefined number), `author` |
| `FileRef` | `u8 version`; `[u8;32] checksum` when version ≥ 3; `string path`; `string locatorUrl` when version ≥ 1 and (path non-empty or version ≥ 3) (`tools/tmmaps/src/map.rs::read_file_ref`, `header.rs::FileRef`) |
| `MwBuffer` / data | `i32 length` + raw bytes |
| optimized int | 1/2/4 bytes wide, chosen by a COUNT: `count < 0xFF → u8, < 0xFFFF → u16, else u32` (GBX.NET's thresholds, `<` not `<=`: an item with exactly 255 positions writes u16 indices). A lone index is sized by the number of things it indexes; a length-prefixed index array by ITS OWN length (`tools/mapgeom/src/crystal_model.rs`) |
| times | milliseconds as `u32`/`i32`; printed everywhere in this project as seconds with a decimal (`22.730`) |

## 8. Class ids met in this project

The class id is the top of every chunk id of that class (`0x03043000` →
chunks `0x030430xx`). Community reference: `next.openplanet.dev/Game/<Class>`.

| id | class | page |
|---|---|---|
| `0x03043000` | CGameCtnChallenge (map) | `map-cgamectnchallenge.md` |
| `0x03059000` | CGameCtnBlockSkin | `map-blocks.md` |
| `0x2E009000` | CGameWaypointSpecialProperty | `map-blocks.md`, `map-items.md` |
| `0x03101000` | CGameCtnAnchoredObject (item placement) | `map-items.md` |
| `0x0311D000` | CGameCtnZoneGenealogy | `map-genealogy.md` |
| `0x03079000` / `0x0307A000` / `0x03078000` | CGameCtnMediaClip / ClipGroup / Track | `map-mediatracker.md` |
| `0x030A0000` … `0x030AB000`, `0x0304B000`, `0x0304C000`, `0x03199000` | MediaTracker blocks | `map-mediatracker.md` |
| `0x0303F000` | CGameGhost | `ghost-cgamectnghost.md` |
| `0x03092000` | CGameCtnGhost | `ghost-cgamectnghost.md` |
| `0x03093000` | CGameCtnReplayRecord | `replay-cgamectnreplayrecord.md` |
| `0x0911F000` | CPlugEntRecordData (telemetry) | `ghost-telemetry-cplugentrecorddata.md` |
| `0x0A018000` / `0x0A00C000` | CSceneVehicleVis / CSceneVehicleVisState | `ghost-telemetry-cplugentrecorddata.md` |
| `0x03262000` | CGameSaveLaunchedCheckpoints | `ghost-caches.md` |
| `0x2E002000` | CGameItemModel | `item-cgameitemmodel.md` |
| `0x2E027000` / `0x2E026000` | CGameCommonItemEntityModel / …Edition (crystal item) | `item-cgameitemmodel.md`, `crystal-cplugcrystal.md` |
| `0x2E020000` | CGameItemPlacementParam | `item-cgameitemmodel.md` |
| `0x2E025000`-family, "CGameBlockItem" | custom block item (`.Block.Gbx`) | `blockinfo-cgamectnblockinfo.md` §9 |
| `0x09159000` | CPlugStaticObjectModel | `prefab-and-dyna.md` |
| `0x09145000` | CPlugPrefab | `prefab-and-dyna.md` |
| `0x09144000` / `0x2F0CA000` | CPlugDynaObjectModel / NPlugDyna_SKinematicConstraint | `prefab-and-dyna.md` |
| `0x090BB000` | CPlugSolid2Model | `mesh-cplugsolid2model.md` |
| `0x0901E000` / `0x09006000` / `0x0902C000` / `0x0906A000` / `0x09057000` | CPlugVisualIndexedTriangles / CPlugVisual / CPlugVisual3D / CPlugVisualIndexed / CPlugIndexBuffer | `mesh-cplugsolid2model.md` |
| `0x09056000` | CPlugVertexStream | `mesh-cplugsolid2model.md` |
| `0x0900C000` | CPlugSurface | `collision-cplugsurface.md` |
| `0x09003000` | CPlugCrystal | `crystal-cplugcrystal.md` |
| `0x090FD000` / `0x09079000` / `0x0903A000` | CPlugMaterialUserInst / CPlugMaterial / CPlugMaterialCustom | `materials.md` |
| `0x0901D000`, `0x0400B000` … | CPlugLight, GxLight* | `prefab-and-dyna.md` |
| `0x0915C000`, `0x090B3000`, `0x090B2000`, `0x090B5000`, `0x090C5000`, `0x090C6000` | CPlugFxSystem and the particle chain | `prefab-and-dyna.md` |
| `0x2F086000` | CPlugVegetTreeModel | `prefab-and-dyna.md` |
| `0x0917B000`, `0x09179000` | NPlugTrigger_SSpawn companion, NPlugTrigger_SGateSpecial | `item-cgameitemmodel.md` |
| `0x0304E000`-family, `0x0315B000`, `0x03122000`, `0x03036000` | CGameCtnBlockInfo*, Variant, Mobil, BlockUnit | `blockinfo-cgamectnblockinfo.md` |
| `0x0303B000` | CGameCtnDecorationSize | `blockinfo-cgamectnblockinfo.md` §10 (no reader yet) |
| `0x090EC000`, `0x090ED000` | CPlugVehiclePhyTuning (the physics packs) | `pak-nadeopak.md` §7 |

## 9. What is NOT known

* Header chunk `0x03043004`, `0x03043008` payload layouts beyond "copied
  verbatim"; only `0x03043002/003/005/007` are parsed (`map-cgamectnchallenge.md`).
* The `0x80000000` lookback flag: never seen; treated as `0x40000000`.
* Text-format (`'T'`) containers: never seen, no reader.
* `refTableCompression` other than `'U'`: never seen.
