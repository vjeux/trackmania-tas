# Summer 2026 - 01, half-scale experiment

`Summer-2026-01-Tiny.Map.Gbx` is generated from the official Summer 2026 - 01 map (`buNzfsVlp2NF2oWtHM3729dEylg`, source MD5 `1563b24baad901364e4f86ce76b3f8f6`).

The route is rebuilt at 0.5 scale around a new elevated anchor `(1024, 300, 1024)`. The output uses a distinct UID, embeds the required half-scale Item.Gbx models, parks the original authored blocks and unused items below the map, and explicitly rebuilds Spawn/Checkpoint/Goal item tags.

Generated with:

```text
tools/target/release/tmmaps tiny Summer-2026-01.Map.Gbx \
  --out Summer-2026-01-Tiny.Map.Gbx \
  --mapping tools/tmmaps/tiny/roadtech-half.tsv \
  --library tools/tmmaps/tiny/roadtech-half-items.zip \
  --scale 0.5 \
  --anchor 1024,300,1024
```

Structural verification:

- output MD5: `5d8e8fd66acd62b04841b541b3edc876`
- output UID: `TinybuNzfsVlp2NF2oWtHM3729d`
- 39 active route items: 36 mapped block placements plus three waypoint fallbacks/preserved item waypoints
- embedded item ZIP: 3,370,799 bytes
- parser reload verifies every generated model, position, scale, and waypoint tag
- `cargo test -p tmmaps`: 9 tests passed

The current library has exact mappings for the common RoadTech pieces established from the original and tiny U10S campaign pairs. `RoadTechCurve5`, `RoadTechOnLandHillSlopeBase`, and `RoadTechBranchCross` are explicit stand-ins in `roadtech-half.tsv`; the real client must visually/collision-check and revalidate this first build before it is published or treated as race-ready.
