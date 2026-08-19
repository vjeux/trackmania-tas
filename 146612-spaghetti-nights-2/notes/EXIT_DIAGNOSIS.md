# 146612 exit problem — the diagnosis, and the arithmetic that constrains it

2026-08-19 07:31Z. Supersedes nothing; extends `CORRECTION-st01-offroad-v1.md`.
Every position below is from `fk btraj` on the tape's own run, not from a ladder.

## 1. What the car is actually hitting

The sector-5 road **rises**: the human world record's own y goes 42.02 → 43.98
between z = 736 and z = 794, smoothly, wheels down, gaining speed 75.3 → 79.6.
The block is `RoadTechTiltTransition2UpRight` (cell 1168, 42, 752) — a banked
transition, 32 m wide, surface x ∈ [1152, 1184].

Our tapes arrive **above it**:

| z | WR y (surface) | jump y | beam-winner y |
|---|---|---|---|
| 745 | 42.15 | 45.35 | 44.50 |
| 755 | 42.50 | 44.67 | 43.95 |
| 765 | 42.79 | 44.58 | 44.87 |
| 775 | 43.28 | 43.23 | 44.85 |
| 785 | 43.61 | 40.55 (gone off) | 44.38 |

**The jump does not land on the road and drive away; it comes down ONTO a rising
banked road from 1.5–2.5 m up.** The impact is the deceleration I had recorded
and mis-attributed: the beam winner goes 71.2 → 41.5 m/s in 300 ms (≈ 100 m/s²,
a collision, not friction) at z ≈ 755, and the jump tape goes 74.5 → 22.6 m/s
slightly later and 2.3 m outside the road edge, where the wall
`PlatformDirtWallOutCurve0` (1200, 42, 752) is waiting.

So the exit problem is **three-way, and I had only two of the axes**:

* **heading** — land pointing along +z, not across;
* **lateral** — land inside x ≤ 1184, which the raw jump does not;
* **height and range** — *land short enough and low enough to be settled on the
  surface before z ≈ 750*, which neither tape does.

The third is the one 21° is worst at: the angle that maximises reach to CP5 is
the angle that carries the car long, high and past the road.

## 2. The lookahead objective works, and here is the evidence

Scoring arrival at station *k+3* instead of station *k* (see
`GREEDY_CRAWL_NOTE-v1.md`):

| at station 04 | arrival | vs WR (34.982) | on the road at z = 760? |
|---|---|---|---|
| greedy crawl winner | 36.214 | +1.232 | — |
| **lookahead beam winner** | **35.489** | **+0.507** | **yes, x = 1178.5** |
| jump tape | (crashes) | — | **no, x = 1186.3** |

**725 ms better at the same station, and the winner is on the road** where the
greedy crawl's lineage was not. The lookahead does select against the off-road
decoy, exactly as intended — a car outside the surface cannot reach the later
station in competitive time. It does not yet select against *landing hard*,
because a hard landing still gets there.

## 3. The arithmetic, and it is not comfortable

The jump reaches CP5 at **32.702**. To finish under the author time:

```
sector 5 required  =  38.530 − 32.702  =  5.828 s
best sector 5 ever driven on this map:
    6.147   (this project, from rank 2's CP5 state)
    6.396   (best human, rank 2)
deficit: 319 ms better than anything achieved, from a state that arrives
         5 m/s SLOWER (70.3 vs the WR's 75.3) and 3 m higher
```

**So the jump as it stands cannot reach the author time even with a perfect
exit.** Solving the exit is necessary and not sufficient. What is needed is a
launch that is simultaneously early to CP5 *and* arrives with the speed and
attitude to drive sector 5 — i.e. the trade has to be resolved in favour of the
exit, accepting a later CP5 than 32.702, and the useful objective is therefore
**CP5 arrival + sector-5 time**, never CP5 arrival alone.

That reframes the target usefully: we do not need to keep all 1.128 s of the
sector-4 saving. Giving 300 ms of it back to arrive at 33.0 on the road, at
75 m/s, pointing down the road, would leave 5.53 s of sector 5 to find — still
hard, but it is the same shape of problem as the field's own 6.396 rather than
319 ms beyond the best ever driven.

**Flag for the fleet bound:** the other arm's 38.523 implies a sector 5 of
5.821 s (38.523 − 32.702). I cannot reproduce that component — the best sector 5
I know of is 6.147. If 5.821 came from a marginal best over a different CP5
state, the bound should be restated as 32.702 + 6.147 = **38.849**, which is
+319 ms over the author time, not −7 ms. I may be missing a tape; worth
reconciling before anyone treats −7 ms as the headline.

## 4. What to search next

1. Objective = **CP5 arrival + arrival at station 06**, jointly — or simply
   score station 06 alone from a mutation window that includes the launch, which
   is what the beam is doing now. Never score CP5 alone again.
2. Add a **height gate**: reject or penalise any candidate whose y exceeds the
   road surface by more than ~0.5 m at z = 750. The surface height is known
   analytically from the WR's own trace (42.02 at z = 736 rising ≈ 0.034 m/m).
3. Sweep the launch **shorter**: the current release is at 29.66–29.75 s; a
   release 100–200 ms earlier gives a flatter, shorter flight that lands before
   the tilt transition. That is the untested direction and it is cheap.
