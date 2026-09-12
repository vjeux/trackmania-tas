# The mesh-modeler crystal: `CPlugCrystal` (`0x09003000`)

The editor's editable mesh, and the item form whose placement SCALE the game
honours. Complete typed model (byte-exact round trip on 2223/2223 items — the
2222 RuurdBijlsma `Nadeo.zip` items + Sheep): `tools/mapgeom/src/crystal_model.rs`;
the writer `tools/mapgeom/src/crystal.rs` (`build_item`, `edit_materials`);
report `tools/tmmaps/tiny/re/crystal-model-REPORT.md`. Definitions follow
GBX.NET's `CPlugCrystal.cs` / `.chunkl`. Confidence **[FILE]**; §5
**[VERIFIED-GAME]**.

## 1. Where it sits

```text
CGameItemModel 0x2E002000 → 0x2E002019 (Model, v≥8) → entityModelEdition:
  CGameCommonItemEntityModelEdition 0x2E026000 → CPlugCrystal 0x09003000
```

Every Nadeo.zip item is this kind (class `0x2E026000`); no pack Solid2 in
them, so they are useless as a Solid2 corpus.

## 2. Chunks (always in this order on the corpus)

| chunk | content |
|---|---|
| `0x09051000` | (CPlugTreeGenerator, inherited) one `i32` |
| `0x09003003` v2 | `Material[]`: `{ string name; when empty: ref node → CPlugMaterialUserInst }` — a NAMED material has no node |
| `0x09003004` (skippable) | `{ u32 version, u8[] data, [v≥1] i32, trailing }` kept raw |
| `0x09003005` v0 | `Layer[]` (§3) |
| `0x09003006` | lightmap UVs: `V0(Vec2[])` / `V1([u16;2][])` / `V2 { coords: [u16;2][], indices: u32[] }` — v2 everywhere; the index count = the corner count of the enabled + visible geometry layers |
| `0x09003007` | `u32 version, f32[] smoothingGroups, i32[] perFaceInts` — the ints equal the face count of the lit layers (the collision layer is excluded) |
| `0x09003000` | the legacy one-layer form: implemented, never seen |

## 3. Layers

`LayerBase { u32 version, u32 crystalEnabled, Id layerId ("Layer0"…), string
name, [v≥1] u32 isEnabled }` + the kind; modifier layers carry `Modifier {
version, PartInLayer[] mask { i32 groupIndex, Id layerId } }`.

| type | kind | fields |
|---|---|---|
| 0 | Geometry | `version, Crystal, i32[] u02 (one per group), bool isVisible, [v≥1] bool collidable` |
| 1 | SubdivideSmooth | modifier, `subdivisions` |
| 2 | Translation | modifier, `Vec3` |
| 3 | Rotation | modifier, `rotation (rad), axis, independently` |
| 4 | Scale | modifier, `Vec3 scale, independently` |
| 5 | Mirror | modifier, `axis, distance, independently` |
| 6 | MoveToGround | modifier, `bool` |
| 7 | Extrude | modifier, `Vec3 size` |
| 8 | Subdivide | modifier, `subdivisions` |
| 9 | Chaos | modifier, `minDistance, i32, [v≥1] maxDistance` |
| 10 | Smooth | modifier, `factor, independently` |
| 11 | BorderTransition | modifier, `u01, f32, ref[] visuals` |
| 12 | Deformation | modifier, `[f32;6] boxAligned, Iso4` |
| 13 | Cubes | NOT parsed (`VoxelSpace` throws in GBX.NET; none in the corpus) |
| 14 | Trigger | `version, Crystal, [v≥1] i32[]` |
| 15 | SpawnPosition | modifier, `Vec3 position, horizontalAngle, verticalAngle, [v≥1] rollAngle` |
| 18 | Light | modifier, `ref[] lights, LightPos[] { i32, Iso4 }` |

Corpus census: Geometry 4227 (2202 items a visible + collidable "Geometry",
2004 also a "Geometry (Collisions)" layer visible = 0 collidable = 1, 20
visible-only), Trigger 119, SpawnPosition 97; every crystal v37, layers chunk
v0, lightmap v2, every material node (v11, v5, v0); 209 items have ≥ 255 groups.

## 4. The `Crystal` archive (v37; every earlier form 21..36 implemented)

`u32 version, [v≥13] i32, VisualLevel[] { i32, f32 }, [v≥23] AnchorInfo[] {
bool, bool, Iso4, string, i32 }, [v≥22] Part[] groups { [v≥31] i32, u8|i32,
i32 parent, string name, i32, i32[] children }, u8|bool isEmbedded (+ two
extra copies below v29), [v≥33] i32 u02 (= max face material), i32 u03 (= max
group index), Vec3[] positions, [v≥35] i32 edgeCount + optimized-int pair array
(else plain int pairs), [v≥37] Vec2[] texCoords, Face[] { verts, [v≥37]
uvIndex | [v<37] uvs, [v<27] normal, [v≥25] material, group }, [v<30] per-face
int, [v<29] position extra, u04, the v7..31 crystal-link block, [v<36] counted
blocks + u07`. Non-embedded crystals and a `CCrystalLink` array > 0 are errors
(none in the corpus).

**Optimized ints**: width from a COUNT with `< 0xFF → u8, < 0xFFFF → u16, else
u32`. A lone index (a face's vertex/material/group index) is sized by the
number of things it indexes (`DecoWallTiltTransition1DownRight` has exactly
255 positions and writes u16 indices); a length-prefixed array (tex-coord
indices, lightmap indices, v35+ edge pairs) by ITS OWN length
(`OpenDirtHillsShortCurve1In`'s collision layer: 4536 indices into 3 tex
coords as u16). 1921 items fall in different bands under the two rules, so
they are not interchangeable.

Groups are a TREE: folders (name `""`, parent −1, children) and leaves
(`"part"`, parent = folder). **A leaf whose parent is itself hangs the game in
an infinite loading spinner** (process alive: ping the plugin, not tasklist).

## 5. What the game requires of a written crystal item **[VERIFIED-GAME]**

* The lightmap UVs (`0x09003006`) must be a NON-OVERLAPPING atlas (overlap ⇒
  a translucent bounding box); smoothing group 2 per face; UVs box-mapped, one
  `RoadTech` tile per 32 m, v in 0.06..0.94 (else black stripes).
* Collision surfaces are the trustworthy geometry source when converting
  (visual index streams mis-decode); collision winding is REVERSED relative to
  the crystal's.
* Materials are `CPlugMaterialUserInst` nodes with plain Link strings (pack
  path minus `.Material.Gbx`), gated by the environment whitelist
  (`materials.md` §4); later inline node indices shift with the material count
  (`Graph::noderef_sites` renumbers them; `numNodes` follows).
* Placement scale IS honoured (1.0 vs 0.5 side by side) — the only item kind
  where it is; nameless bodies (archive crystals) need a body ident before the
  game finds them (`set_body_ident_nameless`: name = NEW string, author = REF
  1, shift-free).
* Crystal virtual links resolve to real files at bake time:
  `Material\SpecialSignTurbo → Modifier\Turbo\Sign`, `SpecialSignOff →
  Modifier\Turbo\SignOff`, `DecalSpecialTurbo → Modifier\Turbo\Decal`;
  `SpecialFXTurbo` has no file and stays virtual.

## 6. Not known

* Cubes layers, non-embedded crystals, material chunk `0x090FD001` v2, the
  `0x80000000` lookback flag (all "GBX.NET throws"; none in the corpus).
* Crystal versions < 37 and lightmap v0/v1 against real bytes.
