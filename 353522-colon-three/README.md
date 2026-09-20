# colon three

**A 7.6-second map published two weeks ago. The author time falls by a
millisecond, 0.042 under a world record that was set the same day this was
filmed.**

**colon three** — TAS **7.626** — **the author time, beaten** (−0.001) | AT 7.627 | WR 7.668 by Matik_K

https://github.com/user-attachments/assets/d52b0f0c-4036-4ba8-9af8-fedb4ce5a251

*Single car: the 7.626, with its own inputs drawn on.*

https://github.com/user-attachments/assets/0e373acc-8527-449b-911f-70d088353f20

*Two cars: the same 7.626 against Matik_K's 7.668 world record, the camera on
the TAS. The gap is 0.042 — one car length at the finish arch; for most of the
run the two cars share the same metre of road.*

The author time is beaten, by **0.001**. The human world record is beaten by
**0.042**.

| | time | vs TAS | vs AT |
|---|---:|---:|---:|
| TAS | 7.626 | — | −0.001 |
| TAS, the earlier tape | 7.627 | +0.001 | ±0 |
| Author time (never beaten by a human) | 7.627 | +0.001 | — |
| Human WR (Matik_K, 2026-09-19) | 7.668 | +0.042 | +0.041 |
| 2nd (in-.-, keyboard) | 7.670 | +0.044 | +0.043 |
| 3rd (Epilogue.) | 7.678 | +0.052 | +0.051 |

TMX map [353522](https://trackmania.exchange/maps/353522) · author
**WalkingKat** · published 2026-09-03 · **38 recorded runs** (board
2026-09-19). The top of the board is tight — three players within 0.010 of
each other, all of them 0.041 or more off the author time.

## Files

| file | what |
|---|---|
| `replays/tas_7626_film_v1.Ghost.Gbx` | **the run**, regenerated for film — md5 `a7a4aaf4b96639019f166115998d7f7b` |
| `replays/tas_7627_film_v1.Ghost.Gbx` | the earlier tape, level with the author time — md5 `774bfe10014e9803d7bf65fd5334ba1f` |
| `inputs/tas_7626_film_v1.inputs.csv` | the run's per-tick inputs, `race_ms,steer,accel,brake` |
| `inputs/tas_7627_film_v1.inputs.csv` | the 7.627's per-tick inputs |

### What these recordings are

Each file above is a search output (`tas_7626_v1.Ghost.Gbx` md5
`191b3418373dd177ce4147b2ee051536`, `tas_7627_v1.Ghost.Gbx` md5
`edfb61040972469f3c5fcfce96eff228`; neither is redistributed here) regenerated
for the client: the car's telemetry was read out of the engine driving that
tape, regridded to 153 samples every 50 ms spanning 0.000 .. the declared time,
with the tape's own steer / gas / brake bytes on every sample. Tape/record
agreement kappa 1.000 over all 153 samples on both; the plain oracle
re-simulates each written file to its declared time (**7.626** and 7.627);
identity `TAS` (skin `TAS.zip`, no account id). The 7.626 was seeded from the
keyboard world record (in-.-, 7.670) and sits in that record's container; the
7.627 was seeded from Matik_K's 7.668 and sits in his.

Both clips are the game's own MediaTracker renders at 1080p30 — of the 7.626,
and in the two-car clip of Matik_K's downloaded record beside it — trimmed to
the run and with the tape's inputs drawn on; nothing in the picture is
composited. The camera is the stock external chase on the TAS car.
