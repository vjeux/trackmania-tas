# Summer 2026 - 01 — chasing the world record

Map uid `buNzfsVlp2NF2oWtHM3729dEylg`. **This is not an unbeaten-author-time
map** and it is not a map we have taken. The world record is a human's and it
still stands; this directory is the chase.

    world record   19.538   tweenTM
    ours           20.386   certified
    gap             0.848

## TAS vs the world record, one clip

Both cars, full lap, synchronised on race time. The camera rides our car; the
world record is the second car — beside us at the start, gone up the road by
the end. It crosses the line while we are still about 100 m back.

https://github.com/user-attachments/assets/e17d5488-735a-4875-b272-fcdb603d8f3b

Rendered from the two ghosts directly (`shootctl run`, external chase cam,
1280x720, 30 fps). 21.5 s of clip for a 20.386 s lap: a MediaTracker clip is as
long as the longest ghost block, and a block follows the tape's SAMPLE SPAN,
not its declared time.

## Where the 0.848 s goes

Measured along the record's own path, not at shared instants — comparing two
runs at the same moment answers "who is further down the road", which is the
wrong question.

| | |
|---|---|
| first 780 m | **we are level** — 13 ms behind at race 10.7 of a 19.5 s lap |
| 780 m → 1300 m | **686 ms lost**, in 4.4 s |
| last 530 m | 319 ms, bled at ~50 ms per 100 m with the speed deficit flat |

**One corner decides it**, at x≈990 / z≈1105. Both cars hold *identical inputs*
across it — full left lock, full throttle, no brake, every single sample. The
record enters 4 m tighter and 12 km/h slower and comes out **+11 km/h**; we
enter wide and hot and scrub **64 km/h**. Nothing is hit: one wall contact each,
same corner, same magnitude.

It is a line we never drove, not a speed-versus-time trade-off. The record is
ahead on time *and* slower in speed at that same place, because its line is
shorter — and it is never behind us anywhere on the lap.

## How the lap was built

Two instruments, and the second is the one that surprised us.

1. **A beam search over racing lines**, RL-trained per gate, 22 gates. Took the
   lap to 20.580 and converged — no further milliseconds.
2. **A byte-level search on the input tape** (`tmsearch`), seeded from that
   converged lap. Took **194 ms** more out of it, finding time in *every*
   sector, and certified at 20.386.

A policy can only emit what its architecture permits; the tape search writes
input bytes, so it produces laps no policy of that shape would — a one-tick
flick, a 30-tick ramp with no observable cause. **Use RL for the route and the
tape search for the lap; do not wait for RL to run dry first.** Then it stops
sharply: 194 ms in 3.5 hours, then 0 ms in 28 million further evaluations
across three temperatures.

## Certification

Every banked time cleared the same four checks, and the intermediate ones did
too before seeding the next round:

- `tmauto verdict --repeat 3 --require-finish` — STABLE 3/3, 4 checkpoints
- `ghost tape authored` at the true finish — PASS
- the **same bytes** judged at a deliberately false finish — **REFUSED**
- the previous certified lap re-passing on the same box — PASS

~62 million evaluations over the campaign, 100+ confirmed improvements, and
**zero phantoms**: not once did the search's own evaluator and the plain oracle
disagree about a candidate.
