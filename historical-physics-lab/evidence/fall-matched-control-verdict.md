# Fall 2022 matched-control verdict — 2026-09-03

## Verdict

V5 is **not behaviorally equivalent** to exact Sep. 30, 2022. It is closer to current stock than to the historical reference. Production must remain fail-closed.

## Common control

All three runs used the same map bytes:

- `_hpl/HPL_Fall_Ice_Booster_Jump_Sep30_InBounds_SlotStable.Map.Gbx`
- SHA-256 `9add1183d9f71315e9ab45e5bdb3896ad089942f6daf13b021ab469bf0127974`
- 48×40×48 source bounds; edited road y=30 and gates y=29
- body lookback cardinality `57 -> 57`; item lookback cardinality `13 -> 13`
- native `RoadBumpStart`, four `GateCheckpoint`s, `GateFinish`, and `GateSpecialTurbo` preserved

Each run used approximately 22 seconds of full throttle with no steering and no brake. The game window was explicitly focused before input. Trace data independently confirms gas changed from 0 to 1 while steer and brake stayed 0.

## Captures

| Run | Executable / payload | Hold | Records | Trace SHA-256 |
|---|---|---:|---:|---|
| Exact Sep. 30 | exe `1f5ce987…fe1be` | 22.031 s | 9,553 | `ca3637601212043ed3ac330eab7e29bce2b6d329d3d9cb33fd05fd2a3a047c7d` |
| Current stock | exe `3fc7d8cd…6edda` | 22.010 s | 8,116 | `b2fc3fdd74ea1d3a6e4669fec696bfbc1ce844af3646697116bde3ba04909b39` |
| Current + V5 | same current exe; V5 package `eb65cec6…544d7` | 22.051 s | 7,616 | `b39244d2c3be01f8b05c9fe6b5a48553555432fc9a7bbb93f9728386f23ec3db` |

V5 activation was observed before map load:

```text
[HistoricalPhysics] ACTIVE Fall 2022 ... tunings=25/28
[HistoricalPhysicsHold] READY active=Fall2022 count=25 index=24
```

## Movement-aligned comparison

Alignment uses the first 10 ms simulation tick more than 0.05 m from the spawn position. First-write and last-write policies for duplicate callback ticks produce identical results.

| Pair | >1 mm | >1 cm | >10 cm | >1 m | Mean position error | Max position error |
|---|---:|---:|---:|---:|---:|---:|
| Exact vs stock | 1.330 s | 1.520 s | 1.940 s | 3.840 s | 6.109344 m | 10.016209 m |
| Exact vs V5 | 1.080 s | 1.520 s | 1.940 s | 3.700 s | 7.134936 m | 11.199561 m |
| Stock vs V5 | 1.080 s | 2.350 s | 3.450 s | 9.270 s | 1.356809 m | 2.454341 m |

A phase sweep over a fixed 2.000–18.000 s window, testing lags from −1.000 s to +1.000 s in 0.010 s steps, does not change the conclusion:

| Pair | Best lag | Best mean error | Best max error |
|---|---:|---:|---:|
| Exact vs stock | −0.090 s | 3.151423 m | 7.669566 m |
| Exact vs V5 | −0.100 s | 3.776477 m | 9.236947 m |
| Stock vs V5 | −0.010 s | 0.874798 m | 2.370423 m |

## Key metrics

| Run | Speed at aligned 10.000 s | Peak speed | Terminal forward position |
|---|---:|---:|---:|
| Exact Sep. 30 | 371.930 km/h | 398.520 km/h | 1045.052 m |
| Current stock | 377.790 km/h | 400.213 km/h | 1053.159 m |
| Current + V5 | 378.542 km/h | 400.348 km/h | 1054.730 m |

## Production disposition

- `PROFILE_FALL2022_BEHAVIOR_CERTIFIED` remains `false`.
- `PhysicsProfileId::StadiumFall2022` remains non-selectable.
- The 9,916-byte V5 payload is unchanged.
- The four-profile Stadium catalog is unchanged.
- No V6 was created.
