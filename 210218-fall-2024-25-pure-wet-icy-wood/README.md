# Fall 2024 - 25 (Pure Wet Icy Wood)

**On wet icy wood the car is not steered, it is aimed: the whole leaderboard
drives permanently sideways, and the one player who stops doing that is faster
in eight of the last twelve sectors.**

**Fall 2024 - 25 (pure wet icy wood)** — TAS **96.068** (+1.591) | AT 94.477 | WR 96.281 by iambeeen

https://github.com/user-attachments/assets/5a22e94e-20ee-44eb-b8a1-f76042d0dc56

| run | time | vs author time | vs human WR |
|---|---|---|---|
| [`TAS_96068`](replays/TAS_96068.Ghost.Gbx) | **96.068** | +1.591 | **−0.213** |
| [`TAS_96078_1minimal`](replays/TAS_96078_1minimal.Ghost.Gbx) | 96.078 | +1.601 | −0.203 |
| Author time | 94.477 | — | −1.804 |
| Human WR — iambeeen | 96.281 | +1.804 | — |

TMX map [210218](https://trackmania.exchange/maps/210218) · author **R4igekon**
· 30 recorded runs.

**The author time still stands.** We are 1.591 short of it. The durable result
on this map is not our lap time — it is where the time is, and a human has
already driven all of it.

## The lap is already on the leaderboard, in pieces

The map has 17 checkpoints. Add up the best sector time anybody in the field has
driven and you get **91.826 — 2.651 under the author time**. Even after
discarding every sector that could have carried speed in from the one before it,
the assembled lap is **93.847, still 0.630 under**. Every one of those sector
times is a real clock reading from a human's own lap on this map.

**Nobody has put them together.** The world record is the fastest run in only 6
of the 17 sectors.

## What the field does wrong: it slides

One run tells the whole story. `r21` (SparkSheep) sits **21st** on the
leaderboard, because it loses 81.587 in sectors 1–5 to a driver stuck crawling
on a plateau. Over sectors 6–17 it is **3.195 faster than the world record** —
the fastest run in the field in 8 of those 12 sectors, on the same route, its
checkpoints within a metre or two of the record's at every gate.

The difference is slip. The field drives at roughly 20–30° of slip angle; over
sectors 6–17 r21 drives at **0.3–3.1°**.

| sector | WR slip | r21 slip | WR throttle | r21 throttle | WR mean speed | r21 mean speed | path Δ | r21 gain |
|---|---|---|---|---|---|---|---|---|
| 6 | 24.8° | **0.3°** | 100 % | 100 % | 73.4 m/s | **75.0** | −27.3 m | 0.457 |
| 12 | 10.4° | **3.1°** | **100 %** | **100 %** | 51.7 | **60.5** | **+4.6 m** | 0.574 |
| 13 | 23.5° | **3.1°** | 95.8 % | 100 % | 59.7 | **66.9** | −8.0 m | 0.715 |
| 14 | 28.6° | **2.0°** | 97.2 % | 100 % | 59.4 | **61.9** | +0.6 m | 0.230 |
| 15 | 16.1° | **0.7°** | **100 %** | **100 %** | 44.7 | **48.8** | −2.5 m | 0.514 |
| 16 | 34.1° | **0.5°** | 83.8 % | 100 % | 63.9 | **67.4** | −4.0 m | 0.269 |

**Sectors 12 and 15 are the clean case: identical full throttle, the same line
to within 8 m, and in sector 12 r21's path is 4.6 m *longer* — and it still
carries 17 % and 9 % more mean speed.** Neither throttle nor route explains it.
On this surface sliding scrubs speed, and that is the whole of it.

Sector 6, side by side from the same entry speed (221 against 226 km/h): the
world record swings up to **50 m/s sideways** and thrashes between full left and
full right lock; r21 stays inside **0.43 m/s** of straight with the wheel
centred. Over the run the world record uses 213 distinct steering values. r21's
input tape holds three — it is a keyboard run.

And the five biggest gains are all taken from an entry speed **equal to or lower
than** the world record's, so this is not a run inheriting speed from upstream.

## Sector by sector, off what you can see

The run descends x 1488 → 660 overall. "Wheel centred" means literally zero
steering input, which is what r21 holds most of the time.

| sector | where you are | what to do | gain |
|---|---|---|---|
| **6** | leaving the CP5 gate at ~220 km/h, long descent to z ≈ 500 | wheel centred between a handful of steering inputs, throttle pinned. **Do not catch the car — do not let it start sliding.** Exit at 341 km/h against the record's 308 | **0.457** |
| **7** | flat-out run to z ≈ 338, the fastest ground stretch (401 km/h) | sixteen input changes, full throttle throughout | 0.233 |
| **8** | braking zone into z ≈ 238 | **throttle for only 41 % of the sector** against the field's 57 % — lift earlier and longer | 0.423 |
| **10** | climbing back out, x 785 → 1005, first air | throttle 82 % — hold it *more* than the record does here (48 %) | 0.159 |
| **11** | the long one: x 1005 → 1265 climbing to y 122, 40 % airborne | the single biggest gain on the map. 47 changes, throttle 94 %. The record spends 122 changes fighting this and loses 0.881 | **0.881** |
| **12** | over the top, dropping y 122 → 82, 43 % airborne | **ten input changes for the whole sector.** Full throttle, a touch of brake | 0.574 |
| **13** | landing and running to x 1352, y 58 | 28 changes, full throttle | 0.715 |
| **14** | along z 755 → 971 at y ≈ 50 | sixteen changes | 0.230 |
| **15** | the slow hairpin back, x 1400 → 1244, down to 132 km/h at entry | 22 changes, full throttle | 0.514 |
| **16** | back along z ≈ 960 to x 985, up to 290 km/h | 36 changes, **full throttle where the record is on it only 84 %** | 0.269 |
| **17** | the finish: 58 % airborne, 506 km/h peak | four input changes. Nothing to do but hold it | — |

The general instruction is the same everywhere: **arrive with the car pointed
where it is going, hold the wheel still, and do not chase it.** The thing to
watch is how far sideways the car is travelling, not how fast it is turning.

A player chasing the author time needs sectors 6, 7, 10, 11, 12, 13, 14, 15 and
16 the way SparkSheep drives them in the 21st-place run, sector 8 the way
Sompig. drives it, and the rest from iambeeen's world record.

## How forgiving it is

Not at all, and that is the honest answer. **The low-slip line is about one unit
of steering wide.** Change a single tick of steering by one unit of 254 anywhere
in the run and the lap dies about two thirds of the time; on a keyboard tape it
is still 55 %. There is no slower-but-alive region on this surface — you are
either on the line or you are in the water.

That is also what the leaderboard is made of: what separates a 96-second lap
from a 440-second one is how many times you fall in, not how you drive. The top
five have zero respawns; the last has 34.

So the technique is **simpler to execute than what the field does** — three
steering values, two to three times fewer input changes per sector than the
world record — but it is not more forgiving. Fewer inputs here means less to
execute, not more slack per input. Anyone who has avoided this map because "you
have to catch the slides" has it backwards; anyone who expects the gripping line
to be comfortable will be swimming.

And respawning your way out of a mistake is not a route to the author time: the
cheapest finishing respawn over the last 2.7 s costs +1.787, against a gap of
1.601.

## Files

| file | what |
|---|---|
| `replays/TAS_96068.Ghost.Gbx` | **the fastest lap here, 0.213 under the world record** |
| `replays/TAS_96078_1minimal.Ghost.Gbx` | 96.078, the same line with the input list minimised |
