# CPlugCrystal model — report

Deliverables: `crystal-model.patch` (applies with `patch -p1` in `tools/`; verified
against the current `/home/vjeux/trackmania-tas-tiny/tools`, builds, round trip
2223/2223) and this report. `cargo build --release -p mapgeom` passes; the only
warning is the pre-existing `unused_mut` in `tiny_assets.rs:235` (untouched file).

## Files

| file | change |
|---|---|
| `mapgeom/src/crystal_model.rs` (new, ~1900 lines) | the complete `CPlugCrystal` / `CPlugMaterialUserInst` model: `parse`, `parse_with`, `write`, `locate`, `LookbackState`, `Rd`/`Wr` |
| `mapgeom/src/crystal.rs` | `build_item` / `build_item_with` / `decode_template` rebuilt on the model; new `edit_materials`, `ItemCrystal`, `KEEP_EXTRA_GEOMETRY`. Public API (`build_item`, `build_item_with`, `decode_template`, `dedupe_materials`, `CrystalMesh`, `Face`, `MaterialSpec`, `material_for_*`) unchanged — `tiny_assets.rs` and `main.rs` compile untouched |
| `mapgeom/src/classes.rs` | chunk 0x09003005 delegates to the model (all 17 layer types instead of Geometry/Trigger only); new reader for `CGameCommonItemEntityModelEdition` 0x2E026000 (was declared known but had no handler, so `Graph::parse` failed on every crystal item); 0x2E002019 reads its v13/v15 trailing node refs; the old 130-line `fn crystal` reader removed |
| `mapgeom/src/node.rs` | `Graph::noderef_sites: Vec<(offset, index)>` — every node-reference word the walk reads, for exact renumbering |
| `mapgeom/src/reader.rs` | `lb_version_seen()` / `set_lb_version_seen()` accessors |
| `mapgeom/src/lib.rs` | `pub mod crystal_model` |
| `mapgeom/examples/crystal_roundtrip_all.rs` | requirement 2: corpus round trip + survey |
| `mapgeom/examples/crystal_edit_check.rs` | `edit_materials` over the corpus (identity edit byte-exact, merge edit sane) |
| `mapgeom/examples/crystal_build_check.rs` | builds items through the public API for old-vs-new comparison |
| `mapgeom/examples/crystal_probe.rs`, `crystal_span_cmp.rs`, `body_dump.rs` | debugging aids |

## What is modelled (every field, per GBX.NET's `CPlugCrystal.cs` / `.chunkl`)

**Node `CPlugCrystal` (0x09003000)** — chunks kept in file order (`chunks: Vec<u32>`):
- `0x09051000` (CPlugTreeGenerator, inherited): one int.
- `0x09003000` (legacy one-layer form): version + `Crystal` — implemented, never seen in the corpus.
- `0x09003003` v2: `Vec<Material { name: String, node: Option<NodeRef<CPlugMaterialUserInst>> }>` — a named material has no node; an empty name is followed by a node ref (index + inline body the first time).
- `0x09003004` (skippable): `Chunk4 { version, data: Vec<u8>, u01: Option<i32> (v1+), trailing }` — kept raw as requested, re-emitted with PIKS + size.
- `0x09003005` v0: `Vec<Layer>`.
- `0x09003006`: `Lightmap::V0(Vec<[f32;2]>) | V1(Vec<[u16;2]>) | V2 { coords: Vec<[u16;2]>, indices: Vec<u32> }`.
- `0x09003007`: version, `smoothing_groups: Vec<f32>`, `per_face_ints: Vec<i32>`.

**`Layer`** = `LayerBase { version, crystal_enabled, layer_id: Id, layer_name, is_enabled (v1+) }` + `LayerKind`:

| type id | variant | fields |
|---|---|---|
| 0 | Geometry | `version`, `crystal: Crystal`, `u02: Vec<i32>` (one per group), `is_visible`, `collidable` (v1+) |
| 1 | SubdivideSmooth | `modifier`, `version`, `subdivisions` |
| 2 | Translation | `modifier`, `version`, `translation: [f32;3]` |
| 3 | Rotation | `modifier`, `version`, `rotation` (rad), `axis`, `independently` |
| 4 | Scale | `modifier`, `version`, `scale: [f32;3]`, `independently` |
| 5 | Mirror | `modifier`, `version`, `axis`, `distance`, `independently` |
| 6 | MoveToGround | `modifier`, `version`, `u01: bool` |
| 7 | Extrude | `modifier`, `version`, `size: [f32;3]` |
| 8 | Subdivide | `modifier`, `version`, `subdivisions` |
| 9 | Chaos | `modifier`, `version`, `min_distance`, `u01: i32`, `max_distance` (v1+) |
| 10 | Smooth | `modifier`, `version`, `factor`, `independently` |
| 11 | BorderTransition | `modifier`, `version`, `u01`, `u02: f32`, `visuals: Vec<NodeRef<OpaqueNode>>` |
| 12 | Deformation | `modifier`, `version`, `box_aligned: [f32;6]`, `iso4: [f32;12]` |
| 13 | Cubes | **not parsed** — `VoxelSpace` is `throw` in GBX.NET and no file in the corpus has one; the parser reports it as an error naming the offset |
| 14 | Trigger | `version`, `crystal: Crystal`, `u01: Vec<i32>` (v1+) |
| 15 | SpawnPosition | `modifier`, `version`, `position: [f32;3]`, `horizontal_angle`, `vertical_angle`, `roll_angle` (v1+) |
| 18 | Light | `modifier`, `version`, `lights: Vec<NodeRef<OpaqueNode>>`, `positions: Vec<LightPos { u01: i32, u02: iso4 }>` |

`Modifier { version, mask: Vec<PartInLayer { group_index, layer_id: Id }> }` (the `ModifierLayer` base).

**`Crystal` archive** (versions 21..37 per GBX.NET; every field of the reader, including the pre-37 forms): `version`, `u01` (v13+), `visual_levels: Vec<VisualLevel{u01:i32,u02:f32}>`, `anchor_infos: Vec<AnchorInfo{bool,bool,iso4,string,int}>` (v23+), `groups: Vec<Part{u01 (v31+), u02 (byte v36+/int), u03 parent, name, u04, u05 children}>` (v22+), `is_embedded` (+ the two extra copies below v29; byte from v34), `u02`/`u03` (v33+, = max face material / group index), `positions`, `edge_count` + `edges` (v35+: informational count then optimized-int pair array; earlier: plain int pairs), `tex_coords` (v37+), `faces: Vec<Face{verts, uv_index (v37+) | uvs (v<37), u01 normal (v<27), material (v25+), group}>`, `face_extra` (v<30 per-face int), `position_extra` (v<29), `u04`, the v7..31 crystal-link block (`u05`, `u06`, `old_smoothing`), the v<36 counted blocks + `u07`. Non-embedded crystals (`Crystal.Gbx`) and a CCrystalLink array > 0 are errors, as in GBX.NET; none occur in the corpus.

**`CPlugMaterialUserInst`** (inline node, chunks in file order): `0x090FD000` `MaterialMain { version, is_using_game_material (v11+ byte), material_name: Id, model: Id, base_texture, surface_physic_id, surface_gameplay_id (v10+), link (string when using a game material or v9–10, else id), csts: Vec<Cst{Id,Id,i32}> (v2+), color: Vec<i32>, uv_anims: Vec<UvAnim{Id,Id,f32,u64,Id (v5+)}> (v3+), u01: Vec<Id> (v4+), user_textures: Vec<UserTexture{i32,String}> (v6+), hiding_group: Id (v7+) }`; `0x090FD001` `MaterialTiling { version, atlas: NodeRef, tiling_u, tiling_v, texture_size_in_meters (v3+), u01 (v4+), is_natural (v5+) }` (v2 = GBX.NET throw → error); `0x090FD002` `(version, int)`.

**Lookback strings** (`Id`): `Null` (0xFFFFFFFF), `Str(s)` (a new table entry the first time, `0x40000000|n` afterwards), `Raw` (collection ids / other encodings), `Prior` (a back reference into a table the caller did not seed). `LookbackState { table, version_seen, defined_nodes }` is body-wide: `locate(body)` walks the `CGameItemModel` chunks up to the `MeshCrystal` (0x2E001009 … 0x2E002019 → 0x2E026000) so the crystal sees the item's ident strings at their true indices and the already-defined node indices (edition, crystal). Writing re-derives every encoding from the state, so a table that changes (e.g. a material with `id Link` removed) re-encodes the in-span references correctly.

**Node refs**: `NodeRef<T> { index, inline: Option<Box<T>> }`; an unknown inline class (a `CPlugBitmapAtlas`, `CPlugLightUserModel`, `CPlugVisual`) is kept as `OpaqueNode` bytes if it consists of skippable chunks, otherwise an error — never reached in the corpus (every atlas ref is −1, no Light/BorderTransition layers).

## Optimized ints — what the bytes need

Width = `count < 0xFF → u8, < 0xFFFF → u16, else u32` (GBX.NET's thresholds, not `<= 0xFF`):
`DecoWallTiltTransition1DownRight` has exactly 255 positions and writes its vertex indices as u16.

Which count:
- a lone index (face vertex, material, group) — the number of things indexed (positions / materials / groups);
- a length-prefixed array (tex-coord indices, lightmap indices, v35+ edge pairs) — **its own length**, exactly GBX.NET's `ReadArrayOptimizedInt()`. Evidence: `OpenDirtHillsShortCurve1In`'s collision layer has 4536 indices into 3 tex coords, written as u16. A coord-count rule fails 1814 items; the survey shows 1921 items whose tex-index array length and coord count fall in different bands (121 for the lightmap arrays), so the two rules are not interchangeable.

Old `crystal.rs` used the length rule for both arrays and the GBX.NET threshold; old `classes.rs` used `< 256` — the model settles it.

## Round trip (requirement 2)

`crystal_roundtrip_all /tmp/tiny-full/nadeo /tmp/Sheep.Item.Gbx`:

```
2223 items: 2223 pass, 0 fail
```

(2222 Nadeo items + Sheep; the crystal byte span from the first chunk id to and including FACADE is identical after parse → write, with the lookback table seeded by `locate`.) Corpus survey from the same run: chunk order always `[09051000, 09003003, 09003004, 09003005, 09003006, 09003007]`; every crystal is v37 with layers chunk v0 and lightmap v2; every material node is (v11, v5, v0); layer types present: Geometry 4227 (2202 items visible+collidable "Geometry", 2004 items also a "Geometry (Collisions)" layer visible=0 collidable=1, 20 items visible-only), Trigger 119, SpawnPosition 97. No named materials, no non-game materials, no edge arrays, no anchor infos; 209 items have ≥255 groups (u16 group indices). Lightmap indices always equal the corner count of the enabled+visible geometry layers, and the smoothing ints always equal their face count (2004 items prove the collision layer is excluded).

`Graph::parse` (the full body walk) now succeeds on all 2223 items (0 before the 0x2E026000 reader).

Failures encountered and fixed along the way (all in the width rules above): 1814 → 1 → 0.

## `build_item` on the model (requirement 3)

`ItemCrystal::open` → `locate` + `parse_with`; materials replaced by slots cloned from the template's first material node (chunk versions and unknown fields kept, link/physics set, `is_using_game_material = 1`) at node indices `first..first+n`; the FIRST Geometry layer's crystal filled from the mesh (template version/visual levels kept; groups = folder + "part" leaf; positions; `edge_count` = distinct edges, empty edge array; de-duplicated tex coords + per-corner indices; faces with group 1; `u02` = max material index, `u03` = 1; layer `u02` = one int per group; visible + collidable); lightmap atlas regenerated over every enabled+visible geometry layer's faces in layer order (non-overlapping cells, as before); smoothing ints = 2 per lit face; every non-Geometry layer (Trigger, SpawnPosition, modifiers, Light) written back as parsed; Trigger crystals' face material indices clamped into the new list (the old writer copied them verbatim and left dangling indices, e.g. 31 with 3 materials). Node renumbering: every node index past the old material range moves by the count delta — inside the model through `node_indices_mut` and in the body prefix/suffix at the `Graph::noderef_sites` the full walk recorded (pattern-scan fallback with a warning if the walk fails); header `num_nodes` follows. `set_header_ident` / `set_body_ident_nameless` as before.

Verification against the old writer (`crystal_build_check`, both builds): for RoadTechStraight, RoadTechStart, RoadTechCheckpoint, RoadTechFinish × {own mesh, 3 materials (node count shrinks), doubled materials (grows)} the files are **byte-identical** to the old implementation's, except Checkpoint/Finish × 3 materials where the only differences are the trigger faces' clamped material bytes (71 bytes, all `0x1F → 0x02` plus u02). `mapgeom crystal-roundtrip … --keep 31` reproduces the template's crystal span exactly; `--keep 16` keeps the collision layer.

**Deliberate deviation — extra Geometry layers.** The task lists "collision geometry" among the layers to keep. The main template (`RoadTechStraight`) and 2004 of 2222 Nadeo items carry a "Geometry (Collisions)" layer that is the *template's* collision shape; keeping it under a new mesh would give every generated item a phantom road-shaped collider. `build_item` therefore drops Geometry layers other than the first (as the old writer did, by accident) and writes the first one collidable; `build_item_with(.., KEEP_EXTRA_GEOMETRY /*16*/)` keeps them (face material indices clamped into the new list). `edit_materials` keeps everything. If you want the literal behaviour, flip the default of that one bit.

## `edit_materials` (requirement 4)

`edit_materials(item, |spec| …)`: parses the item, maps every slot's `(link, physics)`, merges slots equal by link (first-seen order; the survivor keeps its OWN node — gameplay id, tiling, every unknown field — and takes the physics of the slot covering the most faces, counted over every layer with a crystal), remaps faces in **every** Geometry and Trigger layer (collision layers included), recomputes each crystal's `u02`, renumbers nodes and `num_nodes` for the removed slots. `crystal_edit_check` over the corpus: identity edit reproduces the body **byte-exactly** for 2223/2223; the merge-to-two-links edit yields 2223/2223 items that re-parse, whose full graph walks, whose node count dropped by exactly the merged slots, whose faces all index the new list, and whose layer count is unchanged.

## Not verified / exceptions

- Cubes layers (`VoxelSpace`), non-embedded crystals, CCrystalLink arrays > 0, material chunk 001 v2: GBX.NET throws; none in the corpus; the parser errors with the offset.
- Inline `CPlugBitmapAtlas` / `CPlugLightUserModel` / `CPlugVisual` nodes: kept opaque only if made of skippable chunks; none in the corpus.
- Crystal versions < 37, lightmap v0/v1, chunk 0x09003000, named materials, materials with `id Link` (custom materials), modifier layers 1–12 and Light: implemented from the GBX.NET definitions but exercised by no Nadeo file, so untested against bytes.
- Lookback ids with the `0x80000000` flag are read like `0x40000000` ones and written as `0x40000000`; no corpus file uses the flag (proved by the round trip).
- Lookback strings the item body defines *after* the crystal are not part of the model; if a suffix chunk back-referenced a string defined inside the crystal span and the material edit changed the number of strings the span defines, that reference would go stale. Nadeo materials define no strings (all ids null), so no corpus item is affected.
- `build_item` on a community item whose body ident already has a name (Sheep) still panics in `set_body_ident_nameless` — pre-existing, outside the crystal.
- Only node-count deltas from the material list are renumbered; layer edits that add/remove inline nodes (lights) are not attempted.
