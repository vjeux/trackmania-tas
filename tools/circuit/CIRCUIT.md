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

## 5. The self-check (`check.rs`) — why the first video had a flying car

The first lap video showed the car leaving the ground 14 times (1.09 m at
Chapel Curve). The cause was not the road: 24 m terrain cells straddling the
13 m road had all four corners past the sink margin, and their flat plane
cut up to 2.3 m through the tarmac. A check on the source heights would not
have seen it; a check on the PLACED collision triangles did, at once.

Every build now walks the lap at 0.5 m across the tarmac and kerbs (~250 k
points) and refuses to write the map when any collidable triangle stands
more than 1 cm above the road (2 cm for another road meeting it flush),
when a tarmac point has no road under it, or when the vertical profile has
a crest sharp enough to launch a car at 60 m/s (`k > g/v²`). Extra roads get
the same surface test against their own drawn heights. `--allow-defects`
writes anyway, for looking at a broken map in the editor.

The terrain that passes it (`terrain.rs`): a 32 m grid over the whole map,
every cell within 30 m of a road replaced by its 4 m sub-cells — nested,
never overlapping — and every heightfield node within 7.5 m beyond a kerb
held at or below that road's plane minus 6 cm (`min(DTM, lid)`, pinned
outright under the road and its 1.5 m shoulder). 7.5 m because a 4 m cell
touching the shoulder reaches 5.7 m further: with every corner of every
such cell conformed, the interpolated ground under the kerbs IS the plane.
Where two roads at different heights run side by side (the pit exit is
1.5 m below Abbey on a bank), the lid is the LOWER road's, so the higher
one can never lift the ground over the lower one's kerb.

## 6. Edges where the LIDAR cannot see the line (`edges.rs`)

A flush asphalt run-off is as dark as the track. Three rules, in order:
the first painted line (a rise of 25 over the core that falls back by 15
within 3 m: Abbey's 55→98 bump at +5.5 m, with 4 m of 73..80 run-off
beyond it); failing that, past 9.5 m on one side, the largest step between
two asphalts (2 m means, at least 8 apart, both flat); and a side that
leaves its own ±60 m running median by 1.5 m while the width exceeds 16 m
is a run-off bulge, filled from its neighbours (Chapel exit: 60 m where the
rejoin asphalt is the track's own shade). Stations whose core reads above
150 are not tarmac (the Wellington footbridge deck) and are interpolated.
Result: 10.8..15.8 m over the lap, mean 13.3 — checked corner by corner
against aerial imagery (`circuit aerial`, ESRI World Imagery export in Web
Mercator, our edges drawn on it; the service refuses anything finer than
0.3 m/px and small boxes in EPSG:27700).

## 7. Everything else that is tarmac (`roads.rs`)

Every OSM `highway=raceway` way the lap does not use (both pit lanes, the
Stowe circuit and its pits, the old Bridge and Priory loops, the Porsche
centre's handling roads, ~40 link roads), except unpaved ones, is an open
road: its own intensity edges, built longest first and clipped against
everything built before it (its centre on an earlier road: slice left out;
its edge reaching one: capped 0.3 m short). Where the cap meets the other
road at the same level (< 0.35 m) the tarmac is contiguous: heights blend
onto the other road's plane over the last 7.5 m and neither side gets a
shoulder — the pit exit meets Abbey flush. Where the levels differ it keeps
its own heights and a skirt. Start and finish come from OSM's
`raceway=start` / `raceway=finish` nodes (0.4 m and 0.2 m from the
centreline; Silverstone's finish line is 150 m upstream of its start line).
Barriers within 3 m of the lap or 0.3 m of any road are dropped (OSM draws
fences across spectator crossings and the pit wall along the pit lane's
painted apron); a building any road runs through starts 5 m up.

## 8. Render-box etiquette

`shootctl lock status` before ANY `quit`/`launch`; take the lock with
`lock acquire --owner …` and do the whole test in one command; other sessions
break locks they consider stale. Kill the game and release when done.

## 9. A second layout on the same venue, read off imagery (`kart.rs`, `imagery.rs`)

Kart Silverstone's Grand Prix layout (1377 m, 18 corners, opened 2025 on the
old Bridge–Priory infield) post-dates every open survey, so its geometry
cannot come from the LIDAR. What is used instead:

- **Topology from OSM**: eleven `sport=karting` ways, twelve junctions. The
  lap is an explicit junction-node sequence (`kart::GP_NODES`, via
  `osm::loop_from_nodes`), settled by reading three agreeing pictures: the
  venue's own outline, Kart Directory's track map (GP in blue, the other
  layouts' cut-throughs in grey, the start/finish tick on the main straight)
  and motorsport-timing.co.uk's sector map (Sector 1 leaves the start towards
  the loop = north-east on the main straight — the direction). Length alone
  could not pick it: several simple cycles measure 1358–1422 m.
- **Geometry from Google's zoom-20 tiles** (`aerial-box --src google`; ESRI
  still showed the track under construction). `imagery.rs` resamples the
  tiles onto a 0.1 m BNG grid and classifies pixels: asphalt is a warm grey
  (r ≥ g ≥ b, g−b ≤ 16), grass olive (g ≥ r−3, g−b ≥ 15), kerbs white
  (max > 185, low saturation), dirt warm and desaturated, barriers and
  shadows dark. `read_edge` walks out from the centre and stops at the first
  ≥ 0.5 m run of non-asphalt — a dark run only when grass, white or dirt
  follows within 1.5 m (a shadow across the road is not an edge); the white
  run at the edge is the kerb. `kart::refine` reads both edges, moves every
  station whose width is plausible for one lane (5.2–12.5 m) to their
  midpoint, smooths the shift over 3 m and rebuilds, three times: 1390 m of
  OSM polyline became 1382 m of lap (official 1377), shift rms 1.1 m on the
  first round, 0.2 m on the third. `circuit kart-trace` draws the result on
  the imagery — look at it (centre white, edges green/red, kerbs yellow,
  stations that did not vote magenta).
- **Ground from the DTM**, smoothed over 15 m: the site was graded.
- **The venue as scenery**: `roads::extra_roads_with` builds the F1 lap as
  the first extra road (closed, its own intensity edges), then every other
  raceway way, then the unused karting runs and the pit lane with imagery
  edges (`kart::unused_karting_runs`). Two rules that the kart junctions
  needed: a station whose centre lies on an earlier road still has its edges
  capped (the slice INTO it otherwise carries the full width and the road's
  own height across the lap — a 1.4 m ramp), and an imagery road always
  blends onto the earlier road's plane, 4 cm low (the DTM predates the
  regrading; the ribbon plane is a 2 m lattice a few cm off the surface).
  The terrain classifier defers to the imagery within 50 m of the kart track.
- **The car**: `tmmaps::MapFile::set_player_model("CarSnow", 10003, "Nadeo")`
  writes body chunk `0x0304300D` and the header's `<playermodel id=…/>`,
  spelled as TMX 141984 (a game-made SnowCar map) spells it. The chunk's two
  strings open the body's lookback table, so the blocks and baked chunks are
  re-encoded with it; the dedicated server loads the result and simulates
  that car (147 vs 185 km/h after the same full-gas start).
- Start/finish: the timing gantry read off the imagery (`kart::GANTRY`,
  BNG 467395/242268); the finish trigger on that line, the spawn 32 stations
  back on the grid. Checkpoints every ~150 m (8).

`circuit build-kart raceway.json DATA intensity.tif HOST OUT.Map.Gbx
--surroundings s.json --cp 150 --author-ms 72577`; result in
`silverstone-1to1/kart-gp/`.
