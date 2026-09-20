# colon three

**A 7.6-second map published two weeks ago. The author time falls by 0.037,
0.078 under a world record that was set the same day this was filmed — and the
whole gain is in the first second and a half.**

**colon three** — TAS **7.590** — **the author time, beaten** (−0.037) | AT 7.627 | WR 7.668 by Matik_K

https://github.com/user-attachments/assets/d972d194-ba7d-4b2e-af62-d60c928364ad

*Single car: the 7.590, with its own inputs drawn on.*

https://github.com/user-attachments/assets/6117d8b6-9f1f-4174-b36a-5ac6a91377b7

*Two cars: the same 7.590 against Matik_K's 7.668 world record, the camera on
the TAS. The gap is 0.078 — two car lengths at the finish arch.*

The author time is beaten, by **0.037**. The human world record is beaten by
**0.078**.

| | time | vs TAS | vs AT |
|---|---:|---:|---:|
| TAS | 7.590 | — | −0.037 |
| TAS, earlier tape | 7.626 | +0.036 | −0.001 |
| TAS, first tape | 7.627 | +0.037 | ±0 |
| Author time (never beaten by a human) | 7.627 | +0.037 | — |
| Human WR (Matik_K, 2026-09-19) | 7.668 | +0.078 | +0.041 |
| 2nd (in-.-, keyboard) | 7.670 | +0.080 | +0.043 |
| 3rd (Epilogue.) | 7.678 | +0.088 | +0.051 |

TMX map [353522](https://trackmania.exchange/maps/353522) · author
**WalkingKat** · published 2026-09-03 · **38 recorded runs** (board
2026-09-19). The top of the board is tight — three players within 0.010 of
each other, all of them 0.041 or more off the author time.

## Where the time came from

The first two tapes (7.627, 7.626) were the keyboard world record's line with
its inputs re-timed tick by tick. The 7.590 is the same line with two things
the search had not been allowed to do before:

- **edit the first 0.2 s of the race.** The resume boundary excluded race
  0.00–0.20 s, so every earlier tape left the world record's launch untouched.
  Opening it is where the start pivot moved.
- **compound moves** — up to three input operations changed together instead
  of one at a time. Single-lever sweeps had saturated at 7.626; the
  0.036 came from moves no single lever reaches.

**The whole gain is the start pivot and the drift exit into the water.** The
water phase itself is unchanged in character — a slight steer into the right
wall, the same as every human at the top of the board.

## Keyboard (low-input) tapes

The 7.590 is an analog tape — steering takes any value, and the search moved
it a unit at a time. A human plays this map on a keyboard: steer is **−127, 0
or +127**, and a finger cannot hold a key for less than a few frames. So a
second search ran with that alphabet and a **minimum hold** applied to every
candidate before scoring (`tmsearch --alphabet kb --minhold N`), seeded from
the 7.590's geometry snapped to keyboard (the snap itself is a 7.777), then
`tmsearch thin` deleted presses one at a time through the plain oracle,
keeping everything that stayed at or under 7.626.

| min hold | best time | input changes | thinned |
|---:|---:|---:|---|
| 30 ms | 7.619 | 41 | 24 changes → 7.624 |
| 50 ms | 7.621 | 32 | 27 → 7.626 |
| 70 ms | **7.610** | 25 | **22 → 7.617** |

Every row beats the author time on a keyboard. 100 ms holds were not reached:
the legalised seed no longer finishes. **The longer hold found the deeper
basin** — 70 ms → 7.610 against 30 ms → 7.619 from the same seed in the same
time — fewer degrees of freedom searched better.

https://github.com/user-attachments/assets/86f7aa5e-e132-4fb2-a25f-2649a0e79599

*The keyboard 7.610 (holds ≥ 70 ms, 25 input changes), with its inputs drawn
on: every steer value is full lock or nothing.*

**The 22-change tape (7.617), in plain words** (race seconds; the exact ticks
are `inputs/tas_kb70_thin22_7617_film_v1.inputs.csv`):

- brake, gas and **full left** held from the countdown, with a 70 ms straighten
  across the start line (−0.01 → 0.06);
- **0.25** release the brake; **0.39** brake again;
- **1.41** tap RIGHT for 90 ms; **1.50** back to full left; **1.53** release
  the brake — this is the pivot, and it is the only place the brake is used
  in the race;
- **2.39** straight;
- then the drift into the water, steered with taps: right 2.73–2.80, left
  2.87–2.94, right 2.94–3.03, right 3.21–3.28, **right 3.43–3.68** (the one
  long hold), right 3.75–3.85, left 3.93–4.00, right 4.00–4.15;
- from **4.15** to the finish: gas, wheel straight, nothing else.

Both keyboard tapes are published under `replays/` and `inputs/` (below); the
7.617 is the one to practise.

## Files

| file | what |
|---|---|
| `replays/tas_7590_film_v1.Ghost.Gbx` | **the run**, regenerated for film — md5 `117ebc403510dda11241e98add2335e8` |
| `replays/tas_7626_film_v1.Ghost.Gbx` | the earlier tape, one under the author time — md5 `a7a4aaf4b96639019f166115998d7f7b` |
| `replays/tas_7627_film_v1.Ghost.Gbx` | the first tape, level with the author time — md5 `774bfe10014e9803d7bf65fd5334ba1f` |
| `replays/tas_kb70_7610_film_v1.Ghost.Gbx` | **keyboard**, holds ≥ 70 ms, 25 input changes — 7.610 — md5 `d73d8ccac4ef04564aee3260971bb6a5` |
| `replays/tas_kb70_thin22_7617_film_v1.Ghost.Gbx` | keyboard, thinned to 22 input changes — 7.617, **the one to practise** — md5 `0907455f6f8513e3020a6e43b2127d11` |
| `inputs/tas_7590_film_v1.inputs.csv` | the run's per-tick inputs, `race_ms,steer,accel,brake` |
| `inputs/tas_7626_film_v1.inputs.csv` | the 7.626's per-tick inputs |
| `inputs/tas_7627_film_v1.inputs.csv` | the 7.627's per-tick inputs |
| `inputs/tas_kb70_7610_film_v1.inputs.csv` | the keyboard 7.610's per-tick inputs (steer is only −127 / 0 / +127) |
| `inputs/tas_kb70_thin22_7617_film_v1.inputs.csv` | the 22-change 7.617's per-tick inputs |

### What these recordings are

Each file above is a search output (`tas_7590_v1.Ghost.Gbx` md5
`79e3ec08ec74e8cfcf125ed91fe64efe`, `tas_7626_v1.Ghost.Gbx` md5
`191b3418373dd177ce4147b2ee051536`, `tas_7627_v1.Ghost.Gbx` md5
`edfb61040972469f3c5fcfce96eff228`, `tas_kb70_7610_v1.Ghost.Gbx` md5
`879a8aa3a939e8dc6c5f4fed36bb0242`, `tas_kb70_thin22_7617_v1.Ghost.Gbx` md5
`01690a6212ff52279b423006eb37ee9a`; none is redistributed here) regenerated
for the client: the car's telemetry was read out of the engine driving that
tape, regridded to one sample every 50 ms spanning 0.000 .. the declared time,
with the tape's own steer / gas / brake bytes on every sample. Tape/record
agreement kappa 1.000 over every sample on each; the plain oracle re-simulates
each written file to its declared time (**7.590**, 7.626, 7.627, 7.610,
7.617); identity `TAS` (skin `TAS.zip`, no account id). The 7.590, the 7.626
and both keyboard tapes were seeded from the keyboard world record (in-.-,
7.670) and sit in that record's container; the 7.627 was seeded from Matik_K's
7.668 and sits in his.

The clips are the game's own MediaTracker renders at 1080p30 — of the 7.590
(and in the two-car clip of Matik_K's downloaded record beside it) and of the
keyboard 7.610 — trimmed to the run and with the tape's inputs drawn on;
nothing in the picture is composited. The camera is the stock external chase on the TAS car.
