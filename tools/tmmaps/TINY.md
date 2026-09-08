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
5. DELETES the original authored blocks and the generated (baked) non-foundation fillers from the block chunks — `MapFile::remove_blocks` re-serialises 0x0304301F and 0x03043048 with the shared lookback table re-encoded, and rewrites everything that lists blocks (0x0304305F free positions, 0x03043062 colours, 0x03043068 lightmap quality, 0x03043069 macroblock refs, the snapped-on tables of 0x03043040); `TINY_KEEP_BAKED=Sea` (default) names the baked records that stay, `TINY_PARK_BLOCKS=1` restores the old way (every record moved to cell 0,0,0 and renamed to a neutral road);
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
