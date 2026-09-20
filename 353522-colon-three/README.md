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

## Files

| file | what |
|---|---|
| `replays/tas_7590_film_v1.Ghost.Gbx` | **the run**, regenerated for film — md5 `117ebc403510dda11241e98add2335e8` |
| `replays/tas_7626_film_v1.Ghost.Gbx` | the earlier tape, one under the author time — md5 `a7a4aaf4b96639019f166115998d7f7b` |
| `replays/tas_7627_film_v1.Ghost.Gbx` | the first tape, level with the author time — md5 `774bfe10014e9803d7bf65fd5334ba1f` |
| `inputs/tas_7590_film_v1.inputs.csv` | the run's per-tick inputs, `race_ms,steer,accel,brake` |
| `inputs/tas_7626_film_v1.inputs.csv` | the 7.626's per-tick inputs |
| `inputs/tas_7627_film_v1.inputs.csv` | the 7.627's per-tick inputs |

### What these recordings are

Each file above is a search output (`tas_7590_v1.Ghost.Gbx` md5
`79e3ec08ec74e8cfcf125ed91fe64efe`, `tas_7626_v1.Ghost.Gbx` md5
`191b3418373dd177ce4147b2ee051536`, `tas_7627_v1.Ghost.Gbx` md5
`edfb61040972469f3c5fcfce96eff228`; none is redistributed here) regenerated
for the client: the car's telemetry was read out of the engine driving that
tape, regridded to one sample every 50 ms spanning 0.000 .. the declared time,
with the tape's own steer / gas / brake bytes on every sample. Tape/record
agreement kappa 1.000 over every sample on each; the plain oracle re-simulates
each written file to its declared time (**7.590**, 7.626, 7.627); identity
`TAS` (skin `TAS.zip`, no account id). The 7.590 and 7.626 were seeded from
the keyboard world record (in-.-, 7.670) and sit in that record's container;
the 7.627 was seeded from Matik_K's 7.668 and sits in his.

Both clips are the game's own MediaTracker renders at 1080p30 — of the 7.590,
and in the two-car clip of Matik_K's downloaded record beside it — trimmed to
the run and with the tape's inputs drawn on; nothing in the picture is
composited. The camera is the stock external chase on the TAS car.
