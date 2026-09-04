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
5. parks the original authored blocks below the map;
6. embeds the supplied converted-item archive;
7. assigns a distinct 27-byte map UID (`Tiny` + the first 23 source UID bytes);
8. reloads the written map and verifies every generated item model, position, scale, and waypoint tag.

The transformation is intentionally split into three write/reload stages. Item model renames change the GBX lookback table, while waypoint nodes and the embedded ZIP are variable-length records. Applying both against one set of offsets is unsafe.

## Current Summer 01 status

Summer 2026 - 01 contains 2,430 authored blocks, 1,704 existing items, and 2,214 baked/generated foundation blocks. The public converted-Nadeo archive provides exact Item.Gbx models for only 23 of the map's 46 distinct authored block models. The remaining 23 are mostly BlueBay terrain (`Land*`, `Beach`, `LandHill*`, `LandCliff12`) plus several support/decor variants.

The command therefore **refuses** Summer 01 today rather than silently dropping half the map. The earlier route-only artifact is not a complete conversion and should not be used as one. Once the 23 missing items are exported from the game, adding them to the mapping/library is enough; the all-object writer and its item-array growth path are implemented and tested.
