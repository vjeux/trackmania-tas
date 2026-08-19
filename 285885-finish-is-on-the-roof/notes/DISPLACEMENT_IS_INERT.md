# 285885 — the (+x,−z) displacement lever does not exist. It is the gate-conflation again.

**Supersedes §4 and §5 of `att_DIRECTION3_INVERTED_ARRIVAL_v1.md`** (mine, one
hour earlier) **and the "trade curve" in `bis197047_MAP_STATE_v4.md` /
`FINDINGS.md`** (the first two agents'). Everything else in all three stands.
This is the third phantom this map has produced from one root cause, and the
first one that was on the record before I arrived.

---

## 1. The claim being retired

On the record since the first agent, reproduced by me from an independent
instrument, and endorsed in my own v1 §4:

> "Every 10 mm of height buys ~0.07 m of the (+x,−z) diagonal" — i.e. a car
> moved **0.42–0.45 m toward (+x,−z)** clears the finish, at **0.111 m of
> clearance per metre**.

Both measurements were real. Both measured **the gate**, not the car.

## 2. Why it is wrong

Displacing the *gate* by (−dx,+dz) is equivalent to displacing the *car* by
(+dx,−dz) **only if the car's height is unchanged**. It is not: the roof is an
11.4° plane and a car that really moves toward (+x,−z) **rides up it**.

| per metre of car movement toward (+x,−z), staying on the roof | |
|---|---|
| ceiling gained (from the gate-offset sweep) | **+0.111 m** |
| car rises (roof gradient `0.09211·0.832 + (−0.17895)·(−0.555)`) | **−0.176 m** |
| **net clearance** | **−0.065 m — it HURTS** |

## 3. The measurement that decides it

Seven real tapes that genuinely cross the patch at different places — three
route families, a **0.913 m spread** of crossing point along the diagonal, each
crossing position read from its own live `fk btraj` trajectory (not from a gate
sweep, not from embedded telemetry), each threshold from a 66-rung 10 mm ladder
at the **true** gate x/z:

| tape | crossing offset along (+x,−z) | threshold |
|---|---|---|
| seedA | **−0.372 m** | 144.11 |
| seedF | +0.083 | 144.09 |
| g4 | +0.216 | 144.08 |
| g1 | +0.229 | 144.08 |
| bis_418.6138_best | +0.309 | **144.07** |
| seedG | +0.374 | 144.08 |
| lat_418.2_best | **+0.541 m** | 144.14 |

> **threshold = 144.092 + 0.003 · s**, n = 7, residual scatter ±0.02–0.05.

**The slope is zero to within the noise.** Not +0.065 as the roof geometry
predicts, not −0.111 as the gate sweep suggested: **nothing**. Moving the line
along the diagonal buys nothing, costs nothing, and is not a lever in either
direction. The geometric prediction and the gate-sweep reading are two errors of
opposite sign, and the truth is that they cancel.

## 4. What replaces it — a much stronger invariant

Every tape that reaches the finish patch at speed, across three unrelated route
families and 0.9 m of lateral spread, needs the gate at **144.07–144.14
(mean 144.092, sd 0.024)** to fire. The real gate is at **144.000**.

> **The deficit is 70–140 mm and it is the same for every fast line. It is not a
> property of a tape, a line, or a crossing point. It is what this finish costs
> an upright car, and the only quantity that moves it is attitude (0.526 m for a
> flip, ≥26° for the threshold) — which §3 of v1 shows is unavailable on an
> uphill approach at any price.**

That is a considerably harder result than "the fast route is 70 mm short", and
it is the thing to quote about this map.

## 5. Why the diagonal ladder still gained a rung, and why it stalled at 0.40 m

The ladder is a valid **ordering** — rung 0 is the untouched map, so a rung-0
fire is a finish — but its rungs are **not reachable by displacement**, because
displacement is inert. The four climbs' rung 11 → 10 gain is ~5 mm of clearance
bought from marginal attitude and height differences (`g1`'s crossing `u_y` is
0.9834 against the seed's 0.9821 — 0.9° of tilt, worth ~4 mm), not from moving
the line. They stalled at 0.40 m because that is where those marginal
differences run out, and there is nothing behind them.

**So the coordinator's question — operators or route? — has a third answer:
neither.** The lane is not the limit (0.9 m of spread exists across banked
tapes, and the widest offset scores *worst*), and the operators are not the
limit (four families, 171 704 evaluations, all stalling together). **The axis
itself is inert**, and no operator and no upstream displacement can make an
inert axis pay.

## 6. The root cause, for the third and last time

Three published-and-wrong claims on this map, one mistake:

1. "the car crashes at (417.5, 1704.6)" — reading fire times as a trajectory
   past the point where a different face of the volume becomes binding;
2. "the ceiling tilts so clearance improves toward −x" — fitting a ceiling slope
   to a sweep in which the tape re-enters the volume at a different point every
   time;
3. "0.42 m along the (+x,−z) diagonal clears it" — treating a gate displacement
   as a car displacement on a **sloping** surface.

> **A gate displacement is not a car displacement.** It changes the relative
> geometry, where the car enters the volume, *and* — on any surface that is not
> level — the car's height relative to the trigger. A displacement sweep
> measures the **map**, and tells you nothing about what a car can do, unless
> you close the loop with tapes that genuinely cross at different places and
> whose positions you read from a re-simulation.

The loop costs one `fk btraj` per tape and seven tapes settled a claim that had
stood for two agent-sessions and ~1.6 M evaluations.

## 7. Files

`att_xpoint.txt` (the seven crossings from live trajectories) ·
`bt_*.csv` (their 100 Hz trajectories) · `att_thr66.txt` (the 66-rung threshold
ladder) · `att_tools.tgz` (adds `xpoint.rs`, `regr.rs`, `netgrad.rs`).
