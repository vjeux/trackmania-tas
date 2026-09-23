# The map lightmap: chunk `0x0304305B` decoded — layout, mapping, authoring

Readers/writers: **`tools/lightmap`** (`lmtool`: parse/round-trip, dump,
paint, synth, bake), `tools/tmmaps/src/map.rs::strip_lightmap`,
`tools/chunkswap` (transplant), `tinyctl lightmap` (the editor bake pipeline),
`mapgeom static-item` (the items' lightmap UVs). Confidence tags: **[FILE]** =
read off the files and round-tripped byte-identically over the 25 Summer 2026
sources + 15 editor-baked tiny maps; **[GAME]** = verified in play mode on the
render box (2026-09-22, session lightmap-re); **[INFERRED]** = consistent with
the data, not yet tested in the game.

## 1. Chunk `0x0304305B` (skippable) **[FILE]**

```text
u32 version            0 on every file seen
u32 hasLightmaps       bool32; 0 = never computed (chunk is then 16 bytes)
u32 u01, u32 u02       0, 0
--- only when hasLightmaps:
u32 lightmapVersion    10
u32 frameCount         3
frameCount × 3 blobs   blob = u32 len + bytes; a WEBP file or len 0 (absent)
u32 cacheUncompressed
u32 cacheCompressed
byte[cacheCompressed]  zlib → the CHmsLightMapCache node (§3)
```

The nine blobs, in order (`frame.image`):

| blob | frame.image | content | typical |
|---|---|---|---|
| 0 | 0.0 | **colour atlas** (VP8 lossy, 1024×1024) — the lightmap the items are lit with | 300–460 KB |
| 1 | 0.1 | **grey atlas** (1024×1024, R=G=B) — a directional/normal-mapping term; no brightness effect **[GAME]** | 130–300 KB |
| 2 | 0.2 | small colour atlas (198²–418², side grows with the vegetation) — the sprite/foliage lightmap **[INFERRED]** | 20–100 KB |
| 3 | 1.0 | **point-light atlas** (1024×1024): black except the baked glow of lamps/checkpoint lights | 90–340 KB |
| 4,5 | 1.1, 1.2 | absent (len 0) | |
| 6 | 2.0 | 1024×1024, nearly black (a third light frame; content unknown) | 3–320 KB |
| 7,8 | 2.1, 2.2 | absent | |

The game accepts **lossless VP8L** WEBP in place of Nadeo's lossy VP8
**[GAME]** (`image-webp`, pure Rust).

## 2. Chart layout and the object mapping **[FILE]** / **[GAME]**

The atlas is laid out in a **2048×2048 texel space** while the stored image is
1024×1024: a chart at layout `(x, y, w, h)` occupies pixels
`[(x+1)/2, (x+1)/2 + w/2) × [(y+1)/2, …)`. Nadeo's charts have odd positions
and even sizes (1-pixel gutters between charts). Nadeo fills ~78 % of the
image; chart sizes run 4×4 … 354×112 layout texels (2×2 … 177×56 px), roughly
**1.1 layout texels per metre** on the tiny maps (an 8 m tile → 18×18).

A chart is bound to an **object by index** (the `binds` table, §3.6):

* **Nadeo/editor maps with authored blocks**: object index space =
  `[unbaked blocks][baked blocks][items]` in file order — items start at
  `n_unbaked + n_baked` (Summer 11: 3647+3429 = 7076; Summer 01: 4644; exact).
* **Tiny builds (0 unbaked blocks)**: `[4096 ground slots][items]` — item `i`
  is object **4096 + i** on Tiny 11 and Tiny 16 (all 4096 low slots carry a
  chart: the 64×64 zone/ground grid the game regenerates at load) **[GAME]**:
  painting chart `4096+i` colours item `i`.
* Stadium-decoration maps (Summer 05, 25): 4 decoration objects at 0..3, the
  map's objects at 16384 + index.
* A block with several visuals gets several charts (sub-index in the bind's
  first word; Summer 11: up to 353 per block); a tiny item gets exactly one.
  Items without lightmappable geometry (2 of 8619 on Tiny 16) get none.
* The per-item lightmap quality byte (chunk `0x03043068`) and the item's
  rotation move a model's chart size by ±2 texels (16/18); otherwise every
  placement of a model has the same chart size.

## 3. The `CHmsLightMapCache` node (inflated blob) **[FILE]**

PIKS-framed chunks, then `FACADE01`, then a **trailer** (§3.8):

| chunk | size | content |
|---|---|---|
| `0x0602200B` | 8 | `u32 1, u32 hash` — one word per map/bake (GBX.NET "MapT3s"); differs between two bakes of the same map only when the map changed |
| `0x0602200F` | 8 | `u32 3, u32 0` on every file (q=3 and q=4 bakes alike) |
| `0x06022013` | 16 | `u32 1, u32 1, u64` — a FILETIME-shaped word that is not the bake time |
| `0x06022015` | 57–65 | `u32 5, u64 hash, u32 3, u32 collection (0x1c BlueBay-family, 0x1a Stadium), f32 2.0, string mood ("Day64", "Sunrise64", "48x48Screen155Day"), u32 1, u32 0, u32 moodhash-or-−1, u32 0, u32 0, u32 0` |
| `0x06022016` | 4 | `u32 8` — the lightmap version the header XML declares (`lightmap="8"`) |
| `0x06022017` | 8 | `u32 0, u32 n` — a count (0 on Stadium maps; 217 / 29983 on the tiny bakes; 2–3 M on Nadeo's) |
| `0x06022018` | 8 | `u64` FILETIME-shaped constant on Nadeo's files, 0 on the editor's tiny bakes |
| `0x06022019` | 4 | `u32 14` |
| `0x0602201A` | 65–400 KB | **the mapping** (§3.1–3.7) |

### 3.1 Mapping head (`0x0602201A`, version 13)

```text
u32 13                      chunk version
u32 1
u32[6] 0
u32 256, u32 64, u32 25
u32 1, 2, 1, 3, 1, 4        (constant on every file)
u32 3                       frame count
3 × 66-byte frame record    (§3.2)
u32 2, u32 0, u32 0         (before the struct below; constant)
```

### 3.2 Frame record (66 bytes, ×3)

```text
u32 kind        3, 3, 2 on every file
u32 0
u32 moodhash    0x9b59 Day64, 0xdaab Sunrise64, 0xceb8 48x48Screen155Day
f32 -FLT_MAX
f32 maxHdr      3.0 (2.7 on Stadium) — the mood's T3LightMap MaxHDR
f32 actualMax   per map (2.15–3.0; frame 2: 0.31–3.0)
f32 bounce      2.0 (1.8 Stadium) — T3LightMap BounceFactor
f32 sky         1.0 — T3LightMap SkyFactor
u32 1
f16[3]          frame 0: a colour (~0.6–1.8); frames 1,2: −1,−1,−1
u32             frame 0: 1; else 0
u32 frameIndex  0, 1, 2
u32 2
f32[3]          frame 0: a colour; frames 1,2: −1,−1,−1
```

The mood constants come from the pack's `Media\Moods\<Mood>\Mood.MoodSetting.xml`
(`<T3LightMap MaxHDR BounceFactor SkyFactor SkyUseClouds/>`, `<LAmbient
HdrColor>`, `<LDirSun HdrColor>`, `Latitude`, `DayTime01`).

### 3.3 The `SHmsLightMapCacheMapping` struct (version 9)

```text
u32 9
u32 1
u32 2048, u32 2048          atlas layout size (the images are 1024²)
f32[3] bboxMin, f32[3] bboxMax   world box of the lit objects
u32 1
u32 N                       chart count
ztable z0                   f32 per chart: −1.0 everywhere
ztable z1                   per chart { u32 objIdxInObject | flags<<24 (0x10 seen), u32 object×4 }  ("ObjBinds")
ztable z2                   per chart { u16 x, u16 y }   layout position
ztable z3                   per chart { u16 w, u16 h }   layout size
u32 0
ztable z4                   { u32 3, 3 × { u32 N, u8[N] } }  one byte per chart PER FRAME
u32 1, f32 (0.5–1.5), u32 0, u32 0
```

`ztable` = `u32 uncompressedSize, u32 compressedSize, zlib bytes`. The tables
are sorted by object. `raw_z` in the parser keeps the stored streams so a
round-trip is byte-identical; any edit recompresses (miniz level 9).

### 3.4 What the per-chart frame byte is **[GAME]**

`fb0` is the chart's **maximum HDR value on a linear scale**: the colour atlas
holds each chart normalised so its brightest channel is ≈255, and the pixel is
multiplied back by `fb0` at load. Measured: items with fb0 = 64 render at
0.37× and fb0 = 224 at 1.5× the brightness of fb0 = 148 (linear prediction
0.43× / 1.51×; the low end is compressed by the tone mapper). fb1/fb2 are the
same per-frame scale for the point-light frames (0 = the chart gets nothing
from that frame). The absolute scale K (fb0 = 255·max/K) is fitted against
Nadeo's bakes (§5).

### 3.5 The grey atlas (frame 0, image 1) **[GAME]**

R=G=B everywhere; 128 where the colour atlas is black; 119 on open water at
sunrise; 40–240 with hard structure on complex items. Setting it to 0 or 255
on an item changes the item's brightness by <5 %: it is a **directional term**
for normal-mapped materials (the "Bump" of the cache name), not light. 128 is
a safe neutral value when authoring.

### 3.6 Binds

`object×4` is a byte offset into a 4-byte-per-object table the game builds
at load (hence the ×4 and the 0x4000/0x10000 bases: 4096 ground slots, 16384
for a Stadium decoration's own list). The first word's low 24 bits index the
chart within the object; bit 28 (0x10) marks 15 % of the charts on Nadeo's
maps (none on tiny bakes; meaning unknown).

#### The object base (`--base auto`) **[FILE]**

Objects are [decoration][authored blocks][the game's generated blocks][items];
the item base is

```text
base = P + N_authored + (S_x·S_z − replaced) + G
```

* P = 16384 on the Stadium decorations (48x48Screen155*: objects 0..3 are the
  decoration's own charts; NoStadium48x48*: none), 0 on the terrain
  collections;
* N_authored = the unbaked blocks (free ones included; embedded
  `…_CustomBlock`s count nothing — the tiny 05's 46 free water tiles);
* S_x·S_z ground tiles generated for the map grid — the baked records in the
  file do NOT count (01 ×2 carries 1886 Sea records: base 128² exactly; a tiny
  build's 2412 fill records: 4096); on a terrain collection an authored block
  replaces its column's tile (17 ×2, 04 ×2: one kept block → S²), on Stadium it
  does not (25 ×2: 16384 + 1 + 96² + 7);
* G = the other generated pieces: the Screen155 stadium's 432 apron pieces
  (the empty tiny 05: 2736 = 2304 + 432), the pillars/walls grown under
  elevated Stadium blocks (7 under 25 ×2's kept platform, 2108 around 05 ×2's
  604 pool tiles) — map-specific; `--base-extra N` or an editor bake for such
  maps (`lmtool bake` warns).

A Nadeo map's baked chunk IS the game's generated set, so there base = P +
unbaked + baked (25/25 sources). Measured by the giant child's `lmtool
itembase` and by `lmtool basecheck` (max charted object − items; both
columns printed).

### 3.7 Validation at load **[INFERRED]**

A stored lightmap whose object space does not fit the scene (the stale source
chunk on a block-deleted map: 10088 objects vs 4096+items) is rejected and a
coarse lightmap is recomputed at load (the old §2 observation); a synthesized
chunk with N = 4096 + items, our own layout, copied small chunks and trailer
is accepted **[GAME]** — no hash over the images or tables is checked.

### 3.8 Trailer (after `FACADE01`, 6.5–30 KB): the probe volume **[FILE]** / **[GAME]**

Decoded 2026-09-22 (`lightmap/src/volume.rs` reads and writes it byte for
byte on every source and editor bake). It is a **volume of light probes**:
16 m cells in 480 m world slots, each occupied slot a "block" whose occupied
cell range is stored level by level as tiles of the small third atlas
(frame 0 image 2 — the atlas earlier called "foliage/sprite"). The probes
light the dynamic objects (the car), the water and the ambient/specular term
of the surfaces (painting probe image 0 green tints the road and the sea
**[GAME]**).

```text
u32 91                       version / magic
u32 1024 ×4                  (constant)
u32 30, 5, 0, 0, 6           (constant)
3 × { f32 scale, u32 end }   per probe image 0..2: its value scale and the END byte
                             offset of its WEBP inside the third-atlas blob (§3.9)
u32 g0, g1, g2               label grid: 32·cols, 16·rows, 32 with cols = ceil(√(n/2))
u32 nblocks
nblocks × 60 B               { u32 origin[3] (label cell of the block: 32·col, 16·row, 0);
                               u32 min[3], max[3] (occupied cell range in label space,
                               2 cells of margin each side, clipped to the block);
                               f32 cell[3] = 16,16,16; f32 pos[3] (world offset) }
u32 npairs                   = Σ (max[1] − min[1])
npairs × { u32 x, u32 y }    per block, per height level: the level's tile position in
                             the third atlas, or (−1, −1) = not stored (the two margin
                             levels under the geometry). Tile = (max0−min0) × (max2−min2)
                             pixels, one probe per pixel, rows = z, plan view
u32 ncell4; ncell4 × u16     per 4×4 atlas cell a 16-bit mask, bit (y%4)·4 + x%4: 0 = the
                             probe is INSIDE geometry (dark; not interpolated), 1 otherwise
u32 5, 3, 5                  slot grid (i, j, k): 480 m × 224 m × 480 m from the origin
u32 30, 14, 30               usable cells per slot (block minus the 2-cell overlap)
u32 32, 16, 32               block size in cells
f32 1/480, 1/224, 1/480      1 / slot pitch;  f32 −0, 0.1696, −0
u32 75; 75 × i32             slot table: block index per slot i + 5j + 15k, −1 = empty
u32 a, b, 0, 0, 0            two counts (unknown; copied from a template, harmless)
u32 6, f32 1.0, u8[128] 0    ×2 (constant; the second copy sometimes absent);  u32 5
```

Probe positions **[GAME, verified with two probe bakes: blobs at known
places → their own blocks]**: x, z = pos + 16·(label + ½), y = pos.y +
16·(label − ½). With the grid origin O (below) and the slot (i, j, k):
pos = O + (480 i, 224 j, 480 k) − 8 − 16·(label origin), so the probes sit at
x = O.x + 480 i + 16 cx, y = O.y + 224 j + 16 (L − 1), z = O.z + 480 k + 16 cz
with cx, cz ∈ [0, 32), L ∈ [0, 16) — the last two cells of a slot duplicate
the next slot's first two (the overlap; 30/14/30 usable). The slot table
index is i + n.x·j + n.x·n.y·k (checked on all 25 sources, j = 1 rows and the
Stadium 5×2×4 grids included). The occupied range is the bounding box of the
16 m cells that contain geometry, +2 cells each side clipped to the block;
levels below the first occupied one are `(−1, −1)`, two levels above it are
stored; a j = 1 block continues from level 0 without a margin.

**The slot grid follows the map** (`probes::SlotGrid::for_map`; read off
the 25 sources and the giant editor bakes 01 ×2 / 04 ×2 (128³ BlueBay), 05 ×2 /
25 ×2 (96³ NoStadium), 17 ×2 (96³ RedIsland), 25 ×3 (128³), 25 ×4 (192³) —
2026-09-23) **[FILE]**:

* origin O per decoration — BlueBay/GreenCoast (0, −38, 0), RedIsland/
  WhiteShore (0, −118, 0), Stadium 48x48Screen155 (−304, −62, 0), NoStadium
  (0, −62, 0): the terrain's bottom in y, the stands' reach in x (`unk_f` =
  −O / pitch). x/z drop to floor(min/16)·16 when lit geometry starts below 0
  (25 ×2: −16); y drops to O.y − 512 when lit geometry lies far below the
  terrain (the giant builds park stock trees at y −900 → −574 — the volume
  does not reach them, their cells are clamped into the bottom row);
* counts n = ceil((max(map grid, lit geometry) − O) / pitch) with the map
  grid = size words × (32, 8, 32) m: 64³ → 5×3×5; Stadium 48×40×48 → 5×2×4
  (x: (1536 + 304 + the far stands) / 480 → 5); 128³ → 9×5×9 (405 slots —
  the 5 rows come from the 1024 m of grid, the items stop at 394 m); 96³ →
  7×4×7; 96³ with the −574 floor → 7×6×7;
* while n.x·n.y·n.z > 512 the **cell doubles**: pitch (960, 448, 960), origin
  − cell/2 so the coarse probes sit on every other fine one, block records
  with `cell` = 32: 25 ×3 (128³ with the deep items: 9×8×9 = 648 at 16 m) →
  5×4×5 at origin (−24, −582, −24); 25 ×4 (192³) → 7×5×7. `lmtool bake`
  reproduces every grid above (`lmtool volcmp EDITOR OURS`: same counts,
  origin, cell and label grid; 47/47 blocks on 25 ×3, 98/99 on 01 ×2 — the
  missing one holds a stock item we have no mesh for).

### 3.9 The third atlas: four WEBPs **[FILE]** / **[GAME]**

Frame 0 image 2 is not one image: it is a **concatenation of four WEBP
files** of the same size, `frame_info[k].end` marking where image k ends
(the fourth runs to the blob's end). Any re-encoding of the blob as ONE
image crashes the client (it reads the next image at the stored offset).

| # | content | scale |
|---|---|---|
| 0 | probe colour: sky + sun irradiance at the probe (sky-tinted, black inside geometry) | `frame_info[0].scale` = max |
| 1 | occlusion: 255 in the open, soft 0 near/inside geometry (greyscale) | `frame_info[1].scale` (0.4–0.9 in the files) |
| 2 | a pale bluish colour image, grey (not black) inside geometry — role unknown | `frame_info[2].scale` ≈ 0.18–0.77 |
| 3 | the point lights' irradiance at the probe (white/cyan glints at lamps and checkpoints) | none stored |

The game accepts lossy VP8 (Nadeo's form) for all four; VP8L also decodes
(the crash attributed to VP8L earlier was the concatenation).

### 3.10 Frame 1 = the items' point lights **[GAME]**

Frame 1 image 0 holds, per chart, the irradiance of the items'
`CPlugLight`s (`CPlugSolid2Model.lights`: socket transform, GxLightSpot
colour/intensity/radius, cone angles) — pools of light on the road under
the border spots, cyan under the checkpoint rings. Fitting the tiny-16
editor bake against our light list: **the cone angles are half-angles** (a
spot "120/170" lights a near-hemisphere; treating them as full angles
kills the correlation), the falloff is a smooth window in d/R ((1−x²)²
fits as well as any), value ≈ 0.27 · I · colour · n·l · att · spot (K = 1
units, per-texel r² 0.17 — VP8 blur and texel misalignment cap it). Frame
2 image 0 is a greyscale copy of the light pools (mostly black); left black
with fb2 = 0 (accepted by the game).

## 4. Authoring **[GAME]**

`lmtool synth MAP --template T --out OUT` writes a complete chunk from
scratch: 4096 flat ground charts + one chart per item (own shelf packing,
1-px gutters and borders), lossless WEBP atlases, frame table / small chunks /
trailer / sprite atlas from the template (same mood), `binds` = object
4096 + i. Play mode renders it (every item flat-lit, no rejection, 42 KB
chunk). `lmtool paint` recolours the charts of a real bake in place (the
red/green/blue-by-index proof). `lmtool bake` computes sky + sun (+ bounce)
irradiance per texel from the map's own item geometry (§5).

## 5. The baker (`lmtool bake`) **[GAME]**

```text
lmtool bake TINY.Map.Gbx --out OUT.Map.Gbx [--mood auto] [--vp8 8] [--templates DIR]
```

`--mood auto` (the default) reads the header's collection and mood, takes
the lighting row from `lightmap/src/moods.rs` and the template chunk
`<Collection>-<Mood>.lmchunk` from the template bank (tm-player/tiny/
lightmap-re/templates/; the bank holds one chunk per collection × mood the
campaign uses, extracted from an editor bake or from the Nadeo source). The
explicit form (`--template T --sun-az … --sky …`) still works.

Geometry: every embedded item's Solid2 visuals at LOD 0 with TexCoord1,
placed like the game places them (`mapgeom::place::anchored`); a binned-SAH
BVH over all triangles (24.7 M on Tiny 16), built in parallel
(deterministic). Per texel of every chart (chart = the item's TexCoord1
bounds → a rect sized by `PreLightGen.u02 × 0.5625 × uv extent`, v = 0 at
the top row): cosine-weighted sky visibility, sun visibility over a small
disc, then

```text
E  = ambient + up·(0.5 + 0.5·n.y) + sky·skyVis + sun·max(0, n·L)·sunVis      (frame 0)
E1 = Σ lights  0.27 · I · colour · max(0, n·l) · (1 − (d/R)²)² · spot · vis     (frame 1)
```

the chart is flood-filled to its edges, normalised to its max, fb = 255·max
(K = 1). The probe volume (§3.8) is built from the same BVH: slots and
occupied cells from the geometry, one probe per 16 m cell (spherical sky
visibility, sun visibility, the point lights, an inside test by back-face
rays), four VP8 images, the mask, the slot table. Big atlases as VP8
(`--vp8 Q`, Nadeo's form, ~0.5 MB) or VP8L without the flag.

The per-mood rows (`lmtool moodfit MAP [--shadow]` fits the sun direction
by per-texel correlation or shadow agreement over the largest charts, then
the four colour terms per channel by least squares against the map's own
editor bake):

```text
collection  mood     conf     az     el   ambient              up                   sky                  sun
BlueBay     Sunrise  fitted   77.5  45.0  0.232,0.206,0.211  0.086,0.097,0.128  0.114,0.126,0.169  0.131,0.087,0.073
BlueBay     Day      fitted  230.0  35.0  0.306,0.361,0.368  0.090,0.116,0.195  0.048,0.058,0.112  0.030,0.034,0.048
RedIsland   Day      fitted   95.0  52.5  0.275,0.197,0.198  0.087,0.119,0.171  0.010,0.010,0.012  0.092,0.069,0.060
Stadium     Day      fitted  197.5  35.0  0.247,0.271,0.278  0.189,0.151,0.174  0.031,0.043,0.036  0.045,0.043,0.042
WhiteShore  Day      derived 230.0  35.0  (BlueBay Day; identical pack XML, SkyFactor 0.5)
GreenCoast  Day      derived 230.0  35.0  (BlueBay Day; identical pack XML)
RedIsland   Sunrise  derived  77.5  45.0  (BlueBay Sunrise × the XML LDirSun ratio)
WhiteShore  Sunset   derived 282.5  20.0  (BlueBay Sunrise × the XML LAmbient/LDirSun ratios, sun mirrored west and low)
```

"fitted" = against an editor q=4 bake of a tiny map of that collection and
mood; "derived" = no editor bake exists (the lightmapper crashes or saves
nothing on the WhiteShore/GreenCoast tiny copies, 2026-09-12 and -22), the
row is the nearest fitted one scaled by the pack's `Mood.MoodSetting.xml`
colour ratios. The Day bakes are nearly shadowless (sun term ≈ 0.03–0.09),
so their fitted directions are weak (the objective is flat); the Sunrise
direction is stable across three estimators. The pack XML's `Latitude` +
`DayTime01` reproduce the Stadium Day elevation with el = asin(cos φ ·
cos(360°·(t − ½))) (34.9° vs 35° fitted) but not BlueBay Sunrise (69° vs
45°) — the 64×64 decorations' moods carry their own time.

Acceptance (tm-player/tiny/lightmap-re/acceptance-20260922/, play-mode
intro frames, editor bake | ours, luminance RMSE per frame with the HUD
strip skipped — the numbers include the camera's sub-frame timing offset):
Tiny 11 BlueBay Day RMSE 13–43 (mean lum 123.6 | 113.0; 143.4 | 140.6),
Tiny 16 BlueBay Sunrise 16–27 (70.1 | 68.8), Tiny 02 RedIsland Day 21–27
(73.8 | 79.2), Tiny 05 Stadium Day 3.5–31 (91.4 | 91.5). Every map of the
five collections × the campaign's moods renders (03, 04, 13, 17 through the
derived rows). Tiny 16: 39 s end to end on the devserver (8615 items).

Not done: the decoration meshes (cliffs/sea) cast no shadow on items; probe
image 2's meaning; the Stadium slot anchor; editor references for the
derived moods.

## 6. What the game does with it (earlier observations, still valid) **[GAME]**

* Rejects a stored lightmap that does not fit the scene and recomputes a coarse
  one at load; the editor's `SaveMap` drops embedded `.dds`, so real bakes are
  transplanted (`chunkswap`, `tinyctl lightmap`).
* `ComputeShadows` reuses `C:\ProgramData\Trackmania\Cache\<64hex>_<16hex>_<Collection>_<mood>.Bump.LightMap.zip`
  by map CONTENT (a fresh uid is still served from the cache).
* Per-item: quality byte chunk `0x03043068`; `CGameItemModel.DisableLightmap`
  (`0x2E00202A`); TexCoord1 = the lightmap chart; a visual without TexCoord1
  is not drawn; `PreLightGen` u02 = metres per uv unit.
