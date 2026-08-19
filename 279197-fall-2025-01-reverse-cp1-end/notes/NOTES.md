# 279197 — running notes

Companion to `PLAN.md`. Chronological, evidence first. Times are local
(America/Los_Angeles), 2026-08-18.

## Status

| | ms | how |
|---|---|---|
| human online WR (ShcrTM) | 10602 | leaderboard, re-simulated exactly |
| **author time (AT)** | **10598** | the thing to beat |
| **our best, validated on the untouched map** | **10596** | `tmsearch` from the WR seed, 7 min |

`tmtas validate --map <ABS>/map.Map.Gbx <ABS>/best/best_10596.Ghost.Gbx`
→ 10596, reproduced on demand, with the human WR alongside it as an identity
control returning 10602 in the same batch.

## Controls that are passing

1. **27/27 human ghosts** (ranks 1-15, 51-53, 151-153, 301-303, 501-503)
   re-simulate to their exact leaderboard millisecond.
2. **Candidate factory**: `tmsearch --verify` round-trips the WR to 10602.
3. **Tape editor**: `tmtas patch` with no `--set` reproduces its input exactly
   (10596 → 10596).
4. **Gate machinery**: the relocatable-gate maps put back at the item's own
   position return 10602 / 10598 / 10800 for three different runs — the map
   surgery is a no-op when it should be.
5. `tmtas selftest` 10/10 on this node.
6. **No shared search root.** Every `tmsearch` after the first carries an
   explicit distinct `--root`; the fleet-wide shared-`/dev/shm/tmsearch` bug
   cannot apply. (`tmtas validate` and `tmmaps oracle` already root themselves
   per-pid.) Zero failed re-validations so far, so nothing in
   `tm-loop/phantoms/`.

## The map, in one paragraph

Two waypoints: a Spawn block and a **Goal that is a custom item** (`cp1end`, a
repurposed `roadborder`, yaw = −π, at (800, 56, 768)). No checkpoints. 10.6 s,
597 m, flat out from a standing start: a bend, a hard chicane flick at 3.2 s, a
downhill from y=66 to y=58, a 2 s straight, and then **one 140 m-radius right
sweeper taken at full throttle from 286 km/h to the flag**. Speed rises
monotonically the whole way and saturates at 341.7 km/h (94.9167 m/s) about
150 ms before the line — every one of the 27 humans is at exactly that speed at
the gate. So the endgame has no speed left to find: **time is distance, at
9.49 cm per millisecond.**

## Finding 1 — the finish gate's inside edge is what limits the racing line

`tmmaps probe --axis x` slides the gate sideways and asks which runs still
trigger it. The gate registers over a 23 m window in x, and the window's lower
end moves run by run — exactly with where each run crosses:

| run | x at the flag | largest gate x that still fires |
|---|---|---|
| r001 (WR, 10602) | 772.54 | 800.5 |
| r051 (10628) | 774.12 | ~801.5 |
| r301 (10724) | 775.05 | ~801.5 |
| r503 (10800) | 778.72 | ~805.5 |

Consistent to the 0.5 m grid: the trigger's inside (low-x) edge sits at world
**x ≈ 772.0**, and the human WR crosses it at 772.54 — **about half a metre of
margin**. Tighter is faster (the sweeper's radius is ~140 m, so shaving the
radius is worth ~10 ms per metre of arc), but tighter than the edge and the run
does not finish at all and never appears on a leaderboard. **The 561-strong
human field is stacked against a trigger boundary they cannot see.** That, not
grip, is what makes the line look ground-flat.

## Finding 2 — the reported millisecond is a coarse, uneven quantiser

Sliding the gate *along* the track (`--axis z`) and re-timing the same tape
gives the exchange rate: **10.5 ms per metre** over ±16 m, i.e. the car meets a
z-normal plane at its capped speed. But at 5-15 mm resolution the staircase is
badly non-uniform — for the human WR the successive bins were 0.042, 0.042,
0.144 and 0.096 m wide, and the value 10599 is **unreachable**, skipped
entirely. Two tapes 8 mm apart in true progress can report 10596 and 10597.

Consequence: **the oracle's integer millisecond is a lossy proxy.** A search
optimising it spends nearly all its time on a plateau where a real improvement
of up to 14 cm is completely invisible, and its Metropolis acceptance is partly
deciding on quantisation artefacts.

## Finding 3 — the tape is a knife edge

Every single-tick throttle lift tried — at ticks 2, 5, 10, 20, 40, 80, 150, 300
and 500 — **DNFs**. So does throttle on tick 0, and so does sliding the whole
tape one tick either way. (Timed against the gate ladder, the tick-0 variant
reaches z=680 at 10148 ms instead of 9821 and gets worse from there: it is not
"one tick ahead", it is wrecked.) Every human tape has the throttle off on tick
0, so there may be a free tick there in principle — ~10 ms if the car could be
put one tick ahead — but reaching it means crossing a deep DNF valley, and this
map gives **no shaping signal at all**: with one waypoint, every DNF returns the
same "reached 1 checkpoint" and nothing else. Recorded as a known, unexploited
lever, not a result.

## Finding 4 — where our 6 ms actually came from

Built a ladder of intermediate gates by relocating the Goal along the final leg
(`tmmaps places`, new subcommand) and timed the field through it:

| plane | our 10596 | WR 10602 | gap |
|---|---|---|---|
| z=680 (t≈9.8 s) | 9821 | 9826 | 5 ms |
| z=700 | 10053 | 10058 | 5 |
| z=720 | 10272 | 10278 | 6 |
| z=740 | 10484 | 10490 | 6 |
| finish | 10596 | 10602 | 6 |

**Five of the six milliseconds are already banked before the last 0.8 s.** The
gain is upstream of the sweeper exit, not in it.

## The instrument: a movable finish plane, and the ratchet built on it

Because the Goal is one relocatable item, the finish plane is ours to place.
`tmmaps probe --keep-model` (new flag) moves the map's own item instead of
swapping in the stock finish-gate model — the old `move_gate` swap deletes the
only Goal a `CP1 End` map has, and every run silently DNFs, which is how the bug
was caught: the *identity* placement returned DNF.

`tmmaps places --rank <ms>` then ranks a set of tapes by the largest plane
offset at which each still reports `<= ms`. At a 5 mm ladder step that is a
**0.05 ms resolution measurement** of who is genuinely further along — twenty
times finer than the oracle's own answer.

`ratchet.sh` turns the instrument into a search objective:

1. measure the champion's staircase edge,
2. build a map with the plane a hair past it, so the champion sits one
   millisecond above the threshold and the smallest true gain reads as a whole
   millisecond,
3. run three arms from the champion on that map,
4. rank the survivors on the fine ladder, adopt the furthest-along, re-aim.

First evidence it works: three arms on the first vernier map found "10595" in
**seconds** where a real-map search had been flat at 10596 for ten minutes and
1.7 M evaluations. On the untouched map those tapes are still 10596 — as they
must be — but the ladder puts them 15-20 mm further along than the champion.
The plane only ever measures; the claim is always re-made with
`tmtas validate` against the untouched map.

## Deliberate choice: no fork server

`reliability.tgz`'s fork server would give ~3x throughput, and `fk btraj` would
give per-tick trajectories of candidates. It is **not** being used here.
`fk btraj` aborts on this map at the shim's tick probe (the checkpoint
calibration is map-specific), and the known open incident on that path is that
the search *banks phantom improvements* — 5 of 82 incumbents in one
investigation did not re-validate. Rule 3 makes a failed re-validation a stop,
and the plain oracle path is measured exact here on 27 independent runs. Three
times the evaluations is not worth putting the integrity of the answer at risk
on a map where the whole margin is 4 ms. Throughput on the plain path is
~1950 eval/s on the 176-core box, which is the physics cost, not overhead.
