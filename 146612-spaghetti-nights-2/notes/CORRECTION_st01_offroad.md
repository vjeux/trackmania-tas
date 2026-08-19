# CORRECTION — map 146612: my "0.639 s ahead 26 m past CP5" was the car OFF THE ROAD

Written 2026-08-19 07:26Z, after the fleet notice on wide-rung decoys
(`FLEET_NOTICE_gate_ladder_three_repairs_v1.md`). **This retracts one number in
`RESULT.md` Part 2 and sharpens the open problem. It does not touch the
checkpoint-5 result, which stands.**

## What I claimed

> The jump is 0.639 s ahead of the world record 26 m past checkpoint 5, and the
> whole advantage is gone 55 m later.

From the ladder: station 01 (1177.2, 42.6, 762.3), human WR 33.931 s, jump tape
**33.272 s**.

## What is actually true

I `btraj`'d the jump tape and asked where the car is at that instant, and
whether it is on the road. The road there is a single cell —
`RoadTechTiltTransition2UpRight`, centre x = 1168, so the surface spans
**x ∈ [1152, 1184]**.

| tape | x at z = 760 | y | speed | on the road? |
|---|---|---|---|---|
| human WR | 1177.2 | 42.6 | 77.5 | yes |
| every other top-15 human | 1172.7 – 1177.4 | 42.6 – 42.9 | 69 – 86 | yes |
| **jump tape** | **1186.3** | **44.8** | 66.4 | **NO — 2.3 m past the edge, 2.2 m in the air** |
| re-aimed AIM3 tape | 1181.0 | 43.6 | 41.9 | yes, barely — and already down to 42 m/s |

**The jump tape crosses station 01's plane while flying past the outside edge of
the road.** A relocated gate is a plane, not a box, so it fired anyway and
reported a time 0.659 s better than the world record's. That is precisely the
decoy the fleet notice describes, and my number is an instance of it.

**Retracted: the 0.639 s lead at st01, and with it the framing that the jump
"still holds most of its advantage 26 m later".** It does not. It is off the
track by then. The ladder times for the jump lineage at st01–st03 should be
read as plane crossings of unknown legality, not as progress.

## What still stands, and why

**The 1.128 s saving to checkpoint 5 is unaffected.** That is not a ladder
number: it is the plain oracle on the **untouched map** reporting
`JUMP_cp5_32702_v1.Ghost.Gbx` → CP5 at 32.702 s with **`cps = 5`**, i.e. the
real checkpoint trigger — a bounded gate volume, not a plane. Position check:
the tape passes 8.1 m from the gate centre at (1164.9, 45.3, 741.4), x well
inside the road's [1152, 1184], against the world record's own 4.3 m at
(1174.3, 42.0, 735.8). Best human sector 4 is 5.674 s; this is 4.546 s.

The technique — take the sector-4 ramp at ~21° across the corridor instead of
square, 0 of 181 humans land where it lands — is unchanged.

## What it changes about the open problem — for the better

The exit problem was posed as "the landing points across the road instead of
along it". The measurement says something stricter and more useful:

> **The jump overshoots the road entirely.** It lands beyond the far edge and
> is still outside it 25 m later, then comes down onto
> `PlatformDirtWallOutCurve0` (1200, 42, 752) — the outside wall — which is the
> 74.5 → 22.6 m/s deceleration I had already measured at st03→st04 but had not
> attributed.

So the search must trade flight *distance* as well as heading: land **shorter**
and inside x ≤ 1184, not merely aimed better. That is a third axis on the
three-way trade, and it is the one the 21° launch is worst on — the angle that
maximises reach to CP5 is the angle that carries the car past the road.

Concretely, for whoever continues:

* Constrain or score the landing point, not just the arrival time. A station
  whose winner is off the road is worthless, and the ladder cannot tell you.
* **`btraj` every march winner and check it against the road cell's x-span
  before believing its number.** Two commands; it would have saved this
  retraction.
* The lookahead objective (score arrival at station *k+3*, not *k*) partially
  self-corrects, because a car off the road cannot reach the later station —
  but only partially, and only after it has already crashed.

## Process note

The check that caught this cost about ninety seconds: decode the tape's own
trajectory, take the road block's cell centre, compare x against centre ± 16 m.
The ladder had no way to signal it and every internal control passed — origin
control clean, 22 distinct hashes, monotone times, plain-oracle agreement on
every full-map claim. **A gate ladder measures "did the car cross this plane",
which is not the same question as "did the car drive this track", and the gap
between those two questions is where a decoy lives.**
