# 146612 — s5_RESULT v3 addendum: final lap 39.430, and the two closing measurements

Agent `s5arm`, 2026-08-19 09:40Z. **Extends `s5_RESULT_v2.md`
(md5 `36e6881024df8e7b1d797f23c68f8ca8`); nothing in v1 or v2 is retracted.**

## 1. Final validated lap: 39.430

```
146612/s5_LAP_39430_v3.Ghost.Gbx      39.430   <- best lap on this map
```

Plain oracle, untouched map (sha256 `c6cca762…`), **one file per invocation at
`--jobs 4`** per the urgent `--jobs >= file count` rule:

```
s5_LAP_39430_v3   39430  cps=-
rank00001_40223   40223  cps=-     control, exact
rank00002_40226   40226  cps=-     control, exact
BEST_39961_v3     39961  cps=-     control, exact
```

Splits by simulation on the segment maps (each returns its declared split for
the reference ghost — the new `build` check):

| | CP1 | CP2 | CP3 | CP4 | CP5 | finish | s4 | s5 |
|---|---|---|---|---|---|---|---|---|
| human WR | 7.311 | 15.718 | 19.980 | 27.834 | 33.584 | 40.223 | 5.750 | 6.639 |
| **s5_LAP_39430** | 7.311 | 15.718 | 19.980 | 27.834 | **33.325** | **39.430** | **5.491** | **6.105** |

**Net for the session: 39.961 → 39.430, 531 ms, +0.900 over the author time.**
Sector-4 record 5.491 (field best 5.674); sector-5 record 5.992 (field 6.396) on
`s5_LAP_39748_v1`, and 6.105 on the best lap.

## 2. How the last 28 ms came, and it confirms §2's method a third time

`39.458 → 39.430` came from the **break-and-repair** pipeline applied at a
*different break point*: damage the finished lap with a station-08 objective
(35.554 → 35.476, 8.4 min), then climb it home over four rungs and the real
finish (5 steps × ~335 s × 64 workers).

That matters because two *other* break points on the same lap both came back
worse (v2 §10c: 39.467 from an x=912 break, and no lead from an earlier
station-08 attempt). **Break point is a hyperparameter with real variance, and
the cheap way to use it is several branches rather than one long run** — the same
conclusion §6's convergence table reaches from the other direction.

| break point | repaired lap |
|---|---|
| x=912 rung (`PLC`) | 39.467 |
| station 08 (`BRK8`→`CB8`) | **39.430** |
| none (plain polish, `POL`) | 39.458 |

## 3. Assembly, finally answered

Three independent measurements, all on this node tonight:

* **Welding a faster upstream tape on: worth exactly zero.** Backward bisection
  pinned the boundary to 18.800/18.900 s while the two tapes' inputs first differ
  at 18.850 s — the weld survives exactly as far as the two tapes are the same
  tape (`s5_BACKWARD_BISECTION_v1.md`, `_v2.md`).
* **Re-driving from a better upstream STATE: worth more than the state itself.**
  A 322 ms better CP4 became 483–531 ms at the line, for ~16 core-hours of
  repair per lap (v2 §2–§3).
* **Cost of the repair is bounded and predictable**: 5 steps, 80–400 s × 56–64
  workers each, delta flat across the staircase every time it worked.

> **Assembly does not decide this map against us. It costs about sixteen
> core-hours per lap and returns more than the upstream gain that provoked it.
> What is worth nothing is assembling *tapes*; what pays is transferring
> *states* and re-driving.**

## 4. Where the map stands, honestly

Best validated lap **39.430**, author time **38.530**, gap **0.900**. The pieces
in other arms' hands (CP3 at 19.815, sectors 1+2 at 12.538) are worth perhaps
another 250 ms through the amplification measured in v2 §3 — if they are
transferred as states and re-driven, which is now demonstrated machinery rather
than a hope. That lands the map near 39.2, and **I do not think 146612 falls
tonight.** The remaining 0.7 s is not a missing technique — the one undiscovered
route anyone found here is measured dead — it is tolerance, and §8's
classification stands: precision-bound.
