# Silverstone Circuit, 1:1, in Trackmania 2020

The Grand Prix layout of Silverstone (5.89 km, 18 corners) rebuilt at true
scale as a Trackmania 2020 Stadium map, from open survey data rather than by
hand: the lap outline and the surroundings come from OpenStreetMap, the ground
(height, camber, the shape of every run-off) from the Environment Agency's
1 m LIDAR, and the tarmac edges from the LIDAR *intensity* raster, where
asphalt returns dark and paint, kerbs and grass return bright.

Every piece of it is a custom static item: the road ribbon with its kerb
strips, 14 checkpoints (one every ~400 m), a heightfield terrain classified
asphalt/grass, the pit building ("the Wing"), the grandstands, the bridges and
the village houses extruded to their surveyed heights. The map sits in a
game-made 120x120 block grid with no stadium around it — the 48x48 stadium is
only the editor's default, not a limit of the game.

| | |
|---|---|
| Map file | `Silverstone.Map.Gbx` (this folder) |
| Map uid | `Silverstone1to1c934b130bce2` |
| Lap | 5887 m, 14 checkpoints, start on the Hamilton straight in front of the Wing |
| Data | OSM (ODbL); EA LIDAR Composite DTM 2022, first-return DSM, National LIDAR Programme intensity 2019 (OGL v3) |
| Tool | [`tools/circuit`](../tools/circuit/CIRCUIT.md) — Rust, one binary, data in, `.Map.Gbx` out |
| Lap ghost | `ghost/Silverstone.lap.Ghost.Gbx` — **130.433 s**, 14/14 checkpoints, validated by the dedicated server (author time) |
| Filmable ghost | `ghost/Silverstone.lap.filmable.Ghost.Gbx` — same inputs with the car telemetry regenerated from the engine; this is what the video shows |
| Video | see below |
| Nadeo | club campaign **Silverstone 1:1** (club 43788, campaign 155871), map id `fad522e7-a4a9-446c-8093-a892f3f30102` — [campaign](https://trackmania.io/#/campaigns/43788/155871) · [leaderboard](https://trackmania.io/#/leaderboard/Silverstone1to12b6f959f2fe7) |

## The lap ghost

**130.433 s, 14/14 checkpoints, 0 respawns**, validated by the Nadeo dedicated
server on this exact map file. It is an unoptimized lap on purpose: a
closed-loop controller driving the surveyed centreline — pure pursuit with a
1 s lookahead, a `sqrt(20 m/s² / curvature)` speed plan capped at 55 m/s — on
the fork engine (`tools/search`, `tmexplore-real drive`), reading the car's
real state out of the paused dedicated server every 100 ms. 199–202 km/h on
the straights, Village 120 km/h, Vale 105 km/h (the slowest corner), never off
the tarmac. The driver's own note, the 10 ms input tape, the validation
transcript and the two ghost files are in [`ghost/`](ghost/) (`NOTE.md`).
Roughly 25–35 s is on the table by raising the speed cap and the cornering
allowance.

Medals: author 2:10.433 · gold 2:20.870 · silver 2:36.520 · bronze 3:15.650.

## The video

_(link)_

## Nadeo

Uploaded to Nadeo's map service (stored bytes byte-identical to
`Silverstone.Map.Gbx` here) and placed in the club campaign **Silverstone
1:1** of club 43788 (campaign 155871). In game: Live → Clubs → the club →
Campaigns → Silverstone 1:1.

## How it was made, in one paragraph

`circuit build raceway.json DATA intensity.tif HOST.Map.Gbx OUT.Map.Gbx
--surroundings surroundings.json --cp 400`: chain the OSM raceway ways
through the named corners into the GP lap; resample it every metre; take the
height and cross-slope from a transverse fit through the DTM; walk the
intensity profile across the tarmac at each station to find both edges and
the kerb bands; emit road items every 100 m and waypoint items with a spawn
and a trigger; classify a 4 m heightfield band and a 24 m whole-grid layer;
extrude the OSM footprints to their DSM height; then patch all of it into a
game-made big host with `tmmaps` (uid, name, author, medal times, cloned item
slots, waypoint tags, the embedded ZIP). The full write-up, with what did
not work, is in [`tools/circuit/CIRCUIT.md`](../tools/circuit/CIRCUIT.md).
