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
| Lap ghost | `ghost/Silverstone.lap.Ghost.Gbx` — **127.702 s**, 14/14 checkpoints, validated by the dedicated server (author time) |
| Filmable ghost | `ghost/Silverstone.lap.filmable.Ghost.Gbx` — same inputs with the car telemetry regenerated from the engine; this is what the video shows |
| Video | [silverstone-1to1-lap-127702.mp4](https://github.com/vjeux/trackmania-tas/releases/download/videos-v1/silverstone-1to1-lap-127702.mp4) (1080p30, 2:10, controls overlay) |
| Nadeo | club campaign **Silverstone 1:1** (club 43788, campaign 155871), map id `b20f6ba7-bb58-470d-96a0-0062f0406dfd` — [campaign](https://trackmania.io/#/campaigns/43788/155871) · [leaderboard](https://trackmania.io/#/leaderboard/Silverstone1to14ae52765fea5) |

## The lap ghost

**127.702 s, 14/14 checkpoints, 0 respawns**, validated by the Nadeo dedicated
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

Medals: author 2:07.702 · gold 2:17.920 · silver 2:33.240 · bronze 3:11.550.

## How accurate is it

- The lap was checked against aerial imagery corner by corner (the ESRI
  World Imagery view of each corner with our detected edges drawn on it):
  the edges sit on the real white lines and kerbs everywhere the imagery is
  cloud-free; the OSM start line lands 0.4 m from our centreline. Widths
  10.8–15.8 m over the lap (mean 13.3 m). Where a flush asphalt run-off
  joins the track (Abbey, Chapel exit, Maggotts) the LIDAR could not see the
  white line; the edge there is taken from the painted-line ridge, the
  asphalt-to-asphalt step, or the neighbours — see `tools/circuit/CIRCUIT.md`.
- Every build runs a surface check over 250,000 points of the lap: nothing
  may stand above the tarmac or the kerbs, no holes, no crest sharp enough
  to launch a car at 60 m/s. The map is not written otherwise.
- Not modelled: barriers/debris fences (OSM has few), kerb colours (Stadium
  has one kerb material), painted markings other than the start/finish and
  checkpoint lines.

## The video

The whole lap from the stock chase camera, rendered by the game from the
filmable ghost, with the controls overlay (gas / brake / steer) at the bottom
left:

https://github.com/vjeux/trackmania-tas/releases/download/videos-v1/silverstone-1to1-lap-127702.mp4

(release asset on the `videos-v1` release, 65 MB, fetched anonymously → 200)

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
