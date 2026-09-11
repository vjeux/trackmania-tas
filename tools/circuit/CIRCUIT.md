# circuit — a real-world race circuit as a 1:1 Trackmania 2020 map

*Written 2026-09-10/11 while building Silverstone. Everything below was run;
what was only inferred is marked.*

```
tools/circuit           the crate (one binary, `circuit`)

# the whole pipeline: data -> lap -> items -> .Map.Gbx (+ the lap as TSV)
circuit build raceway.json DATA_DIR intensity.tif HOST.Map.Gbx OUT.Map.Gbx \
        --surroundings surroundings.json [--cp 400] [--seg 100] [--half 16] \
        [--coarse 24 --fine 4] [--no-terrain] [--no-buildings] [--author-ms 148320]

# look before you build
circuit osm-loop raceway.json               the GP lap as BNG points, corner by corner
circuit preview raceway.json DATA OUT.png --intensity I.tif   lap over the LIDAR relief
circuit raster-png I.tif OUT.png E0,N0,E1,N1 --scale 4 --osm raceway.json --dtm DATA
                                            intensity crop with the detected edges drawn
circuit profiles raceway.json DATA I.tif 100,3100,4800      transverse intensity profiles
circuit map-head MAP / chunk-hex MAP 03043048 / header-hex MAP    what a .Map.Gbx says
circuit probe-item X.Item.Gbx                layers of a mesh-modeler item
```

## 1. Data — the most detailed open sources

| what | source | licence | how |
|---|---|---|---|
| lap outline, corner names, buildings, bridges, barriers | OpenStreetMap via Overpass (`highway=raceway`, `building`, `bridge`, `barrier`), nodes recursed with `(._;>;)` | ODbL | `curl -A '<real UA>' -H 'Accept: */*' -G --data-urlencode data@q.overpass https://overpass-api.de/api/interpreter` (406 without a UA) |
| ground height (1 m), building/stand heights | Environment Agency LIDAR Composite DTM 2022 and first-return DSM, Defra WCS, EPSG:27700 | OGL v3 | `spatialdata/lidar-composite-digital-terrain-model-dtm-1m-2022/wcs`, `GetCoverage&coverageId=13787b9a-…__Lidar_Composite_Elevation_DTM_1m&subset=E(a,b)&subset=N(c,d)&format=image/tiff` (axis labels are `E`/`N`; `x`/`y` answer 500) |
| tarmac edges, run-off shapes | National LIDAR Programme intensity 2019 (1 m, LZW TIFF) | OGL v3 | POST a bare GeoJSON polygon to `https://app.agrimetrics.co.uk/backend/catalog/api/tiles/collections/survey/search`, download `https://environment.data.gov.uk/tiles/collections/survey/national_lidar_programme_intensity/2019/1/SP6540?subscription-key=dspui` |

The point cloud (`national_lidar_programme_point_cloud/2019`) is there too
(LAZ) and was not needed: the 1 m intensity raster already separates asphalt
(returns 45..130) from paint, kerbs and grass (300..700) with sub-pixel edges.

WGS84 -> OSGB36/BNG is the OS Helmert + transverse Mercator (`geo.rs`, tested
against the OS worked example). The datum error (a few metres) was invisible:
the OSM centreline lies inside the dark tarmac band everywhere.

## 2. The lap (`osm.rs`, `track.rs`, `edges.rs`)

OSM draws a circuit as centreline ways broken at every junction and name. The
GP lap is the shortest route over the usable raceway graph through the named
corners in their known order (`GP_ORDER`), refusing ways named for other
layouts (Stowe Circuit, pit lanes, Bridge, Priory). **National Pit Straight
is part of the GP lap** (Woodcote -> Copse): forbidding it produced a 7.4 km
loop with 172 doubled points. Result: 5887 m against the official 5891.

Stations every metre: centripetal Catmull-Rom through the nodes, Gaussian
plan smoothing (sigma 3 m), re-sampled; height and cross-slope from a least
squares line through 9 DTM samples across (+-6 m), smoothed along the lap
(sigma 8 m / 12 m). Edges from the intensity profile at 0.25 m: core = median
within +-2.5 m, threshold = core + max(60, 0.8 core), first crossing at least
4 m out, gaps interpolated, 7-median, sigma 5 m. The bright band past the edge
is the kerb (capped at 4 m). Silverstone: width 11..23 m (mean 13.7), heights
144.85..156.19 m ASL, cross-slope +-3 %, tightest radius 18 m.

## 3. Items (`mesh.rs`, `mapbuild.rs`, `terrain.rs`, `buildings.rs`)

Everything is a Nadeo-style static item written with
`mapgeom::static_item` (`Merged` -> `bake::make_visuals` -> `assemble`):

- **Road**: one item per 100 m of lap, tarmac quad per station between the
  edges, kerb strips (TrackBorders) where the intensity saw a band, 0.4 m
  skirts. UVs: Nadeo's road atlases run ALONG the road in u (one tile per
  32 m) and ACROSS in v, lit band 0.06..0.94 — swap the axes and the deck
  renders as coloured stripes. Box-mapped materials get v squeezed into the
  same band (outside it the atlases are black: the first buildings came out
  black in play mode).
- **Waypoints**: start / finish / checkpoints are 32 m road pieces built in
  the item's own frame (local +z = travel; `yaw = atan2(dx, dz)`), with
  `waypoint_type` 0/1/2, a spawn 0.5 m above the tarmac, and a 12-triangle
  box trigger across the road (not for the start). Verified in play: the car
  spawns on the pit straight facing the lap, the HUD shows 0/14. Checkpoints
  every ~400 m, nudged onto the straightest station within 60 m; finish 25 m
  before the start line; the start in front of the pit building (OSM
  "Silverstone Wing" centroid projected on the lap).
- **Terrain**: two heightfield layers — 24 m cells over the whole grid, 4 m
  cells in a 40 m band around the lap — each cell asphalt (RoadTech, plain
  middle of the atlas) when most of four intensity samples are < 190, else
  Grass. The fine layer sits 6 cm above the coarse one.
- **Surroundings**: OSM building footprints extruded to the 90th-percentile
  first-return height inside the footprint (3..60 m), roofs ear-clipped;
  grandstands metal (Technics), houses/sheds TrackWall, the rest concrete.
  Bridges (OSM `bridge=yes`) as 4 m decks at the DSM clearance, fences/hedges
  within 60 m of the lap as low walls.

## 4. The map (`mapbuild::assemble`)

tmmaps patches a HOST map: uid, same-length name and author, medal times
(`set_times` rebuilds the header), the host's blocks parked at cell 0 and
renamed `RoadTechStraight` (a parked waypoint block is still a spawn), the
item array grown by cloning (`append_item_clones`), every slot re-pointed at
our idents, waypoint tags, and the embedded ZIP (deflated). Three write /
reload stages because renames move lookback tables and tags/zip are
variable-length.

**The 48x48 stadium is not a limit of the game, only of the editor's new-map
dialog.** The game reads the grid size from the file (the Big Decors plugin
only edits `DecoSize` in memory before `EditNewMap2`). But patching a 48x48
host up (size + one genealogy record per cell + one baked Grass per cell)
still answered "Couldn't load map!" — something else is per-cell. What works:
a **game-made big host** — TMX 135841 "Big Stadium Base - 64x64" by
KamiKalash: 120x120x120, decoration `NoStadium48x48Day` (no stands around
a real circuit), 77 items to clone, validated by the game. Other game-made
big hosts on TMX: 38343 (128^3), 80017 (255x200x255), 57142 (64^3 no
stadium), 164465 (64^3, 126 items).

Per-object arrays, MEASURED: `0x03043062` and `0x03043068` hold one byte per
block + baked + item; `0x03043063` and `0x03043065` one byte per ITEM only;
`0x03043069` one i32 per block + item. Growing 63/65 for baked blocks made
the map unloadable.

The dedicated server loads the finished map (`tmauto synth probe` gives
DNF(cps=0) on a null tape, as it should), so the TAS toolchain can drive it.

## 5. Render-box etiquette

`shootctl lock status` before ANY `quit`/`launch`; take the lock with
`lock acquire --owner …` and do the whole test in one command; other sessions
break locks they consider stale. Kill the game and release when done.
