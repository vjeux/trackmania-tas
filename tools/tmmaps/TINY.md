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

`tmmaps tiny` performs the map-wide assembly from a block-to-item mapping and an embedded item archive:

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
2. replaces mapped authored blocks with half-scale custom item models;
3. preserves every waypoint explicitly, using a scaled built-in gate when the library has no mapped waypoint block;
4. parks the original authored blocks and unused items below the map;
5. embeds the supplied item archive;
6. assigns a distinct 27-byte map UID (`Tiny` + the first 23 source UID bytes);
7. reloads the written map and verifies every generated item model, position, scale, and waypoint tag.

The transformation is intentionally split into three write/reload stages. Item model renames change the GBX lookback table, while waypoint nodes and the embedded ZIP are variable-length records. Applying both against one set of offsets is unsafe.

## Current RoadTech library

`tiny/roadtech-half.tsv` covers the route vocabulary needed by Summer 2026 - 01. Six mappings are directly established across the U10S original/tiny pairs (`Start`, `Finish`, `Straight`, `Curve1`, `SpecialTurbo`, and `PlatformTechToRoadTech`). The remaining uncommon shapes are explicit approximations in the mapping file and should be replaced as exact half-scale items become available.

The generated map is therefore an inspectable first build, not a certified race map until the real client opens and revalidates it. `tmmaps` verifies the GBX container and the transformed placement data; it cannot verify pixels or collision geometry without the Trackmania client.
