# 279209 — running notes

Companion to `PLAN.md`. Chronological, evidence first. Times are local
(America/Los_Angeles), 2026-08-18. Everything here was measured on
`117096.od.fbinfra.net` (176 cores) against
`/tmp/tmoracle/server/TrackmaniaServer`.

## Status

| | ms | how |
|---|---|---|
| human online WR (`jujumasterr`) | 6604 | leaderboard, re-simulated exactly |
| **author time (AT)** | **6595** | the thing to beat |
| **our best, validated on the untouched map** | **6578** | `tmsearch` from the WR seed |

AT beaten at t = 17 s of the first search arm (6592), banked immediately.

## Controls that are passing

1. **105/105 human ghosts** (ranks 1–45, 61–75, 101–115, 151–165, 251–265;
   6604–7029 ms) re-simulate to their exact leaderboard millisecond.
2. **Candidate factory**: `tmsearch --verify` round-trips r001 to 6604.
3. **Every banked best re-validates** through the plain oracle against the
   banked copy of the map. Zero failed re-validations so far, so nothing in
   `tm-loop/phantoms/`.
4. **Gate machinery identity control**: the relocatable-gate map with the item
   put back exactly where it already is reproduces 6604 / 6608 / 6655 / 6757 —
   the surgery is a no-op when it should be.
5. **No shared search root.** Every `tmsearch` carries an explicit distinct
   `--root`, and the patched tool also defaults to `/dev/shm/tmsearch-<pid>`
   and refuses a root owned by a live pid.

## The map, in one paragraph

Two waypoints: block#2653 `RoadIceStart` (Spawn) at ≈(1136, 34, 1101), and a
**Goal that is a custom item** (`cp1end`, model `blocks\bob.Gbx.Item.Gbx`,
yaw = +π/2) at (1024, 24, 1280), cell (32, 11, 40). No checkpoints. 6.6 s, on
the ground for every tick, gas held on essentially every tick, brake almost
never used. A standing start on ice down a steep straight (0–2.5 s, 0→130 km/h,
lateral movement under 1 m), a stab of full right (2.8–3.6 s), then **one 77 m
radius left-hand sweeper held at full lock for 3.0 s** to the flag, accelerating
the whole way, 161 → 212 km/h. The finish plane is normal to x at
**x ≈ 1040.68**, and the car meets it at 58 m/s: **1 ms = 5.8 cm, 17.2 ms/m.**

## Finding 1 — a top-3 human run is pure keyboard, 17 inputs

`r003_6608` (rank 3, 4 ms off the human WR) uses a steer alphabet of exactly
{−127, 0, +127} and **17 input change events for the whole lap**. `r075_6737`
is the same shape with 14. So the keyboard ladder on this map is not a guess:
it is read directly off the human tapes, and a three-value tape is already
within 4 ms of the best human. r003's events:

```
   30 LEFT     740 centre   800 RIGHT    900 centre  1020 RIGHT  1170 centre
 1250 RIGHT   1440 centre  1500 RIGHT   1600 centre  1730 LEFT   2410 RIGHT
 2560 centre  2790 RIGHT   3620 centre  3680 LEFT   (held to the flag)
```

Everything after 3.68 s is one held key. Across 16 sampled tapes spanning ranks
1–265, brake appears in **one** tape for **six ticks**, and the population's
modal steer values are −127 (46%), 0 (42%) and +127 (11%).

## Finding 2 — the field is identical for two seconds and then fans out

Cumulative delta against the human WR at a ladder of planes (top 15):

| station | t | spread |
|---|---|---|
| z=1110 | 2.3 s | −0 … +3 ms |
| z=1150 | 2.7 s | −1 … +8 ms |
| z=1190 | 3.7 s | −3 … +22 ms |
| z=1250 | 5.1 s | −14 … +54 ms |

Over the whole 105-run field, corr(speed at t, finish time) is ≈ 0 until
t = 3.2 s and −0.51…−0.59 from t = 3.4 s; corr(yaw, finish) peaks at **+0.80 at
t = 3.8 s**. Our own unbiased 4170-move dump agrees from the other side: **0
improvements in 1342 samples over ticks 0–203**, 8.3 % improving at ticks
475–543. Two independent instruments, same answer: **the ice start and the
straight are solved; the time is in the sweeper.**

## Finding 3 — the gate's trigger window is NOT the limiter here

Sliding the Goal item laterally (`tmmaps places --axis z`) and asking which runs
still trigger it brackets the window:

| run | z at the flag | largest +z gate slide that still fires |
|---|---|---|
| r101 (6757) | 1266.4 | 9 m |
| r015 (6655) | 1270.9 | 15 m |
| r001 (6604, WR) | 1273.8 | 18 m |
| our 6580 | 1273.5 | 21 m |

The window's low edge is therefore at about **item_z − 24.5 m**, i.e. world
z ≈ 1255.5 with the gate in place, and every human crosses **17–18 m inside
it**. This is the opposite of what the 279197 agent found on their map, where
the WR had half a metre of margin and the field was stacked against an
invisible boundary. **On 279209 the gate is not the constraint** — and the
direction of the corner says the same thing: the sweeper's centre is at
(1052, 1206), so the *inside* of the corner is toward LOWER z, yet the faster
runs cross at HIGHER z. Nobody here is being held off a tighter line by the
trigger; they are simply carrying less speed.

Recorded as a checked-and-rejected hypothesis so the next agent does not spend
the hour.

## Finding 4 — the movable plane is a working sub-millisecond ruler

Sliding the gate along the direction of travel is exactly linear at the speed
the car meets it:

| gate slide (+x, toward the car) | our 6580 tape | human WR |
|---|---|---|
| 0.0 m | 6580 | 6604 |
| +0.5 | 6572 | 6595 |
| +1.0 | (see below) | 6587 |
| +2.0 | 6562 @ +1.2 | 6570 |

**17.2 ms/m**, so a 5 mm ladder step is a 0.086 ms ruler off the plain oracle —
no fork server, no instrumented child, no new trust assumptions. Two caveats,
both measured:

- **Negative offsets are invalid.** The item sits at x = 1024.0, which is the
  low edge of its declared cell (32, 11, 40); sliding it to x < 1024 leaves the
  cell while the record still says cell 32, and the readings go non-monotone
  and wrong (−1 m read *later* than −2 m). Only slide toward the car.
- **One rung in a ladder can jump.** At exactly +1.0000 m our 6580 tape reads
  6568 where its neighbours read 6564 and 6563 — reproducible byte-for-byte
  across repeated runs, and **not** present for any of the five other tapes at
  the same placement. The registration is a car-box overlap and this tape is
  the most yawed at the flag, so which corner of the box touches first can
  switch. Practical rule: **read the vernier over three consecutive rungs,
  never one.** The ratchet script enforces it.

## Finding 5 — the edge-heavy operator mix did NOT pay (screen)

Predicted in PLAN.md §6.3 at 1.5–2.5× the improvement rate, because `edge`
improved 12.4 % of unbiased single moves against 1.6 % for `cos`. Implemented as
`--ops edgy` (34 % wide-edge ±8 ticks, 16 % translate-a-whole-hold, 14 %
grow/shrink-a-hold, 12 % tap-inside-a-hold, 24 % stock mix) and screened against
the stock mix, four arms concurrently on the same box, 35 workers each, two RNG
seeds each, seeded from the human WR.

See the table below for the scored result. The direction was **wrong**, and the
reason is instructive: `edge` wins on the *human* tape because a human tape is
all digital holds and its transitions are mistimed. Once the search has spent a
minute on it the tape is no longer a human tape — it has analog values in it —
and the operators that keep paying are the ones that explore the 252 steer
values no human uses. Measuring the neighbourhood of the SEED told me about the
seed, not about the search.

## Deliberate choice: no fork server

Three reasons, in order of weight:

1. A sibling agent reports an unfixed defect in fork resume — the child cannot
   un-consume an input record, so candidate ticks between the master's
   calibration and a worker's actual stop are invisible to the evaluator but
   present in the written tape. Those score as the incumbent, get accepted at
   delta 0, and contaminate the lineage for free (576 phantoms / 170 runs).
2. The whole run here is 6.6 s with gains from tick 200 to tick 610, so the
   resumable prefix is short and the predicate watchdog has almost no dead tail
   to save. The measured ceiling on a comparable map was ~1.2–1.3×.
3. The sub-tick objective, which was the one real reason to want the fork
   server, is available on the plain oracle through the movable plane
   (Finding 4) at 0.086 ms resolution.

`fk btraj` is still used, once per tape, off the search's hot path, to get exact
per-tick trajectories for analysis. It works on this map (664 ticks, validated
time matches).

## Finding 6 — the ratchet works, and it is the only instrument with a gradient left

Nine rounds of `ratchet_loop.sh` from the 6578 champion (4 arms x 25 workers,
240 s per round, plane re-aimed at the champion's own staircase edge each time).
Every adoption was re-validated on the untouched map first and refused if it was
slower there; every ranking was done on a 5 mm ladder.

| | gate offset still reporting 6577 | continuous time |
|---|---|---|
| first tape to reach 6578 | +0.1200 m | 6579.06 ms |
| independent lineage (from `r002`, hours of separate search) | +0.1250 m | 6579.15 ms |
| after nine ratchet rounds | **+0.0700 m** | **6578.20 ms** |

**0.86 ms of real progress that the integer millisecond cannot show**, against a
concurrent plain-map control that made 0 improvements in 271,530 evaluations
from the same champion with the same cores. And the two independent lineages
landing 0.09 ms apart is the strongest statement available that the route is
out of ideas rather than the search.

## Finding 7 — the brake is not the mechanism at the crest

An action-key arm reached for `brk@325` and that looked like it might beat the
throttle lift. It does not, on a keyboard tape: swept over ticks 300-380 x
durations 1-20,

| what | best gain |
|---|---|
| brake tap, throttle stays on | **-2 ms** |
| throttle lift | **-12 ms** |
| both together | -12 ms (identical to the lift alone) |

So it is weight transfer from closing the throttle, not braking force. Worth
knowing, because "tap the brake" is the thing a driver would reach for first and
it is worth one sixth as much.

## Session tally

25 banked tapes, all re-validated exactly in one closing batch alongside three
human identity controls (6604 / 6608 / 7029). **Zero failed re-validations all
session**, on any path, so `tm-loop/phantoms/` received nothing from this map.
The classic oracle path with an explicit per-process `--root` was the only
scoring path used.
