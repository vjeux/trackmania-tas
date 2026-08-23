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
> It is one car split into 27 entities, one per respawn, tiling 0 → 147.000 s end
> to end.** The repair lays down a single fresh entity on its own 50 ms grid —
> which also stops a render drawing the other 26 people's cars.
>
> The times were never in doubt: the oracle reads the input archive, and every
> tape re-simulates to the millisecond in its name. What was wrong was the
> recording. `HUMAN_WR_retries_cut_64871` stays as it is — it is published AS the
> human's lap with the respawns removed, so carrying his trajectory is the point.
>
> **Still no video.** The render box became unavailable before this could be
> filmed.

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
| `replays/TAS_57482.Ghost.Gbx` | **the fastest run on this map, and the only tape here whose recording is its own** — regenerated from engine state, span 0.000 → 57.482, one car in the file |
| `replays/TAS_57518.Ghost.Gbx` | the family's next tape — its telemetry is still the carrier's |
| `replays/TAS_57537.Ghost.Gbx`, `replays/TAS_57577.Ghost.Gbx` | the rest of the family — **one trajectory, not two runs** |
