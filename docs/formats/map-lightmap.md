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

### 3.7 Validation at load **[INFERRED]**

A stored lightmap whose object space does not fit the scene (the stale source
chunk on a block-deleted map: 10088 objects vs 4096+items) is rejected and a
coarse lightmap is recomputed at load (the old §2 observation); a synthesized
chunk with N = 4096 + items, our own layout, copied small chunks and trailer
is accepted **[GAME]** — no hash over the images or tables is checked.

### 3.8 Trailer (after `FACADE01`, 6.5–30 KB) **[FILE, undecoded]**

```text
u32 91
u32 1024 ×4                 the two big atlases' sizes
u32 30, u32 5, u32 0, u32 0, u32 6
3 × { f32, u32 }            per frame: (0.9–2.0, 8470–91876)
u32 a, u32 b, u32 32, u32 n (64/128, 48–112, 32, 6–28)
u32 0, 0, 0
n × 60-byte records         { u32×6, f32 16,16,16, f32[3] position, u32[3] }
then u16 pair tables …      ends with `u32 5`
```

Light-probe-like (positions in a 256-step grid); copied verbatim from a
template of the same mood when authoring, harmless so far **[GAME]**.

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
lmtool bake TINY.Map.Gbx --template EDITOR-BAKE-OF-SAME-MOOD.Map.Gbx --out OUT.Map.Gbx \
  --uv-bounds --sky-model 1 --tpm 1.0 --sky-samples 64 --sun-samples 4 --k 1.0 \
  --sun-az 75 --sun-el 45 --ambient 0.232,0.206,0.209 --up 0.097,0.107,0.140 \
  --sky 0.107,0.121,0.164 --sun 0.125,0.082,0.067          # Sunrise64 (Tiny 16) fit
```

Geometry: every embedded item's Solid2 visuals at LOD 0 with TexCoord1,
placed like the game places them (`mapgeom::place::anchored`); a binned-SAH
BVH over all triangles (24.7 M on Tiny 16). Per texel of every chart
(chart = the item's TexCoord1 bounds → a rect sized by `PreLightGen.u02 ×
0.5625 × uv extent`, v = 0 at the top row): cosine-weighted sky visibility,
sun visibility over a small disc, then

```text
E = ambient + up·(0.5 + 0.5·n.y) + sky·skyVis + sun·max(0, n·L)·sunVis
```

the chart is flood-filled (dilated) to its edges, normalised to its max,
fb0 = 255·max/K. The four colour terms are fitted per mood by least squares
against an editor bake of one map (`lmtool poolfit MAP --sun-az --sun-el
--regressor 1`; the sun direction by `lmtool sunfit2`, per-texel
correlation over the 400 largest charts). Tiny 16 (Sunrise64): az 75°, el
45°; the fit above; per-texel r² 0.22. In play the result is within a
hair of the editor's bake (compare-orig-bake3-bake2.jpg under
tm-player/tiny/lightmap-re/). 8619 items: ~15 s of baking on 166 cores
plus a 35 s single-threaded BVH build; the chunk is 1.1–1.4 MB (lossless).

Not baked yet: frame 1 (the items' `CPlugLight`s: checkpoint/lamp glow),
the foliage atlas and the trailer (copied from the template), the
decoration meshes (cliffs/sea cast no shadow on items).

## 6. What the game does with it (earlier observations, still valid) **[GAME]**

* Rejects a stored lightmap that does not fit the scene and recomputes a coarse
  one at load; the editor's `SaveMap` drops embedded `.dds`, so real bakes are
  transplanted (`chunkswap`, `tinyctl lightmap`).
* `ComputeShadows` reuses `C:\ProgramData\Trackmania\Cache\<64hex>_<16hex>_<Collection>_<mood>.Bump.LightMap.zip`
  by map CONTENT (a fresh uid is still served from the cache).
* Per-item: quality byte chunk `0x03043068`; `CGameItemModel.DisableLightmap`
  (`0x2E00202A`); TexCoord1 = the lightmap chart; a visual without TexCoord1
  is not drawn; `PreLightGen` u02 = metres per uv unit.
