# Spaghetti Nights 2 — the world record falls by 0.262 s, and a 190-metre gap jump nobody takes is worth two thirds of the author's margin

**Author time 38.530 · human world record 40.223 · best validated 39.961.**

| tape | validated | vs human WR | vs AT | steer values | steer events | device |
|---|---|---|---|---|---|---|
| [`BEST_39961_v3`](replays/BEST_39961_v3.Ghost.Gbx) | **39.961** | **−0.262** | +1.431 | 76 | 234 | pad / TAS |
| [`KEYBOARD_39996_v3`](replays/KEYBOARD_39996_v3.Ghost.Gbx) | **39.996** | **−0.227** | +1.466 | **3** | **119** | **keyboard** |
| author time | 38.530 | — | — | — | — | — |
| human WR, jujumasterr *(control)* | 40.223 | — | +1.693 | 226 | 1157 | pad |
| human rank 2 *(control)* | 40.226 | +0.003 | +1.696 | 3 | 114 | keyboard |

TMX map [146612](https://trackmania.exchange/maps/146612) · uid
`jchzEcocJbNJreH4ebIoUYOt286` · authors **AmpelJoe10 + Wakawukwuk** ·
**181 recorded runs**.

**Not submitted to any Nadeo leaderboard, and it never will be.**

---

## The author time did not fall. The question it was asking did.

We are 1.431 s short. But:

> **Sector 4 of this map contains a 190-metre gap jump that all 181 humans
> either avoid or take wrong, and taking it right is worth 1.128 s — two thirds
> of the entire unbeaten gap.**

That is measured, not inferred. [`JUMP_cp5_32702_v1`](replays/JUMP_cp5_32702_v1.Ghost.Gbx)
reaches checkpoint 5 at **32.702** against the best human's 33.830, and the plain
oracle confirms on the **untouched** map that it crossed all five real
checkpoints (`cps = 5`) — a real checkpoint trigger, a bounded volume, not a
relocated plane. Best human sector 4 is 5.674 s; this is 4.546 s.

The technique is to take the sector-4 ramp at about **21° across the corridor**
instead of square. **0 of 181 humans land where it lands.**

The tape does not finish. From that checkpoint-5 state the car is 1.1 s early,
12 m/s slower and pointing differently, so the last 6.4 s has to be re-driven
from nothing, and that search did not converge. **The technique is established
and validated to the checkpoint; the lap that carries it is not.** That is the
honest statement, and it is a well-posed open problem rather than a wall.

## And separately: a keyboard beats the analog world record

**117 key presses, three steering values, 0.222 s faster than the analog human
world record**, on the field's own route, with no jump. That is the drivable
deliverable from this map and it needs no new technique at all.

## The exit from the jump is the unsolved part — and one number was wrong

The first write-up claimed the jump was still 0.639 s ahead of the world record
26 m past checkpoint 5. **That is retracted.** The car was off the road.

The road there is a single cell (`RoadTechTiltTransition2UpRight`, centre
x = 1168, surface x ∈ [1152, 1184]):

| tape | x at z = 760 | y | speed | on the road? |
|---|---|---|---|---|
| human WR | 1177.2 | 42.6 | 77.5 | yes |
| every other top-15 human | 1172.7 – 1177.4 | 42.6 – 42.9 | 69 – 86 | yes |
| **jump tape** | **1186.3** | **44.8** | 66.4 | **no — 2.3 m past the edge, 2.2 m in the air** |

A relocated gate is a **plane**, not a box. It fired anyway and reported a time
0.659 s better than the world record's. The station times for the jump lineage
should be read as plane crossings of unknown legality, not as progress.

The 1.128 s to checkpoint 5 is unaffected — that number comes from the real
checkpoint on the untouched map, and the tape passes 8.1 m from the gate centre
with x well inside [1152, 1184], against the world record's own 4.3 m.

What the retraction *buys* is a sharper problem. The jump does not merely land
across the road; **it overshoots the road entirely**, is still outside it 25 m
later, and comes down on `PlatformDirtWallOutCurve0` at (1200, 42, 752) — the
outside wall. That is the 74.5 → 22.6 m/s deceleration that had been measured
but not attributed. So the search has to trade flight **distance** as well as
heading: land shorter and inside x ≤ 1184. And 21° — the angle that maximises
reach to checkpoint 5 — is the worst angle on that axis.

The check that caught it cost about ninety seconds: decode the tape's own
trajectory, take the road block's cell centre, compare x against centre ± 16 m.

## Three method findings, all reusable

**A ladder makes a plateau searchable.** Same map, same seed, same search:
**0 finishers in 207 000 evaluations** with only the finish as an objective;
**13 of 22 stations climbed** once each station became its own objective. A
search reporting a flat landscape is more often missing an objective than facing
a flat landscape.

**A greedy per-station crawl locks in its own accidents.** Delta to the world
record, per station, for the crawl seeded from the jump tape:

| st02 | st03 | **st04** | st06 | st08 | st10 | st12 | st14 |
|---|---|---|---|---|---|---|---|
| −0.501 | −0.231 | **+1.232** | +1.416 | +1.728 | +1.891 | +2.161 | +2.601 |

**The entire run is decided at one station.** st03 → st04 is 1.813 s for 28 m of
track — a wall contact. Every station after it inherits a dead run, and the crawl
spends the rest of its budget nursing one. Nothing in the crawl notices: each
station reported an improvement over its own seed, every result validated, no
phantom, no error. Fixes, cheapest first — watch the *delta* and re-run any
station whose delta jumps; keep the best *k* per station, not the best one; and
score arrival at station *k+2..3* rather than at *k*.

**Optimise arrival PAST a checkpoint, never at it.** "Fastest to CP5" bought a
state that cannot use its own speed. With a ladder, "fastest to a station
50–100 m beyond" costs exactly the same to evaluate.

And on the fleet's reward-shaping notice: shaping was genuinely live here (the
incumbent does not finish) and the finish rate was still 0 % over 207 000
evaluations. **"Shaping is live" is necessary, not sufficient** — only a nearer
objective crossed this DNF basin.

## Checks

* Field reproduction: **176 of 181 exact.** Five fail, all `DNF cps=1`, **none
  returning a different millisecond** — they re-download byte-identical, and two
  of them contain mid-run respawns, which an input-replay oracle cannot follow.
  Zero wrong-time divergences is the healthy pattern.
* Map sha256 `c6cca762e167eba6e969c07f306798c29c88d0da397b4744d4042c51b21526db`,
  Nadeo-served, 3 824 673 bytes.
* Every published row re-validated twice through the plain oracle on the
  untouched map, in cold batches carrying both human runs as known-answer
  controls (40.223 and 40.226 exact in every batch).

## Notes

* [`RESULT.md`](notes/RESULT.md) — the full write-up
* [`CORRECTION_st01_offroad.md`](notes/CORRECTION_st01_offroad.md) — the retraction, in the
  author's own words
* [`GREEDY_CRAWL_NOTE.md`](notes/GREEDY_CRAWL_NOTE.md) — the crawl failure as a general method note
* [`GATE_PLANE_ORIENTATION.md`](notes/GATE_PLANE_ORIENTATION.md) — a relocated gate is a plane and
  its axis is a byte; this is why a third of well-chosen probe placements are silent
* [`SECTOR_AUDIT.md`](notes/SECTOR_AUDIT.md)

## Update: the segment sum is now 39.229

Later the same night, with three search arms live, the **sum of separately
searched sectors** stands at **39.229** against the author time of 38.530 —
+0.699, down from the +1.431 at the top of this page.

**That is a segment sum, not a lap.** Each sector was searched against its own
objective, and nothing has yet driven them end to end: the assembly is unproven,
and on this map the assembly is exactly the hard part — the jump reaches
checkpoint 5 a full second ahead and then cannot use its own speed. A sum of
sector optima is an upper bound on what the route is worth, not a time anyone
has driven.

The validated lap is still **39.961**, and that is what the index states. This
note records the direction of travel. The arms are still running.
