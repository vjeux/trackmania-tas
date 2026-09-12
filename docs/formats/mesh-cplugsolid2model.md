# The visual mesh: `CPlugSolid2Model` (`0x090BB000`), visuals, vertex streams

Typed reader/writer (byte-exact round trip on the 26 reference items; the
pack corpus of 7989 Solid2 / 48 320 visuals read in 6 s):
`tools/mapgeom/src/static_item/{solid2,visual,vstream,lod,merged,assemble}.rs`,
the crystal-era reader `tools/mapgeom/src/classes.rs`, rules mined by
`examples/s2rules.rs` and enforced by `mapgeom item-check` (rule ids `S2-*`,
`V-*`, `ST-*`, `C-*`, `M-*`, `SH-*`; `tools/mapgeom/FORMAT-RULES.md`).
Confidence **[FILE]** for layouts (GBX.NET's `.chunkl` checked against the
pack), **[VERIFIED-GAME]**/**[EXE]** where marked.

## 1. `CPlugSolid2Model` chunk `0x090BB000` (TM2020 writes version 34)

Fields in file order (GBX.NET `CPlugSolid2Model.chunkl`; `solid2.rs`):

```text
u32 version (34)
Id  u01
ShadedGeom[]  { i32 visualIndex, i32 materialIndex, i32 u01, [v≥1] i32 lodMask, [v≥32] i32 u02 }
[v≥6] i32 visualsDeprec, ref[] visuals              (CPlugVisualIndexedTriangles nodes)
Id[] materialIds
(when no custom materials) i32 materialsDeprec, ref[] materials   (external .Material.Gbx refs)
ref skel
f32[] lodMaxDist                                    (levels − 1 entries)
i32 visCstType                                      (1 on items; 2 on Flag.Mesh = tween-animated)
PreLightGen? { u32 version, i32 u01, f32 u02, bool u03, f32[8] u04, i32[2] spriteCount,
               [f32;6][] boxes, [v≥1] [i32;4][] uvGroups }      (present ⇔ visCstType 1)
u64 fileWriteTime, string u03, string materialsFolder ("Stadium\Media\Material\"), string u04
Light[] lights   { Id u01, bool u02, ref node (external CPlugLight when u02, else a string), string u04,
                   f32[12] iso, i32[6], bool u15, f32[3] }        — the light SOCKETS
[v<16] ref[] materialInsts
ref[] lightUserModels                               (CPlugLightUserModel 0x090F9000, the item-editor form)
(i32, i32)[] lightInsts                             (model, socket)
i32 damageZone, u32 flags, i32 u05, string u06, [v≥30] i32 u07
Material[] customMaterials  { string name; or empty name + ref node → inline CPlugMaterialUserInst }
[v 17..21] [f32;6][] boxes
Id[] joints, i32[] u10, i32 u11, i32[] u12, i32 u13, ref u14, f32 u15, f32 u16, Id u17, i32 u18, [i32;5][] u19
0x090BB002 (skippable)  kept raw (zeros on every reference item)
```

Constants the reference items share (`new_v34`): `flags 0`, `u05 1`, `u07 ±1`
(1 for inline-material items), `visCstType 1`, `0x090BB002` zeros; `u13` at
Solid2+0x2d8 = `0x80024108` on Flag.Mesh, `0x8001C779` on the pusher/rotor
meshes, 0 on statics (probed: no effect on the tween).

## 2. The visual: `CPlugVisualIndexedTriangles` (`0x0901E000`)

Chunks in file order: `CPlugVisual` `0x09006001/004/005/009/00B/00F/010`,
`CPlugVisual3D` `0x0902C002/004`, `CPlugVisualIndexed` `0x0906A001` with its
inline `CPlugIndexBuffer` (`0x09057000`, written with `WriteNode`: chunk ids
directly, no class id, no index).

* `0x0900600F` (v6): the vertex declaration / stream block — `u32 flags`, the
  TexCoord sets, the vertex COUNT, the stream refs, then the `u03/u04` block:
  head `0x30000`, size `40 + 4n`, 5 zero words, `x` (1..n), `n`, `n × 4-byte`
  entries; present ⇔ an Int32 id element (n4); identical across a model's
  visuals; `x`'s meaning open.
* `0x09006005`: the sub-visual table — per animation frame `{vertex base,
  index start, count}` (the flag cloth: 86 frames × 144 vertices = 12 384,
  one 726-index list). **Keeping it on a STATIC visual crashes the client at
  draw** (`0x140a9c174`, `r14 = NULL` frame table: the runtime visual of a
  static object has no table).
* `0x0902C004`: tangents — always present with two EMPTY arrays on pack
  statics; on the inline-vertex form it carries the arrays.
* `0x0902C002` / inline form: for a visual with NO vertex stream the vertices
  are inline: `pos f32×3, normal f32×3, colour f32×4` (chunk flags `0x38`).
  **The inline array is an ALTERNATIVE to a stream**: a reader that chose the
  40-byte inline form without checking for a stream ate 40 bytes per vertex of
  the next thing in the file (210218's ice-and-wood blocks).
* `0x0906A001`: the index buffer, `0x09057001` delta-`i16` coded; vertices are
  numbered by FIRST USE (48 610/48 610 pack visuals), every vertex referenced,
  no degenerate triangles. Index streams flagged absolute may still be delta
  (`0xFFFF = −1`).
* bbox = `[centre, half-extents]` tight to the positions.

## 3. `CPlugVertexStream` (`0x09056000`)

```text
u32 version, u32 count, u32 flags, ref baseStream,
(when the stream owns its data) Decl[] declarations, bool compressLocal3D,
one tightly packed array per declaration, in declaration order
Decl: u32 flags1 = name (bits 0..9) | type (bits 9..18) | stride in 4-byte words (bits 20..27) | space (bits 28..32)
      u32 flags2 (in-vertex byte offset in bits 2..12); when flags2 & 0xFFC != 0: u16, u16 offset
```

Names: Position 0, Normal 5, Color0 8, TexCoord0 10 (TexCoord1 = 11, …),
TangentU 18, TangentV 20. Types: Float1 0, Float2 1, Float3 2, Float4 3, Color
4, Int32 5, Dec3N 14. Spaces: Global3D 0, Local3D 1, Global2D 2. A `Float3` in
Local3D is stored packed as `Dec3N` (one word) when the compress bool is set;
positions are Global3D and stay full floats. Stride words measured: 40 B →
0xA, 44 → 0xB, 36 → 0x9, 28 → 0x7, 24 → 0x6 across all 26 reference items.
Element 4 as Int32 on BlueBay terrain = the per-vertex LAYER ID of the Techno3
`PyPxz_Ids` shaders (1 byte, or `hi:lo` on blend visuals where the vertex
colour's G byte picks lo < 128 or hi).

## 4. Rules the game enforces **[VERIFIED-GAME]** (each a `item-check` rule)

* **SH-01/02 — every visual of ONE material in an item must have the SAME
  vertex declaration.** The client merges same-material visuals into one draw
  with the FIRST visual's declaration and fetches elements by name; a later
  piece lacking an element (TangentU/V, names 0x12/0x14) is read through a
  NULL base at `Trackmania.exe+0x456c35` (crash) or draws missing geometry.
  The discriminator was an ORDER + SUBSET relation, not any per-visual field
  (memory `tm2020-static-item-format-rules.md`). Fix: `harmonize_layouts`
  (the union of declarations per material, synthesised colour `0xFFFF00FF`,
  uv1 = uv0, ids 0, tangent frame), geoms sorted by material.
* **SH-03** — a Deco-layout visual (position, normal, uv0, no tangents) under
  a `PlatformTech` / `Modifier\*\PlatformTech` material crashes the same site
  (the plastic loader reads TangentU); Nadeo's own `SpecialSlope2Up_Air` decks
  are authored without a tangent frame under those modifiers — seed
  TangentU/V (8 B/vertex).
* **A visual without a TexCoord1 set is NOT DRAWN under any valid material**
  (the same cards drew RED under an unknown model); TexCoord1 is the lightmap
  chart (`map-lightmap.md` §3).
* **A static item with 5 LOD levels crashes the client at load** (`ud2` at
  `+0x1e9947`, rax 5 r9 4): `MAX_LOD_LEVELS = 4`; pack static models never
  exceed 4 (the 5-level `Flag.Mesh.Gbx` is a dyna mesh).
* A custom-material DYNA mesh is capped somewhere under 12k vertices (draws
  no leaves).
* Never pin uv0 to a constant (zero uv derivatives); inline-converted streams
  use `flags 3`; pack tangent frames are mixed-handed — the shader takes N
  from the element, never `cross(tu, tv)`.

## 5. LOD and lightmap conventions

* `lodMaxDist.len() = levels − 1`: level k draws while distance < d[k], the
  last level unbounded; Nadeo's ladders are 64/128/256 (also 16/…, 26/…);
  `lodMask` is a bitmask (1 nearest, 2, 4 coarser; multi-bit masks 3, 7, f
  exist); an EMPTY last level is Nadeo's cull idiom (`Sparkler8m [16,128,256]`
  with masks ≤ 4 vanishes past 256 m). Geom order in the packs is level-major
  with unsorted materials inside a level. The client advances levels with
  camera distance × a LOD BIAS of ~×4 (a 32 m step fires between 100 and
  150 m). Emitting every level coplanar drew a LOD1 sheet over LOD0.
* A material's geoms share ONE declaration across all LODs (16 013/16 013
  groups); geom i → visual i, stream node = visual node + 1, material nodes
  consecutive after the last stream, no duplicate links.
* Lightmap atlas (TexCoord1): Granady's items use ONE atlas per item shared
  by all visuals, uniform scale (0.01892 uv/m on Road_17), charts never cross
  source groups, a uv0 seam or a dihedral ≥ 50° splits, margins 0.001; ours
  (`assign_lightmap_atlas`) 0 conflicts, 60–73 % coverage; `PreLightGen.u02 =
  1/texel-scale`, `u04 = uv1 bounds`. Merged items get one cell per PART
  (`repack_lightmap_parts`).
* Lights: `lights[]` sockets name an external `.Light.Gbx`; the bake reads it
  and embeds the `CPlugLight` INLINE with its GxLight (two node indices per
  light), radii × 0.5; the light's beam is along local −Z; inside a prefab the
  light's rotation composes with the entity chain's rotation INVERTED while
  its position follows the geometry (`prefab-and-dyna.md` §5).

## 6. Not known

* `u01`, `u03`, `u04`, `u10`–`u19`, `damageZone`, the `0x0900600F` `x` word.
* The exact per-model vertex inputs beyond what the decl census shows
  (`mapgeom --debug decls`).
