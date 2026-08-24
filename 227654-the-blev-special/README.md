# The Blev Special

**Nothing on this map is won in the final flight: the world record's eleven
respawns cost it 82 seconds, and the seven that are left are all in the
nine-second crawl into the corner at 47 s — get wedged there by 40.0 s instead
of 46.9 s and the author time falls.**

> ### The recording is fixed. `TAS_57482` carries its own run.
>
> Five files were withdrawn from this page because each carried ailiei.'s
> 147.031 trajectory — the human world record — rather than its own. That is
> repaired: **`replays/TAS_57482.Ghost.Gbx` is regenerated from the engine, and
> every position, orientation and speed in it is this run's.**
>
> It took working out why the map had defeated three previous attempts. The
> carrier here is a 27-player server replay, and this project's readers all take
> *the vehicle entity with the most samples* and call that the recording — which
> on this file reads "365 samples spanning 1.310 → 19.480 s" for a 57.482 s run,
> so 38 seconds of the race looked unrecordable. **The record is not truncated.
> It is one car split into 27 entities tiling 0 → 147.000 s end to end.** The
> repair lays down a single fresh entity on its own 50 ms grid — which also
> stops a render drawing the other 26 people's cars.
>
> *(Those entity boundaries were called "one per respawn" here. They are not,
> or not only: the engine moves this run's own car state between objects at
> **19.500 and 36.300 s**, on a tape with no respawn in it at all, and the
> container breaks its entities at 19.490 and 36.300. Two different runs, the
> same boundaries — so they are the MAP's. See below.)*
>
> The times were never in doubt: the oracle reads the input archive, and every
> tape re-simulates to the millisecond in its name. What was wrong was the
> recording. `HUMAN_WR_retries_cut_64871` stays as it is — it is published AS the
> human's lap with the respawns removed, so carrying his trajectory is the point.
>
> **And now the telemetry is its own as well** (2026-08-24). The file was
> regenerated again on the fixed carrier pipeline: wheel rotation, suspension
> travel, rpm, gear, ground contact and the packed reactor field are read out of
> engine memory per sample instead of being the donor container's constants —
> 112 of the 116 sample bytes are this run's, no byte is bit-identical to the
> donor throughout, and the trajectory did not move (0.000495 m mean against the
> file it replaces, which is the client-vs-server shadow floor). What it took is
> below: **this map changes the car under you, so the engine keeps the vehicle
> state in more than one object and the gather had to be taught to say so.**
>
> **Still no video, and the reason is now a measured one, not a missing
> machine.** The game CLIENT cannot import any ghost of this map whose record
> has been rebuilt: it dies on import, every time, in every variant tried.
> Twenty of them, one variable each, are in the table at the bottom of this
> page. The dedicated server does not care — it re-simulates the input chunk and
> never reads the scene — so nothing headless can see it, and every check in
> this project passes on a file the game will not open.

| run | time | vs author time | what it is |
|---|---|---|---|
| [`TAS_57482`](replays/TAS_57482.Ghost.Gbx) | **57.482** | **−0.371** | the record here, regenerated: reach the corner about 7 s early, then drive the human's own escape |
| `TAS_57493` *(no file)* | 57.493 | −0.360 | the same idea, 11 ms slower |
| `TAS_57573` *(no file)* | 57.573 | −0.280 | the first tape to beat the author time here |
| `TAS_59912` *(no file)* | 59.912 | +2.059 | the best keyboard-only run |
| [`HUMAN_WR_retries_cut_64871`](replays/HUMAN_WR_retries_cut_64871.Ghost.Gbx) | 64.871 | +7.018 | **the world record with its eleven respawns spliced out** |
| Author time | 57.853 | — | — |
| Human WR, as recorded | 147.031 | +89.178 | contains 11 respawns |
| Human #2 | 676.640 | — | — |

TMX map [227654](https://trackmania.exchange/maps/227654) · author **Blev..** ·
**2 recorded runs** · DesertCar / SnowCar / Bobsleigh.

## Read the gap correctly: 7 seconds, not 89

The leaderboard makes this look like a joke map — an 89-second gap between the
author time and the world record. It is not. The world record contains **eleven
respawns**. Take them out and the same human's own driving is **64.871**. That
is the number this map should be read against, and the real gap was **7.018**.

Eleven retries is not a sign of a bad driver, either. It is a sign of what the
end of this map asks for, which is the second half of this page.

## What the map does to you: nine seconds to travel eighty metres

At 37.75 s the car is at x = 1040 doing **198 km/h**. It then brakes, crawls at
20–50 km/h for nine seconds, and finally noses into a corner at 46.9 s. Once
there it is genuinely stuck:

```
47.000 - 51.750 s   x = 959.83 ± 0.01   y = 210.96 ± 0.02
                    speed 1.7 - 3.9 km/h, steer full left, gas on
                    sliding only in z: 577.86 -> 578.88, one metre in 4.75 s
```

That is nearly five seconds pinned against a wall with the throttle open. The
world record buries the car there and holds full left for 3.4 s before letting
go.

**Everything before that corner is thrown away** — once the car is wedged, its
whole state is one number, how far it has slid. So the entire margin on this map
is *how early you arrive*, not how well you drove the approach. To beat the
author time you need to be in the corner about seven seconds earlier than the
record: by **roughly 40.0 s instead of 46.9 s**.

## The run, by the clock

| window | what happens |
|---|---|
| 0 – 13 s | accelerate, launch at 617 km/h, land on the plateau at y ≈ 201 |
| 13 – 25 s | the record **fumbles** — wanders a 20 m loop at 25–100 km/h, about 8 s of nothing |
| 25 – 37 s | drive the plateau, one crash down to 12 km/h at 32 s |
| 38 – 52 s | the approach and the **wedge** — from 46.2 s pinned at 2 km/h for over five seconds |
| 52 – 58 s | escape, accelerate to 148 km/h |
| 58 – 64.9 s | enter a flat circular bowl at 130 km/h, one lap at full left lock spinning up to **670 km/h**, release, and fly 717 m to the finish |

## How to drive it

1. **You do not need a new trick. You need to stop crashing.** The record
   holder's own driving, uninterrupted, is 64.871 — and the eight seconds lost
   wandering between 13 and 25 s, plus the crash at 32 s, are ordinary mistakes
   rather than anything the map forces on you.
2. **Do not bury the car in the corner.** At around 47 s the record holder drives
   into the left wall with the gas on and holds full left for 3.4 s. Let go and
   steer right the moment the car stops moving.
3. **Arrive at that corner as early as you can.** Everything you save before it
   is kept in full; everything you do in it is not.
4. **The bowl at the end**: enter at about 130 km/h, hold full left, let the bowl
   spin you up to about 670 km/h over roughly a second and a half, and release.
5. All of the above is **keyboard**. Both recorded runs use three steering
   values and so does the 59.912 tape.

## How forgiving it is

Very, until it is not at all.

- **When you start the full-left hold in the bowl barely matters** — moving it
  most of a second either way gives the identical finish time.
- **The release out of the bowl is a one-to-few-tick window**, and it aims the
  entire 717 m flight: one tick moves the landing 30 to 60 m, and the near
  misses land one cell short of the finish. That single input is what the world
  record holder failed eleven times, and it is the only input on the map that is
  genuinely hard. Expect to fail it repeatedly; that is not you, that is the map.
- **The approach into the corner is forgiving in shape and unforgiving in time.**
  There are many ways to get wedged; what counts is the clock when you do.

## Where the fast tapes are, and are not

The fastest runs here are analog. The keyboard family reaches the bowl launch
six seconds ahead of the human and then dies on the flight arc, every time —
**59.912 is the best keyboard finish, and that appears to be the ceiling for
three steering values on this map** rather than a gap in what has been tried.
The reason is the same input: the launch release is a three-tick window in every
family, including both humans', so making the rest of the run simpler does not
touch the input that decides it.

## Files

| file | what |
|---|---|
| `replays/HUMAN_WR_retries_cut_64871.Ghost.Gbx` | the world record's own driving with the eleven respawns removed — published as his recording, which is what it is |
| `replays/TAS_57482.Ghost.Gbx` | **the fastest run on this map, and the only tape here whose recording is its own** — regenerated from engine state, span 0.000 → 57.482, one car in the file, and since 2026-08-24 its per-sample telemetry (wheels, suspension, rpm, gear, contact, reactor) is this run's too |
| `replays/TAS_57518.Ghost.Gbx` | the family's next tape — its telemetry is still the carrier's |
| `replays/TAS_57537.Ghost.Gbx`, `replays/TAS_57577.Ghost.Gbx` | the rest of the family — **one trajectory, not two runs** |

## The car is three objects, and that is why the telemetry took four attempts

`ghost regen … --carrier layout` reads the per-sample telemetry out of engine
memory by finding the copy of the vehicle state that IS the car — a copy whose
position matches the clean run's own measured path — and requiring its four
wheel-rotation slots to be alive. On every other map in this repo one copy is
the car for the whole run. Here there is no such copy, and the refusal said so
in a number that was bit-identical across twenty-nine attempts:

```
the chosen copy is 112.588863 m from the clean run's own path
```

Printing the whole distribution instead of the median is what broke it open:

```
offset per instant: p0 0.000  p10 0.000  p50 112.589  p90 868.794  p100 887.536 m

record+283472    4/4 wheels  on the car at  337 of 1159 instants  (19.500 .. 36.300 s)
record+287176    3/4 wheels  on the car at   17 of 1159 instants  (19.800 .. 35.800 s)
record+286120    1/4 wheels  on the car at  337 of 1159 instants  (19.500 .. 36.300 s)
record+1049556   0/4 wheels  on the car at 1159 of 1159 instants  (-0.150 .. 57.750 s)
```

The chosen copy is not *near* the car and not lagging it — it **is** the car,
exactly, for 337 instants, and then it stops being the car. Every copy with
live wheels tracks 19.500 → 36.300 s; every copy that holds the position for
the whole run has no wheel data at all. The map is **DesertCar / SnowCar /
Bobsleigh**: it changes the vehicle under you, each vehicle is its own
`CSceneVehicleVisState`, and 19.500 / 36.300 are the changes. They are the same
instants the container breaks its recorded entities at, on a tape with no
respawn in it — which is how we know the boundaries belong to the map.

Two fixes came out of it, both in the tools:

* **`ghost regen`'s step 0 was throwing the answer key away.** The grid rebuild
  stamped every sample with a copy of the car's FIRST sample, so a container
  that already held 1150 correct positions went to the engine as 1150 copies of
  the spawn point. Downstream, the locate's chooser abstains ("a constant
  trajectory identifies nothing"), the orientation veto reports "the container's
  own recording is NOT this run" — which reads as a transplant and here meant
  the recording had been deleted — and the field gather ranks copies of the car
  with nothing to rank them against. A rebuild now KEEPS the car's own samples
  when the car already covers the grid.
* **The field gather probes across the run and can stitch.** The candidate scan
  used one probe instant, so on a three-vehicle map it could only ever see the
  middle vehicle. It now probes at sixteen instants, prints every candidate with
  its live-wheel count and its "on the car" window, and — only where the
  single-copy answer is refused — selects the copy per instant, requiring the
  phases to tile the run and each phase to clear the same sub-millimetre and
  four-live-wheel bars inside its own window. A single-vehicle map is unaffected
  and the control for that is byte-identical output on the fixture.

What the file now carries, measured on the written file:

| channel | |
|---|---|
| wheel rotation 6 / 8 / 10 / 12 | LIVE (253–256 distinct values, changing on ~99.7 % of steps) |
| suspension travel 23 / 25 / 27 / 29 | LIVE (30–33 distinct, ~35 % of steps) |
| byte 89 ground contact | LIVE (4 distinct) |
| byte 90 reactor / booster air control | LIVE (3 distinct) — this map does have a reactor gate |
| byte 91 gear | LIVE (5 distinct) |
| unwritten, left as the container's | 11 channels: 19, 20, 34 and the four dirt slots (all read identically zero in the dedicated server), and 108–111, the countdown |

`ghost verify --engine`: V1–V10 pass, kappa **1.000** on 1150 samples, the
oracle re-simulates the written file to **57.482**, and the engine's own run of
the tape matches the recording to 0.0005 m mean / 0.0009 m worst.

## Why there is still no clip: the client will not import a rebuilt record here

The game client crashes — process gone, mid-import — on every ghost of this map
whose record has been rebuilt or re-cut, and imports the container's own
untouched 29-entity record every time. Each row below is one variable, each run
behind a fresh launch, all on 2026-08-24 on the render box:

| file | record | import |
|---|---|---|
| `HUMAN_WR_retries_cut_64871` | the container's own 29 entities | **imports** |
| the same container with our 57.482 tape in it | 29 entities | **imports** |
| …with the declared time set to 64.871 | 29 entities | **imports** |
| the human container with the declared time set to 57.482 | 29 entities | **imports** |
| the carrier trimmed to **140 / 100 / 80 / 68 / 61 / 60 s** | 29 entities, donor telemetry | **imports** |
| the carrier trimmed to **59 / 58 / 57.482 s** | donor telemetry, the tail entities dropped | CRASH |
| `TAS_57482` as published (before this regeneration) | 1 rebuilt entity | CRASH |
| the new regeneration | 1 rebuilt entity | CRASH |
| …with the container's scene records grafted back | 1 car + 2 scene | CRASH |
| …also with the car's 35 `delta2` blocks restored | 1 car + 2 scene | CRASH |
| …with the skin set to the container's `Stadium.zip` | 1 rebuilt entity | CRASH |
| …with the 82 notice lists stripped | 1 rebuilt entity | CRASH |
| …with the car's `u01` set to the container's first segment's | 1 rebuilt entity | CRASH |
| …extended to the container's own 147.030 span, car parked after the finish | 1 rebuilt entity | CRASH |
| …extended to 59 / 62 / 65 s the same way | 1 rebuilt entity | CRASH |
| a transform-only regeneration, nothing neutralised | 1 rebuilt entity | CRASH |
| the run re-cut into the container's own five segment boundaries | 5 car entities | CRASH |
| …with the notices stripped as well | 5 car entities | CRASH |
| the 62 s version re-cut into seven segments, with and without the scene records | 7 car entities | CRASH |

So it is not the neutralised bytes, not the skin, not the notices, not the
declared time, not `u01`, not the scene records, not the span on its own and not
the entity count on its own. **Two facts point somewhere.** The container
survives being trimmed all the way down to 60.000 s and dies at 59.000 — so a
record whose entity set has been edited is fine, and something about the last
second and a half is not. And OUR record dies at every span we tried, 57.482
through 147.030, in one entity or in seven. The variable those two leave
standing is the one nothing here has isolated yet: the SAMPLE BYTES of a
rebuilt entity against the ones the game wrote itself. The experiment is to
take the file that imports (the carrier trimmed to 60.000) and overwrite its
car samples with ours at the same instants — a `ghost record resample`, no new
research, and it separates "our bytes" from "our record" in one run.

**A trap worth knowing before repeating any of this**: after a crash the game
must be relaunched, and the next import into the corpse fails as
`{"err":"not in the MediaTracker"}` — a silent refusal, not a crash. Two of the
readings above read as "refused" first time and as "crash" once each was run
behind its own `launch --force`. A bisect that does not relaunch between rows
measures the previous row.
