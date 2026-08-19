# Get in the Hole ( Impossible ) — author time beaten, on a keyboard, with 19 key presses

| | time | vs AT | vs human WR | inputs |
|---|---|---|---|---|
| **TAS, unconstrained** | **13984 ms** | **−11** | **−34** | analog |
| **TAS, keyboard only** | **13985 ms** | **−10** | **−33** | **19 change events** |
| TAS, keyboard, 22 events | 13985 ms | −10 | −33 | 22 |
| TAS, keyboard, 15 events | 13990 ms | −5 | −28 | 15 |
| Author time (never beaten by a human) | 13995 ms | — | −23 | — |
| Human WR — in-.- | 14018 ms | +23 | — | — |

TMX map [203330](https://trackmania.exchange/maps/203330) · uid
`RL64wn0vFhuqHfKGLnMOql2SMaj` · **only 5 recorded runs**.

**Not submitted to any Nadeo leaderboard, and it never will be.**

## Why this map was the hardest target on the list

Five recorded runs. Two of them clip a wall, one overshoots the finish
entirely — so the "field" is effectively two finishing attempts. There is
almost no human knowledge to build on, which is why it was left until an agent
with proven methods could take it alongside the one already there.

The map: steering is **disabled** at the start by a `GateSpecial8mNoSteering`,
seven turbo blocks take the car to 810 km/h in 3 seconds, then a 3.5 s dive, a
redirect ramp, a scrubbing ground contact, and a **cannon** at 8.51 s that sets
the speed to exactly **1000 km/h** and fires the car down a 1370 m corridor.

At z = 976 a wall spans the corridor with **one empty cell** — x ∈ [160,192],
y ∈ [64,72]. That is the hole. Clear it, fall, land at z ≈ 1315, slide, and
cross the finish at z = 1507.

## Where the time actually is — and where it is not

Per-tick trajectories read out of the simulator, timed at fixed planes:

```
run              z=500     z=976    z=1200    z=1291    z=1400    z=1507
human WR        9894.5   11773.4   12695.8   13077.9   13542.3   14017.7
TAS 13985       9893.4   11771.4   12693.1   13074.9   13538.0   13980.4
```

**The entire flight is fixed to 2–3 ms.** Time at the hole varies by 2.0 ms
across a 33 ms spread of finish times. And the approach is not improvable
either: 15,000 unbiased random moves over the first 620 ticks produced **zero**
improvements, and forcing the brake off across the whole ground contact that
feeds the cannon changes the finish time by **+0 ms** — the brake is inert
there. The cannon outputs 999.8 km/h for every human and every candidate.

**34 of the 37 ms won came from the last 106 m.** The human world record lands
at x ≈ 182 and hits the lip of the finish platform at z ≈ 1472; its speed
collapses from 800 to 312 km/h and it still finishes at 14018. The TAS lands at
x ≈ 171–175 and **rides the same lip at 858 km/h**. That is the whole margin.

So the coaching point on the "impossible" map is not the hole, and not the
cannon, and not the dive. **It is where you land afterwards.**

## Validation

All five human records re-simulate to their exact leaderboard millisecond as
the identity control. Every banked tape re-validates through the plain oracle
against the untouched map — including all three keyboard tapes
(`notes/validation.txt`). No phantoms.

## Files

| file | what |
|---|---|
| `replays/kb330_19ev_13985.Ghost.Gbx` | **keyboard only, 19 input changes** — the one worth studying |
| `replays/an330_13984.Ghost.Gbx` | fastest run, unconstrained |
| `replays/kb330_22ev_13985.Ghost.Gbx`, `kb330_15ev_13990.Ghost.Gbx` | the rest of the low-input family |
| `inputs/*.tick.txt` | the keyboard runs as readable input scripts |
| `notes/NOTES.md`, `notes/PLAN.md` | measurements, including what the tools got wrong here |
