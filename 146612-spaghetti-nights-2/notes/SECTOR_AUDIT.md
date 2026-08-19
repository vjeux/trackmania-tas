# 146612 — sector audit (agent `w612`, 2026-08-19, node 145855)

All splits below are **simulated on segment maps built from the untouched map**
(`tmmaps build --order 439,494,440,633,492`, all six exact against the reference
ghost), not read from ghost headers. Raw output: `w612_segsplits_v1.txt`.

> **`tmtas splits` cannot audit a synthesised tape.** It reads the ghost header,
> and every tape we have made carries its rank-2 template's telemetry: all six
> of our tapes report `race_time=40226` and rank 2's splits. An audit built on it
> is an audit of rank 2.

---

## 1. The finding that decides where the effort goes

**Every tape this project has produced on this map is rank 2, exactly, up to
checkpoint 4.** Not "similar to" — the same millisecond, at all four:

| tape | CP1 | CP2 | CP3 | CP4 | CP5 | finish |
|---|---|---|---|---|---|---|
| human rank 2 | 7.390 | 15.791 | 20.163 | 28.156 | 33.830 | 40.226 |
| `BEST_39961_v3` | 7.390 | 15.791 | 20.163 | 28.156 | 33.814 | **39.961** |
| `BEST_39973_v2` | 7.390 | 15.791 | 20.163 | 28.156 | 33.822 | 39.973 |
| `BEST_40040_v1` | 7.390 | 15.791 | 20.163 | 28.156 | 33.828 | 40.040 |
| `KEYBOARD_39996_v3` | 7.390 | 15.791 | 20.163 | 28.156 | 33.820 | 39.996 |
| `KEYBOARD_40001_v2` | 7.390 | 15.791 | 20.163 | 28.156 | 33.822 | 40.001 |
| `KEYBOARD_40058_v1` | 7.390 | 15.791 | 20.163 | 28.156 | 33.822 | 40.058 |
| `JUMP_cp5_32702_v1` | 7.390 | 15.791 | 20.163 | 28.144 | **32.702** | DNF |
| human WR rank 1 | 7.311 | 15.718 | 19.980 | 27.834 | 33.584 | 40.223 |

Our entire 262 ms advantage over the human world record is created **after
33.8 s**. The search never touched the first 28 seconds — which is what
`RESULT.md` §7 predicted structurally ("every accepted operator landed in ticks
3491–3997") and this measures directly.

**And we are 322 ms BEHIND the world record at CP4.** The 39.961 is a rank-2 lap
with a good last sector, not a fast lap.

## 2. The audit table

Field statistics over all 181 records (5 excluded per §8: ranks 57, 59, 100,
151, 173).

| sector | ours (= rank 2) | human WR | field min | holder | field mean | spread | corr(final) | path/chord |
|---|---|---|---|---|---|---|---|---|
| 0 start→CP1 | 7.390 | 7.311 | **7.295** | rank 6 | 7.957 | 41.872 | +0.72 | 1.19 |
| 1 CP1→CP2 | **8.401** | 8.407 | **8.401** | rank 2 = us | 9.664 | 23.602 | +0.43 | 1.37 |
| 2 CP2→CP3 | 4.372 | 4.262 | **3.784** | rank 9 | 5.271 | 13.355 | +0.43 | 1.01 |
| 3 CP3→CP4 | 7.993 | **7.854** | **7.854** | rank 1 | 9.806 | 13.877 | +0.71 | **3.36** |
| 4 CP4→CP5 | 5.658 | 5.750 | 5.674 | rank 2 | 7.757 | 18.693 | +0.76 | 1.27 |
| 5 CP5→finish | **6.147** | 6.639 | 6.396 | rank 2 | 8.584 | 12.827 | +0.87 | 1.21 |

**Sector 2's 588 ms is not there.** Sectors 1 and 2 are bimodal and
anti-correlated; rank 9's 3.784 is bought with a 9.098 sector 1. The joint
`s1+s2` leaderboard is the one that matters:

```
12.669 = 8.407 + 4.262   rank 1   <-- the best pair anyone drives
12.773 = 8.401 + 4.372   rank 2 = us
12.783 = 8.404 + 4.379   rank 15
12.835 = 8.574 + 4.261   rank 3
12.882 = 9.098 + 3.784   rank 9   <-- the "fast sector 2" variant, and it LOSES
```

## 3. What sectors 0–3 are actually worth: 338 ms, and that is all

Best jointly-achievable human driving, sectors 0–3:
`7.295 (r6) + 12.669 (r1's pair) + 7.854 (r1) = 27.818` against our **28.156**.

> **Sectors 0–3 contain 338 ms of recoverable time against the best human
> driving that exists. The gap to the author time is 1.431 s.**

So four fifths of the gap is not in the first 28 seconds, and no amount of
execution work there will close it. That is the answer to "where is our tape
merely copying the field": everywhere except the last six seconds, and copying
the field there is nearly free.

## 4. The bound, and what the jump does to it

| bound | value | vs AT 38.530 |
|---|---|---|
| field marginal splice (`RESULT.md` §3) | 39.404 | +0.874 |
| …joint-corrected for the s1/s2 pair | 39.888 | +1.358 |
| …with our sector 5 (6.147) | 39.639 | +1.109 |
| **…and with the jump's sector 4 (4.558)** | **38.523** | **−0.007** |

`4.558` is `32.702 − 28.144`, both measured on segment maps built from the
untouched map, and `JUMP_cp5_32702_v1` returns `cps=5` from the plain oracle on
the untouched map — a real checkpoint volume, not a relocated plane.

**Before the jump, recombining every known way of driving this map does not
reach the author time. With it, the bound lands 7 ms under, with nothing to
spare.** That is the whole answer to "is the AT reachable": it is, it needs the
jump, and it needs almost everything else to be best-known as well.

Two honest caveats, both load-bearing:

* **A splice bound is not a time on this map.** All 60 of `RESULT.md` §3's
  cross-splices DNF. 38.523 is the same kind of statistic as 39.404.
* **6.147 and 4.558 cannot simply be added.** Our 6.147 was driven from a CP5
  arrival at 33.814 in rank 2's state; the jump arrives at 32.702 12 m/s slower
  and off-line, and overshoots the road. That is session `14bbffec`'s open
  problem and it is the single thing this map now turns on.

## 5. Ranked: where to attack next

| rank | target | recoverable | confidence | why |
|---|---|---|---|---|
| 1 | **sector 3 (CP3→CP4)** | 139 ms known + **unknown** | speculative but the only candidate | **path/chord 3.36** — the car drives 3.36× the straight line, by far the largest detour on the map and the same signature that hid the sector-4 jump (1.27 there). Largest spread of any sector (13.877), corr +0.71, and 7.854 s of track. If this map has a second cut, it is here. |
| 2 | **sector 0 (start→CP1)** | 95 ms | certain | rank 6 does 7.295 and we do 7.390. Pure execution; the start is the one place with no upstream state to preserve, so it is also the cheapest to search. |
| 3 | **sector 1+2 as a pair** | 104 ms | certain | adopt rank 1's variant (8.407 + 4.262) instead of rank 2's (8.401 + 4.372). Must be taken as a pair. |
| — | sector 2 alone | **0 ms** | — | the 588 ms marginal minimum is an artefact of the bimodality; taking it costs 697 ms in sector 1. |
| — | sector 1 alone | **0 ms** | — | we already hold the field minimum. |

**A structural constraint on all of it** (`RESULT.md` §7, `ACQUISITION-addendum-146612-v1.md` (c)):
this map's tail cannot absorb ANY upstream change — a tape 29 ms faster at CP1
returns `DNF cps=1` on the full map and no 0–10-tick tail shift rescues it. So
sectors 0–3 must be chained **backward from CP4**, and every gain there costs a
re-drive of everything downstream of it. **338 ms of upstream gain would cost
re-driving 12 seconds of tape.** That is the real reason to look for a technique
in sector 3 rather than to grind sectors 0–2 for execution.

## 6. Instrument notes carried into this audit

* The ladder works on the **untouched** map with no model swap: this map has
  four `GateExpandableFinish` **Goal blocks** (`#2652`–`#2655`) on top of its
  four finish road blocks, and moving one by rewriting the three cell bytes in
  its own record is position-only. See
  `ACQUISITION_addendum_146612_gate_plane_orientation_v1.md`.
* **A relocated gate is a plane, and its axis is the `dir` byte.** dir 0/2 →
  z-plane at `z=32cz+16`; dir 1/3 → x-plane at `x=32cx+16`. Calibrated to
  −23 ms against the WR's own crossings.
* **A rung must be as narrow as the road.** A 4-cell curtain produced a "winner"
  318 ms ahead of our best tape which was the car airborne off the right-hand
  edge at x=1208, y falling 43 → 12. `fk btraj` every winner.
* `seg2` is **not** faithful for optimisation (CP2 is the map's only
  `GateCheckpointLeft32m`); it is fine for measuring an existing finisher, whose
  line sits inside both volumes. Sector 1 must be scored against `seg3`.
